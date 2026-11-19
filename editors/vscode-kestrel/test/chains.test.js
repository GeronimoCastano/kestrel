'use strict';

// Deep chains are where a syntactic editor usually gives up: the type of the
// fifth call depends on the first four. These tests pin down completion, hover,
// signature help, inlay hints and inferred binding types at every depth.

const assert = require('assert');
const kestrel = require('../out/kestrel');
const infer = require('../out/infer');
const { loadExtension, endOf, labelsAt, hoverAt, signatureAt, inlayHints } = require('../test-support/harness');

const { providers } = loadExtension();

function typesOf(text) {
  return Object.fromEntries(
    infer.inferredBindings(text, kestrel.parseDocument(text)).map(binding => [binding.name, binding.type || ''])
  );
}

function hintLabels(text) {
  return inlayHints(providers, text).map(hint => hint.label);
}

const kesdas = 'import std.kesdas.{Series, DataFrame}\n';

// A local class that returns itself is the clearest deep chain.
const counter = `class Counter {
    public final value : Int
    new(value : Int) { this.value = value }
    public function next() : Counter = Counter(this.value + 1)
}
let counter := Counter(0)
`;
for (const depth of [3, 4, 5]) {
  const chain = `counter${'.next()'.repeat(depth)}.`;
  const labels = labelsAt(providers, counter + chain, (counter + chain).length);
  for (const name of ['next', 'value']) {
    assert(labels.includes(name), `a ${depth}-dot Counter chain offers ${name}: ${labels.slice(0, 8)}`);
  }
  const deep = `${counter}let deep := counter${'.next()'.repeat(depth)}\n`;
  assert.strictEqual(typesOf(deep).deep, 'Counter', `${depth}-dot Counter type`);
  assert(hintLabels(deep).includes(': Counter'), `${depth}-dot Counter inlay type: ${hintLabels(deep)}`);
}

// typeAt agrees with completion at every dot, not just the last one.
function receiverType(text, dot) {
  const model = kestrel.parseDocument(text);
  const expression = infer.memberReceiverExpression(text, dot + 1);
  return infer.typeAt(text, dot + 1, model, expression);
}
for (const depth of [1, 2, 3, 4, 5]) {
  const text = `${counter}counter${'.next()'.repeat(depth - 1)}.next().\n`;
  const dot = text.lastIndexOf('.');
  assert.strictEqual(receiverType(text, dot), 'Counter', `${depth}-dot receiver type`);
}

// Type propagation at every link, not only at the end of the statement.
const stages = typesOf(`${kesdas}let df := DataFrame([Series([1, 2, 3], name = "v")])
let cleaned := df.fillNa(0.0)
let ordered := cleaned.sortValues("v")
let unwrapped := ordered?
let top := unwrapped.head(2)
let bottom := top.tail(1)
let column := df.col("v")?
let filled := column.fillNa(0.0)
let scaled := filled.multiply(2.0)
let shifted := scaled.add(1.0)
`);
assert.deepStrictEqual(
  [stages.cleaned, stages.ordered, stages.unwrapped, stages.top, stages.bottom],
  ['DataFrame', 'DataFrame or Error', 'DataFrame', 'DataFrame', 'DataFrame'],
  'frame stages keep their types'
);
assert.deepStrictEqual(
  [stages.column, stages.filled, stages.scaled, stages.shifted],
  ['Series', 'Series', 'Series', 'Series'],
  'series stages keep their types'
);

const listStages = typesOf(`let numbers := [1, 2, 3, 4]
let mapped := numbers.map(n => n * 2)
let filtered := mapped.filter(n => n > 2)
let array := filtered.toArray()
let again := array.map(n => n + 1)
`);
assert.deepStrictEqual(
  [listStages.numbers, listStages.mapped, listStages.filtered, listStages.array, listStages.again],
  ['List[Int]', 'Lazy[Int]', 'Lazy[Int]', 'Array[Int]', 'Lazy[Int]'],
  'list stages keep their types'
);

// Hover and signature help read the same receiver at depth.
const counterChain = `${counter}counter.next().next().next().next().next().value\n`;
let searchFrom = counterChain.indexOf('counter.next()');
for (let depth = 1; depth <= 5; depth += 1) {
  const method = counterChain.indexOf('next()', searchFrom);
  const counterHover = hoverAt(providers, counterChain, method + 1);
  assert(counterHover && counterHover.contents.value.includes('next() : Counter'), `hover names Counter.next at depth ${depth}`);
  searchFrom = method + 'next()'.length;
}
const valueHover = hoverAt(providers, counterChain, counterChain.indexOf('.value') + 2);
assert(valueHover && valueHover.contents.value.includes('field value : Int'), 'hover names a deep Counter field');
for (const depth of [3, 4, 5]) {
  const counterSignature = `${counter}counter${'.next()'.repeat(depth - 1)}.next(\n`;
  const counterHelp = signatureAt(providers, counterSignature, counterSignature.length);
  assert(counterHelp && counterHelp.signatures[0].label.includes('next() : Counter'), `signature help at depth ${depth}`);
}

