'use strict';
const assert = require('assert');
const fs = require('fs');
const path = require('path');
const os = require('os');
const { spawnSync } = require('child_process');
const { CheckDaemon } = require('../out/daemon');
// The `smt` tactic needs a solver. Without one the proofs stay incomplete, so
// the trust markers this test checks never appear.
if (spawnSync('z3', ['-version']).status !== 0) {
  console.log('no z3 on PATH; skipping the SMT trust test');
  return;
}
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'kestrel-editor-goals-'));
const compiler = path.resolve(__dirname, '../../../target/release/kestrel');
const daemon = new CheckDaemon(compiler, root);
(async () => {
  try {
    const text = 'theorem linear(n:Int):n+1>n by {smt}\ntheorem nonlinear(n:Int):n*n>=0 by {smt}\n';
    for (const [name, expected] of [['linear', false], ['nonlinear', true]]) {
      const start = text.indexOf(`theorem ${name}`);
      const offset = text.indexOf('smt', start) + 3;
      const result = await daemon.goals({path:path.join(root,'proof.kes'),text,offset});
      assert.strictEqual(result.theorem,name);
      assert.strictEqual(result.complete,true);
      assert.strictEqual(result.trusted,expected,name);
      assert.strictEqual(result.trustReasons.some(reason=>reason.startsWith('smt')),expected);
    }
    const text2 = 'enum Tree {Leaf, Add(Tree,Tree)}\ntheorem same(t:Tree):t=t by {induction t {Leaf => {rfl} Add(a,b) iha ihb => {rfl}}}';
    const result = await daemon.goals({path:path.join(root,'proof.kes'),text:text2,offset:text2.lastIndexOf('rfl')});
    const hypotheses=result.goals.flatMap(goal=>goal.hypotheses.map(([name])=>name));
    assert(hypotheses.includes('iha')&&hypotheses.includes('ihb'),JSON.stringify(result));
    console.log('Real compiler proof-state trust and multi-field induction contexts pass');
  } finally {daemon.dispose();fs.rmSync(root,{recursive:true,force:true});}
})().catch(error=>{console.error(error);process.exitCode=1;});
