# std.kesdas reference

Part of [`std.kesdas`](kesdas.md); this page lists its members, the guide explains the workflow.

Every public member of `std.kesdas`, grouped by type. The signature column
names each argument with its type and its default; a method's receiver is implicit.

The path of every member is written from the package root: `kesdas.Series.mean`.
After `import std.kesdas` (or the shorter `import kesdas`) the namespace is
`kesdas`, so the call is `kesdas.mean(...)` for a module function and
`series.mean()` for a method.

Methods return new values, so a whole pipeline is one expression:

```kestrel
let top := sales.groupBy("region")?.sum().sortValues("revenue", ascending = false)?
let average := sales.col("units")?.rolling(3).mean()
let scaled := sales.select(["units", "price"])?.mapNumeric(x => x / 100.0)
```


### Types

- `kesdas.Column`: A typed column of a data frame, with its own missing markers.
- `kesdas.Scalar`: One typed value, possibly missing.
- `kesdas.Series`: A named column with the pandas-style operations.
- `kesdas.Rolling`: A rolling window over a series.
- `kesdas.Expanding`: A window from the first row to the current one.
- `kesdas.EWM`: An exponentially weighted window.
- `kesdas.StringMethods`: Text operations reached through a text series.
- `kesdas.DataError`: The error type the data-frame functions return.
- `kesdas.DataFrame`: Named columns of equal length, with grouping, joins and windows.
- `kesdas.FrameRolling`: A rolling window over every numeric column of a frame.
- `kesdas.FrameExpanding`: An expanding window over a frame.
- `kesdas.FrameEWM`: An exponentially weighted window over a frame.
- `kesdas.GroupBy`: A frame split by key columns, ready to aggregate.

### Functions

| Member | Signature | What it does |
|---|---|---|
| `kesdas.Column.ofInts` | `Column.ofInts(values : List[Int], missing : List[Bool]) : Column` | An integer column. |
| `kesdas.Column.ofFloats` | `Column.ofFloats(values : List[double], missing : List[Bool]) : Column` | A float column. |
| `kesdas.Column.ofBools` | `Column.ofBools(values : List[Bool], missing : List[Bool]) : Column` | A boolean column. |
| `kesdas.Column.ofStrings` | `Column.ofStrings(values : List[String], missing : List[Bool]) : Column` | A text column. |
| `kesdas.Column.noneMissing` | `Column.noneMissing(n : Int) : List[Bool]` | A missing mask that marks nothing missing. |
| `kesdas.Column.ids` | `Column.ids(count : Int) : Column` | A column of row numbers. |
| `kesdas.Column.ints` | `Column.ints(values : List[Int]) : Column` | An integer column from a list. |
| `kesdas.Column.floats` | `Column.floats(values : List[double]) : Column` | A float column from a list. |
| `kesdas.Column.bools` | `Column.bools(values : List[Bool]) : Column` | A boolean column from a list. |
| `kesdas.Column.strings` | `Column.strings(values : List[String]) : Column` | A text column from a list. |
| `kesdas.Column.empty` | `Column.empty(kind : Int, size : Int) : Column` | An empty value of this type. |
| `kesdas.Scalar.ofInt` | `Scalar.ofInt(value : Int) : Scalar` | An integer scalar. |
| `kesdas.Scalar.ofFloat` | `Scalar.ofFloat(value : double) : Scalar` | A float scalar. |
| `kesdas.Scalar.ofBool` | `Scalar.ofBool(value : Bool) : Scalar` | A boolean scalar. |
| `kesdas.Scalar.ofString` | `Scalar.ofString(value : String) : Scalar` | A text scalar. |
| `kesdas.Scalar.missing` | `Scalar.missing() : Scalar` | The positions that are missing. |
| `kesdas.Series.concat` | `Series.concat(parts : List[Series]) : Series` | Joins values end to end. |
| `kesdas.DataFrame.empty` | `DataFrame.empty() : DataFrame` | An empty value of this type. |
| `kesdas.DataFrame.concat` | `DataFrame.concat(frames : List[DataFrame]) : DataFrame` | Joins values end to end. |
| `kesdas.DataFrame.concatColumns` | `DataFrame.concatColumns(frames : List[DataFrame]) : DataFrame or Error` | Joins frames side by side. |
| `kesdas.DataFrame.parseCsv` | `DataFrame.parseCsv(text : String, separator : String, header : Bool, skipRows : Int, nRows : Int) : DataFrame` | A frame from CSV text. |
| `kesdas.DataFrame.parseCsvLines` | `DataFrame.parseCsvLines(lines : List[String], separator : String, header : Bool, skipRows : Int, nRows : Int) : DataFrame` | A frame from CSV lines. |
| `kesdas.readCsv` | `std.kesdas.readCsv(path : String, separator : String, header : Bool, skipRows : Int, nRows : Int) : DataFrame or Error with Effects` | A frame from a CSV file. |

### Column

| Member | Signature | What it does |
|---|---|---|
| `kesdas.Column.tile` | `Column.tile(times : Int) : Column` | The values repeated n times. |
| `kesdas.Column.concat` | `Column.concat(other : Column) : Column` | Joins values end to end. |
| `kesdas.Column.copy` | `Column.copy() : Column` | A value with the same contents. |
| `kesdas.Column.dtype` | `Column.dtype() : String` | The type of the values. |
| `kesdas.Column.isNumeric` | `Column.isNumeric() : Bool` | True for a numeric column. |
| `kesdas.Column.orderPositions` | `Column.orderPositions(start : List[Int], ascending : Bool, naFirst : Bool) : List[Int]` | The positions that sort the values. |
| `kesdas.Column.topPositions` | `Column.topPositions(n : Int, ascending : Bool) : List[Int]` | The positions of the largest values. |
| `kesdas.Column.gather` | `Column.gather(positions : List[Int]) : Column` | Picks the values at the given positions. |
| `kesdas.Column.head` | `Column.head(n : Int) : Column` | The first rows. |
| `kesdas.Column.join` | `Column.join(other : Column) : Column` | Joins two values together. |
| `kesdas.Column.toFloats` | `Column.toFloats() : List[double]` | The values as doubles. |
| `kesdas.Column.texts` | `Column.texts() : List[String]` | The text elements of the figure. |
| `kesdas.Column.keys` | `Column.keys() : List[String]` | The key values of a grouped frame. |

### Scalar

| Member | Signature | What it does |
|---|---|---|
| `kesdas.Scalar.isMissing` | `Scalar.isMissing() : Bool` | True when the value is missing. |
| `kesdas.Scalar.isNumeric` | `Scalar.isNumeric() : Bool` | True for a numeric column. |
| `kesdas.Scalar.asDouble` | `Scalar.asDouble() : double` | The value as a double. |
| `kesdas.Scalar.asInt` | `Scalar.asInt() : Int` | The value as an Int. |
| `kesdas.Scalar.asString` | `Scalar.asString() : String` | The value as a String. |
| `kesdas.Scalar.asBool` | `Scalar.asBool() : Bool` | The value as a Bool, when it is one. |
| `kesdas.Scalar.text` | `Scalar.text() : String` | The value as text. |
| `kesdas.Scalar.equals` | `Scalar.equals(other : Scalar) : Bool` | True when both values are equal. |

### Series

