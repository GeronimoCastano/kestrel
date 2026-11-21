//! Sessions keep accepted source for checking and execute each native entry once.
use crate::{
    core,
    diagnostic::Diagnostic,
    lexer::{self, TokenKind},
    modules, native, parse_file,
    source::{SourceFile, Span},
    syntax::{Node, SyntaxKind as K},
    types::Type,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Default)]
struct History {
    source: String,
    original: String,
    offsets: Vec<usize>,
    aliases: BTreeMap<String, String>,
    definitions: BTreeMap<String, String>,
    names: BTreeMap<String, String>,
    serial: usize,
    value_names: BTreeSet<String>,
    /// Diagnostics count lines from this offset of `original` (a notebook cell's start).
    view_start: Option<usize>,
}
impl History {
    /// Keeps an aborted entry's lines as blank lines, so later entries report the
    /// line numbers the user typed (REVIEW2 section 2).
    fn pad(&mut self, text: &str) {
        let lines = text.lines().count().max(1);
        for _ in 0..lines {
            self.offsets.push(self.original.len());
            self.original.push('\n');
            self.source.push('\n');
        }
    }
    fn readable(&self, text: &str) -> String {
        let mut aliases = self.names.iter().collect::<Vec<_>>();
        aliases.sort_by_key(|(internal, _)| std::cmp::Reverse(internal.len()));
        let mut result = text.to_owned();
        for (internal, name) in aliases {
            result = result.replace(internal, name);
        }
        result
    }
    fn diagnostic(&self, diagnostic: &Diagnostic) -> String {
        let mut diagnostic = diagnostic.clone();
        let offset = |at: usize| self.offsets.get(at).copied().unwrap_or(self.original.len());
        diagnostic.span = Span::new(offset(diagnostic.span.start), offset(diagnostic.span.end));
        diagnostic.message = self.readable(&diagnostic.message);
        diagnostic.hint = diagnostic.hint.map(|s| self.readable(&s));
        if let Some(view) = self.view_start
            && diagnostic.span.start >= view
        {
            diagnostic.span = Span::new(
                diagnostic.span.start - view,
                diagnostic.span.end.saturating_sub(view),
            );
            return diagnostic.render(&SourceFile::new("<cell>", self.original[view..].to_owned()));
        }
        diagnostic.render(&SourceFile::new("<session>", self.original.clone()))
    }
    fn remap_locations(&self, program: &mut core::Program) {
        fn visit(
            history: &History,
            code: &mut [core::Statement],
            session: &SourceFile,
            cell: Option<&SourceFile>,
        ) {
            for statement in code {
                match statement {
                    core::Statement::Set(_, _, location)
                    | core::Statement::Store(_, _, _, location)
                    | core::Statement::Evaluate(_, location)
                    | core::Statement::Spawn { location, .. }
                        if location.file == "<session>" =>
                    {
                        let offset = |at: usize| {
                            history
                                .offsets
                                .get(at)
                                .copied()
                                .unwrap_or(history.original.len())
                        };
                        let start = offset(location.span.start);
                        let end = offset(location.span.end);
                        // A notebook cell's statements name their own cell, so a runtime
                        // message counts lines from the cell like a compile error does.
                        let (source, base) = match (cell, history.view_start) {
                            (Some(cell), Some(view)) if start >= view => (cell, view),
                            _ => (session, 0),
                        };
                        let span = Span::new(start - base, end.saturating_sub(base));
                        let (line, column) = source.position(span.start);
                        location.span = span;
                        location.file = source.name.to_owned();
                        location.line = line;
                        location.column = column;
                        location.expression = source.slice(span).to_owned();
                    }
                    core::Statement::If(_, left, right) => {
                        visit(history, left, session, cell);
                        visit(history, right, session, cell);
                    }
                    core::Statement::Match(_, arms) => {
                        for arm in arms {
                            visit(history, &mut arm.test, session, cell);
                            visit(history, &mut arm.bindings, session, cell);
                            if let Some((code, _)) = &mut arm.guard {
                                visit(history, code, session, cell);
                            }
                            visit(history, &mut arm.body, session, cell);
                            visit(history, &mut arm.cleanup, session, cell);
                        }
                    }
                    core::Statement::Loop {
                        condition,
                        body,
                        step,
                        ..
                    } => {
                        visit(history, condition, session, cell);
                        visit(history, body, session, cell);
                        visit(history, step, session, cell);
                    }
                    _ => {}
                }
            }
        }
        let session = SourceFile::new("<session>", self.original.clone());
        let cell = self
            .view_start
            .map(|view| SourceFile::new("<cell>", self.original[view..].to_owned()));
        for function in &mut program.functions {
            visit(self, &mut function.body, &session, cell.as_ref());
        }
    }
    fn add(&mut self, text: &str) -> Result<usize, String> {
        // A notebook cell is parsed on its own, so its positions already count from the cell.
        let name = if self.view_start.is_some() {
            "<cell>"
        } else {
            "<session>"
        };
        let parsed = crate::parse_file_with_values(name, text, &self.value_names);
        if !parsed.is_ok() {
            return Err(parsed
                .diagnostics
                .iter()
                .map(|d| d.render(&parsed.source))
                .collect());
        }
        for token in &parsed.tokens {
            let name = parsed.source.slice(token.span);
            if token.kind == TokenKind::Word
                && (name.starts_with("session__") || name.starts_with("Session__"))
            {
                return Err(Diagnostic::new(
                    "E_NAMING",
                    "This name does not use Kestrel's naming rules.",
                    token.span,
                )
                .render(&parsed.source));
            }
        }
        let mut edits = BTreeMap::new();
        for node in &parsed.root.children {
            let mut locals = BTreeSet::new();
            if node.kind == K::Import {
                if node.child(K::ImportNames).is_none()
                    && node
                        .children
                        .first()
                        .is_some_and(|head| !matches!(&text[head.span.range()], "c" | "python"))
                {
                    let path = lexer::path_name(&text[node.children[0].span.range()]);
                    let original = node
                        .children
                        .get(1)
                        .map(|name| text[name.span.range()].to_owned())
                        .unwrap_or_else(|| path.rsplit('.').next().unwrap().to_owned());
                    self.serial += 1;
                    let internal = format!("session__{}_{original}", self.serial);
                    if let Some(alias) = node.children.get(1) {
                        edits.insert(alias.span, internal.clone());
                    } else {
                        edits.insert(
                            Span::new(node.span.end, node.span.end),
                            format!(" as {internal}"),
                        );
                    }
                    self.aliases.insert(original.clone(), internal.clone());
                    self.names.insert(internal, original);
                    continue;
                }
                let selected = node
                    .child(K::ImportNames)
                    .map(|n| n.children.iter().collect::<Vec<_>>())
                    .unwrap_or_else(|| {
                        if node
                            .children
                            .first()
                            .is_some_and(|n| &text[n.span.range()] == "python")
                        {
                            node.children
                                .last()
                                .filter(|n| n.kind == K::Name)
                                .into_iter()
                                .collect()
                        } else {
                            vec![]
                        }
                    });
                for name in selected {
                    let original = &text[name.span.range()];
                    self.serial += 1;
                    let internal = format!(
                        "{}{}_{original}",
                        if original
                            .chars()
                            .next()
                            .is_some_and(crate::identifiers::type_start)
                        {
                            "Session__"
                        } else {
                            "session__"
                        },
                        self.serial
                    );
                    edits.insert(name.span, internal.clone());
                    self.aliases.insert(original.to_owned(), internal.clone());
                    self.names.insert(internal, original.to_owned());
                }
            } else if matches!(node.kind, K::Binding | K::Const) {
                let pattern = node
                    .children
                    .iter()
                    .find(|n| is_pattern(n.kind))
                    .or_else(|| node.child(K::Name))
                    .ok_or("This binding has no name.")?;
                for child in node.children.iter().filter(|n| n.span != pattern.span) {
                    walk(child, text, &self.aliases, &mut locals, &mut edits);
                }
                let names = if node.kind == K::Const {
                    pattern
                        .descendants()
                        .find(|n| n.kind == K::Name)
                        .into_iter()
                        .collect()
                } else if pattern.kind == K::Name {
                    vec![pattern]
                } else {
                    pattern_names(pattern)
                };
                for name in names {
                    self.define(name, text, node, &mut edits)?;
                }
                // Constructor names in a destructuring pattern still resolve in the old environment.
                if node.kind != K::Const {
                    walk_pattern(
                        pattern,
                        text,
                        &self.aliases,
                        &mut BTreeSet::new(),
                        &mut edits,
                    );
                }
            } else if matches!(
                node.kind,
                K::Function | K::Class | K::Trait | K::Enum | K::Alias | K::Const
            ) {
                let name = if node.kind == K::Const {
                    node.descendants().find(|n| n.kind == K::Name)
                } else {
                    node.child(K::Name)
                };
                if let Some(name) = name {
                    self.define(name, text, node, &mut edits)?;
                }
                walk(node, text, &self.aliases, &mut locals, &mut edits);
            } else {
                walk(node, text, &self.aliases, &mut locals, &mut edits);
            }
        }
        let start = self.source.len();
        let original_start = self.original.len();
        self.original.push_str(text);
        let separator = !text.ends_with('\n');
        if separator {
            self.original.push('\n');
        }
        let mut cursor = 0;
        for (span, replacement) in edits {
            if span.start < cursor {
                continue;
            }
            for at in cursor..span.start {
                self.offsets.push(original_start + at);
            }
            self.source.push_str(&text[cursor..span.start]);
            for _ in replacement.bytes() {
                self.offsets.push(original_start + span.start);
            }
            self.source.push_str(&replacement);
            cursor = span.end;
        }
        for at in cursor..text.len() {
            self.offsets.push(original_start + at);
        }
        self.source.push_str(&text[cursor..]);
        if separator {
            self.source.push('\n');
            self.offsets.push(original_start + text.len());
        }
        Ok(start)
    }
    fn define(
        &mut self,
        name: &Node,
        text: &str,
        node: &Node,
        edits: &mut BTreeMap<Span, String>,
    ) -> Result<(), String> {
        let original = &text[name.span.range()];
        if matches!(node.kind, K::Class | K::Trait | K::Enum | K::Alias)
            && (matches!(
                original,
                "Int"
                    | "double"
                    | "Bool"
                    | "String"
                    | "void"
                    | "BigInt"
                    | "Object"
                    | "Error"
                    | "Text"
                    | "Equals"
                    | "Comparable"
                    | "Iterable"
                    | "Iterator"
                    | "Multipliable"
                    | "RightMultipliable"
                    | "List"
                    | "Array"
                    | "Map"
                    | "MutMap"
                    | "Set"
                    | "MutSet"
                    | "SortedSet"
                    | "Option"
                    | "Box"
                    | "Mutex"
                    | "Atomic"
                    | "Channel"
                    | "Reader"
                    | "Writer"
                    | "PythonValue"
            ) || crate::interop::C_TYPES.contains(&original))
        {
            return Err(Diagnostic::new(
                if original == "Error" {
                    "E_ERROR_ROOT_REDEFINITION"
                } else {
                    "E_DUPLICATE_NAME"
                },
                format!("The built-in type `{original}` cannot be replaced."),
                name.span,
            )
            .render(&SourceFile::new("<session>", text)));
        }
        let first = original.chars().next();
        let form = match node.kind {
            K::Class | K::Trait | K::Enum | K::Alias => "PascalCase",
            K::Const => "SCREAMING_SNAKE_CASE",
            K::Binding => "value identifier",
            _ => "camelCase",
        };
        let valid = crate::identifiers::valid(original, form);
        if !valid {
            return Err(Diagnostic::new(
                "E_NAMING",
                format!("`{original}` must use {form}."),
                name.span,
            )
            .render(&SourceFile::new("<session>", text)));
        }
        if matches!(node.kind, K::Binding | K::Const) {
            self.value_names.insert(original.to_owned());
        } else {
            self.value_names.remove(original);
        }
        self.serial += 1;
        let internal = format!(
            "{}{}_{original}",
            if first.is_some_and(crate::identifiers::type_start) {
                "Session__"
            } else {
                "session__"
            },
            self.serial
        );
        edits.insert(name.span, internal.clone());
        self.aliases.insert(original.to_owned(), internal.clone());
        self.names.insert(internal.clone(), original.to_owned());
        self.definitions
            .insert(internal, text[node.span.range()].to_owned());
        Ok(())
    }
    fn inspect(&self, project: &modules::ProjectCheck, start: usize) -> Option<String> {
        let module = &project.modules[0];
        let node = module
            .parsed
            .root
            .children
            .iter()
            .find(|n| n.span.start >= start)?;
        let ty = module
            .checked
            .expression_types
            .iter()
            .find(|(span, _)| *span == node.span)?
            .1
            .clone();
        if !matches!(ty, Type::Function(_) | Type::Overloaded(_)) {
            return None;
        }
        let internal = module.parsed.source.slice(node.span);
        let definition = self
            .definitions
            .get(internal)
            .cloned()
            .or_else(|| {
                let Type::Function(f) = &ty else {
                    return None;
                };
                let Some(crate::resolution::Callable::Declaration { id, .. }) = &f.callable else {
                    return None;
                };
                project
                    .modules
                    .iter()
                    .find(|m| m.parsed.source.name == id.file)
                    .map(|m| m.parsed.source.slice(id.span).to_owned())
            })
            .unwrap_or_else(|| {
                if node.kind == K::Name {
                    "<native function>".into()
                } else {
                    let start = self.offsets.get(node.span.start).copied().unwrap_or(0);
                    let end = self
                        .offsets
                        .get(node.span.end)
                        .copied()
                        .unwrap_or(self.original.len());
                    self.original[start..end].trim().to_owned()
                }
            });
        Some(if node.kind == K::Name {
            format!(
                "{} : {}\n{}\n",
                self.readable(internal),
                self.readable(&ty.to_string()),
                definition
            )
        } else {
            format!("{}\n{}\n", self.readable(&ty.to_string()), definition)
        })
    }
}
fn is_pattern(kind: K) -> bool {
    matches!(
        kind,
        K::PatternName
            | K::PatternWildcard
            | K::PatternConstructor
            | K::NamedPattern
            | K::PatternTuple
            | K::PatternList
            | K::PatternRest
            | K::PatternAlt
            | K::PatternRange
            | K::PatternRangeInclusive
    )
}
fn pattern_names(node: &Node) -> Vec<&Node> {
    node.descendants()
        .filter_map(|n| match n.kind {
            K::PatternName => Some(n.child(K::Name).unwrap_or(n)),
            K::PatternRest => n.children.first(),
            _ => None,
        })
        .collect()
}
fn walk_pattern(
    node: &Node,
    text: &str,
    aliases: &BTreeMap<String, String>,
    locals: &mut BTreeSet<String>,
    edits: &mut BTreeMap<Span, String>,
) {
    match node.kind {
        K::PatternName | K::PatternRest => {
            for name in pattern_names(node) {
                locals.insert(text[name.span.range()].to_owned());
            }
        }
        K::PatternConstructor => {
            if let Some(case) = node.children.first() {
                walk(case, text, aliases, locals, edits);
            }
            for child in node.children.iter().skip(1) {
                walk_pattern(child, text, aliases, locals, edits);
            }
        }
        _ => {
            for child in &node.children {
                walk_pattern(child, text, aliases, locals, edits);
            }
        }
    }
}
fn parameters(
    node: &Node,
    text: &str,
    aliases: &BTreeMap<String, String>,
    locals: &mut BTreeSet<String>,
    edits: &mut BTreeMap<Span, String>,
) {
    for parameter in &node.children {
        if matches!(parameter.kind, K::Name | K::PatternName) {
            locals.insert(text[parameter.span.range()].to_owned());
            continue;
        }
        if let Some(name) = parameter.child(K::Name) {
            locals.insert(text[name.span.range()].to_owned());
        }
        let declared = parameter.child(K::Name).map(|name| name.span);
        for child in parameter
            .children
            .iter()
            .filter(|n| Some(n.span) != declared)
        {
            walk(child, text, aliases, locals, edits);
        }
    }
    if node.children.is_empty() && !text[node.span.range()].trim().is_empty() {
        locals.insert(text[node.span.range()].to_owned());
    }
}
fn walk(
    node: &Node,
    text: &str,
    aliases: &BTreeMap<String, String>,
    locals: &mut BTreeSet<String>,
    edits: &mut BTreeMap<Span, String>,
) {
    match node.kind {
        K::Name => {
            let name = lexer::path_name(&text[node.span.range()]);
            let root = name.split('.').next().unwrap_or(&name);
            if !locals.contains(root)
                && let Some(alias) = aliases.get(root)
            {
                let span = if name.contains('.') {
                    Span::new(node.span.start, node.span.start + root.len())
                } else {
                    node.span
                };
                edits.insert(span, alias.clone());
            }
        }
        K::Import => {}
        K::AssociatedTypeDeclaration => {}
        K::AssociatedType => {
            if let Some(value) = node.children.get(1) {
                walk(value, text, aliases, locals, edits);
            }
        }
        K::Parameter => {
            for child in node
                .children
                .iter()
                .filter(|child| child.kind != K::Name && child.kind != K::Modifier)
            {
                walk(child, text, aliases, locals, edits);
            }
        }
        K::Member | K::NamedArgument | K::RecordField => {
            let child = if node.kind == K::Member {
                node.children.first()
            } else {
                node.children.last()
            };
            if let Some(child) = child {
                walk(child, text, aliases, locals, edits);
            }
        }
        K::Binding | K::Const => {
            let pattern = node.children.iter().find(|n| is_pattern(n.kind));
            for child in node
                .children
                .iter()
                .filter(|n| Some(n.span) != pattern.map(|p| p.span))
            {
                walk(child, text, aliases, locals, edits);
            }
            if let Some(pattern) = pattern {
                walk_pattern(pattern, text, aliases, locals, edits);
            }
        }
        K::Ensures => {
            let mut nested = locals.clone();
            nested.insert("result".into());
            for child in &node.children {
                walk(child, text, aliases, &mut nested, edits);
            }
        }
        K::Function | K::Constructor | K::Lambda | K::Theorem => {
            let mut nested = locals.clone();
            let declaration = (node.kind == K::Function)
                .then(|| node.child(K::Name))
                .flatten();
            if let Some(name) = declaration
                && !edits.contains_key(&name.span)
            {
                nested.insert(text[name.span.range()].to_owned());
            }
            for params in node.children.iter().filter(|n| {
                matches!(
                    n.kind,
                    K::TypeParameters | K::ParameterList | K::LambdaParameters
                )
            }) {
                parameters(params, text, aliases, &mut nested, edits);
            }
            if node.kind == K::Theorem {
                for requirement in node.children.iter().filter(|n| n.kind == K::Requires) {
                    if let Some(name) = requirement.children.first() {
                        nested.insert(text[name.span.range()].to_owned());
                    }
                }
            }
            for child in node.children.iter().filter(|n| {
                Some(n.span) != declaration.map(|d| d.span)
                    && !matches!(
                        n.kind,
                        K::TypeParameters | K::ParameterList | K::LambdaParameters
                    )
            }) {
                walk(child, text, aliases, &mut nested, edits);
            }
        }
        K::Class | K::Trait | K::Enum => {
            let mut nested = locals.clone();
            for declaration in node
                .children
                .iter()
                .filter(|child| child.kind == K::AssociatedTypeDeclaration)
            {
                if let Some(name) = declaration.child(K::Name) {
                    nested.insert(text[name.span.range()].to_owned());
                }
            }
            if let Some(params) = node.child(K::TypeParameters) {
                parameters(params, text, aliases, &mut nested, edits);
            }
            for child in node
                .children
                .iter()
                .filter(|n| !matches!(n.kind, K::Name | K::TypeParameters))
            {
                if child.kind == K::Variant {
                    for ty in child.children.iter().filter(|n| n.kind != K::Name) {
                        walk(ty, text, aliases, &mut nested, edits);
                    }
                } else {
                    walk(child, text, aliases, &mut nested.clone(), edits);
                }
            }
        }
        K::Alias => {
            for child in node.children.iter().filter(|n| n.kind != K::Name) {
                walk(child, text, aliases, locals, edits);
            }
        }
        K::Block | K::For | K::MatchArm => {
            let mut nested = locals.clone();
            for child in &node.children {
                if is_pattern(child.kind) {
                    walk_pattern(child, text, aliases, &mut nested, edits);
                } else {
                    walk(child, text, aliases, &mut nested, edits);
                }
            }
        }
        K::ForBinding => {
            if let Some(value) = node.children.last() {
                walk(value, text, aliases, locals, edits);
            }
            for pattern in node.children.iter().filter(|n| is_pattern(n.kind)) {
                walk_pattern(pattern, text, aliases, locals, edits);
            }
            for ty in node.children.iter().filter(|n| {
                !is_pattern(n.kind) && Some(n.span) != node.children.last().map(|n| n.span)
            }) {
                walk(ty, text, aliases, locals, edits);
            }
        }
        K::Using => {
            let mut nested = locals.clone();
            if let Some(value) = node.children.get(1) {
                walk(value, text, aliases, &mut nested, edits);
            }
            if let Some(name) = node.children.first() {
                nested.insert(text[name.span.range()].to_owned());
            }
            for child in node.children.iter().skip(2) {
                walk(child, text, aliases, &mut nested, edits);
            }
        }
        K::Comprehension => {
            let mut nested = locals.clone();
            walk(&node.children[2], text, aliases, &mut nested, edits);
            walk_pattern(&node.children[1], text, aliases, &mut nested, edits);
            walk(&node.children[0], text, aliases, &mut nested, edits);
            for child in node.children.iter().skip(3) {
                walk(child, text, aliases, &mut nested, edits);
            }
        }
        _ => {
            for child in &node.children {
                walk(child, text, aliases, locals, edits);
            }
        }
    }
}

