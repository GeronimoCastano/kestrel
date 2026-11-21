# std.term

ANSI colours, a progress bar and leveled logs.

```kestrel
import std.term

print(term.bold("done"))
print(term.green("ok"))
print(term.progressBar(3, 10, 20))
```

A style function wraps the text in escape codes and returns the string; nothing
is printed until you print it. Set `NO_COLOR` in the environment to get the plain
text instead.

## Styles

| Function | Effect |
|---|---|
| `bold(text)` | Bold. |
| `dim(text)` | Dim. |
| `underline(text)` | Underlined. |
| `red(text)` | Red. |
| `green(text)` | Green. |
| `yellow(text)` | Yellow. |
| `blue(text)` | Blue. |
| `magenta(text)` | Magenta. |
| `cyan(text)` | Cyan. |

Each takes one `String` and returns a `String`.

## Progress and logs

### `progressBar(done : Int, total : Int, width : Int) : String`

A bar of `width` characters, filled in proportion to `done / total`, with the
counts.

| Argument | Meaning |
|---|---|
| `done : Int` | How much is finished. |
| `total : Int` | How much there is in all. |
| `width : Int` | How many characters the filled part may span. |

### `log(threshold : Int, level : Int, message : String) : void`

Writes one line when `level` is at least `threshold`. The label is `DEBUG`,
`INFO`, `WARNING` or `ERROR`, coloured unless `NO_COLOR` is set.

| Argument | Meaning |
|---|---|
| `threshold : Int` | The lowest level that is shown. |
| `level : Int` | This message's level. |
| `message : String` | The text. |

## Levels

| Constant | Value |
|---|---|
| `term.DEBUG` | 10 |
| `term.INFO` | 20 |
| `term.WARNING` | 30 |
| `term.ERROR` | 40 |

```kestrel
term.log(term.INFO, term.DEBUG, "not shown")
term.log(term.INFO, term.WARNING, "shown")
```

## Notes

- The functions never detect whether the output is a terminal. Redirecting to a
  file keeps the escape codes; prefer `NO_COLOR` when that matters.
- `progressBar` returns a string. Print it with `\r` and `io.flush` for a bar that
  updates in place.
