'use strict';

const assert = require('assert');
const builtins = require('../out/builtins');

for (const name of [
  'print', 'parseInt', 'parseDouble', 'parseBigInt', 'box', 'Some', 'None',
  'open', 'create', 'readLine', 'assertEq', 'args', 'sqrt', 'pow', 'abs',
  'min', 'max', 'trim', 'toUpper', 'toLower', 'length', 'contains', 'split'
]) {
  assert(builtins.named(name).length > 0, `missing standard library entry: ${name}`);
}

assert.strictEqual(
  builtins.named('print')[0].signature,
  'print[T : Text](value : T) : void with Effects'
);
assert(builtins.methods().some(entry => entry.name === 'length'));
assert(builtins.methods().some(entry => entry.name === 'map'));
assert.strictEqual(builtins.named('None')[0].signature, 'None : Option[T]');

assert(builtins.named('generate').some(entry => entry.signature === 'Array.generate(size : Int, factory : Int -> T) : Array[T]'));
assert(!builtins.functions().some(entry => entry.name === 'generate'));
assert(builtins.functions().some(entry => entry.name === 'floor' && entry.module === 'std.math'));
assert(builtins.methods().some(entry => entry.name === 'toArray'));
assert(builtins.methods().some(entry => entry.name === 'append' && entry.receiver === 'Array[T]'));
assert(builtins.methods().some(entry => entry.name === 'prepend' && entry.receiver === 'Array[T]'));
assert(builtins.methods().some(entry => entry.name === 'pop' && entry.receiver === 'Stack[T]'));
assert(builtins.methods().some(entry => entry.name === 'first' && entry.receiver === 'Queue[T]'));
assert(builtins.methods().some(entry => entry.name === 'dequeue' && entry.receiver === 'Queue[T]'));
assert(builtins.methods().some(entry => entry.name === 'enqueue' && entry.receiver === 'Queue[T]'));
assert(builtins.methods().some(entry => entry.name === 'peek' && entry.receiver === 'MinHeap[T]'));
assert(builtins.methods().some(entry => entry.name === 'startsWith' && entry.receiver === 'String'));
assert(builtins.methods().some(entry => entry.name === 'any' && entry.receiver === 'Iterable[T]'));
assert(builtins.methods().some(entry => entry.name === 'isSome' && entry.receiver === 'Option[T]'));
assert(builtins.methods().some(entry => entry.name === 'join' && entry.receiver === 'Iterable[T]' && entry.signature.endsWith(': String')));
assert(builtins.methods().filter(entry => entry.name === 'index' && !entry.field).map(entry => entry.receiver).sort().join(',') === 'Array[T],List[T],String');
for (const name of ['fold', 'sum', 'product', 'enumerate', 'zip']) {
  assert(builtins.methods().some(entry => entry.name === name && entry.receiver === 'Iterable[T]'), `missing Iterable method: ${name}`);
}
assert(builtins.methods().some(entry => entry.name === 'sorted' && entry.receiver === 'List[T]' && entry.parameters.length === 1));
assert(builtins.methods().some(entry => entry.name === 'sort' && entry.receiver === 'Array[T]' && entry.parameters.length === 1));
assert(builtins.methods().some(entry => entry.name === 'get' && entry.receiver === 'Map[K, V]' && entry.signature.endsWith(': V')));
assert(builtins.methods().some(entry => entry.name === 'partition' && entry.signature.endsWith('(String, String, String)')));
for (const name of ['isalpha', 'isspace', 'capitalize', 'center', 'zfill', 'strip', 'expandtabs', 'find', 'replace', 'splitlines', 'removeprefix']) {
  assert(builtins.methods().some(entry => entry.name === name && entry.receiver === 'String'), `missing String method: ${name}`);
}

console.log('kestrel builtin metadata tests pass');

