use std::{
    io::Write,
    process::{Command, Output, Stdio},
};
fn session(source: &str) -> Output {
    session_at(source, std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
}
fn session_at(source: &str, root: &std::path::Path) -> Output {
    let mut process = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["session", "--root"])
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    process.wait_with_output().unwrap()
}
fn shows(source: &str, expected: &str) {
    let output = session(source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    assert!(output.stderr.is_empty());
}
#[test]
fn both_stage_five_acceptance_fixtures_display_their_last_expression() {
    shows(include_str!("../testdata/language/script.kes"), "3\n");
    shows(
        include_str!("../testdata/language/03-programs.kes"),
        "total is 6\n",
    );
}

#[test]
fn written_mutable_callback_types_preserve_state_across_entries() {
    shows(
        &format!(
            "{}alias Counter = String\nprint(next(count))\nprint(ticker.next())\n",
            include_str!("../testdata/language/callbacks.kes")
        ),
        "1\n2\n11\n12\n21\n3\n13\n",
    );
}

#[test]
fn collection_counts_and_copied_defaults_survive_session_versions() {
    shows(
        &format!(
            "{}let values:=[1,2,3]\nlet size:=values.length\nlet values:=[4]\nprint(size())\nprint(count(values))\nfunction saved() : () -> Int {{let local:=[1,2]\nlocal.length}}\nlet f:=saved()\nprint(f())\nfunction snapshot() : Int {{let mut text:=\"abc\"\nlet size:=text.length\ntext=\"x\"\nsize()}}\nprint(snapshot())\n",
            include_str!("../testdata/language/collectioncounts.kes")
        ),
        "3\n2\n2\n2\n2\n2\nkept text\n3\n1\n2\n3\n",
    );
}

#[test]
fn saved_method_results_widen_to_error_unions_between_entries() {
    shows(
        "function count() : Int or Error {let length:=[1,2].length\nlength()}\nmatch count() {n=>n,_=>0}\nfunction label() : String or Error {let text:=42.text\ntext()}\nmatch label() {s=>s,_=>\"error\"}\n",
        "2\n42\n",
    );
}

#[test]
fn named_arrow_input_labels_remain_stable_between_session_versions() {
    shows(
        include_str!("../testdata/language/functioninputs.kes"),
        "1\n2\nowned value\n5\n",
    );
    shows(
        "let value:=4\nalias Step = (value : Int) -> Int\nfunction step(actual : Int) : Int=actual+1\nlet f : Step=step\nf(value=3)\nlet value:=5\nf(value=6)\n",
        "4\n7\n",
    );
}

#[test]
fn plain_defaults_read_borrowed_objects_and_callbacks_between_entries() {
    shows(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction text(p : Point,label : String = p.label) : String=label\nfunction value(f : () -> String,label : String = f()) : String=label\ntext(Point(\"session\"+\" object\"))\nvalue(() => \"session\"+\" callback\")\n",
        "session object\nsession callback\n",
    );
}

#[test]
fn scaled_length_signatures_work_across_session_entries() {
    shows(include_str!("../testdata/language/lengths.kes"), "6\n12\n");
}

#[test]
fn multiterm_length_facts_work_across_session_entries() {
    shows(
        include_str!("../testdata/language/lengthmatches.kes"),
        "3\n",
    );
}

#[test]
fn mixed_subclass_views_persist_across_session_entries() {
    shows(
        include_str!("../testdata/language/classviews.kes"),
        "4.0\nbase figure\n4.0\n1.0\ntrue\n100.0\n4.0\n3.0\n12.0\n",
    );
    shows(
        &format!(
            "{}\nlet saved := bigger\nclass Later extends Figure {{new() {{this.scale=1.0\nthis.label=\"later\"}}\n@override public function area() : double = 42.0}}\nlet later := Later()\nlet retained := saved(square,later)\nretained.me().area()\nfunction bigger(a : Figure,b : Figure) : Figure = b\nsaved(square,figure).area()\nbigger(square,figure).area()",
            include_str!("../testdata/language/classviews.kes")
        ),
        "4.0\nbase figure\n4.0\n1.0\ntrue\n100.0\n4.0\n3.0\n12.0\n42.0\n12.0\n1.0\n",
    );
}

#[test]
fn defaults_keep_definition_versions_and_reject_temporary_escape() {
    shows(
        "let seed:=1\nfunction read(n : Int = seed) : Int = n\nlet saved:=read\nlet seed:=9\nread()\nfunction caller(seed : Int) : Int = read()\ncaller(7)\nfunction read(n : Int = seed) : Int = n\nread()\nsaved()\n",
        "1\n1\n9\n1\n",
    );
    let output = session(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction identity(p : Point = Point(\"fresh\" + \" value\")) : Point = p\nfunction bad() : Point = identity()\nidentity().label\n",
    );
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"fresh value\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("E_BORROW_TEMPORARY"));
}

#[test]
fn long_call_chains_cannot_hide_a_borrow_in_a_session() {
    let mut source = "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction id0(p : Point) : Point = p\n".to_owned();
    for i in 1..24 {
        source.push_str(&format!(
            "function id{i}(p : Point) : Point = id{}(p)\n",
            i - 1
        ));
    }
    source.push_str("function bad() : Point {let p := Point(\"local\")\nid23(p)}\nlet p := Point(\"kept\" + \" alive\")\nid23(p).label\np.label\n");
    source.push_str("function consume(move p : Point) : void {}\nfunction badLoop() : String {\n");
    for i in 0..8 {
        source.push_str(&format!(
            "let p{i} := Point(\"p\" + {i}.text())\nlet mut a{i} := id23(p{i})\n"
        ));
    }
    source.push_str("for (let i : Int = 0; i < 8; i = i + 1) {\n");
    for i in 0..7 {
        source.push_str(&format!("a{i}=a{}\n", i + 1));
    }
    source.push_str("}\nconsume(p7)\na0.label}\n");
    let output = session(&source);
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"kept alive\nkept alive\n");
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(errors.contains("E_BORROW_ESCAPE"), "{errors}");
    assert!(errors.contains("E_BORROW_CONFLICT"), "{errors}");
    assert!(!errors.contains("session__"), "{errors}");
}

#[test]
fn primitive_import_cycles_work_across_session_entries() {
    shows(
        "import testdata.language.cycleeven as cycle\ncycle.even(2000000)\ncycle.relay(\"heap\" + \" value\",100000)\nimport testdata.language.cycleodd as odd\nodd.odd(2000001)\ncycle.even(4)\n",
        "true\nheap value\ntrue\ntrue\n",
    );
}

#[test]
fn data_import_cycles_work_across_session_entries() {
    shows(
        "import testdata.language.cycleexpr.{Expr, eval}\nimport testdata.language.cycleblock.{Block}\nlet one := Expr.Group(Block.Then(Expr.Lit(5), Block.Done))\neval(one)\nlet two := Expr.Group(Block.Then(Expr.Lit(1), Block.Then(Expr.Lit(2), Block.Done)))\neval(two)\n",
        "5\n3\n",
    );
}

#[test]
fn values_go_into_new_boxes_across_session_entries() {
    shows(
        "trait Shape {\nfunction area() : double\n}\nclass Square {\nhas Shape\npublic final side : double\nnew(side : double) {this.side = side}\npublic function area() : double = this.side * this.side\n}\nlet shapes : List[Box[dyn Shape]] = [Square(2.0), Square(3.0)]\nlet b : Box[dyn Shape] = Square(5.0)\nb.area()\nlet more : List[Box[dyn Shape]] = [Square(1.0), Square(3.0), Square(2.0)]\nfunction total(xs : List[Box[dyn Shape]]) : double {\nlet mut sum := 0.0\nfor (let s in xs) {\nsum = sum + s.area()\n}\nsum\n}\ntotal(more)\n",
        "25.0\n14.0\n",
    );
}

#[test]
fn a_later_subclass_stands_in_for_an_earlier_function_parameter() {
    shows(
        "class Figure {\nnew() {}\npublic function area() : double = 0.0\n}\nfunction describe(f : Figure) : double = f.area()\ndescribe(Figure())\nclass Square extends Figure {\npublic final side : double\nnew(side : double) {this.side = side}\n@override public function area() : double = this.side * this.side\n}\nlet sq := Square(3.0)\ndescribe(sq)\nsq.area()\n",
        "0.0\n9.0\n9.0\n",
    );
}

#[test]
fn stored_copyable_cursor_options_survive_session_entries_and_later_steps() {
    shows(
        "class Cursor {has Iterator\npublic var value : Option[String]\npublic var index : Int\nnew() {this.value=None\nthis.index=0}\npublic mut function next() : Option[String] {this.index=this.index+1\nthis.value=if this.index > 2 then None else Some(\"p\" + this.index.text())\nthis.value}}\nlet cursor : Box[dyn Iterator(Item = String)] = box(Cursor())\nlet first := cursor.next()\ncursor.next()\nfirst\ncursor.next()\nfirst\n",
        "Some(p2)\nSome(p1)\nNone\nSome(p1)\n",
    );
}

#[test]
fn changing_field_aliases_keep_their_owners_in_session_functions() {
    let mut source = include_str!("../testdata/language/aliases.kes").to_owned();
    source.push_str("function take(move h : Holder) : void {}\nfunction bad(other : Bool) : String {let left := Holder(Label(\"left\"))\nlet right := Holder(Label(\"right\"))\nlet mut view := left.label\nif other {view=right.label}\ntake(left)\nview.value}\npick(true)\n");
    let output = session(&source);
    assert!(!output.status.success());
    assert_eq!(
        output.stdout,
        b"left value | left value\nleft value | right value\nleft value | right value\n"
    );
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(errors.contains("E_BORROW_CONFLICT"), "{errors}");
    assert!(!errors.contains("session__"), "{errors}");
}

#[test]
fn postconditions_keep_their_result_binding_and_definition_versions_in_a_session() {
    shows(
        "let result := 900\nfunction value(n : Int) : Int ensures result == n = n\nvalue(2)\nlet saved := value\nfunction value(n : Int) : Int ensures result == (n + 1) = n + 1\nvalue(2)\nsaved(2)\nresult\n",
        "2\n3\n2\n900\n",
    );
}

#[test]
fn a_session_reports_postcondition_failure_before_displaying_a_result() {
    let output =
        session("let limit := 0\nfunction broken() : Int ensures result > limit = 0\nbroken()\n");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(
        errors.contains("Postcondition failed: result > limit"),
        "{errors}"
    );
    assert!(!errors.contains("session__"), "{errors}");
}

#[test]
fn owned_recursive_payload_moves_work_in_a_session() {
    shows(
        "enum Chain[T] {End,Link(T,Chain[T])}\nfunction reverse(move xs : Chain[T], move acc : Chain[T]) : Chain[T] = match xs {End => acc,Link(x,rest) => reverse(rest,Link(x,acc)),}\nfunction first(xs : Chain[T], fallback : T) : T = match xs {End => fallback,Link(x,_) => x,}\nlet items := reverse(Link(\"left\" + \"most\",Link(\"right\" + \"most\",End)),End)\nfirst(items,\"empty\")\nfirst(items,\"empty\")\nfunction pop(move xs : Chain[T]) : Chain[T] = match xs {End => End,Link(_,rest) => rest,}\nlet remaining := pop(items)\nfirst(remaining,\"empty\")\n",
        "rightmost\nrightmost\nleftmost\n",
    );
}

#[test]
fn checked_theorems_and_requirements_survive_session_entries() {
    shows(
        "function add(a : Nat,b : Nat) : Nat = match a {Zero => b,Succ(k) => Succ(add(k,b)),}\n@simp\ntheorem zeroRight(n : Nat) : add(n,Zero) = n {induction n {Zero => rfl,Succ(k) ih => congrArg(Succ,ih),}}\ntheorem twice(n : Nat) : add(add(n,Zero),Zero) = n {simp}\nfunction count(n : Nat) : Nat requires zeroRight(n) {n}\ncount(3)\ncount(4)\n",
        "3\n4\n",
    );
}

#[test]
fn theorem_bounds_and_local_names_survive_session_versions() {
    shows(
        "let n := \"global\"\nfunction identity[T : Equals](n : T) : T = n\ntheorem same[T : Equals](n : T) : identity(n) = n {rfl}\nfunction keep[U : Equals](n : U) : U requires same(n) = identity(n)\nfunction identity[T : Equals](n : T) : T = n\nkeep(6)\nn\n",
        "6\nglobal\n",
    );
    let output = session(
        "function identity[T : Equals](x : T) : T = x\ntheorem same(x : T) : identity(x) = x {rfl}\ntheorem same[T : Equals](x : T) : identity(x) = x {rfl}\nfunction keep[U : Equals](x : U) : U requires same(x) = x\nkeep(9)\n",
    );
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"9\n");
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(errors.contains("E_PROOF_BOUND"), "{errors}");
    assert!(!errors.contains("session__"), "{errors}");
}

#[test]
fn an_undefined_proof_is_rejected_before_a_bounded_replacement() {
    // A Nat subtraction below Zero stays undefined in a proof (REVIEW 2.8).
    let output = session(
        "theorem same(n : Nat) : (n - 1) = (n - 1) {rfl}\ntheorem same(n : Nat) : (n - 1) = (n - 1) requires positive : n > Zero {rfl}\nfunction f(n : Nat) : Nat requires same(n) {n}\nf(6)\n",
    );
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"6\n");
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(errors.contains("E_PROOF_UNDEFINED"), "{errors}");
    assert!(!errors.contains("session__"), "{errors}");
}

#[test]
fn imported_function_definitions_remain_available_to_session_proofs() {
    shows(
        "import testdata.language.proofmath as math\ntheorem same(n : Nat) : math.identity(n) = n {rfl}\ntheorem zeroRight(n : Nat) : math.add(n,Zero) = n {induction n {Zero => rfl,Succ(k) ih => congrArg(Succ,ih),}}\nfunction count(n : Nat) : Nat requires zeroRight(n) {math.identity(n)}\ncount(8)\nimport std.str as math\ncount(9)\nmath.length(\"λ\")\n",
        "8\n9\n1\n",
    );
}
#[test]
fn entries_keep_state_without_replaying_their_output() {
    shows(
        "let mut total := 0\nprint(\"once\")\ntotal = total + 1\ntotal\ntotal = total + 1\ntotal\nlet old := total\nlet total := total + 10\nold\ntotal\n",
        "once\n1\n2\n2\n12\n",
    );
}
#[test]
fn redefinitions_keep_earlier_implementations_and_saved_functions() {
    shows(
        "function amount() : Int = 1\nfunction twice() : Int = amount() * 2\nlet saved := amount\nfunction amount() : Int = 5\ntwice()\namount()\nsaved()\nlet n := 3\nfunction before() : Int = n\nlet n := \"new\"\nbefore()\nn\n",
        "2\n5\n1\n3\nnew\n",
    );
}
#[test]
fn function_names_show_their_signature_and_original_source() {
    shows(
        "function add(n : Int) : Int = n + 1\nadd\nlet twice := (n : Int) => n * 2\ntwice\n",
        "add : (n : Int) -> Int\nfunction add(n : Int) : Int = n + 1\ntwice : (n : Int) -> Int\nlet twice := (n : Int) => n * 2\n",
    );
}
#[test]
fn classes_closures_and_collection_versions_survive_new_libraries() {
    shows(
        r#"class Label { has Text
public final textValue : String
new(textValue : String) { this.textValue = textValue }
public function text() : String = this.textValue
public function debug() : String = "debug:" + this.textValue }
let label := Label("value")
label
function make(n : Int) : Int -> Int = move (x : Int) => x + n
let add := make(7)
let mut m := Map("a" => 1)
let old := m
m = m.put("a", 2)
old.get("a")
m.get("a")
add(3)
label
"#,
        "debug:value\nSome(1)\nSome(2)\n10\ndebug:value\n",
    );
}
#[test]
fn counters_keep_changing_when_the_same_function_is_called_again() {
    shows(
        "let mut count := 0\nlet amount := () => { count = count + 1\ncount }\namount()\namount()\ncount\nlet amount := () => 100\namount()\ncount\n",
        "1\n2\n2\n100\n2\n",
    );
}
#[test]
fn rejected_entries_leave_existing_values_available() {
    let output = session(
        "let mut total := 1\ntotal = \"bad\"\ntotal\nlet total := missing\ntotal\nlet total := 7\ntotal\n",
    );
    assert!(!output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "1\n1\n7\n");
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(errors.contains("E_TYPE"));
    assert!(errors.contains("E_UNKNOWN_NAME"));
    assert!(!errors.contains("session__"), "{errors}");
}
#[test]
fn moves_and_borrows_are_checked_across_entries() {
    let output = session(
        "class Point { public final n : Int\nnew(n : Int) { this.n = n } }\nlet p := Point(3)\nlet q := p\np.n\nq.n\n",
    );
    assert!(!output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "3\n");
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(errors.contains("E_USE_AFTER_MOVE"), "{errors}");
}
#[test]
fn session_input_is_left_for_read_line_while_the_entry_runs() {
    shows(
        "function line() : String or Error with Effects = readLine()\nmatch line() { s => s, _ => \"error\", }\nuser input\n3 + 4\n",
        "user input\n7\n",
    );
}
#[test]
fn multiline_cancellation_and_quitting_do_not_execute_discarded_code() {
    shows(
        "function cancelled(\n:cancel\n3\n:quit\nprint(\"unread\")\n",
        "3\n",
    );
}

#[test]
fn old_callers_keep_their_code_while_explicit_mutable_state_changes() {
    shows(
        r#"class Counter { var n : Int
new() { this.n = 0 }
public mut function increment() : Int { this.n = this.n + 1
this.n } }
function amount(mut c : Counter) : Int = c.increment()
function twice(mut c : Counter) : Int = amount(c) + amount(c)
let c := Counter()
twice(c)
twice(c)
function amount(mut c : Counter) : Int = 100
amount(c)
twice(c)
"#,
        "3\n7\n100\n11\n",
    );
}

