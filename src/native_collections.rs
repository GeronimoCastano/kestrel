//! Persistent collection storage shares immutable entries without copying class values.
use super::{c_type, suffix};
use crate::core::{LayoutKind, Program, Ty};
use std::fmt::Write;

fn scalar(ty: Ty) -> bool {
    matches!(ty, Ty::Int | Ty::Nat | Ty::Double | Ty::Bool)
}

pub(super) fn declare_data(program: &Program, out: &mut String) {
    for (id, layout) in program.layouts.iter().enumerate() {
        if let LayoutKind::List(element) = layout.kind {
            if scalar(element) {
                writeln!(out,"typedef struct KListCell_{id} {{ {} value; }} KListCell_{id};\nstruct KListData_{id} {{ atomic_size_t references; size_t length,capacity; KListCell_{id} *storage; }};",c_type(element)).unwrap();
                continue;
            }
            writeln!(out,"typedef struct KListCell_{id} {{ atomic_size_t references; {} value; void *owner; void (*release_owner)(void *,KDropStack *); }} KListCell_{id};",c_type(element)).unwrap();
            writeln!(
                out,
                "static void k_list_{id}_cell_drop_task(void *,KDropStack *);"
            )
            .unwrap();
            writeln!(out,"struct KListData_{id} {{ atomic_size_t references; size_t length, capacity; KListCell_{id} **items; }};").unwrap();
        }
    }
}
pub(super) fn drop_body(id: usize, element: Ty, out: &mut String) {
    if scalar(element) {
        writeln!(out,"    struct KListData_{id} *data=value->handle;value->handle=NULL;if(data && atomic_fetch_sub_explicit(&data->references,1,memory_order_acq_rel)==1) {{free(data->storage);free(data);}}").unwrap();
        return;
    }
    writeln!(out,"    struct KListData_{id} *data=value->handle;value->handle=NULL;if(data && atomic_fetch_sub_explicit(&data->references,1,memory_order_acq_rel)==1) {{k_drop_push(stack,data,k_drop_free);k_drop_push(stack,data->items,k_drop_free);for(size_t i=0;i<data->length;++i) k_drop_run(data->items[i],k_list_{id}_cell_drop_task);}}").unwrap();
}
fn emit_list_at(program: &Program, id: usize, element: Ty, c: &str, out: &mut String) {
    let access = if scalar(element) {
        "list.handle->storage[index].value"
    } else {
        "list.handle->items[index]->value"
    };
    let value = crate::native::static_copy(program, element, access);
    // A Map or closure element's copy function is emitted after the lists.
    if let Ty::Record(other) = element
        && value.contains(&format!("k_record_{other}_copy("))
    {
        writeln!(
            out,
            "static KRecord_{other} k_record_{other}_copy(KRecord_{other});"
        )
        .unwrap();
    }
    writeln!(
        out,
        "static {c} k_list_{id}_at(KRecord_{id} list, int64_t index, KLocation at) {{ if (!list.handle || index < 0 || (size_t)index >= list.handle->length) k_trap(at, \"List index out of bounds\"); return {value}; }}"
    )
    .unwrap();
}

