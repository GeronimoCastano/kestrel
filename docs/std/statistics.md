# std.statistics

Summaries of a `List` of doubles. Tensors and data frames carry their own
statistics in [std.stats](index.md#numbers) and [std.kesdas](index.md#text-and-data).

```kestrel
import std.statistics

let xs := [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]
print(statistics.mean(xs)?)      // 5.0
print(statistics.stddev(xs)?)    // 2.1380899352993952
print(statistics.median(xs)?)    // 4.5
```

Every function takes the list first and returns `or Error` for an empty or too
short sample.

## Functions

### `mean(xs : List[double]) : double or Error`

The arithmetic mean. An empty list is an error.

| Argument | Meaning |
|---|---|
| `xs : List[double]` | The values. |

```kestrel
print(statistics.mean([1.0, 2.0, 3.0])?)   // 2.0
```

### `variance(xs : List[double]) : double or Error`

The sample variance, dividing by $n-1$. Two values are the minimum; one value is
an error.

| Argument | Meaning |
|---|---|
| `xs : List[double]` | The values. |

```kestrel
print(statistics.variance([1.0, 2.0, 3.0])?)   // 1.0
```

### `stddev(xs : List[double]) : double or Error`

The square root of `variance`, so it has the same units as the values.

| Argument | Meaning |
|---|---|
| `xs : List[double]` | The values. |

### `quantile(xs : List[double], q : double) : double or Error`

The value at fraction `q` of the sorted data, interpolating linearly between
neighbours as NumPy's default does. `q` runs from `0.0` to `1.0`; anything else
is an error.

| Argument | Meaning |
|---|---|
| `xs : List[double]` | The values. |
| `q : double` | Fraction of the way through the sorted data. |

```kestrel
print(statistics.quantile([1.0, 2.0, 3.0, 4.0], 0.25)?)   // 1.75
```

### `median(xs : List[double]) : double or Error`

`quantile(xs, 0.5)`: the middle value, or the mean of the two middle values.

| Argument | Meaning |
|---|---|
| `xs : List[double]` | The values. |

### `percentile(xs : List[double], p : double) : double or Error`

`quantile(xs, p / 100.0)`, so `p` runs from `0.0` to `100.0`.

| Argument | Meaning |
|---|---|
| `xs : List[double]` | The values. |
| `p : double` | Percentile, `0` to `100`. |

```kestrel
print(statistics.percentile(xs, 75.0)?)   // 5.5
```

### `mode(xs : List[double]) : double or Error`

The most frequent value. When several values tie, the smallest one wins. An empty
list is an error.

| Argument | Meaning |
|---|---|
| `xs : List[double]` | The values. |

```kestrel
print(statistics.mode([4.0, 1.0, 4.0, 2.0])?)   // 4.0
```

### `histogram(xs : List[double], bins : Int) : List[Int] or Error`

Counts of values in `bins` equal-width bins from the minimum to the maximum. The
maximum falls in the last bin.

| Argument | Meaning |
|---|---|
| `xs : List[double]` | The values. |
| `bins : Int` | How many bins, at least one. |

```kestrel
print(statistics.histogram(xs, 4))   // [1, 5, 1, 1]
```

When every value is equal, the width is `1.0` and every value lands in the first
bin.

## Notes

- All functions copy nothing and sort a copy when they need order.
- For data in a data frame, use its own `mean`, `std` and `quantile`, which skip
  missing values.
