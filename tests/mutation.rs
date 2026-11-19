use kestrel::{
    checker::{Checked, check},
    parse_file,
};

const COUNTER: &str = include_str!("../testdata/language/mutation.kes");
fn checked(source: &str) -> Checked {
    check(&parse_file("mutation.kes", source))
}
fn good(source: &str) -> Checked {
    let result = checked(source);
    assert!(result.is_ok(), "{source}\n{:#?}", result.diagnostics);
    result
}
fn refuses(source: &str, code: &str) {
    let result = checked(source);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code),
        "Expected {code}: {source}\n{:#?}",
        result.diagnostics
    );
}

#[test]
fn receiver_argument_and_io_permissions_are_independent() {
    good(COUNTER);
    good(&format!(
        "{COUNTER}let a := Counter()\nlet b := Counter()\na.incrementBoth(b)"
    ));
    good(&format!(
        "{COUNTER}function increment(mut c : Counter) : void = c.increment()"
    ));
    good(&format!(
        "{COUNTER}function report(c : Counter) : void with Effects = c.printSomething()"
    ));
    good(&format!(
        "{COUNTER}function both(mut c : Counter) : void with Effects = c.incrementAndPrint()"
    ));
    good(&format!(
        "{COUNTER}function owned(move c : Counter) : void = c.increment()"
    ));
    refuses(
        &format!("{COUNTER}function missing(c : Counter) : void with Effects = c.increment()"),
        "E_MUT_PARAMETER",
    );
    refuses(
        &format!("{COUNTER}function missing(mut c : Counter) : void = c.printSomething()"),
        "E_EFFECT_REQUIRED",
    );
}

#[test]
fn mut_this_does_not_allow_mutation_of_an_ordinary_argument() {
    refuses(
        &format!(
            "{COUNTER}class Other {{ new() {{}}\npublic mut function change(other : Counter) : void = other.increment() }}"
        ),
        "E_MUT_PARAMETER",
    );
    good(&format!(
        "{COUNTER}class Other {{ new() {{}}\npublic function change(mut other : Counter) : void = other.increment() }}"
    ));
    refuses(
        &format!(
            "{COUNTER}class Other {{ var n : Int\nnew() {{ this.n = 0 }}\npublic function change(mut other : Counter) : void {{ this.n = 1\nother.increment() }} }}"
        ),
        "E_MUT_RECEIVER",
    );
}

#[test]
fn field_assignment_and_method_calls_require_the_same_permission() {
    refuses(
        "class Value { var n : Int\nnew() { this.n = 0 }\nfunction bad() : void { this.n = 1 } }",
        "E_MUT_RECEIVER",
    );
    good(
        "class Value { var n : Int\nnew() { this.n = 0 }\nmut function change() : void { this.n = 1 } }",
    );
    refuses(
        &format!(
            "{COUNTER}function bad(c : Counter) : void {{ let change := c.increment\nchange() }}"
        ),
        "E_MUT_PARAMETER",
    );
    good(&format!(
        "{COUNTER}function good(mut c : Counter) : void {{ let change := c.increment\nchange() }}"
    ));
}

#[test]
fn calls_reject_overlapping_exclusive_receiver_and_arguments() {
    refuses(
        &format!("{COUNTER}let a := Counter()\na.incrementBoth(a)"),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &format!("{COUNTER}let a := Counter()\n(a).incrementBoth(other = (a))"),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &format!(
            "{COUNTER}function both(mut a : Counter, mut b : Counter) : void {{ a.increment()\nb.increment() }}\nlet a := Counter()\nboth(a, a)"
        ),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &format!(
            "{COUNTER}function inspect(mut a : Counter, b : Counter) : Int {{ a.increment()\nb.current() }}\nlet a := Counter()\ninspect(a, a)"
        ),
        "E_BORROW_CONFLICT",
    );
    good(&format!(
        "{COUNTER}function inspect(a : Counter, b : Counter) : Int = a.current() + b.current()\nlet a := Counter()\ninspect(a, a)"
    ));
}

