'use strict';

// Probability distributions mirrored from src/probability.rs. The compiler is
// still the authority; this table drives editor completion and inference.
const list = [
  { name: 'Normal', kind: 0, params: [['mean', 'double'], ['sigma', 'double']], discrete: false },
  { name: 'Uniform', kind: 1, params: [['low', 'double'], ['high', 'double']], discrete: false },
  { name: 'Exponential', kind: 2, params: [['rate', 'double']], discrete: false },
  { name: 'Gamma', kind: 3, params: [['alpha', 'double'], ['beta', 'double']], discrete: false },
  { name: 'Beta', kind: 4, params: [['alpha', 'double'], ['beta', 'double']], discrete: false },
  { name: 'ChiSquared', kind: 5, params: [['dof', 'double']], discrete: false },
  { name: 'StudentT', kind: 6, params: [['dof', 'double']], discrete: false },
  { name: 'Cauchy', kind: 7, params: [['location', 'double'], ['scale', 'double']], discrete: false },
  { name: 'Laplace', kind: 8, params: [['location', 'double'], ['scale', 'double']], discrete: false },
  { name: 'Logistic', kind: 9, params: [['location', 'double'], ['scale', 'double']], discrete: false },
  { name: 'Weibull', kind: 10, params: [['k', 'double'], ['lambda', 'double']], discrete: false },
  { name: 'Lognormal', kind: 11, params: [['mu', 'double'], ['sigma', 'double']], discrete: false },
  { name: 'Pareto', kind: 12, params: [['scale', 'double'], ['alpha', 'double']], discrete: false },
  { name: 'Gumbel', kind: 13, params: [['location', 'double'], ['scale', 'double']], discrete: false },
  { name: 'Rayleigh', kind: 14, params: [['sigma', 'double']], discrete: false },
  { name: 'Triangular', kind: 15, params: [['low', 'double'], ['mode', 'double'], ['high', 'double']], discrete: false },
  { name: 'Bernoulli', kind: 16, params: [['p', 'double']], discrete: true },
  { name: 'Binomial', kind: 17, params: [['trials', 'Int'], ['p', 'double']], discrete: true },
  { name: 'Poisson', kind: 18, params: [['lambda', 'double']], discrete: true },
  { name: 'Geometric', kind: 19, params: [['p', 'double']], discrete: true },
  { name: 'NegativeBinomial', kind: 20, params: [['successes', 'Int'], ['p', 'double']], discrete: true },
  { name: 'Hypergeometric', kind: 21, params: [['population', 'Int'], ['successes', 'Int'], ['draws', 'Int']], discrete: true },
  { name: 'DiscreteUniform', kind: 22, params: [['low', 'Int'], ['high', 'Int']], discrete: true }
];

const names = list.map(distribution => distribution.name);
const byName = new Map(list.map(distribution => [distribution.name, distribution]));

function probabilityName(distribution) {
  return distribution.discrete ? 'pmf' : 'pdf';
}

function logProbabilityName(distribution) {
  return distribution.discrete ? 'logPmf' : 'logPdf';
}

function parameterList(distribution) {
  return distribution.params.map(([name, type]) => `${name} : ${type}`).join(', ');
}

module.exports = { list, names, byName, probabilityName, logProbabilityName, parameterList };
