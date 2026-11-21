/* Windows support for the runtime. The runtime is written against POSIX; on Windows this
   file supplies the part of pthreads, dlfcn, dirent and unistd that it uses, on top of
   the Win32 API and the Universal CRT. It serves both Clang flavours: MSVC, and the
   MinGW toolchain the Windows install bundles. Everything here is empty on other systems.

   MinGW's headers declare some POSIX names themselves, so every definition here has a
   k_win_ or KWin name, and macros map the POSIX names onto them only after every system
   header the runtime uses has been included. */
#ifdef _WIN32
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0A00
#endif
#if defined(__MINGW32__) && !defined(__USE_MINGW_ANSI_STDIO)
/* C99 printf for %zu, %lld and friends, as on the other platforms. */
#define __USE_MINGW_ANSI_STDIO 1
#endif
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
#include <float.h>
#include <inttypes.h>
#include <io.h>
#include <limits.h>
#include <locale.h>
#include <math.h>
#include <process.h>
#include <setjmp.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <time.h>

#if !defined(_SSIZE_T_DEFINED) && !defined(__MINGW32__)
typedef ptrdiff_t ssize_t;
#endif

/* Threads. A KWinThread keeps the handle for join and the id for equality. */
typedef struct { HANDLE handle; DWORD id; } KWinThread;
typedef struct { void *(*run)(void *); void *argument; } KWinThreadStart;
static unsigned __stdcall k_win_thread_entry(void *start) {
    KWinThreadStart copy = *(KWinThreadStart *)start;
    free(start);
    copy.run(copy.argument);
    return 0;
}
static int k_win_pthread_create(KWinThread *thread, const void *attributes, void *(*run)(void *), void *argument) {
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
static int k_win_pthread_join(KWinThread thread, void **result) {
    if (result) *result = NULL;
    if (WaitForSingleObject(thread.handle, INFINITE) != WAIT_OBJECT_0) return EINVAL;
    CloseHandle(thread.handle);
    return 0;
}
static KWinThread k_win_pthread_self(void) {
    KWinThread self = { NULL, GetCurrentThreadId() };
    return self;
}
static int k_win_pthread_equal(KWinThread a, KWinThread b) { return a.id == b.id; }
static int k_win_sched_yield(void) { SwitchToThread(); return 0; }

/* Mutexes. An SRW lock plus the owning thread, so recursive and error-checking
   mutexes can tell a second lock from the same thread. */
enum { K_WIN_MUTEX_NORMAL = 0, K_WIN_MUTEX_RECURSIVE = 1, K_WIN_MUTEX_ERRORCHECK = 2 };
typedef struct { int kind; } KWinMutexAttributes;
typedef struct { SRWLOCK lock; volatile LONG owner; unsigned depth; int kind; } KWinMutex;
#undef PTHREAD_MUTEX_INITIALIZER
#define PTHREAD_MUTEX_INITIALIZER { SRWLOCK_INIT, 0, 0, K_WIN_MUTEX_NORMAL }
static int k_win_pthread_mutexattr_init(KWinMutexAttributes *attributes) { attributes->kind = K_WIN_MUTEX_NORMAL; return 0; }
static int k_win_pthread_mutexattr_settype(KWinMutexAttributes *attributes, int kind) { attributes->kind = kind; return 0; }
static int k_win_pthread_mutexattr_destroy(KWinMutexAttributes *attributes) { (void)attributes; return 0; }
static int k_win_pthread_mutex_init(KWinMutex *mutex, const KWinMutexAttributes *attributes) {
    InitializeSRWLock(&mutex->lock);
    mutex->owner = 0;
    mutex->depth = 0;
    mutex->kind = attributes ? attributes->kind : K_WIN_MUTEX_NORMAL;
    return 0;
}
static int k_win_pthread_mutex_destroy(KWinMutex *mutex) { return mutex->owner ? EBUSY : 0; }
static int k_win_pthread_mutex_lock(KWinMutex *mutex) {
    LONG self = (LONG)GetCurrentThreadId();
    if (mutex->kind != K_WIN_MUTEX_NORMAL && mutex->owner == self) {
        if (mutex->kind == K_WIN_MUTEX_ERRORCHECK) return EDEADLK;
        ++mutex->depth;
        return 0;
    }
    AcquireSRWLockExclusive(&mutex->lock);
    InterlockedExchange(&mutex->owner, self);
    mutex->depth = 0;
    return 0;
}
static int k_win_pthread_mutex_trylock(KWinMutex *mutex) {
    LONG self = (LONG)GetCurrentThreadId();
    if (mutex->owner == self) {
        if (mutex->kind != K_WIN_MUTEX_RECURSIVE) return EBUSY;
        ++mutex->depth;
        return 0;
    }
    if (!TryAcquireSRWLockExclusive(&mutex->lock)) return EBUSY;
    InterlockedExchange(&mutex->owner, self);
    mutex->depth = 0;
    return 0;
}
static int k_win_pthread_mutex_unlock(KWinMutex *mutex) {
    if (mutex->kind != K_WIN_MUTEX_NORMAL && mutex->owner != (LONG)GetCurrentThreadId()) return EPERM;
    if (mutex->depth) { --mutex->depth; return 0; }
    InterlockedExchange(&mutex->owner, 0);
    ReleaseSRWLockExclusive(&mutex->lock);
    return 0;
}

/* Condition variables wait on the mutex's SRW lock. */
typedef struct { CONDITION_VARIABLE variable; } KWinCondition;
#undef PTHREAD_COND_INITIALIZER
#define PTHREAD_COND_INITIALIZER { CONDITION_VARIABLE_INIT }
static int k_win_pthread_cond_init(KWinCondition *condition, const void *attributes) {
    (void)attributes;
    InitializeConditionVariable(&condition->variable);
    return 0;
}
static int k_win_pthread_cond_destroy(KWinCondition *condition) { (void)condition; return 0; }
static int k_win_pthread_cond_wait(KWinCondition *condition, KWinMutex *mutex) {
    unsigned depth = mutex->depth;
    InterlockedExchange(&mutex->owner, 0);
    BOOL woke = SleepConditionVariableSRW(&condition->variable, &mutex->lock, INFINITE, 0);
    InterlockedExchange(&mutex->owner, (LONG)GetCurrentThreadId());
    mutex->depth = depth;
    return woke ? 0 : EINVAL;
}
static int k_win_pthread_cond_signal(KWinCondition *condition) { WakeConditionVariable(&condition->variable); return 0; }
static int k_win_pthread_cond_broadcast(KWinCondition *condition) { WakeAllConditionVariable(&condition->variable); return 0; }

/* One-time initialization. */
typedef INIT_ONCE KWinOnce;
#undef PTHREAD_ONCE_INIT
#define PTHREAD_ONCE_INIT INIT_ONCE_STATIC_INIT
static BOOL CALLBACK k_win_once_entry(PINIT_ONCE once, PVOID run, PVOID *context) {
    (void)once;
    (void)context;
    void (*function)(void);
    memcpy(&function, &run, sizeof function);
    function();
    return TRUE;
}
static int k_win_pthread_once(KWinOnce *once, void (*run)(void)) {
    PVOID argument;
    memcpy(&argument, &run, sizeof argument);
    return InitOnceExecuteOnce(once, k_win_once_entry, argument, NULL) ? 0 : EINVAL;
}

/* Shared libraries. LoadLibrary searches the program's folder and PATH. */
#undef RTLD_LAZY
#define RTLD_LAZY 1
#undef RTLD_NOW
#define RTLD_NOW 2
#undef RTLD_LOCAL
#define RTLD_LOCAL 0
static char k_win_library_error[512];
static void *k_win_dlopen(const char *name, int flags) {
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
static void *k_win_dlsym(void *library, const char *name) {
    FARPROC found = GetProcAddress((HMODULE)library, name);
    void *symbol;
    memcpy(&symbol, &found, sizeof symbol);
    return symbol;
}
static int k_win_dlclose(void *library) { return FreeLibrary((HMODULE)library) ? 0 : -1; }
static const char *k_win_dlerror(void) { return k_win_library_error; }

/* Directory listing. */
struct k_win_dirent { char d_name[MAX_PATH]; };
typedef struct { HANDLE find; WIN32_FIND_DATAA data; bool pending; struct k_win_dirent entry; } KWinDir;
static KWinDir *k_win_opendir(const char *name) {
    size_t length = strlen(name);
    char *pattern = malloc(length + 3);
    if (!pattern) { errno = ENOMEM; return NULL; }
    memcpy(pattern, name, length);
    size_t end = length;
    if (end && name[end - 1] != '/' && name[end - 1] != '\\') pattern[end++] = '\\';
    pattern[end++] = '*';
    pattern[end] = 0;
    KWinDir *directory = calloc(1, sizeof *directory);
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
static struct k_win_dirent *k_win_readdir(KWinDir *directory) {
    if (!directory->pending && !FindNextFileA(directory->find, &directory->data)) return NULL;
    directory->pending = false;
    strncpy(directory->entry.d_name, directory->data.cFileName, sizeof directory->entry.d_name - 1);
    directory->entry.d_name[sizeof directory->entry.d_name - 1] = 0;
    return &directory->entry;
}
static int k_win_closedir(KWinDir *directory) {
    FindClose(directory->find);
    free(directory);
    return 0;
}

/* Files, processes and the C library names that differ. */
#undef S_ISDIR
#define S_ISDIR(mode) (((mode) & _S_IFMT) == _S_IFDIR)
#undef S_ISREG
#define S_ISREG(mode) (((mode) & _S_IFMT) == _S_IFREG)
#undef mkdir
#define mkdir(name, mode) _mkdir(name)
#undef getcwd
#define getcwd(buffer, size) _getcwd((buffer), (int)(size))
#undef getpid
#define getpid() _getpid()
#undef fileno
#define fileno(file) _fileno(file)
#undef fdopen
#define fdopen(descriptor, mode) _fdopen((descriptor), (mode))
#undef write
#define write(descriptor, data, size) _write((descriptor), (data), (unsigned)(size))
#undef close
#define close(descriptor) _close(descriptor)
#undef popen
#define popen(command, mode) _popen((command), (mode))
#undef pclose
#define pclose(file) _pclose(file)
#undef flockfile
#define flockfile(file) _lock_file(file)
#undef funlockfile
#define funlockfile(file) _unlock_file(file)
#undef strerror_r
#define strerror_r(code, buffer, size) strerror_s((buffer), (size), (code))
/* _pclose returns the command's exit code itself. */
#undef WIFEXITED
#define WIFEXITED(status) 1
#undef WEXITSTATUS
#define WEXITSTATUS(status) (status)
#undef WTERMSIG
#define WTERMSIG(status) 0
#undef _SC_NPROCESSORS_ONLN
#define _SC_NPROCESSORS_ONLN 84
static long k_win_sysconf(int name) {
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
static ssize_t k_win_getline(char **line, size_t *capacity, FILE *file) {
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
static char *k_win_mkdtemp(char *pattern) {
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
#undef CLOCK_REALTIME
#define CLOCK_REALTIME 0
#undef CLOCK_MONOTONIC
#define CLOCK_MONOTONIC 1
static int k_win_clock_gettime(int clock, struct timespec *now) {
    if (clock == CLOCK_REALTIME) return timespec_get(now, TIME_UTC) ? 0 : -1;
    LARGE_INTEGER counter, frequency;
    QueryPerformanceCounter(&counter);
    QueryPerformanceFrequency(&frequency);
    now->tv_sec = (time_t)(counter.QuadPart / frequency.QuadPart);
    now->tv_nsec = (long)((counter.QuadPart % frequency.QuadPart) * 1000000000 / frequency.QuadPart);
    return 0;
}
/* Sleep can wake up to one timer tick early, so wait on the performance counter until the
   whole interval has passed, as POSIX k_win_nanosleep promises. */
static int k_win_nanosleep(const struct timespec *wait, struct timespec *rest) {
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
typedef int KWinLocale;
#undef LC_NUMERIC_MASK
#define LC_NUMERIC_MASK 0
static KWinLocale k_win_newlocale(int mask, const char *name, KWinLocale base) { (void)mask; (void)name; (void)base; return 1; }
static KWinLocale k_win_uselocale(KWinLocale locale) { return locale; }

/* The POSIX names the runtime uses. */
#undef pthread_create
#define pthread_create k_win_pthread_create
#undef pthread_join
#define pthread_join k_win_pthread_join
#undef pthread_self
#define pthread_self k_win_pthread_self
#undef pthread_equal
#define pthread_equal k_win_pthread_equal
#undef sched_yield
#define sched_yield k_win_sched_yield
#undef pthread_mutexattr_init
#define pthread_mutexattr_init k_win_pthread_mutexattr_init
#undef pthread_mutexattr_settype
#define pthread_mutexattr_settype k_win_pthread_mutexattr_settype
#undef pthread_mutexattr_destroy
#define pthread_mutexattr_destroy k_win_pthread_mutexattr_destroy
#undef pthread_mutex_init
#define pthread_mutex_init k_win_pthread_mutex_init
#undef pthread_mutex_destroy
#define pthread_mutex_destroy k_win_pthread_mutex_destroy
#undef pthread_mutex_lock
#define pthread_mutex_lock k_win_pthread_mutex_lock
#undef pthread_mutex_trylock
#define pthread_mutex_trylock k_win_pthread_mutex_trylock
#undef pthread_mutex_unlock
#define pthread_mutex_unlock k_win_pthread_mutex_unlock
#undef pthread_cond_init
#define pthread_cond_init k_win_pthread_cond_init
#undef pthread_cond_destroy
#define pthread_cond_destroy k_win_pthread_cond_destroy
#undef pthread_cond_wait
#define pthread_cond_wait k_win_pthread_cond_wait
#undef pthread_cond_signal
#define pthread_cond_signal k_win_pthread_cond_signal
#undef pthread_cond_broadcast
#define pthread_cond_broadcast k_win_pthread_cond_broadcast
#undef pthread_once
#define pthread_once k_win_pthread_once
#undef dlopen
#define dlopen k_win_dlopen
#undef dlsym
#define dlsym k_win_dlsym
#undef dlclose
#define dlclose k_win_dlclose
#undef dlerror
#define dlerror k_win_dlerror
#undef opendir
#define opendir k_win_opendir
#undef readdir
#define readdir k_win_readdir
#undef closedir
#define closedir k_win_closedir
#undef sysconf
#define sysconf k_win_sysconf
#undef getline
#define getline k_win_getline
#undef mkdtemp
#define mkdtemp k_win_mkdtemp
#undef clock_gettime
#define clock_gettime k_win_clock_gettime
#undef nanosleep
#define nanosleep k_win_nanosleep
#undef newlocale
#define newlocale k_win_newlocale
#undef uselocale
#define uselocale k_win_uselocale
#undef pthread_t
#define pthread_t KWinThread
#undef pthread_mutexattr_t
#define pthread_mutexattr_t KWinMutexAttributes
#undef pthread_mutex_t
#define pthread_mutex_t KWinMutex
#undef pthread_cond_t
#define pthread_cond_t KWinCondition
#undef pthread_once_t
#define pthread_once_t KWinOnce
#undef DIR
#define DIR KWinDir
#undef locale_t
#define locale_t KWinLocale
#undef dirent
#define dirent k_win_dirent
#undef PTHREAD_MUTEX_NORMAL
#define PTHREAD_MUTEX_NORMAL K_WIN_MUTEX_NORMAL
#undef PTHREAD_MUTEX_RECURSIVE
#define PTHREAD_MUTEX_RECURSIVE K_WIN_MUTEX_RECURSIVE
#undef PTHREAD_MUTEX_ERRORCHECK
#define PTHREAD_MUTEX_ERRORCHECK K_WIN_MUTEX_ERRORCHECK
#endif
/* A session entry is a shared library; Windows exports only the functions marked here. */
#ifdef _WIN32
#define K_EXPORT __declspec(dllexport)
#else
#define K_EXPORT
#endif