#[test]
fn arrow_argument_modes_keep_mutation_explicit_and_reject_overlapping_borrows() {
    refuses(
        "class Counter {public mut function bump() : Int=1}\nlet f:=(c : Counter) => c.bump()",
        "E_MUT_PARAMETER",
    );
    good(
        "class Counter {new() {}\npublic mut function bump() : Int=1}\nlet f:=(mut c : Counter) => c.bump()\nlet c:=Counter()\nf(c)",
    );
    let header = "class Counter {var n : Int\nnew() {this.n=0}\npublic mut function bump() : Int {this.n=this.n+1\nthis.n}}\nalias Update = (mut value : Counter) -> Int\n";
    good(&format!(
        "{header}function apply(f : Update,mut counter : Counter) : Int=f(value=counter)"
    ));
    refuses(
        &format!("{header}function bad(f : Update,counter : Counter) : Int=f(value=counter)"),
        "E_MUT_PARAMETER",
    );
    refuses(
        &format!(
            "{header}function change(mut actual : Counter) : Int=actual.bump()\nlet f : Counter -> Int=change"
        ),
        "E_TYPE",
    );
    refuses(
        &format!(
            "{header}alias Both = (mut left : Counter,mut right : Counter) -> Int\nfunction bad(f : Both,mut c : Counter) : Int=f(left=c,right=c)"
        ),
        "E_BORROW_CONFLICT",
    );
    good(
        "alias Runner = (mut callback : () -> Int) -> Int\nfunction run(f : Runner,mut callback : () -> Int) : Int=f(callback=callback)",
    );
}

#[test]
fn fields_of_one_object_can_be_borrowed_separately() {
    let pair = "class Pair { public var left : Counter\npublic var right : Counter\nnew() { this.left = Counter()\nthis.right = Counter() } }\n";
    good(&format!(
        "{COUNTER}{pair}let pair := Pair()\npair.left.incrementBoth(pair.right)"
    ));
    refuses(
        &format!("{COUNTER}{pair}let pair := Pair()\npair.left.incrementBoth(pair.left)"),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &format!(
            "{COUNTER}{pair}function inspect(mut pair : Pair, counter : Counter) : Int = counter.current()\nlet pair := Pair()\ninspect(pair, pair.left)"
        ),
        "E_BORROW_CONFLICT",
    );
}

#[test]
fn mut_argument_permissions_survive_aliases_partials_and_pipes() {
    let change = "function change(mut c : Counter) : void = c.increment()\n";
    refuses(
        &format!(
            "{COUNTER}{change}function bad(c : Counter) : void {{ let call := change\ncall(c) }}"
        ),
        "E_MUT_PARAMETER",
    );
    refuses(
        &format!(
            "{COUNTER}{change}function bad(c : Counter) : void {{ let call := change(_)\ncall(c) }}"
        ),
        "E_MUT_PARAMETER",
    );
    refuses(
        &format!("{COUNTER}{change}function bad(c : Counter) : void = c |> change"),
        "E_MUT_PARAMETER",
    );
    good(&format!(
        "{COUNTER}{change}function good(mut c : Counter) : void {{ let call := change(_)\nc |> call }}"
    ));
}

#[test]
fn traits_overrides_and_dynamic_views_keep_receiver_modes() {
    let protocol = "trait Increment { public mut function increment() : void }\nclass Value { has Increment\nnew() {}\npublic mut function increment() : void {} }\n";
    good(&format!(
        "{protocol}function run(mut value : &dyn Increment) : void = value.increment()"
    ));
    good(&format!(
        "{protocol}function run[T : Increment](mut value : T) : void = value.increment()"
    ));
    refuses(
        &format!("{protocol}function run[T : Increment](value : T) : void = value.increment()"),
        "E_MUT_PARAMETER",
    );
    refuses(
        "trait Increment { mut function increment() : void }\nclass Value { has Increment\nfunction increment() : void {} }",
        "E_TRAIT_METHOD",
    );
    refuses(
        "class Parent { public mut function change() : void {} }\nclass Child extends Parent { @override public function change() : void {} }",
        "E_OVERRIDE_RECEIVER",
    );
    good(
        "class Parent { public mut function change() : void {} }\nclass Child extends Parent { @override public mut function change() : void {} }",
    );
}

