# Mathematics in Kestrel

Kestrel has two layers for mathematics.

The **runtime layer** computes with natural numbers, integers, reals, complex
numbers, sets, functions, vectors and matrices. The standard modules in the
table below supply the names, and native code runs them.

The **proof layer** checks theorems about natural numbers, integers, booleans,
enums and type variables. The checker erases proofs before code generation. A
`by { ... }` block contains a tactic script; an unmarked `{ ... }` block still
holds an explicit proof term.

```kestrel
function add(a : Nat, b : Nat) : Nat = match a {
    Zero    => b,
    Succ(k) => Succ(add(k, b)),
}

theorem addZeroR(n : Nat) : add(n, Zero) = n by {
    induction n {
        Zero       => { rfl }
        Succ(k) ih => { simp [ih] }
    }
}
```

## Syntax choices

The project requires three options per language feature, scored equally on
correctness, versatility and performance.

### Proof-script marker

| Option | Correctness | Versatility | Performance | Score |
|---|---:|---:|---:|---:|
| `by { ... }` before the block (chosen) | 5 | 5 | 5 | 5.00 |
| `proof { ... }` | 5 | 4 | 5 | 4.67 |
| A bare `{ ... }` always means tactics | 3 | 5 | 5 | 4.33 |

`by` keeps every existing term proof valid. It separates tactic parsing from
ordinary expression parsing, so a name such as `rfl` or `simp` never becomes a
keyword outside a proof. The parser pays one token of lookahead.

### Quantifier syntax

| Option | Correctness | Versatility | Performance | Score |
|---|---:|---:|---:|---:|
| `forall n : Nat, P` and `exists n : Nat, P` (chosen) | 5 | 5 | 5 | 5.00 |
| `forall (n : Nat), P` only | 5 | 4 | 5 | 4.67 |
| `for all n : Nat, P` | 4 | 4 | 5 | 4.33 |

The first spelling matches the rest of Kestrel's `name : Type` notation. The
parenthesized form also parses. `forall` and `exists` only act as quantifiers
inside a theorem statement, where a value cannot start a term.

### Live goal display

| Option | Correctness | Versatility | Performance | Score |
|---|---:|---:|---:|---:|
| A Webview view beside the editor (chosen) | 5 | 5 | 4 | 4.67 |
| Hover text at the cursor | 5 | 2 | 5 | 4.00 |
| A status-bar item | 4 | 2 | 5 | 3.67 |

The view holds multiple goals, hypothesis context, errors and completion. It
redraws after a short debounce, so a burst of typing costs one checker request.
The compiler still does all checking; the webview only formats the reply.

## Logic

Statements combine:

```text
P and Q          P or Q          not P
P -> Q           forall n : Nat, P          exists n : Nat, P
true             false
x = y            a < b            bool expressions
```

`->` is right-associative and binds looser than `and` and `or`. A quantifier
body extends as far right as possible. The kernel supports `Nat`, `Int`, `Bool`,
enum types and the theorem's own type variables as quantified types.

Proof terms for the connectives:

| Goal | Term |
|---|---|
| `P and Q` | `(p, q)` |
| `P or Q` | `Left(p)` or `Right(q)` |
| `not P` | `h => proof of false` |
| `P -> Q` | `h => proof of Q` |
| `forall n : Nat, P` | `n => proof of P` |
| `exists n : Nat, P` | `exists(witness, proof)` |
| `true` | `decide` |

Statements and proofs may name only pure functions that the kernel can unfold.
Everything else is a test, not a theorem.

## Sets and subsets

Proof-level sets use the existing type `Set[U]`, where `U` is a type variable.
A theorem parameter can be a set:

```kestrel
theorem subsetRefl[U](A : Set[U]) : A ⊆ A by {
    intro x
    intro h
    exact h
}
```

`x ∈ A` is a proposition. `A ⊆ B` is stored and printed as written. A tactic
that needs the definition unfolds it to
`forall x : U, x ∈ A -> x ∈ B`, so `intro`, `apply`, `have` and `exact` work
as they do for any quantified implication. The proof state keeps the surface
form `A ⊆ B`.

```kestrel
theorem subsetTrans[U](A : Set[U], B : Set[U], C : Set[U],
                      h1 : A ⊆ B, h2 : B ⊆ C) : A ⊆ C by {
    intro x
    intro h3
    have h4 := h1(h3)
    exact h2(h4)
}
```

