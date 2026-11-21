# std.func

Three functions for passing behaviour around: `identity`, `compose` and
`constant`.

```kestrel
import std.func

function twice(x : Int) : Int = x * 2
function plusOne(x : Int) : Int = x + 1

print(identity(3))                      // 3
print(compose(twice, plusOne)(2))       // 6
print(constant(7, "ignored"))           // 7
```

## Functions

### `identity[T](x : T) : T`

The value it was given. It looks pointless until a function position needs one.

| Argument | Meaning |
|---|---|
| `x : T` | The value to return. |

### `compose[A, B, C](f : B -> C, g : A -> B) : A -> C`

The function that applies `g` first and then `f`.

| Argument | Meaning |
|---|---|
| `f : B -> C` | The second step. |
| `g : A -> B` | The first step. |

```kestrel
let addOneThenDouble := compose(twice, plusOne)
print(addOneThenDouble(2))   // 6
```

### `constant[A, B](value : B, ignored : A) : B`

A function of two arguments that always returns the first. It exists so a
callback that receives a value can stand in for one that does not need it.

| Argument | Meaning |
|---|---|
| `value : B` | The result. |
| `ignored : A` | Accepted and thrown away. |

## Notes

- Function types are written `A -> B`, and a function value is called the same
  way as a named one.
- `compose` reads right to left, as in mathematics: `compose(f, g)(x)` is
  `f(g(x))`.
