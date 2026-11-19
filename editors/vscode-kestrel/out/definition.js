'use strict';

// Go-to-definition for Kestrel. The extension does not run the compiler, so the
// target is found the same way hover and completion find a declaration: the
// syntactic model of the open file, plus the declarations its selective imports
// bring in. A local declaration answers with an offset in this file; an imported
// one answers with the file and offset recorded when the import was read.
//
// The cursor can land on a use (`State`, `state.at`, `Mode.Fast`, `this.field`,
// `DEAD_END`), on a declaration itself, or on a name inside an import. Names a
// later declaration introduces are not visible yet, so only bindings before the
// cursor count.

const kestrel = require('./kestrel');
const infer = require('./infer');
const parameters = require('./parameters');

// Kestrel identifiers follow Unicode XID plus underscore. The checker's table is
// the authority; this range is a close reader for navigation.
const IDENT_CONTINUE = /^[\p{ID_Continue}_]$/u;

function continuesIdentifier(character) {
  return Boolean(character) && IDENT_CONTINUE.test(character);
}

// The identifier the cursor points at. A click just past the last character
// still means that word, as it does in VS Code's own word navigation. A word
// inside a comment or string is not code and has no declaration.
function wordAt(text, offset) {
  const clean = kestrel.sanitize(text);
  let at = Math.max(0, Math.min(offset, text.length));
  if (!continuesIdentifier(text[at])) {
    if (at > 0 && continuesIdentifier(text[at - 1])) at -= 1;
    else return null;
  }
  if (clean[at] !== text[at]) return null;
  let start = at;
  while (start > 0 && continuesIdentifier(text[start - 1])) start -= 1;
  let end = at;
  while (end < text.length && continuesIdentifier(text[end])) end += 1;
  if (clean.slice(start, end) !== text.slice(start, end)) return null;
  return { start, end, word: text.slice(start, end) };
}

// A call may put generic arguments between the name and its parenthesis:
// `first[Int](xs)`. A field or case is never called.
function calledAfter(text, end) {
  let i = end;
  while (i < text.length && /[ \t]/.test(text[i])) i += 1;
  if (text[i] === '[') {
    let depth = 0;
    for (; i < text.length; i += 1) {
      if (text[i] === '[') depth += 1;
      else if (text[i] === ']') {
        depth -= 1;
        if (depth === 0) {
          i += 1;
          break;
        }
      }
    }
    while (i < text.length && /[ \t]/.test(text[i])) i += 1;
  }
  return text[i] === '(';
}

