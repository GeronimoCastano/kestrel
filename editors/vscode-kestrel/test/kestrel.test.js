'use strict';

const assert = require('assert');
const kestrel = require('../out/kestrel');

const source = `// hello.kes
function greet(name : String) : void with Effects {
    print(f"Hello {name}")
}

class Counter {
    public var count : Int
    new() { this.count = 0 }
    public mut function bump() : Int {
        this.count = this.count + 1
        this.count
    }
}

let point := Point("x")
greet("Geronimo")
`;

const model = kestrel.parseDocument(source);
assert.strictEqual(model.functions.length, 2);
const greet = model.functions.find(fn => fn.name === 'greet');
assert.strictEqual(greet.owner, '');
assert.deepStrictEqual(greet.parameters.map(p => [p.name, p.type]), [['name', 'String']]);
assert.strictEqual(kestrel.signature(greet), 'greet(name : String) : void with Effects');

const bump = model.functions.find(fn => fn.name === 'bump');
assert.strictEqual(bump.owner, 'Counter');
assert.deepStrictEqual(bump.parameters, []);
assert.strictEqual(model.constructors.length, 1);
assert.strictEqual(model.constructors[0].name, 'Counter');
assert.strictEqual(kestrel.signature(model.constructors[0]), 'Counter() : Counter');

const fieldModel = kestrel.parseDocument(`class Counter {
    new(start : Int) {
        final this.start : Int := start
        var this.hits : Int := 0
    }
}
`);
assert.deepStrictEqual(
  fieldModel.classes[0].fields.map(field => [field.name, field.type, field.visibility]),
  [['start', 'Int', 'private'], ['hits', 'Int', 'private']]
);

const callOffset = source.indexOf('greet("Geronimo")') + 'greet('.length;
const call = kestrel.callContext(source, callOffset);
assert.strictEqual(call.callee, 'greet');
assert.strictEqual(call.declarationParams, false);
assert.strictEqual(call.argumentIndex, 0);

const declarationOffset = source.indexOf('function greet(') + 'function greet('.length;
const declaration = kestrel.callContext(source, declarationOffset);
assert.strictEqual(declaration.declarationParams, true);

const nested = kestrel.callContext('greet(identity("x", 1), ', 'greet(identity("x", 1), '.length);
assert.strictEqual(nested.callee, 'greet');
assert.strictEqual(nested.argumentIndex, 1);

assert(kestrel.parametersInUse('name = "x", ').has('name'));
assert(!kestrel.parametersInUse('name == "x"').has('name'));

assert(kestrel.collectLocals(source, source.indexOf('print('), model).includes('name'));
assert(kestrel.collectLocals(source, source.indexOf('greet("Geronimo")'), model).includes('point'));
const expressionFunction = 'function identify(name : String) : String = name\n';
const expressionModel = kestrel.parseDocument(expressionFunction);
const expressionOffset = expressionFunction.lastIndexOf('name');
assert.strictEqual(expressionModel.functions[0].end >= expressionOffset, true);
assert(kestrel.collectLocals(expressionFunction, expressionOffset, expressionModel).includes('name'));

// Theorems are proof facts, not functions. Their parameters and the names a
// tactic introduces are locals inside the proof.
const proofSource = `theorem interSubsetSwap[U](A B : Set[U]) : A ∩ B ⊆ B ∩ A by {
    intro x h
    cases h
    constructor
    exact
}

theorem interComm[U](A B : Set[U]) : A ∩ B = B ∩ A by {
    ext
    exact interSubsetSwap
}
`;
const proofModel = kestrel.parseDocument(proofSource);
assert.deepStrictEqual(proofModel.theorems.map(theorem => theorem.name), ['interSubsetSwap', 'interComm']);
assert.deepStrictEqual(proofModel.theorems[1].parameters.map(parameter => parameter.name), ['A', 'B']);
assert.strictEqual(proofModel.theorems[1].result, 'A ∩ B = B ∩ A');
assert.strictEqual(
  kestrel.signature(proofModel.theorems[1]),
  'interComm(A : Set[U], B : Set[U]) : A ∩ B = B ∩ A'
);
const swapLocals = kestrel.collectLocals(proofSource, proofSource.indexOf('exact\n'), proofModel);
for (const name of ['A', 'B', 'x', 'h']) {
  assert(swapLocals.includes(name), `the proof offers ${name}: ${swapLocals}`);
}
const commLocals = kestrel.collectLocals(proofSource, proofSource.lastIndexOf('exact interSubsetSwap'), proofModel);
assert(commLocals.includes('A') && commLocals.includes('B'), `the second proof keeps its parameters: ${commLocals}`);
assert(!commLocals.includes('x') && !commLocals.includes('h'), `locals stop at the theorem: ${commLocals}`);
assert.deepStrictEqual(
  kestrel.parseParameters('A B : Set[U]').map(parameter => [parameter.name, parameter.type]),
  [['A', 'Set[U]'], ['B', 'Set[U]']]
);
assert.deepStrictEqual(
  kestrel.parseParameters('mut a b : Int').map(parameter => parameter.label),
  ['mut a : Int', 'mut b : Int']
);

const axiomModel = kestrel.parseDocument('axiom all[U](A : Set[U]) : exists x : U, x ∈ A\n');
assert.strictEqual(axiomModel.theorems[0].kind, 'axiom');
assert.strictEqual(axiomModel.theorems[0].result, 'exists x : U, x ∈ A');
assert.strictEqual(axiomModel.theorems[0].end, axiomModel.theorems[0].bodyStart);

