use ratatui::text::Span;
use syntect::highlighting::HighlightState;
use syntect::parsing::ParseState;

use crate::highlight::Highlighter;

/// Cached syntax highlighting state for incremental highlighting.
pub(crate) struct HighlightCache {
    /// Incrementally-computed highlighted spans for file lines.
    /// May be shorter than total lines; remaining lines are computed on demand
    /// during render via `ensure_up_to`.
    pub highlighted_lines: Vec<Vec<Span<'static>>>,
    /// Saved syntect parse state after the last highlighted line, enabling
    /// incremental highlighting without replaying from the beginning.
    hl_parse_state: Option<ParseState>,
    /// Saved syntect highlight state after the last highlighted line.
    hl_highlight_state: Option<HighlightState>,
}

impl HighlightCache {
    pub fn new() -> Self {
        Self {
            highlighted_lines: Vec::new(),
            hl_parse_state: None,
            hl_highlight_state: None,
        }
    }

    pub fn clear(&mut self) {
        self.highlighted_lines.clear();
        self.hl_parse_state = None;
        self.hl_highlight_state = None;
    }

    /// Incrementally compute syntax highlighting up to (exclusive) the given line index.
    ///
    /// Syntect is stateful -- each line's highlighting depends on all preceding
    /// lines. We cache the `ParseState` and `HighlightState` after the last
    /// highlighted line so that subsequent calls resume in O(new_lines) rather
    /// than replaying from line 0.
    pub fn ensure_up_to(
        &mut self,
        up_to: usize,
        lines: &[String],
        syntax_name: &str,
        highlighter: &Highlighter,
    ) {
        let already = self.highlighted_lines.len();
        let target = up_to.min(lines.len());
        if already >= target {
            return;
        }

        let syntax_ref = highlighter
            .syntax_set
            .find_syntax_by_name(syntax_name)
            .unwrap_or_else(|| highlighter.syntax_set.find_syntax_plain_text());

        // Restore saved state or create fresh state for the first call.
        let mut hl_lines = match (self.hl_highlight_state.take(), self.hl_parse_state.take()) {
            (Some(hs), Some(ps)) => {
                syntect::easy::HighlightLines::from_state(&highlighter.theme, hs, ps)
            }
            _ => highlighter.new_highlight_state(syntax_ref),
        };

        // Highlight only the new lines.
        self.highlighted_lines.reserve(target - already);
        for line in &lines[already..target] {
            let spans = highlighter.highlight_line(&mut hl_lines, &format!("{line}\n"));
            self.highlighted_lines.push(spans);
        }

        // Save state for the next incremental call via `state(self)`.
        let (hs, ps) = hl_lines.state();
        self.hl_highlight_state = Some(hs);
        self.hl_parse_state = Some(ps);
    }
}
