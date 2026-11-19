// Native numerical kernels. LAPACK is loaded only when a decomposition is requested.
#include <dlfcn.h>
#include <float.h>
#include <limits.h>
#include <unistd.h>

typedef struct { KTensor first, second, third; } KNumericalParts;
static void *k_lapack_handle = NULL;
static pthread_once_t k_lapack_once = PTHREAD_ONCE_INIT;
static void k_lapack_initialize(void) {
    const char *libraries[] = {"/System/Library/Frameworks/Accelerate.framework/Accelerate", "liblapack.so.3", "libopenblas.so.0", "liblapack.so", NULL};
    for (int i = 0; libraries[i]; ++i) {
        k_lapack_handle = dlopen(libraries[i], RTLD_LAZY | RTLD_LOCAL);
        if (k_lapack_handle) return;
    }
}
static void *k_lapack_symbol(const char *name, KLocation loc) {
    pthread_once(&k_lapack_once, k_lapack_initialize);
    if (!k_lapack_handle) k_trap(loc, "Native LAPACK backend is unavailable; install a compatible LP64 LAPACK library");
    void *symbol = dlsym(k_lapack_handle, name);
    if (!symbol) k_trap(loc, "The native LAPACK backend is missing a required routine");
    return symbol;
}
static void *k_numerical_alloc(size_t count, size_t size, KLocation loc) {
    if (size && count > SIZE_MAX / size) k_trap(loc, "Numerical workspace is too large");
    void *result = calloc(count ? count : 1, size);
    if (!result) k_fatal("Out of memory");
    return result;
}
static void k_linalg_finite(const KTensor *a, KLocation loc) {
    for (int64_t i = 0; i < a->length; ++i) if (!isfinite(a->items[i])) k_trap(loc, "Linear algebra requires finite matrix elements");
}
static void k_linalg_matrix(const KTensor *a, bool square, KLocation loc) {
    if (a->rank != 2) k_trap(loc, "Linear algebra requires a rank-2 matrix");
    if (square && a->shape[0] != a->shape[1]) k_trap(loc, "Linear algebra requires a square matrix");
    if (a->shape[0] > INT_MAX || a->shape[1] > INT_MAX) k_trap(loc, "Matrix dimensions exceed the LAPACK integer range");
    k_linalg_finite(a, loc);
}
static void k_linalg_symmetric(const KTensor *a, KLocation loc) {
    k_linalg_matrix(a, true, loc);
    int64_t n = a->shape[0];
    for (int64_t i = 0; i < n; ++i) for (int64_t j = 0; j < i; ++j)
        if (a->items[i*n+j] != a->items[j*n+i]) k_trap(loc, "This operation requires a symmetric matrix");
}
static double *k_linalg_columns(const KTensor *a, KLocation loc) {
    double *result = k_numerical_alloc((size_t)a->length, sizeof(double), loc);
    // A blocked transpose: the tile fits in cache and each destination row
    // leaves as one contiguous copy.
    int64_t rows = a->shape[0], columns = a->shape[1];
    const int64_t block = 32;
    double tile[32][32];
    for (int64_t i0 = 0; i0 < rows; i0 += block)
        for (int64_t j0 = 0; j0 < columns; j0 += block) {
            int64_t i1 = i0 + block < rows ? i0 + block : rows;
            int64_t j1 = j0 + block < columns ? j0 + block : columns;
            for (int64_t i = i0; i < i1; ++i)
                for (int64_t j = j0; j < j1; ++j) tile[j - j0][i - i0] = a->items[i * columns + j];
            for (int64_t j = j0; j < j1; ++j)
                memcpy(result + j * rows + i0, tile[j - j0], (size_t)(i1 - i0) * sizeof(double));
        }
    return result;
}
static void k_linalg_rows(KTensor *a, const double *columns) {
    int64_t rows = a->shape[0], cols = a->shape[1];
    const int64_t block = 32;
    double tile[32][32];
    for (int64_t i0 = 0; i0 < rows; i0 += block)
        for (int64_t j0 = 0; j0 < cols; j0 += block) {
            int64_t i1 = i0 + block < rows ? i0 + block : rows;
            int64_t j1 = j0 + block < cols ? j0 + block : cols;
            for (int64_t j = j0; j < j1; ++j)
                memcpy(tile[j - j0], columns + j * rows + i0, (size_t)(i1 - i0) * sizeof(double));
            for (int64_t i = i0; i < i1; ++i)
                for (int64_t j = j0; j < j1; ++j) a->items[i * cols + j] = tile[j - j0][i - i0];
        }
}

