'use strict';

// Writes the editor's standard-library table as JSON for the Jupyter kernel.
// Regenerate after changing editors/vscode-kestrel/out/builtins.js:
//   node tools/export_builtins_metadata.js
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const builtins = require(path.join(root, 'editors/vscode-kestrel/out/builtins.js'));
const standardModules = require(path.join(root, 'editors/vscode-kestrel/out/modules.js'));

const methods = {};
const statics = {};
const constructors = {};
const fields = {};
const functions = [];

function add(table, owner, entry) {
  (table[owner] ||= []).push({
    name: entry.name,
    receiver: entry.receiver || '',
    signature: entry.signature,
    parameters: entry.parameters || [],
    documentation: entry.documentation || '',
    module: entry.module || ''
  });
}

for (const entry of builtins.entries) {
  if (entry.receiver) {
    if (entry.field) {
      (fields[entry.receiver] ||= {})[entry.name] = entry.signature.slice(entry.signature.lastIndexOf(':') + 1).trim();
    } else {
      add(methods, entry.receiver, entry);
    }
  } else if (entry.static) {
    add(statics, entry.static, entry);
  } else if (entry.named) {
    add(constructors, entry.name, entry);
  } else if (!entry.type) {
    functions.push({
      name: entry.name,
      signature: entry.signature,
      parameters: entry.parameters || [],
      documentation: entry.documentation || '',
      module: entry.module || ''
    });
  }
}

// The kernel needs to know which names each standard module exports, both for
// `namespace.` completion and for qualified calls such as `linalg.inverse`.
const modules = standardModules.modules.map(module => ({
  path: module.path,
  names: module.names
}));

const output = { module: 'builtins', modules, functions, constructors, statics, methods, fields };
const target = path.join(root, 'editors/jupyter/kestrel_kernel/builtins_metadata.json');
const text = `${JSON.stringify(output, null, 2)}\n`;
if (process.argv.includes('--check')) {
  const current = fs.existsSync(target) ? fs.readFileSync(target, 'utf8') : '';
  if (current !== text) {
    console.error(`${target} is stale; run node tools/export_builtins_metadata.js`);
    process.exit(1);
  }
  console.log(`${target} is up to date`);
} else {
  fs.writeFileSync(target, text);
  console.log(`Wrote ${target}`);
}
