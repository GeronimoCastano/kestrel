//! Built-in sequence proofs use the existing array-backed List unchanged.
use kestrel::{checker::check, parse_file};

fn good(source: &str) {
    let checked = check(&parse_file("lists.kes", source));
    assert!(checked.is_ok(), "{source}\n{:#?}", checked.diagnostics);
}

fn refuses(source: &str, code: &str) {
    let checked = check(&parse_file("lists.kes", source));
    assert!(
        checked.diagnostics.iter().any(|d| d.code == code),
        "Expected {code}: {source}\n{:#?}",
        checked.diagnostics
    );
}

#[test]
fn list_induction_uses_brackets_and_checks_recursive_functions() {
    good(include_str!("../testdata/language/listproofs.kes"));
    good(
        r#"
function copy(xs : List[Int]) : List[Int] = match xs {
    [] => [],
    [x, ..rest] => [x] + copy(rest),
}
theorem copyIdentity(xs : List[Int]) : copy(xs) = xs {
    induction xs {
        [] => rfl,
        [x, ..rest] ih => congrArg(r => [x] + r, ih),
    }
}
theorem appendEmpty(xs : List[T]) : xs + [] = xs {
    match xs {
        [] => rfl,
        [x, ..rest] => congrArg(r => [x] + r, appendEmpty(rest)),
    }
}
theorem appendAssoc(xs ys zs : List[T]) : (xs + ys) + zs = xs + (ys + zs) by {
    induction xs {
        [] => { rfl }
        [x, ..rest] ih => { simp [ih] }
    }
}
theorem listQuantifier() : forall xs : List[Int], xs = xs by { intro xs; rfl }
theorem concrete() : copy([1, 2, 3]) = [1, 2, 3] { rfl }
function keep(xs : List[Int]) : List[Int] requires appendEmpty(xs) = xs
let items := [1, 2, 3]
let kept := keep(items)
"#,
    );
}

#[test]
fn list_operations_have_computational_equations() {
    good(
        r#"
theorem singletonLength(x : T) : [x].length() = 1 { rfl }
theorem getHead(x : Int, xs : List[Int]) : ([x] + xs).first() = Some(x) { rfl }
theorem last() : [1, 2, 3].last() = Some(3) { rfl }
theorem reversed() : [1, 2, 3].reversed() = [3, 2, 1] { rfl }
theorem empty() : [].isEmpty() { decide }
theorem nested(xs : List[Option[List[Int]]]) : xs = xs { rfl }
"#,
    );
}

#[test]
fn bool_and_option_support_both_proof_interfaces() {
    good(
        r#"
function negate(b : Bool) : Bool = match b { true => false, false => true, }
theorem doubleNegation(b : Bool) : negate(negate(b)) = b {
    match b { true => rfl, false => rfl, }
}
theorem doubleNegationTactic(b : Bool) : !(!b) = b by {
    cases b { true => { rfl } false => { rfl } }
}
function optionIdentity(move o : Option[T]) : Option[T] = match o {
    Some(x) => Some(x), None => None,
}
theorem optionIdentityProof(o : Option[T]) : optionIdentity(o) = o {
    match o { Some(x) => rfl, None => rfl, }
}
theorem optionCases(o : Option[Int]) : o.isSome() or o.isNone() by {
    cases o { Some(x) => { left; decide } None => { right; decide } }
}
theorem optionQuantifier() : forall o : Option[Bool], o = o by { intro o; rfl }
"#,
    );
}

#[test]
fn false_list_facts_and_fake_descent_are_rejected() {
    refuses(
        "theorem bad(xs : List[Int]) : xs = [] { rfl }",
        "E_PROOF_MISMATCH",
    );
    refuses(
        "theorem bad(x y : Int) : [x] = [y] { rfl }",
        "E_PROOF_MISMATCH",
    );
    refuses(
        "theorem bad(xs : List[Int]) : 0 = 1 { match xs { [..rest] => bad(rest), } }",
        "E_PROOF_RECURSION",
    );
    refuses(
        "theorem bad(xs : List[Int]) : xs = xs { induction xs { [] => rfl, [x, ..rest] => rfl, } }",
        "E_PROOF_CASES",
    );
    refuses(
        "theorem bad(xs : List[Int]) : xs = xs { match xs { [] => rfl, [x] => rfl, } }",
        "E_PROOF_CASES",
    );
    refuses(
        "theorem bad(xs : List[Int]) : xs = xs by { cases xs { [] => { rfl } [x] => { rfl } } }",
        "E_PROOF_CASES",
    );
    refuses(
        "function loop(xs : List[Int]) : Int = match xs { [..all] => loop(all), }\ntheorem bad(xs : List[Int]) : loop(xs) = 0 { rfl }",
        "E_PROOF_OPAQUE",
    );
}

#[test]
fn sequence_contents_cannot_hide_undefined_work() {
    refuses(
        "function bad(xs : List[Nat]) : List[Nat] { let zero : Nat = 0; [zero - 1] + xs }\ntheorem t(xs : List[Nat]) : bad(xs) = bad(xs) { rfl }",
        "E_PROOF_UNDEFINED",
    );
    refuses(
        "theorem bad(xs : List[Int]) : xs == xs { rfl }",
        "E_PROOF_UNSUPPORTED",
    );
    refuses(
        "theorem bad(xs : Array[Int]) : xs = xs { rfl }",
        "E_PROOF_UNSUPPORTED",
    );
    refuses(
        "theorem bad(xs : List[Int]) : xs = xs { match xs { Some(x) => rfl, } }",
        "E_PROOF_TYPE",
    );
    refuses(
        "class Item { public final n : Int; new(n : Int) { this.n = n } }\ntheorem bad(xs : List[Item]) : xs.first() = xs.first() { rfl }",
        "E_PROOF_UNSUPPORTED",
    );
    refuses(
        "theorem bad(xs : List[Int]) : xs.get(0) = None { rfl }",
        "E_PROOF_UNSUPPORTED",
    );
}

