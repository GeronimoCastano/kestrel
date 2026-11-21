'use strict';

// Finds the compiler to run. A Kestrel checkout usually has both a debug and a
// release binary; the release one checks the standard library several times
// faster, and the editor starts a long-lived process either way. The setting
// always wins, so this guesses only when nothing is configured.

const fs = require('fs');
const os = require('os');
const path = require('path');

function executable(file) {
  try {
    return fs.statSync(file).isFile();
  } catch (error) {
    return false;
  }
}

// The first `kestrel` on PATH, with symlinks followed so a Cargo target folder is visible.
function onPath(name) {
  const extensions = process.platform === 'win32' ? ['.exe', '.cmd', '.bat', ''] : [''];
  for (const folder of (process.env.PATH || '').split(path.delimiter)) {
    if (!folder) continue;
    for (const extension of extensions) {
      const candidate = path.join(folder, `${name}${extension}`);
      try {
        fs.accessSync(candidate, fs.constants.X_OK);
        return { invoked: candidate, real: fs.realpathSync(candidate) };
      } catch (error) {
        // Try the next candidate.
      }
    }
  }
  return null;
}

// A debug compiler under `target/debug` points at a release sibling when that
// sibling is at least as new. An older release could check different rules, so
// it is never chosen over the debug binary the developer just built.
function releaseSibling(real) {
  const match = /([\\/])target[\\/]debug[\\/]([^\\/]+)$/i.exec(real);
  if (!match) return '';
  const release = real.replace(
    /[\\/]target[\\/]debug[\\/]([^\\/]+)$/i,
    `${match[1]}target${match[1]}release${match[1]}${match[2]}`
  );
  if (!executable(release)) return '';
  try {
    if (fs.statSync(release).mtimeMs < fs.statSync(real).mtimeMs) return '';
  } catch (error) {
    return '';
  }
  return release;
}

// Where the installers put the compiler: install.ps1 on Windows, Homebrew elsewhere. An
// editor opened from the Dock or the Start menu keeps the PATH it started with, so it can
// miss a compiler installed after it launched.
function installed(platform = process.platform, env = process.env, home = os.homedir()) {
  const candidates = platform === 'win32'
    ? [path.join(env.LOCALAPPDATA || path.join(home, 'AppData', 'Local'), 'Programs', 'Kestrel', 'bin', 'kestrel.exe')]
    : ['/opt/homebrew/bin/kestrel', '/usr/local/bin/kestrel', '/home/linuxbrew/.linuxbrew/bin/kestrel',
      path.join(home, '.linuxbrew', 'bin', 'kestrel')];
  return candidates.find(executable) || '';
}

// `{ command, note }`. `note` is logged once when it changes which binary runs.
function resolveCompiler(configured) {
  const setting = typeof configured === 'string' ? configured.trim() : '';
  if (setting) return { command: setting, note: '' };
  const found = onPath('kestrel');
  if (!found) {
    const fallback = installed();
    if (fallback) return { command: fallback, note: `Using ${fallback}; kestrel is not on this editor's PATH.` };
    return { command: 'kestrel', note: '' };
  }
  const release = releaseSibling(found.real);
  if (release) {
    return {
      command: release,
      note: `Using ${release} instead of the debug compiler ${found.real}; checks are much faster.`
    };
  }
  return { command: found.invoked, note: '' };
}

// The modification and size of the compiler, so a rebuilt binary gives a warm
// checker a new identity instead of serving the old rules until the window
// reloads. A command that is not a path has no stamp.
function compilerStamp(command) {
  try {
    const stat = fs.statSync(command);
    return `${stat.mtimeMs}:${stat.size}`;
  } catch (error) {
    return '';
  }
}

module.exports = { resolveCompiler, onPath, installed, compilerStamp };
