'use strict';

// Go to definition: a local declaration, an imported one, and the member paths
// that take an expression such as `state.at(...)` or `Mode.Fast` to its source.

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { loadExtension, documentFor } = require('../test-support/harness');

const base = fs.mkdtempSync(path.join(os.tmpdir(), 'kestrel-definition-'));
const planning = path.join(base, 'planning');
const heuristics = path.join(base, 'heuristics');

function write(relative, text) {
  const file = path.join(base, relative);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
  return file;
}

const facts = write('planning/facts.kes', `/// A fact.
public record Fact(variable : Int, value : Int)

/// A state.
public record State(values : List[Int]) {
    /// The value of one variable.
    public function at(variable : Int) : Int = this.values.at(variable)
}

/// A finite set of modes.
public enum Mode {
    Fast
    Slow(Int)
}

public const LIMIT : Int = 7

public trait Reader {
    function read(state : State) : Int
}

public function helper(fact : Fact) : Int = fact.value
`);
write('planning/kestrel.project', 'package planning\n');
const problem = write('planning/problem.kes', `import facts.{Fact, State}

public record Problem(
    variables : List[State],
    initial : State,
    goal : List[Fact]
) {
    public function isGoal(state : State) : Bool = this.goal.length() > 0
}
`);
write('heuristics/kestrel.project', 'package heuristics\ndepends planning\n');
const demo = write('heuristics/demo.kes', `import planning.facts.{Fact, State, LIMIT, Mode, Reader, helper}
import planning.problem.{Problem}

// A comment that names Planner and LIMIT.
public class Planner {
    private final problem : Problem
    private final fallback : Int

    public new(problem : Problem, fallback : Int) {
        this.problem = problem
        this.fallback = fallback
    }

    private function cost(fact : Fact) : Int = fact.value + LIMIT + helper(fact)

    public function readThrough(reader : Reader, state : State, fact : Fact) : Int = reader.read(state) + helper(fact)

    public function report(problem : Problem) : Int {
        let state := problem.initial
        let mode := Mode.Fast
        let label := "problem and LIMIT are text here"
        let size := label.length()
        let fact := problem.goal.at(0)
        let first := fact.value
        let total := state.at(0) + this.cost(problem.goal.at(0)) + this.fallback + first
        if this.problem.isGoal(state) { return total }
        match mode {
            Fast => total
            Slow(n) => n
        }
    }
}
`);

const namespaceFile = write('heuristics/namespace.kes', `import planning.facts

public class Holder {
    has facts.Reader
    new() {}
}

public function read(state : facts.State, fact : facts.Fact, mode : facts.Mode) : Int {
    let total := state.at(0) + fact.variable
    match mode {
        facts.Mode.Fast => total
        facts.Mode.Slow(n) => n
    }
}
`);
const namespaceText = fs.readFileSync(namespaceFile, 'utf8');

const demoText = fs.readFileSync(demo, 'utf8');
const { providers } = loadExtension({});
assert(providers.definition, 'definition provider registered');

function offsetOf(text, marker, word, occurrence = 0) {
  let from = 0;
  let at = -1;
  for (let i = 0; i <= occurrence; i += 1) {
    at = text.indexOf(marker, from);
    assert(at >= 0, `marker not found: ${marker}`);
    from = at + 1;
  }
  const wordIndex = marker.indexOf(word);
  assert(wordIndex >= 0, `word ${word} not in marker`);
  return at + wordIndex;
}

async function definitionAt(text, file, offset) {
  const document = documentFor(text, file);
  return providers.definition.provideDefinition(document, document.positionAt(offset));
}

function sourceTextAt(location) {
  const text = fs.readFileSync(location.uri.fsPath, 'utf8');
  const lines = text.split('\n');
  const start = lines.slice(0, location.range.start.line).reduce((offset, line) => offset + line.length + 1, 0)
    + location.range.start.character;
  const length = location.range.end.character - location.range.start.character;
  return text.slice(start, start + length);
}

function assertTarget(locations, file, name, message) {
  assert(locations && locations.length, `${message}: no definition`);
  assert.strictEqual(locations[0].uri.fsPath, file, `${message}: file`);
  assert.strictEqual(sourceTextAt(locations[0]), name, `${message}: range`);
}

