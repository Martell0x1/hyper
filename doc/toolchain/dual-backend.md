# Compiler-only toolchain

Hyper is a **compiled language**. There is no interpreter: every program runs through Cranelift (JIT) or native codegen (`--emit-exe`).

## Commands

| Command | Backend | Use when |
|---------|---------|----------|
| `hyper run file.hyp` | Cranelift JIT | Everyday execution (same engine as `compile`). |
| `hyper compile file.hyp` | Cranelift JIT | Explicit JIT. |
| `hyper compile file.hyp --emit-ir` | Compiler | Debugging IR lowering. |
| `hyper compile file.hyp --emit-exe out` | AOT + C runtime | Standalone binary (needs a host C linker on Linux / macOS / Windows). |
| `hyper typecheck file.hyp` | Semantic analysis only | Checking types without running. |
| `hyper tokenize` / `hyper parse` | Frontend only | Debugging the lexer / parser. |

## Platforms

Linux, macOS, and native Windows are first-class. On Windows, WSL is optional — not required. JIT needs only Rust; `--emit-exe` needs a C toolchain (`clang` / MinGW / MSVC). See [Building from source](../building.md).

## Semantics

- Type errors are **fatal** before codegen (`run` and `compile`).
- Core language, I/O, JSON, strings, collections, `raise` / `handle`, traits, and `pub` / `mut` lower on the compile path. Remaining gaps: [Known limitations](../compiler/known-limitations.md).

## Direction

Grow the compiler (threaded `@parallel`, SIMD/GPU `@vectorize`, library interop). Do not reintroduce a tree-walk interpreter.
