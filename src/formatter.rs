//! `kestrel fmt`: whitespace-only formatting for Kestrel source.
//!
//! The lexer keeps every comment, newline and literal, so the formatter works
//! from tokens rather than from the syntax tree. It builds a tree of matched
//! brackets, then decides indentation and line breaks from that tree. No token
//! text is rewritten: a formatted file has the same lexed tokens, in the same
//! order, as the input. That is what keeps comments, strings and the written
//! form of `x++`, `x += 1` and `(a b : Nat)` intact.
//!
//! A line may only be broken after an operator, or inside brackets. The parser
//! stops an expression at a newline, and a member access cannot cross one, so
//! the formatter never breaks before an operator or a `.`.

use crate::{
    diagnostic::Diagnostic,
    lexer::{Token, TokenKind, lex},
    source::Span,
};

/// The formatter style revision. Bump it when formatting output changes.
pub const FORMAT_REVISION: u32 = 1;

/// The widest line the formatter aims for, counted in characters.
pub const MAX_WIDTH: usize = 100;

/// Formats Kestrel source. The result lexes to the same tokens as the input.
///
/// Lexical errors are returned unchanged, so a caller can render them with the
/// source it already has. A file with a syntax error that still lexes is
/// formatted: the formatter only needs balanced brackets, not a full parse.
pub fn format_text(text: &str) -> Result<String, Vec<Diagnostic>> {
    let lexed = lex(text);
    if !lexed.diagnostics.is_empty() {
        return Err(lexed.diagnostics);
    }
    let mut at = 0;
    let elements = parse_elements(text, &lexed.tokens, &mut at, None, 0)?;
    Ok(format_elements(&elements))
}

#[derive(Clone, Debug)]
enum Element {
    Atom(Atom),
    Newline,
    Group(Group),
}

#[derive(Clone, Debug)]
struct Atom {
    kind: TokenKind,
    text: String,
    /// Whitespace immediately before this token in the input. The token stream
    /// alone cannot tell `f(x)` from a tactic written `trans (x)`, so the two
    /// ambiguous bracket cases keep the author's spacing.
    space_before: bool,
}

#[derive(Clone, Debug)]
struct Group {
    open: char,
    close: char,
    children: Vec<Element>,
    /// A newline directly inside this group, not inside a nested one.
    multiline: bool,
    space_before: bool,
}

fn token_before_is_space(tokens: &[Token], boundary: usize) -> bool {
    boundary > 0 && tokens[boundary - 1].kind == TokenKind::Whitespace
}