#[test]
fn imported_list_theorems_keep_their_types_and_simp_rules() {
    use kestrel::{modules::check_path, native::TemporaryDirectory};
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    std::fs::write(
        dir.path().join("facts.kes"),
        include_str!("../testdata/language/listproofs.kes"),
    )
    .unwrap();
    let main = dir.path().join("main.kes");
    std::fs::write(&main, "import facts.{appendEmpty, reverseTwice}\ntheorem imported(xs : List[Option[Int]]) : xs + [] = xs { simp }\ntheorem reverse(xs : List[Bool]) : xs.reversed().reversed() = xs { reverseTwice(xs) }").unwrap();
    let checked = check_path(&main, dir.path()).unwrap();
    assert!(checked.is_ok(), "{}", checked.render_diagnostics());
}

#[test]
fn list_goal_reports_and_missing_case_fixes_use_square_brackets() {
    use kestrel::proof::goals_at;
    let source = "theorem same(xs : List[Int]) : xs + [] = xs by { induction xs { [] => { rfl } [x, ..rest] ih => { simp [ih] } } }";
    let parsed = parse_file("lists.kes", source);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{:?}", checked.diagnostics);
    let goals = goals_at(&parsed, &checked, source.find("simp [").unwrap());
    assert_eq!(
        goals
            .cases
            .iter()
            .map(|case| case.label.as_str())
            .collect::<Vec<_>>(),
        ["[]", "[x, ..rest]"]
    );
    assert!(
        goals.goals[0]
            .hypotheses
            .iter()
            .any(|(name, _)| name == "ih")
    );
    for body in [
        "{ induction xs { [] => rfl, } }",
        "by { induction xs { [] => { rfl } } }",
    ] {
        let source = format!("theorem same(xs : List[Int]) : xs + [] = xs {body}");
        let checked = check(&parse_file("lists.kes", source));
        let missing = checked
            .diagnostics
            .iter()
            .find(|d| d.code == "E_PROOF_CASES")
            .unwrap();
        assert!(
            format!("{missing:?}").contains("[value, ..rest] ihrest"),
            "{missing:?}"
        );
    }
}

#[test]
fn logical_list_equations_match_native_execution() {
    use kestrel::{
        core,
        modules::{ProjectCheck, check_source},
        native::{self, TemporaryDirectory},
    };
    let cases: &[&[i64]] = &[&[], &[0], &[1, 1], &[-2, 0, 3], &[3, 2, 1, 0, -1]];
    let mut source = String::new();
    let mut proofs = String::new();
    let mut expected = String::new();
    let literal = |items: &[i64]| {
        format!(
            "[{}]",
            items
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    for (i, items) in cases.iter().enumerate() {
        let xs = literal(items);
        let reverse = literal(&items.iter().rev().copied().collect::<Vec<_>>());
        source.push_str(&format!("let xs{i} : List[Int] = {xs}\nprint(xs{i}.length())\nprint(xs{i}.reversed())\nprint(xs{i}.first())\nprint(xs{i}.last())\n"));
        let option = |item: Option<&i64>| item.map_or("None".into(), |n| format!("Some({n})"));
        let first = option(items.first());
        let last = option(items.last());
        expected.push_str(&format!("{}\n{reverse}\n{first}\n{last}\n", items.len()));
        proofs.push_str(&format!("function literal{i}() : List[Int] = {xs}\ntheorem sampleLength{i}() : literal{i}().length() = {} {{ rfl }}\ntheorem sampleReverse{i}() : literal{i}().reversed() = {reverse} {{ rfl }}\ntheorem sampleFirst{i}() : literal{i}().first() = {first} {{ rfl }}\ntheorem sampleLast{i}() : literal{i}().last() = {last} {{ rfl }}\n", items.len()));
    }
    source.push_str(&proofs);
    let project = ProjectCheck {
        modules: vec![check_source("lists.kes", source)],
    };
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let binary = dir.path().join("lists");
    native::build(&core::lower(&project).unwrap(), &binary).unwrap();
    let output = std::process::Command::new(binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn list_proofs_are_erased_without_changing_generated_code() {
    use kestrel::{
        core,
        modules::{ProjectCheck, check_source},
        native,
    };
    let source = "function copy(xs : List[Int]) : List[Int] = match xs { [] => [], [x, ..rest] => [x] + copy(rest), }\nprint(copy([3, 1, 2]))\n";
    let proof = "theorem copyIdentity(xs : List[Int]) : copy(xs) = xs { induction xs { [] => rfl, [x, ..rest] ih => congrArg(r => [x] + r, ih), } }";
    let emit = |source: String| {
        let project = ProjectCheck {
            modules: vec![check_source("lists.kes", source)],
        };
        assert!(project.is_ok(), "{}", project.render_diagnostics());
        native::emit_c(&core::lower(&project).unwrap()).unwrap()
    };
    assert_eq!(emit(source.into()), emit(format!("{source}{proof}")));
}
