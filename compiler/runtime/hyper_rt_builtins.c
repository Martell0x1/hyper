/* Python-like builtins for AOT (`len`, `abs`, `min`, …). */
#include <ctype.h>
#include <limits.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum {
    KIND_I64 = 0,
    KIND_F64 = 1,
    KIND_STR = 2,
    KIND_BOOL = 3,
    KIND_NONE = 4,
    KIND_LIST = 5,
    KIND_DICT = 6,
    KIND_U64 = 10
};

typedef struct {
    int64_t kind;
    int64_t payload;
} RtValue;

typedef struct {
    RtValue *items;
    size_t len;
    size_t cap;
} RtList;

typedef struct {
    char *key;
    RtValue value;
} RtDictEntry;

typedef struct {
    RtDictEntry *entries;
    size_t len;
    size_t cap;
} RtDict;

extern int64_t hyper_rt_coll_len(int64_t payload, int64_t kind, int64_t line, int64_t line_kind);
extern int64_t hyper_rt_list_new(void);
extern void hyper_rt_list_push(int64_t list_h, int64_t value, int64_t kind);
extern int64_t hyper_rt_value_to_str(int64_t payload, int64_t kind);
extern int64_t hyper_rt_pow_i64(int64_t base, int64_t exp);
extern double hyper_rt_pow_f64(double base, double exp);
extern int64_t hyper_rt_floor_div_i64(int64_t a, int64_t b);
extern double hyper_rt_floor_div_f64(double a, double b);
extern void hyper_rt_div_by_zero(int64_t line);

static void runtime_error(int64_t line, const char *msg) {
    fflush(stdout);
    fprintf(stderr, "RuntimeError: line %lld: %s\n", (long long)line, msg);
    exit(70);
}

static char *rt_strdup(const char *s) {
    size_t n = strlen(s) + 1;
    char *out = (char *)malloc(n);
    if (out) {
        memcpy(out, s, n);
    }
    return out;
}

static const char *cstr(int64_t payload) {
    return payload ? (const char *)(intptr_t)payload : "";
}

static void set_out_kind(int64_t *out_kind, int64_t kind) {
    if (out_kind) {
        *out_kind = kind;
    }
}

static int is_numeric(int64_t kind) {
    return kind == KIND_I64 || kind == KIND_U64 || kind == KIND_F64 || kind == KIND_BOOL;
}

static double as_f64(int64_t payload, int64_t kind, int64_t line, const char *ctx) {
    if (kind == KIND_I64 || kind == KIND_BOOL) {
        return (double)payload;
    }
    if (kind == KIND_U64) {
        return (double)(uint64_t)payload;
    }
    if (kind == KIND_F64) {
        double d;
        memcpy(&d, &payload, sizeof(d));
        return d;
    }
    {
        char buf[128];
        snprintf(buf, sizeof(buf), "%s: expected a number", ctx);
        runtime_error(line, buf);
    }
    return 0.0;
}

static int64_t f64_bits(double d) {
    int64_t bits;
    memcpy(&bits, &d, sizeof(bits));
    return bits;
}

static const RtList *as_list(int64_t payload, int64_t kind, int64_t line, const char *ctx) {
    if (kind != KIND_LIST) {
        char buf[128];
        snprintf(buf, sizeof(buf), "%s: expected a list", ctx);
        runtime_error(line, buf);
    }
    if (!payload) {
        return NULL;
    }
    return (const RtList *)(intptr_t)payload;
}

static int is_truthy(int64_t payload, int64_t kind) {
    if (kind == KIND_NONE) {
        return 0;
    }
    if (kind == KIND_BOOL || kind == KIND_I64 || kind == KIND_U64) {
        return payload != 0;
    }
    if (kind == KIND_F64) {
        double d;
        memcpy(&d, &payload, sizeof(d));
        return d != 0.0;
    }
    if (kind == KIND_STR) {
        return cstr(payload)[0] != '\0';
    }
    if (kind == KIND_LIST) {
        if (!payload) {
            return 0;
        }
        return ((const RtList *)(intptr_t)payload)->len != 0;
    }
    if (kind == KIND_DICT) {
        if (!payload) {
            return 0;
        }
        return ((const RtDict *)(intptr_t)payload)->len != 0;
    }
    return 1;
}

