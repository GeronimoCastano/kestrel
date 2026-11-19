# mathlib

A checked library of basic mathematics for Kestrel, in the spirit of Mathlib for
Lean. The kernel checks every theorem; nothing here uses `sorry`, `axiom`,
`opaque` or `smt`.

The library also tests the kernel. Each file targets the fragment the kernel can
check, so a lemma that resists shows exactly where the checker still falls short.

## What it proves

| File | Contents | Theorems |
|---|---|---:|
| `logic.kes` | propositional logic over `Prop` parameters: conjunction, disjunction, negation, implication, iff, quantifiers | 36 |
| `set.kes` | proof-level sets: subset, union, intersection, complement, families, builder, finite sets | 36 |
| `nat.kes` | addition, multiplication, exponentiation, truncated subtraction, predecessor, Boolean order, cancellation, min and max | 35 |
| `order.kes` | propositional `<=` and `<` on `Nat`, monotonicity, cancellation, and the `subNat` characterization | 13 |
| `algebra.kes` | law dictionaries for semigroups, monoids, groups and rings, powers and inverses | 11 |
| `list.kes` | append, length, reversal, map, filter, fold, sum, concatenation, counting | 27 |
| `relation.kes` | named reflexivity, symmetry and transitivity; classes; partial orders; equivalence | 7 |
| `divisibility.kes` | the named predicate `dvd`, its bounds and order laws, and `prime` | 12 |
| `instances.kes` | audit instances: `Nat` addition and multiplication monoids, the set lattice | 3 |
| `z2.kes` | the two-element field Z2: definitions, fourteen ring laws and their conjunction | 15 |
| `showcase.kes` | acceptance examples that cite the library | examples |

195 theorems and 23 functions. The checker reads the whole library in under three seconds.

## Build and check

```sh
cargo build
./tools/check_mathlib.sh
```

The script checks every file, rejects an admitted proof in `showcase.kes` and
rejects a banned construct. `cargo test` also checks the library and the kernel
regression tests in `tests/kernel_fixes.rs`.

## A proof

```kestrel
public theorem appendAssoc[T](xs : List[T], ys : List[T], zs : List[T]) :
        (xs + ys) + zs = xs + (ys + zs) by {
    induction xs {
        [] => { rfl }
        [x, ..rest] ih => { simp [ih] }
    }
}
```

The reverse direction of a rewrite is written `←`:

```kestrel
public theorem singletonSubset[U](a : U, S : Set[U]) :
        ({a} ⊆ S) ↔ a ∈ S by {
    constructor
    { intro h
      apply h
      rfl }
    { intro h
      intro x
      intro hx
      have hxa : x = a := hx
      rw [hxa]
      exact h }
}
```

Named predicates are ordinary definitions. The kernel unfolds them in a
statement, so the proof reads like the definition:

```kestrel
public function dvd(d : Nat, n : Nat) : Prop = exists k : Nat, n = d * k

public theorem dvdTrans(a : Nat, b : Nat, c : Nat) :
        dvd(a, b) -> dvd(b, c) -> dvd(a, c) by {
    intro h1
    intro h2
    cases h1
    cases h2
    exists k * k1
    trans (b * k1)
    { exact h2w }
    trans ((a * k) * k1)
    { have h := congrArg(x => x * k1, h1w)
      exact h }
    { exact mulAssoc(a, k, k1) }
}
```

Strong induction and calc are tactics:

```kestrel
strongInduction n {
    Zero => { rfl }
    Succ(k) ih => {
        have hk := ih(k, rfl)
        rw [hk]
        rfl
    }
}
```

```kestrel
calc a = b := h1
     _ = c := h2
```

## Reading order

`logic.kes`, `set.kes`, `nat.kes`, `order.kes`, `algebra.kes`, `list.kes`,
`relation.kes`, `divisibility.kes`, `instances.kes`, `z2.kes`, `showcase.kes`.

## What is not here

Division, gcd and Bezout still wait for a definition the kernel can unfold.
Their natural definitions recurse on `subNat(a, b)`, which is smaller but not a
structural piece of a match, so the kernel marks the function opaque. Strong
induction proves properties about such a function only after the definition
itself gets a `decreases` measure.

Recursive predicates wait for proposition-mode match arms. `Int` order waits
for a kernel decision, because `Int` has no induction principle and this
library does not use `smt`.