static int k_linalg_workspace(double query, KLocation loc) {
    if (!isfinite(query) || query < 1.0 || query > INT_MAX) k_trap(loc, "LAPACK workspace is too large");
    return (int)ceil(query);
}
static void k_linalg_info(int info, KLocation loc) {
    if (info < 0) k_trap(loc, "The native numerical backend rejected an argument");
    if (info > 0) k_trap(loc, "The numerical algorithm did not converge");
}
static double k_linalg_dot(const KTensor *a, const KTensor *b, KLocation loc) {
    if (a->rank != 1 || b->rank != 1) k_trap(loc, "dot requires two rank-1 vectors");
    if (a->length != b->length) k_trap(loc, "dot requires vectors of equal length");
    double result = 0.0;
    if (k_blas_ddot(a->length, a->items, b->items, &result)) return result;
    for (int64_t i = 0; i < a->length; ++i) result += a->items[i] * b->items[i];
    return result;
}
static KTensor k_linalg_outer(const KTensor *a, const KTensor *b, KLocation loc) {
    if (a->rank != 1 || b->rank != 1) k_trap(loc, "outer requires two rank-1 vectors");
    int64_t shape[] = {a->length, b->length};
    KTensor result = k_tensor_new(2, shape, 0.0, loc);
    for (int64_t i = 0; i < a->length; ++i) for (int64_t j = 0; j < b->length; ++j) result.items[i*b->length+j] = a->items[i] * b->items[j];
    return result;
}
static KNumericalParts k_linalg_lu(const KTensor *a, KLocation loc) {
    k_linalg_matrix(a, false, loc);
    int m = (int)a->shape[0], n = (int)a->shape[1], k = m<n?m:n, info = 0;
    int64_t pivot_shape[]={k};
    KNumericalParts result = {k_tensor_copy(a, loc), k_tensor_new(1, pivot_shape, 0.0, loc), {0}};
    if (!k) return result;
    double *data = k_linalg_columns(a, loc);
    int *pivots = k_numerical_alloc((size_t)k, sizeof(int), loc);
    typedef void (*Fn)(int*,int*,double*,int*,int*,int*);
    ((Fn)k_lapack_symbol("dgetrf_",loc))(&m,&n,data,&m,pivots,&info);
    if (info < 0) k_linalg_info(info,loc);
    k_linalg_rows(&result.first,data);
    for (int i = 0; i < k; ++i) result.second.items[i] = pivots[i]-1;
    free(data); free(pivots); return result;
}
static KTensor k_linalg_factor_solve(const KTensor *factors, const KTensor *pivots, const KTensor *b, KLocation loc) {
    k_linalg_matrix(factors,true,loc);
    int n = (int)factors->shape[0];
    if ((b->rank!=1 && b->rank!=2) || b->shape[0]!=n) k_trap(loc,"Linear solve has incompatible right-hand dimensions");
    if (pivots->rank!=1 || pivots->length!=n) k_trap(loc,"LU pivot shape is invalid");
    k_linalg_finite(b,loc);
    int nrhs = b->rank==2 ? (b->shape[1]>INT_MAX ? -1 : (int)b->shape[1]) : 1;
    if (nrhs<0) k_trap(loc,"Too many right-hand sides for LAPACK");
    int *indices=k_numerical_alloc((size_t)n,sizeof(int),loc);
    for(int i=0;i<n;++i) {
        double p=pivots->items[i];
        if(!isfinite(p)||p<i||p>=n||floor(p)!=p) k_trap(loc,"LU pivot value is invalid");
        indices[i]=(int)p+1;
        if(factors->items[(int64_t)i*n+i]==0.0) k_trap(loc,"Matrix is singular");
    }
    KTensor result=k_tensor_copy(b,loc);
    if(!n||!nrhs) {free(indices); return result;}
    double *data=k_linalg_columns(factors,loc);
    double *rhs=k_numerical_alloc((size_t)b->length,sizeof(double),loc);
    for(int i=0;i<n;++i) for(int j=0;j<nrhs;++j) rhs[(int64_t)j*n+i]=b->items[(int64_t)i*nrhs+j];
    int info=0; char trans='N';
    typedef void (*Fn)(char*,int*,int*,double*,int*,int*,double*,int*,int*,size_t);
    ((Fn)k_lapack_symbol("dgetrs_",loc))(&trans,&n,&nrhs,data,&n,indices,rhs,&n,&info,1);
    k_linalg_info(info,loc);
    for(int i=0;i<n;++i) for(int j=0;j<nrhs;++j) result.items[(int64_t)i*nrhs+j]=rhs[(int64_t)j*n+i];
    free(indices);free(data);free(rhs);k_linalg_finite(&result,loc);return result;
}
static KTensor k_linalg_factor_solve_array(const KTensor *factors,int64_t count,const int64_t *pivots,const KTensor *b,KLocation loc) {
    k_linalg_matrix(factors,true,loc);
    if(count!=factors->shape[0]) k_trap(loc,"LU pivot shape is invalid");
    KTensor metadata=k_tensor_new(1,&count,0.0,loc);
    for(int64_t i=0;i<count;++i) metadata.items[i]=(double)pivots[i];
    KTensor result=k_linalg_factor_solve(factors,&metadata,b,loc);k_tensor_drop(&metadata);return result;
}
static KTensor k_linalg_solve(const KTensor *a, const KTensor *b, int triangular, KLocation loc) {
    if(!triangular) {
        // Factor and solve entirely in column-major storage. Going through the
        // row-major LU view would transpose the matrix twice more.
        k_linalg_matrix(a,true,loc);
        int n=(int)a->shape[0];
        if((b->rank!=1&&b->rank!=2)||b->shape[0]!=n) k_trap(loc,"Linear solve has incompatible right-hand dimensions");
        k_linalg_finite(b,loc);
        int nrhs=b->rank==2?(b->shape[1]>INT_MAX?-1:(int)b->shape[1]):1;
        if(nrhs<0) k_trap(loc,"Too many right-hand sides for LAPACK");
        int64_t rows=b->shape[0];
        double *data=k_linalg_columns(a,loc);
        int *pivots=k_numerical_alloc((size_t)n,sizeof(int),loc);
        int info=0;
        double *rhs=k_numerical_alloc((size_t)b->length,sizeof(double),loc);
        if(b->rank==1) memcpy(rhs,b->items,(size_t)n*sizeof(double));
        else for(int i=0;i<n;++i) for(int j=0;j<nrhs;++j) rhs[(int64_t)j*n+i]=b->items[(int64_t)i*nrhs+j];
        typedef void (*Solve)(int*,int*,double*,int*,int*,double*,int*,int*);
        ((Solve)k_lapack_symbol("dgesv_",loc))(&n,&nrhs,data,&n,pivots,rhs,&n,&info);
        if(info>0) k_trap(loc,"Matrix is singular");
        k_linalg_info(info,loc);
        int64_t shape[]={rows};
        KTensor result=b->rank==1?k_tensor_new(1,shape,0.0,loc):k_tensor_new(2,b->shape,0.0,loc);
        if(b->rank==1) memcpy(result.items,rhs,(size_t)n*sizeof(double));
        else for(int i=0;i<n;++i) for(int j=0;j<nrhs;++j) result.items[(int64_t)i*nrhs+j]=rhs[(int64_t)j*n+i];
        free(data);free(pivots);free(rhs);
        k_linalg_finite(&result,loc);
        return result;
    }
    k_linalg_matrix(a,true,loc);
    int64_t n=a->shape[0];
    if((b->rank!=1&&b->rank!=2)||b->shape[0]!=n) k_trap(loc,"Linear solve has incompatible right-hand dimensions");
    k_linalg_finite(b,loc);KTensor result=k_tensor_copy(b,loc);
    int64_t cols=b->rank==2?b->shape[1]:1;
    for(int64_t i=0;i<n;++i) for(int64_t j=0;j<n;++j)
        if((triangular==1?j>i:j<i)&&a->items[i*n+j]!=0.0) k_trap(loc,"Matrix is not triangular");
    for(int64_t step=0;step<n;++step) {
        int64_t i=triangular==1?step:n-1-step;
        double diagonal=a->items[i*n+i]; if(diagonal==0.0) k_trap(loc,"Matrix is singular");
        for(int64_t j=0;j<cols;++j) {
            double v=result.items[i*cols+j];
            for(int64_t k=0;k<n;++k) if(triangular==1?k<i:k>i) v-=a->items[i*n+k]*result.items[k*cols+j];
            result.items[i*cols+j]=v/diagonal;
        }
    }
    k_linalg_finite(&result,loc);return result;
}
static KTensor k_linalg_inverse(const KTensor *a,KLocation loc) {
    k_linalg_matrix(a,true,loc);int64_t n=a->shape[0];
    KTensor identity=k_tensor_new(2,a->shape,0.0,loc);
    for(int64_t i=0;i<n;++i) identity.items[i*n+i]=1.0;
    KTensor result=k_linalg_solve(a,&identity,0,loc);k_tensor_drop(&identity);return result;
}
static double k_linalg_det(const KTensor *a,KLocation loc) {
    k_linalg_matrix(a,true,loc);
    KNumericalParts lu=k_linalg_lu(a,loc);double result=1.0;int64_t n=a->shape[0];
    for(int64_t i=0;i<n;++i) {result*=lu.first.items[i*n+i];if(lu.second.items[i]!=i) result=-result;}
    k_tensor_drop(&lu.first);k_tensor_drop(&lu.second);return result;
}
static KTensor k_linalg_cholesky(const KTensor *a,KLocation loc) {
    k_linalg_symmetric(a,loc);int n=(int)a->shape[0],info=0;KTensor result=k_tensor_copy(a,loc);
    if(!n) return result;
    double *data=k_linalg_columns(a,loc);char lower='L';
    typedef void (*Fn)(char*,int*,double*,int*,int*,size_t);
    ((Fn)k_lapack_symbol("dpotrf_",loc))(&lower,&n,data,&n,&info,1);
    if(info>0) k_trap(loc,"Matrix is not positive definite");k_linalg_info(info,loc);
    for(int i=0;i<n;++i) for(int j=0;j<n;++j) result.items[(int64_t)i*n+j]=j<=i?data[(int64_t)j*n+i]:0.0;
    free(data);return result;
}
static KNumericalParts k_linalg_qr(const KTensor *a,KLocation loc) {
    k_linalg_matrix(a,false,loc);int m=(int)a->shape[0],n=(int)a->shape[1],k=m<n?m:n,info=0,lwork=-1;
    int64_t qs[]={m,k},rs[]={k,n};
    KNumericalParts result={k_tensor_new(2,qs,0.0,loc),k_tensor_new(2,rs,0.0,loc),{0}};
    if(!k) return result;
    double *data=k_linalg_columns(a,loc),*tau=k_numerical_alloc((size_t)k,sizeof(double),loc),query=0.0;
    typedef void (*QR)(int*,int*,double*,int*,double*,double*,int*,int*);
    QR qr=(QR)k_lapack_symbol("dgeqrf_",loc);
    qr(&m,&n,data,&m,tau,&query,&lwork,&info);k_linalg_info(info,loc);lwork=k_linalg_workspace(query,loc);
    double *work=k_numerical_alloc((size_t)lwork,sizeof(double),loc);
    qr(&m,&n,data,&m,tau,work,&lwork,&info);k_linalg_info(info,loc);free(work);
    for(int i=0;i<k;++i) for(int j=i;j<n;++j) result.second.items[(int64_t)i*n+j]=data[(int64_t)j*m+i];
    typedef void (*Q)(int*,int*,int*,double*,int*,double*,double*,int*,int*);
    Q form=(Q)k_lapack_symbol("dorgqr_",loc);lwork=-1;
    form(&m,&k,&k,data,&m,tau,&query,&lwork,&info);k_linalg_info(info,loc);lwork=k_linalg_workspace(query,loc);
    work=k_numerical_alloc((size_t)lwork,sizeof(double),loc);
    form(&m,&k,&k,data,&m,tau,work,&lwork,&info);k_linalg_info(info,loc);
    k_linalg_rows(&result.first,data);free(work);free(tau);free(data);return result;
}
static KNumericalParts k_linalg_svd(const KTensor *a,KLocation loc) {
    k_linalg_matrix(a,false,loc);int m=(int)a->shape[0],n=(int)a->shape[1],k=m<n?m:n,info=0,lwork=-1;
    int64_t us[]={m,k},ss[]={k},vs[]={k,n};
    KNumericalParts result={k_tensor_new(2,us,0.0,loc),k_tensor_new(1,ss,0.0,loc),k_tensor_new(2,vs,0.0,loc)};
    if(!k) return result;
    double *data=k_linalg_columns(a,loc),*u=k_numerical_alloc((size_t)m*k,sizeof(double),loc),*vt=k_numerical_alloc((size_t)k*n,sizeof(double),loc),query=0.0;char reduced='S';
    int *iwork=k_numerical_alloc((size_t)8*k,sizeof(int),loc);
    // Divide-and-conquer SVD: the same reduced factors as dgesvd, and much
    // faster on the square matrices these benchmarks use.
    typedef void (*Fn)(char*,int*,int*,double*,int*,double*,double*,int*,double*,int*,double*,int*,int*,int*,size_t);
    Fn svd=(Fn)k_lapack_symbol("dgesdd_",loc);
    svd(&reduced,&m,&n,data,&m,result.second.items,u,&m,vt,&k,&query,&lwork,iwork,&info,1);k_linalg_info(info,loc);lwork=k_linalg_workspace(query,loc);
    double *work=k_numerical_alloc((size_t)lwork,sizeof(double),loc);
    svd(&reduced,&m,&n,data,&m,result.second.items,u,&m,vt,&k,work,&lwork,iwork,&info,1);k_linalg_info(info,loc);
    k_linalg_rows(&result.first,u);k_linalg_rows(&result.third,vt);
    free(data);free(u);free(vt);free(work);free(iwork);return result;
}
static KNumericalParts k_linalg_eigh(const KTensor *a,KLocation loc) {
    k_linalg_symmetric(a,loc);int n=(int)a->shape[0],info=0,lwork=-1,liwork=-1;
    KNumericalParts result={k_tensor_new(1,a->shape,0.0,loc),k_tensor_copy(a,loc),{0}};
    if(!n) return result;
    double *data=k_linalg_columns(a,loc),query=0.0;char vectors='V',lower='L';
    // Divide-and-conquer symmetric eigenproblem, which is what NumPy calls.
    typedef void (*Fn)(char*,char*,int*,double*,int*,double*,double*,int*,int*,int*,int*,size_t,size_t);
    Fn eig=(Fn)k_lapack_symbol("dsyevd_",loc);
    int workspace_hint=0;
    eig(&vectors,&lower,&n,data,&n,result.first.items,&query,&lwork,&workspace_hint,&liwork,&info,1,1);
    k_linalg_info(info,loc);
    lwork=k_linalg_workspace(query,loc);
    if(liwork<1) liwork=3+5*n;
    double *work=k_numerical_alloc((size_t)lwork,sizeof(double),loc);
    int *iwork=k_numerical_alloc((size_t)liwork,sizeof(int),loc);
    eig(&vectors,&lower,&n,data,&n,result.first.items,work,&lwork,iwork,&liwork,&info,1,1);
    k_linalg_info(info,loc);
    k_linalg_rows(&result.second,data);free(data);free(iwork);free(work);return result;
}
static double k_linalg_cutoff(const KTensor *a,const KTensor *s,double rcond,bool automatic,KLocation loc) {
    if(!automatic&&(!isfinite(rcond)||rcond<0.0)) k_trap(loc,"rcond must be finite and nonnegative");
    double scale=automatic?(double)(a->shape[0]>a->shape[1]?a->shape[0]:a->shape[1])*DBL_EPSILON:rcond;
    return s->length?scale*s->items[0]:0.0;
}
static void k_linalg_parts_drop(KNumericalParts *parts) { k_tensor_drop(&parts->first);k_tensor_drop(&parts->second);k_tensor_drop(&parts->third); }
static int64_t k_linalg_rank(const KTensor *a,double rcond,bool automatic,KLocation loc) {
    KNumericalParts parts=k_linalg_svd(a,loc);double cutoff=k_linalg_cutoff(a,&parts.second,rcond,automatic,loc);int64_t result=0;
    for(int64_t i=0;i<parts.second.length;++i) if(parts.second.items[i]>cutoff) ++result;
    k_linalg_parts_drop(&parts);return result;
}
static double k_linalg_spectral_norm(const KTensor *a,KLocation loc) {
    KNumericalParts parts=k_linalg_svd(a,loc);double result=parts.second.length?parts.second.items[0]:0.0;k_linalg_parts_drop(&parts);return result;
}
static KTensor k_linalg_pinv(const KTensor *a,double rcond,bool automatic,KLocation loc) {
    KNumericalParts parts=k_linalg_svd(a,loc);double cutoff=k_linalg_cutoff(a,&parts.second,rcond,automatic,loc);
    int64_t m=a->shape[0],n=a->shape[1],k=parts.second.length,shape[]={n,m};KTensor result=k_tensor_new(2,shape,0.0,loc);
    for(int64_t i=0;i<n;++i) for(int64_t j=0;j<m;++j) for(int64_t p=0;p<k;++p)
        if(parts.second.items[p]>cutoff) result.items[i*m+j]+=(parts.third.items[p*n+i]/parts.second.items[p])*parts.first.items[j*k+p];
    k_linalg_parts_drop(&parts);k_linalg_finite(&result,loc);return result;
}
static KTensor k_linalg_least_squares(const KTensor *a,const KTensor *b,double rcond,bool automatic,KLocation loc) {
    k_linalg_matrix(a,false,loc);
    if((b->rank!=1&&b->rank!=2)||b->shape[0]!=a->shape[0]) k_trap(loc,"Least squares has incompatible right-hand dimensions");
    k_linalg_finite(b,loc);KTensor inverse=k_linalg_pinv(a,rcond,automatic,loc);KTensor result=k_tensor_matmul(&inverse,b,loc);k_tensor_drop(&inverse);return result;
}
static double k_math_scalar(double x,int operation) {
    switch(operation) {
        case 0:return exp(x);case 1:return expm1(x);case 2:return log(x);case 3:return log1p(x);case 4:return log2(x);case 5:return log10(x);
        case 6:return sin(x);case 7:return cos(x);case 8:return tan(x);case 9:return asin(x);case 10:return acos(x);case 11:return atan(x);
        case 12:return sinh(x);case 13:return cosh(x);case 14:return tanh(x);case 15:return sqrt(x);case 16:return fabs(x);case 17:return floor(x);case 18:return ceil(x);default:return round(x);
    }
}
static KTensor k_math_tensor(const KTensor *source,int operation,KLocation loc) {
    KTensor result=k_tensor_new_empty(source->rank,source->shape,loc);
    for(int64_t i=0;i<source->length;++i) result.items[i]=k_math_scalar(source->items[i],operation);
    return result;
}
static KTensor k_stats_reduce(const KTensor *source,int operation,int64_t axis,int64_t ddof,bool total,KLocation loc) {
    if(!total&&(axis<0||axis>=source->rank)) k_trap(loc,"Tensor axis out of range");
    int64_t count=total?source->length:source->shape[axis];
    if(!source->length||!count) k_trap(loc,"Tensor is empty");
    if(ddof<0||ddof>=count) k_trap(loc,"ddof must be nonnegative and smaller than the observation count");
    int64_t rank=total?0:source->rank-1,*shape=k_numerical_alloc((size_t)rank,sizeof(int64_t),loc),position=0,stride=1;
    if(!total) for(int64_t d=0;d<source->rank;++d) if(d!=axis) shape[position++]=source->shape[d];
    if(!total) for(int64_t d=axis+1;d<source->rank;++d) stride*=source->shape[d];
    KTensor result=k_tensor_new_empty(rank,shape,loc);free(shape);
    for(int64_t i=0;i<result.length;++i) {
        int64_t offset=total?0:(i/stride)*count*stride+i%stride;
        // Two passes: an unrolled mean, then an unrolled sum of squared
        // deviations. Welford's update divided once per element.
        double part[8]={0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0};
        int64_t j=0;
        for(;j+8<=count;j+=8) for(int t=0;t<8;++t) part[t]+=source->items[offset+(j+t)*stride];
        for(;j<count;++j) part[0]+=source->items[offset+j*stride];
        double mean=((part[0]+part[1])+(part[2]+part[3]))+((part[4]+part[5])+(part[6]+part[7]));
        mean/=(double)count;
        if(operation==0) {result.items[i]=mean;continue;}
        double squares[8]={0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0};
        j=0;
        for(;j+8<=count;j+=8) for(int t=0;t<8;++t) {double d=source->items[offset+(j+t)*stride]-mean;squares[t]+=d*d;}
        for(;j<count;++j) {double d=source->items[offset+j*stride]-mean;squares[0]+=d*d;}
        double m2=((squares[0]+squares[1])+(squares[2]+squares[3]))+((squares[4]+squares[5])+(squares[6]+squares[7]));
        double variance=m2/(double)(count-ddof);if(variance<0.0) variance=0.0;
        result.items[i]=operation==1?variance:sqrt(variance);
    }
    return result;
}
static double k_stats_scalar(KTensor value) { double result=value.items[0];k_tensor_drop(&value);return result; }
static KTensor k_stats_covariance(const KTensor *source,int64_t ddof,bool correlation,KLocation loc) {
    if(source->rank!=2) k_trap(loc,"Covariance requires a matrix with observations in rows");
    int64_t rows=source->shape[0],columns=source->shape[1];
    if(ddof<0||ddof>=rows) k_trap(loc,"ddof must be nonnegative and smaller than the observation count");
    int64_t shape[]={columns,columns};KTensor result=k_tensor_new(2,shape,0.0,loc);
    double *means=k_numerical_alloc((size_t)columns,sizeof(double),loc);
    for(int64_t row=0;row<rows;++row) for(int64_t col=0;col<columns;++col) means[col]=means[col]*((double)row/(double)(row+1))+source->items[row*columns+col]/(double)(row+1);
    for(int64_t i=0;i<columns;++i) for(int64_t j=0;j<=i;++j) {
        double value=0.0;for(int64_t row=0;row<rows;++row) value+=(source->items[row*columns+i]-means[i])*(source->items[row*columns+j]-means[j]);
        result.items[i*columns+j]=result.items[j*columns+i]=value/(double)(rows-ddof);
    }
    if(correlation) {
        for(int64_t i=0;i<columns;++i) {means[i]=sqrt(result.items[i*columns+i]);if(!(means[i]>0.0)||!isfinite(means[i])) k_trap(loc,"Correlation requires finite, nonzero column variances");}
        for(int64_t i=0;i<columns;++i) for(int64_t j=0;j<columns;++j) {double v=result.items[i*columns+j]/means[i]/means[j];result.items[i*columns+j]=i==j?1.0:fmax(-1.0,fmin(1.0,v));}
    }
    free(means);return result;
}
// PCG-XSH-RR 64/32, using a fixed odd stream increment. State is copied as bits into Int storage.
static uint32_t k_rng_step(int64_t *state) {
    uint64_t old;memcpy(&old,state,sizeof old);uint64_t next=old*UINT64_C(6364136223846793005)+UINT64_C(1442695040888963407);memcpy(state,&next,sizeof next);
    uint32_t mixed=(uint32_t)(((old>>18u)^old)>>27u),rotation=(uint32_t)(old>>59u);
    return (mixed>>rotation)|(mixed<<((-rotation)&31u));
}
static int64_t k_rng_seed(int64_t seed) { int64_t state=0;k_rng_step(&state);uint64_t bits;memcpy(&bits,&state,sizeof bits);bits+=(uint64_t)seed;memcpy(&state,&bits,sizeof bits);k_rng_step(&state);return state; }
static uint64_t k_rng_u64(int64_t *state) {uint64_t high=k_rng_step(state);return(high<<32)|k_rng_step(state);}
static double k_rng_unit(int64_t *state) {return(double)(k_rng_u64(state)>>11)*0x1.0p-53;}
static double k_rng_uniform(int64_t *state,double low,double high,KLocation loc) {
    if(!isfinite(low)||!isfinite(high)||!(high>low)) k_trap(loc,"Uniform requires finite bounds with low < high");
    double u=k_rng_unit(state),result=low*(1.0-u)+high*u;
    return result<high?result:nextafter(high,low);
}
static double k_rng_normal(int64_t *state,double mean,double sigma,KLocation loc) {
    if(!isfinite(mean)||!isfinite(sigma)||!(sigma>0.0)) k_trap(loc,"Normal requires a finite mean and positive finite sigma");
    double radius=sqrt(-2.0*log1p(-k_rng_unit(state))),angle=6.283185307179586476925286766559*k_rng_unit(state);
    double result=fma(sigma,radius*cos(angle),mean);
    if(!isfinite(result)) k_trap(loc,"Normal sample is not finite");
    return result;
}
static int64_t k_rng_integers(int64_t *state,int64_t low,int64_t high,KLocation loc) {
    if(high<=low) k_trap(loc,"Random integer bounds require low < high");
    uint64_t width=(uint64_t)high-(uint64_t)low,threshold=(UINT64_C(0)-width)%width,value;
    do {value=k_rng_u64(state);} while(value<threshold);
    uint64_t bits=(uint64_t)low+value%width;int64_t result;memcpy(&result,&bits,sizeof result);return result;
}

