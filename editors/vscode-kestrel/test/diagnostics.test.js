'use strict';

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const Module = require('module');
const check = require('../out/check');

const BINARY = path.join(__dirname, '..', '..', '..', 'target', 'release', 'kestrel');
const haveBinary = fs.existsSync(BINARY);

// Output of the checker, captured from the release binary.
const typeErrors = [
  '-:1:18: error[E_TYPE]: Expected String, found Int in `1`.',
  '  1 | let a : String = 1',
  '    |                  ^',
  '  hint: Use a value of the expected type; number conversions must be explicit.',
  '-:3:22: error[E_TYPE]: Expected String, found Array[Int] in `array`.',
  '  3 | let wrong : String = array',
  '    |                      ^^^^^',
  '  hint: Use a value of the expected type; number conversions must be explicit.',
  ''
].join('\n');
const parseError = [
  '-:2:1: error[E_EXPRESSION]: Expected a value or expression, found ``.',
  '  2 | ',
  '    | ^',
  '  hint: Put a value, name or expression here.',
  ''
].join('\n');

const parsed = check.parseCheckOutput(typeErrors);
assert.deepStrictEqual(parsed.map(item => [item.line, item.column, item.code, item.length]), [
  [1, 18, 'E_TYPE', 1],
  [3, 22, 'E_TYPE', 5]
]);
assert.strictEqual(parsed[0].message, 'Expected String, found Int in `1`.');
assert.strictEqual(parsed[0].severity, 'error');
assert.strictEqual(parsed[0].hint, 'Use a value of the expected type; number conversions must be explicit.');

const parsedParse = check.parseCheckOutput(parseError);
assert.strictEqual(parsedParse.length, 1);
assert.strictEqual(parsedParse[0].code, 'E_EXPRESSION');
assert.strictEqual(parsedParse[0].length, 1);

// A clean check prints a success line, which is not an error.
assert.deepStrictEqual(check.parseCheckOutput('-: checked names and supported types\n'), []);

// Errors for other files are not read as this document's errors.
assert.deepStrictEqual(check.parseCheckOutput('other.kes:1:1: error[E_TYPE]: nope\n'), []);
// A notebook cell is checked under the internal session name.
assert.strictEqual(check.parseCheckOutput('<session>:1:1: error[E_TYPE]: nope\n')[0].code, 'E_TYPE');

// The runner gives the real checker the editor's text and returns its errors.
function runChecker(compiler, text, notebook = false) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'kestrel-check-'));
  return check.startCheck(compiler, root, text, 10000, notebook).promise;
}

const notes = [];
const logged = [];

async function integration() {
  if (!haveBinary) {
    notes.push('skipped the checker runs: no release binary at ' + BINARY);
    return;
  }
  const result = await runChecker(BINARY, 'let a : String = 1\n');
  assert.strictEqual(result.code, 1);
  assert.strictEqual(check.parseCheckOutput(result.output)[0].code, 'E_TYPE');

  const clean = await runChecker(BINARY, 'let ok : String = "fine"\n');
  assert.strictEqual(clean.code, 0);
  assert.deepStrictEqual(check.parseCheckOutput(clean.output), []);

  // A script and a notebook cell both allow a top-level ? at compile time; the
  // script ends the run on an Error, which `kestrel run` reports separately.
  const cellText = 'let n := parseInt("3")?\n';
  const script = await runChecker(BINARY, cellText);
  assert.strictEqual(script.code, 0, script.output);
  assert.deepStrictEqual(check.parseCheckOutput(script.output), []);
  const cell = await runChecker(BINARY, cellText, true);
  assert.strictEqual(cell.code, 0, cell.output);
  assert.deepStrictEqual(check.parseCheckOutput(cell.output), []);

  // The extension turns each change into diagnostics for the document, and drops old ones on close.
  await extensionWiring();
}

