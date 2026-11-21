//! Regression tests for the language gaps and libraries added after the second review (REVIEW2.md
//! sections 3 and 4).
use kestrel::{
    modules::{ProjectCheck, check_source},
    native::TemporaryDirectory,
};
use std::process::{Command, Output};

fn checked(source: &str) -> ProjectCheck {
    ProjectCheck {
        modules: vec![check_source("features.kes", source)],
    }
}
/// Runs a program through `kestrel run`, which loads the prelude and standard modules.
fn run(source: &str) -> Output {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
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
fn diagnostics(source: &str) -> String {
    checked(source).render_diagnostics()
}

#[test]
fn generic_records_print_compare_and_take_methods() {
    let output = stdout(
        r####"record Pair[A, B](first : A, second : B)
let p := Pair(1, "x")
print(p)
print(p == Pair(1, "x"))
print(p.first + 1)
print(p.second)
let q : Pair[String, double] = Pair("k", 2.5)
print(q)
record Box1[T : Text](value : T) {
    public function show() : String = f"<{this.value}>"
}
print(Box1(3).show())
print([Pair(2, "b"), Pair(1, "a")].sorted())
"####,
    );
    assert_eq!(
        output,
        r####"Pair(first = 1, second = x)
true
2
x
Pair(first = k, second = 2.5)
<3>
[Pair(first = 1, second = a), Pair(first = 2, second = b)]
"####
    );
}

#[test]
fn comprehensions_take_several_for_clauses() {
    let output = stdout(
        r####"let xs := [1, 2, 3]
let ys := ["a", "b"]
print([(x, y) for x in xs for y in ys])
print([x * y for x in xs if x > 1 for y in xs if y != x])
let grid := [[1, 2], [3, 4, 5]]
print([v for row in grid for v in row])
print([f"{x}{y}{z}" for x in [1, 2] for y in [3] for z in ["p", "q"]])
"####,
    );
    assert_eq!(
        output,
        r####"[(1, a), (1, b), (2, a), (2, b), (3, a), (3, b)]
[2, 6, 3, 6]
[1, 2, 3, 4, 5]
[13p, 13q, 23p, 23q]
"####
    );
}

#[test]
fn a_list_pattern_takes_its_rest_anywhere() {
    let output = stdout(
        r####"let xs := [1, 2, 3, 4]
print(match xs { [..init, last] => f"{init} {last}", [] => "empty" })
print(match xs { [a, ..mid, z] => f"{a} {mid} {z}", _ => "short" })
print(match [7] { [a, ..mid, z] => f"{a} {mid} {z}", _ => "short" })
print(match xs { [..front, 3, 4] => f"ends 34 {front}", _ => "no" })
print(match ["a", "b"] { [..rest, "b"] => rest.length(), _ => -1 })
function last(xs : List[Int]) : Int = match xs { [] => 0, [.., x] => x }
print(last([5, 6, 9]))
"####,
    );
    assert_eq!(
        output,
        r####"[1, 2, 3] 4
1 [2, 3] 4
short
ends 34 [1, 2]
1
9
"####
    );
}

#[test]
fn extend_blocks_add_methods_to_built_in_types() {
    let output = stdout(
        r####"extend List {
    public function second(xs : List[Int]) : Option[Int] = match xs { [_, b, ..] => Some(b), _ => None }
    public function plus(xs : List[Int], n : Int) : List[Int] = xs.map(x => x + n).toList()
}
extend String {
    public function shout(s : String) : String = s.toUpper() + "!"
}
let xs := [1, 2, 3]
print(xs.second())
print(xs.plus(10))
print("hi".shout())
print([5].second())
"####,
    );
    assert_eq!(
        output,
        r####"Some(2)
[11, 12, 13]
HI!
None
"####
    );
}

#[test]
fn strings_numbers_and_lists_gain_everyday_methods() {
    let output = stdout(
        r####"let s := "héllo wörld"
print(s.substring(1, 4))
print(s.slice(-5, 100))
print(s.substring(4, 2) == "")
print(s.reversed())
print("A".ord())
print(233.chr())
print("ab".byteLength() + "é".byteLength())
print("é".byteAt(0))
print("7".padStart(3, "0") + "|" + "x".padEnd(3) + "|")
print("a\nb\nc".lines())
print("  one two  three ".words())
print(12.gcd(18))
print(4.lcm(6))
print((-7).mod(3))
print(17.isqrt())
print(97.isPrime())
print((-5).sign())
print(255.popCount())
print(15.clamp(0, 10))
print(2.5.clamp(0.0, 1.0))
print((-2.7).trunc())
print(27.0.cbrt())
print(3.0.hypot(4.0))
let xs := [1, 2, 3, 4, 5]
print(xs.slice(1, 3))
print(xs.slice(-2, 99))
print(xs.reversed())
print([[1], [2]].reversed())
"####,
    );
    assert_eq!(
        output,
        r####"éll
wörld
true
dlröw olléh
65
é
4
195
007|x  |
[a, b, c]
[one, two, three]
6
12
2
4
true
-1
8
10
1.0
-2.0
3.0
5.0
[2, 3]
[4, 5]
[5, 4, 3, 2, 1]
[[2], [1]]
"####
    );
}

