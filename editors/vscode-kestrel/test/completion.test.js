'use strict';

const assert = require('assert');
const Module = require('module');
const path = require('path');

// Loads the extension with a stand-in vscode module and returns its completion provider.
const captured = {};
class CompletionItem {
  constructor(label, kind) { this.label = label; this.kind = kind; }
}
const stub = () => ({ dispose() {} });
const vscode = {
  CompletionItem,
  CompletionItemKind: new Proxy({}, { get: (target, name) => String(name) }),
  MarkdownString: class MarkdownString { constructor(value) { this.value = value; } },
  SnippetString: class SnippetString { constructor(value) { this.value = value; } },
  Hover: class Hover { constructor(contents, range) { this.contents = contents; this.range = range; } },
  SignatureHelp: class SignatureHelp {},
  SignatureInformation: class SignatureInformation { constructor(label) { this.label = label; } },
  ParameterInformation: class ParameterInformation { constructor(label) { this.label = label; } },
  EventEmitter: class EventEmitter { constructor() { this.event = stub; } fire() {} dispose() {} },
  ConfigurationTarget: { Global: 1 },
  window: {
    createOutputChannel: () => ({ appendLine() {}, dispose() {} }),
    onDidChangeTextEditorSelection: stub,
    setStatusBarMessage: stub
  },
  commands: { registerCommand: stub },
  workspace: {
    getConfiguration: () => ({ get: () => undefined, update: () => Promise.resolve() }),
    onDidChangeConfiguration: stub,
    onDidSaveTextDocument: stub,
    getWorkspaceFolder: () => undefined,
    textDocuments: [],
    onDidOpenTextDocument: stub,
    onDidChangeTextDocument: stub,
    onDidCloseTextDocument: stub
  },
  languages: new Proxy({
    createDiagnosticCollection: () => ({ set() {}, delete() {}, dispose() {} }),
    registerCompletionItemProvider: (selector, provider) => { captured.completion = provider; return stub(); },
    registerHoverProvider: (selector, provider) => { captured.hover = provider; return stub(); },
    registerSignatureHelpProvider: (selector, provider) => { captured.signature = provider; return stub(); }
  }, { get: (target, name) => (name in target ? target[name] : stub) })
};
const load = Module._load;
Module._load = function (request, parent, isMain) {
  if (request === 'vscode') {
    return new Proxy(vscode, { get: (target, name) => (name in target ? target[name] : class Stub {}) });
  }
  return load.call(this, request, parent, isMain);
};
const extension = require(path.join(__dirname, '..', 'out', 'extension.js'));
extension.activate({ subscriptions: { push() {} } });
Module._load = load;

// The labels offered at the end of `source`, which must end with the member dot and any prefix.
function labelsAfter(source, at = source.length) {
  const document = {
    getText: () => source,
    offsetAt: position => position,
    positionAt: offset => offset
  };
  const items = captured.completion.provideCompletionItems(document, at) || [];
  return items.map(item => item.label);
}

// A literal receiver offers its methods, just as a named list does.
const literal = labelsAfter('let array := [1, 2, 3].');
for (const name of ['toArray', 'toList', 'map', 'filter', 'length', 'contains', 'at']) {
  assert(literal.includes(name), `a list literal offers ${name}: ${literal.slice(0, 8)}`);
}

// A lazy result from map offers the sequence methods again.
const chained = labelsAfter('let doubled := [1, 2, 3].map(n => n * 2).');
for (const name of ['toArray', 'toList', 'map', 'filter']) {
  assert(chained.includes(name), `a map result offers ${name}: ${chained.slice(0, 8)}`);
}

// The same holds for filter, and for a prefix already typed after the dot.
const filtered = labelsAfter('let f := [1, 2, 3].filter(n => n > 1).to');
assert(filtered.includes('toArray') && filtered.includes('toList'), 'a filter result offers its methods');

// A named list still works, as before.
const named = labelsAfter('let list := [1, 2, 3]\nlist.');
assert(named.includes('toArray') && named.includes('length') && named.includes('at'), 'a named list offers its methods');

