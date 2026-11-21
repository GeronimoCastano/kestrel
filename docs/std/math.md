# std.math

Elementary functions and constants for `double`. The prelude already gives you
`sqrt`, `abs`, `min` and `max` without an import; this module adds the rest.

```kestrel
import std.math

print(math.PI)              // 3.1415926535897931
print(math.sqrt(2.0))       // 1.4142135623730951
print(math.pow(2.0, 10.0))  // 1024.0
```

## Constants

| Name | Value |
|---|---|
| `math.PI` | 3.1415926535897931 |
| `math.E` | 2.7182818284590451 |

## One argument

Every function takes a `double` and returns a `double`.

| Function | Meaning |
|---|---|
| `exp(x)` | $e^{x}$ |
| `expm1(x)` | $e^{x} - 1$, accurate for small `x` |
| `log(x)` | Natural logarithm |
| `log1p(x)` | $\log(1 + x)$, accurate for small `x` |
| `log2(x)` | Base-2 logarithm |
| `log10(x)` | Base-10 logarithm |
| `sin(x)`, `cos(x)`, `tan(x)` | Trigonometric functions, radians |
| `asin(x)`, `acos(x)`, `atan(x)` | Inverse trigonometric functions |
| `sinh(x)`, `cosh(x)`, `tanh(x)` | Hyperbolic functions |
| `sqrt(x)` | Square root |
| `abs(x)` | Absolute value |
| `floor(x)` | Largest integer not above `x` |
| `ceil(x)` | Smallest integer not below `x` |
| `round(x)` | Nearest integer, halfway away from zero |

```kestrel
print(math.exp(1.0))      // 2.7182818284590451
print(math.log(math.E))   // 1.0
print(math.floor(-1.5))   // -2.0
print(math.round(2.5))    // 3.0
```

## Two arguments

### `pow(a : double, b : double) : double`

`a` raised to `b`.

| Argument | Meaning |
|---|---|
| `a : double` | The base. |
| `b : double` | The exponent. |

```kestrel
print(math.pow(9.0, 0.5))   // 3.0
```

## Whole numbers

`abs`, `min` and `max` also accept `Int` and `BigInt`, and return the type they
were given.

```kestrel
print(math.min(3, 7))       // 3
print(math.abs(-4))         // 4
```

## Notes

- These are the C library functions. A result that is not finite stays a
  `double`: `math.log(-1.0)` is `NaN` rather than an error.
- `sqrt` and `abs` are also available unqualified, without the import.