static int64_t clone_item(int64_t payload, int64_t kind) {
    if (kind == KIND_STR) {
        return (int64_t)(intptr_t)rt_strdup(cstr(payload));
    }
    return payload;
}

int64_t hyper_rt_builtin_len(int64_t payload, int64_t kind, int64_t line, int64_t line_kind) {
    return hyper_rt_coll_len(payload, kind, line, line_kind);
}

int64_t hyper_rt_builtin_abs(
    int64_t payload,
    int64_t kind,
    int64_t line,
    int64_t line_kind,
    int64_t *out_kind
) {
    (void)line_kind;
    if (kind == KIND_I64) {
        set_out_kind(out_kind, KIND_I64);
        if (payload == INT64_MIN) {
            return payload;
        }
        return payload < 0 ? -payload : payload;
    }
    if (kind == KIND_U64) {
        set_out_kind(out_kind, KIND_U64);
        return payload;
    }
    if (kind == KIND_F64) {
        double d;
        memcpy(&d, &payload, sizeof(d));
        set_out_kind(out_kind, KIND_F64);
        return f64_bits(fabs(d));
    }
    runtime_error(line, "abs() expected a number");
    return 0;
}

static int64_t min_max_list(
    int64_t payload,
    int64_t kind,
    int64_t line,
    int64_t *out_kind,
    int want_max,
    const char *name
) {
    const RtList *list = as_list(payload, kind, line, name);
    size_t n = list ? list->len : 0;
    if (n == 0) {
        char buf[96];
        snprintf(buf, sizeof(buf), "%s() arg is an empty sequence", name);
        runtime_error(line, buf);
    }
    RtValue best = list->items[0];
    if (!is_numeric(best.kind)) {
        char buf[96];
        snprintf(buf, sizeof(buf), "%s() expected numeric list elements", name);
        runtime_error(line, buf);
    }
    int use_float = best.kind == KIND_F64;
    for (size_t i = 1; i < n; i++) {
        RtValue item = list->items[i];
        if (!is_numeric(item.kind)) {
            char buf[96];
            snprintf(buf, sizeof(buf), "%s() expected numeric list elements", name);
            runtime_error(line, buf);
        }
        if (item.kind == KIND_F64) {
            use_float = 1;
        }
        double left = as_f64(best.payload, best.kind, line, name);
        double right = as_f64(item.payload, item.kind, line, name);
        if (want_max ? (right > left) : (right < left)) {
            best = item;
        }
    }
    if (use_float) {
        set_out_kind(out_kind, KIND_F64);
        return f64_bits(as_f64(best.payload, best.kind, line, name));
    }
    if (best.kind == KIND_U64) {
        set_out_kind(out_kind, KIND_U64);
        return best.payload;
    }
    set_out_kind(out_kind, KIND_I64);
    return best.payload;
}

int64_t hyper_rt_builtin_min(
    int64_t payload,
    int64_t kind,
    int64_t line,
    int64_t line_kind,
    int64_t *out_kind
) {
    (void)line_kind;
    return min_max_list(payload, kind, line, out_kind, 0, "min");
}

int64_t hyper_rt_builtin_max(
    int64_t payload,
    int64_t kind,
    int64_t line,
    int64_t line_kind,
    int64_t *out_kind
) {
    (void)line_kind;
    return min_max_list(payload, kind, line, out_kind, 1, "max");
}

int64_t hyper_rt_builtin_sum(
    int64_t payload,
    int64_t kind,
    int64_t line,
    int64_t line_kind,
    int64_t *out_kind
) {
    (void)line_kind;
    const RtList *list = as_list(payload, kind, line, "sum");
    size_t n = list ? list->len : 0;
    int use_float = 0;
    for (size_t i = 0; i < n; i++) {
        if (!is_numeric(list->items[i].kind)) {
            runtime_error(line, "sum() expected a list of numbers");
        }
        if (list->items[i].kind == KIND_F64) {
            use_float = 1;
        }
    }
    if (use_float) {
        double acc = 0.0;
        for (size_t i = 0; i < n; i++) {
            acc += as_f64(list->items[i].payload, list->items[i].kind, line, "sum");
        }
        set_out_kind(out_kind, KIND_F64);
        return f64_bits(acc);
    }
    int64_t acc = 0;
    for (size_t i = 0; i < n; i++) {
        acc += list->items[i].payload;
    }
    set_out_kind(out_kind, KIND_I64);
    return acc;
}

