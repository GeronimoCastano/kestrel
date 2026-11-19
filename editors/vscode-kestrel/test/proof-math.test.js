'use strict';

const assert = require('assert');
const fs = require('fs');
const path = require('path');

const math = require('../out/proof-math');

// The printed spelling is what the two display printers emit, so these are the
// inputs the panel really receives.
assert.strictEqual(math.toTex('x ∈ A'), 'x\\in A');
assert.strictEqual(math.toTex('x ∈ (A ∪ B)'), 'x\\in(A\\cup B)');
assert.strictEqual(math.toTex('x ∈ ⋃₀ {A, B}'), 'x\\in\\bigcup_{0}\\{A,B\\}');
assert.strictEqual(math.toTex('x ∈ ⋂₀ F'), 'x\\in\\bigcap_{0}F');
assert.strictEqual(math.toTex('⋃₀ {A, B} ⊆ (A ∪ B)'), '\\bigcup_{0}\\{A,B\\}\\subseteq(A\\cup B)');
assert.strictEqual(math.toTex('∀ x : U, x ∈ A → x ∈ B'), '\\forall\\,x:U,x\\in A\\to x\\in B');
assert.strictEqual(math.toTex('¬(x ∈ A)'), '\\neg(x\\in A)');
assert.strictEqual(math.toTex('x ≠ 3'), 'x\\ne3');
assert.strictEqual(math.toTex('a ↔ b'), 'a\\iff b');

// Identifiers keep their own shape: one letter is math italic, a longer name is
// wrapped so TeX does not read it as a product, and a type name stays upright.
assert.strictEqual(math.toTex('f(n) = f(n)'), '\\mathsf{f}(n)=\\mathsf{f}(n)');
assert.strictEqual(math.toTex('hx'), '\\mathit{hx}');
assert.strictEqual(math.toTex('Set[U]'), '\\mathrm{Set}[U]');
assert.strictEqual(math.toTex('a contradiction'), 'a\\mathit{contradiction}');

// The second printer spells connectives as words.
assert.strictEqual(math.toTex('a and b'), 'a\\land b');
assert.strictEqual(math.toTex('a or b'), 'a\\lor b');
assert.strictEqual(math.toTex('not a'), '\\neg a');
assert.strictEqual(math.toTex('true'), '\\mathrm{True}');

// Longest match wins, so an indexed union is one spelling and not a union
// followed by a digit.
assert.deepStrictEqual(math.tokenize('⋃₀ F').map(piece => piece.text), ['⋃₀', 'F']);
assert.deepStrictEqual(math.tokenize('A => B').map(piece => piece.text), ['A', '=>', 'B']);

// A modifier letter is an identifier character, so `Aᶜ` must still split.
assert.strictEqual(math.toTex('Aᶜ ⊆ B'), 'A{}^{c}\\subseteq B');
assert.strictEqual(math.toTex('(A ∪ B)ᶜ'), '(A\\cup B){}^{c}');
assert.deepStrictEqual(math.tokenize('Aᶜ').map(piece => piece.text), ['A', 'ᶜ']);

// A control word followed by a letter needs a space, or TeX reads one command.
assert(math.toTex('x ∈ A').includes('\\in A'));
assert(!math.toTex('x ∈ A').includes('\\inA'));

// Punctuation that carries meaning in TeX is escaped.
assert.strictEqual(math.toTex('{x : U | x ∈ A}'), '\\{x:U\\mid x\\in A\\}');
assert.strictEqual(math.toTex('h_x'), '\\mathit{h\\_x}');

// A glyph without a mapping is visible as text rather than dropped, and the
// user table can fill the gap.
assert(math.toTex('x ★ y').includes('\\text{★}'));
assert(math.toTex('a ⊕ b', { '⊕': '\\oplus' }).includes('\\oplus'));

// Every proposition the checker printed in the sample games typesets, and none
// of them needed the text fallback. This is the guard against a new printer
// glyph arriving without a mapping.
const corpus = JSON.parse(fs.readFileSync(path.join(__dirname, 'fixtures', 'goal-props.json'), 'utf8'));
assert(corpus.length >= 20);
for (const proposition of corpus) {
  const tex = math.toTex(proposition);
  assert(!tex.includes('\\text{'), `no fallback glyph in ${proposition}`);
  assert(!/[^\x00-\x7F]/.test(tex), `no raw glyph left in ${proposition}: ${tex}`);
}

// Each rendered proposition is an inline drawing that inherits the theme
// colour: a literal hex here would break every theme but the one it was
// written for.
const target = math.inlineSvg('x ∈ ⋃₀ {A, B}');
assert(target.startsWith('<svg'), 'a proposition renders');
assert(target.includes('class="k-math"'), 'the drawing is marked for panel CSS');
assert(target.includes('k-sym'), 'operators carry the symbol role');
assert(target.includes('currentColor'), 'the drawing inherits the theme colour');
assert(!/#[0-9a-fA-F]{3,6}/.test(target), 'no baked colour');
assert(target.includes('aria-label="x ∈ ⋃₀ {A, B}"'), 'the printed text stays available');
assert(target.includes('data-c="2208"'), 'the membership glyph comes from the TeX font');
assert(target.includes('data-c="22C3"'), 'the indexed union glyph comes from the TeX font');

// A quantifier binds the term after it and still carries the operator colour.
// MathJax emits `∀` as an identifier node, so this is the regression guard for
// its colour lookup.
assert(math.inlineSvg('∀ x : U').includes('k-sym'), 'a quantifier is coloured');
assert(math.inlineSvg('∃ s : Set[U], s ∈ F').includes('k-sym'), 'an existential is coloured');

// Roles line up with the printed form the panel colours today.
assert(math.inlineSvg('Set[U]').includes('k-type'));
assert(math.inlineSvg('add(n, m)').includes('k-call'));
assert(math.inlineSvg('True').includes('k-lit'));
assert(!math.inlineSvg('x ∈ A').includes('k-type'));

// Every sample proposition draws. The panel falls back to text on null, so a
// null here is a mapping regression that the tests should name.
for (const proposition of corpus) {
  assert(math.inlineSvg(proposition), `renders ${proposition}`);
}

// Nothing that cannot be drawn throws: the panel keeps its printed form.
assert.strictEqual(math.inlineSvg(''), null);
assert.strictEqual(math.inlineSvg(undefined), null);
assert.strictEqual(math.inlineSvg(null), null);
assert.strictEqual(math.inlineSvg('   '), null);
// An escaped brace is a glyph, not a group, so an unbalanced one still draws.
assert(math.inlineSvg('x ∈ {A'));
// Oversized input is refused rather than thrown, so the panel keeps text.
const long = 'x ∈ ' + Array.from({ length: 4000 }, (_, index) => `A${index}`).join(' ∪ ');
assert.strictEqual(math.inlineSvg(long), null);

// A repeated proposition is served from the cache and the cache can be cleared.
const first = math.inlineSvg('a ∪ b');
assert.strictEqual(math.inlineSvg('a ∪ b'), first);
math.clearCache();
assert.strictEqual(math.inlineSvg('a ∪ b'), first);
