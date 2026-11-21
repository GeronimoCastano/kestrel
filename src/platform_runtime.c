/* Windows support for the runtime. The runtime is written against POSIX; on Windows this
   file supplies the part of pthreads, dlfcn, dirent and unistd that it uses, on top of
   the Win32 API and the Universal CRT. Everything here is empty on other systems. */
#ifdef _WIN32
#ifndef _CRT_SECURE_NO_WARNINGS
#define _CRT_SECURE_NO_WARNINGS
#endif
#ifndef _CRT_NONSTDC_NO_DEPRECATE
#define _CRT_NONSTDC_NO_DEPRECATE
#endif
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#ifndef NOGDI
#define NOGDI
#endif
#include <windows.h>
#include <direct.h>
#include <errno.h>
#include <fcntl.h>
#include <io.h>
#include <process.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <time.h>

typedef ptrdiff_t ssize_t;

/* Threads. A pthread_t keeps the handle for join and the id for equality. */
typedef struct { HANDLE handle; DWORD id; } pthread_t;
typedef struct { void *(*run)(void *); void *argument; } KWinThreadStart;
static unsigned __stdcall k_win_thread_entry(void *start) {
    KWinThreadStart copy = *(KWinThreadStart *)start;
    free(start);
    copy.run(copy.argument);
    return 0;
}
static int pthread_create(pthread_t *thread, const void *attributes, void *(*run)(void *), void *argument) {
    (void)attributes;
    KWinThreadStart *start = malloc(sizeof *start);
    if (!start) return ENOMEM;
    start->run = run;
    start->argument = argument;
    unsigned id = 0;
    /* Reserve 8 MiB, as Linux and macOS give a new thread's stack room to grow. */
    uintptr_t handle = _beginthreadex(NULL, 8u << 20, k_win_thread_entry, start, STACK_SIZE_PARAM_IS_A_RESERVATION, &id);
    if (!handle) { free(start); return EAGAIN; }
    thread->handle = (HANDLE)handle;
    thread->id = id;
    return 0;
}
static int pthread_join(pthread_t thread, void **result) {
    if (result) *result = NULL;
    if (WaitForSingleObject(thread.handle, INFINITE) != WAIT_OBJECT_0) return EINVAL;
    CloseHandle(thread.handle);
    return 0;
}
static pthread_t pthread_self(void) {
    pthread_t self = { NULL, GetCurrentThreadId() };
    return self;
}
static int pthread_equal(pthread_t a, pthread_t b) { return a.id == b.id; }
static int sched_yield(void) { SwitchToThread(); return 0; }

/* Mutexes. An SRW lock plus the owning thread, so recursive and error-checking
   mutexes can tell a second lock from the same thread. */