fn parse_elements(
    text: &str,
    tokens: &[Token],
    at: &mut usize,
    close: Option<char>,
    depth: usize,
) -> Result<Vec<Element>, Vec<Diagnostic>> {
    if depth > 512 {
        return Err(vec![Diagnostic::new(
            "E_FORMAT_SYNTAX",
            "This file nests brackets too deeply to format.",
            Span::new(text.len(), text.len()),
        )]);
    }
    let mut elements = vec![];
    while let Some(token) = tokens.get(*at).copied() {
        if token.kind == TokenKind::Eof {
            return if close.is_some() {
                Err(vec![Diagnostic::new(
                    "E_FORMAT_SYNTAX",
                    "A delimiter in this file is never closed.",
                    Span::new(text.len(), text.len()),
                )])
            } else {
                Ok(elements)
            };
        }
        let word = &text[token.span.range()];
        let space_before = token_before_is_space(tokens, *at);
        match word {
            ")" | "]" | "}" => {
                let found = word.chars().next().unwrap();
                if close == Some(found) {
                    return Ok(elements);
                }
                return Err(vec![
                    Diagnostic::new(
                        "E_FORMAT_SYNTAX",
                        format!("`{found}` closes a different delimiter."),
                        token.span,
                    )
                    .with_hint("Match this closing bracket with the one that opens the group."),
                ]);
            }
            "(" | "[" | "{" => {
                let open = word.chars().next().unwrap();
                let close_char = match open {
                    '(' => ')',
                    '[' => ']',
                    _ => '}',
                };
                let open_span = token.span;
                *at += 1;
                let children = parse_elements(text, tokens, at, Some(close_char), depth + 1)?;
                let Some(close_token) = tokens.get(*at).copied() else {
                    return Err(vec![Diagnostic::new(
                        "E_FORMAT_SYNTAX",
                        "A delimiter in this file is never closed.",
                        open_span,
                    )]);
                };
                if close_token.kind == TokenKind::Eof {
                    return Err(vec![
                        Diagnostic::new(
                            "E_FORMAT_SYNTAX",
                            "A delimiter in this file is never closed.",
                            open_span,
                        )
                        .with_hint("Close this group with its matching bracket."),
                    ]);
                }
                let multiline = children
                    .iter()
                    .any(|element| matches!(element, Element::Newline));
                *at += 1;
                elements.push(Element::Group(Group {
                    open,
                    close: close_char,
                    children,
                    multiline,
                    space_before,
                }));
            }
            _ => {
                *at += 1;
                match token.kind {
                    TokenKind::Whitespace => {}
                    TokenKind::Newline => elements.push(Element::Newline),
                    kind => elements.push(Element::Atom(Atom {
                        kind,
                        text: word.to_owned(),
                        space_before,
                    })),
                }
            }
        }
    }
    if close.is_some() {
        Err(vec![Diagnostic::new(
            "E_FORMAT_SYNTAX",
            "A delimiter in this file is never closed.",
            Span::new(text.len(), text.len()),
        )])
    } else {
        Ok(elements)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ItemKind {
    Comment,
    Import,
    Declaration,
    Statement,
}

#[derive(Clone, Debug)]
struct Item {
    elements: Vec<Element>,
    blank_before: usize,
    kind: ItemKind,
    import: Option<ImportSpec>,
}

#[derive(Clone, Debug)]
struct ImportSpec {
    path: String,
    names: Vec<String>,
    alias: Option<String>,
    /// `Some((kind, literal))` for `import c "..."` and `import python "..."`.
    foreign: Option<(String, String)>,
    comment: Option<String>,
}

impl ImportSpec {
    fn sort_key(&self) -> String {
        match &self.foreign {
            Some((kind, literal)) => format!(
                "{} {} {}",
                kind,
                literal,
                self.alias.as_deref().unwrap_or("")
            ),
            None => format!(
                "{} {} {}",
                self.path,
                self.names.join(","),
                self.alias.as_deref().unwrap_or("")
            ),
        }
    }
}

fn format_elements(elements: &[Element]) -> String {
    let mut items = split_items(elements);
    sort_imports(&mut items);
    let mut output = String::new();
    let mut previous: Option<&Item> = None;
    for item in &items {
        if item.elements.is_empty() {
            continue;
        }
        let blank = if previous.is_some() {
            root_blank(previous, item)
        } else {
            0
        };
        if let Some(previous) = previous {
            if previous.elements.is_empty() {
                continue;
            }
            for _ in 0..blank {
                output.push('\n');
            }
            output.push('\n');
        }
        output.push_str(&render_item(item, 0));
        previous = Some(item);
    }
    let output = output.trim_end();
    if output.is_empty() {
        String::new()
    } else {
        format!("{output}\n")
    }
}

fn split_items(elements: &[Element]) -> Vec<Item> {
    let mut items = vec![];
    let mut current = vec![];
    let mut blank = 0;
    let mut at = 0;
    while at < elements.len() {
        if matches!(elements[at], Element::Newline) {
            let mut count = 0;
            while at < elements.len() && matches!(elements[at], Element::Newline) {
                count += 1;
                at += 1;
            }
            if current.is_empty() {
                blank += count;
                continue;
            }
            if continues(&current, elements.get(at)) {
                continue;
            }
            items.push(make_item(std::mem::take(&mut current), blank));
            blank = count.saturating_sub(1);
            continue;
        }
        current.push(elements[at].clone());
        at += 1;
    }
    if !current.is_empty() {
        items.push(make_item(current, blank));
    }
    items
}

fn make_item(elements: Vec<Element>, blank_before: usize) -> Item {
    let kind = item_kind(&elements);
    let import = if kind == ItemKind::Import {
        parse_import(&elements)
    } else {
        None
    };
    Item {
        elements,
        blank_before,
        kind,
        import,
    }
}

fn item_kind(elements: &[Element]) -> ItemKind {
    if elements.iter().all(|element| match element {
        Element::Atom(atom) => atom.kind == TokenKind::Comment,
        _ => false,
    }) {
        return ItemKind::Comment;
    }
    let Some(first) = first_text(elements) else {
        return ItemKind::Statement;
    };
    if first == "import" {
        return ItemKind::Import;
    }
    if declaration_start(elements) {
        return ItemKind::Declaration;
    }
    ItemKind::Statement
}

fn declaration_start(elements: &[Element]) -> bool {
    let mut at = 0;
    while let Some(Element::Atom(atom)) = elements.get(at) {
        if atom.text == "@" {
            at += 2;
            continue;
        }
        if matches!(
            atom.text.as_str(),
            "public" | "private" | "package" | "static" | "override" | "sealed" | "mut"
        ) {
            at += 1;
            continue;
        }
        return matches!(
            atom.text.as_str(),
            "function"
                | "class"
                | "trait"
                | "enum"
                | "record"
                | "alias"
                | "const"
                | "theorem"
                | "example"
                | "axiom"
                | "opaque"
                | "infixl"
                | "infixr"
                | "infix"
                | "type"
                | "has"
                | "invariant"
                | "new"
                | "final"
                | "var"
        );
    }
    false
}

fn continues(current: &[Element], next: Option<&Element>) -> bool {
    let Some(last) = current.last() else {
        return false;
    };
    if matches!(
        last,
        Element::Atom(Atom {
            kind: TokenKind::Comment,
            ..
        })
    ) {
        return false;
    }
    if attribute_only(current) {
        return true;
    }
    let last = match last {
        Element::Atom(atom) => atom.text.as_str(),
        Element::Group(group) => match group.close {
            ')' => ")",
            ']' => "]",
            _ => "}",
        },
        Element::Newline => return false,
    };
    let next = match next {
        Some(Element::Atom(atom)) => atom.text.as_str(),
        Some(Element::Group(group)) => match group.open {
            '(' => "(",
            '[' => "[",
            _ => "{",
        },
        Some(Element::Newline) | None => "",
    };
    if matches!(next, "else" | "otherwise") {
        return true;
    }
    matches!(
        last,
        "=" | ":="
            | "=>"
            | "then"
            | "else"
            | ":"
            | "->"
            | "→"
            | "|>"
            | "+"
            | "-"
            | "*"
            | "/"
            | "%"
            | "**"
            | "=="
            | "!="
            | "<"
            | "<="
            | ">"
            | ">="
            | "&&"
            | "||"
            | "&"
            | "|"
            | "^"
            | "<<"
            | ">>"
            | "@"
            | "and"
            | "or"
            | "∈"
            | "⊆"
            | "∪"
            | "∩"
            | "≠"
            | "≤"
            | "≥"
            | ".."
    )
}

fn attribute_only(elements: &[Element]) -> bool {
    let mut at = 0;
    let mut seen = false;
    while at < elements.len() {
        match (elements.get(at), elements.get(at + 1)) {
            (Some(Element::Atom(at_sign)), Some(Element::Atom(name)))
                if at_sign.text == "@" && name.kind == TokenKind::Word =>
            {
                seen = true;
                at += 2;
            }
            _ => return false,
        }
    }
    seen
}

fn root_blank(previous: Option<&Item>, current: &Item) -> usize {
    let Some(previous) = previous else {
        return 0;
    };
    if current.kind == ItemKind::Comment || previous.kind == ItemKind::Comment {
        return current.blank_before.min(1);
    }
    if previous.kind == ItemKind::Import && current.kind == ItemKind::Import {
        return usize::from(current.blank_before > 0);
    }
    if previous.kind == ItemKind::Import || current.kind == ItemKind::Import {
        return 1;
    }
    if attribute_only(&previous.elements) && current.kind == ItemKind::Declaration {
        return 0;
    }
    if current.kind == ItemKind::Declaration && previous.kind == ItemKind::Declaration {
        return 1;
    }
    current.blank_before.min(1)
}

fn first_text(elements: &[Element]) -> Option<&str> {
    match elements.first()? {
        Element::Atom(atom) => Some(atom.text.as_str()),
        Element::Group(group) => Some(match group.open {
            '(' => "(",
            '[' => "[",
            _ => "{",
        }),
        Element::Newline => None,
    }
}

fn sort_imports(items: &mut [Item]) {
    let mut at = 0;
    while at < items.len() {
        if items[at].kind != ItemKind::Import || items[at].import.is_none() {
            at += 1;
            continue;
        }
        let mut end = at;
        while end + 1 < items.len()
            && items[end + 1].kind == ItemKind::Import
            && items[end + 1].import.is_some()
            && items[end + 1].blank_before == 0
        {
            end += 1;
        }
        let mut specs: Vec<ImportSpec> = items[at..=end]
            .iter()
            .filter_map(|item| item.import.clone())
            .collect();
        specs.sort_by_key(ImportSpec::sort_key);
        for (offset, spec) in specs.into_iter().enumerate() {
            items[at + offset].import = Some(spec);
        }
        at = end + 1;
    }
}

fn parse_import(elements: &[Element]) -> Option<ImportSpec> {
    let mut end = elements.len();
    let mut comment = None;
    if let Some(Element::Atom(atom)) = elements.last()
        && atom.kind == TokenKind::Comment
    {
        comment = Some(atom.text.clone());
        end -= 1;
    }
    let elements = &elements[..end];
    if first_text(elements)? != "import" {
        return None;
    }
    let mut at = 1;
    if let Some(Element::Atom(atom)) = elements.get(at)
        && matches!(atom.text.as_str(), "c" | "python")
    {
        let kind = atom.text.clone();
        at += 1;
        let Some(Element::Atom(literal)) = elements.get(at) else {
            return None;
        };
        if literal.kind != TokenKind::String {
            return None;
        }
        let literal = literal.text.clone();
        at += 1;
        let mut alias = None;
        if let Some(Element::Atom(atom)) = elements.get(at)
            && atom.text == "as"
        {
            let Some(Element::Atom(name)) = elements.get(at + 1) else {
                return None;
            };
            alias = Some(name.text.clone());
            at += 2;
        }
        if at != elements.len() {
            return None;
        }
        return Some(ImportSpec {
            path: String::new(),
            names: vec![],
            alias,
            foreign: Some((kind, literal)),
            comment,
        });
    }
    let mut path = String::new();
    while let Some(element) = elements.get(at) {
        match element {
            Element::Atom(atom) if atom.kind == TokenKind::Word && atom.text == "as" => break,
            Element::Atom(atom) if atom.kind == TokenKind::Word => {
                path.push_str(&atom.text);
                at += 1;
            }
            Element::Atom(atom) if atom.text == "." => {
                path.push('.');
                at += 1;
            }
            _ => break,
        }
    }
    if path.is_empty() {
        return None;
    }
    let mut names = vec![];
    if let Some(Element::Group(group)) = elements.get(at) {
        if group.open != '{' {
            return None;
        }
        let mut name = String::new();
        for child in &group.children {
            match child {
                Element::Newline => {}
                Element::Atom(atom) if atom.kind == TokenKind::Word => name.push_str(&atom.text),
                Element::Atom(atom) if atom.text == "." => name.push('.'),
                Element::Atom(atom) if atom.text == "," => {
                    if name.is_empty() {
                        return None;
                    }
                    names.push(std::mem::take(&mut name));
                }
                _ => return None,
            }
        }
        if !name.is_empty() {
            names.push(name);
        }
        if names.is_empty() {
            return None;
        }
        names.sort();
        at += 1;
    }
    let mut alias = None;
    if let Some(Element::Atom(atom)) = elements.get(at)
        && atom.text == "as"
    {
        let Some(Element::Atom(name)) = elements.get(at + 1) else {
            return None;
        };
        alias = Some(name.text.clone());
        at += 2;
    }
    if at != elements.len() {
        return None;
    }
    Some(ImportSpec {
        path,
        names,
        alias,
        foreign: None,
        comment,
    })
}

fn render_item(item: &Item, indent: usize) -> String {
    if let Some(spec) = &item.import {
        return render_doc(&import_doc(spec), indent);
    }
    render_doc(&item_doc(&item.elements), indent)
}

fn import_doc(spec: &ImportSpec) -> Doc {
    let mut parts = vec![text("import ")];
    match &spec.foreign {
        Some((kind, literal)) => {
            parts.push(text(kind.clone()));
            parts.push(text(format!(" {literal}")));
        }
        None => {
            parts.push(text(spec.path.clone()));
            if !spec.names.is_empty() {
                let rows: Vec<Row> = spec
                    .names
                    .iter()
                    .map(|name| Row {
                        elements: vec![Element::Atom(Atom {
                            kind: TokenKind::Word,
                            text: name.clone(),
                            space_before: false,
                        })],
                        comma: false,
                        trailing: None,
                    })
                    .collect();
                parts.push(comma_group_doc('{', '}', rows, false, true, false));
            }
        }
    }
    if let Some(alias) = &spec.alias {
        parts.push(text(format!(" as {alias}")));
    }
    if let Some(comment) = &spec.comment {
        parts.push(text(format!(" {comment}")));
    }
    Doc::Group(Box::new(Doc::Concat(parts)))
}

#[derive(Clone, Debug)]
enum Doc {
    Text(String),
    /// A space when the enclosing group is flat, a line break when it breaks.
    Line,
    /// The break opportunity after a binary operator, tagged with its
    /// precedence so a line only splits at its loosest operator first.
    Break(u8),
    /// Nothing when flat, a line break when broken.
    SoftLine,
    /// A line break in every mode.
    HardLine,
    Concat(Vec<Doc>),
    Nest(usize, Box<Doc>),
    Group(Box<Doc>),
    /// Renders its contents with the indentation of the line it starts on.
    AlignToLine(Box<Doc>),
    IfBreak {
        broken: String,
        flat: String,
    },
    /// A brace that must stay attached to the text before it. It always
    /// closes the current line segment, but its own contents may stay flat.
    HardGroup(Box<Doc>),
    /// Makes the enclosing group break; renders nothing.
    BreakParent,
}

fn text(value: impl Into<String>) -> Doc {
    Doc::Text(value.into())
}

fn flat_width(doc: &Doc) -> Option<usize> {
    match doc {
        Doc::Text(value) => Some(value.chars().count()),
        Doc::Line | Doc::Break(_) => Some(1),
        Doc::SoftLine => Some(0),
        Doc::HardLine | Doc::HardGroup(_) | Doc::BreakParent => None,
        Doc::Concat(parts) => {
            let mut width = 0;
            for part in parts {
                width += flat_width(part)?;
            }
            Some(width)
        }
        Doc::Nest(_, inner) | Doc::Group(inner) | Doc::AlignToLine(inner) => flat_width(inner),
        Doc::IfBreak { flat, .. } => Some(flat.chars().count()),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

struct Renderer {
    output: String,
    column: usize,
    line_indent: usize,
    width: usize,
}

impl Renderer {
    fn new(width: usize) -> Self {
        Self {
            output: String::new(),
            column: 0,
            line_indent: 0,
            width,
        }
    }

    fn render(&mut self, doc: &Doc, indent: usize, mode: Mode) {
        match doc {
            Doc::Text(value) => {
                self.output.push_str(value);
                self.column += value.chars().count();
            }
            Doc::Line | Doc::Break(_) => {
                if mode == Mode::Flat {
                    self.output.push(' ');
                    self.column += 1;
                } else {
                    self.newline(indent);
                }
            }
            Doc::SoftLine => {
                if mode == Mode::Break {
                    self.newline(indent);
                }
            }
            Doc::HardLine => self.newline(indent),
            Doc::Concat(parts) => {
                for part in parts {
                    self.render(part, indent, mode);
                }
            }
            Doc::Nest(extra, inner) => self.render(inner, indent + extra, mode),
            Doc::AlignToLine(inner) => {
                let line = self.line_indent;
                self.render(inner, line, mode);
            }
            Doc::Group(inner) => {
                let flat = mode == Mode::Flat
                    || flat_width(inner)
                        .is_some_and(|width| self.column.saturating_add(width) <= self.width);
                self.render(inner, indent, if flat { Mode::Flat } else { Mode::Break });
            }
            Doc::IfBreak { broken, flat } => {
                let value = if mode == Mode::Break { broken } else { flat };
                self.output.push_str(value);
                self.column += value.chars().count();
            }
            Doc::HardGroup(inner) => self.render(inner, indent, Mode::Break),
            Doc::BreakParent => {}
        }
    }

    fn newline(&mut self, indent: usize) {
        while self.output.ends_with(' ') {
            self.output.pop();
        }
        self.output.push('\n');
        for _ in 0..indent {
            self.output.push(' ');
        }
        self.column = indent;
        self.line_indent = indent;
    }
}

fn render_doc(doc: &Doc, indent: usize) -> String {
    let mut renderer = Renderer::new(MAX_WIDTH);
    for _ in 0..indent {
        renderer.output.push(' ');
    }
    renderer.column = indent;
    renderer.line_indent = indent;
    renderer.render(doc, indent, Mode::Break);
    let rendered = renderer
        .output
        .split('\n')
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    rendered.trim_end_matches('\n').to_owned()
}

fn item_doc(elements: &[Element]) -> Doc {
    if let Some(attributes) = leading_attributes(elements) {
        let mut parts = vec![];
        for (index, attribute) in attributes.iter().enumerate() {
            if index > 0 {
                parts.push(Doc::HardLine);
            }
            parts.push(text(attribute.clone()));
        }
        if attributes.len() * 2 < elements.len() {
            parts.push(Doc::HardLine);
            parts.push(item_doc(&elements[attributes.len() * 2..]));
        }
        return Doc::Concat(parts);
    }
    if let Some(at) = elements.iter().position(|element| {
        matches!(
            element,
            Element::Atom(Atom {
                kind: TokenKind::Comment,
                ..
            })
        )
    }) {
        let before = &elements[..at];
        let comment = match &elements[at] {
            Element::Atom(atom) => atom.text.clone(),
            _ => String::new(),
        };
        let after = &elements[at + 1..];
        if before.is_empty() {
            if after.is_empty() {
                return text(comment);
            }
            return Doc::Concat(vec![
                text(comment),
                Doc::HardLine,
                Doc::Nest(4, Box::new(item_doc(after))),
            ]);
        }
        let mut parts = vec![Doc::Group(Box::new(elements_doc(before)))];
        parts.push(text(format!(" {comment}")));
        if !after.is_empty() {
            parts.push(Doc::HardLine);
            parts.push(Doc::Nest(4, Box::new(item_doc(after))));
        }
        return Doc::Concat(parts);
    }
    Doc::Group(Box::new(elements_doc(elements)))
}

fn leading_attributes(elements: &[Element]) -> Option<Vec<String>> {
    let mut attributes = vec![];
    let mut at = 0;
    while at + 1 < elements.len() {
        match (&elements[at], &elements[at + 1]) {
            (Element::Atom(at_sign), Element::Atom(name))
                if at_sign.text == "@" && name.kind == TokenKind::Word =>
            {
                attributes.push(format!("@{}", name.text));
                at += 2;
            }
            _ => break,
        }
    }
    if attributes.is_empty() {
        None
    } else {
        Some(attributes)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PrevKind {
    Word,
    Number,
    String,
    Format,
    Symbol,
    Close,
}

#[derive(Clone)]
struct Prev {
    text: String,
    kind: PrevKind,
    prefix: bool,
    postfix: bool,
    tight_after: bool,
}

fn elements_doc(elements: &[Element]) -> Doc {
    elements_doc_inner(elements, true)
}

fn elements_doc_inner(elements: &[Element], allow_force: bool) -> Doc {
    let parameter_group = parameter_group_index(elements);
    let force_parameter_break =
        allow_force && parameter_group.is_some_and(|_| signature_too_wide(elements));
    let mut parts = vec![];
    let mut current: Vec<Doc> = vec![];
    let mut previous: Option<Prev> = None;
    let mut after_break = false;
    let mut after_parameter_group = false;
    for (index, element) in elements.iter().enumerate() {
        match element {
            Element::Atom(atom) => {
                if atom.kind == TokenKind::Comment {
                    current.push(text(format!(" {}", atom.text)));
                    continue;
                }
                let at_start = previous.is_none();
                let prefix = is_prefix(&atom.text, previous.as_ref(), at_start);
                let postfix = matches!(atom.text.as_str(), "?" | "++" | "--");
                let colon_after_parameters = atom.text == ":" && after_parameter_group;
                let range_equal =
                    atom.text == "=" && previous.as_ref().is_some_and(|prev| prev.text == "..");
                after_parameter_group = false;
                if atom.text == "else"
                    && previous
                        .as_ref()
                        .is_none_or(|previous| previous.text != "}")
                {
                    current.push(Doc::Break(0));
                    after_break = true;
                }
                let space = !after_break
                    && needs_space(
                        previous.as_ref(),
                        &atom.text,
                        atom.kind,
                        prefix,
                        atom.space_before,
                    );
                after_break = false;
                let mut value = String::new();
                if space {
                    value.push(' ');
                }
                value.push_str(&atom.text);
                current.push(text(value));
                let tight_after = atom.text == ".."
                    || (atom.text == "="
                        && previous.as_ref().is_some_and(|prev| prev.text == ".."));
                if let Some(precedence) = break_precedence(&atom.text)
                    && !prefix
                    && !postfix
                    && !colon_after_parameters
                    && !range_equal
                {
                    current.push(Doc::Break(precedence));
                    after_break = true;
                }
                previous = Some(Prev {
                    text: atom.text.clone(),
                    kind: prev_kind(atom.kind),
                    prefix,
                    postfix,
                    tight_after,
                });
            }
            Element::Group(group) => {
                let open = group.open.to_string();
                let space = !after_break
                    && needs_space(
                        previous.as_ref(),
                        &open,
                        TokenKind::Symbol,
                        false,
                        group.space_before,
                    );
                after_break = false;
                let call_like = group.open == '('
                    && previous.as_ref().is_some_and(|prev| {
                        prev.kind == PrevKind::Close
                            || prev.postfix
                            || (is_word_like(prev) && !is_space_keyword(&prev.text))
                    });
                let force = force_parameter_break && parameter_group == Some(index);
                let block_context = group.open == '{' && follows_block_header(previous.as_ref());
                let doc = group_doc(group, call_like, force, block_context);
                after_parameter_group = group.open == '(';
                if parameter_group == Some(index) {
                    if space {
                        current.push(text(" "));
                    }
                    current.push(doc);
                    flush_segment(&mut parts, &mut current);
                    previous = Some(Prev {
                        text: group.close.to_string(),
                        kind: PrevKind::Close,
                        prefix: false,
                        postfix: false,
                        tight_after: false,
                    });
                    continue;
                }
                if flat_width(&doc).is_none() {
                    flush_segment(&mut parts, &mut current);
                    if space && !parts.is_empty() {
                        parts.push(text(" "));
                    }
                    parts.push(doc);
                } else {
                    if space {
                        current.push(text(" "));
                    }
                    current.push(doc);
                }
                previous = Some(Prev {
                    text: group.close.to_string(),
                    kind: PrevKind::Close,
                    prefix: false,
                    postfix: false,
                    tight_after: false,
                });
            }
            Element::Newline => {}
        }
    }
    flush_segment(&mut parts, &mut current);
    Doc::Concat(parts)
}

/// Whether the declaration's own signature, without its body, is wider than
/// the limit. A parameter list is the first place to break there, before the
/// return type or the proof block.
fn signature_too_wide(elements: &[Element]) -> bool {
    let end = signature_end(elements);
    if end == 0 {
        return false;
    }
    let doc = elements_doc_inner(&elements[..end], false);
    flat_width(&doc).is_some_and(|width| width > MAX_WIDTH)
}

fn signature_end(elements: &[Element]) -> usize {
    let mut at = 0;
    while let Some(Element::Atom(atom)) = elements.get(at) {
        if atom.text == "@" {
            at += 2;
            continue;
        }
        if matches!(
            atom.text.as_str(),
            "public" | "private" | "package" | "static" | "override" | "sealed"
        ) {
            at += 1;
            continue;
        }
        break;
    }
    let theorem = match elements.get(at) {
        Some(Element::Atom(atom)) => matches!(atom.text.as_str(), "theorem" | "example" | "axiom"),
        _ => false,
    };
    for (index, element) in elements.iter().enumerate().skip(at + 1) {
        match element {
            Element::Group(group) if group.open == '{' => return index,
            Element::Atom(atom) if !theorem && matches!(atom.text.as_str(), "=" | ":=") => {
                return index;
            }
            _ => {}
        }
    }
    elements.len()
}

/// The parameter list of a function, theorem, record, enum or constructor.
/// It is its own segment so it can break before the return type does.
fn parameter_group_index(elements: &[Element]) -> Option<usize> {
    let mut at = 0;
    while let Some(Element::Atom(atom)) = elements.get(at) {
        if atom.text == "@" {
            at += 2;
            continue;
        }
        if matches!(
            atom.text.as_str(),
            "public" | "private" | "package" | "static" | "override" | "sealed"
        ) {
            at += 1;
            continue;
        }
        break;
    }
    let first = match elements.get(at) {
        Some(Element::Atom(atom)) => atom.text.as_str(),
        _ => return None,
    };
    if !matches!(
        first,
        "function" | "theorem" | "example" | "axiom" | "record" | "enum" | "new"
    ) {
        return None;
    }
    elements
        .iter()
        .enumerate()
        .skip(at + 1)
        .find_map(|(index, element)| match element {
            Element::Group(group) if group.open == '(' => Some(index),
            _ => None,
        })
}

fn flush_segment(parts: &mut Vec<Doc>, current: &mut Vec<Doc>) {
    if current.is_empty() {
        return;
    }
    // A break at the end separates this segment from a following hard group.
    // The group supplies the line, so the break becomes the space before it.
    if matches!(current.last(), Some(Doc::Break(_) | Doc::Line)) {
        current.pop();
        current.push(text(" "));
    }
    let content = std::mem::take(current);
    parts.push(Doc::Group(Box::new(assemble_segment(content))));
}

/// Splits a segment at its loosest operator first, then fills lines greedily.
/// Higher-precedence operators only break when their own operand still does
/// not fit, so `n >= 0 && ...` breaks at `&&`, not after `>=`.
fn assemble_segment(parts: Vec<Doc>) -> Doc {
    let Some(lowest) = parts
        .iter()
        .filter_map(|part| match part {
            Doc::Break(precedence) => Some(*precedence),
            _ => None,
        })
        .min()
    else {
        return Doc::Concat(parts);
    };
    let mut operands: Vec<Vec<Doc>> = vec![vec![]];
    for part in parts {
        match part {
            Doc::Break(precedence) if precedence == lowest => operands.push(vec![]),
            other => operands.last_mut().unwrap().push(other),
        }
    }
    let mut docs: Vec<Doc> = operands.into_iter().map(assemble_segment).collect();
    if docs.len() == 1 {
        docs.remove(0)
    } else {
        Doc::Nest(4, Box::new(fill(docs.split_off(0))))
    }
}

/// Greedy line filling at one precedence level. The caller nests the whole
/// sequence once, so every line at this level aligns at the same indent
/// instead of stepping further right with each `else`.
fn fill(mut docs: Vec<Doc>) -> Doc {
    if docs.len() == 1 {
        return docs.remove(0);
    }
    let first = docs.remove(0);
    let rest = fill(docs);
    Doc::Group(Box::new(Doc::Concat(vec![first, Doc::Line, rest])))
}

fn prev_kind(kind: TokenKind) -> PrevKind {
    match kind {
        TokenKind::Word => PrevKind::Word,
        TokenKind::Integer | TokenKind::Double => PrevKind::Number,
        TokenKind::String => PrevKind::String,
        TokenKind::FormatString => PrevKind::Format,
        _ => PrevKind::Symbol,
    }
}

fn is_word_like(prev: &Prev) -> bool {
    matches!(
        prev.kind,
        PrevKind::Word | PrevKind::Number | PrevKind::String | PrevKind::Format | PrevKind::Close
    ) || prev.text.starts_with('\\')
}

fn value_end(previous: Option<&Prev>) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    if previous.kind == PrevKind::Close || previous.postfix {
        return true;
    }
    if previous.prefix {
        return false;
    }
    match previous.kind {
        PrevKind::Word => !is_space_keyword(&previous.text),
        PrevKind::Number | PrevKind::String | PrevKind::Format => true,
        PrevKind::Close => true,
        PrevKind::Symbol => previous.text.starts_with('\\') || previous.text == "ᶜ",
    }
}

fn is_prefix(value: &str, previous: Option<&Prev>, at_start: bool) -> bool {
    if !matches!(value, "-" | "+" | "!" | "¬" | "&" | "@" | "~") {
        return false;
    }
    at_start || !value_end(previous)
}

fn needs_space(
    previous: Option<&Prev>,
    value: &str,
    kind: TokenKind,
    prefix: bool,
    space_before: bool,
) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    if matches!(
        value,
        ")" | "]" | "," | ";" | "." | ".." | "?" | "ᶜ" | "++" | "--"
    ) {
        return false;
    }
    if matches!(previous.text.as_str(), "(" | "[" | "." | "..") {
        return false;
    }
    if previous.postfix {
        return false;
    }
    if previous.prefix {
        // A prefix operator stays attached, except where the two tokens would
        // lex as one: `- -1`, `+ +1`, `& &x`, `! =` and the compound pairs.
        return merges(&previous.text, value);
    }
    if value == ":" || previous.text == ":" {
        return true;
    }
    if value == "{" {
        return true;
    }
    if value == "(" {
        if previous.kind == PrevKind::Close || previous.postfix {
            return false;
        }
        if is_space_keyword(&previous.text) {
            return true;
        }
        return if is_word_like(previous) {
            space_before
        } else {
            true
        };
    }
    if value == "[" {
        if previous.kind == PrevKind::Close || previous.postfix {
            return false;
        }
        if is_space_keyword(&previous.text) {
            return true;
        }
        return if is_word_like(previous) {
            space_before
        } else {
            true
        };
    }
    if previous.tight_after {
        return false;
    }
    if prefix
        && matches!(
            previous.kind,
            PrevKind::Word | PrevKind::Number | PrevKind::String | PrevKind::Format
        )
    {
        return true;
    }
    let _ = kind;
    true
}

fn merges(left: &str, right: &str) -> bool {
    matches!(
        (left, right),
        ("-", "-") | ("-", "=") | ("+", "+") | ("+", "=") | ("!", "=") | ("&", "&") | ("&", "=")
    )
}

fn break_precedence(value: &str) -> Option<u8> {
    Some(match value {
        "=" | ":=" | "=>" | "|>" => 0,
        "->" | "→" => 2,
        "||" | "or" | "∨" => 3,
        "&&" | "and" | "∧" => 4,
        "|" => 5,
        "^" => 6,
        "&" => 7,
        "==" | "!=" | "<" | "<=" | ">" | ">=" | "≠" | "≤" | "≥" | "∈" | "∉" | "⊆" | "∪" => {
            8
        }
        "∩" | "<<" | ">>" => 9,
        "+" | "-" => 10,
        "*" | "/" | "%" | "@" => 11,
        "**" => 12,
        _ => return None,
    })
}

fn is_space_keyword(value: &str) -> bool {
    matches!(
        value,
        "if" | "while"
            | "for"
            | "match"
            | "return"
            | "then"
            | "else"
            | "in"
            | "and"
            | "or"
            | "not"
            | "requires"
            | "ensures"
            | "with"
            | "has"
            | "move"
            | "mut"
            | "final"
            | "var"
            | "function"
            | "class"
            | "trait"
            | "enum"
            | "record"
            | "alias"
            | "const"
            | "theorem"
            | "example"
            | "axiom"
            | "opaque"
            | "import"
            | "public"
            | "private"
            | "package"
            | "static"
            | "override"
            | "sealed"
            | "infixl"
            | "infixr"
            | "infix"
            | "type"
            | "as"
            | "otherwise"
            | "using"
            | "unsafe"
            | "scope"
            | "spawn"
            | "let"
            | "break"
            | "continue"
    )
}

#[derive(Clone, Debug)]
struct Row {
    elements: Vec<Element>,
    comma: bool,
    /// A `//` comment that followed the comma on the same source line.
    trailing: Option<String>,
}

fn split_rows(elements: &[Element]) -> Vec<Row> {
    let mut rows = vec![];
    let mut current = vec![];
    let mut quantifier = false;
    let mut at = 0;
    while at < elements.len() {
        match &elements[at] {
            Element::Newline => {
                let comment_last = matches!(
                    current.last(),
                    Some(Element::Atom(Atom {
                        kind: TokenKind::Comment,
                        ..
                    }))
                );
                if comment_last && !current.is_empty() {
                    rows.push(Row {
                        elements: std::mem::take(&mut current),
                        comma: false,
                        trailing: None,
                    });
                }
                at += 1;
            }
            Element::Atom(atom)
                if matches!(
                    atom.text.as_str(),
                    "forall" | "exists" | "∀" | "∃" | "\\forall" | "\\exists"
                ) =>
            {
                quantifier = true;
                current.push(elements[at].clone());
                at += 1;
            }
            Element::Atom(atom) if atom.text == "," => {
                if quantifier {
                    // `forall A : Set[U], P` has a comma that belongs to the
                    // binder, not to the surrounding parameter or call list.
                    quantifier = false;
                    current.push(elements[at].clone());
                    at += 1;
                } else if !current.is_empty() {
                    let mut trailing = None;
                    at += 1;
                    if let Some(Element::Atom(comment)) = elements.get(at)
                        && comment.kind == TokenKind::Comment
                    {
                        trailing = Some(comment.text.clone());
                        at += 1;
                    }
                    rows.push(Row {
                        elements: std::mem::take(&mut current),
                        comma: true,
                        trailing,
                    });
                } else {
                    at += 1;
                }
            }
            element => {
                current.push(element.clone());
                at += 1;
            }
        }
    }
    if !current.is_empty() {
        rows.push(Row {
            elements: current,
            comma: false,
            trailing: None,
        });
    }
    rows
}

fn is_comment_row(row: &Row) -> bool {
    row.elements.iter().all(|element| match element {
        Element::Atom(atom) => atom.kind == TokenKind::Comment,
        Element::Newline => true,
        Element::Group(_) => false,
    })
}

fn row_has_comment(row: &Row) -> bool {
    row.trailing.is_some()
        || row.elements.iter().any(|element| match element {
            Element::Atom(atom) => atom.kind == TokenKind::Comment,
            _ => false,
        })
}

fn group_doc(group: &Group, call_like: bool, force_break: bool, block_context: bool) -> Doc {
    match group.open {
        '(' | '[' => {
            let rows = split_rows(&group.children);
            let trailing = rows.len() > 1
                || rows.last().is_some_and(|row| row.comma)
                || (group.open == '(' && call_like);
            comma_group_doc(group.open, group.close, rows, false, trailing, force_break)
        }
        _ => {
            if only_newlines(&group.children) {
                return text("{}");
            }
            if is_match_like(group) {
                let rows = split_rows(&group.children);
                return comma_group_doc('{', '}', rows, true, true, false);
            }
            if group.multiline {
                return Doc::HardGroup(Box::new(block_doc(group)));
            }
            let rows = split_rows(&group.children);
            let trailing = rows.len() > 1 || rows.last().is_some_and(|row| row.comma);
            let inner = comma_group_doc('{', '}', rows, true, trailing, false);
            if block_context || is_statement_block(group) {
                Doc::HardGroup(Box::new(inner))
            } else {
                inner
            }
        }
    }
}

fn only_newlines(elements: &[Element]) -> bool {
    elements
        .iter()
        .all(|element| matches!(element, Element::Newline))
}

fn is_match_like(group: &Group) -> bool {
    let mut arrow = false;
    let mut comma = false;
    for element in &group.children {
        if let Element::Atom(atom) = element {
            if atom.text == "=>" {
                arrow = true;
            }
            if atom.text == "," {
                comma = true;
            }
            if matches!(
                atom.text.as_str(),
                "let"
                    | "return"
                    | "if"
                    | "for"
                    | "while"
                    | "match"
                    | "break"
                    | "continue"
                    | "function"
                    | "class"
                    | "trait"
                    | "enum"
                    | "record"
                    | "alias"
                    | "const"
                    | "theorem"
                    | "example"
                    | "axiom"
                    | "public"
                    | "private"
                    | "var"
                    | "final"
                    | "new"
                    | "has"
            ) {
                return false;
            }
        }
    }
    arrow && comma
}

/// Whether a `{` follows the header of a block: a condition, a match
/// scrutinee, a declaration name, or `else`/`by`/`=>`. A `{` after `=`, `:=`,
/// `(`, `[` or a comma is a set or record literal instead.
fn follows_block_header(previous: Option<&Prev>) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    if matches!(
        previous.text.as_str(),
        "else" | "=>" | "by" | "try" | "repeat" | "do" | "otherwise"
    ) {
        return true;
    }
    if previous.kind == PrevKind::Close || previous.postfix {
        return true;
    }
    if matches!(
        previous.kind,
        PrevKind::Number | PrevKind::String | PrevKind::Format
    ) {
        return true;
    }
    previous.kind == PrevKind::Word && !is_space_keyword(&previous.text)
}

/// A brace that holds statements or tactics rather than a set or record
/// literal. It stays attached to the text before it, even when it is written
/// on one line, so `Zero => { cases ... }` never breaks between `=>` and `{`.
fn is_statement_block(group: &Group) -> bool {
    group.children.iter().any(|element| match element {
        Element::Atom(atom) => matches!(
            atom.text.as_str(),
            "let"
                | "return"
                | "if"
                | "for"
                | "while"
                | "match"
                | "break"
                | "continue"
                | "function"
                | "class"
                | "trait"
                | "enum"
                | "record"
                | "alias"
                | "const"
                | "theorem"
                | "example"
                | "axiom"
                | "public"
                | "private"
                | "package"
                | "static"
                | "override"
                | "sealed"
                | "var"
                | "final"
                | "new"
                | "has"
                | "invariant"
                | "type"
                | "import"
                | "cases"
                | "induction"
                | "strongInduction"
                | "simp"
                | "rw"
                | "rewrite"
                | "repeat"
                | "try"
                | "skip"
                | "have"
                | "calc"
                | "intro"
                | "intros"
                | "apply"
                | "exact"
                | "constructor"
                | "left"
                | "right"
                | "exists"
                | "use"
                | "trans"
                | "symm"
                | "contradiction"
                | "assumption"
                | "decide"
                | "smt"
                | "rfl"
                | "unfold"
                | "subst"
                | "clear"
                | "revert"
                | "generalize"
                | "specialize"
                | "congrArg"
                | "byCases"
                | "ext"
        ),
        Element::Group(_) | Element::Newline => false,
    })
}

/// A group with comma-separated rows. `padded` puts one space inside the
/// delimiters when the group is flat; `trailing` allows a trailing comma when
/// the group breaks. A comma is never invented where it would change the tree:
/// a one-element parenthesized expression must not become a tuple.
fn comma_group_doc(
    open: char,
    close: char,
    rows: Vec<Row>,
    padded: bool,
    trailing: bool,
    force_break: bool,
) -> Doc {
    if rows.is_empty() {
        return text(format!("{open}{close}"));
    }
    let has_comments = rows.iter().any(row_has_comment);
    let last_code = rows.iter().rposition(|row| !is_comment_row(row));
    let mut body = vec![];
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            body.push(Doc::Line);
        }
        body.push(item_doc(&row.elements));
        if Some(index) == last_code {
            if rows.len() == 1 && row.comma {
                body.push(text(","));
            } else if trailing {
                body.push(Doc::IfBreak {
                    broken: ",".into(),
                    flat: String::new(),
                });
            }
        } else if !is_comment_row(row) {
            body.push(text(","));
        }
        if let Some(comment) = &row.trailing {
            body.push(text(format!(" {comment}")));
        }
    }
    let padding = if padded {
        vec![Doc::IfBreak {
            broken: String::new(),
            flat: " ".into(),
        }]
    } else {
        vec![]
    };
    let closing_padding = if padded {
        vec![Doc::IfBreak {
            broken: String::new(),
            flat: " ".into(),
        }]
    } else {
        vec![]
    };
    let mut parts = vec![text(open.to_string())];
    parts.extend(padding);
    parts.push(Doc::Nest(
        4,
        Box::new(Doc::Concat(vec![Doc::SoftLine, Doc::Concat(body)])),
    ));
    parts.extend(closing_padding);
    parts.push(Doc::SoftLine);
    parts.push(text(close.to_string()));
    if has_comments || force_break {
        parts.push(Doc::BreakParent);
    }
    Doc::Group(Box::new(Doc::AlignToLine(Box::new(Doc::Concat(parts)))))
}

fn block_doc(group: &Group) -> Doc {
    let items = split_items(&group.children);
    if items.is_empty() {
        return text("{}");
    }
    let mut body = vec![];
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            body.push(Doc::HardLine);
            if item.blank_before > 0 {
                body.push(Doc::HardLine);
            }
        }
        body.push(item_doc(&item.elements));
    }
    Doc::Concat(vec![
        text("{"),
        Doc::AlignToLine(Box::new(Doc::Concat(vec![
            Doc::Nest(
                4,
                Box::new(Doc::Concat(vec![Doc::HardLine, Doc::Concat(body)])),
            ),
            Doc::HardLine,
            text("}"),
        ]))),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(source: &str) -> String {
        format_text(source).unwrap_or_else(|diagnostics| {
            panic!(
                "format failed: {:?}",
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code)
                    .collect::<Vec<_>>()
            )
        })
    }

    fn same(source: &str, expected: &str) {
        let once = format(source);
        assert_eq!(once, expected);
        assert_eq!(format(&once), expected, "formatting is not idempotent");
    }

    fn tokens(source: &str) -> Vec<(TokenKind, String)> {
        lex(source)
            .tokens
            .into_iter()
            .filter(|token| {
                !token.is_trivia() && !matches!(token.kind, TokenKind::Newline | TokenKind::Eof)
            })
            .map(|token| (token.kind, source[token.span.range()].to_owned()))
            .collect()
    }

    #[test]
    fn keeps_tokens_and_comments() {
        let source = "let  x:=1 // one\n// two\nlet y := 2\n";
        let formatted = format(source);
        assert_eq!(tokens(source), tokens(&formatted));
        assert!(formatted.contains("// one"));
        assert!(formatted.contains("// two"));
    }

    #[test]
    fn basic_spacing() {
        same("let  x:=1\n", "let x := 1\n");
        same("let y:int=2\n", "let y : int = 2\n");
        same("f( 1,2 )\n", "f(1, 2)\n");
        same("let a:=f()?.b\n", "let a := f()?.b\n");
    }

    #[test]
    fn one_item_per_line_when_it_does_not_fit() {
        let source = "function many(first : Int, second : Int, third : Int, fourth : Int, fifth : Int, sixth : Int, seventh : Int, eighth : Int) : Int = first + second + third + fourth + fifth + sixth + seventh + eighth\n";
        let formatted = format(source);
        assert!(formatted.contains("function many(\n    first : Int,\n"));
        assert!(
            formatted.contains("    eighth : Int,\n) : Int = first + second"),
            "{formatted}"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn match_arms_break_at_commas() {
        let source = "let x := match value {\n    FirstPatternName => firstVeryLongName, SecondPatternName => secondVeryLongName, ThirdPatternName => thirdVeryLongName, FourthPatternName => fourthVeryLongName\n}\n";
        let formatted = format(source);
        assert!(
            formatted.contains("    FirstPatternName => firstVeryLongName,\n"),
            "{formatted}"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn comments_keep_their_own_lines() {
        same(
            "let x := 1 // note\nlet y := 2\n",
            "let x := 1 // note\nlet y := 2\n",
        );
        same(
            "f(\n    1, // note\n    2,\n)\n",
            "f(\n    1, // note\n    2,\n)\n",
        );
    }

    #[test]
    fn braces_keep_block_lines() {
        same(
            "function f() : Int {\n\n    let x := 1\n\n    return x\n}\n",
            "function f() : Int {\n    let x := 1\n\n    return x\n}\n",
        );
    }

    #[test]
    fn imports_are_sorted() {
        same(
            "import std.random.{Rng}\nimport std.math.{sin, cos}\n\nlet x := 1\n",
            "import std.math.{cos, sin}\nimport std.random.{Rng}\n\nlet x := 1\n",
        );
    }

    #[test]
    fn literals_are_not_rewritten() {
        let source = "let a := 1_000_000\nlet b := 0xFF\nlet c := 1E3\nlet d := \"a\\\\nb\"\nlet e := f\"{a:.2f}\"\n";
        let formatted = format(source);
        for literal in ["1_000_000", "0xFF", "1E3", "\"a\\\\nb\"", "f\"{a:.2f}\""] {
            assert!(formatted.contains(literal), "{literal} in {formatted}");
        }
    }

    #[test]
    fn declarations_get_one_blank_line() {
        same(
            "function a() : Int = 1\nfunction b() : Int = 2\n",
            "function a() : Int = 1\n\nfunction b() : Int = 2\n",
        );
    }
}

#[cfg(test)]
mod construct_tests {
    use super::*;

    fn format(source: &str) -> String {
        format_text(source).unwrap()
    }

    fn same(source: &str, expected: &str) {
        let once = format(source);
        assert_eq!(once, expected);
        assert_eq!(format(&once), expected, "formatting is not idempotent");
    }

    #[test]
    fn declarations_and_records_stay_compact() {
        same(
            "record Rectangle(width:Int,height:Int)\n",
            "record Rectangle(width : Int, height : Int)\n",
        );
        same(
            "public class Stack[T] {\nprivate var items:Array[T]\nnew() { this.items = Array.empty() }\n}\n",
            "public class Stack[T] {\n    private var items : Array[T]\n    new() { this.items = Array.empty() }\n}\n",
        );
    }

    #[test]
    fn enums_keep_their_variants() {
        same("enum Z2 { Zero, One }\n", "enum Z2 { Zero, One }\n");
        same(
            "enum Direction {\nNorth\nSouth\n}\n",
            "enum Direction {\n    North\n    South\n}\n",
        );
    }

    #[test]
    fn calls_and_named_arguments() {
        same("print(f(1,2))\n", "print(f(1, 2))\n");
        same(
            "scaled(scale=3.0,value=2.0)\n",
            "scaled(scale = 3.0, value = 2.0)\n",
        );
        same("let x := f(a=1,b=2)\n", "let x := f(a = 1, b = 2)\n");
    }

    #[test]
    fn lists_comprehensions_and_sets() {
        same("let xs:=[1,2,3]\n", "let xs := [1, 2, 3]\n");
        same(
            "let ys := [x * 2 for x in xs if x > 0]\n",
            "let ys := [x * 2 for x in xs if x > 0]\n",
        );
        same("let s := {a, b}\n", "let s := { a, b }\n");
        same("let s := {x : U | x = a}\n", "let s := { x : U | x = a }\n");
    }

    #[test]
    fn match_arms_and_guards() {
        same(
            "function sign(n:Int):String = match n {\n-10..0 => \"negative\",\n0 => \"zero\",\n_ => \"positive\",\n}\n",
            "function sign(n : Int) : String = match n { -10..0 => \"negative\", 0 => \"zero\", _ => \"positive\" }\n",
        );
        same(
            "let x := match n { A if ready => 1, B => 2 }\n",
            "let x := match n { A if ready => 1, B => 2 }\n",
        );
    }

    #[test]
    fn if_then_else_and_loops() {
        same(
            "if x>0 { print(x) } else { print(0) }\n",
            "if x > 0 { print(x) } else { print(0) }\n",
        );
        same(
            "for (x in xs) { print(x) }\n",
            "for (x in xs) { print(x) }\n",
        );
        same("while n>0 { n = n - 1 }\n", "while n > 0 { n = n - 1 }\n");
    }

    #[test]
    fn binary_expressions_break_after_the_loosest_operator() {
        let source = "let ok := firstCondition && secondCondition && thirdCondition && fourthCondition && fifthCondition && sixthCondition\n";
        let formatted = format(source);
        assert!(formatted.contains("firstCondition &&\n"), "{formatted}");
        assert!(
            formatted
                .lines()
                .all(|line| line.chars().count() <= MAX_WIDTH),
            "{formatted}"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn unary_prefix_and_postfix() {
        same(
            "let a := -1\nlet b := !ready\nlet c := ~mask\nlet d := f()?\n",
            "let a := -1\nlet b := !ready\nlet c := ~mask\nlet d := f()?\n",
        );
    }

    #[test]
    fn ranges_stay_tight() {
        same(
            "let r := 1..9\nlet p := 0..=9\n",
            "let r := 1..9\nlet p := 0..=9\n",
        );
    }

    #[test]
    fn compound_assignments_are_not_expanded() {
        same("x += 1\nx -= 2\ny++\nz--\n", "x += 1\nx -= 2\ny++\nz--\n");
    }

    #[test]
    fn proofs_and_tactics_keep_their_lines() {
        same(
            "theorem t(n:Nat) : n = n by {\ninduction n {\nZero => rfl\nSucc(k) ih => rfl\n}\n}\n",
            "theorem t(n : Nat) : n = n by {\n    induction n {\n        Zero => rfl\n        Succ(k) ih => rfl\n    }\n}\n",
        );
    }

    #[test]
    fn attributes_get_their_own_line() {
        same(
            "@simp theorem t(n : Nat) : n = n { rfl }\n",
            "@simp\ntheorem t(n : Nat) : n = n { rfl }\n",
        );
    }

    #[test]
    fn format_strings_are_untouched() {
        let source =
            "let text := f\"{value:.2f} and {other}\"\nlet plain := \"a\\\\nb // not a comment\"\n";
        let formatted = format(source);
        assert!(
            formatted.contains("f\"{value:.2f} and {other}\""),
            "{formatted}"
        );
        assert!(
            formatted.contains("\"a\\\\nb // not a comment\""),
            "{formatted}"
        );
    }

    #[test]
    fn comments_stay_attached() {
        same(
            "/// A rectangle.\nrecord Rectangle(width:Int,height:Int)\n",
            "/// A rectangle.\nrecord Rectangle(width : Int, height : Int)\n",
        );
        same(
            "// banner\n\nfunction f():Int=1\n",
            "// banner\n\nfunction f() : Int = 1\n",
        );
        same(
            "f(\n1, // first\n2,\n)\n",
            "f(\n    1, // first\n    2,\n)\n",
        );
    }

    #[test]
    fn blank_lines_are_canonical() {
        same(
            "function a():Int=1\n\n\n\nfunction b():Int=2\n",
            "function a() : Int = 1\n\nfunction b() : Int = 2\n",
        );
        same(
            "function a():Int {\n\n\nlet x:=1\n\n\nreturn x\n}\n",
            "function a() : Int {\n    let x := 1\n\n    return x\n}\n",
        );
    }

    #[test]
    fn imports_sort_names_and_declarations() {
        same(
            "import std.random.{Rng}\nimport std.math.{sin, cos}\n\n",
            "import std.math.{cos, sin}\nimport std.random.{Rng}\n",
        );
        same(
            "import std.math as m\nimport c \"stdlib.h\"\n",
            "import c \"stdlib.h\"\nimport std.math as m\n",
        );
    }

    #[test]
    fn type_arrows_and_applications() {
        same(
            "let f : (A, B) -> C = g\nlet xs : List[Int] = []\n",
            "let f : (A, B) -> C = g\nlet xs : List[Int] = []\n",
        );
    }

    #[test]
    fn long_parameter_lists_break_one_per_line() {
        let source = "public function combine(first : Int, second : Int, third : Int, fourth : Int, fifth : Int, sixth : Int, seventh : Int, eighth : Int) : Int = first + second + third + fourth + fifth + sixth + seventh + eighth\n";
        let formatted = format(source);
        assert!(
            formatted.contains("public function combine(\n    first : Int,\n"),
            "{formatted}"
        );
        assert!(
            formatted.contains("    eighth : Int,\n) : Int = first + second"),
            "{formatted}"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn import_runs_split_on_blank_lines_and_comments() {
        same(
            "import std.z\n\nimport std.a\n",
            "import std.z\n\nimport std.a\n",
        );
        same(
            "import std.z\n// middle\nimport std.a\n",
            "import std.z\n// middle\nimport std.a\n",
        );
        same(
            "import std.z // last\nimport std.a // first\n",
            "import std.a // first\nimport std.z // last\n",
        );
    }

    #[test]
    fn a_blank_line_between_docs_and_a_declaration_stays() {
        same(
            "/// not attached\n\nfunction f() : Int = 1\n",
            "/// not attached\n\nfunction f() : Int = 1\n",
        );
        same(
            "/// attached\nfunction f() : Int = 1\n",
            "/// attached\nfunction f() : Int = 1\n",
        );
    }

    #[test]
    fn comments_inside_expressions_survive() {
        let source = "let value := compute(\n    first, // the first part\n    second,\n)\n";
        let formatted = format(source);
        assert!(
            formatted.contains("    first, // the first part\n"),
            "{formatted}"
        );
        assert_eq!(format(&formatted), formatted);
    }
}
