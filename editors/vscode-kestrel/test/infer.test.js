'use strict';

const assert = require('assert');
const kestrel = require('../out/kestrel');
const infer = require('../out/infer');

const text = `class Point { new(label : String) {} }
let list := [1, 2, 3]
let name := "Kestrel"
let number := 1.5
let point := Point("p")
print(list)
`;
const model = kestrel.parseDocument(text);
const bindings = infer.inferredBindings(text, model);
const types = Object.fromEntries(bindings.map(binding => [binding.name, binding.type]));
assert.strictEqual(types.list, 'List[Int]');
assert.strictEqual(types.name, 'String');
assert.strictEqual(types.number, 'double');
assert.strictEqual(types.point, 'Point');

const locals = infer.localTypes(text, text.indexOf('print'), model);
assert.strictEqual(locals.get('list').type, 'List[Int]');
assert.strictEqual(locals.get('list').kind, 'binding');

const annotated = infer.inferredBindings('let list : List[Int] = [1]\n', kestrel.parseDocument('let list : List[Int] = [1]\n'));
assert.strictEqual(annotated[0].annotation, 'List[Int]');
assert.strictEqual(annotated[0].type, 'List[Int]');

const map = infer.inferredBindings('let table := Map(1 => "a", 2 => "b")\n', kestrel.parseDocument('let table := Map(1 => "a", 2 => "b")\n'));
assert.strictEqual(map[0].type, 'Map[Int, String]');

const generated = infer.inferredBindings('let array := Array.generate(3, n => n + 1)\n', kestrel.parseDocument('let array := Array.generate(3, n => n + 1)\n'));
assert.strictEqual(generated[0].type, 'Array[Int]');

const ranged = infer.inferredBindings('let values := range(10)\n', kestrel.parseDocument('let values := range(10)\n'));
assert.strictEqual(ranged[0].type, 'Lazy[Int]');

// The factory's lambda may annotate its Int index, and arithmetic keeps the operand type.
const typedGenerate = 'let array := Array.generate(10, x : Int => x*2)\n';
assert.strictEqual(infer.inferredBindings(typedGenerate, kestrel.parseDocument(typedGenerate))[0].type, 'Array[Int]');
const untypedGenerate = 'let array := Array.generate(10, x => x * 2)\n';
assert.strictEqual(infer.inferredBindings(untypedGenerate, kestrel.parseDocument(untypedGenerate))[0].type, 'Array[Int]');

// The type hint goes after the name, not inside the keyword: `let e` must not place the hint after `le`.
const hinted = 'let e := Val(3)\n';
const [hintedBinding] = infer.inferredBindings(hinted, kestrel.parseDocument(hinted));
assert.strictEqual(hinted.slice(hintedBinding.nameStart, hintedBinding.nameEnd), 'e');
const loop = 'for (f in xs) {}\n';
const [loopBinding] = infer.inferredBindings(loop, kestrel.parseDocument(loop));
assert.strictEqual(loop.slice(loopBinding.nameStart, loopBinding.nameEnd), 'f');

// Tensor types: literal dimensions give a static shape, and run-time dimensions give DynTensor.
const tensors = [
  ['let a := Tensor.generate((5, 6), (i, j) => (i * 6 + j).toDouble())\n', 'Tensor[double, (5, 6)]'],
  ['let z : Tensor[double, (2, 3)] = Tensor.zeros((2, 3))\nlet o := Tensor.onesLike(z)\n', 'Tensor[double, (2, 3)]'],
  ['let o := Tensor.ones((3,))\n', 'Tensor[double, (3,)]'],
  ['let m := Tensor.zeros((5, 6))\nlet c := m.copy()\n', 'Tensor[double, (5, 6)]'],
  ['let m := Tensor.zeros((5, 6))\nlet b := m.broadcastTo((4, 2, 3))\n', ''],
  ['let m := Tensor.zeros((5, 6))\nlet b := m.broadcastTo(t)\n', 'DynTensor[double]'],
  ['let n := 3\nlet d := Tensor.zeros((n, 2))\n', 'DynTensor[double]'],
  ['let xs := Array.generate(3, i => i + 1)\nlet f := Tensor.fromArray(xs)\n', 'DynTensor[double]'],
  ['let a := Tensor.zeros((2, 3))\nlet b := Tensor.zeros((3, 4))\nlet c := a @ b\n', 'Tensor[double, (2, 4)]'],
  ['let a := Tensor.zeros((2, 3))\nlet c := a * a\nlet d := 2.0 * a\n', 'Tensor[double, (2, 3)]'],
  ['let a := Tensor.zeros((2, 3))\nlet b := Tensor.zeros((2, 3))\nlet c := a @ b\n', '']
];
for (const [source, expected] of tensors) {
  const last = infer.inferredBindings(source, kestrel.parseDocument(source)).at(-1);
  assert.strictEqual(last.type, expected, `${JSON.stringify(source)} should infer ${expected}`);
}