enum { PTHREAD_MUTEX_NORMAL = 0, PTHREAD_MUTEX_RECURSIVE = 1, PTHREAD_MUTEX_ERRORCHECK = 2 };
typedef struct { int kind; } pthread_mutexattr_t;
typedef struct { SRWLOCK lock; volatile LONG owner; unsigned depth; int kind; } pthread_mutex_t;
#define PTHREAD_MUTEX_INITIALIZER { SRWLOCK_INIT, 0, 0, PTHREAD_MUTEX_NORMAL }
static int pthread_mutexattr_init(pthread_mutexattr_t *attributes) { attributes->kind = PTHREAD_MUTEX_NORMAL; return 0; }
static int pthread_mutexattr_settype(pthread_mutexattr_t *attributes, int kind) { attributes->kind = kind; return 0; }
static int pthread_mutexattr_destroy(pthread_mutexattr_t *attributes) { (void)attributes; return 0; }
static int pthread_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attributes) {
    InitializeSRWLock(&mutex->lock);
    mutex->owner = 0;
    mutex->depth = 0;
    mutex->kind = attributes ? attributes->kind : PTHREAD_MUTEX_NORMAL;
    return 0;
}
static int pthread_mutex_destroy(pthread_mutex_t *mutex) { return mutex->owner ? EBUSY : 0; }
static int pthread_mutex_lock(pthread_mutex_t *mutex) {
    LONG self = (LONG)GetCurrentThreadId();
    if (mutex->kind != PTHREAD_MUTEX_NORMAL && mutex->owner == self) {
        if (mutex->kind == PTHREAD_MUTEX_ERRORCHECK) return EDEADLK;
        ++mutex->depth;
        return 0;
    }
    AcquireSRWLockExclusive(&mutex->lock);
    InterlockedExchange(&mutex->owner, self);
    mutex->depth = 0;
    return 0;
}
static int pthread_mutex_trylock(pthread_mutex_t *mutex) {
    LONG self = (LONG)GetCurrentThreadId();
    if (mutex->owner == self) {
        if (mutex->kind != PTHREAD_MUTEX_RECURSIVE) return EBUSY;
        ++mutex->depth;
        return 0;
    }
    if (!TryAcquireSRWLockExclusive(&mutex->lock)) return EBUSY;
    InterlockedExchange(&mutex->owner, self);
    mutex->depth = 0;
    return 0;
}
static int pthread_mutex_unlock(pthread_mutex_t *mutex) {
    if (mutex->kind != PTHREAD_MUTEX_NORMAL && mutex->owner != (LONG)GetCurrentThreadId()) return EPERM;
    if (mutex->depth) { --mutex->depth; return 0; }
    InterlockedExchange(&mutex->owner, 0);
    ReleaseSRWLockExclusive(&mutex->lock);
    return 0;
}

/* Condition variables wait on the mutex's SRW lock. */
typedef struct { CONDITION_VARIABLE variable; } pthread_cond_t;
#define PTHREAD_COND_INITIALIZER { CONDITION_VARIABLE_INIT }
static int pthread_cond_init(pthread_cond_t *condition, const void *attributes) {
    (void)attributes;
    InitializeConditionVariable(&condition->variable);
    return 0;
}
static int pthread_cond_destroy(pthread_cond_t *condition) { (void)condition; return 0; }
static int pthread_cond_wait(pthread_cond_t *condition, pthread_mutex_t *mutex) {
    unsigned depth = mutex->depth;
    InterlockedExchange(&mutex->owner, 0);
    BOOL woke = SleepConditionVariableSRW(&condition->variable, &mutex->lock, INFINITE, 0);
    InterlockedExchange(&mutex->owner, (LONG)GetCurrentThreadId());
    mutex->depth = depth;
    return woke ? 0 : EINVAL;
}
static int pthread_cond_signal(pthread_cond_t *condition) { WakeConditionVariable(&condition->variable); return 0; }
static int pthread_cond_broadcast(pthread_cond_t *condition) { WakeAllConditionVariable(&condition->variable); return 0; }

/* One-time initialization. */
typedef INIT_ONCE pthread_once_t;
#define PTHREAD_ONCE_INIT INIT_ONCE_STATIC_INIT
static BOOL CALLBACK k_win_once_entry(PINIT_ONCE once, PVOID run, PVOID *context) {
    (void)once;
    (void)context;
    void (*function)(void);
    memcpy(&function, &run, sizeof function);
    function();
    return TRUE;
}
static int pthread_once(pthread_once_t *once, void (*run)(void)) {
    PVOID argument;
    memcpy(&argument, &run, sizeof argument);
    return InitOnceExecuteOnce(once, k_win_once_entry, argument, NULL) ? 0 : EINVAL;
}

/* Shared libraries. LoadLibrary searches the program's folder and PATH. */
#define RTLD_LAZY 1
#define RTLD_NOW 2
#define RTLD_LOCAL 0
static char k_win_library_error[512];
static void *dlopen(const char *name, int flags) {
    (void)flags;
    HMODULE module = LoadLibraryA(name);
    if (!module) {
        DWORD code = GetLastError();
        DWORD length = FormatMessageA(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS, NULL, code, 0,
            k_win_library_error, sizeof k_win_library_error, NULL);
        if (!length) snprintf(k_win_library_error, sizeof k_win_library_error, "Windows error %lu", (unsigned long)code);
    }
    return (void *)module;
}
static void *dlsym(void *library, const char *name) {
    FARPROC found = GetProcAddress((HMODULE)library, name);
    void *symbol;
    memcpy(&symbol, &found, sizeof symbol);
    return symbol;
}
static int dlclose(void *library) { return FreeLibrary((HMODULE)library) ? 0 : -1; }
static const char *dlerror(void) { return k_win_library_error; }

