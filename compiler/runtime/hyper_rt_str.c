#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { KIND_STR = 2, KIND_NONE = 4 };

extern int64_t hyper_rt_list_new(void);
extern void hyper_rt_list_push(int64_t list, int64_t payload, int64_t kind);

static void rt_fatal(int64_t line, const char *msg) {
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

static const char *require_str(int64_t payload, int64_t kind, int64_t line) {
    if (kind != KIND_STR) {
        rt_fatal(line, "expected a string receiver");
    }
    return payload ? (const char *)(intptr_t)payload : "";
}

static const char *optional_str_arg(int64_t payload, int64_t kind, const char *fallback) {
    if (kind == KIND_NONE) {
        return fallback;
    }
    if (kind == KIND_STR) {
        return payload ? (const char *)(intptr_t)payload : "";
    }
    return fallback;
}

static const char *require_str_arg(int64_t payload, int64_t kind, int64_t line, const char *method) {
    if (kind != KIND_STR) {
        rt_fatal(line, "'replace' expects a string argument");
        (void)method;
    }
    return payload ? (const char *)(intptr_t)payload : "";
}

static char *ascii_upper(const char *s) {
    size_t n = strlen(s);
    char *out = (char *)malloc(n + 1);
    if (!out) {
        return NULL;
    }
    for (size_t i = 0; i < n; i++) {
        out[i] = (char)toupper((unsigned char)s[i]);
    }
    out[n] = '\0';
    return out;
}

static char *ascii_lower(const char *s) {
    size_t n = strlen(s);
    char *out = (char *)malloc(n + 1);
    if (!out) {
        return NULL;
    }
    for (size_t i = 0; i < n; i++) {
        out[i] = (char)tolower((unsigned char)s[i]);
    }
    out[n] = '\0';
    return out;
}

static char *trim_both(const char *s) {
    while (*s && isspace((unsigned char)*s)) {
        s++;
    }
    if (!*s) {
        return rt_strdup("");
    }
    const char *end = s + strlen(s) - 1;
    while (end > s && isspace((unsigned char)*end)) {
        end--;
    }
    size_t len = (size_t)(end - s + 1);
    char *out = (char *)malloc(len + 1);
    if (!out) {
        return NULL;
    }
    memcpy(out, s, len);
    out[len] = '\0';
    return out;
}

static char *trim_start(const char *s) {
    while (*s && isspace((unsigned char)*s)) {
        s++;
    }
    return rt_strdup(s);
}

static char *trim_end(const char *s) {
    size_t n = strlen(s);
    while (n > 0 && isspace((unsigned char)s[n - 1])) {
        n--;
    }
    char *out = (char *)malloc(n + 1);
    if (!out) {
        return NULL;
    }
    memcpy(out, s, n);
    out[n] = '\0';
    return out;
}

static int64_t return_str_copy(const char *s) {
    char *copy = rt_strdup(s);
    return (int64_t)(intptr_t)copy;
}

static int64_t return_owned(char *owned) {
    if (!owned) {
        return 0;
    }
    return (int64_t)(intptr_t)owned;
}

int64_t hyper_rt_str_upper(int64_t payload, int64_t kind, int64_t line, int64_t _line_kind) {
    (void)_line_kind;
    const char *s = require_str(payload, kind, line);
    return return_owned(ascii_upper(s));
}

int64_t hyper_rt_str_lower(int64_t payload, int64_t kind, int64_t line, int64_t _line_kind) {
    (void)_line_kind;
    const char *s = require_str(payload, kind, line);
    return return_owned(ascii_lower(s));
}

int64_t hyper_rt_str_strip(int64_t payload, int64_t kind, int64_t line, int64_t _line_kind) {
    (void)_line_kind;
    const char *s = require_str(payload, kind, line);
    char *trimmed = trim_both(s);
    if (trimmed && strcmp(trimmed, s) == 0) {
        free(trimmed);
        return return_str_copy(s);
    }
    return return_owned(trimmed);
}

int64_t hyper_rt_str_lstrip(int64_t payload, int64_t kind, int64_t line, int64_t _line_kind) {
    (void)_line_kind;
    const char *s = require_str(payload, kind, line);
    char *trimmed = trim_start(s);
    if (trimmed && strcmp(trimmed, s) == 0) {
        free(trimmed);
        return return_str_copy(s);
    }
    return return_owned(trimmed);
}

int64_t hyper_rt_str_rstrip(int64_t payload, int64_t kind, int64_t line, int64_t _line_kind) {
    (void)_line_kind;
    const char *s = require_str(payload, kind, line);
    char *trimmed = trim_end(s);
    if (trimmed && strcmp(trimmed, s) == 0) {
        free(trimmed);
        return return_str_copy(s);
    }
    return return_owned(trimmed);
}

int64_t hyper_rt_str_startswith(
    int64_t payload,
    int64_t kind,
    int64_t prefix,
    int64_t prefix_kind,
    int64_t line,
    int64_t _line_kind
) {
    (void)_line_kind;
    const char *s = require_str(payload, kind, line);
    const char *p = require_str_arg(prefix, prefix_kind, line, "startswith");
    size_t plen = strlen(p);
    if (plen == 0) {
        return 1;
    }
    return strncmp(s, p, plen) == 0 ? 1 : 0;
}

int64_t hyper_rt_str_endswith(
    int64_t payload,
    int64_t kind,
    int64_t suffix,
    int64_t suffix_kind,
    int64_t line,
    int64_t _line_kind
) {
    (void)_line_kind;
    const char *s = require_str(payload, kind, line);
    const char *p = require_str_arg(suffix, suffix_kind, line, "endswith");
    size_t slen = strlen(s);
    size_t plen = strlen(p);
    if (plen > slen) {
        return 0;
    }
    return strcmp(s + slen - plen, p) == 0 ? 1 : 0;
}

int64_t hyper_rt_str_split(
    int64_t payload,
    int64_t kind,
    int64_t delim,
    int64_t delim_kind,
    int64_t line,
    int64_t _line_kind
) {
    (void)_line_kind;
    const char *s = require_str(payload, kind, line);
    const char *delimiter = optional_str_arg(delim, delim_kind, " ");
    int64_t list = hyper_rt_list_new();
    if (!*s) {
        return list;
    }

    size_t dlen = strlen(delimiter);
    if (dlen == 0) {
        for (const char *p = s; *p; p++) {
            char ch[2] = {*p, '\0'};
            hyper_rt_list_push(list, (int64_t)(intptr_t)rt_strdup(ch), KIND_STR);
        }
        return list;
    }

    const char *start = s;
    const char *found;
    while ((found = strstr(start, delimiter)) != NULL) {
        size_t part_len = (size_t)(found - start);
        char *part = (char *)malloc(part_len + 1);
        if (part) {
            memcpy(part, start, part_len);
            part[part_len] = '\0';
            hyper_rt_list_push(list, (int64_t)(intptr_t)part, KIND_STR);
        }
        start = found + dlen;
    }
    hyper_rt_list_push(list, (int64_t)(intptr_t)rt_strdup(start), KIND_STR);
    return list;
}

int64_t hyper_rt_str_replace(
    int64_t payload,
    int64_t kind,
    int64_t old,
    int64_t old_kind,
    int64_t new_val,
    int64_t new_kind,
    int64_t line,
    int64_t _line_kind
) {
    (void)_line_kind;
    const char *s = require_str(payload, kind, line);
    const char *old_s = require_str_arg(old, old_kind, line, "replace");
    const char *new_s = require_str_arg(new_val, new_kind, line, "replace");

    if (!*old_s) {
        return return_str_copy(s);
    }

    size_t old_len = strlen(old_s);
    size_t new_len = strlen(new_s);
    size_t s_len = strlen(s);
    size_t count = 0;
    const char *p = s;
    while ((p = strstr(p, old_s)) != NULL) {
        count++;
        p += old_len;
    }

    size_t out_len = s_len + count * (new_len - old_len);
    char *out = (char *)malloc(out_len + 1);
    if (!out) {
        return 0;
    }

    char *dst = out;
    p = s;
    while ((p = strstr(p, old_s)) != NULL) {
        size_t prefix = (size_t)(p - s);
        memcpy(dst, s, prefix);
        dst += prefix;
        memcpy(dst, new_s, new_len);
        dst += new_len;
        s = p + old_len;
        p = s;
    }
    strcpy(dst, s);
    return (int64_t)(intptr_t)out;
}