// Hover and signature help use the same receiver, so a chain names its methods there too.
const hoverSource = 'let d := [1, 2, 3].toArray()\n';
const hover = captured.hover.provideHover(
  { getText: range => (range ? hoverSource.slice(range.start, range.end) : hoverSource), offsetAt: position => position, positionAt: offset => offset,
    getWordRangeAtPosition: position => {
      let start = position; let end = position;
      while (start > 0 && /[A-Za-z0-9_]/.test(hoverSource[start - 1])) start -= 1;
      while (end < hoverSource.length && /[A-Za-z0-9_]/.test(hoverSource[end])) end += 1;
      return { start, end };
    } },
  hoverSource.indexOf('toArray') + 2
);
assert(hover && hover.contents.value.includes('toArray() : Array[T]'), 'hover names the chained method');

const signatureSource = 'let d := [1, 2, 3].map(';
const signature = captured.signature.provideSignatureHelp(
  { getText: () => signatureSource, offsetAt: position => position, positionAt: offset => offset },
  signatureSource.length
);
assert(signature && signature.signatures[0].label.includes('map(f'), 'signature help names map on a literal');

console.log('kestrel completion tests pass');

// A trait object offers its trait's method once, and methods stay out of the global list.
const traitSource = 'trait Area {\n    function area() : double\n}\nclass Square {\n    has Area\n    final side : double\n    new(side : double) { this.side = side }\n    public function area() : double { return this.side * this.side }\n}\nfunction biggerArea(s1 : &dyn Area, s2 : &dyn Area) : double {\n    return max(s1.area(), s2.area())\n}\n';
const dynLabels = labelsAfter(traitSource.slice(0, traitSource.indexOf('s2.area()') + 3));
assert.strictEqual(dynLabels.filter(label => label === 'area').length, 1, `one area for a trait object: ${dynLabels.slice(0, 8)}`);
const globalLabels = labelsAfter(traitSource + 'ar');
assert(!globalLabels.includes('area'), 'methods are not global completions');
assert(globalLabels.includes('biggerArea'), 'top-level functions are still global completions');

// The prelude containers are offered as constructors, and a String offers get.
const globals = labelsAfter('let s := St');
for (const name of ['Stack', 'Queue', 'MinHeap', 'MaxHeap']) {
  assert(globals.includes(name), `global completions offer ${name}: ${globals.slice(0, 8)}`);
}
const strings = labelsAfter('let s := "abc".');
assert(strings.includes('get'), `a String offers get: ${strings.slice(0, 8)}`);

// A constructor with explicit type arguments gives the receiver its type, inside a function too.
const bracketed = labelsAfter('let stack := Stack[String]()\nstack.');
for (const name of ['push', 'pop', 'peek', 'size']) {
  assert(bracketed.includes(name), `Stack[String]() offers ${name}: ${bracketed.slice(0, 8)}`);
}
const inFunction = labelsAfter('function f(s : String) : Bool {\n    let stack := Stack[String]()\n    stack.');
assert(inFunction.includes('push'), `a Stack in a function offers push: ${inFunction.slice(0, 8)}`);
const queue = labelsAfter('let q := Queue[Int]()\nq.');
assert(queue.includes('enqueue') && queue.includes('dequeue'), `Queue[Int]() offers enqueue and dequeue: ${queue.slice(0, 8)}`);

// List and Array offer sorted(), and a mutable Array also offers sort().
const sortedList = labelsAfter('let xs := [3, 1, 2]\nxs.');
assert(sortedList.includes('sorted'), `a List offers sorted: ${sortedList.slice(0, 8)}`);
const sortedArray = labelsAfter('let arr := Array(3, 0)\narr.');
assert(sortedArray.includes('sorted') && sortedArray.includes('sort'), `an Array offers sorted and sort: ${sortedArray.slice(0, 8)}`);

// `Tensor.` names a type, so its static constructors are offered, including ones and zerosLike.
const tensorStatics = labelsAfter('let a := Tensor.');
for (const name of ['generate', 'zeros', 'ones', 'filled', 'zerosLike', 'onesLike', 'fromArray']) {
  assert(tensorStatics.includes(name), `Tensor. offers ${name}: ${tensorStatics.slice(0, 12)}`);
}
// A tensor value offers its instance methods.
const tensorMethods = labelsAfter('let z : Tensor[double, (2, 3)] = Tensor.zeros((2, 3))\nlet b := z.');
for (const name of ['copy', 'checkShape', 'broadcastTo', 'transpose']) {
  assert(tensorMethods.includes(name), `a tensor offers ${name}: ${tensorMethods.slice(0, 12)}`);
}

