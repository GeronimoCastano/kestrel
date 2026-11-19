//! Python boundary helpers: values are copied into Python objects and checked on the way back.
use super::{c_type, copies, quoted, suffix};
use crate::core::{LayoutKind, MapStyle, Program, Ty};
use std::{collections::BTreeSet, fmt::Write};

fn element_types(program: &Program, ty: Ty) -> Vec<Ty> {
    match ty {
        Ty::Record(id) => match &program.layouts[id].kind {
            LayoutKind::List(element) => vec![*element],
            LayoutKind::Map(info) if info.style == MapStyle::Map => vec![info.key, info.value],
            LayoutKind::Map(info) => vec![info.key],
            LayoutKind::Tuple => program.layouts[id].fields.iter().map(|f| f.ty).collect(),
            _ => vec![],
        },
        _ => vec![],
    }
}
fn collect(program: &Program, ty: Ty, found: &mut BTreeSet<Ty>) {
    // A borrowed record converts through its value converter.
    if let Ty::Ref(id) = ty {
        found.insert(ty);
        return collect(program, Ty::Record(id), found);
    }
    if found.insert(ty) {
        for element in element_types(program, ty) {
            collect(program, element, found);
        }
    }
}
fn is_scalar_helper(ty: Ty) -> bool {
    matches!(
        ty,
        Ty::Int | Ty::Double | Ty::Bool | Ty::String | Ty::Python
    )
}

pub(super) fn emit(program: &Program, out: &mut String) {
    if program.python_sites.is_empty() {
        return;
    }
    let mut inputs = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    for site in &program.python_sites {
        for (ty, _) in &site.arguments {
            collect(program, *ty, &mut inputs);
        }
        let (success, _) = union_parts(program, site.result);
        collect(program, success, &mut outputs);
    }
    // A callback reads its parameters from Python and hands its result back (Q54).
    for ty in inputs.clone() {
        if let Ty::Record(id) = ty
            && let LayoutKind::Closure { parameters, result } = &program.layouts[id].kind
        {
            for (parameter, _) in parameters {
                collect(program, *parameter, &mut outputs);
            }
            collect(program, callback_success(program, *result), &mut inputs);
        }
    }
    // Prototypes first: collection converters call each other.
    for ty in inputs.iter().filter(|t| !is_scalar_helper(**t)) {
        writeln!(
            out,
            "static PyObject *k_py_from_{}({} value);",
            suffix(*ty),
            c_type(*ty)
        )
        .unwrap();
    }
    for ty in outputs
        .iter()
        .filter(|t| !is_scalar_helper(**t) && **t != Ty::Unit && !matches!(t, Ty::Ref(_)))
    {
        writeln!(
            out,
            "static bool k_py_to_{}(PyObject *object, {} *out, KString *why);",
            suffix(*ty),
            c_type(*ty)
        )
        .unwrap();
    }
    for ty in inputs.iter().filter(|t| !is_scalar_helper(**t)) {
        from_python_value(program, *ty, out);
    }
    for ty in outputs.iter().filter(|t| !is_scalar_helper(**t)) {
        to_kestrel_value(program, *ty, out);
    }
    writeln!(
        out,
        "static PyObject *k_python_modules[{}];",
        program.python_modules.len().max(1)
    )
    .unwrap();
    writeln!(out, "static PyObject *k_python_module(size_t index) {{\n    static const char *names[] = {{ {} }};\n    if (!k_python_modules[index]) k_python_modules[index] = PyImport_ImportModule(names[index]);\n    return k_python_modules[index];\n}}", program.python_modules.iter().map(|m| quoted(m)).chain(std::iter::once("NULL".into())).collect::<Vec<_>>().join(", ")).unwrap();
    for (index, site) in program.python_sites.iter().enumerate() {
        site_helper(program, index, site, out);
    }
}

/// What a callback gives Python: the success part when its result can be an Error.
fn callback_success(program: &Program, result: Ty) -> Ty {
    callback_union(program, result).map_or(result, |union| union.success)
}

struct CallbackUnion {
    success: Ty,
    case: usize,
    errors: Vec<(usize, Ty)>,
}