Consecutive parameters of one type may share the annotation:
`theorem setPair(x y : U, A : Set[U], hx : x ∈ A, hy : y ∈ A)`. This is
sugar for writing `x : U, y : U` separately.

The theorem parameter list accepts hypotheses, not only data parameters. In the
example above `h1` and `h2` are proof parameters; they are checked as assumptions
and do not appear in citations, which supply only `A`, `B` and `C`.

### Union, intersection and complement

`A ∪ B`, `A ∩ B` and `Aᶜ` are sets. Membership is stored and printed as
written. When a tactic needs the outer definition, it unfolds one step:

- `x ∈ A ∪ B` becomes `x ∈ A or x ∈ B`.
- `x ∈ A ∩ B` becomes `x ∈ A and x ∈ B`.
- `x ∈ Aᶜ` becomes `not (x ∈ A)`.
- `x ∉ A` is `not (x ∈ A)`.

The usual logical tactics therefore work directly: `left`/`right` for a union,
`constructor` for an intersection, and `intro` for a complement. The children
stay in their written form, so a goal does not grow until the proof needs it.

```kestrel
theorem memUnion[U](x : U, A : Set[U], B : Set[U], h : x ∈ A) : x ∈ A ∪ B by {
    left
    exact h
}

theorem memCompl[U](A : Set[U], x : U) : x ∈ Aᶜ ↔ x ∉ A by {
    rfl
}
```

`ext` proves a set equality by extensionality: it replaces `A = B` with the two
goals `A ⊆ B` and `B ⊆ A`. `byContra h` is the classical rule: to prove `P`,
it assumes `h : not P` and changes the goal to `false`. It is needed for results such as `Aᶜᶜ = A` and
the De Morgan law for intersection. `pushNeg` rewrites negations inward when
that is more convenient, for example `not (not (x ∈ A))` becomes `x ∈ A`.

```kestrel
theorem complCompl[U](A : Set[U]) : Aᶜᶜ = A by {
    ext
    intro x h
    byContra hx
    exact h(hx)
    intro x hx
    intro h
    exact h(hx)
}
```

`testdata/language/sets.kes` reproduces the six levels of the Subset world in Velleman's
Set Theory Game. `kestrel game` serves them as a **Sets** world with the same
goal display. Membership here is a proof relation over an abstract set. It is
unrelated to the runtime hash set's `contains`, which remains a computation.

### Family sets, set-builder and finite sets

A family is a set of sets. `⋃₀ F` is its union and `⋂₀ F` its intersection;
`\union0` and `\inter0` are the keyboard spellings. The operator expects
`F : Set[Set[U]]` and produces a `Set[U]`:

```kestrel
theorem familyUnionIntro[U](F : Set[Set[U]], A : Set[U], hF : A ∈ F,
                            x : U, hx : x ∈ A) : x ∈ ⋃₀ F by {
    exists A
    constructor
    exact hF
    exact hx
}
```

A set can also be written from a predicate or listed directly. The builder
`{x : U | P}` holds the values that satisfy `P`; the finite forms are `{a}`,
`{a, b}` and the empty set `∅` (`\empty` or `\emptyset`). The empty set takes its universe
from the expected type, as in `x ∈ ∅`.

Membership is stored and printed as written, and a tactic that needs the
definition unfolds one step:

- `x ∈ ⋃₀ F` becomes `exists A : Set[U], A ∈ F and x ∈ A`.
- `x ∈ ⋂₀ F` becomes `forall A : Set[U], A ∈ F -> x ∈ A`.
- `x ∈ {y : U | P}` becomes `P` with `y` replaced by `x`.
- `x ∈ {a}` becomes `x = a`; `x ∈ {a, b}` becomes `x = a or x = b`; and
  `x ∈ ∅` becomes `false`.

`intro`, `cases`, `exists`, `constructor`, `left`, `right`, `apply`, `simp`
and `rw` therefore work on families exactly as they do on the logical forms
above. `ext` still proves a set equality. Quantifiers may bind over sets:
`forall A : Set[U], P` and `exists A : Set[U], P` check, including
`Set[Set[U]]`.

