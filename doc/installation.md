# Installation

Hyper’s reference implementation is written in **Rust**. You need a recent Rust toolchain (`cargo` + `rustc`).

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable)
- Git
- On Windows, WSL works well if you develop from Linux-style shells

## Clone & build

```bash
git clone https://github.com/Yusupov-Muhammadyusuf/hyperlang.git
cd hyperlang
cargo build
```

Debug binary path:

```text
target/debug/Interpreters
```

## Verify

```bash
cargo run -- run test.hyp
```

If `test.hyp` prints without a syntax error, your toolchain is ready.

## Docs site (optional)

Documentation is built with [MkDocs Material](https://squidfunk.github.io/mkdocs-material/).

```bash
pip install mkdocs-material
mkdocs serve
```

Then open `http://127.0.0.1:8000`.
