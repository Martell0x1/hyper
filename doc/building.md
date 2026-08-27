# Building from source

Hyper has **no official release yet**. To try it today, clone the repository and build the reference toolchain with Rust.

Everything lives in a **single Cargo package** (`Interpreters`). The `src/` tree holds the language frontend, interpreter, and compiler:

| Module / path | Role |
|---------------|------|
| `scanner.rs`, `parser.rs`, `ast.rs`, `driver.rs` | Lexer, parser, AST, program driver |
| `semantic.rs` | Type checker |
| `interpreter.rs`, `environment.rs`, `text_utils.rs` | Tree-walk interpreter (`run`) |
| `ir.rs`, `compiler.rs`, `codegen.rs`, `runtime.rs` | IR, lowering, Cranelift codegen (`compile`) |
| `src/runtime/hyper_rt.c` | C runtime linked for `--emit-exe` |
| `main.rs` | CLI (`tokenize`, `parse`, `run`, `typecheck`, `compile`, …) |

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable) — `cargo` + `rustc`
- Git
- **Optional (compile to executable):** a C compiler (`cc`, `clang`, or `gcc`) for linking `--emit-exe`

On Windows, [WSL](https://learn.microsoft.com/en-us/windows/wsl/) is the smoothest path for building and running; native Windows works for `cargo build`, but AOT linking may need MSVC or MinGW.

## Clone and build

```bash
git clone https://github.com/muhammadyusufpov/hyper.git
cd hyper
cargo build
```

Debug binary:

```text
target/debug/Interpreters
```

Release binary:

```bash
cargo build --release
# target/release/Interpreters
```

## Run a program

**Interpreter (default):**

```bash
cargo run -- run your_file.hyp
```

**Compiler (JIT):**

```bash
cargo run -- compile your_file.hyp
```

**Compiler (dump IR / emit artifacts):**

```bash
cargo run -- compile your_file.hyp --emit-ir
cargo run -- compile your_file.hyp --emit-obj out.o
cargo run -- compile your_file.hyp --emit-exe my_app
```

## Quick sanity check

If the repo includes `test.hyp`:

```bash
cargo run -- run test.hyp
cargo run -- compile test.hyp
```

Both should finish without syntax errors.

## Docs site (optional)

Documentation uses [MkDocs Material](https://squidfunk.github.io/mkdocs-material/):

```bash
pip install mkdocs-material
mkdocs serve
```

Open `http://127.0.0.1:8000`.

## What is not supported yet

Hyper is under active development. The compiler path does **not** cover the full language yet (e.g. structs/traits, real `@parallel` codegen). Use `run` when a feature fails under `compile`.

There are no published packages or installers — building from source is the only supported way to get the toolchain today.