#[test]
fn strings_and_lists_slice_with_colons() {
    let output = stdout(
        r####"let s := "hello"
print(s[1:3])
print(s[:2] + s[-2:])
let xs := [1, 2, 3]
print(xs[1:])
print(xs[:-1])
"####,
    );
    assert_eq!(
        output,
        r####"el
helo
[2, 3]
[1, 2]
"####
    );
}

#[test]
fn list_option_map_and_set_library_methods() {
    let output = stdout(
        r####"class C {
    has Text
    public final n : Int
    new(n : Int) { this.n = n }
    public function text() : String = f"C{this.n}"
}
let xs := [5, 3, 8, 1, 9, 2]
print(xs.take(2))
print(xs.drop(4))
print(xs.takeWhile(x => x > 2))
print(xs.dropWhile(x => x > 2))
print(xs.find(x => x > 6))
print(xs.findIndex(x => x > 6))
print(xs.count(x => x % 2 == 1))
print(xs.partition(x => x > 4))
print(xs.flatMap(x => [x, x * 10]).take(4))
print([[1, 2], [], [3]].flatten())
print(xs.minBy(x => (x - 4).abs()))
print(xs.maxBy(x => -x))
print([1, 2, 1, 3, 2].distinct())
print(xs.chunks(4))
print(xs.windows(5))
print(xs.scan(0, (a, x) => a + x))
print(["apple", "avocado", "banana"].groupBy(s => s[0:1]))
print([1, 3, 5, 7, 9].binarySearch(7))
print([1, 3, 5, 7, 9].binarySearch(4))
print(xs.sortedBy(x => -x))
print(xs.concat([0]))
let cs := [C(3), C(1), C(2)]
print(cs.take(2))
print(cs.reversed())
print(cs.find(c => c.n == 1))
print(cs.minBy(c => c.n))
print(cs.partition(c => c.n > 1))
print(cs.chunks(2))
print(cs.groupBy(c => c.n % 2))
print(cs.dropWhile(c => c.n > 2))
let o : Option[Int] = Some(4)
let none : Option[Int] = None
print(o.map(v => v * 2))
print(none.map(v => v * 2))
print(o.flatMap(v => if v > 3 then Some(v) else None))
print(o.filter(v => v > 10))
print(none.unwrapOr(7))
print(o.unwrapOrElse(() => 0))
print(none.orElse(Some(1)))
print(match none.okOr("missing") { RuntimeError.Message(m) => m, _ => "other" })
let m := Map("a" => 1, "b" => 2, "c" => 3)
print(m.keys())
print(m.values())
print(m.entries())
print(m.remove("b"))
print(m.keep((k, v) => v != 1))
print(m.mapValues(v => v * 100))
print(m.merge(Map("c" => 30, "d" => 4)))
let s := Set(1, 2, 3)
let t := Set(2, 3, 4)
print(s.union(t))
print(s.intersection(t))
print(s.difference(t))
print(Set(2).isSubset(s))
print(s.isDisjoint(Set(9)))
"####,
    );
    assert_eq!(
        output,
        r####"[5, 3]
[9, 2]
[5, 3, 8]
[1, 9, 2]
Some(8)
Some(2)
4
([5, 8, 9], [3, 1, 2])
[5, 50, 3, 30]
[1, 2, 3]
Some(5)
Some(1)
[1, 2, 3]
[[5, 3, 8, 1], [9, 2]]
[[5, 3, 8, 1, 9], [3, 8, 1, 9, 2]]
[0, 5, 8, 16, 17, 26, 28]
Map(a => [apple, avocado], b => [banana])
Some(3)
None
[9, 8, 5, 3, 2, 1]
[5, 3, 8, 1, 9, 2, 0]
[C3, C1]
[C2, C1, C3]
Some(C1)
Some(C1)
([C3, C2], [C1])
[[C3, C1], [C2]]
Map(0 => [C2], 1 => [C3, C1])
[C1, C2]
Some(8)
None
Some(4)
None
7
4
Some(1)
missing
[a, c, b]
[1, 3, 2]
[(a, 1), (c, 3), (b, 2)]
Map(a => 1, c => 3)
Map(c => 3, b => 2)
Map(a => 100, c => 300, b => 200)
Map(a => 1, c => 30, b => 2, d => 4)
Set(3, 2, 4, 1)
Set(3, 2)
Set(1)
true
true
"####
    );
}

