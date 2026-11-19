/* Unicode 17.0 default case conversion and scalar operations. */
#define K_COUNT(array) (sizeof(array)/sizeof((array)[0]))
static uint32_t k_utf8_next(KString text, size_t *index) {
    uint32_t code = (unsigned char)text.data[(*index)++];
    if (code < 0x80) return code;
    unsigned count = code < 0xe0 ? 1 : code < 0xf0 ? 2 : 3;
    code &= count == 1 ? 31 : count == 2 ? 15 : 7;
    while (count--) code = (code << 6) | ((unsigned char)text.data[(*index)++] & 63);
    return code;
}
static size_t k_utf8_width(uint32_t code) { return code < 0x80 ? 1 : code < 0x800 ? 2 : code < 0x10000 ? 3 : 4; }
static void k_utf8_write(char *text, size_t *index, uint32_t code) {
    size_t width = k_utf8_width(code);
    if (width == 1) { text[(*index)++] = (char)code; return; }
    size_t begin = *index;
    *index += width;
    for (size_t j = width-1; j > 0; --j) { text[begin+j] = (char)(0x80 | (code & 63)); code >>= 6; }
    text[begin] = (char)((width == 2 ? 0xc0 : width == 3 ? 0xe0 : 0xf0) | code);
}
static bool k_unicode_property(const KCodeRange *ranges, size_t count, uint32_t code) {
    size_t low = 0, high = count;
    while (low < high) {
        size_t mid = low + (high-low)/2;
        if (code < ranges[mid].start) high = mid;
        else if (code > ranges[mid].end) low = mid+1;
        else return true;
    }
    return false;
}
static bool k_unicode_is_cased(uint32_t code) { return k_unicode_property(k_unicode_cased,K_COUNT(k_unicode_cased),code); }
static bool k_unicode_is_ignorable(uint32_t code) { return k_unicode_property(k_unicode_ignorable,K_COUNT(k_unicode_ignorable),code); }
static bool k_unicode_is_space(uint32_t code) { return k_unicode_property(k_unicode_space,K_COUNT(k_unicode_space),code); }
static const KCaseMap *k_unicode_mapping(const KCaseMap *table, size_t count, uint32_t code) {
    size_t low = 0, high = count;
    while (low < high) { size_t mid = low + (high-low)/2; if (code < table[mid].code) high = mid; else if (code > table[mid].code) low = mid+1; else return &table[mid]; }
    return NULL;
}
static KString k_string_slice(KString text, size_t begin, size_t end) {
    if (begin == 0 && end == text.length) return k_string_copy(text);
    if (begin == end) return k_empty;
    // Short slices of line-sized text share the source allocation. Splitting a
    // CSV line then costs one owner instead of one allocation per field, and
    // the owner is released when its last field dies. Large sources still copy,
    // so taking one character from a megabyte does not retain the megabyte.
    if (text.owner && text.length <= 4096) {
        if (atomic_fetch_add_explicit(&text.owner->references, 1, memory_order_relaxed) == SIZE_MAX)
            k_fatal("String reference count overflow");
        KString shared = {text.data + begin, end - begin, text.owner};
        return shared;
    }
    KString result = k_string_new(end-begin);
    memcpy((char *)result.data,text.data+begin,end-begin);
    return result;
}
static KString k_string_slice_shared(KString text, size_t begin, size_t end) {
    if (begin == 0 && end == text.length) return k_string_copy(text);
    if (begin == end) return k_empty;
    if (text.owner) {
        if (atomic_fetch_add_explicit(&text.owner->references, 1, memory_order_relaxed) == SIZE_MAX)
            k_fatal("String reference count overflow");
        KString shared = {text.data + begin, end - begin, text.owner};
        return shared;
    }
    KString result = k_string_new(end-begin);
    memcpy((char *)result.data,text.data+begin,end-begin);
    return result;
}

