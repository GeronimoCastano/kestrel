use kestrel::{
    Parsed,
    lexer::{TokenKind, lex},
    parse_file, parse_project,
    source::SourceFile,
    syntax::{Node, SyntaxKind as K},
};

fn good(source: &str) -> Parsed {
    let parsed = parse_file("test.kes", source);
    assert!(
        parsed.is_ok(),
        "{}",
        parsed
            .diagnostics
            .iter()
            .map(|d| d.render(&parsed.source))
            .collect::<String>()
    );
    parsed
}

fn has(parsed: &Parsed, kind: K) -> &Node {
    parsed
        .root
        .descendants()
        .find(|n| n.kind == kind)
        .unwrap_or_else(|| panic!("Missing {kind:?}: {}", parsed.root.dump(&parsed.source)))
}

#[test]
fn precedence_and_parentheses_remain_visible() {
    let parsed = good("let x := (a + b) * c - d / e");
    let binary = has(&parsed, K::Binary);
    assert_eq!(parsed.source.slice(binary.children[1].span), "-");
    assert_eq!(binary.children[0].kind, K::Binary);
    assert_eq!(binary.children[0].children[0].kind, K::Group);
    assert_eq!(
        parsed.source.slice(binary.children[2].children[1].span),
        "/"
    );
}

#[test]
fn java_c_common_operator_order_is_explicit() {
    let parsed = good("a || b && c | d ^ e & f == g < h << i + j * k");
    let mut node = &parsed.root.children[0];
    for operator in ["||", "&&", "|", "^", "&", "==", "<", "<<", "+", "*"] {
        assert_eq!(parsed.source.slice(node.children[1].span), operator);
        node = &node.children[2];
    }
}

#[test]
fn arithmetic_is_left_associative() {
    let parsed = good("a - b - c");
    let binary = has(&parsed, K::Binary);
    assert_eq!(binary.children[0].kind, K::Binary);
    assert_eq!(parsed.source.slice(binary.children[2].span), "c");
}

#[test]
fn pipe_is_loose_and_lambda_body_extends_right() {
    let parsed = good("xs |> x => x + 1 |> twice");
    let pipe = &parsed.root.children[0];
    assert_eq!(pipe.kind, K::Pipe);
    assert_eq!(pipe.children[2].kind, K::Lambda);
    assert_eq!(pipe.children[2].children.last().unwrap().kind, K::Pipe);
}

#[test]
fn partial_application_and_pattern_wildcards_differ() {
    let parsed = good(
        "function f(xs : Option[Int]) : Int = match xs { Some(_) => 1, _ => 0, }\nlet g := add(_, 1)",
    );
    assert_eq!(
        parsed
            .root
            .descendants()
            .filter(|n| n.kind == K::Hole)
            .count(),
        1
    );
    assert_eq!(
        parsed
            .root
            .descendants()
            .filter(|n| n.kind == K::PatternWildcard)
            .count(),
        2
    );
}

#[test]
fn function_block_and_record_literal_are_distinct() {
    let parsed = good(
        "function a() : Circle { Circle { r: 1.0 } }\nfunction b() : Circle = Circle { r: 2.0 }",
    );
    assert_eq!(
        parsed.root.children[0].children.last().unwrap().kind,
        K::Block
    );
    assert_eq!(
        parsed.root.children[1].children.last().unwrap().kind,
        K::Record
    );
}

#[test]
fn conditions_and_scrutinees_do_not_steal_body_braces() {
    let parsed = good(
        "if flag { work() } else { stop() }\nmatch None { None => 0, Some(x) => x, }\nwhile flag { stop() }",
    );
    has(&parsed, K::If);
    has(&parsed, K::Match);
    has(&parsed, K::While);
}

#[test]
fn lambda_annotations_and_capture_modes() {
    let parsed = good(
        "let f := move (x : Int, y : Int) : Int => { x + y }\nlet g := x : Int => x\nlet h := () => 1",
    );
    let lambda = has(&parsed, K::Lambda);
    assert_eq!(lambda.children[0].kind, K::Modifier);
    assert_eq!(parsed.source.slice(lambda.children[0].span), "move");
    assert_eq!(has(&parsed, K::LambdaParameters).children.len(), 2);
    has(&parsed, K::ReturnType);
}

