# std.test

Assertions for `@test` functions and for any check a program makes about itself.

```kestrel
import std.test.{assertEq, assertTrue}

@test
function addition() : void {
    assertEq(1 + 1, 2)
    assertTrue(2 > 1)
}
```

A failing assertion stops the test and names what it saw. `kestrel test` runs
every `@test` function in a package's `tests` folder in its own process.

## Functions

### `assertEq[T : Equals](a : T, b : T) : void`

Fails unless `a == b`, showing both values.

| Argument | Meaning |
|---|---|
| `a : T` | The value produced. |
| `b : T` | The value expected. |

### `assertNotEq[T : Equals](a : T, b : T) : void`

Fails when `a == b`.

| Argument | Meaning |
|---|---|
| `a : T` | The first value. |
| `b : T` | The second value. |

### `assertTrue(value : Bool) : void`

Fails unless the condition holds.

| Argument | Meaning |
|---|---|
| `value : Bool` | The condition. |

### `assertFalse(value : Bool) : void`

Fails when the condition holds.

| Argument | Meaning |
|---|---|
| `value : Bool` | The condition. |

### `assertNear(a : double, b : double, tolerance : double) : void`

Fails when `|a - b|` is larger than `tolerance`. Use it for floating-point
results.

| Argument | Meaning |
|---|---|
| `a : double` | The value produced. |
| `b : double` | The value expected. |
| `tolerance : double` | The largest acceptable difference. |

```kestrel
assertNear(0.1 + 0.2, 0.3, 1e-12)
```

### `assertError(value : T) : void`

Fails when the value is not an error. It takes the `or Error` result directly.

| Argument | Meaning |
|---|---|
| `value : T` | A value that should have failed. |

```kestrel
assertError(parseInt("not a number"))
```

## Notes

- `kestrel test` reports each test separately and continues after a failure in
  another test.
- An assertion is an ordinary function call, so it works inside `main` too; the
  process stops with a message when it fails.