```kestrel
theorem familyInterSubset[U](F : Set[Set[U]], A : Set[U], hF : A ∈ F) :
        ⋂₀ F ⊆ A by {
    intro x
    intro hx
    apply hx
    exact hF
}

theorem builderSingleton[U](a : U) : {x : U | x = a} = {a} by {
    ext
    intro x
    intro h
    exact h
    intro x
    intro h
    exact h
}
```

[families.kes](testdata/language/families.kes) proves the monotonicity and
pair laws, one De Morgan law for families, and the singleton theorem.

### Code values in sets

A set is not a separate mathematical universe. A class value can be an element,
and a class type can be the universe of a set:

```kestrel
class Dog {
    public final name : String
    new(name : String) { this.name = name }
}

theorem dogInSet(d : Dog, A : Set[Dog], h : d ∈ A) : d ∈ A by {
    exact h
}
```

The kernel treats a class value as an opaque element. It checks membership and
subset, but it does not inspect the object's fields or call its methods. A
generic theorem such as `A ⊆ A` works at `Set[Dog]` without a separate proof.
[classsets.kes](testdata/language/classsets.kes) is the runnable version.

### `sorry`

`sorry` closes any goal and emits `W_PROOF_SORRY`. It is useful for a proof
skeleton:

```kestrel
theorem sketch(n : Nat) : n = Zero by {
    sorry
}
```

A theorem that uses `sorry` is admitted. It can be cited while drafting, and
a theorem that cites it also becomes admitted, but no function may `requires`
an admitted theorem. The checker still exits successfully, so a sketch can sit
in a file while you fill it in. The proof-state view marks an admitted proof
with a `sorry` badge instead of `proved`.

## Typing math without a symbol keyboard

Membership and subset have no word spellings. Write the backslash form or paste
the glyph:

```kestrel
theorem a[U](x : U, A : Set[U]) : x ∈ A -> x ∈ A by { intro h; exact h }
theorem b[U](x : U, A : Set[U]) : x \in A -> x \in A by { intro h; exact h }
```

`\in` and `\subseteq` are the portable spellings for `∈` and `⊆`. The goal
view always prints the Unicode form. `forall` and `exists` keep their word forms
because they are language keywords, and `forall` and `\forall` mean the same
binder. The arrow can be written `->` or `\to`. Comparisons accept `>=` or
`\ge`, `<=` or `\le`, and `!=` or `\ne` in proof statements.

The VS Code extension expands a backslash name when you type a space:
`x \in A` becomes `x ∈ A`. Turn `kestrel.expandSymbols` off to keep the
backslash spelling. The parser accepts either form, so an editor without the
extension still works.

Immutable classes and records with readable constructors support field and pure
method expressions in theorems, with no extra marker. Subclass overrides of a
method used by a theorem are refused across the whole checked program. The same
reading applies to theorem citations, simplification and executable proof
arguments; see `testdata/language/classtheorems.kes`.

A class value can be an element of a proof-level set;
[classsets.kes](testdata/language/classsets.kes) proves a membership fact about
`Dog` objects.

## Operator aliases

An operator declaration maps a spelling to a function. It needs no proof:

```kestrel
function add(a : Nat, b : Nat) : Nat = a + b

infixl 10 "⊕" as \oplus = add

theorem twoPlusThree() : 2 ⊕ 3 = 2 \oplus 3 by {
    rfl
}
```

`infixl` and `infixr` choose associativity. The number is the binding power,
from 1 (loosest) to 13 (tightest), matching the compiler's operator table. A
use becomes an ordinary call to `add`, so the checker and kernel typecheck it
like any other call. The declaration is file-local in this version; imported
operator tables are not visible to an importing file yet.

The supported declaration forms are:

| Form | Grouping |
|---|---|
| `infixl 10 "⊕" = add` | `a ⊕ b ⊕ c` means `(a ⊕ b) ⊕ c` |
| `infixr 10 "⊕" = add` | `a ⊕ b ⊕ c` means `a ⊕ (b ⊕ c)` |
| `infix 10 "⊕" = add` | Left grouping; chaining is accepted for now |

`testdata/language/operators.kes` is the runnable version.

## Rewriting `add(a, b)` to `a + b`

A checked `@simp` theorem is a rewrite rule, which is the metaprogramming hook
for a function such as `add`:

