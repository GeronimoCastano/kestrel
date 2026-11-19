'use strict';

// Every public member of src/kesnet.kes must have an editor entry. The compiler
// remains the authority; this test catches metadata drift after edits.
const assert = require('assert');
const fs = require('fs');
const path = require('path');
const kesnet = require('../out/kesnet');
const variable = require('../out/variable');

const source = fs.readFileSync(path.join(__dirname, '..', '..', '..', 'src', 'kesnet.kes'), 'utf8');
const classes = {};
const classPattern = /public class (\w+)[^{]*\{/g;
let match;
while ((match = classPattern.exec(source)) !== null) {
  const name = match[1];
  const start = classPattern.lastIndex;
  let depth = 1;
  let i = start;
  while (i < source.length && depth > 0) {
    if (source[i] === '{') depth += 1;
    else if (source[i] === '}') depth -= 1;
    i += 1;
  }
  const body = source.slice(start, i - 1);
  const members = { methods: new Set(), fields: new Set(), statics: new Set(), constructors: 0 };
  let braces = 0;
  for (const line of body.split('\n')) {
    const text = line.trim();
    if (braces === 0) {
      let found;
      if ((found = /^(@override\s+)?public function (\w+)/.exec(text))) members.methods.add(found[2]);
      else if ((found = /^public static function (\w+)/.exec(text))) members.statics.add(found[1]);
      else if ((found = /^public (?:final|var) (\w+)\s*:/.exec(text))) members.fields.add(found[1]);
      else if (/^new\s*\(/.test(text)) members.constructors += 1;
    }
    braces += (line.match(/{/g) || []).length - (line.match(/}/g) || []).length;
  }
  classes[name] = members;
}

for (const [name, expected] of Object.entries(classes)) {
  assert(kesnet.types.includes(name), `${name} has a type entry`);
  const entries = kesnet.entries.filter(entry => entry.receiver === name);
  const methods = new Set(entries.filter(entry => !entry.field && !entry.static).map(entry => entry.name));
  const fields = new Set(entries.filter(entry => entry.field).map(entry => entry.name));
  const statics = new Set(kesnet.entries.filter(entry => entry.static === name).map(entry => entry.name));
  const constructors = kesnet.entries.filter(entry => entry.named && entry.name === name).length;
  for (const method of expected.methods) assert(methods.has(method), `${name}.${method} is missing from kesnet.js`);
  for (const field of expected.fields) assert(fields.has(field), `${name}.${field} is missing from kesnet.js`);
  for (const method of expected.statics) assert(statics.has(method), `${name}.${method} is missing from kesnet.js`);
  assert.strictEqual(constructors, expected.constructors, `${name} constructor metadata`);
}

// A model's inherited Module methods are reached through the parent map.
assert.strictEqual(kesnet.parents.Linear, 'Module');
assert.strictEqual(kesnet.parents.Sequential, 'Module');
assert(kesnet.names.includes('mseLoss') && kesnet.names.includes('accuracy'), 'free functions are exported');

// The native Variable table is large enough to cover the documented surface.
const variableMethods = new Set(variable.entries.filter(entry => entry.receiver === 'Variable').map(entry => entry.name));
for (const name of ['relu', 'matmul', 'softmax', 'crossEntropy', 'conv2d', 'batchNorm', 'backward', 'gradVariable', 'reshape', 'cat', 'slice']) {
  assert(variableMethods.has(name), `Variable.${name} is in the editor table`);
}
const variableStatics = new Set(variable.entries.filter(entry => entry.static === 'Variable').map(entry => entry.name));
for (const name of ['zeros', 'fromArray', 'fromTensor', 'parameter', 'embedding', 'liveNodes']) {
  assert(variableStatics.has(name), `Variable.${name} static is in the editor table`);
}

console.log('kestrel kesnet metadata tests pass');