function closingBrace(clean, open) {
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

// The innermost `match` whose body contains the offset, with the subject text
// that names the enum. A case pattern such as `Link(x, rest)` then resolves
// through that enum even when several enums share a case name.
function enclosingMatch(clean, offset) {
  const pattern = /\bmatch\b/g;
  let match;
  let best = null;
  while ((match = pattern.exec(clean)) !== null) {
    if (match.index >= offset) break;
    let i = match.index + match[0].length;
    let round = 0;
    let square = 0;
    for (; i < clean.length; i += 1) {
      const current = clean[i];
      if (current === '(') round += 1;
      else if (current === ')') round -= 1;
      else if (current === '[') square += 1;
      else if (current === ']') square -= 1;
      else if (current === '{' && round === 0 && square === 0) break;
    }
    if (i >= clean.length) continue;
    const close = closingBrace(clean, i);
    if (close < 0 || offset <= i || offset >= close) continue;
    if (!best || i > best.brace) best = { start: match.index, brace: i, close };
  }
  return best;
}

// The source name of a declaration. A namespace import qualifies a type's name
// (`facts.State`) while its declaration keeps the final segment.
function declaredName(declaration) {
  const name = declaration.name || '';
  return name.includes('.') ? name.slice(name.lastIndexOf('.') + 1) : name;
}

// A declaration's original position. Imported declarations keep their current
// offsets blank so offset-mapping features skip them; `file` and `origin` hold
// the position go-to-definition needs. Members such as fields and cases carry
// the same pair.
function locationOf(declaration) {
  if (!declaration) return null;
  const length = declaredName(declaration).length;
  if (!length) return null;
  if (declaration.file && declaration.origin) {
    const offset = declaration.origin.nameIndex;
    return offset >= 0 ? { file: declaration.file, offset, length } : null;
  }
  if (Number.isInteger(declaration.nameIndex) && declaration.nameIndex >= 0) {
    return { file: '', offset: declaration.nameIndex, length };
  }
  return null;
}

function dedupe(locations) {
  const seen = new Set();
  const result = [];
  for (const location of locations) {
    if (!location) continue;
    const key = `${location.file || ''}:${location.offset}`;
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(location);
  }
  return result;
}

function locationsOf(declarations) {
  return dedupe((declarations || []).map(locationOf));
}

// A name the file declares itself wins over the same name from an import.
function localFirst(declarations) {
  const local = declarations.filter(declaration => !declaration.imported && !declaration.file);
  return local.length ? local : declarations;
}

// The members of a type along its inheritance chain, including trait defaults.
// A value receiver sees fields and instance methods; a type receiver sees enum
// cases, static fields and static methods. A call prefers methods, while a plain
// member access prefers fields.
function memberFromViews(model, type, word, options) {
  const call = Boolean(options.call);
  const typeName = Boolean(options.typeName);
  const onlyCases = Boolean(options.cases);
  for (const view of kestrel.typeViews(model, type)) {
    const declaration = view.declaration;
    const cases = (declaration.cases || []).filter(item => item.name === word);
    if (onlyCases) {
      if (cases.length) return locationsOf(cases);
      continue;
    }
    if (typeName && cases.length) return locationsOf(cases);
    const fields = (declaration.fields || []).filter(field =>
      field.name === word && typeName === Boolean(field.static));
    const methods = (model.functionsByOwner.get(declaration.name) || []).filter(fn =>
      fn.name === word && typeName === Boolean(fn.static));
    // Nearer views win: a class method hides the trait requirement it replaces,
    // and an inherited member is found only after the class has no answer.
    if (call) {
      if (methods.length) return locationsOf(methods);
      continue;
    }
    if (fields.length) return locationsOf(fields);
    if (methods.length) return locationsOf(methods);
  }
  return [];
}

// A name with several declarations (overloads, or one local and one imported)
// keeps every real target. The editor shows them as a peek list. `methodsOnly`
// keeps a receiver's member from matching a top-level function of the same name.
function bestLocations(model, word, options = {}) {
  const declarations = [];
  const type = model.typesByName.get(word);
  if (type) declarations.push(type);
  const constant = model.constantsByName.get(word);
  if (constant) declarations.push(constant);
  for (const theorem of model.theoremsByName.get(word) || []) declarations.push(theorem);
  for (const fn of model.functionsByName.get(word) || []) {
    if (options.methodsOnly ? fn.owner : (!fn.owner || options.includeMethods)) declarations.push(fn);
  }
  const locations = locationsOf(localFirst(declarations));
  if (locations.length) return locations;
  if (!type && !options.methodsOnly) {
    const constructors = model.constructorsByName.get(word) || [];
    return locationsOf(localFirst(constructors));
  }
  return [];
}

function uniqueEnumCase(model, word) {
  const matches = [];
  for (const declaration of model.classes) {
    if (declaration.kind !== 'enum') continue;
    for (const item of declaration.cases || []) {
      if (item.name === word) matches.push(item);
    }
  }
  const local = matches.filter(item => !item.file);
  const chosen = local.length ? local : matches;
  return chosen.length === 1 ? locationOf(chosen[0]) : null;
}

function enumCaseInMatch(text, clean, token, model) {
  const match = enclosingMatch(clean, token.start);
  if (!match) return [];
  const subject = text.slice(match.start + 'match'.length, match.brace).trim();
  if (!subject) return [];
  const type = infer.typeAt(text, match.start, model, subject);
  return type ? memberFromViews(model, type, token.word, { cases: true }) : [];
}

function ownerOf(model, position) {
  const owner = model.functions.find(fn => fn.start <= position && position <= fn.end);
  if (owner) return owner;
  return (model.theorems || []).find(theorem => theorem.start <= position && position <= theorem.end) || null;
}

// `for ((index, value) in pairs)` binds both names. The inference scanner reads
// simple loop heads; this reader covers the destructured form too.
function destructuredLoopBindings(clean) {
  const found = [];
  const pattern = /\bfor\s*\(\s*\(([^()]*)\)\s*in\b/g;
  let match;
  while ((match = pattern.exec(clean)) !== null) {
    const base = match.index + match[0].indexOf(match[1]);
    for (const name of match[1].matchAll(/[A-Za-z_][A-Za-z0-9_]*/g)) {
      if (name[0] === '_') continue;
      found.push({ name: name[0], nameStart: base + name.index, nameEnd: base + name.index + name[0].length });
    }
  }
  return found;
}

// The binding a use refers to: a function or theorem parameter, a `let`/`var`,
// a `for` head, or a lambda parameter. Only names before the cursor count, and
// a binding from another function is out of scope. Kestrel forbids shadowing
// inside a function, so the nearest earlier binding is the answer.
function nearestBinding(text, clean, token, model) {
  const word = token.word;
  const candidates = [];
  const add = (name, index) => {
    if (name === word && index >= 0 && index <= token.start) {
      candidates.push({ file: '', offset: index, length: word.length });
    }
  };
  const target = ownerOf(model, token.start);
  for (const fn of model.functions) {
    if (!(fn.start <= token.start && token.start <= fn.end)) continue;
    for (const parameter of fn.parameters || []) add(parameter.name, parameter.nameIndex);
  }
  for (const theorem of model.theorems || []) {
    if (!(theorem.start <= token.start && token.start <= theorem.end)) continue;
    for (const parameter of theorem.parameters || []) add(parameter.name, parameter.nameIndex);
  }
  for (const binding of [...infer.bindings(text), ...infer.loopBindings(text, clean), ...destructuredLoopBindings(clean)]) {
    const owner = ownerOf(model, binding.nameStart);
    if (owner && owner !== target) continue;
    if (!target && owner) continue;
    add(binding.name, binding.nameStart);
  }
  for (const lambda of parameters.lambdaParameters(clean)) {
    // A lambda parameter is visible only inside the lambda's body.
    if (lambda.from <= token.start && token.start <= lambda.to) {
      for (const head of lambda.heads) add(head.name, head.start);
    }
  }
  candidates.sort((left, right) => right.offset - left.offset);
  return candidates[0] || null;
}

function resolveMember(text, clean, token, receiver, model) {
  const word = token.word;
  const call = calledAfter(text, token.end);
  const enclosing = kestrel.enclosingClass(model, token.start);
  if (receiver === 'this' && enclosing) {
    return memberFromViews(model, enclosing.name, word, { call });
  }
  if (receiver === 'super' && enclosing) {
    const parent = /^[\w.]+/.exec(enclosing.parent || '')?.[0];
    return parent ? memberFromViews(model, parent, word, { call }) : [];
  }
  const known = infer.localTypes(text, token.start, model);
  // A namespace import names a type with its alias: `iface.Measure` and
  // `geometry.Point` are the qualified class keys the import model carries.
  if (!known.has(receiver)) {
    const qualified = model.typesByName.get(`${receiver}.${word}`);
    if (qualified) return locationsOf([qualified]);
  }
  // A bare type name is a namespace for static members and enum cases, not a value.
  if (!known.has(receiver) && model.typesByName.has(receiver)) {
    return memberFromViews(model, receiver, word, { call, typeName: true });
  }
  let type = known.get(receiver)?.type || '';
  if (!type) type = infer.typeAt(text, token.start, model, receiver, known);
  if (type) return memberFromViews(model, type, word, { call });
  // Without a type, a method name in the loaded project is still a possible
  // answer; the editor shows every match as a peek list.
  if (!model.typesByName.has(receiver)) return bestLocations(model, word, { methodsOnly: true });
  return [];
}

function resolveUnqualified(text, clean, token, model) {
  const binding = nearestBinding(text, clean, token, model);
  if (binding) return [binding];

  // A match pattern names an enum case through its subject's type.
  const matchCase = enumCaseInMatch(text, clean, token, model);
  if (matchCase.length) return matchCase;

  const declarations = bestLocations(model, token.word);
  if (declarations.length) return declarations;

  // A case written without its enum name is unambiguous when one enum declares it.
  const bareCase = uniqueEnumCase(model, token.word);
  if (bareCase) return [bareCase];

  // An unqualified member inside a class body still names that class's member.
  const enclosing = kestrel.enclosingClass(model, token.start);
  if (enclosing) {
    return memberFromViews(model, enclosing.name, token.word, { call: calledAfter(text, token.end) });
  }
  return [];
}

// Resolve the cursor to declarations in this file (`file` empty) or in an
// imported file (`file` set). A click on a comment, string, keyword or builtin
// returns no target.
function definitionAt(text, offset, model) {
  const token = wordAt(text, offset);
  if (!token) return [];
  const clean = kestrel.sanitize(text);
  const receiver = infer.memberReceiverExpression(text, token.start);
  if (receiver) return dedupe(resolveMember(text, clean, token, receiver, model));
  return dedupe(resolveUnqualified(text, clean, token, model));
}

module.exports = { definitionAt, wordAt, locationOf };
