mod actions;
pub mod checker;
pub mod contracts;
pub mod core;
pub mod diagnostic;
pub mod doctor;
pub mod formatter;
pub mod game;
mod identifiers;
pub mod interop;
pub mod lexer;
pub mod modules;
pub mod native;
mod numerical;
pub mod ownership;
mod package_lock;
pub mod parser;
pub mod paths;
mod probability;
pub mod proof;
pub mod resolution;
pub mod scaffold;
pub mod server;
pub mod session;
pub mod source;
pub mod strings;
pub mod syntax;
pub mod testing;
pub mod types;

use diagnostic::Diagnostic;
use lexer::Token;
use source::SourceFile;
use syntax::Node;

#[derive(Debug)]
pub struct Parsed {
    pub source: SourceFile,
    pub tokens: Vec<Token>,
    pub root: Node,
    pub diagnostics: Vec<Diagnostic>,
}

impl Parsed {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn reconstructed_source(&self) -> String {
        self.tokens
            .iter()
            .map(|token| self.source.slice(token.span))
            .collect()
    }
}

pub fn parse_file(name: impl Into<String>, text: impl Into<String>) -> Parsed {
    parse(name, text, false)
}

pub fn parse_project(name: impl Into<String>, text: impl Into<String>) -> Parsed {
    parse(name, text, true)
}

pub(crate) fn parse_file_with_values(
    name: impl Into<String>,
    text: impl Into<String>,
    values: &std::collections::BTreeSet<String>,
) -> Parsed {
    parse_with_values(name, text, false, values)
}

fn parse(name: impl Into<String>, text: impl Into<String>, project: bool) -> Parsed {
    parse_with_values(name, text, project, &Default::default())
}

fn parse_with_values(
    name: impl Into<String>,
    text: impl Into<String>,
    project: bool,
    values: &std::collections::BTreeSet<String>,
) -> Parsed {
    let source = SourceFile::new(name, text);
    let lexed = lexer::lex(&source.text);
    let (root, mut diagnostics) =
        parser::parse_tokens_with_values(&source.text, &lexed.tokens, project, values);
    diagnostics.splice(0..0, lexed.diagnostics);
    Parsed {
        source,
        tokens: lexed.tokens,
        root,
        diagnostics,
    }
}
