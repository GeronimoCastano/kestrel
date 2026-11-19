use kestrel::{
    core::{self, Program},
    modules::{ProjectCheck, check_source},
    native::{self, TemporaryDirectory},
};
use std::process::{Command, Output};

fn lower(source: &str) -> Program {
    let project = ProjectCheck {
        modules: vec![check_source("invariants.kes", source)],
    };
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    core::lower(&project).unwrap_or_else(|errors| panic!("{errors:#?}\n{source}"))
}

fn execute(source: &str) -> Output {
    execute_program(&lower(source))
}

fn execute_program(program: &Program) -> Output {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let binary = dir.path().join("program");
    native::build(program, &binary).unwrap();
    Command::new(&binary).output().unwrap()
}

fn prints(source: &str, expected: &str) {
    let output = execute(source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

fn stops(source: &str, needles: &[&str]) {
    let output = execute(source);
    assert!(!output.status.success(), "the program should stop");
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(!errors.is_empty(), "a failed invariant prints a message");
    for needle in needles {
        assert!(errors.contains(needle), "missing `{needle}` in:\n{errors}");
    }
}

fn refuses(source: &str, code: &str) {
    let project = ProjectCheck {
        modules: vec![check_source("invariants.kes", source)],
    };
    assert!(
        project.modules[0]
            .checked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code),
        "Expected {code}: {source}\n{:#?}",
        project.modules[0].checked.diagnostics
    );
}

const ACCOUNT: &str = r#"
class Account {
    invariant this.balance >= 0.0

    public var balance : double

    new(start : double) { this.balance = start }

    public mut function withdraw(amount : double) : void or Error {
        if amount > this.balance { return Error("insufficient funds") }
        this.balance = this.balance - amount
    }

    public function current() : double = this.balance
}
"#;

#[test]
fn a_constructor_that_leaves_a_bad_state_stops() {
    let output = execute(&format!(
        "{ACCOUNT}\nlet a := Account(-5.0)\nprint(a.current())"
    ));
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(errors.contains("Invariant of Account"), "{errors}");
    assert!(errors.contains("Account.new"), "{errors}");
    assert!(errors.contains("this.balance >= 0.0"), "{errors}");
}

#[test]
fn a_public_method_that_leaves_a_bad_state_names_the_class_and_method() {
    stops(
        r#"
class Account {
    invariant this.balance >= 0.0

    public var balance : double

    new(start : double) { this.balance = start }

    public mut function withdraw(amount : double) : void {
        this.balance = this.balance - amount
    }
}

let a := Account(5.0)
a.withdraw(50.0)
print("unreachable")
"#,
        &[
            "Invariant of Account",
            "Account.withdraw",
            "this.balance >= 0.0",
        ],
    );
}

#[test]
fn a_private_helper_may_break_the_invariant_and_restore_it() {
    prints(
        r#"
class Account {
    invariant this.balance >= 0.0

    public var balance : double

    new(start : double) { this.balance = start }

    private mut function settle() : void {
        let saved := this.balance
        this.balance = -1.0
        this.balance = saved
    }

    public mut function deposit(amount : double) : void {
        this.settle()
        this.balance = this.balance + amount
    }
}
let a := Account(10.0)
a.deposit(2.5)
print(a.balance)
"#,
        "12.5\n",
    );
}

#[test]
fn an_error_return_is_exempt_from_the_invariant() {
    prints(
        &format!(
            "{ACCOUNT}\nlet a := Account(5.0)\nmatch a.withdraw(50.0) {{\nRuntimeError.Message(message) => print(message),\n_ => print(\"unexpected\"),\n}}\nprint(\"exempt\")"
        ),
        "insufficient funds\nexempt\n",
    );
}

#[test]
fn a_private_helper_that_leaves_a_bad_state_is_caught_by_its_public_caller() {
    stops(
        r#"
class Account {
    invariant this.balance >= 0.0

    public var balance : double

    new(start : double) { this.balance = start }

    private mut function lose() : void { this.balance = this.balance - 100.0 }

    public mut function drain() : void { this.lose() }
}
let a := Account(10.0)
a.drain()
print("unreachable")
"#,
        &["Invariant of Account", "Account.drain"],
    );
}

#[test]
fn a_package_method_is_checked_like_a_public_one() {
    stops(
        r#"
class Counter {
    invariant this.n >= 0

    public var n : Int

    new() { this.n = 0 }

    package mut function breakIt() : void { this.n = -1 }
}
let c := Counter()
c.breakIt()
print("unreachable")
"#,
        &["Invariant of Counter", "Counter.breakIt"],
    );
}

#[test]
fn a_cleanup_error_is_exempt_like_any_other_error_return() {
    // The check runs after the body finishes its cleanup, so a failed task join
    // still returns Error instead of stopping on the state it left behind.
    prints(
        r#"
function fail() : void or Error = RuntimeError.Parse("joined")
class Runner {
    invariant this.n >= 0

    public var n : Int

    new() { this.n = 0 }

    public mut function run() : Int or Error with Effects {
        this.n = -1
        let _ := scope {
            spawn fail()
            return 1
        }
        2
    }
}
let r := Runner()
print(match r.run() {
    RuntimeError.Parse(message) => message,
    n => n.text(),
    _ => "other",
})
print("exempt")
"#,
        "joined\nexempt\n",
    );
}

#[test]
fn a_method_that_traps_is_exempt_because_the_program_ends() {
    let output = execute(
        r#"
class Counter {
    invariant this.n >= 0

    public var n : Int

    new() { this.n = 0 }

    public mut function boom() : void {
        this.n = -1
        let zero := 0
        this.n = 10 / zero
    }
}
let c := Counter()
c.boom()
print("unreachable")
"#,
    );
    assert!(!output.status.success());
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(errors.contains("division by zero"), "{errors}");
    assert!(!errors.contains("Invariant of Counter"), "{errors}");
}

#[test]
fn a_subclass_inherits_the_parent_invariant_and_checks_its_own() {
    stops(
        r#"
class Account {
    invariant this.balance >= 0.0

    public var balance : double

    new(start : double) { this.balance = start }
}

class Savings extends Account {
    invariant this.rate >= 0.0

    public var rate : double

    new(start : double, rate : double) {
        this.balance = start
        this.rate = rate
    }

    public mut function lose() : void {
        this.balance = this.balance - 100.0
    }

    public mut function dropRate() : void {
        this.rate = -1.0
    }
}

let s := Savings(10.0, 0.5)
s.lose()
"#,
        &[
            "Invariant of Account",
            "Savings.lose",
            "this.balance >= 0.0",
        ],
    );
    stops(
        r#"
class Account {
    invariant this.balance >= 0.0

    public var balance : double

    new(start : double) { this.balance = start }
}

class Savings extends Account {
    invariant this.rate >= 0.0

    public var rate : double

    new(start : double, rate : double) {
        this.balance = start
        this.rate = rate
    }

    public mut function dropRate() : void {
        this.rate = -1.0
    }
}

let s := Savings(10.0, 0.5)
s.dropRate()
"#,
        &[
            "Invariant of Savings",
            "Savings.dropRate",
            "this.rate >= 0.0",
        ],
    );
}

#[test]
fn two_invariants_on_one_class_must_all_hold() {
    let source = r#"
class Cell {
    invariant this.n >= 0
    invariant this.n < 10

    public var n : Int

    new(n : Int) { this.n = n }

    public mut function grow() : void { this.n = this.n + 1 }
}
"#;
    prints(
        &format!("{source}\nlet c := Cell(3)\nc.grow()\nprint(c.n)"),
        "4\n",
    );
    stops(
        &format!("{source}\nlet c := Cell(9)\nc.grow()\nprint(\"unreachable\")"),
        &["Invariant of Cell", "Cell.grow", "this.n < 10"],
    );
}

#[test]
fn an_external_field_write_is_not_checked() {
    // Invariants are checked at method and constructor boundaries, not after a
    // public field write from outside the class.
    prints(
        r#"
class Cell {
    invariant this.n >= 0

    public var n : Int

    new(n : Int) { this.n = n }
}
let c := Cell(1)
c.n = -5
print(c.n)
"#,
        "-5\n",
    );
}

#[test]
fn a_read_only_method_may_be_called_from_an_invariant() {
    prints(
        r#"
class Counter {
    invariant this.ping() >= 0

    public var n : Int

    new(n : Int) { this.n = n }

    public function ping() : Int = this.n

    public mut function set(n : Int) : void { this.n = n }
}
let c := Counter(5)
print(c.ping())
c.set(7)
print(c.ping())
"#,
        "5\n7\n",
    );
    stops(
        r#"
class Counter {
    invariant this.ping() >= 0

    public var n : Int

    new(n : Int) { this.n = n }

    public function ping() : Int = this.n

    public mut function set(n : Int) : void { this.n = n }
}
let c := Counter(5)
c.set(-1)
"#,
        &["Invariant of Counter", "Counter.set", "this.ping() >= 0"],
    );
}

#[test]
fn a_parent_constructor_called_with_super_leaves_the_whole_chain_to_the_subclass() {
    prints(
        r#"
class Base {
    invariant this.n >= 0

    public var n : Int

    new(n : Int) { this.n = n }
}

class Child extends Base {
    new() {
        super(-5)
        this.n = 3
    }
}
let c := Child()
print(c.n)
"#,
        "3\n",
    );
    stops(
        r#"
class Base {
    invariant this.n >= 0

    public var n : Int

    new(n : Int) { this.n = n }
}

class Child extends Base {
    new() {
        super(1)
        this.n = -2
    }
}
let c := Child()
print("unreachable")
"#,
        &["Invariant of Base", "Child.new", "this.n >= 0"],
    );
}

#[test]
fn invariants_are_rejected_outside_a_class_body_and_must_be_pure_bools() {
    refuses(
        "trait T {\n    invariant true\n    function value() : Int\n}",
        "E_INVARIANT_CONTEXT",
    );
    refuses(
        "record P(x : Int) {\n    invariant x > 0\n}",
        "E_INVARIANT_RECORD",
    );
    refuses(
        "class C {\n    invariant this.n\n    public var n : Int\n    new() { this.n = 1 }\n}",
        "E_TYPE",
    );
    refuses(
        "class C {\n    invariant print(\"no\") == void\n    public var n : Int\n    new() { this.n = 1 }\n}",
        "E_EFFECT_REQUIRED",
    );
    refuses(
        "class C {\n    invariant this.touch()\n    public var n : Int\n    new() { this.n = 1 }\n    private mut function touch() : Bool { this.n = 2\n        true }\n}",
        "E_INVARIANT_MUT",
    );
}

#[test]
fn the_example_runs() {
    prints(
        include_str!("../testdata/language/invariants.kes"),
        "42.0\ntrue\ninsufficient funds\n110.0\n",
    );
}
