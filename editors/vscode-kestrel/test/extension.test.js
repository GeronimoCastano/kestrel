'use strict';

const assert = require('assert');
const Module = require('module');
const path = require('path');

class CompletionItem {
  constructor(label, kind) {
    this.label = label;
    this.kind = kind;
    this.detail = '';
    this.documentation = undefined;
    this.insertText = label;
    this.filterText = '';
    this.sortText = '';
  }
}
class CompletionItemKind {}
for (const name of [
  'Function', 'Class', 'Interface', 'Enum', 'Constant', 'Variable', 'Property',
  'Keyword', 'Snippet', 'TypeParameter', 'Method', 'Field'
]) CompletionItemKind[name] = name;
class SnippetString { constructor(value) { this.value = value; } }
class MarkdownString { constructor(value) { this.value = value; } }
class SignatureHelp { constructor() { this.signatures = []; this.activeSignature = 0; this.activeParameter = 0; } }
class SignatureInformation {
  constructor(label, documentation) { this.label = label; this.documentation = documentation; this.parameters = []; }
}
class ParameterInformation {
  constructor(label, documentation) { this.label = label; this.documentation = documentation; }
}
class Hover { constructor(contents, range) { this.contents = contents; this.range = range; } }
class Location { constructor(uri, range) { this.uri = uri; this.range = range; } }
const Uri = { file(value) { return { scheme: 'file', fsPath: value, path: value, toString: () => `file://${value}` }; } };
class InlayHint {
  constructor(position, label, kind) {
    this.position = position; this.label = label; this.kind = kind;
    this.paddingLeft = false; this.paddingRight = false;
  }
}
const InlayHintKind = { Type: 'Type', Parameter: 'Parameter' };
class Range { constructor(start, end) { this.start = start; this.end = end; } }
class Position { constructor(line, character) { this.line = line; this.character = character; } }
class DocumentSymbol {
  constructor(name, detail, kind, range, selectionRange) {
    this.name = name; this.detail = detail; this.kind = kind;
    this.range = range; this.selectionRange = selectionRange;
  }
}
const SymbolKind = { Class: 'Class', Interface: 'Interface', Enum: 'Enum', Function: 'Function', Constant: 'Constant' };
class SemanticTokensLegend {
  constructor(tokenTypes, tokenModifiers) { this.tokenTypes = tokenTypes; this.tokenModifiers = tokenModifiers; }
}
class SemanticTokensBuilder {
  constructor(legend) { this.legend = legend; this.data = []; }
  push(line, character, length, type, modifiers) { this.data.push([line, character, length, type, modifiers]); }
  build() { return this.data; }
}
const registered = {};
// Settings the tests set, read through workspace.getConfiguration like the editor reads them.
const settings = {};
class EventEmitter {
  constructor() { this.listeners = []; this.event = listener => { this.listeners.push(listener); return { dispose() {} }; }; }
  fire(value) { for (const listener of this.listeners) listener(value); }
  dispose() {}
}
const vscode = {
  EventEmitter,
  ConfigurationTarget: { Global: 1 },
  CompletionItem, CompletionItemKind, SnippetString, MarkdownString,
  SignatureHelp, SignatureInformation, ParameterInformation, Hover, Location, Uri,
  Range, Position, DocumentSymbol, SymbolKind, InlayHint, InlayHintKind,
  SemanticTokensLegend, SemanticTokensBuilder,
  Diagnostic: class Diagnostic { constructor(range, message, severity) { this.range = range; this.message = message; this.severity = severity; } },
  DiagnosticSeverity: { Error: 0, Warning: 1 },
  window: {
    activeTextEditor: undefined,
    createOutputChannel() { return { appendLine() {}, dispose() {} }; },
    onDidChangeTextEditorSelection() { return { dispose() {} }; },
    setStatusBarMessage() { return { dispose() {} }; }
  },
  commands: { registerCommand() { return { dispose() {} }; } },
  workspace: {
    getConfiguration() {
      return {
        get(key) { return settings[key]; },
        update(key, value) { settings[key] = value; return Promise.resolve(); }
      };
    },
    onDidChangeConfiguration() { return { dispose() {} }; },
    onDidSaveTextDocument() { return { dispose() {} }; },
    getWorkspaceFolder() { return undefined; },
    openTextDocument() { return Promise.resolve(document); },
    textDocuments: [],
    onDidOpenTextDocument() { return { dispose() {} }; },
    onDidChangeTextDocument(callback) { registered.change = callback; return { dispose() {} }; },
    onDidCloseTextDocument() { return { dispose() {} }; }
  },
  languages: {
    createDiagnosticCollection() { return { set() {}, delete() {}, dispose() {} }; },
    registerCompletionItemProvider(selector, provider) { registered.completion = provider; return {}; },
    registerSignatureHelpProvider(selector, provider) { registered.signature = provider; return {}; },
    registerHoverProvider(selector, provider) { registered.hover = provider; return {}; },
    registerDefinitionProvider(selector, provider) { registered.definition = provider; return {}; },
    registerDocumentSymbolProvider(selector, provider) { registered.symbols = provider; return {}; },
    registerInlayHintsProvider(selector, provider) { registered.inlay = provider; return {}; },
    registerDocumentSemanticTokensProvider(selector, provider, legend) { registered.semantic = provider; registered.legend = legend; return {}; },
    registerCodeActionsProvider(selector, provider) { registered.codeActions = provider; return {}; }
  }
};
const load = Module._load;
Module._load = function (request, parent, isMain) {
  if (request === 'vscode') return vscode;
  return load.call(this, request, parent, isMain);
};

