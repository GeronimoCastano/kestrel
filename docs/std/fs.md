# std.fs

Files and directories. Every call that touches the disk carries `Effects` and
returns `or Error`.

```kestrel
import std.fs

fs.writeFile("notes.txt", "first line\n")?
print(fs.readFile("notes.txt")?)   // first line
```

Paths are plain strings. Use [std.path](path.md) to build them, and pass them
unchanged: this module does no normalisation.

## Reading

### `readFile(path : String) : String or Error`

The whole file as text.

| Argument | Meaning |
|---|---|
| `path : String` | The file to read. |

```kestrel
let text := fs.readFile("README.md")?
```

Text that is not valid UTF-8 is refused. Read the file as bytes instead.

### `readBytes(path : String) : Bytes or Error`

The whole file as raw bytes.

| Argument | Meaning |
|---|---|
| `path : String` | The file to read. |

```kestrel
let bytes := fs.readBytes("logo.png")?
print(bytes.length())
```

## Writing

### `writeFile(path : String, text : String) : void or Error`

Creates the file, or truncates it, then writes the text as UTF-8. The directory
must already exist.

| Argument | Meaning |
|---|---|
| `path : String` | The file to write. |
| `text : String` | The complete new contents. |

```kestrel
fs.writeFile("out.txt", "hello\n")?
```

### `appendFile(path : String, text : String) : void or Error`

Creates the file when it does not exist, then appends the text.

| Argument | Meaning |
|---|---|
| `path : String` | The file to append to. |
| `text : String` | The text to add at the end. |

```kestrel
fs.appendFile("log.txt", "one more line\n")?
```

### `writeBytes(path : String, bytes : Bytes) : void or Error`

Creates the file, or truncates it, then writes raw bytes.

| Argument | Meaning |
|---|---|
| `path : String` | The file to write. |
| `bytes : Bytes` | The bytes to store. |

### `appendBytes(path : String, bytes : Bytes) : void or Error`

Appends raw bytes to the file, creating it when needed.

## Looking around

### `exists(path : String) : Bool`

True when the path exists. Symbolic links are followed, and a path that cannot
be inspected is `false` rather than an error.

| Argument | Meaning |
|---|---|
| `path : String` | The path to test. |

```kestrel
if fs.exists("cache.json") { print("cached") }
```

### `isDir(path : String) : Bool`

True when the path exists and is a directory.

| Argument | Meaning |
|---|---|
| `path : String` | The path to test. |

### `listDir(path : String) : List[String] or Error`

The entry names in the directory, without `.` and `..`, sorted by byte order.
The result holds names, not paths.

| Argument | Meaning |
|---|---|
| `path : String` | The directory to read. |

```kestrel
for (name in fs.listDir("src")?) { print(name) }
```

## Changing things

### `createDirs(path : String) : void or Error`

Creates every missing directory along the path. A path whose directories already
exist is not an error.

| Argument | Meaning |
|---|---|
| `path : String` | The directory to create, parents included. |

```kestrel
fs.createDirs("build/reports")?
```

### `remove(path : String) : void or Error`

Removes a file or an empty directory. A directory with entries is refused.

| Argument | Meaning |
|---|---|
| `path : String` | The file or empty directory to remove. |

### `rename(from : String, to : String) : void or Error`

Moves or renames. Whether an existing `to` is replaced is the operating system's
decision.

| Argument | Meaning |
|---|---|
| `from : String` | The current path. |
| `to : String` | The new path. |

### `copy(from : String, to : String) : void or Error`

Copies the bytes. An existing regular file at `to` is truncated first, and
copying a file onto itself is refused.

| Argument | Meaning |
|---|---|
| `from : String` | The file to read. |
| `to : String` | The file to create or overwrite. |

```kestrel
fs.copy("template.kes", "main.kes")?
```

### `tempDir() : String or Error`

Creates a new empty directory in the system temporary location and returns its
path. Each call creates another directory.

```kestrel
let dir := fs.tempDir()?
fs.writeFile(path.join(dir, "x.txt"), "x")?
```

## Notes

- `writeFile`, `writeBytes` and `copy` truncate; the `append` functions do not.
- `exists` and `isDir` answer with a `Bool` and never fail, so a missing path is
  not an error.
- The directory must exist before a write. Use `createDirs` first when the path
  is not guaranteed.
