# Hyper

**Hyper** is a programming language that aims for **Python-like readability**, **C/C++-class performance**, and **Rust-inspired safety**.

The repository ships a **dual toolchain**: a tree-walk **interpreter** (`run`) and an experimental **compiler** path (`compile`) built on Cranelift. Hyper has **not** had an official release yet — see [Building from source](building.md).

## Quick start

```bash
cargo run -- run your_file.hyp
```

Compile with JIT:

```bash
cargo run -- compile your_file.hyp
```

See [Building from source](building.md) for prerequisites, repo layout, and compiler flags.

## Example snippets

Browse `.hyp` files under `doc/examples/` (syntax sketches and runnable samples).
