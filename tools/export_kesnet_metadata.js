'use strict';

// Generates the editor's KesNet tables from the Kestrel sources, so completion,
// hover and result types cannot drift from the library. The parser reads only
// public declarations and signatures; the compiler remains the authority.
//
//   node tools/export_kesnet_metadata.js

const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const outDir = path.join(root, 'editors/vscode-kestrel/out');
const check = process.argv.includes('--check');
let stale = false;
function writeOrCheck(file, text) {
  if (check) {
    const current = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '';
    if (current !== text) { console.error(`${file} is stale; run node tools/export_kesnet_metadata.js`); stale = true; }
    return;
  }
  fs.writeFileSync(file, text);
}

function matching(text, open, openChar, closeChar) {
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    if (text[i] === openChar) depth += 1;
    else if (text[i] === closeChar) {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  return -1;
}

function signatureFrom(text, start) {
  // start points at the opening parenthesis of a parameter list.
  const close = matching(text, start, '(', ')');
  if (close < 0) return null;
  const parameters = splitTopLevel(text.slice(start + 1, close));
  let i = close + 1;
  while (i < text.length && /\s/.test(text[i])) i++;
  let result = '';
  if (text[i] === ':') {
    i++;
    let end = i;
    let depth = 0;
    while (end < text.length) {
      const c = text[end];
      if (depth === 0 && (c === '=' || c === '{' || c === '\n' || c === ';')) break;
      if ('([<'.includes(c)) depth++;
      else if (')]>'.includes(c)) depth--;
      end++;
    }
    result = text.slice(i, end).trim();
  }
  return { parameters: parameters.map(p => p.trim()).filter(Boolean), result };
}

function splitTopLevel(text) {
  const parts = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if ('([{'.includes(c)) depth++;
    else if (')]}'.includes(c)) depth--;
    else if (c === ',' && depth === 0) {
      parts.push(text.slice(start, i));
      start = i + 1;
    }
  }
  parts.push(text.slice(start));
  return parts;
}

