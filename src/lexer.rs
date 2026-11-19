use crate::{diagnostic::Diagnostic, source::Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Word,
    Integer,
    Double,
    String,
    FormatString,
    Symbol,
    Whitespace,
    Newline,
    Comment,
    Invalid,
    Eof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn is_trivia(self) -> bool {
        matches!(self.kind, TokenKind::Whitespace | TokenKind::Comment)
    }
}

#[derive(Debug)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn lex(text: &str) -> Lexed {
    lex_range(text, 0, text.len())
}

pub(crate) fn lex_range(text: &str, start: usize, end: usize) -> Lexed {
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut i = start;
    let bytes = text.as_bytes();
    while i < end {
        let start = i;
        let ch = text[i..end].chars().next().unwrap();
        let kind = match ch {
            '\n' => {
                i += 1;
                TokenKind::Newline
            }
            '\r' => {
                i += 1;
                if i < end && bytes[i] == b'\n' {
                    i += 1;
                }
                TokenKind::Newline
            }
            c if c.is_whitespace() => {
                i += c.len_utf8();
                while i < end {
                    let c = text[i..end].chars().next().unwrap();
                    if !c.is_whitespace() || matches!(c, '\n' | '\r') {
                        break;
                    }
                    i += c.len_utf8();
                }
                TokenKind::Whitespace
            }
            '/' if text[i..end].starts_with("//") => {
                while i < end && !matches!(bytes[i], b'\n' | b'\r') {
                    i += 1;
                }
                TokenKind::Comment
            }
            '"' | 'f' if ch == '"' || text[i..end].starts_with("f\"") => {
                let formatted = ch == 'f';
                let quote = i + usize::from(formatted);
                let (next, closed) = string_end(text, quote, end, formatted, 0);
                i = next;
                if !closed {
                    diagnostics.push(
                        Diagnostic::new(
                            "E_UNCLOSED_STRING",
                            "This string is missing its closing quote.",
                            Span::new(start, i),
                        )
                        .with_hint("Close the string with a double quote."),
                    );
                }
                if closed {
                    let result = if formatted {
                        crate::strings::validate_formatted(text, quote + 1, i - 1)
                    } else {
                        crate::strings::validate(&text[quote + 1..i - 1], quote + 1, false)
                    };
                    if let Err(error) = result {
                        diagnostics.push(error);
                    }
                }
                if formatted {
                    TokenKind::FormatString
                } else {
                    TokenKind::String
                }
            }
            '0'..='9' => {
                i += 1;
                if ch == '0' && i < end && matches!(bytes[i], b'x' | b'X') {
                    i += 1;
                    let digits = i;
                    while i < end && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
                        i += 1;
                    }
                    if digits == i || !valid_digits(&text[digits..i], 16) {
                        diagnostics.push(Diagnostic::new(
                            "E_NUMBER",
                            "Expected hexadecimal digits after 0x.",
                            Span::new(start, i),
                        ));
                    }
                    TokenKind::Integer
                } else {
                    while i < end && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                        i += 1;
                    }
                    let mut kind = TokenKind::Integer;
                    if i + 1 < end && bytes[i] == b'.' && bytes[i + 1].is_ascii_digit() {
                        kind = TokenKind::Double;
                        i += 1;
                        while i < end && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                            i += 1;
                        }
                    }
                    if text[start..i]
                        .split('.')
                        .any(|part| !valid_digits(part, 10))
                    {
                        diagnostics.push(Diagnostic::new(
                            "E_NUMBER",
                            "Put a single underscore between digits.",
                            Span::new(start, i),
                        ));
                    }
                    if i < end && matches!(bytes[i], b'e' | b'E') {
                        kind = TokenKind::Double;
                        i += 1;
                        if i < end && matches!(bytes[i], b'+' | b'-') {
                            i += 1;
                        }
                        let digits = i;
                        while i < end && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                            i += 1;
                        }
                        if !valid_digits(&text[digits..i], 10) {
                            diagnostics.push(Diagnostic::new("E_NUMBER", "An exponent needs decimal digits, with single underscores only between digits.", Span::new(start, i)).with_hint("Write an exponent such as 1e3, 2.5e-4 or 1E+10."));
                        }
                    }
                    kind
                }
            }
            c if crate::identifiers::start(c) => {
                i += c.len_utf8();
                while i < end {
                    let c = text[i..end].chars().next().unwrap();
                    // U+1D9C is a modifier letter, so the identifier table
                    // accepts it; set complement needs it as its own token.
                    if !crate::identifiers::continuation(c) || c == '\u{1d9c}' {
                        break;
                    }
                    i += c.len_utf8();
                }
                TokenKind::Word
            }
            '\\' => {
                i += 1;
                while i < end {
                    let c = text[i..end].chars().next().unwrap();
                    if !crate::identifiers::continuation(c) {
                        break;
                    }
                    i += c.len_utf8();
                }
                TokenKind::Symbol
            }
            c if "(){}[],:;.@?+-*/%&|^!~=<>∈⊆→∧∨¬∀∃∅₀".contains(c) => {
                const PAIRS: &[&str] = &[
                    ":=", "=>", "->", "|>", "==", "!=", "<=", ">=", "&&", "||", "**", "<<", ">>",
                    "..", "++", "--", "+=", "-=", "*=", "/=", "%=",
                ];
                if let Some(pair) = PAIRS.iter().find(|pair| text[i..end].starts_with(**pair)) {
                    i += pair.len();
                } else {
                    i += c.len_utf8();
                }
                TokenKind::Symbol
            }
            // The subscripted family operators are one token, so the parser
            // can recognize `⋃₀ F` without a second lookahead for `₀`.
            c if (c == '⋃' || c == '⋂') && text[i + c.len_utf8()..end].starts_with('₀') => {
                i += c.len_utf8() + '₀'.len_utf8();
                TokenKind::Symbol
            }
            _ if is_math_symbol(ch) => {
                // Mathematical operators can become declared symbols. Letters,
                // marks, emoji and other runes keep their own diagnostics.
                i += ch.len_utf8();
                TokenKind::Symbol
            }
            _ => {
                i += ch.len_utf8();
                diagnostics.push(Diagnostic::new(
                    "E_CHARACTER",
                    format!("Unexpected character {ch:?}."),
                    Span::new(start, i),
                ));
                TokenKind::Invalid
            }
        };
        tokens.push(Token {
            kind,
            span: Span::new(start, i),
        });
    }
    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span::new(end, end),
    });
    Lexed {
        tokens,
        diagnostics,
    }
}