| Member | Signature | What it does |
|---|---|---|
| `kesdas.Series.copy` | `Series.copy() : Series` | A value with the same contents. |
| `kesdas.Series.length` | `Series.length() : Int` | How many values. |
| `kesdas.Series.size` | `Series.size() : Int` | How many values. |
| `kesdas.Series.ndim` | `Series.ndim() : Int` | The number of dimensions. |
| `kesdas.Series.isEmpty` | `Series.isEmpty() : Bool` | True when there are no values. |
| `kesdas.Series.dtype` | `Series.dtype() : String` | The type of the values. |
| `kesdas.Series.derive` | `Series.derive(data : Column) : Series` | A new series computed from this one. |
| `kesdas.Series.take` | `Series.take(positions : List[Int]) : Series` | The first values. |
| `kesdas.Series.toScalars` | `Series.toScalars() : List[Scalar]` | The values as scalars. |
| `kesdas.Series.items` | `Series.items() : List[(Scalar, Scalar)]` | The values with their labels. |
| `kesdas.Series.equals` | `Series.equals(other : Series) : Bool` | True when both values are equal. |
| `kesdas.Series.rename` | `Series.rename(name : String) : Series` | A copy with a value renamed. |
| `kesdas.Series.withIndex` | `Series.withIndex(labels : List[String]) : Series` | A copy with a new index. |
| `kesdas.Series.withIndex` | `Series.withIndex(labels : List[Int]) : Series` | A copy with a new index. |
| `kesdas.Series.withIndexColumn` | `Series.withIndexColumn(labels : Column) : Series` | A copy with a column as its index. |
| `kesdas.Series.named` | `Series.named(indexName : String) : Series` | A copy with a column name. |
| `kesdas.Series.toInts` | `Series.toInts() : List[Int]` | The values as integers. |
| `kesdas.Series.toFloats` | `Series.toFloats() : List[double]` | The values as doubles. |
| `kesdas.Series.toStrings` | `Series.toStrings() : List[String]` | The values as text. |
| `kesdas.Series.toBools` | `Series.toBools() : List[Bool]` | The values as booleans. |
| `kesdas.Series.indexLabels` | `Series.indexLabels() : List[String]` | The index labels. |
| `kesdas.Series.missingMask` | `Series.missingMask() : List[Bool]` | A mask that marks the missing values. |
| `kesdas.Series.astype` | `Series.astype(dtype : String) : Series` | Converts the values to another type. |
| `kesdas.Series.head` | `Series.head(n : Int) : Series` | The first rows. |
| `kesdas.Series.tail` | `Series.tail(n : Int) : Series` | The last rows. |
| `kesdas.Series.slice` | `Series.slice(start : Int, stop : Int) : Series` | A contiguous part. |
| `kesdas.Series.iloc` | `Series.iloc(position : Int) : Scalar` | A row by position. |
| `kesdas.Series.iloc` | `Series.iloc(positions : List[Int]) : Series` | A row by position. |
| `kesdas.Series.iat` | `Series.iat(position : Int) : Scalar` | The value at a row and column position. |
| `kesdas.Series.takeGaps` | `Series.takeGaps(positions : List[Int]) : Series` | Keeps the values where the mask is true. |
| `kesdas.Series.positionOf` | `Series.positionOf(label : Int) : Option[Int]` | The position of a value. |
| `kesdas.Series.positionOf` | `Series.positionOf(label : String) : Option[Int]` | The position of a value. |
| `kesdas.Series.positionOf` | `Series.positionOf(label : double) : Option[Int]` | The position of a value. |
| `kesdas.Series.positionOf` | `Series.positionOf(label : Bool) : Option[Int]` | The position of a value. |
| `kesdas.Series.loc` | `Series.loc(label : Int) : Scalar` | A row or column by label. |
| `kesdas.Series.loc` | `Series.loc(label : String) : Scalar` | A row or column by label. |
| `kesdas.Series.loc` | `Series.loc(label : double) : Scalar` | A row or column by label. |
| `kesdas.Series.loc` | `Series.loc(label : Bool) : Scalar` | A row or column by label. |
| `kesdas.Series.at` | `Series.at(label : Int) : Scalar` | The value at a position or label. |
| `kesdas.Series.at` | `Series.at(label : String) : Scalar` | The value at a position or label. |
| `kesdas.Series.at` | `Series.at(label : double) : Scalar` | The value at a position or label. |
| `kesdas.Series.at` | `Series.at(label : Bool) : Scalar` | The value at a position or label. |
| `kesdas.Series.filter` | `Series.filter(mask : Series) : Series` | The values whose mask is true. |
| `kesdas.Series.where` | `Series.where(mask : Series, other : Scalar) : Series` | The values kept where a condition holds. |
| `kesdas.Series.where` | `Series.where(mask : Series, other : Series) : Series` | The values kept where a condition holds. |
| `kesdas.Series.mask` | `Series.mask(mask : Series, other : Scalar) : Series` | Replaces the values where a condition holds. |
| `kesdas.Series.sortValues` | `Series.sortValues(ascending : Bool, naPosition : String) : Series` | A frame sorted by a column. |
| `kesdas.Series.sortIndex` | `Series.sortIndex(ascending : Bool, naPosition : String) : Series` | A frame sorted by its index. |
| `kesdas.Series.nlargest` | `Series.nlargest(n : Int) : Series` | The n largest values. |
| `kesdas.Series.nsmallest` | `Series.nsmallest(n : Int) : Series` | The n smallest values. |
| `kesdas.Series.resetIndex` | `Series.resetIndex() : Series` | A frame with a fresh row index. |
| `kesdas.Series.add` | `Series.add(other : Series) : Series` | Elementwise addition. |
| `kesdas.Series.subtract` | `Series.subtract(other : Series) : Series` | Elementwise subtraction. |
| `kesdas.Series.multiply` | `Series.multiply(other : Series) : Series` | Elementwise multiplication. |
| `kesdas.Series.divide` | `Series.divide(other : Series) : Series` | Elementwise division. |
| `kesdas.Series.floorDivide` | `Series.floorDivide(other : Series) : Series` | Elementwise floor division. |
| `kesdas.Series.mod` | `Series.mod(other : Series) : Series` | Elementwise remainder. |
| `kesdas.Series.pow` | `Series.pow(other : Series) : Series` | Elementwise power. |
| `kesdas.Series.add` | `Series.add(other : double) : Series` | Elementwise addition. |
| `kesdas.Series.subtract` | `Series.subtract(other : double) : Series` | Elementwise subtraction. |
| `kesdas.Series.multiply` | `Series.multiply(other : double) : Series` | Elementwise multiplication. |
| `kesdas.Series.divide` | `Series.divide(other : double) : Series` | Elementwise division. |
| `kesdas.Series.floorDivide` | `Series.floorDivide(other : double) : Series` | Elementwise floor division. |
| `kesdas.Series.mod` | `Series.mod(other : double) : Series` | Elementwise remainder. |
| `kesdas.Series.pow` | `Series.pow(other : double) : Series` | Elementwise power. |
| `kesdas.Series.add` | `Series.add(other : Int) : Series` | Elementwise addition. |
| `kesdas.Series.subtract` | `Series.subtract(other : Int) : Series` | Elementwise subtraction. |
| `kesdas.Series.multiply` | `Series.multiply(other : Int) : Series` | Elementwise multiplication. |
| `kesdas.Series.divide` | `Series.divide(other : Int) : Series` | Elementwise division. |
| `kesdas.Series.floorDivide` | `Series.floorDivide(other : Int) : Series` | Elementwise floor division. |
| `kesdas.Series.mod` | `Series.mod(other : Int) : Series` | Elementwise remainder. |
| `kesdas.Series.pow` | `Series.pow(other : Int) : Series` | Elementwise power. |
| `kesdas.Series.add` | `Series.add(other : String) : Series` | Elementwise addition. |
| `kesdas.Series.rightAdd` | `Series.rightAdd(other : double) : Series` | The value added to an argument on its right. |
| `kesdas.Series.rightSubtract` | `Series.rightSubtract(other : double) : Series` | An argument on the right minus the value. |
| `kesdas.Series.rightMultiply` | `Series.rightMultiply(other : double) : Series` | The value multiplied by an argument on its right. |
| `kesdas.Series.rightDivide` | `Series.rightDivide(other : double) : Series` | An argument on the right divided by the value. |
| `kesdas.Series.rightAdd` | `Series.rightAdd(other : Int) : Series` | The value added to an argument on its right. |
| `kesdas.Series.rightSubtract` | `Series.rightSubtract(other : Int) : Series` | An argument on the right minus the value. |
| `kesdas.Series.rightMultiply` | `Series.rightMultiply(other : Int) : Series` | The value multiplied by an argument on its right. |
| `kesdas.Series.rightDivide` | `Series.rightDivide(other : Int) : Series` | An argument on the right divided by the value. |
| `kesdas.Series.negate` | `Series.negate() : Series` | Elementwise negation. |
| `kesdas.Series.abs` | `Series.abs() : Series` | The absolute value. |
| `kesdas.Series.round` | `Series.round(decimals : Int) : Series` | Rounds each value. |
| `kesdas.Series.sqrt` | `Series.sqrt() : Series` | The square root. |
| `kesdas.Series.clip` | `Series.clip(low : double, high : double) : Series` | Limits values to a range. |
| `kesdas.Series.clip` | `Series.clip(low : Int, high : Int) : Series` | Limits values to a range. |
| `kesdas.Series.item` | `Series.item(position : Int) : Scalar` | The single value. |
| `kesdas.Series.isUnique` | `Series.isUnique() : Bool` | True when every value appears once. |
| `kesdas.Series.isMonotonic` | `Series.isMonotonic(increasing : Bool) : Bool` | True when the values never go back. |
| `kesdas.Series.repeat` | `Series.repeat(times : Int) : Series` | The value repeated n times. |
| `kesdas.Series.eq` | `Series.eq(other : Series) : Series` | Elementwise equality. |
| `kesdas.Series.ne` | `Series.ne(other : Series) : Series` | Elementwise inequality. |
| `kesdas.Series.lt` | `Series.lt(other : Series) : Series` | Elementwise less-than. |
| `kesdas.Series.le` | `Series.le(other : Series) : Series` | Elementwise less-or-equal. |
| `kesdas.Series.gt` | `Series.gt(other : Series) : Series` | Elementwise greater-than. |
| `kesdas.Series.ge` | `Series.ge(other : Series) : Series` | Elementwise greater-or-equal. |
| `kesdas.Series.eq` | `Series.eq(other : double) : Series` | Elementwise equality. |
| `kesdas.Series.ne` | `Series.ne(other : double) : Series` | Elementwise inequality. |
| `kesdas.Series.lt` | `Series.lt(other : double) : Series` | Elementwise less-than. |
| `kesdas.Series.le` | `Series.le(other : double) : Series` | Elementwise less-or-equal. |
| `kesdas.Series.gt` | `Series.gt(other : double) : Series` | Elementwise greater-than. |
| `kesdas.Series.ge` | `Series.ge(other : double) : Series` | Elementwise greater-or-equal. |
| `kesdas.Series.eq` | `Series.eq(other : Int) : Series` | Elementwise equality. |
| `kesdas.Series.ne` | `Series.ne(other : Int) : Series` | Elementwise inequality. |
| `kesdas.Series.lt` | `Series.lt(other : Int) : Series` | Elementwise less-than. |
| `kesdas.Series.le` | `Series.le(other : Int) : Series` | Elementwise less-or-equal. |
| `kesdas.Series.gt` | `Series.gt(other : Int) : Series` | Elementwise greater-than. |
| `kesdas.Series.ge` | `Series.ge(other : Int) : Series` | Elementwise greater-or-equal. |
| `kesdas.Series.eq` | `Series.eq(other : String) : Series` | Elementwise equality. |
| `kesdas.Series.ne` | `Series.ne(other : String) : Series` | Elementwise inequality. |
| `kesdas.Series.lt` | `Series.lt(other : String) : Series` | Elementwise less-than. |
| `kesdas.Series.le` | `Series.le(other : String) : Series` | Elementwise less-or-equal. |
| `kesdas.Series.gt` | `Series.gt(other : String) : Series` | Elementwise greater-than. |
| `kesdas.Series.ge` | `Series.ge(other : String) : Series` | Elementwise greater-or-equal. |
| `kesdas.Series.eq` | `Series.eq(other : Bool) : Series` | Elementwise equality. |
| `kesdas.Series.ne` | `Series.ne(other : Bool) : Series` | Elementwise inequality. |
| `kesdas.Series.both` | `Series.both(other : Series) : Series` | A pair of series or columns. |
| `kesdas.Series.either` | `Series.either(other : Series) : Series` | The union of two boolean masks. |
| `kesdas.Series.invert` | `Series.invert() : Series` | The complement of a boolean mask. |
| `kesdas.Series.between` | `Series.between(low : double, high : double) : Series` | True where the value lies in a range. |
| `kesdas.Series.between` | `Series.between(low : Int, high : Int) : Series` | True where the value lies in a range. |
| `kesdas.Series.between` | `Series.between(low : String, high : String) : Series` | True where the value lies in a range. |
| `kesdas.Series.isIn` | `Series.isIn(values : List[Int]) : Series` | True where the value is in a collection. |
| `kesdas.Series.isIn` | `Series.isIn(values : List[double]) : Series` | True where the value is in a collection. |
| `kesdas.Series.isIn` | `Series.isIn(values : List[String]) : Series` | True where the value is in a collection. |
| `kesdas.Series.isIn` | `Series.isIn(values : List[Bool]) : Series` | True where the value is in a collection. |
| `kesdas.Series.isNa` | `Series.isNa() : Series` | True where the value is missing. |
| `kesdas.Series.notNa` | `Series.notNa() : Series` | True where the value is present. |
| `kesdas.Series.fillNa` | `Series.fillNa(value : double) : Series` | Replaces missing values. |
| `kesdas.Series.fillNa` | `Series.fillNa(value : Int) : Series` | Replaces missing values. |
| `kesdas.Series.fillNa` | `Series.fillNa(value : String) : Series` | Replaces missing values. |
| `kesdas.Series.fillNa` | `Series.fillNa(value : Bool) : Series` | Replaces missing values. |
| `kesdas.Series.fillNa` | `Series.fillNa(value : Scalar) : Series` | Replaces missing values. |
| `kesdas.Series.dropNa` | `Series.dropNa() : Series` | A frame without rows that hold a missing value. |
| `kesdas.Series.ffill` | `Series.ffill() : Series` | Fills a missing value from the previous one. |
| `kesdas.Series.bfill` | `Series.bfill() : Series` | Fills a missing value from the next one. |
| `kesdas.Series.interpolate` | `Series.interpolate() : Series` | Fills missing values from their neighbours. |
| `kesdas.Series.count` | `Series.count() : Int` | The number of non-missing values. |
| `kesdas.Series.sum` | `Series.sum() : Scalar` | The sum. |
| `kesdas.Series.min` | `Series.min() : Scalar` | The smallest value. |
| `kesdas.Series.max` | `Series.max() : Scalar` | The largest value. |
| `kesdas.Series.mean` | `Series.mean() : Scalar` | The mean. |
| `kesdas.Series.median` | `Series.median() : Scalar` | The median. |
| `kesdas.Series.quantile` | `Series.quantile(q : double) : Scalar` | The value at a fraction of the sorted data. |
| `kesdas.Series.quantile` | `Series.quantile(qs : List[double]) : Series` | The value at a fraction of the sorted data. |
| `kesdas.Series.std` | `Series.std(ddof : Int) : Scalar` | The standard deviation. |
| `kesdas.Series.variance` | `Series.variance(ddof : Int) : Scalar` | The variance. |
| `kesdas.Series.sem` | `Series.sem(ddof : Int) : Scalar` | The standard error of the mean. |
| `kesdas.Series.skew` | `Series.skew() : Scalar` | The skewness. |
| `kesdas.Series.kurt` | `Series.kurt() : Scalar` | The kurtosis. |
| `kesdas.Series.prod` | `Series.prod() : Scalar` | The product. |
| `kesdas.Series.any` | `Series.any() : Bool` | True when at least one value satisfies the predicate. |
| `kesdas.Series.all` | `Series.all() : Bool` | True when every value satisfies the predicate. |
| `kesdas.Series.nunique` | `Series.nunique() : Int` | How many distinct values. |
| `kesdas.Series.idxMax` | `Series.idxMax() : Scalar` | The position of the largest value. |
| `kesdas.Series.idxMin` | `Series.idxMin() : Scalar` | The position of the smallest value. |
| `kesdas.Series.argMax` | `Series.argMax() : Int` | The position of the largest value. |
| `kesdas.Series.argMin` | `Series.argMin() : Int` | The position of the smallest value. |
| `kesdas.Series.unique` | `Series.unique() : Series` | The distinct values. |
| `kesdas.Series.duplicated` | `Series.duplicated(keep : String) : Series` | True where a row repeats an earlier one. |
| `kesdas.Series.dropDuplicates` | `Series.dropDuplicates(keep : String) : Series` | A frame without repeated rows. |
| `kesdas.Series.valueCounts` | `Series.valueCounts(ascending : Bool, normalize : Bool, sort : Bool, dropNa : Bool) : Series` | How often each distinct value appears. |
| `kesdas.Series.mode` | `Series.mode() : Series` | The most frequent value. |
| `kesdas.Series.cumSum` | `Series.cumSum() : Series` | The running sum. |
| `kesdas.Series.cumProd` | `Series.cumProd() : Series` | The running product. |
| `kesdas.Series.cumMax` | `Series.cumMax() : Series` | The running maximum. |
| `kesdas.Series.cumMin` | `Series.cumMin() : Series` | The running minimum. |
| `kesdas.Series.shift` | `Series.shift(periods : Int) : Series` | The values moved by n positions. |
| `kesdas.Series.diff` | `Series.diff(periods : Int) : Series` | The difference between consecutive values. |
| `kesdas.Series.pctChange` | `Series.pctChange(periods : Int) : Series` | The fractional change between consecutive values. |
| `kesdas.Series.rolling` | `Series.rolling(window : Int, minPeriods : Int) : Rolling` | A rolling window over the series. |
| `kesdas.Series.expanding` | `Series.expanding(minPeriods : Int) : Expanding` | A window from the first row to the current one. |
| `kesdas.Series.ewm` | `Series.ewm(alpha : double, adjust : Bool, minPeriods : Int) : EWM` | An exponentially weighted window. |
| `kesdas.Series.ewmSpan` | `Series.ewmSpan(span : Int, adjust : Bool, minPeriods : Int) : EWM` | An exponentially weighted window by span. |
| `kesdas.Series.ewmHalfLife` | `Series.ewmHalfLife(halflife : double, adjust : Bool, minPeriods : Int) : EWM` | An exponentially weighted window by half-life. |
| `kesdas.Series.rank` | `Series.rank(ascending : Bool, method : String, pct : Bool) : Series` | The rank of each value. |
| `kesdas.Series.apply` | `Series.apply(f : double -> double) : Series` | Applies a function to every value. |
| `kesdas.Series.applyInt` | `Series.applyInt(f : Int -> Int) : Series` | Applies a function to an integer series. |
| `kesdas.Series.applyString` | `Series.applyString(f : String -> String) : Series` | Applies a function to a text series. |
| `kesdas.Series.applyBool` | `Series.applyBool(f : double -> Bool) : Series` | Applies a function to a boolean series. |
| `kesdas.Series.map` | `Series.map(f : Scalar -> Scalar) : Series` | Applies a function to every value. |
| `kesdas.Series.replace` | `Series.replace(old : String, replacement : String) : Series` | Values replaced. |
| `kesdas.Series.replace` | `Series.replace(old : Int, replacement : Int) : Series` | Values replaced. |
| `kesdas.Series.replace` | `Series.replace(old : double, replacement : double) : Series` | Values replaced. |
| `kesdas.Series.replace` | `Series.replace(old : Bool, replacement : Bool) : Series` | Values replaced. |
| `kesdas.Series.replace` | `Series.replace(old : Scalar, replacement : Scalar) : Series` | Values replaced. |
| `kesdas.Series.replaceMany` | `Series.replaceMany(pairs : List[(Scalar, Scalar)]) : Series` | Several replacements in one pass. |
| `kesdas.Series.str` | `Series.str() : StringMethods` | The values as text. |
| `kesdas.Series.mapValues` | `Series.mapValues(mapping : Map[String, String]) : Series` | Transforms the values of a map or frame. |
| `kesdas.Series.splitToFrame` | `Series.splitToFrame(separator : String) : DataFrame` | A frame from the parts of a split. |
| `kesdas.Series.cov` | `Series.cov(other : Series, ddof : Int) : Scalar` | The covariance with another series. |
| `kesdas.Series.corr` | `Series.corr(other : Series) : Scalar` | The correlation with another series. |
| `kesdas.Series.autocorr` | `Series.autocorr(lag : Int) : Scalar` | The autocorrelation at a lag. |
| `kesdas.Series.factorize` | `Series.factorize() : (Series, Series)` | Maps each distinct value to an integer code. |
| `kesdas.Series.toCsv` | `Series.toCsv(separator : String, header : Bool, includeIndex : Bool) : String` | The frame as CSV text. |
| `kesdas.Series.toTensor` | `Series.toTensor() : DynTensor[double]` | The values as a tensor. |
| `kesdas.Series.append` | `Series.append(other : Series) : Series` | Appends the rows of another frame. |
| `kesdas.Series.combineFirst` | `Series.combineFirst(other : Series) : Series` | Fills missing values from another series. |
| `kesdas.Series.update` | `Series.update(other : Series) : Series` | Updates the values in place. |
| `kesdas.Series.describe` | `Series.describe() : Series` | Summary statistics per column. |
| `kesdas.Series.toFrame` | `Series.toFrame() : DataFrame` | A frame from the values. |
| `kesdas.Series.text` | `Series.text() : String` | The value as text. |
| `kesdas.Series.html` | `Series.html() : String` | The frame as an HTML table. |

