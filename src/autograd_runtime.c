// KesNet differentiable values.
//
// A KVariable is one counted handle to a KAdNode. Nodes hold row-major double
// storage plus an optional gradient. Operations keep strong references to their
// parents, so dropping a result frees the graph fragment it owns; parameters
// stay alive through the module that holds them. Gradient buffers belong to the
// leaf nodes until zero_grad clears them.

typedef enum {
    K_AD_F64 = 0,
    K_AD_F32 = 1,
    K_AD_F16 = 2,
    K_AD_BF16 = 3
} KAdDtype;

typedef enum {
    K_AD_CPU = 0,
    K_AD_MPS = 1,
    K_AD_CUDA = 2
} KAdDevice;

typedef enum {
    K_AD_OP_NONE = 0,
    K_AD_OP_ADD,
    K_AD_OP_SUB,
    K_AD_OP_MUL,
    K_AD_OP_DIV,
    K_AD_OP_NEG,
    K_AD_OP_MATMUL,
    K_AD_OP_SUM,
    K_AD_OP_MEAN,
    K_AD_OP_PROD,
    K_AD_OP_MAX,
    K_AD_OP_MIN,
    K_AD_OP_RESHAPE,
    K_AD_OP_TRANSPOSE,
    K_AD_OP_BROADCAST,
    K_AD_OP_RELU,
    K_AD_OP_SIGMOID,
    K_AD_OP_TANH,
    K_AD_OP_EXP,
    K_AD_OP_LOG,
    K_AD_OP_SQRT,
    K_AD_OP_ABS,
    K_AD_OP_POW,
    K_AD_OP_CLAMP,
    K_AD_OP_SOFTMAX,
    K_AD_OP_LOGSOFTMAX,
    K_AD_OP_LEAKY_RELU,
    K_AD_OP_ELU,
    K_AD_OP_GELU,
    K_AD_OP_SILU,
    K_AD_OP_SOFTPLUS,
    K_AD_OP_INDEX,
    K_AD_OP_CLONE,
    K_AD_OP_CAST,
    K_AD_OP_CROSS_ENTROPY,
    K_AD_OP_NLL,
    K_AD_OP_BCE,
    K_AD_OP_CONV2D,
    K_AD_OP_MAXPOOL2D,
    K_AD_OP_AVGPOOL2D,
    K_AD_OP_BATCHNORM,
    K_AD_OP_LAYERNORM,
    K_AD_OP_EMBEDDING,
    K_AD_OP_BMM,
    K_AD_OP_CAT,
    K_AD_OP_SLICE,
    K_AD_OP_ADAPTIVE_AVGPOOL2D,
    K_AD_OP_NORM,
    K_AD_OP_LSTM_CELL,
    K_AD_OP_LSTM_HIDDEN,
    K_AD_OP_LSTM_SEQUENCE
} KAdOpKind;

typedef struct KAdNode KAdNode;

static void k_ad_adaptive_avg_pool2d_grad(KAdNode *a, const double *g, int64_t out_h, int64_t out_w, KLocation loc);
static void k_ad_im2col(const double *x, int64_t channel, int64_t h, int64_t w, int64_t cpg, int64_t kh, int64_t kw, int64_t out_h, int64_t out_w, int64_t stride, int64_t padding, int64_t dilation, double *col);

typedef struct KAdOp {
    KAdOpKind kind;
    KAdNode *a;
    KAdNode *b;
    KAdNode *c;
    KAdNode *d;
    KAdNode *e;
    KTensor saved_a;
    KTensor saved_b;
    KAdNode *keep_a;
    KAdNode *keep_b;
    int64_t stride_h, stride_w, pad_h, pad_w, dilation_h, dilation_w, groups;
    double *xhat;
    double *invstd;
    int64_t normalized;
    int64_t axis;
    bool keepdim;
    int64_t *axes;
    int64_t rank;
    double scalar;
    double scalar2;
    bool flag;
    int64_t *indices;
    int64_t index_rank;
    KAdDtype cast_to;
} KAdOp;

struct KAdNode {
    _Atomic int64_t references;
    bool requires_grad;
    bool is_leaf;
    bool retain_grad;
    bool grad_enabled;
    KAdDtype dtype;
    KAdDevice device;
    int64_t rank;
    int64_t length;
    int64_t *shape;
    double *data;
    double *grad;
    KAdOp *op;
    int64_t version;
    int64_t id;
};

typedef struct { KAdNode *node; } KVariable;

static _Atomic int64_t k_ad_next_id = 1;
static _Atomic int64_t k_ad_live_nodes = 0;
static _Atomic int64_t k_ad_live_bytes = 0;
static bool k_ad_grad_enabled = true;
static uint64_t k_ad_rng_state = UINT64_C(0x9e3779b97f4a7c15);

static void k_ad_trap(KLocation loc, const char *message) { k_trap(loc, message); }

static double k_ad_round_dtype(double value, KAdDtype dtype) {
    if (dtype == K_AD_F32) return (double)(float)value;
    if (dtype == K_AD_F16) {
        if (!isfinite(value) || value == 0.0) return value;
        float source = (float)value;
        uint32_t bits;
        memcpy(&bits, &source, sizeof bits);
        uint32_t sign = bits & 0x80000000u;
        int32_t exponent = (int32_t)((bits >> 23) & 0xFFu) - 127;
        uint32_t mantissa = bits & 0x7FFFFFu;
        int32_t halfExponent = exponent + 15;
        if (halfExponent >= 31) return sign ? -INFINITY : INFINITY;
        if (halfExponent <= 0) {
            // Subnormal half: the implicit bit joins the mantissa and the
            // leading bits are shifted out with round-to-nearest-even.
            if (halfExponent < -10) return sign ? -0.0 : 0.0;
            uint32_t full = mantissa | 0x800000u;
            int shift = 14 - halfExponent;
            uint32_t half = full >> shift;
            uint32_t remainder = full & ((1u << shift) - 1u);
            uint32_t halfway = 1u << (shift - 1);
            if (remainder > halfway || (remainder == halfway && (half & 1u))) half += 1;
            double result = ldexp((double)half, -24);
            return sign ? -result : result;
        }
        uint32_t half = mantissa >> 13;
        uint32_t remainder = mantissa & 0x1FFFu;
        if (remainder > 0x1000u || (remainder == 0x1000u && (half & 1u))) {
            half += 1;
            if (half == 0x400u) {
                half = 0;
                halfExponent += 1;
                if (halfExponent >= 31) return sign ? -INFINITY : INFINITY;
            }
        }
        double result = ldexp((double)(0x400u | half), halfExponent - 25);
        return sign ? -result : result;
    }
    if (dtype == K_AD_BF16) {
        float rounded = (float)value;
        uint32_t bits;
        memcpy(&bits, &rounded, sizeof bits);
        bits = (bits + 0x8000u) & 0xFFFF0000u;
        float bfloat;
        memcpy(&bfloat, &bits, sizeof bfloat);
        return (double)bfloat;
    }
    return value;
}

static uint64_t k_ad_rng_next(void) {
    uint64_t x = k_ad_rng_state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    k_ad_rng_state = x;
    return x * UINT64_C(2685821657736338717);
}

static double k_ad_uniform01(void) {
    return (double)(k_ad_rng_next() >> 11) * (1.0 / 9007199254740992.0);
}

static double k_ad_normal_sample(void) {
    // Box-Muller. Guard the logarithm so a zero draw is still finite.
    double u1 = k_ad_uniform01();
    double u2 = k_ad_uniform01();
    if (u1 < 1.0e-300) u1 = 1.0e-300;
    return sqrt(-2.0 * log(u1)) * cos(6.28318530717958647692 * u2);
}

static int64_t k_ad_length_for(int64_t rank, const int64_t *shape, KLocation loc) {
    int64_t length = 1;
    if (rank < 0) k_ad_trap(loc, "Variable rank is negative");
    for (int64_t d = 0; d < rank; ++d) {
        if (shape[d] < 0) k_ad_trap(loc, "Variable dimensions cannot be negative");
        if (shape[d] == 0) {
            length = 0;
        } else if (length > 0) {
            if (length > INT64_MAX / shape[d]) k_ad_trap(loc, "Variable shape is too large");
            length *= shape[d];
        }
    }
    if (rank == 0) length = 1;
    return length;
}

static KAdNode *k_ad_node_new(int64_t rank, const int64_t *shape, double fill, KAdDtype dtype, bool requires_grad, KLocation loc) {
    KAdNode *node = calloc(1, sizeof *node);
    if (!node) k_fatal("Out of memory");
    node->references = 1;
    node->requires_grad = requires_grad;
    node->is_leaf = true;
    node->grad_enabled = requires_grad;
    node->dtype = dtype;
    node->device = K_AD_CPU;
    node->rank = rank;
    node->length = k_ad_length_for(rank, shape, loc);
    node->shape = malloc((size_t)(rank ? rank : 1) * sizeof(int64_t));
    if (!node->shape) k_fatal("Out of memory");
    if (rank) memcpy(node->shape, shape, (size_t)rank * sizeof(int64_t));
    node->data = malloc((size_t)(node->length ? node->length : 1) * sizeof(double));
    if (!node->data) k_fatal("Out of memory");
    for (int64_t i = 0; i < node->length; ++i) node->data[i] = k_ad_round_dtype(fill, dtype);
    node->id = atomic_fetch_add_explicit(&k_ad_next_id, 1, memory_order_relaxed);
    atomic_fetch_add_explicit(&k_ad_live_nodes, 1, memory_order_relaxed);
    atomic_fetch_add_explicit(&k_ad_live_bytes, (long long)(sizeof(KAdNode) + (rank ? rank : 1) * (int64_t)sizeof(int64_t) + (node->length ? node->length : 1) * (int64_t)sizeof(double)), memory_order_relaxed);
    return node;
}

static KAdOp *k_ad_op_new(KAdOpKind kind, KAdNode *a, KAdNode *b) {
    KAdOp *op = calloc(1, sizeof *op);
    if (!op) k_fatal("Out of memory");
    op->kind = kind;
    op->a = a;
    op->b = b;
    if (a) atomic_fetch_add_explicit(&a->references, 1, memory_order_relaxed);
    if (b) atomic_fetch_add_explicit(&b->references, 1, memory_order_relaxed);
    return op;
}

static void k_ad_tensor_drop(KTensor *value) { if (value) k_tensor_drop(value); }

static void k_ad_node_release(KAdNode *node);

static void k_ad_node_release(KAdNode *node);
// Saved tensors borrow the input's storage instead of copying it. The op and
// keep references hold the input node, so the storage outlives the product.
static KTensor k_ad_saved_view(const KAdNode *node) {
    KTensor view = {0};
    if (!node) return view;
    view.rank = node->rank;
    view.length = node->length;
    view.shape = node->shape;
    view.items = node->data;
    view.borrowed = true;
    return view;
}
static void k_ad_op_save(KAdOp *op, bool second, KAdNode *node) {
    if (!op || !node) return;
    atomic_fetch_add_explicit(&node->references, 1, memory_order_relaxed);
    if (second) { op->saved_b = k_ad_saved_view(node); op->keep_b = node; }
    else { op->saved_a = k_ad_saved_view(node); op->keep_a = node; }
}
static void k_ad_op_drop(KAdOp *op) {
    if (!op) return;
    if (op->a) k_ad_node_release(op->a);
    if (op->b) k_ad_node_release(op->b);
    if (op->c) k_ad_node_release(op->c);
    if (op->d) k_ad_node_release(op->d);
    if (op->e) k_ad_node_release(op->e);
    if (op->keep_a) k_ad_node_release(op->keep_a);
    if (op->keep_b) k_ad_node_release(op->keep_b);
    k_ad_tensor_drop(&op->saved_a);
    k_ad_tensor_drop(&op->saved_b);
    free(op->axes);
    free(op->indices);
    free(op->xhat);
    free(op->invstd);
    free(op);
}

static void k_ad_node_release(KAdNode *node) {
    if (!node) return;
    if (atomic_fetch_sub_explicit(&node->references, 1, memory_order_acq_rel) != 1) return;
    int64_t bytes = (long long)(sizeof(KAdNode) + (node->rank ? node->rank : 1) * (int64_t)sizeof(int64_t) + (node->length ? node->length : 1) * (int64_t)sizeof(double));
    free(node->shape);
    free(node->data);
    if (node->grad) free(node->grad);
    k_ad_op_drop(node->op);
    atomic_fetch_sub_explicit(&k_ad_live_nodes, 1, memory_order_relaxed);
    atomic_fetch_sub_explicit(&k_ad_live_bytes, (long long)bytes + (node->grad ? (long long)node->length * (long long)sizeof(double) : 0), memory_order_relaxed);
    free(node);
}

static KAdNode *k_ad_retain_node(KAdNode *node) {
    if (node) atomic_fetch_add_explicit(&node->references, 1, memory_order_relaxed);
    return node;
}

static KVariable k_ad_handle(KAdNode *node) { KVariable v = {node}; return v; }

static KVariable k_variable_copy(KVariable value) {
    return k_ad_handle(k_ad_retain_node(value.node));
}

static KVariable k_variable_empty(void) { return k_ad_handle(NULL); }

static void k_variable_drop(KVariable *value) {
    if (!value) return;
    k_ad_node_release(value->node);
    value->node = NULL;
}

static KVariable k_variable_take(KVariable *value) {
    KVariable result = *value;
    value->node = NULL;
    return result;
}

static void k_variable_set(KVariable *slot, KVariable value) {
    if (!slot) { k_variable_drop(&value); return; }
    KVariable previous = *slot;
    *slot = value;
    k_variable_drop(&previous);
}



static bool k_ad_same_shape(const KAdNode *a, const KAdNode *b) {
    if (a->rank != b->rank) return false;
    for (int64_t d = 0; d < a->rank; ++d) if (a->shape[d] != b->shape[d]) return false;
    return true;
}

static int64_t k_ad_broadcast_rank(const KAdNode *a, const KAdNode *b) {
    return a->rank > b->rank ? a->rank : b->rank;
}

static void k_ad_broadcast_shape(const KAdNode *a, const KAdNode *b, int64_t rank, int64_t *shape, KLocation loc) {
    for (int64_t d = 0; d < rank; ++d) {
        int64_t ai = a->rank - (rank - d);
        int64_t bi = b->rank - (rank - d);
        int64_t ad = ai >= 0 ? a->shape[ai] : 1;
        int64_t bd = bi >= 0 ? b->shape[bi] : 1;
        if (ad != bd && ad != 1 && bd != 1) k_ad_trap(loc, "Variable shapes cannot broadcast");
        shape[d] = ad == 1 ? bd : ad;
    }
}

static int64_t k_ad_broadcast_offset(const KAdNode *node, int64_t out_rank, const int64_t *out_shape, int64_t flat) {
    int64_t offset = 0;
    int64_t stride = 1;
    for (int64_t d = out_rank; d-- > 0;) {
        int64_t index = flat % out_shape[d];
        flat /= out_shape[d];
        int64_t source = d - (out_rank - node->rank);
        if (source >= 0) {
            if (node->shape[source] != 1) offset += index * stride;
            stride *= node->shape[source];
        }
    }
    return offset;
}

static double k_ad_read(const KAdNode *node, int64_t offset) { return node->data[offset]; }

static double k_ad_scalar(const KAdNode *node) {
    if (node->length != 1) k_ad_trap((KLocation){0}, "Variable is not a scalar");
    return node->data[0];
}

static void k_ad_grad_reset(KAdNode *node) {
    if (node->grad) {
        atomic_fetch_sub_explicit(&k_ad_live_bytes, (long long)node->length * (long long)sizeof(double), memory_order_relaxed);
        free(node->grad);
        node->grad = NULL;
    }
}

static double *k_ad_grad_buffer(KAdNode *node) {
    if (!node->grad) {
        node->grad = calloc((size_t)(node->length ? node->length : 1), sizeof(double));
        if (!node->grad) k_fatal("Out of memory");
        atomic_fetch_add_explicit(&k_ad_live_bytes, (long long)node->length * (long long)sizeof(double), memory_order_relaxed);
    }
    return node->grad;
}

// Reduce a broadcast-shaped gradient to the shape of `node` and add it.
static int64_t k_ad_index_into(const KAdNode *target, int64_t out_rank, const int64_t *out_shape, int64_t flat);

static int64_t k_ad_shape_length(int64_t rank, const int64_t *shape) {
    int64_t length = 1;
    for (int64_t d = 0; d < rank; ++d) length *= shape[d];
    return length;
}

static void k_ad_accumulate(KAdNode *node, int64_t rank, const int64_t *shape, const double *values) {
    if (!node || !node->requires_grad) return;
    double *target = k_ad_grad_buffer(node);
    bool same = rank == node->rank;
    if (same) for (int64_t d = 0; d < rank; ++d) if (shape[d] != node->shape[d]) { same = false; break; }
    if (same) {
        for (int64_t i = 0; i < node->length; ++i) target[i] += values[i];
        return;
    }
    int64_t total = k_ad_shape_length(rank, shape);
    for (int64_t flat = 0; flat < total; ++flat) {
        int64_t target_index = k_ad_index_into(node, rank, shape, flat);
        target[target_index] += values[flat];
    }
}

static KVariable k_ad_output(int64_t rank, const int64_t *shape, double fill, KAdDtype dtype, KLocation loc) {
    KAdNode *node = k_ad_node_new(rank, shape, fill, dtype, false, loc);
    node->is_leaf = true;
    return k_ad_handle(node);
}

static void k_ad_set_op(KVariable result, KAdOpKind kind, KVariable a, KVariable b) {
    KAdNode *out = result.node;
    KAdNode *left = a.node;
    KAdNode *right = b.node;
    out->is_leaf = false;
    out->requires_grad = false;
    if (k_ad_grad_enabled) {
        out->requires_grad = (left && left->requires_grad) || (right && right->requires_grad);
        out->grad_enabled = out->requires_grad;
    }
    if (out->requires_grad) {
        out->op = k_ad_op_new(kind, left, right);
    }
}

// ---------------------------------------------------------------- factories

static KVariable k_ad_zeros(int64_t rank, const int64_t *shape, double fill, bool requires_grad, KAdDtype dtype, KLocation loc) {
    KAdNode *node = k_ad_node_new(rank, shape, fill, dtype, requires_grad, loc);
    node->is_leaf = true;
    return k_ad_handle(node);
}

static KVariable k_ad_arange(double start, double stop, double step, bool requires_grad, KAdDtype dtype, KLocation loc) {
    if (step == 0.0) k_ad_trap(loc, "Variable.arange step cannot be zero");
    double count = ceil((stop - start) / step);
    int64_t length = count > 0.0 ? (int64_t)count : 0;
    int64_t shape[1] = {length};
    KVariable result = k_ad_zeros(1, shape, 0.0, requires_grad, dtype, loc);
    for (int64_t i = 0; i < length; ++i) result.node->data[i] = k_ad_round_dtype(start + (double)i * step, dtype);
    return result;
}

static KVariable k_ad_linspace(double start, double stop, int64_t steps, bool requires_grad, KAdDtype dtype, KLocation loc) {
    if (steps < 0) k_ad_trap(loc, "Variable.linspace steps cannot be negative");
    int64_t shape[1] = {steps};
    KVariable result = k_ad_zeros(1, shape, 0.0, requires_grad, dtype, loc);
    for (int64_t i = 0; i < steps; ++i) {
        double value = steps <= 1 ? start : start + (stop - start) * (double)i / (double)(steps - 1);
        result.node->data[i] = k_ad_round_dtype(value, dtype);
    }
    return result;
}

static KVariable k_ad_from_values(int64_t length, const double *values, int64_t rank, const int64_t *shape, bool requires_grad, KAdDtype dtype, KLocation loc) {
    int64_t count = k_ad_length_for(rank, shape, loc);
    if (count != length) k_ad_trap(loc, "Variable values and shape have different element counts");
    KVariable result = k_ad_zeros(rank, shape, 0.0, requires_grad, dtype, loc);
    for (int64_t i = 0; i < length; ++i) result.node->data[i] = k_ad_round_dtype(values[i], dtype);
    return result;
}

static KVariable k_ad_from_tensor(const KTensor *source, bool requires_grad, KAdDtype dtype, KLocation loc) {
    return k_ad_from_values(source->length, source->items, source->rank, source->shape, requires_grad, dtype, loc);
}

static KVariable k_ad_randn(int64_t rank, const int64_t *shape, bool requires_grad, KAdDtype dtype, KLocation loc) {
    KVariable result = k_ad_zeros(rank, shape, 0.0, requires_grad, dtype, loc);
    for (int64_t i = 0; i < result.node->length; ++i) result.node->data[i] = k_ad_round_dtype(k_ad_normal_sample(), dtype);
    return result;
}

static KVariable k_ad_uniform(int64_t rank, const int64_t *shape, double low, double high, bool requires_grad, KAdDtype dtype, KLocation loc) {
    KVariable result = k_ad_zeros(rank, shape, 0.0, requires_grad, dtype, loc);
    for (int64_t i = 0; i < result.node->length; ++i) result.node->data[i] = k_ad_round_dtype(low + (high - low) * k_ad_uniform01(), dtype);
    return result;
}

