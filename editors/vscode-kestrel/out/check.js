'use strict';

// Runs the Kestrel checker on editor text and reads its errors. This module does not
// import vscode, so Node can test it directly. The checker reads standard input when
// the file is `-`, so unsaved text is checked without a save.

const { spawn } = require('child_process');

const HEADER = /^(?:-|<session>):(\d+):(\d+): (error|warning)\[([^\]]+)\]: (.*)$/;
const HINT = /^\s*hint: (.*)$/;
const CARETS = /^\s*\|\s*(\^+)/;

// Reads each `-:line:column: severity[CODE]: message` block. A block can carry a hint line and a
// caret line that underlines the bad text. Errors for other files are not part of this output.
function parseCheckOutput(output) {
  const found = [];
  let current = null;
  for (const line of output.split(/\r?\n/)) {
    const header = HEADER.exec(line);
    if (header) {
      current = {
        line: Number(header[1]),
        column: Number(header[2]),
        severity: header[3],
        code: header[4],
        message: header[5],
        hint: '',
        length: 0
      };
      found.push(current);
      continue;
    }
    if (!current) continue;
    const hint = HINT.exec(line);
    if (hint) {
      current.hint = hint[1];
      continue;
    }
    const carets = CARETS.exec(line);
    if (carets && current.length === 0) current.length = carets[1].length;
  }
  return found;
}

// Starts one check of `text` against the project at `root`. `notebook` checks a cell as a
// session entry, where a top-level ? is legal. `kill` stops it early, for example when newer
// text arrives. The promise resolves with the checker's output once it exits.
function startCheck(compiler, root, text, timeoutMs = 10000, notebook = false, sourcePath = '') {
  const args = ['check', '--root', root];
  if (notebook) args.push('--notebook');
  // An unsaved buffer keeps its real path, so project-local imports resolve
  // while the checker's diagnostics still start with `-:`.
  else if (sourcePath) args.push('--stdin-path', sourcePath);
  args.push('-');
  const child = spawn(compiler, args, { cwd: root });
  let output = '';
  child.stdout.on('data', chunk => {
    output += chunk;
  });
  child.stderr.on('data', chunk => {
    output += chunk;
  });
  child.stdin.on('error', () => {});
  const promise = new Promise((resolve, reject) => {
    const timer = setTimeout(() => child.kill(), timeoutMs);
    child.on('error', error => {
      clearTimeout(timer);
      reject(error);
    });
    child.on('close', code => {
      clearTimeout(timer);
      resolve({ output, code });
    });
  });
  child.stdin.end(text);
  return { promise, kill: () => child.kill() };
}

module.exports = { parseCheckOutput, startCheck };
