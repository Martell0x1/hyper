# Compiler supported features

This list reflects what **`hyper compile`** can lower today. For syntax samples see `doc/examples/` — that folder is **code only**, not prose documentation.

## Language constructs

- Variables: `let`, `let mut`, typed bindings (`name: Type = …`, `Array[T]`, `Dict[K, V]`)
- Functions: `fn` / `def`, parameters including `ref` (passed by value today)
- Control flow: `if` / `elif` / `else`, `while`, `for` / `for-in`, ternary `a if cond else b`
- Operators: arithmetic (`+`, `-`, `*`, `/`, `//`, `%`, `**`), comparisons, `and` / `or`, compound assignment
- Literals: integers, floats, strings, f-strings, lists, dicts, `None`, booleans
- Structs: fields, methods, `__init__`, field get/set
- Modules: `import m`, `import m as alias`, `from m import name`
- Builtins: `print` (variadic), integer/float math with division-by-zero guards on integer `/`, `%`, `//`

## Codegen modes

- JIT via Cranelift
- Object emission (`--emit-obj`)
- Executable linking with `hyper_rt.c` runtime (`--emit-exe`)

## CI-verified parity

These programs must match between `run`, `compile` (JIT), and `--emit-exe`:

- `ci/smoke.hyp` — structs, modules, equality, floats, dict order, forward calls
- `ci/divzero.hyp` — integer division by zero exits with `RuntimeError`

## Not compiled (see limitations)

File I/O (`with`, `open`), JSON module calls, `input()`, memory-mapped files, trait generics, and real `@parallel` thread codegen — listed in [Known limitations](known-limitations.md).
