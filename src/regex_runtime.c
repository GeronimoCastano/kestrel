/* std.regex: a Pike VM (Thompson NFA with capture slots). Matching takes time linear in
   the text for a given pattern, so no pattern backtracks exponentially. Matches are
   leftmost-first, as in Python and JavaScript. There are no backreferences or lookaround. */

enum { KRX_CHAR, KRX_ANY, KRX_CLASS, KRX_SPLIT, KRX_JMP, KRX_SAVE, KRX_MATCH, KRX_BOL, KRX_EOL, KRX_WORD, KRX_NOTWORD };
enum { KRXN_EMPTY, KRXN_CHAR, KRXN_ANY, KRXN_CLASS, KRXN_CAT, KRXN_ALT, KRXN_REPEAT, KRXN_GROUP, KRXN_BOL, KRXN_EOL, KRXN_WORD, KRXN_NOTWORD };
#define KRX_MAX_CODE 60000
#define KRX_MAX_GROUPS 100
#define KRX_MAX_REPEAT 1000

typedef struct { uint32_t lo, hi; } KRxRange;
typedef struct { KRxRange *ranges; size_t count, capacity; bool negated; } KRxClass;
typedef struct { int op; uint32_t c; int x, y; } KRxInst;
typedef struct { int kind; uint32_t c; int cls, group, min, max, a, b; bool greedy; } KRxNode;
typedef struct {
    KRxInst *code; size_t length, capacity;
    KRxClass *classes; size_t class_count, class_capacity;
    KRxNode *nodes; size_t node_count, node_capacity;
    int groups; bool icase; char error[200];
    const char *src; size_t at, end;
} KRxProgram;

