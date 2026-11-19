'use strict';
const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { fileURLToPath } = require('url');
const math = require('../out/documentation');
const kestrel = require('../out/kestrel');
const { loadExtension, hoverAt, labelsAt, signatureAt, documentFor } = require('../test-support/harness');
const storage = fs.mkdtempSync(path.join(os.tmpdir(), 'kestrel-math-test-'));
const { providers } = loadExtension();
math.configure(storage);
function svg(markdown) {
  const url = /(?:\]\(|src=")(?<url>file:\/\/[^)"]+\.svg)(?:\)|")/.exec(markdown)?.groups.url;
  assert(url, `Rendered image in ${markdown}`);
  const text = fs.readFileSync(fileURLToPath(url), 'utf8');
  assert(text.startsWith('<svg'));
  assert(text.includes('<path'), 'glyphs are included offline');
  assert(!/<script|(?:href|src)="(?:https?:|file:)/.test(text), 'self-contained math');
  return text;
}
try {
  const relation = svg(math.render(String.raw`$h(s) = \infty$`));
  for (const glyph of ['3D', '221E']) assert(relation.includes(`data-c="${glyph}"`), `complete relation: ${glyph}`);
  const domain = svg(math.render(String.raw`$h : S \to \mathbb{N}_0 \cup \{\infty\}$`));
  for (const glyph of ['2192', '2115', '222A', '221E']) assert(domain.includes(`data-c="${glyph}"`), `complete domain: ${glyph}`);
  for (const size of [10, 14, 20, 28]) {
    const rendered = svg(math.render('$h(s)$', '#333333', size));
    const box = /viewBox="([^"]+)"/.exec(rendered)[1].split(' ').map(Number);
    assert(Math.abs(box[1] + box[3] / 2 + 250) < 0.01, 'inline math is centered on its axis');
    assert(Math.abs(Number(/height="([^"]+)"/.exec(rendered)[1]) - box[3] * size / 1000) < 0.01, 'configured font scale');
  }
  assert(!math.render('Literal <img src="file:///tmp/untrusted.svg"> and $x$').includes('untrusted.svg'), 'authored HTML stays inert');
  assert(math.render('<https://example.invalid> and $x$').includes('<https://example.invalid>'), 'autolinks survive');
  assert(math.render('`<img>` and $x$').includes('`<img>`'), 'HTML in code stays literal');
  for (const expression of [String.raw`\frac{x^2}{2}`, String.raw`\sum_{i=0}^{n} i`, String.raw`\alpha + \beta \leq \infty`, String.raw`\begin{pmatrix}1&2\\3&4\end{pmatrix}`, String.raw`\sqrt{a^2+b^2}`]) {
    svg(math.render(`A formula: $${expression}$ and text.`));
    svg(math.render(`Before\n\n$$\n${expression}\n$$\n\nAfter`));
  }
  for (const source of [String.raw`A price of \$5 and \$10.`, '`$x$`', '``$x$``', '```latex\n$x$\n```', '~~~\n$$x$$\n~~~', '[cost](https://example.invalid/$x$)', '$unfinished', '$\\notARealMacro{x}$', '$ x $', '$5 and $10']) {
    assert.strictEqual(math.render(source), source, source);
  }
  assert.strictEqual(math.render(String.raw`$\def\loop{\loop}\loop$`), String.raw`$\def\loop{\loop}\loop$`, 'recursive expansion is bounded');
  svg(math.render(String.raw`$\newcommand{\thing}{x}\thing$`));
  assert.strictEqual(math.render(String.raw`$\thing$`), String.raw`$\thing$`, 'macros cannot leak between comments');
  assert(svg(math.render('$x$', '#e0e0e0')).includes('#e0e0e0'));
  assert(svg(math.render('$x$', '#333333')).includes('#333333'));
  const source = String.raw`/// Computes $\frac{x^2}{2}$.
function energy(x : Int) : Int = x
/// A class with $r^2$.
class Circle {
    new() {}
}
energy(2)
`;
  const hover = hoverAt(providers, source, source.lastIndexOf('energy') + 2);
  svg(hover.contents.value);
  assert.strictEqual(hover.contents.supportHtml, true, 'generated alignment markup is enabled');
  assert(!hover.contents.isTrusted, 'math never enables commands');
  assert(hover.contents.value.includes('```kestrel'), 'signature stays a code block');
  svg(hoverAt(providers, source, source.indexOf('frac')).contents.value);
  svg(hoverAt(providers, source, source.indexOf('class Circle') + 8).contents.value);
  // The class keeps its equations when its constructor has a note of its own,
  // and the constructor keeps its note under its own signature.
  const noted = String.raw`/// The goal-count heuristic $h^{GC}$.
///
/// $$h^{GC}(s) = \left|\{\, f \in G : s(f.\mathrm{variable}) \neq f.\mathrm{value} \,\}\right|$$
public class NotedHeuristic {
    /// Builds the heuristic when the problem is known.
    public new(problem : Int) {
        this.problem = problem
    }
}
`;
  const notedHover = hoverAt(providers, noted, noted.indexOf('class NotedHeuristic') + 8).contents.value;
  svg(notedHover);
  assert(notedHover.includes('Builds the heuristic when the problem is known.'), 'constructor documentation is not lost');
  assert(
    notedHover.indexOf('goal-count heuristic') < notedHover.indexOf('Builds the heuristic when the problem'),
    'class documentation leads the hover'
  );
  const doc = documentFor(source);
  const items = providers.completion.provideCompletionItems(doc, doc.positionAt(source.length));
  svg(items.find(item => item.label === 'energy').documentation.value);
  svg(signatureAt(providers, source, source.lastIndexOf('energy(') + 7).signatures[0].documentation.value);
  const trailing = kestrel.parseDocument('/// Keeps two spaces.  \r\n/// Next line.\r\nfunction f():Int=1');
  assert.strictEqual(trailing.functions[0].documentation, 'Keeps two spaces.  \nNext line.');
  for (let i = 0; i < 140; i += 1) math.render(`$x+${i}$`);
  assert(fs.readdirSync(path.join(storage, 'doc-math-v2')).length <= 128, 'bounded equation cache');
} finally { fs.rmSync(storage, { recursive: true, force: true }); }
console.log('Documentation math, scope, Markdown and provider tests pass');
