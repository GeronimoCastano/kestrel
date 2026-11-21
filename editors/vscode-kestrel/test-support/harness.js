'use strict';

// A stand-in VS Code API that records the providers the extension registers.
// The tests drive those providers directly; no editor or compiler process is
// needed, so the timing bounds measure the extension's own work.

const fs = require('fs');
const Module = require('module');
const os = require('os');
const path = require('path');

class CompletionItem {
  constructor(label, kind) {
    this.label = label;
    this.kind = kind;
  }
}

class Range {
  constructor(startLine, startCharacter, endLine, endCharacter) {
    if (typeof startLine === 'object') {
      this.start = startLine;
      this.end = startCharacter;
    } else {
      this.start = { line: startLine, character: startCharacter };
      this.end = { line: endLine, character: endCharacter };
    }
  }
}

class Hover {
  constructor(contents, range) {
    this.contents = contents;
    this.range = range;
  }
}

class Location {
  constructor(uri, range) {
    this.uri = uri;
    this.range = range;
  }
}

const Uri = {
  file(value) {
    return {
      scheme: 'file',
      fsPath: value,
      path: value,
      toString: () => `file://${value}`
    };
  }
};

class SignatureHelp {
  constructor() {
    this.signatures = [];
    this.activeSignature = 0;
    this.activeParameter = 0;
  }
}

class SignatureInformation {
  constructor(label) {
    this.label = label;
    this.parameters = [];
  }
}

class ParameterInformation {
  constructor(label) {
    this.label = label;
  }
}

class InlayHint {
  constructor(position, label, kind) {
    this.position = position;
    this.label = label;
    this.kind = kind;
  }
}

class CodeAction {
  constructor(title, kind) {
    this.title = title;
    this.kind = kind;
    this.edit = undefined;
    this.diagnostics = undefined;
    this.isPreferred = false;
  }
}

class WorkspaceEdit {
  constructor() {
    this.edits = [];
  }
  replace(uri, range, newText) {
    this.edits.push({ uri, range, newText });
  }
}

class EventEmitter {
  constructor() {
    this.event = () => ({ dispose() {} });
  }
  fire() {}
  dispose() {}
}

const disposable = () => ({ dispose() {} });
const stub = () => disposable();

function loadExtension(configuration = {}) {
  const providers = {};
  const vscode = {
    CompletionItem,
    CompletionItemKind: new Proxy({}, { get: (target, name) => String(name) }),
    MarkdownString: class MarkdownString {
      constructor(value) { this.value = value; }
    },
    SnippetString: class SnippetString {
      constructor(value) { this.value = value; }
    },
    Hover,
    Location,
    Uri,
    SignatureHelp,
    SignatureInformation,
    ParameterInformation,
    InlayHint,
    InlayHintKind: { Type: 1, Parameter: 2 },
    CodeAction,
    CodeActionKind: { QuickFix: 'quickfix' },
    WorkspaceEdit,
    SymbolKind: new Proxy({}, { get: (target, name) => String(name) }),
    DocumentSymbol: class DocumentSymbol {
      constructor(name, detail, kind, range, selectionRange) {
        Object.assign(this, { name, detail, kind, range, selectionRange });
      }
    },
    SemanticTokensLegend: class SemanticTokensLegend {
      constructor(types, modifiers) {
        Object.assign(this, { tokenTypes: types, tokenModifiers: modifiers });
      }
    },
    SemanticTokensBuilder: class SemanticTokensBuilder {
      push() {}
      build() { return {}; }
    },
    EventEmitter,
    Range,
    ConfigurationTarget: { Global: 1 },
    window: {
      createOutputChannel: () => ({ appendLine() {}, dispose() {} }),
      onDidChangeTextEditorSelection: stub,
      setStatusBarMessage: stub,
      activeTextEditor: undefined
    },
    commands: { registerCommand: stub },
    workspace: {
      getConfiguration: () => ({
        get: key => configuration[key],
        update: () => Promise.resolve()
      }),
      onDidChangeConfiguration: stub,
      onDidSaveTextDocument: stub,
      getWorkspaceFolder: () => undefined,
      openTextDocument: async uri => documentFor(fs.readFileSync(uri.fsPath, 'utf8'), uri.fsPath),
      textDocuments: [],
      notebookDocuments: [],
      onDidOpenTextDocument: stub,
      onDidChangeTextDocument: stub,
      onDidCloseTextDocument: stub
    },
    languages: new Proxy({
      createDiagnosticCollection: () => ({ set() {}, delete() {}, dispose() {} }),
      registerCompletionItemProvider: (selector, provider) => {
        providers.completion = provider;
        return disposable();
      },
      registerHoverProvider: (selector, provider) => {
        providers.hover = provider;
        return disposable();
      },
      registerDefinitionProvider: (selector, provider) => {
        providers.definition = provider;
        return disposable();
      },
      registerSignatureHelpProvider: (selector, provider) => {
        providers.signature = provider;
        return disposable();
      },
      registerInlayHintsProvider: (selector, provider) => {
        providers.inlay = provider;
        return disposable();
      },
      registerDocumentSymbolProvider: (selector, provider) => {
        providers.symbols = provider;
        return disposable();
      },
      registerDocumentSemanticTokensProvider: (selector, provider, legend) => {
        providers.tokens = provider;
        return disposable();
      },
      registerCodeActionsProvider: (selector, provider) => {
        providers.codeActions = provider;
        return disposable();
      }
    }, {
      get: (target, name) => {
        if (name in target) return target[name];
        throw new Error(`The test harness has no vscode.languages.${String(name)}`);
      }
    })
  };

  const load = Module._load;
  Module._load = function (request, parent, isMain) {
    if (request === 'vscode') {
      return new Proxy(vscode, { get: (target, name) => (name in target ? target[name] : class Stub {}) });
    }
    return load.call(this, request, parent, isMain);
  };
  const file = path.join(__dirname, '..', 'out', 'extension.js');
  delete require.cache[require.resolve(file)];
  for (const key of Object.keys(require.cache)) {
    if (key.includes(`${path.sep}vscode-kestrel${path.sep}out${path.sep}`)) delete require.cache[key];
  }
  const extension = require(file);
  extension.activate({ subscriptions: { push() {} } });
  Module._load = load;

  return { providers, documentFor, configuration, vscode, extension };
}