#[test]
fn imports_can_be_added_and_rebound_without_changing_earlier_callers() {
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    std::fs::write(
        dir.path().join("one.kes"),
        "public function value() : Int = 1",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("two.kes"),
        "public function value() : Int = 2",
    )
    .unwrap();
    let source = "import one.{value}\nfunction earlier() : Int = value()\nlet saved := value\nimport two.{value}\nvalue()\nearlier()\nsaved()\nimport std.str.{length}\nlength(\"λ\")\nimport std.math.{sqrt}\nsqrt(9.0)\nimport std.str.{length}\nlength(\"abc\")\n";
    let mut process = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["session", "--root"])
        .arg(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = process.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "2\n1\n1\n1\n3.0\n3\n"
    );
}

#[test]
fn saved_trait_callers_accept_implementations_from_later_entries() {
    shows(
        r#"trait Label { function name() : String }
class First { has Label
new() {}
public function name() : String = "first" }
function name(value : &dyn Label) : String = value.name()
let read := name
let first : Box[dyn Label] = box(First())
read(first)
class Second { has Label
new() {}
public function name() : String = "second" }
let second : Box[dyn Label] = box(Second())
read(second)
read(first)
sealed trait Code { function code() : Int }
class One { has Code
new() {}
public function code() : Int = 1 }
function code(value : &dyn Code) : Int = value.code()
let saved := code
let one : Box[dyn Code] = box(One())
saved(one)
class Two { has Code
new() {}
public function code() : Int = 2 }
let two : Box[dyn Code] = box(Two())
saved(two)
"#,
        "first\nsecond\nfirst\n1\n2\n",
    );
}

