'use strict';

const assert = require('assert');
const fs = require('fs');
const path = require('path');
const { loadExtension, documentFor } = require('../test-support/harness');

const BINARY = path.join(__dirname, '..', '..', '..', 'target', 'release', process.platform === 'win32' ? 'kestrel.exe' : 'kestrel');
if (!fs.existsSync(BINARY)) {
  console.log('skipped code action runs: no release binary at ' + BINARY);
  process.exit(0);
}

const { providers, vscode } = loadExtension({ compilerPath: BINARY });
assert(providers.codeActions);

const cases = [
  {
    text: 'trait Shape { function area() : double }\nclass C { has Shape }\n',
    line: 1,
    title: 'area',
    expected: 'public function area() : double'
  },
  {
    text: 'enum Color { Red, Green }\nfunction f(c : Color) : Int = match c { Red => 1, }\n',
    line: 1,
    title: 'Green',
    expected: 'Green => { }'
  },
  {
    text: 'theorem t(n : Nat) : n = n { induction n { Zero => rfl, } }\n',
    line: 0,
    title: 'Succ(k) ihk',
    expected: 'Succ(k) ihk => { }'
  },
  {
    text: 'theorem t(n : Nat) : n = n { induction n { Zero => rfl, } }\n',
    line: 0,
    title: 'Succ(k) ihk',
    expected: 'Succ(k) ihk => { }',
    notebook: true
  },
  {
    text: 'theorem t(n : Nat) : n = n by {\n    induction n {\n        Zero => { rfl }\n    }\n}\n',
    line: 2,
    title: 'Succ(k) ihk',
    expected: 'Succ(k) ihk => { }'
  }
];

async function main() {
  for (const [index, item] of cases.entries()) {
    const document = documentFor(item.text, `case${index}.kes`, Boolean(item.notebook));
    const line = item.text.split('\n')[item.line];
    const range = new vscode.Range(item.line, 0, item.line, line.length);
    const actions = await providers.codeActions.provideCodeActions(document, range, { diagnostics: [] });
    const found = actions.find(action => action.title.includes(item.title));
    assert(found, `an action for ${item.title}: ${actions.map(action => action.title).join(', ')}`);
    assert(found.edit.edits.length >= 1, 'the action has an edit');
    assert(
      found.edit.edits.some(edit => edit.newText.includes(item.expected)),
      `the edit writes ${item.expected}: ${found.edit.edits.map(edit => edit.newText).join('')}`
    );
  }
}

main().then(
  () => console.log('kestrel code action runs pass'),
  error => {
    console.error(error);
    process.exitCode = 1;
  }
);