// Loads the extension with a stand-in vscode module that records what the diagnostics do.
function extensionWiring() {
  const sets = new Map();
  const collection = {
    set(uri, items) { sets.set(uri.toString(), items); },
    delete(uri) { sets.delete(uri.toString()); },
    dispose() {}
  };
  let changed = null;
  let closed = null;
  let compilerPath = BINARY;
  const stub = () => ({ dispose() {} });
  const vscode = {
    Range: class Range {
      constructor(startLine, startCharacter, endLine, endCharacter) {
        this.start = { line: startLine, character: startCharacter };
        this.end = { line: endLine, character: endCharacter };
      }
    },
    Diagnostic: class Diagnostic {
      constructor(range, message, severity) { this.range = range; this.message = message; this.severity = severity; }
    },
    DiagnosticSeverity: { Error: 0, Warning: 1 },
    EventEmitter: class EventEmitter { constructor() { this.event = () => ({ dispose() {} }); } fire() {} dispose() {} },
    ConfigurationTarget: { Global: 1 },
    window: {
      createOutputChannel() { return { appendLine(text) { logged.push(text); }, dispose() {} }; },
      onDidChangeTextEditorSelection: () => ({ dispose() {} }),
      setStatusBarMessage: () => ({ dispose() {} })
    },
    commands: { registerCommand: () => ({ dispose() {} }) },
    workspace: {
      getConfiguration() { return { get: key => (key === 'compilerPath' ? compilerPath : undefined) }; },
      onDidChangeConfiguration: () => ({ dispose() {} }),
      onDidSaveTextDocument: () => ({ dispose() {} }),
      getWorkspaceFolder() { return undefined; },
      textDocuments: [],
      notebookDocuments: [],
      onDidOpenTextDocument: stub,
      onDidChangeTextDocument(callback) { changed = callback; return { dispose() {} }; },
      onDidCloseTextDocument(callback) { closed = callback; return { dispose() {} }; }
    },
    languages: new Proxy({
      createDiagnosticCollection: () => collection
    }, { get: (target, name) => (name in target ? target[name] : stub) })
  };
  const load = Module._load;
  Module._load = function (request, parent, isMain) {
    if (request === 'vscode') return new Proxy(vscode, { get: (target, name) => (name in target ? target[name] : class Stub {}) });
    return load.call(this, request, parent, isMain);
  };
  const extension = require(path.join(__dirname, '..', 'out', 'extension.js'));
  extension.activate({ subscriptions: { push() {} } });
  Module._load = load;

  const folder = fs.mkdtempSync(path.join(os.tmpdir(), 'kestrel-doc-'));
  const file = path.join(folder, 'hello.kes');
  let text = 'let a : String = 1\n';
  const document = {
    uri: { toString: () => 'file://' + file, fsPath: file },
    languageId: 'kestrel',
    getText: () => text
  };
  const wait = ms => new Promise(resolve => setTimeout(resolve, ms));

  return (async () => {
    changed({ document });
    await wait(1200);
    const items = sets.get(document.uri.toString());
    assert(items && items.length === 1, 'one diagnostic after the first check');
    assert.strictEqual(items[0].severity, 0);
    assert.strictEqual(items[0].code, 'E_TYPE');
    assert.strictEqual(items[0].source, 'kestrel');
    assert.strictEqual(items[0].range.start.line, 0);
    assert.strictEqual(items[0].range.start.character, 17);
    assert.strictEqual(items[0].range.end.character, 18);
    assert(items[0].message.includes('hint: Use a value of the expected type'));

    text = 'let a : String = "fine"\n';
    changed({ document });
    await wait(1200);
    assert.deepStrictEqual(sets.get(document.uri.toString()), [], 'the fixed text clears the diagnostics');

    // A notebook cell is checked in notebook mode, so its top-level ? is accepted.
    const cellFile = path.join(folder, 'hello.ipynb');
    const cell = {
      uri: { toString: () => 'vscode-notebook-cell://' + cellFile, fsPath: cellFile, scheme: 'vscode-notebook-cell' },
      languageId: 'kestrel',
      notebook: {},
      getText: () => 'let n := parseInt("3")?\n'
    };
    changed({ document: cell });
    await wait(1200);
    assert.deepStrictEqual(sets.get(cell.uri.toString()), [], 'top-level ? is valid in a notebook cell');

    // A cell is checked behind the code cells above it, so their names stay defined,
    // and only the edited cell's own diagnostics are reported.
    const first = {
      uri: { toString: () => 'vscode-notebook-cell://' + cellFile + '#one', fsPath: cellFile, scheme: 'vscode-notebook-cell' },
      languageId: 'kestrel',
      notebook: {},
      getText: () => 'let kept := 7\n'
    };
    let secondText = 'print(kept)\n';
    const second = {
      uri: { toString: () => 'vscode-notebook-cell://' + cellFile + '#two', fsPath: cellFile, scheme: 'vscode-notebook-cell' },
      languageId: 'kestrel',
      notebook: {},
      getText: () => secondText
    };
    vscode.workspace.notebookDocuments = [{ getCells: () => [{ document: first }, { document: second }] }];
    changed({ document: second });
    await wait(1200);
    assert.deepStrictEqual(sets.get(second.uri.toString()), [], 'a name bound in the cell above is defined');

    secondText = 'print(absent)\n';
    changed({ document: second });
    await wait(1200);
    const missing = sets.get(second.uri.toString()) || [];
    assert(missing.length >= 1, 'a name defined in no cell is still an error');
    assert(missing.every(item => item.range.start.line === 0), 'each diagnostic is counted from the cell');
    assert(missing.some(item => item.message.includes('absent')), 'the diagnostic names the missing binding');

    // Moving the cell above the one that binds the name takes the name away again.
    vscode.workspace.notebookDocuments = [{ getCells: () => [{ document: second }, { document: first }] }];
    secondText = 'print(kept)\n';
    changed({ document: second });
    await wait(1200);
    const moved = sets.get(second.uri.toString()) || [];
    assert(moved.some(item => item.message.includes('kept')), 'a name bound only below the cell is not defined');

    // A compiler that cannot start is reported in the Kestrel output, and no diagnostics are set.
    compilerPath = path.join(folder, 'no-such-kestrel');
    text = 'let a : String = 1\n';
    const before = logged.length;
    changed({ document });
    await wait(1200);
    assert(logged.length > before, 'a missing compiler is logged');
    assert.deepStrictEqual(sets.get(document.uri.toString()), [], 'no stale diagnostics after a failed run');

    closed(document);
    assert(!sets.has(document.uri.toString()), 'closing the document clears its diagnostics');
  })();
}

integration()
  .then(() => {
    for (const note of notes) console.log(note);
    console.log('kestrel diagnostics tests pass');
  })
  .catch(error => {
    console.error(error);
    process.exitCode = 1;
  });