/// Numeric classification shared by expression and pattern checking. A hex
/// digit e/E is part of an integer, including in a negative pattern.
pub(crate) fn is_double_literal(text: &str) -> bool {
    let digits = text
        .strip_prefix('-')
        .or_else(|| text.strip_prefix('+'))
        .unwrap_or(text);
    !digits.starts_with("0x") && !digits.starts_with("0X") && digits.contains(['.', 'e', 'E'])
}

/// Resolve a dotted source name independently of trivia between its tokens.
pub(crate) fn path_name(text: &str) -> String {
    if !text.chars().any(char::is_whitespace) && !text.contains("//") {
        return text.into();
    }
    lex(text)
        .tokens
        .into_iter()
        .filter(|token| {
            !token.is_trivia() && !matches!(token.kind, TokenKind::Newline | TokenKind::Eof)
        })
        .map(|token| &text[token.span.range()])
        .collect()
}

/// Ranges that carry mathematical operators. A declaration can claim one of
/// these as an infix symbol; typography elsewhere stays an error.
fn is_math_symbol(c: char) -> bool {
    matches!(c,
        '\u{00ac}' | '\u{00b1}' | '\u{00d7}' | '\u{00f7}'
        | '\u{2190}'..='\u{21ff}'
        | '\u{2200}'..='\u{22ff}'
        | '\u{1d9c}'
        | '\u{2080}'
        | '\u{27f0}'..='\u{27ff}'
        | '\u{2980}'..='\u{29ff}'
        | '\u{2a00}'..='\u{2aff}'
    )
}

fn valid_digits(text: &str, radix: u32) -> bool {
    !text.is_empty()
        && !text.starts_with('_')
        && !text.ends_with('_')
        && !text.contains("__")
        && text.chars().all(|c| c == '_' || c.is_digit(radix))
}

pub(crate) fn string_end(
    text: &str,
    quote: usize,
    end: usize,
    formatted: bool,
    depth: usize,
) -> (usize, bool) {
    if depth > 64 {
        return (end, false);
    }
    let bytes = text.as_bytes();
    let mut i = quote + 1;
    while i < end {
        match bytes[i] {
            b'\\' => {
                i += 1;
                if i < end {
                    i += text[i..end].chars().next().unwrap().len_utf8();
                }
            }
            b'"' => return (i + 1, true),
            b'\n' | b'\r' => return (i, false),
            b'{' if formatted => {
                if i + 1 < end && bytes[i + 1] == b'{' {
                    i += 2;
                    continue;
                }
                let Some(close) = interpolation_end(text, i + 1, end, depth + 1) else {
                    let recovery = text[i..end]
                        .find(['\r', '\n'])
                        .map(|offset| i + offset)
                        .unwrap_or(end);
                    return (recovery, false);
                };
                i = close + 1;
            }
            b'}' if formatted && i + 1 < end && bytes[i + 1] == b'}' => i += 2,
            _ => i += text[i..end].chars().next().unwrap().len_utf8(),
        }
    }
    (i, false)
}

pub(crate) fn interpolation_end(
    text: &str,
    start: usize,
    end: usize,
    depth: usize,
) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut braces = 0;
    let mut i = start;
    while i < end {
        match bytes[i] {
            b'"' => {
                let (next, closed) = string_end(text, i, end, false, depth);
                if !closed {
                    return None;
                }
                i = next;
            }
            b'f' if i + 1 < end && bytes[i + 1] == b'"' => {
                let (next, closed) = string_end(text, i + 1, end, true, depth);
                if !closed {
                    return None;
                }
                i = next;
            }
            b'/' if i + 1 < end && bytes[i + 1] == b'/' => {
                while i < end && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'{' => {
                braces += 1;
                i += 1;
            }
            b'}' if braces == 0 => return Some(i),
            b'}' => {
                braces -= 1;
                i += 1;
            }
            _ => i += text[i..end].chars().next().unwrap().len_utf8(),
        }
    }
    None
}
