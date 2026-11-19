'use strict';
const assert = require('assert');
const kestrel = require('../out/kestrel');
const infer = require('../out/infer');
const parameters = require('../out/parameters');
const grammar = require('../syntaxes/kestrel.tmLanguage.json');
const { loadExtension, labelsAt, hoverAt, signatureAt } = require('../test-support/harness');
const { providers } = loadExtension();
const source = `trait Measure[T] {
    /// Returns the supplied value.
    public function measure(value : T) : T = value
}
class Parent { new() {} }
class Sample extends Parent {
    has Measure[Int]
    new() {}
}
record SampleRecord(value : Int) { has Measure[Int] }
let sample := Sample()
let record := SampleRecord(1)
let view : &dyn Measure[Int] = sample
`;
for (const variable of ['sample', 'record', 'view']) {
  const text = source + `${variable}.`;
  assert(labelsAt(providers, text, text.length).includes('measure'), variable);
  const call = source + `${variable}.measure(2)`;
  const model = kestrel.parseDocument(call);
  assert.strictEqual(infer.infer(`${variable}.measure(2)`, model, new Map([...infer.localTypes(call, call.length, model)].map(([name, binding]) => [name, binding.type]))), 'Int', variable);
  assert(hoverAt(providers, call, call.lastIndexOf('measure') + 2).contents.value.includes('value : Int'), variable);
  assert(signatureAt(providers, call, call.length - 1).signatures[0].label.includes('value : Int'), variable);
}
const override = source.replace('new() {}\n}\nrecord', '@override public function measure(value : Int) : Int = value\n    new() {}\n}\nrecord');
assert.strictEqual(kestrel.memberFunctions(kestrel.parseDocument(override), 'Sample').filter(fn => fn.name === 'measure').length, 1);
const nested = `trait Root {public function value():Int=1}
trait Left {has Root}
trait Right {has Root}
class Diamond {has Left, Right;new(){}}
let d:=Diamond()
d.`;
assert.strictEqual(labelsAt(providers, nested, nested.length).filter(name => name === 'value').length, 1);
const tree = `enum Tree { Leaf, Add(Tree, Tree) }
theorem mirror(t : Tree) : t = t by {
    induction t {
        Leaf => { rfl }
        Add(a, b) iha ihb => { simp [iha, ihb] }
    }
}
`;
const cursor = tree.indexOf('simp') + 4;
const names = labelsAt(providers, tree, cursor);
for (const name of ['t', 'a', 'b', 'iha', 'ihb']) assert(names.includes(name), name);
const types = infer.localTypes(tree, cursor, kestrel.parseDocument(tree));
assert.strictEqual(types.get('t').type, 'Tree');
assert.strictEqual(types.get('a').type, 'Tree');
assert.strictEqual(types.get('iha').type, 'Proof');
assert(!labelsAt(providers, tree, tree.indexOf('rfl') + 2).includes('iha'), 'hypotheses stay in their branch');
const tokens = parameters.parameterUses(tree, kestrel.parseDocument(tree));
assert(tokens.some(token => token.start === tree.lastIndexOf('ihb')), 'IH use has a semantic token');
const proof = 'theorem same(n:Int):n=n{rfl}\nfunction accept(n:Int,proof h:n==n):Int=n\naccept(1, ';
assert(!labelsAt(providers, proof, proof.length).includes('OrLeft'), 'tactic-only proof constructors stay out of executable arguments');
assert(labelsAt(providers, proof, proof.length).includes('same'), 'theorem suggestion in erased proof argument');
assert(signatureAt(providers, proof + 'same(', proof.length + 5).signatures[0].label.startsWith('same('));
assert(hoverAt(providers, proof + 'same(1))', proof.length + 2).contents.value.includes('n = n') || hoverAt(providers, proof + 'same(1))', proof.length + 2).contents.value.includes('n=n'));
for (const keyword of ['record', 'theorem', 'proof', 'depends', 'from', 'at', 'smt']) {
  assert(grammar.repository.keywords.patterns.some(rule => new RegExp(rule.match).test(keyword)), keyword);
}
console.log('Five-feature completion, inference, signatures, hover, scope and highlighting tests pass');

const optional = kestrel.parseDocument('trait Optional {public function get(n:Int=2):Int=n}\nclass OptionClass {has Optional;new(){}}');
assert.strictEqual(infer.infer('s.get()',optional,new Map([['s','OptionClass']])),'Int');
const associated = kestrel.parseDocument('trait Source {type Item\npublic function value(item:Item):Item=item}\nclass IntSource {has Source(Item = Int);new(){}}');
assert.strictEqual(infer.infer('s.value(3)',associated,new Map([['s','IntSource']])),'Int');
const multi = kestrel.parseDocument('trait Convert {public function convert(value:Int):Int=value\npublic function convert(value:String):String=value}\nclass Converter {has Convert;new(){}}');
assert.strictEqual(infer.infer('s.convert("hello")',multi,new Map([['s','Converter']])),'String');

const parent = 'class ParentValue[T] {public final value:T}\nclass IntValue extends ParentValue[Int] {}\nlet item:=IntValue()\n';
assert(labelsAt(providers,parent+'item.',parent.length+5).includes('value'));
assert(hoverAt(providers,parent+'item.value',parent.length+7).contents.value.includes('value : Int'));
const parentModel=kestrel.parseDocument(parent);
assert.strictEqual(infer.infer('item.value',parentModel,new Map([['item','IntValue']])),'Int');