#[test]
fn native_calls_and_session_arguments_work_after_other_entries() {
    let mut process = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["session", "--", "one two", "λ", ""])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process.stdin.take().unwrap().write_all(b"import std.process.{args}\nargs\nlet copy := args\nimport c \"math.h\"\nunsafe { sqrt(9.0) }\ncopy\n").unwrap();
    let output = process.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[one two, λ, ]\n3.0\n[one two, λ, ]\n"
    );
}

/// Whether a Python to embed exists. CI sets KESTREL_TEST_REQUIRE_PYTHON, so a missing
/// Python fails the test there instead of skipping it.
fn python_ready() -> bool {
    let ready = kestrel::native::python_available();
    assert!(
        ready || std::env::var_os("KESTREL_TEST_REQUIRE_PYTHON").is_none(),
        "KESTREL_TEST_REQUIRE_PYTHON is set, but no Python to embed was found"
    );
    ready
}

#[test]
fn python_interpreter_state_is_shared_by_separate_entries() {
    if !python_ready() {
        return;
    }
    shows(
        r#"import python "sys" as sys
function change() : void or Error with Effects { let ignored : void = sys.setrecursionlimit(901)? }
match change() { _ => "changed", }
function read() : Int or Error with Effects = sys.getrecursionlimit()
match read() { n => n.text(), _ => "error", }
import python "math" as math
function root() : double or Error with Effects = math.sqrt(16.0)
match root() { n => n.text(), _ => "error", }
match read() { n => n.text(), _ => "error", }
"#,
        "changed\n901\n4.0\n901\n",
    );
}