const extension = require(path.join(__dirname, '..', 'out', 'extension.js'));
extension.activate({ subscriptions: { push() {} } });
assert(registered.completion && registered.signature && registered.hover && registered.definition && registered.symbols && registered.inlay);

const source = `function greet(name : String) : void with Effects {
    print(f"Hello {name}")
}

greet("Geronimo")
`;
const document = {
  getText: range => {
    if (!range) return source;
    return source.slice(range.start, range.end);
  },
  offsetAt: position => position,
  positionAt: offset => offset,
  getWordRangeAtPosition: position => {
    let start = position;
    let end = position;
    while (start > 0 && /[A-Za-z0-9_]/.test(source[start - 1])) start -= 1;
    while (end < source.length && /[A-Za-z0-9_]/.test(source[end])) end += 1;
    return { start, end };
  }
};
const callOffset = source.indexOf('greet("Geronimo")') + 'greet('.length;
const completionItems = registered.completion.provideCompletionItems(document, callOffset);
const named = completionItems.find(item => item.label === 'name');
assert(named, 'named argument completion');
assert.strictEqual(named.detail, 'String');
assert.strictEqual(named.insertText.value, 'name = $0');

const signature = registered.signature.provideSignatureHelp(document, callOffset);
assert(signature);
assert.strictEqual(signature.signatures[0].label, 'greet(name : String) : void with Effects');
assert.strictEqual(signature.activeParameter, 0);

const declarationOffset = source.indexOf('function greet(') + 'function greet('.length;
const declarationItems = registered.completion.provideCompletionItems(document, declarationOffset);
assert(declarationItems.some(item => item.label === 'parameter'));
assert.strictEqual(registered.signature.provideSignatureHelp(document, declarationOffset), null);

const printOffset = source.indexOf('print(') + 'print('.length;
const printSignature = registered.signature.provideSignatureHelp(document, printOffset);
assert(printSignature);
assert.strictEqual(printSignature.signatures[0].label, 'print[T : Text](value : T) : void with Effects');

const printHover = registered.hover.provideHover(document, source.indexOf('print') + 2);
assert(printHover);
assert(printHover.contents.value.includes('print[T : Text](value : T) : void with Effects'));

