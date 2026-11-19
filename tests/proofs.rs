//! Stage 7: explicit proofs, simplification, requirements and arithmetic automation.
use kestrel::{checker::check, parse_file};

const ADD: &str = "function add(a : Nat, b : Nat) : Nat = match a {\n    Zero    => b,\n    Succ(k) => Succ(add(k, b)),\n}\n";

fn good(source: &str) {
    let parsed = parse_file("proofs.kes", source);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{source}\n{:#?}", checked.diagnostics);
}
fn refuses(source: &str, code: &str) {
    let parsed = parse_file("proofs.kes", source);
    let checked = check(&parsed);
    assert!(
        checked.diagnostics.iter().any(|d| d.code == code),
        "Expected {code}: {source}\n{:#?}",
        checked.diagnostics
    );
}
fn with_add(theorems: &str) -> String {
    format!("{ADD}{theorems}")
}

#[test]
fn the_example_file_proves_every_theorem() {
    good(include_str!("../testdata/language/proofs.kes"));
    good(include_str!("../testdata/language/chains.kes"));
    good(include_str!("../testdata/language/classtheorems.kes"));
}

#[test]
fn the_syntax_examples_check_once_add_is_in_scope() {
    good(&with_add(include_str!(
        "../testdata/language/28-theorems.kes"
    )));
    good(&with_add(include_str!(
        "../testdata/language/29-tactics.kes"
    )));
}

#[test]
fn rfl_needs_both_sides_to_reduce_to_the_same_term() {
    good(&with_add("theorem t(n : Nat) : add(Zero, n) = n { rfl }"));
    good(&with_add(
        "theorem t(n : Nat) : add(2, n) = Succ(Succ(n)) requires room : n <= 18446744073709551613 { rfl }",
    ));
    refuses(
        &with_add("theorem t(n : Nat) : add(n, Zero) = n { rfl }"),
        "E_PROOF_MISMATCH",
    );
}

#[test]
fn self_citation_must_be_structurally_smaller() {
    good(&with_add(
        "theorem t(n : Nat) : add(n, Zero) = n { match n { Zero => rfl, Succ(k) => congrArg(Succ, t(k)), } }",
    ));
    refuses(
        &with_add("theorem t(n : Nat) : add(n, Zero) = n { t(n) }"),
        "E_PROOF_RECURSION",
    );
    refuses(
        &with_add(
            "theorem t(n : Nat) : add(n, Zero) = n { match n { Zero => rfl, Succ(k) => congrArg(Succ, t(n)), } }",
        ),
        "E_PROOF_RECURSION",
    );
}

#[test]
fn theorems_cite_only_the_theorems_above_them() {
    refuses(
        &with_add("theorem a(n : Nat) : n = n { b(n) }\ntheorem b(n : Nat) : n = n { rfl }"),
        "E_PROOF_ORDER",
    );
    refuses(
        &with_add(
            "theorem a(n : Nat) : add(n, Zero) = n { rfl }\ntheorem b(n : Nat) : add(n, Zero) = n { a(n) }",
        ),
        "E_PROOF_UNCHECKED",
    );
}

#[test]
fn and_or_not_use_tuples_cases_and_lambdas() {
    let base = with_add("theorem l(n : Nat) : add(Zero, n) = n { rfl }\n");
    good(&format!(
        "{base}theorem t(n : Nat) : add(Zero, n) = n and n = n {{ (l(n), rfl) }}"
    ));
    good(
        &format!("{base}theorem t(n : Nat) : n = n {{ let (p, q) := (l(n), l(n))\nrfl }}")
            .replace("let (p, q) := (l(n), l(n))", "let p := l(n)"),
    );
    good(&format!(
        "{base}theorem t(n : Nat) : n = n or n = Zero {{ Left(rfl) }}"
    ));
    good(&format!(
        "{base}theorem t(k : Nat) : not (Zero = Succ(k)) requires room : k < 18446744073709551615 {{ h => absurd(h) }}"
    ));
    good(&format!(
        "{base}theorem t(n : Nat) : n = n requires h : n = Zero or not (n = Zero) {{ match h {{ Left(p) => rfl, Right(q) => rfl, }} }}"
    ));
    refuses(
        &format!("{base}theorem t(n : Nat) : n = n {{ (rfl, rfl) }}"),
        "E_PROOF_SHAPE",
    );
    refuses(
        &format!("{base}theorem t(n : Nat) : n = n {{ Left(rfl) }}"),
        "E_PROOF_SHAPE",
    );
}

#[test]
fn symm_trans_congr_arg_absurd_and_decide() {
    let base = with_add(
        "theorem r(n : Nat) : add(n, Zero) = n { induction n { Zero => rfl, Succ(k) ih => congrArg(Succ, ih), } }\n",
    );
    good(&format!(
        "{base}theorem t(n : Nat) : n = add(n, Zero) {{ symm(r(n)) }}"
    ));
    good(&format!(
        "{base}theorem t(n : Nat) : add(add(n, Zero), Zero) = n {{ trans(r(add(n, Zero)), r(n)) }}"
    ));
    good(&format!(
        "{base}theorem t(n : Nat) : add(2, add(n, Zero)) = add(2, n) requires room : n <= 18446744073709551613 {{ congrArg(x => add(2, x), r(n)) }}"
    ));
    good("theorem t() : 3 > 2 and not (1 = 2) { decide }");
    refuses("theorem t() : 2 > 3 { decide }", "E_PROOF_FALSE");
    refuses(
        &with_add("theorem t(n : Nat) : add(n, Zero) = n { decide }"),
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem t(k : Nat) : k = Zero requires h : k = k { absurd(h) }",
        "E_PROOF_SHAPE",
    );
}

#[test]
fn assumptions_must_be_established_where_a_theorem_is_cited() {
    let base = with_add(
        "theorem z(n : Nat) : add(n, Zero) = Zero requires h : n = Zero { trans(congrArg(x => add(x, Zero), h), rfl) }\n",
    );
    good(&format!(
        "{base}theorem t(m : Nat) : add(m, Zero) = Zero requires e : m = Zero {{ z(m) }}"
    ));
    good(&format!(
        "{base}theorem t() : add(Zero, Zero) = Zero {{ z(Zero) }}"
    ));
    refuses(
        &format!("{base}theorem t(m : Nat) : add(m, Zero) = Zero {{ z(m) }}"),
        "E_PROOF_ASSUMPTION",
    );
}

#[test]
fn case_splits_cover_every_case_and_induction_names_its_hypothesis() {
    refuses(
        &with_add("theorem t(n : Nat) : add(n, Zero) = n { match n { Zero => rfl, } }"),
        "E_PROOF_CASES",
    );
    good(
        "enum Color { Red, Green }\nfunction flip(c : Color) : Color = match c { Red => Green, Green => Red, }\ntheorem twice(c : Color) : flip(flip(c)) = c { match c { Red => rfl, Green => rfl, } }",
    );
    // An assumption about n holds for ih at k when known facts give it (Q81).
    good(&with_add(
        "theorem t(n : Nat) : add(n, Zero) = n requires h : n = n { induction n { Zero => rfl, Succ(k) ih => congrArg(Succ, ih), } }",
    ));
}

#[test]
fn statements_may_name_only_pure_structurally_recursive_functions() {
    refuses(
        "function noisy(n : Nat) : Nat with Effects { print(1)\nn }\ntheorem t(n : Nat) : noisy(n) = n { rfl }",
        "E_PROOF_OPAQUE",
    );
    refuses(
        "function loop(n : Nat) : Nat = loop(n)\ntheorem t(n : Nat) : loop(n) = n { rfl }",
        "E_PROOF_OPAQUE",
    );
    refuses(
        "function a(n : Nat) : Nat = match n { Zero => Zero, Succ(k) => b(k), }\nfunction b(n : Nat) : Nat = match n { Zero => Zero, Succ(k) => a(k), }\ntheorem t(n : Nat) : a(n) = a(n) { rfl }",
        "E_PROOF_OPAQUE",
    );
    refuses(
        "theorem t(s : String) : s = s { rfl }",
        "E_PROOF_UNSUPPORTED",
    );
    refuses(
        "theorem t(n : Nat, i : Int) : n = i { rfl }",
        "E_PROOF_TYPE",
    );
}

#[test]
fn simp_closes_definitional_equalities_without_rules() {
    good("theorem t(n : Nat) : n = n { simp }");
    good(&with_add(
        "theorem t(n : Nat) : add(2,n) = Succ(Succ(n)) requires room : n <= 18446744073709551613 { simp }",
    ));
    refuses(
        &with_add("theorem t(n : Nat) : add(n,Zero) = n { simp }"),
        "E_PROOF_MISMATCH",
    );
}

const ZERO_RIGHT: &str = "theorem zeroRight(n : Nat) : add(n,Zero) = n { induction n { Zero => rfl, Succ(k) ih => congrArg(Succ,ih), } }\n";

#[test]
fn simp_uses_marked_equations_and_rewrites_nested_terms_on_both_sides() {
    good(&with_add(&format!(
        "@simp\n{ZERO_RIGHT}theorem t(n : Nat) : add(add(n,Zero),Zero) = add(n,Zero) {{ simp }}"
    )));
    good(&with_add(&format!(
        "@simp\n{ZERO_RIGHT}theorem t(n : Nat) : Succ(n) = Succ(add(n,Zero)) requires room : n < 18446744073709551615 {{ simp }}"
    )));
    good(&with_add(&format!(
        "{ZERO_RIGHT}theorem t(n : Nat) : add(n,Zero) = n {{ simp [zeroRight] }}"
    )));
    refuses(
        &with_add(&format!(
            "{ZERO_RIGHT}theorem t(n : Nat) : add(n,Zero) = n {{ simp }}"
        )),
        "E_PROOF_MISMATCH",
    );
}

#[test]
fn simp_can_use_an_explicit_local_equation_and_induction_hypothesis() {
    good(&with_add(
        "theorem t(n : Nat) : add(n,Zero) = n { induction n { Zero => simp, Succ(k) ih => simp [ih], } }",
    ));
    good(&with_add(
        "theorem t(n : Nat) : add(n,Zero) = Zero requires h : n = Zero { simp [h] }",
    ));
}

#[test]
fn simp_requires_conditional_lemmas_assumptions() {
    let conditional = "theorem z(n : Nat) : add(n,Zero) = Zero requires h : n = Zero { trans(congrArg(x => add(x,Zero),h),rfl) }\n";
    good(&with_add(&format!(
        "{conditional}theorem t(n : Nat) : add(n,Zero) = Zero requires h : n = Zero {{ simp [z] }}"
    )));
    refuses(
        &with_add(&format!(
            "{conditional}theorem t(n : Nat) : add(n,Zero) = Zero {{ simp [z] }}"
        )),
        "E_PROOF_MISMATCH",
    );
}

#[test]
fn simp_never_uses_unchecked_or_later_theorems_or_itself() {
    refuses(
        "theorem t(n : Nat) : n = n { simp [missing] }",
        "E_PROOF_UNKNOWN",
    );
    refuses(
        "@simp\ntheorem t(n : Nat) : n = n { simp [t] }",
        "E_PROOF_RECURSION",
    );
    refuses(
        "theorem t(n : Nat) : n = n { simp [later] }\ntheorem later(n : Nat) : n = n { rfl }",
        "E_PROOF_ORDER",
    );
    refuses(
        "@simp\ntheorem bad() : 1 = 2 { rfl }\ntheorem t() : 1 = 2 { simp [bad] }",
        "E_PROOF_UNCHECKED",
    );
}

#[test]
fn simp_rejects_non_equations_and_undetermined_parameters() {
    refuses(
        "enum Empty {}\ntheorem vacuous(e : Empty) : Zero = Succ(Zero) {match e {}}\ntheorem falseGoal() : Zero = Succ(Zero) {simp [vacuous]}",
        "E_PROOF_SIMP_RULE",
    );
    refuses(
        "theorem unused(n : Nat) : Zero = Zero {rfl}\ntheorem t() : Zero = Zero {simp [unused]}",
        "E_PROOF_SIMP_RULE",
    );
    refuses(
        "@simp\ntheorem positive() : 3 > 2 { decide }",
        "E_PROOF_SIMP_RULE",
    );
    refuses(
        "theorem positive() : 3 > 2 { decide }\ntheorem t() : 1 = 1 { simp [positive] }",
        "E_PROOF_SIMP_RULE",
    );
    refuses(
        "theorem unused(n : Nat, m : Nat) : n = n requires h : m = m { rfl }\ntheorem t(n : Nat) : n = n { simp [unused] }",
        "E_PROOF_SIMP_RULE",
    );
}

#[test]
fn simp_does_not_apply_an_outer_hypothesis_to_a_shadowed_match_binder() {
    refuses(
        "theorem bad(n : Nat,m : Nat) : (match m {Zero => Zero,Succ(n) => n,}) = (match m {Zero => Zero,Succ(n) => Zero,}) requires h : n = Zero {simp [h]}",
        "E_PROOF_MISMATCH",
    );
}

#[test]
fn simp_does_not_instantiate_nat_rules_at_an_int_variable() {
    good(&with_add(&format!(
        "@simp\n{ZERO_RIGHT}theorem t(n : Int) : n = n {{ simp }}"
    )));
    refuses(
        &with_add(&format!(
            "@simp\n{ZERO_RIGHT}theorem t(n : Int) : n = 0 {{ simp }}"
        )),
        "E_PROOF_MISMATCH",
    );
}

#[test]
fn simp_bounds_expanding_and_cyclic_rules() {
    let right = "theorem expand(n : Nat) : n = add(n,Zero) { symm(zeroRight(n)) }\n";
    refuses(
        &with_add(&format!(
            "{ZERO_RIGHT}{right}theorem t(n : Nat) : n = n {{ simp [expand] }}"
        )),
        "E_PROOF_LIMIT",
    );
    refuses(
        &with_add(&format!(
            "{ZERO_RIGHT}{right}theorem t(n : Nat) : n = n {{ simp [zeroRight,expand] }}"
        )),
        "E_PROOF_LIMIT",
    );
}

const ONLY_ZERO: &str = "theorem atZero(n : Nat) : n = Zero requires h : n = Zero { h }\nfunction onlyZero(n : Nat) : Nat requires atZero(n) { n }\n";

#[test]
fn theorem_requirements_need_no_error_result_and_are_checked_at_calls() {
    good(&format!("{ONLY_ZERO}let n := onlyZero(Zero)"));
    good(&format!(
        "{ONLY_ZERO}function guarded(n : Nat) : Nat = if n == Zero then onlyZero(n) else n"
    ));
    good(&format!(
        "{ONLY_ZERO}function guarded(n : Nat) : Nat = match n {{Zero => onlyZero(n),Succ(k) => k,}}"
    ));
    good(&format!(
        "{ONLY_ZERO}function forward(n : Nat) : Nat requires atZero(n) {{ onlyZero(n) }}"
    ));
    good(&format!(
        "{ONLY_ZERO}function defaulted(n : Nat = Zero) : Nat requires atZero(n) {{n}}\nlet n := defaulted()"
    ));
    refuses(
        &format!("{ONLY_ZERO}let n := onlyZero(1)"),
        "E_PROOF_ASSUMPTION",
    );
    refuses(
        &format!("{ONLY_ZERO}function unguarded(n : Nat) : Nat = onlyZero(n)"),
        "E_PROOF_ASSUMPTION",
    );
}

#[test]
fn a_required_theorems_conclusion_can_discharge_a_body_contract() {
    good(&with_add(&format!(
        "{ZERO_RIGHT}function checked(n : Nat) : Nat or Error requires add(n,Zero) == n {{n}}\nfunction useFact(n : Nat) : Nat or Error requires zeroRight(n) {{checked(n)}}"
    )));
}

#[test]
fn theorem_requirements_check_named_arguments_types_and_checked_evidence() {
    good(&format!("{ONLY_ZERO}let n := onlyZero(n = Zero)"));
    refuses(
        "theorem bad(n : Nat) : n = Zero { rfl }\nfunction f(n : Nat) : Nat requires bad(n) {n}",
        "E_PROOF_UNCHECKED",
    );
    refuses(
        &format!("{ONLY_ZERO}function f(n : Int) : Int requires atZero(n) {{n}}"),
        "E_PROOF_TYPE",
    );
    refuses(
        &format!("{ONLY_ZERO}function f(n : Nat) : Nat requires atZero() {{n}}"),
        "E_PROOF_SHAPE",
    );
    refuses(
        &format!(
            "{ONLY_ZERO}function noisy(n : Nat) : Nat with Effects {{print(n)\nn}}\nfunction f(n : Nat) : Nat requires atZero(noisy(n)) {{n}}"
        ),
        "E_PROOF_OPAQUE",
    );
}

#[test]
fn negated_boolean_theorem_assumptions_do_not_loop() {
    good(
        "theorem no(b : Bool) : not b requires h : not b {h}\nfunction checked(b : Bool) : Bool requires no(b) {b}\nfunction f(b : Bool) : Bool = if !b then checked(b) else b",
    );
    refuses(
        "theorem no(b : Bool) : not b requires h : not b {h}\nfunction checked(b : Bool) : Bool requires no(b) {b}\nfunction f(b : Bool) : Bool = checked(b)",
        "E_PROOF_ASSUMPTION",
    );
}

#[test]
fn conditional_theorem_requirements_cannot_disappear_through_function_values() {
    refuses(
        &format!("{ONLY_ZERO}let saved : Nat -> Nat = onlyZero\nlet n := saved(1)"),
        "E_PROOF_FUNCTION_VALUE",
    );
    refuses(
        &format!(
            "{ONLY_ZERO}function identity(n : Nat) : Nat = n\nlet saved := if true then onlyZero else identity\nlet n := saved(1)"
        ),
        "E_PROOF_FUNCTION_VALUE",
    );
    refuses(
        &format!("{ONLY_ZERO}let saved := onlyZero\nlet n := saved(1)"),
        "E_PROOF_FUNCTION_VALUE",
    );
    refuses(
        &format!("{ONLY_ZERO}let saved := onlyZero(_)\nlet n := saved(1)"),
        "E_PROOF_ASSUMPTION",
    );
    good(&with_add(&format!(
        "{ZERO_RIGHT}function unrestricted(n : Nat) : Nat requires zeroRight(n) {{n}}\nlet saved := unrestricted\nlet n := saved(3)"
    )));
}

#[test]
fn theorem_requirements_respect_overloads_pipes_and_invalidated_facts() {
    good(&format!(
        "{ONLY_ZERO}function onlyZero(n : Int) : Int = n\nlet n := onlyZero(2)\nlet z : Nat = Zero |> onlyZero"
    ));
    refuses(
        &format!(
            "{ONLY_ZERO}function f(n : Nat) : Nat {{let mut value := n\nif value == Zero {{value=1\nreturn onlyZero(value)}}\nn}}"
        ),
        "E_PROOF_ASSUMPTION",
    );
    refuses(
        &format!("{ONLY_ZERO}function use(f : Nat -> Nat) : Nat = f(1)\nlet n := use(onlyZero)"),
        "E_PROOF_FUNCTION_VALUE",
    );
}

#[test]
fn the_standard_nat_cannot_be_redefined_and_numerals_are_nats() {
    refuses("enum Nat { Zero, Succ(Nat) }", "E_DUPLICATE_NAME");
    good(
        "let n : Nat = 3\nlet bigger : Bool = n > 2\nlet same : Bool = n == 3\nlet next : Nat = Succ(n)",
    );
}

fn has_z3() -> bool {
    std::process::Command::new("z3")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn smt_proves_bounded_arithmetic_and_refuses_false_goals() {
    if !has_z3() {
        return;
    }
    good("theorem positive(n : Int) : n > 0 requires h : n >= 1 {smt}");
    good("theorem minimum() : -9223372036854775808 < 0 {smt}");
    good("theorem negate(n : Int) : (-n) = (-n) {smt}");
    good("theorem step(n : Int) : (n + 1) > n requires h : n < 9223372036854775807 {smt}");
    good(
        "theorem nonnegative(n : Int) : (n * n) >= 0 requires lo : n >= -10 requires hi : n <= 10 {smt}",
    );
    refuses(
        "theorem falseGoal(n : Int) : n > 0 {smt}",
        "E_PROOF_SMT_FAILED",
    );
    // The integers have no largest value (REVIEW 2.8).
    good("theorem successor(n : Int) : (n + 1) > n {smt}");
    good("theorem overflow() : (9223372036854775807 + 1) = (9223372036854775807 + 1) {smt}");
}

#[test]
fn smt_checks_nat_underflow_and_short_circuit_definedness() {
    if !has_z3() {
        return;
    }
    good("theorem identity(n : Nat) : (n + 0) = n {smt}");
    good("theorem same(n : Nat) : (1 + n) = (n + 1) requires h : n < 18446744073709551615 {smt}");
    good("theorem nonnegative(n : Nat) : n >= Zero {induction n {Zero => smt,Succ(k) ih => smt,}}");
    good("theorem previous(n : Nat) : (if n == Zero then Zero else n - 1) <= n {smt}");
    good("theorem short(n : Nat) : (n == 18446744073709551615) || ((n + 1) > n) {smt}");
    refuses(
        "theorem underflow(n : Nat) : (n - 1) < n {smt}",
        "E_PROOF_SMT_FAILED",
    );
    good("theorem overflow(n : Nat) : Succ(n) > n {smt}");
    good("theorem successor(n : Nat) : Succ(n) > n requires h : n < 18446744073709551615 {smt}");
}

#[test]
fn smt_refuses_recursive_calls_it_cannot_encode() {
    if !has_z3() {
        return;
    }
    refuses(
        &with_add("theorem t(n : Nat) : add(n,Zero) = n {smt}"),
        "E_PROOF_SMT_UNSUPPORTED",
    );
    refuses(
        "function discarded(n : Nat) : Nat {let unused := n + 1\nZero}\ntheorem unsafeProof(n : Nat) : discarded(n) = Zero {smt}",
        "E_PROOF_SMT_UNSUPPORTED",
    );
    good("theorem bools(a : Bool,b : Bool) : (a && b) = (b && a) {smt}");
}

#[test]
fn transparent_wrappers_cannot_hide_opaque_or_nonterminating_functions() {
    good("theorem qualified() : Nat.Zero = Zero {rfl}");
    refuses(
        "theorem bad() : wrong.Nat.Zero = Zero {rfl}",
        "E_PROOF_UNSUPPORTED",
    );
    refuses(
        "function loop(n : Nat) : Nat = loop(n)\nfunction inner(n : Nat) : Nat = loop(n)\nfunction wrapper(n : Nat) : Nat = inner(n)\ntheorem bad(n : Nat) : wrapper(n) = wrapper(n) {rfl}",
        "E_PROOF_OPAQUE",
    );
    refuses(
        "function identity(n : Nat) : Nat = n\ntheorem bad(n : Nat) : wrong.identity(n) = n {rfl}",
        "E_PROOF_OPAQUE",
    );
    refuses(
        "theorem bad(n : Nat) : wrong.Nat.Succ(n) = Succ(n) {rfl}",
        "E_PROOF_OPAQUE",
    );
}

#[test]
fn explicit_proofs_require_defined_checked_arithmetic() {
    good(include_str!(
        "../testdata/language/43-defined-proof-arithmetic.kes"
    ));
    for tactic in ["rfl", "simp"] {
        // Overflow is not a proof obligation: proofs use the integers (REVIEW 2.8).
        good(&format!(
            "theorem same(n : Int) : (n + 1) = (n + 1) {{{tactic}}}"
        ));
        good(&format!(
            "theorem overflow() : (9223372036854775807 + 1) = (9223372036854775807 + 1) {{{tactic}}}"
        ));
        refuses(
            &format!("theorem underflow(n : Nat) : (n - 1) = (n - 1) {{{tactic}}}"),
            "E_PROOF_UNDEFINED",
        );
        good(&format!(
            "theorem same(n : Int) : (n + 1) = (n + 1) requires room : n < 9223372036854775807 {{{tactic}}}"
        ));
        good(&format!(
            "theorem previous(n : Nat) : (n - 1) = (n - 1) requires positive : n > Zero {{{tactic}}}"
        ));
    }
    good("theorem negate(n : Int) : (-n) = (-n) {rfl}");
    good("theorem negate(n : Int) : (-n) = (-n) requires room : n > -9223372036854775808 {rfl}");
    good("theorem subtract(n : Int) : (n - n) = 0 {smt}");
    good("theorem safe(n : Int) : (n - n) = (n - n) {rfl}");
    good("theorem successor(n : Nat) : Succ(n) = Succ(n) {rfl}");
}

#[test]
fn unfolded_functions_preserve_unused_checked_work() {
    // An unused Nat subtraction still has to be defined; Int overflow does not
    // count in a proof (REVIEW 2.8).
    good(
        "function discarded(n : Int) : Int {let unused := n + 1\n0}\ntheorem fine() : discarded(9223372036854775807) = 0 {rfl}",
    );
    let function = "function discarded(n : Nat) : Nat {let unused := n - 1\n0}\n";
    for proof in ["rfl", "simp", "decide"] {
        refuses(
            &format!("{function}theorem bad() : discarded(0) = 0 {{{proof}}}"),
            "E_PROOF_UNDEFINED",
        );
        good(&format!(
            "{function}theorem fine() : discarded(3) = 0 {{{proof}}}"
        ));
    }
    refuses(
        &format!("{function}theorem bad(n : Nat) : discarded(n) = discarded(n) {{rfl}}"),
        "E_PROOF_UNDEFINED",
    );
    good(&format!(
        "{function}theorem fine(n : Nat) : discarded(n) = 0 requires positive : n > Zero {{rfl}}"
    ));
    refuses(
        &format!(
            "{function}theorem identity(n : Nat) : n = n {{rfl}}\ntheorem bad(n : Nat) : discarded(n) = 0 {{identity(0)}}"
        ),
        "E_PROOF_UNDEFINED",
    );
    refuses(
        &format!(
            "{function}theorem identity(n : Nat) : n = n {{rfl}}\ntheorem bad(n : Nat) : discarded(n) = 0 {{trans(identity(0),rfl)}}"
        ),
        "E_PROOF_UNDEFINED",
    );
}

#[test]
fn definedness_follows_branches_and_short_circuit_evaluation() {
    good(
        "theorem same(n : Nat) : (if n == Zero then Zero else n - 1) = (if n == Zero then Zero else n - 1) {rfl}",
    );
    good(
        "theorem same(n : Int) : ((n == 9223372036854775807) || ((n + 1) > n)) = ((n == 9223372036854775807) || ((n + 1) > n)) {rfl}",
    );
    good(
        "theorem same(n : Nat) : ((n != Zero) && ((n - 1) < n)) = ((n != Zero) && ((n - 1) < n)) {simp}",
    );
    refuses(
        "theorem bad(n : Nat) : ((n == Zero) && ((n - 1) < n)) = ((n == Zero) && ((n - 1) < n)) {rfl}",
        "E_PROOF_UNDEFINED",
    );
    refuses(
        "theorem bad(n : Nat) : n = n or (n - 1) = (n - 1) {Left(rfl)}",
        "E_PROOF_UNDEFINED",
    );
    refuses(
        "theorem bad(n : Int) : not ((n + 1) = (n + 1)) {h => absurd(h)}",
        "E_PROOF_SHAPE",
    );
}

#[test]
fn theorem_arguments_and_assumptions_cannot_hide_undefined_work() {
    let identity = "theorem identity(n : Int) : 0 = 0 {rfl}\n";
    good(&format!(
        "{identity}theorem fine(n : Int) : 0 = 0 {{identity(n + 1)}}"
    ));
    refuses(
        "theorem natural(n : Nat) : 0 = 0 {rfl}\ntheorem bad(n : Nat) : 0 = 0 {natural(n - 1)}",
        "E_PROOF_UNDEFINED",
    );
    good(&format!(
        "{identity}theorem fine(n : Int) : 0 = 0 requires room : n < 9223372036854775807 {{identity(n + 1)}}"
    ));
    // An assumption carries its own definedness inside the proof (Q82), but a
    // function cannot require a theorem that relies on that.
    good("theorem circular(n : Int) : (n + 1) = (n + 1) requires badBound : (n + 1) > n {rfl}");
    refuses(
        "theorem circular(n : Int) : (n + 1) = (n + 1) requires badBound : (n + 1) > n {rfl}\nfunction f(n : Int) : Int requires circular(n) {n}",
        "E_PROOF_UNDEFINED",
    );
    good(
        "theorem ordered(n : Int) : (n + 1) = (n + 1) requires room : n < 9223372036854775807 requires safe : (n + 1) > n {rfl}",
    );
    refuses(
        &format!("{identity}function bad(n : Int) : Int requires identity(n + 1) {{n}}"),
        "E_PROOF_UNDEFINED",
    );
    good(&format!(
        "{identity}function fine(n : Int) : Int or Error requires n < 9223372036854775807 requires identity(n + 1) {{n}}"
    ));
}

#[test]
fn definedness_matches_checked_integer_operations_at_boundaries() {
    let verify = |source: String, succeeds: bool| {
        let parsed = parse_file("boundary.kes", &source);
        let checked = check(&parsed);
        assert_eq!(
            checked.is_ok(),
            succeeds,
            "{source}\n{:#?}",
            checked.diagnostics
        );
        if !succeeds {
            assert!(
                checked
                    .diagnostics
                    .iter()
                    .any(|d| d.code == "E_PROOF_UNDEFINED")
            );
        }
    };
    // A proof uses the integers and the naturals (REVIEW 2.8): Int arithmetic and Nat
    // addition never fail there, even at the 64-bit limits. Nat subtraction below
    // zero still does.
    for value in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX] {
        for other in [-2_i64, -1, 0, 1, 2] {
            for op in ["+", "-", "*"] {
                verify(
                    format!(
                        "theorem boundary(n : Int) : (n {op} ({other})) = (n {op} ({other})) requires h : n = {value} {{rfl}}"
                    ),
                    true,
                );
            }
        }
    }
    for value in [0_u64, 1, 2, u64::MAX - 1, u64::MAX] {
        for other in [0_u64, 1, 2, u64::MAX] {
            for (op, result) in [("+", Some(0)), ("-", value.checked_sub(other))] {
                verify(
                    format!(
                        "theorem boundary(n : Nat) : (n {op} {other}) = (n {op} {other}) requires h : n = {value} {{rfl}}"
                    ),
                    result.is_some(),
                );
            }
        }
    }
}

#[test]
fn proof_definition_preparation_bounds_repeated_substitution() {
    let mut function = String::from("function expanded(n : Int) : Int {let x0 := n\n");
    for i in 1..30 {
        function.push_str(&format!("let x{i} := x{} + x{}\n", i - 1, i - 1));
    }
    function.push_str("x29}\n");
    good(&function);
    refuses(
        &format!("{function}theorem t() : expanded(0) = 0 {{rfl}}"),
        "E_PROOF_OPAQUE",
    );
}

#[test]
fn proof_terms_check_constructor_conditions_and_omitted_arguments() {
    good(
        "function value(n : Int,m : Int=n) : Int=m\ntheorem same(n : Int,m : Int) : value(n,m)=m {rfl}",
    );
    refuses(
        "function value(n : Int,m : Int=n) : Int=m\ntheorem omitted() : value(1)=1 {rfl}",
        "E_PROOF_UNKNOWN",
    );
    refuses(
        "theorem positive(n : Int) : n > 0 requires h : n > 0 {h}\nfunction guarded(n : Int,m : Int=n) : Int requires positive(m)=m\nguarded(1)",
        "E_PROOF_ASSUMPTION",
    );
    refuses(
        "theorem bad(i : Int) : Succ(i) = Succ(i) {rfl}",
        "E_PROOF_TYPE",
    );
    refuses(
        "theorem bad(i : Int) : (if i then Zero else Zero) = Zero {rfl}",
        "E_PROOF_TYPE",
    );
    refuses(
        "theorem bad(n : Nat) : n = n {match n {Zero => rfl,Succ() => rfl,}}",
        "E_PROOF_SHAPE",
    );
    refuses(
        "theorem same(i : Int) : i = i {rfl}\ntheorem bad() : Zero = Zero {let h := congrArg(Succ,same(0))\nrfl}",
        "E_PROOF_TYPE",
    );
    let default = "function f(n : Int = 9223372036854775807 + 1) : Int = 0\n";
    refuses(
        &format!("{default}function wrapper() : Int = f()\ntheorem bad() : wrapper() = 0 {{rfl}}"),
        "E_PROOF_OPAQUE",
    );
    refuses(
        &format!("{default}theorem bad() : f() = 0 {{rfl}}"),
        "E_PROOF_UNKNOWN",
    );
    good(&format!("{default}theorem fine() : f(0) = 0 {{rfl}}"));
    good(
        "function three() : Nat {let one : Nat = 1\none + 2}\ntheorem t() : three() = Succ(Succ(Succ(Zero))) {rfl}",
    );
}

#[test]
fn functions_that_require_theorems_unfold_where_the_assumptions_hold() {
    let zero = "theorem zero(n : Nat) : n = Zero requires h : n = Zero { h }\nfunction onlyZero(n : Nat) : Nat requires zero(n) { n }\n";
    good(&format!(
        "{zero}theorem a() : onlyZero(Zero) = Zero {{ rfl }}"
    ));
    good(&format!(
        "{zero}theorem b(n : Nat) : onlyZero(n) = n requires h : n = Zero {{ rfl }}"
    ));
    good(&format!(
        "{zero}function wrap(n : Nat) : Nat requires zero(n) {{ onlyZero(n) }}\ntheorem c(n : Nat) : wrap(n) = n requires h : n = Zero {{ rfl }}"
    ));
    refuses(
        &format!("{zero}theorem d(n : Nat) : onlyZero(n) = n {{ rfl }}"),
        "E_PROOF_ASSUMPTION",
    );
    refuses(
        &format!("{zero}theorem e() : onlyZero(Succ(Zero)) = Succ(Zero) {{ rfl }}"),
        "E_PROOF_ASSUMPTION",
    );
    // A universal requirement holds at every call.
    good(&with_add(
        "theorem addZeroR(n : Nat) : add(n, Zero) = n { induction n { Zero => rfl, Succ(k) ih => congrArg(Succ, ih), } }\nfunction count(n : Nat) : Nat requires addZeroR(n) { add(n, Zero) }\ntheorem f(n : Nat) : count(Zero) = Zero { rfl }",
    ));
    // A run-time precondition is an obligation at each call (Q80).
    refuses(
        "function pred(n : Nat) : Nat or Error requires n > 0 { n - 1 }\ntheorem t(k : Nat) : pred(k) = pred(k) { rfl }",
        "E_PROOF_PRECONDITION",
    );
}

const CHAIN: &str = "enum Chain[T] {\n    End\n    Link(T, Chain[T])\n}\nfunction append(move xs : Chain[T], move ys : Chain[T]) : Chain[T] = match xs {\n    End => ys,\n    Link(x, rest) => Link(x, append(rest, ys)),\n}\nfunction size(xs : Chain[T]) : Nat = match xs {\n    End => Zero,\n    Link(x, rest) => Succ(size(rest)),\n}\n@simp theorem appendEnd(xs : Chain[T]) : append(xs, End) = xs {\n    induction xs {\n        End => rfl,\n        Link(x, rest) ih => congrArg(r => Link(x, r), ih),\n    }\n}\n";

#[test]
fn generic_theorems_quantify_over_implicit_type_variables() {
    good(CHAIN);
    good(&format!(
        "{CHAIN}theorem nats(xs : Chain[Nat]) : append(xs, End) = xs {{ appendEnd(xs) }}"
    ));
    good(&format!(
        "{CHAIN}theorem twice(xs : Chain[T]) : append(append(xs, End), End) = xs {{ simp }}"
    ));
    good(&format!(
        "{CHAIN}theorem one(x : T) : append(Link(x, End), End) = Link(x, End) {{ rfl }}"
    ));
    good(&format!(
        "{CHAIN}theorem sized(x : Nat) : size(Link(x, Link(x, End))) = 2 {{ rfl }}"
    ));
    good(&format!(
        "{CHAIN}theorem lifted(a : Chain[Bool], b : Chain[Bool]) : Link(true, append(a, End)) = Link(true, a) {{ congrArg(r => Link(true, r), appendEnd(a)) }}"
    ));
    refuses(
        &format!("{CHAIN}theorem bad(xs : Chain[T]) : append(xs, xs) = xs {{ rfl }}"),
        "E_PROOF_MISMATCH",
    );
    // A theorem about every T cannot treat T as Nat.
    refuses("theorem bad(x : T) : x = Zero { rfl }", "E_PROOF_TYPE");
    refuses(
        &format!("{CHAIN}theorem bad(x : T, n : Nat) : Link(x, End) = Link(n, End) {{ rfl }}"),
        "E_PROOF_TYPE",
    );
    // A citation fills T once for every argument.
    refuses(
        &format!(
            "{CHAIN}theorem same(xs : Chain[T], ys : Chain[T]) : append(xs, ys) = append(xs, ys) {{ rfl }}\ntheorem mix(a : Chain[Nat], b : Chain[Int]) : append(a, End) = append(a, End) {{ same(a, b) }}"
        ),
        "E_PROOF_TYPE",
    );
    refuses(
        &format!(
            "{CHAIN}theorem cases(xs : Chain[T]) : append(xs, End) = xs {{ match xs {{ End => rfl, }} }}"
        ),
        "E_PROOF_CASES",
    );
}

#[test]
fn generic_trait_method_calls_stay_opaque() {
    refuses(
        "trait Sized { function size() : Nat }\nfunction measure[S : Sized](s : S) : Nat = s.size()\ntheorem t[S : Sized](s : S) : measure(s) = measure(s) { rfl }",
        "E_PROOF_OPAQUE",
    );
}

const BOUNDED_IDENTITY: &str = "function identity[T : Equals](x : T) : T = x\ntheorem same[T : Equals](x : T) : identity(x) = x {rfl}\n";

#[test]
fn generic_proofs_check_function_and_citation_bounds() {
    good(&format!(
        "{BOUNDED_IDENTITY}theorem natural(n : Nat) : identity(n) = n {{same(n)}}\ntheorem renamed[U : Equals](x : U) : identity(x) = x {{same(x)}}"
    ));
    refuses(
        "function identity[T : Equals](x : T) : T = x\ntheorem missing(x : T) : identity(x) = x {rfl}",
        "E_PROOF_BOUND",
    );
    refuses(
        &format!(
            "{BOUNDED_IDENTITY}class Token {{\nnew() {{}}\n}}\ntheorem missing(x : Token) : identity(x) = x {{rfl}}"
        ),
        "E_PROOF_BOUND",
    );
    refuses(
        "theorem same[T : Equals](x : T) : x = x {rfl}\ntheorem missing(x : U) : x = x {same(x)}",
        "E_PROOF_BOUND",
    );
}

#[test]
fn proof_bounds_substitute_associated_types_and_check_every_trait() {
    let source = "function keep[T : Iterable(Item = U)](x : T, y : U) : T = x\n";
    good(&format!(
        "{source}theorem same[T : Iterable(Item = V)](x : T, y : V) : keep(x,y) = x {{rfl}}"
    ));
    refuses(
        &format!(
            "{source}theorem wrong[T : Iterable(Item = Bool)](x : T, y : Int) : keep(x,y) = x {{rfl}}"
        ),
        "E_PROOF_BOUND",
    );
    let source = "function identity[T : Text + Equals](x : T) : T = x\n";
    good(&format!(
        "{source}theorem same[U : Text + Equals](x : U) : identity(x) = x {{rfl}}\ntheorem concrete(n : Nat) : identity(n) = n {{rfl}}"
    ));
    refuses(
        &format!("{source}theorem missing[U : Equals](x : U) : identity(x) = x {{rfl}}"),
        "E_PROOF_BOUND",
    );
}

#[test]
fn runtime_equals_cannot_establish_propositional_equality() {
    refuses(
        "theorem equal[T : Equals](x : T,y : T) : x = y requires h : x = y {h}\nfunction needs[T : Equals](x : T,y : T) : Bool requires equal(x,y) = true\nfunction unsafeFact[T : Equals](x : T,y : T) : Bool = if x == y then needs(x,y) else false",
        "E_PROOF_ASSUMPTION",
    );
    // The bridge still recognizes built-in exact equality and reflexive facts.
    good(
        "theorem equal(x : Int,y : Int) : x = y requires h : x = y {h}\nfunction needs(x : Int,y : Int) : Bool requires equal(x,y) = true\nfunction safe(x : Int,y : Int) : Bool = if x == y then needs(x,y) else false",
    );
    good(
        "theorem equal(x : T) : x = x {rfl}\nfunction safe(x : T) : Bool requires equal(x) = true",
    );
}

#[test]
fn function_requirements_preserve_generic_theorem_bounds() {
    good(&format!(
        "{BOUNDED_IDENTITY}function checked[U : Equals](x : U) : U requires same(x) {{identity(x)}}\nfunction natural(n : Nat) : Nat requires same(n) {{n}}"
    ));
    refuses(
        &format!("{BOUNDED_IDENTITY}function missing(x : U) : U requires same(x) {{x}}"),
        "E_PROOF_BOUND",
    );
}

#[test]
fn bounded_simp_rules_require_their_type_constraints() {
    let source = "enum Chain[T] {End,Link(T,Chain[T])}\nfunction append(move xs : Chain[T], move ys : Chain[T]) : Chain[T] = match xs {End => ys, Link(x,rest) => Link(x,append(rest,ys)),}\n@simp\ntheorem right[T : Equals](xs : Chain[T]) : append(xs,End) = xs {induction xs {End => rfl,Link(x,rest) ih => congrArg(t => Link(x,t),ih),}}\n";
    good(&format!(
        "{source}theorem again[U : Equals](xs : Chain[U]) : append(xs,End) = xs {{simp}}"
    ));
    let parsed = parse_file(
        "proof.kes",
        format!("{source}theorem missing(xs : Chain[U]) : append(xs,End) = xs {{simp}}"),
    );
    let bad = check(&parsed);
    assert!(
        !bad.is_ok(),
        "A bounded rewrite was applied without its bound."
    );
}

#[test]
fn proofs_cannot_ignore_a_function_postcondition_that_may_fail() {
    refuses(
        "function f() : Nat ensures false = 0\ntheorem falsePromise() : f() = 0 {rfl}",
        "E_PROOF_OPAQUE",
    );
}

#[test]
fn only_functions_that_cannot_fail_skip_the_definedness_check() {
    good(&format!(
        "{CHAIN}theorem same(xs : Chain[T], ys : Chain[T]) : append(xs, ys) = append(xs, ys) {{ rfl }}"
    ));
    let partial = "function drop(n : Nat) : Nat = n - 1\nfunction wrap(b : Bool, n : Nat) : Nat = if b { drop(n) } else { n }\n";
    good(&format!(
        "{partial}theorem t(n : Nat) : wrap(false, n) = n {{ rfl }}"
    ));
    refuses(
        &format!("{partial}theorem u(b : Bool, n : Nat) : wrap(b, n) = wrap(b, n) {{ rfl }}"),
        "E_PROOF_UNDEFINED",
    );
    // Counting with Succ cannot fail over the naturals.
    good(&format!(
        "{CHAIN}theorem v(xs : Chain[T]) : size(xs) = size(xs) {{ rfl }}"
    ));
}

#[test]
fn calls_with_run_time_preconditions_stand_for_their_success_value() {
    let pred = "function pred(n : Nat) : Nat or Error requires n > 0 = match n {\n    Zero => Zero,\n    Succ(k) => k,\n}\n";
    good(&format!(
        "{pred}theorem predSucc(k : Nat) : pred(Succ(k)) = k requires room : k < 18446744073709551615 {{ rfl }}"
    ));
    good(&format!(
        "{pred}theorem same(n : Nat) : pred(n) = pred(n) requires h : n > 0 {{ rfl }}"
    ));
    good(&format!("{pred}theorem three() : pred(3) = 2 {{ rfl }}"));
    // A wrapper's own condition establishes the inner one.
    good(&format!(
        "{pred}function down(n : Nat) : Nat or Error requires n > 1 = pred(n)\ntheorem d(n : Nat) : down(n) = pred(n) requires h : n > 1 {{ rfl }}"
    ));
    refuses(
        &format!("{pred}theorem zero() : pred(Zero) = Zero {{ rfl }}"),
        "E_PROOF_PRECONDITION",
    );
    refuses(
        &format!(
            "{pred}function down(n : Nat) : Nat or Error requires n > 1 = pred(n)\ntheorem d(n : Nat) : down(n) = pred(n) requires h : n > 0 {{ rfl }}"
        ),
        "E_PROOF_PRECONDITION",
    );
    // The arithmetic form from the question: n - 1 is defined once n > 0 holds.
    good(
        "function before(n : Nat) : Nat or Error requires n > 0 = n - 1\ntheorem b() : before(5) = 4 { rfl }",
    );
}

#[test]
fn definedness_checks_a_stuck_match_arm_by_arm() {
    // In the Succ arm, n is at least one, so n - 1 cannot fail.
    good(
        "function minusOne(n : Nat) : Nat = match n {\n    Zero => Zero,\n    Succ(k) => n - 1,\n}\ntheorem t(n : Nat) : minusOne(n) = minusOne(n) { rfl }",
    );
    // Succ(add(k, b)) cannot fail over the naturals, so a stuck add is defined.
    good(&with_add(
        "theorem t(n : Nat) : add(n, Zero) = add(n, Zero) { rfl }",
    ));
    good(&with_add(
        "theorem t(n : Nat) : add(add(n, Zero), Zero) = add(add(n, Zero), Zero) { rfl }",
    ));
    // A wildcard arm knows nothing about its value.
    refuses(
        "function f(n : Nat) : Nat = match n {\n    Zero => Zero,\n    _ => n - 1,\n}\ntheorem t(n : Nat) : f(n) = f(n) { rfl }",
        "E_PROOF_UNDEFINED",
    );
}

#[test]
fn induction_hypotheses_need_their_assumptions_at_the_smaller_value() {
    good(&with_add(
        "theorem small(n : Nat) : add(n, Zero) = n requires h : n < 1000 { induction n { Zero => rfl, Succ(k) ih => congrArg(Succ, ih), } }",
    ));
    good(&with_add(
        "theorem viaSimp(n : Nat) : add(n, Zero) = n requires h : n < 1000 { induction n { Zero => rfl, Succ(k) ih => simp [ih], } }",
    ));
    // Succ(k) > 5 gives only k > 4, so ih cannot be used.
    refuses(
        &with_add(
            "theorem notEnough(n : Nat) : add(n, Zero) = n requires h : n > 5 { induction n { Zero => rfl, Succ(k) ih => congrArg(Succ, ih), } }",
        ),
        "E_PROOF_ASSUMPTION",
    );
    // An assumption that does not mention n still holds for ih unchanged.
    good(&with_add(
        "theorem other(n : Nat, m : Nat) : add(n, Zero) = n requires h : m < 5 { induction n { Zero => rfl, Succ(k) ih => congrArg(Succ, ih), } }",
    ));
}

#[test]
fn assumptions_carry_their_own_definedness_and_citations_establish_it() {
    let succ = "theorem addSuccR(a : Nat, b : Nat) : add(a, Succ(b)) = Succ(add(a, b)) requires room : add(a, b) < 18446744073709551615 { induction a { Zero => rfl, Succ(k) ih => congrArg(Succ, ih), } }\n";
    good(&with_add(succ));
    good(&with_add(&format!(
        "{succ}theorem use(b : Nat) : add(Zero, Succ(b)) = Succ(add(Zero, b)) requires r : b < 18446744073709551615 {{ addSuccR(Zero, b) }}"
    )));
    refuses(
        &with_add(&format!(
            "{succ}theorem missing(a : Nat, b : Nat) : add(a, Succ(b)) = Succ(add(a, b)) {{ addSuccR(a, b) }}"
        )),
        "E_PROOF_ASSUMPTION",
    );
    // The citation must also show that add(n, n) succeeds, which bounds alone do not.
    refuses(
        &with_add(&format!(
            "{succ}theorem doubled(n : Nat) : add(n, Succ(n)) = Succ(add(n, n)) requires small : n < 10 {{ addSuccR(n, n) }}"
        )),
        "E_PROOF_ASSUMPTION",
    );
    refuses(
        &with_add(&format!(
            "{succ}function f(a : Nat, b : Nat) : Nat requires addSuccR(a, b) {{ a }}"
        )),
        "E_PROOF_UNDEFINED",
    );
}

#[test]
fn readable_records_and_classes_reduce_fields_and_methods() {
    good(
        "record ProofPoint(x : Int, y : Int)\ntheorem pointField(p : ProofPoint) : p.x = p.x { rfl }\ntheorem pointValue() : ProofPoint(2, 3).x = 2 { rfl }\ntheorem everyPoint() : forall p : ProofPoint, p.x = p.x { p => rfl }",
    );
    good(
        "class ProofSquare {\npublic final side : Int\nnew(side : Int) { this.side = side }\npublic function area() : Int = this.side * this.side\n}\ntheorem areaAtTwo() : ProofSquare(2).area() = 4 { rfl }",
    );
    good(
        "class CompactSquare {public final side:Int\nnew(side:Int){this.side=side}\npublic function area():Int=this.side*this.side}\ntheorem compact():CompactSquare(2).area()=4{rfl}",
    );
}

#[test]
fn class_parameter_projections_follow_the_actual_argument() {
    let square = "class ProofSquare {\npublic final side : Int\nnew(side : Int) { this.side = side }\npublic function area() : Int = this.side * this.side\n}\n";
    good(&format!(
        "{square}function field(s : ProofSquare) : Int = s.side\nfunction total(a : ProofSquare, b : ProofSquare) : Int = a.side + b.side\nfunction areaOf(s : ProofSquare) : Int = s.area()\ntheorem constructorField() : field(ProofSquare(3)) = 3 {{ rfl }}\ntheorem twoArguments() : total(ProofSquare(2), ProofSquare(5)) = 7 {{ rfl }}\ntheorem methodReceiver() : areaOf(ProofSquare(4)) = 16 {{ rfl }}"
    ));
    refuses(
        &format!(
            "{square}function field(s : ProofSquare) : Int = s.side\ntheorem noCapture(other : ProofSquare) : field(ProofSquare(3)) = other.side {{ rfl }}"
        ),
        "E_PROOF_MISMATCH",
    );
    good(
        "class Left { public final value : Int\nnew(value : Int) { this.value = value } }\nclass Right { public final value : Int\nnew(value : Int) { this.value = value } }\nfunction total(left : Left, right : Right) : Int = left.value + right.value\ntheorem distinctClasses() : total(Left(2), Right(5)) = 7 { rfl }",
    );
}

#[test]
fn readable_class_methods_unfold_through_smt() {
    let square = "class Square {\npublic final side : Int\nnew(side : Int) { this.side = side }\npublic function area() : Int = this.side * this.side\npublic function scaled(k : Int) : Square = Square(this.side * k)\n}\n";
    good(&format!(
        "{square}theorem areaScales(s : Square, k : Int) : s.scaled(k).area() = k * k * s.area() {{ smt }}"
    ));
}

#[test]
fn trait_defaults_specialize_for_each_readable_implementing_class() {
    let source = "trait Measure {\npublic function value() : Int\npublic function measure() : Int = this.value()\n}\nclass MeasureOne { has Measure\nnew() {}\npublic function value() : Int = 1 }\nclass MeasureTwo { has Measure\nnew() {}\npublic function value() : Int = 2 }\ntheorem one() : MeasureOne().measure() = 1 { rfl }\ntheorem two() : MeasureTwo().measure() = 2 { rfl }";
    good(source);
    refuses(
        &format!(
            "{source}\ntheorem distinct() : MeasureOne().measure() = MeasureTwo().measure() {{ rfl }}"
        ),
        "E_PROOF_MISMATCH",
    );
}

#[test]
fn trait_default_methods_receive_concrete_class_and_record_fields() {
    let source = "trait HasWidth { public function value(k : Int) : Int\npublic function measure(k : Int) : Int = this.value(k) }\nclass WidthClass { has HasWidth\npublic final width : Int\nnew(width : Int) { this.width = width }\npublic function value(k : Int) : Int = this.width + k }\nrecord WidthRecord(width : Int) { has HasWidth\npublic function value(k : Int) : Int = this.width + k }\ntheorem classParam(s : WidthClass, k : Int) : s.measure(k) = s.width + k { rfl }\ntheorem recordParam(r : WidthRecord, k : Int) : r.measure(k) = r.width + k { rfl }\ntheorem classValue() : WidthClass(3).measure(4) = 7 { rfl }\ntheorem recordValue() : WidthRecord(3).measure(4) = 7 { rfl }";
    good(source);
    refuses(
        &format!(
            "{source}\ntheorem unrelated(other : WidthClass, k : Int) : WidthClass(2).measure(k) = other.width + k {{ rfl }}"
        ),
        "E_PROOF_MISMATCH",
    );
}

#[test]
fn private_class_members_remain_private_in_theorems() {
    refuses(
        "class ProofBox { final hidden : Int\nnew(value : Int) { this.hidden = value }\npublic function value() : Int = this.hidden }\ntheorem field(x : ProofBox) : x.hidden = x.hidden { rfl }",
        "E_MEMBER_PRIVATE",
    );
    refuses(
        "class ProofSecret { new() {}\nfunction hidden() : Int = 1 }\ntheorem method(x : ProofSecret) : x.hidden() = 1 { rfl }",
        "E_MEMBER_PRIVATE",
    );
    good(
        "class ProofBox { final hidden : Int\nnew(value : Int) { this.hidden = value }\npublic function value() : Int = this.hidden }\ntheorem publicMethod() : ProofBox(7).value() = 7 { rfl }",
    );
}

#[test]
fn mutable_fields_and_unreadable_constructors_report_the_blocking_member() {
    refuses(
        "class ProofMutable { public var value : Int\nnew(value : Int) { this.value = value } }\ntheorem field(x : ProofMutable) : x.value = x.value { rfl }",
        "E_PROOF_UNSUPPORTED",
    );
    let source = "class ProofEffectful {\npublic final value : Int\nnew(value : Int) with Effects { print(value)\nthis.value = value }\n}\ntheorem field(x : ProofEffectful) : x.value = x.value { rfl }";
    let checked = check(&parse_file("proofs.kes", source));
    let diagnostic = checked
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E_PROOF_UNSUPPORTED")
        .expect("readability failure should name its constructor");
    assert!(diagnostic.message.contains("value"), "{diagnostic:?}");
    assert!(
        diagnostic.message.contains("proofs.kes:3:1"),
        "{diagnostic:?}"
    );
}

#[test]
fn class_projection_keeps_every_eager_constructor_and_method_computation() {
    refuses(
        "class ProofBox { public final n : Nat\nnew(x : Nat) { let unused := x - 1\nthis.n = Zero } }\ntheorem unused() : ProofBox(Zero).n = Zero { rfl }",
        "E_PROOF_UNDEFINED",
    );
    refuses(
        "class ProofPair { public final a : Nat\npublic final b : Nat\nnew(n : Nat) { this.a = Zero\nthis.b = n - 1 } }\ntheorem discarded() : ProofPair(Zero).a = Zero { rfl }",
        "E_PROOF_UNDEFINED",
    );
    refuses(
        "class ProofPair { public final a : Nat\npublic final b : Nat\nnew(n : Nat) { this.a = Zero\nthis.b = n - 1 }\npublic function first() : Nat = this.a }\ntheorem discardedByMethod() : ProofPair(Zero).first() = Zero { rfl }",
        "E_PROOF_UNDEFINED",
    );
}

#[test]
fn readable_constructor_dependencies_are_not_source_ordered() {
    good(
        "class A { public final b : B\nnew(move b : B) { this.b = b } }\nclass B { public final c : C\nnew(move c : C) { this.c = c } }\nclass C { public final d : D\nnew(move d : D) { this.d = d } }\nclass D { public final value : Int\nnew(value : Int) { this.value = value } }\ntheorem read(a : A) : a.b.c.d.value = a.b.c.d.value { rfl }",
    );
}

#[test]
fn inherited_methods_read_fields_from_the_declaring_layout() {
    good(
        "class Grand { public final p : Int = 1\nnew() {}\npublic function getP() : Int = this.p }\nclass Parent extends Grand { public final q : Int = 2\nnew() {} }\nclass Child extends Parent { public final c : Int = 3\nnew() {} }\ntheorem read(x : Child) : x.getP() = x.p { rfl }",
    );
}

#[test]
fn override_safety_tracks_inherited_methods_but_not_overloads() {
    refuses(
        "class ProofBase { new() {}\npublic function value() : Int = 0 }\nclass ProofChild extends ProofBase { new() {}\n@override public function value() : Int = 1 }\ntheorem rejected(x : ProofBase) : x.value() = 0 { rfl }",
        "E_PROOF_OVERRIDDEN",
    );
    refuses(
        "class ProofBase { new() {}\npublic function value() : Int = 0 }\nclass ProofChild extends ProofBase { new() {} }\nclass ProofGrandchild extends ProofChild { new() {}\n@override public function value() : Int = 1 }\ntheorem inherited(x : ProofChild) : x.value() = 0 { rfl }",
        "E_PROOF_OVERRIDDEN",
    );
    good(
        "class ProofBase { new() {}\npublic function value() : Int = 0 }\nclass ProofHidden extends ProofBase { new() {}\npublic function value(n : Int) : Int = n }\ntheorem overloaded(x : ProofBase) : x.value() = 0 { rfl }",
    );
}
