#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
/* Internal runtime for the typed C backend. */
#include <stdbool.h>
#include <stdint.h>
#include <inttypes.h>
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#include <float.h>
#include <stdatomic.h>
#include <limits.h>
#include <errno.h>
#include <locale.h>
#include <setjmp.h>
#include <signal.h>
#ifndef _WIN32
#include <pthread.h>
#include <sched.h>
#include <unistd.h>
#endif

typedef struct { const char *file; size_t line, column; const char *expression; } KLocation;
_Noreturn static void k_fatal(const char *message) {
    fprintf(stderr, "kestrel: %s\n", message);
    exit(1);
}
/* A sanitizer build reports stack overflows itself and owns each thread's signal stack. */
#if defined(__has_feature)
#if __has_feature(address_sanitizer) || __has_feature(thread_sanitizer)
#define K_SANITIZED 1
#endif
#endif
#if !defined(K_SANITIZED) && defined(__SANITIZE_ADDRESS__)
#define K_SANITIZED 1
#endif
#ifndef K_SANITIZED
#define K_SANITIZED 0
#endif
#ifdef _WIN32
/* Windows raises EXCEPTION_STACK_OVERFLOW on the thread that ran out. Each thread keeps
   a stack guarantee so the handler has room to report it. */
static LONG CALLBACK k_overflow_handler(EXCEPTION_POINTERS *exception) {
    if (exception->ExceptionRecord->ExceptionCode != EXCEPTION_STACK_OVERFLOW) return EXCEPTION_CONTINUE_SEARCH;
    static const char message[] =
        "kestrel: stack overflow: a chain of calls went too deep, probably unbounded recursion\n";
    DWORD written = 0;
    WriteFile(GetStdHandle(STD_ERROR_HANDLE), message, sizeof message - 1, &written, NULL);
    TerminateProcess(GetCurrentProcess(), 1);
    return EXCEPTION_CONTINUE_SEARCH;
}
static void *k_overflow_stack_install(void) __attribute__((unused));
static void *k_overflow_stack_install(void) {
    ULONG reserve = 1 << 16;
    if (!K_SANITIZED) SetThreadStackGuarantee(&reserve);
    return NULL;
}
static void k_overflow_stack_remove(void *stack) __attribute__((unused));
static void k_overflow_stack_remove(void *stack) { (void)stack; }
static void k_overflow_guard(void) __attribute__((unused));
static void k_overflow_guard(void) {
    if (K_SANITIZED) return;
    k_overflow_stack_install();
    AddVectoredExceptionHandler(1, k_overflow_handler);
}
/* The console shows UTF-8 only in code page 65001; restore the caller's page at exit. */
static UINT k_console_page;
static void k_console_restore(void) { if (k_console_page) SetConsoleOutputCP(k_console_page); }
/* Output is bytes, as on POSIX: no CRLF translation on standard output or error. */
static void k_platform_start(void) __attribute__((unused));
static void k_platform_start(void) {
    _setmode(_fileno(stdout), _O_BINARY);
    _setmode(_fileno(stderr), _O_BINARY);
    DWORD mode;
    if (GetConsoleMode(GetStdHandle(STD_OUTPUT_HANDLE), &mode) || GetConsoleMode(GetStdHandle(STD_ERROR_HANDLE), &mode)) {
        UINT page = GetConsoleOutputCP();
        if (page != CP_UTF8 && SetConsoleOutputCP(CP_UTF8)) {
            k_console_page = page;
            atexit(k_console_restore);
        }
    }
}
#else
static void k_platform_start(void) __attribute__((unused));
static void k_platform_start(void) {}
/* A Kestrel program cannot dereference a bad pointer, so a segmentation fault means the
   stack ran out. Report it from a separate signal stack instead of dying silently. */
static void k_overflow_handler(int signal_number) {
    (void)signal_number;
    static const char message[] =
        "kestrel: stack overflow: a chain of calls went too deep, probably unbounded recursion\n";
    ssize_t written = write(2, message, sizeof message - 1);
    (void)written;
    _exit(1);
}
static void *k_overflow_stack_install(void) __attribute__((unused));
static void *k_overflow_stack_install(void) {
    /* A sanitizer or another runtime may already own this thread's signal stack;
       keep it rather than replacing it (REVIEW2 1.12). */
    if (K_SANITIZED) return NULL;
    stack_t current;
    if (sigaltstack(NULL, &current) == 0 && !(current.ss_flags & SS_DISABLE)) return NULL;
    stack_t alternate;
    alternate.ss_size = 1 << 16;
    alternate.ss_sp = malloc(alternate.ss_size);
    alternate.ss_flags = 0;
    if (!alternate.ss_sp) return NULL;
    if (sigaltstack(&alternate, NULL)) { free(alternate.ss_sp); return NULL; }
    return alternate.ss_sp;
}
static void k_overflow_stack_remove(void *stack) __attribute__((unused));
static void k_overflow_stack_remove(void *stack) {
    if (!stack) return;
    stack_t disabled;
    memset(&disabled, 0, sizeof disabled);
    disabled.ss_flags = SS_DISABLE;
    sigaltstack(&disabled, NULL);
    free(stack);
}
/* The main thread's signal stack lives until exit; this pointer keeps it reachable. */
static void *k_overflow_main_stack __attribute__((unused));
static void k_overflow_guard(void) __attribute__((unused));
static void k_overflow_guard(void) {
    if (K_SANITIZED) return;
    k_overflow_main_stack = k_overflow_stack_install();
    if (!k_overflow_main_stack) return;
    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_handler = k_overflow_handler;
    action.sa_flags = SA_ONSTACK;
    sigemptyset(&action.sa_mask);
    sigaction(SIGSEGV, &action, NULL);
    sigaction(SIGBUS, &action, NULL);
}
#endif
/* Set by a session entry that stopped early on a propagated Error; the host reads it. */
static int k_session_abort __attribute__((unused));
/* A notebook host arms these around one entry, so a runtime stop ends the cell and
   returns to the host instead of ending the process. Scripts leave them zero. */
