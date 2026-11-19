'use strict';

// A notebook runs its cells as one session, so the language services read a cell
// behind the code cells above it. A name bound earlier keeps its type, which is
// what member completion, hover and signature help need.

const assert = require('assert');
const { loadExtension, documentFor } = require('../test-support/harness');

const { providers, vscode } = loadExtension();

const cell = (text, name) => documentFor(text, `hello.ipynb#${name}`, true);
const at = (document, text) => document.positionAt(text.length);
const labels = (document, text) =>
  (providers.completion.provideCompletionItems(document, at(document, text)) || []).map(item => item.label);
const open = documents => {
  vscode.workspace.notebookDocuments = [{ getCells: () => documents.map(document => ({ document })) }];
};

const binding = cell([
  'import std.kesdas as kd',
  'let sales := kd.DataFrame([',
  '    kd.Series(["north", "south"], name = "region"),',
  '    kd.Series([1, 2], name = "units"),',
  '])',
  ''
].join('\n'), 'binding');
const members = cell('sales.\n', 'members');
const chained = cell('sales.col("region")?.\n', 'chained');
const call = cell('sales.sortValues(', 'call');
const typed = cell('sal', 'typed');
open([binding, members, chained, call, typed]);

// A DataFrame member is offered in the cell below the binding.
const frameMembers = labels(members, 'sales.');
for (const name of ['fillNa', 'sortValues', 'columns', 'length']) {
  assert(frameMembers.includes(name), `a frame from the cell above offers ${name}: ${frameMembers.slice(0, 8)}`);
}

// A chain keeps the type through `?`, so a column offers Series members.
const seriesMembers = labels(chained, 'sales.col("region")?.');
assert(seriesMembers.includes('mean'), `a column from an earlier cell offers mean: ${seriesMembers.slice(0, 8)}`);
assert(!seriesMembers.includes('col'), 'a column does not offer frame members');

// The name is listed as a plain completion with its type, and hover shows it.
assert(labels(typed, 'sal').includes('sales'), 'a name bound above completes in the next cell');
const hover = providers.hover.provideHover(members, { line: 0, character: 2 });
assert(hover && /sales : DataFrame/.test(hover.contents.value || ''), 'hover knows the type from the cell above');

// Signature help reads the receiver's type the same way.
const help = providers.signature.provideSignatureHelp(call, at(call, 'sales.sortValues('));
assert(help && help.signatures.length > 0, 'signature help finds sortValues');
assert(help.signatures[0].label.includes('by : String'), `the signature names its parameters: ${help.signatures[0].label}`);

// Import completion edits this cell, so its range stays inside the cell.
const imports = cell('import std.kes', 'imports');
open([binding, imports]);
const importItems = providers.completion.provideCompletionItems(imports, at(imports, 'import std.kes')) || [];
assert(importItems.some(item => item.label === 'kesdas'), 'a later cell completes module names');
assert.strictEqual(importItems[0].range.start.character, 'import std.'.length, 'the import edit starts in the cell');

// A cell above the binding does not see it.
const before = cell('sales.\n', 'before');
open([before, binding]);
assert(!labels(before, 'sales.').includes('fillNa'), 'a name bound only below the cell has no members');

// A plain file keeps reading only its own text.
const script = documentFor('let sales := [1, 2, 3]\nsales.');
assert(labels(script, script.getText()).includes('map'), 'a plain file still completes its own bindings');

console.log('kestrel notebook tests pass');
