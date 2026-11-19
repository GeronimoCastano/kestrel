'use strict';

const modules = require('./modules');

// A small, dependency-free reader for top-level Kestrel declarations. It is
// intentionally forgiving: the compiler remains the source of truth, while
// this module gives the editor enough information for completion and parameter
// hints.

const KEYWORDS = [
  'let', 'var', 'mut', 'move', 'function', 'class', 'trait', 'enum', 'alias',
  'const', 'new', 'public', 'private', 'package', 'static', 'final',
  'sealed', 'abstract', 'with', 'Effects', 'requires', 'ensures', 'has',
  'extends', 'import', 'as', 'depends', 'if', 'then', 'else', 'match', 'for',
  'in', 'while', 'break', 'continue', 'return', 'scope', 'spawn', 'using',
  'unsafe', 'theorem', 'proof', 'by', 'forall', 'exists', 'true', 'false',
  'this', 'super', 'extend'
];

const TYPES = [
  'Int', 'Nat', 'BigInt', 'double', 'Bool', 'String', 'Bytes', 'void', 'Error', 'Text',
  'Comparable', 'List', 'Map', 'Set', 'SortedSet', 'Array', 'Mutex', 'Atomic',
  'Channel', 'Arena', 'ArenaRef', 'Iterator', 'Iterable', 'Option',
  'PythonValue'
];

// One entry per tactic, shared by completion and hover. `signature` is the
// shape shown in the editor, while `insert` is the snippet completion places.
const TACTICS = [
  {
    name: 'intro',
    aliases: ['intros'],
    signature: 'intro h',
    insert: 'intro ${1:h}',
    summary: 'Introduce a forall binder, an implication premise, or a negation.',
    body: '`intro h` on `forall x : T, P` gives you a parameter `h : T` and the goal `P`. On `P -> Q` it gives a hypothesis `h : P` and the goal `Q`. On `not P` it gives `h : P` and the goal `false`. `intros` introduces as many binders as it can and names them from the statement.',
    example: 'intro n\nintro h'
  },
  {
    name: 'exact',
    aliases: [],
    signature: 'exact term',
    insert: 'exact ${1:h}',
    summary: 'Close the goal with a proof term.',
    body: '`exact term` checks that `term` proves the current goal. The term can be a hypothesis, a conjunction projection such as `h.left`, a theorem applied to arguments, or a constructor.',
    example: 'exact h'
  },
  {
    name: 'apply',
    aliases: [],
    signature: 'apply rule',
    insert: 'apply ${1:theorem}',
    summary: 'Apply a theorem or hypothesis to the goal.',
    body: '`apply rule` matches the conclusion of `rule` against the goal and leaves its premises as new goals. `rule` may be a hypothesis, a projection such as `h.left`, or a theorem. Pass theorem parameters in order when matching cannot infer them, as in `apply subsetRefl(A)`.',
    example: 'apply subsetRefl'
  },
  {
    name: 'have',
    aliases: [],
    signature: 'have h := proof',
    insert: 'have ${1:h} := ${2:proof}',
    summary: 'Name an intermediate fact, inferring its statement.',
    body: '`have h := proof` infers the statement from the proof term. Write `have h : P := proof` to state it explicitly, or `have h : P by { ... }` for a tactic proof.',
    example: 'have h4 := h1(h3)'
  },
  {
    name: 'cases',
    aliases: [],
    signature: 'cases h',
    insert: 'cases ${1:h}',
    summary: 'Split a value or hypothesis into cases.',
    body: '`cases h` on an `and` adds `<name>l` and `<name>r`, so `cases h` gives `hl : P` and `hr : Q`. On an `or` it leaves one goal per side, named `<name>l` and `<name>r`. On an `iff` it adds `<name>mp` and `<name>mpr`; on an `exists` it names the witness and adds `<name>w`. `cases n { ... }` splits a `Nat` or enum parameter and needs one branch per constructor.',
    example: 'cases n {\n    Zero => { rfl }\n    Succ(k) => { simp }\n}'
  },
  {
    name: 'induction',
    aliases: [],
    signature: 'induction n { ... }',
    insert: 'induction ${1:n} {\n    ${2:Zero} => { ${3:rfl} }\n    ${4:Succ(k) ih} => { ${5:simp [ih]} }\n}',
    summary: 'Prove by structural induction.',
    body: '`induction n { ... }` gives one branch per constructor of a `Nat` or enum. A recursive branch names one hypothesis per direct recursive field, in field order: `Succ(k) ih` or `Add(a, b) iha ihb`. A nonrecursive branch takes no hypothesis names. In a `by { ... }` script each branch body is a block such as `{ rfl }`; directly inside a theorem brace body each branch is a proof term such as `rfl`.',
    example: 'induction n {\n    Zero => { rfl }\n    Succ(k) ih => { simp [ih] }\n}'
  },
  {
    name: 'simp',
    aliases: [],
    signature: 'simp [rule]',
    insert: 'simp [${1:rule}]',
    summary: 'Rewrite with the simplifier and close simple goals.',
    body: '`simp` unfolds definitions and applies `@simp` lemmas. `simp [rule, ...]` adds checked equalities for this step. Put `←` before a rule, as in `simp [← h]`, to use it from right to left. It fails when the goal does not reduce to `true` or to a reflexive equality.',
    example: 'simp [ih]'
  },
  {
    name: 'rw',
    aliases: ['rewrite'],
    signature: 'rw [rule]',
    insert: 'rw [${1:rule}]',
    summary: 'Rewrite the goal with a cited equality.',
    body: '`rw [rule]` rewrites with a cited checked equality, plus the `@simp` lemmas, throughout the goal. Put `←` before a rule, as in `rw [← h]`, to use it from right to left. It leaves the rewritten goal and fails when nothing changes.',
    example: 'rw [addZero]'
  },
  {
    name: 'constructor',
    aliases: [],
    signature: 'constructor',
    insert: 'constructor',
    summary: 'Build a conjunction or `true`.',
    body: 'On `P and Q`, `constructor` leaves the two goals `P` and `Q`. On `true`, it closes the goal.',
    example: 'constructor'
  },
  {
    name: 'left',
    aliases: [],
    signature: 'left',
    insert: 'left',
    summary: 'Choose the left side of a disjunction.',
    body: 'On `P or Q`, `left` leaves the goal `P`.',
    example: 'left'
  },
  {
    name: 'right',
    aliases: [],
    signature: 'right',
    insert: 'right',
    summary: 'Choose the right side of a disjunction.',
    body: 'On `P or Q`, `right` leaves the goal `Q`.',
    example: 'right'
  },
  {
    name: 'exists',
    aliases: ['use'],
    signature: 'exists witness',
    insert: 'exists ${1:witness}',
    summary: 'Give a witness for an existential goal.',
    body: '`exists w` checks `w` against the quantified type, substitutes it into the body, and leaves the body as the goal. `use` is an alias.',
    example: 'exists 0'
  },
  {
    name: 'assumption',
    aliases: [],
    signature: 'assumption',
    insert: 'assumption',
    summary: 'Close the goal with a matching hypothesis.',
    body: 'Searches the context for a hypothesis whose proposition is the goal and closes it. Fails when none matches.',
    example: 'assumption'
  },
  {
    name: 'contradiction',
    aliases: [],
    signature: 'contradiction',
    insert: 'contradiction',
    summary: 'Close the goal from contradictory hypotheses.',
    body: 'Closes the goal when a hypothesis is `false`, or when two hypotheses give an equality between different constructor values.',
    example: 'contradiction'
  },
  {
    name: 'exfalso',
    aliases: [],
    signature: 'exfalso',
    insert: 'exfalso',
    summary: 'Replace the goal with `false`.',
    body: 'Changes any goal to `false`. Use it when the hypotheses are already inconsistent and the goal does not follow directly.',
    example: 'exfalso'
  },
  {
    name: 'rfl',
    aliases: [],
    signature: 'rfl',
    insert: 'rfl',
    summary: 'Prove an equality by computation.',
    body: 'Closes `a = b` when both sides reduce to the same term. When they do not, split the variable with `cases` or `induction` so the definitions can unfold.',
    example: 'rfl'
  },
  {
    name: 'symm',
    aliases: [],
    signature: 'symm',
    insert: 'symm',
    summary: 'Swap the sides of an equality goal.',
    body: 'Turns `a = b` into `b = a`.',
    example: 'symm'
  },
  {
    name: 'trans',
    aliases: [],
    signature: 'trans middle',
    insert: 'trans ${1:middle}',
    summary: 'Prove an equality through a middle term.',
    body: '`trans m` turns `a = b` into the two goals `a = m` and `m = b`.',
    example: 'trans 0'
  },
  {
    name: 'decide',
    aliases: [],
    signature: 'decide',
    insert: 'decide',
    summary: 'Evaluate a closed proposition.',
    body: '`decide` computes a proposition with no variables and closes it when the result is `true`. It fails on a false result or when variables remain.',
    example: 'decide'
  },
  {
    name: 'trivial',
    aliases: [],
    signature: 'trivial',
    insert: 'trivial',
    summary: 'Try the simple closers.',
    body: 'Closes `true`, reflexive equalities, simple conjunctions and disjunctions, and goals that match a hypothesis.',
    example: 'trivial'
  },
  {
    name: 'pushNeg',
    aliases: [],
    signature: 'pushNeg',
    insert: 'pushNeg',
    summary: 'Push negations inward.',
    body: '`pushNeg` rewrites the goal using negated connectives: `not (not P)` becomes `P`, `not (P and Q)` becomes `not P or not Q`, and `not (P or Q)` becomes `not P and not Q`. It also moves negations through `->`, `forall` and `exists`. It is classical, like `byContra`.',
    example: 'pushNeg'
  },
  {
    name: 'ext',
    aliases: [],
    signature: 'ext',
    insert: 'ext',
    summary: 'Prove a set equality by extensionality.',
    body: '`ext` replaces a goal `A = B` between sets with the two subset goals `A ⊆ B` and `B ⊆ A`.',
    example: 'ext'
  },
  {
    name: 'byContra',
    aliases: [],
    signature: 'byContra h',
    insert: 'byContra ${1:h}',
    summary: 'Prove a goal by contradiction.',
    body: '`byContra h` is the classical rule. To prove `P`, it adds `h : not P` to the context and changes the goal to `false`.',
    example: 'byContra h'
  },
  {
    name: 'smt',
    aliases: [],
    signature: 'smt',
    insert: 'smt',
    summary: 'Discharge an arithmetic goal with Z3.',
    body: 'Asks Z3 about integer arithmetic and independently checks supported linear certificates. Other successful solver results remain trusted and are listed by `kestrel check --trusted`. Nat underflow and division by zero must be ruled out. Install `z3` or set `KESTREL_Z3`.',
    example: 'smt'
  },
  {
    name: 'repeat',
    aliases: [],
    signature: 'repeat { ... }',
    insert: 'repeat {\n    ${1:tactic}\n}',
    summary: 'Run a tactic block until it stops changing the goal.',
    body: '`repeat { ... }` runs the block until the goals close or a round makes no change. It stops with an error after 64 rounds.',
    example: 'repeat {\n    simp\n}'
  },
  {
    name: 'try',
    aliases: [],
    signature: 'try { ... }',
    insert: 'try {\n    ${1:tactic}\n}',
    summary: 'Run a tactic block, but ignore failure.',
    body: '`try { ... }` runs the block once. If it fails, the goals return to their previous state and the proof continues.',
    example: 'try {\n    simp\n}'
  },
  {
    name: 'skip',
    aliases: [],
    signature: 'skip',
    insert: 'skip',
    summary: 'Do nothing.',
    body: 'Leaves the goal unchanged. Useful as a placeholder while sketching a proof.',
    example: 'skip'
  },
  {
    name: 'done',
    aliases: [],
    signature: 'done',
    insert: 'done',
    summary: 'Check that no goals remain.',
    body: 'Fails when any goal is still open. Put it at the end of a proof to make unfinished work explicit.',
    example: 'done'
  },
  {
    name: 'sorry',
    aliases: [],
    signature: 'sorry',
    insert: 'sorry',
    summary: 'Admit the goal while sketching.',
    body: 'Closes the current goal and emits `W_PROOF_SORRY`. The theorem is admitted, not trusted, and cannot satisfy a function requirement.',
    example: 'sorry'
  }
];

