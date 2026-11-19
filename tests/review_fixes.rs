//! Regression tests for the problems found in the 2026-11-14 review (REVIEW.md).
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
fn list_strings_survive_being_stored_elsewhere() {
    let output = stdout(
        r#"let words := "b a c".split(" ")
let m : MutMap[String, Int] = MutMap()
for (w in words) { m.put(w, 1) }
let s : MutSet[String] = MutSet()
for (w in words) { s.add(w) }
class Keep {
    private var items : Array[String]
    new() { this.items = Array.empty() }
    public mut function push(move value : String) : void { this.items.append(value) }
}
let st := Keep()
for (w in words) { st.push(w) }
let mut ss := SortedSet("z")
for (w in words) { ss = ss.add(w) }
let vm : MutMap[Int, String] = MutMap()
for (w in words) { vm.put(1, w) }
let k := "key"
m.put(k, 2)
print(words)
print(k)
print(m.length())
"#,
    );
    assert_eq!(output, "[b, a, c]\nkey\n4\n");
}

#[test]
fn an_ignored_error_result_is_refused() {
    let save = "function save() : void or Error with Effects { return Error(\"disk full\") }\n";
    for body in [
        "save()\n",
        "function main() : void with Effects {\n    save()\n    print(1)\n}\n",
        "function main() : void or Error with Effects {\n    for (i in range(2)) { save() }\n}\n",
        "function main() : void or Error with Effects {\n    if true { save() }\n}\n",
    ] {
        let found = diagnostics(&format!("{save}{body}"));
        assert!(found.contains("E_UNUSED_ERROR"), "{body}\n{found}");
    }
    for body in [
        "let _ := save()\n",
        "match save() { _ => print(\"handled\") }\n",
        "function main() : void or Error with Effects {\n    save()?\n    save()\n}\n",
    ] {
        let found = diagnostics(&format!("{save}{body}"));
        assert!(!found.contains("error["), "{body}\n{found}");
    }
}