### Rolling

| Member | Signature | What it does |
|---|---|---|
| `kesdas.Rolling.mean` | `Rolling.mean() : Series` | The mean. |
| `kesdas.Rolling.sum` | `Rolling.sum() : Series` | The sum. |
| `kesdas.Rolling.min` | `Rolling.min() : Series` | The smallest value. |
| `kesdas.Rolling.max` | `Rolling.max() : Series` | The largest value. |
| `kesdas.Rolling.std` | `Rolling.std() : Series` | The standard deviation. |
| `kesdas.Rolling.variance` | `Rolling.variance() : Series` | The variance. |
| `kesdas.Rolling.median` | `Rolling.median() : Series` | The median. |
| `kesdas.Rolling.count` | `Rolling.count() : Series` | The number of non-missing values. |
| `kesdas.Rolling.sem` | `Rolling.sem() : Series` | The standard error of the mean. |
| `kesdas.Rolling.skew` | `Rolling.skew() : Series` | The skewness. |
| `kesdas.Rolling.kurt` | `Rolling.kurt() : Series` | The kurtosis. |
| `kesdas.Rolling.quantile` | `Rolling.quantile(q : double) : Series` | The value at a fraction of the sorted data. |
| `kesdas.Rolling.apply` | `Rolling.apply(f : List[double] -> double) : Series` | Applies a function to every value. |