const TACTIC_INDEX = new Map();
for (const tactic of TACTICS) {
  TACTIC_INDEX.set(tactic.name, tactic);
  for (const alias of tactic.aliases) TACTIC_INDEX.set(alias, tactic);
}

function tactic(name) {
  return TACTIC_INDEX.get(name);
}

// Proof-term introduction rules. They build a proof when the goal has the
// matching shape; unlike tactics they are arguments to exact, have or a
// theorem body.
const CONSTRUCTORS = [
  {
    name: 'And',
    parameters: ['p', 'q'],
    signature: 'And(p, q)',
    insert: 'And(${1:p}, ${2:q})',
    summary: 'Build a proof of P and Q.',
    body: '`And(p, q)` proves `P and Q` from a proof of each side. It is a proof term, so write `exact And(p, q)` or `have h := And(p, q)`.'
  },
  {
    name: 'OrLeft',
    parameters: ['p'],
    signature: 'OrLeft(p)',
    insert: 'OrLeft(${1:p})',
    summary: 'Prove P or Q from the left side.',
    body: '`OrLeft(p)` proves `P or Q` from a proof of `P`. It is a proof term, so write `exact OrLeft(p)`. `Left(p)` is the same rule.'
  },
  {
    name: 'OrRight',
    parameters: ['q'],
    signature: 'OrRight(q)',
    insert: 'OrRight(${1:q})',
    summary: 'Prove P or Q from the right side.',
    body: '`OrRight(q)` proves `P or Q` from a proof of `Q`. It is a proof term, so write `exact OrRight(q)`. `Right(q)` is the same rule.'
  },
  {
    name: 'Iff',
    parameters: ['f', 'g'],
    signature: 'Iff(f, g)',
    insert: 'Iff(${1:f}, ${2:g})',
    summary: 'Build an iff from both directions.',
    body: '`Iff(f, g)` proves `P ↔ Q` when `f` proves `P -> Q` and `g` proves `Q -> P`. It is a proof term, so write `exact Iff(f, g)`.'
  },
  {
    name: 'Exists',
    parameters: ['w', 'p'],
    signature: 'Exists(w, p)',
    insert: 'Exists(${1:w}, ${2:p})',
    summary: 'Build an existential from a witness and a proof.',
    body: '`Exists(w, p)` proves `exists x, P` with witness `w` and a proof `p`. It is a proof term, so write `exact Exists(w, p)`. `exists(w, p)` is the same rule.'
  },
  {
    name: 'True',
    parameters: [],
    signature: 'True',
    insert: 'True',
    summary: 'Prove true.',
    body: '`True` proves the proposition `true`. It is a proof term, so write `exact True` or `have h := True`.'
  }
];

const CONSTRUCTOR_INDEX = new Map(CONSTRUCTORS.map(constructor => [constructor.name, constructor]));

function proofConstructor(name) {
  return CONSTRUCTOR_INDEX.get(name);
}

// The editor asks for the same text from several providers in a row. A tiny
// cache turns the second and later questions into map lookups. Strings compare
// by value, and V8 caches a string's hash, so a large buffer is still cheap.
const CACHE_LIMIT = 6;
const cleanCache = new Map();
const modelCache = new Map();

