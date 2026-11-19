use kestrel::{
    checker::{Checked, check},
    parse_file,
};

const COUNTER: &str = include_str!("../testdata/language/mutation.kes");
const OWN: &str =
    "function own(move c : Counter) : void {}\nfunction read(c : Counter) : Int = c.current()\n";
const SHAPES: &str = "class Square {\npublic final side : double\nnew(side : double) { this.side = side }\npublic function area() : double = this.side * this.side\n}\nclass Panel {\npublic final square : &Square\nnew(square : Square) { this.square = square }\npublic function area() : double = this.square.area()\n}\nfunction consume(move s : Square) : void {}\n";

fn checked(source: &str) -> Checked {
    check(&parse_file("ownership.kes", source))
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
fn counter(code: &str) -> String {
    format!("{COUNTER}{OWN}{code}")
}
fn shapes(code: &str) -> String {
    format!("{SHAPES}{code}")
}

#[test]
fn binding_or_passing_to_move_hands_a_class_value_over() {
    refuses(
        &counter("let a := Counter()\nlet b := a\na.current()"),
        "E_USE_AFTER_MOVE",
    );
    refuses(
        &counter("let c := Counter()\nown(c)\nc.current()"),
        "E_USE_AFTER_MOVE",
    );
    refuses(
        &counter("let c := Counter()\nc |> own\nread(c)"),
        "E_USE_AFTER_MOVE",
    );
    refuses(
        &counter("let c := Counter()\nown(c)\nown(c)"),
        "E_USE_AFTER_MOVE",
    );
    good(&counter("let c := Counter()\nread(c)\nread(c)\nown(c)"));
    good(&counter("let a := Counter()\nlet b := a\nb.current()"));
}

#[test]
fn arrow_move_inputs_consume_the_argument_and_refuse_a_borrowed_payload() {
    good(
        "class Payload {public final label : String\nnew(label : String) {this.label=label}}\nalias Consume = (move value : Payload) -> String\nfunction take(move p : Payload) : String=p.label\nlet f : Consume=(move actual : Payload) => take(actual)\nf(value=Payload(\"owned\"))",
    );
    let header = "class Payload {public final label : String\nnew(label : String) {this.label=label}}\nalias Consume = (move value : Payload) -> String\n";
    good(&format!(
        "{header}function good(f : Consume,move p : Payload) : String=f(value=p)"
    ));
    refuses(
        &format!("{header}function bad(f : Consume,p : Payload) : String=f(value=p)"),
        "E_MOVE_BORROWED",
    );
    refuses(
        &format!(
            "{header}function bad(f : Consume) : String {{let p:=Payload(\"heap\"+\" value\")\nf(value=p)\np.label}}"
        ),
        "E_USE_AFTER_MOVE",
    );
}

#[test]
fn written_mutable_callback_types_move_without_hiding_borrowed_captures() {
    let header = "alias Counter = mut (() -> Int)\nfunction make() : Counter {let mut n:=0\nmove () => {n=n+1\nn}}\nfunction take(move f : Counter) : Int=f()\n";
    good(&format!(
        "{header}let f:=make()\nlet g:=f\ng()\ng()\ntake(g)"
    ));
    refuses(
        &format!("{header}let f:=make()\nlet g:=f\nf()"),
        "E_USE_AFTER_MOVE",
    );
    refuses(
        &format!("{header}let f:=make()\ntake(f)\nf()"),
        "E_USE_AFTER_MOVE",
    );
    refuses(
        "function bad() : mut (() -> Int) {let mut n:=0\n() => {n=n+1\nn}}",
        "E_LAMBDA_ESCAPE",
    );
    refuses(
        "function take(move f : mut (() -> Int)) : Int=f()\nfunction bad() : Int {let mut n:=0\ntake(() => {n=n+1\nn})}",
        "E_LAMBDA_ESCAPE",
    );
}

#[test]
fn borrowed_parameters_cannot_be_handed_over() {
    refuses(
        &counter("function pass(c : Counter) : void = own(c)"),
        "E_MOVE_BORROWED",
    );
    refuses(
        &counter("function pass(mut c : Counter) : void = own(c)"),
        "E_MOVE_BORROWED",
    );
    good(&counter("function pass(move c : Counter) : void = own(c)"));
    refuses(
        &counter("class Holder { var c : Counter\nnew(c : Counter) { this.c = c } }"),
        "E_MOVE_BORROWED",
    );
    good(&counter(
        "class Holder { var c : Counter\nnew(move c : Counter) { this.c = c } }",
    ));
    refuses(
        &counter("enum Slot { Full(Counter)\nEmpty }\nfunction wrap(c : Counter) : Slot = Full(c)"),
        "E_MOVE_BORROWED",
    );
    refuses(
        &counter("function wrap(c : Counter) : List[Counter] = [c]"),
        "E_MOVE_BORROWED",
    );
}

#[test]
fn strings_and_persistent_collections_copy_like_int() {
    good(
        "let key := \"score\"\nlet m := Map(key => 10)\nlet value : Option[Int] = m.get(key)\nlet again := key",
    );
    good(
        "enum ParseError extends Error { Parse(String) }\nfunction parse(s : String) : Int or ParseError { if s == \"\" { return Parse(s) }\n42 }\nlet text := \"\"\nlet n := parse(text)\nlet same := text",
    );
    good("class Name { final value : String\nnew(value : String) { this.value = value } }");
    good(
        "let xs := Set(1, 2)\nlet ys := xs\nlet zs := xs.add(3)\nlet list := [\"a\"]\nlet same := [list, list]",
    );
    good(
        "function first(m : Map[String, String]) : String = match m.get(\"a\") { Some(v) => v, None => \"\", }",
    );
}

#[test]
fn branches_and_reassignment_change_what_is_moved() {
    refuses(
        &counter(
            "function f(flag : Bool) : Int { let c := Counter()\nif flag { own(c) }\nc.current() }",
        ),
        "E_USE_AFTER_MOVE",
    );
    good(&counter(
        "function f(flag : Bool) : Int { let c := Counter()\nif flag { own(c)\nreturn 0 }\nc.current() }",
    ));
    good(&counter(
        "function f(flag : Bool) : void { let c := Counter()\nif flag { own(c) } else { own(c) } }",
    ));
    good(&counter(
        "function f() : Int { let mut c := Counter()\nown(c)\nc = Counter()\nc.current() }",
    ));
    refuses(
        &counter(
            "function f(flag : Bool) : Int { let c := Counter()\nlet n := match flag { true => { own(c)\n1 }, false => 0, }\nc.current() }",
        ),
        "E_USE_AFTER_MOVE",
    );
}

#[test]
fn loops_cannot_hand_over_the_same_value_twice() {
    refuses(
        &counter("function f() : void { let c := Counter()\nfor (x in [1, 2]) { own(c) } }"),
        "E_USE_AFTER_MOVE",
    );
    refuses(
        &counter("function f(go : Bool) : void { let c := Counter()\nwhile go { own(c) } }"),
        "E_USE_AFTER_MOVE",
    );
    good(&counter(
        "function f() : void { let mut c := Counter()\nfor (x in [1, 2]) { own(c)\nc = Counter() } }",
    ));
    good(&counter(
        "function f() : void { let c := Counter()\nfor (x in [1, 2]) { own(c)\nbreak } }",
    ));
    good(&counter(
        "function f() : void { for (x in [1, 2]) { let c := Counter()\nown(c) } }",
    ));
}

#[test]
fn fields_and_pattern_parts_move_only_when_handed_over() {
    let pair = "class Pair { public var left : Counter\npublic var right : Counter\nnew() { this.left = Counter()\nthis.right = Counter() } }\n";
    good(&counter(&format!(
        "{pair}let pair := Pair()\nlet l := pair.left\nread(l)\nread(pair.left)"
    )));
    refuses(
        &counter(&format!(
            "{pair}let pair := Pair()\nlet l := pair.left\nown(l)\nread(pair.left)"
        )),
        "E_USE_AFTER_MOVE",
    );
    good(&counter(&format!(
        "{pair}let pair := Pair()\nlet l := pair.left\nown(l)\nread(pair.right)"
    )));
    refuses(
        &counter(
            "enum Slot { Full(Counter)\nEmpty }\nfunction f() : Int { let slot := Full(Counter())\nmatch slot { Full(c) => own(c), Empty => {}, }\nmatch slot { Full(c) => c.current(), Empty => 0, } }",
        ),
        "E_USE_AFTER_MOVE",
    );
    refuses(
        &counter("function f(xs : List[Counter]) : void { for (c in xs) { own(c) } }"),
        "E_MOVE_BORROWED",
    );
}

#[test]
fn panel_borrows_a_square_that_outlives_it() {
    good(include_str!("../testdata/language/panel.kes"));
    good(&shapes(
        "function total(s : Square) : double { let p := Panel(s)\np.area() }",
    ));
    good(&shapes("let p := Panel(Square(2.0))\np.area()"));
    refuses(
        &shapes("let sq := Square(2.0)\nlet p := Panel(sq)\nconsume(sq)\np.area()"),
        "E_BORROW_CONFLICT",
    );
    good(&shapes(
        "let sq := Square(2.0)\nlet p := Panel(sq)\np.area()\nconsume(sq)",
    ));
    refuses(
        &shapes("function make() : Panel { let sq := Square(2.0)\nPanel(sq) }"),
        "E_BORROW_ESCAPE",
    );
    refuses(
        &shapes("function make() : Panel = Panel(Square(2.0))"),
        "E_BORROW_TEMPORARY",
    );
    good(&shapes("function make(s : Square) : Panel = Panel(s)"));
}

#[test]
fn stored_borrows_must_come_from_the_constructor_caller() {
    refuses(
        &shapes(
            "class Frame { public var square : &Square\nnew(s : Square) { this.square = s }\npublic mut function attach(s : Square) : void { this.square = s } }",
        ),
        "E_BORROW_ESCAPE",
    );
    refuses(
        &shapes(
            "class Frame { public final square : &Square\nnew() { let s := Square(1.0)\nthis.square = s } }",
        ),
        "E_BORROW_ESCAPE",
    );
}

#[test]
fn returned_borrows_stay_tied_to_their_arguments() {
    let biggest = "function biggest(a : Square, b : Square) : Square = if a.area() > b.area() then a else b\n";
    good(&shapes(&format!(
        "{biggest}let s := Square(1.0)\nlet t := Square(2.0)\nlet big := biggest(s, t)\nbig.area()"
    )));
    refuses(
        &shapes(&format!(
            "{biggest}let s := Square(1.0)\nlet t := Square(2.0)\nlet big := biggest(s, t)\nconsume(s)\nbig.area()"
        )),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &shapes(&format!(
            "{biggest}function make() : Square {{ let s := Square(1.0)\nlet t := Square(2.0)\nbiggest(s, t) }}"
        )),
        "E_BORROW_ESCAPE",
    );
    good(&shapes(&format!(
        "{biggest}function pick(a : Square, b : Square) : Square = biggest(b, a)"
    )));
    good(&shapes(
        "function make() : Square { let s := Square(1.0)\ns }",
    ));
    good(&shapes("function side(s : Square) : double = s.side"));
}

fn borrowing_chain(length: usize, reverse: bool) -> String {
    let mut functions = vec!["function id0(s : Square) : Square = s\n".to_owned()];
    functions.extend(
        (1..length).map(|i| format!("function id{i}(s : Square) : Square = id{}(s)\n", i - 1)),
    );
    if reverse {
        functions.reverse();
    }
    functions.concat()
}

#[test]
fn long_call_chains_reach_complete_borrow_summaries_in_both_source_orders() {
    for reverse in [false, true] {
        let chain = borrowing_chain(24, reverse);
        good(&shapes(&format!(
            "{chain}let s := Square(1.0)\nlet view := id23(s)\nview.area()"
        )));
        refuses(
            &shapes(&format!(
                "{chain}function bad() : Square {{let s := Square(1.0)\nid23(s)}}"
            )),
            "E_BORROW_ESCAPE",
        );
        refuses(
            &shapes(&format!(
                "{chain}let s := Square(1.0)\nlet view := id23(s)\nconsume(s)\nview.area()"
            )),
            "E_BORROW_CONFLICT",
        );
    }
}

#[test]
fn unfinished_borrow_analysis_fails_instead_of_exporting_partial_facts() {
    refuses(&shapes(&borrowing_chain(300, true)), "E_BORROW_LIMIT");
}

#[test]
fn deeply_nested_loops_cannot_move_the_same_value_again() {
    let mut outer = "function nested() : void {\n".to_owned();
    for i in 0..4 {
        outer.push_str(&format!(
            "for (let i{i} : Int = 0; i{i} < 2; i{i} = i{i} + 1) {{\n"
        ));
    }
    refuses(
        &shapes(&format!(
            "{outer}let s := Square(1.0)\nfor (let j : Int = 0; j < 2; j = j + 1) {{consume(s)}}\n{}",
            "}\n".repeat(5)
        )),
        "E_USE_AFTER_MOVE",
    );
    good(&shapes(&format!(
        "{outer}for (let j : Int = 0; j < 2; j = j + 1) {{let s := Square(1.0)\nconsume(s)}}\n{}",
        "}\n".repeat(5)
    )));
}

#[test]
fn loop_carried_borrows_reach_all_possible_owners() {
    let mut source = "function borrow(s : Square) : Square = s\n".to_owned();
    for i in 0..8 {
        source.push_str(&format!(
            "let p{i} := Square(1.0)\nlet mut a{i} := borrow(p{i})\n"
        ));
    }
    source.push_str("for (let i : Int = 0; i < 8; i = i + 1) {\n");
    for i in 0..7 {
        source.push_str(&format!("a{i}=a{}\n", i + 1));
    }
    source.push_str("}\n");
    good(&shapes(&format!("{source}a0.area()")));
    refuses(
        &shapes(&format!("{source}consume(p7)\na0.area()")),
        "E_BORROW_CONFLICT",
    );
}

#[test]
fn deep_loops_without_changing_ownership_stabilize_without_replay() {
    let mut source = "function nested() : void {\n".to_owned();
    for i in 0..18 {
        source.push_str(&format!(
            "for (let i{i} : Int = 0; i{i} < 2; i{i} = i{i} + 1) {{\n"
        ));
    }
    source.push_str("let n := 1\n");
    source.push_str(&"}\n".repeat(19));
    good(&source);
}

#[test]
fn recursive_parameter_permutations_keep_every_possible_owner_borrowed() {
    let source = "function rotate(a : Square,b : Square,c : Square,again : Bool) : Square = if again then rotate(b,c,a,false) else a\n";
    for owner in ["a", "b", "c"] {
        refuses(
            &shapes(&format!(
                "{source}let a := Square(1.0)\nlet b := Square(2.0)\nlet c := Square(3.0)\nlet view := rotate(a,b,c,true)\nconsume({owner})\nview.area()"
            )),
            "E_BORROW_CONFLICT",
        );
    }
}

#[test]
fn borrowing_lambdas_cannot_escape_without_move() {
    refuses(
        "function adder(n : Int) : Int -> Int = x => x + n",
        "E_LAMBDA_ESCAPE",
    );
    good("function adder(n : Int) : Int -> Int = move x => x + n");
    good("function double() : Int -> Int = x => x * 2");
    refuses(
        "function adder(n : Int) : Int -> Int { let add := x => x + n\nadd }",
        "E_LAMBDA_ESCAPE",
    );
    good("function apply(n : Int) : Int { let add := x => x + n\nadd(1) }");
    refuses(
        &counter("let c := Counter()\nlet f := move () => c.current()\nc.current()"),
        "E_USE_AFTER_MOVE",
    );
    good(&counter(
        "let c := Counter()\nlet f := () => c.current()\nf()\nc.current()",
    ));
    refuses(
        &counter("let c := Counter()\nlet f := () => own(c)"),
        "E_MOVE_BORROWED",
    );
    refuses(
        &counter("let c := Counter()\nlet f := () => c.current()\nown(c)\nf()"),
        "E_BORROW_CONFLICT",
    );
}

#[test]
fn changing_an_owner_while_a_borrow_is_used_is_refused() {
    let grow = "class Tile { public var side : double\nnew(side : double) { this.side = side }\npublic mut function grow() : void { this.side = this.side * 2.0 }\npublic function area() : double = this.side * this.side }\nclass View { public var tile : &Tile\nnew(tile : Tile) { this.tile = tile }\npublic function area() : double = this.tile.area() }\n";
    refuses(
        &format!("{grow}let t := Tile(1.0)\nlet v := View(t)\nt.grow()\nv.area()"),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &format!("{grow}let t := Tile(1.0)\nlet v := View(t)\nt.side = 3.0\nv.area()"),
        "E_BORROW_CONFLICT",
    );
    good(&format!(
        "{grow}let t := Tile(1.0)\nlet v := View(t)\nv.area()\nt.grow()"
    ));
}

const TILE: &str = "class Tile { public var side : double\nnew(side : double) { this.side = side }\npublic mut function grow() : void { this.side = this.side * 2.0 }\npublic function area() : double = this.side * this.side }\nclass View { public var tile : &Tile\nnew(tile : Tile) { this.tile = tile }\npublic function area() : double = this.tile.area() }\n";

#[test]
fn borrow_conflicts_follow_loops() {
    refuses(
        &format!(
            "{TILE}let t := Tile(1.0)\nlet v := View(t)\nfor (i in [1, 2]) {{ v.area()\nt.grow() }}"
        ),
        "E_BORROW_CONFLICT",
    );
    good(&format!(
        "{TILE}let t := Tile(1.0)\nfor (i in [1, 2]) {{ let v := View(t)\nv.area()\nt.grow() }}"
    ));
}

#[test]
fn generic_receivers_and_function_values_may_return_borrows() {
    let holder = "trait Holder { function get() : Square }\nclass Keeper { has Holder\npublic final square : &Square\nnew(square : Square) { this.square = square }\npublic function get() : Square = this.square }\n";
    good(&shapes(&format!(
        "{holder}function pick[T : Holder](h : T) : Square = h.get()"
    )));
    refuses(
        &shapes(&format!(
            "{holder}function pick[T : Holder](h : T) : Square = h.get()\nfunction make() : Square {{ let s := Square(1.0)\nlet k := Keeper(s)\npick(k) }}"
        )),
        "E_BORROW_ESCAPE",
    );
    refuses(
        &shapes(
            "function apply(f : Square -> Square, s : Square) : Square = f(s)\nfunction make() : Square { let s := Square(1.0)\napply(x => x, s) }",
        ),
        "E_BORROW_ESCAPE",
    );
}

#[test]
fn lazy_sequences_keep_the_lambdas_they_were_given() {
    refuses(
        "let make := (n : Int) => [1, 2].map(x => x * n)",
        "E_LAMBDA_ESCAPE",
    );
    good("let make := (n : Int) => [1, 2].map(x => x * n).toList()");
}

#[test]
fn values_holding_borrows_cannot_be_handed_over() {
    refuses(
        &shapes(
            "function keep(move p : Panel) : void {}\nlet s := Square(1.0)\nlet p := Panel(s)\nkeep(p)",
        ),
        "E_MOVE_BORROWED",
    );
}

#[test]
fn mixed_subclass_results_keep_each_possible_owner_borrowed() {
    let source = "class Figure {public var n : Int\nnew() {this.n=0}}\nclass Square extends Figure {new() {this.n=1}}\nfunction choose(a : Figure,b : Figure,flag : Bool) : Figure = if flag then a else b\nfunction consume(move s : Square) : void {}\n";
    refuses(
        &format!(
            "{source}let square:=Square()\nlet base:=Figure()\nlet view:=choose(square,base,true)\nconsume(square)\nview.n"
        ),
        "E_BORROW_CONFLICT",
    );
    good(&format!(
        "{source}let square:=Square()\nlet base:=Figure()\nlet view:=choose(square,base,true)\nview.n\nconsume(square)"
    ));
    refuses(
        &format!(
            "{source}function escape(base : Figure) : Figure {{let square:=Square()\nchoose(square,base,true)}}"
        ),
        "E_BORROW_ESCAPE",
    );
}

#[test]
fn omitted_default_arguments_keep_temporary_borrows_visible() {
    let source = "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction identity(p : Point = Point(\"fresh\")) : Point = p\n";
    refuses(
        &format!("{source}function make() : Point = identity()"),
        "E_BORROW_TEMPORARY",
    );
    good(&format!(
        "{source}let p:=identity()\np.label\nfunction text() : String = identity().label"
    ));
    let list = "function first(values : List[Point] = [Point(\"item\")]) : Point = match values { [item, ..rest] => item, [] => Point(\"empty\"), }\n";
    refuses(
        &format!("{source}{list}function make() : Point = first()"),
        "E_BORROW_TEMPORARY",
    );
    good(&format!(
        "{source}function own(move p : Point = Point(\"owned\")) : Point = p\nfunction make() : Point = own()"
    ));
    refuses(
        &format!(
            "{source}class Panel {{public final point : &Point\nnew(point : Point = Point(\"view\")) {{this.point=point}}}}\nfunction make() : Panel = Panel()"
        ),
        "E_BORROW_TEMPORARY",
    );
}

#[test]
fn defaults_borrowing_script_values_keep_their_actual_owner() {
    let source = "class Point {public final label : String\nnew(label : String) {this.label=label}}\nlet point:=Point(\"global\")\nfunction identity(p : Point = point) : Point = p\nfunction make() : Point = identity()\nfunction consume(move p : Point) : void {}\n";
    good(&format!(
        "{source}let view:=make()\nview.label\nconsume(point)"
    ));
    refuses(
        &format!("{source}let view:=make()\nconsume(point)\nview.label"),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &format!("{source}function take(move p : Point = point) : void {{}}\ntake()"),
        "E_MOVE_BORROWED",
    );
    let getter =
        "function get() : Point = point\nfunction indirect(p : Point = get()) : Point = p\n";
    good(&format!(
        "{source}{getter}function view() : Point = indirect()\nview().label"
    ));
    refuses(
        &format!("{source}{getter}function take(move p : Point = get()) : void {{}}\ntake()"),
        "E_MOVE_BORROWED",
    );
}

#[test]
fn recursive_default_expansion_has_a_bounded_refusal() {
    refuses(
        "function recursive(n : Int = recursive()) : Int = n\nrecursive()",
        "E_DEFAULT_RECURSION",
    );
    refuses(
        "function a(n : Int = b()) : Int = n\nfunction b(n : Int = a()) : Int = n\na()",
        "E_DEFAULT_RECURSION",
    );
    good("function step(n : Int = step(1)) : Int = n + 1\nstep()");
}

#[test]
fn borrows_cannot_leave_inner_blocks_or_a_guard() {
    refuses(
        &shapes(
            "function same(s : Square) : Square = s\nlet view := { let owner := Square(2.0)\nsame(owner) }\nview.area()",
        ),
        "E_BORROW_ESCAPE",
    );
    good(&shapes(
        "let owner := { let local := Square(2.0)\nlocal }\nowner.area()",
    ));
    refuses(
        &shapes("let panel := { let owner := Square(2.0)\nPanel(owner) }\npanel.area()"),
        "E_BORROW_ESCAPE",
    );
    refuses(
        &shapes(
            "function get(m : Mutex[Square]) : Square or Error with Effects { using g := m.lock()? { g.value } }",
        ),
        "E_BORROW_ESCAPE",
    );
}
#[test]
fn persistent_map_copy_handles_still_keep_payload_borrows_tied_to_storage() {
    refuses(
        &shapes(
            "function bad() : Option[&Square] { let map := Map(\"a\" => Square(2.0))\nmap.get(\"a\") }",
        ),
        "E_BORROW_ESCAPE",
    );
    refuses(
        &shapes(
            "let mut map := Map(\"a\" => Square(2.0))\nlet item := map.get(\"a\")\nmap = Map(\"b\" => Square(3.0))\nmatch item { Some(s) => s.area(), None => 0.0, }",
        ),
        "E_BORROW_CONFLICT",
    );
    good(&shapes(
        "function get(map : Map[String, Square]) : Option[&Square] = map.get(\"a\")\nlet map := Map(\"a\" => Square(2.0))\nlet item := get(map)\nmatch item { Some(s) => s.area(), None => 0.0, }",
    ));
}

#[test]
fn read_only_collection_items_do_not_become_owned_return_values() {
    refuses(
        &shapes(
            "function bad() : Square {let local := [Square(2.0)]\nmatch local {[square] => square,_ => Square(0.0),}}",
        ),
        "E_BORROW_ESCAPE",
    );
    good(&shapes(
        "function first(values : List[Square]) : Square = match values {[square] => square,_ => Square(0.0),}\nlet values := [Square(2.0)]\nlet view := first(values)\nview.area()",
    ));
}
#[test]
fn named_functions_check_captured_owners_at_call_sites() {
    refuses(
        &shapes(
            "let square := Square(2.0)\nfunction area() : double = square.area()\nconsume(square)\narea()",
        ),
        "E_USE_AFTER_MOVE",
    );
    refuses(
        &shapes(
            "let square := Square(2.0)\nfunction get() : Square = square\nlet view := get()\nconsume(square)\nview.area()",
        ),
        "E_BORROW_CONFLICT",
    );
    for owner in ["left", "right"] {
        refuses(
            &shapes(&format!(
                "let left := Square(1.0)\nlet right := Square(2.0)\nfunction first(stop : Bool) : double = if stop then left.area() else second(true)\nfunction second(stop : Bool) : double = if stop then right.area() else first(true)\nconsume({owner})\nfirst(false)\nsecond(false)"
            )),
            "E_USE_AFTER_MOVE",
        );
    }
}
#[test]
fn task_borrows_cannot_outlive_their_inner_owner_block() {
    refuses(
        &counter(
            "function run() : void with Effects { scope { if true { let local := Counter()\nspawn read(local) } } }",
        ),
        "E_SPAWN_RESULT",
    );
    refuses(
        &counter(
            "function inspect(c : Counter) : void {}\nfunction run() : void { scope { if true { let local := Counter()\nspawn inspect(local) } } }",
        ),
        "E_EFFECT_REQUIRED",
    );
    refuses(
        &counter(
            "function inspect(c : Counter) : void {}\nfunction run() : void with Effects { scope { if true { let local := Counter()\nspawn inspect(local) } } }",
        ),
        "E_TASK_LIFETIME",
    );
}

#[test]
fn pipes_preserve_returned_borrow_lifetimes() {
    refuses(
        &shapes(
            "function same(s : Square) : Square = s\nlet view := { let owner := Square(2.0)\nowner |> same }\nview.area()",
        ),
        "E_BORROW_ESCAPE",
    );
    refuses(
        &shapes(
            "function same(s : Square) : Square = s\nlet owner := Square(2.0)\nlet view := owner |> same\nlet taken := owner\nview.area()",
        ),
        "E_BORROW_CONFLICT",
    );
}
#[test]
fn assigning_an_inner_borrow_to_an_outer_name_is_rejected() {
    refuses(
        &shapes(
            "let first := Square(1.0)\nlet mut panel := Panel(first)\n{ let inner := Square(2.0)\npanel = Panel(inner) }\npanel.area()",
        ),
        "E_BORROW_ESCAPE",
    );
}
#[test]
fn returned_borrows_do_not_grant_mutation_permission() {
    let counter = include_str!("../testdata/language/mutation.kes");
    refuses(
        &format!(
            "{counter}\nfunction view(c : Counter) : Counter = c\nlet c := Counter()\nlet borrowed := view(c)\nborrowed.increment()"
        ),
        "E_MUT_PARAMETER",
    );
    refuses(
        &format!(
            "{counter}\nfunction view(c : Counter) : Counter = c\nlet c := Counter()\nview(c).increment()"
        ),
        "E_MUT_PARAMETER",
    );
}

#[test]
fn persistent_iteration_does_not_detach_class_views_from_the_list_owner() {
    refuses(
        &shapes(
            "function first() : Square { let values := [Square(2.0)]\nfor (value in values) { return value }\nSquare(0.0) }",
        ),
        "E_BORROW_ESCAPE",
    );
    let counter = include_str!("../testdata/language/mutation.kes");
    refuses(
        &format!(
            "{counter}\nlet values := [Counter()]\nfor (value in values) {{ value.increment() }}"
        ),
        "E_MUT_PARAMETER",
    );
}

#[test]
fn results_of_borrowing_closures_keep_the_capture_owner() {
    refuses(
        &shapes(
            "let view := { let owner := Square(2.0)\nlet get := () => owner\nget() }\nview.area()",
        ),
        "E_BORROW_ESCAPE",
    );
    let counter = include_str!("../testdata/language/mutation.kes");
    refuses(
        &format!(
            "{counter}\nlet values := [Counter()]\nmatch values {{ [value] => value.increment(), _ => {{}}, }}"
        ),
        "E_MUT_PARAMETER",
    );
}

#[test]
fn lists_of_trait_object_views_keep_their_owners_borrowed() {
    let shape = "trait Area { function area() : double }\nclass Disk { has Area\nnew() {}\npublic function area() : double = 1.0 }\nfunction consume(move d : Disk) : void {}\n";
    good(&format!(
        "{shape}let d := Disk()\nlet views : List[&dyn Area] = [d]\nfor (v in views) {{ v.area() }}\nd.area()"
    ));
    refuses(
        &format!("{shape}let views : List[&dyn Area] = {{ let d := Disk()\n[d] }}\nviews"),
        "E_BORROW_ESCAPE",
    );
    refuses(
        &format!("{shape}let d := Disk()\nlet views : List[&dyn Area] = [d]\nconsume(d)\nviews"),
        "E_BORROW_CONFLICT",
    );
}

#[test]
fn method_values_borrow_their_receiver() {
    refuses(
        "class Counter {\n    var count : Int\n    new(count : Int) { this.count = count }\n    public function scaled(by : Int) : Int = this.count * by\n}\nfunction make() : Int -> Int {\n    let c := Counter(3)\n    c.scaled\n}",
        "E_LAMBDA_ESCAPE",
    );
    good(
        "class Counter {\n    var count : Int\n    new(count : Int) { this.count = count }\n    public function scaled(by : Int) : Int = this.count * by\n}\nfunction use(c : Counter) : Int {\n    let f := c.scaled\n    f(2)\n}",
    );
}

#[test]
fn owned_array_views_prevent_overwrite_escape_and_transfer() {
    let header = "class Point {public final x : Int\nnew(x : Int) {this.x=x}}\n";
    refuses(
        &format!(
            "{header}let xs := Array.generate(2, i => Point(i))\nlet p := xs[0]\nxs[0] = Point(9)\nprint(p.x)"
        ),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &format!(
            "{header}function bad() : Point {{let xs := Array.generate(2, i => Point(i))\nxs[0]}}"
        ),
        "E_BORROW_ESCAPE",
    );
    refuses(
        &format!(
            "{header}function take(move p : Point) : Int = p.x\nlet xs := Array.generate(2, i => Point(i))\nprint(take(xs[0]))"
        ),
        "E_MOVE_BORROWED",
    );
}

/// Q67: a dynamic traversal lends each class item only until its next step.
const STEP_POINT: &str = "class Point {\npublic final label : String\npublic var n : Int\nnew(label : String) {this.label=label\nthis.n=0}\npublic mut function bump() : void {this.n=this.n+1}\n}\nfunction consume(move p : Point) : String = p.label\n";

fn step(code: &str) -> String {
    format!("{STEP_POINT}{code}")
}

#[test]
fn dynamic_loop_items_are_read_only_and_cannot_outlive_their_step() {
    let header = "function f(mut values : &dyn Iterable(Item = Point))";
    refuses(
        &step(&format!(
            "{header} : void with Effects {{ for (p in values) {{ p.bump() }} }}"
        )),
        "E_MUT_PARAMETER",
    );
    refuses(
        &step(&format!(
            "{header} : String with Effects {{ for (p in values) {{ return consume(p) }}\n\"none\" }}"
        )),
        "E_MOVE_BORROWED",
    );
    refuses(
        &step(&format!(
            "{header} : Point with Effects {{ for (p in values) {{ return p }}\nPoint(\"none\") }}"
        )),
        "E_BORROW_ESCAPE",
    );
    refuses(
        &step(&format!(
            "{header} : String with Effects {{ let mut kept := Point(\"x\")\nfor (p in values) {{ kept = p }}\nkept.label }}"
        )),
        "E_BORROW_ESCAPE",
    );
    refuses(
        &step(&format!(
            "{header} : String with Effects {{ let mut get := () => \"x\"\nfor (p in values) {{ get = () => p.label }}\nget() }}"
        )),
        "E_BORROW_ESCAPE",
    );
    refuses(
        &step(
            "function f[T : Iterable(Item = Point)](mut values : T) : Point with Effects { for (p in values) { return p }\nPoint(\"x\") }",
        ),
        "E_BORROW_ESCAPE",
    );
    good(&step(&format!(
        "{header} : List[String] with Effects = [p.label for p in values]"
    )));
    good(&step(&format!(
        "{header} : List[Point] with Effects = values.toList()"
    )));
    good(&step(
        "function f(mut values : &dyn Iterable(Item = Int)) : Int with Effects { for (n in values) { return n }\n0 }",
    ));
    // A concrete List keeps lending its own elements, so they may be returned.
    good(&step(
        "function f(values : List[Point], fallback : Point) : Point { for (p in values) { return p }\nfallback }",
    ));
}

#[test]
fn hidden_cursor_items_end_when_the_cursor_advances() {
    refuses(
        &step(
            "function f(mut values : &dyn Iterable(Item = Point)) : String with Effects { let cursor := values.items()\nlet Some(p) := cursor.next() otherwise { return \"none\" }\nlet Some(q) := cursor.next() otherwise { return \"none\" }\np.label }",
        ),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &step(
            "function f(mut values : &dyn Iterable(Item = Point)) : String with Effects { let cursor := values.items()\nlet first := cursor.next()\nlet second := cursor.next()\nmatch first { Some(p) => p.label, None => \"none\", } }",
        ),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &step(
            "function f(mut c : &dyn Iterator(Item = Point)) : String with Effects { let Some(p) := c.next() otherwise { return \"none\" }\nlet Some(q) := c.next() otherwise { return \"none\" }\np.label }",
        ),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &step(
            "function f(mut c : &dyn Iterator(Item = Point)) : Point with Effects { let Some(p) := c.next() otherwise { return Point(\"none\") }\np }",
        ),
        "E_BORROW_ESCAPE",
    );
    good(&step(
        "function f(mut values : &dyn Iterable(Item = Point)) : List[String] with Effects { let cursor := values.items()\nlet Some(p) := cursor.next() otherwise { return [] }\nlet label := p.label\nlet Some(q) := cursor.next() otherwise { return [label] }\n[label, q.label] }",
    ));
    good(&step(
        "function f(mut c : &dyn Iterator(Item = Point)) : Int with Effects { let mut n := 0\nwhile true { let Some(p) := c.next() otherwise { break }\nn = n + p.n }\nn }",
    ));
}

#[test]
fn mutable_field_aliases_keep_their_earlier_and_branch_owners() {
    let holder = "class Holder {public final square : Square\nnew(move square : Square) {this.square=square}}\nfunction take(move h : Holder) : void {}\n";
    for owner in ["left", "right"] {
        refuses(
            &shapes(&format!(
                "{holder}let left := Holder(Square(1.0))\nlet right := Holder(Square(2.0))\nlet mut view := left.square\nif false {{view=right.square}}\ntake({owner})\nview.area()"
            )),
            "E_BORROW_CONFLICT",
        );
    }
    refuses(
        &shapes(&format!(
            "{holder}let left := Holder(Square(1.0))\nlet right := Holder(Square(2.0))\nlet mut view := left.square\nlet old := view\nview=right.square\ntake(left)\nold.area()"
        )),
        "E_BORROW_CONFLICT",
    );
}

#[test]
fn mutations_through_a_changing_alias_protect_other_borrows() {
    let pair = "class Pair {public var left : Counter\npublic var right : Counter\nnew() {this.left=Counter()\nthis.right=Counter()}}\nlet pair := Pair()\nlet mut view := pair.left\nif false {view=pair.right}\n";
    refuses(
        &counter(&format!(
            "{pair}let other := pair.left\nview.increment()\nread(other)"
        )),
        "E_BORROW_CONFLICT",
    );
    good(&counter(&format!("{pair}view.increment()\nread(view)")));
}

#[test]
fn mutable_parameters_keep_their_field_and_returned_views_borrowed() {
    let pair = "class Pair {public var left : Counter\nnew() {this.left=Counter()}\npublic function get() : Counter = this.left}\n";
    for body in [
        "let view := pair.left\npair.left=Counter()\nread(view)",
        "let view := pair.get()\npair.left=Counter()\nread(view)",
        "let mut view := pair.left\nlet other := pair.left\nview.increment()\nread(other)",
    ] {
        refuses(
            &counter(&format!(
                "{pair}function bad(mut pair : Pair) : Int {{{body}}}"
            )),
            "E_BORROW_CONFLICT",
        );
    }
    good(&counter(&format!(
        "{pair}function good(mut pair : Pair) : Int {{let view := pair.get()\nread(view)\npair.left=Counter()\nread(pair.left)}}"
    )));
}

#[test]
fn tasks_check_the_possible_owners_of_changing_aliases() {
    let pair = "class Pair {public var left : Counter\npublic var right : Counter\nnew() {this.left=Counter()\nthis.right=Counter()}}\nfunction change(mut c : Counter) : void = c.increment()\nfunction inspect(c : Counter) : void {let n := read(c)}\nfunction takePair(move pair : Pair) : void {}\nlet pair := Pair()\nlet mut view := pair.left\nif false {view=pair.right}\n";
    for body in [
        "spawn change(view)\nspawn inspect(pair.left)",
        "spawn change(view)\nread(pair.left)",
        "spawn inspect(view)\ntakePair(pair)",
    ] {
        refuses(
            &counter(&format!("{pair}scope {{{body}}}")),
            "E_TASK_CONFLICT",
        );
    }
    good(&counter(
        "class Pair {public var left : Counter\npublic var right : Counter\nnew() {this.left=Counter()\nthis.right=Counter()}}\nfunction change(mut c : Counter) : void = c.increment()\nfunction inspect(c : Counter) : void {let n := read(c)}\nlet pair := Pair()\nlet view := pair.right\nscope {spawn change(view)\nspawn inspect(pair.left)}",
    ));
}

#[test]
fn deeply_nested_destructuring_keeps_the_collection_owner() {
    let depth = 40;
    let mut literal = "Square(2.0)".to_owned();
    for _ in 0..depth {
        literal = format!("[{literal}]");
    }
    let mut lines = vec![format!("let local := {literal}")];
    for i in 1..=depth {
        let previous = if i == 1 {
            "local".to_owned()
        } else {
            format!("a{}", i - 1)
        };
        lines.push(format!(
            "let [a{i}] := {previous} otherwise {{ return Square(0.0) }}"
        ));
    }
    let body = lines.join("\n");
    refuses(
        &shapes(&format!("function bad() : Square {{\n{body}\na{depth}\n}}")),
        "E_BORROW_ESCAPE",
    );
    let read = body.replace("return Square(0.0)", "return 0.0");
    good(&shapes(&format!(
        "function area() : double {{\n{read}\na{depth}.area()\n}}"
    )));
}

#[test]
fn a_value_put_into_a_new_box_is_handed_over() {
    let shapes = "trait Shape {\nfunction area() : double\n}\nclass Square {\nhas Shape\npublic final side : double\nnew(side : double) {this.side = side}\npublic function area() : double = this.side * this.side\n}\nfunction look(s : Box[dyn Shape]) : double = s.area()\n";
    for body in [
        "let sq := Square(4.0)\nlet xs : List[Box[dyn Shape]] = [sq]\nsq.area()",
        "let sq := Square(4.0)\nlook(sq)\nsq.area()",
        "let sq := Square(4.0)\nlet b : Box[dyn Shape] = sq\nsq.area()",
    ] {
        refuses(&format!("{shapes}{body}"), "E_USE_AFTER_MOVE");
    }
    refuses(
        &format!(
            "{shapes}function f(s : Square) : double {{let b : Box[dyn Shape] = s\nb.area()}}"
        ),
        "E_MOVE_BORROWED",
    );
    good(&format!(
        "{shapes}let sq := Square(4.0)\nlet n := sq.area()\nlet xs : List[Box[dyn Shape]] = [sq, Square(1.0)]\nlook(Square(2.0))"
    ));
}

#[test]
fn unicode_combining_names_keep_borrow_and_move_permissions() {
    good(&shapes(
        "function calcul(p : Square) : double {let f : Square -> double = carré́ => carré́.area()\nf(p)}",
    ));
    good(&shapes(
        "let carré́ := Square(2.0)\nlet panneaú := Panel(carré́)\npanneaú.area()",
    ));
    refuses(
        &shapes(
            "let carré́ := Square(2.0)\nlet panneaú := Panel(carré́)\nconsume(carré́)\npanneaú.area()",
        ),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &shapes("let carré́ := Square(2.0)\nconsume(carré́)\ncarré́.area()"),
        "E_USE_AFTER_MOVE",
    );
}

#[test]
fn earlier_copied_default_parameters_do_not_hide_temporary_or_global_loans() {
    good(include_str!("../testdata/language/dependentdefaults.kes"));
    refuses(
        "class Point {new(label : String) {}}\nfunction identity(n : Int, p : Point = Point(n.text())) : Point = p\nfunction bad() : Point = identity(1)",
        "E_BORROW_TEMPORARY",
    );
}

#[test]
fn reusable_partials_cannot_consume_noncopyable_captures() {
    let source = "class Point {new() {}}\nfunction take(move p : Point,n : Int) : Int = n\nfunction identity(move p : Point) : Point = p\nclass Factory {new() {}\npublic function take(move p : Point,n : Int) : Int = n}\nenum Pair {Hold(Point,Int)}\n";
    for code in [
        "let p:=Point()\nlet f:=take(p,_)",
        "let p:=Point()\nlet f:=take(n=_,p=p)",
        "let p:=Point()\nlet f:=take(p |> identity,_)",
        "let f:=take(Point(),_)",
        "let factory:=Factory()\nlet p:=Point()\nlet f:=factory.take(p,_)",
        "let p:=Point()\nlet f:=Pair.Hold(p,_)",
        "function make(move p : Point) : Int -> Int = take(p,_)",
    ] {
        refuses(&format!("{source}{code}"), "E_PARTIAL_MOVE_CAPTURE");
    }
    good(&format!(
        "{source}let f:=take(_,_)\nf(Point(),1)\nf(Point(),2)"
    ));
    good(
        "function take(move value : String,n : Int) : String=value\nlet f:=take(\"a\"+\"b\",_)\nf(1)\nf(2)",
    );
    good(
        "class Point {new() {}}\nfunction take(n : Int,move p : Point=Point()) : Int=n\nlet f:=take(_)\nf(1)\nf(2)",
    );
}

const CALLBACKS: &str = "class Point {public final label : String\nnew(label : String) {this.label=label}\npublic function read(n : Int) : String=this.label}\nfunction keep(f : Int -> String) : Int -> String=f\nfunction relay(f : Int -> String) : Int -> String=keep(f)\n";

#[test]
fn returned_function_parameters_keep_their_capture_owners() {
    for body in [
        "keep((n : Int) => p.label)",
        "relay(p.read)",
        "let f:=relay((n : Int) => p.label)\nf",
    ] {
        refuses(
            &format!(
                "{CALLBACKS}function bad() : Int -> String {{let p:=Point(\"heap\"+\" label\")\n{body}}}"
            ),
            "E_LAMBDA_ESCAPE",
        );
    }
    good(&format!(
        "{CALLBACKS}function good() : Int -> String {{let p:=Point(\"owned\")\nrelay(move (n : Int) => p.label)}}"
    ));
    good(&format!(
        "{CALLBACKS}function local(p : Point) : String {{let f:=relay(p.read)\nf(1)}}"
    ));
    good(
        "function keep[T](f : Int -> T) : Int -> T=f\nfunction make() : Int -> String {let f:=move (n : Int) => n.text()\nkeep(f)}",
    );
}

#[test]
fn stored_callbacks_keep_capture_loans_and_setters_require_owned_captures() {
    let header = format!(
        "{CALLBACKS}class Holder {{public var f : Int -> String\nnew(f : Int -> String) {{this.f=f}}}}\nfunction wrap(f : Int -> String) : Holder=Holder(f)\nfunction unwrap(f : Int -> String) : Int -> String=Holder(f).f\n"
    );
    for body in ["wrap(p.read)", "Holder(p.read)"] {
        refuses(
            &format!("{header}function bad() : Holder {{let p:=Point(\"heap\")\n{body}}}"),
            "E_BORROW_ESCAPE",
        );
    }
    for (body, code) in [
        ("unwrap(p.read)", "E_LAMBDA_ESCAPE"),
        ("let h:=Holder(p.read)\nh.f", "E_BORROW_ESCAPE"),
    ] {
        refuses(
            &format!("{header}function bad() : Int -> String {{let p:=Point(\"heap\")\n{body}}}"),
            code,
        );
    }
    refuses(
        "class Holder {var f : Int -> Int\nnew(move f : Int -> Int) {this.f=f}\npublic mut function set(f : Int -> Int) : void {this.f=f}}",
        "E_MOVE_BORROWED",
    );
    good(
        "class Holder {var f : Int -> Int\nnew(move f : Int -> Int) {this.f=f}\npublic mut function set(move f : Int -> Int) : void {this.f=f}}",
    );
    good(&format!(
        "{header}function read(p : Point) : String {{let h:=wrap(p.read)\nh.f(1)}}"
    ));
}

#[test]
fn callbacks_inside_copied_collections_and_factories_keep_capture_loans() {
    let header = format!(
        "{CALLBACKS}function collect(f : Int -> String) : List[Int -> String]=[1].map(n => keep(f)).toList()\nfunction generate(f : Int -> String) : Array[Int -> String]=Array.generate(2,n => keep(f))\n"
    );
    for (ty, body) in [
        ("List[Int -> String]", "collect(p.read)"),
        ("Array[Int -> String]", "generate(p.read)"),
    ] {
        refuses(
            &format!("{header}function bad() : {ty} {{let p:=Point(\"heap\")\n{body}}}"),
            "E_BORROW_ESCAPE",
        );
    }
    refuses(
        "function wrap(f : Int -> String) : Int -> String=move (n : Int) => f(n)",
        "E_LAMBDA_ESCAPE",
    );
    good("function wrap(move f : Int -> String) : Int -> String=move (n : Int) => f(n)");
    good(&format!(
        "{header}function make() : List[Int -> String] {{let p:=Point(\"owned\")\ncollect(move (n : Int) => p.label)}}"
    ));
    good(
        "function copy(move fs : List[Int -> String]) : List[Int -> String]=fs\nfunction label(n : Int) : String=n.text()\nlet fs:=[label]\nlet again:=copy(fs)",
    );
}

#[test]
fn named_function_values_keep_transitive_script_capture_loans() {
    let header = "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction consume(move p : Point) : void {}\nfunction keep(f : Int -> String) : Int -> String=f\nlet p:=Point(\"heap\"+\" label\")\nfunction read(n : Int) : String=p.label\n";
    for body in [
        "let f:=keep(read)\nconsume(p)\nf(1)",
        "let f:=read\nconsume(p)\nf(1)",
        "let saved:=read\nfunction second(n : Int) : String=saved(n)\nlet f:=keep(second)\nconsume(p)\nf(1)",
        "function factory(n : Int) : Int -> String=read\nlet maker:=factory\nlet f:=maker(1)\nconsume(p)\nf(1)",
    ] {
        refuses(&format!("{header}{body}"), "E_BORROW_CONFLICT");
    }
    refuses(
        &format!("{header}consume(p)\nlet f:=read\nf(1)"),
        "E_USE_AFTER_MOVE",
    );
    good(&format!("{header}let f:=keep(read)\nf(1)\nconsume(p)"));
}

#[test]
fn call_arguments_hold_callback_owners_until_the_call_finishes() {
    let header = "class Point {public final label : String\nnew(label : String) {this.label=label}\npublic function read(n : Int) : String=this.label}\nfunction apply(f : Int -> String,move p : Point) : String=f(1)\nlet p:=Point(\"heap\")\nfunction read(n : Int) : String=p.label\n";
    for call in [
        "apply(read,p)",
        "apply(p.read,p)",
        "apply((n : Int) => p.label,p)",
    ] {
        refuses(&format!("{header}{call}"), "E_BORROW_CONFLICT");
    }
    good(&format!(
        "{header}let other:=Point(\"other\")\napply(read,other)"
    ));
    refuses(
        &format!("{header}function both(a : Point,move b : Point) : String=a.label\nboth(p,p)"),
        "E_BORROW_CONFLICT",
    );
    refuses(
        "class Point {public final label : String\nnew(label : String) {this.label=label}\npublic function eat(move other : Point) : String=this.label}\nlet p:=Point(\"heap\")\np.eat(p)",
        "E_BORROW_CONFLICT",
    );
    refuses(
        &counter("let c:=Counter()\nc.incrementBoth(c)"),
        "E_BORROW_CONFLICT",
    );
}

#[test]
fn foreign_named_callbacks_pin_script_storage() {
    let header = "import python \"builtins\" as py\nclass Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction consume(move p : Point) : void {}\nlet p:=Point(\"heap\")\nfunction read(n : Int) : String=p.label\n";
    for body in [
        "let result : String or Error=py.str(read)\nconsume(p)",
        "let saved:=read\nlet result : String or Error=py.str(saved)\nconsume(p)",
        "function export() : String or Error with Effects=py.str(read)\nlet result:=export()\nconsume(p)",
    ] {
        refuses(&format!("{header}{body}"), "E_BORROW_CONFLICT");
    }
    good(&format!(
        "{header}let result : String or Error=py.str(read)\np.label"
    ));
}

#[test]
fn reusable_move_lambdas_cannot_return_their_noncopyable_captures() {
    let header =
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\n";
    for body in [
        "let p:=Point(\"heap\")\nlet f:=move () => p",
        "function make(move p : Point) : () -> Point=move () => p",
        "enum Held {Value(Point)}\nlet held:=Held.Value(Point(\"heap\"))\nlet f:=move () => held",
        "let p:=Point(\"heap\")\nlet f:=move () => {let result:=p\nresult}",
    ] {
        refuses(&format!("{header}{body}"), "E_LAMBDA_ESCAPE");
    }
    good(&format!(
        "{header}function make() : () -> Point {{let p:=Point(\"heap\")\nmove () => Point(p.label)}}"
    ));
    good("function make() : () -> String {let text:=\"a\"+\"b\"\nmove () => text}");
}

#[test]
fn nested_move_captures_cannot_consume_an_outer_closures_owner() {
    let header =
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\n";
    refuses(
        &format!("{header}let p:=Point(\"heap\")\nlet outer:=move () => move () => p.label"),
        "E_MOVE_BORROWED",
    );
    refuses(
        &format!("{header}let p:=Point(\"heap\")\nlet outer:=() => move () => p.label"),
        "E_MOVE_BORROWED",
    );
    good(&format!(
        "{header}let outer:=() => {{let p:=Point(\"fresh\")\nmove () => p.label}}"
    ));
    good("let text:=\"copied\"+\" value\"\nlet outer:=move () => move () => text");
    refuses(
        &format!(
            "{header}let p:=Point(\"heap\")\nfunction make() : () -> String=move () => p.label"
        ),
        "E_LAMBDA_ESCAPE",
    );
    good(&format!(
        "{header}let p:=Point(\"heap\")\nfunction read() : String {{let f:=move () => p.label\nf()}}\nread()"
    ));
}

#[test]
fn stored_borrows_survive_global_views_and_container_factories() {
    let header = "class Point {public final label : String\nnew(label : String) {this.label=label}}\nclass Panel {public final point : &Point\nnew(point : Point) {this.point=point}}\nfunction consume(move p : Point) : void {}\n";
    refuses(
        &format!(
            "{header}let p:=Point(\"heap\")\nlet panel:=Panel(p)\nfunction point() : Point=panel.point\nlet view:=point()\nconsume(p)\nview.label"
        ),
        "E_BORROW_CONFLICT",
    );
    let array = "function panels(p : Point) : Array[Panel]=Array.generate(2,i => Panel(p))\n";
    refuses(
        &format!(
            "{header}{array}function bad() : Array[Panel] {{let p:=Point(\"heap\")\npanels(p)}}"
        ),
        "E_BORROW_ESCAPE",
    );
    good(&format!(
        "{header}{array}let p:=Point(\"heap\")\nlet values:=panels(p)\nvalues[0].point.label"
    ));
    refuses(
        &format!(
            "{header}function bad() : () -> String {{let p:=Point(\"heap\")\nlet xs : List[&Point]=[p]\nmove () => match xs {{[q,..rest]=>q.label,[]=>\"empty\"}}}}"
        ),
        "E_LAMBDA_ESCAPE",
    );
}

#[test]
fn trait_views_borrow_source_storage_and_cannot_return_copied_parameters() {
    let header = "class Point {has Text\npublic final label : String\nnew(label : String) {this.label=label}\npublic function text() : String=this.label}\nfunction consume(move p : Point) : void {}\n";
    good(&format!(
        "{header}let p:=Point(\"heap\")\nlet view : &dyn Text=p\np.label\nview.text()"
    ));
    refuses(
        &format!("{header}let p:=Point(\"heap\")\nlet view : &dyn Text=p\nconsume(p)\nview.text()"),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &format!(
            "{header}function bad() : &dyn Text {{let p:=Point(\"heap\")\nlet view : &dyn Text=p\nview}}"
        ),
        "E_BORROW_ESCAPE",
    );
    for ty in ["Int", "String", "List[Int]"] {
        refuses(
            &format!("function bad(value : {ty}) : &dyn Text=value"),
            "E_BORROW_ESCAPE",
        );
    }
    refuses(
        "function bad[T : Text](value : T) : &dyn Text=value",
        "E_BORROW_ESCAPE",
    );
    refuses(
        "const VALUE : Int=3\nfunction bad() : &dyn Text=VALUE",
        "E_BORROW_TEMPORARY",
    );
    refuses("let f : Int -> &dyn Text=(n : Int) => n", "E_BORROW_ESCAPE");
    refuses(
        "class View {final value : &dyn Text\nnew(value : Int) {this.value=value}}",
        "E_BORROW_ESCAPE",
    );
    refuses(
        &format!("{header}function bad() : (&dyn Text) or Error=Point(\"temporary\")"),
        "E_BORROW_TEMPORARY",
    );
    refuses(
        "function bad(value : Int) : (&dyn Text) or Error=value",
        "E_BORROW_ESCAPE",
    );
    good(&format!(
        "{header}function view(value : Point) : &dyn Text=value\nlet p:=Point(\"heap\")\nlet v:=view(p)\nv.text()"
    ));
    good(
        "function identity(value : &dyn Text) : &dyn Text=value\nlet text:=\"heap\"+\" text\"\nlet view:=identity(text)\nview.text()\ntext",
    );
}

#[test]
fn named_calls_and_returned_wrappers_do_not_hide_captured_storage() {
    let header = "class Point {public final label : String\nnew(label : String) {this.label=label}}\nclass Panel {public final point : &Point\nnew(point : Point) {this.point=point}}\nfunction consume(move p : Point) : void {}\n";
    refuses(
        &format!("{header}let p:=Point(\"heap\")\nlet factory:=move () => Panel(p)"),
        "E_LAMBDA_ESCAPE",
    );
    refuses(
        &format!(
            "{header}let p:=Point(\"heap\")\nfunction read() : String=p.label\nlet f:=move () => read()\nconsume(p)\nf()"
        ),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &format!(
            "{header}let p:=Point(\"heap\")\nfunction read() : String=p.label\nlet f:=move () => p.label + read()"
        ),
        "E_MOVE_BORROWED",
    );
    refuses(
        "function make(move text : String) : () -> &dyn Text=move () => text",
        "E_LAMBDA_ESCAPE",
    );
}

#[test]
fn bigint_values_follow_owned_moves_and_shared_read_borrows() {
    good(
        "function consume(move value : BigInt) : void {}\nlet number : BigInt=1000000000000000000000\nconsume(number)",
    );
    refuses(
        "function consume(move value : BigInt) : void {}\nlet number : BigInt=1000000000000000000000\nconsume(number)\nprint(number)",
        "E_USE_AFTER_MOVE",
    );
    good(
        "function identity(value : BigInt) : BigInt=value\nlet number : BigInt=1000000000000000000000\nlet view:=identity(number)\nprint(view)\nprint(number)",
    );
}

#[test]
fn if_and_match_arms_that_exclude_each_other_do_not_conflict() {
    good(
        r#"enum Tree[T] {
    Leaf
    Node(T, Tree[T], Tree[T])
}
function insert(move tree : Tree[Int], value : Int) : Tree[Int] = match tree {
    Leaf => Node(value, Leaf, Leaf),
    Node(current, left, right) => if value < current {
        Node(current, insert(left, value), right)
    } else {
        Node(current, left, insert(right, value))
    },
}
"#,
    );
}