#[test]
fn function_type_is_not_a_lambda_arrow() {
    let parsed = good("function apply(f : Int -> Int, x : Int) : Int = f(x)");
    has(&parsed, K::FunctionType);
    assert!(!parsed.root.descendants().any(|n| n.kind == K::Lambda));
}

#[test]
fn arrow_types_can_name_inputs_with_independent_passing_modes() {
    let parsed = good(
        "alias Update = (mut value : Counter, move payload : Payload) -> Int\nalias Mixed = (Int, right : Int) -> Int\nalias Nested = mut ((mut callback : () -> Int) -> Int)",
    );
    assert_eq!(
        parsed
            .root
            .descendants()
            .filter(|node| node.kind == K::Parameter)
            .count(),
        4
    );
    has(&parsed, K::MutableType);
}

#[test]
fn mutable_function_types_keep_their_scope_and_effects() {
    let parsed = good(
        "alias Counter = mut (() -> Int)\nfunction apply(mut f : Counter) : Int=f()\nfunction make() : mut (Int -> void with Effects) = x => print(x)",
    );
    assert_eq!(
        parsed
            .root
            .descendants()
            .filter(|node| node.kind == K::MutableType)
            .count(),
        2
    );
    has(&parsed, K::FunctionType);
    has(&parsed, K::Effects);
}

#[test]
fn defaults_and_named_arguments_keep_names() {
    let parsed = good(
        "function scaled(x : double, scale : double = 1.0) : double = x * scale\nscaled(2.0, scale = 3.0)",
    );
    let named = has(&parsed, K::NamedArgument);
    assert_eq!(parsed.source.slice(named.children[0].span), "scale");
}

#[test]
fn named_argument_order_reports_expression_and_hint() {
    let parsed = parse_file("test.kes", "scaled(scale = 3.0, 2.0)\nlet next := 1");
    assert_eq!(parsed.diagnostics[0].code, "E_ARGUMENT_ORDER");
    let rendered = parsed.diagnostics[0].render(&parsed.source);
    assert!(
        rendered.contains("2.0") && rendered.contains("hint:") && rendered.contains("test.kes:1:")
    );
    assert_eq!(parsed.root.children.len(), 2);
}

#[test]
fn map_arrows_and_call_lambdas_are_retained() {
    let parsed = good("let m := Map(\"a\" => 1, key => value)\nxs.map(x => x + 1)");
    assert_eq!(
        parsed
            .root
            .descendants()
            .filter(|n| n.kind == K::ArrowArgument)
            .count(),
        2
    );
    has(&parsed, K::Lambda);
}

#[test]
fn indexed_and_borrowed_generic_types() {
    let parsed = good(
        "alias Ref = &dyn Shape\nfunction head(v : Vec[T](n + 1)) : T = match v { Cons(x, _) => x, }\nlet c := Channel[Int]()",
    );
    for kind in [
        K::Alias,
        K::BorrowType,
        K::DynType,
        K::IndexedType,
        K::TypeApplication,
        K::BracketApplication,
    ] {
        has(&parsed, kind);
    }
}

#[test]
fn bounds_has_lists_and_associated_types() {
    let parsed = good(
        "class Box[T : Text + Equals] {\n has (Equals[T], Parser(Input = Bytes))\n final value : T\n}\nfunction f[T : Text + Equals](x : T) : T = x",
    );
    for kind in [K::TypeParameters, K::Bound, K::Has, K::AssociatedType] {
        has(&parsed, kind);
    }
}

#[test]
fn both_loop_forms_and_control_statements() {
    let parsed = good(
        "function loop(xs : List[Int]) : void {\nfor (let x : Int in xs) { if x == 0 { continue } }\nfor (let i : Int = 0; i < 3; i = i + 1) { if i == 2 { break } }\nreturn\n}",
    );
    for kind in [
        K::For,
        K::ForBinding,
        K::Assignment,
        K::Continue,
        K::Break,
        K::Return,
    ] {
        has(&parsed, kind);
    }
}

