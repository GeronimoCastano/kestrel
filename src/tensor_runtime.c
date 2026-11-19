typedef struct {
    int64_t rank, length;
    int64_t *shape;
    double *items;
    bool borrowed;
    bool transposed;
} KTensor;
typedef struct { int kind; int64_t lo, hi; } KTensorIndex;

static KTensor k_tensor_allocate(int64_t rank, const int64_t *shape, bool fill, double value, KLocation loc) {
    if (rank < 0 || (uint64_t)rank > SIZE_MAX / sizeof(int64_t)) k_trap(loc, "Tensor rank is too large");
    bool empty = false;
    for (int64_t d = 0; d < rank; ++d) {
        if (shape[d] < 0) k_trap(loc, "Tensor dimensions cannot be negative");
        if (!shape[d]) empty = true;
    }
    int64_t length = empty ? 0 : 1;
    if (!empty) for (int64_t d = 0; d < rank; ++d) {
        if (shape[d] && length > INT64_MAX / shape[d]) k_trap(loc, "Tensor shape is too large");
        length *= shape[d];
    }
    if ((uint64_t)length > SIZE_MAX / sizeof(double)) k_trap(loc, "Tensor storage is too large");
    KTensor result = {rank, length, malloc((size_t)(rank ? rank : 1) * sizeof(int64_t)), malloc((size_t)(length ? length : 1) * sizeof(double)), false, false};
    if (!result.shape || !result.items) k_fatal("Out of memory");
    if (rank) memcpy(result.shape, shape, (size_t)rank * sizeof(int64_t));
    if (fill) for (int64_t i = 0; i < length; ++i) result.items[i] = value;
    return result;
}
static KTensor k_tensor_new(int64_t rank, const int64_t *shape, double fill, KLocation loc) {
    return k_tensor_allocate(rank, shape, true, fill, loc);
}
// For results whose every element is written before the value is published.
static KTensor k_tensor_new_empty(int64_t rank, const int64_t *shape, KLocation loc) {
    return k_tensor_allocate(rank, shape, false, 0.0, loc);
}
static void k_tensor_drop(KTensor *value) {
    if (!value->borrowed) { free(value->shape); free(value->items); }
    *value = (KTensor){0};
}
static int64_t k_tensor_dimension(const KTensor *value, int64_t dimension) {
    return value->transposed ? value->shape[value->rank - 1 - dimension] : value->shape[dimension];
}
static int64_t k_tensor_stride(const KTensor *value, int64_t dimension) {
    int64_t source = value->transposed ? value->rank - 1 - dimension : dimension;
    int64_t stride = 1;
    for (int64_t d = source + 1; d < value->rank; ++d) stride *= value->shape[d];
    return stride;
}
static double k_tensor_flat(const KTensor *value, int64_t flat) {
    if (!value->transposed) return value->items[flat];
    int64_t offset = 0;
    for (int64_t d = value->rank; d-- > 0;) {
        int64_t size = k_tensor_dimension(value, d);
        offset += (flat % size) * k_tensor_stride(value, d);
        flat /= size;
    }
    return value->items[offset];
}
static KTensor k_tensor_transpose_view(const KTensor *value) {
    KTensor result = *value;
    result.borrowed = true;
    result.transposed = !value->transposed;
    return result;
}
static KTensor k_tensor_new_like(const KTensor *value, double fill, KLocation loc) {
    int64_t rank = value->rank;
    int64_t *shape = malloc((size_t)(rank ? rank : 1) * sizeof(int64_t));
    if (!shape) k_fatal("Out of memory");
    for (int64_t d = 0; d < rank; ++d) shape[d] = k_tensor_dimension(value, d);
    KTensor result = k_tensor_new(rank, shape, fill, loc);
    free(shape);
    return result;
}
static KTensor k_tensor_new_like_empty(const KTensor *value, KLocation loc) {
    int64_t rank = value->rank;
    int64_t *shape = malloc((size_t)(rank ? rank : 1) * sizeof(int64_t));
    if (!shape) k_fatal("Out of memory");
    for (int64_t d = 0; d < rank; ++d) shape[d] = k_tensor_dimension(value, d);
    KTensor result = k_tensor_new_empty(rank, shape, loc);
    free(shape);
    return result;
}
static KTensor k_tensor_copy(const KTensor *value, KLocation loc) {
    KTensor result = k_tensor_new_like_empty(value, loc);
    if (!value->transposed) memcpy(result.items, value->items, (size_t)value->length * sizeof(double));
    else for (int64_t i = 0; i < value->length; ++i) result.items[i] = k_tensor_flat(value, i);
    return result;
}
static KTensor k_tensor_contiguous(const KTensor *value, KLocation loc) {
    if (!value->transposed) {
        KTensor result = *value;
        result.borrowed = true;
        return result;
    }
    return k_tensor_copy(value, loc);
}
static bool k_tensor_same_shape(const KTensor *a, const KTensor *b) {
    if (a->rank != b->rank) return false;
    for (int64_t d = 0; d < a->rank; ++d) if (k_tensor_dimension(a, d) != k_tensor_dimension(b, d)) return false;
    return true;
}
static bool k_tensor_has_shape(const KTensor *value, int64_t rank, const int64_t *shape) {
    if (rank != value->rank) return false;
    for (int64_t d = 0; d < rank; ++d) if (shape[d] != k_tensor_dimension(value, d)) return false;
    return true;
}
static int64_t k_tensor_offset(const KTensor *value, int64_t rank, const int64_t *indices, KLocation loc) {
    if (rank != value->rank) k_trap(loc, "Tensor index count must equal rank");
    for (int64_t d = 0; d < rank; ++d) if (indices[d] < 0 || indices[d] >= k_tensor_dimension(value, d)) k_trap(loc, "Tensor index out of range");
    int64_t offset = 0;
    for (int64_t d = 0; d < rank; ++d) offset += indices[d] * k_tensor_stride(value, d);
    return offset;
}
static double k_tensor_at(const KTensor *value, int64_t rank, const int64_t *indices, KLocation loc) {
    return value->items[k_tensor_offset(value, rank, indices, loc)];
}
static void k_tensor_set(KTensor *value, double item, int64_t rank, const int64_t *indices, KLocation loc) {
    if (value->borrowed) k_trap(loc, "A tensor view is read-only");
    value->items[k_tensor_offset(value, rank, indices, loc)] = item;
}
static int64_t k_tensor_coordinate(const KTensor *value, int64_t offset, int64_t dimension) {
    for (int64_t d = value->rank - 1; d > dimension; --d) offset /= k_tensor_dimension(value, d);
    return offset % k_tensor_dimension(value, dimension);
}
static KTensor k_tensor_slice(const KTensor *value, int64_t rank, const KTensorIndex *indices, KLocation loc) {
    if (rank != value->rank) k_trap(loc, "Tensor index count must equal rank");
    int64_t *shape = malloc((size_t)(rank ? rank : 1) * sizeof(int64_t));
    int64_t *low = malloc((size_t)(rank ? rank : 1) * sizeof(int64_t));
    if (!shape || !low) k_fatal("Out of memory");
    int64_t output_rank = 0;
    for (int64_t d = 0; d < rank; ++d) {
        int64_t dimension = k_tensor_dimension(value, d);
        if (!(indices[d].kind & 1)) {
            if (indices[d].lo < 0 || indices[d].lo >= dimension) k_trap(loc, "Tensor index out of range");
            low[d] = indices[d].lo;
        } else {
            int64_t lo = indices[d].kind & 2 ? 0 : indices[d].lo;
            int64_t hi = indices[d].kind & 4 ? dimension : indices[d].hi;
            if (lo < 0 || hi < lo || hi > dimension) k_trap(loc, "Tensor slice bounds out of range");
            low[d] = lo; shape[output_rank++] = hi - lo;
        }
    }
    KTensor result = k_tensor_new_empty(output_rank, shape, loc);
    // A full-range slice is a dense hyperrectangle of the source. Copy the
    // innermost dimension in one move per outer coordinate instead of calling
    // k_tensor_at for every element.
    bool all_ranges = !value->transposed;
    for (int64_t d = 0; d < rank && all_ranges; ++d) all_ranges = (indices[d].kind & 1) != 0;
    if (all_ranges) {
        int64_t stride[64];
        int64_t step = 1;
        for (int64_t d = rank; d-- > 0;) { stride[d] = step; step *= k_tensor_dimension(value, d); }
        int64_t inner = output_rank ? shape[output_rank - 1] : 0;
        int64_t outer = 1;
        for (int64_t d = 0; d + 1 < output_rank; ++d) outer *= shape[d];
        int64_t *coordinates = calloc((size_t)(output_rank ? output_rank : 1), sizeof(int64_t));
        if (!coordinates) k_fatal("Out of memory");
        for (int64_t o = 0; o < outer; ++o) {
            int64_t position = o, offset = low[rank - 1];
            for (int64_t d = output_rank - 1; d-- > 0;) {
                coordinates[d] = position % shape[d];
                position /= shape[d];
                offset += (low[d] + coordinates[d]) * stride[d];
            }
            if (inner) memcpy(result.items + o * inner, value->items + offset, (size_t)inner * sizeof(double));
        }
        free(coordinates);
        free(shape); free(low);
        return result;
    }
    int64_t source_index[rank ? rank : 1];
    for (int64_t i = 0; i < result.length; ++i) {
        int64_t position = i, output_dimension = output_rank;
        for (int64_t d = rank; d-- > 0;) {
            int64_t coordinate = low[d];
            if (indices[d].kind & 1) {
                int64_t size = shape[--output_dimension];
                coordinate += position % size; position /= size;
            }
            source_index[d] = coordinate;
        }
        result.items[i] = k_tensor_at(value, rank, source_index, loc);
    }
    free(shape); free(low); return result;
}
static KTensor k_tensor_multiply(const KTensor *a, const KTensor *b, KLocation loc) {
    if (!k_tensor_same_shape(a, b)) k_trap(loc, "Tensor multiplication requires equal shapes");
    KTensor result = k_tensor_new_like_empty(a, loc);
    if (!a->transposed && !b->transposed) {
        double *out = result.items;
        const double *left = a->items, *right = b->items;
        for (int64_t i = 0; i < a->length; ++i) out[i] = left[i] * right[i];
    } else for (int64_t i = 0; i < a->length; ++i) result.items[i] = k_tensor_flat(a, i) * k_tensor_flat(b, i);
    return result;
}
static KTensor k_tensor_scale(const KTensor *a, double scalar, KLocation loc) {
    KTensor result = k_tensor_new_like_empty(a, loc);
    if (!a->transposed) {
        double *out = result.items; const double *in = a->items;
        for (int64_t i = 0; i < a->length; ++i) out[i] = in[i] * scalar;
    } else for (int64_t i = 0; i < a->length; ++i) result.items[i] = k_tensor_flat(a, i) * scalar;
    return result;
}
static KTensor k_tensor_divide(const KTensor *a, double scalar, KLocation loc) {
    KTensor result = k_tensor_new_like_empty(a, loc);
    if (!a->transposed) {
        double *out = result.items; const double *in = a->items;
        for (int64_t i = 0; i < a->length; ++i) out[i] = in[i] / scalar;
    } else for (int64_t i = 0; i < a->length; ++i) result.items[i] = k_tensor_flat(a, i) / scalar;
    return result;
}
static KTensor k_tensor_matmul(const KTensor *a, const KTensor *b, KLocation loc) {
    if ((a->rank != 1 && a->rank != 2) || (b->rank != 1 && b->rank != 2) || (a->rank == 1 && b->rank == 1))
        k_trap(loc, "Tensor @ requires a matrix and vector or two matrices; use linalg.dot for two vectors");
    int64_t rows = a->rank == 2 ? k_tensor_dimension(a, 0) : 1;
    int64_t inner = k_tensor_dimension(a, a->rank - 1);
    int64_t cols = b->rank == 2 ? k_tensor_dimension(b, 1) : 1;
    if (inner != k_tensor_dimension(b, 0)) k_trap(loc, "Tensor matrix multiplication has different inner dimensions");
    int64_t shape[] = {rows, cols};
    int64_t rank = a->rank == 2 && b->rank == 2 ? 2 : 1;
    if (rank == 1) shape[0] = a->rank == 2 ? rows : cols;
    KTensor result = inner == 0 ? k_tensor_new(rank, shape, 0.0, loc) : k_tensor_new_empty(rank, shape, loc);
    // The CBLAS path handles transposed views directly; the transpose flag of
    // a view means its storage holds the transpose of the logical matrix.
    bool a_transposed = a->rank == 2 && a->transposed;
    bool b_transposed = b->rank == 2 && b->transposed;
    if (k_blas_dgemm(rows, inner, cols, a->items, a_transposed, b->items, b_transposed, result.items)) return result;
    for (int64_t i = 0; i < rows; ++i)
        for (int64_t k = 0; k < inner; ++k) {
            double value = a->rank == 2 ? a->items[a_transposed ? k * rows + i : i * inner + k] : a->items[k];
            for (int64_t j = 0; j < cols; ++j) {
                double operand = b->rank == 2 ? b->items[b_transposed ? j * inner + k : k * cols + j] : b->items[k];
                result.items[i * cols + j] += value * operand;
            }
        }
    return result;
}
static KTensor k_tensor_negate(const KTensor *value, KLocation loc) {
    KTensor result = k_tensor_new_like_empty(value, loc);
    if (!value->transposed) {
        double *out = result.items; const double *in = value->items;
        for (int64_t i = 0; i < value->length; ++i) out[i] = -in[i];
    } else for (int64_t i = 0; i < value->length; ++i) result.items[i] = -k_tensor_flat(value, i);
    return result;
}
static KTensor k_tensor_power_double(const KTensor *value, double exponent, KLocation loc) {
    KTensor result = k_tensor_new_like_empty(value, loc);
    if (!value->transposed) {
        double *out = result.items; const double *in = value->items;
        for (int64_t i = 0; i < value->length; ++i) out[i] = pow(in[i], exponent);
    } else for (int64_t i = 0; i < value->length; ++i) result.items[i] = pow(k_tensor_flat(value, i), exponent);
    return result;
}
static KTensor k_tensor_identity(int64_t n, KLocation loc) {
    int64_t shape[] = {n, n};
    KTensor result = k_tensor_new(2, shape, 0.0, loc);
    for (int64_t i = 0; i < n; ++i) result.items[i * n + i] = 1.0;
    return result;
}
static KTensor k_tensor_matrix_power(const KTensor *value, int64_t exponent, KLocation loc) {
    int64_t n = k_tensor_dimension(value, 0);
    KTensor result = k_tensor_identity(n, loc);
    KTensor base = k_tensor_copy(value, loc);
    uint64_t power = (uint64_t)exponent;
    while (power) {
        if (power & 1u) {
            KTensor next = k_tensor_matmul(&result, &base, loc);
            k_tensor_drop(&result); result = next;
        }
        power >>= 1u;
        if (power) {
            KTensor next = k_tensor_matmul(&base, &base, loc);
            k_tensor_drop(&base); base = next;
        }
    }
    k_tensor_drop(&base);
    return result;
}
static KTensor k_tensor_power_int(const KTensor *value, int64_t exponent, KLocation loc) {
    if (exponent < 0) k_trap(loc, "Tensor matrix power requires a non-negative exponent");
    if (value->rank == 2 && k_tensor_dimension(value, 0) == k_tensor_dimension(value, 1)) {
        return k_tensor_matrix_power(value, exponent, loc);
    }
    KTensor result = k_tensor_new_like_empty(value, loc);
    for (int64_t i = 0; i < value->length; ++i) result.items[i] = pow(k_tensor_flat(value, i), (double)exponent);
    return result;
}
static double k_tensor_norm(const KTensor *value) {
    // Scaled sum of squares: the scale keeps large and small values in range.
    double scale = 0.0, sum = 0.0;
    for (int64_t i = 0; i < value->length; ++i) {
        double item = fabs(value->items[i]);
        if (item == 0.0) continue;
        if (scale < item) { double ratio = scale / item; sum = sum * ratio * ratio + 1.0; scale = item; }
        else { double ratio = item / scale; sum += ratio * ratio; }
    }
    return scale * sqrt(sum);
}
static KTensor k_tensor_broadcast(const KTensor *value, int64_t rank, const int64_t *shape, KLocation loc) {
    if (rank < value->rank) k_trap(loc, "Tensor broadcast target rank is too small");
    for (int64_t d = 0; d < rank; ++d) {
        if (shape[d] < 0) k_trap(loc, "Tensor dimensions cannot be negative");
        if (d >= rank - value->rank) {
            int64_t source = k_tensor_dimension(value, d - (rank - value->rank));
            if (source != 1 && source != shape[d]) k_trap(loc, "Tensor dimensions cannot broadcast to target shape");
        }
    }
    KTensor result = k_tensor_new_empty(rank, shape, loc);
    // A whole-shape broadcast is a copy, and the common rank-1 to rank-2 case
    // repeats one row, both without computing an index per element.
    if (rank == value->rank && !value->transposed && k_tensor_same_shape(value, &result)) {
        memcpy(result.items, value->items, (size_t)result.length * sizeof(double));
        return result;
    }
    if (rank == 2 && value->rank == 1 && !value->transposed && shape[1] == value->length) {
        for (int64_t row = 0; row < shape[0]; ++row) memcpy(result.items + row * shape[1], value->items, (size_t)shape[1] * sizeof(double));
        return result;
    }
    for (int64_t i = 0; i < result.length; ++i) {
        int64_t position = i, offset = 0, stride = 1;
        for (int64_t d = rank; d-- > 0;) {
            int64_t coordinate = position % shape[d]; position /= shape[d];
            int64_t source_dimension = d - (rank - value->rank);
            if (source_dimension >= 0) {
                int64_t size = k_tensor_dimension(value, source_dimension);
                if (size != 1) offset += coordinate * stride;
                stride *= size;
            }
        }
        result.items[i] = k_tensor_flat(value, offset);
    }
    return result;
}
static KTensor k_tensor_from_array(int64_t length, const double *items, KLocation loc) {
    KTensor result = k_tensor_new_empty(1, &length, loc);
    if (length) memcpy(result.items, items, (size_t)length * sizeof(double));
    return result;
}

