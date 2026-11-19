use kestrel::{
    checker::{Checked, check},
    parse_file,
};

const COUNTER: &str = include_str!("../testdata/language/mutation.kes");

fn checked(source: &str) -> Checked {
    check(&parse_file("resources.kes", source))
}
fn good(source: &str) {
    let result = checked(source);
    assert!(result.is_ok(), "{source}\n{:#?}", result.diagnostics);
}
fn refuses(source: &str, code: &str) {
    let result = checked(source);
    assert!(
        result.diagnostics.iter().any(|d| d.code == code),
        "Expected {code}: {source}\n{:#?}",
        result.diagnostics
    );
}

#[test]
fn a_mutex_is_reached_only_through_a_using_lock() {
    good(
        "function run() : void or Error with Effects { let counter := Mutex(0)\nusing g := counter.lock()? {\ng.value = g.value + 1\n} }",
    );
    good(
        "function bump(m : Mutex[Int]) : Int or Error with Effects { using g := m.lock()? { g.value = g.value + 1\ng.value } }",
    );
    refuses("let counter := Mutex(0)\ncounter.value", "E_MUTEX_GUARD");
    refuses(
        "let counter := Mutex(0)\nlet g := counter.lock()",
        "E_MUTEX_GUARD",
    );
    refuses(
        "function bump(m : Mutex[Int]) : void or Error { using g := m.lock()? { g.value = 1 } }",
        "E_EFFECT_REQUIRED",
    );
    refuses(
        "function run() : void or Error with Effects { let counter := Mutex(0)\nusing g := counter.lock()? { g.size } }",
        "E_UNKNOWN_MEMBER",
    );
    refuses("using x := 5 { }", "E_USING_RESOURCE");
}

#[test]
fn readers_read_and_writers_write() {
    good(
        "function firstLine(path : String) : String or Error with Effects {\nusing f := open(path)? { f.readLine()? }\n}",
    );
    good(
        "function save(path : String, line : String) : void or Error with Effects {\nusing w := create(path)? { w.writeLine(line)? }\n}",
    );
    good(
        "function first(mut r : Reader) : String or Error with Effects = r.readLine()\nfunction run(path : String) : String or Error with Effects {\nusing r := open(path)? { first(r) }\n}",
    );
    refuses(
        "function f(path : String) : void or Error with Effects {\nusing r := open(path)? { r.writeLine(\"x\")? }\n}",
        "E_UNKNOWN_MEMBER",
    );
    refuses(
        "function f(path : String) : void with Effects {\nusing r := open(path) { }\n}",
        "E_USING_RESOURCE",
    );
    refuses(
        "function f(path : String) : String or Error = open(path)?.readLine()",
        "E_EFFECT_REQUIRED",
    );
    refuses(
        "function first(r : Reader) : String or Error with Effects = r.readLine()",
        "E_MUT_PARAMETER",
    );
    refuses(
        "function keep(move r : Reader) : void {}\nfunction f(path : String) : void or Error with Effects {\nusing r := open(path)? { keep(r) }\n}",
        "E_MOVE_BORROWED",
    );
}

#[test]
fn tasks_start_inside_a_scope_and_report_errors_through_it() {
    let save = "function save(text : String) : void with Effects = print(text)\nfunction store(text : String) : void or Error with Effects {\nusing w := create(\"out.txt\")? { w.writeLine(text)? }\n}\n";
    good(&format!(
        "{save}function run() : void with Effects {{ scope {{ spawn save(\"a\")\nspawn save(\"b\") }} }}"
    ));
    good(&format!(
        "{save}function run() : void or Error with Effects {{ scope {{ spawn store(\"a\")\nspawn save(\"b\") }}? }}"
    ));
    refuses(
        &format!("{save}function run() : void with Effects {{ spawn save(\"a\") }}"),
        "E_SPAWN_SCOPE",
    );
    refuses(
        &format!("{save}function run() : void {{ scope {{ spawn save(\"a\") }} }}"),
        "E_EFFECT_REQUIRED",
    );
    refuses(
        "function size() : Int = 1\nfunction run() : void with Effects { scope { spawn size() } }",
        "E_SPAWN_RESULT",
    );
    refuses(
        &format!(
            "{save}function run() : void with Effects {{ scope {{ let f := () => {{ spawn save(\"a\") }} }} }}"
        ),
        "E_SPAWN_SCOPE",
    );
}