static jmp_buf k_session_jump;
static int k_session_trap_armed __attribute__((unused));
static pthread_t k_session_trap_thread __attribute__((unused));
static KLocation k_session_trap_location __attribute__((unused));
static const char *k_session_trap_message __attribute__((unused));
typedef struct { uint64_t arena; size_t slot; } KArenaRef;
static _Atomic(uint64_t) k_arena_identity = 0;
static uint64_t k_arena_new_identity(void) {
    uint64_t previous = atomic_load_explicit(&k_arena_identity, memory_order_relaxed);
    for (;;) {
        if (previous == UINT64_MAX) k_fatal("Arena identities exhausted");
        if (atomic_compare_exchange_weak_explicit(&k_arena_identity, &previous, previous + 1,
                memory_order_relaxed, memory_order_relaxed)) return previous + 1;
    }
}
static bool k_arenaref_equal(KArenaRef a, KArenaRef b) {
    return a.arena == b.arena && a.slot == b.slot;
}
_Noreturn static void k_trap(KLocation loc, const char *message) {
    /* Only the thread that armed the entry may jump; a task thread still ends the process. */
    if (k_session_trap_armed && pthread_equal(pthread_self(), k_session_trap_thread)) {
        k_session_trap_armed = 0;
        k_session_trap_location = loc;
        k_session_trap_message = message;
        longjmp(k_session_jump, 1);
    }
    fprintf(stderr, "%s:%zu:%zu: runtime error: %s\n  %s\n",
            loc.file, loc.line, loc.column, message, loc.expression);
    exit(1);
}
static pthread_once_t k_locale_once = PTHREAD_ONCE_INIT;
static locale_t k_numeric_locale;
static void k_initialize_locale(void) {
    k_numeric_locale = newlocale(LC_NUMERIC_MASK, "C", (locale_t)0);
    if (!k_numeric_locale) k_fatal("Cannot create the numeric locale");
}
static locale_t k_number_locale(void) {
    if (pthread_once(&k_locale_once, k_initialize_locale)) k_fatal("Cannot initialize the numeric locale");
    return k_numeric_locale;
}

static int64_t k_add(int64_t a, int64_t b, KLocation loc) {
    int64_t r;
    if (__builtin_add_overflow(a, b, &r)) k_trap(loc, "Int addition overflow");
    return r;
}
static int64_t k_sub(int64_t a, int64_t b, KLocation loc) {
    int64_t r;
    if (__builtin_sub_overflow(a, b, &r)) k_trap(loc, "Int subtraction overflow");
    return r;
}
static int64_t k_mul(int64_t a, int64_t b, KLocation loc) {
    int64_t r;
    if (__builtin_mul_overflow(a, b, &r)) k_trap(loc, "Int multiplication overflow");
    return r;
}
/* Q11: exponentiation by squaring; a square that overflows means the result would too. */
static int64_t k_pow(int64_t base, int64_t exponent, KLocation loc) {
    if (exponent < 0) k_trap(loc, "Int power with a negative exponent");
    int64_t result = 1;
    while (exponent) {
        if ((exponent & 1) && __builtin_mul_overflow(result, base, &result)) k_trap(loc, "Int power overflow");
        exponent >>= 1;
        if (exponent && __builtin_mul_overflow(base, base, &base)) k_trap(loc, "Int power overflow");
    }
    return result;
}
/* Q52: a count outside 0 to 63 stops; << keeps the low 64 bits; >> keeps the sign. */
static int64_t k_shl(int64_t a, int64_t count, KLocation loc) {
    if (count < 0 || count > 63) k_trap(loc, "Shift count outside 0 to 63");
    return (int64_t)((uint64_t)a << count);
}
static int64_t k_shr(int64_t a, int64_t count, KLocation loc) {
    if (count < 0 || count > 63) k_trap(loc, "Shift count outside 0 to 63");
    return a < 0 ? ~(~a >> count) : a >> count;
}
static int64_t k_array_index(int64_t index, int64_t length, KLocation loc) {
    if (index < 0 || index >= length) k_trap(loc, "Array index out of range");
    return index;
}
static int64_t k_div(int64_t a, int64_t b, KLocation loc) {
    if (b == 0) k_trap(loc, "Int division by zero");
    if (a == INT64_MIN && b == -1) k_trap(loc, "Int division overflow");
    return a / b;
}
static int64_t k_rem(int64_t a, int64_t b, KLocation loc) {
    if (b == 0) k_trap(loc, "Int remainder by zero");
    /* The mathematical remainder is zero even when the quotient cannot fit. */
    if (a == INT64_MIN && b == -1) return 0;
    return a % b;
}
static uint64_t k_nat_succ(uint64_t value, KLocation loc) {
    if (value == UINT64_MAX) k_trap(loc, "Nat overflow");
    return value + 1;
}
static uint64_t k_nat_add(uint64_t a, uint64_t b, KLocation loc) {
    uint64_t result;
    if (__builtin_add_overflow(a, b, &result)) k_trap(loc, "Nat addition overflow");
    return result;
}
static uint64_t k_nat_mul(uint64_t a, uint64_t b, KLocation loc) {
    uint64_t result;
    if (__builtin_mul_overflow(a, b, &result)) k_trap(loc, "Nat multiplication overflow");
    return result;
}
static uint64_t k_nat_sub(uint64_t a, uint64_t b, KLocation loc) {
    if (b > a) k_trap(loc, "Nat subtraction below zero");
    return a - b;
}
static uint64_t k_nat_div(uint64_t a, uint64_t b, KLocation loc) {
    if (b == 0) k_trap(loc, "Nat division by zero");
    return a / b;
}
static uint64_t k_nat_rem(uint64_t a, uint64_t b, KLocation loc) {
    if (b == 0) k_trap(loc, "Nat remainder by zero");
    return a % b;
}
/* Integer helpers for Int methods (REVIEW2 section 4). */
static __attribute__((unused)) int64_t k_int_gcd(int64_t a, int64_t b, KLocation loc) {
    if (a == INT64_MIN || b == INT64_MIN) k_trap(loc, "Int gcd overflow");
    if (a < 0) a = -a;
    if (b < 0) b = -b;
    while (b) { int64_t t = a % b; a = b; b = t; }
    return a;
}
static __attribute__((unused)) int64_t k_int_lcm(int64_t a, int64_t b, KLocation loc) {
    if (a == 0 || b == 0) return 0;
    int64_t g = k_int_gcd(a, b, loc), q = (a < 0 ? -a : a) / g, r;
    if (__builtin_mul_overflow(q, b < 0 ? -b : b, &r)) k_trap(loc, "Int lcm overflow");
    return r;
}
static __attribute__((unused)) int64_t k_int_mod(int64_t a, int64_t b, KLocation loc) {
    if (b == 0) k_trap(loc, "Int modulo by zero");
    if (b == -1) return 0;
    int64_t r = a % b;
    return (r != 0 && ((r < 0) != (b < 0))) ? r + b : r;
}
static __attribute__((unused)) int64_t k_int_isqrt(int64_t a, KLocation loc) {
    if (a < 0) k_trap(loc, "isqrt() of a negative Int");
    int64_t r = (int64_t)sqrt((double)a);
    while (r > 0 && r > a / r) --r;
    while ((r + 1) <= a / (r + 1)) ++r;
    return r;
}
static __attribute__((unused)) bool k_int_is_prime(int64_t n) {
    if (n < 2) return false;
    if (n < 4) return true;
    if (n % 2 == 0 || n % 3 == 0) return false;
    for (int64_t i = 5; i <= n / i; i += 6) if (n % i == 0 || n % (i + 2) == 0) return false;
    return true;
}
static __attribute__((unused)) int64_t k_int_clamp(int64_t x, int64_t low, int64_t high, KLocation loc) {
    if (low > high) k_trap(loc, "clamp() needs low <= high");
    return x < low ? low : x > high ? high : x;
}
static __attribute__((unused)) double k_double_clamp(double x, double low, double high, KLocation loc) {
    if (!(low <= high)) k_trap(loc, "clamp() needs low <= high");
    return x < low ? low : x > high ? high : x;
}
static int64_t k_neg(int64_t a, KLocation loc) {
    if (a == INT64_MIN) k_trap(loc, "Int negation overflow");
    return -a;
}
static int64_t k_abs_int(int64_t a, KLocation loc) {
    if (a == INT64_MIN) k_trap(loc, "Int abs overflow");
    return a < 0 ? -a : a;
}

