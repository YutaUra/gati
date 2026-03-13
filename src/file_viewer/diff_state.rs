use std::path::Path;

use ratatui::text::Span;

use crate::diff::{LineDiff, UnifiedDiff, UnifiedDiffLine};
use crate::highlight::Highlighter;

/// Diff-related state grouped together for organization.
pub struct DiffState {
    /// Per-line diff info for gutter markers in normal mode.
    pub line_diff: Option<LineDiff>,
    /// Parsed unified diff for diff mode.
    pub unified_diff: Option<UnifiedDiff>,
    /// Pre-computed syntax-highlighted spans for each unified diff line.
    pub(super) diff_highlighted_lines: Vec<Vec<Span<'static>>>,
    /// Whether the viewer is currently in diff mode.
    pub diff_mode: bool,
    /// Mapping from diff display index to file line number (1-indexed).
    /// `None` for Removed lines (not in current file), `Some(n)` for Context/Added.
    pub(super) diff_line_numbers: Vec<Option<usize>>,
}

impl DiffState {
    pub fn new() -> Self {
        Self {
            line_diff: None,
            unified_diff: None,
            diff_highlighted_lines: Vec::new(),
            diff_mode: false,
            diff_line_numbers: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.line_diff = None;
        self.unified_diff = None;
        self.diff_highlighted_lines.clear();
        self.diff_mode = false;
        self.diff_line_numbers.clear();
    }

    /// Set diff data for the currently loaded file.
    ///
    /// Syntax highlighting for diff lines is deferred until diff mode is
    /// actually rendered (see `ensure_highlighted`), avoiding expensive
    /// O(n) syntect work on every file selection.
    pub fn set(&mut self, line_diff: Option<LineDiff>, unified_diff: Option<UnifiedDiff>) {
        self.line_diff = line_diff;
        self.diff_highlighted_lines.clear();
        self.unified_diff = unified_diff;
        self.compute_line_numbers();
    }

    /// Lazily compute syntax-highlighted spans for unified diff lines.
    ///
    /// Called on the first diff-mode render after `set`. Subsequent
    /// calls are no-ops while the cached highlights remain valid.
    pub fn ensure_highlighted(
        &mut self,
        file_path: Option<&Path>,
        highlighter: &Highlighter,
    ) {
        if !self.diff_highlighted_lines.is_empty() {
            return;
        }
        let diff = match self.unified_diff {
            Some(ref d) => d,
            None => return,
        };

        let syntax = match file_path {
            Some(path) => {
                let first_line = diff.lines.iter().find_map(|l| match l {
                    UnifiedDiffLine::Context(s) | UnifiedDiffLine::Added(s) => Some(s.as_str()),
                    _ => None,
                }).unwrap_or("");
                highlighter.detect_syntax(path, first_line).name.clone()
            }
            None => "Plain Text".to_string(),
        };

        let syntax_ref = highlighter.syntax_set
            .find_syntax_by_name(&syntax)
            .unwrap_or_else(|| highlighter.syntax_set.find_syntax_plain_text());
        let mut hl_state = highlighter.new_highlight_state(syntax_ref);

        for diff_line in &diff.lines {
            match diff_line {
                UnifiedDiffLine::HunkHeader(_) => {
                    self.diff_highlighted_lines.push(Vec::new());
                }
                UnifiedDiffLine::Context(s)
                | UnifiedDiffLine::Added(s)
                | UnifiedDiffLine::Removed(s) => {
                    let spans = highlighter.highlight_line(
                        &mut hl_state,
                        &format!("{s}\n"),
                    );
                    self.diff_highlighted_lines.push(spans);
                }
            }
        }
    }

    /// Build the mapping from diff display index to file line number.
    /// Context/Added lines get `Some(line_number)` (1-indexed);
    /// Removed lines get `None` (not present in the current file).
    pub fn compute_line_numbers(&mut self) {
        self.diff_line_numbers.clear();
        let Some(ref diff) = self.unified_diff else {
            return;
        };
        let mut new_lineno: usize = 0;
        for line in &diff.lines {
            match line {
                UnifiedDiffLine::HunkHeader(_) => {}
                UnifiedDiffLine::Context(_) | UnifiedDiffLine::Added(_) => {
                    new_lineno += 1;
                    self.diff_line_numbers.push(Some(new_lineno));
                }
                UnifiedDiffLine::Removed(_) => {
                    self.diff_line_numbers.push(None);
                }
            }
        }
    }

    /// Resolve the file line at a given cursor position, falling back to the
    /// nearest file line when sitting on a Removed line in diff mode.
    /// Searches downward first (Removed->Added is the natural pair), then up.
    pub fn resolve_nearest_file_line(&self, cursor_line: usize) -> Option<usize> {
        if let Some(fl) = self.file_line_at_display(cursor_line) {
            return Some(fl);
        }
        // On a Removed line in diff mode -- scan neighbours
        let len = self.diff_line_numbers.len();
        // Down first (Added lines typically follow Removed lines)
        for i in (cursor_line + 1)..len {
            if let Some(Some(fl)) = self.diff_line_numbers.get(i) {
                return Some(*fl);
            }
        }
        // Then up
        for i in (0..cursor_line).rev() {
            if let Some(Some(fl)) = self.diff_line_numbers.get(i) {
                return Some(*fl);
            }
        }
        None
    }

    /// Reverse lookup: given a 1-indexed file line number, find the diff
    /// display index that maps to it. Returns the exact match or the closest
    /// display index if no exact match exists.
    pub fn display_index_for_file_line(&self, file_line: usize) -> usize {
        // Exact match
        if let Some(pos) = self.diff_line_numbers.iter().position(|n| *n == Some(file_line)) {
            return pos;
        }
        // Closest: find the entry with the smallest distance
        let mut best_idx = 0;
        let mut best_dist = usize::MAX;
        for (i, n) in self.diff_line_numbers.iter().enumerate() {
            if let Some(fl) = n {
                let dist = (*fl as isize - file_line as isize).unsigned_abs();
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = i;
                }
            }
        }
        best_idx
    }

