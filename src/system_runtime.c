/* Format specs, files, processes and clocks (REVIEW2 section 4). */
#include <dirent.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

/* Python's format mini-language: [[fill]align][sign][#][0][width][,|_][.precision][type]. */
typedef struct {
    uint32_t fill;
    char align, sign, group, type;
    bool alternate;
    int64_t width, precision;
} KFormat;

static K_UNUSED KFormat k_format_parse(KString spec) {
    KFormat f = { ' ', 0, '-', 0, 0, false, -1, -1 };
    size_t i = 0;
    if (spec.length) {
        size_t next = 0;
        uint32_t first = k_utf8_next(spec, &next);
        if (next < spec.length && strchr("<>^=", spec.data[next])) {
            f.fill = first;
            f.align = spec.data[next];
            i = next + 1;
        } else if (strchr("<>^=", spec.data[0])) {
            f.align = spec.data[0];
            i = 1;
        }
    }
    if (i < spec.length && strchr("+- ", spec.data[i])) f.sign = spec.data[i++];
    if (i < spec.length && spec.data[i] == '#') { f.alternate = true; ++i; }
    if (i < spec.length && spec.data[i] == '0') {
        if (!f.align) { f.fill = '0'; f.align = '='; }
        ++i;
    }
    if (i < spec.length && spec.data[i] >= '0' && spec.data[i] <= '9') {
        f.width = 0;
        while (i < spec.length && spec.data[i] >= '0' && spec.data[i] <= '9' && f.width < 100000)
            f.width = f.width * 10 + (spec.data[i++] - '0');
    }
    if (i < spec.length && (spec.data[i] == ',' || spec.data[i] == '_')) f.group = spec.data[i++];
    if (i < spec.length && spec.data[i] == '.') {
        ++i;
        f.precision = 0;
        while (i < spec.length && spec.data[i] >= '0' && spec.data[i] <= '9' && f.precision < 100000)
            f.precision = f.precision * 10 + (spec.data[i++] - '0');
    }
    if (i < spec.length) f.type = spec.data[i];
    return f;
}

/* Pads `body` to the width. `prefix` is the sign and base marker, which `=` keeps in front of the fill. */
static K_UNUSED KString k_format_pad(KFormat f, KString prefix, KString body, char default_align) {
    char align = f.align ? f.align : default_align;
    int64_t length = (int64_t)(k_string_scalars(prefix) + k_string_scalars(body));
    int64_t pad = f.width > length ? f.width - length : 0;
    int64_t before = align == '>' || align == '=' ? pad : align == '^' ? pad / 2 : 0;
    int64_t after = pad - before;
    KTextBuilder b = {0};
    if (align == '=') k_text_put_bytes(&b, prefix, 0, prefix.length);
    for (int64_t i = 0; i < before; ++i) k_text_put(&b, f.fill);
    if (align != '=') k_text_put_bytes(&b, prefix, 0, prefix.length);
    k_text_put_bytes(&b, body, 0, body.length);
    for (int64_t i = 0; i < after; ++i) k_text_put(&b, f.fill);
    return k_text_finish(&b);
}

/* Inserts the group separator into the digits before the decimal point. */
static K_UNUSED KString k_format_group(KString digits, char group, size_t every) {
    if (!group) return k_string_copy(digits);
    size_t end = 0;
    while (end < digits.length && digits.data[end] != '.' && digits.data[end] != 'e' && digits.data[end] != 'E') ++end;
    KTextBuilder b = {0};
    for (size_t i = 0; i < end; ++i) {
        if (i > 0 && (end - i) % every == 0) k_text_put(&b, (uint32_t)group);
        k_text_put(&b, (unsigned char)digits.data[i]);
    }
    k_text_put_bytes(&b, digits, end, digits.length);
    return k_text_finish(&b);
}

static K_UNUSED KString k_format_text(KString text, KString spec) {
    KFormat f = k_format_parse(spec);
    KString body = f.precision >= 0 ? k_string_substring(text, 0, f.precision) : k_string_copy(text);
    KString result = k_format_pad(f, k_empty, body, '<');
    k_string_drop(&body);
    return result;
}