// Text for a tensor follows its shape with nested brackets, such as [[1.0, 2.0], [3.0, 4.0]].
typedef struct { char *bytes; size_t length, capacity; } KTensorText;

static void k_tensor_text_put(KTensorText *text, const char *data, size_t length, KLocation loc) {
    if (length > SIZE_MAX - text->length - 1) k_trap(loc, "Tensor text is too large");
    if (text->length + length + 1 > text->capacity) {
        size_t capacity = text->capacity ? text->capacity : 64;
        while (capacity < text->length + length + 1) {
            if (capacity > SIZE_MAX / 2) k_fatal("Out of memory");
            capacity *= 2;
        }
        char *grown = realloc(text->bytes, capacity);
        if (!grown) k_fatal("Out of memory");
        text->bytes = grown;
        text->capacity = capacity;
    }
    if (length) memcpy(text->bytes + text->length, data, length);
    text->length += length;
    text->bytes[text->length] = '\0';
}

static void k_tensor_text_put_double(KTensorText *text, double item, KLocation loc) {
    KString digits = k_text_double(item);
    k_tensor_text_put(text, digits.data, digits.length, loc);
    k_string_drop(&digits);
}

static void k_tensor_text_level(KTensorText *text, const KTensor *value, int64_t dimension, int64_t *indices, KLocation loc) {
    k_tensor_text_put(text, "[", 1, loc);
    int64_t size = k_tensor_dimension(value, dimension);
    bool leaf = dimension + 1 == value->rank;
    for (int64_t i = 0; i < size; ++i) {
        if (i) {
            if (leaf) {
                k_tensor_text_put(text, ", ", 2, loc);
            } else {
                k_tensor_text_put(text, ",\n", 2, loc);
                for (int64_t space = 0; space <= dimension; ++space) k_tensor_text_put(text, " ", 1, loc);
            }
        }
        indices[dimension] = i;
        if (leaf) k_tensor_text_put_double(text, k_tensor_at(value, value->rank, indices, loc), loc);
        else k_tensor_text_level(text, value, dimension + 1, indices, loc);
    }
    k_tensor_text_put(text, "]", 1, loc);
}