(async () => {
  // A class, method, field, parameter and local name all resolve inside the file.
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'public class Planner', 'Planner')), demo, 'Planner', 'class');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'this.cost(problem.goal', 'cost')), demo, 'cost', 'method call');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, '+ this.fallback', 'fallback')), demo, 'fallback', 'field');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'public new(problem : Problem', 'problem')), demo, 'problem', 'constructor parameter');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'problem.initial', 'problem')), demo, 'problem', 'parameter use');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'let state := problem.initial', 'state')), demo, 'state', 'let binding');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'public function report', 'report')), demo, 'report', 'method declaration');

  // Imported classes, record fields, methods, enum cases, constants and functions.
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'private final problem : Problem', 'Problem')), problem, 'Problem', 'imported class in a field type');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'problem.goal.at', 'goal')), problem, 'goal', 'imported record field');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'this.problem.isGoal', 'isGoal')), problem, 'isGoal', 'imported method');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'fact.value + LIMIT', 'value')), facts, 'value', 'imported field through a local');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'let first := fact.value', 'value')), facts, 'value', 'field through an indexed list element');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, '+ LIMIT', 'LIMIT')), facts, 'LIMIT', 'imported constant');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'Mode.Fast', 'Fast')), facts, 'Fast', 'qualified enum case');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, '            Fast => total', 'Fast')), facts, 'Fast', 'bare enum case in a match');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, '+ helper(fact)', 'helper')), facts, 'helper', 'imported function');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, '.read(state)', 'read')), facts, 'read', 'imported trait method');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, '.at(0) + this.cost', 'at')), facts, 'at', 'imported method through a local');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'problem.initial', 'initial')), problem, 'initial', 'imported field on a parameter');

  // A whole-module import keeps its types under the namespace alias.
  assertTarget(await definitionAt(namespaceText, namespaceFile, offsetOf(namespaceText, 'state : facts.State', 'State')), facts, 'State', 'namespace-qualified type');
  assertTarget(await definitionAt(namespaceText, namespaceFile, offsetOf(namespaceText, 'fact.variable', 'variable')), facts, 'variable', 'field through a namespace-qualified type');
  assertTarget(await definitionAt(namespaceText, namespaceFile, offsetOf(namespaceText, '.at(0) + fact.variable', 'at')), facts, 'at', 'method through a namespace-qualified type');
  assertTarget(await definitionAt(namespaceText, namespaceFile, offsetOf(namespaceText, 'facts.Mode.Fast', 'Fast')), facts, 'Fast', 'namespace-qualified enum case');
  assertTarget(await definitionAt(namespaceText, namespaceFile, offsetOf(namespaceText, 'facts.Mode.Slow', 'Slow')), facts, 'Slow', 'namespace-qualified enum payload case');
  assertTarget(await definitionAt(namespaceText, namespaceFile, offsetOf(namespaceText, 'has facts.Reader', 'Reader')), facts, 'Reader', 'namespace-qualified trait');

  // A record field declared in its parameter list points at the name itself.
  const factsText = fs.readFileSync(facts, 'utf8');
  assertTarget(await definitionAt(factsText, facts, offsetOf(factsText, 'this.values', 'values')), facts, 'values', 'record field');

  // Builtins, comments, strings and unknown names have no source to visit.
  assert.strictEqual(await definitionAt(demoText, demo, offsetOf(demoText, 'label.length()', 'length')), null, 'builtin method');
  assert.strictEqual(await definitionAt(demoText, demo, offsetOf(demoText, '// A comment', 'comment')), null, 'comment');
  assert.strictEqual(await definitionAt(demoText, demo, offsetOf(demoText, '"problem and LIMIT', 'problem')), null, 'string');
  assertTarget(await definitionAt(demoText, demo, offsetOf(demoText, 'this.fallback', 'fallback') + 'fallback'.length), demo, 'fallback', 'click after a word');

  fs.rmSync(base, { recursive: true, force: true });
  console.log('kestrel go-to-definition tests pass');
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