static int64_t k_string_length(KString text) {
    uint64_t length = 0;
    for (size_t i = 0; i < text.length; ++i) if (((unsigned char)text.data[i] & 0xc0) != 0x80) ++length;
    if (length > INT64_MAX) k_fatal("String length exceeds Int");
    return (int64_t)length;
}
/* Byte range of the character at a counted position; false when the position is outside the text. */
static bool k_string_scalar_range(KString text, int64_t index, size_t *begin, size_t *end) {
    if (index < 0) return false;
    size_t i = 0;
    for (int64_t n = 0; i < text.length; ++n) {
        size_t start = i;
        k_utf8_next(text,&i);
        if (n == index) { *begin = start; *end = i; return true; }
    }
    return false;
}
static KString k_string_get_scalar(KString text, int64_t index, bool *found) {
    size_t begin = 0, end = 0;
    *found = k_string_scalar_range(text,index,&begin,&end);
    return *found ? k_string_slice(text,begin,end) : k_empty;
}
static KString k_string_at(KString text, int64_t index, KLocation loc) {
    bool found = false;
    KString character = k_string_get_scalar(text,index,&found);
    if (!found) k_trap(loc,"String index out of range");
    return character;
}
static KString k_string_trim(KString text) {
    size_t first = text.length, end = text.length;
    for (size_t i = 0; i < text.length;) {
        size_t begin = i;
        uint32_t code = k_utf8_next(text,&i);
        if (!k_unicode_is_space(code)) { if (first == text.length) first = begin; end = i; }
    }
    return k_string_slice(text,first,end);
}
static KString k_string_case(KString text, bool upper) {
    const KCaseMap *table = upper ? k_unicode_upper : k_unicode_lower;
    size_t count = upper ? K_COUNT(k_unicode_upper) : K_COUNT(k_unicode_lower);
    size_t length = 0;
    bool changed = false;
    KString result = k_empty;
    for (unsigned pass = 0; pass < 2; ++pass) {
        bool previous_cased = false;
        size_t output = 0;
        for (size_t i = 0; i < text.length;) {
            uint32_t code = k_utf8_next(text,&i), mapped[3];
            const KCaseMap *mapping = k_unicode_mapping(table,count,code);
            unsigned n = mapping ? mapping->length : 1;
            if (mapping) memcpy(mapped,mapping->mapped,sizeof mapped); else mapped[0] = code;
            if (!upper && code == 0x3a3 && previous_cased) {
                size_t ahead = i;
                bool following_cased = false;
                while (ahead < text.length) { uint32_t next = k_utf8_next(text,&ahead); if (k_unicode_is_ignorable(next)) continue; following_cased = k_unicode_is_cased(next); break; }
                if (!following_cased) mapped[0] = 0x3c2;
            }
            if (!k_unicode_is_ignorable(code)) previous_cased = k_unicode_is_cased(code);
            changed |= n != 1 || mapped[0] != code;
            for (unsigned j = 0; j < n; ++j) {
                if (pass == 0) { size_t width = k_utf8_width(mapped[j]); if (length > SIZE_MAX-width) k_fatal("String is too large"); length += width; }
                else k_utf8_write((char *)result.data,&output,mapped[j]);
            }
        }
        if (pass == 0) { if (!changed) return k_string_copy(text); result = k_string_new(length); }
    }
    return result;
}
static bool k_string_starts_with(KString text, KString prefix) {
    return prefix.length <= text.length && (!prefix.length || memcmp(text.data, prefix.data, prefix.length) == 0);
}
static bool k_string_ends_with(KString text, KString suffix) {
    return suffix.length <= text.length && (!suffix.length || memcmp(text.data + text.length - suffix.length, suffix.data, suffix.length) == 0);
}
static bool k_string_contains(KString text, KString needle) {
    if (!needle.length) return true;
    if (needle.length > text.length) return false;
    if (needle.length == 1) return memchr(text.data,(unsigned char)needle.data[0],text.length) != NULL;
    if (needle.length > SIZE_MAX/sizeof(size_t)) k_fatal("String is too large");
    size_t *prefix = malloc(needle.length*sizeof *prefix);
    if (!prefix) k_fatal("Out of memory");
    prefix[0] = 0;
    for (size_t i = 1, n = 0; i < needle.length; ++i) { while (n && needle.data[i] != needle.data[n]) n = prefix[n-1]; if (needle.data[i] == needle.data[n]) ++n; prefix[i] = n; }
    bool found = false;
    for (size_t i = 0, n = 0; i < text.length; ++i) { while (n && text.data[i] != needle.data[n]) n = prefix[n-1]; if (text.data[i] == needle.data[n]) ++n; if (n == needle.length) { found = true; break; } }
    free(prefix);
    return found;
}

