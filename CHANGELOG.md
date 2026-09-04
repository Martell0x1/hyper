# Changelog

All notable changes to Hyper are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Hyper 0.3.0

**Breaking:** Hyper is now **compiler-only**. The tree-walk interpreter is removed.

### Removed

- `src/interpreter.rs`, `src/text_utils.rs`, `src/collection_utils.rs`
- `hyper evaluate`
- `hyper run --interpret` / `HYPER_BACKEND=interpret`

### Changed

- `hyper run` always uses Cranelift JIT (same engine as `hyper compile`).
- Type errors are fatal on `run` (same as `compile`).
- Docs, CI, and issue templates describe a single compiled execution model.
- `environment.rs` keeps `HyperValue` only as the JSON bridge host type — not an execution backend.

### Notes

- String methods live on the compile runtime: `compiler/runtime/str.rs` (JIT) and `hyper_rt_str.c` (AOT).
- `@parallel` has no interpreter thread pool anymore; compile path remains sequential until threaded codegen lands.
- Migration: replace `hyper run --interpret` with `hyper run` / `hyper compile`. Prefer `print(...)` programs instead of `evaluate`.

See release [0.3.0](https://github.com/muhammadyusufpov/hyper/releases/tag/v0.3.0)

## Hyper 0.2.0

Memory-safety and performance release. Dual backend remains: `hyper run` (interpreter) and `hyper compile` (JIT / `--emit-exe`).

### Fixed

- File handles release the OS fd on `close` (Rust and C runtimes).
- Overwriting list / dict / struct slots frees the previous runtime value (leak fixes).
- Shared list / dict / array payloads via `Rc` on the interpreter path (`ref` / nested assign).

### Improved

- Interpreter: reuse `for`-loop environments; cheaper `print` / string concat; shared Instance metadata via `Rc`.
- Compiler: skip monomorphic kind SSA on hot locals; string-key dict get without malloc; tighter f-string lowering; faster JIT formatting paths.

### Added

- `SECURITY.md` and supported-versions notes.
- CI smoke for shared list `ref` behavior where applicable.

### Notes

- Interpreter is still present in this release (including real `@parallel` threads on `run`).
- Compile-path `@parallel` remains sequential.

See release [0.2.0](https://github.com/muhammadyusufpov/hyper/releases/tag/v0.2.0)

## Hyper 0.1.0

First public release of Hyper: a Python-shaped language with a Cranelift compiler (`compile`) and a transitional interpreter (`run`).

### Added

- **Language core:** `let` / `let mut`, functions (`fn` / `def`), `if` / `elif` / `else`, `while`, `for` / `for-in`, ternary expressions, f-strings, lists, dicts, typed bindings (`Array[T]`, `Dict[K, V]`).
- **Structs and modules:** fields, methods, `__init__`, `import` / `from … import`.
- **Visibility and mutability:** `pub` / `mut` enforced on struct fields and methods (both backends).
- **Traits:** method name + arity conformance checks (no generics yet).
- **`ref` parameters:** mutable bindings; struct instances share field storage.
- **Loop control:** `break` / `continue` on `run` and `compile` (rejected inside `@parallel` bodies).
- **Explicit error flow:** `raise`, `raises` on functions, `handle … else …` (no `try` / `except`).
- **Builtins / stdlib on the compile path:** `print`, `open` / `with`, file methods, `open_mmap`, `import json`, `input()`, `clock()`, collection methods (`len`, `append`, `keys`), full string methods.
- **Decorators:** `@parallel` / `@vectorize` on `for` (interpreter: real threads for `@parallel`; compiler: sequential loops with the same per-index results).
- **Toolchain:** `hyper run`, `hyper compile` (JIT), `--emit-exe` / `--emit-obj` / `--emit-ir`, `hyper typecheck`.
- **CI smokes:** core language, I/O, JSON, mmap, strings, break/continue, raise/handle, traits, pub/mut, ref, vectorize.
- **Documentation:** mdBook under `doc/`, contributing guide, issue templates.

### Known limitations

See [Compiler known limitations](doc/compiler/known-limitations.md). Notable gaps for this release:

- Generics / full trait system
- Shared `ref` for list / dict / array payloads
- Compile-path threaded `@parallel` and SIMD `@vectorize`
- Full Python / stdlib / NumPy parity
- Production GPU codegen

### Notes

- Versioning starts at **0.1.0** (pre-1.0 SemVer): the language surface may still evolve before 1.0.
- Build from source with Rust; see [Building from source](doc/building.md).

See release [0.1.0](https://github.com/muhammadyusufpov/hyper/releases/tag/v0.1.0)
