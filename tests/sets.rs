//! Proof-level sets, subset notation, `sorry` sketches and Nat rewrite rules.
use kestrel::{checker::check, parse_file, proof::goals_at};

fn checked(source: &str) -> kestrel::checker::Checked {
    let parsed = parse_file("sets.kes", source);
    assert!(parsed.is_ok(), "{source}\n{:#?}", parsed.diagnostics);
    check(&parsed)
}

fn good(source: &str) {
    let result = checked(source);
    assert!(result.is_ok(), "{source}\n{:#?}", result.diagnostics);
}

#[test]
fn subset_world_example_checks() {
    good(include_str!("../testdata/language/sets.kes"));
}

#[test]
fn class_values_can_be_set_elements() {
    good(include_str!("../testdata/language/classsets.kes"));
}

#[test]
fn rewrite_example_checks() {
    good(include_str!("../testdata/language/rewrite.kes"));
}

#[test]
fn sketch_example_warns_but_succeeds() {
    let result = checked(include_str!("../testdata/language/sketch.kes"));
    assert!(result.is_ok(), "{:#?}", result.diagnostics);
    assert!(result.diagnostics.iter().any(|d| d.code == "W_PROOF_SORRY"));
}

#[test]
fn membership_and_subset_are_first_class_propositions() {
    good(
        "theorem setExact[U](x : U, A : Set[U], h : x ∈ A) : x ∈ A by { exact h }\n\
         theorem setApply[U](x : U, A : Set[U], B : Set[U], h1 : A ⊆ B, h2 : x ∈ A) : x ∈ B by { exact h1(h2) }\n\
         theorem setTrans[U](A : Set[U], B : Set[U], C : Set[U], h1 : A ⊆ B, h2 : B ⊆ C) : A ⊆ C by {\n\
            intro x\n\
            intro h3\n\
            have h4 : x ∈ B := h1(h3)\n\
            exact h2(h4)\n\
         }\n",
    );
}

#[test]
fn subset_theorems_can_be_cited_alpha_equivalently() {
    good(
        "theorem subsetRefl[U](A : Set[U]) : A ⊆ A by { intro x; intro h; exact h }\n\
         theorem useRefl[U](A : Set[U]) : A ⊆ A by { exact subsetRefl(A) }\n",
    );
}

#[test]
fn sorry_is_a_warning_and_theorem_remains_admitted() {
    let result = checked("theorem guessed(n : Nat) : n = Zero by { sorry }\n");
    assert!(result.is_ok(), "{:#?}", result.diagnostics);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "W_PROOF_SORRY"
                && d.severity == kestrel::diagnostic::Severity::Warning)
    );
    let parsed = parse_file(
        "sets.kes",
        "theorem guessed(n : Nat) : n = Zero by { sorry }\n",
    );
    let result = check(&parsed);
    let report = goals_at(&parsed, &result, 0);
    assert!(report.admitted);
}

#[test]
fn an_admitted_theorem_cannot_discharge_a_runtime_requirement() {
    let result = checked(
        "theorem guessed(n : Nat) : n = Zero by { sorry }\n\
         function bad(n : Nat) : Nat requires guessed(n) { 0 }\n",
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "E_PROOF_SORRY"),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn a_checked_rewrite_turns_add_into_builtin_addition() {
    good(
        "function add(a : Nat, b : Nat) : Nat = match a {\n\
            Zero => b,\n\
            Succ(k) => Succ(add(k, b)),\n\
         }\n\
         @simp\n\
         theorem addEq(a : Nat, b : Nat) : add(a, b) = a + b requires room : a + b < 18446744073709551615 by {\n\
            induction a {\n\
                Zero => { rfl }\n\
                Succ(k) ih => { simp [ih] }\n\
            }\n\
         }\n\
         theorem use(n : Nat, m : Nat) : add(n, m) = n + m requires room : n + m < 18446744073709551615 by {\n\
            simp [addEq]\n\
         }\n",
    );
}
