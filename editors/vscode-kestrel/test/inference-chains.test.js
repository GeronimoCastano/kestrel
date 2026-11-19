'use strict';

const assert = require('assert');
const kestrel = require('../out/kestrel');
const infer = require('../out/infer');

function typesOf(text) {
  return Object.fromEntries(
    infer.inferredBindings(text, kestrel.parseDocument(text)).map(binding => [binding.name, binding.type || ''])
  );
}

// Each case is a Kestrel program and the type the editor must give each binding.
const cases = [
  ['playground toArray', 'let array := [1, 2, 3].toArray()\nprint(array[0])\n', { array: 'Array[Int]' }],
  ['list then toArray', 'let list := [1, 2, 3]\nlet array := list.toArray()\n', { array: 'Array[Int]' }],
  ['range toList', 'let r := range(3)\nlet l := r.toList()\n', { l: 'List[Int]' }],
  ['range toArray', 'let a := range(3).toArray()\n', { a: 'Array[Int]' }],
  ['map chain', 'let list := [1, 2, 3]\nlet doubled := list.map(n => n * 2).toList()\n', { doubled: 'List[Int]' }],
  ['map stays lazy', 'let d := range(4).map(n => n * 2)\n', { d: 'Lazy[Int]' }],
  ['filter chain', 'let f := [1, 2, 3].filter(n => n > 1).toArray()\n', { f: 'Array[Int]' }],
  ['map changes type', 'let s := [1, 2].map(n => n + 1).toList()\n', { s: 'List[Int]' }],
  ['array index', 'let a := Array(3, 0)\nlet x := a[0]\nlet n := a.length()\n', { a: 'Array[Int]', x: 'Int', n: 'Int' }],
  ['generate annotated', 'let array := Array.generate(10, x : Int => x*2)\nlet first := array[0]\n', { array: 'Array[Int]', first: 'Int' }],
  ['generate plain', 'let array := Array.generate(10, x => x*2)\n', { array: 'Array[Int]' }],
  ['array get', 'let a := Array(3, 0)\nlet g := a.get(1)\n', { g: 'Option[Int]' }],
  ['string length', 'let s := "hi"\nlet n := s.length() + 1\n', { n: 'Int' }],
  ['arithmetic precedence', 'let x := (1 + 2) * 3 - 4 / 2\nlet y := 2 ** 3\nlet z := 1.5 + 2\n', { x: 'Int', y: 'Int', z: 'double' }],
  ['unary and logic', 'let neg := -(3 * 2)\nlet b := !(1 < 2) || 3 == 3\n', { neg: 'Int', b: 'Bool' }],
  ['string concatenation', 'let s := "a" + "b" + "c"\n', { s: 'String' }],
  ['pipe to a function', 'function twice(n : Int) : Int = n * 2\nlet p := 3 |> twice\n', { p: 'Int' }],
  ['class fields and methods', 'class Point {\n  public final x : Int\n  new(x : Int) { this.x = x }\n  public function twice() : Int = x * 2\n}\nlet p := Point(3)\nlet px := p.x\nlet pt := p.twice()\n', { p: 'Point', px: 'Int', pt: 'Int' }],
  ['named arguments', 'let a := Array(size = 3, value = 0)\n', { a: 'Array[Int]' }],
  ['map literal', 'let m := Map("a" => 1)\nlet v := m.length()\n', { m: 'Map[String, Int]', v: 'Int' }],
  ['annotated lambda value', 'let f := (x : Int) => x * 2\n', { f: '(Int) -> Int' }],
  ['nested collections', 'let nested := [[1], [2]].toArray()\n', { nested: 'Array[List[Int]]' }],
  ['parameters in a function', 'function f(n : Int) : Int {\n  let k := n + 1\n  return k\n}\n', { k: 'Int' }],
  ['this in a method', 'class P {\n  public final x : Int\n  new(x : Int) { this.x = x }\n  public function twice() : Int {\n    let y := this.x * 2\n    return y\n  }\n}\n', { y: 'Int' }],
  ['for in a list', 'for (let v in [1, 2]) {\n  let w := v * 2\n}\n', { v: 'Int', w: 'Int' }],
  ['trailing comment', 'let a := [1, 2] // numbers\nlet s := "a // b"\n', { a: 'List[Int]', s: 'String' }],
  ['for over a range', 'for (i in range(3)) {\n  let q := i + 1\n}\n', { i: 'Int', q: 'Int' }],
  ['string index', 'let s := "abc"\nlet c := s[0]\n', { c: 'String' }],
  ['tuple index', 'let pair := (1, "a")\nlet first := pair[0]\nlet second := pair[1]\n', { first: 'Int', second: 'String' }],
  ['tensor square root', 'let t := Tensor.ones((2, 2))\nlet x := sqrt(t)\n', { x: 'Tensor[double, (2, 2)]' }],
  ['tensor absolute value', 'let t := Tensor.ones((2, 2))\nlet x := abs(t)\n', { x: 'Tensor[double, (2, 2)]' }],
  ['conversion pipe to double', 'let d := 1 |> double\n', { d: 'double' }],
  ['conversion pipe to Tensor', 'let t := [0, 0] |> Tensor\n', { t: 'Tensor[double, (2,)]' }],
  ['inherited members', 'class A {\n  public final x : Int\n  new(x : Int) { this.x = x }\n  public function a() : Int = this.x\n}\nclass B extends A {\n  public final y : Int\n  new(x : Int) { this.x = x\n  this.y = x }\n  public function b() : Int = this.y\n}\nlet value := B(1)\nlet px := value.x\nlet pa := value.a()\n', { px: 'Int', pa: 'Int' }],
  ['enum case value', 'enum Shape {\n  Circle(double)\n  Square(double)\n}\nlet s := Shape.Circle(1.0)\n', { s: 'Shape' }],
  ['static method result', 'class A {\n  public static function make() : A = A()\n  public function hi() : Int = 1\n}\nlet a := A.make()\nlet h := a.hi()\n', { a: 'A', h: 'Int' }],
  ['static field result', 'class A {\n  public static final name : String = "x"\n}\nlet n := A.name\n', { n: 'String' }],
  ['constructor-declared fields', 'class Counter {\n  new(start : Int) {\n    final this.start : Int := start\n    var this.hits : Int := 0\n  }\n  public function value() : Int = this.start + this.hits\n}\nlet c := Counter(10)\nlet v := c.value()\n', { v: 'Int' }],
];

for (const [label, text, expected] of cases) {
  const types = typesOf(text);
  for (const [name, type] of Object.entries(expected)) {
    assert.strictEqual(types[name], type, `${label}: ${name}`);
  }
}

// Kestrel has no indexing on List, so the editor stays silent rather than guess.
assert.strictEqual(typesOf('let l := [1, 2]\nlet q := l[0]\n').q, '');

console.log('kestrel inference chain tests pass');
