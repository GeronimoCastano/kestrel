'use strict';

const vscode = require('vscode');
const kestrel = require('./kestrel');
const builtins = require('./builtins');
const modules = require('./modules');
const infer = require('./infer');
const parameters = require('./parameters');
const definition = require('./definition');
const values = require('./values');
const check = require('./check');
const project = require('./project');
const projectImports = require('./imports');
const { CheckDaemon } = require('./daemon');
const { resolveCompiler, compilerStamp } = require('./compiler');
const { startFormat } = require('./format');
const path = require('path');
const docMath = require('./documentation');

function markdownString(value) {
  const kind = vscode.window.activeColorTheme?.kind;
  const color = kind === 2 || kind === 3 ? '#e0e0e0' : '#333333';
  const editor = vscode.workspace.getConfiguration('editor', vscode.window.activeTextEditor?.document);
  const fontSize = editor.get('fontSize') || 14;
  const rendered = docMath.render(value, color, fontSize);
  const markdown = new vscode.MarkdownString(rendered);
  markdown.supportHtml = rendered !== value && rendered.includes('<img src="file:');
  return markdown;
}

const selector = { language: 'kestrel' };

// The model for editor features that may look past the open file: its own
// declarations plus the project declarations its selective imports name. A
// document without a path has no package to read, so it keeps its own model.
function documentModel(document, text) {
  if (!document.uri || !document.uri.fsPath) return kestrel.parseDocument(text);
  return projectImports.modelFor(text, { path: document.uri.fsPath, root: checkRoot(document) });
}

// The compiler resolves imports from a package root: the folder holding
// `kestrel.project`. The workspace folder is the root when it is a package, which
// matches `kestrel check --root` run from the folder the user opened. A workspace
// that is not a package itself is checked file by file from the nearest package
// above each document, and the workspace folder is the fallback when there is none.
function checkRoot(document) {
  const folder = vscode.workspace.getWorkspaceFolder(document.uri);
  const first = (vscode.workspace.workspaceFolders || [])[0];
  const fallback = folder
    ? folder.uri.fsPath
    : document.uri.fsPath
      ? path.dirname(document.uri.fsPath)
      : first
        ? first.uri.fsPath
        : process.cwd();
  return project.rootFor(document.uri.fsPath, fallback);
}

/// Backslash names that the parser also accepts directly. The editor can turn
/// them into the symbols the goal view displays.
const SYMBOLS = {
  in: '∈',
  mem: '∈',
  subseteq: '⊆',
  cup: '∪',
  cap: '∩',
  compl: 'ᶜ',
  forall: '∀',
  exists: '∃',
  to: '→',
  larr: '←',
  leftarrow: '←',
  ot: '←',
  iff: '↔',
  land: '∧',
  lor: '∨',
  not: '¬',
  neg: '¬',
  notin: '∉',
  ge: '≥',
  le: '≤',
  ne: '≠'
};

function expandSymbols(text) {
  return text.replace(/\\([A-Za-z]+)(?=\s|$)/g, (whole, name) => SYMBOLS[name] || whole);
}

function symbolEdit(document, change, setting) {
  if (!setting || change.text !== ' ' || !change.range.isEmpty) return undefined;
  const line = document.lineAt(change.range.start.line).text;
  const before = line.slice(0, change.range.start.character);
  const match = /\\([A-Za-z]+)$/.exec(before);
  if (!match) return undefined;
  const symbol = SYMBOLS[match[1]];
  if (!symbol) return undefined;
  const start = new vscode.Position(change.range.start.line, match.index);
  return { range: new vscode.Range(start, change.range.start), symbol };
}

function signatureOf(callable) {
  return kestrel.signature(callable);
}

// The signature block first, then the declaration's Markdown documentation.
function hoverText(lines, documentation) {
  const code = '```kestrel\n' + lines.join('\n') + '\n```';
  return documentation ? `${code}\n\n${documentation}` : code;
}

// One section per documentation block. A hover that names two declarations
// with different documentation shows each under its own signature instead of
// keeping only the first one it met.
function hoverSections(sections) {
  return sections.map(section => hoverText(section.lines, section.documentation)).join('\n\n');
}

function callablesNamed(model, name) {
  return [...(model.functionsByName.get(name) || []).filter(fn => !fn.owner), ...(model.constructorsByName.get(name) || [])];
}

function builtinsFor(context, name, type, model) {
  const entries = builtins.named(name);
  if (context?.method) {
    const module = builtins.moduleForReceiver(model, context.receiverExpression || context.receiver);
    if (module && !type) return entries.filter(entry => entry.module === module && !entry.receiver && !entry.static);
    const statics = entries.filter(entry => entry.static === context.receiver && builtins.staticAvailable(entry, model, context.receiverExpression || context.receiver));
    if (statics.length) return statics;
    if (type) return entries.filter(entry => entry.receiver && receiverMatches(type, entry.receiver));
    return entries.filter(entry => entry.receiver && !entry.gated);
  }
  return entries.filter(entry => !entry.receiver && !entry.static && builtins.available(entry, model));
}

function isConstantEntry(entry) {
  return !entry.receiver && !entry.signature.includes('(');
}

function memberReceiver(text, offset) {
  const match = /([A-Za-z_][A-Za-z0-9_]*)\s*\.\s*[A-Za-z0-9_]*$/.exec(text.slice(0, offset));
  return match ? match[1] : '';
}