// ---------------------------------------------------------------- metadata

static KTensor k_ad_tensor_view(KAdNode *node) {
    KTensor view;
    view.rank = node->rank;
    view.length = node->length;
    view.shape = node->shape;
    view.items = node->data;
    view.borrowed = true;
    view.transposed = false;
    return view;
}

static KTensor k_ad_tensor_copy_node(KAdNode *node, KLocation loc) {
    KTensor view = k_ad_tensor_view(node);
    return k_tensor_copy(&view, loc);
}

static KString k_ad_tensor_text_node(KAdNode *node, KLocation loc) {
    KTensor view = k_ad_tensor_view(node);
    return k_tensor_text(&view, loc);
}

static KTensor k_ad_data(KVariable value, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable is empty");
    return k_ad_tensor_copy_node(value.node, loc);
}

static KTensor k_ad_grad(KVariable value, KLocation loc) {
    if (!value.node || !value.node->grad) return (KTensor){0};
    KTensor view;
    view.rank = value.node->rank;
    view.length = value.node->length;
    view.shape = value.node->shape;
    view.items = value.node->grad;
    view.borrowed = true;
    view.transposed = false;
    return k_tensor_copy(&view, loc);
}

static int64_t k_ad_rank(KVariable value) { return value.node ? value.node->rank : 0; }
static int64_t k_ad_count(KVariable value) { return value.node ? value.node->length : 0; }
static double k_ad_item(KVariable value, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable is empty");
    if (value.node->length != 1) k_ad_trap(loc, "Variable.item needs exactly one element; use data() for a whole tensor");
    return value.node->data[0];
}
static bool k_ad_requires_grad(KVariable value) { return value.node && value.node->requires_grad; }
static bool k_ad_is_leaf(KVariable value) { return value.node && value.node->is_leaf; }
static int64_t k_ad_version(KVariable value) { return value.node ? value.node->version : 0; }

static const char *k_ad_dtype_name(KAdDtype dtype) {
    switch (dtype) {
        case K_AD_F32: return "float32";
        case K_AD_F16: return "float16";
        case K_AD_BF16: return "bfloat16";
        default: return "float64";
    }
}

static KString k_ad_dtype_text(KVariable value) {
    return k_string_bytes(k_ad_dtype_name(value.node ? value.node->dtype : K_AD_F64), strlen(k_ad_dtype_name(value.node ? value.node->dtype : K_AD_F64)));
}

static KString k_ad_device_text(KVariable value) {
    (void)value;
    return K_TEXT("cpu");
}

static KString k_ad_text(KVariable value, KLocation loc) {
    if (!value.node) return K_TEXT("Variable[]");
    KTensor view = k_ad_tensor_view(value.node);
    return k_tensor_text(&view, loc);
}

static KString k_ad_debug(KVariable value, KLocation loc) {
    if (!value.node) return K_TEXT("Variable[empty]");
    KString text = k_ad_tensor_text_node(value.node, loc);
    // Prefix the value text with its dtype and shape.
    KString pieces[9];
    size_t count = 0;
    pieces[count++] = K_TEXT("Variable[");
    const char *dtype = k_ad_dtype_name(value.node->dtype);
    pieces[count++] = k_string_bytes(dtype, strlen(dtype));
    pieces[count++] = K_TEXT(", (");
    for (int64_t d = 0; d < value.node->rank; ++d) {
        if (d) pieces[count++] = K_TEXT(", ");
        char number[32];
        int length = snprintf(number, sizeof number, "%lld", (long long)value.node->shape[d]);
        pieces[count++] = k_string_bytes(number, (size_t)length);
    }
    pieces[count++] = K_TEXT(")] ");
    pieces[count++] = text;
    KString result = k_string_join(count, pieces);
    k_string_drop(&text);
    return result;
}

// ---------------------------------------------------------------- broadcasting helpers

typedef struct {
    int64_t rank;
    int64_t length;
    int64_t *shape;
    int64_t *a_offset;
    int64_t *b_offset;
} KAdBroadcast;

static KAdBroadcast k_ad_broadcast_plan(const KAdNode *a, const KAdNode *b, KLocation loc) {
    KAdBroadcast plan;
    plan.rank = k_ad_broadcast_rank(a, b);
    if (plan.rank > 64) k_ad_trap(loc, "Variable rank exceeds 64");
    plan.shape = malloc((size_t)(plan.rank ? plan.rank : 1) * sizeof(int64_t));
    plan.a_offset = malloc((size_t)(plan.rank ? plan.rank : 1) * sizeof(int64_t));
    plan.b_offset = malloc((size_t)(plan.rank ? plan.rank : 1) * sizeof(int64_t));
    if (!plan.shape || !plan.a_offset || !plan.b_offset) k_fatal("Out of memory");
    k_ad_broadcast_shape(a, b, plan.rank, plan.shape, loc);
    plan.length = 1;
    for (int64_t d = 0; d < plan.rank; ++d) plan.length *= plan.shape[d];
    // Offsets count elements between neighbours along each output dimension.
    for (int64_t d = 0; d < plan.rank; ++d) {
        int64_t ai = a->rank - (plan.rank - d);
        int64_t bi = b->rank - (plan.rank - d);
        int64_t a_stride = 1, b_stride = 1;
        for (int64_t e = d + 1; e < plan.rank; ++e) {
            int64_t ae = a->rank - (plan.rank - e);
            int64_t be = b->rank - (plan.rank - e);
            if (ae >= 0) a_stride *= a->shape[ae];
            if (be >= 0) b_stride *= b->shape[be];
        }
        plan.a_offset[d] = ai >= 0 && a->shape[ai] != 1 ? a_stride : 0;
        plan.b_offset[d] = bi >= 0 && b->shape[bi] != 1 ? b_stride : 0;
    }
    return plan;
}

static void k_ad_broadcast_free(KAdBroadcast *plan) {
    free(plan->shape);
    free(plan->a_offset);
    free(plan->b_offset);
}

static void k_ad_binary_loop(KAdNode *out, const KAdBroadcast *plan, const KAdNode *a, const KAdNode *b, double op, KLocation loc) {
    (void)loc;
    int64_t a_index = 0, b_index = 0;
    int64_t coordinates[64];
    for (int64_t flat = 0; flat < plan->length; ++flat) {
        int64_t rest = flat;
        a_index = 0;
        b_index = 0;
        for (int64_t d = plan->rank; d-- > 0;) {
            coordinates[d] = rest % plan->shape[d];
            rest /= plan->shape[d];
            a_index += coordinates[d] * plan->a_offset[d];
            b_index += coordinates[d] * plan->b_offset[d];
        }
        double av = a->data[a_index];
        double bv = b->data[b_index];
        double value;
        switch ((int)op) {
            case 0: value = av + bv; break;
            case 1: value = av - bv; break;
            case 2: value = av * bv; break;
            case 3: value = av / bv; break;
            default: value = 0.0; break;
        }
        out->data[flat] = k_ad_round_dtype(value, out->dtype);
    }
}

// One LSTM step with the gate split and the cell update in one node. The
// gates tensor is (batch, 4*hidden) in input, forget, cell, output order.
static KVariable k_ad_lstm_cell(KVariable gates, KVariable cell, KLocation loc) {
    if (!gates.node || !cell.node) k_ad_trap(loc, "lstmCell needs two values");
    if (gates.node->rank != 2 || cell.node->rank != 2) k_ad_trap(loc, "lstmCell needs rank-2 values");
    int64_t batch = cell.node->shape[0], hidden = cell.node->shape[1];
    if (gates.node->shape[0] != batch || gates.node->shape[1] != hidden * 4)
        k_ad_trap(loc, "lstmCell gates must have shape (batch, 4*hidden)");
    KVariable result = k_ad_zeros(2, cell.node->shape, 0.0, false, cell.node->dtype, loc);
    k_ad_set_op(result, K_AD_OP_LSTM_CELL, gates, cell);
    if (result.node->requires_grad && result.node->op) {
        k_ad_op_save(result.node->op, false, gates.node);
        k_ad_op_save(result.node->op, true, cell.node);
    }
    const double *source = gates.node->data, *previous = cell.node->data;
    double *target = result.node->data;
    for (int64_t row = 0; row < batch; ++row)
        for (int64_t column = 0; column < hidden; ++column) {
            double *gate = (double *)source + (row * hidden * 4);
            double input = 1.0 / (1.0 + exp(-gate[column]));
            double forget = 1.0 / (1.0 + exp(-gate[hidden + column]));
            double candidate = tanh(gate[hidden * 2 + column]);
            target[row * hidden + column] = forget * previous[row * hidden + column] + input * candidate;
        }
    return result;
}

static KVariable k_ad_lstm_hidden(KVariable gates, KVariable cell, KLocation loc) {
    if (!gates.node || !cell.node) k_ad_trap(loc, "lstmHidden needs two values");
    if (gates.node->rank != 2 || cell.node->rank != 2) k_ad_trap(loc, "lstmHidden needs rank-2 values");
    int64_t batch = cell.node->shape[0], hidden = cell.node->shape[1];
    if (gates.node->shape[0] != batch || gates.node->shape[1] != hidden * 4)
        k_ad_trap(loc, "lstmHidden gates must have shape (batch, 4*hidden)");
    KVariable result = k_ad_zeros(2, cell.node->shape, 0.0, false, cell.node->dtype, loc);
    k_ad_set_op(result, K_AD_OP_LSTM_HIDDEN, gates, cell);
    if (result.node->requires_grad && result.node->op) {
        k_ad_op_save(result.node->op, false, gates.node);
        k_ad_op_save(result.node->op, true, cell.node);
    }
    const double *source = gates.node->data, *cellValue = cell.node->data;
    double *target = result.node->data;
    for (int64_t row = 0; row < batch; ++row)
        for (int64_t column = 0; column < hidden; ++column) {
            double *gate = (double *)source + (row * hidden * 4);
            double output = 1.0 / (1.0 + exp(-gate[hidden * 3 + column]));
            target[row * hidden + column] = output * tanh(cellValue[row * hidden + column]);
        }
    return result;
}

static KVariable k_ad_binary(KVariable left, KVariable right, int kind, KLocation loc) {
    if (!left.node || !right.node) k_ad_trap(loc, "Variable operation needs values");
    KAdNode *a = left.node;
    KAdNode *b = right.node;
    KAdBroadcast plan = k_ad_broadcast_plan(a, b, loc);
    KVariable result = k_ad_zeros(plan.rank, plan.shape, 0.0, false, a->dtype, loc);
    KAdOpKind op_kind = kind == 0 ? K_AD_OP_ADD : kind == 1 ? K_AD_OP_SUB : kind == 2 ? K_AD_OP_MUL : K_AD_OP_DIV;
    k_ad_set_op(result, op_kind, left, right);
    if (result.node->requires_grad && result.node->op) {
        // Saved tensors make the backward pass independent of later in-place writes.
        k_ad_op_save(result.node->op, false, a);
        k_ad_op_save(result.node->op, true, b);
    }
    k_ad_binary_loop(result.node, &plan, a, b, (double)kind, loc);
    k_ad_broadcast_free(&plan);
    return result;
}

static KVariable k_ad_binary_scalar(KVariable left, double scalar, int kind, KLocation loc) {
    if (!left.node) k_ad_trap(loc, "Variable operation needs a value");
    KAdNode *a = left.node;
    KVariable result = k_ad_zeros(a->rank, a->shape, 0.0, false, a->dtype, loc);
    KAdOpKind op_kind = kind == 0 ? K_AD_OP_ADD : kind == 1 ? K_AD_OP_SUB : kind == 2 ? K_AD_OP_MUL : K_AD_OP_DIV;
    k_ad_set_op(result, op_kind, left, k_variable_empty());
    if (result.node->requires_grad && result.node->op) {
        result.node->op->scalar = scalar;
        result.node->op->scalar2 = (double)kind;
        k_ad_op_save(result.node->op, false, a);
    }
    for (int64_t i = 0; i < a->length; ++i) {
        double x = a->data[i];
        double value = kind == 0 ? x + scalar : kind == 1 ? x - scalar : kind == 2 ? x * scalar : x / scalar;
        result.node->data[i] = k_ad_round_dtype(value, result.node->dtype);
    }
    return result;
}

static KVariable k_ad_scalar_binary(double scalar, KVariable right, int kind, KLocation loc) {
    if (!right.node) k_ad_trap(loc, "Variable operation needs a value");
    KAdNode *b = right.node;
    KVariable result = k_ad_zeros(b->rank, b->shape, 0.0, false, b->dtype, loc);
    KAdOpKind op_kind = kind == 0 ? K_AD_OP_ADD : kind == 1 ? K_AD_OP_SUB : kind == 2 ? K_AD_OP_MUL : K_AD_OP_DIV;
    k_ad_set_op(result, op_kind, k_variable_empty(), right);
    if (result.node->requires_grad && result.node->op) {
        result.node->op->scalar = scalar;
        result.node->op->scalar2 = (double)kind;
        k_ad_op_save(result.node->op, true, b);
    }
    for (int64_t i = 0; i < b->length; ++i) {
        double y = b->data[i];
        double value = kind == 0 ? scalar + y : kind == 1 ? scalar - y : kind == 2 ? scalar * y : scalar / y;
        result.node->data[i] = k_ad_round_dtype(value, result.node->dtype);
    }
    return result;
}

// ---------------------------------------------------------------- unary ops

typedef enum {
    K_AD_UNARY_NEG = 0,
    K_AD_UNARY_EXP,
    K_AD_UNARY_LOG,
    K_AD_UNARY_SQRT,
    K_AD_UNARY_TANH,
    K_AD_UNARY_SIGMOID,
    K_AD_UNARY_RELU,
    K_AD_UNARY_ABS,
    K_AD_UNARY_SIN,
    K_AD_UNARY_COS,
    K_AD_UNARY_TAN,
    K_AD_UNARY_ASIN,
    K_AD_UNARY_ACOS,
    K_AD_UNARY_ATAN,
    K_AD_UNARY_SINH,
    K_AD_UNARY_COSH,
    K_AD_UNARY_LOG1P,
    K_AD_UNARY_EXPM1,
    K_AD_UNARY_RSQRT,
    K_AD_UNARY_FLOOR,
    K_AD_UNARY_CEIL,
    K_AD_UNARY_ROUND,
    K_AD_UNARY_SIGN
} KAdUnary;

static KAdOpKind k_ad_unary_op(int operation) {
    switch (operation) {
        case K_AD_UNARY_NEG: return K_AD_OP_NEG;
        case K_AD_UNARY_EXP:
        case K_AD_UNARY_EXPM1: return K_AD_OP_EXP;
        case K_AD_UNARY_LOG:
        case K_AD_UNARY_LOG1P: return K_AD_OP_LOG;
        case K_AD_UNARY_SQRT:
        case K_AD_UNARY_RSQRT: return K_AD_OP_SQRT;
        case K_AD_UNARY_TANH:
        case K_AD_UNARY_SINH:
        case K_AD_UNARY_COSH: return K_AD_OP_TANH;
        case K_AD_UNARY_SIGMOID: return K_AD_OP_SIGMOID;
        case K_AD_UNARY_RELU: return K_AD_OP_RELU;
        case K_AD_UNARY_ABS: return K_AD_OP_ABS;
        default: return K_AD_OP_NONE;
    }
}

static double k_ad_unary_value(double x, int operation) {
    switch (operation) {
        case K_AD_UNARY_NEG: return -x;
        case K_AD_UNARY_EXP: return exp(x);
        case K_AD_UNARY_LOG: return log(x);
        case K_AD_UNARY_SQRT: return sqrt(x);
        case K_AD_UNARY_TANH: return tanh(x);
        case K_AD_UNARY_SIGMOID: return 1.0 / (1.0 + exp(-x));
        case K_AD_UNARY_RELU: return x > 0.0 ? x : 0.0;
        case K_AD_UNARY_ABS: return fabs(x);
        case K_AD_UNARY_SIN: return sin(x);
        case K_AD_UNARY_COS: return cos(x);
        case K_AD_UNARY_TAN: return tan(x);
        case K_AD_UNARY_ASIN: return asin(x);
        case K_AD_UNARY_ACOS: return acos(x);
        case K_AD_UNARY_ATAN: return atan(x);
        case K_AD_UNARY_SINH: return sinh(x);
        case K_AD_UNARY_COSH: return cosh(x);
        case K_AD_UNARY_LOG1P: return log1p(x);
        case K_AD_UNARY_EXPM1: return expm1(x);
        case K_AD_UNARY_RSQRT: return 1.0 / sqrt(x);
        case K_AD_UNARY_FLOOR: return floor(x);
        case K_AD_UNARY_CEIL: return ceil(x);
        case K_AD_UNARY_ROUND: return round(x);
        case K_AD_UNARY_SIGN: return x > 0.0 ? 1.0 : x < 0.0 ? -1.0 : 0.0;
        default: return x;
    }
}

static KVariable k_ad_unary(KVariable value, int operation, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable operation needs a value");
    KAdNode *a = value.node;
    KVariable result = k_ad_zeros(a->rank, a->shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, k_ad_unary_op(operation), value, k_variable_empty());
    if (result.node->requires_grad && result.node->op) {
        result.node->op->scalar = (double)operation;
        k_ad_op_save(result.node->op, false, a);
    }
    for (int64_t i = 0; i < a->length; ++i) result.node->data[i] = k_ad_round_dtype(k_ad_unary_value(a->data[i], operation), result.node->dtype);
    return result;
}

// ---------------------------------------------------------------- matmul


// One LSTM over a whole sequence: the input projection runs as one product,
// the hidden projection and gate update run once per step, and the backward
// pass walks the sequence once. The graph therefore holds one node instead of
// one node per gate operation per step.
static bool k_ad_lstm_gemm(int64_t rows, int64_t inner, int64_t cols,
        const double *a, bool transpose_a, const double *b, bool transpose_b, double *out) {
    if (k_blas_dgemm(rows, inner, cols, a, transpose_a, b, transpose_b, out)) return true;
    for (int64_t i = 0; i < rows; ++i)
        for (int64_t j = 0; j < cols; ++j) {
            double sum = 0.0;
            for (int64_t q = 0; q < inner; ++q) {
                double left = transpose_a ? a[q * rows + i] : a[i * inner + q];
                double right = transpose_b ? b[j * inner + q] : b[q * cols + j];
                sum += left * right;
            }
            out[i * cols + j] = sum;
        }
    return true;
}