int64_t hyper_rt_builtin_round(
    int64_t payload,
    int64_t kind,
    int64_t ndigits,
    int64_t ndigits_kind,
    int64_t line,
    int64_t line_kind,
    int64_t *out_kind
) {
    (void)line_kind;
    double x = as_f64(payload, kind, line, "round");
    if (ndigits_kind == KIND_NONE) {
        set_out_kind(out_kind, KIND_I64);
        return (int64_t)round(x);
    }
    if (ndigits_kind != KIND_I64 && ndigits_kind != KIND_U64) {
        runtime_error(line, "round() ndigits must be an integer");
    }
    double factor = pow(10.0, (double)ndigits);
    double rounded = round(x * factor) / factor;
    set_out_kind(out_kind, KIND_F64);
    return f64_bits(rounded);
}

int64_t hyper_rt_builtin_pow(
    int64_t base,
    int64_t base_kind,
    int64_t exp,
    int64_t exp_kind,
    int64_t line,
    int64_t line_kind,
    int64_t *out_kind
) {
    (void)line_kind;
    if (!is_numeric(base_kind) || !is_numeric(exp_kind)) {
        runtime_error(line, "pow() expected numbers");
    }
    int base_is_int = base_kind == KIND_I64 || base_kind == KIND_U64 || base_kind == KIND_BOOL;
    int exp_is_int = exp_kind == KIND_I64 || exp_kind == KIND_U64 || exp_kind == KIND_BOOL;
    if (base_is_int && exp_is_int && exp >= 0) {
        set_out_kind(out_kind, KIND_I64);
        return hyper_rt_pow_i64(base, exp);
    }
    set_out_kind(out_kind, KIND_F64);
    return f64_bits(hyper_rt_pow_f64(
        as_f64(base, base_kind, line, "pow"),
        as_f64(exp, exp_kind, line, "pow")
    ));
}

int64_t hyper_rt_builtin_divmod(
    int64_t a,
    int64_t a_kind,
    int64_t b,
    int64_t b_kind,
    int64_t line,
    int64_t line_kind
) {
    (void)line_kind;
    if (!is_numeric(a_kind) || !is_numeric(b_kind)) {
        runtime_error(line, "divmod() expected numbers");
    }
    int64_t list = hyper_rt_list_new();
    int use_float = a_kind == KIND_F64 || b_kind == KIND_F64;
    if (use_float) {
        double af = as_f64(a, a_kind, line, "divmod");
        double bf = as_f64(b, b_kind, line, "divmod");
        if (bf == 0.0) {
            hyper_rt_div_by_zero(line);
        }
        double q = hyper_rt_floor_div_f64(af, bf);
        double r = af - q * bf;
        hyper_rt_list_push(list, f64_bits(q), KIND_F64);
        hyper_rt_list_push(list, f64_bits(r), KIND_F64);
    } else {
        if (b == 0) {
            hyper_rt_div_by_zero(line);
        }
        int64_t q = hyper_rt_floor_div_i64(a, b);
        int64_t r = a - q * b;
        hyper_rt_list_push(list, q, KIND_I64);
        hyper_rt_list_push(list, r, KIND_I64);
    }
    return list;
}

static int utf8_encode(uint32_t cp, char out[5]) {
    if (cp <= 0x7F) {
        out[0] = (char)cp;
        out[1] = 0;
        return 1;
    }
    if (cp <= 0x7FF) {
        out[0] = (char)(0xC0 | (cp >> 6));
        out[1] = (char)(0x80 | (cp & 0x3F));
        out[2] = 0;
        return 2;
    }
    if (cp <= 0xFFFF) {
        out[0] = (char)(0xE0 | (cp >> 12));
        out[1] = (char)(0x80 | ((cp >> 6) & 0x3F));
        out[2] = (char)(0x80 | (cp & 0x3F));
        out[3] = 0;
        return 3;
    }
    out[0] = (char)(0xF0 | (cp >> 18));
    out[1] = (char)(0x80 | ((cp >> 12) & 0x3F));
    out[2] = (char)(0x80 | ((cp >> 6) & 0x3F));
    out[3] = (char)(0x80 | (cp & 0x3F));
    out[4] = 0;
    return 4;
}

