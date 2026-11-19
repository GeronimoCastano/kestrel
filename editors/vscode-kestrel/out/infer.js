'use strict';

// A small, conservative type reader for editor hints. It recognises the
// expressions that have one obvious Kestrel type and stays silent otherwise;
// the compiler remains the authority.

const kestrel = require('./kestrel');
const builtins = require('./builtins');
const numerical = require('./numerical');
const distributions = require('./distributions');
const kesdas = require('./kesdas');
const ml = require('./ml');
const kesnet = require('./kesnet');
const kesnetOptim = require('./kesnet_optim');
const kesnetData = require('./kesnet_data');

// Parsing and inference are pure functions of the text, and one editor request
// asks for them several times. Keep the recent answers; the open file is always
// the most recently used entry.
const CACHE_LIMIT = 4;
const bindingCache = new Map();
const loopBindingCache = new Map();
const inferredCache = new Map();
const commentCache = new Map();
const lambdaCache = new Map();

function cached(cache, key) {
  const value = cache.get(key);
  if (value === undefined) return undefined;
  cache.delete(key);
  cache.set(key, value);
  return value;
}

function remember(cache, key, value) {
  cache.set(key, value);
  if (cache.size > CACHE_LIMIT) cache.delete(cache.keys().next().value);
}

const NUMBER = /^[+-]?(?:0[xX][0-9a-fA-F_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d[\d_]*)?)$/;
const IDENTIFIER = /^[A-Za-z_][A-Za-z0-9_]*$/;

function splitTopLevel(text) {
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
      parts.push(text.slice(start, i));
      start = i + 1;
    }
    i += 1;
  }
  if (text.slice(start).trim() !== '') parts.push(text.slice(start));
  return parts;
}

function topLevel(text, wanted) {
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

function balanced(text) {
  let depth = 0;
  let i = 0;
  while (i < text.length) {
    const current = text[i];
    if (current === '"') {
      i = skipString(text, i);
      continue;
    }
    if ('([{'.includes(current)) depth += 1;
    else if (')]}'.includes(current)) {
      depth -= 1;
      if (depth < 0) return false;
    }
    i += 1;
  }
  return depth === 0;
}

function skipString(text, start) {
  let i = start + 1;
  while (i < text.length) {
    if (text[i] === '\\') {
      i += 2;
      continue;
    }
    if (text[i] === '"') return i + 1;
    i += 1;
  }
  return text.length;
}

// The index of the bracket that opens the one closing at `closeIndex`, or -1.
function openerOf(text, closeIndex) {
  const stack = [];
  let i = 0;
  while (i < text.length) {
    const current = text[i];
    if (current === '"') {
      i = skipString(text, i);
      continue;
    }
    if ('([{'.includes(current)) stack.push(i);
    else if (')]}'.includes(current)) {
      const open = stack.pop();
      if (i === closeIndex) return open === undefined ? -1 : open;
    }
    i += 1;
  }
  return -1;
}

function expressionEnd(clean, start) {
  let round = 0;
  let square = 0;
  let curly = 0;
  let i = start;
  while (i < clean.length) {
    const current = clean[i];
    if (current === '(') round += 1;
    else if (current === ')') round -= 1;
    else if (current === '[') square += 1;
    else if (current === ']') square -= 1;
    else if (current === '{') curly += 1;
    else if (current === '}') curly -= 1;
    else if (round === 0 && square === 0 && curly === 0 && (current === '\n' || current === ';')) break;
    i += 1;
  }
  return i;
}

function bindings(text) {
  const hit = cached(bindingCache, text);
  if (hit !== undefined) return hit;
  const clean = kestrel.sanitize(text);
  const pattern = /\b(let|var)\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*([^=;\n]+?))?\s*(:=|=)/g;
  const found = [];
  let match;
  while ((match = pattern.exec(clean)) !== null) {
    const name = match[2];
    // The name follows the keyword and any `mut`. Searching the whole match would find a
    // letter inside `let` first, which moved the type hint into the keyword.
    const head = /^\b(let|var)\s+(?:mut\s+)?/.exec(match[0])[0];
    const nameStart = match.index + head.length;
    const nameEnd = nameStart + name.length;
    const rhsStart = match.index + match[0].length;
    const rhsEnd = expressionEnd(clean, rhsStart);
    found.push({
      name,
      nameStart,
      nameEnd,
      rhsStart,
      rhsEnd,
      annotation: match[3] ? match[3].trim() : '',
      kind: match[1]
    });
  }
  const tuplePattern = /\b(let|var)\s+(?:mut\s+)?\(([^()]*)\)\s*(?::\s*([^=;\n]+?))?\s*(:=|=)/g;
  while ((match = tuplePattern.exec(clean)) !== null) {
    const names = splitTopLevel(match[2]).map(part => part.trim()).filter(Boolean);
    if (names.length < 2 || names.some(name => !/^[A-Za-z_][A-Za-z0-9_]*$/.test(name))) continue;
    const rhsStart = match.index + match[0].length;
    const rhsEnd = expressionEnd(clean, rhsStart);
    const namesStart = match.index + match[0].indexOf(match[2]);
    const namePattern = /[A-Za-z_][A-Za-z0-9_]*/g;
    let index = 0;
    let item;
    while ((item = namePattern.exec(match[2])) !== null) {
      const component = index++;
      if (item[0] === '_') continue;
      found.push({
        name: item[0],
        nameStart: namesStart + item.index,
        nameEnd: namesStart + item.index + item[0].length,
        rhsStart,
        rhsEnd,
        annotation: '',
        kind: match[1],
        component
      });
    }
  }
  remember(bindingCache, text, found);
  return found;
}

function enclosingFunction(model, offset) {
  let best = null;
  for (const fn of model.functions) {
    if (fn.start <= offset && offset <= fn.end) {
      if (!best || fn.start > best.start) best = fn;
    }
  }
  return best;
}

function functionResult(fn) {
  if (!fn.result) return '';
  return fn.result.replace(/\s+with\s+Effects\b.*$/, '').trim();
}

function signatureResult(entry) {
  const colon = entry.signature.lastIndexOf(':');
  if (colon < 0) return '';
  return entry.signature.slice(colon + 1).replace(/^[\s]+/, '').trim();
}

function substitute(type, bindings) {
  let result = type;
  for (const [name, value] of Object.entries(bindings)) {
    result = result.replace(new RegExp(`\\b${name}\\b`, 'g'), value);
  }
  return result;
}

const MAX_DEPTH = 24;
const NUMERIC = ['Int', 'double', 'BigInt'];
const ITERABLE_TYPES = ['List', 'Set', 'SortedSet', 'MutSet', 'Array', 'Lazy'];
const NAMED_ARGUMENT = /^[A-Za-z_][A-Za-z0-9_]*\s*=(?![=>])\s*/;
const LAMBDA = /^\s*\(?\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*([^=()]+?))?\s*\)?\s*=>\s*([\s\S]+)$/;
const OPERATOR_TOKENS = ['|>', '||', '&&', '==', '!=', '<=', '>=', '**', '=>', '->', '<', '>', '+', '-', '*', '/', '%', '@'];
// Lowest precedence first. `pick` chooses the split point: left-associative operators split at the last one.
const BINARY_LEVELS = [
  { ops: ['|>'], pick: 'last' },
  { ops: ['||'], pick: 'last' },
  { ops: ['&&'], pick: 'last' },
  { ops: ['==', '!=', '<=', '>=', '<', '>'], pick: 'first' },
  { ops: ['+', '-'], pick: 'last' },
  { ops: ['*', '/', '%', '@'], pick: 'last' },
  { ops: ['**'], pick: 'first' }
];

// Type text such as `Map[String, List[Int]]` split into its top-level arguments.
function typeArguments(type) {
  const open = type.indexOf('[');
  if (open < 0 || !type.trim().endsWith(']')) return [];
  return splitTopLevel(type.slice(open + 1, type.lastIndexOf(']')));
}

function baseName(type) {
  const match = /^(?:[A-Za-z_][A-Za-z0-9_]*\.)*([A-Za-z_][A-Za-z0-9_]*)/.exec((type || '').trim().replace(/^(?:&\s*)?(?:dyn\s+)?/, ''));
  return match ? match[1] : '';
}

// `Series or Error` is not a Series: without `?` the success branch has no methods.
function errorUnion(type) {
  return /\bor\s+Error\b/.test(type || '');
}

// A standalone capital letter is a generic variable that no argument has fixed yet.
function unresolved(type) {
  return /(^|[^A-Za-z0-9_])[A-Z](?![A-Za-z0-9_])/.test(type);
}

function resultOf(entry) {
  return signatureResult(entry).replace(/\s+with\s+Effects\b.*$/, '').trim();
}

function receiverBindings(actual, formal) {
  if (baseName(actual) !== baseName(formal)) return null;
  const actualArgs = typeArguments(actual);
  const formalArgs = typeArguments(formal);
  if (actualArgs.length !== formalArgs.length) return null;
  const values = {};
  formalArgs.forEach((formal, index) => {
    // `shape` is the tensor's dimension variable, so `Tensor[double, shape]` binds it as well.
    const name = formal.trim();
    if (/^[A-Z]$/.test(name) || name === 'shape') values[name] = actualArgs[index].trim();
  });
  return values;
}

