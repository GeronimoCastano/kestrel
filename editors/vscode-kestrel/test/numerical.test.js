'use strict';
const assert = require('assert');
const kestrel = require('../out/kestrel');
const infer = require('../out/infer');
const builtins = require('../out/builtins');
const matrix = 'let A := Tensor.ones((3, 2))\n';
for (const [source, expected] of [
  [matrix + 'let x := Tensor.ones((2,))\nlet y := A @ x', 'Tensor[double, (3,)]'],
  [matrix + 'let x := Tensor.ones((3,))\nlet y := x @ A', 'Tensor[double, (2,)]'],
  ['let x := Tensor.ones((2,))\nlet y := x @ x', ''],
  [matrix + 'let size := A.norm()', 'double'],
  ['let A := Tensor.ones((2, 2))\nlet B := A ** 2', 'Tensor[double, (2, 2)]'],
  ['let A := Tensor.ones((2, 2))\nlet B := A ** 2.0', 'Tensor[double, (2, 2)]'],
  ['let A := Tensor.ones((2, 2))\nlet B := A.power(2)', 'Tensor[double, (2, 2)]'],
  ['let A := Tensor.ones((2, 2))\nlet B := -A', 'Tensor[double, (2, 2)]'],
  ['let A := Tensor.ones((2, 2))\nlet B := A.negate()', 'Tensor[double, (2, 2)]'],
  ['let v := Tensor.ones((2,))\nlet B := v ** 2', 'Tensor[double, (2,)]'],
  ['let A := Tensor.ones((2, 3))\nlet B := A.T', 'Tensor[double, (3, 2)]'],
  ['let A := Tensor.ones((2, 3))\nlet B := A.T.T', 'Tensor[double, (2, 3)]'],
  ['let A := Tensor.ones((2, 3))\nlet B := A.T[0, 1]', 'double'],
  ['let A := Tensor.ones((2, 3))\nlet B := A + 1', 'Tensor[double, (2, 3)]'],
  ['let A := Tensor.ones((2, 3))\nlet B := A / 2', 'Tensor[double, (2, 3)]'],
  ['let A := Tensor.ones((2, 2))\nlet B := 2 * A', 'Tensor[double, (2, 2)]'],
  ['import std.math\nlet result := math.exp(1)', 'double'],
  ['let A := Tensor.ones((2, 3))\nlet B := A.T @ Tensor.ones((2,))', 'Tensor[double, (3,)]'],
  ['import std.linalg\n' + matrix + 'let result := linalg.pinv(A)', 'Tensor[double, (2, 3)]'],
  ['import std.linalg as la\n' + matrix + 'let result := la.rank(A)', 'Int'],
  ['import std.linalg.{outer}\nlet result := outer(Tensor.ones((2,)), Tensor.ones((3,)))', 'Tensor[double, (2, 3)]'],
  ['import std.linalg\n' + matrix + 'let factor := linalg.qr(A)\nlet Q := factor.q', 'Tensor[double, (3, 2)]'],
  ['import std.linalg\n' + matrix + 'let factor := linalg.svd(A)\nlet S := factor.s', 'Tensor[double, (2,)]'],
  ['import std.linalg\n' + matrix + 'let factor := linalg.svd(A)\nlet V := factor.vt', 'Tensor[double, (2, 2)]'],
  ['import std.linalg\nlet A := Tensor.ones((2, 2))\nlet factor := linalg.lu(A)\nlet x := factor.solve(Tensor.ones((2,)))', 'Tensor[double, (2,)]'],
  ['import std.linalg\n' + matrix + 'let result := linalg.inverse(A)', ''],
  ['import std.math as m\n' + matrix + 'let result := m.exp(A)', 'Tensor[double, (3, 2)]'],
  ['import std.math.{exp}\nlet result := exp(1.0)', 'double'],
  ['let result := exp(1.0)', ''],
  ['import std.stats\n' + matrix + 'let result := stats.variance(A, axis = 0)', 'Tensor[double, (2,)]'],
  ['import std.stats\n' + matrix + 'let result := stats.covariance(A)', 'Tensor[double, (2, 2)]'],
  ['import std.random.{Rng}\nlet result := Rng.seeded(42)', 'Rng'],
  ['import std.random\nlet result := random.Rng.seeded(42)', 'Rng'],
  ['import std.prob.{Normal}\nlet normal := Normal.create(0.0, 1.0)\nlet result := normal.pdf(0.0)', 'double'],
  ['import std.prob.{Normal}\nlet result := Normal.sample(mean = 0.0, sigma = 1.0)', 'double'],
  ['import std.prob.{Normal}\nlet result := Normal.sample(mean = 0.0, sigma = 1.0, shape = 5)', 'Tensor[double, (5,)]'],
  ['import std.prob.{Normal}\nlet result := Normal.sample(mean = 0.0, sigma = 1.0, shape = (2, 3))', 'Tensor[double, (2, 3)]'],
  ['import std.prob.{Normal}\nlet result := Normal.sample(mean = 0.0, sigma = 1.0, shape = Array([2, 3]))', 'DynTensor[double]'],
  ['import std.prob.{Normal}\nimport std.random.{Rng}\nlet rng := Rng.seeded(1)\nlet result := Normal.sample(rng, mean = 0.0, sigma = 1.0)', 'double'],
  ['import std.prob.{Binomial}\nlet result := Binomial.sample(trials = 10, p = 0.3)', 'Int'],
  ['import std.prob.{Binomial}\nlet result := Binomial.sample(trials = 10, p = 0.3, shape = 4)', 'Tensor[double, (4,)]'],
  ['import std.prob.{Normal}\nlet normal := Normal.create(mean = 0.0, sigma = 2.0)\nlet result := normal.sigma', 'double'],
  ['import std.prob as p\nlet result := p.Normal.sample(mean = 0.0, sigma = 1.0, shape = 2)', 'Tensor[double, (2,)]'],
  ['import std.complex.{Complex}\nlet z := Complex(3.0, 4.0)\nlet result := z * z', 'Complex'],
  ['import std.complex.{Complex}\nlet z := Complex(3.0, 4.0)\nlet result := z ** 2', 'Complex'],
  ['import std.complex.{Complex}\nlet z := Complex(3.0, 4.0)\nlet result := -z', 'Complex'],
  ['import std.complex.{Complex}\nlet z := Complex(3.0, 4.0)\nlet result := z.power(2)', 'Complex'],
  ['import std.ml.{StandardScaler}\nlet X : DynTensor[double] = Tensor.ones((2, 2))\nlet result := StandardScaler.fit(X)', 'FittedStandardScaler'],
  ['import std.ml.{StandardScaler}\nlet X := Tensor([0, 0, 1, 1], shape = (2, 2))\nlet scaler := StandardScaler.fit(X)\nlet result := scaler.means', 'DynTensor[double]'],
  ['import std.ml.{LinearRegression}\nlet X := Tensor([0, 0, 1, 1], shape = (2, 2))\nlet y := Tensor([1, 2])\nlet model := LinearRegression.fit(X, y)\nlet result := model.predict(X)', 'DynTensor[double]'],
  ['import std.ml.{StandardScaler}\nlet X : DynTensor[double] = Tensor.ones((2, 2))\nlet scaler := StandardScaler.fit(X)\nlet result := scaler.means', 'DynTensor[double]'],
  ['import std.ml.{LinearRegression}\nlet X : DynTensor[double] = Tensor.ones((2, 2))\nlet y : DynTensor[double] = Tensor.ones((2,))\nlet model := LinearRegression.fit(X, y)\nlet result := model.predict(X)', 'DynTensor[double]'],
  ['import std.ml.{accuracy}\nlet a : DynTensor[double] = Tensor.ones((2,))\nlet b : DynTensor[double] = Tensor.ones((2,))\nlet result := accuracy(a, b)', 'double'],
  ['import std.ml.{KNeighborsClassifier}\nlet X : DynTensor[double] = Tensor.ones((2, 2))\nlet y : DynTensor[double] = Tensor.ones((2,))\nlet m := KNeighborsClassifier.fit(X, y)\nlet result := m.predict(X)', 'DynTensor[double]'],
  ['import std.ml.{MinMaxScaler}\nlet X : DynTensor[double] = Tensor.ones((2, 2))\nlet m := MinMaxScaler.fit(X)\nlet result := m.minimums', 'DynTensor[double]'],
  ['import std.ml.{PCA}\nlet X : DynTensor[double] = Tensor.ones((2, 2))\nlet m := PCA.fit(X, 1)\nlet result := m.explainedVariance', 'DynTensor[double]'],
  ['import std.ml.{trainTestSplit}\nimport std.random.{Rng}\nlet X : DynTensor[double] = Tensor.ones((2, 2))\nlet y : DynTensor[double] = Tensor.ones((2,))\nlet rng := Rng.seeded(1)\nlet result := trainTestSplit(X, y, 1, rng)', '(DynTensor[double], DynTensor[double], DynTensor[double], DynTensor[double])'],
  ['import std.complex.{Complex}\nlet z := Complex(3.0, 4.0)\nlet result := z.abs()', 'double'],
  ['import std.complex.{Complex}\nlet z := Complex(3.0, 4.0)\nlet result := z.re', 'double'],
  ['import std.complex.{Complex}\nlet z := Complex(3.0, 4.0)\nlet result := 2.0 + z', 'Complex'],
  ['import std.complex.{Complex}\nlet result := Complex.i()', 'Complex'],
  ['import std.complex\nlet z := complex.Complex(3.0, 4.0)\nlet result := z.conjugate()', 'Complex'],
  ['import std.complex as c\nlet z := c.Complex(3.0, 4.0)\nlet result := 2.0 / z', 'Complex'],
  ['let z := Complex(3.0, 4.0)\nlet result := z.abs()', '']
]) {
  assert.strictEqual(infer.inferredBindings(source, kestrel.parseDocument(source)).at(-1).type, expected, source);
}
assert(!builtins.functions(kestrel.parseDocument('')).some(entry => entry.name === 'svd'));
assert(!builtins.functions(kestrel.parseDocument('import std.linalg')).some(entry => entry.name === 'svd'));
assert(builtins.functions(kestrel.parseDocument('import std.linalg.{svd}')).some(entry => entry.name === 'svd'));
console.log('kestrel numerical inference tests pass');
