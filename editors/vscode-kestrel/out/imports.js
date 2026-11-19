'use strict';

// The declarations a document imports from its own project, for the editor
// features that need more than the open file: signature help and parameter
// hints at an imported constructor, hover on an imported type, and member
// completion on its values. The compiler stays the authority; this module reads
// the same files it would, and keeps only what it can read safely.

const fs = require('fs');
const path = require('path');
const os = require('os');
const { createHash } = require('crypto');
const kestrel = require('./kestrel');
const modules = require('./modules');
const project = require('./project');

const PROJECT_FILE = project.PROJECT_FILE;
const projectCache = new Map();
const fileCache = new Map();

// The stamp of a regular file, or '' when it is missing. A cached answer stays
// valid until the file changes on disk.
function stamp(file) {
  try {
    const stat = fs.statSync(file);
    return stat.isFile() ? `${stat.mtimeMs}:${stat.size}` : '';
  } catch (error) {
    return '';
  }
}

// `kestrel.project` names the package and its sibling dependencies.
function readPackage(folder) {
  const file = path.join(folder, PROJECT_FILE);
  const current = stamp(file);
  if (!current) return null;
  const hit = projectCache.get(folder);
  if (hit && hit.stamp === current) return hit.data;
  let text = '';
  try {
    text = fs.readFileSync(file, 'utf8');
  } catch (error) {
    return null;
  }
  let name = '';
  const depends = [];
  const sources = {};
  for (const line of text.split(/\r?\n/)) {
    let quoted = false, stop = line.length;
    for (let i = 0; i < line.length; i += 1) {
      if (line[i] === '\\' && quoted) { i += 1; continue; }
      if (line[i] === '"') quoted = !quoted;
      if (!quoted && line.slice(i, i + 2) === '//') { stop = i; break; }
    }
    const clean = line.slice(0, stop).trim();
    if (!clean) continue;
    const declared = /^package\s+([a-z][a-z0-9_]*)/.exec(clean);
    if (declared) {
      name = declared[1];
      continue;
    }
    const git = /^depends\s+([a-z][a-z0-9_]*)\s+from\s+("(?:\\.|[^"\\])*")\s+at\s+("(?:\\.|[^"\\])*")\s*$/.exec(clean);
    if (git) {
      try { sources[git[1]] = { url: JSON.parse(git[2]), reference: JSON.parse(git[3]) }; depends.push(git[1]); } catch (_) { /* compiler reports malformed manifests */ }
      continue;
    }
    const dependency = /^depends\s+(.+)$/.exec(clean);
    if (!dependency) continue;
    for (const item of dependency[1].split(/[\s,]+/)) {
      if (/^[a-z][a-z0-9_]*$/.test(item) && !depends.includes(item)) depends.push(item);
    }
  }
  const data = { folder, name, depends, sources };
  projectCache.set(folder, { stamp: current, data });
  return data;
}

// The compiler tries every split of a dotted name between folders and a file
// stem, so `tests.domains` reads `tests/domains.kes` and `facts` reads `facts.kes`.
function candidates(folder, name) {
  const parts = name.split('.');
  for (let split = 0; split < parts.length; split += 1) {
    const file = path.join(folder, ...parts.slice(0, split), `${parts.slice(split).join('.')}.kes`);
    if (stamp(file)) return file;
  }
  return '';
}

function isStandard(name) {
  return name.startsWith('std.') || modules.modules.some(module => module.path === `std.${name}`);
}

function lockedRoot(root, name, source) {
  try {
    const lock = JSON.parse(fs.readFileSync(path.join(root, 'kestrel.lock'), 'utf8'));
    const entry = lock.version === 1 && lock.packages.find(entry => entry.name === name && entry.url === source.url && entry.reference === source.reference);
    if (!entry || !/^[0-9a-f]{40}(?:[0-9a-f]{24})?$/i.test(entry.commit)) return '';
    const cache = process.env.XDG_CACHE_HOME ? path.join(process.env.XDG_CACHE_HOME, 'kestrel/packages')
      : process.platform === 'darwin' ? path.join(os.homedir(), 'Library/Caches/kestrel/packages')
      : process.platform === 'win32' && process.env.LOCALAPPDATA ? path.join(process.env.LOCALAPPDATA, 'Kestrel/cache/packages')
      : path.join(os.homedir(), '.cache/kestrel/packages');
    return path.join(cache, name, createHash('sha256').update(entry.url).digest('hex'), entry.commit, 'source');
  } catch (_) { return ''; }
}

