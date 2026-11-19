/* Signed integers in little-endian base 2^32. Zero has no allocation. */
typedef struct { uint32_t *limbs; size_t length; int sign; } KBigInt;
static KBigInt k_bigint_allocate(size_t length) {
    if (length > SIZE_MAX / sizeof(uint32_t)) k_fatal("BigInt is too large");
    if (!length) return (KBigInt){0};
    uint32_t *limbs = calloc(length, sizeof *limbs);
    if (!limbs) k_fatal("Out of memory");
    return (KBigInt){limbs, length, 1};
}
static void k_bigint_drop(KBigInt *value) {
    free(value->limbs);
    *value = (KBigInt){0};
}
static void k_bigint_normalize(KBigInt *value) {
    while (value->length && !value->limbs[value->length - 1]) --value->length;
    if (!value->length) k_bigint_drop(value);
}
static KBigInt k_bigint_clone(const KBigInt *value) {
    KBigInt result = k_bigint_allocate(value->length);
    if (value->length) memcpy(result.limbs, value->limbs, value->length * sizeof *value->limbs);
    result.sign = value->sign;
    return result;
}
static KBigInt k_bigint_one(int sign) {
    KBigInt result = k_bigint_allocate(1);
    result.limbs[0] = 1;
    result.sign = sign;
    return result;
}
static KBigInt k_bigint_literal(KString text) {
    size_t start = 0;
    unsigned base = 10;
    if (text.length >= 2 && text.data[0] == '0' && (text.data[1] == 'x' || text.data[1] == 'X')) {
        start = 2;
        base = 16;
    }
    /* Eight decimal digits fit a limb too; this bound also covers hexadecimal. */
    KBigInt result = k_bigint_allocate((text.length - start) / 8 + 1);
    size_t used = 0;
    for (size_t i = start; i < text.length; ++i) {
        unsigned char c = (unsigned char)text.data[i];
        unsigned digit = c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 'a' + 10 : c >= 'A' && c <= 'F' ? c - 'A' + 10 : 255;
        if (digit >= base) k_fatal("Invalid BigInt literal");
        uint64_t carry = digit;
        for (size_t j = 0; j < used; ++j) {
            uint64_t product = (uint64_t)result.limbs[j] * base + carry;
            result.limbs[j] = (uint32_t)product;
            carry = product >> 32;
        }
        if (carry) result.limbs[used++] = (uint32_t)carry;
    }
    result.length = used;
    k_bigint_normalize(&result);
    return result;
}
static int k_bigint_magnitude_compare(const KBigInt *a, const KBigInt *b) {
    if (a->length != b->length) return a->length < b->length ? -1 : 1;
    for (size_t i = a->length; i-- > 0;) {
        if (a->limbs[i] != b->limbs[i]) return a->limbs[i] < b->limbs[i] ? -1 : 1;
    }
    return 0;
}
static int64_t k_bigint_compare(const KBigInt *a, const KBigInt *b) {
    if (a->sign != b->sign) return a->sign < b->sign ? -1 : 1;
    return (int64_t)a->sign * k_bigint_magnitude_compare(a, b);
}
static KBigInt k_bigint_add_magnitude(const KBigInt *a, const KBigInt *b) {
    size_t length = a->length > b->length ? a->length : b->length;
    if (length == SIZE_MAX) k_fatal("BigInt is too large");
    KBigInt result = k_bigint_allocate(length + 1);
    uint64_t carry = 0;
    for (size_t i = 0; i < length; ++i) {
        uint64_t sum = carry + (i < a->length ? a->limbs[i] : 0) + (uint64_t)(i < b->length ? b->limbs[i] : 0);
        result.limbs[i] = (uint32_t)sum;
        carry = sum >> 32;
    }
    result.limbs[length] = (uint32_t)carry;
    k_bigint_normalize(&result);
    return result;
}
/* Magnitude a is at least magnitude b. */
static KBigInt k_bigint_sub_magnitude(const KBigInt *a, const KBigInt *b) {
    KBigInt result = k_bigint_allocate(a->length);
    uint64_t borrow = 0;
    for (size_t i = 0; i < a->length; ++i) {
        uint64_t difference = (uint64_t)a->limbs[i] - (i < b->length ? b->limbs[i] : 0) - borrow;
        result.limbs[i] = (uint32_t)difference;
        borrow = difference >> 63;
    }
    k_bigint_normalize(&result);
    return result;
}
static KBigInt k_bigint_add(const KBigInt *a, const KBigInt *b) {
    if (!a->sign) return k_bigint_clone(b);
    if (!b->sign) return k_bigint_clone(a);
    if (a->sign == b->sign) {
        KBigInt result = k_bigint_add_magnitude(a, b);
        result.sign = a->sign;
        return result;
    }
    int comparison = k_bigint_magnitude_compare(a, b);
    if (!comparison) return (KBigInt){0};
    KBigInt result = comparison > 0 ? k_bigint_sub_magnitude(a, b) : k_bigint_sub_magnitude(b, a);
    result.sign = comparison > 0 ? a->sign : b->sign;
    return result;
}
static KBigInt k_bigint_sub(const KBigInt *a, const KBigInt *b) {
    KBigInt opposite = *b;
    opposite.sign = -opposite.sign;
    return k_bigint_add(a, &opposite);
}
static KBigInt k_bigint_neg(const KBigInt *value) {
    KBigInt result = k_bigint_clone(value);
    result.sign = -result.sign;
    return result;
}
static KBigInt k_bigint_mul(const KBigInt *a, const KBigInt *b) {
    if (!a->sign || !b->sign) return (KBigInt){0};
    if (a->length > SIZE_MAX - b->length) k_fatal("BigInt is too large");
    KBigInt result = k_bigint_allocate(a->length + b->length);
    for (size_t i = 0; i < a->length; ++i) {
        uint64_t carry = 0;
        for (size_t j = 0; j < b->length; ++j) {
            uint64_t product = (uint64_t)a->limbs[i] * b->limbs[j] + result.limbs[i + j] + carry;
            result.limbs[i + j] = (uint32_t)product;
            carry = product >> 32;
        }
        result.limbs[i + b->length] = (uint32_t)carry;
    }
    k_bigint_normalize(&result);
    result.sign = a->sign * b->sign;
    return result;
}
/* Divide a mutable magnitude by one limb and return the remainder. */
static uint32_t k_bigint_div_small(KBigInt *value, uint32_t divisor) {
    uint64_t remainder = 0;
    for (size_t i = value->length; i-- > 0;) {
        uint64_t dividend = (remainder << 32) | value->limbs[i];
        value->limbs[i] = (uint32_t)(dividend / divisor);
        remainder = dividend % divisor;
    }
    k_bigint_normalize(value);
    return (uint32_t)remainder;
}
/* Normalized basecase long division, with quotient correction and addback. */
static void k_bigint_divmod(const KBigInt *a, const KBigInt *b, KBigInt *quotient, KBigInt *remainder, KLocation loc) {
    if (!b->sign) k_trap(loc, "BigInt division by zero");
    *quotient = (KBigInt){0};
    *remainder = (KBigInt){0};
    int comparison = k_bigint_magnitude_compare(a, b);
    if (comparison < 0) { *remainder = k_bigint_clone(a); return; }
    if (!comparison) { *quotient = k_bigint_one(a->sign * b->sign); return; }
    if (b->length == 1) {
        *quotient = k_bigint_clone(a);
        uint32_t rest = k_bigint_div_small(quotient, b->limbs[0]);
        quotient->sign = quotient->length ? a->sign * b->sign : 0;
        if (rest) { *remainder = k_bigint_allocate(1); remainder->limbs[0] = rest; remainder->sign = a->sign; }
        return;
    }
    size_t n = b->length;
    if (a->length == SIZE_MAX) k_fatal("BigInt is too large");
    KBigInt u = k_bigint_allocate(a->length + 1);
    KBigInt v = k_bigint_allocate(n);
    unsigned shift = 0;
    for (uint32_t high = b->limbs[n - 1]; !(high & UINT32_C(0x80000000)); high <<= 1) ++shift;
    uint64_t carry = 0;
    for (size_t i = 0; i < a->length; ++i) {
        uint64_t digit = ((uint64_t)a->limbs[i] << shift) | carry;
        u.limbs[i] = (uint32_t)digit;
        carry = digit >> 32;
    }
    u.limbs[a->length] = (uint32_t)carry;
    carry = 0;
    for (size_t i = 0; i < n; ++i) {
        uint64_t digit = ((uint64_t)b->limbs[i] << shift) | carry;
        v.limbs[i] = (uint32_t)digit;
        carry = digit >> 32;
    }
    *quotient = k_bigint_allocate(a->length - n + 1);
    const uint64_t base = UINT64_C(1) << 32;
    for (size_t j = quotient->length; j-- > 0;) {
        uint64_t qhat, rhat;
        if (u.limbs[j + n] == v.limbs[n - 1]) {
            qhat = base - 1;
            rhat = (uint64_t)u.limbs[j + n - 1] + v.limbs[n - 1];
        } else {
            uint64_t dividend = ((uint64_t)u.limbs[j + n] << 32) | u.limbs[j + n - 1];
            qhat = dividend / v.limbs[n - 1];
            rhat = dividend % v.limbs[n - 1];
        }
        while (rhat < base && qhat * v.limbs[n - 2] > (rhat << 32) + u.limbs[j + n - 2]) {
            --qhat;
            rhat += v.limbs[n - 1];
        }
        uint64_t borrow = 0;
        for (size_t i = 0; i < n; ++i) {
            uint64_t product = qhat * v.limbs[i] + borrow;
            uint64_t difference = (uint64_t)u.limbs[j + i] - (uint32_t)product;
            u.limbs[j + i] = (uint32_t)difference;
            borrow = (product >> 32) + (difference >> 63);
        }
        bool negative = u.limbs[j + n] < borrow;
        u.limbs[j + n] = (uint32_t)((uint64_t)u.limbs[j + n] - borrow);
        if (negative) {
            --qhat;
            carry = 0;
            for (size_t i = 0; i < n; ++i) {
                uint64_t sum = (uint64_t)u.limbs[j + i] + v.limbs[i] + carry;
                u.limbs[j + i] = (uint32_t)sum;
                carry = sum >> 32;
            }
            u.limbs[j + n] += (uint32_t)carry;
        }
        quotient->limbs[j] = (uint32_t)qhat;
    }
    *remainder = k_bigint_allocate(n);
    for (size_t i = 0; i < n; ++i) {
        remainder->limbs[i] = u.limbs[i] >> shift;
        if (shift) remainder->limbs[i] |= u.limbs[i + 1] << (32 - shift);
    }
    k_bigint_normalize(quotient);
    k_bigint_normalize(remainder);
    quotient->sign = quotient->length ? a->sign * b->sign : 0;
    remainder->sign = remainder->length ? a->sign : 0;
    k_bigint_drop(&u);
    k_bigint_drop(&v);
}
static KBigInt k_bigint_div(const KBigInt *a, const KBigInt *b, KLocation loc) {
    KBigInt quotient, remainder;
    k_bigint_divmod(a, b, &quotient, &remainder, loc);
    k_bigint_drop(&remainder);
    return quotient;
}
static KBigInt k_bigint_rem(const KBigInt *a, const KBigInt *b, KLocation loc) {
    KBigInt quotient, remainder;
    k_bigint_divmod(a, b, &quotient, &remainder, loc);
    k_bigint_drop(&quotient);
    return remainder;
}
static KBigInt k_bigint_pow(const KBigInt *value, const KBigInt *exponent, KLocation loc) {
    if (exponent->sign < 0) k_trap(loc, "BigInt power with a negative exponent");
    if (!exponent->sign) return k_bigint_one(1);
    if (!value->sign) return (KBigInt){0};
    if (value->length == 1 && value->limbs[0] == 1) return k_bigint_one(value->sign < 0 && (exponent->limbs[0] & 1) ? -1 : 1);
    KBigInt result = k_bigint_one(1);
    bool started = false;
    for (size_t i = exponent->length; i-- > 0;) {
        for (unsigned bit = 32; bit-- > 0;) {
            bool set = (exponent->limbs[i] >> bit) & 1;
            if (!started && !set) continue;
            if (started) { KBigInt next = k_bigint_mul(&result, &result); k_bigint_drop(&result); result = next; }
            if (set) { KBigInt next = k_bigint_mul(&result, value); k_bigint_drop(&result); result = next; }
            started = true;
        }
    }
    return result;
}
static KString k_bigint_text(const KBigInt *value) {
    if (!value->sign) return K_TEXT("0");
    if (value->length > (SIZE_MAX / sizeof(uint32_t) - 1) / 2) k_fatal("BigInt text is too large");
    uint32_t *groups = malloc((value->length * 2 + 1) * sizeof *groups);
    if (!groups) k_fatal("Out of memory");
    KBigInt copy = k_bigint_clone(value);
    size_t count = 0;
    do { groups[count++] = k_bigint_div_small(&copy, UINT32_C(1000000000)); } while (copy.length);
    if (count > (SIZE_MAX - 1) / 9) k_fatal("BigInt text is too large");
    size_t digits = (count - 1) * 9 + (value->sign < 0);
    for (uint32_t top = groups[count - 1]; top; top /= 10) ++digits;
    KString result = k_string_new(digits);
    char *cursor = (char *)result.data + digits;
    for (size_t i = 0; i < count; ++i) {
        uint32_t group = groups[i];
        if (i + 1 < count) { for (unsigned j = 0; j < 9; ++j) { *--cursor = (char)('0' + group % 10); group /= 10; } }
        else { do { *--cursor = (char)('0' + group % 10); group /= 10; } while (group); }
    }
    if (value->sign < 0) *--cursor = '-';
    free(groups);
    return result;
}
static uint64_t k_bigint_hash(const KBigInt *value) {
    uint64_t hash = UINT64_C(14695981039346656037) ^ (uint64_t)(value->sign + 1);
    for (size_t i = 0; i < value->length; ++i) hash = (hash ^ value->limbs[i]) * UINT64_C(1099511628211);
    return hash;
}
static KBigInt k_bigint_fromInt(int64_t value) {
    if (!value) return (KBigInt){0};
    uint64_t magnitude = value < 0 ? UINT64_C(0) - (uint64_t)value : (uint64_t)value;
    KBigInt result = k_bigint_allocate(magnitude >> 32 ? 2 : 1);
    result.limbs[0] = (uint32_t)magnitude;
    if (result.length == 2) result.limbs[1] = (uint32_t)(magnitude >> 32);
    result.sign = value < 0 ? -1 : 1;
    return result;
}
static KBigInt k_bigint_fromNat(uint64_t value) {
    if (!value) return (KBigInt){0};
    KBigInt result = k_bigint_allocate(value >> 32 ? 2 : 1);
    result.limbs[0] = (uint32_t)value;
    if (result.length == 2) result.limbs[1] = (uint32_t)(value >> 32);
    result.sign = 1;
    return result;
}
static bool k_bigint_to_int(const KBigInt *value, int64_t *out) {
    if (value->length > 2) return false;
    uint64_t magnitude = value->length ? value->limbs[0] : 0;
    if (value->length == 2) magnitude |= (uint64_t)value->limbs[1] << 32;
    uint64_t limit = value->sign < 0 ? UINT64_C(1) << 63 : INT64_MAX;
    if (magnitude > limit) return false;
    *out = value->sign < 0 ? magnitude == (UINT64_C(1) << 63) ? INT64_MIN : -(int64_t)magnitude : (int64_t)magnitude;
    return true;
}
static bool k_bigint_to_double(const KBigInt *value, double *out) {
    if (value->length > 32) return false;
    if (!value->length) { *out = 0.0; return true; }
    unsigned high_bits = 0;
    for (uint32_t high = value->limbs[value->length - 1]; high; high >>= 1) ++high_bits;
    size_t bits = (value->length - 1) * 32 + high_bits;
    size_t shift = bits > 53 ? bits - 53 : 0;
    uint64_t top = 0;
    for (size_t i = bits; i-- > shift;) top = (top << 1) | ((value->limbs[i / 32] >> (i % 32)) & 1);
    if (shift) {
        size_t guard_bit = shift - 1;
        bool guard = (value->limbs[guard_bit / 32] >> (guard_bit % 32)) & 1;
        bool sticky = false;
        for (size_t i = 0; i < guard_bit / 32; ++i) sticky |= value->limbs[i] != 0;
        unsigned part = (unsigned)(guard_bit % 32);
        if (part) sticky |= (value->limbs[guard_bit / 32] & ((UINT32_C(1) << part) - 1)) != 0;
        if (guard && (sticky || (top & 1))) ++top;
    }
    double result = ldexp((double)top, (int)shift);
    if (!isfinite(result)) return false;
    *out = value->sign < 0 ? -result : result;
    return true;
}
static bool k_bigint_parse_value(KString text, KBigInt *out) {
    *out = (KBigInt){0};
    bool negative = text.length && text.data[0] == '-';
    if (text.length && (negative || text.data[0] == '+')) { ++text.data; --text.length; }
    if (!text.length) return false;
    for (size_t i = 0; i < text.length; ++i) if (text.data[i] < '0' || text.data[i] > '9') return false;
    *out = k_bigint_literal(text);
    if (negative) out->sign = -out->sign;
    return true;
}
static KBigInt k_bigint_abs(const KBigInt *value) {
    KBigInt result = k_bigint_clone(value);
    if (result.sign < 0) result.sign = 1;
    return result;
}
static KBigInt k_bigint_min(const KBigInt *a, const KBigInt *b) {
    return k_bigint_clone(k_bigint_compare(a, b) <= 0 ? a : b);
}
static KBigInt k_bigint_max(const KBigInt *a, const KBigInt *b) {
    return k_bigint_clone(k_bigint_compare(a, b) >= 0 ? a : b);
}