function parseSource(source, moduleName) {
  const entries = [];
  const names = [];
  const types = [];
  const fieldTypes = new Map();
  const parents = {};
  const add = entry => { entries.push(entry); if (!names.includes(entry.name)) names.push(entry.name); };
  const typeEntry = name => {
    if (!types.includes(name)) types.push(name);
    add({ name, module: moduleName, gated: true, type: true, signature: name, parameters: [], documentation: `Imported ${name} from ${moduleName}.` });
  };
  const fieldEntry = (owner, name, type) => {
    add({ name, module: moduleName, gated: true, receiver: owner, field: true, signature: `${owner}.${name} : ${type}`, parameters: [], documentation: `Field ${name} of ${owner}.` });
    fieldTypes.set(`${owner}.${name}`, type);
  };
  const methodEntry = (owner, name, parameters, result, staticMethod) => {
    const signature = `${owner}.${name}(${parameters.join(', ')})${result ? ` : ${result}` : ''}`;
    add({ name, module: moduleName, gated: true, named: true, receiver: staticMethod ? undefined : owner, static: staticMethod ? owner : undefined, signature, parameters, documentation: `Member of ${owner}.` });
  };
  const constructorEntry = (owner, parameters, result) => {
    add({ name: owner, module: moduleName, gated: true, named: true, signature: `${owner}(${parameters.join(', ')}) : ${result || owner}`, parameters, documentation: `Constructs a ${owner}.` });
  };
  const functionEntry = (name, parameters, result) => {
    add({ name, module: moduleName, gated: true, named: true, signature: `${name}(${parameters.join(', ')}) : ${result || 'void'}`, parameters, documentation: `Free function from ${moduleName}.` });
  };

  let i = 0;
  while (i < source.length) {
    const classMatch = /^public class (\w+)/.exec(source.slice(i));
    const traitMatch = /^public trait (\w+)/.exec(source.slice(i));
    const freeMatch = /^public function (\w+)/.exec(source.slice(i));
    if (classMatch || traitMatch) {
      const name = (classMatch || traitMatch)[1];
      const brace = source.indexOf('{', i);
      const close = brace < 0 ? -1 : matching(source, brace, '{', '}');
      if (close < 0) break;
      if (classMatch) {
        typeEntry(name);
        const header = source.slice(i, source.indexOf('{', i));
        const parent = /\bextends\s+([A-Z][A-Za-z0-9_]*)/.exec(header);
        if (parent) parents[name] = parent[1];
      }
      const body = source.slice(brace + 1, close);
      // Fields.
      for (const line of body.split('\n')) {
        const field = /^\s*public (?:final|var) (\w+)\s*:\s*([^\n=]+)/.exec(line);
        if (field) fieldEntry(name, field[1], field[2].trim());
      }
      // Constructors and methods in source order.
      let cursor = 0;
      while (cursor < body.length) {
        const ctor = /\bnew\s*\(/.exec(body.slice(cursor));
        const method = /\b(?:public\s+)?(mut\s+)?(static\s+)?function\s+(\w+)\s*(?:\[[^\]]*\])?\s*\(/.exec(body.slice(cursor));
        const nextCtor = ctor ? cursor + ctor.index : Infinity;
        const nextMethod = method ? cursor + method.index : Infinity;
        if (nextCtor === Infinity && nextMethod === Infinity) break;
        if (nextCtor < nextMethod) {
          const open = body.indexOf('(', nextCtor);
          const sig = signatureFrom(body, open);
          if (sig) constructorEntry(name, sig.parameters, sig.result);
          cursor = open + 1;
        } else {
          const open = method.index + method[0].length - 1;
          const sig = signatureFrom(body, cursor + open);
          if (sig) {
            const staticMethod = Boolean(method[2]);
            // A function without an access modifier is package-private; still
            // useful for class metadata, but only public methods get entries.
            const publicMethod = /public/.test(body.slice(cursor + method.index, cursor + method.index + method[0].length));
            if (publicMethod) methodEntry(name, method[3], sig.parameters, sig.result, staticMethod);
          }
          cursor = cursor + open + 1;
        }
      }
      i = close + 1;
      continue;
    }
    if (freeMatch) {
      const open = source.indexOf('(', i);
      const sig = signatureFrom(source, open);
      if (sig) functionEntry(freeMatch[1], sig.parameters, sig.result);
      i = open + 1;
      continue;
    }
    i++;
  }
  return { entries, names, types, fieldTypes, parents };
}

function emitModule(sourceFile, moduleName, outFile) {
  const source = fs.readFileSync(path.join(root, sourceFile), 'utf8');
  const parsed = parseSource(source, moduleName);
  const body = `'use strict';\n\n// Generated from ${sourceFile} by tools/export_kesnet_metadata.js.\nconst entries = ${JSON.stringify(parsed.entries, null, 2)};\n\nconst names = ${JSON.stringify(parsed.names, null, 2)};\nconst types = ${JSON.stringify(parsed.types, null, 2)};\nconst fieldTypes = new Map();\nfor (const entry of entries) {\n  if (entry.field) fieldTypes.set(\`\${entry.receiver}.\${entry.name}\`, entry.signature.slice(entry.signature.lastIndexOf(':') + 1).trim());\n}\n\nconst parents = ${JSON.stringify(parsed.parents)};\n\nmodule.exports = { types, names, entries, fieldTypes, parents };\n`;
  writeOrCheck(path.join(outDir, outFile), body);
  return parsed;
}

// The built-in Variable type is native, so its table is kept here rather than
// parsed from a .kes file. Keep it in step with src/checker_variable.rs.
const VARIABLE_METHODS = [
  ['shape', [], 'Array[Int]'], ['rank', [], 'Int'], ['count', [], 'Int'],
  ['dtype', [], 'String'], ['device', [], 'String'], ['requiresGrad', [], 'Bool'],
  ['isLeaf', [], 'Bool'], ['grad', [], 'Option[DynTensor[double]]'], ['gradVariable', [], 'Variable'],
  ['data', [], 'DynTensor[double]'], ['toTensor', [], 'DynTensor[double]'], ['item', [], 'double'],
  ['copy', [], 'Variable'], ['clone', [], 'Variable'], ['detach', [], 'Variable'],
  ['text', [], 'String'], ['debug', [], 'String'], ['zeroGrad', [], 'void'],
  ['retainGrad', [], 'Variable'], ['backward', [], 'void'], ['backward', ['retainGraph : Bool'], 'void'],
  ['requiresGrad_', ['requiresGrad : Bool'], 'Variable'], ['to', ['dtype : String'], 'Variable'],
  ['negate', [], 'Variable'], ['abs', [], 'Variable'], ['exp', [], 'Variable'], ['expm1', [], 'Variable'],
  ['log', [], 'Variable'], ['log1p', [], 'Variable'], ['log2', [], 'Variable'], ['log10', [], 'Variable'],
  ['sqrt', [], 'Variable'], ['rsqrt', [], 'Variable'], ['sin', [], 'Variable'], ['cos', [], 'Variable'],
  ['tan', [], 'Variable'], ['asin', [], 'Variable'], ['acos', [], 'Variable'], ['atan', [], 'Variable'],
  ['sinh', [], 'Variable'], ['cosh', [], 'Variable'], ['tanh', [], 'Variable'], ['sigmoid', [], 'Variable'],
  ['relu', [], 'Variable'], ['silu', [], 'Variable'], ['gelu', [], 'Variable'],
  ['leakyRelu', ['slope : double'], 'Variable'], ['elu', ['alpha : double'], 'Variable'],
  ['softplus', ['beta : double', 'threshold : double'], 'Variable'],
  ['softmax', [], 'Variable'], ['softmax', ['axis : Int'], 'Variable'],
  ['logSoftmax', [], 'Variable'], ['logSoftmax', ['axis : Int'], 'Variable'],
  ['clamp', ['min : double', 'max : double'], 'Variable'],
  ['add', ['other : Variable'], 'Variable'], ['add', ['other : double'], 'Variable'],
  ['subtract', ['other : Variable'], 'Variable'], ['subtract', ['other : double'], 'Variable'],
  ['multiply', ['other : Variable'], 'Variable'], ['multiply', ['other : double'], 'Variable'],
  ['divide', ['other : Variable'], 'Variable'], ['divide', ['other : double'], 'Variable'],
  ['rightAdd', ['left : double'], 'Variable'], ['rightSubtract', ['left : double'], 'Variable'],
  ['rightMultiply', ['left : double'], 'Variable'], ['rightDivide', ['left : double'], 'Variable'],
  ['matmul', ['other : Variable'], 'Variable'], ['mm', ['other : Variable'], 'Variable'],
  ['bmm', ['other : Variable'], 'Variable'], ['dot', ['other : Variable'], 'double'],
  ['power', ['other : Variable'], 'Variable'], ['power', ['exponent : double'], 'Variable'],
  ['sum', [], 'Variable'], ['sum', ['axis : Int'], 'Variable'], ['sum', ['axis : Int', 'keepdim : Bool'], 'Variable'],
  ['mean', [], 'Variable'], ['mean', ['axis : Int'], 'Variable'], ['mean', ['axis : Int', 'keepdim : Bool'], 'Variable'],
  ['prod', [], 'Variable'], ['max', [], 'Variable'], ['max', ['axis : Int'], 'Variable'],
  ['min', [], 'Variable'], ['min', ['axis : Int'], 'Variable'],
  ['argmax', [], 'Variable'], ['argmax', ['axis : Int'], 'Variable'],
  ['argmin', [], 'Variable'], ['argmin', ['axis : Int'], 'Variable'], ['norm', [], 'Variable'],
  ['reshape', ['shape : (Int, ...) or Array[Int]'], 'Variable'],
  ['transpose', ['dim0 : Int', 'dim1 : Int'], 'Variable'],
  ['permute', ['axes : (Int, ...) or Array[Int]'], 'Variable'],
  ['squeeze', ['dim : Int'], 'Variable'], ['unsqueeze', ['dim : Int'], 'Variable'],
  ['flatten', [], 'Variable'], ['flatten', ['startDim : Int'], 'Variable'],
  ['broadcastTo', ['shape : (Int, ...) or Array[Int]'], 'Variable'],
  ['expand', ['shape : (Int, ...) or Array[Int]'], 'Variable'],
  ['cat', ['other : Variable', 'axis : Int'], 'Variable'],
  ['slice', ['axis : Int', 'start : Int', 'end : Int'], 'Variable'],
  ['conv2d', ['weight : Variable', 'bias : Variable', 'stride : Int', 'padding : Int', 'dilation : Int', 'groups : Int'], 'Variable'],
  ['maxPool2d', ['kernel : Int', 'stride : Int', 'padding : Int'], 'Variable'],
  ['avgPool2d', ['kernel : Int', 'stride : Int', 'padding : Int'], 'Variable'],
  ['adaptiveAvgPool2d', ['outHeight : Int', 'outWidth : Int'], 'Variable'],
  ['batchNorm', ['weight : Variable', 'bias : Variable', 'runningMean : Variable', 'runningVar : Variable', 'epsilon : double', 'momentum : double', 'training : Bool'], 'Variable'],
  ['layerNorm', ['weight : Variable', 'bias : Variable', 'epsilon : double'], 'Variable'],
  ['crossEntropy', ['target : Variable'], 'Variable'], ['nllLoss', ['target : Variable'], 'Variable'],
  ['binaryCrossEntropy', ['target : Variable'], 'Variable'],
  ['save', ['path : String'], 'void'], ['copyFromFile', ['path : String'], 'Variable'],
  ['add_', ['other : Variable'], 'Variable'], ['add_', ['other : double'], 'Variable'],
  ['sub_', ['other : Variable'], 'Variable'], ['sub_', ['other : double'], 'Variable'],
  ['mul_', ['other : Variable'], 'Variable'], ['mul_', ['other : double'], 'Variable'],
  ['div_', ['other : Variable'], 'Variable'], ['div_', ['other : double'], 'Variable'],
  ['fill_', ['value : double'], 'Variable'], ['zero_', [], 'Variable'], ['copy_', ['other : Variable'], 'Variable']
];

const VARIABLE_STATICS = [
  ['zeros', ['shape : (Int, ...) or Array[Int]'], 'Variable'],
  ['ones', ['shape : (Int, ...) or Array[Int]'], 'Variable'],
  ['filled', ['shape : (Int, ...) or Array[Int]', 'value : double'], 'Variable'],
  ['fromArray', ['values : Array[double]'], 'Variable'],
  ['fromArray', ['values : Array[double]', 'shape : (Int, ...) or Array[Int]'], 'Variable'],
  ['parameter', ['values : Array[double]'], 'Variable'],
  ['parameter', ['values : Array[double]', 'shape : (Int, ...) or Array[Int]'], 'Variable'],
  ['fromTensor', ['source : DynTensor[double]'], 'Variable'],
  ['scalar', ['value : double'], 'Variable'],
  ['empty', [], 'Variable'],
  ['uniform', ['shape : (Int, ...) or Array[Int]', 'low : double', 'high : double'], 'Variable'],
  ['normal', ['shape : (Int, ...) or Array[Int]', 'mean : double', 'sigma : double'], 'Variable'],
  ['bernoulli', ['shape : (Int, ...) or Array[Int]', 'p : double'], 'Variable'],
  ['randn', ['shape : (Int, ...) or Array[Int]'], 'Variable'],
  ['randint', ['low : Int', 'high : Int', 'shape : (Int, ...) or Array[Int]'], 'Variable'],
  ['rand', ['shape : (Int, ...) or Array[Int]'], 'Variable'],
  ['eye', ['n : Int'], 'Variable'],
  ['arange', ['start : Int', 'stop : Int'], 'Variable'],
  ['arange', ['start : Int', 'stop : Int', 'step : Int'], 'Variable'],
  ['linspace', ['start : Int', 'stop : Int', 'steps : Int'], 'Variable'],
  ['zerosLike', ['source : Variable'], 'Variable'],
  ['onesLike', ['source : Variable'], 'Variable'],
  ['randLike', ['source : Variable'], 'Variable'],
  ['embedding', ['weight : Variable', 'indices : Variable'], 'Variable'],
  ['load', ['path : String'], 'Variable'],
  ['seed', ['value : Int'], 'void'],
  ['setTraining', ['training : Bool'], 'void'], ['isTraining', [], 'Bool'],
  ['setGradEnabled', ['enabled : Bool'], 'void'], ['isGradEnabled', [], 'Bool'],
  ['liveNodes', [], 'Int'], ['liveBytes', [], 'Int']
];

function variableMetadata() {
  const entries = [{ name: 'Variable', module: 'std.core', type: true, signature: 'Variable', parameters: [], documentation: 'A counted differentiable tensor with reverse-mode gradients.' }];
  for (const [name, parameters, result] of VARIABLE_METHODS) {
    entries.push({ name, module: 'std.core', named: true, receiver: 'Variable', signature: `Variable.${name}(${parameters.join(', ')}) : ${result}`, parameters, documentation: `Variable.${name} is differentiable where the operation is.` });
  }
  for (const [name, parameters, result] of VARIABLE_STATICS) {
    entries.push({ name, module: 'std.core', named: true, static: 'Variable', signature: `Variable.${name}(${parameters.join(', ')}) : ${result}`, parameters, documentation: `Variable.${name} builds or inspects Variables.` });
  }
  return { entries, names: ['Variable'], types: ['Variable'] };
}

const kesnet = emitModule('src/kesnet.kes', 'std.kesnet', 'kesnet.js');
const optim = emitModule('src/kesnet_optim.kes', 'std.kesnet.optim', 'kesnet_optim.js');
const data = emitModule('src/kesnet_data.kes', 'std.kesnet.data', 'kesnet_data.js');
const variable = variableMetadata();
writeOrCheck(path.join(outDir, 'variable.js'), `'use strict';\n\n// Generated by tools/export_kesnet_metadata.js.\nconst entries = ${JSON.stringify(variable.entries, null, 2)};\nmodule.exports = { entries, names: ${JSON.stringify(variable.names)}, types: ${JSON.stringify(variable.types)} };\n`);

// A combined table for the Jupyter analyzer. Its loader merges methods,
// statics, fields, constructors and functions for every listed module.
function grouped(modules) {
  const methods = {}, statics = {}, constructors = {}, fields = {}, functions = [];
  const types = new Set();
  const names = new Set();
  const parents = {};
  for (const [module, parsed] of modules) {
    for (const name of parsed.names) names.add(name);
    for (const [name, parent] of Object.entries(parsed.parents || {})) parents[name] = parent;
    for (const entry of parsed.entries) {
      const copy = { ...entry };
      if (copy.receiver) {
        if (copy.field) (fields[copy.receiver] ||= {})[copy.name] = copy.signature.slice(copy.signature.lastIndexOf(':') + 1).trim();
        else (methods[copy.receiver] ||= []).push(copy);
      } else if (copy.static) (statics[copy.static] ||= []).push(copy);
      else if (copy.named) (constructors[copy.name] ||= []).push(copy);
      else if (!copy.type) functions.push(copy);
      if (copy.type) types.add(copy.name);
    }
  }
  return { types: [...types], names: [...names], methods, statics, constructors, fields, functions, parents };
}

const variableParsed = { entries: variable.entries, names: variable.names, types: variable.types };
const combined = grouped([
  ['std.kesnet', kesnet],
  ['std.kesnet.optim', optim],
  ['std.kesnet.data', data],
  ['std.core', variableParsed]
]);
combined.module = 'std.kesnet';
combined.modules = [
  { path: 'std.kesnet', names: kesnet.names },
  { path: 'std.kesnet.optim', names: optim.names },
  { path: 'std.kesnet.data', names: data.names }
];
writeOrCheck(path.join(root, 'editors/jupyter/kestrel_kernel/kesnet_metadata.json'), `${JSON.stringify(combined, null, 2)}\n`);
if (check) {
  if (stale) process.exit(1);
  console.log('KesNet editor and Jupyter metadata are up to date');
} else {
  console.log('Wrote kesnet editor and Jupyter metadata');
}
