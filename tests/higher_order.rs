//! First-class functions in proof statements: parameters, application, lambdas,
//! quantification and opaque predicates over function types.
use kestrel::{checker::check, parse_file};

fn good(source: &str) {
    let parsed = parse_file("higher_order.kes", source);
    assert!(parsed.is_ok(), "{source}\n{:#?}", parsed.diagnostics);
    let checked = check(&parsed);
    assert!(checked.is_ok(), "{source}\n{:#?}", checked.diagnostics);
}

fn refuses(source: &str, code: &str) {
    let parsed = parse_file("higher_order.kes", source);
    let checked = check(&parsed);
    assert!(
        checked.diagnostics.iter().any(|d| d.code == code),
        "Expected {code}: {source}\n{:#?}",
        checked.diagnostics
    );
}

#[test]
fn function_parameters_are_applied_like_terms() {
    good("theorem self(f : Nat -> Nat, n : Nat) : f(n) = f(n) by { rfl }");
    good("theorem twice(f : Nat -> Nat, n : Nat) : f(f(n)) = f(f(n)) by { rfl }");
    good("theorem curried(f : Nat -> Nat -> Nat, n : Nat) : f(n)(n) = f(n)(n) by { rfl }");
}

#[test]
fn quantifiers_bind_function_types() {
    good(
        "theorem pointwise() : forall f : Nat -> Nat, forall n : Nat, f(n) = f(n) by {\n\
            intro f\n\
            intro n\n\
            rfl\n\
        }",
    );
    good(
        "theorem instantiate(f : Nat -> Nat) : (forall g : Nat -> Nat, g(Zero) = Zero) -> f(Zero) = Zero by {\n\
            intro h\n\
            exact h(f)\n\
        }",
    );
}

#[test]
fn lambdas_type_from_the_expected_function_type() {
    good(
        "opaque function isMax(f : (Int, Int) -> Int) : Prop\n\
         function max2(a : Int, b : Int) : Int = if a < b { b } else { a }\n\
         axiom maxIsMax() : isMax((a : Int, b : Int) => max2(a, b))\n\
         theorem t() : isMax((a : Int, b : Int) => max2(a, b)) by { apply maxIsMax }",
    );
    good("theorem annotated(n : Nat) : ((x : Nat) => x + 1)(n) = n + 1 by { rfl }");
}

#[test]
fn analysis_style_sequences_are_expressible() {
    good(
        "opaque type Real\n\
         opaque function converges(f : Nat -> Real, L : Real) : Prop\n\
         opaque function limit(f : Nat -> Real) : Real\n\
         axiom limitSpec(f : Nat -> Real) : converges(f, limit(f))\n\
         theorem limitConverges(f : Nat -> Real) : converges(f, limit(f)) by { apply limitSpec }\n\
         theorem cite(f : Nat -> Real) : (forall g : Nat -> Real, converges(g, limit(g))) -> converges(f, limit(f)) by {\n\
            intro h\n\
            exact h(f)\n\
         }",
    );
}

#[test]
fn function_types_ignore_parameter_names_but_not_types() {
    good("theorem a(f : (x : Nat) -> Nat, n : Nat) : f(n) = f(n) by { rfl }");
    good("theorem b(f : (Nat, Nat) -> Nat, x : Nat) : f(x, x) = f(x, x) by { rfl }");
    refuses(
        "theorem c(f : Nat -> Nat, g : Nat -> Bool) : (forall h : Nat -> Nat, h(Zero) = Zero) -> g(Zero) = g(Zero) by { intro h; exact h(g) }",
        "E_PROOF_TYPE",
    );
}

#[test]
fn opaque_functions_accept_lambdas_and_equations_apply_through_them() {
    good(
        "opaque function apply1(f : Nat -> Nat, n : Nat) : Nat\n\
         axiom apply1Id(f : Nat -> Nat, n : Nat) : apply1(f, n) = f(n)\n\
         theorem uses(n : Nat) : apply1((x : Nat) => x, n) = n by { apply apply1Id }",
    );
}