// A tensor made by a generator and used on the next line, as in an editor session, offers transpose.
const generated = labelsAfter('let a := Tensor.generate((3, 2), (x, y) => (x + y).toDouble())\na.');
assert(generated.includes('transpose'), `a generated tensor offers transpose: ${generated.slice(0, 12)}`);
const partial = labelsAfter('let a := Tensor.generate((3, 2), (x, y) => (x + y).toDouble())\na.tra');
assert(partial.includes('transpose'), `a partly typed transpose is still offered: ${partial.slice(0, 12)}`);

console.log('kestrel completion constructor tests pass');

for (const source of [
  'let a := Tensor.ones((2, 3))\na.',
  'let n := 2\nlet a := Tensor.ones((n, 3))\na.',
  'let a := Tensor.ones((2, 3))\na.reshape((3, 2)).',
  'let a := Tensor.ones((2, 3))\na.map(x => x * 0.5).',
  'let a := Tensor.ones((2, 3))\na.sum(axis: 0).',
  'let a := Tensor.ones((2, 3))\na[1, :].'
]) {
  const methods = labelsAfter(source);
  for (const name of ['sum', 'mean', 'min', 'max', 'map', 'reshape', 'rank', 'shape', 'count', 'debug']) {
    assert(methods.includes(name), `${source} offers ${name}: ${methods}`);
  }
}
assert(labelsAfter('let a := Tensor.ones((2, 3))\na.sum(').includes('axis'));
assert(!labelsAfter('let a := Tensor.ones((2, 3))\na.sum(axis: 0').includes('axis'));
assert(labelsAfter('let a := Tensor.generate((2, 3), (i, j) => i.').includes('toDouble'));
assert(labelsAfter('let a := Tensor.ones((2, 3))\na.map(x => x.').includes('sqrt'));

const upperTensor = labelsAfter('let X_c := Tensor([1.0, 2.0], shape = (1, 2))\nX_c.');
for (const name of ['shape', 'broadcastTo', 'reshape', 'mean', 'transpose']) assert(upperTensor.includes(name), name);
const integerTensor = labelsAfter('let X_i := Tensor([0, 0])\nX_i.');
for (const name of ['sum', 'mean', 'min', 'max', 'map', 'reshape', 'rank', 'shape', 'count', 'debug']) {
  assert(integerTensor.includes(name), `Tensor([0, 0]) offers ${name}: ${integerTensor}`);
}
const splitTensor = labelsAfter(`import std.ml.{trainTestSplit}
import std.random.{Rng}
let X := Tensor([0, 0, 1, 1], shape = (2, 2))
let y := Tensor([0, 0])
let rng := Rng.seeded(1)
let (trainingX, testingX, trainingY, testingY) := trainTestSplit(X, y, 1, rng)
trainingX.`);
for (const name of ['sum', 'mean', 'copy', 'transpose', 'reshape']) {
  assert(splitTensor.includes(name), `a destructured trainingX offers ${name}: ${splitTensor}`);
}
assert(labelsAfter('let X_c := Tensor([1.0])\nX_').includes('X_c'));

assert(labelsAfter('let X := Tensor.ones((2, 3))\nX.').includes('divide'));
assert(labelsAfter('let dims := Array([2, 3])\nlet X := Tensor.ones(dims)\nX.').includes('divide'));

assert(!labelsAfter('let X := ').includes('svd'));
const linearAlgebra = labelsAfter('import std.linalg as la\nla.');
for (const name of ['solve', 'svd', 'eigh', 'pinv', 'outer', 'LU']) assert(linearAlgebra.includes(name), name);
const statistics = labelsAfter('import std.stats\nstats.');
for (const name of ['variance', 'stddev', 'covariance']) assert(statistics.includes(name), name);
assert(labelsAfter('import std.random.{Rng}\nRng.').includes('seeded'));
assert(labelsAfter('import std.random.{').includes('seed'));
assert(labelsAfter('import std.random\nrandom.Rng.').includes('seeded'));
assert(labelsAfter('import std.linalg\nlet A := Tensor.ones((3, 2))\nlet factor := linalg.qr(A)\nfactor.').includes('q'));
assert(labelsAfter('import std.linalg\nlinalg.pinv(Tensor.ones((2, 2)), ').includes('rcond'));

