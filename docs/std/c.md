# std.c

Call C. The compiler reads a header, brings its function signatures into scope,
and every call sits inside an `unsafe` block so the risk is visible.

```kestrel
import c "math.h"

let root := unsafe { sqrt(2.0) }
print(root)              // 1.4142135623730951
```

## Foreign imports

`import c "header.h"` reads the header and adds what it declares. The header must
be findable by the C compiler that builds the program; use a system header or a
path relative to the package.

```kestrel
import std.c.{CUInt64}
import c "stdlib.h"

function allocate() : void or Error with Effects {
    let memory := unsafe { malloc(CUInt64(64)?) }
    if memory.isNull() {
        return RuntimeError.Io("allocation failed")
    }
    unsafe { free(memory) }
}
```

## The types

| Name | Meaning |
|---|---|
| `std.c.CPointer[T]` | A mutable pointer to `T`. |
| `std.c.CConstPointer[T]` | A pointer that cannot be written through. |
| `std.c.CVoid` | `void`. |
| `std.c.CInt`, `CUInt`, `CLong`, `CULong`, `CInt64`, `CUInt64`, `CSize`, `CUChar`, `CChar`, ... | The C integer types. |

The exact widths come from the C compiler, so a `CLong` is 64 bits on macOS and
Linux and 32 bits on Windows.

## Safety

- Every C call and every dereference goes inside `unsafe { ... }`.
- A pointer has `isNull()`; there is no automatic check when it is used.
- A C string is a pointer to bytes. Converting it to `String` requires a length;
  a pointer does not carry one.
- Memory returned by `malloc` is freed with `free`; the compiler does not track
  it.

## Notes

- A header that the compiler cannot find is an import error that names the header.
- A C function may be declared but not linked; the link step then reports the
  missing symbol and suggests `--link`, as in `--link m`.
- Passing a Kestrel struct to C requires the struct to have a C layout, which the
  language does not promise; pass primitive pointers instead.
