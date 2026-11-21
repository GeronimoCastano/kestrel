# std.str

Text helpers that read as free functions. The methods on `String` — `substring`,
`split`, `replace`, `lines` and the rest — live in [`std.core`](core.md); this
module gives the few that are used as plain calls.

```kestrel
import std.str

print(str.trim("  hello  "))        // hello
print(str.toUpper("kestrel"))       // KESTREL
print(str.length("kestrel"))        // 7
print(str.contains("kestrel", "str"))   // true
print(str.split("a,b,c", ","))      // [a, b, c]
```

## Functions

### `trim(text : String) : String`

Removes whitespace at both ends.

| Argument | Meaning |
|---|---|
| `text : String` | The text to trim. |

### `toUpper(text : String) : String`

Upper case. The change is applied to the UTF-8 text, not to one byte at a time.

| Argument | Meaning |
|---|---|
| `text : String` | The text. |

### `toLower(text : String) : String`

Lower case, under the same rule.

| Argument | Meaning |
|---|---|
| `text : String` | The text. |

### `length(text : String) : Int`

The number of characters, not bytes. For the byte count use `text.byteLength()`.

| Argument | Meaning |
|---|---|
| `text : String` | The text. |

### `contains(text : String, part : String) : Bool`

True when `part` appears in `text`.

| Argument | Meaning |
|---|---|
| `text : String` | The text to search. |
| `part : String` | The text to look for. |

### `split(text : String, separator : String) : List[String]`

Splits on every occurrence of `separator`.

| Argument | Meaning |
|---|---|
| `text : String` | The text to split. |
| `separator : String` | The delimiter; an empty one splits into characters. |

## Notes

- `String` methods do more: `trim`, `split`, `replace`, `lines`, `words`,
  `startsWith`, `padStart`, `slice` and `graphemes`. They are listed under
  [`std.core`](core.md).