static KVariable k_ad_lstm_sequence(KVariable x, KVariable wih, KVariable bih,
        KVariable whh, KVariable bhh, KLocation loc) {
    if (!x.node || !wih.node || !bih.node || !whh.node || !bhh.node)
        k_ad_trap(loc, "lstmForward needs five values");
    if (x.node->rank != 3) k_ad_trap(loc, "lstmForward input must be (steps, batch, features)");
    if (wih.node->rank != 2 || whh.node->rank != 2) k_ad_trap(loc, "lstmForward weights must be matrices");
    int64_t steps = x.node->shape[0], batch = x.node->shape[1], features = x.node->shape[2];
    int64_t gates = wih.node->shape[0];
    if (gates % 4) k_ad_trap(loc, "lstmForward weight rows must be four times the hidden size");
    int64_t hidden = gates / 4;
    if (wih.node->shape[1] != features || whh.node->shape[0] != gates || whh.node->shape[1] != hidden)
        k_ad_trap(loc, "lstmForward weight shapes do not match the input");
    if (bih.node->length != gates || bhh.node->length != gates)
        k_ad_trap(loc, "lstmForward bias must have four times the hidden size");
    int64_t shape[3] = {steps, batch, hidden};
    KVariable result = k_ad_zeros(3, shape, 0.0, false, x.node->dtype, loc);
    result.node->is_leaf = false;
    result.node->requires_grad = k_ad_grad_enabled
        && (x.node->requires_grad || wih.node->requires_grad || bih.node->requires_grad
            || whh.node->requires_grad || bhh.node->requires_grad);
    result.node->grad_enabled = result.node->requires_grad;
    if (result.node->requires_grad) {
        KAdOp *op = k_ad_op_new(K_AD_OP_LSTM_SEQUENCE, x.node, wih.node);
        op->c = bih.node; if (op->c) atomic_fetch_add_explicit(&op->c->references, 1, memory_order_relaxed);
        op->d = whh.node; if (op->d) atomic_fetch_add_explicit(&op->d->references, 1, memory_order_relaxed);
        op->e = bhh.node; if (op->e) atomic_fetch_add_explicit(&op->e->references, 1, memory_order_relaxed);
        result.node->op = op;
    }
    int64_t cellsAt = steps * batch * gates;
    int64_t hiddenAt = cellsAt + steps * batch * hidden;
    int64_t scratchAt = hiddenAt + steps * batch * hidden;
    int64_t zeroAt = scratchAt + batch * gates;
    int64_t nextAt = zeroAt + batch * hidden;
    size_t total = (size_t)(nextAt + batch * hidden);
    double *buffers = calloc(total ? total : 1, sizeof(double));
    if (!buffers) k_fatal("Out of memory");
    if (result.node->op) result.node->op->xhat = buffers;
    double *combined = buffers;
    double *cells = combined + cellsAt, *hiddens = combined + hiddenAt;
    double *scratch = combined + scratchAt, *zeros = combined + zeroAt, *nextHidden = combined + nextAt;
    memset(zeros, 0, (size_t)(batch * hidden) * sizeof(double));
    const double *source = x.node->data, *wInput = wih.node->data, *wHidden = whh.node->data;
    const double *bInput = bih.node->data, *bHidden = bhh.node->data;
    k_ad_lstm_gemm(steps * batch, features, gates, source, false, wInput, true, combined);
    for (int64_t step = 0; step < steps; ++step)
        for (int64_t row = 0; row < batch; ++row)
            for (int64_t gate = 0; gate < gates; ++gate)
                combined[(step * batch + row) * gates + gate] += bInput[gate] + bHidden[gate];
    double *previous = zeros;
    for (int64_t step = 0; step < steps; ++step) {
        k_ad_lstm_gemm(batch, hidden, gates, previous, false, wHidden, true, scratch);
        double *gatesRow = combined + step * batch * gates;
        double *cellRow = cells + step * batch * hidden;
        for (int64_t row = 0; row < batch; ++row) {
            double *gate = gatesRow + row * gates;
            const double *projected = scratch + row * gates;
            double *cell = cellRow + row * hidden;
            for (int64_t column = 0; column < gates; ++column) gate[column] += projected[column];
            for (int64_t column = 0; column < hidden; ++column) {
                double input = 1.0 / (1.0 + exp(-gate[column]));
                double forget = 1.0 / (1.0 + exp(-gate[hidden + column]));
                double candidate = tanh(gate[hidden * 2 + column]);
                double output = 1.0 / (1.0 + exp(-gate[hidden * 3 + column]));
                double value = forget * (step ? cells[(step - 1) * batch * hidden + row * hidden + column] : 0.0)
                    + input * candidate;
                cell[column] = value;
                hiddens[step * batch * hidden + row * hidden + column] = output * tanh(value);
            }
        }
        memcpy(nextHidden, hiddens + step * batch * hidden, (size_t)(batch * hidden) * sizeof(double));
        previous = nextHidden;
        memcpy(result.node->data + step * batch * hidden, nextHidden, (size_t)(batch * hidden) * sizeof(double));
    }
    if (!result.node->op) free(buffers);
    return result;
}

static void k_ad_matmul_values(const KAdNode *a, const KAdNode *b, KAdNode *out, KLocation loc) {
    int64_t rows = a->rank >= 2 ? a->shape[a->rank - 2] : 1;
    int64_t inner = a->shape[a->rank - 1];
    int64_t cols = b->rank == 1 ? 1 : b->shape[b->rank - 1];
    if (b->rank == 1 ? inner != b->shape[0] : inner != b->shape[0]) k_ad_trap(loc, "Variable matrix multiplication has different inner dimensions");
    if (out->length != rows * cols) k_ad_trap(loc, "Variable matrix multiplication result shape");
    if (out->dtype == K_AD_F64 && k_blas_dgemm(rows, inner, cols, a->data, false, b->data, false, out->data)) return;
    for (int64_t i = 0; i < rows; ++i) {
        for (int64_t j = 0; j < cols; ++j) {
            double sum = 0.0;
            for (int64_t k = 0; k < inner; ++k) {
                int64_t ai = a->rank == 1 ? k : i * inner + k;
                int64_t bi = b->rank == 1 ? k : k * cols + j;
                sum += a->data[ai] * b->data[bi];
            }
            out->data[i * cols + j] = k_ad_round_dtype(sum, out->dtype);
        }
    }
}

static KVariable k_ad_matmul(KVariable left, KVariable right, KLocation loc) {
    if (!left.node || !right.node) k_ad_trap(loc, "Variable matmul needs two values");
    KAdNode *a = left.node;
    KAdNode *b = right.node;
    if (a->rank < 1 || a->rank > 2 || b->rank < 1 || b->rank > 2) k_ad_trap(loc, "Variable matmul supports rank 1 or 2 without batching");
    if (a->rank == 1 && b->rank == 1) k_ad_trap(loc, "Variable matmul of two vectors has no result; use dot");
    int64_t inner = a->rank == 1 ? a->shape[0] : a->shape[1];
    int64_t b_inner = b->rank == 1 ? b->shape[0] : b->shape[0];
    if (inner != b_inner) k_ad_trap(loc, "Variable matmul inner dimensions differ");
    int64_t rows = a->rank == 1 ? 1 : a->shape[0];
    int64_t cols = b->rank == 1 ? 1 : b->shape[1];
    int64_t shape[2] = {rows, cols};
    int64_t rank = a->rank == 1 ? 1 : b->rank == 1 ? 1 : 2;
    if (a->rank == 1 && b->rank == 1) rank = 0;
    KVariable result = k_ad_zeros(rank, shape, 0.0, false, a->dtype, loc);
    result.node->is_leaf = false;
    result.node->requires_grad = k_ad_grad_enabled && (a->requires_grad || b->requires_grad);
    result.node->grad_enabled = result.node->requires_grad;
    if (result.node->requires_grad) {
        result.node->op = k_ad_op_new(K_AD_OP_MATMUL, a, b);
        k_ad_op_save(result.node->op, false, a);
        k_ad_op_save(result.node->op, true, b);
    }
    k_ad_matmul_values(a, b, result.node, loc);
    return result;
}

static double k_ad_dot(KVariable a, KVariable b, KLocation loc) {
    if (!a.node || !b.node || a.node->length != b.node->length) k_ad_trap(loc, "Variable.dot needs vectors of equal length");
    double sum = 0.0;
    for (int64_t i = 0; i < a.node->length; ++i) sum += a.node->data[i] * b.node->data[i];
    return sum;
}

// ---------------------------------------------------------------- reductions

static KVariable k_ad_reduce(KVariable value, int kind, int64_t axis, bool keepdim, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable reduction needs a value");
    KAdNode *a = value.node;
    bool total = axis <= -1000000;
    if (!total) {
        if (axis < 0) axis += a->rank;
        if (axis < 0 || axis >= a->rank) k_ad_trap(loc, "Variable axis out of range");
    }
    int64_t out_rank = total ? 0 : (keepdim ? a->rank : a->rank - 1);
    int64_t *out_shape = malloc((size_t)(out_rank ? out_rank : 1) * sizeof(int64_t));
    if (!out_shape) k_fatal("Out of memory");
    int64_t out_length = 1;
    if (total) {
        if (keepdim) {
            out_rank = a->rank;
            for (int64_t d = 0; d < out_rank; ++d) out_shape[d] = 1;
            out_length = 1;
        }
    } else if (keepdim) {
        for (int64_t d = 0; d < out_rank; ++d) out_shape[d] = d == axis ? 1 : a->shape[d];
        out_length = a->length / a->shape[axis];
    } else {
        for (int64_t d = 0; d < out_rank; ++d) out_shape[d] = d < axis ? a->shape[d] : a->shape[d + 1];
        out_length = a->length / a->shape[axis];
    }
    if (total) out_length = 1;
    KVariable result = k_ad_zeros(total ? (keepdim ? a->rank : 0) : out_rank, out_shape, kind == 1 ? 0.0 : (kind == 2 ? 1.0 : (kind == 3 ? -INFINITY : INFINITY)), false, a->dtype, loc);
    KAdOpKind op_kind = kind == 0 ? K_AD_OP_SUM : kind == 1 ? K_AD_OP_MEAN : kind == 2 ? K_AD_OP_PROD : kind == 3 ? K_AD_OP_MAX : K_AD_OP_MIN;
    k_ad_set_op(result, op_kind, value, k_variable_empty());
    if (result.node->requires_grad && result.node->op) {
        result.node->op->axis = axis;
        result.node->op->keepdim = keepdim;
        k_ad_op_save(result.node->op, false, a);
    }
    if (total) {
        double accumulator = kind == 2 ? 1.0 : (kind == 0 || kind == 1 ? 0.0 : (kind == 3 ? -INFINITY : INFINITY));
        if (kind == 1) {
            for (int64_t i = 0; i < a->length; ++i) accumulator += a->data[i];
            accumulator /= a->length ? a->length : 1;
        } else if (kind == 0) {
            for (int64_t i = 0; i < a->length; ++i) accumulator += a->data[i];
        } else if (kind == 2) {
            for (int64_t i = 0; i < a->length; ++i) accumulator *= a->data[i];
        } else if (kind == 3) {
            for (int64_t i = 0; i < a->length; ++i) if (a->data[i] > accumulator || i == 0) accumulator = a->data[i];
        } else {
            for (int64_t i = 0; i < a->length; ++i) if (a->data[i] < accumulator || i == 0) accumulator = a->data[i];
        }
        result.node->data[0] = k_ad_round_dtype(accumulator, result.node->dtype);
    } else {
        int64_t inner = 1;
        for (int64_t d = axis + 1; d < a->rank; ++d) inner *= a->shape[d];
        int64_t outer = 1;
        for (int64_t d = 0; d < axis; ++d) outer *= a->shape[d];
        int64_t width = a->shape[axis];
        for (int64_t o = 0; o < outer; ++o) {
            for (int64_t i = 0; i < inner; ++i) {
                double accumulator = kind == 1 || kind == 0 ? 0.0 : (kind == 2 ? 1.0 : (kind == 3 ? -INFINITY : INFINITY));
                for (int64_t a_i = 0; a_i < width; ++a_i) {
                    double item = a->data[(o * width + a_i) * inner + i];
                    if (kind == 1 || kind == 0) accumulator += item;
                    else if (kind == 2) accumulator *= item;
                    else if (kind == 3) { if (a_i == 0 || item > accumulator) accumulator = item; }
                    else { if (a_i == 0 || item < accumulator) accumulator = item; }
                }
                if (kind == 1) accumulator /= width ? width : 1;
                result.node->data[o * inner + i] = k_ad_round_dtype(accumulator, result.node->dtype);
            }
        }
    }
    free(out_shape);
    return result;
}

// ---------------------------------------------------------------- shape ops

static KVariable k_ad_reshape(KVariable value, int64_t rank, const int64_t *shape, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable reshape needs a value");
    int64_t resolved[64];
    int64_t inferred = -1;
    int64_t known = 1;
    if (rank > 64) k_ad_trap(loc, "Variable rank exceeds 64");
    for (int64_t d = 0; d < rank; ++d) {
        resolved[d] = shape[d];
        if (shape[d] == -1) {
            if (inferred >= 0) k_ad_trap(loc, "Variable reshape accepts one inferred dimension");
            inferred = d;
        } else if (shape[d] < 0) {
            k_ad_trap(loc, "Variable reshape dimensions cannot be negative");
        } else {
            known *= shape[d];
        }
    }
    if (inferred >= 0) {
        if (known == 0 || value.node->length % known) k_ad_trap(loc, "Variable reshape cannot infer this dimension");
        resolved[inferred] = value.node->length / known;
    }
    int64_t length = k_ad_length_for(rank, resolved, loc);
    if (length != value.node->length) k_ad_trap(loc, "Variable reshape needs the same number of elements");
    KVariable result = k_ad_zeros(rank, resolved, 0.0, false, value.node->dtype, loc);
    k_ad_set_op(result, K_AD_OP_RESHAPE, value, k_variable_empty());
    memcpy(result.node->data, value.node->data, (size_t)length * sizeof(double));
    return result;
}

static KVariable k_ad_transpose(KVariable value, int64_t axis0, int64_t axis1, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable transpose needs a value");
    KAdNode *a = value.node;
    if (axis0 < 0 || axis0 >= a->rank || axis1 < 0 || axis1 >= a->rank) k_ad_trap(loc, "Variable transpose axis out of range");
    int64_t *shape = malloc((size_t)(a->rank ? a->rank : 1) * sizeof(int64_t));
    if (!shape) k_fatal("Out of memory");
    for (int64_t d = 0; d < a->rank; ++d) shape[d] = a->shape[d];
    int64_t saved = shape[axis0];
    shape[axis0] = shape[axis1];
    shape[axis1] = saved;
    KVariable result = k_ad_zeros(a->rank, shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_TRANSPOSE, value, k_variable_empty());
    if (result.node->requires_grad && result.node->op) {
        result.node->op->axis = axis0;
        result.node->op->scalar2 = (double)axis1;
        k_ad_op_save(result.node->op, false, a);
    }
    if (a->rank == 2) {
        // The common case, and the one every Linear layer takes: a blocked
        // transpose without per-element coordinate arithmetic.
        int64_t rows = a->shape[0], cols = a->shape[1];
        const int64_t block = 32;
        for (int64_t i0 = 0; i0 < rows; i0 += block)
            for (int64_t j0 = 0; j0 < cols; j0 += block) {
                int64_t i1 = i0 + block < rows ? i0 + block : rows;
                int64_t j1 = j0 + block < cols ? j0 + block : cols;
                for (int64_t i = i0; i < i1; ++i)
                    for (int64_t j = j0; j < j1; ++j)
                        result.node->data[j * rows + i] = a->data[i * cols + j];
            }
        free(shape);
        return result;
    }
    // Multi-head attention transposes the last two axes of rank-3 and rank-4
    // values. Nested loops give each element a fixed offset.
    if ((a->rank == 3 || a->rank == 4) && axis0 == 1 && axis1 == 2) {
        if (a->rank == 3) {
            int64_t d0 = a->shape[0], d1 = a->shape[1], d2 = a->shape[2];
            double *out = result.node->data;
            const double *in = a->data;
            for (int64_t i = 0; i < d0; ++i)
                for (int64_t j = 0; j < d2; ++j)
                    for (int64_t k = 0; k < d1; ++k)
                        out[(i * d2 + j) * d1 + k] = in[(i * d1 + k) * d2 + j];
        } else {
            int64_t d0 = a->shape[0], d1 = a->shape[1], d2 = a->shape[2], d3 = a->shape[3];
            double *out = result.node->data;
            const double *in = a->data;
            for (int64_t i = 0; i < d0; ++i)
                for (int64_t j = 0; j < d2; ++j)
                    for (int64_t k = 0; k < d1; ++k) {
                        const double *source = in + (((i * d1 + k) * d2) + j) * d3;
                        double *target = out + (((i * d2 + j) * d1) + k) * d3;
                        for (int64_t d = 0; d < d3; ++d) target[d] = source[d];
                    }
        }
        free(shape);
        return result;
    }
    int64_t coordinates[64];
    for (int64_t flat = 0; flat < a->length; ++flat) {
        int64_t rest = flat;
        for (int64_t d = a->rank; d-- > 0;) {
            coordinates[d] = shape[d] ? rest % shape[d] : 0;
            rest = shape[d] ? rest / shape[d] : 0;
        }
        int64_t source[64];
        for (int64_t d = 0; d < a->rank; ++d) source[d] = coordinates[d];
        int64_t swap = source[axis0];
        source[axis0] = source[axis1];
        source[axis1] = swap;
        int64_t offset = 0, stride = 1;
        for (int64_t d = a->rank; d-- > 0;) {
            offset += source[d] * stride;
            stride *= a->shape[d];
        }
        result.node->data[flat] = a->data[offset];
    }
    free(shape);
    return result;
}

static KVariable k_ad_permute(KVariable value, int64_t rank, const int64_t *axes, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable permute needs a value");
    KAdNode *a = value.node;
    if (rank != a->rank) k_ad_trap(loc, "Variable permute needs one axis per dimension");
    int64_t *shape = malloc((size_t)(rank ? rank : 1) * sizeof(int64_t));
    if (!shape) k_fatal("Out of memory");
    for (int64_t d = 0; d < rank; ++d) {
        if (axes[d] < 0 || axes[d] >= rank) k_ad_trap(loc, "Variable permute axis out of range");
        shape[d] = a->shape[axes[d]];
    }
    KVariable result = k_ad_zeros(rank, shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_TRANSPOSE, value, k_variable_empty());
    if (result.node->requires_grad && result.node->op) {
        result.node->op->rank = rank;
        result.node->op->axes = malloc((size_t)(rank ? rank : 1) * sizeof(int64_t));
        if (!result.node->op->axes) k_fatal("Out of memory");
        memcpy(result.node->op->axes, axes, (size_t)rank * sizeof(int64_t));
        k_ad_op_save(result.node->op, false, a);
    }
    int64_t coordinates[64];
    int64_t source[64];
    for (int64_t flat = 0; flat < a->length; ++flat) {
        int64_t rest = flat;
        for (int64_t d = rank; d-- > 0;) {
            coordinates[d] = rest % shape[d];
            rest /= shape[d];
        }
        for (int64_t d = 0; d < rank; ++d) source[axes[d]] = coordinates[d];
        int64_t offset = 0, stride = 1;
        for (int64_t d = rank; d-- > 0;) { offset += source[d] * stride; stride *= a->shape[d]; }
        result.node->data[flat] = a->data[offset];
    }
    free(shape);
    return result;
}

static KVariable k_ad_broadcast_to(KVariable value, int64_t rank, const int64_t *shape, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable broadcast needs a value");
    KAdNode *a = value.node;
    int64_t target[64];
    if (rank > 64) k_ad_trap(loc, "Variable rank exceeds 64");
    for (int64_t d = 0; d < rank; ++d) {
        int64_t source = d - (rank - a->rank);
        int64_t size = source >= 0 ? a->shape[source] : 1;
        if (size != 1 && size != shape[d]) k_ad_trap(loc, "Variable cannot broadcast to the target shape");
        target[d] = shape[d];
    }
    KVariable result = k_ad_zeros(rank, target, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_BROADCAST, value, k_variable_empty());
    int64_t coordinates[64];
    for (int64_t flat = 0; flat < result.node->length; ++flat) {
        int64_t rest = flat;
        for (int64_t d = rank; d-- > 0;) { coordinates[d] = rest % target[d]; rest /= target[d]; }
        int64_t offset = 0, stride = 1;
        for (int64_t d = a->rank; d-- > 0;) {
            int64_t out_d = d + (rank - a->rank);
            offset += (a->shape[d] == 1 ? 0 : coordinates[out_d]) * stride;
            stride *= a->shape[d];
        }
        result.node->data[flat] = a->data[offset];
    }
    return result;
}

static KVariable k_ad_squeeze(KVariable value, int64_t dim, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable squeeze needs a value");
    KAdNode *a = value.node;
    int64_t rank = 0;
    int64_t *shape = malloc((size_t)(a->rank ? a->rank : 1) * sizeof(int64_t));
    if (!shape) k_fatal("Out of memory");
    for (int64_t d = 0; d < a->rank; ++d) {
        if (a->shape[d] == 1 && (dim < 0 || d == dim)) continue;
        shape[rank++] = a->shape[d];
    }
    KVariable result = k_ad_zeros(rank, shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_RESHAPE, value, k_variable_empty());
    memcpy(result.node->data, a->data, (size_t)a->length * sizeof(double));
    free(shape);
    return result;
}

static KVariable k_ad_unsqueeze(KVariable value, int64_t dim, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable unsqueeze needs a value");
    KAdNode *a = value.node;
    if (dim < 0 || dim > a->rank) k_ad_trap(loc, "Variable unsqueeze axis out of range");
    int64_t rank = a->rank + 1;
    int64_t *shape = malloc((size_t)rank * sizeof(int64_t));
    if (!shape) k_fatal("Out of memory");
    for (int64_t d = 0, s = 0; d < rank; ++d) shape[d] = d == dim ? 1 : a->shape[s++];
    KVariable result = k_ad_zeros(rank, shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_RESHAPE, value, k_variable_empty());
    memcpy(result.node->data, a->data, (size_t)a->length * sizeof(double));
    free(shape);
    return result;
}

// ---------------------------------------------------------------- in place

static void k_ad_require_leaf(KVariable value, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable is empty");
    if (!value.node->is_leaf) k_ad_trap(loc, "In-place update needs a leaf Variable; call detach().copy() or update a parameter");
}

static KVariable k_ad_inplace(KVariable value, KVariable other, int kind, KLocation loc) {
    k_ad_require_leaf(value, loc);
    if (!other.node) k_ad_trap(loc, "In-place update needs a value");
    KAdNode *a = value.node;
    KAdNode *b = other.node;
    KAdBroadcast plan = k_ad_broadcast_plan(a, b, loc);
    for (int64_t d = 0; d < plan.rank; ++d) {
        int64_t source = d - (plan.rank - a->rank);
        if (source < 0 ? plan.shape[d] != 1 : a->shape[source] != plan.shape[d]) {
            k_ad_broadcast_free(&plan);
            k_ad_trap(loc, "In-place update cannot grow a Variable");
        }
    }
    int64_t coordinates[64];
    for (int64_t flat = 0; flat < a->length; ++flat) {
        int64_t rest = flat;
        int64_t b_index = 0;
        for (int64_t d = plan.rank; d-- > 0;) {
            coordinates[d] = rest % plan.shape[d];
            rest /= plan.shape[d];
            b_index += coordinates[d] * plan.b_offset[d];
        }
        double x = a->data[flat], y = b->data[b_index];
        double result = kind == 0 ? x + y : kind == 1 ? x - y : kind == 2 ? x * y : x / y;
        a->data[flat] = k_ad_round_dtype(result, a->dtype);
    }
    a->version += 1;
    k_ad_broadcast_free(&plan);
    return k_variable_copy(value);
}