#[test]
fn mut_modifier_is_reserved_for_instance_methods() {
    refuses(
        "public mut function change() : void {}",
        "E_MUT_METHOD_CONTEXT",
    );
    refuses(
        "class Value { public static mut function change() : void {} }",
        "E_MUT_METHOD_CONTEXT",
    );
    refuses("class Value { mut new() {} }", "E_MUT_METHOD_CONTEXT");
    refuses("mut class Value {}", "E_MUT_METHOD_CONTEXT");
    refuses("mut const VALUE : Int = 1", "E_MUT_METHOD_CONTEXT");
    refuses(
        "trait Value { static mut function change() : void }",
        "E_MUT_METHOD_CONTEXT",
    );
}

#[test]
fn local_mutation_in_lambdas_does_not_become_an_io_effect() {
    good(
        "function run() : Int { let calculate := () => { let mut n := 0\nn = 2\nn }\ncalculate() }",
    );
    good(&format!(
        "{COUNTER}function run() : void {{ let c := Counter()\nlet change := () => c.increment()\nchange() }}"
    ));
    refuses(
        &format!(
            "{COUNTER}function run() : void {{ let c := Counter()\nlet report := () => c.printSomething()\nreport() }}"
        ),
        "E_EFFECT_REQUIRED",
    );
}

#[test]
fn mutation_only_cursor_iteration_requires_no_io_permission() {
    let cursors = "class Cursor { has Iterator\nvar n : Int\nnew() { this.n = 0 }\npublic mut function next() : Option[Int] { this.n = this.n + 1\nNone } }\nclass Numbers { has Iterable\nnew() {}\npublic function items() : Cursor = Cursor() }\n";
    good(&format!(
        "{cursors}function run(numbers : Numbers) : void {{ for (n in numbers) {{}} }}"
    ));
    good(&format!(
        "{cursors}function next(mut cursor : Cursor) : Option[Int] = cursor.next()"
    ));
    refuses(
        &format!(
            "{cursors}function next(cursor : Cursor) : Option[Int] with Effects = cursor.next()"
        ),
        "E_MUT_PARAMETER",
    );
}

#[test]
fn contract_predicates_cannot_mutate_their_receiver_or_arguments() {
    refuses(
        "class Value { new() {}\npublic mut function valid() : Bool = true\npublic mut function change() : void or Error requires this.valid() {} }",
        "E_MUT_RECEIVER",
    );
    refuses(
        &format!(
            "{COUNTER}function valid(mut c : Counter) : Bool {{ c.increment()\ntrue }}\nfunction change(mut c : Counter) : void or Error requires valid(c) {{}}"
        ),
        "E_MUT_PARAMETER",
    );
}

#[test]
fn local_aliases_retain_read_only_or_mutable_access() {
    refuses(
        &format!(
            "{COUNTER}function bad(other : Counter) : void {{ let view := other\nview.increment() }}"
        ),
        "E_MUT_BORROW",
    );
    refuses(
        &format!(
            "{COUNTER}function bad(other : Counter) : void {{ let view : Counter = other\nlet again := view\nagain.increment() }}"
        ),
        "E_MUT_BORROW",
    );
    good(&format!(
        "{COUNTER}function change(mut other : Counter) : void {{ let view := other\nview.increment() }}"
    ));
    refuses(
        "class Value { var n : Int\nnew() { this.n = 0 }\nfunction bad() : void { let view := this\nview.n = 1 } }",
        "E_MUT_BORROW",
    );
    good("function change(n : Int) : Int { let mut local := n\nlocal = 1\nlocal }");
}