/* Directory listing. */
struct dirent { char d_name[MAX_PATH]; };
typedef struct { HANDLE find; WIN32_FIND_DATAA data; bool pending; struct dirent entry; } DIR;
static DIR *opendir(const char *name) {
    size_t length = strlen(name);
    char *pattern = malloc(length + 3);
    if (!pattern) { errno = ENOMEM; return NULL; }
    memcpy(pattern, name, length);
    size_t end = length;
    if (end && name[end - 1] != '/' && name[end - 1] != '\\') pattern[end++] = '\\';
    pattern[end++] = '*';
    pattern[end] = 0;
    DIR *directory = calloc(1, sizeof *directory);
    if (!directory) { free(pattern); errno = ENOMEM; return NULL; }
    directory->find = FindFirstFileA(pattern, &directory->data);
    free(pattern);
    if (directory->find == INVALID_HANDLE_VALUE) {
        DWORD code = GetLastError();
        free(directory);
        errno = code == ERROR_FILE_NOT_FOUND || code == ERROR_PATH_NOT_FOUND ? ENOENT
            : code == ERROR_DIRECTORY ? ENOTDIR : EACCES;
        return NULL;
    }
    directory->pending = true;
    return directory;
}
static struct dirent *readdir(DIR *directory) {
    if (!directory->pending && !FindNextFileA(directory->find, &directory->data)) return NULL;
    directory->pending = false;
    strncpy(directory->entry.d_name, directory->data.cFileName, sizeof directory->entry.d_name - 1);
    directory->entry.d_name[sizeof directory->entry.d_name - 1] = 0;
    return &directory->entry;
}
static int closedir(DIR *directory) {
    FindClose(directory->find);
    free(directory);
    return 0;
}

/* Files, processes and the C library names that differ. */
#define S_ISDIR(mode) (((mode) & _S_IFMT) == _S_IFDIR)
#define S_ISREG(mode) (((mode) & _S_IFMT) == _S_IFREG)
#define mkdir(name, mode) _mkdir(name)
#define getcwd(buffer, size) _getcwd((buffer), (int)(size))
#define getpid() _getpid()
#define fileno(file) _fileno(file)
#define fdopen(descriptor, mode) _fdopen((descriptor), (mode))
#define write(descriptor, data, size) _write((descriptor), (data), (unsigned)(size))
#define close(descriptor) _close(descriptor)
#define popen(command, mode) _popen((command), (mode))
#define pclose(file) _pclose(file)
#define flockfile(file) _lock_file(file)
#define funlockfile(file) _unlock_file(file)
#define strerror_r(code, buffer, size) strerror_s((buffer), (size), (code))
/* _pclose returns the command's exit code itself. */
#define WIFEXITED(status) 1
#define WEXITSTATUS(status) (status)
#define WTERMSIG(status) 0
#define _SC_NPROCESSORS_ONLN 84
static long sysconf(int name) {
    if (name != _SC_NPROCESSORS_ONLN) return -1;
    DWORD count = GetActiveProcessorCount(ALL_PROCESSOR_GROUPS);
    return count ? (long)count : 1;
}
/* Standard input may be shared with the session that started this program, which reads
   the next entry from the same pipe. The C runtime reads ahead of a line even when
   unbuffered, so take standard input one byte at a time from its handle. */