#[test]
fn tasks_cannot_change_what_another_part_of_the_scope_uses() {
    let tasks = "function bump(mut c : Counter) : void = c.increment()\nfunction look(c : Counter) : void with Effects = print(c.current())\n";
    good(&format!(
        "{COUNTER}{tasks}function run() : void with Effects {{ let c := Counter()\nscope {{ spawn look(c)\nspawn look(c) }} }}"
    ));
    good(&format!(
        "{COUNTER}{tasks}function run() : void with Effects {{ let a := Counter()\nlet b := Counter()\nscope {{ spawn bump(a)\nspawn bump(b) }} }}"
    ));
    refuses(
        &format!(
            "{COUNTER}{tasks}function run() : void with Effects {{ let c := Counter()\nscope {{ spawn bump(c)\nspawn bump(c) }} }}"
        ),
        "E_TASK_CONFLICT",
    );
    refuses(
        &format!(
            "{COUNTER}{tasks}function run() : void with Effects {{ let c := Counter()\nscope {{ spawn bump(c)\nspawn look(c) }} }}"
        ),
        "E_TASK_CONFLICT",
    );
    refuses(
        &format!(
            "{COUNTER}{tasks}function run() : Int with Effects {{ let c := Counter()\nscope {{ spawn bump(c)\nc.current() }}\n0 }}"
        ),
        "E_TASK_CONFLICT",
    );
    refuses(
        &format!(
            "{COUNTER}{tasks}function run() : void with Effects {{ let c := Counter()\nscope {{ spawn look(c)\nc.increment() }} }}"
        ),
        "E_TASK_CONFLICT",
    );
    good(&format!(
        "{COUNTER}{tasks}function run() : Int with Effects {{ let c := Counter()\nscope {{ spawn bump(c) }}\nc.current() }}"
    ));
}

#[test]
fn channels_need_effects_and_keep_what_they_are_sent() {
    good(
        "let c := Channel[Int]()\nc.send(5)?\nlet n := match c.receive() { Some(n) => n, None => 0, }",
    );
    refuses(
        "function next(c : Channel[Int]) : Option[Int] = c.receive()",
        "E_EFFECT_REQUIRED",
    );
    good("function next(c : Channel[Int]) : Option[Int] with Effects = c.receive()");
    refuses(
        &format!(
            "{COUNTER}let ch := Channel[Counter]()\nlet k := Counter()\nch.send(k)?\nk.current()"
        ),
        "E_USE_AFTER_MOVE",
    );
}

#[test]
fn using_a_file_cannot_escalate_a_read_only_borrow() {
    refuses(
        "function close(r : Reader) : void or Error with Effects { using view := r {} }",
        "E_MUT_PARAMETER",
    );
    good("function close(mut r : Reader) : void or Error with Effects { using view := r {} }");
    refuses(
        "class Holder { public var reader : Reader\nnew(move reader : Reader) { this.reader=reader } }\nfunction close(holder : Holder) : void or Error with Effects {using r := holder.reader {}}",
        "E_MUT_PARAMETER",
    );
}

#[test]
fn loop_jumps_cannot_leave_blocks_whose_exit_can_fail() {
    refuses(
        "function f(paths : List[String]) : void or Error with Effects { for (p in paths) { using w := create(p)? { if p == \"\" { continue }\nw.writeLine(p)? }? } }",
        "E_LOOP_EXIT",
    );
    refuses(
        "function work() : void or Error with Effects = RuntimeError.Io(\"x\")\nfunction f() : void or Error with Effects { while true { scope { spawn work()\nbreak }? } }",
        "E_LOOP_EXIT",
    );
    // A guard inside a file block does not hide the file's failure.
    refuses(
        "function f(m : Mutex[Int], p : String) : void or Error with Effects { while true { using w := create(p)? { using g := m.lock()? { break } }? } }",
        "E_LOOP_EXIT",
    );
    good(
        "function f(paths : List[String]) : void or Error with Effects { for (p in paths) { using w := create(p)? { for (q in paths) { if q == p { break } } }? } }",
    );
    good(
        "function f(m : Mutex[Int]) : void or Error with Effects { while true { using g := m.lock()? { break } } }",
    );
    good(
        "function work() : void with Effects = print(1)\nfunction f() : void with Effects { while true { scope { spawn work()\nbreak } } }",
    );
    good(
        "function f(p : String) : void or Error with Effects { for (n in [1]) { using w := create(p)? { let skip := () => { for (m in [2]) { break } } }? } }",
    );
}