static KVariable k_ad_inplace_scalar(KVariable value, double scalar, int kind, KLocation loc) {
    k_ad_require_leaf(value, loc);
    KAdNode *a = value.node;
    for (int64_t i = 0; i < a->length; ++i) {
        double x = a->data[i];
        double result = kind == 0 ? x + scalar : kind == 1 ? x - scalar : kind == 2 ? x * scalar : x / scalar;
        a->data[i] = k_ad_round_dtype(result, a->dtype);
    }
    a->version += 1;
    return k_variable_copy(value);
}

static KVariable k_ad_fill(KVariable value, double fill, KLocation loc) {
    k_ad_require_leaf(value, loc);
    for (int64_t i = 0; i < value.node->length; ++i) value.node->data[i] = k_ad_round_dtype(fill, value.node->dtype);
    value.node->version += 1;
    return k_variable_copy(value);
}

static KVariable k_ad_copy_inplace(KVariable target, KVariable source, KLocation loc) {
    k_ad_require_leaf(target, loc);
    if (!source.node || !k_ad_same_shape(target.node, source.node)) k_ad_trap(loc, "Variable.copy_ needs values with the same shape");
    memcpy(target.node->data, source.node->data, (size_t)target.node->length * sizeof(double));
    target.node->version += 1;
    return k_variable_copy(target);
}

// ---------------------------------------------------------------- autograd

typedef struct {
    KAdNode **slots;
    size_t capacity;
    size_t length;
} KAdSet;

static uint64_t k_ad_hash_pointer(const void *pointer) {
    uint64_t value = (uint64_t)(uintptr_t)pointer;
    value ^= value >> 33;
    value *= UINT64_C(0xff51afd7ed558ccd);
    value ^= value >> 33;
    return value;
}

static void k_ad_set_init(KAdSet *set) {
    set->capacity = 64;
    set->length = 0;
    set->slots = calloc(set->capacity, sizeof *set->slots);
    if (!set->slots) k_fatal("Out of memory");
}

static void k_ad_set_free(KAdSet *set) { free(set->slots); set->slots = NULL; set->capacity = set->length = 0; }

static void k_ad_set_grow(KAdSet *set) {
    size_t capacity = set->capacity * 2;
    KAdNode **slots = calloc(capacity, sizeof *slots);
    if (!slots) k_fatal("Out of memory");
    for (size_t i = 0; i < set->capacity; ++i) {
        if (!set->slots[i]) continue;
        size_t slot = (size_t)k_ad_hash_pointer(set->slots[i]) & (capacity - 1);
        while (slots[slot]) slot = (slot + 1) & (capacity - 1);
        slots[slot] = set->slots[i];
    }
    free(set->slots);
    set->slots = slots;
    set->capacity = capacity;
}

static bool k_ad_set_contains(KAdSet *set, KAdNode *node) {
    size_t slot = (size_t)k_ad_hash_pointer(node) & (set->capacity - 1);
    while (set->slots[slot]) {
        if (set->slots[slot] == node) return true;
        slot = (slot + 1) & (set->capacity - 1);
    }
    return false;
}

static void k_ad_set_add(KAdSet *set, KAdNode *node) {
    if (k_ad_set_contains(set, node)) return;
    if (set->length * 2 >= set->capacity) k_ad_set_grow(set);
    size_t slot = (size_t)k_ad_hash_pointer(node) & (set->capacity - 1);
    while (set->slots[slot]) slot = (slot + 1) & (set->capacity - 1);
    set->slots[slot] = node;
    set->length += 1;
}

static void k_ad_topo_visit(KAdNode *node, KAdSet *seen, KAdNode ***order, size_t *length, size_t *capacity) {
    if (!node || k_ad_set_contains(seen, node)) return;
    k_ad_set_add(seen, node);
    k_ad_retain_node(node);
    if (node->op) {
        k_ad_topo_visit(node->op->a, seen, order, length, capacity);
        k_ad_topo_visit(node->op->b, seen, order, length, capacity);
    }
    if (*length == *capacity) {
        *capacity = *capacity ? *capacity * 2 : 64;
        KAdNode **grown = realloc(*order, *capacity * sizeof **order);
        if (!grown) k_fatal("Out of memory");
        *order = grown;
    }
    (*order)[(*length)++] = node;
}

static KVariable k_ad_ones_like(KVariable value, KLocation loc) {
    return k_ad_zeros(value.node->rank, value.node->shape, 1.0, false, value.node->dtype, loc);
}

static int64_t k_ad_index_into(const KAdNode *target, int64_t out_rank, const int64_t *out_shape, int64_t flat) {
    int64_t coordinates[64];
    int64_t rest = flat;
    for (int64_t d = out_rank; d-- > 0;) {
        coordinates[d] = out_shape[d] ? rest % out_shape[d] : 0;
        rest = out_shape[d] ? rest / out_shape[d] : 0;
    }
    int64_t offset = 0, stride = 1;
    for (int64_t d = target->rank; d-- > 0;) {
        int64_t out_d = d + (out_rank - target->rank);
        int64_t coordinate = target->shape[d] == 1 ? 0 : (out_d >= 0 ? coordinates[out_d] : 0);
        offset += coordinate * stride;
        stride *= target->shape[d];
    }
    return offset;
}

static void k_ad_transpose_grad(KAdNode *a, const double *g, int64_t rank, const int64_t *shape, const int64_t *axes, int64_t axis0, int64_t axis1) {
    double *contrib = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
    if (!contrib) k_fatal("Out of memory");
    if (a->rank == 2 && !axes) {
        int64_t rows = a->shape[0], cols = a->shape[1];
        for (int64_t j = 0; j < cols; ++j)
            for (int64_t i = 0; i < rows; ++i)
                contrib[i * cols + j] += g[j * rows + i];
        k_ad_accumulate(a, a->rank, a->shape, contrib);
        free(contrib);
        return;
    }
    if ((a->rank == 3 || a->rank == 4) && !axes && axis0 == 1 && axis1 == 2) {
        int64_t d0 = a->shape[0], d1 = a->shape[1], d2 = a->shape[2];
        if (a->rank == 3) {
            for (int64_t i = 0; i < d0; ++i)
                for (int64_t j = 0; j < d2; ++j)
                    for (int64_t k = 0; k < d1; ++k)
                        contrib[(i * d1 + k) * d2 + j] += g[(i * d2 + j) * d1 + k];
        } else {
            int64_t d3 = a->shape[3];
            for (int64_t i = 0; i < d0; ++i)
                for (int64_t j = 0; j < d2; ++j)
                    for (int64_t k = 0; k < d1; ++k) {
                        double *target = contrib + (((i * d1 + k) * d2) + j) * d3;
                        const double *source = g + (((i * d2 + j) * d1) + k) * d3;
                        for (int64_t d = 0; d < d3; ++d) target[d] += source[d];
                    }
        }
        k_ad_accumulate(a, a->rank, a->shape, contrib);
        free(contrib);
        return;
    }
    int64_t coordinates[64], source[64];
    for (int64_t flat = 0; flat < a->length; ++flat) {
        int64_t rest = flat;
        for (int64_t d = rank; d-- > 0;) { coordinates[d] = shape[d] ? rest % shape[d] : 0; rest = shape[d] ? rest / shape[d] : 0; }
        if (axes) {
            for (int64_t d = 0; d < rank; ++d) source[axes[d]] = coordinates[d];
        } else {
            for (int64_t d = 0; d < rank; ++d) source[d] = coordinates[d];
            int64_t swap = source[axis0]; source[axis0] = source[axis1]; source[axis1] = swap;
        }
        int64_t offset = 0, stride = 1;
        for (int64_t d = rank; d-- > 0;) { offset += source[d] * stride; stride *= a->shape[d]; }
        contrib[offset] += g[flat];
    }
    k_ad_accumulate(a, a->rank, a->shape, contrib);
    free(contrib);
}

// The Adam update as one pass over the leaf storage. It is deliberately not
// part of the autograd graph: optimizers change leaves in place.
static void k_ad_adam_step(KVariable parameter, KVariable gradient, KVariable first, KVariable second,
                           double learningRate, double beta1, double beta2, double epsilon,
                           double correction1, double correction2, double weightDecay, KLocation loc) {
    if (!parameter.node || !gradient.node || !first.node || !second.node) k_ad_trap(loc, "Variable.adamStep needs four values");
    KAdNode *p = parameter.node, *g = gradient.node, *m = first.node, *v = second.node;
    if (g->length != p->length || m->length != p->length || v->length != p->length) k_ad_trap(loc, "Variable.adamStep needs matching shapes");
    if (!p->is_leaf) k_ad_trap(loc, "Variable.adamStep updates a leaf parameter");
    double *pv = p->data, *gv = g->data, *mv = m->data, *vv = v->data;
    double keep1 = 1.0 - beta1, keep2 = 1.0 - beta2;
    double inv1 = correction1 > 0.0 ? 1.0 / correction1 : 1.0;
    double inv2 = correction2 > 0.0 ? 1.0 / correction2 : 1.0;
    for (int64_t i = 0; i < p->length; ++i) {
        double grad = gv[i] + weightDecay * pv[i];
        mv[i] = beta1 * mv[i] + keep1 * grad;
        vv[i] = beta2 * vv[i] + keep2 * grad * grad;
        pv[i] -= learningRate * (mv[i] * inv1) / (sqrt(vv[i] * inv2) + epsilon);
    }
    p->version += 1;
    m->version += 1;
    v->version += 1;
}