// k-nearest-neighbour prediction for std.ml. Each test row scans the training
// rows directly, keeps the k smallest squared distances in sorted arrays, and
// votes with the same first-past-the-post rule as the Kestrel implementation.
// The scan runs on several threads; the workload is a few hundred million
// fused multiply-adds, which one core cannot hide.
typedef struct {
    const double *training;
    const double *labels;
    const double *test;
    const double *trainNorms;
    double *result;
    int64_t trainRows, features, neighbors;
    int64_t totalRows;
    _Atomic int64_t *next;
} KKnnJob;

static inline void k_knn_offer(double distance, double label,
        double *distances, double *chosen, int64_t *count, int64_t k) {
    if (*count < k) {
        int64_t position = *count;
        while (position > 0 && distances[position - 1] > distance) --position;
        for (int64_t move = *count; move > position; --move) {
            distances[move] = distances[move - 1];
            chosen[move] = chosen[move - 1];
        }
        distances[position] = distance;
        chosen[position] = label;
        ++*count;
    } else if (distance < distances[k - 1]) {
        int64_t position = k - 1;
        while (position > 0 && distances[position - 1] > distance) --position;
        for (int64_t move = k - 1; move > position; --move) {
            distances[move] = distances[move - 1];
            chosen[move] = chosen[move - 1];
        }
        distances[position] = distance;
        chosen[position] = label;
    }
}