pub struct Session {
    root: PathBuf,
    libraries: Vec<String>,
    history: History,
    state: core::SessionState,
    sources: BTreeMap<PathBuf, String>,
    runtime: native::session::Runtime,
}

/// What became of one submitted entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Done,
    /// The entry stopped early on an Error met by `?`; its names were not kept.
    Aborted,
}
impl Session {
    pub fn new(
        root: &Path,
        libraries: Vec<String>,
        arguments: &[String],
        null_input: bool,
        notebook: bool,
    ) -> Result<Self, String> {
        Ok(Self {
            root: root.canonicalize().map_err(|e| e.to_string())?,
            libraries,
            history: History::default(),
            state: core::SessionState::default(),
            sources: BTreeMap::new(),
            runtime: native::session::Runtime::new(arguments, null_input, notebook)?,
        })
    }
    pub fn submit(&mut self, text: &str) -> Result<(), String> {
        self.submit_entry(text, false).map(|_| ())
    }

    /// Submits one entry. In notebook mode only the entry's last statement can display, and
    /// it displays as a marked result block; diagnostics count lines from the entry's start.
    pub fn submit_entry(&mut self, text: &str, notebook: bool) -> Result<Outcome, String> {
        let mut history = self.history.clone();
        history.view_start = notebook.then_some(history.original.len());
        let start = history.add(text)?;
        let mut sources = self.sources.clone();
        let project =
            modules::check_session_cached(history.source.clone(), &self.root, &mut sources)
                .map_err(|e| e.to_string())?;
        if !project.is_ok() {
            return Err(project
                .modules
                .iter()
                .flat_map(|m| {
                    m.checked.diagnostics.iter().map(|d| {
                        if m.parsed.source.name == "<session>" {
                            history.diagnostic(d)
                        } else {
                            d.render(&m.parsed.source)
                        }
                    })
                })
                .collect());
        }
        // In a notebook, only a trailing expression displays.
        let display_from = if notebook {
            project.modules[0]
                .parsed
                .root
                .children
                .iter()
                .rfind(|n| n.span.start >= start)
                .filter(|n| shows_value(n.kind))
                .map_or(usize::MAX, |n| n.span.start)
        } else {
            start
        };
        let display_errors = crate::checker::session_display(
            &project.modules[0].parsed,
            &project.modules[0].checked,
            display_from,
            notebook,
        );
        if !display_errors.is_empty() {
            return Err(display_errors
                .iter()
                .map(|d| history.diagnostic(d))
                .collect());
        }
        let inspect = history.inspect(&project, start);
        let mut state = self.state.clone();
        let lowered = if notebook {
            core::lower_notebook_cell(&project, start, display_from, &mut state)
        } else {
            core::lower_session(&project, start, &mut state)
        };
        let mut program = lowered.map_err(|errors| {
            errors
                .iter()
                .map(|e| {
                    if e.module == 0 {
                        history.diagnostic(&e.diagnostic)
                    } else {
                        e.diagnostic
                            .render(&project.modules[e.module].parsed.source)
                    }
                })
                .collect::<String>()
        })?;
        history.remap_locations(&mut program);
        program.c_include_dirs.push(self.root.clone());
        for header in &program.c_headers {
            let source = self.root.join(header).with_extension("c");
            if header.ends_with(".h") && source.is_file() && !program.c_sources.contains(&source) {
                program.c_sources.push(source);
            }
        }
        program.c_libraries = self.libraries.clone();
        let executes = program.functions[0].body.iter().any(|statement| !matches!(statement,core::Statement::Return(core::Atom::Unit,cleanups) if cleanups.is_empty()));
        let aborted = executes && self.runtime.execute(&program)?;
        if aborted {
            // The worker already holds this entry's slots, so keep them reserved; only the
            // names the entry bound are forgotten, because later ones never ran.
            state.forget_names(&self.state);
            self.state = state;
            self.history.serial = history.serial;
            if self.history.view_start.is_none() {
                self.history.pad(text);
            }
            self.sources = sources;
            return Ok(Outcome::Aborted);
        }
        self.history = history;
        self.state = state;
        self.sources = sources;
        if let Some(inspect) = inspect {
            print!("{inspect}");
            io::stdout().flush().map_err(|e| e.to_string())?;
        }
        Ok(Outcome::Done)
    }
    pub fn finish(&mut self) -> Result<(), String> {
        self.runtime.finish()
    }
}

