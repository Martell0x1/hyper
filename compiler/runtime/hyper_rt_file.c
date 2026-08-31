#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum {
    KIND_I64 = 0,
    KIND_STR = 2,
    KIND_BOOL = 3,
    KIND_NONE = 4,
    KIND_LIST = 5
};

#define BUFFER_CAPACITY 65536

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
    FILE *fp;
    char *path;
    char *mode;
    int readable;
    int writable;
    int closed;
    unsigned char *read_buf;
    size_t read_cap;
    size_t read_pos;
    size_t read_len;
    unsigned char *write_buf;
    size_t write_cap;
    size_t write_len;
} RtFile;

extern int64_t hyper_rt_list_new(void);
extern void hyper_rt_list_push(int64_t list, int64_t value, int64_t kind);

static void hyper_rt_runtime_error(int64_t line, const char *msg) {
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

static char *rt_strndup(const unsigned char *s, size_t n) {
    char *out = (char *)malloc(n + 1);
    if (!out) {
        return NULL;
    }
    memcpy(out, s, n);
    out[n] = '\0';
    return out;
}

static int parse_mode(const char *mode, int *readable, int *writable, const char **fopen_mode) {
    char norm[16];
    size_t j = 0;
    for (size_t i = 0; mode[i] && j + 1 < sizeof(norm); i++) {
        if (mode[i] != 'b' && mode[i] != 't') {
            norm[j++] = mode[i];
        }
    }
    norm[j] = '\0';
    if (norm[0] == '\0' || strcmp(norm, "r") == 0) {
        *readable = 1;
        *writable = 0;
        *fopen_mode = "rb";
        return 0;
    }
    if (strcmp(norm, "r+") == 0 || strcmp(norm, "+r") == 0) {
        *readable = 1;
        *writable = 1;
        *fopen_mode = "r+b";
        return 0;
    }
    if (strcmp(norm, "w") == 0) {
        *readable = 0;
        *writable = 1;
        *fopen_mode = "wb";
        return 0;
    }
    if (strcmp(norm, "w+") == 0 || strcmp(norm, "+w") == 0) {
        *readable = 1;
        *writable = 1;
        *fopen_mode = "w+b";
        return 0;
    }
    if (strcmp(norm, "a") == 0) {
        *readable = 0;
        *writable = 1;
        *fopen_mode = "ab";
        return 0;
    }
    if (strcmp(norm, "a+") == 0 || strcmp(norm, "+a") == 0) {
        *readable = 1;
        *writable = 1;
        *fopen_mode = "a+b";
        return 0;
    }
    return -1;
}

static RtFile *rt_file_from_handle(int64_t handle, int64_t line) {
    if (handle == 0) {
        hyper_rt_runtime_error(line, "invalid file handle");
    }
    return (RtFile *)(intptr_t)handle;
}

static void rt_check_open(RtFile *f, int64_t line) {
    if (f->closed) {
        hyper_rt_runtime_error(line, "I/O operation on closed file");
    }
}

static void rt_flush_writes(RtFile *f) {
    if (f->write_len > 0) {
        fwrite(f->write_buf, 1, f->write_len, f->fp);
        f->write_len = 0;
    }
}

static size_t rt_unread(RtFile *f) {
    return f->read_len - f->read_pos;
}

static void rt_release_read_ahead(RtFile *f) {
    size_t pending = rt_unread(f);
    if (pending > 0) {
        fseek(f->fp, -(long)pending, SEEK_CUR);
    }
    f->read_pos = 0;
    f->read_len = 0;
}

static size_t rt_fill(RtFile *f) {
    if (f->read_pos < f->read_len) {
        return rt_unread(f);
    }
    rt_flush_writes(f);
    if (!f->read_buf) {
        f->read_buf = (unsigned char *)malloc(BUFFER_CAPACITY);
        f->read_cap = BUFFER_CAPACITY;
    }
    size_t n = fread(f->read_buf, 1, f->read_cap, f->fp);
    f->read_pos = 0;
    f->read_len = n;
    return n;
}

static void rt_io_error(RtFile *f, int64_t line, const char *op) {
    char buf[512];
    snprintf(buf, sizeof(buf), "%s failed on '%s': I/O error", op, f->path ? f->path : "?");
    hyper_rt_runtime_error(line, buf);
}

int64_t hyper_rt_file_open(int64_t path, int64_t path_kind, int64_t mode, int64_t mode_kind,
                           int64_t line, int64_t _line_kind) {
    (void)_line_kind;
    if (path_kind != KIND_STR) {
        hyper_rt_runtime_error(line, "open expects a file path");
    }
    const char *path_str = path ? (const char *)(intptr_t)path : "";
    const char *mode_str = "r";
    if (mode_kind == KIND_STR && mode != 0) {
        mode_str = (const char *)(intptr_t)mode;
    }
    int readable = 0;
    int writable = 0;
    const char *fopen_mode = "rb";
    if (parse_mode(mode_str, &readable, &writable, &fopen_mode) != 0) {
        char buf[128];
        snprintf(buf, sizeof(buf), "could not open '%s': invalid file mode '%s'", path_str, mode_str);
        hyper_rt_runtime_error(line, buf);
    }
    FILE *fp = fopen(path_str, fopen_mode);
    if (!fp) {
        char buf[256];
        snprintf(buf, sizeof(buf), "could not open '%s': %s", path_str, strerror(errno));
        hyper_rt_runtime_error(line, buf);
    }
    RtFile *f = (RtFile *)calloc(1, sizeof(RtFile));
    if (!f) {
        fclose(fp);
        hyper_rt_runtime_error(line, "out of memory");
    }
    f->fp = fp;
    f->path = rt_strdup(path_str);
    f->mode = rt_strdup(mode_str);
    f->readable = readable;
    f->writable = writable;
    return (int64_t)(intptr_t)f;
}

void hyper_rt_file_close(int64_t handle, int64_t _handle_kind, int64_t line, int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    if (handle == 0) {
        return;
    }
    RtFile *f = (RtFile *)(intptr_t)handle;
    if (!f->closed) {
        rt_flush_writes(f);
        fflush(f->fp);
        f->closed = 1;
        fclose(f->fp);
        f->fp = NULL;
    }
}

int64_t hyper_rt_file_read_all(int64_t handle, int64_t _handle_kind, int64_t line,
                               int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    RtFile *f = rt_file_from_handle(handle, line);
    rt_check_open(f, line);
    if (!f->readable) {
        rt_io_error(f, line, "read");
    }
    rt_flush_writes(f);
    unsigned char *out = NULL;
    size_t out_len = 0;
    size_t out_cap = rt_unread(f) + 4096;
    out = (unsigned char *)malloc(out_cap > 0 ? out_cap : 1);
    if (rt_unread(f) > 0) {
        memcpy(out, f->read_buf + f->read_pos, rt_unread(f));
        out_len = rt_unread(f);
        f->read_pos = f->read_len;
    }
    unsigned char chunk[4096];
    size_t n;
    while ((n = fread(chunk, 1, sizeof(chunk), f->fp)) > 0) {
        if (out_len + n > out_cap) {
            out_cap = (out_len + n) * 2;
            out = (unsigned char *)realloc(out, out_cap);
        }
        memcpy(out + out_len, chunk, n);
        out_len += n;
    }
    char *text = rt_strndup(out, out_len);
    free(out);
    return (int64_t)(intptr_t)text;
}

int64_t hyper_rt_file_read_n(int64_t handle, int64_t _handle_kind, int64_t count, int64_t count_kind,
                            int64_t line, int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    RtFile *f = rt_file_from_handle(handle, line);
    rt_check_open(f, line);
    if (!f->readable) {
        rt_io_error(f, line, "read");
    }
    size_t want = count_kind == KIND_I64 && count > 0 ? (size_t)count : 0;
    unsigned char *out = (unsigned char *)malloc(want + 1);
    size_t out_len = 0;
    while (out_len < want) {
        size_t avail = rt_fill(f);
        if (avail == 0) {
            break;
        }
        size_t take = want - out_len;
        if (take > avail) {
            take = avail;
        }
        memcpy(out + out_len, f->read_buf + f->read_pos, take);
        f->read_pos += take;
        out_len += take;
    }
    char *text = rt_strndup(out, out_len);
    free(out);
    return (int64_t)(intptr_t)text;
}

int64_t hyper_rt_file_readline(int64_t handle, int64_t _handle_kind, int64_t line,
                               int64_t _line_kind, int64_t *out_kind) {
    (void)_line_kind;
    RtFile *f = rt_file_from_handle(handle, line);
    rt_check_open(f, line);
    if (!f->readable) {
        rt_io_error(f, line, "readline");
    }
    unsigned char acc[4096];
    size_t acc_len = 0;
    for (;;) {
        size_t avail = rt_fill(f);
        if (avail == 0) {
            break;
        }
        const unsigned char *slice = f->read_buf + f->read_pos;
        const unsigned char *nl = (const unsigned char *)memchr(slice, '\n', avail);
        if (nl) {
            size_t idx = (size_t)(nl - slice);
            if (acc_len + idx < sizeof(acc)) {
                memcpy(acc + acc_len, slice, idx);
                acc_len += idx;
            }
            f->read_pos += idx + 1;
            if (acc_len > 0 && acc[acc_len - 1] == '\r') {
                acc_len--;
            }
            if (out_kind) {
                *out_kind = KIND_STR;
            }
            return (int64_t)(intptr_t)rt_strndup(acc, acc_len);
        }
        if (acc_len + avail < sizeof(acc)) {
            memcpy(acc + acc_len, slice, avail);
            acc_len += avail;
        }
        f->read_pos = f->read_len;
    }
    if (acc_len == 0) {
        if (out_kind) {
            *out_kind = KIND_NONE;
        }
        return 0;
    }
    if (acc[acc_len - 1] == '\r') {
        acc_len--;
    }
    if (out_kind) {
        *out_kind = KIND_STR;
    }
    return (int64_t)(intptr_t)rt_strndup(acc, acc_len);
}

int64_t hyper_rt_file_readlines(int64_t handle, int64_t _handle_kind, int64_t line,
                                int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    int64_t list = hyper_rt_list_new();
    for (;;) {
        int64_t kind = 0;
        int64_t payload = hyper_rt_file_readline(handle, _handle_kind, line, _line_kind, &kind);
        if (kind == KIND_NONE) {
            break;
        }
        hyper_rt_list_push(list, payload, KIND_STR);
    }
    return list;
}

int64_t hyper_rt_file_write(int64_t handle, int64_t _handle_kind, int64_t text, int64_t text_kind,
                            int64_t line, int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    RtFile *f = rt_file_from_handle(handle, line);
    rt_check_open(f, line);
    if (!f->writable) {
        rt_io_error(f, line, "write");
    }
    const char *bytes = text_kind == KIND_STR && text ? (const char *)(intptr_t)text : "";
    size_t len = strlen(bytes);
    rt_release_read_ahead(f);
    if (len >= BUFFER_CAPACITY) {
        rt_flush_writes(f);
        fwrite(bytes, 1, len, f->fp);
    } else {
        if (!f->write_buf) {
            f->write_buf = (unsigned char *)malloc(BUFFER_CAPACITY);
            f->write_cap = BUFFER_CAPACITY;
        }
        if (f->write_len + len > f->write_cap) {
            rt_flush_writes(f);
        }
        memcpy(f->write_buf + f->write_len, bytes, len);
        f->write_len += len;
    }
    return (int64_t)len;
}

int64_t hyper_rt_file_writelines(int64_t handle, int64_t _handle_kind, int64_t list,
                                 int64_t list_kind, int64_t line, int64_t _line_kind) {
    (void)_line_kind;
    if (list_kind != KIND_LIST || list == 0) {
        hyper_rt_runtime_error(line, "writelines expects a list");
    }
    RtList *items = (RtList *)(intptr_t)list;
    int64_t total = 0;
    for (size_t i = 0; i < items->len; i++) {
        total += hyper_rt_file_write(handle, _handle_kind, items->items[i].payload,
                                     items->items[i].kind, line, _line_kind);
    }
    return total;
}

int64_t hyper_rt_file_seek(int64_t handle, int64_t _handle_kind, int64_t offset, int64_t offset_kind,
                           int64_t whence, int64_t whence_kind, int64_t line, int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    RtFile *f = rt_file_from_handle(handle, line);
    rt_check_open(f, line);
    int64_t off = offset_kind == KIND_I64 ? offset : 0;
    int64_t wh = whence_kind == KIND_I64 ? whence : 0;
    rt_flush_writes(f);
    rt_release_read_ahead(f);
    int origin = SEEK_SET;
    if (wh == 1) {
        origin = SEEK_CUR;
    } else if (wh == 2) {
        origin = SEEK_END;
    }
    if (fseek(f->fp, (long)off, origin) != 0) {
        rt_io_error(f, line, "seek");
    }
    return (int64_t)ftell(f->fp);
}

int64_t hyper_rt_file_tell(int64_t handle, int64_t _handle_kind, int64_t line,
                           int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    RtFile *f = rt_file_from_handle(handle, line);
    rt_check_open(f, line);
    long pos = ftell(f->fp);
    return (int64_t)((pos >= 0 ? (uint64_t)pos : 0) + f->write_len - rt_unread(f));
}

int64_t hyper_rt_file_size(int64_t handle, int64_t _handle_kind, int64_t line,
                           int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    RtFile *f = rt_file_from_handle(handle, line);
    rt_check_open(f, line);
    rt_flush_writes(f);
    long cur = ftell(f->fp);
    fseek(f->fp, 0, SEEK_END);
    long end = ftell(f->fp);
    fseek(f->fp, cur, SEEK_SET);
    return end >= 0 ? (int64_t)end : 0;
}

void hyper_rt_file_flush(int64_t handle, int64_t _handle_kind, int64_t line, int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    RtFile *f = rt_file_from_handle(handle, line);
    rt_check_open(f, line);
    rt_flush_writes(f);
    fflush(f->fp);
}

int64_t hyper_rt_file_is_closed(int64_t handle, int64_t _handle_kind) {
    (void)_handle_kind;
    if (handle == 0) {
        return 1;
    }
    RtFile *f = (RtFile *)(intptr_t)handle;
    return f->closed ? 1 : 0;
}

int64_t hyper_rt_file_path(int64_t handle, int64_t _handle_kind, int64_t line,
                           int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    RtFile *f = rt_file_from_handle(handle, line);
    return (int64_t)(intptr_t)rt_strdup(f->path ? f->path : "");
}

int64_t hyper_rt_file_mode(int64_t handle, int64_t _handle_kind, int64_t line,
                           int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    RtFile *f = rt_file_from_handle(handle, line);
    return (int64_t)(intptr_t)rt_strdup(f->mode ? f->mode : "");
}
