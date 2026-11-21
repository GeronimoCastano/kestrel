# std.set

The set operations that read as free functions. The methods live on `Set` in
[`std.core`](core.md); these are the names that take two sets as arguments.

```kestrel
import std.set

let a := Set().add(1).add(2)
let b := Set().add(2).add(3)

print(set.cardinality(a))     // 2
print(set.subset(a, b))       // false
print(set.disjoint(a, b))     // false
```

`Set` is persistent: `add` returns a new set and leaves the old one alone.

## Functions

### `isEmpty[T : Equals](s : Set[T]) : Bool`

True when the set has no values.

| Argument | Meaning |
|---|---|
| `s : Set[T]` | The set. |

### `member[T : Equals](s : Set[T], value : T) : Bool`

True when `value` is in `s`.

| Argument | Meaning |
|---|---|
| `s : Set[T]` | The set. |
| `value : T` | The value to look for. |

### `subset[T : Equals](a : Set[T], b : Set[T]) : Bool`

True when every value of `a` is in `b`, including the case where they are equal.

| Argument | Meaning |
|---|---|
| `a : Set[T]` | The candidate subset. |
| `b : Set[T]` | The set to compare against. |

### `properSubset[T : Equals](a : Set[T], b : Set[T]) : Bool`

True when `a` is a subset of `b` and the two are not equal.

| Argument | Meaning |
|---|---|
| `a : Set[T]` | The candidate subset. |
| `b : Set[T]` | The set to compare against. |

### `disjoint[T : Equals](a : Set[T], b : Set[T]) : Bool`

True when the sets share no value.

| Argument | Meaning |
|---|---|
| `a : Set[T]` | The first set. |
| `b : Set[T]` | The second set. |

### `cardinality[T : Equals](s : Set[T]) : Int`

The number of values in the set.

| Argument | Meaning |
|---|---|
| `s : Set[T]` | The set. |

## Notes

- `Set.union`, `intersection`, `difference`, `isSubset` and `isDisjoint` are the
  method forms on the type itself.
- A value type needs `Equals` to be a set element.
