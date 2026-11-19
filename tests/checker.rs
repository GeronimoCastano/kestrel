use kestrel::{
    checker::{Checked, check},
    parse_file,
    types::Type,
};

fn checked(source: &str) -> Checked {
    check(&parse_file("test.kes", source))
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

#[test]
fn standard_error_exists_without_a_local_enum() {
    good(
        "function parse(s : String) : Int or Error = parseInt(s)\nfunction pass(error : Error) : Int or Error = error",
    );
    refuses("enum Error { Parse(String) }", "E_ERROR_ROOT_REDEFINITION");
}

#[test]
fn named_function_type_inputs_preserve_labels_modes_and_old_type_grouping() {
    good(
        "trait Apply {function run(f : (left : Int) -> Int) : Int}\nclass Base {public function run(f : (left : Int) -> Int) : Int=f(left=1)}\nclass Impl extends Base {has Apply\n@override public function run(f : (right : Int) -> Int) : Int=f(right=2)}",
    );
    good(
        "trait Source {function next() : (left : Int) -> Int}\nclass Impl {has Source\npublic function next() : (right : Int) -> Int=(actual : Int) => actual}",
    );
    good(include_str!("../testdata/language/functioninputs.kes"));
    good(
        "alias MutableCallback = mut (() -> Int)\nalias BorrowCallback = (MutableCallback) -> Int\nalias OldGrouping = ((Int,Int)) -> Int\nalias Mixed = (Int, right : Int) -> Int",
    );
    refuses("alias Bad = (x : Int)", "E_FUNCTION_TYPE_PARAMETER");
    refuses("alias Bad = (x : Int,x : Int) -> Int", "E_DUPLICATE_NAME");
    good("alias Named = (Upper_case : Int) -> Int");
}

#[test]
fn collection_lengths_and_defaults_are_read_only_and_pure() {
    good(include_str!("../testdata/language/collectioncounts.kes"));
    good(
        "function list[T](values : List[T]) : Int=values.length()\nfunction map[K,V](values : Map[K,V]) : Int=values.length()\nfunction sets[T](values : Set[T]) : Int=values.length()\nfunction mutable(values : MutMap[Int,Int]) : Int=values.length()\nfunction sorted[T](values : SortedSet[T]) : Int=values.length()",
    );
}

#[test]
fn a_trait_names_other_traits_in_has() {
    good(
        "trait Parent { function p() : Int }\ntrait Child { has Parent\nfunction c() : Int }\nclass Both { has Child\nnew() {}\npublic function p() : Int = 1\npublic function c() : Int = 2 }",
    );
    good(
        "trait Expr { has Text\nfunction eval() : Int }\nclass Val { has Expr\npublic final value : Int\nnew(value : Int) { this.value = value }\npublic function eval() : Int = this.value\npublic function text() : String = f\"{this.value}\" }\nfunction show[T : Text](x : T) : String with Effects = x.text()\nlet v := Val(3)\nprint(show(v))",
    );
    refuses(
        "trait Expr { has Text\nfunction eval() : Int }\nclass Bad { has Expr\npublic final value : Int\nnew(value : Int) { this.value = value }\npublic function eval() : Int = this.value }",
        "E_TRAIT_METHOD",
    );
}

#[test]
fn traits_declare_methods_and_associated_types_instead_of_fields() {
    refuses(
        "trait T { var x : Int\nfunction f() : Int }",
        "E_TRAIT_FIELD",
    );
    refuses("trait T { final x : Int }", "E_TRAIT_FIELD");
    good("trait T { type Item\nfunction f() : Item }");
}

#[test]
fn constructor_bodies_produce_no_value() {
    refuses("class C { final x : Int\nnew() {this.x=1\n42} }", "E_TYPE");
    good("class C { final x : Int\nnew() {this.x=1} }");
}

#[test]
fn constructors_do_not_write_return_types() {
    refuses(
        "class C { final x : Int\nnew() : Int {this.x=1} }",
        "E_CONSTRUCTOR_RETURN",
    );
    good("class C { final x : Int\nnew() {this.x=1} }");
}

#[test]
fn static_final_fields_are_lazy_immutable_type_members() {
    good(
        "class C { public static final count : Int = 2 + 3\npublic static final text : String = \"a\" + \"b\"\npublic static final items : List[Int] = [1, 2]\nnew() {} }\nprint(C.count)\nprint(C.text)\nC.items.length()",
    );
    good(
        "class C { public static final base : Int = 10\npublic static final doubled : Int = C.base * 2\nnew() {} }\nC.doubled",
    );
    good(
        "class C { private static final secret : Int = 1\nnew() {}\npublic static function read() : Int = C.secret }\nC.read()",
    );
    refuses(
        "class C { public static final value : MutMap[String, Int] = MutMap()\nnew() {} }",
        "E_STATIC_TYPE",
    );
    refuses(
        "class C { public static final value : Int\nnew() {} }",
        "E_STATIC_INIT",
    );
    refuses(
        "class C[T] { public static final value : Int = 1\nnew() {} }",
        "E_STATIC_GENERIC",
    );
    refuses(
        "class C { public static final count : Int = 1\nnew() {} }\nC.count = 2",
        "E_STATIC_ASSIGNMENT",
    );
    refuses(
        "class C { public static final count : Int = 1\nnew() {}\npublic static function count() : Int = 2 }",
        "E_DUPLICATE_NAME",
    );
    refuses(
        "class C { private static final secret : Int = 1\nnew() {} }\nC.secret",
        "E_MEMBER_PRIVATE",
    );
    refuses(
        "let mut n := 1\nclass C { public static final value : Int = n\nnew() {} }\nC.value",
        "E_CONST_DEPENDENCY",
    );
}

#[test]
fn constants_hold_stable_immutable_values() {
    refuses(
        "const M : MutMap[String, Int] = MutMap()\nM.put(\"a\", 1)",
        "E_CONST_TYPE",
    );
    refuses("const A : Array[Int] = Array(2, 0)", "E_CONST_TYPE");
    refuses("let mut x := 1\nconst N : Int = x", "E_CONST_DEPENDENCY");
    refuses(
        "class P { public var x : Int\nnew() {this.x=1} }\nlet p := P()\nconst N : Int = p.x",
        "E_CONST_DEPENDENCY",
    );
    good("let x := 1\nconst N : Int = x");
    good("const XS : List[Int] = [1, 2, 3]");
    good("const N : Int = 1 + 2");
    good("function f(n : Int) : Int = n + 1\nconst N : Int = f(1)");
}

#[test]
fn integer_literals_fit_their_inferred_numeric_type() {
    refuses(
        "let n : Int = 9223372036854775808",
        "E_INTEGER_LITERAL_RANGE",
    );
    refuses(
        "let n : Int = 0x10000000000000000",
        "E_INTEGER_LITERAL_RANGE",
    );
    refuses(
        "let n : Int = 1\nmatch n {100000000000000000000 => 1, _ => 0}",
        "E_INTEGER_LITERAL_RANGE",
    );
    refuses(
        "let n : Int = 1\nmatch n {-9223372036854775809 => 1, _ => 0}",
        "E_INTEGER_LITERAL_RANGE",
    );
    good("let n : Int = 1\nmatch n {-9223372036854775808 => 1, _ => 0}");
    refuses(
        "let n : Nat = 18446744073709551616",
        "E_INTEGER_LITERAL_RANGE",
    );
    good("let n : Int = 9223372036854775807");
    good("let n : Int = -9223372036854775808");
    good("let n : Int = -0x8000000000000000");
    good("let n : Nat = 18446744073709551615");
    good("let n : BigInt = 100000000000000000000");
}

#[test]
fn this_is_reserved_for_the_current_object() {
    for source in [
        "let this := 1",
        "function this() : Int = 1",
        "function f(this : Int) : Int = this",
        "class C { final this : Int\nnew() {this.this=1} }",
        "let (this, other) := (1, 2)",
    ] {
        refuses(source, "E_RESERVED_NAME");
    }
    good("class C { final x : Int\nnew() {this.x=1}\npublic function f() : Int = this.x }");
}

#[test]
fn an_override_cannot_lower_visibility() {
    refuses(
        "class B { new() {}\npublic function f() : Int = 1 }\nclass C extends B { new() {}\n@override private function f() : Int = 2 }",
        "E_OVERRIDE_VISIBILITY",
    );
    good(
        "class B { new() {}\npackage function f() : Int = 1 }\nclass C extends B { new() {}\n@override public function f() : Int = 2 }",
    );
}

#[test]
fn trait_methods_keep_their_generic_bounds() {
    refuses(
        "trait T { function f() : Int }\nclass C { has T\nnew() {}\npublic function f[U : Text]() : Int = 1 }",
        "E_TRAIT_METHOD",
    );
    refuses(
        "trait T { function f[U : Text]() : Int }\nclass C { has T\nnew() {}\npublic function f[U]() : Int = 1 }",
        "E_TRAIT_METHOD",
    );
    good(
        "trait T { function f[U : Text]() : Int }\nclass C { has T\nnew() {}\npublic function f[U : Text]() : Int = 1 }",
    );
}

#[test]
fn overrides_and_trait_methods_keep_their_static_or_instance_form() {
    refuses(
        "class B { public function f() : Int = 1 }\nclass C extends B { @override public static function f() : Int = 2 }",
        "E_OVERRIDE_STATIC",
    );
    refuses(
        "class B { public static function f() : Int = 1 }\nclass C extends B { @override public function f() : Int = 2 }",
        "E_OVERRIDE_STATIC",
    );
    refuses(
        "trait T { function f() : Int }\nclass C { has T\nnew() {}\npublic static function f() : Int = 2 }",
        "E_TRAIT_METHOD",
    );
    refuses(
        "class C { public function f() : Int = 1\npublic static function f(x : Int) : Int = x }",
        "E_STATIC_OVERLOAD",
    );
    good(
        "class B { public static function f() : Int = 1 }\nclass C extends B { @override public static function f() : Int = 2 }",
    );
    good(
        "trait T { static function f() : Int }\nclass C { has T\nnew() {}\npublic static function f() : Int = 2 }",
    );
}

#[test]
fn an_override_cannot_add_outside_effects() {
    refuses(
        "class B { new() {}\npublic function f() : Int = 1 }\nclass C extends B { new() {}\n@override public function f() : Int with Effects = 1 }",
        "E_OVERRIDE_EFFECTS",
    );
    good(
        "class B { new() {}\npublic function f() : Int with Effects = 1 }\nclass C extends B { new() {}\n@override public function f() : Int = 2 }",
    );
    good(
        "class B { new() {}\npublic function f() : Int with Effects = 1 }\nclass C extends B { new() {}\n@override public function f() : Int with Effects = 2 }",
    );
}

#[test]
fn repeated_members_and_dead_fallbacks_are_refused() {
    refuses(
        "class C { final x : Int\nnew(x : Int) {this.x=x}\nnew(x : Int) {this.x=x} }",
        "E_DUPLICATE_SIGNATURE",
    );
    refuses(
        "trait T { function f() : Int }\nclass C { has T, T\nnew() {}\npublic function f() : Int = 1 }",
        "E_DUPLICATE_TRAIT",
    );
    refuses(
        "function f() : Int { let x := 1\nlet y := x otherwise { return 0 }\ny }",
        "E_OTHERWISE_IRREFUTABLE",
    );
    good("function f(n : Int) : Int { let Some(v) := Some(n) otherwise { return 0 }\nv }");
}

#[test]
fn modifiers_and_attributes_are_refused_where_they_have_no_meaning() {
    for (source, code) in [
        ("@override function f() : Int = 1", "E_ATTRIBUTE_CONTEXT"),
        (
            "class C { public override function f() : Int = 1 }",
            "E_OVERRIDE_SYNTAX",
        ),
        ("static function f() : Int = 1", "E_STATIC_CONTEXT"),
        ("sealed function f() : Int = 1", "E_SEALED_CONTEXT"),
        ("override class C { new() {} }", "E_OVERRIDE_CONTEXT"),
        ("static class C { new() {} }", "E_STATIC_CONTEXT"),
        ("sealed enum E { A }", "E_SEALED_CONTEXT"),
        ("class C { override new() {} }", "E_OVERRIDE_CONTEXT"),
        (
            "class C { override final x : Int\nnew() {this.x=1} }",
            "E_OVERRIDE_CONTEXT",
        ),
        ("@test class C { new() {} }", "E_ATTRIBUTE_CONTEXT"),
        ("@simp function f() : Int = 1", "E_ATTRIBUTE_CONTEXT"),
        ("@frobnicate function f() : Int = 1", "E_ATTRIBUTE_UNKNOWN"),
        (
            "@test @test function check() : void {}",
            "E_ATTRIBUTE_DUPLICATE",
        ),
        (
            "class C { @test public function f() : Int = 1 }",
            "E_ATTRIBUTE_CONTEXT",
        ),
        (
            "public public function f() : Int = 1",
            "E_MODIFIER_DUPLICATE",
        ),
        (
            "public private function f() : Int = 1",
            "E_MODIFIER_CONFLICT",
        ),
        (
            "class C { public static static function f() : Int = 1 }",
            "E_MODIFIER_DUPLICATE",
        ),
        ("public import std.math", "E_MODIFIER_CONTEXT"),
        (
            "trait T { public type Item\nfunction f() : Item }",
            "E_MODIFIER_CONTEXT",
        ),
        (
            "trait T { static type Item\nfunction f() : Item }",
            "E_STATIC_CONTEXT",
        ),
    ] {
        refuses(source, code);
    }
    good("sealed trait T { function f() : Int }");
    good("@test function check() : void {}");
    good("@simp theorem t(n : Int) : n = n {rfl}");
}

#[test]
fn trait_methods_may_supply_default_bodies() {
    good("trait Sized { function size() : Int = 1 }\nclass One { has Sized }");
    good(
        "trait Sized { function size() : Int }\nclass One { has Sized\nnew() {}\npublic function size() : Int = 1 }",
    );
    refuses("trait Sized { function size() : Int = true }", "E_TYPE");
}

#[test]
fn trait_defaults_need_explicit_overrides_and_report_conflicts() {
    refuses(
        "trait Named { public function name() : String = \"anon\" }\nclass User { has Named\npublic function name() : String = \"Ada\" }",
        "E_OVERRIDE_REQUIRED",
    );
    good(
        "trait Named { public function name() : String = \"anon\" }\nclass User { has Named\n@override public function name() : String = \"Ada\" }",
    );
    let conflict = checked(
        "trait First { public function name() : String = \"first\" }\ntrait Second { public function name() : String = \"second\" }\nclass User { has First, Second }",
    );
    assert!(
        conflict
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E_TRAIT_CONFLICT"),
        "{:#?}",
        conflict.diagnostics
    );
    assert!(
        !conflict
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E_TRAIT_METHOD"),
        "{:#?}",
        conflict.diagnostics
    );
    good(
        "trait First { public function name() : String = \"first\" }\ntrait Second { public function name() : String = \"second\" }\nclass User { has First, Second\n@override public function name() : String = \"both\" }",
    );
}

#[test]
fn default_method_visibility_matches_the_trait_declaration() {
    good(
        "trait PrivateName { function name() : String = \"anon\" }\nclass User { has PrivateName\nnew() {} }\nfunction viaTrait(user : &dyn PrivateName) : String = user.name()",
    );
    refuses(
        "trait PrivateName { function name() : String = \"anon\" }\nclass User { has PrivateName\nnew() {} }\nfunction direct(user : User) : String = user.name()",
        "E_MEMBER_PRIVATE",
    );
}

#[test]
fn trait_defaults_follow_has_chains_and_diamonds_and_inherit_class_methods() {
    good(
        "trait Named { public function name() : String = \"anon\" }\ntrait Labeled { has Named }\ntrait Tagged { has Named }\nclass User { has Labeled, Tagged }",
    );
    good(
        "trait Named { public function name() : String = \"anon\" }\nclass Parent { public function name() : String = \"parent\" }\nclass User extends Parent { has Named }",
    );
}

#[test]
fn abstract_trait_defaults_are_checked_in_their_declaration_scope() {
    good("trait Area {function width() : Int\nfunction area(height : Int = this.width()) : Int}");
    refuses("trait Bad {function read(n : Int = true) : Int}", "E_TYPE");
    refuses(
        "trait Bad {function read(n : Int = missing) : Int}",
        "E_UNKNOWN_NAME",
    );
    refuses(
        "trait Bad {function read(n : Int = {print(1)\n3}) : Int}",
        "E_EFFECT_REQUIRED",
    );
    refuses(
        "trait Bad {function read(n : Int = {return 3}) : Int}",
        "E_DEFAULT_CONTROL",
    );
    refuses(
        "trait Bad {mut function bump() : Int\nfunction read(n : Int = this.bump()) : Int}",
        "E_MUT_RECEIVER",
    );
}

#[test]
fn written_mutable_function_types_preserve_permissions_in_aliases_and_results() {
    let result = good(include_str!("../testdata/language/callbacks.kes"));
    let callback = result
        .resolved_calls
        .values()
        .find_map(|call| {
            if let Type::Function(function) = call.signature.result.as_ref() {
                Some(function)
            } else {
                None
            }
        })
        .expect("counter returns a callback");
    assert!(callback.receiver_mutates);
    assert_eq!(
        Type::Function(callback.clone()).to_string(),
        "mut (() -> Int)"
    );
    for source in [
        "alias Wrong = mut Int",
        "alias Wrong = mut (Int, Int)",
        "function bad(value : mut Int) : void {}",
    ] {
        refuses(source, "E_MUT_FUNCTION_TYPE");
    }
    refuses(
        "class Wrong {final step : mut (() -> Int)\nnew(move step : mut (() -> Int)) {this.step=step}}",
        "E_FINAL_TYPE",
    );
}

#[test]
fn text_bounds_and_handwritten_text_methods() {
    good(
        "class Point { has Text\nfunction text() : String = \"point\"\n}\nfunction show[T : Text](x : T) : String = x.text()\nfunction main(p : Point) : String = show(p)",
    );
    refuses(
        "function show[T : Display](x : T) : String = \"x\"",
        "E_TEXT_TRAIT_NAME",
    );
    refuses("class Point { has Text }", "E_TRAIT_METHOD");
}

#[test]
fn generic_bound_is_enforced_on_calls() {
    refuses(
        "class Point {}\nfunction show[T : Text](x : T) : String = x.text()\nfunction bad(p : Point) : String = show(p)",
        "E_TRAIT_BOUND",
    );
}

#[test]
fn generic_parameters_stay_rigid_inside_the_body() {
    refuses("function bad(x : T) : T = x + 1", "E_OPERATOR_TRAIT");
    good("function same(x : T) : T = x\nlet a := same(1)\nlet b := same(true)");
}

#[test]
fn local_lambdas_are_generalized_without_sharing_call_variables() {
    good("function demo() : Bool { let id := x => x\nlet a := id(3)\nid(true) }");
}

#[test]
fn infinite_lambda_types_are_refused() {
    refuses("let omega := x => x(x)", "E_TYPE");
}

#[test]
fn numeric_literals_and_same_type_equality() {
    good(
        "let large : BigInt = 1_000_000_000_000_000_000_000\nlet same := 1 == 2\nlet mixed := 1 == 1.0",
    );
    refuses("let x : Int = true", "E_TYPE");
}

#[test]
fn strict_operators_need_grouping() {
    refuses(
        "function f(a : Int, b : Int, c : Int) : Bool = a & b == c",
        "E_OPERATOR_MIX",
    );
    good("function f(a : Int, b : Int, c : Int) : Bool = (a & b) == c");
    // Q11: a comparison may sit directly inside && and ||.
    good("function f(n : Int) : Bool = n > 0 && n < 10 || n == 100");
    good("function f(n : Int) : Bool = (n > 0) && (n < 10)");
    // Arithmetic under a comparison needs no parentheses, as the user asked. Bitwise operators still do.
    good("function f(n : Int) : Bool = n + 1 > 0 && n < 10");
    good("function f(v : Int) : Bool = v % 2 == 0");
    refuses(
        "function f(a : Int, b : Int) : Bool = a & b > 0 && true",
        "E_OPERATOR_MIX",
    );
    refuses(
        "function f(a : Bool, b : Int) : Int = a && b + 1",
        "E_OPERATOR_MIX",
    );
    good("function f(a : Int) : Int = 2 * a ** 2 - 1");
    good("function f(a : double) : double = (-a) ** 2.0");
    refuses(
        "function f(a : Int, b : double) : Int = a ** b",
        "E_NUMERIC_TYPE",
    );
}

#[test]
fn arrays_index_with_brackets_and_need_mut_to_change() {
    good(
        "let a := Array(3, 0)\na[0] = 5\nlet x : Int = a[0] + a.length()\nlet y : Option[Int] = a.get(9)",
    );
    good("function f(mut xs : Array[String]) : void { xs[0] = xs[1] + \"!\" }");
    refuses(
        "function f(xs : Array[Int]) : void { xs[0] = 1 }",
        "E_MUT_PARAMETER",
    );
    refuses("let a := Array(3, 0)\na[0] = \"x\"", "E_TYPE");
    refuses("let a := Array(3, 0)\nlet b := a[true]", "E_TYPE");
    refuses(
        "let m := Map(\"a\" => 1)\nlet v := m[\"a\"]",
        "E_INDEX_TYPE",
    );
    // sorted() needs a List or Array of Comparable items, and sort() needs a mutable Array (Q123).
    refuses(
        "let s := Set(1, 2)\nlet t := s.sorted()",
        "E_UNKNOWN_MEMBER",
    );
    refuses("let xs := [1, 2]\nxs.sort()", "E_UNKNOWN_MEMBER");
    refuses(
        "function f(a : Array[Int]) : void { a.sort() }",
        "E_MUT_PARAMETER",
    );
    refuses(
        "class Item {\n    public final key : Int\n    new(k : Int) { this.key = k }\n}\nlet xs := [Item(1)]\nlet ys := xs.sorted()",
        "E_TRAIT_BOUND",
    );
    refuses("let xs := [1, 2]\nlet v := xs[0]", "E_INDEX_TYPE");
    // A return in a match arm still has the function's return type (Q122).
    refuses(
        "function f(n : Int) : Bool {\nmatch n {\n0 => return 5,\n_ => true,\n}\n}",
        "E_TYPE",
    );
    refuses(
        "let a := Array(2, 0)\nlet b := a\nprint(a[0])",
        "E_USE_AFTER_MOVE",
    );
}

#[test]
fn partial_calls_and_pipes_infer_function_types() {
    let result = good(
        "function add(a : double, b : double) : double = a + b\nlet plusOne := add(1.0, _)\nlet result : double = 3.0 |> plusOne",
    );
    assert!(
        result
            .expression_types
            .iter()
            .any(|(_, ty)| *ty == Type::named("double"))
    );
}

#[test]
fn conversion_names_can_be_pipe_targets() {
    good(
        "let a := 1 |> double\nlet b := [0, 0] |> Tensor\nlet c : double = 1 |> double\nlet d : Tensor[double, (2,)] = [0, 0] |> Tensor",
    );
    refuses("let a := [0, 0] |> double", "E_CONVERSION");
    refuses("let b := 1 |> Tensor", "E_TENSOR_ELEMENT");
}

#[test]
fn constructor_fields_declare_private_class_state() {
    good(
        "class Counter {\npublic var label : String\nnew(label : String, start : Int) {\nthis.label = label\nfinal this.start : Int := start\nvar this.hits : Int := 0\n}\npublic function value() : Int = this.start + this.hits\npublic mut function bump() : void { this.hits = this.hits + 1 }\n}",
    );
    // The declaration can live in one constructor; another constructor initializes the same field.
    good(
        "class Pair {\nnew() {\nfinal this.x : Int := 0\n}\nnew(x : Int) {\nthis.x = x\n}\npublic function get() : Int = this.x\n}",
    );
    refuses(
        "class A {\nnew() {\npublic final this.x : Int := 1\n}\n}",
        "E_CONSTRUCTOR_FIELD_VISIBILITY",
    );
    refuses(
        "class A {\nnew() {\nstatic final this.x : Int := 1\n}\n}",
        "E_CONSTRUCTOR_FIELD_STATIC",
    );
    refuses(
        "class A {\nnew() {\nif true {\nfinal this.x : Int := 1\n}\n}\n}",
        "E_FIELD_DECLARATION_CONTEXT",
    );
    refuses(
        "class A {\nnew() {}\npublic function f() : Int {\nfinal this.x : Int := 1\nthis.x\n}\n}",
        "E_FIELD_DECLARATION_CONTEXT",
    );
    refuses(
        "class A {\nnew() {\nfinal this.x : Int := \"one\"\n}\n}",
        "E_TYPE",
    );
    refuses(
        "class A {\nnew() {\nfinal this.x : Int := 1\n}\nnew(y : Int) {}\n}",
        "E_FIELD_UNINITIALIZED",
    );
    refuses(
        "class A {\nnew() {\nfinal this.x : Int := 1\n}\nnew(x : Int) {\nfinal this.x : Int := x\n}\n}",
        "E_DUPLICATE_NAME",
    );
    refuses(
        "class Base {\nnew() {\nfinal this.x : Int := 1\n}\n}\nclass Child extends Base {\nnew() {}\n}",
        "E_CONSTRUCTOR_FIELD_INHERITANCE",
    );
}

#[test]
fn named_and_default_arguments_are_checked() {
    good(
        "function scaled(x : double, scale : double = 1.0) : double = x * scale\nscaled(2.0)\nscaled(2.0, scale = 3.0)",
    );
    refuses(
        "function scaled(x : double) : double = x\nscaled(y = 2.0)",
        "E_CALL_ARGUMENTS",
    );
    good("function scaled(x : double) : double = x\nscaled(2)");
    refuses(
        "function scaled(x : double) : double = x\nscaled(\"2\")",
        "E_CALL_ARGUMENTS",
    );
}

#[test]
fn increment_needs_a_mutable_numeric_place() {
    good("let mut k := 0\nk++\nk--\nfor (let mut i : Int = 0; i < 3; i++) { }");
    refuses("let k := 0\nk++", "E_IMMUTABLE_ASSIGNMENT");
    refuses("1++", "E_ASSIGNMENT_TARGET");
    refuses("let mut text := \"\"\ntext++", "E_OPERATOR_TRAIT");
}

#[test]
fn argument_type_errors_name_the_argument_and_both_types() {
    let result = checked("function greet(name : Int) : void with Effects {}\ngreet(\"Geronimo\")");
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E_CALL_ARGUMENTS")
        .expect("call argument diagnostic");
    assert!(
        diagnostic
            .message
            .contains("Argument 1 of `greet` (`name`)"),
        "{}",
        diagnostic.message
    );
    assert!(
        diagnostic.message.contains("expects type `Int`"),
        "{}",
        diagnostic.message
    );
    assert!(
        diagnostic
            .message
            .contains("`\"Geronimo\"` has type `String`"),
        "{}",
        diagnostic.message
    );
}

#[test]
fn call_argument_count_and_names_are_explicit() {
    let missing = checked("function pair(a : Int, b : Int) : Int = a + b\npair(1)");
    let missing = missing
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E_CALL_ARGUMENTS")
        .expect("missing argument diagnostic");
    assert!(
        missing
            .message
            .contains("expects 2 arguments (a : Int, b : Int), but 1 was supplied"),
        "{}",
        missing.message
    );

    let unknown = checked("function greet(name : Int) : Int = name\ngreet(other = 1)");
    let unknown = unknown
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E_CALL_ARGUMENTS")
        .expect("unknown name diagnostic");
    assert!(
        unknown.message.contains("has no parameter named `other`"),
        "{}",
        unknown.message
    );
}

#[test]
fn unknown_names_naming_and_local_shadowing() {
    refuses("function snake_case(x : Int) : Int = x", "E_NAMING");
    refuses("function f(x : Int) : Int { let x := 2\nx }", "E_SHADOW");
    refuses("function f() : Int = unknown", "E_UNKNOWN_NAME");
    good("function f() : Int { if true { let x := 1 } else { let x := 2 }\n0 }");
}

#[test]
fn assignments_require_let_mut_and_the_same_type() {
    good("function f() : Int with Effects { let mut n := 0\nn = 2\nn }");
    refuses("let n := 0\nn = 2", "E_IMMUTABLE_ASSIGNMENT");
    refuses("let mut n := 0\nn = true", "E_TYPE");
}

#[test]
fn if_branches_and_returns_are_checked() {
    refuses("function f() : Int = if true then 1 else false", "E_TYPE");
    refuses("function f() : Int { return true }", "E_TYPE");
    good("function f(n : Int) : Int { if n > 0 { return 1 }\n0 }");
}

#[test]
fn nested_enum_patterns_are_exhaustive() {
    good(
        "enum Flag { Flag(Bool) }\nfunction f(x : Flag) : Int = match x { Flag(true) => 1, Flag(false) => 0, }",
    );
    refuses(
        "enum Flag { Flag(Bool) }\nfunction f(x : Flag) : Int = match x { Flag(true) => 1, }",
        "E_MATCH_EXHAUSTIVE",
    );
    refuses(
        "enum Flag { On\nOff }\nfunction f(x : Flag) : Int = match x { On => 1, }",
        "E_MATCH_EXHAUSTIVE",
    );
}

#[test]
fn guards_do_not_count_as_complete_coverage() {
    refuses(
        "function f(x : Option[Int]) : Int = match x { Some(n) if n > 0 => n, None => 0, }",
        "E_MATCH_EXHAUSTIVE",
    );
    good(
        "function f(x : Option[Int]) : Int = match x { Some(n) if n > 0 => n, Some(_) => 0, None => 0, }",
    );
}

#[test]
fn list_and_tuple_patterns_cover_nested_cases() {
    good("function f(xs : List[Int]) : Int = match xs { [] => 0, [x, ..rest] => x, }");
    refuses(
        "function f(x : (Bool, Bool)) : Int = match x { (true, _) => 1, (false, true) => 2, }",
        "E_MATCH_EXHAUSTIVE",
    );
    good(
        "function f(x : (Bool, Bool)) : Int = match x { (true, _) => 1, (false, true) => 2, (false, false) => 3, }",
    );
}

#[test]
fn union_name_patterns_cover_values_but_not_errors() {
    refuses(
        "function f(x : Int or Error) : Int = match x { n => n, }",
        "E_MATCH_EXHAUSTIVE",
    );
    good("function f(x : Int or Error) : Int = match x { n => n, _ => 0, }");
}

#[test]
fn trait_implementation_and_override_rules_are_checked() {
    good(
        "trait Shape { function area() : double }\nclass Figure { public function area() : double = 0.0 }\nclass Square extends Figure { has Shape\n@override public function area() : double = 1.0 }",
    );
    refuses(
        "class Figure { public function area() : double = 0.0 }\nclass Square extends Figure { @override public function area() : Int = 1 }",
        "E_OVERRIDE_RETURN",
    );
    refuses(
        "class Figure { public function area() : double = 0.0 }\nclass Square extends Figure { public function area() : double = 1.0 }",
        "E_OVERRIDE_REQUIRED",
    );
    refuses(
        "class Square { @override function area() : double = 1.0 }",
        "E_OVERRIDE_MISSING",
    );
}

#[test]
fn constructor_fields_and_generic_trait_calls_work() {
    good(
        "trait Shape { function area() : double }\nclass Square { has Shape\nfinal side : double\nnew(side : double) { this.side = side }\npublic function area() : double = this.side * this.side }\nfunction biggest[S : Shape](a : S, b : S) : S = if a.area() >= b.area() then a else b\nlet a := Square(1.0)\nlet b := Square(2.0)\nbiggest(a, b).area()",
    );
}

#[test]
fn function_type_arguments_are_refused() {
    refuses(
        "function same(x : T) : T = x\nsame[Int](1)",
        "E_CALL_TYPE_ARGUMENTS",
    );
}

#[test]
fn unsupported_forms_never_count_as_checked_successes() {
    good("let m := Map(\"a\" => 1)");
    // Q71: Nat is standard now, and a numeral is a Nat where one is expected.
    good("function f(n : Nat) : Int = 1\nlet two : Nat = 2\nlet next : Nat = Succ(two)");
    refuses("enum Nat { Zero, Succ(Nat) }", "E_DUPLICATE_NAME");
}

#[test]
fn original_rules_module_checks_after_settled_example_corrections() {
    good(include_str!("../testdata/language/rules.kes"));
}

#[test]
fn generic_classes_enforce_bounds_and_keep_them_in_methods() {
    let source = "class Holder[T : Text] { final value : T\nnew(move value : T) { this.value = value }\npublic function describe() : String = this.value.text() }\n";
    good(&format!("{source}let h := Holder(3)\nh.describe()"));
    refuses(
        &format!("{source}class Point {{ new() {{ }} }}\nHolder(Point())"),
        "E_TRAIT_BOUND",
    );
    refuses(
        "class Holder { final value : T }",
        "E_UNBOUND_TYPE_PARAMETER",
    );
}

#[test]
fn annotated_partial_calls_and_generic_bounds_survive_aliasing() {
    good(
        "function add(a : Int, b : Int) : Int = a + b\nlet plusOne : Int -> Int = add(1, _)\nplusOne(2)",
    );
    good(
        "function show[T : Text](x : T) : String = x.text()\nlet formatter := show(_)\nformatter(1)",
    );
    refuses(
        "class Point { new() {} }\nfunction show[T : Text](x : T) : String = x.text()\nlet formatter := show(_)\nformatter(Point())",
        "E_TRAIT_BOUND",
    );
}

#[test]
fn expected_return_type_supplies_a_return_only_type_variable() {
    good("function empty() : List[T] = []\nlet xs : List[Int] = empty()");
    good(
        "function empty() : List[T] = []\nlet xs : List[String] = empty()\nlet ys : List[Int] = empty()",
    );
}

#[test]
fn map_and_filter_remain_lazy_until_to_list() {
    good(
        "let xs := [1, 2, 3]\nlet ys : List[Int] = xs.filter(x => x > 1).map(x => x * 2).toList()",
    );
    refuses(
        "let xs := [1, 2]\nlet ys : List[Int] = xs.map(x => x * 2)",
        "E_TYPE",
    );
    good(
        "function sum(xs : List[Int]) : Int with Effects { let mut total := 0\nfor (x in xs.map(x => x * 2)) { total = total + x }\ntotal }",
    );
}

#[test]
fn aliases_work_and_cycles_have_a_bounded_check() {
    good("alias Count = Int\nfunction twice(x : Count) : Count = x + x\nlet n : Count = twice(2)");
    refuses("alias A = List[A]", "E_TYPE_CYCLE");
    refuses("alias A = B\nalias B = A", "E_TYPE_CYCLE");
}

#[test]
fn named_constant_initializers_are_checked() {
    good("const FIRST : Int = 1\nconst SECOND : Int = FIRST");
    refuses("const FIRST : Int = missing", "E_UNKNOWN_NAME");
}

#[test]
fn private_members_and_self_are_scoped_to_the_class() {
    good(
        "class Point { private final x : Int\nnew(x : Int) { this.x = x }\npublic function value() : Int = this.x }\nPoint(1).value()",
    );
    refuses(
        "class Point { private final x : Int\nnew(x : Int) { this.x = x } }\nPoint(1).x",
        "E_MEMBER_PRIVATE",
    );
    refuses(
        "class Point { private function value() : Int = 1 }\nfunction get(p : Point) : Int = p.value()",
        "E_MEMBER_PRIVATE",
    );
}

const VEC: &str = "enum Vec[T](n : Nat) {\nNil : Vec[T](0)\nCons(x : T, xs : Vec[T](n)) : Vec[T](n + 1)\n}\nfunction head(v : Vec[T](n + 1)) : T = match v { Cons(x, _) => x, }\n";

#[test]
fn successor_lengths_are_solved_before_runtime() {
    good(&format!(
        "{VEC}let xs : Vec[Int](3) = Cons(1, Cons(2, Cons(3, Nil)))\nlet first : Int = head(xs)"
    ));
    refuses(
        &format!("{VEC}let xs : Vec[Int](0) = Nil\nhead(xs)"),
        "E_INDEX_LENGTH",
    );
}

#[test]
fn generic_length_calls_have_independent_index_variables() {
    good(&format!(
        "{VEC}function same(v : Vec[T](n)) : Vec[T](n) = v\nlet a : Vec[Int](1) = same(Cons(1, Nil))\nlet b : Vec[Int](2) = same(Cons(1, Cons(2, Nil)))"
    ));
}

#[test]
fn additive_length_identities_preserve_universal_variables() {
    good(&format!(
        "{VEC}function regroup(a : Vec[Int](n), b : Vec[Int](m), total : Vec[Int](n + (m + n))) : Vec[Int]((n + n) + m) = total"
    ));
    refuses(
        &format!(
            "{VEC}function wrong(a : Vec[Int](n), b : Vec[Int](m), total : Vec[Int](n + n)) : Vec[Int](n + m) = total"
        ),
        "E_TYPE",
    );
}

#[test]
fn numeric_mixing_requires_explicit_conversions_and_matching_literals() {
    good("function positive(x : double) : Bool = x > 0");
    good("function positive(x : double) : Bool = x > 0.0");
    good("function bigger(a : double, b : Int) : Bool = a > b.toDouble()");
    refuses(
        "function compare(x : double, n : Int) : Bool = x > n",
        "E_NUMERIC_TYPE",
    );
    refuses("let n : Int = 1\nlet x : double = n", "E_TYPE");
}

#[test]
fn recursive_empty_types_do_not_exhaust_the_checker_stack() {
    refuses(
        "enum Loop { Next(Loop) }\nfunction absurd(x : Loop) : Int = match x {}",
        "E_MATCH_COMPLEX",
    );
}

#[test]
fn generic_functions_can_be_passed_as_function_values() {
    good(
        "function same(x : T) : T = x\nfunction apply(f : T -> U, x : T) : U = f(x)\nlet n : Int = apply(same, 3)\nlet b : Bool = apply(same, true)",
    );
    good("function same(x : T) : T = x\nlet f : Int -> Int = same\nf(3)");
}

#[test]
fn callback_bounds_are_not_erased_by_function_types() {
    good(
        "function show[T : Text](x : T) : String = x.text()\nfunction use(f : Int -> String) : String = f(3)\nuse(show)",
    );
    refuses(
        "class Point {}\nfunction show[T : Text](x : T) : String = x.text()\nfunction use(f : Point -> String, p : Point) : String = f(p)\nfunction bad(p : Point) : String = use(show, p)",
        "E_TRAIT_BOUND",
    );
}

#[test]
fn annotated_error_unions_accept_both_value_and_error_branches() {
    good("function choose(ok : Bool, error : Error) : Int or Error = if ok then 1 else error");
}

#[test]
fn borrowed_trait_views_and_owned_boxes_are_explicitly_typed() {
    let shape = "trait Shape { function area() : double }\nclass Square { has Shape\nfinal side : double\nnew(side : double) { this.side = side }\npublic function area() : double = this.side * this.side }\n";
    good(&format!(
        "{shape}function area(s : &dyn Shape) : double = s.area()\nlet sq := Square(2.0)\narea(sq)"
    ));
    good(&format!(
        "{shape}let s : Box[dyn Shape] = box(Square(2.0))\ns.area()"
    ));
    good(&format!(
        "{shape}function wrap(move value : T) : Box[T] = box(value)\nlet s : Box[dyn Shape] = wrap(Square(2.0))\ns.area()"
    ));
    refuses(
        "trait Shape { function area() : double }\nclass Point { new() {} }\nfunction area(s : &dyn Shape) : double = s.area()\narea(Point())",
        "E_TYPE",
    );
}

#[test]
fn dyn_rejects_methods_that_need_a_hidden_concrete_type() {
    refuses(
        "function compare(x : &dyn Equals) : void {}",
        "E_DYN_OBJECT_SAFETY",
    );
    refuses(
        "trait Generic { function same(x : T) : T }\nfunction use(x : &dyn Generic) : void {}",
        "E_DYN_OBJECT_SAFETY",
    );
}

#[test]
fn field_initializers_are_type_checked() {
    good("class Point { final x : Int = 1 }");
    refuses("class Point { final x : Int = true }", "E_TYPE");
    refuses("class Holder[T] { final value : T = 1 }", "E_TYPE");
}

#[test]
fn calls_supply_expected_types_to_literals_and_lambda_parameters() {
    good("function same(x : BigInt) : BigInt = x\nsame(1_000_000_000_000_000_000_000)");
    good("let xs := [1, 2, 3]\nlet ys : List[Int] = xs.map(x => -x).toList()");
    good("function apply(f : T -> U, x : T) : U = f(x)\nlet n : Int = apply(x => -x, 3)");
}

#[test]
fn print_constraints_survive_partial_application_and_aliases() {
    good("let write := print(_)\nwrite(1)");
    refuses(
        "class Point { new() {} }\nlet write := print(_)\nwrite(Point())",
        "E_TRAIT_BOUND",
    );
}

#[test]
fn local_length_annotations_cannot_rebind_a_functions_universal_index() {
    refuses(
        &format!("{VEC}function bad(v : Vec[Int](n)) : Int {{ let x : Vec[Int](n) = Nil\n0 }}"),
        "E_TYPE",
    );
}

#[test]
fn index_literals_and_sums_never_fail_silently() {
    good(&format!("{VEC}let one : Vec[Int](0x1) = Cons(1, Nil)"));
    refuses(
        &format!("{VEC}function f(v : Vec[Int](18446744073709551616)) : Int = 0"),
        "E_INDEX_RANGE",
    );
    refuses(
        &format!("{VEC}function f(v : Vec[Int](18446744073709551615 + 1)) : Int = 0"),
        "E_INDEX_RANGE",
    );
}

#[test]
fn mutable_storage_keeps_one_element_type_across_uses() {
    good("let m := MutMap()\nm.put(\"a\", 1)\nm.put(\"b\", 2)");
    refuses(
        "let m := MutMap()\nm.put(\"a\", 1)\nm.put(\"b\", true)",
        "E_CALL_ARGUMENTS",
    );
    good("let s := MutSet()\ns.add(1)\ns.contains(2)");
    refuses(
        "let s := MutSet()\ns.add(1)\ns.add(\"wrong\")",
        "E_CALL_ARGUMENTS",
    );
}

#[test]
fn immutable_map_updates_have_new_collection_types() {
    good(
        "let empty : Map[String, Int] = Map()\nlet updated := empty.put(\"a\", 1)\nfunction get(m : Map[String, Int]) : Int = match m.get(\"a\") { Some(v) => v, None => 0, }\nget(empty)\nget(updated)",
    );
}

#[test]
fn closures_cannot_generalize_the_type_of_captured_mutable_storage() {
    refuses(
        "let m := MutMap()\nlet get := () => m.get(\"a\")\nm.put(\"a\", 1)\nlet wrong : Option[String] = get()",
        "E_TYPE",
    );
    refuses(
        "function make() : T -> void { let m := MutMap()\nmove x => { m.put(\"a\", x) } }\nlet store := make()\nstore(1)\nstore(true)",
        "E_CALL_ARGUMENTS",
    );
}

#[test]
fn maps_accept_variable_and_expression_keys() {
    good(
        "let key := \"score\"\nlet m := Map(key => 10, \"bonus\" => 5)\nlet value : Option[Int] = m.get(key)",
    );
    good("let m := Map((1 + 2) => true)\nlet value : Option[Bool] = m.get(3)");
    refuses("Map(1 => true, \"two\" => false)", "E_TYPE");
    refuses("Map(1)", "E_MAP_ENTRY");
    refuses("class Key { new() {} }\nMap(Key() => 1)", "E_TRAIT_BOUND");
}

#[test]
fn map_key_requirements_survive_method_values_and_partial_calls() {
    refuses(
        "class Key { new() {} }\nlet m : Map[Key, Int] = Map()\nlet put := m.put\nput(Key(), 1)",
        "E_TRAIT_BOUND",
    );
    refuses(
        "class Key { new() {} }\nlet m : Map[Key, Int] = Map()\nlet put := m.put(_, 1)\nput(Key())",
        "E_TRAIT_BOUND",
    );
    good("let m := Map()\nlet put := m.put(_, 1)\nlet a := put(\"a\")");
}

const ERROR: &str = "enum ParseError extends Error { Invalid(String)\nMissing }\nfunction parse(s : String) : Int or ParseError { if s == \"\" { return Invalid(s) }\n42 }\n";

#[test]
fn named_error_enums_widen_to_standard_root() {
    good(&format!(
        "{ERROR}function total(s : String) : Int or Error = parse(s)\nfunction pass(e : ParseError) : Error = e\nfunction make() : Error = Missing"
    ));
    good(&format!(
        "{ERROR}function value(s : String) : Int = match parse(s) {{ Invalid(_) => 0, Missing => 0, n => n, }}"
    ));
    refuses(
        &format!(
            "{ERROR}function value(s : String) : Int = match parse(s) {{ Invalid(_) => 0, n => n, }}"
        ),
        "E_MATCH_EXHAUSTIVE",
    );
    refuses("enum Wrong extends Int { Nope }", "E_ENUM_EXTENSION");
}

#[test]
fn matching_open_error_root_requires_fallback() {
    good(&format!(
        "{ERROR}function handle(e : Error) : Int = match e {{ Invalid(_) => 1, Missing => 2, _ => 0, }}"
    ));
    refuses(
        &format!(
            "{ERROR}function handle(e : Error) : Int = match e {{ Invalid(_) => 1, Missing => 2, }}"
        ),
        "E_MATCH_EXHAUSTIVE",
    );
}

const PRODUCT: &str = "class Vector { has Multipliable[double], RightMultipliable[double]\nnew() {}\npublic function multiply(other : double) : Vector = this\npublic function rightMultiply(left : double) : Vector = this\n}\n";

#[test]
fn multiplication_dispatches_on_left_then_builtin_right_fallback() {
    good(&format!(
        "{PRODUCT}let v := Vector()\nlet a : Vector = v * 2.0\nlet b : Vector = 2.0 * v"
    ));
    refuses(
        &format!("{PRODUCT}let v := Vector()\nv * 2"),
        "E_OPERATOR_TRAIT",
    );
    refuses(
        "class Value { has RightMultipliable[double]\nnew() {}\npublic function rightMultiply(x : double) : Value = this }\nlet v := Value()\nv * 2.0",
        "E_OPERATOR_TRAIT",
    );
    refuses(
        "class Left { new() {} }\nclass Right { has RightMultipliable[Left]\nnew() {}\npublic function rightMultiply(x : Left) : Int = 1 }\nLeft() * Right()",
        "E_OPERATOR_TRAIT",
    );
}

#[test]
fn multiplication_contracts_check_operands_purity_and_output() {
    refuses(
        "class Value { has Multipliable[double]\npublic function multiply(x : Int) : Int = x }",
        "E_TRAIT_METHOD",
    );
    refuses(
        "class Value { has Multipliable[double]\npublic function multiply(x : double) : double with Effects = x }",
        "E_TRAIT_METHOD",
    );
    good(
        "class Value { has Multipliable[Int](Output = Bool)\nnew() {}\npublic function multiply(x : Int) : Bool = true }\nlet b : Bool = Value() * 2",
    );
    refuses(
        "class Value { has Multipliable[Int](Output = Bool)\npublic function multiply(x : Int) : Int = x }",
        "E_TRAIT_METHOD",
    );
}

#[test]
fn generic_multiplication_infers_associated_output_at_call() {
    good(&format!(
        "{PRODUCT}function scale[T : Multipliable[double](Output = R)](value : T) : R = value * 2.0\nlet scaled : Vector = scale(Vector())"
    ));
    refuses(
        &format!(
            "{PRODUCT}function scale[T : Multipliable[double](Output = R)](value : T) : R = value * 2.0\nlet wrong : Int = scale(Vector())"
        ),
        "E_TRAIT_BOUND",
    );
}

const ITERATION: &str = "class NumberCursor { has Iterator\nnew() {}\npublic function next() : Option[Int] = None }\nclass Numbers { has Iterable\nnew() {}\npublic function items() : NumberCursor = NumberCursor() }\n";

#[test]
fn concrete_custom_iteration_infers_item_from_next() {
    good(&format!(
        "{ITERATION}function sum(values : Numbers) : Int with Effects {{ let mut result := 0\nfor (n in values) {{ result = result + n }}\nresult }}\nlet total : Int = sum(Numbers())"
    ));
    good(&format!(
        "{ITERATION}let cursor := Numbers().items()\nlet n : Option[Int] = cursor.next()"
    ));
    good("let cursor := [1, 2].items()\nlet n : Option[Int] = cursor.next()");
    refuses(
        "class Bad { has Iterator\npublic function next() : Int = 1 }",
        "E_TRAIT_METHOD",
    );
    refuses(
        "class Bad { has Iterable\npublic function items() : Int = 1 }",
        "E_TRAIT_METHOD",
    );
    refuses("class Bad { has Iterable }", "E_TRAIT_METHOD");
}

#[test]
fn generic_iterable_bounds_preserve_item_types() {
    good(&format!(
        "{ITERATION}function count[T : Iterable(Item = Int)](mut values : T) : Int with Effects {{ let mut n := 0\nfor (value in values) {{ n = n + value }}\nn }}\ncount(Numbers())\ncount([1, 2])"
    ));
    refuses(
        "function sum[T : Iterable(Item = Int)](values : T) : Int = 0\nsum([true])",
        "E_TRAIT_BOUND",
    );
}

#[test]
fn collection_traits_are_conditional_on_elements() {
    good("print(Map(\"a\" => Some(1)))\nlet equal : Bool = [1, 2] == [1, 2]");
    refuses(
        "class Value { new() {} }\nprint([Value()])",
        "E_TRAIT_BOUND",
    );
    refuses(
        "class Value { new() {} }\nlet equal := [Value()] == [Value()]",
        "E_OPERATOR_TRAIT",
    );
}

#[test]
fn expected_enum_cases_and_qualified_names_do_not_collide_with_option() {
    good("enum Flag { Flag(Bool) }\nlet a : Flag = Flag(true)\nlet b : Flag = Flag.Flag(false)");
    good(
        "enum Choice { Some(Int)\nNone }\nlet a : Choice = Some(1)\nlet b : Choice = None\nlet c := Choice.Some(2)\nlet option := Option.Some(3)\nlet empty : Option[Int] = Option.None",
    );
    refuses(
        "enum Choice { Some(Int) }\nlet x := Some(1)",
        "E_CASE_AMBIGUOUS",
    );
    good("class Value { new() {} }\nalias Short = Value\nlet value : Value = Short()");
}

#[test]
fn numeric_lambda_parameters_are_inferred_before_trait_dispatch() {
    good(
        "let twice := x => x * 2.0\nlet a : double = twice(3.0)\nlet scale := x => 2 * x\nlet b : Int = scale(3)",
    );
}

#[test]
fn associated_slots_are_checked_and_dynamic_iterator_types_are_bound() {
    refuses(
        "class Value { has Multipliable[Int](Result = Bool)\npublic function multiply(x : Int) : Bool = true }",
        "E_ASSOCIATED_TYPE",
    );
    refuses(
        "function f(x : &dyn Iterator) : void {}",
        "E_DYN_OBJECT_SAFETY",
    );
    good("function f(mut x : &dyn Iterator(Item = Int)) : Option[Int] with Effects = x.next()");
    good(&format!(
        "{ITERATION}function f(mut x : &dyn Iterator(Item = Int)) : Option[Int] with Effects = x.next()\nf(NumberCursor())"
    ));
}

#[test]
fn local_mutation_does_not_require_outside_effects() {
    good("function f() : Int { let mut n := 0\nn = 1\nn }");
    good("function f() : void { let mut n := 0 }");
    good("function f() : void { for (let i : Int = 0; i < 2; i = i + 1) {} }");
    good("function f() : Int { let m := MutMap()\nm.put(\"a\", 1)\n1 }");
}

#[test]
fn effectful_calls_aliases_partial_functions_and_pipes_preserve_effects() {
    refuses("function f() : void = print(1)", "E_EFFECT_REQUIRED");
    good("function f() : void with Effects = print(1)");
    refuses(
        "function f() : void { let output := print(_)\noutput(1) }",
        "E_EFFECT_REQUIRED",
    );
    good("function f() : void { let output := print(_) }");
    refuses("function f() : void = 1 |> print", "E_EFFECT_REQUIRED");
    refuses(
        "function change(m : MutMap[String, Int]) : void = m.put(\"a\", 1)",
        "E_MUT_PARAMETER",
    );
    good("function change(mut m : MutMap[String, Int]) : void = m.put(\"a\", 1)");
    refuses(
        "function change(s : MutSet[Int]) : void = s.add(1)",
        "E_MUT_PARAMETER",
    );
}

#[test]
fn lambda_effects_are_inferred_and_body_creation_has_no_effect() {
    good("function f() : Int { let output := () => print(1)\n1 }");
    refuses(
        "function f() : void { let output := () => print(1)\noutput() }",
        "E_EFFECT_REQUIRED",
    );
    good("function f() : void with Effects { let output := () => print(1)\noutput() }");
    refuses(
        "function apply(f : Int -> void) : void = f(1)\napply(x => print(x))",
        "E_CALL_ARGUMENTS",
    );
    refuses("function f() : Int -> void = x => print(x)", "E_TYPE");
    refuses(
        "function outer() : void with Effects { function inner() : void = print(1) }",
        "E_EFFECT_REQUIRED",
    );
}

#[test]
fn cursor_effects_are_checked_by_direct_calls_and_for_loops() {
    let cursors = "class Cursor { has Iterator\nnew() {}\npublic function next() : Option[Int] with Effects { let mut n := 0\nNone } }\nclass Numbers { has Iterable\nnew() {}\npublic function items() : Cursor = Cursor() }\n";
    good(&format!(
        "{cursors}function run(n : Numbers) : void with Effects {{ for (x in n) {{}} }}"
    ));
    refuses(
        &format!("{cursors}function run(n : Numbers) : void {{ for (x in n) {{}} }}"),
        "E_EFFECT_REQUIRED",
    );
    refuses(
        &format!("{cursors}function run(c : Cursor) : Option[Int] = c.next()"),
        "E_EFFECT_REQUIRED",
    );
    refuses(
        "function run[T : Iterator(Item = Int)](cursor : T) : Option[Int] = cursor.next()",
        "E_EFFECT_REQUIRED",
    );
    good("function read(xs : List[Int]) : Option[Int] = xs.items().next()");
}

#[test]
fn error_propagation_requires_the_enclosing_error_return_type() {
    refuses(
        "function f() : Int = parseInt(\"1\")?",
        "E_PROPAGATION_RETURN",
    );
    good("let n := parseInt(\"1\")?");
    refuses("const N : Int = parseInt(\"1\")?", "E_PROPAGATION_RETURN");
    good("function f() : Int or Error = parseInt(\"1\")?");
    good(&format!(
        "{ERROR}function f() : Int or Error = parse(\"1\")?"
    ));
    refuses(
        "function f() : Int or Error { let get := () => parseInt(\"1\")?\nget() }",
        "E_PROPAGATION_RETURN",
    );
}

#[test]
fn contracts_expose_errors_and_cannot_hide_effects_in_predicates() {
    refuses(
        "function f(n : Int) : Int requires n > 0 = n",
        "E_CONTRACT_ERROR_TYPE",
    );
    good("function f(n : Int) : Int or Error requires n > 0 = n");
    refuses(
        "function valid() : Bool with Effects = true\nfunction f(n : Int) : Int or Error with Effects requires valid() = n",
        "E_EFFECT_REQUIRED",
    );
}

#[test]
fn constants_and_field_defaults_cannot_perform_effects() {
    refuses(
        "function read() : Int with Effects = 1\nconst VALUE : Int = read()",
        "E_EFFECT_REQUIRED",
    );
    refuses(
        "function read() : Int with Effects = 1\nclass Value { final n : Int = read() }",
        "E_EFFECT_REQUIRED",
    );
}

#[test]
fn inner_functions_and_lambdas_do_not_inherit_outer_loop_control() {
    refuses(
        "for (x in [1]) { let stop := () => { break } }",
        "E_LOOP_CONTEXT",
    );
    refuses(
        "for (x in [1]) { function stop() : void { continue } }",
        "E_LOOP_CONTEXT",
    );
}

#[test]
fn right_multiplication_accepts_nonnumeric_builtin_left_types() {
    good(
        "class Value { has RightMultipliable[String]\nnew() {}\npublic function rightMultiply(left : String) : Int = 1 }\nlet result : Int = \"x\" * Value()",
    );
}

#[test]
fn collection_equality_constraints_are_preserved() {
    refuses("class Value { new() {} }\nSet(Value())", "E_TRAIT_BOUND");
    refuses(
        "class Value { new() {} }\nlet xs := [Value()]\nlet contains := xs.contains\ncontains(Value())",
        "E_TRAIT_BOUND",
    );
    good("let s := Set(1, 2)\nlet add := s.add\nadd(3)");
}

#[test]
fn script_functions_capture_only_earlier_immutable_values_and_constants() {
    good("let n := 3\nfunction value() : Int = n\nvalue()");
    good("const MAX_TIME : Int = 10\nfunction value() : Int = MAX_TIME");
    refuses(
        "let mut n := 3\nfunction value() : Int with Effects = n",
        "E_SCRIPT_MUTABLE_CAPTURE",
    );
    refuses(
        "let m := MutMap()\nfunction value() : Option[Int] = m.get(\"a\")",
        "E_SCRIPT_MUTABLE_CAPTURE",
    );
    good("let mut n := 3\nfunction value(n : Int) : Int = n\nvalue(n)");
    refuses("function value() : Int = n\nlet n := 3", "E_UNKNOWN_NAME");
}

#[test]
fn ordinary_borrow_annotations_belong_only_in_stored_fields() {
    let square = "class Square { new() {} }\n";
    refuses(
        &format!("{square}function f(s : &Square) : void {{}}"),
        "E_BORROW_POSITION",
    );
    refuses(
        &format!("{square}function f(s : Square) : &Square = s"),
        "E_BORROW_POSITION",
    );
    refuses(
        &format!("{square}let s := Square()\nlet borrowed : &Square = s"),
        "E_BORROW_POSITION",
    );
    good(include_str!("../testdata/language/panel.kes"));
    good("function f(x : &dyn Text) : String = x.text()");
}

#[test]
fn qualified_patterns_match_the_owning_enum() {
    good(
        "enum First { Value(Int) }\nenum Second { Value(Bool) }\nfunction get(x : First or Second) : Int = match x { First.Value(n) => n, Second.Value(_) => 0, }",
    );
    good(
        "function get(x : Option[Int]) : Int = match x { Option.Some(n) => n, Option.None => 0, }",
    );
}

#[test]
fn static_vars_are_synchronized_shared_state() {
    good(
        "class C { public static var count : Int = 0\nnew() {}\npublic static function bump() : void with Effects { C.count = C.count + 1 }\npublic static function read() : Int with Effects = C.count }\nC.bump()",
    );
    good(
        "class C { public static var text : String = \"a\"\nnew() {} }\nfunction set(value : String) : void with Effects { C.text = value }\nset(\"b\")",
    );
    refuses(
        "class C { public static var count : Int = 0\nnew() {} }\nfunction read() : Int = C.count",
        "E_EFFECT_REQUIRED",
    );
    refuses(
        "class C { public static var count : Int = 0\nnew() {} }\nfunction set() : void { C.count = 1 }",
        "E_EFFECT_REQUIRED",
    );
    refuses(
        "class C { public static var a : Int = 1\npublic static final b : Int = C.a + 1\nnew() {} }",
        "E_CONST_DEPENDENCY",
    );
    refuses(
        "class C { public static var value : MutMap[String, Int] = MutMap()\nnew() {} }",
        "E_STATIC_TYPE",
    );
    refuses(
        "class C[T] { public static var value : Int = 1\nnew() {} }",
        "E_STATIC_GENERIC",
    );
}

#[test]
fn static_fields_need_a_value_and_an_initializer() {
    refuses(
        "class Counter { public static var count : Int }",
        "E_STATIC_INIT",
    );
    good("class Counter { private static final total : Int = 0 }");
}

#[test]
fn constructors_are_explicit_and_final_fields_cannot_change_later() {
    refuses(
        "class Point { public final x : Int }\nPoint(1)",
        "E_CONSTRUCTOR_REQUIRED",
    );
    refuses(
        "class Point { public final x : Int\nnew(x : Int) { this.x = x } }\nlet p := Point(1)\np.x = 2",
        "E_FINAL_ASSIGNMENT",
    );
    refuses(
        "class Point { final x : Int\nnew(x : Int) with Effects { this.x = x\nthis.x = x } }",
        "E_FINAL_ASSIGNMENT",
    );
    refuses(
        "class Point { final x : Int = 0\nnew(x : Int) with Effects { this.x = x } }",
        "E_FINAL_ASSIGNMENT",
    );
    good(
        "class Point { public var x : Int\nnew(x : Int) { this.x = x } }\nlet p := Point(1)\np.x = 2",
    );
}

#[test]
fn final_fields_require_immutable_types_including_generic_instances() {
    refuses("class Holder { final values : Array[Int] }", "E_FINAL_TYPE");
    refuses(
        "class Holder[T] { final value : T\nnew(value : T) { this.value = value } }\nHolder(MutMap())",
        "E_FINAL_TYPE",
    );
    good("class Holder { var values : Array[Int] }");
}

#[test]
fn changing_borrowed_parameter_fields_requires_mut() {
    let point = "class Point { public var x : Int\nnew() { this.x = 0 } }\n";
    refuses(
        &format!("{point}function change(p : Point) : void with Effects {{ p.x = 1 }}"),
        "E_MUT_PARAMETER",
    );
    good(&format!(
        "{point}function change(mut p : Point) : void with Effects {{ p.x = 1 }}"
    ));
    good(&format!(
        "{point}function change(move p : Point) : void with Effects {{ p.x = 1 }}"
    ));
}

#[test]
fn inherited_fields_preserve_types_and_visibility() {
    good(
        "class Parent[T] { public var value : T }\nclass Child extends Parent[Int] { new() { this.value = 0 } }\nfunction value(c : Child) : Int = c.value",
    );
    // A subclass that sets nothing would leave the inherited field unset.
    refuses(
        "class Parent[T] { public var value : T }\nclass Child extends Parent[Int] { new() {} }",
        "E_FIELD_UNINITIALIZED",
    );
    refuses(
        "class Parent { private var value : Int }\nclass Child extends Parent { new() {}\nfunction value() : Int = this.value }",
        "E_MEMBER_PRIVATE",
    );
    refuses(
        "class Parent { public final value : Int = 1 }\nclass Child extends Parent { new() with Effects { this.value = 2 } }",
        "E_FINAL_ASSIGNMENT",
    );
}

#[test]
fn constructor_initialization_is_checked_on_each_path() {
    good(
        "class Point { final x : Int\nnew(n : Int) { if n > 0 { this.x = n } else { this.x = 0 } } }",
    );
    good(
        "class Point { final x : Int\nnew(b : Bool) { match b { true => { this.x = 1 }, false => { this.x = 0 }, } } }",
    );
    refuses(
        "class Point { final x : Int\nnew(n : Int) { if n > 0 { this.x = n } } }",
        "E_FIELD_UNINITIALIZED",
    );
    refuses(
        "class Point { final x : Int\nnew(n : Int) { if n > 0 { return }\nthis.x = n } }",
        "E_FIELD_UNINITIALIZED",
    );
    refuses(
        "class Point { final x : Int\nnew() { this.x = this.x } }",
        "E_FIELD_UNINITIALIZED",
    );
    refuses(
        "class Point { final x : Int\nnew() with Effects { while true { this.x = 1 } } }",
        "E_FIELD_UNINITIALIZED",
    );
    refuses(
        "class Point { final x : Int\nnew() with Effects { for (n in [1, 2]) { this.x = n } } }",
        "E_FINAL_ASSIGNMENT",
    );
}

#[test]
fn parameter_modes_survive_function_values_and_partial_application() {
    good(
        "function own(move value : String) : String = value\nlet ownLater := own(_)\nownLater(\"value\")",
    );
    refuses(
        "function own(move value : String) : String = value\nfunction apply(f : String -> String) : String = f(\"value\")\napply(own)",
        "E_CALL_ARGUMENTS",
    );
    good(
        "function read(value : String) : String = value\nfunction apply(f : String -> String) : String = f(\"value\")\napply(read)",
    );
    refuses(
        "class Value { has Equals\npublic function equals(move other : Value) : Bool = true }",
        "E_TRAIT_METHOD",
    );
    refuses(
        "class Parent { public function read(value : String) : String = value }\nclass Child extends Parent { @override public function read(move value : String) : String = value }",
        "E_OVERRIDE_PARAMETER_MODE",
    );
    refuses(
        "class Value { has Multipliable[String]\npublic function multiply(move value : String) : Int = 1 }",
        "E_TRAIT_METHOD",
    );
}

#[test]
fn sorted_sets_require_pure_comparable_methods_instead_of_equality() {
    good(
        "class Key { has Comparable\npublic final number : Int\nnew(n : Int) { this.number = n }\npublic function compare(other : Key) : Int = this.number - other.number }\nlet keys := SortedSet(Key(1),Key(2))\nlet more := keys.add(Key(3))\nkeys.contains(Key(1))",
    );
    refuses(
        "class Key { has Comparable\npublic function compare(other : Key) : Bool = true }",
        "E_TRAIT_METHOD",
    );
    refuses(
        "class Key { has Comparable\npublic function compare(other : Key) : Int with Effects { print(1)\n0 } }",
        "E_TRAIT_METHOD",
    );
    refuses(
        "class Key { has Equals\nnew() {}\npublic function equals(other : Key) : Bool = true }\nlet keys := SortedSet(Key())",
        "E_TRAIT_BOUND",
    );
    good("function order[T : Comparable](a : T, b : T) : Int = a.compare(b)\norder(1,2)");
}

#[test]
fn let_bindings_keep_the_error_alternative_of_a_union() {
    refuses("let n := parseInt(\"5\")\nlet m : Int = n", "E_TYPE");
    good(
        "function f() : String {\nlet n := parseInt(\"5\")\nmatch n { RuntimeError.Parse(_) => \"bad\", value => value.text(), _ => \"other\", }\n}",
    );
    good("function f() : Int or Error {\nlet n := parseInt(\"5\")?\nn + 1\n}");
}

#[test]
fn python_callbacks_need_annotated_owning_functions() {
    let header = "import python \"builtins\" as builtins\n";
    good(&format!(
        "{header}function f(x : Int) : Int = x\nfunction run() : void or Error with Effects {{ let ys := builtins.map(f, [1])? }}"
    ));
    good(&format!(
        "{header}function run() : void or Error with Effects {{ let n := 2\nlet ys := builtins.map(move (x : Int) => x * n, [1])? }}"
    ));
    refuses(
        &format!(
            "{header}function run() : void or Error with Effects {{ let ys := builtins.map(x => x, [1])? }}"
        ),
        "E_PYTHON_TYPE",
    );
    good(&format!(
        "{header}import std.python.{{PythonValue}}\nfunction f(x : PythonValue) : Int = 1\nfunction run() : void or Error with Effects {{ let ys := builtins.map(f, [1])? }}"
    ));
    refuses(
        &format!(
            "{header}function run() : void or Error with Effects {{ let n := 2\nlet ys := builtins.map((x : Int) => x * n, [1])? }}"
        ),
        "E_LAMBDA_ESCAPE",
    );
}

#[test]
fn debug_cannot_hide_mutation_or_outside_effects_in_text_display() {
    for method in [
        "public mut function debug() : String = \"x\"",
        "public function debug() : String with Effects { print(1)\n\"x\" }",
        "public function debug() : Int = 1",
    ] {
        refuses(
            &format!(
                "class Item {{ has Text\npublic function text() : String = \"text\"\n{method} }}"
            ),
            "E_TRAIT_METHOD",
        );
    }
    good(
        "class Item { has Text\npublic function text() : String = \"text\"\npublic function debug(level : Int) : String = \"other\" }",
    );
}

#[test]
fn array_factories_check_effects_and_keep_repeated_fill_copyable() {
    good("class Item {new() {}}\nfunction create() : Array[Item] = Array.generate(3, i => Item())");
    refuses(
        "class Item {new() {}}\nlet xs := Array(3, Item())",
        "E_ARRAY_FILL",
    );
    refuses(
        "function noisy(i : Int) : Int with Effects {print(i)\ni}\nfunction bad() : Array[Int] = Array.generate(3, noisy)",
        "E_EFFECT_REQUIRED",
    );
}

#[test]
fn exponent_literals_are_double_in_expressions_and_patterns() {
    good(
        "let a : double = 1e3\nlet b : double = 2.5E-4\nlet h : Int = 0xDEAD\nlet k : Int = 0x1e3\nmatch a {1e3 => 1, _ => 0,}",
    );
    refuses("let a : Int = 1e3", "E_TYPE");
    refuses("let a : Nat = 1e3", "E_TYPE");
    good("let b : double = 1e3 + 1");
    refuses("let n := 3\nmatch n {3e0 => 1, _ => 0,}", "E_TYPE");
}

const ASSOCIATED_SOURCE: &str = "trait Source {type Item\nfunction next() : Option[Item]}\nclass Counter {has Source\nnew() {}\npublic function next() : Option[Int] = Some(1)}\n";

#[test]
fn user_traits_infer_associated_types_for_bounds_and_dynamic_views() {
    good(&format!(
        "{ASSOCIATED_SOURCE}function read[T : Source(Item = Int)](value : T) : Int = match value.next() {{Some(n) => n, None => 0,}}\nread(Counter())\nlet view : &dyn Source(Item = Int) = Counter()\nview.next()"
    ));
    refuses(
        &format!(
            "{ASSOCIATED_SOURCE}function read[T : Source(Item = String)](value : T) : void {{}}\nread(Counter())"
        ),
        "E_TRAIT_BOUND",
    );
    refuses(
        &format!("{ASSOCIATED_SOURCE}let view : &dyn Source = Counter()"),
        "E_DYN_OBJECT_SAFETY",
    );
}

#[test]
fn associated_type_declarations_and_bindings_have_checked_names() {
    refuses("class Bad {type Item\nnew() {}}", "E_ASSOCIATED_CONTEXT");
    refuses("trait Bad {type Item\ntype Item}", "E_ASSOCIATED_TYPE");
    refuses("trait Bad[Item] {type Item}", "E_ASSOCIATED_TYPE");
    refuses(
        &format!("{ASSOCIATED_SOURCE}function f[T : Source(Missing = Int)](value : T) : void {{}}"),
        "E_ASSOCIATED_TYPE",
    );
    good(
        "trait Marker {type Item}\nclass Mark {has Marker(Item = Int)\nnew() {}}\nfunction accept[T : Marker(Item = Int)](x : T) : void {}\naccept(Mark())",
    );
    refuses(
        "trait Marker {type Item}\nclass Mark {has Marker\nnew() {}}",
        "E_ASSOCIATED_INFER",
    );
}

#[test]
fn associated_types_unify_all_methods_and_require_unique_overloads() {
    good(
        "trait Codec {type Item\nfunction encode(x : Item) : String\nfunction decode(x : String) : Option[Item]}\nclass Numbers {has Codec\nnew() {}\npublic function encode(x : Int) : String = x.text()\npublic function decode(x : String) : Option[Int] = Some(1)}\nfunction next[T : Codec(Item = Int)](c : T) : Option[Int] = c.decode(\"a\")\nnext(Numbers())",
    );
    refuses(
        "trait Pair {type Item\nfunction first() : Item\nfunction second() : Item}\nclass Bad {has Pair\nnew() {}\npublic function first() : Int = 1\npublic function second() : String = \"a\"}",
        "E_ASSOCIATED_INFER",
    );
    let overloaded = "trait Echo {type Item\nfunction echo(x : Item) : Item}\nclass Both {has Echo\nnew() {}\npublic function echo(x : Int) : Int = x\npublic function echo(x : String) : String = x}\n";
    refuses(overloaded, "E_ASSOCIATED_INFER");
    good(&overloaded.replace("has Echo", "has Echo(Item = Int)"));
    refuses(
        "trait Source {type Item\nfunction next() : Item}\nclass Bad {has Source(Item = String)\nnew() {}\npublic function next() : Int = 1}",
        "E_ASSOCIATED_INFER",
    );
}

#[test]
fn associated_types_keep_trait_and_class_parameters_separate() {
    good(
        "class Item {new() {}}\ntrait Source {type Item\nfunction next() : Item}\nclass Value[T] {has Source\npublic final value : T\nnew(move value : T) {this.value = value}\npublic function next() : T = this.value}\nfunction read[T : Source(Item = Int)](x : T) : Int = x.next()\nread(Value(3))",
    );
    good(
        "trait Convert[Input] {type Output\nfunction convert(x : Input) : Output}\nclass Num {has Convert[String]\nnew() {}\npublic function convert(x : String) : Int = 1}\nfunction use[T : Convert[String](Output = Int)](x : T) : Int = x.convert(\"1\")\nuse(Num())",
    );
    refuses(
        "trait Bad {type Item\nfunction run[Item : Text](x : Item) : void}",
        "E_ASSOCIATED_TYPE",
    );
}

#[test]
fn associated_protocols_preserve_receiver_and_effect_permissions() {
    let source = "trait Step {type Item\nmut function next() : Option[Item] with Effects}\nclass Cursor {has Step\nnew() {}\npublic mut function next() : Option[Int] with Effects = Some(1)}\n";
    good(&format!(
        "{source}function run[T : Step(Item = Int)](mut source : T) : Option[Int] with Effects = source.next()\nrun(Cursor())"
    ));
    refuses(
        &format!(
            "{source}function run[T : Step(Item = Int)](source : T) : Option[Int] with Effects = source.next()"
        ),
        "E_MUT_PARAMETER",
    );
    refuses(
        &format!(
            "{source}function run[T : Step(Item = Int)](mut source : T) : Option[Int] = source.next()"
        ),
        "E_EFFECT_REQUIRED",
    );
    refuses(
        "trait Step {type Item\nmut function next() : Item}\nclass Bad {has Step\nnew() {}\npublic function next() : Int = 1}",
        "E_ASSOCIATED_INFER",
    );
}

#[test]
fn associated_instances_keep_generic_arguments_when_inferred_from_a_bound() {
    let source = "trait Source[Domain] {type Item\nfunction get() : Item}\nclass Value {has Source[String]\nnew() {}\npublic function get() : Int = 7}\nfunction infer[T : Source[Domain](Item = Int)](x : T) : Option[Domain] = None\nlet result := infer(Value())\n";
    good(&format!(
        "{source}function accept(x : Option[String]) : void {{}}\naccept(result)"
    ));
    refuses(
        &format!("{source}function accept(x : Option[Int]) : void {{}}\naccept(result)"),
        "E_CALL_ARGUMENTS",
    );
}

#[test]
fn match_arms_refine_a_fixed_length_by_the_case() {
    let append = "function append(move xs : Vec[T](n), move ys : Vec[T](m)) : Vec[T](n + m) = match xs { Nil => ys, Cons(x, rest) => Cons(x, append(rest, ys)), }\n";
    good(&format!("{VEC}{append}"));
    good(&format!(
        "{VEC}function rev(move xs : Vec[T](n), move acc : Vec[T](m)) : Vec[T](n + m) = match xs {{ Nil => acc, Cons(x, rest) => rev(rest, Cons(x, acc)), }}"
    ));
    // Once a is Cons, b has length k + 1, so its Nil case cannot occur.
    good(&format!(
        "{VEC}function pairs(move a : Vec[T](n), move b : Vec[T](n)) : Vec[T](n) = match a {{ Nil => b, Cons(x, rest) => match b {{ Cons(y, more) => Cons(x, pairs(rest, more)), }}, }}"
    ));
    good(&format!(
        "{VEC}function second(v : Vec[T](n + 2)) : T = match v {{ Cons(_, rest) => match rest {{ Cons(y, _) => y, }}, }}"
    ));
    // The refinement holds only inside its arm.
    refuses(
        &format!(
            "{VEC}{append}function lost(move xs : Vec[T](n), move ys : Vec[T](m)) : Vec[T](n + m) = match xs {{ Nil => ys, Cons(x, rest) => append(rest, ys), }}"
        ),
        "E_TYPE",
    );
    refuses(
        &format!(
            "{VEC}function swapped(move a : Vec[T](n), move b : Vec[T](m)) : Vec[T](n) = match a {{ Nil => b, Cons(x, rest) => Cons(x, rest), }}"
        ),
        "E_TYPE",
    );
    refuses(
        &format!(
            "{VEC}function second(v : Vec[T](n + 1)) : T = match v {{ Cons(_, rest) => match rest {{ Cons(y, _) => y, }}, }}"
        ),
        "E_MATCH_EXHAUSTIVE",
    );
}

#[test]
fn fixed_length_multiples_normalize_and_solve_exactly() {
    good(&format!(
        "{VEC}function regroup(total : Vec[Int](2 * (n + m) + 3)) : Vec[Int](n + n + m + m + 3) = total"
    ));
    good(&format!(
        "{VEC}function right(total : Vec[Int](n * 3)) : Vec[Int](n + n + n) = total"
    ));
    good(&format!(
        "{VEC}function nested(total : Vec[Int]((2 * n) * 3)) : Vec[Int](6 * n) = total"
    ));
    good(&format!(
        "{VEC}function zero(v : Vec[Int](0 * n)) : Vec[Int](0) = Nil\nfunction one(v : Vec[Int](1 * n)) : Vec[Int](n) = v"
    ));
    good(&format!(
        "{VEC}function accept(v : Vec[Int](2 * n)) : Int = 0\naccept(Cons(1,Cons(2,Nil)))\naccept(Nil)\naccept(Cons(1,Cons(2,Cons(3,Cons(4,Nil)))))"
    ));
    refuses(
        &format!("{VEC}function accept(v : Vec[Int](2 * n)) : Int = 0\naccept(Cons(1,Nil))"),
        "E_TYPE",
    );
    good(&format!(
        "{VEC}function dup(move xs : Vec[Int](n)) : Vec[Int](2 * n) = match xs {{Nil => Nil,Cons(x,rest) => Cons(x,Cons(x,dup(rest))),}}"
    ));
    refuses(
        &format!("{VEC}function wrong(total : Vec[Int](2 * n)) : Vec[Int](3 * n) = total"),
        "E_TYPE",
    );
}

#[test]
fn match_arms_keep_multiterm_and_scaled_length_equations() {
    for length in [
        "n + m",
        "2 * n + 3 * m",
        "n + m + 2",
        "18446744073709551615 * n + 18446744073709551615 * m",
    ] {
        let empty = if length.ends_with("+ 2") {
            ""
        } else {
            "Nil => Nil,"
        };
        good(&format!(
            "{VEC}function rebuild(move xs : Vec[T]({length}), a : Vec[T](n), b : Vec[T](m)) : Vec[T]({length}) = match xs {{{empty}Cons(x,rest) => Cons(x,rest),}}"
        ));
        refuses(
            &format!(
                "{VEC}function lost(move xs : Vec[T]({length}), a : Vec[T](n), b : Vec[T](m)) : Vec[T]({length}) = match xs {{{empty}Cons(_,rest) => rest,}}"
            ),
            "E_TYPE",
        );
    }
    // A second match must respect the tail length learned by the first one.
    good(&format!(
        "{VEC}function tails(move a : Vec[T](n + m),move b : Vec[T](n + m),naming : Vec[T](n)) : Vec[T](n + m) = match a {{Nil => b,Cons(x,rest) => match b {{Cons(_,more) => Cons(x,more),}},}}"
    ));
    refuses(
        &format!(
            "{VEC}function tails(move a : Vec[T](n + m),move b : Vec[T](n + m),naming : Vec[T](n)) : Vec[T](n + m) = match a {{Nil => b,Cons(x,rest) => match b {{Cons(_,more) => more,}},}}"
        ),
        "E_TYPE",
    );
}

#[test]
fn zero_total_refines_each_length_but_positive_totals_do_not_choose_a_split() {
    good(&format!(
        "{VEC}function zeroParts(move total : Vec[T](n + m),move a : Vec[T](n),move b : Vec[T](m)) : Vec[T](n) = match total {{Nil => b,Cons(_,_) => a,}}"
    ));
    refuses(
        &format!(
            "{VEC}function noChoice(move total : Vec[T](n + m),move a : Vec[T](n),move b : Vec[T](m)) : Vec[T](n) = match total {{Nil => b,Cons(_,_) => b,}}"
        ),
        "E_TYPE",
    );
    // Zero facts must be restored before checking the next arm and function.
    refuses(
        &format!(
            "{VEC}function after(move total : Vec[T](n + m),move a : Vec[T](n),move b : Vec[T](m)) : Vec[T](n) {{let ignored := match total {{Nil => 0,Cons(_,_) => 1,}}\nb}}"
        ),
        "E_TYPE",
    );
}

#[test]
fn several_length_equations_combine_without_choosing_fixed_names() {
    good(&format!(
        "{VEC}function exchange(move a : Vec[T](n + m),move b : Vec[T](n + p),move c : Vec[T](m),move d : Vec[T](p)) : Vec[T](m) = match a {{Nil => match b {{Nil => d,Cons(_,_) => c,}},Cons(_,_) => c,}}"
    ));
    refuses(
        &format!(
            "{VEC}function wrong(move a : Vec[T](n + m),move b : Vec[T](n + p),move c : Vec[T](m),move d : Vec[T](p)) : Vec[T](m) = match a {{Nil => match b {{Nil => d,Cons(_,_) => d,}},Cons(_,_) => c,}}"
        ),
        "E_TYPE",
    );
    // A positive total does not make both parts positive.
    refuses(
        &format!(
            "{VEC}function noChoice(a : Vec[T](n + m),b : Vec[T](n)) : Int = match a {{Nil => 0,Cons(_,_) => match b {{Cons(_,_) => 1,}},}}"
        ),
        "E_MATCH_EXHAUSTIVE",
    );
}

#[test]
fn later_arguments_determine_earlier_length_sums() {
    let source = format!(
        "{VEC}function total(v : Vec[Int](n + m),a : Vec[Int](n),b : Vec[Int](m)) : Int = 0\n"
    );
    good(&format!(
        "{source}total(Cons(1,Cons(2,Nil)),Cons(3,Nil),Cons(4,Nil))"
    ));
    refuses(
        &format!("{source}total(Cons(1,Cons(2,Nil)),Cons(3,Nil),Nil)"),
        "E_INDEX_UNSPECIFIED",
    );
    refuses(
        &format!(
            "{VEC}function ambiguous(v : Vec[Int](n + m)) : Int = 0\nambiguous(Cons(1,Cons(2,Nil)))"
        ),
        "E_INDEX_UNSPECIFIED",
    );
}

#[test]
fn accepted_branch_length_equalities_hold_for_concrete_natural_lengths() {
    use kestrel::types::{Index, Inference};
    let n = Index::Rigid("n".into());
    let m = Index::Rigid("m".into());
    let tail = Index::Variable("tail".into());
    let mut inference = Inference::default();
    assert!(inference.refine_index(
        &Index::Add(Box::new(n.clone()), Box::new(m.clone())),
        &Index::Add(Box::new(tail.clone()), Box::new(Index::Number(1))),
    ));
    let mut expressions = vec![];
    for a in 0..3 {
        for b in 0..3 {
            for c in 0..3 {
                for offset in 0..2 {
                    let index = [(&n, a), (&m, b), (&tail, c)].into_iter().fold(
                        Index::Number(offset),
                        |sum, (name, count)| {
                            Index::Add(
                                Box::new(sum),
                                Box::new(Index::Scale(count, Box::new(name.clone()))),
                            )
                        },
                    );
                    expressions.push((index, [a, b, c, offset]));
                }
            }
        }
    }
    for (left, a) in &expressions {
        for (right, b) in &expressions {
            let ty = |index: &Index| Type::Indexed(Box::new(Type::named("Vec")), index.clone());
            let equal = inference.clone().unify(&ty(left), &ty(right));
            let impossible = inference.impossible_lengths(left, right);
            if equal || impossible {
                for n in 0..5 {
                    for m in 0..5 {
                        if n + m > 0 {
                            let value =
                                |c: &[u64; 4]| c[0] * n + c[1] * m + c[2] * (n + m - 1) + c[3];
                            if equal {
                                assert_eq!(value(a), value(b), "{left} = {right} at n={n}, m={m}");
                            }
                            if impossible {
                                assert_ne!(value(a), value(b), "{left} != {right} at n={n}, m={m}");
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn scaled_lengths_stay_compact_and_report_unsupported_arithmetic() {
    good(&format!(
        "{VEC}function huge(total : Vec[Int](18446744073709551615 * (n + m))) : Vec[Int]((18446744073709551615 * n) + (18446744073709551615 * m)) = total"
    ));
    for index in ["n * m", "(n + 1) * (m + 2)", "n / 2", "n - 1"] {
        refuses(
            &format!("{VEC}function f(v : Vec[Int]({index})) : Int = 0"),
            "E_INDEX_COMPLEX",
        );
    }
    for index in [
        "2 * 18446744073709551615",
        "(18446744073709551615 * n) * 2",
        "(18446744073709551615 * n) + n",
        "18446744073709551615 * (n + 2)",
    ] {
        refuses(
            &format!("{VEC}function f(v : Vec[Int]({index})) : Int = 0"),
            "E_INDEX_RANGE",
        );
    }
}

#[test]
fn subclasses_stand_in_only_where_a_superclass_is_borrowed() {
    let family = "class Figure {\nnew() {}\npublic function area() : double = 0.0\n}\nclass Square extends Figure {\npublic final side : double\nnew(side : double) {this.side = side}\n@override public function area() : double = this.side * this.side\n}\nclass Circle {\nnew() {}\n}\n";
    good(&format!(
        "{family}function describe(f : Figure) : double = f.area()\nfunction grow(mut f : Figure) : void {{}}\nlet mut sq := Square(2.0)\nlet a := describe(sq)\ngrow(sq)\nlet b := describe(Square(1.0))\nlet c := sq.area()"
    ));
    for body in [
        "let f : Figure = Square(3.0)",
        "let all : List[Figure] = [Figure(), Square(3.0)]",
        "function make() : Figure = Square(1.0)",
    ] {
        let result = checked(&format!("{family}{body}"));
        assert!(
            result.diagnostics.iter().any(|d| d.code == "E_TYPE"
                && d.hint
                    .as_deref()
                    .is_some_and(|h| h.contains("Leave the type off"))),
            "{body}: {:#?}",
            result.diagnostics
        );
    }
    refuses(
        &format!("{family}function take(move f : Figure) : void {{}}\ntake(Square(1.0))"),
        "E_CALL_ARGUMENTS",
    );
    refuses(
        &format!(
            "{family}function describe(f : Figure) : double = f.area()\nlet x := describe(Circle())"
        ),
        "E_TYPE",
    );
    let result = checked(&format!("{family}let f : Figure = Circle()"));
    assert!(
        result.diagnostics.iter().any(|d| d.code == "E_TYPE"
            && !d
                .hint
                .as_deref()
                .is_some_and(|h| h.contains("Leave the type off"))),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn generic_superclasses_bind_declared_parameters_and_enforce_bounds() {
    good(
        "class Base[T] {public var value : T\nnew(move value : T) {this.value=value}\npublic function get() : T = this.value}\nclass Child[U] extends Base[U] {new(move value : U) {this.value=value}}\nlet c:=Child(7)\nlet n : Int = c.get()",
    );
    refuses(
        "class Base[T] {new() {}}\nclass Child[T] extends Base[Missing] {new() {}}",
        "E_UNKNOWN_TYPE",
    );
    good("class Base[T : Text] {new() {}}\nclass Child[T : Text] extends Base[T] {new() {}}");
    refuses(
        "class Base[T : Text] {new() {}}\nclass Child[T] extends Base[T] {new() {}}",
        "E_TRAIT_BOUND",
    );
}

#[test]
fn defaults_produce_values_and_lambdas_keep_their_own_returns() {
    refuses(
        "function f(n : Int = {return 1}) : Int = n",
        "E_DEFAULT_CONTROL",
    );
    refuses(
        "function f(n : Int = parseInt(\"bad\")?) : Int or Error = n",
        "E_DEFAULT_CONTROL",
    );
    good("function f(n : Int or Error = parseInt(\"bad\")) : Int or Error = n?");
    good("function f(g : Int -> Int = (n : Int) => {return n}) : Int = g(3)\nf()");
}

#[test]
fn a_final_field_may_hold_a_list_or_option_of_its_own_class() {
    good(
        "class Node {\npublic final children : List[Node]\npublic final next : Option[Node]\nnew(children : List[Node], move next : Option[Node]) {this.children = children\nthis.next = next}\n}\nclass Left {\npublic final right : Option[Right]\nnew() {this.right = None}\n}\nclass Right {\npublic final left : Option[Left]\nnew() {this.left = None}\n}",
    );
    good(
        "class Tree {\npublic final left : Option[Tree]\npublic final right : Option[Tree]\nnew(move l : Option[Tree], move r : Option[Tree]) {this.left = l\nthis.right = r}\n}\nlet t := Tree(None, None)",
    );
    refuses(
        "class Bag {\npublic var items : List[Int]\nnew() {this.items = []}\n}\nclass Holder {\npublic final bag : Bag\nnew(move bag : Bag) {this.bag = bag}\n}\nfunction both(move a : Bag, mut b : Bag) : void {}\nlet h := Bag()\nboth(h, h)",
        "E_BORROW_CONFLICT",
    );
}

#[test]
fn arithmetic_operators_on_classes_need_their_trait_and_method() {
    let point = "class P {\npublic final x : Int\nnew(x : Int) {this.x = x}\n";
    good(&format!(
        "{point}has Addable, Subtractable\npublic function add(o : P) : P = P(this.x + o.x)\npublic function add(n : Int) : P = P(this.x + n)\npublic function subtract(o : P) : P = P(this.x - o.x)\n}}\nlet a := P(1) + P(2)\nlet b := P(1) + 5\nlet c := a - b\nlet d : Int = (a + b).x"
    ));
    refuses(
        &format!("{point}}}\nlet a := P(1) + P(2)"),
        "E_OPERATOR_TRAIT",
    );
    refuses(
        &format!("{point}has Addable\n}}\nlet a := P(1)"),
        "E_TRAIT_METHOD",
    );
    refuses(
        &format!(
            "{point}has Divisible\npublic function divide(o : P) : P with Effects = P(this.x / o.x)\n}}\nlet a := P(1)"
        ),
        "E_TRAIT_METHOD",
    );
    good("let s := \"a\" + \"b\"\nlet n : Nat = 2\nlet m := n + 3\nlet x := 1 + 2 * 3");
}

#[test]
fn unicode_names_share_one_case_policy_and_preserve_exact_spelling() {
    good(include_str!("../testdata/language/identifiers.kes"));
    good("let ℘value := 1\nlet 数 := ℘value\nconst É_MAX : Int = 数");
    good("let lower_case := 1\nlet X_c := lower_case");
    refuses("class 数 {}", "E_NAMING");
    refuses("function ǅata() : Int = 1", "E_NAMING");
    refuses("const Émax : Int = 1", "E_NAMING");
    refuses("let café := 1\ncafé", "E_UNKNOWN_NAME");
}

#[test]
fn earlier_parameters_are_read_only_while_defaults_evaluate() {
    good("function f(mut a : Int, b : Int = a) : Int = b");
    refuses(
        "function change(mut a : Int) : Int = a\nfunction f(mut a : Int, b : Int = change(a)) : Int = b",
        "E_MUT_PARAMETER",
    );
}

const ERASED_MUTATION: &str = "trait Mutable {mut function bump() : void\nfunction current() : Int}\ntrait ReadOnly {function current() : Int}\nclass Tally {has Mutable, ReadOnly\nvar n : Int\nnew() {this.n=0}\npublic mut function bump() : void {this.n=this.n+1}\npublic function current() : Int=this.n}\n";

#[test]
fn dynamic_views_preserve_mutability_for_script_captures_and_final_fields() {
    for body in [
        "let value : Box[dyn Mutable]=box(Tally())\nfunction change() : void=value.bump()",
        "let value : Box[dyn Mutable]=box(Tally())\nfunction read() : Int=value.current()",
        "alias View = Box[dyn Mutable]\nlet value : View=box(Tally())\nfunction change() : void=value.bump()",
    ] {
        refuses(
            &format!("{ERASED_MUTATION}{body}"),
            "E_SCRIPT_MUTABLE_CAPTURE",
        );
    }
    refuses(
        &format!("{ERASED_MUTATION}class Holder {{final value : Box[dyn Mutable]}}"),
        "E_FINAL_TYPE",
    );
    good(&format!(
        "{ERASED_MUTATION}class Holder {{final value : Box[dyn ReadOnly]}}\nlet value : Box[dyn ReadOnly]=box(Tally())\nfunction read() : Int=value.current()\nread()"
    ));
    good(&format!(
        "{ERASED_MUTATION}let value : Box[dyn Mutable]=box(Tally())\nfunction change(mut value : Box[dyn Mutable]) : void=value.bump()\nchange(value)"
    ));
    refuses(
        "trait Source {type Item\nmut function next() : Item}\nclass Numbers {has Source\nvar n : Int\nnew() {this.n=0}\npublic mut function next() : Int {this.n=this.n+1\nthis.n}}\nlet source : Box[dyn Source(Item=Int)]=box(Numbers())\nfunction read() : Int=source.next()",
        "E_SCRIPT_MUTABLE_CAPTURE",
    );
}

#[test]
fn mutating_callables_and_hidden_cursors_are_mutable_script_values() {
    refuses(
        &format!(
            "{ERASED_MUTATION}let tally:=Tally()\nlet bump:=move () => tally.bump()\nfunction change() : void=bump()"
        ),
        "E_SCRIPT_MUTABLE_CAPTURE",
    );
    refuses(
        "let cursor:=[1,2].items()\nfunction read() : Option[Int]=cursor.next()",
        "E_SCRIPT_MUTABLE_CAPTURE",
    );
    good("let f:=move (n : Int) => n + 1\nfunction read() : Int=f(1)\nread()");
    good(
        "let sequence:=[1,2].map(n => n + 1)\nfunction read() : List[Int]=sequence.toList()\nread()",
    );
}

#[test]
fn constructors_cannot_expose_this_before_all_fields_are_initialized() {
    for expression in [
        "this.read()",
        "((this)).read()",
        "{let read:=this.read\nread()}",
        "(() => this.read())()",
        "inspect(this)",
        "inspect((this))",
        "{let view:=this\ninspect(view)}",
    ] {
        refuses(
            &format!(
                "function inspect(p : Point) : String=p.label\nclass Point {{public final label : String\nnew() {{let ignored:={expression}\nthis.label=\"ready\"}}\npublic function read() : String=this.label}}"
            ),
            "E_THIS_UNINITIALIZED",
        );
    }
    refuses(
        "class Point {public final label : String\nnew() {let ignored:=((this)).label\nthis.label=\"ready\"}}",
        "E_FIELD_UNINITIALIZED",
    );
    refuses(
        "class Point {public final n : Int\npublic final label : String\nnew() {this.n=3\nlet ignored:=this.read()\nthis.label=\"ready\"}\npublic function read() : Int=this.n}",
        "E_THIS_UNINITIALIZED",
    );
    good(
        "function inspect(p : Point) : String=p.label\nclass Point {public final n : Int\npublic final label : String\nnew() {((this)).n=3\nthis.label=((this)).n.text()\nlet ignored:=this.read()\nlet saved:=() => this.read()\nlet value:=saved()\nlet other:=inspect(this)}\npublic function read() : String=this.label}",
    );
}

#[test]
fn bigint_literal_contexts_preserve_numeric_type_boundaries() {
    good(
        "function value() : BigInt=1000000000000000000000+1\nlet n:=value()\nlet a:=n+2\nlet b:=(1+2)+n\nlet c:=n==1\nlet d:=1==n\nlet e:=parseBigInt(\"12\")\nlet f:=12.toBigInt()",
    );
    // An Int widens to BigInt beside a BigInt, so these are valid (Q125). BigInt never narrows to Int.
    good(
        "let n : BigInt=1\nlet small:=2\nlet sum:=n+small\nlet same:=n==small\nlet ordered:=small<n\nlet times:=small*n",
    );
    refuses("let n : BigInt=1\nlet small : Int = n", "E_TYPE");
    refuses(
        "let n : BigInt=1\nlet small:=2\nlet back : Int = n+small",
        "E_TYPE",
    );
    refuses("let n : BigInt=1\nlet small:=2\nn+2.5", "E_NUMERIC_TYPE");
    refuses("let n : BigInt=1\nn==1.0", "E_EQUALITY_TYPE");
    refuses("let n : BigInt=1\nn & 1", "E_TYPE");
    good("let same := 1==1.0");
}

#[test]
fn to_array_copies_copyable_items_and_refuses_move_only_elements() {
    good(
        "let list := [1, 2, 3]\nlet array := list.toArray()\nlet lazy := range(3).map(n => n * 2).toArray()\n",
    );
    refuses(
        "class Point {\n    public final x : Int\n    new(x : Int) { this.x = x }\n}\nlet points := [Point(1)]\nlet array := points.toArray()\n",
        "E_TO_ARRAY",
    );
}

#[test]
fn array_append_grows_a_mutable_array_and_refuses_live_borrows() {
    good("let mut xs : Array[Int] = Array(0, 0)\nxs.append(3)\nxs.append(4)\n");
    refuses(
        "class Point {\n    public final x : Int\n    new(x : Int) { this.x = x }\n}\nlet mut points : Array[Point] = Array.generate(1, i => Point(i))\nlet first := points.get(0)\npoints.append(Point(2))\nmatch first {\n    Some(p) => print(p.x),\n    None => print(0),\n}\n",
        "E_BORROW_CONFLICT",
    );
}

#[test]
fn empty_arrays_take_their_element_type_from_later_use_or_an_annotation() {
    good("let xs := [].toArray()\nxs.append(3)\nprint(xs)\n");
    good("let xs : Array[Int] = [].toArray()\nprint(xs)\n");
    refuses("let xs := [].toArray()\n", "E_UNKNOWN_ELEMENT");
}

#[test]
fn arrays_are_mutable_values_that_change_through_a_plain_let() {
    good("let xs : Array[Int] = Array(0, 0)\nxs.append(3)\nxs[0] = 1\n");
    good("let m := MutMap[Int, Int]()\nm.put(1, 2)\n");
    refuses(
        "let xs : Array[Int] = Array(1, 1)\nxs = Array(2, 2)\n",
        "E_IMMUTABLE_ASSIGNMENT",
    );
    refuses(
        "function fill(xs : Array[Int]) : void {\n    xs.append(3)\n}\n",
        "E_MUT_PARAMETER",
    );
    good("function fill(mut xs : Array[Int]) : void {\n    xs.append(3)\n}\n");
    refuses(
        "class Bag {\n    public final items : Array[Int]\n    new() { this.items = Array(0, 0) }\n}\n",
        "E_FINAL_TYPE",
    );
}

#[test]
fn join_index_and_iteration_check_their_element_requirements() {
    good("let words := [\"a\", \"b\"].join(\"-\")\nlet found := [1, 2].index(2)\n");
    good("let xs : Array[Int] = Array(0, 0)\nfor (x in xs) {\n    print(x)\n}\n");
    refuses(
        "class Point {\n    public final x : Int\n    new(x : Int) { this.x = x }\n}\nlet s := [Point(1)].join(\",\")\n",
        "E_TRAIT_BOUND",
    );
}

#[test]
fn basic_methods_have_the_types_their_names_promise() {
    good(
        "let t : String = \"  a \".trim()\nlet e : Bool = [1].isEmpty()\nlet m : Option[Int] = [1, 2].min()\nlet n : Int = (3).min(9)\nlet b : Bool = \"ab\".startsWith(\"a\")\n",
    );
    refuses("let s : Int = \"a\".trim()\n", "E_TYPE");
    refuses("let s : Int = [1].first()\n", "E_TYPE");
}

#[test]
fn min_and_max_on_classes_give_views_and_need_comparable() {
    good(
        "class Shape {\n    has Comparable\n    public final side : Int\n    new(side : Int) { this.side = side }\n    public function compare(other : Shape) : Int = this.side - other.side\n}\nlet shapes := [Shape(3)]\nlet top : Option[&Shape] = shapes.max()\n",
    );
    refuses(
        "class Plain {\n    public final side : Int\n    new(side : Int) { this.side = side }\n}\nlet plain := [Plain(3)]\nlet top := plain.max()\n",
        "E_TRAIT_BOUND",
    );
}

#[test]
fn question_mark_follows_the_option_or_error_return_type() {
    good(
        "function half(n : Int) : Option[Int] = if (n % 2) == 0 { Some(n / 2) } else { None }\nfunction quarter(n : Int) : Option[Int] {\n    let h := half(n)?\n    return Some(h)\n}\n",
    );
    refuses(
        "function half(n : Int) : Option[Int] = if (n % 2) == 0 { Some(n / 2) } else { None }\nfunction bad(n : Int) : Int {\n    let h := half(n)?\n    return h\n}\n",
        "E_PROPAGATION_RETURN",
    );
    refuses(
        "function half(n : Int) : Option[Int] = if (n % 2) == 0 { Some(n / 2) } else { None }\nfunction bad(n : Int) : Int or Error {\n    let h := half(n)?\n    return h\n}\n",
        "E_PROPAGATION_OPTION",
    );
}

#[test]
fn question_mark_reads_an_option_field_as_a_borrow() {
    good(
        "class Node {\npublic var left : Option[Node]\nnew(move left : Option[Node]) { this.left = left }\npublic function hasLeftChild() : Option[Bool] {\n    let child := this.left?\n    return Some(child.left.isSome())\n}\n}\n",
    );
}

#[test]
fn records_declare_read_only_fields_and_need_typed_fields() {
    refuses(
        "record Point(x : Int, y : Int)\nlet p := Point(1, 2)\np.x = 3",
        "E_FINAL_ASSIGNMENT",
    );
    refuses("record Point(x : Int, y)", "E_RECORD_FIELD");
    good("record Point(x : Int, y : Int)\nlet p := Point(1, 2)\nlet total : Int = p.x + p.y");
}

#[test]
fn empty_list_literals_take_the_type_they_are_passed_to() {
    // A field type reaches the empty list, including through another record, and
    // so does a function parameter. Nobody has to annotate the brackets.
    good(
        "record Crate(items : List[Int])\nrecord Duo(left : Crate, right : Crate)\nfunction takes(xs : List[Int]) : Int = xs.length()\nlet p := Duo(Crate([]), Crate([]))\nprint(takes([]))\nprint(p.left.items.length())",
    );
    // With no field or parameter to take the type from, the list stays unknown.
    refuses("let xs := []", "E_TYPE_UNKNOWN");
}

#[test]
fn map_get_with_a_default_returns_the_value_type() {
    good("let m := Map(\"a\"=>1)\nlet v : Int = m.get(\"a\", 0)");
    good("let m := MutMap[String, Int]()\nlet v : Int = m.get(\"a\", 0)");
    refuses(
        "let m := Map(\"a\"=>1)\nlet v : Option[Int] = m.get(\"a\", 0)",
        "E_TYPE",
    );
}

#[test]
fn tuple_index_and_destructuring_refuse_bad_positions_and_targets() {
    refuses("let t := (1, 2)\nprint(t[2].text())", "E_TUPLE_INDEX");
    refuses(
        "let mut a := 0\nlet mut b := 0\nlet mut c := 0\n(a, b, c) = (1, 2)",
        "E_DESTRUCTURE_ARITY",
    );
    refuses(
        "let a := 1\nlet b := 2\n(a, b) = (b, a)",
        "E_IMMUTABLE_ASSIGNMENT",
    );
    good("let mut a := 1\nlet mut b := 2\n(a, b) = (b, a)");
}

#[test]
fn string_methods_refuse_arguments_of_the_wrong_type() {
    refuses(
        "print(\"abc\".split(1).length().text())",
        "E_CALL_ARGUMENTS",
    );
    refuses("print(\"abc\".zfill(\"x\"))", "E_CALL_ARGUMENTS");
    refuses("print(\"abc\".find(3).text())", "E_CALL_ARGUMENTS");
    refuses("print(\"abc\".center(\"x\", \"*\"))", "E_CALL_ARGUMENTS");
}

#[test]
fn sum_is_for_numbers_only() {
    refuses(
        "let xs := [\"a\"]\nprint(xs.sum().text())",
        "E_UNKNOWN_MEMBER",
    );
}

#[test]
fn tensor_static_shapes_slices_and_generic_signatures() {
    good(
        r#"
function product(a : Tensor[double, (m, k)], b : Tensor[double, (k, n)]) : Tensor[double, (m, n)] = a @ b
let a := Tensor.generate((5, 6), (i, j) => (i * 6 + j).toDouble())
let z : Tensor[double, (2, 3)] = Tensor.zeros((2, 3))
let o := Tensor.onesLike(z)
let f := Tensor.filled((2, 3), 0.5)
let s : Tensor[double, (2, 2)] = a[1:3, 2:4]
let row : Tensor[double, (6,)] = a[1, :]
let tail : Tensor[double, (3, 2)] = a[2:, :2]
let i := 1
let j := 3
let t : DynTensor[double] = a[i:j, 0:2]
let p : Tensor[double, (5, 6)] = a * a
let q := 2.0 * a
let r := a * 2.0
let b := Tensor.zeros((6, 4))
let m : Tensor[double, (5, 4)] = product(a, b)
let w : Tensor[double, (4, 2, 3)] = z.broadcastTo((4, 2, 3))
let x : double = a[1, 2]
a[1, 2] = 9.0
"#,
    );
}

#[test]
fn tensor_checker_refuses_static_mismatch_rank_elements_and_unsupported_operators() {
    for (source, code) in [
        (
            "let a := Tensor.zeros((2, 3))\nlet b := Tensor.zeros((3, 2))\nlet p := a * b",
            "E_TENSOR_SHAPE",
        ),
        (
            "let a := Tensor.zeros((2, 3))\nlet b := Tensor.zeros((2, 4))\nlet p := a @ b",
            "E_TENSOR_SHAPE",
        ),
        (
            "let a := Tensor.zeros((2,))\nlet p := a @ a",
            "E_TENSOR_RANK",
        ),
        (
            "let a := Tensor.zeros((2, 3))\nlet x := a[1]",
            "E_TENSOR_RANK",
        ),
        (
            "let a : Tensor[Int, (2,)] = Tensor.zeros((2,))",
            "E_TENSOR_ELEMENT",
        ),
        (
            "let a := Tensor.zeros((2, 3))\nlet b := a.broadcastTo((2, 4))",
            "E_TENSOR_BROADCAST",
        ),
        (
            "let a := Tensor.zeros((2, 3))\nlet b := a / a",
            "E_TENSOR_OPERATOR",
        ),
        ("let a := Tensor.filled((2, 3), true)", "E_TYPE"),
    ] {
        refuses(source, code);
    }
}

#[test]
fn tensor_ownership_moves_borrows_and_mutable_fields_follow_array_rules() {
    good(
        "function read(a : Tensor[double, (2, 3)]) : double = a[0, 0]\nfunction set(mut a : Tensor[double, (2, 3)]) : void { a[0, 0] = 5.0 }\nlet a := Tensor.zeros((2, 3))\nlet b := a.copy()\nset(a)\nlet x := read(a)\na[0, 0] = x",
    );
    refuses(
        "let a := Tensor.zeros((2, 3))\nlet b := a\nlet x := a[0, 0]",
        "E_USE_AFTER_MOVE",
    );
    refuses(
        "function set(a : Tensor[double, (2, 3)]) : void { a[0, 0] = 5.0 }",
        "E_MUT_PARAMETER",
    );
    refuses(
        "class Holder { public final value : Tensor[double, (2, 3)] }",
        "E_FINAL_TYPE",
    );
    refuses(
        "class Holder { public final value : DynTensor[double] }",
        "E_FINAL_TYPE",
    );
    refuses(
        "let a := Tensor.zeros((2, 3))\na[:, :] = Tensor.zeros((2, 3))",
        "E_TENSOR_SLICE_ASSIGN",
    );
}

#[test]
fn tensor_scalar_operator_traits_and_shape_erasure() {
    good(
        r#"
function scale[T : Multipliable[double](Output = R)](value : T) : R = value * 2.0
function rightScale[T : RightMultipliable[double](Output = R)](value : T) : R = 2.0 * value
let a := Tensor.zeros((2, 3))
let b : Tensor[double, (2, 3)] = scale(a)
let c : Tensor[double, (2, 3)] = rightScale(a)
let d : DynTensor[double] = a.copy()
let e : DynTensor[double] = Tensor.zeros((1,))
"#,
    );
    refuses(
        "let a := Tensor.zeros((2, 3))\nlet b : DynTensor[double] = a\nlet c := a[0, 0]",
        "E_USE_AFTER_MOVE",
    );
    refuses(
        "let n := 2\nlet a := Tensor.zeros((n, 3))\nlet b : Tensor[double, (2, 3)] = a",
        "E_TYPE",
    );
}

#[test]
fn tensors_satisfy_text() {
    good(
        "let a := Tensor.zeros((2, 3))\nfunction show[T : Text](x : T) : String = x.text()\nprint(show(a))\nprint(a.text())",
    );
    good("let d := Tensor.fromArray(Array(2, 1.0))\nprint(d)");
}

#[test]
fn a_trait_that_names_iterator_must_give_its_item_type() {
    refuses(
        "trait Ticker { has Iterator\npublic function label() : String }",
        "E_ASSOCIATED_INFER",
    );
    good(
        "trait Ticker { has Iterator(Item = Int)\npublic function label() : String }\nclass Count { has Ticker\npublic var n : Int\nnew() { this.n = 0 }\npublic function label() : String = \"count\"\npublic mut function next() : Option[Int] { this.n = this.n + 1\nif this.n < 3 then Some(this.n) else None } }\nlet t : Box[dyn Ticker] = box(Count())\nprint(t.next())",
    );
}

#[test]
fn transpose_checks_its_axes_against_the_shape() {
    good(
        "let a := Tensor.zeros((2, 3, 4))\nlet b : Tensor[double, (4, 3, 2)] = a.transpose()\nlet c : Tensor[double, (3, 2, 4)] = a.transpose((1, 0, 2))",
    );
    refuses(
        "let a := Tensor.zeros((2, 3))\nlet b := a.transpose((0, 0))",
        "E_TENSOR_AXES",
    );
    refuses(
        "let a := Tensor.zeros((2, 3))\nlet b := a.transpose((0, 1, 2))",
        "E_TENSOR_RANK",
    );
    refuses(
        "let a := Tensor.zeros((2, 3, 4))\nlet b : Tensor[double, (2, 3, 4)] = a.transpose()",
        "E_TYPE",
    );
}

#[test]
fn tensor_add_requires_equal_shapes() {
    refuses(
        "let a := Tensor.zeros((2, 3))\nlet b := a + Tensor.zeros((3, 2))",
        "E_TENSOR_SHAPE",
    );
    refuses(
        "let a := Tensor.zeros((2, 3))\nlet b := a - Tensor.zeros((1, 3))",
        "E_TENSOR_SHAPE",
    );
    good("let a := Tensor.zeros((2, 3))\nlet b := a + 1");
}

#[test]
fn tensor_reduction_axis_out_of_range_is_refused() {
    for axis in ["2", "-1", "99"] {
        refuses(
            &format!("let a := Tensor.zeros((2, 3))\nlet b := a.sum(axis: {axis})"),
            "E_TENSOR_AXES",
        );
    }
    good(
        "let a := Tensor.zeros((2, 3))\nlet s : double = a.sum()\nlet c : Tensor[double, (3,)] = a.sum(axis: 0)\nlet row : Tensor[double, (2,)] = a.mean(axis: 1)\nlet axis := 1\nlet d : DynTensor[double] = a.max(axis: axis)\nlet v := Tensor.ones((3,))\nlet scalar : double = v.sum(axis: axis)",
    );
    good("let n := 2\nlet a := Tensor.zeros((n, 3))\nlet b : DynTensor[double] = a.sum(axis: 1)");
}

#[test]
fn tensor_map_refuses_a_lambda_that_returns_int() {
    refuses(
        "let a := Tensor.ones((2, 3))\nlet n : Int = 1\nlet b := a.map(x => n)",
        "E_TYPE",
    );
    refuses(
        "function mapped() : Tensor[double, (2,)] = Tensor.ones((2,)).map(x => { print(x)\nx })",
        "E_EFFECT_REQUIRED",
    );
}

#[test]
fn reshape_element_count_mismatch_is_refused() {
    refuses(
        "let a := Tensor.ones((2, 3))\nlet b := a.reshape((3, 3))",
        "E_TENSOR_SHAPE",
    );
    good(
        "let a := Tensor.ones((2, 3))\nlet b : Tensor[double, (3, 2)] = a.reshape((3, 2))\nlet n := 3\nlet d : DynTensor[double] = a.reshape((n, 2))",
    );
}

#[test]
fn tensor_reshape_compares_large_literal_products_exactly() {
    refuses(
        "function bad(a : Tensor[double, (9223372036854775807, 9223372036854775807, 9223372036854775807)]) : Tensor[double, (1,)] = a.reshape((1,))",
        "E_TENSOR_SHAPE",
    );
    good(
        "function same(a : Tensor[double, (9223372036854775807, 9223372036854775807, 9223372036854775807)]) : Tensor[double, (1, 9223372036854775807, 9223372036854775807, 9223372036854775807)] = a.reshape((1, 9223372036854775807, 9223372036854775807, 9223372036854775807))",
    );
}

#[test]
fn conversion_constructor_types_and_checks() {
    good(
        "let a : Tensor[double, (3,)] = Tensor([1.0, 2.0, 3.0])\nlet b : Tensor[double, (1, 3)] = Tensor([1.0, 2.0, 3.0], shape = (1, 3))\nlet xs := [1.0, 2.0]\nlet d : DynTensor[double] = Tensor(xs)\nlet empty : Array[Int] = Array([])\nlet n : double = double(3)\nlet text : String = String(true)",
    );
    refuses(
        "let a := Tensor([1.0, 2.0], shape = (2, 2))",
        "E_TENSOR_SHAPE",
    );
    good("let a := Tensor([1, 2])");
    refuses("let a := Tensor([true, false])", "E_TYPE");
    refuses("let a := double(\"1.5\")", "E_CONVERSION");
    refuses(
        "class Item { new() {} }\nlet a := Array([Item()])",
        "E_TO_ARRAY",
    );
}

#[test]
fn tensor_shapes_accept_int_arrays_but_refuse_other_element_types() {
    good(
        "let X := Tensor([1.0, 2.0], shape = (1, 2))\nlet Shape : Array[Int] = X.shape()\nlet A : DynTensor[double] = X.broadcastTo(Shape)\nlet B : DynTensor[double] = X.reshape(Shape)\nlet C : DynTensor[double] = Tensor.zeros(Shape)\nlet D : DynTensor[double] = Tensor([1.0, 2.0], shape = Shape)\nlet E : DynTensor[double] = X.transpose(Array([1, 0]))\nlet F : DynTensor[double] = Tensor.generate(Shape, (I, J) => double(I + J))",
    );
    for expression in [
        "Tensor.zeros(Array([1.0]))",
        "Tensor([1.0], shape = Array([1.0]))",
        "Tensor.ones((1,)).reshape(Array([1.0]))",
    ] {
        refuses(&format!("let x := {expression}"), "E_TENSOR_SHAPE");
    }
    refuses(
        "let x := Tensor.ones((1,)).transpose(Array([0.0]))",
        "E_TENSOR_AXES",
    );
    refuses(
        "let x := Tensor.ones((1,)).transpose((0.0,))",
        "E_TENSOR_AXES",
    );
}

#[test]
fn tensor_division_by_double_preserves_shape_and_provides_divisible() {
    good(
        "let X := Tensor.ones((2, 3))\nlet Y : Tensor[double, (2, 3)] = X / 2.0\nlet Z : Tensor[double, (2, 3)] = X.divide(2.0)\nlet W : Tensor[double, (2, 3)] = X / 2\nlet V : Tensor[double, (2, 3)] = X.divide(2)\nlet dims := X.shape()\nlet D := Tensor.ones(dims)\nlet E : DynTensor[double] = D / 2.0\nfunction halve[T : Divisible[double](Output = R)](value : T) : R = value / 2.0\nlet H : Tensor[double, (2, 3)] = halve(X)",
    );
    for expression in ["2.0 / X", "X / X"] {
        refuses(
            &format!("let X := Tensor.ones((2, 3))\nlet Y := {expression}"),
            "E_TENSOR_OPERATOR",
        );
    }
    refuses(
        "let X := Tensor.ones((2, 3))\nlet Y := X.divide(\"2\")",
        "E_CALL_ARGUMENTS",
    );
}

#[test]
fn numerical_imports_static_shapes_and_errors() {
    good(
        "let a := Tensor.ones((3, 2))\nlet x := Tensor.ones((2,))\nlet y : Tensor[double, (3,)] = a @ x\nlet z : Tensor[double, (2,)] = y @ a",
    );
    good(
        "import std.linalg\nimport std.math\nimport std.stats\nlet a := Tensor.ones((3, 2))\nlet p : Tensor[double, (2, 3)] = linalg.pinv(a)\nlet e : Tensor[double, (3, 2)] = math.exp(a)\nlet qr := linalg.qr(a)\nlet q : Tensor[double, (3, 2)] = qr.q.copy()\nlet svd := linalg.svd(a)\nlet s : Tensor[double, (2,)] = svd.s.copy()\nlet v : Tensor[double, (2,)] = stats.variance(a, axis = 0)",
    );
    refuses("let x := exp(1.0)", "E_UNKNOWN_NAME");
    good(
        "import std.math\nlet x : double = math.sqrt(4.0)\nlet y : double = math.abs(-1.0)\nlet z : double = math.round(1.5)",
    );
    good(
        "import std.math.{exp, sin}\nlet x : double = exp(1.0)\nlet y : Tensor[double, (2,)] = exp(Tensor.ones((2,)))\nlet z : Tensor[double, (2,)] = sin(Tensor.ones((2,)))",
    );
    refuses(
        "let x := solve(Tensor.ones((2, 2)), Tensor.ones((2,)))",
        "E_UNKNOWN_NAME",
    );
    refuses(
        "import std.stats\nlet x := stats.covariance(Tensor.ones((3,)))",
        "E_LINALG_RANK",
    );
    refuses(
        "import std.random.{Rng}\nlet rng := Rng.seeded(1)\nlet x := rng.normal(0.0)",
        "E_CALL_ARGUMENTS",
    );
    refuses(
        "import std.linalg\nlet x := linalg.inverse(Tensor.ones((2, 3)))",
        "E_LINALG_SHAPE",
    );
    refuses(
        "import std.linalg\nlet x := linalg.dot(Tensor.ones((2,)), Tensor.ones((3,)))",
        "E_LINALG_SHAPE",
    );
    refuses(
        "import std.linalg\nlet x := linalg.solve(Tensor.ones((2, 2)), Tensor.ones((3,)))",
        "E_LINALG_SHAPE",
    );
    refuses(
        "import std.linalg\nlet x := linalg.qr(Tensor.ones((2,)))",
        "E_LINALG_RANK",
    );
}

#[test]
fn numerical_shapes_and_mutation_permissions_remain_checked() {
    good(
        "import std.linalg\nimport std.random.{Rng}\nimport std.prob.{Normal}\nlet A := Tensor.ones((3, 2))\nlet factor := linalg.lu(A)\nlet pivots : Array[Int] = Array(factor.pivots)\nfunction draw(mut rng : Rng) : double = rng.normal(0.0, 1.0)\nfunction sample(normal : Normal, mut rng : Rng) : double = normal.sample(rng)",
    );
    refuses(
        "import std.linalg\nlet factor := linalg.lu(Tensor.ones((2, 2)))\nlet x := factor.solve(Tensor.ones((3,)))",
        "E_LINALG_SHAPE",
    );
    refuses(
        "import std.random.{Rng}\nfunction draw(rng : Rng) : double = rng.normal(0.0, 1.0)",
        "E_MUT_PARAMETER",
    );
    refuses(
        "import std.random.{Rng}\nlet rng := Rng.seeded(42)\nprint(rng.state)",
        "E_MEMBER_PRIVATE",
    );
}

#[test]
fn complex_class_traits_methods_and_final_fields_are_checked() {
    good(
        "import std.complex.{Complex}\nlet z := Complex(3.0, 4.0)\nlet a : Complex = z + z\nlet b : Complex = 2.0 + z\nlet c : Complex = z * 2.0\nlet d : Complex = 2.0 / z\nlet e : double = z.abs()\nlet f : double = z.arg()\nlet g : Complex = z.conjugate()\nlet h : Bool = z == Complex(3.0, 4.0)\nlet i : String = z.text()\nlet j : Complex = Complex.i()\nlet k : double = z.re
let l : Complex = -z
let m : Complex = z ** 2
let n : Complex = z.power(2)",
    );
    refuses(
        "import std.complex.{Complex}\nlet z := Complex(3.0, 4.0)\nz.re = 1.0",
        "E_FINAL_ASSIGNMENT",
    );
    refuses(
        "import std.complex.{Complex}\nlet z := Complex(3.0, 4.0)\nz + \"x\"",
        "E_OPERATOR_TRAIT",
    );
    refuses(
        "import std.complex.{Complex}\nlet z := Complex(3.0)",
        "E_CALL_ARGUMENTS",
    );
    refuses("let z := Complex(3.0, 4.0)", "E_UNKNOWN_NAME");
    refuses(
        "import std.complex.{Complex}\nlet z := Complex(3.0, 4.0)\nlet q := z ** 2.0",
        "E_OPERATOR_TRAIT",
    );
}

#[test]
fn probability_static_api_shapes_effects_and_parameter_names_are_checked() {
    good(
        "import std.prob.{Normal, Bernoulli, Binomial}\nimport std.random.{Rng}\nlet rng := Rng.seeded(1)\nlet a : double = Normal.sample(mean = 0.0, sigma = 1.0)\nlet b : Int = Bernoulli.sample(p = 0.5)\nlet c : Tensor[double, (5,)] = Normal.sample(mean = 0.0, sigma = 1.0, shape = 5)\nlet d : Tensor[double, (2, 3)] = Normal.sample(mean = 0.0, sigma = 1.0, shape = (2, 3))\nlet e : DynTensor[double] = Normal.sample(mean = 0.0, sigma = 1.0, shape = Array([2, 3]))\nfunction draw(mut rng : Rng) : double = Normal.sample(rng, mean = 0.0, sigma = 1.0)\nlet f : double = draw(rng)",
    );
    refuses(
        "import std.prob.{Normal}\nfunction pure() : double = Normal.sample(mean = 0.0, sigma = 1.0)",
        "E_EFFECT_REQUIRED",
    );
    refuses(
        "import std.prob.{Normal}\nlet a := Normal.sample(mean = 0.0, sigma = 1.0, shape = -1)",
        "E_PROB_SHAPE",
    );
    refuses(
        "import std.prob.{Normal}\nlet a := Normal.sample(mean = 0.0)",
        "E_CALL_ARGUMENTS",
    );
    refuses(
        "import std.prob.{Normal}\nlet a := Normal.create(mean = 0.0, stddev = 1.0)",
        "E_CALL_ARGUMENTS",
    );
    refuses(
        "import std.prob.{Normal}\nlet a := Normal.create(0.0, 1.0)\nlet b := a.stddev",
        "E_UNKNOWN_MEMBER",
    );
}

#[test]
fn power_and_negation_operators_use_tensor_and_trait_rules() {
    good(
        "let A := Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2))\nlet v := Tensor([1.0, 2.0, 3.0], shape = (3,))\nlet a : Tensor[double, (2, 2)] = A ** 2\nlet b : Tensor[double, (2, 2)] = A ** 2.0\nlet c : Tensor[double, (2, 2)] = A.power(2)\nlet d : Tensor[double, (2, 2)] = -A\nlet e : Tensor[double, (2, 2)] = A.negate()\nlet f : Tensor[double, (3,)] = v ** 2",
    );
    good(
        "class MyBox {\n    public final x : Int\n    new(x : Int) { this.x = x }\n    has Negatable, Powerable\n    public function negate() : MyBox = MyBox(-this.x)\n    public function power(exponent : Int) : MyBox = MyBox(this.x ** exponent)\n}\nlet b := MyBox(3)\nlet c := -b\nlet d := b ** 2",
    );
    refuses(
        "let A := Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2))\nlet B := A ** \"2\"",
        "E_OPERATOR_TRAIT",
    );
    refuses(
        "class Plain {\n    public final x : Int\n    new(x : Int) { this.x = x }\n}\nlet p := Plain(1)\nlet q := -p",
        "E_OPERATOR_TRAIT",
    );
    refuses(
        "let A := Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2))\nlet B := A.power(\"2\")",
        "E_TENSOR_OPERATOR",
    );
}

#[test]
fn transpose_property_is_a_read_only_view_with_reversed_shape() {
    good(
        "let A := Tensor([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], shape = (2, 3))\nlet T := A.T\nlet a : double = T[0, 1]\nlet b := A.T\nlet c := A.T.T\nlet d : Tensor[double, (3, 2)] = A.T.copy()\nlet e : double = A.T.sum()\nlet f : Tensor[double, (3,)] = A.T @ Tensor([1.0, 2.0], shape = (2,))\nlet g : Tensor[double, (3, 2)] = A.T + A.T\nlet mut n := -A.T\nn[0, 0] = 9.0",
    );
    refuses(
        "let A := Tensor([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], shape = (2, 3))\nA.T[0, 0] = 9.0",
        "E_TENSOR_VIEW_WRITE",
    );
    refuses(
        "let A := Tensor([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], shape = (2, 3))\nlet T := A.T\nT[0, 0] = 9.0",
        "E_MUT_BORROW",
    );
    refuses(
        "let A := Tensor([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], shape = (2, 3))\nlet T := A.T\nlet B := A\nprint(T)",
        "E_BORROW_CONFLICT",
    );
    refuses(
        "let A := Tensor([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], shape = (2, 3))\nlet B : Tensor[double, (3, 2)] = A.T",
        "E_TYPE",
    );
    refuses(
        "function bad(t : Tensor[double, (2, 2)]) : DynTensor[double] = t.T",
        "E_VIEW_ESCAPE",
    );
}

#[test]
fn integer_literals_adopt_double_only_when_exactly_representable() {
    good(
        "let a : double = 1\nlet b : double = 1 + 2\nlet c : double = 1 / 2\nlet d : double = -1\nlet e : double = 0x10\nlet f : double = 9007199254740992\nlet g : double = 1e3 + 1\nlet h : double = 1 + 2.0\nlet i : double = 2.0 + 1\nlet j : Bool = 1 == 1.0\nlet k : Bool = 1 < 2.0\nlet xs : List[double] = [1, 2]\nfunction value(x : double) : double = x\nlet l : double = value(1)\nlet m : Int = match 1.0 {1 => 1, _ => 0}\nlet n : double = 18446744073709551616\nlet o : double = 0x10000000000000000",
    );
    refuses(
        "let a : double = 9007199254740993",
        "E_DOUBLE_LITERAL_EXACT",
    );
    refuses(
        "let a : double = 0x20000000000001",
        "E_DOUBLE_LITERAL_EXACT",
    );
    refuses(
        "let a : double = 18446744073709551615",
        "E_DOUBLE_LITERAL_EXACT",
    );
    refuses(
        "let a : double = 18446744073709551617",
        "E_DOUBLE_LITERAL_EXACT",
    );
    refuses("let n : Int = 1\nlet a : double = n", "E_TYPE");
    refuses(
        "let m : Int = match 1.0 {9007199254740993 => 1, _ => 0}",
        "E_DOUBLE_LITERAL_EXACT",
    );
}

#[test]
fn pipes_give_the_left_side_its_parameter_type() {
    good(
        "function f(v : double) : double = v
let a := 1 |> f
let b := (1) |> f(_)
function add(a : double, b : double) : double = a + b
let c := 1 |> add(_, 2.0)
let d := 1 |> add(2.0, _)
let e := 1 |> add(_, 2.0) |> f
function g(xs : List[double]) : Int = 0
let h := [1, 2] |> g
function t(pair : (double, Int)) : Int = 0
let i := (1, 2) |> t
function use(fn : (double) -> double) : double = fn(1.0)
let j := (x => x + 1) |> use
function id[T](v : T) : T = v
let k : double = 1 |> id
function over(v : double) : double = v
function over(v : String) : String = v
let l := 1 |> over",
    );
    refuses(
        "function f(v : double) : double = v
let n := 1
let x := n |> f",
        "E_CALL_ARGUMENTS",
    );
}

#[test]
fn tuple_items_take_the_expected_part_types() {
    good(
        "let n := 1
let a : (BigInt, Int) = (n, 2)
let b : (DynTensor[double], Int) = (Tensor([0, 0]), 1)
let c : ((BigInt, BigInt), Int) = ((n, 2), 3)
function f(pair : (BigInt, Int)) : Int = 0
let d := f((n, 2))
function g() : (DynTensor[double], Int) = (Tensor([0, 0]), 1)
let e : (double, Int) = (1, 2)
let ts : List[(BigInt, Int)] = [(n, 2)]",
    );
    refuses("let a : (BigInt, Int) = (\"a\", 2)", "E_TYPE");
    refuses("let a : (DynTensor[double], Int) = (1, 2)", "E_TYPE");
}

#[test]
fn machine_learning_models_fit_predict_and_score() {
    good(
        "import std.ml.{StandardScaler, MinMaxScaler, PolynomialFeatures, LinearRegression, KNeighborsClassifier, LogisticRegression, KMeans, PCA, trainTestSplit, foldIndices, meanSquaredError, meanAbsoluteError, r2Score, accuracy, confusionMatrix, precision, recall, f1Score}\nimport std.random.{Rng}\nlet X : DynTensor[double] = Tensor.ones((4, 2))\nlet y : DynTensor[double] = Tensor.ones((4,))\nlet scaler := StandardScaler.fit(X)\nlet scaled : DynTensor[double] = scaler.transform(X)\nlet direct : DynTensor[double] = StandardScaler.fitTransform(X)\nlet minimum : DynTensor[double] = MinMaxScaler.fitTransform(X)\nlet polynomial : DynTensor[double] = PolynomialFeatures.fitTransform(X)\nlet model := LinearRegression.fit(scaled, y)\nlet coefficients : DynTensor[double] = model.coefficients\nlet intercept : double = model.intercept\nlet predicted : DynTensor[double] = model.predict(scaled)\nlet mse : double = meanSquaredError(y, predicted)\nlet mae : double = meanAbsoluteError(y, predicted)\nlet r2 : double = r2Score(y, predicted)\nlet hit : double = accuracy(y, predicted)\nlet noIntercept := LinearRegression.fit(X, y, fitIntercept = false)\nlet neighbors := KNeighborsClassifier.fit(X, y, 1)\nlet neighborPrediction : DynTensor[double] = neighbors.predict(X)\nlet logistic := LogisticRegression.fit(X, y, 0.1, 10)\nlet logisticPrediction : DynTensor[double] = logistic.predict(X)\nlet clusters := KMeans.fit(X, 2, 10)\nlet clusterLabels : DynTensor[double] = clusters.predict(X)\nlet pca := PCA.fit(X, 1)\nlet components : DynTensor[double] = pca.transform(X)\nlet rng := Rng.seeded(1)\nlet (trainingX, testingX, trainingY, testingY) := trainTestSplit(X, y, 1, rng)\nlet (foldTrain, foldTest) := foldIndices(4, 2, 0)\nlet (trueNegative, falsePositive, falseNegative, truePositive) := confusionMatrix(y, y)\nlet p : double = precision(y, y)\nlet r : double = recall(y, y)\nlet f : double = f1Score(y, y)",
    );
    refuses(
        "let X : DynTensor[double] = Tensor.ones((2, 2))\nlet scaler := StandardScaler.fit(X)",
        "E_UNKNOWN_NAME",
    );
    refuses(
        "import std.ml.{LinearRegression}\nlet X : DynTensor[double] = Tensor.ones((2, 2))\nlet model := LinearRegression.fit(X)",
        "E_CALL_ARGUMENTS",
    );
}

#[test]
fn error_unions_pass_their_value_type_to_literals_and_collections() {
    good(
        "let a : double or Error = 1
function f() : double or Error = 1 + 1
function g() : Nat or Error = 1
let xs : List[double] or Error = [1, 2]
let t : (double, Int) or Error = (1, 2)
let q : Map[String, double] or Error = Map(\"a\" => 1)
let s : Set[double] or Error = Set(1, 2)
function h(b : Bool) : double or Error = match b { true => 1, false => 2.0 }
function i(x : double or Error) : double or Error = x",
    );
    good("function f() : double or Error = RuntimeError.Parse(\"x\")");
    refuses("let x : double or Error = \"a\"", "E_TYPE");
    refuses("let x : double or Error = Some(1)", "E_TYPE");
}

#[test]
fn nat_literals_fit_patterns_and_numeric_contexts() {
    good(
        "let n : Nat = 1
let a : Bool = match n { 1 => true, _ => false }
let b : Bool = n == 1
let c : Nat = n + 1
let d : Nat = match n { 0 => 0, _ => 1 }",
    );
    refuses(
        "let n : Nat = 1\nlet a : Bool = match n { -1 => true, _ => false }",
        "E_INTEGER_LITERAL_RANGE",
    );
}
