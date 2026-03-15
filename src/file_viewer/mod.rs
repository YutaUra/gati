mod comment_renderer;
mod content;
mod diff_state;
mod highlight_cache;
mod minimap;
mod render_utils;

pub use content::ViewerContent;
pub(crate) use content::read_and_classify;
pub(crate) use diff_state::DiffState;
pub(crate) use highlight_cache::HighlightCache;
pub(crate) use render_utils::{fill_row_bg, gutter_spans, line_number_width, skip_chars_in_spans};

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use crate::comments::Comment;
use crate::components::{Action, Component};

use crate::diff::{LineDiff, UnifiedDiff, UnifiedDiffLine};
use crate::highlight::Highlighter;

/// Identity of a visual row in the file viewer, used to map click positions
/// back to code lines or comment blocks.
#[derive(Clone, Copy, Debug)]
enum VisualRowContent {
    /// Code line at the given 0-indexed position.
    CodeLine(usize),
    /// Comment row for the comment spanning (start_line, end_line), both 1-indexed.
    CommentRow(usize, usize),
}

/// Columns to scroll per horizontal scroll tick.
pub const H_SCROLL_AMOUNT: usize = 4;

/// Extra padding (in columns) added beyond the longest line for horizontal scroll.
const H_SCROLL_PADDING: usize = 2;
/// Extra padding (in lines) added beyond the last line for vertical scroll.
const V_SCROLL_PADDING: usize = 1;

/// Number of rows occupied by the inline comment editor (editor + separator).
const COMMENT_EDITOR_ROWS: usize = 2;
/// Background color for stale (outdated) comment ranges.
const STALE_COMMENT_BG: Color = Color::Indexed(52);
/// Background color for active comment ranges.
const COMMENT_RANGE_BG: Color = Color::Indexed(236);
/// Background color for added lines in diff mode.
const DIFF_ADDED_BG: Color = Color::Rgb(0, 40, 0);
/// Background color for removed lines in diff mode.
const DIFF_REMOVED_BG: Color = Color::Rgb(40, 0, 0);
/// Character-level selection range, computed in `prepare_for_render()`.
/// Lines are 1-indexed, columns are 0-indexed character offsets.
/// `start` is always <= `end` (normalized from anchor + cursor).
#[derive(Debug, Clone, Copy)]
pub struct CharSelectRange {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

/// Render-time context passed from App to FileViewer for each frame.
///
/// These values are computed in `prepare_for_render()` rather than being
/// stored as persistent fields on FileViewer. This separates business logic
/// (comment loading, staleness checks) from the render path.
pub struct ViewerRenderContext<'a> {
    /// Comments for the currently viewed file, each paired with a staleness flag.
    pub comments: &'a [(Comment, bool)],
    /// Inline comment editor state (Some when in CommentInput mode).
    pub comment_edit: Option<&'a CommentEditState>,
    /// Line-select range for V mode highlighting (1-indexed start, end).
    pub line_select_range: Option<(usize, usize)>,
    /// Character-level selection range for mouse drag highlighting.
    pub char_select_range: Option<CharSelectRange>,
}

/// Active inline comment editor state, passed from App before each render.
pub struct CommentEditState {
    /// Start of the comment range (1-indexed).
    pub start_line: usize,
    /// End of the comment range (1-indexed). The editor renders below this line.
    pub target_line: usize,
    /// Current text being edited.
    pub text: String,
}

pub struct FileViewer {
    pub content: ViewerContent,
    pub scroll_offset: usize,
    /// Height of the viewer (set during render, used for half-page scroll).
    pub visible_height: usize,
    /// Cursor line position within the file (0-indexed).
    pub cursor_line: usize,
    /// Cursor column position within the line (0-indexed char offset).
    /// Set during mouse click/drag for character-level selection. None when unused.
    pub cursor_col: Option<usize>,
    /// Horizontal scroll offset in characters (0 = no horizontal scroll).
    pub h_scroll: usize,
    /// Width available for code content in characters (set during render, excludes gutter).
    pub visible_content_width: usize,
    highlighter: Highlighter,
    /// Cached syntax highlighting state for incremental highlighting.
    highlight_cache: HighlightCache,
    /// Diff-related state (line diff, unified diff, diff mode, etc.).
    pub diff: DiffState,
    /// Cached minimap rectangle from the last render (for mouse hit-testing).
    pub minimap_rect: Option<Rect>,
    /// When Some, the cursor is on a comment row identified by (start_line, end_line).
    /// cursor_line stays at end_line - 1 (0-indexed) so the comment renders just below it.
    pub cursor_on_comment: Option<(usize, usize)>,
    /// Cached content area rectangle from the last render (for mouse hit-testing).
    pub content_rect: Option<Rect>,
    /// Mapping from visual row offset (relative to content area) to content identity.
    /// Built during render, consumed by click_line.
    row_map: Vec<VisualRowContent>,
}

impl FileViewer {
    pub fn new() -> Self {
        Self {
            content: ViewerContent::Placeholder,
            scroll_offset: 0,
            visible_height: 20,
            cursor_line: 0,
            cursor_col: None,
            h_scroll: 0,
            visible_content_width: 0,
            highlighter: Highlighter::new(),
            highlight_cache: HighlightCache::new(),
            diff: DiffState::new(),
            minimap_rect: None,
            cursor_on_comment: None,
            content_rect: None,
            row_map: Vec::new(),
        }
    }

    /// Set diff data for the currently loaded file.
    pub fn set_diff(&mut self, line_diff: Option<LineDiff>, unified_diff: Option<UnifiedDiff>) {
        self.diff.set(line_diff, unified_diff);
    }

    /// Lazily compute syntax-highlighted spans for unified diff lines.
    fn ensure_diff_highlighted(&mut self) {
        let file_path = match &self.content {
            ViewerContent::File { path, .. } => Some(path.as_path()),
            _ => None,
        };
        self.diff.ensure_highlighted(file_path, &self.highlighter);
    }

    /// Load a file into the viewer.
    pub fn load_file(&mut self, path: &Path) {
        self.scroll_offset = 0;
        self.cursor_line = 0;
        self.cursor_col = None;
        self.h_scroll = 0;
        self.cursor_on_comment = None;
        self.diff.clear();
        self.highlight_cache.clear();

        // Highlighting is deferred: `ensure_highlighted_up_to` computes spans
        // incrementally during render, keeping file selection snappy.
        self.content = read_and_classify(path, &self.highlighter);
    }

    /// Re-read the current file from disk without resetting cursor/scroll position.
    /// Used by the filesystem watcher to keep content fresh.
    /// Returns true if the content was updated, false if no file is loaded or read failed.
    pub fn reload_content(&mut self) -> bool {
        let path = match &self.content {
            ViewerContent::File { path, .. } => path.clone(),
            _ => return false,
        };

        let new_content = read_and_classify(&path, &self.highlighter);

        // Clamp cursor if file got shorter
        if let ViewerContent::File { ref lines, .. } = new_content {
            let max_line = lines.len().saturating_sub(1);
            if self.cursor_line > max_line {
                self.cursor_line = max_line;
            }
        }

        // Clear highlight cache — will be recomputed lazily during render
        self.highlight_cache.clear();

        self.content = new_content;
        true
    }

    fn total_lines(&self) -> usize {
        if self.diff.diff_mode {
            return self.diff.total_lines();
        }
        match &self.content {
            ViewerContent::File { lines, .. } => lines.len(),
            _ => 0,
        }
    }

    /// Return the 1-indexed file line number at the current cursor position.
    /// In normal mode: `cursor_line + 1`. In diff mode: looked up from
    /// `diff_line_numbers` (None for Removed lines).
    pub fn cursor_file_line(&self) -> Option<usize> {
        if self.diff.diff_mode {
            self.diff.file_line_at_display(self.cursor_line)
        } else {
            Some(self.cursor_line + 1)
        }
    }

    /// Resolve the file line at the current cursor, falling back to the
    /// nearest file line when sitting on a Removed line in diff mode.
    fn resolve_nearest_file_line(&self) -> Option<usize> {
        if !self.diff.diff_mode {
            return Some(self.cursor_line + 1);
        }
        self.diff.resolve_nearest_file_line(self.cursor_line)
    }

    /// Like `cursor_file_line` but for an arbitrary cursor position.
    fn effective_file_line(&self, cursor: usize) -> Option<usize> {
        if self.diff.diff_mode {
            self.diff.file_line_at_display(cursor)
        } else {
            Some(cursor + 1)
        }
    }

    /// Find comment that ends at a given file line (1-indexed).
    /// Returns (comment, is_stale).
    fn comment_at_end_line(
        comments: &[(Comment, bool)],
        line: usize,
    ) -> Option<(&Comment, bool)> {
        comments
            .iter()
            .find(|(c, _)| c.end_line == line)
            .map(|(c, stale)| (c, *stale))
    }

    /// Return the character count for a given 0-indexed line.
    pub fn line_char_count(&self, line_idx: usize) -> usize {
        if self.diff.diff_mode {
            // In diff mode, get the text from the diff line
            self.diff.line_text_at_display(line_idx)
                .map(|s| s.chars().count())
                .unwrap_or(0)
        } else {
            match &self.content {
                ViewerContent::File { lines, .. } => {
                    lines.get(line_idx).map(|l| l.chars().count()).unwrap_or(0)
                }
                _ => 0,
            }
        }
    }

    /// Return the text content of a given 0-indexed line.
    fn line_text(&self, line_idx: usize) -> Option<&str> {
        if self.diff.diff_mode {
            self.diff.line_text_at_display(line_idx)
        } else {
            match &self.content {
                ViewerContent::File { lines, .. } => lines.get(line_idx).map(|s| s.as_str()),
                _ => None,
            }
        }
    }

    /// Move cursor to the start of the previous word.
    pub fn cursor_word_left(&mut self) {
        let col = self.cursor_col.unwrap_or(0);
        let text = self.line_text(self.cursor_line).unwrap_or("").to_string();
        self.cursor_col = Some(word_boundary_left(&text, col));
    }

    /// Move cursor to the start of the next word.
    pub fn cursor_word_right(&mut self) {
        let col = self.cursor_col.unwrap_or(0);
        let text = self.line_text(self.cursor_line).unwrap_or("").to_string();
        self.cursor_col = Some(word_boundary_right(&text, col));
    }

    /// Return the current gutter width in terminal columns.
    /// Accounts for diff marker column and line number width.
    pub fn current_gutter_cols(&self) -> usize {
        let line_count = if self.diff.diff_mode {
            // Use file line count for consistent gutter width between modes
            match &self.content {
                ViewerContent::File { lines, .. } => lines.len(),
                _ => self.diff.total_lines(),
            }
        } else {
            match &self.content {
                ViewerContent::File { lines, .. } => lines.len(),
                _ => 0,
            }
        };
        let gutter_width = line_number_width(line_count);
        let has_diff = self.diff.line_diff.is_some() || self.diff.diff_mode;
        gutter_width + 1 + if has_diff { 1 } else { 0 }
    }