typedef struct { atomic_size_t references; size_t length, capacity; char bytes[]; } KStringOwner;
typedef struct { const char *data; size_t length; KStringOwner *owner; } KString;
static const KString k_empty = { "", 0, NULL };
static KString k_string_new(size_t length) {
    if (length > SIZE_MAX - sizeof(KStringOwner) - 1) k_fatal("String is too large");
    KStringOwner *owner = malloc(sizeof(KStringOwner) + length + 1);
    if (!owner) k_fatal("Out of memory");
    atomic_init(&owner->references, 1);
    owner->length = length;
    owner->capacity = length;
    owner->bytes[length] = '\0';
    return (KString) { owner->bytes, length, owner };
}
static KString k_string_copy(KString s) {
    if (s.owner && atomic_fetch_add_explicit(&s.owner->references, 1, memory_order_relaxed) == SIZE_MAX)
        k_fatal("String reference count overflow");
    return s;
}
static void k_string_drop(KString *s) {
    if (s->owner && atomic_fetch_sub_explicit(&s->owner->references, 1, memory_order_acq_rel) == 1)
        free(s->owner);
    *s = k_empty;
}
static KString k_string_take(KString *s) {
    KString result = *s;
    *s = k_empty;
    return result;
}
static void k_string_set(KString *target, KString owned) {
    k_string_drop(target);
    *target = owned;
}
static KString k_string_join(size_t count, const KString *parts) {
    if (count == 0) return k_empty;
    if (count == 1) return k_string_copy(parts[0]);
    size_t length = 0;
    for (size_t i = 0; i < count; ++i) {
        if (parts[i].length > SIZE_MAX - length) k_fatal("String is too large");
        length += parts[i].length;
    }
    KString result = k_string_new(length);
    char *out = result.owner->bytes;
    for (size_t i = 0; i < count; ++i) {
        if (parts[i].length) memcpy(out, parts[i].data, parts[i].length);
        out += parts[i].length;
    }
    return result;
}
static KString k_string_concat(KString a, KString b) {
    return k_string_join(2, (KString[]) { a, b });
}
static bool k_string_equal(KString a, KString b) {
    return a.length == b.length && (a.length == 0 || memcmp(a.data, b.data, a.length) == 0);
}
static KString k_string_bytes(const char *data, size_t length) {
    KString result = k_string_new(length);
    if (length) memcpy(result.owner->bytes, data, length);
    return result;
}
/* A String's debug view: quoted, with quotes, backslashes and control characters escaped. */
static KString k_string_debug(KString value) __attribute__((unused));
static KString k_string_debug(KString value) {
    size_t capacity = value.length * 6 + 3, used = 0;
    char *buffer = malloc(capacity);
    if (!buffer) k_fatal("Out of memory");
    buffer[used++] = '"';
    for (size_t i = 0; i < value.length; ++i) {
        unsigned char c = (unsigned char)value.data[i];
        switch (c) {
        case '"': buffer[used++] = '\\'; buffer[used++] = '"'; break;
        case '\\': buffer[used++] = '\\'; buffer[used++] = '\\'; break;
        case '\n': buffer[used++] = '\\'; buffer[used++] = 'n'; break;
        case '\r': buffer[used++] = '\\'; buffer[used++] = 'r'; break;
        case '\t': buffer[used++] = '\\'; buffer[used++] = 't'; break;
        default:
            if (c < 0x20 || c == 0x7f) used += (size_t)snprintf(buffer + used, capacity - used, "\\u%04x", c);
            else buffer[used++] = (char)c;
        }
    }
    buffer[used++] = '"';
    KString result = k_string_bytes(buffer, used);
    free(buffer);
    return result;
}
/* Decimal digits written from the end of a buffer; unsigned arithmetic covers INT64_MIN. */
static KString k_text_int(int64_t value) {
    char buffer[24];
    char *end = buffer + sizeof buffer, *p = end;
    uint64_t magnitude = value < 0 ? 0 - (uint64_t)value : (uint64_t)value;
    do {
        *--p = (char)('0' + magnitude % 10);
        magnitude /= 10;
    } while (magnitude);
    if (value < 0) *--p = '-';
    return k_string_bytes(p, (size_t)(end - p));
}
static KString k_text_nat(uint64_t value) {
    char buffer[32];
    int length = snprintf(buffer, sizeof buffer, "%" PRIu64, value);
    if (length < 0 || (size_t)length >= sizeof buffer) k_fatal("Nat formatting failed");
    return k_string_bytes(buffer, (size_t)length);
}
static KString k_text_bool(bool value) {
    return value ? (KString) { "true", 4, NULL } : (KString) { "false", 5, NULL };
}
static KString k_text_double(double value) {
    /* C libraries disagree on a NaN's sign and spelling; print it as macOS does. */
    if (isnan(value)) return (KString) { "nan", 3, NULL };
    char buffer[64];
    locale_t previous = uselocale(k_number_locale());
    int length = snprintf(buffer, sizeof buffer, "%.17g", value);
    uselocale(previous);
    if (length < 0 || (size_t)length + 2 >= sizeof buffer) k_fatal("double formatting failed");
    if (isfinite(value) && !strchr(buffer, '.') && !strchr(buffer, 'e') && !strchr(buffer, 'E')) {
        buffer[length++] = '.';
        buffer[length++] = '0';
    }
    return k_string_bytes(buffer, (size_t)length);
}
static KString k_text_fixed(double value, uint32_t precision) {
    if (precision > INT_MAX) k_fatal("Format precision is too large");
    if (isnan(value)) return k_string_bytes("nan", 3);
    locale_t previous = uselocale(k_number_locale());
    int length = snprintf(NULL, 0, "%.*f", (int)precision, value);
    uselocale(previous);
    if (length < 0) k_fatal("double formatting failed");
    KString result = k_string_new((size_t)length);
    previous = uselocale(k_number_locale());
    int actual = snprintf(result.owner->bytes, (size_t)length + 1, "%.*f", (int)precision, value);
    uselocale(previous);
    if (actual != length) k_fatal("double formatting failed");
    return result;
}
static void k_print_string(KString value) {
    flockfile(stdout);
    if (value.length && fwrite(value.data, 1, value.length, stdout) != value.length)
        k_fatal("Cannot write standard output");
    if (fputc('\n', stdout) == EOF) k_fatal("Cannot write standard output");
    funlockfile(stdout);
}
/* A script's top-level `?` met an Error: report it and end the program unsuccessfully. */
_Noreturn static void k_script_fail(KString text) __attribute__((unused));
static void k_script_fail(KString text) {
    fflush(stdout);
    fputs("error: ", stderr);
    if (text.length) fwrite(text.data, 1, text.length, stderr);
    fputc('\n', stderr);
    exit(1);
}
static void k_print_nat(uint64_t value) {
    flockfile(stdout);
    if (printf("%" PRIu64 "\n", value) < 0) k_fatal("Cannot write standard output");
    funlockfile(stdout);
}
static void k_print_int(int64_t value) {
    flockfile(stdout);
    if (printf("%" PRId64 "\n", value) < 0) k_fatal("Cannot write standard output");
    funlockfile(stdout);
}
static void k_print_bool(bool value) {
    k_print_string(k_text_bool(value));
}
static void k_print_double(double value) {
    KString text = k_text_double(value);
    k_print_string(text);
    k_string_drop(&text);
}
_Noreturn static void k_assert_failed(KString a, KString b, KLocation loc) {
    fprintf(stderr, "%s:%zu:%zu: runtime error: assertEq failed\n  %s\n  left: ",
            loc.file, loc.line, loc.column, loc.expression);
    if (a.length) fwrite(a.data, 1, a.length, stderr);
    fputs("\n  right: ", stderr);
    if (b.length) fwrite(b.data, 1, b.length, stderr);
    fputc('\n', stderr);
    exit(1);
}
static void k_assert_int(int64_t a, int64_t b, KLocation loc) {
    if (a != b) k_assert_failed(k_text_int(a), k_text_int(b), loc);
}
static void k_assert_double(double a, double b, KLocation loc) {
    if (a != b) k_assert_failed(k_text_double(a), k_text_double(b), loc);
}
static void k_assert_bool(bool a, bool b, KLocation loc) {
    if (a != b) k_assert_failed(k_text_bool(a), k_text_bool(b), loc);
}
static void k_assert_string(KString a, KString b, KLocation loc) {
    if (!k_string_equal(a, b)) k_assert_failed(a, b, loc);
}


