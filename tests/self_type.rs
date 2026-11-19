//! A `Self` type inside traits and classes.
use kestrel::{
    checker::{Checked, check},
    core,
    modules::{ProjectCheck, check_source},
    native::{self, TemporaryDirectory},
    parse_file,
    types::Type,
};
use std::process::Command;

fn checked(source: &str) -> Checked {
    check(&parse_file("self_type.kes", source))
}
fn good(source: &str) -> Checked {
    let result = checked(source);
    assert!(result.is_ok(), "{source}\n{:#?}", result.diagnostics);
    result
}
fn refuses(source: &str, code: &str) {
    let result = checked(source);
    assert!(
        result.diagnostics.iter().any(|d| d.code == code),
        "Expected {code}: {source}\n{:#?}",
        result.diagnostics
    );
}
fn output(source: &str) -> String {
    let project = ProjectCheck {
        modules: vec![check_source("self_type.kes", source)],
    };
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let program = core::lower(&project).unwrap_or_else(|errors| panic!("{errors:#?}\n{source}"));
    let dir = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let binary = dir.path().join("program");
    native::build(&program, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

const BUILDER: &str = "\
trait Builder {
    function reset() : Self
    function named(name : String) : Self
}
class Query {
    has Builder
    public final text : String
    new() { this.text = \"\" }
    new(text : String) { this.text = text }
    @override public function reset() : Query = Query()
    @override public function named(name : String) : Query = Query(name)
}
";

#[test]
fn the_example_checks_and_runs() {
    let example = include_str!("../testdata/language/selfintraits.kes");
    good(example);
    assert_eq!(output(example), "hello\nx\nx\nd\n1\nab\n");
}

#[test]
fn a_fluent_chain_on_a_class_keeps_the_class() {
    let result = good(&format!(
        "{BUILDER}let q := Query().reset().named(\"hi\")\nprint(q.text)"
    ));
    assert!(
        result
            .expression_types
            .iter()
            .any(|(_, ty)| *ty == Type::named("Query"))
    );
    assert_eq!(
        output(&format!(
            "{BUILDER}let q := Query().reset().named(\"hi\")\nprint(q.text)"
        )),
        "hi\n"
    );
}

#[test]
fn a_generic_bound_binds_self_to_the_type_parameter() {
    let source = format!(
        "{BUILDER}function twice[T : Builder](b : T) : T = b.reset().named(\"x\")\nprint(twice(Query()).text)"
    );
    good(&source);
    assert_eq!(output(&source), "x\n");
    // The chain really has type T: a Query result is not accepted for it.
    let wrong = format!("{BUILDER}function wrong[T : Builder](b : T) : Query = b.reset()");
    refuses(&wrong, "E_TYPE");
    let message = checked(&wrong)
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(message.contains("found T"), "{message}");
}

#[test]
fn two_classes_each_return_their_own_class() {
    let source = format!(
        "{BUILDER}\
class Form {{
    has Builder
    public final label : String
    new(label : String) {{ this.label = label }}
    @override public function reset() : Form = Form(\"\")
    @override public function named(name : String) : Form = Form(name)
}}
print(Form(\"a\").reset().named(\"b\").label)
print(Query().named(\"c\").text)"
    );
    good(&source);
    assert_eq!(output(&source), "b\nc\n");
}

#[test]
fn a_trait_default_body_may_return_this_for_self() {
    let source = "\
trait Copyable {
    public function copy() : Self = this
}
class Counter {
    has Copyable
    public final hits : Int
    new(hits : Int) { this.hits = hits }
}
print(Counter(3).copy().hits)
";
    good(source);
    assert_eq!(output(source), "3\n");
}

#[test]
fn self_in_a_parameter_resolves_to_the_implementing_class() {
    let source = "\
trait Combiner {
    function merge(other : Self) : Self
}
class Acc {
    has Combiner
    public final n : Int
    new(n : Int) { this.n = n }
    @override public function merge(other : Acc) : Acc = Acc(this.n + other.n)
}
print(Acc(2).merge(Acc(3)).n)
";
    good(source);
    assert_eq!(output(source), "5\n");
}

#[test]
fn a_subclass_override_may_return_the_subclass() {
    let source = format!(
        "{BUILDER}\
class SubQuery extends Query {{
    new() {{ super() }}
    new(text : String) {{ super(text) }}
    @override public function reset() : SubQuery = SubQuery()
    @override public function named(name : String) : SubQuery = SubQuery(name)
}}
print(SubQuery(\"a\").reset().named(\"b\").text)"
    );
    good(&source);
    assert_eq!(output(&source), "b\n");

    // A class method written `Self` allows the same narrowing.
    good(
        "class Figure {\n\
         public final n : Int\n\
         new(n : Int) { this.n = n }\n\
         public function copy() : Self = Figure(this.n)\n\
         }\n\
         class Square extends Figure {\n\
         new(n : Int) { super(n) }\n\
         @override public function copy() : Square = Square(this.n)\n\
         }\n\
         print(Square(2).copy().n)",
    );
}

#[test]
fn an_override_may_not_widen_or_retype_a_self_result() {
    refuses(
        "class Figure {\n\
         public final n : Int\n\
         new(n : Int) { this.n = n }\n\
         public function copy() : Self = Figure(this.n)\n\
         }\n\
         class Square extends Figure {\n\
         new(n : Int) { super(n) }\n\
         @override public function copy() : Int = 1\n\
         }",
        "E_OVERRIDE_RETURN",
    );
}

#[test]
fn a_parent_result_where_the_trait_says_self_is_refused() {
    refuses(
        "trait Builder { function reset() : Self }\n\
         class Base {\n\
         public final n : Int\n\
         new(n : Int) { this.n = n }\n\
         }\n\
         class Child extends Base {\n\
         has Builder\n\
         new(n : Int) { super(n) }\n\
         @override public function reset() : Base = Base(this.n)\n\
         }",
        "E_TRAIT_METHOD",
    );
}

#[test]
fn a_dyn_view_refuses_a_self_method() {
    refuses(
        "trait Builder { function reset() : Self }\n\
         class Query {\n\
         has Builder\n\
         new() {}\n\
         public function reset() : Query = Query()\n\
         }\n\
         function use(b : &dyn Builder) : Int = 1",
        "E_DYN_OBJECT_SAFETY",
    );
    refuses(
        "trait Builder { function reset() : Self }\n\
         class Query {\n\
         has Builder\n\
         new() {}\n\
         public function reset() : Query = Query()\n\
         }\n\
         let views : List[Box[dyn Builder]] = []",
        "E_DYN_OBJECT_SAFETY",
    );
    // A trait whose methods have no Self stays object safe.
    good(
        "trait Shape { function area() : double }\n\
         class Square {\n\
         has Shape\n\
         new() {}\n\
         public function area() : double = 1.0\n\
         }\n\
         let views : List[&dyn Shape] = []",
    );
}

#[test]
fn self_outside_a_trait_or_class_is_refused() {
    refuses("function free() : Self = 1", "E_SELF_OUTSIDE");
    refuses("let x : Self = 1", "E_SELF_OUTSIDE");
    refuses("function merge(other : Self) : Int = 1", "E_SELF_OUTSIDE");
}

#[test]
fn self_names_the_class_in_a_field_and_a_body_annotation() {
    let source = "\
class Node {
    public final label : String
    public final next : Option[Self]
    new(label : String) { this.label = label; this.next = None }
    public function self() : Self {
        let me : Self = this
        this.transform(me)
    }
    public function transform(n : Self) : Self = n
}
print(Node(\"a\").self().label)
print(Node(\"a\").next.isSome())
";
    good(source);
    assert_eq!(output(source), "a\nfalse\n");
}
