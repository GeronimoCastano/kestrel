use kestrel::{
    checker::{Checked, check},
    contracts::Status,
    parse_file,
};

const HALF: &str = "function half(n : Int) : Int or Error requires n > 0 = n / 2\n";
const ACCOUNT: &str = "class Account { var balance : double\nnew() { this.balance = 0.0 }\npublic mut function deposit(amount : double) : void or Error requires amount > 0.0 { this.balance = this.balance + amount } }\n";

fn checked(source: &str) -> Checked {
    check(&parse_file("contracts.kes", source))
}
/// The status of each checked call, in source order.
fn statuses(source: &str) -> Vec<Status> {
    let result = checked(source);
    assert!(result.is_ok(), "{source}\n{:#?}", result.diagnostics);
    result.contract_checks.values().copied().collect()
}
fn refuses(source: &str, detail: &str) {
    let result = checked(source);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "E_CONTRACT_VIOLATED" && d.message.contains(detail)),
        "Expected E_CONTRACT_VIOLATED with {detail:?}: {source}\n{:#?}",
        result.diagnostics
    );
}
use Status::{Proved, Runtime};

#[test]
fn constant_arguments_are_decided_at_compile_time() {
    assert_eq!(
        statuses(&format!("{HALF}let a := half(4)\nlet b := half(n = 6)")),
        [Proved, Proved]
    );
    refuses(&format!("{HALF}let a := half(0)"), "n is 0");
    refuses(&format!("{HALF}let a := half(-3)"), "n is -3");
    assert_eq!(
        statuses(&format!("{ACCOUNT}let a := Account()\na.deposit(5.0)?")),
        [Proved]
    );
    refuses(
        &format!("{ACCOUNT}let a := Account()\na.deposit(-5.0)?"),
        "amount is -5.0",
    );
}

#[test]
fn unknown_arguments_keep_the_run_time_check() {
    assert_eq!(
        statuses(&format!(
            "{HALF}function f(x : Int) : Int or Error = half(x)"
        )),
        [Runtime]
    );
    assert_eq!(
        statuses(&format!(
            "{ACCOUNT}function f(mut a : Account, x : double) : void or Error = a.deposit(x)"
        )),
        [Runtime]
    );
}

#[test]
fn floating_point_reflexivity_keeps_a_guard_for_nan() {
    assert_eq!(
        statuses(
            "function checked(x : double) : double or Error requires x == x {x}\nfunction f(x : double) : double or Error = checked(x)"
        ),
        [Runtime]
    );
}

#[test]
fn mutation_invalidates_facts_about_pure_calls_that_read_fields() {
    assert_eq!(
        statuses(
            "class Counter {public var x : Int\nnew() {this.x=0}\npublic mut function increment() : void {this.x=this.x+1}}\nfunction read(c : Counter) : Int = c.x\nfunction checked(c : Counter) : Int or Error requires read(c) == 0 {c.x}\nfunction f(mut c : Counter) : Int or Error {if read(c) == 0 {c.increment()\nreturn checked(c)}\n0}"
        ),
        [Runtime]
    );
}

#[test]
fn conditions_on_the_path_prove_the_requirement() {
    assert_eq!(
        statuses(&format!(
            "{HALF}function f(x : Int) : Int or Error = if x > 3 then half(x) else half(1)"
        )),
        [Proved, Proved]
    );
    assert_eq!(
        statuses(&format!(
            "{HALF}function f(x : Int) : Int or Error {{ if x < 1 {{ return 0 }}\nhalf(x) }}"
        )),
        [Proved]
    );
    assert_eq!(
        statuses(&format!(
            "{HALF}function f(x : Int) : Int or Error requires x >= 5 = half(x)"
        )),
        [Proved]
    );
    assert_eq!(
        statuses(&format!(
            "{HALF}function f(x : Int) : Int or Error = match x {{ n if n > 0 => half(n), _ => 0, }}"
        )),
        [Proved]
    );
    assert_eq!(
        statuses(&format!(
            "{HALF}function f(x : Int) : Int or Error = if 0 < x then half(x) else 0"
        )),
        [Proved]
    );
    refuses(
        &format!("{HALF}function f(x : Int) : Int or Error = if x < 0 then half(x) else 0"),
        "always false",
    );
}