typedef struct KDropStack KDropStack;
typedef struct { void *value; void (*run)(void *, KDropStack *); } KDropTask;
struct KDropStack { KDropTask *tasks; size_t count, capacity; KDropTask small[32]; };
static void k_drop_push(KDropStack *stack, void *value, void (*run)(void *, KDropStack *)) {
    if (stack->count == stack->capacity) {
        if (stack->capacity > SIZE_MAX / 2 / sizeof(KDropTask)) k_fatal("Destruction work stack is too large");
        size_t capacity = stack->capacity * 2;
        KDropTask *tasks = malloc(capacity * sizeof *tasks);
        if (!tasks) k_fatal("Out of memory");
        memcpy(tasks, stack->tasks, stack->count * sizeof *tasks);
        if (stack->tasks != stack->small) free(stack->tasks);
        stack->tasks = tasks;
        stack->capacity = capacity;
    }
    stack->tasks[stack->count++] = (KDropTask) { value, run };
}
static void k_drop_run(void *value, void (*run)(void *, KDropStack *)) {
    KDropStack stack;
    stack.tasks = stack.small;
    stack.count = 0;
    stack.capacity = sizeof stack.small / sizeof stack.small[0];
    run(value, &stack);
    while (stack.count) {
        KDropTask task = stack.tasks[--stack.count];
        task.run(task.value, &stack);
    }
    if (stack.tasks != stack.small) free(stack.tasks);
}
static void k_drop_free(void *value, KDropStack *stack) { (void)stack; free(value); }
/* Blocks aligned past max_align_t. The UCRT has no aligned_alloc, and its aligned blocks
   need their own free. */
