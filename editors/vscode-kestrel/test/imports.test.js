'use strict';

// Project-local imports: the editor reads the declarations a file imports, so
// signature help, hover and parameter hints work on them without a save.

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const kestrel = require('../out/kestrel');
const imports = require('../out/imports');
const { loadExtension, documentFor, labelsAt, hoverAt, signatureAt, inlayHints } = require('../test-support/harness');

const base = fs.mkdtempSync(path.join(os.tmpdir(), 'kestrel-imports-'));
const { providers } = loadExtension({});

function write(relative, text) {
  const file = path.join(base, relative);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
  return file;
}

const planning = path.join(base, 'planning');
const heuristics = path.join(base, 'heuristics');
const facts = write('planning/facts.kes', `public record Fact(variable : Int, value : Int)
public record State(values : List[Int]) {
    public function at(variable : Int) : Int = this.values.at(variable)
}
package function samePackage() : Int = 1
`);
write('planning/kestrel.project', 'package planning\n');
write('planning/action.kes', 'import facts.{Fact}\npublic record Action(name : String, cost : Int, precondition : List[Fact], effect : List[Fact])\n');
write('planning/problem.kes', `import facts.{Fact, State}
import action.{Action}
/// A planning problem over finite variables.
/// Its goal is a list of facts.
public record Problem(variables : List[String], initial : State, goal : List[Fact], actions : List[Action]) {
    /// Whether every goal fact holds in the state.
    public function isGoal(state : State) : Bool = false
}
private function hidden() : Int = 1
`);
write('planning/extra.kes', 'package function internal() : Int = 1\npublic function external() : Int = 2\n');
write('heuristics/kestrel.project', 'package heuristics\ndepends planning\n');
const demo = write('heuristics/demo.kes', `import planning.problem.{Problem}
import planning.facts.{Fact, State}
import planning.action.{Action}
`);

// Resolution follows the compiler: the package's folder, a dependency's folder
// beside it, and standard modules from the built-in list.
assert.strictEqual(imports.resolve(heuristics, 'planning.problem'), path.join(planning, 'problem.kes'));
assert.strictEqual(imports.resolve(planning, 'facts'), facts);
assert.strictEqual(imports.resolve(heuristics, 'facts'), '');
assert.strictEqual(imports.resolve(heuristics, 'std.linalg'), '');
assert.strictEqual(imports.resolve(heuristics, 'planning.absent'), '');

// A dependency's public declarations reach the model, with fields, methods and
// the constructor. Private declarations stay home.
const text = fs.readFileSync(demo, 'utf8');
const model = imports.modelFor(text, { path: demo, root: heuristics });
for (const name of ['Problem', 'Fact', 'State', 'Action']) {
  assert(model.classesByName.has(name), `imported type ${name}`);
}
assert.strictEqual(model.classesByName.get('Problem').kind, 'record');
assert.deepStrictEqual(
  model.classesByName.get('Problem').fields.map(field => field.name),
  ['variables', 'initial', 'goal', 'actions']
);
assert.deepStrictEqual(
  (model.functionsByOwner.get('Problem') || []).map(fn => fn.name),
  ['isGoal']
);
assert.strictEqual(
  kestrel.signature(model.constructorsByName.get('Problem')[0]),
  'Problem(variables : List[String], initial : State, goal : List[Fact], actions : List[Action]) : Problem'
);
assert.strictEqual((model.functionsByOwner.get('State') || []).length, 1, 'imported method');
assert(!model.functions.some(fn => fn.name === 'hidden'), 'private function stays out');
assert(!model.classesByName.get('Problem').imported || model.classesByName.get('Problem').imported === true);

// `package` visibility crosses a file inside one package, not a package boundary.
const samePackage = imports.modelFor('import facts.{samePackage}\n', { path: path.join(planning, 'use.kes'), root: planning });
assert(samePackage.functionsByName.has('samePackage'), 'package declaration inside its package');
const acrossPackage = imports.modelFor('import planning.facts.{samePackage}\n', { path: demo, root: heuristics });
assert(!acrossPackage.functionsByName.has('samePackage'), 'package declaration stays inside the package');
const publicAcross = imports.modelFor('import planning.extra.{internal, external}\n', { path: demo, root: heuristics });
assert(publicAcross.functionsByName.has('external'), 'public declaration crosses packages');
assert(!publicAcross.functionsByName.has('internal'), 'package declaration does not');

// A namespace import stays qualified, so nothing joins the unqualified model.
const namespace = imports.modelFor('import planning.problem\n', { path: demo, root: heuristics });
assert(!namespace.classesByName.has('Problem'), 'namespace import adds no unqualified type');

// The providers read the imported constructor: signature help, hover and the
// field names beside each argument.
const call = `${text}\npublic function sample() : Problem {\n    Problem([], State([0]), [], [])\n}\n`;
const callAt = call.lastIndexOf('Problem(') + 'Problem('.length;
const help = signatureAt(providers, call, callAt, demo);
assert(help && help.signatures[0].label.includes('Problem(variables : List[String]'), 'imported constructor signature help');
assert(
  help.signatures[0].documentation.value.startsWith('Record constructor'),
  'imported record constructor'
);
const hovered = hoverAt(providers, call, call.indexOf('Problem([') + 2, demo);
assert(hovered && hovered.contents.value.includes('record Problem'), 'imported record hover');
assert(
  hovered.contents.value.includes('A planning problem over finite variables.\nIts goal is a list of facts.'),
  'imported record documentation in hover'
);
assert(
  help.signatures[0].documentation.value.includes('A planning problem over finite variables.'),
  'imported record documentation in signature help'
);
const hints = inlayHints(providers, call, 0, call.length, demo).map(hint => hint.label);
assert(hints.includes('variables =') && hints.includes('initial =') && hints.includes('goal ='), `imported field hints: ${hints}`);
const offered = labelsAt(providers, `${text}\nPro`, `${text}\nPro`.length, demo);
assert(offered.includes('Problem'), 'imported type completion');
const offerDocument = documentFor(`${text}\nPro`, demo);
const problemItem = providers.completion
  .provideCompletionItems(offerDocument, offerDocument.positionAt(`${text}\nPro`.length))
  .find(item => item.label === 'Problem');
assert(
  problemItem && problemItem.documentation && problemItem.documentation.value.includes('A planning problem'),
  'imported type documentation in completion'
);


// A loop over a record's list names its element's fields; the list itself
// offers the list members, never the element's fields.
const method = `${text}
public function sample(problem : Problem) : Int {
    for (let action : Action in problem.actions) {
        action.
    }
    0
}
`;
const actionAt = method.lastIndexOf('action.') + 'action.'.length;
const actionFields = labelsAt(providers, method, actionAt, demo);
for (const field of ['name', 'cost', 'precondition', 'effect']) {
  assert(actionFields.includes(field), `loop element field ${field}`);
}
const list = `${text}
public function sample(problem : Problem) : Int {
    problem.actions.
}
`;
const listAt = list.indexOf('problem.actions.') + 'problem.actions.'.length;
const listMembers = labelsAt(providers, list, listAt, demo);
assert(listMembers.includes('length') && listMembers.includes('map'), 'list members');
assert(!listMembers.includes('name'), 'a list does not offer the element fields');

fs.rmSync(base, { recursive: true, force: true });
console.log('kestrel import index tests pass');
