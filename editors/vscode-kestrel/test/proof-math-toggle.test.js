'use strict';

const assert = require('assert');
const fs = require('fs');
const path = require('path');
const Module = require('module');

// The two title-bar entries share one toggle. A typo in the manifest or in the
// registration leaves a visible button that does nothing, so the two sides are
// compared here.
const writes = [];
const stub = () => ({ dispose() {} });
const vscode = new Proxy({
  window: { setStatusBarMessage: stub },
  commands: { executeCommand: stub },
  workspace: {
    getConfiguration: () => ({
      get: key => (key === 'proofState.math' ? writes[writes.length - 1] : undefined),
      update: (key, value) => {
        writes.push(value);
        return Promise.resolve();
      }
    })
  },
  ConfigurationTarget: { Global: 1 }
// Works both as `stub()` and as `new Stub()`: the module builds a legend and a
// few providers at load time, before any command runs.
}, { get: (target, name) => (name in target ? target[name] : function Stub() { return { dispose() {} }; }) });

const load = Module._load;
Module._load = function (request, parent, isMain) {
  if (request === 'vscode') return vscode;
  return load.call(this, request, parent, isMain);
};
const extension = require(path.join(__dirname, '..', 'out', 'extension.js'));
Module._load = load;

// The toggle flips the setting, so one button both turns typesetting on and
// takes it away again.
assert.strictEqual(extension.toggleProofStateMath.length, 0);
extension.toggleProofStateMath();
assert.deepStrictEqual(writes, [false], 'on by default, so the first press turns it off');
extension.toggleProofStateMath();
assert.deepStrictEqual(writes, [false, true], 'the second press turns it back on');

const manifest = require(path.join(__dirname, '..', 'package.json'));
const contributed = manifest.contributes.commands.map(entry => entry.command);
const source = fs.readFileSync(path.join(__dirname, '..', 'out', 'extension.js'), 'utf8');

for (const id of contributed) {
  assert(source.includes(`registerCommand('${id}'`), `${id} is contributed but never registered`);
}
for (const entry of manifest.contributes.menus['view/title']) {
  assert(contributed.includes(entry.command), `${entry.command} is in the title bar but not contributed`);
}

// Both entries drive the same toggle, and each names the context key that
// decides which icon shows, so the button reports the state it is in.
const entries = manifest.contributes.menus['view/title'].filter(entry => entry.command.startsWith('kestrel.proofStateMath'));
assert.deepStrictEqual(entries.map(entry => entry.command).sort(), ['kestrel.proofStateMathOff', 'kestrel.proofStateMathOn']);
assert(entries.some(entry => entry.when.includes('kestrel.proofStateMath') && !entry.when.includes('!')));
assert(entries.some(entry => entry.when.includes('!kestrel.proofStateMath')));
assert(source.includes("'setContext', 'kestrel.proofStateMath'"), 'the context key is set for the icon');