/// A callback can name closed error enums as well as the open Error root.
fn callback_union(program: &Program, result: Ty) -> Option<CallbackUnion> {
    let Ty::Record(id) = result else { return None };
    let LayoutKind::Union(_, cases) = &program.layouts[id].kind else {
        return None;
    };
    let mut errors = vec![];
    let mut success = None;
    for (case, variant) in cases.iter().enumerate() {
        let ty = variant.fields.first().copied().unwrap_or(Ty::Unit);
        if ty == Ty::Error || matches!(ty, Ty::Record(id) if program.layouts[id].is_error) {
            errors.push((case, ty));
        } else {
            success = Some((ty, case));
        }
    }
    let (success, case) = success?;
    (!errors.is_empty()).then_some(CallbackUnion {
        success,
        case,
        errors,
    })
}

/// The success layout of a `T or Error` union, and the union's case numbers.
fn union_parts(program: &Program, result: Ty) -> (Ty, (usize, usize)) {
    let Ty::Record(id) = result else {
        unreachable!("a Python result union")
    };
    let LayoutKind::Union(parts, cases) = &program.layouts[id].kind else {
        unreachable!("a Python result union")
    };
    let error = parts
        .iter()
        .position(|p| *p == crate::types::Type::named("Error"))
        .unwrap();
    let success = 1 - error;
    (
        cases[success].fields.first().copied().unwrap_or(Ty::Unit),
        (success, error),
    )
}

