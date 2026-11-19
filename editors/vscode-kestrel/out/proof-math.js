'use strict';

// The proof state prints a proposition as Unicode Kestrel text, and a monospace
// face draws some operators at the cap height of a letter: in
// `h : x ∈ ⋃₀ {A, B}` beside `A : Set[U]`, the cup and the indexed union read as
// the type variable U. This module typesets the same text instead. It maps the
// spelling the checker already printed to TeX and returns an inline SVG, which
// keeps the glyphs apart and cannot be confused with an identifier.
//
// It is a view: the propositions arrive here already rendered, and nothing in
// the checker depends on it. A proposition that cannot be typeset falls back to
// the plain text, so a mapping gap costs appearance, never information.

// Surface spelling to TeX. The left side is what `Prop`'s and `Term`'s display
// code prints, so the table mirrors those two printers rather than the parser:
// the user may extend it through `kestrel.proofState.mathSymbols`.
const SYMBOLS = {
  '⋃₀': '\\bigcup_{0}',
  '⋂₀': '\\bigcap_{0}',
  '->': '\\to',
  '=>': '\\Rightarrow',
  '∈': '\\in',
  '∉': '\\notin',
  '⊆': '\\subseteq',
  '⊂': '\\subset',
  '∪': '\\cup',
  '∩': '\\cap',
  '∧': '\\land',
  '∨': '\\lor',
  '¬': '\\neg',
  '→': '\\to',
  '↔': '\\iff',
  '⇒': '\\Rightarrow',
  '↦': '\\mapsto',
  '∀': '\\forall',
  '∃': '\\exists',
  '∅': '\\emptyset',
  // `Aᶜᶜ` would otherwise reach TeX as a double superscript, which it refuses;
  // the empty base lets each complement attach on its own.
  'ᶜ': '{}^{c}',
  '≠': '\\ne',
  '≤': '\\le',
  '≥': '\\ge',
  '⊢': '\\vdash',
  '×': '\\times',
  '·': '\\cdot',
  'ℕ': '\\mathbb{N}',
  'ℤ': '\\mathbb{Z}',
  'ℚ': '\\mathbb{Q}',
  'ℝ': '\\mathbb{R}',
  'ℂ': '\\mathbb{C}',
  '−': '-'
};

// The glyphs the panel colours today. A typeset operator keeps that colour, so
// switching the proof state to math changes the font, not the palette.
const COLOURED = [
  '\\bigcup_{0}', '\\bigcap_{0}', '\\forall', '\\exists', '\\to', '\\iff',
  '\\land', '\\lor', '\\neg', '\\vdash', '\\in', '\\notin', '\\subseteq',
  '\\subset', '\\cup', '\\cap', '\\emptyset', '\\mathbb{N}', '\\mathbb{Z}',
  '\\mathbb{Q}', '\\mathbb{R}', '\\mathbb{C}'
];

// Words the second printer (`Display for Prop`) uses in error text. The goal
// printer writes the Unicode forms above, but both can reach the panel.
const WORDS = {
  and: '\\land',
  or: '\\lor',
  not: '\\neg',
  forall: '\\forall',
  exists: '\\exists',
  true: '\\mathrm{True}',
  false: '\\mathrm{False}',
  fun: '\\lambda',
  match: '\\mathsf{match}',
  if: '\\mathsf{if}',
  then: '\\mathsf{then}',
  else: '\\mathsf{else}'
};

const CHARS = {
  '{': '\\{',
  '}': '\\}',
  '|': '\\mid',
  '_': '\\_',
  '%': '\\%',
  '&': '\\&',
  '#': '\\#',
  '$': '\\$'
};

const CACHE_LIMIT = 200;
const cache = new Map();
const KEYS = Object.keys(SYMBOLS).sort((a, b) => b.length - a.length);
let engine = null;

function engineFor() {
  if (engine === null) {
    try {
      engine = require('./math-svg');
    } catch (error) {
      engine = false;
    }
  }
  return engine || null;
}