static KString k_tensor_text(const KTensor *value, KLocation loc) {
    KTensorText text = {NULL, 0, 0};
    if (value->rank == 0) k_tensor_text_put_double(&text, k_tensor_at(value, 0, NULL, loc), loc);
    else {
        int64_t *indices = calloc((size_t)value->rank, sizeof *indices);
        if (!indices) k_fatal("Out of memory");
        k_tensor_text_level(&text, value, 0, indices, loc);
        free(indices);
    }
    KString result = k_string_bytes(text.bytes ? text.bytes : "", text.length);
    free(text.bytes);
    return result;
}

// Transpose moves axes: result axis i is source axis axes[i]. With no axes, every axis is reversed.
static KTensor k_tensor_transpose(const KTensor *value, int64_t count, const int64_t *axes, KLocation loc) {
    int64_t rank = value->rank;
    if (count != -1 && count != rank) k_trap(loc, "Tensor transpose needs one axis for each dimension");
    size_t slots = (size_t)(rank ? rank : 1);
    int64_t *order = malloc(slots * sizeof *order);
    int64_t *shape = malloc(slots * sizeof *shape);
    int64_t *index = calloc(slots, sizeof *index);
    if (!order || !shape || !index) k_fatal("Out of memory");
    for (int64_t i = 0; i < rank; ++i) order[i] = count == -1 ? rank - 1 - i : axes[i];
    for (int64_t i = 0; i < rank; ++i) {
        if (order[i] < 0 || order[i] >= rank) k_trap(loc, "Tensor transpose axes must name dimensions of the tensor");
        for (int64_t j = 0; j < i; ++j) {
            if (order[j] == order[i]) k_trap(loc, "Tensor transpose needs each axis once");
        }
        shape[i] = k_tensor_dimension(value, order[i]);
    }
    KTensor result = k_tensor_new_empty(rank, shape, loc);
    int64_t *source = calloc(slots, sizeof *source);
    if (!source) k_fatal("Out of memory");
    for (int64_t flat = 0; flat < result.length; ++flat) {
        for (int64_t i = 0; i < rank; ++i) source[order[i]] = index[i];
        result.items[flat] = k_tensor_at(value, rank, source, loc);
        for (int64_t i = rank - 1; i >= 0; --i) {
            if (++index[i] < shape[i]) break;
            index[i] = 0;
        }
    }
    free(order);
    free(shape);
    free(index);
    free(source);
    return result;
}