static int k_win_next_byte(FILE *file, HANDLE raw) {
    if (raw == INVALID_HANDLE_VALUE) return getc(file);
    unsigned char byte;
    DWORD got = 0;
    return ReadFile(raw, &byte, 1, &got, NULL) && got == 1 ? byte : EOF;
}
static ssize_t getline(char **line, size_t *capacity, FILE *file) {
    HANDLE raw = file == stdin ? (HANDLE)_get_osfhandle(_fileno(stdin)) : INVALID_HANDLE_VALUE;
    size_t length = 0;
    int c;
    while ((c = k_win_next_byte(file, raw)) != EOF) {
        if (length + 2 > *capacity) {
            size_t grown = *capacity ? *capacity * 2 : 128;
            char *next = realloc(*line, grown);
            if (!next) { errno = ENOMEM; return -1; }
            *line = next;
            *capacity = grown;
        }
        (*line)[length++] = (char)c;
        if (c == '\n') break;
    }
    if (!length) return -1;
    (*line)[length] = 0;
    return (ssize_t)length;
}
/* Replaces the trailing XXXXXX with fresh characters until a new folder appears. */
static char *mkdtemp(char *pattern) {
    size_t length = strlen(pattern);
    if (length < 6 || strcmp(pattern + length - 6, "XXXXXX")) { errno = EINVAL; return NULL; }
    static const char letters[] = "abcdefghijklmnopqrstuvwxyz0123456789";
    LARGE_INTEGER counter;
    for (int attempt = 0; attempt < 100; ++attempt) {
        QueryPerformanceCounter(&counter);
        uint64_t value = (uint64_t)counter.QuadPart * UINT64_C(6364136223846793005)
            ^ ((uint64_t)GetCurrentProcessId() << 32) ^ (uint64_t)attempt * UINT64_C(1442695040888963407);
        for (size_t i = 0; i < 6; ++i) {
            pattern[length - 6 + i] = letters[value % 36];
            value /= 36;
        }
        if (_mkdir(pattern) == 0) return pattern;
        if (errno != EEXIST) return NULL;
    }
    return NULL;
}

/* Clocks. */
#define CLOCK_REALTIME 0
#define CLOCK_MONOTONIC 1
static int clock_gettime(int clock, struct timespec *now) {
    if (clock == CLOCK_REALTIME) return timespec_get(now, TIME_UTC) ? 0 : -1;
    LARGE_INTEGER counter, frequency;
    QueryPerformanceCounter(&counter);
    QueryPerformanceFrequency(&frequency);
    now->tv_sec = (time_t)(counter.QuadPart / frequency.QuadPart);
    now->tv_nsec = (long)((counter.QuadPart % frequency.QuadPart) * 1000000000 / frequency.QuadPart);
    return 0;
}
/* Sleep can wake up to one timer tick early, so wait on the performance counter until the
   whole interval has passed, as POSIX nanosleep promises. */
static int nanosleep(const struct timespec *wait, struct timespec *rest) {
    /* Read the interval first: callers may pass the same struct as both arguments. */
    double total = (double)wait->tv_sec + (double)wait->tv_nsec / 1e9;
    if (rest) { rest->tv_sec = 0; rest->tv_nsec = 0; }
    LARGE_INTEGER frequency, start, now;
    QueryPerformanceFrequency(&frequency);
    QueryPerformanceCounter(&start);
    for (;;) {
        QueryPerformanceCounter(&now);
        double left = total - (double)(now.QuadPart - start.QuadPart) / (double)frequency.QuadPart;
        if (left <= 0) return 0;
        double milliseconds = left * 1000.0;
        Sleep(milliseconds >= 0x7fffffff ? 0x7fffffff : milliseconds < 1.0 ? 0 : (DWORD)milliseconds);
    }
}

/* The UCRT starts a program in the "C" locale and a Kestrel program never changes it,
   so number formatting needs no per-thread locale switch. */
typedef int locale_t;
#define LC_NUMERIC_MASK 0
static locale_t newlocale(int mask, const char *name, locale_t base) { (void)mask; (void)name; (void)base; return 1; }
static locale_t uselocale(locale_t locale) { return locale; }
#endif
/* A session entry is a shared library; Windows exports only the functions marked here. */
#ifdef _WIN32
#define K_EXPORT __declspec(dllexport)
#else
#define K_EXPORT
#endif
