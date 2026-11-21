'use strict';

// The editor finds a compiler the installers put in their default folders even when
// the editor's PATH predates the install.
const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { installed } = require('../out/compiler');

const home = fs.mkdtempSync(path.join(os.tmpdir(), 'kestrel-compiler-'));
try {
  const local = path.join(home, 'Local');
  assert.strictEqual(installed('win32', { LOCALAPPDATA: local }, home), '');
  const bin = path.join(local, 'Programs', 'Kestrel', 'bin');
  fs.mkdirSync(bin, { recursive: true });
  fs.writeFileSync(path.join(bin, 'kestrel.exe'), '');
  assert.strictEqual(installed('win32', { LOCALAPPDATA: local }, home), path.join(bin, 'kestrel.exe'));

  const brew = path.join(home, '.linuxbrew', 'bin');
  fs.mkdirSync(brew, { recursive: true });
  fs.writeFileSync(path.join(brew, 'kestrel'), '');
  const found = installed('linux', {}, home);
  assert(found === path.join(brew, 'kestrel') || found.startsWith('/'), found);
  console.log('kestrel compiler install-folder tests pass');
} finally {
  fs.rmSync(home, { recursive: true, force: true });
}
