/* Stable sorting of item indices, the algorithm Python uses (Timsort, after CPython's listobject.c).
   The caller names items by index and supplies a comparison of two indices, so one routine sorts
   every element type. Equal items keep their order. */
#define K_SORT_MIN_GALLOP 7
#define K_SORT_MAX_RUNS 96

typedef struct { size_t base, length; } KSortRun;
typedef struct {
    size_t *keys;
    size_t *buffer;
    size_t buffer_length;
    int64_t (*compare)(void *, size_t, size_t);
    void *context;
    size_t min_gallop;
} KSorter;

static __attribute__((unused)) int k_sort_less(KSorter *sorter, size_t left, size_t right) {
    return sorter->compare(sorter->context, left, right) < 0;
}

/* The position in the sorted run that a key goes before, once equal items are passed. */
static __attribute__((unused)) size_t k_sort_gallop_left(KSorter *sorter, size_t key, const size_t *keys, size_t length, size_t hint) {
    ptrdiff_t last = 0, offset = 1, limit, previous;
    if (k_sort_less(sorter, keys[hint], key)) {
        limit = (ptrdiff_t)length - (ptrdiff_t)hint;
        while (offset < limit && k_sort_less(sorter, keys[(ptrdiff_t)hint + offset], key)) {
            last = offset;
            offset = (offset << 1) + 1;
        }
        if (offset > limit) offset = limit;
        last += (ptrdiff_t)hint;
        offset += (ptrdiff_t)hint;
    } else {
        limit = (ptrdiff_t)hint + 1;
        while (offset < limit && !k_sort_less(sorter, keys[(ptrdiff_t)hint - offset], key)) {
            last = offset;
            offset = (offset << 1) + 1;
        }
        if (offset > limit) offset = limit;
        previous = last;
        last = (ptrdiff_t)hint - offset;
        offset = (ptrdiff_t)hint - previous;
    }
    last++;
    while (last < offset) {
        ptrdiff_t middle = last + ((offset - last) >> 1);
        if (k_sort_less(sorter, keys[middle], key)) last = middle + 1;
        else offset = middle;
    }
    return (size_t)offset;
}

/* The position in the sorted run that a key goes after, once equal items are passed. */
static __attribute__((unused)) size_t k_sort_gallop_right(KSorter *sorter, size_t key, const size_t *keys, size_t length, size_t hint) {
    ptrdiff_t last = 0, offset = 1, limit, previous;
    if (k_sort_less(sorter, key, keys[hint])) {
        limit = (ptrdiff_t)hint + 1;
        while (offset < limit && k_sort_less(sorter, key, keys[(ptrdiff_t)hint - offset])) {
            last = offset;
            offset = (offset << 1) + 1;
        }
        if (offset > limit) offset = limit;
        previous = last;
        last = (ptrdiff_t)hint - offset;
        offset = (ptrdiff_t)hint - previous;
    } else {
        limit = (ptrdiff_t)length - (ptrdiff_t)hint;
        while (offset < limit && !k_sort_less(sorter, key, keys[(ptrdiff_t)hint + offset])) {
            last = offset;
            offset = (offset << 1) + 1;
        }
        if (offset > limit) offset = limit;
        last += (ptrdiff_t)hint;
        offset += (ptrdiff_t)hint;
    }
    last++;
    while (last < offset) {
        ptrdiff_t middle = last + ((offset - last) >> 1);
        if (k_sort_less(sorter, key, keys[middle])) offset = middle;
        else last = middle + 1;
    }
    return (size_t)offset;
}

/* Length of the run at the front: a non-descending run, or a strictly descending run that is reversed. */
static __attribute__((unused)) size_t k_sort_count_run(KSorter *sorter, size_t *keys, size_t length) {
    size_t run = 2;
    if (length <= 1) return length;
    if (k_sort_less(sorter, keys[1], keys[0])) {
        while (run < length && k_sort_less(sorter, keys[run], keys[run - 1])) ++run;
        for (size_t i = 0, j = run - 1; i < j; ++i, --j) {
            size_t value = keys[i];
            keys[i] = keys[j];
            keys[j] = value;
        }
    } else {
        while (run < length && !k_sort_less(sorter, keys[run], keys[run - 1])) ++run;
    }
    return run;
}

/* Sorts keys[0..length) by insertion, where keys[0..start) is already sorted. */
static __attribute__((unused)) void k_sort_binary_insert(KSorter *sorter, size_t *keys, size_t length, size_t start) {
    for (size_t i = start; i < length; ++i) {
        size_t pivot = keys[i], left = 0, right = i;
        while (left < right) {
            size_t middle = left + ((right - left) >> 1);
            if (k_sort_less(sorter, pivot, keys[middle])) right = middle;
            else left = middle + 1;
        }
        memmove(keys + left + 1, keys + left, (i - left) * sizeof *keys);
        keys[left] = pivot;
    }
}

