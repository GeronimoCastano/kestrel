'use strict';

// Hover, inlay hints, signature help, named arguments and plain completion.
// These are the reading aids that must agree with the inferred binding types.

const assert = require('assert');
const { loadExtension, labelsAt, hoverAt, signatureAt, inlayHints } = require('../test-support/harness');

const configuration = {};
const { providers } = loadExtension(configuration);

function hover(text, needle, offset = 1) {
  const found = text.indexOf(needle);
  assert(found >= 0, `missing ${needle}`);
  return hoverAt(providers, text, found + offset);
}

function hintLabels(text) {
  return inlayHints(providers, text).map(hint => hint.label);
}

const basics = `function add(a : Int, b : Int) : Int = a + b
let total := add(1, 2)
const LIMIT : Int = 5
class Point {
    public final x : Int
    new(x : Int) { this.x = x }
    public function next() : Point = Point(this.x + 1)
}
let point := Point(0)
let px := point.x
`;

// Hover names the declaration that applies at the cursor.
assert(hover(basics, 'add(a : Int').contents.value.includes('add(a : Int, b : Int) : Int'), 'function hover');
assert(hover(basics, 'a : Int', 0).contents.value.includes('parameter a : Int'), 'parameter hover');
assert(hover(basics, 'total').contents.value.includes('total : Int'), 'inferred local hover');
assert(hover(basics, 'LIMIT').contents.value.includes('const LIMIT : Int'), 'constant hover');
assert(hover(basics, 'Point').contents.value.includes('class Point'), 'class hover');
assert(hoverAt(providers, basics, basics.indexOf('point.x') + 6).contents.value.includes('field x : Int'), 'field hover');
assert(hover(basics, 'point := Point').contents.value.includes('point : Point'), 'constructor result hover');

// Tactics explain themselves inside a proof, and only there.
const proof = `theorem same : forall n : Nat, n = n by {
    intro n
    rfl
}
`;
assert(hover(proof, 'intro').contents.value.includes('Introduce a forall binder'), 'intro tactic hover');
assert(hover(proof, 'rfl').contents.value.includes('Prove an equality by computation'), 'rfl tactic hover');
assert(hover(proof, 'intro').contents.value.includes('```kestrel\nintro h\n```'), 'tactic hover shows the shape');
const quantified = `theorem q :
    exists n : Nat, n = n
by {
    exists 0
    rfl
}
`;
assert.strictEqual(hover(quantified, 'exists'), null, 'a quantifier in a statement is not a tactic');
assert(hover(quantified, 'exists 0').contents.value.includes('Give a witness'), 'exists tactic hover in the proof');
const aliased = `theorem q : forall n : Nat, n = n by {
    intros
    rfl
}
`;
assert(hover(aliased, 'intros').contents.value.includes('Introduce a forall binder'), 'tactic alias hover');
assert(hover(aliased, 'intros').contents.value.includes('Also written `intros`'), 'alias is named in the hover');

const setProof = `theorem unionComm[U](A B : Set[U]) : A ∪ B = B ∪ A by {
    ext
    pushNeg
    byContra h
    sorry
}
`;
assert(hover(setProof, 'ext').contents.value.includes('extensionality'), 'ext tactic hover');
assert(hover(setProof, 'byContra').contents.value.includes('classical rule'), 'byContra tactic hover');
assert(hover(setProof, 'pushNeg').contents.value.includes('negations inward'), 'pushNeg tactic hover');

// A theorem brace body uses proof terms, and its branches still name tactics.
const braceProof = `theorem appendEnd(xs : Chain[T]) : append(xs, End) = xs {
    induction xs {
        End => rfl,
        Link(x, rest) ih => congrArg(r => Link(x, r), ih),
    }
}
`;
assert(hover(braceProof, 'induction').contents.value.includes('Prove by structural induction'), 'induction in a brace body');
assert(hover(braceProof, 'rfl').contents.value.includes('Prove an equality by computation'), 'rfl after a branch arrow');
assert(hover(braceProof, 'congrArg') === null, 'a proof term after a branch arrow is not a tactic');