// The file a local import names, or '' when the import is standard or missing.
// A dependency's modules carry its name first and live beside the package.
function resolve(root, name) {
  if (!root || !name || isStandard(name)) return '';
  const declared = readPackage(root);
  const parts = name.split('.');
  if (declared && parts.length > 1 && declared.depends.includes(parts[0])) {
    const source = declared.sources[parts[0]];
    const folder = source ? lockedRoot(root, parts[0], source) : path.join(path.dirname(root), parts[0]);
    return folder ? candidates(folder, parts.slice(1).join('.')) : '';
  }
  return candidates(root, name);
}

function importedModel(file, visited = new Set(), memo = new Map()) {
  if (visited.has(file) || visited.size >= 96) return null;
  if (memo.has(file)) return memo.get(file);
  const current = stamp(file);
  if (!current) return null;
  let hit = fileCache.get(file);
  if (!hit || hit.stamp !== current) {
    let text;
    try { text = fs.readFileSync(file, 'utf8'); } catch (_) { return null; }
    hit = { stamp: current, text };
    fileCache.set(file, hit);
  }
  const result = modelFor(hit.text, { path: file, root: project.findRoot(path.dirname(file)) || path.dirname(file), visited, memo });
  memo.set(file, result);
  return result;
}

// `public` crosses packages; `package` stops at the package; the default keeps a
// declaration inside its module.
function visible(declaration, samePackage) {
  const visibility = declaration.visibility || 'private';
  return visibility !== 'private' && (samePackage || visibility === 'public');
}

// An imported declaration keeps its shape but loses this file's offsets, so
// features that map an offset leave it alone. The original name position and
// source file stay under `origin`, which is what go-to-definition navigates to.
function originOf(declaration) {
  if (declaration.origin) return declaration.origin;
  return {
    nameIndex: declaration.nameIndex,
    start: declaration.start,
    paramsOpen: declaration.paramsOpen,
    paramsClose: declaration.paramsClose,
    bodyStart: declaration.bodyStart,
    end: declaration.end
  };
}

function withoutOffsets(declaration, file) {
  return {
    ...declaration,
    imported: true,
    file: declaration.file || file || '',
    origin: originOf(declaration),
    nameIndex: -1,
    start: -1,
    paramsOpen: -1,
    paramsClose: -1,
    bodyStart: -1,
    end: -1
  };
}

// A field or enum case carries its own provenance, because its class can be
// reached through one file while the member was declared in that same file.
function withMemberOrigin(member, file) {
  return {
    ...member,
    file: member.file || file || '',
    origin: member.origin || { nameIndex: member.nameIndex, start: member.start },
    nameIndex: -1,
    start: -1
  };
}

function importedType(declaration, file) {
  const copied = withoutOffsets(declaration, file);
  return {
    ...copied,
    fields: (declaration.fields || []).map(field => withMemberOrigin(field, copied.file)),
    cases: (declaration.cases || []).map(item => withMemberOrigin(item, copied.file))
  };
}

function additionsFor(model, wanted, samePackage, file) {
  const additions = { functions: [], constructors: [], classes: [], aliases: [], constants: [], theorems: [] };
  const selectedTypes = new Set();
  for (const declaration of model.classes) {
    if (declaration.imported || !wanted.has(declaration.name) || !visible(declaration, samePackage)) continue;
    additions.classes.push(importedType(declaration, file));
    selectedTypes.add(declaration.name);
  }
  // Keep the type metadata used by inherited/default methods, even when only
  // the concrete class was imported. It is not an unqualified import.
  const pending = [...selectedTypes];
  while (pending.length) {
    const name = pending.shift();
    const type = model.classesByName.get(name);
    const references = [type?.parent, ...(type?.traits || []), ...(type?.fields || []).map(field => field.type),
      ...(model.functionsByOwner.get(name) || []).flatMap(fn => [fn.result, ...fn.parameters.map(parameter => parameter.type)])].filter(Boolean);
    for (const dependency of references.flatMap(written => written.match(/[A-Za-z_][\w.]*/g) || [])) {
      if (!dependency || selectedTypes.has(dependency) || !model.classesByName.has(dependency)) continue;
      selectedTypes.add(dependency); pending.push(dependency);
      additions.classes.push({ ...importedType(model.classesByName.get(dependency), file), internal: true });
    }
  }
  for (const theorem of model.theorems) {
    if (!theorem.imported && wanted.has(theorem.name) && visible(theorem, samePackage)) additions.theorems.push(withoutOffsets(theorem, file));
  }
  for (const alias of model.aliases) {
    if (alias.imported || !wanted.has(alias.name) || !visible(alias, samePackage)) continue;
    additions.aliases.push(withoutOffsets(alias, file));
  }
  for (const fn of model.functions) {
    const selected = fn.owner ? selectedTypes.has(fn.owner) : !fn.imported && wanted.has(fn.name);
    if (!selected) continue;
    // A trait's methods are its public face: they need no `public` mark, and
    // omitting one does not make the method private to the importing file.
    const traitOwner = fn.owner && model.typesByName.get(fn.owner)?.kind === 'trait';
    if (!traitOwner && !visible(fn, samePackage)) continue;
    additions.functions.push(withoutOffsets(fn, file));
  }
  for (const constructor of model.constructors) {
    const selected = !constructor.imported && wanted.has(constructor.owner || constructor.name);
    if (!selected) continue;
    additions.constructors.push(withoutOffsets(constructor, file));
  }
  for (const constant of model.constants) {
    if (constant.imported || !wanted.has(constant.name) || !visible(constant, samePackage)) continue;
    additions.constants.push(withoutOffsets(constant, file));
  }
  return additions;
}