// Metadata-declared parents, such as Linear extends Module, join with the
// current document's local class chain.
function metadataParent(name) {
  for (const module of [kesnet, kesnetOptim, kesnetData]) {
    if (module.parents && module.parents[name]) return module.parents[name];
  }
  return '';
}

function typeChain(type, model) {
  const chain = [];
  let current = baseName(type);
  while (current && !chain.includes(current)) {
    chain.push(current);
    current = (model && model.classes.find(item => item.name === current)?.parent) || metadataParent(current) || '';
  }
  return chain;
}

// `Iterable[T]` methods apply to every collection and lazy sequence, with its element bound to T.
function matchReceiver(actual, formal) {
  if (baseName(formal) === 'Iterable') {
    const args = typeArguments(actual);
    return ITERABLE_TYPES.includes(baseName(actual)) && args.length === 1 ? { T: args[0] } : null;
  }
  return receiverBindings(actual, formal);
}

function listType(items, model, scope, depth) {
  if (items.length === 0) return '';
  const types = items.map(item => infer(item, model, scope, depth + 1)).filter(Boolean);
  if (types.length !== items.length) return '';
  const unique = [...new Set(types)];
  if (unique.length === 1) return `List[${unique[0]}]`;
  if (unique.every(type => type === 'Int' || type === 'double')) return 'List[double]';
  return '';
}

function parseLambda(text) {
  const match = LAMBDA.exec(text.trim());
  if (!match) return null;
  return { name: match[1], annotation: match[2] ? match[2].trim() : '', body: match[3] };
}

// Argument text with any `name =` prefix removed, so named calls infer like positional ones.
function argumentsOf(text) {
  return splitTopLevel(text).map(part => part.trim().replace(NAMED_ARGUMENT, '').replace(/^axis\s*:\s*/, '').trim()).filter(Boolean);
}

// The arguments of `Name(...)` when the call's own parenthesis closes the value, or null.
function constructorArgs(value, name) {
  const match = new RegExp(`^${name}\\s*\\(`).exec(value);
  if (!match || !value.endsWith(')')) return null;
  const open = match[0].length - 1;
  if (openerOf(value, value.length - 1) !== open) return null;
  return value.slice(open + 1, -1);
}

// The last `.` outside brackets and strings that is not a decimal point, or -1.
function lastMemberDot(text) {
  let depth = 0;
  let found = -1;
  let i = 0;
  while (i < text.length) {
    const current = text[i];
    if (current === '"') {
      i = skipString(text, i);
      continue;
    }
    if ('([{'.includes(current)) depth += 1;
    else if (')]}'.includes(current)) depth -= 1;
    else if (current === '.' && depth === 0 && !(/[0-9]/.test(text[i - 1] || '') && /[0-9]/.test(text[i + 1] || ''))) found = i;
    i += 1;
  }
  return found;
}

// Operators at bracket depth zero, as { op, index }. Arrows and strings are skipped.
function topLevelOperators(value) {
  const found = [];
  let depth = 0;
  let i = 0;
  while (i < value.length) {
    const current = value[i];
    if (current === '"') {
      i = skipString(value, i);
      continue;
    }
    if ('([{'.includes(current)) depth += 1;
    else if (')]}'.includes(current)) depth -= 1;
    else if (depth === 0) {
      const token = OPERATOR_TOKENS.find(item => value.startsWith(item, i));
      if (token === '=>' || token === '->') {
        i += 2;
        continue;
      }
      if (token) {
        found.push({ op: token, index: i });
        i += token.length;
        continue;
      }
    }
    i += 1;
  }
  return found;
}

// An operator is binary only after an operand. `-x` and `2 * -x` keep the sign as a prefix.
function isBinaryAt(value, index) {
  return /[A-Za-z0-9_)\]"]$/.test(value.slice(0, index).trimEnd());
}

function splitBinary(value) {
  const operators = topLevelOperators(value).filter(entry => isBinaryAt(value, entry.index));
  for (const level of BINARY_LEVELS) {
    const candidates = operators.filter(entry => level.ops.includes(entry.op));
    if (candidates.length === 0) continue;
    const chosen = level.pick === 'last' ? candidates[candidates.length - 1] : candidates[0];
    return {
      op: chosen.op,
      left: value.slice(0, chosen.index).trim(),
      right: value.slice(chosen.index + chosen.op.length).trim()
    };
  }
  return null;
}

// Only number literals give a static shape here. A name may be a run-time value, which the checker
// types as DynTensor, so names are left unknown rather than guessed.
const DIMENSION = /^\d[\d_]*$/;

function shapeFrom(parts) {
  const dimensions = parts.map(part => part.trim());
  if (!dimensions.every(part => DIMENSION.test(part))) return '';
  return dimensions.length === 1 ? `(${dimensions[0]},)` : `(${dimensions.join(', ')})`;
}

// The shape written as a tuple argument: `(5, 6)` gives `(5, 6)`, and `(3,)` gives `(3,)`.
function shapeText(argument) {
  const value = argument.trim();
  if (!value.startsWith('(') || !value.endsWith(')') || !balanced(value.slice(1, -1))) return '';
  return shapeFrom(splitTopLevel(value.slice(1, -1)).map(part => part.trim()).filter(Boolean));
}

function setShape(bindings, shape) {
  if (shape) bindings.shape = shape;
  else delete bindings.shape;
}

const TENSOR = /^Tensor\[double, \((.*)\)\]$/;

function isTensorType(type) {
  return TENSOR.test(type || '') || type === 'DynTensor[double]';
}

function tensorDimensions(type) {
  const match = TENSOR.exec(type || '');
  return match ? match[1].split(',').map(part => part.trim()).filter(Boolean) : null;
}

// `a * b` keeps the shape of equal tensors, and `2.0 * a` keeps the tensor's shape. `a @ b` is 2-D matrix
// multiplication: (m, k) @ (k, n) gives (m, n). Any other combination has no type here.
function tensorOperator(op, left, right) {
  if (op === '**') return isTensorType(left) && ['Int', 'double'].includes(right) ? left : '';
  if (op === '/') return isTensorType(left) && right === 'double' ? left : '';
  if (['*', '+', '-'].includes(op)) {
    if (isTensorType(left) && right === 'double') return left;
    if (left === 'double' && isTensorType(right)) return right;
    if (!isTensorType(left) || !isTensorType(right)) return '';
    if (left === right) return left;
    if (left === 'DynTensor[double]') return right;
    if (right === 'DynTensor[double]') return left;
    return '';
  }
  if (op === '@' && isTensorType(left) && isTensorType(right)) {
    const a = tensorDimensions(left);
    const b = tensorDimensions(right);
    if (a && ![1, 2].includes(a.length) || b && ![1, 2].includes(b.length) || a?.length === 1 && b?.length === 1) return '';
    if (!a || !b) return 'DynTensor[double]';
    if (a.at(-1) !== b[0]) return '';
    return tensorType([...(a.length === 2 ? [a[0]] : []), ...(b.length === 2 ? [b[1]] : [])]);
  }
  return '';
}

function integerLiteralText(value) {
  return /^[+-]?(?:0[xX][0-9a-fA-F]+|[0-9][0-9_]*)$/.test(value.trim());
}

// An integer literal expression whose every leaf converts exactly to double, as
// the checker accepts in a double context: `1`, `-1`, `1 + 1`, `0x10 * 2`.
function exactDoubleLiteralExpression(value) {
  let text = value.trim();
  while (text.startsWith('(') && text.endsWith(')') && balanced(text.slice(1, -1))) {
    text = text.slice(1, -1).trim();
  }
  if (integerLiteralText(text)) return exactDoubleInteger(text);
  const split = splitBinary(text);
  return Boolean(split)
    && ['+', '-', '*', '/', '%', '**'].includes(split.op)
    && exactDoubleLiteralExpression(split.left)
    && exactDoubleLiteralExpression(split.right);
}

// An integer literal the checker accepts where a double is expected: the value must
// convert exactly, as `check_exact_double_literal` requires. Hex and underscores are
// allowed, and 2^128 is the compiler's upper bound.
function exactDoubleInteger(value) {
  const text = value.trim();
  if (!/^[+-]?(?:0[xX][0-9a-fA-F_]+|[0-9][0-9_]*)$/.test(text)) return false;
  try {
    const negative = text.startsWith('-');
    const digits = (negative ? text.slice(1) : text.replace(/^\+/, '')).replace(/_/g, '');
    let magnitude = BigInt(digits);
    if (magnitude >= (1n << 128n)) return false;
    if (negative) magnitude = -magnitude;
    return BigInt(Number(magnitude)) === magnitude;
  } catch (_) {
    return false;
  }
}

// An arithmetic expression built only from integer literals and the numeric operators,
// such as `1 + 2`. Each literal must still convert exactly; a name or call makes it false.
function integerLiteralExpression(value) {
  const literals = value.match(/0[xX][0-9a-fA-F_]+|\d[\d_]*/g) || [];
  if (literals.length === 0 || !literals.every(exactDoubleInteger)) return false;
  return value.replace(/0[xX][0-9a-fA-F_]+|\d[\d_]*/g, '').replace(/[+\-*/%()\s]/g, '') === '';
}

function numericResult(left, right) {
  if (left === right && NUMERIC.includes(left)) return left;
  if (['Int', 'double'].includes(left) && ['Int', 'double'].includes(right)) return 'double';
  return '';
}

