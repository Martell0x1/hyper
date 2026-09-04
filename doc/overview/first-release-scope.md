# First release scope (v0.1)

Hyper v0.1 is the first public snapshot where developers can **clone, build, compile real programs, and evaluate Hyper** as a **Python-compatible, compile-first language for performance and AI workloads**. It is not a claim of full Python/stdlib/GPU parity — see [Why Hyper](why-hyper.md) for the long-term vision and [Known limitations](../compiler/known-limitations.md) for today’s gaps.

Release packaging (CHANGELOG, git tag `v0.1.0`, GitHub Release) is part of shipping this snapshot.

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
| I/O, JSON, mmap, `input`, `clock`, collection-method compile smokes (`ci/*_compile.hyp`) | Required |

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
| Collection methods (`len`, `append`, `keys`) | Yes (compile path) |
| Common / full string methods | Yes (compile path; Python-compatible) |
| `break` / `continue` | Yes (compile path; rejected in `@parallel` loop bodies) |
| `pub` / `mut` on struct members | Yes (enforced on both backends) |
| Traits (method name + arity) | Yes (no generics) |
| `raise` / `raises` / `handle` | Yes (compile path; no `try` / `except`) |
| `ref` (mutable binding; shared structs) | Yes |

### Compiler gaps acceptable in v0.1 (must be documented)

| Feature | v0.1 expectation |
|---------|------------------|
| `@parallel` / `@vectorize` | Sequential compile codegen OK; interpreter has real `@parallel` threads; GPU/SIMD post v0.1 |
| Generics / full trait system | Documented as not implemented |
| Shared `ref` for list/dict/array | Shared on `run` (Rc); compile path uses handles |
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
- Full trait / generic system (basic trait method checks ship in v0.1)
- Interpreter removal (post v0.1 roadmap)
- `try` / `except` (Hyper uses `raise` / `handle` instead)

## When we announce “v0.1 ready”

**v0.1.0** is the first public tag. Remaining gaps stay listed in [Known limitations](../compiler/known-limitations.md) with no surprises.
