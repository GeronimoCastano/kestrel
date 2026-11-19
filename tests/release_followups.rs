//! Regressions for the bugs and features that followed audit/RELEASE-REVIEW-2026-11-15.md:
//! library validation, parent construction, copyable immutable enums, comparison
//! operators, exported extensions, text and binary data, error context, and the
//! ownership and tail-call holes found while checking them.
use kestrel::native::TemporaryDirectory;
use std::process::{Command, Output};

fn run(source: &str, files: &[(&str, &str)], command: &str) -> Output {
    let directory = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    std::fs::write(directory.path().join("main.kes"), source).unwrap();
    for (name, text) in files {
        std::fs::write(directory.path().join(name), text).unwrap();
    }
    Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .arg(command)
        .arg("main.kes")
        .current_dir(directory.path())
        .output()
        .unwrap()
}

fn stdout(source: &str, files: &[(&str, &str)]) -> String {
    let output = run(source, files, "run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn refused(source: &str, code: &str) {
    let output = run(source, &[], "check");
    let text = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(text.contains(code), "expected {code}:\n{source}\n{text}");
}

#[test]
fn decoders_return_errors_for_malformed_bytes() {
    assert_eq!(
        stdout(
            r####"import std.encoding
for (s in ["c080", "e08080", "eda080", "f4908080", "ff808080", "c3a9"]) {
 print(s + " => " + (match encoding.hexDecode(s) { RuntimeError.Message(m) => "ERROR " + m, v => f"{v:?}", _ => "ERROR" }))
}
for (s in ["====", "A===", "YQ==Yg==", "YQ==", "wIA=", "w6k="]) {
 print(s + " => " + (match encoding.base64Decode(s) { RuntimeError.Message(m) => "ERROR " + m, v => f"{v:?}", _ => "ERROR" }))
}
"####,
            &[]
        ),
        r####"c080 => ERROR The decoded bytes are not valid UTF-8
e08080 => ERROR The decoded bytes are not valid UTF-8
eda080 => ERROR The decoded bytes are not valid UTF-8
f4908080 => ERROR The decoded bytes are not valid UTF-8
ff808080 => ERROR The decoded bytes are not valid UTF-8
c3a9 => "é"
==== => ERROR Base64 padding is only allowed at the end
A=== => ERROR Base64 padding is only allowed at the end
YQ==Yg== => ERROR Base64 padding is only allowed at the end
YQ== => "a"
wIA= => ERROR The decoded bytes are not valid UTF-8
w6k= => "é"
"####
    );
}

#[test]
fn json_rejects_malformed_numbers() {
    assert_eq!(
        stdout(
            r####"import std.json
for (s in ["01", "-01", "1.", "1.e2", "-.5", "1e999", "9007199254740993", "1e-9999", "0", "1.25", "-2e3"]) {
 print(s + " => " + (match json.parse(s) { RuntimeError.Message(m) => "ERROR " + m, v => json.stringify(v), _ => "ERROR" }))
}
"####,
            &[]
        ),
        r####"01 => ERROR JSON: a number cannot have a leading zero at byte 1.
-01 => ERROR JSON: a number cannot have a leading zero at byte 2.
1. => ERROR JSON: expected digits after the decimal point at byte 2.
1.e2 => ERROR JSON: expected digits after the decimal point at byte 2.
-.5 => ERROR JSON: expected a digit at byte 1.
1e999 => ERROR JSON: bad number 1e999 at byte 5.
9007199254740993 => 9007199254740992.0
1e-9999 => ERROR JSON: bad number 1e-9999 at byte 7.
0 => 0
1.25 => 1.25
-2e3 => -2000
"####
    );
}

#[test]
fn toml_rejects_what_toml_1_0_rejects() {
    assert_eq!(
        stdout(
            r####"import std.toml
print("0" + " => " + (match toml.parse("a = 1e_2") { RuntimeError.Message(m) => "ERROR " + m, v => "ACCEPT", _ => "ERROR" }))
print("1" + " => " + (match toml.parse("a = 1_e2") { RuntimeError.Message(m) => "ERROR " + m, v => "ACCEPT", _ => "ERROR" }))
print("2" + " => " + (match toml.parse("a = 0_1.0") { RuntimeError.Message(m) => "ERROR " + m, v => "ACCEPT", _ => "ERROR" }))
print("3" + " => " + (match toml.parse("a = .5") { RuntimeError.Message(m) => "ERROR " + m, v => "ACCEPT", _ => "ERROR" }))
print("4" + " => " + (match toml.parse("a = 1.") { RuntimeError.Message(m) => "ERROR " + m, v => "ACCEPT", _ => "ERROR" }))
print("5" + " => " + (match toml.parse("a = 2026-99-99") { RuntimeError.Message(m) => "ERROR " + m, v => "ACCEPT", _ => "ERROR" }))
print("6" + " => " + (match toml.parse("a = 25:99:99") { RuntimeError.Message(m) => "ERROR " + m, v => "ACCEPT", _ => "ERROR" }))
print("7" + " => " + (match toml.parse("a = { x = 1, }") { RuntimeError.Message(m) => "ERROR " + m, v => "ACCEPT", _ => "ERROR" }))
print("8" + " => " + (match toml.parse("[x]\na=1\n[x]\nb=2") { RuntimeError.Message(m) => "ERROR " + m, v => "ACCEPT", _ => "ERROR" }))
print("9" + " => " + (match toml.parse("a = \"x\u0001y\"") { RuntimeError.Message(m) => "ERROR " + m, v => "ACCEPT", _ => "ERROR" }))
print("10" + " => " + (match toml.parse("a = \"x\\ey\"") { RuntimeError.Message(m) => "ERROR " + m, v => "ACCEPT", _ => "ERROR" }))
"####,
            &[]
        ),
        r####"0 => ERROR TOML line 1: `1e_2` is not a TOML number.
1 => ERROR TOML line 1: `1_e2` is not a TOML number.
2 => ERROR TOML line 1: `0_1.0` has a leading zero.
3 => ERROR TOML line 1: `.5` is not a TOML number.
4 => ERROR TOML line 1: `1.` is not a TOML number.
5 => ERROR TOML line 1: invalid date or time.
6 => ERROR TOML line 1: invalid date or time.
7 => ERROR TOML line 1: expected a key.
8 => ERROR TOML line 3: a table is declared twice.
9 => ERROR TOML contains an invalid control character.
10 => ERROR TOML line 1: unknown escape.
"####
    );
}

#[test]
fn dates_reject_impossible_offsets_and_seconds() {
    assert_eq!(
        stdout(
            r####"import std.datetime
for (s in ["2026-01-01T00:00:00+99:99", "2026-01-01T00:00:00+02:60", "2026-01-01T00:00:60Z", "2026-01-01T00:00:00+02:00"]) {
 print(s + " => " + (match datetime.parse(s) { RuntimeError.Message(m) => "ERROR " + m, t => datetime.format(t), _ => "ERROR" }))
}
"####,
            &[]
        ),
        r####"2026-01-01T00:00:00+99:99 => ERROR Invalid UTC offset in "2026-01-01T00:00:00+99:99".
2026-01-01T00:00:00+02:60 => ERROR Invalid UTC offset in "2026-01-01T00:00:00+02:60".
2026-01-01T00:00:60Z => ERROR "2026-01-01T00:00:60Z" has no valid HH:MM:SS time.
2026-01-01T00:00:00+02:00 => 2025-12-31T22:00:00Z
"####
    );
}

#[test]
fn file_errors_stay_in_the_result() {
    assert_eq!(
        stdout(
            r####"import std.fs
let path := "not-a-directory"
fs.writeFile(path, "data")?
print(match fs.createDirs(path) { RuntimeError.Io(m) => "ERROR " + m, _ => "SUCCESS" })
print(fs.isDir(path))
print(match fs.readFile("missing\u0000suffix") { RuntimeError.Io(m) => "ERROR " + m, _ => "OTHER" })
"####,
            &[]
        ),
        r####"ERROR Cannot create directory: a path component is not a directory or is inaccessible
false
ERROR A file path contains a NUL character
"####
    );
}

#[test]
fn format_specs_match_python() {
    assert_eq!(
        stdout(
            r####"let x := 1234.5678
print(f"{x:.2}")
print(f"{x:#.0f}")
print(f"{x:010,.2f}")
print(f"{12.0:#g}")
print(f"{42: }")
print(f"{42:+}")
print(f"{42: =8}")
let n : Nat = 18446744073709551615
print(f"{n:d}")
print(f"{n:x}")
print("[" + f"{42: }" + "]")
print("[" + f"{-3: }" + "]")
print("[" + f"{2.5: .1f}" + "]")
print("[" + "{: }".format(7) + "]")
"####,
            &[]
        ),
        r####"1.2e+03
1235.
001,234.57
12.0000
 42
+42
      42
18446744073709551615
ffffffffffffffff
[ 42]
[-3]
[ 2.5]
[ 7]
"####
    );
}

#[test]
fn parents_construct_records_reuse_compare_extend_text_bytes_and_errors() {
    assert_eq!(
        stdout(
            r####"import helpers
import std.fs
import std.test

// Parent construction
class Shape {
    public final name : String
    new(name : String) { this.name = name }
    public function describe() : String = "shape " + this.name
}
class Square extends Shape {
    public final side : Int
    new(side : Int) {
        super("square")
        this.side = side
    }
    @override public function describe() : String = super.describe() + f" of side {this.side}"
}
class Cube extends Square {
    new(side : Int) { super(side) }
    @override public function describe() : String = "cube, " + super.describe()
}
print(Square(3).describe())
print(Cube(2).describe())
print(Cube(2).name)

// Recursive enums are reused
enum Expr { Lit(Int), Add(Expr, Expr), Mul(Expr, Expr) }
function eval(e : Expr) : Int = match e {
    Lit(n) => n,
    Add(a, b) => eval(a) + eval(b),
    Mul(a, b) => eval(a) * eval(b),
}
let two := Lit(2)
let sum := Add(two, two)
let square := Mul(sum, sum)
print([two, two])
print(eval(square))
print(eval(Add(square, square)))

// Comparison syntax
record P(name : String, age : Int)
enum Level { Low, Mid, High }
print("apple" < "banana")
print("b" >= "a")
print(P("a", 2) < P("a", 3))
print((1, "z") > (1, "a"))
print(Low < High)
print([3, 1, 2].sorted() == [1, 2, 3])

// Exported extensions
print("hello".shout())
print([4, 5, 6].second())

// Text and binary data
let word := "é👩‍👩‍👧‍👦á🇦🇷"
print(word.chars().toList())
print(word.graphemes().toList())
print(word.graphemeLength())
print(word.length())
let bytes := Bytes([0, 255, 195, 169])?
print(bytes.toList())
print(bytes.toHex())
print(bytes.slice(2, 4).decodeUtf8()?)
print(bytes.get(9))
print("hé".bytes().toList())
fs.writeBytes("raw.bin", bytes)?
print(fs.readBytes("raw.bin")?.toList())
fs.remove("raw.bin")?
test.assertError(bytes.decodeUtf8())

// Errors
function fail() : Int or Error = Error("disk full")
function okay() : Int or Error = 7
match fail().withContext("saving notes") {
    Error(e) => {
        print(e)
        print(e.cause().isSome())
    },
    value => print(value),
}
match fail().mapError((move e : Error) => Error("mapped")) {
    Error(e) => print(e),
    value => print(value),
}
print(okay().mapError((move e : Error) => Error("no"))?)
print(okay().withContext("unused")?)
"####,
            &[(
                "helpers.kes",
                r####"extend String { public function shout(s : String) : String = s.toUpper() + "!" }
extend List { public function second(xs : List[Int]) : Int = xs.drop(1).take(1).sum() }
"####
            )]
        ),
        r####"shape square of side 3
cube, shape square of side 2
square
[Lit(2), Lit(2)]
16
32
true
true
true
true
true
true
HELLO!
5
[é, 👩, ‍, 👩, ‍, 👧, ‍, 👦, a, ́, 🇦, 🇷]
[é, 👩‍👩‍👧‍👦, á, 🇦🇷]
4
12
[0, 255, 195, 169]
00ffc3a9
é
None
[104, 195, 169]
[0, 255, 195, 169]
saving notes: RuntimeError.Message("disk full")
true
RuntimeError.Message("mapped")
7
7
"####
    );
}

#[test]
fn named_error_enums_take_context_and_mapped_errors() {
    assert_eq!(
        stdout(
            r####"class Doc { has Text
    public final title : String
    new(title : String) { this.title = title }
    public function text() : String = "Doc " + this.title }
enum AppError extends Error { NotFound(String), Bad }
function find(k : String) : Int or AppError = if k == "a" then 1 else NotFound(k)
function load(k : String) : Doc or AppError = if k == "a" then Doc("intro") else Bad
match find("z").withContext("loading settings") {
    Error(e) => print(e),
    v => print(v),
}
print(find("a").withContext("unused")?)
match load("z").mapError((move e : Error) => Error("replaced")) {
    Error(e) => print(e),
    d => print(d),
}
match load("a").withContext("unused") {
    Error(e) => print(e),
    d => print(d),
}
match Bad.context("plain") {
    e => print(e),
}
"####,
            &[]
        ),
        r####"loading settings: AppError.NotFound("z")
1
RuntimeError.Message("replaced")
Doc intro
plain: AppError.Bad
"####
    );
}