fn from_python_value(program: &Program, ty: Ty, out: &mut String) {
    if let Ty::Ref(id) = ty {
        writeln!(out, "static PyObject *k_py_from_ref_{id}(KRecord_{id} *value) {{ return k_py_from_record_{id}(*value); }}").unwrap();
        return;
    }
    let Ty::Record(id) = ty else { return };
    let name = suffix(ty);
    let c = c_type(ty);
    match &program.layouts[id].kind {
        LayoutKind::List(element) => {
            let access = if matches!(element, Ty::Int | Ty::Nat | Ty::Double | Ty::Bool) {
                "value.handle->storage[i].value"
            } else {
                "value.handle->items[i]->value"
            };
            writeln!(out, "static PyObject *k_py_from_{name}({c} value) {{\n    size_t count = value.handle ? value.handle->length : 0;\n    PyObject *list = PyList_New((Py_ssize_t)count);\n    if (!list) return NULL;\n    for (size_t i = 0; i < count; ++i) {{\n        PyObject *item = k_py_from_{}({access});\n        if (!item) {{ Py_DECREF(list); return NULL; }}\n        PyList_SET_ITEM(list, (Py_ssize_t)i, item);\n    }}\n    return list;\n}}", suffix(*element)).unwrap();
        }
        LayoutKind::Map(info) => {
            let packed = super::maps::flat(*info);
            let key = if packed {
                "value.handle->entries[i].key"
            } else if info.style == MapStyle::SortedSet {
                "value.handle->ordered[i]->key"
            } else {
                "value.handle->buckets[i]->key"
            };
            let filled = if packed || info.style == MapStyle::SortedSet {
                "i < value.handle->length"
            } else {
                "i < value.handle->capacity"
            };
            let skip = if packed || info.style == MapStyle::SortedSet {
                ""
            } else {
                "if (!value.handle->buckets[i]) continue; "
            };
            if info.style == MapStyle::Map {
                let payload = if packed {
                    "value.handle->entries[i].value"
                } else {
                    "value.handle->buckets[i]->value"
                };
                writeln!(out, "static PyObject *k_py_from_{name}({c} value) {{\n    PyObject *dict = PyDict_New();\n    if (!dict || !value.handle) return dict;\n    for (size_t i = 0; {filled}; ++i) {{\n        {skip}PyObject *k = k_py_from_{}({key});\n        PyObject *v = k ? k_py_from_{}({payload}) : NULL;\n        if (!k || !v || PyDict_SetItem(dict, k, v)) {{ Py_XDECREF(k); Py_XDECREF(v); Py_DECREF(dict); return NULL; }}\n        Py_DECREF(k); Py_DECREF(v);\n    }}\n    return dict;\n}}", suffix(info.key), suffix(info.value)).unwrap();
            } else {
                writeln!(out, "static PyObject *k_py_from_{name}({c} value) {{\n    PyObject *set = PySet_New(NULL);\n    if (!set || !value.handle) return set;\n    for (size_t i = 0; {filled}; ++i) {{\n        {skip}PyObject *k = k_py_from_{}({key});\n        if (!k || PySet_Add(set, k)) {{ Py_XDECREF(k); Py_DECREF(set); return NULL; }}\n        Py_DECREF(k);\n    }}\n    return set;\n}}", suffix(info.key)).unwrap();
            }
        }
        LayoutKind::Closure { parameters, result } => {
            // The capsule holds a counted copy of the closure until Python drops the function.
            writeln!(out, "static void k_py_closure_{id}_free(PyObject *capsule) {{\n    {c} *closure = PyCapsule_GetPointer(capsule, \"kestrel.function\");\n    if (closure) {{ k_record_{id}_drop(closure); free(closure); }}\n}}").unwrap();
            let count = parameters.len();
            let owned = |ty| match ty {
                Ty::Ref(id) => Ty::Record(id),
                other => other,
            };
            writeln!(out, "static PyObject *k_py_closure_{id}_invoke(PyObject *self, PyObject *args) {{\n    {c} *closure = PyCapsule_GetPointer(self, \"kestrel.function\");\n    if (!closure) return NULL;\n    if (PyTuple_GET_SIZE(args) != {count}) {{ PyErr_Format(PyExc_TypeError, \"this Kestrel function takes {count} arguments, not %zd\", PyTuple_GET_SIZE(args)); return NULL; }}\n    KString why = k_empty;").unwrap();
            for (i, (ty, _)) in parameters.iter().enumerate() {
                let release = (0..i)
                    .filter(|j| owned(parameters[*j].0).owns())
                    .map(|j| format!("k_{}_drop(&a{j}); ", suffix(owned(parameters[j].0))))
                    .collect::<String>();
                writeln!(out, "    {} a{i};\n    if (!k_py_to_{}(PyTuple_GET_ITEM(args, {i}), &a{i}, &why)) {{ {release}PyObject *message = PyUnicode_DecodeUTF8(why.data, (Py_ssize_t)why.length, \"replace\"); k_string_drop(&why); if (message) {{ PyErr_SetObject(PyExc_TypeError, message); Py_DECREF(message); }} return NULL; }}", c_type(owned(*ty)), suffix(owned(*ty))).unwrap();
            }
            // Borrowed records stay in this frame; the callee owns other converted values.
            let values = std::iter::once("closure->env".to_owned())
                .chain((0..count).map(|i| {
                    if matches!(parameters[i].0, Ty::Ref(_)) {
                        format!("&a{i}")
                    } else if copies(program, parameters[i].0) || !parameters[i].0.owns() {
                        format!("a{i}")
                    } else {
                        format!("k_{}_take(&a{i})", suffix(parameters[i].0))
                    }
                }))
                .collect::<Vec<_>>()
                .join(", ");
            let cleanup = parameters
                .iter()
                .enumerate()
                .filter(|(_, (ty, _))| matches!(ty, Ty::Ref(_)))
                .map(|(i, (ty, _))| format!("k_{}_drop(&a{i});", suffix(owned(*ty))))
                .collect::<Vec<_>>()
                .join(" ");
            let success = callback_success(program, *result);
            if *result == Ty::Unit {
                writeln!(
                    out,
                    "    PyThreadState *thread = PyEval_SaveThread();\n    closure->code({values});\n    PyEval_RestoreThread(thread);\n    {cleanup}\n    Py_RETURN_NONE;"
                )
                .unwrap();
            } else if success != *result {
                let Ty::Record(union) = result else {
                    unreachable!()
                };
                let CallbackUnion { case, errors, .. } = callback_union(program, *result).unwrap();
                let mut error = String::from("KError error={0};switch(r.tag) {");
                for (case, ty) in errors {
                    let value = format!("r.data.c{case}.f0");
                    let boxed = match ty {
                        Ty::Error => value,
                        Ty::Record(id) => format!("(KError){{NULL,UINT64_C({}),&{value}}}", id + 1),
                        _ => unreachable!(),
                    };
                    write!(error, "case {}: error={boxed};break;", case + 1).unwrap();
                }
                error.push('}');
                let payload = if success == Ty::Unit {
                    "Py_NewRef(Py_None)".to_owned()
                } else {
                    format!("k_py_from_{}(r.data.c{case}.f0)", suffix(success))
                };
                writeln!(out, "    PyThreadState *thread = PyEval_SaveThread();\n    {} r = closure->code({values});\n    PyEval_RestoreThread(thread);\n    {cleanup}\n    PyObject *out = NULL;\n    if (r.tag == {}) out = {payload};\n    else {{ {error} KString text=k_error_text(error); PyObject *message=PyUnicode_DecodeUTF8(text.data,(Py_ssize_t)text.length,\"replace\");k_string_drop(&text);if(message) {{PyErr_SetObject(PyExc_RuntimeError,message);Py_DECREF(message);}} }}\n    k_record_{union}_drop(&r);\n    return out;", c_type(*result), case + 1).unwrap();
            } else {
                let release = if result.owns() {
                    format!("k_{}_drop(&r); ", suffix(*result))
                } else {
                    String::new()
                };
                writeln!(out, "    PyThreadState *thread = PyEval_SaveThread();\n    {} r = closure->code({values});\n    PyEval_RestoreThread(thread);\n    {cleanup}\n    PyObject *out = k_py_from_{}(r);\n    {release}return out;", c_type(*result), suffix(*result)).unwrap();
            }
            writeln!(out,"}}\nstatic PyObject *k_py_closure_{id}_call(PyObject *self,PyObject *args) {{if(!k_python_callback_begin()) return NULL;PyObject *result=k_py_closure_{id}_invoke(self,args);k_python_callback_end();return result;}}\nstatic PyMethodDef k_py_closure_{id}_def = {{ \"kestrel_function\", k_py_closure_{id}_call, METH_VARARGS, NULL }};").unwrap();
            writeln!(out, "static PyObject *k_py_from_{name}({c} value) {{\n    {c} *copy = malloc(sizeof *copy);\n    if (!copy) return PyErr_NoMemory();\n    *copy = k_record_{id}_copy(value);\n    PyObject *capsule = PyCapsule_New(copy, \"kestrel.function\", k_py_closure_{id}_free);\n    if (!capsule) {{ k_record_{id}_drop(copy); free(copy); return NULL; }}\n    PyObject *function = PyCFunction_New(&k_py_closure_{id}_def, capsule);\n    Py_DECREF(capsule);\n    return function;\n}}").unwrap();
        }
        LayoutKind::Tuple => {
            let fields = &program.layouts[id].fields;
            writeln!(out, "static PyObject *k_py_from_{name}({c} value) {{\n    PyObject *tuple = PyTuple_New({});\n    if (!tuple) return NULL;", fields.len()).unwrap();
            for (i, field) in fields.iter().enumerate() {
                writeln!(out, "    PyObject *f{i} = k_py_from_{}(value.f{i});\n    if (!f{i}) {{ Py_DECREF(tuple); return NULL; }}\n    PyTuple_SET_ITEM(tuple, {i}, f{i});", suffix(field.ty)).unwrap();
            }
            writeln!(out, "    return tuple;\n}}").unwrap();
        }
        _ => {}
    }
}