/* Python's str methods (Q136). Predicates follow CPython; positions and widths count Unicode scalars, as
   String.length does. Every function here is unused in programs that never call it, so each carries the
   unused attribute, which the -Werror build of generated programs requires. */
#define K_UNUSED __attribute__((unused))
static K_UNUSED size_t k_string_scalars(KString text) {
    size_t count = 0;
    for (size_t i = 0; i < text.length; ++i) if (((unsigned char)text.data[i] & 0xc0) != 0x80) ++count;
    return count;
}
static K_UNUSED bool k_unicode_is_letter(uint32_t code) { return k_unicode_property(k_unicode_letter, K_COUNT(k_unicode_letter), code); }
static K_UNUSED bool k_unicode_is_decimal(uint32_t code) { return k_unicode_property(k_unicode_decimal, K_COUNT(k_unicode_decimal), code); }
static K_UNUSED bool k_unicode_is_digit(uint32_t code) { return k_unicode_property(k_unicode_digit, K_COUNT(k_unicode_digit), code); }
static K_UNUSED bool k_unicode_is_numeric(uint32_t code) { return k_unicode_property(k_unicode_numeric, K_COUNT(k_unicode_numeric), code); }
static K_UNUSED bool k_unicode_is_upper_property(uint32_t code) { return k_unicode_property(k_unicode_upper_property, K_COUNT(k_unicode_upper_property), code); }
static K_UNUSED bool k_unicode_is_lower_property(uint32_t code) { return k_unicode_property(k_unicode_lower_property, K_COUNT(k_unicode_lower_property), code); }
static K_UNUSED bool k_unicode_is_titlecase(uint32_t code) { return k_unicode_property(k_unicode_titlecase, K_COUNT(k_unicode_titlecase), code); }
static K_UNUSED bool k_unicode_is_xid_start(uint32_t code) { return code == '_' || k_unicode_property(k_unicode_xid_start, K_COUNT(k_unicode_xid_start), code); }
static K_UNUSED bool k_unicode_is_xid_continue(uint32_t code) { return k_unicode_property(k_unicode_xid_continue, K_COUNT(k_unicode_xid_continue), code); }
static K_UNUSED bool k_unicode_is_unprintable(uint32_t code) { return k_unicode_property(k_unicode_unprintable, K_COUNT(k_unicode_unprintable), code); }
static K_UNUSED bool k_unicode_is_alnum(uint32_t code) { return k_unicode_is_letter(code) || k_unicode_is_decimal(code) || k_unicode_is_digit(code) || k_unicode_is_numeric(code); }
/* Whitespace for strip and split: the White_Space property plus the separator controls U+001C to U+001F. */
static K_UNUSED bool k_unicode_is_blank(uint32_t code) { return k_unicode_is_space(code) || (code >= 0x1c && code <= 0x1f); }

