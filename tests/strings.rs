use kestrel::{parse_file, strings::decode};

#[test]
fn json_escapes_decode_including_unicode_pairs() {
    assert_eq!(
        decode(r#"\"\\\/\b\f\n\r\t\u0000\u03bb\uD83D\uDE00"#, 0, false).unwrap(),
        "\"\\/\u{8}\u{c}\n\r\t\0λ😀"
    );
    assert_eq!(decode("plain λ 😀", 0, false).unwrap(), "plain λ 😀");
}
#[test]
fn invalid_escapes_and_raw_controls_are_refused() {
    for text in [
        r"\x20",
        r"\uZZZZ",
        r"\u123",
        r"\uD800",
        r"\uD800\u0000",
        r"\uDC00",
        "\\",
        "\n",
        "\0",
    ] {
        assert!(decode(text, 0, false).is_err(), "{text:?}");
    }
}
#[test]
fn diagnostic_spans_always_follow_utf8_boundaries() {
    for text in [r"\λ", r"\u12😀", r"\u😀😀", r"\uD800\u😀"] {
        let error = decode(text, 5, false).unwrap_err();
        assert!(text.is_char_boundary(error.span.start - 5), "{error:?}");
        assert!(text.is_char_boundary(error.span.end - 5), "{error:?}");
    }
}
#[test]
fn formatted_braces_are_collapsed_before_escape_decoding() {
    assert_eq!(decode(r"{{hi}} \u007b\u007b", 0, true).unwrap(), "{hi} {{");
    let source = r#"print(f"{{ \u03bb {f"{1}"} \u007d }}")"#;
    let parsed = parse_file("strings.kes", source);
    assert!(parsed.is_ok(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.reconstructed_source(), source);
}
#[test]
fn lexer_rejects_invalid_plain_and_formatted_escapes() {
    for source in [
        r#"let a := "\q""#,
        r#"f"bad \uDE00 {1}""#,
        r#"f"ok { "\z" }""#,
    ] {
        let parsed = parse_file("strings.kes", source);
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| d.code == "E_STRING_ESCAPE"),
            "{source}\n{:?}",
            parsed.diagnostics
        );
        for d in &parsed.diagnostics {
            let _ = d.render(&parsed.source);
        }
    }
    assert!(!parse_file("strings.kes", "\"raw\nline\"").is_ok());
}