#[test]
fn a_return_that_holds_the_move_ends_the_frame_before_later_uses() {
    good(
        r#"enum Tree[T] {
    Leaf
    Node(T, Tree[T], Tree[T])
}
function insert(move tree : Tree[Int], value : Int) : Tree[Int] = match tree {
    Leaf => Node(value, Leaf, Leaf),
    Node(current, left, right) => {
        if value < current {
            return Node(current, insert(left, value), right)
        }
        return Node(current, left, insert(right, value))
    },
}
"#,
    );
}

#[test]
fn a_borrow_used_after_a_change_in_one_branch_still_conflicts() {
    refuses(
        r#"class Square {
    public final side : double
    new(side : double) { this.side = side }
}
let mut squares : Array[Square] = Array.empty()
squares.append(Square(1.0))
let flag := true
let top := squares.first()
if flag {
    squares.append(Square(2.0))
}
match top {
    Some(s) => print(s.side),
    None => print(0.0),
}
"#,
        "E_BORROW_CONFLICT",
    );
    refuses(
        r#"class Square {
    public final side : double
    new(side : double) { this.side = side }
}
let mut squares : Array[Square] = Array.empty()
squares.append(Square(1.0))
let flag := true
let top := squares.first()
if flag {
    squares.append(Square(2.0))
    match top {
        Some(s) => print(s.side),
        None => print(0.0),
    }
}
"#,
        "E_BORROW_CONFLICT",
    );
}

#[test]
fn a_borrow_used_in_the_other_branch_does_not_conflict() {
    good(
        r#"class Square {
    public final side : double
    new(side : double) { this.side = side }
}
let mut squares : Array[Square] = Array.empty()
squares.append(Square(1.0))
let flag := false
let top := squares.first()
if flag {
    squares.append(Square(2.0))
} else {
    match top {
        Some(s) => print(s.side),
        None => print(0.0),
    }
}
"#,
    );
}

#[test]
fn question_mark_on_a_borrowed_option_field_refuses_moving_the_child_out() {
    refuses(
        "class Node {\npublic var left : Option[Node]\nnew(move left : Option[Node]) { this.left = left }\npublic function take() : Option[Node] {\n    let child := this.left?\n    return Some(child)\n}\n}\n",
        "E_MOVE_BORROWED",
    );
}
