//! JSON escape decoding for literals, with original byte positions in errors.
use crate::{diagnostic::Diagnostic, source::Span};

pub fn decode(text: &str, offset: usize, formatted: bool) -> Result<String, Diagnostic> {
    let mut output = String::with_capacity(text.len());
    scan(text, offset, formatted, Some(&mut output))?;
    Ok(output)
}

pub(crate) fn validate(text: &str, offset: usize, formatted: bool) -> Result<(), Diagnostic> {
    scan(text, offset, formatted, None)
}

fn error(offset: usize, start: usize, end: usize, message: &str) -> Diagnostic {
    Diagnostic::new("E_STRING_ESCAPE", message, Span::new(offset + start, offset + end))
        .with_hint("Use JSON escapes such as \\n, \\t, \\\" or \\u03bb; Unicode surrogate pairs must be complete.")
}

fn hex(text: &str, offset: usize, start: usize) -> Result<u32, Diagnostic> {
    let end = (start + 4).min(text.len());
    let digits = &text.as_bytes()[start..end];
    if digits.len() != 4 || !digits.iter().all(u8::is_ascii_hexdigit) {
        let mut boundary = end;
        while !text.is_char_boundary(boundary) {
            boundary += 1;
        }
        return Err(error(
            offset,
            start.saturating_sub(2),
            boundary,
            "A Unicode escape needs exactly four hexadecimal digits.",
        ));
    }
    Ok(digits.iter().fold(0, |n, digit| {
        n * 16 + (*digit as char).to_digit(16).unwrap()
    }))
}

fn scan(
    text: &str,
    offset: usize,
    formatted: bool,
    mut output: Option<&mut String>,
) -> Result<(), Diagnostic> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let ch = if bytes[i] == b'\\' {
            i += 1;
            let Some(escape) = bytes.get(i).copied() else {
                return Err(error(
                    offset,
                    start,
                    i,
                    "This escape is missing its final character.",
                ));
            };
            i += 1;
            match escape {
                b'"' => '"',
                b'\\' => '\\',
                b'/' => '/',
                b'b' => '\u{0008}',
                b'f' => '\u{000c}',
                b'n' => '\n',
                b'r' => '\r',
                b't' => '\t',
                b'u' => {
                    let first = hex(text, offset, i)?;
                    i += 4;
                    let scalar = if (0xd800..=0xdbff).contains(&first) {
                        if !text.as_bytes()[i..].starts_with(b"\\u") {
                            return Err(error(
                                offset,
                                start,
                                i,
                                "A high Unicode surrogate needs a following low surrogate.",
                            ));
                        }
                        let second = hex(text, offset, i + 2)?;
                        if !(0xdc00..=0xdfff).contains(&second) {
                            return Err(error(
                                offset,
                                start,
                                i + 6,
                                "The second Unicode escape must be a low surrogate.",
                            ));
                        }
                        i += 6;
                        0x10000 + ((first - 0xd800) << 10) + second - 0xdc00
                    } else if (0xdc00..=0xdfff).contains(&first) {
                        return Err(error(
                            offset,
                            start,
                            i,
                            "A low Unicode surrogate needs a preceding high surrogate.",
                        ));
                    } else {
                        first
                    };
                    char::from_u32(scalar).unwrap()
                }
                _ => {
                    let end = start + 1 + text[start + 1..].chars().next().unwrap().len_utf8();
                    return Err(error(
                        offset,
                        start,
                        end,
                        "This is not a JSON string escape.",
                    ));
                }
            }
        } else {
            let ch = text[i..].chars().next().unwrap();
            if ch < '\u{0020}' {
                return Err(error(
                    offset,
                    i,
                    i + ch.len_utf8(),
                    "A control character must be written as a JSON escape.",
                ));
            }
            i += ch.len_utf8();
            if formatted && matches!(ch, '{' | '}') && bytes.get(i) == Some(&(ch as u8)) {
                i += 1;
            }
            ch
        };
        if let Some(out) = &mut output {
            out.push(ch);
        }
    }
    Ok(())
}

/// Validate only literal chunks; the parser lexes interpolation expressions separately.
pub(crate) fn validate_formatted(text: &str, start: usize, end: usize) -> Result<(), Diagnostic> {
    let mut i = start;
    let mut chunk = start;
    let bytes = text.as_bytes();
    while i < end {
        if bytes[i] == b'\\' {
            i += 1;
            if i < end {
                i += text[i..end].chars().next().unwrap().len_utf8();
            }
        } else if matches!(bytes[i], b'{' | b'}') && bytes.get(i + 1) == Some(&bytes[i]) {
            i += 2;
        } else if bytes[i] == b'{' {
            validate(&text[chunk..i], chunk, true)?;
            let Some(close) = crate::lexer::interpolation_end(text, i + 1, end, 0) else {
                return Ok(());
            };
            i = close + 1;
            chunk = i;
        } else {
            i += text[i..end].chars().next().unwrap().len_utf8();
        }
    }
    validate(&text[chunk..end], chunk, true)
}