    /// Convert a terminal x coordinate to a character offset within the line.
    /// Returns None if the click is on the gutter.
    pub fn column_from_terminal_x(&self, terminal_col: u16) -> Option<usize> {
        let cr = self.content_rect?;
        if terminal_col < cr.x {
            return None;
        }
        let col_in_content = (terminal_col - cr.x) as usize;
        let gutter = self.current_gutter_cols();
        if col_in_content < gutter {
            return None;
        }
        Some(col_in_content - gutter + self.h_scroll)
    }

    /// Move the cursor to the line at the given terminal coordinates.
    /// Returns true if the click was within the content area and cursor was moved.
    pub fn click_line(&mut self, row: u16, column: u16) -> bool {
        let cr = match self.content_rect {
            Some(r) => r,
            None => return false,
        };
        if row < cr.y || row >= cr.y + cr.height || column < cr.x || column >= cr.x + cr.width {
            return false;
        }
        let inner_row = (row - cr.y) as usize;
        if self.row_map.is_empty() {
            // Fallback when row_map has not been built yet (before first render)
            let target = self.scroll_offset + inner_row;
            let max = self.total_lines().saturating_sub(1);
            self.cursor_line = target.min(max);
            self.cursor_on_comment = None;
        } else {
            match self.row_map.get(inner_row) {
                Some(VisualRowContent::CodeLine(idx)) => {
                    self.cursor_line = *idx;
                    self.cursor_on_comment = None;
                }
                Some(VisualRowContent::CommentRow(start, end)) => {
                    self.cursor_line = end.saturating_sub(1); // 0-indexed
                    self.cursor_on_comment = Some((*start, *end));
                }
                None => {
                    // Click beyond rendered content — fallback to last code line
                    let max = self.total_lines().saturating_sub(1);
                    self.cursor_line = max;
                    self.cursor_on_comment = None;
                }
            }
        }

        // Track column for character-level selection
        if let Some(char_col) = self.column_from_terminal_x(column) {
            let max_col = self.line_char_count(self.cursor_line);
            self.cursor_col = Some(char_col.min(max_col));
        } else {
            self.cursor_col = Some(0);
        }

        true
    }

    /// Scroll so that the line corresponding to a minimap row is centered in the viewport.
    pub fn scroll_to_minimap_row(&mut self, row: u16, minimap_height: u16) {
        let line = minimap::row_to_line(row, minimap_height, self.total_lines());
        let half = self.visible_height / 2;
        self.scroll_offset = line.saturating_sub(half);
        self.scroll_offset = self.scroll_offset.min(self.max_scroll());
        self.cursor_line = line;
        self.cursor_on_comment = None;
    }

    fn max_scroll(&self) -> usize {
        (self.total_lines() + V_SCROLL_PADDING).saturating_sub(self.visible_height)
    }