// kesdas Series overloads +, -, *, /, %, **, floor division and unary minus, as well as
// + with a String. The element kind is not tracked, but the result is always a Series.
function isSeries(type) {
  return baseName(type) === 'Series' && !errorUnion(type);
}

function seriesOperator(op, left, right) {
  if (!isSeries(left) && !isSeries(right)) return '';
  if (op === '+' && isSeries(left) && right === 'String') return 'Series';
  if (!isSeries(left) && left !== 'Int' && left !== 'double') return '';
  if (!isSeries(right) && right !== 'Int' && right !== 'double') return '';
  return ['+', '-', '*', '/', '%', '**', '//'].includes(op) ? 'Series' : '';
}

function combineBinary(split, model, scope, depth) {
  const { op, left, right } = split;
  if (op === '|>') return pipeResult(left, right, model, scope, depth);
  if (op === '||' || op === '&&') {
    return infer(left, model, scope, depth + 1) === 'Bool' && infer(right, model, scope, depth + 1) === 'Bool'
      ? 'Bool'
      : '';
  }
  if (['==', '!=', '<=', '>=', '<', '>'].includes(op)) {
    return infer(left, model, scope, depth + 1) && infer(right, model, scope, depth + 1) ? 'Bool' : '';
  }
  const leftType = infer(left, model, scope, depth + 1);
  const rightType = infer(right, model, scope, depth + 1);
  if (!leftType || !rightType) return '';
  if (leftType === 'Variable' || rightType === 'Variable') {
    if (op === '@' || op === '**' || ['+', '-', '*', '/'].includes(op)) return 'Variable';
    return '';
  }
  if (isTensorType(leftType) || isTensorType(rightType)) {
    let tensorLeft = leftType;
    let tensorRight = rightType;
    if (isTensorType(tensorLeft) && ['+', '-', '*', '/'].includes(op) && integerLiteralText(right)) tensorRight = 'double';
    if (isTensorType(tensorRight) && ['+', '-', '*', '/'].includes(op) && integerLiteralText(left)) tensorLeft = 'double';
    return tensorOperator(op, tensorLeft, tensorRight);
  }
  if (op === '+' && leftType === 'String' && rightType === 'String') return 'String';
  const series = seriesOperator(op, leftType, rightType);
  if (series) return series;
  if (leftType === 'Complex' || rightType === 'Complex') {
    if (op === '**') return leftType === 'Complex' && rightType === 'Int' ? 'Complex' : '';
    return ['+', '-', '*', '/'].includes(op)
      && ['Complex', 'double'].includes(leftType)
      && ['Complex', 'double'].includes(rightType)
      ? 'Complex'
      : '';
  }
  return numericResult(leftType, rightType);
}

// `value |> name` calls the named function with the value, so its result type is the function's.
// A conversion name such as `double` or `Tensor` is callable here even though it is not a value.
function pipeResult(left, right, model, scope, depth) {
  const name = /^([A-Za-z_][A-Za-z0-9_]*)$/.exec(right.trim());
  if (!name) return '';
  if (['Tensor', 'Array', 'double', 'String'].includes(name[1]) && !(scope && scope.has(name[1]))) {
    return conversionResult(name[1], [left], model, scope, depth) || '';
  }
  const fn = model.functions.find(item => item.name === name[1] && !item.owner);
  if (fn) return functionResult(fn);
  const entry = builtins.named(name[1]).find(item => !item.receiver && !item.static);
  return entry ? resultOf(entry) : '';
}

// Fills generic variables from the receiver and from the arguments: a lambda argument fixes its
// output variable from its body, and a plain argument fixes a bare variable such as T.
function bindArguments(entry, initial, args, model, scope, depth) {
  const bindings = { ...initial };
  (entry.parameters || []).forEach((parameter, index) => {
    const argument = args[index];
    if (argument === undefined) return;
    const colon = parameter.indexOf(':');
    const declared = colon < 0 ? '' : parameter.slice(colon + 1).trim();
    // Tensor dimensions: `(Int, ...)` takes a tuple, and `Int, ...` takes the remaining numbers.
    // A dimension that is not a literal or a name makes the shape unknown, so the shape variable is dropped.
    if (declared === 'Int, ...') {
      setShape(bindings, shapeFrom(args.slice(index)));
      return;
    }
    if (declared === '(Int, ...)') {
      setShape(bindings, shapeText(argument));
      return;
    }
    if (/\bshape\b/.test(declared)) {
      const values = receiverBindings(infer(argument, model, scope, depth + 1), declared);
      for (const [key, value] of Object.entries(values || {})) {
        if (!(key in bindings)) bindings[key] = value;
      }
      return;
    }
    const arrow = declared.indexOf('->');
    if (arrow >= 0) {
      const lambda = parseLambda(argument);
      if (!lambda) return;
      const inputText = declared.slice(0, arrow).trim().replace(/^\((.*)\)$/, '$1').trim();
      const input = (lambda.annotation || substitute(inputText, bindings)).trim();
      if (!input || unresolved(input)) return;
      const inner = new Map(scope || []);
      inner.set(lambda.name, input);
      const body = infer(lambda.body, model, inner, depth + 1);
      const output = declared.slice(arrow + 2).trim();
      if (body && /^[A-Z]$/.test(output) && !(output in bindings)) bindings[output] = body;
      return;
    }
    if (/^[A-Z]$/.test(declared) && !(declared in bindings)) {
      const type = infer(argument, model, scope, depth + 1);
      if (type) bindings[declared] = type;
    }
  });
  return bindings;
}

function finish(entry, bindings) {
  const result = substitute(resultOf(entry), bindings);
  // A tensor whose dimensions are not literals has a run-time shape, which the checker calls DynTensor.
  const dynamic = result.replace(/\bTensor\[double, shape\]/g, 'DynTensor[double]');
  if (dynamic !== result && !/\bshape\b/.test(dynamic)) return dynamic;
  return unresolved(result) ? '' : result;
}

// The axes of a transpose written as a tuple of numbers, such as (1, 0, 2), or null when they are not literal.
function transposeAxes(argument) {
  const shape = shapeText(argument);
  if (!shape) return null;
  return shape.slice(1, -1).split(',').map(part => part.trim()).filter(Boolean).map(Number);
}

// `transpose()` reverses every axis, as NumPy's .T does, and `transpose((1, 0, 2))` permutes them.
// Dynamic shapes stay DynTensor, and an invalid permutation has no type here.
function transposeResult(receiver, args) {
  if (baseName(receiver) === 'DynTensor') return 'DynTensor[double]';
  const dims = tensorDimensions(receiver);
  if (!dims) return '';
  const order = args.length === 0 ? dims.map((_, index) => dims.length - 1 - index) : transposeAxes(args[0]);
  if (!order || order.length !== dims.length || new Set(order).size !== dims.length) return '';
  if (order.some(index => !Number.isInteger(index) || index < 0 || index >= dims.length)) return '';
  const shape = shapeFrom(order.map(index => dims[index]));
  return shape ? `Tensor[double, ${shape}]` : '';
}

function tensorType(dimensions) {
  if (dimensions === null) return 'DynTensor[double]';
  return `Tensor[double, (${dimensions.join(', ')}${dimensions.length === 1 ? ',' : ''})]`;
}

function literalDimension(value) {
  const text = value.trim().replace(/_/g, '');
  if (!/^(?:0[xX][0-9a-fA-F]+|[0-9]+)$/.test(text)) return null;
  try { return BigInt(text); } catch (_) { return null; }
}

function tensorIndexResult(receiver, written) {
  const dimensions = tensorDimensions(receiver);
  const indices = splitTopLevel(written).map(part => part.trim()).filter(Boolean);
  if (dimensions && indices.length !== dimensions.length) return '';
  const output = [];
  let dynamic = false;
  let sliced = false;
  for (let i = 0; i < indices.length; i += 1) {
    if (!indices[i].includes(':')) continue;
    sliced = true;
    const bounds = indices[i].split(':');
    if (bounds.length !== 2) return '';
    const [lo, hi] = bounds.map(bound => bound.trim());
    if (!lo && !hi && dimensions) { output.push(dimensions[i]); continue; }
    const low = lo ? literalDimension(lo) : 0n;
    const high = hi ? literalDimension(hi) : dimensions ? literalDimension(dimensions[i]) : null;
    if (low === null || high === null) dynamic = true;
    else if (high < low) return '';
    else output.push(String(high - low));
  }
  return sliced ? tensorType(dynamic ? null : output) : 'double';
}

// Preserve names until the builtin's parameter order is known.
function orderedArguments(raw, labels) {
  const result = [];
  let positional = 0;
  for (const argument of raw) {
    const named = /^([A-Za-z_]\w*)\s*=(?![=>])\s*([\s\S]*)$/.exec(argument) || /^(axis)\s*:\s*([\s\S]*)$/.exec(argument);
    const index = named ? labels.indexOf(named[1]) : positional++;
    if (index < 0 || index >= labels.length || result[index] !== undefined) return null;
    result[index] = named ? named[2] : argument;
  }
  return result;
}

