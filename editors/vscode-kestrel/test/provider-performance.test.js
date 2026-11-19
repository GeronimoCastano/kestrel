'use strict';

// Provider-level timing on a large file. The bounds are intentionally loose;
// the quadratic version of these providers exceeded them by orders of
// magnitude, while a linear version passes on a busy machine.

const assert = require('assert');
const fs = require('fs');
const path = require('path');
const { loadExtension, documentFor, labelsAt, hoverAt, signatureAt, inlayHints } = require('../test-support/harness');

const { providers } = loadExtension();

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
  parts.push(`class Link {
    public final value : Int
    new(value : Int) { this.value = value }
    public function next() : Link = Link(this.value + 1)
}
let link := Link(0)
let deep := link.next().next().next().next().next()
`);
  return parts.join('\n');
}

function measure(fn) {
  const started = process.hrtime.bigint();
  const result = fn();
  return { result, ms: Number(process.hrtime.bigint() - started) / 1e6 };
}

const text = program(2200);
const chain = `${text}link.next().next().next().next().next().`;
const completions = measure(() => labelsAt(providers, chain, chain.length));
assert(completions.result.includes('next'), `a deep chain offers next: ${completions.result.slice(0, 10)}`);
assert(completions.ms < 2000, `deep chain completion took ${completions.ms.toFixed(0)} ms`);
const warmCompletion = measure(() => labelsAt(providers, chain, chain.length));
assert(warmCompletion.ms < 250, `warm deep chain completion took ${warmCompletion.ms.toFixed(0)} ms`);

const hoverSource = `${text}link.next().next().next().next().next().value\n`;
const hover = measure(() => hoverAt(providers, hoverSource, hoverSource.indexOf('.value') + 2));
assert(hover.result && hover.result.contents.value.includes('field value : Int'), 'deep chain hover on a large file');
assert(hover.ms < 1000, `deep chain hover took ${hover.ms.toFixed(0)} ms`);

const signatureSource = `${text}link.next().next().next().next(\n`;
const signature = measure(() => signatureAt(providers, signatureSource, signatureSource.length));
assert(signature.result && signature.result.signatures[0].label.includes('next() : Link'), 'deep chain signature on a large file');
assert(signature.ms < 1000, `deep chain signature took ${signature.ms.toFixed(0)} ms`);

const viewportStart = chain.length - 5000;
const inlay = measure(() => inlayHints(providers, chain, Math.max(0, viewportStart), chain.length));
assert(inlay.ms < 2000, `large-file inlay hints took ${inlay.ms.toFixed(0)} ms`);
assert(inlay.result.some(hint => hint.label === ': Link'), 'the visible chain still gets a Link hint');

const tokens = measure(() => providers.tokens.provideDocumentSemanticTokens(documentFor(chain)));
assert(tokens.ms < 2000, `large-file semantic tokens took ${tokens.ms.toFixed(0)} ms`);

// The real 4k-line library is the worst case the editor meets in this repo.
const libraryPath = path.join(__dirname, '..', '..', '..', 'src', 'kesdas.kes');
if (fs.existsSync(libraryPath)) {
  const library = fs.readFileSync(libraryPath, 'utf8');
  const probe = `${library}\nlet libraryProbe := 1\nlibraryProbe.`;
  const libraryCompletion = measure(() => labelsAt(providers, probe, probe.length));
  assert(libraryCompletion.ms < 2000, `library completion took ${libraryCompletion.ms.toFixed(0)} ms`);
  const libraryHover = measure(() => hoverAt(providers, library, library.indexOf('sortValues') + 1));
  assert(libraryHover.ms < 1000, `library hover took ${libraryHover.ms.toFixed(0)} ms`);
  const libraryInlay = measure(() => inlayHints(providers, library, library.length - 5000, library.length));
  assert(libraryInlay.ms < 2000, `library inlay hints took ${libraryInlay.ms.toFixed(0)} ms`);
  const libraryTokens = measure(() => providers.tokens.provideDocumentSemanticTokens(documentFor(library)));
  assert(libraryTokens.ms < 2000, `library semantic tokens took ${libraryTokens.ms.toFixed(0)} ms`);
}

console.log('kestrel provider performance tests pass');
