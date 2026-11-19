use kestrel::{
    Parsed,
    checker::{Checked, check},
    parse_file,
    resolution::{Argument, Callable, DeclarationKind, ResolvedCall},
    syntax::SyntaxKind,
    types::Type,
};

fn checked(source: &str) -> (Parsed, Checked) {
    let parsed = parse_file("resolution.kes", source);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{source}\n{:#?}", checked.diagnostics);
    (parsed, checked)
}
fn call<'a>(parsed: &Parsed, checked: &'a Checked, expression: &str) -> &'a ResolvedCall {
    checked
        .resolved_calls
        .iter()
        .find_map(|(span, call)| (parsed.source.slice(*span) == expression).then_some(call))
        .unwrap_or_else(|| panic!("Missing call {expression}: {:?}", checked.resolved_calls))
}
fn declaration(call: &ResolvedCall, name: &str, kind: DeclarationKind) {
    assert!(
        matches!(&call.target, Callable::Declaration { name: found, kind: found_kind, .. } if found == name && found_kind == &kind),
        "{:?}",
        call.target
    );
}

#[test]
fn free_calls_aliases_and_overloads_keep_declaration_identity() {
    let (parsed, checked) = checked(
        "function twice(x : Int) : Int = x * 2\nfunction twice(x : double) : double = x * 2.0\nlet aliasValue := twice\nlet a := twice(2)\nlet b := aliasValue(2.0)",
    );
    let first = call(&parsed, &checked, "twice(2)");
    let second = call(&parsed, &checked, "aliasValue(2.0)");
    declaration(first, "twice", DeclarationKind::Function);
    declaration(second, "twice", DeclarationKind::Function);
    assert_ne!(first.target, second.target);
    assert_eq!(second.overload, 1);
    assert_eq!(second.signature.parameters[0].ty, Type::named("double"));
}

#[test]
fn generic_calls_record_concrete_and_enclosing_rigid_arguments() {
    let (parsed, checked) = checked(
        "function same(x : T) : T = x\nfunction outer(x : T) : T = same(x)\nlet value : Int = outer(3)",
    );
    assert_eq!(
        call(&parsed, &checked, "same(x)").type_arguments["T"],
        Type::Rigid("T".into())
    );
    assert_eq!(
        call(&parsed, &checked, "outer(3)").type_arguments["T"],
        Type::named("Int")
    );
}

#[test]
fn parameter_order_and_source_evaluation_order_are_distinct() {
    let (parsed, checked) =
        checked("function f(a : Int, b : Int, c : Int = 3) : Int = a + b + c\nf(b = 2, a = 1)");
    let selected = call(&parsed, &checked, "f(b = 2, a = 1)");
    assert_eq!(
        selected
            .evaluation_order
            .iter()
            .map(|span| parsed.source.slice(*span))
            .collect::<Vec<_>>(),
        ["2", "1"]
    );
    assert_eq!(
        selected
            .arguments
            .iter()
            .map(|argument| match argument {
                Argument::Supplied { expression, .. } => parsed.source.slice(*expression),
                Argument::Default => "default",
            })
            .collect::<Vec<_>>(),
        ["1", "2", "default"]
    );
}

#[test]
fn partial_application_and_lambda_calls_preserve_closure_locations() {
    let (parsed, checked) = checked(
        "function add(a : Int, b : Int) : Int = a + b\nlet later := add(1, _)\nlater(2)\nlet twice := x => x * 2\ntwice(3)",
    );
    assert!(call(&parsed, &checked, "add(1, _)").partial);
    let Callable::Closure(id) = &call(&parsed, &checked, "later(2)").target else {
        panic!("Expected a closure")
    };
    assert_eq!(parsed.source.slice(id.span), "add(1, _)");
    let Callable::Closure(id) = &call(&parsed, &checked, "twice(3)").target else {
        panic!("Expected a lambda")
    };
    assert_eq!(parsed.source.slice(id.span), "x => x * 2");
}

#[test]
fn function_parameters_and_runtime_function_choices_remain_indirect() {
    let (parsed, checked) = checked(
        "function one(x : Int) : Int = x + 1\nfunction two(x : Int) : Int = x + 2\nfunction apply(f : Int -> Int) : Int = f(1)\nlet mut changing := one\nchanging = two\nchanging(3)\nlet copied := changing\ncopied(4)\nlet chosen := if true then one else two\nchosen(5)",
    );
    for expression in ["f(1)", "changing(3)", "copied(4)", "chosen(5)"] {
        assert_eq!(
            call(&parsed, &checked, expression).target,
            Callable::Indirect,
            "{expression}"
        );
    }
}