```kestrel
@simp
theorem addEq(a : Nat, b : Nat) : add(a, b) = a + b by {
    induction a {
        Zero       => { rfl }
        Succ(k) ih => { simp [ih] }
    }
}

theorem auto(n : Nat, m : Nat) : add(n, m) = n + m by {
    simp
}
```

The kernel normalizes built-in natural addition and multiplication by the same
structural steps as `Zero` and `Succ`, so the induction proof goes through.
Once `addEq` is marked `@simp`, plain `simp` rewrites `add(a, b)` everywhere. A
rule with assumptions is applied only where those assumptions hold.

In a proof, `Int` is the integers and `Nat` the natural numbers, so neither
theorem needs an overflow bound. Only a Nat subtraction below `Zero` and a
division by zero stay undefined and need an assumption. The 64-bit limits come
back in one place: a function that requires a theorem passes its facts to
run-time code, so its callers must establish that theorem's assumptions.

The same mechanism works for any checked equation. Mark it `@simp` to add it to
the default rule set, or cite it as `simp [name]` for one proof. Put `←` before
a rule to use it from right to left: `rw [← h]` turns `x ∈ A` into `x ∈ s`
when `h : s = A`, and `simp [← h]` does the same inside the simplifier.
`testdata/language/rewrite.kes` is the runnable version.

## Axioms and examples

`example` is an unnamed theorem. The checker proves it and forgets it:

```kestrel
example[U](A : Set[U]) : A ⊆ A by {
    intro x
    intro h
    exact h
}
```

An example cannot be cited, so it does not add a name to the environment.
`example` and `axiom` are reserved words now.

`axiom` is a named statement accepted without proof:

```kestrel
axiom allSetsInhabited[U](A : Set[U]) : ∃ x : U, x ∈ A

theorem useAxiom[U](A : Set[U]) : ∃ x : U, x ∈ A by {
    exact allSetsInhabited(A)
}
```

The checker reports `W_PROOF_AXIOM`. An axiom is admitted, like a theorem
proved with `sorry`, so a function cannot use it to remove a run-time check.
[axioms.kes](testdata/language/axioms.kes) has both forms.

## Tactics

A tactic acts on the first goal. Branches and `{ ... }` bullets keep the order
Lean users expect.

| Tactic | Effect |
|---|---|
| `intro h` | Introduces a `forall` variable, an implication premise or a `not` premise. |
| `intros` | Repeats `intro` with generated names until the goal is not a quantifier or implication. |
| `exact p` | Closes the goal with the proof term `p`, checked by the kernel. `p` may be a projection such as `h.left`. |
| `assumption` | Closes the goal with a hypothesis of the same proposition. |
| `apply thm` / `apply h` | Matches a theorem's conclusion against the goal and makes its premises new goals. Arguments may be explicit: `apply addZeroR(n)`. |
| `ext` | Proves a set equality `A = B` through the two subset goals `A ⊆ B` and `B ⊆ A`. |
| `byContra h` | Classical proof by contradiction: assumes `h : not P` and changes the goal `P` to `false`. |
| `pushNeg` | Pushes negations inward: `not (not P)` becomes `P`, `not (P and Q)` becomes `not P or not Q`, `not (P or Q)` becomes `not P and not Q`, and negations move through `->`, `forall` and `exists`. |
| `constructor` | Splits `P and Q` into two goals, or `P ↔ Q` into the two implications; closes `true`. |
| `left` / `right` | Proves one side of `P or Q`. |
| `exists w` | Proves `exists x, P` with the witness `w`. `use` is the same tactic. |
| `cases h` | Splits an `and`, `or`, `iff` or `exists` hypothesis. The new names come from the split fact: `cases h3` on an `and` adds `h3l` and `h3r`; `or` gives two goals, one with `h3l` and one with `h3r`; `iff` adds `h3mp` and `h3mpr`; `exists` adds `h3w`. `cases h` still gives `hl`, `hr`, `hmp`/`hmpr` and `hw`. |
| `cases n { ... }` | Splits a `Nat` or enum into one tactic block per constructor. |
| `induction n { ... }` | Splits a value and adds one hypothesis per direct recursive field. Write `Succ(k) ih => { ... }` or `Add(a, b) iha ihb => { ... }`. |
| `simp` / `simp [rules]` | Rewrites with `@simp` theorems, named theorems, local equalities and `↔` facts, then closes the goal. It handles equality goals and logical goals, and closes by a hypothesis when the rewritten goal matches one. A rule written `← h` is used from right to left. |
| `rw [rules]` | Rewrites the goal with equalities and `↔` facts, without requiring it to close. A rule written `← h` is used from right to left. |
| `have h := p` | Adds a checked local proof and infers its statement from `p`. |
| `have h : P := p` | Adds a checked local proof with an explicit statement. |
| `have h : P by { ... }` | Proves `P` in a nested script and adds it as a hypothesis. |
| `symm` | Swaps an equality goal. |
| `trans m` | Replaces `a = c` with `a = m` and `m = c`. |
| `exfalso` | Replaces the goal with `false`, keeping the hypotheses. |
| `contradiction` | Closes a goal from a false hypothesis or an equation between different constructors. |
| `rfl` | Closes an equality or an iff whose sides reduce to the same term. |
| `decide` | Computes a closed proposition. |
| `trivial` | Tries reflexivity, `true`, a hypothesis and structural conjunction/disjunction. |
| `smt` | Discharges a quantifier-free integer arithmetic goal with Z3; the kernel checks supported linear certificates. `kestrel check --trusted FILE` lists remaining solver results and admitted proofs. It rejects a counterexample and a Nat subtraction that may go below zero. Int and Nat have no upper limit in a proof. |
| `sorry` | Closes the goal with an admitted proof and a warning. |
| `skip` | Does nothing. |
| `done` | Fails unless every goal is closed. |
| `repeat { ... }` | Repeats a block until it makes no progress or every goal closes. |
| `try { ... }` | Runs a block and restores the state if it fails. |
| `{ ... }` | A bullet: runs the block on the current goal. |