int64_t hyper_rt_builtin_chr(int64_t payload, int64_t kind, int64_t line, int64_t line_kind) {
    (void)line_kind;
    if (kind != KIND_I64 && kind != KIND_U64) {
        runtime_error(line, "chr() expected an integer");
    }
    if (payload < 0 || payload > 0x10FFFF) {
        runtime_error(line, "chr() arg not in range(0x110000)");
    }
    uint32_t cp = (uint32_t)payload;
    if (cp >= 0xD800 && cp <= 0xDFFF) {
        runtime_error(line, "chr() arg is a surrogate");
    }
    char buf[5];
    utf8_encode(cp, buf);
    return (int64_t)(intptr_t)rt_strdup(buf);
}

static int utf8_first_scalar(const char *s, uint32_t *out) {
    unsigned char c0 = (unsigned char)s[0];
    if (c0 == 0) {
        return 0;
    }
    if (c0 < 0x80) {
        *out = c0;
        return s[1] == 0 ? 1 : 2;
    }
    if ((c0 & 0xE0) == 0xC0) {
        unsigned char c1 = (unsigned char)s[1];
        if ((c1 & 0xC0) != 0x80) {
            return -1;
        }
        *out = ((uint32_t)(c0 & 0x1F) << 6) | (uint32_t)(c1 & 0x3F);
        return s[2] == 0 ? 1 : 2;
    }
    if ((c0 & 0xF0) == 0xE0) {
        unsigned char c1 = (unsigned char)s[1];
        unsigned char c2 = (unsigned char)s[2];
        if ((c1 & 0xC0) != 0x80 || (c2 & 0xC0) != 0x80) {
            return -1;
        }
        *out = ((uint32_t)(c0 & 0x0F) << 12) | ((uint32_t)(c1 & 0x3F) << 6) | (uint32_t)(c2 & 0x3F);
        return s[3] == 0 ? 1 : 2;
    }
    if ((c0 & 0xF8) == 0xF0) {
        unsigned char c1 = (unsigned char)s[1];
        unsigned char c2 = (unsigned char)s[2];
        unsigned char c3 = (unsigned char)s[3];
        if ((c1 & 0xC0) != 0x80 || (c2 & 0xC0) != 0x80 || (c3 & 0xC0) != 0x80) {
            return -1;
        }
        *out = ((uint32_t)(c0 & 0x07) << 18) | ((uint32_t)(c1 & 0x3F) << 12)
            | ((uint32_t)(c2 & 0x3F) << 6) | (uint32_t)(c3 & 0x3F);
        return s[4] == 0 ? 1 : 2;
    }
    return -1;
}

int64_t hyper_rt_builtin_ord(int64_t payload, int64_t kind, int64_t line, int64_t line_kind) {
    (void)line_kind;
    if (kind != KIND_STR) {
        runtime_error(line, "ord() expected a string");
    }
    uint32_t cp = 0;
    int status = utf8_first_scalar(cstr(payload), &cp);
    if (status != 1) {
        runtime_error(line, "ord() expected a character string of length 1");
    }
    return (int64_t)cp;
}

static int64_t format_radix(
    int64_t payload,
    int64_t kind,
    int64_t line,
    const char *name,
    const char *prefix,
    int radix
) {
    if (kind != KIND_I64 && kind != KIND_U64 && kind != KIND_BOOL) {
        char buf[64];
        snprintf(buf, sizeof(buf), "%s() expected an integer", name);
        runtime_error(line, buf);
    }
    char digits[128];
    char out[160];
    int neg = 0;
    uint64_t v;
    if (kind == KIND_U64) {
        v = (uint64_t)payload;
    } else if (payload < 0) {
        neg = 1;
        v = (uint64_t)(-(payload + 1)) + 1; /* wrapping abs for INT64_MIN */
    } else {
        v = (uint64_t)payload;
    }
    if (v == 0) {
        strcpy(digits, "0");
    } else {
        int i = 0;
        char tmp[128];
        while (v > 0) {
            uint64_t d = v % (uint64_t)radix;
            tmp[i++] = (char)(d < 10 ? ('0' + d) : ('a' + (d - 10)));
            v /= (uint64_t)radix;
        }
        for (int j = 0; j < i; j++) {
            digits[j] = tmp[i - 1 - j];
        }
        digits[i] = 0;
    }
    if (neg) {
        snprintf(out, sizeof(out), "-%s%s", prefix, digits);
    } else {
        snprintf(out, sizeof(out), "%s%s", prefix, digits);
    }
    return (int64_t)(intptr_t)rt_strdup(out);
}