static void *k_aligned_alloc(size_t alignment, size_t size) {
#ifdef _WIN32
    return _aligned_malloc(size, alignment);
#else
    return aligned_alloc(alignment, size);
#endif
}
static void k_drop_aligned_free(void *value, KDropStack *stack) {
    (void)stack;
#ifdef _WIN32
    _aligned_free(value);
#else
    free(value);
#endif
}

/* Collection cells and map entries come from per-thread free lists in 16-byte classes.
   A global list keeps every chunk reachable. Sanitizer and leak-check builds use malloc,
   so those tools still see each block. */
#if defined(__has_feature)
#if __has_feature(address_sanitizer)
#define K_NO_POOL 1
#endif
#endif
#if defined(__SANITIZE_ADDRESS__)
#define K_NO_POOL 1
#endif
#ifdef K_NO_POOL
static void *k_small_alloc(size_t size) { void *block = malloc(size); if (!block) k_fatal("Out of memory"); return block; }
static void k_small_free(void *block, size_t size) { (void)size; free(block); }
#else
typedef struct KSmallBlock { struct KSmallBlock *next; } KSmallBlock;
typedef struct KSmallChunk { struct KSmallChunk *next; max_align_t align; } KSmallChunk;
static _Thread_local KSmallBlock *k_small_lists[33];
static _Thread_local char *k_small_cursor, *k_small_end;
static KSmallChunk *k_small_chunks;
static pthread_mutex_t k_small_lock = PTHREAD_MUTEX_INITIALIZER;
static void *k_small_alloc(size_t size) {
    if (size > 256) { void *block = malloc(size); if (!block) k_fatal("Out of memory"); return block; }
    size_t class = (size + 7) >> 3;
    if (class == 0) class = 1;
    KSmallBlock *block = k_small_lists[class];
    if (block) { k_small_lists[class] = block->next; return block; }
    size_t bytes = class << 3;
    if ((size_t)(k_small_end - k_small_cursor) < bytes) {
        size_t size_of_chunk = (size_t)1 << 16;
        KSmallChunk *chunk = malloc(size_of_chunk);
        if (!chunk) k_fatal("Out of memory");
        pthread_mutex_lock(&k_small_lock);
        chunk->next = k_small_chunks;
        k_small_chunks = chunk;
        pthread_mutex_unlock(&k_small_lock);
        k_small_cursor = (char *)(chunk + 1);
        k_small_end = (char *)chunk + size_of_chunk;
    }
    void *fresh = k_small_cursor;
    k_small_cursor += bytes;
    return fresh;
}
static void k_small_free(void *block, size_t size) {
    if (size > 256) { free(block); return; }
    KSmallBlock *freed = block;
    size_t class = (size + 7) >> 3;
    if (class == 0) class = 1;
    freed->next = k_small_lists[class];
    k_small_lists[class] = freed;
}
#endif
static void k_string_drop_task(void *value, KDropStack *stack) { (void)stack; k_string_drop((KString *)value); }

/* Erased iteration keeps an owned value here when a result List must retain it. */
typedef struct KOwner {
    atomic_size_t references;
    void (*destroy)(void *, KDropStack *);
    max_align_t align;
    unsigned char bytes[];
} KOwner;
static KOwner *k_owner_allocate(size_t size, void (*destroy)(void *, KDropStack *)) {
    if (size > SIZE_MAX - sizeof(KOwner)) k_fatal("Iteration item is too large");
    KOwner *owner = malloc(sizeof(KOwner) + size);
    if (!owner) k_fatal("Out of memory");
    atomic_init(&owner->references, 1);
    owner->destroy = destroy;
    return owner;
}
static KOwner *k_owner_copy(KOwner *owner) {
    if (owner && atomic_fetch_add_explicit(&owner->references, 1, memory_order_relaxed) == SIZE_MAX)
        k_fatal("Iteration owner reference count overflow");
    return owner;
}
static void k_owner_release(void *payload, KDropStack *stack) {
    KOwner *owner = payload;
    if (owner && atomic_fetch_sub_explicit(&owner->references, 1, memory_order_acq_rel) == 1) {
        k_drop_push(stack, owner, k_drop_free);
        owner->destroy(owner->bytes, stack);
    }
}
static KOwner *k_owner_require(KOwner *owner) {
    if (!owner) k_fatal("Cannot collect a borrowed item; return an owned Option instead");
    return owner;
}
static void k_owner_drop_task(void *payload, KDropStack *stack) {
    KOwner **slot = payload, *owner = *slot;
    *slot = NULL;
    k_owner_release(owner, stack);
}
static void k_owner_drop(KOwner **slot) { if (*slot) k_drop_run(slot, k_owner_drop_task); }
static KOwner *k_owner_take(KOwner **slot) { KOwner *owner = *slot; *slot = NULL; return owner; }
static void k_owner_set(KOwner **slot, KOwner *owned) { k_owner_drop(slot); *slot = owned; }

