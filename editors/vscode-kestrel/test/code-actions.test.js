'use strict';

const assert = require('assert');
const { loadExtension, documentFor } = require('../test-support/harness');

const { providers, vscode, extension } = loadExtension();
assert(providers.codeActions, 'the extension registers a code action provider');

const text = 'theorem t(n : Nat) : n = n { induction n { Zero => rfl, } }\n';
const document = documentFor('nat.kes', text);
const range = new vscode.Range(0, 0, 0, 0);
const diagnostic = { code: 'E_PROOF_CASES', range };

async function main() {
  const provider = new extension.KestrelCodeActionProvider(async (target, requested) => {
    assert.strictEqual(target, document);
    assert.strictEqual(requested, range);
    return [{
      title: 'Add the `Succ(k) ih` case',
      code: 'E_PROOF_CASES',
      edits: [{ start: 10, end: 10, text: '    Succ(k) ih => { },\n' }]
    }];
  });
  const actions = await provider.provideCodeActions(document, range, { diagnostics: [diagnostic] });
  assert.strictEqual(actions.length, 1);
  assert.strictEqual(actions[0].title, 'Add the `Succ(k) ih` case');
  assert.strictEqual(actions[0].kind, vscode.CodeActionKind.QuickFix);
  assert.strictEqual(actions[0].edit.edits.length, 1);
  assert.deepStrictEqual(actions[0].edit.edits[0].range.start, document.positionAt(10));
  assert.strictEqual(actions[0].edit.edits[0].newText, '    Succ(k) ih => { },\n');
  assert.deepStrictEqual(actions[0].diagnostics, [diagnostic]);
  assert.strictEqual(actions[0].isPreferred, true);

  const other = documentFor('plain.kes', 'let n := 1\n');
  other.languageId = 'markdown';
  assert.deepStrictEqual(await provider.provideCodeActions(other, range, {}), []);
}

main().then(
  () => console.log('code-actions: ok'),
  error => {
    console.error(error);
    process.exitCode = 1;
  }
);
