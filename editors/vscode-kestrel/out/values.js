'use strict';

const kestrel = require('./kestrel');
const infer = require('./infer');
const builtins = require('./builtins');

// The standard constants are named with capital letters, so they look like
// types to the grammar. This set is fixed at load time.
const CONSTANT_NAMES = new Set(builtins.entries
  .filter(entry => !entry.receiver && !entry.static && entry.signature.startsWith(`${entry.name} :`))
  .map(entry => entry.name));

// Semantic tokens are requested after each edit for the whole document. Caching
// the answer for the current text keeps a second request (and the providers that
// run just after it) from repeating the scan.
const CACHE_LIMIT = 4;
const valueCache = new Map();
function cached(text) {
  const found = valueCache.get(text);
  if (found === undefined) return undefined;
  valueCache.delete(text);
  valueCache.set(text, found);
  return found;
}
function remember(text, found) {
  valueCache.set(text, found);
  if (valueCache.size > CACHE_LIMIT) valueCache.delete(valueCache.keys().next().value);
}

// Uppercase spelling alone cannot distinguish a value from a type. Override the
// grammar for declared values, using their block scopes and leaving type annotations alone.
function valueUses(text, model) {
  const hit = cached(text);
  if (hit !== undefined) return hit;
  const clean = kestrel.sanitize(text);
  const scopes = [];
  const stack = [];
  for (let i = 0; i < clean.length; i += 1) {
    if ('{['.includes(clean[i])) {
      const scope = { start: i, end: clean.length, kind: clean[i] };
      scopes.push(scope);
      stack.push(scope);
    } else if ('}]'.includes(clean[i])) {
      if (stack.length) stack.pop().end = i + 1;
    }
  }
  const enclosing = (position, kind) => scopes.filter(scope => scope.kind === kind
    && scope.start < position && position < scope.end).at(-1);
  const symbols = [];
  const typeRanges = [];
  const add = (name, start, from = start, end = null) => {
    if (!/^[A-Z]/.test(name)) return;
    symbols.push({ name, start, from, end: end ?? enclosing(start, '{')?.end ?? clean.length });
  };

  for (const binding of [...infer.bindings(text), ...infer.loopBindings(text, clean)]) {
    // Tuple components are added by the dedicated destructuring pass below, which
    // also covers names that a shared right side would otherwise hide.
    if (binding.component !== undefined) continue;
    const { name, nameStart, nameEnd, rhsStart } = binding;
    typeRanges.push([nameEnd, rhsStart]);
    let end;
    if (binding.kind === 'loop') {
      const body = scopes.find(scope => scope.kind === '{' && scope.start > binding.rhsEnd
        && !clean.slice(binding.rhsEnd + 1, scope.start).trim());
      end = body?.end ?? binding.rhsEnd;
    }
    add(name, nameStart, rhsStart, end);
  }
  for (const constant of model.constants) {
    add(constant.name, constant.nameIndex);
    const equals = clean.indexOf('=', constant.nameIndex + constant.name.length);
    if (equals >= 0) typeRanges.push([constant.nameIndex + constant.name.length, equals]);
  }
  // Bare := bindings and tuple destructuring have no inferred type hint yet.
  for (const match of clean.matchAll(/(?<![\w.])([A-Z]\w*)\s*:=/g)) add(match[1], match.index);
  for (const match of clean.matchAll(/\b(?:let|var)\s+(?:mut\s+)?\(([^()]*)\)\s*:=/g)) {
    const offset = match.index + match[0].indexOf('(') + 1;
    for (const name of match[1].matchAll(/[A-Z]\w*/g)) add(name[0], offset + name.index);
  }
  for (const match of clean.matchAll(/\bfor\s+([A-Z]\w*)\s+in\b/g)) {
    const scope = enclosing(match.index, '[');
    if (scope) add(match[1], match.index + match[0].indexOf(match[1]), scope.start, scope.end);
  }
  for (const fn of [...model.functions, ...model.constructors]) {
    typeRanges.push([fn.nameIndex, fn.bodyStart ?? fn.paramsClose + 1]);
  }
  for (const declaration of model.classes) typeRanges.push([declaration.start, declaration.bodyStart]);
  for (const alias of model.aliases) {
    const end = clean.indexOf('\n', alias.start);
    typeRanges.push([alias.start, end < 0 ? clean.length : end]);
  }
  // A byte per position is cheaper than testing every range for every name.
  const inType = new Uint8Array(clean.length + 1);
  for (const [from, to] of typeRanges) {
    const start = Math.max(0, from);
    const end = Math.min(clean.length, to);
    if (end > start) inType.fill(1, start, end);
  }
  const symbolsByName = new Map();
  for (const symbol of symbols) {
    const list = symbolsByName.get(symbol.name);
    if (list) list.push(symbol);
    else symbolsByName.set(symbol.name, [symbol]);
  }
  const found = [];
  for (const match of clean.matchAll(/\b[A-Z][A-Za-z0-9_]*\b/g)) {
    const name = match[0];
    const start = match.index;
    if (inType[start]) continue;
    let before = start - 1;
    while (before >= 0 && /\s/.test(clean[before])) before -= 1;
    const qualified = before >= 0 && clean[before] === '.';
    const visible = !qualified && (symbolsByName.get(name) || []).some(symbol =>
      symbol.start === start || (symbol.from <= start && start < symbol.end));
    if (visible || CONSTANT_NAMES.has(name)) found.push({ start, length: name.length });
  }
  remember(text, found);
  return found;
}

module.exports = { valueUses };