#[test]
fn comprehensions_and_destructuring_patterns() {
    let parsed = good(
        "let (age, name) := person\nlet [x, ..rest] := xs otherwise { return 0 }\nlet ys := [x * 2 for x in xs if x > 1]",
    );
    for kind in [
        K::PatternTuple,
        K::PatternList,
        K::PatternRest,
        K::Otherwise,
        K::Comprehension,
        K::Guard,
    ] {
        has(&parsed, kind);
    }
}

#[test]
fn resources_effects_and_error_propagation() {
    let parsed = good(
        "function line(path : String) : String or Error with Effects { using f := open(path)? { f.readLine()? } }\nunsafe { sqrt(2.0) }\nscope { spawn save(report) }",
    );
    for kind in [
        K::Effects,
        K::UnionType,
        K::Using,
        K::Propagate,
        K::Unsafe,
        K::Scope,
        K::Spawn,
    ] {
        has(&parsed, kind);
    }
}

#[test]
fn theorem_equality_does_not_become_assignment() {
    let parsed = good(
        "@simp\ntheorem t(n : Nat) : add(n, Zero) = n requires h : n > 0 { induction n { Zero => rfl, Succ(k) ih => ih, } }",
    );
    for kind in [
        K::Attribute,
        K::Theorem,
        K::Proposition,
        K::Requires,
        K::Proof,
        K::Induction,
    ] {
        has(&parsed, kind);
    }
    assert!(!parsed.root.descendants().any(|n| n.kind == K::Assignment));
}

#[test]
fn proposition_logic_is_header_only() {
    let parsed = good("theorem t(n : Nat) : n = n and not false or true { rfl }");
    assert_eq!(parsed.root.children[0].kind, K::Theorem);
    assert!(!parse_file("test", "let x := a = b").is_ok());
}

#[test]
fn proof_bodies_are_parsed_as_proof_terms() {
    let parsed = parse_file("test", "theorem t(n : Nat) : n = n { rfl(] }");
    assert!(!parsed.is_ok());
    let parsed =
        good("theorem t(n : Nat) : n = n requires a : n = n requires b : n = n { simp [a, b] }");
    has(&parsed, K::Simp);
    assert_eq!(
        parsed
            .root
            .descendants()
            .filter(|n| n.kind == K::Requires)
            .count(),
        2
    );
}

#[test]
fn formatted_string_expressions_and_specs_keep_absolute_spans() {
    let text = "let price := f\"cost {total:.2f} {p:?} {{braces}} {if ok then 1 else 0}\"";
    let parsed = good(text);
    let interpolations: Vec<_> = parsed
        .root
        .descendants()
        .filter(|n| n.kind == K::Interpolation)
        .collect();
    assert_eq!(interpolations.len(), 3);
    assert_eq!(
        parsed.source.slice(interpolations[0].children[0].span),
        "total"
    );
    assert_eq!(parsed.source.slice(has(&parsed, K::FormatSpec).span), ".2f");
    assert_eq!(interpolations[2].children[0].kind, K::If);
}