pub(super) fn emit(program: &Program, out: &mut String) {
    for (id, layout) in program.layouts.iter().enumerate() {
        let LayoutKind::List(element) = layout.kind else {
            continue;
        };
        let c = c_type(element);
        if scalar(element) {
            emit_scalar(id, &c, out);
            emit_list_sort(program, id, element, out);
            emit_list_at(program, id, element, &c, out);
            continue;
        }
        emit_list_at(program, id, element, &c, out);
        writeln!(out,"static void k_list_{id}_cell_drop_task(void *payload,KDropStack *stack) {{KListCell_{id} *cell=payload;if(atomic_fetch_sub_explicit(&cell->references,1,memory_order_acq_rel)==1) {{if(cell->owner) cell->release_owner(cell->owner,stack);").unwrap();
        if element.owns() {
            writeln!(
                out,
                "else k_{}_drop_task(&cell->value,stack);",
                suffix(element)
            )
            .unwrap();
        }
        writeln!(out, "k_small_free(cell,sizeof *cell);}} }}").unwrap();
        writeln!(out,"static KRecord_{id} k_record_{id}_copy(KRecord_{id} value) {{ if (value.handle && atomic_fetch_add_explicit(&value.handle->references,1,memory_order_relaxed) == SIZE_MAX) k_fatal(\"List reference count overflow\"); return value; }}").unwrap();
        writeln!(out,"static void k_list_{id}_push_cell(KRecord_{id} *list, KListCell_{id} *cell) {{ struct KListData_{id} *data = list->handle; if (!data) {{ data = calloc(1,sizeof *data); if (!data) k_fatal(\"Out of memory\"); atomic_init(&data->references,1); list->handle = data; }} if (atomic_load_explicit(&data->references,memory_order_relaxed) != 1) k_fatal(\"A shared immutable List cannot be changed\"); if (data->length >= INT64_MAX) k_fatal(\"List length exceeds Int\"); if (data->length == data->capacity) {{ size_t capacity = data->capacity ? data->capacity*2 : 8; if (capacity < data->capacity || capacity > SIZE_MAX/sizeof *data->items) k_fatal(\"List is too large\"); KListCell_{id} **items = realloc(data->items,capacity*sizeof *items); if (!items) k_fatal(\"Out of memory\"); data->items = items; data->capacity = capacity; }} data->items[data->length++] = cell; }}").unwrap();
        writeln!(out,"static void k_list_{id}_append(KRecord_{id} *list, {c} owned) {{ KListCell_{id} *cell = k_small_alloc(sizeof *cell); atomic_init(&cell->references,1); cell->value = owned; cell->owner=NULL;cell->release_owner=NULL;k_list_{id}_push_cell(list,cell); }}").unwrap();
        writeln!(out,"static void k_list_{id}_append_view(KRecord_{id} *list,{c} value,void *owner,void (*release)(void *,KDropStack *)) {{KListCell_{id} *cell=k_small_alloc(sizeof *cell);atomic_init(&cell->references,1);cell->value=value;cell->owner=owner;cell->release_owner=release;k_list_{id}_push_cell(list,cell);}}").unwrap();
        writeln!(out,"static void k_list_{id}_append_from(KRecord_{id} *list, KRecord_{id} source, int64_t index) {{ KListCell_{id} *cell = source.handle->items[index]; if (atomic_fetch_add_explicit(&cell->references,1,memory_order_relaxed) == SIZE_MAX) k_fatal(\"List entry reference count overflow\"); k_list_{id}_push_cell(list,cell); }}").unwrap();
        writeln!(out,"static KRecord_{id} k_list_{id}_slice(KRecord_{id} source,size_t skip,size_t keep) {{KRecord_{id} result={{0}};if(source.handle) for(size_t i=skip;i+keep<source.handle->length;++i) k_list_{id}_append_from(&result,source,(int64_t)i);return result;}}\nstatic KRecord_{id} k_list_{id}_range(KRecord_{id} source,int64_t start,int64_t stop) {{KRecord_{id} result={{0}};int64_t length=source.handle?(int64_t)source.handle->length:0;if(start<0) start=start+length<0?0:start+length;if(stop<0) stop=stop+length<0?0:stop+length;if(start>length) start=length;if(stop>length) stop=length;for(int64_t i=start;i<stop;++i) k_list_{id}_append_from(&result,source,i);return result;}}\nstatic KRecord_{id} k_list_{id}_reversed(KRecord_{id} source) {{KRecord_{id} result={{0}};if(source.handle) for(size_t i=source.handle->length;i>0;--i) k_list_{id}_append_from(&result,source,(int64_t)(i-1));return result;}}\nstatic KRecord_{id} k_list_{id}_concat(KRecord_{id} left,KRecord_{id} right) {{KRecord_{id} result=k_list_{id}_slice(left,0,0);if(right.handle) for(size_t i=0;i<right.handle->length;++i) k_list_{id}_append_from(&result,right,(int64_t)i);return result;}}").unwrap();
        writeln!(out,"static KRecord_{id} k_list_{id}_new(size_t count, {c} *owned) {{ KRecord_{id} list = {{0}}; for (size_t i = 0; i < count; ++i) k_list_{id}_append(&list,owned[i]); return list; }}").unwrap();
        if element == Ty::String {
            // split() splits on runs of whitespace; splitlines() splits at line breaks (Q136).
            writeln!(out,"static __attribute__((unused)) KRecord_{id} k_list_{id}_split_space(KString text) {{ KRecord_{id} list = {{0}}; size_t i = 0; while (i < text.length) {{ size_t next = i; uint32_t code = k_utf8_next(text,&next); if (k_unicode_is_blank(code)) {{ i = next; continue; }} size_t begin = i; i = next; while (i < text.length) {{ size_t look = i; code = k_utf8_next(text,&look); if (k_unicode_is_blank(code)) break; i = look; }} k_list_{id}_append(&list,k_string_slice_shared(text,begin,i)); }} return list; }}").unwrap();
            writeln!(out,"static __attribute__((unused)) KRecord_{id} k_list_{id}_split_lines(KString text) {{ KRecord_{id} list = {{0}}; size_t begin = 0, i = 0; while (i < text.length) {{ size_t next = i; uint32_t code = k_utf8_next(text,&next); if (code == '\\r' && next < text.length && text.data[next] == '\\n') {{ k_list_{id}_append(&list,k_string_slice_shared(text,begin,i)); begin = i = next + 1; continue; }} if (code == '\\n' || code == '\\r' || code == 0x0b || code == 0x0c || code == 0x1c || code == 0x1d || code == 0x1e || code == 0x85 || code == 0x2028 || code == 0x2029) {{ k_list_{id}_append(&list,k_string_slice_shared(text,begin,i)); begin = next; }} i = next; }} if (begin < text.length) k_list_{id}_append(&list,k_string_slice_shared(text,begin,text.length)); return list; }}").unwrap();
            writeln!(out,"static KRecord_{id} k_list_{id}_split(KString text, KString separator) {{ KRecord_{id} list = {{0}}; if (!separator.length) {{ for (size_t i = 0; i < text.length;) {{ size_t begin = i; k_utf8_next(text,&i); k_list_{id}_append(&list,k_string_slice_shared(text,begin,i)); }} return list; }} if (separator.length > text.length) {{ k_list_{id}_append(&list,k_string_copy(text)); return list; }} if (separator.length > SIZE_MAX/sizeof(size_t)) k_fatal(\"String is too large\"); size_t *prefix = malloc(separator.length*sizeof *prefix); if (!prefix) k_fatal(\"Out of memory\"); prefix[0] = 0; for (size_t i = 1, n = 0; i < separator.length; ++i) {{ while (n && separator.data[i] != separator.data[n]) n = prefix[n-1]; if (separator.data[i] == separator.data[n]) ++n; prefix[i] = n; }} size_t begin = 0; for (size_t i = 0, n = 0; i < text.length; ++i) {{ while (n && text.data[i] != separator.data[n]) n = prefix[n-1]; if (text.data[i] == separator.data[n]) ++n; if (n == separator.length) {{ k_list_{id}_append(&list,k_string_slice_shared(text,begin,i+1-separator.length)); begin = i+1; n = 0; }} }} free(prefix); k_list_{id}_append(&list,k_string_slice_shared(text,begin,text.length)); return list; }}").unwrap();
        }
        emit_list_sort(program, id, element, out);
    }
}