#[test]
fn immutable_recursive_enums_are_shared_safely() {
    assert_eq!(
        stdout(
            r####"enum Tree { Leaf, Node(String, Tree, Tree) }
enum Json { JNull, JStr(String), JArr(List[Json]) }
record Pair(left : Tree, right : Tree)
function insert(t : Tree, key : String) : Tree = match t {
    Leaf => Node(key, Leaf, Leaf),
    Node(k, l, r) => if key < k then Node(k, insert(l, key), r) else if key > k then Node(k, l, insert(r, key)) else t,
}
function size(t : Tree) : Int = match t { Leaf => 0, Node(_, l, r) => 1 + size(l) + size(r) }
function leftmost(t : Tree) : Tree = match t { Leaf => Leaf, Node(_, Leaf, _) => t, Node(_, l, _) => leftmost(l) }
function keys(t : Tree) : List[String] = match t { Leaf => [], Node(k, l, r) => keys(l) + [k] + keys(r) }
function mirror(t : Tree) : Tree = match t { Leaf => Leaf, Node(k, l, r) => Node(k, mirror(r), mirror(l)) }

let mut t := Leaf
for (w in "the quick brown fox jumps over the lazy dog".split(" ")) { t = insert(t, w) }
let snapshot := t
t = insert(t, "zebra")
print(size(snapshot))
print(size(t))
print(keys(leftmost(t)))
print(keys(mirror(snapshot)).take(3))
let pair := Pair(t, snapshot)
let copies := [pair, pair, Pair(Leaf, t)]
print(copies.map(p => size(p.left) + size(p.right)).toList())
let maybe : Option[Tree] = Some(t)
let again := maybe
print(match again { Some(x) => size(x), None => -1 })
let table := Map("a" => t, "b" => snapshot)
print(match table.get("b") { Some(x) => size(x), None => -1 })
let tup := (t, snapshot, "label")
let (first, second, label) := tup
print(size(first) + size(second))
print(label)
let doc := JArr([JStr("x"), JArr([JNull, JStr("y")])])
let docs := [doc, doc]
print(docs)
let mut chain := Leaf
for (i in range(2000)) { chain = Node(f"{i}", chain, Leaf) }
let kept := chain
chain = Leaf
print(size(kept))
"####,
            &[]
        ),
        r####"8
9
[brown, dog, fox, jumps, lazy, over]
[the, quick, over]
[17, 17, 9]
9
8
17
label
[JArr([JStr(x), JArr([JNull, JStr(y)])]), JArr([JStr(x), JArr([JNull, JStr(y)])])]
2000
"####
    );
}