    fn max_line_len(&self) -> usize {
        match &self.content {
            ViewerContent::File { lines, .. } => {
                lines.iter().map(|l| l.chars().count()).max().unwrap_or(0)
            }
            _ => 0,
        }
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = (self.scroll_offset + amount).min(self.max_scroll());
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_right(&mut self) {
        let max_len = self.max_line_len();
        if max_len == 0 {
            return;
        }
        let max = (max_len + H_SCROLL_PADDING).saturating_sub(self.visible_content_width);
        self.h_scroll = (self.h_scroll + H_SCROLL_AMOUNT).min(max);
    }

    pub fn scroll_left(&mut self) {
        self.h_scroll = self.h_scroll.saturating_sub(H_SCROLL_AMOUNT);
    }

    pub fn render_to_buffer(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        focused: bool,
        ctx: &ViewerRenderContext,
    ) {
        let title = if self.diff.diff_mode { " Diff " } else { " Preview " };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(crate::components::border_style(focused))
            .title(title);

        let inner = block.inner(area);
        block.render(area, buf);

        // Decide whether to show minimap: only for File content with sufficient width
        let show_minimap = matches!(self.content, ViewerContent::File { .. })
            && inner.width >= minimap::MINIMAP_MIN_WIDTH;

        let (content_area, minimap_area) = if show_minimap {
            let content_w = inner.width.saturating_sub(minimap::MINIMAP_WIDTH);
            let content = Rect::new(inner.x, inner.y, content_w, inner.height);
            let minimap = Rect::new(inner.x + content_w, inner.y, minimap::MINIMAP_WIDTH, inner.height);
            (content, Some(minimap))
        } else {
            (inner, None)
        };
        self.minimap_rect = minimap_area;
        self.content_rect = Some(content_area);

        self.visible_height = content_area.height as usize;

        // Diff mode rendering
        if self.diff.diff_mode {
            self.render_diff_mode(content_area, buf, ctx);
            if let Some(mr) = minimap_area {
                minimap::render(mr, buf, self.total_lines(), self.scroll_offset, self.visible_height, &self.diff, ctx.comments);
            }
            return;
        }

        match self.content {
            ViewerContent::File { .. } => {
                self.render_file_content(content_area, minimap_area, buf, focused, ctx);
            }
            _ => {
                self.render_placeholder(inner, buf);
            }
        }
    }

    /// Scroll the viewport so the inline comment editor remains visible.
    ///
    /// Called at the start of both `render_file_content` and
    /// `render_diff_mode` to avoid duplicating the same scroll logic.
    fn auto_scroll_for_editor(
        &mut self,
        viewport_height: usize,
        comment_edit: Option<&CommentEditState>,
    ) {
        if let Some(edit) = comment_edit {
            let target_idx = edit.target_line.saturating_sub(1);
            let need_visible = target_idx + 1 + COMMENT_EDITOR_ROWS;
            if need_visible > self.scroll_offset + viewport_height {
                self.scroll_offset = need_visible.saturating_sub(viewport_height);
            }
        }
    }

    /// Render a single-line status message for non-file content states.
    fn render_placeholder(&mut self, area: Rect, buf: &mut Buffer) {
        self.minimap_rect = None;
        let render_msg = |msg: &str, color: Color, buf: &mut Buffer| {
            let line = Line::from(Span::styled(msg, Style::default().fg(color)));
            buf.set_line(area.x, area.y, &line, area.width);
        };
        match &self.content {
            ViewerContent::Placeholder => render_msg("Select a file to preview", Color::DarkGray, buf),
            ViewerContent::Binary(_) => render_msg("Binary file — cannot display", Color::Yellow, buf),
            ViewerContent::Empty(_) => render_msg("Empty file", Color::DarkGray, buf),
            ViewerContent::Error(msg) => render_msg(msg, Color::Red, buf),
            ViewerContent::File { .. } => {}
        }
    }

    /// Build gutter spans (diff marker + line number) for a single line.
    /// Render file content with gutter, syntax highlighting, and inline comments.
    fn render_file_content(
        &mut self,
        content_area: Rect,
        minimap_area: Option<Rect>,
        buf: &mut Buffer,
        focused: bool,
        ctx: &ViewerRenderContext,
    ) {
        // Extract line count to call ensure_up_to outside the borrow.
        let line_count = match &self.content {
            ViewerContent::File { lines, .. } => lines.len(),
            _ => 0,
        };

        self.auto_scroll_for_editor(content_area.height as usize, ctx.comment_edit);

        // Incrementally compute highlights up to the last visible line.
        // Borrow-split: content, highlight_cache, and highlighter are separate fields.
        let need_up_to = (self.scroll_offset + content_area.height as usize).min(line_count);
        if let ViewerContent::File { lines, syntax_name, .. } = &self.content {
            self.highlight_cache.ensure_up_to(need_up_to, lines, syntax_name, &self.highlighter);
        }

        let gutter_width = line_number_width(line_count);
        let has_diff = self.diff.line_diff.is_some();
        // gutter = diff marker (1 if present) + line number digits + 1 space
        let gutter_cols = gutter_width + 1 + if has_diff { 1 } else { 0 };
        self.visible_content_width = (content_area.width as usize).saturating_sub(gutter_cols);

        let mut render_row: u16 = 0;
        let max_rows = content_area.height;
        self.row_map.clear();

        // Use index-based iteration instead of borrowing `lines` across the
        // loop body, avoiding a borrow conflict with &mut self methods called
        // inside (e.g. render_inline_comments).
        for code_line_idx in self.scroll_offset..line_count {
            if render_row >= max_rows {
                break;
            }

            let line_num = code_line_idx + 1;

            let mut spans = gutter_spans(line_num, gutter_width, self.diff.line_diff.as_ref());

            // Use pre-computed highlight cache (O(1) per line instead of O(scroll_offset))
            let highlighted = if code_line_idx < self.highlight_cache.highlighted_lines.len() {
                self.highlight_cache.highlighted_lines[code_line_idx].clone()
            } else {
                match &self.content {
                    ViewerContent::File { lines, .. } => vec![Span::raw(lines[code_line_idx].clone())],
                    _ => vec![Span::raw(String::new())],
                }
            };
            if self.h_scroll > 0 {
                spans.extend(skip_chars_in_spans(highlighted, self.h_scroll));
            } else {
                spans.extend(highlighted);
            }

            let line = Line::from(spans);

            let y = content_area.y + render_row;
            buf.set_line(content_area.x, y, &line, content_area.width);

            // Determine background highlight for this line
            let in_line_select = ctx.line_select_range.is_some_and(|(s, e)| {
                line_num >= s && line_num <= e
            });
            let comment_range_info = ctx
                .comments
                .iter()
                .find(|(c, _)| line_num >= c.start_line && line_num <= c.end_line);
            let in_comment_range = comment_range_info.is_some();
            let in_stale_comment = comment_range_info.is_some_and(|(_, s)| *s);

            // Check for character-level selection on this line
            let char_col_range = ctx.char_select_range.and_then(|r| {
                if line_num < r.start_line || line_num > r.end_line {
                    return None;
                }
                let line_len = match &self.content {
                    ViewerContent::File { lines, .. } => lines.get(code_line_idx).map(|l| l.chars().count()).unwrap_or(0),
                    _ => 0,
                };
                if r.start_line == r.end_line {
                    // Single line: highlight start_col..end_col
                    Some((r.start_col, r.end_col.min(line_len)))
                } else if line_num == r.start_line {
                    Some((r.start_col, line_len))
                } else if line_num == r.end_line {
                    Some((0, r.end_col.min(line_len)))
                } else {
                    // Middle line: full line
                    Some((0, line_len))
                }
            });

            let is_cursor_line = focused && code_line_idx == self.cursor_line && self.cursor_on_comment.is_none();

            // Cursor line: highlight only the gutter line number (White instead of DarkGray)
            if is_cursor_line && !in_line_select && char_col_range.is_none() {
                let gutter_end = content_area.x + gutter_cols as u16;
                for x in content_area.x..gutter_end.min(content_area.x + content_area.width) {
                    let cell = &mut buf[(x, y)];
                    if cell.fg == Color::DarkGray {
                        cell.set_fg(Color::White);
                    }
                }
            }

            // Full-row background for line-select and comment ranges (not for cursor line alone)
            let highlight_bg = if char_col_range.is_some() {
                // Char select uses per-cell highlighting below, skip full-row bg
                None
            } else if in_line_select {
                Some(Color::DarkGray)
            } else if in_stale_comment {
                Some(STALE_COMMENT_BG)
            } else if in_comment_range {
                Some(COMMENT_RANGE_BG)
            } else {
                None
            };
            if let Some(bg) = highlight_bg {
                for x in content_area.x..content_area.x + content_area.width {
                    let cell = &mut buf[(x, y)];
                    cell.set_bg(bg);
                    // Keep line numbers readable on dark background
                    if in_comment_range && cell.fg == Color::DarkGray {
                        cell.set_fg(Color::Gray);
                    }
                }
            }

            // Apply character-level selection highlight
            if let Some((sel_start, sel_end)) = char_col_range {
                let gutter_x = content_area.x + gutter_cols as u16;
                // Convert char offsets to terminal x positions accounting for h_scroll
                let term_start = if sel_start >= self.h_scroll {
                    gutter_x + (sel_start - self.h_scroll) as u16
                } else {
                    gutter_x
                };
                let term_end = if sel_end >= self.h_scroll {
                    gutter_x + (sel_end - self.h_scroll) as u16
                } else {
                    gutter_x
                };
                let max_x = content_area.x + content_area.width;
                for x in term_start..term_end.min(max_x) {
                    let cell = &mut buf[(x, y)];
                    cell.set_bg(Color::DarkGray);
                }
            }

            // Block cursor: invert fg/bg at the cursor character position
            if is_cursor_line {
                let col = self.cursor_col.unwrap_or(0);
                let gutter_x = content_area.x + gutter_cols as u16;
                if col >= self.h_scroll {
                    let cursor_x = gutter_x + (col - self.h_scroll) as u16;
                    if cursor_x < content_area.x + content_area.width {
                        let cell = &mut buf[(cursor_x, y)];
                        let fg = cell.fg;
                        let bg = cell.bg;
                        cell.set_fg(bg);
                        cell.set_bg(fg);
                    }
                }
            }

            self.row_map.push(VisualRowContent::CodeLine(code_line_idx));
            render_row += 1;

            self.render_inline_comments(
                line_num, content_area, buf, &mut render_row, max_rows, focused, ctx,
            );
        }

        // Render minimap after content
        if let Some(mr) = minimap_area {
            minimap::render(mr, buf, self.total_lines(), self.scroll_offset, self.visible_height, &self.diff, ctx.comments);
        }
    }

    /// Render inline comment editor or comment block after a code line.
    ///
    /// Shared by `render_file_content` and `render_diff_mode` to avoid
    /// duplicating the editor-vs-block dispatch, row_map registration,
    /// and cursor highlight logic.
    #[allow(clippy::too_many_arguments)]
    fn render_inline_comments(
        &mut self,
        file_line_num: usize,
        inner: Rect,
        buf: &mut Buffer,
        render_row: &mut u16,
        max_rows: u16,
        focused: bool,
        ctx: &ViewerRenderContext,
    ) {
        let editing_this_line = ctx
            .comment_edit
            .is_some_and(|e| e.target_line == file_line_num);

        if editing_this_line {
            if let Some(edit) = ctx.comment_edit {
                let before = *render_row;
                comment_renderer::render_comment_editor(edit, inner, buf, render_row, max_rows);
                for _ in before..*render_row {
                    self.row_map
                        .push(VisualRowContent::CommentRow(edit.start_line, edit.target_line));
                }
            }
        } else if let Some((comment, is_stale)) = Self::comment_at_end_line(ctx.comments, file_line_num)
            && *render_row < max_rows
        {
            let cursor_on_this = focused
                && self
                    .cursor_on_comment
                    .is_some_and(|(s, e)| s == comment.start_line && e == comment.end_line);
            let comment_render_start = *render_row;
            let c_start = comment.start_line;
            let c_end = comment.end_line;
            comment_renderer::render_comment_block(comment, is_stale, inner, buf, render_row, max_rows);
            for _ in comment_render_start..*render_row {
                self.row_map
                    .push(VisualRowContent::CommentRow(c_start, c_end));
            }
            if cursor_on_this {
                for r in comment_render_start..*render_row {
                    let y = inner.y + r;
                    fill_row_bg(buf, inner.x, y, inner.width, Color::DarkGray);
                }
            }
        }
    }


    /// Render unified diff content with language syntax highlighting and inline comments.
    fn render_diff_mode(&mut self, inner: Rect, buf: &mut Buffer, ctx: &ViewerRenderContext) {
        self.ensure_diff_highlighted();

        // Collect displayable lines (skip hunk headers) into an owned vec so
        // we don't hold an immutable borrow on self.diff.unified_diff across the
        // &mut self calls below (render_inline_comments, auto_scroll_for_editor).
        //
        // Each entry is (orig_index, cloned UnifiedDiffLine).
        let displayable: Vec<(usize, UnifiedDiffLine)> = match self.diff.unified_diff {
            Some(ref diff) => diff
                .lines
                .iter()
                .enumerate()
                .filter(|(_, l)| !matches!(l, UnifiedDiffLine::HunkHeader(_)))
                .map(|(i, l)| (i, l.clone()))
                .collect(),
            None => {
                let msg = "No changes";
                let line = Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)));
                buf.set_line(inner.x, inner.y, &line, inner.width);
                return;
            }
        };

        if displayable.is_empty() {
            let msg = "No changes";
            let line = Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)));
            buf.set_line(inner.x, inner.y, &line, inner.width);
            return;
        }

        // Use the file's total line count for gutter width so it matches preview mode
        let file_line_count = match &self.content {
            ViewerContent::File { lines, .. } => lines.len(),
            _ => displayable.iter().filter(|(_, l)| {
                !matches!(l, UnifiedDiffLine::Removed(_))
            }).count(),
        };
        let gutter_width = line_number_width(file_line_count);

        self.auto_scroll_for_editor(inner.height as usize, ctx.comment_edit);

        let mut render_row: u16 = 0;
        let max_rows = inner.height;
        self.row_map.clear();

        for (disp_idx, (orig_idx, diff_line)) in displayable
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
        {
            if render_row >= max_rows {
                break;
            }

            let y = inner.y + render_row;

            // Prefix (+/-/space) in the same column as the diff marker (▎) in preview mode
            let prefix = match diff_line {
                UnifiedDiffLine::Added(_) => "+",
                UnifiedDiffLine::Removed(_) => "-",
                _ => " ",
            };
            let prefix_style = match diff_line {
                UnifiedDiffLine::Added(_) => Style::default().fg(Color::Green),
                UnifiedDiffLine::Removed(_) => Style::default().fg(Color::Red),
                _ => Style::default().fg(Color::DarkGray),
            };

            // Line number gutter (same position as preview mode)
            let num_str = match self.diff.diff_line_numbers.get(disp_idx).copied().flatten() {
                Some(n) => format!("{:>width$} ", n, width = gutter_width),
                None => format!("{:>width$} ", "", width = gutter_width),
            };

            let mut spans = vec![
                Span::styled(prefix, prefix_style),
                Span::styled(num_str, Style::default().fg(Color::DarkGray)),
            ];

            // Use pre-computed syntax-highlighted spans if available
            if *orig_idx < self.diff.diff_highlighted_lines.len()
                && !self.diff.diff_highlighted_lines[*orig_idx].is_empty()
            {
                spans.extend(self.diff.diff_highlighted_lines[*orig_idx].clone());
            } else {
                let text = match diff_line {
                    UnifiedDiffLine::Added(s)
                    | UnifiedDiffLine::Removed(s)
                    | UnifiedDiffLine::Context(s) => s.as_str(),
                    _ => "",
                };
                spans.push(Span::raw(text.to_string()));
            }

            let line = Line::from(spans);
            buf.set_line(inner.x, y, &line, inner.width);

            // Cursor highlight for code line
            let is_cursor_line = disp_idx == self.cursor_line && self.cursor_on_comment.is_none();
            let file_ln = self.diff.diff_line_numbers.get(disp_idx).copied().flatten();

            // Check for character-level selection in diff mode
            let diff_char_col_range = file_ln.and_then(|ln| {
                let r = ctx.char_select_range?;
                if ln < r.start_line || ln > r.end_line {
                    return None;
                }
                let line_text = match diff_line {
                    UnifiedDiffLine::Added(s)
                    | UnifiedDiffLine::Removed(s)
                    | UnifiedDiffLine::Context(s) => s.as_str(),
                    _ => "",
                };
                let line_len = line_text.chars().count();
                if r.start_line == r.end_line {
                    Some((r.start_col, r.end_col.min(line_len)))
                } else if ln == r.start_line {
                    Some((r.start_col, line_len))
                } else if ln == r.end_line {
                    Some((0, r.end_col.min(line_len)))
                } else {
                    Some((0, line_len))
                }
            });

            // Cursor line: highlight only the gutter line number (White instead of DarkGray)
            if is_cursor_line {
                let diff_gutter_end = inner.x + (1 + gutter_width + 1) as u16;
                for x in inner.x..diff_gutter_end.min(inner.x + inner.width) {
                    let cell = &mut buf[(x, y)];
                    if cell.fg == Color::DarkGray {
                        cell.set_fg(Color::White);
                    }
                }
            }

            // Apply background tint for added/removed lines (no longer for cursor line)
            if diff_char_col_range.is_none() {
                match diff_line {
                    UnifiedDiffLine::Added(_) => {
                        fill_row_bg(buf, inner.x, y, inner.width, DIFF_ADDED_BG);
                    }
                    UnifiedDiffLine::Removed(_) => {
                        fill_row_bg(buf, inner.x, y, inner.width, DIFF_REMOVED_BG);
                    }
                    _ => {}
                }
            }

            // Apply character-level selection highlight in diff mode
            if let Some((sel_start, sel_end)) = diff_char_col_range {
                let diff_gutter_cols = 1 + gutter_width + 1; // prefix + line number + space
                let gutter_x = inner.x + diff_gutter_cols as u16;
                let term_start = gutter_x + sel_start as u16;
                let term_end = gutter_x + sel_end as u16;
                let max_x = inner.x + inner.width;
                for x in term_start..term_end.min(max_x) {
                    let cell = &mut buf[(x, y)];
                    cell.set_bg(Color::DarkGray);
                }
            }

            // Block cursor: invert fg/bg at the cursor character position
            if is_cursor_line {
                let col = self.cursor_col.unwrap_or(0);
                let diff_gutter_cols = 1 + gutter_width + 1;
                let gutter_x = inner.x + diff_gutter_cols as u16;
                let cursor_x = gutter_x + col as u16;
                if cursor_x < inner.x + inner.width {
                    let cell = &mut buf[(cursor_x, y)];
                    let fg = cell.fg;
                    let bg = cell.bg;
                    cell.set_fg(bg);
                    cell.set_bg(fg);
                }
            }

            self.row_map.push(VisualRowContent::CodeLine(disp_idx));
            render_row += 1;

            // Render inline comment editor or existing comment block after this line
            if let Some(ln) = file_ln {
                self.render_inline_comments(
                    ln, inner, buf, &mut render_row, max_rows, true, ctx,
                );
            }
        }
    }

    /// Move cursor down by one line, auto-scrolling viewport if needed.
    /// Stops on comment rows that appear between code lines.
    pub fn cursor_down(&mut self, comments: &[(Comment, bool)]) {
        let max = self.total_lines().saturating_sub(1);
        if self.cursor_on_comment.is_some() {
            // Currently on a comment row → move to the next code line
            self.cursor_on_comment = None;
            if self.cursor_line < max {
                self.cursor_line += 1;
            }
        } else if self.cursor_line < max {
            // Check if there is a comment ending at the current file line.
            // If so, stop on the comment row instead of advancing cursor_line.
            if let Some(ln) = self.effective_file_line(self.cursor_line) {
                if let Some((c, _)) = Self::comment_at_end_line(comments, ln) {
                    self.cursor_on_comment = Some((c.start_line, c.end_line));
                } else {
                    self.cursor_line += 1;
                }
            } else {
                // Removed line in diff mode — no comment possible, just advance
                self.cursor_line += 1;
            }
        }
        // Clamp cursor_col to new line length
        if let Some(col) = self.cursor_col {
            let max_col = self.line_char_count(self.cursor_line);
            self.cursor_col = Some(col.min(max_col));
        }
        self.ensure_cursor_visible();
    }

    /// Move cursor up by one line, auto-scrolling viewport if needed.
    /// Stops on comment rows that appear between code lines.
    pub fn cursor_up(&mut self, comments: &[(Comment, bool)]) {
        if self.cursor_on_comment.is_some() {
            // Currently on a comment row → move up to the code line above (cursor_line stays)
            self.cursor_on_comment = None;
            // cursor_line is already at the code line just above the comment
        } else if self.cursor_line > 0 {
            // Check the line above for a comment.
            // In normal mode: comment_at_end_line(cursor_line) checks the line above.
            // In diff mode: use the file line number of cursor_line - 1.
            let check_line = if self.diff.diff_mode {
                // Look at the previous display line's file line number
                self.effective_file_line(self.cursor_line.saturating_sub(1))
            } else {
                // cursor_line is 0-indexed, so cursor_line gives 1-indexed line above
                Some(self.cursor_line)
            };
            if let Some(ln) = check_line {
                if let Some((c, _)) = Self::comment_at_end_line(comments, ln) {
                    self.cursor_on_comment = Some((c.start_line, c.end_line));
                    self.cursor_line -= 1;
                } else {
                    self.cursor_line -= 1;
                }
            } else {
                // Removed line — just move up
                self.cursor_line -= 1;
            }
        }
        // Clamp cursor_col to new line length
        if let Some(col) = self.cursor_col {
            let max_col = self.line_char_count(self.cursor_line);
            self.cursor_col = Some(col.min(max_col));
        }
        self.ensure_cursor_visible();
    }

    /// Scroll to a specific line (1-indexed) and place the cursor there.
    pub fn scroll_to_line(&mut self, line: usize) {
        self.cursor_line = line.saturating_sub(1);
        self.cursor_on_comment = None;
        self.ensure_cursor_visible();
    }

    /// Get the path of the currently loaded file, if any.
    pub fn current_file(&self) -> Option<&Path> {
        match &self.content {
            ViewerContent::File { path, .. } => Some(path),
            _ => None,
        }
    }

    /// Get the lines of the currently loaded file content.
    pub fn current_lines(&self) -> &[String] {
        match &self.content {
            ViewerContent::File { lines, .. } => lines,
            _ => &[],
        }
    }

    /// Move cursor column left by one character.
    pub fn cursor_left(&mut self) {
        let col = self.cursor_col.unwrap_or(0);
        self.cursor_col = Some(col.saturating_sub(1));
    }

    /// Move cursor column right by one character, clamped to line length.
    pub fn cursor_right(&mut self) {
        let col = self.cursor_col.unwrap_or(0);
        let max_col = self.line_char_count(self.cursor_line);
        self.cursor_col = Some((col + 1).min(max_col));
    }

    /// Ensure the cursor line is visible within the viewport.
    fn ensure_cursor_visible(&mut self) {
        if self.cursor_line < self.scroll_offset {
            self.scroll_offset = self.cursor_line;
        } else if self.cursor_line >= self.scroll_offset + self.visible_height {
            self.scroll_offset = self.cursor_line - self.visible_height + 1;
        }
    }
}