#[test]
fn formatted_strings_can_nest_and_contain_quotes_and_record_braces() {
    let parsed = good(r#"let a := f"{f"inner {x}"} {call("quoted")} {Circle { r: 1.0 }}""#);
    assert_eq!(
        parsed
            .root
            .descendants()
            .filter(|n| n.kind == K::FormatString)
            .count(),
        2
    );
    has(&parsed, K::Record);
}

#[test]
fn literal_braces_are_not_interpolations() {
    let parsed = good("f\"{{1, 2}}\"\n\"{plain}\"");
    assert!(
        !parsed
            .root
            .descendants()
            .any(|n| n.kind == K::Interpolation)
    );
}

#[test]
fn bad_format_fields_get_specific_diagnostics() {
    for (text, expected) in [
        ("f\"{x:bad}\"", "E_FORMAT_SPEC"),
        ("f\"oops }\"", "E_FORMAT_BRACE"),
        ("f\"{}\"", "E_EXPRESSION"),
        ("f\"{x\"", "E_UNCLOSED_STRING"),
    ] {
        let parsed = parse_file("test", text);
        assert!(
            parsed.diagnostics.iter().any(|d| d.code == expected),
            "{text}: {:?}",
            parsed.diagnostics
        );
    }
}

#[test]
fn lexer_keeps_comments_crlf_and_unicode_offsets() {
    let text = "// comment\r\nlet café := \"😀\"\r\n";
    let parsed = good(text);
    assert_eq!(parsed.reconstructed_source(), text);
    let name = parsed
        .root
        .descendants()
        .find(|n| n.kind == K::Name && parsed.source.slice(n.span) == "café")
        .unwrap();
    assert_eq!(parsed.source.position(name.span.start), (2, 5));
    assert!(parsed.tokens.iter().any(|t| t.kind == TokenKind::Comment));
}

#[test]
fn source_columns_count_characters() {
    let source = SourceFile::new("test", "α😀x\nsecond");
    assert_eq!(source.position("α😀".len()), (1, 3));
    assert_eq!(source.position("α😀x\n".len()), (2, 1));
    assert_eq!(source.line(1), "α😀x");
    let source = SourceFile::new("test", "one\rtwo\r\nthree\n");
    assert_eq!(source.position(4), (2, 1));
    assert_eq!(source.position(9), (3, 1));
}

#[test]
fn lexer_uses_longest_operator_and_distinguishes_dot_from_decimal() {
    let text =
        "a:=1.0 |> f(_, 7.checkedDivide(0)) // hi\n0xFF 1_000 => -> == != <= >= && || ** << >> ..";
    let lexed = lex(text);
    assert!(lexed.diagnostics.is_empty());
    let words: Vec<_> = lexed
        .tokens
        .iter()
        .filter(|t| !t.is_trivia() && t.kind != TokenKind::Eof)
        .map(|t| &text[t.span.range()])
        .collect();
    assert!(words.windows(3).any(|w| w == ["7", ".", "checkedDivide"]));
    assert!(words.contains(&"1.0") && words.contains(&"0xFF") && words.contains(&"**"));
}

#[test]
fn typed_bindings_accept_both_assignment_spellings() {
    let parsed =
        good("let a : Int := 1\nlet b : Int = 2\nfor (let i : Int := 0; i < 2; i = i + 1) {}");
    assert!(
        parsed
            .root
            .descendants()
            .filter(|node| node.kind == K::Binding)
            .count()
            >= 3,
        "{}",
        parsed.root.dump(&parsed.source)
    );
}

#[test]
fn constructor_fields_are_their_own_declaration() {
    let parsed = good(
        "class Counter {\nnew(start : Int) {\nfinal this.start : Int := start\nvar this.hits : Int := 0\n}\n}",
    );
    let fields = parsed
        .root
        .descendants()
        .filter(|node| node.kind == K::FieldDeclaration)
        .count();
    assert_eq!(fields, 2, "{}", parsed.root.dump(&parsed.source));
}

#[test]
fn numbers_are_not_converted_during_lexing() {
    good(
        "let huge : BigInt = 1_000_000_000_000_000_000_000\nlet limit : Int = 9223372036854775807",
    );
    for bad in ["0x", "0x_", "1__0", "10_"] {
        assert!(!lex(bad).diagnostics.is_empty(), "{bad}");
    }
}

#[test]
fn unterminated_string_recovers_on_next_line() {
    let parsed = parse_file("test", "let broken := \"oops\nfunction good() : Int = 1");
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.code == "E_UNCLOSED_STRING")
    );
    assert!(parsed.root.children.iter().any(|n| n.kind == K::Function));
}

#[test]
fn unterminated_interpolation_does_not_swallow_the_next_declaration() {
    let parsed = parse_file(
        "test",
        "let broken := f\"oops {value\"\nfunction good() : Int = 1",
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.code == "E_UNCLOSED_STRING")
    );
    assert!(parsed.root.children.iter().any(|n| n.kind == K::Function));
}

