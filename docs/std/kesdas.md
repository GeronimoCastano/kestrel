# std.kesdas

Series and DataFrames with a pandas-shaped API: columns by name, grouping,
joins, windows and missing values.

```kestrel
import std.kesdas

let sales := DataFrame([
    Series(["north", "south", "north"], name = "region"),
    Series([120, 80, 95], name = "units"),
    Series([2.5, 3.0, 2.5], name = "price"),
])

let revenue := sales.assign("revenue", sales.col("units")? * sales.col("price")?)
print(revenue.filter(revenue.col("revenue")?.gt(200.0)))
print(revenue.groupBy("region")?.sum())
```

A frame is built from named columns. Every operation returns a new frame or a
`Series`; nothing is changed in place unless the method says `mut`.

## Types

| Type | Holds |
|---|---|
| `Series` | One named column: numbers, text, booleans or missing values. |
| `DataFrame` | Named columns of the same length, plus their row order. |
| `GroupBy` | A frame split by one or more columns, ready to aggregate. |
| `Rolling`, `Expanding`, `EWM` | Window views over a series. |
| `FrameRolling`, `FrameExpanding`, `FrameEWM` | The same over a frame. |
| `StringMethods` | Text operations on a text series. |
| `DataError` | The error type these functions return. |

## Building

| Call | Meaning |
|---|---|
| `Series(values, name = "x")` | A column from a `List` of numbers, strings or booleans. |
| `DataFrame([Series(...), ...])` | A frame from named columns. |
| `series.name` | The column's name. |
| `frame.names` | The column names, in order. |

## Looking at a frame

| Method | Meaning |
|---|---|
| `frame.col("x")` | The column as a `Series`, or an error. |
| `frame.select(["a", "b"])` | A frame with those columns. |
| `frame.head(n)`, `frame.tail(n)` | The first or last rows. |
| `frame.length()`, `frame.shape()`, `frame.width` | Row count, shape, and column count. |
| `frame.describe()` | Count, mean, std, min, quartiles and max per numeric column. |
| `print(frame)` | Draws the frame as a table. |

## Changing a frame

| Method | Meaning |
|---|---|
| `frame.assign("x", series)` | A frame with a new or replaced column. |
| `frame.filter(series_of_bools)` | The rows where the condition holds. |
| `frame.sortValues("x", ascending = false)` | Rows ordered by a column. |
| `frame.drop(["x"])` | A frame without those columns. |
| `frame.renameColumns(mapping)` | A frame with columns renamed from an old-to-new `Map`. |

## Grouping and joining

| Method | Meaning |
|---|---|
| `frame.groupBy("x")` | A `GroupBy` over one or more columns. |
| `group.sum()`, `mean()`, `median()`, `min()`, `max()`, `count()` | Aggregations. |
| `frame.merge(other, on = "x", how = "left")` | A join on a column. |
| `frame.append(other)` | Rows of both frames, one after the other. |
| `frame.melt(idVars = [...], valueVars = [...])` | Long form. |
| `frame.pivot(index, columns, values)` | Wide form. |

```kestrel
let byRegion := revenue.groupBy("region")?.sum()
print(byRegion.sortValues("revenue", ascending = false)?)
```

## Windows

| Method | Meaning |
|---|---|
| `series.rolling(n)` | A window of the last `n` rows; `.mean()`, `.sum()`, `.std()` and others follow. |
| `series.expanding()` | A window from the first row to the current one. |
| `series.ewm(alpha)` | An exponentially weighted window. |
| `frame.rolling(n)` | The same over every numeric column. |

## Reading and writing

| Call | Meaning |
|---|---|
| `kesdas.readCsv(path, separator = ",", header = true, skipRows = 0, nRows = -1)` | A frame from a CSV file; it carries `Effects`. |
| `frame.toCsv(separator = ",", header = true, includeIndex = false)` | The frame as CSV text. |
| `series.toCsv(...)` | One column as CSV text. |

## Missing values

A missing cell is not a zero. Numeric columns hold a missing marker, and the
aggregations skip it: `sum`, `mean`, `count` and the rest behave as pandas does.
Use `frame.dropNa()` to remove rows with a missing cell, `series.fillNa(value)` to
replace them, or `interpolate`, `ffill` and `bfill` to fill from neighbours.

## Notes

- Columns are found by name, not by position.
- A frame stores each column's type. Mixing a number into a text column is an
  error rather than a silent conversion.
- Plotting lives in [`std.kesplot`](index.md#text-and-data), and reading typed
  frames from CSV is the job of `readCsv` here, not [`std.csv`](csv.md).
