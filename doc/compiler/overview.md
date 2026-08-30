# Compiler overview

The Hyper compiler lowers parsed, type-checked AST into **Hyper IR**, then Cranelift generates machine code. The same pipeline powers JIT execution and `--emit-obj` / `--emit-exe`.

## Pipeline

```text
source → scanner → parser → typecheck → lower (compiler.rs) → IR (ir.rs) → codegen (codegen.rs) → JIT / object / executable
```

Module imports are resolved at lower time: `.hyp` files under the entry directory are parsed once and their top-level bindings become mangled IR symbols.

## CLI

```bash
# JIT (default)
hyper compile program.hyp

# Inspect IR
hyper compile program.hyp --emit-ir

# Object file
hyper compile program.hyp --emit-obj out.o

# Linked executable (requires cc/clang/gcc)
hyper compile program.hyp --emit-exe my_app
```

## Struct-aware lowering

The compiler tracks which struct type each local holds (constructors, field access, annotated parameters, return types). Method calls like `p.move(1)` lower to direct calls on mangled IR functions instead of dynamic dispatch.

When inference is ambiguous, add a type annotation:

```hyper
fn shift(p: Point):
    p.move(1)
```

See [Supported features](supported-features.md) and [Known limitations](known-limitations.md).
