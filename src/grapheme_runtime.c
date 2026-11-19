/* Extended grapheme clusters, UAX #29 revision 47 (Unicode 17). */
static unsigned k_grapheme_property(const KGraphemeRange *ranges, size_t count, uint32_t code) {
    size_t first = 0;
    while (first < count) {
        size_t middle = first + (count - first) / 2;
        if (code < ranges[middle].first) count = middle;
        else if (code > ranges[middle].last) first = middle + 1;
        else return ranges[middle].value;
    }
    return 0;
}
#define K_GRAPHEME(table, code) k_grapheme_property(k_grapheme_##table, sizeof k_grapheme_##table / sizeof *k_grapheme_##table, code)
static size_t k_grapheme_next(KString text, size_t at) {
    if (at == text.length) return at;
    uint32_t code = k_utf8_next(text, &at);
    unsigned previous = K_GRAPHEME(gcb, code);
    size_t regional = previous == G_REGIONAL_INDICATOR;
    bool pictographic = K_GRAPHEME(pictographic, code) != 0, after_zwj = false;
    unsigned indic = K_GRAPHEME(incb, code);
    bool consonant = indic == 1, linker = false;
    while (at < text.length) {
        size_t next = at;
        code = k_utf8_next(text, &next);
        unsigned current = K_GRAPHEME(gcb, code);
        unsigned next_indic = K_GRAPHEME(incb, code);
        bool next_pictographic = K_GRAPHEME(pictographic, code) != 0;
        bool keep;
        if (previous == G_CR && current == G_LF) keep = true;                       /* GB3 */
        else if (previous == G_CR || previous == G_LF || previous == G_CONTROL ||
                 current == G_CR || current == G_LF || current == G_CONTROL) keep = false; /* GB4/5 */
        else if (previous == G_L && (current == G_L || current == G_V || current == G_LV || current == G_LVT)) keep = true;
        else if ((previous == G_LV || previous == G_V) && (current == G_V || current == G_T)) keep = true;
        else if ((previous == G_LVT || previous == G_T) && current == G_T) keep = true;
        else if (current == G_EXTEND || current == G_ZWJ || current == G_SPACINGMARK || previous == G_PREPEND) keep = true;
        else if (consonant && linker && next_indic == 1) keep = true;              /* GB9c */
        else if (after_zwj && next_pictographic) keep = true;                     /* GB11 */
        else keep = previous == G_REGIONAL_INDICATOR && current == G_REGIONAL_INDICATOR && regional % 2 == 1;
        if (!keep) break;
        after_zwj = current == G_ZWJ && pictographic;
        if (current != G_EXTEND) pictographic = next_pictographic;
        if (next_indic == 1) { consonant = true; linker = false; }
        else if (consonant && next_indic == 3) linker = true;
        else if (next_indic != 2) { consonant = false; linker = false; }
        regional = current == G_REGIONAL_INDICATOR ? regional + 1 : 0;
        previous = current;
        at = next;
    }
    return at;
}
static K_UNUSED int64_t k_text_next_boundary(KString text, int64_t start, bool graphemes, KLocation loc) {
    if (start < 0 || (uint64_t)start > text.length || ((size_t)start < text.length && ((unsigned char)text.data[start] & 0xc0) == 0x80))
        k_trap(loc, "Text cursor is not at a UTF-8 boundary");
    size_t at = (size_t)start;
    if (graphemes) at = k_grapheme_next(text, at);
    else if (at < text.length) k_utf8_next(text, &at);
    return (int64_t)at;
}
#undef K_GRAPHEME