#[test]
fn generic_local_functions_stay_polymorphic_as_new_declarations_are_added() {
    shows(
        "let identity := x => x\nidentity(3)\nfunction add(a : Int,b : Int) : Int = a + b\nidentity(\"text\")\nlet n := 5\nlet plus := (x : Int) => x + n\nplus(2)\nlet n := 100\nplus(2)\n",
        "3\ntext\n7\n7\n",
    );
}

/// Runs a session over a project with one C file whose function counts its calls.
fn session_with_project_c(dir: &std::path::Path) -> std::process::Output {
    std::fs::write(
        dir.join("local.h"),
        "#include <stdint.h>\nint64_t next(void);\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("local.c"),
        "#include \"local.h\"\nstatic int64_t count; int64_t next(void) { return ++count; }\n",
    )
    .unwrap();
    let mut process = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["session", "--root"])
        .arg(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(
            b"import c \"local.h\"\nunsafe { next().toInt() }\n0\nunsafe { next().toInt() }\n",
        )
        .unwrap();
    process.wait_with_output().unwrap()
}

#[test]
fn project_c_state_is_shared_instead_of_being_linked_again_for_each_entry() {
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let output = session_with_project_c(dir.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "1\n0\n2\n");
}

#[test]
fn redefining_a_class_keeps_earlier_objects_and_parameter_types_intact() {
    shows(
        r#"class Point { has Text
public final n : Int
new(n : Int) { this.n = n }
public function text() : String = this.n.text() }
let first := Point(7)
function old(p : Point) : Int = p.n
class Point { has Text
public final label : String
new(label : String) { this.label = label }
public function text() : String = this.label }
let second := Point("new")
old(first)
second
first
"#,
        "7\nnew\n7\n",
    );
}

#[test]
fn bindings_without_text_stay_silent_and_builtin_types_cannot_be_replaced() {
    let output = session(
        "class Point { public final n : Int\nnew(n : Int) { this.n = n } }\nlet p := Point(3)\np\np.n\nenum Error { Bad }\nclass Int { new() {} }\n4\n",
    );
    assert!(!output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "3\n4\n");
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(errors.contains("E_SESSION_DISPLAY"), "{errors}");
    assert!(errors.contains("E_ERROR_ROOT_REDEFINITION"), "{errors}");
    assert!(errors.contains("E_DUPLICATE_NAME"), "{errors}");
}

#[test]
fn constants_and_destructured_names_are_kept_for_later_functions() {
    shows(
        "const MAX : Int = 3\nfunction before() : Int = MAX\nconst MAX : Int = MAX + 1\nMAX\nbefore()\nlet (a, b) := (1, 2)\nfunction sum() : Int = a + b\nsum()\nlet [..tail] := [3, 4]\nfunction values() : List[Int] = tail\nvalues()\n",
        "4\n3\n3\n[3, 4]\n",
    );
}

#[test]
fn runtime_failures_end_only_their_entry_without_replaying_earlier_code() {
    // A runtime stop ends its entry and the session goes on (REVIEW 1.7); the final
    // exit still reports the failure.
    let output = session("1\n9223372036854775807 + 1\nprint(\"after\")\n");
    assert!(!output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "1\nafter\n");
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Int addition overflow")
    );
}

#[test]
fn scoped_workers_update_persistent_atomics_between_entries() {
    shows(
        "let total := Atomic(0)\nfunction add(total : Atomic[Int]) : void or Error with Effects { let previous := total.fetchAdd(1)? }\nlet result := scope { spawn add(total)\nspawn add(total) }\ntotal.load()\nlet result := scope { spawn add(total) }\ntotal.load()\n",
        "2\n3\n",
    );
}

#[test]
fn boxes_display_through_their_values_and_runtime_errors_keep_the_typed_expression() {
    shows(
        "box(3)\nclass Label { has Text\nnew() {}\npublic function text() : String = \"label\" }\nbox(Label())\n",
        "3\nlabel\n",
    );
    let output = session("let n := 9223372036854775807\nn + 1\n");
    assert!(!output.status.success());
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(
        errors.contains("<session>:2:1: runtime error: Int addition overflow"),
        "{errors}"
    );
    assert!(errors.contains("n + 1"), "{errors}");
    assert!(!errors.contains("session__"), "{errors}");
}