// An example has no name to cite, but its parameters are locals in its proof.
const exampleSource = 'example[U](A : Set[U], x : U, h : x ∈ A) : x ∈ A by {\n    exact\n}\n';
const exampleModel = kestrel.parseDocument(exampleSource);
assert.strictEqual(exampleModel.theorems[0].kind, 'example');
assert.strictEqual(exampleModel.theorems[0].name, '');
assert.deepStrictEqual(
  kestrel.collectLocals(exampleSource, exampleSource.indexOf('exact'), exampleModel),
  ['A', 'x', 'h']
);

// Every tactic carries the text the editor shows on hover, and aliases resolve
// to the entry that documents them.
const tacticNames = new Set(kestrel.TACTICS.map(tactic => tactic.name));
for (const name of [
  'intro', 'exact', 'apply', 'have', 'cases', 'induction', 'simp', 'rw',
  'constructor', 'left', 'right', 'exists', 'assumption', 'contradiction',
  'exfalso', 'rfl', 'symm', 'trans', 'decide', 'trivial', 'smt', 'pushNeg', 'ext',
  'byContra', 'repeat', 'try', 'skip', 'done', 'sorry'
]) {
  assert(tacticNames.has(name), `tactic metadata covers ${name}`);
}
for (const tactic of kestrel.TACTICS) {
  assert(tactic.signature && tactic.insert, `${tactic.name} has a shape and a snippet`);
  assert(tactic.summary && tactic.body, `${tactic.name} has hover text`);
  assert.strictEqual(kestrel.tactic(tactic.name), tactic);
  for (const alias of tactic.aliases) assert.strictEqual(kestrel.tactic(alias), tactic);
}
assert.strictEqual(kestrel.tactic('intros').name, 'intro');
assert.strictEqual(kestrel.tactic('use').name, 'exists');
assert.strictEqual(kestrel.tactic('rewrite').name, 'rw');

// Proof constructors carry the call shape and text the editor shows.
const constructorNames = new Set(kestrel.CONSTRUCTORS.map(constructor => constructor.name));
for (const name of ['And', 'OrLeft', 'OrRight', 'Iff', 'Exists', 'True']) {
  assert(constructorNames.has(name), `constructor metadata covers ${name}`);
}
for (const constructor of kestrel.CONSTRUCTORS) {
  assert(constructor.signature && constructor.insert, `${constructor.name} has a shape and a snippet`);
  assert(constructor.summary && constructor.body, `${constructor.name} has hover text`);
  assert.strictEqual(kestrel.proofConstructor(constructor.name), constructor);
}


// A record reads its fields from the parameter list, and its name constructs a
// value. The model treats it as a type with a constructor, like a class.
const recordModel = kestrel.parseDocument(`public record Reading(temperature : Int, humidity : Int) {
    public function span() : Int = this.temperature - this.humidity
}
record Pair[A, B](first : A, second : B)
`);
const reading = recordModel.classesByName.get('Reading');
assert.strictEqual(reading.kind, 'record');
assert.strictEqual(reading.visibility, 'public');
assert.deepStrictEqual(reading.fields.map(field => [field.name, field.type, field.visibility]), [
  ['temperature', 'Int', 'public'],
  ['humidity', 'Int', 'public']
]);
assert.strictEqual(reading.bodyStart > 0, true);
const readingConstructor = recordModel.constructorsByName.get('Reading')[0];
assert.strictEqual(readingConstructor.record, true);
assert.strictEqual(kestrel.signature(readingConstructor), 'Reading(temperature : Int, humidity : Int) : Reading');
assert.deepStrictEqual(
  (recordModel.functionsByOwner.get('Reading') || []).map(fn => fn.name),
  ['span']
);
const pair = recordModel.classesByName.get('Pair');
assert.deepStrictEqual(pair.typeParameters, ['A', 'B']);
assert.deepStrictEqual(pair.fields.map(field => field.name), ['first', 'second']);
assert.deepStrictEqual(
  recordModel.types.filter(type => type.kind === 'record').map(type => type.name),
  ['Reading', 'Pair']
);


// `///` lines directly above a declaration are its Markdown documentation.
const documented = kestrel.parseDocument(`/// Adds two numbers.
/// Second line.
function add(a : Int, b : Int) : Int = a + b

// An implementation note, not documentation.
function plain() : Int = 1

/// A point in the plane.
record Point(x : Int, y : Int)

/// The limit.
const LIMIT : Int = 5

/// A documented test.
@test
function documentedTest() : void {}

/// Detached by a blank line.

function detached() : Int = 1

//// A banner, not documentation.
function banner() : Int = 1
`);
assert.strictEqual(documented.functionsByName.get('add')[0].documentation, 'Adds two numbers.\nSecond line.');
assert.strictEqual(documented.functionsByName.get('plain')[0].documentation, '');
assert.strictEqual(documented.classesByName.get('Point').documentation, 'A point in the plane.');
assert.strictEqual(documented.constantsByName.get('LIMIT').documentation, 'The limit.');
assert.strictEqual(documented.functionsByName.get('documentedTest')[0].documentation, 'A documented test.');
assert.strictEqual(documented.functionsByName.get('detached')[0].documentation, '');
assert.strictEqual(documented.functionsByName.get('banner')[0].documentation, '');
assert.strictEqual(
  documented.constructorsByName.get('Point')[0].documentation,
  'A point in the plane.'
);

// The block keeps Markdown, drops one leading space, and a bare `///` is a
// blank line.
const markdownDoc = kestrel.parseDocument(`/// **Bold** and \`code\`.
///
/// - item
/// $h^{GC}(s)$
function estimate() : Int = 0
`);
assert.strictEqual(
  markdownDoc.functions[0].documentation,
  '**Bold** and \`code\`.\n\n- item\n$h^{GC}(s)$'
);

console.log('kestrel editor parser tests pass');
