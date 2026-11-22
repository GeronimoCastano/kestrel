//! `kestrel fmt` must be a whitespace-only rewrite: every token, comment and
//! syntax node survives, and a second run produces the same bytes.

use kestrel::{
    formatter::format_text,
    lexer::{TokenKind, lex},
    parse_file,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

fn corpus() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![];
    for folder in ["src", "testdata/language"] {
        let mut found: Vec<PathBuf> = fs::read_dir(root.join(folder))
            .unwrap_or_else(|error| panic!("cannot read {folder}: {error}"))
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "kes"))
            .collect();
        found.sort();
        files.extend(found);
    }
    files
}

fn source(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

fn tokens(text: &str) -> Vec<(TokenKind, String)> {
    lex(text)
        .tokens
        .into_iter()
        .filter(|token| {
            !matches!(
                token.kind,
                TokenKind::Whitespace | TokenKind::Newline | TokenKind::Eof
            )
        })
        .map(|token| (token.kind, text[token.span.range()].to_owned()))
        .collect()
}

fn token_multiset(text: &str) -> BTreeMap<(String, String), usize> {
    let mut counts = BTreeMap::new();
    for (kind, value) in tokens(text) {
        if value == "," {
            // The formatter adds trailing commas in comma-separated groups.
            continue;
        }
        *counts.entry((format!("{kind:?}"), value)).or_insert(0) += 1;
    }
    counts
}

fn comments(text: &str) -> Vec<String> {
    let mut found: Vec<String> = lex(text)
        .tokens
        .into_iter()
        .filter(|token| token.kind == TokenKind::Comment)
        .map(|token| text[token.span.range()].to_owned())
        .collect();
    found.sort();
    found
}

fn spanless(dump: &str) -> String {
    dump.lines()
        .map(|line| {
            if let Some(at) = line.find('@') {
                let head = &line[..at];
                let rest = &line[at..];
                let tail = rest.find(' ').map_or("", |space| &rest[space..]);
                format!("{head}{tail}")
            } else {
                line.to_owned()
            }
        })
        .map(|line| {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("Block \"")
                && let Some(text) = rest.strip_suffix('"')
                && text.chars().all(|c| matches!(c, '{' | '}' | ' '))
            {
                // `{ }` and `{}` are the same empty block.
                let indent = &line[..line.len() - trimmed.len()];
                return format!("{indent}Block \"{{}}\"");
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn without_imports(dump: &str) -> String {
    let mut kept = vec![];
    let mut skipping: Option<usize> = None;
    for line in dump.lines() {
        let indent = line.len() - line.trim_start().len();
        if let Some(skip) = skipping {
            if line.trim().is_empty() || indent > skip {
                continue;
            }
            skipping = None;
        }
        let first = line.split_whitespace().next().unwrap_or("");
        if first == "Import" {
            skipping = Some(indent);
            continue;
        }
        kept.push(line);
    }
    kept.join("\n")
}

fn diagnostics(parsed: &kestrel::Parsed) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = parsed
        .diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.code.to_owned(), diagnostic.message.clone()))
        .collect();
    found.sort();
    found
}

#[test]
fn formats_every_language_fixture_and_standard_library_file() {
    for path in corpus() {
        let original = source(&path);
        let formatted = format_text(&original).unwrap_or_else(|diagnostics| {
            panic!(
                "{} did not format: {:?}",
                path.display(),
                diagnostics.iter().map(|d| d.code).collect::<Vec<_>>()
            )
        });
        assert!(
            formatted.is_empty() || formatted.ends_with('\n'),
            "{} has no final newline",
            path.display()
        );
        assert!(
            !formatted.ends_with("\n\n"),
            "{} ends with a blank line",
            path.display()
        );
        for (number, line) in formatted.lines().enumerate() {
            assert_eq!(
                line,
                line.trim_end(),
                "{}:{} has trailing whitespace",
                path.display(),
                number + 1
            );
        }
        let again = format_text(&formatted).unwrap();
        assert_eq!(again, formatted, "{} is not idempotent", path.display());
    }
}

#[test]
fn comments_and_literals_survive_the_round_trip() {
    for path in corpus() {
        let original = source(&path);
        let formatted = format_text(&original).unwrap();
        assert_eq!(
            comments(&original),
            comments(&formatted),
            "{} changed its comments",
            path.display()
        );
        assert_eq!(
            token_multiset(&original),
            token_multiset(&formatted),
            "{} changed a token",
            path.display()
        );
    }
}

#[test]
fn formatting_preserves_the_syntax_tree_and_diagnostics() {
    for path in corpus() {
        let original = source(&path);
        let formatted = format_text(&original).unwrap();
        let before = parse_file(path.display().to_string(), original);
        let after = parse_file(path.display().to_string(), formatted);
        assert_eq!(
            diagnostics(&before),
            diagnostics(&after),
            "{} changed its diagnostics",
            path.display()
        );
        let before_tree = without_imports(&spanless(&before.root.dump(&before.source)));
        let after_tree = without_imports(&spanless(&after.root.dump(&after.source)));
        assert_eq!(
            before_tree,
            after_tree,
            "{} changed its syntax tree",
            path.display()
        );
    }
}

#[test]
fn every_construct_has_a_stable_round_trip() {
    let cases = [
        "function f(a : Int, b : Int) : Int = a + b",
        "class C[T] {\n    var value : T\n    new(value : T) { this.value = value }\n}",
        "record Point(x : Int, y : Int)",
        "enum E { A, B }",
        "let xs := [1, 2, 3]",
        "let m := Map([(\"a\", 1)])",
        "match x { A => 1, B => 2 }",
        "if x then y else z",
        "for (x in xs) { print(x) }",
        "while x > 0 { x = x - 1 }",
        "theorem t(n : Nat) : n = n by { rfl }",
        "import std.math.{cos, sin}",
        "x += 1",
        "y++",
        "f(a = 1, b = 2)",
        "let s := \"a\\\\nb\"",
        "let s := f\"{x:.2f}\"",
    ];
    for source in cases {
        let once = format_text(source).unwrap();
        let twice = format_text(&once).unwrap();
        assert_eq!(once, twice, "{source}");
        assert_eq!(
            tokens(source)
                .into_iter()
                .filter(|(_, value)| value != ",")
                .collect::<Vec<_>>(),
            tokens(&once)
                .into_iter()
                .filter(|(_, value)| value != ",")
                .collect::<Vec<_>>(),
            "{source}"
        );
    }
}

#[test]
fn a_file_with_a_semantic_error_still_formats() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/language/named-order.kes");
    let original = source(&path);
    let formatted = format_text(&original).unwrap();
    assert!(formatted.contains("scaled(scale = 3.0, 2.0)"));
    assert_eq!(format_text(&formatted).unwrap(), formatted);
}

#[test]
fn comment_shapes_are_kept() {
    let source = "// banner\n//// section\n/// docs\nf(\n    1, // trailing\n    // own line\n    2,\n)\nlet text := \"// not a comment\"\n";
    let formatted = format_text(source).unwrap();
    assert_eq!(comments(source), comments(&formatted));
    assert!(formatted.contains("// banner"));
    assert!(formatted.contains("//// section"));
    assert!(formatted.contains("/// docs"));
    assert!(formatted.contains("1, // trailing"));
    assert!(formatted.contains("\"// not a comment\""));
    let set: BTreeSet<_> = comments(&formatted).into_iter().collect();
    assert_eq!(set.len(), 5);
}