// A five-call List pipeline keeps producing List, then Array at toArray.
const list = `let numbers := [1, 2, 3, 4]
let result := numbers.map(n => n * 2).filter(n => n > 2).map(n => n + 1).filter(n => n < 100).toArray()
`;
assert.strictEqual(typesOf(list).result, 'Array[Int]', 'five-call List pipeline type');
assert(hintLabels(list).includes(': Array[Int]'), 'five-call List pipeline inlay type');
const listChain = `${list}numbers.map(n => n * 2).filter(n => n > 2).map(n => n + 1).filter(n => n < 100).toArray().`;
const listLabels = labelsAt(providers, listChain, listChain.length);
for (const name of ['map', 'filter', 'toList', 'toArray', 'length', 'sorted']) {
  assert(listLabels.includes(name), `a five-call List chain offers ${name}: ${listLabels.slice(0, 8)}`);
}

// DataFrame: fillNa -> sortValues? -> head -> tail -> copy has the frame's members again.
const frame = `${kesdas}let df := DataFrame([Series([1, 2, 3], name = "v")])
let sorted := df.fillNa(0.0).sortValues("v", ascending = false)?.head(2).tail(1).copy()
`;
assert.strictEqual(typesOf(frame).sorted, 'DataFrame', 'five-call DataFrame chain type');
assert(hintLabels(frame).includes(': DataFrame'), 'five-call DataFrame inlay type');
const frameChain = `${frame}df.fillNa(0.0).sortValues("v", ascending = false)?.head(2).tail(1).copy().`;
const frameLabels = labelsAt(providers, frameChain, frameChain.length);
for (const name of ['sortValues', 'head', 'tail', 'select', 'describe', 'col', 'groupBy']) {
  assert(frameLabels.includes(name), `a five-call DataFrame chain offers ${name}: ${frameLabels.slice(0, 10)}`);
}
assert(!frameLabels.includes('toFrame'), `a DataFrame chain does not offer Series.toFrame: ${frameLabels.slice(0, 10)}`);

const frameHover = hoverAt(providers, frameChain, frameChain.indexOf('sortValues') + 1);
assert(frameHover && frameHover.contents.value.includes('DataFrame.sortValues'), 'hover keeps the frame after fillNa');
const frameHelp = signatureAt(providers, `${frame}df.fillNa(0.0).sortValues(\n`, frame.length + 'df.fillNa(0.0).sortValues(\n'.length);
assert(frameHelp && frameHelp.signatures.some(signature => signature.label.includes('sortValues(by')), 'signature help follows a frame chain');
const headHover = hoverAt(providers, frameChain, frameChain.indexOf('head(2)') + 1);
assert(headHover && headHover.contents.value.includes('head(n'), 'hover follows sortValues?.head');

const sortedHeadCall = `${frame}df.fillNa(0.0).sortValues("v")?.head(\n`;
const sortedHeadHelp = signatureAt(providers, sortedHeadCall, sortedHeadCall.length);
assert(sortedHeadHelp && sortedHeadHelp.signatures.some(signature => signature.label.includes('head(n')), 'signature help after ?.head');
const sortedParameterHints = hintLabels(`${frame}df.fillNa(0.0).sortValues("v", ascending = false)?\n`);
assert(sortedParameterHints.includes('by ='), `parameter hints follow a sorted frame: ${sortedParameterHints}`);

// A selected column is a Series, and Series methods chain too.
const series = `${kesdas}let df := DataFrame([Series([1, 2, 3], name = "v")])
let column := df.col("v")?.fillNa(0.0).multiply(2.0).add(1.0).head(3).tail(2)
`;
assert.strictEqual(typesOf(series).column, 'Series', 'five-call Series chain type');
assert(hintLabels(series).includes(': Series'), 'five-call Series inlay type');
const seriesChain = `${series}df.col("v")?.fillNa(0.0).multiply(2.0).add(1.0).head(3).tail(2).`;
const seriesLabels = labelsAt(providers, seriesChain, seriesChain.length);
for (const name of ['mean', 'sum', 'str', 'rolling', 'toFrame', 'fillNa']) {
  assert(seriesLabels.includes(name), `a five-call Series chain offers ${name}: ${seriesLabels.slice(0, 10)}`);
}
assert(!seriesLabels.includes('groupBy'), `a Series chain does not offer DataFrame.groupBy: ${seriesLabels.slice(0, 10)}`);

