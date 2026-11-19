//! Alternatives and ranges in patterns: one arm body for several shapes, and
//! integer intervals written with `..` and `..=`.
use kestrel::{
    checker::check,
    core,
    modules::{ProjectCheck, check_source},
    native, parse_file,
};
use std::process::{Command, Output};

fn good(source: &str) {
    let checked = check(&parse_file("patterns.kes", source));
    assert!(checked.is_ok(), "{source}\n{:#?}", checked.diagnostics);
}

fn refuses(source: &str, code: &str) {
    let checked = check(&parse_file("patterns.kes", source));
    assert!(
        checked.diagnostics.iter().any(|d| d.code == code),
        "Expected {code}: {source}\n{:#?}",
        checked.diagnostics
    );
}

fn execute(source: &str) -> Output {
    let project = ProjectCheck {
        modules: vec![check_source("patterns.kes", source)],
    };
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let program = core::lower(&project).unwrap_or_else(|errors| panic!("{errors:#?}"));
    let dir = native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let binary = dir.path().join("program");
    native::build(&program, &binary).unwrap();
    Command::new(&binary).output().unwrap()
}

#[test]
fn class_of_splits_bytes_with_ranges_and_alternatives() {
    let source = r#"
function classOf(b : Int) : String = match b {
    48..=57 => "digit",
    65..=90 | 97..=122 => "letter",
    32 | 9 | 10 | 13 => "space",
    _ => "other",
}
print(classOf(50))
print(classOf(65))
print(classOf(122))
print(classOf(9))
print(classOf(0))
"#;
    good(source);
    let output = execute(source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "digit\nletter\nletter\nspace\nother\n"
    );
}

#[test]
fn true_or_false_covers_bool_without_a_wildcard() {
    good(
        r#"
function both(b : Bool) : Int = match b {
    true | false => 1,
}
"#,
    );
    refuses(
        r#"
function one(b : Bool) : Int = match b {
    true => 1,
}
"#,
        "E_MATCH_EXHAUSTIVE",
    );
}

#[test]
fn ranges_do_not_make_an_int_match_exhaustive() {
    refuses(
        r#"
function digit(b : Int) : Bool = match b {
    48..=57 => true,
}
"#,
        "E_MATCH_EXHAUSTIVE",
    );
}

#[test]
fn only_int_and_nat_take_ranges() {
    refuses(
        r#"
function f(n : BigInt) : Bool = match n {
    0..10 => true,
    _ => false,
}
"#,
        "E_PATTERN_RANGE_TYPE",
    );
}

#[test]
fn enum_alternatives_without_bindings_share_a_body() {
    let output = execute(
        r#"
enum Direction { North, South, East }
function vertical(d : Direction) : Bool = match d {
    North | South => true,
    East => false,
}
print(vertical(Direction.North))
print(vertical(Direction.East))
"#,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "true\nfalse\n");
}

#[test]
fn every_alternative_binds_the_same_names_with_the_same_types() {
    refuses(
        r#"
enum Mixed {
    A(Int)
    B(String)
}
function f(m : Mixed) : Int = match m {
    A(x) | B(x) => 1,
}
"#,
        "E_PATTERN_ALT_BINDINGS",
    );
    refuses(
        r#"
enum Mixed {
    A(Int)
    B(Int)
}
function f(m : Mixed) : Int = match m {
    A(x) | B(y) => 1,
}
"#,
        "E_PATTERN_ALT_BINDINGS",
    );
}

#[test]
fn a_duplicate_binding_inside_one_alternative_is_refused() {
    refuses(
        r#"
enum Pair { Two(Int, Int), One(Int) }
function f(p : Pair) : Int = match p {
    Two(x, x) | One(x) => x,
}
"#,
        "E_SHADOW",
    );
}

#[test]
fn negative_ranges_and_alternative_guards_work() {
    let source = r#"
function pick(n : Int, flag : Bool) : String = match n {
    1 | 2 if flag => "small flag",
    -5..0 => "negative",
    3..=5 => "three to five",
    _ => "other",
}
print(pick(1, true))
print(pick(2, false))
print(pick(-4, false))
print(pick(5, false))
print(pick(9, false))
"#;
    good(source);
    let output = execute(source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "small flag\nother\nnegative\nthree to five\nother\n"
    );
}

#[test]
fn a_list_rest_still_means_the_tail_beside_a_scalar_range() {
    let source = r#"
function firstOrZero(xs : List[Int]) : Int = match xs {
    [] => 0,
    [x, ..rest] => x + rest.length(),
    _ => -1,
}
function bucket(n : Int) : Int = match n {
    0..10 => 0,
    _ => 1,
}
print(firstOrZero([7, 8, 9]))
print(bucket(4))
print(bucket(14))
"#;
    good(source);
    let output = execute(source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "9\n0\n1\n");
}

#[test]
fn proofs_unfold_functions_with_alternatives_and_ranges() {
    good(
        r#"
function isSmall(n : Int) : Bool = match n {
    0 | 1 | 2 => true,
    _ => false,
}
function isDigit(n : Int) : Bool = match n {
    48..=57 => true,
    _ => false,
}
enum Wrapped {
    A(Int)
    B(Int)
}
function unwrap(w : Wrapped) : Int = match w {
    A(x) | B(x) => x,
}
theorem oneIsSmall() : isSmall(1) = true { rfl }
theorem sevenIsNotSmall() : isSmall(7) = false { rfl }
theorem fiveIsDigit() : isDigit(53) = true { rfl }
theorem fiveIsNotDigit() : isDigit(5) = false { rfl }
theorem unwrapA() : unwrap(Wrapped.A(3)) = 3 { rfl }
theorem unwrapB() : unwrap(Wrapped.B(4)) = 4 { rfl }
"#,
    );
}

#[test]
fn the_example_checks_and_runs() {
    let source = include_str!("../testdata/language/patterns.kes");
    good(source);
    let output = execute(source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "digit\nletter\nspace\nother\n1\ntrue\nfalse\nnegative\nzero\nsmall\nlarge\nloud edge\nother\n"
    );
}