static void k_ad_backward(KVariable root, bool retain_graph, KLocation loc) {
    if (!root.node) k_ad_trap(loc, "Variable.backward needs a value");
    if (!root.node->requires_grad) k_ad_trap(loc, "Variable.backward needs a value with requiresGrad");
    KAdSet seen;
    k_ad_set_init(&seen);
    KAdNode **order = NULL;
    size_t length = 0, capacity = 0;
    k_ad_topo_visit(root.node, &seen, &order, &length, &capacity);
    double *seed = malloc((size_t)(root.node->length ? root.node->length : 1) * sizeof(double));
    if (!seed) k_fatal("Out of memory");
    for (int64_t i = 0; i < root.node->length; ++i) seed[i] = 1.0;
    k_ad_accumulate(root.node, root.node->rank, root.node->shape, seed);
    free(seed);
    for (size_t step = length; step-- > 0;) {
        KAdNode *node = order[step];
        if (!node->grad || !node->op) continue;
        KAdOp *op = node->op;
        KAdNode *a = op->a;
        KAdNode *b = op->b;
        int64_t n = node->length;
        double *g = node->grad;
        switch (op->kind) {
            case K_AD_OP_ADD:
                if (a) k_ad_accumulate(a, node->rank, node->shape, g);
                if (b) k_ad_accumulate(b, node->rank, node->shape, g);
                break;
            case K_AD_OP_SUB:
                if (a) k_ad_accumulate(a, node->rank, node->shape, g);
                if (b) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = -g[i];
                    k_ad_accumulate(b, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_MUL:
                if (a && b) {
                    double *ga = malloc((size_t)n * sizeof(double));
                    double *gb = malloc((size_t)n * sizeof(double));
                    if (!ga || !gb) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) {
                        int64_t ai = k_ad_index_into(a, node->rank, node->shape, i);
                        int64_t bi = k_ad_index_into(b, node->rank, node->shape, i);
                        ga[i] = g[i] * op->saved_b.items[bi];
                        gb[i] = g[i] * op->saved_a.items[ai];
                    }
                    k_ad_accumulate(a, node->rank, node->shape, ga);
                    k_ad_accumulate(b, node->rank, node->shape, gb);
                    free(ga); free(gb);
                } else {
                    double scalar = op->scalar;
                    double sign = (op->kind == K_AD_OP_MUL) ? 1.0 : 1.0;
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = g[i] * scalar * sign;
                    if (a) k_ad_accumulate(a, node->rank, node->shape, contrib);
                    if (b) k_ad_accumulate(b, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_DIV:
                if (a && b) {
                    double *ga = malloc((size_t)n * sizeof(double));
                    double *gb = malloc((size_t)n * sizeof(double));
                    if (!ga || !gb) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) {
                        int64_t ai = k_ad_index_into(a, node->rank, node->shape, i);
                        int64_t bi = k_ad_index_into(b, node->rank, node->shape, i);
                        double bv = op->saved_b.items[bi];
                        ga[i] = g[i] / bv;
                        gb[i] = -g[i] * op->saved_a.items[ai] / (bv * bv);
                    }
                    k_ad_accumulate(a, node->rank, node->shape, ga);
                    k_ad_accumulate(b, node->rank, node->shape, gb);
                    free(ga); free(gb);
                } else if (a) {
                    double scalar = op->scalar;
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = g[i] / scalar;
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                } else if (b) {
                    double scalar = op->scalar;
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) {
                        int64_t bi = k_ad_index_into(b, node->rank, node->shape, i);
                        double bv = op->saved_b.items[bi];
                        contrib[i] = -g[i] * scalar / (bv * bv);
                    }
                    k_ad_accumulate(b, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_NEG: {
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = -g[i];
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            }
            case K_AD_OP_MATMUL:
                if (a && b && a->rank == 2 && b->rank == 2) {
                    int64_t rows = a->shape[0], inner = a->shape[1], cols = b->shape[1];
                    double *ga = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    double *gb = calloc((size_t)(b->length ? b->length : 1), sizeof(double));
                    if (!ga || !gb) k_fatal("Out of memory");
                    bool computed = g && k_blas_dgemm(rows, cols, inner, g, false, b->data, true, ga)
                        && k_blas_dgemm(inner, rows, cols, a->data, true, g, false, gb);
                    if (!computed) {
                        for (int64_t i = 0; i < rows; ++i) {
                            for (int64_t q = 0; q < inner; ++q) {
                                for (int64_t j = 0; j < cols; ++j) {
                                    ga[i * inner + q] += g[i * cols + j] * b->data[q * cols + j];
                                    gb[q * cols + j] += a->data[i * inner + q] * g[i * cols + j];
                                }
                            }
                        }
                    }
                    k_ad_accumulate(a, a->rank, a->shape, ga);
                    k_ad_accumulate(b, b->rank, b->shape, gb);
                    free(ga); free(gb);
                } else if (a && b && a->rank == 1 && b->rank == 2) {
                    int64_t inner = a->length, cols = b->shape[1];
                    double *ga = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    double *gb = calloc((size_t)(b->length ? b->length : 1), sizeof(double));
                    if (!ga || !gb) k_fatal("Out of memory");
                    bool computed = g && k_blas_dgemm(1, cols, inner, g, false, b->data, true, ga)
                        && k_blas_dgemm(inner, 1, cols, a->data, false, g, false, gb);
                    if (!computed) {
                        for (int64_t j = 0; j < cols; ++j)
                            for (int64_t q = 0; q < inner; ++q) {
                                ga[q] += g[j] * b->data[q * cols + j];
                                gb[q * cols + j] += a->data[q] * g[j];
                            }
                    }
                    k_ad_accumulate(a, a->rank, a->shape, ga);
                    k_ad_accumulate(b, b->rank, b->shape, gb);
                    free(ga); free(gb);
                } else if (a && b && b->rank == 1) {
                    int64_t rows = a->shape[0], inner = a->shape[1];
                    double *ga = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    double *gb = calloc((size_t)(b->length ? b->length : 1), sizeof(double));
                    if (!ga || !gb) k_fatal("Out of memory");
                    bool computed = g && k_blas_dgemm(rows, 1, inner, g, false, b->data, false, ga)
                        && k_blas_dgemm(inner, rows, 1, a->data, true, g, false, gb);
                    if (!computed) {
                        for (int64_t i = 0; i < rows; ++i)
                            for (int64_t q = 0; q < inner; ++q) {
                                ga[i * inner + q] += g[i] * b->data[q];
                                gb[q] += a->data[i * inner + q] * g[i];
                            }
                    }
                    k_ad_accumulate(a, a->rank, a->shape, ga);
                    k_ad_accumulate(b, b->rank, b->shape, gb);
                    free(ga); free(gb);
                } else if (a && b) {
                    double *ga = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    double *gb = calloc((size_t)(b->length ? b->length : 1), sizeof(double));
                    if (!ga || !gb) k_fatal("Out of memory");
                    for (int64_t k = 0; k < a->length; ++k) {
                        ga[k] = g[0] * b->data[k];
                        gb[k] = g[0] * a->data[k];
                    }
                    k_ad_accumulate(a, a->rank, a->shape, ga);
                    k_ad_accumulate(b, b->rank, b->shape, gb);
                    free(ga); free(gb);
                }
                break;
            case K_AD_OP_SUM:
            case K_AD_OP_MEAN:
            case K_AD_OP_PROD:
            case K_AD_OP_MAX:
            case K_AD_OP_MIN:
                if (a) {
                    int64_t count = a->length;
                    double *contrib = calloc((size_t)(count ? count : 1), sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    int64_t axis = op->axis;
                    bool total = axis <= -1000000;
                    if (op->kind == K_AD_OP_SUM || op->kind == K_AD_OP_MEAN) {
                        if (!total && axis < 0) axis += a->rank;
                        double divisor = 1.0;
                        if (op->kind == K_AD_OP_MEAN) divisor = total ? (count ? count : 1) : (a->shape[axis] ? a->shape[axis] : 1);
                        if (total) {
                            for (int64_t i = 0; i < count; ++i) contrib[i] = g[0] / divisor;
                        } else {
                            int64_t inner = 1; for (int64_t d = axis + 1; d < a->rank; ++d) inner *= a->shape[d];
                            int64_t outer = 1; for (int64_t d = 0; d < axis; ++d) outer *= a->shape[d];
                            int64_t width = a->shape[axis];
                            for (int64_t o = 0; o < outer; ++o)
                                for (int64_t i = 0; i < inner; ++i)
                                    for (int64_t q = 0; q < width; ++q)
                                        contrib[(o * width + q) * inner + i] = g[o * inner + i] / divisor;
                        }
                    } else if (op->kind == K_AD_OP_PROD) {
                        if (total) {
                            for (int64_t i = 0; i < count; ++i) {
                                double product = 1.0;
                                for (int64_t q = 0; q < count; ++q) if (q != i) product *= a->data[q];
                                contrib[i] = g[0] * product;
                            }
                        } else {
                            int64_t inner = 1; for (int64_t d = axis + 1; d < a->rank; ++d) inner *= a->shape[d];
                            int64_t outer = 1; for (int64_t d = 0; d < axis; ++d) outer *= a->shape[d];
                            int64_t width = a->shape[axis];
                            for (int64_t o = 0; o < outer; ++o)
                                for (int64_t i = 0; i < inner; ++i)
                                    for (int64_t q = 0; q < width; ++q) {
                                        double product = 1.0;
                                        for (int64_t r = 0; r < width; ++r) if (r != q) product *= a->data[(o * width + r) * inner + i];
                                        contrib[(o * width + q) * inner + i] = g[o * inner + i] * product;
                                    }
                        }
                    } else {
                        if (total) {
                            int64_t best = 0;
                            for (int64_t i = 1; i < count; ++i) {
                                if ((op->kind == K_AD_OP_MAX && a->data[i] > a->data[best]) || (op->kind == K_AD_OP_MIN && a->data[i] < a->data[best])) best = i;
                            }
                            if (count) contrib[best] = g[0];
                        } else {
                            int64_t inner = 1; for (int64_t d = axis + 1; d < a->rank; ++d) inner *= a->shape[d];
                            int64_t outer = 1; for (int64_t d = 0; d < axis; ++d) outer *= a->shape[d];
                            int64_t width = a->shape[axis];
                            for (int64_t o = 0; o < outer; ++o)
                                for (int64_t i = 0; i < inner; ++i) {
                                    int64_t best = 0;
                                    for (int64_t q = 1; q < width; ++q) {
                                        double candidate = a->data[(o * width + q) * inner + i];
                                        double current = a->data[(o * width + best) * inner + i];
                                        if ((op->kind == K_AD_OP_MAX && candidate > current) || (op->kind == K_AD_OP_MIN && candidate < current)) best = q;
                                    }
                                    if (width) contrib[(o * width + best) * inner + i] = g[o * inner + i];
                                }
                        }
                    }
                    k_ad_accumulate(a, a->rank, a->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_RESHAPE:
            case K_AD_OP_CLONE:
            case K_AD_OP_CAST:
                // Reshape keeps row-major order, so the gradient is the same flat array.
                if (a) k_ad_accumulate(a, a->rank, a->shape, g);
                break;
            case K_AD_OP_BROADCAST:
                if (a) k_ad_accumulate(a, node->rank, node->shape, g);
                break;
            case K_AD_OP_TRANSPOSE:
                if (a) k_ad_transpose_grad(a, g, node->rank, node->shape, op->axes, op->axis, (int64_t)op->scalar2);
                break;
            case K_AD_OP_LEAKY_RELU:
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = g[i] * (op->saved_a.items[i] >= 0.0 ? 1.0 : op->scalar);
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_ELU:
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) {
                        double x = op->saved_a.items[i];
                        contrib[i] = g[i] * (x >= 0.0 ? 1.0 : op->scalar * exp(x));
                    }
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_GELU:
                if (a) {
                    const double root = 0.70710678118654752440;
                    const double scale = 0.39894228040143267794; // 1/sqrt(2*pi)
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) {
                        double x = op->saved_a.items[i];
                        double inner = root * (x + 0.044715 * x * x * x);
                        double t = tanh(inner);
                        double derivative = 0.5 * (1.0 + t) + 0.5 * x * (1.0 - t * t) * root * (1.0 + 3.0 * 0.044715 * x * x);
                        contrib[i] = g[i] * derivative;
                    }
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_SILU:
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) {
                        double x = op->saved_a.items[i];
                        double sigmoid = 1.0 / (1.0 + exp(-x));
                        contrib[i] = g[i] * sigmoid * (1.0 + x * (1.0 - sigmoid));
                    }
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_SOFTPLUS:
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) {
                        double x = op->saved_a.items[i];
                        contrib[i] = g[i] * (op->scalar * x > op->scalar2 ? 1.0 : 1.0 / (1.0 + exp(-op->scalar * x)));
                    }
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_CLAMP:
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) {
                        double x = op->saved_a.items[i];
                        contrib[i] = (x >= op->scalar && x <= op->scalar2) ? g[i] : 0.0;
                    }
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_NORM:
                if (a) {
                    double total = 0.0;
                    for (int64_t i = 0; i < a->length; ++i) total += op->saved_a.items[i] * op->saved_a.items[i];
                    double length = sqrt(total);
                    double *contrib = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    if (length > 0.0) for (int64_t i = 0; i < a->length; ++i) contrib[i] = g[0] * op->saved_a.items[i] / length;
                    k_ad_accumulate(a, a->rank, a->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_RELU:
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = op->saved_a.items[i] > 0.0 ? g[i] : 0.0;
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_SIGMOID:
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = g[i] * node->data[i] * (1.0 - node->data[i]);
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_TANH:
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = g[i] * (1.0 - node->data[i] * node->data[i]);
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_EXP:
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = g[i] * op->saved_a.items[i] * exp(op->saved_a.items[i]);
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_LOG:
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = g[i] / op->saved_a.items[i];
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_SQRT:
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = g[i] / (2.0 * node->data[i]);
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_ABS:
                if (a) {
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = g[i] * (op->saved_a.items[i] > 0.0 ? 1.0 : op->saved_a.items[i] < 0.0 ? -1.0 : 0.0);
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_LSTM_SEQUENCE: {
                if (a && b && op->xhat) {
                    int64_t steps = node->shape[0], batch = node->shape[1], hidden = node->shape[2];
                    int64_t gates = hidden * 4;
                    if (a->rank == 3) {
                        int64_t features = a->shape[2];
                        double *combined = op->xhat;
                        double *cells = combined + steps * batch * gates;
                        double *hiddens = cells + steps * batch * hidden;
                        double *dhNext = hiddens + steps * batch * hidden;
                        double *dcNext = dhNext + batch * hidden;
                        memset(dhNext, 0, (size_t)(batch * hidden) * sizeof(double));
                        memset(dcNext, 0, (size_t)(batch * hidden) * sizeof(double));
                        const double *grad = g;
                        for (int64_t step = steps - 1; step >= 0; --step) {
                            double *gate = combined + step * batch * gates;
                            for (int64_t row = 0; row < batch; ++row) {
                                double *dg = gate + row * gates;
                                const double *cell = cells + step * batch * hidden + row * hidden;
                                const double *previous = step ? cells + (step - 1) * batch * hidden + row * hidden : NULL;
                                const double *dh = grad + (step * batch + row) * hidden;
                                for (int64_t column = 0; column < hidden; ++column) {
                                    double input = 1.0 / (1.0 + exp(-dg[column]));
                                    double forget = 1.0 / (1.0 + exp(-dg[hidden + column]));
                                    double candidate = tanh(dg[hidden * 2 + column]);
                                    double output = 1.0 / (1.0 + exp(-dg[hidden * 3 + column]));
                                    double squashed = tanh(cell[column]);
                                    double dc = dcNext[row * hidden + column] + (dh[column] + dhNext[row * hidden + column])
                                        * output * (1.0 - squashed * squashed);
                                    double dOutput = (dh[column] + dhNext[row * hidden + column]) * squashed;
                                    dg[column] = dc * candidate * input * (1.0 - input);
                                    dg[hidden + column] = dc * (previous ? previous[column] : 0.0) * forget * (1.0 - forget);
                                    dg[hidden * 2 + column] = dc * input * (1.0 - candidate * candidate);
                                    dg[hidden * 3 + column] = dOutput * output * (1.0 - output);
                                    dcNext[row * hidden + column] = dc * forget;
                                }
                            }
                            k_ad_lstm_gemm(batch, gates, hidden, gate, false, op->d->data, false, dhNext);
                        }
                        double *previousStates = calloc((size_t)(steps * batch * hidden ? steps * batch * hidden : 1), sizeof(double));
                        double *dInput = calloc((size_t)(steps * batch * features ? steps * batch * features : 1), sizeof(double));
                        double *dWeightInput = calloc((size_t)(gates * features ? gates * features : 1), sizeof(double));
                        double *dWeightHidden = calloc((size_t)(gates * hidden ? gates * hidden : 1), sizeof(double));
                        double *dBias = calloc((size_t)gates, sizeof(double));
                        if (!previousStates || !dInput || !dWeightInput || !dWeightHidden || !dBias) k_fatal("Out of memory");
                        for (int64_t step = 1; step < steps; ++step)
                            memcpy(previousStates + step * batch * hidden, hiddens + (step - 1) * batch * hidden, (size_t)(batch * hidden) * sizeof(double));
                        for (int64_t row = 0; row < steps * batch; ++row)
                            for (int64_t column = 0; column < gates; ++column)
                                dBias[column] += combined[row * gates + column];
                        k_ad_lstm_gemm(steps * batch, gates, features, combined, false, b->data, false, dInput);
                        k_ad_lstm_gemm(gates, steps * batch, features, combined, true, a->data, false, dWeightInput);
                        k_ad_lstm_gemm(gates, steps * batch, hidden, combined, true, previousStates, false, dWeightHidden);
                        k_ad_accumulate(a, a->rank, a->shape, dInput);
                        k_ad_accumulate(b, b->rank, b->shape, dWeightInput);
                        k_ad_accumulate(op->c, op->c->rank, op->c->shape, dBias);
                        k_ad_accumulate(op->d, op->d->rank, op->d->shape, dWeightHidden);
                        k_ad_accumulate(op->e, op->e->rank, op->e->shape, dBias);
                        free(previousStates); free(dInput); free(dWeightInput); free(dWeightHidden); free(dBias);
                    }
                }
                break;
            }
            case K_AD_OP_LSTM_CELL: {
                if (a || b) {
                    const KTensor *gates = &op->saved_a;
                    const KTensor *previous = &op->saved_b;
                    int64_t batch = node->shape[0], hidden = node->shape[1];
                    double *dg = calloc((size_t)(gates->length ? gates->length : 1), sizeof(double));
                    double *dc = calloc((size_t)(previous->length ? previous->length : 1), sizeof(double));
                    if (!dg || !dc) k_fatal("Out of memory");
                    for (int64_t row = 0; row < batch; ++row)
                        for (int64_t column = 0; column < hidden; ++column) {
                            const double *gate = gates->items + row * hidden * 4;
                            double input = 1.0 / (1.0 + exp(-gate[column]));
                            double forget = 1.0 / (1.0 + exp(-gate[hidden + column]));
                            double candidate = tanh(gate[hidden * 2 + column]);
                            double dnext = g[row * hidden + column];
                            dg[row * hidden * 4 + column] = dnext * candidate * input * (1.0 - input);
                            dg[row * hidden * 4 + hidden + column] = dnext * previous->items[row * hidden + column] * forget * (1.0 - forget);
                            dg[row * hidden * 4 + hidden * 2 + column] = dnext * input * (1.0 - candidate * candidate);
                            dc[row * hidden + column] = dnext * forget;
                        }
                    k_ad_accumulate(a, gates->rank, gates->shape, dg);
                    k_ad_accumulate(b, previous->rank, previous->shape, dc);
                    free(dg); free(dc);
                }
                break;
            }
            case K_AD_OP_LSTM_HIDDEN: {
                if (a || b) {
                    const KTensor *gates = &op->saved_a;
                    const KTensor *cellValue = &op->saved_b;
                    int64_t batch = node->shape[0], hidden = node->shape[1];
                    double *dg = calloc((size_t)(gates->length ? gates->length : 1), sizeof(double));
                    double *dc = calloc((size_t)(cellValue->length ? cellValue->length : 1), sizeof(double));
                    if (!dg || !dc) k_fatal("Out of memory");
                    for (int64_t row = 0; row < batch; ++row)
                        for (int64_t column = 0; column < hidden; ++column) {
                            const double *gate = gates->items + row * hidden * 4;
                            double output = 1.0 / (1.0 + exp(-gate[hidden * 3 + column]));
                            double squashed = tanh(cellValue->items[row * hidden + column]);
                            double dh = g[row * hidden + column];
                            dg[row * hidden * 4 + hidden * 3 + column] = dh * squashed * output * (1.0 - output);
                            dc[row * hidden + column] = dh * output * (1.0 - squashed * squashed);
                        }
                    k_ad_accumulate(a, gates->rank, gates->shape, dg);
                    k_ad_accumulate(b, cellValue->rank, cellValue->shape, dc);
                    free(dg); free(dc);
                }
                break;
            }
            case K_AD_OP_SOFTMAX:
            case K_AD_OP_LOGSOFTMAX: {
                if (a) {
                    int64_t axis = op->axis;
                    int64_t inner = 1; for (int64_t d = axis + 1; d < node->rank; ++d) inner *= node->shape[d];
                    int64_t outer = 1; for (int64_t d = 0; d < axis; ++d) outer *= node->shape[d];
                    int64_t width = node->shape[axis];
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t o = 0; o < outer; ++o) {
                        for (int64_t i = 0; i < inner; ++i) {
                            double sum_gy = 0.0, sum_g = 0.0, sum_p = 0.0;
                            for (int64_t k = 0; k < width; ++k) {
                                double y = node->data[(o * width + k) * inner + i];
                                double gg = g[(o * width + k) * inner + i];
                                if (op->kind == K_AD_OP_SOFTMAX) sum_gy += gg * y;
                                if (op->kind == K_AD_OP_LOGSOFTMAX) { sum_g += gg; sum_p += exp(y); }
                            }
                            for (int64_t k = 0; k < width; ++k) {
                                int64_t at = (o * width + k) * inner + i;
                                if (op->kind == K_AD_OP_SOFTMAX) {
                                    contrib[at] = node->data[at] * (g[at] - sum_gy);
                                } else {
                                    contrib[at] = g[at] - exp(node->data[at]) * sum_g;
                                }
                            }
                        }
                    }
                    k_ad_accumulate(a, a->rank, a->shape, contrib);
                    free(contrib);
                }
                break;
            }
            case K_AD_OP_CROSS_ENTROPY:
                if (a && b) {
                    int64_t rows = a->shape[0], classes = a->shape[1];
                    double *ga = calloc((size_t)a->length, sizeof(double));
                    if (!ga) k_fatal("Out of memory");
                    double divisor = (double)(rows ? rows : 1);
                    for (int64_t row = 0; row < rows; ++row) {
                        double maximum = -INFINITY;
                        for (int64_t c = 0; c < classes; ++c) if (op->saved_a.items[row * classes + c] > maximum) maximum = op->saved_a.items[row * classes + c];
                        double sum = 0.0;
                        for (int64_t c = 0; c < classes; ++c) sum += exp(op->saved_a.items[row * classes + c] - maximum);
                        int64_t label = (int64_t)op->saved_b.items[row];
                        for (int64_t c = 0; c < classes; ++c) {
                            double probability = exp(op->saved_a.items[row * classes + c] - maximum) / sum;
                            ga[row * classes + c] = (probability - (c == label ? 1.0 : 0.0)) * g[0] / divisor;
                        }
                    }
                    k_ad_accumulate(a, a->rank, a->shape, ga);
                    free(ga);
                }
                break;
            case K_AD_OP_NLL:
                if (a && b) {
                    int64_t rows = a->shape[0], classes = a->shape[1];
                    double *ga = calloc((size_t)a->length, sizeof(double));
                    if (!ga) k_fatal("Out of memory");
                    double divisor = (double)(rows ? rows : 1);
                    for (int64_t row = 0; row < rows; ++row) {
                        int64_t label = (int64_t)op->saved_b.items[row];
                        if (label >= 0 && label < classes) ga[row * classes + label] = -g[0] / divisor;
                    }
                    k_ad_accumulate(a, a->rank, a->shape, ga);
                    free(ga);
                }
                break;
            case K_AD_OP_BCE:
                if (a && b) {
                    double *ga = calloc((size_t)n, sizeof(double));
                    if (!ga) k_fatal("Out of memory");
                    int64_t ai = 0, bi = 0;
                    for (int64_t flat = 0; flat < node->length; ++flat) {
                        int64_t rest = flat;
                        // node is a scalar for BCE; recompute from the broadcast plan.
                        (void)rest;
                        double x = op->saved_a.items[ai];
                        double y = op->saved_b.items[bi];
                        (void)x; (void)y;
                    }
                    // The broadcast is saved in flat form by k_ad_binary_cross_entropy_with_logits.
                    int64_t total = op->saved_a.length;
                    for (int64_t i = 0; i < total; ++i) {
                        double x = op->saved_a.items[i];
                        double y = op->saved_b.items[i % (op->saved_b.length ? op->saved_b.length : 1)];
                        double sigmoid = 1.0 / (1.0 + exp(-x));
                        ga[i] = (sigmoid - y) * g[0] / (double)(total ? total : 1);
                    }
                    k_ad_accumulate(a, node->rank, node->shape, ga);
                    free(ga);
                }
                break;
            case K_AD_OP_POW:
                if (a && b) {
                    double *ga = malloc((size_t)n * sizeof(double));
                    double *gb = malloc((size_t)n * sizeof(double));
                    if (!ga || !gb) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) {
                        int64_t ai = k_ad_index_into(a, node->rank, node->shape, i);
                        int64_t bi = k_ad_index_into(b, node->rank, node->shape, i);
                        double av = op->saved_a.items[ai];
                        double bv = op->saved_b.items[bi];
                        ga[i] = g[i] * bv * pow(av, bv - 1.0);
                        gb[i] = g[i] * pow(av, bv) * log(av);
                    }
                    k_ad_accumulate(a, node->rank, node->shape, ga);
                    k_ad_accumulate(b, node->rank, node->shape, gb);
                    free(ga); free(gb);
                } else if (a) {
                    double exponent = op->scalar;
                    double *contrib = calloc((size_t)n, sizeof(double));
                    if (!contrib) k_fatal("Out of memory");
                    for (int64_t i = 0; i < n; ++i) contrib[i] = g[i] * exponent * pow(op->saved_a.items[i], exponent - 1.0);
                    k_ad_accumulate(a, node->rank, node->shape, contrib);
                    free(contrib);
                }
                break;
            case K_AD_OP_CONV2D:
                if (a && b) {
                    int64_t n = a->shape[0], c = a->shape[1], h = a->shape[2], width = a->shape[3];
                    int64_t o = b->shape[0], cpg = b->shape[1], kh = b->shape[2], kw = b->shape[3];
                    int64_t out_h = node->shape[2], out_w = node->shape[3];
                    int64_t groups = op->groups ? op->groups : 1;
                    int64_t ops = o / groups, plane = out_h * out_w, depth = cpg * kh * kw;
                    double *gx = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    double *gw = calloc((size_t)(b->length ? b->length : 1), sizeof(double));
                    double *gb = calloc((size_t)(o ? o : 1), sizeof(double));
                    double *col = malloc((size_t)(depth ? depth : 1) * (size_t)(plane ? plane : 1) * sizeof(double));
                    double *gcol = malloc((size_t)(depth ? depth : 1) * (size_t)(plane ? plane : 1) * sizeof(double));
                    if (!gx || !gw || !gb || !col || !gcol) k_fatal("Out of memory");
                    for (int64_t ni = 0; ni < n; ++ni) {
                        for (int64_t group = 0; group < groups; ++group) {
                            const double *image = a->data + (ni * c + group * cpg) * h * width;
                            k_ad_im2col(image, 0, h, width, cpg, kh, kw, out_h, out_w, op->stride_h, op->pad_h, op->dilation_h, col);
                            const double *filters = op->saved_b.items + group * ops * depth;
                            const double *gout = g + (ni * o + group * ops) * plane;
                            double *gw_group = gw + group * ops * depth;
                            if (!k_blas_dgemm(depth, ops, plane, filters, true, gout, false, gcol)) {
                                for (int64_t k = 0; k < depth; ++k)
                                    for (int64_t p = 0; p < plane; ++p) {
                                        double sum = 0.0;
                                        for (int64_t oi = 0; oi < ops; ++oi) sum += filters[oi * depth + k] * gout[oi * plane + p];
                                        gcol[k * plane + p] = sum;
                                    }
                            }
                            for (int64_t ci = 0; ci < cpg; ++ci)
                                for (int64_t ky = 0; ky < kh; ++ky)
                                    for (int64_t kx = 0; kx < kw; ++kx) {
                                        int64_t k = (ci * kh + ky) * kw + kx;
                                        int64_t channel = group * cpg + ci;
                                        for (int64_t oy = 0; oy < out_h; ++oy) {
                                            int64_t iy = oy * op->stride_h - op->pad_h + ky * op->dilation_h;
                                            if (iy < 0 || iy >= h) continue;
                                            for (int64_t ox = 0; ox < out_w; ++ox) {
                                                int64_t ix = ox * op->stride_w - op->pad_w + kx * op->dilation_w;
                                                if (ix < 0 || ix >= width) continue;
                                                gx[((ni * c + channel) * h + iy) * width + ix] += gcol[k * plane + oy * out_w + ox];
                                            }
                                        }
                                    }
                            if (!k_blas_dgemm(ops, plane, depth, gout, false, col, true, gw_group)) {
                                for (int64_t oi = 0; oi < ops; ++oi)
                                    for (int64_t k = 0; k < depth; ++k) {
                                        double sum = 0.0;
                                        for (int64_t p = 0; p < plane; ++p) sum += gout[oi * plane + p] * col[k * plane + p];
                                        gw_group[oi * depth + k] += sum;
                                    }
                            }
                            for (int64_t oi = 0; oi < ops; ++oi) {
                                double sum = 0.0;
                                for (int64_t p = 0; p < plane; ++p) sum += gout[oi * plane + p];
                                gb[group * ops + oi] += sum;
                            }
                        }
                    }
                    free(col); free(gcol);
                    k_ad_accumulate(a, a->rank, a->shape, gx);
                    k_ad_accumulate(b, b->rank, b->shape, gw);
                    if (op->c) k_ad_accumulate(op->c, 1, (int64_t[]){o}, gb);
                    free(gx); free(gw); free(gb);
                }
                break;
            case K_AD_OP_MAXPOOL2D:
            case K_AD_OP_ADAPTIVE_AVGPOOL2D:
                if (a) k_ad_adaptive_avg_pool2d_grad(a, g, (int64_t)op->scalar, (int64_t)op->scalar2, loc);
                break;
            case K_AD_OP_AVGPOOL2D:
                if (a) {
                    int64_t n = a->shape[0], c = a->shape[1], h = a->shape[2], width = a->shape[3];
                    int64_t out_h = node->shape[2], out_w = node->shape[3];
                    int64_t kernel = (int64_t)op->scalar;
                    double *gx = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    if (!gx) k_fatal("Out of memory");
                    for (int64_t ni = 0; ni < n; ++ni)
                        for (int64_t ci = 0; ci < c; ++ci)
                            for (int64_t oy = 0; oy < out_h; ++oy)
                                for (int64_t ox = 0; ox < out_w; ++ox) {
                                    int64_t out_index = ((ni * c + ci) * out_h + oy) * out_w + ox;
                                    if (op->kind == K_AD_OP_MAXPOOL2D && op->indices) {
                                        gx[op->indices[out_index]] += g[out_index];
                                    } else {
                                        int64_t count = 0;
                                        for (int64_t ky = 0; ky < kernel; ++ky) {
                                            int64_t iy = oy * op->stride_h - op->pad_h + ky;
                                            for (int64_t kx = 0; kx < kernel; ++kx) {
                                                int64_t ix = ox * op->stride_w - op->pad_w + kx;
                                                if (iy >= 0 && iy < h && ix >= 0 && ix < width) count += 1;
                                            }
                                        }
                                        if (count == 0) continue;
                                        for (int64_t ky = 0; ky < kernel; ++ky) {
                                            int64_t iy = oy * op->stride_h - op->pad_h + ky;
                                            if (iy < 0 || iy >= h) continue;
                                            for (int64_t kx = 0; kx < kernel; ++kx) {
                                                int64_t ix = ox * op->stride_w - op->pad_w + kx;
                                                if (ix < 0 || ix >= width) continue;
                                                gx[((ni * c + ci) * h + iy) * width + ix] += g[out_index] / (double)count;
                                            }
                                        }
                                    }
                                }
                    k_ad_accumulate(a, a->rank, a->shape, gx);
                    free(gx);
                }
                break;
            case K_AD_OP_BATCHNORM:
                if (a) {
                    int64_t channels = op->normalized;
                    int64_t per_group = 1;
                    for (int64_t d = 2; d < a->rank; ++d) per_group *= a->shape[d];
                    int64_t outer = a->length / (channels * (per_group ? per_group : 1));
                    double *gx = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    double *gw = calloc((size_t)(channels ? channels : 1), sizeof(double));
                    double *gb = calloc((size_t)(channels ? channels : 1), sizeof(double));
                    if (!gx || !gw || !gb) k_fatal("Out of memory");
                    for (int64_t c = 0; c < channels; ++c) {
                        double sum = 0.0, sum_xhat = 0.0;
                        for (int64_t o = 0; o < outer; ++o)
                            for (int64_t i = 0; i < per_group; ++i) {
                                int64_t at = (o * channels + c) * per_group + i;
                                sum += g[at];
                                sum_xhat += g[at] * op->xhat[at];
                            }
                        double gamma = b ? b->data[c] : 1.0;
                        double count = (double)(outer * per_group);
                        for (int64_t o = 0; o < outer; ++o)
                            for (int64_t i = 0; i < per_group; ++i) {
                                int64_t at = (o * channels + c) * per_group + i;
                                gx[at] = gamma * op->invstd[c] / (count ? count : 1.0) * (count * g[at] - sum - op->xhat[at] * sum_xhat);
                                gw[c] += g[at] * op->xhat[at];
                                gb[c] += g[at];
                            }
                    }
                    k_ad_accumulate(a, a->rank, a->shape, gx);
                    if (b) k_ad_accumulate(b, b->rank, b->shape, gw);
                    if (op->c) k_ad_accumulate(op->c, op->c->rank, op->c->shape, gb);
                    free(gx); free(gw); free(gb);
                }
                break;
            case K_AD_OP_LAYERNORM:
                if (a) {
                    int64_t width = op->normalized;
                    int64_t rows = width ? a->length / width : 0;
                    double *gx = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    double *gw = calloc((size_t)(width ? width : 1), sizeof(double));
                    double *gb = calloc((size_t)(width ? width : 1), sizeof(double));
                    if (!gx || !gw || !gb) k_fatal("Out of memory");
                    for (int64_t row = 0; row < rows; ++row) {
                        double sum = 0.0, sum_xhat = 0.0;
                        for (int64_t i = 0; i < width; ++i) {
                            int64_t at = row * width + i;
                            sum += g[at];
                            sum_xhat += g[at] * op->xhat[at];
                        }
                        for (int64_t i = 0; i < width; ++i) {
                            int64_t at = row * width + i;
                            double gamma = b ? b->data[i] : 1.0;
                            gx[at] = gamma * op->invstd[row] / (double)(width ? width : 1) * ((double)width * g[at] - sum - op->xhat[at] * sum_xhat);
                            gw[i] += g[at] * op->xhat[at];
                            gb[i] += g[at];
                        }
                    }
                    k_ad_accumulate(a, a->rank, a->shape, gx);
                    if (b) k_ad_accumulate(b, b->rank, b->shape, gw);
                    if (op->c) k_ad_accumulate(op->c, op->c->rank, op->c->shape, gb);
                    free(gx); free(gw); free(gb);
                }
                break;
            case K_AD_OP_EMBEDDING:
                if (a && b) {
                    int64_t dimension = a->shape[1];
                    double *gw = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    if (!gw) k_fatal("Out of memory");
                    for (int64_t i = 0; i < b->length; ++i) {
                        int64_t index = (int64_t)op->saved_b.items[i];
                        if (index < 0 || index >= a->shape[0]) continue;
                        for (int64_t d = 0; d < dimension; ++d) gw[index * dimension + d] += g[i * dimension + d];
                    }
                    k_ad_accumulate(a, a->rank, a->shape, gw);
                    free(gw);
                }
                break;
            case K_AD_OP_SLICE:
                if (a) {
                    int64_t axis = op->axis;
                    int64_t start = (int64_t)op->scalar;
                    int64_t end = (int64_t)op->scalar2;
                    int64_t inner = 1;
                    for (int64_t d = axis + 1; d < a->rank; ++d) inner *= a->shape[d];
                    int64_t outer = 1;
                    for (int64_t d = 0; d < axis; ++d) outer *= a->shape[d];
                    double *gx = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    if (!gx) k_fatal("Out of memory");
                    for (int64_t o = 0; o < outer; ++o) {
                        int64_t target = (o * a->shape[axis] + start) * inner;
                        int64_t source = o * (end - start) * inner;
                        memcpy(gx + target, g + source, (size_t)(end - start) * inner * sizeof(double));
                    }
                    k_ad_accumulate(a, a->rank, a->shape, gx);
                    free(gx);
                }
                break;
            case K_AD_OP_CAT:
                if (a && b) {
                    int64_t axis = op->axis;
                    int64_t inner = 1;
                    for (int64_t d = axis + 1; d < a->rank; ++d) inner *= a->shape[d];
                    int64_t outer = 1;
                    for (int64_t d = 0; d < axis; ++d) outer *= a->shape[d];
                    int64_t left_block = a->shape[axis] * inner;
                    int64_t right_block = b->shape[axis] * inner;
                    double *ga = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    double *gb = calloc((size_t)(b->length ? b->length : 1), sizeof(double));
                    if (!ga || !gb) k_fatal("Out of memory");
                    for (int64_t o = 0; o < outer; ++o) {
                        memcpy(ga + o * left_block, g + o * (left_block + right_block), (size_t)left_block * sizeof(double));
                        memcpy(gb + o * right_block, g + o * (left_block + right_block) + left_block, (size_t)right_block * sizeof(double));
                    }
                    k_ad_accumulate(a, a->rank, a->shape, ga);
                    k_ad_accumulate(b, b->rank, b->shape, gb);
                    free(ga); free(gb);
                }
                break;
            case K_AD_OP_INDEX:
                if (a && op->indices) {
                    int64_t count = op->normalized;
                    double *gx = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    if (!gx) k_fatal("Out of memory");
                    int64_t stride = 1;
                    int64_t base = 0;
                    for (int64_t d = a->rank; d-- > 0;) {
                        if (d < count) base += op->indices[d] * stride;
                        stride *= a->shape[d];
                    }
                    for (int64_t i = 0; i < node->length; ++i) gx[base + i] += g[i];
                    k_ad_accumulate(a, a->rank, a->shape, gx);
                    free(gx);
                }
                break;
            case K_AD_OP_BMM:
                if (a && b) {
                    int64_t batches = a->shape[0], rows = a->shape[1], inner = a->shape[2], cols = b->shape[2];
                    double *ga = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
                    double *gb = calloc((size_t)(b->length ? b->length : 1), sizeof(double));
                    if (!ga || !gb) k_fatal("Out of memory");
                    for (int64_t batch = 0; batch < batches; ++batch) {
                        bool computed = k_blas_dgemm(rows, cols, inner, g + batch * rows * cols, false,
                                op->saved_b.items + batch * inner * cols, true, ga + batch * rows * inner)
                            && k_blas_dgemm(inner, rows, cols, op->saved_a.items + batch * rows * inner, true,
                                g + batch * rows * cols, false, gb + batch * inner * cols);
                        if (computed) continue;
                        for (int64_t i = 0; i < rows; ++i)
                            for (int64_t k = 0; k < inner; ++k) {
                                double sum = 0.0;
                                for (int64_t j = 0; j < cols; ++j) sum += g[(batch * rows + i) * cols + j] * op->saved_b.items[(batch * inner + k) * cols + j];
                                ga[(batch * rows + i) * inner + k] = sum;
                            }
                        for (int64_t k = 0; k < inner; ++k)
                            for (int64_t j = 0; j < cols; ++j) {
                                double sum = 0.0;
                                for (int64_t i = 0; i < rows; ++i) sum += op->saved_a.items[(batch * rows + i) * inner + k] * g[(batch * rows + i) * cols + j];
                                gb[(batch * inner + k) * cols + j] = sum;
                            }
                    }
                    k_ad_accumulate(a, a->rank, a->shape, ga);
                    k_ad_accumulate(b, b->rank, b->shape, gb);
                    free(ga); free(gb);
                }
                break;
            default:
                break;
        }
        if (!node->is_leaf && !node->retain_grad && node != root.node) k_ad_grad_reset(node);
        if (!retain_graph && node != root.node && node->op) {
            KAdOp *saved = node->op;
            node->op = NULL;
            k_ad_op_drop(saved);
        }
    }
    for (size_t i = 0; i < length; ++i) {
        if (order[i]->grad) {
        }
        k_ad_node_release(order[i]);
    }
    free(order);
    k_ad_set_free(&seen);
}

static void k_ad_zero_grad(KVariable value) {
    if (!value.node) return;
    k_ad_grad_reset(value.node);
}

static void k_ad_set_requires_grad(KVariable value, bool required, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable is empty");
    if (!value.node->is_leaf && required) k_ad_trap(loc, "Only a leaf Variable can start requiring gradients");
    value.node->requires_grad = required;
    value.node->grad_enabled = required;
}

static void k_ad_set_grad_enabled(bool enabled) { k_ad_grad_enabled = enabled; }
static bool k_ad_get_grad_enabled(void) { return k_ad_grad_enabled; }

static KVariable k_ad_detach(KVariable value) {
    if (!value.node) return k_variable_empty();
    KVariable result = k_ad_zeros(value.node->rank, value.node->shape, 0.0, false, value.node->dtype, (KLocation){0});
    memcpy(result.node->data, value.node->data, (size_t)value.node->length * sizeof(double));
    return result;
}

static KVariable k_ad_clone(KVariable value, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.clone needs a value");
    KVariable result = k_ad_zeros(value.node->rank, value.node->shape, 0.0, false, value.node->dtype, loc);
    k_ad_set_op(result, K_AD_OP_CLONE, value, k_variable_empty());
    memcpy(result.node->data, value.node->data, (size_t)value.node->length * sizeof(double));
    return result;
}

static KVariable k_ad_cast(KVariable value, const char *name, KLocation loc) {
    KAdDtype dtype = K_AD_F64;
    if (!strcmp(name, "float32")) dtype = K_AD_F32;
    else if (!strcmp(name, "float16")) dtype = K_AD_F16;
    else if (!strcmp(name, "bfloat16")) dtype = K_AD_BF16;
    else if (strcmp(name, "float64") && strcmp(name, "double")) k_ad_trap(loc, "Unknown Variable dtype; use float64, float32, float16 or bfloat16");
    KVariable result = k_ad_zeros(value.node->rank, value.node->shape, 0.0, false, dtype, loc);
    for (int64_t i = 0; i < value.node->length; ++i) result.node->data[i] = k_ad_round_dtype(value.node->data[i], dtype);
    return result;
}

static KVariable k_ad_to_device(KVariable value, const char *name, KLocation loc) {
    if (!strcmp(name, "cpu")) return k_variable_copy(value);
    if (!strcmp(name, "mps")) k_ad_trap(loc, "The MPS backend is not available in this build; use cpu");
    if (!strcmp(name, "cuda")) k_ad_trap(loc, "The CUDA backend is not available in this build; use cpu");
    k_ad_trap(loc, "Unknown Variable device; use cpu");
    return k_variable_copy(value);
}

static void k_ad_seed(int64_t seed) { k_ad_rng_state = (uint64_t)seed + UINT64_C(0x9e3779b97f4a7c15); }

// ---------------------------------------------------------------- additional kernels

static void k_ad_fill_shape(KVariable value, int64_t *out) {
    if (!value.node || !out) return;
    for (int64_t d = 0; d < value.node->rank; ++d) out[d] = value.node->shape[d];
}

static KVariable k_ad_eye(int64_t n, bool requires_grad, KLocation loc) {
    int64_t shape[2] = {n, n};
    KVariable result = k_ad_zeros(2, shape, 0.0, requires_grad, K_AD_F64, loc);
    for (int64_t i = 0; i < n; ++i) result.node->data[i * n + i] = 1.0;
    return result;
}

static KVariable k_ad_pow(KVariable left, KVariable right, KLocation loc) {
    if (!left.node || !right.node) k_ad_trap(loc, "Variable.power needs two values");
    KAdBroadcast plan = k_ad_broadcast_plan(left.node, right.node, loc);
    KVariable result = k_ad_zeros(plan.rank, plan.shape, 0.0, false, left.node->dtype, loc);
    k_ad_set_op(result, K_AD_OP_POW, left, right);
    if (result.node->requires_grad && result.node->op) {
        result.node->op->scalar2 = 1.0;
        k_ad_op_save(result.node->op, false, left.node);
        k_ad_op_save(result.node->op, true, right.node);
    }
    int64_t coordinates[64];
    for (int64_t flat = 0; flat < plan.length; ++flat) {
        int64_t rest = flat, ai = 0, bi = 0;
        for (int64_t d = plan.rank; d-- > 0;) {
            coordinates[d] = rest % plan.shape[d];
            rest /= plan.shape[d];
            ai += coordinates[d] * plan.a_offset[d];
            bi += coordinates[d] * plan.b_offset[d];
        }
        result.node->data[flat] = k_ad_round_dtype(pow(left.node->data[ai], right.node->data[bi]), result.node->dtype);
    }
    k_ad_broadcast_free(&plan);
    return result;
}

static KVariable k_ad_pow_scalar(KVariable value, double exponent, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.power needs a value");
    KAdNode *a = value.node;
    KVariable result = k_ad_zeros(a->rank, a->shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_POW, value, k_variable_empty());
    if (result.node->requires_grad && result.node->op) {
        result.node->op->scalar = exponent;
        result.node->op->scalar2 = 1.0;
        k_ad_op_save(result.node->op, false, a);
    }
    for (int64_t i = 0; i < a->length; ++i) result.node->data[i] = k_ad_round_dtype(pow(a->data[i], exponent), result.node->dtype);
    return result;
}

static KVariable k_ad_flatten_start(KVariable value, int64_t start, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.flatten needs a value");
    if (start < 0) start += value.node->rank;
    if (start < 0 || start > value.node->rank) k_ad_trap(loc, "Variable.flatten start dimension out of range");
    if (start == value.node->rank) return k_variable_copy(value);
    int64_t shape[64];
    int64_t rank = start + 1;
    int64_t trailing = 1;
    for (int64_t d = 0; d < start; ++d) shape[d] = value.node->shape[d];
    for (int64_t d = start; d < value.node->rank; ++d) trailing *= value.node->shape[d];
    shape[start] = trailing;
    return k_ad_reshape(value, rank, shape, loc);
}

static KVariable k_ad_flatten(KVariable value, KLocation loc) {
    return k_ad_flatten_start(value, 0, loc);
}

static KVariable k_ad_arg_reduce(KVariable value, bool maximum, int64_t axis, bool keepdim, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.arg reduction needs a value");
    KAdNode *a = value.node;
    bool total = axis <= -1000000;
    if (!total) {
        if (axis < 0) axis += a->rank;
        if (axis < 0 || axis >= a->rank) k_ad_trap(loc, "Variable axis out of range");
    }
    int64_t rank = total ? (keepdim ? a->rank : 0) : (keepdim ? a->rank : a->rank - 1);
    int64_t shape[64];
    int64_t length = 1;
    if (total) {
        if (keepdim) for (int64_t d = 0; d < a->rank; ++d) shape[d] = 1;
        length = 1;
    } else if (keepdim) {
        for (int64_t d = 0; d < a->rank; ++d) shape[d] = d == axis ? 1 : a->shape[d];
        length = a->length / (a->shape[axis] ? a->shape[axis] : 1);
    } else {
        for (int64_t d = 0; d < rank; ++d) shape[d] = d < axis ? a->shape[d] : a->shape[d + 1];
        length = a->length / (a->shape[axis] ? a->shape[axis] : 1);
    }
    KVariable result = k_ad_zeros(rank, shape, -1.0, false, K_AD_F64, loc);
    (void)length;
    if (total) {
        int64_t best = 0;
        for (int64_t i = 1; i < a->length; ++i) {
            if ((maximum && a->data[i] > a->data[best]) || (!maximum && a->data[i] < a->data[best])) best = i;
        }
        result.node->data[0] = (double)best;
    } else {
        int64_t inner = 1; for (int64_t d = axis + 1; d < a->rank; ++d) inner *= a->shape[d];
        int64_t outer = 1; for (int64_t d = 0; d < axis; ++d) outer *= a->shape[d];
        int64_t width = a->shape[axis];
        for (int64_t o = 0; o < outer; ++o) {
            for (int64_t i = 0; i < inner; ++i) {
                int64_t best = 0;
                for (int64_t k = 1; k < width; ++k) {
                    double candidate = a->data[(o * width + k) * inner + i];
                    double current = a->data[(o * width + best) * inner + i];
                    if ((maximum && candidate > current) || (!maximum && candidate < current)) best = k;
                }
                result.node->data[o * inner + i] = (double)best;
            }
        }
    }
    return result;
}

// ---------------------------------------------------------------- gradient and losses

static KVariable k_ad_grad_variable(KVariable value, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.gradVariable needs a value");
    if (!value.node->grad) {
        return k_ad_zeros(value.node->rank, value.node->shape, 0.0, false, value.node->dtype, loc);
    }
    KVariable result = k_ad_zeros(value.node->rank, value.node->shape, 0.0, false, value.node->dtype, loc);
    memcpy(result.node->data, value.node->grad, (size_t)value.node->length * sizeof(double));
    return result;
}

static KVariable k_ad_softmax(KVariable value, int64_t axis, bool log_mode, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.softmax needs a value");
    KAdNode *a = value.node;
    if (a->rank == 0) k_ad_trap(loc, "Variable.softmax needs at least one dimension");
    if (axis < 0) axis += a->rank;
    if (axis < 0 || axis >= a->rank) k_ad_trap(loc, "Variable.softmax axis out of range");
    KVariable result = k_ad_zeros(a->rank, a->shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, log_mode ? K_AD_OP_LOGSOFTMAX : K_AD_OP_SOFTMAX, value, k_variable_empty());
    if (result.node->requires_grad && result.node->op) {
        result.node->op->axis = axis;
        k_ad_op_save(result.node->op, false, a);
    }
    int64_t inner = 1;
    for (int64_t d = axis + 1; d < a->rank; ++d) inner *= a->shape[d];
    int64_t outer = 1;
    for (int64_t d = 0; d < axis; ++d) outer *= a->shape[d];
    int64_t width = a->shape[axis];
    for (int64_t o = 0; o < outer; ++o) {
        for (int64_t i = 0; i < inner; ++i) {
            double maximum = -INFINITY;
            for (int64_t k = 0; k < width; ++k) {
                double item = a->data[(o * width + k) * inner + i];
                if (item > maximum) maximum = item;
            }
            double sum = 0.0;
            for (int64_t k = 0; k < width; ++k) sum += exp(a->data[(o * width + k) * inner + i] - maximum);
            for (int64_t k = 0; k < width; ++k) {
                double shifted = a->data[(o * width + k) * inner + i] - maximum;
                double item = log_mode ? shifted - log(sum) : exp(shifted) / sum;
                result.node->data[(o * width + k) * inner + i] = k_ad_round_dtype(item, result.node->dtype);
            }
        }
    }
    return result;
}

static KVariable k_ad_cross_entropy(KVariable logits, KVariable target, KLocation loc) {
    if (!logits.node || !target.node) k_ad_trap(loc, "Variable.crossEntropy needs logits and targets");
    KAdNode *a = logits.node;
    KAdNode *b = target.node;
    if (a->rank != 2) k_ad_trap(loc, "Variable.crossEntropy needs rank-2 logits with rows of class scores");
    if (!(b->rank == 1 || (b->rank == 2 && b->shape[1] == 1))) k_ad_trap(loc, "Variable.crossEntropy needs one integer class per row");
    int64_t rows = a->shape[0], classes = a->shape[1];
    if (b->length != rows) k_ad_trap(loc, "Variable.crossEntropy needs one target for each logits row");
    int64_t shape[1] = {1};
    KVariable result = k_ad_zeros(0, shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_CROSS_ENTROPY, logits, target);
    if (result.node->requires_grad && result.node->op) {
        k_ad_op_save(result.node->op, false, a);
        k_ad_op_save(result.node->op, true, b);
    }
    double total = 0.0;
    for (int64_t row = 0; row < rows; ++row) {
        double maximum = -INFINITY;
        for (int64_t c = 0; c < classes; ++c) {
            double item = a->data[row * classes + c];
            if (item > maximum) maximum = item;
        }
        double sum = 0.0;
        for (int64_t c = 0; c < classes; ++c) sum += exp(a->data[row * classes + c] - maximum);
        int64_t label = (int64_t)b->data[row];
        if (label < 0 || label >= classes) k_ad_trap(loc, "Variable.crossEntropy target is outside the class range");
        total += (maximum + log(sum)) - a->data[row * classes + label];
    }
    result.node->data[0] = k_ad_round_dtype(total / (double)(rows ? rows : 1), result.node->dtype);
    return result;
}

static KVariable k_ad_nll_loss(KVariable log_probs, KVariable target, KLocation loc) {
    if (!log_probs.node || !target.node) k_ad_trap(loc, "Variable.nllLoss needs log probabilities and targets");
    KAdNode *a = log_probs.node;
    KAdNode *b = target.node;
    if (a->rank != 2) k_ad_trap(loc, "Variable.nllLoss needs rank-2 log probabilities");
    int64_t rows = a->shape[0], classes = a->shape[1];
    if (!(b->rank == 1 || (b->rank == 2 && b->shape[1] == 1)) || b->length != rows) k_ad_trap(loc, "Variable.nllLoss needs one target for each row");
    int64_t shape[1] = {1};
    KVariable result = k_ad_zeros(0, shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_NLL, log_probs, target);
    if (result.node->requires_grad && result.node->op) {
        k_ad_op_save(result.node->op, false, a);
        k_ad_op_save(result.node->op, true, b);
    }
    double total = 0.0;
    for (int64_t row = 0; row < rows; ++row) {
        int64_t label = (int64_t)b->data[row];
        if (label < 0 || label >= classes) k_ad_trap(loc, "Variable.nllLoss target is outside the class range");
        total -= a->data[row * classes + label];
    }
    result.node->data[0] = k_ad_round_dtype(total / (double)(rows ? rows : 1), result.node->dtype);
    return result;
}

static KVariable k_ad_binary_cross_entropy_with_logits(KVariable logits, KVariable target, KLocation loc) {
    if (!logits.node || !target.node) k_ad_trap(loc, "Variable.binaryCrossEntropy needs logits and targets");
    KAdBroadcast plan = k_ad_broadcast_plan(logits.node, target.node, loc);
    KVariable result = k_ad_zeros(0, plan.shape, 0.0, false, logits.node->dtype, loc);
    k_ad_set_op(result, K_AD_OP_BCE, logits, target);
    if (result.node->requires_grad && result.node->op) {
        k_ad_op_save(result.node->op, false, logits.node);
        k_ad_op_save(result.node->op, true, target.node);
    }
    double total = 0.0;
    int64_t coordinates[64];
    for (int64_t flat = 0; flat < plan.length; ++flat) {
        int64_t rest = flat, ai = 0, bi = 0;
        for (int64_t d = plan.rank; d-- > 0;) {
            coordinates[d] = plan.shape[d] ? rest % plan.shape[d] : 0;
            rest = plan.shape[d] ? rest / plan.shape[d] : 0;
            ai += coordinates[d] * plan.a_offset[d];
            bi += coordinates[d] * plan.b_offset[d];
        }
        double x = logits.node->data[ai], y = target.node->data[bi];
        double loss = fmax(x, 0.0) - x * y + log1p(exp(-fabs(x)));
        total += loss;
    }
    result.node->data[0] = k_ad_round_dtype(total / (double)(plan.length ? plan.length : 1), result.node->dtype);
    k_ad_broadcast_free(&plan);
    return result;
}

static KVariable k_ad_uniform_shape(int64_t rank, const int64_t *shape, double low, double high, bool requires_grad, KLocation loc) {
    return k_ad_uniform(rank, shape, low, high, requires_grad, K_AD_F64, loc);
}

static KVariable k_ad_to(KVariable value, KString name, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.to needs a value");
    if ((name.length == 3 && !strncmp(name.data, "cpu", 3))) return k_variable_copy(value);
    if ((name.length == 3 && !strncmp(name.data, "mps", 3))) k_ad_trap(loc, "The MPS backend is not available in this build; use cpu");
    if ((name.length == 4 && !strncmp(name.data, "cuda", 4))) k_ad_trap(loc, "The CUDA backend is not available in this build; use cpu");
    if ((name.length == 7 && !strncmp(name.data, "float64", 7)) || (name.length == 6 && !strncmp(name.data, "double", 6))) return k_variable_copy(value);
    if (name.length == 7 && !strncmp(name.data, "float32", 7)) {
        KVariable result = k_ad_zeros(value.node->rank, value.node->shape, 0.0, false, K_AD_F32, loc);
        for (int64_t i = 0; i < value.node->length; ++i) result.node->data[i] = k_ad_round_dtype(value.node->data[i], K_AD_F32);
        return result;
    }
    if (name.length == 7 && !strncmp(name.data, "float16", 7)) {
        KVariable result = k_ad_zeros(value.node->rank, value.node->shape, 0.0, false, K_AD_F16, loc);
        for (int64_t i = 0; i < value.node->length; ++i) result.node->data[i] = k_ad_round_dtype(value.node->data[i], K_AD_F16);
        return result;
    }
    if (name.length == 8 && !strncmp(name.data, "bfloat16", 8)) {
        KVariable result = k_ad_zeros(value.node->rank, value.node->shape, 0.0, false, K_AD_BF16, loc);
        for (int64_t i = 0; i < value.node->length; ++i) result.node->data[i] = k_ad_round_dtype(value.node->data[i], K_AD_BF16);
        return result;
    }
    k_ad_trap(loc, "Unknown Variable dtype or device");
    return k_variable_copy(value);
}

static bool k_ad_training = true;
static void k_ad_set_training(bool value) { k_ad_training = value; }
static bool k_ad_is_training(void) { return k_ad_training; }

static KVariable k_ad_normal_tensor(int64_t rank, const int64_t *shape, double mean, double sigma, bool requires_grad, KLocation loc) {
    KVariable result = k_ad_zeros(rank, shape, 0.0, requires_grad, K_AD_F64, loc);
    for (int64_t i = 0; i < result.node->length; ++i) result.node->data[i] = mean + sigma * k_ad_normal_sample();
    return result;
}

static KVariable k_ad_bernoulli(int64_t rank, const int64_t *shape, double probability, bool requires_grad, KLocation loc) {
    KVariable result = k_ad_zeros(rank, shape, 0.0, requires_grad, K_AD_F64, loc);
    for (int64_t i = 0; i < result.node->length; ++i) result.node->data[i] = k_ad_uniform01() < probability ? 1.0 : 0.0;
    return result;
}

static void k_variable_drop_task(void *payload, KDropStack *stack) {
    (void)stack;
    k_variable_drop((KVariable *)payload);
}

// ---------------------------------------------------------------- convolution and pooling

// im2col writes one window plane per kernel position. Windows that reach
// outside the padded input read as zero. The plane holds [K][out_h*out_w].
static void k_ad_im2col(const double *x, int64_t channel, int64_t h, int64_t w,
                        int64_t cpg, int64_t kh, int64_t kw, int64_t out_h, int64_t out_w,
                        int64_t stride, int64_t padding, int64_t dilation, double *col) {
    int64_t plane = out_h * out_w;
    for (int64_t ci = 0; ci < cpg; ++ci)
        for (int64_t ky = 0; ky < kh; ++ky)
            for (int64_t kx = 0; kx < kw; ++kx) {
                double *block = col + (ci * kh + ky) * kw * plane + kx * plane;
                for (int64_t oy = 0; oy < out_h; ++oy) {
                    int64_t iy = oy * stride - padding + ky * dilation;
                    if (iy < 0 || iy >= h) { for (int64_t ox = 0; ox < out_w; ++ox) block[oy * out_w + ox] = 0.0; continue; }
                    const double *row = x + ((channel + ci) * h + iy) * w;
                    for (int64_t ox = 0; ox < out_w; ++ox) {
                        int64_t ix = ox * stride - padding + kx * dilation;
                        block[oy * out_w + ox] = (ix < 0 || ix >= w) ? 0.0 : row[ix];
                    }
                }
            }
}

static KVariable k_ad_conv2d(KVariable input, KVariable weight, KVariable bias, int64_t stride, int64_t padding, int64_t dilation, int64_t groups, KLocation loc) {
    if (!input.node || !weight.node) k_ad_trap(loc, "Conv2d needs an input and a weight");
    KAdNode *x = input.node, *w = weight.node;
    if (x->rank != 4 || w->rank != 4) k_ad_trap(loc, "Conv2d needs 4-D input and weight");
    int64_t n = x->shape[0], c = x->shape[1], h = x->shape[2], width = x->shape[3];
    int64_t o = w->shape[0], per_group = w->shape[1], kh = w->shape[2], kw = w->shape[3];
    if (groups < 1 || c % groups || o % groups || per_group != c / groups) k_ad_trap(loc, "Conv2d channel groups do not match");
    int64_t out_h = (h + 2 * padding - dilation * (kh - 1) - 1) / stride + 1;
    int64_t out_w = (width + 2 * padding - dilation * (kw - 1) - 1) / stride + 1;
    if (out_h <= 0 || out_w <= 0) k_ad_trap(loc, "Conv2d output would be empty");
    int64_t shape[4] = {n, o, out_h, out_w};
    KVariable result = k_ad_zeros(4, shape, 0.0, false, x->dtype, loc);
    result.node->is_leaf = false;
    result.node->requires_grad = k_ad_grad_enabled && (x->requires_grad || w->requires_grad || (bias.node && bias.node->requires_grad));
    if (result.node->requires_grad) {
        KAdOp *op = k_ad_op_new(K_AD_OP_CONV2D, x, w);
        if (bias.node) { op->c = bias.node; k_ad_retain_node(bias.node); }
        op->stride_h = stride; op->stride_w = stride;
        op->pad_h = padding; op->pad_w = padding;
        op->dilation_h = dilation; op->dilation_w = dilation;
        op->groups = groups;
        k_ad_op_save(op, false, x);
        k_ad_op_save(op, true, w);
        result.node->op = op;
    }
    int64_t cpg = c / groups;
    int64_t ops = o / groups, plane = out_h * out_w, depth = cpg * kh * kw;
    double *col = malloc((size_t)(depth ? depth : 1) * (size_t)(plane ? plane : 1) * sizeof(double));
    if (!col) k_fatal("Out of memory");
    for (int64_t ni = 0; ni < n; ++ni) {
        for (int64_t group = 0; group < groups; ++group) {
            const double *image = x->data + (ni * c + group * cpg) * h * width;
            k_ad_im2col(image, 0, h, width, cpg, kh, kw, out_h, out_w, stride, padding, dilation, col);
            const double *filters = w->data + group * ops * depth;
            double *out = result.node->data + (ni * o + group * ops) * plane;
            if (!k_blas_dgemm(ops, depth, plane, filters, false, col, false, out)) {
                // No BLAS: the same product in plain loops.
                for (int64_t oi = 0; oi < ops; ++oi)
                    for (int64_t p = 0; p < plane; ++p) {
                        double sum = 0.0;
                        for (int64_t k = 0; k < depth; ++k) sum += filters[oi * depth + k] * col[k * plane + p];
                        out[oi * plane + p] = sum;
                    }
            }
            for (int64_t oi = 0; oi < ops; ++oi) {
                double offset = bias.node ? bias.node->data[group * ops + oi] : 0.0;
                for (int64_t p = 0; p < plane; ++p) out[oi * plane + p] = k_ad_round_dtype(out[oi * plane + p] + offset, result.node->dtype);
            }
        }
    }
    free(col);
    return result;
}

static KVariable k_ad_pool2d(KVariable input, int64_t kernel, int64_t stride, int64_t padding, bool maximum, KLocation loc) {
    if (!input.node) k_ad_trap(loc, "Pooling needs an input");
    KAdNode *x = input.node;
    if (x->rank != 4) k_ad_trap(loc, "2-D pooling needs 4-D input");
    int64_t n = x->shape[0], c = x->shape[1], h = x->shape[2], width = x->shape[3];
    int64_t out_h = (h + 2 * padding - kernel) / stride + 1;
    int64_t out_w = (width + 2 * padding - kernel) / stride + 1;
    if (out_h <= 0 || out_w <= 0) k_ad_trap(loc, "Pooling output would be empty");
    int64_t shape[4] = {n, c, out_h, out_w};
    KVariable result = k_ad_zeros(4, shape, 0.0, false, x->dtype, loc);
    result.node->is_leaf = false;
    result.node->requires_grad = k_ad_grad_enabled && x->requires_grad;
    if (result.node->requires_grad) {
        KAdOp *op = k_ad_op_new(maximum ? K_AD_OP_MAXPOOL2D : K_AD_OP_AVGPOOL2D, x, NULL);
        op->stride_h = stride; op->stride_w = stride; op->pad_h = padding; op->pad_w = padding;
        op->scalar = (double)kernel;
        k_ad_op_save(op, false, x);
        if (maximum) {
            op->indices = malloc((size_t)(result.node->length ? result.node->length : 1) * sizeof(int64_t));
            if (!op->indices) k_fatal("Out of memory");
        }
        result.node->op = op;
    }
    for (int64_t ni = 0; ni < n; ++ni) {
        for (int64_t ci = 0; ci < c; ++ci) {
            for (int64_t oy = 0; oy < out_h; ++oy) {
                for (int64_t ox = 0; ox < out_w; ++ox) {
                    double best = -INFINITY;
                    int64_t best_index = 0;
                    double sum = 0.0;
                    int64_t count = 0;
                    for (int64_t ky = 0; ky < kernel; ++ky) {
                        int64_t iy = oy * stride - padding + ky;
                        for (int64_t kx = 0; kx < kernel; ++kx) {
                            int64_t ix = ox * stride - padding + kx;
                            if (iy < 0 || iy >= h || ix < 0 || ix >= width) continue;
                            int64_t index = ((ni * c + ci) * h + iy) * width + ix;
                            double item = x->data[index];
                            sum += item;
                            count += 1;
                            if (item > best) { best = item; best_index = index; }
                        }
                    }
                    int64_t out_index = ((ni * c + ci) * out_h + oy) * out_w + ox;
                    result.node->data[out_index] = k_ad_round_dtype(maximum ? best : (count ? sum / (double)count : 0.0), result.node->dtype);
                    if (maximum && result.node->op) result.node->op->indices[out_index] = best_index;
                }
            }
        }
    }
    return result;
}

// ---------------------------------------------------------------- normalization

static KVariable k_ad_batch_norm(KVariable input, KVariable weight, KVariable bias, KVariable running_mean, KVariable running_var, double epsilon, double momentum, bool training, KLocation loc) {
    if (!input.node) k_ad_trap(loc, "BatchNorm needs an input");
    KAdNode *x = input.node;
    if (x->rank < 2) k_ad_trap(loc, "BatchNorm needs at least two dimensions");
    int64_t channels = x->shape[1];
    int64_t per_channel = x->length / (channels ? channels : 1);
    if (weight.node && weight.node->length != channels) k_ad_trap(loc, "BatchNorm weight must have one value per channel");
    if (bias.node && bias.node->length != channels) k_ad_trap(loc, "BatchNorm bias must have one value per channel");
    KVariable result = k_ad_zeros(x->rank, x->shape, 0.0, false, x->dtype, loc);
    result.node->is_leaf = false;
    result.node->requires_grad = k_ad_grad_enabled && (x->requires_grad || (weight.node && weight.node->requires_grad) || (bias.node && bias.node->requires_grad));
    double *mean = calloc((size_t)channels, sizeof(double));
    double *variance = calloc((size_t)channels, sizeof(double));
    double *invstd = calloc((size_t)channels, sizeof(double));
    double *xhat = malloc((size_t)(x->length ? x->length : 1) * sizeof(double));
    if (!mean || !variance || !invstd || !xhat) k_fatal("Out of memory");
    int64_t outer = x->length / (channels * (per_channel / (per_channel ? 1 : 1)) ? channels : 1);
    // The layout is (N, C, ...), so channel c occupies `per_channel` values in
    // each of the `outer` groups.
    int64_t per_group = 1;
    for (int64_t d = 2; d < x->rank; ++d) per_group *= x->shape[d];
    outer = x->length / (channels * per_group);
    (void)per_channel;
    for (int64_t c = 0; c < channels; ++c) {
        double sum = 0.0;
        for (int64_t o = 0; o < outer; ++o)
            for (int64_t i = 0; i < per_group; ++i) sum += x->data[(o * channels + c) * per_group + i];
        int64_t count = outer * per_group;
        mean[c] = sum / (double)(count ? count : 1);
        double variation = 0.0;
        for (int64_t o = 0; o < outer; ++o)
            for (int64_t i = 0; i < per_group; ++i) {
                double difference = x->data[(o * channels + c) * per_group + i] - mean[c];
                variation += difference * difference;
            }
        variance[c] = variation / (double)(count ? count : 1);
        invstd[c] = 1.0 / sqrt(variance[c] + epsilon);
        for (int64_t o = 0; o < outer; ++o)
            for (int64_t i = 0; i < per_group; ++i) {
                int64_t at = (o * channels + c) * per_group + i;
                xhat[at] = (x->data[at] - mean[c]) * invstd[c];
                double gamma = weight.node ? weight.node->data[c] : 1.0;
                double beta = bias.node ? bias.node->data[c] : 0.0;
                result.node->data[at] = k_ad_round_dtype(xhat[at] * gamma + beta, result.node->dtype);
            }
    }
    if (training && running_mean.node && running_var.node && running_mean.node->length == channels && running_var.node->length == channels) {
        int64_t count = outer * per_group;
        double unbiased = count > 1 ? (double)count / (double)(count - 1) : 1.0;
        for (int64_t c = 0; c < channels; ++c) {
            running_mean.node->data[c] = (1.0 - momentum) * running_mean.node->data[c] + momentum * mean[c];
            running_var.node->data[c] = (1.0 - momentum) * running_var.node->data[c] + momentum * variance[c] * unbiased;
        }
        running_mean.node->version += 1;
        running_var.node->version += 1;
    }
    if (result.node->requires_grad) {
        KAdOp *op = k_ad_op_new(K_AD_OP_BATCHNORM, x, weight.node);
        if (bias.node) { op->c = bias.node; k_ad_retain_node(bias.node); }
        op->xhat = xhat;
        op->invstd = invstd;
        op->normalized = channels;
        op->scalar = epsilon;
        result.node->op = op;
    } else {
        free(xhat);
        free(invstd);
    }
    free(mean);
    free(variance);
    return result;
}

static KVariable k_ad_layer_norm(KVariable input, KVariable weight, KVariable bias, double epsilon, KLocation loc) {
    if (!input.node) k_ad_trap(loc, "LayerNorm needs an input");
    KAdNode *x = input.node;
    if (x->rank < 1) k_ad_trap(loc, "LayerNorm needs at least one dimension");
    int64_t width = x->shape[x->rank - 1];
    int64_t rows = width ? x->length / width : 0;
    if (weight.node && weight.node->length != width) k_ad_trap(loc, "LayerNorm weight length does not match the last dimension");
    if (bias.node && bias.node->length != width) k_ad_trap(loc, "LayerNorm bias length does not match the last dimension");
    KVariable result = k_ad_zeros(x->rank, x->shape, 0.0, false, x->dtype, loc);
    result.node->is_leaf = false;
    result.node->requires_grad = k_ad_grad_enabled && (x->requires_grad || (weight.node && weight.node->requires_grad) || (bias.node && bias.node->requires_grad));
    double *invstd = calloc((size_t)(rows ? rows : 1), sizeof(double));
    double *xhat = malloc((size_t)(x->length ? x->length : 1) * sizeof(double));
    if (!invstd || !xhat) k_fatal("Out of memory");
    for (int64_t row = 0; row < rows; ++row) {
        double sum = 0.0;
        for (int64_t i = 0; i < width; ++i) sum += x->data[row * width + i];
        double mean = sum / (double)(width ? width : 1);
        double variance = 0.0;
        for (int64_t i = 0; i < width; ++i) {
            double difference = x->data[row * width + i] - mean;
            variance += difference * difference;
        }
        variance /= (double)(width ? width : 1);
        invstd[row] = 1.0 / sqrt(variance + epsilon);
        for (int64_t i = 0; i < width; ++i) {
            double item = (x->data[row * width + i] - mean) * invstd[row];
            xhat[row * width + i] = item;
            double gamma = weight.node ? weight.node->data[i] : 1.0;
            double beta = bias.node ? bias.node->data[i] : 0.0;
            result.node->data[row * width + i] = k_ad_round_dtype(item * gamma + beta, result.node->dtype);
        }
    }
    if (result.node->requires_grad) {
        KAdOp *op = k_ad_op_new(K_AD_OP_LAYERNORM, x, weight.node);
        if (bias.node) { op->c = bias.node; k_ad_retain_node(bias.node); }
        op->xhat = xhat;
        op->invstd = invstd;
        op->normalized = width;
        op->scalar = epsilon;
        result.node->op = op;
    } else {
        free(xhat); free(invstd);
    }
    return result;
}

// ---------------------------------------------------------------- embedding and batched matmul

static KVariable k_ad_embedding(KVariable weight, KVariable indices, KLocation loc) {
    if (!weight.node || !indices.node) k_ad_trap(loc, "Embedding needs a weight and indices");
    KAdNode *w = weight.node, *t = indices.node;
    if (w->rank != 2) k_ad_trap(loc, "Embedding weight must be rank 2");
    int64_t vocabulary = w->shape[0], dimension = w->shape[1];
    int64_t rank = t->rank + 1;
    int64_t shape[64];
    for (int64_t d = 0; d < t->rank; ++d) shape[d] = t->shape[d];
    shape[t->rank] = dimension;
    KVariable result = k_ad_zeros(rank, shape, 0.0, false, w->dtype, loc);
    result.node->is_leaf = false;
    result.node->requires_grad = k_ad_grad_enabled && w->requires_grad;
    if (result.node->requires_grad) {
        KAdOp *op = k_ad_op_new(K_AD_OP_EMBEDDING, w, t);
        k_ad_op_save(op, true, t);
        result.node->op = op;
    }
    for (int64_t i = 0; i < t->length; ++i) {
        int64_t index = (int64_t)t->data[i];
        if (index < 0 || index >= vocabulary) k_ad_trap(loc, "Embedding index is outside the vocabulary");
        for (int64_t d = 0; d < dimension; ++d) result.node->data[i * dimension + d] = w->data[index * dimension + d];
    }
    return result;
}

static KVariable k_ad_bmm(KVariable left, KVariable right, KLocation loc) {
    if (!left.node || !right.node) k_ad_trap(loc, "Variable.bmm needs two values");
    KAdNode *a = left.node, *b = right.node;
    if (a->rank != 3 || b->rank != 3) k_ad_trap(loc, "Variable.bmm needs rank-3 tensors");
    if (a->shape[0] != b->shape[0] || a->shape[2] != b->shape[1]) k_ad_trap(loc, "Variable.bmm batch or inner dimensions differ");
    int64_t batches = a->shape[0], rows = a->shape[1], inner = a->shape[2], cols = b->shape[2];
    int64_t shape[3] = {batches, rows, cols};
    KVariable result = k_ad_zeros(3, shape, 0.0, false, a->dtype, loc);
    result.node->is_leaf = false;
    result.node->requires_grad = k_ad_grad_enabled && (a->requires_grad || b->requires_grad);
    if (result.node->requires_grad) {
        KAdOp *op = k_ad_op_new(K_AD_OP_BMM, a, b);
        k_ad_op_save(op, false, a);
        k_ad_op_save(op, true, b);
        result.node->op = op;
    }
    for (int64_t batch = 0; batch < batches; ++batch) {
        if (result.node->dtype == K_AD_F64 && k_blas_dgemm(rows, inner, cols,
                a->data + batch * rows * inner, false, b->data + batch * inner * cols, false,
                result.node->data + batch * rows * cols)) continue;
        for (int64_t i = 0; i < rows; ++i)
            for (int64_t j = 0; j < cols; ++j) {
                double sum = 0.0;
                for (int64_t k = 0; k < inner; ++k) {
                    sum += a->data[(batch * rows + i) * inner + k] * b->data[(batch * inner + k) * cols + j];
                }
                result.node->data[(batch * rows + i) * cols + j] = k_ad_round_dtype(sum, result.node->dtype);
            }
    }
    return result;
}

static int64_t k_ad_live_nodes_count(void) { return atomic_load_explicit(&k_ad_live_nodes, memory_order_relaxed); }
static int64_t k_ad_live_bytes_count(void) { return atomic_load_explicit(&k_ad_live_bytes, memory_order_relaxed); }

static void k_ad_write_all(FILE *file, const void *data, size_t size, KLocation loc) {
    if (size && fwrite(data, 1, size, file) != size) k_ad_trap(loc, "Cannot write Variable file");
}

static void k_ad_read_all(FILE *file, void *data, size_t size, KLocation loc) {
    if (size && fread(data, 1, size, file) != size) k_ad_trap(loc, "Variable file is truncated");
}

static void k_ad_save(KVariable value, KString path, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Cannot save an empty Variable");
    if (!path.length || memchr(path.data, 0, path.length)) k_ad_trap(loc, "Variable file path is invalid");
    char *name = malloc((size_t)path.length + 1);
    if (!name) k_fatal("Out of memory");
    memcpy(name, path.data, (size_t)path.length);
    name[path.length] = 0;
    FILE *file = fopen(name, "wb");
    free(name);
    if (!file) k_ad_trap(loc, "Cannot open a Variable file for writing");
    uint32_t magic = UINT32_C(0x4E53454B);
    uint32_t version = 1;
    uint32_t dtype = (uint32_t)value.node->dtype;
    uint32_t flags = value.node->requires_grad ? 1u : 0u;
    int64_t rank = value.node->rank;
    fwrite(&magic, sizeof magic, 1, file);
    fwrite(&version, sizeof version, 1, file);
    fwrite(&dtype, sizeof dtype, 1, file);
    fwrite(&flags, sizeof flags, 1, file);
    fwrite(&rank, sizeof rank, 1, file);
    k_ad_write_all(file, value.node->shape, (size_t)rank * sizeof(int64_t), loc);
    k_ad_write_all(file, value.node->data, (size_t)value.node->length * sizeof(double), loc);
    if (fclose(file) != 0) k_ad_trap(loc, "Cannot close a Variable file");
}

static KVariable k_ad_load(KString path, KLocation loc) {
    if (!path.length || memchr(path.data, 0, path.length)) k_ad_trap(loc, "Variable file path is invalid");
    char *name = malloc((size_t)path.length + 1);
    if (!name) k_fatal("Out of memory");
    memcpy(name, path.data, (size_t)path.length);
    name[path.length] = 0;
    FILE *file = fopen(name, "rb");
    free(name);
    if (!file) k_ad_trap(loc, "Cannot open a Variable file for reading");
    uint32_t magic = 0, version = 0, dtype = 0, flags = 0;
    int64_t rank = 0;
    k_ad_read_all(file, &magic, sizeof magic, loc);
    k_ad_read_all(file, &version, sizeof version, loc);
    k_ad_read_all(file, &dtype, sizeof dtype, loc);
    k_ad_read_all(file, &flags, sizeof flags, loc);
    k_ad_read_all(file, &rank, sizeof rank, loc);
    if (magic != UINT32_C(0x4E53454B) || version != 1 || dtype > K_AD_BF16 || rank < 0 || rank > 64) {
        fclose(file);
        k_ad_trap(loc, "Variable file has an unknown format");
    }
    int64_t shape[64];
    k_ad_read_all(file, shape, (size_t)rank * sizeof(int64_t), loc);
    KVariable result = k_ad_zeros(rank, shape, 0.0, (flags & 1u) != 0, (KAdDtype)dtype, loc);
    k_ad_read_all(file, result.node->data, (size_t)result.node->length * sizeof(double), loc);
    fclose(file);
    return result;
}

// ---------------------------------------------------------------- indexing

static KVariable k_ad_index(KVariable value, int64_t count, const int64_t *indices, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable indexing needs a value");
    KAdNode *a = value.node;
    if (count < 0 || count > a->rank) k_ad_trap(loc, "Variable index count does not match the rank");
    int64_t *resolved = malloc((size_t)(count ? count : 1) * sizeof(int64_t));
    if (!resolved) k_fatal("Out of memory");
    int64_t rank = 0;
    int64_t shape[64];
    for (int64_t d = 0; d < count; ++d) {
        int64_t index = indices[d];
        if (index < 0) index += a->shape[d];
        if (index < 0 || index >= a->shape[d]) {
            free(resolved);
            k_ad_trap(loc, "Variable index out of range");
        }
        resolved[d] = index;
    }
    for (int64_t d = count; d < a->rank; ++d) shape[rank++] = a->shape[d];
    KVariable result = k_ad_zeros(rank, shape, 0.0, false, a->dtype, loc);
    result.node->is_leaf = false;
    result.node->requires_grad = k_ad_grad_enabled && a->requires_grad;
    if (result.node->requires_grad) {
        KAdOp *op = k_ad_op_new(K_AD_OP_INDEX, a, NULL);
        op->indices = resolved;
        op->normalized = count;
        k_ad_op_save(op, false, a);
        result.node->op = op;
    }
    int64_t base = 0;
    int64_t stride = 1;
    for (int64_t d = a->rank; d-- > 0;) {
        if (d < count) base += resolved[d] * stride;
        stride *= a->shape[d];
    }
    memcpy(result.node->data, a->data + base, (size_t)result.node->length * sizeof(double));
    if (!result.node->op) free(resolved);
    return result;
}

static KVariable k_ad_cat(KVariable left, KVariable right, int64_t axis, KLocation loc) {
    if (!left.node || !right.node) k_ad_trap(loc, "Variable.cat needs two values");
    KAdNode *a = left.node, *b = right.node;
    if (a->rank != b->rank || a->rank == 0) k_ad_trap(loc, "Variable.cat needs values with the same rank");
    if (axis < 0) axis += a->rank;
    if (axis < 0 || axis >= a->rank) k_ad_trap(loc, "Variable.cat axis out of range");
    int64_t shape[64];
    for (int64_t d = 0; d < a->rank; ++d) {
        if (d == axis) shape[d] = a->shape[d] + b->shape[d];
        else {
            if (a->shape[d] != b->shape[d]) k_ad_trap(loc, "Variable.cat dimensions differ outside the axis");
            shape[d] = a->shape[d];
        }
    }
    KVariable result = k_ad_zeros(a->rank, shape, 0.0, false, a->dtype, loc);
    result.node->is_leaf = false;
    result.node->requires_grad = k_ad_grad_enabled && (a->requires_grad || b->requires_grad);
    if (result.node->requires_grad) {
        KAdOp *op = k_ad_op_new(K_AD_OP_CAT, a, b);
        op->axis = axis;
        k_ad_op_save(op, false, a);
        k_ad_op_save(op, true, b);
        result.node->op = op;
    }
    int64_t inner = 1;
    for (int64_t d = axis + 1; d < a->rank; ++d) inner *= a->shape[d];
    int64_t outer = 1;
    for (int64_t d = 0; d < axis; ++d) outer *= a->shape[d];
    for (int64_t o = 0; o < outer; ++o) {
        int64_t left_block = a->shape[axis] * inner;
        int64_t right_block = b->shape[axis] * inner;
        memcpy(result.node->data + o * (left_block + right_block), a->data + o * left_block, (size_t)left_block * sizeof(double));
        memcpy(result.node->data + o * (left_block + right_block) + left_block, b->data + o * right_block, (size_t)right_block * sizeof(double));
    }
    return result;
}

static KVariable k_ad_retain_grad(KVariable value, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.retainGrad needs a value");
    value.node->retain_grad = true;
    return k_variable_copy(value);
}

static KVariable k_ad_slice(KVariable value, int64_t axis, int64_t start, int64_t end, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.slice needs a value");
    KAdNode *a = value.node;
    if (axis < 0) axis += a->rank;
    if (axis < 0 || axis >= a->rank) k_ad_trap(loc, "Variable.slice axis out of range");
    int64_t width = a->shape[axis];
    if (start < 0) start += width;
    if (end < 0) end += width;
    if (start < 0) start = 0;
    if (end > width) end = width;
    if (end < start) k_ad_trap(loc, "Variable.slice end is before its start");
    int64_t shape[64];
    for (int64_t d = 0; d < a->rank; ++d) shape[d] = a->shape[d];
    shape[axis] = end - start;
    KVariable result = k_ad_zeros(a->rank, shape, 0.0, false, a->dtype, loc);
    result.node->is_leaf = false;
    result.node->requires_grad = k_ad_grad_enabled && a->requires_grad;
    if (result.node->requires_grad) {
        KAdOp *op = k_ad_op_new(K_AD_OP_SLICE, a, NULL);
        op->axis = axis;
        op->scalar = (double)start;
        op->scalar2 = (double)end;
        k_ad_op_save(op, false, a);
        result.node->op = op;
    }
    int64_t inner = 1;
    for (int64_t d = axis + 1; d < a->rank; ++d) inner *= a->shape[d];
    int64_t outer = 1;
    for (int64_t d = 0; d < axis; ++d) outer *= a->shape[d];
    for (int64_t o = 0; o < outer; ++o) {
        int64_t source = (o * width + start) * inner;
        int64_t target = o * (end - start) * inner;
        memcpy(result.node->data + target, a->data + source, (size_t)(end - start) * inner * sizeof(double));
    }
    return result;
}

// ---------------------------------------------------------------- activation variants

static KVariable k_ad_leaky_relu(KVariable value, double slope, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.leakyRelu needs a value");
    KAdNode *a = value.node;
    KVariable result = k_ad_zeros(a->rank, a->shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_LEAKY_RELU, value, k_variable_empty());
    if (result.node->op) {
        result.node->op->scalar = slope;
        k_ad_op_save(result.node->op, false, a);
    }
    for (int64_t i = 0; i < a->length; ++i) {
        double x = a->data[i];
        result.node->data[i] = k_ad_round_dtype(x >= 0.0 ? x : slope * x, result.node->dtype);
    }
    return result;
}

static KVariable k_ad_elu(KVariable value, double alpha, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.elu needs a value");
    KAdNode *a = value.node;
    KVariable result = k_ad_zeros(a->rank, a->shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_ELU, value, k_variable_empty());
    if (result.node->op) {
        result.node->op->scalar = alpha;
        k_ad_op_save(result.node->op, false, a);
    }
    for (int64_t i = 0; i < a->length; ++i) {
        double x = a->data[i];
        result.node->data[i] = k_ad_round_dtype(x >= 0.0 ? x : alpha * (exp(x) - 1.0), result.node->dtype);
    }
    return result;
}

static KVariable k_ad_gelu(KVariable value, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.gelu needs a value");
    KAdNode *a = value.node;
    KVariable result = k_ad_zeros(a->rank, a->shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_GELU, value, k_variable_empty());
    if (result.node->op) k_ad_op_save(result.node->op, false, a);
    const double root = 0.70710678118654752440; // sqrt(1/2)
    for (int64_t i = 0; i < a->length; ++i) {
        double x = a->data[i];
        double inner = root * (x + 0.044715 * x * x * x);
        result.node->data[i] = k_ad_round_dtype(0.5 * x * (1.0 + tanh(inner)), result.node->dtype);
    }
    return result;
}

static KVariable k_ad_silu(KVariable value, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.silu needs a value");
    KAdNode *a = value.node;
    KVariable result = k_ad_zeros(a->rank, a->shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_SILU, value, k_variable_empty());
    if (result.node->op) k_ad_op_save(result.node->op, false, a);
    for (int64_t i = 0; i < a->length; ++i) {
        double x = a->data[i];
        result.node->data[i] = k_ad_round_dtype(x / (1.0 + exp(-x)), result.node->dtype);
    }
    return result;
}

static KVariable k_ad_softplus(KVariable value, double beta, double threshold, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.softplus needs a value");
    KAdNode *a = value.node;
    KVariable result = k_ad_zeros(a->rank, a->shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_SOFTPLUS, value, k_variable_empty());
    if (result.node->op) {
        result.node->op->scalar = beta;
        result.node->op->scalar2 = threshold;
        k_ad_op_save(result.node->op, false, a);
    }
    for (int64_t i = 0; i < a->length; ++i) {
        double x = a->data[i];
        double y = beta * x > threshold ? x : log1p(exp(beta * x)) / beta;
        result.node->data[i] = k_ad_round_dtype(y, result.node->dtype);
    }
    return result;
}

static KVariable k_ad_clamp(KVariable value, double minimum, double maximum, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.clamp needs a value");
    KAdNode *a = value.node;
    KVariable result = k_ad_zeros(a->rank, a->shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_CLAMP, value, k_variable_empty());
    if (result.node->op) {
        result.node->op->scalar = minimum;
        result.node->op->scalar2 = maximum;
        k_ad_op_save(result.node->op, false, a);
    }
    for (int64_t i = 0; i < a->length; ++i) {
        double x = a->data[i];
        result.node->data[i] = k_ad_round_dtype(x < minimum ? minimum : x > maximum ? maximum : x, result.node->dtype);
    }
    return result;
}

static KVariable k_ad_norm(KVariable value, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.norm needs a value");
    KAdNode *a = value.node;
    double total = 0.0;
    for (int64_t i = 0; i < a->length; ++i) total += a->data[i] * a->data[i];
    int64_t shape[1] = {1};
    KVariable result = k_ad_zeros(0, shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_NORM, value, k_variable_empty());
    if (result.node->op) k_ad_op_save(result.node->op, false, a);
    result.node->data[0] = k_ad_round_dtype(sqrt(total), result.node->dtype);
    return result;
}

static KVariable k_ad_randint(int64_t rank, const int64_t *shape, int64_t low, int64_t high, KLocation loc) {
    if (high <= low) k_ad_trap(loc, "Variable.randint needs a nonempty integer range");
    KVariable result = k_ad_zeros(rank, shape, 0.0, false, K_AD_F64, loc);
    for (int64_t i = 0; i < result.node->length; ++i) {
        double draw = k_ad_uniform01();
        result.node->data[i] = (double)(low + (int64_t)(draw * (double)(high - low)));
    }
    return result;
}

static KVariable k_ad_adaptive_avg_pool2d(KVariable value, int64_t out_h, int64_t out_w, KLocation loc) {
    if (!value.node) k_ad_trap(loc, "Variable.adaptiveAvgPool2d needs a value");
    KAdNode *a = value.node;
    if (a->rank != 4) k_ad_trap(loc, "Variable.adaptiveAvgPool2d expects (N, C, H, W)");
    if (out_h < 1 || out_w < 1) k_ad_trap(loc, "Variable.adaptiveAvgPool2d output size must be positive");
    int64_t shape[4] = {a->shape[0], a->shape[1], out_h, out_w};
    KVariable result = k_ad_zeros(4, shape, 0.0, false, a->dtype, loc);
    k_ad_set_op(result, K_AD_OP_ADAPTIVE_AVGPOOL2D, value, k_variable_empty());
    if (result.node->op) {
        result.node->op->scalar = out_h;
        result.node->op->scalar2 = out_w;
    }
    int64_t H = a->shape[2], W = a->shape[3];
    for (int64_t n = 0; n < a->shape[0]; ++n)
        for (int64_t c = 0; c < a->shape[1]; ++c)
            for (int64_t oy = 0; oy < out_h; ++oy)
                for (int64_t ox = 0; ox < out_w; ++ox) {
                    int64_t y0 = oy * H / out_h, y1 = ((oy + 1) * H + out_h - 1) / out_h;
                    int64_t x0 = ox * W / out_w, x1 = ((ox + 1) * W + out_w - 1) / out_w;
                    double sum = 0.0;
                    for (int64_t y = y0; y < y1; ++y)
                        for (int64_t x = x0; x < x1; ++x)
                            sum += a->data[((n * a->shape[1] + c) * H + y) * W + x];
                    result.node->data[((n * a->shape[1] + c) * out_h + oy) * out_w + ox] =
                        k_ad_round_dtype(sum / (double)((y1 - y0) * (x1 - x0)), result.node->dtype);
                }
    return result;
}

static void k_ad_adaptive_avg_pool2d_grad(struct KAdNode *a, const double *g, int64_t out_h, int64_t out_w, KLocation loc) {
    double *contrib = calloc((size_t)(a->length ? a->length : 1), sizeof(double));
    if (!contrib) k_fatal("Out of memory");
    int64_t H = a->shape[2], W = a->shape[3];
    for (int64_t n = 0; n < a->shape[0]; ++n)
        for (int64_t c = 0; c < a->shape[1]; ++c)
            for (int64_t oy = 0; oy < out_h; ++oy)
                for (int64_t ox = 0; ox < out_w; ++ox) {
                    int64_t y0 = oy * H / out_h, y1 = ((oy + 1) * H + out_h - 1) / out_h;
                    int64_t x0 = ox * W / out_w, x1 = ((ox + 1) * W + out_w - 1) / out_w;
                    double share = g[((n * a->shape[1] + c) * out_h + oy) * out_w + ox] /
                        (double)((y1 - y0) * (x1 - x0));
                    for (int64_t y = y0; y < y1; ++y)
                        for (int64_t x = x0; x < x1; ++x)
                            contrib[((n * a->shape[1] + c) * H + y) * W + x] += share;
                }
    k_ad_accumulate(a, a->rank, a->shape, contrib);
    free(contrib);
}