#[test]
fn recursive_enums_print_inside_lists_and_copies_pass_together() {
    assert_eq!(
        stdout(
            r####"enum Json { JNull, JStr(String), JArr(List[Json]) }
let doc := JArr([JStr("x"), JNull])
print(doc)
print([doc])
print([JNull])
enum T2 { L, N(Int, T2) }
record P(a : T2, b : T2)
let t := N(1, L)
let s := t
print(P(t, s))
print(P(t, t))
function two(move a : T2, move b : T2) : Int = 2
print(two(t, t))
"####,
            &[]
        ),
        r####"JArr([JStr(x), JNull])
[JArr([JStr(x), JNull])]
[JNull]
P(a = N(1, L), b = N(1, L))
P(a = N(1, L), b = N(1, L))
2
"####
    );
}

#[test]
fn local_functions_return_views_of_captures() {
    assert_eq!(
        stdout(
            r####"class C { public var n : Int
 new(n : Int) { this.n = n } }
class H { public var c : C
 new() { this.c = C(7) } }
function test() : Int {
    let a := Array.generate(2, i => C(i + 5))
    let h := H()
    function head() : C = a[0]
    function child() : C = h.c
    let get := head
    let first := head()
    let other := get()
    let kid := child()
    return first.n + other.n + kid.n
}
print(test())
"####,
            &[]
        ),
        r####"17
"####
    );
}

