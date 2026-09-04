# Changelog

All notable changes to Hyper are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-09-04

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

[0.1.0]: https://github.com/muhammadyusufpov/hyper/releases/tag/v0.1.0