static K_UNUSED KString k_format_double(double value, KString spec) {
    KFormat f = k_format_parse(spec);
    bool negative = signbit(value) && !isnan(value);
    double magnitude = fabs(value);
    char type = f.type ? f.type : (f.precision >= 0 ? 'g' : 0);
    KString digits;
    if (type == 0) {
        digits = k_text_double(magnitude);
    } else {
        char format[8];
        int precision = f.precision >= 0 ? (int)f.precision : 6;
        if (type == '%') magnitude *= 100;
        snprintf(format, sizeof format, f.alternate ? "%%#.*%c" : "%%.*%c", type == '%' ? 'f' : type);
        locale_t previous = uselocale(k_number_locale());
        int length = snprintf(NULL, 0, format, precision, magnitude);
        uselocale(previous);
        if (length < 0) k_fatal("double formatting failed");
        digits = k_string_new((size_t)length + (type == '%'));
        previous = uselocale(k_number_locale());
        snprintf(digits.owner->bytes, (size_t)length + 1, format, precision, magnitude);
        uselocale(previous);
        if (type == '%') digits.owner->bytes[length] = '%';
        if (!f.type && f.precision >= 0 && !memchr(digits.data, '.', digits.length)
            && !memchr(digits.data, 'e', digits.length) && isfinite(magnitude)) {
            KString pieces[] = { digits, K_TEXT(".0") };
            KString decimal = k_string_join(2, pieces);
            k_string_drop(&digits);
            digits = decimal;
        }
    }
    KString grouped = k_format_group(digits, f.group, 3);
    k_string_drop(&digits);
    const char *sign = negative ? "-" : f.sign == '+' ? "+" : f.sign == ' ' ? " " : "";
    KString prefix = { sign, strlen(sign), NULL };
    KString result = k_format_pad(f, prefix, grouped, '>');
    k_string_drop(&grouped);
    return result;
}

static K_UNUSED KString k_format_integer(uint64_t magnitude, bool negative, KString spec, KLocation loc) {
    KFormat f = k_format_parse(spec);
    if (f.type && strchr("eEfFgG%", f.type))
        return k_format_double(negative ? -(double)magnitude : (double)magnitude, spec);
    if (f.type == 'c') {
        KString character = k_string_chr(negative || magnitude > UINT64_C(1114111) ? -1 : (int64_t)magnitude, loc);
        KString result = k_format_pad(f, k_empty, character, '<');
        k_string_drop(&character);
        return result;
    }
    unsigned base = f.type == 'x' || f.type == 'X' ? 16 : f.type == 'o' ? 8 : f.type == 'b' ? 2 : 10;
    char buffer[80];
    size_t length = 0;
    const char *alphabet = f.type == 'X' ? "0123456789ABCDEF" : "0123456789abcdef";
    do { buffer[length++] = alphabet[magnitude % base]; magnitude /= base; } while (magnitude);
    for (size_t i = 0; i < length / 2; ++i) { char t = buffer[i]; buffer[i] = buffer[length - 1 - i]; buffer[length - 1 - i] = t; }
    KString digits = k_string_bytes(buffer, length);
    KString grouped = k_format_group(digits, f.group, base == 10 ? 3 : 4);
    k_string_drop(&digits);
    char prefix_text[8] = {0};
    size_t p = 0;
    if (negative) prefix_text[p++] = '-';
    else if (f.sign == '+') prefix_text[p++] = '+';
    else if (f.sign == ' ') prefix_text[p++] = ' ';
    if (f.alternate && base != 10) {
        prefix_text[p++] = '0';
        prefix_text[p++] = base == 16 ? (f.type == 'X' ? 'X' : 'x') : base == 8 ? 'o' : 'b';
    }
    KString prefix = k_string_bytes(prefix_text, p);
    KString result = k_format_pad(f, prefix, grouped, '>');
    k_string_drop(&prefix);
    k_string_drop(&grouped);
    return result;
}
static K_UNUSED KString k_format_int(int64_t value, KString spec, KLocation loc) {
    uint64_t magnitude = value < 0 ? (uint64_t)0 - (uint64_t)value : (uint64_t)value;
    return k_format_integer(magnitude, value < 0, spec, loc);
}
static K_UNUSED KString k_format_nat(uint64_t value, KString spec, KLocation loc) {
    return k_format_integer(value, false, spec, loc);
}

