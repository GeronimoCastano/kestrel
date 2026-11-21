# std.memory

The arena types. An `Arena` allocates values and hands out `ArenaRef` handles
that stay valid while the arena is alive.

```kestrel
import std.memory

let mut arena := Arena()
let handle := arena.insert(1)
let same := handle == handle
print(same)      // true
```

## The types

| Type | Meaning |
|---|---|
| `Arena` | A typed region that owns the values inserted into it. |
| `ArenaRef` | A handle to a value inside an arena. |

## Methods

### `insert(value : T) : ArenaRef[T]`

Stores a value and returns a handle to it. The arena must be `mut`.

| Argument | Meaning |
|---|---|
| `value : T` | The value to store. |

### `replace(handle : ArenaRef[T], value : T) : void`

Writes a new value through a handle.

| Argument | Meaning |
|---|---|
| `handle : ArenaRef[T]` | The slot to change. |
| `value : T` | The new value. |

## Notes

- An `ArenaRef` behaves as its value in ordinary expressions, and two handles are
  equal when they point at the same slot.
- There is no public constructor for `ArenaRef`; handles come from `insert`.
- The arena frees everything it holds when it is dropped. The type is there so a
  graph of values, such as closures that refer to each other, can outlive the
  expression that built it without copying.
