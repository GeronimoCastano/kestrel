//! Regression tests for the problems found in the second review (REVIEW2.md).
use kestrel::{
    core::{self, Program},
    modules::{ProjectCheck, check_source},
    native::{self, TemporaryDirectory},
};
use std::process::{Command, Output};

fn checked(source: &str) -> ProjectCheck {
    ProjectCheck {
        modules: vec![check_source("review.kes", source)],
    }
}
fn lower(source: &str) -> Program {
    let project = checked(source);
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    core::lower(&project).unwrap_or_else(|errors| panic!("{errors:#?}\n{source}"))
}
fn execute(source: &str) -> Output {
    let program = lower(source);
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let binary = dir.path().join("program");
    native::build(&program, &binary).unwrap();
    Command::new(&binary).output().unwrap()
}
fn stdout(source: &str) -> String {
    let output = execute(source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn diagnostics(source: &str) -> String {
    checked(source).render_diagnostics()
}

#[test]
fn local_functions_count_as_changes_of_what_they_change() {
    let class = "class C { public var n : Int\n  new(n : Int) { this.n = n } }\n";
    let found = diagnostics(&format!(
        "{class}function f() : Int {{\n    let a := Array.generate(1, i => C(7))\n    function grow() : void {{ for (i in range(1000)) {{ a.append(C(i)) }} }}\n    let first := a[0]\n    grow()\n    first.n\n}}\n"
    ));
    assert!(found.contains("E_BORROW_CONFLICT"), "{found}");
    let found = diagnostics(&format!(
        "{class}class H {{ public var c : C\n  new() {{ this.c = C(1) }} }}\nfunction f() : Int {{\n    let h := H()\n    function reset() : void {{ h.c = C(2) }}\n    let v := h.c\n    reset()\n    v.n\n}}\n"
    ));
    assert!(found.contains("E_BORROW_CONFLICT"), "{found}");
    // Reading through a local function while a view is live stays allowed.
    let output = stdout(&format!(
        "{class}function f() : Int {{\n    let a := Array.generate(1, i => C(7))\n    function size() : Int = a.length()\n    let first := a[0]\n    first.n + size()\n}}\nprint(f())\n"
    ));
    assert_eq!(output, "8\n");
}

#[test]
fn a_stack_overflow_in_a_task_is_still_reported() {
    let output = execute(
        "function f(n : Int) : Int = if n == 0 then 0 else 1 + f(n - 1)\nfunction work() : void with Effects { print(f(100000000)) }\nscope {\n    spawn work()\n}\n",
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("stack overflow"));
}

#[test]
fn question_mark_handles_named_error_enums_and_refuses_values_that_cannot_fail() {
    let errors = "enum AppError extends Error {\n    NotFound(String)\n    Invalid(Int)\n}\nenum Other extends Error {\n    Bad\n}\nfunction find(k : String) : Int or AppError = if k == \"a\" then 1 else NotFound(k)\nfunction check(n : Int) : Int or AppError = if n > 0 then n else Invalid(n)\nfunction both(k : String) : Int or AppError = check(find(k)? - 5)?\nfunction wrap(k : String) : Int or Error = both(k)? * 2\n";
    let output = stdout(&format!(
        "{errors}print(match both(\"a\") {{ NotFound(k) => \"nf \" + k, Invalid(n) => f\"inv {{n}}\", v => v.text() }})\nprint(match both(\"z\") {{ NotFound(k) => \"nf \" + k, Invalid(n) => f\"inv {{n}}\", v => v.text() }})\nprint(match wrap(\"z\") {{ AppError.NotFound(k) => \"open \" + k, _ => \"other\", v => v.text() }})\n"
    ));
    assert_eq!(output, "inv -4\nnf z\nopen z\n");
    let found = diagnostics(&format!(
        "{errors}function wrong() : Int or Other = find(\"a\")?\n"
    ));
    assert!(found.contains("error of type AppError"), "{found}");
    for body in [
        "function five() : Int = 5\nfunction main() : void or Error with Effects {\n    print(five()?)\n}\n",
        "function five() : Int = 5\nfunction main() : void or Error {\n    let x : Int = five()?\n}\n",
    ] {
        let found = diagnostics(body);
        assert!(found.contains("E_PROPAGATION_TYPE"), "{body}\n{found}");
    }
}

#[test]
fn recursive_changes_through_option_fields_compile_as_tail_calls() {
    let output = stdout(
        "class N {\n    public var next : Option[N]\n    public final v : Int\n    new(v : Int) { this.v = v; this.next = None }\n    public mut function push(x : Int) : void {\n        if let Some(n) := this.next { n.push(x) } else { this.next = Some(N(x)) }\n    }\n    public function len() : Int = 1 + (if let Some(n) := this.next { n.len() } else { 0 })\n}\nfunction insert(mut t : N, x : Int) : void {\n    match t.next {\n        Some(n) => { insert(n, x) },\n        None => { t.next = Some(N(x)) },\n    }\n}\nlet h := N(0)\nfor (i in range(1000)) { h.push(i) }\ninsert(h, 7)\nprint(h.len())\n",
    );
    assert_eq!(output, "1002\n");
}

#[test]
fn break_and_continue_work_as_match_arms() {
    let output = stdout(
        "let mut total := 0\nfor (x in [1, 2, 3, 4, 5, 6]) {\n    match x {\n        2 => continue,\n        5 => break,\n        _ => { total += x },\n    }\n}\nprint(total)\n",
    );
    assert_eq!(output, "8\n");
}

#[test]
fn lists_sort_by_key_and_tuples_records_and_enums_order_by_their_parts() {
    let output = stdout(
        "record P(name : String, age : Int)\nenum Level { Low, Mid, High }\nlet ps := [P(\"b\", 3), P(\"a\", 5), P(\"a\", 1)]\nprint(ps.sorted(key = p => p.age))\nprint(ps.sorted())\nlet pairs := [(\"b\", 2), (\"a\", 2), (\"a\", 1)]\nprint(pairs.sorted(key = p => -p[1]))\nprint(pairs.sorted())\nprint([High, Low, Mid].sorted())\nprint(Low.compare(High))\nprint(SortedSet(P(\"z\", 1), P(\"a\", 2)))\n",
    );
    assert_eq!(
        output,
        "[P(name = a, age = 1), P(name = b, age = 3), P(name = a, age = 5)]\n[P(name = a, age = 1), P(name = a, age = 5), P(name = b, age = 3)]\n[(b, 2), (a, 2), (a, 1)]\n[(a, 1), (a, 2), (b, 2)]\n[Low, Mid, High]\n-1\nSortedSet(P(name = a, age = 2), P(name = z, age = 1))\n"
    );
}

#[test]
fn collections_cannot_change_inside_a_loop_over_them_or_alias_one_element() {
    let class = "class C { public var n : Int\n  new(n : Int) { this.n = n } }\nfunction both(mut a : C, mut b : C) : void { a.n = 1\n b.n = 2 }\n";
    for body in [
        "let a := Array.generate(2, i => C(i))\nfor (c in a) { a.append(C(9)) }\n",
        "let m : MutMap[Int, Int] = MutMap()\nm.put(1, 1)\nfor ((k, v) in m) { m.put(k + 1, v) }\n",
        "let xs := Array.generate(2, i => C(i))\nboth(xs[0], xs[0])\n",
        "let xs := Array.generate(2, i => C(i))\nlet i := 1\nboth(xs[i], xs[0])\n",
    ] {
        let found = diagnostics(&format!("{class}{body}"));
        assert!(found.contains("E_BORROW_CONFLICT"), "{body}\n{found}");
    }
    let output = stdout(&format!(
        "{class}let xs := Array.generate(2, i => C(i))\nboth(xs[0], xs[1])\nprint(xs[0].n + xs[1].n)\n"
    ));
    assert_eq!(output, "3\n");
}

#[test]
fn checking_time_grows_linearly_with_the_number_of_functions() {
    let mut source = String::new();
    for i in 0..4000 {
        source.push_str(&format!("function f{i}(x : Int) : Int = x + {i}\n"));
    }
    let start = std::time::Instant::now();
    let found = diagnostics(&source);
    assert!(!found.contains("error["), "{found}");
    // A debug build took about 9 seconds before the fix; linear checking takes well under one.
    assert!(start.elapsed().as_secs_f64() < 5.0, "{:?}", start.elapsed());
}

#[test]
fn smaller_runtime_and_text_fixes() {
    let output = stdout(
        r#"enum AppError extends Error {
    Bad(Int)
}
enum Status { Active, Off }
let s := "x\"y"
print(f"{s:?}")
print(f"{[s, "a"]:?}")
print([s])
print(f"{3:.2f} {7:.0f}")
let set : MutSet[String] = MutSet()
set.add("b")
set.add("a")
let mut letters := 0
for (x in set) { letters += x.length() }
print(letters)
let counts : MutMap[String, Int] = MutMap()
counts.put("k", 1)
print(counts)
print(Error("boom"))
print(Bad(3))
let m := Map("a" => Active)
print(m.get("a") == Some(Active))
print([1, 2, 3] |> (xs => xs.sum()))
"#,
    );
    assert_eq!(
        output,
        "\"x\\\"y\"\n[\"x\\\"y\", \"a\"]\n[x\"y]\n3.00 7\n2\nMap(k => 1)\nRuntimeError.Message(\"boom\")\nAppError.Bad(3)\ntrue\n6\n"
    );
    let output = execute("let m := -9223372036854775808\nprint(m.abs())\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Int abs overflow"));
    let output = execute("let m := -9223372036854775808\nprint(-m)\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Int negation overflow"));
}

