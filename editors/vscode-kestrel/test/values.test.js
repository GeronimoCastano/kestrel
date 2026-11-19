'use strict';

const assert = require('assert');
const kestrel = require('../out/kestrel');
const { valueUses } = require('../out/values');

function tokens(source) {
  return valueUses(source, kestrel.parseDocument(source));
}
function names(source) {
  return tokens(source).map(token => source.slice(token.start, token.start + token.length));
}

const tensor = `let X : Tensor[double, (3, 2)] = Tensor.ones((3, 2))
let mu := X.mean(axis: 0)
let X_c := X - mu.broadcastTo(X.shape())
print(X_c / PI)
`;
assert.deepStrictEqual(names(tensor), ['X', 'X', 'X_c', 'X', 'X', 'X_c', 'PI']);
assert.deepStrictEqual(names('const LIMIT : Int = 3\nprint(LIMIT)\nprint(PI + E + math.PI)\n'),
  ['LIMIT', 'LIMIT', 'PI', 'E', 'PI']);
assert.deepStrictEqual(names('let MixedCase := 1\nprint(MixedCase)\n'), ['MixedCase', 'MixedCase']);
assert.deepStrictEqual(names('X_c := 1\nprint(X_c)\n'), ['X_c', 'X_c']);
assert.deepStrictEqual(names('let (N, K) := (2, 3)\nprint(N + K)\n'), ['N', 'K', 'N', 'K']);
assert.deepStrictEqual(names('print([Word[0] for Word in words])\nprint(Word)\n'), ['Word', 'Word']);
assert.deepStrictEqual(names('for (Row in rows) { print(Row) }\nprint(Row)\n'), ['Row', 'Row']);
assert.deepStrictEqual(names('let X := 1\nprint("X PI") // X PI\nprint(obj.X)\n'), ['X']);

// A value in one function must not recolor a type with the same spelling elsewhere.
const scopes = `class Data { new() {} }
function first() : Int { let Data := 1
    Data }
function second() : Data = Data()
`;
assert.deepStrictEqual(names(scopes), ['Data', 'Data']);

// Annotations stay types even when a value of the same spelling is visible.
const annotations = 'let Data := 1\nlet next : Data = Data\nfunction f(x : Data) : Data = Data()\n';
const positions = tokens(annotations).map(token => token.start);
assert(!positions.includes(annotations.indexOf(': Data') + 2));
assert(!positions.includes(annotations.indexOf('x : Data') + 4));
assert(!positions.includes(annotations.indexOf(') : Data') + 4));

const manifest = require('../package.json');
assert.deepStrictEqual(manifest.contributes.semanticTokenScopes[0].scopes.variable, ['variable.other.kestrel']);
console.log('kestrel value coloring tests pass');
