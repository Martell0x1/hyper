# File handling

Hyper's file syntax follows Python. Underneath, handles are **buffered streams**: one OS file descriptor plus a 64 KB read-ahead buffer and a 64 KB write-behind buffer, so tight read/write loops amortize syscalls.

**Compiler status:** file I/O runs on the interpreter (`run`) today. The compiler rejects `with` blocks with an explicit error until lowering lands in v0.1. See [Compiler known limitations](../compiler/known-limitations.md).

## Opening a file

```hyper
with open("notes.txt", "w") as f:
    f.write("first line\n")
```

`open(path)` defaults to mode `"r"`. The block form closes the file (flushing buffered writes) when the block ends, even on early return from a function.

Standalone handle:

```hyper
let f = open("notes.txt", "r")
print(f.readline())
f.close()
```

### Modes

| Mode | Read | Write | Notes |
|------|------|-------|-------|
| `"r"` | yes | no | Default. File must exist. |
| `"w"` | no | yes | Create or truncate. |
| `"a"` | no | yes | Create or append. |
| `"r+"` | yes | yes | Read/write; file must exist. |
| `"w+"` | yes | yes | Read/write; truncates first. |
| `"a+"` | yes | yes | Read anywhere; writes append. |
| `"x"` / `"x+"` | | yes | Fail if file exists. |

A `b` or `t` suffix (`"rb"`, `"w+b"`) is accepted for familiarity; Hyper returns **text**, decoding invalid UTF-8 as replacement characters.

## Methods

| Method | Result |
|--------|--------|
| `read()` | Remaining file as one string |
| `read(n)` | At most `n` bytes |
| `readline()` | Next line without newline, or empty at EOF |
| `readlines()` | List of remaining lines, newlines stripped |
| `write(text)` | Bytes written |
| `writelines(list)` | Bytes written; no extra newlines |
| `seek(offset)` / `seek(offset, whence)` | New position; whence 0=start, 1=current, 2=end |
| `tell()` | Current offset |
| `size()` | File size in bytes |
| `flush()` | Push buffered writes to OS |
| `close()` | Flush and close |
| `closed()` | Whether handle is closed |
| `path()` / `mode()` | Open path and mode |

Unlike Python, `readline` and `readlines` strip trailing newlines.

Code samples: `doc/examples/file_handling/standard.hyp`.

## Memory-mapped files

For very large files, `open_mmap` maps the file and `read_chunk` copies a slice:

```hyper
with open_mmap("huge_model.bin") as mapped_file:
    let chunk = mapped_file.read_chunk(0, 1024)
```

Offsets past EOF return an empty string. Compiler support is not planned for v0.1.

Sample: `doc/examples/file_handling/mmap.hyo`.

## Related

- [JSON module](json-module.md) — reading and writing JSON through files
- [Building from source](../building.md) — run vs compile