#[test]
fn deque_counter_and_bitset() {
    let output = stdout(
        r####"let d : Deque[String] = Deque()
d.pushBack("b")
d.pushBack("c")
d.pushFront("a")
print(d.popBack())
print(d.popFront())
print(d.popFront())
print(d.popFront())
print(d.size())
for (i in range(1000)) { d.pushBack(f"{i}") }
let mut total := 0
while !d.isEmpty() { total += match d.popFront() { Some(s) => s.length(), None => 0 } }
print(total)
let c : Counter[String] = Counter()
c.addAll("the cat and the hat and the bat".split(" "))
print(c.count("the"))
print(c.mostCommon(2))
print(c.distinctCount())
let b := BitSet()
b.add(3)
b.add(70)
b.add(3)
print(b.contains(70))
b.remove(70)
print(b.contains(70))
print(b.size())
"####,
    );
    assert_eq!(
        output,
        r####"Some(c)
Some(a)
Some(b)
None
0
2890
3
[(the, 3), (and, 2)]
5
true
false
1
"####
    );
}

#[test]
fn format_specs_follow_python() {
    let output = stdout(
        r####"let x := 1234567.891
let n := 255
let name := "Kes"
print(f"[{x:>15,.2f}]")
print(f"[{x:<12.1f}]")
print(f"[{n:08d}] [{n:x}] [{n:#X}] [{n:b}] [{n:#o}] [{-n:+,}]")
print(f"[{1234567:,}] [{1234567:_}] [{0.256:.1%}] [{x:.3e}]")
print(f"[{name:^9}] [{name:*>6}] [{name:.2}] [{true:>6}]")
print(f"[{x:.2f}] [{3:.1f}] [{name:?}]")
print(f"[{-42:=+8}] [{42: }] [{65:c}]")
"####,
    );
    assert_eq!(
        output,
        r####"[   1,234,567.89]
[1234567.9   ]
[00000255] [ff] [0XFF] [11111111] [0o377] [-255]
[1,234,567] [1_234_567] [25.6%] [1.235e+06]
[   Kes   ] [***Kes] [Ke] [  true]
[1234567.89] [3.0] ["Kes"]
[-     42] [ 42] [A]
"####
    );
}

#[test]
fn json_parses_and_prints() {
    let output = stdout(
        r####"import std.json

let text := "{\"name\": \"K\\u00e9s \\ud83d\\ude00\", \"tags\": [1, 2.5, -3e2, true, null], \"nested\": {\"empty\": [], \"o\": {}}, \"q\": \"a\\\"b\\n\"}"
let value := json.parse(text)?
print(json.stringify(value))
print(json.pretty(value))
print(match value { JObject(fields) => fields.length(), _ => -1 })
for (bad in ["{", "[1,]", "\"abc", "tru", "1 2", "{\"a\" 1}"]) {
    print(match json.parse(bad) { RuntimeError.Message(m) => m, _ => "parsed" })
}
"####,
    );
    assert_eq!(
        output,
        r####"{"name":"Kés 😀","tags":[1,2.5,-300,true,null],"nested":{"empty":[],"o":{}},"q":"a\"b\n"}
{
  "name": "Kés 😀",
  "tags": [
    1,
    2.5,
    -300,
    true,
    null
  ],
  "nested": {
    "empty": [],
    "o": {}
  },
  "q": "a\"b\n"
}
4
JSON: expected a field name at byte 1.
JSON: unexpected character at byte 3.
JSON: unterminated string at byte 4.
JSON: expected true at byte 0.
JSON: text after the value at byte 2.
JSON: expected : at byte 5.
"####
    );
}

#[test]
fn path_datetime_and_args_modules() {
    let output = stdout(
        r####"import std.path
import std.datetime
import std.args

print(path.fileName("a/b/report.tar.gz"))
print(path.stem("a/b/report.tar.gz"))
print(path.extension("a/b/report.tar.gz"))
print(path.parent("a/b/report.tar.gz"))
print(path.join("a/", "b") + " " + path.join("a", "/abs"))
print(path.parts("/usr//local/bin/"))
let t := datetime.fromUnix(1763199000)
print(datetime.format(t))
print(datetime.toUnix(t))
print(datetime.dayOfWeek(t))
print(datetime.format(datetime.fromUnix(-1)))
print(datetime.format(datetime.parse("2024-02-29T23:30:00+02:00")?))
print(match datetime.parse("2023-02-29") { RuntimeError.Message(m) => m, _ => "?" })
print(datetime.formatDate(datetime.addDays(datetime.parse("2024-12-31")?, 1)))
let p := args.parse(["build", "--verbose", "--out=x.txt", "-j", "4", "file.kes", "--", "--literal"], valued = ["j"])
print(p.flag("verbose"))
print(p.option("out"))
print(p.intOption("j")?)
print(p.positional())
print(p.optionOr("mode", "debug"))
"####,
    );
    assert_eq!(
        output,
        r####"report.tar.gz
report.tar
gz
a/b
a/b /abs
[usr, local, bin]
2025-11-15T09:30:00Z
1763199000
5
1969-12-31T23:59:59Z
2024-02-29T21:30:00Z
"2023-02-29" is not a calendar date.
2025-01-01
true
Some(x.txt)
Some(4)
[build, file.kes, --literal]
debug
"####
    );
}