#[test]
fn c_functions_need_unsafe_direct_calls_effects_and_boundary_types() {
    refuses(
        "import c \"math.h\"\nfunction root() : double with Effects = sqrt(2.0)",
        "E_UNSAFE_CALL",
    );
    refuses(
        "import c \"math.h\"\nlet f := unsafe { sqrt }",
        "E_UNSAFE_CALL",
    );
    refuses(
        "import c \"math.h\"\nfunction root() : double = unsafe { sqrt(2.0) }",
        "E_EFFECT_REQUIRED",
    );
    good("import c \"math.h\"\nfunction root() : double with Effects = unsafe { sqrt(2.0) }");
    // Q51/Q64: text arguments and typed opaque pointer results.
    good("import c \"string.h\"\nlet n := unsafe { strlen(\"x\") }");
    good("import c \"string.h\"\nlet n := unsafe { strdup(\"x\") }");
    refuses(
        "import c \"string.h\"\nlet n := unsafe { strlen(3) }",
        "E_CALL_ARGUMENTS",
    );
    refuses(
        // A numeral adapts to CInt, but an Int value needs an explicit conversion.
        "import c \"stdlib.h\"\nlet x := 3\nlet n := unsafe { abs(x) }",
        "E_CALL_ARGUMENTS",
    );
    refuses("import c \"no-such-header.h\"", "E_C_HEADER");
    good(
        "import std.c.{CInt32, CUInt64}\nfunction f(n : CInt32, m : CUInt64) : Int or Error { let a := n.toInt()\nlet b := m.checkedToInt()?\na + b }",
    );
    refuses(
        "import std.c.{CUInt64}\nfunction f(m : CUInt64) : Int = m.toInt()",
        "E_UNKNOWN_MEMBER",
    );
}

#[test]
fn python_calls_need_effects_error_results_and_boundary_values() {
    let numpy = "import python \"math\" as pymath\n";
    good(&format!(
        "{numpy}function f() : double or Error with Effects = pymath.sqrt(2.0)"
    ));
    refuses(
        &format!("{numpy}function f() : double or Error = pymath.sqrt(2.0)"),
        "E_EFFECT_REQUIRED",
    );
    refuses(
        &format!("{numpy}function f() : double with Effects = pymath.sqrt(2.0)"),
        "E_PYTHON_RESULT",
    );
    refuses(
        &format!(
            "{numpy}class Box2 {{ new() {{}} }}\nfunction f() : void or Error with Effects {{ let r := pymath.sqrt(Box2())? }}"
        ),
        "E_PYTHON_TYPE",
    );
    refuses(&format!("{numpy}let m := pymath"), "E_PYTHON_MODULE");
    refuses(
        &format!("{numpy}function f() : double or Error with Effects = pymath.pow(x = 2.0, 3.0)"),
        "E_ARGUMENT_ORDER",
    );
    good(&format!(
        "{numpy}import std.python.{{PythonValue}}\nfunction f(v : PythonValue) : PythonValue or Error with Effects = v.real"
    ));
    good(&format!(
        "{numpy}function f() : List[Int] or Error with Effects = pymath.floor(2.5)"
    ));
}

const ARENA_POINT: &str = "import std.memory.{Arena, ArenaRef}\nclass Point {public var x : Int\nnew(x : Int) {this.x = x}\npublic mut function bump() : void {this.x = this.x + 1}}\n";