// transpose() reverses the axes and transpose((1, 0, 2)) permutes them; dynamic shapes give DynTensor.
const transposed = [
  ['let c := Tensor.zeros((2, 3, 4))\nlet t := c.transpose()\n', 'Tensor[double, (4, 3, 2)]'],
  ['let c := Tensor.zeros((2, 3, 4))\nlet t := c.transpose((1, 0, 2))\n', 'Tensor[double, (3, 2, 4)]'],
  ['let c := Tensor.zeros((2, 3))\nlet t := c.transpose((0, 0))\n', ''],
  ['let n := 2\nlet d := Tensor.zeros((n, 3))\nlet t := d.transpose()\n', 'DynTensor[double]']
];
for (const [source, expected] of transposed) {
  const last = infer.inferredBindings(source, kestrel.parseDocument(source)).at(-1);
  assert.strictEqual(last.type, expected, `${JSON.stringify(source)} should infer ${expected}`);
}

console.log('kestrel inference tests pass');

const phaseTwoTensors = [
  ['let a := Tensor.ones((2, 3))\nlet b := a + 2.0\n', 'Tensor[double, (2, 3)]'],
  ['let a := Tensor.ones((2, 3))\nlet b := 2.0 - a\n', 'Tensor[double, (2, 3)]'],
  ['let n := 2\nlet a := Tensor.ones((n, 3))\nlet b := a + a\n', 'DynTensor[double]'],
  ['let n := 2\nlet a := Tensor.ones((n, 3))\nlet b := 2.0 * a\n', 'DynTensor[double]'],
  ['let a := Tensor.ones((2, 3))\nlet b := a + Tensor.ones((3, 2))\n', ''],
  ['let a := Tensor.ones((2, 3))\nlet b := a - 1\n', 'Tensor[double, (2, 3)]'],
  ['let a := Tensor.ones((2, 3))\nlet b := a.rank()\n', 'Int'],
  ['let a := Tensor.ones((2, 3))\nlet b := a.shape()\n', 'Array[Int]'],
  ['let n := 2\nlet a := Tensor.ones((n, 3))\nlet b := a.count()\n', 'Int'],
  ['let a := Tensor.ones((2, 3))\nlet b := a.debug()\n', 'String'],
  ['let a := Tensor.ones((2, 3))\nlet b := a.map(x => x * 0.5)\n', 'Tensor[double, (2, 3)]'],
  ['let n := 2\nlet a := Tensor.ones((n, 3))\nlet b := a.map(x => x + 1.0)\n', 'DynTensor[double]'],
  ['let a := Tensor.ones((2, 3))\nlet b := a.map(x => 1)\n', 'Tensor[double, (2, 3)]'],
  ['let a := Tensor.ones((2, 3))\nlet b := a.map(x => 1 + 1)\n', 'Tensor[double, (2, 3)]'],
  ['let a := Tensor.ones((2, 3))\nlet b := a.reshape((3, 2))\n', 'Tensor[double, (3, 2)]'],
  ['let a := Tensor.ones((2, 3))\nlet n := 3\nlet b := a.reshape((n, 2))\n', 'DynTensor[double]'],
  ['let a := Tensor.ones((2, 3))\nlet b := a.reshape((3, 3))\n', ''],
  ['let a := Tensor.ones((1,))\nlet b := a.reshape(())\n', 'Tensor[double, ()]'],
  ['let a := Tensor.ones((2, 3))\nlet b := a.sum()\n', 'double'],
  ['let a := Tensor.ones((2, 3))\nlet b := a.mean(axis: 0)\n', 'Tensor[double, (3,)]'],
  ['let a := Tensor.ones((2, 3))\nlet b := a.min(axis = 1)\n', 'Tensor[double, (2,)]'],
  ['let a := Tensor.ones((2, 3))\nlet axis := 1\nlet b := a.max(axis: axis)\n', 'DynTensor[double]'],
  ['let a := Tensor.ones((3,))\nlet b := a.sum(axis: 0)\n', 'double'],
  ['let n := 3\nlet a := Tensor.ones((n,))\nlet b := a.sum(axis: 0)\n', 'DynTensor[double]'],
  ['let a := Tensor.ones((2, 3))\nlet b := a.sum(axis: 2)\n', ''],
  ['let a := Tensor.ones((2, 3))\nlet b := a.mean(axis: -1)\n', ''],
  ['let a := Tensor.ones((2, 3))\nlet b := a[1, 2]\n', 'double'],
  ['let a := Tensor.ones((2, 3))\nlet b := a[1, :]\n', 'Tensor[double, (3,)]'],
  ['let a := Tensor.ones((5, 6))\nlet b := a[1:3, 2:4]\n', 'Tensor[double, (2, 2)]'],
  ['let a := Tensor.ones((5, 6))\nlet i := 2\nlet b := a[i:, :]\n', 'DynTensor[double]'],
  ['let a := Tensor.ones((5, 6))\nlet b := a[:2, 3:]\n', 'Tensor[double, (2, 3)]'],
  ['let b := Tensor.filled(value = 0.5, shape = (2, 3))\n', 'Tensor[double, (2, 3)]'],
  ['function f(v : DynTensor[double]) : void {\nlet b := v.checkShape(3)?\n}\n', 'Tensor[double, (3,)]']
];
for (const [source, expected] of phaseTwoTensors) {
  const inferred = infer.inferredBindings(source, kestrel.parseDocument(source));
  assert.strictEqual(inferred[inferred.length - 1].type, expected, source);
}