#[test]
fn parser_reports_multiple_errors_and_keeps_later_declarations() {
    let parsed = parse_file(
        "test",
        "let first : Int = )\nlet second : = 3\nfunction good() : Int = 7\n",
    );
    assert!(parsed.diagnostics.len() >= 2);
    assert!(parsed.root.children.iter().any(|n| n.kind == K::Function));
    assert!(parsed.root.descendants().any(|n| n.kind == K::Error));
    assert_eq!(parsed.reconstructed_source(), parsed.source.text);
}

#[test]
fn recovery_inside_a_class_keeps_its_next_method() {
    let parsed = parse_file(
        "test",
        "class Square {\nfinal side double\nfunction area() : double = 1.0\n}",
    );
    assert!(!parsed.is_ok());
    assert!(parsed.root.descendants().any(|n| n.kind == K::Function));
}

#[test]
fn match_recovery_keeps_later_arms() {
    let parsed = parse_file("test", "match x { Some(a) 7, None => 0, }");
    assert!(!parsed.is_ok());
    assert_eq!(
        parsed
            .root
            .descendants()
            .filter(|n| n.kind == K::MatchArm)
            .count(),
        1
    );
}

#[test]
fn power_groups_strictly_and_ensures_has_its_own_header_node() {
    // Q11: `**` binds tighter than `*`; a signed base and a chain need parentheses.
    assert!(
        parse_file("test", "let n := 2 * 3 ** 2")
            .diagnostics
            .is_empty()
    );
    assert!(
        parse_file("test", "let n := (-2) ** 2")
            .diagnostics
            .is_empty()
    );
    for source in ["let n := -2 ** 2", "let n := 2 ** 3 ** 2"] {
        assert!(
            parse_file("test", source)
                .diagnostics
                .iter()
                .any(|d| d.code == "E_POWER_GROUPING"),
            "{source}"
        );
    }
    // Q53: brackets after a value index it; after a type name they take type arguments.
    let index = parse_file("test", "a[i + 1] = xs[0]\nlet c := Channel[Int]()");
    assert!(index.diagnostics.is_empty());
    let ensures = parse_file("test", "function f() : Int ensures result > 0 = 1");
    assert!(ensures.is_ok(), "{:?}", ensures.diagnostics);
    assert!(ensures.root.children[0].child(K::Ensures).is_some());
}

#[test]
fn project_files_have_a_separate_parser() {
    assert!(
        parse_project(
            "project",
            "package geometry\ndepends matrix\ndepends other.tools\n"
        )
        .is_ok()
    );
    assert!(!parse_project("project", "depends matrix").is_ok());
    assert!(!parse_project("project", "package geometry\npackage other").is_ok());
    assert!(!parse_file("source", "depends matrix").is_ok());
}

#[test]
fn empty_and_comments_only_files_parse() {
    for source in ["", "// comment", "\r\n\t // end\n"] {
        good(source);
    }
}

#[test]
fn deep_expressions_report_a_limit_without_a_panic() {
    let text = format!(
        "{}x{}\nfunction good() : Int = 1",
        "(".repeat(1000),
        ")".repeat(1000)
    );
    let parsed = parse_file("test", text);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.code == "E_NESTING_LIMIT")
    );
}

#[test]
fn long_chains_report_a_limit_without_deep_drop_recursion() {
    let text = std::iter::repeat_n("1", 10_000)
        .collect::<Vec<_>>()
        .join(" + ");
    let parsed = parse_file("test", text);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.code == "E_NESTING_LIMIT")
    );
}

#[test]
fn deep_else_if_chains_report_a_limit() {
    let text = format!("{}{{ 0 }}", "if flag { 1 } else ".repeat(1000));
    let parsed = parse_file("test", text);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.code == "E_NESTING_LIMIT")
    );
}

#[test]
fn damaged_input_always_makes_progress() {
    let alphabet = [
        "function", "let", "x", "(", ")", "[", "]", "{", "}", ":", ":=", "=", "=>", ",", ";", "\n",
        "1", "\"s\"", "if", "match", "@", "?",
    ];
    let mut state = 0x4b65737472656cu64;
    for _ in 0..1500 {
        let mut text = String::new();
        for _ in 0..40 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push_str(alphabet[(state >> 32) as usize % alphabet.len()]);
            text.push(' ');
        }
        let parsed = parse_file("damaged", &text);
        assert_eq!(parsed.reconstructed_source(), text);
        for diagnostic in &parsed.diagnostics {
            assert!(diagnostic.span.end <= text.len());
            let _ = diagnostic.render(&parsed.source);
        }
    }
}

