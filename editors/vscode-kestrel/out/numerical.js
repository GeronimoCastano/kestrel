'use strict';

const unary = new Set(['exp', 'expm1', 'log', 'log1p', 'log2', 'log10', 'sin', 'cos', 'tan', 'asin', 'acos', 'atan', 'sinh', 'cosh', 'tanh', 'sqrt', 'abs', 'floor', 'ceil', 'round']);
const components = { LU: ['factors', 'pivots'], QR: ['q', 'r'], SVD: ['u', 's', 'vt'], Eigh: ['values', 'vectors'] };
function fieldResult(receiver, name, split) {
  const match = /^(LU|QR|SVD|Eigh)\[(.*)\]$/.exec(receiver);
  if (!match) return null;
  const index = components[match[1]].indexOf(name);
  return index < 0 ? '' : split(match[2])[index]?.trim() || '';
}
function result(module, name, raw, h) {
  if (module === 'std.math' && unary.has(name)) {
    const args = h.ordered(raw, ['value']);
    if (!args || args.length !== 1) return '';
    const input = h.infer(args[0]);
    if (input === 'double' || h.isTensor(input)) return input;
    return /^[+-]?(?:0[xX][0-9a-fA-F]+|[0-9][0-9_]*)$/.test(args[0].trim()) ? 'double' : '';
  }
  const stats = module === 'std.stats';
  if (!stats && module !== 'std.linalg') return null;
  const labels = stats ? name === 'mean' ? ['source', 'axis'] : ['variance', 'stddev'].includes(name) ? ['source', 'axis', 'ddof'] : ['source', 'ddof']
    : ['pinv', 'rank'].includes(name) ? ['a', 'rcond'] : name === 'leastSquares' ? ['a', 'b', 'rcond']
      : ['dot', 'outer', 'solve', 'solveLower', 'solveUpper'].includes(name) ? ['a', 'b'] : ['a'];
  const args = h.ordered(raw, labels);
  if (!args || args[0] === undefined) return '';
  const first = h.infer(args[0]);
  if (!h.isTensor(first)) return '';
  const a = h.dimensions(first);
  const tensor = dims => dims ? h.tensor(dims) : 'DynTensor[double]';
  if (stats) {
    if (['covariance', 'correlation'].includes(name)) return a ? a.length === 2 ? tensor([a[1], a[1]]) : '' : tensor(null);
    if (!['mean', 'variance', 'stddev'].includes(name)) return '';
    if (args[1] === undefined) return 'double';
    const axis = /^-?\d+$/.test(args[1].trim()) ? Number(args[1]) : null;
    if (a && axis !== null && (axis < 0 || axis >= a.length)) return '';
    if (a?.length === 1) return 'double';
    return a && axis !== null ? tensor(a.filter((_, i) => i !== axis)) : tensor(null);
  }
  if (['norm', 'frobeniusNorm'].includes(name)) return raw.length === 1 ? 'double' : '';
  if (a && a.length !== (['dot', 'outer'].includes(name) ? 1 : 2)) return '';
  const square = ['solve', 'solveLower', 'solveUpper', 'inverse', 'det', 'eigh', 'cholesky'].includes(name);
  if (square && a && a[0] !== a[1]) return '';
  if (['dot', 'outer', 'solve', 'solveLower', 'solveUpper', 'leastSquares'].includes(name)) {
    if (args[1] === undefined) return '';
    const second = h.infer(args[1]);
    if (!h.isTensor(second)) return '';
    const b = h.dimensions(second);
    if (b && (['dot', 'outer'].includes(name) ? b.length !== 1 : ![1, 2].includes(b.length))) return '';
    if (name !== 'outer' && a && b && a[0] !== b[0]) return '';
    if (name === 'dot') return 'double';
    if (name === 'outer') return tensor(a && b ? [a[0], b[0]] : null);
    if (name === 'leastSquares') return tensor(a && b ? [a[1], ...b.slice(1)] : null);
    return second;
  }
  if (['det', 'spectralNorm'].includes(name)) return 'double';
  if (name === 'rank') return 'Int';
  if (name === 'pinv') return tensor(a ? [a[1], a[0]] : null);
  if (['inverse', 'cholesky'].includes(name)) return first;
  const k = a && a.every(d => /^\d+$/.test(d)) ? String(BigInt(a[0]) < BigInt(a[1]) ? BigInt(a[0]) : BigInt(a[1]))
    : a && a[0] === a[1] ? a[0] : null;
  if (name === 'lu') return `LU[${first}, Array[Int]]`;
  if (name === 'qr') return `QR[${tensor(a && k ? [a[0], k] : null)}, ${tensor(a && k ? [k, a[1]] : null)}]`;
  if (name === 'svd') return `SVD[${tensor(a && k ? [a[0], k] : null)}, ${tensor(k ? [k] : null)}, ${tensor(a && k ? [k, a[1]] : null)}]`;
  if (name === 'eigh') return `Eigh[${tensor(a ? [a[0]] : null)}, ${first}]`;
  return '';
}
module.exports = { result, fieldResult, unary };
