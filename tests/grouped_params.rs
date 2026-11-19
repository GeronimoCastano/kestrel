//! `(a b : T)` is sugar for `(a : T, b : T)`.
use kestrel::checker::check;
use kestrel::parse_file;

fn checked(source: &str) -> kestrel::checker::Checked {
    let parsed = parse_file("grouped.kes", source);
    assert!(parsed.is_ok(), "{source}\n{:#?}", parsed.diagnostics);
    check(&parsed)
}

fn good(source: &str) {
    let result = checked(source);
    assert!(result.is_ok(), "{source}\n{:#?}", result.diagnostics);
}

#[test]
fn grouped_example_checks() {
    good(include_str!("../testdata/language/grouped.kes"));
}

#[test]
fn functions_can_group_parameters() {
    good(
        "function add2(a b : Nat) : Nat = a + b\n\
         function scale(x y : double) : double = x * y\n\
         function use() : Nat = add2(2, 3)\n",
    );
}

#[test]
fn theorems_and_examples_can_group_parameters() {
    good(
        "theorem same(a b : Nat) : a = a by { rfl }\n\
         example(A B : Set[Nat]) : A ⊆ A by { intro x; intro h; exact h }\n",
    );
}

#[test]
fn lambdas_can_group_parameters() {
    good(
        "function apply(f : (Int, Int) -> Int) : Int = f(2, 3)\n\
         function g() : Int = apply((x y : Int) => x + y)\n",
    );
}

#[test]
fn grouped_names_need_one_annotation() {
    let parsed = parse_file("grouped.kes", "function f(a b) : Nat = a + b\n");
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PARAMETER_TYPE"),
        "{:#?}",
        parsed.diagnostics
    );
}

#[test]
fn a_grouped_default_is_rejected() {
    let parsed = parse_file("grouped.kes", "function f(a b : Nat = 0) : Nat = a + b\n");
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.code == "E_PARAMETER_DEFAULT"),
        "{:#?}",
        parsed.diagnostics
    );
}