#[test]
fn statistics_encoding_hashes_and_terminal_text() {
    let output = stdout(
        r####"import std.statistics
import std.encoding
import std.term

let xs := [3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0]
print(statistics.mean(xs)?)
print(statistics.median(xs)?)
print(statistics.quantile(xs, 0.25)?)
print(statistics.mode(xs)?)
print(f"{statistics.stddev(xs)?:.4f}")
print(statistics.histogram(xs, 4)?)
print(encoding.base64Encode("Kestrel ✓"))
print(encoding.base64Decode(encoding.base64Encode("Kestrel ✓"))?)
print(encoding.base64Encode("ab") + " " + encoding.base64Encode("a"))
print(encoding.hexEncode("hi✓"))
print(encoding.hexDecode("6869e29c93")?)
print(f"{encoding.crc32("The quick brown fox jumps over the lazy dog"):08x}")
print(encoding.sha256(""))
print(encoding.sha256("abc"))
print(term.progressBar(5, 10))
print(term.bold("x").length())
"####,
    );
    assert_eq!(
        output,
        r####"3.875
3.5
1.75
1.0
2.7484
[3, 2, 2, 1]
S2VzdHJlbCDinJM=
Kestrel ✓
YWI= YQ==
6869e29c93
hi✓
414fa339
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
[##########..........]  50%
9
"####
    );
}

#[test]
fn files_console_time_and_processes() {
    // process.run hands the command to the system shell: sh, or cmd.exe on Windows.
    let command = if cfg!(windows) {
        "echo hello& exit 3"
    } else {
        "echo hello; exit 3"
    };
    let output = stdout(
        &r####"import std.fs
import std.io
import std.time
import std.process

function main() : void or Error with Effects {
    let dir := fs.tempDir()?
    let file := dir + "/notes.txt"
    fs.writeFile(file, "one\ntwo\n")?
    fs.appendFile(file, "three\n")?
    print(fs.readFile(file)?.lines())
    print(fs.exists(file))
    print(fs.isDir(dir))
    fs.createDirs(dir + "/a/b")?
    fs.copy(file, dir + "/a/copy.txt")?
    fs.rename(dir + "/a/copy.txt", dir + "/a/moved.txt")?
    print(fs.listDir(dir)?)
    print(fs.listDir(dir + "/a")?)
    fs.remove(dir + "/a/moved.txt")?
    print(fs.exists(dir + "/a/moved.txt"))
    print(match fs.readFile(dir + "/missing") { RuntimeError.Io(m) => m.startsWith("Cannot open file"), _ => false })
    io.write("no newline|")
    io.write("\n")
    io.eprint("to stderr")
    let start := time.monotonic()
    time.sleep(0.01)
    print(time.monotonic() - start >= 10000000)
    print(time.now() > 1700000000.0)
    print(process.env("PATH").isSome())
    print(process.env("KESTREL_NOT_SET_ANYWHERE"))
    print(process.cwd().length() > 0)
    let (status, output) := process.run("COMMAND")?
    print(status)
    print(output.trim())
}
main()?
"####
            .replace("COMMAND", command),
    );
    assert_eq!(
        output,
        r####"[one, two, three]
true
true
[a, notes.txt]
[b, moved.txt]
false
true
no newline|
true
true
true
None
true
3
hello
"####
    );
}

#[test]
fn nat_division_and_list_plus() {
    let output = run(
        "let a : Nat = 17\nlet b : Nat = 5\nprint(a / b)\nprint(a % b)\nprint([1] + [2, 3])\nlet z : Nat = 0\nprint(a / z)\n",
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "3\n2\n[1, 2, 3]\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Nat division by zero"));
}

