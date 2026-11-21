# std.kesdas reference

Every public member of `std.kesdas`, grouped by type. The signature column
names each argument with its type and its default; a method's receiver is implicit.

The API chains. Methods return new values, so a whole pipeline is one expression:

```kestrel
let top := sales.groupBy("region")?.sum().sortValues("revenue", ascending = false)?
let average := sales.col("units")?.rolling(3).mean()
let scaled := sales.select(["units", "price"])?.mapNumeric(x => x / 100.0)
```


### Types

- `Column`: A typed column of a data frame, with its own missing markers.
- `Scalar`: One typed value, possibly missing.
- `Series`: A named column with the pandas-style operations.
- `Rolling`: A rolling window over a series.
- `Expanding`: A window from the first row to the current one.
- `EWM`: An exponentially weighted window.
- `StringMethods`: Text operations reached through a text series.
- `DataError`: The error type the data-frame functions return.
- `DataFrame`: Named columns of equal length, with grouping, joins and windows.
- `FrameRolling`: A rolling window over every numeric column of a frame.
- `FrameExpanding`: An expanding window over a frame.
- `FrameEWM`: An exponentially weighted window over a frame.
- `GroupBy`: A frame split by key columns, ready to aggregate.

### Functions

| Member | Signature | What it does |
|---|---|---|
| `ofInts` | `Column.ofInts(values : List[Int], missing : List[Bool]) : Column` | An integer column. |
| `ofFloats` | `Column.ofFloats(values : List[double], missing : List[Bool]) : Column` | A float column. |
| `ofBools` | `Column.ofBools(values : List[Bool], missing : List[Bool]) : Column` | A boolean column. |
| `ofStrings` | `Column.ofStrings(values : List[String], missing : List[Bool]) : Column` | A text column. |
| `noneMissing` | `Column.noneMissing(n : Int) : List[Bool]` | A missing mask that marks nothing missing. |
| `ids` | `Column.ids(count : Int) : Column` | A column of row numbers. |
| `ints` | `Column.ints(values : List[Int]) : Column` | An integer column from a list. |
| `floats` | `Column.floats(values : List[double]) : Column` | A float column from a list. |
| `bools` | `Column.bools(values : List[Bool]) : Column` | A boolean column from a list. |
| `strings` | `Column.strings(values : List[String]) : Column` | A text column from a list. |
| `empty` | `Column.empty(kind : Int, size : Int) : Column` | An empty value of this type. |
| `ofInt` | `Scalar.ofInt(value : Int) : Scalar` | An integer scalar. |
| `ofFloat` | `Scalar.ofFloat(value : double) : Scalar` | A float scalar. |
| `ofBool` | `Scalar.ofBool(value : Bool) : Scalar` | A boolean scalar. |
| `ofString` | `Scalar.ofString(value : String) : Scalar` | A text scalar. |
| `missing` | `Scalar.missing() : Scalar` | The positions that are missing. |
| `concat` | `Series.concat(parts : List[Series]) : Series` | Joins values end to end. |
| `empty` | `DataFrame.empty() : DataFrame` | An empty value of this type. |
| `concat` | `DataFrame.concat(frames : List[DataFrame]) : DataFrame` | Joins values end to end. |
| `concatColumns` | `DataFrame.concatColumns(frames : List[DataFrame]) : DataFrame or Error` | Joins frames side by side. |
| `parseCsv` | `DataFrame.parseCsv(text : String, separator : String, header : Bool, skipRows : Int, nRows : Int) : DataFrame` | A frame from CSV text. |
| `parseCsvLines` | `DataFrame.parseCsvLines(lines : List[String], separator : String, header : Bool, skipRows : Int, nRows : Int) : DataFrame` | A frame from CSV lines. |
| `readCsv` | `std.kesdas.readCsv(path : String, separator : String, header : Bool, skipRows : Int, nRows : Int) : DataFrame or Error with Effects` | A frame from a CSV file. |

### Column

| Member | Signature | What it does |
|---|---|---|
| `tile` | `Column.tile(times : Int) : Column` | The values repeated n times. |
| `concat` | `Column.concat(other : Column) : Column` | Joins values end to end. |
| `copy` | `Column.copy() : Column` | A value with the same contents. |
| `dtype` | `Column.dtype() : String` | The type of the values. |
| `isNumeric` | `Column.isNumeric() : Bool` | True for a numeric column. |
| `orderPositions` | `Column.orderPositions(start : List[Int], ascending : Bool, naFirst : Bool) : List[Int]` | The positions that sort the values. |
| `topPositions` | `Column.topPositions(n : Int, ascending : Bool) : List[Int]` | The positions of the largest values. |
| `gather` | `Column.gather(positions : List[Int]) : Column` | Picks the values at the given positions. |
| `head` | `Column.head(n : Int) : Column` | The first rows. |
| `join` | `Column.join(other : Column) : Column` | Joins two values together. |
| `toFloats` | `Column.toFloats() : List[double]` | The values as doubles. |
| `texts` | `Column.texts() : List[String]` | The text elements of the figure. |
| `keys` | `Column.keys() : List[String]` | The key values of a grouped frame. |

### Scalar

| Member | Signature | What it does |
|---|---|---|
| `isMissing` | `Scalar.isMissing() : Bool` | True when the value is missing. |
| `isNumeric` | `Scalar.isNumeric() : Bool` | True for a numeric column. |
| `asDouble` | `Scalar.asDouble() : double` | The value as a double. |
| `asInt` | `Scalar.asInt() : Int` | The value as an Int. |
| `asString` | `Scalar.asString() : String` | The value as a String. |
| `asBool` | `Scalar.asBool() : Bool` | The value as a Bool, when it is one. |
| `text` | `Scalar.text() : String` | The value as text. |
| `equals` | `Scalar.equals(other : Scalar) : Bool` | True when both values are equal. |

