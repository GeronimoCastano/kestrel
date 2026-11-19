//! Operator aliases and portable spellings for mathematical symbols.
use kestrel::checker::check;
use kestrel::parse_file;

fn checked(source: &str) -> kestrel::checker::Checked {
    let parsed = parse_file("operators.kes", source);
    assert!(parsed.is_ok(), "{source}\n{:#?}", parsed.diagnostics);
    check(&parsed)
}

fn good(source: &str) {
    let result = checked(source);
    assert!(result.is_ok(), "{source}\n{:#?}", result.diagnostics);
}

#[test]
fn operator_example_checks() {
    good(include_str!("../testdata/language/operators.kes"));
}

#[test]
fn a_custom_operator_is_an_unproved_alias_for_a_function() {
    good(
        "function add(a : Nat, b : Nat) : Nat = a + b\n\
         infixl 10 \"⊕\" as \\oplus = add\n\
         theorem t() : 2 ⊕ 3 = 2 \\oplus 3 by { rfl }\n",
    );
}

#[test]
fn right_associative_operators_group_to_the_right() {
    good(
        "function cat(a : Nat, b : Nat) : Nat = a + 1\n\
         infixr 10 \"⊕\" = cat\n\
         theorem t() : 1 ⊕ 2 ⊕ 3 = 2 by { rfl }\n",
    );
}

#[test]
fn word_spellings_of_set_symbols_are_rejected() {
    for source in [
        "theorem a[U](x : U, A : Set[U], h : x in A) : x in A by { exact h }\n",
        "theorem b[U](A : Set[U]) : A subset A by { intro x; intro h; exact h }\n",
    ] {
        let parsed = parse_file("operators.kes", source);
        assert!(!parsed.is_ok(), "{source}");
    }
}

#[test]
fn backslash_spellings_parse_without_unicode_keystrokes() {
    good(
        "theorem a[U](x : U, A : Set[U], h : x \\in A) : x \\in A by { exact h }\n\
         theorem b[U](A : Set[U]) : A \\subseteq A by { intro x; intro h; exact h }\n\
         theorem c(n : Nat) : n = n \\to n = n by { intro h; exact h }\n",
    );
}

#[test]
fn comparison_aliases_mean_the_same_operator() {
    good(
        "theorem a(n : Nat) : n >= 1 -> n \\ge 1 by { intro h; exact h }\n\
         theorem b(n : Nat) : n <= 1 -> n \\le 1 by { intro h; exact h }\n\
         theorem c(n : Nat) : n != 0 -> n \\ne 0 by { intro h; exact h }\n",
    );
}

#[test]
fn quantifier_and_connective_aliases_parse() {
    good(
        "theorem a(n : Nat) : (n = n) \\land (n = n) by { exact (rfl, rfl) }\n\
         theorem b(n : Nat) : (n = n) \\lor (n = n) by { left; rfl }\n\
         theorem c(n : Nat) : \\exists k : Nat, k = n by { exists n; rfl }\n",
    );
}

#[test]
fn unicode_connectives_parse() {
    good(
        "theorem a[U](x : U, A : Set[U], h : x ∈ A) : x ∈ A by { exact h }\n\
         theorem b[U](A : Set[U]) : ∀ x : U, x ∈ A → x ∈ A by { intro x; intro h; exact h }\n",
    );
}

#[test]
fn an_operator_must_name_a_two_parameter_function() {
    let parsed = parse_file("operators.kes", "infixl 10 \"⊕\" = missing\n");
    let result = check(&parsed);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "E_OPERATOR_FUNCTION"),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn a_builtin_operator_cannot_be_redeclared() {
    let parsed = parse_file(
        "operators.kes",
        "function f(a : Nat, b : Nat) : Nat = a + b\ninfixl 10 \"+\" = f\n",
    );
    let result = check(&parsed);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "E_OPERATOR_SYMBOL"),
        "{:#?}",
        result.diagnostics
    );
}