/// The C expression ordering two items of an element type: negative, zero or positive. Records use their
/// compare helper, and None means the type has no order (Q123).
pub(super) fn order_expression(
    program: &Program,
    element: Ty,
    left: &str,
    right: &str,
) -> Option<String> {
    Some(match element {
        Ty::Int | Ty::Nat | Ty::Bool => {
            format!("((int64_t)({left}>{right})-(int64_t)({left}<{right}))")
        }
        Ty::Double => format!("k_compare_double({left},{right})"),
        Ty::String => format!("k_compare_string({left},{right})"),
        Ty::Record(id) => {
            let function = program.sort_comparators.get(&id)?;
            let reference = matches!(program.functions[*function].parameters[0].1, Ty::Ref(_));
            if reference {
                format!("k_fn_{function}(&{left},&{right})")
            } else {
                format!("k_fn_{function}({left},{right})")
            }
        }
        _ => return None,
    })
}

/// sorted() on a List: the indices of the items are stably sorted, then the new List shares the cells in that
/// order (Q123). The helpers are unused in programs that never sort, so they carry the unused attribute.
fn emit_list_sort(program: &Program, id: usize, element: Ty, out: &mut String) {
    // A List of (key, item) pairs sorts by the key, which is the pair's first field (Q135).
    if let Ty::Record(tuple) = element
        && matches!(program.layouts[tuple].kind, LayoutKind::Tuple)
    {
        emit_pair_sort(program, id, tuple, out);
        // A tuple that orders by its parts also sorts as a whole (REVIEW2 section 3).
        if !program.sort_comparators.contains_key(&tuple) {
            return;
        }
    }
    let left = if scalar(element) {
        "list->handle->storage[left].value"
    } else {
        "list->handle->items[left]->value"
    };
    let right = if scalar(element) {
        "list->handle->storage[right].value"
    } else {
        "list->handle->items[right]->value"
    };
    let Some(order) = order_expression(program, element, left, right) else {
        return;
    };
    writeln!(out,"static __attribute__((unused)) int64_t k_list_{id}_compare_at(void *context, size_t left, size_t right) {{ KRecord_{id} *list = context; return {order}; }}").unwrap();
    writeln!(out,"static __attribute__((unused)) KRecord_{id} k_list_{id}_sorted(KRecord_{id} source) {{ KRecord_{id} result = {{0}}; size_t length = source.handle ? (size_t)source.handle->length : 0; if (!length) return result; size_t *order = malloc(length * sizeof *order); if (!order) k_fatal(\"Out of memory\"); for (size_t i = 0; i < length; ++i) order[i] = i; k_sort_indices(order, length, k_list_{id}_compare_at, &source); for (size_t i = 0; i < length; ++i) k_list_{id}_append_from(&result, source, (int64_t)order[i]); free(order); return result; }}").unwrap();
}