typedef struct KErrorOwner KErrorOwner;
typedef struct { KErrorOwner *owner; uint64_t type; void *payload; } KError;
struct KErrorOwner {
    atomic_size_t references;
    void (*destroy)(void *, KDropStack *);
    KError cause;
    KString context;
    max_align_t alignment;
    unsigned char bytes[];
};
_Static_assert(offsetof(KErrorOwner, bytes) % _Alignof(max_align_t) == 0, "Error payload alignment");
static KError k_error_allocate(uint64_t type, size_t size, void (*destroy)(void *, KDropStack *)) {
    if (size > SIZE_MAX - sizeof(KErrorOwner)) k_fatal("Error payload is too large");
    KErrorOwner *owner = malloc(sizeof(KErrorOwner) + size);
    if (!owner) k_fatal("Out of memory");
    atomic_init(&owner->references, 1);
    owner->destroy = destroy;
    owner->cause = (KError){0};
    owner->context = k_empty;
    return (KError) { owner, type, owner->bytes };
}
static KError k_error_copy(KError error) {
    if (error.owner && atomic_fetch_add_explicit(&error.owner->references, 1, memory_order_relaxed) == SIZE_MAX)
        k_fatal("Error reference count overflow");
    return error;
}
static void k_error_drop_task(void *value, KDropStack *stack) {
    KError error = *(KError *)value;
    *(KError *)value = (KError) {0};
    if (error.owner && atomic_fetch_sub_explicit(&error.owner->references, 1, memory_order_acq_rel) == 1) {
        k_drop_push(stack, error.owner, k_drop_free);
        k_string_drop(&error.owner->context);
        k_drop_push(stack, &error.owner->cause, k_error_drop_task);
        if (error.owner->destroy) error.owner->destroy(error.payload, stack);
    }
}
static void k_error_drop(KError *error) { k_drop_run(error, k_error_drop_task); }
static __attribute__((unused)) KError k_error_context(KError original, KString message) {
    KError result = k_error_allocate(original.type, 0, NULL);
    result.payload = original.payload;
    result.owner->cause = k_error_copy(original);
    result.owner->context = k_string_copy(message);
    return result;
}
static KError k_error_take(KError *error) {
    KError result = *error;
    *error = (KError) {0};
    return result;
}
static void k_error_set(KError *target, KError owned) {
    k_error_drop(target);
    *target = owned;
}

static bool k_parse_int_value(KString text, int64_t *result) {
    size_t i = 0;
    bool negative = false;
    if (i < text.length && (text.data[i] == '-' || text.data[i] == '+'))
        negative = text.data[i++] == '-';
    if (i == text.length) return false;
    uint64_t magnitude = 0, limit = negative ? UINT64_C(9223372036854775808) : INT64_MAX;
    for (; i < text.length; ++i) {
        unsigned char ch = (unsigned char)text.data[i];
        if (ch < '0' || ch > '9') return false;
        unsigned digit = ch - '0';
        if (magnitude > (limit - digit) / 10) return false;
        magnitude = magnitude * 10 + digit;
    }
    *result = negative ? (magnitude == UINT64_C(9223372036854775808) ? INT64_MIN : -(int64_t)magnitude) : (int64_t)magnitude;
    return true;
}
static bool k_parse_double_value(KString text, double *result) {
    size_t i = 0, digits = 0;
    bool nonzero = false;
    if (i < text.length && (text.data[i] == '-' || text.data[i] == '+')) ++i;
    while (i < text.length && text.data[i] >= '0' && text.data[i] <= '9') {
        nonzero |= text.data[i] != '0';
        ++i; ++digits;
    }
    if (i < text.length && text.data[i] == '.') {
        ++i;
        while (i < text.length && text.data[i] >= '0' && text.data[i] <= '9') {
            nonzero |= text.data[i] != '0';
            ++i; ++digits;
        }
    }
    if (digits == 0) return false;
    if (i < text.length && (text.data[i] == 'e' || text.data[i] == 'E')) {
        ++i;
        if (i < text.length && (text.data[i] == '-' || text.data[i] == '+')) ++i;
        size_t begin = i;
        while (i < text.length && text.data[i] >= '0' && text.data[i] <= '9') ++i;
        if (i == begin) return false;
    }
    if (i != text.length) return false;
    /* KString storage always has a trailing NUL; the grammar excludes embedded NUL. */
    char *end = NULL;
    locale_t previous = uselocale(k_number_locale());
    *result = strtod(text.data, &end);
    uselocale(previous);
    return end == text.data + text.length && isfinite(*result) && (*result != 0.0 || !nonzero);
}

#define K_TEXT(text) ((KString){ (text), sizeof(text)-1, NULL })
#ifdef K_SESSION_RUNTIME
/* Published versions are immutable and stay reachable until process exit. */
typedef struct KSessionVersion {
    struct KSessionVersion *previous;
    void **globals;
    void ***dispatch;
    KError (*error)(uint32_t,KString);
} KSessionVersion;
static _Atomic(KSessionVersion *) k_session_version;
static KError k_runtime_error(uint32_t tag, KString message) {
    KSessionVersion *version=atomic_load_explicit(&k_session_version,memory_order_acquire);
    if (!version) k_fatal("The session has no runtime error layout");
    return version->error(tag,message);
}
#else
static KError k_runtime_error(uint32_t tag, KString message);
#endif
static KError k_io_error(const char *context) {
    int saved = errno;
    KString prefix = { context, strlen(context), NULL };
    char buffer[256];
#if defined(__GLIBC__) && defined(_GNU_SOURCE)
    const char *description = strerror_r(saved,buffer,sizeof buffer);
#else
    const char *description = buffer;
    if (strerror_r(saved,buffer,sizeof buffer)) snprintf(buffer,sizeof buffer,"System error %d",saved);
#endif
    KString detail = {description, strlen(description),NULL};
    KString pieces[] = { prefix, K_TEXT(": "), detail };
    return k_runtime_error(3,k_string_join(3,pieces));
}
/* A String passed to a `const char *` parameter; a NUL inside it would cut the text (Q51). */
static char *k_c_text(KString text) {
    if (text.length && memchr(text.data, 0, text.length)) k_fatal("A String passed to C contains a NUL character");
    char *copy = malloc(text.length + 1);
    if (!copy) k_fatal("Out of memory");
    if (text.length) memcpy(copy, text.data, text.length);
    copy[text.length] = 0;
    return copy;
}
static void k_pthread_check(int status) { if (status) k_fatal("Synchronization operation failed"); }

