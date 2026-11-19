'use strict';

// KesNet editor support: Variable chains, module classes, inherited Module
// members, completions, hover and signature help.
const assert = require('assert');
const Module = require('module');
const path = require('path');

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
  InlayHint: class InlayHint { constructor(position, label, kind) { this.position = position; this.label = label; this.kind = kind; } },
  InlayHintKind: { Type: 'Type', Parameter: 'Parameter' },
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
    registerSignatureHelpProvider: (selector, provider) => { captured.signature = provider; return stub(); },
    registerInlayHintsProvider: (selector, provider) => { captured.inlay = provider; return stub(); }
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

const kestrel = require(path.join(__dirname, '..', 'out', 'kestrel'));
const infer = require(path.join(__dirname, '..', 'out', 'infer'));

function labelsAfter(source) {
  const document = {
    getText: () => source,
    offsetAt: position => position,
    positionAt: offset => offset
  };
  const items = captured.completion.provideCompletionItems(document, source.length) || [];
  return items.map(item => item.label);
}

function typesOf(text) {
  return Object.fromEntries(infer.inferredBindings(text, kestrel.parseDocument(text)).map(binding => [binding.name, binding.type || '']));
}

// Variable factories and chains.
const factories = labelsAfter('let x := Variable.');
for (const name of ['zeros', 'ones', 'fromArray', 'fromTensor', 'parameter', 'embedding', 'setTraining', 'liveNodes', 'uniform', 'normal', 'bernoulli']) {
  assert(factories.includes(name), `Variable. offers ${name}`);
}

const methodLabels = labelsAfter('let x := Variable.zeros((2, 3))\nx.');
for (const name of ['relu', 'sigmoid', 'matmul', 'sum', 'mean', 'reshape', 'transpose', 'backward', 'zeroGrad', 'grad', 'data', 'shape', 'conv2d', 'batchNorm', 'crossEntropy']) {
  assert(methodLabels.includes(name), `a Variable offers ${name}`);
}

// Module classes from std.kesnet complete their own and inherited members.
const moduleLabels = labelsAfter('import std.kesnet.{Sequential, Linear}\nlet m := Sequential([box(Linear(2, 3))])\nm.');
for (const name of ['forward', 'parameters', 'parameterCount', 'zeroGrad', 'train', 'eval']) {
  assert(moduleLabels.includes(name), `Sequential. offers ${name}`);
}
const linearLabels = labelsAfter('import std.kesnet.{Linear}\nlet l := Linear(2, 3)\nl.');
for (const name of ['forward', 'weight', 'biasValue', 'parameters', 'zeroGrad']) {
  assert(linearLabels.includes(name), `Linear. offers ${name}`);
}

// A model extending Module inherits its helpers in completion and inference.
const modelSource = `import std.kesnet.{Module}
class Net extends Module {
  public var hidden : Variable
  new() {
    this.registered = Array.empty()
    this.trainingMode = true
    this.hidden = Variable.zeros((2, 2))
  }
  @override public function forward(x : Variable) : Variable { x @ this.hidden }
}
let model := Net()
model.`;
for (const name of ['forward', 'parameters', 'parameterCount', 'zeroGrad', 'summary']) {
  assert(labelsAfter(modelSource).includes(name), `a Module subclass offers ${name}`);
}

const inferred = typesOf(`import std.kesnet.{Linear, ReLU, Sequential, Module}
let x := Variable.zeros((2, 3))
let linear := Linear(3, 4)
let y := linear.forward(x)
let activated := y.relu()
let total := activated.sum()
let shape := total.shape()
let gradient := total.grad()
let data := total.data()
let count := total.count()
let model := Sequential([box(Linear(2, 3))])
let params := model.parameters()
let rate := Variable.isTraining()
`);
assert.strictEqual(inferred.x, 'Variable');
assert.strictEqual(inferred.linear, 'Linear');
assert.strictEqual(inferred.y, 'Variable');
assert.strictEqual(inferred.activated, 'Variable');
assert.strictEqual(inferred.total, 'Variable');
assert.strictEqual(inferred.shape, 'Array[Int]');
assert.strictEqual(inferred.gradient, 'Option[DynTensor[double]]');
assert.strictEqual(inferred.data, 'DynTensor[double]');
assert.strictEqual(inferred.count, 'Int');
assert.strictEqual(inferred.model, 'Sequential');
assert.strictEqual(inferred.params, 'List[Variable]');
assert.strictEqual(inferred.rate, 'Bool');

// Operators on Variables stay Variables.
const operators = typesOf('let a := Variable.zeros((2, 3))\nlet b := Variable.ones((2, 3))\nlet c := a * b + 1.0\nlet d := a @ b.transpose(0, 1)\nlet e := a ** 2.0\n');
assert.strictEqual(operators.c, 'Variable');
assert.strictEqual(operators.d, 'Variable');
assert.strictEqual(operators.e, 'Variable');

// Signature help and hover see the metadata.
const call = 'let x := Variable.zeros(';
const help = captured.signature.provideSignatureHelp(
  { getText: () => call, offsetAt: position => position, positionAt: offset => offset },
  call.length
);
assert(help && help.signatures[help.activeSignature].label.includes('shape'), 'Variable.zeros signature help');

const hoverSource = 'import std.kesnet.{Linear}\nlet l := Linear(2, 3)\n';
const hover = captured.hover.provideHover(
  {
    getText: range => (range ? hoverSource.slice(range.start, range.end) : hoverSource),
    offsetAt: position => position,
    positionAt: offset => offset,
    getWordRangeAtPosition: position => {
      let start = position; let end = position;
      while (start > 0 && /[A-Za-z0-9_]/.test(hoverSource[start - 1])) start -= 1;
      while (end < hoverSource.length && /[A-Za-z0-9_]/.test(hoverSource[end])) end += 1;
      return { start, end };
    }
  },
  hoverSource.indexOf('Linear') + 2
);
assert(hover && hover.contents.value.includes('Linear'), 'Linear hover names its constructor');

// Imports complete the kesnet module names.
// Inlay hints expose the inferred types and named parameters of KesNet calls.
const hintSource = `import std.kesnet.{Linear}
let x := Variable.zeros((2, 3))
let z := Variable.zeros((3,))
let layer := Linear(3, 4)
let y := layer.forward(z)
let params := layer.parameters()
`;
const hintDocument = {
  getText: () => hintSource,
  offsetAt: position => position,
  positionAt: offset => offset,
  uri: { toString: () => 'file:///kesnet-inlay.kes' },
  languageId: 'kestrel'
};
const hints = captured.inlay.provideInlayHints(hintDocument, { start: 0, end: hintSource.length });
const typeHints = hints.filter(hint => hint.kind === 'Type').map(hint => hint.label);
for (const expected of [': Variable', ': Linear', ': List[Variable]']) {
  assert(typeHints.includes(expected), `inlay hints include ${expected}: ${typeHints}`);
}
const parameterHints = hints.filter(hint => hint.kind === 'Parameter').map(hint => hint.label);
for (const expected of ['inFeatures =', 'outFeatures =', 'x =']) {
  assert(parameterHints.includes(expected), `parameter hints include ${expected}: ${parameterHints}`);
}

const moduleImport = 'import std.kesnet.';
assert(labelsAfter(moduleImport).includes('Linear'), 'std.kesnet names its classes');
const selective = 'import std.kesnet.{';
assert(labelsAfter(selective).includes('Sequential'), 'selective import offers Sequential');

console.log('kestrel kesnet editor tests pass');