/* A predicate over every character. An empty string fails, as it does for Python's isalpha and its kin. */
static K_UNUSED bool k_string_every(KString text, bool (*test)(uint32_t)) {
    if (!text.length) return false;
    for (size_t i = 0; i < text.length;) if (!test(k_utf8_next(text,&i))) return false;
    return true;
}
static K_UNUSED bool k_string_isalpha(KString text) { return k_string_every(text, k_unicode_is_letter); }
static K_UNUSED bool k_string_isdecimal(KString text) { return k_string_every(text, k_unicode_is_decimal); }
static K_UNUSED bool k_string_isdigit(KString text) { return k_string_every(text, k_unicode_is_digit); }
static K_UNUSED bool k_string_isnumeric(KString text) { return k_string_every(text, k_unicode_is_numeric); }
static K_UNUSED bool k_string_isalnum(KString text) { return k_string_every(text, k_unicode_is_alnum); }
static K_UNUSED bool k_string_isspace(KString text) { return k_string_every(text, k_unicode_is_blank); }
static K_UNUSED bool k_string_isascii(KString text) {
    for (size_t i = 0; i < text.length; ++i) if ((unsigned char)text.data[i] >= 0x80) return false;
    return true;
}
static K_UNUSED bool k_string_isprintable(KString text) {
    for (size_t i = 0; i < text.length;) {
        uint32_t code = k_utf8_next(text,&i);
        if (code != ' ' && k_unicode_is_unprintable(code)) return false;
    }
    return true;
}
static K_UNUSED bool k_string_isidentifier(KString text) {
    if (!text.length) return false;
    size_t i = 0;
    if (!k_unicode_is_xid_start(k_utf8_next(text,&i))) return false;
    while (i < text.length) if (!k_unicode_is_xid_continue(k_utf8_next(text,&i))) return false;
    return true;
}
/* Python's isupper: no lowercase character, and at least one uppercase. islower is the mirror image. */
static K_UNUSED bool k_string_isupper(KString text) {
    bool cased = false;
    for (size_t i = 0; i < text.length;) {
        uint32_t code = k_utf8_next(text,&i);
        if (k_unicode_is_lower_property(code) || k_unicode_is_titlecase(code)) return false;
        if (k_unicode_is_upper_property(code)) cased = true;
    }
    return cased;
}
static K_UNUSED bool k_string_islower(KString text) {
    bool cased = false;
    for (size_t i = 0; i < text.length;) {
        uint32_t code = k_utf8_next(text,&i);
        if (k_unicode_is_upper_property(code) || k_unicode_is_titlecase(code)) return false;
        if (k_unicode_is_lower_property(code)) cased = true;
    }
    return cased;
}
/* CPython's istitle: an uppercase letter may only start a word, and lowercase letters must follow one. */
static K_UNUSED bool k_string_istitle(KString text) {
    bool cased = false, previous_cased = false;
    for (size_t i = 0; i < text.length;) {
        uint32_t code = k_utf8_next(text,&i);
        if (k_unicode_is_upper_property(code) || (k_unicode_is_cased(code) && !k_unicode_is_lower_property(code))) {
            if (previous_cased) return false;
            previous_cased = cased = true;
        } else if (k_unicode_is_lower_property(code)) {
            if (!previous_cased) return false;
            previous_cased = cased = true;
        } else {
            previous_cased = false;
        }
    }
    return cased;
}

/* A growable UTF-8 buffer for the methods that change their text. */
typedef struct { char *data; size_t length, capacity; } KTextBuilder;
static K_UNUSED void k_text_reserve(KTextBuilder *b, size_t extra) {
    if (b->length + extra <= b->capacity) return;
    size_t capacity = b->capacity ? b->capacity : 64;
    while (capacity < b->length + extra) capacity *= 2;
    char *data = realloc(b->data, capacity);
    if (!data) k_fatal("Out of memory");
    b->data = data;
    b->capacity = capacity;
}
static K_UNUSED void k_text_put(KTextBuilder *b, uint32_t code) {
    k_text_reserve(b, 4);
    k_utf8_write(b->data, &b->length, code);
}
static K_UNUSED void k_text_put_bytes(KTextBuilder *b, KString text, size_t begin, size_t end) {
    k_text_reserve(b, end - begin);
    memcpy(b->data + b->length, text.data + begin, end - begin);
    b->length += end - begin;
}
static K_UNUSED KString k_text_finish(KTextBuilder *b) {
    KString result = k_string_bytes(b->data ? b->data : "", b->length);
    free(b->data);
    return result;
}
/* A character through a single-character table, or the character itself when the table has no entry. */
static K_UNUSED void k_text_put_mapped(KTextBuilder *b, const KCaseMap *table, size_t count, uint32_t code) {
    const KCaseMap *mapping = k_unicode_mapping(table, count, code);
    if (!mapping) { k_text_put(b, code); return; }
    for (unsigned j = 0; j < mapping->length; ++j) k_text_put(b, mapping->mapped[j]);
}
/* Lowercase for the character at byte `start` (Q136). Capital sigma becomes final sigma when Python's Final_Sigma
   rule applies: a cased letter comes before it, skipping case-ignorable marks, and no cased letter follows it. */
static K_UNUSED void k_text_put_lower_at(KTextBuilder *b, KString text, size_t start, uint32_t code) {
    if (code != 0x3a3) { k_text_put_mapped(b, k_unicode_lower, K_COUNT(k_unicode_lower), code); return; }
    bool before = false;
    for (size_t end = start; end > 0;) {
        size_t begin = end - 1;
        while (begin > 0 && ((unsigned char)text.data[begin] & 0xc0) == 0x80) --begin;
        size_t at = begin;
        uint32_t previous = k_utf8_next(text,&at);
        if (!k_unicode_is_ignorable(previous)) { before = k_unicode_is_cased(previous); break; }
        end = begin;
    }
    bool after = false;
    for (size_t i = start + k_utf8_width(code); i < text.length;) {
        uint32_t next = k_utf8_next(text,&i);
        if (k_unicode_is_ignorable(next)) continue;
        after = k_unicode_is_cased(next);
        break;
    }
    k_text_put(b, before && !after ? 0x3c2 : 0x3c3);
}

