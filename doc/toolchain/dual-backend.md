# Interpreter and compiler today

Hyper currently ships **two execution paths** in one binary. This is a **transition layout**, not the permanent design.

## Commands

| Command | Backend | Use when |
|---------|---------|----------|
| `hyper run file.hyp` | Tree-walk interpreter | Exploring language features, file I/O, JSON, `input()`, or when `compile` reports an unsupported construct. |
| `hyper compile file.hyp` | Cranelift JIT | Fast native execution for supported programs. |
| `hyper compile file.hyp --emit-ir` | Compiler | Debugging IR lowering. |
| `hyper compile file.hyp --emit-exe out` | AOT + C runtime | Standalone binary (needs a C linker). |
| `hyper typecheck file.hyp` | Semantic analysis only | Checking types without running. |

## Differences that matter

- **`run`** treats type errors as **warnings** and continues.
- **`compile`** treats type errors as **failures** and stops before codegen.
- Some builtins (`open`, `input`, `json` in modules) are **interpreter-only until lowered** — the compiler emits a clear `SyntaxError` at compile time rather than failing silently.

## Direction

Hyper moves **toward compile-only** in measured steps:

1. First release: compiler covers the **core language**; interpreter fills gaps.
2. Each release: move another subsystem (files, JSON, parallel loops) onto the compiler.
3. End state: `run` removed or aliased to JIT compile of a scratch module.

Do not build long-term workflows that depend on interpreter-only behavior unless you accept migration work later.
