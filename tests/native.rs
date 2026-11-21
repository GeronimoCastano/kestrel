use kestrel::{
    core::{self, Program},
    modules::{ProjectCheck, check_path, check_source},
    native::{self, TemporaryDirectory},
};
use std::{
    fs,
    process::{Command, Output},
};

fn lower(source: &str) -> Program {
    let project = ProjectCheck {
        modules: vec![check_source("native.kes", source)],
    };
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    core::lower(&project).unwrap_or_else(|errors| panic!("{errors:#?}\n{source}"))
}
fn execute(source: &str) -> Output {
    execute_program(&lower(source))
}
#[test]
fn primitive_import_cycles_share_function_identities_and_tail_call_groups() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = check_path(root.join("testdata/language/cycles.kes"), root).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let program = core::lower(&project).unwrap();
    let output = execute_program(&program);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"true\nheap value\n");
}

#[test]
fn data_import_cycles_share_layouts_methods_and_generic_code() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = check_path(root.join("testdata/language/datacycles.kes"), root).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"7\n");
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("shapes.kes"),
        "import units.{Unit, Measured, total, larger}\npublic trait Shape {\nfunction area() : double\n}\npublic class Square {\nhas Shape\npublic final side : double\npublic final unit : Unit\nnew(side : double, move unit : Unit) {this.side = side\nthis.unit = unit}\npublic function area() : double = this.side * this.side\npublic function measured() : Measured = Measured(this.area())\n}\npublic function both(a : Square, b : Square) : double = total(a.measured(), b.measured())\npublic function bigger(a : Square, b : Square) : Square = larger(a, b)",
    )
    .unwrap();
    fs::write(
        dir.path().join("units.kes"),
        "import shapes.{Shape, Square}\npublic enum Unit {\nMeters\nFeet\n}\npublic class Measured {\npublic final value : double\nnew(value : double) {this.value = value}\n}\npublic function total(a : Measured, b : Measured) : double = a.value + b.value\npublic function larger[S : Shape](a : S, b : S) : S = if a.area() >= b.area() then a else b\npublic function unitName(s : Square) : String = match s.unit {\nMeters => \"m\",\nFeet => \"ft\",\n}",
    )
    .unwrap();
    fs::write(
        dir.path().join("app.kes"),
        "import shapes.{Square, both, bigger}\nimport units.{Unit, unitName}\nlet small := Square(2.0, Unit.Meters)\nlet large := Square(3.0, Unit.Feet)\nprint(both(small, large))\nprint(unitName(bigger(small, large)))\nprint(unitName(bigger(large, small)))",
    )
    .unwrap();
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "13.0\nft\nft\n");
}

#[test]
fn trait_defaults_dispatch_on_concrete_and_dyn_receivers() {
    let output = execute(
        r#"
trait Named {
public function value() : String
public function name() : String = this.value()
}
trait NamedView { has Named }
trait NamedAgain { has Named }
class User { has NamedView, NamedAgain
new() {}
public function value() : String = "Ada"
}
class OverrideUser { has Named
new() {}
public function value() : String = "Grace"
@override public function name() : String = "override"
}
trait Identity[T] { public function identity(value : T) : T = value }
class Number { has Identity[Int]
new() {}
}
trait Source { type Item
public function value() : Item
public function current() : Item = this.value()
}
class IntegerSource { has Source
new() {}
public function value() : Int = 11
}
trait PrintableValue[T : Text] { public function value() : T
public function display() : String = this.value().text()
}
class Word { has PrintableValue[String]
new() {}
public function value() : String = "hello"
}
trait Echo { type Item
public function value() : Item
public function describe(value : Item) : String
public function echo() : String = this.describe(this.value())
}
class EchoWord { has Echo
new() {}
public function value() : String = "hello"
public function describe(value : String) : String = "[" + value + "]"
}
let user := User()
print(user.name())
let view : &dyn Named = user
print(view.name())
let overrideUser := OverrideUser()
print(overrideUser.name())
let overrideView : &dyn Named = overrideUser
print(overrideView.name())
print(Number().identity(7))
let identityView : &dyn Identity[Int] = Number()
print(identityView.identity(9))
let source := IntegerSource()
print(source.current())
let sourceView : &dyn Source(Item = Int) = source
print(sourceView.current())
let word := Word()
print(word.display())
let printableView : &dyn PrintableValue[String] = word
print(printableView.display())
let echo := EchoWord()
print(echo.echo())
let echoView : &dyn Echo(Item = String) = echo
print(echoView.echo())
"#,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"Ada\nAda\noverride\noverride\n7\n9\n11\n11\nhello\nhello\n[hello]\n[hello]\n"
    );
}

#[test]
fn imported_trait_defaults_keep_their_body_for_concrete_and_dyn_calls() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("names.kes"),
        "public trait Named { public function name() : String = \"imported\" }",
    )
    .unwrap();
    fs::write(
        dir.path().join("app.kes"),
        "import names.{Named}\nclass User { has Named\nnew() {} }\nlet user := User()\nprint(user.name())\nlet view : &dyn Named = user\nprint(view.name())",
    )
    .unwrap();
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"imported\nimported\n");
}

#[test]
fn a_class_can_extend_one_from_the_other_side_of_an_import_cycle() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("dogs.kes"),
        "import animals.{Animal, describe}\npublic class Dog extends Animal {\nnew(name : String) {this.name = name}\npublic function sound() : String = \"woof\"\n}\npublic function make() : Dog = Dog(\"rex\")\npublic function label(d : Dog) : String = describe(d) + \" says \" + d.sound()",
    )
    .unwrap();
    fs::write(
        dir.path().join("animals.kes"),
        "import dogs.{Dog, make}\npublic class Animal {\npublic var name : String\nnew(name : String) {this.name = name}\n}\npublic function describe(a : Animal) : String = a.name\npublic function dogName() : String = make().name",
    )
    .unwrap();
    fs::write(
        dir.path().join("app.kes"),
        "import dogs.{make, label}\nimport animals.{dogName}\nprint(dogName())\nprint(label(make()))",
    )
    .unwrap();
    for entry in ["dogs.kes", "animals.kes"] {
        let project = check_path(dir.path().join(entry), dir.path()).unwrap();
        assert!(project.is_ok(), "{entry}: {}", project.render_diagnostics());
    }
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "rex\nrex says woof\n"
    );
}

const SHAPE_FAMILY: &str = r#"trait Shape {
    function area() : double
}
class Figure {
    has Shape
    new() {}
    public function area() : double = 0.0
}
class Square extends Figure {
    public final side : double
    new(side : double) { this.side = side }
    @override public function area() : double = this.side * this.side
}
class Circle {
    has Shape
    public final radius : double
    new(radius : double) { this.radius = radius }
    public function area() : double = 3.0 * this.radius * this.radius
}
"#;

#[test]
fn values_go_into_new_boxes_where_a_box_is_expected() {
    prints(
        &format!(
            "{SHAPE_FAMILY}{}",
            r#"
let figures : List[Box[dyn Shape]] = [Figure(), Square(3.0), Circle(1.0)]
for (let g in figures) {
    print(g.area())
}
let b : Box[dyn Shape] = Square(3.0)
print(b.area())
let c : Box[Square] = Square(2.0)
print(c.side)
function keep(move s : Box[dyn Shape]) : double = s.area()
print(keep(Circle(2.0)))
function look(s : Box[dyn Shape]) : double = s.area()
print(look(Square(2.0)))
function make(flag : Bool) : Box[dyn Shape] = if flag then Square(2.0) else Circle(1.0)
print(make(true).area())
print(make(false).area())
class Holder {
    public final shape : Box[dyn Shape]
    new(move s : Square) { this.shape = s }
}
print(Holder(Square(5.0)).shape.area())
function pick(x : Square) : String = "exact"
function pick(x : Box[dyn Shape]) : String = "boxed"
print(pick(Square(1.0)))
print(pick(Circle(1.0)))
let sq := Square(4.0)
let reused : Box[Square] = Square(6.0)
let xs : List[Box[dyn Shape]] = [sq, reused, box(Circle(1.0))]
for (let g in xs) {
    print(g.area())
}
"#
        ),
        "0.0\n9.0\n3.0\n9.0\n2.0\n12.0\n4.0\n4.0\n3.0\n25.0\nexact\nboxed\n16.0\n36.0\n3.0\n",
    );
}

const FIGURE_FAMILY: &str = r#"class Figure {
    public var scale : double
    new() { this.scale = 1.0 }
    public function area() : double = 0.0
    public function fits(limit : double) : Bool = this.area() <= limit
    public mut function bump() : void { this.scale = this.scale + 1.0 }
    public function me() : Figure = this
}
class Square extends Figure {
    public var side : double
    new(side : double) { this.side = side
this.scale = 1.0 }
    @override public function area() : double = this.side * this.side * this.scale
}
class Tile extends Square {
    new(side : double) { this.side = side
this.scale = 1.0 }
    @override public function area() : double = 100.0
}
"#;

#[test]
fn subclasses_stand_in_for_borrowed_superclasses_and_keep_their_overrides() {
    prints(
        &format!(
            "{FIGURE_FAMILY}{}",
            r#"
function describe(f : Figure) : double = f.area()
function grow(mut f : Figure) : void = f.bump()
function scaleOf(f : Figure) : double = f.scale
function check(f : Figure) : Bool = f.fits(5.0)
function bigger(a : Figure, b : Figure) : Figure = if a.area() >= b.area() then a else b
function depth(f : Figure, n : Int) : double = if n == 0 then f.area() else depth(f, n - 1)
function name(f : Figure) : String = "figure"
function name(s : Square) : String = "square"
let mut sq := Square(2.0)
print(describe(sq))
print(sq.area())
grow(sq)
print(scaleOf(sq))
print(describe(sq))
print(check(sq))
print(check(Square(1.0)))
print(bigger(Square(1.0), Square(3.0)).area())
print(depth(Tile(1.0), 100000))
print(name(Figure()))
print(name(Square(1.0)))
print(name(Tile(1.0)))
print(describe(Tile(1.0)))
print(describe(Figure()))
print(sq.me().area())
let g := describe
print(g(Square(3.0)))
"#
        ),
        "4.0\n4.0\n2.0\n8.0\nfalse\ntrue\n9.0\n100.0\nfigure\nsquare\nsquare\n100.0\n0.0\n8.0\n9.0\n",
    );
    let mixed = format!(
        "{FIGURE_FAMILY}function bigger(a : Figure, b : Figure) : Figure = if a.area() >= b.area() then a else b\nprint(bigger(Square(1.0), Figure()).area())\n"
    );
    prints(&mixed, "1.0\n");
}

#[test]
fn classes_hold_themselves_through_options_and_lists() {
    prints(
        include_str!("../testdata/language/trees.kes"),
        "6\n3\n601\n",
    );
    prints(
        "enum Color {\nRed\nBlue\n}\nclass Pair {\npublic final a : Color\npublic final b : Color\nnew(move a : Color, move b : Color) { this.a = a\nthis.b = b }\n}\nlet p := Pair(Red, Red)\nlet q := Pair(Color.Blue, Color.Blue)\nlet o : Option[Pair] = None\nprint(match q.a { Red => 1, Blue => 2 })\n",
        "2\n",
    );
}

#[test]
fn mixed_subclass_views_keep_methods_fields_and_caller_storage() {
    prints(
        include_str!("../testdata/language/classviews.kes"),
        "4.0\nbase figure\n4.0\n1.0\ntrue\n100.0\n4.0\n3.0\n12.0\n",
    );
    prints(
        include_str!("../testdata/language/47-borrowed-subclass-results.kes"),
        "4.0\n0.0\n",
    );
    prints(
        &format!(
            "{FIGURE_FAMILY}\nfunction choose(a : Figure,b : Figure,flag : Bool) : Figure = if flag then a else b\nfunction deep(a : Figure,n : Int) : Figure = if n == 0 then a else deep(a,n-1)\nlet square:=Square(2.0)\nlet figure:=Figure()\nprint(deep(choose(square,figure,true),100000).area())"
        ),
        "4.0\n",
    );
}

#[test]
fn mixed_class_views_cannot_discard_owned_return_storage() {
    let source = format!(
        "{FIGURE_FAMILY}\nfunction maybe(s : Square,flag : Bool) : Square = if flag then s else Square(3.0)\nfunction id(s : Square,flag : Bool) : Square = maybe(s,flag)\nfunction bigger(a : Figure,b : Figure) : Figure = if a.area() > b.area() then a else b\nfunction escape(s : Square,f : Figure,flag : Bool) : Figure = bigger(id(s,flag),f)\nlet s := Square(2.0)\nlet f := Figure()\nprint(escape(s,f,false).area())"
    );
    let project = ProjectCheck {
        modules: vec![check_source("native.kes", &source)],
    };
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let errors = core::lower(&project).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.diagnostic.code == "E_NATIVE_OWNERSHIP"
                && error.diagnostic.message.contains("newly owned result")),
        "{errors:?}"
    );
    // Keeping the view within the storage owner's function remains valid.
    prints(
        &format!(
            "{FIGURE_FAMILY}\nfunction maybe(s : Square,flag : Bool) : Square = if flag then s else Square(3.0)\nfunction bigger(a : Figure,b : Figure) : Figure = if a.area() > b.area() then a else b\nfunction local(s : Square,f : Figure) : double with Effects {{let first:=maybe(s,false)\n{{let view:=bigger(first,f)\nprint(view.area())}}\nfirst.area()}}\nlet s:=Square(2.0)\nlet f:=Figure()\nprint(local(s,f))"
        ),
        "9.0\n9.0\n",
    );
}

#[test]
fn subclass_views_respect_generic_instances_and_copyable_fields() {
    prints(
        r#"class Base[T] {public var value : T
public var values : List[Int]
new(move value : T) {this.value=value
this.values=[1,2]}
public function get() : T = this.value}
class Child[T] extends Base[T] {
new(move value : T) {this.value=value
this.values=[3,4]}}
function choose(a : Base[T],b : Base[T],flag : Bool) : Base[T] = if flag then a else b
function unrelated(a : Base[Int],b : Base[String]) : Base[Int] = a
function inferred[T](a : T) : T = a
let a:=Base(7)
let b:=Child(9)
let text:=Base("text" + " value")
let textChild:=Child("child" + " value")
print(choose(a,b,true).value)
print(choose(a,b,false).values)
print(choose(text,textChild,false).get())
print(unrelated(b,text).get())
print(inferred(choose(a,b,false)).get())
function list(a : Base[Int],b : Base[Int],flag : Bool) : List[Int] = if flag then a.values else b.values
print(list(a,b,false))
"#,
        "7\n[3, 4]\nchild value\n9\n9\n[3, 4]\n",
    );
}

#[test]
fn defaults_use_definition_names_and_instantiated_generic_types() {
    prints(
        include_str!("../testdata/language/48-default-argument-lifetimes.kes"),
        "fresh value\nowned value\n1\n42\n",
    );
    prints(
        r#"let seed:=1
function read(value : Int = seed) : Int = value
function caller(seed : Int) : Int {let value:=read()
value+seed}
print(caller(9))
class Holder {public final seed : Int
new(seed : Int) {this.seed=seed}}
let source:=Holder(5)
function member(seed : Int = source.seed) : Int = seed
print(member())
function helper(n : Int) : Int = n
function named(n : Int = helper(n=7)) : Int = n
print(named())
function empty(values : List[T] = []) : List[T] = values
let ints : List[Int] = empty()
let strings : List[String] = empty()
print(ints)
print(strings)
let saved:=read
print(saved())
"#,
        "10\n5\n7\n[]\n[]\n1\n",
    );
}

#[test]
fn default_objects_live_through_borrowed_reads_and_owned_returns() {
    prints(
        r#"class Point {public final label : String
new(label : String) {this.label=label}}
function identity(p : Point = Point("fresh" + " value")) : Point = p
function text() : String = identity().label
function own(move p : Point = Point("owned" + " value")) : Point = p
function make() : Point = own()
function first(values : List[Point] = [Point("list" + " value")]) : Point = match values { [item, ..rest] => item, [] => Point("empty"), }
print(text())
let point:=identity()
print(point.label)
print(make().label)
let item:=first()
print(item.label)
let global:=Point("global" + " value")
function fromGlobal(p : Point = global) : Point = p
function borrow() : Point = fromGlobal()
print(borrow().label)
function label(n : Int,p : Point = Point("partial" + " value")) : String = p.label
let partial:=label(_)
print(partial(2))
"#,
        "fresh value\nfresh value\nowned value\nlist value\nglobal value\npartial value\n",
    );
}

#[test]
fn borrowed_results_of_copyable_move_parameters_keep_caller_storage() {
    prints(
        r#"class Point {public final label : String
new(label : String) {this.label=label}}
function first(move values : List[Point]) : Point = match values {
    [item,..rest] => item,
    [] => Point("empty"),
}
let point:=first([Point("literal" + " value")])
print(point.label)
print(point.label)
let values:=[Point("copied" + " value")]
let copied:=first(values)
print(copied.label)
for (value in values) {print(value.label)}
function defaulted(move values : List[Point] = [Point("default" + " value")]) : Point = first(values)
let defaultedPoint:=defaulted()
print(defaultedPoint.label)
"#,
        "literal value\nliteral value\ncopied value\ncopied value\ndefault value\n",
    );
    prints(
        r#"class Point {public final label : String
new(label : String) {this.label=label}}
class Panel {public final point : &Point
new(point : Point) {this.point=point}}
function panel(move values : List[Point],fallback : Point) : Panel = match values {
    [point,..rest] => Panel(point),
    [] => Panel(fallback),
}
let fallback:=Point("fallback")
let view:=panel([Point("retained" + " item")],fallback)
print(view.point.label)
print(view.point.label)
"#,
        "retained item\nretained item\n",
    );
}

#[test]
fn classes_define_arithmetic_operators_through_traits() {
    prints(
        include_str!("../testdata/language/vectors.kes"),
        "3.0\n6.0\n1.5\n6.0\nabcd\n5\n7\n",
    );
}

#[test]
fn long_borrowing_call_chains_keep_native_views_alive() {
    let mut source = "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction id0(p : Point) : Point = p\n".to_owned();
    for i in (1..24).rev() {
        source.push_str(&format!(
            "function id{i}(p : Point) : Point = id{}(p)\n",
            i - 1
        ));
    }
    source.push_str("let p := Point(\"kept\" + \" alive\")\nprint(id23(p).label)\nprint(p.label)");
    for i in 0..8 {
        source.push_str(&format!(
            "\nlet p{i} := Point(\"p\" + {i}.text())\nlet mut a{i} := id23(p{i})\n"
        ));
    }
    source.push_str("for (let i : Int = 0; i < 8; i = i + 1) {\n");
    for i in 0..7 {
        source.push_str(&format!("a{i}=a{}\n", i + 1));
    }
    source.push_str("}\nprint(a0.label)\nprint(p7.label)");
    prints(&source, "kept alive\nkept alive\np7\np7\n");
}

#[test]
fn reassigning_a_field_alias_keeps_earlier_views_on_their_original_values() {
    prints(
        include_str!("../testdata/language/aliases.kes"),
        "left value | left value\nleft value | right value\n",
    );
}

#[test]
fn postconditions_check_success_values_and_preserve_error_returns() {
    prints(
        include_str!("../testdata/language/postconditions.kes"),
        "0\n4\nhello\n3\nparse\n",
    );
    prints(
        include_str!("../testdata/language/44-postconditions.kes"),
        "0\n4\n",
    );
    prints(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction maybe(p : Point, fresh : Bool) : Point ensures result.label != \"\" = if fresh then Point(\"new\" + \"!\") else p\nlet p := Point(\"old\" + \"!\")\nprint(maybe(p,false).label)\nprint(maybe(p,true).label)\nprint(p.label)",
        "old!\nnew!\nold!\n",
    );
    prints(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction make() : Point ensures result.label == \"kept alive\" = Point(\"kept\" + \" alive\")\nfunction view(p : Point) : Point ensures result.label == \"kept alive\" = p\nfunction fallible(ok : Bool) : Point or Error ensures result.label == \"kept alive\" = if ok then make() else RuntimeError.Parse(\"no\")\nfunction quiet() : void ensures true {}\nfunction failed() : void or Error ensures false {let n := parseInt(\"invalid\")?}\nfunction values(n : Int) : List[Int] ensures result == [n] = [n]\nlet p := make()\nprint(view(p).label)\nprint(match fallible(true) {p => p.label, _ => \"error\",})\nprint(match fallible(false) {RuntimeError.Parse(_) => \"exempt\", _ => \"other\",})\nquiet()\nprint(match failed() {RuntimeError.Parse(_) => \"void exempt\", _ => \"other\",})\nprint(values(7))",
        "kept alive\nkept alive\nexempt\nvoid exempt\n[7]\n",
    );
}

#[test]
fn postcondition_failures_stop_at_implicit_and_early_returns() {
    for body in ["0", "{return 0}", "{if true {return 0}\n1}"] {
        let output = execute(&format!(
            "function broken() : Int ensures result > 0 = {body}\nprint(broken())"
        ));
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let errors = String::from_utf8_lossy(&output.stderr);
        assert!(
            errors.contains("Postcondition failed: result > 0"),
            "{errors}"
        );
    }
    let output = execute("function quiet() : void ensures false {}\nquiet()");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Postcondition failed: false"));
    let output =
        execute("function broken() : Int ensures result >= 0 ensures result < 5 = 7\nbroken()");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Postcondition failed: result < 5"));
}

#[test]
fn postconditions_use_signature_names_and_the_method_state_after_return() {
    prints(
        "let bound := 10\nfunction f() : Int ensures result < bound {let bound := 0\nreturn 5}\nclass Counter {public var n : Int\nnew() {this.n=0}\npublic mut function set(n : Int) : void ensures this.n == n {this.n=n}}\nfunction identity(n : Int) : Int ensures result == n {let result := 100\nreturn n}\nprint(f())\nprint(identity(6))\nlet c := Counter()\nc.set(8)\nprint(c.n)",
        "5\n6\n8\n",
    );
}

#[test]
fn task_cleanup_errors_are_exempt_from_postconditions() {
    prints(
        "function fail() : void or Error = RuntimeError.Parse(\"joined\")\nfunction run() : Int or Error with Effects ensures false {let _ := scope {spawn fail()\nreturn 1}\n2}\nprint(match run() {RuntimeError.Parse(message) => message, n => n.text(), _ => \"other\",})",
        "joined\n",
    );
}

#[test]
fn generic_and_union_postconditions_keep_specialized_results() {
    prints(
        "function identity[T : Text](x : T) : T ensures result.text() == result.text() = x\nfunction accepted(x : Int or String) : Bool = match x {1 => true, \"ok\" => true, _ => false,}\nfunction choice(flag : Bool) : Int or String or Error ensures accepted(result) = if flag then 1 else \"ok\"\nprint(identity(7))\nprint(identity(\"heap\" + \" text\"))\nlet a := choice(true)\nlet b := choice(false)\nprint(\"checked\")",
        "7\nheap text\nchecked\n",
    );
}

#[test]
fn imported_postconditions_run_in_the_declaring_module() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("lib.kes"),
        "const LIMIT : Int = 10\npublic function f(n : Int) : Int ensures result < LIMIT = n",
    )
    .unwrap();
    fs::write(
        dir.path().join("main.kes"),
        "import lib.{f}\nprint(f(3))\nprint(f(12))",
    )
    .unwrap();
    let project = check_path(dir.path().join("main.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"3\n");
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(
        errors.contains("lib.kes:2:") && errors.contains("Postcondition failed: result < LIMIT"),
        "{errors}"
    );
}

#[test]
fn naming_a_class_field_borrows_it_and_leaves_the_owner_whole() {
    prints(
        r#"
class Label {
    public final value : String
    new(value : String) {this.value=value}
}
class Holder {
    public final label : Label
    new(move label : Label) {this.label=label}
}
class Outer {
    public final inner : Holder
    new(move inner : Holder) {this.inner=inner}
}
function make() : Holder = Holder(Label("t" + "1"))
function plain() : String {
    let left := Holder(Label("a" + "b"))
    let view := left.label
    let before := view
    before.value + "|" + view.value + "|" + left.label.value
}
function nested() : String {
    let o := Outer(Holder(Label("c" + "d")))
    let v := o.inner.label
    let w := o.inner
    v.value + "|" + w.label.value + "|" + o.inner.label.value
}
function looped(n : Int) : String {
    let left := Holder(Label("l" + "1"))
    let right := Holder(Label("r" + "2"))
    let mut view := left.label
    let mut i := 0
    while i < n { view = right.label
    i = i + 1 }
    view.value + "|" + left.label.value + "|" + right.label.value
}
function temporary() : String {
    let l := make().label
    l.value
}
function parameter(h : Holder) : String {
    let l := h.label
    l.value + "|" + h.label.value
}
print(plain())
print(nested())
print(looped(0))
print(looped(3))
print(temporary())
let h := Holder(Label("p" + "q"))
let top := h.label
print(parameter(h) + "|" + top.value + "|" + h.label.value)
"#,
        "ab|ab|ab\ncd|cd|cd\nl1|l1|r2\nr2|l1|r2\nt1\npq|pq|pq|pq\n",
    );
}
fn execute_program(program: &Program) -> Output {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let binary = dir.path().join("program");
    native::build(program, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    if output.status.success() && std::env::var_os("KESTREL_LEAK_CHECK").is_some() {
        assert_no_leaks(&binary);
    }
    if output.status.success() && std::env::var_os("KESTREL_SANITIZE").is_some() {
        assert_sanitizer_clean(program, &output);
    }
    output
}
/// Rebuilds the generated C with AddressSanitizer and UndefinedBehaviorSanitizer; the run
/// must print the same output and report nothing.
fn assert_sanitizer_clean(program: &Program, expected: &Output) {
    assert_sanitizer_clean_with_args(program, expected, &[]);
}
fn assert_sanitizer_clean_with_args(program: &Program, expected: &Output, arguments: &[&str]) {
    let mut python_flags = vec![];
    if !program.python_sites.is_empty() {
        for options in [&["--includes"][..], &["--ldflags", "--embed"][..]] {
            let configured = Command::new("python3-config")
                .args(options)
                .output()
                .unwrap();
            assert!(configured.status.success());
            python_flags.extend(
                String::from_utf8(configured.stdout)
                    .unwrap()
                    .split_whitespace()
                    .map(str::to_owned),
            );
        }
    }
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let source = dir.path().join("program.c");
    let binary = native::executable_path(&dir.path().join("checked"));
    fs::write(&source, native::emit_c(program).unwrap()).unwrap();
    let built = Command::new("clang")
        .args([
            "-std=c11",
            "-O1",
            "-g",
            "-fsanitize=address,undefined",
            "-fno-sanitize-recover=undefined",
            "-Wno-unused-function",
        ])
        .args(native::thread_flags())
        .arg(&source)
        .args(&program.c_sources)
        .args(
            program
                .c_include_dirs
                .iter()
                .flat_map(|d| [std::ffi::OsStr::new("-I"), d.as_os_str()]),
        )
        .args(python_flags)
        .args(program.c_libraries.iter().map(|name| format!("-l{name}")))
        .args(native::system_libraries())
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let run = Command::new(&binary)
        .args(arguments)
        .env("ASAN_OPTIONS", "detect_leaks=0")
        .envs(native::sanitizer_runtime_path().map(|path| ("PATH", path)))
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{:?}: {}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(run.stdout, expected.stdout);
}
/// macOS `leaks` reports heap blocks that no live pointer reaches when the program exits.
fn assert_no_leaks(binary: &std::path::Path) {
    assert_no_leaks_with_args(binary, &[]);
}
fn assert_no_leaks_with_args(binary: &std::path::Path, arguments: &[&str]) {
    // `leaks` runs only when asked for, so an ordinary `cargo test` never waits on it.
    if !cfg!(target_os = "macos") || std::env::var_os("KESTREL_LEAK_CHECK").is_none() {
        return;
    }
    let mut child = Command::new("leaks")
        .arg("--atExit")
        .arg("--")
        .arg(binary)
        .args(arguments)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    // Read the report on its own thread so a long report cannot fill the pipe.
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut stdout, &mut text).unwrap();
        text
    });
    // `leaks` needs macOS permission to inspect a process. If that request waits
    // on a dialog, fail after two minutes instead of hanging the whole suite.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "`leaks` did not finish within two minutes for {}. A pending macOS Developer Tools permission dialog can block it; dismiss it and rerun.",
                binary.display()
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let text = reader.join().unwrap();
    assert!(text.contains(" 0 leaks for 0 total leaked bytes"), "{text}");
}
fn execute_c(source: &str) -> Output {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let path = dir.path().join("test.c");
    let binary = native::executable_path(&dir.path().join("test"));
    fs::write(&path, source).unwrap();
    let built = Command::new("clang")
        .args([
            "-std=c11",
            "-O2",
            "-Werror",
            "-Wno-unused-function",
            "-Wno-unused-label",
        ])
        .args(native::thread_flags())
        .arg(&path)
        .args(native::system_libraries())
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    Command::new(binary).output().unwrap()
}
fn intercept_close(program: &Program, implementation: &str) -> Output {
    let source = native::emit_c(program)
        .unwrap()
        .replace("fclose(", "k_test_close(");
    // <setjmp.h> follows <stdio.h> in the runtime on every platform.
    let source = source.replacen(
        "#include <setjmp.h>",
        &format!("#include <setjmp.h>\nstatic int k_test_close(FILE *file) {{ {implementation} }}"),
        1,
    );
    execute_c(&source)
}
/// Like `prints`, but a line holding a fractional number may differ from the expected one
/// in its last bits: libm's lgamma and exp round differently on each C library.
fn prints_close(source: &str, expected: &str) {
    let output = execute(source);
    assert!(
        output.status.success(),
        "{:?}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let got = String::from_utf8(output.stdout).unwrap();
    let (found, wanted) = (
        got.lines().collect::<Vec<_>>(),
        expected.lines().collect::<Vec<_>>(),
    );
    assert_eq!(found.len(), wanted.len(), "{got}");
    for (have, want) in found.iter().zip(&wanted) {
        match (have.parse::<f64>(), want.parse::<f64>()) {
            (Ok(a), Ok(b)) if want.contains('.') => {
                assert!(
                    (a - b).abs() <= 1e-12 * b.abs().max(1.0),
                    "{have} != {want}\n{got}"
                )
            }
            _ => assert_eq!(have, want, "{got}"),
        }
    }
}
fn prints(source: &str, expected: &str) {
    let output = execute(source);
    assert!(
        output.status.success(),
        "{:?}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn primitive_functions_overloads_defaults_and_generic_specialization() {
    let source = r#"
function identity[T](x : T) : T = x
function choose(n : Int, scale : Int = 2) : Int = n * scale
function choose(text : String) : String = text + "!"
function recursive(n : Int) : Int = if n == 0 then 0 else n + recursive(n - 1)
print(identity(choose(scale = 3, n = 4)))
print(identity(choose("hello")))
print(identity(2.5))
print(choose(5))
print(recursive(5))
"#;
    let program = lower(source);
    let code = native::emit_c(&program).unwrap();
    assert_eq!(
        program
            .functions
            .iter()
            .filter(|f| f.name == "identity")
            .count(),
        3
    );
    assert!(
        code.contains("static int64_t")
            && code.contains("static KString")
            && code.contains("static double")
    );
    let output = execute_program(&program);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "12\nhello!\n2.5\n10\n15\n"
    );
}
#[test]
fn constructor_fields_hold_private_state() {
    prints(
        r#"
class Counter {
    public var label : String

    new(label : String, start : Int) {
        this.label = label
        final this.start : Int := start
        var this.hits : Int := 0
    }

    new(label : String) {
        this.label = label
        this.start = 0
        this.hits = 0
    }

    public function value() : Int = this.start + this.hits

    public mut function bump() : void {
        this.hits = this.hits + 1
    }
}

let mut a := Counter("a", 10)
let b := Counter("b")
a.bump()
a.bump()
print(a.label)
print(a.value())
print(b.value())
"#,
        "a\n12\n0\n",
    );
}
#[test]
fn local_partials_pipes_and_lambdas_execute() {
    prints(
        r#"
function add(a : double, b : double) : double = a + b
function twice(n : double) : double = n * 2.0
let addOne := add(1.0, _)
print((3.0 |> addOne |> twice) + (3.0 |> add(_, 1.0)))
let offset := 7
let addOffset := (n : Int) => n + offset
print(addOffset(5))
"#,
        "12.0\n12\n",
    );
}
#[test]
fn conversion_pipes_execute_and_keep_the_tensor_shape() {
    prints(
        r#"
print(1 |> double)
let t := [0, 0] |> Tensor
print(t)
print(t.shape())
"#,
        "1.0\n[0.0, 0.0]\n[2]\n",
    );
}
#[test]
fn pipes_pass_integer_literals_and_tuples_keep_their_conversions() {
    prints(
        r#"
function twice(n : double) : double = n * 2.0
function add(a : double, b : double) : double = a + b
print(1 |> twice)
print(1 |> add(_, 2.0))
print(1 |> add(2.0, _))
let n := 1
let t : (BigInt, Int) = (n, 2)
print(t[0])
print(t[1])
let u : (DynTensor[double], Int) = (Tensor([0, 0]), 3)
print(u[0])
print(u[1])
"#,
        "2.0\n3.0\n3.0\n1\n2\n[0.0, 0.0]\n3\n",
    );
}
#[test]
fn strings_copy_reassign_return_and_survive_nested_scopes() {
    prints(
        r#"
function greeting(name : String) : String {
    let result := "Hello, " + name
    return result
}
function pick(flag : Bool) : String = if flag then { let a := "a" + "b"
a } else "other"
let mut a := greeting("λ")
let b := a
a = "changed"
print(b)
print(a)
print(pick(true))
print(pick(false))
print("\uD83D\uDE00\u0000end")
print(f"{{ \u03bb {2.5:.2f} \u007b\u007b }}")
"#,
        "Hello, λ\nchanged\nab\nother\n😀\0end\n{ λ 2.50 {{ }\n",
    );
}
#[test]
fn short_circuit_and_ieee_double_zero() {
    prints(
        r#"
print(false && ((1 / 0) == 0))
print(true || ((1 / 0) == 0))
print((1.0 / 0.0).isFinite())
print(-0.0)
print("a" == "a")
print("a" != "b")
"#,
        "false\ntrue\nfalse\n-0.0\ntrue\ntrue\n",
    );
}
#[test]
fn numeric_loops_break_continue_and_early_return() {
    prints(
        r#"
function total(n : Int) : Int {
    let mut sum := 0
    for (let i : Int = 0; i < n; i = i + 1) {
        if i == 2 { continue }
        if i == 5 { break }
        sum = sum + i
    }
    let mut j := 0
    while j < 3 {
        j = j + 1
        sum = sum + j
    }
    sum
}
function early(n : Int) : Int {
    let s := "clean" + "up"
    if n > 0 { return 7 } else { return 9 }
}
print(total(20))
print(early(1))
print(early(0))
"#,
        "14\n7\n9\n",
    );
}
#[test]
fn captures_and_constants_are_read_from_the_correct_declaration() {
    prints(
        r#"
const BASE : Int = 4
let earlier := 3
function captured(n : Int) : Int = n + earlier + BASE
print(captured(2))
"#,
        "9\n",
    );
}
#[test]
fn numeric_failures_trap_with_the_source_expression() {
    for (expression, message) in [
        ("9223372036854775807 + 1", "addition overflow"),
        ("-9223372036854775808 - 1", "subtraction overflow"),
        ("4611686018427387904 * 2", "multiplication overflow"),
        ("-(-9223372036854775808)", "negation overflow"),
        ("-9223372036854775808 / -1", "division overflow"),
        ("1 / 0", "division by zero"),
        ("1 % 0", "remainder by zero"),
    ] {
        let output = execute(&format!("print({expression})"));
        assert_eq!(output.status.code(), Some(1), "{expression}");
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            error.contains("native.kes:1:7: runtime error:")
                && error.contains(message)
                && error.contains(expression),
            "{error}"
        );
    }
    prints(
        "print(-9223372036854775808 % -1)\nprint(-0x8000_0000_0000_0000)",
        "0\n-9223372036854775808\n",
    );
}
#[test]
fn imports_use_checked_declaration_ids_for_overloads_and_constants() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(dir.path().join("maths.kes"), "public const BASE : Int = 7\npublic function plus(x : Int) : Int = x + BASE\npublic function plus(x : String) : String = x + \"!\"\n").unwrap();
    fs::write(
        dir.path().join("rules.kes"),
        include_str!("../testdata/language/rules.kes"),
    )
    .unwrap();
    fs::write(
        dir.path().join("app.kes"),
        "import maths.{plus, BASE}\nimport rules.{pipesAndCurrying, mutation}\nprint(plus(BASE))\nprint(plus(\"ok\"))\nprint(pipesAndCurrying())\nprint(mutation())",
    )
    .unwrap();
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "14\nok!\n12.0\n2.0\n"
    );
}
#[test]
fn erased_collection_cursors_borrow_noncopyable_items() {
    prints(
        "class P {public final x : Int\nnew(x : Int) {this.x=x}}\nfunction count(mut values : &dyn Iterable(Item = P)) : Int with Effects {let mut n := 0\nfor (_ in values) {n=n+1}\nn}\nlet values : Box[dyn Iterable(Item = P)] = box([P(1)])\nprint(count(values))",
        "1\n",
    );
}