/// sorted(key = f) on a List of (key, item) pairs: the pairs are stably sorted by their keys (Q135).
fn emit_pair_sort(program: &Program, id: usize, tuple: usize, out: &mut String) {
    let key = program.layouts[tuple].fields[0].ty;
    let Some(order) = order_expression(
        program,
        key,
        "list->handle->items[left]->value.f0",
        "list->handle->items[right]->value.f0",
    ) else {
        return;
    };
    writeln!(out,"static __attribute__((unused)) int64_t k_list_{id}_key_compare_at(void *context, size_t left, size_t right) {{ KRecord_{id} *list = context; return {order}; }}").unwrap();
    writeln!(out,"static __attribute__((unused)) KRecord_{id} k_list_{id}_sorted_by_key(KRecord_{id} source) {{ KRecord_{id} result = {{0}}; size_t length = source.handle ? (size_t)source.handle->length : 0; if (!length) return result; size_t *order = malloc(length * sizeof *order); if (!order) k_fatal(\"Out of memory\"); for (size_t i = 0; i < length; ++i) order[i] = i; k_sort_indices(order, length, k_list_{id}_key_compare_at, &source); for (size_t i = 0; i < length; ++i) k_list_{id}_append_from(&result, source, (int64_t)order[i]); free(order); return result; }}").unwrap();
}

/// sort() on an Array: the items are permuted into their stable order in place (Q123).
pub(super) fn emit_array_sort(
    program: &Program,
    id: usize,
    element: Ty,
    c: &str,
    out: &mut String,
) {
    if matches!(element, Ty::Int | Ty::Nat) {
        // The radix path sorts the integers themselves, without an index
        // permutation or a callback per comparison, and stays stable.
        writeln!(out,"static __attribute__((unused)) void k_array_{id}_sort(KRecord_{id} *array) {{ k_sort_int64_stable((int64_t *)array->items, (size_t)array->length); }}").unwrap();
        return;
    }
    let Some(order) = order_expression(
        program,
        element,
        "array->items[left]",
        "array->items[right]",
    ) else {
        return;
    };
    writeln!(out,"static __attribute__((unused)) int64_t k_array_{id}_compare_at(void *context, size_t left, size_t right) {{ KRecord_{id} *array = context; return {order}; }}").unwrap();
    writeln!(out,"static __attribute__((unused)) void k_array_{id}_sort(KRecord_{id} *array) {{ size_t length = (size_t)array->length; if (length < 2) return; size_t *order = malloc(length * sizeof *order); if (!order) k_fatal(\"Out of memory\"); {c} *items = malloc(length * sizeof *items); if (!items) k_fatal(\"Out of memory\"); for (size_t i = 0; i < length; ++i) order[i] = i; k_sort_indices(order, length, k_array_{id}_compare_at, array); for (size_t i = 0; i < length; ++i) items[i] = array->items[order[i]]; memcpy(array->items, items, length * sizeof *items); free(items); free(order); }}").unwrap();
}