#[test]
fn constructors_inherited_methods_and_static_methods_have_distinct_targets() {
    let (parsed, checked) = checked(
        "class Parent { public function value() : Int = 1 }\nclass Child extends Parent { new() {}\npublic static function unit() : Child = Child() }\nlet child := Child.unit()\nchild.value()",
    );
    declaration(
        call(&parsed, &checked, "Child()"),
        "new",
        DeclarationKind::Constructor,
    );
    declaration(
        call(&parsed, &checked, "Child.unit()"),
        "unit",
        DeclarationKind::StaticMethod,
    );
    let inherited = call(&parsed, &checked, "child.value()");
    declaration(inherited, "value", DeclarationKind::Method);
    assert!(
        matches!(&inherited.target, Callable::Declaration { owner: Some(owner), .. } if owner == "Parent")
    );
    assert_eq!(
        inherited.signature.receiver.as_deref(),
        Some(&Type::named("Child"))
    );
}

#[test]
fn trait_calls_retain_generic_or_dynamic_receiver_types() {
    let (parsed, checked) = checked(
        "trait Shape { function area() : double }\nfunction generic[S : Shape](s : S) : double = s.area()\nfunction dynamic(s : &dyn Shape) : double = s.area()",
    );
    let calls = checked
        .resolved_calls
        .iter()
        .filter(|(span, _)| parsed.source.slice(**span) == "s.area()")
        .map(|(_, call)| call)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    for selected in &calls {
        declaration(selected, "area", DeclarationKind::TraitMethod);
    }
    assert_eq!(
        calls[0].signature.receiver.as_deref(),
        Some(&Type::Rigid("S".into()))
    );
    assert!(
        matches!(calls[1].signature.receiver.as_deref(), Some(Type::Borrow(inner)) if matches!(inner.as_ref(), Type::Dynamic(_)))
    );
}

#[test]
fn enum_case_constructors_are_scoped_declarations() {
    let (parsed, checked) = checked(
        "enum First { Value(Int) }\nenum Second { Value(Bool) }\nlet first := First.Value(1)\nlet second := Second.Value(true)",
    );
    let first = call(&parsed, &checked, "First.Value(1)");
    let second = call(&parsed, &checked, "Second.Value(true)");
    declaration(first, "Value", DeclarationKind::Variant);
    assert_ne!(first.target, second.target);
}

#[test]
fn builtins_and_map_entries_have_typed_calls() {
    let (parsed, checked) = checked(
        "import std.math.{abs}\nlet key := \"a\"\nlet m := Map(key => 1, \"b\" => 2)\nlet v := m.get(key)\nabs(-1.0)\nprint(v)",
    );
    let entries = call(&parsed, &checked, "Map(key => 1, \"b\" => 2)");
    assert_eq!(
        entries.target,
        Callable::Builtin("std.collections.Map".into())
    );
    assert_eq!(entries.type_arguments["K"], Type::named("String"));
    assert_eq!(entries.arguments.len(), 4);
    assert_eq!(
        call(&parsed, &checked, "abs(-1.0)").target,
        Callable::Builtin("std.math.abs".into())
    );
    assert!(
        matches!(&call(&parsed, &checked, "m.get(key)").target, Callable::Builtin(name) if name.ends_with("Map.get"))
    );
}

#[test]
fn right_multiplication_records_receiver_and_left_first_evaluation() {
    let (parsed, checked) = checked(
        "class Value { has RightMultipliable[Int]\nnew() {}\npublic function rightMultiply(left : Int) : Int = left }\nlet v := Value()\nlet n := 2 * v",
    );
    let selected = call(&parsed, &checked, "2 * v");
    declaration(selected, "rightMultiply", DeclarationKind::Method);
    assert_eq!(parsed.source.slice(selected.receiver.unwrap()), "v");
    assert_eq!(
        selected
            .evaluation_order
            .iter()
            .map(|span| parsed.source.slice(*span))
            .collect::<Vec<_>>(),
        ["2", "v"]
    );
}

#[test]
fn all_explicit_calls_in_examples_have_a_selected_call_record() {
    for source in [
        include_str!("../testdata/language/rules.kes"),
        include_str!("examples/rules.kes"),
        include_str!("../testdata/language/mutation.kes"),
    ] {
        if source.contains("import testdata.language.rules") {
            continue;
        }
        let (parsed, checked) = checked(source);
        for node in parsed
            .root
            .descendants()
            .filter(|node| matches!(node.kind, SyntaxKind::Call | SyntaxKind::Pipe))
        {
            assert!(
                checked.resolved_calls.contains_key(&node.span),
                "Missing {}",
                parsed.source.slice(node.span)
            );
        }
    }
}