// The test stops a hung session through its Unix process group.
#[cfg(unix)]
#[test]
fn python_background_callbacks_keep_stable_globals_and_dispatch_between_entries() {
    if !python_ready() {
        return;
    }
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    std::fs::write(
        dir.path().join("driver.py"),
        r#"import threading
stop = threading.Event()
ready = threading.Event()
errors = []
calls = 0
def start(function):
    global thread
    def work():
        global calls
        ready.set()
        try:
            while not stop.is_set():
                try:
                    value = function()
                except RuntimeError as error:
                    if "Python bridge is busy on another thread during a callback" in str(error):
                        continue
                    raise
                if value != "label!":
                    errors.append("changed value")
                calls += 1
                ready.set()
        except BaseException as error:
            errors.append(str(error))
            ready.set()
    thread = threading.Thread(target=work, daemon=True)
    thread.start()
    if not ready.wait(5):
        raise RuntimeError("callback did not start")
def request_stop():
    stop.set()
def finish():
    return not thread.is_alive() and not errors and calls > 0
"#,
    )
    .unwrap();
    let mut source = String::from(
        r#"class Label { has Text
public final value : String
new(value : String) { this.value = value }
public function text() : String = this.value }
let label : Box[dyn Text] = box(Label("label" + "!"))
import python "builtins" as builtins
function read() : String or Error with Effects = builtins.str(label.text())
import python "driver" as driver
function start() : void or Error with Effects { let done : void = driver.start(read)? }
let done := start()
"#,
    );
    for index in 0..8 {
        source.push_str(&format!("class Extra{index} {{ has Text\nnew() {{}}\npublic function text() : String = \"extra\" }}\nlet item{index} : Box[dyn Text] = box(Extra{index}())\n"));
    }
    source.push_str("function requestStop() : void or Error with Effects { let done : void = driver.request_stop()? }\nlet done := requestStop()\nfunction finish() : Bool or Error with Effects = driver.finish()\nmatch finish() { ok => ok.text(), _ => \"error\", }\n");
    use std::os::unix::process::CommandExt;
    let mut process = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .process_group(0)
        .args(["session", "--root"])
        .arg(dir.path())
        .env("PYTHONPATH", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    while process.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            unsafe extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            // This subprocess and its native worker have their own process group.
            unsafe {
                kill(-(process.id() as i32), 9);
            }
            let output = process.wait_with_output().unwrap();
            panic!(
                "callback process timed out: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let output = process.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "true\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn python_finalizers_cannot_call_into_released_session_values() {
    if !python_ready() {
        return;
    }
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let marker = dir.path().join("closed.txt");
    std::fs::write(
        dir.path().join("driver.py"),
        r#"import os
function = None
class Watcher:
    def __del__(self):
        result = "ran"
        try:
            function()
        except RuntimeError:
            result = "closed"
        with open(os.environ["KESTREL_CALLBACK_MARKER"], "w") as output:
            output.write(result)
def watch():
    return Watcher()
def arm(f):
    global function
    function = f
"#,
    )
    .unwrap();
    let source = r#"import python "driver" as driver
let watcher := driver.watch()
class Label { has Text
new() {}
public function text() : String = "label" }
let label : Box[dyn Text] = box(Label())
function read() : String = label.text()
function arm() : void or Error with Effects { let done : void = driver.arm(read)? }
let done := arm()
1
"#;
    let mut process = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["session", "--root"])
        .arg(dir.path())
        .env("PYTHONPATH", dir.path())
        .env("KESTREL_CALLBACK_MARKER", &marker)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = process.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "1\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "closed");
}

#[test]
fn owned_arrays_and_dynamic_cursors_work_across_later_session_definitions() {
    shows(
        r#"class Counter {has Iterator
public var n : Int
new(n : Int) {this.n=n}
public mut function next() : Option[Int] = if this.n == 0 then None else {this.n=this.n-1
Some(this.n)} }
class Range {has Iterable
public final n : Int
new(n : Int) {this.n=n}
public function items() : Counter = Counter(this.n) }
function total(mut values : &dyn Iterable(Item = Int)) : Int with Effects {let mut sum := 0
for (value in values) {sum=sum+value}
sum}
let first := Range(4)
total(first)
let saved := total
let values := Array.generate(3, i => Range(i))
values[2].n
class Later {has Iterable
new() {}
public function items() : Counter = Counter(5) }
let later := Later()
saved(later)
let numbers := [7, 8]
total(numbers)
let mut cursor := numbers.items()
match cursor.next() {Some(value) => value, None => -1,}
values[1] = Range(9)
values[1].n
"#,
        "6\n2\n10\n15\n7\n9\n",
    );
}

#[test]
fn python_callback_errors_show_original_session_type_names() {
    if !python_ready() {
        return;
    }
    shows(
        r#"import python "builtins" as py
enum ParseError extends Error {Bad(String)}
function reject(n : Int) : Int or Error = ParseError.Bad("bad")
function invoke() : List[Int] or Error with Effects = py.list(py.map(reject,[1])?)
match invoke() {RuntimeError.Python(message) => message, _ => "wrong",}
"#,
        "RuntimeError: ParseError.Bad(\"bad\")\n",
    );
}

#[test]
fn arena_handles_keep_their_identity_across_session_libraries() {
    shows(
        "import std.memory.{Arena, ArenaRef}\nlet arena := Arena[String]()\nlet first := arena.insert(\"a\" + \"b\")\nmatch arena.get(first) {Some(s) => s, None => \"missing\",}\nlet saved := first\nlet arena := Arena[String]()\nlet second := arena.insert(\"c\" + \"d\")\nfirst == saved\nfirst == second\nmatch arena.get(first) {Some(s) => s, None => \"expired\",}\nmatch arena.get(second) {Some(s) => s, None => \"missing\",}\n",
        "ab\ntrue\nfalse\nexpired\ncd\n",
    );
}

#[test]
fn exponent_literals_keep_their_type_in_sessions() {
    shows(
        "let scale := 1e3\nscale\nscale + 2E2\nf\"{2.5e-4:.4f}\"\n",
        "1000.0\n1200.0\n0.0003\n",
    );
}

#[test]
fn associated_types_preserve_slots_and_redefinitions_in_sessions() {
    shows(
        "class Item {new() {}}\ntrait Source {type Item\nfunction next() : Option[Item]}\nclass Item {new() {}}\nclass Counter {has Source\nnew() {}\npublic function next() : Option[Int] = Some(7)}\nfunction read[T : Source(Item = Int)](source : T) : Int = match source.next() {Some(n) => n, None => 0,}\nread(Counter())\n",
        "7\n",
    );
}

