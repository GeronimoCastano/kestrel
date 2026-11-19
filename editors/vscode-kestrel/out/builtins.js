'use strict';

const distributions = require('./distributions');
const kesdas = require('./kesdas');
const ml = require('./ml');
const modules = require('./modules');
const kesnet = require('./kesnet');
const kesnetOptim = require('./kesnet_optim');
const kesnetData = require('./kesnet_data');
const variable = require('./variable');

// Type information for the compiler-provided standard library. Keep this in
// step with src/checker.rs and src/modules.rs; the extension is deliberately
// independent of the compiler, so a hover must not have to run it.

const entries = [
  {"name": "Tensor", "module": "std.core", "signature": "Tensor[double, shape]", "parameters": [], "documentation": "An owned mutable tensor of doubles with a checked static shape."},
  {"name": "DynTensor", "module": "std.core", "signature": "DynTensor[double]", "parameters": [], "documentation": "An owned mutable tensor of doubles whose shape is checked at run time."},
  {"name": "zeros", "module": "std.tensor", "static": "Tensor", "named": false, "signature": "Tensor.zeros(shape : (Int, ...) or Array[Int]) : Tensor[double, shape]", "parameters": ["shape : (Int, ...) or Array[Int]"], "documentation": "Creates a zero-filled tensor. Shape accepts a tuple or Array[Int]."},
  {"name": "filled", "module": "std.tensor", "static": "Tensor", "named": false, "signature": "Tensor.filled(shape : (Int, ...) or Array[Int], value : double) : Tensor[double, shape]", "parameters": ["shape : (Int, ...) or Array[Int]", "value : double"], "documentation": "Creates a tensor filled with one double. Shape accepts a tuple or Array[Int]."},
  {"name": "generate", "module": "std.tensor", "static": "Tensor", "named": false, "signature": "Tensor.generate(shape : (Int, ...) or Array[Int], factory : (Int, ...) -> double) : Tensor[double, shape]", "parameters": ["shape : (Int, ...) or Array[Int]", "factory : (Int, ...) -> double"], "documentation": "Calls the factory once per element in row-major order. Variable dimensions return DynTensor."},
  {"name": "ones", "module": "std.tensor", "static": "Tensor", "named": false, "signature": "Tensor.ones(shape : (Int, ...) or Array[Int]) : Tensor[double, shape]", "parameters": ["shape : (Int, ...) or Array[Int]"], "documentation": "Creates a tensor of ones with the given shape."},
  {"name": "zerosLike", "module": "std.tensor", "static": "Tensor", "named": false, "signature": "Tensor.zerosLike(source : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["source : Tensor[double, shape]"], "documentation": "Creates an independent tensor of zeros with the source shape."},
  {"name": "onesLike", "module": "std.tensor", "static": "Tensor", "named": false, "signature": "Tensor.onesLike(source : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["source : Tensor[double, shape]"], "documentation": "Creates an independent tensor of ones with the source shape."},
  {"name": "fromArray", "module": "std.tensor", "static": "Tensor", "named": false, "signature": "Tensor.fromArray(source : Array[double]) : DynTensor[double]", "parameters": ["source : Array[double]"], "documentation": "Copies an Array into a rank-one tensor; the Array remains usable."},
  {"name": "copy", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].copy() : Tensor[double, shape]", "parameters": [], "documentation": "Copies the data into a second independent owner."},
  {"name": "T", "receiver": "Tensor[double, shape]", "field": true, "signature": "Tensor[double, shape].T : Tensor[double, reversed shape]", "parameters": [], "documentation": "A read-only transpose view. Reverses every axis without copying; writes are refused."},
  {"name": "negate", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].negate() : Tensor[double, shape]", "parameters": [], "documentation": "Negates every element, as unary minus does."},
  {"name": "power", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].power(exponent : Int or double) : Tensor[double, shape]", "parameters": ["exponent : Int or double"], "documentation": "An Int exponent on a square matrix is a matrix power; other cases are elementwise."},
  {"name": "text", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].text() : String", "parameters": [], "documentation": "Nested brackets that follow the shape, such as [[1.0, 2.0], [3.0, 4.0]]."},
  {"name": "transpose", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].transpose() : Tensor[double, reversed shape]", "parameters": [], "documentation": "Reverses every axis, as NumPy's .T does. A 2-by-3 tensor becomes 3-by-2, and a 2-by-3-by-4 tensor becomes 4-by-3-by-2."},
  {"name": "transpose", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].transpose(axes : (Int, ...) or Array[Int]) : Tensor[double, permuted shape]", "parameters": ["axes : (Int, ...) or Array[Int]"], "documentation": "Moves the axes: result axis i is source axis axes[i]. transpose((1, 0, 2)) swaps the first two axes of a 3-D tensor."},
  {"name": "transpose", "receiver": "DynTensor[double]", "signature": "DynTensor[double].transpose() : DynTensor[double]", "parameters": [], "documentation": "Reverses every axis when the shape is known only at run time."},
  {"name": "transpose", "receiver": "DynTensor[double]", "signature": "DynTensor[double].transpose(axes : (Int, ...) or Array[Int]) : DynTensor[double]", "parameters": ["axes : (Int, ...) or Array[Int]"], "documentation": "Permutes the axes when the shape is known only at run time."},
  {"name": "checkShape", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].checkShape(dimensions : Int, ...) : Tensor[double, shape] or Error", "parameters": ["dimensions : Int, ..."], "documentation": "Checks dimensions and returns an independent copy on success. A mismatch returns RuntimeError.Message."},
  {"name": "broadcastTo", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].broadcastTo(shape : (Int, ...) or Array[Int]) : Tensor[double, shape]", "parameters": ["shape : (Int, ...) or Array[Int]"], "documentation": "Copies into an explicit shape tuple or Array[Int]. Trailing dimensions must match or the source dimension must be one."},
  {"name": "copy", "receiver": "DynTensor[double]", "signature": "DynTensor[double].copy() : DynTensor[double]", "parameters": [], "documentation": "Copies the data into a second independent owner."},
  {"name": "T", "receiver": "DynTensor[double]", "field": true, "signature": "DynTensor[double].T : DynTensor[double]", "parameters": [], "documentation": "A read-only transpose view. Reverses every axis without copying; writes are refused."},
  {"name": "negate", "receiver": "DynTensor[double]", "signature": "DynTensor[double].negate() : DynTensor[double]", "parameters": [], "documentation": "Negates every element, as unary minus does."},
  {"name": "power", "receiver": "DynTensor[double]", "signature": "DynTensor[double].power(exponent : Int or double) : DynTensor[double]", "parameters": ["exponent : Int or double"], "documentation": "An Int exponent on a runtime square matrix is a matrix power; other cases are elementwise."},
  {"name": "checkShape", "receiver": "DynTensor[double]", "signature": "DynTensor[double].checkShape(dimensions : Int, ...) : Tensor[double, shape] or Error", "parameters": ["dimensions : Int, ..."], "documentation": "Checks dimensions and returns an independent copy on success. A mismatch returns RuntimeError.Message."},
  {"name": "broadcastTo", "receiver": "DynTensor[double]", "signature": "DynTensor[double].broadcastTo(shape : (Int, ...) or Array[Int]) : Tensor[double, shape]", "parameters": ["shape : (Int, ...) or Array[Int]"], "documentation": "Copies into an explicit shape tuple or Array[Int]. Trailing dimensions must match or the source dimension must be one."},
  {"name": "rank", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].rank() : Int", "parameters": [], "documentation": "The number of dimensions."},
  {"name": "shape", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].shape() : Array[Int]", "parameters": [], "documentation": "Copies the dimensions into a new Array of Int values."},
  {"name": "count", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].count() : Int", "parameters": [], "documentation": "The total number of elements."},
  {"name": "debug", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].debug() : String", "parameters": [], "documentation": "The type and actual shape followed by all values, such as Tensor[double, (2, 3)] [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]."},
  {"name": "map", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].map(transform : double -> double) : Tensor[double, shape]", "parameters": ["transform : double -> double"], "documentation": "Returns an independent tensor with the same shape, applying transform to every double in row-major order."},
  {"name": "reshape", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].reshape(shape : (Int, ...) or Array[Int]) : Tensor[double, shape]", "parameters": ["shape : (Int, ...) or Array[Int]"], "documentation": "Copies the row-major elements into a target tuple or Array[Int]. The element count must match; variable dimensions return DynTensor."},
  {"name": "sum", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].sum() : double", "parameters": [], "documentation": "Reduces all elements to one double. An empty tensor sums to 0.0."},
  {"name": "sum", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].sum(axis : Int) : Tensor[double, reduced shape] or double", "parameters": ["axis : Int"], "documentation": "Reduces the named axis and drops that dimension. A static rank-1 tensor returns double; DynTensor returns DynTensor, including rank-0 results. An empty tensor sums to 0.0."},
  {"name": "mean", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].mean() : double", "parameters": [], "documentation": "Reduces all elements to one double. An empty tensor stops the program."},
  {"name": "mean", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].mean(axis : Int) : Tensor[double, reduced shape] or double", "parameters": ["axis : Int"], "documentation": "Reduces the named axis and drops that dimension. A static rank-1 tensor returns double; DynTensor returns DynTensor, including rank-0 results. An empty tensor stops the program."},
  {"name": "min", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].min() : double", "parameters": [], "documentation": "Reduces all elements to one double. An empty tensor stops the program."},
  {"name": "min", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].min(axis : Int) : Tensor[double, reduced shape] or double", "parameters": ["axis : Int"], "documentation": "Reduces the named axis and drops that dimension. A static rank-1 tensor returns double; DynTensor returns DynTensor, including rank-0 results. An empty tensor stops the program."},
  {"name": "max", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].max() : double", "parameters": [], "documentation": "Reduces all elements to one double. An empty tensor stops the program."},
  {"name": "max", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].max(axis : Int) : Tensor[double, reduced shape] or double", "parameters": ["axis : Int"], "documentation": "Reduces the named axis and drops that dimension. A static rank-1 tensor returns double; DynTensor returns DynTensor, including rank-0 results. An empty tensor stops the program."},
  {"name": "add", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].add(other : double) : Tensor[double, shape]", "parameters": ["other : double"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "add", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].add(other : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["other : Tensor[double, shape]"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "subtract", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].subtract(other : double) : Tensor[double, shape]", "parameters": ["other : double"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "subtract", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].subtract(other : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["other : Tensor[double, shape]"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "multiply", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].multiply(other : double) : Tensor[double, shape]", "parameters": ["other : double"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "multiply", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].multiply(other : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["other : Tensor[double, shape]"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "rightAdd", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].rightAdd(left : double) : Tensor[double, shape]", "parameters": ["left : double"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "rightSubtract", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].rightSubtract(left : double) : Tensor[double, shape]", "parameters": ["left : double"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "rightMultiply", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].rightMultiply(left : double) : Tensor[double, shape]", "parameters": ["left : double"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "rank", "receiver": "DynTensor[double]", "signature": "DynTensor[double].rank() : Int", "parameters": [], "documentation": "The number of dimensions."},
  {"name": "shape", "receiver": "DynTensor[double]", "signature": "DynTensor[double].shape() : Array[Int]", "parameters": [], "documentation": "Copies the dimensions into a new Array of Int values."},
  {"name": "count", "receiver": "DynTensor[double]", "signature": "DynTensor[double].count() : Int", "parameters": [], "documentation": "The total number of elements."},
  {"name": "debug", "receiver": "DynTensor[double]", "signature": "DynTensor[double].debug() : String", "parameters": [], "documentation": "The type and actual shape followed by all values, such as Tensor[double, (2, 3)] [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]."},
  {"name": "text", "receiver": "DynTensor[double]", "signature": "DynTensor[double].text() : String", "parameters": [], "documentation": "Writes the values in nested brackets that follow the shape."},
  {"name": "map", "receiver": "DynTensor[double]", "signature": "DynTensor[double].map(transform : double -> double) : DynTensor[double]", "parameters": ["transform : double -> double"], "documentation": "Returns an independent tensor with the same shape, applying transform to every double in row-major order."},
  {"name": "reshape", "receiver": "DynTensor[double]", "signature": "DynTensor[double].reshape(shape : (Int, ...) or Array[Int]) : Tensor[double, shape]", "parameters": ["shape : (Int, ...) or Array[Int]"], "documentation": "Copies the row-major elements into a target tuple or Array[Int]. The element count must match; variable dimensions return DynTensor."},
  {"name": "sum", "receiver": "DynTensor[double]", "signature": "DynTensor[double].sum() : double", "parameters": [], "documentation": "Reduces all elements to one double. An empty tensor sums to 0.0."},
  {"name": "sum", "receiver": "DynTensor[double]", "signature": "DynTensor[double].sum(axis : Int) : DynTensor[double]", "parameters": ["axis : Int"], "documentation": "Reduces the named axis and drops that dimension. A static rank-1 tensor returns double; DynTensor returns DynTensor, including rank-0 results. An empty tensor sums to 0.0."},
  {"name": "mean", "receiver": "DynTensor[double]", "signature": "DynTensor[double].mean() : double", "parameters": [], "documentation": "Reduces all elements to one double. An empty tensor stops the program."},
  {"name": "mean", "receiver": "DynTensor[double]", "signature": "DynTensor[double].mean(axis : Int) : DynTensor[double]", "parameters": ["axis : Int"], "documentation": "Reduces the named axis and drops that dimension. A static rank-1 tensor returns double; DynTensor returns DynTensor, including rank-0 results. An empty tensor stops the program."},
  {"name": "min", "receiver": "DynTensor[double]", "signature": "DynTensor[double].min() : double", "parameters": [], "documentation": "Reduces all elements to one double. An empty tensor stops the program."},
  {"name": "min", "receiver": "DynTensor[double]", "signature": "DynTensor[double].min(axis : Int) : DynTensor[double]", "parameters": ["axis : Int"], "documentation": "Reduces the named axis and drops that dimension. A static rank-1 tensor returns double; DynTensor returns DynTensor, including rank-0 results. An empty tensor stops the program."},
  {"name": "max", "receiver": "DynTensor[double]", "signature": "DynTensor[double].max() : double", "parameters": [], "documentation": "Reduces all elements to one double. An empty tensor stops the program."},
  {"name": "max", "receiver": "DynTensor[double]", "signature": "DynTensor[double].max(axis : Int) : DynTensor[double]", "parameters": ["axis : Int"], "documentation": "Reduces the named axis and drops that dimension. A static rank-1 tensor returns double; DynTensor returns DynTensor, including rank-0 results. An empty tensor stops the program."},
  {"name": "add", "receiver": "DynTensor[double]", "signature": "DynTensor[double].add(other : double) : DynTensor[double]", "parameters": ["other : double"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "add", "receiver": "DynTensor[double]", "signature": "DynTensor[double].add(other : DynTensor[double]) : DynTensor[double]", "parameters": ["other : DynTensor[double]"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "subtract", "receiver": "DynTensor[double]", "signature": "DynTensor[double].subtract(other : double) : DynTensor[double]", "parameters": ["other : double"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "subtract", "receiver": "DynTensor[double]", "signature": "DynTensor[double].subtract(other : DynTensor[double]) : DynTensor[double]", "parameters": ["other : DynTensor[double]"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "multiply", "receiver": "DynTensor[double]", "signature": "DynTensor[double].multiply(other : double) : DynTensor[double]", "parameters": ["other : double"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "multiply", "receiver": "DynTensor[double]", "signature": "DynTensor[double].multiply(other : DynTensor[double]) : DynTensor[double]", "parameters": ["other : DynTensor[double]"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "rightAdd", "receiver": "DynTensor[double]", "signature": "DynTensor[double].rightAdd(left : double) : DynTensor[double]", "parameters": ["left : double"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "rightSubtract", "receiver": "DynTensor[double]", "signature": "DynTensor[double].rightSubtract(left : double) : DynTensor[double]", "parameters": ["left : double"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "rightMultiply", "receiver": "DynTensor[double]", "signature": "DynTensor[double].rightMultiply(left : double) : DynTensor[double]", "parameters": ["left : double"], "documentation": "Returns an independent tensor. Tensor operands need equal shapes; scalars are doubles."},
  {"name": "Tensor", "module": "std.core", "named": true, "signature": "Tensor(values : List[double] or Array[double]) : Tensor[double, (length,)]", "parameters": ["values : List[double] or Array[double]"], "documentation": "Copies values into an owned tensor. A direct list literal has a static length; other lengths are checked at run time."},
  {"name": "Tensor", "module": "std.core", "named": true, "signature": "Tensor(values : List[double] or Array[double], shape : (Int, ...) or Array[Int]) : Tensor[double, shape]", "parameters": ["values : List[double] or Array[double]", "shape : (Int, ...) or Array[Int]"], "documentation": "Constructs the final shape directly. Its element count must match; Array shapes return DynTensor."},
  {"name": "double", "module": "std.core", "named": true, "signature": "double(value : Int or Nat or double) : double", "parameters": ["value : Int or Nat or double"], "documentation": "Explicit numeric conversion. Integers can round; text and BigInt use parseDouble and checkedToDouble."},
  {"name": "String", "module": "std.core", "named": true, "signature": "String(value : T) : String", "parameters": ["value : T"], "documentation": "Converts any Text value using its text() implementation."},
  {"name": "divide", "receiver": "Tensor[double, shape]", "signature": "Tensor[double, shape].divide(other : double) : Tensor[double, shape]", "parameters": ["other : double"], "documentation": "Divides every element by a double scalar into independent storage. Keeps the source shape and follows ordinary double division, including infinity and NaN for zero divisors."},
  {"name": "divide", "receiver": "DynTensor[double]", "signature": "DynTensor[double].divide(other : double) : DynTensor[double]", "parameters": ["other : double"], "documentation": "Divides every element by a double scalar into independent storage. Keeps the source shape and follows ordinary double division, including infinity and NaN for zero divisors."},
  // std.core
  {
    name: 'range',
    module: 'std.core',
    named: true,
    signature: 'range(stop : Int) : Lazy[Int]',
    parameters: ['stop : Int'],
    documentation: 'Zero through stop, exclusive, as a lazy iterable.'
  },
  {
    name: 'range',
    module: 'std.core',
    named: true,
    signature: 'range(start : Int, stop : Int) : Lazy[Int]',
    parameters: ['start : Int', 'stop : Int'],
    documentation: 'Start through stop, exclusive, as a lazy iterable.'
  },
  {
    name: 'range',
    module: 'std.core',
    named: true,
    signature: 'range(start : Int, stop : Int, step : Int) : Lazy[Int]',
    parameters: ['start : Int', 'stop : Int', 'step : Int'],
    documentation: 'A lazy arithmetic range with an explicit step.'
  },
  {
    name: 'Array',
    module: 'std.core',
    named: false,
    signature: 'Array(size : Int, value : T) : Array[T]',
    parameters: ['size : Int', 'value : T'],
    documentation: 'An Array with `size` slots, each holding a copy of `value`. The element type must be copyable; use Array.generate for other element types.'
  },
  {"name": "Array", "module": "std.core", "named": true, "signature": "Array(values : Iterable[T]) : Array[T]", "parameters": ["values : Iterable[T]"], "documentation": "Copies copyable items into a new Array, following toArray permissions and effects."},
  {
    name: 'generate',
    module: 'std.array',
    static: 'Array',
    named: false,
    signature: 'Array.generate(size : Int, factory : Int -> T) : Array[T]',
    parameters: ['size : Int', 'factory : Int -> T'],
    documentation: 'An Array with `size` slots. The factory runs once for each index, in order, and each result moves into its slot. Factory effects need `with Effects`.'
  },
  {
    name: 'print',
    module: 'std.core',
    named: true,
    signature: 'print[T : Text](value : T) : void with Effects',
    parameters: ['value : T'],
    documentation: 'Writes a value through its Text implementation. This is outside work, so the calling function needs `with Effects`.'
  },
  {
    name: 'parseInt',
    module: 'std.core',
    signature: 'parseInt(text : String) : Int or Error',
    parameters: ['text : String'],
    documentation: 'Parses decimal integer text. A bad number is an Error.'
  },
  {
    name: 'parseDouble',
    module: 'std.core',
    signature: 'parseDouble(text : String) : double or Error',
    parameters: ['text : String'],
    documentation: 'Parses finite double text. A bad number is an Error.'
  },
  {
    name: 'parseBigInt',
    module: 'std.core',
    signature: 'parseBigInt(text : String) : BigInt or Error',
    parameters: ['text : String'],
    documentation: 'Parses arbitrary-precision integer text. A bad number is an Error.'
  },
  {
    name: 'box',
    module: 'std.core',
    signature: 'box[T](value : T) : Box[T]',
    parameters: ['value : T'],
    documentation: 'Puts a value in a box, which gives it a stable address.'
  },
  {
    name: 'Some',
    module: 'std.core',
    signature: 'Some[T](value : T) : Option[T]',
    parameters: ['value : T'],
    documentation: 'An Option with a value.'
  },
  {
    name: 'None',
    module: 'std.core',
    signature: 'None : Option[T]',
    parameters: [],
    documentation: 'The empty Option.'
  },
  {
    name: 'open',
    module: 'std.core',
    signature: 'open(path : String) : Reader or Error with Effects',
    parameters: ['path : String'],
    documentation: 'Opens a file for reading. Close it with a `using` block.'
  },
  {
    name: 'create',
    module: 'std.core',
    signature: 'create(path : String) : Writer or Error with Effects',
    parameters: ['path : String'],
    documentation: 'Creates a file for writing. Close it with a `using` block.'
  },
  {
    name: 'readLine',
    module: 'std.core',
    signature: 'readLine() : String or Error with Effects',
    parameters: [],
    documentation: 'Reads one line of text from the terminal.'
  },

  // std.test
  {
    name: 'assertEq',
    module: 'std.test',
    named: true,
    signature: 'assertEq[T : Equals](a : T, b : T) : void',
    parameters: ['a : T', 'b : T'],
    documentation: 'Checks that two equalable values are equal.'
  },

  // std.process
  {
    name: 'args',
    module: 'std.process',
    signature: 'args : List[String]',
    parameters: [],
    documentation: 'The command-line arguments after `--`.'
  },

  // std.math
  {
    name: 'PI',
    module: 'std.math',
    signature: 'PI : double',
    parameters: [],
    documentation: 'The ratio of a circle’s circumference to its diameter.'
  },
  {
    name: 'E',
    module: 'std.math',
    signature: 'E : double',
    parameters: [],
    documentation: 'Euler’s number.'
  },
  {
    name: 'sqrt',
    module: 'std.math',
    signature: 'sqrt(x : double) : double',
    parameters: ['x : double'],
    documentation: 'The non-negative square root.'
  },
  {
    name: 'sqrt',
    module: 'std.math',
    signature: 'sqrt(x : Tensor[double, shape]) : Tensor[double, shape]',
    parameters: ['x : Tensor[double, shape]'],
    documentation: 'The non-negative square root of every element.'
  },
  {
    name: 'pow',
    module: 'std.math',
    signature: 'pow(base : double, exponent : double) : double',
    parameters: ['base : double', 'exponent : double'],
    documentation: 'Raises the base to the exponent.'
  },
  {
    name: 'floor',
    module: 'std.math',
    signature: 'floor(x : double) : double',
    parameters: ['x : double'],
    documentation: 'The largest whole number not above x.'
  },
  {
    name: 'floor',
    module: 'std.math',
    signature: 'floor(x : Tensor[double, shape]) : Tensor[double, shape]',
    parameters: ['x : Tensor[double, shape]'],
    documentation: 'The largest whole number not above each element.'
  },
  {
    name: 'ceil',
    module: 'std.math',
    signature: 'ceil(x : double) : double',
    parameters: ['x : double'],
    documentation: 'The smallest whole number not below x.'
  },
  {
    name: 'ceil',
    module: 'std.math',
    signature: 'ceil(x : Tensor[double, shape]) : Tensor[double, shape]',
    parameters: ['x : Tensor[double, shape]'],
    documentation: 'The smallest whole number not below each element.'
  },
  {
    name: 'round',
    module: 'std.math',
    signature: 'round(x : double) : double',
    parameters: ['x : double'],
    documentation: 'The nearest whole number, with halves moved away from zero.'
  },
  {
    name: 'round',
    module: 'std.math',
    signature: 'round(x : Tensor[double, shape]) : Tensor[double, shape]',
    parameters: ['x : Tensor[double, shape]'],
    documentation: 'The nearest whole number for each element, with halves moved away from zero.'
  },
  {
    name: 'abs',
    module: 'std.math',
    signature: 'abs(x : Int) : Int',
    parameters: ['x : Int'],
    documentation: 'The absolute value of an Int.'
  },
  {
    name: 'abs',
    module: 'std.math',
    signature: 'abs(x : double) : double',
    parameters: ['x : double'],
    documentation: 'The absolute value of a double.'
  },
  {
    name: 'abs',
    module: 'std.math',
    signature: 'abs(x : BigInt) : BigInt',
    parameters: ['x : BigInt'],
    documentation: 'The absolute value of a BigInt.'
  },
  {
    name: 'abs',
    module: 'std.math',
    signature: 'abs(x : Tensor[double, shape]) : Tensor[double, shape]',
    parameters: ['x : Tensor[double, shape]'],
    documentation: 'The absolute value of every element.'
  },
  {
    name: 'min',
    module: 'std.math',
    signature: 'min(a : Int, b : Int) : Int',
    parameters: ['a : Int', 'b : Int'],
    documentation: 'The smaller of two Int values.'
  },
  {
    name: 'min',
    module: 'std.math',
    signature: 'min(a : double, b : double) : double',
    parameters: ['a : double', 'b : double'],
    documentation: 'The smaller of two double values.'
  },
  {
    name: 'min',
    module: 'std.math',
    signature: 'min(a : BigInt, b : BigInt) : BigInt',
    parameters: ['a : BigInt', 'b : BigInt'],
    documentation: 'The smaller of two BigInt values.'
  },
  {
    name: 'max',
    module: 'std.math',
    signature: 'max(a : Int, b : Int) : Int',
    parameters: ['a : Int', 'b : Int'],
    documentation: 'The larger of two Int values.'
  },
  {
    name: 'max',
    module: 'std.math',
    signature: 'max(a : double, b : double) : double',
    parameters: ['a : double', 'b : double'],
    documentation: 'The larger of two double values.'
  },
  {
    name: 'max',
    module: 'std.math',
    signature: 'max(a : BigInt, b : BigInt) : BigInt',
    parameters: ['a : BigInt', 'b : BigInt'],
    documentation: 'The larger of two BigInt values.'
  },

  // std.str
  {
    name: 'trim',
    module: 'std.str',
    signature: 'trim(text : String) : String',
    parameters: ['text : String'],
    documentation: 'Removes leading and trailing whitespace.'
  },
  {
    name: 'toUpper',
    module: 'std.str',
    signature: 'toUpper(text : String) : String',
    parameters: ['text : String'],
    documentation: 'Converts text to uppercase.'
  },
  {
    name: 'toLower',
    module: 'std.str',
    signature: 'toLower(text : String) : String',
    parameters: ['text : String'],
    documentation: 'Converts text to lowercase.'
  },
  {
    name: 'length',
    module: 'std.str',
    signature: 'length(text : String) : Int',
    parameters: ['text : String'],
    documentation: 'Counts Unicode scalar values.'
  },
  {
    name: 'contains',
    module: 'std.str',
    signature: 'contains(text : String, part : String) : Bool',
    parameters: ['text : String', 'part : String'],
    documentation: 'Whether one String contains another.'
  },
  {
    name: 'split',
    module: 'std.str',
    signature: 'split(text : String, separator : String) : List[String]',
    parameters: ['text : String', 'separator : String'],
    documentation: 'Splits text, preserving empty pieces.'
  },

  // Common collection and protocol methods.
  { name: 'length', receiver: 'List[T]', signature: 'List[T].length() : Int', parameters: [], documentation: 'The stored element count.' },
  { name: 'length', receiver: 'String', signature: 'String.length() : Int', parameters: [], documentation: 'Counts Unicode scalar values.' },
  { name: 'length', receiver: 'Map[K, V]', signature: 'Map[K, V].length() : Int', parameters: [], documentation: 'The number of entries.' },
  { name: 'length', receiver: 'Set[T]', signature: 'Set[T].length() : Int', parameters: [], documentation: 'The number of elements.' },
  { name: 'length', receiver: 'SortedSet[T]', signature: 'SortedSet[T].length() : Int', parameters: [], documentation: 'The number of elements.' },
  { name: 'length', receiver: 'Array[T]', signature: 'Array[T].length() : Int', parameters: [], documentation: 'The current number of slots.' },
  { name: 'prepend', receiver: 'Array[T]', signature: 'Array[T].prepend(value : T) : void', parameters: ['value : T'], documentation: 'Adds the value at the front, moving the other items one slot right. Needs a mutable array.' },
  { name: 'join', receiver: 'Iterable[T]', signature: 'Iterable[T].join(separator : String) : String', parameters: ['separator : String'], documentation: 'Joins the items into one String with the separator between them. Each item uses its Text form.' },
  { name: 'at', receiver: 'List[T]', signature: 'List[T].at(index : Int) : T', parameters: ['index : Int'], documentation: 'The item at a position, read in constant time without copying the whole List. Only Lists of Int, Nat, double, Bool or String have it, and an index outside the list is a runtime error.' },
  { name: 'index', receiver: 'List[T]', signature: 'List[T].index(value : T) : Option[Int]', parameters: ['value : T'], documentation: 'The position of the first equal item, or None.' },
  { name: 'index', receiver: 'Array[T]', signature: 'Array[T].index(value : T) : Option[Int]', parameters: ['value : T'], documentation: 'The position of the first equal item, or None.' },
  { name: 'trim', receiver: 'String', signature: 'String.trim() : String', parameters: [], documentation: 'The text without leading and trailing whitespace.' },
  { name: 'toUpper', receiver: 'String', signature: 'String.toUpper() : String', parameters: [], documentation: 'The text in upper case.' },
  { name: 'toLower', receiver: 'String', signature: 'String.toLower() : String', parameters: [], documentation: 'The text in lower case.' },
  { name: 'isEmpty', receiver: 'String', signature: 'String.isEmpty() : Bool', parameters: [], documentation: 'Whether the text has no characters.' },
  { name: 'contains', receiver: 'String', signature: 'String.contains(part : String) : Bool', parameters: ['part : String'], documentation: 'Whether the text contains the part.' },
  { name: 'startsWith', receiver: 'String', signature: 'String.startsWith(prefix : String) : Bool', parameters: ['prefix : String'], documentation: 'Whether the text begins with the prefix.' },
  { name: 'endsWith', receiver: 'String', signature: 'String.endsWith(suffix : String) : Bool', parameters: ['suffix : String'], documentation: 'Whether the text ends with the suffix.' },
  { name: 'split', receiver: 'String', signature: 'String.split(separator : String) : List[String]', parameters: ['separator : String'], documentation: 'Splits the text at each separator, keeping empty pieces.' },
  { name: 'abs', receiver: 'Int', signature: 'Int.abs() : Int', parameters: [], documentation: 'The absolute value.' },
  { name: 'min', receiver: 'Int', signature: 'Int.min(other : Int) : Int', parameters: ['other : Int'], documentation: 'The smaller of the two numbers.' },
  { name: 'max', receiver: 'Int', signature: 'Int.max(other : Int) : Int', parameters: ['other : Int'], documentation: 'The larger of the two numbers.' },
  { name: 'isEven', receiver: 'Int', signature: 'Int.isEven() : Bool', parameters: [], documentation: 'Whether the number is divisible by two.' },
  { name: 'isOdd', receiver: 'Int', signature: 'Int.isOdd() : Bool', parameters: [], documentation: 'Whether the number is not divisible by two.' },
  { name: 'abs', receiver: 'double', signature: 'double.abs() : double', parameters: [], documentation: 'The absolute value.' },
  { name: 'sqrt', receiver: 'double', signature: 'double.sqrt() : double', parameters: [], documentation: 'The square root.' },
  { name: 'pow', receiver: 'double', signature: 'double.pow(exponent : double) : double', parameters: ['exponent : double'], documentation: 'This number raised to the exponent.' },
  { name: 'floor', receiver: 'double', signature: 'double.floor() : double', parameters: [], documentation: 'The greatest whole number not above this one.' },
  { name: 'ceil', receiver: 'double', signature: 'double.ceil() : double', parameters: [], documentation: 'The least whole number not below this one.' },
  { name: 'round', receiver: 'double', signature: 'double.round() : double', parameters: [], documentation: 'The nearest whole number, with halves rounded away from zero.' },
  { name: 'min', receiver: 'double', signature: 'double.min(other : double) : double', parameters: ['other : double'], documentation: 'The smaller of the two numbers.' },
  { name: 'max', receiver: 'double', signature: 'double.max(other : double) : double', parameters: ['other : double'], documentation: 'The larger of the two numbers.' },
  { name: 'isFinite', receiver: 'double', signature: 'double.isFinite() : Bool', parameters: [], documentation: 'Whether the number is neither infinite nor NaN.' },
  { name: 'first', receiver: 'Iterable[T]', signature: 'Iterable[T].first() : Option[T]', parameters: [], documentation: 'The first item, or None when there is none. An item that cannot be copied comes out as a read-only view, Option[&T].' },
  { name: 'last', receiver: 'Iterable[T]', signature: 'Iterable[T].last() : Option[T]', parameters: [], documentation: 'The last item, or None when there is none. An item that cannot be copied comes out as a read-only view, Option[&T].' },
  { name: 'min', receiver: 'Iterable[T]', signature: 'Iterable[T].min() : Option[T]', parameters: [], documentation: 'The smallest item by compare, or None when there is none. Items implement Comparable; one that cannot be copied comes out as a read-only view.' },
  { name: 'max', receiver: 'Iterable[T]', signature: 'Iterable[T].max() : Option[T]', parameters: [], documentation: 'The largest item by compare, or None when there is none. Items implement Comparable; one that cannot be copied comes out as a read-only view.' },
  { name: 'any', receiver: 'Iterable[T]', signature: 'Iterable[T].any(test : (T) -> Bool) : Bool', parameters: ['test : (T) -> Bool'], documentation: 'Whether some item passes the test. Stops at the first one that does.' },
  { name: 'all', receiver: 'Iterable[T]', signature: 'Iterable[T].all(test : (T) -> Bool) : Bool', parameters: ['test : (T) -> Bool'], documentation: 'Whether every item passes the test. Stops at the first one that does not.' },
  { name: 'isEmpty', receiver: 'Iterable[T]', signature: 'Iterable[T].isEmpty() : Bool', parameters: [], documentation: 'Whether there are no items.' },
  { name: 'isSome', receiver: 'Option[T]', signature: 'Option[T].isSome() : Bool', parameters: [], documentation: 'Whether the option holds a value.' },
  { name: 'isNone', receiver: 'Option[T]', signature: 'Option[T].isNone() : Bool', parameters: [], documentation: 'Whether the option is empty.' },
  { name: 'pop', receiver: 'Array[T]', signature: 'Array[T].pop() : Option[T]', parameters: [], documentation: 'Removes the last item and returns it, or None when the array is empty. Needs a mutable array.' },
  { name: 'swap', receiver: 'Array[T]', signature: 'Array[T].swap(i : Int, j : Int) : void', parameters: ['i : Int', 'j : Int'], documentation: 'Exchanges the items at two indexes. Needs a mutable array.' },
  { name: 'empty', static: 'Array', module: 'std.array', named: false, signature: 'Array.empty() : Array[T]', parameters: [], documentation: 'An empty array. Its element type comes from the context, as in let xs : Array[Int] = Array.empty().' },
  { name: 'Stack', module: 'prelude', named: false, signature: 'Stack[T]() : Stack[T]', parameters: [], documentation: 'An empty last-in, first-out stack. Write Stack[Int]() or let the type annotation supply T, as in let s : Stack[Int] = Stack().' },
  { name: 'Queue', module: 'prelude', named: false, signature: 'Queue[T]() : Queue[T]', parameters: [], documentation: 'An empty first-in, first-out queue. Write Queue[Int]() or let the type annotation supply T.' },
  { name: 'MinHeap', module: 'prelude', named: false, signature: 'MinHeap[T]() : MinHeap[T]', parameters: [], documentation: 'An empty heap whose pop returns the smallest item. T must be Comparable.' },
  { name: 'MaxHeap', module: 'prelude', named: false, signature: 'MaxHeap[T]() : MaxHeap[T]', parameters: [], documentation: 'An empty heap whose pop returns the largest item. T must be Comparable.' },
  { name: 'sorted', receiver: 'List[T]', signature: 'List[T].sorted() : List[T]', parameters: [], documentation: 'A new List of the items in ascending order. The items must be Comparable, and the sort is stable. The original List is unchanged.' },
  { name: 'sorted', receiver: 'Array[T]', signature: 'Array[T].sorted() : List[T]', parameters: [], documentation: 'A new List of the items in ascending order. The items must be Comparable, and the sort is stable. The Array is unchanged.' },
  { name: 'sort', receiver: 'Array[T]', signature: 'Array[T].sort() : void', parameters: [], documentation: 'Puts the items in ascending order, in place. Needs a mutable array and Comparable items. The sort is stable.' },
  { name: 'get', receiver: 'String', signature: 'String.get(index : Int) : Option[String]', parameters: ['index : Int'], documentation:'The character at a position counted in Unicode scalars, as a one-character String, or None when the position is outside the text. s[index] reads the same character and stops the program when it is outside the text.' },
  // Python-style methods (Q127, Q133 to Q136): Map default, folds, pairs, keyed sorts and String methods.
  { name: 'get', receiver: 'Map[K, V]', signature: 'Map[K, V].get(key : K, default : V) : V', parameters: ['key : K', 'default : V'], documentation: 'The value for the key, or the default when the map has no such key. Always returns a value, not an Option.' },
  { name: 'get', receiver: 'MutMap[K, V]', signature: 'MutMap[K, V].get(key : K, default : V) : V', parameters: ['key : K', 'default : V'], documentation: 'The value for the key, or the default when the map has no such key. Always returns a value, not an Option.' },
  { name: 'fold', receiver: 'Iterable[T]', signature: 'Iterable[T].fold(initial : R, step : (R, T) -> R) : R', parameters: ['initial : R', 'step : (R, T) -> R'], documentation: 'Combines the items from first to last. step receives the accumulator and one item, starting from initial, and its last result is returned.' },
  { name: 'sum', receiver: 'Iterable[T]', signature: 'Iterable[T].sum() : T', parameters: [], documentation: 'The total of the items, for Int, Nat and double items. An empty iterable gives zero.' },
  { name: 'product', receiver: 'Iterable[T]', signature: 'Iterable[T].product() : T', parameters: [], documentation: 'The product of the items, for Int, Nat and double items. An empty iterable gives one.' },
  { name: 'enumerate', receiver: 'Iterable[T]', signature: 'Iterable[T].enumerate() : List[(Int, T)]', parameters: [], documentation: 'Pairs each item with its position from zero. Eager, so the items must be copyable.' },
  { name: 'zip', receiver: 'Iterable[T]', signature: 'Iterable[T].zip(other : Iterable[B]) : List[(T, B)]', parameters: ['other : Iterable[B]'], documentation: 'Pairs items by position with the other iterable’s items, and stops at the shorter one. Eager, so the items must be copyable.' },
  { name: 'sorted', receiver: 'List[T]', signature: 'List[T].sorted(key : (T) -> K) : List[T]', parameters: ['key : (T) -> K'], documentation: 'A new List ordered by the key of each item. The key must be Comparable, and the sort is stable. The original List is unchanged.' },
  { name: 'sorted', receiver: 'Array[T]', signature: 'Array[T].sorted(key : (T) -> K) : List[T]', parameters: ['key : (T) -> K'], documentation: 'A new List ordered by the key of each item. The key must be Comparable, and the sort is stable. The Array is unchanged.' },
  { name: 'sort', receiver: 'Array[T]', signature: 'Array[T].sort(key : (T) -> K) : void', parameters: ['key : (T) -> K'], documentation: 'Puts the items in order by their key, in place. Needs a mutable array; the key must be Comparable, and the sort is stable.' },
  { name: 'isalpha', receiver: 'String', signature: 'String.isalpha() : Bool', parameters: [], documentation: 'Whether every character is a letter. An empty text is false.' },
  { name: 'isdecimal', receiver: 'String', signature: 'String.isdecimal() : Bool', parameters: [], documentation: 'Whether every character is a decimal digit. An empty text is false.' },
  { name: 'isdigit', receiver: 'String', signature: 'String.isdigit() : Bool', parameters: [], documentation: 'Whether every character is a digit, including superscript digits. An empty text is false.' },
  { name: 'isnumeric', receiver: 'String', signature: 'String.isnumeric() : Bool', parameters: [], documentation: 'Whether every character is numeric, including fractions and Roman numerals. An empty text is false.' },
  { name: 'isalnum', receiver: 'String', signature: 'String.isalnum() : Bool', parameters: [], documentation: 'Whether every character is a letter or a number. An empty text is false.' },
  { name: 'isspace', receiver: 'String', signature: 'String.isspace() : Bool', parameters: [], documentation: 'Whether every character is whitespace. An empty text is false.' },
  { name: 'isupper', receiver: 'String', signature: 'String.isupper() : Bool', parameters: [], documentation: 'Whether the text has cased characters, all in upper case.' },
  { name: 'islower', receiver: 'String', signature: 'String.islower() : Bool', parameters: [], documentation: 'Whether the text has cased characters, all in lower case.' },
  { name: 'istitle', receiver: 'String', signature: 'String.istitle() : Bool', parameters: [], documentation: 'Whether the text is title-cased: each word starts with an upper-case letter and continues in lower case.' },
  { name: 'isascii', receiver: 'String', signature: 'String.isascii() : Bool', parameters: [], documentation: 'Whether every character is ASCII. An empty text is true.' },
  { name: 'isprintable', receiver: 'String', signature: 'String.isprintable() : Bool', parameters: [], documentation: 'Whether every character is printable. Space counts as printable, and an empty text is true.' },
  { name: 'isidentifier', receiver: 'String', signature: 'String.isidentifier() : Bool', parameters: [], documentation: 'Whether the text is a name that follows the Unicode identifier rules.' },
  { name: 'capitalize', receiver: 'String', signature: 'String.capitalize() : String', parameters: [], documentation: 'The first character in title case and the rest in lower case.' },
  { name: 'title', receiver: 'String', signature: 'String.title() : String', parameters: [], documentation: 'Each word starts with a title-case character and continues in lower case.' },
  { name: 'swapcase', receiver: 'String', signature: 'String.swapcase() : String', parameters: [], documentation: 'Each upper-case character becomes lower case, and each lower-case character becomes upper case.' },
  { name: 'casefold', receiver: 'String', signature: 'String.casefold() : String', parameters: [], documentation: 'The text folded for caseless matching. It can be longer than the original, since ß folds to ss.' },
  { name: 'lower', receiver: 'String', signature: 'String.lower() : String', parameters: [], documentation: 'The text in lower case, using full Unicode case mapping.' },
  { name: 'upper', receiver: 'String', signature: 'String.upper() : String', parameters: [], documentation: 'The text in upper case, using full Unicode case mapping. For example ß becomes SS.' },
  { name: 'center', receiver: 'String', signature: 'String.center(width : Int) : String', parameters: ['width : Int'], documentation: 'Pads both sides to the width, counted in characters. The fill is one character, a space by default; text already as wide as the width is returned unchanged.' },
  { name: 'center', receiver: 'String', signature: 'String.center(width : Int, fill : String) : String', parameters: ['width : Int', 'fill : String'], documentation: 'Pads both sides to the width, counted in characters. The fill is one character, a space by default; text already as wide as the width is returned unchanged.' },
  { name: 'ljust', receiver: 'String', signature: 'String.ljust(width : Int) : String', parameters: ['width : Int'], documentation: 'Pads the right side to the width, counted in characters. The fill is one character, a space by default.' },
  { name: 'ljust', receiver: 'String', signature: 'String.ljust(width : Int, fill : String) : String', parameters: ['width : Int', 'fill : String'], documentation: 'Pads the right side to the width, counted in characters. The fill is one character, a space by default.' },
  { name: 'rjust', receiver: 'String', signature: 'String.rjust(width : Int) : String', parameters: ['width : Int'], documentation: 'Pads the left side to the width, counted in characters. The fill is one character, a space by default.' },
  { name: 'rjust', receiver: 'String', signature: 'String.rjust(width : Int, fill : String) : String', parameters: ['width : Int', 'fill : String'], documentation: 'Pads the left side to the width, counted in characters. The fill is one character, a space by default.' },
  { name: 'zfill', receiver: 'String', signature: 'String.zfill(width : Int) : String', parameters: ['width : Int'], documentation: 'Pads on the left with zeros to the width. A leading + or - stays in front of the zeros.' },
  { name: 'strip', receiver: 'String', signature: 'String.strip() : String', parameters: [], documentation: 'Removes leading and trailing whitespace.' },
  { name: 'strip', receiver: 'String', signature: 'String.strip(chars : String) : String', parameters: ['chars : String'], documentation: 'Removes leading and trailing the characters found in chars.' },
  { name: 'lstrip', receiver: 'String', signature: 'String.lstrip() : String', parameters: [], documentation: 'Removes leading whitespace.' },
  { name: 'lstrip', receiver: 'String', signature: 'String.lstrip(chars : String) : String', parameters: ['chars : String'], documentation: 'Removes leading the characters found in chars.' },
  { name: 'rstrip', receiver: 'String', signature: 'String.rstrip() : String', parameters: [], documentation: 'Removes trailing whitespace.' },
  { name: 'rstrip', receiver: 'String', signature: 'String.rstrip(chars : String) : String', parameters: ['chars : String'], documentation: 'Removes trailing the characters found in chars.' },
  { name: 'expandtabs', receiver: 'String', signature: 'String.expandtabs() : String', parameters: [], documentation: 'Replaces each tab with spaces up to the next multiple of 8 columns.' },
  { name: 'expandtabs', receiver: 'String', signature: 'String.expandtabs(tabsize : Int) : String', parameters: ['tabsize : Int'], documentation: 'Replaces each tab with spaces up to the next multiple of tabsize columns.' },
  { name: 'find', receiver: 'String', signature: 'String.find(part : String) : Int', parameters: ['part : String'], documentation: 'The position of the first match, or -1 when there is none. Positions count characters.' },
  { name: 'rfind', receiver: 'String', signature: 'String.rfind(part : String) : Int', parameters: ['part : String'], documentation: 'The position of the last match, or -1 when there is none. Positions count characters.' },
  { name: 'count', receiver: 'String', signature: 'String.count(part : String) : Int', parameters: ['part : String'], documentation: 'How many non-overlapping matches the text has.' },
  { name: 'index', receiver: 'String', signature: 'String.index(part : String) : Option[Int]', parameters: ['part : String'], documentation: 'The position of the first match, or None.' },
  { name: 'rindex', receiver: 'String', signature: 'String.rindex(part : String) : Option[Int]', parameters: ['part : String'], documentation: 'The position of the last match, or None.' },
  { name: 'replace', receiver: 'String', signature: 'String.replace(old : String, replacement : String) : String', parameters: ['old : String', 'replacement : String'], documentation: 'Replaces every non-overlapping match of old with the replacement.' },
  { name: 'partition', receiver: 'String', signature: 'String.partition(separator : String) : (String, String, String)', parameters: ['separator : String'], documentation: 'Splits at the first separator into the text before it, the separator and the text after it. Without a separator, the result is the text, then two empty strings.' },
  { name: 'rpartition', receiver: 'String', signature: 'String.rpartition(separator : String) : (String, String, String)', parameters: ['separator : String'], documentation: 'Splits at the last separator into the text before it, the separator and the text after it. Without a separator, the result is two empty strings, then the text.' },
  { name: 'splitlines', receiver: 'String', signature: 'String.splitlines() : List[String]', parameters: [], documentation: 'Splits at line breaks and drops them. Line breaks include CRLF and the Unicode line and paragraph separators.' },
  { name: 'split', receiver: 'String', signature: 'String.split() : List[String]', parameters: [], documentation: 'Splits at runs of whitespace and drops empty pieces.' },
  { name: 'removeprefix', receiver: 'String', signature: 'String.removeprefix(prefix : String) : String', parameters: ['prefix : String'], documentation: 'The text without the prefix when it starts with it; otherwise the text unchanged.' },
  { name: 'removesuffix', receiver: 'String', signature: 'String.removesuffix(suffix : String) : String', parameters: ['suffix : String'], documentation: 'The text without the suffix when it ends with it; otherwise the text unchanged.' },
  { name: 'push', receiver: 'Stack[T]', signature: 'Stack[T].push(move value : T) : void', parameters: ['value : T'], documentation: 'Puts a value on the top. Needs a mutable stack.' },
  { name: 'pop', receiver: 'Stack[T]', signature: 'Stack[T].pop() : Option[T]', parameters: [], documentation: 'Removes and returns the top value, or None when the stack is empty.' },
  { name: 'peek', receiver: 'Stack[T]', signature: 'Stack[T].peek() : Option[&T]', parameters: [], documentation: 'A read-only view of the top value, or None when the stack is empty.' },
  { name: 'size', receiver: 'Stack[T]', signature: 'Stack[T].size() : Int', parameters: [], documentation: 'The number of values in the stack.' },
  { name: 'enqueue', receiver: 'Queue[T]', signature: 'Queue[T].enqueue(move value : T) : void', parameters: ['value : T'], documentation: 'Adds a value at the back. Needs a mutable queue.' },
  { name: 'dequeue', receiver: 'Queue[T]', signature: 'Queue[T].dequeue() : Option[T]', parameters: [], documentation: 'Removes and returns the front value, or None when the queue is empty. The values behind it move one place forward. Needs a mutable queue.' },
  { name: 'first', receiver: 'Queue[T]', signature: 'Queue[T].first() : Option[&T]', parameters: [], documentation: 'A read-only view of the front value, or None when the queue is empty.' },
  { name: 'push', receiver: 'MinHeap[T]', signature: 'MinHeap[T].push(move value : T) : void', parameters: ['value : T'], documentation: 'Adds a value. Needs a mutable heap.' },
  { name: 'pop', receiver: 'MinHeap[T]', signature: 'MinHeap[T].pop() : Option[T]', parameters: [], documentation: 'Removes and returns the smallest value, or None when the heap is empty.' },
  { name: 'peek', receiver: 'MinHeap[T]', signature: 'MinHeap[T].peek() : Option[&T]', parameters: [], documentation: 'A read-only view of the smallest value, or None when the heap is empty.' },
  { name: 'size', receiver: 'MinHeap[T]', signature: 'MinHeap[T].size() : Int', parameters: [], documentation: 'The number of values in the heap.' },
  { name: 'push', receiver: 'MaxHeap[T]', signature: 'MaxHeap[T].push(move value : T) : void', parameters: ['value : T'], documentation: 'Adds a value. Needs a mutable heap.' },
  { name: 'pop', receiver: 'MaxHeap[T]', signature: 'MaxHeap[T].pop() : Option[T]', parameters: [], documentation: 'Removes and returns the largest value, or None when the heap is empty.' },
  { name: 'peek', receiver: 'MaxHeap[T]', signature: 'MaxHeap[T].peek() : Option[&T]', parameters: [], documentation: 'A read-only view of the largest value, or None when the heap is empty.' },
  { name: 'size', receiver: 'MaxHeap[T]', signature: 'MaxHeap[T].size() : Int', parameters: [], documentation: 'The number of values in the heap.' },
  { name: 'append', receiver: 'Array[T]', signature: 'Array[T].append(value : T) : void', parameters: ['value : T'], documentation: 'Adds the value in a new slot at the end, growing the array in place. Needs a mutable array; the value moves in.' },
  { name: 'length', receiver: 'MutMap[K, V]', signature: 'MutMap[K, V].length() : Int', parameters: [], documentation: 'The number of entries.' },
  { name: 'length', receiver: 'MutSet[T]', signature: 'MutSet[T].length() : Int', parameters: [], documentation: 'The number of elements.' },
  { name: 'contains', receiver: 'List[T]', signature: 'List[T].contains(value : T) : Bool', parameters: ['value : T'], documentation: 'Whether the list contains an equal value.' },
  { name: 'contains', receiver: 'Set[T]', signature: 'Set[T].contains(value : T) : Bool', parameters: ['value : T'], documentation: 'Whether the set contains an equal value.' },
  { name: 'contains', receiver: 'Map[K, V]', signature: 'Map[K, V].contains(key : K) : Bool', parameters: ['key : K'], documentation: 'Whether the map has a key.' },
  { name: 'contains', receiver: 'SortedSet[T]', signature: 'SortedSet[T].contains(value : T) : Bool', parameters: ['value : T'], documentation: 'Whether the sorted set contains an equal value.' },
  { name: 'contains', receiver: 'MutSet[T]', signature: 'MutSet[T].contains(value : T) : Bool', parameters: ['value : T'], documentation: 'Whether the mutable set contains an equal value.' },
  { name: 'get', receiver: 'Map[K, V]', signature: 'Map[K, V].get(key : K) : Option[V]', parameters: ['key : K'], documentation: 'The value for a key, or None.' },
  { name: 'get', receiver: 'Array[T]', signature: 'Array[T].get(index : Int) : Option[T]', parameters: ['index : Int'], documentation: 'The element at an index, or None.' },
  { name: 'get', receiver: 'Arena[T]', signature: 'Arena[T].get(handle : ArenaRef[T]) : Option[T]', parameters: ['handle : ArenaRef[T]'], documentation: 'The value behind an arena handle, or None.' },
  { name: 'insert', receiver: 'Arena[T]', signature: 'Arena[T].insert(value : T) : ArenaRef[T]', parameters: ['value : T'], documentation: 'Copies a value into the arena and returns its handle.' },
  { name: 'replace', receiver: 'Arena[T]', signature: 'Arena[T].replace(handle : ArenaRef[T], value : T) : void or Error', parameters: ['handle : ArenaRef[T]', 'value : T'], documentation: 'Replaces the value behind a live handle.' },
  { name: 'put', receiver: 'Map[K, V]', signature: 'Map[K, V].put(key : K, value : V) : Map[K, V]', parameters: ['key : K', 'value : V'], documentation: 'A new map with the entry added or replaced.' },
  { name: 'put', receiver: 'MutMap[K, V]', signature: 'MutMap[K, V].put(key : K, value : V) : void', parameters: ['key : K', 'value : V'], documentation: 'Changes the map in place.' },
  { name: 'update', receiver: 'MutMap[K, V]', signature: 'MutMap[K, V].update(key : K, change : (V) -> V) : Bool', parameters: ['key : K', 'change : (V) -> V'], documentation: 'Changes the stored value and returns whether the key was present. A number, String or record is replaced by the result of change; an object with var fields reaches change as a mut parameter and changes in place.' },
  { name: 'add', receiver: 'Set[T]', signature: 'Set[T].add(value : T) : Set[T]', parameters: ['value : T'], documentation: 'A new set with the value added.' },
  { name: 'add', receiver: 'SortedSet[T]', signature: 'SortedSet[T].add(value : T) : SortedSet[T]', parameters: ['value : T'], documentation: 'A new sorted set with the value added.' },
  { name: 'add', receiver: 'MutSet[T]', signature: 'MutSet[T].add(value : T) : void', parameters: ['value : T'], documentation: 'Changes the set in place.' },
  { name: 'map', receiver: 'Iterable[T]', signature: 'Iterable[T].map(f : (T) -> U) : Lazy[U]', parameters: ['f : (T) -> U'], documentation: 'A lazy sequence of mapped values.' },
  { name: 'filter', receiver: 'Iterable[T]', signature: 'Iterable[T].filter(f : (T) -> Bool) : Lazy[T]', parameters: ['f : (T) -> Bool'], documentation: 'A lazy sequence of values that pass the predicate.' },
  { name: 'toList', receiver: 'Iterable[T]', signature: 'Iterable[T].toList() : List[T]', parameters: [], documentation: 'Collects a traversal into a List.' },
  { name: 'toArray', receiver: 'Iterable[T]', signature: 'Iterable[T].toArray() : Array[T]', parameters: [], documentation: 'Collects a traversal into a new mutable Array. The element type must be copyable.' },
  { name: 'items', receiver: 'Iterable[T]', signature: 'Iterable[T].items() : Iterator[T]', parameters: [], documentation: 'A cursor over the elements.' },
  { name: 'next', receiver: 'Iterator[T]', signature: 'Iterator[T].next() : Option[T]', parameters: [], documentation: 'Advances the cursor one step.' },
  { name: 'text', receiver: 'Text', signature: 'Text.text() : String', parameters: [], documentation: 'The value’s text.' },
  { name: 'debug', receiver: 'Text', signature: 'Text.debug() : String', parameters: [], documentation: 'The value’s debugging text.' },
  { name: 'toDouble', receiver: 'Int', signature: 'Int.toDouble() : double', parameters: [], documentation: 'Converts an Int to a double.' },
  { name: 'toBigInt', receiver: 'Int', signature: 'Int.toBigInt() : BigInt', parameters: [], documentation: 'Converts an Int to a BigInt.' },
  { name: 'checkedToInt', receiver: 'double', signature: 'double.checkedToInt() : Int or Error', parameters: [], documentation: 'Converts a finite double to Int, or reports an Error.' },
  { name: 'checkedToInt', receiver: 'BigInt', signature: 'BigInt.checkedToInt() : Int or Error', parameters: [], documentation: 'Converts a BigInt to Int when it fits.' },
  { name: 'checkedToDouble', receiver: 'BigInt', signature: 'BigInt.checkedToDouble() : double or Error', parameters: [], documentation: 'Converts a BigInt to a finite double when it fits.' },
  { name: 'isFinite', receiver: 'double', signature: 'double.isFinite() : Bool', parameters: [], documentation: 'Whether the value is finite.' },
  { name: 'checkedAdd', receiver: 'Int', signature: 'Int.checkedAdd(value : Int) : Int or Error', parameters: ['value : Int'], documentation: 'Addition that reports overflow.' },
  { name: 'checkedSubtract', receiver: 'Int', signature: 'Int.checkedSubtract(value : Int) : Int or Error', parameters: ['value : Int'], documentation: 'Subtraction that reports overflow.' },
  { name: 'checkedMultiply', receiver: 'Int', signature: 'Int.checkedMultiply(value : Int) : Int or Error', parameters: ['value : Int'], documentation: 'Multiplication that reports overflow.' },
  { name: 'checkedDivide', receiver: 'Int', signature: 'Int.checkedDivide(value : Int) : Int or Error', parameters: ['value : Int'], documentation: 'Division that reports a zero divisor or overflow.' },
  { name: 'checkedAdd', receiver: 'double', signature: 'double.checkedAdd(value : double) : double or Error', parameters: ['value : double'], documentation: 'Addition that reports a non-finite result.' },
  { name: 'checkedSubtract', receiver: 'double', signature: 'double.checkedSubtract(value : double) : double or Error', parameters: ['value : double'], documentation: 'Subtraction that reports a non-finite result.' },
  { name: 'checkedMultiply', receiver: 'double', signature: 'double.checkedMultiply(value : double) : double or Error', parameters: ['value : double'], documentation: 'Multiplication that reports a non-finite result.' },
  { name: 'checkedDivide', receiver: 'double', signature: 'double.checkedDivide(value : double) : double or Error', parameters: ['value : double'], documentation: 'Division that reports a zero divisor or non-finite result.' },
  { name: 'checkedAdd', receiver: 'Nat', signature: 'Nat.checkedAdd(value : Nat) : Nat or Error', parameters: ['value : Nat'], documentation: 'Addition that reports overflow.' },
  { name: 'checkedSubtract', receiver: 'Nat', signature: 'Nat.checkedSubtract(value : Nat) : Nat or Error', parameters: ['value : Nat'], documentation: 'Subtraction that reports underflow.' },
  { name: 'checkedDivide', receiver: 'BigInt', signature: 'BigInt.checkedDivide(value : BigInt) : BigInt or Error', parameters: ['value : BigInt'], documentation: 'Division that reports a zero divisor.' },
  { name: 'isNull', receiver: 'CPointer[T]', signature: 'CPointer[T].isNull() : Bool', parameters: [], documentation: 'Whether the opaque C pointer is null.' },
  { name: 'load', receiver: 'Atomic[T]', signature: 'Atomic[T].load() : T with Effects', parameters: [], documentation: 'Reads the atomic value.' },
  { name: 'store', receiver: 'Atomic[T]', signature: 'Atomic[T].store(value : T) : void with Effects', parameters: ['value : T'], documentation: 'Writes the atomic value.' },
  { name: 'fetchAdd', receiver: 'Atomic[Int]', signature: 'Atomic[Int].fetchAdd(value : Int) : Int or Error with Effects', parameters: ['value : Int'], documentation: 'Adds atomically and reports the previous value.' },
  { name: 'compareExchange', receiver: 'Atomic[T]', signature: 'Atomic[T].compareExchange(expected : T, replacement : T) : Bool with Effects', parameters: ['expected : T', 'replacement : T'], documentation: 'Replaces the value when it still equals the expected value.' },
  { name: 'fetchAdd', receiver: 'Atomic[double]', signature: 'Atomic[double].fetchAdd(value : double) : double with Effects', parameters: ['value : double'], documentation: 'Adds atomically and returns the previous value.' },
  { name: 'send', receiver: 'Channel[T]', signature: 'Channel[T].send(value : T) : void or Error with Effects', parameters: ['value : T'], documentation: 'Sends a value to the channel.' },
  { name: 'receive', receiver: 'Channel[T]', signature: 'Channel[T].receive() : Option[T] with Effects', parameters: [], documentation: 'Receives a value, or None when the channel is closed.' },
  { name: 'close', receiver: 'Channel[T]', signature: 'Channel[T].close() : void with Effects', parameters: [], documentation: 'Closes the channel.' },
  { name: 'readLine', receiver: 'Reader', signature: 'Reader.readLine() : String or Error with Effects', parameters: [], documentation: 'Reads one line and advances the file position.' },
  { name: 'writeLine', receiver: 'Writer', signature: 'Writer.writeLine(text : String) : void or Error with Effects', parameters: ['text : String'], documentation: 'Writes a line and advances the file position.' },
  { name: 'compare', receiver: 'Comparable', signature: 'Comparable.compare(other : Self) : Int', parameters: ['other : Self'], documentation: 'A negative, zero or positive ordering result.' }
];

entries.push(...[
  {"name": "norm", "module": "std.tensor", "gated": true, "signature": "Tensor[double, shape].norm() : double", "parameters": [], "documentation": "Euclidean magnitude of all elements (Frobenius norm for matrices), computed without intermediate squaring overflow.", "receiver": "Tensor[double, shape]"},
  {"name": "norm", "module": "std.tensor", "gated": true, "signature": "DynTensor[double].norm() : double", "parameters": [], "documentation": "Euclidean magnitude of all elements (Frobenius norm for matrices), computed without intermediate squaring overflow.", "receiver": "DynTensor[double]"},
  {"name": "exp", "module": "std.math", "gated": true, "signature": "std.math.exp(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "exp", "module": "std.math", "gated": true, "signature": "std.math.exp(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "expm1", "module": "std.math", "gated": true, "signature": "std.math.expm1(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "expm1", "module": "std.math", "gated": true, "signature": "std.math.expm1(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "log", "module": "std.math", "gated": true, "signature": "std.math.log(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "log", "module": "std.math", "gated": true, "signature": "std.math.log(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "log1p", "module": "std.math", "gated": true, "signature": "std.math.log1p(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "log1p", "module": "std.math", "gated": true, "signature": "std.math.log1p(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "log2", "module": "std.math", "gated": true, "signature": "std.math.log2(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "log2", "module": "std.math", "gated": true, "signature": "std.math.log2(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "log10", "module": "std.math", "gated": true, "signature": "std.math.log10(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "log10", "module": "std.math", "gated": true, "signature": "std.math.log10(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "sin", "module": "std.math", "gated": true, "signature": "std.math.sin(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "sin", "module": "std.math", "gated": true, "signature": "std.math.sin(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "cos", "module": "std.math", "gated": true, "signature": "std.math.cos(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "cos", "module": "std.math", "gated": true, "signature": "std.math.cos(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "tan", "module": "std.math", "gated": true, "signature": "std.math.tan(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "tan", "module": "std.math", "gated": true, "signature": "std.math.tan(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "asin", "module": "std.math", "gated": true, "signature": "std.math.asin(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "asin", "module": "std.math", "gated": true, "signature": "std.math.asin(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "acos", "module": "std.math", "gated": true, "signature": "std.math.acos(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "acos", "module": "std.math", "gated": true, "signature": "std.math.acos(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "atan", "module": "std.math", "gated": true, "signature": "std.math.atan(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "atan", "module": "std.math", "gated": true, "signature": "std.math.atan(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "sinh", "module": "std.math", "gated": true, "signature": "std.math.sinh(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "sinh", "module": "std.math", "gated": true, "signature": "std.math.sinh(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "cosh", "module": "std.math", "gated": true, "signature": "std.math.cosh(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "cosh", "module": "std.math", "gated": true, "signature": "std.math.cosh(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "tanh", "module": "std.math", "gated": true, "signature": "std.math.tanh(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "tanh", "module": "std.math", "gated": true, "signature": "std.math.tanh(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "sqrt", "module": "std.math", "gated": true, "signature": "std.math.sqrt(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "sqrt", "module": "std.math", "gated": true, "signature": "std.math.sqrt(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "abs", "module": "std.math", "gated": true, "signature": "std.math.abs(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "abs", "module": "std.math", "gated": true, "signature": "std.math.abs(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "floor", "module": "std.math", "gated": true, "signature": "std.math.floor(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "floor", "module": "std.math", "gated": true, "signature": "std.math.floor(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "ceil", "module": "std.math", "gated": true, "signature": "std.math.ceil(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "ceil", "module": "std.math", "gated": true, "signature": "std.math.ceil(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "round", "module": "std.math", "gated": true, "signature": "std.math.round(value : double) : double", "parameters": ["value : double"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "round", "module": "std.math", "gated": true, "signature": "std.math.round(value : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["value : Tensor[double, shape]"], "documentation": "Applies the scalar mathematical function; a tensor result owns independent storage and preserves its shape."},
  {"name": "dot", "module": "std.linalg", "gated": true, "signature": "std.linalg.dot(a : Tensor[double, shape], b : Tensor[double, shape]) : double", "parameters": ["a : Tensor[double, shape]", "b : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "outer", "module": "std.linalg", "gated": true, "signature": "std.linalg.outer(a : Tensor[double, shape], b : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["a : Tensor[double, shape]", "b : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "norm", "module": "std.linalg", "gated": true, "signature": "std.linalg.norm(a : Tensor[double, shape]) : double", "parameters": ["a : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "frobeniusNorm", "module": "std.linalg", "gated": true, "signature": "std.linalg.frobeniusNorm(a : Tensor[double, shape]) : double", "parameters": ["a : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "spectralNorm", "module": "std.linalg", "gated": true, "signature": "std.linalg.spectralNorm(a : Tensor[double, shape]) : double", "parameters": ["a : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "solve", "module": "std.linalg", "gated": true, "signature": "std.linalg.solve(a : Tensor[double, shape], b : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["a : Tensor[double, shape]", "b : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "solveLower", "module": "std.linalg", "gated": true, "signature": "std.linalg.solveLower(a : Tensor[double, shape], b : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["a : Tensor[double, shape]", "b : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "solveUpper", "module": "std.linalg", "gated": true, "signature": "std.linalg.solveUpper(a : Tensor[double, shape], b : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["a : Tensor[double, shape]", "b : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "lu", "module": "std.linalg", "gated": true, "signature": "std.linalg.lu(a : Tensor[double, shape]) : LU[F, P]", "parameters": ["a : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "qr", "module": "std.linalg", "gated": true, "signature": "std.linalg.qr(a : Tensor[double, shape]) : QR[Q, R]", "parameters": ["a : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "cholesky", "module": "std.linalg", "gated": true, "signature": "std.linalg.cholesky(a : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["a : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "svd", "module": "std.linalg", "gated": true, "signature": "std.linalg.svd(a : Tensor[double, shape]) : SVD[U, S, V]", "parameters": ["a : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "pinv", "module": "std.linalg", "gated": true, "signature": "std.linalg.pinv(a : Tensor[double, shape], rcond : double = automatic) : Tensor[double, shape]", "parameters": ["a : Tensor[double, shape]", "rcond : double = automatic"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "rank", "module": "std.linalg", "gated": true, "signature": "std.linalg.rank(a : Tensor[double, shape], rcond : double = automatic) : Int", "parameters": ["a : Tensor[double, shape]", "rcond : double = automatic"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "det", "module": "std.linalg", "gated": true, "signature": "std.linalg.det(a : Tensor[double, shape]) : double", "parameters": ["a : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "inverse", "module": "std.linalg", "gated": true, "signature": "std.linalg.inverse(a : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["a : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "eigh", "module": "std.linalg", "gated": true, "signature": "std.linalg.eigh(a : Tensor[double, shape]) : Eigh[W, V]", "parameters": ["a : Tensor[double, shape]"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "leastSquares", "module": "std.linalg", "gated": true, "signature": "std.linalg.leastSquares(a : Tensor[double, shape], b : Tensor[double, shape], rcond : double = automatic) : Tensor[double, shape]", "parameters": ["a : Tensor[double, shape]", "b : Tensor[double, shape]", "rcond : double = automatic"], "documentation": "Native double linear algebra. Known shape errors are checked at compile time; invalid runtime shapes and numerical failures stop with a message. QR/SVD are reduced; symmetric eigendecomposition has ascending eigenvalues."},
  {"name": "mean", "module": "std.stats", "gated": true, "signature": "std.stats.mean(source : Tensor[double, shape], axis : Int = all) : double or Tensor[double, shape]", "parameters": ["source : Tensor[double, shape]", "axis : Int = all"], "documentation": "Population statistics by default. Rows are observations for covariance/correlation. Invalid axes, ddof and undefined correlations stop with a message."},
  {"name": "variance", "module": "std.stats", "gated": true, "signature": "std.stats.variance(source : Tensor[double, shape], axis : Int = all, ddof : Int = 0) : double or Tensor[double, shape]", "parameters": ["source : Tensor[double, shape]", "axis : Int = all", "ddof : Int = 0"], "documentation": "Population statistics by default. Rows are observations for covariance/correlation. Invalid axes, ddof and undefined correlations stop with a message."},
  {"name": "stddev", "module": "std.stats", "gated": true, "signature": "std.stats.stddev(source : Tensor[double, shape], axis : Int = all, ddof : Int = 0) : double or Tensor[double, shape]", "parameters": ["source : Tensor[double, shape]", "axis : Int = all", "ddof : Int = 0"], "documentation": "Population statistics by default. Rows are observations for covariance/correlation. Invalid axes, ddof and undefined correlations stop with a message."},
  {"name": "covariance", "module": "std.stats", "gated": true, "signature": "std.stats.covariance(source : Tensor[double, shape], ddof : Int = 0) : Tensor[double, shape]", "parameters": ["source : Tensor[double, shape]", "ddof : Int = 0"], "documentation": "Population statistics by default. Rows are observations for covariance/correlation. Invalid axes, ddof and undefined correlations stop with a message."},
  {"name": "correlation", "module": "std.stats", "gated": true, "signature": "std.stats.correlation(source : Tensor[double, shape], ddof : Int = 0) : Tensor[double, shape]", "parameters": ["source : Tensor[double, shape]", "ddof : Int = 0"], "documentation": "Population statistics by default. Rows are observations for covariance/correlation. Invalid axes, ddof and undefined correlations stop with a message."},
  {"name": "factors", "module": "std.linalg", "gated": true, "receiver": "LU[F, P]", "signature": "LU.factors : F", "parameters": [], "field": true, "documentation": "Owned factorization component; reading the field borrows its tensor."},
  {"name": "pivots", "module": "std.linalg", "gated": true, "receiver": "LU[F, P]", "signature": "LU.pivots : P", "parameters": [], "field": true, "documentation": "Owned factorization component; reading the field borrows its tensor."},
  {"name": "q", "module": "std.linalg", "gated": true, "receiver": "QR[Q, R]", "signature": "QR.q : Q", "parameters": [], "field": true, "documentation": "Owned factorization component; reading the field borrows its tensor."},
  {"name": "r", "module": "std.linalg", "gated": true, "receiver": "QR[Q, R]", "signature": "QR.r : R", "parameters": [], "field": true, "documentation": "Owned factorization component; reading the field borrows its tensor."},
  {"name": "u", "module": "std.linalg", "gated": true, "receiver": "SVD[U, S, V]", "signature": "SVD.u : U", "parameters": [], "field": true, "documentation": "Owned factorization component; reading the field borrows its tensor."},
  {"name": "s", "module": "std.linalg", "gated": true, "receiver": "SVD[U, S, V]", "signature": "SVD.s : S", "parameters": [], "field": true, "documentation": "Owned factorization component; reading the field borrows its tensor."},
  {"name": "vt", "module": "std.linalg", "gated": true, "receiver": "SVD[U, S, V]", "signature": "SVD.vt : V", "parameters": [], "field": true, "documentation": "Owned factorization component; reading the field borrows its tensor."},
  {"name": "values", "module": "std.linalg", "gated": true, "receiver": "Eigh[W, V]", "signature": "Eigh.values : W", "parameters": [], "field": true, "documentation": "Owned factorization component; reading the field borrows its tensor."},
  {"name": "vectors", "module": "std.linalg", "gated": true, "receiver": "Eigh[W, V]", "signature": "Eigh.vectors : V", "parameters": [], "field": true, "documentation": "Owned factorization component; reading the field borrows its tensor."},
  {"name": "solve", "module": "std.linalg", "gated": true, "signature": "LU[F, P].solve(b : Tensor[double, shape]) : Tensor[double, shape]", "parameters": ["b : Tensor[double, shape]"], "documentation": "Reuses this LU factorization to solve for a vector or matrix right-hand side.", "receiver": "LU[F, P]"},
  {"name": "Rng", "module": "std.random", "gated": true, "signature": "Rng", "parameters": [], "documentation": "Explicitly imported numerical type."},
  {"name": "seed", "module": "std.random", "gated": true, "signature": "seed(seed : Int) : void with Effects", "parameters": ["seed : Int"], "documentation": "Seeds the thread-local default generator used by the no-rng static samplers."},
  {"name": "seeded", "module": "std.random", "gated": true, "signature": "Rng.seeded(seed : Int) : Rng", "parameters": ["seed : Int"], "documentation": "Creates an owned PCG32 generator. Same seed and call sequence reproduce draws on the same platform.", "static": "Rng"},
  {"name": "uniform", "module": "std.random", "gated": true, "signature": "Rng.uniform(low : double, high : double) : double", "parameters": ["low : double", "high : double"], "documentation": "Advances this generator. Integer and uniform ranges include low and exclude high. Invalid parameters stop with a message.", "receiver": "Rng"},
  {"name": "normal", "module": "std.random", "gated": true, "signature": "Rng.normal(mean : double, sigma : double) : double", "parameters": ["mean : double", "sigma : double"], "documentation": "Advances this generator. Integer and uniform ranges include low and exclude high. Invalid parameters stop with a message.", "receiver": "Rng"},
  {"name": "integers", "module": "std.random", "gated": true, "signature": "Rng.integers(low : Int, high : Int) : Int", "parameters": ["low : Int", "high : Int"], "documentation": "Advances this generator. Integer and uniform ranges include low and exclude high. Invalid parameters stop with a message.", "receiver": "Rng"},
]);

entries.push(...[{"name": "LU", "module": "std.linalg", "gated": true, "type": true, "signature": "LU[F, P]", "parameters": [], "documentation": "Native factorization result with typed tensor components."}, {"name": "QR", "module": "std.linalg", "gated": true, "type": true, "signature": "QR[Q, R]", "parameters": [], "documentation": "Native factorization result with typed tensor components."}, {"name": "SVD", "module": "std.linalg", "gated": true, "type": true, "signature": "SVD[U, S, V]", "parameters": [], "documentation": "Native factorization result with typed tensor components."}, {"name": "Eigh", "module": "std.linalg", "gated": true, "type": true, "signature": "Eigh[W, V]", "parameters": [], "documentation": "Native factorization result with typed tensor components."}]);

entries.push(...[
  {"name": "Complex", "module": "std.complex", "gated": true, "type": true, "signature": "Complex", "parameters": [], "documentation": "An immutable double-precision complex number with re and im components."},
  {"name": "Complex", "module": "std.complex", "gated": true, "named": true, "signature": "Complex(re : double, im : double) : Complex", "parameters": ["re : double", "im : double"], "documentation": "Builds a complex number from its real and imaginary components."},
  {"name": "zero", "module": "std.complex", "gated": true, "static": "Complex", "signature": "Complex.zero() : Complex", "parameters": [], "documentation": "Returns 0.0+0.0i."},
  {"name": "one", "module": "std.complex", "gated": true, "static": "Complex", "signature": "Complex.one() : Complex", "parameters": [], "documentation": "Returns 1.0+0.0i."},
  {"name": "i", "module": "std.complex", "gated": true, "static": "Complex", "signature": "Complex.i() : Complex", "parameters": [], "documentation": "Returns the imaginary unit 0.0+1.0i."},
  {"name": "re", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.re : double", "parameters": [], "field": true, "documentation": "The real component."},
  {"name": "im", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.im : double", "parameters": [], "field": true, "documentation": "The imaginary component."},
  {"name": "add", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.add(other : Complex) : Complex", "parameters": ["other : Complex"], "documentation": "Adds another complex number."},
  {"name": "subtract", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.subtract(other : Complex) : Complex", "parameters": ["other : Complex"], "documentation": "Subtracts another complex number."},
  {"name": "multiply", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.multiply(other : Complex) : Complex", "parameters": ["other : Complex"], "documentation": "Multiplies by another complex number."},
  {"name": "divide", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.divide(other : Complex) : Complex", "parameters": ["other : Complex"], "documentation": "Divides by another complex number."},
  {"name": "rightAdd", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.rightAdd(left : double) : Complex", "parameters": ["left : double"], "documentation": "Adds this complex number to a double on the left."},
  {"name": "rightSubtract", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.rightSubtract(left : double) : Complex", "parameters": ["left : double"], "documentation": "Subtracts this complex number from a double on the left."},
  {"name": "rightMultiply", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.rightMultiply(left : double) : Complex", "parameters": ["left : double"], "documentation": "Multiplies a double on the left by this complex number."},
  {"name": "rightDivide", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.rightDivide(left : double) : Complex", "parameters": ["left : double"], "documentation": "Divides a double on the left by this complex number."},
  {"name": "conjugate", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.conjugate() : Complex", "parameters": [], "documentation": "Returns the complex conjugate."},
  {"name": "normSquared", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.normSquared() : double", "parameters": [], "documentation": "Returns re squared plus im squared, without a square root."},
  {"name": "abs", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.abs() : double", "parameters": [], "documentation": "Returns the magnitude with overflow-safe scaling."},
  {"name": "arg", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.arg() : double", "parameters": [], "documentation": "Returns the angle in radians, from -pi to pi."},
  {"name": "isFinite", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.isFinite() : Bool", "parameters": [], "documentation": "Whether both components are finite."},
  {"name": "copy", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.copy() : Complex", "parameters": [], "documentation": "Returns an independent copy."},
  {"name": "negate", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.negate() : Complex", "parameters": [], "documentation": "Returns the additive inverse."},
  {"name": "power", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.power(exponent : Int) : Complex", "parameters": ["exponent : Int"], "documentation": "Raises this complex number to an integer power, including negative powers."},
  {"name": "FittedStandardScaler", "module": "std.ml", "gated": true, "type": true, "signature": "FittedStandardScaler", "parameters": [], "documentation": "A fitted standard scaler with per-feature means and scales."},
  {"name": "StandardScaler", "module": "std.ml", "gated": true, "type": true, "signature": "StandardScaler", "parameters": [], "documentation": "Centers each feature and divides by its population standard deviation."},
  {"name": "FittedLinearRegression", "module": "std.ml", "gated": true, "type": true, "signature": "FittedLinearRegression", "parameters": [], "documentation": "A fitted ordinary least-squares linear regression."},
  {"name": "LinearRegression", "module": "std.ml", "gated": true, "type": true, "signature": "LinearRegression", "parameters": [], "documentation": "Ordinary least-squares regression with an optional intercept."},
  {"name": "fit", "module": "std.ml", "gated": true, "static": "StandardScaler", "signature": "StandardScaler.fit(X : DynTensor[double]) : FittedStandardScaler", "parameters": ["X : DynTensor[double]"], "documentation": "Computes per-feature means and population standard deviations."},
  {"name": "fitTransform", "module": "std.ml", "gated": true, "static": "StandardScaler", "signature": "StandardScaler.fitTransform(X : DynTensor[double]) : DynTensor[double]", "parameters": ["X : DynTensor[double]"], "documentation": "Fits the scaler and transforms the same features."},
  {"name": "fit", "module": "std.ml", "gated": true, "static": "LinearRegression", "signature": "LinearRegression.fit(X : DynTensor[double], y : DynTensor[double], fitIntercept : Bool = true) : FittedLinearRegression", "parameters": ["X : DynTensor[double]", "y : DynTensor[double]", "fitIntercept : Bool = true"], "documentation": "Fits ordinary least squares, centered for an intercept by default."},
  {"name": "means", "module": "std.ml", "gated": true, "receiver": "FittedStandardScaler", "field": true, "signature": "FittedStandardScaler.means : DynTensor[double]", "parameters": [], "documentation": "Per-feature means."},
  {"name": "scales", "module": "std.ml", "gated": true, "receiver": "FittedStandardScaler", "field": true, "signature": "FittedStandardScaler.scales : DynTensor[double]", "parameters": [], "documentation": "Per-feature scales; a zero-variance feature uses 1.0."},
  {"name": "transform", "module": "std.ml", "gated": true, "receiver": "FittedStandardScaler", "signature": "FittedStandardScaler.transform(X : DynTensor[double]) : DynTensor[double]", "parameters": ["X : DynTensor[double]"], "documentation": "Centers and scales new rows with the fitted parameters."},
  {"name": "coefficients", "module": "std.ml", "gated": true, "receiver": "FittedLinearRegression", "field": true, "signature": "FittedLinearRegression.coefficients : DynTensor[double]", "parameters": [], "documentation": "One coefficient per feature."},
  {"name": "intercept", "module": "std.ml", "gated": true, "receiver": "FittedLinearRegression", "field": true, "signature": "FittedLinearRegression.intercept : double", "parameters": [], "documentation": "The fitted intercept, or 0.0 when fitIntercept is false."},
  {"name": "predict", "module": "std.ml", "gated": true, "receiver": "FittedLinearRegression", "signature": "FittedLinearRegression.predict(X : DynTensor[double]) : DynTensor[double]", "parameters": ["X : DynTensor[double]"], "documentation": "Predicts one value per row."},
  {"name": "meanSquaredError", "module": "std.ml", "gated": true, "signature": "meanSquaredError(actual : DynTensor[double], predicted : DynTensor[double]) : double", "parameters": ["actual : DynTensor[double]", "predicted : DynTensor[double]"], "documentation": "Mean of the squared residuals."},
  {"name": "meanAbsoluteError", "module": "std.ml", "gated": true, "signature": "meanAbsoluteError(actual : DynTensor[double], predicted : DynTensor[double]) : double", "parameters": ["actual : DynTensor[double]", "predicted : DynTensor[double]"], "documentation": "Mean of the absolute residuals."},
  {"name": "r2Score", "module": "std.ml", "gated": true, "signature": "r2Score(actual : DynTensor[double], predicted : DynTensor[double]) : double", "parameters": ["actual : DynTensor[double]", "predicted : DynTensor[double]"], "documentation": "Coefficient of determination. A constant target gives 1.0 for a perfect fit and 0.0 otherwise."},
  {"name": "accuracy", "module": "std.ml", "gated": true, "signature": "accuracy(actual : DynTensor[double], predicted : DynTensor[double]) : double", "parameters": ["actual : DynTensor[double]", "predicted : DynTensor[double]"], "documentation": "Fraction of equal labels. Labels are integer-valued doubles."},
  {"name": "equals", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.equals(other : Complex) : Bool", "parameters": ["other : Complex"], "documentation": "Compares both components exactly."},
  {"name": "text", "module": "std.complex", "gated": true, "receiver": "Complex", "signature": "Complex.text() : String", "parameters": [], "documentation": "Writes the value as a+bi."}
]);

for (const distribution of distributions.list) {
  const name = distribution.name;
  const finite = distribution.params.map(([parameter]) => parameter).join(', ');
  const parameterNames = distribution.params.map(([parameter]) => parameter);
  const parameterTypes = distribution.params.map(([, type]) => type);
  entries.push({
    name,
    module: 'std.prob',
    gated: true,
    type: true,
    signature: name,
    parameters: [],
    documentation: 'Imported probability distribution. Use create(...) for an object or the static methods directly.'
  });
  entries.push({
    name: 'create',
    module: 'std.prob',
    gated: true,
    static: name,
    signature: `${name}.create(${distributions.parameterList(distribution)}) : ${name}`,
    parameters: distribution.params.map(([parameter, type]) => `${parameter} : ${type}`),
    documentation: 'Validates the distribution parameters and returns a reusable distribution object.'
  });
  for (const [parameter, type] of distribution.params) {
    entries.push({
      name: parameter,
      module: 'std.prob',
      gated: true,
      receiver: name,
      field: true,
      signature: `${name}.${parameter} : ${type}`,
      parameters: [],
      documentation: 'A distribution parameter.'
    });
  }
  const observation = distribution.discrete
    ? ['k', 'Int']
    : ['x', 'double'];
  const probability = distributions.probabilityName(distribution);
  const logProbability = distributions.logProbabilityName(distribution);
  for (const [operation, result] of [[probability, 'double'], [logProbability, 'double'], ['cdf', 'double']]) {
    entries.push({
      name: operation,
      module: 'std.prob',
      gated: true,
      receiver: name,
      signature: `${name}.${operation}(${observation[0]} : ${observation[1]}) : ${result}`,
      parameters: [`${observation[0]} : ${observation[1]}`],
      documentation: 'Evaluates this distribution without changing generator state.'
    });
  }
  entries.push({
    name: 'sample',
    module: 'std.prob',
    gated: true,
    receiver: name,
    signature: `${name}.sample(mut rng : Rng) : ${distribution.discrete ? 'Int' : 'double'}`,
    parameters: ['rng : Rng'],
    documentation: 'Draws one sample and advances the explicitly supplied generator.'
  });
  for (const [operation, result] of [[probability, 'double'], [logProbability, 'double'], ['cdf', 'double']]) {
    entries.push({
      name: operation,
      module: 'std.prob',
      gated: true,
      static: name,
      signature: `${name}.${operation}(${observation[0]} : ${observation[1]}, ${distributions.parameterList(distribution)}) : ${result}`,
      parameters: [`${observation[0]} : ${observation[1]}`, ...distribution.params.map(([parameter, type]) => `${parameter} : ${type}`)],
      documentation: 'Evaluates the distribution directly, without creating an object.'
    });
  }
  entries.push({
    name: 'sample',
    module: 'std.prob',
    gated: true,
    static: name,
    signature: `${name}.sample(${distributions.parameterList(distribution)}, shape : Int or Tuple or Array[Int] = none) : ${distribution.discrete ? 'Int or Tensor[double, shape]' : 'double or Tensor[double, shape]'}`,
    parameters: [...distribution.params.map(([parameter, type]) => `${parameter} : ${type}`), 'shape : Int or Tuple or Array[Int] = none'],
    documentation: 'Draws one value, or a tensor when shape is supplied. Without an rng this uses the thread-local default generator and requires Effects.'
  });
  entries.push({
    name: 'sample',
    module: 'std.prob',
    gated: true,
    static: name,
    signature: `${name}.sample(mut rng : Rng, ${distributions.parameterList(distribution)}, shape : Int or Tuple or Array[Int] = none) : ${distribution.discrete ? 'Int or Tensor[double, shape]' : 'double or Tensor[double, shape]'}`,
    parameters: ['rng : Rng', ...distribution.params.map(([parameter, type]) => `${parameter} : ${type}`), 'shape : Int or Tuple or Array[Int] = none'],
    documentation: 'Draws one value, or a tensor when shape is supplied, from an explicit generator.'
  });
}

entries.push(...kesdas.entries);
entries.push(...ml.entries);
entries.push(...variable.entries);
entries.push(...kesnet.entries);
entries.push(...kesnetOptim.entries);
entries.push(...kesnetData.entries);
const handwrittenEntries = entries.slice();
for (const entry of require('./library_metadata.json').entries) {
  if (!handwrittenEntries.some(old => old.name === entry.name && old.receiver === entry.receiver && old.static === entry.static && old.module === entry.module)) entries.push(entry);
}


// These tables are read for every suggestion and hover. Index them once at
// load so a lookup does not scan a thousand standard-library entries.
const byName = new Map();
for (const entry of entries) {
  const found = byName.get(entry.name);
  if (found) found.push(entry);
  else byName.set(entry.name, [entry]);
}
const methodEntries = entries.filter(entry => entry.receiver);
const functionEntries = entries.filter(entry => !entry.receiver && !entry.static);
const byStatic = new Map();
const byModule = new Map();
for (const entry of entries) {
  if (entry.static) {
    const found = byStatic.get(entry.static);
    if (found) found.push(entry);
    else byStatic.set(entry.static, [entry]);
  }
  if (entry.module) {
    const found = byModule.get(entry.module);
    if (found) found.push(entry);
    else byModule.set(entry.module, [entry]);
  }
}

function named(name) {
  return byName.get(name) || [];
}

// Static members such as Array.generate are reached through their type name, so
// they are not offered as free functions.
function available(entry, model) {
  if (!entry.gated) return true;
  const imports = model?.imports || [];
  if (imports.some(imp => imp.module === entry.module && imp.names.includes(entry.static || entry.name))) {
    return true;
  }
  // A whole-module import also binds the module's type names, so a type, its
  // constructor and its static members are usable without the namespace alias.
  // Only names that have a type entry qualify; `named` alone would also match a
  // free function with named parameters.
  const declaredType = name => (byName.get(name) || []).some(candidate =>
    candidate.type && candidate.module === entry.module);
  const typeName = entry.static || (declaredType(entry.name) ? entry.name : '');
  return Boolean(typeName) && imports.some(imp =>
    imp.module === entry.module && modules.names(imp.module).includes(typeName));
}
function moduleForReceiver(model, receiver) { return (model?.imports || []).find(imp => imp.alias === receiver)?.module || ''; }
function moduleForName(model, name) { return (model?.imports || []).find(imp => imp.names.includes(name))?.module || ''; }
function staticAvailable(entry, model, receiver) {
  const qualified = (receiver || '').split('.');
  return available(entry, model) || qualified.length === 2 && qualified[1] === entry.static && moduleForReceiver(model, qualified[0]) === entry.module;
}

function functions(model) {
  return functionEntries.filter(entry => available(entry, model));
}

function methods() {
  return methodEntries;
}

// Every entry exported by a standard module, for `module.` namespace completion.
function moduleEntries(module) {
  return byModule.get(module) || [];
}

// The static members of a type name, such as Tensor.zeros or Array.generate.
function statics(typeName) {
  return byStatic.get(typeName) || [];
}

function hover(name) {
  return named(name).map(entry => entry.signature);
}

module.exports = { entries, named, functions, methods, moduleEntries, statics, hover, available, moduleForReceiver, moduleForName, staticAvailable };
