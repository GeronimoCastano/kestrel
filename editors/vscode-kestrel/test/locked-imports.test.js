'use strict';
const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync } = require('child_process');
const imports = require('../out/imports');
const { loadExtension, labelsAt, hoverAt, signatureAt } = require('../test-support/harness');
// A file URL Git accepts on every platform: file:///C:/... on Windows.
const fileUrl = folder => {
  const slashed = folder.split(path.sep).join('/');
  return slashed.startsWith('/') ? `file://${slashed}` : `file:///${slashed}`;
};
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'kestrel-locked-editor-'));
const previous = process.env.XDG_CACHE_HOME;
process.env.XDG_CACHE_HOME = path.join(temporary, 'cache');
const { providers } = loadExtension();
try {
  const upstream = path.join(temporary, 'geometry');
  const app = path.join(temporary, 'app');
  fs.mkdirSync(upstream); fs.mkdirSync(app);
  fs.writeFileSync(path.join(upstream, 'kestrel.project'), 'package geometry\n');
  fs.writeFileSync(path.join(upstream, 'interfaces.kes'), `
public trait Measure {
    /// Width from the interface.
    public function width():Int=3
}
`);
  fs.writeFileSync(path.join(upstream, 'qualified.kes'), 'import interfaces as iface\npublic class Qualified {has iface.Measure;new(){}}\n');
  fs.writeFileSync(path.join(upstream, 'shapes.kes'), `import interfaces.{Measure}
/// A square with $s^2$ area.
public class Square {has Measure;new(){}}
/// A class theorem.
public theorem width(s:Square):s.width()=3{rfl}
private theorem hidden():0=0{rfl}
`);
  const git = args => execFileSync('git', ['-C', upstream, ...args], {stdio:'pipe'});
  git(['init', '--quiet']); git(['add', '.']);
  git(['-c','user.name=Editor test','-c','user.email=editor@example.invalid','commit','--quiet','-m','Fixture']);
  const commit = git(['rev-parse','HEAD']).toString().trim();
  fs.writeFileSync(path.join(app, 'kestrel.project'), `package app\ndepends geometry from "${fileUrl(upstream)}" at "${commit}"\n`);
  const compiler = path.resolve(__dirname, '../../../target/release/kestrel');
  execFileSync(compiler, ['fetch'], {cwd:app, stdio:'pipe'});
  const filename = path.join(app, 'main.kes');
  const text = 'import geometry.shapes.{Square, width}\nlet square:=Square()\nsquare.';
  const resolved = imports.resolve(app, 'geometry.shapes');
  assert(resolved.startsWith(process.env.XDG_CACHE_HOME));
  fs.rmSync(upstream, {recursive:true,force:true});
  assert.strictEqual(imports.resolve(app, 'geometry.shapes'), resolved, 'offline resolution');
  const qualified = 'import geometry.qualified.{Qualified}\nlet item:=Qualified()\nitem.';
  assert(labelsAt(providers,qualified,qualified.length,filename).includes('width'), 'default via qualified trait import');
  assert(labelsAt(providers,text,text.length,filename).includes('width'), 'default from an unselected trait');
  const proof = 'import geometry.shapes.{Square, width}\ntheorem t(s:Square):s.width()=3{ ';
  const names = labelsAt(providers,proof,proof.length,filename);
  assert(names.includes('width')); assert(!names.includes('hidden')); assert(!names.includes('Measure'));
  assert(hoverAt(providers, proof+'width(s)}', proof.length+2,filename).contents.value.includes('A class theorem.'));
  assert(signatureAt(providers,proof+'width(',proof.length+6,filename).signatures[0].label.match(/s\s*:\s*Square/));
  const selecting = 'import geometry.shapes.{';
  assert(labelsAt(providers,selecting,selecting.length,filename).includes('width'), 'import suggestions');
  const lockfile = path.join(app,'kestrel.lock');
  const lock = JSON.parse(fs.readFileSync(lockfile,'utf8'));
  lock.packages[0].reference='wrong-ref';fs.writeFileSync(lockfile,JSON.stringify(lock));
  assert.strictEqual(imports.resolve(app,'geometry.shapes'),'','a stale lock never falls back to a sibling');
} finally {
  if (previous === undefined) delete process.env.XDG_CACHE_HOME; else process.env.XDG_CACHE_HOME=previous;
  fs.rmSync(temporary,{recursive:true,force:true});
}
console.log('Locked offline package imports, docs, defaults, theorem and import suggestions pass');
