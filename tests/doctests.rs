use kestrel::{
    native::TemporaryDirectory,
    testing::{self, Options},
};
use std::{fs, path::Path, process::Command};

/// Writes `files` under a fresh folder; each entry is a relative path and its text.
fn workspace(files: &[(&str, &str)]) -> TemporaryDirectory {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    for (path, text) in files {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    dir
}

fn report(package: &Path, filter: Option<&str>) -> (bool, String) {
    let mut out = vec![];
    let passed = testing::run(
        package,
        &Options {
            timeout: None,
            filter: filter.map(str::to_owned),
            jobs: 4,
            libraries: vec![],
        },
        &mut out,
    )
    .unwrap();
    (passed, String::from_utf8(out).unwrap())
}

const PROJECT: &str = "package sample\n";

#[test]
fn a_doctest_runs_every_expected_value_in_order() {
    let dir = workspace(&[
        ("kestrel.project", PROJECT),
        (
            "math.kes",
            r#"/// Doubles a number.
///
/// ```kestrel
/// double(2)      // => 4
/// double(-3)     // => -6
/// ```
public function double(n : Int) : Int = n * 2
"#,
        ),
    ]);
    let (passed, text) = report(dir.path(), None);
    assert!(passed, "{text}");
    assert!(text.contains("running 1 test from 1 file"), "{text}");
    assert!(text.contains("ok    math.kes  double (doctest)"), "{text}");
    assert!(text.contains("1 passed, 0 failed"), "{text}");
}

#[test]
fn a_wrong_expected_value_names_the_declaration_and_line() {
    let dir = workspace(&[
        ("kestrel.project", PROJECT),
        (
            "math.kes",
            r#"/// Doubles a number.
///
/// ```kestrel
/// double(2)      // => 5
/// double(-3)     // => -6
/// ```
public function double(n : Int) : Int = n * 2
"#,
        ),
    ]);
    let (passed, text) = report(dir.path(), None);
    assert!(!passed, "{text}");
    assert!(text.contains("FAIL  math.kes  double (doctest)"), "{text}");
    assert!(
        text.contains("doctest on `double` in math.kes (fence at line 3)"),
        "{text}"
    );
    assert!(
        text.contains("math.kes:4: expected `5`, found `4`"),
        "{text}"
    );
    assert!(text.contains("0 passed, 1 failed"), "{text}");
}

#[test]
fn a_doctest_runs_let_bindings_statements_and_a_double() {
    let dir = workspace(&[
        ("kestrel.project", PROJECT),
        (
            "numbers.kes",
            r#"/// Halves a number.
///
/// ```kestrel
/// let n := 1
/// half(n)      // => 0.5
/// half(2)
/// ```
public function half(n : Int) : double = n.toDouble() / 2.0
"#,
        ),
    ]);
    let (passed, text) = report(dir.path(), None);
    assert!(passed, "{text}");
    assert!(text.contains("ok    numbers.kes  half (doctest)"), "{text}");
}

#[test]
fn an_ignored_fence_runs_nothing() {
    let dir = workspace(&[
        ("kestrel.project", PROJECT),
        (
            "math.kes",
            r#"/// Doubles a number.
///
/// ```kestrel ignore
/// double(2)      // => 5
/// ```
public function double(n : Int) : Int = n * 2
"#,
        ),
    ]);
    let (passed, text) = report(dir.path(), None);
    assert!(passed, "{text}");
    assert!(text.contains("running 0 tests from 0 files"), "{text}");
    assert!(text.contains("0 passed, 0 failed"), "{text}");
}

#[test]
fn a_fence_that_does_not_compile_fails_only_its_doctest() {
    let dir = workspace(&[
        ("kestrel.project", PROJECT),
        (
            "math.kes",
            r#"/// Doubles a number.
///
/// ```kestrel
/// double("nope") // => 4
/// ```
public function double(n : Int) : Int = n * 2

/// Adds one.
///
/// ```kestrel
/// addOne(2)      // => 3
/// ```
public function addOne(n : Int) : Int = n + 1
"#,
        ),
    ]);
    let (passed, text) = report(dir.path(), None);
    assert!(!passed, "{text}");
    assert!(text.contains("FAIL  math.kes  double (doctest)"), "{text}");
    assert!(text.contains("ok    math.kes  addOne (doctest)"), "{text}");
    assert!(
        text.contains("expects type `Int`") && text.contains("E_CALL_ARGUMENTS"),
        "{text}"
    );
    assert!(text.contains("1 passed, 1 failed"), "{text}");
}

#[test]
fn a_doctest_sees_the_modules_public_names_only() {
    let dir = workspace(&[
        ("kestrel.project", PROJECT),
        (
            "math.kes",
            r#"/// Uses the helper.
///
/// ```kestrel
/// helper(2)      // => 3
/// ```
public function usesHelper() : Int = helper(2)

/// A visible call.
///
/// ```kestrel
/// usesHelper()   // => 3
/// ```
public function visible() : Int = usesHelper()

// A private helper, out of reach for a snippet.
function helper(n : Int) : Int = n + 1
"#,
        ),
    ]);
    let (passed, text) = report(dir.path(), None);
    assert!(!passed, "{text}");
    assert!(
        text.contains("FAIL  math.kes  usesHelper (doctest)"),
        "{text}"
    );
    assert!(text.contains("ok    math.kes  visible (doctest)"), "{text}");
    assert!(text.contains("Name `helper` is not defined."), "{text}");
    assert!(text.contains("E_UNKNOWN_NAME"), "{text}");
    assert!(text.contains("1 passed, 1 failed"), "{text}");
}

#[test]
fn the_filter_selects_one_doctest() {
    let dir = workspace(&[
        ("kestrel.project", PROJECT),
        (
            "geometry/math.kes",
            r#"/// Doubles.
///
/// ```kestrel
/// double(2)      // => 4
/// ```
public function double(n : Int) : Int = n * 2

/// Triples.
///
/// ```kestrel
/// triple(2)      // => 6
/// ```
public function triple(n : Int) : Int = n * 3
"#,
        ),
    ]);
    let (passed, text) = report(dir.path(), Some("double"));
    assert!(passed, "{text}");
    assert!(text.contains("running 1 test from 1 file"), "{text}");
    assert!(
        text.contains("ok    geometry/math.kes  double (doctest)"),
        "{text}"
    );
    assert!(!text.contains("triple (doctest)"), "{text}");
    // A module name selects every doctest in the module.
    let (passed, text) = report(dir.path(), Some("geometry.math"));
    assert!(passed, "{text}");
    assert!(text.contains("2 passed, 0 failed"), "{text}");
}

#[test]
fn an_ordinary_documentation_block_runs_nothing() {
    let dir = workspace(&[
        ("kestrel.project", PROJECT),
        (
            "math.kes",
            r#"/// Doubles a number.
///
/// `double(2)` returns 4.
public function double(n : Int) : Int = n * 2

/// A fence detached by a blank line is not documentation.
///
/// ```kestrel
/// double(2)      // => 5
/// ```

public function detached() : Int = 1

/// ```python
/// double(2)      // => 5
/// ```
public function prose() : Int = 1

//// ```kestrel
//// double(2)      // => 5
//// ```
public function banner() : Int = 1
"#,
        ),
    ]);
    let (passed, text) = report(dir.path(), None);
    assert!(passed, "{text}");
    assert!(text.contains("running 0 tests from 0 files"), "{text}");
    assert!(text.contains("0 passed, 0 failed"), "{text}");
}

#[test]
fn doctests_run_after_the_test_files_in_the_report() {
    let dir = workspace(&[
        ("kestrel.project", PROJECT),
        (
            "math.kes",
            r#"/// Doubles.
///
/// ```kestrel
/// double(2)      // => 4
/// ```
public function double(n : Int) : Int = n * 2
"#,
        ),
        (
            "tests/math.kes",
            r#"import math.{double}
import std.test.{assertEq}

@test
function doubles() : void { assertEq(double(2), 4) }
"#,
        ),
    ]);
    let (passed, text) = report(dir.path(), None);
    assert!(passed, "{text}");
    let test = text.find("ok    tests/math.kes  doubles").unwrap();
    let doctest = text.find("ok    math.kes  double (doctest)").unwrap();
    assert!(test < doctest, "doctests belong after the tests:\n{text}");
}

#[test]
fn check_does_not_run_doctests() {
    let dir = workspace(&[
        ("kestrel.project", PROJECT),
        (
            "math.kes",
            r#"/// Prints when the fence runs.
///
/// ```kestrel
/// print("doctest ran")
/// 1      // => 2
/// ```
public function double(n : Int) : Int = n * 2
"#,
        ),
    ]);
    let output = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["check", "math.kes"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("doctest ran"));
}

#[test]
fn a_documentation_block_attaches_only_when_it_touches_the_declaration() {
    let parsed = kestrel::parse_file(
        "docs.kes",
        r#"/// Adds two numbers.
/// Second line.
function add(a : Int, b : Int) : Int = a + b

// An implementation note.
function plain() : Int = 1

/// Detached by a blank line.

function detached() : Int = 1

//// A banner.
function banner() : Int = 1
"#,
    );
    let docs = parsed
        .root
        .children
        .iter()
        .filter_map(|node| node.child(kestrel::syntax::SyntaxKind::Doc))
        .map(|doc| {
            doc.children
                .iter()
                .map(|line| parsed.source.slice(line.span))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>();
    assert_eq!(docs, vec!["Adds two numbers.\nSecond line."]);
}

#[test]
fn the_cli_exits_unsuccessfully_when_a_doctest_fails() {
    let dir = workspace(&[
        ("kestrel.project", PROJECT),
        (
            "math.kes",
            r#"/// Doubles a number.
///
/// ```kestrel
/// double(2)      // => 4
/// ```
public function double(n : Int) : Int = n * 2
"#,
        ),
    ]);
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_kestrel"))
            .arg("test")
            .current_dir(dir.path())
            .output()
            .unwrap()
    };
    let good = run();
    assert_eq!(good.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&good.stdout).contains("1 passed, 0 failed"));
    fs::write(
        dir.path().join("math.kes"),
        "/// Doubles a number.\n///\n/// ```kestrel\n/// double(2)      // => 5\n/// ```\npublic function double(n : Int) : Int = n * 2\n",
    )
    .unwrap();
    let bad = run();
    assert_eq!(bad.status.code(), Some(1));
    let text = String::from_utf8_lossy(&bad.stdout);
    assert!(text.contains("FAIL  math.kes  double (doctest)"), "{text}");
    assert!(text.contains("expected `5`, found `4`"), "{text}");
}