/* assertTrue, assertFalse, assertNotEq and assertNear report the check and what they saw. */
_Noreturn static __attribute__((unused)) void k_check_failed(const char *check, KString detail, KLocation loc) {
    fprintf(stderr, "%s:%zu:%zu: runtime error: %s failed\n  %s\n  ",
            loc.file, loc.line, loc.column, check, loc.expression);
    if (detail.length) fwrite(detail.data, 1, detail.length, stderr);
    fputc('\n', stderr);
    exit(1);
}
static __attribute__((unused)) void k_assert_near(double a, double b, double tolerance, KLocation loc) {
    if (fabs(a - b) <= tolerance) return;
    KString left = k_text_double(a), right = k_text_double(b), limit = k_text_double(tolerance);
    KString pieces[] = { K_TEXT("left: "), left, K_TEXT(", right: "), right, K_TEXT(", tolerance: "), limit };
    k_check_failed("assertNear", k_string_join(6, pieces), loc);
}

/* Files. Each returns false and fills `error` when the system call fails. */
static K_UNUSED bool k_fs_read_bytes(KString path, KString *text, KError *error) {
    if (memchr(path.data, 0, path.length)) { *error = k_runtime_error(3, K_TEXT("A file path contains a NUL character")); return false; }
    char *name = k_c_text(path);
    FILE *file = fopen(name, "rb");
    free(name);
    if (!file) { *error = k_io_error("Cannot open file"); return false; }
    KTextBuilder b = {0};
    char chunk[65536];
    size_t count;
    while ((count = fread(chunk, 1, sizeof chunk, file)) > 0) {
        KString piece = { chunk, count, NULL };
        k_text_put_bytes(&b, piece, 0, count);
    }
    bool failed = ferror(file);
    fclose(file);
    KString result = k_text_finish(&b);
    if (failed) { k_string_drop(&result); *error = k_io_error("Cannot read file"); return false; }
    *text = result;
    return true;
}
static K_UNUSED bool k_fs_read_file(KString path, KString *text, KError *error) {
    KString result;
    if (!k_fs_read_bytes(path, &result, error)) return false;
    if (!k_valid_utf8(result)) {
        k_string_drop(&result);
        *error = k_runtime_error(3, K_TEXT("The file is not valid UTF-8 text"));
        return false;
    }
    *text = result;
    return true;
}
static K_UNUSED bool k_fs_write_file(KString path, KString text, bool append, KError *error) {
    if (memchr(path.data, 0, path.length)) { *error = k_runtime_error(3, K_TEXT("A file path contains a NUL character")); return false; }
    char *name = k_c_text(path);
    FILE *file = fopen(name, append ? "ab" : "wb");
    free(name);
    if (!file) { *error = k_io_error("Cannot open file"); return false; }
    bool ok = fwrite(text.data, 1, text.length, file) == text.length;
    ok = fclose(file) == 0 && ok;
    if (!ok) { *error = k_io_error("Cannot write file"); return false; }
    return true;
}
static K_UNUSED bool k_fs_exists(KString path) {
    if (memchr(path.data, 0, path.length)) return false;
    char *name = k_c_text(path);
    struct stat info;
    bool found = stat(name, &info) == 0;
    free(name);
    return found;
}
static K_UNUSED bool k_fs_is_dir(KString path) {
    if (memchr(path.data, 0, path.length)) return false;
    char *name = k_c_text(path);
    struct stat info;
    bool found = stat(name, &info) == 0 && S_ISDIR(info.st_mode);
    free(name);
    return found;
}
static K_UNUSED bool k_fs_create_dirs(KString path, KError *error) {
    if (memchr(path.data, 0, path.length)) { *error = k_runtime_error(3, K_TEXT("A file path contains a NUL character")); return false; }
    char *name = k_c_text(path);
    size_t length = strlen(name);
    for (size_t i = 1; i <= length; ++i) {
        if (name[i] != '/' && name[i] != 0) continue;
        char saved = name[i];
        name[i] = 0;
        struct stat info;
        if (mkdir(name, 0777) != 0 && (errno != EEXIST || stat(name, &info) != 0 || !S_ISDIR(info.st_mode))) {
            *error = k_runtime_error(3, K_TEXT("Cannot create directory: a path component is not a directory or is inaccessible"));
            free(name);
            return false;
        }
        name[i] = saved;
    }
    free(name);
    return true;
}
static K_UNUSED bool k_fs_remove(KString path, KError *error) {
    if (memchr(path.data, 0, path.length)) { *error = k_runtime_error(3, K_TEXT("A file path contains a NUL character")); return false; }
    char *name = k_c_text(path);
    int status = remove(name);
    free(name);
    if (status != 0) { *error = k_io_error("Cannot remove"); return false; }
    return true;
}
static K_UNUSED bool k_fs_rename(KString from, KString to, KError *error) {
    if (memchr(from.data, 0, from.length) || memchr(to.data, 0, to.length)) { *error = k_runtime_error(3, K_TEXT("A file path contains a NUL character")); return false; }
    char *a = k_c_text(from), *b = k_c_text(to);
    int status = rename(a, b);
    free(a);
    free(b);
    if (status != 0) { *error = k_io_error("Cannot rename"); return false; }
    return true;
}
static K_UNUSED bool k_fs_copy(KString from, KString to, KError *error) {
    if (memchr(from.data, 0, from.length) || memchr(to.data, 0, to.length)) { *error = k_runtime_error(3, K_TEXT("A file path contains a NUL character")); return false; }
    char *a = k_c_text(from);
    FILE *in = fopen(a, "rb");
    free(a);
    if (!in) { *error = k_io_error("Cannot open file"); return false; }
    char *b = k_c_text(to);
    /* Open without truncation so path aliases cannot erase the input. */
    int descriptor = open(b, O_WRONLY | O_CREAT, 0666);
    free(b);
    if (descriptor < 0) { *error = k_io_error("Cannot create file"); fclose(in); return false; }
    struct stat input_info, output_info;
    if (fstat(fileno(in), &input_info) != 0 || fstat(descriptor, &output_info) != 0) {
        *error = k_io_error("Cannot inspect files for copying");
        close(descriptor);
        fclose(in);
        return false;
    }
    if (input_info.st_dev == output_info.st_dev && input_info.st_ino == output_info.st_ino) {
        *error = k_runtime_error(3, K_TEXT("Cannot copy a file onto itself"));
        close(descriptor);
        fclose(in);
        return false;
    }
    if (S_ISREG(output_info.st_mode) && ftruncate(descriptor, 0) != 0) {
        *error = k_io_error("Cannot truncate destination file");
        close(descriptor);
        fclose(in);
        return false;
    }
    FILE *out = fdopen(descriptor, "wb");
    if (!out) {
        *error = k_io_error("Cannot create file stream");
        close(descriptor);
        fclose(in);
        return false;
    }
    char chunk[65536];
    size_t count;
    bool ok = true;
    while (ok && (count = fread(chunk, 1, sizeof chunk, in)) > 0) ok = fwrite(chunk, 1, count, out) == count;
    ok = !ferror(in) && ok;
    fclose(in);
    ok = fclose(out) == 0 && ok;
    if (!ok) { *error = k_io_error("Cannot copy file"); return false; }
    return true;
}
static int k_fs_name_order(const void *a, const void *b) {
    const KString *x = a, *y = b;
    size_t n = x->length < y->length ? x->length : y->length;
    int order = memcmp(x->data, y->data, n);
    return order ? order : (x->length > y->length) - (x->length < y->length);
}
/* Entry names in byte order, without `.` and `..`. The caller frees the array. */
static K_UNUSED bool k_fs_list_dir(KString path, KString **names, size_t *count, KError *error) {
    if (memchr(path.data, 0, path.length)) { *error = k_runtime_error(3, K_TEXT("A file path contains a NUL character")); return false; }
    char *name = k_c_text(path);
    DIR *directory = opendir(name);
    free(name);
    if (!directory) { *error = k_io_error("Cannot open directory"); return false; }
    size_t capacity = 16;
    *count = 0;
    *names = malloc(capacity * sizeof **names);
    if (!*names) k_fatal("Out of memory");
    struct dirent *entry;
    while ((entry = readdir(directory))) {
        if (!strcmp(entry->d_name, ".") || !strcmp(entry->d_name, "..")) continue;
        if (*count == capacity) {
            capacity *= 2;
            KString *grown = realloc(*names, capacity * sizeof **names);
            if (!grown) k_fatal("Out of memory");
            *names = grown;
        }
        (*names)[(*count)++] = k_string_bytes(entry->d_name, strlen(entry->d_name));
    }
    closedir(directory);
    qsort(*names, *count, sizeof **names, k_fs_name_order);
    return true;
}
static K_UNUSED bool k_fs_temp_dir(KString *path, KError *error) {
    const char *base = getenv("TMPDIR");
    if (!base || !*base) base = "/tmp";
    char pattern[4096];
    int length = snprintf(pattern, sizeof pattern, "%s%skestrel-XXXXXX", base, base[strlen(base) - 1] == '/' ? "" : "/");
    if (length < 0 || (size_t)length >= sizeof pattern || !mkdtemp(pattern)) {
        *error = k_io_error("Cannot create a temporary directory");
        return false;
    }
    *path = k_string_bytes(pattern, strlen(pattern));
    return true;
}