#[test]
fn functions_return_views_of_array_elements() {
    assert_eq!(
        stdout(
            r####"class C { public var n : Int
 new(n : Int) { this.n = n } }
class H { public var c : C
 new() { this.c = C(7) } }
function head(a : Array[C]) : C = a[0]
function child(h : H) : C = h.c
let a := Array.generate(2, i => C(i + 5))
let h := H()
print(head(a).n + child(h).n)
"####,
            &[]
        ),
        r####"12
"####
    );
}

#[test]
fn callbacks_over_frame_locals_are_not_tail_calls() {
    assert_eq!(
        stdout(
            r####"class C { public var n : Int
 new(n : Int) { this.n = n } }
function apply(f : () -> void) : void { f() }
function applyInt(f : () -> Int) : Int = f()
function test() : Int {
    let a := Array.generate(1, i => C(7))
    function grow() : void { a.append(C(9)) }
    function count() : Int = a.length()
    apply(grow)
    apply(grow)
    return applyInt(count)
}
print(test())
"####,
            &[]
        ),
        r####"3
"####
    );
}

#[test]
fn lambdas_over_frame_locals_are_not_tail_calls() {
    assert_eq!(
        stdout(
            r####"class C { public var n : Int
 new(n : Int) { this.n = n } }
function applyInt(f : () -> Int) : Int = f()
function test() : Int {
    let a := Array.generate(1, i => C(7))
    let count := () => a.length()
    return applyInt(count)
}
print(test())
"####,
            &[]
        ),
        r####"1
"####
    );
}