function conversionResult(name, raw, model, scope, depth) {
  if (name === 'Tensor') {
    const args = orderedArguments(raw, raw.length === 2 ? ['values', 'shape'] : ['values']);
    if (!args || args[0] === undefined) return '';
    const source = infer(args[0], model, scope, depth + 1);
    const literal = args[0].trim();
    const list = literal.startsWith('[') && literal.endsWith(']') && balanced(literal.slice(1, -1));
    const count = list ? splitTopLevel(literal.slice(1, -1)).length : null;
    // A direct list of integer-literal expressions adopts double element by element,
    // exactly as `Tensor([0, 0])` and `Tensor([1 + 1, 2])` check. An Int variable,
    // an Array[Int] value, or an inexact literal does not.
    const items = list ? splitTopLevel(literal.slice(1, -1)).map(item => item.trim()).filter(Boolean) : [];
    const adoptable = items.length > 0 && items.every(item =>
      infer(item, model, scope, depth + 1) === 'Int' && integerLiteralExpression(item));
    if (
      source !== 'List[double]' &&
      source !== 'Array[double]' &&
      !(list && count === 0) &&
      !adoptable
    ) return '';
    if (args.length === 1) return count === null ? 'DynTensor[double]' : tensorType([String(count)]);
    const shape = shapeText(args[1]);
    if (!shape) return 'DynTensor[double]';
    const dimensions = shape.slice(1, -1).split(',').map(d => d.trim()).filter(Boolean);
    if (count !== null && dimensions.reduce((n, d) => n * BigInt(d.replace(/_/g, '')), 1n) !== BigInt(count)) return '';
    return `Tensor[double, ${shape}]`;
  }
  if (raw.length !== 1) return null;
  const args = orderedArguments(raw, [name === 'Array' ? 'values' : 'value']);
  if (!args || args[0] === undefined) return '';
  const source = infer(args[0], model, scope, depth + 1);
  if (name === 'double') return ['Int', 'Nat', 'double'].includes(source) ? 'double' : '';
  if (name === 'String') return 'String';
  if (name === 'Array') {
    const item = elementType(source);
    if (item) return `Array[${item}]`;
    return source ? methodResult(source, 'toArray', [], model, scope, depth + 1) : '';
  }
  return null;
}

function tensorFactoryResult(name, raw, model, scope, depth) {
  const labels = ['filled', 'generate'].includes(name) ? ['shape', name === 'filled' ? 'value' : 'factory']
    : ['onesLike', 'zerosLike', 'fromArray'].includes(name) ? ['source'] : ['shape'];
  const args = orderedArguments(raw, labels);
  if (!args || args.length !== labels.length || labels.some((_, index) => args[index] === undefined)) return '';
  if (name === 'fromArray') return 'DynTensor[double]';
  if (name === 'onesLike' || name === 'zerosLike') {
    const source = infer(args[0], model, scope, depth + 1);
    return isTensorType(source) ? source : '';
  }
  if (!['zeros', 'ones', 'filled', 'generate'].includes(name)) return '';
  const shape = shapeText(args[0]);
  return shape ? `Tensor[double, ${shape}]` : 'DynTensor[double]';
}

function tensorMethodResult(receiver, name, raw, model, scope, depth) {
  if (['text', 'debug'].includes(name)) return raw.length ? '' : 'String';
  if (['rank', 'count', 'shape', 'copy', 'norm'].includes(name)) {
    if (raw.length) return '';
    return name === 'copy' ? receiver : name === 'shape' ? 'Array[Int]' : name === 'norm' ? 'double' : 'Int';
  }
  if (['sum', 'mean', 'min', 'max'].includes(name)) {
    if (!raw.length) return 'double';
    const args = orderedArguments(raw, ['axis']);
    if (!args || args.length !== 1) return '';
    const dimensions = tensorDimensions(receiver);
    const axisText = args[0].trim().replace(/_/g, '');
    const axis = /^-?(?:0[xX][0-9a-fA-F]+|[0-9]+)$/.test(axisText) ? Number(axisText) : null;
    if (dimensions && axis !== null && (!Number.isSafeInteger(axis) || axis < 0 || axis >= dimensions.length)) return '';
    if (!dimensions) return 'DynTensor[double]';
    if (dimensions.length === 1) return 'double';
    return axis === null ? 'DynTensor[double]' : tensorType(dimensions.filter((_, index) => index !== axis));
  }
  if (name === 'transpose') {
    const args = argumentsOf(raw.join(','));
    if (args.length === 1 && infer(args[0], model, scope, depth + 1) === 'Array[Int]') return 'DynTensor[double]';
    return transposeResult(receiver, args);
  }
  if (name === 'checkShape') {
    if (raw.some(argument => NAMED_ARGUMENT.test(argument))) return '';
    const shape = shapeFrom(raw);
    return (shape ? `Tensor[double, ${shape}]` : 'DynTensor[double]') + ' or Error';
  }
  if (['broadcastTo', 'reshape'].includes(name)) {
    const args = orderedArguments(raw, ['shape']);
    if (!args || args.length !== 1) return '';
    const shape = shapeText(args[0]);
    if (!shape) return 'DynTensor[double]';
    const target = shape === '()' ? [] : shape.slice(1, -1).split(',').map(p => p.trim()).filter(Boolean);
    const source = tensorDimensions(receiver);
    if (source) {
      if (name === 'broadcastTo' && (source.length > target.length || source.slice().reverse().some((d, i) => d !== '1' && d !== target[target.length - 1 - i]))) return '';
      const count = dims => dims.some(d => literalDimension(d) === 0n) ? 0n : dims.reduce((total, d) => total === null || literalDimension(d) === null ? null : total * literalDimension(d), 1n);
      if (name === 'reshape' && count(source) !== null && count(source) !== count(target)) return '';
    }
    return `Tensor[double, ${shape}]`;
  }
  if (name === 'map') {
    const args = orderedArguments(raw, ['transform']);
    if (!args || args.length !== 1) return '';
    const lambda = parseLambda(args[0]);
    if (lambda) {
      const inner = new Map(scope || []);
      inner.set(lambda.name, 'double');
      const result = infer(lambda.body, model, inner, depth + 1);
      // The checker lets an exact integer literal expression adopt double here,
      // so `map(x => 1)` and `map(x => 1 + 1)` keep the tensor type.
      const literalDouble = result === 'Int' && exactDoubleLiteralExpression(lambda.body);
      if (result && result !== 'double' && !literalDouble) return '';
    }
    return receiver;
  }
  if (['add', 'subtract', 'multiply', 'divide', 'rightAdd', 'rightSubtract', 'rightMultiply'].includes(name)) {
    const args = orderedArguments(raw, [name.startsWith('right') ? 'left' : 'other']);
    if (!args || args.length !== 1) return '';
    const other = infer(args[0], model, scope, depth + 1);
    if (name.startsWith('right') && other !== 'double') return '';
    const result = tensorOperator(name === 'divide' ? '/' : name.toLowerCase().includes('multiply') ? '*' : name.toLowerCase().includes('subtract') ? '-' : '+', receiver, other);
    return result ? receiver : '';
  }
  return null;
}

function methodResult(receiver, name, args, model, scope, depth, raw = args) {
  if (errorUnion(receiver)) return '';
  if (receiver.startsWith('LU[') && name === 'solve') {
    const actual = orderedArguments(raw, ['b']);
    return actual?.length === 1 && isTensorType(infer(actual[0], model, scope, depth + 1)) ? infer(actual[0], model, scope, depth + 1) : '';
  }
  const base = baseName(receiver);
  // Series.iloc is the one overloaded kesdas method whose result differs by argument:
  // a position makes a Scalar, and a List[Int] makes a Series.
  if (base === 'Series' && name === 'iloc' && args.length === 1) {
    const argument = infer(args[0], model, scope, depth + 1);
    return baseName(argument) === 'List' ? 'Series' : argument ? 'Scalar' : '';
  }
  if (model.classes.some(item => item.name === base)) {
    const fn = chooseOverload(kestrel.memberFunctions(model, receiver).filter(item => item.name === name && !item.static), args, model, scope, depth, parameter => parameter.type);
    if (fn) {
      const bindings = bindArguments({ parameters: fn.parameters.map(parameter => parameter.label) }, {}, args, model, scope, depth);
      return substitute(functionResult(fn), bindings);
    }
  }
  if (isTensorType(receiver)) {
    const result = tensorMethodResult(receiver, name, raw, model, scope, depth);
    if (result !== null) return result;
  }
  let candidates = builtins.methods().filter(item => item.name === name && matchReceiver(receiver, item.receiver));
  if (candidates.length === 0) {
    for (const owner of typeChain(receiver, model).slice(1)) {
      candidates = builtins.methods().filter(item => item.name === name && matchReceiver(owner, item.receiver));
      if (candidates.length > 0) break;
    }
  }
  // A no-argument overload and a one-argument overload of the same name can return
  // different types, as DataFrame.dropNa() and dropNa(subset) do. Prefer the arity
  // the call actually uses; default parameters make the fallback necessary.
  const exact = candidates.filter(entry => (entry.parameters || []).length === args.length);
  for (const entry of exact.length > 0 ? exact : candidates) {
    const actual = matchReceiver(receiver, entry.receiver)
      ? receiver
      : typeChain(receiver, model).slice(1).find(owner => matchReceiver(owner, entry.receiver));
    if (!actual) continue;
    const values = matchReceiver(actual, entry.receiver);
    return finish(entry, bindArguments(entry, values, args, model, scope, depth));
  }
  return '';
}