// A chain uses the receiver of the word, not the first name in the statement.
const chain = `${basics}point.next().next().next().next().x\n`;
const fieldHover = hoverAt(providers, chain, chain.indexOf('.x') + 2);
assert(fieldHover && fieldHover.contents.value.includes('field x : Int'), 'deep chain field hover');
const methodHover = hoverAt(providers, chain, chain.indexOf('next()') + 1);
assert(methodHover && methodHover.contents.value.includes('next() : Point'), 'deep chain method hover');

// The standard library and kesdas metadata answer hover the same way.
const kesdas = `import std.kesdas.{Series, DataFrame}
let df := DataFrame([Series([1, 2, 3], name = "v")])
let column := df.col("v")?
let average := column.mean()
let frame := df.head(2).tail(1).head(1)
`;
assert(hover(kesdas, 'mean()').contents.value.includes('Series.mean() : Scalar'), 'kesdas method hover');
assert(hover(kesdas, 'head(2)').contents.value.includes('DataFrame.head'), 'kesdas deep frame hover');
assert(hover('let root := sqrt(4.0)\n', 'sqrt').contents.value.includes('sqrt'), 'standard function hover');
assert(hover(kesdas, 'df := DataFrame').contents.value.includes('df : DataFrame'), 'namespace import local hover');
assert(hover(kesdas, 'DataFrame').contents.value.includes('DataFrame'), 'namespace import type hover');

// Type inlay hints appear for inferred names, loop variables and tuple parts.
const hints = hintLabels(`let x := 1
let xs := [1, 2]
let named : String = "x"
for (let item in [1, 2]) {
    let doubled := item * 2
}
let (a, b) := (1, "x")
`);
assert(hints.includes(': Int'), 'Int binding hint');
assert(hints.includes(': List[Int]'), 'List binding hint');
assert(hints.includes(': String'), 'String binding hint');
assert(hints.includes(': Int') && hints.filter(label => label === ': Int').length >= 2, 'loop and tuple hints');
assert(!hints.includes(': void'), 'no void hints');

// Parameter names are offered for function and method calls.
const call = `function add(a : Int, b : Int) : Int = a + b
class Adder {
    new() {}
    public function add(a : Int, b : Int) : Adder = this
}
let adder := Adder()
let total := add(1, 2)
adder.add(3, 4).add(5, 6)
`;
const callHints = hintLabels(call);
assert(callHints.includes('a =') && callHints.includes('b ='), `parameter hints: ${callHints}`);

// A same-named argument is quiet by default, and loud when the setting is off.
const sameName = `function echo(value : Int) : Int = value
let value := 3
let copied := echo(value)
`;
configuration['inlayHints.skipMatchingNames'] = true;
assert(!hintLabels(sameName).includes('value ='), 'matching argument name is skipped');
configuration['inlayHints.skipMatchingNames'] = false;
assert(hintLabels(sameName).includes('value ='), 'matching argument name shows when skipping is off');
configuration['inlayHints.skipMatchingNames'] = true;

configuration['inlayHints.parameterNameStyle'] = 'colon';
assert(hintLabels(call).includes('a:'), 'colon parameter style');
configuration['inlayHints.parameterNameStyle'] = 'equals';

configuration['inlayHints.minimumArguments'] = 2;
const oneArgument = hintLabels('function square(x : Int) : Int = x * x\nlet squared := square(2)\n');
assert(!oneArgument.includes('x ='), 'a one-argument call is quiet at minimum two');
configuration['inlayHints.minimumArguments'] = 1;

// Standard functions and static methods also name their parameters.
assert(hintLabels('let t := Tensor.ones((2, 3))\n').includes('shape ='), 'static method parameter hint');
assert(hintLabels('let r := range(3)\n').includes('stop ='), 'standard function parameter hint');
assert(hintLabels('let n := parseInt("3")\n').includes('text ='), 'standard parse parameter hint');

configuration['inlayHints.parameterNames'] = 'off';
const offHints = hintLabels(call);
assert(!offHints.includes('a =') && offHints.includes(': Int'), 'parameter names off keeps type hints');
configuration['inlayHints.parameterNames'] = 'always';

// Signature help tracks the argument being typed.
const addCall = `${basics}add(1, 2\n`;
const addHelp = signatureAt(providers, addCall, addCall.length);
assert(addHelp && addHelp.signatures[0].label.includes('add(a : Int, b : Int) : Int'), 'signature help names add');
assert.strictEqual(addHelp.activeParameter, 1, 'signature help points at the second argument');