fn to_kestrel_value(program: &Program, ty: Ty, out: &mut String) {
    if ty == Ty::Unit {
        writeln!(out, "static bool k_py_to_unit(PyObject *object, KString *why) {{ if (object == Py_None) return true; return k_python_mismatch(why, \"None\", object); }}").unwrap();
        return;
    }
    let Ty::Record(id) = ty else { return };
    let name = suffix(ty);
    let c = c_type(ty);
    match &program.layouts[id].kind {
        LayoutKind::List(element) => {
            writeln!(out, "static bool k_py_to_{name}(PyObject *object, {c} *out, KString *why) {{\n    if (PyUnicode_Check(object) || PyBytes_Check(object) || PyDict_Check(object)) return k_python_mismatch(why, \"a sequence\", object);\n    PyObject *items = PySequence_Fast(object, \"\");\n    if (!items) {{ PyErr_Clear(); return k_python_mismatch(why, \"a sequence\", object); }}\n    {c} list = {{0}};\n    for (Py_ssize_t i = 0; i < PySequence_Fast_GET_SIZE(items); ++i) {{\n        {} item;\n        if (!k_py_to_{}(PySequence_Fast_GET_ITEM(items, i), &item, why)) {{ k_record_{id}_drop(&list); Py_DECREF(items); return false; }}\n        k_list_{id}_append(&list, item);\n    }}\n    Py_DECREF(items);\n    *out = list;\n    return true;\n}}", c_type(*element), suffix(*element)).unwrap();
        }
        LayoutKind::Map(info) if info.style == MapStyle::Map => {
            writeln!(out, "static bool k_py_to_{name}(PyObject *object, {c} *out, KString *why) {{\n    if (!PyMapping_Check(object) || PySequence_Check(object)) return k_python_mismatch(why, \"a mapping\", object);\n    PyObject *pairs = PyMapping_Items(object);\n    if (!pairs) {{ PyErr_Clear(); return k_python_mismatch(why, \"a mapping\", object); }}\n    {c} map = {{0}};\n    for (Py_ssize_t i = 0; i < PyList_GET_SIZE(pairs); ++i) {{\n        PyObject *pair = PyList_GET_ITEM(pairs, i);\n        {} key; {} value;\n        if (!k_py_to_{}(PyTuple_GET_ITEM(pair, 0), &key, why)) {{ k_record_{id}_drop(&map); Py_DECREF(pairs); return false; }}\n        if (!k_py_to_{}(PyTuple_GET_ITEM(pair, 1), &value, why)) {{ {}k_record_{id}_drop(&map); Py_DECREF(pairs); return false; }}\n        k_map_{id}_put_owned(&map, key, value);\n    }}\n    Py_DECREF(pairs);\n    *out = map;\n    return true;\n}}", c_type(info.key), c_type(info.value), suffix(info.key), suffix(info.value), if info.key.owns() { format!("k_{}_drop(&key); ", suffix(info.key)) } else { String::new() }).unwrap();
        }
        LayoutKind::Map(info) => {
            writeln!(out, "static bool k_py_to_{name}(PyObject *object, {c} *out, KString *why) {{\n    PyObject *iterator = PyObject_GetIter(object);\n    if (!iterator || PyUnicode_Check(object)) {{ Py_XDECREF(iterator); PyErr_Clear(); return k_python_mismatch(why, \"a set\", object); }}\n    {c} set = {{0}};\n    PyObject *item;\n    while ((item = PyIter_Next(iterator))) {{\n        {} key;\n        bool converted = k_py_to_{}(item, &key, why);\n        Py_DECREF(item);\n        if (!converted) {{ k_record_{id}_drop(&set); Py_DECREF(iterator); return false; }}\n        k_map_{id}_put_owned(&set, key, true);\n    }}\n    Py_DECREF(iterator);\n    if (PyErr_Occurred()) {{ PyErr_Clear(); k_record_{id}_drop(&set); *why = K_TEXT(\"iterating the Python set failed\"); return false; }}\n    *out = set;\n    return true;\n}}", c_type(info.key), suffix(info.key)).unwrap();
        }
        LayoutKind::Tuple => {
            let fields = &program.layouts[id].fields;
            writeln!(out, "static bool k_py_to_{name}(PyObject *object, {c} *out, KString *why) {{\n    if (!PyTuple_Check(object) || PyTuple_GET_SIZE(object) != {}) return k_python_mismatch(why, \"a tuple of {} items\", object);\n    {c} tuple = {{0}};", fields.len(), fields.len()).unwrap();
            for (i, field) in fields.iter().enumerate() {
                writeln!(out, "    if (!k_py_to_{}(PyTuple_GET_ITEM(object, {i}), &tuple.f{i}, why)) {{ k_record_{id}_drop(&tuple); return false; }}", suffix(field.ty)).unwrap();
            }
            writeln!(out, "    *out = tuple;\n    return true;\n}}").unwrap();
        }
        _ => {}
    }
}

