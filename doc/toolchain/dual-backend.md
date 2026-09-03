# Interpreter and compiler today

Hyper ships **two execution paths** in one binary while the toolchain moves toward **compile-by-default** native execution for AI and data workloads. This layout is **transitional**, not the permanent design.

## Commands

| Command | Backend | Use when |
|---------|---------|----------|
| `hyper run file.hyp` | Tree-walk interpreter | Prototyping, or when `compile` reports an unsupported construct. |
| `hyper compile file.hyp` | Cranelift JIT | **Preferred** — native-speed execution for supported programs. |
| `hyper compile file.hyp --emit-ir` | Compiler | Debugging IR lowering. |
| `hyper compile file.hyp --emit-exe out` | AOT + C runtime | Standalone binary (needs a C linker). |
| `hyper typecheck file.hyp` | Semantic analysis only | Checking types without running. |

## Differences that matter

- **`run`** treats type errors as **warnings** and continues.
- **`compile`** treats type errors as **failures** and stops before codegen.
- Core I/O builtins — `open`, `with`, file methods, `open_mmap`, `import json`, `input()`, `clock()`, collection methods, and **full string methods** — are **supported on the compile path** (JIT and `--emit-exe`). Remaining gaps are listed in [Known limitations](../compiler/known-limitations.md).

## Direction

Hyper moves **toward compile-only** in measured steps:

1. **v0.1** — Compiler covers core language plus standard I/O and JSON; interpreter fills rare gaps.
2. **Post v0.1** — Real `@parallel` / GPU codegen, Python library interop.
3. **End state** — `run` retired; Hyper is the **fast, Python-compatible, AI-oriented** compiled runtime.

Do not build long-term workflows that depend on interpreter-only behavior unless you accept migration work later.
