//! Axioms and unnamed examples.
use kestrel::checker::check;
use kestrel::parse_file;

fn checked(source: &str) -> kestrel::checker::Checked {
    let parsed = parse_file("axioms.kes", source);
    assert!(parsed.is_ok(), "{source}\n{:#?}", parsed.diagnostics);
    check(&parsed)
}

#[test]
fn the_axiom_example_checks() {
    let result = checked(include_str!("../testdata/language/axioms.kes"));
    assert!(result.is_ok(), "{:#?}", result.diagnostics);
    assert!(result.diagnostics.iter().any(|d| d.code == "W_PROOF_AXIOM"));
}

#[test]
fn an_example_is_checked_without_a_name() {
    let result = checked(
        "example(n : Nat) : n = n by { rfl }\n\
         example[U](A : Set[U]) : A ⊆ A by { intro x; intro h; exact h }\n",
    );
    assert!(result.is_ok(), "{:#?}", result.diagnostics);
}

#[test]
fn an_axiom_can_be_cited_in_a_proof() {
    let result = checked(
        "axiom all[U](A : Set[U]) : ∃ x : U, x ∈ A\n\
         theorem use[U](A : Set[U]) : ∃ x : U, x ∈ A by { exact all(A) }\n",
    );
    assert!(result.is_ok(), "{:#?}", result.diagnostics);
}

#[test]
fn an_axiom_cannot_discharge_a_runtime_requirement() {
    let result = checked(
        "axiom all[U](A : Set[U]) : ∃ x : U, x ∈ A\n\
         function bad(A : Set[Nat]) : Nat requires all(A) { 0 }\n",
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PROOF_SORRY" || d.code == "E_PROOF_UNSUPPORTED"),
        "{:#?}",
        result.diagnostics
    );
}