// Bare standard module paths complete without the `std.` prefix.
assert(labelsAfter('import ').includes('kesdas'));
assert(labelsAfter('import ').includes('math'));
assert(labelsAfter('import k').includes('kesdas'));
assert(labelsAfter('import kesnet.').includes('optim'));
assert(labelsAfter('import std.kesnet.').includes('optim'));
assert(labelsAfter('import std.kesnet.').includes('Linear'));
assert(labelsAfter('import std.kesnet.').includes('optim'));
assert(labelsAfter('import kesdas.{').includes('DataFrame'));
assert(labelsAfter('import kesdas.{Data').includes('DataFrame'));
assert(labelsAfter('import math.{').includes('max'));
const bareNamespace = labelsAfter('import math\nmath.');
for (const name of ['max', 'sqrt', 'abs']) {
  assert(bareNamespace.includes(name), `bare math namespace offers ${name}: ${bareNamespace.slice(0, 8)}`);
}
const bareFrame = labelsAfter('import kesdas.{DataFrame}\nDataFrame.');
assert(bareFrame.length > 0, 'bare kesdas.DataFrame offers statics');
const chainedFrame = labelsAfter('import kesdas.{DataFrame, Series}\nlet frame := DataFrame([Series([1.0, 2.0], name = "x")])\nframe.');
for (const name of ['shape', 'names', 'select']) {
  assert(chainedFrame.includes(name), `bare DataFrame chain offers ${name}: ${chainedFrame.slice(0, 12)}`);
}

// Import positions complete standard module paths and the names each module exports.
assert(labelsAfter('import ').includes('std'));
assert(labelsAfter('import st').includes('std'));
const stdModules = labelsAfter('import std.');
for (const name of ['collections', 'core', 'linalg', 'math', 'memory', 'prob', 'process', 'random', 'stats', 'str', 'test']) {
  assert(stdModules.includes(name), `std. offers ${name}: ${stdModules}`);
}
assert(labelsAfter('import std.l').includes('linalg'));
const linearAlgebraNames = labelsAfter('import std.linalg.{');
for (const name of ['solve', 'svd', 'LU', 'leastSquares']) {
  assert(linearAlgebraNames.includes(name), `std.linalg. offers ${name}: ${linearAlgebraNames}`);
}
assert(labelsAfter('import std.linalg.{sol').includes('solve'));
assert(!labelsAfter('import std.linalg.{solve, ').includes('solve'));
assert(labelsAfter('import std.random.{').includes('Rng'));
for (const name of ['StandardScaler', 'LinearRegression', 'KNeighborsClassifier', 'KMeans', 'PCA', 'trainTestSplit', 'meanSquaredError', 'accuracy']) {
  assert(labelsAfter('import std.ml.{').includes(name), `std.ml. offers ${name}`);
}
const scalerStatics = labelsAfter('import std.ml.{StandardScaler}\nStandardScaler.');
for (const name of ['fit', 'fitTransform']) assert(scalerStatics.includes(name), `StandardScaler. offers ${name}`);
const modelMethods = labelsAfter('import std.ml.{LinearRegression}\nlet X : DynTensor[double] = Tensor.ones((2, 2))\nlet y : DynTensor[double] = Tensor.ones((2,))\nlet m := LinearRegression.fit(X, y)\nm.');
for (const name of ['coefficients', 'intercept', 'predict']) assert(modelMethods.includes(name), `FittedLinearRegression offers ${name}`);
assert(labelsAfter('import std.prob.{').includes('Normal'));
for (const name of ['Gamma', 'Beta', 'Binomial', 'Poisson', 'NegativeBinomial', 'Hypergeometric']) {
  assert(labelsAfter('import std.prob.{').includes(name), `std.prob. offers ${name}`);
}
const normalStatics = labelsAfter('import std.prob.{Normal}\nNormal.');
for (const name of ['create', 'pdf', 'cdf', 'sample']) {
  assert(normalStatics.includes(name), `Normal. offers ${name}: ${normalStatics}`);
}
assert(labelsAfter('let t := Tensor.ones((2, 2))\nt.').includes('T'));
const operatorTensorMethods = labelsAfter('let t := Tensor.ones((2, 2))\nt.');
for (const name of ['negate', 'power']) {
  assert(operatorTensorMethods.includes(name), `Tensor offers ${name}: ${operatorTensorMethods}`);
}
const operatorComplexMethods = labelsAfter('import std.complex.{Complex}\nlet z := Complex(1.0, 2.0)\nz.');
for (const name of ['negate', 'power']) {
  assert(operatorComplexMethods.includes(name), `Complex offers ${name}: ${operatorComplexMethods}`);
}
const normalMethods = labelsAfter('import std.prob.{Normal}\nlet n := Normal.create(0.0, 1.0)\nn.');
for (const name of ['mean', 'sigma', 'pdf', 'logPdf', 'cdf', 'sample']) {
  assert(normalMethods.includes(name), `Normal object offers ${name}: ${normalMethods}`);
}
assert(labelsAfter('import std.stats.{').includes('correlation'));
assert(labelsAfter('import std.math.{').includes('exp'));
assert(labelsAfter('import std.linalg.{solve}\nsol').includes('solve'));
assert.deepStrictEqual(labelsAfter('import std.linalg as '), []);

