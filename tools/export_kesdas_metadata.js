'use strict';

// Writes the editor's std.kesdas table as JSON for the Jupyter kernel. The
// extension and the kernel then share one source of truth:
//   node tools/export_kesdas_metadata.js
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const metadata = require(path.join(root, 'editors/vscode-kestrel/out/kesdas.js'));

const methods = {};
const statics = {};
const constructors = {};
const fields = {};
const functions = [];

function add(table, owner, entry) {
  (table[owner] ||= []).push({
    name: entry.name,
    signature: entry.signature,
    parameters: entry.parameters || [],
    documentation: entry.documentation || ''
  });
}

for (const entry of metadata.entries) {
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
    functions.push({ name: entry.name, signature: entry.signature, parameters: entry.parameters || [], documentation: entry.documentation || '' });
  }
}

const output = {
  module: 'std.kesdas',
  types: metadata.names.filter(name => /^[A-Z]/.test(name)),
  names: metadata.names,
  functions,
  constructors,
  statics,
  methods,
  fields
};

const target = path.join(root, 'editors/jupyter/kestrel_kernel/kesdas_metadata.json');
const text = `${JSON.stringify(output, null, 2)}\n`;
if (process.argv.includes('--check')) {
  const current = fs.existsSync(target) ? fs.readFileSync(target, 'utf8') : '';
  if (current !== text) {
    console.error(`${target} is stale; run node tools/export_kesdas_metadata.js`);
    process.exit(1);
  }
  console.log(`${target} is up to date`);
} else {
  fs.writeFileSync(target, text);
  console.log(`Wrote ${target}`);
}
