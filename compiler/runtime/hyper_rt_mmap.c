#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#else
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#endif

enum { KIND_STR = 2, KIND_I64 = 0 };

typedef struct {
    unsigned char *data;
    size_t len;
    int mapped;
#ifdef _WIN32
    void *file_handle;
    void *map_handle;
#endif
} RtMmap;

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

static char *str_arg(int64_t payload, int64_t kind, int64_t line, const char *ctx) {
    if (kind != KIND_STR) {
        runtime_error(line, ctx);
    }
    if (payload == 0) {
        return rt_strdup("");
    }
    return rt_strdup((const char *)(intptr_t)payload);
}

static RtMmap *mmap_from_handle(int64_t handle, int64_t line) {
    if (handle == 0) {
        runtime_error(line, "invalid mapped file handle");
    }
    return (RtMmap *)(intptr_t)handle;
}

#ifdef _WIN32
int64_t hyper_rt_mmap_open(int64_t path, int64_t path_kind, int64_t line, int64_t _line_kind) {
    (void)_line_kind;
    char *path_str = str_arg(path, path_kind, line, "open_mmap expects a file path");
    HANDLE file = CreateFileA(
        path_str,
        GENERIC_READ,
        FILE_SHARE_READ,
        NULL,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        NULL
    );
    if (file == INVALID_HANDLE_VALUE) {
        char buf[512];
        snprintf(buf, sizeof(buf), "could not map file '%s'", path_str);
        free(path_str);
        runtime_error(line, buf);
    }
    LARGE_INTEGER size;
    if (!GetFileSizeEx(file, &size) || size.QuadPart < 0) {
        CloseHandle(file);
        free(path_str);
        runtime_error(line, "could not stat mapped file");
    }
    RtMmap *m = (RtMmap *)calloc(1, sizeof(RtMmap));
    if (!m) {
        CloseHandle(file);
        free(path_str);
        runtime_error(line, "out of memory");
    }
    m->file_handle = file;
    if (size.QuadPart == 0) {
        free(path_str);
        return (int64_t)(intptr_t)m;
    }
    HANDLE mapping = CreateFileMappingA(file, NULL, PAGE_READONLY, 0, 0, NULL);
    if (!mapping) {
        CloseHandle(file);
        free(m);
        free(path_str);
        runtime_error(line, "mmap failed");
    }
    void *view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
    if (!view) {
        CloseHandle(mapping);
        CloseHandle(file);
        free(m);
        free(path_str);
        runtime_error(line, "mmap failed");
    }
    m->len = (size_t)size.QuadPart;
    m->data = (unsigned char *)view;
    m->map_handle = mapping;
    m->mapped = 1;
    free(path_str);
    return (int64_t)(intptr_t)m;
}

void hyper_rt_mmap_close(int64_t handle, int64_t _handle_kind, int64_t line, int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    (void)line;
    if (handle == 0) {
        return;
    }
    RtMmap *m = (RtMmap *)(intptr_t)handle;
    if (m->mapped && m->data) {
        UnmapViewOfFile(m->data);
    }
    if (m->map_handle) {
        CloseHandle((HANDLE)m->map_handle);
    }
    if (m->file_handle) {
        CloseHandle((HANDLE)m->file_handle);
    }
    free(m);
}
#else
int64_t hyper_rt_mmap_open(int64_t path, int64_t path_kind, int64_t line, int64_t _line_kind) {
    (void)_line_kind;
    char *path_str = str_arg(path, path_kind, line, "open_mmap expects a file path");
    int fd = open(path_str, O_RDONLY);
    if (fd < 0) {
        char buf[512];
        snprintf(buf, sizeof(buf), "could not map file '%s': %s", path_str, strerror(errno));
        free(path_str);
        runtime_error(line, buf);
    }
    struct stat st;
    if (fstat(fd, &st) != 0) {
        close(fd);
        free(path_str);
        runtime_error(line, "could not stat mapped file");
    }
    RtMmap *m = (RtMmap *)calloc(1, sizeof(RtMmap));
    if (!m) {
        close(fd);
        free(path_str);
        runtime_error(line, "out of memory");
    }
    if (st.st_size == 0) {
        close(fd);
        free(path_str);
        return (int64_t)(intptr_t)m;
    }
    m->len = (size_t)st.st_size;
    m->data = (unsigned char *)mmap(NULL, m->len, PROT_READ, MAP_PRIVATE, fd, 0);
    close(fd);
    free(path_str);
    if (m->data == MAP_FAILED) {
        free(m);
        runtime_error(line, "mmap failed");
    }
    m->mapped = 1;
    return (int64_t)(intptr_t)m;
}

void hyper_rt_mmap_close(int64_t handle, int64_t _handle_kind, int64_t line, int64_t _line_kind) {
    (void)_handle_kind;
    (void)_line_kind;
    (void)line;
    if (handle == 0) {
        return;
    }
    RtMmap *m = (RtMmap *)(intptr_t)handle;
    if (m->mapped && m->data) {
        munmap(m->data, m->len);
    }
    free(m);
}
#endif

int64_t hyper_rt_mmap_read_chunk(
    int64_t handle,
    int64_t _handle_kind,
    int64_t offset,
    int64_t offset_kind,
    int64_t size,
    int64_t size_kind,
    int64_t line,
    int64_t _line_kind
) {
    (void)_handle_kind;
    (void)_line_kind;
    RtMmap *m = mmap_from_handle(handle, line);
    size_t off = (offset_kind == KIND_I64 && offset > 0) ? (size_t)offset : 0;
    size_t n = (size_kind == KIND_I64 && size > 0) ? (size_t)size : 0;
    if (off >= m->len) {
        return (int64_t)(intptr_t)rt_strdup("");
    }
    size_t end = off + n;
    if (end > m->len || end < off) {
        end = m->len;
    }
    size_t chunk_len = end - off;
    char *out = (char *)malloc(chunk_len + 1);
    if (!out) {
        runtime_error(line, "out of memory");
    }
    if (m->data && chunk_len > 0) {
        memcpy(out, m->data + off, chunk_len);
    }
    out[chunk_len] = '\0';
    return (int64_t)(intptr_t)out;
}
