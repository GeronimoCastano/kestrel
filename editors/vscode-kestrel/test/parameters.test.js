'use strict';

const assert = require('assert');
const kestrel = require('../out/kestrel');
const parameters = require('../out/parameters');

function uses(source) {
  return parameters.parameterUses(source, kestrel.parseDocument(source))
    .map(token => source.slice(token.start, token.start + token.length));
}

function positions(source) {
  return parameters.parameterUses(source, kestrel.parseDocument(source))
    .map(token => source.slice(0, token.start).split('\n').length);
}

// A typed lambda colors its head and its body use; the arrow's body ends at the comma.
const typed = 'let a := Array.generate(10, x : Int => x*2)\nprint(a)\n';
assert.deepStrictEqual(uses(typed), ['x', 'x']);

// An untyped lambda inside a group, with a second argument after the body.
const grouped = 'let b := Array.generate(3, (i) => i + 1)\n';
assert.deepStrictEqual(uses(grouped), ['i', 'i']);

// A member name such as p.x is not a parameter use, and a match arm is not a lambda.
const arms = 'match h { Some(p) => print(p.x), None => print(p) }\n';
assert.deepStrictEqual(uses(arms), []);

// A function parameter's uses are colored in the function body.
const fn = 'function f(n : Int) : Int { return n + this.n }\n';
assert.deepStrictEqual(uses(fn), ['n', 'n']);

// Comments and strings are blank in the scan, so their words are not uses.
const quoted = '// x => x\nlet s := "x => x" + Array.generate(2, y => y)\n';
assert.deepStrictEqual(uses(quoted), ['y', 'y']);
assert.deepStrictEqual(positions(quoted), [2, 2]);

console.log('kestrel parameter use tests pass');

assert.deepStrictEqual(uses('let A := Array.generate(2, Index => Index + 1)\n'), ['Index', 'Index']);
assert.deepStrictEqual(uses('function head(X_c : Int) : Int = X_c + 1\n'), ['X_c', 'X_c']);