// A document whose positions are plain offsets. VS Code positions are
// line/character pairs, so both directions are implemented through line starts.
function documentFor(text, name = 'test.kes', notebook = false) {
  // The temporary folder exists on every platform; /tmp does not exist on Windows.
  const fsPath = path.isAbsolute(name) ? name : path.join(os.tmpdir(), name);
  const lines = text.split('\n');
  const starts = [];
  let offset = 0;
  for (const line of lines) {
    starts.push(offset);
    offset += line.length + 1;
  }
  const offsetOf = position => (starts[position.line] || 0) + position.character;
  return {
    uri: {
      toString: () => `${notebook ? 'vscode-notebook-cell' : 'file'}://${fsPath}`,
      fsPath,
      scheme: notebook ? 'vscode-notebook-cell' : 'file'
    },
    languageId: 'kestrel',
    notebook: notebook ? {} : undefined,
    version: 1,
    lineCount: lines.length,
    getText: range => range
      ? text.slice(offsetOf(range.start), offsetOf(range.end))
      : text,
    offsetAt: offsetOf,
    positionAt: value => {
      let line = 0;
      while (line + 1 < starts.length && starts[line + 1] <= value) line += 1;
      return { line, character: value - starts[line] };
    },
    getWordRangeAtPosition: position => {
      const base = offsetOf(position);
      let start = base;
      let end = base;
      while (start > 0 && /[A-Za-z0-9_]/.test(text[start - 1])) start -= 1;
      while (end < text.length && /[A-Za-z0-9_]/.test(text[end])) end += 1;
      if (start === end) return undefined;
      return { start: positionOf(start), end: positionOf(end) };
    }
  };

  function positionOf(value) {
    let line = 0;
    while (line + 1 < starts.length && starts[line + 1] <= value) line += 1;
    return { line, character: value - starts[line] };
  }
}

// The end offset of `marker`, for pointing a provider at a dot or a call.
function endOf(text, marker) {
  const start = text.indexOf(marker);
  if (start < 0) throw new Error(`Marker not found: ${marker}`);
  return start + marker.length;
}

function labelsAt(providers, text, offset, name) {
  const document = documentFor(text, name);
  const items = providers.completion.provideCompletionItems(document, document.positionAt(offset)) || [];
  return items.map(item => item.label);
}

function hoverAt(providers, text, offset, name) {
  const document = documentFor(text, name);
  return providers.hover.provideHover(document, document.positionAt(offset));
}

function signatureAt(providers, text, offset, name) {
  const document = documentFor(text, name);
  return providers.signature.provideSignatureHelp(document, document.positionAt(offset));
}

function inlayHints(providers, text, start = 0, end = text.length, name) {
  const document = documentFor(text, name);
  return providers.inlay.provideInlayHints(document, {
    start: document.positionAt(start),
    end: document.positionAt(end)
  });
}

module.exports = { loadExtension, documentFor, endOf, labelsAt, hoverAt, signatureAt, inlayHints };
