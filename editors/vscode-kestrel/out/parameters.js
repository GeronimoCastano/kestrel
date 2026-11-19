'use strict';

// Finds every use of a lambda or function parameter, so the editor can color
// each use like its declaration. The editor does not run the compiler, so a
// lambda body is read with bracket and line-end rules. That is enough for
// colors: Kestrel has no shadowing within a function, so a name inside a body
// means the parameter.

const kestrel = require('./kestrel');

const IDENTIFIER = '[A-Za-z_][A-Za-z0-9_]*';

// `x =>` and `x : Int =>`. The lookbehind skips member names such as `p.x`.
const SINGLE_LAMBDA = new RegExp(`(?<![A-Za-z0-9_.])(${IDENTIFIER})(?:\\s*:[^=(){}\\n]*?)?\\s*=>`, 'g');
// `(a : Int, b : Int) =>`
const GROUP_LAMBDA = /\(([^()]*)\)\s*=>/g;

function bodyEnd(clean, start) {
  let i = start;
  while (i < clean.length && /\s/.test(clean[i])) i += 1;
  if (clean[i] === '{') {
    const close = findClose(clean, i);
    return close < 0 ? clean.length : close + 1;
  }
  let depth = 0;
  for (let j = i; j < clean.length; j += 1) {
    const current = clean[j];
    if ('([{'.includes(current)) {
      depth += 1;
    } else if (')]}'.includes(current)) {
      if (depth === 0) return j;
      depth -= 1;
    } else if (depth === 0 && (current === ',' || current === '\n')) {
      return j;
    }
  }
  return clean.length;
}

function findClose(clean, open) {
  let depth = 0;
  for (let i = open; i < clean.length; i += 1) {
    if (clean[i] === '{') depth += 1;
    else if (clean[i] === '}') {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  return -1;
}

function usesWithin(clean, name, from, to) {
  const found = [];
  const pattern = new RegExp(`(?<![A-Za-z0-9_.])${name}(?![A-Za-z0-9_])`, 'g');
  const part = clean.slice(from, to);
  let match;
  while ((match = pattern.exec(part)) !== null) {
    found.push({ start: from + match.index, length: name.length });
  }
  return found;
}

function lambdaParameters(clean) {
  const lambdas = [];
  let match;
  SINGLE_LAMBDA.lastIndex = 0;
  while ((match = SINGLE_LAMBDA.exec(clean)) !== null) {
    const prefix = clean.slice(0, match.index);
    if (/[,{]\s*$/.test(prefix) && /\bmatch\b[^{}]*\{[^{}]*$/.test(prefix)) continue;
    const arrow = match.index + match[0].length;
    lambdas.push({
      heads: [{ name: match[1], start: match.index, length: match[1].length }],
      from: arrow,
      to: bodyEnd(clean, arrow)
    });
  }
  GROUP_LAMBDA.lastIndex = 0;
  while ((match = GROUP_LAMBDA.exec(clean)) !== null) {
    // A match arm such as `Some(x) => ...` binds names; it is not a lambda.
    if (/[A-Z][A-Za-z0-9_]*\s*$/.test(clean.slice(0, match.index))) continue;
    const heads = [];
    let offset = match.index + 1;
    for (const part of match[1].split(',')) {
      const head = new RegExp(`^(\\s*)(${IDENTIFIER})`).exec(part);
      if (head) {
        heads.push({ name: head[2], start: offset + head[1].length, length: head[2].length });
      }
      offset += part.length + 1;
    }
    const arrow = match.index + match[0].length;
    lambdas.push({ heads, from: arrow, to: bodyEnd(clean, arrow) });
  }
  return lambdas;
}

// Returns { start, length } for each parameter name, sorted by position. The
// answer is pure for a given text, and semantic tokens ask for it repeatedly.
const CACHE_LIMIT = 4;
const useCache = new Map();

function parameterUses(text, model) {
  const hit = useCache.get(text);
  if (hit !== undefined) {
    useCache.delete(text);
    useCache.set(text, hit);
    return hit;
  }
  const clean = kestrel.sanitize(text);
  const found = new Map();
  const add = token => found.set(`${token.start}`, token);

  for (const lambda of lambdaParameters(clean)) {
    for (const head of lambda.heads) {
      add(head);
      for (const use of usesWithin(clean, head.name, lambda.from, lambda.to)) add({ ...use, name: head.name });
    }
  }

  for (const fn of [...model.functions, ...model.theorems]) {
    for (const parameter of fn.parameters || []) {
      if (!parameter.name) continue;
      for (const use of usesWithin(clean, parameter.name, fn.paramsOpen + 1, fn.end)) add(use);
    }
  }

  for (const theorem of model.theorems) {
    const pattern = /\b[A-Z]\w*\s*\([^()]*\)\s*((?:[a-z_]\w*\s*)+)=>/g;
    pattern.lastIndex = theorem.bodyStart;
    let branch;
    while ((branch = pattern.exec(clean)) && branch.index < theorem.end) {
      const from = pattern.lastIndex, end = bodyEnd(clean, from);
      const head = branch.index + branch[0].lastIndexOf(branch[1]);
      for (const name of branch[1].match(/[a-z_]\w*/g) || []) {
        for (const use of usesWithin(clean, name, head, end)) add(use);
      }
    }
  }
  const uses = [...found.values()]
    .map(({ start, length }) => ({ start, length }))
    .sort((a, b) => a.start - b.start);
  useCache.set(text, uses);
  if (useCache.size > CACHE_LIMIT) useCache.delete(useCache.keys().next().value);
  return uses;
}

module.exports = { parameterUses, lambdaParameters, bodyEnd };