### Series

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Series.copy() : Series` | A value with the same contents. |
| `length` | `Series.length() : Int` | How many values. |
| `size` | `Series.size() : Int` | How many values. |
| `ndim` | `Series.ndim() : Int` | The number of dimensions. |
| `isEmpty` | `Series.isEmpty() : Bool` | True when there are no values. |
| `dtype` | `Series.dtype() : String` | The type of the values. |
| `derive` | `Series.derive(data : Column) : Series` | A new series computed from this one. |
| `take` | `Series.take(positions : List[Int]) : Series` | The first values. |
| `toScalars` | `Series.toScalars() : List[Scalar]` | The values as scalars. |
| `items` | `Series.items() : List[(Scalar, Scalar)]` | The values with their labels. |
| `equals` | `Series.equals(other : Series) : Bool` | True when both values are equal. |
| `rename` | `Series.rename(name : String) : Series` | A copy with a value renamed. |
| `withIndex` | `Series.withIndex(labels : List[String]) : Series` | A copy with a new index. |
| `withIndex` | `Series.withIndex(labels : List[Int]) : Series` | A copy with a new index. |
| `withIndexColumn` | `Series.withIndexColumn(labels : Column) : Series` | A copy with a column as its index. |
| `named` | `Series.named(indexName : String) : Series` | A copy with a column name. |
| `toInts` | `Series.toInts() : List[Int]` | The values as integers. |
| `toFloats` | `Series.toFloats() : List[double]` | The values as doubles. |
| `toStrings` | `Series.toStrings() : List[String]` | The values as text. |
| `toBools` | `Series.toBools() : List[Bool]` | The values as booleans. |
| `indexLabels` | `Series.indexLabels() : List[String]` | The index labels. |
| `missingMask` | `Series.missingMask() : List[Bool]` | A mask that marks the missing values. |
| `astype` | `Series.astype(dtype : String) : Series` | Converts the values to another type. |
| `head` | `Series.head(n : Int) : Series` | The first rows. |
| `tail` | `Series.tail(n : Int) : Series` | The last rows. |
| `slice` | `Series.slice(start : Int, stop : Int) : Series` | A contiguous part. |
| `iloc` | `Series.iloc(position : Int) : Scalar` | A row by position. |
| `iloc` | `Series.iloc(positions : List[Int]) : Series` | A row by position. |
| `iat` | `Series.iat(position : Int) : Scalar` | The value at a row and column position. |
| `takeGaps` | `Series.takeGaps(positions : List[Int]) : Series` | Keeps the values where the mask is true. |
| `positionOf` | `Series.positionOf(label : Int) : Option[Int]` | The position of a value. |
| `positionOf` | `Series.positionOf(label : String) : Option[Int]` | The position of a value. |
| `positionOf` | `Series.positionOf(label : double) : Option[Int]` | The position of a value. |
| `positionOf` | `Series.positionOf(label : Bool) : Option[Int]` | The position of a value. |
| `loc` | `Series.loc(label : Int) : Scalar` | A row or column by label. |
| `loc` | `Series.loc(label : String) : Scalar` | A row or column by label. |
| `loc` | `Series.loc(label : double) : Scalar` | A row or column by label. |
| `loc` | `Series.loc(label : Bool) : Scalar` | A row or column by label. |
| `at` | `Series.at(label : Int) : Scalar` | The value at a position or label. |
| `at` | `Series.at(label : String) : Scalar` | The value at a position or label. |
| `at` | `Series.at(label : double) : Scalar` | The value at a position or label. |
| `at` | `Series.at(label : Bool) : Scalar` | The value at a position or label. |
| `filter` | `Series.filter(mask : Series) : Series` | The values whose mask is true. |
| `where` | `Series.where(mask : Series, other : Scalar) : Series` | The values kept where a condition holds. |
| `where` | `Series.where(mask : Series, other : Series) : Series` | The values kept where a condition holds. |
| `mask` | `Series.mask(mask : Series, other : Scalar) : Series` | Replaces the values where a condition holds. |
| `sortValues` | `Series.sortValues(ascending : Bool, naPosition : String) : Series` | A frame sorted by a column. |
| `sortIndex` | `Series.sortIndex(ascending : Bool, naPosition : String) : Series` | A frame sorted by its index. |
| `nlargest` | `Series.nlargest(n : Int) : Series` | The n largest values. |
| `nsmallest` | `Series.nsmallest(n : Int) : Series` | The n smallest values. |
| `resetIndex` | `Series.resetIndex() : Series` | A frame with a fresh row index. |
| `add` | `Series.add(other : Series) : Series` | Elementwise addition. |
| `subtract` | `Series.subtract(other : Series) : Series` | Elementwise subtraction. |
| `multiply` | `Series.multiply(other : Series) : Series` | Elementwise multiplication. |
| `divide` | `Series.divide(other : Series) : Series` | Elementwise division. |
| `floorDivide` | `Series.floorDivide(other : Series) : Series` | Elementwise floor division. |
| `mod` | `Series.mod(other : Series) : Series` | Elementwise remainder. |
| `pow` | `Series.pow(other : Series) : Series` | Elementwise power. |
| `add` | `Series.add(other : double) : Series` | Elementwise addition. |
| `subtract` | `Series.subtract(other : double) : Series` | Elementwise subtraction. |
| `multiply` | `Series.multiply(other : double) : Series` | Elementwise multiplication. |
| `divide` | `Series.divide(other : double) : Series` | Elementwise division. |
| `floorDivide` | `Series.floorDivide(other : double) : Series` | Elementwise floor division. |
| `mod` | `Series.mod(other : double) : Series` | Elementwise remainder. |
| `pow` | `Series.pow(other : double) : Series` | Elementwise power. |
| `add` | `Series.add(other : Int) : Series` | Elementwise addition. |
| `subtract` | `Series.subtract(other : Int) : Series` | Elementwise subtraction. |
| `multiply` | `Series.multiply(other : Int) : Series` | Elementwise multiplication. |
| `divide` | `Series.divide(other : Int) : Series` | Elementwise division. |
| `floorDivide` | `Series.floorDivide(other : Int) : Series` | Elementwise floor division. |
| `mod` | `Series.mod(other : Int) : Series` | Elementwise remainder. |
| `pow` | `Series.pow(other : Int) : Series` | Elementwise power. |
| `add` | `Series.add(other : String) : Series` | Elementwise addition. |
| `rightAdd` | `Series.rightAdd(other : double) : Series` | The value added to an argument on its right. |
| `rightSubtract` | `Series.rightSubtract(other : double) : Series` | An argument on the right minus the value. |
| `rightMultiply` | `Series.rightMultiply(other : double) : Series` | The value multiplied by an argument on its right. |
| `rightDivide` | `Series.rightDivide(other : double) : Series` | An argument on the right divided by the value. |
| `rightAdd` | `Series.rightAdd(other : Int) : Series` | The value added to an argument on its right. |
| `rightSubtract` | `Series.rightSubtract(other : Int) : Series` | An argument on the right minus the value. |
| `rightMultiply` | `Series.rightMultiply(other : Int) : Series` | The value multiplied by an argument on its right. |
| `rightDivide` | `Series.rightDivide(other : Int) : Series` | An argument on the right divided by the value. |
| `negate` | `Series.negate() : Series` | Elementwise negation. |
| `abs` | `Series.abs() : Series` | The absolute value. |
| `round` | `Series.round(decimals : Int) : Series` | Rounds each value. |
| `sqrt` | `Series.sqrt() : Series` | The square root. |
| `clip` | `Series.clip(low : double, high : double) : Series` | Limits values to a range. |
| `clip` | `Series.clip(low : Int, high : Int) : Series` | Limits values to a range. |
| `item` | `Series.item(position : Int) : Scalar` | The single value. |
| `isUnique` | `Series.isUnique() : Bool` | True when every value appears once. |
| `isMonotonic` | `Series.isMonotonic(increasing : Bool) : Bool` | True when the values never go back. |
| `repeat` | `Series.repeat(times : Int) : Series` | The value repeated n times. |
| `eq` | `Series.eq(other : Series) : Series` | Elementwise equality. |
| `ne` | `Series.ne(other : Series) : Series` | Elementwise inequality. |
| `lt` | `Series.lt(other : Series) : Series` | Elementwise less-than. |
| `le` | `Series.le(other : Series) : Series` | Elementwise less-or-equal. |
| `gt` | `Series.gt(other : Series) : Series` | Elementwise greater-than. |
| `ge` | `Series.ge(other : Series) : Series` | Elementwise greater-or-equal. |
| `eq` | `Series.eq(other : double) : Series` | Elementwise equality. |
| `ne` | `Series.ne(other : double) : Series` | Elementwise inequality. |
| `lt` | `Series.lt(other : double) : Series` | Elementwise less-than. |
| `le` | `Series.le(other : double) : Series` | Elementwise less-or-equal. |
| `gt` | `Series.gt(other : double) : Series` | Elementwise greater-than. |
| `ge` | `Series.ge(other : double) : Series` | Elementwise greater-or-equal. |
| `eq` | `Series.eq(other : Int) : Series` | Elementwise equality. |
| `ne` | `Series.ne(other : Int) : Series` | Elementwise inequality. |
| `lt` | `Series.lt(other : Int) : Series` | Elementwise less-than. |
| `le` | `Series.le(other : Int) : Series` | Elementwise less-or-equal. |
| `gt` | `Series.gt(other : Int) : Series` | Elementwise greater-than. |
| `ge` | `Series.ge(other : Int) : Series` | Elementwise greater-or-equal. |
| `eq` | `Series.eq(other : String) : Series` | Elementwise equality. |
| `ne` | `Series.ne(other : String) : Series` | Elementwise inequality. |
| `lt` | `Series.lt(other : String) : Series` | Elementwise less-than. |
| `le` | `Series.le(other : String) : Series` | Elementwise less-or-equal. |
| `gt` | `Series.gt(other : String) : Series` | Elementwise greater-than. |
| `ge` | `Series.ge(other : String) : Series` | Elementwise greater-or-equal. |
| `eq` | `Series.eq(other : Bool) : Series` | Elementwise equality. |
| `ne` | `Series.ne(other : Bool) : Series` | Elementwise inequality. |
| `both` | `Series.both(other : Series) : Series` | A pair of series or columns. |
| `either` | `Series.either(other : Series) : Series` | The union of two boolean masks. |
| `invert` | `Series.invert() : Series` | The complement of a boolean mask. |
| `between` | `Series.between(low : double, high : double) : Series` | True where the value lies in a range. |
| `between` | `Series.between(low : Int, high : Int) : Series` | True where the value lies in a range. |
| `between` | `Series.between(low : String, high : String) : Series` | True where the value lies in a range. |
| `isIn` | `Series.isIn(values : List[Int]) : Series` | True where the value is in a collection. |
| `isIn` | `Series.isIn(values : List[double]) : Series` | True where the value is in a collection. |
| `isIn` | `Series.isIn(values : List[String]) : Series` | True where the value is in a collection. |
| `isIn` | `Series.isIn(values : List[Bool]) : Series` | True where the value is in a collection. |
| `isNa` | `Series.isNa() : Series` | True where the value is missing. |
| `notNa` | `Series.notNa() : Series` | True where the value is present. |
| `fillNa` | `Series.fillNa(value : double) : Series` | Replaces missing values. |
| `fillNa` | `Series.fillNa(value : Int) : Series` | Replaces missing values. |
| `fillNa` | `Series.fillNa(value : String) : Series` | Replaces missing values. |
| `fillNa` | `Series.fillNa(value : Bool) : Series` | Replaces missing values. |
| `fillNa` | `Series.fillNa(value : Scalar) : Series` | Replaces missing values. |
| `dropNa` | `Series.dropNa() : Series` | A frame without rows that hold a missing value. |
| `ffill` | `Series.ffill() : Series` | Fills a missing value from the previous one. |
| `bfill` | `Series.bfill() : Series` | Fills a missing value from the next one. |
| `interpolate` | `Series.interpolate() : Series` | Fills missing values from their neighbours. |
| `count` | `Series.count() : Int` | The number of non-missing values. |
| `sum` | `Series.sum() : Scalar` | The sum. |
| `min` | `Series.min() : Scalar` | The smallest value. |
| `max` | `Series.max() : Scalar` | The largest value. |
| `mean` | `Series.mean() : Scalar` | The mean. |
| `median` | `Series.median() : Scalar` | The median. |
| `quantile` | `Series.quantile(q : double) : Scalar` | The value at a fraction of the sorted data. |
| `quantile` | `Series.quantile(qs : List[double]) : Series` | The value at a fraction of the sorted data. |
| `std` | `Series.std(ddof : Int) : Scalar` | The standard deviation. |
| `variance` | `Series.variance(ddof : Int) : Scalar` | The variance. |
| `sem` | `Series.sem(ddof : Int) : Scalar` | The standard error of the mean. |
| `skew` | `Series.skew() : Scalar` | The skewness. |
| `kurt` | `Series.kurt() : Scalar` | The kurtosis. |
| `prod` | `Series.prod() : Scalar` | The product. |
| `any` | `Series.any() : Bool` | True when at least one value satisfies the predicate. |
| `all` | `Series.all() : Bool` | True when every value satisfies the predicate. |
| `nunique` | `Series.nunique() : Int` | How many distinct values. |
| `idxMax` | `Series.idxMax() : Scalar` | The position of the largest value. |
| `idxMin` | `Series.idxMin() : Scalar` | The position of the smallest value. |
| `argMax` | `Series.argMax() : Int` | The position of the largest value. |
| `argMin` | `Series.argMin() : Int` | The position of the smallest value. |
| `unique` | `Series.unique() : Series` | The distinct values. |
| `duplicated` | `Series.duplicated(keep : String) : Series` | True where a row repeats an earlier one. |
| `dropDuplicates` | `Series.dropDuplicates(keep : String) : Series` | A frame without repeated rows. |
| `valueCounts` | `Series.valueCounts(ascending : Bool, normalize : Bool, sort : Bool, dropNa : Bool) : Series` | How often each distinct value appears. |
| `mode` | `Series.mode() : Series` | The most frequent value. |
| `cumSum` | `Series.cumSum() : Series` | The running sum. |
| `cumProd` | `Series.cumProd() : Series` | The running product. |
| `cumMax` | `Series.cumMax() : Series` | The running maximum. |
| `cumMin` | `Series.cumMin() : Series` | The running minimum. |
| `shift` | `Series.shift(periods : Int) : Series` | The values moved by n positions. |
| `diff` | `Series.diff(periods : Int) : Series` | The difference between consecutive values. |
| `pctChange` | `Series.pctChange(periods : Int) : Series` | The fractional change between consecutive values. |
| `rolling` | `Series.rolling(window : Int, minPeriods : Int) : Rolling` | A rolling window over the series. |
| `expanding` | `Series.expanding(minPeriods : Int) : Expanding` | A window from the first row to the current one. |
| `ewm` | `Series.ewm(alpha : double, adjust : Bool, minPeriods : Int) : EWM` | An exponentially weighted window. |
| `ewmSpan` | `Series.ewmSpan(span : Int, adjust : Bool, minPeriods : Int) : EWM` | An exponentially weighted window by span. |
| `ewmHalfLife` | `Series.ewmHalfLife(halflife : double, adjust : Bool, minPeriods : Int) : EWM` | An exponentially weighted window by half-life. |
| `rank` | `Series.rank(ascending : Bool, method : String, pct : Bool) : Series` | The rank of each value. |
| `apply` | `Series.apply(f : double -> double) : Series` | Applies a function to every value. |
| `applyInt` | `Series.applyInt(f : Int -> Int) : Series` | Applies a function to an integer series. |
| `applyString` | `Series.applyString(f : String -> String) : Series` | Applies a function to a text series. |
| `applyBool` | `Series.applyBool(f : double -> Bool) : Series` | Applies a function to a boolean series. |
| `map` | `Series.map(f : Scalar -> Scalar) : Series` | Applies a function to every value. |
| `replace` | `Series.replace(old : String, replacement : String) : Series` | Values replaced. |
| `replace` | `Series.replace(old : Int, replacement : Int) : Series` | Values replaced. |
| `replace` | `Series.replace(old : double, replacement : double) : Series` | Values replaced. |
| `replace` | `Series.replace(old : Bool, replacement : Bool) : Series` | Values replaced. |
| `replace` | `Series.replace(old : Scalar, replacement : Scalar) : Series` | Values replaced. |
| `replaceMany` | `Series.replaceMany(pairs : List[(Scalar, Scalar)]) : Series` | Several replacements in one pass. |
| `str` | `Series.str() : StringMethods` | The values as text. |
| `mapValues` | `Series.mapValues(mapping : Map[String, String]) : Series` | Transforms the values of a map or frame. |
| `splitToFrame` | `Series.splitToFrame(separator : String) : DataFrame` | A frame from the parts of a split. |
| `cov` | `Series.cov(other : Series, ddof : Int) : Scalar` | The covariance with another series. |
| `corr` | `Series.corr(other : Series) : Scalar` | The correlation with another series. |
| `autocorr` | `Series.autocorr(lag : Int) : Scalar` | The autocorrelation at a lag. |
| `factorize` | `Series.factorize() : (Series, Series)` | Maps each distinct value to an integer code. |
| `toCsv` | `Series.toCsv(separator : String, header : Bool, includeIndex : Bool) : String` | The frame as CSV text. |
| `toTensor` | `Series.toTensor() : DynTensor[double]` | The values as a tensor. |
| `append` | `Series.append(other : Series) : Series` | Appends the rows of another frame. |
| `combineFirst` | `Series.combineFirst(other : Series) : Series` | Fills missing values from another series. |
| `update` | `Series.update(other : Series) : Series` | Updates the values in place. |
| `describe` | `Series.describe() : Series` | Summary statistics per column. |
| `toFrame` | `Series.toFrame() : DataFrame` | A frame from the values. |
| `text` | `Series.text() : String` | The value as text. |
| `html` | `Series.html() : String` | The frame as an HTML table. |

### Rolling

| Member | Signature | What it does |
|---|---|---|
| `mean` | `Rolling.mean() : Series` | The mean. |
| `sum` | `Rolling.sum() : Series` | The sum. |
| `min` | `Rolling.min() : Series` | The smallest value. |
| `max` | `Rolling.max() : Series` | The largest value. |
| `std` | `Rolling.std() : Series` | The standard deviation. |
| `variance` | `Rolling.variance() : Series` | The variance. |
| `median` | `Rolling.median() : Series` | The median. |
| `count` | `Rolling.count() : Series` | The number of non-missing values. |
| `sem` | `Rolling.sem() : Series` | The standard error of the mean. |
| `skew` | `Rolling.skew() : Series` | The skewness. |
| `kurt` | `Rolling.kurt() : Series` | The kurtosis. |
| `quantile` | `Rolling.quantile(q : double) : Series` | The value at a fraction of the sorted data. |
| `apply` | `Rolling.apply(f : List[double] -> double) : Series` | Applies a function to every value. |

### Expanding

| Member | Signature | What it does |
|---|---|---|
| `mean` | `Expanding.mean() : Series` | The mean. |
| `sum` | `Expanding.sum() : Series` | The sum. |
| `min` | `Expanding.min() : Series` | The smallest value. |
| `max` | `Expanding.max() : Series` | The largest value. |
| `std` | `Expanding.std() : Series` | The standard deviation. |
| `variance` | `Expanding.variance() : Series` | The variance. |
| `median` | `Expanding.median() : Series` | The median. |
| `count` | `Expanding.count() : Series` | The number of non-missing values. |
| `sem` | `Expanding.sem() : Series` | The standard error of the mean. |
| `quantile` | `Expanding.quantile(q : double) : Series` | The value at a fraction of the sorted data. |
| `apply` | `Expanding.apply(f : List[double] -> double) : Series` | Applies a function to every value. |

### EWM

| Member | Signature | What it does |
|---|---|---|
| `mean` | `EWM.mean() : Series` | The mean. |
| `sum` | `EWM.sum() : Series` | The sum. |
| `variance` | `EWM.variance(bias : Bool) : Series` | The variance. |
| `std` | `EWM.std(bias : Bool) : Series` | The standard deviation. |
| `count` | `EWM.count() : Series` | The number of non-missing values. |

### StringMethods

| Member | Signature | What it does |
|---|---|---|
| `upper` | `StringMethods.upper() : Series` | The text in upper case. |
| `lower` | `StringMethods.lower() : Series` | The lower-triangular part. |
| `swapcase` | `StringMethods.swapcase() : Series` | The text with the case of each character swapped. |
| `strip` | `StringMethods.strip() : Series` | Removes whitespace at both ends. |
| `stripLeading` | `StringMethods.stripLeading() : Series` | Removes whitespace at the start. |
| `stripTrailing` | `StringMethods.stripTrailing() : Series` | Removes whitespace at the end. |
| `replace` | `StringMethods.replace(old : String, replacement : String) : Series` | Values replaced. |
| `capitalize` | `StringMethods.capitalize() : Series` | The text with its first character upper-cased. |
| `title` | `StringMethods.title() : Series` | The text in title case. |
| `slice` | `StringMethods.slice(start : Int, stop : Int) : Series` | A contiguous part. |
| `sliceReplace` | `StringMethods.sliceReplace(start : Int, stop : Int, replacement : String) : Series` | A copy with a range replaced. |
| `cat` | `StringMethods.cat(other : String) : Series` | Joins variables along an axis. |
| `repeat` | `StringMethods.repeat(times : Int) : Series` | The value repeated n times. |
| `pad` | `StringMethods.pad(width : Int) : Series` | Pads a value to a length. |
| `ljust` | `StringMethods.ljust(width : Int) : Series` | Pads the text on the right. |
| `rjust` | `StringMethods.rjust(width : Int) : Series` | Pads the text on the left. |
| `center` | `StringMethods.center(width : Int) : Series` | Centres a layer or an axis. |
| `zfill` | `StringMethods.zfill(width : Int) : Series` | Pads the number with leading zeros. |
| `contains` | `StringMethods.contains(part : String) : Series` | True where the text contains a part. |
| `startsWith` | `StringMethods.startsWith(part : String) : Series` | True where the text starts with a part. |
| `endsWith` | `StringMethods.endsWith(part : String) : Series` | True where the text ends with a part. |
| `isEmpty` | `StringMethods.isEmpty() : Series` | True when there are no values. |
| `len` | `StringMethods.len() : Series` | How many values. |
| `count` | `StringMethods.count(part : String) : Series` | How often a part appears in the text. |
| `find` | `StringMethods.find(part : String) : Series` | The first position where the predicate holds. |
| `rfind` | `StringMethods.rfind(part : String) : Series` | The last position of a part. |
| `get` | `StringMethods.get(position : Int) : Series` | The value at a key or label. |
| `splitPart` | `StringMethods.splitPart(separator : String, position : Int) : Series` | One part of a split. |

### DataFrame

| Member | Signature | What it does |
|---|---|---|
| `length` | `DataFrame.length() : Int` | How many values. |
| `width` | `DataFrame.width() : Int` | How many columns. |
| `shape` | `DataFrame.shape() : (Int, Int)` | The shape. |
| `isEmpty` | `DataFrame.isEmpty() : Bool` | True when there are no values. |
| `columns` | `DataFrame.columns() : List[String]` | The column names. |
| `hasColumn` | `DataFrame.hasColumn(name : String) : Bool` | True when the column exists. |
| `ndim` | `DataFrame.ndim() : Int` | The number of dimensions. |
| `size` | `DataFrame.size() : Int` | How many values. |
| `equals` | `DataFrame.equals(other : DataFrame) : Bool` | True when both values are equal. |
| `copy` | `DataFrame.copy() : DataFrame` | A value with the same contents. |
| `dtypes` | `DataFrame.dtypes() : Series` | The type of each column. |
| `keyColumn` | `DataFrame.keyColumn(name : String) : Column` | The column a group is keyed by. |
| `keyCodes` | `DataFrame.keyCodes(name : String) : (List[Int], Int)` | The integer code of each key. |
| `col` | `DataFrame.col(name : String) : Series or Error` | One column as a series. |
| `get` | `DataFrame.get(name : String) : Option[Series]` | The value at a key or label. |
| `items` | `DataFrame.items() : List[(String, Series)]` | The values with their labels. |
| `select` | `DataFrame.select(selected : List[String]) : DataFrame or Error` | A frame with the chosen columns. |
| `take` | `DataFrame.take(positions : List[Int]) : DataFrame` | The first values. |
| `head` | `DataFrame.head(n : Int) : DataFrame` | The first rows. |
| `tail` | `DataFrame.tail(n : Int) : DataFrame` | The last rows. |
| `slice` | `DataFrame.slice(start : Int, stop : Int) : DataFrame` | A contiguous part. |
| `iloc` | `DataFrame.iloc(positions : List[Int]) : DataFrame` | A row by position. |
| `takeGaps` | `DataFrame.takeGaps(positions : List[Int]) : DataFrame` | Keeps the values where the mask is true. |
| `iloc` | `DataFrame.iloc(position : Int) : Series` | A row by position. |
| `row` | `DataFrame.row(position : Int) : Series` | One row of a frame. |
| `at` | `DataFrame.at(row : Int, column : String) : Scalar or Error` | The value at a position or label. |
| `iat` | `DataFrame.iat(row : Int, column : Int) : Scalar or Error` | The value at a row and column position. |
| `iterrows` | `DataFrame.iterrows() : List[(Scalar, Series)]` | Iterates the rows as pairs. |
| `filter` | `DataFrame.filter(mask : Series) : DataFrame` | The rows whose mask is true. |
| `loc` | `DataFrame.loc(label : Int) : DataFrame` | A row or column by label. |
| `loc` | `DataFrame.loc(label : String) : DataFrame` | A row or column by label. |
| `sortValues` | `DataFrame.sortValues(by : String, ascending : Bool, naPosition : String) : DataFrame or Error` | A frame sorted by a column. |
| `sortValues` | `DataFrame.sortValues(by : List[String], ascending : Bool, naPosition : String) : DataFrame or Error` | A frame sorted by a column. |
| `sortValues` | `DataFrame.sortValues(by : List[String], ascending : List[Bool], naPosition : String) : DataFrame or Error` | A frame sorted by a column. |
| `sortIndex` | `DataFrame.sortIndex(ascending : Bool, naPosition : String) : DataFrame` | A frame sorted by its index. |
| `nlargest` | `DataFrame.nlargest(n : Int, by : String) : DataFrame or Error` | The n largest values. |
| `nsmallest` | `DataFrame.nsmallest(n : Int, by : String) : DataFrame or Error` | The n smallest values. |
| `drop` | `DataFrame.drop(column : String) : DataFrame` | A frame without the named columns. |
| `drop` | `DataFrame.drop(removed : List[String]) : DataFrame` | A frame without the named columns. |
| `dropRows` | `DataFrame.dropRows(positions : List[Int]) : DataFrame` | A frame without the rows at the given positions. |
| `rename` | `DataFrame.rename(old : String, replacement : String) : DataFrame` | A copy with a value renamed. |
| `renameColumns` | `DataFrame.renameColumns(mapping : Map[String, String]) : DataFrame` | A copy with columns renamed from a map. |
| `assign` | `DataFrame.assign(name : String, series : Series) : DataFrame` | Adds or replaces a column. |
| `assign` | `DataFrame.assign(name : String, values : List[Int]) : DataFrame` | Adds or replaces a column. |
| `assign` | `DataFrame.assign(name : String, values : List[double]) : DataFrame` | Adds or replaces a column. |
| `assign` | `DataFrame.assign(name : String, values : List[String]) : DataFrame` | Adds or replaces a column. |
| `assign` | `DataFrame.assign(name : String, values : List[Bool]) : DataFrame` | Adds or replaces a column. |
| `assign` | `DataFrame.assign(name : String, value : Int) : DataFrame` | Adds or replaces a column. |
| `assign` | `DataFrame.assign(name : String, value : double) : DataFrame` | Adds or replaces a column. |
| `assign` | `DataFrame.assign(name : String, value : String) : DataFrame` | Adds or replaces a column. |
| `assign` | `DataFrame.assign(name : String, value : Bool) : DataFrame` | Adds or replaces a column. |
| `setIndex` | `DataFrame.setIndex(name : String) : DataFrame or Error` | A frame with a new index. |
| `resetIndex` | `DataFrame.resetIndex(drop : Bool) : DataFrame` | A frame with a fresh row index. |
| `astype` | `DataFrame.astype(dtype : String) : DataFrame` | Converts the values to another type. |
| `selectDtypes` | `DataFrame.selectDtypes(dtype : String) : DataFrame` | The columns of the given types. |
| `round` | `DataFrame.round(decimals : Int) : DataFrame` | Rounds each value. |
| `abs` | `DataFrame.abs() : DataFrame` | The absolute value. |
| `clip` | `DataFrame.clip(low : double, high : double) : DataFrame` | Limits values to a range. |
| `shift` | `DataFrame.shift(periods : Int) : DataFrame` | The values moved by n positions. |
| `diff` | `DataFrame.diff(periods : Int) : DataFrame` | The difference between consecutive values. |
| `pctChange` | `DataFrame.pctChange(periods : Int) : DataFrame` | The fractional change between consecutive values. |
| `cumSum` | `DataFrame.cumSum() : DataFrame` | The running sum. |
| `cumProd` | `DataFrame.cumProd() : DataFrame` | The running product. |
| `cumMax` | `DataFrame.cumMax() : DataFrame` | The running maximum. |
| `cumMin` | `DataFrame.cumMin() : DataFrame` | The running minimum. |
| `rank` | `DataFrame.rank(ascending : Bool, method : String, pct : Bool) : DataFrame` | The rank of each value. |
| `interpolate` | `DataFrame.interpolate() : DataFrame` | Fills missing values from their neighbours. |
| `where` | `DataFrame.where(mask : DataFrame, other : Scalar) : DataFrame` | The values kept where a condition holds. |
| `where` | `DataFrame.where(mask : DataFrame) : DataFrame` | The values kept where a condition holds. |
| `mask` | `DataFrame.mask(mask : DataFrame, other : Scalar) : DataFrame` | Replaces the values where a condition holds. |
| `mask` | `DataFrame.mask(mask : DataFrame) : DataFrame` | Replaces the values where a condition holds. |
| `replace` | `DataFrame.replace(old : String, replacement : String) : DataFrame` | Values replaced. |
| `replace` | `DataFrame.replace(old : Int, replacement : Int) : DataFrame` | Values replaced. |
| `replace` | `DataFrame.replace(old : double, replacement : double) : DataFrame` | Values replaced. |
| `replace` | `DataFrame.replace(old : Bool, replacement : Bool) : DataFrame` | Values replaced. |
| `replace` | `DataFrame.replace(old : Scalar, replacement : Scalar) : DataFrame` | Values replaced. |
| `apply` | `DataFrame.apply(f : double -> double) : DataFrame` | Applies a function to the numeric columns. |
| `insert` | `DataFrame.insert(position : Int, name : String, series : Series) : DataFrame or Error` | Adds a column at a position. |
| `isNa` | `DataFrame.isNa() : DataFrame` | True where the value is missing. |
| `notNa` | `DataFrame.notNa() : DataFrame` | True where the value is present. |
| `fillNa` | `DataFrame.fillNa(value : double) : DataFrame` | Replaces missing values. |
| `fillNa` | `DataFrame.fillNa(value : Int) : DataFrame` | Replaces missing values. |
| `fillNa` | `DataFrame.fillNa(value : String) : DataFrame` | Replaces missing values. |
| `fillNa` | `DataFrame.fillNa(value : Bool) : DataFrame` | Replaces missing values. |
| `dropNa` | `DataFrame.dropNa(how : String, thresh : Int) : DataFrame` | A frame without rows that hold a missing value. |
| `dropNa` | `DataFrame.dropNa(subset : List[String], how : String, thresh : Int) : DataFrame or Error` | A frame without rows that hold a missing value. |
| `duplicated` | `DataFrame.duplicated(keep : String) : Series` | True where a row repeats an earlier one. |
| `dropDuplicates` | `DataFrame.dropDuplicates(keep : String) : DataFrame` | A frame without repeated rows. |
| `dropDuplicates` | `DataFrame.dropDuplicates(subset : List[String], keep : String) : DataFrame or Error` | A frame without repeated rows. |
| `agg` | `DataFrame.agg(statistic : String) : Series` | Aggregates each column with named statistics. |
| `sum` | `DataFrame.sum() : Series` | The sum. |
| `prod` | `DataFrame.prod() : Series` | The product. |
| `mean` | `DataFrame.mean() : Series` | The mean. |
| `median` | `DataFrame.median() : Series` | The median. |
| `min` | `DataFrame.min() : Series` | The smallest value. |
| `max` | `DataFrame.max() : Series` | The largest value. |
| `count` | `DataFrame.count() : Series` | The number of non-missing values. |
| `std` | `DataFrame.std() : Series` | The standard deviation. |
| `variance` | `DataFrame.variance() : Series` | The variance. |
| `sem` | `DataFrame.sem() : Series` | The standard error of the mean. |
| `skew` | `DataFrame.skew() : Series` | The skewness. |
| `kurt` | `DataFrame.kurt() : Series` | The kurtosis. |
| `any` | `DataFrame.any() : Series` | True when at least one value satisfies the predicate. |
| `all` | `DataFrame.all() : Series` | True when every value satisfies the predicate. |
| `nunique` | `DataFrame.nunique() : Series` | How many distinct values. |
| `idxMax` | `DataFrame.idxMax() : Series` | The position of the largest value. |
| `idxMin` | `DataFrame.idxMin() : Series` | The position of the smallest value. |
| `mode` | `DataFrame.mode() : DataFrame` | The most frequent value. |
| `agg` | `DataFrame.agg(spec : List[(String, String)]) : Series` | Aggregates each column with named statistics. |
| `quantile` | `DataFrame.quantile(q : double) : Series` | The value at a fraction of the sorted data. |
| `quantile` | `DataFrame.quantile(qs : List[double]) : DataFrame` | The value at a fraction of the sorted data. |
| `describe` | `DataFrame.describe() : DataFrame` | Summary statistics per column. |
| `corr` | `DataFrame.corr() : DataFrame` | The correlation with another series. |
| `cov` | `DataFrame.cov(ddof : Int) : DataFrame` | The covariance with another series. |
| `corrwith` | `DataFrame.corrwith(other : DataFrame) : Series` | The correlation of each column with another series. |
| `valueCounts` | `DataFrame.valueCounts(column : String, normalize : Bool) : Series or Error` | How often each distinct value appears. |
| `valueCounts` | `DataFrame.valueCounts(subset : List[String], normalize : Bool) : DataFrame or Error` | How often each distinct value appears. |
| `groupBy` | `DataFrame.groupBy(key : String) : GroupBy or Error` | Splits the frame by one or more columns. |
| `groupBy` | `DataFrame.groupBy(keys : List[String]) : GroupBy or Error` | Splits the frame by one or more columns. |
| `merge` | `DataFrame.merge(other : DataFrame, on : String, how : String, leftSuffix : String, rightSuffix : String) : DataFrame or Error` | Joins two frames on columns. |
| `merge` | `DataFrame.merge(other : DataFrame, on : List[String], how : String, leftSuffix : String, rightSuffix : String) : DataFrame or Error` | Joins two frames on columns. |
| `merge` | `DataFrame.merge(other : DataFrame, leftOn : String, rightOn : String, how : String, leftSuffix : String, rightSuffix : String) : DataFrame or Error` | Joins two frames on columns. |
| `append` | `DataFrame.append(other : DataFrame) : DataFrame` | Appends the rows of another frame. |
| `pivotTable` | `DataFrame.pivotTable(index : String, columns : String, values : String, aggfunc : String) : DataFrame or Error` | A pivot table with an aggregation. |
| `melt` | `DataFrame.melt(idVars : List[String], valueVars : List[String], varName : String, valueName : String) : DataFrame or Error` | The frame in long form. |
| `pivot` | `DataFrame.pivot(index : String, columns : String, values : String) : DataFrame or Error` | The frame in wide form. |
| `crosstab` | `DataFrame.crosstab(index : String, columns : String) : DataFrame or Error` | A table of counts for two columns. |
| `getDummies` | `DataFrame.getDummies(columns : List[String], prefix : String, dropFirst : Bool, dummyNa : Bool) : DataFrame or Error` | One-hot columns for a categorical column. |
| `transpose` | `DataFrame.transpose() : DataFrame or Error` | The transpose. |
| `toTensor` | `DataFrame.toTensor() : DynTensor[double]` | The values as a tensor. |
| `toCsv` | `DataFrame.toCsv(separator : String, header : Bool, includeIndex : Bool) : String` | The frame as CSV text. |
| `text` | `DataFrame.text() : String` | The value as text. |
| `html` | `DataFrame.html() : String` | The frame as an HTML table. |
| `cell` | `DataFrame.cell(row : Int, column : String) : Scalar or Error` | One cell of a frame. |
| `mapNumeric` | `DataFrame.mapNumeric(f : double -> double) : DataFrame` | Applies a function to the numeric columns only. |
| `rolling` | `DataFrame.rolling(window : Int, minPeriods : Int) : FrameRolling` | A rolling window over the series. |
| `expanding` | `DataFrame.expanding(minPeriods : Int) : FrameExpanding` | A window from the first row to the current one. |
| `ewm` | `DataFrame.ewm(alpha : double, adjust : Bool, minPeriods : Int) : FrameEWM` | An exponentially weighted window. |
| `saveCsv` | `DataFrame.saveCsv(path : String, separator : String, includeIndex : Bool) : void or Error with Effects` | Writes the frame as a CSV file. |
| `info` | `DataFrame.info() : void with Effects` | A summary of the columns and their types. |

### FrameRolling

| Member | Signature | What it does |
|---|---|---|
| `mean` | `FrameRolling.mean() : DataFrame` | The mean. |
| `sum` | `FrameRolling.sum() : DataFrame` | The sum. |
| `min` | `FrameRolling.min() : DataFrame` | The smallest value. |
| `max` | `FrameRolling.max() : DataFrame` | The largest value. |
| `std` | `FrameRolling.std() : DataFrame` | The standard deviation. |
| `count` | `FrameRolling.count() : DataFrame` | The number of non-missing values. |
| `median` | `FrameRolling.median() : DataFrame` | The median. |
| `variance` | `FrameRolling.variance() : DataFrame` | The variance. |
| `sem` | `FrameRolling.sem() : DataFrame` | The standard error of the mean. |
| `skew` | `FrameRolling.skew() : DataFrame` | The skewness. |
| `kurt` | `FrameRolling.kurt() : DataFrame` | The kurtosis. |
| `quantile` | `FrameRolling.quantile(q : double) : DataFrame` | The value at a fraction of the sorted data. |

### FrameExpanding

| Member | Signature | What it does |
|---|---|---|
| `mean` | `FrameExpanding.mean() : DataFrame` | The mean. |
| `sum` | `FrameExpanding.sum() : DataFrame` | The sum. |
| `min` | `FrameExpanding.min() : DataFrame` | The smallest value. |
| `max` | `FrameExpanding.max() : DataFrame` | The largest value. |
| `std` | `FrameExpanding.std() : DataFrame` | The standard deviation. |
| `count` | `FrameExpanding.count() : DataFrame` | The number of non-missing values. |
| `median` | `FrameExpanding.median() : DataFrame` | The median. |
| `variance` | `FrameExpanding.variance() : DataFrame` | The variance. |
| `sem` | `FrameExpanding.sem() : DataFrame` | The standard error of the mean. |
| `quantile` | `FrameExpanding.quantile(q : double) : DataFrame` | The value at a fraction of the sorted data. |

### FrameEWM

| Member | Signature | What it does |
|---|---|---|
| `mean` | `FrameEWM.mean() : DataFrame` | The mean. |
| `sum` | `FrameEWM.sum() : DataFrame` | The sum. |
| `variance` | `FrameEWM.variance(bias : Bool) : DataFrame` | The variance. |
| `std` | `FrameEWM.std(bias : Bool) : DataFrame` | The standard deviation. |
| `count` | `FrameEWM.count() : DataFrame` | The number of non-missing values. |

### GroupBy

| Member | Signature | What it does |
|---|---|---|
| `count` | `GroupBy.count() : Int` | The number of non-missing values. |
| `aggregate` | `GroupBy.aggregate(statistic : String) : DataFrame` | Aggregates the columns of a grouped frame. |
| `sum` | `GroupBy.sum() : DataFrame` | The sum. |
| `mean` | `GroupBy.mean() : DataFrame` | The mean. |
| `median` | `GroupBy.median() : DataFrame` | The median. |
| `min` | `GroupBy.min() : DataFrame` | The smallest value. |
| `max` | `GroupBy.max() : DataFrame` | The largest value. |
| `std` | `GroupBy.std() : DataFrame` | The standard deviation. |
| `variance` | `GroupBy.variance() : DataFrame` | The variance. |
| `first` | `GroupBy.first() : DataFrame` | The first value. |
| `last` | `GroupBy.last() : DataFrame` | The last value. |
| `nunique` | `GroupBy.nunique() : DataFrame` | How many distinct values. |
| `prod` | `GroupBy.prod() : DataFrame` | The product. |
| `counts` | `GroupBy.counts() : DataFrame` | The number of non-missing values per row or group. |
| `size` | `GroupBy.size() : Series` | How many values. |
| `head` | `GroupBy.head(n : Int) : DataFrame` | The first rows. |
| `tail` | `GroupBy.tail(n : Int) : DataFrame` | The last rows. |
| `nth` | `GroupBy.nth(n : Int) : DataFrame` | The value at a position. |
| `cumCount` | `GroupBy.cumCount(ascending : Bool) : Series` | The running count. |
| `ngroup` | `GroupBy.ngroup() : Series` | How many groups there are. |
| `getGroup` | `GroupBy.getGroup(label : Int) : DataFrame or Error` | One group of a grouped frame. |
| `getGroup` | `GroupBy.getGroup(label : String) : DataFrame or Error` | One group of a grouped frame. |
| `getGroup` | `GroupBy.getGroup(label : double) : DataFrame or Error` | One group of a grouped frame. |
| `getGroup` | `GroupBy.getGroup(label : Bool) : DataFrame or Error` | One group of a grouped frame. |
| `transformAll` | `GroupBy.transformAll(statistic : String) : DataFrame` | Transforms every column. |
| `shift` | `GroupBy.shift(column : String, periods : Int) : Series` | The values moved by n positions. |
| `diff` | `GroupBy.diff(column : String, periods : Int) : Series` | The difference between consecutive values. |
| `pctChange` | `GroupBy.pctChange(column : String, periods : Int) : Series` | The fractional change between consecutive values. |
| `cumSum` | `GroupBy.cumSum(column : String) : Series` | The running sum. |
| `cumProd` | `GroupBy.cumProd(column : String) : Series` | The running product. |
| `cumMax` | `GroupBy.cumMax(column : String) : Series` | The running maximum. |
| `cumMin` | `GroupBy.cumMin(column : String) : Series` | The running minimum. |
| `rank` | `GroupBy.rank(column : String, ascending : Bool, method : String, pct : Bool) : Series` | The rank of each value. |
| `sem` | `GroupBy.sem() : DataFrame` | The standard error of the mean. |
| `skew` | `GroupBy.skew() : DataFrame` | The skewness. |
| `kurt` | `GroupBy.kurt() : DataFrame` | The kurtosis. |
| `all` | `GroupBy.all() : DataFrame` | True when every value satisfies the predicate. |
| `any` | `GroupBy.any() : DataFrame` | True when at least one value satisfies the predicate. |
| `quantile` | `GroupBy.quantile(q : double) : DataFrame` | The value at a fraction of the sorted data. |
| `agg` | `GroupBy.agg(spec : List[(String, String)]) : DataFrame` | Aggregates each column with named statistics. |
| `filterGroups` | `GroupBy.filterGroups(keep : Int -> Bool) : DataFrame` | Keeps the groups whose aggregate passes a test. |
| `transform` | `GroupBy.transform(column : String, statistic : String) : Series` | Transforms new data with the fitted parameters. |
