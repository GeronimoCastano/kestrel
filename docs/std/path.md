# std.path

Paths as strings, with `/` between parts.

```kestrel
import std.path

let report := "runs/2026-11/q3.csv"
print(path.fileName(report))          // q3.csv
print(path.parent(report))            // runs/2026-11
print(path.extension(report))         // csv
print(path.stem(report))              // q3
print(path.join("runs", "q3.csv"))    // runs/q3.csv
```

A path here is only text. Nothing in this module checks that a file exists.
`walk` is the one function that reads the disk; the rest only read the string.

## Functions

### `fileName(path : String) : String`

The last part of the path. A trailing slash is ignored.

```kestrel
path.fileName("a/b.txt")   // "b.txt"
path.fileName("a/b/")      // "b"
path.fileName("/")         // ""
```

### `parent(path : String) : String`

Everything before the last part. `parent` of a bare name is `""`, and `parent`
of a path at the root is `"/"`.

```kestrel
path.parent("a/b.txt")     // "a"
path.parent("b.txt")       // ""
path.parent("/b.txt")      // "/"
```

### `extension(path : String) : String`

The text after the last dot of the file name, without the dot. A name that
starts with a dot, or has no dot, has no extension.

```kestrel
path.extension("a/b.tar.gz")  // "gz"
path.extension(".bashrc")     // ""
path.extension("Makefile")    // ""
```

### `stem(path : String) : String`

The file name without its extension. Only the last dot is removed.

```kestrel
path.stem("a/b.tar.gz")    // "b.tar"
path.stem(".bashrc")       // ".bashrc"
```

### `join(base : String, part : String) : String`

Joins two paths with exactly one `/`. An absolute `part` replaces `base`, and an
empty side leaves the other one alone.

```kestrel
path.join("a", "b")        // "a/b"
path.join("a/", "b")       // "a/b"
path.join("a", "/b")       // "/b"
path.join("", "b")         // "b"
path.join("a", "")         // "a"
```

### `isAbsolute(path : String) : Bool`

True when the path starts with `/`. This is a syntactic test: on Windows a path
such as `C:\a` is not absolute by this rule.

```kestrel
path.isAbsolute("/a")      // true
path.isAbsolute("a")       // false
```

### `parts(path : String) : List[String]`

The non-empty parts between slashes.

```kestrel
path.parts("/a//b/")       // ["a", "b"]
```

### `walk(root : String) : List[String] or Error with Effects`

Every file below `root`, with paths that start with `root`. Directories
themselves are not listed. The walk is depth first, and each directory's entries
are visited in byte order.

```kestrel
for (file in path.walk("src")?) {
    print(file)
}
```

`walk` reads the file system, so it carries `Effects` and returns `or Error`.
A directory it cannot read stops the walk and returns the error.

## Notes

- Separators are `/` on every platform. The functions do not translate `\` and
  do not normalize `..` or `.`.
- This module does not check that a path exists. Use `std.fs` for that.