/// Read one byte at a time: a running readLine() in the worker uses the same stdin,
/// so the command reader must never read ahead into the program's input.
fn input_line(input: &mut impl Read) -> io::Result<Option<String>> {
    let mut bytes = vec![];
    let mut byte = [0];
    loop {
        match input.read(&mut byte)? {
            0 => break,
            _ => {
                bytes.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
            }
        }
    }
    if bytes.is_empty() {
        Ok(None)
    } else {
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Session input is not UTF-8"))
    }
}
fn incomplete(text: &str) -> bool {
    let lexed = lexer::lex(text);
    let mut depth = 0;
    for token in &lexed.tokens {
        if token.kind == TokenKind::Symbol {
            match &text[token.span.range()] {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth -= 1,
                _ => {}
            }
        }
    }
    if depth > 0
        || lexed
            .diagnostics
            .iter()
            .any(|d| d.code == "E_UNCLOSED_STRING")
    {
        return true;
    }
    let parsed = parse_file("<session>", text);
    !parsed.diagnostics.is_empty()
        && parsed
            .diagnostics
            .iter()
            .all(|d| d.span.start >= text.trim_end().len())
}
pub fn run(root: &Path, libraries: Vec<String>, arguments: &[String]) -> Result<bool, String> {
    let interactive = io::stdin().is_terminal();
    // A private handle to standard input: the worker inherits the same stream for readLine.
    #[cfg(unix)]
    let handle = {
        use std::os::fd::AsFd;
        io::stdin().as_fd().try_clone_to_owned()
    };
    #[cfg(windows)]
    let handle = {
        use std::os::windows::io::AsHandle;
        io::stdin().as_handle().try_clone_to_owned()
    };
    let mut input = std::fs::File::from(handle.map_err(|e| e.to_string())?);
    let mut session = Session::new(root, libraries, arguments, false, false)?;
    let (mut buffer, mut success) = (String::new(), true);
    loop {
        if interactive {
            print!("{}", if buffer.is_empty() { "kes> " } else { "...> " });
            io::stdout().flush().map_err(|e| e.to_string())?;
        }
        let Some(line) = input_line(&mut input).map_err(|e| e.to_string())? else {
            break;
        };
        if line.trim() == ":cancel" {
            buffer.clear();
            continue;
        }
        if buffer.is_empty() {
            if let Some(filter) = line.trim().strip_prefix(":test")
                && (filter.is_empty() || filter.starts_with(char::is_whitespace))
            {
                let filter = filter.trim();
                let options = crate::testing::Options {
                    filter: (!filter.is_empty()).then(|| filter.to_owned()),
                    jobs: std::thread::available_parallelism().map_or(4, |n| n.get()),
                    libraries: session.libraries.clone(),
                    timeout: None,
                };
                match crate::testing::run(&session.root, &options, &mut io::stdout()) {
                    Ok(passed) => success &= passed,
                    Err(error) => {
                        eprintln!("{error}");
                        success = false;
                    }
                }
                io::stdout().flush().map_err(|e| e.to_string())?;
                continue;
            }
            match line.trim() {
                ":quit" => break,
                ":help" => {
                    println!(
                        "Enter Kestrel code. :test [FILTER] runs the package's saved tests; :quit ends the session; :cancel discards an unfinished entry."
                    );
                    continue;
                }
                "" => continue,
                _ => {}
            }
        }
        buffer.push_str(&line);
        if incomplete(&buffer) {
            continue;
        }
        match session.submit_entry(&buffer, false) {
            // A runtime stop or a top-level `?` ends only this entry, but the session's
            // final exit reports it (REVIEW 1.7).
            Ok(Outcome::Aborted) => success = false,
            Ok(_) => {}
            Err(error) => {
                eprint!("{error}");
                if !error.ends_with('\n') {
                    eprintln!();
                }
                success = false;
                if !session.runtime.is_running()? {
                    buffer.clear();
                    break;
                }
            }
        }
        buffer.clear();
    }
    if !buffer.trim().is_empty() {
        match session.submit_entry(&buffer, false) {
            Ok(Outcome::Aborted) => success = false,
            Ok(_) => {}
            Err(error) => {
                eprint!("{error}");
                success = false;
            }
        }
    }
    session.finish()?;
    Ok(success)
}