const printItems = registered.completion.provideCompletionItems(document, source.indexOf('print') + 'print'.length);
assert(printItems.some(item => item.label === 'print'));

const shapeSource = `class Shape {
    private var area : double
    new(area : double) { this.area = area }
    public function text() : String = "shape"
    public function getArea() : double { return this.area }
}
let shape := Shape(1.0)
shape.
`;
const shapeDocument = {
  getText: range => range ? shapeSource.slice(range.start, range.end) : shapeSource,
  offsetAt: position => position,
  positionAt: offset => offset
};
const thisItems = registered.completion.provideCompletionItems(
  shapeDocument,
  shapeSource.indexOf('return this.') + 'return this.'.length
);
assert(thisItems.some(item => item.label === 'area' && item.kind === 'Field'));
assert(thisItems.some(item => item.label === 'getArea' && item.kind === 'Method'));
assert(!thisItems.some(item => item.label === 'add'));

const outsideItems = registered.completion.provideCompletionItems(shapeDocument, shapeSource.lastIndexOf('shape.') + 'shape.'.length);
assert(outsideItems.some(item => item.label === 'getArea'));
assert(!outsideItems.some(item => item.label === 'area'));

const methodSource = `let xs := [1, 2, 3]\nxs.length\n`;
const methodDocument = {
  getText: range => range ? methodSource.slice(range.start, range.end) : methodSource,
  offsetAt: position => position,
  positionAt: offset => offset,
  getWordRangeAtPosition: position => {
    let start = position;
    let end = position;
    while (start > 0 && /[A-Za-z0-9_]/.test(methodSource[start - 1])) start -= 1;
    while (end < methodSource.length && /[A-Za-z0-9_]/.test(methodSource[end])) end += 1;
    return { start, end };
  }
};
const methodItems = registered.completion.provideCompletionItems(methodDocument, methodSource.lastIndexOf('length') + 'length'.length);
assert(methodItems.some(item => item.label === 'length' && item.kind === 'Method'));
assert(!methodItems.some(item => item.label === 'add' && item.kind === 'Method'));
assert(!methodItems.some(item => item.label === 'print'));

const signatureSource = `let xs := [1, 2, 3]\nxs.length(\n`;
const signatureDocument = {
  getText: range => range ? signatureSource.slice(range.start, range.end) : signatureSource,
  offsetAt: position => position,
  positionAt: offset => offset
};
const lengthSignature = registered.signature.provideSignatureHelp(
  signatureDocument,
  signatureSource.indexOf('xs.length(') + 'xs.length('.length
);
assert(lengthSignature);
assert.strictEqual(lengthSignature.signatures[0].label, 'List[T].length() : Int');

const lengthHover = registered.hover.provideHover(
  methodDocument,
  methodSource.indexOf('xs.length') + 'xs.'.length
);
assert(lengthHover);
assert(lengthHover.contents.value.includes('List[T].length() : Int'));
assert(!lengthHover.contents.value.includes('Set[T].add'));

const listSource = `let list := [1, 2, 3]\nprint(list)\n`;
const listDocument = {
  getText: range => range ? listSource.slice(range.start, range.end) : listSource,
  offsetAt: position => position,
  positionAt: offset => offset,
  getWordRangeAtPosition: position => {
    let start = position;
    let end = position;
    while (start > 0 && /[A-Za-z0-9_]/.test(listSource[start - 1])) start -= 1;
    while (end < listSource.length && /[A-Za-z0-9_]/.test(listSource[end])) end += 1;
    return { start, end };
  }
};
const listHints = registered.inlay.provideInlayHints(listDocument, { start: 0, end: listSource.length });
assert(listHints.some(hint => hint.label === ': List[Int]' && hint.kind === 'Type'));
// print names its value, so its argument gets the `value =` hint.
assert(listHints.some(hint => hint.label === 'value =' && hint.kind === 'Parameter'));
const listHover = registered.hover.provideHover(listDocument, listSource.indexOf('list') + 2);
assert(listHover);
assert(listHover.contents.value.includes('list : List[Int]'));