#[test]
fn the_kernel_and_smt_compute_division_remainder_and_powers() {
    let found = diagnostics(
        r####"enum Box2 { A(Int), B }
theorem inj(h : A(1) = A(2)) : 1 = 0 by { contradiction }
theorem refl(h : not (3 = 3)) : 1 = 0 by { contradiction }
theorem half(n : Int) : (n * 2) / 2 = n by { smt }
theorem parity(n : Int) : n % 2 == 0 || n % 2 == 1 || n % 2 == -1 by { smt }
theorem lit() : 17 / 5 = 3 by { rfl }
theorem litmod() : 17 % 5 = 2 by { decide }
theorem pow() : 2 ** 10 = 1024 by { rfl }
theorem sq(n : Int) : n ** 2 >= 0 by { smt }
theorem natdiv(n : Nat) : Succ(Succ(Succ(Succ(Zero)))) / Succ(Succ(Zero)) = Succ(Succ(Zero)) by { rfl }
"####,
    );
    assert!(!found.contains("error["), "{found}");
    for wrong in [
        "theorem bad(n : Int) : n / 0 = 0 by { smt }\n",
        "theorem bad(n : Int) : (-7) / 2 = -4 by { smt }\n",
        "enum B { A(Int), C }\ntheorem bad(h : A(1) = A(1)) : 1 = 0 by { contradiction }\n",
    ] {
        let found = diagnostics(wrong);
        assert!(found.contains("error["), "{wrong}\n{found}");
    }
}

#[test]
fn copy_bounds_and_format_errors_are_checked() {
    let found =
        diagnostics("class C { new() {} }\nfunction f[T : Copy](x : T) : T = x\nlet r := f(C())\n");
    assert!(found.contains("E_TRAIT_BOUND"), "{found}");
    let found = diagnostics("let s := \"a\"\nprint(f\"{s:d}\")\n");
    assert!(found.contains("E_FORMAT_SPEC"), "{found}");
    let found = diagnostics("extend List {\n    function bad(n : Int) : Int = n\n}\n");
    assert!(found.contains("E_EXTEND_RECEIVER"), "{found}");
}

#[test]
fn a_tail_call_with_string_arguments_does_not_reuse_the_callers_frame() {
    let output = stdout(
        "import std.encoding\nprint(encoding.base64Decode(\"YQ==\")?)\nprint(encoding.hexDecode(\"6869\")?)\n",
    );
    assert_eq!(output, "a\nhi\n");
}

#[test]
fn a_program_may_replace_a_prelude_type_and_find_does_not_leak() {
    let output = stdout(
        "class Counter {\n    public var n : Int\n    new() { this.n = 0 }\n    public mut function inc() : void { this.n += 1 }\n}\nlet c := Counter()\nc.inc()\nprint(c.n)\nlet s : Stack[Int] = Stack()\ns.push(3)\nprint(s.size())\nprint(\"a/b/c\".rfind(\"/\"))\n",
    );
    assert_eq!(output, "1\n1\n3\n");
}

#[test]
fn generic_class_methods_see_their_type_arguments() {
    let output = stdout(
        r####"class Box2[T] {
    private var items : Array[T]
    new() { this.items = Array.empty() }
    public mut function add(move v : T) : void { this.helper(v) }
    private mut function helper(move v : T) : void { this.items.append(v) }
    public function size() : Int = this.items.length()
}
class Local[T] {
    private var items : Array[T]
    new() { this.items = Array.empty() }
    public mut function push(move v : T) : void { this.items.append(v) }
    public function count() : Int {
        let spare : Array[Int] = Array.empty()
        let other : Array[T] = Array.empty()
        spare.append(other.length())
        return this.items.length() + spare.length()
    }
}
let b : Box2[String] = Box2()
b.add("x")
print(b.size())
let l : Local[String] = Local()
l.push("y")
print(l.count())
"####,
    );
    assert_eq!(
        output,
        r####"1
2
"####
    );
}

#[test]
fn generic_methods_without_type_parameters_build_local_arrays() {
    let output = stdout(
        r####"function f[T](x : T) : Int {
    let other : Array[T] = Array.empty()
    return other.length()
}
print(f("a"))
class L[T] {
    new() {}
    public function g(x : T) : Int {
        let other : Array[T] = Array.empty()
        return other.length()
    }
    public function h() : Int {
        let other : Array[T] = Array.empty()
        return other.length()
    }
}
let l : L[String] = L()
print(l.g("a"))
print(l.h())
"####,
    );
    assert_eq!(
        output,
        r####"0
0
0
"####
    );
}