/* The shortest run that is sorted by insertion before merging starts, between 32 and 64. */
static __attribute__((unused)) size_t k_sort_min_run(size_t length) {
    size_t remainder = 0;
    while (length >= 64) {
        remainder |= length & 1;
        length >>= 1;
    }
    return length + remainder;
}

static __attribute__((unused)) void k_sort_reserve(KSorter *sorter, size_t length) {
    size_t *buffer;
    if (sorter->buffer_length >= length) return;
    buffer = realloc(sorter->buffer, length * sizeof *buffer);
    if (!buffer) k_fatal("Out of memory");
    sorter->buffer = buffer;
    sorter->buffer_length = length;
}

/* Merges two adjacent runs when the left one is no longer than the right one. */
static __attribute__((unused)) void k_sort_merge_low(KSorter *sorter, size_t base_a, size_t na, size_t base_b, size_t nb) {
    size_t *keys = sorter->keys, *tmp;
    size_t dest = base_a, ca = 0, cb = base_b, min_gallop = sorter->min_gallop;
    k_sort_reserve(sorter, na);
    tmp = sorter->buffer;
    memcpy(tmp, keys + base_a, na * sizeof *keys);
    keys[dest++] = keys[cb++];
    if (--nb == 0) goto succeed;
    if (na == 1) goto copy_b;
    for (;;) {
        size_t a_wins = 0, b_wins = 0;
        for (;;) {
            if (k_sort_less(sorter, keys[cb], tmp[ca])) {
                keys[dest++] = keys[cb++];
                ++b_wins;
                a_wins = 0;
                if (--nb == 0) goto succeed;
                if (b_wins >= min_gallop) break;
            } else {
                keys[dest++] = tmp[ca++];
                ++a_wins;
                b_wins = 0;
                if (--na == 1) goto copy_b;
                if (a_wins >= min_gallop) break;
            }
        }
        ++min_gallop;
        for (;;) {
            min_gallop -= min_gallop > 1;
            sorter->min_gallop = min_gallop;
            a_wins = k_sort_gallop_right(sorter, keys[cb], tmp + ca, na, 0);
            if (a_wins) {
                memcpy(keys + dest, tmp + ca, a_wins * sizeof *keys);
                dest += a_wins;
                ca += a_wins;
                na -= a_wins;
                if (na == 1) goto copy_b;
                if (na == 0) goto succeed;
            }
            keys[dest++] = keys[cb++];
            if (--nb == 0) goto succeed;
            b_wins = k_sort_gallop_left(sorter, tmp[ca], keys + cb, nb, 0);
            if (b_wins) {
                memmove(keys + dest, keys + cb, b_wins * sizeof *keys);
                dest += b_wins;
                cb += b_wins;
                nb -= b_wins;
                if (nb == 0) goto succeed;
            }
            keys[dest++] = tmp[ca++];
            if (--na == 1) goto copy_b;
            --min_gallop;
            if (a_wins < K_SORT_MIN_GALLOP && b_wins < K_SORT_MIN_GALLOP) break;
        }
        ++min_gallop;
        sorter->min_gallop = min_gallop;
    }
succeed:
    if (na) memcpy(keys + dest, tmp + ca, na * sizeof *keys);
    sorter->min_gallop = min_gallop;
    return;
copy_b:
    memmove(keys + dest, keys + cb, nb * sizeof *keys);
    keys[dest + nb] = tmp[ca];
    sorter->min_gallop = min_gallop;
}