#[test]
fn changes_after_the_condition_undo_the_proof() {
    assert_eq!(
        statuses(&format!(
            "{HALF}function f(x : Int) : Int or Error {{ let mut m := x\nif m > 0 {{ m = m - 5\nreturn half(m) }}\n0 }}"
        )),
        [Runtime]
    );
    assert_eq!(
        statuses(&format!(
            "{HALF}function f(x : Int) : Int or Error {{ let mut m := x\nif m > 0 {{ while m < 100 {{ half(m)?\nm = m - 10 }} }}\n0 }}"
        )),
        [Runtime]
    );
    assert_eq!(
        statuses(&format!(
            "{HALF}function f(x : Int) : Int or Error {{ let mut m := x\nwhile m > 0 {{ half(m)?\nm = m - 1 }}\n0 }}"
        )),
        [Proved]
    );
    assert_eq!(
        statuses(&format!(
            "{HALF}function f(n : Int) : Int or Error {{ let mut total := 0\nfor (let i : Int = n; i > 0; i = i - 1) {{ total = total + half(i)? }}\ntotal }}"
        )),
        [Proved]
    );
}

#[test]
fn combined_conditions_constants_and_pipes() {
    let between = "const LIMIT : Int = 10\nfunction between(n : Int) : Int or Error requires (n > 0) && (n < LIMIT) = n\n";
    assert_eq!(statuses(&format!("{between}let a := between(5)")), [Proved]);
    refuses(&format!("{between}let a := between(12)"), "n is 12");
    assert_eq!(
        statuses(&format!(
            "{between}function f(x : Int) : Int or Error = if (x > 2) && (x < 8) then between(x) else 0"
        )),
        [Proved]
    );
    assert_eq!(statuses(&format!("{HALF}let a := 4 |> half")), [Proved]);
    refuses(&format!("{HALF}let a := 0 |> half"), "n is 0");
    assert_eq!(statuses(&format!("{HALF}let later := half(_)")), []);
}

#[test]
fn postconditions_bind_the_success_result_and_keep_copyable_parameters() {
    for source in [
        "function half(n : Int) : Int or Error requires n >= 0 ensures result <= n = n / 2",
        "function identity(n : Int) : Int ensures result == n {if n > 0 {return n}\nn}",
        "function text(s : String) : String ensures result == s = s",
        "function quiet() : void ensures true {}",
        "function quiet() : void or Error ensures true {}",
        "class Point {public final n : Int\nnew(n : Int) {this.n=n}}\nfunction make(n : Int) : Point ensures result.n == n = Point(n)\nfunction view(p : Point) : Point ensures result.n >= 0 = p",
        "class Counter {public var n : Int\nnew() {this.n=0}\npublic mut function increment() : void ensures this.n > 0 {this.n=this.n+1}}",
    ] {
        let result = checked(source);
        assert!(result.is_ok(), "{source}\n{:?}", result.diagnostics);
    }
}

#[test]
fn postconditions_reject_effects_moves_and_escaping_control_flow() {
    for (source, code) in [
        (
            "function f(result : Int) : Int ensures result > 0 = result",
            "E_ENSURES_RESULT_NAME",
        ),
        (
            "class P {public final n : Int\nnew(n : Int) {this.n=n}}\nfunction f(p : P) : Int ensures result == p.n = p.n",
            "E_ENSURES_PARAMETER",
        ),
        (
            "function noisy() : Bool with Effects {print(1)\ntrue}\nfunction f() : Int ensures noisy() = 1",
            "E_EFFECT_REQUIRED",
        ),
        (
            "function f() : Int ensures {return 1} = 1",
            "E_ENSURES_CONTROL",
        ),
        (
            "function test() : Bool or Error = true\nfunction f() : Int or Error ensures test()? = 1",
            "E_ENSURES_CONTROL",
        ),
        ("class P {new() ensures true {}}", "E_ENSURES_CONSTRUCTOR"),
        (
            "trait T {function f() : Int ensures true}",
            "E_ENSURES_ABSTRACT",
        ),
        (
            "class P {new() {}}\nfunction take(move p : P) : Bool = true\nfunction f() : P ensures take(result) = P()",
            "E_MOVE_BORROWED",
        ),
        (
            "class P {public var n : Int\nnew() {this.n=0}\npublic mut function change() : Bool {this.n=1\ntrue}}\nfunction f() : P ensures result.change() = P()",
            "E_MUT_PARAMETER",
        ),
    ] {
        let result = checked(source);
        assert!(
            result.diagnostics.iter().any(|d| d.code == code),
            "{source}\nExpected {code}, got {:?}",
            result.diagnostics
        );
    }
}

