# std.python

Call Python from Kestrel. A foreign import names a module and binds it to a
name; the calls are checked while the program runs, not by the checker.

```kestrel
import python "numpy" as np

let a := np.array([1.0, 2.0])
print(a)
```

## How it works

- `import python "module" as name` loads a Python module. The module must be
  importable by the Python that the program is linked against.
- A Python value is a `PythonValue`. Calls, attribute reads and indexing happen
  through it, and each result is checked when it runs.
- A type mistake therefore shows up at run time, not at `check`.
- The compiler links the Python library when a program imports this module.
  `std.python` is only the type; the import is the interface.

## PythonValue

| Operation | Meaning |
|---|---|
| `value.attribute` | Read an attribute. |
| `value.method(args)` | Call a method. |
| `value[index]` | Index. |
| `value(arguments)` | Call a callable. |

The value converts to a Kestrel value when the surrounding expression needs one,
and a mismatch becomes a `RuntimeError`.

## Errors

A Python exception becomes an error in Kestrel. It can be caught with `match` on
`RuntimeError`, and its message carries the Python exception's text.

## Notes

- Python is embedded: the interpreter runs inside the Kestrel program, and the
  program owns its lifetime.
- Worker callbacks may call Python while the bridge is idle. If another thread
  owns it, the nested call returns `RuntimeError.Python` instead of waiting.
- A program that uses Python needs the Python development files when it is
  compiled.
