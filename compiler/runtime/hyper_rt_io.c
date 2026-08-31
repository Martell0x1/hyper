#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { KIND_NONE = 4 };

extern void hyper_rt_print_value(int64_t payload, int64_t kind);
extern void hyper_rt_print_newline(void);

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

int64_t hyper_rt_input(int64_t prompt, int64_t prompt_kind, int64_t line, int64_t _line_kind) {
    (void)_line_kind;
    if (prompt_kind != KIND_NONE) {
        hyper_rt_print_value(prompt, prompt_kind);
        hyper_rt_print_newline();
    }
    char buf[4096];
    if (!fgets(buf, sizeof(buf), stdin)) {
        runtime_error(line, "failed to read line from stdin");
    }
    size_t len = strlen(buf);
    while (len > 0 && (buf[len - 1] == '\n' || buf[len - 1] == '\r')) {
        buf[--len] = '\0';
    }
    return (int64_t)(intptr_t)rt_strdup(buf);
}