const parameterHints = registered.inlay.provideInlayHints(document, { start: 0, end: source.length });
assert(parameterHints.some(hint => hint.label === 'name =' && hint.kind === 'Parameter'));
assert(parameterHints.some(hint => hint.label === 'value =' && hint.kind === 'Parameter'));

// range has overloads by count: range(2, n) names start and stop, and range(4) names only stop.
const rangeSource = 'for (k in range(2, n)) {}\nfor (i in range(4)) {}\n';
const rangeDocument = {
  getText: range => range ? rangeSource.slice(range.start, range.end) : rangeSource,
  offsetAt: position => position,
  positionAt: offset => offset,
  getWordRangeAtPosition: () => undefined
};
const rangeHints = registered.inlay.provideInlayHints(rangeDocument, { start: 0, end: rangeSource.length });
assert.strictEqual(rangeHints.filter(hint => hint.label === 'start =').length, 1);
assert.strictEqual(rangeHints.filter(hint => hint.label === 'stop =').length, 2);

// Array.generate is a static member: it gets hover and signature help, and no free-function completion.
const generateSource = 'let array := Array.generate(10, x : Int => x*2)\n';
const generateDocument = {
  getText: range => range ? generateSource.slice(range.start, range.end) : generateSource,
  offsetAt: position => position,
  positionAt: offset => offset,
  getWordRangeAtPosition: position => {
    let start = position;
    let end = position;
    while (start > 0 && /[A-Za-z0-9_]/.test(generateSource[start - 1])) start -= 1;
    while (end < generateSource.length && /[A-Za-z0-9_]/.test(generateSource[end])) end += 1;
    return { start, end };
  }
};
const generateHover = registered.hover.provideHover(generateDocument, generateSource.indexOf('generate') + 2);
assert(generateHover);
assert(generateHover.contents.value.includes('Array.generate(size : Int, factory : Int -> T) : Array[T]'));
const generateSignature = registered.signature.provideSignatureHelp(
  generateDocument,
  generateSource.indexOf('generate(') + 'generate('.length
);
assert(generateSignature);
assert.strictEqual(generateSignature.signatures[0].label, 'Array.generate(size : Int, factory : Int -> T) : Array[T]');
const generateHints = registered.inlay.provideInlayHints(generateDocument, { start: 0, end: generateSource.length });
assert(generateHints.some(hint => hint.label === ': Array[Int]' && hint.kind === 'Type'));
const generateItems = registered.completion.provideCompletionItems(generateDocument, generateSource.length);
assert(!generateItems.some(item => item.label === 'generate'));

// Parameter uses are colored like their declaration, for lambdas and function parameters.
const semanticSource = 'let array := Array.generate(10, x => x*2)\nfunction f(n : Int) : Int { return n + 1 }\n';
const semanticDocument = {
  getText: () => semanticSource,
  positionAt: offset => {
    const before = semanticSource.slice(0, offset).split('\n');
    return { line: before.length - 1, character: before[before.length - 1].length };
  }
};
const semanticTokens = registered.semantic.provideDocumentSemanticTokens(semanticDocument);
assert.deepStrictEqual(registered.legend.tokenTypes, ['parameter', 'variable']);
assert.strictEqual(semanticTokens.length, 4);
assert.deepStrictEqual(semanticTokens.map(token => token[2]), [1, 1, 1, 1]);
assert.deepStrictEqual(semanticTokens.map(token => token[0]), [0, 0, 1, 1]);

console.log('kestrel extension glue tests pass');