function cached(cache, key) {
  const value = cache.get(key);
  if (value === undefined) return undefined;
  // Refresh the entry so the open file stays in the small cache.
  cache.delete(key);
  cache.set(key, value);
  return value;
}

function remember(cache, key, value) {
  cache.set(key, value);
  if (cache.size > CACHE_LIMIT) cache.delete(cache.keys().next().value);
}

function sanitize(text) {
  const hit = cached(cleanCache, text);
  if (hit !== undefined) return hit;
  const characters = text.split('');
  let i = 0;
  while (i < text.length) {
    if (text.startsWith('//', i)) {
      while (i < text.length && text[i] !== '\n') {
        characters[i] = ' ';
        i += 1;
      }
      continue;
    }
    if (text[i] === '"') {
      characters[i] = ' ';
      i += 1;
      while (i < text.length) {
        const current = text[i];
        characters[i] = ' ';
        if (current === '\\') {
          i += 1;
          if (i < text.length) {
            characters[i] = ' ';
            i += 1;
          }
          continue;
        }
        i += 1;
        if (current === '"') break;
      }
      continue;
    }
    i += 1;
  }
  const clean = characters.join('');
  remember(cleanCache, text, clean);
  return clean;
}

function wordBefore(clean, index) {
  let end = index - 1;
  while (end >= 0 && /\s/.test(clean[end])) end -= 1;
  let start = end;
  while (start >= 0 && /[A-Za-z0-9_]/.test(clean[start])) start -= 1;
  return clean.slice(start + 1, end + 1);
}

// True when the cursor sits in a proof script: a `by { ... }` block or the
// brace body of a `theorem` / `example`. Plain function bodies do not count, so
// a name such as `exists` in a statement keeps its ordinary meaning.
function proofScriptAt(text, offset) {
  const clean = sanitize(text);
  const stack = [];
  let header = false;
  let i = 0;
  const limit = Math.min(offset, clean.length);
  while (i < limit) {
    const current = clean[i];
    if (current === '{') {
      stack.push(wordBefore(clean, i) === 'by' || header);
      header = false;
      i += 1;
      continue;
    }
    if (current === '}') {
      stack.pop();
      i += 1;
      continue;
    }
    if (/[A-Za-z_]/.test(current)) {
      let end = i + 1;
      while (end < limit && /[A-Za-z0-9_]/.test(clean[end])) end += 1;
      const word = clean.slice(i, end);
      if ((word === 'theorem' || word === 'example') && stack.length === 0) header = true;
      i = end;
      continue;
    }
    i += 1;
  }
  return stack.some(Boolean);
}

// A tactic begins a line, or follows `{`, `;` or a branch arrow on the same
// line. This keeps a tactic name used as an argument from replacing the
// argument's own hover.
function tacticPosition(text, offset) {
  let i = offset - 1;
  while (i >= 0 && (text[i] === ' ' || text[i] === '\t' || text[i] === '\r')) i -= 1;
  if (i < 0) return true;
  if (text[i] === '\n') return true;
  if (text[i] === '{' || text[i] === ';') return true;
  return text[i] === '>' && text[i - 1] === '=';
}

