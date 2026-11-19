//! The pinned identifier profile shared by lexing, naming and session rewriting.

#[path = "identifier_data.rs"]
mod data;

fn in_ranges(c: char, ranges: &[(u32, u32)]) -> bool {
    let code = c as u32;
    let i = ranges.partition_point(|(_, end)| *end < code);
    ranges.get(i).is_some_and(|(start, _)| *start <= code)
}

pub(crate) fn start(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphabetic() || c == '_'
    } else {
        in_ranges(c, data::START) && !in_ranges(c, data::IGNORABLE)
    }
}

pub(crate) fn continuation(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphanumeric() || c == '_'
    } else {
        in_ranges(c, data::CONTINUE) && !in_ranges(c, data::IGNORABLE)
    }
}

pub(crate) fn word(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(start) && chars.all(continuation)
}

pub(crate) fn type_start(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_uppercase()
    } else {
        start(c) && (in_ranges(c, data::UPPER) || in_ranges(c, data::TITLE))
    }
}

fn lower(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_lowercase()
    } else {
        in_ranges(c, data::LOWER)
    }
}

pub(crate) fn valid(text: &str, form: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    match form {
        "value identifier" => word(text) && text != "_",
        "camelCase" => {
            first != '_'
                && start(first)
                && !type_start(first)
                && chars.all(|c| c != '_' && continuation(c))
        }
        "PascalCase" => type_start(first) && chars.all(|c| c != '_' && continuation(c)),
        "SCREAMING_SNAKE_CASE" => type_start(first) && chars.all(|c| continuation(c) && !lower(c)),
        _ => false,
    }
}