for (const name of ['Tensor', 'DynTensor']) assert(builtins.named(name).length > 0);
for (const name of ['zeros', 'filled', 'generate', 'onesLike', 'fromArray']) {
  assert(builtins.named(name).some(entry => entry.static === 'Tensor'), `missing Tensor.${name}`);
  assert(!builtins.functions().some(entry => entry.static === 'Tensor'));
}
for (const receiver of ['Tensor[double, shape]', 'DynTensor[double]']) {
  for (const name of ['copy', 'checkShape', 'broadcastTo']) {
    assert(builtins.methods().some(entry => entry.name === name && entry.receiver === receiver));
  }
}

for (const receiver of ['Tensor[double, shape]', 'DynTensor[double]']) {
  for (const name of ['sum', 'mean', 'min', 'max', 'map', 'reshape', 'rank', 'shape', 'count', 'debug']) {
    assert(builtins.methods().some(entry => entry.name === name && entry.receiver === receiver), `${receiver}.${name}`);
  }
}

for (const [name, count] of [['Tensor', 1], ['Tensor', 2], ['Array', 1], ['double', 1], ['String', 1]]) {
  const entry = builtins.named(name).find(entry => !entry.receiver && entry.parameters.length === count);
  assert(entry && entry.signature.includes('(') && entry.named, `${name}/${count} conversion metadata`);
}
for (const name of ['zeros', 'ones', 'filled', 'generate']) {
  assert(builtins.statics('Tensor').find(entry => entry.name === name).signature.includes('Array[Int]'), name);
}

for (const receiver of ['Tensor[double, shape]', 'DynTensor[double]']) {
  const divide = builtins.named('divide').find(entry => entry.receiver === receiver);
  assert(divide && divide.signature.endsWith(`: ${receiver}`));
  assert.deepStrictEqual(divide.parameters, ['other : double']);
}

assert(builtins.named('Complex').some(entry => entry.type));
assert(builtins.named('Complex').some(entry => entry.signature.startsWith('Complex(re')));
for (const name of ['abs', 'arg', 'conjugate', 'copy', 'im', 'negate', 're', 'text']) {
  assert(builtins.methods().some(entry => entry.name === name && entry.receiver === 'Complex'), `missing Complex.${name}`);
}

for (const name of ['Normal', 'Gamma', 'Binomial', 'Poisson', 'NegativeBinomial']) {
  assert(builtins.named(name).some(entry => entry.type), `missing distribution type ${name}`);
}
assert(builtins.named('create').some(entry => entry.static === 'Normal' && entry.signature.includes('sigma')));
assert(!builtins.named('Normal').some(entry => entry.signature.includes('stddev')));
assert(builtins.named('create').some(entry => entry.static === 'Binomial' && entry.signature.includes('trials : Int')));
assert(builtins.methods().some(entry => entry.name === 'pmf' && entry.receiver === 'Binomial'));

// std.kesdas metadata: types drive completion, and fields infer their own types.
for (const name of ['Series', 'DataFrame', 'Column', 'Scalar', 'Rolling', 'StringMethods', 'GroupBy', 'DataError']) {
  assert(builtins.named(name).some(entry => entry.type), `missing kesdas type ${name}`);
}
assert(builtins.named('DataFrame').some(entry => entry.named && entry.signature.includes('List[Series]')));
assert(builtins.named('Series').some(entry => entry.named && entry.signature.includes('List[Int]')));
for (const name of ['col', 'groupBy', 'sortValues', 'assign', 'merge', 'describe']) {
  assert(builtins.methods().some(entry => entry.receiver === 'DataFrame' && entry.name === name), `missing DataFrame.${name}`);
}
for (const name of ['mean', 'sum', 'str', 'rolling', 'describe', 'iloc']) {
  assert(builtins.methods().some(entry => entry.receiver === 'Series' && entry.name === name), `missing Series.${name}`);
}
assert(builtins.methods().some(entry => entry.receiver === 'DataFrame' && entry.name === 'names' && entry.field));
assert(builtins.methods().some(entry => entry.receiver === 'Series' && entry.name === 'data' && entry.field));
assert(builtins.statics('DataFrame').some(entry => entry.name === 'parseCsv'));
assert(builtins.named('readCsv').some(entry => !entry.receiver && !entry.static));
