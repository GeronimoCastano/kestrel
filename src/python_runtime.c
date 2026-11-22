/* Embedded CPython bridge. Foreign calls serialize even when an extension
   releases the interpreter lock (Q47). Busy worker callbacks fail (Q84). */
typedef struct { PyObject *object; } KPython;
static pthread_once_t k_python_once = PTHREAD_ONCE_INIT;
static atomic_bool k_python_callbacks_open = true;
static pthread_mutex_t k_python_callback_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t k_python_callback_done = PTHREAD_COND_INITIALIZER;
static size_t k_python_callbacks_active;
static _Thread_local size_t k_python_callback_depth;
/* Generated code reaches the depth through these: thread-local data cannot leave a
   Windows DLL, so a session's entries cannot name it. */
static size_t k_python_depth(void) { return k_python_callback_depth; }
static void k_python_set_depth(size_t depth) { k_python_callback_depth = depth; }
static pthread_mutex_t k_python_bridge;
static void k_python_start(void) {
    pthread_mutexattr_t attributes;
    if (pthread_mutexattr_init(&attributes) || pthread_mutexattr_settype(&attributes, PTHREAD_MUTEX_RECURSIVE)
        || pthread_mutex_init(&k_python_bridge, &attributes)) k_fatal("Cannot initialize the Python bridge");
    pthread_mutexattr_destroy(&attributes);
    Py_InitializeEx(0);
    /* Release the interpreter lock so any thread can take it with PyGILState_Ensure. */
    PyEval_SaveThread();
}
static PyGILState_STATE k_python_enter(void) {
    if (pthread_once(&k_python_once, k_python_start)) k_fatal("Cannot start Python");
    /* A Python callback already holds the interpreter lock. Release it before
       waiting for the bridge so another thread can finish its current call. */
    PyThreadState *thread = PyGILState_Check() ? PyEval_SaveThread() : NULL;
    k_pthread_check(pthread_mutex_lock(&k_python_bridge));
    if (thread) PyEval_RestoreThread(thread);
    return PyGILState_Ensure();
}
static void k_python_leave(PyGILState_STATE state) {
    PyGILState_Release(state);
    k_pthread_check(pthread_mutex_unlock(&k_python_bridge));
}
/* Q84: a callback must not wait for another thread's active bridge call.
   The recursive mutex still permits same-thread calls and idle worker calls. */
static bool k_python_call_enter(PyGILState_STATE *state) {
    if (!k_python_callback_depth) { *state = k_python_enter(); return true; }
    int status = pthread_mutex_trylock(&k_python_bridge);
    if (status == EBUSY) return false;
    k_pthread_check(status);
    *state = PyGILState_Ensure();
    return true;
}
/* Python calls these while holding its interpreter lock. The counter keeps
   session globals alive until every callback already in progress has returned. */
static bool k_python_callback_begin(void) {
    k_pthread_check(pthread_mutex_lock(&k_python_callback_mutex));
    if (!atomic_load_explicit(&k_python_callbacks_open,memory_order_acquire)) {
        k_pthread_check(pthread_mutex_unlock(&k_python_callback_mutex));
        PyErr_SetString(PyExc_RuntimeError,"the Kestrel session has ended");
        return false;
    }
    if (k_python_callbacks_active==SIZE_MAX) k_fatal("Too many Python callbacks");
    ++k_python_callbacks_active;
    ++k_python_callback_depth;
    k_pthread_check(pthread_mutex_unlock(&k_python_callback_mutex));
    return true;
}
static void k_python_callback_end(void) {
    --k_python_callback_depth;
    k_pthread_check(pthread_mutex_lock(&k_python_callback_mutex));
    if (!--k_python_callbacks_active) k_pthread_check(pthread_cond_broadcast(&k_python_callback_done));
    k_pthread_check(pthread_mutex_unlock(&k_python_callback_mutex));
}
static void k_python_close_callbacks(void) {
    if (!Py_IsInitialized()) return;
    PyGILState_STATE state=k_python_enter();
    atomic_store_explicit(&k_python_callbacks_open,false,memory_order_release);
    k_python_leave(state);
    k_pthread_check(pthread_mutex_lock(&k_python_callback_mutex));
    while (k_python_callbacks_active) k_pthread_check(pthread_cond_wait(&k_python_callback_done,&k_python_callback_mutex));
    k_pthread_check(pthread_mutex_unlock(&k_python_callback_mutex));
}
static void k_python_drop(KPython *value) {
    PyObject *object = value->object;
    value->object = NULL;
    if (object) {
        /* A callback or capsule destructor already runs inside Python. Its
           reference work must not wait for a bridge call joining this thread. */
        if (PyGILState_Check()) { Py_DECREF(object); return; }
        if (k_python_callback_depth) {
            PyGILState_STATE state=PyGILState_Ensure();
            Py_DECREF(object); PyGILState_Release(state); return;
        }
        PyGILState_STATE state = k_python_enter();
        Py_DECREF(object);
        k_python_leave(state);
    }
}
static void k_python_drop_task(void *value, KDropStack *stack) { (void)stack; k_python_drop((KPython *)value); }
/* A borrowed call holds its own reference for the callee's frame. The Python
   object stays shared; this does not clone or invoke it. */