function escapeBraced(name) {
  return name.replace(/([_&#%$])/g, '\\$1');
}

// Split printed text into the pieces the table knows. The longest spelling
// wins, so `⋃₀` is one piece and `=>` is not `=` followed by `>`.
function tokenize(text) {
  const pieces = [];
  let i = 0;
  while (i < text.length) {
    const char = String.fromCodePoint(text.codePointAt(i));
    if (/\s/.test(char)) {
      i += char.length;
      continue;
    }
    let surface = null;
    for (const key of KEYS) {
      if (text.startsWith(key, i)) {
        surface = key;
        break;
      }
    }
    if (surface !== null) {
      pieces.push({ kind: 'symbol', text: surface });
      i += surface.length;
      continue;
    }
    const rest = text.slice(i);
    const word = /^[\p{L}_][\p{L}\p{N}_'′]*/u.exec(rest);
    if (word) {
      // A modifier letter is an identifier character too, so `Aᶜ` would arrive
      // as one name. Cut the word where a known spelling starts instead.
      let length = word[0].length;
      for (const key of KEYS) {
        const at = word[0].indexOf(key);
        if (at > 0 && at < length) length = at;
      }
      pieces.push({ kind: 'name', text: word[0].slice(0, length) });
      i += length;
      continue;
    }
    const number = /^\d+(?:\.\d+)?/.exec(rest);
    if (number) {
      pieces.push({ kind: 'number', text: number[0] });
      i += number[0].length;
      continue;
    }
    pieces.push({ kind: 'char', text: char });
    i += char.length;
  }
  return pieces;
}

// A quantifier binds the term after it, and TeX sets no space of its own there,
// so the binder would touch its variable. The thin space is a separate node, so
// the operator keeps its own source spelling for the colour lookup.
const BINDING = new Set(['\\forall', '\\exists']);

function nameTex(name, isCall) {
  if (WORDS[name] !== undefined) return WORDS[name];
  if (isCall) return `\\mathsf{${escapeBraced(name)}}`;
  if (name.length === 1) return escapeBraced(name);
  if (/^\p{Lu}/u.test(name)) return `\\mathrm{${escapeBraced(name)}}`;
  return `\\mathit{${escapeBraced(name)}}`;
}

// Build the TeX for one proposition. A control word must be followed by a
// space when the next piece starts with a letter, or `\in` and `A` would read
// as one command.
function toTex(text, overrides) {
  const table = overrides && typeof overrides === 'object' ? { ...SYMBOLS, ...overrides } : SYMBOLS;
  const pieces = tokenize(text);
  let out = '';
  pieces.forEach((piece, index) => {
    let tex;
    if (piece.kind === 'symbol') {
      const mapped = table[piece.text];
      tex = mapped === undefined ? `\\text{${piece.text}}` : mapped;
    } else if (piece.kind === 'number') {
      tex = piece.text;
    } else if (piece.kind === 'char') {
      // An unmapped non-ASCII glyph becomes text, so it stays visible instead
      // of leaving a gap in the drawing. The user table can still claim it.
      if (table[piece.text]) tex = table[piece.text];
      else if (CHARS[piece.text]) tex = CHARS[piece.text];
      else if (piece.text.codePointAt(0) > 127) tex = `\\text{${piece.text}}`;
      else tex = piece.text;
    } else {
      const next = pieces[index + 1];
      tex = nameTex(piece.text, Boolean(next && next.kind === 'char' && next.text === '('));
    }
    if (!tex) return;
    if (/\\[a-zA-Z]+$/.test(out) && /^[a-zA-Z]/.test(tex)) out += ' ';
    out += tex;
    if (BINDING.has(tex) && index + 1 < pieces.length) out += '\\,';
  });
  return out;
}

function attribute(tag, name) {
  const found = new RegExp(`${name}="([^"]*)"`).exec(tag);
  return found ? found[1] : null;
}

// MathJax marks every group with its MathML node and its TeX source, so the
// roles the panel colours today can be recovered after typesetting. The
// `fill="currentColor"` is what lets a CSS `color` rule reach the glyphs: the
// root group resolves `fill` once, so a nested group needs its own keyword.
function decorate(svg) {
  const coloured = new Set(COLOURED);
  return svg.replace(/<g\b[^>]*>/g, tag => {
    const kind = attribute(tag, 'data-mml-node');
    const latex = attribute(tag, 'data-latex') || '';
    let role = null;
    // `⋃₀` typesets as an under-script around the cup, so the indexed forms are
    // matched on the group that carries both the operator and its subscript.
    // TeX calls `∀` and `∃` ordinary symbols and MathJax emits them as `mi`, so
    // the operator node kinds are checked together.
    if ((kind === 'mo' || kind === 'mi' || kind === 'munder' || kind === 'mover') && coloured.has(latex)) role = 'k-sym';
    else if (latex.startsWith('\\mathrm{True}') || latex.startsWith('\\mathrm{False}')) role = 'k-lit';
    else if (latex.startsWith('\\mathsf')) role = 'k-call';
    else if (latex.startsWith('\\mathrm')) role = 'k-type';
    if (!role) return tag;
    if (/\bclass="/.test(tag)) return tag.replace('class="', `class="${role} `).replace('<g ', '<g fill="currentColor" ');
    return tag.replace('<g ', `<g class="${role}" fill="currentColor" `);
  });
}

/**
 * Typeset one proposition. Returns an inline SVG, or null when the text cannot
 * be rendered, so the caller falls back to the printed form.
 */
function inlineSvg(text, overrides) {
  const render = engineFor();
  if (!render || typeof text !== 'string' || !text.trim()) return null;
  const key = `${JSON.stringify(overrides || null)}\u0000${text}`;
  const hit = cache.get(key);
  if (hit !== undefined) return hit;
  let result = null;
  try {
    const svg = render(toTex(text, overrides), false);
    if (svg && svg.startsWith('<svg') && svg.length < 200000) {
      const label = text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;');
      result = decorate(svg).replace('<svg ', `<svg class="k-math" aria-label="${label}" `);
    }
  } catch (error) {
    result = null;
  }
  if (cache.size >= CACHE_LIMIT) cache.delete(cache.keys().next().value);
  cache.set(key, result);
  return result;
}

function clearCache() {
  cache.clear();
}

module.exports = { SYMBOLS, COLOURED, WORDS, toTex, inlineSvg, tokenize, clearCache };