/* Merges two adjacent runs when the left one is longer than the right one, from the top down. */
static __attribute__((unused)) void k_sort_merge_high(KSorter *sorter, size_t base_a, size_t na, size_t base_b, size_t nb) {
    size_t *keys = sorter->keys, *tmp;
    ptrdiff_t dest, ca, cb;
    size_t min_gallop = sorter->min_gallop;
    k_sort_reserve(sorter, nb);
    tmp = sorter->buffer;
    memcpy(tmp, keys + base_b, nb * sizeof *keys);
    dest = (ptrdiff_t)(base_b + nb) - 1;
    ca = (ptrdiff_t)(base_a + na) - 1;
    cb = (ptrdiff_t)nb - 1;
    keys[dest--] = keys[ca--];
    if (--na == 0) goto succeed;
    if (nb == 1) goto copy_a;
    for (;;) {
        size_t a_wins = 0, b_wins = 0;
        for (;;) {
            if (k_sort_less(sorter, tmp[cb], keys[ca])) {
                keys[dest--] = keys[ca--];
                ++a_wins;
                b_wins = 0;
                if (--na == 0) goto succeed;
                if (a_wins >= min_gallop) break;
            } else {
                keys[dest--] = tmp[cb--];
                ++b_wins;
                a_wins = 0;
                if (--nb == 1) goto copy_a;
                if (b_wins >= min_gallop) break;
            }
        }
        ++min_gallop;
        for (;;) {
            size_t position;
            min_gallop -= min_gallop > 1;
            sorter->min_gallop = min_gallop;
            position = k_sort_gallop_right(sorter, tmp[cb], keys + base_a, na, na - 1);
            a_wins = na - position;
            if (a_wins) {
                dest -= (ptrdiff_t)a_wins;
                ca -= (ptrdiff_t)a_wins;
                memmove(keys + dest + 1, keys + ca + 1, a_wins * sizeof *keys);
                na -= a_wins;
                if (na == 0) goto succeed;
            }
            keys[dest--] = tmp[cb--];
            if (--nb == 1) goto copy_a;
            position = k_sort_gallop_left(sorter, keys[ca], tmp, nb, nb - 1);
            b_wins = nb - position;
            if (b_wins) {
                dest -= (ptrdiff_t)b_wins;
                cb -= (ptrdiff_t)b_wins;
                memcpy(keys + dest + 1, tmp + cb + 1, b_wins * sizeof *keys);
                nb -= b_wins;
                if (nb == 1) goto copy_a;
            }
            keys[dest--] = keys[ca--];
            if (--na == 0) goto succeed;
            --min_gallop;
            if (a_wins < K_SORT_MIN_GALLOP && b_wins < K_SORT_MIN_GALLOP) break;
        }
        ++min_gallop;
        sorter->min_gallop = min_gallop;
    }
succeed:
    if (nb) memcpy(keys + dest - (ptrdiff_t)nb + 1, tmp, nb * sizeof *keys);
    sorter->min_gallop = min_gallop;
    return;
copy_a:
    dest -= (ptrdiff_t)na;
    ca -= (ptrdiff_t)na;
    memmove(keys + dest + 1, keys + ca + 1, na * sizeof *keys);
    keys[dest] = tmp[cb];
    sorter->min_gallop = min_gallop;
}

static __attribute__((unused)) void k_sort_merge_at(KSorter *sorter, KSortRun *runs, size_t *count, size_t index) {
    size_t base_a = runs[index].base, na = runs[index].length;
    size_t base_b = runs[index + 1].base, nb = runs[index + 1].length;
    size_t position;
    runs[index].length = na + nb;
    for (size_t j = index + 1; j + 1 < *count; ++j) runs[j] = runs[j + 1];
    --*count;
    /* Elements of a that are already below b's first item stay where they are. */
    position = k_sort_gallop_right(sorter, sorter->keys[base_b], sorter->keys + base_a, na, 0);
    base_a += position;
    na -= position;
    if (na == 0) return;
    /* Elements of b that are already above a's last item stay where they are. */
    nb = k_sort_gallop_left(sorter, sorter->keys[base_a + na - 1], sorter->keys + base_b, nb, nb - 1);
    if (nb == 0) return;
    if (na <= nb) k_sort_merge_low(sorter, base_a, na, base_b, nb);
    else k_sort_merge_high(sorter, base_a, na, base_b, nb);
}

static __attribute__((unused)) void k_sort_merge_collapse(KSorter *sorter, KSortRun *runs, size_t *count) {
    while (*count > 1) {
        size_t n = *count - 2;
        if ((n > 0 && runs[n - 1].length <= runs[n].length + runs[n + 1].length) ||
            (n > 1 && runs[n - 2].length <= runs[n - 1].length + runs[n].length)) {
            if (runs[n - 1].length < runs[n + 1].length) --n;
        } else if (runs[n].length > runs[n + 1].length) {
            break;
        }
        k_sort_merge_at(sorter, runs, count, n);
    }
}

static __attribute__((unused)) void k_sort_merge_force(KSorter *sorter, KSortRun *runs, size_t *count) {
    while (*count > 1) {
        size_t n = *count - 2;
        if (n > 0 && runs[n - 1].length < runs[n + 1].length) --n;
        k_sort_merge_at(sorter, runs, count, n);
    }
}

