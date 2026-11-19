use std::ops::Range;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn range(self) -> Range<usize> {
        self.start..self.end
    }
}

#[derive(Clone, Debug)]
pub struct SourceFile {
    pub name: String,
    pub text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let mut line_starts = vec![0];
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\r' {
                i += 1;
                if bytes.get(i) == Some(&b'\n') {
                    i += 1;
                }
                line_starts.push(i);
            } else if bytes[i] == b'\n' {
                i += 1;
                line_starts.push(i);
            } else {
                i += 1;
            }
        }
        Self {
            name: name.into(),
            text,
            line_starts,
        }
    }

    pub fn slice(&self, span: Span) -> &str {
        &self.text[span.range()]
    }

    /// One-based line and character column; spans themselves use UTF-8 bytes.
    pub fn position(&self, offset: usize) -> (usize, usize) {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.line_starts.partition_point(|start| *start <= offset) - 1;
        let column = self.text[self.line_starts[line]..offset].chars().count();
        (line + 1, column + 1)
    }

    /// The byte offset of a one-based line and character column. A column past
    /// the end of its line stops at the line break.
    pub fn offset(&self, line: usize, column: usize) -> usize {
        let Some(&start) = self.line_starts.get(line.saturating_sub(1)) else {
            return self.text.len();
        };
        let mut offset = start;
        for ch in self.text[start..].chars().take(column.saturating_sub(1)) {
            if ch == '\n' || ch == '\r' {
                break;
            }
            offset += ch.len_utf8();
        }
        offset.min(self.text.len())
    }

    pub fn line(&self, line: usize) -> &str {
        let Some(&start) = self.line_starts.get(line.saturating_sub(1)) else {
            return "";
        };
        let end = self
            .line_starts
            .get(line)
            .copied()
            .unwrap_or(self.text.len());
        self.text[start..end].trim_end_matches(['\r', '\n'])
    }
}
