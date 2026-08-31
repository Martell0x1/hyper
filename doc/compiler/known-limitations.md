# Compiler known limitations

Hyper v0.1 targets a **working compiler for core programs**, not full language parity. When a construct is unsupported, lowering reports a **`SyntaxError`** with a line number before codegen starts (multiple errors collected in one pass when possible).

## Interpreter-only today

| Construct | Compile-time message (typical) | Workaround |
|-----------|-------------------------------|------------|
| `with open_mmap(...) as m:` | `memory-mapped file blocks are not supported by the compiler yet` | Use `hyper run` |
| `input()` as a call | `'input' is only available on the interpreter path` | Use `hyper run` |

`open(...)`, `with open(...) as f:`, file methods, and `import json` (`loads`, `dumps`, `load`, `dump`) **are supported** on the compile path (JIT and `--emit-exe`).

## Lowered differently than interpreted

| Construct | Compiler behavior |
|-----------|-------------------|
| `@parallel` / `@vectorize` on `for` | Emitted as **sequential** loops (same semantics for pure numeric loops; no thread pool yet) |
| Type errors | **Fatal** under `compile`; **warnings** under `run` |

## Language features not implemented (any backend)

- Generics (`make_it_speak[T: Speaker]` in docs is aspirational)
- `break` / `continue`
- Enforced `pub` / `mut` on struct members (parsed, not enforced)
- Real `ref` semantics (zero-copy references)
- Collection methods: `len`, `append`, `keys` on Hyper values

## Struct method resolution

The compiler must know the struct type at the call site. It follows:

- Constructor assignments (`let p = Point(...)`)
- Field access chains
- Annotated parameters and return types
- Function return tracking

If a method call fails to resolve, you get a compile error naming the missing field or method — add an annotation or restructure so the type is known earlier.

## Error message format

All diagnostics use:

```text
SyntaxError: line N: …
IndentationError: line N: …
RuntimeError: line N: …
```

User programs should use **`print()`** only — errors go to stderr through the runtime, not via language-level logging APIs.