// Conversion constructors and uppercase values retain their types through chains.
for (const [source, expected] of [
  ['let X := Tensor([0, 0])', 'Tensor[double, (2,)]'],
  ['let X := Tensor([0, 0], shape = (1, 2))', 'Tensor[double, (1, 2)]'],
  ['let X := Tensor([1 + 1, 2])', 'Tensor[double, (2,)]'],
  ['let X := Tensor([0x10, 0x20])', 'Tensor[double, (2,)]'],
  ['let X := Tensor([-1, 2])', 'Tensor[double, (2,)]'],
  ['let X := Tensor([1_000, 2_000])', 'Tensor[double, (2,)]'],
  ['let X := Tensor([9007199254740992, 0])', 'Tensor[double, (2,)]'],
  ['let X := Tensor([9007199254740993, 0])', ''],
  ['let n := 5\nlet X := Tensor([n, n])', ''],
  ['let X := Tensor(Array([0, 0]))', ''],
  ['let X := Tensor([0, 0])\nlet Y := X[0]', 'double'],
  ['let X := Tensor([0, 0])\nlet Y := X + 1', 'Tensor[double, (2,)]'],
  ['let X := Tensor([0, 0])\nlet Y := X.copy()', 'Tensor[double, (2,)]'],
  ['let X_c := Tensor([0, 0])', 'Tensor[double, (2,)]'],
  ['let X_c := Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2))', 'Tensor[double, (2, 2)]'],
  ['let X := Tensor([1.0, 2.0])\nlet Row := X[0]', 'double'],
  ['let X := Tensor([1.0, 2.0])\nlet Copy := X.copy()', 'Tensor[double, (2,)]'],
  ['let X := Tensor([1.0, 2.0])\nlet Shape := X.shape()', 'Array[Int]'],
  ['let X := Tensor([1.0, 2.0])\nlet Y := X.broadcastTo(X.shape())', 'DynTensor[double]'],
  ['let Shape := Array([2, 1])\nlet X := Tensor([1.0, 2.0], shape = Shape)', 'DynTensor[double]'],
  ['let Shape := Array([2, 1])\nlet X := Tensor.ones(Shape)', 'DynTensor[double]'],
  ['let X := Tensor([1.0, 2.0])\nlet Y := X.reshape(Array([1, 2]))', 'DynTensor[double]'],
  ['let X := Tensor([1.0, 2.0])\nlet Y := X.transpose(Array([0]))', 'DynTensor[double]'],
  ['let X := Tensor([1.0, 2.0], shape = (3, 2))', ''],
  ['let X := Tensor([])', 'Tensor[double, (0,)]'],
  ['let X := Tensor([1.0, 2.0]).reshape((1, 2))', 'Tensor[double, (1, 2)]'],
  ['let A := Array([1, 2, 3])', 'Array[Int]'],
  ['let D := double(1 + 2)', 'double'],
  ['let S := String(3)', 'String'],
  ['let D := double("3")', '']
]) {
  assert.strictEqual(infer.inferredBindings(source, kestrel.parseDocument(source)).at(-1).type, expected, source);
}