### Expanding

| Member | Signature | What it does |
|---|---|---|
| `kesdas.Expanding.mean` | `Expanding.mean() : Series` | The mean. |
| `kesdas.Expanding.sum` | `Expanding.sum() : Series` | The sum. |
| `kesdas.Expanding.min` | `Expanding.min() : Series` | The smallest value. |
| `kesdas.Expanding.max` | `Expanding.max() : Series` | The largest value. |
| `kesdas.Expanding.std` | `Expanding.std() : Series` | The standard deviation. |
| `kesdas.Expanding.variance` | `Expanding.variance() : Series` | The variance. |
| `kesdas.Expanding.median` | `Expanding.median() : Series` | The median. |
| `kesdas.Expanding.count` | `Expanding.count() : Series` | The number of non-missing values. |
| `kesdas.Expanding.sem` | `Expanding.sem() : Series` | The standard error of the mean. |
| `kesdas.Expanding.quantile` | `Expanding.quantile(q : double) : Series` | The value at a fraction of the sorted data. |
| `kesdas.Expanding.apply` | `Expanding.apply(f : List[double] -> double) : Series` | Applies a function to every value. |

### EWM

| Member | Signature | What it does |
|---|---|---|
| `kesdas.EWM.mean` | `EWM.mean() : Series` | The mean. |
| `kesdas.EWM.sum` | `EWM.sum() : Series` | The sum. |
| `kesdas.EWM.variance` | `EWM.variance(bias : Bool) : Series` | The variance. |
| `kesdas.EWM.std` | `EWM.std(bias : Bool) : Series` | The standard deviation. |
| `kesdas.EWM.count` | `EWM.count() : Series` | The number of non-missing values. |

