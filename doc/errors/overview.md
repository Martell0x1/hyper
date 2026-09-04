# Error kinds

Hyper reports failures through three error kinds. There is **no** `try` / `except`. Recoverable failures use explicit **`raise`**, **`raises`**, and **`handle`**.

| Kind | Exit code | When |
|------|-----------|------|
| `SyntaxError` | 65 | Invalid syntax, bad tokens, unsupported compile constructs, typecheck failures under `compile` |
| `IndentationError` | 65 | Unexpected indent, bad dedent, mixed tabs and spaces in indent |
| `RuntimeError` | 70 | Division by zero, undefined names, type mismatches at runtime, I/O failures, uncaught `raise` |

## Explicit error flow (no try/except)

| Form | Role |
|------|------|
| `raise <expr>` | Signal a failure value (string, number, …). Uncaught → `RuntimeError` and exit **70**. |
| `fn f(...) raises` / `fn f(...) -> T raises` / `fn f(...) raises -> T` | Marks a function that may `raise`. A bare `raise` inside a function without `raises` is a `SyntaxError`. |
| `handle <attempt> else <fallback>` | Evaluates `attempt`; if it raises, yields `fallback` instead. |

Example:

```hyper
fn parse_score(n: i64) raises -> i64:
    if n < 0:
        raise "score must be non-negative"
    return n

print(handle parse_score(10) else 0)
print(handle parse_score(-3) else 0)
```

Module-level `raise` is allowed and exits the process. Prefer `handle` at call sites of `raises` functions.

## Format

```text
SyntaxError: line 5: at ':': expected expression
SyntaxError: line 1: module 'foo' not found (searched ./foo.hyp)
IndentationError: line 2: unexpected indent
RuntimeError: line 10: division by zero
```

Errors are written to **stderr**. User output uses **`print()`** only.

## Code samples vs error reference

| Location | Content |
|----------|---------|
| **`doc/examples/errors/`** | `.hyp` programs you **run** to see an error message or `handle` recovery |
| **`doc/errors/`** (this book) | **Prose** explaining error kinds, exit codes, and `run` vs `compile` |

Run a sample:

```bash
hyper run doc/examples/errors/runtime_error.hyp
hyper run doc/examples/errors/raise_handle.hyp
hyper run doc/examples/errors/module_not_found.hyp
```

## `run` vs `compile`

- Parser and scanner errors are always `SyntaxError` / `IndentationError` regardless of command.
- Under `compile`, semantic/type failures are reported as `SyntaxError` and block codegen.
- Under `run`, the same type issues may appear as **`warning:`** on stderr while execution continues.
- `raise` / `handle` lower on both backends.