function empty(additions) {
  return Object.values(additions).every(list => list.length === 0);
}

// The open document's model plus the project declarations its selective imports
// name. A namespace import (`import facts`) stays qualified, so it adds nothing
// unqualified; the completion still offers the alias itself.
function modelFor(text, options = {}) {
  const base = kestrel.parseDocument(text);
  if (!base.imports.length || !options.root) return base;
  const visited = new Set(options.visited || []);
  const memo = options.memo || new Map();
  if (options.path) visited.add(options.path);
  const additions = { functions: [], constructors: [], classes: [], aliases: [], constants: [], theorems: [] };
  const seen = new Set();
  for (const type of base.classes) seen.add(`type:${type.name}`);
  for (const alias of base.aliases) seen.add(`type:${alias.name}`);
  for (const fn of base.functions) seen.add(`function:${fn.owner || ''}:${fn.name}`);
  for (const constructor of base.constructors) seen.add(`constructor:${constructor.owner || ''}:${constructor.name}`);
  for (const constant of base.constants) seen.add(`constant:${constant.name}`);
  for (const theorem of base.theorems) seen.add(`theorem:${theorem.name}`);
  for (const entry of base.imports) {
    const file = resolve(options.root, entry.module);
    const imported = file ? importedModel(file, visited, memo) : null;
    if (!imported) continue;
    const importedRoot = project.findRoot(path.dirname(file)) || '';
    const samePackage = importedRoot === options.root;
    const wanted = entry.names.length ? entry.names : imported.classes.filter(type => !type.imported && visible(type, samePackage)).map(type => type.name);
    const found = additionsFor(imported, new Set(wanted), samePackage, file);
    if (!entry.names.length) {
      // Qualified trait metadata supports `has module.Trait` without making
      // the module's type names available as unqualified imports.
      const names = new Map(found.classes.map(type => [type.name, `${entry.alias}.${type.name}`]));
      const qualify = text => (text || '').replace(/\b[A-Za-z_][\w.]*/g, name => names.get(name) || name);
      found.classes = found.classes.map(type => ({ ...type, name: names.get(type.name), internal: true,
        parent: qualify(type.parent), traits: (type.traits || []).map(qualify),
        fields: type.fields.map(field => ({ ...field, type: qualify(field.type) })) }));
      found.functions = found.functions.map(fn => ({ ...fn, owner: names.get(fn.owner) || fn.owner,
        result: qualify(fn.result), parameters: fn.parameters.map(parameter => ({ ...parameter, type: qualify(parameter.type), label: qualify(parameter.label) })) }));
      found.constructors = []; found.constants = []; found.theorems = [];
    }
    for (const [kind, list] of Object.entries(found)) {
      for (const declaration of list) {
        const key = kind === 'functions'
          ? `function:${declaration.owner || ''}:${declaration.name}`
          : kind === 'constructors'
            ? `constructor:${declaration.owner || ''}:${declaration.name}`
            : kind === 'constants' || kind === 'theorems'
              ? `${kind === 'constants' ? 'constant' : 'theorem'}:${declaration.name}`
              : `type:${declaration.name}`;
        if (seen.has(key)) continue;
        seen.add(key);
        additions[kind].push(declaration);
      }
    }
  }
  if (empty(additions)) return base;
  return kestrel.withDeclarations(base, additions);
}

function exportedNames(root, module) {
  const file = resolve(root, module);
  const model = file && importedModel(file);
  if (!model) return [];
  const same = project.findRoot(path.dirname(file)) === root;
  return [...model.classes, ...model.aliases, ...model.constants, ...model.theorems, ...model.functions.filter(fn => !fn.owner)]
    .filter(declaration => !declaration.imported && !declaration.internal && visible(declaration, same)).map(declaration => declaration.name).filter(Boolean);
}
module.exports = { modelFor, resolve, readPackage, exportedNames };