int64_t hyper_rt_builtin_bin(int64_t payload, int64_t kind, int64_t line, int64_t line_kind) {
    (void)line_kind;
    return format_radix(payload, kind, line, "bin", "0b", 2);
}

int64_t hyper_rt_builtin_hex(int64_t payload, int64_t kind, int64_t line, int64_t line_kind) {
    (void)line_kind;
    return format_radix(payload, kind, line, "hex", "0x", 16);
}

int64_t hyper_rt_builtin_oct(int64_t payload, int64_t kind, int64_t line, int64_t line_kind) {
    (void)line_kind;
    return format_radix(payload, kind, line, "oct", "0o", 8);
}

int64_t hyper_rt_builtin_int(
    int64_t payload,
    int64_t kind,
    int64_t line,
    int64_t line_kind,
    int64_t *out_kind
) {
    (void)line_kind;
    set_out_kind(out_kind, KIND_I64);
    if (kind == KIND_I64 || kind == KIND_BOOL || kind == KIND_U64) {
        return payload;
    }
    if (kind == KIND_F64) {
        double d;
        memcpy(&d, &payload, sizeof(d));
        return (int64_t)d;
    }
    if (kind == KIND_STR) {
        const char *s = cstr(payload);
        while (*s && isspace((unsigned char)*s)) {
            s++;
        }
        char *end = NULL;
        long long n = strtoll(s, &end, 10);
        while (end && *end && isspace((unsigned char)*end)) {
            end++;
        }
        if (!end || end == s || *end != '\0') {
            char buf[160];
            snprintf(buf, sizeof(buf), "invalid literal for int(): '%s'", cstr(payload));
            runtime_error(line, buf);
        }
        return (int64_t)n;
    }
    runtime_error(line, "int() argument must be a number or string");
    return 0;
}

int64_t hyper_rt_builtin_float(
    int64_t payload,
    int64_t kind,
    int64_t line,
    int64_t line_kind,
    int64_t *out_kind
) {
    (void)line_kind;
    set_out_kind(out_kind, KIND_F64);
    if (kind == KIND_I64 || kind == KIND_BOOL) {
        return f64_bits((double)payload);
    }
    if (kind == KIND_U64) {
        return f64_bits((double)(uint64_t)payload);
    }
    if (kind == KIND_F64) {
        return payload;
    }
    if (kind == KIND_STR) {
        const char *s = cstr(payload);
        while (*s && isspace((unsigned char)*s)) {
            s++;
        }
        char *end = NULL;
        double d = strtod(s, &end);
        while (end && *end && isspace((unsigned char)*end)) {
            end++;
        }
        if (!end || end == s || *end != '\0') {
            char buf[160];
            snprintf(buf, sizeof(buf), "could not convert string to float: '%s'", cstr(payload));
            runtime_error(line, buf);
        }
        return f64_bits(d);
    }
    runtime_error(line, "float() argument must be a number or string");
    return 0;
}

int64_t hyper_rt_builtin_str(int64_t payload, int64_t kind, int64_t line, int64_t line_kind) {
    (void)line;
    (void)line_kind;
    return hyper_rt_value_to_str(payload, kind);
}

int64_t hyper_rt_builtin_bool(int64_t payload, int64_t kind, int64_t line, int64_t line_kind) {
    (void)line;
    (void)line_kind;
    return is_truthy(payload, kind) ? 1 : 0;
}