#[test]
fn subclasses_may_set_inherited_fields_without_super() {
    assert_eq!(
        stdout(
            r####"class A { public var n : Int = 4
 new() {} }
class Bc extends A { new() {} }
print(Bc().n)
"####,
            &[]
        ),
        r####"4
"####
    );
}

#[test]
fn a_local_function_result_keeps_its_capture_borrowed() {
    refused(
        r####"class C { public var n : Int
 new(n : Int) { this.n = n } }
class H { public var c : C
 new() { this.c = C(7) } }
function test() : Int {
    let h := H()
    function child() : C = h.c
    let first := child()
    h.c = C(99)
    first.n
}
print(test())
"####,
        "E_BORROW_CONFLICT",
    );
}

#[test]
fn an_element_view_keeps_its_array_borrowed() {
    refused(
        r####"class C { public var n : Int
 new(n : Int) { this.n = n } }
function head(a : Array[C]) : C = a[0]
function test() : Int {
    let a := Array.generate(2, i => C(i + 5))
    let first := head(a)
    a.append(C(1))
    return first.n
}
print(test())
"####,
        "E_BORROW_CONFLICT",
    );
}

#[test]
fn a_subclass_must_run_the_parent_constructor() {
    refused(
        r####"class A { public final n : Int
 public final s : String
 new(n : Int) { this.n = n
 this.s = "set" } }
class Bc extends A { new() {} }
print(Bc().n)
print(Bc().s)
"####,
        "E_SUPER_REQUIRED",
    );
}