impl Component for FileViewer {
    fn handle_event(&mut self, key: KeyEvent) -> anyhow::Result<Action> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                Ok(Action::CursorDown)
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                Ok(Action::CursorUp)
            }
            (KeyCode::Char('c'), KeyModifiers::NONE) => Ok(Action::StartComment),
            (KeyCode::Char('x'), KeyModifiers::NONE)
            | (KeyCode::Delete, _)
            | (KeyCode::Backspace, _) => Ok(Action::DeleteComment),
            (KeyCode::Char('V'), KeyModifiers::SHIFT) => Ok(Action::StartLineSelect),
            (KeyCode::Char('e'), KeyModifiers::NONE) => Ok(Action::ExportComments),
            (KeyCode::Char('B'), KeyModifiers::SHIFT) => Ok(Action::BugReport),
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                // Toggle diff mode (only when unified diff data is available)
                if self.diff.unified_diff.is_some() {
                    // 1. Remember the file line and viewport-relative position
                    let file_line = self.resolve_nearest_file_line();
                    let screen_row = self.cursor_line.saturating_sub(self.scroll_offset);

                    // 2. Toggle mode
                    self.diff.diff_mode = !self.diff.diff_mode;
                    self.cursor_on_comment = None;
                    if self.diff.diff_mode {
                        self.diff.compute_line_numbers();
                    }

                    // 3. Map the saved file line to the new mode's index
                    if let Some(fl) = file_line {
                        self.cursor_line = if self.diff.diff_mode {
                            self.diff.display_index_for_file_line(fl)
                        } else {
                            fl.saturating_sub(1) // 1-indexed → 0-indexed
                        };
                        // Clamp cursor to valid range
                        let max_line = self.total_lines().saturating_sub(1);
                        self.cursor_line = self.cursor_line.min(max_line);

                        // 4. Restore viewport-relative position
                        self.scroll_offset = self.cursor_line.saturating_sub(screen_row);
                        let max_s = self.max_scroll();
                        if self.scroll_offset > max_s {
                            self.scroll_offset = max_s;
                        }
                    } else {
                        // No file line found (empty diff) – reset
                        self.scroll_offset = 0;
                        self.cursor_line = 0;
                    }
                }
                Ok(Action::None)
            }
            (KeyCode::Char('h'), KeyModifiers::NONE) => {
                self.scroll_left();
                Ok(Action::None)
            }
            (KeyCode::Left, KeyModifiers::ALT) => {
                self.cursor_word_left();
                Ok(Action::None)
            }
            (KeyCode::Left, _) => {
                self.cursor_left();
                Ok(Action::None)
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) => {
                self.scroll_right();
                Ok(Action::None)
            }
            (KeyCode::Right, KeyModifiers::ALT) => {
                self.cursor_word_right();
                Ok(Action::None)
            }
            (KeyCode::Right, _) => {
                self.cursor_right();
                Ok(Action::None)
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                let half = self.visible_height / 2;
                self.scroll_down(half);
                // Move cursor with the scroll
                let max = self.total_lines().saturating_sub(1);
                self.cursor_line = (self.cursor_line + half).min(max);
                self.cursor_on_comment = None;
                self.ensure_cursor_visible();
                Ok(Action::None)
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                let half = self.visible_height / 2;
                self.scroll_up(half);
                // Move cursor with the scroll
                self.cursor_line = self.cursor_line.saturating_sub(half);
                self.cursor_on_comment = None;
                self.ensure_cursor_visible();
                Ok(Action::None)
            }
            (KeyCode::Tab, _) => Ok(Action::SwitchFocus),
            (KeyCode::Char('q'), KeyModifiers::NONE) => Ok(Action::Quit),
            _ => Ok(Action::None),
        }
    }
}

/// Find the char offset of the previous word boundary.
/// A "word" is a run of alphanumeric/underscore chars, or a run of punctuation.
fn word_boundary_left(text: &str, col: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    if col == 0 {
        return 0;
    }
    let mut pos = col.min(chars.len());
    // Skip whitespace going left
    while pos > 0 && chars[pos - 1].is_whitespace() {
        pos -= 1;
    }
    if pos == 0 {
        return 0;
    }
    // Skip word or punctuation chars
    if chars[pos - 1].is_alphanumeric() || chars[pos - 1] == '_' {
        while pos > 0 && (chars[pos - 1].is_alphanumeric() || chars[pos - 1] == '_') {
            pos -= 1;
        }
    } else {
        while pos > 0
            && !chars[pos - 1].is_alphanumeric()
            && chars[pos - 1] != '_'
            && !chars[pos - 1].is_whitespace()
        {
            pos -= 1;
        }
    }
    pos
}

