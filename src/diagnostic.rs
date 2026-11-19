use crate::{
    actions::Fix,
    source::{SourceFile, Span},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub hint: Option<String>,
    pub severity: Severity,
    /// Editor quick fixes derived from this diagnostic.
    pub(crate) fixes: Vec<Fix>,
}

impl Diagnostic {
    pub fn new(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            message: message.into(),
            span,
            hint: None,
            severity: Severity::Error,
            fixes: vec![],
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::new(code, message, span)
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Attaches the editor actions that repair this gap.
    pub(crate) fn with_fixes(mut self, fixes: Vec<Fix>) -> Self {
        self.fixes = fixes;
        self
    }

    pub fn render(&self, source: &SourceFile) -> String {
        let (line, column) = source.position(self.span.start);
        let shown = source.line(line);
        let width = source
            .text
            .get(self.span.range())
            .unwrap_or("")
            .split('\n')
            .next()
            .unwrap_or("")
            .chars()
            .count()
            .clamp(1, 80);
        let mut result = format!(
            "{}:{line}:{column}: {}[{}]: {}\n  {line} | {shown}\n    | {}{}\n",
            source.name,
            match self.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            },
            self.code,
            self.message,
            " ".repeat(column - 1),
            "^".repeat(width),
        );
        if let Some(hint) = &self.hint {
            result.push_str(&format!("  hint: {hint}\n"));
        }
        result
    }
}
