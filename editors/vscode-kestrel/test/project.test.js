'use strict';

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const project = require('../out/project');

const BINARY = path.join(__dirname, '..', '..', '..', 'target', 'release', process.platform === 'win32' ? 'kestrel.exe' : 'kestrel');
const haveBinary = fs.existsSync(BINARY);

// A fake tree answers the walk without touching the disk, which keeps these cases
// independent of wherever the test runs.
const tree = (...files) => {
  const present = new Set(files.map(file => path.resolve(file)));
  return file => present.has(path.resolve(file));
};

assert.strictEqual(
  project.findRoot('/work/app/src', tree('/work/kestrel.project', '/work/app/kestrel.project')),
  path.resolve('/work/app')
);
assert.strictEqual(
  project.findRoot(
    '/work/app/nested/deep',
    tree('/work/kestrel.project', '/work/app/kestrel.project', '/work/app/nested/kestrel.project')
  ),
  path.resolve('/work/app/nested')
);
assert.strictEqual(project.findRoot('/work/app/src', tree()), '');
assert.strictEqual(project.findRoot('/', tree('/kestrel.project')), path.resolve('/'));
assert.strictEqual(project.isPackage('/work/app', tree('/work/app/kestrel.project')), true);
assert.strictEqual(project.isPackage('/work/app', tree()), false);
assert.strictEqual(project.isPackage('', tree('/kestrel.project')), false);

// A workspace folder that is a package stays the root, matching `kestrel check
// --root .` from the folder the user opened. Otherwise the file's own package wins,
// and a file with no package above it keeps the workspace folder.
assert.strictEqual(
  project.rootFor(
    '/work/app/vendor/thing.kes',
    '/work/app',
    tree('/work/app/kestrel.project', '/work/app/vendor/kestrel.project')
  ),
  path.resolve('/work/app')
);
assert.strictEqual(
  project.rootFor('/work/app/thing.kes', '/work/app', tree('/work/app/kestrel.project')),
  path.resolve('/work/app')
);
assert.strictEqual(
  project.rootFor('/work/planner/src/thing.kes', '/work', tree('/work/planner/kestrel.project')),
  path.resolve('/work/planner')
);
assert.strictEqual(project.rootFor('/work/thing.kes', '/work', tree()), '/work');
assert.strictEqual(project.rootFor('', '/work', tree('/work/kestrel.project')), '/work');

// The real file system: a folder named kestrel.project is not a project file, and a
// file inside a package resolves to the package above it.
const workspace = fs.mkdtempSync(path.join(os.tmpdir(), 'kestrel-project-'));
const packageFolder = path.join(workspace, 'planner');
fs.mkdirSync(path.join(packageFolder, 'src'), { recursive: true });
fs.writeFileSync(path.join(packageFolder, 'kestrel.project'), 'package planner\n');
fs.writeFileSync(path.join(packageFolder, 'src', 'main.kes'), '');
fs.mkdirSync(path.join(workspace, 'not-a-project', 'kestrel.project'), { recursive: true });
assert.strictEqual(project.findRoot(path.join(packageFolder, 'src')), packageFolder);
assert.strictEqual(project.findRoot(path.join(workspace, 'not-a-project')), '');
assert.strictEqual(project.rootFor(path.join(packageFolder, 'src', 'main.kes'), workspace), packageFolder);
assert.strictEqual(project.rootFor(path.join(workspace, 'loose.kes'), workspace), workspace);
assert.strictEqual(project.rootFor(path.join(workspace, 'loose.kes'), packageFolder), packageFolder);

// The editor's failure this fixes: checking an importing file from a workspace folder
// that is not a package reports E_MODULE_NOT_FOUND, and checking it from the
// document's package does not. Skipped when this checkout has no release compiler.
if (haveBinary) {
  fs.writeFileSync(
    path.join(packageFolder, 'facts.kes'),
    'public record Fact(variable : Int, value : Int)\n'
  );
  const importing = path.join(packageFolder, 'src', 'main.kes');
  fs.writeFileSync(importing, 'import facts.{Fact}\npublic function value(fact : Fact) : Int = fact.value\n');
  const check = root => {
    const run = spawnSync(BINARY, ['check', '--root', root, importing], { encoding: 'utf8' });
    return `${run.stdout}${run.stderr}`;
  };
  assert.match(check(workspace), /E_MODULE_NOT_FOUND/);
  const output = check(project.rootFor(importing, workspace));
  assert.doesNotMatch(output, /E_MODULE_NOT_FOUND/);
  assert.match(output, /checked names and supported types/);
}

fs.rmSync(workspace, { recursive: true, force: true });
