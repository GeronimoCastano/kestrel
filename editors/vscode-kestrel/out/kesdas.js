'use strict';

// Generated metadata for std.kesdas. The compiler and src/kesdas.kes remain the authority.
// Regenerate this table when public members change.
const types = [
  "Column",
  "DataError",
  "DataFrame",
  "EWM",
  "Expanding",
  "FrameRolling",
  "GroupBy",
  "Rolling",
  "Scalar",
  "Series",
  "StringMethods",
  "FrameEWM",
  "FrameExpanding"
];

const names = [
  "Column",
  "DataError",
  "DataFrame",
  "EWM",
  "Expanding",
  "FrameRolling",
  "GroupBy",
  "Rolling",
  "Scalar",
  "Series",
  "StringMethods",
  "FrameEWM",
  "FrameExpanding",
  "readCsv"
];

const entries = [
  {
    "name": "tile",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "signature": "Column.tile(times : Int) : Column",
    "parameters": [
      "times : Int"
    ],
    "documentation": "Repeats this column's values."
  },
  {
    "name": "concat",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "signature": "Column.concat(other : Column) : Column",
    "parameters": [
      "other : Column"
    ],
    "documentation": "Joins another column after this one."
  },
  {
    "name": "ids",
    "module": "std.kesdas",
    "gated": true,
    "static": "Column",
    "signature": "Column.ids(count : Int) : Column",
    "parameters": [
      "count : Int"
    ],
    "documentation": "A column of consecutive integer identifiers."
  },
  {
    "name": "Column",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "Column",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "Scalar",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "Scalar",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "Series",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "Series",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "Rolling",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "Rolling",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "StringMethods",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "StringMethods",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "DataFrame",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "DataFrame",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "GroupBy",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "GroupBy",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "DataError",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "DataError",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "MissingColumn",
    "module": "std.kesdas",
    "gated": true,
    "static": "DataError",
    "signature": "DataError.MissingColumn(name : String) : DataError",
    "parameters": [
      "name : String"
    ],
    "documentation": "A frame operation named a column that is not there."
  },
  {
    "name": "BadArgument",
    "module": "std.kesdas",
    "gated": true,
    "static": "DataError",
    "signature": "DataError.BadArgument(message : String) : DataError",
    "parameters": [
      "message : String"
    ],
    "documentation": "A frame operation received an unusable argument."
  },
  {
    "name": "Column",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "Column(kind : Int, ints : List[Int], floats : List[double], bools : List[Bool], strs : List[String], missing : List[Bool], size : Int) : Column",
    "parameters": [
      "kind : Int",
      "ints : List[Int]",
      "floats : List[double]",
      "bools : List[Bool]",
      "strs : List[String]",
      "missing : List[Bool]",
      "size : Int"
    ],
    "documentation": "A typed column with a mask of missing entries. Most code gets a Column from a Series."
  },
  {
    "name": "ints",
    "module": "std.kesdas",
    "gated": true,
    "static": "Column",
    "signature": "Column.ints(values : List[Int]) : Column",
    "parameters": [
      "values : List[Int]"
    ],
    "documentation": "An Int column with no missing entries."
  },
  {
    "name": "floats",
    "module": "std.kesdas",
    "gated": true,
    "static": "Column",
    "signature": "Column.floats(values : List[double]) : Column",
    "parameters": [
      "values : List[double]"
    ],
    "documentation": "A double column with no missing entries."
  },
  {
    "name": "bools",
    "module": "std.kesdas",
    "gated": true,
    "static": "Column",
    "signature": "Column.bools(values : List[Bool]) : Column",
    "parameters": [
      "values : List[Bool]"
    ],
    "documentation": "A Bool column with no missing entries."
  },
  {
    "name": "strings",
    "module": "std.kesdas",
    "gated": true,
    "static": "Column",
    "signature": "Column.strings(values : List[String]) : Column",
    "parameters": [
      "values : List[String]"
    ],
    "documentation": "A String column with no missing entries."
  },
  {
    "name": "ofInts",
    "module": "std.kesdas",
    "gated": true,
    "static": "Column",
    "signature": "Column.ofInts(values : List[Int], missing : List[Bool]) : Column",
    "parameters": [
      "values : List[Int]",
      "missing : List[Bool]"
    ],
    "documentation": "An Int column with an explicit missing mask. A missing entry promotes the column to double, as in pandas."
  },
  {
    "name": "ofFloats",
    "module": "std.kesdas",
    "gated": true,
    "static": "Column",
    "signature": "Column.ofFloats(values : List[double], missing : List[Bool]) : Column",
    "parameters": [
      "values : List[double]",
      "missing : List[Bool]"
    ],
    "documentation": "A double column with an explicit missing mask."
  },
  {
    "name": "ofBools",
    "module": "std.kesdas",
    "gated": true,
    "static": "Column",
    "signature": "Column.ofBools(values : List[Bool], missing : List[Bool]) : Column",
    "parameters": [
      "values : List[Bool]",
      "missing : List[Bool]"
    ],
    "documentation": "A Bool column with an explicit missing mask."
  },
  {
    "name": "ofStrings",
    "module": "std.kesdas",
    "gated": true,
    "static": "Column",
    "signature": "Column.ofStrings(values : List[String], missing : List[Bool]) : Column",
    "parameters": [
      "values : List[String]",
      "missing : List[Bool]"
    ],
    "documentation": "A String column with an explicit missing mask."
  },
  {
    "name": "noneMissing",
    "module": "std.kesdas",
    "gated": true,
    "static": "Column",
    "signature": "Column.noneMissing(n : Int) : List[Bool]",
    "parameters": [
      "n : Int"
    ],
    "documentation": "A mask of n false entries."
  },
  {
    "name": "empty",
    "module": "std.kesdas",
    "gated": true,
    "static": "Column",
    "signature": "Column.empty(kind : Int, size : Int) : Column",
    "parameters": [
      "kind : Int",
      "size : Int"
    ],
    "documentation": "A column of one kind whose entries are all missing."
  },
  {
    "name": "kind",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "field": true,
    "signature": "Column.kind : Int",
    "parameters": [],
    "documentation": "KIND_INT, KIND_FLOAT, KIND_BOOL or KIND_STR."
  },
  {
    "name": "ints",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "field": true,
    "signature": "Column.ints : List[Int]",
    "parameters": [],
    "documentation": "Int storage; empty for another kind."
  },
  {
    "name": "floats",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "field": true,
    "signature": "Column.floats : List[double]",
    "parameters": [],
    "documentation": "double storage; empty for another kind."
  },
  {
    "name": "bools",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "field": true,
    "signature": "Column.bools : List[Bool]",
    "parameters": [],
    "documentation": "Bool storage; empty for another kind."
  },
  {
    "name": "strs",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "field": true,
    "signature": "Column.strs : List[String]",
    "parameters": [],
    "documentation": "String storage; empty for another kind."
  },
  {
    "name": "missing",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "field": true,
    "signature": "Column.missing : List[Bool]",
    "parameters": [],
    "documentation": "One flag per entry: true means the entry is missing."
  },
  {
    "name": "size",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "field": true,
    "signature": "Column.size : Int",
    "parameters": [],
    "documentation": "The number of entries."
  },
  {
    "name": "copy",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "signature": "Column.copy() : Column",
    "parameters": [],
    "documentation": "An independent copy of the storage and mask."
  },
  {
    "name": "dtype",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "signature": "Column.dtype() : String",
    "parameters": [],
    "documentation": "The pandas-style dtype: int64, float64, bool or str."
  },
  {
    "name": "isNumeric",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "signature": "Column.isNumeric() : Bool",
    "parameters": [],
    "documentation": "Whether the kind is Int or double."
  },
  {
    "name": "gather",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "signature": "Column.gather(positions : List[Int]) : Column",
    "parameters": [
      "positions : List[Int]"
    ],
    "documentation": "A new column holding the given row positions, in that order."
  },
  {
    "module": "std.kesdas",
    "gated": true,
    "name": "orderPositions",
    "signature": "Column.orderPositions(start : List[Int], ascending : Bool, naFirst : Bool) : List[Int]",
    "parameters": [
      "start : List[Int]",
      "ascending : Bool",
      "naFirst : Bool"
    ],
    "documentation": "Stable positions sorted by this column, with missing entries first or last.",
    "receiver": "Column"
  },
  {
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "name": "topPositions",
    "signature": "Column.topPositions(n : Int, ascending : Bool) : List[Int]",
    "parameters": [
      "n : Int",
      "ascending : Bool"
    ],
    "documentation": "The n largest or smallest present row positions, in rank order."
  },
  {
    "name": "head",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "signature": "Column.head(n : Int) : Column",
    "parameters": [
      "n : Int"
    ],
    "documentation": "The first n entries."
  },
  {
    "name": "join",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "signature": "Column.join(other : Column) : Column",
    "parameters": [
      "other : Column"
    ],
    "documentation": "Appends another column of the same kind below this one."
  },
  {
    "name": "toFloats",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "signature": "Column.toFloats() : List[double]",
    "parameters": [],
    "documentation": "The numbers as doubles; missing entries become NaN."
  },
  {
    "name": "texts",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "signature": "Column.texts() : List[String]",
    "parameters": [],
    "documentation": "One display string per entry, with NaN for a missing entry."
  },
  {
    "name": "keys",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Column",
    "signature": "Column.keys() : List[String]",
    "parameters": [],
    "documentation": "One comparable key per entry; equal values give equal keys."
  },
  {
    "name": "Scalar",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "Scalar(kind : Int, whole : Int, real : double, flag : Bool, word : String) : Scalar",
    "parameters": [
      "kind : Int",
      "whole : Int",
      "real : double",
      "flag : Bool",
      "word : String"
    ],
    "documentation": "One cell lifted out of a column: a number, a flag, a text, or a missing entry."
  },
  {
    "name": "ofInt",
    "module": "std.kesdas",
    "gated": true,
    "static": "Scalar",
    "signature": "Scalar.ofInt(value : Int) : Scalar",
    "parameters": [
      "value : Int"
    ],
    "documentation": "A scalar holding an Int."
  },
  {
    "name": "ofFloat",
    "module": "std.kesdas",
    "gated": true,
    "static": "Scalar",
    "signature": "Scalar.ofFloat(value : double) : Scalar",
    "parameters": [
      "value : double"
    ],
    "documentation": "A scalar holding a double; NaN becomes a missing scalar."
  },
  {
    "name": "ofBool",
    "module": "std.kesdas",
    "gated": true,
    "static": "Scalar",
    "signature": "Scalar.ofBool(value : Bool) : Scalar",
    "parameters": [
      "value : Bool"
    ],
    "documentation": "A scalar holding a Bool."
  },
  {
    "name": "ofString",
    "module": "std.kesdas",
    "gated": true,
    "static": "Scalar",
    "signature": "Scalar.ofString(value : String) : Scalar",
    "parameters": [
      "value : String"
    ],
    "documentation": "A scalar holding a String."
  },
  {
    "name": "missing",
    "module": "std.kesdas",
    "gated": true,
    "static": "Scalar",
    "signature": "Scalar.missing() : Scalar",
    "parameters": [],
    "documentation": "A missing scalar."
  },
  {
    "name": "kind",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "field": true,
    "signature": "Scalar.kind : Int",
    "parameters": [],
    "documentation": "The cell kind, or a negative value when missing."
  },
  {
    "name": "whole",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "field": true,
    "signature": "Scalar.whole : Int",
    "parameters": [],
    "documentation": "Int storage."
  },
  {
    "name": "real",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "field": true,
    "signature": "Scalar.real : double",
    "parameters": [],
    "documentation": "double storage."
  },
  {
    "name": "flag",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "field": true,
    "signature": "Scalar.flag : Bool",
    "parameters": [],
    "documentation": "Bool storage."
  },
  {
    "name": "word",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "field": true,
    "signature": "Scalar.word : String",
    "parameters": [],
    "documentation": "String storage."
  },
  {
    "name": "isMissing",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "signature": "Scalar.isMissing() : Bool",
    "parameters": [],
    "documentation": "Whether this scalar is a missing entry."
  },
  {
    "name": "isNumeric",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "signature": "Scalar.isNumeric() : Bool",
    "parameters": [],
    "documentation": "Whether the kind is Int or double."
  },
  {
    "name": "asDouble",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "signature": "Scalar.asDouble() : double",
    "parameters": [],
    "documentation": "The value as a double; a non-number is NaN."
  },
  {
    "name": "asInt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "signature": "Scalar.asInt() : Int",
    "parameters": [],
    "documentation": "The value as an Int, rounding a double down."
  },
  {
    "name": "asString",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "signature": "Scalar.asString() : String",
    "parameters": [],
    "documentation": "The same text that text() gives."
  },
  {
    "name": "asBool",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "signature": "Scalar.asBool() : Bool",
    "parameters": [],
    "documentation": "True only for a Bool scalar holding true."
  },
  {
    "name": "text",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "signature": "Scalar.text() : String",
    "parameters": [],
    "documentation": "The display text, with NaN for a missing entry."
  },
  {
    "name": "equals",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Scalar",
    "signature": "Scalar.equals(other : Scalar) : Bool",
    "parameters": [
      "other : Scalar"
    ],
    "documentation": "Whether two scalars have the same kind and value."
  },
  {
    "name": "Series",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "Series(name : String, move data : Column, move index : Column, indexName : String) : Series",
    "parameters": [
      "name : String",
      "move data : Column",
      "move index : Column",
      "indexName : String"
    ],
    "documentation": "A Series from Int values, with a default 0..n-1 index."
  },
  {
    "name": "Series",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "Series(values : List[Int], name : String = \"\") : Series",
    "parameters": [
      "values : List[Int]",
      "name : String = \"\""
    ],
    "documentation": "A Series from double values."
  },
  {
    "name": "Series",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "Series(values : List[double], name : String = \"\") : Series",
    "parameters": [
      "values : List[double]",
      "name : String = \"\""
    ],
    "documentation": "A Series from String values."
  },
  {
    "name": "Series",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "Series(values : List[String], name : String = \"\") : Series",
    "parameters": [
      "values : List[String]",
      "name : String = \"\""
    ],
    "documentation": "A Series from Bool values."
  },
  {
    "name": "Series",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "Series(values : List[Bool], name : String = \"\") : Series",
    "parameters": [
      "values : List[Bool]",
      "name : String = \"\""
    ],
    "documentation": "A Series from explicit storage, index and names. The data and index move into the new Series."
  },
  {
    "name": "name",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "field": true,
    "signature": "Series.name : String",
    "parameters": [],
    "documentation": "The name printed above the values."
  },
  {
    "name": "data",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "field": true,
    "signature": "Series.data : Column",
    "parameters": [],
    "documentation": "The column of values."
  },
  {
    "name": "index",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "field": true,
    "signature": "Series.index : Column",
    "parameters": [],
    "documentation": "The row labels."
  },
  {
    "name": "indexName",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "field": true,
    "signature": "Series.indexName : String",
    "parameters": [],
    "documentation": "The name printed over the index."
  },
  {
    "name": "copy",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.copy() : Series",
    "parameters": [],
    "documentation": "An independent copy of the data and index."
  },
  {
    "name": "length",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.length() : Int",
    "parameters": [],
    "documentation": "The number of values."
  },
  {
    "name": "isEmpty",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.isEmpty() : Bool",
    "parameters": [],
    "documentation": "Whether the Series has no values."
  },
  {
    "name": "dtype",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.dtype() : String",
    "parameters": [],
    "documentation": "The pandas-style dtype: int64, float64, bool or str."
  },
  {
    "name": "derive",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.derive(move data : Column) : Series",
    "parameters": [
      "move data : Column"
    ],
    "documentation": "A Series with new data and this Series name and index. The column moves into the result."
  },
  {
    "name": "take",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.take(positions : List[Int]) : Series",
    "parameters": [
      "positions : List[Int]"
    ],
    "documentation": "Rows at the given positions, in that order."
  },
  {
    "name": "rename",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.rename(name : String) : Series",
    "parameters": [
      "name : String"
    ],
    "documentation": "A copy with a new Series name."
  },
  {
    "name": "withIndex",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.withIndex(labels : List[String]) : Series",
    "parameters": [
      "labels : List[String]"
    ],
    "documentation": "A copy with String row labels."
  },
  {
    "name": "withIndex",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.withIndex(labels : List[Int]) : Series",
    "parameters": [
      "labels : List[Int]"
    ],
    "documentation": "A copy with Int row labels."
  },
  {
    "name": "withIndexColumn",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.withIndexColumn(labels : Column) : Series",
    "parameters": [
      "labels : Column"
    ],
    "documentation": "A copy with explicit index labels."
  },
  {
    "name": "named",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.named(indexName : String) : Series",
    "parameters": [
      "indexName : String"
    ],
    "documentation": "A copy with a name for the index."
  },
  {
    "name": "toInts",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.toInts() : List[Int]",
    "parameters": [],
    "documentation": "The values as Ints."
  },
  {
    "name": "toFloats",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.toFloats() : List[double]",
    "parameters": [],
    "documentation": "The values as doubles; missing entries become NaN."
  },
  {
    "name": "toStrings",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.toStrings() : List[String]",
    "parameters": [],
    "documentation": "The display text of every value."
  },
  {
    "name": "toBools",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.toBools() : List[Bool]",
    "parameters": [],
    "documentation": "The values as Bools."
  },
  {
    "name": "indexLabels",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.indexLabels() : List[String]",
    "parameters": [],
    "documentation": "The index labels as text."
  },
  {
    "name": "missingMask",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.missingMask() : List[Bool]",
    "parameters": [],
    "documentation": "One flag per value: true means missing."
  },
  {
    "name": "astype",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.astype(dtype : String) : Series",
    "parameters": [
      "dtype : String"
    ],
    "documentation": "Converts the values to int64, float64, bool or str."
  },
  {
    "name": "head",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.head(n : Int = 5) : Series",
    "parameters": [
      "n : Int = 5"
    ],
    "documentation": "The first n rows."
  },
  {
    "name": "tail",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.tail(n : Int = 5) : Series",
    "parameters": [
      "n : Int = 5"
    ],
    "documentation": "The last n rows."
  },
  {
    "name": "slice",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.slice(start : Int, stop : Int) : Series",
    "parameters": [
      "start : Int",
      "stop : Int"
    ],
    "documentation": "Rows in the half-open range start to stop."
  },
  {
    "name": "iloc",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.iloc(position : Int) : Scalar",
    "parameters": [
      "position : Int"
    ],
    "documentation": "One value by row position."
  },
  {
    "name": "iloc",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.iloc(positions : List[Int]) : Series",
    "parameters": [
      "positions : List[Int]"
    ],
    "documentation": "Rows at the given positions."
  },
  {
    "name": "positionOf",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.positionOf(label : Int) : Option[Int]",
    "parameters": [
      "label : Int"
    ],
    "documentation": "The row position of an Int index label."
  },
  {
    "name": "positionOf",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.positionOf(label : String) : Option[Int]",
    "parameters": [
      "label : String"
    ],
    "documentation": "The row position of a String index label."
  },
  {
    "name": "loc",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.loc(label : Int) : Scalar",
    "parameters": [
      "label : Int"
    ],
    "documentation": "One value by Int index label; a missing label gives a missing scalar."
  },
  {
    "name": "loc",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.loc(label : String) : Scalar",
    "parameters": [
      "label : String"
    ],
    "documentation": "One value by String index label; a missing label gives a missing scalar."
  },
  {
    "name": "filter",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.filter(mask : Series) : Series",
    "parameters": [
      "mask : Series"
    ],
    "documentation": "Keeps the rows where the Bool mask is true."
  },
  {
    "name": "where",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.where(mask : Series, other : Scalar) : Series",
    "parameters": [
      "mask : Series",
      "other : Scalar"
    ],
    "documentation": "Keeps a value where the mask is true and fills the other rows with a scalar."
  },
  {
    "name": "sortValues",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.sortValues(ascending : Bool = true, naPosition : String = \"last\") : Series",
    "parameters": [
      "ascending : Bool = true",
      "naPosition : String = \"last\""
    ],
    "documentation": "Sorts by value."
  },
  {
    "name": "sortIndex",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.sortIndex(ascending : Bool = true, naPosition : String = \"last\") : Series",
    "parameters": [
      "ascending : Bool = true",
      "naPosition : String = \"last\""
    ],
    "documentation": "Sorts by index label."
  },
  {
    "name": "nlargest",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.nlargest(n : Int = 5) : Series",
    "parameters": [
      "n : Int = 5"
    ],
    "documentation": "The n largest values, largest first."
  },
  {
    "name": "nsmallest",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.nsmallest(n : Int = 5) : Series",
    "parameters": [
      "n : Int = 5"
    ],
    "documentation": "The n smallest values, smallest first."
  },
  {
    "name": "resetIndex",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.resetIndex() : Series",
    "parameters": [],
    "documentation": "Replaces the index with 0..n-1."
  },
  {
    "name": "add",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.add(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise addition with another Series."
  },
  {
    "name": "subtract",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.subtract(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise subtraction with another Series."
  },
  {
    "name": "multiply",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.multiply(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise multiplication with another Series."
  },
  {
    "name": "divide",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.divide(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise division by another Series."
  },
  {
    "name": "floorDivide",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.floorDivide(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise floor division."
  },
  {
    "name": "mod",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.mod(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise remainder."
  },
  {
    "name": "pow",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.pow(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise power."
  },
  {
    "name": "rightAdd",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.rightAdd(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "A scalar added from the left, as in 2.0 + series."
  },
  {
    "name": "rightSubtract",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.rightSubtract(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "A scalar subtracted from the left, as in 2.0 - series."
  },
  {
    "name": "rightMultiply",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.rightMultiply(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "A scalar multiplied from the left."
  },
  {
    "name": "rightDivide",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.rightDivide(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "A scalar divided from the left, as in 2.0 / series."
  },
  {
    "name": "negate",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.negate() : Series",
    "parameters": [],
    "documentation": "Negates every value."
  },
  {
    "name": "abs",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.abs() : Series",
    "parameters": [],
    "documentation": "The absolute value of every number."
  },
  {
    "name": "round",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.round(decimals : Int = 0) : Series",
    "parameters": [
      "decimals : Int = 0"
    ],
    "documentation": "Rounds every number to the given number of decimals."
  },
  {
    "name": "sqrt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.sqrt() : Series",
    "parameters": [],
    "documentation": "The square root of every number."
  },
  {
    "name": "clip",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.clip(low : double, high : double) : Series",
    "parameters": [
      "low : double",
      "high : double"
    ],
    "documentation": "Limits every number to the low and high bounds."
  },
  {
    "name": "eq",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.eq(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise equality; the result is a Bool Series."
  },
  {
    "name": "ne",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.ne(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise inequality."
  },
  {
    "name": "lt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.lt(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise less-than."
  },
  {
    "name": "le",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.le(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise less-than-or-equal."
  },
  {
    "name": "gt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.gt(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise greater-than."
  },
  {
    "name": "ge",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.ge(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise greater-than-or-equal."
  },
  {
    "name": "both",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.both(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise logical and of two Bool Series."
  },
  {
    "name": "either",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.either(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Elementwise logical or of two Bool Series."
  },
  {
    "name": "invert",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.invert() : Series",
    "parameters": [],
    "documentation": "Elementwise logical not."
  },
  {
    "name": "between",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.between(low : double, high : double) : Series",
    "parameters": [
      "low : double",
      "high : double"
    ],
    "documentation": "A Bool Series that is true inside the inclusive bounds."
  },
  {
    "name": "between",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.between(low : Int, high : Int) : Series",
    "parameters": [
      "low : Int",
      "high : Int"
    ],
    "documentation": "A Bool Series that is true inside the inclusive Int bounds."
  },
  {
    "name": "between",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.between(low : String, high : String) : Series",
    "parameters": [
      "low : String",
      "high : String"
    ],
    "documentation": "A Bool Series that is true inside the inclusive String bounds."
  },
  {
    "name": "isIn",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.isIn(values : List[Int]) : Series",
    "parameters": [
      "values : List[Int]"
    ],
    "documentation": "A Bool Series that is true where the value is in the list."
  },
  {
    "name": "isIn",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.isIn(values : List[double]) : Series",
    "parameters": [
      "values : List[double]"
    ],
    "documentation": "A Bool Series that is true where the value is in the list."
  },
  {
    "name": "isIn",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.isIn(values : List[String]) : Series",
    "parameters": [
      "values : List[String]"
    ],
    "documentation": "A Bool Series that is true where the value is in the list."
  },
  {
    "name": "isIn",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.isIn(values : List[Bool]) : Series",
    "parameters": [
      "values : List[Bool]"
    ],
    "documentation": "A Bool Series that is true where the value is in the list."
  },
  {
    "name": "isNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.isNa() : Series",
    "parameters": [],
    "documentation": "A Bool Series that is true for missing entries."
  },
  {
    "name": "notNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.notNa() : Series",
    "parameters": [],
    "documentation": "A Bool Series that is true for present entries."
  },
  {
    "name": "fillNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.fillNa(value : double) : Series",
    "parameters": [
      "value : double"
    ],
    "documentation": "Replaces missing entries with a double."
  },
  {
    "name": "fillNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.fillNa(value : Int) : Series",
    "parameters": [
      "value : Int"
    ],
    "documentation": "Replaces missing entries with an Int."
  },
  {
    "name": "fillNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.fillNa(value : String) : Series",
    "parameters": [
      "value : String"
    ],
    "documentation": "Replaces missing entries with a String."
  },
  {
    "name": "fillNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.fillNa(value : Bool) : Series",
    "parameters": [
      "value : Bool"
    ],
    "documentation": "Replaces missing entries with a Bool."
  },
  {
    "name": "fillNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.fillNa(value : Scalar) : Series",
    "parameters": [
      "value : Scalar"
    ],
    "documentation": "Replaces missing entries with a Scalar."
  },
  {
    "name": "dropNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.dropNa() : Series",
    "parameters": [],
    "documentation": "Removes rows with a missing entry."
  },
  {
    "name": "ffill",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.ffill() : Series",
    "parameters": [],
    "documentation": "Carries the last present value forward."
  },
  {
    "name": "bfill",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.bfill() : Series",
    "parameters": [],
    "documentation": "Carries the next present value back."
  },
  {
    "name": "count",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.count() : Int",
    "parameters": [],
    "documentation": "The number of present entries."
  },
  {
    "name": "sum",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.sum() : Scalar",
    "parameters": [],
    "documentation": "The sum of the present numbers."
  },
  {
    "name": "min",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.min() : Scalar",
    "parameters": [],
    "documentation": "The smallest present value."
  },
  {
    "name": "max",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.max() : Scalar",
    "parameters": [],
    "documentation": "The largest present value."
  },
  {
    "name": "mean",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.mean() : Scalar",
    "parameters": [],
    "documentation": "The mean of the present numbers."
  },
  {
    "name": "median",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.median() : Scalar",
    "parameters": [],
    "documentation": "The median of the present numbers."
  },
  {
    "name": "quantile",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.quantile(q : double) : Scalar",
    "parameters": [
      "q : double"
    ],
    "documentation": "The q quantile of the present numbers."
  },
  {
    "name": "std",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.std(ddof : Int = 1) : Scalar",
    "parameters": [
      "ddof : Int = 1"
    ],
    "documentation": "The sample standard deviation."
  },
  {
    "name": "variance",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.variance(ddof : Int = 1) : Scalar",
    "parameters": [
      "ddof : Int = 1"
    ],
    "documentation": "The sample variance."
  },
  {
    "name": "prod",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.prod() : Scalar",
    "parameters": [],
    "documentation": "The product of the present numbers."
  },
  {
    "name": "any",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.any() : Bool",
    "parameters": [],
    "documentation": "Whether any present Bool entry is true."
  },
  {
    "name": "all",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.all() : Bool",
    "parameters": [],
    "documentation": "Whether every present Bool entry is true."
  },
  {
    "name": "nunique",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.nunique() : Int",
    "parameters": [],
    "documentation": "The number of distinct present values."
  },
  {
    "name": "idxMax",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.idxMax() : Scalar",
    "parameters": [],
    "documentation": "The index label of the largest value."
  },
  {
    "name": "idxMin",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.idxMin() : Scalar",
    "parameters": [],
    "documentation": "The index label of the smallest value."
  },
  {
    "name": "unique",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.unique() : Series",
    "parameters": [],
    "documentation": "The distinct values, in first-seen order."
  },
  {
    "name": "duplicated",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.duplicated(keep : String = \"first\") : Series",
    "parameters": [
      "keep : String = \"first\""
    ],
    "documentation": "A Bool Series that is true for repeated values."
  },
  {
    "name": "dropDuplicates",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.dropDuplicates(keep : String = \"first\") : Series",
    "parameters": [
      "keep : String = \"first\""
    ],
    "documentation": "Keeps the first occurrence of every value."
  },
  {
    "name": "valueCounts",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.valueCounts(ascending : Bool = false, normalize : Bool = false, sort : Bool = true, dropNa : Bool = true) : Series",
    "parameters": [
      "ascending : Bool = false",
      "normalize : Bool = false",
      "sort : Bool = true",
      "dropNa : Bool = true"
    ],
    "documentation": "Counts of each distinct value; absent entries are dropped."
  },
  {
    "name": "mode",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.mode() : Series",
    "parameters": [],
    "documentation": "The most frequent value or values."
  },
  {
    "name": "cumSum",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.cumSum() : Series",
    "parameters": [],
    "documentation": "A running sum."
  },
  {
    "name": "cumProd",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.cumProd() : Series",
    "parameters": [],
    "documentation": "A running product."
  },
  {
    "name": "cumMax",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.cumMax() : Series",
    "parameters": [],
    "documentation": "A running maximum."
  },
  {
    "name": "cumMin",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.cumMin() : Series",
    "parameters": [],
    "documentation": "A running minimum."
  },
  {
    "name": "shift",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.shift(periods : Int = 1) : Series",
    "parameters": [
      "periods : Int = 1"
    ],
    "documentation": "Moves values down by the given number of rows, leaving missing entries behind."
  },
  {
    "name": "diff",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.diff(periods : Int = 1) : Series",
    "parameters": [
      "periods : Int = 1"
    ],
    "documentation": "The difference between each value and the value that many rows earlier."
  },
  {
    "name": "pctChange",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.pctChange(periods : Int = 1) : Series",
    "parameters": [
      "periods : Int = 1"
    ],
    "documentation": "The fractional change from the value that many rows earlier."
  },
  {
    "name": "rolling",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.rolling(window : Int, minPeriods : Int = -1) : Rolling",
    "parameters": [
      "window : Int",
      "minPeriods : Int = -1"
    ],
    "documentation": "A rolling-window view; call mean, sum, min, max, std or count on it."
  },
  {
    "name": "rank",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.rank(ascending : Bool = true, method : String = \"average\", pct : Bool = false) : Series",
    "parameters": [
      "ascending : Bool = true",
      "method : String = \"average\"",
      "pct : Bool = false"
    ],
    "documentation": "Each value place in sorted order; tied values share the average place."
  },
  {
    "name": "apply",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.apply(f : double -> double) : Series",
    "parameters": [
      "f : double -> double"
    ],
    "documentation": "Applies a function to every number."
  },
  {
    "name": "applyInt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.applyInt(f : Int -> Int) : Series",
    "parameters": [
      "f : Int -> Int"
    ],
    "documentation": "Applies a function to every Int."
  },
  {
    "name": "applyString",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.applyString(f : String -> String) : Series",
    "parameters": [
      "f : String -> String"
    ],
    "documentation": "Applies a function to the display text of every value."
  },
  {
    "name": "applyBool",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.applyBool(f : double -> Bool) : Series",
    "parameters": [
      "f : double -> Bool"
    ],
    "documentation": "Applies a predicate to every number and gives a Bool Series."
  },
  {
    "name": "replace",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.replace(old : String, replacement : String) : Series",
    "parameters": [
      "old : String",
      "replacement : String"
    ],
    "documentation": "Replaces matching String values."
  },
  {
    "name": "replace",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.replace(old : Int, replacement : Int) : Series",
    "parameters": [
      "old : Int",
      "replacement : Int"
    ],
    "documentation": "Replaces matching Int values."
  },
  {
    "name": "replace",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.replace(old : double, replacement : double) : Series",
    "parameters": [
      "old : double",
      "replacement : double"
    ],
    "documentation": "Replaces matching double values."
  },
  {
    "name": "str",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.str() : StringMethods",
    "parameters": [],
    "documentation": "String helpers such as upper, contains and splitPart."
  },
  {
    "name": "describe",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.describe() : Series",
    "parameters": [],
    "documentation": "Count, mean, standard deviation, quartiles and extremes for numbers, or count, unique, top and freq for other values."
  },
  {
    "name": "toFrame",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.toFrame() : DataFrame",
    "parameters": [],
    "documentation": "A one-column DataFrame holding this Series."
  },
  {
    "name": "text",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.text() : String",
    "parameters": [],
    "documentation": "The plain-text table shown in a notebook."
  },
  {
    "name": "html",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.html() : String",
    "parameters": [],
    "documentation": "The HTML table shown in a notebook."
  },
  {
    "name": "Rolling",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "Rolling(move series : Series, window : Int) : Rolling",
    "parameters": [
      "move series : Series",
      "window : Int"
    ],
    "documentation": "A rolling-window view over a Series. Series.rolling builds one."
  },
  {
    "name": "series",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "field": true,
    "signature": "Rolling.series : Series",
    "parameters": [],
    "documentation": "The underlying Series."
  },
  {
    "name": "window",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "field": true,
    "signature": "Rolling.window : Int",
    "parameters": [],
    "documentation": "The window width."
  },
  {
    "name": "mean",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.mean() : Series",
    "parameters": [],
    "documentation": "The mean of each window."
  },
  {
    "name": "sum",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.sum() : Series",
    "parameters": [],
    "documentation": "The sum of each window."
  },
  {
    "name": "min",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.min() : Series",
    "parameters": [],
    "documentation": "The smallest value in each window."
  },
  {
    "name": "max",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.max() : Series",
    "parameters": [],
    "documentation": "The largest value in each window."
  },
  {
    "name": "std",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.std() : Series",
    "parameters": [],
    "documentation": "The sample standard deviation of each window."
  },
  {
    "name": "count",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.count() : Series",
    "parameters": [],
    "documentation": "The number of present entries in each window."
  },
  {
    "name": "StringMethods",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "StringMethods(move series : Series) : StringMethods",
    "parameters": [
      "move series : Series"
    ],
    "documentation": "String helpers over a Series. Series.str builds one."
  },
  {
    "name": "series",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "field": true,
    "signature": "StringMethods.series : Series",
    "parameters": [],
    "documentation": "The underlying Series."
  },
  {
    "name": "upper",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.upper() : Series",
    "parameters": [],
    "documentation": "Uppercases every value."
  },
  {
    "name": "lower",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.lower() : Series",
    "parameters": [],
    "documentation": "Lowercases every value."
  },
  {
    "name": "strip",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.strip() : Series",
    "parameters": [],
    "documentation": "Trims whitespace from every value."
  },
  {
    "name": "replace",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.replace(old : String, replacement : String) : Series",
    "parameters": [
      "old : String",
      "replacement : String"
    ],
    "documentation": "Replaces every occurrence of a substring."
  },
  {
    "name": "capitalize",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.capitalize() : Series",
    "parameters": [],
    "documentation": "Uppercases the first letter and lowercases the rest."
  },
  {
    "name": "title",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.title() : Series",
    "parameters": [],
    "documentation": "Uppercases the start of every word."
  },
  {
    "name": "slice",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.slice(start : Int, stop : Int) : Series",
    "parameters": [
      "start : Int",
      "stop : Int"
    ],
    "documentation": "Keeps the half-open character range of every value."
  },
  {
    "name": "cat",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.cat(other : String) : Series",
    "parameters": [
      "other : String"
    ],
    "documentation": "Appends a suffix to every value."
  },
  {
    "name": "pad",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.pad(width : Int) : Series",
    "parameters": [
      "width : Int"
    ],
    "documentation": "Pads every value on the left to a width."
  },
  {
    "name": "contains",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.contains(part : String) : Series",
    "parameters": [
      "part : String"
    ],
    "documentation": "A Bool Series that is true where the value contains the text."
  },
  {
    "name": "startsWith",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.startsWith(part : String) : Series",
    "parameters": [
      "part : String"
    ],
    "documentation": "A Bool Series that is true where the value starts with the text."
  },
  {
    "name": "endsWith",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.endsWith(part : String) : Series",
    "parameters": [
      "part : String"
    ],
    "documentation": "A Bool Series that is true where the value ends with the text."
  },
  {
    "name": "isEmpty",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.isEmpty() : Series",
    "parameters": [],
    "documentation": "A Bool Series that is true for empty values."
  },
  {
    "name": "len",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.len() : Series",
    "parameters": [],
    "documentation": "The length of every value."
  },
  {
    "name": "splitPart",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.splitPart(separator : String, position : Int) : Series",
    "parameters": [
      "separator : String",
      "position : Int"
    ],
    "documentation": "Splits each value and keeps one part."
  },
  {
    "name": "DataFrame",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "DataFrame(names : List[String], move cols : Array[Column], move index : Column, indexName : String) : DataFrame",
    "parameters": [
      "names : List[String]",
      "move cols : Array[Column]",
      "move index : Column",
      "indexName : String"
    ],
    "documentation": "A frame from named Series that share a length."
  },
  {
    "name": "DataFrame",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "DataFrame(series : List[Series]) : DataFrame",
    "parameters": [
      "series : List[Series]"
    ],
    "documentation": "A frame from name/value pairs of Int columns."
  },
  {
    "name": "DataFrame",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "DataFrame(columns : List[(String, List[Int])]) : DataFrame",
    "parameters": [
      "columns : List[(String, List[Int])]"
    ],
    "documentation": "A frame from name/value pairs of double columns."
  },
  {
    "name": "DataFrame",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "DataFrame(columns : List[(String, List[double])]) : DataFrame",
    "parameters": [
      "columns : List[(String, List[double])]"
    ],
    "documentation": "A frame from name/value pairs of String columns."
  },
  {
    "name": "DataFrame",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "DataFrame(columns : List[(String, List[String])]) : DataFrame",
    "parameters": [
      "columns : List[(String, List[String])]"
    ],
    "documentation": "A frame from name/value pairs of Bool columns."
  },
  {
    "name": "DataFrame",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "DataFrame(columns : List[(String, List[Bool])]) : DataFrame",
    "parameters": [
      "columns : List[(String, List[Bool])]"
    ],
    "documentation": "A frame from explicit column storage. The storage and index move into the new frame."
  },
  {
    "name": "names",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "field": true,
    "signature": "DataFrame.names : List[String]",
    "parameters": [],
    "documentation": "The column names, in order."
  },
  {
    "name": "index",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "field": true,
    "signature": "DataFrame.index : Column",
    "parameters": [],
    "documentation": "The row labels."
  },
  {
    "name": "indexName",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "field": true,
    "signature": "DataFrame.indexName : String",
    "parameters": [],
    "documentation": "The name printed over the index."
  },
  {
    "name": "columnsLabel",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "field": true,
    "signature": "DataFrame.columnsLabel : String",
    "parameters": [],
    "documentation": "The name printed over the columns, such as the column pivotTable grouped by."
  },
  {
    "name": "empty",
    "module": "std.kesdas",
    "gated": true,
    "static": "DataFrame",
    "signature": "DataFrame.empty() : DataFrame",
    "parameters": [],
    "documentation": "A frame with no rows and no columns."
  },
  {
    "name": "concat",
    "module": "std.kesdas",
    "gated": true,
    "static": "DataFrame",
    "signature": "DataFrame.concat(frames : List[DataFrame]) : DataFrame",
    "parameters": [
      "frames : List[DataFrame]"
    ],
    "documentation": "Stacks frames one below the other, matching columns by name."
  },
  {
    "name": "parseCsv",
    "module": "std.kesdas",
    "gated": true,
    "static": "DataFrame",
    "signature": "DataFrame.parseCsv(text : String, separator : String = \",\", header : Bool = true, skipRows : Int = 0, nRows : Int = -1) : DataFrame",
    "parameters": [
      "text : String",
      "separator : String = \"",
      "\"",
      "header : Bool = true",
      "skipRows : Int = 0",
      "nRows : Int = -1"
    ],
    "documentation": "Reads CSV text: the first line names the columns, and each column takes the narrowest kind that fits."
  },
  {
    "name": "parseCsvLines",
    "module": "std.kesdas",
    "gated": true,
    "static": "DataFrame",
    "signature": "DataFrame.parseCsvLines(lines : List[String], separator : String = \",\", header : Bool = true, skipRows : Int = 0, nRows : Int = -1) : DataFrame",
    "parameters": [
      "lines : List[String]",
      "separator : String = \",\"",
      "header : Bool = true",
      "skipRows : Int = 0",
      "nRows : Int = -1"
    ],
    "documentation": "A frame from CSV lines already read from a file or notebook."
  },
  {
    "name": "length",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.length() : Int",
    "parameters": [],
    "documentation": "The number of rows."
  },
  {
    "name": "width",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.width() : Int",
    "parameters": [],
    "documentation": "The number of columns."
  },
  {
    "name": "shape",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.shape() : (Int, Int)",
    "parameters": [],
    "documentation": "The row and column counts."
  },
  {
    "name": "isEmpty",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.isEmpty() : Bool",
    "parameters": [],
    "documentation": "Whether the frame has no rows or no columns."
  },
  {
    "name": "columns",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.columns() : List[String]",
    "parameters": [],
    "documentation": "The column names, in order."
  },
  {
    "name": "hasColumn",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.hasColumn(name : String) : Bool",
    "parameters": [
      "name : String"
    ],
    "documentation": "Whether a column with this name exists."
  },
  {
    "name": "copy",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.copy() : DataFrame",
    "parameters": [],
    "documentation": "An independent copy of every column and the index."
  },
  {
    "name": "dtypes",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.dtypes() : Series",
    "parameters": [],
    "documentation": "One dtype string per column, indexed by column name."
  },
  {
    "name": "keyColumn",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.keyColumn(name : String) : Column",
    "parameters": [
      "name : String"
    ],
    "documentation": "A copy of a column by name."
  },
  {
    "module": "std.kesdas",
    "gated": true,
    "name": "keyCodes",
    "signature": "DataFrame.keyCodes(name : String) : (List[Int], Int)",
    "parameters": [
      "name : String"
    ],
    "documentation": "Factorizes one key column: integer codes and the number of distinct codes.",
    "receiver": "DataFrame"
  },
  {
    "name": "col",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.col(name : String) : Series or Error",
    "parameters": [
      "name : String"
    ],
    "documentation": "One column as a Series. A missing name is a DataError.MissingColumn."
  },
  {
    "name": "get",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.get(name : String) : Option[Series]",
    "parameters": [
      "name : String"
    ],
    "documentation": "One column as a Series, or None when the name is absent."
  },
  {
    "name": "select",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.select(selected : List[String]) : DataFrame or Error",
    "parameters": [
      "selected : List[String]"
    ],
    "documentation": "A frame with the named columns, in that order."
  },
  {
    "name": "take",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.take(positions : List[Int]) : DataFrame",
    "parameters": [
      "positions : List[Int]"
    ],
    "documentation": "Rows at the given positions, in that order."
  },
  {
    "name": "head",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.head(n : Int = 5) : DataFrame",
    "parameters": [
      "n : Int = 5"
    ],
    "documentation": "The first n rows."
  },
  {
    "name": "tail",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.tail(n : Int = 5) : DataFrame",
    "parameters": [
      "n : Int = 5"
    ],
    "documentation": "The last n rows."
  },
  {
    "name": "slice",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.slice(start : Int, stop : Int) : DataFrame",
    "parameters": [
      "start : Int",
      "stop : Int"
    ],
    "documentation": "Rows in the half-open range start to stop."
  },
  {
    "name": "iloc",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.iloc(positions : List[Int]) : DataFrame",
    "parameters": [
      "positions : List[Int]"
    ],
    "documentation": "Rows at the given positions."
  },
  {
    "name": "row",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.row(position : Int) : Series",
    "parameters": [
      "position : Int"
    ],
    "documentation": "One row as a Series indexed by column names."
  },
  {
    "name": "filter",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.filter(mask : Series) : DataFrame",
    "parameters": [
      "mask : Series"
    ],
    "documentation": "Keeps the rows where the Bool mask is true."
  },
  {
    "name": "loc",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.loc(label : Int) : DataFrame",
    "parameters": [
      "label : Int"
    ],
    "documentation": "Rows whose Int index label matches."
  },
  {
    "name": "loc",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.loc(label : String) : DataFrame",
    "parameters": [
      "label : String"
    ],
    "documentation": "Rows whose String index label matches."
  },
  {
    "name": "sortValues",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.sortValues(by : String, ascending : Bool = true, naPosition : String = \"last\") : DataFrame or Error",
    "parameters": [
      "by : String",
      "ascending : Bool = true",
      "naPosition : String = \"last\""
    ],
    "documentation": "Sorts rows by one column."
  },
  {
    "name": "sortValues",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.sortValues(by : List[String], ascending : Bool = true, naPosition : String = \"last\") : DataFrame or Error",
    "parameters": [
      "by : List[String]",
      "ascending : Bool = true",
      "naPosition : String = \"last\""
    ],
    "documentation": "Sorts rows by several columns, the first key most significant."
  },
  {
    "name": "sortIndex",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.sortIndex(ascending : Bool = true, naPosition : String = \"last\") : DataFrame",
    "parameters": [
      "ascending : Bool = true",
      "naPosition : String = \"last\""
    ],
    "documentation": "Sorts rows by index label."
  },
  {
    "name": "nlargest",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.nlargest(n : Int, by : String) : DataFrame or Error",
    "parameters": [
      "n : Int",
      "by : String"
    ],
    "documentation": "The n rows with the largest value in a column."
  },
  {
    "name": "nsmallest",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.nsmallest(n : Int, by : String) : DataFrame or Error",
    "parameters": [
      "n : Int",
      "by : String"
    ],
    "documentation": "The n rows with the smallest value in a column."
  },
  {
    "name": "drop",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.drop(column : String) : DataFrame",
    "parameters": [
      "column : String"
    ],
    "documentation": "A frame without one column."
  },
  {
    "name": "drop",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.drop(removed : List[String]) : DataFrame",
    "parameters": [
      "removed : List[String]"
    ],
    "documentation": "A frame without the named columns."
  },
  {
    "name": "dropRows",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.dropRows(positions : List[Int]) : DataFrame",
    "parameters": [
      "positions : List[Int]"
    ],
    "documentation": "A frame without the rows at the given positions."
  },
  {
    "name": "rename",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.rename(old : String, replacement : String) : DataFrame",
    "parameters": [
      "old : String",
      "replacement : String"
    ],
    "documentation": "A frame with one column renamed."
  },
  {
    "name": "renameColumns",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.renameColumns(mapping : Map[String, String]) : DataFrame",
    "parameters": [
      "mapping : Map[String, String]"
    ],
    "documentation": "A frame whose columns are renamed through a map; unlisted names stay."
  },
  {
    "name": "assign",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.assign(name : String, series : Series) : DataFrame",
    "parameters": [
      "name : String",
      "series : Series"
    ],
    "documentation": "A frame with a column added or replaced."
  },
  {
    "name": "assign",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.assign(name : String, values : List[Int]) : DataFrame",
    "parameters": [
      "name : String",
      "values : List[Int]"
    ],
    "documentation": "A frame with an Int column added or replaced."
  },
  {
    "name": "assign",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.assign(name : String, values : List[double]) : DataFrame",
    "parameters": [
      "name : String",
      "values : List[double]"
    ],
    "documentation": "A frame with a double column added or replaced."
  },
  {
    "name": "assign",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.assign(name : String, values : List[String]) : DataFrame",
    "parameters": [
      "name : String",
      "values : List[String]"
    ],
    "documentation": "A frame with a String column added or replaced."
  },
  {
    "name": "assign",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.assign(name : String, values : List[Bool]) : DataFrame",
    "parameters": [
      "name : String",
      "values : List[Bool]"
    ],
    "documentation": "A frame with a Bool column added or replaced."
  },
  {
    "name": "setIndex",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.setIndex(name : String) : DataFrame or Error",
    "parameters": [
      "name : String"
    ],
    "documentation": "Uses one column as the index and removes it from the columns."
  },
  {
    "name": "resetIndex",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.resetIndex(drop : Bool = false) : DataFrame",
    "parameters": [
      "drop : Bool = false"
    ],
    "documentation": "Replaces the index with 0..n-1; by default the old index becomes a column."
  },
  {
    "name": "astype",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.astype(dtype : String) : DataFrame",
    "parameters": [
      "dtype : String"
    ],
    "documentation": "Converts every column to int64, float64, bool or str."
  },
  {
    "name": "isNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.isNa() : DataFrame",
    "parameters": [],
    "documentation": "A Bool frame that is true for missing entries."
  },
  {
    "name": "notNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.notNa() : DataFrame",
    "parameters": [],
    "documentation": "A Bool frame that is true for present entries."
  },
  {
    "name": "fillNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.fillNa(value : double) : DataFrame",
    "parameters": [
      "value : double"
    ],
    "documentation": "Replaces missing entries with a double where the column kind fits."
  },
  {
    "name": "fillNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.fillNa(value : Int) : DataFrame",
    "parameters": [
      "value : Int"
    ],
    "documentation": "Replaces missing entries with an Int where the column kind fits."
  },
  {
    "name": "fillNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.fillNa(value : String) : DataFrame",
    "parameters": [
      "value : String"
    ],
    "documentation": "Replaces missing entries with a String where the column kind fits."
  },
  {
    "name": "fillNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.fillNa(value : Bool) : DataFrame",
    "parameters": [
      "value : Bool"
    ],
    "documentation": "Replaces missing entries with a Bool where the column kind fits."
  },
  {
    "name": "dropNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.dropNa() : DataFrame",
    "parameters": [],
    "documentation": "Selects rows by the named columns."
  },
  {
    "name": "dropNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.dropNa(subset : List[String]) : DataFrame or Error",
    "parameters": [
      "subset : List[String]"
    ],
    "documentation": "Selects rows by the named columns."
  },
  {
    "name": "dropNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.dropNa(how : String = \"any\", thresh : Int = 0) : DataFrame",
    "parameters": [
      "how : String = \"any\"",
      "thresh : Int = 0"
    ],
    "documentation": "Removes rows with a missing entry."
  },
  {
    "name": "dropNa",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.dropNa(subset : List[String], how : String = \"any\", thresh : Int = 0) : DataFrame or Error",
    "parameters": [
      "subset : List[String]",
      "how : String = \"any\"",
      "thresh : Int = 0"
    ],
    "documentation": "Selects rows by the named columns."
  },
  {
    "name": "duplicated",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.duplicated(keep : String = \"first\") : Series",
    "parameters": [
      "keep : String = \"first\""
    ],
    "documentation": "A Bool Series that is true for repeated rows."
  },
  {
    "name": "dropDuplicates",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.dropDuplicates() : DataFrame",
    "parameters": [],
    "documentation": "Selects rows by the named columns."
  },
  {
    "name": "dropDuplicates",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.dropDuplicates(subset : List[String]) : DataFrame or Error",
    "parameters": [
      "subset : List[String]"
    ],
    "documentation": "Selects rows by the named columns."
  },
  {
    "name": "dropDuplicates",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.dropDuplicates(keep : String = \"first\") : DataFrame",
    "parameters": [
      "keep : String = \"first\""
    ],
    "documentation": "Keeps the first occurrence of every row."
  },
  {
    "name": "dropDuplicates",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.dropDuplicates(subset : List[String], keep : String = \"first\") : DataFrame or Error",
    "parameters": [
      "subset : List[String]",
      "keep : String = \"first\""
    ],
    "documentation": "Selects rows by the named columns."
  },
  {
    "name": "agg",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.agg(statistic : String) : Series",
    "parameters": [
      "statistic : String"
    ],
    "documentation": "Reduces every column with a named statistic."
  },
  {
    "name": "sum",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.sum() : Series",
    "parameters": [],
    "documentation": "The sum of every column."
  },
  {
    "name": "mean",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.mean() : Series",
    "parameters": [],
    "documentation": "The mean of every numeric column."
  },
  {
    "name": "median",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.median() : Series",
    "parameters": [],
    "documentation": "The median of every numeric column."
  },
  {
    "name": "min",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.min() : Series",
    "parameters": [],
    "documentation": "The smallest value in every column."
  },
  {
    "name": "max",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.max() : Series",
    "parameters": [],
    "documentation": "The largest value in every column."
  },
  {
    "name": "count",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.count() : Series",
    "parameters": [],
    "documentation": "The number of present entries in every column."
  },
  {
    "name": "std",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.std() : Series",
    "parameters": [],
    "documentation": "The sample standard deviation of every numeric column."
  },
  {
    "name": "variance",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.variance() : Series",
    "parameters": [],
    "documentation": "The sample variance of every numeric column."
  },
  {
    "name": "nunique",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.nunique() : Series",
    "parameters": [],
    "documentation": "The number of distinct values in every column."
  },
  {
    "name": "describe",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.describe() : DataFrame",
    "parameters": [],
    "documentation": "Summary statistics: one column per numeric column, or counts for a frame without numbers."
  },
  {
    "name": "corr",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.corr() : DataFrame",
    "parameters": [],
    "documentation": "The Pearson correlation of every pair of numeric columns."
  },
  {
    "name": "groupBy",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.groupBy(key : String) : GroupBy or Error",
    "parameters": [
      "key : String"
    ],
    "documentation": "Groups rows by one column."
  },
  {
    "name": "groupBy",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.groupBy(keys : List[String]) : GroupBy or Error",
    "parameters": [
      "keys : List[String]"
    ],
    "documentation": "Groups rows by several columns."
  },
  {
    "name": "merge",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.merge(other : DataFrame, on : String, how : String = \"inner\", leftSuffix : String = \"_x\", rightSuffix : String = \"_y\") : DataFrame or Error",
    "parameters": [
      "other : DataFrame",
      "on : String",
      "how : String = \"inner\"",
      "leftSuffix : String = \"_x\"",
      "rightSuffix : String = \"_y\""
    ],
    "documentation": "Joins another frame on a column with the same name."
  },
  {
    "name": "merge",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.merge(other : DataFrame, on : List[String], how : String = \"inner\", leftSuffix : String = \"_x\", rightSuffix : String = \"_y\") : DataFrame or Error",
    "parameters": [
      "other : DataFrame",
      "on : List[String]",
      "how : String = \"inner\"",
      "leftSuffix : String = \"_x\"",
      "rightSuffix : String = \"_y\""
    ],
    "documentation": "Joins another frame on several columns with the same names."
  },
  {
    "name": "merge",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.merge(other : DataFrame, leftOn : String, rightOn : String, how : String = \"inner\", leftSuffix : String = \"_x\", rightSuffix : String = \"_y\") : DataFrame or Error",
    "parameters": [
      "other : DataFrame",
      "leftOn : String",
      "rightOn : String",
      "how : String = \"inner\"",
      "leftSuffix : String = \"_x\"",
      "rightSuffix : String = \"_y\""
    ],
    "documentation": "Joins another frame on different column names."
  },
  {
    "name": "append",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.append(other : DataFrame) : DataFrame",
    "parameters": [
      "other : DataFrame"
    ],
    "documentation": "Stacks another frame below this one, matching columns by name."
  },
  {
    "name": "pivotTable",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.pivotTable(index : String, columns : String, values : String, aggfunc : String = \"mean\") : DataFrame or Error",
    "parameters": [
      "index : String",
      "columns : String",
      "values : String",
      "aggfunc : String = \"mean\""
    ],
    "documentation": "Lays one column values across the top, aggregating a third column."
  },
  {
    "name": "toTensor",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.toTensor() : DynTensor[double]",
    "parameters": [],
    "documentation": "Copies the numeric columns into a rows-by-columns tensor."
  },
  {
    "name": "toCsv",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.toCsv(separator : String = \",\", header : Bool = true, includeIndex : Bool = false) : String",
    "parameters": [
      "separator : String = \"",
      "\"",
      "header : Bool = true",
      "includeIndex : Bool = false"
    ],
    "documentation": "Writes the frame as CSV text."
  },
  {
    "name": "text",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.text() : String",
    "parameters": [],
    "documentation": "The plain-text table shown in a notebook."
  },
  {
    "name": "html",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.html() : String",
    "parameters": [],
    "documentation": "The HTML table shown in a notebook."
  },
  {
    "name": "cell",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.cell(row : Int, column : String) : Scalar or Error",
    "parameters": [
      "row : Int",
      "column : String"
    ],
    "documentation": "One value by row position and column name."
  },
  {
    "name": "mapNumeric",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.mapNumeric(f : double -> double) : DataFrame",
    "parameters": [
      "f : double -> double"
    ],
    "documentation": "Applies a function to every number column and keeps the others."
  },
  {
    "name": "saveCsv",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.saveCsv(path : String, separator : String = \",\", includeIndex : Bool = false) : void or Error with Effects",
    "parameters": [
      "path : String",
      "separator : String = \"",
      "\"",
      "includeIndex : Bool = false"
    ],
    "documentation": "Writes the frame to a file."
  },
  {
    "name": "info",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.info() : void with Effects",
    "parameters": [],
    "documentation": "Prints the column names, non-null counts and dtypes."
  },
  {
    "name": "GroupBy",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "GroupBy(move frame : DataFrame, keys : List[String]) : GroupBy",
    "parameters": [
      "move frame : DataFrame",
      "keys : List[String]"
    ],
    "documentation": "A grouped frame. DataFrame.groupBy builds one."
  },
  {
    "name": "frame",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "field": true,
    "signature": "GroupBy.frame : DataFrame",
    "parameters": [],
    "documentation": "The frame whose rows are grouped."
  },
  {
    "name": "keys",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "field": true,
    "signature": "GroupBy.keys : List[String]",
    "parameters": [],
    "documentation": "The columns whose values define the groups."
  },
  {
    "name": "count",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.count() : Int",
    "parameters": [],
    "documentation": "The number of groups."
  },
  {
    "name": "aggregate",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.aggregate(statistic : String) : DataFrame",
    "parameters": [
      "statistic : String"
    ],
    "documentation": "Applies a named statistic to every value column."
  },
  {
    "name": "sum",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.sum() : DataFrame",
    "parameters": [],
    "documentation": "The sum of every value column in each group."
  },
  {
    "name": "mean",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.mean() : DataFrame",
    "parameters": [],
    "documentation": "The mean of every numeric value column in each group."
  },
  {
    "name": "median",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.median() : DataFrame",
    "parameters": [],
    "documentation": "The median of every numeric value column in each group."
  },
  {
    "name": "min",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.min() : DataFrame",
    "parameters": [],
    "documentation": "The smallest value in every value column in each group."
  },
  {
    "name": "max",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.max() : DataFrame",
    "parameters": [],
    "documentation": "The largest value in every value column in each group."
  },
  {
    "name": "std",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.std() : DataFrame",
    "parameters": [],
    "documentation": "The sample standard deviation of every numeric value column."
  },
  {
    "name": "variance",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.variance() : DataFrame",
    "parameters": [],
    "documentation": "The sample variance of every numeric value column."
  },
  {
    "name": "first",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.first() : DataFrame",
    "parameters": [],
    "documentation": "The first row of every value column in each group."
  },
  {
    "name": "last",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.last() : DataFrame",
    "parameters": [],
    "documentation": "The last row of every value column in each group."
  },
  {
    "name": "nunique",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.nunique() : DataFrame",
    "parameters": [],
    "documentation": "The number of distinct values in every value column."
  },
  {
    "name": "prod",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.prod() : DataFrame",
    "parameters": [],
    "documentation": "The product of every numeric value column in each group."
  },
  {
    "name": "counts",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.counts() : DataFrame",
    "parameters": [],
    "documentation": "The number of present entries in every value column."
  },
  {
    "name": "size",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.size() : Series",
    "parameters": [],
    "documentation": "The number of rows in each group."
  },
  {
    "name": "agg",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.agg(spec : List[(String, String)]) : DataFrame",
    "parameters": [
      "spec : List[(String, String)]"
    ],
    "documentation": "Applies a statistic to named columns: agg([(\"price\", \"mean\"), (\"qty\", \"sum\")])."
  },
  {
    "name": "filterGroups",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.filterGroups(keep : Int -> Bool) : DataFrame",
    "parameters": [
      "keep : Int -> Bool"
    ],
    "documentation": "Keeps the rows of every group whose size the test accepts."
  },
  {
    "name": "transform",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.transform(column : String, statistic : String) : Series",
    "parameters": [
      "column : String",
      "statistic : String"
    ],
    "documentation": "Gives every row the statistic of its group, in the original order."
  },
  {
    "name": "readCsv",
    "module": "std.kesdas",
    "gated": true,
    "signature": "readCsv(path : String, separator : String = \",\", header : Bool = true, skipRows : Int = 0, nRows : Int = -1) : DataFrame or Error with Effects",
    "parameters": [
      "path : String",
      "separator : String = \"",
      "\"",
      "header : Bool = true",
      "skipRows : Int = 0",
      "nRows : Int = -1"
    ],
    "documentation": "Reads a CSV file into a frame."
  },
  {
    "name": "EWM",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "EWM",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "Expanding",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "Expanding",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "FrameRolling",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "FrameRolling",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "size",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.size() : Int",
    "parameters": [],
    "documentation": "Member of Series."
  },
  {
    "name": "ndim",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.ndim() : Int",
    "parameters": [],
    "documentation": "Member of Series."
  },
  {
    "name": "toScalars",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.toScalars() : List[Scalar]",
    "parameters": [],
    "documentation": "Member of Series."
  },
  {
    "name": "items",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.items() : List[(Scalar, Scalar)]",
    "parameters": [],
    "documentation": "Member of Series."
  },
  {
    "name": "equals",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.equals(other : Series) : Bool",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "iat",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.iat(position : Int) : Scalar",
    "parameters": [
      "position : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "takeGaps",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.takeGaps(positions : List[Int]) : Series",
    "parameters": [
      "positions : List[Int]"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "positionOf",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.positionOf(label : double) : Option[Int]",
    "parameters": [
      "label : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "loc",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.loc(label : double) : Scalar",
    "parameters": [
      "label : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "at",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.at(label : Int) : Scalar",
    "parameters": [
      "label : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "at",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.at(label : String) : Scalar",
    "parameters": [
      "label : String"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "where",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.where(mask : Series, other : Series) : Series",
    "parameters": [
      "mask : Series",
      "other : Series"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "mask",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.mask(mask : Series, other : Scalar) : Series",
    "parameters": [
      "mask : Series",
      "other : Scalar"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "add",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.add(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "subtract",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.subtract(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "multiply",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.multiply(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "divide",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.divide(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "floorDivide",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.floorDivide(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "mod",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.mod(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "pow",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.pow(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "add",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.add(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "rightAdd",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.rightAdd(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "rightSubtract",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.rightSubtract(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "rightMultiply",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.rightMultiply(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "rightDivide",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.rightDivide(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "eq",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.eq(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "ne",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.ne(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "lt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.lt(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "le",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.le(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "gt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.gt(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "ge",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.ge(other : double) : Series",
    "parameters": [
      "other : double"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "eq",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.eq(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "ne",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.ne(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "lt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.lt(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "le",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.le(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "gt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.gt(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "ge",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.ge(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "interpolate",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.interpolate() : Series",
    "parameters": [],
    "documentation": "Member of Series."
  },
  {
    "name": "quantile",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.quantile(qs : List[double]) : Series",
    "parameters": [
      "qs : List[double]"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "sem",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.sem(ddof : Int = 1) : Scalar",
    "parameters": [
      "ddof : Int = 1"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "skew",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.skew() : Scalar",
    "parameters": [],
    "documentation": "Member of Series."
  },
  {
    "name": "kurt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.kurt() : Scalar",
    "parameters": [],
    "documentation": "Member of Series."
  },
  {
    "name": "argMax",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.argMax() : Int",
    "parameters": [],
    "documentation": "Member of Series."
  },
  {
    "name": "argMin",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.argMin() : Int",
    "parameters": [],
    "documentation": "Member of Series."
  },
  {
    "name": "expanding",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.expanding(minPeriods : Int = 1) : Expanding",
    "parameters": [
      "minPeriods : Int = 1"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "ewm",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.ewm(alpha : double, adjust : Bool = true, minPeriods : Int = 0) : EWM",
    "parameters": [
      "alpha : double",
      "adjust : Bool = true",
      "minPeriods : Int = 0"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "ewmSpan",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.ewmSpan(span : Int, adjust : Bool = true, minPeriods : Int = 0) : EWM",
    "parameters": [
      "span : Int",
      "adjust : Bool = true",
      "minPeriods : Int = 0"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "ewmHalfLife",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.ewmHalfLife(halflife : double, adjust : Bool = true, minPeriods : Int = 0) : EWM",
    "parameters": [
      "halflife : double",
      "adjust : Bool = true",
      "minPeriods : Int = 0"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "map",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.map(f : Scalar -> Scalar) : Series",
    "parameters": [
      "f : Scalar -> Scalar"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "replace",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.replace(old : Bool, replacement : Bool) : Series",
    "parameters": [
      "old : Bool",
      "replacement : Bool"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "replaceMany",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.replaceMany(pairs : List[(Scalar, Scalar)]) : Series",
    "parameters": [
      "pairs : List[(Scalar, Scalar)]"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "mapValues",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.mapValues(mapping : Map[String, String]) : Series",
    "parameters": [
      "mapping : Map[String, String]"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "splitToFrame",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.splitToFrame(separator : String) : DataFrame",
    "parameters": [
      "separator : String"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "cov",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.cov(other : Series, ddof : Int = 1) : Scalar",
    "parameters": [
      "other : Series",
      "ddof : Int = 1"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "corr",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.corr(other : Series) : Scalar",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "autocorr",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.autocorr(lag : Int = 1) : Scalar",
    "parameters": [
      "lag : Int = 1"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "factorize",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.factorize() : (Series, Series)",
    "parameters": [],
    "documentation": "Member of Series."
  },
  {
    "name": "toCsv",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.toCsv(separator : String = \",\", header : Bool = true, includeIndex : Bool = true) : String",
    "parameters": [
      "separator : String = \"",
      "\"",
      "header : Bool = true",
      "includeIndex : Bool = true"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "toTensor",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.toTensor() : DynTensor[double]",
    "parameters": [],
    "documentation": "Member of Series."
  },
  {
    "name": "concat",
    "module": "std.kesdas",
    "gated": true,
    "static": "Series",
    "signature": "Series.concat(parts : List[Series]) : Series",
    "parameters": [
      "parts : List[Series]"
    ],
    "documentation": "Static member of Series."
  },
  {
    "name": "append",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.append(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "combineFirst",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.combineFirst(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "update",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Series",
    "signature": "Series.update(other : Series) : Series",
    "parameters": [
      "other : Series"
    ],
    "documentation": "Member of Series."
  },
  {
    "name": "minPeriods",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "field": true,
    "signature": "Rolling.minPeriods : Int",
    "parameters": [],
    "documentation": "Field of Rolling."
  },
  {
    "name": "Rolling",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "Rolling(move series : Series, window : Int, minPeriods : Int) : Rolling",
    "parameters": [
      "move series : Series",
      "window : Int",
      "minPeriods : Int"
    ],
    "documentation": "Builds a Rolling."
  },
  {
    "name": "variance",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.variance() : Series",
    "parameters": [],
    "documentation": "Member of Rolling."
  },
  {
    "name": "median",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.median() : Series",
    "parameters": [],
    "documentation": "Member of Rolling."
  },
  {
    "name": "sem",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.sem() : Series",
    "parameters": [],
    "documentation": "Member of Rolling."
  },
  {
    "name": "skew",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.skew() : Series",
    "parameters": [],
    "documentation": "Member of Rolling."
  },
  {
    "name": "kurt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.kurt() : Series",
    "parameters": [],
    "documentation": "Member of Rolling."
  },
  {
    "name": "quantile",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.quantile(q : double) : Series",
    "parameters": [
      "q : double"
    ],
    "documentation": "Member of Rolling."
  },
  {
    "name": "apply",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Rolling",
    "signature": "Rolling.apply(f : List[double] -> double) : Series",
    "parameters": [
      "f : List[double] -> double"
    ],
    "documentation": "Member of Rolling."
  },
  {
    "name": "series",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "field": true,
    "signature": "Expanding.series : Series",
    "parameters": [],
    "documentation": "Field of Expanding."
  },
  {
    "name": "minPeriods",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "field": true,
    "signature": "Expanding.minPeriods : Int",
    "parameters": [],
    "documentation": "Field of Expanding."
  },
  {
    "name": "Expanding",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "Expanding(move series : Series, minPeriods : Int) : Expanding",
    "parameters": [
      "move series : Series",
      "minPeriods : Int"
    ],
    "documentation": "Builds a Expanding."
  },
  {
    "name": "mean",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "signature": "Expanding.mean() : Series",
    "parameters": [],
    "documentation": "Member of Expanding."
  },
  {
    "name": "sum",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "signature": "Expanding.sum() : Series",
    "parameters": [],
    "documentation": "Member of Expanding."
  },
  {
    "name": "min",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "signature": "Expanding.min() : Series",
    "parameters": [],
    "documentation": "Member of Expanding."
  },
  {
    "name": "max",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "signature": "Expanding.max() : Series",
    "parameters": [],
    "documentation": "Member of Expanding."
  },
  {
    "name": "std",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "signature": "Expanding.std() : Series",
    "parameters": [],
    "documentation": "Member of Expanding."
  },
  {
    "name": "variance",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "signature": "Expanding.variance() : Series",
    "parameters": [],
    "documentation": "Member of Expanding."
  },
  {
    "name": "median",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "signature": "Expanding.median() : Series",
    "parameters": [],
    "documentation": "Member of Expanding."
  },
  {
    "name": "count",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "signature": "Expanding.count() : Series",
    "parameters": [],
    "documentation": "Member of Expanding."
  },
  {
    "name": "sem",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "signature": "Expanding.sem() : Series",
    "parameters": [],
    "documentation": "Member of Expanding."
  },
  {
    "name": "quantile",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "signature": "Expanding.quantile(q : double) : Series",
    "parameters": [
      "q : double"
    ],
    "documentation": "Member of Expanding."
  },
  {
    "name": "apply",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "Expanding",
    "signature": "Expanding.apply(f : List[double] -> double) : Series",
    "parameters": [
      "f : List[double] -> double"
    ],
    "documentation": "Member of Expanding."
  },
  {
    "name": "series",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "EWM",
    "field": true,
    "signature": "EWM.series : Series",
    "parameters": [],
    "documentation": "Field of EWM."
  },
  {
    "name": "alpha",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "EWM",
    "field": true,
    "signature": "EWM.alpha : double",
    "parameters": [],
    "documentation": "Field of EWM."
  },
  {
    "name": "adjust",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "EWM",
    "field": true,
    "signature": "EWM.adjust : Bool",
    "parameters": [],
    "documentation": "Field of EWM."
  },
  {
    "name": "minPeriods",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "EWM",
    "field": true,
    "signature": "EWM.minPeriods : Int",
    "parameters": [],
    "documentation": "Field of EWM."
  },
  {
    "name": "EWM",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "EWM(move series : Series, alpha : double, adjust : Bool, minPeriods : Int) : EWM",
    "parameters": [
      "move series : Series",
      "alpha : double",
      "adjust : Bool",
      "minPeriods : Int"
    ],
    "documentation": "Builds a EWM."
  },
  {
    "name": "mean",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "EWM",
    "signature": "EWM.mean() : Series",
    "parameters": [],
    "documentation": "Member of EWM."
  },
  {
    "name": "sum",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "EWM",
    "signature": "EWM.sum() : Series",
    "parameters": [],
    "documentation": "Member of EWM."
  },
  {
    "name": "variance",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "EWM",
    "signature": "EWM.variance(bias : Bool = false) : Series",
    "parameters": [
      "bias : Bool = false"
    ],
    "documentation": "Member of EWM."
  },
  {
    "name": "std",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "EWM",
    "signature": "EWM.std(bias : Bool = false) : Series",
    "parameters": [
      "bias : Bool = false"
    ],
    "documentation": "Member of EWM."
  },
  {
    "name": "count",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "EWM",
    "signature": "EWM.count() : Series",
    "parameters": [],
    "documentation": "Member of EWM."
  },
  {
    "name": "swapcase",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.swapcase() : Series",
    "parameters": [],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "stripLeading",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.stripLeading() : Series",
    "parameters": [],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "stripTrailing",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.stripTrailing() : Series",
    "parameters": [],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "sliceReplace",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.sliceReplace(start : Int, stop : Int, replacement : String) : Series",
    "parameters": [
      "start : Int",
      "stop : Int",
      "replacement : String"
    ],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "repeat",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.repeat(times : Int) : Series",
    "parameters": [
      "times : Int"
    ],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "ljust",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.ljust(width : Int) : Series",
    "parameters": [
      "width : Int"
    ],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "rjust",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.rjust(width : Int) : Series",
    "parameters": [
      "width : Int"
    ],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "center",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.center(width : Int) : Series",
    "parameters": [
      "width : Int"
    ],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "zfill",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.zfill(width : Int) : Series",
    "parameters": [
      "width : Int"
    ],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "count",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.count(part : String) : Series",
    "parameters": [
      "part : String"
    ],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "find",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.find(part : String) : Series",
    "parameters": [
      "part : String"
    ],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "rfind",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.rfind(part : String) : Series",
    "parameters": [
      "part : String"
    ],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "get",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "StringMethods",
    "signature": "StringMethods.get(position : Int) : Series",
    "parameters": [
      "position : Int"
    ],
    "documentation": "Member of StringMethods."
  },
  {
    "name": "ndim",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.ndim() : Int",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "size",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.size() : Int",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "equals",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.equals(other : DataFrame) : Bool",
    "parameters": [
      "other : DataFrame"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "items",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.items() : List[(String, Series)]",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "takeGaps",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.takeGaps(positions : List[Int]) : DataFrame",
    "parameters": [
      "positions : List[Int]"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "iloc",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.iloc(position : Int) : Series",
    "parameters": [
      "position : Int"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "at",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.at(row : Int, column : String) : Scalar or Error",
    "parameters": [
      "row : Int",
      "column : String"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "iat",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.iat(row : Int, column : Int) : Scalar or Error",
    "parameters": [
      "row : Int",
      "column : Int"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "iterrows",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.iterrows() : List[(Scalar, Series)]",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "sortValues",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.sortValues(by : List[String], ascending : List[Bool], naPosition : String = \"last\") : DataFrame or Error",
    "parameters": [
      "by : List[String]",
      "ascending : List[Bool]",
      "naPosition : String = \"last\""
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "assign",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.assign(name : String, value : Int) : DataFrame",
    "parameters": [
      "name : String",
      "value : Int"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "assign",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.assign(name : String, value : double) : DataFrame",
    "parameters": [
      "name : String",
      "value : double"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "selectDtypes",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.selectDtypes(dtype : String) : DataFrame",
    "parameters": [
      "dtype : String"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "round",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.round(decimals : Int = 0) : DataFrame",
    "parameters": [
      "decimals : Int = 0"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "abs",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.abs() : DataFrame",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "clip",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.clip(low : double, high : double) : DataFrame",
    "parameters": [
      "low : double",
      "high : double"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "shift",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.shift(periods : Int = 1) : DataFrame",
    "parameters": [
      "periods : Int = 1"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "diff",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.diff(periods : Int = 1) : DataFrame",
    "parameters": [
      "periods : Int = 1"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "pctChange",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.pctChange(periods : Int = 1) : DataFrame",
    "parameters": [
      "periods : Int = 1"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "cumSum",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.cumSum() : DataFrame",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "cumProd",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.cumProd() : DataFrame",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "cumMax",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.cumMax() : DataFrame",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "cumMin",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.cumMin() : DataFrame",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "rank",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.rank(ascending : Bool = true, method : String = \"average\", pct : Bool = false) : DataFrame",
    "parameters": [
      "ascending : Bool = true",
      "method : String = \"average\"",
      "pct : Bool = false"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "interpolate",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.interpolate() : DataFrame",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "where",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.where(mask : DataFrame, other : Scalar) : DataFrame",
    "parameters": [
      "mask : DataFrame",
      "other : Scalar"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "mask",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.mask(mask : DataFrame, other : Scalar) : DataFrame",
    "parameters": [
      "mask : DataFrame",
      "other : Scalar"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "replace",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.replace(old : String, replacement : String) : DataFrame",
    "parameters": [
      "old : String",
      "replacement : String"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "replace",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.replace(old : Int, replacement : Int) : DataFrame",
    "parameters": [
      "old : Int",
      "replacement : Int"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "replace",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.replace(old : double, replacement : double) : DataFrame",
    "parameters": [
      "old : double",
      "replacement : double"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "apply",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.apply(f : double -> double) : DataFrame",
    "parameters": [
      "f : double -> double"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "insert",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.insert(position : Int, name : String, series : Series) : DataFrame or Error",
    "parameters": [
      "position : Int",
      "name : String",
      "series : Series"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "prod",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.prod() : Series",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "sem",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.sem() : Series",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "skew",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.skew() : Series",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "kurt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.kurt() : Series",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "any",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.any() : Series",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "all",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.all() : Series",
    "parameters": [],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "agg",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.agg(spec : List[(String, String)]) : Series",
    "parameters": [
      "spec : List[(String, String)]"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "quantile",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.quantile(q : double) : Series",
    "parameters": [
      "q : double"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "cov",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.cov(ddof : Int = 1) : DataFrame",
    "parameters": [
      "ddof : Int = 1"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "corrwith",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.corrwith(other : DataFrame) : Series",
    "parameters": [
      "other : DataFrame"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "valueCounts",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.valueCounts(column : String, normalize : Bool = false) : Series or Error",
    "parameters": [
      "column : String",
      "normalize : Bool = false"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "concatColumns",
    "module": "std.kesdas",
    "gated": true,
    "static": "DataFrame",
    "signature": "DataFrame.concatColumns(frames : List[DataFrame]) : DataFrame or Error",
    "parameters": [
      "frames : List[DataFrame]"
    ],
    "documentation": "Static member of DataFrame."
  },
  {
    "name": "melt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.melt(idVars : List[String] = [], valueVars : List[String] = [], varName : String = \"variable\", valueName : String = \"value\") : DataFrame or Error",
    "parameters": [
      "idVars : List[String] = []",
      "valueVars : List[String] = []",
      "varName : String = \"variable\"",
      "valueName : String = \"value\""
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "pivot",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.pivot(index : String, columns : String, values : String) : DataFrame or Error",
    "parameters": [
      "index : String",
      "columns : String",
      "values : String"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "crosstab",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.crosstab(index : String, columns : String) : DataFrame or Error",
    "parameters": [
      "index : String",
      "columns : String"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "getDummies",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.getDummies(columns : List[String] = [], prefix : String = \"\", dropFirst : Bool = false, dummyNa : Bool = false) : DataFrame or Error",
    "parameters": [
      "columns : List[String] = []",
      "prefix : String = \"\"",
      "dropFirst : Bool = false",
      "dummyNa : Bool = false"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "rolling",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "DataFrame",
    "signature": "DataFrame.rolling(window : Int, minPeriods : Int = -1) : FrameRolling",
    "parameters": [
      "window : Int",
      "minPeriods : Int = -1"
    ],
    "documentation": "Member of DataFrame."
  },
  {
    "name": "head",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.head(n : Int = 5) : DataFrame",
    "parameters": [
      "n : Int = 5"
    ],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "tail",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.tail(n : Int = 5) : DataFrame",
    "parameters": [
      "n : Int = 5"
    ],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "nth",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.nth(n : Int) : DataFrame",
    "parameters": [
      "n : Int"
    ],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "cumCount",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.cumCount(ascending : Bool = true) : Series",
    "parameters": [
      "ascending : Bool = true"
    ],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "ngroup",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.ngroup() : Series",
    "parameters": [],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "getGroup",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.getGroup(label : Int) : DataFrame or Error",
    "parameters": [
      "label : Int"
    ],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "getGroup",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.getGroup(label : String) : DataFrame or Error",
    "parameters": [
      "label : String"
    ],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "transformAll",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.transformAll(statistic : String) : DataFrame",
    "parameters": [
      "statistic : String"
    ],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "shift",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.shift(column : String, periods : Int = 1) : Series",
    "parameters": [
      "column : String",
      "periods : Int = 1"
    ],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "diff",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.diff(column : String, periods : Int = 1) : Series",
    "parameters": [
      "column : String",
      "periods : Int = 1"
    ],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "pctChange",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.pctChange(column : String, periods : Int = 1) : Series",
    "parameters": [
      "column : String",
      "periods : Int = 1"
    ],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "sem",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.sem() : DataFrame",
    "parameters": [],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "skew",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.skew() : DataFrame",
    "parameters": [],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "kurt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.kurt() : DataFrame",
    "parameters": [],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "all",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.all() : DataFrame",
    "parameters": [],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "any",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.any() : DataFrame",
    "parameters": [],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "quantile",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "GroupBy",
    "signature": "GroupBy.quantile(q : double) : DataFrame",
    "parameters": [
      "q : double"
    ],
    "documentation": "Member of GroupBy."
  },
  {
    "name": "frame",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "field": true,
    "signature": "FrameRolling.frame : DataFrame",
    "parameters": [],
    "documentation": "Field of FrameRolling."
  },
  {
    "name": "window",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "field": true,
    "signature": "FrameRolling.window : Int",
    "parameters": [],
    "documentation": "Field of FrameRolling."
  },
  {
    "name": "minPeriods",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "field": true,
    "signature": "FrameRolling.minPeriods : Int",
    "parameters": [],
    "documentation": "Field of FrameRolling."
  },
  {
    "name": "FrameRolling",
    "module": "std.kesdas",
    "gated": true,
    "named": true,
    "signature": "FrameRolling(move frame : DataFrame, window : Int, minPeriods : Int) : FrameRolling",
    "parameters": [
      "move frame : DataFrame",
      "window : Int",
      "minPeriods : Int"
    ],
    "documentation": "Builds a FrameRolling."
  },
  {
    "name": "mean",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "signature": "FrameRolling.mean() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameRolling."
  },
  {
    "name": "sum",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "signature": "FrameRolling.sum() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameRolling."
  },
  {
    "name": "min",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "signature": "FrameRolling.min() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameRolling."
  },
  {
    "name": "max",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "signature": "FrameRolling.max() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameRolling."
  },
  {
    "name": "std",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "signature": "FrameRolling.std() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameRolling."
  },
  {
    "name": "count",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "signature": "FrameRolling.count() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameRolling."
  },
  {
    "name": "median",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "signature": "FrameRolling.median() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameRolling."
  },
  {
    "name": "variance",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "signature": "FrameRolling.variance() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameRolling."
  },
  {
    "name": "sem",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "signature": "FrameRolling.sem() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameRolling."
  },
  {
    "name": "skew",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "signature": "FrameRolling.skew() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameRolling."
  },
  {
    "name": "kurt",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "signature": "FrameRolling.kurt() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameRolling."
  },
  {
    "name": "quantile",
    "module": "std.kesdas",
    "gated": true,
    "receiver": "FrameRolling",
    "signature": "FrameRolling.quantile(q : double) : DataFrame",
    "parameters": [
      "q : double"
    ],
    "documentation": "Member of FrameRolling."
  },
  {
    "name": "FrameEWM",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "FrameEWM",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "FrameExpanding",
    "module": "std.kesdas",
    "gated": true,
    "type": true,
    "signature": "FrameExpanding",
    "parameters": [],
    "documentation": "Imported kesdas type."
  },
  {
    "name": "positionOf",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.positionOf(label : Bool) : Option[Int]",
    "parameters": [
      "label : Bool"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "loc",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.loc(label : Bool) : Scalar",
    "parameters": [
      "label : Bool"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "at",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.at(label : Bool) : Scalar",
    "parameters": [
      "label : Bool"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "subtract",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.subtract(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "multiply",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.multiply(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "divide",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.divide(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "floorDivide",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.floorDivide(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "mod",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.mod(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "pow",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.pow(other : Int) : Series",
    "parameters": [
      "other : Int"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "add",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.add(other : String) : Series",
    "parameters": [
      "other : String"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "clip",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.clip(low : Int, high : Int) : Series",
    "parameters": [
      "low : Int",
      "high : Int"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "item",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.item(position : Int = 0) : Scalar",
    "parameters": [
      "position : Int = 0"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "isUnique",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.isUnique() : Bool",
    "parameters": [],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "isMonotonic",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.isMonotonic(increasing : Bool = true) : Bool",
    "parameters": [
      "increasing : Bool = true"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "repeat",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.repeat(times : Int) : Series",
    "parameters": [
      "times : Int"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "eq",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.eq(other : Bool) : Series",
    "parameters": [
      "other : Bool"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "ne",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.ne(other : Bool) : Series",
    "parameters": [
      "other : Bool"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "lt",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.lt(other : String) : Series",
    "parameters": [
      "other : String"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "le",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.le(other : String) : Series",
    "parameters": [
      "other : String"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "gt",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.gt(other : String) : Series",
    "parameters": [
      "other : String"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "ge",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.ge(other : String) : Series",
    "parameters": [
      "other : String"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "replace",
    "module": "std.kesdas",
    "gated": true,
    "signature": "Series.replace(old : Scalar, replacement : Scalar) : Series",
    "parameters": [
      "old : Scalar",
      "replacement : Scalar"
    ],
    "documentation": "Member of Series.",
    "receiver": "Series"
  },
  {
    "name": "assign",
    "module": "std.kesdas",
    "gated": true,
    "signature": "DataFrame.assign(name : String, value : Bool) : DataFrame",
    "parameters": [
      "name : String",
      "value : Bool"
    ],
    "documentation": "Member of DataFrame.",
    "receiver": "DataFrame"
  },
  {
    "name": "where",
    "module": "std.kesdas",
    "gated": true,
    "signature": "DataFrame.where(mask : DataFrame) : DataFrame",
    "parameters": [
      "mask : DataFrame"
    ],
    "documentation": "Member of DataFrame.",
    "receiver": "DataFrame"
  },
  {
    "name": "mask",
    "module": "std.kesdas",
    "gated": true,
    "signature": "DataFrame.mask(mask : DataFrame) : DataFrame",
    "parameters": [
      "mask : DataFrame"
    ],
    "documentation": "Member of DataFrame.",
    "receiver": "DataFrame"
  },
  {
    "name": "replace",
    "module": "std.kesdas",
    "gated": true,
    "signature": "DataFrame.replace(old : Scalar, replacement : Scalar) : DataFrame",
    "parameters": [
      "old : Scalar",
      "replacement : Scalar"
    ],
    "documentation": "Member of DataFrame.",
    "receiver": "DataFrame"
  },
  {
    "name": "idxMax",
    "module": "std.kesdas",
    "gated": true,
    "signature": "DataFrame.idxMax() : Series",
    "parameters": [],
    "documentation": "Member of DataFrame.",
    "receiver": "DataFrame"
  },
  {
    "name": "idxMin",
    "module": "std.kesdas",
    "gated": true,
    "signature": "DataFrame.idxMin() : Series",
    "parameters": [],
    "documentation": "Member of DataFrame.",
    "receiver": "DataFrame"
  },
  {
    "name": "mode",
    "module": "std.kesdas",
    "gated": true,
    "signature": "DataFrame.mode() : DataFrame",
    "parameters": [],
    "documentation": "Member of DataFrame.",
    "receiver": "DataFrame"
  },
  {
    "name": "quantile",
    "module": "std.kesdas",
    "gated": true,
    "signature": "DataFrame.quantile(qs : List[double]) : DataFrame",
    "parameters": [
      "qs : List[double]"
    ],
    "documentation": "Member of DataFrame.",
    "receiver": "DataFrame"
  },
  {
    "name": "valueCounts",
    "module": "std.kesdas",
    "gated": true,
    "signature": "DataFrame.valueCounts(subset : List[String] = [], normalize : Bool = false) : DataFrame or Error",
    "parameters": [
      "subset : List[String] = []",
      "normalize : Bool = false"
    ],
    "documentation": "Member of DataFrame.",
    "receiver": "DataFrame"
  },
  {
    "name": "transpose",
    "module": "std.kesdas",
    "gated": true,
    "signature": "DataFrame.transpose() : DataFrame or Error",
    "parameters": [],
    "documentation": "Member of DataFrame.",
    "receiver": "DataFrame"
  },
  {
    "name": "expanding",
    "module": "std.kesdas",
    "gated": true,
    "signature": "DataFrame.expanding(minPeriods : Int = 1) : FrameExpanding",
    "parameters": [
      "minPeriods : Int = 1"
    ],
    "documentation": "Member of DataFrame.",
    "receiver": "DataFrame"
  },
  {
    "name": "ewm",
    "module": "std.kesdas",
    "gated": true,
    "signature": "DataFrame.ewm(alpha : double, adjust : Bool = true, minPeriods : Int = 0) : FrameEWM",
    "parameters": [
      "alpha : double",
      "adjust : Bool = true",
      "minPeriods : Int = 0"
    ],
    "documentation": "Member of DataFrame.",
    "receiver": "DataFrame"
  },
  {
    "name": "getGroup",
    "module": "std.kesdas",
    "gated": true,
    "signature": "GroupBy.getGroup(label : Bool) : DataFrame or Error",
    "parameters": [
      "label : Bool"
    ],
    "documentation": "Member of GroupBy.",
    "receiver": "GroupBy"
  },
  {
    "name": "cumSum",
    "module": "std.kesdas",
    "gated": true,
    "signature": "GroupBy.cumSum(column : String) : Series",
    "parameters": [
      "column : String"
    ],
    "documentation": "Member of GroupBy.",
    "receiver": "GroupBy"
  },
  {
    "name": "cumProd",
    "module": "std.kesdas",
    "gated": true,
    "signature": "GroupBy.cumProd(column : String) : Series",
    "parameters": [
      "column : String"
    ],
    "documentation": "Member of GroupBy.",
    "receiver": "GroupBy"
  },
  {
    "name": "cumMax",
    "module": "std.kesdas",
    "gated": true,
    "signature": "GroupBy.cumMax(column : String) : Series",
    "parameters": [
      "column : String"
    ],
    "documentation": "Member of GroupBy.",
    "receiver": "GroupBy"
  },
  {
    "name": "cumMin",
    "module": "std.kesdas",
    "gated": true,
    "signature": "GroupBy.cumMin(column : String) : Series",
    "parameters": [
      "column : String"
    ],
    "documentation": "Member of GroupBy.",
    "receiver": "GroupBy"
  },
  {
    "name": "rank",
    "module": "std.kesdas",
    "gated": true,
    "signature": "GroupBy.rank(column : String, ascending : Bool = true, method : String = \"average\", pct : Bool = false) : Series",
    "parameters": [
      "column : String",
      "ascending : Bool = true",
      "method : String = \"average\"",
      "pct : Bool = false"
    ],
    "documentation": "Member of GroupBy.",
    "receiver": "GroupBy"
  },
  {
    "name": "frame",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding.frame : DataFrame",
    "parameters": [],
    "documentation": "Field of FrameExpanding.",
    "receiver": "FrameExpanding",
    "field": true
  },
  {
    "name": "minPeriods",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding.minPeriods : Int",
    "parameters": [],
    "documentation": "Field of FrameExpanding.",
    "receiver": "FrameExpanding",
    "field": true
  },
  {
    "name": "FrameExpanding",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding(move frame : DataFrame, minPeriods : Int) : FrameExpanding",
    "parameters": [
      "move frame : DataFrame",
      "minPeriods : Int"
    ],
    "documentation": "Builds a FrameExpanding.",
    "named": true
  },
  {
    "name": "mean",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding.mean() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameExpanding.",
    "receiver": "FrameExpanding"
  },
  {
    "name": "sum",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding.sum() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameExpanding.",
    "receiver": "FrameExpanding"
  },
  {
    "name": "min",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding.min() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameExpanding.",
    "receiver": "FrameExpanding"
  },
  {
    "name": "max",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding.max() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameExpanding.",
    "receiver": "FrameExpanding"
  },
  {
    "name": "std",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding.std() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameExpanding.",
    "receiver": "FrameExpanding"
  },
  {
    "name": "count",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding.count() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameExpanding.",
    "receiver": "FrameExpanding"
  },
  {
    "name": "median",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding.median() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameExpanding.",
    "receiver": "FrameExpanding"
  },
  {
    "name": "variance",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding.variance() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameExpanding.",
    "receiver": "FrameExpanding"
  },
  {
    "name": "sem",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding.sem() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameExpanding.",
    "receiver": "FrameExpanding"
  },
  {
    "name": "quantile",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameExpanding.quantile(q : double) : DataFrame",
    "parameters": [
      "q : double"
    ],
    "documentation": "Member of FrameExpanding.",
    "receiver": "FrameExpanding"
  },
  {
    "name": "frame",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameEWM.frame : DataFrame",
    "parameters": [],
    "documentation": "Field of FrameEWM.",
    "receiver": "FrameEWM",
    "field": true
  },
  {
    "name": "alpha",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameEWM.alpha : double",
    "parameters": [],
    "documentation": "Field of FrameEWM.",
    "receiver": "FrameEWM",
    "field": true
  },
  {
    "name": "adjust",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameEWM.adjust : Bool",
    "parameters": [],
    "documentation": "Field of FrameEWM.",
    "receiver": "FrameEWM",
    "field": true
  },
  {
    "name": "minPeriods",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameEWM.minPeriods : Int",
    "parameters": [],
    "documentation": "Field of FrameEWM.",
    "receiver": "FrameEWM",
    "field": true
  },
  {
    "name": "FrameEWM",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameEWM(move frame : DataFrame, alpha : double, adjust : Bool, minPeriods : Int) : FrameEWM",
    "parameters": [
      "move frame : DataFrame",
      "alpha : double",
      "adjust : Bool",
      "minPeriods : Int"
    ],
    "documentation": "Builds a FrameEWM.",
    "named": true
  },
  {
    "name": "mean",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameEWM.mean() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameEWM.",
    "receiver": "FrameEWM"
  },
  {
    "name": "sum",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameEWM.sum() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameEWM.",
    "receiver": "FrameEWM"
  },
  {
    "name": "variance",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameEWM.variance(bias : Bool = false) : DataFrame",
    "parameters": [
      "bias : Bool = false"
    ],
    "documentation": "Member of FrameEWM.",
    "receiver": "FrameEWM"
  },
  {
    "name": "std",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameEWM.std(bias : Bool = false) : DataFrame",
    "parameters": [
      "bias : Bool = false"
    ],
    "documentation": "Member of FrameEWM.",
    "receiver": "FrameEWM"
  },
  {
    "name": "count",
    "module": "std.kesdas",
    "gated": true,
    "signature": "FrameEWM.count() : DataFrame",
    "parameters": [],
    "documentation": "Member of FrameEWM.",
    "receiver": "FrameEWM"
  }
];

const fieldTypes = new Map();
for (const entry of entries) {
  if (entry.field) fieldTypes.set(`${entry.receiver}.${entry.name}`, entry.signature.slice(entry.signature.lastIndexOf(':') + 1).trim());
}

module.exports = { types, names, entries, fieldTypes };