/* title() capitalizes each word, swapcase() exchanges case, and casefold() folds case fully (Q136). */
static K_UNUSED KString k_string_title(KString text) {
    KTextBuilder b = {0};
    bool previous_cased = false;
    for (size_t i = 0; i < text.length;) {
        size_t start = i;
        uint32_t code = k_utf8_next(text,&i);
        if (previous_cased) k_text_put_lower_at(&b, text, start, code);
        else k_text_put_mapped(&b, k_unicode_title, K_COUNT(k_unicode_title), code);
        previous_cased = k_unicode_is_cased(code);
    }
    return k_text_finish(&b);
}
static K_UNUSED KString k_string_swapcase(KString text) {
    KTextBuilder b = {0};
    for (size_t i = 0; i < text.length;) {
        size_t start = i;
        uint32_t code = k_utf8_next(text,&i);
        if (k_unicode_is_upper_property(code)) k_text_put_lower_at(&b, text, start, code);
        else if (k_unicode_is_lower_property(code)) k_text_put_mapped(&b, k_unicode_upper, K_COUNT(k_unicode_upper), code);
        else k_text_put(&b, code);
    }
    return k_text_finish(&b);
}
static K_UNUSED KString k_string_casefold(KString text) {
    KTextBuilder b = {0};
    for (size_t i = 0; i < text.length;) k_text_put_mapped(&b, k_unicode_fold, K_COUNT(k_unicode_fold), k_utf8_next(text,&i));
    return k_text_finish(&b);
}
/* capitalize() titles the first character and lowercases the rest. The rest sees the whole text, so Final_Sigma
   still knows what comes before it. */
static K_UNUSED KString k_string_capitalize(KString text) {
    if (!text.length) return k_empty;
    KTextBuilder b = {0};
    size_t i = 0;
    uint32_t code = k_utf8_next(text,&i);
    k_text_put_mapped(&b, k_unicode_title, K_COUNT(k_unicode_title), code);
    while (i < text.length) {
        size_t start = i;
        code = k_utf8_next(text,&i);
        k_text_put_lower_at(&b, text, start, code);
    }
    return k_text_finish(&b);
}

/* ljust, rjust and center pad to a width in characters. center splits the padding the way CPython does. */
static K_UNUSED KString k_string_pad(KString text, int64_t width, KString fill, int mode, KLocation loc) {
    size_t length = k_string_scalars(text);
    if (fill.length == 0 || k_string_scalars(fill) != 1) k_trap(loc, "The fill character must be exactly one character");
    if (width <= (int64_t)length) return k_string_copy(text);
    uint64_t pad = (uint64_t)width - length;
    uint64_t left = mode == 1 ? pad : mode == 2 ? pad / 2 + (pad & (uint64_t)width & 1) : 0;
    uint64_t right = pad - left;
    KTextBuilder b = {0};
    for (uint64_t i = 0; i < left; ++i) k_text_put_bytes(&b, fill, 0, fill.length);
    k_text_put_bytes(&b, text, 0, text.length);
    for (uint64_t i = 0; i < right; ++i) k_text_put_bytes(&b, fill, 0, fill.length);
    return k_text_finish(&b);
}
/* zfill keeps a leading sign and pads the digits after it with zeros. */
static K_UNUSED KString k_string_zfill(KString text, int64_t width) {
    size_t length = k_string_scalars(text);
    if (width <= (int64_t)length) return k_string_copy(text);
    size_t sign = text.length && (text.data[0] == '+' || text.data[0] == '-') ? 1 : 0;
    uint64_t pad = (uint64_t)width - length;
    KTextBuilder b = {0};
    k_text_put_bytes(&b, text, 0, sign);
    for (uint64_t i = 0; i < pad; ++i) k_text_put(&b, '0');
    k_text_put_bytes(&b, text, sign, text.length);
    return k_text_finish(&b);
}