#[test]
fn a_grandparent_field_also_needs_super() {
    refused(
        r####"class A { public var a : Int
 new() { this.a = 1 } }
class B extends A { public var b : Int
 new() { this.a = 2
 this.b = 3 } }
class Cc extends B { new() { this.b = 4 } }
"####,
        "E_SUPER_REQUIRED",
    );
}

#[test]
fn super_must_come_first_once() {
    refused(
        r####"class A { public final n : Int
 new(n : Int) { this.n = n } }
class Bc extends A { new() { super(1)
 super(2) } }
"####,
        "E_SUPER_POSITION",
    );
}

#[test]
fn super_needs_a_parent() {
    refused(
        r####"class A { new() { super() } }
"####,
        "E_SUPER_CONTEXT",
    );
}

#[test]
fn an_enum_holding_a_class_still_moves() {
    refused(
        r####"class C { public var n : Int
 new(n : Int) { this.n = n } }
enum B { Hold(C) }
let b := Hold(C(1))
let both := [b, b]
"####,
        "E_USE_AFTER_MOVE",
    );
}

#[test]
fn classes_without_comparable_have_no_order() {
    refused(
        r####"class C { public var n : Int
 new(n : Int) { this.n = n } }
print(C(1) < C(2))
"####,
        "E_OPERATOR_TRAIT",
    );
}

#[test]
fn skip_needs_a_test() {
    refused(
        r####"@skip
function f() : void {}
"####,
        "E_ATTRIBUTE_CONTEXT",
    );
}

/// Every case in Unicode's GraphemeBreakTest.txt for Unicode 17 splits as UAX #29 says.
#[test]
fn graphemes_pass_the_unicode_conformance_file() {
    let fixture = include_str!("fixtures/GraphemeBreakTest-17.0.0.txt");
    let mut literals = vec![];
    let mut expected = String::new();
    for line in fixture.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let mut literal = String::from("\"");
        let mut sizes = vec![];
        let mut current = 0;
        for token in line.split_whitespace().skip(1) {
            match token {
                "÷" => {
                    sizes.push(current.to_string());
                    current = 0;
                }
                "×" => {}
                code => {
                    let code = u32::from_str_radix(code, 16).unwrap();
                    let mut units = [0; 2];
                    for unit in char::from_u32(code).unwrap().encode_utf16(&mut units) {
                        literal.push_str(&format!("\\u{unit:04x}"));
                    }
                    current += 1;
                }
            }
        }
        literal.push('"');
        literals.push(literal);
        expected.push_str(&format!("[{}]\n", sizes.join(", ")));
    }
    assert!(literals.len() > 700, "{}", literals.len());
    let source = format!(
        "let cases := [{}]\nfor (s in cases) {{\n    print(s.graphemes().map(g => g.length()).toList())\n}}\n",
        literals.join(",\n")
    );
    assert_eq!(stdout(&source, &[]), expected);
}

/// A `move` lambda that keeps a borrowed function parameter needs that parameter
/// declared `move`; a plain lambda needs `move` itself. Each hint says which.
#[test]
fn lambda_escape_hints_name_the_missing_move() {
    let output = run(
        "function compose(a : (Int) -> Int, b : (Int) -> Int) : (Int) -> Int = move x => b(a(x))\n",
        &[],
        "check",
    );
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(
        text.contains("E_LAMBDA_ESCAPE")
            && text.contains("Declare the borrowed parameter as `move name : Type`"),
        "{text}"
    );
    let output = run(
        "function greet(name : String) : () -> String = () => \"hi \" + name\n",
        &[],
        "check",
    );
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(
        text.contains("E_LAMBDA_ESCAPE") && text.contains("Write `move` before a lambda"),
        "{text}"
    );
}