function staticResult(typeName, name, args, model, scope, depth, qualified = typeName) {
  for (const owner of kestrel.classChain(model, typeName)) {
    const fn = model.functions.find(item => item.owner === owner && item.name === name && item.static);
    if (fn) return functionResult(fn);
  }
  for (const entry of builtins.entries.filter(item => item.static === typeName && item.name === name && builtins.staticAvailable(item, model, qualified))) {
    const result = finish(entry, bindArguments(entry, {}, args, model, scope, depth));
    if (result) return result;
  }
  return '';
}

// A generic class's constructor has the class's type arguments, read from the arguments:
// `Node(3, None, None)` for `class Node[T]` is `Node[Int]`. An argument that does not fix a
// type argument leaves the bare name, rather than a guess.
function constructedType(declared, model, args, scope, depth) {
  const names = declared.typeParameters || [];
  if (names.length === 0) return declared.name;
  const constructor = model.constructors.find(item => item.owner === declared.name);
  const entry = {
    parameters: (constructor ? constructor.parameters : []).map(parameter => `${parameter.name} : ${parameter.type}`)
  };
  const bindings = bindArguments(entry, {}, args, model, scope, depth);
  const values = names.map(name => bindings[name]);
  if (values.some(value => !value || unresolved(value))) return declared.name;
  return `${declared.name}[${values.join(', ')}]`;
}

// Whether a declared parameter can receive an argument of `actual`. This is a
// conservative match for choosing one overload: generic names and unknown
// argument types fit, and the only numeric adaptation is Int -> double/BigInt.
function parameterAccepts(declared, actual, model) {
  const wanted = (declared || '').replace(/\s*=[\s\S]*$/, '').trim();
  if (!wanted || !actual) return true;
  const base = baseName(wanted);
  const found = baseName(actual);
  if (!base || !found) return true;
  if (base === found) return true;
  if (/^[A-Z]$/.test(base) || base === 'T' || base === 'shape') return true;
  if (base === 'double' && (found === 'Int' || found === 'Nat')) return true;
  if (base === 'BigInt' && (found === 'Int' || found === 'Nat')) return true;
  if (base === 'Iterable' && ITERABLE_TYPES.includes(found)) return true;
  if (base === 'DynTensor' && found === 'Tensor') return true;
  if (base === 'Text' && model && ['String', 'Int', 'double', 'Bool', 'List', 'Map', 'Set', 'Option'].includes(found)) return true;
  return false;
}

// Among functions whose parameter count fits, prefer the one whose parameter
// types match the arguments: `sqrt(tensor)` must not take the double overload.
function chooseOverload(candidates, args, model, scope, depth, parameterOf) {
  const labelOf = parameter => typeof parameter === 'string' ? parameter : parameter.label || parameter.type || '';
  const declaredOf = parameter => typeof parameter === 'string' ? entryParameters([parameter])[0].type : parameterOf(parameter);
  const fitting = candidates.filter(candidate => {
    const parameters = candidate.parameters || [];
    const required = parameters.filter(parameter => !/=\s*[^,]*$/.test(labelOf(parameter))).length;
    return args.length >= required && args.length <= parameters.length;
  });
  if (fitting.length <= 1) return fitting[0] || candidates[0] || null;
  const actuals = args.map(argument => infer(argument, model, scope, depth + 1));
  const typed = fitting.find(candidate => (candidate.parameters || []).every((parameter, index) => {
    if (index >= actuals.length) return true;
    return parameterAccepts(declaredOf(parameter), actuals[index], model);
  }));
  return typed || fitting[0];
}

function callResult(name, args, model, scope, depth) {
  const fn = chooseOverload(
    model.functions.filter(item => item.name === name && !item.owner),
    args, model, scope, depth,
    parameter => parameter.type
  );
  if (fn) return functionResult(fn);
  const declared = model.classes.find(type => type.name === name);
  if (declared) return constructedType(declared, model, args, scope, depth);
  const candidate = chooseOverload(
    builtins.named(name).filter(item => !item.receiver && !item.static && !item.type && builtins.available(item, model)),
    args, model, scope, depth,
    parameter => parameter.type
  );
  if (!candidate) return '';
  const result = finish(candidate, bindArguments(candidate, {}, args, model, scope, depth));
  if (result) return result;
  // If the matching overload cannot fix its generics, try the other entries in
  // order; an unknown signature still yields no type rather than a wrong one.
  for (const entry of builtins.named(name).filter(item => !item.receiver && !item.static && !item.type && builtins.available(item, model))) {
    if (entry === candidate) continue;
    const fallback = finish(entry, bindArguments(entry, {}, args, model, scope, depth));
    if (fallback) return fallback;
  }
  return '';
}

// `Stack[String]()` and `Queue[Int]()` name their type arguments, so the type is known without a binding.
function explicitConstructorResult(callee, model) {
  const match = /^([A-Z][A-Za-z0-9_]*)\s*\[([\s\S]+)\]$/.exec(callee);
  if (!match) return '';
  const name = match[1];
  const declared = model.classes.find(type => type.name === name);
  const builtin = builtins.named(name).some(item => !item.receiver && !item.static && item.signature.startsWith(`${name}[`));
  if (!declared && !builtin) return '';
  return `${name}[${splitTopLevel(match[2]).map(part => part.trim()).join(', ')}]`;
}

function fieldResult(receiver, name, model) {
  if (errorUnion(receiver)) return '';
  if (name === 'T' && isTensorType(receiver)) return transposeResult(receiver, []);
  const kesdasField = kesdas.fieldTypes.get(`${baseName(receiver)}.${name}`);
  if (kesdasField) return kesdasField;
  const mlField = ml.fieldTypes.get(`${baseName(receiver)}.${name}`);
  if (mlField) return mlField;
  const kesnetField = kesnet.fieldTypes.get(`${baseName(receiver)}.${name}`)
    || kesnetOptim.fieldTypes.get(`${baseName(receiver)}.${name}`)
    || kesnetData.fieldTypes.get(`${baseName(receiver)}.${name}`);
  if (kesnetField) return kesnetField;
  if (baseName(receiver) === 'Complex' && (name === 're' || name === 'im')) return 'double';
  const distribution = distributions.byName.get(baseName(receiver));
  if (distribution) {
    const parameter = distribution.params.find(([parameter]) => parameter === name);
    if (parameter) return parameter[1];
  }
  const numericalField = numerical.fieldResult(receiver, name, splitTopLevel);
  if (numericalField !== null) return numericalField;
  for (const view of kestrel.typeViews(model, receiver)) {
    const field = view.declaration.fields.find(item => item.name === name);
    if (field) return field.type.replace(/\b[A-Za-z_]\w*\b/g, word => view.bindings[word] || word);
  }
  for (const owner of typeChain(receiver, model)) {
    const info = model.classes.find(item => item.name === owner);
    const field = info && info.fields.find(item => item.name === name);
    if (field) return field.type;
    const metadataField = kesnet.fieldTypes.get(`${owner}.${name}`)
      || kesnetOptim.fieldTypes.get(`${owner}.${name}`)
      || kesnetData.fieldTypes.get(`${owner}.${name}`);
    if (metadataField) return metadataField;
  }
  return '';
}

// Postfix forms read the value left to right: `a[i]`, `a.name`, `a.name(...)` and `f(...)`.
// Returns null when the value has none of these shapes.
function numericalResult(module, name, raw, model, scope, depth) {
  return numerical.result(module, name, raw, {
    ordered: orderedArguments, infer: expression => infer(expression, model, scope, depth + 1),
    dimensions: tensorDimensions, tensor: tensorType, isTensor: isTensorType
  });
}

function distributionSampleResult(receiverText, raw, model, scope, depth) {
  const match = /^(?:[A-Za-z_]\w*\.)?([A-Z][A-Za-z0-9_]*)$/.exec(receiverText);
  if (!match || (scope && scope.has(receiverText))) return null;
  const distribution = distributions.byName.get(match[1]);
  if (!distribution) return null;
  let shape = null;
  const positional = [];
  for (const argument of raw) {
    const named = /^([A-Za-z_]\w*)\s*=(?!=)\s*([\s\S]+)$/.exec(argument);
    if (named) {
      if (named[1] === 'shape') shape = named[2].trim();
      continue;
    }
    positional.push(argument);
  }
  let offset = 0;
  if (positional.length) {
    const first = infer(positional[0], model, scope, depth + 1);
    if (first && baseName(first) === 'Rng') offset = 1;
  }
  if (shape === null && positional.length > offset + distribution.params.length) {
    shape = positional[offset + distribution.params.length];
  }
  if (shape === null) return distribution.discrete ? 'Int' : 'double';
  const literal = literalDimension(shape);
  if (literal !== null) return tensorType([literal]);
  const dimensions = shape.startsWith('(') && shape.endsWith(')')
    ? splitTopLevel(shape.slice(1, -1)).map(part => literalDimension(part))
    : null;
  if (dimensions && dimensions.every(value => value !== null)) return tensorType(dimensions);
  return 'DynTensor[double]';
}

// The value written in one call argument, without a `name =` prefix.
function argumentValue(text) {
  const named = /^([A-Za-z_][A-Za-z0-9_]*)\s*=(?![=>])\s*([\s\S]*)$/.exec(text.trim());
  return (named ? named[2] : text).trim();
}

