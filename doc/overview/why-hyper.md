# Why Hyper

Hyper is a compiled language that keeps **Python-like readability** while targeting **native performance** through a Cranelift-based compiler. The long-term direction is a **single compiled toolchain** — not a permanent split between “run with the interpreter” and “compile for speed.”

## What Hyper offers today

| Strength | What it means in practice |
|----------|---------------------------|
| **Readable syntax** | Indentation-based blocks, familiar operators, structs, modules, and typed bindings where you want them. |
| **Dual backend (transition)** | `run` for exploration; `compile` for JIT, object files, and native executables. The compiler path is the focus of the first release. |
| **Static struct flow** | The compiler tracks struct types through constructors, fields, returns, and annotations so method calls lower to direct code. |
| **Clear errors** | `SyntaxError`, `IndentationError`, and `RuntimeError` with line numbers — no scattered debug prints in user programs. |
| **Buffered I/O** | File handles use read/write buffers (interpreter today; compiler support is in progress for v0.1). |

## Why pick Hyper over …

**Python** — Hyper is for teams that outgrow interpreter overhead but do not want to rewrite everything in C++ or Rust. Syntax stays approachable; execution moves toward native code.

**Rust / C++** — Hyper trades maximum low-level control for faster iteration: less ceremony for scripts, small tools, and learning projects.

**Zig** — Hyper prioritizes approachability and a Python-shaped surface first; systems-level manual memory control is not the primary story.

Hyper’s first release is intentionally **modest**: enough to write small programs, compile them, and feel the direction. It is not claiming production parity with mature languages on day one — the same path every new language takes.

## Where Hyper is headed

1. **v0.1** — Compiler-backed first release: core language compiles reliably; interpreter remains for gaps during transition.
2. **Post v0.1** — Close compiler gaps (`with`, builtins, parallel codegen), tighten semantics (`pub`, `mut`, `ref`).
3. **Long term** — Interpreter retired; Hyper is compile-by-default.

See [First release scope](first-release-scope.md) for the concrete v0.1 checklist.