// Complex is a standard-library class with fields, methods and static constructors.
assert(stdModules.includes('complex'));
assert(labelsAfter('import std.complex.{').includes('Complex'));
assert(labelsAfter('import std.complex.{C').includes('Complex'));
const complexMethods = labelsAfter('import std.complex.{Complex}\nlet z := Complex(3.0, 4.0)\nz.');
for (const name of ['abs', 'arg', 'conjugate', 'copy', 'im', 'negate', 're', 'text']) {
  assert(complexMethods.includes(name), `Complex. offers ${name}: ${complexMethods}`);
}
const complexStatics = labelsAfter('import std.complex.{Complex}\nComplex.');
for (const name of ['i', 'one', 'zero']) {
  assert(complexStatics.includes(name), `Complex offers ${name}: ${complexStatics}`);
}
assert(labelsAfter('import std.complex\ncomplex.').includes('Complex'));

// std.kesdas: the receiver's own type decides which members are offered.
const kesdasImport = 'import std.kesdas.{Series, DataFrame}\n';
const frameSource = `${kesdasImport}let df := DataFrame([Series([1, 2], name = "v")])\ndf.`;
const frameLabels = labelsAfter(frameSource);
for (const name of ['names', 'index', 'shape', 'col', 'groupBy', 'sortValues', 'assign', 'merge']) {
  assert(frameLabels.includes(name), `a DataFrame offers ${name}: ${frameLabels.slice(0, 12)}`);
}
assert(!frameLabels.includes('upper'), `a DataFrame does not offer StringMethods.upper: ${frameLabels.slice(0, 12)}`);
assert(!frameLabels.includes('push'), `a DataFrame does not offer Stack.push: ${frameLabels.slice(0, 12)}`);

const columnSource = `${kesdasImport}let df := DataFrame([Series([1, 2], name = "v")])\ndf.col("v")?.`;
const columnLabels = labelsAfter(columnSource);
for (const name of ['mean', 'sum', 'head', 'str', 'rolling', 'describe', 'toFrame']) {
  assert(columnLabels.includes(name), `a selected column offers ${name}: ${columnLabels.slice(0, 12)}`);
}
assert(!columnLabels.includes('groupBy'), `a Series does not offer DataFrame.groupBy: ${columnLabels.slice(0, 12)}`);

const scalarSource = `${kesdasImport}let df := DataFrame([Series([1, 2], name = "v")])\ndf.col("v")?.mean().`;
const scalarLabels = labelsAfter(scalarSource);
for (const name of ['asDouble', 'asInt', 'isMissing', 'text']) {
  assert(scalarLabels.includes(name), `a Scalar offers ${name}: ${scalarLabels.slice(0, 12)}`);
}

const groupedSource = `${kesdasImport}let df := DataFrame([Series([1, 2], name = "v")])\ndf.groupBy("v")?.`;
const groupedLabels = labelsAfter(groupedSource);
for (const name of ['sum', 'mean', 'count', 'size', 'transform']) {
  assert(groupedLabels.includes(name), `a GroupBy offers ${name}: ${groupedLabels.slice(0, 12)}`);
}

const stringMethodSource = `${kesdasImport}let s := Series(["a", "b"], name = "v")\ns.str().`;
assert(labelsAfter(stringMethodSource).includes('upper'), 'Series.str() offers upper');
assert(labelsAfter(`${kesdasImport}let s := Series([1, 2], name = "v")\ns.rolling(2).`).includes('mean'), 'Series.rolling offers mean');
assert(labelsAfter(`${kesdasImport}DataFrame.`).includes('parseCsv'), 'DataFrame. offers parseCsv');
assert(labelsAfter('import std.kesdas.{').includes('DataFrame'), 'std.kesdas import offers DataFrame');
assert(labelsAfter(`${kesdasImport}let df := DataFrame([Series([1], name = "v")])\ndf.col(`).includes('name'), 'col( offers its name parameter');

