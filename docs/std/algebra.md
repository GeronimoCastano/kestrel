# std.algebra

The traits for algebraic structures, with the two-element group as a worked
example.

```kestrel
import std.algebra.{Z2, z2Plus, z2Times}

print(z2Plus(Z2.One, Z2.One))   // Zero
print(z2Times(Z2.One, Z2.One))  // One
print(z2Plus(Z2.Zero, Z2.One))  // One
```

## Traits

### `Group`

| Member | Meaning |
|---|---|
| `type Element` | The carrier type. |
| `op(a, b) : Element` | The operation. |
| `identity() : Element` | The neutral element. |
| `inverse(a) : Element` | The opposite of `a`. |

### `Ring`

| Member | Meaning |
|---|---|
| `type Element` | The carrier type. |
| `zero() : Element` | The additive identity. |
| `one() : Element` | The multiplicative identity. |
| `plus(a, b) : Element` | Addition. |
| `times(a, b) : Element` | Multiplication. |
| `negate(a) : Element` | The additive inverse. |

## The example: integers modulo two

| Name | Meaning |
|---|---|
| `Z2` | The enum with `Zero` and `One`. |
| `z2Plus(a, b)` | Addition modulo two. |
| `z2Times(a, b)` | Multiplication modulo two. |
| `z2Neg(a)` | Negation, which is the identity here. |
| `Z2Group` | The `Group` instance for `Z2`. |

`Z2Group` has the operations as methods, so it can be passed where a `Group` is
expected:

```kestrel
let group := Z2Group()
print(group.op(Z2.One, Z2.One))   // Zero
print(group.identity())           // Zero
```

## Notes

- The traits describe operations, not values; a type joins one by writing
  `has Group(Element = ...)`.
- The module also carries checked theorems, such as associativity of `z2Plus`,
  which the kernel verifies. They are proof material, not runtime code.
