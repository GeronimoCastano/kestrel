//! Family sets, the set builder and finite sets in the proof kernel.
use kestrel::{checker::check, parse_file, proof::goals_at};

fn checked(source: &str) -> kestrel::checker::Checked {
    let parsed = parse_file("families.kes", source);
    assert!(parsed.is_ok(), "{source}\n{:#?}", parsed.diagnostics);
    check(&parsed)
}

fn good(source: &str) {
    let result = checked(source);
    assert!(result.is_ok(), "{source}\n{:#?}", result.diagnostics);
}

#[test]
fn family_example_checks() {
    good(include_str!("../testdata/language/families.kes"));
}

#[test]
fn family_membership_unfolds_to_quantifiers() {
    good(
        "theorem unionMem[U](F : Set[Set[U]], x : U) :\
            x ∈ ⋃₀ F ↔ exists A : Set[U], A ∈ F and x ∈ A by { rfl }\n\
         theorem interMem[U](F : Set[Set[U]], x : U) :\
            x ∈ ⋂₀ F ↔ forall A : Set[U], A ∈ F -> x ∈ A by { rfl }\n",
    );
}

#[test]
fn finite_sets_and_the_builder_unfold_to_membership() {
    good(
        "theorem emptyMem[U](x : U) : x ∈ ∅ ↔ false by { rfl }\n\
         theorem singleMem[U](x a : U) : x ∈ {a} ↔ x = a by { rfl }\n\
         theorem pairMem[U](x a b : U) : x ∈ {a, b} ↔ x = a or x = b by { rfl }\n\
         theorem builderMem[U](x a : U) : x ∈ {y : U | y = a} ↔ x = a by { rfl }\n",
    );
}

#[test]
fn set_quantifiers_accept_families_and_nested_sets() {
    good(
        "theorem familyReflexive[U](F : Set[Set[U]]) :\
            forall G : Set[Set[U]], G = G by { intro G; rfl }\n\
         theorem setExists[U](A : Set[U]) :\
            exists B : Set[U], B = A by { exists A; rfl }\n",
    );
}

#[test]
fn keyboard_aliases_parse_and_mean_the_same() {
    good(
        "theorem aliasUnion[U](F : Set[Set[U]], x : U, h : x ∈ \\union0 F) :\
            x ∈ ⋃₀ F by { exact h }\n\
         theorem aliasInter[U](F : Set[Set[U]], x : U, h : x ∈ \\inter0 F) :\
            x ∈ ⋂₀ F by { exact h }\n\
         theorem aliasEmpty[U](x : U) : x ∈ \\emptyset ↔ false by { rfl }\n",
    );
}

#[test]
fn goals_keep_the_family_surface_notation() {
    let source = "theorem fam[U](F : Set[Set[U]], x : U) : x ∈ ⋃₀ F by { sorry }\n";
    let parsed = parse_file("families.kes", source);
    assert!(parsed.is_ok(), "{:#?}", parsed.diagnostics);
    let checked = check(&parsed);
    let report = goals_at(&parsed, &checked, 0);
    assert_eq!(report.goals[0].target, "x ∈ ⋃₀ F");
}

#[test]
fn the_empty_set_needs_a_type() {
    let result = checked("theorem bad[U]() : ∅ = ∅ by { rfl }\n");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PROOF_TYPE" && d.message.contains("empty set")),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn a_family_operator_needs_a_family() {
    let result = checked("theorem bad[U](A : Set[U]) : ⋃₀ A = A by { rfl }\n");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| { d.code == "E_PROOF_TYPE" && d.message.contains("family of sets") }),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn a_builder_predicate_must_be_a_proposition() {
    let result = checked("theorem bad[U](a : U) : a ∈ {x : U | x} by { sorry }\n");
    assert!(
        result.diagnostics.iter().any(|d| d.code == "E_PROOF_TYPE"),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn a_quantifier_over_an_unsupported_type_is_refused() {
    let result = checked("theorem bad(n : Nat) : forall x : double, x = x by { sorry }\n");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| { d.code == "E_PROOF_UNSUPPORTED" && d.message.contains("quantify") }),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn simp_closes_membership_of_finite_and_built_sets() {
    good(
        "theorem singletonSimp[U](a : U) : a ∈ {a} by { simp }\n\
         theorem pairSimp[U](a b : U) : a ∈ {a, b} by { simp }\n\
         theorem builderSimp[U](a : U) : a ∈ {x : U | x = a} by { simp }\n",
    );
}

#[test]
fn builder_binders_are_alpha_equivalent() {
    good(
        "theorem builderAlpha[U](a : U) :\
            {x : U | x = a} = {y : U | y = a} by { rfl }\n",
    );
}

#[test]
fn family_theorems_can_be_cited_at_another_element_type() {
    good(
        "theorem citeFamily[U](F : Set[Set[U]], x : U, h : x ∈ ⋃₀ F) :\
            x ∈ ⋃₀ F by { exact h }\n\
         theorem useFamily(F : Set[Set[Nat]], n : Nat, h : n ∈ ⋃₀ F) :\
            n ∈ ⋃₀ F by { exact citeFamily(F, n) }\n",
    );
}

#[test]
fn rw_rewrites_inside_family_terms() {
    good(
        "theorem familyRewrite[U](F : Set[Set[U]], G : Set[Set[U]], h : G = F, x : U) :\
            x ∈ ⋃₀ F -> x ∈ ⋃₀ G by {\n\
            intro hx\n\
            rw [h]\n\
            exact hx\n\
         }\n",
    );
}

#[test]
fn an_empty_family_takes_its_type_from_the_goal() {
    good(
        "theorem emptyUnion[U](x : U) : x ∈ ⋃₀ ∅ -> false by {\n\
            intro h\n\
            cases h\n\
            cases hw\n\
            exact hwl\n\
         }\n\
         theorem emptyInter[U](x : U) : x ∈ ⋂₀ ∅ by {\n\
            intro s\n\
            intro hs\n\
            exfalso\n\
            exact hs\n\
         }\n",
    );
}

#[test]
fn apply_matches_builders_up_to_their_binder() {
    good(
        "theorem builderFact[U](a : U) : a ∈ {x : U | x = a} by { simp }\n\
         theorem builderApply[U](a : U) : a ∈ {y : U | y = a} by {\n\
            apply builderFact\n\
         }\n",
    );
}

#[test]
fn cases_substitutes_an_equality() {
    good(
        "theorem equalityCases[U](A : Set[U], B : Set[U], h : A = B, x : U, hx : x ∈ A) :\
            x ∈ B by {\n\
            cases h\n\
            exact hx\n\
         }\n",
    );
}

#[test]
fn the_legacy_empty_alias_still_parses() {
    good("theorem legacyEmpty[U](x : U) : x ∈ \\empty ↔ false by { rfl }\n");
}
