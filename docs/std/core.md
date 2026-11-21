# std.core

The types and traits every program starts with: the containers, `Option`,
`String`, `Bytes`, `Error`, and the data structures the prelude loads.

```kestrel
let stack := Stack()
stack.push(1)
stack.push(2)
print(stack.peek())    // Some(2)
print(stack.pop())     // Some(2)

let squares := [1, 2, 3, 4].map(x => x * x).filter(x => x > 4).toList()
print(squares)         // [9, 16]

let lookup := Map().put("a", 1)
print(lookup.get("a")) // Some(1)
```

A program does not import this module; the compiler loads the prelude into every
file. `import std.core` only makes the names explicit.

## Data structures

| Type | Methods | Notes |
|---|---|---|
| `Stack[T]` | `push`, `pop`, `peek`, `size` | Last in, first out. |
| `Queue[T]` | `enqueue`, `dequeue`, `first`, `size` | First in, first out. |
| `Deque[T]` | `pushFront`, `pushBack`, `popFront`, `popBack`, `size`, `isEmpty` | Both ends. |
| `MinHeap[T]` | `push`, `pop`, `peek`, `size` | The smallest value comes out first. |
| `MaxHeap[T]` | `push`, `pop`, `peek`, `size` | The largest first. |
| `Counter[T]` | `add`, `addAll`, `count`, `distinctCount`, `mostCommon` | Counts occurrences. |
| `BitSet` | `add`, `remove`, `contains`, `size` | A set of small integers, stored as bits. |

`pop`, `peek` and `first` return `Option`, so an empty container is not a crash.

## List

| Method | Meaning |
|---|---|
| `take(n)`, `drop(n)` | A prefix, or the rest. |
| `takeWhile(pred)`, `dropWhile(pred)` | The same, driven by a predicate. |
| `concat(other)` | Both lists, in order. |
| `find(pred)`, `findIndex(pred)` | The first match, as `Option`. |
| `count(pred)` | How many values match. |
| `partition(pred)` | Two lists: matches and the rest. |
| `flatMap(f)` | Map, then flatten one level. |
| `distinct()` | Remove duplicates, keeping the first occurrence. |
| `chunks(n)`, `windows(n)` | Groups of `n`, without or with overlap. |
| `groupBy(key)` | A `Map` from key to the values with that key. |
| `scan(f, init)` | The running accumulation. |
| `sorted()`, `sortedBy(key)` | A sorted copy. |
| `binarySearch(value)` | The position in a sorted list. |

## Option

| Method | Meaning |
|---|---|
| `map(f)`, `flatMap(f)` | Transform the value, if there is one. |
| `filter(pred)` | Keep the value only when the predicate holds. |
| `unwrapOr(fallback)` | The value, or a fallback. |
| `unwrapOrElse(f)` | The value, or the result of calling `f`. |
| `orElse(other)` | This option, or another. |
| `okOr(error)` | A value or an `Error`. |

## Map and Set

| Type | Method | Meaning |
|---|---|---|
| `Map` | `get(key)` | `Option[V]`. |
| `Map` | `put(key, value)` | A new map with the pair. |
| `Map` | `keys()`, `values()`, `entries()` | The contents as lists. |
| `Map` | `keep(pred)`, `mapValues(f)` | A filtered or transformed copy. |
| `Map` | `merge(other)` | The pairs of both, the second winning. |
| `Set` | `add(value)`, `remove(value)` | A new set. |
| `Set` | `union(other)`, `intersection(other)`, `difference(other)` | Set algebra. |
| `Set` | `isSubset(other)`, `isDisjoint(other)` | Comparisons. |

## String

| Method | Meaning |
|---|---|
| `length()` | Characters, not bytes. |
| `byteLength()`, `bytes()` | The UTF-8 bytes. |
| `substring(start, end)` | By character position. |
| `slice(start, end)` | The same, with negative positions counted from the end. |
| `byteSlice(start, end)`, `byteAt(i)` | By byte position. |
| `chars()`, `graphemes()` | Characters, or what a reader sees as one character. |
| `graphemeLength()` | The number of visible characters. |
| `split(separator)`, `lines()`, `words()` | Splitting. |
| `padStart(n, fill)`, `padEnd(n, fill)` | Padding. |
| `reversed()`, `ord()` | A reversed copy, and the code point. |
| `nextCharBoundary(i)`, `nextGraphemeBoundary(i)` | The next safe cut. |

## Bytes

| Method | Meaning |
|---|---|
| `length()`, `isEmpty()` | Size. |
| `get(i)`, `at(i)` | One byte. |
| `slice(start, end)` | A range. |
| `concat(other)` | Both, in order. |
| `toList()` | The bytes as `Int`s. |
| `toHex()` | Hexadecimal text. |
| `decodeUtf8()` | Text, or an error. |

## Error

`Error` has `context()` and `cause()`, so a failure can be wrapped without losing
the original. `RuntimeError` is the standard set of runtime failures:
`RuntimeError.Io`, `RuntimeError.Python`, and the rest.

## Traits

| Trait | Meaning |
|---|---|
| `Equals` | `equals(other)`. |
| `Comparable` | An order. |
| `Iterable`, `Iterator` | What `for` uses. |
| `Text` | `text()` for printing. |
| `Multipliable`, `RightMultipliable` | `*` with a scalar on either side. |
| `Box` | Wraps a value that a heterogeneous list can hold. |

## Notes

- `Array` indexes with `a[i]`; `List` reads with `.at(i)` for the element types it
  supports, so a list of records is walked with `for`.
- The prelude is part of the compiler, not a file you install.
