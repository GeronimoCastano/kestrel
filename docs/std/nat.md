# std.nat

Natural-number arithmetic written as structural functions, so a proof can unfold
it. The built-in `+` and `*` stay available; these are the names that reduce to
`match` on `Zero` and `Succ`.

```kestrel
import std.nat.{plus, times}

print(plus(Succ(Zero), Succ(Zero)))    // 2
print(times(Succ(Succ(Zero)), Succ(Succ(Zero))))    // 4
```

## Functions

### `plus(a : Nat, b : Nat) : Nat`

Addition by recursion on `a`.

| Argument | Meaning |
|---|---|
| `a : Nat` | The first addend; its structure drives the recursion. |
| `b : Nat` | The second addend. |

### `times(a : Nat, b : Nat) : Nat`

Multiplication by recursion on `a`, using `plus`.

| Argument | Meaning |
|---|---|
| `a : Nat` | The multiplier. |
| `b : Nat` | The multiplicand. |

## Notes

- `Nat` values are built from `Zero` and `Succ(...)`. Numeric literals are `Int`,
  so write `Succ(Succ(Zero))` for two.
- The module carries checked identities, such as `plus(n, Zero) = n`, with the
  recursion made explicit so `induction` can use it.
- Use these in proofs. For ordinary arithmetic, the built-in `+` and `*` on
  `Nat` do the same work.