// A namespace import also exposes the module's types, their constructors and
// their static members unqualified; free functions stay qualified.
assert(labelsAfter('import std.kesdas as kd\nDat').includes('DataFrame'), 'a namespace import completes its types');
assert(labelsAfter('import std.kesdas as kd\nDataFrame.').includes('parseCsv'), 'a namespace import exposes static members of its types');
assert(!labelsAfter('import std.kesdas as kd\nrea').includes('readCsv'), 'a namespace import keeps free functions qualified');

// A namespace import offers its exports after the alias, and qualified values keep their type.
const namespaceLabels = labelsAfter('import std.kesdas\nkesdas.');
for (const name of ['Series', 'DataFrame', 'readCsv']) {
  assert(namespaceLabels.includes(name), `kesdas. offers ${name}: ${namespaceLabels.slice(0, 10)}`);
}
const qualifiedFrame = labelsAfter('import std.kesdas as kd\nlet df := kd.DataFrame([kd.Series([1], name = "v")])\ndf.');
assert(qualifiedFrame.includes('col'), `a qualified DataFrame offers col: ${qualifiedFrame.slice(0, 10)}`);

// A type name offers static members only; instance members belong after a value.
const frameStatics = labelsAfter(`${kesdasImport}DataFrame.`);
for (const name of ['empty', 'concat', 'parseCsv']) assert(frameStatics.includes(name), `DataFrame. offers ${name}`);
assert(!frameStatics.includes('col'), `DataFrame. does not offer the instance method col: ${frameStatics.slice(0, 10)}`);
assert(!frameStatics.includes('names'), `DataFrame. does not offer the field names: ${frameStatics.slice(0, 10)}`);
const seriesType = labelsAfter(`${kesdasImport}Series.`);
assert(!seriesType.includes('mean'), `Series. does not offer the instance method mean: ${seriesType.slice(0, 10)}`);

// Signature help and hover use the same kesdas member metadata.
const callSource = `${kesdasImport}let df := DataFrame([Series([1], name = "v")])\ndf.col(`;
const callHelp = captured.signature.provideSignatureHelp(
  { getText: () => callSource, offsetAt: position => position, positionAt: offset => offset },
  callSource.length
);
assert(callHelp && callHelp.signatures.some(signature => signature.label.includes('col(name')), 'signature help names col');
const kesdasHoverSource = `${kesdasImport}let df := DataFrame([Series([1], name = "v")])\ndf.col("v")?.mean()\n`;
const kesdasHover = captured.hover.provideHover(
  { getText: range => (range ? kesdasHoverSource.slice(range.start, range.end) : kesdasHoverSource), offsetAt: position => position, positionAt: offset => offset,
    getWordRangeAtPosition: position => {
      let start = position; let end = position;
      while (start > 0 && /[A-Za-z0-9_]/.test(kesdasHoverSource[start - 1])) start -= 1;
      while (end < kesdasHoverSource.length && /[A-Za-z0-9_]/.test(kesdasHoverSource[end])) end += 1;
      return { start, end };
    } },
  kesdasHoverSource.indexOf('mean') + 2
);
assert(kesdasHover && kesdasHover.contents.value.includes('mean() : Scalar'), 'hover names Series.mean');

// Overload help follows the arity being typed: dropNa() is not dropNa(subset).
const dropCall = `${kesdasImport}let df := DataFrame([Series([1], name = "v")])\ndf.dropNa(`;
const dropHelp = captured.signature.provideSignatureHelp(
  { getText: () => dropCall, offsetAt: position => position, positionAt: offset => offset },
  dropCall.length
);
assert(dropHelp && dropHelp.signatures[dropHelp.activeSignature].label.includes('dropNa()'), 'dropNa() help');

