# Hyper

**Hyper** is a programming language with **Python-like readability** and a **Cranelift-based compiler** aimed at native performance without losing approachability.

Hyper is moving from a dual interpreter/compiler toolchain toward **compile-by-default**. The first public release (v0.1) focuses on letting you **build small programs with `compile`**, understand the language direction, and try it on real (if modest) projects.

## Quick start

```bash
cargo run -- compile your_file.hyp
```

Interpreter path (while compiler gaps remain):

```bash
cargo run -- run your_file.hyp
```

See [Building from source](building.md) for prerequisites and compiler flags.

---

## Documentation layout

Everything under `doc/` is prose documentation (this book). **Executable Hyper samples** live only in `doc/examples/` — `.hyp` files with short comments, not long guides.

### Root files (`doc/`)

| File | Purpose |
|------|---------|
| [readme.md](readme.md) | This page — start here |
| [SUMMARY.md](SUMMARY.md) | mdBook table of contents (sidebar navigation) |
| [building.md](building.md) | Clone, build, run, compile flags, mdBook |
| [COMMIT_CONVENTION.md](COMMIT_CONVENTION.md) | Git commit prefix rules for contributors |

### `doc/overview/` — project direction

| File | Find it when you need… |
|------|-------------------------|
| [why-hyper.md](overview/why-hyper.md) | Why Hyper exists, comparison with Python / Rust / C++ / Zig |
| [first-release-scope.md](overview/first-release-scope.md) | v0.1 checklist — what “ready for first release” means |

### `doc/toolchain/` — how you run Hyper today

| File | Find it when you need… |
|------|-------------------------|
| [dual-backend.md](toolchain/dual-backend.md) | `run` vs `compile`, when to use each, long-term compile-only direction |

### `doc/compiler/` — the compile path

| File | Find it when you need… |
|------|-------------------------|
| [overview.md](compiler/overview.md) | Pipeline (AST → IR → Cranelift), CLI flags |
| [supported-features.md](compiler/supported-features.md) | What `hyper compile` lowers today |
| [known-limitations.md](compiler/known-limitations.md) | What still requires `run` or is not implemented |

### `doc/standard-library/` — builtins and I/O

| File | Find it when you need… |
|------|-------------------------|
| [file-handling.md](standard-library/file-handling.md) | `open`, `with`, file methods, `open_mmap` |
| [json-module.md](standard-library/json-module.md) | `import json`, `loads` / `dumps` / `load` / `dump` |

### `doc/errors/` — error **reference** (prose)

| File | Find it when you need… |
|------|-------------------------|
| [overview.md](errors/overview.md) | What `SyntaxError`, `IndentationError`, and `RuntimeError` mean; exit codes; compile vs run |

This folder is **not** duplicate sample code. For **runnable `.hyp` files** that trigger each error, see `doc/examples/errors/` below.

### `doc/examples/` — Hyper **code samples only**

No markdown guides here — only `.hyp` (and occasional `.hyo` sketches) organized by topic:

| Folder | What’s inside |
|--------|----------------|
| `examples/variable/` | `let`, `let mut` |
| `examples/function/` | Functions, types, `ref` syntax |
| `examples/operator/` | Arithmetic, comparison, assignment |
| `examples/conditional/` | `if` / `elif` / `else`, ternary |
| `examples/loop/` | `for`, `while`; `advanced/` for `@parallel` / `@vectorize` |
| `examples/collection/` | Lists, arrays, dicts |
| `examples/data_type/` | Numbers, strings, booleans, `None` |
| `examples/struct/` | Structs, inheritance, traits (some aspirational) |
| `examples/module/` | `import math.hyp` — module + import samples |
| `examples/file_handling/` | `open`, JSON I/O samples |
| `examples/io/` | `print`, `input` |
| `examples/errors/` | **Code** that triggers Syntax / Indentation / Runtime errors when you `run` it |

**`doc/errors/` vs `doc/examples/errors/`**

- `doc/errors/` → read **what** the error kinds are (documentation).
- `doc/examples/errors/` → **run** small programs to **see** those errors in the terminal.

---

## Project status

Hyper has **not** tagged v0.1 yet. Release notes, git tags, and GitHub Releases come **after** the [first release checklist](overview/first-release-scope.md) is complete.