#[test]
fn named_arguments_run_in_source_order_before_parameter_reordering() {
    prints(
        r#"
function mark(n : Int) : Int with Effects { print(n)
n }
function pair(a : Int, b : Int) : Int = (a * 10) + b
print(pair(b = mark(2), a = mark(1)))
"#,
        "2\n1\n12\n",
    );
}
#[test]
fn loop_string_cleanup_handles_continue_break_and_returns() {
    prints(
        r#"
function build(n : Int) : String {
    let mut result := ""
    for (let i : Int = 0; i < n; i = i + 1) {
        let piece := "x" + i.text()
        if i == 1 { continue }
        if i == 4 { break }
        result = result + piece
    }
    result
}
print(build(100))
"#,
        "x0x2x3\n",
    );
}

#[test]
fn tail_recursion_uses_mandatory_tail_calls_with_bounded_stack() {
    let source = r#"
function count(n : Int, total : Int) : Int =
    if n == 0 then total else count(n - 1, total + 1)
function even(n : Int) : Bool = if n == 0 then true else odd(n - 1)
function odd(n : Int) : Bool = if n == 0 then false else even(n - 1)
function keep(n : Int, text : String) : String =
    if n == 0 then text else keep(n - 1, text)
print(count(3000000, 0))
print(even(3000000))
print(keep(300000, "heap" + " value"))
"#;
    let program = lower(source);
    assert!(
        native::emit_c(&program)
            .unwrap()
            .contains("__attribute__((musttail))")
    );
    let output = execute_program(&program);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "3000000\ntrue\nheap value\n"
    );
}
#[test]
fn tail_cycles_refuse_only_borrows_of_a_frame_that_ends() {
    prints(
        "function a(n : Int) : Bool = if n == 0 then true else b(n - 1, 0)\nfunction b(n : Int, unused : Int) : Bool = a(n)\nprint(a(2000001))\n",
        "true\n",
    );
    let refused = lower_errors(
        "class Node {public final value : Int\nnew(value : Int) {this.value=value}}\nfunction a(node : Node, n : Int) : Int {\n    if n == 0 { return node.value }\n    let local := Node(n)\n    b(local, n - 1, 0)\n}\nfunction b(node : Node, n : Int, unused : Int) : Int = a(node, n)\nprint(a(Node(1), 3))",
    );
    assert!(refused.iter().any(|code| code == "E_NATIVE_TAIL_CALL"));
}

#[test]
fn void_tail_calls_and_generic_text_bounds_execute() {
    prints(
        r#"
function finish(n : Int) : void = if n > 0 then finish(n - 1) else {}
function show[T : Text](value : T) : void with Effects {
    print(value)
    print(value.text())
}
finish(1000000)
show(3)
show("hello" + " world")
"#,
        "3\n3\nhello world\nhello world\n",
    );
}
#[test]
fn native_assertions_report_both_values_on_failure() {
    prints(
        "import std.test.{assertEq}\nassertEq(\"a\" + \"b\", \"ab\")\nassertEq(3, 3)",
        "",
    );
    let output = execute("import std.test.{assertEq}\nassertEq(41, 42)");
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("native.kes:2:1: runtime error: assertEq failed")
            && error.contains("left: 41")
            && error.contains("right: 42"),
        "{error}"
    );
}

#[test]
fn classes_and_stored_borrows_compile_without_heap_boxing() {
    let source = format!(
        "{}\nprint(p.area())",
        include_str!("../testdata/language/panel.kes")
    );
    let program = lower(&source);
    assert!(program.layouts.len() >= 2);
    let generated = native::emit_c(&program).unwrap();
    assert!(generated.contains("struct KRecord_") && generated.contains("KRecord_0 *"));
    let output = execute_program(&program);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "4.0\n");
}
#[test]
fn class_text_and_debug_fallback_execute_user_methods() {
    let source = format!(
        "{}\nprint(f\"{{p:?}}\")\nprint(p)",
        include_str!("../testdata/language/debug.kes")
    );
    prints(&source, "square of side 2.0\nsquare of side 2.0\n");
}
#[test]
fn mut_methods_and_mut_arguments_change_their_original_objects() {
    prints(
        r#"
class Counter {
    var value : Int
    new(n : Int) { this.value = n }
    public mut function increment() : void { this.value = this.value + 1 }
    public function current() : Int = this.value
    public mut function both(mut other : Counter) : void { this.increment()
other.increment() }
}
let a := Counter(2)
let b := Counter(5)
a.both(b)
print(a.current())
print(b.current())
"#,
        "3\n6\n",
    );
}
#[test]
fn returned_class_borrows_and_generic_trait_calls_preserve_identity() {
    prints(
        r#"
trait Shape { function area() : double }
class Square {
    has Shape
    public final side : double
    new(side : double) { this.side = side }
    public function area() : double = this.side * this.side
}
function biggest[S : Shape](a : S, b : S) : S = if a.area() > b.area() then a else b
let a := Square(2.0)
let b := Square(3.0)
let chosen := biggest(a, b)
print(chosen.area())
print(a.area())
"#,
        "9.0\n4.0\n",
    );
}
#[test]
fn owned_class_returns_moves_partial_fields_and_string_cleanup() {
    prints(
        r#"
class Item {
    public final label : String
    new(label : String) { this.label = label }
}
class Pair {
    public var left : Item
    public var right : Item
    new(move left : Item, move right : Item) { this.left = left
this.right = right }
}
function pass(move item : Item) : Item = item
let pair := Pair(Item("left" + "!"), Item("right" + "!"))
let item := pass(pair.left)
print(item.label)
print(pair.right.label)
"#,
        "left!\nright!\n",
    );
}
#[test]
fn inherited_methods_dispatch_to_concrete_overrides() {
    prints(
        r#"
class Figure {
    public function area() : double = 0.0
    public function twice() : double = this.area() * 2.0
}
class Square extends Figure {
    public final side : double
    new(side : double) { this.side = side }
    @override public function area() : double = this.side * this.side
}
let square := Square(3.0)
print(square.twice())
"#,
        "18.0\n",
    );
}

#[test]
fn generic_class_defaults_and_returned_field_borrows_survive_blocks() {
    prints(
        r#"
class Item {
    public final name : String
    new(name : String) { this.name = name }
}
class Holder[T] {
    public var value : T
    public final label : String = "holder" + "!"
    new(move value : T) { this.value = value }
    public function current() : T { this.value }
}
let holder := Holder(Item("kept"))
let item := holder.current()
print(item.name)
print(holder.label)
"#,
        "kept\nholder!\n",
    );
}

#[test]
fn tagged_enums_guards_and_tuple_destructuring_execute() {
    prints(
        r#"
enum Choice { Number(Int), Words(String), Empty }
function describe(value : Choice) : String = match value {
    Number(n) if n > 2 => f"large: {n}",
    Number(n) => f"small: {n}",
    Words(s) => s,
    Empty => "empty",
}
print(describe(Number(3)))
print(describe(Number(1)))
print(describe(Words("hello" + "!")))
print(describe(Empty))
let original := "kept" + "!"
let pair := (3, original)
let (n, label) := pair
print(n)
print(label)
print(original)
"#,
        "large: 3\nsmall: 1\nhello!\nempty\n3\nkept!\nkept!\n",
    );
}
#[test]
fn nested_patterns_short_circuit_before_reading_inactive_payloads() {
    prints(
        r#"
enum Inner { Number(Int), Words(String) }
enum Outer { Wrapped(Inner), Empty }
function select(value : Outer) : Int = match value {
    Wrapped(Number(n)) => n,
    Wrapped(Words(_)) => 2,
    Empty => 0,
}
print(select(Empty))
print(select(Wrapped(Words("text"))))
print(select(Wrapped(Number(9))))
"#,
        "0\n2\n9\n",
    );
}
#[test]
fn option_otherwise_and_guards_only_execute_when_reached() {
    prints(
        r#"
function check(n : Int) : Bool with Effects { print(n)
false }
function select(value : Option[Int]) : Int with Effects = match value {
    None => 0,
    Some(n) if check(n) => 1,
    Some(n) => n,
}
function unwrap(value : Option[Int]) : Int {
    let Some(n) := value otherwise { return -1 }
    n
}
print(select(None))
print(select(Some(8)))
print(unwrap(None))
print(unwrap(Some(5)))
"#,
        "0\n8\n8\n-1\n5\n",
    );
}
#[test]
fn automatic_recursive_enum_boxes_are_destroyed_without_native_stack_growth() {
    prints(
        r#"
enum Chain { End, Link(Chain) }
let mut value := End
for (let i : Int = 0; i < 150000; i = i + 1) { value = Link(value) }
print(match value { End => 0, Link(_) => 1, })
"#,
        "1\n",
    );
}
#[test]
fn typed_errors_propagate_and_widen_to_the_open_error_root() {
    let source = format!(
        r#"{}
print(totalOrZero(""))
print(totalOrZero("ok"))
print(match total("", "1") {{ Parse(msg) => "parse: " + msg, n => n.text(), _ => "other", }})
print(match total("ok", "ok") {{ Parse(msg) => "bad", n => n.text(), _ => "other", }})
"#,
        include_str!("../testdata/language/errors.kes")
    );
    prints(&source, "0\n42\nparse: \n84\n");
}
#[test]
fn annotated_union_bindings_and_void_success_values_have_tagged_layouts() {
    prints(
        r#"
enum Failure extends Error { Failed(String) }
function maybe(flag : Bool) : void or Failure = if flag then {} else Failed("no")
let n : Int or Error = 3
print(match n { value => value, _ => 0, })
print(match maybe(true) { Failed(_) => false, value => true, })
print(match maybe(false) { Failed(msg) => msg, value => "yes", })
let error : Error = Failed("boxed")
print(match error { Failed(msg) => msg, _ => "other", })
"#,
        "3\ntrue\nno\nboxed\n",
    );
}

#[test]
fn borrowed_temporaries_keep_owners_until_the_let_scope_ends() {
    prints(
        r#"
class Square {
    public final side : double
    new(side : double) { this.side = side }
    public function area() : double = this.side * this.side
}
class Panel {
    public final square : &Square
    new(square : Square) { this.square = square }
    public function area() : double = this.square.area()
}
function same(s : Square) : Square = s
let view := { same(Square(3.0)) }
print(view.area())
let panel := { Panel(Square(4.0)) }
print(panel.area())
let number := { let local := Square(2.0)
local.side }
print(number)
let owner := Square(5.0)
let field := { let local := Panel(owner)
local.square }
print(field.area())
"#,
        "9.0\n16.0\n2.0\n25.0\n",
    );
}
#[test]
fn immutable_class_script_captures_are_borrowed_by_named_functions() {
    prints(
        r#"
class Item {
    public final name : String
    new(name : String) { this.name = name }
}
let item := Item("kept" + "!")
function get() : Item { item }
print(get().name)
print(item.name)
"#,
        "kept!\nkept!\n",
    );
}

#[test]
fn match_arms_return_directly() {
    prints_from_file(
        r#"
function clamp(n : Int) : Int {
    match n {
        x if x > 9 => return 9,
        x => return x,
    }
}
function pick(n : Int) : Int {
    let value := match n {
        0 => return 100,
        k => k * 2,
    }
    value + 1
}
function check(text : String) : Bool {
    let stack := Stack[String]()
    for (c in text) {
        if c == "(" {
            stack.push(c)
        } else if c == ")" {
            match stack.pop() {
                Some(_) => stack.size(),
                None => return false,
            }
        }
    }
    return stack.size() == 0
}
function announce(flag : Bool) : void with Effects {
    match flag {
        true => return,
        false => print("kept going"),
    }
    print("after")
}
print(clamp(12))
print(clamp(3))
print(pick(0))
print(pick(4))
print(check("(()())"))
print(check("())("))
announce(true)
announce(false)
"#,
        "9\n3\n100\n9\ntrue\nfalse\nkept going\nafter\n",
    );
}

#[test]
fn nats_widen_to_bigint_where_a_bigint_is_expected() {
    prints(
        r#"
function widen(m : Nat) : BigInt = m
let n : Nat = 18446744073709551615
let b : BigInt = n
print(b.text())
let small : Nat = 3
let mut total : BigInt = 2
total = total * small
print(total.text())
print((small * total + 1).text())
print((small < total).debug())
print(widen(0).text())
print((total - small).text())
"#,
        "18446744073709551615\n6\n19\ntrue\n0\n3\n",
    );
}

#[test]
fn ints_widen_to_bigint_where_a_bigint_is_expected() {
    prints(
        r#"
function factorial(n : Int) : BigInt {
    let mut result : BigInt = 1
    for (k in range(1, n + 1)) {
        result = k * result
    }
    result
}
function double(b : BigInt) : BigInt = b * 2
function pick(k : Int) : BigInt {
    return k
}
let k := 7
let mut total : BigInt = 1
total = k
print(factorial(25).text())
print(double(k).text())
print(pick(k).text())
print((k + 3 * total - 2).text())
print((total == k).debug())
print((k < (total + 1)).debug())
"#,
        "15511210043330985984000000\n14\n7\n26\ntrue\ntrue\n",
    );
}

#[test]
fn sorted_gives_a_new_list_and_sort_orders_an_array_in_place() {
    prints(
        r#"
class Item {
    has Comparable
    public final key : Int
    public final name : String
    new(key : Int, name : String) {
        this.key = key
        this.name = name
    }
    public function compare(other : Item) : Int = this.key - other.key
}
function top[T : Sortable(Item = Int)](values : T) : String = values.sorted().join(",")
let xs := [3, 1, 2, 9, -4]
print(xs.sorted().join(", "))
print(xs.join(", "))
print([2.5, -1.0, 0.25].sorted().join(" "))
print(["pear", "apple"].sorted().join(" "))
print(top(Array.generate(3, i => 3 - i)))
let empty : List[Int] = []
print(empty.sorted().length())
let items := [Item(2, "b"), Item(1, "a"), Item(2, "c"), Item(1, "d")]
print(items.sorted().map(i => i.name).join(""))
let mut arr := Array(4, 0)
arr[0] = 4
arr[1] = 2
arr[2] = 3
arr[3] = 1
arr.sort()
print(arr.toList().join(" "))
let mut owned := Array.generate(3, i => Item(3 - i, "n"))
owned.sort()
print(owned[0].key)
"#,
        "-4, 1, 2, 3, 9\n3, 1, 2, 9, -4\n-1.0 0.25 2.5\napple pear\n1,2,3\n0\nadbc\n1 2 3 4\n1\n",
    );
}

#[test]
fn error_builds_a_runtime_error_message_case() {
    prints(
        r#"
function withdraw(balance : Int, amount : Int) : Int or Error {
    if amount > balance {
        return Error("Invalid amount")
    }
    balance - amount
}
let direct := Error("direct")
print(match direct { RuntimeError.Message(message) => message, _ => "other", })
print(match withdraw(5, 9) { RuntimeError.Message(message) => message, value => value.text(), _ => "other", })
print(match withdraw(5, 1) { RuntimeError.Message(message) => message, value => value.text(), _ => "other", })
"#,
        "direct\nInvalid amount\n4\n",
    );
}

#[test]
fn strings_index_get_and_iterate_by_character() {
    prints(
        r#"
function firstTwo(word : String) : String { word[0] + word[1] }
let s := "héllo"
print(match s.get(1) { Some(c) => c, None => "none", })
print(match s.get(9) { Some(c) => c, None => "none", })
print(s.length())
let text := "abc"
for (c in text) { print(c) }
print(firstTwo("kestrel"))
print(text[2])
"#,
        "é\nnone\n5\na\nb\nc\nke\nc\n",
    );
    let output = execute("let text := \"abc\"\nprint(text[3])");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("String index out of range"));
}

#[test]
fn strict_parsing_checked_numbers_and_exact_integer_conversion() {
    prints(
        r#"
function integer(text : String) : String = match parseInt(text) {
    RuntimeError.Parse(_) => "parse", n => n.text(), _ => "other",
}
function decimal(text : String) : String = match parseDouble(text) {
    RuntimeError.Parse(_) => "parse", n => n.text(), _ => "other",
}
function whole(n : double) : String = match n.checkedToInt() {
    RuntimeError.Arithmetic(_) => "arithmetic", value => value.text(), _ => "other",
}
print(integer("-9223372036854775808"))
print(integer("9223372036854775807"))
print(integer("9223372036854775808"))
print(integer(" 12 "))
print(integer("12abc"))
print(integer("+12"))
print(decimal("-1.25e+2"))
print(decimal("1e999"))
print(decimal("nan"))
print(decimal("1.2x"))
print(whole(2.0))
print(whole(1.5))
print(whole(1.0 / 0.0))
print(match 9223372036854775807.checkedAdd(1) { RuntimeError.Arithmetic(_) => "overflow", n => n.text(), _ => "other", })
print(match (-9223372036854775808).checkedDivide(-1) { RuntimeError.Arithmetic(_) => "overflow", n => n.text(), _ => "other", })
print(match 1.0.checkedDivide(0.0) { RuntimeError.Arithmetic(_) => "nonfinite", n => n.text(), _ => "other", })
print(match 8.checkedMultiply(7) { n => n.text(), _ => "error", })
"#,
        "-9223372036854775808\n9223372036854775807\nparse\nparse\nparse\n12\n-125.0\nparse\nparse\nparse\n2\narithmetic\narithmetic\noverflow\noverflow\nnonfinite\n56\n",
    );
}

#[test]
fn contracts_keep_runtime_checks_for_unknown_inputs_and_specialize_proved_calls() {
    let source = r#"
function half(n : Int) : Int or Error requires n > 0 = n / 2
function run(n : Int) : Int or Error = half(n)
print(match run(-1) { RuntimeError.Contract(message) => message, value => value.text(), _ => "other", })
print(match run(8) { value => value.text(), _ => "error", })
print(match half(10) { value => value.text(), _ => "error", })
"#;
    let program = lower(source);
    assert_eq!(
        program
            .functions
            .iter()
            .filter(|f| f.name == "half")
            .count(),
        2
    );
    let output = execute_program(&program);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "n > 0\n4\n5\n");
}

#[test]
fn conditional_borrowed_temporaries_survive_outer_blocks() {
    prints(
        r#"
class Item {
    public final label : String
    new(label : String) { this.label = label }
}
function same(x : Item) : Item = x
let first := { if true then { let view := same(Item("yes" + "!"))
view } else same(Item("no")) }
print(first.label)
let second := { match true { true => same(Item("match" + "!")), false => same(Item("no")), } }
print(second.label)
"#,
        "yes!\nmatch!\n",
    );
}

#[test]
fn indexed_enum_lengths_erase_after_static_checks() {
    prints(
        r#"
enum Vec[T](n : Nat) {
    Nil : Vec[T](0)
    Cons(x : T, xs : Vec[T](n)) : Vec[T](n + 1)
}
function head(v : Vec[T](n + 1)) : T = match v { Cons(x, _) => x, }
let values : Vec[Int](2) = Cons(7, Cons(8, Nil))
print(head(values))
"#,
        "7\n",
    );
}

#[test]
fn mutex_guards_release_after_return_and_reentrant_locks_return_errors() {
    prints(
        r#"
function increment(m : Mutex[Int]) : Int or Error with Effects {
    using g := m.lock()? { g.value = g.value + 1
return g.value }
}
function reenter(m : Mutex[Int]) : void or Error with Effects {
    using outer := m.lock()? { using inner := m.lock()? { inner.value = 99 } }
}
let m := Mutex(0)
print(match increment(m) { n => n.text(), _ => "error", })
print(match reenter(m) { RuntimeError.Contract(_) => "reentrant", value => "unexpected", _ => "other", })
print(match increment(m) { n => n.text(), _ => "error", })
"#,
        "1\nreentrant\n2\n",
    );
}
#[test]
fn closed_channels_drain_and_failed_sends_release_owned_payloads() {
    prints(
        r#"
class Item { public final name : String
new(name : String) { this.name = name } }
let ch := Channel[Item]()
let _ := ch.send(Item("kept" + "!"))
ch.close()
ch.close()
print(match ch.send(Item("discarded" + "!")) { RuntimeError.Contract(_) => "closed", value => "unexpected", _ => "other", })
print(match ch.receive() { Some(item) => item.name, None => "empty", })
print(match ch.receive() { Some(item) => item.name, None => "empty", })
"#,
        "closed\nkept!\nempty\n",
    );
}
#[test]
fn atomic_numeric_operations_are_shared_effects_and_check_overflow() {
    prints(
        r#"
function load(a : Atomic[Int]) : Int with Effects = a.load()
let a := Atomic(4)
print(load(a))
print(match a.fetchAdd(3) { n => n.text(), _ => "error", })
print(a.compareExchange(7,9))
print(a.compareExchange(7,10))
a.store(9223372036854775807)
print(match a.fetchAdd(1) { RuntimeError.Arithmetic(_) => "overflow", n => n.text(), _ => "other", })
print(a.load())
let d := Atomic(1.5)
print(d.fetchAdd(0.5))
print(d.load())
print(d.compareExchange(2.0,3.0))
d.store(4.0)
print(d.load())
"#,
        "4\n4\ntrue\nfalse\noverflow\n9223372036854775807\n1.5\n2.0\ntrue\n4.0\n",
    );
}
#[test]
fn files_read_write_blank_lines_eof_and_early_return() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let path = dir.path().join("lines.txt");
    let quoted = format!("{:?}", path.to_str().unwrap());
    let source = format!(
        r#"
function store(path : String) : void or Error with Effects {{
    using w := create(path)? {{ w.writeLine("λ\u0000end")?
w.writeLine("")?
return }}
}}
function read(path : String) : String or Error with Effects {{
    using r := open(path)? {{
        let first := r.readLine()?
        let blank := r.readLine()?
        let last := match r.readLine() {{ RuntimeError.EndOfFile(_) => "eof", line => line, _ => "error", }}
        first + blank + last
    }}
}}
print(match store({quoted}) {{ value => "stored", _ => "error", }})
print(match read({quoted}) {{ s => s, _ => "error", }})
print(match read({quoted} + ".missing") {{ RuntimeError.Io(_) => "missing", s => s, _ => "error", }})
"#
    );
    prints(&source, "stored\nλ\0endeof\nmissing\n");
    assert_eq!(fs::read(path).unwrap(), "λ\0end\n\n".as_bytes());
}

#[test]
fn scoped_tasks_join_before_return_and_can_share_channels_and_atomics() {
    prints(
        r#"
function bump(a : Atomic[Int]) : void or Error with Effects {
    for (let i : Int = 0; i < 10000; i = i + 1) { a.fetchAdd(1)? }
}
function send(c : Channel[Int], value : Int) : void or Error with Effects = c.send(value)
function parallel(a : Atomic[Int], c : Channel[Int]) : void or Error with Effects {
    scope { spawn bump(a)
spawn bump(a)
spawn send(c,5)
spawn send(c,9) }?
}
let a := Atomic(0)
let c := Channel[Int]()
print(match parallel(a,c) { value => "joined", _ => "error", })
c.close()
let first := match c.receive() { Some(n) => n, None => 0, }
let second := match c.receive() { Some(n) => n, None => 0, }
print(first + second)
print(a.load())
function early(n : Atomic[Int]) : void with Effects {
    scope { spawn n.store(42)
return }
}
early(a)
print(a.load())
"#,
        "joined\n14\n20000\n42\n",
    );
}

#[test]
fn close_failures_are_typed_close_all_resources_and_preserve_body_errors() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let path = format!("{:?}", dir.path().join("output.txt").to_str().unwrap());
    let source = format!(
        r#"
enum Failure extends Error {{ Failed(String) }}
function normal(path : String) : String or Error with Effects ensures false {{
    using w := create(path)? {{ "ok" }}
}}
function early(path : String) : String or Error with Effects ensures false {{
    using outer := create(path)? {{ using inner := create(path + ".inner")? {{ return "ok" }} }}
}}
function fail(path : String) : String or Error with Effects {{
    using w := create(path)? {{ return Failed("primary") }}
}}
function narrow(path : String) : String or Error with Effects {{
    using w := create(path)? {{ Failed("narrow") }}
}}
print(match normal({path}) {{ RuntimeError.Io(_) => "io", value => value, _ => "other", }})
print(match early({path}) {{ RuntimeError.Io(_) => "io", value => value, _ => "other", }})
print(match fail({path}) {{ Failed(text) => text, value => value, _ => "other", }})
print(match narrow({path}) {{ Failed(text) => text, value => value, _ => "other", }})
"#
    );
    let program = lower(&source);
    let output = intercept_close(
        &program,
        "int result = fclose(file); (void)result; fputc('C',stdout); errno = EIO; return EOF;",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Cio\nCCio\nCprimary\nCnarrow\n"
    );
}
#[test]
fn owned_resources_release_in_reverse_local_and_field_declaration_order() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let mut paths = vec![];
    for label in ["A", "B", "C"] {
        let path = dir.path().join(label);
        fs::write(&path, label).unwrap();
        paths.push(format!("{:?}", path.to_str().unwrap()));
    }
    let source = format!(
        r#"
class Pair {{
    var z : Reader
    var a : Reader
    new(move z : Reader, move a : Reader) {{ this.z = z
this.a = a }}
}}
function run(a : String, b : String, c : String) : void or Error with Effects {{
    let first := open(a)?
    let second := open(b)?
    let pair := Pair(first,second)
    let last := open(c)?
    return
}}
run({},{},{})?
"#,
        paths[0], paths[1], paths[2]
    );
    let output = intercept_close(
        &lower(&source),
        "int label = fgetc(file); fputc(label,stdout); return fclose(file);",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "CBA");
}

#[test]
fn unicode_string_length_trim_full_case_mapping_and_binary_safe_contains() {
    prints(
        r#"
import std.str.{length, trim, toUpper, toLower, contains}
print(length("λ😀e\u0301\u0000"))
print(trim("\u00a0\u2003λ\u2003\u00a0"))
print(length(trim("\u2003\u202f")))
print(toUpper("Straße ﬃ λ"))
print(toLower("İ ΟΣ ΟΣΑ Σ Ο\u0301Σ"))
print(toLower("ALREADY simple"))
print(contains("ab\u0000cd", "\u0000c"))
print(contains("aaaaab", "aaaab"))
print(contains("λ", ""))
print(contains("λ", "x"))
"#,
        "5\nλ\n0\nSTRASSE FFI Λ\ni\u{0307} ος οσα σ ο\u{0301}ς\nalready simple\ntrue\ntrue\ntrue\nfalse\n",
    );
}

#[test]
fn persistent_lists_comprehensions_and_lazy_pipelines_retain_class_storage() {
    prints(
        r#"
function values() : List[double] = [1.0,2.0,3.0]
function copy(xs : List[double]) : List[double] = xs
function twice(n : double) : double = n * 2.0
let xs := values()
let ys := copy(xs)
print([x * 2.0 for x in ys if x > 1.0])
let pipeline := xs.filter(x => x > 1.0).map(x => x * 3.0)
print(pipeline.toList())
print(pipeline.toList())
print(xs.map(twice).toList())
print(xs == ys)
print(xs.contains(2.0))
print(xs.contains(9.0))
class Item {
    public final label : String
    new(label : String) { this.label = label }
}
let items := [Item("one"),Item("two")]
let selected := items.filter(x => x.label == "two").toList()
for (item in selected) { print(item.label) }
for (item in items) { print(item.label) }
function first(xs : List[Item], fallback : Item) : Item {
    for (item in xs) { return item }
    fallback
}
let fallback := Item("fallback")
let view := { first([Item("temporary")],fallback) }
print(view.label)
"#,
        "[4.0, 6.0]\n[6.0, 9.0]\n[6.0, 9.0]\n[2.0, 4.0, 6.0]\ntrue\ntrue\nfalse\ntwo\none\ntwo\ntemporary\n",
    );
}
#[test]
fn literal_string_split_preserves_empty_pieces_and_splits_unicode_scalars() {
    prints(
        r#"
import std.str.{split}
for (piece in split("a,,",",")) { print(f"<{piece}>") }
for (piece in split("λ😀", "")) { print(f"<{piece}>") }
for (piece in split("", "")) { print("unexpected") }
for (piece in split("", ",")) { print(f"<{piece}>") }
for (piece in split("aaaaa","aa")) { print(f"<{piece}>") }
"#,
        "<a>\n<>\n<>\n<λ>\n<😀>\n<>\n<>\n<>\n<a>\n",
    );
}

#[test]
fn task_errors_follow_spawn_order_and_preserve_an_earlier_body_error() {
    prints(
        r#"
function first(c : Channel[Int]) : void or Error with Effects {
    c.receive()
    RuntimeError.Parse("first")
}
function second(c : Channel[Int], done : Atomic[Int]) : void or Error with Effects {
    c.send(1)?
    done.store(2)
    RuntimeError.Parse("second")
}
function run(c : Channel[Int], done : Atomic[Int]) : void or Error with Effects {
    scope { spawn first(c)
spawn second(c,done) }?
}
function early(c : Channel[Int], done : Atomic[Int]) : Int or Error with Effects {
    let _ := scope { spawn first(c)
spawn second(c,done)
return RuntimeError.Contract("body") }
    3
}
let c := Channel[Int]()
let done := Atomic(0)
print(match run(c,done) { RuntimeError.Parse(text) => text, value => "success", _ => "other", })
print(done.load())
done.store(0)
print(match early(c,done) { RuntimeError.Contract(text) => text, value => value.text(), _ => "other", })
print(done.load())
"#,
        "first\n2\nbody\n2\n",
    );
}
#[test]
fn generic_comparison_and_atomic_double_comparison_keep_the_confirmed_ordering() {
    prints(
        r#"
function order[T : Comparable](a : T,b : T) : Int = a.compare(b)
print(order(1,2))
print(order(-0.0,0.0))
print(order(0.0 / 0.0,1.0 / 0.0))
print(order("λ","😀"))
class Value { has Comparable
public final n : Int
new(n : Int) { this.n=n }
public function compare(other : Value) : Int = this.n.compare(other.n) }
print(order(Value(5),Value(3)))
let a := Atomic(-0.0)
print(a.compareExchange(0.0,2.0))
print(a.compareExchange(-0.0,2.0))
let nan := 0.0 / 0.0
a.store(nan)
print(a.compareExchange(nan,3.0))
print(a.load())
"#,
        "-1\n-1\n1\n-1\n1\nfalse\ntrue\ntrue\n3.0\n",
    );
}

#[test]
fn persistent_values_copy_through_move_parameters_and_channel_sends() {
    prints(
        r#"
function pass(move xs : List[Int]) : List[Int] = xs
let xs := [3,4]
let ys := pass(xs)
print(xs)
print(ys)
let channel := Channel[List[Int]]()
let _ := channel.send(xs)
channel.close()
print(xs)
print(match channel.receive() { Some(items) => items, None => [], })
"#,
        "[3, 4]\n[3, 4]\n[3, 4]\n[3, 4]\n",
    );
}

#[test]
fn owned_and_borrowed_return_branches_share_one_result_layout() {
    prints(
        r#"import std.str.{length}
class Item {public final label : String
new(label : String) {this.label=label}}
function choose(item : Item, flag : Bool) : Item {
    if flag { return item }
    Item("owned")
}
function pick(item : Item, flag : Bool) : Item = if flag then item else Item("fresh")
function nested(item : Item, flag : Bool) : Item {
    if flag { return choose(item, false) }
    item
}
let item:=Item("borrowed")
print(choose(item,false).label)
print(choose(item,true).label)
print(pick(item,false).label + pick(item,true).label)
print(nested(item,true).label + nested(item,false).label)
let mut total := 0
for (let i : Int = 0; i < 3; i = i + 1) {
    let chosen := choose(item, i == 1)
    total = total + length(chosen.label)
}
print(total)
"#,
        "owned\nborrowed\nfreshborrowed\nownedborrowed\n18\n",
    );
}
#[test]
fn persistent_maps_keep_old_values_first_equal_keys_and_borrowed_payloads() {
    let source = format!(
        r#"{}
print(m.get("b"))
print(n.get("b"))
class Key {{has Equals
public final id : Int
public final label : String
new(id : Int,label : String) {{this.id=id
this.label=label}}
public function equals(other : Key) : Bool = this.id==other.id}}
let original:=Map(Key(1,"first")=>3)
let changed:=original.put(Key(1,"second"),9)
for ((key,value) in changed) {{print(key.label)
print(value)}}
print(original.get(Key(1,"lookup")))
"#,
        include_str!("../testdata/language/persistent-map.kes")
    );
    prints(&source, "None\nSome(2)\nfirst\n9\nSome(3)\n");
    prints(
        r#"
class Item {public final label : String
new(label : String) {this.label=label}}
let old:=Map("a"=>Item("old"+"!"))
let newMap:=old.put("a",Item("new"+"!"))
print(match old.get("a") {Some(item)=>item.label,None=>"none",})
print(match newMap.get("a") {Some(item)=>item.label,None=>"none",})
let view:={Map("a"=>Item("temporary"+"!")).get("a")}
print(match view {Some(item)=>item.label,None=>"none",})
"#,
        "old!\nnew!\ntemporary!\n",
    );
}
#[test]
fn maps_grow_copy_values_and_mutate_through_explicit_mut_parameters() {
    prints(
        r#"
let mut map : Map[Int,Int]=Map()
for (let i : Int = 0; i<10000; i=i+1) {map=map.put(i,i*2)}
let snapshot:=map
map=map.put(9999,7)
print(snapshot.get(9999))
print(map.get(9999))
print(map.contains(9998))
print(map.contains(20000))
function update(mut map : MutMap[String,Int]) : void {map.put("x",5)
map.put("x",6)}
let mutable : MutMap[String,Int]=MutMap()
update(mutable)
print(mutable.get("x"))
let nested:=Map("a"=>[1,2])
let values:=match nested.get("a") {Some(xs)=>xs,None=>[],}
print(values)
print(nested.get("a"))
"#,
        "Some(19998)\nSome(7)\ntrue\nfalse\nSome(6)\n[1, 2]\nSome([1, 2])\n",
    );
}

#[test]
fn constructors_return_their_initialized_object_on_bare_early_return() {
    prints(
        r#"
class Item {
    public final label : String
    public var value : Int
    new(early : Bool) {
        this.label="kept"+"!"
        this.value=1
        if early { return }
        this.value=2
    }
}
print(Item(true).value)
print(Item(false).value)
print(Item(true).label)
"#,
        "1\n2\nkept!\n",
    );
}
#[test]
fn recursive_enum_back_edges_and_owned_option_class_trees_are_boxed() {
    prints(
        r#"
enum A {End,Wrap(B)}
enum B {Back(A)}
let value:=Wrap(Back(End))
print(match value {End=>0,Wrap(Back(End))=>1,Wrap(Back(Wrap(_)))=>2,})
class Node {
    public var next : Option[Node]
    new(move next : Option[Node]) {this.next=next}
}
let node:=Node(Some(Node(None)))
print(match node.next {None=>0,Some(child)=>match child.next {None=>1,Some(_)=>2,},})
"#,
        "1\n1\n",
    );
}

#[test]
fn returns_moved_into_branches_still_release_the_skipped_block_values() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let path = format!("{:?}", dir.path().join("line.txt").to_str().unwrap());
    let source = format!(
        r#"
function files(path : String) : String or Error with Effects {{
    using w := create(path)? {{ w.writeLine("line" + "!")? }}?
    using r := open(path)? {{ r.readLine()? }}
}}
function pick(flag : Bool) : String {{
    let kept := "kept" + "!"
    let other := "other" + "!"
    if flag then kept else other
}}
print(match files({path}) {{ text => text, _ => "error", }})
print(pick(true))
print(pick(false))
"#
    );
    let program = lower(&source);
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let binary = dir.path().join("program");
    native::build(&program, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "line!\nkept!\nother!\n"
    );
    assert_no_leaks(&binary);
}