// A frame-returning method keeps the frame's members through `?` and further calls.
const sortedFrame = `${kesdasImport}let df := DataFrame([Series([1, 2], name = "v")])\ndf.sortValues("v", ascending = false)?.`;
for (const name of ['sortValues', 'head', 'col', 'describe', 'groupBy']) {
  assert(labelsAfter(sortedFrame).includes(name), `a sorted frame offers ${name}: ${labelsAfter(sortedFrame).slice(0, 10)}`);
}
const sortedAgain = `${kesdasImport}let df := DataFrame([Series([1, 2], name = "v")])\ndf.sortValues("v")?.head(1).`;
assert(labelsAfter(sortedAgain).includes('sortValues'), `a second frame call keeps DataFrame members: ${labelsAfter(sortedAgain).slice(0, 10)}`);

// `T or Error` is not T: no successful-branch methods until the user writes ?.
const unionLabels = labelsAfter(`${kesdasImport}let df := DataFrame([Series([1], name = "v")])\ndf.col("v").`);
assert(!unionLabels.includes('mean'), `no Series.mean before ?: ${unionLabels.slice(0, 10)}`);
assert(!unionLabels.includes('str'), `no Series.str before ?: ${unionLabels.slice(0, 10)}`);
assert(unionLabels.includes('?'), `a T or Error receiver offers ? to unwrap: ${unionLabels.slice(0, 10)}`);

// A string index is a one-character String, so its methods are offered.
const stringIndex = labelsAfter('let s := "abc"\ns[0].');
assert(stringIndex.includes('toUpper') && stringIndex.includes('length'), `a String index offers String methods: ${stringIndex.slice(0, 8)}`);

// A tuple index keeps the item type, so a String item offers String methods.
const tupleIndex = labelsAfter('let pair := (1, "a")\npair[1].');
assert(tupleIndex.includes('toUpper'), `a tuple String item offers String methods: ${tupleIndex.slice(0, 8)}`);

// The implicit tensor math names keep the tensor type, so the chain stays on Tensor.
for (const name of ['sqrt', 'abs', 'floor', 'ceil', 'round']) {
  const tensorMath = labelsAfter(`let t := Tensor.ones((2, 2))\nlet x := ${name}(t)\nx.`);
  assert(tensorMath.includes('transpose'), `${name}(tensor) offers Tensor methods: ${tensorMath.slice(0, 8)}`);
}

// A subclass offers inherited public fields and methods.
const inherited = labelsAfter('class A {\n  public final x : Int\n  new(x : Int) { this.x = x }\n  public function a() : Int = this.x\n}\nclass B extends A {\n  public final y : Int\n  new(x : Int) { this.x = x\n  this.y = x }\n  public function b() : Int = this.y\n}\nlet b := B(1)\nb.');
for (const name of ['x', 'y', 'a', 'b']) {
  assert(inherited.includes(name), `a subclass offers inherited ${name}: ${inherited.slice(0, 8)}`);
}

// A type name offers enum cases and static methods, but not instance methods.
const enumStatics = labelsAfter('enum Shape {\n  Circle(double)\n  Square(double)\n}\nShape.');
assert(enumStatics.includes('Circle') && enumStatics.includes('Square'), `Shape. offers its cases: ${enumStatics}`);
const staticMethods = labelsAfter('class A {\n  public static function make() : A = A()\n  public function hi() : Int = 1\n}\nA.');
assert(staticMethods.includes('make'), `A. offers its static method: ${staticMethods}`);
assert(!staticMethods.includes('hi'), `A. does not offer its instance method: ${staticMethods}`);

// After `import std.<module>.` the module's names are offered as selective imports.
const mathMembers = labelsAfter('import std.math.');
for (const name of ['PI', 'sqrt', 'exp']) assert(mathMembers.includes(name), `import std.math. offers ${name}: ${mathMembers.slice(0, 8)}`);
const linalgMembers = labelsAfter('import std.linalg.');
for (const name of ['inverse', 'svd', 'solve']) assert(linalgMembers.includes(name), `import std.linalg. offers ${name}: ${linalgMembers.slice(0, 8)}`);
assert(labelsAfter('import std.linalg.in').includes('inverse'));

// Constructor-declared private fields complete on this inside the class, and stay
// hidden on an outside value.
const counterClass = `class Counter {
    public var label : String
    new(start : Int) {
        final this.start : Int := start
        var this.hits : Int := 0
    }
    public function value() : Int = this.start + this.hits
}
`;
const inside = labelsAfter(`${counterClass}let c := Counter(1)\nfunction inClass(x : Counter) : Int {\n    x.`);
for (const name of ['label', 'value']) assert(inside.includes(name), `a Counter offers ${name}: ${inside.slice(0, 8)}`);
assert(!inside.includes('start') && !inside.includes('hits'), `private constructor fields stay hidden outside: ${inside.slice(0, 8)}`);
const thisFields = labelsAfter(`class Counter {
    new(start : Int) {
        final this.start : Int := start
        var this.hits : Int := 0
    }
    public function sum() : Int {
        this.`);
