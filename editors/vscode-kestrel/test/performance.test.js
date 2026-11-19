'use strict';

// A large-file guard. The bounds are deliberately generous: the bug these catch
// was quadratic and took minutes, not milliseconds.

const assert = require('assert');
const kestrel = require('../out/kestrel');
const infer = require('../out/infer');
const values = require('../out/values');
const parameters = require('../out/parameters');

function program(lines) {
  const parts = ['import std.kesdas.{Series, DataFrame}\n'];
  for (let index = 0; parts.join('\n').split('\n').length < lines; index += 1) {
    parts.push(`class Box${index} {
    public final value : Int
    new(value : Int) { this.value = value }
    public function next() : Box${index} = Box${index}(this.value + 1)
}
function sum${index}(items : List[Int]) : Int {
    let total := 0
    for (let item in items) {
        total = total + item
    }
    let doubled := items.map(value => value * 2).filter(value => value > 0)
    let first := doubled.toList()
    return total
}
`);
  }
  return parts.join('\n');
}

function measure(fn) {
  const started = process.hrtime.bigint();
  const result = fn();
  return { result, ms: Number(process.hrtime.bigint() - started) / 1e6 };
}

const text = program(2400);
const model = kestrel.parseDocument(text);
const offset = text.length;

const parsed = measure(() => kestrel.parseDocument(text));
assert(parsed.ms < 500, `parsing a large file took ${parsed.ms.toFixed(0)} ms`);

const bindings = measure(() => infer.inferredBindings(text, model));
assert(bindings.ms < 2000, `inference over a large file took ${bindings.ms.toFixed(0)} ms`);
const bindingsAgain = measure(() => infer.inferredBindings(text, model));
assert(bindingsAgain.ms < 50, `cached inference took ${bindingsAgain.ms.toFixed(0)} ms`);

const locals = measure(() => infer.localTypes(text, offset, model));
assert(locals.ms < 250, `local types took ${locals.ms.toFixed(0)} ms`);
const localsAgain = measure(() => infer.localTypes(text, offset, model));
assert(localsAgain.ms < 50, `cached local types took ${localsAgain.ms.toFixed(0)} ms`);

const uses = measure(() => values.valueUses(text, model));
assert(uses.ms < 2000, `value coloring took ${uses.ms.toFixed(0)} ms`);
const usesAgain = measure(() => values.valueUses(text, model));
assert(usesAgain.ms < 50, `cached value coloring took ${usesAgain.ms.toFixed(0)} ms`);

const parameterUses = measure(() => parameters.parameterUses(text, model));
assert(parameterUses.ms < 2000, `parameter uses took ${parameterUses.ms.toFixed(0)} ms`);
const parameterUsesAgain = measure(() => parameters.parameterUses(text, model));
assert(parameterUsesAgain.ms < 50, `cached parameter uses took ${parameterUsesAgain.ms.toFixed(0)} ms`);

console.log('kestrel large-file performance tests pass');