#[test]
fn trait_objects_dispatch_through_vtables_and_own_their_boxed_values() {
    let source = r#"
trait Shape { function area() : double }
class Circle { has Shape
public final r : double
new(r : double) { this.r = r }
public function area() : double = 3.0 * this.r * this.r }
class Square { has Shape
public final s : double
new(s : double) { this.s = s }
public function area() : double = this.s * this.s }
function totalArea(shapes : List[&dyn Shape]) : double {
    let mut total := 0.0
    for (s in shapes) { total = total + s.area() }
    total
}
let c := Circle(1.0)
let q := Square(2.0)
print(totalArea([c, q]))
print(c.r)
let b : Box[dyn Shape] = box(Square(3.0))
print(b.area())
let owned : List[Box[dyn Shape]] = [box(Circle(2.0)), box(Square(1.0))]
let mut sum := 0.0
for (s in owned) { sum = sum + s.area() }
print(sum)
let plain := box(Circle(1.0))
print(plain.r)
print(plain.area())
class Label { has Text
public final value : String
new(value : String) { this.value = value }
public function text() : String = "label:" + this.value }
function show(items : List[&dyn Text]) : void with Effects { for (t in items) { print(t) } }
show([Label("a" + "b"), Label("c")])
print(box(41).text())
"#;
    let program = lower(source);
    let code = native::emit_c(&program).unwrap();
    assert!(
        code.contains("k_vtables_"),
        "open traits use a table of function pointers"
    );
    let output = execute_program(&program);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "7.0\n1.0\n9.0\n13.0\n1.0\n3.0\nlabel:ab\nlabel:c\n41\n"
    );
}
#[test]
fn sealed_traits_dispatch_with_a_switch_and_boxes_reach_through_parameters() {
    let source = r#"
sealed trait Animal { function name() : String
function scaled(factor : Int, label : String) : String }
class Dog { has Animal
public final age : Int
new(age : Int) { this.age = age }
public function name() : String = "dog" + "!"
public function scaled(factor : Int, label : String) : String = label + (this.age * factor).text() }
class Cat { has Animal
new() {}
public function name() : String = "cat"
public function scaled(factor : Int, label : String) : String = label + "cat" + factor.text() }
function describe(a : &dyn Animal) : String = a.name() + " " + a.scaled(3, "x")
let pets : List[Box[dyn Animal]] = [box(Dog(2)), box(Cat())]
for (p in pets) { print(describe(p)) }
class Zoo { public var star : Box[dyn Animal]
new(move star : Box[dyn Animal]) { this.star = star } }
let zoo := Zoo(box(Dog(5)))
print(zoo.star.name())
enum Expr { Num(Int)
Add(Box[Expr], Box[Expr]) }
function eval(e : Expr) : Int = match e { Num(n) => n, Add(a, b) => eval(a) + eval(b), }
print(eval(Add(box(Num(2)), box(Add(box(Num(3)), box(Num(4)))))))
"#;
    let program = lower(source);
    let code = native::emit_c(&program).unwrap();
    assert!(code.contains("switch (self.impl)") && !code.contains("KVtable_"));
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let binary = dir.path().join("program");
    native::build(&program, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "dog! x6\ncat xcat3\ndog!\n9\n"
    );
    assert_no_leaks(&binary);
}
#[test]
fn mut_receivers_change_the_object_behind_a_trait_object() {
    prints(
        r#"
trait Increment { public mut function increment() : void
function current() : Int }
class Counter { has Increment
var n : Int
new() { this.n = 0 }
public mut function increment() : void { this.n = this.n + 1 }
public function current() : Int = this.n }
function bump(mut c : &dyn Increment) : void { c.increment()
c.increment() }
let items : List[Box[dyn Increment]] = [box(Counter())]
for (item in items) { print(item.current()) }
let c := Counter()
bump(c)
print(c.current())
"#,
        "0\n2\n",
    );
}

#[test]
fn function_values_become_counted_closures_where_they_escape() {
    let source = r#"
function add(a : Int, b : Int) : Int = a + b
class Item { public final label : String
new(label : String) { this.label = label } }
function apply(f : Int -> Int) : Int = f(1)
function greet(f : String -> String) : String = f("bob")
function transform(xs : List[Int], f : Int -> Int) : List[Int] = [f(x) for x in xs]
function twiceAll(xs : List[String], f : String -> String) : List[String] = xs.map(f).toList()
function prefixer(p : String) : String -> String = move s => p + s
function labeller(item : Item) : String = { let f := () => item.label + "?"
f() }
let item := Item("box" + "!")
let words := ["a", "b"]
print(greet(s => s + "!" + item.label))
print(greet(prefixer("hi " + "")))
let hello := prefixer("hello ")
let copy := hello
print(copy("x"))
print(hello("y"))
print(transform([1, 2, 3], x => x * x))
print(twiceAll(words, w => w + w))
let scale := 3
let fs : List[Int -> Int] = [move x => x + scale, move x => x * scale, add(100, _)]
print(fs.map(f => f(10)).toList())
print(labeller(item))
function run(f : () -> String) : String = f()
print(run(() => item.label + "-" + hello("z")))
let mut counter := 0
let bump := () => { counter = counter + 1 }
bump()
bump()
print(counter)
function adders() : List[Int -> Int] = [move x => x + 1, move x => x + 2]
for (f in adders()) { print(f(5)) }
"#;
    let program = lower(source);
    assert!(!program.closures.is_empty());
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let binary = dir.path().join("program");
    native::build(&program, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "bob!box!
hi bob
hello x
hello y
[1, 4, 9]
[aa, bb]
[13, 30, 110]
box!?
box!-hello z
2
6
7
"
    );
    assert_no_leaks(&binary);
}
#[test]
fn mutating_closures_run_through_mut_parameters_and_reassigned_function_variables() {
    prints(
        r#"
class Counter { var n : Int
new() { this.n = 0 }
public mut function increment() : Int { this.n = this.n + 1
this.n } }
function twice(mut f : () -> void) : void { f()
f() }
function repeat(n : Int, mut step : () -> Int) : Int { let mut last := 0
for (let i : Int = 0; i < n; i = i + 1) { last = step() }
last }
let mut total := 0
let bump := () => { total = total + 1 }
twice(bump)
print(total)
let c := Counter()
let tick := move () => c.increment()
print(repeat(5, tick))
let shared := move (x : Int) => x * 2
let copy := shared
print(shared(2) + copy(3))
function one(n : Int) : Int = n + 1
function two(n : Int) : Int = n + 2
let mut f := one
f = two
print(f(1))
let g := if total > 1 then one else two
print(g(1))
function each(xs : List[Int], f : Int -> void with Effects) : void with Effects { for (x in xs) { f(x) } }
each([7, 8], x => print(x))
"#,
        "2
5
10
3
2
7
8
",
    );
}

#[test]
fn c_headers_supply_signatures_and_boundary_numbers_check_their_range() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("local.h"),
        "static inline int twice(int x) { return 2 * x; }\nstatic inline unsigned long long big(void) { return 18446744073709551615ULL; }\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("app.kes"),
        r#"import c "math.h"
import c "stdlib.h"
import c "local.h"
import std.c.{CInt32, CFloat32}
let r := unsafe { sqrt(2.0) }
print(r)
function hyp(a : double, b : double) : double with Effects = unsafe { hypot(a, b) }
print(hyp(3.0, 4.0))
function absolute(n : Int) : Int or Error with Effects {
    let c := CInt32(n)?
    unsafe { abs(c) }.toInt()
}
print(match absolute(-7) { n => n.text(), _ => "error", })
print(match absolute(9999999999) { RuntimeError.Arithmetic(m) => m, n => n.text(), _ => "error", })
print(unsafe { lround(2.6) }.toInt())
function root(x : double) : double or Error with Effects {
    let f := CFloat32(x)?
    unsafe { sqrtf(f) }.toDouble()
}
print(match root(16.0) { n => n.text(), _ => "error", })
print(match root(10000000000000000000000000000000000000000.0) { RuntimeError.Arithmetic(m) => m, n => n.text(), _ => "error", })
function local(n : Int) : Int or Error with Effects = unsafe { twice(CInt32(n)?) }.toInt()
print(match local(21) { n => n.text(), _ => "error", })
print(match unsafe { big() }.checkedToInt() { RuntimeError.Arithmetic(m) => m, n => n.text(), _ => "error", })
"#,
    )
    .unwrap();
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "1.4142135623730951\n5.0\n7\nValue does not fit CInt32\n3\n4.0\nValue does not fit CFloat32\n42\nCUInt64 value is larger than Int\n"
    );
}

#[test]
fn c_text_parameters_copy_strings_and_project_c_files_compile_beside_headers() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::create_dir(dir.path().join("lib")).unwrap();
    fs::write(
        dir.path().join("lib/vowels.h"),
        "int vowels(const char *text);\nvoid shout(const char *restrict text, int times);\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("lib/vowels.c"),
        "#include <stdio.h>\n#include \"vowels.h\"\nint vowels(const char *text) { int n = 0; for (; *text; text++) n += *text == 'a' || *text == 'e' || *text == 'i' || *text == 'o' || *text == 'u'; return n; }\nvoid shout(const char *restrict text, int times) { while (times-- > 0) printf(\"%s!\\n\", text); fflush(stdout); }\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("app.kes"),
        r#"import c "string.h"
import c "stdlib.h"
import c "lib/vowels.h"
import std.c.{CInt32}
function size(text : String) : String with Effects =
    match unsafe { strlen(text) }.checkedToInt() { n => n.text(), _ => "error", }
let word := "kestrel"
print(size(word))
print(unsafe { strcmp("abc", "abd") }.toInt() < 0)
print(unsafe { atoi("42") }.toInt())
print(unsafe { vowels("education") }.toInt())
function twice(text : String) : void or Error with Effects {
    unsafe { shout(text + "?", CInt32(2)?) }
}
let done := twice("ok")
print(size("a\u0000b"))
"#,
    )
    .unwrap();
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let program = core::lower(&project).unwrap();
    assert_eq!(program.c_sources.len(), 1);
    let output = execute_program(&program);
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "7\ntrue\n42\n5\nok?!\nok?!\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("contains a NUL character"));
}

fn python_available(module: &str) -> bool {
    Command::new("python3-config")
        .arg("--includes")
        .output()
        .is_ok_and(|o| o.status.success())
        && Command::new("python3")
            .args(["-c", &format!("import {module}")])
            .output()
            .is_ok_and(|o| o.status.success())
}
#[test]
fn python_values_cross_the_boundary_by_copy_and_are_checked_when_they_return() {
    if !python_available("json") {
        eprintln!("skipped: no embeddable CPython");
        return;
    }
    prints(
        r#"
import python "builtins" as py
import python "json" as json
import python "no_such_module_here" as missing
import std.python.{PythonValue}
function make() : PythonValue or Error with Effects = py.list([3, 1, 2])
function work(n : Int, sum : Atomic[Int]) : void or Error with Effects {
    let total : Int or Error = py.sum([n, n, n])
    let old := sum.fetchAdd(total?)?
}
function many(sum : Atomic[Int]) : void or Error with Effects {
    scope { spawn work(1, sum)
spawn work(2, sum)
spawn work(3, sum) }?
}
function pairText() : String or Error with Effects {
    let pair : (Int, String) or Error = py.tuple((7, "seven"))
    let (n, s) := pair?
    s + n.text()
}
function run() : void or Error with Effects {
    let xs := make()?
    let none : void or Error = xs.append(9)
    print(match none { RuntimeError.Python(m) => m, _ => "appended", })
    let sorted : List[Int] or Error = py.sorted(xs)
    print(match sorted { items => items.text(), _ => "error", })
    let text : String or Error = json.dumps(Map("b" => [1, 2], "a" => [3]), sort_keys = true)
    print(match text { t => t, _ => "error", })
    let back : Map[String, List[Int]] or Error = json.loads("{\"x\": [1, 2], \"y\": []}")
    print(match back { m => m.text(), _ => "error", })
    print(match pairText() { t => t, _ => "error", })
    let flag : Bool or Error = py.bool(1)
    print(match flag { b => b.text(), _ => "error", })
    let notInt : Int or Error = py.bool(1)
    print(match notInt { RuntimeError.Python(m) => m, n => n.text(), _ => "other", })
    let unique : Set[String] or Error = py.set(["a", "b", "a"])
    print(match unique { s => s.contains("a").text(), _ => "error", })
    let gone : Int or Error = missing.anything
    print(match gone { RuntimeError.Python(m) => m, n => n.text(), _ => "other", })
    let word : String or Error = py.str("λ" + "!")
    print(match word { w => w, _ => "error", })
    let sum := Atomic(0)
    print(match many(sum) { RuntimeError.Python(m) => m, _ => "tasks done", })
    print(sum.load())
}
run()?
"#,
        "appended
[1, 2, 3, 9]
{\"a\": [3], \"b\": [1, 2]}
Map(x => [1, 2], y => [])
seven7
true
expected Int, found bool
true
ModuleNotFoundError: No module named 'no_such_module_here'
λ!
tasks done
18
",
    );
}
#[test]
fn numpy_results_follow_the_expected_type() {
    if !python_available("numpy") {
        eprintln!("skipped: numpy is not installed");
        return;
    }
    prints(
        r#"
import python "numpy" as np
import python "math" as pymath
import std.python.{PythonValue}
function run() : void or Error with Effects {
    let n : Int or Error = np.add(1, 2)
    print(match n { value => value.text(), _ => "error", })
    let a := np.array([1.0, 2.0, 3.5])?
    let total : double or Error = a.sum()
    print(match total { value => value.text(), _ => "error", })
    let size : Int or Error = a.size
    print(match size { value => value.text(), _ => "error", })
    let z := np.zeros(3, dtype = "float64")?
    let zs : List[double] or Error = z.tolist()
    print(match zs { xs => xs.text(), _ => "error", })
    let bad : Int or Error = pymath.sqrt(4.0)
    print(match bad { RuntimeError.Python(m) => m, value => value.text(), _ => "other", })
    let failing : Int or Error = pymath.sqrt(-1.0)
    print(match failing { RuntimeError.Python(m) => m, value => value.text(), _ => "other", })
}
run()?
"#,
        "3
6.5
3
[0.0, 0.0, 0.0]
expected Int, found float
ValueError: math domain error
",
    );
}

#[test]
fn boxed_strings_trees_and_closures_free_each_block_once() {
    let program = lower(
        r#"
enum Tree { Leaf
Node(Tree, Tree) }
function make(depth : Int) : Tree = if depth == 0 then Node(Leaf, Leaf) else Node(make(depth - 1), make(depth - 1))
function check(t : Tree) : Int = match t { Leaf => 0, Node(l, r) => 1 + check(l) + check(r), }
enum Chain { End
Link(String, Chain) }
let mut chain := End
for (let i : Int = 0; i < 2000; i = i + 1) { chain = Link(i.text(), chain) }
print(match chain { Link(s, _) => s, End => "", })
print(check(make(10)))
let words := box("hi" + "!")
print(words.text())
let prefix := "p" + ":"
let tag := move (s : String) => prefix + s
print(tag("x"))
let mut text := ""
for (let i : Int = 0; i < 100; i = i + 1) { text = text + i.text() }
let copy := text
text = text + "!"
print(copy.text().text())
print(text)
"#,
    );
    let output = execute_program(&program);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let digits = (0..100).map(|i: i32| i.to_string()).collect::<String>();
    assert_eq!(
        String::from_utf8(output.stdout.clone()).unwrap(),
        format!("1999\n2047\nhi!\np:x\n{digits}\n{digits}!\n")
    );
    assert_sanitizer_clean(&program, &output);
}

#[test]
fn user_iterables_expression_defaults_and_lambdas_that_return_early() {
    prints(
        r#"
class Countdown { has Iterator
var n : Int
new(n : Int) { this.n = n }
public mut function next() : Option[Int] {
    if this.n == 0 { return None }
    this.n = this.n - 1
    Some(this.n + 1)
} }
class Range { has Iterable
public final top : Int
new(top : Int) { this.top = top }
public function items() : Countdown = Countdown(this.top) }
let mut total := 0
for (n in Range(4)) { total = total + n }
print(total)
print([n * 10 for n in Range(3)])
const BASE : Int = 2 * 21
function scaled(n : Int, factor : Int = BASE + 1) : Int = n * factor
print(scaled(2))
function firstBig(xs : List[Int]) : Int {
    let pick := (limit : Int) : Int => {
        for (x in xs) { if x > limit { return x } }
        0
    }
    pick(5)
}
print(firstBig([1, 7, 9]))
"#,
        "10
[30, 20, 10]
86
7
",
    );
}

/// A fixture whose last `last` lines are values a session would show, printed instead.
fn fixture_with_prints(name: &str, last: usize) -> String {
    let text = fs::read_to_string(format!(
        "{}/testdata/language/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let lines = text.lines().collect::<Vec<_>>();
    let split = lines.len() - last;
    let shown = lines[split..].iter().map(|line| format!("print({line})"));
    lines[..split]
        .iter()
        .map(|line| line.to_string())
        .chain(shown)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn every_runtime_acceptance_fixture_produces_its_expected_result() {
    prints(&fixture_with_prints("braces.kes", 0), "{1, 2}\n");
    prints(&fixture_with_prints("double-zero.kes", 1), "false\n");
    prints(&fixture_with_prints("panel.kes", 1), "4.0\n");
    prints(&fixture_with_prints("debug.kes", 1), "square of side 2.0\n");
    prints(
        &fixture_with_prints("persistent-map.kes", 2),
        "None\nSome(2)\n",
    );
    prints(&fixture_with_prints("scope.kes", 1), "Some(5)\n");
    let errors = fixture_with_prints("errors.kes", 0);
    let errors = errors.replace(
        "let _ := total(\"\", \"1\")\ntotalOrZero(\"\")",
        "print(match total(\"\", \"1\") { Parse(m) => f\"Parse(\\\"{m}\\\")\", n => n.text(), _ => \"other\", })\nprint(totalOrZero(\"\"))",
    );
    prints(&errors, "Parse(\"\")\n0\n");
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("rules.kes"),
        include_str!("../testdata/language/rules.kes"),
    )
    .unwrap();
    fs::write(
        dir.path().join("app.kes"),
        "import rules.{pipesAndCurrying, mutation, sideList}\nprint(pipesAndCurrying())\nprint(mutation())\nprint(sideList())",
    )
    .unwrap();
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "12.0\n2.0\n[1.0, 2.0, 3.0]\n"
    );
}

#[test]
fn arrays_change_in_place_and_stop_on_a_bad_index() {
    prints(
        r#"
class Board {
    var cells : Array[String]
    new(size : Int) { this.cells = Array(size, ".") }
    public mut function mark(i : Int, text : String) : void { this.cells[i] = text }
    public function row() : String {
        let mut out := ""
        for (let i : Int = 0; i < this.cells.length(); i = i + 1) { out = out + this.cells[i] }
        out
    }
}
let mut b := Board(4)
b.mark(1, "x")
b.mark(3, "o")
print(b.row())
let words := Array(2, "a")
words[1] = words[0] + "b"
print(words[1])
print(match words.get(1) { Some(w) => w, None => "none", })
print(match words.get(-1) { Some(w) => w, None => "none", })
let lists := Array(2, [1, 2])
lists[0] = [3]
print(lists[0].text() + lists[1].text())
function bump(mut xs : Array[Int]) : void {
    for (let i : Int = 0; i < xs.length(); i = i + 1) { xs[i] = xs[i] + i }
}
let ns := Array(3, 10)
bump(ns)
print(ns[0] + ns[1] + ns[2])
let moved := ns
print(moved.length())
let empty := Array(0, 0)
print(empty.length())
"#,
        ".x.o\nab\nab\nnone\n[3][1, 2]\n33\n3\n0\n",
    );
    for (source, message) in [
        (
            "let a := Array(2, 0)\nprint(a[2])",
            "Array index out of range",
        ),
        (
            "let a := Array(2, 0)\na[-1] = 3",
            "Array index out of range",
        ),
        (
            "let n := -1\nlet a := Array(n, 0)",
            "Array size is negative",
        ),
    ] {
        let output = execute(source);
        assert!(!output.status.success(), "{source}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn power_and_shifts_follow_q11_and_q52() {
    prints(
        "print(2 * 3 ** 2)\nprint((-2) ** 3)\nprint(-(2 ** 2))\nprint(2 ** (3 ** 2))\nprint(7 ** 0)\nprint(2.0 ** 0.5)\nprint(1 << 3)\nprint(1 << 63)\nprint(-8 >> 1)\nprint(-1 >> 63)\nprint(5 >> 1)\nlet n := 7\nprint(n > 0 && n < 10 || n == 100)\n",
        "18\n-8\n-4\n512\n1\n1.4142135623730951\n8\n-9223372036854775808\n-4\n-1\n2\ntrue\n",
    );
    for (source, message) in [
        ("let e := -1\nprint(2 ** e)", "negative exponent"),
        ("print(3 ** 40)", "Int power overflow"),
        ("let c := 64\nprint(1 << c)", "Shift count outside 0 to 63"),
        ("let c := -1\nprint(1 >> c)", "Shift count outside 0 to 63"),
    ] {
        let output = execute(source);
        assert!(!output.status.success(), "{source}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{source}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn bound_method_values_and_method_partials_keep_their_receiver() {
    prints(
        r#"
class Counter {
    public var count : Int
    var label : String
    new(label : String) { this.count = 0; this.label = label }
    public mut function increment() : void { this.count = this.count + 1 }
    public function scaled(by : Int, plus : Int) : Int = this.count * by + plus
    public function name(suffix : String) : String = this.label + suffix
}
function twice(mut f : () -> void) : void { f(); f() }
function apply(f : Int -> Int, x : Int) : Int = f(x)
let mut c := Counter("c")
let bump := c.increment
bump()
twice(c.increment)
print(c.count)
let s := c.scaled
print(s(10, 1))
let p := c.scaled(_, 5)
print(p(2))
print(apply(c.scaled(_, 1), 4))
let n := c.name
print(n("!"))
print([1, 2, 3].map(c.scaled(_, 0)).toList().text())
"#,
        "3\n31\n11\n13\nc!\n[3, 6, 9]\n",
    );
}

#[test]
fn a_stored_method_value_becomes_a_closure_over_its_receiver() {
    prints(
        r#"
class Counter {
    public var count : Int
    new(count : Int) { this.count = count }
    public function scaled(by : Int) : Int = this.count * by
    public function offset(by : Int, plus : Int) : Int = this.count * by + plus
}
class Holder {
    var f : Int -> Int
    new(f : Int -> Int) { this.f = f }
    public function run(x : Int) : Int = this.f(x)
}
function demo(c : Counter) : Int {
    let h := Holder(c.scaled)
    let g := Holder(c.offset(_, 1))
    h.run(5) + g.run(10)
}
print(demo(Counter(4)))
"#,
        "61\n",
    );
}

#[test]
fn tail_calls_forward_borrowed_and_mut_parameters_and_owned_result_boxes() {
    prints(
        r#"
class Node {public final value : Int
new(value : Int) {this.value=value}}
function sum(node : Node, n : Int, total : Int) : Int {
    if n == 0 { return total }
    sum(node, n - 1, total + node.value)
}
function last(mut counter : Node, n : Int) : Int {
    if n == 0 { return counter.value }
    last(counter, n - 1)
}
function walk(node : Node, n : Int) : Node {
    if n == 0 { return node }
    if n == 5 { return Node(55) }
    walk(node, n - 1)
}
let node := Node(2)
print(sum(node, 3000000, 0))
let mut c := Node(5)
print(last(c, 3000000))
print(walk(node, 3000000).value + walk(node, 3).value)
"#,
        "6000000\n5\n57\n",
    );
    let refused = lower_errors(
        "class Node {public final value : Int\nnew(value : Int) {this.value=value}}\nfunction f(node : Node, n : Int) : Int {\n    if n == 0 { return node.value }\n    let local := Node(n)\n    f(local, n - 1)\n}\nprint(f(Node(1), 3))",
    );
    assert!(refused.iter().any(|code| code == "E_NATIVE_TAIL_CALL"));
}

fn lower_errors(source: &str) -> Vec<String> {
    let project = ProjectCheck {
        modules: vec![check_source("native.kes", source)],
    };
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    core::lower(&project)
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|e| e.diagnostic.code.to_string())
        .collect()
}

#[test]
fn tail_cycles_with_different_signatures_share_an_argument_block() {
    prints(
        r#"
class Step {public final size : Int
new(size : Int) {this.size=size}}
function first(n : Int) : Bool {
    if n == 0 { return true }
    second(n - 1, "x")
}
function second(n : Int, tag : String) : Bool {
    if n == 0 { return false }
    third(n - 1, tag + "!", Step(1).size)
}
function third(n : Int, tag : String, k : Int) : Bool {
    if n == 0 { return tag == "y" }
    first(n - k)
}
function down(s : Step, n : Int) : Int {
    if n <= 0 { return n }
    up(n - s.size, s, 7)
}
function up(n : Int, s : Step, extra : Int) : Int {
    if n <= 0 { return extra }
    down(s, n - 1)
}
print(first(3000000))
print(first(3000001))
print(down(Step(2), 3000000))
print(down(Step(2), 3000001))
"#,
        "true\nfalse\n0\n7\n",
    );
}

#[test]
fn trait_objects_view_and_box_built_in_values() {
    prints(
        r#"
class Point { has Text
    public final x : Int
    new(x : Int) { this.x = x }
    public function text() : String = "P" + this.x.text()
}
function show(item : &dyn Text) : String = item.text()
let n := 42
let name := "kes"
let p := Point(1)
let xs := [1, 2]
print(show(n) + " " + show(name) + " " + show(p) + " " + show(xs) + " " + show(2.5))
let items : List[&dyn Text] = [n, name, p, 3, "lit" + "eral"]
for (let item : &dyn Text in items) { print(item.text()) }
print(items)
let owned : List[Box[dyn Text]] = [box(7), box("seven"), box(Point(7)), box([8, 9])]
for (let item : Box[dyn Text] in owned) { print(item.text()) }
"#,
        "42 kes P1 [1, 2] 2.5\n42\nkes\nP1\n3\nliteral\n[42, kes, P1, 3, literal]\n7\nseven\nP7\n[8, 9]\n",
    );
}

#[test]
fn iterator_trait_objects_bind_their_item_type() {
    prints(
        r#"
class Countdown { has Iterator
    var n : Int
    new(n : Int) { this.n = n }
    public mut function next() : Option[Int] {
        if this.n == 0 { return None }
        this.n = this.n - 1
        Some(this.n)
    }
}
class Twice { has Iterator
    var left : Int
    new(left : Int) { this.left = left }
    public mut function next() : Option[Int] {
        if this.left == 0 { return None }
        this.left = this.left - 1
        Some(100)
    }
}
function drain(mut cursor : &dyn Iterator(Item = Int)) : Int with Effects {
    let mut sum := 0
    let mut more := true
    while (more) {
        match cursor.next() {
            Some(v) => { sum = sum + v },
            None => { more = false },
        }
    }
    sum
}
function pick(first : Bool) : Box[dyn Iterator(Item = Int)] = if first then box(Countdown(4)) else box(Twice(2))
let mut a := pick(true)
let mut b := pick(false)
print(drain(a) + drain(b))
let mut c := Countdown(3)
print(drain(c))
"#,
        "206\n3\n",
    );
}

#[test]
fn kestrel_functions_cross_into_python_as_callables() {
    if !python_available("functools") {
        eprintln!("skipped: no embeddable CPython");
        return;
    }
    prints(
        r#"import std.str.{length}
import python "builtins" as builtins
import python "functools" as functools
function double(x : Int) : Int = x * 2
function run() : void or Error with Effects {
    let doubled : List[Int] = builtins.list(builtins.map(double, [1, 2, 3])?)?
    print(doubled)
    let offset := 10
    let shifted : List[Int] = builtins.list(builtins.map(move (x : Int) => x + offset, [1, 2])?)?
    print(shifted)
    let ordered : List[String] = builtins.sorted(["bb", "a", "ccc"], key = (s : String) => 0 - length(s))?
    print(ordered)
    let total : Int = functools.reduce((a : Int, b : Int) => a + b, [1, 2, 3, 4])?
    print(total)
    let bad : Int or Error = functools.reduce((a : Int, b : Int) => a + b, ["x", "y"])
    print(match bad { RuntimeError.Python(m) => m, n => n.text(), _ => "other", })
}
function text() : void or Error with Effects {
    let xs := builtins.list([3, 1, 2])?
    let s : String = builtins.str(xs)?
    print(s)
}
print(match run() { RuntimeError.Python(m) => m, _ => "done", })
print(match text() { RuntimeError.Python(m) => m, _ => "done", })
"#,
        "[2, 4, 6]\n[11, 12]\n[ccc, bb, a]\n10\nTypeError: expected Int, found str\ndone\n[3, 1, 2]\ndone\n",
    );
}

#[test]
fn list_patterns_check_lengths_before_payloads_and_retain_rest_entries() {
    prints(
        r#"
class Item { has Text
public final label : String
new(label : String) { this.label = label }
public function text() : String = this.label }
function rest(xs : List[Item]) : List[Item] {
    let [_, ..tail] := xs otherwise { return [] }
    tail
}
function classify(xs : List[List[Int]]) : String = match xs {
    [[1, 2], ..tail] if tail == [[3]] => "nested",
    [[_, _, _, ..], ..] => "long",
    [[], ..] => "empty head",
    _ => "other",
}
print(classify([]))
print(classify([[]]))
print(classify([[1, 2], [3]]))
print(classify([[1]]))
print(classify([[1, 2, 3]]))
print(rest([Item("first"), Item("second"), Item("third")]))
print(rest([]))
function patterns() : void with Effects {
let [one, ..all] := [1, 2, 3] otherwise { return }
print(one)
print(all)
let [..whole] := ["a", "b"]
print(whole)
}
patterns()
let mut total := 0
for (let i : Int = 0; i < 100; i = i + 1) {
    total = total + match [i, i + 1] {
        [_, ..tail] if tail == [] => 0,
        [x, ..tail] => x,
        _ => 0,
    }
}
print(total)
"#,
        "other\nempty head\nnested\nother\nlong\n[second, third]\n[]\n1\n[2, 3]\n[a, b]\n4950\n",
    );
}

#[test]
fn custom_iterable_pipelines_delay_traversal_until_consumed() {
    prints(
        r#"
class Countdown { has Iterator
var n : Int
new(n : Int) { this.n = n }
public mut function next() : Option[Int] {
    if this.n == 0 { return None }
    let current := this.n
    this.n = this.n - 1
    Some(current)
} }
class Range { has Iterable
public final top : Int
new(top : Int) { this.top = top }
public function items() : Countdown = Countdown(this.top) }
function scaled[T : Iterable(Item = Int)](mut source : T) : List[Int] with Effects =
    source.filter(x => (x % 2) == 0).map(x => x * 10).toList()
print(Range(5).filter(x => x > 2).map(x => x * 2).toList())
print(Range(3).toList())
print(scaled(Range(4)))
class LoudRange { has Iterable
new() {}
public function items() : Countdown with Effects { print("traverse")
Countdown(2) } }
function delayed() : void with Effects {
    let pipeline := LoudRange().map(x => x + 1)
    print("created")
    print(pipeline.toList())
    print(pipeline.filter(x => x == 3).toList())
}
delayed()
"#,
        "[10, 8, 6]\n[3, 2, 1]\n[40, 20]\ncreated\ntraverse\n[3, 2]\ntraverse\n[3]\n",
    );
}

#[test]
fn native_scripts_receive_only_their_arguments_as_unicode_strings() {
    let program = lower(
        "import std.process.{args}\nfor (arg in args) { print(\"<\" + arg + \">\") }\nfunction values() : List[String] = args\nprint(values())",
    );
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let binary = dir.path().join("arguments");
    native::build(&program, &binary).unwrap();
    let run = Command::new(&binary)
        .args(["hello world", "λ", "", "--help"])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8(run.stdout.clone()).unwrap(),
        "<hello world>\n<λ>\n<>\n<--help>\n[hello world, λ, , --help]\n"
    );
    if std::env::var_os("KESTREL_SANITIZE").is_some() {
        assert_sanitizer_clean_with_args(&program, &run, &["hello world", "λ", "", "--help"]);
    }
    if std::env::var_os("KESTREL_LEAK_CHECK").is_some() {
        assert_no_leaks_with_args(&binary, &["hello world", "λ", "", "--help"]);
    }
}

#[test]
fn maps_and_sets_compare_by_their_entries() {
    prints(
        r#"
class P { has Equals
    public final x : Int
    new(x : Int) { this.x = x }
    public function equals(other : P) : Bool = this.x == other.x
}
print(Map("a" => 1, "b" => 2) == Map("b" => 2, "a" => 1))
print(Map("a" => 1) == Map("a" => 2))
print(Map("a" => 1) == Map("b" => 1))
print(Map("a" => 1) == Map("a" => 1, "b" => 2))
print(Set(1, 2) == Set(2, 1))
print(Set(1, 2) == Set(1, 3))
print(Map(1 => P(1)) == Map(1 => P(1)))
print(Map(1 => P(1)) == Map(1 => P(2)))
print(Map("k" => [1, 2]) == Map("k" => [1, 2]))
print(SortedSet("a", "b") == SortedSet("b", "a"))
"#,
        "true\nfalse\nfalse\nfalse\ntrue\nfalse\ntrue\nfalse\ntrue\ntrue\n",
    );
}

#[test]
fn python_callbacks_accept_owned_handles_and_preserve_error_details() {
    if !python_available("builtins") {
        return;
    }
    prints(
        r#"import std.python.{PythonValue}
import python "builtins" as py
import python "functools" as functools
enum CallbackError extends Error { Failed(String, Int, Bool, double) }
function identity(value : PythonValue) : PythonValue = value
function take(move value : PythonValue) : PythonValue = value
function size(value : PythonValue) : Int or Error with Effects = py.len(value)
function pairSize(pair : (PythonValue, PythonValue)) : Int or Error with Effects {
    let (a, b) := pair
    size(a)? + size(b)?
}
function reject(value : PythonValue) : Int or Error = CallbackError.Failed("bad λ", 7, true, 2.5)
function closed(value : PythonValue) : Int or CallbackError or RuntimeError = CallbackError.Failed("closed", 9, false, 1.0)
function sumLengths(values : List[PythonValue]) : Int or Error with Effects {
    let mut sum := 0
    for (value in values) { sum = sum + size(value)? }
    for (value in values) { sum = sum + size(value)? }
    sum
}
function checked(first : PythonValue, second : Int) : Int = second
function run() : void or Error with Effects {
    let handle := py.list([1, 2, 3])?
    print(size(handle)?)
    print(size(handle)?)
    let same := identity(handle)
    print(size(same)?)
    let values : List[String] = py.list(py.map(identity, ["a", "bc"])?)?
    print(values)
    let taken : List[String] = py.list(py.map(take, ["x", "yz"])?)?
    print(taken)
    let sizes : List[Int] = py.list(py.map(size, ["a", "bc"])?)?
    print(sizes)
    let pairs : List[Int] = py.list(py.map(pairSize, [("a", "bc"), ("de", "f")])?)?
    print(pairs)
    let nested : List[Int] = py.list(py.map(sumLengths, [["a", "bc"], ["d"]])?)?
    print(nested)
    let closedFailure : List[Int] or Error = py.list(py.map(closed, ["x"])?)
    print(match closedFailure { RuntimeError.Python(message) => message, _ => "wrong", })
    let failed : List[Int] or Error = py.list(py.map(reject, ["x"])?)
    print(match failed { RuntimeError.Python(message) => message, _ => "wrong", })
    let mismatch : Int or Error = functools.reduce(checked, ["first", "wrong"])
    print(match mismatch { RuntimeError.Python(message) => message, _ => "wrong", })
}
print(match run() { RuntimeError.Python(message) => message, _ => "done", })
"#,
        "3\n3\n3\n[a, bc]\n[x, yz]\n[1, 2]\n[3, 3]\n[6, 2]\nRuntimeError: CallbackError.Failed(\"closed\", 9, false, 1.0)\nRuntimeError: CallbackError.Failed(\"bad λ\", 7, true, 2.5)\nTypeError: expected Int, found str\ndone\n",
    );
}

#[test]
fn scalar_collection_storage_preserves_snapshots_through_growth_and_replacement() {
    prints(
        r#"import std.test.{assertEq}
function count(values : Map[Int, Int]) : Int {
    let mut n := 0
    for (_ in values) { n = n + 1 }
    n
}
function run() : void with Effects {
    let mut values : Map[Int, Int] = Map()
    for (let i : Int = 0; i < 64; i = i + 1) { values = values.put(i, i * 2) }
    let earlier := values
    for (let i : Int = 0; i < 2048; i = i + 1) { values = values.put(i, i * 3) }
    assertEq(count(earlier), 64)
    assertEq(earlier.get(3), Some(6))
    assertEq(count(values), 2048)
    assertEq(values.get(3), Some(9))
    let items := [v + k for (k, v) in earlier]
    let [_, ..rest] := items otherwise { return }
    let saved := rest
    let copied := [n for n in saved]
    assertEq(rest, copied)
    let zeros := Map(-0.0 => 1).put(0.0, 2)
    for ((key, value) in zeros) { print(key); print(value) }
    let flags := Map(false => 2.5, true => 3.5)
    assertEq(flags.get(false), Some(2.5))
    assertEq(flags.get(true), Some(3.5))
    let doubles := [1.5, 2.5, 3.5]
    let [_, ..tail] := doubles otherwise { return }
    assertEq(tail, [2.5, 3.5])
    assertEq([true, false], [true, false])
    print(count(earlier))
    print(count(values))
}
run()
"#,
        "-0.0\n2\n64\n2048\n",
    );
}

#[test]
fn array_factories_create_distinct_owners_and_borrow_their_elements() {
    prints(
        r#"class Point {
public final label : String
new(label : String) {this.label=label}
}
function create() : Array[Point] = Array.generate(3, i => Point(i.text() + "!"))
function make(i : Int) : Point with Effects { print(i)
Point(i.text()) }
let points := create()
print(points[0].label)
print(points[2].label)
print(match points.get(1) { Some(point) => point.label, None => "missing", })
points[1] = Point("replaced" + "!")
print(points[1].label)
let empty := Array.generate(0, make)
print(empty.length())
let others := Array.generate(2, make)
print(others[1].label)
let mut count := 0
let mut factory := (i : Int) => { count = count + 1
Point((i + count).text()) }
let counted := Array.generate(3, factory)
print(count)
print(counted[2].label)
let nested := Array.generate(2, i => Array.generate(2, j => Point((i + j).text())))
print(nested[1][1].label)
let boxes := Array.generate(2, i => box(Point(i.text())))
print(boxes[1].label)
print(match boxes.get(0) {Some(point) => point.label, None => "missing",})
let simple := Array(2, 7)
print(simple[1])
let map := Map(1 => box(Point("mapped" + "!")))
print(match map.get(1) {Some(point) => point.label, None => "missing",})
print(match map.get(1) {Some(point) => point.label, None => "missing",})
"#,
        "0!\n2!\n1!\nreplaced!\n0\n0\n1\n1\n3\n5\n2\n1\n0\n7\nmapped!\nmapped!\n",
    );
    let stopped = execute(
        "function make(i : Int) : Int with Effects {print(i)\ni}\nlet xs := Array.generate(-1, make)",
    );
    assert!(!stopped.status.success());
    assert!(stopped.stdout.is_empty());
    assert!(String::from_utf8_lossy(&stopped.stderr).contains("Array size is negative"));
}

#[test]
fn opaque_c_pointers_preserve_targets_constness_and_text_arguments() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("pointers.h"),
        r#"#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <string.h>
typedef struct Handle Handle;
Handle *make_handle(int32_t value);
const Handle *view_handle(Handle *value);
int32_t read_handle(const Handle *value);
void free_handle(Handle *value);
void *identity_void(void *value);
const void *identity_const(const void *value);
Handle *null_handle(void);
char echo_char(char value);
const char *fixed_text(void);
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("pointers.c"),
        r#"#include "pointers.h"
#include <stdlib.h>
struct Handle {int32_t value;};
Handle *make_handle(int32_t value) {Handle *h=malloc(sizeof *h);if(h) h->value=value;return h;}
const Handle *view_handle(Handle *h) {return h;}
int32_t read_handle(const Handle *h) {return h->value;}
void free_handle(Handle *h) {free(h);}
void *identity_void(void *h) {return h;}
const void *identity_const(const void *h) {return h;}
Handle *null_handle(void) {return NULL;}
char echo_char(char value) {return value;}
const char *fixed_text(void) {return "hello";}
"#,
    )
    .unwrap();
    let source = r#"import std.c.{CInt32, CChar, CConstPointer}
import c "pointers.h"
function read(handle : CConstPointer[Handle]) : Int with Effects = unsafe {read_handle(handle).toInt()}
function run() : void or Error with Effects {
    let handle := unsafe {make_handle(CInt32(42)?)}
    print(handle.isNull())
    let second := handle
    print(read(second))
    let view := unsafe {view_handle(handle)}
    print(unsafe {read_handle(view).toInt()})
    print(unsafe {identity_void(handle).isNull()})
    print(unsafe {identity_const(view).isNull()})
    print(unsafe {null_handle().isNull()})
    let letter := CChar(65)?
    print(unsafe {echo_char(letter).toInt()})
    print(unsafe {strlen("abc").checkedToInt()?})
    let text := unsafe {fixed_text()}
    print(unsafe {strlen(text).checkedToInt()?})
    print(unsafe {strcmp(text, "hello").toInt()})
    unsafe {free_handle(handle)}
    print(match CChar(9999) { RuntimeError.Arithmetic(_) => "range", _ => "wrong", })
}
print(match run() { RuntimeError.Io(message) => message, _ => "done", })
"#;
    let file = dir.path().join("pointers.kes");
    fs::write(&file, source).unwrap();
    let project = check_path(&file, dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let program = core::lower(&project).unwrap();
    let output = execute_program(&program);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "false\n42\n42\nfalse\nfalse\ntrue\n65\n3\n5\n0\nrange\ndone\n"
    );
    for body in [
        "let h := unsafe {view_handle(null_handle())}\nunsafe {free_handle(h)}",
        "let h := unsafe {view_handle(null_handle())}\nunsafe {identity_void(h)}",
        "unsafe {free_handle(fixed_text())}",
        "let h := make_handle(CInt32(1))",
    ] {
        fs::write(
            &file,
            format!("import std.c.{{CInt32}}\nimport c \"pointers.h\"\n{body}"),
        )
        .unwrap();
        assert!(!check_path(&file, dir.path()).unwrap().is_ok(), "{body}");
    }
}

#[test]
fn dynamic_iterables_hide_owned_and_borrowed_cursors_and_clean_up_early_exits() {
    prints(
        r#"class Counter { has Iterator
public var i : Int
public final label : String
new(i : Int) {this.i=i
this.label="cursor" + "!"}
public mut function next() : Option[Int] = if this.i == 0 then None else {this.i=this.i - 1
Some(this.i)} }
class Range { has Iterable
public final count : Int
new(count : Int) {this.count=count}
public function items() : Counter = Counter(this.count) }
class Saved { has Iterable
public var cursor : Counter
new(count : Int) {this.cursor=Counter(count)}
public function items() : Counter = this.cursor }
class Source { has Iterable
public final count : Int
new(count : Int) {this.count=count}
public function items() : Borrowing = Borrowing(this) }
class Borrowing { has Iterator
public final source : &Source
public var i : Int
new(source : Source) {this.source=source
this.i=source.count}
public mut function next() : Option[Int] = if this.i == 0 then None else {this.i=this.i - 1
Some(this.i + this.source.count)} }
function total(mut values : &dyn Iterable(Item = Int)) : Int with Effects {
let mut sum := 0
for (value in values) {sum=sum+value}
sum }
function first(mut values : &dyn Iterable(Item = Int)) : Int with Effects {
for (value in values) {return value}
-1 }
function until(mut values : &dyn Iterable(Item = Int)) : Int with Effects {
let mut sum := 0
for (value in values) {if value == 1 {break}
sum=sum+value}
sum }
function pipeline(mut values : &dyn Iterable(Item = Int)) : List[Int] with Effects = values.map(x => x + 1).filter(x => x > 2).toList()
let range := Range(4)
print(total(range))
let boxed : Box[dyn Iterable(Item = Int)] = box(Range(5))
print(total(boxed))
print(first(range))
print(until(range))
print(pipeline(range))
let saved := Saved(4)
print(first(saved))
print(total(saved))
print(total(saved))
let source := Source(3)
print(total(source))
print(first(source))
print(until(source))
"#,
        "6\n10\n3\n5\n[4, 3]\n3\n3\n0\n12\n5\n12\n",
    );
}