/* Console and process. */
static K_UNUSED void k_os_write(KString text, bool error_stream) {
    FILE *stream = error_stream ? stderr : stdout;
    flockfile(stream);
    if (text.length && fwrite(text.data, 1, text.length, stream) != text.length)
        k_fatal("Cannot write output");
    funlockfile(stream);
}
static K_UNUSED bool k_os_env(KString name, KString *value) {
    char *key = k_c_text(name);
    const char *found = getenv(key);
    free(key);
    if (!found) return false;
    *value = k_string_bytes(found, strlen(found));
    return true;
}
static K_UNUSED KString k_os_cwd(void) {
    char buffer[4096];
    if (!getcwd(buffer, sizeof buffer)) return k_empty;
    return k_string_bytes(buffer, strlen(buffer));
}
_Noreturn static K_UNUSED void k_os_exit(int64_t code) {
    fflush(stdout);
    fflush(stderr);
    exit((int)(code & 255));
}
/* Runs a shell command and captures its standard output. */
static K_UNUSED bool k_os_run(KString command, int64_t *status, KString *output, KError *error) {
    char *text = k_c_text(command);
    fflush(stdout);
    FILE *pipe = popen(text, "r");
    free(text);
    if (!pipe) { *error = k_io_error("Cannot run command"); return false; }
    KTextBuilder b = {0};
    char chunk[4096];
    size_t count;
    while ((count = fread(chunk, 1, sizeof chunk, pipe)) > 0) {
        KString piece = { chunk, count, NULL };
        k_text_put_bytes(&b, piece, 0, count);
    }
    int code = pclose(pipe);
    *output = k_text_finish(&b);
    *status = code == -1 ? -1 : WIFEXITED(code) ? WEXITSTATUS(code) : 128 + WTERMSIG(code);
    return true;
}