for (const name of ['start', 'hits', 'sum']) assert(thisFields.includes(name), `this. offers ${name}: ${thisFields.slice(0, 8)}`);

// A proof script offers every tactic, including the ones without a short shape.
const tacticLabels = labelsAfter('theorem t : forall n : Nat, n = n by {\n    ');
for (const name of [
  'intro', 'exact', 'apply', 'have', 'cases', 'induction', 'simp', 'rw',
  'constructor', 'left', 'right', 'exists', 'assumption', 'contradiction',
  'exfalso', 'rfl', 'symm', 'trans', 'decide', 'trivial', 'smt', 'pushNeg', 'ext',
  'byContra', 'repeat', 'try', 'skip', 'done', 'sorry'
]) {
  assert(tacticLabels.includes(name), `a proof script offers ${name}: ${tacticLabels.slice(0, 8)}`);
}

// A proof offers the theorems declared above the cursor and the proof's own
// locals, so `exact` completes them by name.
const proofSource = `theorem interSubsetSwap[U](A B : Set[U]) : A ∩ B ⊆ B ∩ A by {
    intro x h
    cases h
    constructor
    exact
}

theorem interComm[U](A B : Set[U]) : A ∩ B = B ∩ A by {
    ext
    exact interSubsetSwap`;
const proofLabels = labelsAfter(proofSource);
for (const name of ['interSubsetSwap', 'interComm', 'A', 'B', 'And', 'OrLeft', 'OrRight', 'Iff', 'Exists', 'True']) {
  assert(proofLabels.includes(name), `the proof offers ${name}: ${proofLabels.slice(0, 8)}`);
}
const introEnd = proofSource.indexOf('exact\n') + 'exact'.length;
const proofNames = labelsAfter(proofSource.slice(0, introEnd));
assert(proofNames.includes('x') && proofNames.includes('h'), `intro names complete: ${proofNames.slice(0, 8)}`);

// Only earlier theorems are usable, so a later declaration stays out of the list.
const laterSource = 'theorem earlier(n : Nat) : n = n by {\n    exact \n}\ntheorem later(n : Nat) : n = n by { rfl }\n';
const earlierLabels = labelsAfter(laterSource, laterSource.indexOf('exact ') + 'exact '.length);
assert(earlierLabels.includes('earlier'), `the proof offers earlier: ${earlierLabels.slice(0, 8)}`);
assert(!earlierLabels.includes('later'), `a later theorem stays hidden: ${earlierLabels.slice(0, 8)}`);

// Theorems are proof facts, not values.
const valueLabels = labelsAfter('theorem t(n : Nat) : n = n by { rfl }\nlet x := ');
assert(!valueLabels.includes('t'), `theorems stay out of value positions: ${valueLabels.slice(0, 8)}`);

// An explicit term proof cites theorems too, so their names complete there.
const explicitProof = 'theorem pair(n : Nat) : n = n and n = n by { constructor; { rfl }; { rfl } }\ntheorem use(n : Nat) : n = n { pair';
const explicitLabels = labelsAfter(explicitProof);
assert(explicitLabels.includes('pair'), `an explicit proof offers pair: ${explicitLabels.slice(0, 8)}`);

// Proof constructors are terms, so they stay out of value positions.
assert(!valueLabels.includes('And'), `constructors stay out of value positions: ${valueLabels.slice(0, 8)}`);
assert(!valueLabels.includes('True'), `constructors stay out of value positions: ${valueLabels.slice(0, 8)}`);

// Inside a constructor call the argument positions hold terms, not tactics.
const constructorArgs = `theorem t[U](x : U, A B : Set[U], h1 : x ∈ A, h2 : x ∈ B) : x ∈ A ∩ B by {\n    exact And(`;
const argumentLabels = labelsAfter(constructorArgs);
assert(argumentLabels.includes('h1') && argumentLabels.includes('h2'), `locals complete inside And(: ${argumentLabels.slice(0, 8)}`);
assert(!argumentLabels.includes('intro'), `tactics stay out of And(: ${argumentLabels.slice(0, 8)}`);