const partial = `${basics}add(\n`;
assert.strictEqual(signatureAt(providers, partial, partial.length).activeParameter, 0, 'signature help starts at the first argument');

const declaration = `${basics}function incomplete(\n`;
assert.strictEqual(signatureAt(providers, declaration, declaration.length), null, 'no signature help in a declaration');

const constructorCall = `${basics}Point(\n`;
const constructorHelp = signatureAt(providers, constructorCall, constructorCall.length);
assert(constructorHelp && constructorHelp.signatures[0].label.includes('Point(x : Int)'), 'constructor signature help');

const sortedCall = `${kesdas}df.fillNa(0.0).sortValues("v", \n`;
const sortedHelp = signatureAt(providers, sortedCall, sortedCall.length);
assert(sortedHelp && sortedHelp.signatures.some(signature => signature.label.includes('sortValues(by')), 'kesdas chain signature help');
assert.strictEqual(sortedHelp.activeParameter, 1, 'kesdas chain signature parameter');

// A theorem call in a proof has signature help and a hover with its statement.
// Its parameters are positional, so no `name =` completion appears.
const theoremCall = `theorem pair(first : Nat, second : Nat) : first = first by { rfl }
theorem use(n : Nat) : n = n by {
    exact pair(`;
const theoremHelp = signatureAt(providers, theoremCall, theoremCall.length);
assert(
  theoremHelp && theoremHelp.signatures[0].label.includes('pair(first : Nat, second : Nat) : first = first'),
  'theorem signature help'
);
assert.strictEqual(theoremHelp.activeParameter, 0, 'theorem signature starts at the first argument');
const theoremHover = hoverAt(providers, theoremCall, theoremCall.lastIndexOf('pair'));
assert(
  theoremHover && theoremHover.contents.value.includes('pair(first : Nat, second : Nat) : first = first'),
  'theorem hover shows the statement'
);
const theoremArguments = labelsAt(providers, theoremCall, theoremCall.length);
assert(!theoremArguments.includes('first') && !theoremArguments.includes('second'), 'theorem arguments stay positional');

// A proof constructor shows its shape and tracks the argument being typed.
const andCall = `theorem construct[U](x : U, A B : Set[U], h1 : x ∈ A, h2 : x ∈ B) : x ∈ A ∩ B by {
    exact And(`;
const andHelp = signatureAt(providers, andCall, andCall.length);
assert(andHelp && andHelp.signatures[0].label === 'And(p, q)', 'constructor signature help');
assert.strictEqual(andHelp.activeParameter, 0, 'constructor signature starts at the first argument');
const andSecond = `${andCall}h1, `;
assert.strictEqual(signatureAt(providers, andSecond, andSecond.length).activeParameter, 1, 'constructor signature moves to the second argument');

// Named-argument completion is filtered by the parameters already used.
const namedCall = `${basics}add(`;
assert(labelsAt(providers, namedCall, namedCall.length).includes('a'), 'named argument a');
assert(labelsAt(providers, `${namedCall}a = 1, `, `${namedCall}a = 1, `.length).includes('b'), 'named argument b after a');
assert(!labelsAt(providers, `${namedCall}a = 1, `, `${namedCall}a = 1, `.length).includes('a'), 'used named argument is hidden');

// Plain completion still offers the local scope, declarations and keywords.
const terms = labelsAt(providers, `${basics}ad`, `${basics}ad`.length);
for (const label of ['add', 'total', 'LIMIT', 'Point', 'function']) {
  assert(terms.includes(label), `global completion offers ${label}: ${terms.slice(0, 8)}`);
}
const parameterLabels = labelsAt(providers, `${basics}function inside(a : Int) : Int {\n    `, `${basics}function inside(a : Int) : Int {\n    `.length);
assert(parameterLabels.includes('a'), 'a parameter is offered inside its function');

// Import paths and selective names are completed.
assert(labelsAt(providers, 'import std.', 'import std.'.length).includes('linalg'), 'module path completion');
assert(labelsAt(providers, 'import std.linalg.{', 'import std.linalg.{'.length).includes('solve'), 'selective import completion');
assert.deepStrictEqual(labelsAt(providers, 'import std.linalg as ', 'import std.linalg as '.length), [], 'an alias position offers no names');

