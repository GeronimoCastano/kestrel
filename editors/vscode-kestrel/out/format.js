'use strict';

// Runs `kestrel fmt --stdin` on editor text. This module does not import
// vscode, so Node can test it directly. The formatter reads standard input, so
// an unsaved buffer is formatted without a save.

const { spawn } = require('child_process');

function startFormat(compiler, text, timeoutMs = 10000) {
  const child = spawn(compiler, ['fmt', '--stdin']);
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

module.exports = { startFormat };