#[test]
fn a_match_moves_an_item_popped_from_an_array() {
    let output = stdout(
        r####"class C {
    has Text
    public final n : Int
    new(n : Int) { this.n = n }
    public function text() : String = f"C{this.n}"
}
class Two[T] {
    private var a : Array[T]
    private var b : Array[T]
    new() {
        this.a = Array.empty()
        this.b = Array.empty()
    }
    public mut function push(move v : T) : void { this.a.append(v) }
    public mut function shift() : void {
        for (i in range(this.a.length())) {
            match this.a.pop() {
                Some(v) => { this.b.append(v) },
                None => {},
            }
        }
    }
    public mut function take() : Option[T] = this.b.pop()
}
let t : Two[C] = Two()
t.push(C(1))
t.push(C(2))
t.shift()
print(t.take())
print(t.take())
print(t.take())
let arr := Array.generate(2, i => C(i + 10))
let mut got : Array[C] = Array.empty()
match arr.pop() {
    Some(c) => { got.append(c) },
    None => {},
}
print(got[0].n)
print(arr.length())
"####,
    );
    assert_eq!(
        output,
        r####"Some(C1)
Some(C2)
None
11
1
"####
    );
}

#[test]
fn deque_holds_class_items() {
    let output = stdout(
        r####"class C {
    has Text
    public final n : Int
    new(n : Int) { this.n = n }
    public function text() : String = f"C{this.n}"
}
let d : Deque[C] = Deque()
for (i in range(6)) { d.pushBack(C(i)) }
print(d.popFront())
print(d.popBack())
d.pushFront(C(100))
print(d.popFront())
print(d.popFront())
print(d.popBack())
print(d.size())
let q : Deque[Int] = Deque()
for (i in range(10000)) { q.pushBack(i) }
let mut sum := 0
while !q.isEmpty() {
    sum += match q.popFront() { Some(v) => v, None => 0 }
    sum += match q.popBack() { Some(v) => v, None => 0 }
}
print(sum)
"####,
    );
    assert_eq!(
        output,
        r####"Some(C0)
Some(C5)
Some(C100)
Some(C1)
Some(C4)
2
49995000
"####
    );
}

#[test]
fn map_and_mutmap_remove_keys() {
    let output = stdout(
        r####"class C {
    has Text
    public final n : Int
    new(n : Int) { this.n = n }
    public function text() : String = f"C{this.n}"
}
let m : MutMap[String, C] = MutMap()
for (i in range(50)) { m.put(f"k{i}", C(i)) }
let mut removed := 0
for (i in range(0, 50, 3)) { if m.remove(f"k{i}") { removed += 1 } }
print(removed)
print(m.remove("k0"))
print(m.length())
let mut found := 0
for (i in range(50)) { if m.contains(f"k{i}") { found += 1 } }
print(found)
print(m.get("k1"))
let ints : MutMap[Int, Int] = MutMap()
for (i in range(1000)) { ints.put(i, i * 2) }
for (i in range(0, 1000, 2)) { ints.remove(i) }
let mut total := 0
for (i in range(1000)) { total += ints.get(i, 0) }
print(ints.length())
print(total)
let p := Map("a" => 1, "b" => 2, "c" => 3)
let q := p.remove("b")
print(p)
print(q)
print(q.remove("zz").length())
for (i in range(1000)) { ints.put(i, 1) }
print(ints.length())
"####,
    );
    assert_eq!(
        output,
        r####"17
false
33
33
Some(C1)
500
500000
Map(a => 1, c => 3, b => 2)
Map(a => 1, c => 3)
2
1000
"####
    );
}

#[test]
fn mutmap_remove_agrees_with_a_model() {
    let output = stdout(
        r####"let flat : MutMap[Int, Int] = MutMap()
let boxed : MutMap[String, Int] = MutMap()
let model : Array[Int] = Array(512, -1)
let mut seed := 12345
let mut bad := 0
for (step in range(200000)) {
    seed = (seed * 1103515245 + 12345) % 2147483648
    let key := seed % 512
    let action := (seed / 512) % 3
    if action == 0 {
        flat.put(key, step)
        boxed.put(f"{key}", step)
        model[key] = step
    } else {
        let present := model[key] >= 0
        if flat.remove(key) != present { bad += 1 }
        if boxed.remove(f"{key}") != present { bad += 1 }
        model[key] = -1
    }
}
let mut count := 0
for (k in range(512)) {
    let expected := model[k]
    if expected >= 0 { count += 1 }
    if flat.get(k, -1) != expected { bad += 1 }
    let b := match boxed.get(f"{k}") { Some(v) => v, None => -1 }
    if b != expected { bad += 1 }
}
print(bad)
print(flat.length() == count && boxed.length() == count)
"####,
    );
    assert_eq!(
        output,
        r####"0
true
"####
    );
}

#[test]
fn string_format_fills_a_literal_template() {
    let output = stdout(
        r####"record P(name : String, age : Int)
let name := "Ana"
let score := 0.91234
print("{} scored {:.1%}".format(name, score))
print("{1} before {0}, then {1:>6}".format("a", "b"))
print("{{literal}} {:08.3f} {}".format(3.14159, P("x", 3)))
print("{:?} {}".format("quoted", [1, 2]))
print("none".format())
"####,
    );
    assert_eq!(
        output,
        r####"Ana scored 91.2%
b before a, then      b
{literal} 0003.142 P(name = x, age = 3)
"quoted" [1, 2]
none
"####
    );
}

