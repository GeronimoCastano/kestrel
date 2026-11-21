# std.io

Writing without a newline, flushing, and reading one line. `print` already
writes a line; this module is for prompts and for standard error.

```kestrel
import std.io

io.write("name: ")
io.flush()
let name := io.input("")?
io.eprint(f"read {name.length()} bytes\n")
```

## Output

### `write(text : String) : void`

Writes to standard output. No newline is added and the stream is not flushed.

| Argument | Meaning |
|---|---|
| `text : String` | The text to write. |

```kestrel
io.write("loading")
io.write("...")
io.write("done\n")
```

### `eprint(text : String) : void`

Writes to standard error, with no newline. Diagnostics belong here, so they stay
separate from a program's output.

| Argument | Meaning |
|---|---|
| `text : String` | The text to write. |

### `flush() : void`

Flushes the standard streams, so text written earlier appears before the program
waits for input.

```kestrel
io.write("name: ")
io.flush()
```

## Input

### `input(prompt : String) : String or Error`

Writes the prompt without a newline, then reads one line from standard input.
The line ending (`\n` or `\r\n`) is removed.

| Argument | Meaning |
|---|---|
| `prompt : String` | Text shown before reading; pass `""` for none. |

```kestrel
let answer := io.input("continue? ")?
if answer == "y" { print("go") }
```

At end of input the call returns an error rather than an empty string, so a loop
can tell "no more input" from "the user pressed Enter".

## Notes

- `write` and `eprint` do not flush. Call `flush` before a prompt or after a
  partial line that must appear immediately.
- Standard output and standard error are separate streams. A terminal shows both;
  a redirected file receives only the first.