#[test]
fn smaller_checker_fixes() {
    // A follow-on error about a type that was never worked out does not repeat the real one.
    let found = diagnostics("print(\"abc\".nope())\n");
    assert!(found.contains("E_UNKNOWN_MEMBER"), "{found}");
    assert!(!found.contains('?'), "{found}");
    // An empty list with no item type says so before native lowering.
    let found = diagnostics("print([].isEmpty())\n");
    assert!(found.contains("E_TYPE_UNKNOWN"), "{found}");
    // Two overloads that each fit when a numeral becomes a double are ambiguous.
    let found = diagnostics(
        "function g(x : Int, y : double) : Int = 1\nfunction g(x : double, y : Int) : Int = 2\nlet r := g(1, 1)\n",
    );
    assert!(found.contains("E_AMBIGUOUS_CALL"), "{found}");
    // A value moved by the right side of an assignment is not described as a loop.
    let found = diagnostics(
        "class N { public var next : Option[N]\n  new() { this.next = None } }\nlet n := N()\nn.next = Some(n)\n",
    );
    assert!(found.contains("moved by the value on the right"), "{found}");
    // A theorem that relies on `sorry` warns too.
    let found = diagnostics(
        "theorem t(n : Nat) : n = Zero by { sorry }\ntheorem u(n : Nat) : n = Zero { t(n) }\n",
    );
    assert!(found.contains("`u` relies on a theorem"), "{found}");
}