The proof must close every goal. An unfinished script reports
`E_PROOF_INCOMPLETE` with the remaining target. A tactic used on the wrong shape
reports `E_PROOF_SHAPE`; a conclusion mismatch reports `E_PROOF_MISMATCH`.

### Proof constructors

An introduction rule builds a proof of a connective from proofs of its parts.
These terms apply one directly, without a `constructor` tactic:

| Term | Goal |
|---|---|
| `And(p, q)` | `P and Q` |
| `OrLeft(p)` / `OrRight(q)` | `P or Q` |
| `Iff(f, g)` | `P ↔ Q`, with `f : P -> Q` and `g : Q -> P` |
| `Exists(w, p)` | `exists x, P`, with witness `w` |
| `True` | `true` |

A constructor is a proof term, so it appears under `exact`, in a theorem body or
in a stated `have`:

```kestrel
theorem interIntro[U](x : U, A B : Set[U], h1 : x ∈ A, h2 : x ∈ B) : x ∈ A ∩ B {
    And(h1, h2)
}

theorem viaExact[U](x : U, A B : Set[U], h1 : x ∈ A, h2 : x ∈ B) : x ∈ A ∩ B by {
    exact And(h1, h2)
}
```

`have h := And(h1, h2)` infers the statement. `OrLeft`, `OrRight` and `exists`
need a stated type because the argument determines only one part of the
proposition; write `have h : x ∈ A ∪ B := OrLeft(h1)`. `Iff(f, g)` infers when
`f` and `g` are named proofs.

The old spellings stay: `(p, q)`, `Left(p)`, `Right(q)` and `exists(w, p)`.

Implication, `forall`, `not` and subset keep a lambda as their introduction
rule. `rfl` introduces an equality, and `false` has no introduction rule.

### Quantifier elimination

```kestrel
theorem fromExists(n : Nat) : (exists m : Nat, m = m) -> n = n by {
    intro h
    cases h          // hw : w = w for a fresh w
    rfl
}
```

`cases` on `exists` introduces a fresh witness. `cases` on `and` names its two
parts after the fact: `cases h` gives `hl` and `hr`, and `cases h3` gives `h3l`
and `h3r`. `cases` on `or` produces two goals, one with `<name>l` and one with `<name>r`.

### Conjunction projections

An `and` fact has two sides, and either one is a proof term on its own. No case
split is needed when the proof uses only one of them:

```kestrel
theorem leftOf(n : Nat) : n = n by {
    have h : n = n and n = n := (rfl, rfl)
    exact(h.left)          // h.right proves the other side
}
```