#[test]
fn exclusive_conflicts_include_simple_aliases_and_bound_methods() {
    refuses(
        &format!(
            "{COUNTER}function bad(mut c : Counter) : void {{ let view := c\nc.incrementBoth(view) }}"
        ),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &format!("{COUNTER}let c := Counter()\nlet change := c.incrementBoth\nchange(c)"),
        "E_BORROW_CONFLICT",
    );
    refuses(
        &format!("{COUNTER}let c := Counter()\nlet change := c.incrementBoth(_)\nchange(c)"),
        "E_BORROW_CONFLICT",
    );
    good(&format!(
        "{COUNTER}let c := Counter()\nlet other := Counter()\nlet change := c.incrementBoth(_)\nchange(other)"
    ));
}

#[test]
fn borrowed_map_values_use_ordinary_method_access_but_cannot_be_mutated() {
    good(&format!(
        "{COUNTER}function read(m : Map[String, Counter]) : Int = match m.get(\"value\") {{ Some(c) => c.current(), None => 0, }}"
    ));
    refuses(
        &format!(
            "{COUNTER}function change(m : Map[String, Counter]) : void with Effects {{ let Some(c) := m.get(\"value\") otherwise {{ return }}\nc.increment() }}"
        ),
        "E_MUT_BORROW",
    );
    refuses(
        &format!(
            "{COUNTER}function change(m : Map[String, Counter]) : void {{ let Some(c) := m.get(\"value\") otherwise {{ return }}\nlet view : Counter = c\nview.increment() }}"
        ),
        "E_MUT_BORROW",
    );
}

#[test]
fn multiplication_protocols_do_not_mutate_their_operands() {
    refuses(
        "class Value { has Multipliable[Int]\npublic mut function multiply(n : Int) : Int = n }",
        "E_TRAIT_METHOD",
    );
}

#[test]
fn overload_selection_checks_only_the_selected_receiver_mode() {
    let methods = "class Value { new() {}\npublic function choose(flag : Bool) : Int = 1\npublic mut function choose(n : Int) : Int = n }\n";
    good(&format!(
        "{methods}function read(value : Value) : Int = value.choose(true)"
    ));
    good(&format!(
        "{methods}function read(value : Value) : Int = true |> value.choose"
    ));
    refuses(
        &format!("{methods}function change(value : Value) : Int = value.choose(1)"),
        "E_MUT_PARAMETER",
    );
    good(&format!(
        "{methods}function change(mut value : Value) : Int = value.choose(1)"
    ));
}

#[test]
fn closures_that_change_their_captures_move_and_need_mut_parameters() {
    let counter = "class Tally { var n : Int\nnew() { this.n = 0 }\npublic mut function increment() : Int { this.n = this.n + 1\nthis.n } }\n";
    refuses(
        "let mut n := 0\nlet bump := () => { n = n + 1 }\nfunction twice(f : () -> void) : void { f()\nf() }\ntwice(bump)",
        "E_MUT_PARAMETER",
    );
    good(
        "let mut n := 0\nlet bump := () => { n = n + 1 }\nfunction twice(mut f : () -> void) : void { f()\nf() }\ntwice(bump)",
    );
    refuses(
        &format!(
            "{counter}let c := Tally()\nlet tick := move () => c.increment()\nlet other := tick\ntick()"
        ),
        "E_USE_AFTER_MOVE",
    );
    refuses(
        "let mut n := 0\nlet bump := () => { n = n + 1 }\nlet outer := () => bump()\nfunction twice(f : () -> void) : void { f() }\ntwice(outer)",
        "E_MUT_PARAMETER",
    );
    good("let shared := move (x : Int) => x * 2\nlet copy := shared\nlet a := shared(1) + copy(2)");
    good("let n := 3\nfunction run(f : Int -> Int) : Int = f(1)\nlet a := run(x => x + n)");
}