// Hints for a source text, with the given settings. Positions are offsets, except that a line
// number is available for the cursor-line setting.
function hintsFor(source, options = {}) {
  for (const key of Object.keys(settings)) delete settings[key];
  Object.assign(settings, options.settings || {});
  const lineOf = offset => source.slice(0, offset).split('\n').length - 1;
  const document = {
    uri: { toString: () => 'file:///test.kes' },
    languageId: 'kestrel',
    getText: () => source,
    offsetAt: offset => offset,
    positionAt: offset => ({ line: lineOf(offset), character: 0, offset })
  };
  vscode.window.activeTextEditor = options.cursorLine === undefined
    ? undefined
    : { document, selection: { active: { line: options.cursorLine } } };
  const hints = registered.inlay.provideInlayHints(document, { start: 0, end: source.length });
  vscode.window.activeTextEditor = undefined;
  return hints;
}

const showSource = 'function show(value : Int) : Int = value\nlet value := 2\nshow(value)\nshow(3)\n';
assert(!hintsFor(showSource).some(hint => hint.label === 'value =' && hint.position.offset === showSource.indexOf('value)')));
assert(hintsFor(showSource).some(hint => hint.label === 'value ='));
assert(hintsFor(showSource, { settings: { 'inlayHints.skipMatchingNames': false } }).some(hint => hint.label === 'value =' && hint.position.offset === showSource.indexOf('value)')));
assert(!hintsFor(showSource, { settings: { 'inlayHints.parameterNames': 'off' } }).some(hint => hint.kind === 'Parameter'));
assert(hintsFor(showSource, { settings: { 'inlayHints.parameterNameStyle': 'colon' } }).some(hint => hint.label === 'value:'));
assert(!hintsFor(showSource, { settings: { 'inlayHints.minimumArguments': 2 } }).some(hint => hint.kind === 'Parameter'));
assert(!hintsFor(showSource, { settings: { 'inlayHints.types': false } }).some(hint => hint.kind === 'Type'));

// On the cursor-line setting, only the call on the cursor's line keeps its name hint.
const cursorSource = 'function show(value : Int) : Int = value\nshow(3)\nshow(4)\n';
const cursorHints = hintsFor(cursorSource, { settings: { 'inlayHints.parameterNames': 'cursorLine' }, cursorLine: 2 });
assert.strictEqual(cursorHints.filter(hint => hint.kind === 'Parameter').length, 1);
assert.strictEqual(cursorHints.find(hint => hint.kind === 'Parameter').position.line, 2);
assert.strictEqual(hintsFor(cursorSource, { settings: { 'inlayHints.parameterNames': 'cursorLine' } }).filter(hint => hint.kind === 'Parameter').length, 0);

// A generic class's constructor is typed with its type arguments.
const genericSource = 'class Node[T] {\n    public var value : T\n    new(move value : T) { this.value = value }\n}\nlet tree := Node(3)\n';
assert(hintsFor(genericSource).some(hint => hint.kind === 'Type' && hint.label === ': Node[Int]'));

// A call split across lines: the hint sits before its argument's first token, on that token's line.
const splitSource = 'function pair(a : Int, b : Int) : Int = a\npair(4,\n    5)\n';
const splitHints = hintsFor(splitSource, { settings: { 'inlayHints.parameterNames': 'cursorLine' }, cursorLine: 2 });
const secondHint = splitHints.find(hint => hint.label === 'b =');
assert(secondHint, 'the split argument keeps its name hint');
assert.strictEqual(secondHint.position.offset, splitSource.indexOf('5)'));
assert.strictEqual(secondHint.position.line, 2);

// print takes a named value, so its argument gets the name hint like any other call.
const printHintsSource = 'print(3)\n';
assert(hintsFor(printHintsSource, { settings: { 'inlayHints.parameterNames': 'cursorLine' }, cursorLine: 0 })
  .some(hint => hint.label === 'value ='));
assert(!hintsFor(printHintsSource, { settings: { 'inlayHints.parameterNames': 'cursorLine' }, cursorLine: 1 })
  .some(hint => hint.kind === 'Parameter'));