Projections chain, so `h.right.left` takes the left side of the conjunction on
the right. A theorem application projects the same way, as in `pair(n).left`.
The base unfolds one definitional step first: a membership in an intersection
is an `and`, so `h.left` on `h : x ∈ A ∩ B` proves `x ∈ A`.

```kestrel
theorem interLeft[U](x : U, A B : Set[U], h : x ∈ A ∩ B) : x ∈ A by {
    exact(h.left)
}
```

`apply(h.left)` treats a projected implication like a hypothesis, and
`h.left(x)` applies it to an argument. A disjunction has no recoverable side,
because `P or Q` does not say which side holds: `h.left` on an `or` reports
`E_PROOF_SHAPE`, and `cases h` stays the eliminator.

## Live goals in the editor

`kestrel serve` answers a `goals` request with the state at a UTF-16 offset in
the buffer:

```json
{"id":1,"op":"goals","path":"/work/nat.kes","text":"...","offset":123}
```

```json
{"id":1,"ok":true,"theorem":"addZeroR",
 "goals":[{"variables":[["k","Nat"]],
           "hypotheses":[["ih","add(k, 0) = k"]],
           "target":"add(Succ(k), 0) = Succ(k)"}],
 "error":null,"complete":false}
```

The same report is available from the command line:

```sh
kestrel goals FILE --line 8 --column 27
kestrel goals FILE --line 8 --column 27 --json
```

The line and column are one-based. The VS Code extension opens a **Kestrel Proof
State** view in the activity bar. Put the cursor in a theorem and the view shows
the variables, hypotheses and target for that point. A green `proved` badge
appears when the script closes every goal. The view escapes all compiler text
before rendering it.

### Typing while the proof is incomplete

The goal state stays visible while a tactic line is half written. An unknown
tactic, a missing argument, or an unclosed `by {` block reports an error beside
the goal instead of removing the panel. The compiler also keeps the last usable
proof state when a parse error prevents the header from being checked, and marks
it as stale.

### Case goals

`cases` and `induction` with branches create one goal per branch. The proof
state lists every branch under **Cases**, marks the branch under the cursor,
and shows that branch's goal underneath. Click a branch to move the cursor into
it. The compiler still checks every branch before the theorem passes.

`cases h` on an `and`, `or` or `exists` assumption without branch blocks puts
the resulting goals directly in the goal list.

## The game

`kestrel game` serves a browser game on `127.0.0.1:7878`. Each level is a real
Kestrel file with one proof hole; the browser sends the submitted script back to
the same kernel that checks a file. The page shows the live goal state, hints,
the level list and saved progress. It needs no network access and no build step.

```sh
cargo run -- game --port 7878
```

The levels cover reflexivity, conjunction, implication, symmetry, induction,
disjunction, existentials, universals, contradiction, case analysis and
rewriting. A **Sets** world adds the six Subset World levels: exact membership,
applying a subset proof, `have`, implication, reflexivity and transitivity.
Those six start with an empty proof so the goal is visible; the solved proofs
are in `testdata/language/sets.kes`. Use `sorry` only when you want to move on with a
sketch. `src/game.rs` holds them, and `src/game.html` is the client.

## Standard modules

| Module | Contents |
|---|---|
| `std.nat` | Structural `plus` and `times`, with checked `plusZeroR`, `plusSuccR` and `timesZeroR`. |
| `std.set` | `isEmpty`, `member`, `subset`, `properSubset`, `disjoint`, `cardinality` over the built-in `Set[T]`. |
| `std.func` | `identity`, `compose`, `constant`. |
| `std.real` | `Real = double`, `PI`, `E`, `abs`, `sqrt`, `min`, `max`, `isFinite`. |
| `std.algebra` | `Group` and `Ring` traits, a `Z2Group` instance, finite `Z/2Z`, and checked associativity, identity, inverse and distributivity laws. |
| `std.complex` | The existing double-precision complex class. |
| `std.linalg` | The existing dot, norms, solves, decompositions and matrix operations over `Tensor`. |
| `std.math` | The existing scalar functions and constants. |

`testdata/language/math.kes` exercises the first five modules together.

## Kernel scope