/* Sorts the item indices in keys[0..length) stably, by compare(context, a, b) on those indices. */
static __attribute__((unused)) void k_sort_indices(size_t *keys, size_t length, int64_t (*compare)(void *, size_t, size_t), void *context) {
    KSorter sorter;
    KSortRun runs[K_SORT_MAX_RUNS];
    size_t count = 0, start = 0, minimum;
    if (length < 2) return;
    sorter.keys = keys;
    sorter.buffer = NULL;
    sorter.buffer_length = 0;
    sorter.compare = compare;
    sorter.context = context;
    sorter.min_gallop = K_SORT_MIN_GALLOP;
    minimum = k_sort_min_run(length);
    while (start < length) {
        size_t remaining = length - start;
        size_t run = k_sort_count_run(&sorter, keys + start, remaining);
        if (run < minimum) {
            size_t forced = remaining < minimum ? remaining : minimum;
            k_sort_binary_insert(&sorter, keys + start, forced, run);
            run = forced;
        }
        runs[count].base = start;
        runs[count].length = run;
        ++count;
        k_sort_merge_collapse(&sorter, runs, &count);
        start += run;
    }
    k_sort_merge_force(&sorter, runs, &count);
    free(sorter.buffer);
}

/* Stable least-significant-digit radix sort for 64-bit integers. The caller uses
   it for whole Int and Nat arrays, where it avoids the per-comparison callback
   and the index permutation of the general routine. Only the digits that can
   differ are visited, so small ranges take one or two passes. */
// In-place MSD bucket partition by one byte, then a small LSD radix inside
// each bucket. Only the largest bucket needs a scratch buffer, so two million
// integers sort without a second full-size array. Equal integers cannot be
// told apart, so stability is unobservable and the in-place partition is safe.
static void k_sort_int64_partition(int64_t *items, size_t length, int shift,
        size_t *starts, size_t *counts) {
    const uint64_t bias = UINT64_C(1) << 63;
    size_t found[256] = {0};
    for (size_t i = 0; i < length; ++i) found[(((uint64_t)items[i] ^ bias) >> shift) & 0xFFu] += 1;
    size_t total = 0;
    for (int digit = 0; digit < 256; ++digit) { starts[digit] = total; counts[digit] = found[digit]; total += found[digit]; }
    size_t next[256];
    memcpy(next, starts, sizeof next);
    for (int digit = 0; digit < 256; ++digit) {
        while (next[digit] < starts[digit] + counts[digit]) {
            size_t index = next[digit];
            unsigned place = (unsigned)((((uint64_t)items[index] ^ bias) >> shift) & 0xFFu);
            while (place != (unsigned)digit) {
                size_t target = next[place];
                int64_t saved = items[target]; items[target] = items[index]; items[index] = saved;
                next[place] += 1;
                place = (unsigned)((((uint64_t)items[index] ^ bias) >> shift) & 0xFFu);
            }
            next[digit] += 1;
        }
    }
}

static void k_sort_int64_bucket(int64_t *items, size_t length, int64_t *scratch, int passes) {
    const uint64_t bias = UINT64_C(1) << 63;
    int64_t *source = items, *target = scratch;
    for (int pass = 0; pass < passes; ++pass) {
        int shift = pass * 8;
        size_t counts[256] = {0};
        for (size_t i = 0; i < length; ++i) counts[(((uint64_t)source[i] ^ bias) >> shift) & 0xFFu] += 1;
        size_t total = 0;
        for (int digit = 0; digit < 256; ++digit) { size_t count = counts[digit]; counts[digit] = total; total += count; }
        for (size_t i = 0; i < length; ++i) {
            unsigned digit = (unsigned)((((uint64_t)source[i] ^ bias) >> shift) & 0xFFu);
            target[counts[digit]++] = source[i];
        }
        int64_t *swap = source; source = target; target = swap;
    }
    if (source != items) memcpy(items, source, length * sizeof *items);
}

static __attribute__((unused)) void k_sort_int64_stable(int64_t *items, size_t length) {
    if (length < 2) return;
    const uint64_t bias = UINT64_C(1) << 63;
    uint64_t first = (uint64_t)items[0] ^ bias, differing = 0;
    for (size_t i = 1; i < length; ++i) differing |= ((uint64_t)items[i] ^ bias) ^ first;
    int passes = 0;
    while (differing) { passes += 1; differing >>= 8; }
    if (!passes) return;
    size_t starts[256], counts[256];
    k_sort_int64_partition(items, length, (passes - 1) * 8, starts, counts);
    size_t largest = 0;
    for (int digit = 0; digit < 256; ++digit) if (counts[digit] > largest) largest = counts[digit];
    int64_t *scratch = malloc((largest ? largest : 1) * sizeof *scratch);
    if (!scratch) k_fatal("Out of memory");
    for (int digit = 0; digit < 256; ++digit)
        if (counts[digit] > 1) k_sort_int64_bucket(items + starts[digit], counts[digit], scratch, passes - 1);
    free(scratch);
}