const tensorSource = 'let tensor := Tensor.generate((2, 3), (i, j) => (i + j).toDouble())\n';
const tensorDocument = {
  getText: range => range ? tensorSource.slice(range.start, range.end) : tensorSource,
  offsetAt: position => position,
  positionAt: offset => offset,
  getWordRangeAtPosition: position => {
    let start = position;
    let end = position;
    while (start > 0 && /[A-Za-z0-9_]/.test(tensorSource[start - 1])) start -= 1;
    while (end < tensorSource.length && /[A-Za-z0-9_]/.test(tensorSource[end])) end += 1;
    return { start, end };
  }
};
const tensorSignature = registered.signature.provideSignatureHelp(tensorDocument, tensorSource.indexOf('generate(') + 'generate('.length);
assert.strictEqual(tensorSignature.signatures.length, 1);
assert(tensorSignature.signatures[0].label.startsWith('Tensor.generate('));
const tensorHover = registered.hover.provideHover(tensorDocument, tensorSource.indexOf('generate') + 2);
assert(tensorHover.contents.value.includes('Tensor.generate('));
assert(!tensorHover.contents.value.includes('Array.generate('));

// Verify the displayed inlay hints, not just the inference helper's return values.
for (const [expression, expected] of [
  ['t.sum()', 'double'],
  ['t.mean(axis: 0)', 'Tensor[double, (3,)]'],
  ['t.min(axis: 1)', 'Tensor[double, (2,)]'],
  ['t.max()', 'double'],
  ['t.map(x => x * 0.5)', 'Tensor[double, (2, 3)]'],
  ['t.reshape((3, 2))', 'Tensor[double, (3, 2)]'],
  ['t.rank()', 'Int'], ['t.shape()', 'Array[Int]'], ['t.count()', 'Int'],
  ['t.debug()', 'String'], ['t[0, :]', 'Tensor[double, (3,)]'],
  ['t + 2.0', 'Tensor[double, (2, 3)]']
]) {
  const source = `let t := Tensor.ones((2, 3))\nlet result := ${expression}\n`;
  assert(hintsFor(source).some(hint => hint.kind === 'Type' && hint.label === `: ${expected}`
    && hint.position.offset === source.indexOf('result') + 'result'.length), source);
}
const axisSource = 'let t := Tensor.ones((2, 3))\nt.reshape((3, 2)).sum(axis: ';
const axisDocument = { getText: () => axisSource, offsetAt: value => value, positionAt: value => value };
const axisHelp = registered.signature.provideSignatureHelp(axisDocument, axisSource.length);
assert(axisHelp.signatures[axisHelp.activeSignature].label.includes('axis : Int'));
assert(axisHelp.signatures[axisHelp.activeSignature].label.startsWith('Tensor['));

for (const [expression, expected] of [
  ['Tensor([0, 0])', 'Tensor[double, (2,)]'],
  ['Tensor([0, 0], shape = (1, 2))', 'Tensor[double, (1, 2)]'],
  ['Tensor([1.0, 2.0], shape = (1, 2))', 'Tensor[double, (1, 2)]'],
  ['Tensor([1.0, 2.0]).reshape((1, 2))', 'Tensor[double, (1, 2)]'],
  ['Array([1, 2])', 'Array[Int]'], ['double(3)', 'double'], ['String(3)', 'String']
]) {
  const source = `let X_c := ${expression}\n`;
  assert(hintsFor(source).some(hint => hint.kind === 'Type' && hint.label === `: ${expected}`
    && hint.position.offset === source.indexOf('X_c') + 3), source);
}
const tupleHintSource = 'let (whole, part) := (1, 2.0)\n';
const tupleHints = hintsFor(tupleHintSource);
assert(tupleHints.some(hint => hint.kind === 'Type' && hint.label === ': Int'
  && hint.position.offset === tupleHintSource.indexOf('whole') + 'whole'.length), 'whole is Int');