static KString k_tensor_debug(const KTensor *value, KLocation loc) {
    KTensorText text = {NULL, 0, 0};
    k_tensor_text_put(&text, "Tensor[double, (", 16, loc);
    for (int64_t d = 0; d < value->rank; ++d) {
        if (d) k_tensor_text_put(&text, ", ", 2, loc);
        KString dimension = k_text_int(k_tensor_dimension(value, d));
        k_tensor_text_put(&text, dimension.data, dimension.length, loc);
        k_string_drop(&dimension);
    }
    if (value->rank == 1) k_tensor_text_put(&text, ",", 1, loc);
    k_tensor_text_put(&text, ")]", 2, loc);
    if (value->rank >= 2) k_tensor_text_put(&text, "\n", 1, loc);
    else k_tensor_text_put(&text, " ", 1, loc);
    if (value->rank) {
        int64_t *indices = calloc((size_t)value->rank, sizeof *indices);
        if (!indices) k_fatal("Out of memory");
        k_tensor_text_level(&text, value, 0, indices, loc);
        free(indices);
    } else k_tensor_text_put_double(&text, k_tensor_flat(value, 0), loc);
    KString result = k_string_bytes(text.bytes, text.length);
    free(text.bytes);
    return result;
}
static KTensor k_tensor_add(const KTensor *a, const KTensor *b, KLocation loc) {
    if (!k_tensor_same_shape(a, b)) k_trap(loc, "Tensor addition requires equal shapes");
    KTensor result = k_tensor_new_like_empty(a, loc);
    if (!a->transposed && !b->transposed) {
        double *out = result.items; const double *left = a->items, *right = b->items;
        for (int64_t i = 0; i < a->length; ++i) out[i] = left[i] + right[i];
    } else for (int64_t i = 0; i < a->length; ++i) result.items[i] = k_tensor_flat(a, i) + k_tensor_flat(b, i);
    return result;
}
static KTensor k_tensor_subtract(const KTensor *a, const KTensor *b, KLocation loc) {
    if (!k_tensor_same_shape(a, b)) k_trap(loc, "Tensor subtraction requires equal shapes");
    KTensor result = k_tensor_new_like_empty(a, loc);
    if (!a->transposed && !b->transposed) {
        double *out = result.items; const double *left = a->items, *right = b->items;
        for (int64_t i = 0; i < a->length; ++i) out[i] = left[i] - right[i];
    } else for (int64_t i = 0; i < a->length; ++i) result.items[i] = k_tensor_flat(a, i) - k_tensor_flat(b, i);
    return result;
}
static KTensor k_tensor_add_scalar(const KTensor *a, double scalar, KLocation loc) {
    KTensor result = k_tensor_new_like_empty(a, loc);
    if (!a->transposed) {
        double *out = result.items; const double *in = a->items;
        for (int64_t i = 0; i < a->length; ++i) out[i] = in[i] + scalar;
    } else for (int64_t i = 0; i < a->length; ++i) result.items[i] = k_tensor_flat(a, i) + scalar;
    return result;
}
static KTensor k_tensor_subtract_scalar(const KTensor *a, double scalar, KLocation loc) {
    KTensor result = k_tensor_new_like_empty(a, loc);
    if (!a->transposed) {
        double *out = result.items; const double *in = a->items;
        for (int64_t i = 0; i < a->length; ++i) out[i] = in[i] - scalar;
    } else for (int64_t i = 0; i < a->length; ++i) result.items[i] = k_tensor_flat(a, i) - scalar;
    return result;
}
static KTensor k_tensor_right_subtract(const KTensor *a, double scalar, KLocation loc) {
    KTensor result = k_tensor_new_like_empty(a, loc);
    if (!a->transposed) {
        double *out = result.items; const double *in = a->items;
        for (int64_t i = 0; i < a->length; ++i) out[i] = scalar - in[i];
    } else for (int64_t i = 0; i < a->length; ++i) result.items[i] = scalar - k_tensor_flat(a, i);
    return result;
}
static KTensor k_tensor_reshape(const KTensor *value, int64_t rank, const int64_t *shape, KLocation loc) {
    // Validate the shape and element count before allocating its data.
    bool empty = false;
    for (int64_t d = 0; d < rank; ++d) {
        if (shape[d] < 0) k_trap(loc, "Tensor dimensions cannot be negative");
        if (!shape[d]) empty = true;
    }
    int64_t count = empty ? 0 : 1;
    if (!empty) for (int64_t d = 0; d < rank; ++d) {
        if (shape[d] && count > INT64_MAX / shape[d]) k_trap(loc, "Tensor reshape needs the same number of elements");
        count *= shape[d];
    }
    if (count != value->length) k_trap(loc, "Tensor reshape needs the same number of elements");
    KTensor result = k_tensor_new_empty(rank, shape, loc);
    if (!value->transposed) memcpy(result.items, value->items, (size_t)count * sizeof(double));
    else for (int64_t i = 0; i < count; ++i) result.items[i] = k_tensor_flat(value, i);
    return result;
}

