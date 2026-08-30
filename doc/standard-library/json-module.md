# JSON module

`json` is a **builtin module** implemented in the Rust runtime — no `.hyp` file to install.

**Compiler status:** JSON calls compile only after the module is lowered; today use `hyper run`. See [Known limitations](../compiler/known-limitations.md).

## Import

```hyper
import json
```

## Functions

| Function | Result |
|----------|--------|
| `json.loads(text)` | Value from a JSON string |
| `json.dumps(value)` | Compact JSON string |
| `json.dumps(value, indent)` | Pretty-print with `indent` spaces |
| `json.load(file)` | Value from an open file handle |
| `json.dump(value, file)` | Write compact JSON |
| `json.dump(value, file, indent)` | Write pretty JSON |

## Type mapping

| JSON | Hyper |
|------|-------|
| object | dict (insertion order preserved on load) |
| array | list |
| string | string |
| number (integer) | `i64` |
| number (fraction) | `f64` |
| true / false | boolean |
| null | `None` |

Struct instances serialize as JSON objects by field name. On dump, dict keys are sorted for stable output.

## Example

Code sample: `doc/examples/file_handling/json_io.hyp`.

In-memory round trip:

```hyper
import json

let config = {"name": "Hyper", "version": 1}
let text = json.dumps(config)
let parsed = json.loads(text)
print(parsed["name"])
```

## Related

- [File handling](file-handling.md) — `open` and `with` for reading/writing files
