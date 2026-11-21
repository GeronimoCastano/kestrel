# std.complex

Double-precision complex numbers.

```kestrel
import std.complex

let z := Complex(3.0, 4.0)
print(z.abs())          // 5.0
print(z.conjugate())    // 3.0-4.0i
```

## The Complex type

`Complex(re : double, im : double)` builds a value. The fields `re` and `im` are
public and read-only.

| Static | Value |
|---|---|
| `Complex.zero()` | $0 + 0i$ |
| `Complex.one()` | $1 + 0i$ |
| `Complex.i()` | $0 + 1i$ |

## Arithmetic

Each operation accepts a `Complex` or a `double` on the right, and the `right*`
forms accept a `double` on the left.

| Function | Meaning |
|---|---|
| `add(other)` | `this + other` |
| `subtract(other)` | `this - other` |
| `multiply(other)` | `this × other` |
| `divide(other)` | `this ÷ other` |
| `rightAdd(left)` | `left + this` |
| `rightSubtract(left)` | `left - this` |
| `rightMultiply(left)` | `left × this` |
| `rightDivide(left)` | `left ÷ this` |
| `negate()` | `-this` |
| `power(exponent : Int)` | `this` raised to a whole power |

```kestrel
let a := Complex(1.0, 2.0)
let b := Complex(3.0, -1.0)
print(a.add(b))        // 4.0+1.0i
print(a.multiply(b))   // 5.0+5.0i
print(b.rightSubtract(1.0))   // -2.0+1.0i
```

## Properties

| Function | Meaning |
|---|---|
| `conjugate()` | The complex conjugate |
| `normSquared()` | `re² + im²` |
| `abs()` | The modulus $\sqrt{re^{2} + im^{2}}$ |
| `arg()` | The argument, in radians |
| `isFinite()` | True when both parts are finite |

```kestrel
print(Complex(3.0, 4.0).normSquared())   // 25.0
print(Complex(0.0, 1.0).arg())           // 1.5707963267948966
```

## Comparing and printing

| Function | Meaning |
|---|---|
| `equals(other)` | True when both parts are equal |
| `copy()` | A value with the same parts |
| `text()` | `"3.0+4.0i"`, or `"3.0-4.0i"` |

## Notes

- Division by zero gives `NaN` or infinity, not an error.
- `Complex` is immutable: every operation returns a new value.