// Parameters of a builtin entry, such as `value : T`, split into name and type.
function entryParameters(parameters) {
  return (parameters || []).map(text => {
    const label = text.trim();
    const colon = label.indexOf(':');
    if (colon < 0) return { name: '', type: label, label };
    return {
      name: label.slice(0, colon).trim(),
      type: label.slice(colon + 1).replace(/\s*=\s*[\s\S]*$/, '').trim(),
      label
    };
  });
}

function chooseCallEntry(entries, supplied) {
  const exact = entries.filter(entry => (entry.parameters || []).length === supplied);
  return (exact.length === 1 ? exact[0] : null) || (entries.length === 1 ? entries[0] : null);
}

// The declared parameter list of the callable behind `callee`, for a partial call.
// Returns null when the callable is not a function, method, static or constructor
// whose signature the editor can read.
function callSignature(callee, raw, model, scope, depth) {
  const supplied = raw.length;
  const dot = lastMemberDot(callee);
  if (dot < 0) {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(callee) || (scope && scope.has(callee))) return null;
    const fn = model.functions.find(item => item.name === callee && !item.owner);
    if (fn) return { name: callee, params: fn.parameters || [], result: functionResult(fn) };
    const constructor = model.constructors.find(item => item.name === callee);
    if (constructor) return { name: callee, params: constructor.parameters || [], result: callee };
    const entry = chooseCallEntry(
      builtins.named(callee).filter(item => !item.receiver && !item.static && builtins.available(item, model)),
      supplied
    );
    if (entry) return { name: callee, params: entryParameters(entry.parameters), result: resultOf(entry), entry };
    return null;
  }
  const receiverText = callee.slice(0, dot).trim();
  const name = callee.slice(dot + 1).trim();
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) return null;
  const module = builtins.moduleForReceiver(model, receiverText);
  if (module) {
    const entry = chooseCallEntry(
      builtins.named(name).filter(item => item.module === module && !item.receiver),
      supplied
    );
    if (entry) return { name, params: entryParameters(entry.parameters), result: resultOf(entry), entry };
    return null;
  }
  if (/^[A-Z][A-Za-z0-9_]*$/.test(receiverText) && !(scope && scope.has(receiverText))) {
    const entry = chooseCallEntry(
      builtins.named(name).filter(item => item.static === receiverText),
      supplied
    );
    if (entry) return { name, params: entryParameters(entry.parameters), result: resultOf(entry), entry };
    const fn = model.functions.find(item => item.owner === receiverText && item.name === name);
    if (fn) return { name, params: fn.parameters || [], result: functionResult(fn) };
    return null;
  }
  const receiver = infer(receiverText, model, scope, depth + 1);
  if (!receiver || errorUnion(receiver)) return null;
  const base = baseName(receiver);
  const fn = model.functions.find(item => item.owner === base && item.name === name);
  if (fn) return { name, params: fn.parameters || [], result: functionResult(fn) };
  const entry = chooseCallEntry(
    builtins.methods().filter(item => item.name === name && matchReceiver(receiver, item.receiver)),
    supplied
  );
  if (entry) return { name, params: entryParameters(entry.parameters), result: resultOf(entry), entry };
  return null;
}

// A call with `_` leaves a function waiting for its open slots, not the call's
// result: `add(1.0, _)` is `(b : double) -> double`. An unknown signature gives
// an empty type rather than a wrong result type.
function partialCallResult(callee, raw, model, scope, depth) {
  // The checker treats `double`, `Tensor`, `Array` and `String` as conversion
  // syntax, not as callables that can leave a slot open; do not invent a type
  // for a call the compiler refuses.
  if (/^(?:Tensor|Array|double|String)$/.test(callee) && !(scope && scope.has(callee))) return '';
  const signature = callSignature(callee, raw, model, scope, depth);
  if (!signature) return '';
  // Tensor calls are checked by a dedicated path that does not accept open
  // slots, so the editor must not claim a partial the compiler rejects.
  if (signature.entry
    && (signature.entry.static === 'Tensor'
      || /^(?:Dyn)?Tensor\b/.test(signature.entry.receiver || ''))) return '';
  const supplied = new Map();
  let position = 0;
  for (const argument of raw) {
    const named = /^([A-Za-z_][A-Za-z0-9_]*)\s*=(?![=>])\s*([\s\S]*)$/.exec(argument.trim());
    if (named) {
      const index = signature.params.findIndex(parameter => parameter.name === named[1]);
      if (index >= 0) supplied.set(index, named[2].trim());
    } else {
      while (supplied.has(position)) position += 1;
      if (position < signature.params.length) supplied.set(position, argument.trim());
      position += 1;
    }
  }
  const holes = [];
  signature.params.forEach((_, index) => {
    if (supplied.get(index) === '_') holes.push(index);
  });
  if (holes.length === 0) return null;
  const ordered = signature.params.map((_, index) => supplied.get(index));
  const bindings = bindArguments({ parameters: signature.params.map(parameter => parameter.label) }, {}, ordered, model, scope, depth);
  const remaining = [];
  for (const index of holes) {
    const type = substitute(signature.params[index].type, bindings);
    if (!type || unresolved(type)) return '';
    remaining.push(signature.params[index].name ? `${signature.params[index].name} : ${type}` : type);
  }
  const result = substitute(signature.result, bindings).replace(/\s+with\s+Effects\b.*$/, '').trim();
  if (!result || unresolved(result)) return '';
  return `(${remaining.join(', ')}) -> ${result}`;
}

function postfixResult(value, model, scope, depth) {
  if (value.endsWith(']')) {
    const open = openerOf(value, value.length - 1);
    if (open <= 0) return null;
    const receiver = infer(value.slice(0, open), model, scope, depth + 1);
    if (receiver === 'Variable') return 'Variable';
    if (isTensorType(receiver)) return tensorIndexResult(receiver, value.slice(open + 1, -1));
    if (baseName(receiver) === 'String') return 'String';
    if (receiver.startsWith('(') && receiver.endsWith(')') && balanced(receiver.slice(1, -1))) {
      const index = literalDimension(value.slice(open + 1, -1));
      const parts = splitTopLevel(receiver.slice(1, -1)).map(part => part.trim());
      return index === null ? '' : parts[Number(index)] || '';
    }
    return baseName(receiver) === 'Array' ? typeArguments(receiver)[0] || '' : '';
  }
  if (value.endsWith(')')) {
    const open = openerOf(value, value.length - 1);
    if (open < 0) return null;
    const callee = value.slice(0, open).trim();
    const raw = splitTopLevel(value.slice(open + 1, -1)).map(part => part.trim()).filter(Boolean);
    const args = argumentsOf(value.slice(open + 1, -1));
    if (raw.some(argument => argumentValue(argument) === '_')) {
      return partialCallResult(callee, raw, model, scope, depth);
    }
    const dot = lastMemberDot(callee);
    if (dot >= 0) {
      const receiverText = callee.slice(0, dot).trim();
      const name = callee.slice(dot + 1).trim();
      if (!/^[A-Za-z_]\w*$/.test(name)) return '';
      if (name === 'sample') {
        const probability = distributionSampleResult(receiverText, raw, model, scope, depth);
        if (probability !== null) return probability;
      }
      const module = builtins.moduleForReceiver(model, receiverText);
      if (module && !(scope && scope.has(receiverText))) {
        const result = numericalResult(module, name, raw, model, scope, depth);
        if (result !== null) return result;
        if (module === 'std.complex' && name === 'Complex') return 'Complex';
        // A namespace import exposes constructors and functions: kesdas.DataFrame(...)
        // and kesdas.readCsv(...), without listing the names in the import.
        const typeEntry = builtins.named(name).find(entry => entry.type && entry.module === module);
        const constructor = builtins.named(name).find(entry => entry.named && entry.module === module);
        if (typeEntry && constructor) return name;
        const functionEntry = builtins.named(name).find(entry => !entry.receiver && !entry.static && entry.module === module && entry.signature.includes('('));
        if (functionEntry) {
          const qualifiedResult = finish(functionEntry, bindArguments(functionEntry, {}, args, model, scope, depth));
          if (qualifiedResult) return qualifiedResult;
        }
      }
      const qualifiedClass = receiverText.split('.');
      if (qualifiedClass.length === 2) {
        const ownerModule = builtins.moduleForReceiver(model, qualifiedClass[0]);
        const owner = qualifiedClass[1];
        if (ownerModule) {
          const member = builtins.named(name).find(entry => entry.static === owner && entry.module === ownerModule);
          if (member) {
            const qualifiedResult = finish(member, bindArguments(member, {}, args, model, scope, depth));
            if (qualifiedResult) return qualifiedResult;
          }
          return staticResult(owner, name, args, model, scope, depth, receiverText);
        }
      }
      if (receiverText === 'Tensor' && !(scope && scope.has(receiverText))) return tensorFactoryResult(name, raw, model, scope, depth);
      const enumcase = model.classes.find(type => type.name === receiverText && type.kind === 'enum'
        && type.cases.some(variant => variant.name === name));
      if (enumcase && !(scope && scope.has(receiverText))) return enumcase.name;
      const staticField = !(scope && scope.has(receiverText))
        && model.classes.find(type => type.name === receiverText)
          ?.fields.find(field => field.static && field.name === name);
      if (staticField) return staticField.type;
      if (/^[A-Z][A-Za-z0-9_]*$/.test(receiverText) && !(scope && scope.has(receiverText))) return staticResult(receiverText, name, args, model, scope, depth);
      const receiver = infer(receiverText, model, scope, depth + 1);
      return receiver ? methodResult(receiver, name, args, model, scope, depth, raw) : '';
    }
    if (/^[A-Za-z_]\w*$/.test(callee)) {
      const module = builtins.moduleForName(model, callee);
      if (module && !(scope && scope.has(callee))) {
        const result = numericalResult(module, callee, raw, model, scope, depth);
        if (result !== null) return result;
      }
      if (['Tensor', 'Array', 'double', 'String'].includes(callee) && !(scope && scope.has(callee)) && !model.classes.some(c => c.name === callee)) {
        const result = conversionResult(callee, raw, model, scope, depth);
        if (result !== null) return result;
      }
      return callResult(callee, args, model, scope, depth);
    }
    return explicitConstructorResult(callee, model);
  }
  const dot = lastMemberDot(value);
  if (dot >= 0) {
    const name = value.slice(dot + 1).trim();
    if (/^[A-Za-z_]\w*$/.test(name)) {
      const receiverText = value.slice(0, dot).trim();
      const enumcase = model.classes.find(type => type.name === receiverText && type.kind === 'enum'
        && type.cases.some(variant => variant.name === name));
      if (enumcase && !(scope && scope.has(receiverText))) return enumcase.name;
      const staticField = !(scope && scope.has(receiverText))
        && model.classes.find(type => type.name === receiverText)
          ?.fields.find(field => field.static && field.name === name);
      if (staticField) return staticField.type;
      const receiver = infer(receiverText, model, scope, depth + 1);
      return receiver ? fieldResult(receiver, name, model) : '';
    }
  }
  return null;
}