#[test]
fn function_types_can_declare_outside_effects() {
    good(
        "function each(xs : List[Int], f : Int -> void with Effects) : void with Effects { for (x in xs) { f(x) } }\neach([1], x => print(x))\neach([1], x => {})",
    );
    refuses(
        "function each(f : Int -> void) : void = f(1)\neach(x => print(x))",
        "E_CALL_ARGUMENTS",
    );
    refuses(
        "function each(f : Int -> void with Effects) : void = f(1)",
        "E_EFFECT_REQUIRED",
    );
    good("function make() : (Int -> void with Effects) = x => print(x)");
    // In a declaration header, a trailing `with Effects` still marks the declaration.
    good("function make() : Int -> Int with Effects { print(1)\nx => x }");
}

#[test]
fn written_callback_types_keep_mut_borrows_separate_from_outside_effects() {
    good(
        "alias Action = mut (Int -> void with Effects)\nfunction run(mut action : Action) : void with Effects=action(1)\nfunction own(move action : Action) : void with Effects=action(1)",
    );
    refuses(
        "function bad(action : mut (() -> Int)) : Int=action()",
        "E_MUT_PARAMETER",
    );
    refuses(
        "function bad(mut action : mut (() -> void with Effects)) : void=action()",
        "E_EFFECT_REQUIRED",
    );
    refuses(
        "class Holder {var step : mut (() -> Int)\nnew(move step : mut (() -> Int)) {this.step=step}\npublic function bad() : Int=this.step()}",
        "E_MUT_RECEIVER",
    );
    good("function take(move f : mut (() -> Int)) : Int=f()\ntake(() => 7)");
    refuses(
        "function bad(mut values : Array[mut (() -> Int)]) : Int=values[0]()",
        "E_MUT_PARAMETER",
    );
}

#[test]
fn annotations_and_containers_cannot_erase_mutating_capture_permissions() {
    let header = "class Tally {var n : Int\nnew() {this.n=0}\npublic mut function bump() : Int {this.n=this.n+1\nthis.n}}\n";
    for body in [
        "let tally:=Tally()\nlet bump : () -> Int=move () => tally.bump()",
        "let tally:=Tally()\nlet bump:=move () => tally.bump()\nlet fs : List[() -> Int]=[bump]",
        "function make() : () -> Int {let tally:=Tally()\nmove () => tally.bump()}",
        "function use(move f : () -> Int) : Int=f()\nlet tally:=Tally()\nlet bump:=move () => tally.bump()\nuse(bump)",
        "class Holder {var f : () -> Int\nnew(move f : () -> Int) {this.f=f}}\nlet tally:=Tally()\nlet bump:=move () => tally.bump()\nlet holder:=Holder(bump)",
        "function use(fs : List[() -> Int]) : void {}\nlet tally:=Tally()\nlet bump:=move () => tally.bump()\nuse([bump])",
        "function leak(mut f : () -> Int) : () -> Int=f",
    ] {
        refuses(&format!("{header}{body}"), "E_CAPTURE_TYPE");
    }
    refuses(
        "function read(f : () -> Int) : Int=f()\nfunction outer(mut f : () -> Int) : Int {let g:=() => f()\nread(g)}",
        "E_MUT_PARAMETER",
    );
    good(&format!(
        "{header}function run(mut f : () -> Int) : Int=f()\nlet tally:=Tally()\nlet bump:=move () => tally.bump()\nrun(bump)\nrun(bump)\nrun(() => 7)"
    ));
    refuses(
        &format!(
            "{header}let tally:=Tally()\nlet bump:=move () => tally.bump()\nlet fs:=[bump]\nfor (f in fs) {{f()}}"
        ),
        "E_MUT_PARAMETER",
    );
}
