# std.regex

Regular expressions with a linear-time engine: no pattern can make a search run
exponentially long.

```kestrel
import std.regex

let date := regex.compile("(\\d{4})-(\\d{2})-(\\d{2})")?
match date.find("due 2026-11-15") {
    Some(m) => print(m.group(1)),   // Some(2026)
    None => print("no date"),
}
print(regex.compile("\\s+")?.replace("a   b  c", " "))
```

## Syntax

| Form | Means |
|---|---|
| `abc` | Literal text. |
| `.` | Any character. |
| `[a-z]`, `[^...]` | A character class, or its complement. |
| `\d \w \s \D \W \S` | Digit, word character, whitespace, and their complements. |
| `\b \B` | A word boundary, or a position that is not one. |
| `^ $` | The start and the end of the text. |
| `( )` | A capturing group; `(?: )` groups without capturing. |
| `\|` | Either side. |
| `* + ? {n} {n,} {n,m}` | Repetition; a following `?` takes the shortest match. |
| `(?i)` | Ignores ASCII case. |

There are no backreferences and no lookaround, which is what keeps matching
linear.

## Functions

### `compile(pattern : String) : Regex or Error`

Builds a matcher. A malformed pattern returns an error that names the problem.

| Argument | Meaning |
|---|---|
| `pattern : String` | The pattern. |

### `escape(text : String) : String`

The text with every regular-expression metacharacter escaped, ready to be used
as a literal pattern.

| Argument | Meaning |
|---|---|
| `text : String` | The text to quote. |

```kestrel
let literal := regex.escape("a+b")
print(regex.compile(literal)?.isMatch("a+b"))   // true
```

## Matching

### `isMatch(text : String) : Bool`

True when the pattern matches anywhere in the text.

| Argument | Meaning |
|---|---|
| `text : String` | The text to search. |

### `fullMatch(text : String) : Bool`

True when the pattern matches the whole text.

| Argument | Meaning |
|---|---|
| `text : String` | The text to test. |

### `find(text : String) : Option[Match]`

The first match, or `None`.

| Argument | Meaning |
|---|---|
| `text : String` | The text to search. |

### `findAll(text : String) : List[Match]`

Every non-overlapping match, left to right.

| Argument | Meaning |
|---|---|
| `text : String` | The text to search. |

## Replacing and splitting

### `replace(text : String, replacement : String) : String`

Every match replaced. In `replacement`, `$0` to `$9` insert a group and `$$` is
a dollar sign.

| Argument | Meaning |
|---|---|
| `text : String` | The text to change. |
| `replacement : String` | The text to put in place of each match. |

### `replaceFirst(text : String, replacement : String) : String`

The same, for the first match only.

### `split(text : String) : List[String]`

The pieces between matches.

| Argument | Meaning |
|---|---|
| `text : String` | The text to split. |

## The Match record

| Field | Meaning |
|---|---|
| `text` | The matched text. |
| `start`, `end` | The span in characters. |
| `startByte`, `endByte` | The same span in bytes, for `byteSlice`. |
| `groups` | The captures; index 0 is the whole match. |
| `present` | Whether each group took part. |

### `group(index : Int) : Option[String]`

One capture by number. A group that did not participate gives `None`.

| Argument | Meaning |
|---|---|
| `index : Int` | The group number. |

## Notes

- Matching is linear in the length of the text, so it is safe on input you do not
  control.
- Groups count from zero, and zero is the whole match.