function baseType(type) {
  const match = /^(?:&|StoredView\[|Box\[)?(?:dyn\s+)?(?:[A-Za-z_][A-Za-z0-9_]*\.)*([A-Za-z_][A-Za-z0-9_]*)/.exec(type || '');
  return match ? match[1] : '';
}

function receiverTypeName(text, offset, model, receiver, known) {
  if (!receiver) return '';
  if (receiver === 'this') {
    const type = kestrel.enclosingClass(model, offset);
    return type ? type.name : '';
  }
  const local = (known || infer.localTypes(text, offset, model)).get(receiver);
  return local ? local.type : '';
}

// The type of the expression before the member dot, so chains such as `[1, 2].map(f).` have a type.
// Falls back to a plain name and to `this` when the expression has no type of its own. `known`
// lets a caller that already inferred the file reuse the answer.
function receiverTypeAt(text, offset, model, known) {
  const expression = infer.memberReceiverExpression(text, offset);
  const type = expression ? infer.typeAt(text, offset, model, expression, known) : '';
  return type || receiverTypeName(text, offset, model, memberReceiver(text, offset), known);
}

function receiverType(text, offset, model) {
  return receiverTypeAt(text, offset, model);
}

function receiverMatches(actual, formal) {
  // Never offer the success branch's members on `T or Error`; the user must write `?`.
  if (/\bor\s+Error\b/.test(actual || '')) return false;
  const actualBase = baseType(actual);
  const formalBase = baseType(formal);
  if (!actualBase || !formalBase) return false;
  if (actualBase === formalBase) return true;
  if (formalBase === 'Iterable' && ['List', 'Set', 'SortedSet', 'MutSet', 'Array', 'Map', 'MutMap', 'Lazy'].includes(actualBase)) {
    return true;
  }
  if (formalBase === 'Text' && ['String', 'Int', 'Nat', 'BigInt', 'double', 'Bool', 'List', 'Set', 'SortedSet', 'Map', 'Option'].includes(actualBase)) {
    return true;
  }
  if (formalBase === 'Comparable' && ['Int', 'Nat', 'BigInt', 'double', 'String'].includes(actualBase)) {
    return true;
  }
  return false;
}

const importKinds = {
  Module: vscode.CompletionItemKind.Module,
  Class: vscode.CompletionItemKind.Class,
  Function: vscode.CompletionItemKind.Function,
  Constant: vscode.CompletionItemKind.Constant
};

function importItem(document, context, label, kind, detail, insertText) {
  const item = new vscode.CompletionItem(label, kind);
  item.detail = detail;
  item.insertText = insertText;
  item.filterText = label;
  item.sortText = `0_${label}`;
  item.range = new vscode.Range(document.positionAt(context.start), document.positionAt(context.end));
  return item;
}

// Module paths and selective-import names are separate from ordinary expression
// completion, so the user sees only importable names while editing an import.
function importCompletionItems(document, context) {
  if (context.kind === 'none') return [];
  if (context.kind === 'root') {
    const items = modules.modules
      .map(module => module.path.slice(4))
      .filter(name => !context.prefix || name.startsWith(context.prefix))
      .map(name => importItem(document, context, name, importKinds.Module, `std.${name}`, name));
    if ('std'.startsWith(context.prefix)) {
      items.push(importItem(
        document,
        context,
        'std',
        importKinds.Module,
        'Kestrel standard library',
        context.leadingSpace ? ' std' : 'std'
      ));
    }
    return items;
  }
  if (context.kind === 'module') {
    const items = modules.modules
      .map(module => module.path.slice(4))
      .filter(name => name.startsWith(context.prefix))
      .map(name => importItem(
        document,
        context,
        name,
        importKinds.Module,
        `std.${name}`,
        name
      ));
    if ('std'.startsWith(context.prefix)) {
      items.push(importItem(document, context, 'std', importKinds.Module, 'Kestrel standard library', 'std'));
    }
    return items;
  }
  if (context.kind === 'nested') {
    const canonicalBase = context.canonicalBase
      || (context.base.startsWith('std.') ? context.base : `std.${context.base}`);
    const base = `${canonicalBase}.`;
    const items = [...new Set(modules.modules
      .map(module => module.path)
      .filter(path => path.startsWith(base))
      .map(path => path.slice(base.length).split('.')[0]))]
      .filter(name => name.startsWith(context.prefix))
      .map(name => importItem(
        document,
        context,
        name,
        importKinds.Module,
        `${canonicalBase}.${name}`,
        `${context.base}.${name}`
      ));
    // A parent module can also export names. `import std.kesnet.` offers both
    // `optim`/`data` and `{Linear}`.
    for (const name of modules.names(canonicalBase).filter(name => name.startsWith(context.prefix))) {
      if (items.some(item => item.label === name)) continue;
      items.push(importItem(
        document,
        context,
        name,
        /^[A-Z]/.test(name) ? importKinds.Class : importKinds.Function,
        `Import ${name} from ${context.base}`,
        `{${name}}`
      ));
    }
    return items;
  }
  if (context.kind === 'members') {
    return [...new Set([...modules.names(context.module), ...projectImports.exportedNames(document.uri?.fsPath ? checkRoot(document) : '', context.module)])]
      .filter(name => name.startsWith(context.prefix))
      .map(name => {
        const kind = name === 'PI' || name === 'E'
          ? importKinds.Constant
          : /^[A-Z]/.test(name) ? importKinds.Class : importKinds.Function;
        return importItem(
          document,
          context,
          name,
          kind,
          `Import ${name} from ${context.written || context.module}`,
          `{${name}}`
        );
      });
  }
  return [...new Set([...modules.names(context.module), ...projectImports.exportedNames(document.uri?.fsPath ? checkRoot(document) : '', context.module)])]
    .filter(name => !context.selected.has(name) && name.startsWith(context.prefix))
    .map(name => {
      const kind = name === 'PI' || name === 'E'
        ? importKinds.Constant
        : /^[A-Z]/.test(name) ? importKinds.Class : importKinds.Function;
      return importItem(
        document,
        context,
        name,
        kind,
        `Exported by ${context.module}`,
        name
      );
    });
}

// A notebook runs its cells as one session, so the language tooling has to read
// the code cells above the edited one: their names, imports and types stay
// defined. The prefix is the text of those cells, each ending on its own line.
function notebookPrefix(document) {
  if (!document.uri) return '';
  const notebooks = vscode.workspace.notebookDocuments || [];
  const uri = document.uri.toString();
  const notebook = notebooks.find(candidate =>
    candidate.getCells().some(cell => cell.document.uri.toString() === uri));
  if (!notebook) return '';
  const cells = notebook.getCells();
  const index = cells.findIndex(cell => cell.document.uri.toString() === uri);
  if (index < 1) return '';
  return cells.slice(0, index)
    .map(cell => cell.document)
    .filter(doc => doc.languageId === 'kestrel')
    .map(doc => doc.getText().replace(/\s*$/, '') + '\n')
    .filter(text => text.trim())
    .join('');
}

// The text and cursor a provider should analyze. `shift` is how far the cell's
// own text moved into the session text, so positions can be mapped back.
function sessionText(document, position) {
  const prefix = notebookPrefix(document);
  return {
    text: prefix + document.getText(),
    offset: prefix.length + document.offsetAt(position),
    shift: prefix.length
  };
}

function propertyCompletionItems(document, text, offset, model, localTypes, shift = 0) {
  const items = [];
  const seen = new Set();
  const add = (key, item) => {
    if (seen.has(key)) return;
    seen.add(key);
    items.push(item);
  };
  const receiver = memberReceiver(text, offset);
  const receiverExpression = infer.memberReceiverExpression(text, offset);
  const tail = /\.[ \t]*([A-Za-z0-9_]*)$/.exec(text.slice(0, offset));
  const prefix = tail ? tail[1] : '';
  const namespace = builtins.moduleForReceiver(model, receiverExpression);
  if (namespace && !localTypes.has(receiverExpression)) {
    for (const entry of builtins.moduleEntries(namespace).filter(entry => !entry.receiver && !entry.static)) {
      const item = new vscode.CompletionItem(entry.name, entry.type ? vscode.CompletionItemKind.Class : vscode.CompletionItemKind.Function);
      item.detail = entry.signature; item.insertText = entry.name;
      item.documentation = markdownString(entry.documentation || '');
      add(`namespace:${entry.name}`, item);
    }
    return items;
  }
  const type = receiverTypeAt(text, offset, model, localTypes);
  const base = baseType(type);
  // `T or Error` has no members until `?` unwraps it. Offer the one token that
  // turns the chain into a valid one, instead of members the compiler rejects.
  if (/\bor\s+Error\b/.test(type)) {
    if (prefix !== '' || !tail) return items;
    const dot = offset - tail[0].length + tail[0].indexOf('.');
    const item = new vscode.CompletionItem('?', vscode.CompletionItemKind.Operator || vscode.CompletionItemKind.Keyword);
    item.detail = `Unwrap ${base || 'the Error branch'}; then choose a member`;
    item.documentation = markdownString('`T or Error` exposes `T` members only after `?`.');
    item.insertText = '?.';
    item.filterText = '?';
    item.sortText = '0_?';
    item.range = new vscode.Range(document.positionAt(dot - shift), document.positionAt(offset - shift));
    item.command = { command: 'editor.action.triggerSuggest', title: 'Show members' };
    items.push(item);
    return items;
  }
  // A bare type name such as DataFrame names a namespace for static members, not a value.
  // Its instance methods and fields belong after a value, not after the type itself.
  const typeName = /^[A-Z][A-Za-z0-9_]*$/.test(receiver)
    && !localTypes.has(receiver)
    && (builtins.named(receiver).some(entry => entry.type)
      || model.classesByName.has(receiver));
  const enclosing = kestrel.enclosingClass(model, offset);
  const sameClass = Boolean(enclosing && enclosing.name === base);
  const includePrivate = receiver === 'this' || sameClass;
  // After a bare type name the receiver itself is the class being named; after
  // a value, the inferred base type is.
  const classInfo = model.classesByName.get(typeName ? receiver : base);
  if (!typeName) {
    for (const view of kestrel.typeViews(model, type)) {
      const info = view.declaration;
      for (const field of info.fields.filter(field => !field.static && (includePrivate || field.visibility !== 'private'))) {
        const item = new vscode.CompletionItem(field.name, vscode.CompletionItemKind.Field);
        item.detail = field.type.replace(/\b[A-Za-z_]\w*\b/g, word => view.bindings[word] || word);
        item.insertText = field.name;
        item.sortText = `0_${field.name}`;
        add(`field:${info.name}:${field.name}`, item);
      }
    }
  }
  // A trait's methods are the public face of a trait object, so they need no `public` mark.
  const trait = model.typesByName.get(base)?.kind === 'trait';
  if (!typeName) for (const fn of kestrel.memberFunctions(model, type)) {
    if (fn.static || !(includePrivate || trait || fn.visibility !== 'private')) continue;
    const item = new vscode.CompletionItem(fn.name, vscode.CompletionItemKind.Method);
    item.detail = signatureOf(fn); item.insertText = fn.name; item.sortText = `0_${fn.name}`;
    if (fn.documentation) item.documentation = markdownString(fn.documentation);
    add(`local-method:${fn.name}`, item);
  }
  // `Shape.` names the type, so its enum cases and static members belong here.
  if (classInfo && typeName) {
    for (const variant of classInfo.cases) {
      const item = new vscode.CompletionItem(variant.name, vscode.CompletionItemKind.EnumMember);
      item.detail = classInfo.name;
      item.insertText = variant.name;
      item.sortText = `0_${variant.name}`;
      add(`case:${classInfo.name}:${variant.name}`, item);
    }
    for (const field of classInfo.fields.filter(field => field.static)) {
      const item = new vscode.CompletionItem(field.name, vscode.CompletionItemKind.Field);
      item.detail = `static ${field.type}`;
      item.insertText = field.name;
      item.sortText = `0_${field.name}`;
      add(`static-field:${classInfo.name}:${field.name}`, item);
    }
    for (const fn of model.functionsByOwner.get(classInfo.name) || []) {
      if (!fn.static) continue;
      const item = new vscode.CompletionItem(fn.name, vscode.CompletionItemKind.Method);
      item.detail = signatureOf(fn);
      item.insertText = fn.name;
      item.sortText = `0_${fn.name}`;
      add(`static-method:${fn.name}`, item);
    }
  }
  // `Tensor.` names a type rather than a value, so its static members are offered here.
  for (const entry of builtins.statics(receiver).filter(entry => builtins.staticAvailable(entry, model, receiverExpression))) {
    const item = new vscode.CompletionItem(entry.name, vscode.CompletionItemKind.Function);
    item.detail = entry.signature;
    item.documentation = markdownString(
      `${entry.documentation || ''}\n\n${entry.module || 'Kestrel standard library'}`
    );
    item.insertText = entry.name;
    item.sortText = `0_${entry.name}`;
    add(`static:${receiver}:${entry.name}`, item);
  }
  if (type && !typeName && !/\bor\s+Error\b/.test(type)) {
    const chain = infer.typeChain(type, model);
    for (const entry of builtins.methods()) {
      if (!chain.some(owner => receiverMatches(owner, entry.receiver))) continue;
      const item = new vscode.CompletionItem(entry.name, entry.field ? vscode.CompletionItemKind.Field : vscode.CompletionItemKind.Method);
      item.detail = entry.signature;
      item.documentation = markdownString(
        `${entry.documentation || ''}\n\n${entry.module || 'Kestrel standard library'}`
      );
      item.insertText = entry.name;
      item.sortText = `0_${entry.name}`;
      add(`method:${entry.receiver}:${entry.name}`, item);
    }
  }
  return items;
}

class KestrelCompletionProvider {
  provideCompletionItems(document, position) {
    // An import is a statement of this cell and its edit lands in the cell, so the
    // context stays cell-relative even when the cell has earlier cells above it.
    const importContext = kestrel.importContext(document.getText(), document.offsetAt(position));
    if (importContext) return importCompletionItems(document, importContext);
    const session = sessionText(document, position);
    const text = session.text;
    const offset = session.offset;
    const model = documentModel(document, text);
    if (/\.[ \t]*[A-Za-z0-9_]*$/.test(text.slice(0, offset))) {
      return propertyCompletionItems(
        document, text, offset, model, infer.localTypes(text, offset, model), session.shift
      );
    }
    const context = kestrel.callContext(text, offset);
    const items = [];
    const seen = new Set();
    const labels = new Set();
    const add = (key, item) => {
      if (seen.has(key)) return;
      seen.add(key);
      labels.add(item.label);
      items.push(item);
    };

    // Inside a proof script, offer the tactics with their shapes. The
    // proof-state view supplies the live types; these snippets supply the syntax.
    // A proof block holds facts even without a `by` script: an explicit term
    // proof cites theorems by name too.
    const inProof = kestrel.proofScriptAt(text, offset);
    const proofArgument = Boolean(context && callablesNamed(model, context.callee).some(fn => fn.parameters[context.argumentIndex]?.proof));
    if (inProof && text.lastIndexOf('by', offset) >= 0 && !context) {
      for (const tactic of kestrel.TACTICS) {
        const item = new vscode.CompletionItem(tactic.name, vscode.CompletionItemKind.Keyword);
        item.detail = tactic.summary;
        item.documentation = tacticDocumentation(tactic);
        item.insertText = new vscode.SnippetString(tactic.insert);
        item.sortText = `0_tactic_${tactic.name}`;
        add(`tactic:${tactic.name}`, item);
      }
    }

    if (context && context.declarationParams) {
      const parameter = new vscode.CompletionItem('parameter', vscode.CompletionItemKind.Snippet);
      parameter.detail = 'name : Type';
      parameter.documentation = markdownString('Add a typed parameter, for example `name : String`.');
      parameter.insertText = new vscode.SnippetString('${1:name} : ${2:String}');
      parameter.sortText = '0_parameter';
      add('parameter', parameter);
    }

    if (context) {
      const used = kestrel.parametersInUse(context.argumentsText);
      for (const callable of callablesNamed(model, context.callee)) {
        for (const parameter of callable.parameters) {
          if (!parameter.name || used.has(parameter.name)) continue;
          const item = new vscode.CompletionItem(parameter.name, vscode.CompletionItemKind.Property);
          item.detail = parameter.type || 'parameter';
          item.documentation = markdownString(
            `Named argument for **${callable.name}**\n\n\`${parameter.label}\``
          );
          item.insertText = new vscode.SnippetString(`${parameter.name} = $0`);
          item.filterText = parameter.name;
          item.sortText = `0_${parameter.name}`;
          add(`argument:${parameter.name}`, item);
        }
      }
    }

    if (context && !context.declarationParams) {
      const type = context.method ? receiverTypeAt(text, context.open, model) : '';
      const selected = builtinsFor({ ...context, receiverExpression: infer.memberReceiverExpression(text, context.open) }, context.callee, type, model);
      const used = kestrel.parametersInUse(context.argumentsText);
      for (const entry of selected.filter(entry => entry.gated)) for (const label of entry.parameters) {
        const name = parameterName(label);
        if (!name || used.has(name)) continue;
        const item = new vscode.CompletionItem(name, vscode.CompletionItemKind.Property);
        item.detail = label; item.insertText = new vscode.SnippetString(`${name} = $0`); item.sortText = `0_${name}`;
        add(`argument:${name}`, item);
      }
    }

    if (context && !context.declarationParams && ['sum', 'mean', 'min', 'max'].includes(context.callee)) {
      const receiver = receiverTypeAt(text, context.open, model);
      if (['Tensor', 'DynTensor'].includes(baseType(receiver)) && !kestrel.parametersInUse(context.argumentsText).has('axis')) {
        const item = new vscode.CompletionItem('axis', vscode.CompletionItemKind.Property);
        item.detail = 'axis : Int';
        item.insertText = new vscode.SnippetString('axis: $0');
        item.sortText = '0_axis';
        add('argument:axis', item);
      }
    }

    const localTypes = infer.localTypes(text, offset, model);
    for (const name of new Set([...kestrel.collectLocals(text, offset, model), ...localTypes.keys()])) {
      const item = new vscode.CompletionItem(name, vscode.CompletionItemKind.Variable);
      const known = localTypes.get(name);
      if (known) item.detail = known.type;
      item.sortText = `1_${name}`;
      add(`local:${name}`, item);
    }

    // Proof constructors are terms, not tactics, so they sit with the locals
    // and insert a call with its arguments.
    if (inProof) {
      for (const constructor of kestrel.CONSTRUCTORS) {
        const item = new vscode.CompletionItem(constructor.name, vscode.CompletionItemKind.Constructor);
        item.detail = constructor.summary;
        item.documentation = markdownString(constructor.body);
        item.insertText = new vscode.SnippetString(constructor.insert);
        item.sortText = `1_constructor_${constructor.name}`;
        add(`constructor:${constructor.name}`, item);
      }
    }

    // Proof scripts cite earlier facts. Executable evidence positions can also
    // cite checked theorems, without offering tactic-only proof constructors.
    if (inProof || proofArgument) {
      for (const theorem of model.theorems) {
        if (!theorem.name || inProof && theorem.start > offset || !inProof && theorem.kind === 'axiom') continue;
        const item = new vscode.CompletionItem(theorem.name, vscode.CompletionItemKind.Function);
        item.detail = signatureOf(theorem);
        item.documentation = markdownString(
          theorem.kind === 'axiom'
            ? 'An axiom of this file, accepted without proof.'
            : 'A theorem declared in this file.'
        );
        if (theorem.documentation) item.documentation = markdownString(theorem.documentation);
        item.insertText = theorem.name;
        item.sortText = `1_${theorem.name}`;
        add(`theorem:${theorem.name}`, item);
      }
    }

    for (const fn of model.functions.filter(fn => !fn.owner)) {
      const item = new vscode.CompletionItem(fn.name, vscode.CompletionItemKind.Function);
      item.detail = signatureOf(fn);
      if (fn.documentation) item.documentation = markdownString(fn.documentation);
      item.insertText = fn.name;
      item.sortText = `2_${fn.name}`;
      add(`function:${fn.name}`, item);
    }

    for (const constant of model.constants) {
      const item = new vscode.CompletionItem(constant.name, vscode.CompletionItemKind.Constant);
      item.detail = constant.type ? `const ${constant.name} : ${constant.type}` : `const ${constant.name}`;
      if (constant.documentation) item.documentation = markdownString(constant.documentation);
      item.insertText = constant.name;
      item.sortText = `2_${constant.name}`;
      add(`constant:${constant.name}`, item);
    }

    for (const type of model.types.filter(type => !type.internal)) {
      let kind = vscode.CompletionItemKind.Class;
      if (type.kind === 'trait') kind = vscode.CompletionItemKind.Interface;
      else if (type.kind === 'enum') kind = vscode.CompletionItemKind.Enum;
      else if (type.kind === 'alias') kind = vscode.CompletionItemKind.TypeParameter;
      else if (type.kind === 'record') kind = vscode.CompletionItemKind.Struct;
      const item = new vscode.CompletionItem(type.name, kind);
      item.detail = type.kind;
      if (type.documentation) item.documentation = markdownString(type.documentation);
      item.insertText = type.name;
      item.sortText = `2_${type.name}`;
      add(`type:${type.name}`, item);
    }

    for (const imported of model.imports || []) {
      if (!imported.alias) continue;
      const item = new vscode.CompletionItem(imported.alias, vscode.CompletionItemKind.Variable);
      item.detail = imported.module; item.insertText = imported.alias;
      add(`namespace:${imported.alias}`, item);
    }

    for (const entry of builtins.functions(model)) {
      if (labels.has(entry.name)) continue;
      const kind = entry.type
        ? vscode.CompletionItemKind.Class
        : isConstantEntry(entry)
          ? vscode.CompletionItemKind.Constant
          : vscode.CompletionItemKind.Function;
      const item = new vscode.CompletionItem(entry.name, kind);
      item.detail = entry.signature;
      item.documentation = markdownString(
        `${entry.documentation || ''}\n\n${entry.module || 'Kestrel standard library'}`
      );
      item.insertText = entry.name;
      item.sortText = `2_${entry.name}`;
      add(`builtin:${entry.name}`, item);
    }
    for (const keyword of kestrel.KEYWORDS) {
      const item = new vscode.CompletionItem(keyword, vscode.CompletionItemKind.Keyword);
      item.sortText = `3_${keyword}`;
      add(`keyword:${keyword}`, item);
    }

    for (const type of kestrel.TYPES) {
      const item = new vscode.CompletionItem(type, vscode.CompletionItemKind.TypeParameter);
      item.sortText = `3_${type}`;
      add(`type:${type}`, item);
    }

    return items;
  }
}

class KestrelSignatureProvider {
  provideSignatureHelp(document, position) {
    const session = sessionText(document, position);
    const text = session.text;
    const offset = session.offset;
    const model = documentModel(document, text);
    const context = kestrel.callContext(text, offset);
    if (!context || context.declarationParams) return null;
    const constructor = kestrel.proofConstructor(context.callee);
    if (constructor && kestrel.proofScriptAt(text, offset)) {
      const help = new vscode.SignatureHelp();
      const information = new vscode.SignatureInformation(constructor.signature);
      information.parameters = constructor.parameters.map(
        parameter => new vscode.ParameterInformation(parameter)
      );
      information.documentation = markdownString(constructor.body);
      help.signatures = [information];
      help.activeSignature = 0;
      help.activeParameter = Math.min(
        context.argumentIndex,
        Math.max(0, constructor.parameters.length - 1)
      );
      return help;
    }
    const type = context.method
      ? receiverTypeAt(text, context.open, model)
      : '';
    const base = baseType(type);
    let callables = context.method && type ? kestrel.memberFunctions(model, type).filter(fn => fn.name === context.callee) : callablesNamed(model, context.callee);
    if (kestrel.proofScriptAt(text, offset) || !callables.length) {
      callables = [...callables, ...(model.theoremsByName.get(context.callee) || [])];
    }
    if (context.method && base) {
      const enclosing = kestrel.enclosingClass(model, offset);
      const includePrivate = context.receiver === 'this' || Boolean(enclosing && enclosing.name === base);
      const owned = callables.filter(callable => callable.owner === base && (includePrivate || callable.visibility !== 'private'));
      if (owned.length > 0) callables = owned;
    }
    context.receiverExpression = infer.memberReceiverExpression(text, context.open);
    const builtinEntries = builtinsFor(context, context.callee, type, model)
      .filter(entry => entry.signature.includes("("));
    if (callables.length === 0 && builtinEntries.length === 0) return null;

    const help = new vscode.SignatureHelp();
    if (callables.length > 0) {
      help.signatures = callables.map(callable => {
        const information = new vscode.SignatureInformation(signatureOf(callable));
        information.parameters = callable.parameters.map(parameter => {
          const detail = parameter.type ? `\`${parameter.label}\`` : parameter.label;
          return new vscode.ParameterInformation(parameter.label, markdownString(detail));
        });
        const kind = callable.kind === 'constructor'
          ? (callable.record ? 'Record constructor' : 'Constructor')
          : callable.kind === 'theorem' || callable.kind === 'axiom'
            ? 'Kestrel theorem'
            : 'Kestrel function';
        information.documentation = markdownString(
          callable.documentation ? `${kind}\n\n${callable.documentation}` : kind
        );
        return information;
      });
    } else {
      help.signatures = builtinEntries.map(entry => {
        const information = new vscode.SignatureInformation(entry.signature);
        information.parameters = entry.parameters.map(label => new vscode.ParameterInformation(label));
        information.documentation = markdownString(
          `${entry.documentation || ''}\n\n${entry.module || 'Kestrel standard library'}`
        );
        return information;
      });
    }
    help.activeSignature = 0;
    if (callables.length === 0 && builtinEntries.length > 0) {
      // Pick the overload with the arity being typed, so a subset call shows subset.
      const arity = context.argumentsText.trim() ? context.argumentIndex + 1 : 0;
      const candidate = builtinEntries.findIndex(entry => entry.parameters.length === arity);
      if (candidate >= 0) help.activeSignature = candidate;
    }
    if (callables.length === 0 && context.argumentsText.trim() && ['sum', 'mean', 'min', 'max'].includes(context.callee)) {
      const axis = builtinEntries.findIndex(entry => entry.parameters.some(parameter => parameter.startsWith('axis :')));
      if (axis >= 0) help.activeSignature = axis;
    }
    if (callables.length === 0 && ["Tensor", "Array"].includes(context.callee)) {
      const arity = context.argumentIndex + 1;
      const candidate = builtinEntries.findIndex(entry => entry.parameters.length === arity);
      if (candidate >= 0) help.activeSignature = candidate;
    }
    const parameters = callables.length > 0 ? callables[0].parameters : builtinEntries[help.activeSignature].parameters;
    help.activeParameter = parameters.length === 0
      ? 0
      : Math.min(context.argumentIndex, parameters.length - 1);
    return help;
  }
}

function tacticDocumentation(tactic) {
  const parts = [
    `**${tactic.name}**: ${tactic.summary}`,
    '',
    '```kestrel',
    tactic.signature,
    '```',
    '',
    tactic.body
  ];
  if (tactic.example) parts.push('', '```kestrel', tactic.example, '```');
  if (tactic.aliases.length > 0) {
    parts.push('', `Also written ${tactic.aliases.map(alias => `\`${alias}\``).join(', ')}.`);
  }
  return markdownString(parts.join('\n'));
}

class KestrelHoverProvider {
  provideHover(document, position) {
    const comment = docMath.atComment(document.getText(), document.offsetAt(position));
    if (comment) return new vscode.Hover(markdownString(comment));
    const range = document.getWordRangeAtPosition(position, /[A-Za-z_][A-Za-z0-9_]*/);
    if (!range) return null;
    const word = document.getText(range);
    const session = sessionText(document, position);
    const text = session.text;
    const offset = session.offset;
    const start = session.shift + document.offsetAt(range.start);
    const tactic = kestrel.tactic(word);
    const inProof = kestrel.proofScriptAt(text, start);
    if (tactic && kestrel.tacticPosition(text, start) && inProof) {
      return new vscode.Hover(tacticDocumentation(tactic), range);
    }
    const model = documentModel(document, text);
    // A declaration's documentation belongs under its own signature. Keeping
    // one documentation slot for the whole hover let a documented constructor
    // replace the documentation of the class it belongs to, so the class's
    // Markdown, including its equations, never reached the hover.
    const sections = [];
    const add = (signature, documentation = '') => {
      const last = sections[sections.length - 1];
      if (!last || (documentation && last.documentation && documentation !== last.documentation)) {
        sections.push({ lines: [signature], documentation });
        return;
      }
      last.lines.push(signature);
      if (documentation && !last.documentation) last.documentation = documentation;
    };
    const local = infer.localTypes(text, offset, model).get(word);
    if (local) {
      add(`${local.kind === 'parameter' ? 'parameter ' : ''}${word} : ${local.type}`);
    }
    const memberType = /\.\s*$/.test(document.getText().slice(0, document.offsetAt(range.start))) ? receiverType(text, offset, model) : '';
    // The type keeps its place above the constructor: a class name that builds
    // a value names the type first, and the `new` signature follows it.
    for (const type of model.types.filter(type => type.name === word)) {
      add(`${type.kind} ${type.name}`, type.documentation);
    }
    const hoveredCallables = memberType ? kestrel.memberFunctions(model, memberType).filter(fn => fn.name === word) : [...callablesNamed(model, word), ...(model.functionsByOwner.get(kestrel.enclosingClass(model, offset)?.name) || []).filter(fn => fn.name === word)];
    for (const callable of hoveredCallables) {
      add(signatureOf(callable), callable.documentation);
    }
    for (const theorem of model.theoremsByName.get(word) || []) {
      add(signatureOf(theorem), theorem.documentation);
    }
    for (const constant of model.constants.filter(constant => constant.name === word)) {
      add(constant.type ? `const ${constant.name} : ${constant.type}` : `const ${constant.name}`, constant.documentation);
    }
    if (sections.length === 0) {
      const before = document.getText().slice(0, document.offsetAt(range.start));
      const member = /\.\s*$/.test(before);
      const type = member ? receiverType(text, offset, model) : '';
      if (member && type) {
        for (const view of kestrel.typeViews(model, type)) {
          const field = view.declaration.fields.find(field => field.name === word);
          if (!field) continue;
          const fieldType = field.type.replace(/\b[A-Za-z_]\w*\b/g, word => view.bindings[word] || word);
          add(`field ${word} : ${fieldType}`);
          break;
        }
      }
      if (sections.length > 0) {
        return new vscode.Hover(markdownString(hoverSections(sections)), range);
      }
      const selected = builtinsFor({ method: member, receiver: memberReceiver(text, offset), receiverExpression: infer.memberReceiverExpression(text, offset) }, word, type, model);
      // Builtin metadata holds one entry per receiver and overload, all sharing
      // a name. One block with the first entry's documentation keeps that list
      // readable.
      if (selected.length === 0) return null;
      const lines = selected.map(entry => entry.signature);
      const documentation = selected.find(entry => entry.documentation)?.documentation || '';
      return new vscode.Hover(markdownString(hoverText(lines, documentation)), range);
    }
    return new vscode.Hover(markdownString(hoverSections(sections)), range);
  }
}

// Alt/Option-click and Go to Definition use the same declaration model as hover.
// A local target is a position in this document; an imported target is a real
// file, opened through the editor so positions follow that file's text.
class KestrelDefinitionProvider {
  async provideDefinition(document, position) {
    const session = sessionText(document, position);
    const model = documentModel(document, session.text);
    const found = definition.definitionAt(session.text, session.offset, model);
    if (!found.length) return null;
    const locations = [];
    for (const target of found) {
      if (!target.file) {
        const start = target.offset - session.shift;
        if (start < 0 || start + target.length > document.getText().length) continue;
        locations.push(new vscode.Location(
          document.uri,
          new vscode.Range(document.positionAt(start), document.positionAt(start + target.length))
        ));
        continue;
      }
      try {
        const opened = await vscode.workspace.openTextDocument(vscode.Uri.file(target.file));
        locations.push(new vscode.Location(
          opened.uri,
          new vscode.Range(
            opened.positionAt(target.offset),
            opened.positionAt(target.offset + target.length)
          )
        ));
      } catch (error) {
        // A dependency that vanished between the model read and the click is
        // not a navigation target.
      }
    }
    return locations.length ? locations : null;
  }
}

const CONTROL_WORDS = new Set([
  'if', 'for', 'while', 'match', 'switch', 'scope', 'using', 'unsafe',
  'spawn', 'function', 'new', 'return', 'class', 'trait', 'enum', 'alias',
  'const', 'theorem', 'proof'
]);

// One pass records the matching closer of every opening bracket. Parameter
// hints used to rescan the rest of the file for every call, which made a large
// file quadratic.
function bracketMap(clean) {
  const closing = new Int32Array(clean.length);
  closing.fill(-1);
  const stack = [];
  for (let i = 0; i < clean.length; i += 1) {
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
  return closing;
}

function closingParen(clean, open, closing) {
  if (closing) return open >= 0 && open < closing.length ? closing[open] : -1;
  let depth = 0;
  for (let i = open; i < clean.length; i += 1) {
    if (clean[i] === '(') depth += 1;
    else if (clean[i] === ')') {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  return -1;
}

function argumentRanges(text, open, close) {
  const ranges = [];
  let start = open + 1;
  let round = 0;
  let square = 0;
  let curly = 0;
  let i = open + 1;
  while (i < close) {
    const current = text[i];
    if (current === '"') {
      i += 1;
      while (i < close) {
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
      if (text.slice(start, i).trim() !== '') ranges.push({ start, end: i });
      start = i + 1;
    }
    i += 1;
  }
  if (text.slice(start, close).trim() !== '') ranges.push({ start, end: close });
  return ranges;
}

function parameterName(label) {
  const colon = label.indexOf(':');
  if (colon < 0) return '';
  return label.slice(0, colon).trim();
}

function parameterNames(parameters) {
  return parameters.map(parameter => typeof parameter === 'string'
    ? parameterName(parameter)
    : parameter.name || '').filter(Boolean);
}

// Overloads share one name, so the hints come from the overload whose parameter count matches the call.
// When none matches exactly, the first overload with room for the call is used.
function overloadFor(candidates, count) {
  const exact = candidates.find(item => (item.parameters || []).length === count);
  if (exact) return exact;
  const roomy = candidates.find(item => (item.parameters || []).length > count);
  return roomy || candidates[0];
}

// Editor settings, read on each use so a change applies without a reload.
function setting(key, fallback) {
  const value = vscode.workspace.getConfiguration('kestrel').get(key);
  return value === undefined || value === null ? fallback : value;
}

// One toggle behind two title-bar entries. Each entry carries the icon for the
// state it is in, so the button shows what the panel is doing instead of
// looking like a blind switch.
async function toggleProofStateMath() {
  const next = setting('proofState.math', true) === false;
  await vscode.workspace
    .getConfiguration('kestrel')
    .update('proofState.math', next, vscode.ConfigurationTarget.Global);
  syncMathButton();
  vscode.window.setStatusBarMessage(`Kestrel proof state: ${next ? 'typeset' : 'printed'}`, 2500);
  return next;
}

function syncMathButton() {
  vscode.commands.executeCommand?.('setContext', 'kestrel.proofStateMath', setting('proofState.math', true) !== false);
}

function inlaySettings() {
  return {
    parameterNames: setting('inlayHints.parameterNames', 'always'),
    parameterStyle: setting('inlayHints.parameterNameStyle', 'equals'),
    skipMatchingNames: setting('inlayHints.skipMatchingNames', true),
    minimumArguments: setting('inlayHints.minimumArguments', 1),
    types: setting('inlayHints.types', true)
  };
}

function parameterHints(document, text, model, start, end, options) {
  const clean = kestrel.sanitize(text);
  const hints = [];
  if (options.parameterNames === 'off') return hints;
  const closing = bracketMap(clean);
  // One inference pass supplies every binding type. Parameters are added per
  // enclosing function, so a hint in one function cannot see another function's
  // parameter with the same name.
  const bindingTypes = new Map();
  for (const binding of infer.inferredBindings(text, model, end)) {
    if (binding.type) bindingTypes.set(binding.name, binding.type);
  }
  const scopes = new Map();
  const scopeFor = open => {
    const fn = infer.enclosingFunction(model, open);
    const key = fn ? fn.start : -1;
    let scope = scopes.get(key);
    if (!scope) {
      scope = infer.scopeAt(model, fn ? fn.start : end, bindingTypes);
      scopes.set(key, scope);
    }
    return scope;
  };
  const identifier = /[A-Za-z_][A-Za-z0-9_]*/g;
  // A call whose callee starts just above the visible range still has arguments
  // in it; a bounded look-back catches those without scanning from the top of a
  // large file.
  identifier.lastIndex = Math.max(0, start - 2000);
  let match;
  while ((match = identifier.exec(clean)) !== null) {
    const name = match[0];
    const wordStart = match.index;
    if (wordStart >= end) break;
    if (CONTROL_WORDS.has(name)) continue;
    let open = wordStart + name.length;
    while (open < clean.length && /\s/.test(clean[open])) open += 1;
    if (clean[open] !== '(') continue;
    const beforeWord = clean.slice(Math.max(0, wordStart - 24), wordStart);
    if (/\b(function|new)\s+$/.test(beforeWord)) continue;
    const close = closingParen(clean, open, closing);
    if (close < 0) continue;

    let cursor = wordStart - 1;
    while (cursor >= 0 && /\s/.test(clean[cursor])) cursor -= 1;
    const method = cursor >= 0 && clean[cursor] === '.';
    let receiver = '';
    let receiverExpression = '';
    if (method) {
      let endOfReceiver = cursor;
      while (endOfReceiver > 0 && /\s/.test(clean[endOfReceiver - 1])) endOfReceiver -= 1;
      let startOfReceiver = endOfReceiver;
      while (startOfReceiver > 0 && /[A-Za-z0-9_]/.test(clean[startOfReceiver - 1])) startOfReceiver -= 1;
      receiver = clean.slice(startOfReceiver, endOfReceiver);
      receiverExpression = infer.memberReceiverExpression(clean, wordStart) || receiver;
    }
    let type = '';
    if (method) {
      const scope = scopeFor(open);
      const known = scope.get(receiver);
      if (known && receiverExpression === receiver) {
        type = known;
      } else if (receiverExpression === 'this') {
        type = scope.get('this') || kestrel.enclosingClass(model, open)?.name || '';
      } else {
        type = infer.infer(receiverExpression, model, scope);
      }
    }
    const ranges = argumentRanges(text, open, close);
    const count = ranges.length;
    let parameters = [];
    if (method) {
      const base = baseType(type);
      const local = overloadFor(model.functionsByOwnerName.get(`${base}\u0000${name}`) || [], count);
      if (local) parameters = local.parameters.map(parameter => parameter.name).filter(Boolean);
      if (parameters.length === 0) {
        const entry = overloadFor(
          builtins.named(name).filter(item => item.receiver && receiverMatches(type, item.receiver)),
          count
        );
        if (entry) parameters = parameterNames(entry.parameters || []);
      }
      if (parameters.length === 0) {
        // `Tensor.ones(...)` reaches a static method through a type name, not a value.
        const entry = overloadFor(
          builtins.named(name).filter(item => item.static && item.static === receiver),
          count
        );
        if (entry) parameters = parameterNames(entry.parameters || []);
      }
    } else {
      const fn = overloadFor((model.topFunctionsByName.get(name) || []), count);
      const constructor = (model.constructorsByName.get(name) || [])[0];
      const entry = overloadFor(builtins.named(name).filter(item => !item.receiver), count);
      if (constructor) parameters = constructor.parameters.map(parameter => parameter.name).filter(Boolean);
      else if (fn) parameters = fn.parameters.map(parameter => parameter.name).filter(Boolean);
      else if (entry) parameters = parameterNames(entry.parameters || []);
    }
    if (parameters.length === 0) continue;
    if (ranges.length < options.minimumArguments) continue;

    for (let index = 0; index < ranges.length && index < parameters.length; index += 1) {
      const range = ranges[index];
      const argument = text.slice(range.start, range.end);
      if (/^\s*[a-zA-Z_][A-Za-z0-9_]*\s*=(?!=)/.test(argument)) continue;
      // An argument range begins right after its comma or parenthesis, so a call split across
      // lines starts with the line break. The hint belongs before the argument's first token.
      const first = range.start + (argument.length - argument.trimStart().length);
      if (first > range.end) continue;
      if (first < start || first > end) continue;
      if (options.skipMatchingNames && argument.trim() === parameters[index]) continue;
      const position = document.positionAt(first);
      if (options.parameterNames === 'cursorLine' && position.line !== options.cursorLine) continue;
      const label = options.parameterStyle === 'colon' ? `${parameters[index]}:` : `${parameters[index]} =`;
      const hint = new vscode.InlayHint(
        position,
        label,
        vscode.InlayHintKind.Parameter
      );
      hint.paddingRight = true;
      hints.push(hint);
    }
  }
  return hints;
}

class KestrelInlayHintsProvider {
  constructor() {
    this.changed = new vscode.EventEmitter();
    this.onDidChangeInlayHints = this.changed.event;
  }

  provideInlayHints(document, range) {
    const text = document.getText();
    const model = documentModel(document, text);
    const start = document.offsetAt(range.start);
    const end = document.offsetAt(range.end);
    const hints = [];
    const options = inlaySettings();
    const active = vscode.window.activeTextEditor;
    options.cursorLine = active && active.document === document ? active.selection.active.line : undefined;
    const parameterNameHints = parameterHints(document, text, model, start, end, options);
    for (const binding of options.types ? infer.inferredBindings(text, model, end) : []) {
      if (!binding.type || binding.annotation) continue;
      if (binding.nameEnd < start || binding.nameEnd > end) continue;
      const hint = new vscode.InlayHint(
        document.positionAt(binding.nameEnd),
        `: ${binding.type}`,
        vscode.InlayHintKind.Type
      );
      hint.paddingLeft = true;
      hints.push(hint);
    }
    hints.push(...parameterNameHints);
    return hints;
  }
}

class KestrelSymbolProvider {
  provideDocumentSymbols(document) {
    const model = kestrel.parseDocument(document.getText());
    const symbols = [];
    const range = (start, end) => new vscode.Range(
      document.positionAt(start),
      document.positionAt(Math.max(start, end))
    );
    for (const type of model.classes) {
      const kind = type.kind === 'trait'
        ? vscode.SymbolKind.Interface
        : type.kind === 'enum' ? vscode.SymbolKind.Enum : vscode.SymbolKind.Class;
      symbols.push(new vscode.DocumentSymbol(
        type.name,
        type.kind,
        kind,
        range(type.start, type.end),
        range(type.nameIndex, type.nameIndex + type.name.length)
      ));
    }
    for (const fn of model.functions) {
      symbols.push(new vscode.DocumentSymbol(
        fn.name,
        fn.result || '',
        vscode.SymbolKind.Function,
        range(fn.start, fn.end),
        range(fn.nameIndex, fn.nameIndex + fn.name.length)
      ));
    }
    for (const constant of model.constants) {
      symbols.push(new vscode.DocumentSymbol(
        constant.name,
        constant.type || '',
        vscode.SymbolKind.Constant,
        range(constant.start, constant.start + constant.name.length),
        range(constant.nameIndex, constant.nameIndex + constant.name.length)
      ));
    }
    return symbols;
  }
}

// Parameters keep their parameter color; uppercase values override the grammar's
// type-name guess with the same scope used for ordinary variables.
const semanticLegend = new vscode.SemanticTokensLegend(['parameter', 'variable'], []);

class KestrelSemanticTokensProvider {
  provideDocumentSemanticTokens(document) {
    const text = document.getText();
    const builder = new vscode.SemanticTokensBuilder(semanticLegend);
    const model = kestrel.parseDocument(text);
    const tokens = new Map(values.valueUses(text, model).map(token => [token.start, { ...token, type: 1 }]));
    // Parameters retain their existing color, including when their names are uppercase.
    for (const token of parameters.parameterUses(text, model)) tokens.set(token.start, { ...token, type: 0 });
    for (const token of [...tokens.values()].sort((a, b) => a.start - b.start)) {
      const start = document.positionAt(token.start);
      builder.push(start.line, start.character, token.length, token.type, 0);
    }
    return builder.build();
  }
}

// The proof-state view follows the cursor. The checker answers with the goals
// that are live at the current offset, so a `by { ... }` block behaves like a
// Lean infoview: each tactic updates the context and the target underneath it.
const PROOF_PAUSE_MS = 80;

function escapeHtml(value) {
  return String(value == null ? '' : value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

// Colour the logical connectives without parsing the proposition again. The
// checker already prints them as Unicode, so the view only wraps symbols.
function highlightProposition(value) {
  return escapeHtml(value)
    .replace(/(∀|∃|→|↔|∧|∨|¬|⊢|∈|∉|⊆|⊂|∪|∩|ᶜ|ℝ|ℂ|ℕ|ℤ|ℚ)/g, '<span class="symbol">$1</span>')
    .replace(/\b([A-Za-z_][A-Za-z0-9_]*)\(/g, '<span class="call">$1</span>(')
    .replace(/\b(True|False)\b/g, '<span class="literal">$1</span>');
}

// The TeX renderer loads on first use: it carries the maths fonts, and a
// window that never opens the proof state should not pay for them.
let proofMath;
function mathRenderer() {
  if (proofMath === undefined) {
    try {
      proofMath = require('./proof-math');
    } catch (error) {
      proofMath = null;
    }
  }
  return proofMath;
}

function proofStateHtml(state, settings) {
  const options = settings || {};
  const typeset = Boolean(options.math) && Boolean(mathRenderer());

  // A typeset proposition is the same text drawn by TeX. Anything the renderer
  // cannot draw keeps the printed form, so a mapping gap never hides a goal.
  const show = value => {
    if (typeset) {
      const svg = mathRenderer().inlineSvg(String(value == null ? '' : value), options.symbols);
      if (svg) return svg;
    }
    return highlightProposition(value);
  };

  const title = state && state.theorem ? escapeHtml(state.theorem) : 'Kestrel';
  const goals = (state && state.goals) || [];
  const cases = (state && state.cases) || [];
  const complete = Boolean(state && state.complete);
  const admitted = Boolean(state && state.admitted);
  const trusted = Boolean(state && state.trusted);
  const error = state && state.error;
  const count = goals.length;
  const badge = complete
    ? (admitted
      ? '<span class="badge admitted">sorry</span>'
      : trusted ? '<span class="badge admitted">trusted SMT</span>' : '<span class="badge complete">proved</span>')
    : `<span class="badge">${count === 1 ? '1 goal' : `${count} goals`}</span>`;
  let body = '';
  if (state && state.disabled) {
    body = `<div class="empty">
      <div class="empty-title">Proof state is off</div>
      <div class="hint">Enable <code>kestrel.proofState.enabled</code> to follow the cursor through a theorem.</div>
    </div>`;
  } else if (!state || !state.theorem) {
    body = `<div class="empty">
      <div class="empty-title">No theorem here</div>
      <div class="hint">Put the cursor inside a theorem with a <code>by { ... }</code> proof.</div>
    </div>`;
  } else {
    if (state && state.stale) body += `<div class="stale-note">Showing the last proof state. The current text has a parse error.</div>`;
    if (error) body += `<div class="error">${escapeHtml(error)}</div>`;
    if (trusted && !complete) body += `<div class="admitted-note">This theorem relies on a trusted SMT result. See <code>kestrel check --trusted</code>.</div>`;
    if (cases.length) {
      body += `<div class="cases"><div class="cases-head">Cases</div>${cases.map((branch, index) =>
        `<button class="case${branch.active ? ' active' : ''}" data-line="${branch.line}" data-column="${branch.column}">
          <span class="case-label">${escapeHtml(branch.label || `case ${index + 1}`)}</span>
          <span class="case-target">⊢ ${show(branch.target || '')}</span>
        </button>`
      ).join('')}</div>`;
    }
    if (!count) {
      if (complete && admitted) body += `<div class="admitted-note">No goals, but this proof uses <code>sorry</code>. It cannot discharge a function requirement.</div>`;
      else if (complete && trusted) body += `<div class="admitted-note">No goals. This proof relies on a solver result without an independently checked certificate. See <code>kestrel check --trusted</code>.</div>`;
      else if (complete) body += `<div class="qed">No goals. The proof is complete.</div>`;
    } else {
      body += goals.map((goal, index) => {
        const variables = (goal.variables || []).map(([name, type]) =>
          `<div class="entry variable"><span class="name">${escapeHtml(name)}</span><span class="colon">:</span><span class="type">${show(type)}</span></div>`
        ).join('');
        const hypotheses = (goal.hypotheses || []).map(([name, prop]) =>
          `<div class="entry hypothesis"><span class="name">${escapeHtml(name)}</span><span class="colon">:</span><span class="prop">${show(prop)}</span></div>`
        ).join('');
        return `<section class="goal">
          <div class="goal-head"><span class="goal-number">Goal ${index + 1}</span>${count > 1 ? `<span class="goal-of">of ${count}</span>` : ''}</div>
          <div class="context">${variables}${variables && hypotheses ? '<div class="divider"></div>' : ''}${hypotheses}</div>
          ${variables || hypotheses ? '<div class="target-divider"></div>' : ''}
          <div class="target${typeset ? ' typeset' : ''}"><span class="turnstile">⊢</span>${typeset ? `<span class="k-fit">${show(goal.target)}</span>` : ` ${show(goal.target)}`}</div>
        </section>`;
      }).join('');
    }
    if (state.path) body += `<div class="source">${escapeHtml(state.path)}</div>`;
  }
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline';">
<style>
  :root { color-scheme: light dark; }
  body {
    margin: 0; padding: 10px 10px 16px;
    color: var(--vscode-foreground);
    background: var(--vscode-sideBar-background, var(--vscode-editor-background));
    font-family: var(--vscode-font-family);
    font-size: var(--vscode-font-size, 13px);
  }
  header { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 10px; }
  .theorem { font-weight: 600; font-size: 1.05em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .theorem::before { content: "theorem "; color: var(--vscode-descriptionForeground); font-weight: 400; }
  .badge {
    flex: none; padding: 1px 7px; border-radius: 10px; font-size: 0.78em;
    background: var(--vscode-badge-background); color: var(--vscode-badge-foreground);
  }
  .badge.complete { background: var(--vscode-testing-iconPassed, #2ea043); color: #fff; }
  .badge.admitted { background: var(--vscode-editorWarning-foreground, #d19a66); color: #1b1405; }
  .admitted-note { color: var(--vscode-editorWarning-foreground, #d19a66); padding: 10px 2px; line-height: 1.45; }
  .stale-note { color: var(--vscode-descriptionForeground); border-left: 3px solid var(--vscode-editorWarning-foreground, #d19a66); padding: 6px 8px; margin-bottom: 9px; font-size: 0.9em; line-height: 1.4; }
  .goal {
    border: 1px solid var(--vscode-widget-border, var(--vscode-panel-border, #3c3c3c55));
    border-radius: 7px; padding: 8px 9px; margin-bottom: 9px;
    background: var(--vscode-editor-background);
  }
  .goal-head { display: flex; align-items: baseline; justify-content: space-between; margin-bottom: 6px; }
  .goal-number { font-size: 0.76em; text-transform: uppercase; letter-spacing: 0.07em; color: var(--vscode-descriptionForeground); }
  .goal-of { font-size: 0.76em; color: var(--vscode-descriptionForeground); }
  .cases { margin-bottom: 9px; border: 1px solid var(--vscode-widget-border, #3c3c3c55); border-radius: 7px; overflow: hidden; }
  .cases-head { padding: 5px 9px; font-size: 0.76em; text-transform: uppercase; letter-spacing: 0.07em; color: var(--vscode-descriptionForeground); background: var(--vscode-editorWidget-background, #00000022); }
  button.case { display: flex; gap: 8px; width: 100%; text-align: left; padding: 5px 9px; font-family: var(--vscode-editor-font-family, monospace); font-size: 0.9em; border: 0; border-top: 1px solid var(--vscode-widget-border, #3c3c3c33); background: transparent; color: inherit; cursor: pointer; }
  button.case:hover { background: var(--vscode-list-hoverBackground, #ffffff11); }
  .case.active { background: var(--vscode-list-activeSelectionBackground, #04395e); color: var(--vscode-list-activeSelectionForeground, #fff); }
  /* The active row's own foreground must win over the syntax colours. In a
     light theme the selection is blue, and a purple label or a keyword-coloured
     connective disappears against it. */
  .case.active .case-label,
  .case.active .case-target,
  .case.active .case-target .symbol,
  .case.active .case-target .call,
  .case.active .case-target .literal,
  .case.active .case-target .k-sym,
  .case.active .case-target .k-call,
  .case.active .case-target .k-lit,
  .case.active .case-target .k-type { color: inherit; }
  .case-label { color: var(--vscode-symbolIcon-keywordForeground, #c586c0); flex: none; font-weight: 600; }
  .case-target { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .entry { display: flex; gap: 6px; line-height: 1.5; margin: 1px 0; font-family: var(--vscode-editor-font-family, monospace); font-size: var(--vscode-editor-font-size, 12.5px); }
  .entry .name { color: var(--vscode-symbolIcon-variableForeground, var(--vscode-foreground)); font-weight: 600; flex: none; }
  .entry .colon { color: var(--vscode-descriptionForeground); flex: none; }
  .entry .type { color: var(--vscode-typeForeground, #4ec9b0); }
  .entry .prop { color: var(--vscode-foreground); white-space: pre-wrap; }
  .entry.hypothesis .name { color: var(--vscode-symbolIcon-propertyForeground, #d7ba7d); }
  .divider { height: 1px; background: var(--vscode-widget-border, #3c3c3c55); margin: 6px 0; }
  .target-divider { height: 1px; background: var(--vscode-widget-border, #3c3c3c55); margin: 7px 0 6px; }
  .target { font-family: var(--vscode-editor-font-family, monospace); font-size: var(--vscode-editor-font-size, 12.5px); white-space: pre-wrap; color: var(--vscode-foreground); }
  .turnstile { color: var(--vscode-descriptionForeground); margin-right: 3px; }
  .symbol { color: var(--vscode-symbolIcon-keywordForeground, #c586c0); }
  .call { color: var(--vscode-symbolIcon-functionForeground, #dcdcaa); }
  .literal { color: var(--vscode-symbolIcon-booleanForeground, #569cd6); }
  /* A wide formula shrinks to the panel instead of being clipped. The height
     must follow the constrained width, or the drawing keeps its own size. */
  .k-math { max-width: 100%; height: auto; }
  /* The target line holds the turnstile and the formula. As a flex row the
     formula takes the space left beside the turnstile, so a long one shrinks
     there instead of moving to a line of its own. */
  .target.typeset { display: flex; align-items: baseline; gap: 4px; }
  .target.typeset .turnstile { flex: none; }
  .target.typeset .k-fit { flex: 1 1 auto; min-width: 0; }
  /* The drawing inherits currentColor, so these four rules are the whole theme
     story: the same variables the printed form uses, no colour of their own. */
  .k-math .k-sym { color: var(--vscode-symbolIcon-keywordForeground, #c586c0); }
  .k-math .k-call { color: var(--vscode-symbolIcon-functionForeground, #dcdcaa); }
  .k-math .k-lit { color: var(--vscode-symbolIcon-booleanForeground, #569cd6); }
  .k-math .k-type { color: var(--vscode-typeForeground, #4ec9b0); }
  .error {
    border-left: 3px solid var(--vscode-inputValidation-errorBorder, #f14c4c);
    background: var(--vscode-inputValidation-errorBackground, #f14c4c22);
    padding: 7px 8px; margin-bottom: 9px; border-radius: 4px;
    font-size: 0.92em; white-space: pre-wrap;
  }
  .qed { color: var(--vscode-testing-iconPassed, #2ea043); padding: 10px 2px; }
  .empty { padding: 8px 2px; }
  .empty-title { font-weight: 600; margin-bottom: 4px; }
  .hint { color: var(--vscode-descriptionForeground); line-height: 1.45; }
  code { font-family: var(--vscode-editor-font-family, monospace); background: var(--vscode-textCodeBlock-background, #00000022); padding: 1px 3px; border-radius: 3px; }
  .source { margin-top: 12px; color: var(--vscode-descriptionForeground); font-size: 0.78em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
</head>
<body>
<header><span class="theorem">${title}</span>${state && state.theorem ? badge : ''}</header>
${body}
<script>
  const api = acquireVsCodeApi();
  for (const button of document.querySelectorAll('button.case')) {
    button.addEventListener('click', () => {
      api.postMessage({
        type: 'focusCase',
        line: Number(button.dataset.line),
        column: Number(button.dataset.column)
      });
    });
  }
</script>
</body>
</html>`;
}

class ProofStateView {
  constructor(context) {
    this.context = context;
    this.view = null;
    this.daemons = new Map();
    this.timer = null;
    this.sequence = 0;
    this.last = new Map();
    this.log = vscode.window.createOutputChannel('Kestrel Proof State');
    const refresh = editor => { if (!editor || editor === vscode.window.activeTextEditor) this.schedule(editor); };
    context.subscriptions.push(
      this.log,
      vscode.window.onDidChangeTextEditorSelection(event => refresh(event.textEditor)),
      vscode.window.onDidChangeActiveTextEditor?.(editor => this.schedule(editor)) || { dispose() {} },
      vscode.workspace.onDidSaveTextDocument(() => this.schedule(vscode.window.activeTextEditor)),
      vscode.workspace.onDidChangeConfiguration?.(event => {
        const math = event.affectsConfiguration('kestrel.proofState.math')
          || event.affectsConfiguration('kestrel.proofState.mathSymbols');
        if (!math && !event.affectsConfiguration('kestrel.proofState.enabled')) return;
        if (math) syncMathButton();
        this.schedule(vscode.window.activeTextEditor);
      }) || { dispose() {} },
      vscode.commands.registerCommand('kestrel.refreshProofState', () => this.schedule(vscode.window.activeTextEditor))
    );
  }

  resolveWebviewView(view) {
    this.view = view;
    view.webview.options = { enableScripts: true };
    this.show(null);
    view.onDidReceiveMessage?.(message => {
      if (message?.type !== 'focusCase') return;
      const editor = vscode.window.activeTextEditor;
      if (!editor) return;
      const position = new vscode.Position(Math.max(0, message.line - 1), Math.max(0, message.column - 1));
      editor.selection = new vscode.Selection(position, position);
      editor.revealRange?.(new vscode.Range(position, position));
    });
    view.onDidChangeVisibility?.(() => {
      if (view.visible) this.schedule(vscode.window.activeTextEditor);
    });
    this.schedule(vscode.window.activeTextEditor);
  }

  async reveal() {
    try {
      await vscode.commands.executeCommand('kestrel.proofState.focus');
    } catch (error) {
      this.log.appendLine(`Could not reveal the proof state: ${error.message}`);
      vscode.window.showInformationMessage('Open the Kestrel view in the activity bar to see proof goals.');
    }
  }

  rootOf(document) {
    return checkRoot(document);
  }

  daemonFor(root) {
    const { command, note } = resolveCompiler(vscode.workspace.getConfiguration('kestrel').get('compilerPath') || '');
    if (note) this.log.appendLine(note);
    const key = `${command}\u0000${root}`;
    const stamp = compilerStamp(command);
    let entry = this.daemons.get(key);
    if (entry && entry.stamp !== stamp) {
      entry.daemon.dispose();
      this.daemons.delete(key);
      entry = undefined;
    }
    if (!entry) {
      entry = {
        stamp,
        daemon: new CheckDaemon(command, root, { log: text => { if (text) this.log.appendLine(text); } })
      };
      this.daemons.set(key, entry);
    }
    return entry.daemon;
  }

  settings() {
    return {
      math: setting('proofState.math', true),
      symbols: setting('proofState.mathSymbols', {})
    };
  }

  show(state) {
    if (!this.view) return;
    this.view.webview.html = proofStateHtml(state, this.settings());
  }

  schedule(editor) {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.update(editor), PROOF_PAUSE_MS);
  }

  async update(editor) {
    if (!this.view || this.view.visible === false) return;
    if (!setting('proofState.enabled', true)) {
      this.show({ disabled: true });
      return;
    }
    if (!editor || !editor.document || editor.document.languageId !== 'kestrel') {
      this.show(null);
      return;
    }
    const document = editor.document;
    const position = editor.selection ? editor.selection.active : { line: 0, character: 0 };
    const text = document.getText();
    const version = document.version;
    const sequence = ++this.sequence;
    const root = this.rootOf(document);
    const sourcePath = document.uri.fsPath || path.join(root, 'untitled.kes');
    try {
      const result = await this.daemonFor(root).goals({
        path: sourcePath,
        text,
        offset: document.offsetAt(position)
      });
      if (!result || result.cancelled || sequence !== this.sequence) return;
      if (document.version !== undefined && document.version !== version) return;
      if (result.inTheorem === false) {
        this.last.delete(sourcePath);
        this.show(null);
        return;
      }
      const hasState = result.theorem && (result.goals.length > 0 || result.complete);
      if (hasState) {
        this.last.set(sourcePath, result);
        this.show({ ...result, path: sourcePath });
        return;
      }
      const previous = this.last.get(sourcePath);
      if (previous) {
        this.show({
          ...previous,
          path: sourcePath,
          stale: true,
          error: result.error || previous.error
        });
        return;
      }
      this.show({ ...result, path: sourcePath });
    } catch (error) {
      if (sequence !== this.sequence) return;
      this.log.appendLine(`Proof state unavailable: ${error.message}`);
      this.show({ theorem: '', goals: [], error: `The checker is unavailable: ${error.message}` });
    }
  }

  dispose() {
    clearTimeout(this.timer);
    for (const entry of this.daemons.values()) entry.daemon.dispose();
    this.daemons.clear();
  }
}

// Errors from the checker appear while you type. The first check starts one
// long-lived `kestrel serve` process, which keeps the standard library checked;
// after that a keystroke costs only the open file's parse and check. An older
// compiler without `serve` still works through one process per check.
const CHECK_PAUSE_MS = 60;

function setupDiagnostics(context) {
  const diagnostics = vscode.languages.createDiagnosticCollection('kestrel');
  const log = vscode.window.createOutputChannel('Kestrel');
  const pending = new Map();
  const running = new Map();
  const daemons = new Map();
  const failed = new Set();
  const logged = new Set();

  const rootOf = document => checkRoot(document);

  function compiler() {
    const chosen = resolveCompiler(vscode.workspace.getConfiguration('kestrel').get('compilerPath') || '');
    if (chosen.note && !logged.has(chosen.command)) {
      logged.add(chosen.command);
      log.appendLine(chosen.note);
    }
    return chosen.command;
  }

  function toDiagnostic(item) {
    const startLine = Math.max(0, item.line - 1);
    const startColumn = Math.max(0, item.column - 1);
    let endLine = Math.max(startLine, (item.endLine || item.line) - 1);
    let endColumn = Math.max(0, (item.endColumn || item.column) - 1);
    if (endLine === startLine && endColumn <= startColumn) endColumn = startColumn + 1;
    const range = new vscode.Range(startLine, startColumn, endLine, endColumn);
    const severity = item.severity === 'warning' ? vscode.DiagnosticSeverity.Warning : vscode.DiagnosticSeverity.Error;
    const message = item.hint ? `${item.message}\nhint: ${item.hint}` : item.message;
    const found = new vscode.Diagnostic(range, message, severity);
    found.code = item.code;
    found.source = 'kestrel';
    return found;
  }

  // The fallback reads the checker's human-readable output, including the
  // caret line that gives the error's width.
  function toTextDiagnostic(item) {
    const start = Math.max(0, item.column - 1);
    const range = new vscode.Range(item.line - 1, start, item.line - 1, start + Math.max(1, item.length));
    const severity = item.severity === 'warning' ? vscode.DiagnosticSeverity.Warning : vscode.DiagnosticSeverity.Error;
    const message = item.hint ? `${item.message}\nhint: ${item.hint}` : item.message;
    const found = new vscode.Diagnostic(range, message, severity);
    found.code = item.code;
    found.source = 'kestrel';
    return found;
  }

  function daemonFor(command, root) {
    const key = `${command}\u0000${root}`;
    const stamp = compilerStamp(command);
    let entry = daemons.get(key);
    if (entry && entry.stamp !== stamp) {
      entry.daemon.dispose();
      daemons.delete(key);
      failed.delete(key);
      actionsUnavailable.delete(key);
      entry = undefined;
    }
    if (!entry) {
      entry = {
        stamp,
        daemon: new CheckDaemon(command, root, {
          log: text => { if (text) log.appendLine(text); }
        })
      };
      daemons.set(key, entry);
    }
    return { key, daemon: entry.daemon };
  }

  // Diagnostics are counted from the start of the checked text. Lines in the
  // prefix belong to earlier cells, so they are dropped and the rest move up to
  // the cell's own first line.
  function inCell(items, prefixLines) {
    if (!prefixLines) return items;
    return items
      .filter(item => item.line > prefixLines)
      .map(item => ({
        ...item,
        line: item.line - prefixLines,
        endLine: item.endLine === undefined ? undefined : item.endLine - prefixLines
      }));
  }

  // The text the checker sees: a notebook cell is preceded by every earlier
  // cell, because the kernel runs them as one session. The prefix length also
  // converts the server's offsets back into this document's coordinates.
  function bufferFor(document) {
    const scheme = document.uri.scheme || 'file';
    const notebook = Boolean(document.notebook) || scheme === 'vscode-notebook-cell';
    const prefix = notebook ? notebookPrefix(document) : '';
    const root = rootOf(document);
    const sourcePath = !notebook && scheme === 'file' && document.uri.fsPath
      ? document.uri.fsPath
      : path.join(root, 'untitled.kes');
    return {
      text: prefix + document.getText(),
      prefix,
      prefixLines: prefix ? prefix.split('\n').length - 1 : 0,
      notebook,
      root,
      sourcePath
    };
  }

  async function run(document) {
    const key = document.uri.toString();
    pending.delete(key);
    if (running.has(key)) running.get(key).kill();
    const text = document.getText();
    const version = document.version;
    const buffer = bufferFor(document);
    const checked = buffer.text;
    const prefixLines = buffer.prefixLines;
    const root = buffer.root;
    const sourcePath = buffer.sourcePath;
    const notebook = buffer.notebook;
    // The checker answers asynchronously; a reply for text the user already
    // changed must not replace newer diagnostics.
    const current = () => (document.version === undefined || document.version === version)
      && document.getText() === text;

    if (setting('useServer', true)) {
      const command = compiler();
      const { key: daemonKey, daemon } = daemonFor(command, root);
      if (!failed.has(daemonKey)) {
        try {
          const result = await daemon.check({ path: sourcePath, text: checked, notebook });
          if (!result || result.cancelled || !current()) return;
          diagnostics.set(document.uri, inCell(result.diagnostics || [], prefixLines).map(toDiagnostic));
          return;
        } catch (error) {
          failed.add(daemonKey);
          log.appendLine(`The warm checker at ${command} is unavailable: ${error.message}`);
          log.appendLine('Falling back to one checker process per edit. Rebuild the compiler for instant checks.');
        }
      }
    }

    const check_ = check.startCheck(compiler(), root, checked, 10000, notebook, sourcePath);
    running.set(key, check_);
    try {
      const result = await check_.promise;
      if (running.get(key) !== check_) return;
      running.delete(key);
      if (!current()) return;
      if (result.code > 1) log.appendLine(`The checker exited with ${result.code}:\n${result.output}`);
      diagnostics.set(document.uri, inCell(check.parseCheckOutput(result.output), prefixLines).map(toTextDiagnostic));
    } catch (error) {
      if (running.get(key) === check_) running.delete(key);
      log.appendLine(`Could not run the checker at ${compiler()}: ${error.message}`);
    }
  }

  // Quick-fix requests use the same warm checker. The checker attached each
  // fix to the diagnostic that explains the gap, so this forwards only the
  // cursor's range and converts the returned offsets back to this document.
  const actionsUnavailable = new Set();
  async function requestActions(document, range) {
    if (!setting('useServer', true)) return [];
    const buffer = bufferFor(document);
    const start = document.offsetAt(range.start) + buffer.prefix.length;
    const end = document.offsetAt(range.end) + buffer.prefix.length;
    const command = compiler();
    const { key: daemonKey, daemon } = daemonFor(command, buffer.root);
    if (failed.has(daemonKey) || actionsUnavailable.has(daemonKey)) return [];
    try {
      const result = await daemon.actions({
        path: buffer.sourcePath,
        text: buffer.text,
        notebook: buffer.notebook,
        start,
        end
      });
      if (!result || result.cancelled) return [];
      return (result.actions || []).map(action => ({
        ...action,
        edits: (action.edits || []).map(edit => ({
          start: Math.max(0, edit.start - buffer.prefix.length),
          end: Math.max(0, edit.end - buffer.prefix.length),
          text: edit.text
        }))
      }));
    } catch (error) {
      actionsUnavailable.add(daemonKey);
      log.appendLine(`Quick fixes unavailable: ${error.message}`);
      return [];
    }
  }

  function schedule(document, immediate = false) {
    if (document.languageId !== 'kestrel') return;
    const key = document.uri.toString();
    clearTimeout(pending.get(key));
    // With checking on type off, the file is checked when it is saved.
    if (!setting('checkOnType.enabled', true)) return;
    const pause = immediate ? 0 : Math.max(0, Number(setting('checkOnType.pauseMs', CHECK_PAUSE_MS)) || 0);
    pending.set(key, setTimeout(() => run(document), pause));
  }

  function resetServers() {
    for (const entry of daemons.values()) entry.daemon.dispose();
    daemons.clear();
    failed.clear();
    logged.clear();
    actionsUnavailable.clear();
  }

  context.subscriptions.push(
    diagnostics,
    log,
    vscode.workspace.onDidOpenTextDocument(document => schedule(document, true)),
    vscode.workspace.onDidChangeTextDocument(event => schedule(event.document)),
    vscode.workspace.onDidSaveTextDocument(document => {
      if (document.languageId === 'kestrel') run(document);
    }),
    vscode.workspace.onDidCloseTextDocument(document => {
      const key = document.uri.toString();
      clearTimeout(pending.get(key));
      pending.delete(key);
      if (running.has(key)) running.get(key).kill();
      running.delete(key);
      diagnostics.delete(document.uri);
    }),
    vscode.workspace.onDidChangeConfiguration(event => {
      if (!event.affectsConfiguration('kestrel.compilerPath') && !event.affectsConfiguration('kestrel.useServer')) return;
      resetServers();
      (vscode.workspace.textDocuments || []).forEach(document => schedule(document, true));
    }),
    { dispose: resetServers }
  );
  // Opening the editor warms the standard library before the first keystroke.
  (vscode.workspace.textDocuments || []).forEach(document => schedule(document, true));
  return { actionsFor: requestActions };
}

// The light bulb on an incomplete class, match or proof. The checker supplies
// fully formed edits, so the provider only turns offsets into a WorkspaceEdit
// and attaches the matching diagnostics.
class KestrelCodeActionProvider {
  constructor(actionsFor) {
    this.actionsFor = actionsFor;
  }

  async provideCodeActions(document, range, context) {
    if (document.languageId !== 'kestrel') return [];
    let found = [];
    try {
      found = await this.actionsFor(document, range);
    } catch (error) {
      return [];
    }
    const diagnostics = (context && context.diagnostics) || [];
    return found.map(item => {
      const action = new vscode.CodeAction(item.title, vscode.CodeActionKind.QuickFix);
      const edit = new vscode.WorkspaceEdit();
      for (const change of item.edits || []) {
        edit.replace(
          document.uri,
          new vscode.Range(document.positionAt(change.start), document.positionAt(change.end)),
          change.text
        );
      }
      action.edit = edit;
      const matching = diagnostics.filter(diagnostic =>
        !item.code || String(diagnostic.code) === item.code
      );
      if (matching.length) action.diagnostics = matching;
      action.isPreferred = found.length === 1;
      return action;
    });
  }
}

// Formatting goes through `kestrel fmt --stdin`, so an unsaved buffer is
// formatted without a save. Registering the provider is enough for VS Code's
// `editor.formatOnSave` setting to work.
class KestrelFormattingProvider {
  async provideDocumentFormattingEdits(document) {
    const chosen = resolveCompiler(
      vscode.workspace.getConfiguration('kestrel').get('compilerPath') || ''
    );
    const text = document.getText();
    const version = document.version;
    let result;
    try {
      result = await startFormat(chosen.command, text).promise;
    } catch (error) {
      vscode.window.showErrorMessage(`Kestrel formatter is unavailable: ${error.message}`);
      return [];
    }
    if (result.code !== 0) {
      const detail = result.output.trim();
      vscode.window.showErrorMessage(
        `Kestrel could not format this file${detail ? `: ${detail}` : '.'}`
      );
      return [];
    }
    if (result.output === text) return [];
    if (document.version !== undefined && document.version !== version) return [];
    const range = new vscode.Range(document.positionAt(0), document.positionAt(text.length));
    return [vscode.TextEdit.replace(range, result.output)];
  }
}

function activate(context) {
  docMath.configure(context.globalStorageUri?.fsPath);
  syncMathButton();
  const inlay = new KestrelInlayHintsProvider();
  context.subscriptions.push(
    inlay.changed,
    // Hints that follow the cursor line are recomputed when the cursor moves.
    vscode.window.onDidChangeTextEditorSelection(() => {
      if (setting('inlayHints.parameterNames', 'always') === 'cursorLine') inlay.changed.fire();
    }),
    vscode.workspace.onDidChangeConfiguration(event => {
      if (event.affectsConfiguration('kestrel')) inlay.changed.fire();
    }),
    vscode.commands.registerCommand('kestrel.proofStateMathOn', () => toggleProofStateMath()),
    vscode.commands.registerCommand('kestrel.proofStateMathOff', () => toggleProofStateMath()),
    vscode.commands.registerCommand('kestrel.cycleParameterNames', async () => {
      const order = ['always', 'cursorLine', 'off'];
      const current = setting('inlayHints.parameterNames', 'always');
      const next = order[(order.indexOf(current) + 1) % order.length];
      await vscode.workspace.getConfiguration('kestrel').update('inlayHints.parameterNames', next, vscode.ConfigurationTarget.Global);
      vscode.window.setStatusBarMessage(`Kestrel parameter names: ${next}`, 2500);
    }),
    vscode.languages.registerDocumentSemanticTokensProvider(
      selector,
      new KestrelSemanticTokensProvider(),
      semanticLegend
    ),
    vscode.languages.registerCompletionItemProvider(
      selector,
      new KestrelCompletionProvider(),
      '(', ',', '=', '.'
    ),
    vscode.languages.registerSignatureHelpProvider(
      selector,
      new KestrelSignatureProvider(),
      '(', ','
    ),
    vscode.languages.registerHoverProvider(selector, new KestrelHoverProvider()),
    vscode.languages.registerDefinitionProvider(selector, new KestrelDefinitionProvider()),
    vscode.languages.registerInlayHintsProvider(selector, inlay),
    vscode.languages.registerDocumentSymbolProvider(selector, new KestrelSymbolProvider()),
    vscode.languages.registerDocumentFormattingEditProvider(
      selector,
      new KestrelFormattingProvider()
    )
  );
  if (vscode.workspace.onDidChangeTextDocument) {
    const symbols = vscode.workspace.onDidChangeTextDocument(async event => {
      if (event.document.languageId !== 'kestrel') return;
      const setting = vscode.workspace.getConfiguration('kestrel').get('expandSymbols', true);
      if (!setting) return;
      const edits = [];
      for (const change of event.contentChanges) {
        const found = symbolEdit(event.document, change, setting);
        if (found) edits.push(found);
      }
      if (!edits.length) return;
      const edit = new vscode.WorkspaceEdit();
      for (const { range, symbol } of edits) {
        if (symbol) edit.replace(event.document.uri, range, symbol);
      }
      await vscode.workspace.applyEdit(edit);
    });
    context.subscriptions.push(symbols);
  }
  const diagnostics = setupDiagnostics(context);
  context.subscriptions.push(
    vscode.languages.registerCodeActionsProvider(
      selector,
      new KestrelCodeActionProvider(diagnostics.actionsFor)
    )
  );
  const proofState = new ProofStateView(context);
  context.subscriptions.push(proofState);
  if (vscode.window.registerWebviewViewProvider) {
    context.subscriptions.push(
      vscode.window.registerWebviewViewProvider('kestrel.proofState', proofState, {
        webviewOptions: { retainContextWhenHidden: true }
      }),
      vscode.commands.registerCommand('kestrel.showProofState', () => proofState.reveal())
    );
  }
}

function deactivate() {}

module.exports = {
  activate,
  deactivate,
  proofStateHtml,
  toggleProofStateMath,
  escapeHtml,
  ProofStateView,
  KestrelCodeActionProvider,
  KestrelFormattingProvider,
  expandSymbols,
  symbolEdit
};
