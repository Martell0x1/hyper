# First release scope (v0.1)

Hyper v0.1 is **not** a “100% finished compiler” release. It is the first public snapshot where developers can **clone, build, compile small programs, and judge whether Hyper fits their projects**.

Release packaging (CHANGELOG, git tag, GitHub Release) happens **after** the criteria below are met — not before.

## Goals

- Ship a **compiler-first story** while the interpreter still covers gaps.
- Let users run **`compile`** on real tutorial-scale code, not only `ci/smoke.hyp`.
- Document honestly what works and what does not.
- Keep syntax stable — no breaking changes without a version bump after v0.1.

## v0.1 ready checklist

### Toolchain

| Criterion | Status |
|-----------|--------|
| `cargo build` / `cargo test` pass on CI | Required |
| `compile` JIT matches `run` on `ci/smoke.hyp` | Required |
| `--emit-exe` smoke passes | Required |
| Division-by-zero parity (`ci/divzero.hyp`) | Required |

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

### Compiler gaps acceptable in v0.1 (must be documented)

| Feature | v0.1 expectation |
|---------|------------------|
| `with open(...)` / file methods | Target: compile; until then documented + interpreter fallback |
| `import json`, `input()` | Target: compile or clear error |
| `@parallel` / `@vectorize` | Sequential codegen OK; real parallelism post v0.1 |
| Generics / traits | Documented as not implemented |
| `open_mmap` | Interpreter only for v0.1 |

### Documentation

| Criterion | Required |
|-----------|----------|
| mdBook builds (`mdbook build`) | Yes |
| Compiler supported / limitations pages | Yes |
| `doc/examples/` — code samples only | Yes |
| Standard library docs under `doc/standard-library/` | Yes |

### Explicitly out of v0.1

- Published installers / package managers
- Perfect Python parity
- Full trait / generic system
- Interpreter removal (post v0.1 roadmap)

## When we announce “v0.1 ready”

You will get a direct message when **all Required rows above are green** and the remaining gaps are listed in [Known limitations](../compiler/known-limitations.md) with no surprises.

Estimated remaining compiler work for v0.1 file I/O: **1–2 focused weeks** after core checklist (depends on `with` + runtime lowering scope).