assert(tupleHints.some(hint => hint.kind === 'Type' && hint.label === ': double'
  && hint.position.offset === tupleHintSource.indexOf('part') + 'part'.length), 'part is double');
for (const [source, expected] of [
  ['Tensor(', 'Tensor(values :'], ['Tensor([1.0], ', 'shape :'],
  ['Array(', 'Array(values :'], ['Array(3, ', 'Array(size :'],
  ['double(', 'double(value :'], ['String(', 'String(value :']
]) {
  const doc = { getText: () => source, offsetAt: value => value, positionAt: value => value };
  const help = registered.signature.provideSignatureHelp(doc, source.length);
  assert(help && help.signatures[help.activeSignature].label.includes(expected), source);
}
assert(hintsFor('let X_c := Tensor([1.0], shape = (1,))\n').some(hint => hint.label === 'values ='));

const dividedSource = 'let X := Tensor.ones((2, 3))\nlet result := X / 2.0\n';
assert(hintsFor(dividedSource).some(hint => hint.kind === 'Type' && hint.label === ': Tensor[double, (2, 3)]'
  && hint.position.offset === dividedSource.indexOf('result') + 'result'.length));
const divisionCall = 'let X := Tensor.ones((2, 3))\nX.divide(';
const divisionDoc = { getText: () => divisionCall, offsetAt: value => value, positionAt: value => value };
const divisionHelp = registered.signature.provideSignatureHelp(divisionDoc, divisionCall.length);
assert(divisionHelp.signatures[divisionHelp.activeSignature].label.includes('.divide(other : double)'));

// Verify the semantic tokens sent to VS Code, including parameter priority.
const upperColors = 'let X := Tensor.ones((2, 3))\nlet X_c := X / PI\nfunction head(X : Int) : Int = X\n';
const colorDoc = {
  getText: () => upperColors,
  positionAt: offset => {
    const lines = upperColors.slice(0, offset).split('\n');
    return { line: lines.length - 1, character: lines.at(-1).length };
  }
};
const colorTokens = registered.semantic.provideDocumentSemanticTokens(colorDoc);
const colorNames = colorTokens.map(([line, column, length, type]) => [
  upperColors.split('\n')[line].slice(column, column + length), registered.legend.tokenTypes[type]
]);
assert.deepStrictEqual(colorNames, [
  ['X', 'variable'], ['X_c', 'variable'], ['X', 'variable'], ['PI', 'variable'],
  ['X', 'parameter'], ['X', 'parameter']
]);
for (let i = 1; i < colorTokens.length; i += 1) {
  const [line, column] = colorTokens[i];
  const previous = colorTokens[i - 1];
  assert(line > previous[0] || line === previous[0] && column >= previous[1] + previous[2]);
}

for (const [expression, expected] of [
  ['A @ Tensor.ones((2,))', 'Tensor[double, (3,)]'],
  ['linalg.pinv(A)', 'Tensor[double, (2, 3)]'],
  ['math.exp(A)', 'Tensor[double, (3, 2)]'],
  ['A.norm()', 'double'],
  ['stats.variance(A, axis = 0)', 'Tensor[double, (2,)]']
]) {
  const source = `import std.linalg\nimport std.math\nimport std.stats\nlet A := Tensor.ones((3, 2))\nlet result := ${expression}\n`;
  assert(hintsFor(source).some(hint => hint.kind === 'Type' && hint.label === `: ${expected}`
    && hint.position.offset === source.indexOf('result') + 6), source);
}
const solveSource = 'import std.linalg\nlinalg.solve(';
const solveDoc = { getText: () => solveSource, offsetAt: n => n, positionAt: n => n };
assert(registered.signature.provideSignatureHelp(solveDoc, solveSource.length).signatures[0].label.includes('std.linalg.solve'));