/// Whether a top-level statement is an expression whose value a session can display.
fn shows_value(kind: K) -> bool {
    !matches!(
        kind,
        K::Import
            | K::Function
            | K::Class
            | K::Trait
            | K::Enum
            | K::Alias
            | K::Const
            | K::Binding
            | K::Assignment
            | K::For
            | K::While
            | K::Break
            | K::Continue
            | K::Return
            | K::Theorem
            | K::Axiom
            | K::Example
    )
}

/// The marker that starts every control line a notebook host writes. It is the ASCII
/// record separator, which ordinary program output does not contain.
const MARK: char = '\u{1e}';

fn error_block(message: &str) -> String {
    format!("{MARK}[error]\n{}\n{MARK}[end]\n", message.trim_end())
}

impl Session {
    /// Runs one notebook cell. The whole cell is one entry; when it does not compile, its
    /// statements run one at a time so the earlier ones still take effect, and the first
    /// failing one reports its error. Output goes to standard output as marked blocks.
    pub fn run_cell(&mut self, code: &str) -> &'static str {
        let outcome = self.submit_entry(code, true);
        let failure = match outcome {
            Ok(Outcome::Done) => return "ok",
            Ok(Outcome::Aborted) => return "abort",
            Err(message) => message,
        };
        // Statement by statement: only when the cell has several and the worker is alive.
        let parts = split_statements(code);
        if parts.len() > 1 && self.runtime.is_running().unwrap_or(false) {
            let count = parts.len();
            for (index, part) in parts.iter().enumerate() {
                match self.submit_entry(part, index + 1 == count) {
                    Ok(Outcome::Done) => {}
                    Ok(Outcome::Aborted) => return "abort",
                    Err(message) => {
                        print!("{}", error_block(&message));
                        return "error";
                    }
                }
            }
            return "ok";
        }
        print!("{}", error_block(&failure));
        "error"
    }
}

