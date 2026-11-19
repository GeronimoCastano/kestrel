//! Union, intersection and complement in the proof kernel.
use kestrel::{checker::check, parse_file};

fn checked(source: &str) -> kestrel::checker::Checked {
    let parsed = parse_file("setops.kes", source);
    assert!(parsed.is_ok(), "{source}\n{:#?}", parsed.diagnostics);
    check(&parsed)
}

fn good(source: &str) {
    let result = checked(source);
    assert!(result.is_ok(), "{source}\n{:#?}", result.diagnostics);
}

#[test]
fn union_membership_is_or() {
    good(
        "theorem leftOr[U](x : U, A : Set[U], B : Set[U], h : x ∈ A) : x ∈ A ∪ B by {\n\
            left\n\
            exact h\n\
         }\n\
         theorem rightOr[U](x : U, A : Set[U], B : Set[U], h : x ∈ B) : x ∈ A ∪ B by {\n\
            right\n\
            exact h\n\
         }\n\
         theorem fromOr[U](x : U, A : Set[U], B : Set[U], h : x ∈ A ∪ B) : x ∈ B ∪ A by {\n\
            cases h\n\
            right\n\
            exact hl\n\
            left\n\
            exact hr\n\
         }\n",
    );
}

#[test]
fn intersection_membership_is_and() {
    good(
        "theorem both[U](x : U, A : Set[U], B : Set[U], h1 : x ∈ A, h2 : x ∈ B) : x ∈ A ∩ B by {\n\
            constructor\n\
            exact h1\n\
            exact h2\n\
         }\n\
         theorem leftOfAnd[U](x : U, A : Set[U], B : Set[U], h : x ∈ A ∩ B) : x ∈ A by {\n\
            cases h\n\
            exact hl\n\
         }\n",
    );
}

#[test]
fn complement_membership_is_negation() {
    good(
        "theorem memCompl[U](A : Set[U], x : U) : x ∈ Aᶜ ↔ x ∉ A by { rfl }\n\
         theorem complIntro[U](A : Set[U], x : U, h : x ∉ A) : x ∈ Aᶜ by {\n\
            intro hx\n\
            exact h(hx)\n\
         }\n",
    );
}

#[test]
fn ext_splits_set_equality_into_two_subsets() {
    good(
        "theorem unionComm[U](A B : Set[U]) : A ∪ B = B ∪ A by {\n\
            ext\n\
            intro x h\n\
            cases h\n\
            right\n\
            exact hl\n\
            left\n\
            exact hr\n\
            intro x h\n\
            cases h\n\
            right\n\
            exact hl\n\
            left\n\
            exact hr\n\
         }\n",
    );
}

#[test]
fn preferred_spelling_proves_double_complement() {
    good(
        "theorem complCompl[U](A : Set[U]) : Aᶜᶜ = A by {\n\
            ext\n\
            intro x h\n\
            byContra hx\n\
            exact h(hx)\n\
            intro x hx\n\
            intro h\n\
            exact h(hx)\n\
         }\n",
    );
}

#[test]
fn constructor_and_cases_handle_iff() {
    good(
        "theorem subsetCompl[U](A B : Set[U]) : A ⊆ B ↔ Bᶜ ⊆ Aᶜ by {\n\
            constructor\n\
            intro h\n\
            intro x hx\n\
            intro ha\n\
            exact hx(h(ha))\n\
            intro h\n\
            intro x hx\n\
            byContra hb\n\
            have ha : x ∈ Aᶜ := h(hb)\n\
            exact ha(hx)\n\
         }\n\
         theorem useIff[U](A B : Set[U], h : A ⊆ B ↔ Bᶜ ⊆ Aᶜ) : A ⊆ B → Bᶜ ⊆ Aᶜ by {\n\
            cases h\n\
            exact hmp\n\
         }\n",
    );
}

#[test]
fn set_operations_need_sets() {
    let result = checked("theorem bad(x : U, y : U) : x ∪ y = x by { rfl }\n");
    assert!(
        result.diagnostics.iter().any(|d| d.code == "E_PROOF_TYPE"),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn have_infers_the_statement_from_the_proof_term() {
    good(
        "theorem complContradiction[U](A B : Set[U], x : U, h1 : x ∈ A, h2 : x ∉ B) : not (A ⊆ B) by {\n\
            intro h3\n\
            have h4 := h3(h1)\n\
            exact h2(h4)\n\
         }\n\
         theorem trans[U](A B C : Set[U], h1 : A ⊆ B, h2 : B ⊆ C) : A ⊆ C by {\n\
            intro x\n\
            intro hx\n\
            have hxy := h1(hx)\n\
            have hyz := h2(hxy)\n\
            exact hyz\n\
         }\n",
    );
}

#[test]
fn push_neg_removes_double_negation() {
    good(
        "theorem double[U](A : Set[U], x : U, h : x ∈ A) : x ∈ Aᶜᶜ by {\n\
            pushNeg\n\
            exact h\n\
         }\n",
    );
}

#[test]
fn the_lean_by_contra_spelling_is_rejected() {
    let result = checked(
        "theorem double[U](A : Set[U]) : Aᶜᶜ = A by {\n\
            ext\n\
            intro x h\n\
            by_contra hx\n\
            exact h(hx)\n\
            intro x hx\n\
            intro h\n\
            exact h(hx)\n\
         }\n",
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "E_TACTIC_UNKNOWN"),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn goals_keep_the_surface_set_notation() {
    let source = "theorem complIff[U](A B : Set[U]) : A ⊆ B ↔ Bᶜ ⊆ Aᶜ by { sorry }\n";
    let parsed = parse_file("setops.kes", source);
    assert!(parsed.is_ok(), "{:#?}", parsed.diagnostics);
    let checked = check(&parsed);
    let report = kestrel::proof::goals_at(&parsed, &checked, 0);
    assert_eq!(report.goals[0].target, "A ⊆ B ↔ Bᶜ ⊆ Aᶜ");
}
