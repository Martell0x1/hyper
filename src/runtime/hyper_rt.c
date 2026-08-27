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

void hyper_rt_print_value(int64_t payload, int64_t kind) {
    RtValue v;
    v.kind = kind;
    v.payload = payload;
    format_value(&v);
    putchar(' ');
}

int64_t hyper_rt_list_get(int64_t list_h, int64_t index, int64_t *out_kind) {
    if (!list_h || !out_kind) {
        return 0;
    }
    const RtList *list = (const RtList *)(intptr_t)list_h;
    if (index < 0 || (size_t)index >= list->len) {
        *out_kind = KIND_NONE;
        return 0;
    }
    *out_kind = list->items[index].kind;
    return list->items[index].payload;
}

void hyper_rt_list_set(int64_t list_h, int64_t index, int64_t value, int64_t kind) {
    if (!list_h) {
        return;
    }
    RtList *list = (RtList *)(intptr_t)list_h;
    if (index < 0 || (size_t)index >= list->len) {
        return;
    }
    list->items[index].kind = kind;
    list->items[index].payload = value;
}

int64_t hyper_rt_dict_get(
    int64_t dict_h,
    int64_t key,
    int64_t key_kind,
    int64_t *out_kind
) {
    if (!dict_h || !out_kind) {
        return 0;
    }
    const RtDict *dict = (const RtDict *)(intptr_t)dict_h;
    char *k = key_to_string(key, key_kind);
    if (!k) {
        *out_kind = KIND_NONE;
        return 0;
    }
    for (size_t i = 0; i < dict->len; i++) {
        if (dict->entries[i].key && strcmp(dict->entries[i].key, k) == 0) {
            free(k);
            *out_kind = dict->entries[i].value.kind;
            return dict->entries[i].value.payload;
        }
    }
    free(k);
    *out_kind = KIND_NONE;
    return 0;
}

void hyper_rt_dict_set(
    int64_t dict_h,
    int64_t key,
    int64_t key_kind,
    int64_t value,
    int64_t val_kind
) {
    if (!dict_h) {
        return;
    }
    RtDict *dict = (RtDict *)(intptr_t)dict_h;
    char *k = key_to_string(key, key_kind);
    if (!k) {
        return;
    }
    for (size_t i = 0; i < dict->len; i++) {
        if (dict->entries[i].key && strcmp(dict->entries[i].key, k) == 0) {
            free(k);
            dict->entries[i].value.kind = val_kind;
            dict->entries[i].value.payload = value;
            return;
        }
    }
    if (dict->len + 1 > dict->cap) {
        size_t ncap = dict->cap ? dict->cap * 2 : 4;
        RtDictEntry *ne = (RtDictEntry *)realloc(dict->entries, ncap * sizeof(RtDictEntry));
        if (!ne) {
            free(k);
            return;
        }
        dict->entries = ne;
        dict->cap = ncap;
    }
    dict->entries[dict->len].key = k;
    dict->entries[dict->len].value.kind = val_kind;
    dict->entries[dict->len].value.payload = value;
    dict->len++;
}

int64_t hyper_rt_list_len(int64_t list_h) {
    if (!list_h) {
        return 0;
    }
    const RtList *list = (const RtList *)(intptr_t)list_h;
    return (int64_t)list->len;
}

int64_t hyper_rt_value_to_str(int64_t payload, int64_t kind) {
    RtValue v;
    v.kind = kind;
    v.payload = payload;

    char buf[512];
    switch (kind) {
    case KIND_I64:
        snprintf(buf, sizeof(buf), "%lld", (long long)payload);
        break;
    case KIND_F64: {
        double d;
        memcpy(&d, &payload, sizeof(d));
        snprintf(buf, sizeof(buf), "%g", d);
        break;
    }
    case KIND_STR: {
        const char *s = payload ? (const char *)(intptr_t)payload : "";
        size_t n = strlen(s);
        char *out = (char *)malloc(n + 1);
        if (!out) {
            return 0;
        }
        memcpy(out, s, n + 1);
        return (int64_t)(intptr_t)out;
    }
    case KIND_BOOL:
        snprintf(buf, sizeof(buf), "%s", payload ? "true" : "false");
        break;
    case KIND_NONE:
        snprintf(buf, sizeof(buf), "None");
        break;
    case KIND_LIST:
    case KIND_DICT: {
        /* Fall back to a small fixed buffer via format helpers into temp FILE-less path. */
        snprintf(buf, sizeof(buf), "<?>");
        break;
    }
    default:
        snprintf(buf, sizeof(buf), "<?>");
        break;
    }
    size_t n = strlen(buf);
    char *out = (char *)malloc(n + 1);
    if (!out) {
        return 0;
    }
    memcpy(out, buf, n + 1);
    return (int64_t)(intptr_t)out;
}

int64_t hyper_rt_str_concat(int64_t left, int64_t right) {
    const char *a = left ? (const char *)(intptr_t)left : "";
    const char *b = right ? (const char *)(intptr_t)right : "";
    size_t na = strlen(a);
    size_t nb = strlen(b);
    char *out = (char *)malloc(na + nb + 1);
    if (!out) {
        return 0;
    }
    memcpy(out, a, na);
    memcpy(out + na, b, nb + 1);
    return (int64_t)(intptr_t)out;
}

int main(void) {
    __main__();
    return 0;
}