### StringMethods

| Member | Signature | What it does |
|---|---|---|
| `kesdas.StringMethods.upper` | `StringMethods.upper() : Series` | The text in upper case. |
| `kesdas.StringMethods.lower` | `StringMethods.lower() : Series` | The lower-triangular part. |
| `kesdas.StringMethods.swapcase` | `StringMethods.swapcase() : Series` | The text with the case of each character swapped. |
| `kesdas.StringMethods.strip` | `StringMethods.strip() : Series` | Removes whitespace at both ends. |
| `kesdas.StringMethods.stripLeading` | `StringMethods.stripLeading() : Series` | Removes whitespace at the start. |
| `kesdas.StringMethods.stripTrailing` | `StringMethods.stripTrailing() : Series` | Removes whitespace at the end. |
| `kesdas.StringMethods.replace` | `StringMethods.replace(old : String, replacement : String) : Series` | Values replaced. |
| `kesdas.StringMethods.capitalize` | `StringMethods.capitalize() : Series` | The text with its first character upper-cased. |
| `kesdas.StringMethods.title` | `StringMethods.title() : Series` | The text in title case. |
| `kesdas.StringMethods.slice` | `StringMethods.slice(start : Int, stop : Int) : Series` | A contiguous part. |
| `kesdas.StringMethods.sliceReplace` | `StringMethods.sliceReplace(start : Int, stop : Int, replacement : String) : Series` | A copy with a range replaced. |
| `kesdas.StringMethods.cat` | `StringMethods.cat(other : String) : Series` | Joins variables along an axis. |
| `kesdas.StringMethods.repeat` | `StringMethods.repeat(times : Int) : Series` | The value repeated n times. |
| `kesdas.StringMethods.pad` | `StringMethods.pad(width : Int) : Series` | Pads a value to a length. |
| `kesdas.StringMethods.ljust` | `StringMethods.ljust(width : Int) : Series` | Pads the text on the right. |
| `kesdas.StringMethods.rjust` | `StringMethods.rjust(width : Int) : Series` | Pads the text on the left. |
| `kesdas.StringMethods.center` | `StringMethods.center(width : Int) : Series` | Centres a layer or an axis. |
| `kesdas.StringMethods.zfill` | `StringMethods.zfill(width : Int) : Series` | Pads the number with leading zeros. |
| `kesdas.StringMethods.contains` | `StringMethods.contains(part : String) : Series` | True where the text contains a part. |
| `kesdas.StringMethods.startsWith` | `StringMethods.startsWith(part : String) : Series` | True where the text starts with a part. |
| `kesdas.StringMethods.endsWith` | `StringMethods.endsWith(part : String) : Series` | True where the text ends with a part. |
| `kesdas.StringMethods.isEmpty` | `StringMethods.isEmpty() : Series` | True when there are no values. |
| `kesdas.StringMethods.len` | `StringMethods.len() : Series` | How many values. |
| `kesdas.StringMethods.count` | `StringMethods.count(part : String) : Series` | How often a part appears in the text. |
| `kesdas.StringMethods.find` | `StringMethods.find(part : String) : Series` | The first position where the predicate holds. |
| `kesdas.StringMethods.rfind` | `StringMethods.rfind(part : String) : Series` | The last position of a part. |
| `kesdas.StringMethods.get` | `StringMethods.get(position : Int) : Series` | The value at a key or label. |
| `kesdas.StringMethods.splitPart` | `StringMethods.splitPart(separator : String, position : Int) : Series` | One part of a split. |

### DataFrame