static uint32_t k_rx_lower(uint32_t c) { return c >= 'A' && c <= 'Z' ? c + 32 : c; }
static uint32_t k_rx_upper(uint32_t c) { return c >= 'a' && c <= 'z' ? c - 32 : c; }
static bool k_rx_word(uint32_t c) { return c == '_' || (c >= '0' && c <= '9') || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z'); }

static bool k_rx_fail(KRxProgram *p, const char *message) {
    if (!p->error[0]) snprintf(p->error, sizeof p->error, "regex: %s at position %zu", message, p->at);
    return false;
}
static int k_rx_node(KRxProgram *p, int kind) {
    if (p->node_count == p->node_capacity) {
        size_t capacity = p->node_capacity ? p->node_capacity * 2 : 32;
        KRxNode *grown = realloc(p->nodes, capacity * sizeof *grown);
        if (!grown) k_fatal("Out of memory");
        p->nodes = grown;
        p->node_capacity = capacity;
    }
    KRxNode node = { kind, 0, -1, -1, 0, 0, -1, -1, true };
    p->nodes[p->node_count] = node;
    return (int)p->node_count++;
}
static int k_rx_class_new(KRxProgram *p, bool negated) {
    if (p->class_count == p->class_capacity) {
        size_t capacity = p->class_capacity ? p->class_capacity * 2 : 8;
        KRxClass *grown = realloc(p->classes, capacity * sizeof *grown);
        if (!grown) k_fatal("Out of memory");
        p->classes = grown;
        p->class_capacity = capacity;
    }
    KRxClass empty = { NULL, 0, 0, negated };
    p->classes[p->class_count] = empty;
    return (int)p->class_count++;
}
static void k_rx_range(KRxProgram *p, int cls, uint32_t lo, uint32_t hi) {
    KRxClass *c = &p->classes[cls];
    if (c->count == c->capacity) {
        size_t capacity = c->capacity ? c->capacity * 2 : 4;
        KRxRange *grown = realloc(c->ranges, capacity * sizeof *grown);
        if (!grown) k_fatal("Out of memory");
        c->ranges = grown;
        c->capacity = capacity;
    }
    KRxRange range = { lo, hi };
    c->ranges[c->count++] = range;
}
/* \d, \w and \s as ranges. */
static void k_rx_shorthand(KRxProgram *p, int cls, char kind) {
    if (kind == 'd') k_rx_range(p, cls, '0', '9');
    else if (kind == 'w') { k_rx_range(p, cls, 'a', 'z'); k_rx_range(p, cls, 'A', 'Z'); k_rx_range(p, cls, '0', '9'); k_rx_range(p, cls, '_', '_'); }
    else { k_rx_range(p, cls, ' ', ' '); k_rx_range(p, cls, '\t', '\r'); }
}
static bool k_rx_more(KRxProgram *p) { return p->at < p->end; }
static uint32_t k_rx_next(KRxProgram *p) {
    KString text = { p->src, p->end, NULL };
    size_t i = p->at;
    uint32_t c = k_utf8_next(text, &i);
    p->at = i;
    return c;
}
static uint32_t k_rx_peek(KRxProgram *p) {
    size_t saved = p->at;
    uint32_t c = k_rx_next(p);
    p->at = saved;
    return c;
}
static bool k_rx_hex(KRxProgram *p, int digits, uint32_t *out) {
    uint32_t value = 0;
    for (int i = 0; i < digits; ++i) {
        if (!k_rx_more(p)) return k_rx_fail(p, "expected hex digits");
        char c = p->src[p->at++];
        int d = c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 87 : c >= 'A' && c <= 'F' ? c - 55 : -1;
        if (d < 0) return k_rx_fail(p, "expected hex digits");
        value = value * 16 + (uint32_t)d;
    }
    *out = value;
    return true;
}
/* An escape that stands for one character, after the backslash. */
static bool k_rx_escape_char(KRxProgram *p, uint32_t e, uint32_t *out) {
    switch (e) {
        case 'n': *out = '\n'; return true;
        case 't': *out = '\t'; return true;
        case 'r': *out = '\r'; return true;
        case 'f': *out = '\f'; return true;
        case 'v': *out = '\v'; return true;
        case '0': *out = 0; return true;
        case 'x': return k_rx_hex(p, 2, out);
        case 'u': return k_rx_hex(p, 4, out);
        default:
            if (e < 128 && !k_rx_word(e)) { *out = e; return true; }
            return k_rx_fail(p, "unknown escape");
    }
}

static int k_rx_alt(KRxProgram *p, int depth);

static int k_rx_class(KRxProgram *p) {
    bool negated = k_rx_more(p) && p->src[p->at] == '^';
    if (negated) ++p->at;
    int cls = k_rx_class_new(p, negated);
    bool first = true;
    for (;;) {
        if (!k_rx_more(p)) { k_rx_fail(p, "a [ class is never closed"); return -1; }
        uint32_t c = k_rx_next(p);
        if (c == ']' && !first) break;
        first = false;
        uint32_t lo = c;
        if (c == '\\') {
            if (!k_rx_more(p)) { k_rx_fail(p, "a pattern cannot end with \\"); return -1; }
            uint32_t e = k_rx_next(p);
            if (e == 'd' || e == 'w' || e == 's') { k_rx_shorthand(p, cls, (char)e); continue; }
            if (e == 'D' || e == 'W' || e == 'S') { k_rx_fail(p, "\\D, \\W and \\S cannot appear inside [ ]"); return -1; }
            if (e == 'b') lo = 8;
            else if (!k_rx_escape_char(p, e, &lo)) return -1;
        }
        uint32_t hi = lo;
        if (k_rx_more(p) && p->src[p->at] == '-' && p->at + 1 < p->end && p->src[p->at + 1] != ']') {
            ++p->at;
            hi = k_rx_next(p);
            if (hi == '\\') {
                if (!k_rx_more(p)) { k_rx_fail(p, "a pattern cannot end with \\"); return -1; }
                if (!k_rx_escape_char(p, k_rx_next(p), &hi)) return -1;
            }
            if (hi < lo) { k_rx_fail(p, "a range in [ ] runs backwards"); return -1; }
        }
        k_rx_range(p, cls, lo, hi);
    }
    int node = k_rx_node(p, KRXN_CLASS);
    p->nodes[node].cls = cls;
    return node;
}

static int k_rx_atom(KRxProgram *p, int depth) {
    uint32_t c = k_rx_next(p);
    if (c == '(') {
        if (p->at + 1 < p->end && p->src[p->at] == '?' && p->src[p->at + 1] == ':') {
            p->at += 2;
            int inner = k_rx_alt(p, depth + 1);
            if (inner < 0) return -1;
            if (!k_rx_more(p) || p->src[p->at] != ')') { k_rx_fail(p, "a ( group is never closed"); return -1; }
            ++p->at;
            return inner;
        }
        if (p->at + 2 < p->end && p->src[p->at] == '?' && p->src[p->at + 1] == 'i' && p->src[p->at + 2] == ')') {
            p->at += 3;
            p->icase = true;
            return k_rx_node(p, KRXN_EMPTY);
        }
        if (k_rx_more(p) && p->src[p->at] == '?') { k_rx_fail(p, "only (?:...) and (?i) groups are supported"); return -1; }
        if (p->groups >= KRX_MAX_GROUPS) { k_rx_fail(p, "too many groups"); return -1; }
        int group = p->groups++;
        int inner = k_rx_alt(p, depth + 1);
        if (inner < 0) return -1;
        if (!k_rx_more(p) || p->src[p->at] != ')') { k_rx_fail(p, "a ( group is never closed"); return -1; }
        ++p->at;
        int node = k_rx_node(p, KRXN_GROUP);
        p->nodes[node].group = group;
        p->nodes[node].a = inner;
        return node;
    }
    if (c == '[') return k_rx_class(p);
    if (c == '.') return k_rx_node(p, KRXN_ANY);
    if (c == '^') return k_rx_node(p, KRXN_BOL);
    if (c == '$') return k_rx_node(p, KRXN_EOL);
    if (c == '*' || c == '+' || c == '?' || c == '{') { k_rx_fail(p, "nothing to repeat"); return -1; }
    if (c == ')') { k_rx_fail(p, "a ) has no matching ("); return -1; }
    if (c == '\\') {
        if (!k_rx_more(p)) { k_rx_fail(p, "a pattern cannot end with \\"); return -1; }
        uint32_t e = k_rx_next(p);
        if (e == 'b') return k_rx_node(p, KRXN_WORD);
        if (e == 'B') return k_rx_node(p, KRXN_NOTWORD);
        if (e == 'd' || e == 'w' || e == 's' || e == 'D' || e == 'W' || e == 'S') {
            int cls = k_rx_class_new(p, e == 'D' || e == 'W' || e == 'S');
            k_rx_shorthand(p, cls, (char)k_rx_lower(e));
            int node = k_rx_node(p, KRXN_CLASS);
            p->nodes[node].cls = cls;
            return node;
        }
        if (!k_rx_escape_char(p, e, &c)) return -1;
    }
    int node = k_rx_node(p, KRXN_CHAR);
    p->nodes[node].c = c;
    return node;
}

static bool k_rx_number(KRxProgram *p, int *out) {
    if (!k_rx_more(p) || p->src[p->at] < '0' || p->src[p->at] > '9') return false;
    int value = 0;
    while (k_rx_more(p) && p->src[p->at] >= '0' && p->src[p->at] <= '9') {
        value = value * 10 + (p->src[p->at++] - '0');
        if (value > KRX_MAX_REPEAT) { k_rx_fail(p, "a repeat count above 1000"); return false; }
    }
    *out = value;
    return true;
}

static int k_rx_repeat(KRxProgram *p, int depth) {
    int atom = k_rx_atom(p, depth);
    if (atom < 0) return -1;
    if (!k_rx_more(p)) return atom;
    char q = p->src[p->at];
    int min, max;
    if (q == '*') { min = 0; max = -1; ++p->at; }
    else if (q == '+') { min = 1; max = -1; ++p->at; }
    else if (q == '?') { min = 0; max = 1; ++p->at; }
    else if (q == '{') {
        size_t saved = p->at++;
        if (!k_rx_number(p, &min)) {
            if (p->error[0]) return -1;
            p->at = saved;
            return atom;
        }
        max = min;
        if (k_rx_more(p) && p->src[p->at] == ',') {
            ++p->at;
            if (!k_rx_number(p, &max)) {
                if (p->error[0]) return -1;
                max = -1;
            }
        }
        if (!k_rx_more(p) || p->src[p->at] != '}') { k_rx_fail(p, "expected } after a repeat count"); return -1; }
        ++p->at;
        if (max >= 0 && max < min) { k_rx_fail(p, "a repeat range runs backwards"); return -1; }
    } else return atom;
    bool greedy = true;
    if (k_rx_more(p) && p->src[p->at] == '?') { greedy = false; ++p->at; }
    if (k_rx_more(p) && strchr("*+?", p->src[p->at])) { k_rx_fail(p, "a repeat cannot be repeated"); return -1; }
    int node = k_rx_node(p, KRXN_REPEAT);
    p->nodes[node].a = atom;
    p->nodes[node].min = min;
    p->nodes[node].max = max;
    p->nodes[node].greedy = greedy;
    return node;
}

static int k_rx_cat(KRxProgram *p, int depth) {
    int result = k_rx_node(p, KRXN_EMPTY);
    while (k_rx_more(p) && p->src[p->at] != '|' && p->src[p->at] != ')') {
        int next = k_rx_repeat(p, depth);
        if (next < 0) return -1;
        int cat = k_rx_node(p, KRXN_CAT);
        p->nodes[cat].a = result;
        p->nodes[cat].b = next;
        result = cat;
    }
    return result;
}

static int k_rx_alt(KRxProgram *p, int depth) {
    if (depth > 200) { k_rx_fail(p, "groups nest too deeply"); return -1; }
    int left = k_rx_cat(p, depth);
    if (left < 0) return -1;
    while (k_rx_more(p) && p->src[p->at] == '|') {
        ++p->at;
        int right = k_rx_cat(p, depth);
        if (right < 0) return -1;
        int alt = k_rx_node(p, KRXN_ALT);
        p->nodes[alt].a = left;
        p->nodes[alt].b = right;
        left = alt;
    }
    return left;
}

static int k_rx_emit(KRxProgram *p, int op, uint32_t c, int x, int y) {
    if (p->length >= KRX_MAX_CODE) { k_rx_fail(p, "the pattern is too large"); return -1; }
    if (p->length == p->capacity) {
        size_t capacity = p->capacity ? p->capacity * 2 : 64;
        KRxInst *grown = realloc(p->code, capacity * sizeof *grown);
        if (!grown) k_fatal("Out of memory");
        p->code = grown;
        p->capacity = capacity;
    }
    KRxInst inst = { op, c, x, y };
    p->code[p->length] = inst;
    return (int)p->length++;
}

static bool k_rx_compile_node(KRxProgram *p, int index, int depth) {
    if (depth > 400) return k_rx_fail(p, "the pattern nests too deeply");
    KRxNode n = p->nodes[index];
    switch (n.kind) {
        case KRXN_EMPTY: return true;
        case KRXN_CHAR: return k_rx_emit(p, KRX_CHAR, n.c, 0, 0) >= 0;
        case KRXN_ANY: return k_rx_emit(p, KRX_ANY, 0, 0, 0) >= 0;
        case KRXN_CLASS: return k_rx_emit(p, KRX_CLASS, 0, n.cls, 0) >= 0;
        case KRXN_BOL: return k_rx_emit(p, KRX_BOL, 0, 0, 0) >= 0;
        case KRXN_EOL: return k_rx_emit(p, KRX_EOL, 0, 0, 0) >= 0;
        case KRXN_WORD: return k_rx_emit(p, KRX_WORD, 0, 0, 0) >= 0;
        case KRXN_NOTWORD: return k_rx_emit(p, KRX_NOTWORD, 0, 0, 0) >= 0;
        case KRXN_CAT: return k_rx_compile_node(p, n.a, depth + 1) && k_rx_compile_node(p, n.b, depth + 1);
        case KRXN_GROUP:
            return k_rx_emit(p, KRX_SAVE, 0, 2 * n.group, 0) >= 0 && k_rx_compile_node(p, n.a, depth + 1) &&
                   k_rx_emit(p, KRX_SAVE, 0, 2 * n.group + 1, 0) >= 0;
        case KRXN_ALT: {
            int split = k_rx_emit(p, KRX_SPLIT, 0, 0, 0);
            if (split < 0) return false;
            p->code[split].x = (int)p->length;
            if (!k_rx_compile_node(p, n.a, depth + 1)) return false;
            int jump = k_rx_emit(p, KRX_JMP, 0, 0, 0);
            if (jump < 0) return false;
            p->code[split].y = (int)p->length;
            if (!k_rx_compile_node(p, n.b, depth + 1)) return false;
            p->code[jump].x = (int)p->length;
            return true;
        }
        case KRXN_REPEAT: {
            for (int i = 0; i < n.min; ++i) if (!k_rx_compile_node(p, n.a, depth + 1)) return false;
            if (n.max < 0) {
                int split = k_rx_emit(p, KRX_SPLIT, 0, 0, 0);
                if (split < 0 || !k_rx_compile_node(p, n.a, depth + 1)) return false;
                if (k_rx_emit(p, KRX_JMP, 0, split, 0) < 0) return false;
                int body = split + 1, after = (int)p->length;
                p->code[split].x = n.greedy ? body : after;
                p->code[split].y = n.greedy ? after : body;
                /* A loop head remembers its exit: an iteration that matched nothing leaves
                   the loop there, as in Perl and Python. */
                p->code[split].c = (uint32_t)after + 1;
                return true;
            }
            int optional = n.max - n.min;
            int *splits = malloc((size_t)(optional > 0 ? optional : 1) * sizeof *splits);
            if (!splits) k_fatal("Out of memory");
            for (int i = 0; i < optional; ++i) {
                splits[i] = k_rx_emit(p, KRX_SPLIT, 0, 0, 0);
                if (splits[i] < 0 || !k_rx_compile_node(p, n.a, depth + 1)) { free(splits); return false; }
            }
            int after = (int)p->length;
            for (int i = 0; i < optional; ++i) {
                p->code[splits[i]].x = n.greedy ? splits[i] + 1 : after;
                p->code[splits[i]].y = n.greedy ? after : splits[i] + 1;
            }
            free(splits);
            return true;
        }
    }
    return k_rx_fail(p, "internal error");
}

static void k_rx_free(KRxProgram *p) {
    free(p->code);
    for (size_t i = 0; i < p->class_count; ++i) free(p->classes[i].ranges);
    free(p->classes);
    free(p->nodes);
    free(p);
}

static KRxProgram *k_rx_compile(KString pattern) {
    KRxProgram *p = calloc(1, sizeof *p);
    if (!p) k_fatal("Out of memory");
    p->src = pattern.data;
    p->end = pattern.length;
    p->groups = 1;
    int root = k_rx_alt(p, 0);
    if (root >= 0 && k_rx_more(p)) k_rx_fail(p, "a ) has no matching (");
    if (!p->error[0]) {
        k_rx_emit(p, KRX_SAVE, 0, 0, 0);
        k_rx_compile_node(p, root, 0);
        k_rx_emit(p, KRX_SAVE, 0, 1, 0);
        k_rx_emit(p, KRX_MATCH, 0, 0, 0);
    }
    free(p->nodes);
    p->nodes = NULL;
    p->node_count = p->node_capacity = 0;
    return p;
}

/* The last pattern each thread used, so a findAll loop compiles it once. */
static _Thread_local KRxProgram *k_rx_cached;
static _Thread_local char *k_rx_cached_pattern;
static _Thread_local size_t k_rx_cached_length;

static KRxProgram *k_rx_program(KString pattern) {
    if (k_rx_cached && k_rx_cached_length == pattern.length &&
        (pattern.length == 0 || memcmp(k_rx_cached_pattern, pattern.data, pattern.length) == 0))
        return k_rx_cached;
    if (k_rx_cached) {
        k_rx_free(k_rx_cached);
        free(k_rx_cached_pattern);
    }
    k_rx_cached = k_rx_compile(pattern);
    k_rx_cached_pattern = malloc(pattern.length + 1);
    if (!k_rx_cached_pattern) k_fatal("Out of memory");
    if (pattern.length) memcpy(k_rx_cached_pattern, pattern.data, pattern.length);
    k_rx_cached_length = pattern.length;
    return k_rx_cached;
}

static bool k_rx_class_has(const KRxProgram *p, int cls, uint32_t c) {
    const KRxClass *k = &p->classes[cls];
    bool found = false;
    for (size_t i = 0; i < k->count && !found; ++i) {
        found = (c >= k->ranges[i].lo && c <= k->ranges[i].hi);
        if (!found && p->icase) {
            uint32_t a = k_rx_lower(c), b = k_rx_upper(c);
            found = (a >= k->ranges[i].lo && a <= k->ranges[i].hi) || (b >= k->ranges[i].lo && b <= k->ranges[i].hi);
        }
    }
    return found != k->negated;
}

typedef struct { int pc; int64_t *caps; } KRxThread;
typedef struct { KRxThread *threads; size_t count; int64_t *storage; } KRxList;
typedef struct { int pc; int slot; int64_t value; } KRxStep;

static bool k_rx_boundary(KString text, size_t pos) {
    bool before = false, after = false;
    if (pos > 0) {
        size_t i = pos - 1;
        while (i > 0 && ((unsigned char)text.data[i] & 0xc0) == 0x80) --i;
        size_t j = i;
        before = k_rx_word(k_utf8_next(text, &j));
    }
    if (pos < text.length) {
        size_t j = pos;
        after = k_rx_word(k_utf8_next(text, &j));
    }
    return before != after;
}

/* Adds the thread at `pc` and every thread its jumps, splits, saves and assertions reach. */
static void k_rx_add(const KRxProgram *p, KRxList *list, int pc0, int64_t *caps, size_t pos, KString text,
                     int *marks, int generation, KRxStep *stack, size_t slots) {
    size_t top = 0;
    KRxStep start = { pc0, -1, 0 };
    stack[top++] = start;
    while (top) {
        KRxStep step = stack[--top];
        if (step.slot >= 0) { caps[step.slot] = step.value; continue; }
        int pc = step.pc;
        if (marks[pc] == generation) continue;
        marks[pc] = generation;
        KRxInst in = p->code[pc];
        switch (in.op) {
            case KRX_JMP: {
                /* Back at a loop head already reached at this position: the iteration
                   matched nothing, so continue after the loop. */
                const KRxInst *head = &p->code[in.x];
                int target = head->op == KRX_SPLIT && head->c && marks[in.x] == generation ? (int)head->c - 1 : in.x;
                KRxStep s = { target, -1, 0 };
                stack[top++] = s;
                break;
            }
            case KRX_SPLIT: { KRxStep second = { in.y, -1, 0 }, first = { in.x, -1, 0 }; stack[top++] = second; stack[top++] = first; break; }
            case KRX_SAVE: {
                KRxStep restore = { 0, in.x, caps[in.x] };
                stack[top++] = restore;
                caps[in.x] = (int64_t)pos;
                KRxStep s = { pc + 1, -1, 0 };
                stack[top++] = s;
                break;
            }
            case KRX_BOL: if (pos == 0) { KRxStep s = { pc + 1, -1, 0 }; stack[top++] = s; } break;
            case KRX_EOL: if (pos == text.length) { KRxStep s = { pc + 1, -1, 0 }; stack[top++] = s; } break;
            case KRX_WORD: if (k_rx_boundary(text, pos)) { KRxStep s = { pc + 1, -1, 0 }; stack[top++] = s; } break;
            case KRX_NOTWORD: if (!k_rx_boundary(text, pos)) { KRxStep s = { pc + 1, -1, 0 }; stack[top++] = s; } break;
            default: {
                KRxThread *t = &list->threads[list->count];
                t->pc = pc;
                t->caps = list->storage + list->count * slots;
                memcpy(t->caps, caps, slots * sizeof *caps);
                ++list->count;
            }
        }
    }
}

/* The leftmost-first match at or after byte `start`; `out` receives 2 slots per group. */
static bool k_rx_search(const KRxProgram *p, KString text, size_t start, int64_t *out) {
    size_t slots = (size_t)p->groups * 2, n = p->length;
    KRxList current = { malloc(n * sizeof(KRxThread)), 0, malloc(n * slots * sizeof(int64_t)) };
    KRxList next = { malloc(n * sizeof(KRxThread)), 0, malloc(n * slots * sizeof(int64_t)) };
    int *marks = malloc(n * sizeof *marks);
    KRxStep *stack = malloc((3 * n + 4) * sizeof *stack);
    int64_t *scratch = malloc(slots * sizeof *scratch);
    if (!current.threads || !current.storage || !next.threads || !next.storage || !marks || !stack || !scratch) k_fatal("Out of memory");
    for (size_t i = 0; i < n; ++i) marks[i] = -1;
    bool matched = false;
    int generation = 0;
    size_t pos = start;
    for (;;) {
        if (!matched) {
            for (size_t i = 0; i < slots; ++i) scratch[i] = -1;
            k_rx_add(p, &current, 0, scratch, pos, text, marks, generation, stack, slots);
        }
        /* With no live thread, a found match is final; otherwise try the next start. */
        if (current.count == 0 && (matched || pos >= text.length)) break;
        uint32_t c = 0;
        size_t after = pos;
        bool more = pos < text.length;
        if (more) c = k_utf8_next(text, &after);
        ++generation;
        next.count = 0;
        for (size_t i = 0; i < current.count; ++i) {
            KRxThread *t = &current.threads[i];
            KRxInst in = p->code[t->pc];
            bool step = false;
            if (in.op == KRX_MATCH) {
                memcpy(out, t->caps, slots * sizeof *out);
                matched = true;
                break;
            }
            if (!more) continue;
            if (in.op == KRX_CHAR) step = c == in.c || (p->icase && k_rx_lower(c) == k_rx_lower(in.c));
            else if (in.op == KRX_ANY) step = c != '\n';
            else if (in.op == KRX_CLASS) step = k_rx_class_has(p, in.x, c);
            if (step) k_rx_add(p, &next, t->pc + 1, t->caps, after, text, marks, generation, stack, slots);
        }
        KRxList swap = current;
        current = next;
        next = swap;
        if (!more) {
            /* Threads that reached MATCH at the end of the text. */
            for (size_t i = 0; i < current.count; ++i) {
                if (p->code[current.threads[i].pc].op == KRX_MATCH) {
                    memcpy(out, current.threads[i].caps, slots * sizeof *out);
                    matched = true;
                    break;
                }
            }
            break;
        }
        pos = after;
    }
    free(current.threads); free(current.storage); free(next.threads); free(next.storage);
    free(marks); free(stack); free(scratch);
    return matched;
}

/* The error message for an invalid pattern, or "" for a valid one. */
static K_UNUSED KString k_rx_check(KString pattern) {
    KRxProgram *p = k_rx_program(pattern);
    return k_string_bytes(p->error, strlen(p->error));
}
static K_UNUSED KRxProgram *k_rx_valid(KString pattern, KLocation loc) {
    KRxProgram *p = k_rx_program(pattern);
    if (p->error[0]) k_trap(loc, p->error);
    return p;
}