// Tensors carry their shape through transpose, reshape and map.
const tensor = `let t := Tensor.ones((2, 3))
let transposed := t.transpose()
let reshaped := transposed.reshape((3, 2)).map(x => x * 2.0)
`;
assert.strictEqual(typesOf(tensor).transposed, 'Tensor[double, (3, 2)]', 'transpose type');
assert.strictEqual(typesOf(tensor).reshaped, 'Tensor[double, (3, 2)]', 'reshape/map type');
const tensorChain = `${tensor}t.transpose().reshape((3, 2)).map(x => x * 2.0).sum(axis = 0).`;
const tensorLabels = labelsAt(providers, tensorChain, tensorChain.length);
for (const name of ['transpose', 'reshape', 'sum', 'mean', 'shape', 'broadcastTo']) {
  assert(tensorLabels.includes(name), `a five-call tensor chain offers ${name}: ${tensorLabels.slice(0, 10)}`);
}
assert(!tensorLabels.includes('push'), `a tensor chain does not offer Stack.push: ${tensorLabels.slice(0, 10)}`);

// Parameter names follow the receiver through a chain, not just the first call.
const adder = `class Adder {
    new() {}
    public function add(a : Int, b : Int) : Adder = this
}
let adder := Adder()
`;
const adderHints = hintLabels(`${adder}adder.add(1, 2).add(3, 4)\n`);
assert(adderHints.includes('a =') && adderHints.includes('b ='), `chained call parameter hints: ${adderHints}`);

// `this`, parenthesized receivers, line breaks and comments keep the chain type.
const methods = `class Node {
    public final value : Int
    new(value : Int) { this.value = value }
    public function next() : Node = Node(this.value + 1)
    public function deep() : Int {
        this.next().next().
    }
}
let node := Node(0)
`;
const thisChain = methods.replace('this.next().next().', 'this.next().next().');
const thisLabels = labelsAt(providers, thisChain, thisChain.indexOf('this.next().next().') + 'this.next().next().'.length);
assert(thisLabels.includes('next') && thisLabels.includes('value'), `this. chain offers members: ${thisLabels.slice(0, 8)}`);
const parenthesized = `${methods}let p := (node.next()).next().`;
assert(labelsAt(providers, parenthesized, parenthesized.length).includes('next'), 'parenthesized receiver chain');
const multiline = `${methods}node.next()\n    .next()\n    .`;
assert(labelsAt(providers, multiline, multiline.length).includes('next'), 'multiline receiver chain');
const commented = `${methods}node.next() // keep going\n    .next().`;
assert(labelsAt(providers, commented, commented.length).includes('next'), 'comment inside chain');

const multilineHoverSource = `${methods}node.next()\n    .next().next().value\n`;
const multilineHover = hoverAt(providers, multilineHoverSource, multilineHoverSource.indexOf('.value') + 2);
assert(multilineHover && multilineHover.contents.value.includes('field value : Int'), 'hover follows a multiline chain');
const multilineCall = `${methods}node.next()\n    .next()\n    .next(\n`;
const multilineHelp = signatureAt(providers, multilineCall, multilineCall.length);
assert(multilineHelp && multilineHelp.signatures[0].label.includes('next() : Node'), 'signature help follows a multiline chain');

// A local function's declared result type feeds the chain too.
const factory = `${kesdas}function makeFrame() : DataFrame = DataFrame([Series([1, 2], name = "v")])
let made := makeFrame()
made.fillNa(0.0).sortValues("v")?.head(1).`;
const factoryLabels = labelsAt(providers, factory, factory.length);
for (const name of ['sortValues', 'head', 'tail', 'col']) {
  assert(factoryLabels.includes(name), `a local factory chain offers ${name}: ${factoryLabels.slice(0, 10)}`);
}
const staticFactory = `class Factory {
    public static function make() : Factory = Factory()
    public function next() : Factory = this
}
let created := Factory.make()
created.next().next().`;
assert(labelsAt(providers, staticFactory, staticFactory.length).includes('next'), 'static factory chain');
const namespaceFrame = `import std.kesdas as kd
let frame := kd.DataFrame([kd.Series([1, 2], name = "v")])
frame.fillNa(0.0).head(1).`;
assert(labelsAt(providers, namespaceFrame, namespaceFrame.length).includes('head'), 'namespace constructor chain');
const indexed = `let cells := Array(3, 0)\ncells[0].`;
const indexedLabels = labelsAt(providers, indexed, indexed.length);
assert(indexedLabels.includes('toDouble') && indexedLabels.includes('text'), `array index keeps Int members: ${indexedLabels.slice(0, 8)}`);

// No members are invented for a receiver with no known type.
const unknown = labelsAt(providers, 'let mystery := somethingUnknown()\nmystery.', 'let mystery := somethingUnknown()\nmystery.'.length);
assert.deepStrictEqual(unknown, [], `an unknown receiver has no members: ${unknown.slice(0, 8)}`);

console.log('kestrel deep chain tests pass');