for (const [expression, expected] of [
  ['X / 2.0', 'Tensor[double, (2, 3)]'],
  ['X.divide(2.0)', 'Tensor[double, (2, 3)]'],
  ['X / 2', 'Tensor[double, (2, 3)]'], ['2.0 / X', ''], ['X / X', '']
]) {
  const source = `let X := Tensor.ones((2, 3))\nlet result := ${expression}\n`;
  assert.strictEqual(infer.inferredBindings(source, kestrel.parseDocument(source)).at(-1).type, expected, source);
}
const dynamicDivision = 'let dims := Array([2, 3])\nlet X := Tensor.ones(dims)\nlet result := X / 2.0\n';
assert.strictEqual(infer.inferredBindings(dynamicDivision, kestrel.parseDocument(dynamicDivision)).at(-1).type, 'DynTensor[double]');

// std.kesdas: constructors, column lookups, chained methods and fields.
const kesdasSource = `import std.kesdas.{Series, DataFrame, readCsv}
let sales := DataFrame([
    Series(["north", "south"], name = "region"),
    Series([1, 2], name = "units"),
])
let units := sales.col("units")?
let total := units.sum()
let value := total.asDouble()
let groups := sales.groupBy("region")?
let grouped := groups.sum()
let ordered := sales.sortValues("units")?
let row := sales.row(0)
let shape := sales.shape()
let names := sales.names
let first := sales.get("units")?
let parsed := DataFrame.parseCsv("v\\n1\\n")
let loaded := readCsv("data.csv")?
let scaled := units * 2.0
let one := units.iloc(0)
let many := units.iloc([0, 1])
let rolled := units.rolling(2).mean()
let upper := units.str().upper()
let failed := sales.col("missing")
`;
const kesdasTypes = Object.fromEntries(
  infer.inferredBindings(kesdasSource, kestrel.parseDocument(kesdasSource)).map(binding => [binding.name, binding.type])
);
for (const [name, type] of Object.entries({
  sales: 'DataFrame',
  units: 'Series',
  total: 'Scalar',
  value: 'double',
  groups: 'GroupBy',
  grouped: 'DataFrame',
  ordered: 'DataFrame',
  row: 'Series',
  shape: '(Int, Int)',
  names: 'List[String]',
  first: 'Series',
  parsed: 'DataFrame',
  loaded: 'DataFrame',
  scaled: 'Series',
  one: 'Scalar',
  many: 'Series',
  rolled: 'Series',
  upper: 'Series',
  failed: 'Series or Error'
})) {
  assert.strictEqual(kesdasTypes[name], type, `kesdas ${name} should infer ${type}`);
}

// An overload is chosen by argument count, so a subset call keeps its `or Error` result.
const dropSource = 'import std.kesdas.{Series, DataFrame}\nlet df := DataFrame([Series([1], name = "v")])\nlet all := df.dropNa()\nlet some := df.dropNa(["v"])?\nlet unique := df.dropDuplicates(["v"])?\n';
const dropTypes = Object.fromEntries(
  infer.inferredBindings(dropSource, kestrel.parseDocument(dropSource)).map(binding => [binding.name, binding.type])
);
assert.strictEqual(dropTypes.all, 'DataFrame');
assert.strictEqual(dropTypes.some, 'DataFrame');
assert.strictEqual(dropTypes.unique, 'DataFrame');

// A `?` receiver keeps its success type through a member chain, as `df.col("x")?.mean()` does.
const kesdasChain = 'import std.kesdas.{DataFrame, Series}\nlet df := DataFrame([Series([1], name = "v")])\nlet mean := df.col("v")?.mean().asDouble()\n';
assert.strictEqual(infer.inferredBindings(kesdasChain, kestrel.parseDocument(kesdasChain)).at(-1).type, 'double');

// A namespace import keeps qualified constructors and functions typed.
const namespaceKesdas = `import std.kesdas as kd
let df := kd.DataFrame([kd.Series([1, 2], name = "v")])
let column := df.col("v")?
let loaded := kd.readCsv("data.csv")?
let empty := kd.DataFrame.empty()
`;
const namespaceTypes = Object.fromEntries(
  infer.inferredBindings(namespaceKesdas, kestrel.parseDocument(namespaceKesdas)).map(binding => [binding.name, binding.type])
);
assert.strictEqual(namespaceTypes.df, 'DataFrame');
assert.strictEqual(namespaceTypes.column, 'Series');
assert.strictEqual(namespaceTypes.loaded, 'DataFrame');
assert.strictEqual(namespaceTypes.empty, 'DataFrame');

