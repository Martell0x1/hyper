# Compiler supported features

This list reflects what **`hyper compile`** can lower today (JIT and `--emit-exe`). For syntax samples see `doc/examples/`.

## Language constructs

- Variables: `let`, `let mut`, typed bindings (`name: Type = …`, `Array[T]`, `Dict[K, V]`)
- Functions: `fn` / `def`, parameters including `ref` (passed by value today)
- Control flow: `if` / `elif` / `else`, `while`, `for` / `for-in`, ternary `a if cond else b`
- Operators: arithmetic (`+`, `-`, `*`, `/`, `//`, `%`, `**`), comparisons, `and` / `or`, compound assignment
- Literals: integers, floats, strings, f-strings, lists, dicts, `None`, booleans
- Structs: fields, methods, `__init__`, field get/set
- Modules: `import m`, `import m as alias`, `from m import name`
- Decorators: `@parallel`, `@vectorize` on `for` (emitted as sequential loops today)

## Builtins and standard library (compile path)

| Feature | Notes |
|---------|--------|
| `print(...)` | Variadic |
| `open(path, mode?)` | Buffered file handle |
| `with open(...) as f:` | Auto-close; file methods |
| File methods | `read`, `readline`, `readlines`, `write`, `seek`, `tell`, `size`, `flush`, `close`, … |
| `with open_mmap(path) as m:` | `read_chunk(offset, size)` |
| `input(prompt?)` | Stdin line read |
| `clock()` | Seconds since UNIX epoch (`f64`) |
| Collection methods | list/array `len()`, `append(x)`; dict `len()`, `keys()`; string `len()` |
| String methods | `upper`, `lower`, `strip`, `lstrip`, `rstrip`, `startswith`, `endswith`, `split`, `replace` |
| `import json` | `loads`, `dumps`, `load`, `dump` |

Integer `/`, `%`, `//` guard division by zero at runtime.

## Codegen modes

- JIT via Cranelift (`hyper compile`)
- Object emission (`--emit-obj`)
- Executable linking with C runtime (`--emit-exe`)

## CI-verified programs

| Program | What it checks |
|---------|----------------|
| `ci/smoke.hyp` | Core language; run / JIT / `--emit-exe` output parity |
| `ci/divzero.hyp` | `RuntimeError` exit code 70 |
| `ci/io_compile.hyp` | File I/O on compile path |
| `ci/json_compile.hyp` | JSON module on compile path |
| `ci/mmap_compile.hyp` | Memory-mapped files on compile path |
| `ci/input_compile.hyp` | `input()` on compile path |
| `ci/clock_compile.hyp` | `clock()` on compile path |
| `ci/collections_compile.hyp` | list/array/dict `len`, `append`, `keys` on compile path |
| `ci/strings_compile.hyp` | string methods on compile path (JIT and `--emit-exe`) |

## Not compiled (see limitations)

Generics, full trait enforcement, `break` / `continue`, real `@parallel` thread/GPU codegen, Python library interop — [Known limitations](known-limitations.md).
