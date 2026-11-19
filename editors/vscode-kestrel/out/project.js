'use strict';

// A Kestrel package is the folder holding `kestrel.project`, and that folder is the
// root the compiler resolves module imports from. A workspace folder can hold
// several packages, or none, so the editor locates the package instead of assuming
// the workspace folder is one.

const fs = require('fs');
const path = require('path');

const PROJECT_FILE = 'kestrel.project';

function isFile(file) {
  try {
    return fs.statSync(file).isFile();
  } catch (error) {
    return false;
  }
}

function isPackage(folder, fileExists = isFile) {
  return Boolean(folder) && fileExists(path.join(folder, PROJECT_FILE));
}

// The nearest folder at or above `folder` that holds a project file, or '' when the
// walk reaches the file system root first. `fileExists` lets a test answer without
// a real tree.
function findRoot(folder, fileExists = isFile) {
  let current = path.resolve(folder);
  for (;;) {
    if (fileExists(path.join(current, PROJECT_FILE))) return current;
    const parent = path.dirname(current);
    if (parent === current) return '';
    current = parent;
  }
}

// The root to check `filePath` from. `fallback` is the workspace folder, or the
// document's folder when it is outside the workspace. A fallback that is itself a
// package wins, because that is the project the user opened and the root they would
// pass to `kestrel check`; otherwise the file's own package is the nearest one above
// it. Files with no package anywhere use the fallback.
function rootFor(filePath, fallback, fileExists = isFile) {
  if (isPackage(fallback, fileExists)) return fallback;
  if (!filePath) return fallback;
  return findRoot(path.dirname(filePath), fileExists) || fallback;
}

module.exports = { PROJECT_FILE, findRoot, isPackage, rootFor };
