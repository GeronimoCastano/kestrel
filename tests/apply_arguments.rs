//! `apply` with explicit arguments plus goal-inferred holes, and the rule that
//! a supplied argument never rewrites a goal variable that shares its name.
use kestrel::{checker::check, parse_file};

fn good(source: &str) {
    let parsed = parse_file("apply.kes", source);
    assert!(parsed.is_ok(), "{source}\n{:#?}", parsed.diagnostics);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{source}\n{:#?}", checked.diagnostics);
}

#[test]
fn explicit_arguments_bind_theorem_parameters_and_foralls_come_from_the_goal() {
    good(
        "theorem same(a : Nat) : forall b : Nat, b = b by { intro b; rfl }\n\
         theorem uses(n : Nat) : n = n by { apply same(1) }",
    );
}

#[test]
fn a_supplied_argument_does_not_rewrite_the_goal() {
    // `apply step(1)` supplies the theorem's `a`, while the goal's `a` is a
    // different variable. The matcher must not rewrite the goal's `a` to 1.
    good(
        "function f(a : Nat, b : Nat) : Nat = a\n\
         theorem step(a : Nat) : forall b : Nat, f(a, b) = a by { intro b; rfl }\n\
         theorem uses(a : Nat, b : Nat) : f(1, b) = 1 by { apply step(1) }",
    );
}

#[test]
fn a_compound_proposition_can_be_supplied_for_a_prop_parameter() {
    good(
        "theorem same(P : Prop) : P -> P by { intro h; exact h }\n\
         theorem uses(A : Prop, B : Prop) : (A or B) -> (A or B) by { apply same }",
    );
}
