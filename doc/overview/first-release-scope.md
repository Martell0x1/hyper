# First release scope (v0.1)

Hyper v0.1 is the first public snapshot where developers can **clone, build, compile real programs, and evaluate Hyper** as a **Python-compatible, compile-first language for performance and AI workloads**. It is not a claim of full Python/stdlib/GPU parity — see [Why Hyper](why-hyper.md) for the long-term vision and [Known limitations](../compiler/known-limitations.md) for today’s gaps.

Release packaging (CHANGELOG, git tag, GitHub Release) happens **after** the criteria below are met — not before.

## Goals

- Ship a **compiler-first story** aligned with native speed and AI/data pipelines.
- Let users run **`compile`** on tutorial-scale code (core language + I/O + JSON + builtins).
- Document honestly what works today vs what is roadmap (NumPy interop, GPU codegen, full Python parity).
- Keep syntax stable — no breaking changes without a version bump after v0.1.

## v0.1 ready checklist

### Toolchain

| Criterion | Status |
|-----------|--------|
| `cargo build` / `cargo test` pass on CI | Required |
| `compile` JIT matches `run` on `ci/smoke.hyp` | Required |
| `--emit-exe` smoke passes | Required |
| Division-by-zero parity (`ci/divzero.hyp`) | Required |
| I/O, JSON, mmap, `input`, `clock` compile smokes (`ci/*_compile.hyp`) | Required |

### Compiler coverage (core language)

| Feature | Required for v0.1 |
|---------|-------------------|
| Functions, `let`, control flow | Yes |
| Structs, methods, fields | Yes |
| Modules (`import` / `from … import`) | Yes |
| Typed bindings (`Array[]`, `Dict[]`) | Yes |
| Arithmetic including `//`, `%`, `**` | Yes |
| Strings, lists, dicts, equality | Yes |
| F-strings, `print` | Yes |
| `open`, `with`, file methods, `open_mmap`, `import json`, `input()`, `clock()` | Yes (compile path) |

### Compiler gaps acceptable in v0.1 (must be documented)

| Feature | v0.1 expectation |
|---------|------------------|
| `@parallel` / `@vectorize` | Sequential codegen OK; real multithreading/GPU post v0.1 |
| Generics / full trait system | Documented as not implemented |
| `break` / `continue`, collection methods | Post v0.1 or contributor issues |
| NumPy / CPython extension interop | Vision in [Why Hyper](why-hyper.md); not required for v0.1 tag |

### Documentation

| Criterion | Required |
|-----------|----------|
| mdBook builds (`mdbook build`) | Yes |
| Official vision ([Why Hyper](why-hyper.md)) | Yes |
| Compiler supported / limitations pages | Yes |
| `doc/examples/` — code samples only | Yes |
| Standard library docs under `doc/standard-library/` | Yes |

### Explicitly out of v0.1

- Published installers / package managers
- **Full** Python language and stdlib parity
- Guaranteed NumPy wheel compatibility without integration work
- Production GPU kernel codegen
- Full trait / generic system
- Interpreter removal (post v0.1 roadmap)

## When we announce “v0.1 ready”

You will get a direct message when **all Required rows above are green** and the remaining gaps are listed in [Known limitations](../compiler/known-limitations.md) with no surprises.