#[test]
fn decimal_exponents_keep_signs_separators_and_source_spelling() {
    let source = "1e3 2.5e-4 1E+10 1_000.5_0e-1_0 0xDEAD 0x1e3 1.exp";
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let tokens = lexed
        .tokens
        .iter()
        .filter(|t| !t.is_trivia() && t.kind != TokenKind::Eof)
        .map(|t| (t.kind, &source[t.span.range()]))
        .collect::<Vec<_>>();
    assert_eq!(
        tokens,
        vec![
            (TokenKind::Double, "1e3"),
            (TokenKind::Double, "2.5e-4"),
            (TokenKind::Double, "1E+10"),
            (TokenKind::Double, "1_000.5_0e-1_0"),
            (TokenKind::Integer, "0xDEAD"),
            (TokenKind::Integer, "0x1e3"),
            (TokenKind::Integer, "1"),
            (TokenKind::Symbol, "."),
            (TokenKind::Word, "exp")
        ]
    );
    good("let a := 1e3\nlet b := f\"{2.5e-4:.4f}\"\nmatch a {1E3 => 1, _ => 0,}");
}

#[test]
fn malformed_exponents_report_number_errors_without_losing_following_lines() {
    for literal in [
        "1e", "1E+", "1e-", "1e_2", "1e2_", "1e2__3", "1_e3", "1.0_e3", "1e+_3",
    ] {
        let source = format!("let x := {literal}\nlet after := 7");
        let parsed = parse_file("bad.kes", &source);
        assert!(
            parsed.diagnostics.iter().any(|d| d.code == "E_NUMBER"),
            "{source}: {:?}",
            parsed.diagnostics
        );
        assert!(
            parsed
                .root
                .descendants()
                .any(|n| n.kind == K::Name && parsed.source.slice(n.span) == "after")
        );
    }
}

#[test]
fn associated_type_declarations_remain_distinct_from_bindings() {
    let parsed = good(
        "trait Source {type Item\nfunction next() : Option[Item]}\nclass Counter {has Source(Item = Int)\nnew() {}}\n",
    );
    assert_eq!(
        parsed
            .source
            .slice(has(&parsed, K::AssociatedTypeDeclaration).span),
        "type Item"
    );
    assert_eq!(
        parsed
            .root
            .descendants()
            .filter(|n| n.kind == K::AssociatedType)
            .count(),
        1
    );
}

#[test]
fn whole_module_import_aliases_keep_the_module_path() {
    let parsed = good("import geometry.shapes as g\nimport std.math\ng.area(2.0)");
    let imports = parsed
        .root
        .children
        .iter()
        .filter(|n| n.kind == K::Import)
        .collect::<Vec<_>>();
    assert_eq!(
        parsed.source.slice(imports[0].children[0].span),
        "geometry.shapes"
    );
    assert_eq!(parsed.source.slice(imports[0].children[1].span), "g");
    assert_eq!(imports[1].children.len(), 1);
}

#[test]
fn unicode_identifier_tokens_keep_combining_marks_and_uncased_letters() {
    let text = "let café := 1\nlet 数 := café\nclass ǅata[T] {}\n";
    let parsed = good(text);
    assert_eq!(parsed.reconstructed_source(), text);
    for name in ["café", "数", "ǅata"] {
        assert!(
            parsed
                .tokens
                .iter()
                .any(|token| token.kind == TokenKind::Word
                    && parsed.source.slice(token.span) == name)
        );
    }
    let parsed = good("function f(x : ǅata[Int]) : Int = match x { ǅata(n) => n, }");
    has(&parsed, K::PatternConstructor);
    let parsed = good("ǅata[Int](1)");
    has(&parsed, K::BracketApplication);
}

