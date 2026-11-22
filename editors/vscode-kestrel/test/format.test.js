'use strict';

// Format-on-save talks to `kestrel fmt --stdin`, so the extension never writes
// a file itself. These tests drive that module directly.

const assert = require('assert');
const fs = require('fs');
const path = require('path');
const { startFormat } = require('../out/format');

const BINARY = path.join(
  __dirname,
  '..',
  '..',
  '..',
  'target',
  'release',
  process.platform === 'win32' ? 'kestrel.exe' : 'kestrel'
);

if (!fs.existsSync(BINARY)) {
  console.log('skipped the formatter runs: no release binary at ' + BINARY);
  process.exit(0);
}

async function main() {
  const formatted = await startFormat(
    BINARY,
    'function  f( a:Int ) : Int {\nlet x:=a // keep\nreturn x\n}\n'
  ).promise;
  assert.strictEqual(formatted.code, 0, formatted.output);
  assert.strictEqual(
    formatted.output,
    'function f(a : Int) : Int {\n    let x := a // keep\n    return x\n}\n'
  );

  const broken = await startFormat(BINARY, 'let x := "unclosed\n').promise;
  assert.strictEqual(broken.code, 2);
  assert(broken.output.includes('E_UNCLOSED_STRING'), broken.output);

  const unchanged = await startFormat(BINARY, 'let x := 1\n').promise;
  assert.strictEqual(unchanged.code, 0);
  assert.strictEqual(unchanged.output, 'let x := 1\n');

  console.log('kestrel formatter editor tests pass');
}

main().catch(error => {
  console.error(error);
  process.exit(1);
});