/// Scalar entries have no independently owned payload. Slices copy their values,
/// so a List needs only contiguous storage rather than one allocation per entry.
fn emit_scalar(id: usize, c: &str, out: &mut String) {
    writeln!(out,"static KRecord_{id} k_record_{id}_copy(KRecord_{id} value) {{if(value.handle && atomic_fetch_add_explicit(&value.handle->references,1,memory_order_relaxed)==SIZE_MAX) k_fatal(\"List reference count overflow\");return value;}}").unwrap();
    writeln!(out,"static void k_list_{id}_append(KRecord_{id} *list,{c} value) {{\n    struct KListData_{id} *data=list->handle;\n    if(!data) {{data=calloc(1,sizeof *data);if(!data) k_fatal(\"Out of memory\");atomic_init(&data->references,1);list->handle=data;}}\n    if(atomic_load_explicit(&data->references,memory_order_relaxed)!=1) k_fatal(\"A shared immutable List cannot be changed\");\n    if(data->length>=INT64_MAX) k_fatal(\"List length exceeds Int\");\n    if(data->length==data->capacity) {{size_t capacity=data->capacity ? data->capacity*2 : 8;if(capacity<data->capacity || capacity>SIZE_MAX/sizeof *data->storage) k_fatal(\"List is too large\");\n        KListCell_{id} *storage=realloc(data->storage,capacity*sizeof *storage);if(!storage) k_fatal(\"Out of memory\");data->storage=storage;data->capacity=capacity;}}\n    data->storage[data->length++].value=value;\n}}").unwrap();
    writeln!(out,"static void k_list_{id}_append_from(KRecord_{id} *list,KRecord_{id} source,int64_t index) {{k_list_{id}_append(list,source.handle->storage[index].value);}}\nstatic void k_list_{id}_append_view(KRecord_{id} *list,{c} value,void *owner,void (*release)(void *,KDropStack *)) {{k_list_{id}_append(list,value);if(owner) k_drop_run(owner,release);}}\nstatic KRecord_{id} k_list_{id}_slice(KRecord_{id} source,size_t skip,size_t keep) {{KRecord_{id} result={{0}};if(source.handle) for(size_t i=skip;i+keep<source.handle->length;++i) k_list_{id}_append_from(&result,source,(int64_t)i);return result;}}\nstatic KRecord_{id} k_list_{id}_range(KRecord_{id} source,int64_t start,int64_t stop) {{KRecord_{id} result={{0}};int64_t length=source.handle?(int64_t)source.handle->length:0;if(start<0) start=start+length<0?0:start+length;if(stop<0) stop=stop+length<0?0:stop+length;if(start>length) start=length;if(stop>length) stop=length;for(int64_t i=start;i<stop;++i) k_list_{id}_append_from(&result,source,i);return result;}}\nstatic KRecord_{id} k_list_{id}_reversed(KRecord_{id} source) {{KRecord_{id} result={{0}};if(source.handle) for(size_t i=source.handle->length;i>0;--i) k_list_{id}_append_from(&result,source,(int64_t)(i-1));return result;}}\nstatic KRecord_{id} k_list_{id}_concat(KRecord_{id} left,KRecord_{id} right) {{KRecord_{id} result=k_list_{id}_slice(left,0,0);if(right.handle) for(size_t i=0;i<right.handle->length;++i) k_list_{id}_append_from(&result,right,(int64_t)i);return result;}}\nstatic KRecord_{id} k_list_{id}_new(size_t count,{c} *values) {{KRecord_{id} result={{0}};for(size_t i=0;i<count;++i) k_list_{id}_append(&result,values[i]);return result;}}").unwrap();
    writeln!(out,"static KRecord_{id} k_list_{id}_blank(size_t count) {{ KRecord_{id} result={{0}}; struct KListData_{id} *data=calloc(1,sizeof *data); if(!data) k_fatal(\"Out of memory\"); atomic_init(&data->references,1); data->length=data->capacity=count; result.handle=data; if(count) {{ data->storage=malloc(count*sizeof *data->storage); if(!data->storage) k_fatal(\"Out of memory\"); }} return result; }}").unwrap();
}
