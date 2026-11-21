// Native BLAS bridge. Matrix products and dot products go through the system's
// CBLAS when a library is present (Accelerate on macOS, OpenBLAS or a BLAS
// package on Linux, OpenBLAS on PATH on Windows) and fall back to the plain loops in the caller otherwise.
// Nothing here needs a link flag: the library is opened on first use.

#ifndef _WIN32
#include <dlfcn.h>
#endif

typedef void (*KBlasDgemm)(int, int, int, int, int, int, double, const double *, int, const double *, int, double, double *, int);
typedef void (*KBlasDgemv)(int, int, int, int, double, const double *, int, const double *, int, double, double *, int);
typedef void (*KBlasDaxpy)(int, double, const double *, int, double *, int);
typedef void (*KBlasDscal)(int, double, double *, int);
typedef double (*KBlasDdot)(int, const double *, int, const double *, int);

enum { K_BLAST_ROW_MAJOR = 101, K_BLAST_NO_TRANS = 111, K_BLAST_TRANS = 112 };

static void *k_blas_handle = NULL;
static KBlasDgemm k_blas_dgemm_fn = NULL;
static KBlasDgemv k_blas_dgemv_fn = NULL;
static KBlasDaxpy k_blas_daxpy_fn = NULL;
static KBlasDscal k_blas_dscal_fn = NULL;
static KBlasDdot k_blas_ddot_fn = NULL;

static void k_blas_initialize(void) {
    /* KESTREL_LAPACK names a library outside the search path, as Homebrew's OpenBLAS is. */
    const char *chosen = getenv("KESTREL_LAPACK");
    const char *libraries[] = {
        chosen && *chosen ? chosen : "",
        "/System/Library/Frameworks/Accelerate.framework/Accelerate",
        "libopenblas.so.0",
        "libblas.so.3",
        "libblas.so",
        "libopenblas.dll",
        "openblas.dll",
        NULL,
    };
    for (int i = 0; libraries[i]; ++i) {
        if (!*libraries[i]) continue;
        void *handle = dlopen(libraries[i], RTLD_LAZY | RTLD_LOCAL);
        if (!handle) continue;
        void *gemm = dlsym(handle, "cblas_dgemm");
        void *gemv = dlsym(handle, "cblas_dgemv");
        if (!gemm || !gemv) { dlclose(handle); continue; }
        k_blas_handle = handle;
        k_blas_dgemm_fn = (KBlasDgemm)gemm;
        k_blas_dgemv_fn = (KBlasDgemv)gemv;
        k_blas_daxpy_fn = (KBlasDaxpy)dlsym(handle, "cblas_daxpy");
        k_blas_dscal_fn = (KBlasDscal)dlsym(handle, "cblas_dscal");
        k_blas_ddot_fn = (KBlasDdot)dlsym(handle, "cblas_ddot");
        return;
    }
}

static bool k_blas_ready(void) {
    static pthread_once_t once = PTHREAD_ONCE_INIT;
    pthread_once(&once, k_blas_initialize);
    return k_blas_dgemm_fn != NULL;
}

// k_blas_dgemm multiplies a (rows x inner) by b (inner x cols) into c
// (rows x cols, row major). When trans_a is set, a's storage holds the
// transpose (inner x rows), and likewise for b. Returns false when no CBLAS
// library is available, leaving c untouched.
static bool k_blas_dgemm(int64_t rows, int64_t inner, int64_t cols,
                         const double *a, bool trans_a, const double *b, bool trans_b,
                         double *c) {
    if (!k_blas_ready()) return false;
    if (rows < 0 || inner < 0 || cols < 0 || rows > INT_MAX || inner > INT_MAX || cols > INT_MAX) return false;
    // A zero-sized product is already an empty or zero result; CBLAS rejects
    // a zero leading dimension.
    if (rows == 0 || inner == 0 || cols == 0) return true;
    int m = (int)rows, k = (int)inner, n = (int)cols;
    int lda = trans_a ? m : k;
    int ldb = trans_b ? k : n;
    k_blas_dgemm_fn(K_BLAST_ROW_MAJOR, trans_a ? K_BLAST_TRANS : K_BLAST_NO_TRANS, trans_b ? K_BLAST_TRANS : K_BLAST_NO_TRANS,
                    m, n, k, 1.0, a, lda, b, ldb, 0.0, c, n);
    return true;
}

static bool k_blas_dgemv(int64_t rows, int64_t cols, const double *a, bool trans_a,
                         const double *x, double *y) {
    if (!k_blas_ready() || !k_blas_dgemv_fn) return false;
    if (rows < 0 || cols < 0 || rows > INT_MAX || cols > INT_MAX) return false;
    int m = (int)rows, n = (int)cols;
    // A vector on the left is a row vector, so multiply A^T by x.
    if (trans_a) {
        k_blas_dgemv_fn(K_BLAST_ROW_MAJOR, K_BLAST_TRANS, n, m, 1.0, a, m, x, 1, 0.0, y, 1);
    } else {
        k_blas_dgemv_fn(K_BLAST_ROW_MAJOR, K_BLAST_NO_TRANS, m, n, 1.0, a, n, x, 1, 0.0, y, 1);
    }
    return true;
}

static bool k_blas_ddot(int64_t count, const double *a, const double *b, double *result) {
    if (!k_blas_ready() || !k_blas_ddot_fn) return false;
    if (count < 0 || count > INT_MAX) return false;
    *result = k_blas_ddot_fn((int)count, a, 1, b, 1);
    return true;
}