// A member dot after `?` keeps the unwrapped call as the receiver expression.
const questionImport = 'import std.kesdas.{Series, DataFrame}\n';
const questionSource = `${questionImport}let df := DataFrame([Series([1], name = "v")])\ndf.col("v")?.`;
assert.strictEqual(infer.memberReceiverExpression(questionSource, questionSource.length), 'df.col("v")?');

// Without ?, a Series-or-Error offers no Series members and no inferred result.
const unionSource = 'import std.kesdas.{Series, DataFrame}\nlet df := DataFrame([Series([1], name = "v")])\nlet bad := df.col("v").sum()\n';
assert.strictEqual(infer.inferredBindings(unionSource, kestrel.parseDocument(unionSource)).at(-1).type, '');

// Tuple destructuring names each component of the right side's tuple type.
const tupleSource = `function pair() : (Int, double) = (1, 2.0)
let (whole, part) := pair()
let (ignored, second) := (1, 2.0)
let (_, wildcard) := (1, 2.0)
`;
const tupleTypes = Object.fromEntries(
  infer.inferredBindings(tupleSource, kestrel.parseDocument(tupleSource)).map(binding => [binding.name, binding.type])
);
assert.strictEqual(tupleTypes.whole, 'Int');
assert.strictEqual(tupleTypes.part, 'double');
assert.strictEqual(tupleTypes.second, 'double');
assert.strictEqual(tupleTypes.ignored, 'Int');
assert.strictEqual(tupleTypes.wildcard, 'double');

// The ML split helpers return tuples, so their components keep the tensor and index types.
const splitSource = `import std.ml.{trainTestSplit, foldIndices}
import std.random.{Rng}
let X := Tensor([0, 0, 1, 1], shape = (2, 2))
let y := Tensor([0, 0])
let rng := Rng.seeded(1)
let (trainingX, testingX, trainingY, testingY) := trainTestSplit(X, y, 1, rng)
let (foldTrain, foldTest) := foldIndices(4, 2, 0)
`;
const splitTypes = Object.fromEntries(
  infer.inferredBindings(splitSource, kestrel.parseDocument(splitSource)).map(binding => [binding.name, binding.type])
);
for (const name of ['trainingX', 'testingX', 'trainingY', 'testingY']) {
  assert.strictEqual(splitTypes[name], 'DynTensor[double]', `trainTestSplit gives ${name}`);
}
assert.strictEqual(splitTypes.foldTrain, 'Array[Int]');
assert.strictEqual(splitTypes.foldTest, 'Array[Int]');

// A call with an open slot is a function waiting for that slot, not the result.
const partialSource = `function add(a : double, b : double) : double = a + b
let second := add(1.0, _)
let first := add(_, 2.0)
let both := add(_, _)
class C {
    new() {}
    public function m(v : Int, w : double) : String = "x"
}
let c := C()
let method := c.m(1, _)
`;
const partialTypes = Object.fromEntries(
  infer.inferredBindings(partialSource, kestrel.parseDocument(partialSource)).map(binding => [binding.name, binding.type])
);
assert.strictEqual(partialTypes.second, '(b : double) -> double');
assert.strictEqual(partialTypes.first, '(a : double) -> double');
assert.strictEqual(partialTypes.both, '(a : double, b : double) -> double');
assert.strictEqual(partialTypes.method, '(w : double) -> String');

// Open slots are inferred only where the compiler accepts them. A conversion
// name and an overloaded intrinsic keep no partial type.
const refusedPartials = `let sqrtPartial := sqrt(_)
let absPartial := abs(_)
let doublePartial := double(_)
let tensorPartial := Tensor(_)
let tensorFactoryPartial := Tensor.zeros(_)
let tensorMethodPartial := Tensor.zeros((2, 2)).reshape(_)
let rangePartial := range(_)
`;
const refusedPartialTypes = Object.fromEntries(
  infer.inferredBindings(refusedPartials, kestrel.parseDocument(refusedPartials)).map(binding => [binding.name, binding.type])
);
assert.strictEqual(refusedPartialTypes.sqrtPartial, '');
assert.strictEqual(refusedPartialTypes.absPartial, '');
assert.strictEqual(refusedPartialTypes.doublePartial, '');
assert.strictEqual(refusedPartialTypes.tensorPartial, '');
assert.strictEqual(refusedPartialTypes.tensorFactoryPartial, '');
assert.strictEqual(refusedPartialTypes.tensorMethodPartial, '');
assert.strictEqual(refusedPartialTypes.rangePartial, '(stop : Int) -> Lazy[Int]');