/* Clocks. */
static K_UNUSED double k_time_now(void) {
    struct timespec now;
    clock_gettime(CLOCK_REALTIME, &now);
    return (double)now.tv_sec + (double)now.tv_nsec / 1e9;
}
static K_UNUSED int64_t k_time_monotonic(void) {
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return (int64_t)now.tv_sec * 1000000000 + now.tv_nsec;
}
static K_UNUSED void k_time_sleep(double seconds) {
    if (!(seconds > 0)) return;
    struct timespec wait = { (time_t)seconds, (long)((seconds - (double)(time_t)seconds) * 1e9) };
    while (nanosleep(&wait, &wait) != 0 && errno == EINTR) {}
}

/* Bytes share counted storage with String, but only decodeUtf8 can cross the type boundary. */
static K_UNUSED KString k_bytes_slice(KString bytes, int64_t start, int64_t end) {
    int64_t n = (int64_t)bytes.length;
    if (start < 0) start += n;
    if (end < 0) end += n;
    if (start < 0) start = 0; if (start > n) start = n;
    if (end < start) end = start; if (end > n) end = n;
    return k_string_bytes(bytes.data + start, (size_t)(end - start));
}
static K_UNUSED KString k_bytes_hex(KString bytes) {
    const char *digits = "0123456789abcdef";
    KString result = k_string_new(bytes.length > SIZE_MAX / 2 ? (k_fatal("Byte sequence is too large"), 0) : bytes.length * 2);
    for (size_t i = 0; i < bytes.length; ++i) {
        unsigned char b = (unsigned char)bytes.data[i];
        result.owner->bytes[2*i] = digits[b >> 4]; result.owner->bytes[2*i+1] = digits[b & 15];
    }
    return result;
}