/// What a format spec such as `>8,.2f` asks of its value (Python's mini-language).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatKind {
    /// `{x:?}`: the debug view.
    Debug,
    /// `{x:.2f}`: the original fixed-point fast path.
    Fixed(u32),
    /// Only fill, alignment, width or precision: any Text value.
    Text,
    /// A float presentation (`e f g %`) or a sign, `#` or grouping: Int, Nat or double.
    Number,
    /// An integer presentation (`d x X o b c`): Int or Nat.
    Integer,
}

/// Reads `[[fill]align][sign][#][0][width][,|_][.precision][type]`, or None when it is malformed.
pub fn format_kind(spec: &str) -> Option<FormatKind> {
    if spec == "?" {
        return Some(FormatKind::Debug);
    }
    if let Some(digits) = spec.strip_prefix('.').and_then(|s| s.strip_suffix('f'))
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
    {
        return digits.parse().ok().map(FormatKind::Fixed);
    }
    let chars = spec.chars().collect::<Vec<_>>();
    let mut i = 0;
    if chars.len() >= 2 && "<>^=".contains(chars[1]) {
        i = 2;
    } else if chars.first().is_some_and(|c| "<>^=".contains(*c)) {
        i = 1;
    }
    let mut numeric = i > 0 && chars[i - 1] == '=';
    if chars.get(i).is_some_and(|c| "+- ".contains(*c)) {
        numeric = true;
        i += 1;
    }
    if chars.get(i) == Some(&'#') {
        numeric = true;
        i += 1;
    }
    while chars.get(i).is_some_and(char::is_ascii_digit) {
        i += 1;
    }
    if chars.get(i).is_some_and(|c| *c == ',' || *c == '_') {
        numeric = true;
        i += 1;
    }
    if chars.get(i) == Some(&'.') {
        i += 1;
        if !chars.get(i).is_some_and(char::is_ascii_digit) {
            return None;
        }
        while chars.get(i).is_some_and(char::is_ascii_digit) {
            i += 1;
        }
    }
    let kind = match chars.get(i) {
        None => {
            if numeric {
                FormatKind::Number
            } else {
                FormatKind::Text
            }
        }
        Some(c) if "dxXobc".contains(*c) => FormatKind::Integer,
        Some(c) if "eEfFgG%".contains(*c) => FormatKind::Number,
        Some('s') if !numeric => FormatKind::Text,
        Some(_) => return None,
    };
    (i + usize::from(chars.get(i).is_some()) == chars.len()).then_some(kind)
}

/// One part of a `"...".format(...)` template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplatePiece {
    Text(String),
    /// The argument's position and its format spec, if any.
    Slot(usize, Option<String>),
}

/// Reads `{}`, `{0}`, `{:>8}` and `{1:.2f}`; `{{` and `}}` are literal braces. As in
/// Python, a template numbers its slots either automatically or explicitly, not both.
pub fn template_pieces(template: &str) -> Result<Vec<TemplatePiece>, String> {
    let mut pieces = vec![];
    let mut text = String::new();
    let mut chars = template.chars().peekable();
    let mut automatic = 0;
    let mut numbering: Option<bool> = None;
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                text.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                text.push('}');
            }
            '}' => return Err("A `}` needs a matching `{`; write `}}` for a brace.".into()),
            '{' => {
                let mut inside = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => inside.push(c),
                        None => return Err("A `{` is never closed; write `{{` for a brace.".into()),
                    }
                }
                let (position, spec) = match inside.split_once(':') {
                    Some((position, spec)) => (position.trim().to_owned(), Some(spec.to_owned())),
                    None => (inside.trim().to_owned(), None),
                };
                if let Some(spec) = &spec
                    && format_kind(spec).is_none()
                {
                    return Err(format!("`{spec}` is not a format spec."));
                }
                let explicit = !position.is_empty();
                if numbering.is_some_and(|known| known != explicit) {
                    return Err("Number every slot, as in {0} {1}, or none, as in {} {}.".into());
                }
                numbering = Some(explicit);
                let index = if explicit {
                    position
                        .parse::<usize>()
                        .map_err(|_| format!("`{position}` is not an argument position."))?
                } else {
                    automatic += 1;
                    automatic - 1
                };
                if !text.is_empty() {
                    pieces.push(TemplatePiece::Text(std::mem::take(&mut text)));
                }
                pieces.push(TemplatePiece::Slot(index, spec));
            }
            c => text.push(c),
        }
    }
    if !text.is_empty() {
        pieces.push(TemplatePiece::Text(text));
    }
    Ok(pieces)
}