#[test]
fn unicode_identifier_profile_rejects_nonletters_and_invisible_controls() {
    for text in ["a²", "a\u{200d}b", "a\u{fe0f}b", "\u{301}accent", "😀"] {
        let lexed = lex(text);
        assert!(
            lexed.diagnostics.iter().any(|d| d.code == "E_CHARACTER"),
            "{text:?}"
        );
        assert_eq!(
            lexed
                .tokens
                .iter()
                .map(|t| &text[t.span.range()])
                .collect::<String>(),
            text
        );
    }
    // U+2118 is an Other_ID_Start character, rather than an alphabetic letter.
    assert!(lex("℘value").diagnostics.is_empty());
}

#[test]
fn newlines_continue_only_at_grammar_positions_that_need_more_input() {
    for text in [
        "let total :=\n  1 + // next operand\n  2",
        "f(\n1\n,\n2\n)",
        "[\n1\n,\n2\n]",
        "if true\n{1}\nelse\n{2}",
        "if true then\n1\nelse\n2",
        "function value() : Int =\n  3",
    ] {
        assert_eq!(good(text).reconstructed_source(), text);
    }
    let parsed = good("first\n+second");
    assert_eq!(parsed.root.children.len(), 2);
    assert_eq!(parsed.root.children[1].kind, K::Unary);
    let parsed = good("function stop() : void {return\n7}");
    assert!(has(&parsed, K::Return).children.is_empty());
    for text in ["(1\n+2)", "f(1\n2)", "first.\nsecond", "first\n.second"] {
        assert!(!parse_file("test", text).is_ok(), "{text}");
    }
}

#[test]
fn if_let_is_a_match_with_a_binding_arm_and_an_else_arm() {
    let parsed = good("let v := if let Some(n) := m { n } else { 0 }\n");
    let matched = has(&parsed, K::Match);
    assert_eq!(matched.children.len(), 3);
    assert_eq!(matched.children[1].kind, K::MatchArm);
    assert_eq!(matched.children[2].kind, K::MatchArm);
    assert_eq!(matched.children[2].children[0].kind, K::PatternWildcard);
}

#[test]
fn if_let_needs_an_else_arm() {
    let parsed = parse_file("test.kes", "let v := if let Some(n) := m { n }\n");
    assert!(
        parsed.diagnostics.iter().any(|d| d.code == "E_EXPECTED"),
        "{:?}",
        parsed.diagnostics
    );
}

#[test]
fn a_named_while_binds_a_proof_of_its_condition_in_the_body() {
    let parsed = good("let mut k := 3\nwhile h : k > 0 {\n    k = k - 1\n}\n");
    let loop_node = has(&parsed, K::While);
    let body = &loop_node.children[1];
    assert_eq!(body.children[0].kind, K::Binding);
    assert_eq!(body.children[0].children[1].kind, K::ProofWitness);
}

#[test]
fn a_proof_parameter_is_written_as_a_proposition() {
    let parsed = good("function half(n : Int, proof h : n > 0) : Int = n / 2\n");
    let parameter = parsed
        .root
        .descendants()
        .find(|n| n.kind == K::Parameter && n.children.iter().any(|c| c.kind == K::Proposition))
        .expect("a parameter with a proposition");
    assert_eq!(parameter.children.len(), 3);
}

#[test]
fn a_named_proof_needs_a_block_in_the_then_form() {
    let parsed = parse_file("test.kes", "let v := if h : true then 1 else 0\n");
    assert!(parsed.diagnostics.iter().any(|d| d.code == "E_PROOF_BLOCK"));
}

#[test]
fn a_compound_assignment_is_an_assignment_of_a_binary_value() {
    let parsed = good("let mut total := 1\ntotal += 4\n");
    let assignment = has(&parsed, K::Assignment);
    assert_eq!(assignment.children[0].kind, K::Name);
    let value = &assignment.children[1];
    assert_eq!(value.kind, K::Binary);
    assert_eq!(value.children[1].kind, K::Operator);
    assert_eq!(parsed.source.slice(value.children[1].span), "+");
}

#[test]
fn a_compound_assignment_refuses_a_call_in_its_target() {
    let parsed = parse_file(
        "test.kes",
        "let mut counts := Array(3, 0)\ncounts[next()] += 5\n",
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.code == "E_COMPOUND_TARGET")
    );
}