function matching(clean, open, opening, closing) {
  let depth = 0;
  for (let i = open; i < clean.length; i += 1) {
    if (clean[i] === opening) depth += 1;
    else if (clean[i] === closing) {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  return -1;
}

function lastIdentifier(text) {
  const found = /([A-Za-z_][A-Za-z0-9_]*)\s*$/.exec(text);
  return found ? found[1] : '';
}

function findTopLevel(text, wanted) {
  let round = 0;
  let square = 0;
  let curly = 0;
  let i = 0;
  while (i < text.length) {
    const current = text[i];
    if (current === '"') {
      i += 1;
      while (i < text.length) {
        if (text[i] === '\\') {
          i += 2;
          continue;
        }
        if (text[i] === '"') {
          i += 1;
          break;
        }
        i += 1;
      }
      continue;
    }
    if (current === '(') round += 1;
    else if (current === ')') round -= 1;
    else if (current === '[') square += 1;
    else if (current === ']') square -= 1;
    else if (current === '{') curly += 1;
    else if (current === '}') curly -= 1;
    else if (current === wanted && round === 0 && square === 0 && curly === 0) return i;
    i += 1;
  }
  return -1;
}

// Top-level comma-separated parts with the offset each one starts at, so a
// parameter list can map a name back to the file. The reader that only needs
// the text drops the offsets.
function splitTopLevelRanges(text) {
  const parts = [];
  let start = 0;
  let round = 0;
  let square = 0;
  let curly = 0;
  let i = 0;
  while (i < text.length) {
    const current = text[i];
    if (current === '"') {
      i += 1;
      while (i < text.length) {
        if (text[i] === '\\') {
          i += 2;
          continue;
        }
        if (text[i] === '"') {
          i += 1;
          break;
        }
        i += 1;
      }
      continue;
    }
    if (current === '(') round += 1;
    else if (current === ')') round -= 1;
    else if (current === '[') square += 1;
    else if (current === ']') square -= 1;
    else if (current === '{') curly += 1;
    else if (current === '}') curly -= 1;
    else if (current === ',' && round === 0 && square === 0 && curly === 0) {
      parts.push({ text: text.slice(start, i), start });
      start = i + 1;
    }
    i += 1;
  }
  if (text.slice(start).trim() !== '') parts.push({ text: text.slice(start), start });
  return parts;
}

function splitTopLevel(text) {
  return splitTopLevelRanges(text).map(part => part.text);
}

// `base` is the offset of `raw` in the file, so a parameter knows where its
// name is declared. A direct call without a file uses offsets from the string.
function parseParameter(raw, base = 0) {
  const trimmed = raw.trim();
  const trimmedStart = base + raw.indexOf(trimmed);
  let declaration = trimmed;
  let defaultValue = '';
  const proof = /^proof\b/.test(trimmed);
  const equals = proof ? -1 : findTopLevel(trimmed, '=');
  if (equals >= 0) {
    declaration = trimmed.slice(0, equals).trim();
    defaultValue = trimmed.slice(equals + 1).trim();
  }
  const colon = findTopLevel(declaration, ':');
  let name = '';
  let nameIndex = -1;
  let type = '';
  if (colon >= 0) {
    const head = declaration.slice(0, colon);
    name = lastIdentifier(head);
    nameIndex = name ? trimmedStart + head.lastIndexOf(name) : -1;
    type = declaration.slice(colon + 1).trim();
  } else {
    name = lastIdentifier(declaration);
    nameIndex = name ? trimmedStart + declaration.lastIndexOf(name) : -1;
  }
  return { name, nameIndex, type: proof ? 'Proof' : type, proof, proposition: proof ? type : '', default: defaultValue, label: trimmed };
}

// The type parameters in `class Node[T : Comparable, U]`, read after the class name.
function typeParameters(clean, from) {
  let i = from;
  while (i < clean.length && /\s/.test(clean[i])) i += 1;
  if (clean[i] !== '[') return [];
  const close = clean.indexOf(']', i);
  if (close < 0) return [];
  return clean
    .slice(i + 1, close)
    .split(',')
    .map(part => part.split(':')[0].trim())
    .filter(name => /^[A-Z][A-Za-z0-9_]*$/.test(name));
}

// The case names of an enum, read at the top level of its body. A case's
// parameter list may hold nested brackets; only the case name itself is wanted.
function enumCases(clean, declaration) {
  const cases = [];
  let i = declaration.bodyStart + 1;
  const limit = Math.max(i, declaration.end - 1);
  while (i < limit) {
    while (i < limit && /[\s,;]/.test(clean[i])) i += 1;
    if (i >= limit) break;
    const name = /^[A-Z][A-Za-z0-9_]*/.exec(clean.slice(i, limit));
    if (!name) {
      i += 1;
      continue;
    }
    const after = i + name[0].length;
    const open = clean.slice(after).search(/\S/);
    const payload = open >= 0 && clean[after + open] === '(' ? after + open : -1;
    const payloadEnd = payload >= 0 ? matching(clean, payload, '(', ')') : -1;
    cases.push({ name: name[0], nameIndex: i, start: i,
      fields: payloadEnd >= 0 ? splitTopLevel(clean.slice(payload + 1, payloadEnd)).map(field => field.trim()) : [] });
    i += name[0].length;
    // Skip an optional payload, so a type name inside its brackets is not a case.
    while (i < limit) {
      while (i < limit && /\s/.test(clean[i])) i += 1;
      if (clean[i] !== '(' && clean[i] !== '[') break;
      const close = matching(clean, i, clean[i], clean[i] === '(' ? ')' : ']');
      if (close < 0) {
        i = limit;
        break;
      }
      i = close + 1;
    }
  }
  return cases;
}

// Private fields declared in a constructor: `final this.radius : double := r`.
// They join the class's fields so `this.` and member inference can see them.
function constructorFields(clean, declaration) {
  const found = [];
  const body = clean.slice(declaration.bodyStart, declaration.end);
  const pattern = /(?:(public|private|package)\s+)?(?:(static)\s+)?(final|var)\s+this\s*\.\s*([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([^:=\n]+?)\s*:=/g;
  let match;
  while ((match = pattern.exec(body)) !== null) {
    const name = match[4];
    if (found.some(field => field.name === name)) continue;
    const dot = match[0].indexOf('.');
    const nameIndex = declaration.bodyStart + match.index + match[0].indexOf(name, dot);
    found.push({
      name,
      type: match[5].trim(),
      visibility: 'private',
      static: Boolean(match[2]),
      constructor: true,
      nameIndex,
      start: declaration.bodyStart + match.index
    });
  }
  return found;
}

function parseParameters(text, base = 0) {
  const parameters = [];
  for (const part of splitTopLevelRanges(text)) {
    const raw = part.text;
    const start = base + part.start;
    const declaration = raw.trim();
    const declarationStart = start + raw.indexOf(declaration);
    const colon = findTopLevel(declaration, ':');
    // `proof h : P` is one parameter even though its head has two words; the
    // proof marker is a keyword, not a name.
    const proof = /^proof\b/.test(declaration);
    // One annotation may cover several names: `A B : Set[U]` is two
    // parameters. A default applies to one name, so grouped names with `=`
    // fall back to the single-parameter reader.
    if (!proof && colon >= 0 && findTopLevel(declaration, '=') < 0) {
      const modifier = /^(mut|move)\s+/.exec(declaration);
      const head = declaration.slice(0, colon).trim();
      const namesStart = declarationStart + declaration.indexOf(head);
      const names = head.replace(/^(?:mut|move)\s+/, '').split(/\s+/).filter(Boolean);
      if (names.length > 1 && names.every(name => /^[A-Za-z_][A-Za-z0-9_]*$/.test(name))) {
        const type = declaration.slice(colon + 1).trim();
        // Walk the written head so `mut a b : Int` gives both names their own
        // offset. The modifier is a keyword, not a parameter.
        let cursor = namesStart;
        for (const name of names) {
          const found = head.indexOf(name, cursor - namesStart);
          const nameIndex = namesStart + found;
          cursor = nameIndex + name.length;
          parameters.push({
            name,
            nameIndex,
            type,
            default: '',
            label: `${modifier ? `${modifier[1]} ` : ''}${name} : ${type}`
          });
        }
        continue;
      }
    }
    parameters.push(parseParameter(raw, start));
  }
  return parameters.filter(parameter => parameter.name !== '');
}

function parseReturnType(text, clean, from) {
  let i = from;
  while (i < clean.length && /\s/.test(clean[i])) i += 1;
  if (clean[i] !== ':') return '';
  let depth = 0;
  let j = i + 1;
  while (j < clean.length) {
    const current = clean[j];
    if (depth === 0 && (current === '=' || current === '{' || current === '\n')) break;
    if (depth === 0 && /\b(requires|ensures)\b/.test(clean.slice(j, j + 10))) break;
    if (current === '(' || current === '[' || current === '{') depth += 1;
    else if (current === ')' || current === ']' || current === '}') {
      if (depth === 0) break;
      depth -= 1;
    }
    j += 1;
  }
  return text.slice(i + 1, j).trim();
}

// The statement after a theorem's colon, up to its proof or its first
// `requires`. An axiom has no proof, so its statement stops at the line end.
function theoremStatement(text, clean, from, multiline) {
  let i = from;
  while (i < clean.length && /\s/.test(clean[i])) i += 1;
  if (clean[i] !== ':') return '';
  let depth = 0;
  let j = i + 1;
  while (j < clean.length) {
    const current = clean[j];
    if (depth === 0) {
      if (current === ';' || current === '{') break;
      if (current === '\n' && !multiline) break;
      if (/^(?:by|requires)\b/.test(clean.slice(j, j + 9))) break;
    }
    if (current === '(' || current === '[') depth += 1;
    else if (current === ')' || current === ']') depth -= 1;
    j += 1;
  }
  return text.slice(i + 1, j).trim();
}

function bodyRange(clean, from) {
  let open = -1;
  for (let i = from; i < clean.length; i += 1) {
    if (clean[i] === '{') {
      open = i;
      break;
    }
    if (clean[i] === '=') {
      let depth = 0;
      let j = i + 1;
      while (j < clean.length) {
        const current = clean[j];
        if (current === '(' || current === '[' || current === '{') depth += 1;
        else if (current === ')' || current === ']' || current === '}') {
          if (depth === 0) break;
          depth -= 1;
        }
        if (depth === 0 && current === '\n') break;
        j += 1;
      }
      return { start: i + 1, end: j };
    }
  }
  if (open < 0) return { start: from, end: clean.length };
  const close = matching(clean, open, '{', '}');
  return { start: open, end: close < 0 ? clean.length : close + 1 };
}

// Lookup tables make completion and hover independent of file size: a member
// question becomes a map read instead of a scan over every declaration.
function addIndex(map, key, value) {
  const found = map.get(key);
  if (found) found.push(value);
  else map.set(key, [value]);
}

function indexModel(model) {
  const functionsByName = new Map();
  const topFunctionsByName = new Map();
  const functionsByOwner = new Map();
  const functionsByOwnerName = new Map();
  for (const fn of model.functions) {
    addIndex(functionsByName, fn.name, fn);
    if (!fn.owner) addIndex(topFunctionsByName, fn.name, fn);
    addIndex(functionsByOwner, fn.owner || '', fn);
    addIndex(functionsByOwnerName, `${fn.owner || ''}\u0000${fn.name}`, fn);
  }
  const theoremsByName = new Map();
  for (const theorem of model.theorems) {
    if (theorem.name) addIndex(theoremsByName, theorem.name, theorem);
  }
  const constructorsByName = new Map();
  const constructorsByOwner = new Map();
  for (const constructor of model.constructors) {
    addIndex(constructorsByName, constructor.name, constructor);
    addIndex(constructorsByOwner, constructor.owner || '', constructor);
  }
  const classesByName = new Map(model.classes.map(declaration => [declaration.name, declaration]));
  const typesByName = new Map(model.types.map(declaration => [declaration.name, declaration]));
  const constantsByName = new Map(model.constants.map(constant => [constant.name, constant]));
  // Non-enumerable: these are an implementation detail, not part of the model.
  Object.defineProperties(model, {
    functionsByName: { value: functionsByName },
    topFunctionsByName: { value: topFunctionsByName },
    functionsByOwner: { value: functionsByOwner },
    functionsByOwnerName: { value: functionsByOwnerName },
    constructorsByName: { value: constructorsByName },
    constructorsByOwner: { value: constructorsByOwner },
    theoremsByName: { value: theoremsByName },
    classesByName: { value: classesByName },
    typesByName: { value: typesByName },
    constantsByName: { value: constantsByName }
  });
  return model;
}

// The visibility written directly before a declaration on its line. A modifier
// further left belongs to an earlier declaration.
function visibilityOf(clean, index) {
  const lineStart = clean.lastIndexOf('\n', index) + 1;
  const modifiers = /(?:\b(?:public|private|package)\s+)*$/.exec(clean.slice(lineStart, index))[0];
  return (modifiers.match(/\b(public|private|package)\b/g) || []).pop() || 'private';
}

// `///` lines directly above a declaration are its documentation. One optional
// space after the marker is dropped and the rest stays Markdown; `//` and
// `////` stay ordinary comments.
function documentationIndex(text) {
  const lines = text.split('\n');
  const starts = [];
  const docs = new Map();
  let offset = 0;
  for (let index = 0; index < lines.length; index += 1) {
    starts.push(offset);
    const match = /^\s*\/\/\/(?!\/) ?(.*)$/.exec(lines[index].replace(/\r$/, ''));
    if (match) docs.set(index, match[1].replace(/\r$/, ''));
    offset += lines[index].length + 1;
  }
  return { lines, starts, docs };
}

function lineAt(starts, offset) {
  let low = 0;
  let high = starts.length - 1;
  while (low < high) {
    const middle = (low + high + 1) >> 1;
    if (starts[middle] <= offset) low = middle;
    else high = middle - 1;
  }
  return low;
}

// The contiguous `///` block above the declaration at `offset`, skipping any
// attribute lines between the block and the declaration. A blank line or an
// ordinary comment ends the block, so only a block that touches the declaration
// counts as its documentation.
function documentationAt(index, offset) {
  let line = lineAt(index.starts, offset);
  while (line > 0 && /^\s*@/.test(index.lines[line - 1])) line -= 1;
  if (line === 0 || !index.docs.has(line - 1)) return '';
  let first = line - 1;
  while (first > 0 && index.docs.has(first - 1)) first -= 1;
  const block = [];
  for (let current = first; current < line; current += 1) block.push(index.docs.get(current));
  return block.join('\n');
}

function parseDocument(text) {
  const hit = cached(modelCache, text);
  if (hit !== undefined) return hit;
  const clean = sanitize(text);
  const docs = documentationIndex(text);
  const docAt = offset => documentationAt(docs, offset);
  const classes = [];
  const classPattern = /\b(class|trait|enum|record)\s+([A-Z][A-Za-z0-9_]*)(?:\s*\[[^\]]*\])?(?:\s+extends\s+([A-Z][A-Za-z0-9_]*))?/g;
  let match;
  while ((match = classPattern.exec(clean)) !== null) {
    const kind = match[1];
    const nameIndex = clean.indexOf(match[2], match.index);
    const visibility = visibilityOf(clean, match.index);
    if (kind === 'record') {
      // A record holds its fields in the parameter list, and its body is optional.
      let open = nameIndex + match[2].length;
      while (open < clean.length && /\s/.test(clean[open])) open += 1;
      if (clean[open] === '[') {
        const parameterClose = matching(clean, open, '[', ']');
        open = parameterClose < 0 ? clean.length : parameterClose + 1;
        while (open < clean.length && /\s/.test(clean[open])) open += 1;
      }
      if (clean[open] !== '(') continue;
      const close = matching(clean, open, '(', ')');
      if (close < 0) continue;
      let bodyStart = close + 1;
      let end = close + 1;
      let next = close + 1;
      while (next < clean.length && /\s/.test(clean[next])) next += 1;
      if (clean[next] === '{') {
        bodyStart = next;
        const bodyClose = matching(clean, next, '{', '}');
        end = bodyClose < 0 ? clean.length : bodyClose + 1;
      }
      classes.push({
        kind,
        name: match[2],
        visibility,
        documentation: docAt(match.index),
        nameIndex,
        start: match.index,
        parent: '',
        bodyStart,
        typeParameters: typeParameters(clean, nameIndex + match[2].length),
        parameters: parseParameters(text.slice(open + 1, close), open + 1),
        end
      });
      continue;
    }
    let open = clean.indexOf('{', nameIndex + match[2].length);
    if (open < 0) open = nameIndex + match[2].length;
    const close = open < clean.length && clean[open] === '{'
      ? matching(clean, open, '{', '}')
      : open;
    classes.push({
      kind,
      name: match[2],
      visibility,
      documentation: docAt(match.index),
      nameIndex,
      start: match.index,
      parent: match[3] || '',
      bodyStart: open,
      typeParameters: typeParameters(clean, nameIndex + match[2].length),
      end: close < 0 ? clean.length : close + 1
    });
  }

  for (const declaration of classes) {
    declaration.fields = [];
    declaration.traits = [];
    let nested = 0;
    for (let at = declaration.bodyStart + 1; at < declaration.end - 1; at += 1) {
      if (clean[at] === '{') nested += 1;
      if (clean[at] === '}') nested -= 1;
      if (nested !== 0 || !/^has\b/.test(clean.slice(at, at + 4))) continue;
      const tail = clean.slice(at + 3, declaration.end).split(/[;\n}]/)[0];
      declaration.traits.push(...splitTopLevel(tail).map(value => value.trim()).filter(Boolean));
      at += 3 + tail.length;
    }
    const header = clean.slice(declaration.start, declaration.bodyStart);
    const parentType = /\bextends\s+([A-Z]\w*(?:\[[^\n]*\])?)/.exec(header);
    if (parentType) declaration.parent = parentType[1];
    declaration.cases = declaration.kind === 'enum' ? enumCases(clean, declaration) : [];
    if (declaration.kind === 'record') {
      // `record Point(x : Int)` reads its fields from the parameter list. They are
      // public, and the record name builds the value, so no `new` constructor is
      // declared separately.
      for (const parameter of declaration.parameters) {
        if (!parameter.name) continue;
        declaration.fields.push({
          name: parameter.name,
          type: parameter.type,
          visibility: 'public',
          static: false,
          nameIndex: Number.isInteger(parameter.nameIndex) ? parameter.nameIndex : -1,
          start: Number.isInteger(parameter.nameIndex) ? parameter.nameIndex : -1
        });
      }
      continue;
    }
    let depth = 0;
    for (let i = declaration.bodyStart + 1; i < declaration.end - 1; i += 1) {
      const current = clean[i];
      if (current === '{') {
        depth += 1;
        continue;
      }
      if (current === '}') {
        depth -= 1;
        continue;
      }
      if (depth !== 0) continue;
      const rest = clean.slice(i);
      const field = /^(?:(public|private|package)\s+)?(?:(static)\s+)?(final|var)\s+([a-z_][A-Za-z0-9_]*)\s*:\s*([^=\n}]+)/.exec(rest);
      if (!field) continue;
      const name = field[4];
      const nameOffset = i + field[0].indexOf(name);
      declaration.fields.push({
        name,
        type: field[5].trim(),
        visibility: field[1] || 'private',
        static: Boolean(field[2]),
        nameIndex: nameOffset,
        start: i
      });
      i += field[0].length - 1;
    }
    for (const field of constructorFields(clean, declaration)) {
      if (!declaration.fields.some(existing => existing.name === field.name)) {
        declaration.fields.push(field);
      }
    }
    declaration.fields.sort((left, right) => left.start - right.start);
  }

  const ownerOf = index => {
    let best = null;
    for (const declaration of classes) {
      if (declaration.bodyStart <= index && index < declaration.end) {
        if (!best || declaration.bodyStart > best.bodyStart) best = declaration;
      }
    }
    return best ? best.name : '';
  };

  const functions = [];
  const constructors = [];
  const functionPattern = /\bfunction\s+([a-z_][A-Za-z0-9_]*)\s*(?:\[[^\]]*\])?\s*\(/g;
  while ((match = functionPattern.exec(clean)) !== null) {
    const name = match[1];
    const nameIndex = clean.indexOf(name, match.index);
    const open = clean.indexOf('(', match.index);
    const close = matching(clean, open, '(', ')');
    if (open < 0 || close < 0) continue;
    const rawParameters = text.slice(open + 1, close);
    const parameters = parseParameters(rawParameters, open + 1);
    const result = parseReturnType(text, clean, close + 1);
    const body = bodyRange(clean, close + 1);
    const lineStart = clean.lastIndexOf('\n', match.index) + 1;
    // Only the modifiers directly before `function`, so an earlier declaration
    // that happens to share the line does not lend it `static`.
    const modifiers = /(?:\b(?:public|private|package|static|override|sealed)\s+)*$/
      .exec(clean.slice(lineStart, match.index))[0];
    const visibility = (modifiers.match(/\b(public|private|package)\b/g) || []).pop() || 'private';
    functions.push({
      kind: 'function',
      name,
      visibility,
      documentation: docAt(match.index),
      static: /\bstatic\b/.test(modifiers),
      owner: ownerOf(match.index),
      hasBody: (clean[body.start] === '{' || clean[body.start - 1] === '=') && !/\b(?:function|class|trait|record|enum|new|theorem|alias)\b/.test(clean.slice(close + 1, body.start)),
      nameIndex,
      start: match.index,
      paramsOpen: open,
      paramsClose: close,
      bodyStart: body.start,
      end: body.end,
      result,
      parameters
    });
  }

  const constructorPattern = /\bnew\s*\(/g;
  while ((match = constructorPattern.exec(clean)) !== null) {
    const owner = ownerOf(match.index);
    if (!owner) continue;
    const open = clean.indexOf('(', match.index);
    const close = matching(clean, open, '(', ')');
    if (close < 0) continue;
    const parameters = parseParameters(text.slice(open + 1, close), open + 1);
    const body = bodyRange(clean, close + 1);
    constructors.push({
      kind: 'constructor',
      name: owner,
      owner,
      documentation: docAt(match.index),
      nameIndex: match.index,
      start: match.index,
      paramsOpen: open,
      paramsClose: close,
      bodyStart: body.start,
      end: body.end,
      result: owner,
      parameters
    });
  }

  // `Point(1, 2)` builds a record, so its name is a constructor the same way a
  // class name is. The fields are the parameters, in order.
  for (const declaration of classes) {
    if (declaration.kind !== 'record') continue;
    constructors.push({
      kind: 'constructor',
      record: true,
      name: declaration.name,
      owner: '',
      documentation: declaration.documentation,
      visibility: declaration.visibility,
      nameIndex: declaration.nameIndex,
      start: declaration.start,
      paramsOpen: -1,
      paramsClose: -1,
      bodyStart: declaration.start,
      end: declaration.start,
      result: declaration.name,
      parameters: declaration.parameters
    });
  }

  const theorems = [];
  const theoremPattern = /\b(theorem|axiom|example)\b\s*(?:([A-Za-z_][A-Za-z0-9_]*)\s*)?(?:\[[^\]]*\])?\s*\(/g;
  while ((match = theoremPattern.exec(clean)) !== null) {
    const kind = match[1];
    const name = match[2] || '';
    const nameIndex = name ? clean.indexOf(name, match.index) : match.index + kind.length;
    const open = clean.indexOf('(', match.index);
    const close = matching(clean, open, '(', ')');
    if (close < 0) continue;
    const result = theoremStatement(text, clean, close + 1, kind === 'theorem');
    let bodyStart;
    let end;
    if (kind === 'axiom') {
      const lineEnd = clean.indexOf('\n', close + 1);
      bodyStart = lineEnd < 0 ? clean.length : lineEnd;
      end = bodyStart;
    } else {
      const proof = clean.indexOf('{', close + 1);
      if (proof < 0) {
        bodyStart = clean.length;
        end = clean.length;
      } else {
        const proofEnd = matching(clean, proof, '{', '}');
        bodyStart = proof;
        end = proofEnd < 0 ? clean.length : proofEnd + 1;
      }
    }
    theorems.push({
      kind,
      name,
      visibility: visibilityOf(clean, match.index),
      documentation: docAt(match.index),
      nameIndex,
      start: match.index,
      paramsOpen: open,
      paramsClose: close,
      typeParameters: typeParameters(clean, nameIndex + name.length),
      parameters: parseParameters(text.slice(open + 1, close), open + 1),
      result,
      bodyStart,
      end
    });
  }

  const constants = [];
  const constantPattern = /\bconst\s+([A-Z][A-Z0-9_]*)\s*(?::\s*([^=\n]+))?/g;
  while ((match = constantPattern.exec(clean)) !== null) {
    const name = match[1];
    constants.push({
      kind: 'constant',
      name,
      visibility: visibilityOf(clean, match.index),
      documentation: docAt(match.index),
      type: match[2] ? match[2].trim() : '',
      nameIndex: clean.indexOf(name, match.index),
      start: match.index
    });
  }

  const aliases = [];
  const aliasPattern = /\balias\s+([A-Z][A-Za-z0-9_]*)/g;
  while ((match = aliasPattern.exec(clean)) !== null) {
    const name = match[1];
    aliases.push({
      kind: 'alias',
      name,
      visibility: visibilityOf(clean, match.index),
      documentation: docAt(match.index),
      nameIndex: clean.indexOf(name, match.index),
      start: match.index
    });
  }

  const types = [
    ...classes,
    ...aliases
  ];

  const imports = [...clean.matchAll(/\bimport\s+((?!c\b|python\b)(?:std\.)?[a-z0-9]+(?:\.[a-z0-9]+)*)(?:\.\s*\{([^}]*)\})?(?:\s+as\s+([A-Za-z_]\w*))?/g)].map(match => {
    const written = match[1];
    const canonical = written.startsWith('std.')
      ? written
      : (modules.modules.some(module => module.path === `std.${written}`) ? `std.${written}` : written);
    return {
      module: canonical,
      written,
      names: match[2] ? match[2].split(',').map(name => name.trim()).filter(Boolean) : [],
      alias: match[2] ? '' : match[3] || written.split('.').at(-1)
    };
  });
  const model = {
    imports,
    functions,
    constructors,
    callables: [...functions, ...constructors],
    classes,
    types,
    constants,
    aliases,
    theorems
  };
  indexModel(model);
  remember(modelCache, text, model);
  return model;
}

// A model with declarations from the files the open document imports. The
// imported entries keep no offsets in this file, so code that maps an offset
// skips them; `parseDocument` alone remains the model for document-local work.
function withDeclarations(model, additions) {
  const functions = [...model.functions, ...(additions.functions || [])];
  const constructors = [...model.constructors, ...(additions.constructors || [])];
  const classes = [...model.classes, ...(additions.classes || [])];
  const aliases = [...model.aliases, ...(additions.aliases || [])];
  return indexModel({
    imports: model.imports,
    functions,
    constructors,
    callables: [...functions, ...constructors],
    classes,
    types: [...classes, ...aliases],
    constants: [...model.constants, ...(additions.constants || [])],
    aliases,
    theorems: [...model.theorems, ...(additions.theorems || [])]
  });
}

function signature(callable) {
  const parameters = callable.parameters.map(parameter => parameter.label).join(', ');
  const prefix = callable.kind === 'constructor' ? callable.name : callable.name;
  return `${prefix}(${parameters})${callable.result ? ` : ${callable.result}` : ''}`;
}

function topLevelCommas(text) {
  let round = 0;
  let square = 0;
  let curly = 0;
  let count = 0;
  let i = 0;
  while (i < text.length) {
    const current = text[i];
    if (current === '"') {
      i += 1;
      while (i < text.length) {
        if (text[i] === '\\') {
          i += 2;
          continue;
        }
        if (text[i] === '"') {
          i += 1;
          break;
        }
        i += 1;
      }
      continue;
    }
    if (current === '(') round += 1;
    else if (current === ')') round -= 1;
    else if (current === '[') square += 1;
    else if (current === ']') square -= 1;
    else if (current === '{') curly += 1;
    else if (current === '}') curly -= 1;
    else if (current === ',' && round === 0 && square === 0 && curly === 0) count += 1;
    i += 1;
  }
  return count;
}

// A class and its superclasses, nearest first. Inherited fields and methods are
// offered on a subclass value, as the compiler resolves them.
function typeViews(model, written) {
  const pending = [written], result = [], seen = new Set();
  while (pending.length && result.length < 96) {
    const type = pending.shift().replace(/^(?:&\s*)?(?:dyn\s+)?/, '');
    const name = /^[\w.]+/.exec(type)?.[0];
    if (!name || seen.has(name)) continue;
    seen.add(name);
    const declaration = model.classesByName.get(name);
    if (!declaration) continue;
    const args = type.includes('[') ? splitTopLevel(type.slice(type.indexOf('[') + 1, type.lastIndexOf(']'))) : [];
    const bindings = Object.fromEntries((declaration.typeParameters || []).map((parameter, i) => [parameter, args[i] || parameter]));
    const associated = /\((.*)\)$/.exec(type)?.[1];
    if (associated) for (const item of splitTopLevel(associated)) {
      const match = /^\s*(\w+)\s*=\s*(.+)$/.exec(item);
      if (match) bindings[match[1]] = match[2];
    }
    result.push({ name, type, declaration, bindings });
    const replace = text => text.replace(/\b[A-Za-z_]\w*\b/g, word => bindings[word] || word);
    if (declaration.parent) pending.unshift(replace(declaration.parent));
    pending.push(...(declaration.traits || []).map(replace));
  }
  return result;
}

function memberFunctions(model, type) {
  const result = [], seen = new Set();
  const views = typeViews(model, type);
  const traitReceiver = views[0]?.declaration.kind === 'trait';
  for (const view of views) for (const fn of model.functionsByOwner.get(view.name) || []) {
    if (view.declaration.kind === 'trait' && !traitReceiver && !fn.hasBody) continue;
    const replace = value => (value || '').replace(/\b[A-Za-z_]\w*\b/g, word => view.bindings[word] || word);
    const parameters = fn.parameters.map(parameter => ({ ...parameter, type: replace(parameter.type), label: replace(parameter.label) }));
    const key = `${fn.static}:${fn.name}:${parameters.map(parameter => parameter.type).join(',')}`;
    if (seen.has(key)) continue;
    seen.add(key);
    result.push({ ...fn, parameters, result: replace(fn.result) });
  }
  return result;
}

function proofBindings(text, offset, model) {
  const theorem = model.theorems.find(item => item.start <= offset && offset <= item.end);
  if (!theorem) return [];
  const result = theorem.parameters.filter(parameter => parameter.name).map(parameter => ({
    name: parameter.name,
    nameIndex: parameter.nameIndex,
    type: parameter.type,
    kind: 'parameter'
  }));
  const clean = sanitize(text);
  for (const requirement of clean.slice(theorem.paramsClose, theorem.bodyStart).matchAll(/\brequires\s+(\w+)\s*:/g)) {
    result.push({ name: requirement[1], type: 'Proof', kind: 'proof' });
  }
  const branches = /\b([A-Z]\w*)\s*\(([^()]*)\)\s*((?:[a-z_]\w*\s*)*)=>/g;
  branches.lastIndex = theorem.bodyStart;
  let branch;
  while ((branch = branches.exec(clean)) && branch.index < offset) {
    const from = branches.lastIndex;
    let start = from;
    while (/\s/.test(clean[start] || '') && start < clean.length) start += 1;
    const end = clean[start] === '{' ? matching(clean, start, '{', '}') : (() => { const next = clean.slice(start).search(/[,\n}]/); return next < 0 ? theorem.end : start + next; })();
    if (offset < from || end >= 0 && offset > end) continue;
    const prior = clean.slice(theorem.bodyStart, branch.index);
    const inductions = [...prior.matchAll(/\binduction\s+(\w+)\s*\{/g)];
    const target = result.find(binding => binding.name === inductions.at(-1)?.[1]);
    const view = typeViews(model, target?.type || '').find(item => item.declaration.kind === 'enum');
    const variant = view?.declaration.cases.find(item => item.name === branch[1]);
    splitTopLevel(branch[2]).forEach((name, index) => {
      if (/^[a-z_]\w*$/.test(name.trim())) result.push({ name: name.trim(), type: (variant?.fields?.[index] || '').replace(/\b\w+\b/g, word => view?.bindings[word] || word), kind: 'parameter' });
    });
    for (const name of branch[3].match(/[a-z_]\w*/g) || []) result.push({ name, type: 'Proof', kind: 'proof' });
  }
  return result;
}

function classChain(model, name) {
  const chain = [];
  let current = name;
  while (current && !chain.includes(current)) {
    chain.push(current);
    current = /^[\w.]+/.exec(model.classesByName.get(current)?.parent || '')?.[0] || '';
  }
  return chain;
}

function enclosingClass(model, offset) {
  let best = null;
  for (const declaration of model.classes) {
    if (declaration.bodyStart <= offset && offset < declaration.end) {
      if (!best || declaration.bodyStart > best.bodyStart) best = declaration;
    }
  }
  return best;
}

function callContext(text, offset) {
  const clean = sanitize(text).slice(0, offset);
  const stack = [];
  const pairs = { ')': '(', ']': '[', '}': '{' };
  for (let i = 0; i < clean.length; i += 1) {
    const current = clean[i];
    if (current === '(' || current === '[' || current === '{') stack.push({ char: current, index: i });
    else if (pairs[current]) {
      for (let j = stack.length - 1; j >= 0; j -= 1) {
        if (stack[j].char === pairs[current]) {
          stack.length = j;
          break;
        }
      }
    }
  }
  let openEntry = null;
  for (let i = stack.length - 1; i >= 0; i -= 1) {
    if (stack[i].char === '(') {
      openEntry = stack[i];
      break;
    }
    if (stack[i].char === '{') return null;
  }
  if (!openEntry) return null;
  const open = openEntry.index;
  const before = clean.slice(0, open);
  const declaration = /\bfunction\s+[a-z_][A-Za-z0-9_]*\s*(?:\[[^\]]*\])?\s*$/.test(before);
  let calleeText = before.replace(/\s+$/, '');
  if (calleeText.endsWith(']')) {
    let depth = 0;
    for (let i = calleeText.length - 1; i >= 0; i -= 1) {
      if (calleeText[i] === ']') depth += 1;
      else if (calleeText[i] === '[') {
        depth -= 1;
        if (depth === 0) {
          calleeText = calleeText.slice(0, i);
          break;
        }
      }
    }
  }
  const method = /\.\s*[A-Za-z_][A-Za-z0-9_]*\s*$/.test(calleeText);
  const receiverMatch = /([A-Za-z_][A-Za-z0-9_]*)\s*\.\s*[A-Za-z_][A-Za-z0-9_]*\s*$/.exec(calleeText);
  const callee = lastIdentifier(calleeText);
  if (!callee) return null;
  const argumentsText = text.slice(open + 1, offset);
  return {
    declarationParams: declaration,
    method,
    receiver: receiverMatch ? receiverMatch[1] : '',
    callee,
    open,
    argumentIndex: topLevelCommas(argumentsText),
    argumentsText
  };
}

