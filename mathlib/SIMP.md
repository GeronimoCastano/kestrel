# The `@simp` register

Marking a theorem `@simp` adds it to the default rule set, so plain `simp` and
every `simp [...]` may rewrite with it. Only an equality can carry the mark; the
checker refuses `@simp` on an iff.

The rule must be orientable, terminating, and safe in every context. If two
rules rewrite into each other, the checker stops at `E_PROOF_LIMIT`.

| Rule | Orientation | Why it is safe |
|---|---|---|
| `addZeroR` | `n + 0` → `n` | The right side is smaller and needs no assumption. |
| `addSuccR` | `n + Succ(m)` → `Succ(n + m)` | Peels a constructor; smaller. |
| `mulZeroR` | `n * 0` → `0` | The left side is smaller; no assumption. |
| `mulOneR` | `n * 1` → `n` | The right side is smaller; no assumption. |

Not in the register, on purpose:

- `addComm`, `mulComm`, `unionComm`, `interComm`: a commutation law rewrites
  into its own instance and loops. Use `rw [addComm(a, b)]` or `have`.
- `addAssoc`, `mulAssoc`, `appendAssoc`: orientable one way, but the library
  needs both directions; a global rule fires where the proof wanted the other
  orientation. Cite it: `simp [addAssoc]` or `rw [← addAssoc]`.
- Any theorem with an assumption. Plain `simp` would have to discharge the
  assumption at every occurrence.
- Injectivity laws. They are not oriented toward smaller terms.

Adding a rule: prove the equality, mark it `@simp`, run
`./tools/check_mathlib.sh`, and add a row here. If a proof that used an explicit
`rw` now closes with plain `simp`, the rule is earning its place.
