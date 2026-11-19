//! Rewriting and simplification with `↔` rules.
use kestrel::{checker::check, parse_file};

fn good(source: &str) {
    let parsed = parse_file("iff.kes", source);
    assert!(parsed.is_ok(), "{source}\n{:#?}", parsed.diagnostics);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{source}\n{:#?}", checked.diagnostics);
}

fn refuses(source: &str, code: &str) {
    let parsed = parse_file("iff.kes", source);
    let checked = check(&parsed);
    assert!(
        checked.diagnostics.iter().any(|d| d.code == code),
        "Expected {code}: {source}\n{:#?}",
        checked.diagnostics
    );
}

#[test]
fn rw_uses_an_iff_hypothesis_left_to_right() {
    good(
        "theorem backward(A : Prop, B : Prop, h : A ↔ B) : B -> A by {\n\
            intro hb\n\
            rw [h]\n\
            exact hb\n\
         }",
    );
    good(
        "theorem underAnd(A : Prop, B : Prop, C : Prop, h : A ↔ B) : (B and C) -> (A and C) by {\n\
            intro hx\n\
            rw [h]\n\
            exact hx\n\
         }",
    );
}

#[test]
fn rw_uses_a_checked_iff_theorem() {
    good(
        "theorem andSelf(A : Prop) : (A and A) ↔ A by {\n\
            constructor\n\
            { intro h; exact(h.left) }\n\
            { intro h; constructor; { exact h }; { exact h } }\n\
         }\n\
         theorem uses(A : Prop) : A -> (A and A) by {\n\
            intro h\n\
            rw [andSelf]\n\
            exact h\n\
         }",
    );
}

#[test]
fn simp_closes_when_the_rewritten_goal_is_true() {
    good(
        "theorem closes(A : Prop, h : A ↔ true) : A -> true by {\n\
            intro ha\n\
            simp [h]\n\
         }",
    );
}

#[test]
fn rw_refuses_an_iff_whose_left_side_does_not_occur() {
    refuses(
        "theorem forward(A : Prop, B : Prop, h : A ↔ B) : A -> B by { intro ha; rw [h]; exact ha }",
        "E_PROOF_MISMATCH",
    );
}

#[test]
fn rw_rewrites_arguments_of_an_opaque_predicate() {
    good(
        "opaque function divides(a : Int, b : Int) : Prop\n\
         theorem six() : 2 * 3 = 6 by { rfl }\n\
         axiom dvdSix(n : Int) : divides(6, n)\n\
         theorem t(n : Int) : divides(2 * 3, n) by {\n\
            rw [six]\n\
            apply dvdSix\n\
         }",
    );
}
