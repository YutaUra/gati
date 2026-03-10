use std::path::Path;

use ratatui::style::{Color, Style};
use ratatui::text::Span;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

// Re-export for public API
pub use syntect::parsing::SyntaxReference;

/// Shared syntax highlighting resources.
pub struct Highlighter {
    pub syntax_set: SyntaxSet,
    pub theme: Theme,
}

impl Highlighter {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        // base16-eighties.dark works well on dark terminals
        let theme = theme_set.themes["base16-eighties.dark"].clone();
        Self { syntax_set, theme }
    }

    /// Detect syntax for a file path, falling back to first-line detection, then plain text.
    pub fn detect_syntax(&self, path: &Path, first_line: &str) -> &SyntaxReference {
        // Try extension-based detection
        if let Some(syntax) = self
            .syntax_set
            .find_syntax_for_file(path)
            .ok()
            .flatten()
        {
            return syntax;
        }

        // Try first-line detection (shebang, modeline)
        if let Some(syntax) = self.syntax_set.find_syntax_by_first_line(first_line) {
            return syntax;
        }

        // Fall back to plain text
        self.syntax_set.find_syntax_plain_text()
    }

    /// Create a `HighlightLines` state for incremental line-by-line highlighting.
    pub fn new_highlight_state(&self, syntax: &SyntaxReference) -> syntect::easy::HighlightLines<'_> {
        syntect::easy::HighlightLines::new(syntax, &self.theme)
    }

    /// Highlight a single line of code, returning styled ratatui Spans.
    pub fn highlight_line(
        &self,
        state: &mut syntect::easy::HighlightLines<'_>,
        line: &str,
    ) -> Vec<Span<'static>> {
        let regions = state
            .highlight_line(line, &self.syntax_set)
            .unwrap_or_default();

        regions
            .into_iter()
            .map(|(style, text)| {
                let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                Span::styled(text.to_string(), Style::default().fg(fg))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_highlighter_with_defaults() {
        let h = Highlighter::new();
        // Should have loaded syntax definitions
        assert!(h.syntax_set.syntaxes().len() > 10);
    }

    // 2.1: Extension-based detection
    #[test]
    fn detect_syntax_recognizes_rust_by_extension() {
        let h = Highlighter::new();
        let syntax = h.detect_syntax(Path::new("main.rs"), "");
        assert_eq!(syntax.name, "Rust");
    }

    #[test]
    fn detect_syntax_recognizes_python_by_extension() {
        let h = Highlighter::new();
        let syntax = h.detect_syntax(Path::new("script.py"), "");
        assert_eq!(syntax.name, "Python");
    }

    #[test]
    fn detect_syntax_recognizes_javascript_by_extension() {
        let h = Highlighter::new();
        let syntax = h.detect_syntax(Path::new("app.js"), "");
        assert_eq!(syntax.name, "JavaScript");
    }

    // 2.2: First-line fallback (shebang)
    #[test]
    fn detect_syntax_recognizes_bash_shebang() {
        let h = Highlighter::new();
        let syntax = h.detect_syntax(Path::new("run_script"), "#!/bin/bash");
        assert_eq!(syntax.name, "Bourne Again Shell (bash)");
    }

    #[test]
    fn detect_syntax_recognizes_python_shebang() {
        let h = Highlighter::new();
        let syntax = h.detect_syntax(Path::new("script"), "#!/usr/bin/env python3");
        assert_eq!(syntax.name, "Python");
    }

    // 2.3: Plain text fallback
    #[test]
    fn detect_syntax_falls_back_to_plain_text() {
        let h = Highlighter::new();
        let syntax = h.detect_syntax(Path::new("unknown_file.xyz123"), "some random content");
        assert_eq!(syntax.name, "Plain Text");
    }

    // 3.1 + 3.2: Highlight line produces styled spans
    #[test]
    fn highlight_line_produces_spans_for_rust_code() {
        let h = Highlighter::new();
        let syntax = h.detect_syntax(Path::new("test.rs"), "");
        let mut state = h.new_highlight_state(syntax);

        let spans = h.highlight_line(&mut state, "fn main() {}\n");
        assert!(!spans.is_empty());
        // Each span should have an RGB color
        for span in &spans {
            match span.style.fg {
                Some(Color::Rgb(_, _, _)) => {}
                _ => panic!("Expected Rgb color, got {:?}", span.style.fg),
            }
        }
    }

    #[test]
    fn highlight_line_plain_text_produces_spans() {
        let h = Highlighter::new();
        let syntax = h.syntax_set.find_syntax_plain_text();
        let mut state = h.new_highlight_state(syntax);

        let spans = h.highlight_line(&mut state, "just plain text\n");
        assert!(!spans.is_empty());
    }
}