// A record constructs by name and its fields are the parameters, so hover,
// signature help and the parameter hints name the same fields.
const records = `public record Reading(temperature : Int, humidity : Int) {
    public function span() : Int = this.temperature - this.humidity
}
let reading := Reading(20, 40)
`;
assert(hover(records, 'Reading').contents.value.includes('record Reading'), 'record hover names the record');
assert(
  hover(records, 'Reading').contents.value.includes('Reading(temperature : Int, humidity : Int) : Reading'),
  'record hover shows the constructor shape'
);
const recordHelp = signatureAt(providers, `${records}Reading(`, `${records}Reading(`.length);
assert(
  recordHelp && recordHelp.signatures[0].label.includes('Reading(temperature : Int, humidity : Int) : Reading'),
  'record constructor signature help'
);
assert.strictEqual(recordHelp.activeParameter, 0, 'record signature starts at the first field');
assert.strictEqual(recordHelp.signatures[0].documentation.value, 'Record constructor', 'signature help names a record constructor');
const recordHints = hintLabels(records);
assert(
  recordHints.includes('temperature =') && recordHints.includes('humidity ='),
  `record field hints: ${recordHints}`
);
const recordMembers = labelsAt(providers, `${records}reading.`, `${records}reading.`.length);
assert(recordMembers.includes('temperature') && recordMembers.includes('humidity'), 'record fields complete');
assert(recordMembers.includes('span'), 'record methods complete');


// `///` documentation appears under the signature in hover and signature help.
const documented = `/// Adds two numbers.
/// The second line stays.
function add(a : Int, b : Int) : Int = a + b

// An implementation note.
function plain() : Int = 1

/// A point in the plane.
record Point(x : Int, y : Int)

add(1, 2)
plain()
Point(1, 2)
`;
const addHover = hoverAt(providers, documented, documented.indexOf('add(1') + 1);
assert(addHover.contents.value.includes('add(a : Int, b : Int) : Int'), 'documented signature');
assert(
  addHover.contents.value.includes('Adds two numbers.\nThe second line stays.'),
  'function documentation in hover'
);
const plainHover = hoverAt(providers, documented, documented.lastIndexOf('plain()') + 1);
assert(!plainHover.contents.value.includes('implementation note'), 'ordinary comment is not documentation');
const pointHover = hoverAt(providers, documented, documented.lastIndexOf('Point(1') + 1);
assert(pointHover.contents.value.includes('A point in the plane.'), 'record documentation in hover');
assert(pointHover.contents.value.includes('record Point'), 'record hover keeps its kind');
const addCallDoc = documented.indexOf('add(1') + 'add('.length;
const documentedHelp = signatureAt(providers, documented, addCallDoc);
assert(
  documentedHelp.signatures[0].documentation.value.includes('Adds two numbers.'),
  'signature help carries the documentation'
);

// A documented constructor keeps its documentation under its own signature
// instead of replacing the documentation of the class it belongs to.
const documentedClass = `/// The goal-count heuristic.
public class GoalCountHeuristic {
    /// Builds the heuristic for \`problem\`.
    public new(problem : Int) {
        this.problem = problem
    }
}
`;
const classHover = hover(documentedClass, 'class GoalCountHeuristic', 'class '.length + 1);
assert(
  classHover.contents.value.includes('The goal-count heuristic.'),
  `class documentation in hover: ${classHover.contents.value}`
);
assert(
  classHover.contents.value.includes('Builds the heuristic for `problem`.'),
  'constructor documentation stays in hover'
);
assert(
  classHover.contents.value.indexOf('The goal-count heuristic.') <
    classHover.contents.value.indexOf('Builds the heuristic for'),
  'class documentation comes before the constructor documentation'
);
const undocumentedConstructor = `/// Keeps its documentation.
class Quiet {
    new() {}
}
`;
const quietHover = hover(undocumentedConstructor, 'class Quiet', 'class '.length + 1);
assert(quietHover.contents.value.includes('Keeps its documentation.'), 'class documentation without a constructor note');
assert.strictEqual(
  quietHover.contents.value.split('```').length - 1,
  2,
  'an undocumented constructor shares the class signature block'
);

console.log('kestrel hover, hint and signature tests pass');
