//! Proposition variables and opaque (uninterpreted) declarations.
use kestrel::{checker::check, parse_file};

fn good(source: &str) {
    let parsed = parse_file("opaque.kes", source);
    assert!(parsed.is_ok(), "{source}\n{:#?}", parsed.diagnostics);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{source}\n{:#?}", checked.diagnostics);
}

fn refuses(source: &str, code: &str) {
    let parsed = parse_file("opaque.kes", source);
    let checked = check(&parsed);
    assert!(
        checked.diagnostics.iter().any(|d| d.code == code),
        "Expected {code}: {source}\n{:#?}",
        checked.diagnostics
    );
}

#[test]
fn prop_parameters_are_opaque_propositions() {
    good("theorem identity(P : Prop) : P -> P by { intro h; exact h }");
    good(
        "theorem both(P : Prop, Q : Prop) : P -> Q -> (P and Q) by { intro hp; intro hq; constructor; { exact hp }; { exact hq } }",
    );
    good("theorem split(P : Prop, Q : Prop) : (P and Q) -> P by { intro h; exact(h.left) }");
    good("theorem classical(P : Prop) : P -> (P or not P) by { intro h; left; exact h }");
    good("theorem negneg(P : Prop) : not (not P) -> P by { intro h; byContra hn; exact h(hn) }");
    good(
        "theorem swapped(P : Prop, Q : Prop) : (P -> Q) -> (not Q -> not P) by { intro h; intro hn; intro hp; have hq := h(hp); exact hn(hq) }",
    );
}

#[test]
fn prop_arguments_pass_to_other_theorems() {
    good(
        "theorem identity(P : Prop) : P -> P by { intro h; exact h }\n\
         theorem uses(Q : Prop) : Q -> Q by { apply identity }",
    );
}

#[test]
fn opaque_types_and_predicates_are_quantified_and_constrained_by_axioms() {
    good(
        "opaque type Point\n\
         opaque function between(a : Point, b : Point, c : Point) : Prop\n\
         axiom betwSym(a : Point, b : Point, c : Point) : between(a, b, c) -> between(c, b, a)\n\
         theorem reverse(a : Point, b : Point, c : Point) : between(a, b, c) -> between(c, b, a) by {\n\
            intro h\n\
            apply betwSym\n\
            exact h\n\
         }",
    );
    good(
        "opaque type Point\n\
         opaque function line(A : Point, B : Point) : Set[Point]\n\
         axiom onLine(A : Point, B : Point) : A ∈ line(A, B)\n\
         theorem lies(A : Point, B : Point) : A ∈ line(A, B) by { apply onLine }",
    );
    good(
        "opaque function divides(a : Int, b : Int) : Prop\n\
         axiom dvdRefl(a : Int) : divides(a, a)\n\
         theorem refl(a : Int) : divides(a, a) by { apply dvdRefl }",
    );
}

#[test]
fn opaque_functions_take_type_parameters() {
    good(
        "opaque function pick[U](A : Set[U], x : U) : U\n\
         axiom pickMem[U](A : Set[U], x : U) : x ∈ A -> pick(A, x) ∈ A\n\
         theorem picked[U](A : Set[U], x : U) : x ∈ A -> pick(A, x) ∈ A by { apply pickMem }",
    );
}

#[test]
fn opaque_functions_are_refused_in_runtime_code() {
    // The declaration itself is fine; `@test` functions cannot call it because
    // it has no body and no run-time representation.
    refuses(
        "opaque function f(a : Int) : Int\n@test function t() : void { let x := f(1); }",
        "E_OPAQUE_BODY",
    );
}

#[test]
fn prop_parameters_cannot_be_used_as_values() {
    refuses("theorem t(P : Prop) : P by { exact P }", "E_PROOF_UNKNOWN");
}

#[test]
fn prop_parameters_instantiate_at_compound_propositions() {
    good(
        "theorem identity(P : Prop) : P -> P by { intro h; exact h }\n\
         theorem compound(A : Prop, B : Prop) : (A and B) -> (A and B) by { apply identity }",
    );
    good(
        "theorem identity(P : Prop) : P -> P by { intro h; exact h }\n\
         theorem quantified(A : Prop, B : Prop) : (A and B) -> (A and B) by {\n\
            apply identity\n\
         }\n\
         theorem nested(A : Prop, B : Prop) : ((A and B) or not (A and B)) -> ((A and B) or not (A and B)) by { apply identity }",
    );
}

#[test]
fn quantifiers_bind_prop() {
    good(
        "theorem identity() : forall P : Prop, P -> P by {\n\
            intro P\n\
            intro h\n\
            exact h\n\
        }",
    );
    good(
        "theorem identity() : forall P : Prop, P -> P by { intro P; intro h; exact h }\n\
         theorem uses(A : Prop, B : Prop) : (A and B) -> (A and B) by { apply identity }",
    );
}

#[test]
fn compound_propositions_pass_to_opaque_predicates() {
    good(
        "opaque function says(a : Int, p : Prop) : Prop\n\
         axiom truth(a : Int, p : Prop) : says(a, p) -> p\n\
         theorem t(a : Int, A : Prop, B : Prop) : says(a, A and B) -> (A and B) by {\n\
            intro h\n\
            apply truth(a)\n\
            exact h\n\
         }",
    );
}