int64_t hyper_rt_builtin_all(int64_t payload, int64_t kind, int64_t line, int64_t line_kind) {
    (void)line_kind;
    const RtList *list = as_list(payload, kind, line, "all");
    size_t n = list ? list->len : 0;
    for (size_t i = 0; i < n; i++) {
        if (!is_truthy(list->items[i].payload, list->items[i].kind)) {
            return 0;
        }
    }
    return 1;
}

int64_t hyper_rt_builtin_any(int64_t payload, int64_t kind, int64_t line, int64_t line_kind) {
    (void)line_kind;
    const RtList *list = as_list(payload, kind, line, "any");
    size_t n = list ? list->len : 0;
    for (size_t i = 0; i < n; i++) {
        if (is_truthy(list->items[i].payload, list->items[i].kind)) {
            return 1;
        }
    }
    return 0;
}

static int cmp_i64(const void *a, const void *b) {
    int64_t x = *(const int64_t *)a;
    int64_t y = *(const int64_t *)b;
    return (x > y) - (x < y);
}

static int cmp_f64(const void *a, const void *b) {
    double x = *(const double *)a;
    double y = *(const double *)b;
    if (x < y) {
        return -1;
    }
    if (x > y) {
        return 1;
    }
    return 0;
}

static int cmp_str_ptr(const void *a, const void *b) {
    const char *x = *(const char *const *)a;
    const char *y = *(const char *const *)b;
    return strcmp(x ? x : "", y ? y : "");
}

int64_t hyper_rt_builtin_sorted(int64_t payload, int64_t kind, int64_t line, int64_t line_kind) {
    (void)line_kind;
    const RtList *list = as_list(payload, kind, line, "sorted");
    int64_t out = hyper_rt_list_new();
    size_t n = list ? list->len : 0;
    if (n == 0) {
        return out;
    }
    int64_t elem_kind = list->items[0].kind;
    for (size_t i = 1; i < n; i++) {
        if (list->items[i].kind != elem_kind) {
            runtime_error(line, "sorted() requires a homogeneous list");
        }
    }
    if (elem_kind == KIND_I64 || elem_kind == KIND_BOOL) {
        int64_t *vals = (int64_t *)malloc(n * sizeof(int64_t));
        if (!vals) {
            runtime_error(line, "out of memory");
        }
        for (size_t i = 0; i < n; i++) {
            vals[i] = list->items[i].payload;
        }
        qsort(vals, n, sizeof(int64_t), cmp_i64);
        for (size_t i = 0; i < n; i++) {
            hyper_rt_list_push(out, vals[i], elem_kind);
        }
        free(vals);
    } else if (elem_kind == KIND_F64) {
        double *vals = (double *)malloc(n * sizeof(double));
        if (!vals) {
            runtime_error(line, "out of memory");
        }
        for (size_t i = 0; i < n; i++) {
            memcpy(&vals[i], &list->items[i].payload, sizeof(double));
        }
        qsort(vals, n, sizeof(double), cmp_f64);
        for (size_t i = 0; i < n; i++) {
            hyper_rt_list_push(out, f64_bits(vals[i]), KIND_F64);
        }
        free(vals);
    } else if (elem_kind == KIND_STR) {
        const char **vals = (const char **)malloc(n * sizeof(char *));
        if (!vals) {
            runtime_error(line, "out of memory");
        }
        for (size_t i = 0; i < n; i++) {
            vals[i] = cstr(list->items[i].payload);
        }
        qsort(vals, n, sizeof(char *), cmp_str_ptr);
        for (size_t i = 0; i < n; i++) {
            hyper_rt_list_push(out, (int64_t)(intptr_t)rt_strdup(vals[i]), KIND_STR);
        }
        free(vals);
    } else {
        runtime_error(line, "sorted() supports lists of int, float, or str");
    }
    return out;
}

int64_t hyper_rt_builtin_reversed(int64_t payload, int64_t kind, int64_t line, int64_t line_kind) {
    (void)line_kind;
    const RtList *list = as_list(payload, kind, line, "reversed");
    int64_t out = hyper_rt_list_new();
    size_t n = list ? list->len : 0;
    for (size_t i = n; i > 0; i--) {
        RtValue item = list->items[i - 1];
        hyper_rt_list_push(out, clone_item(item.payload, item.kind), item.kind);
    }
    return out;
}