/// Find the char offset of the next word boundary.
fn word_boundary_right(text: &str, col: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if col >= len {
        return len;
    }
    let mut pos = col;
    // Skip current word/punctuation
    if chars[pos].is_alphanumeric() || chars[pos] == '_' {
        while pos < len && (chars[pos].is_alphanumeric() || chars[pos] == '_') {
            pos += 1;
        }
    } else if !chars[pos].is_whitespace() {
        while pos < len
            && !chars[pos].is_alphanumeric()
            && chars[pos] != '_'
            && !chars[pos].is_whitespace()
        {
            pos += 1;
        }
    }
    // Skip whitespace
    while pos < len && chars[pos].is_whitespace() {
        pos += 1;
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn tmp_file(name: &str, content: &[u8]) -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(name);
        fs::write(&path, content).unwrap();
        (tmp, path)
    }

    fn empty_ctx() -> ViewerRenderContext<'static> {
        ViewerRenderContext {
            comments: &[],
            comment_edit: None,
            line_select_range: None,
            char_select_range: None,
        }
    }

    /// Process a key event on a viewer, executing cursor actions immediately.
    ///
    /// Since cursor_down/cursor_up now return Actions instead of moving the
    /// cursor directly (comments are passed from the caller), tests that only
    /// exercise FileViewer in isolation use this helper with no comments.
    fn handle_and_apply(viewer: &mut FileViewer, key: KeyEvent) -> Action {
        let action = viewer.handle_event(key).unwrap();
        match action {
            Action::CursorDown => viewer.cursor_down(&[]),
            Action::CursorUp => viewer.cursor_up(&[]),
            _ => {}
        }
        action
    }

    // 4.1: Load file
    #[test]
    fn load_file_reads_contents_into_lines() {
        let (_tmp, path) = tmp_file("test.rs", b"line1\nline2\nline3");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        match &viewer.content {
            ViewerContent::File { lines, .. } => {
                assert_eq!(lines, &vec!["line1", "line2", "line3"]);
            }
            other => panic!("Expected File content, got {:?}", other),
        }
    }

    // Cursor line
    #[test]
    fn initial_cursor_line_is_zero() {
        let (_tmp, path) = tmp_file("test.rs", b"line1\nline2\nline3");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        assert_eq!(viewer.cursor_line, 0);
    }

    #[test]
    fn j_moves_cursor_down() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        handle_and_apply(&mut viewer, key(KeyCode::Char('j')));
        assert_eq!(viewer.cursor_line, 1);
    }

    #[test]
    fn k_moves_cursor_up() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.cursor_line = 5;

        handle_and_apply(&mut viewer, key(KeyCode::Char('k')));
        assert_eq!(viewer.cursor_line, 4);
    }

    #[test]
    fn cursor_clamped_at_end() {
        let (_tmp, path) = tmp_file("short.txt", b"line1\nline2");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.cursor_line = 1; // last line

        handle_and_apply(&mut viewer, key(KeyCode::Char('j')));
        assert_eq!(viewer.cursor_line, 1); // stays at max
    }

    #[test]
    fn cursor_clamped_at_beginning() {
        let (_tmp, path) = tmp_file("short.txt", b"line1\nline2");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        handle_and_apply(&mut viewer, key(KeyCode::Char('k')));
        assert_eq!(viewer.cursor_line, 0); // stays at 0
    }

    #[test]
    fn cursor_scrolls_viewport_when_moving_below() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.visible_height = 10;
        viewer.load_file(&path);
        // Move cursor to bottom of viewport
        viewer.cursor_line = 9;
        handle_and_apply(&mut viewer, key(KeyCode::Char('j')));
        // Cursor moved to line 10, viewport should scroll to keep it visible
        assert_eq!(viewer.cursor_line, 10);
        assert!(viewer.scroll_offset > 0);
    }

    #[test]
    fn scroll_down_stops_when_last_line_at_viewport_bottom() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.visible_height = 20;
        viewer.load_file(&path);

        // Scroll all the way down
        for _ in 0..200 {
            viewer.scroll_down(1);
        }

        // Last line (index 99) should be at the bottom of the viewport + padding,
        // so scroll_offset = total_lines + V_SCROLL_PADDING - visible_height = 100 + 3 - 20 = 83
        assert_eq!(viewer.scroll_offset, 100 + V_SCROLL_PADDING - 20);
    }

    #[test]
    fn short_file_does_not_scroll() {
        let (_tmp, path) = tmp_file("short.txt", b"line1\nline2\nline3");
        let mut viewer = FileViewer::new();
        viewer.visible_height = 20;
        viewer.load_file(&path);

        viewer.scroll_down(10);
        // 3 lines < 20 visible height → no scrolling possible
        assert_eq!(viewer.scroll_offset, 0);
    }

    #[test]
    fn cursor_scrolls_viewport_when_moving_above() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.visible_height = 10;
        viewer.load_file(&path);
        viewer.scroll_offset = 20;
        viewer.cursor_line = 20;
        handle_and_apply(&mut viewer, key(KeyCode::Char('k')));
        assert_eq!(viewer.cursor_line, 19);
        // Viewport should stay or adjust so cursor is visible
        assert!(viewer.scroll_offset <= 19);
    }

    #[test]
    fn arrow_keys_move_cursor() {
        let content: Vec<u8> = (0..10).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("file.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        handle_and_apply(&mut viewer, key(KeyCode::Down));
        assert_eq!(viewer.cursor_line, 1);

        handle_and_apply(&mut viewer, key(KeyCode::Up));
        assert_eq!(viewer.cursor_line, 0);
    }

    #[test]
    fn ctrl_d_scrolls_half_page_and_moves_cursor() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.visible_height = 20;
        viewer.load_file(&path);

        viewer.handle_event(ctrl_key('d')).unwrap();
        assert_eq!(viewer.scroll_offset, 10);
        // Cursor should move with scroll
        assert_eq!(viewer.cursor_line, 10);
    }

    #[test]
    fn ctrl_u_scrolls_half_page_and_moves_cursor() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.visible_height = 20;
        viewer.load_file(&path);
        viewer.scroll_offset = 20;
        viewer.cursor_line = 20;

        viewer.handle_event(ctrl_key('u')).unwrap();
        assert_eq!(viewer.scroll_offset, 10);
        assert_eq!(viewer.cursor_line, 10);
    }

    // 4.4: Binary detection
    #[test]
    fn detects_binary_file() {
        let data = vec![0x48, 0x65, 0x00, 0x6C, 0x6C, 0x6F]; // "He\0llo"
        let (_tmp, path) = tmp_file("binary.dat", &data);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        assert!(matches!(viewer.content, ViewerContent::Binary(_)));
    }

    #[test]
    fn text_file_not_detected_as_binary() {
        let (_tmp, path) = tmp_file("text.txt", b"Hello, world!");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        assert!(matches!(viewer.content, ViewerContent::File { .. }));
    }

    // 4.5: Empty file and placeholder
    #[test]
    fn empty_file_shows_empty_message() {
        let (_tmp, path) = tmp_file("empty.txt", b"");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        assert!(matches!(viewer.content, ViewerContent::Empty(_)));
    }

    #[test]
    fn no_file_selected_shows_placeholder() {
        let viewer = FileViewer::new();
        assert_eq!(viewer.content, ViewerContent::Placeholder);
    }

    // 4.6: Permission errors
    #[test]
    fn unreadable_file_shows_error() {
        let (_tmp, path) = tmp_file("secret.txt", b"secret");
        // Make file unreadable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        }

        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        #[cfg(unix)]
        assert!(
            matches!(viewer.content, ViewerContent::Error(_)),
            "Expected Error content, got {:?}",
            viewer.content
        );

        // Restore permissions for cleanup
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        }
    }

    // Tab and q
    #[test]
    fn tab_returns_switch_focus() {
        let mut viewer = FileViewer::new();
        let action = viewer.handle_event(key(KeyCode::Tab)).unwrap();
        assert_eq!(action, Action::SwitchFocus);
    }

    #[test]
    fn q_returns_quit() {
        let mut viewer = FileViewer::new();
        let action = viewer.handle_event(key(KeyCode::Char('q'))).unwrap();
        assert_eq!(action, Action::Quit);
    }

    // Syntax highlighting integration
    #[test]
    fn load_file_detects_rust_syntax() {
        let (_tmp, path) = tmp_file("test.rs", b"fn main() {}\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        match &viewer.content {
            ViewerContent::File { syntax_name, .. } => {
                assert_eq!(syntax_name, "Rust");
            }
            other => panic!("Expected File content, got {:?}", other),
        }
    }

    #[test]
    fn load_file_falls_back_to_plain_text_for_unknown_extension() {
        let (_tmp, path) = tmp_file("data.xyz999", b"some content\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        match &viewer.content {
            ViewerContent::File { syntax_name, .. } => {
                assert_eq!(syntax_name, "Plain Text");
            }
            other => panic!("Expected File content, got {:?}", other),
        }
    }

    #[test]
    fn render_highlighted_file_does_not_panic() {
        let (_tmp, path) = tmp_file("test.rs", b"fn main() {\n    println!(\"hello\");\n}\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true, &empty_ctx());
        // Should not panic; content should be rendered
    }

    #[test]
    fn render_plain_text_file_does_not_panic() {
        let (_tmp, path) = tmp_file("notes.xyz999", b"line 1\nline 2\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true, &empty_ctx());
    }

    #[test]
    fn binary_file_unaffected_by_highlighting() {
        let data = vec![0x48, 0x65, 0x00, 0x6C, 0x6C, 0x6F];
        let (_tmp, path) = tmp_file("image.png", &data);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        assert!(matches!(viewer.content, ViewerContent::Binary(_)));
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true, &empty_ctx());
    }

    #[test]
    fn empty_file_unaffected_by_highlighting() {
        let (_tmp, path) = tmp_file("empty.rs", b"");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        assert!(matches!(viewer.content, ViewerContent::Empty(_)));
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true, &empty_ctx());
    }

    #[test]
    fn placeholder_unaffected_by_highlighting() {
        let mut viewer = FileViewer::new();
        assert_eq!(viewer.content, ViewerContent::Placeholder);
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, false, &empty_ctx());
    }

    #[test]
    fn nonexistent_file_shows_deleted_message() {
        let mut viewer = FileViewer::new();
        viewer.load_file(Path::new("/nonexistent/deleted_file.rs"));

        match &viewer.content {
            ViewerContent::Error(msg) => {
                assert!(
                    msg.contains("deleted") || msg.contains("Deleted"),
                    "Error for nonexistent file should mention deletion, got: {msg}"
                );
                assert!(
                    !msg.contains("os error"),
                    "Error should not expose raw OS error, got: {msg}"
                );
            }
            other => panic!("Expected Error content, got {:?}", other),
        }
    }

    // Diff mode syntax highlighting (lazy: computed on first ensure_diff_highlighted call)
    #[test]
    fn diff_highlighted_lines_populated_on_ensure() {
        let (_tmp, path) = tmp_file("test.rs", b"fn main() {\n    println!(\"hi\");\n}\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        let diff = UnifiedDiff {
            lines: vec![
                UnifiedDiffLine::HunkHeader("@@ -1,3 +1,3 @@".to_string()),
                UnifiedDiffLine::Context("fn main() {".to_string()),
                UnifiedDiffLine::Removed("    println!(\"hi\");".to_string()),
                UnifiedDiffLine::Added("    println!(\"hello\");".to_string()),
                UnifiedDiffLine::Context("}".to_string()),
            ],
        };
        viewer.set_diff(None, Some(diff));

        // Highlights should NOT be computed eagerly
        assert!(viewer.diff.diff_highlighted_lines.is_empty());

        // Trigger lazy computation
        viewer.ensure_diff_highlighted();

        assert_eq!(viewer.diff.diff_highlighted_lines.len(), 5);

        // Hunk header should have no syntax highlighting (empty spans)
        assert!(viewer.diff.diff_highlighted_lines[0].is_empty());

        // Code lines should have syntax-highlighted spans with RGB colors
        let code_spans = &viewer.diff.diff_highlighted_lines[1]; // "fn main() {"
        assert!(!code_spans.is_empty());
        assert!(
            code_spans.iter().any(|s| matches!(s.style.fg, Some(Color::Rgb(_, _, _)))),
            "Code spans should have RGB colors from syntax highlighting"
        );
    }

    #[test]
    fn diff_mode_render_with_syntax_highlighting_does_not_panic() {
        let (_tmp, path) = tmp_file("test.rs", b"fn main() {\n    println!(\"hi\");\n}\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        let diff = UnifiedDiff {
            lines: vec![
                UnifiedDiffLine::HunkHeader("@@ -1,3 +1,3 @@".to_string()),
                UnifiedDiffLine::Context("fn main() {".to_string()),
                UnifiedDiffLine::Removed("    println!(\"hi\");".to_string()),
                UnifiedDiffLine::Added("    println!(\"hello\");".to_string()),
                UnifiedDiffLine::Context("}".to_string()),
            ],
        };
        viewer.set_diff(None, Some(diff));
        viewer.diff.diff_mode = true;

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true, &empty_ctx());
        // Should render without panic
    }

    #[test]
    fn diff_mode_does_not_render_hunk_headers() {
        let (_tmp, path) = tmp_file("test.rs", b"fn main() {}\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        let diff = UnifiedDiff {
            lines: vec![
                UnifiedDiffLine::HunkHeader("@@ -1,1 +1,1 @@".to_string()),
                UnifiedDiffLine::Context("fn main() {}".to_string()),
            ],
        };
        viewer.set_diff(None, Some(diff));
        viewer.diff.diff_mode = true;

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true, &empty_ctx());

        // Extract rendered text from first row
        let row_text: String = (0..60).map(|x| buf[(x, 1)].symbol().to_string()).collect();
        assert!(
            !row_text.contains("@@"),
            "Hunk headers should not appear in diff view, got: {row_text}"
        );
    }

    #[test]
    fn diff_mode_shows_line_numbers_for_context_and_added() {
        let (_tmp, path) = tmp_file("test.rs", b"line1\nline2\nline3\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        let diff = UnifiedDiff {
            lines: vec![
                UnifiedDiffLine::HunkHeader("@@ -1,3 +1,4 @@".to_string()),
                UnifiedDiffLine::Context("line1".to_string()),
                UnifiedDiffLine::Context("line2".to_string()),
                UnifiedDiffLine::Added("new_line".to_string()),
                UnifiedDiffLine::Context("line3".to_string()),
            ],
        };
        viewer.set_diff(None, Some(diff));
        viewer.diff.diff_mode = true;

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true, &empty_ctx());

        // Row 1 (inner y=1): Context "line1" → line number 1
        let row1: String = (0..10).map(|x| buf[(x, 1)].symbol().to_string()).collect();
        assert!(row1.contains("1"), "Context line should show line number 1, got: {row1}");

        // Row 3 (inner y=3): Added "new_line" → line number 3
        let row3: String = (0..10).map(|x| buf[(x, 3)].symbol().to_string()).collect();
        assert!(row3.contains("3"), "Added line should show line number 3, got: {row3}");
    }

    #[test]
    fn diff_mode_no_line_number_for_removed_lines() {
        let (_tmp, path) = tmp_file("test.rs", b"line1\nline3\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        let diff = UnifiedDiff {
            lines: vec![
                UnifiedDiffLine::HunkHeader("@@ -1,3 +1,2 @@".to_string()),
                UnifiedDiffLine::Context("line1".to_string()),
                UnifiedDiffLine::Removed("line2".to_string()),
                UnifiedDiffLine::Context("line3".to_string()),
            ],
        };
        viewer.set_diff(None, Some(diff));
        viewer.diff.diff_mode = true;

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true, &empty_ctx());

        // Row 2 (inner y=2): Removed "line2" → no line number, just spaces in gutter
        let row2_gutter: String = (0..5).map(|x| buf[(x, 2)].symbol().to_string()).collect();
        // Should not contain any digit
        assert!(
            !row2_gutter.chars().any(|c| c.is_ascii_digit()),
            "Removed line should not show a line number, got gutter: '{row2_gutter}'"
        );
    }

    #[test]
    fn diff_mode_gutter_aligns_with_preview_mode() {
        // Gutter layout should be the same width in both modes so line numbers don't shift
        let (_tmp, path) = tmp_file("test.rs", b"line1\nline2\nline3\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        let diff = UnifiedDiff {
            lines: vec![
                UnifiedDiffLine::HunkHeader("@@ -1,3 +1,3 @@".to_string()),
                UnifiedDiffLine::Context("line1".to_string()),
                UnifiedDiffLine::Removed("line2".to_string()),
                UnifiedDiffLine::Added("LINE2".to_string()),
                UnifiedDiffLine::Context("line3".to_string()),
            ],
        };
        viewer.set_diff(Some(crate::diff::LineDiff { lines: vec![crate::diff::DiffLineKind::Unchanged; 3] }), Some(diff));

        let area = Rect::new(0, 0, 60, 10);
        let mut buf_preview = Buffer::empty(area);
        viewer.diff.diff_mode = false;
        viewer.render_to_buffer(area, &mut buf_preview, true, &empty_ctx());

        let mut buf_diff = Buffer::empty(area);
        viewer.diff.diff_mode = true;
        viewer.scroll_offset = 0;
        viewer.render_to_buffer(area, &mut buf_diff, true, &empty_ctx());

        // Find where "1" line number starts in preview (first row, inner y=1)
        let preview_row: String = (0..15).map(|x| buf_preview[(x, 1)].symbol().to_string()).collect();
        let diff_row: String = (0..15).map(|x| buf_diff[(x, 1)].symbol().to_string()).collect();

        // Find the column of the first digit "1" in each row
        let preview_num_col = preview_row.find('1').expect("Preview should show line number 1");
        let diff_num_col = diff_row.find('1').expect("Diff should show line number 1");

        assert_eq!(
            preview_num_col, diff_num_col,
            "Line number column should be the same in both modes.\n  Preview: '{preview_row}'\n  Diff:    '{diff_row}'"
        );
    }

    #[test]
    fn diff_mode_total_lines_excludes_hunk_headers() {
        let (_tmp, path) = tmp_file("test.rs", b"line1\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        let diff = UnifiedDiff {
            lines: vec![
                UnifiedDiffLine::HunkHeader("@@ -1,1 +1,1 @@".to_string()),
                UnifiedDiffLine::Context("line1".to_string()),
            ],
        };
        viewer.set_diff(None, Some(diff));
        viewer.diff.diff_mode = true;

        // total_lines should only count non-hunk-header lines
        assert_eq!(viewer.total_lines(), 1);
    }

    #[test]
    fn diff_highlighted_lines_cleared_on_load_file() {
        let (_tmp, path) = tmp_file("test.rs", b"fn main() {}\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        let diff = UnifiedDiff {
            lines: vec![UnifiedDiffLine::Context("fn main() {}".to_string())],
        };
        viewer.set_diff(None, Some(diff));
        viewer.ensure_diff_highlighted();
        assert!(!viewer.diff.diff_highlighted_lines.is_empty());

        // Reload a file — diff highlights should be cleared
        viewer.load_file(&path);
        assert!(viewer.diff.diff_highlighted_lines.is_empty());
    }

    // Horizontal scroll tests
    #[test]
    fn h_scroll_defaults_to_zero() {
        let viewer = FileViewer::new();
        assert_eq!(viewer.h_scroll, 0);
    }

    #[test]
    fn h_scroll_resets_on_load_file() {
        let (_tmp, path) = tmp_file("test.rs", b"line1\nline2");
        let mut viewer = FileViewer::new();
        viewer.h_scroll = 8;
        viewer.load_file(&path);
        assert_eq!(viewer.h_scroll, 0);
    }

    #[test]
    fn scroll_right_increases_h_scroll() {
        let (_tmp, path) = tmp_file("wide.txt", b"a]234567890123456789012345678901234567890");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.scroll_right();
        assert_eq!(viewer.h_scroll, H_SCROLL_AMOUNT);
    }

    #[test]
    fn scroll_right_stops_when_longest_line_at_pane_edge() {
        // 30-char line, content width 10 → max = 30 + H_SCROLL_PADDING - 10 = 24
        let (_tmp, path) = tmp_file("wide.txt", b"123456789012345678901234567890");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.visible_content_width = 10;

        for _ in 0..100 {
            viewer.scroll_right();
        }
        assert_eq!(viewer.h_scroll, 30 + H_SCROLL_PADDING - 10);
    }

    #[test]
    fn scroll_right_no_scroll_when_content_fits_viewport() {
        // 5-char line + padding = 9, content width 20 → content fits, no scrolling
        let (_tmp, path) = tmp_file("short.txt", b"abcde\nhi");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.visible_content_width = 20;

        viewer.scroll_right();
        assert_eq!(viewer.h_scroll, 0);
    }

    #[test]
    fn scroll_right_no_file_loaded_stays_zero() {
        let mut viewer = FileViewer::new();
        viewer.scroll_right();
        assert_eq!(viewer.h_scroll, 0);
    }

    #[test]
    fn scroll_left_decreases_h_scroll() {
        let mut viewer = FileViewer::new();
        viewer.h_scroll = 8;
        viewer.scroll_left();
        assert_eq!(viewer.h_scroll, 4);
    }

    #[test]
    fn scroll_left_floors_at_zero() {
        let mut viewer = FileViewer::new();
        viewer.h_scroll = 2;
        viewer.scroll_left();
        assert_eq!(viewer.h_scroll, 0);
    }

    #[test]
    fn h_key_scrolls_left_in_viewer() {
        let mut viewer = FileViewer::new();
        viewer.h_scroll = 8;
        viewer.handle_event(key(KeyCode::Char('h'))).unwrap();
        assert_eq!(viewer.h_scroll, 4);
    }

    #[test]
    fn l_key_scrolls_right_in_viewer() {
        let (_tmp, path) = tmp_file("wide.txt", b"abcdefghijklmnopqrstuvwxyz");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.handle_event(key(KeyCode::Char('l'))).unwrap();
        assert_eq!(viewer.h_scroll, H_SCROLL_AMOUNT);
    }

    #[test]
    fn left_arrow_moves_cursor_col_left() {
        let (_tmp, path) = tmp_file("test.txt", b"hello");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.cursor_col = Some(3);
        viewer.handle_event(key(KeyCode::Left)).unwrap();
        assert_eq!(viewer.cursor_col, Some(2));
    }

    #[test]
    fn left_arrow_clamps_at_zero() {
        let mut viewer = FileViewer::new();
        viewer.cursor_col = Some(0);
        viewer.handle_event(key(KeyCode::Left)).unwrap();
        assert_eq!(viewer.cursor_col, Some(0));
    }

    #[test]
    fn right_arrow_moves_cursor_col_right() {
        let (_tmp, path) = tmp_file("test.txt", b"hello");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.cursor_col = Some(2);
        viewer.handle_event(key(KeyCode::Right)).unwrap();
        assert_eq!(viewer.cursor_col, Some(3));
    }

    #[test]
    fn right_arrow_clamps_at_line_length() {
        let (_tmp, path) = tmp_file("test.txt", b"abc");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.cursor_col = Some(3); // already at end
        viewer.handle_event(key(KeyCode::Right)).unwrap();
        assert_eq!(viewer.cursor_col, Some(3));
    }

    // Comment navigation tests
    fn make_comment(path: &Path, start: usize, end: usize) -> Comment {
        Comment {
            file: path.to_path_buf(),
            start_line: start,
            end_line: end,
            text: "test comment".into(),
            code_context: vec![],
        }
    }

    #[test]
    fn cursor_down_stops_on_comment_row() {
        let content: Vec<u8> = (0..10).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        // Add comment ending at line 3 (1-indexed)
        let comments = vec![(make_comment(&path, 3, 3), false)];
        viewer.cursor_line = 1; // 0-indexed, line 2 in 1-indexed

        // Move down: cursor_line is 1, comment ends at line 2 (cursor_line+1=2, but
        // comment_at_end_line checks cursor_line+1 in 1-indexed which is 2... let's think:
        // cursor_line=1 → current 1-indexed line is 2. Next 1-indexed line would be 3.
        // comment_at_end_line(cursor_line+1) = comment_at_end_line(2) → no match (comment end_line=3)
        // So cursor moves to cursor_line=2.
        viewer.cursor_down(&comments);
        assert_eq!(viewer.cursor_line, 2);
        assert!(viewer.cursor_on_comment.is_none());

        // Move down again: cursor_line=2 → comment_at_end_line(cursor_line+1=3) → match!
        viewer.cursor_down(&comments);
        assert_eq!(viewer.cursor_line, 2); // stays on code line above comment
        assert_eq!(viewer.cursor_on_comment, Some((3, 3)));
    }

    #[test]
    fn cursor_down_from_comment_moves_to_next_code_line() {
        let content: Vec<u8> = (0..10).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        let comments = vec![(make_comment(&path, 3, 3), false)];
        viewer.cursor_line = 2; // 0-indexed
        viewer.cursor_on_comment = Some((3, 3));

        // Move down from comment → next code line
        viewer.cursor_down(&comments);
        assert!(viewer.cursor_on_comment.is_none());
        assert_eq!(viewer.cursor_line, 3); // 0-indexed, line 4 in 1-indexed
    }

    #[test]
    fn cursor_up_stops_on_comment_row() {
        let content: Vec<u8> = (0..10).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        let comments = vec![(make_comment(&path, 3, 3), false)];
        viewer.cursor_line = 3; // 0-indexed, line 4 in 1-indexed

        // Move up: comment_at_end_line(cursor_line=3) → match (comment end_line=3)
        viewer.cursor_up(&comments);
        assert_eq!(viewer.cursor_line, 2);
        assert_eq!(viewer.cursor_on_comment, Some((3, 3)));
    }

    #[test]
    fn cursor_up_from_comment_moves_to_code_line_above() {
        let content: Vec<u8> = (0..10).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        let comments = vec![(make_comment(&path, 3, 3), false)];
        viewer.cursor_line = 2;
        viewer.cursor_on_comment = Some((3, 3));

        // Move up from comment → code line above (cursor_line stays at 2)
        viewer.cursor_up(&comments);
        assert!(viewer.cursor_on_comment.is_none());
        assert_eq!(viewer.cursor_line, 2);
    }

    #[test]
    fn ctrl_d_clears_cursor_on_comment() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.visible_height = 20;
        viewer.load_file(&path);
        viewer.cursor_on_comment = Some((3, 3));

        viewer.handle_event(ctrl_key('d')).unwrap();
        assert!(viewer.cursor_on_comment.is_none());
    }

    #[test]
    fn ctrl_u_clears_cursor_on_comment() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.visible_height = 20;
        viewer.load_file(&path);
        viewer.cursor_line = 30;
        viewer.scroll_offset = 20;
        viewer.cursor_on_comment = Some((25, 25));

        viewer.handle_event(ctrl_key('u')).unwrap();
        assert!(viewer.cursor_on_comment.is_none());
    }

    #[test]
    fn load_file_clears_cursor_on_comment() {
        let (_tmp, path) = tmp_file("test.txt", b"line1\nline2");
        let mut viewer = FileViewer::new();
        viewer.cursor_on_comment = Some((1, 1));
        viewer.load_file(&path);
        assert!(viewer.cursor_on_comment.is_none());
    }

    // ── cursor_file_line / diff integration tests ──

    fn make_diff_viewer(diff_lines: Vec<UnifiedDiffLine>) -> FileViewer {
        let mut viewer = FileViewer::new();
        let diff = UnifiedDiff { lines: diff_lines };
        viewer.diff.unified_diff = Some(diff);
        viewer.diff.compute_line_numbers();
        viewer
    }

    #[test]
    fn cursor_file_line_normal_mode() {
        let content: Vec<u8> = (0..5).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.cursor_line = 0;
        assert_eq!(viewer.cursor_file_line(), Some(1));
        viewer.cursor_line = 3;
        assert_eq!(viewer.cursor_file_line(), Some(4));
    }

    #[test]
    fn cursor_file_line_diff_mode_context_line() {
        let mut viewer = make_diff_viewer(vec![
            UnifiedDiffLine::Context("a".into()),
            UnifiedDiffLine::Added("b".into()),
            UnifiedDiffLine::Removed("c".into()),
        ]);
        viewer.diff.diff_mode = true;
        viewer.cursor_line = 0;
        assert_eq!(viewer.cursor_file_line(), Some(1));
        viewer.cursor_line = 1;
        assert_eq!(viewer.cursor_file_line(), Some(2));
    }

    #[test]
    fn cursor_file_line_diff_mode_removed_line_is_none() {
        let mut viewer = make_diff_viewer(vec![
            UnifiedDiffLine::Context("a".into()),
            UnifiedDiffLine::Removed("old".into()),
            UnifiedDiffLine::Added("new".into()),
        ]);
        viewer.diff.diff_mode = true;
        viewer.cursor_line = 1; // Removed line
        assert_eq!(viewer.cursor_file_line(), None);
    }

    #[test]
    fn diff_mode_cursor_down_skips_removed_for_comments() {
        // Setup: diff with Context, Removed, Added, Context
        // Comment at file line 1 (Context line at disp_idx=0)
        let content: Vec<u8> = b"ctx1\nnew\nctx2\n".to_vec();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.diff.diff_mode = true;
        viewer.diff.unified_diff = Some(UnifiedDiff {
            lines: vec![
                UnifiedDiffLine::Context("ctx1".into()),  // disp 0, file 1
                UnifiedDiffLine::Removed("old".into()),   // disp 1, file None
                UnifiedDiffLine::Added("new".into()),      // disp 2, file 2
                UnifiedDiffLine::Context("ctx2".into()),  // disp 3, file 3
            ],
        });
        viewer.diff.compute_line_numbers();
        let comments = vec![(make_comment(&path, 1, 1), false)]; // comment at file line 1

        viewer.cursor_line = 0; // on Context line (file line 1)
        // cursor_down should stop on comment (comment ends at file line 1)
        viewer.cursor_down(&comments);
        assert_eq!(viewer.cursor_on_comment, Some((1, 1)));
        assert_eq!(viewer.cursor_line, 0);

        // cursor_down from comment → next display line
        viewer.cursor_down(&comments);
        assert!(viewer.cursor_on_comment.is_none());
        assert_eq!(viewer.cursor_line, 1); // Removed line

        // cursor_down on Removed line (None) → just advance, no comment stop
        viewer.cursor_down(&comments);
        assert!(viewer.cursor_on_comment.is_none());
        assert_eq!(viewer.cursor_line, 2); // Added line
    }

    #[test]
    fn diff_mode_cursor_up_stops_on_comment() {
        let content: Vec<u8> = b"ctx1\nnew\nctx2\n".to_vec();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.diff.diff_mode = true;
        viewer.diff.unified_diff = Some(UnifiedDiff {
            lines: vec![
                UnifiedDiffLine::Context("ctx1".into()),  // disp 0, file 1
                UnifiedDiffLine::Added("new".into()),      // disp 1, file 2
                UnifiedDiffLine::Context("ctx2".into()),  // disp 2, file 3
            ],
        });
        viewer.diff.compute_line_numbers();
        let comments = vec![(make_comment(&path, 1, 1), false)]; // comment at file line 1

        viewer.cursor_line = 1; // on Added line (file line 2)
        // cursor_up: previous line (disp 0) has file line 1, comment ends there → stop
        viewer.cursor_up(&comments);
        assert_eq!(viewer.cursor_on_comment, Some((1, 1)));
        assert_eq!(viewer.cursor_line, 0);

        // cursor_up from comment → stays at code line 0
        viewer.cursor_up(&comments);
        assert!(viewer.cursor_on_comment.is_none());
        assert_eq!(viewer.cursor_line, 0);
    }

    // --- Tests for click-to-focus ---

    #[test]
    fn click_line_moves_cursor_to_clicked_row() {
        let content: Vec<u8> = (0..20).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.visible_height = 10;
        viewer.scroll_offset = 0;
        // Simulate content_rect as if rendered at (5, 2) with 80x10
        viewer.content_rect = Some(Rect::new(5, 2, 80, 10));

        // Click on row 5 (inner row = 5 - 2 = 3, target = 0 + 3 = 3)
        assert!(viewer.click_line(5, 10));
        assert_eq!(viewer.cursor_line, 3);
        assert!(viewer.cursor_on_comment.is_none());
    }

    #[test]
    fn click_line_with_scroll_offset() {
        let content: Vec<u8> = (0..20).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.visible_height = 10;
        viewer.scroll_offset = 5;
        viewer.content_rect = Some(Rect::new(5, 2, 80, 10));

        // Click on row 4 (inner row = 4 - 2 = 2, target = 5 + 2 = 7)
        assert!(viewer.click_line(4, 10));
        assert_eq!(viewer.cursor_line, 7);
    }

    #[test]
    fn click_line_clamps_to_max_line() {
        let content: Vec<u8> = (0..5).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.visible_height = 10;
        viewer.scroll_offset = 0;
        viewer.content_rect = Some(Rect::new(0, 0, 80, 10));

        // Click on row 8 → target = 8 but only 5 lines → clamp to 4
        assert!(viewer.click_line(8, 10));
        assert_eq!(viewer.cursor_line, 4);
    }

    #[test]
    fn click_line_outside_content_rect_returns_false() {
        let mut viewer = FileViewer::new();
        viewer.content_rect = Some(Rect::new(5, 2, 80, 10));

        // Click above content area
        assert!(!viewer.click_line(1, 10));
        // Click left of content area
        assert!(!viewer.click_line(5, 3));
    }

    #[test]
    fn click_line_without_content_rect_returns_false() {
        let mut viewer = FileViewer::new();
        assert!(!viewer.click_line(5, 10));
    }

    #[test]
    fn click_line_accounts_for_comment_rows() {
        let content: Vec<u8> = (0..10).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.visible_height = 20;
        viewer.scroll_offset = 0;
        viewer.content_rect = Some(Rect::new(0, 0, 80, 20));

        // Simulate row_map as if line 2 (0-indexed) has a 2-row comment block after it:
        // row 0 → CodeLine(0)
        // row 1 → CodeLine(1)
        // row 2 → CodeLine(2)
        // row 3 → CommentRow(2, 3) ← comment header
        // row 4 → CommentRow(2, 3) ← comment separator
        // row 5 → CodeLine(3)
        // row 6 → CodeLine(4)
        viewer.row_map = vec![
            VisualRowContent::CodeLine(0),
            VisualRowContent::CodeLine(1),
            VisualRowContent::CodeLine(2),
            VisualRowContent::CommentRow(2, 3),
            VisualRowContent::CommentRow(2, 3),
            VisualRowContent::CodeLine(3),
            VisualRowContent::CodeLine(4),
        ];

        // Click on visual row 5 → should land on CodeLine(3), not CodeLine(5)
        assert!(viewer.click_line(5, 10));
        assert_eq!(viewer.cursor_line, 3);
        assert!(viewer.cursor_on_comment.is_none());

        // Click on visual row 3 (comment header) → should set cursor_on_comment
        assert!(viewer.click_line(3, 10));
        assert_eq!(viewer.cursor_line, 2); // end_line(3) - 1 = 2, 0-indexed
        assert_eq!(viewer.cursor_on_comment, Some((2, 3)));

        // Click on visual row 0 → CodeLine(0)
        assert!(viewer.click_line(0, 10));
        assert_eq!(viewer.cursor_line, 0);
        assert!(viewer.cursor_on_comment.is_none());

        // Click beyond row_map → fallback to last code line
        assert!(viewer.click_line(15, 10));
        assert_eq!(viewer.cursor_line, 9); // 10 lines, max = 9
        assert!(viewer.cursor_on_comment.is_none());
    }

    // --- Tests for scroll position preservation on diff/preview toggle ---

    #[test]
    fn toggle_preview_to_diff_preserves_position() {
        let content: Vec<u8> = (0..20).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.visible_height = 10;

        // Diff: all context lines (1:1 mapping with file lines)
        let diff_lines: Vec<UnifiedDiffLine> = (0..20)
            .map(|i| UnifiedDiffLine::Context(format!("line {i}")))
            .collect();
        viewer.set_diff(None, Some(UnifiedDiff { lines: diff_lines }));

        // Position cursor at line 12 (0-indexed), scroll_offset = 8
        viewer.cursor_line = 12;
        viewer.scroll_offset = 8;
        // screen_row = 12 - 8 = 4

        // Toggle to diff mode
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        let _ = viewer.handle_event(key);

        assert!(viewer.diff.diff_mode);
        // File line was 13 (1-indexed), diff display index should be 12 (0-indexed)
        assert_eq!(viewer.cursor_line, 12);
        // screen_row preserved: scroll_offset = cursor_line - screen_row = 12 - 4 = 8
        assert_eq!(viewer.scroll_offset, 8);
    }

    #[test]
    fn toggle_diff_to_preview_preserves_position() {
        let content: Vec<u8> = (0..20).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.visible_height = 10;

        let diff_lines: Vec<UnifiedDiffLine> = (0..20)
            .map(|i| UnifiedDiffLine::Context(format!("line {i}")))
            .collect();
        viewer.set_diff(None, Some(UnifiedDiff { lines: diff_lines }));

        // Start in diff mode
        viewer.diff.diff_mode = true;
        viewer.diff.compute_line_numbers();
        viewer.cursor_line = 15;
        viewer.scroll_offset = 11;
        // screen_row = 15 - 11 = 4

        // Toggle to preview
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        let _ = viewer.handle_event(key);

        assert!(!viewer.diff.diff_mode);
        // diff display 15 → file line 16 → preview index 15
        assert_eq!(viewer.cursor_line, 15);
        assert_eq!(viewer.scroll_offset, 11);
    }

    #[test]
    fn toggle_from_removed_line_preserves_position() {
        let content: Vec<u8> = b"ctx\nnew\nctx2\n".to_vec();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.visible_height = 10;

        viewer.diff.diff_mode = true;
        viewer.diff.unified_diff = Some(UnifiedDiff {
            lines: vec![
                UnifiedDiffLine::Context("ctx".into()),   // disp 0, file 1
                UnifiedDiffLine::Removed("old".into()),    // disp 1, file None
                UnifiedDiffLine::Added("new".into()),       // disp 2, file 2
                UnifiedDiffLine::Context("ctx2".into()),  // disp 3, file 3
            ],
        });
        viewer.diff.compute_line_numbers();
        viewer.cursor_line = 1; // Removed line
        viewer.scroll_offset = 0;

        // Toggle to preview
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        let _ = viewer.handle_event(key);

        assert!(!viewer.diff.diff_mode);
        // Resolved nearest file line is 2 (Added below), preview index = 1
        assert_eq!(viewer.cursor_line, 1);
    }

    #[test]
    fn toggle_clamps_scroll_offset() {
        let content: Vec<u8> = (0..5).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.visible_height = 10;

        // Diff with extra Removed lines (more total lines than preview)
        viewer.diff.diff_mode = true;
        let mut diff_lines: Vec<UnifiedDiffLine> = Vec::new();
        for i in 0..5 {
            diff_lines.push(UnifiedDiffLine::Removed(format!("old {i}")));
            diff_lines.push(UnifiedDiffLine::Added(format!("line {i}")));
        }
        viewer.diff.unified_diff = Some(UnifiedDiff { lines: diff_lines });
        viewer.diff.compute_line_numbers();
        // 10 display lines, cursor at 8, scroll at 5 → screen_row = 3
        viewer.cursor_line = 8;
        viewer.scroll_offset = 5;

        // Toggle to preview (only 5 lines → max_scroll might be 0)
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        let _ = viewer.handle_event(key);

        assert!(!viewer.diff.diff_mode);
        // scroll_offset should be clamped to max_scroll
        assert!(viewer.scroll_offset <= viewer.max_scroll());
        assert!(viewer.cursor_line < viewer.total_lines());
    }

    #[test]
    fn range_comment_navigation_round_trip() {
        let content: Vec<u8> = (0..10).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("test.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        // Range comment L2-L5 (1-indexed), renders after end_line=5
        let comments = vec![(make_comment(&path, 2, 5), false)];
        viewer.cursor_line = 3; // 0-indexed, 1-indexed line 4

        // Move down: no comment ends at line 4, so cursor moves to cursor_line=4 (line 5)
        viewer.cursor_down(&comments);
        assert!(viewer.cursor_on_comment.is_none());
        assert_eq!(viewer.cursor_line, 4);

        // Move down again: comment ends at line 5 → stops on comment
        viewer.cursor_down(&comments);
        assert_eq!(viewer.cursor_on_comment, Some((2, 5)));
        assert_eq!(viewer.cursor_line, 4);

        // Move down from comment → next code line (end_line=5, 0-indexed=5)
        viewer.cursor_down(&comments);
        assert!(viewer.cursor_on_comment.is_none());
        assert_eq!(viewer.cursor_line, 5);

        // Move back up → should stop on the comment again
        viewer.cursor_up(&comments);
        assert_eq!(viewer.cursor_on_comment, Some((2, 5)));
        assert_eq!(viewer.cursor_line, 4);

        // Move up from comment → code line above
        viewer.cursor_up(&comments);
        assert!(viewer.cursor_on_comment.is_none());
        assert_eq!(viewer.cursor_line, 4);
    }

    #[test]
    fn word_boundary_left_basic() {
        // "hello world" col 11 (end) → 6 (start of "world")
        assert_eq!(word_boundary_left("hello world", 11), 6);
        // col 6 → 0 (start of "hello")
        assert_eq!(word_boundary_left("hello world", 6), 0);
        // col 0 stays 0
        assert_eq!(word_boundary_left("hello world", 0), 0);
    }

    #[test]
    fn word_boundary_left_underscore_word() {
        // "foo_bar baz" col 8 → 8 (skips no whitespace, goes back over "baz" to 8)
        assert_eq!(word_boundary_left("foo_bar baz", 11), 8);
        // col 7 (space) → 0 (skips space, goes back over "foo_bar")
        assert_eq!(word_boundary_left("foo_bar baz", 7), 0);
    }

    #[test]
    fn word_boundary_left_punctuation() {
        // "a::b" col 3 → 2 (skips "b" as word), col 2 → 1 (skips "::" as punctuation)
        assert_eq!(word_boundary_left("a::b", 4), 3);
        assert_eq!(word_boundary_left("a::b", 3), 1);
        assert_eq!(word_boundary_left("a::b", 1), 0);
    }

    #[test]
    fn word_boundary_right_basic() {
        // "hello world" col 0 → 6 (skip "hello" then space)
        assert_eq!(word_boundary_right("hello world", 0), 6);
        // col 6 → 11 (skip "world")
        assert_eq!(word_boundary_right("hello world", 6), 11);
        // col 11 (end) stays 11
        assert_eq!(word_boundary_right("hello world", 11), 11);
    }

    #[test]
    fn word_boundary_right_punctuation() {
        // "a::b" col 0 → 1 (skip "a"), col 1 → 3 (skip "::"), col 3 → 4 (skip "b")
        assert_eq!(word_boundary_right("a::b", 0), 1);
        assert_eq!(word_boundary_right("a::b", 1), 3);
        assert_eq!(word_boundary_right("a::b", 3), 4);
    }

    #[test]
    fn word_boundary_empty_string() {
        assert_eq!(word_boundary_left("", 0), 0);
        assert_eq!(word_boundary_right("", 0), 0);
    }

}
