'use strict';

const assert = require('assert');
const path = require('path');
const Module = require('module');

// The view's renderer is pure HTML. Load the extension through the same stub
// boundary the other editor tests use, so no VS Code process is needed.
const root = path.join(__dirname, '..');
const stub = () => ({ dispose() {} });
const vscode = new Proxy({
  window: {
    createOutputChannel: () => ({ appendLine() {}, dispose() {} }),
    onDidChangeTextEditorSelection: stub,
    setStatusBarMessage: stub,
    activeTextEditor: undefined
  },
  commands: { registerCommand: stub },
  workspace: {
    getConfiguration: () => ({ get: () => undefined }),
    onDidChangeConfiguration: stub,
    onDidSaveTextDocument: stub,
    getWorkspaceFolder: () => undefined,
    textDocuments: []
  },
  languages: new Proxy({}, { get: () => stub })
}, { get: (target, name) => (name in target ? target[name] : class Stub {}) });
const load = Module._load;
Module._load = function (request, parent, isMain) {
  if (request === 'vscode') return vscode;
  return load.call(this, request, parent, isMain);
};
const extension = require(path.join(root, 'out', 'extension.js'));
Module._load = load;

const { proofStateHtml, escapeHtml, expandSymbols } = extension;

assert.strictEqual(expandSymbols('x \\in A'), 'x ∈ A');
assert.strictEqual(expandSymbols('A \\subseteq B'), 'A ⊆ B');
assert.strictEqual(expandSymbols('\\forall x : U, x \\in A'), '∀ x : U, x ∈ A');
assert.strictEqual(expandSymbols('x \\notin A'), 'x ∉ A');
assert.strictEqual(expandSymbols('A \\cup B'), 'A ∪ B');
assert.strictEqual(expandSymbols('\\larr h'), '← h');
assert.strictEqual(expandSymbols('\\leftarrow'), '←');
assert.strictEqual(expandSymbols('\\ot h'), '← h');
assert.strictEqual(expandSymbols('A \\cap B'), 'A ∩ B');
assert.strictEqual(expandSymbols('A \\compl'), 'A ᶜ');
assert.strictEqual(expandSymbols('P \\iff Q'), 'P ↔ Q');
assert.strictEqual(expandSymbols('n \\ge 1'), 'n ≥ 1');
assert.strictEqual(expandSymbols('n \\neg P'), 'n ¬ P');

assert.strictEqual(escapeHtml('<script>"&'), '&lt;script&gt;&quot;&amp;');

const empty = proofStateHtml(null);
assert(empty.includes('No theorem here'));
assert(empty.includes('by { ... }'));

const off = proofStateHtml({ disabled: true });
assert(off.includes('Proof state is off'));
assert(off.includes('kestrel.proofState.enabled'));

const solved = proofStateHtml({ theorem: 'addZeroR', goals: [], complete: true, error: null });
assert(solved.includes('addZeroR'));
assert(solved.includes('proved'));
assert(solved.includes('No goals'));

const admitted = proofStateHtml({ theorem: 'sketch', goals: [], complete: true, admitted: true });
assert(admitted.includes('sorry'));
assert(admitted.includes('cannot discharge a function requirement'));
assert(!admitted.includes('badge complete'));

const stale = proofStateHtml({ theorem: 't', goals: [{ variables: [], hypotheses: [], target: 'P' }], complete: false, stale: true });
assert(stale.includes('Showing the last proof state'));

const branches = proofStateHtml({
  theorem: 'split',
  goals: [{ variables: [], hypotheses: [], target: 'n = n' }],
  cases: [
    { label: 'Zero', target: 'Zero = Zero', line: 3, column: 9, active: true },
    { label: 'Succ', target: 'Succ(k) = Succ(k)', line: 4, column: 9, active: false }
  ],
  complete: false
});
assert(branches.includes('Cases'));
assert(branches.includes('Zero'));
assert(branches.includes('Succ'));
assert(branches.includes('data-line="4"'));
assert(branches.includes('active'));
// The active row must not keep syntax colours that vanish on the theme's
// selection background.
assert(branches.includes('.case.active .case-label'));
assert(branches.includes('.case.active .case-target .symbol'));

