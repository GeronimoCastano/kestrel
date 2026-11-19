'use strict';

// The editor's std.kesdas table must cover every public member of the library.
// The compiler remains the authority; this test catches metadata drift after
// src/kesdas.kes changes.
const assert = require('assert');
const fs = require('fs');
const path = require('path');
const kesdas = require('../out/kesdas');

const source = fs.readFileSync(path.join(__dirname, '..', '..', '..', 'src', 'kesdas.kes'), 'utf8');
const classes = {};
const classPattern = /public class (\w+)\s*\{/g;
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
      if ((found = /^public function (\w+)/.exec(text))) members.methods.add(found[1]);
      else if ((found = /^public static function (\w+)/.exec(text))) members.statics.add(found[1]);
      else if ((found = /^public (?:final|var) (\w+)\s*:/.exec(text))) members.fields.add(found[1]);
      else if (/^new\s*\(/.test(text)) members.constructors += 1;
    }
    braces += (line.match(/{/g) || []).length - (line.match(/}/g) || []).length;
  }
  classes[name] = members;
}

for (const [name, expected] of Object.entries(classes)) {
  const entries = kesdas.entries.filter(entry => entry.receiver === name);
  const methods = new Set(entries.filter(entry => !entry.field && !entry.static).map(entry => entry.name));
  const fields = new Set(entries.filter(entry => entry.field).map(entry => entry.name));
  const statics = new Set(kesdas.entries.filter(entry => entry.static === name).map(entry => entry.name));
  const constructors = kesdas.entries.filter(entry => entry.named && entry.name === name).length;
  for (const method of expected.methods) assert(methods.has(method), `${name}.${method} is missing from kesdas.js`);
  for (const field of expected.fields) assert(fields.has(field), `${name}.${field} is missing from kesdas.js`);
  for (const method of expected.statics) assert(statics.has(method), `${name}.${method} is missing from kesdas.js`);
  assert.strictEqual(constructors, expected.constructors, `${name} constructor metadata`);
  for (const method of methods) assert(expected.methods.has(method), `${name}.${method} is not public in kesdas.kes`);
  for (const field of fields) assert(expected.fields.has(field), `${name}.${field} is not public in kesdas.kes`);
  for (const method of statics) assert(expected.statics.has(method), `${name}.${method} is not public in kesdas.kes`);
}

assert(kesdas.names.includes('readCsv'), 'the free readCsv function is exported');
assert(kesdas.entries.filter(entry => entry.receiver === 'DataFrame' && entry.field).length >= 3, 'DataFrame fields are metadata');

console.log('kestrel kesdas metadata tests pass');
