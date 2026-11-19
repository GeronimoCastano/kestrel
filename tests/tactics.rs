//! The tactic language: proof scripts, quantifiers and live goal states.
use kestrel::{checker::check, parse_file, proof::goals_at};

const NAT: &str = "function add(a : Nat, b : Nat) : Nat = match a {\n    Zero => b,\n    Succ(k) => Succ(add(k, b)),\n}\n";

fn good(source: &str) {
    let parsed = parse_file("tactics.kes", source);
    assert!(parsed.is_ok(), "{source}\n{:#?}", parsed.diagnostics);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{source}\n{:#?}", checked.diagnostics);
}

fn refuses(source: &str, code: &str) {
    let parsed = parse_file("tactics.kes", source);
    let checked = check(&parsed);
    assert!(
        checked.diagnostics.iter().any(|d| d.code == code),
        "Expected {code}: {source}\n{:#?}",
        checked.diagnostics
    );
}

fn with_nat(body: &str) -> String {
    format!("{NAT}{body}")
}

#[test]
fn induction_and_simp_close_standard_nat_goals() {
    good(&with_nat(
        "theorem addZeroR(n : Nat) : add(n, Zero) = n by {\n    induction n {\n        Zero => { rfl }\n        Succ(k) ih => { simp [ih] }\n    }\n}",
    ));
    good(&with_nat(
        "theorem addZeroR(n : Nat) : add(n, Zero) = n by {\n    induction n {\n        Zero => { rfl }\n        Succ(k) ih => { simp [ih] }\n    }\n}\ntheorem twice(n : Nat) : add(add(n, Zero), Zero) = n by { rw [addZeroR]; rfl }",
    ));
}

#[test]
fn intro_exact_apply_and_assumption_use_hypotheses() {
    good("theorem identity(n : Nat) : n = n -> n = n by { intro h; exact h }");
    good("theorem fromAssumption(n : Nat) : n = n -> n = n by { intro h; assumption }");
    good(&with_nat(
        "theorem addZeroR(n : Nat) : add(n, Zero) = n by { induction n { Zero => { rfl }, Succ(k) ih => { simp [ih] } } }\ntheorem applied(n : Nat) : add(n, Zero) = n by { apply addZeroR }",
    ));
    good(&with_nat(
        "theorem addZeroR(n : Nat) : add(n, Zero) = n by { induction n { Zero => { rfl }, Succ(k) ih => { simp [ih] } } }\ntheorem applied(n : Nat) : add(n, Zero) = n by { apply addZeroR(n) }",
    ));
}

#[test]
fn and_or_and_exists_have_their_introduction_and_elimination_tactics() {
    good("theorem both(n : Nat) : n = n and n = n by { constructor; { rfl }; { rfl } }");
    good("theorem either(n : Nat) : n = n or n = Zero by { left; rfl }");
    good("theorem witness(n : Nat) : exists m : Nat, m = m by { exists n; rfl }");
    good(
        "theorem splitAnd(n : Nat) : n = n by { have h : n = n and n = n := (rfl, rfl); cases h; assumption }",
    );
    good(
        "theorem splitOr(n : Nat) : n = n by { have h : n = n or n = n := Left(rfl); cases h; { assumption }; { assumption } }",
    );
    good(
        "theorem splitExists(n : Nat) : n = n by { have h : exists m : Nat, m = m := exists(n, rfl); cases h; rfl }",
    );
}

#[test]
fn have_contradiction_exfalso_trivial_and_decide_work() {
    good("theorem named(n : Nat) : n = n by { have h : n = n := rfl; exact h }");
    good("theorem namedBlock(n : Nat) : n = n by { have h : n = n by { rfl }; exact h }");
    good("theorem impossible(k : Nat) : not (Zero = Succ(k)) by { intro h; contradiction }");
    good(
        "theorem ex(n : Nat) : n = n requires h : Zero = Succ(Zero) by { exfalso; contradiction }",
    );
    good("theorem triv(n : Nat) : n = n by { trivial }");
    good("theorem decided() : 3 > 2 by { decide }");
}

#[test]
fn quantifier_statements_accept_lambdas_and_witnesses() {
    good("theorem reflexive(n : Nat) : forall m : Nat, m = m { m => rfl }");
    good("theorem withParens(n : Nat) : forall (m : Nat), m = m { m => rfl }");
    good("theorem implication(n : Nat) : (n = n) -> n = n { h => h }");
    good("theorem witness() : exists n : Nat, n = n { exists(Zero, rfl) }");
    good("theorem truth() : true { decide }");
    good("theorem forallTactic(n : Nat) : forall m : Nat, m = m by { intro m; rfl }");
    good("theorem existsTactic() : exists n : Nat, n = n by { exists Zero; rfl }");
}

#[test]
fn repeat_try_trans_intros_and_use_are_available() {
    good("theorem both(n : Nat) : n = n and n = n by { constructor; repeat { rfl } }");
    good(
        "theorem bothTry(n : Nat) : n = n and n = n by { constructor; { try { skip }; rfl }; { rfl } }",
    );
    good("theorem middle(n : Nat) : n = n by { trans n; { rfl }; { rfl } }");
    good("theorem quantified(n : Nat) : forall m : Nat, m = m by { intros; rfl }");
}

#[test]
fn simp_closes_logical_goals_structurally() {
    good("theorem both(n : Nat) : n = n and n = n by { simp }");
    good("theorem either(n : Nat) : n = n or n = Zero by { simp }");
    good("theorem truth() : true by { simp }");
}

#[test]
fn tactics_report_precise_failures() {
    refuses(
        &with_nat(
            "theorem unfinished(n : Nat) : add(n, Zero) = n by { induction n { Zero => { rfl }, Succ(k) ih => { skip } } }",
        ),
        "E_PROOF_INCOMPLETE",
    );
    refuses(
        "theorem wrong(n : Nat) : n = n by { intro h; exact h }",
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem missing(n : Nat) : n = n by { cases n { Zero => { rfl } } }",
        "E_PROOF_CASES",
    );
    refuses(
        "theorem applyWrong(n : Nat) : n = Zero by { apply addZeroR }",
        "E_PROOF_UNKNOWN",
    );
    refuses(
        "theorem unknown(n : Nat) : n = n by { frobnicate }",
        "E_TACTIC_UNKNOWN",
    );
    refuses(
        "theorem noProof(n : Nat) : n = n by { }",
        "E_PROOF_INCOMPLETE",
    );
}

#[test]
fn tactic_induction_keeps_assumptions_at_the_smaller_value() {
    good(&with_nat(
        "theorem bounded(n : Nat) : add(n, Zero) = n requires h : n < 100 by {\n    induction n {\n        Zero => { rfl }\n        Succ(k) ih => { simp [ih] }\n    }\n}",
    ));
}

#[test]
fn apply_does_not_instantiate_a_free_hypothesis_variable() {
    refuses(
        "theorem bad(n : Nat, m : Nat) : m = Zero requires h : n = Zero by { apply h }",
        "E_PROOF_MISMATCH",
    );
    good(
        "theorem good(n : Nat) : n = n by { have h : forall m : Nat, m = m := m => rfl; apply h }",
    );
}

#[test]
fn conjunction_projection_reaches_one_side() {
    good(
        "theorem left(n : Nat) : n = n by { have h : n = n and n = n := (rfl, rfl); exact(h.left) }",
    );
    good(
        "theorem right(n : Nat) : n = n by { have h : n = n and n = n := (rfl, rfl); exact(h.right) }",
    );
    good(
        "theorem nested(n : Nat) : n = n by { have h : n = n and (n = n and n = n) := (rfl, (rfl, rfl)); exact(h.right.right) }",
    );
    good(
        "theorem applied(n : Nat) : n = n by { have h : (n = n -> n = n) and n = n := (x => x, rfl); apply(h.left); rfl }",
    );
    good("theorem interLeft[U](x : U, A B : Set[U], h : x ∈ A ∩ B) : x ∈ A by { exact(h.left) }");
    good("theorem explicit(n : Nat, h : n = n and n = n) : n = n { h.left }");
    good(
        "theorem explicitNested(n : Nat, h : (n = n and n = n) and n = n) : n = n { h.left.right }",
    );
    good(
        "theorem pair(n : Nat) : n = n and n = n { (rfl, rfl) }\ntheorem fromCall(n : Nat) : n = n { pair(n).left }",
    );
    good(
        "theorem appliedProjection(n : Nat) : n = n by { have h : (n = n -> n = n) and n = n := (x => x, rfl); exact(h.left(rfl)) }",
    );
}

#[test]
fn named_introduction_constructors_build_each_connective() {
    good("theorem andTerm(n : Nat) : n = n and n = n { And(rfl, rfl) }");
    good("theorem andTactic(n : Nat) : n = n and n = n by { exact And(rfl, rfl) }");
    good(
        "theorem andInfer(n : Nat) : n = n by { have h1 : n = n := rfl; have h2 : n = n := rfl; have h := And(h1, h2); exact(h.left) }",
    );
    good("theorem orLeftTerm(n : Nat) : n = n or n = Zero { OrLeft(rfl) }");
    good("theorem orRightTerm(n : Nat) : n = Zero or n = n { OrRight(rfl) }");
    good("theorem iffTerm(n : Nat) : n = n ↔ n = n { Iff(x => x, x => x) }");
    good(
        "theorem iffInfer(n : Nat) : n = n ↔ n = n by { have f : n = n -> n = n := x => x; have g : n = n -> n = n := x => x; have h := Iff(f, g); exact(h) }",
    );
    good("theorem existsTerm(n : Nat) : exists m : Nat, m = m { Exists(n, rfl) }");
    good("theorem trueTerm() : true { True }");
    good("theorem trueInfer() : true by { have h := True; exact(h) }");
    good(
        "theorem nested(n : Nat, h1 : n = n, h2 : n = n, h3 : n = n) : n = n and (n = n and n = n) { And(h1, And(h2, h3)) }",
    );
    good(
        "theorem interIntro[U](x : U, A B : Set[U], h1 : x ∈ A, h2 : x ∈ B) : x ∈ A ∩ B { And(h1, h2) }",
    );
    good("theorem unionIntro[U](x : U, A B : Set[U], h1 : x ∈ A) : x ∈ A ∪ B { OrLeft(h1) }");
}

#[test]
fn introduction_constructors_report_a_wrong_shape() {
    refuses(
        "theorem bad(n : Nat) : n = n { And(rfl, rfl) }",
        "E_PROOF_SHAPE",
    );
    refuses("theorem bad(n : Nat) : n = n { True }", "E_PROOF_SHAPE");
    refuses(
        "theorem bad(n : Nat) : n = n { OrLeft(rfl) }",
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem bad(n : Nat) : n = n { Iff(x => x, x => x) }",
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem bad(n : Nat) : n = n and n = n { And(rfl) }",
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem bad(n : Nat) : n = n or n = Zero by { have h := OrLeft(rfl); exact h }",
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem bad(n : Nat) : n = Zero or n = n by { have h := OrRight(rfl); exact h }",
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem bad(n : Nat) : n = n by { have h := Iff(x => x, x => x); exact h }",
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem bad(n : Nat) : n = n by { have h := exists(n, rfl); exact h }",
        "E_PROOF_SHAPE",
    );
}

#[test]
fn projections_refuse_a_side_that_does_not_exist() {
    refuses(
        "theorem bad(n : Nat) : n = n by { have h : n = n or n = Zero := Left(rfl); exact(h.left) }",
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem bad(n : Nat) : n = n by { have h : n = n and n = n := (rfl, rfl); exact(h.middle) }",
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem bad(n : Nat) : n = n by { have h : n = n := rfl; exact(h.left) }",
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem bad(n : Nat) : n = n by { have h : n = n ↔ n = n by { constructor; { intro x; exact x }; { intro x; exact x } }; exact(h.left) }",
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem bad(n : Nat) : n = n by { have h : exists m : Nat, n = n := exists(n, rfl); exact(h.left) }",
        "E_PROOF_SHAPE",
    );
}

#[test]
fn cases_names_its_parts_after_the_split_hypothesis() {
    good(
        "theorem names(n : Nat) : n = n by { have h1 : n = n and n = n := (rfl, rfl); cases h1; exact(h1l) }",
    );
    good(
        "theorem names(n : Nat) : n = n by { have h1 : n = n and n = n := (rfl, rfl); cases h1; exact(h1r) }",
    );
    good(
        "theorem names(n : Nat) : n = n or n = Zero by { have h3 : n = n or n = Zero := Left(rfl); cases h3; { left; exact(h3l) }; { right; exact(h3r) } }",
    );
    // A split no longer overwrites a differently named hypothesis.
    good(
        "theorem names(n : Nat) : n = Zero requires hl : n = Zero requires h3 : n = n and n = n by { cases h3; exact(hl) }",
    );
}

#[test]
fn goals_show_derived_case_names() {
    let source = "theorem splitNames(n : Nat) : n = n by {\n    have h3 : n = n and n = n := (rfl, rfl)\n    cases h3\n    exact(h3l)\n}\n";
    let parsed = parse_file("tactics.kes", source);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{:#?}", checked.diagnostics);
    let report = goals_at(&parsed, &checked, source.find("exact(h3l)").unwrap());
    assert!(
        report.goals[0]
            .hypotheses
            .iter()
            .any(|(name, _)| name == "h3l"),
        "{:#?}",
        report.goals[0].hypotheses
    );
    assert!(
        report.goals[0]
            .hypotheses
            .iter()
            .any(|(name, _)| name == "h3r"),
        "{:#?}",
        report.goals[0].hypotheses
    );
}

#[test]
fn the_tactics_example_checks() {
    good(include_str!("../testdata/language/tactics.kes"));
}

#[test]
fn goals_follow_the_cursor_inside_a_tactic_proof() {
    let source = with_nat(
        "theorem addZeroR(n : Nat) : add(n, Zero) = n by {\n    induction n {\n        Zero => { rfl }\n        Succ(k) ih => { simp [ih] }\n    }\n}\n",
    );
    let parsed = parse_file("tactics.kes", &source);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{:#?}", checked.diagnostics);
    let at = source.find("simp [ih]").unwrap();
    let report = goals_at(&parsed, &checked, at);
    assert_eq!(report.theorem, "addZeroR");
    assert!(report.error.is_none(), "{:?}", report.error);
    assert_eq!(report.goals.len(), 1);
    assert_eq!(report.goals[0].target, "add(Succ(k), 0) = Succ(k)");
    assert_eq!(
        report.goals[0].variables,
        vec![("k".to_owned(), "Nat".to_owned())]
    );
    assert_eq!(
        report.goals[0].hypotheses,
        vec![("ih".to_owned(), "add(k, 0) = k".to_owned())]
    );

    let initial = goals_at(&parsed, &checked, source.find("induction").unwrap());
    assert_eq!(initial.goals[0].target, "add(n, 0) = n");

    let outside = goals_at(&parsed, &checked, 0);
    assert!(outside.theorem.is_empty());
}

#[test]
fn goals_report_an_unfinished_script() {
    let source = with_nat("theorem bad(n : Nat) : add(n, Zero) = n by {\n    skip\n}\n");
    let parsed = parse_file("tactics.kes", &source);
    let checked = check(&parsed);
    let at = source.find("skip").unwrap();
    let report = goals_at(&parsed, &checked, at);
    assert_eq!(report.theorem, "bad");
    assert!(!report.complete);
    assert_eq!(report.goals[0].target, "add(n, 0) = n");
    assert!(report.error.is_none(), "{:?}", report.error);
}

#[test]
fn exact_rejects_a_value_with_a_type_error() {
    let parsed = parse_file("tactics.kes", "theorem bad() : 1 = 1 by { exact 3 }\n");
    let result = check(&parsed);
    assert!(
        result.diagnostics.iter().any(|d| d.code == "E_PROOF_VALUE"),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn cases_reports_every_branch_at_once() {
    let source = "theorem split(n : Nat) : n = n by {\n    cases n {\n        Zero => { rfl }\n        Succ(k) => { rfl }\n    }\n}\n";
    let parsed = parse_file("tactics.kes", source);
    let checked = check(&parsed);
    let offset = source.find("Zero =>").unwrap();
    let report = goals_at(&parsed, &checked, offset);
    assert_eq!(
        report
            .cases
            .iter()
            .map(|case| case.label.as_str())
            .collect::<Vec<_>>(),
        ["Zero", "Succ"]
    );
    assert!(report.cases.iter().any(|case| case.active));
    assert_eq!(report.goals.len(), 1);
}

#[test]
fn goals_stay_visible_while_a_tactic_is_half_written() {
    let source = "example[U](A : Set[U], x : U, h : x ∈ A) : x ∈ A by {\n    exac\n}\n";
    let parsed = parse_file("tactics.kes", source);
    let checked = check(&parsed);
    let report = goals_at(&parsed, &checked, source.find("exac").unwrap());
    assert_eq!(report.theorem, "example");
    assert_eq!(report.goals.len(), 1);
    assert_eq!(report.goals[0].target, "x ∈ A");
    assert!(
        report
            .error
            .as_deref()
            .is_some_and(|error| error.contains("E_TACTIC_UNKNOWN"))
    );
}

#[test]
fn goals_stay_visible_before_the_tactic_block_is_closed() {
    let source = "example[U](A : Set[U], x : U, h : x ∈ A) : x ∈ A by {\n";
    let parsed = parse_file("tactics.kes", source);
    let checked = check(&parsed);
    let report = goals_at(&parsed, &checked, source.len());
    assert_eq!(report.theorem, "example");
    assert_eq!(report.goals[0].target, "x ∈ A");
}

#[test]
fn goals_stay_visible_when_exact_has_no_argument_yet() {
    let source = "example[U](A : Set[U], x : U, h : x ∈ A) : x ∈ A by {\n    exact\n}\n";
    let parsed = parse_file("tactics.kes", source);
    let checked = check(&parsed);
    let report = goals_at(&parsed, &checked, source.find("exact").unwrap());
    assert_eq!(report.goals[0].target, "x ∈ A");
    assert!(
        report
            .error
            .as_deref()
            .is_some_and(|error| error.contains("E_TACTIC_ARGUMENT"))
    );
}