/* substring(a, b) counts characters; a negative position counts from the end and both ends clamp,
   as Python slicing does (REVIEW2 section 3). */
static K_UNUSED KString k_string_substring(KString text, int64_t start, int64_t stop) {
    int64_t length = (int64_t)k_string_scalars(text);
    if (start < 0) start = start + length < 0 ? 0 : start + length;
    if (stop < 0) stop = stop + length < 0 ? 0 : stop + length;
    if (start > length) start = length;
    if (stop > length) stop = length;
    if (stop <= start) return k_empty;
    size_t i = 0, begin = text.length, end = text.length;
    for (int64_t n = 0; i <= text.length; ++n) {
        if (n == start) begin = i;
        if (n == stop) { end = i; break; }
        if (i == text.length) break;
        k_utf8_next(text,&i);
    }
    return k_string_slice(text,begin,end);
}
static K_UNUSED KString k_string_reversed(KString text) {
    KString result = k_string_new(text.length);
    char *out = (char *)result.data;
    size_t write = text.length;
    for (size_t i = 0; i < text.length;) {
        size_t begin = i;
        k_utf8_next(text,&i);
        write -= i - begin;
        memcpy(out + write, text.data + begin, i - begin);
    }
    return result;
}
static K_UNUSED int64_t k_string_ord(KString text, KLocation loc) {
    size_t i = 0;
    if (text.length == 0) k_trap(loc,"ord() needs one character, found an empty String");
    uint32_t code = k_utf8_next(text,&i);
    if (i != text.length) k_trap(loc,"ord() needs one character, found several");
    return (int64_t)code;
}
static K_UNUSED KString k_string_chr(int64_t code, KLocation loc) {
    if (code < 0 || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) k_trap(loc,"chr() needs a Unicode scalar value");
    KString result = k_string_new(k_utf8_width((uint32_t)code));
    size_t index = 0;
    k_utf8_write((char *)result.data,&index,(uint32_t)code);
    return result;
}
/* The bytes from start to stop; both ends must fall between characters. */
static K_UNUSED KString k_string_byte_slice(KString text, int64_t start, int64_t stop, KLocation loc) {
    if (start < 0 || stop < start || (uint64_t)stop > text.length) k_trap(loc,"String byte range out of range");
    bool boundary = ((size_t)start == text.length || ((unsigned char)text.data[start] & 0xc0) != 0x80)
        && ((size_t)stop == text.length || ((unsigned char)text.data[stop] & 0xc0) != 0x80);
    if (!boundary) k_trap(loc,"String byte range splits a character");
    return k_string_slice(text,(size_t)start,(size_t)stop);
}
static K_UNUSED int64_t k_string_byte_at(KString text, int64_t index, KLocation loc) {
    if (index < 0 || (uint64_t)index >= text.length) k_trap(loc,"String byte index out of range");
    return (unsigned char)text.data[index];
}