#[test]
fn a_script_question_mark_reports_the_error_and_exits_unsuccessfully() {
    let output = execute(
        "function save() : void or Error with Effects { return Error(\"disk full\") }\nprint(\"before\")\nsave()?\nprint(\"after\")\n",
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"before\n");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("disk full"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn enums_and_records_compare_and_print_by_their_fields() {
    let output = stdout(
        r#"enum Color { Red, Green }
record Point(x : Int, y : Int)
enum Tree[T] {
    Leaf
    Node(Tree[T], T, Tree[T])
}
enum Json {
    JNull
    JStr(String)
    JArr(List[Json])
}
record Custom(v : Int) {
    has Text
    public function text() : String = f"<{this.v}>"
}
print(Red == Green)
print(Point(1, 2) == Point(1, 2))
let t : Tree[String] = Node(Leaf, "a", Node(Leaf, "b", Leaf))
print(t == Node(Leaf, "a", Node(Leaf, "b", Leaf)))
print(JArr([JStr("x"), JNull]) == JArr([JStr("x")]))
print(Red)
print(Point(1, 2))
print(t)
print(f"{JArr([JStr("x"), JNull]):?}")
print(Custom(3))
print([Some(Point(0, 1))])
"#,
    );
    assert_eq!(
        output,
        "false\ntrue\ntrue\nfalse\nRed\nPoint(x = 1, y = 2)\nNode(Leaf, a, Node(Leaf, b, Leaf))\nJArr([JStr(\"x\"), JNull])\n<3>\n[Some(Point(x = 0, y = 1))]\n"
    );
    let found = diagnostics(
        "class Plain {\npublic final v : Int\nnew(v : Int) { this.v = v }\n}\nrecord Holder(p : Plain)\nlet same := Holder(Plain(1)) == Holder(Plain(1))\n",
    );
    assert!(found.contains("compares by its fields"), "{found}");
}

#[test]
fn tuple_enum_and_record_keys_hash_by_their_parts() {
    let output = stdout(
        r#"enum Color { Red, Green, Blue }
record Cell(row : Int, col : Int, name : String)
let m : MutMap[(Int, Int), Int] = MutMap()
for (i in range(100000)) { m.put((i, i + 1), i) }
print(m.length())
print(m.get((5, 6)))
print(m.get((5, 7)))
let r : MutMap[Cell, Int] = MutMap()
for (i in range(20000)) { r.put(Cell(i, i, "c"), i) }
print(r.get(Cell(7, 7, "c")))
print(r.get(Cell(7, 7, "d")))
let c : MutMap[Color, Int] = MutMap()
c.put(Red, 1)
c.put(Blue, 3)
c.put(Red, 5)
print(c.length())
print(Set((1, "a"), (2, "b"), (1, "a")).length())
"#,
    );
    assert_eq!(output, "100000\nSome(5)\nNone\nSome(7)\nNone\n2\n2\n");
}

#[test]
fn functions_inside_functions_compile_with_their_captures() {
    let output = stdout(
        r#"class Acc {
    private final base : Int
    new(base : Int) { this.base = base }
    public function scaled(xs : List[Int]) : Int {
        function one(x : Int) : Int = x * this.base
        let mut t := 0
        for (x in xs) { t += one(x) }
        t
    }
}
function outer(k : Int, label : String) : String {
    function show(n : Int) : String = f"{label}:{n * k}"
    function twice(n : Int) : String = show(n) + "," + show(n * 2)
    function fact(n : Int) : Int = if n < 2 then 1 else n * fact(n - 1)
    twice(fact(3))
}
function counter() : Int {
    let mut total := 0
    function add(n : Int) : void { total = total + n }
    add(2)
    add(5)
    total
}
function squares(xs : List[Int]) : List[Int] {
    function sq(x : Int) : Int = x * x
    xs.map(sq).toList()
}
print(Acc(3).scaled([1, 2]))
print(outer(2, "x"))
print(counter())
print(squares([1, 2, 3]))
print(~5)
"#,
    );
    assert_eq!(output, "9\nx:12,x:24\n7\n[1, 4, 9]\n-6\n");
    let found = diagnostics("let xs := []\nprint(xs.length())\n");
    assert!(found.contains("E_TYPE_UNKNOWN"), "{found}");
    let found = diagnostics("let d := ~1.5\n");
    assert!(found.contains("E_TYPE"), "{found}");
}

#[test]
fn a_stack_overflow_is_reported() {
    let output = execute(
        "function f(n : Int) : Int = if n == 0 then 0 else 1 + f(n - 1)\nprint(f(100000000))\n",
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("stack overflow"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn elements_tuples_and_nested_receivers_borrow_safely() {
    let class = "class C {\n    public var n : Int\n    new() { this.n = 0 }\n    public mut function inc() : void { this.n += 1 }\n    public mut function bump(x : Int) : void { this.n += x }\n    public mut function next() : Int {\n        this.n += 1\n        this.n\n    }\n}\n";
    let output = stdout(&format!(
        r#"{class}class Holder {{
    private var items : Array[C]
    new() {{ this.items = Array.generate(2, i => C()) }}
    public mut function bump(i : Int) : void {{ this.items[i].inc() }}
    public function get(i : Int) : Int = this.items[i].n
}}
function bumpAll(mut xs : Array[C]) : void {{
    for (i in range(xs.length())) {{ xs[i].inc() }}
}}
enum Light {{
    On(String)
    Off
}}
function both(a : Light, b : Light) : String = match (a, b) {{
    (On(x), On(y)) => x + y,
    (On(x), Off) => x,
    _ => "none",
}}
let h := Holder()
h.bump(1)
print(h.get(1))
let a := Array.generate(2, i => C())
bumpAll(a)
print(a[1].n)
let c := C()
c.bump(c.next())
print(c.n)
let first := On("a")
print(both(first, On("b")))
print(both(first, Off))
"#
    ));
    assert_eq!(output, "1\n1\n2\nab\na\n");
    for (body, code) in [
        (
            "function bad(xs : Array[C]) : void { xs[0].inc() }\n",
            "E_MUT_PARAMETER",
        ),
        (
            "class H {\n  private var items : Array[C]\n  new() { this.items = Array.generate(1, i => C()) }\n  public function bad() : void { this.items[0].inc() }\n}\n",
            "E_MUT_RECEIVER",
        ),
        (
            "function grow(mut a : Array[C]) : Int {\n    a.append(C())\n    1\n}\nlet a := Array.generate(2, i => C())\na[0].bump(grow(a))\n",
            "E_BORROW_CONFLICT",
        ),
    ] {
        let found = diagnostics(&format!("{class}{body}"));
        assert!(found.contains(code), "{body}\n{found}");
    }
}

#[test]
fn proofs_use_unbounded_integers_but_keep_nat_subtraction_partial() {
    let base = "function plus(a : Nat, b : Nat) : Nat = match a {\n    Zero => b,\n    Succ(k) => Succ(plus(k, b)),\n}\n";
    for theorem in [
        "theorem assoc(a : Nat, b : Nat, c : Nat) : plus(plus(a, b), c) = plus(a, plus(b, c)) by {\n    induction a {\n        Zero => { rfl }\n        Succ(k) ih => { simp [ih] }\n    }\n}\n",
        "theorem succR(n : Nat, m : Nat) : plus(n, Succ(m)) = Succ(plus(n, m)) by {\n    induction n {\n        Zero => { rfl }\n        Succ(k) ih => { simp [ih] }\n    }\n}\n",
    ] {
        let found = diagnostics(&format!("{base}{theorem}"));
        assert!(!found.contains("error["), "{theorem}\n{found}");
    }
    let found = diagnostics(
        "theorem bad(a : Nat, b : Nat) : a - b + b = a by {\n    induction a {\n        Zero => { rfl }\n        Succ(k) ih => { rfl }\n    }\n}\n",
    );
    assert!(found.contains("error["), "{found}");
}

#[test]
fn mutmap_update_changes_objects_in_place_and_replaces_values() {
    let output = stdout(
        r#"class C {
    public var n : Int
    new() { this.n = 0 }
    public mut function inc() : void { this.n += 1 }
}
record Stats(total : Int, label : String)
let m : MutMap[String, C] = MutMap()
m.put("k", C())
print(m.update("k", c => c.inc()))
print(m.update("missing", c => c.inc()))
match m.get("k") { Some(c) => print(c.n), None => print("none") }
let words := "the cat and the dog and the end".split(" ")
let counts : MutMap[String, Int] = MutMap()
for (w in words) {
    if !counts.update(w, n => n + 1) { counts.put(w, 1) }
}
print(counts.get("the"))
print(words)
let stats : MutMap[String, Stats] = MutMap()
stats.put("a", Stats(1, "x"))
let _ := stats.update("a", s => Stats(s.total + 5, s.label))
print(stats.get("a"))
"#,
    );
    assert_eq!(
        output,
        "true\nfalse\n1\nSome(3)\n[the, cat, and, the, dog, and, the, end]\nSome(Stats(total = 6, label = x))\n"
    );
}
