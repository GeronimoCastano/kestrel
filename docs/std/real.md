# std.real

The functions that work on `Real`, an alias for `double`.

```kestrel
import std.real

print(real.abs(-2.5))        // 2.5
print(real.sqrt(9.0))        // 3.0
print(real.isFinite(1.0 / 0.0))   // false
```

`Real` is a name for `double`, not a distinct type, so every `double` is a
`Real` and the two mix freely.

## Constants

| Name | Value |
|---|---|
| `real.PI` | 3.141592653589793 |
| `real.E` | 2.718281828459045 |

```kestrel
print(real.PI)   // 3.1415926535897931
```

## Functions

### `abs(x : Real) : Real`

The absolute value.

| Argument | Meaning |
|---|---|
| `x : Real` | The value. |

### `sqrt(x : Real) : Real`

The square root. A negative argument gives `NaN`.

| Argument | Meaning |
|---|---|
| `x : Real` | The value. |

### `min(a : Real, b : Real) : Real`

The smaller of two values. When one is `NaN`, the result follows the C library's
`fmin`, which returns the other.

| Argument | Meaning |
|---|---|
| `a : Real` | The first value. |
| `b : Real` | The second value. |

### `max(a : Real, b : Real) : Real`

The larger of two values, with the same rule for `NaN`.

| Argument | Meaning |
|---|---|
| `a : Real` | The first value. |
| `b : Real` | The second value. |

### `isFinite(x : Real) : Bool`

True when the value is neither infinite nor `NaN`.

| Argument | Meaning |
|---|---|
| `x : Real` | The value to test. |

## Notes

- The same functions exist in [`std.math`](math.md) under the same names. Use
  `std.real` when a proof or a signature talks about `Real`, and `std.math` when
  you want the wider set.