The proof kernel reads `Nat`, `Int`, `Bool`, enums, `Set[U]` and type variables.
It unfolds pure, structurally recursive functions. Members of `Set[U]` are
proof objects with `∈`, `⊆`, `∪`, `∩` and `ᶜ`; the runtime hash set's
`contains` is a separate computation. Functions, tensors, matrices and complex numbers remain values,
not proof objects yet.

`Real` is IEEE-754 `double`. It is not an exact real number. The game and the
examples say so instead of pretending otherwise.

`Group` and `Ring` describe the operations an instance provides. Kestrel does
not attach law obligations to a trait yet, so a `Group` implementation must
prove its laws as separate theorems. `std.algebra` does that for `Z/2Z`.

## Proposition parameters and opaque theories

A theorem parameter of type `Prop` is an opaque proposition. The connectives
work on it like on any other proposition, so the propositional-logic worlds are
statements over parameters:

```kestrel
theorem identity(P : Prop) : P -> P by {
    intro h
    exact h
}

theorem contrapositive(P : Prop, Q : Prop) : (P -> Q) -> (not Q -> not P) by {
    intro h
    intro hn
    intro hp
    have hq := h(hp)
    exact hn(hq)
}
```

`exact P` is refused: a proposition parameter is not a proof of itself. A
theorem that quantifies over a proposition can be cited at another one, as in
`exact identity(Q, h)`; `apply identity` also works when the goal is an
implication with the same shape. The instantiation may be a compound
proposition, so `apply identity` proves `(A and B) -> (A and B)` and an opaque
predicate can take one:

```kestrel
opaque function says(a : Int, p : Prop) : Prop
axiom truth(a : Int, p : Prop) : says(a, p) -> p

theorem both(a : Int, A : Prop, B : Prop) : says(a, A and B) -> (A and B) by {
    intro h
    apply truth(a)
    exact h
}
```

A theorem may quantify over propositions directly: `forall P : Prop, P -> P`
is proved with `intro P` and applies at any proposition.

An `opaque` declaration names a symbol the kernel never unfolds. Its type is
checked, it may return `Prop`, and `axiom` statements give its laws:

```kestrel
opaque type Point
opaque function between(a : Point, b : Point, c : Point) : Prop

axiom betwSym(a : Point, b : Point, c : Point) : between(a, b, c) -> between(c, b, a)

theorem reverse(a : Point, b : Point, c : Point) : between(a, b, c) -> between(c, b, a) by {
    intro h
    apply betwSym
    exact h
}
```

`opaque function` may take type parameters and `Set` arguments, so a theory can
state `line(A, B) : Set[Point]` and membership axioms. `opaque` declarations
have no run-time representation: a call inside an ordinary function reports
`E_OPAQUE_BODY`. Axioms are admitted, so a theorem that relies on one is
reported as admitted and cannot satisfy a function's `requires`.

## Functions as proof values

A theorem may take a function as a parameter or quantify over one. A function
parameter is applied directly, and a lambda may stand where a function is
expected:

```kestrel
theorem pointwise() : forall f : Nat -> Nat, forall n : Nat, f(n) = f(n) by {
    intro f
    intro n
    rfl
}

opaque function isMax(f : (Int, Int) -> Int) : Prop
function max2(a : Int, b : Int) : Int = if a < b { b } else { a }

axiom maxIsMax() : isMax((a : Int, b : Int) => max2(a, b))

theorem squareIsMax() : isMax((a : Int, b : Int) => max2(a, b)) by {
    apply maxIsMax
}
```

Application is ordinary term syntax: `f(n)`, `f(n, m)`, and `f(n)(m)` for a
curried type. `rfl` beta-reduces `((x : Nat) => x + 1)(n)` to `n + 1`. Function
types ignore parameter names, so `(x : Nat) -> Nat` and `(Nat) -> Nat` are the
same type. The kernel does not prove `f = g` from pointwise equality; add
`funext` as an axiom in a theory when a game needs it. A function parameter has
no run-time representation, so it exists only in proofs.

A sequence is a function from `Nat`, which is enough for limits:

```kestrel
opaque type Real
opaque function converges(f : Nat -> Real, L : Real) : Prop
opaque function limit(f : Nat -> Real) : Real
axiom limitSpec(f : Nat -> Real) : converges(f, limit(f))

theorem limitConverges(f : Nat -> Real) : converges(f, limit(f)) by {
    apply limitSpec
}
```
