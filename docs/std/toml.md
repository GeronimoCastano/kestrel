# std.toml

TOML configuration text to a `Toml` value, with typed getters.

```kestrel
import std.toml

let doc := toml.parse("name = \"kestrel\"\nport = 8080\n")?
print(toml.getString(doc, "name"))   // Some(kestrel)
print(toml.getInt(doc, "port"))      // Some(8080)
```

## The Toml type

| Case | Holds |
|---|---|
| `TString(String)` | Text. |
| `TInt(Int)` | An integer. |
| `TFloat(double)` | A floating-point number. |
| `TBool(Bool)` | A boolean. |
| `TDateTime(String)` | A timestamp, kept as its text. |
| `TArray(List[Toml])` | An array. |
| `TTable(List[(String, Toml)])` | A table, in document order. |

## Functions

### `parse(text : String) : Toml or Error`

Reads a TOML document.

| Argument | Meaning |
|---|---|
| `text : String` | The document. |

### `getString(doc : Toml, path : String) : Option[String]`

The string at a path, or `None` when the path is missing or holds another type.

| Argument | Meaning |
|---|---|
| `doc : Toml` | The document. |
| `path : String` | A name, or names joined with dots: `"server.port"`. |

### `getInt(doc : Toml, path : String) : Option[Int]`

The integer at a path.

| Argument | Meaning |
|---|---|
| `doc : Toml` | The document. |
| `path : String` | A dotted path. |

### `getFloat(doc : Toml, path : String) : Option[double]`

The float at a path. An integer at that path does not convert.

| Argument | Meaning |
|---|---|
| `doc : Toml` | The document. |
| `path : String` | A dotted path. |

### `getBool(doc : Toml, path : String) : Option[Bool]`

The boolean at a path.

| Argument | Meaning |
|---|---|
| `doc : Toml` | The document. |
| `path : String` | A dotted path. |

## Notes

- A getter returns `None` for a missing path and for a value of the wrong type;
  it does not guess.
- Tables keep the order of the document, so writing a `Toml` back should preserve
  it.
- Dotted paths walk tables; an array is not indexed by a number in a path.