#ifdef K_SESSION_RUNTIME
/* Session entries are separate shared libraries, so a static var needs one
   value cell per key in the shared runtime. The first library with a key
   initializes it; later versions attach to the same cell. */
typedef struct KSessionStatic {
    char *key;
    void *value;
    pthread_mutex_t mutex;
    struct KSessionStatic *next;
} KSessionStatic;
static KSessionStatic *k_session_statics;
static pthread_mutex_t k_session_statics_lock = PTHREAD_MUTEX_INITIALIZER;
static KSessionStatic *k_session_static(const char *key, size_t size, void (*init)(void *)) {
    k_pthread_check(pthread_mutex_lock(&k_session_statics_lock));
    for (KSessionStatic *cell = k_session_statics; cell; cell = cell->next) {
        if (strcmp(cell->key, key) == 0) {
            k_pthread_check(pthread_mutex_unlock(&k_session_statics_lock));
            return cell;
        }
    }
    KSessionStatic *cell = calloc(1, sizeof *cell);
    if (!cell) k_fatal("Out of memory");
    size_t length = strlen(key);
    cell->key = malloc(length + 1);
    cell->value = calloc(1, size ? size : 1);
    if (!cell->key || !cell->value) k_fatal("Out of memory");
    memcpy(cell->key, key, length + 1);
    if (pthread_mutex_init(&cell->mutex, NULL)) k_fatal("Cannot initialize static storage");
    cell->next = k_session_statics;
    k_session_statics = cell;
    init(cell->value);
    k_pthread_check(pthread_mutex_unlock(&k_session_statics_lock));
    return cell;
}
#endif
static uint64_t k_double_bits(double value) { uint64_t bits; memcpy(&bits,&value,sizeof bits); return bits; }
static double k_bits_double(uint64_t bits) { double value; memcpy(&value,&bits,sizeof value); return value; }
static bool k_valid_utf8(KString text) {
    size_t i = 0;
    while (i < text.length) {
        unsigned char first = (unsigned char)text.data[i++];
        if (first < 0x80) continue;
        unsigned count; uint32_t code, minimum;
        if (first >= 0xc2 && first <= 0xdf) { count = 1; code = first & 31; minimum = 0x80; }
        else if (first >= 0xe0 && first <= 0xef) { count = 2; code = first & 15; minimum = 0x800; }
        else if (first >= 0xf0 && first <= 0xf4) { count = 3; code = first & 7; minimum = 0x10000; }
        else return false;
        if (count > text.length-i) return false;
        while (count--) { unsigned char next = (unsigned char)text.data[i++]; if ((next & 0xc0) != 0x80) return false; code = (code << 6) | (next & 63); }
        if (code < minimum || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) return false;
    }
    return true;
}

typedef struct { KString text; KError error; } KReadResult;
static KReadResult k_file_read_line(FILE *file) {
    char *line = NULL;
    size_t capacity = 0;
    errno = 0;
    ssize_t length = getline(&line,&capacity,file);
    if (length < 0) { free(line); return (KReadResult) {k_empty,ferror(file) ? k_io_error("Cannot read line") : k_runtime_error(5,K_TEXT("End of file"))}; }
    size_t n = (size_t)length;
    if (n && line[n-1] == '\n') { --n; if (n && line[n-1] == '\r') --n; }
    KString text = k_string_bytes(line,n);
    free(line);
    if (!k_valid_utf8(text)) { k_string_drop(&text); return (KReadResult) {k_empty,k_runtime_error(3,K_TEXT("File text is not valid UTF-8"))}; }
    return (KReadResult) {text,{0}};
}

typedef struct KTask {
    pthread_t thread;
    struct KTask *next;
    KError (*run)(void *);
    void *environment;
    KError error;
} KTask;
typedef struct { KTask *head, *tail; } KTaskScope;
static void *k_task_entry(void *argument) {
    KTask *task = argument;
    void *overflow_stack = k_overflow_stack_install();
    task->error = task->run(task->environment);
    k_overflow_stack_remove(overflow_stack);
    return NULL;
}
static void k_tasks_start(KTaskScope *scope, KError (*run)(void *), void *environment) {
    KTask *task = calloc(1,sizeof *task);
    if (!task) k_fatal("Out of memory");
    task->run = run;
    task->environment = environment;
    if (pthread_create(&task->thread,NULL,k_task_entry,task)) k_fatal("Cannot start task");
    if (scope->tail) scope->tail->next = task; else scope->head = task;
    scope->tail = task;
}
static KError k_tasks_join(KTaskScope *scope) {
    KError error = {0};
    KTask *task = scope->head;
    scope->head = scope->tail = NULL;
    while (task) {
        KTask *next = task->next;
        k_pthread_check(pthread_join(task->thread,NULL));
        if (!error.type) error = k_error_take(&task->error);
        k_error_drop(&task->error);
        free(task);
        task = next;
    }
    return error;
}
static void k_tasks_drop(KTaskScope *scope) {
    KError error = k_tasks_join(scope);
    if (error.type) k_fatal("An unhandled task error reached scope cleanup");
}