#[test]
fn csv_parses_quotes_line_breaks_and_records() {
    let output = stdout(
        r####"import std.csv

let text := "name,quote,n\r\nAna,\"Hello, \"\"world\"\"\",1\nBo,\"two\nlines\",\n,,3\n"
let rows := csv.parse(text)?
print(rows)
print(rows.length())
let records := csv.parseRecords(text)?
print(records.map(r => r.get("name").unwrapOr("?")).toList())
print(csv.write(rows))
print(csv.parse(csv.write(rows))? == rows)
print(csv.parse("a;b\n1;2", delimiter = ";")?)
for (bad in ["a,\"b", "a,b\"c", "x,\"y\"z"]) {
    print(match csv.parse(bad) { RuntimeError.Message(m) => m, _ => "parsed" })
}
print(match csv.parseRecords("a,b\n1\n") { RuntimeError.Message(m) => m, _ => "parsed" })
"####,
    );
    assert_eq!(
        output,
        r####"[[name, quote, n], [Ana, Hello, "world", 1], [Bo, two
lines, ], [, , 3]]
4
[Ana, Bo, ]
name,quote,n
Ana,"Hello, ""world""",1
Bo,"two
lines",
,,3

true
[[a, b], [1, 2]]
CSV: a quoted field that starts on line 1 is never closed
CSV: a quote inside an unquoted field on line 1
CSV: text after a closing quote on line 1
CSV: row 2 has 1 fields, but the header has 2
"####
    );
}

#[test]
fn toml_reads_tables_arrays_and_values() {
    let output = stdout(
        r####"import std.toml

let text := "# config\ntitle = \"TOML \\u00e9 example\"\nversion = 1_000\nhex = 0xff\nratio = 6.5e-1\nflag = true\nwhen = 1979-05-27 07:32:00Z\nliteral = 'C:\\path'\nlines = \"\"\"\nfirst\nsecond\"\"\"\nsite.\"google.com\" = true\n\n[server]\nhost = \"localhost\"\nport = 8080\nports = [ 8000,\n  8001, # second\n  8002, ]\ninline = { x = 1, y = { z = \"deep\" } }\n\n[server.tls]\nenabled = false\n\n[[fruit]]\nname = \"apple\"\n[[fruit]]\nname = \"banana\"\n"
let doc := toml.parse(text)?
print(toml.getString(doc, "title"))
print(toml.getInt(doc, "version"))
print(toml.getInt(doc, "hex"))
print(toml.getFloat(doc, "ratio"))
print(toml.getBool(doc, "flag"))
print(toml.getString(doc, "when"))
print(toml.getString(doc, "literal"))
print(toml.getString(doc, "lines"))
print(toml.getString(doc, "server.host"))
print(toml.getInt(doc, "server.port"))
print(toml.getFloat(doc, "server.port"))
print(toml.getBool(doc, "server.tls.enabled"))
print(toml.getString(doc, "server.inline.y.z"))
print(toml.getInt(doc, "missing.key"))
print(match doc { TTable(fields) => fields.map(f => f[0]).toList(), _ => [] })
let fruit := match doc { TTable(fields) => fields.filter(f => f[0] == "fruit").map(f => match f[1] { TArray(items) => items.length(), _ => 0 }).toList(), _ => [] }
print(fruit)
for (bad in ["a = 1\na = 2", "a = [1, 2", "a = 012", "a = 1 2", "[x]\nb = 1\n[x.b]\nc = 1", "a = \"open"]) {
    print(match toml.parse(bad) { RuntimeError.Message(m) => m, _ => "parsed" })
}
"####,
    );
    assert_eq!(
        output,
        r####"Some(TOML é example)
Some(1000)
Some(255)
Some(0.65000000000000002)
Some(true)
Some(1979-05-27 07:32:00Z)
Some(C:\path)
Some(first
second)
Some(localhost)
Some(8080)
Some(8080.0)
Some(false)
Some(deep)
None
[title, version, hex, ratio, flag, when, literal, lines, site, server, fruit]
[2]
TOML line 2: `a` is defined twice.
TOML line 1: expected , or ] in an array.
TOML line 1: `012` has a leading zero.
TOML line 1: expected the end of the line.
TOML line 3: `b` is a value, not a table.
TOML line 1: a string is never closed.
"####
    );
}

