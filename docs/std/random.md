# std.random

A seeded random number generator. One `Rng` value owns its state, so two
generators with the same seed produce the same sequence.

```kestrel
import std.random

let mut rng := Rng.seeded(42)
print(rng.integers(1, 7))       // a value from 1 to 6
print(rng.uniform(0.0, 1.0))    // a value in [0, 1)
print(rng.normal(0.0, 1.0))     // a value from the normal distribution
```

The methods change the generator, so the value must be `mut`.

## The Rng type

### `Rng.seeded(seed : Int) : Rng`

A new generator. The same seed gives the same sequence on every platform, so a
test can compare against fixed values.

| Argument | Meaning |
|---|---|
| `seed : Int` | Starting state. |

```kestrel
let mut a := Rng.seeded(7)
let mut b := Rng.seeded(7)
print(a.integers(0, 100) == b.integers(0, 100))   // true
```

### `uniform(low : double, high : double) : double`

A value in `[low, high)`.

| Argument | Meaning |
|---|---|
| `low : double` | Lower bound, included. |
| `high : double` | Upper bound, excluded. |

### `normal(mean : double, sigma : double) : double`

A value from the normal distribution with the given mean and standard deviation.

| Argument | Meaning |
|---|---|
| `mean : double` | Centre of the distribution. |
| `sigma : double` | Standard deviation, positive. |

### `integers(low : Int, high : Int) : Int`

An integer in `[low, high)`.

| Argument | Meaning |
|---|---|
| `low : Int` | Lower bound, included. |
| `high : Int` | Upper bound, excluded. |

```kestrel
let roll := rng.integers(1, 7)   // 1 to 6
```

## Notes

- The generator is not cryptographically secure. Do not use it for keys or
  passwords.
- Copying an `Rng` copies its state, so two copies produce the same sequence.