const live = proofStateHtml({
  theorem: 'addZeroR',
  path: '/work/nat.kes',
  complete: false,
  error: null,
  goals: [
    {
      variables: [['k', 'Nat']],
      hypotheses: [['ih', 'add(k, 0) = k']],
      target: 'add(Succ(k), 0) = Succ(k)'
    },
    {
      variables: [],
      hypotheses: [],
      target: '∀ n : Nat, n = n'
    }
  ]
});
assert(live.includes('2 goals'));
assert(live.includes('Goal 1'));
const liveText = live.replace(/<[^>]+>/g, '');
assert(liveText.includes('add(Succ(k), 0) = Succ(k)'));
assert(liveText.includes('∀ n : Nat, n = n'));
assert(live.includes('symbol'), 'connectives are highlighted');
assert(live.includes('/work/nat.kes'));

// Text from the compiler is escaped before it reaches the webview.
const hostile = proofStateHtml({
  theorem: '<img src=x onerror=alert(1)>',
  goals: [{ variables: [], hypotheses: [['h', '<script>bad()</script>']], target: '<b>x</b>' }],
  complete: false,
  error: '<script>error()</script>'
});
assert(!hostile.includes('<img src=x'));
assert(!hostile.includes('<script>bad()'));
assert(!hostile.includes('<script>error()'));
assert(hostile.includes('&lt;script&gt;'));

// A goal without context still shows the turnstile and the target.
const bare = proofStateHtml({ theorem: 't', goals: [{ variables: [], hypotheses: [], target: 'True' }], complete: false });
assert(bare.includes('⊢'));
assert(bare.includes('True'));

console.log('kestrel proof-state tests pass');

const trusted = proofStateHtml({ theorem: 'nonlinear', goals: [], complete: true, trusted: true });
assert(trusted.includes('trusted SMT'));
assert(trusted.includes('without an independently checked certificate'));
assert(!trusted.includes('badge complete'));


// Math mode draws each proposition as an inline SVG. The printed proposition
// stays in the drawing's label, and the setting is opt-in, so the default panel
// is unchanged.
const goal = {
  theorem: 'unionPairFamily',
  goals: [{
    variables: [['A', 'Set[U]'], ['B', 'Set[U]'], ['x', 'U']],
    hypotheses: [['h', 'x ∈ ⋃₀ {A, B}']],
    target: 'x ∈ (A ∪ B)'
  }],
  complete: false,
  error: null
};
const typeset = proofStateHtml(goal, { math: true });
assert(typeset.includes('class="k-math"'), 'a proposition is typeset');
assert(typeset.includes('aria-label="x ∈ ⋃₀ {A, B}"'), 'the printed form stays available');
assert(typeset.includes('k-type'), 'a type is typeset too');
assert(!typeset.includes('<span class="symbol">'), 'the printed operators are replaced');
assert(typeset.includes('Set'), 'the type name survives');

const printed = proofStateHtml(goal);
assert(!printed.includes('class="k-math"'), 'math mode is opt-in');
assert(printed.includes('<span class="symbol">∪</span>'), 'the printed form is the fallback');

// A user mapping reaches the panel and replaces the guessed TeX for a glyph.
const custom = proofStateHtml({
  theorem: 'custom',
  goals: [{ variables: [], hypotheses: [], target: 'a ⊕ b' }],
  complete: false,
  error: null
}, { math: true, symbols: { '⊕': '\\oplus' } });
assert(custom.includes('class="k-math"'), 'a custom glyph still typesets');
assert(custom.includes('aria-label="a ⊕ b"'), 'the printed form stays available');

// A proposition that cannot be typeset keeps the printed form rather than
// leaving an empty line.
const unreadable = proofStateHtml({
  theorem: 't',
  goals: [{ variables: [], hypotheses: [], target: 'x ∈ {A' }],
  complete: false,
  error: null
}, { math: true });
assert(unreadable.includes('k-math'), 'an unbalanced brace still draws');