function parametersInUse(argumentsText) {
  const used = new Set();
  let i = 0;
  while (i < argumentsText.length) {
    const current = argumentsText[i];
    if (current === '"') {
      i += 1;
      while (i < argumentsText.length) {
        if (argumentsText[i] === '\\') {
          i += 2;
          continue;
        }
        if (argumentsText[i] === '"') {
          i += 1;
          break;
        }
        i += 1;
      }
      continue;
    }
    if (/[A-Za-z_]/.test(current)) {
      let j = i + 1;
      while (j < argumentsText.length && /[A-Za-z0-9_]/.test(argumentsText[j])) j += 1;
      const name = argumentsText.slice(i, j);
      let k = j;
      while (k < argumentsText.length && /\s/.test(argumentsText[k])) k += 1;
      if (argumentsText[k] === '=' && argumentsText[k + 1] !== '=' || name === 'axis' && argumentsText[k] === ':') used.add(name);
      i = j;
      continue;
    }
    i += 1;
  }
  return used;
}

function collectLocals(text, offset, model) {
  const names = new Set();
  let enclosing = null;
  for (const fn of model.functions) {
    if (fn.start <= offset && offset <= fn.end) {
      if (!enclosing || fn.start > enclosing.start) enclosing = fn;
    }
  }
  if (enclosing) {
    for (const parameter of enclosing.parameters) names.add(parameter.name);
  }
  let theorem = null;
  for (const declaration of model.theorems || []) {
    if (declaration.start <= offset && offset <= declaration.end) {
      if (!theorem || declaration.start > theorem.start) theorem = declaration;
    }
  }
  if (theorem) {
    for (const parameter of proofBindings(text, offset, model)) names.add(parameter.name);
  }
  const from = enclosing ? enclosing.start : theorem ? theorem.bodyStart : 0;
  // Sanitizing the whole file once keeps every offset, and the cache makes the
  // many calls in one completion request nearly free.
  const before = sanitize(text).slice(from, offset);
  const pattern = /\b(?:let|var)\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)/g;
  let match;
  while ((match = pattern.exec(before)) !== null) names.add(match[1]);
  const tuplePattern = /\b(?:let|var)\s+(?:mut\s+)?\(([^()]*)\)/g;
  while ((match = tuplePattern.exec(before)) !== null) {
    for (const name of match[1].match(/[A-Za-z_][A-Za-z0-9_]*/g) || []) {
      if (name !== '_') names.add(name);
    }
  }
  if (theorem) {
    // Proof names a tactic introduces by hand: `intro x h`, `have h : P`,
    // `byContra hx`. Generated case names come from the compiler, not the text.
    for (const found of before.matchAll(/^[ \t]*intro[ \t]+([^\n;{}]+)/gm)) {
      for (const name of found[1].match(/[A-Za-z_][A-Za-z0-9_]*/g) || []) {
        if (name !== '_') names.add(name);
      }
    }
    for (const found of before.matchAll(/^[ \t]*(?:have|byContra)[ \t]+([A-Za-z_][A-Za-z0-9_]*)/gm)) {
      names.add(found[1]);
    }
  }
  return [...names];
}