// Operations 0/1 sum/mean; 2/3 min/max use the existing double Comparable order.
// The sum keeps eight partial accumulators so the dependency chain does not
// limit throughput, and min/max scan with plain comparisons while preserving
// the total order's treatment of NaNs and signed zero.
static double k_tensor_reduce_total(const KTensor *value, int operation, KLocation loc) {
    if (operation != 0 && !value->length) k_trap(loc, "Tensor is empty");
    const double *items = value->items;
    int64_t length = value->length;
    if (operation <= 1) {
        double part[8] = {0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0};
        int64_t i = 0;
        for (; i + 8 <= length; i += 8) {
            part[0] += items[i];     part[1] += items[i + 1];
            part[2] += items[i + 2]; part[3] += items[i + 3];
            part[4] += items[i + 4]; part[5] += items[i + 5];
            part[6] += items[i + 6]; part[7] += items[i + 7];
        }
        for (; i < length; ++i) part[0] += items[i];
        double result = ((part[0] + part[1]) + (part[2] + part[3])) + ((part[4] + part[5]) + (part[6] + part[7]));
        return operation == 1 ? result / (double)length : result;
    }
    bool anyNumber = false, hasNan = false, negativeZero = false, positiveZero = false;
    double best = 0.0, nan = 0.0;
    if (length >= 8) {
        double part[8];
        for (int t = 0; t < 8; ++t) part[t] = items[t];
        int64_t i = 8;
        if (operation == 3) {
            for (; i + 8 <= length; i += 8)
                for (int t = 0; t < 8; ++t) { double v = items[i + t]; part[t] = fmax(part[t], v); hasNan |= (v != v); }
        } else {
            for (; i + 8 <= length; i += 8)
                for (int t = 0; t < 8; ++t) { double v = items[i + t]; part[t] = fmin(part[t], v); hasNan |= (v != v); }
        }
        best = part[0];
        for (int t = 1; t < 8; ++t) best = operation == 3 ? fmax(best, part[t]) : fmin(best, part[t]);
        for (int t = 0; t < 8; ++t) {
            double v = items[t];
            if (v != v) { hasNan = true; nan = v; }
            else if (v == 0.0) { if (signbit(v)) negativeZero = true; else positiveZero = true; }
        }
        for (; i < length; ++i) {
            double v = items[i];
            if (v == 0.0) { if (signbit(v)) negativeZero = true; else positiveZero = true; }
        }
        anyNumber = true;
    } else {
        for (int64_t i = 0; i < length; ++i) {
            double item = items[i];
            if (item != item) {
                if (!hasNan) { hasNan = true; nan = item; }
                continue;
            }
            if (item == 0.0) { if (signbit(item)) negativeZero = true; else positiveZero = true; }
            if (!anyNumber) { anyNumber = true; best = item; }
            else if (operation == 2 ? item < best : item > best) best = item;
        }
    }
    if (operation == 3) {
        if (hasNan) {
            for (int64_t i = 0; i < length; ++i) {
                double item = items[i];
                if (item != item) return item;
            }
        }
        if (best == 0.0 && positiveZero) return 0.0;
        return best;
    }
    if (!anyNumber) return nan;
    if (best == 0.0 && negativeZero) return -0.0;
    return best;
}
static KTensor k_tensor_reduce_axis(const KTensor *value, int operation, int64_t axis, KLocation loc) {
    if (axis < 0 || axis >= value->rank) k_trap(loc, "Tensor axis out of range");
    if (operation != 0 && !value->length) k_trap(loc, "Tensor is empty");
    int64_t rank = value->rank - 1;
    int64_t *shape = malloc((size_t)(rank ? rank : 1) * sizeof *shape);
    int64_t *indices = calloc((size_t)(value->rank ? value->rank : 1), sizeof *indices);
    if (!shape || !indices) k_fatal("Out of memory");
    for (int64_t d = 0, next = 0; d < value->rank; ++d) if (d != axis) shape[next++] = k_tensor_dimension(value, d);
    KTensor output = k_tensor_new_empty(rank, shape, loc);
    free(shape);
    int64_t axis_size = k_tensor_dimension(value, axis);
    if (!value->transposed && value->rank == 2) {
        int64_t rows = value->shape[0], cols = value->shape[1];
        if (axis == 0 && operation < 2) {
            // Accumulate whole rows into the output, so reads are sequential.
            for (int64_t j = 0; j < cols; ++j) output.items[j] = 0.0;
            for (int64_t i = 0; i < rows; ++i) {
                const double *row = value->items + i * cols;
                for (int64_t j = 0; j < cols; ++j) output.items[j] += row[j];
            }
            if (operation == 1) for (int64_t j = 0; j < cols; ++j) output.items[j] /= (double)rows;
        } else if (axis == 1 && operation < 2) {
            for (int64_t i = 0; i < rows; ++i) {
                const double *row = value->items + i * cols;
                double part[8] = {0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0};
                int64_t j = 0;
                for (; j + 8 <= cols; j += 8) {
                    part[0] += row[j];     part[1] += row[j + 1];
                    part[2] += row[j + 2]; part[3] += row[j + 3];
                    part[4] += row[j + 4]; part[5] += row[j + 5];
                    part[6] += row[j + 6]; part[7] += row[j + 7];
                }
                for (; j < cols; ++j) part[0] += row[j];
                double total = ((part[0] + part[1]) + (part[2] + part[3])) + ((part[4] + part[5]) + (part[6] + part[7]));
                output.items[i] = operation == 1 ? total / (double)cols : total;
            }
        } else {
            // Plain fmax/fmin fills the result in vectorizable passes; a column
            // or row that contains a NaN or a zero is redone with the total
            // order's rules for NaNs and signed zeros.
            bool *special = calloc((size_t)(axis == 0 ? cols : rows), sizeof(bool));
            if (!special) k_fatal("Out of memory");
            if (axis == 0) {
                bool *anyValue = calloc((size_t)cols, sizeof(bool));
                bool *negativeZero = calloc((size_t)cols, sizeof(bool));
                bool *positiveZero = calloc((size_t)cols, sizeof(bool));
                double *nanValue = calloc((size_t)(cols ? cols : 1), sizeof(double));
                if (!anyValue || !negativeZero || !positiveZero || !nanValue) k_fatal("Out of memory");
                for (int64_t j = 0; j < cols; ++j) {
                    double v = value->items[j];
                    output.items[j] = v;
                    special[j] = v != v;
                    anyValue[j] = v == v;
                    if (v != v) nanValue[j] = v;
                    else if (v == 0.0) { if (signbit(v)) negativeZero[j] = true; else positiveZero[j] = true; }
                }
                for (int64_t i = 1; i < rows; ++i) {
                    const double *row = value->items + i * cols;
                    for (int64_t j = 0; j < cols; ++j) {
                        double v = row[j];
                        if (v != v) {
                            if (!special[j]) { special[j] = true; nanValue[j] = v; }
                            continue;
                        }
                        anyValue[j] = true;
                        if (v == 0.0) { if (signbit(v)) negativeZero[j] = true; else positiveZero[j] = true; }
                        output.items[j] = operation == 3 ? fmax(output.items[j], v) : fmin(output.items[j], v);
                    }
                }
                // The sign of a zero result and the NaN rules are decided from
                // the flags; rescanning the columns here cost more than the
                // whole reduction.
                for (int64_t j = 0; j < cols; ++j) {
                    if (operation == 3) {
                        if (special[j]) output.items[j] = nanValue[j];
                        else if (output.items[j] == 0.0) output.items[j] = positiveZero[j] ? 0.0 : -0.0;
                    } else {
                        if (!anyValue[j]) output.items[j] = nanValue[j];
                        else if (output.items[j] == 0.0) output.items[j] = negativeZero[j] ? -0.0 : 0.0;
                    }
                }
                free(anyValue); free(negativeZero); free(positiveZero); free(nanValue);
            } else {
                for (int64_t i = 0; i < rows; ++i) {
                    const double *row = value->items + i * cols;
                    double part[8];
                    bool nan[8];
                    for (int t = 0; t < 8 && t < cols; ++t) { part[t] = row[t]; nan[t] = row[t] != row[t]; }
                    for (int t = (cols < 8 ? (int)cols : 8); t < 8; ++t) { part[t] = operation == 3 ? -INFINITY : INFINITY; nan[t] = false; }
                    int64_t j = 8;
                    for (; j + 8 <= cols; j += 8)
                        for (int t = 0; t < 8; ++t) { double v = row[j + t]; nan[t] |= (v != v); part[t] = operation == 3 ? fmax(part[t], v) : fmin(part[t], v); }
                    for (; j < cols; ++j) { double v = row[j]; nan[0] |= (v != v); part[0] = operation == 3 ? fmax(part[0], v) : fmin(part[0], v); }
                    bool hasNan = false;
                    for (int t = 0; t < 8; ++t) hasNan |= nan[t];
                    double extreme = part[0];
                    for (int t = 1; t < 8; ++t) extreme = operation == 3 ? fmax(extreme, part[t]) : fmin(extreme, part[t]);
                    if (!hasNan && extreme != 0.0) { output.items[i] = extreme; continue; }
                    double fixed = row[0];
                    bool any = false, hasNan2 = false, negativeZero = false, positiveZero = false;
                    double nanValue = 0.0;
                    for (int64_t q = 0; q < cols; ++q) {
                        double v = row[q];
                        if (v != v) { if (!hasNan2) { hasNan2 = true; nanValue = v; } continue; }
                        if (v == 0.0) { if (signbit(v)) negativeZero = true; else positiveZero = true; }
                        if (!any) { any = true; fixed = v; }
                        else if (operation == 2 ? v < fixed : v > fixed) fixed = v;
                    }
                    output.items[i] = operation == 3 ? (hasNan2 ? nanValue : (fixed == 0.0 && positiveZero ? 0.0 : fixed))
                                                     : (any ? (fixed == 0.0 && negativeZero ? -0.0 : fixed) : nanValue);
                }
            }
            free(special);
        }
        free(indices);
        return output;
    }
    for (int64_t flat = 0; flat < output.length; ++flat) {
        int64_t position = flat;
        for (int64_t d = rank; d-- > 0;) {
            int64_t size = k_tensor_dimension(value, d < axis ? d : d + 1);
            indices[d < axis ? d : d + 1] = position % size;
            position /= size;
        }
        double result = 0.0;
        for (int64_t i = 0; i < axis_size; ++i) {
            indices[axis] = i;
            double item = k_tensor_at(value, value->rank, indices, loc);
            if (operation == 0 || operation == 1) result += item;
            else if (i == 0 || (operation == 2 ? k_compare_double(item, result) < 0 : k_compare_double(item, result) > 0)) result = item;
        }
        output.items[flat] = operation == 1 ? result / (double)axis_size : result;
    }
    free(indices);
    return output;
}
static double k_tensor_reduce(const KTensor *value, int operation, int64_t axis, bool total, KTensor *output, KLocation loc) {
    if (total || !output) return k_tensor_reduce_total(value, operation, loc);
    *output = k_tensor_reduce_axis(value, operation, axis, loc);
    return 0.0;
}
static KTensor k_tensor_reduce_tensor(const KTensor *value, int operation, int64_t axis, KLocation loc) {
    return k_tensor_reduce_axis(value, operation, axis, loc);
}

static KTensor k_tensor_construct(int64_t length, int64_t rank, const int64_t *shape, KLocation loc) {
    bool empty = false;
    for (int64_t d = 0; d < rank; ++d) {
        if (shape[d] < 0) k_trap(loc, "Tensor dimensions cannot be negative");
        if (!shape[d]) empty = true;
    }
    int64_t count = empty ? 0 : 1;
    if (!empty) for (int64_t d = 0; d < rank; ++d) {
        if (shape[d] && count > INT64_MAX / shape[d]) k_trap(loc, "Tensor shape does not match the number of elements");
        count *= shape[d];
    }
    if (count != length) k_trap(loc, "Tensor shape does not match the number of elements");
    return k_tensor_new(rank, shape, 0.0, loc);
}
static void k_tensor_require_rank(const KTensor *value, int64_t expected, KLocation loc) {
    if (value->rank != expected) k_trap(loc, "Tensor rank does not match generator parameter count");
}