// `x : Int => x * 2` has the type `(Int) -> Int`. A lambda without an annotation has no type of its own.
function lambdaType(lambda, model, scope, depth) {
  if (!lambda.annotation) return '';
  const inner = new Map(scope || []);
  inner.set(lambda.name, lambda.annotation);
  const body = infer(lambda.body, model, inner, depth + 1);
  return body ? `(${lambda.annotation}) -> ${body}` : '';
}

function infer(expression, model, scope, depth = 0) {
  if (depth > MAX_DEPTH) return '';
  let value = expression.trim().replace(/;$/, '').trim();
  while (
    value.startsWith('(') &&
    value.endsWith(')') &&
    balanced(value.slice(1, -1)) &&
    splitTopLevel(value.slice(1, -1)).length === 1
  ) {
    value = value.slice(1, -1).trim();
  }
  if (!value) return '';
  if (/^f?"(?:[^"\\]|\\.)*"$/.test(value)) return 'String';
  if (NUMBER.test(value)) return /[.eE]/.test(value) && !/^0[xX]/.test(value) ? 'double' : 'Int';
  if (value === 'true' || value === 'false') return 'Bool';
  if (value === 'None') return 'Option[T]';
  const lambda = parseLambda(value);
  if (lambda) return lambdaType(lambda, model, scope, depth);
  const conditional = /^if\s+([\s\S]+?)\s+then\s+([\s\S]+?)\s+else\s+([\s\S]+)$/.exec(value);
  if (conditional) {
    const left = infer(conditional[2], model, scope, depth + 1);
    const right = infer(conditional[3], model, scope, depth + 1);
    return left && left === right ? left : '';
  }
  if (value.startsWith('[') && value.endsWith(']') && balanced(value.slice(1, -1))) {
    return listType(splitTopLevel(value.slice(1, -1)), model, scope, depth);
  }
  if (value.startsWith('(') && value.endsWith(')') && balanced(value.slice(1, -1))) {
    const parts = splitTopLevel(value.slice(1, -1)).map(part => infer(part, model, scope, depth + 1));
    if (parts.length > 0 && parts.every(Boolean)) return `(${parts.join(', ')})`;
  }
  const mapArgs = constructorArgs(value, 'Map');
  if (mapArgs !== null) {
    const pairs = splitTopLevel(mapArgs).map(part => {
      const arrow = topLevel(part, '=');
      if (arrow < 0 || part[arrow + 1] !== '>') return null;
      const key = part.slice(0, arrow).trim();
      const item = part.slice(arrow + 2).trim();
      return [infer(key, model, scope, depth + 1), infer(item, model, scope, depth + 1)];
    });
    if (pairs.length > 0 && pairs.every(pair => pair && pair[0] && pair[1])) {
      const keys = [...new Set(pairs.map(pair => pair[0]))];
      const values = [...new Set(pairs.map(pair => pair[1]))];
      if (keys.length === 1 && values.length === 1) return `Map[${keys[0]}, ${values[0]}]`;
    }
    return '';
  }
  for (const constructor of ['List', 'Set', 'SortedSet']) {
    const args = constructorArgs(value, constructor);
    if (args === null) continue;
    const parts = splitTopLevel(args);
    const types = parts.map(part => infer(part, model, scope, depth + 1)).filter(Boolean);
    if (types.length > 0 && types.length === parts.length) {
      const unique = [...new Set(types)];
      if (unique.length === 1) return `${constructor}[${unique[0]}]`;
    }
    return '';
  }
  if (IDENTIFIER.test(value)) {
    if (scope && scope.has(value)) return scope.get(value);
    const constant = model.constants.find(entry => entry.name === value && entry.type);
    if (constant) return constant.type;
    const entry = builtins.named(value).find(item => !item.receiver);
    if (entry) return signatureResult(entry) === '' ? '' : entry.signature.slice(entry.signature.indexOf(':') + 1).trim();
    return '';
  }
  const split = splitBinary(value);
  if (split) return combineBinary(split, model, scope, depth);
  if (value.startsWith('-')) {
    const inner = infer(value.slice(1), model, scope, depth + 1);
    return NUMERIC.includes(inner) || isTensorType(inner) || inner === 'Complex' || inner === 'Series' ? inner : '';
  }
  if (value.startsWith('!')) return infer(value.slice(1), model, scope, depth + 1) === 'Bool' ? 'Bool' : '';
  if (value.endsWith('?')) {
    const inner = infer(value.slice(0, -1), model, scope, depth + 1);
    if (/\s+or\s+Error$/.test(inner)) return inner.replace(/\s+or\s+Error$/, '');
    if (baseName(inner) === 'Option') return typeArguments(inner)[0] || '';
    return '';
  }
  const postfix = postfixResult(value, model, scope, depth);
  return postfix === null ? '' : postfix;
}

// The index of the bracket that closes the one opening at `openIndex`, or -1.
function closerOf(text, openIndex) {
  let depth = 0;
  let i = openIndex;
  while (i < text.length) {
    const current = text[i];
    if (current === '"') {
      i = skipString(text, i);
      continue;
    }
    if ('([{'.includes(current)) depth += 1;
    else if (')]}'.includes(current)) {
      depth -= 1;
      if (depth === 0) return i;
    }
    i += 1;
  }
  return -1;
}