/// The top-level statements of a cell, each as its own text.
fn split_statements(code: &str) -> Vec<String> {
    let parsed = parse_file("<cell>", code);
    if !parsed.is_ok() {
        return vec![code.to_owned()];
    }
    parsed
        .root
        .children
        .iter()
        .map(|node| code[node.span.range()].to_owned())
        .collect()
}

/// `kestrel notebook`: serves a Jupyter kernel front end over standard input and output.
/// Requests are `CELL id length` or `CHECK id length` followed by that many bytes of code,
/// or `EXIT`. Each answer ends with a `[done]` line on standard output and on standard
/// error, so the reader knows everything the cell wrote has arrived.
pub fn run_notebook(root: &Path, libraries: Vec<String>) -> Result<bool, String> {
    use std::io::BufRead;
    let mut input = io::BufReader::new(io::stdin().lock());
    let mut session = Session::new(root, libraries, &[], true, true)?;
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            break;
        }
        let words = line.split_whitespace().collect::<Vec<_>>();
        match words.as_slice() {
            ["EXIT"] => break,
            [kind @ ("CELL" | "CHECK"), id, length] => {
                let length = length.parse::<usize>().map_err(|e| e.to_string())?;
                let mut bytes = vec![0; length];
                input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
                let code = String::from_utf8(bytes)
                    .map_err(|_| "A notebook cell is not UTF-8.".to_owned())?;
                let id = (*id).to_owned();
                let status = if *kind == "CHECK" {
                    if incomplete(&code) {
                        "incomplete"
                    } else {
                        "complete"
                    }
                } else {
                    session.run_cell(&code)
                };
                let alive = session.runtime.is_running()?;
                let status = if alive { status } else { "dead" };
                println!("{MARK}[done] {id} {status}");
                eprintln!("{MARK}[done] {id} {status}");
                io::stdout().flush().map_err(|e| e.to_string())?;
                io::stderr().flush().map_err(|e| e.to_string())?;
                if !alive {
                    return Ok(false);
                }
            }
            _ => return Err(format!("Unknown notebook request `{}`.", line.trim())),
        }
    }
    session.finish()?;
    Ok(true)
}
