# std.args

Command-line flags, options and positional words, parsed into one value.

```kestrel
import std.args
import std.process

let parsed := args.parse(process.args, valued = ["out"])
if parsed.flag("verbose") {
    print("verbose")
}
let out := parsed.optionOr("out", "result.txt")
print(out)
```

The parser reads the words like this:

- `--name=value` is an option named `name`.
- `--name value` is an option only when `name` is listed in `valued`; otherwise
  `--name` is a flag.
- `-n` is the short spelling of `--name`, with the same rules.
- `--` ends the options; every later word is positional.
- A word that is neither is positional, in order.
- A valued option with no following word becomes a flag.

## Functions

### `parse(words : List[String], valued : List[String] = []) : Parsed`

| Argument | Meaning |
|---|---|
| `words : List[String]` | The words to parse, usually `process.args`. |
| `valued : List[String]` | Names whose long or short form takes a value, such as `["out", "o"]`. |

```kestrel
let parsed := args.parse(["--verbose", "--out", "report.txt", "main.kes"], valued = ["out"])
```

## The Parsed value

### `flag(name : String) : Bool`

True when `--name` or `-n` appeared without a value.

| Argument | Meaning |
|---|---|
| `name : String` | The name without its dashes. |

```kestrel
let parsed := args.parse(["--verbose"], valued = [])
print(parsed.flag("verbose"))   // true
```

### `option(name : String) : Option[String]`

The value given as `--name value`, `--name=value` or `-n value`.

| Argument | Meaning |
|---|---|
| `name : String` | The name without its dashes. |

```kestrel
let parsed := args.parse(["--out=x.txt"], valued = ["out"])
print(parsed.option("out"))     // Some(x.txt)
```

### `optionOr(name : String, fallback : String) : String`

The option's value, or `fallback` when the option is missing.

| Argument | Meaning |
|---|---|
| `name : String` | The name without its dashes. |
| `fallback : String` | The value to use when the option is absent. |

### `intOption(name : String) : Option[Int] or Error`

The option read as an `Int`. Missing is `None`; a value that is not an integer
returns an error.

| Argument | Meaning |
|---|---|
| `name : String` | The name without its dashes. |

```kestrel
let jobs := parsed.optionOr("jobs", "4").toInt()?
```

### `positional() : List[String]`

The words that are neither flags nor option values, in the order they appeared.

```kestrel
for (word in parsed.positional()) {
    print(word)
}
```

## Notes

- `process.args` holds the words after the program name, so the first positional
  word is the first user argument.
- Names are stored without their leading dashes; `--out=1` and `-o 1` both answer
  to `option("out")` and `option("o")` separately, because the parser keeps the
  spelling it saw.