#[test]
fn namespace_imports_keep_aliases_types_and_saved_function_values_in_sessions() {
    shows(
        "import std.math\nmath.sqrt(9.0)\nlet root := math.sqrt\nimport std.str as math\nmath.length(\"λx\")\nroot(16.0)\nimport std.memory as mem\nlet arena := mem.Arena[Int]()\nlet key : mem.ArenaRef[Int] = arena.insert(7)\nmatch arena.get(key) {Some(n) => n, None => 0,}\n",
        "3.0\n2\n4.0\n7\n",
    );
}

#[test]
fn python_worker_reentry_is_a_typed_failure_in_session_libraries() {
    if !python_ready() {
        return;
    }
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    std::fs::write(
        dir.path().join("driver.py"),
        r#"import threading
def invoke(callback,value):
    done=threading.Event()
    results,errors=[],[]
    def work():
        try:
            results.append(callback(value))
        except BaseException as error:
            errors.append(error)
        finally:
            done.set()
    worker=threading.Thread(target=work,daemon=True)
    worker.start()
    if not done.wait(3):
        raise RuntimeError("callback timed out")
    worker.join()
    if errors:
        raise errors[0]
    return results[0]
"#,
    )
    .unwrap();
    let source = r#"import std.python.{PythonValue}
import std.str.{contains}
import python "builtins" as py
import python "driver" as driver
function label(value : PythonValue) : String or Error with Effects = py.str(value)
function inner(value : PythonValue) : void or Error with Effects {let text : String = py.str(value)?}
function spawned(value : PythonValue) : String or Error with Effects {scope {spawn inner(value)}?
"unexpected"}
function classify(value : String or Error) : String = match value {RuntimeError.Python(message) => if contains(message,"busy") then "busy" else message,text => text,_ => "other",}
function checkDirect() : void with Effects {let result : String or Error = driver.invoke(label,[1,2,3])
print(classify(result))}
checkDirect()
function checkSpawned() : void with Effects {let result : String or Error = driver.invoke(spawned,[4,5,6])
print(classify(result))}
checkSpawned()
"#;
    let mut process = Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .args(["session", "--root"])
        .arg(dir.path())
        .env("PYTHONPATH", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = process.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"busy\nbusy\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn unicode_names_keep_versions_and_distinct_spellings_in_sessions() {
    shows(
        include_str!("../testdata/language/identifiers.kes"),
        "6\n4\n11\n",
    );
    shows(
        "let café := 3\nlet café := 4\nfunction sommé(n : Int = café) : Int = n\nlet café := 9\nsommé()\ncafé\ncafé\nclass ǅata[T] {public final n : T\nnew(move n : T) {this.n=n}}\nlet 数 := ǅata(7)\n数.n\n",
        "3\n9\n4\n7\n",
    );
}

#[test]
fn incomplete_operators_and_delimiters_continue_one_session_entry() {
    shows(
        "let total :=\n1 + // continuation\n2\ntotal\nlet xs := [\n3,\n4\n]\nxs\n",
        "3\n[3, 4]\n",
    );
}

#[test]
fn default_arguments_in_import_cycles_work_in_session_versions() {
    shows(
        include_str!("../testdata/language/cycledefaults.kes"),
        "42\n42\ncyclic default\ncyclic default\ncyclic default\n55\ntrue\n3\n",
    );
}

#[test]
fn runtime_contracts_in_import_cycles_keep_guards_in_session_calls() {
    shows(
        include_str!("../testdata/language/cyclecontracts.kes"),
        "true\ntrue\ntrue\ntrue\ntrue\n",
    );
}

#[test]
fn session_test_commands_filter_saved_tests_and_preserve_values_and_input() {
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    std::fs::write(dir.path().join("kestrel.project"), "package live\n").unwrap();
    std::fs::create_dir(dir.path().join("tests")).unwrap();
    std::fs::write(
        dir.path().join("tests/values.kes"),
        "import std.test.{assertEq}\n@test function first() : void {assertEq(1,1)}\n@test function reads() : void with Effects {assertEq(match readLine() {RuntimeError.EndOfFile(_) => true, _ => false,}, true)}",
    ).unwrap();
    let output = session_at(
        "let total := 3\nfunction value() : Int = total\n:test first\nvalue()\n:test\nvalue()\n:help\n",
        dir.path(),
    );
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("running 1 test from 1 file\n"), "{text}");
    assert!(
        text.contains("1 passed, 0 failed\n3\nrunning 2 tests from 1 file\n"),
        "{text}"
    );
    assert!(text.contains("2 passed, 0 failed\n3\n"), "{text}");
    assert!(text.contains(":test [FILTER]"), "{text}");
}

#[test]
fn failed_session_tests_and_missing_packages_leave_the_session_usable() {
    let dir = kestrel::native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let output = session_at("let total := 7\n:test\ntotal\n", dir.path());
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"7\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("has no kestrel.project"));
    std::fs::write(dir.path().join("kestrel.project"), "package live\n").unwrap();
    std::fs::create_dir(dir.path().join("tests")).unwrap();
    std::fs::write(dir.path().join("tests/values.kes"), "import std.test.{assertEq}\n@test function broken() : void {assertEq(1,2)}\n@test function fine() : void {assertEq(1,1)}").unwrap();
    let output = session_at(
        "let total := 9\n:test broken\ntotal\n:test fine\ntotal + 1\n",
        dir.path(),
    );
    assert!(!output.status.success());
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("FAIL  tests/values.kes  broken"), "{text}");
    assert!(
        text.contains("0 passed, 1 failed\n9\nrunning 1 test"),
        "{text}"
    );
    assert!(text.ends_with("1 passed, 0 failed\n10\n"), "{text}");
}

#[test]
fn dependent_defaults_keep_parameter_bindings_in_sessions() {
    shows(
        "let height:=4\nfunction area(width : Int=height,height : Int=2) : Int=width*height\nlet height:=9\narea()\n",
        "8\n",
    );
    shows(
        include_str!("../testdata/language/dependentdefaults.kes"),
        "9\nab:ab\n4\n5\n10\n16\nglobal default\nglobal default\n",
    );
    shows(
        "let width:=7\nfunction area(width : Int,height : Int=width) : Int = width * height\nlet saved:=area(_)\nlet width:=9\nsaved(3)\nfunction area(width : Int,height : Int=width+1) : Int=width*height\narea(3)\nsaved(3)\n",
        "9\n12\n9\n",
    );
}

