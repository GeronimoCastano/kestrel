# std.csv

Comma-separated text as RFC 4180 describes it: rows of fields, with quoting.

```kestrel
import std.csv

let rows := csv.parse("name,units\nnorth,120\nsouth,80\n")?
print(rows.length())          // 3

let records := csv.parseRecords("name,units\nnorth,120\nsouth,80\n")?
for (record in records) {
    print(record.get("units"))      // Some(120), then Some(80)
}
```

A field in double quotes may hold the delimiter, a line break, and `""` for a
quote. Lines end with LF or CRLF. A trailing line break does not add an empty
row.

## Functions

### `parse(text : String, delimiter : String = ",") : List[List[String]] or Error`

The rows of `text`, each a `List` of fields. The header, if there is one, is just
the first row.

| Argument | Meaning |
|---|---|
| `text : String` | The CSV text. |
| `delimiter : String` | One byte other than a quote or a line break. Defaults to `","`. |

```kestrel
let rows := csv.parse("a;b\n1;2\n", delimiter = ";")?
print(rows)                    // [[a, b], [1, 2]]
```

The delimiter must be exactly one byte. A quote inside an unquoted field, or text
after a closing quote, returns an error that names the line.

### `parseRecords(text : String, delimiter : String = ",") : List[Map[String, String]] or Error`

The rows as maps keyed by the first row's names. Every later row must have as
many fields as the header.

| Argument | Meaning |
|---|---|
| `text : String` | The CSV text, with a header row. |
| `delimiter : String` | Same rule as `parse`. |

```kestrel
let records := csv.parseRecords("city,population\nBerlin,3.7\n")?
for (record in records) {
    print(record.get("population"))   // Some(3.7)
}
```

A row with the wrong number of fields is an error, and the message says which
row.

### `write(rows : List[List[String]], delimiter : String = ",") : String`

CSV text for `rows`, quoting only the fields that need it, with LF after every
row.

| Argument | Meaning |
|---|---|
| `rows : List[List[String]]` | The rows to write. |
| `delimiter : String` | The field separator. |

```kestrel
let text := csv.write([["name", "note"], ["Kes", "says \"hi\""]])
print(text)
```

```text
name,note
Kes,"says ""hi"""
```

## Notes

- This module gives strings. For typed columns and data frames, use
  [`std.kesdas`](index.md#text-and-data).
- `parse` and `write` round-trip: writing rows and parsing them back gives the
  same fields.