| Member | Signature | What it does |
|---|---|---|
| `kesdas.DataFrame.length` | `DataFrame.length() : Int` | How many values. |
| `kesdas.DataFrame.width` | `DataFrame.width() : Int` | How many columns. |
| `kesdas.DataFrame.shape` | `DataFrame.shape() : (Int, Int)` | The shape. |
| `kesdas.DataFrame.isEmpty` | `DataFrame.isEmpty() : Bool` | True when there are no values. |
| `kesdas.DataFrame.columns` | `DataFrame.columns() : List[String]` | The column names. |
| `kesdas.DataFrame.hasColumn` | `DataFrame.hasColumn(name : String) : Bool` | True when the column exists. |
| `kesdas.DataFrame.ndim` | `DataFrame.ndim() : Int` | The number of dimensions. |
| `kesdas.DataFrame.size` | `DataFrame.size() : Int` | How many values. |
| `kesdas.DataFrame.equals` | `DataFrame.equals(other : DataFrame) : Bool` | True when both values are equal. |
| `kesdas.DataFrame.copy` | `DataFrame.copy() : DataFrame` | A value with the same contents. |
| `kesdas.DataFrame.dtypes` | `DataFrame.dtypes() : Series` | The type of each column. |
| `kesdas.DataFrame.keyColumn` | `DataFrame.keyColumn(name : String) : Column` | The column a group is keyed by. |
| `kesdas.DataFrame.keyCodes` | `DataFrame.keyCodes(name : String) : (List[Int], Int)` | The integer code of each key. |
| `kesdas.DataFrame.col` | `DataFrame.col(name : String) : Series or Error` | One column as a series. |
| `kesdas.DataFrame.get` | `DataFrame.get(name : String) : Option[Series]` | The value at a key or label. |
| `kesdas.DataFrame.items` | `DataFrame.items() : List[(String, Series)]` | The values with their labels. |
| `kesdas.DataFrame.select` | `DataFrame.select(selected : List[String]) : DataFrame or Error` | A frame with the chosen columns. |
| `kesdas.DataFrame.take` | `DataFrame.take(positions : List[Int]) : DataFrame` | The first values. |
| `kesdas.DataFrame.head` | `DataFrame.head(n : Int) : DataFrame` | The first rows. |
| `kesdas.DataFrame.tail` | `DataFrame.tail(n : Int) : DataFrame` | The last rows. |
| `kesdas.DataFrame.slice` | `DataFrame.slice(start : Int, stop : Int) : DataFrame` | A contiguous part. |
| `kesdas.DataFrame.iloc` | `DataFrame.iloc(positions : List[Int]) : DataFrame` | A row by position. |
| `kesdas.DataFrame.takeGaps` | `DataFrame.takeGaps(positions : List[Int]) : DataFrame` | Keeps the values where the mask is true. |
| `kesdas.DataFrame.iloc` | `DataFrame.iloc(position : Int) : Series` | A row by position. |
| `kesdas.DataFrame.row` | `DataFrame.row(position : Int) : Series` | One row of a frame. |
| `kesdas.DataFrame.at` | `DataFrame.at(row : Int, column : String) : Scalar or Error` | The value at a position or label. |
| `kesdas.DataFrame.iat` | `DataFrame.iat(row : Int, column : Int) : Scalar or Error` | The value at a row and column position. |
| `kesdas.DataFrame.iterrows` | `DataFrame.iterrows() : List[(Scalar, Series)]` | Iterates the rows as pairs. |
| `kesdas.DataFrame.filter` | `DataFrame.filter(mask : Series) : DataFrame` | The rows whose mask is true. |
| `kesdas.DataFrame.loc` | `DataFrame.loc(label : Int) : DataFrame` | A row or column by label. |
| `kesdas.DataFrame.loc` | `DataFrame.loc(label : String) : DataFrame` | A row or column by label. |
| `kesdas.DataFrame.sortValues` | `DataFrame.sortValues(by : String, ascending : Bool, naPosition : String) : DataFrame or Error` | A frame sorted by a column. |
| `kesdas.DataFrame.sortValues` | `DataFrame.sortValues(by : List[String], ascending : Bool, naPosition : String) : DataFrame or Error` | A frame sorted by a column. |
| `kesdas.DataFrame.sortValues` | `DataFrame.sortValues(by : List[String], ascending : List[Bool], naPosition : String) : DataFrame or Error` | A frame sorted by a column. |
| `kesdas.DataFrame.sortIndex` | `DataFrame.sortIndex(ascending : Bool, naPosition : String) : DataFrame` | A frame sorted by its index. |
| `kesdas.DataFrame.nlargest` | `DataFrame.nlargest(n : Int, by : String) : DataFrame or Error` | The n largest values. |
| `kesdas.DataFrame.nsmallest` | `DataFrame.nsmallest(n : Int, by : String) : DataFrame or Error` | The n smallest values. |
| `kesdas.DataFrame.drop` | `DataFrame.drop(column : String) : DataFrame` | A frame without the named columns. |
| `kesdas.DataFrame.drop` | `DataFrame.drop(removed : List[String]) : DataFrame` | A frame without the named columns. |
| `kesdas.DataFrame.dropRows` | `DataFrame.dropRows(positions : List[Int]) : DataFrame` | A frame without the rows at the given positions. |
| `kesdas.DataFrame.rename` | `DataFrame.rename(old : String, replacement : String) : DataFrame` | A copy with a value renamed. |
| `kesdas.DataFrame.renameColumns` | `DataFrame.renameColumns(mapping : Map[String, String]) : DataFrame` | A copy with columns renamed from a map. |
| `kesdas.DataFrame.assign` | `DataFrame.assign(name : String, series : Series) : DataFrame` | Adds or replaces a column. |
| `kesdas.DataFrame.assign` | `DataFrame.assign(name : String, values : List[Int]) : DataFrame` | Adds or replaces a column. |
| `kesdas.DataFrame.assign` | `DataFrame.assign(name : String, values : List[double]) : DataFrame` | Adds or replaces a column. |
| `kesdas.DataFrame.assign` | `DataFrame.assign(name : String, values : List[String]) : DataFrame` | Adds or replaces a column. |
| `kesdas.DataFrame.assign` | `DataFrame.assign(name : String, values : List[Bool]) : DataFrame` | Adds or replaces a column. |
| `kesdas.DataFrame.assign` | `DataFrame.assign(name : String, value : Int) : DataFrame` | Adds or replaces a column. |
| `kesdas.DataFrame.assign` | `DataFrame.assign(name : String, value : double) : DataFrame` | Adds or replaces a column. |
| `kesdas.DataFrame.assign` | `DataFrame.assign(name : String, value : String) : DataFrame` | Adds or replaces a column. |
| `kesdas.DataFrame.assign` | `DataFrame.assign(name : String, value : Bool) : DataFrame` | Adds or replaces a column. |
| `kesdas.DataFrame.setIndex` | `DataFrame.setIndex(name : String) : DataFrame or Error` | A frame with a new index. |
| `kesdas.DataFrame.resetIndex` | `DataFrame.resetIndex(drop : Bool) : DataFrame` | A frame with a fresh row index. |
| `kesdas.DataFrame.astype` | `DataFrame.astype(dtype : String) : DataFrame` | Converts the values to another type. |
| `kesdas.DataFrame.selectDtypes` | `DataFrame.selectDtypes(dtype : String) : DataFrame` | The columns of the given types. |
| `kesdas.DataFrame.round` | `DataFrame.round(decimals : Int) : DataFrame` | Rounds each value. |
| `kesdas.DataFrame.abs` | `DataFrame.abs() : DataFrame` | The absolute value. |
| `kesdas.DataFrame.clip` | `DataFrame.clip(low : double, high : double) : DataFrame` | Limits values to a range. |
| `kesdas.DataFrame.shift` | `DataFrame.shift(periods : Int) : DataFrame` | The values moved by n positions. |
| `kesdas.DataFrame.diff` | `DataFrame.diff(periods : Int) : DataFrame` | The difference between consecutive values. |
| `kesdas.DataFrame.pctChange` | `DataFrame.pctChange(periods : Int) : DataFrame` | The fractional change between consecutive values. |
| `kesdas.DataFrame.cumSum` | `DataFrame.cumSum() : DataFrame` | The running sum. |
| `kesdas.DataFrame.cumProd` | `DataFrame.cumProd() : DataFrame` | The running product. |
| `kesdas.DataFrame.cumMax` | `DataFrame.cumMax() : DataFrame` | The running maximum. |
| `kesdas.DataFrame.cumMin` | `DataFrame.cumMin() : DataFrame` | The running minimum. |
| `kesdas.DataFrame.rank` | `DataFrame.rank(ascending : Bool, method : String, pct : Bool) : DataFrame` | The rank of each value. |
| `kesdas.DataFrame.interpolate` | `DataFrame.interpolate() : DataFrame` | Fills missing values from their neighbours. |
| `kesdas.DataFrame.where` | `DataFrame.where(mask : DataFrame, other : Scalar) : DataFrame` | The values kept where a condition holds. |
| `kesdas.DataFrame.where` | `DataFrame.where(mask : DataFrame) : DataFrame` | The values kept where a condition holds. |
| `kesdas.DataFrame.mask` | `DataFrame.mask(mask : DataFrame, other : Scalar) : DataFrame` | Replaces the values where a condition holds. |
| `kesdas.DataFrame.mask` | `DataFrame.mask(mask : DataFrame) : DataFrame` | Replaces the values where a condition holds. |
| `kesdas.DataFrame.replace` | `DataFrame.replace(old : String, replacement : String) : DataFrame` | Values replaced. |
| `kesdas.DataFrame.replace` | `DataFrame.replace(old : Int, replacement : Int) : DataFrame` | Values replaced. |
| `kesdas.DataFrame.replace` | `DataFrame.replace(old : double, replacement : double) : DataFrame` | Values replaced. |
| `kesdas.DataFrame.replace` | `DataFrame.replace(old : Bool, replacement : Bool) : DataFrame` | Values replaced. |
| `kesdas.DataFrame.replace` | `DataFrame.replace(old : Scalar, replacement : Scalar) : DataFrame` | Values replaced. |
| `kesdas.DataFrame.apply` | `DataFrame.apply(f : double -> double) : DataFrame` | Applies a function to the numeric columns. |
| `kesdas.DataFrame.insert` | `DataFrame.insert(position : Int, name : String, series : Series) : DataFrame or Error` | Adds a column at a position. |
| `kesdas.DataFrame.isNa` | `DataFrame.isNa() : DataFrame` | True where the value is missing. |
| `kesdas.DataFrame.notNa` | `DataFrame.notNa() : DataFrame` | True where the value is present. |
| `kesdas.DataFrame.fillNa` | `DataFrame.fillNa(value : double) : DataFrame` | Replaces missing values. |
| `kesdas.DataFrame.fillNa` | `DataFrame.fillNa(value : Int) : DataFrame` | Replaces missing values. |
| `kesdas.DataFrame.fillNa` | `DataFrame.fillNa(value : String) : DataFrame` | Replaces missing values. |
| `kesdas.DataFrame.fillNa` | `DataFrame.fillNa(value : Bool) : DataFrame` | Replaces missing values. |
| `kesdas.DataFrame.dropNa` | `DataFrame.dropNa(how : String, thresh : Int) : DataFrame` | A frame without rows that hold a missing value. |
| `kesdas.DataFrame.dropNa` | `DataFrame.dropNa(subset : List[String], how : String, thresh : Int) : DataFrame or Error` | A frame without rows that hold a missing value. |
| `kesdas.DataFrame.duplicated` | `DataFrame.duplicated(keep : String) : Series` | True where a row repeats an earlier one. |
| `kesdas.DataFrame.dropDuplicates` | `DataFrame.dropDuplicates(keep : String) : DataFrame` | A frame without repeated rows. |
| `kesdas.DataFrame.dropDuplicates` | `DataFrame.dropDuplicates(subset : List[String], keep : String) : DataFrame or Error` | A frame without repeated rows. |
| `kesdas.DataFrame.agg` | `DataFrame.agg(statistic : String) : Series` | Aggregates each column with named statistics. |
| `kesdas.DataFrame.sum` | `DataFrame.sum() : Series` | The sum. |
| `kesdas.DataFrame.prod` | `DataFrame.prod() : Series` | The product. |
| `kesdas.DataFrame.mean` | `DataFrame.mean() : Series` | The mean. |
| `kesdas.DataFrame.median` | `DataFrame.median() : Series` | The median. |
| `kesdas.DataFrame.min` | `DataFrame.min() : Series` | The smallest value. |
| `kesdas.DataFrame.max` | `DataFrame.max() : Series` | The largest value. |
| `kesdas.DataFrame.count` | `DataFrame.count() : Series` | The number of non-missing values. |
| `kesdas.DataFrame.std` | `DataFrame.std() : Series` | The standard deviation. |
| `kesdas.DataFrame.variance` | `DataFrame.variance() : Series` | The variance. |
| `kesdas.DataFrame.sem` | `DataFrame.sem() : Series` | The standard error of the mean. |
| `kesdas.DataFrame.skew` | `DataFrame.skew() : Series` | The skewness. |
| `kesdas.DataFrame.kurt` | `DataFrame.kurt() : Series` | The kurtosis. |
| `kesdas.DataFrame.any` | `DataFrame.any() : Series` | True when at least one value satisfies the predicate. |
| `kesdas.DataFrame.all` | `DataFrame.all() : Series` | True when every value satisfies the predicate. |
| `kesdas.DataFrame.nunique` | `DataFrame.nunique() : Series` | How many distinct values. |
| `kesdas.DataFrame.idxMax` | `DataFrame.idxMax() : Series` | The position of the largest value. |
| `kesdas.DataFrame.idxMin` | `DataFrame.idxMin() : Series` | The position of the smallest value. |
| `kesdas.DataFrame.mode` | `DataFrame.mode() : DataFrame` | The most frequent value. |
| `kesdas.DataFrame.agg` | `DataFrame.agg(spec : List[(String, String)]) : Series` | Aggregates each column with named statistics. |
| `kesdas.DataFrame.quantile` | `DataFrame.quantile(q : double) : Series` | The value at a fraction of the sorted data. |
| `kesdas.DataFrame.quantile` | `DataFrame.quantile(qs : List[double]) : DataFrame` | The value at a fraction of the sorted data. |
| `kesdas.DataFrame.describe` | `DataFrame.describe() : DataFrame` | Summary statistics per column. |
| `kesdas.DataFrame.corr` | `DataFrame.corr() : DataFrame` | The correlation with another series. |
| `kesdas.DataFrame.cov` | `DataFrame.cov(ddof : Int) : DataFrame` | The covariance with another series. |
| `kesdas.DataFrame.corrwith` | `DataFrame.corrwith(other : DataFrame) : Series` | The correlation of each column with another series. |
| `kesdas.DataFrame.valueCounts` | `DataFrame.valueCounts(column : String, normalize : Bool) : Series or Error` | How often each distinct value appears. |
| `kesdas.DataFrame.valueCounts` | `DataFrame.valueCounts(subset : List[String], normalize : Bool) : DataFrame or Error` | How often each distinct value appears. |
| `kesdas.DataFrame.groupBy` | `DataFrame.groupBy(key : String) : GroupBy or Error` | Splits the frame by one or more columns. |
| `kesdas.DataFrame.groupBy` | `DataFrame.groupBy(keys : List[String]) : GroupBy or Error` | Splits the frame by one or more columns. |
| `kesdas.DataFrame.merge` | `DataFrame.merge(other : DataFrame, on : String, how : String, leftSuffix : String, rightSuffix : String) : DataFrame or Error` | Joins two frames on columns. |
| `kesdas.DataFrame.merge` | `DataFrame.merge(other : DataFrame, on : List[String], how : String, leftSuffix : String, rightSuffix : String) : DataFrame or Error` | Joins two frames on columns. |
| `kesdas.DataFrame.merge` | `DataFrame.merge(other : DataFrame, leftOn : String, rightOn : String, how : String, leftSuffix : String, rightSuffix : String) : DataFrame or Error` | Joins two frames on columns. |
| `kesdas.DataFrame.append` | `DataFrame.append(other : DataFrame) : DataFrame` | Appends the rows of another frame. |
| `kesdas.DataFrame.pivotTable` | `DataFrame.pivotTable(index : String, columns : String, values : String, aggfunc : String) : DataFrame or Error` | A pivot table with an aggregation. |
| `kesdas.DataFrame.melt` | `DataFrame.melt(idVars : List[String], valueVars : List[String], varName : String, valueName : String) : DataFrame or Error` | The frame in long form. |
| `kesdas.DataFrame.pivot` | `DataFrame.pivot(index : String, columns : String, values : String) : DataFrame or Error` | The frame in wide form. |
| `kesdas.DataFrame.crosstab` | `DataFrame.crosstab(index : String, columns : String) : DataFrame or Error` | A table of counts for two columns. |
| `kesdas.DataFrame.getDummies` | `DataFrame.getDummies(columns : List[String], prefix : String, dropFirst : Bool, dummyNa : Bool) : DataFrame or Error` | One-hot columns for a categorical column. |
| `kesdas.DataFrame.transpose` | `DataFrame.transpose() : DataFrame or Error` | The transpose. |
| `kesdas.DataFrame.toTensor` | `DataFrame.toTensor() : DynTensor[double]` | The values as a tensor. |
| `kesdas.DataFrame.toCsv` | `DataFrame.toCsv(separator : String, header : Bool, includeIndex : Bool) : String` | The frame as CSV text. |
| `kesdas.DataFrame.text` | `DataFrame.text() : String` | The value as text. |
| `kesdas.DataFrame.html` | `DataFrame.html() : String` | The frame as an HTML table. |
| `kesdas.DataFrame.cell` | `DataFrame.cell(row : Int, column : String) : Scalar or Error` | One cell of a frame. |
| `kesdas.DataFrame.mapNumeric` | `DataFrame.mapNumeric(f : double -> double) : DataFrame` | Applies a function to the numeric columns only. |
| `kesdas.DataFrame.rolling` | `DataFrame.rolling(window : Int, minPeriods : Int) : FrameRolling` | A rolling window over the series. |
| `kesdas.DataFrame.expanding` | `DataFrame.expanding(minPeriods : Int) : FrameExpanding` | A window from the first row to the current one. |
| `kesdas.DataFrame.ewm` | `DataFrame.ewm(alpha : double, adjust : Bool, minPeriods : Int) : FrameEWM` | An exponentially weighted window. |
| `kesdas.DataFrame.saveCsv` | `DataFrame.saveCsv(path : String, separator : String, includeIndex : Bool) : void or Error with Effects` | Writes the frame as a CSV file. |
| `kesdas.DataFrame.info` | `DataFrame.info() : void with Effects` | A summary of the columns and their types. |