/// Checks a source and returns the codes of its diagnostics.
fn codes(source: &str) -> Vec<&'static str> {
    checked(source).diagnostics.iter().map(|d| d.code).collect()
}

fn has_z3() -> bool {
    std::process::Command::new("z3")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

const POSITIVE: &str = "theorem positive(n : Int) : n > 0 requires h : n >= 1 { smt }\nfunction half(n : Int) : Int requires positive(n) = n / 2\n";

#[test]
fn whole_number_bounds_are_tightened_for_proofs() {
    if !has_z3() {
        return;
    }
    // Over the whole numbers, `x > 0` is `x >= 1`, so a loop or an if on `> 0` proves the call.
    assert_eq!(
        statuses(&format!(
            "{POSITIVE}function f(x : Int) : Int {{ if x > 0 {{ return half(x) }}\nreturn 0 }}"
        )),
        [Proved]
    );
    assert!(
        codes(&format!(
            "{POSITIVE}function f(x : Int) : Int {{ if x > 0 {{ return half(x) }}\nreturn 0 }}"
        ))
        .is_empty()
    );
    // `x >= 0` does not prove `x >= 1`.
    assert!(
        codes(&format!(
            "{POSITIVE}function f(x : Int) : Int {{ if x >= 0 {{ return half(x) }}\nreturn 0 }}"
        ))
        .contains(&"E_PROOF_ASSUMPTION")
    );
}

const BAG: &str = "class Bag {\nprivate var items : Array[Int]\nnew() { this.items = Array.empty() }\npublic function size() : Int = this.items.length()\npublic mut function push(move value : Int) : void { this.items.append(value) }\npublic mut function pop(proof h : this.size() > 0) : Int {\nlet top := this.items[this.items.length() - 1]\nthis.items.pop()\nreturn top\n}\npublic mut function pop() : Option[Int] { return this.items.pop() }\n}\n";

#[test]
fn a_named_while_proof_discharges_a_proof_parameter() {
    assert!(
        codes(&format!(
            "{BAG}let mut s := Bag()\ns.push(1)\nwhile h : s.size() > 0 {{\nprint(s.pop(h))\n}}\n"
        ))
        .is_empty()
    );
}

#[test]
fn a_proof_parameter_needs_its_proof_in_scope_and_a_true_claim() {
    // Without a proof in scope, the name is unknown.
    assert!(
        codes(&format!(
            "{BAG}let mut s := Bag()\ns.push(1)\nprint(s.pop(h))\n"
        ))
        .contains(&"E_UNKNOWN_NAME")
    );
    // A condition that does not imply the claim (`>= 0` does not give `> 0`) cannot prove it.
    assert!(
        codes(&format!(
            "{BAG}let mut s := Bag()\ns.push(1)\nif h : s.size() >= 0 {{\nprint(s.pop(h))\n}}\n"
        ))
        .contains(&"E_PROOF_ASSUMPTION")
    );
}

#[test]
fn a_mutation_drops_the_facts_that_a_proof_relied_on() {
    assert!(codes(&format!(
        "{BAG}let mut s := Bag()\ns.push(1)\ns.push(2)\nwhile h : s.size() > 0 {{\nprint(s.pop(h))\nprint(s.pop(h))\n}}\n"
    ))
    .contains(&"E_PROOF_ASSUMPTION"));
}

#[test]
fn an_overload_without_a_proof_is_not_burdened_by_another_overloads_claim() {
    let source = "class Bag {\nprivate var items : Array[Int]\nnew() { this.items = Array.empty() }\npublic function size() : Int = this.items.length()\npublic mut function push(move value : Int) : void { this.items.append(value) }\npublic mut function pop(proof h : this.size() > 0) : Int { return 0 }\npublic function pop(extra : Int) : Int = extra\n}\nlet mut s := Bag()\ns.push(4)\nprint(s.pop(7))\n";
    assert!(codes(source).is_empty(), "{:?}", codes(source));
}