#[test]
fn arenas_move_but_handles_copy_without_outside_effects() {
    good(&format!(
        "{ARENA_POINT}function expired() : ArenaRef[Point] {{let a := Arena[Point]()\na.insert(Point(1))}}\nfunction put(mut a : Arena[Point]) : ArenaRef[Point] = a.insert(Point(2))\nfunction use(move a : Arena[Point]) : void {{}}\nfunction local() : void or Error {{let a := Arena[Point]()\nlet h := put(a)\nlet copy := h\na.replace(copy,Point(3))?\nuse(a)}}"
    ));
    refuses(
        &format!(
            "{ARENA_POINT}function put(a : Arena[Point]) : ArenaRef[Point] = a.insert(Point(1))"
        ),
        "E_MUT_PARAMETER",
    );
    refuses(
        &format!("{ARENA_POINT}let a := Arena[Point]()\nlet b := a\na.insert(Point(1))"),
        "E_USE_AFTER_MOVE",
    );
    refuses(
        &format!(
            "{ARENA_POINT}function save(mut a : Arena[Point], p : Point) : ArenaRef[Point] = a.insert(p)"
        ),
        "E_MOVE_BORROWED",
    );
}

#[test]
fn arena_views_cannot_escape_mutate_or_overlap_storage_changes() {
    refuses(
        &format!(
            "{ARENA_POINT}function bad() : Point {{let a := Arena[Point]()\nlet h := a.insert(Point(1))\nmatch a.get(h) {{Some(p) => p, None => Point(0),}}}}"
        ),
        "E_BORROW_ESCAPE",
    );
    refuses(
        &format!(
            "{ARENA_POINT}let a := Arena[Point]()\nlet h := a.insert(Point(1))\nmatch a.get(h) {{Some(p) => p.bump(), None => {{}},}}"
        ),
        "E_MUT_BORROW",
    );
    for change in ["a.insert(Point(2))", "let _ := a.replace(h,Point(2))"] {
        refuses(
            &format!(
                "{ARENA_POINT}let a := Arena[Point]()\nlet h := a.insert(Point(1))\nmatch a.get(h) {{Some(p) => {{{change}\nprint(p.x)}}, None => {{}},}}"
            ),
            "E_BORROW_CONFLICT",
        );
    }
}

#[test]
fn arena_handles_are_typed_opaque_identifiers() {
    refuses(
        "import std.memory.{Arena, ArenaRef}\nlet a := Arena[Int]()\nlet b := Arena[String]()\nlet h := a.insert(3)\nb.get(h)",
        "E_CALL_ARGUMENTS",
    );
    refuses(
        "import std.memory.{ArenaRef}\nArenaRef[Int]()",
        "E_ARENA_HANDLE",
    );
    refuses(
        "import std.memory.{Arena}\nlet a := Arena[Int]()\nprint(a.insert(1))",
        "E_TRAIT_BOUND",
    );
}

#[test]
fn arena_insertion_keeps_existing_limits_on_moving_borrowed_payloads() {
    let definitions = format!(
        "{ARENA_POINT}class Panel {{public var point : &Point\nnew(point : Point) {{this.point = point}}}}\n"
    );
    refuses(
        &format!(
            "{definitions}function bad() : Arena[Panel] {{let a := Arena[Panel]()\nlet p := Point(1)\na.insert(Panel(p))\na}}"
        ),
        "E_MOVE_BORROWED",
    );
    refuses(
        &format!(
            "{definitions}function bad(mut a : Arena[Panel], p : Point) : ArenaRef[Panel] = a.insert(Panel(p))"
        ),
        "E_MOVE_BORROWED",
    );
}

#[test]
fn arena_borrowed_closures_cannot_change_their_captures() {
    refuses(
        &format!(
            "{ARENA_POINT}let a := box(Arena[() -> void]())\nlet p := Point(1)\nlet f := move () => p.bump()\na.insert(f)"
        ),
        "E_ARENA_CAPTURE",
    );
    refuses(
        &format!(
            "{ARENA_POINT}let a := Arena()\nlet p := Point(1)\nlet f := move () => p.bump()\nlet h := a.insert(f)\nmatch a.get(h) {{Some(g) => g(), None => {{}},}}"
        ),
        "E_NOT_CALLABLE",
    );
    refuses(
        &format!(
            "{ARENA_POINT}let a := Arena[() -> void]()\nlet p := Point(1)\nlet f := move () => p.bump()\na.insert(f)"
        ),
        "E_ARENA_CAPTURE",
    );
    refuses(
        &format!(
            "{ARENA_POINT}function read(a : Arena[() -> void]) : void {{}}\nlet a := Arena()\nlet p := Point(1)\nlet f := move () => p.bump()\nlet h := a.insert(f)\nread(a)"
        ),
        "E_CALL_ARGUMENTS",
    );
}
