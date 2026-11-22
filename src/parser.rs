use crate::{
    diagnostic::Diagnostic,
    lexer::{self, Token, TokenKind},
    source::Span,
    syntax::{Node, SyntaxKind as K},
};

const MAX_DEPTH: usize = 96;
type PResult = Result<Node, ()>;

pub(crate) fn parse_tokens_with_values(
    text: &str,
    tokens: &[Token],
    project: bool,
    values: &std::collections::BTreeSet<String>,
) -> (Node, Vec<Diagnostic>) {
    let mut parser = Parser::new(text, tokens, 0);
    parser.value_names = values.clone();
    let root = if project {
        parser.project()
    } else {
        parser.file()
    };
    (root, parser.diagnostics)
}

#[derive(Clone, Copy)]
struct CustomOperator {
    left_bp: u8,
    right_bp: u8,
    function: Span,
}

struct Parser<'a> {
    text: &'a str,
    tokens: Vec<Token>,
    /// Every token, trivia included, so a `///` block can be read before an item.
    raw_tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
    depth: usize,
    proposition: bool,
    /// Inside a theorem's proof, where `induction` and `simp` start tactics.
    proving: bool,
    /// In a declaration's return type, a trailing `with Effects` marks the declaration.
    declaration_return: bool,
    value_names: std::collections::BTreeSet<String>,
    case_names: std::collections::BTreeSet<String>,
    binding_pattern: bool,
    /// The `=>` that ends the guard being parsed, so a bare name there is not
    /// read as a lambda parameter.
    guard_arrow: Option<usize>,
    custom_operators: std::collections::BTreeMap<String, CustomOperator>,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str, tokens: &[Token], depth: usize) -> Self {
        Self {
            text,
            tokens: tokens.iter().copied().filter(|t| !t.is_trivia()).collect(),
            raw_tokens: tokens.to_vec(),
            pos: 0,
            diagnostics: vec![],
            depth,
            proposition: false,
            proving: false,
            declaration_return: false,
            value_names: Default::default(),
            case_names: case_names(text, tokens),
            binding_pattern: false,
            guard_arrow: None,
            custom_operators: Default::default(),
        }
    }

    fn token(&self) -> Token {
        self.tokens[self.pos]
    }
    fn at(&self, text: &str) -> bool {
        self.word() == text
    }
    fn word(&self) -> &'a str {
        &self.text[self.token().span.range()]
    }
    fn peek_word(&self, offset: usize) -> &'a str {
        self.tokens
            .get(self.pos + offset)
            .map(|token| &self.text[token.span.range()])
            .unwrap_or("")
    }
    /// `final this.name : Type := value`, with optional visibility/static
    /// modifiers that the checker rejects rather than silently accepting.
    fn at_constructor_field(&self) -> bool {
        let mut offset = 0;
        while matches!(
            self.peek_word(offset),
            "public" | "private" | "package" | "static"
        ) {
            offset += 1;
        }
        matches!(self.peek_word(offset), "final" | "var") && self.peek_word(offset + 1) == "this"
    }
    fn eof(&self) -> bool {
        self.token().kind == TokenKind::Eof
    }
    fn newline(&self) -> bool {
        self.token().kind == TokenKind::Newline
    }
    fn start(&self) -> usize {
        self.token().span.start
    }
    fn end(&self) -> usize {
        if self.pos == 0 {
            self.start()
        } else {
            self.tokens[self.pos - 1].span.end
        }
    }
    fn node(&self, kind: K, start: usize, children: Vec<Node>) -> Node {
        Node::new(kind, Span::new(start, self.end().max(start)), children)
    }
    fn bump(&mut self) -> Token {
        let token = self.token();
        if !self.eof() {
            self.pos += 1;
        }
        token
    }
    fn eat(&mut self, text: &str) -> bool {
        if self.at(text) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn soft(&mut self) {
        while self.newline() {
            self.bump();
        }
    }
    fn separators(&mut self) {
        while self.newline() || self.at(";") {
            self.bump();
        }
    }
    fn err(&mut self, code: &'static str, message: impl Into<String>, hint: &str) {
        let mut diagnostic = Diagnostic::new(code, message, self.token().span);
        if !hint.is_empty() {
            diagnostic = diagnostic.with_hint(hint);
        }
        self.diagnostics.push(diagnostic);
    }
    fn expect(&mut self, text: &str) -> Result<(), ()> {
        if self.eat(text) {
            return Ok(());
        }
        let found = if self.eof() {
            "the end of the input".into()
        } else {
            format!("`{}`", self.word())
        };
        self.err(
            "E_EXPECTED",
            format!("Expected `{text}`, found {found}."),
            &format!("Put `{text}` here."),
        );
        Err(())
    }
    fn name(&mut self) -> PResult {
        if self.token().kind == TokenKind::Word && !reserved(self.word()) {
            return Ok(Node::leaf(K::Name, self.bump().span));
        }
        self.err(
            "E_NAME",
            format!("Expected a name, found `{}`.", self.word()),
            "Use a name at this position.",
        );
        Err(())
    }
    fn nested(&mut self, f: impl FnOnce(&mut Self) -> PResult) -> PResult {
        if self.depth >= MAX_DEPTH {
            self.err(
                "E_NESTING_LIMIT",
                "This expression or declaration is nested too deeply.",
                "Split it into smaller expressions.",
            );
            return Err(());
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }

    /// The contiguous `///` block that touches the token at `start`, if any. A
    /// blank line, an ordinary `//` comment or a `////` banner ends the block,
    /// and a `///` that follows code on its line is commentary, not documentation.
    fn doc_block(&self, start: usize) -> Option<Node> {
        let mut cursor = self
            .raw_tokens
            .partition_point(|token| token.span.start < start);
        let mut lines = vec![];
        let (mut begin, mut end) = (start, start);
        loop {
            while cursor > 0 && self.raw_tokens[cursor - 1].kind == TokenKind::Whitespace {
                cursor -= 1;
            }
            if cursor == 0 || self.raw_tokens[cursor - 1].kind != TokenKind::Newline {
                break;
            }
            cursor -= 1;
            while cursor > 0 && self.raw_tokens[cursor - 1].kind == TokenKind::Whitespace {
                cursor -= 1;
            }
            if cursor == 0 {
                break;
            }
            let comment = self.raw_tokens[cursor - 1];
            let marker = &self.text[comment.span.range()];
            if comment.kind != TokenKind::Comment
                || !marker.starts_with("///")
                || marker.starts_with("////")
            {
                break;
            }
            let mut before = cursor - 1;
            while before > 0 && self.raw_tokens[before - 1].kind == TokenKind::Whitespace {
                before -= 1;
            }
            if before > 0 && self.raw_tokens[before - 1].kind != TokenKind::Newline {
                break;
            }
            cursor -= 1;
            let content = comment.span.start + 3;
            let content = if self.text.as_bytes().get(content) == Some(&b' ') {
                content + 1
            } else {
                content
            };
            if lines.is_empty() {
                end = comment.span.end;
            }
            lines.push(Node::leaf(K::DocLine, Span::new(content, comment.span.end)));
            begin = comment.span.start;
        }
        if lines.is_empty() {
            return None;
        }
        lines.reverse();
        Some(Node::new(K::Doc, Span::new(begin, end), lines))
    }

    fn file(&mut self) -> Node {
        let mut children = vec![];
        self.separators();
        while !self.eof() {
            let before = self.pos;
            let start = self.start();
            if self.at_extend() {
                match self.extend_block() {
                    Ok(functions) => children.extend(functions),
                    Err(()) => children.push(self.recover(start, before, false)),
                }
            } else {
                match self.item(false) {
                    Ok(node) => children.push(node),
                    Err(()) => children.push(self.recover(start, before, false)),
                }
            }
            if self.pos == before {
                self.bump();
            }
            self.finish_item();
        }
        Node::new(K::File, Span::new(0, self.text.len()), children)
    }

    /// `extend List { ... }`: `extend` is a word only before a type name and a brace.
    fn at_extend(&self) -> bool {
        if self.word() != "extend" {
            return false;
        }
        let word = |offset: usize| {
            self.tokens
                .get(self.pos + offset)
                .map(|t| &self.text[t.span.range()])
                .unwrap_or("")
        };
        word(1).starts_with(|c: char| c.is_ascii_uppercase()) && word(2) == "{"
    }

    /// Each function in an `extend` block becomes a top-level function whose first child names
    /// the extended type. Its first parameter is the receiver: `xs.take(2)` calls `take(xs, 2)`.
    fn extend_block(&mut self) -> Result<Vec<Node>, ()> {
        self.bump();
        let target_start = self.start();
        let name = self.name()?;
        let target = self.node(K::ExtendTarget, target_start, vec![name]);
        self.expect("{")?;
        self.separators();
        let mut functions = vec![];
        while !self.eof() && !self.at("}") {
            let before = self.pos;
            let start = self.start();
            match self.item(true) {
                Ok(mut node) if node.kind == K::Function => {
                    node.children.insert(0, target.clone());
                    functions.push(node);
                }
                Ok(node) => {
                    self.diagnostics.push(
                        Diagnostic::new(
                            "E_EXTEND_MEMBER",
                            "An extend block holds only functions.",
                            node.span,
                        )
                        .with_hint("Move fields and types into a class."),
                    );
                }
                Err(()) => {
                    self.recover(start, before, true);
                }
            }
            if self.pos == before {
                self.bump();
            }
            self.separators();
        }
        self.expect("}")?;
        Ok(functions)
    }

    fn finish_item(&mut self) {
        if !self.eof() && !self.newline() && !self.at(";") && !self.at("}") {
            self.err(
                "E_SEPARATOR",
                format!("Expected a new line or `;`, found `{}`.", self.word()),
                "Separate these statements with a new line.",
            );
            let before = self.pos;
            self.recover(self.start(), before, true);
        }
        self.separators();
    }

    fn recover(&mut self, start: usize, before: usize, in_block: bool) -> Node {
        let mut balance = 0usize;
        while !self.eof() {
            if balance == 0 {
                if self.newline() || self.at(";") {
                    break;
                }
                if self.at("}") {
                    if !in_block && self.pos == before {
                        self.bump();
                    }
                    break;
                }
                if self.pos > before && declaration_start(self.word()) {
                    break;
                }
            }
            match self.word() {
                "{" | "(" | "[" => balance += 1,
                "}" | ")" | "]" if balance > 0 => balance -= 1,
                _ => {}
            }
            self.bump();
        }
        self.node(K::Error, start, vec![])
    }

    fn project(&mut self) -> Node {
        let mut children = vec![];
        let mut seen_package = false;
        self.separators();
        while !self.eof() {
            let start = self.start();
            let before = self.pos;
            let kind = match self.word() {
                "package" if !seen_package && children.is_empty() => {
                    seen_package = true;
                    K::Package
                }
                "depends" if seen_package => K::Dependency,
                _ => {
                    self.err(
                        "E_PROJECT",
                        "Expected `package name` first, then `depends name` lines.",
                        "Pass source code to `parse`, and project contents to `parse --project`.",
                    );
                    children.push(self.recover(start, before, false));
                    if self.pos == before {
                        self.bump();
                    }
                    self.separators();
                    continue;
                }
            };
            self.bump();
            match self.path() {
                Ok(name) => {
                    let mut line = vec![name];
                    if kind == K::Dependency && self.eat("from") {
                        let Some(url) = self.project_string("a quoted source URL after `from`")
                        else {
                            children.push(self.recover(start, before, false));
                            self.finish_item();
                            continue;
                        };
                        line.push(url);
                        if !self.eat("at") {
                            self.err(
                                "E_PROJECT",
                                "Expected `at` and a quoted tag or commit after the source URL.",
                                "Write `depends matrix from \"https://example.org/matrix.git\" at \"v1.2.0\"`.",
                            );
                            children.push(self.recover(start, before, false));
                            self.finish_item();
                            continue;
                        }
                        let Some(reference) =
                            self.project_string("a quoted tag or commit after `at`")
                        else {
                            children.push(self.recover(start, before, false));
                            self.finish_item();
                            continue;
                        };
                        line.push(reference);
                    }
                    children.push(self.node(kind, start, line));
                }
                Err(()) => children.push(self.recover(start, before, false)),
            }
            self.finish_item();
        }
        if !seen_package {
            self.err(
                "E_PROJECT",
                "This project needs a `package name` line.",
                "Start the project file with its package declaration.",
            );
        }
        Node::new(K::Project, Span::new(0, self.text.len()), children)
    }

    fn project_string(&mut self, expected: &str) -> Option<Node> {
        if self.token().kind != TokenKind::String {
            self.err(
                "E_PROJECT",
                format!("Expected {expected}."),
                "Use a double-quoted string.",
            );
            return None;
        }
        Some(Node::leaf(K::Literal, self.bump().span))
    }

    fn path(&mut self) -> PResult {
        let start = self.start();
        let mut children = vec![self.name()?];
        while self.at(".") {
            let next = self.tokens.get(self.pos + 1).copied();
            if next.is_some_and(|t| &self.text[t.span.range()] == "{") {
                break;
            }
            self.bump();
            children.push(self.name()?);
        }
        Ok(self.node(K::Name, start, children))
    }

    /// One rule in `rw [...]` or `simp [...]`. A leading `←` (or `\larr`)
    /// marks the rule as reversed; it is represented as a unary node so the
    /// rule collector in `proof_simp` can swap the equality or iff sides.
    fn rewrite_rule(&mut self) -> PResult {
        let start = self.start();
        if self.at("←") || self.at("\\larr") {
            let operator = Node::leaf(K::Operator, self.bump().span);
            let path = self.expr(0, false)?;
            Ok(self.node(K::Unary, start, vec![operator, path]))
        } else {
            self.expr(0, false)
        }
    }

    fn item(&mut self, member: bool) -> PResult {
        self.nested(|p| p.item_inner(member))
    }

    fn item_inner(&mut self, member: bool) -> PResult {
        let start = self.start();
        if !member && self.at_constructor_field() {
            return self.constructor_field();
        }
        let mut prefix = vec![];
        while self.eat("@") {
            let attribute_start = self.tokens[self.pos - 1].span.start;
            let name = self.name()?;
            prefix.push(self.node(K::Attribute, attribute_start, vec![name]));
            self.soft();
        }
        while matches!(
            self.word(),
            "public" | "private" | "package" | "static" | "override" | "sealed" | "mut"
        ) {
            prefix.push(Node::leaf(K::Modifier, self.bump().span));
        }
        let mut node = match self.word() {
            "import" if !member => self.import()?,
            "function" => self.function(false)?,
            "opaque" if !member => self.opaque()?,
            "new" if member => self.function(true)?,
            "class" | "trait" => self.class()?,
            "record" if !member => self.record_declaration()?,
            "enum" => self.enum_decl()?,
            "infixl" | "infixr" | "infix" if !member => self.operator_declaration()?,
            "alias" => {
                self.bump();
                let name = self.name()?;
                self.expect("=")?;
                let ty = self.ty()?;
                self.node(K::Alias, start, vec![name, ty])
            }
            "const" => self.binding(true)?,
            "has" if member => self.has()?,
            "invariant" if member => self.invariant()?,
            "type" if member => {
                self.bump();
                let name = self.name()?;
                self.node(K::AssociatedTypeDeclaration, start, vec![name])
            }
            "final" | "var" if member => self.field()?,
            "theorem" if !member => self.theorem()?,
            "example" if !member => self.example()?,
            "axiom" if !member => self.axiom()?,
            _ if !prefix.is_empty() => {
                self.err(
                    "E_DECLARATION",
                    "An attribute or modifier must come before a declaration.",
                    "Put a function, class, trait, enum, alias or const declaration here.",
                );
                return Err(());
            }
            _ if member => {
                self.err(
                    "E_MEMBER",
                    "Expected a field, constructor, method or has list.",
                    "Fields start with `final` or `var`; methods start with `function`; invariants start with `invariant`.",
                );
                return Err(());
            }
            _ => self.statement()?,
        };
        node.span.start = start;
        if !prefix.is_empty() {
            prefix.append(&mut node.children);
            node.children = prefix;
        }
        if documented(node.kind)
            && let Some(doc) = self.doc_block(start)
        {
            // The block is part of the declaration it documents, so its span covers it.
            node.span.start = doc.span.start;
            node.children.insert(0, doc);
        }
        Ok(node)
    }

    fn import(&mut self) -> PResult {
        let start = self.start();
        self.bump();
        let mut children = vec![];
        if matches!(self.word(), "c" | "python") {
            children.push(Node::leaf(K::Name, self.bump().span));
            if self.token().kind != TokenKind::String {
                self.err(
                    "E_IMPORT",
                    "Expected a quoted header or Python module name.",
                    "Use a double-quoted string.",
                );
                return Err(());
            }
            children.push(Node::leaf(K::Literal, self.bump().span));
            if self.eat("as") {
                children.push(self.name()?);
            }
        } else {
            children.push(self.path()?);
            if self.eat(".") {
                let names_start = self.start();
                self.expect("{")?;
                let mut names = vec![];
                self.soft();
                while !self.eof() && !self.at("}") {
                    let name = self.name()?;
                    if self.text[name.span.range()]
                        .chars()
                        .next()
                        .is_some_and(crate::identifiers::type_start)
                    {
                        self.case_names.insert(self.text[name.span.range()].into());
                    }
                    names.push(name);
                    self.soft();
                    if !self.eat(",") {
                        break;
                    }
                    self.soft();
                }
                self.expect("}")?;
                children.push(self.node(K::ImportNames, names_start, names));
            } else if self.eat("as") {
                children.push(self.name()?);
            }
        }
        Ok(self.node(K::Import, start, children))
    }

    fn type_parameters(&mut self) -> PResult {
        let start = self.start();
        self.expect("[")?;
        self.soft();
        let mut params = vec![];
        while !self.eof() && !self.at("]") {
            let start = self.start();
            let mut children = vec![self.name()?];
            if self.eat(":") {
                loop {
                    self.soft();
                    let bound_start = self.start();
                    let bound = self.ty_atom()?;
                    children.push(self.node(K::Bound, bound_start, vec![bound]));
                    if !self.eat("+") {
                        break;
                    }
                }
            }
            params.push(self.node(K::TypeParameter, start, children));
            self.soft();
            if !self.eat(",") {
                break;
            }
            self.soft();
        }
        self.expect("]")?;
        Ok(self.node(K::TypeParameters, start, params))
    }

    /// Function, method, constructor and lambda parameters. Names before one
    /// colon share the annotation: `(a b : Nat)` means `(a : Nat, b : Nat)`.
    fn parameters(&mut self) -> PResult {
        let start = self.start();
        self.expect("(")?;
        self.soft();
        let mut params = vec![];
        while !self.eof() && !self.at(")") {
            let param_start = self.start();
            let mut modifiers = vec![];
            if matches!(self.word(), "move" | "mut") || self.proof_modifier() {
                modifiers.push(Node::leaf(K::Modifier, self.bump().span));
            }
            let mut names = vec![self.name()?];
            self.value_names
                .insert(self.text[names[0].span.range()].into());
            while self.token().kind == TokenKind::Word
                && !reserved(self.word())
                && !matches!(self.word(), "move" | "mut")
            {
                let name = self.name()?;
                self.value_names.insert(self.text[name.span.range()].into());
                names.push(name);
            }
            let proof = modifiers.iter().any(|c| self.modifier_is(c, "proof"));
            let mut annotation = None;
            if self.eat(":") {
                if proof {
                    let previous = self.proposition;
                    self.proposition = true;
                    let claim = self.expr(0, false);
                    self.proposition = previous;
                    let claim = claim?;
                    annotation = Some(self.node(K::Proposition, claim.span.start, vec![claim]));
                } else {
                    annotation = Some(self.ty()?);
                }
            } else if names.len() > 1 {
                self.err(
                    "E_PARAMETER_TYPE",
                    "Names grouped before a type need one shared annotation.",
                    "Write `(a b : Nat)`, or separate the parameters with commas.",
                );
                return Err(());
            }
            let default = if self.eat("=") {
                Some(self.expr(0, true)?)
            } else {
                None
            };
            if names.len() > 1 && default.is_some() {
                self.err(
                    "E_PARAMETER_DEFAULT",
                    "A grouped parameter cannot give one default to several names.",
                    "Write separate parameters when each needs its own default.",
                );
                return Err(());
            }
            for name in names.iter() {
                let mut children = modifiers.clone();
                children.push(name.clone());
                if let Some(found) = &annotation {
                    children.push(found.clone());
                }
                if let Some(found) = &default {
                    children.push(found.clone());
                }
                let end = default
                    .as_ref()
                    .or(annotation.as_ref())
                    .map(|node| node.span.end)
                    .unwrap_or(name.span.end);
                let param_start = if names.len() == 1 {
                    param_start
                } else {
                    name.span.start
                };
                params.push(Node::new(
                    K::Parameter,
                    Span::new(param_start, end),
                    children,
                ));
            }
            self.soft();
            if !self.eat(",") {
                break;
            }
            self.soft();
        }
        self.expect(")")?;
        Ok(self.node(K::ParameterList, start, params))
    }

    /// `opaque function f(...) : T` or `opaque type T[...]`.
    ///
    /// An opaque declaration is proof-only: it has no body, never reaches the
    /// native backend, and its behavior is given by `axiom` statements. This is
    /// how the geometry, algebra and analysis theories name their primitives.
    fn opaque(&mut self) -> PResult {
        let start = self.start();
        self.bump();
        let inner = match self.word() {
            "function" => self.function(false)?,
            "type" => {
                let type_start = self.start();
                self.bump();
                let mut children = vec![self.name()?];
                if self.at("[") {
                    children.push(self.type_parameters()?);
                }
                self.node(K::Enum, type_start, children)
            }
            _ => {
                self.err(
                    "E_OPAQUE",
                    "`opaque` is followed by `function` or `type`.",
                    "Write `opaque function name(...) : Type` or `opaque type Name[U]`.",
                );
                return Err(());
            }
        };
        Ok(self.node(K::Opaque, start, vec![inner]))
    }

    fn function(&mut self, constructor: bool) -> PResult {
        let saved_values = self.value_names.clone();
        let result = self.function_inner(constructor);
        self.value_names = saved_values;
        result
    }

    fn function_inner(&mut self, constructor: bool) -> PResult {
        let start = self.start();
        self.bump();
        let mut children = vec![];
        if !constructor {
            children.push(self.name()?);
        }
        if self.at("[") {
            children.push(self.type_parameters()?);
        }
        children.push(self.parameters()?);
        let mut proposition_result = false;
        if self.eat(":") {
            let start = self.start();
            let previous = std::mem::replace(&mut self.declaration_return, true);
            let ty = if self.eat("proof") {
                self.expr(0, false)
                    .map(|claim| self.node(K::Proposition, start, vec![claim]))
            } else {
                self.ty()
            };
            self.declaration_return = previous;
            let ty = ty?;
            proposition_result = ty.kind == K::TypeName && &self.text[ty.span.range()] == "Prop";
            children.push(self.node(K::ReturnType, start, vec![ty]));
        }
        if self.eat("with") {
            let start = self.tokens[self.pos - 1].span.start;
            self.expect("Effects")?;
            children.push(self.node(K::Effects, start, vec![]));
        }
        while self.eat("requires") {
            let start = self.tokens[self.pos - 1].span.start;
            let requirement = self.expr(0, false)?;
            children.push(self.node(K::Requires, start, vec![requirement]));
        }
        while self.eat("ensures") {
            let start = self.tokens[self.pos - 1].span.start;
            let condition = self.expr(0, false)?;
            children.push(self.node(K::Ensures, start, vec![condition]));
        }
        if constructor {
            self.soft();
            children.push(self.block()?);
        } else if self.eat("=") {
            if proposition_result {
                // A function returning Prop has a proposition body. Mark the
                // node so the checker and the kernel read it as a proposition,
                // not as a run-time expression.
                let body_start = self.start();
                let previous = std::mem::replace(&mut self.proposition, true);
                let body = self.expr(0, true);
                self.proposition = previous;
                let body = body?;
                children.push(self.node(K::Proposition, body_start, vec![body]));
            } else {
                children.push(self.expr(0, true)?);
            }
        } else if self.at("{") {
            children.push(self.block()?);
        } else if self.newline() {
            let checkpoint = self.pos;
            self.soft();
            if self.at("{") {
                children.push(self.block()?);
            } else {
                self.pos = checkpoint;
            }
        }
        Ok(self.node(
            if constructor {
                K::Constructor
            } else {
                K::Function
            },
            start,
            children,
        ))
    }

    fn class(&mut self) -> PResult {
        let saved_values = self.value_names.clone();
        let result = self.class_inner();
        self.value_names = saved_values;
        result
    }

    fn class_inner(&mut self) -> PResult {
        let start = self.start();
        let kind = if self.at("class") { K::Class } else { K::Trait };
        self.bump();
        let mut children = vec![self.name()?];
        if self.at("[") {
            children.push(self.type_parameters()?);
        }
        if self.eat("extends") {
            children.push(self.ty_atom()?);
        }
        self.soft();
        self.expect("{")?;
        self.separators();
        while !self.eof() && !self.at("}") {
            let before = self.pos;
            let start = self.start();
            match self.item(true) {
                Ok(member) => children.push(member),
                Err(()) => children.push(self.recover(start, before, true)),
            }
            if self.pos == before && !self.at("}") {
                self.bump();
            }
            self.finish_item();
        }
        self.expect("}")?;
        Ok(self.node(kind, start, children))
    }

    /// `record Point(x : Int, y : Int) { ... }` is a class whose fields are the header's items. The fields
    /// are public and read-only, and the constructor takes them in order. The body is optional (user request).
    fn record_declaration(&mut self) -> PResult {
        let start = self.start();
        let keyword = self.bump().span;
        let mut children = vec![self.name()?, Node::leaf(K::Modifier, keyword)];
        // `record Pair[A, B](first : A, second : B)` takes type parameters as a class does.
        if self.at("[") {
            children.push(self.type_parameters()?);
        }
        let header = self.parameters()?;
        for param in header.children {
            if param.children.len() < 2 {
                self.diagnostics.push(
                    Diagnostic::new("E_RECORD_FIELD", "A record field needs a type.", param.span)
                        .with_hint("Write each field as name : Type, such as x : Int."),
                );
                continue;
            }
            // The class's `record` modifier marks these as header fields, so each field
            // keeps only its own children inside its span.
            children.push(Node::new(K::Binding, param.span, param.children));
        }
        if self.eat("{") {
            self.separators();
            while !self.eof() && !self.at("}") {
                let before = self.pos;
                let member_start = self.start();
                match self.item(true) {
                    Ok(member) => children.push(member),
                    Err(()) => children.push(self.recover(member_start, before, true)),
                }
                if self.pos == before && !self.at("}") {
                    self.bump();
                }
                self.finish_item();
            }
            self.expect("}")?;
        }
        Ok(self.node(K::Class, start, children))
    }

    /// A private field introduced by a constructor: `final this.x : Int := 1`.
    /// The initializer is required, because this form is also the initialization.
    fn constructor_field(&mut self) -> PResult {
        let start = self.start();
        let mut children = vec![];
        while matches!(self.word(), "public" | "private" | "package" | "static") {
            children.push(Node::leaf(K::Modifier, self.bump().span));
        }
        // `at_constructor_field` guarantees `final` or `var` here.
        children.push(Node::leaf(K::Modifier, self.bump().span));
        let member_start = self.start();
        debug_assert!(self.at("this"));
        let receiver = Node::leaf(K::Name, self.bump().span);
        self.expect(".")?;
        let name = self.name()?;
        children.push(self.node(K::Member, member_start, vec![receiver, name]));
        self.expect(":")?;
        children.push(self.ty()?);
        self.expect(":=")?;
        children.push(self.expr(0, true)?);
        Ok(self.node(K::FieldDeclaration, start, children))
    }

    fn field(&mut self) -> PResult {
        let start = self.start();
        let modifier = Node::leaf(K::Modifier, self.bump().span);
        let name = self.name()?;
        self.expect(":")?;
        let ty = self.ty()?;
        let mut children = vec![modifier, name, ty];
        if self.eat("=") {
            children.push(self.expr(0, true)?);
        }
        Ok(self.node(K::Binding, start, children))
    }

    fn invariant(&mut self) -> PResult {
        let start = self.start();
        self.bump();
        let expression = self.expr(0, false)?;
        Ok(self.node(K::Invariant, start, vec![expression]))
    }

    fn has(&mut self) -> PResult {
        let start = self.start();
        self.bump();
        let parens = self.eat("(");
        let mut children = vec![];
        if parens {
            self.soft();
        }
        loop {
            children.push(self.ty_atom()?);
            if parens {
                self.soft();
            }
            if !self.eat(",") {
                break;
            }
            if parens {
                self.soft();
                if self.at(")") {
                    break;
                }
            }
        }
        if parens {
            self.expect(")")?;
        }
        Ok(self.node(K::Has, start, children))
    }

    fn enum_decl(&mut self) -> PResult {
        let start = self.start();
        self.bump();
        let mut children = vec![self.name()?];
        if self.at("[") {
            children.push(self.type_parameters()?);
        }
        if self.at("(") {
            children.push(self.parameters()?);
        }
        if self.eat("extends") {
            children.push(self.ty_atom()?);
        }
        self.soft();
        self.expect("{")?;
        self.separators();
        while !self.eof() && !self.at("}") {
            let before = self.pos;
            let variant_start = self.start();
            match self.variant() {
                Ok(variant) => children.push(variant),
                Err(()) => children.push(self.recover(variant_start, before, true)),
            }
            if self.pos == before && !self.at("}") {
                self.bump();
            }
            if !self.eat(",") && !self.newline() && !self.at("}") && !self.eof() {
                self.err(
                    "E_VARIANT_SEPARATOR",
                    "Expected a new line between enum cases.",
                    "Write each case on its own line.",
                );
                children.push(self.recover(self.start(), self.pos, true));
            }
            self.separators();
        }
        self.expect("}")?;
        Ok(self.node(K::Enum, start, children))
    }

    fn variant(&mut self) -> PResult {
        let start = self.start();
        let mut children = vec![self.name()?];
        if self.eat("(") {
            self.soft();
            while !self.eof() && !self.at(")") {
                let field_start = self.start();
                let named = self
                    .tokens
                    .get(self.pos + 1)
                    .is_some_and(|t| &self.text[t.span.range()] == ":");
                if named {
                    let name = self.name()?;
                    self.expect(":")?;
                    let ty = self.ty()?;
                    children.push(self.node(K::Parameter, field_start, vec![name, ty]));
                } else {
                    children.push(self.ty()?);
                }
                self.soft();
                if !self.eat(",") {
                    break;
                }
                self.soft();
            }
            self.expect(")")?;
        }
        if self.eat(":") {
            children.push(self.ty()?);
        }
        Ok(self.node(K::Variant, start, children))
    }

    fn ty(&mut self) -> PResult {
        self.nested(|p| {
            p.soft();
            let start = p.start();
            let mut ty = p.ty_atom()?;
            if p.eat("or") {
                let right = p.ty()?;
                ty = p.node(K::UnionType, start, vec![ty, right]);
            }
            if p.eat("->") || p.eat("→") || p.eat("\\to") {
                let right = p.ty()?;
                let mut children = vec![ty, right];
                // `A -> B with Effects` is a function type that may do outside work (Q50).
                let effects = p.at("with")
                    && p.tokens
                        .get(p.pos + 1)
                        .is_some_and(|t| &p.text[t.span.range()] == "Effects");
                if effects && !p.declaration_return {
                    let effects_start = p.start();
                    p.eat("with");
                    p.expect("Effects")?;
                    children.push(p.node(K::Effects, effects_start, vec![]));
                }
                ty = p.node(K::FunctionType, start, children);
            }
            Ok(ty)
        })
    }

    fn ty_atom(&mut self) -> PResult {
        // Parentheses and brackets start a new type, where `with Effects` belongs to it.
        let previous = std::mem::replace(&mut self.declaration_return, false);
        let ty = self.nested(|p| p.ty_atom_inner());
        self.declaration_return = previous;
        ty
    }

    fn ty_atom_inner(&mut self) -> PResult {
        self.soft();
        let start = self.start();
        if self.eat("&") {
            let inner = self.ty_atom()?;
            return Ok(self.node(K::BorrowType, start, vec![inner]));
        }
        if self.eat("mut") {
            let inner = self.ty_atom()?;
            return Ok(self.node(K::MutableType, start, vec![inner]));
        }
        if self.eat("dyn") {
            let inner = self.ty_atom()?;
            return Ok(self.node(K::DynType, start, vec![inner]));
        }
        let mut ty = if self.eat("(") {
            self.soft();
            let mut types = vec![];
            while !self.eof() && !self.at(")") {
                let offset = usize::from(matches!(self.word(), "mut" | "move"));
                let named = self
                    .tokens
                    .get(self.pos + offset + 1)
                    .is_some_and(|token| &self.text[token.span.range()] == ":");
                if named {
                    let param_start = self.start();
                    let mut children = vec![];
                    if offset != 0 {
                        children.push(Node::leaf(K::Modifier, self.bump().span));
                    }
                    children.push(self.name()?);
                    self.expect(":")?;
                    children.push(self.ty()?);
                    types.push(self.node(K::Parameter, param_start, children));
                } else {
                    types.push(self.ty()?);
                }
                self.soft();
                if !self.eat(",") {
                    break;
                }
                self.soft();
            }
            self.expect(")")?;
            self.node(K::TupleType, start, types)
        } else {
            let name = self.path()?;
            self.node(K::TypeName, start, vec![name])
        };
        let mut steps = 0;
        loop {
            steps += 1;
            if steps > MAX_DEPTH {
                self.err(
                    "E_NESTING_LIMIT",
                    "This type has too many nested applications.",
                    "Give part of this type an alias.",
                );
                return Err(());
            }
            if self.eat("[") {
                let mut children = vec![ty];
                self.soft();
                while !self.eof() && !self.at("]") {
                    if children.len() == 2
                        && crate::lexer::path_name(&self.text[children[0].span.range()])
                            .rsplit('.')
                            .next()
                            == Some("Tensor")
                    {
                        children.push(self.tensor_shape()?);
                    } else {
                        children.push(self.ty()?);
                    }
                    self.soft();
                    if !self.eat(",") {
                        break;
                    }
                    self.soft();
                }
                self.expect("]")?;
                ty = self.node(K::TypeApplication, start, children);
            } else if self.eat("(") {
                let mut children = vec![ty];
                self.soft();
                while !self.eof() && !self.at(")") {
                    let named = self
                        .tokens
                        .get(self.pos + 1)
                        .is_some_and(|t| &self.text[t.span.range()] == "=");
                    if named {
                        let assoc_start = self.start();
                        let name = self.name()?;
                        self.expect("=")?;
                        let value = self.ty()?;
                        children.push(self.node(K::AssociatedType, assoc_start, vec![name, value]));
                    } else {
                        children.push(self.expr(0, true)?);
                    }
                    self.soft();
                    if !self.eat(",") {
                        break;
                    }
                    self.soft();
                }
                self.expect(")")?;
                ty = self.node(K::IndexedType, start, children);
            } else {
                break;
            }
        }
        Ok(ty)
    }

    fn tensor_shape(&mut self) -> PResult {
        let start = self.start();
        self.expect("(")?;
        self.soft();
        let mut dimensions = vec![];
        while !self.eof() && !self.at(")") {
            dimensions.push(self.expr(0, true)?);
            self.soft();
            if !self.eat(",") {
                break;
            }
            self.soft();
        }
        self.expect(")")?;
        Ok(self.node(K::ShapeType, start, dimensions))
    }

    fn statement(&mut self) -> PResult {
        let start = self.start();
        match self.word() {
            "let" => self.binding(false),
            "for" => self.for_loop(),
            "while" => {
                self.bump();
                let proof = self.proof_name();
                let cond = self.expr(0, false)?;
                self.soft();
                let mut body = self.block()?;
                if let Some(name) = proof {
                    body = proof_binding(name, &cond, body);
                }
                Ok(self.node(K::While, start, vec![cond, body]))
            }
            "return" => {
                self.bump();
                let mut children = vec![];
                // A comma ends a match arm, so `=> return,` returns nothing (Q122).
                if !self.newline() && !self.at(";") && !self.at("}") && !self.at(",") && !self.eof()
                {
                    children.push(self.expr(0, true)?);
                }
                Ok(self.node(K::Return, start, children))
            }
            "break" | "continue" => {
                let kind = if self.at("break") {
                    K::Break
                } else {
                    K::Continue
                };
                self.bump();
                Ok(self.node(kind, start, vec![]))
            }
            _ => {
                let lhs = self.expr(0, true)?;
                if self.at("++") || self.at("--") {
                    let token = self.bump();
                    if !matches!(lhs.kind, K::Name | K::Member | K::Index) {
                        self.diagnostics.push(
                            Diagnostic::new(
                                "E_ASSIGNMENT_TARGET",
                                "Only an existing mutable name, field or Array slot can appear before `++` or `--`.",
                                lhs.span,
                            )
                            .with_hint("Use a variable, field or Array slot here."),
                        );
                    }
                    let operator = Node::leaf(
                        K::Operator,
                        Span::new(token.span.start, token.span.start + 1),
                    );
                    let one = Node::leaf(K::One, token.span);
                    let rhs = Node::new(
                        K::Binary,
                        Span::new(lhs.span.start, token.span.end),
                        vec![lhs.clone(), operator, one],
                    );
                    return Ok(self.node(K::Assignment, start, vec![lhs, rhs]));
                }
                if self.at_compound() {
                    // `place op= value` is `place = place op value`, so the place is read twice (Q117).
                    let token = self.bump();
                    if !matches!(lhs.kind, K::Name | K::Member | K::Index) || !pure_place(&lhs) {
                        self.diagnostics.push(
                            Diagnostic::new(
                                "E_COMPOUND_TARGET",
                                "A compound assignment reads its target twice, so the target can contain no calls.",
                                lhs.span,
                            )
                            .with_hint("Store the call's result in a name, then assign with that name."),
                        );
                    }
                    let value = self.expr(0, true)?;
                    let operator = Node::leaf(
                        K::Operator,
                        Span::new(token.span.start, token.span.start + 1),
                    );
                    let rhs = Node::new(
                        K::Binary,
                        Span::new(lhs.span.start, value.span.end),
                        vec![lhs.clone(), operator, value],
                    );
                    return Ok(self.node(K::Assignment, start, vec![lhs, rhs]));
                }
                if self.eat(":=") {
                    if lhs.kind != K::Name {
                        self.err(
                            "E_BINDING_PATTERN",
                            "Expected a new name before `:=`.",
                            "Use `let` for a destructuring pattern.",
                        );
                    }
                    let rhs = self.expr(0, true)?;
                    self.value_names.insert(self.text[lhs.span.range()].into());
                    Ok(self.node(K::Binding, start, vec![lhs, rhs]))
                } else if self.eat("=") {
                    self.assignment(start, lhs)
                } else {
                    Ok(lhs)
                }
            }
        }
    }

    /// `+=`, `-=`, `*=`, `/=` or `%=` at the start of the current token (Q117).
    fn at_compound(&self) -> bool {
        matches!(self.word(), "+=" | "-=" | "*=" | "/=" | "%=")
    }

    fn assignment(&mut self, start: usize, lhs: Node) -> PResult {
        // A tuple of places assigns each item, which the checker checks place by place (Q130).
        if !matches!(lhs.kind, K::Name | K::Member | K::Index | K::Tuple) {
            self.diagnostics.push(
                Diagnostic::new(
                    "E_ASSIGNMENT_TARGET",
                    "Only an existing mutable name, field or Array slot can appear before `=`.",
                    lhs.span,
                )
                .with_hint(
                    "Use `==` for a comparison. Equality written `=` belongs in a theorem header.",
                ),
            );
        }
        let rhs = self.expr(0, true)?;
        Ok(self.node(K::Assignment, start, vec![lhs, rhs]))
    }

    fn value_pattern(&mut self) -> PResult {
        let previous = std::mem::replace(&mut self.binding_pattern, true);
        let result = self.pattern();
        self.binding_pattern = previous;
        result
    }

    fn binding(&mut self, constant: bool) -> PResult {
        let start = self.start();
        self.bump();
        let mut children = vec![];
        if self.at("mut") {
            children.push(Node::leaf(K::Modifier, self.bump().span));
        }
        children.push(self.value_pattern()?);
        if self.eat(":") {
            children.push(self.ty()?);
            // A typed binding normally uses `=`, but `:=` reads naturally beside
            // inferred declarations, so accept both spellings.
            if !self.eat("=") && !self.eat(":=") {
                self.expect("=")?;
            }
        } else {
            self.expect(":=")?;
        }
        let value = self.expr(0, true)?;
        if self.eat("=") {
            self.diagnostics.push(
                Diagnostic::new(
                    "E_ASSIGNMENT_TARGET",
                    "An initializer cannot contain an assignment written `=`.",
                    value.span,
                )
                .with_hint(
                    "Use `==` for a comparison. Equality written `=` belongs in a theorem header.",
                ),
            );
            let right = self.expr(0, true)?;
            children.push(self.node(K::Error, value.span.start, vec![value, right]));
        } else {
            children.push(value);
        }
        if self.eat("otherwise") {
            let start = self.tokens[self.pos - 1].span.start;
            let body = self.block()?;
            children.push(self.node(K::Otherwise, start, vec![body]));
        }
        Ok(self.node(
            if constant { K::Const } else { K::Binding },
            start,
            children,
        ))
    }

    fn for_loop(&mut self) -> PResult {
        let saved_values = self.value_names.clone();
        let result = self.for_loop_inner();
        self.value_names = saved_values;
        result
    }

    fn for_loop_inner(&mut self) -> PResult {
        let start = self.start();
        self.bump();
        self.expect("(")?;
        self.soft();
        let init_start = self.start();
        let mut init = vec![];
        if self.eat("let") && self.at("mut") {
            init.push(Node::leaf(K::Modifier, self.bump().span));
        }
        init.push(self.value_pattern()?);
        if self.eat(":") {
            init.push(self.ty()?);
        }
        let mut children = vec![];
        if self.eat("in") {
            init.push(self.expr(0, false)?);
            children.push(self.node(K::ForBinding, init_start, init));
        } else {
            if !self.eat("=") && !self.eat(":=") {
                self.expect("=")?;
            }
            init.push(self.expr(0, true)?);
            children.push(self.node(K::Binding, init_start, init));
            self.expect(";")?;
            children.push(self.expr(0, true)?);
            self.expect(";")?;
            children.push(self.statement()?);
        }
        self.soft();
        self.expect(")")?;
        self.soft();
        children.push(self.block()?);
        Ok(self.node(K::For, start, children))
    }

    fn block(&mut self) -> PResult {
        let saved_values = self.value_names.clone();
        let result = self.block_inner();
        self.value_names = saved_values;
        result
    }

    fn block_inner(&mut self) -> PResult {
        self.nested(|p| {
            let start = p.start();
            p.expect("{")?;
            let mut children = vec![];
            p.separators();
            while !p.eof() && !p.at("}") {
                let before = p.pos;
                let start = p.start();
                match p.item(false) {
                    Ok(node) => children.push(node),
                    Err(()) => children.push(p.recover(start, before, true)),
                }
                if p.pos == before && !p.at("}") {
                    p.bump();
                }
                p.finish_item();
            }
            p.expect("}")?;
            Ok(p.node(K::Block, start, children))
        })
    }

    /// Whether the brace at the cursor starts `{x : U | P}` rather than a block.
    /// A colon after the first name cannot begin an expression, so the two
    /// forms stay apart in every context.
    fn set_builder_ahead(&self) -> bool {
        let mut offset = 1;
        while self
            .tokens
            .get(self.pos + offset)
            .is_some_and(|token| token.kind == TokenKind::Newline)
        {
            offset += 1;
        }
        let word = self.peek_word(offset);
        self.tokens
            .get(self.pos + offset)
            .is_some_and(|token| token.kind == TokenKind::Word)
            && !reserved(word)
            && self.peek_word(offset + 1) == ":"
    }

    /// A set written with braces: the builder `{x : U | P}` or a finite set
    /// `{a, b}`. The empty braces are the empty set.
    fn set_braces(&mut self) -> PResult {
        self.nested(|p| {
            let start = p.start();
            p.expect("{")?;
            p.soft();
            if p.token().kind == TokenKind::Word && !reserved(p.word()) && p.peek_word(1) == ":" {
                let name = p.name()?;
                p.expect(":")?;
                let ty = p.ty()?;
                p.soft();
                p.expect("|")?;
                let previous = p.proposition;
                p.proposition = true;
                let body = p.expr(0, true);
                p.proposition = previous;
                let body = body?;
                p.soft();
                p.expect("}")?;
                return Ok(p.node(K::SetBuilder, start, vec![name, ty, body]));
            }
            let mut children = vec![];
            while !p.eof() && !p.at("}") {
                children.push(p.expr(0, true)?);
                p.soft();
                if !p.eat(",") {
                    break;
                }
                p.soft();
            }
            p.soft();
            p.expect("}")?;
            Ok(p.node(K::SetLiteral, start, children))
        })
    }

    fn expr(&mut self, min_bp: u8, records: bool) -> PResult {
        self.nested(|p| p.expr_inner(min_bp, records))
    }

    fn expr_inner(&mut self, min_bp: u8, records: bool) -> PResult {
        self.soft();
        let start = self.start();
        let mut lhs = self.prefix(records)?;
        let mut steps = 0;
        loop {
            if self.newline() {
                break;
            }
            steps += 1;
            if steps > MAX_DEPTH {
                self.err(
                    "E_NESTING_LIMIT",
                    "This expression has too many chained operations.",
                    "Split the chain into named steps.",
                );
                return Err(());
            }
            if self.at("(") {
                lhs = self.call(lhs)?;
                continue;
            }
            if self.eat(".") {
                let name = self.name()?;
                lhs = self.node(K::Member, start, vec![lhs, name]);
                continue;
            }
            // Only type names take bracket arguments, so `a[i]` on anything else indexes (Q53).
            if self.at("[") && !type_head(&lhs, self.text, &self.value_names) {
                self.bump();
                self.soft();
                let index_start = self.start();
                let mut indices = vec![];
                while !self.eof() && !self.at("]") {
                    let part_start = self.start();
                    let low = if self.at(":") {
                        Node::leaf(K::Hole, Span::new(part_start, part_start))
                    } else {
                        self.expr(0, true)?
                    };
                    self.soft();
                    let part = if self.eat(":") {
                        self.soft();
                        let high = if self.at(",") || self.at("]") {
                            Node::leaf(K::Hole, Span::new(self.start(), self.start()))
                        } else {
                            self.expr(0, true)?
                        };
                        self.node(K::Slice, part_start, vec![low, high])
                    } else {
                        low
                    };
                    indices.push(part);
                    self.soft();
                    if !self.eat(",") {
                        break;
                    }
                    self.soft();
                }
                self.expect("]")?;
                let index = if indices.len() == 1 {
                    indices.pop().unwrap()
                } else {
                    self.node(K::Tuple, index_start, indices)
                };
                lhs = self.node(K::Index, start, vec![lhs, index]);
                continue;
            }
            if self.at("[") {
                let mut children = vec![lhs];
                self.bump();
                self.soft();
                while !self.eof() && !self.at("]") {
                    children.push(self.ty()?);
                    self.soft();
                    if !self.eat(",") {
                        break;
                    }
                    self.soft();
                }
                self.expect("]")?;
                lhs = self.node(K::BracketApplication, start, children);
                continue;
            }
            if records
                && self.at("{")
                && lhs.kind == K::Name
                && self.text[lhs.span.range()]
                    .chars()
                    .next()
                    .is_some_and(crate::identifiers::type_start)
            {
                lhs = self.record(lhs)?;
                continue;
            }
            if self.eat("?") {
                lhs = self.node(K::Propagate, start, vec![lhs]);
                continue;
            }
            // `Aᶜ` is postfix. `\compl A` is read in `prefix` instead.
            if self.at("ᶜ") {
                let op = Node::leaf(K::Operator, self.bump().span);
                lhs = self.node(K::Unary, start, vec![op, lhs]);
                continue;
            }
            if self.at("**") && power(&lhs, self.text) {
                self.err(
                    "E_POWER_GROUPING",
                    "A chain of `**` needs parentheses.",
                    "Write 2 ** (3 ** 2) or (2 ** 3) ** 2 (Q11).",
                );
                return Err(());
            }
            let custom = self.custom_operators.get(self.word()).copied();
            if let Some(custom) = custom {
                if custom.left_bp < min_bp {
                    break;
                }
                let op_span = self.bump().span;
                let rhs = self.expr(custom.right_bp, records)?;
                let callee = Node::leaf(K::Name, custom.function);
                let left = Node::new(K::Argument, lhs.span, vec![lhs]);
                let right = Node::new(K::Argument, rhs.span, vec![rhs]);
                lhs = self.node(K::Call, start, vec![callee, left, right]);
                let _ = op_span;
                continue;
            }
            let Some((left_bp, right_bp)) = infix(self.word(), self.proposition) else {
                break;
            };
            if left_bp < min_bp {
                break;
            }
            let pipe = self.at("|>");
            let op = Node::leaf(K::Operator, self.bump().span);
            let rhs = self.expr(right_bp, records)?;
            lhs = self.node(
                if pipe { K::Pipe } else { K::Binary },
                start,
                vec![lhs, op, rhs],
            );
        }
        Ok(lhs)
    }

    fn prefix(&mut self, records: bool) -> PResult {
        let start = self.start();
        if let Some(lambda) = self.try_lambda() {
            return lambda;
        }
        match self.word() {
            "proof" if !self.proving => {
                self.bump();
                let claim = self.expr(0, false)?;
                Ok(self.node(K::ProofWitness, start, vec![claim]))
            }
            "-" | "+" | "!" | "~" => {
                let op = Node::leaf(K::Operator, self.bump().span);
                let value = self.expr(12, records)?;
                if power(&value, self.text) {
                    self.diagnostics.push(
                        Diagnostic::new(
                            "E_POWER_GROUPING",
                            "A sign before the base of `**` needs parentheses.",
                            Span::new(start, value.span.end),
                        )
                        .with_hint("Write -(2 ** 2) or (-2) ** 2 (Q11)."),
                    );
                }
                Ok(self.node(K::Unary, start, vec![op, value]))
            }
            "induction" if self.proving => self.induction(),
            "smt" if self.proving => {
                self.bump();
                Ok(self.node(K::Smt, start, vec![]))
            }
            "simp" if self.proving => {
                self.bump();
                let mut rules = vec![];
                if self.at("[") && !self.newline() {
                    self.bump();
                    while !self.eof() && !self.at("]") {
                        rules.push(self.rewrite_rule()?);
                        if !self.eat(",") {
                            break;
                        }
                    }
                    self.expect("]")?;
                }
                Ok(self.node(K::Simp, start, rules))
            }
            "not" | "¬" | "\\not" | "\\neg" if self.proposition => {
                let op = Node::leaf(K::Operator, self.bump().span);
                let value = self.expr(12, records)?;
                Ok(self.node(K::Unary, start, vec![op, value]))
            }
            "\\compl" => {
                let op = Node::leaf(K::Operator, self.bump().span);
                let value = self.expr(12, records)?;
                Ok(self.node(K::Unary, start, vec![op, value]))
            }
            "⋃₀" | "⋂₀" | "\\union0" | "\\inter0" => {
                let op = Node::leaf(K::Operator, self.bump().span);
                let value = {
                    let previous = self.proposition;
                    self.proposition = true;
                    let value = self.expr(12, records);
                    self.proposition = previous;
                    value?
                };
                Ok(self.node(K::Unary, start, vec![op, value]))
            }
            "∅" | "\\emptyset" | "\\empty" => {
                self.bump();
                Ok(self.node(K::SetLiteral, start, vec![]))
            }
            "forall" | "exists" | "∀" | "∃" | "\\forall" | "\\exists" if self.proposition => {
                self.quantifier()
            }
            "_" => Ok(Node::leaf(K::Hole, self.bump().span)),
            "true" | "false" => Ok(Node::leaf(K::Literal, self.bump().span)),
            "if" => self.if_expr(),
            "match" => self.match_expr(),
            "{" if self.proposition || self.set_builder_ahead() => self.set_braces(),
            "{" => self.block(),
            "[" => self.list(),
            "(" => self.group(),
            "unsafe" | "scope" => {
                let kind = if self.at("unsafe") {
                    K::Unsafe
                } else {
                    K::Scope
                };
                self.bump();
                self.soft();
                let body = self.block()?;
                Ok(self.node(kind, start, vec![body]))
            }
            "using" => {
                self.bump();
                let name = self.name()?;
                self.expect(":=")?;
                let resource = self.expr(0, false)?;
                self.soft();
                let body = self.block()?;
                Ok(self.node(K::Using, start, vec![name, resource, body]))
            }
            "spawn" => {
                self.bump();
                let call = self.expr(0, records)?;
                Ok(self.node(K::Spawn, start, vec![call]))
            }
            _ => match self.token().kind {
                TokenKind::Integer | TokenKind::Double | TokenKind::String => {
                    Ok(Node::leaf(K::Literal, self.bump().span))
                }
                TokenKind::FormatString => self.format_string(),
                TokenKind::Word if !reserved(self.word()) => self.name(),
                _ => {
                    self.err(
                        "E_EXPRESSION",
                        format!("Expected a value or expression, found `{}`.", self.word()),
                        "Put a value, name or expression here.",
                    );
                    Err(())
                }
            },
        }
    }

    fn try_lambda(&mut self) -> Option<PResult> {
        let saved_values = self.value_names.clone();
        let result = self.try_lambda_inner();
        self.value_names = saved_values;
        result
    }

    /// The `=>` that ends a guard: the first one outside brackets. A lambda
    /// arrow inside brackets belongs to the lambda, not to the arm.
    fn ending_arrow(&self, start: usize) -> Option<usize> {
        let mut nesting = 0usize;
        for (index, token) in self.tokens.iter().enumerate().skip(start) {
            match &self.text[token.span.range()] {
                "(" | "[" | "{" => nesting += 1,
                ")" | "]" | "}" => nesting = nesting.saturating_sub(1),
                "=>" if nesting == 0 => return Some(index),
                _ => {}
            }
        }
        None
    }

    fn try_lambda_inner(&mut self) -> Option<PResult> {
        if !self.at("(") && !self.at("move") && self.token().kind != TokenKind::Word {
            return None;
        }
        if reserved(self.word()) && !self.at("move") {
            return None;
        }
        if !self.at("move") {
            if self.at("(") {
                let mut nesting = 0usize;
                let mut after = None;
                for (index, token) in self.tokens.iter().enumerate().skip(self.pos) {
                    match &self.text[token.span.range()] {
                        "(" => nesting += 1,
                        ")" => {
                            nesting -= 1;
                            if nesting == 0 {
                                after = Some((index + 1, self.tokens.get(index + 1)));
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let Some((after_index, Some(after))) = after else {
                    return None;
                };
                if self.guard_arrow == Some(after_index) {
                    return None;
                }
                if !matches!(&self.text[after.span.range()], ":" | "=>") {
                    return None;
                }
            } else {
                if self.guard_arrow == Some(self.pos + 1) {
                    return None;
                }
                if !self
                    .tokens
                    .get(self.pos + 1)
                    .is_some_and(|t| matches!(&self.text[t.span.range()], ":" | "=>"))
                {
                    return None;
                }
            }
        }
        let saved = self.pos;
        let diagnostics = self.diagnostics.len();
        let start = self.start();
        let parsed = (|| -> PResult {
            let mut children = vec![];
            if self.at("move") {
                children.push(Node::leaf(K::Modifier, self.bump().span));
            }
            if self.at("(") {
                let mut params = self.parameters()?;
                params.kind = K::LambdaParameters;
                children.push(params);
            } else {
                let param_start = self.start();
                let name = self.name()?;
                self.value_names.insert(self.text[name.span.range()].into());
                let mut param = vec![name];
                if self.eat(":") {
                    param.push(self.ty()?);
                }
                let param = self.node(K::Parameter, param_start, param);
                children.push(self.node(K::LambdaParameters, param_start, vec![param]));
            }
            if self.eat(":") {
                let ty = self.ty()?;
                children.push(self.node(K::ReturnType, ty.span.start, vec![ty]));
            }
            self.expect("=>")?;
            Ok(self.node(K::Lambda, start, children))
        })();
        match parsed {
            Ok(mut lambda) => {
                let body = match self.expr(0, true) {
                    Ok(body) => body,
                    Err(()) => return Some(Err(())),
                };
                lambda.span.end = body.span.end;
                lambda.children.push(body);
                Some(Ok(lambda))
            }
            Err(()) => {
                self.pos = saved;
                self.diagnostics.truncate(diagnostics);
                None
            }
        }
    }

    fn group(&mut self) -> PResult {
        let start = self.start();
        self.bump();
        self.soft();
        let mut children = vec![];
        let mut tuple = self.at(")");
        while !self.eof() && !self.at(")") {
            children.push(self.expr(0, true)?);
            self.soft();
            if !self.eat(",") {
                break;
            }
            tuple = true;
            self.soft();
        }
        self.expect(")")?;
        Ok(self.node(if tuple { K::Tuple } else { K::Group }, start, children))
    }

    fn call(&mut self, callee: Node) -> PResult {
        let start = callee.span.start;
        let map = callee.kind == K::Name && &self.text[callee.span.range()] == "Map";
        self.bump();
        let axis_label = callee.kind == K::Member
            && matches!(
                &self.text[callee.children[1].span.range()],
                "sum" | "mean" | "min" | "max"
            );
        let mut children = vec![callee];
        let mut named = false;
        self.soft();
        while !self.eof() && !self.at(")") {
            let arg_start = self.start();
            let is_named = self.token().kind == TokenKind::Word
                && self.tokens.get(self.pos + 1).is_some_and(|t| {
                    &self.text[t.span.range()] == "="
                        || axis_label && self.word() == "axis" && &self.text[t.span.range()] == ":"
                });
            let argument = if is_named {
                named = true;
                let name = self.name()?;
                self.bump();
                let value = self.expr(0, true)?;
                self.node(K::NamedArgument, arg_start, vec![name, value])
            } else {
                if named {
                    self.err(
                        "E_ARGUMENT_ORDER",
                        "A positional argument follows a named argument.",
                        "Put positional arguments first, or give this argument its parameter name.",
                    );
                }
                let left = self.expr(0, true)?;
                if map && self.eat("=>") {
                    let right = self.expr(0, true)?;
                    self.node(K::ArrowArgument, arg_start, vec![left, right])
                } else {
                    if map
                        && left.kind == K::Lambda
                        && left.children.first().is_some_and(|p| {
                            p.kind == K::LambdaParameters
                                && p.children.len() == 1
                                && p.children[0].children.len() == 1
                        })
                    {
                        let mut key = left.children[0].children[0].children[0].clone();
                        if matches!(&self.text[key.span.range()], "true" | "false") {
                            key.kind = K::Literal;
                        }
                        let value = left.children.last().unwrap().clone();
                        self.node(K::ArrowArgument, arg_start, vec![key, value])
                    } else {
                        self.node(K::Argument, arg_start, vec![left])
                    }
                }
            };
            children.push(argument);
            self.soft();
            if !self.eat(",") {
                break;
            }
            self.soft();
        }
        self.expect(")")?;
        Ok(self.node(K::Call, start, children))
    }

    fn record(&mut self, name: Node) -> PResult {
        let start = name.span.start;
        let mut children = vec![name];
        self.bump();
        self.soft();
        while !self.eof() && !self.at("}") {
            let start = self.start();
            let name = self.name()?;
            self.expect(":")?;
            let value = self.expr(0, true)?;
            children.push(self.node(K::RecordField, start, vec![name, value]));
            self.soft();
            if !self.eat(",") {
                break;
            }
            self.soft();
        }
        self.expect("}")?;
        Ok(self.node(K::Record, start, children))
    }

    fn list(&mut self) -> PResult {
        let saved_values = self.value_names.clone();
        let result = self.list_inner();
        self.value_names = saved_values;
        result
    }

    fn list_inner(&mut self) -> PResult {
        let start = self.start();
        self.bump();
        self.soft();
        let mut children = vec![];
        if self.eat("]") {
            return Ok(self.node(K::List, start, children));
        }
        // A comprehension's element precedes its binder in the source. Read the
        // binder first so an uppercase receiver such as Word[0] is an index.
        let mut nested = 0_usize;
        for position in self.pos..self.tokens.len() {
            match &self.text[self.tokens[position].span.range()] {
                "]" if nested == 0 => break,
                "(" | "[" | "{" => nested += 1,
                ")" | "]" | "}" => nested = nested.saturating_sub(1),
                "for" if nested == 0 => {
                    let saved_pos = self.pos;
                    let diagnostics = self.diagnostics.len();
                    self.pos = position + 1;
                    let _ = self.value_pattern();
                    self.pos = saved_pos;
                    self.diagnostics.truncate(diagnostics);
                    break;
                }
                _ => {}
            }
        }
        children.push(self.expr(0, true)?);
        if self.eat("for") {
            let pattern = self.value_pattern()?;
            self.expect("in")?;
            let collection = self.expr(0, true)?;
            children.extend([pattern, collection]);
            if self.eat("if") {
                let start = self.tokens[self.pos - 1].span.start;
                let cond = self.expr(0, true)?;
                children.push(self.node(K::Guard, start, vec![cond]));
            }
            // `[(x, y) for x in xs for y in ys]` runs the later clauses inside the earlier ones.
            loop {
                self.soft();
                if !self.at("for") {
                    break;
                }
                let clause_start = self.tokens[self.pos].span.start;
                self.pos += 1;
                let pattern = self.value_pattern()?;
                self.expect("in")?;
                let collection = self.expr(0, true)?;
                let mut clause = vec![pattern, collection];
                if self.eat("if") {
                    let start = self.tokens[self.pos - 1].span.start;
                    let cond = self.expr(0, true)?;
                    clause.push(self.node(K::Guard, start, vec![cond]));
                }
                children.push(self.node(K::ComprehensionFor, clause_start, clause));
            }
            self.soft();
            self.expect("]")?;
            return Ok(self.node(K::Comprehension, start, children));
        }
        self.soft();
        while self.eat(",") {
            self.soft();
            if self.at("]") {
                break;
            }
            children.push(self.expr(0, true)?);
            self.soft();
        }
        self.expect("]")?;
        Ok(self.node(K::List, start, children))
    }

    fn if_expr(&mut self) -> PResult {
        self.nested(|p| p.if_expr_inner())
    }

    /// `h :` right after `if` or `while` names the proof of the condition for its block (Q115).
    fn proof_name(&mut self) -> Option<Span> {
        if self.token().kind != TokenKind::Word || reserved(self.word()) {
            return None;
        }
        let colon = self.tokens.get(self.pos + 1)?;
        if &self.text[colon.span.range()] != ":" {
            return None;
        }
        let name = self.bump().span;
        self.bump();
        Some(name)
    }

    /// `proof` before a parameter name, when a name follows it (Q115).
    fn proof_modifier(&self) -> bool {
        self.word() == "proof"
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.kind == TokenKind::Word)
    }

    fn modifier_is(&self, node: &Node, word: &str) -> bool {
        node.kind == K::Modifier && &self.text[node.span.range()] == word
    }

    /// `if let Some(v) := m { a } else { b }`: a match with two arms. The first binds the pattern,
    /// and the second takes every other value (Q115).
    fn if_let(&mut self, start: usize) -> PResult {
        self.bump();
        let arm_start = self.start();
        let pattern = self.pattern()?;
        self.expect(":=")?;
        let scrutinee = self.expr(0, false)?;
        self.soft();
        let body = self.block()?;
        let found = self.node(K::MatchArm, arm_start, vec![pattern, body]);
        self.soft();
        self.expect("else")?;
        let else_span = self.tokens[self.pos - 1].span;
        self.soft();
        let otherwise = if self.at("if") {
            self.if_expr()?
        } else {
            self.block()?
        };
        let rest = self.node(
            K::MatchArm,
            else_span.start,
            vec![Node::leaf(K::PatternWildcard, else_span), otherwise],
        );
        Ok(self.node(K::Match, start, vec![scrutinee, found, rest]))
    }

    fn if_expr_inner(&mut self) -> PResult {
        let start = self.start();
        self.bump();
        if self.at("let") {
            return self.if_let(start);
        }
        let proof = self.proof_name();
        let cond = self.expr(0, false)?;
        let mut children = vec![cond.clone()];
        if self.eat("then") {
            if let Some(name) = proof {
                self.diagnostics.push(
                    Diagnostic::new(
                        "E_PROOF_BLOCK",
                        "A named proof needs a block after its condition.",
                        name,
                    )
                    .with_hint("Write the branch in braces: if h : cond { ... }."),
                );
            }
            children.push(self.expr(0, true)?);
            self.soft();
            self.expect("else")?;
            children.push(self.expr(0, true)?);
        } else {
            self.soft();
            let mut body = self.block()?;
            if let Some(name) = proof {
                body = proof_binding(name, &cond, body);
            }
            children.push(body);
            let checkpoint = self.pos;
            self.soft();
            if self.eat("else") {
                self.soft();
                children.push(if self.at("if") {
                    self.if_expr()?
                } else {
                    self.block()?
                });
            } else {
                self.pos = checkpoint;
            }
        }
        Ok(self.node(K::If, start, children))
    }

    /// `forall n : Nat, P` or `forall (n : Nat), P`. The binder lives only
    /// inside the body, so it is scoped by `value_names`.
    fn quantifier(&mut self) -> PResult {
        let start = self.start();
        let exists = matches!(self.word(), "exists" | "∃" | "\\exists");
        self.bump();
        let mut binders = vec![];
        let binder_start = self.start();
        if self.eat("(") {
            let mut names = vec![];
            while !self.eof() && !self.at(":") {
                names.push(self.name()?);
                self.soft();
            }
            self.expect(":")?;
            let ty = self.ty()?;
            self.soft();
            self.expect(")")?;
            for name in names {
                let span = Span::new(name.span.start, ty.span.end);
                binders.push(Node::new(K::Parameter, span, vec![name, ty.clone()]));
            }
        } else {
            let name = self.name()?;
            self.expect(":")?;
            let ty = self.ty()?;
            let span = Span::new(name.span.start, ty.span.end);
            binders.push(Node::new(K::Parameter, span, vec![name, ty]));
        }
        if binders.is_empty() {
            self.err(
                "E_QUANTIFIER_BINDER",
                "This quantifier has no variable.",
                "Write forall n : Nat, ... or exists n : Nat, ...",
            );
            return Err(());
        }
        self.expect(",")?;
        let saved_values = self.value_names.clone();
        for binder in &binders {
            if let Some(name) = binder.child(K::Name) {
                self.value_names.insert(self.text[name.span.range()].into());
            }
        }
        self.soft();
        let previous = self.proposition;
        self.proposition = true;
        let body = self.expr(0, false);
        self.proposition = previous;
        self.value_names = saved_values;
        let body = body?;
        let mut children = binders;
        children.push(body);
        let _ = binder_start;
        Ok(self.node(if exists { K::Exists } else { K::Forall }, start, children))
    }

    /// `by { intro n; induction n { ... } }` after a theorem statement.
    fn tactic_proof(&mut self) -> PResult {
        let start = self.start();
        self.bump();
        self.soft();
        let script = self.tactic_script()?;
        Ok(self.node(K::TacticProof, start, vec![script]))
    }

    fn tactic_script(&mut self) -> PResult {
        self.nested(|p| {
            let start = p.start();
            p.expect("{")?;
            p.separators();
            let mut children = vec![];
            while !p.eof() && !p.at("}") {
                let before = p.pos;
                let diagnostics = p.diagnostics.len();
                match p.tactic() {
                    Ok(tactic) => children.push(tactic),
                    Err(()) => {
                        // Keep a tactic node so a half-written line still
                        // leaves the checker enough structure to report the
                        // goal. The kernel reports the malformed tactic.
                        p.diagnostics.truncate(diagnostics);
                        let token = p.tokens.get(before).copied();
                        if let Some(token) = token {
                            let keyword = Node::leaf(K::Name, token.span);
                            let end = p.start().max(token.span.end);
                            children.push(Node::new(
                                K::Tactic,
                                Span::new(token.span.start, end),
                                vec![keyword],
                            ));
                        }
                        p.separators();
                        if p.pos == before && !p.at("}") {
                            p.bump();
                        }
                        continue;
                    }
                }
                if !p.eof() && !p.at("}") && !p.newline() && !p.at(";") {
                    let found = p.word().to_owned();
                    p.err(
                        "E_TACTIC_SEPARATOR",
                        format!("Expected a new line or `;` after this tactic, found `{found}`."),
                        "Put each tactic on its own line.",
                    );
                    while !p.eof() && !p.at("}") && !p.newline() {
                        p.bump();
                    }
                }
                p.separators();
            }
            if !p.eof() {
                p.expect("}")?;
            }
            Ok(p.node(K::TacticScript, start, children))
        })
    }

    fn tactic_block(&mut self) -> PResult {
        self.tactic_script()
    }

    fn tactic_names(&mut self) -> Result<Vec<Node>, ()> {
        let mut names = vec![];
        while self.token().kind == TokenKind::Word
            && !reserved(self.word())
            && !matches!(self.word(), "by" | "at")
        {
            names.push(self.name()?);
            if self.newline() || self.at(";") || self.at("}") {
                break;
            }
        }
        Ok(names)
    }

    fn tactic(&mut self) -> PResult {
        let start = self.start();
        if self.at("{") {
            let script = self.tactic_block()?;
            return Ok(Node::new(
                K::TacticBlock,
                Span::new(start, script.span.end),
                script.children,
            ));
        }
        let keyword = self.word().to_owned();
        let keyword_node = Node::leaf(K::Name, self.token().span);
        match keyword.as_str() {
            "intro" => {
                self.bump();
                let names = self.tactic_names()?;
                let mut children = vec![keyword_node];
                children.extend(names);
                Ok(self.node(K::Tactic, start, children))
            }
            "intros" | "constructor" | "left" | "right" | "assumption" | "contradiction"
            | "exfalso" | "rfl" | "symm" | "decide" | "trivial" | "skip" | "done" | "sorry"
            | "ext" | "pushNeg" => {
                self.bump();
                Ok(self.node(K::Tactic, start, vec![keyword_node]))
            }
            "byContra" => {
                self.bump();
                let name = self.name()?;
                Ok(self.node(K::Tactic, start, vec![keyword_node, name]))
            }
            // Kestrel uses byContra. Keep the Lean spelling in the same shape
            // so the kernel can report a clean unknown-tactic error.
            "by_contra" => {
                self.bump();
                let mut children = vec![keyword_node];
                if !self.eof()
                    && !self.newline()
                    && !self.at(";")
                    && !self.at("}")
                    && let Ok(name) = self.name()
                {
                    children.push(name);
                }
                Ok(self.node(K::Tactic, start, children))
            }
            "exact" | "apply" | "exists" | "use" | "trans" => {
                self.bump();
                let saved = self.pos;
                let diagnostics = self.diagnostics.len();
                // An existential witness may be a set literal such as `{a}`.
                let sets = keyword == "exists" || keyword == "use";
                let previous = self.proposition;
                if sets {
                    self.proposition = true;
                }
                let value = self.expr(0, false);
                self.proposition = previous;
                match value {
                    Ok(value) => Ok(self.node(K::Tactic, start, vec![keyword_node, value])),
                    Err(()) => {
                        self.pos = saved;
                        self.diagnostics.truncate(diagnostics);
                        Ok(self.node(K::Tactic, start, vec![keyword_node]))
                    }
                }
            }
            "rw" | "rewrite" | "simp" => {
                self.bump();
                let mut rules = vec![];
                if self.at("[") {
                    self.bump();
                    while !self.eof() && !self.at("]") {
                        rules.push(self.rewrite_rule()?);
                        if !self.eat(",") {
                            break;
                        }
                        self.soft();
                    }
                    self.expect("]")?;
                }
                let mut children = vec![keyword_node];
                children.extend(rules);
                Ok(self.node(K::Tactic, start, children))
            }
            "have" => {
                self.bump();
                let name = self.name()?;
                if !self.eat(":") {
                    self.expect(":=")?;
                    let proof = self.expr(0, false)?;
                    return Ok(self.node(K::Tactic, start, vec![keyword_node, name, proof]));
                }
                let previous = self.proposition;
                self.proposition = true;
                let claim = self.expr(0, false);
                self.proposition = previous;
                let claim = claim?;
                let proposition = self.node(K::Proposition, claim.span.start, vec![claim]);
                let proof = if self.eat(":=") {
                    self.expr(0, false)?
                } else if self.at("by") {
                    self.tactic_proof()?
                } else {
                    self.err(
                        "E_TACTIC_ARGUMENT",
                        "A have needs a proof after `:=` or `by`.",
                        "Write have h := term, have h : P := term, or have h : P by { ... }.",
                    );
                    return Err(());
                };
                Ok(self.node(
                    K::Tactic,
                    start,
                    vec![keyword_node, name, proposition, proof],
                ))
            }
            "cases" | "induction" | "strongInduction" => {
                self.bump();
                let saved = self.pos;
                let diagnostics = self.diagnostics.len();
                let mut name = match self.name() {
                    Ok(name) => name,
                    Err(()) => {
                        self.pos = saved;
                        self.diagnostics.truncate(diagnostics);
                        return Ok(self.node(K::Tactic, start, vec![keyword_node]));
                    }
                };
                // `cases h.left` splits a conjunction or iff through a
                // projection instead of binding the projection first.
                while self.eat(".") {
                    let member_start = name.span.start;
                    let member = self.name()?;
                    name = self.node(K::Member, member_start, vec![name, member]);
                }
                let mut children = vec![keyword_node, name];
                if keyword == "induction" && self.eat("generalizing") {
                    let group_start = self.start();
                    let mut names = vec![];
                    loop {
                        names.push(self.name()?);
                        if !self.eat(",") {
                            break;
                        }
                        self.soft();
                    }
                    children.push(self.node(K::Group, group_start, names));
                }
                if self.at("{") {
                    children.push(self.tactic_branches()?);
                }
                Ok(self.node(K::Tactic, start, children))
            }
            "byCases" => {
                self.bump();
                let name = self.name()?;
                self.expect(":")?;
                let previous = self.proposition;
                self.proposition = false;
                let value = self.expr(0, false);
                self.proposition = previous;
                let value = value?;
                Ok(self.node(K::Tactic, start, vec![keyword_node, name, value]))
            }
            "calc" => {
                self.bump();
                let mut lines = vec![];
                loop {
                    let line_start = self.start();
                    let left = if self.at("_") {
                        let hole_start = self.start();
                        self.bump();
                        self.node(K::Hole, hole_start, vec![])
                    } else {
                        self.expr(0, false)?
                    };
                    self.expect("=")?;
                    let right = self.expr(0, false)?;
                    self.expect(":=")?;
                    let proof = self.expr(0, false)?;
                    lines.push(self.node(K::Group, line_start, vec![left, right, proof]));
                    self.separators();
                    if self.at("_") {
                        continue;
                    }
                    break;
                }
                let lines = self.node(K::Group, start, lines);
                Ok(self.node(K::Tactic, start, vec![keyword_node, lines]))
            }
            "repeat" | "try" => {
                self.bump();
                let block = self.tactic_block()?;
                Ok(self.node(K::Tactic, start, vec![keyword_node, block]))
            }
            _ => {
                // Keep the form so the kernel reports the unknown tactic. A
                // parser error would stop the checker before the proof state
                // can show the goals the author is still editing.
                self.bump();
                Ok(self.node(K::Tactic, start, vec![keyword_node]))
            }
        }
    }

    fn tactic_branches(&mut self) -> PResult {
        let start = self.start();
        self.expect("{")?;
        self.separators();
        let mut children = vec![];
        while !self.eof() && !self.at("}") {
            let arm_start = self.start();
            let pattern = self.pattern()?;
            let mut arm = vec![pattern];
            while !self.at("=>") && !self.eof() {
                arm.push(self.name()?);
            }
            self.expect("=>")?;
            self.soft();
            arm.push(self.tactic_block()?);
            children.push(self.node(K::TacticBranch, arm_start, arm));
            self.separators();
            if self.eat(",") {
                self.separators();
            }
        }
        if !self.eof() {
            self.expect("}")?;
        }
        Ok(self.node(K::TacticBlock, start, children))
    }

    /// `induction n { Zero => p, Succ(k) ih => q }`: like a match on `n`, where a
    /// name after the pattern receives the induction hypothesis.
    fn induction(&mut self) -> PResult {
        let start = self.start();
        self.bump();
        let mut children = vec![self.name()?];
        self.soft();
        self.expect("{")?;
        self.soft();
        while !self.eof() && !self.at("}") {
            let arm_start = self.start();
            let mut arm = vec![self.pattern()?];
            while !self.at("=>") && !self.eof() {
                arm.push(self.name()?);
            }
            self.expect("=>")?;
            arm.push(self.expr(0, true)?);
            children.push(self.node(K::MatchArm, arm_start, arm));
            self.soft();
            if self.eat(",") {
                self.soft();
            } else if !self.at("}") {
                self.err(
                    "E_MATCH_COMMA",
                    "Expected a comma after this induction case.",
                    "Separate the cases with commas.",
                );
                return Err(());
            }
        }
        self.expect("}")?;
        Ok(self.node(K::Induction, start, children))
    }

    fn match_expr(&mut self) -> PResult {
        let start = self.start();
        self.bump();
        let value = self.expr(0, false)?;
        let mut children = vec![value];
        self.soft();
        self.expect("{")?;
        self.soft();
        while !self.eof() && !self.at("}") {
            let before = self.pos;
            let arm_start = self.start();
            let arm = (|| -> PResult {
                let mut arm = vec![self.pattern()?];
                if self.eat("if") {
                    let start = self.tokens[self.pos - 1].span.start;
                    let arrow = self.ending_arrow(self.pos);
                    let saved_arrow = std::mem::replace(&mut self.guard_arrow, arrow);
                    let guard = self.expr(0, false);
                    self.guard_arrow = saved_arrow;
                    let guard = guard?;
                    arm.push(self.node(K::Guard, start, vec![guard]));
                }
                self.expect("=>")?;
                // `=> return value` is a block holding one return statement (Q122, approved
                // 2026-11-08); `=> break` and `=> continue` work the same way (Q146).
                if self.at("return") || self.at("break") || self.at("continue") {
                    let body_start = self.start();
                    let ret = self.statement()?;
                    arm.push(self.node(K::Block, body_start, vec![ret]));
                } else {
                    arm.push(self.expr(0, true)?);
                }
                Ok(self.node(K::MatchArm, arm_start, arm))
            })();
            match arm {
                Ok(arm) => children.push(arm),
                Err(()) => {
                    while !self.eof() && !self.at(",") && !self.at("}") && !self.newline() {
                        self.bump();
                    }
                    children.push(self.node(K::Error, arm_start, vec![]));
                }
            }
            self.soft();
            if self.eat(",") {
                self.soft();
            } else if !self.at("}") {
                self.err(
                    "E_MATCH_COMMA",
                    "Expected a comma after this match arm.",
                    "Separate match arms with commas.",
                );
            }
            if self.pos == before && !self.eof() && !self.at("}") {
                self.bump();
            }
        }
        self.expect("}")?;
        Ok(self.node(K::Match, start, children))
    }

    fn pattern(&mut self) -> PResult {
        let mut alternatives = vec![self.nested(|p| p.pattern_inner())?];
        // A binding pattern cannot be refuted, so it takes no alternatives.
        if !self.binding_pattern {
            while self.at("|") {
                self.bump();
                self.soft();
                alternatives.push(self.nested(|p| p.pattern_inner())?);
            }
        }
        let pattern = if alternatives.len() == 1 {
            alternatives.pop().unwrap()
        } else {
            let start = alternatives[0].span.start;
            Node::new(K::PatternAlt, Span::new(start, self.end()), alternatives)
        };
        for n in pattern
            .descendants()
            .filter(|n| n.kind == K::PatternName || n.kind == K::PatternRest)
        {
            if let Some(name) = n.child(K::Name) {
                self.value_names.insert(self.text[name.span.range()].into());
            }
        }
        Ok(pattern)
    }

    fn pattern_inner(&mut self) -> PResult {
        self.soft();
        let start = self.start();
        if self.eat("_") {
            return Ok(self.node(K::PatternWildcard, start, vec![]));
        }
        if self.eat("..") {
            let mut children = vec![];
            if self.token().kind == TokenKind::Word {
                children.push(self.name()?);
            }
            return Ok(self.node(K::PatternRest, start, children));
        }
        if self.at("(") || self.at("[") {
            let list = self.at("[");
            self.bump();
            let close = if list { "]" } else { ")" };
            let mut children = vec![];
            self.soft();
            while !self.eof() && !self.at(close) {
                children.push(self.value_pattern()?);
                self.soft();
                if !self.eat(",") {
                    break;
                }
                self.soft();
            }
            self.expect(close)?;
            return Ok(self.node(
                if list {
                    K::PatternList
                } else {
                    K::PatternTuple
                },
                start,
                children,
            ));
        }
        if matches!(
            self.token().kind,
            TokenKind::Integer | TokenKind::Double | TokenKind::String
        ) || matches!(self.word(), "true" | "false")
        {
            let integer = self.token().kind == TokenKind::Integer;
            let literal = Node::leaf(K::PatternLiteral, self.bump().span);
            if integer && !self.binding_pattern && self.at("..") {
                return self.range_pattern(start, literal);
            }
            return Ok(literal);
        }
        if self.eat("-") {
            if !matches!(self.token().kind, TokenKind::Integer | TokenKind::Double) {
                self.err(
                    "E_PATTERN",
                    "Expected a number after `-` in a pattern.",
                    "Use a literal number here.",
                );
                return Err(());
            }
            let integer = self.token().kind == TokenKind::Integer;
            self.bump();
            let literal = self.node(K::PatternLiteral, start, vec![]);
            if integer && !self.binding_pattern && self.at("..") {
                return self.range_pattern(start, literal);
            }
            return Ok(literal);
        }
        let name = self.path()?;
        let constructor = self.text[name.span.range()].contains('.')
            || self.case_names.contains(&self.text[name.span.range()])
            || !self.binding_pattern
                && self.text[name.span.range()]
                    .chars()
                    .next()
                    .is_some_and(crate::identifiers::type_start);
        let mut children = vec![name];
        if self.eat("(") {
            self.soft();
            let mut named = false;
            while !self.eof() && !self.at(")") {
                let payload_start = self.start();
                let is_named = self.token().kind == TokenKind::Word
                    && self
                        .tokens
                        .get(self.pos + 1)
                        .is_some_and(|t| &self.text[t.span.range()] == "=");
                if is_named {
                    named = true;
                    let label = self.name()?;
                    self.expect("=")?;
                    let value = self.value_pattern()?;
                    children.push(self.node(K::NamedPattern, payload_start, vec![label, value]));
                } else {
                    if named {
                        self.err(
                            "E_ARGUMENT_ORDER",
                            "A positional payload follows a named one.",
                            "Put positional payloads first, or name this payload.",
                        );
                    }
                    children.push(self.value_pattern()?);
                }
                self.soft();
                if !self.eat(",") {
                    break;
                }
                self.soft();
            }
            self.expect(")")?;
            Ok(self.node(K::PatternConstructor, start, children))
        } else {
            Ok(self.node(
                if constructor {
                    K::PatternConstructor
                } else {
                    K::PatternName
                },
                start,
                children,
            ))
        }
    }

    /// `0..10` and `0..=9` after a scalar integer pattern.
    fn range_pattern(&mut self, start: usize, lower: Node) -> PResult {
        self.bump();
        let kind = if self.eat("=") {
            K::PatternRangeInclusive
        } else {
            K::PatternRange
        };
        let bound = self.start();
        if self.eat("-") {
            if self.token().kind != TokenKind::Integer {
                self.err(
                    "E_PATTERN_RANGE",
                    "Expected an integer after `-` in a range.",
                    "Use a whole number such as -5.",
                );
                return Err(());
            }
            self.bump();
            let upper = self.node(K::PatternLiteral, bound, vec![]);
            return Ok(self.node(kind, start, vec![lower, upper]));
        }
        if self.token().kind != TokenKind::Integer {
            self.err(
                "E_PATTERN_RANGE",
                "Expected an integer bound after `..`.",
                "Write a range as `0..10` or `0..=9`.",
            );
            return Err(());
        }
        let upper = Node::leaf(K::PatternLiteral, self.bump().span);
        Ok(self.node(kind, start, vec![lower, upper]))
    }

    fn theorem(&mut self) -> PResult {
        let saved_values = self.value_names.clone();
        let result = self.theorem_inner();
        self.value_names = saved_values;
        result
    }

    /// `infixl 10 "⊕" = add` maps a source operator directly to a function.
    /// The declaration is syntax, not a proof: uses become ordinary calls.
    fn operator_declaration(&mut self) -> PResult {
        let start = self.start();
        let keyword = self.word().to_owned();
        self.bump();
        self.soft();
        let precedence_start = self.start();
        let precedence = self.word().parse::<u8>().ok();
        let Some(precedence) = precedence.filter(|bp| (1..=13).contains(bp)) else {
            self.err(
                "E_OPERATOR_PRECEDENCE",
                "An operator precedence is a whole number from 1 to 13.",
                "1 binds loosest and 13 binds tightest.",
            );
            return Err(());
        };
        self.bump();
        self.soft();
        if self.token().kind != TokenKind::String {
            self.err(
                "E_OPERATOR_SYMBOL",
                "Expected a quoted operator symbol.",
                "Write the spelling in double quotes, as in \"⊕\" or \"plus\".",
            );
            return Err(());
        }
        let literal = self.token();
        let raw = &self.text[literal.span.range()];
        let symbol = raw
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or("")
            .to_owned();
        if symbol.is_empty() {
            self.err(
                "E_OPERATOR_SYMBOL",
                "An operator needs a non-empty symbol.",
                "Write one symbol inside the quotes.",
            );
            return Err(());
        }
        if infix(&symbol, true).is_some() || symbol == "in" {
            self.err(
                "E_OPERATOR_SYMBOL",
                format!("`{symbol}` already has a meaning in Kestrel."),
                "Choose a spelling that is not a built-in operator.",
            );
            return Err(());
        }
        self.bump();
        self.soft();
        let mut alias = None;
        if self.eat("as") {
            self.soft();
            let name = self.word().to_owned();
            if !name.starts_with('\\') || name.len() < 2 || self.token().kind != TokenKind::Symbol {
                self.err(
                    "E_OPERATOR_SYMBOL",
                    "Expected a backslash alias after `as`.",
                    "Write `as \\oplus`.",
                );
                return Err(());
            }
            self.bump();
            self.soft();
            alias = Some(name);
        }
        self.expect("=")?;
        self.soft();
        let function = self.path()?;
        let (left_bp, right_bp) = match keyword.as_str() {
            "infixr" => (precedence + 1, precedence),
            _ => (precedence, precedence + 1),
        };
        let operator = CustomOperator {
            left_bp,
            right_bp,
            function: function.span,
        };
        self.custom_operators.insert(symbol.clone(), operator);
        if let Some(alias) = alias {
            if infix(&alias, true).is_some()
                || matches!(
                    alias.as_str(),
                    "\\forall"
                        | "\\exists"
                        | "\\in"
                        | "\\subseteq"
                        | "\\to"
                        | "\\land"
                        | "\\lor"
                        | "\\not"
                        | "\\neg"
                        | "\\ge"
                        | "\\le"
                        | "\\ne"
                        | "\\union0"
                        | "\\inter0"
                        | "\\emptyset"
                        | "\\empty"
                )
                || self.custom_operators.contains_key(&alias)
            {
                self.err(
                    "E_OPERATOR_SYMBOL",
                    format!("`{alias}` already has a meaning in Kestrel."),
                    "Choose a different backslash alias.",
                );
                return Err(());
            }
            self.custom_operators.insert(alias, operator);
        }
        let _ = precedence_start;
        Ok(self.node(
            K::OperatorDeclaration,
            start,
            vec![Node::leaf(K::Name, literal.span), function],
        ))
    }

    /// Theorem parameters may be values (`n : Nat`) or hypotheses
    /// (`h : x ∈ A`). A parameter is a proposition when its text contains a
    /// logical operator before the closing comma or parenthesis.
    fn theorem_parameters(&mut self) -> PResult {
        let start = self.start();
        self.expect("(")?;
        self.soft();
        let mut params = vec![];
        while !self.eof() && !self.at(")") {
            let param_start = self.start();
            let mut modifiers = vec![];
            let proof = self.proof_modifier();
            if proof {
                modifiers.push(Node::leaf(K::Modifier, self.bump().span));
            }
            let mut names = vec![self.name()?];
            self.value_names
                .insert(self.text[names[0].span.range()].into());
            while self.token().kind == TokenKind::Word && !reserved(self.word()) {
                let name = self.name()?;
                self.value_names.insert(self.text[name.span.range()].into());
                names.push(name);
            }
            let mut annotation = None;
            if self.eat(":") {
                if proof || self.proposition_ahead() {
                    let previous = self.proposition;
                    self.proposition = true;
                    let claim = self.expr(0, false);
                    self.proposition = previous;
                    let claim = claim?;
                    annotation = Some(self.node(K::Proposition, claim.span.start, vec![claim]));
                } else {
                    annotation = Some(self.ty()?);
                }
            } else if names.len() > 1 {
                self.err(
                    "E_PARAMETER_TYPE",
                    "Names grouped before a type need one shared annotation.",
                    "Write `(a b : Nat)`, or separate the parameters with commas.",
                );
                return Err(());
            }
            for name in names.iter() {
                let mut children = modifiers.clone();
                children.push(name.clone());
                if let Some(found) = &annotation {
                    children.push(found.clone());
                }
                let end = annotation
                    .as_ref()
                    .map(|node| node.span.end)
                    .unwrap_or(name.span.end);
                let param_start = if names.len() == 1 {
                    param_start
                } else {
                    name.span.start
                };
                params.push(Node::new(
                    K::Parameter,
                    Span::new(param_start, end),
                    children,
                ));
            }
            self.soft();
            if !self.eat(",") {
                break;
            }
            self.soft();
        }
        self.expect(")")?;
        Ok(self.node(K::ParameterList, start, params))
    }

    /// Looks ahead to the end of the current parameter for a propositional
    /// operator. Types never contain these at depth zero.
    fn proposition_ahead(&self) -> bool {
        let mut depth = 0_usize;
        for token in self.tokens.iter().skip(self.pos) {
            let word = &self.text[token.span.range()];
            match word {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                "," if depth == 0 => return false,
                "∈" | "∉" | "⊆" | "\\in" | "\\notin" | "\\subseteq" | "=" | "<" | "<=" | ">"
                | ">=" | "==" | "!=" | "&&" | "||" | "and" | "or" | "not" | "forall" | "exists"
                | "∀" | "∃" | "→" | "↔" | "∧" | "∨" | "¬" | "∪" | "∩" | "ᶜ" | "\\forall"
                | "\\exists" | "\\to" | "\\iff" | "\\land" | "\\lor" | "\\not" | "\\neg"
                | "\\cup" | "\\cap" | "\\compl" | "≤" | "\\le" | "≥" | "\\ge" | "≠" | "\\ne"
                | "⋃₀" | "⋂₀" | "\\union0" | "\\inter0" | "∅" | "\\emptyset" | "\\empty"
                | "true" | "false"
                    if depth == 0 =>
                {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    fn theorem_inner(&mut self) -> PResult {
        self.theorem_like(K::Theorem, true)
    }

    fn example(&mut self) -> PResult {
        self.theorem_like(K::Example, false)
    }

    fn axiom(&mut self) -> PResult {
        self.theorem_like(K::Axiom, true)
    }

    /// Parses `theorem`, `example` and `axiom` headers. An axiom stops after
    /// its statement; the other two require a proof.
    fn theorem_like(&mut self, kind: K, named: bool) -> PResult {
        let start = self.start();
        self.bump();
        let mut children = vec![];
        if named {
            children.push(self.name()?);
        }
        if self.at("[") {
            children.push(self.type_parameters()?);
        }
        children.push(self.theorem_parameters()?);
        self.expect(":")?;
        let previous = self.proposition;
        self.proposition = true;
        let proposition = self.expr(0, false);
        self.proposition = previous;
        let proposition = proposition?;
        children.push(self.node(K::Proposition, proposition.span.start, vec![proposition]));
        while self.eat("requires") {
            let start = self.tokens[self.pos - 1].span.start;
            let name = self.name()?;
            self.expect(":")?;
            self.proposition = true;
            let claim = self.expr(0, false);
            self.proposition = previous;
            children.push(self.node(K::Requires, start, vec![name, claim?]));
            self.soft();
        }
        if kind == K::Axiom {
            return Ok(self.node(kind, start, children));
        }
        self.soft();
        let proof_start = self.start();
        let body = if self.at("by") {
            self.tactic_proof()
        } else {
            let proving = std::mem::replace(&mut self.proving, true);
            let body = self.block();
            self.proving = proving;
            body
        };
        let body = body?;
        children.push(self.node(K::Proof, proof_start, vec![body]));
        Ok(self.node(kind, start, children))
    }

    fn format_string(&mut self) -> PResult {
        let token = self.bump();
        let end = token.span.end;
        let content_end = if self.text.as_bytes().get(end.wrapping_sub(1)) == Some(&b'"') {
            end - 1
        } else {
            end
        };
        let mut i = token.span.start + 2;
        let mut chunk = i;
        let mut children = vec![];
        let bytes = self.text.as_bytes();
        while i < content_end {
            if bytes[i] == b'\\' {
                i += 1;
                if i < content_end {
                    i += self.text[i..content_end].chars().next().unwrap().len_utf8();
                }
                continue;
            }
            if (bytes[i] == b'{' || bytes[i] == b'}')
                && i + 1 < content_end
                && bytes[i] == bytes[i + 1]
            {
                i += 2;
                continue;
            }
            if bytes[i] == b'}' {
                self.diagnostics.push(Diagnostic::new(
                    "E_FORMAT_BRACE",
                    "A literal closing brace in a formatted string needs `}}`.",
                    Span::new(i, i + 1),
                ));
                i += 1;
                continue;
            }
            if bytes[i] != b'{' {
                i += self.text[i..content_end].chars().next().unwrap().len_utf8();
                continue;
            }
            if chunk < i {
                children.push(Node::leaf(K::FormatText, Span::new(chunk, i)));
            }
            let Some(close) = lexer::interpolation_end(self.text, i + 1, content_end, self.depth)
            else {
                self.diagnostics.push(Diagnostic::new(
                    "E_FORMAT_BRACE",
                    "This interpolation is missing its closing brace.",
                    Span::new(i, content_end),
                ));
                break;
            };
            let lexed = lexer::lex_range(self.text, i + 1, close);
            let mut inner = Parser::new(self.text, &lexed.tokens, self.depth + 1);
            inner.value_names = self.value_names.clone();
            inner.case_names = self.case_names.clone();
            let expression = inner.expr(0, true);
            let mut interpolation = vec![];
            if let Ok(expression) = expression {
                interpolation.push(expression);
            }
            // `{x:=+8}` lexes `:=` as one token; its `=` is the alignment.
            let walrus = inner.at(":=");
            if walrus || inner.eat(":") {
                let spec_start = if walrus {
                    let start = inner.start() + 1;
                    inner.bump();
                    start
                } else {
                    // Right after the colon: a leading space is the sign option, as in Python.
                    inner.tokens[inner.pos - 1].span.end
                };
                while !inner.eof() {
                    inner.bump();
                }
                let spec = &self.text[spec_start..close];
                // `?`, or Python's format mini-language such as `>8,.2f` (REVIEW2 section 4).
                if crate::strings::format_kind(spec).is_none() {
                    inner.diagnostics.push(Diagnostic::new(
                        "E_FORMAT_SPEC",
                        "Expected `?` or a format such as `.2f`, `>8`, `,d` or `08.3f`.",
                        Span::new(spec_start, close),
                    ));
                }
                interpolation.push(Node::leaf(K::FormatSpec, Span::new(spec_start, close)));
            } else if !inner.eof() {
                inner.err(
                    "E_FORMAT_EXPRESSION",
                    "Unexpected text after this interpolation expression.",
                    "Put one expression inside each pair of braces.",
                );
            }
            self.diagnostics
                .extend(lexed.diagnostics.into_iter().filter(|d| {
                    !interpolation
                        .iter()
                        .any(|n| n.kind == K::FormatSpec && d.span.start >= n.span.start)
                }));
            self.diagnostics.extend(inner.diagnostics);
            children.push(Node::new(
                K::Interpolation,
                Span::new(i, close + 1),
                interpolation,
            ));
            i = close + 1;
            chunk = i;
        }
        if chunk < content_end {
            children.push(Node::leaf(K::FormatText, Span::new(chunk, content_end)));
        }
        Ok(Node::new(K::FormatString, token.span, children))
    }
}

/// An unparenthesized `a ** b`.
fn power(node: &Node, text: &str) -> bool {
    node.kind == K::Binary
        && node
            .children
            .get(1)
            .is_some_and(|op| &text[op.span.range()] == "**")
}

/// Bound values index with brackets; other uppercase heads take type arguments.
fn type_head(node: &Node, text: &str, values: &std::collections::BTreeSet<String>) -> bool {
    let name = match node.kind {
        K::Name => node,
        K::Member => match node.children.last() {
            // `.T` is the transpose property, not a generic type application.
            Some(name) if &text[name.span.range()] == "T" => return false,
            Some(name) => name,
            None => return false,
        },
        K::BracketApplication => return true,
        _ => return false,
    };
    if node.kind == K::Name && values.contains(&text[name.span.range()]) {
        return false;
    }
    text[name.span.range()]
        .chars()
        .next()
        .is_some_and(crate::identifiers::type_start)
}

/// A place that can be read twice without running code again: a name, a field, or an Array slot
/// whose index has no calls (Q117).
fn pure_place(node: &Node) -> bool {
    match node.kind {
        K::Name | K::Literal | K::One | K::Operator => true,
        K::Member | K::Index | K::Binary | K::Group => node.children.iter().all(pure_place),
        _ => false,
    }
}

/// The block after `if h : cond` or `while h : cond` starts with `let h := <proof of cond>` (Q115).
/// Only the parser makes the witness, so a program cannot write one.
fn proof_binding(name: Span, condition: &Node, mut block: Node) -> Node {
    let witness = Node::new(K::ProofWitness, condition.span, vec![condition.clone()]);
    let span = Span::new(name.start, condition.span.end);
    let pattern = Node::new(K::PatternName, name, vec![Node::leaf(K::Name, name)]);
    let binding = Node::new(K::Binding, span, vec![pattern, witness]);
    block.children.insert(0, binding);
    block
}

fn infix(op: &str, proposition: bool) -> Option<(u8, u8)> {
    if matches!(op, "->" | "→" | "\\to") && proposition {
        return Some((1, 1));
    }
    if matches!(op, "↔" | "\\iff") && proposition {
        return Some((0, 1));
    }
    let bp = match op {
        "|>" => 1,
        "||" => 2,
        "&&" => 3,
        "|" => 4,
        "^" => 5,
        "&" => 6,
        "==" | "!=" => 7,
        "<" | "<=" | ">" | ">=" | "≤" | "\\le" | "≥" | "\\ge" => 8,
        "≠" | "\\ne" => 7,
        "<<" | ">>" => 9,
        "+" | "-" => 10,
        "*" | "/" | "%" | "@" => 11,
        // Above prefix operators (12), so `-2 ** 2` groups as -(2 ** 2) and is then refused.
        "**" => 13,
        "=" if proposition => 7,
        "or" | "∨" | "\\lor" if proposition => 2,
        "and" | "∧" | "\\land" if proposition => 3,
        // `∩` binds tighter than `∪`. Both bind tighter than `⊆` and `∈`.
        "∪" | "\\cup" => 8,
        "∩" | "\\cap" => 9,
        "∈" | "\\in" | "∉" | "\\notin" | "⊆" | "\\subseteq" if proposition => 7,
        _ => return None,
    };
    Some((bp, bp + 1))
}

fn declaration_start(word: &str) -> bool {
    matches!(
        word,
        "function"
            | "class"
            | "trait"
            | "enum"
            | "alias"
            | "infixl"
            | "infixr"
            | "infix"
            | "type"
            | "const"
            | "import"
            | "theorem"
            | "example"
            | "axiom"
            | "@"
            | "public"
            | "private"
            | "package"
    )
}

fn documented(kind: K) -> bool {
    matches!(
        kind,
        K::Function
            | K::Constructor
            | K::Class
            | K::Trait
            | K::Enum
            | K::Alias
            | K::Const
            | K::OperatorDeclaration
            | K::Theorem
            | K::Axiom
            | K::Example
    )
}

/// Whether `word` is a keyword that cannot name a binding or a package.
pub(crate) fn reserved(word: &str) -> bool {
    matches!(
        word,
        "function"
            | "class"
            | "trait"
            | "enum"
            | "alias"
            | "const"
            | "import"
            | "theorem"
            | "example"
            | "axiom"
            | "let"
            | "mut"
            | "move"
            | "new"
            | "has"
            | "extends"
            | "final"
            | "var"
            | "public"
            | "private"
            | "package"
            | "static"
            | "sealed"
            | "if"
            | "then"
            | "else"
            | "for"
            | "in"
            | "while"
            | "return"
            | "break"
            | "continue"
            | "match"
            | "requires"
            | "ensures"
            | "invariant"
            | "with"
            | "or"
            | "and"
            | "not"
            | "otherwise"
            | "unsafe"
            | "using"
            | "scope"
            | "spawn"
            | "dyn"
            | "depends"
            | "as"
    )
}

fn case_names(text: &str, tokens: &[Token]) -> std::collections::BTreeSet<String> {
    let mut names = ["Some", "None", "Zero", "Succ"]
        .into_iter()
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    let mut pending = false;
    let mut depth = 0_usize;
    let mut nested = 0_usize;
    let mut next_case = false;
    for token in tokens.iter().filter(|t| !t.is_trivia()) {
        let word = &text[token.span.range()];
        if depth == 0 {
            if word == "enum" {
                pending = true;
            } else if pending && word == "{" {
                depth = 1;
                pending = false;
                next_case = true;
            }
            continue;
        }
        match word {
            "{" => depth += 1,
            "}" => depth -= 1,
            "(" | "[" => nested += 1,
            ")" | "]" => nested = nested.saturating_sub(1),
            "," | ";" if depth == 1 && nested == 0 => next_case = true,
            _ if token.kind == TokenKind::Newline && depth == 1 && nested == 0 => next_case = true,
            _ if next_case && depth == 1 && nested == 0 && token.kind == TokenKind::Word => {
                names.insert(word.into());
                next_case = false;
            }
            _ => {}
        }
    }
    names
}
