use kestrel::{checker::check, parse_file, proof::goals_at};

const TREE: &str = "enum Tree { Leaf, Node(Tree, Tree) }\nfunction mirror(t : Tree) : Tree = match t { Leaf => Leaf, Node(a,b) => Node(mirror(b),mirror(a)), }\n";
const EXPR: &str = "enum Expr { Lit(Int), Add(Expr, Expr) }\nfunction eval(e : Expr) : Int = match e { Lit(n) => n, Add(a,b) => eval(a) + eval(b), }\nfunction fold(e : Expr) : Expr = match e { Lit(n) => Lit(n), Add(a,b) => Add(fold(a),fold(b)), }\n";

fn checked(source: &str) -> kestrel::checker::Checked {
    let parsed = parse_file("multi.kes", source);
    assert!(parsed.is_ok(), "{:?}", parsed.diagnostics);
    check(&parsed)
}

#[test]
fn both_recursive_fields_receive_hypotheses_in_both_proof_forms() {
    for proof in [
        "{ induction t { Leaf => rfl, Node(a,b) iha ihb => simp [iha, ihb], } }",
        "by { induction t { Leaf => { rfl }, Node(a,b) iha ihb => { simp [iha, ihb] } } }",
    ] {
        let source = format!("{TREE}theorem twice(t : Tree) : mirror(mirror(t)) = t {proof}");
        let result = checked(&source);
        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }
    let source = format!(
        "{EXPR}theorem unchanged(e : Expr) : eval(fold(e)) = eval(e) by {{ induction e {{ Lit(n) => {{ rfl }}, Add(a,b) iha ihb => {{ simp [iha, ihb] }} }} }}"
    );
    let result = checked(&source);
    assert!(result.is_ok(), "{:?}", result.diagnostics);
}

#[test]
fn hypothesis_count_and_distinct_names_are_checked() {
    for names in ["", "iha", "iha ihb ihc", "ih ih"] {
        for proof in [
            format!("{{ induction t {{ Leaf => rfl, Node(a,b) {names} => rfl, }} }}"),
            format!("by {{ induction t {{ Leaf => {{ rfl }}, Node(a,b) {names} => {{ rfl }} }} }}"),
        ] {
            let result = checked(&format!("{TREE}theorem t(t : Tree) : t = t {proof}"));
            assert!(
                result.diagnostics.iter().any(|d| d.code == "E_PROOF_CASES"),
                "{:?}",
                result.diagnostics
            );
        }
    }
    let result = checked(&format!(
        "{TREE}theorem t(t : Tree) : t = t {{ induction t {{ Leaf ih => rfl, Node(a,b) iha ihb => rfl, }} }}"
    ));
    assert!(result.diagnostics.iter().any(|d| d.code == "E_PROOF_CASES"));
}

#[test]
fn goal_reports_and_quick_fixes_include_every_hypothesis() {
    let source = format!(
        "{TREE}theorem twice(t : Tree) : mirror(mirror(t)) = t by {{ induction t {{ Leaf => {{ rfl }}, Node(a,b) iha ihb => {{ simp [iha, ihb] }} }} }}"
    );
    let parsed = parse_file("multi.kes", &source);
    let result = check(&parsed);
    let goals = goals_at(&parsed, &result, source.find("simp [").unwrap());
    assert_eq!(
        goals.goals[0]
            .hypotheses
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>(),
        ["iha", "ihb"]
    );
    for proof in [
        "{ induction t { Leaf => rfl, } }",
        "by { induction t { Leaf => { rfl } } }",
    ] {
        let source = format!("{TREE}theorem twice(t : Tree) : mirror(mirror(t)) = t {proof}");
        let parsed = parse_file("multi.kes", &source);
        let result = check(&parsed);
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|d| d.code == "E_PROOF_CASES")
            .unwrap();
        // Public editor actions expose the same fixes attached by the kernel.
        let rendered = format!("{diagnostic:?}");
        assert!(rendered.contains("ihleft ihright"), "{rendered}");
    }
}

#[test]
fn recursive_fields_need_binders_and_the_same_instantiated_type() {
    for pattern in ["Node(_,b) ihb", "Node(Node(a,b),c) iha ihc"] {
        let result = checked(&format!(
            "{TREE}theorem same(t : Tree) : t = t {{ induction t {{ Leaf => rfl, {pattern} => rfl, }} }}"
        ));
        assert!(
            result.diagnostics.iter().any(|d| d.code == "E_PROOF_CASES"),
            "{:?}",
            result.diagnostics
        );
    }
    let source = "enum Changing[T] { End, Next(Changing[Int]) }\ntheorem same(t : Changing[Bool]) : t = t { induction t { End => rfl, Next(a) => rfl, } }";
    let result = checked(source);
    assert!(result.is_ok(), "{:?}", result.diagnostics);
    let result = checked(&source.replace("Next(a) =>", "Next(a) ih =>"));
    assert!(
        result.diagnostics.iter().any(|d| d.code == "E_PROOF_CASES"),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn the_multi_induction_example_checks() {
    let result = checked(include_str!("../testdata/language/multiinduction.kes"));
    assert!(result.is_ok(), "{:?}", result.diagnostics);
}

#[test]
fn structural_citations_remain_valid_in_tactic_case_splits() {
    for (split, names) in [("cases", ""), ("induction", "iha ihb")] {
        let source = format!(
            "{TREE}theorem twice(t : Tree) : mirror(mirror(t)) = t by {{ {split} t {{ Leaf => {{ rfl }}, Node(a,b) {names} => {{ have ha := twice(a); have hb := twice(b); simp [ha,hb] }} }} }}"
        );
        let result = checked(&source);
        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }
    let result = checked(&format!(
        "{TREE}theorem twice(t : Tree) : mirror(mirror(t)) = t by {{ induction t {{ Leaf => {{ rfl }}, Node(a,b) iha ihb => {{ exact twice(Node(a,b)) }} }} }}"
    ));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PROOF_RECURSION"),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn each_hypothesis_checks_its_own_smaller_assumptions() {
    let source = "enum Branch { Tip(Int), Fork(Branch,Branch) }\nfunction first(t : Branch) : Int = match t { Tip(n) => n, Fork(a,b) => first(a), }\nfunction rebuild(move t : Branch) : Branch = match t { Tip(n) => Tip(n), Fork(a,b) => Fork(rebuild(a),rebuild(b)), }\ntheorem same(t : Branch) : rebuild(t) = t requires h : first(t) = 0 { induction t { Tip(n) => rfl, Fork(a,b) iha ihb => simp [iha,ihb], } }";
    let result = checked(source);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PROOF_ASSUMPTION"),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn generic_cases_with_other_fields_use_only_the_recursive_hypotheses() {
    let source = "enum Branch[T] { Tip, Fork(T,Branch[T],Branch[T]) }\nfunction rebuild(move t : Branch[T]) : Branch[T] = match t { Tip => Tip, Fork(x,a,b) => Fork(x,rebuild(a),rebuild(b)), }\ntheorem same(t : Branch[U]) : rebuild(t) = t by { induction t { Tip => {rfl}, Fork(x,a,b) iha ihb => {simp [iha,ihb]} } }";
    let result = checked(source);
    assert!(result.is_ok(), "{:?}", result.diagnostics);
    let source = source.replace("Fork(x,a,b) iha ihb", "Fork(x,a,b) ihx iha ihb");
    let result = checked(&source);
    assert!(
        result.diagnostics.iter().any(|d| d.code == "E_PROOF_CASES"),
        "{:?}",
        result.diagnostics
    );
}
