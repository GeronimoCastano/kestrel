'use strict';

const assert = require('assert');
const grammar = require('../syntaxes/kestrel.tmLanguage.json');

const includes = grammar.patterns.map(pattern => pattern.include).filter(Boolean);
assert(includes.includes('#variables'));
assert(includes.includes('#method-calls'));
assert(includes.includes('#parameter-names'));
assert(grammar.repository['method-calls'].patterns[0].captures['1'].name === 'entity.name.function.kestrel');
assert.deepStrictEqual(grammar.repository.variables.patterns[0], {
  name: 'variable.other.kestrel',
  match: '\\b[a-z_][A-Za-z0-9_]*\\b'
});

const fstring = grammar.repository.strings.patterns.find(pattern => pattern.name === 'string.interpolated.kestrel');
assert(fstring);
const interpolation = fstring.patterns.find(pattern => pattern.name === 'meta.interpolation.kestrel');
assert(interpolation);
assert(interpolation.patterns.some(pattern => pattern.include === 'source.kestrel'));
assert(interpolation.beginCaptures);
assert(interpolation.endCaptures);

// Reserved words that read as keywords, including the logical ones used in `Int or Error`.
const control = new RegExp(grammar.repository.keywords.patterns[0].match);
for (const word of ['otherwise', 'or', 'and', 'not', 'while']) {
  assert(control.test(word), `${word} is a control keyword`);
}

// A borrowed trait object is one type token, `&dyn Area`, wherever it appears in a type.
const types = grammar.repository.types.patterns;
const borrowedDyn = new RegExp(types[0].match);
assert.strictEqual(borrowedDyn.exec('s2 : &dyn Area) : &dyn Area {')[0], '&dyn Area');
assert.strictEqual(new RegExp(types[1].match).exec('List[Box[dyn Shape]]')[0], 'dyn Shape');
// A borrowed named type after a colon or an arrow is a type, but bitwise `&` is still an operator.
const borrowedName = new RegExp(types[2].match);
assert.strictEqual(borrowedName.exec('x : &Point) : &Point')[0], ' &Point');
assert.strictEqual(borrowedName.exec('if a & MASK > 0 {'), null);

// The reverse-rewrite arrow is an operator, alongside the other set and logic glyphs.
const operators = new RegExp(grammar.repository.operators.patterns[0].match);
for (const glyph of ['←', '→', '∈', '⋃', '⋂']) {
  assert(operators.test(glyph), `${glyph} is an operator`);
}

console.log('kestrel grammar tests pass');

// Value declarations and constants use the same theme scope as ordinary variables.
const valueDeclaration = grammar.repository.declarations.patterns.find(pattern => pattern.captures?.['3']?.name === 'variable.other.kestrel');
for (const source of ['let X := 1', 'let X_c := 1', 'let lower := 1']) {
  assert(new RegExp(valueDeclaration.match).test(source), source);
}
const constant = grammar.repository.constants.patterns[0];
assert.strictEqual(constant.name, 'variable.other.kestrel');
for (const name of ['PI', 'E']) assert(new RegExp(constant.match).test(name));
const constantDeclaration = grammar.repository.declarations.patterns.find(pattern => pattern.match.startsWith('\\b(const)'));
assert.strictEqual(constantDeclaration.captures['2'].name, 'variable.other.kestrel');
