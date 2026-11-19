//! A `←` before a rewrite rule uses that rule from right to left.
use kestrel::{checker::check, parse_file};

fn good(source: &str) {
    let parsed = parse_file("reverse_rewrite.kes", source);
    assert!(parsed.is_ok(), "{source}\n{:#?}", parsed.diagnostics);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{source}\n{:#?}", checked.diagnostics);
}

#[test]
fn a_reversed_equality_rewrites_the_goal_backwards() {
    good("theorem revEq(x : Nat, y : Nat, h : y = x) : x = y by {\n    rw [← h]\n    rfl\n}");
}

#[test]
fn a_reversed_iff_rewrites_propositions_backwards() {
    good(
        "theorem revIff(P : Prop, Q : Prop, h : P ↔ Q) : P -> Q by {\n    intro hp\n    rw [← h]\n    exact hp\n}",
    );
}

#[test]
fn reverse_and_forward_rules_mix_in_one_list() {
    good(
        "theorem mixed(x : Nat, y : Nat, z : Nat, h1 : x = y, h2 : z = y) : x = z by {\n    rw [h1, ← h2]\n    rfl\n}",
    );
}

#[test]
fn simp_accepts_a_reversed_rule() {
    good("theorem simpRev(x : Nat, y : Nat, h : y = x) : x = y by {\n    simp [← h]\n}");
}

#[test]
fn the_ascii_alias_reverses_the_same_way() {
    good(
        "theorem asciiRev(x : Nat, y : Nat, h : y = x) : x = y by {\n    rw [\\larr h]\n    rfl\n}",
    );
}
