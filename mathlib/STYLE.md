# Style

Idioms this library uses. Every example compiles
against the v0.1 kernel.

## 1. A definition matches the argument the proofs reduce first

The kernel unfolds a call only when the matched argument is a constructor. If a
lemma needs `f(a, Zero) = a` by computation, `f` must match on its second
argument.

```kestrel
public function subNat(a : Nat, b : Nat) : Nat = match b {
    Zero => a,
    Succ(k) => match a {
        Zero => Zero,
        Succ(j) => subNat(j, k),
    },
}
```

`subNat(a, Zero)` is now `rfl`. A definition that matched `a` first would need an
induction for the same fact.

Prefer total functions to partial built-ins: `subNat` instead of `-`, `powNat`
instead of `**`.

## 2. A generic function that rebuilds a list needs `Copy`

```kestrel
public function mapList[T : Copy, U : Copy](f : (T) -> U, xs : List[T]) : List[U] = match xs {
    [] => [],
    [y, ..rest] => [f(y)] + mapList(f, rest),
}
```

Theorems about it carry the same bound. A function that only reads the list,
such as `foldList` or `lengthList`, needs no bound.

## 3. Law dictionaries pass laws as parameters

Traits carry no proof fields and the kernel cannot read a trait method, so a
structure is a carrier type, its operations, and one `forall`-typed parameter per
law. One binder per variable: `forall a b : T` does not parse.

```kestrel
public theorem powerAdd[T](op : (T, T) -> T, e : T, x : T,
                           hIdL : forall a : T, op(e, a) = a,
                           hAssoc : forall a : T, forall b : T, forall c : T,
                                    op(op(a, b), c) = op(a, op(b, c)),
                           m : Nat, n : Nat) :
        power(op, e, x, m + n) = op(power(op, e, x, m), power(op, e, x, n)) by {
    induction m {
        Zero => { simp [hIdL] }
        Succ(k) ih => { simp [hAssoc, ih] }
    }
}
```

A law in rule position is instantiated by applying it: `rw [hAssoc(x, y, z)]`.
`apply hIdL` instantiates a premise; bare `exact hIdL` does not.

## 4. Named predicates

Write a predicate as a function that returns `Prop`. The kernel unfolds it in a
statement, so `dvd(d, n)` is the existential it was defined to be.

```kestrel
public function dvd(d : Nat, n : Nat) : Prop = exists k : Nat, n = d * k

public theorem dvdZero(d : Nat) : dvd(d, Zero) by {
    exists Zero
    rw [mulZeroR]
    rfl
}
```

Use a predicate in a hypothesis position, never as a bare theorem parameter.
A parameter `h : reflexive(r)` is a data parameter; state the implication
instead, or write `requires h : reflexive(r)` after the result.

```kestrel
public theorem classReflMem[U](r : (U, U) -> Prop, a : U) :
        reflexive(r) -> a ∈ {x : U | r(a, x)} by {
    intro hRefl
    exact hRefl(a)
}
```

A predicate with a `match` body is not supported yet: the arms parse as value
expressions, not propositions. `dvd` and `prime` are nonrecursive and unfold;
`Sorted` needs the next round.

## 5. Induction

The common shape:

```kestrel
induction xs {
    [] => { rfl }
    [x, ..rest] ih => { simp [ih] }
}
```

The induction hypothesis is stated at the smaller value. If a later variable is
still fixed and the hypothesis must be used at another value, revert it:

```kestrel
induction a generalizing b {
    Zero => { intro b; simp }
    Succ(k) ih => { intro b; ... }
}
```

Use nested `induction` when the inner hypothesis is the statement at the inner
value. The kernel accepts nested induction now. The hypothesis still describes
the outer branch's value, so a nested proof usually needs `generalizing` or the
outer hypothesis.

Strong induction gives the hypothesis at every smaller value:

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

`ih` has type `forall j : Nat, j <= k -> P(j)`. The `Zero` branch takes no
hypothesis. Strong induction is for `Nat`; use `induction` for a list or an
enum.

## 6. Splitting

`cases h` names the parts after the fact:

- `h : P and Q` gives `hl` and `hr`.
- `h : P or Q` gives two goals, one with `hl` and one with `hr`.
- `h : P ↔ Q` gives `hmp` and `hmpr`.
- `h : exists x, P` gives `hw` for the body and a fresh name for the witness.
- `cases h3` gives `h3l`, `h3r`, `h3mp`, `h3mpr`, `h3w`.

A split on an equality between constructors is injectivity: `cases h` on
`Succ(a) = Succ(b)` substitutes one side for the other, and different
constructors close the goal.

To split a projection of a conjunction, write the projection into the tactic:

```kestrel
cases h.left
rfl
```

`cases h.left` and `cases h.right` split a conjunction or an iff. A deeper
projection needs a `have` first.

To split a Bool expression rather than a variable, use `byCases`:

```kestrel
byCases h : p(x)
{ rw [h]; simp [ih] }
{ rw [h]; simp [ih] }
```

The two goals carry `h : p(x) = true` and `h : p(x) = false`.

## 7. Rewriting

`rw [h]` rewrites one occurrence at a time and stops when the goal turns
reflexive. Chain the rules you need; do not expect a rule to fire everywhere at
once unless the goal closes.

```kestrel
rw [interDistribUnion]
rw [unionComm]
rfl
```

Reverse rewriting is `rw [← h]`. A local `forall` hypothesis can be a rule:
`simp [hIdL]`, `rw [hAssoc(x, y, z)]`.

A chain of equalities uses `calc`:

```kestrel
calc a = b := h1
     _ = c := h2
```

Each line is checked against its own equation, and the first and last terms are
checked against the goal. Only equality chains are supported; for an iff chain,
use `trans` or `iffTrans`.

Never mark a commutation law `@simp`: it rewrites into itself. See
[SIMP.md](SIMP.md).

## 8. Set equality

```kestrel
ext
{ intro x
  intro h
  ... }
{ intro x
  intro h
  ... }
```

`ext` produces the two subset goals in that order. Membership of a union,
intersection, complement, family, builder or finite set unfolds one step when a
tactic needs it, so `cases`, `left`, `right`, `constructor`, `exists` and `intro`
work directly.

## 9. Classical steps

`byContra h` proves `P` by assuming `h : not P` and changing the goal to
`false`. `pushNeg` moves negations inward. Both are kernel rules, not axioms.
`notAnd`, `complInter` and `notForall` need one of them; the other De Morgan
laws are constructive.

## 10. Finish

`rfl` proves an equality, an iff, or a comparison that computes to `true`.
`decide` proves a closed proposition. `simp` without a list closes by the
`@simp` register and by assumptions. `done` fails unless every goal is closed.

When a proof reports `simp leaves ...`, read the remaining proposition before
adding rules.
