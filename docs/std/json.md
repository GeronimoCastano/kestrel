# std.json

JSON text to a `Json` value and back, following RFC 8259.

```kestrel
import std.json

let value := json.parse("{\"name\": \"Kes\", \"stars\": 3}")?
match value {
    JObject(fields) => {
        for ((key, item) in fields) {
            print(key)
        }
    },
    _ => print("not an object"),
}
```

## The Json type

| Case | Holds | Written as |
|---|---|---|
| `JNull` | nothing | `null` |
| `JBool(Bool)` | a boolean | `true`, `false` |
| `JNumber(double)` | a number | a number |
| `JString(String)` | text | a string |
| `JArray(List[Json])` | values | an array |
| `JObject(List[(String, Json)])` | named values | an object |

Numbers are stored as `double`. An integer beyond $2^{53}$ loses precision on the
way in, so keep identifiers that large as strings.

An object keeps its fields in the order the text gives them, and `stringify`
writes them in that order.

## Functions

### `parse(text : String) : Json or Error`

Reads one JSON value from `text`. Surrounding whitespace is allowed.

| Argument | Meaning |
|---|---|
| `text : String` | The JSON text. |

```kestrel
let value := json.parse("[1, 2, 3]")?
match value {
    JArray(items) => print(items.length()),   // 3
    _ => print("not an array"),
}
```

A malformed document returns an error that names the byte offset and what the
parser expected there.

### `stringify(value : Json) : String`

The compact form, with no spaces between tokens.

| Argument | Meaning |
|---|---|
| `value : Json` | The value to write. |

```kestrel
let value := json.parse("{\"a\": [1, 2]}")?
print(json.stringify(value))
```

```text
{"a":[1,2]}
```

### `pretty(value : Json, indent : Int = 2) : String`

The indented form. `indent` is the number of spaces per level; `0` gives the same
text as `stringify`.

| Argument | Meaning |
|---|---|
| `value : Json` | The value to write. |
| `indent : Int` | Spaces per nesting level. Defaults to `2`. |

```kestrel
print(json.pretty(value, indent = 4))
```

## Notes

- Keys are ordinary strings; a duplicate key stays in the list of fields rather
  than replacing the earlier one.
- `stringify` escapes what JSON requires and leaves other characters as they are,
  so the output is UTF-8 text rather than ASCII with `\u` escapes.