// The part of an import statement being typed, if any. Comments and strings are
// blanked first, so `import` in a comment does not open a module list.
function importContext(text, offset) {
  const canonical = name => name.startsWith('std.')
    ? name
    : (modules.modules.some(module => module.path === `std.${name}`) ? `std.${name}` : name);
  const before = sanitize(text).slice(0, offset);
  let start = -1;
  const pattern = /\bimport\b/g;
  let match;
  while ((match = pattern.exec(before)) !== null) start = match.index;
  if (start < 0) return null;
  const segment = before.slice(start + 6);
  let depth = 0;
  let open = -1;
  for (let i = 0; i < segment.length; i += 1) {
    const character = segment[i];
    if (character === '{') {
      depth += 1;
      open = i;
    } else if (character === '}') {
      depth = Math.max(0, depth - 1);
      if (depth === 0) open = -1;
    } else if ((character === '\n' || character === ';') && depth === 0) {
      return null;
    }
  }
  if (depth > 0) {
    const module = /^\s*((?:std\.)?[a-z0-9]+(?:\.[a-z0-9]+)*)\s*\.\s*$/.exec(segment.slice(0, open));
    if (!module) return null;
    const inner = segment.slice(open + 1);
    const parts = inner.split(',');
    const partial = (parts.pop() || '').trim();
    if (/\s/.test(partial)) return null;
    const selected = new Set(parts.map(part => part.trim()).filter(Boolean));
    return {
      kind: 'names',
      module: canonical(module[1]),
      written: module[1],
      prefix: partial,
      selected,
      start: start + 6 + open + 1 + inner.length - partial.length,
      end: offset
    };
  }
  if (segment.includes('}')) return null;
  const trimmed = segment.trim();
  if (/\bas(?:\s+[A-Za-z0-9_]*)?$/.test(trimmed)) return { kind: 'none' };
  if (trimmed === '') {
    return {
      kind: 'root',
      prefix: '',
      start: start + 6 + segment.length,
      end: offset,
      leadingSpace: segment.length === 0
    };
  }
  if (/^[A-Za-z0-9_]*$/.test(trimmed)) {
    const prefixStart = start + 6 + segment.indexOf(trimmed);
    return { kind: 'module', prefix: trimmed, start: prefixStart, end: offset };
  }
  // The standard modules are leaves today, but a dotted path may name a module
  // or a member of one. `import std.linalg.` and `import std.linalg.in` offer
  // the module's exported names, inserted inside `{...}`.
  const modulePaths = modules.modules.map(module => module.path);
  const trailingDot = trimmed.endsWith('.');
  const pathText = trailingDot ? trimmed.slice(0, -1) : trimmed;
  const canonicalPath = canonical(pathText);
  const dot = pathText.lastIndexOf('.');
  const parent = dot > 0 ? pathText.slice(0, dot) : '';
  const canonicalParent = canonical(parent);
  const leaf = dot > 0 ? pathText.slice(dot + 1) : '';
  const trimmedStart = start + 6 + segment.indexOf(trimmed);
  if (trailingDot && pathText !== 'std') {
    const children = modulePaths
      .filter(path => path.startsWith(`${canonicalPath}.`))
      .map(path => path.slice(canonicalPath.length + 1).split('.')[0]);
    if (children.length > 0) {
      return { kind: 'nested', base: pathText, canonicalBase: canonicalPath, prefix: '', start: offset, end: offset };
    }
  }
  if (trailingDot && modulePaths.includes(canonicalPath)) {
    return { kind: 'members', module: canonicalPath, written: pathText, prefix: '', start: offset, end: offset };
  }
  if (!trailingDot && modulePaths.includes(canonicalParent) && /^[A-Za-z0-9_]*$/.test(leaf)) {
    return {
      kind: 'members',
      module: canonicalParent,
      written: parent,
      prefix: leaf,
      start: trimmedStart + trimmed.length - leaf.length,
      end: offset
    };
  }
  if (trimmed.startsWith('std.')) {
    const rest = trimmed.slice(4);
    if (rest.includes('.')) return null;
    if (!/^[a-z0-9_]*$/.test(rest)) return null;
    const prefixStart = start + 6 + segment.indexOf(trimmed) + 4;
    return { kind: 'module', prefix: rest, start: prefixStart, end: offset };
  }
  return null;
}

module.exports = {
  KEYWORDS,
  TYPES,
  TACTICS,
  tactic,
  CONSTRUCTORS,
  proofConstructor,
  proofScriptAt,
  tacticPosition,
  sanitize,
  importContext,
  parseDocument,
  withDeclarations,
  parseParameters,
  signature,
  callContext,
  classChain,
  typeViews,
  memberFunctions,
  proofBindings,
  enclosingClass,
  parametersInUse,
  collectLocals,
  lastIdentifier
};