/* strip, lstrip and rstrip remove whitespace, or with chars, any character listed there (Q136). */
static K_UNUSED bool k_string_in_set(uint32_t code, KString set) {
    for (size_t i = 0; i < set.length;) if (k_utf8_next(set,&i) == code) return true;
    return false;
}
static K_UNUSED KString k_string_strip(KString text, KString chars, bool has_chars, bool left, bool right) {
    size_t begin = 0, end = text.length;
    if (left) {
        while (begin < end) {
            size_t next = begin;
            uint32_t code = k_utf8_next(text,&next);
            if (!(has_chars ? k_string_in_set(code, chars) : k_unicode_is_blank(code))) break;
            begin = next;
        }
    }
    if (right) {
        while (end > begin) {
            size_t previous = end - 1;
            while (previous > begin && ((unsigned char)text.data[previous] & 0xc0) == 0x80) --previous;
            size_t cursor = previous;
            uint32_t code = k_utf8_next(text,&cursor);
            if (!(has_chars ? k_string_in_set(code, chars) : k_unicode_is_blank(code))) break;
            end = previous;
        }
    }
    return k_string_slice(text, begin, end);
}
/* expandtabs moves each tab to the next multiple of tabsize columns, and a newline restarts the count. */
static K_UNUSED KString k_string_expandtabs(KString text, int64_t tabsize) {
    KTextBuilder b = {0};
    size_t column = 0;
    for (size_t i = 0; i < text.length;) {
        uint32_t code = k_utf8_next(text,&i);
        if (code == '\t') {
            if (tabsize > 0) {
                size_t stop = column + (size_t)tabsize - column % (size_t)tabsize;
                for (; column < stop; ++column) k_text_put(&b, ' ');
            }
        } else {
            k_text_put(&b, code);
            column = code == '\n' || code == '\r' ? 0 : column + 1;
        }
    }
    return k_text_finish(&b);
}
static K_UNUSED KString k_string_removeprefix(KString text, KString prefix) {
    if (prefix.length <= text.length && memcmp(text.data, prefix.data, prefix.length) == 0) return k_string_slice(text, prefix.length, text.length);
    return k_string_copy(text);
}
static K_UNUSED KString k_string_removesuffix(KString text, KString suffix) {
    if (suffix.length <= text.length && memcmp(text.data + text.length - suffix.length, suffix.data, suffix.length) == 0) return k_string_slice(text, 0, text.length - suffix.length);
    return k_string_copy(text);
}
/* The byte offset of a match, or SIZE_MAX. reverse finds the last match. */
static K_UNUSED size_t k_string_search(KString text, KString sub, bool reverse) {
    if (sub.length > text.length) return SIZE_MAX;
    size_t last = text.length - sub.length;
    for (size_t step = 0; step <= last; ++step) {
        size_t at = reverse ? last - step : step;
        if (memcmp(text.data + at, sub.data, sub.length) == 0) return at;
    }
    return SIZE_MAX;
}
/* Converts a byte offset to a character index. */
static K_UNUSED int64_t k_string_index_of(KString text, size_t offset) {
    if (offset == SIZE_MAX) return -1;
    /* Count the characters in place; a slice here allocated a String that was never freed. */
    KString prefix = { text.data, offset, NULL };
    return (int64_t)k_string_scalars(prefix);
}
static K_UNUSED int64_t k_string_find(KString text, KString sub, bool reverse) {
    return k_string_index_of(text, k_string_search(text, sub, reverse));
}
/* count() counts matches that do not overlap. The empty string matches between every two characters. */
static K_UNUSED int64_t k_string_count(KString text, KString sub) {
    if (!sub.length) return (int64_t)k_string_scalars(text) + 1;
    int64_t count = 0;
    size_t i = 0;
    while (i + sub.length <= text.length) {
        if (memcmp(text.data + i, sub.data, sub.length) == 0) { ++count; i += sub.length; }
        else ++i;
    }
    return count;
}
/* replace(old, new) changes every match. The empty pattern matches before each character and at the end. */
static K_UNUSED KString k_string_replace(KString text, KString old, KString replacement) {
    KTextBuilder b = {0};
    if (!old.length) {
        for (size_t i = 0; i <= text.length;) {
            k_text_put_bytes(&b, replacement, 0, replacement.length);
            if (i == text.length) break;
            size_t next = i;
            k_utf8_next(text,&next);
            k_text_put_bytes(&b, text, i, next);
            i = next;
        }
        return k_text_finish(&b);
    }
    size_t i = 0;
    while (i < text.length) {
        if (i + old.length <= text.length && memcmp(text.data + i, old.data, old.length) == 0) {
            k_text_put_bytes(&b, replacement, 0, replacement.length);
            i += old.length;
        } else {
            k_text_put_bytes(&b, text, i, i + 1);
            ++i;
        }
    }
    return k_text_finish(&b);
}
/* partition splits at the first (or, with reverse, the last) separator. Without one, the text stays whole. */
static K_UNUSED void k_string_partition(KString text, KString sep, bool reverse, KString *before, KString *middle, KString *after) {
    size_t at = k_string_search(text, sep, reverse);
    if (at == SIZE_MAX) {
        if (reverse) { *before = k_empty; *middle = k_empty; *after = k_string_copy(text); }
        else { *before = k_string_copy(text); *middle = k_empty; *after = k_empty; }
        return;
    }
    *before = k_string_slice(text, 0, at);
    *middle = k_string_copy(sep);
    *after = k_string_slice(text, at + sep.length, text.length);
}
static K_UNUSED KString k_string_repeat(KString text, int64_t count) {
    KTextBuilder b = {0};
    for (int64_t i = 0; i < count; ++i) k_text_put_bytes(&b, text, 0, text.length);
    return k_text_finish(&b);
}
