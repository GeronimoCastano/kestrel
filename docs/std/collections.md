# std.collections

The names of the standard containers, in one import.

```kestrel
import std.collections

let names := List()            // an immutable list
let mut counts := MutMap()     // a map you can change
counts.put("a", 1)
print(names.length())
print(counts.get("a"))
```

The types live in the prelude and are available without an import; this module
exists so a file can say what it uses in one line, and so a reader sees the
container names together.

## The containers

| Name | Shape | Changes? | Ordered? |
|---|---|---|---|
| `List[T]` | A sequence with cheap prepend and structural sharing. | No | Yes, by position |
| `Array[T]` | A contiguous sequence with indexing and in-place update. | With `mut` | Yes, by position |
| `Map[K, V]` | A persistent map from keys to values. | No | By key |
| `MutMap[K, V]` | A map that supports `put` and `remove`. | Yes, with `mut` | By key |
| `Set[T]` | A persistent set of distinct values. | No | By value |
| `MutSet[T]` | A set that supports `add` and `remove`. | Yes, with `mut` | By value |
| `SortedSet[T]` | A set that keeps its values in order. | No | Sorted |

## Notes

- A container is a value. `List` and `Map` are persistent: a change returns a new
  value and the old one stays valid. `Array` and the `Mut*` types change in place
  and need `mut`.
- The methods are documented under [`std.core`](core.md): `List.take`,
  `Map.merge`, `Set.union`, `Array.push` and the rest.
- `Map` keys and `Set` values need `Equals`; a sorted container also needs an
  order.