    /// Get the file line number (1-indexed) at a given display index.
    /// Returns `None` for Removed lines (not in current file).
    pub fn file_line_at_display(&self, display_idx: usize) -> Option<usize> {
        self.diff_line_numbers.get(display_idx).copied().flatten()
    }

    /// Count of displayable diff lines (excluding hunk headers).
    pub fn total_lines(&self) -> usize {
        self.unified_diff.as_ref().map_or(0, |d| {
            d.lines.iter().filter(|l| !matches!(l, UnifiedDiffLine::HunkHeader(_))).count()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_diff_state(diff_lines: Vec<UnifiedDiffLine>) -> DiffState {
        let mut state = DiffState::new();
        let diff = UnifiedDiff { lines: diff_lines };
        state.unified_diff = Some(diff);
        state.compute_line_numbers();
        state
    }

    #[test]
    fn compute_line_numbers_context_added_removed() {
        let state = make_diff_state(vec![
            UnifiedDiffLine::Context("ctx1".into()),  // file line 1
            UnifiedDiffLine::Removed("old".into()),   // None
            UnifiedDiffLine::Added("new".into()),      // file line 2
            UnifiedDiffLine::Context("ctx2".into()),  // file line 3
        ]);
        assert_eq!(
            state.diff_line_numbers,
            vec![Some(1), None, Some(2), Some(3)]
        );
    }

    #[test]
    fn compute_line_numbers_skips_hunk_headers() {
        let state = make_diff_state(vec![
            UnifiedDiffLine::HunkHeader("@@ ... @@".into()),
            UnifiedDiffLine::Context("a".into()),
            UnifiedDiffLine::Added("b".into()),
        ]);
        // HunkHeader should not appear in diff_line_numbers
        assert_eq!(state.diff_line_numbers, vec![Some(1), Some(2)]);
    }

    #[test]
    fn display_index_for_file_line_exact_match() {
        let state = make_diff_state(vec![
            UnifiedDiffLine::Context("a".into()),  // idx 0, file 1
            UnifiedDiffLine::Removed("b".into()),   // idx 1, file None
            UnifiedDiffLine::Added("c".into()),      // idx 2, file 2
            UnifiedDiffLine::Context("d".into()),  // idx 3, file 3
        ]);
        assert_eq!(state.display_index_for_file_line(1), 0);
        assert_eq!(state.display_index_for_file_line(2), 2);
        assert_eq!(state.display_index_for_file_line(3), 3);
    }

    #[test]
    fn display_index_for_file_line_nearest() {
        // Only file lines 5 and 10 exist in the diff
        let mut state = DiffState::new();
        state.diff_line_numbers = vec![Some(5), None, Some(10)];
        // file_line 7 is closer to 5 (dist=2) than to 10 (dist=3)
        assert_eq!(state.display_index_for_file_line(7), 0);
        // file_line 8 is closer to 10 (dist=2) than to 5 (dist=3)
        assert_eq!(state.display_index_for_file_line(8), 2);
    }

    #[test]
    fn resolve_nearest_file_line_on_context() {
        let state = make_diff_state(vec![
            UnifiedDiffLine::Context("a".into()),
            UnifiedDiffLine::Added("b".into()),
        ]);
        assert_eq!(state.resolve_nearest_file_line(0), Some(1));
    }

    #[test]
    fn resolve_nearest_file_line_on_removed() {
        let state = make_diff_state(vec![
            UnifiedDiffLine::Context("a".into()),  // idx 0, file 1
            UnifiedDiffLine::Removed("b".into()),   // idx 1, file None
            UnifiedDiffLine::Added("c".into()),      // idx 2, file 2
        ]);
        // Should resolve to file line 2 (next Added line, searching down first)
        assert_eq!(state.resolve_nearest_file_line(1), Some(2));
    }

    #[test]
    fn total_lines_excludes_hunk_headers() {
        let state = make_diff_state(vec![
            UnifiedDiffLine::HunkHeader("@@ -1,1 +1,1 @@".into()),
            UnifiedDiffLine::Context("line1".into()),
        ]);
        assert_eq!(state.total_lines(), 1);
    }
}