static int64_t k_compare_double(double a, double b) {
    if (isnan(a)) return isnan(b) ? 0 : 1;
    if (isnan(b)) return -1;
    uint64_t x = k_double_bits(a), y = k_double_bits(b);
    x = x >> 63 ? ~x : x | (UINT64_C(1)<<63);
    y = y >> 63 ? ~y : y | (UINT64_C(1)<<63);
    return (x>y) - (x<y);
}
static int64_t k_compare_string(KString a, KString b) {
    size_t count = a.length < b.length ? a.length : b.length;
    int compared = memcmp(a.data,b.data,count);
    return compared ? (compared>0) - (compared<0) : (a.length>b.length) - (a.length<b.length);
}

/* Numeric maps: a bucket stores slot + 1 below and the hash's top half above.
   The slot must fit 32 bits, which caps a numeric Map at about 1.6 billion entries. */
#define K_MAP_ROOM(capacity) ((capacity) - (capacity) / 4)
#define K_MAP_SLOT(bucket) ((size_t)((bucket) & UINT64_C(0xffffffff)))
#define K_MAP_TAG(hash) ((hash) & ~UINT64_C(0xffffffff))
#define K_MAP_MAX_CAPACITY ((size_t)1 << 31)
static uint64_t k_hash_mix(uint64_t value) {
    value ^= value >> 30; value *= UINT64_C(0xbf58476d1ce4e5b9);
    value ^= value >> 27; value *= UINT64_C(0x94d049bb133111eb);
    return value ^ (value >> 31);
}
static uint64_t k_hash_string(KString value) {
    uint64_t hash=UINT64_C(14695981039346656037);
    for(size_t i=0;i<value.length;++i) {hash^=(unsigned char)value.data[i];hash*=UINT64_C(1099511628211);}
    return k_hash_mix(hash);
}

/* A closure environment starts with this counted header; release frees what it captured. */
typedef struct KClosure {
    atomic_size_t references;
    void (*release)(struct KClosure *, KDropStack *);
} KClosure;

/* `s = s + piece` writes in place when this variable holds the only reference.
   Storage grows by half again, so building a string from n pieces costs O(n). */
static void k_string_append_bytes(KString *target, const char *data, size_t count) {
    if (!count) return;
    if (count > SIZE_MAX / 2 - target->length) k_fatal("String is too large");
    size_t length = target->length + count;
    KStringOwner *owner = target->owner;
    bool unique = owner && target->data == owner->bytes && target->length == owner->length
        && atomic_load_explicit(&owner->references, memory_order_acquire) == 1;
    if (!unique || length > owner->capacity) {
        size_t capacity = length < 32 ? 32 : length + length / 2;
        if (unique) {
            owner = realloc(owner, sizeof(KStringOwner) + capacity + 1);
            if (!owner) k_fatal("Out of memory");
        } else {
            KStringOwner *grown = malloc(sizeof(KStringOwner) + capacity + 1);
            if (!grown) k_fatal("Out of memory");
            atomic_init(&grown->references, 1);
            if (target->length) memcpy(grown->bytes, target->data, target->length);
            k_string_drop(target);
            owner = grown;
        }
        owner->capacity = capacity;
    }
    memcpy(owner->bytes + (length - count), data, count);
    owner->length = length;
    owner->bytes[length] = '\0';
    *target = (KString) { owner->bytes, length, owner };
}
static void k_string_append(KString *target, KString piece) {
    k_string_append_bytes(target, piece.data, piece.length);
}
/* Direct appends skip the temporary String a text() conversion would allocate. */
static void k_string_append_int(KString *target, int64_t value) {
    char buffer[24];
    char *end = buffer + sizeof buffer, *p = end;
    uint64_t magnitude = value < 0 ? 0 - (uint64_t)value : (uint64_t)value;
    do { *--p = (char)('0' + magnitude % 10); magnitude /= 10; } while (magnitude);
    if (value < 0) *--p = '-';
    k_string_append_bytes(target, p, (size_t)(end - p));
}
static void k_string_append_nat(KString *target, uint64_t value) {
    char buffer[32];
    int length = snprintf(buffer, sizeof buffer, "%" PRIu64, value);
    if (length < 0 || (size_t)length >= sizeof buffer) k_fatal("Nat formatting failed");
    k_string_append_bytes(target, buffer, (size_t)length);
}
static void k_string_append_bool(KString *target, bool value) {
    if (value) k_string_append_bytes(target, "true", 4);
    else k_string_append_bytes(target, "false", 5);
}
static void k_string_append_double(KString *target, double value) {
    if (isnan(value)) { k_string_append_bytes(target, "nan", 3); return; }
    char buffer[64];
    locale_t previous = uselocale(k_number_locale());
    int length = snprintf(buffer, sizeof buffer, "%.17g", value);
    uselocale(previous);
    if (length < 0 || (size_t)length + 2 >= sizeof buffer) k_fatal("double formatting failed");
    if (isfinite(value) && !strchr(buffer, '.') && !strchr(buffer, 'e') && !strchr(buffer, 'E')) {
        buffer[length++] = '.';
        buffer[length++] = '0';
    }
    k_string_append_bytes(target, buffer, (size_t)length);
}

/* A short pause inside a spin-wait, so a waiting thread wastes less of its core.
   On Apple cores `yield` does almost nothing; `isb` waits for the pipeline to drain. */
static inline void k_cpu_relax(void) {
#if defined(__aarch64__)
    __asm__ volatile("isb");
#elif defined(__x86_64__) || defined(__i386__)
    __asm__ volatile("pause");
#endif
}
/* A lock held for a few instructions: one channel end's queue position. */
static void k_spin_lock(atomic_bool *lock) {
    for (unsigned n = 0; atomic_exchange_explicit(lock, true, memory_order_acquire);) {
        while (atomic_load_explicit(lock, memory_order_relaxed)) {
            if (n < 128) { ++n; k_cpu_relax(); } else sched_yield();
        }
    }
}
static void k_spin_unlock(atomic_bool *lock) { atomic_store_explicit(lock, false, memory_order_release); }