static void k_knn_run(KKnnJob *job) {
    int64_t k = job->neighbors, f = job->features, trainRows = job->trainRows;
    const int64_t chunk = 8;
    double *distances = malloc((size_t)k * sizeof(double));
    double *chosen = malloc((size_t)k * sizeof(double));
    double *dots = malloc((size_t)(chunk * trainRows) * sizeof(double));
    if (!distances || !chosen) k_fatal("Out of memory");
    for (;;) {
        // Dynamic chunks keep the performance cores from waiting on the
        // efficiency cores at the end of a fixed split.
        int64_t row = atomic_fetch_add(job->next, chunk);
        if (row >= job->totalRows) break;
        int64_t rows = job->totalRows - row < chunk ? job->totalRows - row : chunk;
        bool used = dots && k_blas_dgemm(rows, f, trainRows, job->test + row * f,
                false, job->training, true, dots);
        for (int64_t within = 0; within < rows; ++within, ++row) {
            const double *point = job->test + row * f;
            double rowNorm = 0.0;
            for (int64_t column = 0; column < f; ++column) rowNorm += point[column] * point[column];
            int64_t count = 0;
            if (used) {
                const double *scores = dots + within * trainRows;
                int64_t train = 0;
                if (k < trainRows)
                    for (; train < k; ++train) {
                        double distance = job->trainNorms[train] + rowNorm - 2.0 * scores[train];
                        k_knn_offer(distance, job->labels[train], distances, chosen, &count, k);
                    }
                for (; train + 4 <= trainRows; train += 4) {
                    double d0 = job->trainNorms[train] + rowNorm - 2.0 * scores[train];
                    double d1 = job->trainNorms[train + 1] + rowNorm - 2.0 * scores[train + 1];
                    double d2 = job->trainNorms[train + 2] + rowNorm - 2.0 * scores[train + 2];
                    double d3 = job->trainNorms[train + 3] + rowNorm - 2.0 * scores[train + 3];
                    if (fmin(fmin(d0, d1), fmin(d2, d3)) < distances[k - 1]) {
                        k_knn_offer(d0, job->labels[train], distances, chosen, &count, k);
                        k_knn_offer(d1, job->labels[train + 1], distances, chosen, &count, k);
                        k_knn_offer(d2, job->labels[train + 2], distances, chosen, &count, k);
                        k_knn_offer(d3, job->labels[train + 3], distances, chosen, &count, k);
                    }
                }
                for (; train < trainRows; ++train) {
                    double distance = job->trainNorms[train] + rowNorm - 2.0 * scores[train];
                    k_knn_offer(distance, job->labels[train], distances, chosen, &count, k);
                }
            } else {
                for (int64_t train = 0; train < trainRows; ++train) {
                    const double *other = job->training + train * f;
                    double distance = 0.0;
                    for (int64_t column = 0; column < f; ++column) {
                        double difference = point[column] - other[column];
                        distance += difference * difference;
                    }
                    k_knn_offer(distance, job->labels[train], distances, chosen, &count, k);
                }
            }
            if (!count) {
                job->result[row] = 0.0;
                continue;
            }
            double best = chosen[0];
            int64_t bestCount = 0;
            for (int64_t candidate = 0; candidate < count; ++candidate) {
                int64_t votes = 0;
                for (int64_t other = 0; other < count; ++other)
                    if (chosen[candidate] == chosen[other]) ++votes;
                if (votes > bestCount) {
                    bestCount = votes;
                    best = chosen[candidate];
                }
            }
            job->result[row] = best;
        }
    }
    free(dots);
    free(distances);
    free(chosen);
}