#[test]
fn regex_finds_replaces_and_splits() {
    let output = stdout(
        r####"import std.regex

let date := regex.compile("(\\d{4})-(\\d{2})-(\\d{2})")?
match date.find("due 2026-11-15, paid 2026-12-01") {
    Some(m) => {
        print(m.text)
        print(f"{m.start} {m.end}")
        print(m.group(1))
        print(m.group(3))
        print(m.group(9))
    },
    None => print("none"),
}
print(date.findAll("due 2026-11-15, paid 2026-12-01").map(m => m.text).toList())
print(regex.compile("\\s+")?.replace("a   b \t c", " "))
print(regex.compile("(\\w+)@(\\w+)\\.com")?.replace("ana@site.com, bo@web.com", "$2:$1 ($$)"))
print(regex.compile(",\\s*")?.split("a, b,c,   d"))
print(regex.compile("^[A-Z][a-z]+$")?.fullMatch("Kestrel"))
print(regex.compile("[a-z]+")?.fullMatch("abc1"))
print(regex.compile("(?i)hello")?.isMatch("say HeLLo"))
print(regex.compile("a|ab")?.find("ab").unwrapOr(Match("-", 0, 0, 0, 0, [], [])).text)
print(regex.compile("a*?b")?.find("aaab").unwrapOr(Match("-", 0, 0, 0, 0, [], [])).text)
print(regex.compile("x*")?.findAll("axxb").map(m => f"{m.start}-{m.end}").toList())
print(regex.compile("é.")?.find("café!").unwrapOr(Match("-", 0, 0, 0, 0, [], [])).start)
print(regex.compile("\\bcat\\b")?.findAll("cat concat cat.").length())
print(regex.compile("(a)|(b)")?.find("b").unwrapOr(Match("-", 0, 0, 0, 0, [], [])).group(1))
print(regex.compile("[^0-9]+")?.findAll("12ab34cd").map(m => m.text).toList())
print(regex.escape("1+1=2?"))
let slow := regex.compile("(a*)*b")?
print(slow.isMatch("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaac"))
for (bad in ["(", "a)", "[z-a]", "*", "a{3,1}", "\\q", "(?=x)"]) {
    print(match regex.compile(bad) { RuntimeError.Message(m) => m, _ => "compiled" })
}
"####,
    );
    assert_eq!(
        output,
        r####"2026-11-15
4 14
Some(2026)
Some(15)
None
[2026-11-15, 2026-12-01]
a b c
site:ana ($), web:bo ($)
[a, b, c, d]
true
false
true
a
aaab
[0-0, 1-3, 3-3, 4-4]
3
2
None
[ab, cd]
1\+1=2\?
false
regex: a ( group is never closed at position 1
regex: a ) has no matching ( at position 1
regex: a range in [ ] runs backwards at position 4
regex: nothing to repeat at position 1
regex: a repeat range runs backwards at position 6
regex: unknown escape at position 2
regex: only (?:...) and (?i) groups are supported at position 1
"####
    );
}

#[test]
fn regex_word_boundaries_need_no_live_thread() {
    let output = stdout(
        r####"import std.regex
let r := regex.compile("\\bcat\\b")?
for (t in ["cat.", " cat.", "x cat", "concat cat", "cat concat cat."]) { print(f"{t}: {r.findAll(t).map(m => m.start).toList()}") }
let q := regex.compile("cat")?
print(q.findAll("cat concat cat.").map(m => m.start).toList())
"####,
    );
    assert_eq!(
        output,
        r####"cat.: [0]
 cat.: [1]
x cat: [2]
concat cat: [7]
cat concat cat.: [0, 11]
[0, 7, 11]
"####
    );
}

#[test]
fn format_templates_and_regex_patterns_report_their_errors() {
    for (source, code) in [
        ("print(\"{} {}\".format(1))\n", "E_FORMAT_ARGUMENTS"),
        ("print(\"{}\".format(1, 2))\n", "E_FORMAT_ARGUMENTS"),
        ("print(\"{0} {}\".format(1, 2))\n", "E_FORMAT_TEMPLATE"),
        ("print(\"{:d}\".format(\"s\"))\n", "E_FORMAT_SPEC"),
        ("let t := \"{}\"\nprint(t.format(1))\n", "E_FORMAT_TEMPLATE"),
    ] {
        let found = diagnostics(source);
        assert!(found.contains(code), "{source}\n{found}");
    }
}

#[test]
fn regex_runs_in_linear_time() {
    // `(a*)*b` takes exponential time in a backtracking engine on this text.
    let started = std::time::Instant::now();
    let output = stdout(
        "import std.regex\nlet r := regex.compile(\"(a*)*b\")?\nprint(r.isMatch(\"a\" * 5000 + \"c\"))\n",
    );
    assert_eq!(output, "false\n");
    assert!(started.elapsed().as_secs() < 30, "{:?}", started.elapsed());
}
