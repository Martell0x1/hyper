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
    KIND_DICT = 6
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

extern int64_t __main__(void);

void hyper_rt_print_i64(int64_t v) {
    printf("%lld ", (long long)v);
}

void hyper_rt_print_f64(double v) {
    printf("%g ", v);
}

void hyper_rt_print_str(const char *s) {
    if (!s) {
        printf(" ");
        return;
    }
    printf("%s ", s);
}

void hyper_rt_print_newline(void) {
    printf("\n");
}

int64_t hyper_rt_pow_i64(int64_t base, int64_t exp) {
    if (exp < 0) {
        return 0;
    }
    int64_t result = 1;
    uint64_t e = (uint64_t)exp;
    int64_t b = base;
    while (e > 0) {
        if (e & 1) {
            result *= b;
        }
        b *= b;
        e >>= 1;
    }
    return result;
}

double hyper_rt_pow_f64(double base, double exp) {
    return pow(base, exp);
}

static void format_value(const RtValue *v);

static void format_list(const RtList *list) {
    putchar('[');
    for (size_t i = 0; i < list->len; i++) {
        if (i > 0) {
            printf(", ");
        }
        format_value(&list->items[i]);
    }
    putchar(']');
}

static void format_dict(const RtDict *dict) {
    putchar('{');
    for (size_t i = 0; i < dict->len; i++) {
        if (i > 0) {
            printf(", ");
        }
        printf("%s: ", dict->entries[i].key ? dict->entries[i].key : "");
        format_value(&dict->entries[i].value);
    }
    putchar('}');
}

static void format_value(const RtValue *v) {
    switch (v->kind) {
    case KIND_I64:
        printf("%lld", (long long)v->payload);
        break;
    case KIND_F64: {
        double d;
        memcpy(&d, &v->payload, sizeof(d));
        printf("%g", d);
        break;
    }
    case KIND_STR:
        if (v->payload) {
            fputs((const char *)(intptr_t)v->payload, stdout);
        }
        break;
    case KIND_BOOL:
        fputs(v->payload ? "true" : "false", stdout);
        break;
    case KIND_NONE:
        fputs("None", stdout);
        break;
    case KIND_LIST:
        if (v->payload) {
            format_list((const RtList *)(intptr_t)v->payload);
        } else {
            fputs("[]", stdout);
        }
        break;
    case KIND_DICT:
        if (v->payload) {
            format_dict((const RtDict *)(intptr_t)v->payload);
        } else {
            fputs("{}", stdout);
        }
        break;
    default:
        fputs("<?>", stdout);
        break;
    }
}

int64_t hyper_rt_list_new(void) {
    RtList *list = (RtList *)calloc(1, sizeof(RtList));
    return (int64_t)(intptr_t)list;
}

void hyper_rt_list_push(int64_t list_h, int64_t value, int64_t kind) {
    if (!list_h) {
        return;
    }
    RtList *list = (RtList *)(intptr_t)list_h;
    if (list->len + 1 > list->cap) {
        size_t ncap = list->cap ? list->cap * 2 : 4;
        RtValue *ni = (RtValue *)realloc(list->items, ncap * sizeof(RtValue));
        if (!ni) {
            return;
        }
        list->items = ni;
        list->cap = ncap;
    }
    list->items[list->len].kind = kind;
    list->items[list->len].payload = value;
    list->len++;
}

void hyper_rt_print_list(int64_t list_h) {
    if (!list_h) {
        printf("[] ");
        return;
    }
    format_list((const RtList *)(intptr_t)list_h);
    putchar(' ');
}

int64_t hyper_rt_dict_new(void) {
    RtDict *dict = (RtDict *)calloc(1, sizeof(RtDict));
    return (int64_t)(intptr_t)dict;
}

static char *key_to_string(int64_t key, int64_t key_kind) {
    if (key_kind == KIND_STR) {
        const char *s = key ? (const char *)(intptr_t)key : "";
        size_t n = strlen(s);
        char *out = (char *)malloc(n + 1);
        if (!out) {
            return NULL;
        }
        memcpy(out, s, n + 1);
        return out;
    }
    char buf[32];
    snprintf(buf, sizeof(buf), "%lld", (long long)key);
    size_t n = strlen(buf);
    char *out = (char *)malloc(n + 1);
    if (!out) {
        return NULL;
    }
    memcpy(out, buf, n + 1);
    return out;
}

void hyper_rt_dict_push(
    int64_t dict_h,
    int64_t key,
    int64_t key_kind,
    int64_t val,
    int64_t val_kind
) {
    if (!dict_h) {
        return;
    }
    RtDict *dict = (RtDict *)(intptr_t)dict_h;
    if (dict->len + 1 > dict->cap) {
        size_t ncap = dict->cap ? dict->cap * 2 : 4;
        RtDictEntry *ne = (RtDictEntry *)realloc(dict->entries, ncap * sizeof(RtDictEntry));
        if (!ne) {
            return;
        }
        dict->entries = ne;
        dict->cap = ncap;
    }
    dict->entries[dict->len].key = key_to_string(key, key_kind);
    dict->entries[dict->len].value.kind = val_kind;
    dict->entries[dict->len].value.payload = val;
    dict->len++;
}

void hyper_rt_print_dict(int64_t dict_h) {
    if (!dict_h) {
        printf("{} ");
        return;
    }
    format_dict((const RtDict *)(intptr_t)dict_h);
    putchar(' ');
}

int main(void) {
    __main__();
    return 0;
}