### FrameRolling

| Member | Signature | What it does |
|---|---|---|
| `kesdas.FrameRolling.mean` | `FrameRolling.mean() : DataFrame` | The mean. |
| `kesdas.FrameRolling.sum` | `FrameRolling.sum() : DataFrame` | The sum. |
| `kesdas.FrameRolling.min` | `FrameRolling.min() : DataFrame` | The smallest value. |
| `kesdas.FrameRolling.max` | `FrameRolling.max() : DataFrame` | The largest value. |
| `kesdas.FrameRolling.std` | `FrameRolling.std() : DataFrame` | The standard deviation. |
| `kesdas.FrameRolling.count` | `FrameRolling.count() : DataFrame` | The number of non-missing values. |
| `kesdas.FrameRolling.median` | `FrameRolling.median() : DataFrame` | The median. |
| `kesdas.FrameRolling.variance` | `FrameRolling.variance() : DataFrame` | The variance. |
| `kesdas.FrameRolling.sem` | `FrameRolling.sem() : DataFrame` | The standard error of the mean. |
| `kesdas.FrameRolling.skew` | `FrameRolling.skew() : DataFrame` | The skewness. |
| `kesdas.FrameRolling.kurt` | `FrameRolling.kurt() : DataFrame` | The kurtosis. |
| `kesdas.FrameRolling.quantile` | `FrameRolling.quantile(q : double) : DataFrame` | The value at a fraction of the sorted data. |

