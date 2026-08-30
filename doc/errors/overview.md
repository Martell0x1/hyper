# Error kinds

Hyper reports failures through three error kinds. There is no `try` / `except` — execution stops and the process exits with a non-zero status.

| Kind | Exit code | When |
|------|-----------|------|
| `SyntaxError` | 65 | Invalid syntax, bad tokens, unsupported compile constructs, typecheck failures under `compile` |
| `IndentationError` | 65 | Unexpected indent, bad dedent, mixed tabs and spaces in indent |
| `RuntimeError` | 70 | Division by zero, undefined names, type mismatches at runtime, I/O failures |

## Format

```text
SyntaxError: line 5: at ':': expected expression
IndentationError: line 2: unexpected indent
RuntimeError: line 10: division by zero
```

Errors are written to **stderr**. User output uses **`print()`** only.

## Code samples vs error reference

| Location | Content |
|----------|---------|
| **`doc/examples/errors/`** | `.hyp` programs you **run** to see an error message |
| **`doc/errors/`** (this book) | **Prose** explaining error kinds, exit codes, and `run` vs `compile` |

Run a sample:

```bash
hyper run doc/examples/errors/runtime_error.hyp
```

Read this page to understand the format before writing tests or CI checks.

## Error kinds

- Parser and scanner errors are always `SyntaxError` / `IndentationError` regardless of command.
- Under `compile`, semantic/type failures are reported as `SyntaxError` and block codegen.
- Under `run`, the same type issues may appear as **`warning:`** on stderr while execution continues.