#[test]
fn built_in_trait_views_and_collection_cursors_dispatch_without_losing_snapshots() {
    prints(
        r#"function product(value : &dyn Multipliable[Int](Output = Int)) : Int = value.multiply(3)
function right(value : &dyn RightMultipliable[double](Output = double)) : double = value.rightMultiply(2.0)
function show(value : &dyn Text) : String = value.text()
function total(mut values : &dyn Iterable(Item = Int)) : Int with Effects {
let mut sum := 0
for (value in values) {sum=sum+value}
sum }
function manual(mut values : &dyn Iterable(Item = Int)) : Int with Effects {
let mut cursor := values.items()
let first := match cursor.next() {Some(n) => n, None => -1,}
let second := match cursor.next() {Some(n) => n, None => -1,}
first + second }
function strings(mut values : &dyn Iterable(Item = String)) : String with Effects {
let mut text := ""
for (value in values) {text=text+value}
text }
function entries(mut values : &dyn Iterable(Item = (Int, Int))) : Int with Effects {
let mut total := 0
for ((key,value) in values) {total=total+key+value}
total }
print(product(7))
let boxed : Box[dyn Multipliable[Int](Output = Int)] = box(8)
print(product(boxed))
print(right(1.5))
let numbers := [1, 2, 3]
print(show(numbers))
print(total(numbers))
let owned : Box[dyn Iterable(Item = Int)] = box([4, 5])
print(total(owned))
print(manual(numbers))
let mut cursor := numbers.items()
print(match cursor.next() {Some(n) => n, None => -1,})
let words := ["a" + "b", "c" + "d"]
print(strings(words))
let set := Set(1, 2, 3)
print(total(set))
let sorted := SortedSet(1, 2, 3)
print(total(sorted))
let map := Map(1 => 2, 3 => 4)
print(entries(map))
"#,
        "21\n24\n3.0\n[1, 2, 3]\n6\n9\n3\n1\nabcd\n6\n6\n10\n",
    );
}

#[test]
fn runtime_completion_syntax_fixtures_execute_verbatim() {
    prints(
        include_str!("../testdata/language/35-array-factories.kes"),
        "0\n9\n",
    );
    prints(
        include_str!("../testdata/language/36-opaque-c-pointers.kes"),
        "released\n",
    );
    prints(
        include_str!("../testdata/language/37-dynamic-iteration.kes"),
        "6\n",
    );
}

#[test]
fn theorem_requirements_and_simp_are_erased_before_native_execution() {
    let source = "theorem atZero(n : Nat) : n = Zero requires h : n = Zero {h}\nfunction guarded(n : Nat) : Nat or Error requires n == Zero {n}\nfunction useFact(n : Nat) : Nat or Error requires atZero(n) {guarded(n)}\nprint(match useFact(Zero) {n => n.text(),_ => \"error\",})";
    let program = lower(source);
    assert!(!native::emit_c(&program).unwrap().contains("n == Zero"));
    let output = execute_program(&program);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"0\n");
    prints(
        &format!(
            "function add(a : Nat,b : Nat) : Nat = match a {{Zero => b,Succ(k) => Succ(add(k,b)),}}\n{}\nprint(count(3))",
            include_str!("../testdata/language/29-tactics.kes")
        ),
        "3\n",
    );
    prints(
        "theorem atZero(n : Nat) : n = Zero requires h : n = Zero {h}\nfunction onlyZero(n : Nat) : Nat requires atZero(n) {n}\nprint(onlyZero(Zero))",
        "0\n",
    );
}

#[test]
fn nat_arithmetic_traps_and_checked_alternatives_preserve_exact_values() {
    prints(
        "let n : Nat = 3\nprint(n + 1)\nprint(1 + n)\nprint(n - 1)\nlet max : Nat = 18446744073709551615\nprint(match max.checkedAdd(1) {RuntimeError.Arithmetic(message) => message,_ => \"wrong\",})\nprint(match n.checkedSubtract(4) {RuntimeError.Arithmetic(message) => message,_ => \"wrong\",})\nprint(match n.checkedAdd(4) {n => n.text(),_ => \"wrong\",})\nprint(match max.checkedSubtract(1) {n => n.text(),_ => \"wrong\",})",
        "4\n4\n2\nNat addition overflow\nNat subtraction below zero\n7\n18446744073709551614\n",
    );
    for (source, message) in [
        (
            "let n : Nat = 18446744073709551615\nprint(n + 1)",
            "Nat addition overflow",
        ),
        (
            "let n : Nat = 0\nprint(n - 1)",
            "Nat subtraction below zero",
        ),
    ] {
        let output = execute(source);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(message));
    }
}

#[test]
fn arithmetic_proof_fixture_executes_without_a_solver_at_run_time() {
    if !std::process::Command::new("z3")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        return;
    }
    prints(
        include_str!("../testdata/language/38-arithmetic-proofs.kes"),
        "4\n2\n",
    );
}

#[test]
fn defined_proof_arithmetic_keeps_the_runtime_guard() {
    prints(
        include_str!("../testdata/language/43-defined-proof-arithmetic.kes"),
        "5\nblocked\n",
    );
}

#[test]
fn bounded_proofs_and_requirements_erase_before_native_execution() {
    prints(
        include_str!("../testdata/language/boundedproofs.kes"),
        "6\nbounded\n",
    );
}

#[test]
fn propositional_identities_do_not_bypass_custom_or_float_equality_checks() {
    prints(
        r#"class Key {has Equals
new() {}
public function equals(other : Key) : Bool = false}
function repeat(n : Nat, x : T) : T = match n {Zero => x, Succ(k) => repeat(k,x),}
theorem repeatSame[T : Equals](n : Nat, x : T) : repeat(n,x) = x {induction n {Zero => rfl, Succ(k) ih => ih,}}
function checked[T : Equals](n : Nat,x : T) : Bool or Error requires repeat(n,x) == x = true
function useFact[T : Equals](n : Nat,x : T) : Bool or Error requires repeatSame(n,x) = checked(n,x)
function floatFact(n : Nat,x : double) : Bool or Error requires repeatSame(n,x) = checked(n,x)
let key := Key()
print(match useFact(2,key) { RuntimeError.Contract(_) => "custom guarded", _ => "bad", })
print(match floatFact(2,0.0 / 0.0) { RuntimeError.Contract(_) => "NaN guarded", _ => "bad", })
print(match useFact(2,7) { value => value.text(), _ => "error", })
"#,
        "custom guarded\nNaN guarded\ntrue\n",
    );
}

#[test]
fn imported_pure_function_proofs_and_private_helpers_execute() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = check_path(root.join("testdata/language/importedproofs.kes"), root).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let program = core::lower(&project).unwrap();
    let output = execute_program(&program);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"7\n");
}

#[test]
fn dynamic_class_items_retain_owners_when_collected_and_release_on_early_exit() {
    prints(
        r#"class Point {
public final label : String
new(label : String) {this.label=label}
}
class Cursor { has Iterator
public var index : Int
new() {this.index=0}
public mut function next() : Option[Point] = if this.index == 4 then None else {
this.index=this.index+1
Some(Point("p" + this.index.text())) }
}
class Source { has Iterable
new() {}
public function items() : Cursor = Cursor()
}
function collect(mut values : &dyn Iterable(Item = Point)) : List[Point] with Effects = values.toList()
function filtered(mut values : &dyn Iterable(Item = Point)) : List[Point] with Effects = values.filter(p => p.label != "p2").toList()
function labels(mut values : &dyn Iterable(Item = Point)) : List[String] with Effects = values.map(p => p.label).toList()
function first(mut values : &dyn Iterable(Item = Point)) : String with Effects {
for (p in values) {return p.label}
"empty" }
function skip(mut values : &dyn Iterable(Item = Point)) : Int with Effects {
let mut n := 0
for (p in values) {if p.label == "p1" {continue}
if p.label == "p3" {break}
n=n+1}
n }
function fresh() : List[Point] with Effects {
let source : Box[dyn Iterable(Item = Point)] = box(Source())
collect(source) }
function fromList() : List[Point] with Effects {
let source : Box[dyn Iterable(Item = Point)] = box([Point("a" + "!"),Point("b" + "!")])
collect(source) }
function mapCopy(mut values : &dyn Iterable(Item = (Int, Point))) : List[(Int, Point)] with Effects = values.toList()
function fromMap() : List[(Int, Point)] with Effects {
let source : Box[dyn Iterable(Item = (Int, Point))] = box(Map(1 => Point("m" + "!")))
mapCopy(source) }
let produced := fresh()
let original := fromList()
let entries := fromMap()
for (p in produced) {print(p.label)}
for (p in original) {print(p.label)}
for ((key,p) in entries) {print(key);print(p.label)}
let source : Box[dyn Iterable(Item = Point)] = box(Source())
print(first(source))
print(skip(source))
for (p in filtered(source)) {print(p.label)}
print(labels(source))
function manual(mut source : &dyn Iterable(Item = Point)) : void with Effects {
let cursor := source.items()
let Some(p) := cursor.next() otherwise {return}
print(p.label)
let Some(q) := cursor.next() otherwise {return}
print(q.label)
}
manual(source)
"#,
        "p1\np2\np3\np4\na!\nb!\n1\nm!\np1\n1\np1\np3\np4\n[p1, p2, p3, p4]\np1\np2\n",
    );
}

#[test]
fn step_items_copy_their_fields_before_the_cursor_advances() {
    prints(
        r#"class Point {
public final label : String
public var n : Int
new(label : String) {this.label=label
this.n=0}
public mut function bump() : void {this.n=this.n+1}
}

function consume(move p : Point) : String = p.label
class Cursor { has Iterator
public var index : Int
new() {this.index=0}
public mut function next() : Option[Point] = if this.index == 3 then None else {
this.index=this.index+1
Some(Point("p" + this.index.text())) }
}
class Source { has Iterable
new() {}
public function items() : Cursor = Cursor()
}
function pair(mut values : &dyn Iterable(Item = Point)) : List[String] with Effects { let cursor := values.items()
let Some(p) := cursor.next() otherwise { return [] }
let label := p.label
let Some(q) := cursor.next() otherwise { return [label] }
[label, q.label] }
function labels(mut values : &dyn Iterable(Item = Point)) : List[String] with Effects = [p.label for p in values]
let source : Box[dyn Iterable(Item = Point)] = box(Source())
print(pair(source))
print(labels(source))
let list : Box[dyn Iterable(Item = Point)] = box([Point("a" + "!"), Point("b" + "!")])
print(pair(list))
print(labels(list))
"#,
        "[p1, p2]\n[p1, p2, p3]\n[a!, b!]\n[a!, b!]\n",
    );
}

#[test]
fn erased_cursors_copy_copyable_items_from_stored_options() {
    prints(
        r#"class Point {public final label : String
new(label : String) {this.label=label}}
class Cursor {has Iterator
public var value : Option[String]
public var index : Int
new() {this.value=None
this.index=0}
public mut function next() : Option[String] {
if this.index == 3 then {this.value=None} else {
this.index=this.index+1
this.value=Some("p" + this.index.text())}
this.value}
}
class Source {has Iterable
new() {}
public function items() : Cursor = Cursor()}
class ListCursor {has Iterator
public var value : Option[List[Point]]
public var index : Int
new() {this.value=None
this.index=0}
public mut function next() : Option[List[Point]] {
if this.index == 3 then {this.value=None} else {
this.index=this.index+1
this.value=Some([Point("p" + this.index.text())])}
this.value}
}
class ListSource {has Iterable
new() {}
public function items() : ListCursor = ListCursor()}
class Empty {has Iterator
public final value : Option[Int]
new() {this.value=None}
public function next() : Option[Int] = this.value}
class Mixed {has Iterator
public final value : Option[Int]
public var first : Bool
new() {this.value=Some(7)
this.first=true}
public mut function next() : Option[Int] = if this.first then {
this.first=false
this.value} else None}
function strings(mut source : &dyn Iterable(Item = String)) : List[String] with Effects = source.toList()
function lists(mut source : &dyn Iterable(Item = List[Point])) : List[List[Point]] with Effects = source.toList()
function fresh() : List[String] with Effects {
let source : Box[dyn Iterable(Item = String)] = box(Source())
strings(source)}
function freshLists() : List[List[Point]] with Effects {
let source : Box[dyn Iterable(Item = List[Point])] = box(ListSource())
lists(source)}
print(fresh())
for (group in freshLists()) {for (p in group) {print(p.label)}}
let empty : Box[dyn Iterator(Item = Int)] = box(Empty())
print(empty.next())
let mixed : Box[dyn Iterator(Item = Int)] = box(Mixed())
print(mixed.next())
print(mixed.next())
let cursor : Box[dyn Iterator(Item = String)] = box(Cursor())
let first := cursor.next()
let second := cursor.next()
print(first)
print(second)
print(cursor.next())
print(cursor.next())
"#,
        "[p1, p2, p3]\np1\np2\np3\nNone\nSome(7)\nNone\nSome(p1)\nSome(p2)\nSome(p3)\nNone\n",
    );
}

#[test]
fn stored_noncopyable_cursor_items_lend_for_one_step() {
    prints(
        r#"
class Point { public final label : String
new(label : String) {this.label=label} }
class Cursor { has Iterator
public final value : Option[Point]
public var done : Bool
new(label : String) {this.value=Some(Point(label))
this.done=false}
public mut function next() : Option[Point] = if this.done then None else {
    this.done=true
    this.value
}
}
let cursor : Box[dyn Iterator(Item = Point)] = box(Cursor("first"))
match cursor.next() {Some(p) => print(p.label), None => print("none")}
print(match cursor.next() {Some(_) => "some", None => "none"})
class Source { has Iterable
public final label : String
new(label : String) {this.label=label}
public function items() : Cursor = Cursor(this.label) }
function count(mut values : &dyn Iterable(Item = Point)) : Int with Effects {
    let mut n := 0
    for (_ in values) { n = n + 1 }
    n
}
let values : Box[dyn Iterable(Item = Point)] = box(Source("x"))
print(count(values))
"#,
        "first\nnone\n1\n",
    );
}

#[test]
fn numeric_maps_find_every_key_after_growth_and_miss_absent_ones() {
    prints(
        r#"function check(n : Int) : Int {
    let mut m : Map[Int, Int] = Map()
    for (let i : Int = 0; i < n; i = i + 1) { m = m.put(i * 4294967296, i) }
    let half := m
    for (let i : Int = 0; i < n; i = i + 1) { m = m.put(i * 4294967296, i + 1) }
    let mut found := 0
    for (let i : Int = 0; i < n; i = i + 1) {
        found = found + match m.get(i * 4294967296) { Some(v) => v - i, None => 100, }
        found = found + match half.get(i * 4294967296) { Some(v) => v - i, None => 100, }
        if m.contains((i * 4294967296) + 1) { found = found + 1000 }
    }
    found
}
print(check(5000))
let mut d : Map[double, Bool] = Map()
for (let i : Int = 0; i < 300; i = i + 1) { d = d.put(i.toDouble() / 4.0, true) }
print(d.contains(74.75))
print(d.contains(74.8))
"#,
        "5000\ntrue\nfalse\n",
    );
}

#[test]
fn chunked_channels_deliver_every_owned_item_across_producers_and_consumers() {
    prints(
        r#"function produce(ch : Channel[String], start : Int, done : Atomic[Int]) : void or Error with Effects {
    for (let i : Int = 0; i < 3000; i = i + 1) { ch.send((start + i).text())? }
    if done.fetchAdd(1)? == 1 { let _ := ch.close() }
}
function consume(ch : Channel[String], total : Atomic[Int], count : Atomic[Int]) : void or Error with Effects {
    let mut running := true
    while running {
        match ch.receive() {
            Some(text) => {
                let added := total.fetchAdd(parseInt(text)?)?
                let seen := count.fetchAdd(1)?
            },
            None => { running = false },
        }
    }
}
function run() : void or Error with Effects {
    let ch := Channel[String]()
    let total := Atomic(0)
    let count := Atomic(0)
    let done := Atomic(0)
    scope {
        spawn consume(ch, total, count)
        spawn produce(ch, 0, done)
        spawn produce(ch, 3000, done)
        spawn consume(ch, total, count)
    }?
    print(total.load())
    print(count.load())
    print(match ch.send("late") { RuntimeError.Contract(_) => "closed", _ => "open", })
    let left := Channel[String]()
    for (let i : Int = 0; i < 600; i = i + 1) { left.send("item " + i.text())? }
    for (let i : Int = 0; i < 10; i = i + 1) { let item := left.receive() }
    print(match left.receive() { Some(text) => text, None => "empty", })
}
print(match run() { RuntimeError.Parse(_) => "parse", _ => "done", })
"#,
        "17997000\n6000\nclosed\nitem 10\ndone\n",
    );
}

#[test]
fn the_standard_nat_runs_as_an_unsigned_number_and_stops_on_overflow() {
    prints(
        r#"function add(a : Nat, b : Nat) : Nat = match a {
    Zero    => b,
    Succ(k) => Succ(add(k, b)),
}
function double(n : Nat) : Nat = match n { Zero => Zero, Succ(k) => Succ(Succ(double(k))), }
theorem addZeroL(n : Nat) : add(Zero, n) = n { rfl }
let three : Nat = 3
let five := add(three, 2)
print(five)
print(five == 5)
print(five > three)
print(double(21))
print(Succ(Succ(Nat.Zero)))
let xs : List[Nat] = [1, 2, 3]
print(xs)
let m : Map[Nat, String] = Map(1 => "one")
print(m.get(1))
print(match Succ(Succ(Zero)) { Zero => "zero", Succ(Zero) => "one", Succ(Succ(k)) => f"two plus {k}", })
let big : Nat = 18446744073709551615
print(big)
"#,
        "5\ntrue\ntrue\n42\n2\n[1, 2, 3]\nSome(one)\ntwo plus 0\n18446744073709551615\n",
    );
    let stopped = execute("let big : Nat = 18446744073709551615\nprint(Succ(big))");
    assert_eq!(stopped.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&stopped.stderr).contains("Nat overflow"));
}

#[test]
fn arena_cycles_are_checked_and_released_as_a_group() {
    prints(
        r#"
import std.memory.{Arena, ArenaRef}
class Node {
    public final label : String
    public final next : Option[ArenaRef[Node]]
    new(label : String, move next : Option[ArenaRef[Node]]) {
        this.label = label
        this.next = next
    }
}
function show(nodes : Arena[Node], h : ArenaRef[Node]) : void with Effects {
    match nodes.get(h) {
        Some(node) => {
            print(node.label)
            match node.next {
                Some(next) => match nodes.get(next) {Some(link) => print(link.label), None => print("missing"),},
                None => print("end"),
            }
        },
        None => print("missing"),
    }
}
function graph() : void or Error with Effects {
    let nodes := Arena[Node]()
    let a := nodes.insert(Node("a", None))
    let b := nodes.insert(Node("b", Some(a)))
    nodes.replace(a, Node("a", Some(b)))?
    show(nodes, a)
    show(nodes, b)
    let other := Arena[Node]()
    print(match other.get(a) {Some(n) => n.label, None => "foreign",})
    match other.replace(a, Node("discard", None)) {
        void => print("unexpected"),
        RuntimeError.Contract(message) => print(message),
        _ => print("other failure"),
    }
}
graph()?
"#,
        "a\nb\nb\na\nforeign\nArena handle does not belong to this arena\n",
    );
}

#[test]
fn arena_handles_survive_growth_moves_and_expired_owners() {
    prints(
        r#"
import std.memory.{Arena, ArenaRef}
function expired() : ArenaRef[Int] {
    let arena := Arena[Int]()
    arena.insert(99)
}
function add(mut arena : Arena[Int], n : Int) : ArenaRef[Int] = arena.insert(n)
function take(move arena : Arena[Int], h : ArenaRef[Int]) : Int = match arena.get(h) {Some(n) => n, None => -1,}
let stale := expired()
let arena := Arena[Int]()
let first := add(arena, 7)
for (let i : Int = 0; i < 1000; i = i + 1) { arena.insert(i) }
print(first == first)
print(first == stale)
print(match arena.get(stale) {Some(n) => n, None => -1,})
let second := arena.insert(8)
print(first == second)
let map := Map(first => 12, second => 15)
print(match map.get(first) {Some(n) => n, None => -1,})
print(take(arena, first))
"#,
        "true\nfalse\n-1\nfalse\n12\n7\n",
    );
}

#[test]
fn arena_get_retains_copyable_payloads_and_borrows_boxes() {
    prints(
        r#"
import std.memory.{Arena, ArenaRef}
class Point {
    public final x : Int
    new(x : Int) {this.x = x}
}
function textValue() : String {
    let arena := Arena[String]()
    let key := arena.insert("a" + "b")
    match arena.get(key) {Some(s) => s, None => "missing",}
}
function run() : void or Error with Effects {
    let arena := Arena[Box[Point]]()
    let key := arena.insert(box(Point(3)))
    print(match arena.get(key) {Some(p) => p.x, None => -1,})
    arena.replace(key, box(Point(4)))?
    print(match arena.get(key) {Some(p) => p.x, None => -1,})
    print(textValue())
}
run()?
"#,
        "3\n4\nab\n",
    );
}

#[test]
fn arena_replacement_releases_old_or_rejected_values_and_drops_in_reverse_order() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let mut paths = vec![];
    for label in ["A", "B", "C", "D"] {
        let path = dir.path().join(label);
        fs::write(&path, label).unwrap();
        paths.push(format!("{:?}", path.to_str().unwrap()));
    }
    let source = format!(
        r#"
import std.memory.{{Arena}}
function run() : void or Error with Effects {{
    let a := Arena[Reader]()
    let first := a.insert(open({})?)
    a.insert(open({})?)
    a.replace(first,open({})?)?
    let foreign := Arena[Reader]()
    let failure := foreign.replace(first,open({})?)
}}
run()?
"#,
        paths[0], paths[1], paths[2], paths[3]
    );
    let output = intercept_close(
        &lower(&source),
        "int label = fgetc(file); fputc(label,stdout); return fclose(file);",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"ADBC");
}

#[test]
fn arenas_store_nested_owners_and_dynamic_values() {
    prints(
        r#"
import std.memory.{Arena, ArenaRef}
class Counter {
    var n : Int
    new() {this.n = 0}
    public mut function bump() : void {this.n = this.n + 1}
}
function nested() : void or Error with Effects {
    let inner := Arena[Int]()
    let key := inner.insert(42)
    let outer := Arena[Arena[Int]]()
    let group := outer.insert(inner)
    print(match outer.get(group) {Some(a) => match a.get(key) {Some(n) => n, None => -1,}, None => -2,})
    let values := Arena[List[Int]]()
    let list := values.insert([3,4])
    let copy := match values.get(list) {Some(xs) => xs, None => [],}
    values.replace(list,[5])?
    print(copy)
    let functions := Arena[(Int) -> String]()
    let suffix := "x" + "!"
    let functionKey := functions.insert(move n : Int => n.text() + suffix)
    let saved := match functions.get(functionKey) {Some(f) => f, None => n : Int => n.text(),}
    functions.replace(functionKey, n : Int => n.text())?
    print(saved(1))
    let changing := Arena()
    let counter := Counter()
    let step := move n : Int => { counter.bump(); n.text() }
    let changeKey := changing.insert(step)
    print(match changing.get(changeKey) {Some(_) => "stored", None => "missing",})
    let text := Arena[Box[dyn Text]]()
    let h := text.insert(box("hello"))
    print(match text.get(h) {Some(value) => value.text(), None => "missing",})
    text.replace(h,box(7))?
    print(match text.get(h) {Some(value) => value.text(), None => "missing",})
}
nested()?
"#,
        "42\n[3, 4]\n1x!\nstored\nhello\n7\n",
    );
}

#[test]
fn arena_syntax_fixture_traverses_a_cycle() {
    prints(include_str!("../testdata/language/39-arenas.kes"), "b\na\n");
    let project = ProjectCheck {
        modules: vec![check_source(
            "arena.kes",
            "import std.memory.{Arena}\nlet a := Arena[void]()",
        )],
    };
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    assert!(
        core::lower(&project)
            .unwrap_err()
            .iter()
            .any(|error| error.diagnostic.code == "E_NATIVE_TYPE")
    );
}

#[test]
fn exponent_literals_run_in_functions_patterns_contracts_and_interpolation() {
    prints(
        include_str!("../testdata/language/40-exponent-literals.kes"),
        "1000.0\n0.000250\n",
    );
    prints(
        r#"
function positive(n : double) : double or Error requires n >= 1e-3 {n}
function select(n : double) : String = match n { 1e3 => "large", -2.5E-4 => "small", _ => "other", }
function run() : void or Error with Effects {
    print(positive(1e3)?)
    print(select(1E+3))
    print(select(-2.5e-4))
    print(f"{1e-3:.4f}")
    let u : double or Int = 2e0
    print(match u {2e0 => "double", _ => "other",})
    print(0xDEAD)
    print(0x1e3)
}
run()?
"#,
        "1000.0\nlarge\nsmall\n0.0010\ndouble\n57005\n483\n",
    );
}

#[test]
fn user_associated_types_run_through_generics_and_dynamic_dispatch() {
    prints(
        include_str!("../testdata/language/41-associated-types.kes"),
        "7\n",
    );
    prints(
        r#"
trait Source {
    type Item
    function next() : Option[Item]
}
class Counter {has Source
    public var count : Int
    new(n : Int) {this.count = n}
    public function next() : Option[Int] = Some(this.count)
}
function read[T : Source(Item = Int)](source : T) : Int = match source.next() {Some(n) => n, None => -1,}
function dynamic(source : &dyn Source(Item = Int)) : Int = match source.next() {Some(n) => n, None => -1,}
function apply(f : (&dyn Source(Item = Int)) -> Int, source : &dyn Source(Item = Int)) : Int = f(source)
let counter := Counter(7)
print(read(counter))
print(apply(dynamic,counter))
let owned : Box[dyn Source(Item = Int)] = box(Counter(8))
print(apply(dynamic,owned))
"#,
        "7\n7\n8\n",
    );
}

#[test]
fn associated_protocols_preserve_mutation_effects_and_several_slots() {
    prints(
        r#"
trait PairSource {type First
    type Second
    function first() : First
    function second() : Second
}
class Pair {has PairSource
    new() {}
    public function first() : Int = 3
    public function second() : String = "a" + "b"
}
function show(value : &dyn PairSource(First = Int, Second = String)) : void with Effects {
    print(value.first())
    print(value.second())
}
function apply(f : (&dyn PairSource(First = Int, Second = String)) -> void with Effects, value : &dyn PairSource(First = Int, Second = String)) : void with Effects = f(value)
let pair := Pair()
apply(show,pair)
trait Step {type Item
    mut function next() : Option[Item] with Effects
}
class Counter {has Step
    public var n : Int
    new() {this.n = 0}
    public mut function next() : Option[Int] with Effects {this.n = this.n + 1
        Some(this.n)}
}
function take(mut source : &dyn Step(Item = Int)) : Int with Effects = match source.next() {Some(n) => n, None => 0,}
let counter := Counter()
print(take(counter))
print(take(counter))
trait Factory {type Product
    function make() : Product
}
class Product {
    public final label : String
    new(label : String) {this.label = label}
}
class Maker {has Factory
    new() {}
    public function make() : Product = Product("made" + "!")
}
function label(factory : &dyn Factory(Product = Product)) : String = factory.make().label
let maker : Box[dyn Factory(Product = Product)] = box(Maker())
print(label(maker))
trait Convert[Input] {type Output
    function convert(value : Input) : Output
}
class Converter {has Convert[String]
    new() {}
    public function convert(value : String) : Int = 9
}
function convert(value : &dyn Convert[String](Output = Int)) : Int = value.convert("x")
let converter : Box[dyn Convert[String](Output = Int)] = box(Converter())
print(convert(converter))
"#,
        "3\nab\n1\n2\nmade!\n9\n",
    );
}

#[test]
fn namespace_standard_functions_constants_types_and_function_values_execute() {
    prints(
        include_str!("../testdata/language/42-namespace-imports.kes"),
        "3.0\n2\n",
    );
    prints(
        r#"
import std.math as m
import std.str
import std.collections as c
import std.memory as mem
let root := m.sqrt
print(root(9.0))
print(str.length("λx"))
let values : c.List[Int] = [1,2]
print(values)
let arena := mem.Arena[Int]()
let key : mem.ArenaRef[Int] = arena.insert(7)
print(match arena.get(key) {Some(n) => n, None => 0,})
print(m.PI > 3.0)
"#,
        "3.0\n2\n[1, 2]\n7\ntrue\n",
    );
}

#[test]
fn filesystem_namespaces_execute_public_functions_constants_classes_and_cases() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::create_dir_all(dir.path().join("geometry")).unwrap();
    fs::write(dir.path().join("geometry/shape.kes"), "public const LIMIT : Int = 10\npublic class Point {public final x : Int\nnew(x : Int) {this.x = x}}\npublic enum Color {Red,Blue}\npublic function area(p : Point) : Int = p.x\npublic function add(a : Int,b : Int) : Int = a+b").unwrap();
    fs::write(dir.path().join("main.kes"), "import geometry . shape as g\nlet p : g . Point = g.Point(3)\nlet area := g.area\nprint(area(p) + g.LIMIT)\nfunction factory() : Int -> Int = g.add(2,_)\nlet add := factory()\nprint(add(3))\nprint(match g.Color.Red {g.Color.Red => 1, g.Color.Blue => 2,})").unwrap();
    let project = check_path(dir.path().join("main.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let program = core::lower(&project).unwrap();
    let output = execute_program(&program);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"13\n5\n1\n");
}

#[test]
fn length_indexed_append_runs() {
    let output = execute(
        "enum Vec[T](n : Nat) {\n    Nil : Vec[T](0)\n    Cons(x : T, xs : Vec[T](n)) : Vec[T](n + 1)\n}\nfunction append(move xs : Vec[T](n), move ys : Vec[T](m)) : Vec[T](n + m) = match xs {\n    Nil => ys,\n    Cons(x, rest) => Cons(x, append(rest, ys)),\n}\nfunction size(v : Vec[T](n)) : Nat = match v {\n    Nil => 0,\n    Cons(_, rest) => size(rest) + 1,\n}\nfunction head(v : Vec[T](n + 1)) : T = match v {\n    Cons(x, _) => x,\n}\nlet a : Vec[Int](2) = Cons(1, Cons(2, Nil))\nlet b : Vec[Int](1) = Cons(3, Nil)\nlet joined : Vec[Int](3) = append(a, b)\nprint(size(joined))\nprint(head(joined))\n",
    );
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "3\n1\n");
}