### FrameExpanding

| Member | Signature | What it does |
|---|---|---|
| `kesdas.FrameExpanding.mean` | `FrameExpanding.mean() : DataFrame` | The mean. |
| `kesdas.FrameExpanding.sum` | `FrameExpanding.sum() : DataFrame` | The sum. |
| `kesdas.FrameExpanding.min` | `FrameExpanding.min() : DataFrame` | The smallest value. |
| `kesdas.FrameExpanding.max` | `FrameExpanding.max() : DataFrame` | The largest value. |
| `kesdas.FrameExpanding.std` | `FrameExpanding.std() : DataFrame` | The standard deviation. |
| `kesdas.FrameExpanding.count` | `FrameExpanding.count() : DataFrame` | The number of non-missing values. |
| `kesdas.FrameExpanding.median` | `FrameExpanding.median() : DataFrame` | The median. |
| `kesdas.FrameExpanding.variance` | `FrameExpanding.variance() : DataFrame` | The variance. |
| `kesdas.FrameExpanding.sem` | `FrameExpanding.sem() : DataFrame` | The standard error of the mean. |
| `kesdas.FrameExpanding.quantile` | `FrameExpanding.quantile(q : double) : DataFrame` | The value at a fraction of the sorted data. |

### FrameEWM

| Member | Signature | What it does |
|---|---|---|
| `kesdas.FrameEWM.mean` | `FrameEWM.mean() : DataFrame` | The mean. |
| `kesdas.FrameEWM.sum` | `FrameEWM.sum() : DataFrame` | The sum. |
| `kesdas.FrameEWM.variance` | `FrameEWM.variance(bias : Bool) : DataFrame` | The variance. |
| `kesdas.FrameEWM.std` | `FrameEWM.std(bias : Bool) : DataFrame` | The standard deviation. |
| `kesdas.FrameEWM.count` | `FrameEWM.count() : DataFrame` | The number of non-missing values. |

### GroupBy

| Member | Signature | What it does |
|---|---|---|
| `kesdas.GroupBy.count` | `GroupBy.count() : Int` | The number of non-missing values. |
| `kesdas.GroupBy.aggregate` | `GroupBy.aggregate(statistic : String) : DataFrame` | Aggregates the columns of a grouped frame. |
| `kesdas.GroupBy.sum` | `GroupBy.sum() : DataFrame` | The sum. |
| `kesdas.GroupBy.mean` | `GroupBy.mean() : DataFrame` | The mean. |
| `kesdas.GroupBy.median` | `GroupBy.median() : DataFrame` | The median. |
| `kesdas.GroupBy.min` | `GroupBy.min() : DataFrame` | The smallest value. |
| `kesdas.GroupBy.max` | `GroupBy.max() : DataFrame` | The largest value. |
| `kesdas.GroupBy.std` | `GroupBy.std() : DataFrame` | The standard deviation. |
| `kesdas.GroupBy.variance` | `GroupBy.variance() : DataFrame` | The variance. |
| `kesdas.GroupBy.first` | `GroupBy.first() : DataFrame` | The first value. |
| `kesdas.GroupBy.last` | `GroupBy.last() : DataFrame` | The last value. |
| `kesdas.GroupBy.nunique` | `GroupBy.nunique() : DataFrame` | How many distinct values. |
| `kesdas.GroupBy.prod` | `GroupBy.prod() : DataFrame` | The product. |
| `kesdas.GroupBy.counts` | `GroupBy.counts() : DataFrame` | The number of non-missing values per row or group. |
| `kesdas.GroupBy.size` | `GroupBy.size() : Series` | How many values. |
| `kesdas.GroupBy.head` | `GroupBy.head(n : Int) : DataFrame` | The first rows. |
| `kesdas.GroupBy.tail` | `GroupBy.tail(n : Int) : DataFrame` | The last rows. |
| `kesdas.GroupBy.nth` | `GroupBy.nth(n : Int) : DataFrame` | The value at a position. |
| `kesdas.GroupBy.cumCount` | `GroupBy.cumCount(ascending : Bool) : Series` | The running count. |
| `kesdas.GroupBy.ngroup` | `GroupBy.ngroup() : Series` | How many groups there are. |
| `kesdas.GroupBy.getGroup` | `GroupBy.getGroup(label : Int) : DataFrame or Error` | One group of a grouped frame. |
| `kesdas.GroupBy.getGroup` | `GroupBy.getGroup(label : String) : DataFrame or Error` | One group of a grouped frame. |
| `kesdas.GroupBy.getGroup` | `GroupBy.getGroup(label : double) : DataFrame or Error` | One group of a grouped frame. |
| `kesdas.GroupBy.getGroup` | `GroupBy.getGroup(label : Bool) : DataFrame or Error` | One group of a grouped frame. |
| `kesdas.GroupBy.transformAll` | `GroupBy.transformAll(statistic : String) : DataFrame` | Transforms every column. |
| `kesdas.GroupBy.shift` | `GroupBy.shift(column : String, periods : Int) : Series` | The values moved by n positions. |
| `kesdas.GroupBy.diff` | `GroupBy.diff(column : String, periods : Int) : Series` | The difference between consecutive values. |
| `kesdas.GroupBy.pctChange` | `GroupBy.pctChange(column : String, periods : Int) : Series` | The fractional change between consecutive values. |
| `kesdas.GroupBy.cumSum` | `GroupBy.cumSum(column : String) : Series` | The running sum. |
| `kesdas.GroupBy.cumProd` | `GroupBy.cumProd(column : String) : Series` | The running product. |
| `kesdas.GroupBy.cumMax` | `GroupBy.cumMax(column : String) : Series` | The running maximum. |
| `kesdas.GroupBy.cumMin` | `GroupBy.cumMin(column : String) : Series` | The running minimum. |
| `kesdas.GroupBy.rank` | `GroupBy.rank(column : String, ascending : Bool, method : String, pct : Bool) : Series` | The rank of each value. |
| `kesdas.GroupBy.sem` | `GroupBy.sem() : DataFrame` | The standard error of the mean. |
| `kesdas.GroupBy.skew` | `GroupBy.skew() : DataFrame` | The skewness. |
| `kesdas.GroupBy.kurt` | `GroupBy.kurt() : DataFrame` | The kurtosis. |
| `kesdas.GroupBy.all` | `GroupBy.all() : DataFrame` | True when every value satisfies the predicate. |
| `kesdas.GroupBy.any` | `GroupBy.any() : DataFrame` | True when at least one value satisfies the predicate. |
| `kesdas.GroupBy.quantile` | `GroupBy.quantile(q : double) : DataFrame` | The value at a fraction of the sorted data. |
| `kesdas.GroupBy.agg` | `GroupBy.agg(spec : List[(String, String)]) : DataFrame` | Aggregates each column with named statistics. |
| `kesdas.GroupBy.filterGroups` | `GroupBy.filterGroups(keep : Int -> Bool) : DataFrame` | Keeps the groups whose aggregate passes a test. |
| `kesdas.GroupBy.transform` | `GroupBy.transform(column : String, statistic : String) : Series` | Transforms new data with the fitted parameters. |
