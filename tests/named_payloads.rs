//! Named payloads for enum cases.
use kestrel::{
    checker::{Checked, check},
    parse_file,
};
use std::process::{Command, Output};

fn checked(source: &str) -> Checked {
    check(&parse_file("named.kes", source))
}
fn refuses(source: &str, code: &str) -> String {
    let result = checked(source);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == code)
        .unwrap_or_else(|| panic!("Expected {code}: {source}\n{:#?}", result.diagnostics));
    diagnostic.message.clone()
}
/// Runs a program through `kestrel run`, which loads the prelude and standard modules.
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

const MOVE: &str = r####"enum Move {
    Jump(height : Int, speed : Int)
    Wait
}
"####;

#[test]
fn named_and_positional_construction_agree() {
    let output = stdout(
        r####"enum Move {
    Jump(height : Int, speed : Int)
    Wait
}
print(Jump(height = 2, speed = 3))
print(Jump(2, 3))
print(Jump(speed = 3, height = 2))
print(Jump(height = 2, speed = 3) == Jump(2, 3))
"####,
    );
    assert_eq!(
        output,
        "Jump(height = 2, speed = 3)\nJump(height = 2, speed = 3)\nJump(height = 2, speed = 3)\ntrue\n"
    );
}

#[test]
fn named_and_positional_patterns_bind_the_declared_fields() {
    let output = stdout(&format!(
        r####"{MOVE}
function named(m : Move) : Int = match m {{
    Jump(speed = s, height = h) => h * 10 + s,
    Wait => -1,
}}
function positional(m : Move) : Int = match m {{
    Jump(h, s) => h * 10 + s,
    Wait => -2,
}}
function destructured() : Int {{
    let Jump(speed = s, height = h) := Jump(2, 3) otherwise {{ return -3 }}
    h * 10 + s
}}
print(named(Jump(2, 3)))
print(positional(Jump(height = 2, speed = 3)))
print(destructured())
"####
    ));
    assert_eq!(output, "23\n23\n23\n");
}

#[test]
fn named_payloads_nest_inside_other_patterns() {
    let output = stdout(&format!(
        r####"{MOVE}
function height(m : Option[Move]) : Int = match m {{
    Some(Jump(height = h, speed = s)) => h + s,
    Some(Wait) => 0,
    None => -1,
}}
print(height(Some(Jump(height = 2, speed = 3))))
print(height(Some(Wait)))
print(height(None))
"####
    ));
    assert_eq!(output, "5\n0\n-1\n");
}

#[test]
fn named_payload_construction_errors_are_distinct() {
    let missing = refuses(
        &format!("{MOVE}let m := Jump(height = 2)"),
        "E_CALL_ARGUMENTS",
    );
    assert!(
        missing.contains("expects 2 arguments (height : Int, speed : Int), but 1 was supplied"),
        "{missing}"
    );
    let unknown = refuses(
        &format!("{MOVE}let m := Jump(height = 2, speed = 3, altitude = 4)"),
        "E_CALL_ARGUMENTS",
    );
    assert!(
        unknown.contains("has no parameter named `altitude`"),
        "{unknown}"
    );
    let duplicate = refuses(
        &format!("{MOVE}let m := Jump(height = 2, height = 3)"),
        "E_CALL_ARGUMENTS",
    );
    assert!(
        duplicate.contains("gets parameter `height` more than once"),
        "{duplicate}"
    );
    let order = refuses(
        &format!("{MOVE}let m := Jump(height = 2, 3)"),
        "E_ARGUMENT_ORDER",
    );
    assert!(
        order.contains("positional argument follows a named argument"),
        "{order}"
    );
}

#[test]
fn named_payload_pattern_errors_are_distinct() {
    let pattern = |arm: &str| {
        format!(
            "{MOVE}function f(m : Move) : Int = match m {{
    {arm} => 1,
    Wait => 0,
}}"
        )
    };
    let missing = refuses(&pattern("Jump(height = h)"), "E_PATTERN_ARITY");
    assert!(missing.contains("is missing payload `speed`"), "{missing}");
    let unknown = refuses(&pattern("Jump(height = h, altitude = s)"), "E_PATTERN_CASE");
    assert!(
        unknown.contains("has no payload named `altitude`"),
        "{unknown}"
    );
    let duplicate = refuses(&pattern("Jump(height = h, height = s)"), "E_PATTERN_CASE");
    assert!(
        duplicate.contains("names payload `height` twice"),
        "{duplicate}"
    );
    let order = refuses(&pattern("Jump(height = h, s)"), "E_ARGUMENT_ORDER");
    assert!(
        order.contains("positional payload follows a named one"),
        "{order}"
    );
}

#[test]
fn a_case_cannot_mix_named_and_positional_payloads() {
    let mixed = refuses(
        "enum Move {\n    Jump(height : Int, Int)\n    Wait\n}",
        "E_PAYLOAD_MIXED",
    );
    assert!(
        mixed.contains("names some payloads and not others"),
        "{mixed}"
    );
    refuses(
        "enum Move {\n    Jump(height : Int, height : Int)\n    Wait\n}",
        "E_DUPLICATE_NAME",
    );
}

#[test]
fn a_named_case_prints_its_labels_and_a_positional_case_does_not() {
    let output = stdout(&format!(
        r####"{MOVE}
enum Tree {{
    Leaf
    Node(Tree, Int, Tree)
}}
print(Jump(height = 2, speed = 3))
print(Node(Leaf, 1, Leaf))
print(Some(2))
let absent : Option[Int] = None
print(absent)
print(Error("boom"))
"####
    ));
    assert_eq!(
        output,
        "Jump(height = 2, speed = 3)\nNode(Leaf, 1, Leaf)\nSome(2)\nNone\nRuntimeError.Message(\"boom\")\n"
    );
}

#[test]
fn named_payload_enums_compare_and_hash_by_position() {
    let output = stdout(&format!(
        r####"{MOVE}
let seen := Set(Jump(2, 3), Jump(height = 2, speed = 3), Jump(speed = 3, height = 2), Wait)
print(seen.length())
let moves : Map[Move, String] = Map(Jump(height = 1, speed = 2) => "jump", Wait => "wait")
print(moves.get(Jump(1, 2)))
print(moves.get(Jump(speed = 2, height = 1)))
print(Jump(2, 3) < Jump(3, 1))
"####
    ));
    assert_eq!(output, "2\nSome(jump)\nSome(jump)\ntrue\n");
}

#[test]
fn a_theorem_over_a_named_payload_enum_still_unfolds() {
    let output = stdout(
        r####"enum Tree {
    Leaf
    Node(left : Tree, right : Tree)
}
function mirror(t : Tree) : Tree = match t {
    Leaf => Leaf,
    Node(a, b) => Node(mirror(b), mirror(a)),
}
theorem mirrorTwice(t : Tree) : mirror(mirror(t)) = t by {
    induction t {
        Leaf => { rfl }
        Node(a, b) iha ihb => { simp [iha, ihb] }
    }
}
print(mirror(Node(Leaf, Node(Leaf, Leaf))))
"####,
    );
    assert_eq!(
        output,
        "Node(left = Node(left = Leaf, right = Leaf), right = Leaf)\n"
    );
}
