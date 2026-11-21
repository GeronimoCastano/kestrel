# std.stats

Summaries of a tensor. The functions work on the entries as one sample, except
`covariance` and `correlation`, which treat the rows as observations.

```kestrel
import std.stats

let m := Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2))
print(stats.mean(m))       // 2.5
print(stats.variance(m))   // 1.25
print(stats.stddev(m))     // 1.1180339887498949
```

## Functions

### `mean(source : Tensor) : double`

The mean of every entry.

| Argument | Meaning |
|---|---|
| `source : Tensor` | The values. |

### `variance(source : Tensor) : double`

The population variance, dividing by $n$. For a sample variance, use
[`std.statistics`](statistics.md), which divides by $n-1$.

| Argument | Meaning |
|---|---|
| `source : Tensor` | The values. |

### `stddev(source : Tensor) : double`

The square root of `variance`.

| Argument | Meaning |
|---|---|
| `source : Tensor` | The values. |

### `covariance(source : Tensor) : Tensor`

The covariance matrix of a matrix whose rows are observations. The result is
`columns × columns` and uses $n$, the number of rows.

| Argument | Meaning |
|---|---|
| `source : Tensor` | A rank-2 tensor, one observation per row. |

```kestrel
let m := Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2))
print(stats.covariance(m))   // [[1.0, 1.0], [1.0, 1.0]]
```

A tensor that is not a matrix stops with a message that says observations go in
rows.

### `correlation(source : Tensor) : Tensor`

The correlation matrix of the same layout, with `1.0` on the diagonal. A column
with zero variance is refused.

| Argument | Meaning |
|---|---|
| `source : Tensor` | A rank-2 tensor, one observation per row. |

```kestrel
let m := Tensor([1.0, 1.0, 2.0, 2.0, 3.0, 4.0], shape = (3, 2))
print(stats.correlation(m))
```

## Notes

- `mean`, `variance` and `stddev` flatten the tensor; shape does not matter.
- `covariance` and `correlation` need rank 2.
- These are population statistics. [`std.statistics`](statistics.md) works on a
  `List` and gives the sample variance, and [`std.kesdas`](index.md#text-and-data)
  gives the pandas defaults on a frame.