fn site_helper(program: &Program, index: usize, site: &crate::core::PythonSite, out: &mut String) {
    let Ty::Record(result) = site.result else {
        unreachable!()
    };
    let (success_ty, (success, error)) = union_parts(program, site.result);
    let mut parameters = vec![];
    if site.module.is_none() {
        parameters.push("KPython receiver".to_owned());
    }
    for (i, (ty, _)) in site.arguments.iter().enumerate() {
        parameters.push(format!("{} a{i}", c_type(*ty)));
    }
    let parameters = if parameters.is_empty() {
        "void".into()
    } else {
        parameters.join(", ")
    };
    writeln!(out, "static KRecord_{result} k_python_site_{index}({parameters}) {{\n    PyGILState_STATE state;\n    if (!k_python_call_enter(&state)) return k_aggregate_{result}_{error}(k_runtime_error(4, K_TEXT(\"Python bridge is busy on another thread during a callback\")));\n    KRecord_{result} result;\n    PyObject *callable = NULL, *args = NULL, *kwargs = NULL, *value = NULL;").unwrap();
    match site.module {
        Some(module) => writeln!(out, "    PyObject *target = k_python_module({module});").unwrap(),
        None => writeln!(out, "    PyObject *target = receiver.object;\n    if (!target) PyErr_SetString(PyExc_ValueError, \"the PythonValue was moved\");").unwrap(),
    }
    writeln!(out, "    if (!target) goto failed;").unwrap();
    match &site.attribute {
        Some(attribute) => writeln!(
            out,
            "    callable = PyObject_GetAttrString(target, {});",
            quoted(attribute)
        )
        .unwrap(),
        None => writeln!(out, "    callable = target; Py_INCREF(callable);").unwrap(),
    }
    writeln!(out, "    if (!callable) goto failed;").unwrap();
    if site.call {
        let positional = site
            .arguments
            .iter()
            .filter(|(_, name)| name.is_none())
            .count();
        writeln!(
            out,
            "    args = PyTuple_New({positional});\n    if (!args) goto failed;"
        )
        .unwrap();
        if site.arguments.iter().any(|(_, name)| name.is_some()) {
            writeln!(
                out,
                "    kwargs = PyDict_New();\n    if (!kwargs) goto failed;"
            )
            .unwrap();
        }
        let mut position = 0;
        for (i, (ty, name)) in site.arguments.iter().enumerate() {
            writeln!(out, "    {{\n        PyObject *item = k_py_from_{}(a{i});\n        if (!item) goto failed;", suffix(*ty)).unwrap();
            match name {
                None => {
                    writeln!(out, "        PyTuple_SET_ITEM(args, {position}, item);").unwrap();
                    position += 1;
                }
                Some(name) => writeln!(out, "        int status = PyDict_SetItemString(kwargs, {}, item);\n        Py_DECREF(item);\n        if (status) goto failed;", quoted(name)).unwrap(),
            }
            writeln!(out, "    }}").unwrap();
        }
        writeln!(out, "    value = PyObject_Call(callable, args, kwargs);").unwrap();
    } else {
        writeln!(out, "    value = callable; callable = NULL;").unwrap();
    }
    writeln!(out, "    if (!value) goto failed;").unwrap();
    if success_ty == Ty::Unit {
        writeln!(out, "    {{\n        KString why = k_empty;\n        if (k_py_to_unit(value, &why)) result = k_aggregate_{result}_{success}();\n        else result = k_aggregate_{result}_{error}(k_runtime_error(4, why));\n    }}").unwrap();
    } else {
        writeln!(out, "    {{\n        KString why = k_empty;\n        {} converted;\n        if (k_py_to_{}(value, &converted, &why)) result = k_aggregate_{result}_{success}(converted);\n        else result = k_aggregate_{result}_{error}(k_runtime_error(4, why));\n    }}", c_type(success_ty), suffix(success_ty)).unwrap();
    }
    writeln!(out, "    goto done;\nfailed:\n    result = k_aggregate_{result}_{error}(k_python_error());\ndone:\n    Py_XDECREF(value);\n    Py_XDECREF(callable);\n    Py_XDECREF(args);\n    Py_XDECREF(kwargs);\n    k_python_leave(state);\n    return result;\n}}").unwrap();
}