static void *k_knn_thread(void *argument) {
    k_knn_run((KKnnJob *)argument);
    return NULL;
}

static KTensor k_ml_knn_predict(const KTensor *training, const KTensor *labels,
        const KTensor *test, int64_t neighbors, KLocation loc) {
    if (training->rank != 2 || test->rank != 2)
        k_trap(loc, "knnPredict needs rank-2 training and test tensors");
    int64_t trainRows = training->shape[0], features = training->shape[1];
    int64_t testRows = test->shape[0];
    if (test->shape[1] != features)
        k_trap(loc, "knnPredict needs the same number of features in both tensors");
    if (labels->rank != 1 || labels->shape[0] != trainRows)
        k_trap(loc, "knnPredict needs one label per training row");
    if (neighbors < 1) neighbors = 1;
    if (neighbors > trainRows) neighbors = trainRows;
    int64_t shape[] = {testRows};
    KTensor result = k_tensor_new(1, shape, 0.0, loc);
    if (!testRows || !trainRows) return result;
    int64_t cpus = sysconf(_SC_NPROCESSORS_ONLN);
    int64_t threads = cpus > 0 ? cpus : 1;
    if (threads > 12) threads = 12;
    if (threads > testRows) threads = testRows;
    double *trainNorms = malloc((size_t)trainRows * sizeof(double));
    KKnnJob *jobs = calloc((size_t)threads, sizeof(KKnnJob));
    pthread_t *handles = calloc((size_t)threads, sizeof(pthread_t));
    if (!trainNorms || !jobs || !handles) k_fatal("Out of memory");
    for (int64_t row = 0; row < trainRows; ++row) {
        const double *point = training->items + row * features;
        double norm = 0.0;
        for (int64_t column = 0; column < features; ++column) norm += point[column] * point[column];
        trainNorms[row] = norm;
    }
    _Atomic int64_t next = 0;
    int64_t started = 0;
    for (int64_t index = 0; index < threads; ++index) {
        jobs[index] = (KKnnJob){
            .training = training->items, .labels = labels->items,
            .test = test->items, .trainNorms = trainNorms, .result = result.items,
            .trainRows = trainRows, .features = features, .neighbors = neighbors,
            .totalRows = testRows, .next = &next,
        };
    }
    // The calling thread is one worker; the pool only adds helpers.
    for (int64_t index = 1; index < threads; ++index)
        if (pthread_create(&handles[started], NULL, k_knn_thread, &jobs[index]) == 0) ++started;
    k_knn_run(&jobs[0]);
    for (int64_t index = 0; index < started; ++index) pthread_join(handles[index], NULL);
    free(handles);
    free(jobs);
    free(trainNorms);
    return result;
}
