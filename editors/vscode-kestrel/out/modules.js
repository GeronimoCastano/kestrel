'use strict';

const distributions = require('./distributions');

// The standard modules the compiler can import and the names each one exports.
// These lists mirror src/modules.rs and src/numerical.rs; the compiler remains
// the authority, and this table only drives editor completion.
const modules = [
  { path: 'std.collections', names: ['List', 'Array', 'Map', 'MutMap', 'Set', 'MutSet', 'SortedSet'] },
  { path: 'std.algebra', names: ['Group', 'Ring', 'Z2', 'Z2Group', 'z2Plus', 'z2Times', 'z2Neg', 'z2PlusAssoc', 'z2PlusZeroL', 'z2PlusNeg', 'z2TimesAssoc', 'z2Distrib'] },
  { path: 'std.complex', names: ['Complex'] },
  { path: 'std.func', names: ['identity', 'compose', 'constant'] },
  { path: 'std.nat', names: ['plus', 'times', 'plusZeroR', 'plusSuccR', 'timesZeroR'] },
  { path: 'std.real', names: ['Real', 'PI', 'abs', 'sqrt', 'min', 'isFinite'] },
  { path: 'std.set', names: ['isEmpty', 'member', 'subset', 'properSubset', 'disjoint', 'cardinality'] },
  { path: 'std.kesdas', names: require('./kesdas').names },
  { path: 'std.kesnet', names: require('./kesnet').names },
  { path: 'std.kesnet.optim', names: require('./kesnet_optim').names },
  { path: 'std.kesnet.data', names: require('./kesnet_data').names },
  { path: 'std.kesplot', names: ['Figure', 'Style', 'version', 'backendReport'] },
  { path: 'std.ml', names: require('./ml').names },
  { path: 'std.core', names: ['Option', 'Tensor', 'DynTensor', 'Variable', 'Error', 'RuntimeError', 'Text', 'Equals', 'Comparable', 'Iterable', 'Iterator', 'Multipliable', 'RightMultipliable', 'Box'] },
  { path: 'std.linalg', names: ['dot', 'outer', 'norm', 'frobeniusNorm', 'spectralNorm', 'solve', 'solveLower', 'solveUpper', 'lu', 'qr', 'cholesky', 'svd', 'pinv', 'rank', 'det', 'inverse', 'eigh', 'leastSquares', 'LU', 'QR', 'SVD', 'Eigh'] },
  { path: 'std.math', names: ['PI', 'E', 'sqrt', 'abs', 'min', 'max', 'pow', 'floor', 'ceil', 'round', 'exp', 'expm1', 'log', 'log1p', 'log2', 'log10', 'sin', 'cos', 'tan', 'asin', 'acos', 'atan', 'sinh', 'cosh', 'tanh'] },
  { path: 'std.memory', names: ['Arena', 'ArenaRef'] },
  { path: 'std.prob', names: distributions.names },
  { path: 'std.process', names: ['args'] },
  { path: 'std.random', names: ['Rng', 'seed'] },
  { path: 'std.stats', names: ['mean', 'variance', 'stddev', 'covariance', 'correlation'] },
  { path: 'std.str', names: ['length', 'trim', 'split', 'contains', 'toUpper', 'toLower'] },
  { path: 'std.test', names: ['assertEq'] }
];

for (const generated of require('./library_metadata.json').modules) {
  const existing = modules.find(module => module.path === generated.path);
  if (existing) existing.names = [...new Set([...existing.names, ...generated.names])];
  else modules.push(generated);
}

const byPath = new Map(modules.map(module => [module.path, module]));

function names(path) {
  return byPath.get(path)?.names || [];
}

module.exports = { modules, names };