#[test]
fn fixed_length_multiplication_executes_without_runtime_indices() {
    prints(include_str!("../testdata/language/lengths.kes"), "6\n12\n");
}

#[test]
fn multiterm_length_matches_execute_with_indices_erased() {
    prints(
        &format!(
            "{}\nclass Point {{public final label : String\nnew(label : String) {{this.label=label}}}}\nfunction head(xs : Vec[T](n + 1)) : T = match xs {{Cons(x,_) => x,}}\nlet pointLeft : Vec[Point](1) = Cons(Point(\"left\"),Nil)\nlet pointRight : Vec[Point](0) = Nil\nlet points : Vec[Point](1) = rebuild(Cons(Point(\"owned\" + \" value\"),Nil),pointLeft,pointRight)\nprint(head(points).label)\nprint(head(points).label)",
            include_str!("../testdata/language/lengthmatches.kes")
        ),
        "3\nowned value\nowned value\n",
    );
    prints(
        include_str!("../testdata/language/46-length-matches.kes"),
        "4\n",
    );
}

#[test]
fn owned_recursive_payloads_move_into_tail_calls() {
    let source = r#"
enum Chain[T] { End, Link(T, Chain[T]) }
function reverse(move xs : Chain[T], move acc : Chain[T]) : Chain[T] = match xs {
    End => acc,
    Link(x, rest) => reverse(rest, Link(x, acc)),
}
function first(xs : Chain[T], fallback : T) : T = match xs {End => fallback,Link(x,_) => x,}
function total(xs : Chain[Int], acc : Int) : Int = match xs {End => acc,Link(x,rest) => total(rest,acc + x),}
let mut xs : Chain[Int] = End
for (let i : Int = 0; i < 100000; i = i + 1) {xs = Link(i,xs)}
let reversed := reverse(xs,End)
print(first(reversed,-1))
print(total(reversed,0))
print(first(reversed,-1))
let words : Chain[String] = Link("a" + "b",Link("c" + "d",End))
let backwards := reverse(words,End)
print(first(backwards,"empty"))
"#;
    let program = lower(source);
    assert!(!program.tail_groups.is_empty());
    let c = native::emit_c(&program).unwrap();
    assert!(c.contains("__attribute__((musttail))"));
    let output = execute_program(&program);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"0\n4999950000\n0\ncd\n");
    // Aggregate passing must work before the more aggressive optimization used
    // by the normal builder. This previously failed at -O1 without sanitizers.
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let source = dir.path().join("tail.c");
    let binary = native::executable_path(&dir.path().join("tail"));
    fs::write(&source, c).unwrap();
    let built = Command::new("clang")
        .args(["-std=c11", "-O1"])
        .args(native::thread_flags())
        .arg(&source)
        .args(native::system_libraries())
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(built.status.success(), "{built:?}");
    let unoptimized = Command::new(binary).output().unwrap();
    assert!(unoptimized.status.success(), "{unoptimized:?}");
    assert_eq!(unoptimized.stdout, output.stdout);
}

#[test]
fn indexed_reverse_and_owned_payload_rebuilding_preserve_values() {
    prints(
        r#"
enum Vec[T](n : Nat) {Nil : Vec[T](0)
Cons(x : T, xs : Vec[T](n)) : Vec[T](n + 1)}
function reverse(move xs : Vec[T](n), move acc : Vec[T](m)) : Vec[T](n + m) = match xs {
    Nil => acc,
    Cons(x, rest) => reverse(rest, Cons(x, acc)),
}

function sum(xs : Vec[Int](n), total : Int) : Int = match xs {Nil => total,Cons(x,rest) => sum(rest,total + x),}
function head(xs : Vec[T](n + 1)) : T = match xs {Cons(x,_) => x,}
let reversed : Vec[Int](4) = reverse(Cons(1,Cons(2,Cons(3,Cons(4,Nil)))),Nil)
print(head(reversed))
print(sum(reversed,0))
class Point {
public final label : String
new(label : String) {this.label=label}
}
enum Chain {End,Link(Point,Chain)}
function rebuild(move xs : Chain) : Chain = match xs {End => End,Link(x,rest) => Link(x,rest),}
function last(xs : Chain) : String = match xs {End => "empty",Link(x,End) => x.label,Link(_,rest) => last(rest),}
let items := rebuild(Link(Point("left" + "most"),Link(Point("right" + "most"),End)))
print(last(items))
print(last(items))
enum Boxes {Empty,More(Box[String],Boxes)}
function reverseBoxes(move xs : Boxes, move acc : Boxes) : Boxes = match xs {Empty => acc,More(x,rest) => reverseBoxes(rest,More(x,acc)),}
function firstBox(xs : Boxes) : String = match xs {Empty => "empty",More(x,_) => x.text(),}
let boxes := reverseBoxes(More(box("first" + " box"),More(box("second" + " box"),Empty)),Empty)
print(firstBox(boxes))
print(firstBox(boxes))
"#,
        "4\n10\nrightmost\nrightmost\nsecond box\nsecond box\n",
    );
}

#[test]
fn owned_enum_remainders_return_by_move_and_borrowed_ones_keep_the_owner() {
    prints(
        r#"
enum Chain[T] {End,Link(T,Chain[T])}
function pop(move xs : Chain[T]) : Chain[T] = match xs {End => End,Link(_,rest) => rest,}
function aliasPop(move xs : Chain[T]) : Chain[T] = match xs {End => End,Link(_,rest) => {let result := rest
result},}
function tail(xs : Chain[T]) : Chain[T] = match xs {End => End,Link(_,rest) => rest,}
function first(xs : Chain[T], fallback : T) : T = match xs {End => fallback,Link(x,_) => x,}
let owned := pop(Link("discard" + " me",Link("keep" + " me",End)))
print(first(owned,"empty"))
print(first(owned,"empty"))
let aliased := aliasPop(Link(1,Link(2,End)))
print(first(aliased,-1))
let original := Link(3,Link(4,End))
let view := tail(original)
print(first(original,-1))
print(first(view,-1))
print(first(original,-1))
let empty : Chain[String] = pop(End)
print(first(empty,"empty"))
"#,
        "keep me\nkeep me\n2\n3\n4\n3\nempty\n",
    );
}

#[test]
fn python_worker_callbacks_copy_and_drop_opaque_handles_without_deadlock() {
    if !python_available("threading") {
        return;
    }
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("driver.py"),
        r#"import threading
def invoke(callback, value):
    results, errors = [], []
    done = threading.Event()
    def work():
        try:
            results.append(callback(value))
        except BaseException as error:
            errors.append(error)
        finally:
            done.set()
    worker = threading.Thread(target=work, daemon=True)
    worker.start()
    if not done.wait(3):
        raise RuntimeError("Kestrel callback did not finish")
    worker.join()
    if errors:
        raise errors[0]
    return results[0]
"#,
    )
    .unwrap();
    let source = format!(
        r#"import std.python.{{PythonValue}}
import python "sys" as sys
import python "driver" as driver
function identity(value : PythonValue) : PythonValue = value
function take(move value : PythonValue) : PythonValue = value
function run() : void or Error with Effects {{
    let path := sys.path?
    path.insert(0, {:?})?
    let copied : List[Int] = driver.invoke(identity, [1,2,3])?
    print(copied)
    let taken : String = driver.invoke(take, "λ")?
    print(taken)
}}
print(match run() {{RuntimeError.Python(message) => message,_ => "done",}})
"#,
        dir.path().to_str().unwrap()
    );
    prints(&source, "[1, 2, 3]\nλ\ndone\n");
}

#[test]
fn python_callback_reentry_reports_busy_and_allows_idle_or_same_thread_calls() {
    if !python_available("threading") {
        return;
    }
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("wait.h"),
        "int release_and_wait(const char *ready, const char *done);\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("wait.c"),
        r#"
#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <unistd.h>
#include <time.h>
int release_and_wait(const char *ready, const char *done) {
    FILE *file=fopen(ready,"w"); if (!file) return 0; fclose(file);
    struct timespec pause={0,1000000};
    for (int i=0;i<3000;++i) {if (access(done,F_OK)==0) return 1;nanosleep(&pause,0);}
    return 0;
}
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("driver.py"),
        r#"import os
import threading
import time
def invoke(callback,value):
    results, errors = [], []
    done = threading.Event()
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
def same_thread(callback,value):
    return callback(value)
def start(callback,value,ready,done):
    global worker, saved, failure
    saved=failure=None
    for path in (ready,done):
        try:
            os.unlink(path)
        except FileNotFoundError:
            pass
    def work():
        global saved,failure
        while not os.path.exists(ready):
            time.sleep(0.001)
        try:
            saved=callback(value)
        except BaseException as error:
            failure=error
        finally:
            open(done,"w").close()
    worker=threading.Thread(target=work,daemon=True)
    worker.start()
def result():
    worker.join()
    if failure is not None:
        raise failure
    return saved
"#,
    )
    .unwrap();
    let source = format!(
        r#"import std.python.{{PythonValue}}
import std.str.{{contains}}
import python "sys" as sys
import python "driver" as driver
import python "builtins" as py
import c {:?}
function label(value : PythonValue) : String or Error with Effects = py.str(value)
function inner(value : PythonValue) : void or Error with Effects {{let text : String = py.str(value)?}}
function spawned(value : PythonValue) : String or Error with Effects {{scope {{spawn inner(value)}}?
"unexpected"}}
function classify(value : String or Error) : String = match value {{
RuntimeError.Python(message) => if contains(message,"busy") then "busy" else message,
text => text,
_ => "other",
}}
function run() : void or Error with Effects {{
    let path := sys.path?
    path.insert(0,{:?})?
    let same : String = driver.same_thread(label,"same thread")?
    print(same)
    let direct : String or Error = driver.invoke(label,[1,2,3])
    print(classify(direct))
    let child : String or Error = driver.invoke(spawned,[1,2,3])
    print(classify(child))
    driver.start(label,123,{:?},{:?})?
    let completed := unsafe {{release_and_wait({:?},{:?})}}
    print(completed.toInt() == 1)
    let idle : String = driver.result()?
    print(idle)
}}
print(match run() {{RuntimeError.Python(message) => message,_ => "done",}})
"#,
        dir.path().join("wait.h").to_str().unwrap(),
        dir.path().to_str().unwrap(),
        dir.path().join("ready").to_str().unwrap(),
        dir.path().join("done").to_str().unwrap(),
        dir.path().join("ready").to_str().unwrap(),
        dir.path().join("done").to_str().unwrap()
    );
    prints(&source, "same thread\nbusy\nbusy\ntrue\n123\ndone\n");
}

#[test]
fn unicode_names_compile_without_relying_on_c_identifier_spelling() {
    prints(
        include_str!("../testdata/language/identifiers.kes"),
        "6\n4\n11\n",
    );
    prints(
        include_str!("../testdata/language/49-unicode-names.kes"),
        "6\n4\n11\n",
    );
}

#[test]
fn default_arguments_in_import_cycles_execute_in_their_defining_module() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = check_path(root.join("testdata/language/cycledefaults.kes"), root).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"42\n42\ncyclic default\ncyclic default\ncyclic default\n55\ntrue\n3\n"
    );
}

#[test]
fn runtime_contracts_in_import_cycles_guard_functions_and_methods() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = check_path(root.join("testdata/language/cyclecontracts.kes"), root).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"true\ntrue\ntrue\ntrue\ntrue\n");
}

#[test]
fn earlier_plain_parameters_feed_defaults_without_consuming_string_storage() {
    prints(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction head(move values : List[Point],n : Int) : String=match values {[p,..rest]=>p.label, []=>\"\",}\nfunction twice(f : Int -> String) : String=f(1)+\"|\"+f(2)\nlet values:=[Point(\"a\"+\"b\")]\nprint(twice(head(values,_)))",
        "ab|ab\n",
    );
    prints(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction take(move p : Point,n : Int) : String=p.label\nlet f:=take(_,_)\nprint(f(Point(\"first\"),1))\nprint(f(Point(\"second\"),2))\nfunction fresh(n : Int,move p : Point=Point(\"fresh\")) : String=p.label\nlet g:=fresh(_)\nprint(g(1))\nprint(g(2))",
        "first\nsecond\nfresh\nfresh\n",
    );
    prints(
        "class Point {public final label : String\nnew(label : String) {this.label=label}}\nfunction first(mut values : List[Point],fallback : Point) : Point=match values {[p,..rest]=>p, []=>fallback,}\nlet mut values:=[Point(\"kept\"+\" value\")]\nlet fallback:=Point(\"fallback\")\nprint(first(values,fallback).label)",
        "kept value\n",
    );
    prints(
        include_str!("../testdata/language/50-dependent-defaults.kes"),
        "9\nab:ab\n4\n5\n10\n16\nglobal default\nglobal default\n",
    );
    prints(
        "let height:=4\nfunction area(width : Int=height,height : Int=2) : Int=width*height\nprint(area())",
        "8\n",
    );
    prints(
        "function pair(move first : String,n : Int,move second : String=first) : String=first+\":\"+second\nfunction twice(f : Int -> String) : String=f(1)+\"|\"+f(2)\nprint(twice(pair(\"a\"+\"b\",_)))\nfunction consume(move text : String) : String=text\nfunction run() : String {let text:=\"a\"+\"b\"\nlet f:=() => consume(text)\nf()+f()}\nprint(run())",
        "ab:ab|ab:ab\nabab\n",
    );
    prints(
        "import std.c.{CInt32}\nfunction read(n : CInt32, value : Int=n.toInt()) : Int=value\nfunction run() : Int or Error = read(CInt32(3)?)\nprint(match run() {n => n, _ => 0,})",
        "3\n",
    );
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(dir.path().join("lib.kes"), "public function area(width : Int,height : Int=width) : Int=width*height\npublic function label[T : Text](value : T,text : String=value.text()) : String=text").unwrap();
    fs::write(dir.path().join("main.kes"), "import lib as l\nprint(l.area(3))\nprint(l.label(4))\nlet later:=l.area(_)\nprint(later(5))").unwrap();
    let project = check_path(dir.path().join("main.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"9\n4\n25\n");
    prints(
        include_str!("../testdata/language/dependentdefaults.kes"),
        "9\nab:ab\n4\n5\n10\n16\nglobal default\nglobal default\n",
    );
    prints(
        "function chain(a : Int, b : Int = a + 1, c : Int = b + 1) : Int = a + b + c\nprint(chain(2))\nfunction noisy(label : String,n : Int) : Int with Effects {print(label)\nn}\nfunction sum(a : Int,b : Int = noisy(\"default\",a)) : Int with Effects = a + b\nprint(sum(noisy(\"supplied\",3)))",
        "9\nsupplied\ndefault\n6\n",
    );
    for source in [
        "function f(n : Int = n) : Int = n\nf()",
        "function f(a : Int, mut b : Int = a) : Int = b\nf(1)",
        "class Counter {public final n : Int\nnew() {this.n=1}\npublic function f(values : List[Int] = [this.n]) : Int=values.length()}\nCounter().f()",
    ] {
        let project = ProjectCheck {
            modules: vec![check_source("test.kes", source)],
        };
        assert!(project.is_ok(), "{}", project.render_diagnostics());
        let errors = core::lower(&project).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.diagnostic.code == "E_DEFAULT_DEPENDENCY"),
            "{errors:?}"
        );
    }
}

#[test]
fn copied_callbacks_keep_owned_environments_through_helpers_and_containers() {
    prints(
        r#"
class Point {public final label : String
new(label : String) {this.label=label}}
class Holder {public var f : Int -> String
new(f : Int -> String) {this.f=f}}
function keep(f : Int -> String) : Int -> String=f
function unwrap(f : Int -> String) : Int -> String=Holder(f).f
function wrap(move f : Int -> String) : Int -> String=move (n : Int) => f(n)
function collect(f : Int -> String) : List[Int -> String]=[1,2].map(n => keep(f)).toList()
function make() : Int -> String {let p:=Point("heap"+" label")
wrap(unwrap(keep(move (n : Int) => p.label)))}
let f:=make()
print(f(1))
print(f(2))
function all() : List[Int -> String]=collect(make())
for (value in all()) {print(value(1))}
function label(n : Int) : String=n.text()
function factory(n : Int) : Int -> String=label
let array:=Array.generate(2,factory)
print(array[0](7))
print(array[1](8))
"#,
        "heap label\nheap label\nheap label\nheap label\n7\n8\n",
    );
}

#[test]
fn exact_returned_wrapper_aliases_move_their_storage_to_the_caller() {
    let header = "class Point {public final label : String\nnew(label : String) {this.label=label}}\nclass Panel {public final p : &Point\nnew(p : Point) {this.p=p}}\nfunction identity(h : Panel) : Panel=h\n";
    for body in [
        "let h:=Panel(p)\nidentity(h)",
        "identity(Panel(p))",
        "{let h:=Panel(p)\nidentity(h)}",
        "if true {return identity(Panel(p))}\nidentity(Panel(p))",
    ] {
        prints(
            &format!(
                "{header}function wrap(p : Point) : Panel {{{body}}}\nlet p:=Point(\"heap\"+\" label\")\nlet panel:=wrap(p)\nprint(panel.p.label)"
            ),
            "heap label\n",
        );
    }
    prints(
        &format!(
            "{header}function direct(p : Point) : Panel {{let h:=Panel(p)\nh}}\nfunction read(p : Point) : String {{let h:=Panel(p)\nidentity(h).p.label}}\nlet p:=Point(\"heap\"+\" label\")\nprint(direct(p).p.label)\nprint(read(p))"
        ),
        "heap label\nheap label\n",
    );
    let generic_header = header.replace(
        "function identity(h : Panel) : Panel=h",
        "function identity[T](h : T) : T=h",
    );
    prints(
        &format!(
            "{generic_header}function wrap(p : Point) : Panel=identity(Panel(p))\nlet p:=Point(\"generic\"+\" owner\")\nprint(wrap(p).p.label)"
        ),
        "generic owner\n",
    );
    // An if or match whose every arm returns the same parameter is exact too.
    prints(
        &format!(
            "{header}function select(h : Panel,flag : Bool) : Panel=if flag then h else h\nfunction pick(h : Panel,flag : Bool) : Panel=match flag {{true=>h,false=>h,}}\nfunction wrap(p : Point) : Panel {{let h:=Panel(p)\nselect(h,true)}}\nfunction wrapMatch(p : Point) : Panel {{let h:=Panel(p)\npick(h,true)}}\nlet p:=Point(\"kept\"+\" label\")\nprint(wrap(p).p.label)\nprint(wrapMatch(p).p.label)"
        ),
        "kept label\nkept label\n",
    );
    let source = format!(
        "{header}function read(h : Panel) : void {{}}\nfunction wrap(p : Point) : Panel with Effects {{let h:=Panel(p)\nscope {{spawn read(h)\nreturn identity(h)}}\nPanel(p)}}\nlet p:=Point(\"kept\")\nwrap(p)"
    );
    let project = ProjectCheck {
        modules: vec![check_source("native.kes", &source)],
    };
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let errors = core::lower(&project).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.diagnostic.code == "E_NATIVE_OWNERSHIP"),
        "{errors:?}"
    );
    let checked_header = header.replace("Panel=h", "Panel ensures false=h");
    let output = execute(&format!(
        "{checked_header}function wrap(p : Point) : Panel=identity(Panel(p))\nlet p:=Point(\"kept\")\nwrap(p)"
    ));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Postcondition failed: false"));
}

#[test]
fn borrowed_callbacks_keep_temporary_receivers_through_copied_results() {
    prints(
        r#"
class Point {public final label : String
new(label : String) {this.label=label}
public function read(n : Int) : String=this.label}
class Holder {public var f : Int -> String
new(f : Int -> String) {this.f=f}}
function keep(f : Int -> String) : Int -> String=f
function collect(f : Int -> String) : List[Int -> String]=[1,2].map(n => keep(f)).toList()
function generate(f : Int -> String) : Array[Int -> String]=Array.generate(2,n => keep(f))
let f:=keep(Point("temporary"+" receiver").read)
print(f(1))
print(f(2))
let all:={collect(Point("collected"+" receiver").read)}
for (value in all) {print(value(1))}
let array:=generate(Point("array"+" receiver").read)
print(array[0](1))
print(array[1](1))
let fromField:=Holder(keep(Point("stored"+" receiver").read)).f
print(fromField(1))
let higher:=move (value : Int -> String) => value
let indirect:=higher(keep(Point("indirect"+" receiver").read))
print(indirect(1))
let fromBlock:={let value:=keep(Point("block"+" receiver").read)
value}
print(fromBlock(1))
"#,
        "temporary receiver\ntemporary receiver\ncollected receiver\ncollected receiver\narray receiver\narray receiver\nstored receiver\nindirect receiver\nblock receiver\n",
    );
}

#[test]
fn dynamic_mutation_stays_explicit_and_read_only_script_views_remain_usable() {
    prints(
        r#"
trait Mutable {mut function bump() : void
function current() : Int}
trait ReadOnly {function current() : Int}
class Tally {has Mutable, ReadOnly
var n : Int
new() {this.n=0}
public mut function bump() : void {this.n=this.n+1}
public function current() : Int=this.n}
alias View=Box[dyn Mutable]
function change(mut value : View) : void=value.bump()
let value : View=box(Tally())
change(value)
change(value)
print(value.current())
let reader : Box[dyn ReadOnly]=box(Tally())
function read() : Int=reader.current()
print(read())
let cursor:=[1,2].items()
print(match cursor.next() {Some(n)=>n,None=>0})
print(match cursor.next() {Some(n)=>n,None=>0})
function run(mut f : () -> Int=() => 7) : Int=f()
let tally:=Tally()
let bump:=move () => {tally.bump()
tally.current()}
print(run(bump))
print(run(bump))
print(run(() => 8))
print(run())
function build[T](factory : () -> T) : T=factory()
let second:=build(() => {let tally:=Tally()
move () => {tally.bump()
tally.current()}})
print(run(second))
print(run(second))
"#,
        "2\n0\n1\n2\n1\n2\n8\n7\n1\n2\n",
    );
}

#[test]
fn reusable_factories_create_fresh_results_instead_of_consuming_captures() {
    prints(
        r#"
class Point {public final label : String
new(label : String) {this.label=label}}
function make() : () -> Point {let p:=Point("heap"+" label")
move () => Point(p.label)}
function get(f : () -> Point) : Point=f()
let factory:=make()
print(get(factory).label)
print(get(factory).label)
function text() : () -> String {let value:="copied"+" capture"
move () => value}
let f:=text()
print(f())
print(f())
"#,
        "heap label\nheap label\ncopied capture\ncopied capture\n",
    );
}

#[test]
fn nested_factories_and_borrow_holding_arrays_keep_their_owners() {
    prints(
        r#"
class Point {public final label : String
new(label : String) {this.label=label}}
class Panel {public final point : &Point
new(point : Point) {this.point=point}}
let outer:=() => {let p:=Point("fresh"+" owner")
move () => p.label}
let first:=outer()
let second:=outer()
print(first())
print(second())
print(first())
function panels(p : Point) : Array[Panel]=Array.generate(2,i => Panel(p))
let p:=Point("array"+" owner")
let values:=panels(p)
print(values[0].point.label)
print(values[1].point.label)
let temporary:=panels(Point("temporary"+" owner"))
print(temporary[1].point.label)
let text:="static"+" text"
function make() : () -> String=move () => text
let f:=make()
print(f())
print(f())
"#,
        "fresh owner\nfresh owner\nfresh owner\narray owner\narray owner\ntemporary owner\nstatic text\nstatic text\n",
    );
}

#[test]
fn trait_views_keep_scalar_and_object_storage_through_helpers_and_fields() {
    prints(
        r#"
class Point {has Text
public final label : String
new(label : String) {this.label=label}
public function text() : String=this.label}
class View {public final value : &dyn Text
new(value : &dyn Text) {this.value=value}}
function identity(value : &dyn Text) : &dyn Text=value
function object(value : Point) : &dyn Text=value
let p:=Point("object"+" owner")
let view : &dyn Text=p
print(p.label)
print(view.text())
print(object(p).text())
let text:="scalar"+" owner"
let scalar : &dyn Text=text
print(text)
print(scalar.text())
let retained:=identity("temporary"+" scalar")
print(retained.text())
let number:=identity(42)
print(number.text())
let holder:=View("field"+" owner")
print(holder.value.text())
let temporary : &dyn Text={Point("block"+" owner")}
print(temporary.text())
function optional(value : Point) : (&dyn Text) or Error=value
let possible:=optional(p)
match possible {value => print(value.text()),_ => print("error")}
"#,
        "object owner\nobject owner\nobject owner\nscalar owner\nscalar owner\ntemporary scalar\n42\nfield owner\nblock owner\nobject owner\n",
    );
}

#[test]
fn returned_aggregates_cannot_leave_their_borrowed_result_boxes_behind() {
    let header = r#"
class Point {public final label : String
new(label : String) {this.label=label}}
class Panel {public final point : &Point
new(point : Point) {this.point=point}}
function maybe(p : Point,flag : Bool) : Point=if flag then Point("fresh"+" owner") else p
"#;
    for body in [
        "Panel(maybe(p,true))",
        "{let current:=maybe(p,true)\nPanel(current)}",
    ] {
        let source = format!(
            "{header}\nfunction wrap(p : Point) : Panel={body}\nlet p:=Point(\"old\"+\" owner\")\nprint(wrap(p).point.label)"
        );
        let project = ProjectCheck {
            modules: vec![check_source("native.kes", &source)],
        };
        assert!(project.is_ok(), "{}", project.render_diagnostics());
        let errors = core::lower(&project).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.diagnostic.code == "E_NATIVE_OWNERSHIP"
                    && error.diagnostic.message.contains("newly owned result")),
            "{errors:?}"
        );
    }
    prints(
        &format!(
            "{header}\nfunction borrowed(p : Point) : Point=p\nfunction wrap(p : Point) : Panel=Panel(borrowed(p))\nfunction local(p : Point) : String {{let panel:=Panel(maybe(p,true))\npanel.point.label}}\nlet p:=Point(\"old\"+\" owner\")\nprint(wrap(p).point.label)\nprint(local(p))"
        ),
        "old owner\nfresh owner\n",
    );
}

#[test]
fn written_mutable_callback_types_return_move_and_store_captured_state() {
    prints(
        include_str!("../testdata/language/51-mutable-callback-types.kes"),
        "1\n2\n11\n12\n21\n",
    );
    prints(
        "function use(move f : mut (() -> Int)) : Int=f()\nprint(use(() => 7))\nlet f : mut (() -> Int)=() => 8\nprint(f())",
        "7\n8\n",
    );
    prints(
        r#"
alias Action = mut (() -> Int)
function readonly() : () -> Int=() => 7
class Holder {public var f : Action
new(move f : Action) {this.f=f}
public mut function next() : Int=this.f()}
let value : Action=readonly()
print(value())
let holder:=Holder(readonly())
print(holder.next())
function widen(move f : () -> Int) : Action=f
let original:=readonly()
let typed:=widen(original)
print(typed())
function effects() : mut (() -> void with Effects) {let mut n:=0
move () => {n=n+1
print(n)}}
let action:=effects()
action()
action()
let values:=Array.generate(2,i => {let mut n:=i*10
move () => {n=n+1
n}})
print(values.length())
"#,
        "7\n7\n7\n1\n2\n2\n",
    );
}

#[test]
fn imported_mutable_callbacks_keep_their_state_and_owned_fields() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("counters.kes"),
        r#"
public alias Counter = mut (() -> Int)
public function make(start : Int) : Counter {let mut n:=start
move () => {n=n+1
n}}
public class Holder {public var step : Counter
new(move step : Counter) {this.step=step}
public mut function next() : Int=this.step()}
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("app.kes"),
        r#"
import counters as c
function next(mut f : c.Counter) : Int=f()
let f : c.Counter=c.make(0)
print(next(f))
print(next(f))
let holder:=c.Holder(c.make(10))
print(holder.next())
print(holder.next())
"#,
    )
    .unwrap();
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"1\n2\n11\n12\n");
}

#[test]
fn collection_lengths_count_stored_entries_through_borrows_and_method_values() {
    prints(
        include_str!("../testdata/language/52-collection-counts.kes"),
        "3\n2\n2\n2\n2\n2\nkept text\n",
    );
    prints(
        r#"
let list : List[Int]=[]
let map : Map[Int,Int]=Map()
let set : Set[Int]=Set()
let sorted : SortedSet[Int]=SortedSet()
let mutmap : MutMap[Int,Int]=MutMap()
let mutset : MutSet[Int]=MutSet()
print(list.length()+map.length()+set.length()+sorted.length()+mutmap.length()+mutset.length())
mutmap.put(1,2)
mutmap.put(1,3)
mutmap.put(2,4)
mutset.add(1)
mutset.add(1)
mutset.add(2)
function count(values : MutMap[Int,Int]) : Int=values.length()
print(count(mutmap))
let length:=mutmap.length
print(length())
print(mutset.length())
let original:=Map(1=>2)
let changed:=original.put(3,4)
print(original.length())
print(changed.length())
function saved() : () -> Int {let values:=[1,2,3]
values.length}
let f:=saved()
print(f())
print(f())
function savedMap() : Int -> Bool {let values:=Map(1=>2)
values.contains}
let contains:=savedMap()
print(contains(1))
print(contains(9))
let characters:="λ😀".length
print(characters())
print(characters())
"#,
        "0\n2\n2\n2\n1\n2\n3\n3\ntrue\nfalse\n2\n2\n",
    );
}

#[test]
fn defaults_read_copied_collection_storage_when_producing_plain_values() {
    prints(
        r#"
function count[T](values : List[T], size : Int = values.length()) : Int=size
print(count([1,2,3]))
print(count(["a"+"b"]))
function nested(values : Map[String,List[Int]], found : Bool = values.contains("x")) : Bool=found
print(nested(Map("x"=>[1,2])))
function label(values : List[Int], value : String = values.length().text()) : String=value
print(label([1,2,3]))
function first(move values : List[String], value : String = match values {[head,..]=>head,_=>""}) : String=value
print(first(["fresh"+" storage"]))
print(first([]))
let captured:=count(_)
print(captured([4,5]))
print(captured([6,7,8]))
"#,
        "3\n1\ntrue\n3\nfresh storage\n\n2\n3\n",
    );
    prints(
        r#"
function list(values : List[Int], other : List[Int] = values) : List[Int]=other
print(list([1]))
class Point {new() {}}
function points(values : List[Point], other : List[Point] = values) : List[Point]=other
print(points([Point()]).length())
function array(values : Array[Int], other : Array[Int] = values) : Array[Int]=other
print(array(Array(1,0)).length())
"#,
        "[1]\n1\n1\n",
    );
}

#[test]
fn imported_collection_defaults_use_the_instantiated_storage() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(dir.path().join("counts.kes"), r#"
public function count[T](values : List[T], size : Int = values.length()) : Int=size
public function first(move values : List[String], value : String = match values {[head,..]=>head,_=>""}) : String=value
"#).unwrap();
    fs::write(
        dir.path().join("app.kes"),
        r#"
import counts as c
print(c.count([1,2,3]))
print(c.count(["heap"+" value"]))
print(c.first(["retained"+" string"]))
"#,
    )
    .unwrap();
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"3\n1\nretained string\n");
}

#[test]
fn copied_method_receivers_snapshot_the_value_when_the_method_is_saved() {
    prints(
        r#"
function lists() : Int {let mut values:=[1,2]
let count:=values.length
values=[3]
count()}
function strings() : Int {let mut value:="abc"+"d"
let count:=value.length
value="x"
count()}
function numbers() : String {let mut value:=1
let text:=value.text
value=2
text()}
function maps() : Int -> Bool {let mut values:=Map(1=>2)
let contains:=values.contains
values=Map(3=>4)
contains}
print(lists())
print(strings())
print(numbers())
let contains:=maps()
print(contains(1))
print(contains(3))
"#,
        "2\n4\n1\ntrue\nfalse\n",
    );
}

#[test]
fn saved_methods_widen_results_and_release_the_source_of_a_copied_receiver() {
    prints(
        r#"
function count() : Int or Error {let length:=[1,2].length
length()}
function identity[T](value : T,n : Int) : T=value
function label() : String or Error {let saved:=identity(_,0)
saved("generic"+" result")}
print(match count() {n=>n,_=>0})
print(match label() {s=>s,_=>"error"})
"#,
        "2\ngeneric result\n",
    );
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let path = format!("{:?}", dir.path().join("written.txt").to_str().unwrap());
    prints(
        &format!(
            r#"
class NamedWriter {{public var file : Writer
public final label : String
new(move file : Writer,label : String) {{this.file=file
this.label=label}}}}
function run(path : String) : Int or Error with Effects {{
let length:={{let w:=create(path)?
w.writeLine("written")?
NamedWriter(w,"abc").label.length}}
using r:=open(path)? {{print(match r.readLine() {{RuntimeError.EndOfFile(_)=>"empty",line=>line,_=>"error"}})}}?
length()
}}
print(match run({path}) {{n=>n,_=> -1}})
"#
        ),
        "written\n3\n",
    );
}

#[test]
fn named_arrow_inputs_mutate_borrowed_arguments_and_consume_owned_arguments() {
    prints(
        include_str!("../testdata/language/53-function-input-permissions.kes"),
        "1\n2\nowned value\n5\n",
    );
    prints(
        r#"
class Counter {public var n : Int
new() {this.n=0}
public mut function bump() : Int {this.n=this.n+1
this.n}}
alias Update = (mut value : Counter) -> Int
class Actions {public final update : Update
new(update : Update) {this.update=update}
public function run(mut c : Counter) : Int=this.update(value=c)}
function increment(mut counter : Counter) : Int=counter.bump()
let actions:=Actions(increment)
let c:=Counter()
print(actions.run(c))
print(actions.run(c))
alias Accumulate = mut ((mut value : Counter) -> Int)
function make() : Accumulate {let mut total:=0
move (mut item : Counter) => {total=total+1
item.bump()+total}}
let accumulate:=make()
print(accumulate(value=c))
print(accumulate(value=c))
alias Mixed = (Int,right : Int) -> Int
function sum(a : Int,b : Int) : Int=a+b
let mixed : Mixed=sum
print(mixed(2,right=3))
trait Source {function next() : (left : Int,right : Int) -> Int}
class Subtract {has Source
new() {}
public function next() : (a : Int,b : Int) -> Int=(x : Int,y : Int) => x-y}
let source : Box[dyn Source]=box(Subtract())
let subtract:=source.next()
print(subtract(right=2,left=8))
"#,
        "1\n2\n4\n6\n5\n6\n",
    );
}

#[test]
fn plain_defaults_read_classes_mutable_containers_and_callbacks() {
    prints(
        r#"
class Point {public final label : String
new(label : String) {this.label=label}
public function read() : String=this.label}
function text(p : Point,label : String = p.label) : String=label
function own(move p : Point,label : String = p.label) : String=label
function list(values : List[Point],size : Int = values.length()) : Int=size
function array(values : Array[Int],size : Int = values.length()) : Int=size
function mapped(values : MutMap[String,Point],label : String = match values.get("a") {Some(p)=>p.label,None=>""}) : String=label
function value(f : () -> String,label : String = f()) : String=label
function number(f : () -> Int,n : Int = f()) : Int=n
function greeting() : String="named"+" callback"
print(text(Point("borrowed"+" object")))
print(own(Point("moved"+" object")))
print(list([Point("a"),Point("b")]))
print(array(Array(3,0)))
let values : MutMap[String,Point]=MutMap()
values.put("a",Point("map"+" value"))
print(mapped(values))
print(value(greeting))
print(value(() => "lambda"+" callback"))
let p:=Point("method"+" callback")
print(value(p.read))
print(number([1,2].length))
"#,
        "borrowed object\nmoved object\n2\n3\nmap value\nnamed callback\nlambda callback\nmethod callback\n2\n",
    );
}

#[test]
fn imported_defaults_keep_owned_python_callbacks_and_class_reads_safe() {
    if !python_available("builtins") {
        eprintln!("skipped: no embeddable CPython");
        return;
    }
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("defaults.kes"),
        r#"
import python "builtins" as py
public class Point {public final label : String
new(label : String) {this.label=label}
public function read(label : String = this.label) : String=label}
public function text(p : Point,label : String = p.label) : String=label
public function value(f : () -> String,label : String = f()) : String=label
public function install(n : Int = {let p:=Point("owned"+" callback")
let ignored:=py.sorted([1,2],key=move (item : Int) => p.label)
7}) : Int with Effects=n
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("app.kes"),
        r#"
import defaults as d
print(d.text(d.Point("imported"+" object")))
print(d.value(() => "imported"+" lambda"))
print(d.install())
let point:=d.Point("imported"+" receiver")
print(point.read())
let read:=point.read
print(read())
"#,
    )
    .unwrap();
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"imported object\nimported lambda\n7\nimported receiver\nimported receiver\n"
    );
}

