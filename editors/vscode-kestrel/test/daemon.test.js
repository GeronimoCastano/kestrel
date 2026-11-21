'use strict';

// Exercises the long-lived checker the editor uses. The first check pays for
// the standard library; later checks must reuse it, which is the whole point of
// the server.

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { CheckDaemon } = require('../out/daemon');

const BINARY = path.join(__dirname, '..', '..', '..', 'target', 'release', process.platform === 'win32' ? 'kestrel.exe' : 'kestrel');
if (!fs.existsSync(BINARY)) {
  console.log('skipped the daemon runs: no release binary at ' + BINARY);
  process.exit(0);
}

const folder = fs.mkdtempSync(path.join(os.tmpdir(), 'kestrel-daemon-'));
const root = folder;
const file = path.join(folder, 'sales.kes');

const source = `import std.kesdas.{Series, DataFrame}

function main() : void or Error with Effects {
    let sales := DataFrame([
        Series(["north", "south"], name = "region"),
        Series([12, 8], name = "units"),
    ])
    let clean := sales.fillNa(0.0)
    let sorted := clean.sortValues(by = "units", ascending = true)?
    print(sorted.head(2))
}
main()?
`;

const logged = [];
const daemon = new CheckDaemon(BINARY, root, {
  timeoutMs: 30000,
  log: text => logged.push(text)
});

async function main() {
  const cold = await daemon.check({ path: file, text: source });
  assert.strictEqual(cold.ok, true);
  assert.deepStrictEqual(cold.diagnostics, [], logged.join('\n'));
  assert(cold.timeMs > 0);

  const warmStart = Date.now();
  const warm = await daemon.check({ path: file, text: `${source}\n// one more line\n` });
  const warmWall = Date.now() - warmStart;
  assert.deepStrictEqual(warm.diagnostics, []);
  // A warm check stays far below the cold standard-library check. The bound is
  // generous so a slow CI machine does not fail a performance test.
  assert(warm.timeMs < 1000, `warm check took ${warm.timeMs} ms`);
  assert(warmWall < 2000, `warm round trip took ${warmWall} ms`);

  // A namespace import also binds its type names unqualified.
  const unqualified = await daemon.check({
    path: file,
    text: 'import std.kesdas as kd\nlet frame : DataFrame = kd.DataFrame([kd.Series([1, 2], name = "v")])\n'
  });
  assert.deepStrictEqual(unqualified.diagnostics, [], 'unqualified type after a namespace import');

  const broken = await daemon.check({ path: file, text: 'let a : String = 1\n' });
  assert.strictEqual(broken.diagnostics.length, 1);
  assert.strictEqual(broken.diagnostics[0].code, 'E_TYPE');
  assert.strictEqual(broken.diagnostics[0].line, 1);
  assert.strictEqual(broken.diagnostics[0].column, 18);
  assert(broken.diagnostics[0].hint.includes('expected type'));

  // Live goals come from the same warm process and the same proof kernel.
  const proof = 'function add(a : Nat, b : Nat) : Nat = match a {\n    Zero => b,\n    Succ(k) => Succ(add(k, b)),\n}\ntheorem addZeroR(n : Nat) : add(n, Zero) = n by {\n    induction n {\n        Zero => { rfl }\n        Succ(k) ih => { simp [ih] }\n    }\n}\n';
  const goals = await daemon.goals({ path: file, text: proof, offset: proof.indexOf('simp [ih]') });
  assert.strictEqual(goals.ok, true);
  assert.strictEqual(goals.theorem, 'addZeroR');
  assert.strictEqual(goals.goals.length, 1);
  assert.strictEqual(goals.goals[0].target, 'add(Succ(k), 0) = Succ(k)');
  assert.deepStrictEqual(goals.goals[0].variables, [['k', 'Nat']]);
  assert.deepStrictEqual(goals.goals[0].hypotheses, [['ih', 'add(k, 0) = k']]);

  // The latest request wins while a check is running; the replaced caller is
  // told to drop its result rather than being left waiting forever.
  const first = daemon.check({ path: file, text: source });
  const second = daemon.check({ path: file, text: 'let a : String = "fine"\n' });
  const results = await Promise.all([first, second]);
  assert(results[0].cancelled === true || results[0].ok === true);
  assert.deepStrictEqual(results[1].diagnostics, []);

  daemon.dispose();
  console.log('kestrel daemon tests pass');
}

main().catch(error => {
  daemon.dispose();
  console.error(error);
  process.exitCode = 1;
});
