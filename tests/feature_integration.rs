//! The two pattern features meet: named payloads inside alternatives, and
//! ranges sharing an arm with an alternative.
use kestrel::{checker::check, parse_file};
use std::process::{Command, Output};

fn run(source: &str) -> Output {
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let file = dir.path().join("main.kes");
    std::fs::write(&file, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .arg("run")
        .arg(&file)
        .current_dir(dir.path())
        .output()
        .unwrap()
}

fn stdout(source: &str) -> String {
    let output = run(source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn named_payloads_work_inside_alternatives() {
    let output = stdout(
        r#"
enum Move {
    Jump(height : Int, speed : Int)
    Wait
}
function score(m : Move) : Int = match m {
    Jump(speed = s, height = h) | Jump(height = h, speed = s) => h * s,
    Wait => 0,
}
print(score(Jump(speed = 3, height = 2)))
print(score(Wait))
"#,
    );
    assert_eq!(output, "6\n0\n");
}

#[test]
fn ranges_and_alternatives_share_an_arm() {
    let output = stdout(
        r#"
function bucket(n : Int) : String = match n {
    0..=9 | 100 => "low",
    10..=99 => "mid",
    _ => "high",
}
print(bucket(5))
print(bucket(100))
print(bucket(50))
print(bucket(-1))
"#,
    );
    assert_eq!(output, "low\nlow\nmid\nhigh\n");
}

#[test]
fn alternatives_that_disagree_on_names_are_refused() {
    let checked = check(&parse_file(
        "mix.kes",
        r#"
enum Move {
    Jump(height : Int, speed : Int)
    Wait
}
function score(m : Move) : Int = match m {
    Jump(speed = s, height = h) | Jump(height = h, speed = h) => h * s,
    Wait => 0,
}
"#,
    ));
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PATTERN_ALT_BINDINGS"),
        "{:#?}",
        checked.diagnostics
    );
}