#[test]
fn imported_named_arrow_fields_use_the_same_native_representation() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("actions.kes"),
        r#"
public class Counter {public var n : Int
new() {this.n=0}}
public alias Update = (mut value : Counter) -> Int
public function increment(mut target : Counter) : Int {target.n=target.n+1
target.n}
public class Actions {public final update : Update
new(update : Update) {this.update=update}
public function run(mut c : Counter) : Int=this.update(value=c)}
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("app.kes"),
        r#"
import actions as a
let actions:=a.Actions(a.increment)
let c:=a.Counter()
print(actions.run(c))
print(actions.run(c))
"#,
    )
    .unwrap();
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"1\n2\n");
}

#[test]
fn plain_method_defaults_read_the_live_receiver() {
    prints(
        include_str!("../testdata/language/54-receiver-and-callback-defaults.kes"),
        "9\n6\n9\n2\n3\n",
    );
    prints(
        r#"
class Rectangle {public var width : Int
new(width : Int) {this.width=width}
public function area(height : Int = this.width) : Int=this.width*height
public function offset(n : Int,extra : Int = this.width) : Int=n+extra}
class Square extends Rectangle {new(width : Int) {this.width=width}}
class Label[T : Text] {public final value : T
new(move value : T) {this.value=value}
public function read(label : String = {let copy : T=this.value
copy.text()}) : String=label}
function apply(f : Int -> Int,n : Int) : Int=f(n)
let rectangle:=Rectangle(3)
print(rectangle.area())
{
let area:=rectangle.area
print(area())
let offset:=rectangle.offset(_)
print(offset(4))
print(apply(offset,5))
}

rectangle.width=6
print(rectangle.offset(4))
print(apply(rectangle.offset(_),5))
print(rectangle.area())
print(Square(4).area())
print(Label("generic"+" label").read())
print(Label(7).read())
"#,
        "9\n9\n7\n8\n10\n11\n36\n16\ngeneric label\n7\n",
    );
}

#[test]
fn abstract_trait_defaults_dispatch_through_the_live_receiver() {
    prints(
        r#"
trait Area {function width() : Int
function area(height : Int = this.width()) : Int}
class Rectangle {has Area
public final size : Int
new(size : Int) {this.size=size}
public function width() : Int=this.size
public function area(height : Int) : Int=this.size*height}
function area(rectangle : &dyn Area) : Int=rectangle.area()
let rectangle : Box[dyn Area]=box(Rectangle(3))
print(rectangle.area())
print(area(rectangle))
let saved:=rectangle.area
print(saved())
print(saved(height=4))
trait Label[T : Text] {function value() : T
function label(text : String = {let copy : T=this.value()
copy.text()}) : String
function from(value : T,text : String = {let copy : T=value
copy.text()}) : String}
class Named {has Label[String]
new() {}
public function value() : String="generic"+" trait"
public function label(text : String) : String=text
public function from(value : String,text : String) : String=text}
let named : Box[dyn Label[String]]=box(Named())
print(named.label())
print(named.from("earlier"+" trait input"))
trait Source {type Item
function value() : Item
function echo(value : Item = this.value()) : Item}
class TextSource {has Source(Item = String)
new() {}
public function value() : String="associated"+" value"
public function echo(value : String) : String=value}
let source : Box[dyn Source(Item = String)]=box(TextSource())
print(source.echo())
"#,
        "9\n9\n9\n12\ngeneric trait\nearlier trait input\nassociated value\n",
    );
}

#[test]
fn receiver_defaults_observe_shared_state_once_per_call() {
    prints(
        r#"
class Gauge {public var current : Atomic[Int]
new(n : Int) {this.current=Atomic(n)}
public function read(n : Int = {print("default")
this.current.load()}) : Int with Effects=n}
let gauge:=Gauge(3)
let read:=gauge.read
print(read())
gauge.current.store(7)
print(read())
print(read(10))
function apply(f : (Int -> Int with Effects)) : Int with Effects=f(12)
print(apply(read))
"#,
        "default\n3\ndefault\n7\n10\n12\n",
    );
}

#[test]
fn returned_exact_wrappers_keep_resources_until_the_caller_releases_them() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let path = format!("{:?}", dir.path().join("written.txt").to_str().unwrap());
    prints(
        &format!(
            r#"
class Point {{public final label : String
new(label : String) {{this.label=label}}}}
class NamedWriter {{public var file : Writer
public final point : &Point
new(move file : Writer,point : Point) {{this.file=file
this.point=point}}}}
function identity(value : NamedWriter) : NamedWriter=value
function wrap(point : Point,move file : Writer) : NamedWriter {{let local:=NamedWriter(file,point)
identity(local)}}
function run(path : String) : void or Error with Effects {{
let point:=Point("kept"+" owner")
{{let file:=create(path)?
file.writeLine("written")?
let writer:=wrap(point,file)
print(writer.point.label)}}
using reader:=open(path)? {{print(reader.readLine()?)}}
}}
run({path})?
"#
        ),
        "kept owner\nwritten\n",
    );
}

#[test]
fn fully_initialized_constructors_can_use_methods_and_callbacks() {
    prints(
        include_str!("../testdata/language/55-construction-order.kes"),
        "7\n",
    );
}

#[test]
fn static_vars_hold_synchronized_copies_of_copyable_values() {
    prints(
        include_str!("../testdata/language/58-static-variables.kes"),
        "3\n",
    );
    prints(
        r#"
class C {
    public static var count : Int = 1
    public static var text : String = "a"
    public static var items : List[Int] = [1]
    new() {}
    public static function bump() : void with Effects { C.count = C.count + 1 }
}
C.count = C.count + 1
C.text = C.text + "b"
C.items = [2, 3]
print(C.count)
print(C.text)
print(C.items)
let saved := C.text
C.text = "z"
print(saved)
print(C.text)
scope { spawn C.bump() }
print(C.count)
"#,
        "2\nab\n[2, 3]\nab\nz\n3\n",
    );
}

#[test]
fn defaults_may_alias_a_free_call_or_constructor_of_earlier_values() {
    prints(
        r#"
class Point { public final label : String
new(label : String) {this.label=label} }
function find(p : Point, n : Int) : Point = p
function fresh(p : Point) : Point = Point(p.label + "!")
function read(p : Point, q : Point = find(p,1)) : String = q.label
function built(p : Point, q : Point = Point(p.label)) : String = q.label
function made(p : Point, q : Point = fresh(p)) : String = q.label
function tail(p : Point) : String = made(p)
function identity(p : Point) : Point = p
function temporary(q : Point = identity(Point("temp"))) : String = q.label
function tailTemporary() : String = temporary()
let point := Point("call")
print(read(point))
print(built(point))
print(made(point))
print(tail(point))
print(tailTemporary())
"#,
        "call\ncall\ncall!\ncall!\ntemp\n",
    );
}

#[test]
fn defaults_may_alias_a_method_call_on_an_earlier_parameter_or_this() {
    prints(
        r#"
class Point { public final label : String
new(label : String) {this.label=label}
public function same() : Point = this }
class Factory { public final label : String
new(label : String) {this.label=label}
public function make() : Point = Point(this.label + "!") }
class Holder { public final point : Point
new(move point : Point) {this.point=point}
public function choose() : Point = this.point
public function label(q : Point = this.choose()) : String = q.label }
function read(p : Point, q : Point = p.same()) : String = q.label
function make(f : Factory, q : Point = f.make()) : String = q.label
function tail(f : Factory) : String = make(f)
let point := Point("found")
print(read(point))
let factory := Factory("fresh")
print(make(factory))
print(tail(factory))
let holder := Holder(point)
print(holder.label())
"#,
        "found\nfresh!\nfresh!\nfound\n",
    );
}

#[test]
fn defaults_may_alias_a_field_path_of_an_earlier_parameter_or_this() {
    prints(
        r#"
class Point { public final label : String
new(label : String) {this.label=label} }
class Panel { public final point : &Point
new(point : Point) {this.point=point}
public function label(q : Point = this.point) : String = q.label }
function read(p : Panel, q : Point = p.point) : String = q.label
let point := Point("field")
let panel := Panel(point)
print(read(panel))
print(panel.label())
"#,
        "field\nfield\n",
    );
}

#[test]
fn defaults_may_alias_an_earlier_borrowed_parameter_or_this() {
    prints(
        r#"
class Point { public final label : String
new(label : String) {this.label=label}
public function same(q : Point = this) : String = q.label + this.label }
function show(p : Point, q : Point = p) : String = p.label + q.label
class Pair {
    public final a : &Point
    public final b : &Point
    new(a : Point, b : Point = a) {this.a=a
    this.b=b}
    public function text() : String = this.a.label + this.b.label
}
let p := Point("c")
print(show(p))
print(p.same())
print(Pair(p).text())
"#,
        "cc\ncc\ncc\n",
    );
}

#[test]
fn static_final_fields_initialize_lazily_and_copy_on_read() {
    prints(
        include_str!("../testdata/language/57-static-fields.kes"),
        "kestrel registry\n3\n",
    );
    prints(
        r#"
class Limits {
    public static final base : Int = 10
    public static final doubled : Int = Limits.base * 2
    public static final text : String = "hello" + " world"
    public static final items : List[Int] = [1, 2, 3]
    public static final lookup : Map[String, Int] = Map("a" => 1)
    new() {}
}
print(Limits.base)
print(Limits.doubled)
print(Limits.text)
print(Limits.items)
print(match Limits.lookup.get("a") {Some(v) => v, None => 0})
print(Limits.text.length())
print(Limits.text)
print(Limits.items.length())
"#,
        "10\n20\nhello world\n[1, 2, 3]\n1\n11\nhello world\n3\n",
    );
    prints(
        r#"
class Shared { public static final text : String = "shared" + " value"
new() {} }
function say() : void with Effects { print(Shared.text) }
scope {
    spawn say()
    spawn say()
    spawn say()
}
"#,
        "shared value\nshared value\nshared value\n",
    );
    let project = ProjectCheck {
        modules: vec![check_source(
            "native.kes",
            "class C { public static final a : Int = C.b\npublic static final b : Int = C.a\nnew() {} }\nprint(C.a)",
        )],
    };
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let errors = core::lower(&project).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.diagnostic.code == "E_NATIVE_STATIC_CYCLE"),
        "{errors:?}"
    );
}

#[test]
fn bigint_literals_arithmetic_comparisons_and_text_execute() {
    prints(
        include_str!("../testdata/language/56-arbitrary-precision-integers.kes"),
        "1000000000000000000001\n333333333333333333333\n1000000000000000000001\noutside Int\n-12345678901234567890\n-3\n2\n1000000000000000000001\n",
    );
    prints(
        r#"
let a : BigInt=1_000_000_000_000_000_000_000
let b : BigInt=3
let zero : BigInt=0
let hex : BigInt=0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF
print(a)
print(hex)
print(a+b)
print(a-b)
print(a*b)
print(a/b)
print(a%b)
print(a > b)
print(a == a)
print(a != b)
print(a-a)
print(-a)
print(+a)
print(zero*hex)
print((-a)/b)
print((-a)%b)
let exponent : BigInt=100
print(b ** exponent)
print(f"{a} {a:?}")
function same(value : BigInt) : BigInt=value
print(same(a).text())
"#,
        "1000000000000000000000\n340282366920938463463374607431768211455\n1000000000000000000003\n999999999999999999997\n3000000000000000000000\n333333333333333333333\n1\ntrue\ntrue\ntrue\n0\n-1000000000000000000000\n1000000000000000000000\n0\n-333333333333333333333\n-1\n515377520732011331036461129765621272702107522001\n1000000000000000000000 1000000000000000000000\n1000000000000000000000\n",
    );
}

#[test]
fn bigint_contextual_literals_conversions_parsing_and_math_are_explicit() {
    prints(
        r#"
import std.math.{abs,min,max}
function value() : BigInt=1_000_000_000_000_000_000_000+1
let n:=value()
print(n+2)
print((1+2)+n)
print(1==n)
print(12.toBigInt())
print((-9223372036854775808).toBigInt())
print(match parseBigInt("+000123456789012345678901234567890") {v=>v.text(),_=>"error"})
print(match parseBigInt("-0") {v=>v.text(),_=>"error"})
for (text in ["","+","-"," 12","12 ","0xff","1_000","1.0","1e3","1\u00002"]) {
print(match parseBigInt(text) {RuntimeError.Parse(_)=>"invalid",_=>"wrong"})}
let smallest : BigInt=-9223372036854775808
let largest : BigInt=9223372036854775807
print(match smallest.checkedToInt() {i=>i.text(),_=>"error"})
print(match largest.checkedToInt() {i=>i.text(),_=>"error"})
print(match (largest+1).checkedToInt() {RuntimeError.Arithmetic(_)=>"range",_=>"wrong"})
print(match (smallest-1).checkedToInt() {RuntimeError.Arithmetic(_)=>"range",_=>"wrong"})
let lower : BigInt=9007199254740993
let upper : BigInt=9007199254740995
print(match lower.checkedToDouble() {d=>d==9007199254740992.0,_=>false})
print(match upper.checkedToDouble() {d=>d==9007199254740996.0,_=>false})
let enormous : BigInt=10 ** 400
print(match enormous.checkedToDouble() {RuntimeError.Arithmetic(_)=>"double range",_=>"wrong"})
print(match n.checkedDivide(0) {RuntimeError.Arithmetic(_)=>"zero",_=>"wrong"})
print(match n.checkedDivide(2) {v=>v.text(),_=>"error"})
print(abs(-n))
print(min(n,2))
print(max(n,2))
"#,
        "1000000000000000000003\n1000000000000000000004\nfalse\n12\n-9223372036854775808\n123456789012345678901234567890\n0\ninvalid\ninvalid\ninvalid\ninvalid\ninvalid\ninvalid\ninvalid\ninvalid\ninvalid\ninvalid\n-9223372036854775808\n9223372036854775807\nrange\nrange\ntrue\ntrue\ndouble range\nzero\n500000000000000000000\n1000000000000000000001\n2\n1000000000000000000001\n",
    );
}

#[test]
fn bigint_collections_owned_fields_boxes_closures_and_generic_operators_work() {
    prints(
        r#"
class Number {public final value : BigInt
new(move value : BigInt) {this.value=value}}
function identity[T](value : T) : T=value
function square[T : Multipliable[T](Output = T)](value : T) : T=value*value
function label(value : &dyn Text) : String=value.text()
let n : BigInt=1000000000000000000000
print(square(n))
print(identity(n))
let number:=Number(n+1)
print(number.value)
let boxed : Box[dyn Text]=box(n+2)
print(label(boxed))
let owned : Box[BigInt]=box(n+3)
print(owned.text())
let factory:=move () => number.value+4
print(factory())
print(factory())
let values : List[BigInt]=[1,n+5,-2]
print(values)
print(values.contains(n+5))
let array:=Array.generate(3,i => n+i.toBigInt())
for (let i : Int=0;i<array.length();i=i+1) {print(array[i])}
let map : Map[BigInt,String]=Map(1=>"one",n+6=>"large",0=>"zero")
print(match map.get(n+6) {Some(v)=>v,None=>"missing"})
let set : Set[BigInt]=Set(1,1,n+7)
print(set.length())
let sorted : SortedSet[BigInt]=SortedSet(n+8,2,-3,0,2)
for (item in sorted) {print(item)}
"#,
        "1000000000000000000000000000000000000000000\n1000000000000000000000\n1000000000000000000001\n1000000000000000000002\n1000000000000000000003\n1000000000000000000005\n1000000000000000000005\n[1, 1000000000000000000005, -2]\ntrue\n1000000000000000000000\n1000000000000000000001\n1000000000000000000002\nlarge\n2\n-3\n0\n2\n1000000000000000000008\n",
    );
}

#[test]
fn bigint_invalid_plain_division_and_powers_stop_with_locations() {
    let numbers = execute(include_str!("../testdata/language/08-numbers.kes"));
    assert!(!numbers.status.success());
    assert!(String::from_utf8_lossy(&numbers.stderr).contains("Int addition overflow"));
    for (expression, message) in [
        ("n/0", "BigInt division by zero"),
        ("n%0", "BigInt division by zero"),
        ("n ** (-1)", "BigInt power with a negative exponent"),
    ] {
        let output = execute(&format!(
            "let n : BigInt=1000000000000000000000\nprint({expression})"
        ));
        assert!(!output.status.success());
        let errors = String::from_utf8_lossy(&output.stderr);
        assert!(
            errors.contains(message) && errors.contains("native.kes:2:"),
            "{errors}"
        );
    }
    prints(
        "let zero : BigInt=0\nlet one : BigInt=1\nprint(zero ** zero)\nprint(zero ** one)\nprint((-one) ** 3)",
        "1\n0\n-1\n",
    );
}

#[test]
fn bigint_literal_patterns_test_union_tags_before_reading_payloads() {
    prints(
        r#"
let number : BigInt=1000000000000000000000
print(match number {1000000000000000000000=>"large",_=>"wrong"})
print(match -number {-1000000000000000000000=>"negative",_=>"wrong"})
let hex : BigInt=0xffffffffffffffffffffffff
print(match hex {0xffffffffffffffffffffffff=>"hex",_=>"wrong"})
function value(failed : Bool) : BigInt or Error=if failed then RuntimeError.Parse("absent") else 1000000000000000000000
for (failed in [true,false]) {print(match value(failed) {1000000000000000000000=>"value",RuntimeError.Parse(_)=>"error",_=>"wrong"})}
"#,
        "large\nnegative\nhex\nerror\nvalue\n",
    );
}

#[test]
fn bigint_division_correction_and_large_products_match_fixed_vectors() {
    use std::fmt::Write;
    let mut source = String::new();
    let mut expected = String::new();
    for row in include_str!("data/bigint-arithmetic.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let fields = row.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 5);
        writeln!(
            source,
            "{{let a : BigInt={}\nlet b : BigInt={}\nprint(a/b)\nprint(a%b)\nprint(a*b)}}",
            fields[0], fields[1]
        )
        .unwrap();
        writeln!(expected, "{}\n{}\n{}", fields[2], fields[3], fields[4]).unwrap();
    }
    prints(&source, &expected);
}

#[test]
fn bigint_values_move_through_channels_and_mutex_guards() {
    prints(
        r#"
function produce(channel : Channel[BigInt]) : void or Error with Effects {
for (let i : Int=0;i<20;i=i+1) {channel.send(1000000000000000000000+i.toBigInt())?}
channel.close()}
function run() : BigInt or Error with Effects {
let channel : Channel[BigInt]=Channel()
let total:=Mutex(0.toBigInt())
let _ := scope {spawn produce(channel)
let mut done:=false
while !done {match channel.receive() {
Some(value)=>using guard:=total.lock()? {guard.value=guard.value+value},
None=>{done=true},
}}}
using guard:=total.lock()? {guard.value+0}}
print(match run() {value=>value.text(),_=>"error"})
"#,
        "20000000000000000000190\n",
    );
}

#[test]
fn bigint_constants_math_callbacks_and_defaults_keep_imported_types() {
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    fs::write(
        dir.path().join("numbers.kes"),
        r#"
import std.math.{max}
public const BASE : BigInt=1000000000000000000000
public function next(value : BigInt=BASE) : BigInt=value+1
public function positive(value : BigInt) : BigInt=max(0,value)
public class Number {public final value : BigInt
new(move value : BigInt) {this.value=value}
public function text() : String=this.value.text()}
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("app.kes"),
        r#"
import numbers as n
import std.math.{abs}
let value:=n.next()
print(value)
print(n.positive(-value))
let magnitude : BigInt -> BigInt=abs
print(magnitude(-value))
let number:=n.Number(n.BASE+2)
print(number.text())
"#,
    )
    .unwrap();
    let project = check_path(dir.path().join("app.kes"), dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"1000000000000000000001\n0\n1000000000000000000001\n1000000000000000000002\n"
    );
}

#[test]
fn bigint_mutable_captures_and_error_payloads_release_their_numbers() {
    prints(
        r#"
function counter() : mut (() -> BigInt) {
let mut total : BigInt=1000000000000000000000
move () => {total=total+1
total+0}}
let count:=counter()
print(count())
print(count())
print(count())
enum NumberError extends Error {Large(BigInt)}
function fail() : Int or Error=NumberError.Large(1000000000000000000004)
print(match fail() {NumberError.Large(n)=>n.text(),_=>"wrong"})
let error : Error=NumberError.Large(1000000000000000000005)
print(match error {NumberError.Large(n)=>n.text(),_=>"wrong"})
"#,
        "1000000000000000000001\n1000000000000000000002\n1000000000000000000003\n1000000000000000000004\n1000000000000000000005\n",
    );
}

#[test]
fn print_accepts_its_named_value_argument() {
    prints("print(value = 1)\nprint(value = \"ok\")", "1\nok\n");
}

#[test]
fn increment_and_decrement_update_local_field_and_array_places() {
    prints(
        r#"
let mut k := 0
k++
k++
print(k)
k--
print(k)
for (let mut i : Int = 0; i < 3; i++) { print(i) }
class Holder {
    public var value : Int
    new() { this.value = 0 }
}
let holder := Holder()
holder.value++
print(holder.value)
let values := Array(2, 0)
values[1]++
print(values[1])
let mut big : BigInt = 1
big++
print(big)
let mut natural : Nat = 1
natural++
print(natural)
"#,
        "2\n1\n0\n1\n2\n1\n1\n2\n2\n",
    );
}

#[test]
fn arrays_implement_text_when_their_elements_do() {
    prints(
        r#"
let values := Array.generate(3, n => n + 1)
print(values)
print(values.text())
print(Array(0, 0))
"#,
        "[1, 2, 3]\n[1, 2, 3]\n[]\n",
    );
}

#[test]
fn range_is_lazy_and_accepts_bounds_and_steps() {
    prints(
        r#"
print(range(5).toList())
print(range(2, 6).toList())
print(range(6, 2, -2).toList())
print(range(10).filter(x => (x % 2) == 0).map(x => x * 10).toList())
let mut seen := -1
for (i in range(1_000_000)) {
    seen = i
    break
}
print(seen)
"#,
        "[0, 1, 2, 3, 4]\n[2, 3, 4, 5]\n[6, 4]\n[0, 20, 40, 60, 80]\n0\n",
    );
}

#[test]
fn to_array_copies_a_list_or_lazy_traversal_once_into_an_array() {
    prints(
        r#"
let list := [1, 2, 3]
let array := list.toArray()
array[0] = 10
print(array)
print(array.length())
print(range(3).map(n => n * 2).toArray())
let words := ["a", "bb"].toArray()
print(words)
"#,
        "[10, 2, 3]\n3\n[0, 2, 4]\n[a, bb]\n",
    );
}

#[test]
fn array_append_grows_in_place_for_values_and_strings() {
    prints(
        r#"
let mut numbers : Array[Int] = Array(0, 0)
for (i in range(1000)) {
    numbers.append(i)
}
print(numbers.length())
print(numbers[999])
let mut words : Array[String] = Array(0, "")
words.append("hi")
words.append("there")
print(words)
"#,
        "1000\n999\n[hi, there]\n",
    );
}

#[test]
fn array_append_moves_class_values_through_growth() {
    prints(
        r#"
class Point {
    public final x : Int
    new(x : Int) { this.x = x }
}
let mut points : Array[Point] = Array.generate(0, i => Point(i))
for (i in range(100)) {
    points.append(Point(i))
}
print(points.length())
print(points[99].x)
"#,
        "100\n99\n",
    );
}

#[test]
fn join_index_and_prepend_work_on_lists_and_arrays() {
    prints(
        r#"
let mut xs : Array[Int] = Array(0, 0)
xs.append(2)
xs.prepend(1)
print(xs)
print(xs.join(" | "))
print([1, 2, 3].join(", "))
match [10, 20, 30].index(20) {
    Some(i) => print(i),
    None => print(-1),
}
match xs.index(7) {
    Some(i) => print(i),
    None => print(-1),
}
for (x in xs) {
    print(x)
}
"#,
        "[1, 2]\n1 | 2\n1, 2, 3\n1\n-1\n1\n2\n",
    );
}

#[test]
fn prepend_moves_class_values_to_the_front() {
    prints(
        r#"
class Point {
    public final x : Int
    new(x : Int) { this.x = x }
}
let mut points : Array[Point] = Array.generate(0, i => Point(i))
for (i in range(50)) {
    points.prepend(Point(i))
}
print(points.length())
print(points[0].x)
print(points[49].x)
"#,
        "50\n49\n0\n",
    );
}

#[test]
fn array_swap_and_pop_move_values_out_without_double_drops() {
    prints(
        r#"
class Point {
    public final x : Int
    new(x : Int) { this.x = x }
}
let mut xs : Array[Int] = Array(0, 0)
xs.append(1)
xs.append(2)
xs.append(3)
xs.swap(0, 2)
print(xs)
match xs.pop() {
    Some(v) => print(v),
    None => print(-1),
}
match xs.pop() {
    Some(v) => print(v),
    None => print(-1),
}
match xs.pop() {
    Some(v) => print(v),
    None => print(-1),
}
match xs.pop() {
    Some(v) => print(v),
    None => print(-1),
}
let mut ps : Array[Point] = Array.generate(0, i => Point(i))
for (i in range(20)) {
    ps.append(Point(i))
}
for (i in range(15)) {
    match ps.pop() {
        Some(p) => print(p.x),
        None => print(-1),
    }
}
print(ps.length())
"#,
        "[3, 2, 1]\n1\n2\n3\n-1\n19\n18\n17\n16\n15\n14\n13\n12\n11\n10\n9\n8\n7\n6\n5\n5\n",
    );
}