#[test]
fn consuming_partial_captures_fail_before_moving_session_values() {
    let output = session(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction take(move p : Point,n : Int) : String=p.label\nlet p:=Point(\"kept\"+\" value\")\nlet f:=take(p,_)\np.label\nlet safe:=take(_,_)\nsafe(Point(\"fresh\"),1)\n",
    );
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"kept value\nfresh\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("E_PARTIAL_MOVE_CAPTURE"));
}

#[test]
fn callback_helper_escapes_fail_before_execution_and_owned_callbacks_persist() {
    let output = session(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction keep(f : Int -> String) : Int -> String=f\nfunction bad() : Int -> String {let p:=Point(\"released\")\nkeep((n : Int) => p.label)}\nfunction good() : Int -> String {let p:=Point(\"kept\"+\" label\")\nkeep(move (n : Int) => p.label)}\nlet f:=good()\nf(1)\nf(2)\n",
    );
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"kept label\nkept label\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("E_LAMBDA_ESCAPE"));
}

#[test]
fn saved_named_callbacks_keep_script_owners_after_rejected_moves() {
    let output = session(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nlet p:=Point(\"kept\"+\" label\")\nfunction read(n : Int) : String=p.label\nfunction keep(f : Int -> String) : Int -> String=f\nlet f:=keep(read)\nfunction consume(move p : Point) : void {}\n{consume(p)\nf(1)}\nf(1)\np.label\n",
    );
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"kept label\nkept label\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("E_BORROW_CONFLICT"));
}

#[test]
fn temporary_callback_owners_survive_later_session_entries() {
    shows(
        "class Point {public final label : String\nnew(label : String) {this.label=label}\npublic function read(n : Int) : String=this.label}\nfunction keep(f : Int -> String) : Int -> String=f\nlet f:={keep(Point(\"temporary\"+\" receiver\").read)}\nf(1)\nlet other:=Point(\"other\")\nf(2)\nfunction keep(f : Int -> String) : Int -> String=move (n : Int) => \"new\"\nf(3)\n",
        "temporary receiver\ntemporary receiver\ntemporary receiver\n",
    );
}

#[test]
fn dynamic_script_mutation_requires_explicit_parameters_between_entries() {
    let output = session(
        "trait Mutable {mut function bump() : void\nfunction current() : Int}\nclass Tally {has Mutable\nvar n : Int\nnew() {this.n=0}\npublic mut function bump() : void {this.n=this.n+1}\npublic function current() : Int=this.n}\nlet value : Box[dyn Mutable]=box(Tally())\nfunction hidden() : void=value.bump()\nfunction change(mut value : Box[dyn Mutable]) : void=value.bump()\nchange(value)\nvalue.current()\nchange(value)\nvalue.current()\n",
    );
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"1\n2\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("E_SCRIPT_MUTABLE_CAPTURE"));
}

#[test]
fn rejected_callback_annotation_does_not_consume_its_capture() {
    let output = session(
        "class Tally {var n : Int\nnew() {this.n=0}\npublic mut function bump() : Int {this.n=this.n+1\nthis.n}}\nlet tally:=Tally()\nlet hidden : () -> Int=move () => tally.bump()\ntally.bump()\nlet bump:=move () => tally.bump()\nfunction run(mut f : () -> Int) : Int=f()\nrun(bump)\nrun(bump)\nrun(() => 7)\n",
    );
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"1\n2\n3\n7\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("E_CAPTURE_TYPE"));
}

#[test]
fn rejected_capture_returns_leave_the_session_owner_live() {
    let output = session(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nlet p:=Point(\"kept\"+\" label\")\nlet bad:=move () => p\np.label\nlet factory:=move () => Point(p.label)\nfactory().label\nfactory().label\n",
    );
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"kept label\nkept label\nkept label\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("E_LAMBDA_ESCAPE"));
}

#[test]
fn global_view_dependencies_and_nested_capture_failures_leave_owners_live() {
    let output = session(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nclass Panel {public final point : &Point\nnew(point : Point) {this.point=point}}\nlet p:=Point(\"kept\"+\" owner\")\nlet bad:=move () => move () => p.label\np.label\nlet panel:=Panel(p)\nfunction point() : Point=panel.point\nlet view:=point()\nfunction consume(move p : Point) : void {}\n{consume(p)\nview.label}\nview.label\n",
    );
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"kept owner\nkept owner\n");
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(errors.contains("E_MOVE_BORROWED"), "{errors}");
    assert!(errors.contains("E_BORROW_CONFLICT"), "{errors}");
}

#[test]
fn explicit_trait_views_keep_their_source_and_temporary_storage_between_entries() {
    shows(
        "class Point {has Text\npublic final label : String\nnew(label : String) {this.label=label}\npublic function text() : String=this.label}\nlet p:=Point(\"object\"+\" owner\")\nlet view : &dyn Text=p\np.label\nview.text()\nfunction identity(value : &dyn Text) : &dyn Text=value\nlet scalar:=identity(\"temporary\"+\" scalar\")\nscalar.text()\nlet temporary : &dyn Text={Point(\"block\"+\" owner\")}\ntemporary.text()\nscalar.text()\n",
        "object owner\nobject owner\ntemporary scalar\nblock owner\ntemporary scalar\n",
    );
}

#[test]
fn receiver_defaults_and_bound_methods_keep_their_session_context() {
    shows(
        "class Rectangle {public var width : Int\nnew(width : Int) {this.width=width}\npublic function area(height : Int = this.width) : Int=this.width*height\npublic function offset(n : Int,extra : Int = this.width) : Int=n+extra}\nlet rectangle:=Rectangle(3)\nlet area:=rectangle.area\nlet offset:=rectangle.offset(_)\narea()\noffset(4)\nfunction apply(f : Int -> Int,n : Int) : Int=f(n)\napply(offset,5)\nlet rectangle:=Rectangle(9)\nrectangle.area()\noffset(4)\narea()\n",
        "9\n7\n8\n81\n7\n9\n",
    );
}

#[test]
fn abstract_trait_defaults_keep_checked_receiver_types_in_sessions() {
    shows(
        "trait Area {function width() : Int\nfunction area(height : Int = this.width()) : Int}\nclass Square {has Area\npublic final side : Int\nnew(side : Int) {this.side=side}\npublic function width() : Int=this.side\npublic function area(height : Int) : Int=this.side*height}\nlet square : Box[dyn Area]=box(Square(3))\nlet area:=square.area\narea()\nlet square : Box[dyn Area]=box(Square(5))\nsquare.area()\narea()\n",
        "9\n25\n9\n",
    );
}

#[test]
fn returned_exact_wrappers_keep_their_storage_and_script_borrows_between_entries() {
    shows(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nclass Panel {public final point : &Point\nnew(point : Point) {this.point=point}}\nfunction identity(value : Panel) : Panel=value\nfunction wrap(point : Point) : Panel {let local:=Panel(point)\nidentity(local)}\nlet point:=Point(\"kept\"+\" owner\")\nlet panel:=wrap(point)\npanel.point.label\nfunction identity(value : Panel) : Panel=Panel(value.point)\npanel.point.label\nlet other:=wrap(point)\nother.point.label\n",
        "kept owner\nkept owner\nkept owner\n",
    );
}

#[test]
fn rejected_early_this_exposure_does_not_run_constructor_effects() {
    let output = session(
        "class Point {public final label : String\nnew() with Effects {print(this.read())\nthis.label=\"bad\"}\npublic function read() : String=this.label}\nclass Point {public final label : String\nnew() {this.label=\"ready\"}\npublic function read() : String=this.label}\nlet point:=Point()\npoint.read()\n",
    );
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"ready\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("E_THIS_UNINITIALIZED"));
}

#[test]
fn bigint_values_and_operations_survive_session_libraries_and_redefinitions() {
    shows(
        "let n : BigInt=1000000000000000000000\nn\nfunction increment(value : BigInt) : BigInt=value+1\nlet next:=increment(n)\nnext\nlet text:=n.text\ntext()\nfunction increment(value : BigInt) : BigInt=value+2\nincrement(n)\nnext\nlet factory:=move () => next+3\nfactory()\nfactory()\nmatch parseBigInt(\"-123456789012345678901\") {v=>v.text(),_=>\"error\"}\n",
        "1000000000000000000000\n1000000000000000000001\n1000000000000000000000\n1000000000000000000002\n1000000000000000000001\n1000000000000000000004\n1000000000000000000004\n-123456789012345678901\n",
    );
}

#[test]
fn static_final_fields_stay_lazy_and_versioned_in_sessions() {
    shows(
        "class C { public static final text : String = \"a\" + \"b\"\nnew() {} }\nfunction first() : String = C.text\nC.text\nclass C { public static final text : String = \"c\" + \"d\"\nnew() {} }\nC.text\nfirst()\n",
        "ab\ncd\nab\n",
    );
}

#[test]
fn static_vars_persist_across_session_entries_and_class_versions() {
    shows(
        "class C { public static var x : Int = 1\nnew() {} }\nC.x\nC.x = 2\nC.x\nfunction read() : Int with Effects = C.x\nread()\nclass C { public static var x : Int = 10\nnew() {} }\nC.x\nread()\n",
        "1\n2\n2\n10\n2\n",
    );
}

#[test]
fn tensors_keep_their_storage_and_ownership_across_session_entries() {
    shows(
        "let a := Tensor.generate((2, 3), (i, j) => (i * 3 + j).toDouble())\nlet b := a.copy()\na[1, 2] = 9.0\nprint(b[1, 2])\nprint(a[1, 2])\nlet c := a[0:1, :]\nc[0, 0] = 7.0\nprint(a[0, 0])\nprint(c[0, 0])\n",
        "5.0\n9.0\n0.0\n7.0\n",
    );
}

#[test]
fn tensor_phase_two_results_survive_session_entries() {
    shows(
        "let t := Tensor.generate((2, 3), (i, j) => (i * 3 + j + 1).toDouble())\nt.sum()\nlet rows := t.sum(axis: 1)\nrows\nlet n := 3\nlet dynamic := Tensor.ones((n,))\nlet scalar := dynamic.sum(axis: 0)\nscalar\nlet reshaped := t.map(x => x * 0.5).reshape((3, 2))\nprint(reshaped.shape())\nprint(reshaped[2, 1])\n",
        "21.0\nTensor[double, (2,)] [6.0, 15.0]\nTensor[double, ()] 3.0\n[3, 2]\n3.0\n",
    );
}

#[test]
fn uppercase_values_constructors_and_array_shapes_survive_session_entries() {
    shows(
        "let X_c := Tensor([1.0, 2.0], shape = (1, 2))\nprint(X_c[0, 1])\nlet Shape := X_c.shape()\nprint(Tensor.ones(Shape))\nlet X_c := Tensor([3.0, 4.0])\nprint(X_c[1])\nfunction head(X : Tensor[double, (2,)]) : double = X[0]\nprint(head(X_c))\n",
        "2.0\n[[1.0, 1.0]]\n4.0\n3.0\n",
    );
}

#[test]
fn native_numerical_modules_and_rng_state_work_across_session_entries() {
    shows(
        "import std.linalg\nimport std.complex.{Complex}\nimport std.random.{Rng}\nlet A := Tensor([2.0, 0.0, 0.0, 4.0], shape = (2, 2))\nlet factor := linalg.lu(A)\nprint(factor.solve(Tensor([2.0, 8.0])))\nlet z := Complex(1.0, 2.0)\nprint(z * z)\nlet rng := Rng.seeded(42)\nlet same := Rng.seeded(42)\nprint(rng.integers(0, 100) == same.integers(0, 100))\nprint(rng.normal(0.0, 1.0) == same.normal(0.0, 1.0))\nprint(linalg.inverse(A))\n",
        "[1.0, 2.0]\n-3.0+4.0i\ntrue\ntrue\n[[0.5, 0.0],\n [0.0, 0.25]]\n",
    );
}

#[test]
fn machine_learning_models_survive_session_entries() {
    shows(
        "import std.ml.{StandardScaler, LinearRegression, meanSquaredError}\nlet X := Tensor([0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0], shape = (4, 2))\nlet y := Tensor([1.0, 2.0, 3.0, 4.0], shape = (4,))\nlet scaler := StandardScaler.fit(X)\nprint(scaler.means)\nlet model := LinearRegression.fit(X, y)\nprint(model.coefficients)\nprint(model.predict(X))\nprint(meanSquaredError(y, model.predict(X)) < 1e-20)\n",
        "[0.5, 0.5]\n[1.0, 2.0]\n[1.0, 2.0, 3.0, 4.0]\ntrue\n",
    );
}

#[test]
fn records_and_enums_display_with_the_names_the_user_wrote() {
    // A session renames each entry's declarations internally (REVIEW2 1.8).
    shows(
        "record P(x : Int, y : Int)\nP(1, 2)\nenum C { R, G }\n[R, G]\nrecord P(x : Int)\nP(5)\n",
        "P(x = 1, y = 2)\n[R, G]\nP(x = 5)\n",
    );
}