// `for (let x in items)` binds x to the element type of items; `for (let x : T in items)` states it.
function loopBindings(text, clean) {
  const hit = cached(loopBindingCache, text);
  if (hit !== undefined) return hit;
  const found = [];
  const pattern = /\bfor\s*\(\s*(?:(?:let|var)\s+(?:mut\s+)?)?([A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*([^=;()]+?))?\s+in\s+/g;
  let match;
  while ((match = pattern.exec(clean)) !== null) {
    const open = clean.indexOf('(', match.index);
    const close = closerOf(clean, open);
    if (close < 0) continue;
    const name = match[1];
    const head = /^\bfor\s*\(\s*(?:(?:let|var)\s+(?:mut\s+)?)?/.exec(match[0])[0];
    const nameStart = match.index + head.length;
    found.push({
      name,
      nameStart,
      nameEnd: nameStart + name.length,
      rhsStart: match.index + match[0].length,
      rhsEnd: close,
      annotation: match[2] ? match[2].trim() : '',
      kind: 'loop',
      text: text.slice(match.index + match[0].length, close)
    });
  }
  remember(loopBindingCache, text, found);
  return found;
}

// The element type of an iterable such as List[Int], Array[Int] or Lazy[Int].
function elementType(type) {
  const args = typeArguments(type || '');
  return ITERABLE_TYPES.includes(baseName(type)) && args.length === 1 ? args[0] : '';
}

// The names visible at an offset: the enclosing function's parameters, and `this` in a method.
function scopeAt(model, offset, scope) {
  const local = new Map(scope);
  const fn = enclosingFunction(model, offset);
  if (fn) {
    for (const parameter of fn.parameters || []) {
      if (parameter.name && parameter.type) local.set(parameter.name, parameter.type);
    }
    if (fn.owner) local.set('this', fn.owner);
  }
  return local;
}

// The source with each // comment blanked out. String literals are kept, so their types still read.
function stripComments(text) {
  const hit = cached(commentCache, text);
  if (hit !== undefined) return hit;
  let result = '';
  let i = 0;
  while (i < text.length) {
    if (text[i] === '"') {
      const end = skipString(text, i);
      result += text.slice(i, end);
      i = end;
      continue;
    }
    if (text.startsWith('//', i)) {
      while (i < text.length && text[i] !== '\n') {
        result += ' ';
        i += 1;
      }
      continue;
    }
    result += text[i];
    i += 1;
  }
  remember(commentCache, text, result);
  return result;
}

function inferredBindings(text, model, until = Infinity) {
  // Asking through the end of the file is a full pass, so it can be cached and
  // every later provider call becomes a filter.
  if (until >= text.length) until = Infinity;
  const hit = cached(inferredCache, text);
  if (hit !== undefined) {
    // A full answer is already known; a prefix is just a slice. This is what a
    // completion at the cursor asks for, so it does not pay for later bindings.
    return until === Infinity ? hit : hit.filter(binding => binding.nameStart < until);
  }
  const clean = kestrel.sanitize(text);
  const code = stripComments(text);
  const result = [];
  const scope = new Map();
  const found = [...bindings(text), ...loopBindings(text, clean)].sort((a, b) => a.nameStart - b.nameStart);
  for (const binding of found) {
    // Names after the cursor cannot affect its scope.
    if (binding.nameStart >= until) break;
    const local = scopeAt(model, binding.nameStart, scope);
    let inferred = '';
    if (!binding.annotation) {
      if (binding.kind === 'loop') {
        const iterable = infer(binding.text, model, local);
        inferred = elementType(iterable);
      } else {
        const end = expressionEnd(clean, binding.rhsStart);
        const source = code.slice(binding.rhsStart, end);
        if (binding.component !== undefined) {
          // `let (a, b) := pair` names the components of the right side's tuple type.
          const tuple = infer(source, model, local);
          const parts = tuple.startsWith('(') && tuple.endsWith(')') && balanced(tuple.slice(1, -1))
            ? splitTopLevel(tuple.slice(1, -1)).map(part => part.trim())
            : [];
          inferred = parts[binding.component] || '';
        } else {
          inferred = infer(source, model, local);
        }
      }
    }
    const type = binding.annotation || inferred;
    const { text: _text, ...entry } = binding;
    result.push({ ...entry, type });
    if (type) scope.set(binding.name, type);
  }
  // A prefix is not the whole answer, so only a full pass is cached.
  if (until === Infinity) remember(inferredCache, text, result);
  return result;
}

// The expression written before the member dot at `offset`: `[1, 2, 3]` in `[1, 2, 3].ma`, or
// `[1, 2].map(f)` in `[1, 2].map(f).`. Brackets are matched and string literals are kept whole.
// Returns '' when no member dot is being typed.
function memberReceiverExpression(text, offset) {
  // Comments are blanked so a dot inside one cannot start a chain, but strings
  // are kept because an argument such as `col("v")` is part of the receiver.
  const source = stripComments(text);
  const dot = /\.\s*[A-Za-z0-9_]*$/.exec(source.slice(0, offset));
  if (!dot) return '';
  const end = dot.index;
  let depth = 0;
  // A dot may sit on its own indented line after a call.
  let i = end - 1;
  while (i >= 0 && /\s/.test(source[i])) i -= 1;
  while (i >= 0) {
    const current = text[i];
    if (current === '"') {
      const open = source.lastIndexOf('"', i - 1);
      if (open < 0) break;
      i = open - 1;
      continue;
    }
    if (')]}'.includes(current)) depth += 1;
    else if ('([{'.includes(current)) {
      if (depth === 0) break;
      depth -= 1;
    } else if (depth === 0 && current === '?') {
      // `df.col("x")?` and `groupBy("k")?.` keep the unwrapped value as the receiver.
    } else if (depth === 0 && !/[A-Za-z0-9_.]/.test(current)) {
      // Whitespace is part of the receiver only when the chain continues after it,
      // as in `node.next()\n    .next()`. A new statement starts otherwise.
      if (/\s/.test(current)) {
        let before = i;
        while (before >= 0 && /\s/.test(source[before])) before -= 1;
        let after = i + 1;
        while (after < end && /\s/.test(source[after])) after += 1;
        if (after < end && source[after] === '.') {
          i = before;
          continue;
        }
      }
      break;
    }
    i -= 1;
  }
  return source.slice(i + 1, end).trim();
}

// The type of an expression written at `offset`, with the names and parameters visible there.
function typeAt(text, offset, model, expression, known) {
  const types = new Map();
  if (known) {
    for (const [name, entry] of known) types.set(name, entry.type);
  } else {
    for (const [name, entry] of localTypes(text, offset, model)) types.set(name, entry.type);
  }
  return infer(expression, model, scopeAt(model, offset, types));
}

// Lambdas are the one place where the editor knows a parameter's type from its
// call. Finding them used to re-scan and re-parse the whole file for every
// hint; the brackets and the lambda facts are now computed once per text.
function bracketFacts(text) {
  const clean = kestrel.sanitize(text);
  const length = clean.length;
  const closing = new Int32Array(length);
  closing.fill(-1);
  const enclosing = new Int32Array(length);
  enclosing.fill(-1);
  const stack = [];
  for (let i = 0; i < length; i += 1) {
    enclosing[i] = stack.length ? stack[stack.length - 1] : -1;
    const current = clean[i];
    if (current === '(' || current === '[' || current === '{') {
      stack.push(i);
    } else if (current === ')' || current === ']' || current === '}') {
      const opening = current === ')' ? '(' : current === ']' ? '[' : '{';
      for (let j = stack.length - 1; j >= 0; j -= 1) {
        if (clean[stack[j]] === opening) {
          closing[stack[j]] = i;
          stack.length = j;
          break;
        }
      }
    }
  }
  return { clean, closing, enclosing };
}

function lambdaFacts(text) {
  const hit = cached(lambdaCache, text);
  if (hit !== undefined) return hit;
  const { clean, closing, enclosing } = bracketFacts(text);
  const pattern = /(\([^()]*\)|[A-Za-z_]\w*(?:\s*:\s*[A-Za-z_]\w*)?)\s*=>/g;
  const found = [];
  let match;
  while ((match = pattern.exec(clean)) !== null) {
    const open = enclosing[match.index];
    if (open < 0 || clean[open] !== '(') continue;
    // An incomplete call has no closer yet. Its lambda parameters are still
    // visible at the cursor, so treat the call as reaching the end of the file.
    const close = closing[open] < 0 ? Number.MAX_SAFE_INTEGER : closing[open];
    // The callee is the identifier directly before the call's opening parenthesis.
    let end = open - 1;
    while (end >= 0 && /\s/.test(clean[end])) end -= 1;
    let start = end + 1;
    while (start > 0 && /[A-Za-z0-9_]/.test(clean[start - 1])) start -= 1;
    const callee = clean.slice(start, end + 1);
    let dot = start - 1;
    while (dot >= 0 && /\s/.test(clean[dot])) dot -= 1;
    const method = dot >= 0 && clean[dot] === '.';
    let receiver = '';
    if (method) {
      let receiverEnd = dot;
      while (receiverEnd > 0 && /\s/.test(clean[receiverEnd - 1])) receiverEnd -= 1;
      let receiverStart = receiverEnd;
      while (receiverStart > 0 && /[A-Za-z0-9_]/.test(clean[receiverStart - 1])) receiverStart -= 1;
      receiver = clean.slice(receiverStart, receiverEnd);
    }
    found.push({ name: match[1], start: match.index, end: close, callee, method, receiver });
  }
  remember(lambdaCache, text, found);
  return found;
}

function tensorCallbackTypes(text, offset, model, types, facts) {
  for (const fact of facts || lambdaFacts(text)) {
    // The parameter is visible only inside the call that receives the lambda.
    if (!(fact.start < offset && offset <= fact.end)) continue;
    let expected = '';
    if (fact.method && fact.callee === 'generate' && fact.receiver === 'Tensor') expected = 'Int';
    if (fact.method && fact.callee === 'map') {
      const known = types.get(fact.receiver);
      if (known && isTensorType(known.type)) expected = 'double';
    }
    if (!expected) continue;
    for (const parameter of splitTopLevel(fact.name.replace(/^\((.*)\)$/, '$1'))) {
      const declaration = /^\s*([A-Za-z_]\w*)(?:\s*:\s*(.+))?\s*$/.exec(parameter);
      if (declaration) types.set(declaration[1], { type: declaration[2] || expected, kind: 'parameter' });
    }
  }
}

function localTypes(text, offset, model) {
  const types = new Map();
  for (const binding of kestrel.proofBindings(text, offset, model)) types.set(binding.name, binding);
  const fn = enclosingFunction(model, offset);
  if (fn) {
    for (const parameter of fn.parameters) {
      if (parameter.name && parameter.type) types.set(parameter.name, { type: parameter.type, kind: 'parameter' });
    }
  }
  for (const binding of inferredBindings(text, model, offset)) {
    if (binding.nameStart < offset && binding.type) {
      types.set(binding.name, { type: binding.type, kind: 'binding' });
    }
  }
  tensorCallbackTypes(text, offset, model, types, lambdaFacts(text));
  return types;
}

module.exports = { typeChain, metadataParent,
  bindings,
  loopBindings,
  inferredBindings,
  infer,
  localTypes,
  enclosingFunction,
  splitTopLevel,
  topLevel,
  memberReceiverExpression,
  scopeAt,
  typeAt
};