/// Checks and runs a program read from a file, so the implicit prelude is loaded (Q112).
fn prints_from_file(source: &str, expected: &str) {
    let source = source.to_owned();
    let expected = expected.to_owned();
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let dir = native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
            let file = dir.path().join("program.kes");
            fs::write(&file, source).unwrap();
            let project = check_path(&file, dir.path()).unwrap();
            assert!(project.is_ok(), "{}", project.render_diagnostics());
            let program = core::lower(&project).unwrap_or_else(|errors| panic!("{errors:#?}"));
            let output = execute_program(&program);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn standard_structures_need_no_import_and_keep_their_order() {
    prints_from_file(
        r#"
let mut s := Stack[Int]()
s.push(1)
s.push(2)
match s.peek() {
    Some(v) => print(v),
    None => print(-1),
}
match s.pop() {
    Some(v) => print(v),
    None => print(-1),
}
let mut q := Queue[String]()
q.enqueue("a")
q.enqueue("b")
match q.first() {
    Some(v) => print(v),
    None => print(-1),
}
match q.dequeue() {
    Some(v) => print(v),
    None => print(-1),
}
match q.first() {
    Some(v) => print(v),
    None => print(-1),
}
let mut idle := Queue[Int]()
match idle.first() {
    Some(v) => print(v),
    None => print(-1),
}
match idle.dequeue() {
    Some(v) => print(v),
    None => print(-1),
}
let mut lo := MinHeap[Int]()
let mut hi := MaxHeap[Int]()
for (v in [5, 1, 9, 3, 7]) {
    lo.push(v)
    hi.push(v)
}
match lo.peek() {
    Some(v) => print(v),
    None => print(-1),
}
match hi.pop() {
    Some(v) => print(v),
    None => print(-1),
}
print(lo.size())
"#,
        "2\n2\na\na\nb\n-1\n-1\n1\n9\n5\n",
    );
}

#[test]
fn standard_structures_hold_classes_and_return_borrowed_peeks() {
    prints_from_file(
        r#"
class Point {
    has Comparable
    public final x : Int
    new(x : Int) { this.x = x }
    public function compare(other : Point) : Int = this.x - other.x
}
let mut stack := Stack[Point]()
stack.push(Point(3))
stack.push(Point(4))
match stack.peek() {
    Some(p) => print(p.x),
    None => print(-1),
}
let mut heap := MinHeap[Point]()
for (i in range(20)) {
    heap.push(Point(20 - i))
}
match heap.pop() {
    Some(p) => print(p.x),
    None => print(-1),
}
match heap.peek() {
    Some(p) => print(p.x),
    None => print(-1),
}
"#,
        "4\n1\n2\n",
    );
}

#[test]
fn min_and_max_return_views_of_comparable_classes() {
    prints_from_file(
        r#"
class Shape {
    has Comparable
    public final side : Int
    new(side : Int) { this.side = side }
    public function compare(other : Shape) : Int = this.side - other.side
}
let shapes := [Shape(3), Shape(8), Shape(5)]
match shapes.max() {
    Some(s) => print(s.side),
    None => print(-1),
}
match shapes.min() {
    Some(s) => print(s.side),
    None => print(-1),
}
match shapes.first() {
    Some(s) => print(s.side),
    None => print(-1),
}
match shapes.last() {
    Some(s) => print(s.side),
    None => print(-1),
}
let empty : Array[Shape] = Array.empty()
match empty.max() {
    Some(s) => print(s.side),
    None => print(-1),
}
let pulled := [Shape(2), Shape(9), Shape(4)].toList()
match pulled.max() {
    Some(s) => print(s.side),
    None => print(-1),
}
"#,
        "8\n3\n3\n5\n-1\n9\n",
    );
}

#[test]
fn math_functions_need_no_import() {
    prints_from_file(
        r#"
print(max(5, 3))
print(min(5, 3))
print(abs(-4))
print(floor(2.7))
print(sqrt(16.0))
"#,
        "5\n3\n4\n2.0\n4.0\n",
    );
}

#[test]
fn a_program_may_declare_its_own_max() {
    prints_from_file(
        r#"
function max(a : Int, b : Int) : Int = a
print(max(5, 3))
print(min(5, 3))
"#,
        "5\n3\n",
    );
}

#[test]
fn string_and_number_methods_execute() {
    prints(
        r#"
print("  hi  ".trim())
print("Hello".toUpper())
print("Hello".toLower())
print("hello".startsWith("he"))
print("hello".endsWith("lo"))
print("hello".endsWith("he"))
print("".isEmpty())
print("x".isEmpty())
print("a,b,c".split(","))
print("hello".contains("ell"))
print((-7).abs())
print((3).min(9))
print((3).max(9))
print((4).isEven())
print((-3).isOdd())
print((2.5).floor())
print((2.5).ceil())
print((2.5).round())
print((-9.0).abs())
print((2.0).sqrt())
print((2.0).pow(3.0))
print((1.0).isFinite())
"#,
        "hi\nHELLO\nhello\ntrue\ntrue\nfalse\ntrue\nfalse\n[a, b, c]\ntrue\n7\n3\n9\ntrue\ntrue\n2.0\n3.0\n3.0\n9.0\n1.4142135623730951\n8.0\ntrue\n",
    );
}

#[test]
fn iterable_and_option_methods_execute_on_every_sequence_kind() {
    prints(
        r#"
let xs := [4, 9, 2, 7]
match xs.first() {
    Some(v) => print(v),
    None => print(-1),
}
match xs.last() {
    Some(v) => print(v),
    None => print(-1),
}
match xs.min() {
    Some(v) => print(v),
    None => print(-1),
}
match xs.max() {
    Some(v) => print(v),
    None => print(-1),
}
print(xs.any(n => n > 8))
print(xs.all(n => n > 1))
print(xs.all(n => n > 3))
print(xs.isEmpty())
let empty : List[Int] = []
match empty.first() {
    Some(v) => print(v),
    None => print(-1),
}
match empty.min() {
    Some(v) => print(v),
    None => print(-1),
}
print(empty.isEmpty())
print(empty.any(n => n > 0))
print(empty.all(n => n > 0))
let names := ["bob", "al", "cy"]
match names.min() {
    Some(v) => print(v),
    None => print(-1),
}
match names.max() {
    Some(v) => print(v),
    None => print(-1),
}
let arr : Array[Int] = Array.generate(3, i => i * 5)
match arr.last() {
    Some(v) => print(v),
    None => print(-1),
}
print(arr.isEmpty())
print(range(0).isEmpty())
print(range(4).isEmpty())
match range(4).last() {
    Some(v) => print(v),
    None => print(-1),
}
match [1].first() {
    Some(v) => print(v),
    None => print(-1),
}
let some := Some(3)
let none : Option[Int] = None
print(some.isSome())
print(some.isNone())
print(none.isSome())
print(none.isNone())
"#,
        "4\n7\n2\n9\ntrue\ntrue\nfalse\nfalse\n-1\n-1\ntrue\nfalse\ntrue\nal\ncy\n10\nfalse\ntrue\nfalse\n3\n1\ntrue\nfalse\nfalse\ntrue\n",
    );
}

#[test]
fn enum_tree_insert_and_search_run_natively() {
    prints_from_file(
        r#"
enum Tree[T] {
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
function contains(tree : Tree[Int], value : Int) : Bool = match tree {
    Leaf => false,
    Node(current, left, right) => if value == current {
        true
    } else if value < current {
        contains(left, value)
    } else {
        contains(right, value)
    },
}
let mut tree := Leaf
for (v in [5, 2, 8, 1, 9, 3]) {
    tree = insert(tree, v)
}
print(contains(tree, 3))
print(contains(tree, 4))
"#,
        "true\nfalse\n",
    );
}

#[test]
fn max_works_inside_class_methods() {
    prints_from_file(
        r#"
class Node {
    public final value : Int
    new(value : Int) { this.value = value }
    public function height() : Int = 1 + max(this.value, 2)
}
print(Node(1).height())
print(Node(7).height())
"#,
        "3\n8\n",
    );
}

#[test]
fn question_mark_returns_none_early_and_if_let_picks_a_value() {
    prints_from_file(
        r#"
function half(n : Int) : Option[Int] = if (n % 2) == 0 { Some(n / 2) } else { None }
function quarter(n : Int) : Option[Int] {
    let h := half(n)?
    let q := half(h)?
    return Some(q)
}
function describe(n : Int) : Int {
    let v := if let Some(q) := quarter(n) { q } else { -1 }
    return v
}
print(describe(12))
print(describe(6))
print(describe(7))
"#,
        "3\n-1\n-1\n",
    );
}

#[test]
fn question_mark_reads_option_fields_of_classes() {
    prints_from_file(
        r#"
class Node {
    public var value : Int
    public var left : Option[Node]
    public var right : Option[Node]
    new(move value : Int, move left : Option[Node], move right : Option[Node]) {
        this.value = value
        this.left = left
        this.right = right
    }
    public function size() : Int {
        let leftSize := if let Some(child) := this.left { child.size() } else { 0 }
        let rightSize := if let Some(child) := this.right { child.size() } else { 0 }
        return 1 + leftSize + rightSize
    }
    public function leftLeftValue() : Option[Int] {
        let left := this.left?
        let leftLeft := left.left?
        return Some(leftLeft.value)
    }
}
let tree := Node(2, Some(Node(1, Some(Node(0, None, None)), None)), Some(Node(3, None, None)))
print(tree.size())
match tree.leftLeftValue() {
    Some(v) => print(v),
    None => print(-1),
}
let small := Node(5, Some(Node(4, None, None)), None)
match small.leftLeftValue() {
    Some(v) => print(v),
    None => print(-1),
}
"#,
        "4\n0\n-1\n",
    );
}

#[test]
fn a_bag_pops_with_a_proof_and_without_one() {
    prints_from_file(
        r#"
class Bag {
    private var items : Array[Int]
    new() { this.items = Array.empty() }
    public function size() : Int = this.items.length()
    public mut function push(move value : Int) : void { this.items.append(value) }
    public mut function pop(proof h : this.size() > 0) : Int {
        let top := this.items[this.items.length() - 1]
        this.items.pop()
        return top
    }
    public mut function pop() : Option[Int] {
        return this.items.pop()
    }
}
let mut s := Bag()
s.push(1)
s.push(2)
while h : s.size() > 0 {
    print(s.pop(h))
}
match s.pop() {
    Some(v) => print(v),
    None => print(-1),
}
"#,
        "2\n1\n-1\n",
    );
}

#[test]
fn a_proof_parameter_takes_no_run_time_argument() {
    prints_from_file(
        r#"
function half(n : Int, proof h : n > 0) : Int = n / 2
let mut k := 3
while h : k > 0 {
    print(half(k, h))
    k = k - 1
}
"#,
        "1\n1\n0\n",
    );
}

#[test]
fn compound_assignments_update_fields_locals_and_slots() {
    prints_from_file(
        r#"
class Account {
    public var balance : Int
    new() { this.balance = 0 }
    public mut function deposit(amount : Int) : void { this.balance += amount }
    public mut function withdraw(amount : Int) : void { this.balance -= amount }
}
let mut a := Account()
a.deposit(10)
a.withdraw(3)
print(a.balance)
let mut x := 6
x *= 3
x /= 2
x %= 5
print(x)
let mut counts := Array(3, 0)
counts[1] += 5
counts[1] *= 2
print(counts[1])
"#,
        "7\n4\n10\n",
    );
}

#[test]
fn map_get_with_a_default_returns_the_value_not_an_option() {
    prints(
        r#"
let m := Map("a"=>1, "b"=>2)
print(m.get("a", 0).text())
print(m.get("z", 9).text())
"#,
        "1\n9\n",
    );
}

#[test]
fn mutable_maps_iterate_as_key_value_tuples() {
    prints(
        r#"
let mm := MutMap[String, Int]()
mm.put("x", 1)
for (kv in mm) { print(kv[0] + "=" + kv[1].text()) }
"#,
        "x=1\n",
    );
}

#[test]
fn tuple_fields_are_read_by_index_and_destructuring_swaps_locals() {
    prints(
        r#"
let t := (1, "two")
print(t[1])
let mut a := 1
let mut b := 2
(a, b) = (b, a)
print(a.text() + b.text())
"#,
        "two\n21\n",
    );
}

#[test]
fn nat_multiplication_multiplies_and_traps_on_overflow() {
    prints(
        r#"
let n : Nat = 6
print((n * 7).text())
"#,
        "42\n",
    );
    let output = execute("let n : Nat = 4294967296\nprint((n * n).text())");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Nat multiplication overflow"));
}

#[test]
fn records_build_from_a_header_and_read_their_fields() {
    prints(
        r#"
record Point(x : Int, y : Int)
let p := Point(3, 4)
print((p.x + p.y).text())
"#,
        "7\n",
    );
}

#[test]
fn recursive_variants_work_without_a_box() {
    prints(
        r#"
enum Expr {
Num(Int)
Add(Expr, Expr)
}
function evaluate(e : Expr) : Int = match e {
Num(n) => n,
Add(x, y) => evaluate(x) + evaluate(y),
}
print(evaluate(Add(Num(2), Num(3))).text())
"#,
        "5\n",
    );
}

#[test]
fn sum_product_and_fold_work_on_any_iterable_of_numbers() {
    prints(
        r#"
let xs := [1, 2, 3, 4]
print(xs.sum().text())
print(xs.product().text())
print(xs.fold(10, (acc, x) => acc + x).text())
print(Set(1, 2, 3).fold(0, (acc, x) => acc + x).text())
"#,
        "10\n24\n20\n6\n",
    );
}

#[test]
fn enumerate_and_zip_pair_items_with_positions_and_partners() {
    prints(
        r#"
let names := ["a", "b"]
for (pair in names.enumerate()) { print(pair[0].text() + pair[1]) }
for (pair in names.zip([1, 2])) { print(pair[1].text() + pair[0]) }
"#,
        "0a\n1b\n1a\n2b\n",
    );
}

#[test]
fn sorted_and_sort_take_a_key_and_keep_equal_items_in_order() {
    prints(
        r#"
let words := ["bb", "aa", "c"]
print(words.sorted(w => w.length()).join(","))
let mut arr : Array[String] = Array.generate(0, i => "x")
arr.append("bb")
arr.append("aa")
arr.append("c")
arr.sort(w => w.length())
print(arr.join(","))
"#,
        "c,bb,aa\nc,bb,aa\n",
    );
}

#[test]
fn center_traps_when_the_fill_is_not_one_character() {
    let output = execute("print(\"abc\".center(9, \"**\"))");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("The fill character must be exactly one character")
    );
}

#[test]
fn python_string_methods_agree_with_cpython_311() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = check_path(root.join("tests/fixtures/pythonstrings.kes"), root).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let output = execute_program(&core::lower(&project).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = fs::read_to_string(root.join("tests/fixtures/pythonstrings.expected")).unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}

#[test]
fn tensor_construction_indexing_slices_multiplication_and_broadcast() {
    prints(
        r#"
let a := Tensor.generate((5, 6), (i, j) => (i * 6 + j).toDouble())
print(a[1, 2])
a[1, 2] = 9.0
print(a[1, 2])
let s : Tensor[double, (2, 2)] = a[1:3, 2:4]
s[0, 0] = 100.0
print(a[1, 2])
print(s[0, 0])
let row : Tensor[double, (6,)] = a[1, :]
print(row[2])
let tail : Tensor[double, (3, 2)] = a[2:, :2]
print(tail[2, 1])
let i := 1
let j := 3
let t : DynTensor[double] = a[i:j, 0:2]
print(t[1, 1])
let z : Tensor[double, (2, 3)] = Tensor.zeros((2, 3))
let o := Tensor.onesLike(z)
let f := Tensor.filled((2, 3), 0.5)
let p := a * a
print(p[1, 2])
let q := 2.0 * a
let r := a * 3.0
print(q[1, 2])
print(r[1, 2])
let w : Tensor[double, (4, 2, 3)] = f.broadcastTo((4, 2, 3))
w[0, 0, 0] = 7.0
print(f[0, 0])
print(w[3, 1, 2])
print(o[1, 2])
let copied := a.copy()
copied[0, 0] = 77.0
print(a[0, 0])
print(copied[0, 0])
"#,
        "8.0\n9.0\n9.0\n100.0\n9.0\n25.0\n13.0\n81.0\n18.0\n27.0\n0.5\n0.5\n1.0\n0.0\n77.0\n",
    );
}

#[test]
fn tensor_matrix_multiplication_infers_shape_variables() {
    prints(
        r#"
function product(a : Tensor[double, (m, k)], b : Tensor[double, (k, n)]) : Tensor[double, (m, n)] = a @ b
let a := Tensor.generate((2, 3), (i, j) => (i * 3 + j + 1).toDouble())
let b := Tensor.onesLike(Tensor.zeros((3, 4)))
let m : Tensor[double, (2, 4)] = product(a, b)
print(m[0, 3])
print(m[1, 0])
"#,
        "6.0\n15.0\n",
    );
}

#[test]
fn tensor_dynamic_construction_checked_shape_and_independent_owners() {
    prints(
        r#"
function readChecked(v : DynTensor[double]) : double or Error {
    let c : Tensor[double, (3,)] = v.checkShape(3)?
    c[0] = 9.0
    return c[0]
}
let xs := Array.generate(3, i => (i + 1).toDouble())
let v : DynTensor[double] = Tensor.fromArray(xs)
xs[0] = 7.0
print(v[0])
print(match readChecked(v) { x => x, _ => 0.0, })
print(v[0])
print(match v.checkShape(4) { RuntimeError.Message(message) => message, _ => "unexpected", })
let n := 2
let d : DynTensor[double] = Tensor.generate((n, 3), (i, j) => (i + j).toDouble())
let o : DynTensor[double] = Tensor.onesLike(d)
let p : DynTensor[double] = d * o
print(p[1, 2])
let m := d @ Tensor.filled((3, 2), 2.0)
print(m[1, 1])
let w := v.broadcastTo((2, 3))
print(w[1, 2])
"#,
        "1.0\n9.0\n1.0\nTensor shape check failed\n3.0\n12.0\n3.0\n",
    );
}

#[test]
fn tensor_empty_slices_and_zero_dimensions_do_not_call_factories() {
    prints(
        r#"
let mut calls := 0
let a := Tensor.generate((0, 3), (i, j) => { calls = calls + 1
    4.0 })
print(calls)
let b := Tensor.zeros((3, 0)) @ a
print(b[2, 2])
let c : Tensor[double, (0, 2)] = Tensor.zeros((2, 3))[1:1, 0:2]
let d : Tensor[double, (0, 2)] = c.copy()
print(7)
"#,
        "0\n0.0\n7\n",
    );
}

#[test]
fn tensor_runtime_shape_rank_and_bounds_fail_with_messages() {
    for (source, message) in [
        (
            "let n := 2\nlet a := Tensor.zeros((n, 3))\nlet b := Tensor.zeros((3, 2))\nlet p := a * b",
            "equal shapes",
        ),
        (
            "let n := 2\nlet a := Tensor.zeros((n, 3))\nlet b := Tensor.zeros((2, 4))\nlet p := a @ b",
            "inner dimensions",
        ),
        (
            "let n := 2\nlet a := Tensor.zeros((n,))\nlet p := a @ a",
            "requires a matrix",
        ),
        (
            "let n := 2\nlet a := Tensor.zeros((n, 3))\nprint(a[0])",
            "index count",
        ),
        (
            "let a := Tensor.zeros((2, 3))\nprint(a[2, 0])",
            "index out of range",
        ),
        (
            "let a := Tensor.zeros((2, 3))\na[0, -1] = 4.0",
            "index out of range",
        ),
        (
            "let a := Tensor.zeros((2, 3))\nlet b := a[0:3, :]",
            "slice bounds",
        ),
        (
            "let a := Tensor.zeros((2, 3))\nlet b := a[1:0, :]",
            "slice bounds",
        ),
        (
            "let a := Tensor.zeros((2, 3))\nlet b := a[-1:, :]",
            "slice bounds",
        ),
        (
            "let n := 4\nlet a := Tensor.zeros((2, 3))\nlet b := a.broadcastTo((2, n))",
            "broadcast",
        ),
        ("let n := -1\nlet a := Tensor.zeros((n, 3))", "negative"),
        (
            "let n := 9223372036854775807\nlet a := Tensor.zeros((n, 2))",
            "too large",
        ),
    ] {
        let output = execute(source);
        assert!(!output.status.success(), "{source}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{source}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn tensor_borrowed_parameters_return_independent_slices_and_trait_results() {
    prints(
        r#"
function scale[T : Multipliable[double](Output = R)](value : T) : R = value * 2.0
function rightScale[T : RightMultipliable[double](Output = R)](value : T) : R = 3.0 * value
function slice(a : Tensor[double, (2, 3)]) : Tensor[double, (1, 2)] = a[0:1, 1:3]
function dynamic(a : DynTensor[double]) : double = a[0, 0]
function change(mut a : Tensor[double, (2, 3)]) : void { a[0, 0] = 4.0 }
let a := Tensor.filled((2, 3), 0.5)
let s := slice(a)
s[0, 0] = 9.0
let b : Tensor[double, (2, 3)] = scale(a)
let c : Tensor[double, (2, 3)] = rightScale(a)
print(a[0, 1])
print(b[0, 0])
print(c[0, 0])
print(dynamic(a))
change(a)
print(a[0, 0])
let moved : DynTensor[double] = a
print(moved[0, 0])
"#,
        "0.5\n1.0\n1.5\n0.5\n4.0\n4.0\n",
    );
}

#[test]
fn tensor_matrix_precedence_keeps_override_annotations() {
    prints(
        r#"
class Parent { new() {}
public function value() : Int = 1 }
class Child extends Parent { new() {}
@override public function value() : Int = 2 }
let a := Tensor.filled((2, 2), 2.0)
let b := Tensor.onesLike(a)
let c := a @ b * 2.0
let d := 2.0 * a @ b
print(c[0, 0])
print(d[0, 0])
print(Child().value())
"#,
        "8.0\n8.0\n2\n",
    );
}

#[test]
fn tensor_named_factories_boxes_and_stored_borrows() {
    prints(
        r#"
class View {
    public var value : &Tensor[double, (2, 3)]
    new(a : Tensor[double, (2, 3)]) { this.value = a }
}
let a := Tensor.filled(value = 0.5, shape = (2, 3))
let view := View(a)
let b := view.value * 2.0
b[0, 0] = 4.0
print(a[0, 0])
print(b[0, 0])
let boxed := box(Tensor.generate(factory = (i, j) => (i + j).toDouble(), shape = (2, 3)))
let unboxedCopy := boxed.copy()
print(unboxedCopy[1, 2])
"#,
        "0.5\n4.0\n3.0\n",
    );
}

#[test]
fn tensor_ones_and_zeros_like_fill_the_same_shape() {
    prints(
        r#"
let o : Tensor[double, (2, 3)] = Tensor.ones((2, 3))
print(o[1, 2])
let z := Tensor.zerosLike(o)
z[1, 2] = 4.0
print(z[1, 2])
print(z[0, 0])
print(o[1, 2])
"#,
        "1.0\n4.0\n0.0\n1.0\n",
    );
}

#[test]
fn a_trait_that_names_text_gives_its_objects_text() {
    prints(
        r#"
trait Expr {
    has Text
    public function eval() : Int
}

class Val {
    has Expr
    public final value : Int
    new(value : Int) { this.value = value }
    public function eval() : Int = this.value
    public function text() : String = f"v{this.value}"
}

class Twice {
    has Expr
    public final inner : Val
    new(move inner : Val) { this.inner = inner }
    public function eval() : Int = this.inner.eval() * 2
    public function text() : String = f"twice {this.inner.text()}"
}

function describe[T : Text](x : T) : String = x.text()

let items : List[Box[dyn Expr]] = [box(Val(4)), box(Twice(Val(5)))]
for (item in items) {
    print(item.text())
    print(item.eval())
}
print(describe(Val(7)))
"#,
        "v4\n4\ntwice v5\n10\nv7\n",
    );
}

#[test]
fn tensor_text_follows_the_shape_and_satisfies_text() {
    prints(
        r#"
let a := Tensor.generate((2, 3), (i, j) => (i * 3 + j).toDouble())
print(a)
print(a.text())
print(Tensor.filled((4,), 0.5))
print(Tensor.zeros((0, 3)))
print(Tensor.zeros((2, 0)))
function show[T : Text](x : T) : String = x.text()
print(show(a))
let xs : Array[double] = Array.generate(3, i => (i + 1).toDouble())
print(Tensor.fromArray(xs))
"#,
        "[[0.0, 1.0, 2.0],\n [3.0, 4.0, 5.0]]\n[[0.0, 1.0, 2.0],\n [3.0, 4.0, 5.0]]\n[0.5, 0.5, 0.5, 0.5]\n[]\n[[],\n []]\n[[0.0, 1.0, 2.0],\n [3.0, 4.0, 5.0]]\n[1.0, 2.0, 3.0]\n",
    );
}

#[test]
fn a_trait_that_names_iterator_gives_its_objects_next() {
    prints(
        r#"
trait Ticker {
    has Iterator(Item = Int)
    public function label() : String
}

class Count {
    has Ticker
    public var n : Int
    new() { this.n = 0 }
    public function label() : String = "count"
    public mut function next() : Option[Int] {
        this.n = this.n + 1
        if this.n < 3 then Some(this.n) else None
    }
}

function show(mut x : &dyn Ticker) : void with Effects {
    print(x.label())
    print(x.next())
}

let t : Box[dyn Ticker] = box(Count())
print(t.next())
show(t)
print(t.next())
"#,
        "Some(1)\ncount\nSome(2)\nNone\n",
    );
}

#[test]
fn tensor_transpose_reverses_or_permutes_axes() {
    prints(
        r#"
let a := Tensor.generate((2, 3), (i, j) => (i * 3 + j).toDouble())
print(a.transpose())
let c := Tensor.generate((2, 3, 4), (i, j, k) => (i * 100 + j * 10 + k).toDouble())
let t := c.transpose()
print(t[3, 2, 1])
let p := c.transpose((1, 0, 2))
print(p[2, 1, 3])
print(p.transpose((2, 0, 1))[3, 2, 1])
let xs : Array[double] = Array.generate(3, i => (i + 1).toDouble())
print(Tensor.fromArray(xs).transpose())
"#,
        "[[0.0, 3.0],\n [1.0, 4.0],\n [2.0, 5.0]]\n123.0\n123.0\n123.0\n[1.0, 2.0, 3.0]\n",
    );
}

#[test]
fn tensor_transpose_with_bad_axes_stops_at_run_time() {
    for (source, message) in [
        (
            "let n := 2\nlet m := Tensor.zeros((n, 3))\nprint(m.transpose((0, 0)))",
            "needs each axis once",
        ),
        (
            "let xs : Array[double] = Array.generate(3, i => (i + 1).toDouble())\nlet v := Tensor.fromArray(xs)\nprint(v.transpose((0, 1)))",
            "one axis for each dimension",
        ),
    ] {
        let output = execute(source);
        assert!(!output.status.success(), "{source}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{source}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn tensor_debug_shows_type_and_values() {
    prints(
        r#"
let t := Tensor.generate((2, 3), (i, j) => (i * 3 + j).toDouble())
print(t.debug())
print(f"{t:?}")
let n := 3
let d := Tensor.ones((n,))
print(d.debug())
print(Tensor.filled((), 2.0).debug())
print(t.text())
"#,
        "Tensor[double, (2, 3)]\n[[0.0, 1.0, 2.0],\n [3.0, 4.0, 5.0]]\nTensor[double, (2, 3)]\n[[0.0, 1.0, 2.0],\n [3.0, 4.0, 5.0]]\nTensor[double, (3,)] [1.0, 1.0, 1.0]\nTensor[double, ()] 2.0\n[[0.0, 1.0, 2.0],\n [3.0, 4.0, 5.0]]\n",
    );
}

#[test]
fn tensor_shape_queries_report_rank_shape_and_count() {
    prints(
        r#"
let a := Tensor.zeros((2, 3))
print(a.rank())
print(a.shape())
print(a.count())
let shape := a.shape()
shape[0] = 9
print(a.shape())
let n := 4
let d := Tensor.ones((n,))
print(d.rank())
print(d.shape())
print(d.count())
let scalar := Tensor.ones(())
print(scalar.rank())
print(scalar.shape())
print(scalar.count())
"#,
        "2\n[2, 3]\n6\n[2, 3]\n1\n[4]\n4\n0\n[]\n1\n",
    );
}

#[test]
fn tensor_elementwise_add_and_subtract() {
    prints(
        r#"
function plus[T : Addable[double](Output = R)](value : T) : R = value + 2.0
function minus[T : RightSubtractable[double](Output = R)](value : T) : R = 10.0 - value
let a := Tensor.generate((2, 3), (i, j) => (i * 3 + j).toDouble())
let b : Tensor[double, (2, 3)] = a + a
print(b)
print(a - a)
print(a + 1.0)
print(1.0 + a)
print(a - 1.0)
print(10.0 - a)
print(plus(a))
print(minus(a))
print(a)
"#,
        "[[0.0, 2.0, 4.0],\n [6.0, 8.0, 10.0]]\n[[0.0, 0.0, 0.0],\n [0.0, 0.0, 0.0]]\n[[1.0, 2.0, 3.0],\n [4.0, 5.0, 6.0]]\n[[1.0, 2.0, 3.0],\n [4.0, 5.0, 6.0]]\n[[-1.0, 0.0, 1.0],\n [2.0, 3.0, 4.0]]\n[[10.0, 9.0, 8.0],\n [7.0, 6.0, 5.0]]\n[[2.0, 3.0, 4.0],\n [5.0, 6.0, 7.0]]\n[[10.0, 9.0, 8.0],\n [7.0, 6.0, 5.0]]\n[[0.0, 1.0, 2.0],\n [3.0, 4.0, 5.0]]\n",
    );
}

#[test]
fn tensor_map_applies_a_function_to_each_element() {
    prints(
        r#"
let a := Tensor.generate((2, 3), (i, j) => (i * 3 + j).toDouble())
let b : Tensor[double, (2, 3)] = a.map(x => x * 0.5)
print(b)
b[0, 0] = 9.0
print(a[0, 0])
let mut calls := 0
let c := a.map(x => { calls = calls + 1
    x + 1.0 })
print(calls)
print(c[1, 2])
let e := Tensor.zeros((0, 3)).map(x => { calls = calls + 1
    x })
print(calls)
print(e)
let n := 2
let d : DynTensor[double] = Tensor.ones((n,)).map(x => x + 2.0)
print(d)
"#,
        "[[0.0, 0.5, 1.0],\n [1.5, 2.0, 2.5]]\n0.0\n6\n6.0\n6\n[]\n[3.0, 3.0]\n",
    );
}

#[test]
fn tensor_reshape_keeps_row_major_order() {
    prints(
        r#"
let a := Tensor.generate((2, 3), (i, j) => (i * 3 + j).toDouble())
let b : Tensor[double, (3, 2)] = a.reshape((3, 2))
print(b)
b[0, 0] = 9.0
print(a[0, 0])
let n := 3
let d : DynTensor[double] = a.reshape((n, 2))
print(d)
print(Tensor.ones((1,)).reshape(()).debug())
print(Tensor.zeros((0, 3)).reshape((2, 0)))
"#,
        "[[0.0, 1.0],\n [2.0, 3.0],\n [4.0, 5.0]]\n0.0\n[[0.0, 1.0],\n [2.0, 3.0],\n [4.0, 5.0]]\nTensor[double, ()] 1.0\n[[],\n []]\n",
    );
}

#[test]
fn tensor_reductions_along_axes() {
    prints(
        r#"
let t := Tensor.generate((2, 3), (i, j) => (i * 3 + j + 1).toDouble())
print(t.sum())
print(t.mean())
print(t.min())
print(t.max())
let columns : Tensor[double, (3,)] = t.sum(axis: 0)
let rows : Tensor[double, (2,)] = t.sum(axis = 1)
print(columns)
print(rows)
print(t.mean(axis: 0))
print(t.min(axis: 1))
print(t.max(axis: 0))
let axis := 1
let d : DynTensor[double] = t.sum(axis: axis)
print(d)
let v := Tensor.ones((3,))
let scalar : double = v.sum(axis: 0)
print(scalar)
let n := 2
let dynamic := Tensor.ones((n, 3))
print(dynamic.sum(axis: 1))
let xs := Array.generate(3, i => (i + 1).toDouble())
let zero : DynTensor[double] = Tensor.fromArray(xs).sum(axis: 0)
print(zero.rank())
print(zero.shape())
print(zero[])
print(zero.debug())
print(Tensor.zeros((0, 3)).sum())
print(Tensor.zeros((2, 0)).sum(axis: 1))
"#,
        "21.0\n3.5\n1.0\n6.0\n[5.0, 7.0, 9.0]\n[6.0, 15.0]\n[2.5, 3.5, 4.5]\n[1.0, 4.0]\n[4.0, 5.0, 6.0]\n[6.0, 15.0]\n3.0\n[3.0, 3.0]\n0\n[]\n6.0\nTensor[double, ()] 6.0\n0.0\n[0.0, 0.0]\n",
    );
}

#[test]
fn tensor_phase_two_runtime_errors_report_shape_axes_and_empty_values() {
    for (source, message) in [
        (
            "let n := 2\nlet a := Tensor.ones((n, 3))\nlet b := a + Tensor.ones((3, 2))",
            "equal shapes",
        ),
        (
            "let n := 2\nlet a := Tensor.ones((n, 3))\nlet b := a - Tensor.ones((1, 3))",
            "equal shapes",
        ),
        (
            "let n := 2\nlet a := Tensor.ones((n, 3))\nlet b := a.reshape((3, 3))",
            "Tensor reshape needs the same number of elements",
        ),
        (
            "let axis := -1\nprint(Tensor.ones((2, 3)).sum(axis: axis))",
            "Tensor axis out of range",
        ),
        (
            "let n := 2\nprint(Tensor.ones((n, 3)).sum(axis: 2))",
            "Tensor axis out of range",
        ),
        ("print(Tensor.zeros((0, 3)).min())", "Tensor is empty"),
        (
            "print(Tensor.zeros((2, 0)).max(axis: 1))",
            "Tensor is empty",
        ),
        ("print(Tensor.zeros((0, 3)).mean())", "Tensor is empty"),
    ] {
        let output = execute(source);
        assert!(!output.status.success(), "{source}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{source}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn tensor_middle_axis_reductions_and_map_use_independent_storage() {
    prints(
        r#"
let a := Tensor.generate((2, 3, 4), (i, j, k) => (i * 100 + j * 10 + k).toDouble())
let sum : Tensor[double, (2, 4)] = a.sum(axis: 1)
print(sum)
print(a.max(axis: 2))
let source := Tensor.ones((2,))
let mapped := source.map(x => { source[1] = 9.0
    x * 2.0 })
print(mapped)
print(source)
let empty := Tensor.zeros((0, 9223372036854775807, 9223372036854775807))
print(empty.debug())
print(empty.transpose().count())
"#,
        "[[30.0, 33.0, 36.0, 39.0],\n [330.0, 333.0, 336.0, 339.0]]\n[[3.0, 13.0, 23.0],\n [103.0, 113.0, 123.0]]\n[2.0, 2.0]\n[1.0, 9.0]\nTensor[double, (0, 9223372036854775807, 9223372036854775807)]\n[]\n0\n",
    );
}

#[test]
fn conversion_constructors_and_direct_tensor_shapes() {
    prints(
        r#"
let xs := [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
let x := Tensor(xs, shape = (3, 2))
print(x)
print(Tensor([0.1, 0.2, 0.5]))
let a := Array([1, 2, 3])
a[0] = 9
print(a)
let b := Array(a)
print(a)
print(b)
print(double(3 + 4))
let n : Nat = 3
print(double(n))
print(double(1.5))
print(String(3))
print(String(x))
let mu := x.mean(axis: 0)
let xCentered := x - mu.broadcastTo((3, 2))
print(xCentered)
let ones := Tensor.ones((3, 1))
print(x - ones @ mu.reshape((1, 2)))
"#,
        "[[1.0, 2.0],\n [3.0, 4.0],\n [5.0, 6.0]]\n[0.10000000000000001, 0.20000000000000001, 0.5]\n[9, 2, 3]\n[9, 2, 3]\n[9, 2, 3]\n7.0\n3.0\n1.5\n3\n[[1.0, 2.0],\n [3.0, 4.0],\n [5.0, 6.0]]\n[[-2.0, -2.0],\n [0.0, 0.0],\n [2.0, 2.0]]\n[[-2.0, -2.0],\n [0.0, 0.0],\n [2.0, 2.0]]\n",
    );
}

#[test]
fn uppercase_and_underscore_values_index_and_call_normally() {
    prints(
        r#"
function head(X_c : Tensor[double, (2, 2)], Row : Int) : double = X_c[Row, 0]
let X_c := Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2))
let A := X_c.copy()
let I := 1
print(A[I, 1])
print(head(X_c, 1))
print(f"{X_c[0, 1]}")
let (N, K) := (2, 2)
print(N + K)
let values := Array.generate(2, Index => Index + 1)
print(values)
for (Row in [0, 1]) { print(X_c[Row, 0]) }
print([Word[0] for Word in ["ab", "cd"]])
"#,
        "4.0\n3.0\n2.0\n4\n[1, 2]\n1.0\n3.0\n[a, c]\n",
    );
}

#[test]
fn tensor_shapes_accept_arrays_and_shape_queries_consistently() {
    prints(
        r#"
let X := Tensor([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], shape = (3, 2))
let Shape := X.shape()
let mu := X.mean(axis: 0)
let X_c := X - mu.broadcastTo(X.shape())
print(X_c)
print(Tensor.ones(Shape))
print(Tensor([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], shape = Shape))
let reshaped := X.reshape(Array([2, 3]))
print(reshaped)
print(Tensor.generate(Shape, (I, J) => double(I + J)))
print(Tensor.filled(Shape, 0.5))
print(Tensor.zeros(Shape))
let dims := reshaped.shape()
print(dims[0])
print(dims[1])
"#,
        "[[-2.0, -2.0],\n [0.0, 0.0],\n [2.0, 2.0]]\n[[1.0, 1.0],\n [1.0, 1.0],\n [1.0, 1.0]]\n[[1.0, 2.0],\n [3.0, 4.0],\n [5.0, 6.0]]\n[[1.0, 2.0, 3.0],\n [4.0, 5.0, 6.0]]\n[[0.0, 1.0],\n [1.0, 2.0],\n [2.0, 3.0]]\n[[0.5, 0.5],\n [0.5, 0.5],\n [0.5, 0.5]]\n[[0.0, 0.0],\n [0.0, 0.0],\n [0.0, 0.0]]\n2\n3\n",
    );
}

#[test]
fn tensor_array_shapes_validate_counts_bounds_and_generator_rank() {
    for (source, message) in [
        (
            "let xs := [1.0, 2.0]\nlet x := Tensor(xs, shape = (2, 2))",
            "Tensor shape does not match the number of elements",
        ),
        (
            "let x := Tensor([1.0], shape = Array([2]))",
            "Tensor shape does not match the number of elements",
        ),
        (
            "let x := Tensor.zeros(Array([-1, 2]))",
            "Tensor dimensions cannot be negative",
        ),
        (
            "let x := Tensor.ones((2,))\nprint(x.broadcastTo(Array([3])))",
            "Tensor dimensions cannot broadcast to target shape",
        ),
        (
            "let x := Tensor.ones((2,))\nprint(x.reshape(Array([3])))",
            "Tensor reshape needs the same number of elements",
        ),
        (
            "let x := Tensor.generate(Array([0, 2]), I => double(I))",
            "Tensor rank does not match generator parameter count",
        ),
        (
            "let x := Tensor.ones((2, 3))\nprint(x.transpose(Array([0, 0])))",
            "Tensor transpose needs each axis once",
        ),
        (
            "let X := Tensor([1.0])\nlet Axes : Array[Int] = Array([])\nprint(X.transpose(Axes))",
            "Tensor transpose needs one axis for each dimension",
        ),
        (
            "let xs := [1.0]\nlet X := Tensor(xs)\nprint(X.transpose(()))",
            "Tensor transpose needs one axis for each dimension",
        ),
    ] {
        let output = execute(source);
        assert!(!output.status.success(), "{source}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{source}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn tensor_constructors_copy_sources_and_accept_zero_rank_array_shapes() {
    prints(
        r#"
let A := Array([1.0, 2.0])
let X := Tensor(A)
A[0] = 9.0
print(X)
print(A)
let Shape : Array[Int] = Array([])
print(Tensor([7.0], shape = Shape).debug())
print(Tensor.generate(Shape, () => 4.0).debug())
print(Tensor([], shape = Array([2, 0])))
print(Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2)).transpose(Array([1, 0])))
let Copy := Array(range(3).map(I => I + 1))
print(Copy)
class Label { has Text
    new() {}
    public function text() : String = "label"
}
print(String(Label()))
"#,
        "[1.0, 2.0]\n[9.0, 2.0]\nTensor[double, ()] 7.0\nTensor[double, ()] 4.0\n[[],\n []]\n[[1.0, 3.0],\n [2.0, 4.0]]\n[1, 2, 3]\nlabel\n",
    );
}

#[test]
fn tensor_scalar_division_supports_covariance_generics_and_ieee_values() {
    prints(
        r#"
function halve[T : Divisible[double](Output = R)](value : T) : R = value / 2.0
function covarianceMatrix(a : Tensor[double, (n, k)]) : Tensor[double, (k, k)] {
    let aCentered := a - a.mean(axis = 0).broadcastTo(a.shape())
    return (aCentered.transpose() @ aCentered) / double(aCentered.shape()[0])
}
let X := Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2))
let Y := X / 2.0
Y[0, 0] = 9.0
print(X)
print(Y)
print(X.divide(2.0))
print(halve(X))
print(covarianceMatrix(X))
let D := Tensor.ones(X.shape())
print(D / 4.0)
print(Tensor([1e-320]) / 1e-320)
let Z := Tensor([1.0, -1.0, 0.0]) / 0.0
print(Z[0].isFinite())
print(Z[1] < 0.0)
print(Z[2] == Z[2])
print(Tensor.zeros((0, 2)) / 0.0)
print((1.0 / X.shape()[0].toDouble()) * (X.transpose() @ X))
"#,
        "[[1.0, 2.0],\n [3.0, 4.0]]\n[[9.0, 1.0],\n [1.5, 2.0]]\n[[0.5, 1.0],\n [1.5, 2.0]]\n[[0.5, 1.0],\n [1.5, 2.0]]\n[[1.0, 1.0],\n [1.0, 1.0]]\n[[0.25, 0.25],\n [0.25, 0.25]]\n[1.0]\nfalse\ntrue\nfalse\n[]\n[[5.0, 7.0],\n [7.0, 10.0]]\n",
    );
}

#[test]
fn numerical_example_shows_vector_math_statistics_and_seeded_sampling() {
    prints(
        include_str!("../testdata/language/numerical.kes"),
        "[5.0, 11.0]\n[7.0, 10.0]\n5.0\n[[1.0, 2.0],\n [2.0, 4.0]]\n[1.0, 2.0]\n[1.0, 2.0]\ntrue\ntrue\ntrue\n2.75\n[3.0, 2.5]\n0.6875\ntrue\ntrue\ntrue\ntrue\n0.5\n",
    );
}

#[test]
fn numerical_modules_vectors_and_native_factorizations() {
    prints(
        r#"
import std.linalg
import std.math
import std.stats
let A := Tensor([4.0, 2.0, 2.0, 3.0], shape = (2, 2))
let x := Tensor([1.0, 2.0])
print(A @ x)
print(x @ A)
print(linalg.dot(x, x))
print(linalg.outer(x, x))
print((x.norm() - 5.0.sqrt()).abs() < 1e-12)
print(linalg.norm(A) == A.norm())
print((linalg.spectralNorm(A) - 5.56155281280883).abs() < 1e-12)
print((linalg.frobeniusNorm(A) - 5.744562646538029).abs() < 1e-12)
print((linalg.solve(A, A @ x) - x).norm() < 1e-12)
let lu := linalg.lu(A)
print((lu.solve(A @ x) - x).norm() < 1e-12)
let qr := linalg.qr(A)
print((qr.q @ qr.r - A).norm() < 1e-12)
let L := linalg.cholesky(A)
print((L @ L.transpose() - A).norm() < 1e-12)
let svd := linalg.svd(A)
let S := Tensor.generate((2, 2), (i, j) => if i == j then svd.s[i] else 0.0)
print((svd.u @ S @ svd.vt - A).norm() < 1e-12)
let eig := linalg.eigh(A)
let W := Tensor.generate((2, 2), (i, j) => if i == j then eig.values[i] else 0.0)
print((eig.vectors @ W @ eig.vectors.transpose() - A).norm() < 1e-12)
print((A @ linalg.inverse(A) - Tensor([1.0, 0.0, 0.0, 1.0], shape = (2, 2))).norm() < 1e-12)
print(linalg.rank(A))
print((linalg.det(A) - 8.0).abs() < 1e-12)
print((linalg.pinv(A) - linalg.inverse(A)).norm() < 1e-12)
print((math.log(math.exp(x)) - x).norm() < 1e-12)
print(stats.variance(x))
print(stats.variance(x, ddof = 1))
print(stats.covariance(A))
"#,
        "[8.0, 8.0]\n[8.0, 8.0]\n5.0\n[[1.0, 2.0],\n [2.0, 4.0]]\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\n2\ntrue\ntrue\ntrue\n0.25\n0.5\n[[1.0, -0.5],\n [-0.5, 0.25]]\n",
    );
}

#[test]
fn numerical_scalar_math_triangular_solves_and_three_dimensional_statistics() {
    prints(
        r#"
import std.linalg
import std.math
import std.stats
let L := Tensor([2.0, 0.0, 3.0, 4.0], shape = (2, 2))
print(linalg.solveLower(L, Tensor([2.0, 11.0])))
let U := Tensor([2.0, 3.0, 0.0, 4.0], shape = (2, 2))
print(linalg.solveUpper(U, Tensor([8.0, 8.0])))
let n := 3
let v := Tensor.generate((n,), i => double(i + 1))
let M := Tensor.generate((n, 2), (i, j) => double(i * 2 + j + 1))
print(v @ M)
print((math.log(math.exp(v)) - v).norm() < 1e-12)
print(math.sin(0.0))
print(math.sqrt(4.0))
print(math.abs(-2.5))
print(math.floor(-1.2))
print(math.round(2.5))
let C := Tensor([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0], shape = (2, 2, 2))
print(stats.mean(C, axis = 0))
print(stats.mean(C, axis = 1))
print(stats.variance(C, axis = 2))
"#,
        "[1.0, 2.0]\n[1.0, 2.0]\n[22.0, 28.0]\ntrue\n0.0\n2.0\n2.5\n-2.0\n3.0\n[[3.0, 4.0],\n [5.0, 6.0]]\n[[2.0, 3.0],\n [6.0, 7.0]]\n[[0.25, 0.25],\n [0.25, 0.25]]\n",
    );
}

#[test]
fn numerical_rectangular_solves_statistics_and_random_generators() {
    prints(
        r#"
import std.linalg as la
import std.stats as st
import std.random.{Rng}
import std.prob.{Normal, Uniform}
let A := Tensor([1.0, 0.0, 0.0, 1.0, 1.0, 1.0], shape = (3, 2))
let b := Tensor([1.0, 2.0, 3.0])
print((la.leastSquares(A, b) - Tensor([1.0, 2.0])).norm() < 1e-12)
print((A @ la.pinv(A) @ A - A).norm() < 1e-12)
let qr := la.qr(A)
print(qr.q.shape())
print(qr.r.shape())
print((qr.q @ qr.r - A).norm() < 1e-12)
let svd := la.svd(A)
print(svd.u.shape())
print(svd.s.shape())
print(svd.vt.shape())
print(st.mean(A, axis = 0))
print((st.variance(A, axis = 0) - Tensor([2.0 / 9.0, 2.0 / 9.0])).norm() < 1e-12)
print(st.stddev(A, axis = 1))
print(st.correlation(Tensor([1.0, 2.0, 2.0, 4.0, 3.0, 6.0], shape = (3, 2))))
let rng := Rng.seeded(42)
let same := Rng.seeded(42)
print(rng.integers(0, 10000) == same.integers(0, 10000))
print(rng.uniform(-1.0, 1.0) == same.uniform(-1.0, 1.0))
print(rng.normal(0.0, 1.0) == same.normal(0.0, 1.0))
let normal := Normal.create(0.0, 1.0)
print((normal.pdf(0.0) - 0.3989422804014327).abs() < 1e-15)
print(normal.cdf(0.0))
print(normal.sample(rng).isFinite())
let uniform := Uniform.create(-2.0, 2.0)
print(uniform.pdf(0.0))
print(uniform.cdf(0.0))
let draw := uniform.sample(rng)
print(draw >= -2.0 && draw < 2.0)
"#,
        "true\ntrue\n[3, 2]\n[2, 2]\ntrue\n[3, 2]\n[2]\n[2, 2]\n[0.66666666666666663, 0.66666666666666663]\ntrue\n[0.5, 0.5, 0.0]\n[[1.0, 1.0],\n [1.0, 1.0]]\ntrue\ntrue\ntrue\ntrue\n0.5\ntrue\n0.25\n0.5\ntrue\n",
    );
}

#[test]
fn tensor_and_class_power_and_negation_follow_the_operator_traits() {
    prints(
        r#"
let A := Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2))
let v := Tensor([1.0, 2.0, 3.0])
print(A ** 0)
print(A ** 1)
print(A ** 2)
print(A ** 3)
print(A ** 2.0)
print(v ** 2)
print(v ** 2.0)
print(-A)
print(A.negate())
print(A.power(2))
let flat := Tensor([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], shape = (2, 3))
print(flat ** 2)
class MyBox {
    public final x : Int
    new(x : Int) { this.x = x }
    has Negatable, Powerable
    public function negate() : MyBox = MyBox(-this.x)
    public function power(exponent : Int) : MyBox = MyBox(this.x ** exponent)
}
let b := MyBox(3)
print((-b).x)
print((b ** 2).x)

"#,
        "[[1.0, 0.0],\n [0.0, 1.0]]\n[[1.0, 2.0],\n [3.0, 4.0]]\n[[7.0, 10.0],\n [15.0, 22.0]]\n[[37.0, 54.0],\n [81.0, 118.0]]\n[[1.0, 4.0],\n [9.0, 16.0]]\n[1.0, 4.0, 9.0]\n[1.0, 4.0, 9.0]\n[[-1.0, -2.0],\n [-3.0, -4.0]]\n[[-1.0, -2.0],\n [-3.0, -4.0]]\n[[7.0, 10.0],\n [15.0, 22.0]]\n[[1.0, 4.0, 9.0],\n [16.0, 25.0, 36.0]]\n-3\n9\n",
    );
}

#[test]
fn probability_example_uses_objects_static_methods_and_shapes() {
    prints(
        include_str!("../testdata/language/probability.kes"),
        "1.0\ntrue\n0.5\n[5]\n[2, 3]\n4\ntrue\ntrue\ntrue\n",
    );
}

#[test]
fn probability_objects_expose_named_parameters_and_instance_methods() {
    prints_close(
        r#"
import std.prob.{Normal, Binomial, Poisson, NegativeBinomial, Hypergeometric, DiscreteUniform, Bernoulli}
import std.random.{Rng}
let n := Normal.create(mean = 0.5, sigma = 2.0)
print(n.mean)
print(n.sigma)
print(n.pdf(0.5))
print(n.cdf(0.5))
let b := Binomial.create(trials = 10, p = 0.3)
print(b.trials)
print(b.p)
print(b.pmf(3))
let p := Poisson.create(lambda = 2.5)
print(p.lambda)
print(p.pmf(2))
let nb := NegativeBinomial.create(successes = 3, p = 0.5)
print(nb.successes)
print(nb.pmf(2))
let h := Hypergeometric.create(population = 20, successes = 7, draws = 5)
print(h.population)
print(h.successes)
print(h.draws)
print(h.pmf(2))
let u := DiscreteUniform.create(low = 1, high = 6)
print(u.low)
print(u.high)
let rng := Rng.seeded(7)
print(Bernoulli.create(0.5).sample(rng) >= 0)

"#,
        "0.5\n2.0\n0.19947114020071632\n0.5\n10\n0.29999999999999999\n0.2668279320000006\n2.5\n0.25651562069968376\n3\n0.18749999999999992\n20\n7\n5\n0.38738390092879071\n1\n6\ntrue\n",
    );
}

#[test]
fn probability_static_sampling_covers_continuous_and_discrete_distributions() {
    prints(
        r#"
import std.prob.{Normal, Uniform, Exponential, Gamma, Beta, ChiSquared, StudentT, Cauchy, Laplace, Logistic, Weibull, Lognormal, Pareto, Gumbel, Rayleigh, Triangular, Bernoulli, Binomial, Poisson, Geometric, NegativeBinomial, Hypergeometric, DiscreteUniform}
import std.random.{Rng, seed}
let rng := Rng.seeded(7)
print(Normal.sample(rng, mean = 0.0, sigma = 1.0).isFinite())
print(Uniform.sample(rng, low = 0.0, high = 1.0) >= 0.0 && Uniform.sample(rng, low = 0.0, high = 1.0) < 1.0)
print(Exponential.sample(rng, rate = 2.0) >= 0.0)
print(Gamma.sample(rng, alpha = 2.5, beta = 1.7) > 0.0)
let betaSample := Beta.sample(rng, alpha = 2.0, beta = 3.0)
print(betaSample >= 0.0 && betaSample <= 1.0)
print(ChiSquared.sample(rng, dof = 4.0) > 0.0)
print(StudentT.sample(rng, dof = 5.0).isFinite())
print(Cauchy.sample(rng, location = 0.0, scale = 1.0).isFinite())
print(Laplace.sample(rng, location = 0.0, scale = 1.0).isFinite())
print(Logistic.sample(rng, location = 0.0, scale = 1.0).isFinite())
print(Weibull.sample(rng, k = 2.0, lambda = 1.0) > 0.0)
print(Lognormal.sample(rng, mu = 0.0, sigma = 1.0) > 0.0)
print(Pareto.sample(rng, scale = 1.0, alpha = 2.0) >= 1.0)
print(Gumbel.sample(rng, location = 0.0, scale = 1.0).isFinite())
print(Rayleigh.sample(rng, sigma = 1.0) > 0.0)
let triangular := Triangular.sample(rng, low = 0.0, mode = 0.5, high = 1.0)
print(triangular >= 0.0 && triangular <= 1.0)
let bernoulli := Bernoulli.sample(rng, p = 0.5)
print(bernoulli == 0 || bernoulli == 1)
let binomial := Binomial.sample(rng, trials = 10, p = 0.3)
print(binomial >= 0 && binomial <= 10)
print(Poisson.sample(rng, lambda = 2.0) >= 0)
print(Geometric.sample(rng, p = 0.5) >= 1)
print(NegativeBinomial.sample(rng, successes = 3, p = 0.5) >= 0)
let hyper := Hypergeometric.sample(rng, population = 20, successes = 7, draws = 5)
print(hyper >= 0 && hyper <= 5)
let discreteUniform := DiscreteUniform.sample(rng, low = 1, high = 6)
print(discreteUniform >= 1 && discreteUniform <= 6)
print(Normal.sample(rng, mean = 0.0, sigma = 1.0, shape = 5).shape())
print(Normal.sample(rng, mean = 0.0, sigma = 1.0, shape = (2, 3)).shape())
print(Normal.sample(rng, mean = 0.0, sigma = 1.0, shape = Array([2, 3])).shape())
print(Normal.sample(rng, mean = 0.0, sigma = 1.0, shape = [2, 3]).shape())
print(Bernoulli.sample(rng, p = 0.5, shape = 4).count())
seed(42)
let first := Normal.sample(mean = 0.0, sigma = 1.0)
seed(42)
let second := Normal.sample(mean = 0.0, sigma = 1.0)
print(first == second)

"#,
        "true\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\n[5]\n[2, 3]\n[2, 3]\n[2, 3]\n4\ntrue\n",
    );
}

#[test]
fn probability_parameter_failures_stop_with_clear_messages() {
    for (expression, message) in [
        (
            "Beta.create(0.0, 1.0)",
            "Beta requires positive finite alpha and beta",
        ),
        ("Binomial.create(-1, 0.5)", "nonnegative integer trials"),
        ("Poisson.create(-1.0)", "nonnegative finite lambda"),
        (
            "Hypergeometric.create(10, 11, 5)",
            "0 <= successes <= population",
        ),
        ("DiscreteUniform.create(2, 1)", "integer low <= high"),
        (
            "NegativeBinomial.create(0, 0.5)",
            "positive integer successes",
        ),
        (
            "Normal.sample(mean = 0.0, sigma = -1.0)",
            "positive finite sigma",
        ),
        (
            "Normal.pdf(x = 0.0, mean = 0.0, sigma = 0.0)",
            "positive finite sigma",
        ),
    ] {
        let source = format!(
            "import std.prob.{{Normal, Beta, Binomial, Poisson, Hypergeometric, NegativeBinomial, DiscreteUniform}}\nlet result := {expression}"
        );
        let output = execute(&source);
        assert!(!output.status.success(), "{source}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{source}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn probability_densities_and_cdfs_match_reference_values() {
    prints(
        r#"
import std.prob.{Normal, Uniform, Exponential, Gamma, Beta, ChiSquared, StudentT, Cauchy, Laplace, Logistic, Weibull, Lognormal, Pareto, Gumbel, Rayleigh, Triangular, Bernoulli, Binomial, Poisson, Geometric, NegativeBinomial, Hypergeometric, DiscreteUniform}
print((Normal.pdf(x = 0.5, mean = 0.3, sigma = 1.2) - 0.3278664300849499).abs() < 1e-12)
print((Normal.cdf(x = 0.5, mean = 0.3, sigma = 1.2) - 0.5661838326109037).abs() < 1e-12)
print((Uniform.pdf(x = 0.5, low = 0.1, high = 1.7) - 0.625).abs() < 1e-12)
print((Uniform.cdf(x = 0.5, low = 0.1, high = 1.7) - 0.25000000000000006).abs() < 1e-12)
print((Exponential.pdf(x = 1.2, rate = 0.7) - 0.3021973664003558).abs() < 1e-12)
print((Exponential.cdf(x = 1.2, rate = 0.7) - 0.5682894765709203).abs() < 1e-12)
print((Gamma.pdf(x = 1.3, alpha = 2.5, beta = 1.7) - 0.4609038407599605).abs() < 1e-12)
print((Gamma.cdf(x = 1.3, alpha = 2.5, beta = 1.7) - 0.5093420213228873).abs() < 1e-12)
print((Beta.pdf(x = 0.4, alpha = 2.2, beta = 3.1) - 1.7914598226264795).abs() < 1e-12)
print((Beta.cdf(x = 0.4, alpha = 2.2, beta = 3.1) - 0.49339638807619507).abs() < 1e-12)
print((ChiSquared.pdf(x = 2.5, dof = 3.5) - 0.18426351557569048).abs() < 1e-12)
print((ChiSquared.cdf(x = 2.5, dof = 3.5) - 0.43604838018256464).abs() < 1e-12)
print((StudentT.pdf(x = 1.1, dof = 4.5) - 0.19613537620613603).abs() < 1e-12)
print((StudentT.cdf(x = 1.1, dof = 4.5) - 0.836667412367305).abs() < 1e-12)
print((Cauchy.pdf(x = 0.3, location = 0.1, scale = 1.3) - 0.2391924000225017).abs() < 1e-12)
print((Cauchy.cdf(x = 0.3, location = 0.1, scale = 1.3) - 0.548589790347529).abs() < 1e-12)
print((Laplace.pdf(x = 0.3, location = 0.1, scale = 1.3) - 0.3297707381386313).abs() < 1e-12)
print((Laplace.cdf(x = 0.3, location = 0.1, scale = 1.3) - 0.5712980404197794).abs() < 1e-12)
print((Logistic.pdf(x = 0.3, location = 0.1, scale = 1.3) - 0.19117425078150183).abs() < 1e-12)
print((Logistic.cdf(x = 0.3, location = 0.1, scale = 1.3) - 0.5383858565626407).abs() < 1e-12)
print((Weibull.pdf(x = 1.3, k = 1.7, lambda = 2.1) - 0.3717520400293271).abs() < 1e-12)
print((Weibull.cdf(x = 1.3, k = 1.7, lambda = 2.1) - 0.35758319447996323).abs() < 1e-12)
print((Lognormal.pdf(x = 1.3, mu = 0.2, sigma = 0.7) - 0.43666169862446724).abs() < 1e-12)
print((Lognormal.cdf(x = 1.3, mu = 0.2, sigma = 0.7) - 0.5354955254695766).abs() < 1e-12)
print((Pareto.pdf(x = 2.5, scale = 1.3, alpha = 2.2) - 0.2087805455840741).abs() < 1e-12)
print((Pareto.cdf(x = 2.5, scale = 1.3, alpha = 2.2) - 0.7627493800180976).abs() < 1e-12)
print((Gumbel.pdf(x = 0.3, location = 0.1, scale = 1.3) - 0.27981843362880265).abs() < 1e-12)
print((Gumbel.cdf(x = 0.3, location = 0.1, scale = 1.3) - 0.42426207250561254).abs() < 1e-12)
print((Rayleigh.pdf(x = 1.3, sigma = 1.1) - 0.5344039835289199).abs() < 1e-12)
print((Rayleigh.cdf(x = 1.3, sigma = 1.1) - 0.5025932153307745).abs() < 1e-12)
print((Triangular.pdf(x = 0.4, low = 0.1, mode = 0.3, high = 1.2) - 1.6161616161616164).abs() < 1e-12)
print((Triangular.cdf(x = 0.4, low = 0.1, mode = 0.3, high = 1.2) - 0.3535353535353535).abs() < 1e-12)
print((Bernoulli.pmf(k = 1, p = 0.3) - 0.3).abs() < 1e-12)
print((Bernoulli.cdf(k = 0, p = 0.3) - 0.7).abs() < 1e-12)
print((Binomial.pmf(k = 3, trials = 10, p = 0.3) - 0.2668279320000006).abs() < 1e-12)
print((Binomial.cdf(k = 3, trials = 10, p = 0.3) - 0.6496107184000001).abs() < 1e-12)
print((Poisson.pmf(k = 2, lambda = 2.4) - 0.26126770547350797).abs() < 1e-12)
print((Poisson.cdf(k = 2, lambda = 2.4) - 0.5697087466575107).abs() < 1e-12)
print((Geometric.pmf(k = 3, p = 0.4) - 0.144).abs() < 1e-12)
print((Geometric.cdf(k = 3, p = 0.4) - 0.784).abs() < 1e-12)
print((NegativeBinomial.pmf(k = 3, successes = 4, p = 0.5) - 0.15624999999999994).abs() < 1e-12)
print((NegativeBinomial.cdf(k = 3, successes = 4, p = 0.5) - 0.5000000000000009).abs() < 1e-12)
print((Hypergeometric.pmf(k = 2, population = 20, successes = 7, draws = 5) - 0.3873839009287907).abs() < 1e-12)
print((Hypergeometric.cdf(k = 2, population = 20, successes = 7, draws = 5) - 0.7932146542827616).abs() < 1e-12)
print((DiscreteUniform.pmf(k = 3, low = 1, high = 6) - 0.16666666666666669).abs() < 1e-12)
print((DiscreteUniform.cdf(k = 3, low = 1, high = 6) - 0.5).abs() < 1e-12)
"#,
        "true\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\n",
    );
}

#[test]
fn numerical_distribution_tails_and_seeded_integer_bounds() {
    prints(
        r#"
import std.random.{Rng}
import std.prob.{Normal, Uniform}
let normal := Normal.create(0.0, 1.0)
print((normal.logPdf(0.0) + 0.9189385332046727).abs() < 1e-15)
print(normal.cdf(-40.0))
print(normal.cdf(40.0))
let uniform := Uniform.create(-1e308, 1e308)
print(uniform.cdf(0.0))
print(uniform.pdf(0.0) > 0.0)
let fixed := Rng.seeded(42)
print(fixed.integers(0, 100))
print(fixed.integers(0, 100))
print(fixed.uniform(0.0, 1.0))
print(fixed.normal(0.0, 1.0))
let rng := Rng.seeded(9)
let mut inside := true
for (i in range(2000)) {
    let value := rng.integers(-3, 4)
    if value < -3 || value >= 4 { inside = false }
}
print(inside)
print(rng.uniform(-2.0, 2.0) < 2.0)
"#,
        "true\n0.0\n1.0\n0.5\ntrue\n5\n15\n0.95970718129751265\n-1.7692427553385659\ntrue\ntrue\n",
    );
}

#[test]
fn numerical_failures_stop_with_clear_runtime_messages() {
    for (expression, message) in [
        ("linalg.inverse(Tensor.ones((2, 2)))", "Matrix is singular"),
        (
            "linalg.solve(Tensor.zeros((2, 2)), Tensor.ones((2,)))",
            "Matrix is singular",
        ),
        (
            "linalg.cholesky(Tensor([1.0, 2.0, 2.0, 1.0], shape = (2, 2)))",
            "not positive definite",
        ),
        (
            "linalg.eigh(Tensor([1.0, 2.0, 0.0, 1.0], shape = (2, 2)))",
            "symmetric matrix",
        ),
        (
            "linalg.inverse(Tensor([1.0 / 0.0], shape = (1, 1)))",
            "finite matrix elements",
        ),
        (
            "linalg.pinv(Tensor.ones((2, 2)), rcond = -1.0)",
            "rcond must be finite",
        ),
        (
            "stats.variance(Tensor.ones((2,)), ddof = 2)",
            "ddof must be nonnegative",
        ),
        (
            "stats.correlation(Tensor.ones((2, 2)))",
            "nonzero column variances",
        ),
        ("Normal.create(0.0, -1.0)", "positive finite sigma"),
        ("Uniform.create(2.0, 1.0)", "low < high"),
        ("Rng.seeded(42).integers(1, 1)", "low < high"),
    ] {
        let source = format!(
            "import std.linalg\nimport std.stats\nimport std.random.{{Rng}}\nimport std.prob.{{Normal, Uniform}}\nlet result := {expression}"
        );
        let output = execute(&source);
        assert!(!output.status.success(), "{source}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{source}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn numerical_zero_shapes_rank_deficiency_and_seeded_samples() {
    prints(
        r#"
import std.linalg
import std.stats
import std.random.{Rng}
import std.prob.{Normal, Uniform}
print(Tensor.ones((3, 0)) @ Tensor.ones((0,)))
print(Tensor.ones((0,)) @ Tensor.ones((0, 2)))
print(Tensor([]).norm())
print(Tensor([1e308, 1e308]).norm().isFinite())
print(Tensor([1e-308, 1e-308]).norm() > 0.0)
let A := Tensor([1.0, 2.0, 2.0, 4.0], shape = (2, 2))
print(linalg.rank(A))
print((A @ linalg.pinv(A) @ A - A).norm() < 1e-12)
let wide := Tensor([1.0, 0.0, 1.0, 0.0, 1.0, 1.0], shape = (2, 3))
let qr := linalg.qr(wide)
print((qr.q @ qr.r - wide).norm() < 1e-12)
print(linalg.svd(Tensor.zeros((0, 3))).s.shape())
print(linalg.pinv(Tensor.zeros((0, 3))).shape())
print(linalg.det(Tensor.zeros((0, 0))))
let rng := Rng.seeded(123)
let normal := Normal.create(0.0, 1.0)
let samples := Tensor.generate((4096,), I => normal.sample(rng))
print(samples.mean().abs() < 0.1)
print((stats.variance(samples) - 1.0).abs() < 0.15)
let uniform := Uniform.create(-1e308, 1e308)
print(uniform.cdf(0.0))
print(uniform.pdf(0.0) > 0.0)
print(uniform.sample(rng).isFinite())
"#,
        "[0.0, 0.0, 0.0]\n[0.0, 0.0]\n0.0\ntrue\ntrue\n1\ntrue\ntrue\n[0]\n[3, 0]\n1.0\ntrue\ntrue\n0.5\ntrue\ntrue\n",
    );
}

#[test]
fn math_example_runs_sets_functions_numbers_and_finite_algebra() {
    prints_from_file(
        include_str!("../testdata/language/math.kes"),
        "8\n7\nfixed\ntrue\ntrue\ntrue\ntrue\n3\n5\n6\n3.1415926535897931\n3.0\n0\n1\n1\n0\n",
    );
}

#[test]
fn complex_example_runs_with_the_imported_class() {
    prints_from_file(
        include_str!("../testdata/language/complex.kes"),
        "4.0+6.0i\n-5.0+10.0i\n2.2000000000000002-0.40000000000000002i\n5.0\n0.92729521800161219\n3.0-4.0i\n25.0\ntrue\n3.0+4.0i\n0.0+0.0i\n1.0+0.0i\n",
    );
}

#[test]
fn complex_class_operators_methods_and_static_constructors_execute() {
    prints_from_file(
        r#"
import std.complex.{Complex}
let z := Complex(3.0, 4.0)
let w := Complex(1.0, -2.0)
print(z + w)
print(z - w)
print(z * w)
print(z / w)
print(z + 2.0)
print(2.0 + z)
print(z - 2.0)
print(2.0 - z)
print(z * 2.0)
print(2.0 * z)
print(z / 2.0)
print(2.0 / z)
print(z.abs())
print(z.arg())
print(z.conjugate())
print(z.normSquared())
print(z.isFinite())
print(z.copy())
print(z.negate())
print(-z)
print(z ** 2)
print(z ** -1)
print(z ** 0)
print(z == Complex(3.0, 4.0))
print(z == w)
print(z.text())
print(z.re)
print(z.im)
print(Complex.zero())
print(Complex.one())
print(3.0 + 4.0 * Complex.i())
print(Complex(0.0, 0.0).abs())
print(Complex(1e308, 1e308) / Complex(1e308, 1e308))
print(Complex(1e-308, 1e-308) / Complex(1e-308, 1e-308))
print(Complex(1e308, 1e308).abs().isFinite())
print(Complex(1e308, 1e308).abs() > 1e308)
print(Complex(1.0, 0.0).arg())
print(Complex(-1.0, 0.0).arg())
print(Complex(0.0, 1.0).arg())
print(Complex(0.0, -1.0).arg())
print(Complex(1.0 / 0.0, 0.0).abs())
"#,
        "4.0+2.0i\n2.0+6.0i\n11.0-2.0i\n-1.0+2.0i\n5.0+4.0i\n5.0+4.0i\n1.0+4.0i\n-1.0-4.0i\n6.0+8.0i\n6.0+8.0i\n1.5+2.0i\n0.23999999999999999-0.32000000000000001i\n5.0\n0.92729521800161219\n3.0-4.0i\n25.0\ntrue\n3.0+4.0i\n-3.0-4.0i\n-3.0-4.0i\n-7.0+24.0i\n0.12-0.16i\n1.0+0.0i\ntrue\nfalse\n3.0+4.0i\n3.0\n4.0\n0.0+0.0i\n1.0+0.0i\n3.0+4.0i\n0.0\n1.0+0.0i\n1.0+0.0i\ntrue\ntrue\n0.0\n3.1415926535897931\n1.5707963267948966\n-1.5707963267948966\ninf\n",
    );
}

#[test]
fn power_example_uses_matrix_power_elementwise_power_and_negation() {
    prints(
        include_str!("../testdata/language/power.kes"),
        "[[1.0, 0.0],\n [0.0, 1.0]]\n[[7.0, 10.0],\n [15.0, 22.0]]\n[[1.0, 4.0],\n [9.0, 16.0]]\n[[-1.0, -2.0],\n [-3.0, -4.0]]\n[1.0, 4.0, 9.0]\n-3\n9\n8\n",
    );
}

#[test]
fn generic_negatable_and_powerable_work_for_numbers_and_tensors() {
    prints(
        r#"
function negate[T : Negatable](x : T) : T = -x
function squared[T : Powerable[Int]](x : T) : T = x ** 2
print(negate(3))
print(negate(2.5))
let A := Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2))
print(negate(A))
print(squared(3))
print(squared(A))
class MyBox {
    public final x : Int
    new(x : Int) { this.x = x }
    has Negatable, Powerable, RightPowerable
    public function negate() : MyBox = MyBox(-this.x)
    public function power(exponent : Int) : MyBox = MyBox(this.x ** exponent)
    public function rightPower(left : Int) : MyBox = MyBox(left ** this.x)
}
let b := MyBox(3)
print(negate(b).x)
print(squared(b).x)
print((2 ** b).x)
"#,
        "-3\n-2.5\n[[-1.0, -2.0],\n [-3.0, -4.0]]\n9\n[[7.0, 10.0],\n [15.0, 22.0]]\n-3\n9\n8\n",
    );
}

#[test]
fn machine_learning_example_fits_scales_predicts_and_scores() {
    prints_from_file(
        include_str!("../testdata/language/ml.kes"),
        "[3.0, 3.0]\n[1.5811388300841898, 1.8708286933869707]\n[[0.0, 0.20000000000000001],\n [0.25, 0.0],\n [0.75, 0.40000000000000002],\n [1.0, 1.0]]\n[[2.0, 3.0, 4.0, 9.0, 6.0]]\ntrue\ntrue\ntrue\ntrue\n[3, 2]\n[1, 2]\n[2, 3]\n[0, 1]\n[0.0, 1.0, 1.0, 1.0]\n1.0\n[0.0, 1.0, 1.0, 1.0]\n1.0\n[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]\n[4, 1]\ntrue\n(1.0, 1.0, 1.0, 1.0)\n0.5\n0.5\n0.5\n",
    );
}

#[test]
fn integer_literals_execute_as_double_in_double_contexts() {
    prints(
        r#"
let a : double = 1
print(a)
print(1 + 2.0)
print(1.0 + 2)
print(1 / 2.0)
print(1e3 + 1)
print(1 == 1.0)
print(1 < 2.0)
function f(x : double) : double = x
print(f(3))
print(-1 + 2.0)
print(0x10 + 0.5)
print(1 / 2)
let b : double = 1 + 2
print(b)
print(9007199254740992.0 == 9007199254740992)

"#,
        "1.0\n3.0\n3.0\n0.5\n1001.0\ntrue\ntrue\n3.0\n1.0\n16.5\n0\n3.0\ntrue\n",
    );
}

#[test]
fn transpose_example_shows_read_only_view_and_owned_copy() {
    prints(
        include_str!("../testdata/language/transpose.kes"),
        "[[1.0, 4.0],\n [2.0, 5.0],\n [3.0, 6.0]]\n[3, 2]\n4.0\n[9.0, 12.0, 15.0]\n21.0\n[[1.0, 2.0, 3.0],\n [4.0, 5.0, 6.0]]\n[[1.0, 4.0],\n [2.0, 5.0],\n [3.0, 6.0]]\n",
    );
}

#[test]
fn tensor_transpose_property_is_a_zero_copy_read_only_view() {
    prints(
        r#"
let A := Tensor([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], shape = (2, 3))
print(A.T)
print(A.T.shape())
print(A.T[0, 1])
print(A.T[1, 0])
print(A.T @ Tensor([1.0, 2.0], shape = (2,)))
print(A.T.copy())
print(A.T.T)
print(A.T.T.shape())
print(A.T + A.T)
print(A.T * 2.0)
print(-A.T)
print(A.T.sum())
print(A.T.sum(axis = 0))
print(A.T.sum(axis = 1))
print(A.T.transpose())
print(A.T.reshape((6,)))
print(A.T.broadcastTo((2, 3, 2)))
print(A.T.map(x => x + 1.0))
let T := A.T
print(T)
print(T[0, 1])
let D := Tensor.ones((2, 3)).T
print(D)

"#,
        "[[1.0, 4.0],\n [2.0, 5.0],\n [3.0, 6.0]]\n[3, 2]\n4.0\n2.0\n[9.0, 12.0, 15.0]\n[[1.0, 4.0],\n [2.0, 5.0],\n [3.0, 6.0]]\n[[1.0, 2.0, 3.0],\n [4.0, 5.0, 6.0]]\n[2, 3]\n[[2.0, 8.0],\n [4.0, 10.0],\n [6.0, 12.0]]\n[[2.0, 8.0],\n [4.0, 10.0],\n [6.0, 12.0]]\n[[-1.0, -4.0],\n [-2.0, -5.0],\n [-3.0, -6.0]]\n21.0\n[6.0, 15.0]\n[5.0, 7.0, 9.0]\n[[1.0, 2.0, 3.0],\n [4.0, 5.0, 6.0]]\n[1.0, 4.0, 2.0, 5.0, 3.0, 6.0]\n[[[1.0, 4.0],\n  [2.0, 5.0],\n  [3.0, 6.0]],\n [[1.0, 4.0],\n  [2.0, 5.0],\n  [3.0, 6.0]]]\n[[2.0, 5.0],\n [3.0, 6.0],\n [4.0, 7.0]]\n[[1.0, 4.0],\n [2.0, 5.0],\n [3.0, 6.0]]\n4.0\n[[1.0, 1.0],\n [1.0, 1.0],\n [1.0, 1.0]]\n",
    );
}

#[test]
fn numerical_modules_materialize_transpose_views() {
    prints(
        r#"
import std.math
import std.stats
import std.linalg
let M := Tensor([1.0, 2.0, 3.0, 5.0], shape = (2, 2))
print(math.exp(M.T))
print(stats.mean(M.T))
print(linalg.inverse(M.T) @ M.T)
print(linalg.norm(M.T))

"#,
        "[[2.7182818284590451, 20.085536923187668],\n [7.3890560989306504, 148.4131591025766]]\n2.75\n[[1.0, 0.0],\n [0.0, 1.0]]\n6.2449979983983983\n",
    );
}

#[test]
fn tensor_matrix_power_rejects_negative_exponents() {
    let output = execute("let A := Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2))\nlet B := A ** -1");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("non-negative exponent"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn list_at_reads_one_item_without_materializing_an_array() {
    let output = execute(
        "let xs := [10, 20, 30]\nprint(xs.at(1))\nlet ss := [\"a\", \"bc\"]\nprint(ss.at(1))\nlet bs := [true, false]\nprint(bs.at(0))\n",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"20\nbc\ntrue\n");
}

/// A method on a copyable record receives the caller's copy and must release it,
/// in tail and ordinary position; KESTREL_LEAK_CHECK=1 makes `leaks` confirm it.
#[test]
fn record_methods_release_their_copied_receivers() {
    prints(
        r#"
trait Named {
    function name() : String
}
record Inner(tag : String, items : List[String])
record R(label : String, items : List[String], inner : Inner) {
    has Named
    public function name() : String = this.label
    public function me() : R = this
    public function first() : String = this.items.at(0)
    public function all() : List[String] = this.items
    public function nested() : Inner = this.inner
    public function tagLength() : Int = this.inner.tag.length()
    public function twice() : String = this.name() + this.me().name()
    public function relabel(s : String) : R = R(s, this.items, this.inner)
    public function count(extra : List[String]) : Int = (this.items + extra).length()
    public function maybe(i : Int) : Option[String] = if i < 0 then None else Some(this.items.slice(i, i + 1).join(""))
}
function describe(n : &dyn Named) : String = n.name()
function useGeneric[T : Named](t : T) : String = t.name()
function build(i : Int) : R = R(f"r{i}", [f"a{i}", "b"], Inner(f"t{i}", ["x"]))
function loop(n : Int, acc : Int) : Int {
    if n == 0 { return acc }
    let r := build(n)
    return loop(n - 1, acc + r.relabel("z").me().all().length())
}
let mut total := 0
let mut text := ""
for (i in range(200)) {
    let r := build(i)
    total += r.first().length() + r.all().length() + r.nested().items.length() + r.tagLength()
    text = r.twice() + r.me().me().name()
    total += r.relabel(f"x{i}").name().length()
    total += r.count(["c"])
    total += match r.maybe(1) { Some(s) => s.length(), None => 0 }
    total += describe(r).length() + useGeneric(r).length()
    total += build(i).me().first().length()
    let held := r.me()
    let rs := [held, build(i + 1)]
    total += rs.map(x => x.name().length()).sum()
    let f := r.name
    total += f().length()
}
print(total)
print(text)
print(loop(1000, 0))
let keep := build(7).me()
print(keep)
"#,
        "7612\nr199r199r199\n2000\nR(label = r7, items = [a7, b], inner = Inner(tag = t7, items = [x]))\n",
    );
}

/// A copyable record is held by value, so a `&dyn` view points at its storage,
/// including a temporary, a field and a boxed list item.
#[test]
fn copyable_records_become_trait_object_views() {
    prints(
        r#"
trait Named {
    function name() : String
}
record R(label : String, items : List[String]) {
    has Named
    public function name() : String = this.label + f"{this.items.length()}"
}
record Holder(inner : R)
public class Dog {
    has Named
    public final n : String
    new(n : String) { this.n = n }
    public function name() : String = this.n
}
function describe(n : &dyn Named) : String = n.name()
function build(i : Int) : R = R(f"r{i}", [f"a{i}"])
let mut total := 0
for (i in range(300)) {
    let r := build(i)
    total += describe(r).length()
    total += describe(build(i)).length()
    total += describe(R("tmp", ["x", "y"])).length()
    let h := Holder(r)
    total += describe(h.inner).length()
    let d := Dog("rex")
    total += describe(d).length()
    let boxes : List[Box[dyn Named]] = [build(i), Dog("a"), R("q", [])]
    for (b in boxes) { total += b.name().length() }
}
print(total)
print(describe(R("end", ["1", "2"])))
"#,
        "8560\nend2\n",
    );
}

/// `f(a)(b)` calls the function value that `f(a)` returns.
#[test]
fn a_call_result_can_be_called_directly() {
    prints(
        r#"
function makeAdder(k : Int) : (Int) -> Int = move x => x + k
function curry(a : Int) : (Int) -> (Int) -> Int = move b => move c => a + b + c
let r := makeAdder(5)(10)
print(r)
print(curry(1)(2)(3))
let total := makeAdder(1)(makeAdder(2)(3))
print(total)
let fs := [makeAdder(1), makeAdder(2)]
print(fs.map(f => f(10)).toList())
print((makeAdder(7))(1) * 2)
function pick(flag : Bool) : (String) -> String = if flag then move s => s.upper() else move s => s + "!"
print(pick(true)("hi") + pick(false)("ho"))
let s : String = pick(true)("typed")
print(s)
"#,
        "15\n6\n6\n[11, 12]\n16\nHIho!\nTYPED\n",
    );
}

/// A recursive enum keeps its nodes in counted boxes (Q169). A function that views a child
/// adds no reference, a temporary moves into the node built from it, and a shared subtree
/// stays valid for every holder; KESTREL_LEAK_CHECK=1 makes `leaks` confirm each node is freed.
#[test]
fn recursive_enum_nodes_are_viewed_moved_and_freed() {
    prints(
        r#"
enum Tree {
    Leaf
    Node(Tree, Tree)
}
record Pair(left : Tree, right : Tree)
function make(depth : Int) : Tree =
    if depth == 0 then Node(Leaf, Leaf) else Node(make(depth - 1), make(depth - 1))
function check(t : Tree) : Int = match t {
    Leaf => 0,
    Node(l, r) => 1 + check(l) + check(r),
}
function twice(t : Tree) : Tree = Node(t, t)
function same(t : Tree) : Tree = t
function pick(flag : Bool, a : Tree, b : Tree) : Tree {
    let chosen := if flag then a else b
    chosen
}
function grow(n : Int) : Tree {
    let mut acc := Leaf
    for (i in range(n)) {
        acc = Node(acc, make(1))
    }
    acc
}
function swap(p : Pair) : Pair = Pair(p.right, p.left)

let t := make(3)
let pair := Node(t, t)
print(check(pair))
print(check(t))
print(check(twice(make(2))))
print(check(same(same(t))))
print(check(pick(true, t, Leaf)) + check(pick(false, t, Leaf)))
print(check(grow(5)))
let base := make(2)
let mut total := 0
for (i in range(3)) {
    total = total + check(Node(base, base)) + check(twice(base))
}
print(total)
print(check(base))
let p := swap(Pair(make(1), make(2)))
print(check(p.left) * 100 + check(p.right))
let trees := [make(1), t, twice(t)]
print(trees.map(x => check(x)).sum())
print(t == make(3))
print(Node(Leaf, Leaf))
"#,
        "31\n15\n15\n15\n15\n20\n90\n7\n703\n49\ntrue\nNode(Leaf, Leaf)\n",
    );
}

#[test]
fn trait_defaults_on_records_release_copied_receivers() {
    prints(
        "trait Named { public function value() : String\npublic function label() : String = this.value()\n}\nrecord Label(payload : String) { has Named\npublic function value() : String = this.payload\n}\nlet label := Label(\"bird\")\nlet mut total := 0\nfor (i in range(1000)) { total += label.label().length() }\nprint(total)\nlet view : &dyn Named = label\nprint(view.label())\n",
        "4000\nbird\n",
    );
}
