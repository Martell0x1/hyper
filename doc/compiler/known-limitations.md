# Compiler known limitations

Hyper v0.1 targets a **working compiler for core programs**, not full language parity. When a construct is unsupported, lowering reports a **`SyntaxError`** with a line number before codegen starts (multiple errors collected in one pass when possible).

## Behavioral notes

| Construct | Compiler behavior |
|-----------|-------------------|
| `@parallel` on `for` | Emitted as a **sequential** loop (same per-index semantics). Threaded codegen is planned. |
| `@vectorize` on `for` | SIMD-oriented hint; still runs every index sequentially until SIMD/GPU backends land. |
| Type errors | **Fatal** under `run` and `compile` before codegen |

## Loop control flow

`break` and `continue` bind to the innermost enclosing `while` / `for` / `for-in` loop. Two cases are rejected:

- Outside any loop — `SyntaxError: line N: break outside loop` from the type checker (fatal before codegen).
- Inside a `@parallel` / `@parallel @vectorize` `for` body, where iterations are split across threads and an early exit has no single meaning.

A function body does not inherit the loop around its declaration, so a `break` inside a nested `fn` is an error.

## Language features not implemented (any backend)

- Generics (`make_it_speak[T: Speaker]` in docs is aspirational)
- Production GPU / SIMD codegen for `@vectorize`
- Full reclaim of every temporary string on the compile path (containers free overwritten elements; file/mmap handles free on close)
- `try` / `except` — Hyper uses explicit `raise` / `raises` / `handle` instead (see [Errors](../errors/overview.md))

## String methods

String methods share one runtime on **`run` and `compile`**. `split()` / `rsplit()` with no separator follow Python whitespace rules. `--emit-exe` case transforms are ASCII-oriented in the C runtime; JIT uses full Unicode case mapping.

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