static KPython k_python_copy(KPython value) {
    if (value.object) {
        if (PyGILState_Check()) { Py_INCREF(value.object); return value; }
        if (k_python_callback_depth) {
            PyGILState_STATE state=PyGILState_Ensure();
            Py_INCREF(value.object); PyGILState_Release(state); return value;
        }
        PyGILState_STATE state = k_python_enter();
        Py_INCREF(value.object);
        k_python_leave(state);
    }
    return value;
}
static KPython k_python_take(KPython *value) { KPython result = *value; value->object = NULL; return result; }
static void k_python_set(KPython *target, KPython owned) { k_python_drop(target); *target = owned; }
static KString k_python_text(PyObject *object) {
    PyObject *text = object ? PyObject_Str(object) : NULL;
    Py_ssize_t length = 0;
    const char *bytes = text ? PyUnicode_AsUTF8AndSize(text, &length) : NULL;
    KString result = bytes ? k_string_bytes(bytes, (size_t)length) : K_TEXT("<unprintable>");
    if (!bytes) PyErr_Clear();
    Py_XDECREF(text);
    return result;
}
/* The pending Python exception becomes RuntimeError.Python("Type: message"). */
static KError k_python_error(void) {
    PyObject *type = NULL, *value = NULL, *trace = NULL;
    PyErr_Fetch(&type, &value, &trace);
    PyErr_NormalizeException(&type, &value, &trace);
    const char *name = type ? ((PyTypeObject *)type)->tp_name : "Error";
    KString message = k_python_text(value);
    KString pieces[] = { { name, strlen(name), NULL }, K_TEXT(": "), message };
    KString text = k_string_join(3, pieces);
    k_string_drop(&message);
    Py_XDECREF(type);
    Py_XDECREF(value);
    Py_XDECREF(trace);
    return k_runtime_error(4, text);
}
static bool k_python_mismatch(KString *why, const char *expected, PyObject *object) {
    const char *found = Py_TYPE(object)->tp_name;
    KString pieces[] = { K_TEXT("expected "), { expected, strlen(expected), NULL }, K_TEXT(", found "), { found, strlen(found), NULL } };
    *why = k_string_join(4, pieces);
    return false;
}
static PyObject *k_py_from_int(int64_t value) { return PyLong_FromLongLong(value); }
static PyObject *k_py_from_double(double value) { return PyFloat_FromDouble(value); }
static PyObject *k_py_from_bool(bool value) { return PyBool_FromLong(value); }
static PyObject *k_py_from_string(KString value) { return PyUnicode_DecodeUTF8(value.data, (Py_ssize_t)value.length, "strict"); }
static PyObject *k_py_from_python(KPython value) {
    if (!value.object) { PyErr_SetString(PyExc_ValueError, "the PythonValue was moved"); return NULL; }
    Py_INCREF(value.object);
    return value.object;
}
static bool k_py_to_int(PyObject *object, int64_t *out, KString *why) {
    if (PyBool_Check(object)) return k_python_mismatch(why, "Int", object);
    PyObject *number = PyNumber_Index(object);
    if (!number) { PyErr_Clear(); return k_python_mismatch(why, "Int", object); }
    int overflow = 0;
    long long value = PyLong_AsLongLongAndOverflow(number, &overflow);
    Py_DECREF(number);
    if (overflow || (value == -1 && PyErr_Occurred())) { PyErr_Clear(); *why = K_TEXT("the Python int does not fit Int"); return false; }
    *out = value;
    return true;
}
static bool k_py_to_double(PyObject *object, double *out, KString *why) {
    /* No implicit conversion from Python int or bool (Q46). */
    if (PyBool_Check(object) || PyLong_Check(object) || PyIndex_Check(object)) return k_python_mismatch(why, "double", object);
    if (!PyFloat_Check(object) && !(Py_TYPE(object)->tp_as_number && Py_TYPE(object)->tp_as_number->nb_float))
        return k_python_mismatch(why, "double", object);
    double value = PyFloat_AsDouble(object);
    if (value == -1.0 && PyErr_Occurred()) { PyErr_Clear(); return k_python_mismatch(why, "double", object); }
    *out = value;
    return true;
}
static bool k_py_to_bool(PyObject *object, bool *out, KString *why) {
    const char *name = Py_TYPE(object)->tp_name;
    if (PyBool_Check(object)) { *out = object == Py_True; return true; }
    if (strcmp(name, "numpy.bool") == 0 || strcmp(name, "numpy.bool_") == 0) {
        int truth = PyObject_IsTrue(object);
        if (truth < 0) { PyErr_Clear(); return k_python_mismatch(why, "Bool", object); }
        *out = truth;
        return true;
    }
    return k_python_mismatch(why, "Bool", object);
}
static bool k_py_to_string(PyObject *object, KString *out, KString *why) {
    if (!PyUnicode_Check(object)) return k_python_mismatch(why, "String", object);
    Py_ssize_t length = 0;
    const char *bytes = PyUnicode_AsUTF8AndSize(object, &length);
    if (!bytes) { PyErr_Clear(); *why = K_TEXT("the Python string is not valid UTF-8"); return false; }
    *out = k_string_bytes(bytes, (size_t)length);
    return true;
}
static bool k_py_to_python(PyObject *object, KPython *out, KString *why) {
    (void)why;
    Py_INCREF(object);
    out->object = object;
    return true;
}
