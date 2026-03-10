use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use syntect::highlighting::HighlightState;
use syntect::parsing::ParseState;

use crate::comments::Comment;
use crate::components::{Action, Component};

use crate::diff::{DiffLineKind, LineDiff, UnifiedDiff, UnifiedDiffLine};
use crate::highlight::Highlighter;

/// Columns to scroll per horizontal scroll tick.
pub const H_SCROLL_AMOUNT: usize = 4;

/// Extra padding (in columns) added beyond the longest line for horizontal scroll.
const H_SCROLL_PADDING: usize = 2;
/// Extra padding (in lines) added beyond the last line for vertical scroll.
const V_SCROLL_PADDING: usize = 1;

/// Minimum inner width (in columns) below which the minimap is hidden.
const MINIMAP_MIN_WIDTH: u16 = 30;
/// Width of the minimap in terminal columns.
const MINIMAP_WIDTH: u16 = 2;

/// Color of a minimap marker for a given row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MinimapMarker {
    Added,
    Modified,
    Removed,
    Comment,
}

/// Content to display in the file viewer.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewerContent {
    /// No file selected yet.
    Placeholder,
    /// File loaded successfully.
    File {
        path: PathBuf,
        lines: Vec<String>,
        /// Name of the detected syntax (e.g., "Rust", "Python", "Plain Text").
        /// Stored as a String to avoid lifetime issues with SyntaxReference.
        syntax_name: String,
    },
    /// Binary file detected.
    Binary(PathBuf),
    /// Empty file.
    Empty(PathBuf),
    /// Error reading file.
    Error(String),
}

pub struct FileViewer {
    pub content: ViewerContent,
    pub scroll_offset: usize,
    /// Height of the viewer (set during render, used for half-page scroll).
    pub visible_height: usize,
    /// Cursor line position within the file (0-indexed).
    pub cursor_line: usize,
    /// Horizontal scroll offset in characters (0 = no horizontal scroll).
    pub h_scroll: usize,
    /// Width available for code content in characters (set during render, excludes gutter).
    pub visible_content_width: usize,
    highlighter: Highlighter,
    /// Incrementally-computed highlighted spans for file lines.
    /// May be shorter than total lines; remaining lines are computed on demand
    /// during render via `ensure_highlighted_up_to`.
    highlighted_lines: Vec<Vec<Span<'static>>>,
    /// Saved syntect state after the last highlighted line, enabling incremental
    /// highlighting without replaying from the beginning.
    hl_parse_state: Option<ParseState>,
    hl_highlight_state: Option<HighlightState>,
    /// Per-line diff info for gutter markers in normal mode.
    pub line_diff: Option<LineDiff>,
    /// Parsed unified diff for diff mode.
    pub unified_diff: Option<UnifiedDiff>,
    /// Pre-computed syntax-highlighted spans for each unified diff line.
    pub diff_highlighted_lines: Vec<Vec<Span<'static>>>,
    /// Whether the viewer is currently in diff mode.
    pub diff_mode: bool,
    /// Comments for the currently viewed file (set before each render by App).
    pub comments: Vec<Comment>,
    /// Cached minimap rectangle from the last render (for mouse hit-testing).
    pub minimap_rect: Option<Rect>,
}

impl FileViewer {
    pub fn new() -> Self {
        Self {
            content: ViewerContent::Placeholder,
            scroll_offset: 0,
            visible_height: 20,
            cursor_line: 0,
            h_scroll: 0,
            visible_content_width: 0,
            highlighter: Highlighter::new(),
            highlighted_lines: Vec::new(),
            hl_parse_state: None,
            hl_highlight_state: None,
            line_diff: None,
            unified_diff: None,
            diff_highlighted_lines: Vec::new(),
            diff_mode: false,
            comments: Vec::new(),
            minimap_rect: None,
        }
    }

    /// Set diff data for the currently loaded file.
    ///
    /// Syntax highlighting for diff lines is deferred until diff mode is
    /// actually rendered (see `ensure_diff_highlighted`), avoiding expensive
    /// O(n) syntect work on every file selection.
    pub fn set_diff(&mut self, line_diff: Option<LineDiff>, unified_diff: Option<UnifiedDiff>) {
        self.line_diff = line_diff;
        self.diff_highlighted_lines.clear();
        self.unified_diff = unified_diff;
    }

    /// Lazily compute syntax-highlighted spans for unified diff lines.
    ///
    /// Called on the first diff-mode render after `set_diff`. Subsequent
    /// calls are no-ops while the cached highlights remain valid.
    fn ensure_diff_highlighted(&mut self) {
        if !self.diff_highlighted_lines.is_empty() {
            return;
        }
        let diff = match self.unified_diff {
            Some(ref d) => d,
            None => return,
        };

        let syntax = match &self.content {
            ViewerContent::File { path, .. } => {
                let first_line = diff.lines.iter().find_map(|l| match l {
                    UnifiedDiffLine::Context(s) | UnifiedDiffLine::Added(s) => Some(s.as_str()),
                    _ => None,
                }).unwrap_or("");
                self.highlighter.detect_syntax(path, first_line).name.clone()
            }
            _ => "Plain Text".to_string(),
        };

        let syntax_ref = self.highlighter.syntax_set
            .find_syntax_by_name(&syntax)
            .unwrap_or_else(|| self.highlighter.syntax_set.find_syntax_plain_text());
        let mut hl_state = self.highlighter.new_highlight_state(syntax_ref);

        for diff_line in &diff.lines {
            match diff_line {
                UnifiedDiffLine::HunkHeader(_) => {
                    self.diff_highlighted_lines.push(Vec::new());
                }
                UnifiedDiffLine::Context(s)
                | UnifiedDiffLine::Added(s)
                | UnifiedDiffLine::Removed(s) => {
                    let spans = self.highlighter.highlight_line(
                        &mut hl_state,
                        &format!("{s}\n"),
                    );
                    self.diff_highlighted_lines.push(spans);
                }
            }
        }
    }

    /// Incrementally compute syntax highlighting up to (exclusive) the given line index.
    ///
    /// Syntect is stateful — each line's highlighting depends on all preceding
    /// lines. We cache the `ParseState` and `HighlightState` after the last
    /// highlighted line so that subsequent calls resume in O(new_lines) rather
    /// than replaying from line 0.
    fn ensure_highlighted_up_to(&mut self, up_to: usize) {
        let (lines, syntax_name) = match &self.content {
            ViewerContent::File {
                lines, syntax_name, ..
            } => (lines, syntax_name.clone()),
            _ => return,
        };

        let already = self.highlighted_lines.len();
        let target = up_to.min(lines.len());
        if already >= target {
            return;
        }

        let syntax_ref = self
            .highlighter
            .syntax_set
            .find_syntax_by_name(&syntax_name)
            .unwrap_or_else(|| self.highlighter.syntax_set.find_syntax_plain_text());

        // Restore saved state or create fresh state for the first call.
        let mut hl_lines = match (self.hl_highlight_state.take(), self.hl_parse_state.take()) {
            (Some(hs), Some(ps)) => {
                syntect::easy::HighlightLines::from_state(&self.highlighter.theme, hs, ps)
            }
            _ => self.highlighter.new_highlight_state(syntax_ref),
        };

        // Highlight only the new lines.
        self.highlighted_lines.reserve(target - already);
        for line in &lines[already..target] {
            let spans = self
                .highlighter
                .highlight_line(&mut hl_lines, &format!("{line}\n"));
            self.highlighted_lines.push(spans);
        }

        // Save state for the next incremental call via `state(self)`.
        let (hs, ps) = hl_lines.state();
        self.hl_highlight_state = Some(hs);
        self.hl_parse_state = Some(ps);
    }

    /// Load a file into the viewer.
    pub fn load_file(&mut self, path: &Path) {
        self.scroll_offset = 0;
        self.cursor_line = 0;
        self.h_scroll = 0;
        self.diff_mode = false;
        self.line_diff = None;
        self.unified_diff = None;
        self.highlighted_lines.clear();
        self.hl_parse_state = None;
        self.hl_highlight_state = None;
        self.diff_highlighted_lines.clear();

        // Try to read the file
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.content = ViewerContent::Error(format!(
                    "{} — File has been deleted from disk",
                    path.display(),
                ));
                return;
            }
            Err(e) => {
                self.content = ViewerContent::Error(format!(
                    "Cannot read {}: {}",
                    path.display(),
                    e
                ));
                return;
            }
        };

        // Check for binary (null bytes in first 512 bytes)
        if is_binary(&bytes) {
            self.content = ViewerContent::Binary(path.to_path_buf());
            return;
        }

        // Convert to string
        let text = String::from_utf8_lossy(&bytes);
        if text.is_empty() {
            self.content = ViewerContent::Empty(path.to_path_buf());
            return;
        }

        let lines: Vec<String> = text.lines().map(String::from).collect();
        let first_line = lines.first().map(|s| s.as_str()).unwrap_or("");
        let syntax = self.highlighter.detect_syntax(path, first_line);
        let syntax_name = syntax.name.clone();

        // Highlighting is deferred: `ensure_highlighted_up_to` computes spans
        // incrementally during render, keeping file selection snappy.

        self.content = ViewerContent::File {
            path: path.to_path_buf(),
            lines,
            syntax_name,
        };
    }

    fn total_lines(&self) -> usize {
        if self.diff_mode {
            return self.unified_diff.as_ref().map_or(0, |d| {
                d.lines.iter().filter(|l| !matches!(l, UnifiedDiffLine::HunkHeader(_))).count()
            });
        }
        match &self.content {
            ViewerContent::File { lines, .. } => lines.len(),
            _ => 0,
        }
    }

    /// Find comment that ends at a given file line (1-indexed).
    fn comment_at_end_line(&self, line: usize) -> Option<&Comment> {
        self.comments.iter().find(|c| c.end_line == line)
    }

    /// Compute minimap markers for each row of the minimap.
    /// Returns a Vec of length `minimap_height` where each entry is the most
    /// important marker for lines mapped to that row (Comment > Diff).
    fn compute_minimap_markers(&self, minimap_height: usize) -> Vec<Option<MinimapMarker>> {
        let total = self.total_lines();
        if total == 0 || minimap_height == 0 {
            return vec![None; minimap_height];
        }

        let mut markers = vec![None; minimap_height];

        if self.diff_mode {
            // Diff mode: use unified diff lines (excluding hunk headers)
            if let Some(ref diff) = self.unified_diff {
                let displayable: Vec<&UnifiedDiffLine> = diff
                    .lines
                    .iter()
                    .filter(|l| !matches!(l, UnifiedDiffLine::HunkHeader(_)))
                    .collect();
                for (i, line) in displayable.iter().enumerate() {
                    let row = i * minimap_height / total.max(1);
                    if row >= minimap_height {
                        break;
                    }
                    let marker = match line {
                        UnifiedDiffLine::Added(_) => Some(MinimapMarker::Added),
                        UnifiedDiffLine::Removed(_) => Some(MinimapMarker::Removed),
                        _ => None,
                    };
                    if let Some(m) = marker {
                        // Diff markers only set if no comment marker already present
                        if markers[row] != Some(MinimapMarker::Comment) {
                            markers[row] = Some(m);
                        }
                    }
                }
            }
        } else {
            // Normal mode: use line diff data
            if let Some(ref ld) = self.line_diff {
                for line_num in 1..=total {
                    let row = (line_num - 1) * minimap_height / total;
                    if row >= minimap_height {
                        break;
                    }
                    let kind = ld.line_kind(line_num);
                    let marker = match kind {
                        DiffLineKind::Added => Some(MinimapMarker::Added),
                        DiffLineKind::Modified => Some(MinimapMarker::Modified),
                        DiffLineKind::Unchanged => None,
                    };
                    if let Some(m) = marker {
                        if markers[row] != Some(MinimapMarker::Comment) {
                            markers[row] = Some(m);
                        }
                    }
                }
            }
        }

        // Comment markers (highest priority, overwrite diff markers)
        for comment in &self.comments {
            for line_num in comment.start_line..=comment.end_line {
                let idx = if self.diff_mode {
                    // In diff mode, approximate position
                    line_num.saturating_sub(1)
                } else {
                    line_num.saturating_sub(1)
                };
                let row = idx * minimap_height / total.max(1);
                if row < minimap_height {
                    markers[row] = Some(MinimapMarker::Comment);
                }
            }
        }

        markers
    }

    /// Translate a minimap row to the corresponding file line index (0-based).
    fn minimap_row_to_line(&self, row: u16, minimap_height: u16) -> usize {
        let total = self.total_lines();
        if minimap_height == 0 || total == 0 {
            return 0;
        }
        (row as usize * total / minimap_height as usize).min(total.saturating_sub(1))
    }

    /// Scroll so that the line corresponding to a minimap row is centered in the viewport.
    pub fn scroll_to_minimap_row(&mut self, row: u16, minimap_height: u16) {
        let line = self.minimap_row_to_line(row, minimap_height);
        let half = self.visible_height / 2;
        self.scroll_offset = line.saturating_sub(half);
        self.scroll_offset = self.scroll_offset.min(self.max_scroll());
        self.cursor_line = line;
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

    pub fn render_to_buffer(&mut self, area: Rect, buf: &mut Buffer, focused: bool) {
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let title = if self.diff_mode { " Diff " } else { " Preview " };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);

        let inner = block.inner(area);
        block.render(area, buf);

        // Decide whether to show minimap: only for File content with sufficient width
        let show_minimap = matches!(self.content, ViewerContent::File { .. })
            && inner.width >= MINIMAP_MIN_WIDTH;

        let (content_area, minimap_area) = if show_minimap {
            let content_w = inner.width.saturating_sub(MINIMAP_WIDTH);
            let content = Rect::new(inner.x, inner.y, content_w, inner.height);
            let minimap = Rect::new(inner.x + content_w, inner.y, MINIMAP_WIDTH, inner.height);
            (content, Some(minimap))
        } else {
            (inner, None)
        };
        self.minimap_rect = minimap_area;

        self.visible_height = content_area.height as usize;

        // Diff mode rendering
        if self.diff_mode {
            self.render_diff_mode(content_area, buf);
            if let Some(mr) = minimap_area {
                self.render_minimap(mr, buf);
            }
            return;
        }

        match &self.content {
            ViewerContent::Placeholder => {
                self.minimap_rect = None;
                let msg = "Select a file to preview";
                let line = Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)));
                buf.set_line(inner.x, inner.y, &line, inner.width);
            }
            ViewerContent::Binary(_) => {
                self.minimap_rect = None;
                let msg = "Binary file — cannot display";
                let line = Line::from(Span::styled(msg, Style::default().fg(Color::Yellow)));
                buf.set_line(inner.x, inner.y, &line, inner.width);
            }
            ViewerContent::Empty(_) => {
                self.minimap_rect = None;
                let msg = "Empty file";
                let line = Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)));
                buf.set_line(inner.x, inner.y, &line, inner.width);
            }
            ViewerContent::Error(msg) => {
                self.minimap_rect = None;
                let line = Line::from(Span::styled(msg.as_str(), Style::default().fg(Color::Red)));
                buf.set_line(inner.x, inner.y, &line, inner.width);
            }
            ViewerContent::File { .. } => {
                // Extract line count to call ensure_highlighted_up_to outside the borrow.
                let line_count = match &self.content {
                    ViewerContent::File { lines, .. } => lines.len(),
                    _ => 0,
                };
                // Incrementally compute highlights up to the last visible line.
                let need_up_to = (self.scroll_offset + content_area.height as usize).min(line_count);
                self.ensure_highlighted_up_to(need_up_to);

                let lines = match &self.content {
                    ViewerContent::File { lines, .. } => lines,
                    _ => unreachable!(),
                };
                let gutter_width = line_number_width(lines.len());
                let has_diff = self.line_diff.is_some();
                // gutter = diff marker (1 if present) + line number digits + 1 space
                let gutter_cols = gutter_width + 1 + if has_diff { 1 } else { 0 };
                self.visible_content_width = (content_area.width as usize).saturating_sub(gutter_cols);

                let mut render_row: u16 = 0;
                let max_rows = content_area.height;

                for code_line_idx in self.scroll_offset..lines.len() {
                    if render_row >= max_rows {
                        break;
                    }

                    let line_num = code_line_idx + 1;

                    let mut spans = Vec::new();

                    // Gutter change marker
                    if has_diff {
                        let kind = self
                            .line_diff
                            .as_ref()
                            .map(|d| d.line_kind(line_num))
                            .unwrap_or(DiffLineKind::Unchanged);
                        let (marker, color) = match kind {
                            DiffLineKind::Modified => ("▎", Some(Color::Yellow)),
                            DiffLineKind::Added => ("▎", Some(Color::Green)),
                            DiffLineKind::Unchanged => (" ", None),
                        };
                        let style = color
                            .map(|c| Style::default().fg(c))
                            .unwrap_or_default();
                        spans.push(Span::styled(marker, style));
                    }

                    let num_str = format!("{:>width$} ", line_num, width = gutter_width);
                    spans.push(Span::styled(num_str, Style::default().fg(Color::DarkGray)));

                    // Use pre-computed highlight cache (O(1) per line instead of O(scroll_offset))
                    let highlighted = if code_line_idx < self.highlighted_lines.len() {
                        self.highlighted_lines[code_line_idx].clone()
                    } else {
                        vec![Span::raw(lines[code_line_idx].clone())]
                    };
                    if self.h_scroll > 0 {
                        spans.extend(skip_chars_in_spans(highlighted, self.h_scroll));
                    } else {
                        spans.extend(highlighted);
                    }

                    let line = Line::from(spans);

                    let y = content_area.y + render_row;
                    buf.set_line(content_area.x, y, &line, content_area.width);

                    // Highlight cursor line with subtle background
                    if focused && code_line_idx == self.cursor_line {
                        for x in content_area.x..content_area.x + content_area.width {
                            let cell = &mut buf[(x, y)];
                            cell.set_bg(Color::DarkGray);
                        }
                    }

                    render_row += 1;

                    // Render inline comment block after this line if applicable
                    if let Some(comment) = self.comment_at_end_line(line_num) {
                        if render_row < max_rows {
                            self.render_comment_block(
                                comment, content_area, buf, &mut render_row, max_rows,
                            );
                        }
                    }
                }

                // Render minimap after content
                if let Some(mr) = minimap_area {
                    self.render_minimap(mr, buf);
                }
            }
        }
    }

    /// Render an inline comment block (2 rows: header + text).
    fn render_comment_block(
        &self,
        comment: &Comment,
        inner: Rect,
        buf: &mut Buffer,
        render_row: &mut u16,
        max_rows: u16,
    ) {
        let comment_style = Style::default().fg(Color::Cyan).bg(Color::Black);

        // Row 1: range + comment text
        let range_str = if comment.start_line == comment.end_line {
            format!("  💬 L{}: {}", comment.start_line, comment.text)
        } else {
            format!(
                "  💬 L{}-{}: {}",
                comment.start_line, comment.end_line, comment.text
            )
        };

        let y = inner.y + *render_row;
        let line = Line::from(Span::styled(&range_str, comment_style));
        buf.set_line(inner.x, y, &line, inner.width);
        // Fill background for entire row
        for x in inner.x..inner.x + inner.width {
            let cell = &mut buf[(x, y)];
            cell.set_bg(Color::Black);
        }
        *render_row += 1;

        // Row 2: separator line
        if *render_row < max_rows {
            let y = inner.y + *render_row;
            let sep = "─".repeat(inner.width as usize);
            let line = Line::from(Span::styled(sep, Style::default().fg(Color::DarkGray)));
            buf.set_line(inner.x, y, &line, inner.width);
            *render_row += 1;
        }
    }

    /// Render the minimap with half-block characters for 2x vertical resolution.
    ///
    /// Column 1 (left): change markers — colored half-blocks showing where diffs/comments are.
    /// Column 2 (right): viewport indicator — shows which portion of the file is visible.
    ///
    /// Half-block rendering doubles the effective vertical resolution by using ▀ (upper half)
    /// and ▄ (lower half) characters, where each terminal row encodes two virtual rows.
    fn render_minimap(&self, area: Rect, buf: &mut Buffer) {
        let minimap_h = area.height as usize;
        let total = self.total_lines();

        // 2x virtual resolution: each terminal row maps to 2 virtual rows
        let virtual_h = minimap_h * 2;
        let markers = self.compute_minimap_markers(virtual_h);

        // Viewport range in virtual rows (ceiling division for end)
        let (vp_start, vp_end) = if total > 0 {
            let start = self.scroll_offset * virtual_h / total;
            let visible = self.visible_height.min(total);
            let end_numer = (self.scroll_offset + visible) * virtual_h;
            let end = ((end_numer + total - 1) / total).min(virtual_h);
            (start, end.max(start + 1))
        } else {
            (0, virtual_h)
        };

        let bg_dim = Color::Rgb(30, 30, 30);
        let vp_color = Color::Rgb(80, 80, 80);

        fn marker_color(m: MinimapMarker) -> Color {
            match m {
                MinimapMarker::Added => Color::Green,
                MinimapMarker::Modified => Color::Yellow,
                MinimapMarker::Removed => Color::Red,
                MinimapMarker::Comment => Color::Cyan,
            }
        }

        for row in 0..minimap_h {
            let y = area.y + row as u16;
            let vr_top = row * 2;
            let vr_bot = row * 2 + 1;

            // Column 1: change markers (half-block, 2x resolution)
            let top_m = markers.get(vr_top).copied().flatten();
            let bot_m = if vr_bot < virtual_h {
                markers.get(vr_bot).copied().flatten()
            } else {
                None
            };

            let (ch1, fg1, bg1) = match (top_m, bot_m) {
                (Some(t), Some(b)) => {
                    let tc = marker_color(t);
                    let bc = marker_color(b);
                    if tc == bc {
                        ("█", tc, bg_dim)
                    } else {
                        // ▀: fg = top pixel, bg = bottom pixel
                        ("▀", tc, bc)
                    }
                }
                (Some(t), None) => ("▀", marker_color(t), bg_dim),
                (None, Some(b)) => ("▄", marker_color(b), bg_dim),
                (None, None) => (" ", bg_dim, bg_dim),
            };
            let line1 = Line::from(Span::styled(ch1, Style::default().fg(fg1).bg(bg1)));
            buf.set_line(area.x, y, &line1, 1);

            // Column 2: viewport indicator (half-block, 2x resolution)
            if area.width > 1 {
                let top_vp = vr_top >= vp_start && vr_top < vp_end;
                let bot_vp = vr_bot >= vp_start && vr_bot < vp_end;

                let (ch2, fg2, bg2) = match (top_vp, bot_vp) {
                    (true, true) => ("█", vp_color, bg_dim),
                    (true, false) => ("▀", vp_color, bg_dim),
                    (false, true) => ("▄", vp_color, bg_dim),
                    (false, false) => (" ", bg_dim, bg_dim),
                };
                let line2 = Line::from(Span::styled(ch2, Style::default().fg(fg2).bg(bg2)));
                buf.set_line(area.x + 1, y, &line2, 1);
            }
        }
    }

    /// Render unified diff content with language syntax highlighting.
    fn render_diff_mode(&mut self, inner: Rect, buf: &mut Buffer) {
        self.ensure_diff_highlighted();
        let Some(ref diff) = self.unified_diff else {
            let msg = "No changes";
            let line = Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)));
            buf.set_line(inner.x, inner.y, &line, inner.width);
            return;
        };

        // Collect displayable lines (skip hunk headers), keeping original indices
        // for diff_highlighted_lines lookup
        let displayable: Vec<(usize, &UnifiedDiffLine)> = diff
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !matches!(l, UnifiedDiffLine::HunkHeader(_)))
            .collect();

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

        // Track new-file line number (increments for Context and Added, not for Removed)
        let mut new_lineno: usize = 0;
        // Pre-compute line numbers for all displayable lines
        let line_numbers: Vec<Option<usize>> = displayable.iter().map(|(_, l)| {
            match l {
                UnifiedDiffLine::Context(_) | UnifiedDiffLine::Added(_) => {
                    new_lineno += 1;
                    Some(new_lineno)
                }
                _ => None,
            }
        }).collect();

        for (render_idx, (disp_idx, (orig_idx, diff_line))) in displayable
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(self.visible_height)
            .enumerate()
        {
            let y = inner.y + render_idx as u16;
            if y >= inner.y + inner.height {
                break;
            }

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
            let num_str = match line_numbers[disp_idx] {
                Some(n) => format!("{:>width$} ", n, width = gutter_width),
                None => format!("{:>width$} ", "", width = gutter_width),
            };

            let mut spans = vec![
                Span::styled(prefix, prefix_style),
                Span::styled(num_str, Style::default().fg(Color::DarkGray)),
            ];

            // Use pre-computed syntax-highlighted spans if available
            if *orig_idx < self.diff_highlighted_lines.len()
                && !self.diff_highlighted_lines[*orig_idx].is_empty()
            {
                spans.extend(self.diff_highlighted_lines[*orig_idx].clone());
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

            // Apply background tint for added/removed lines
            match diff_line {
                UnifiedDiffLine::Added(_) => {
                    for x in inner.x..inner.x + inner.width {
                        let cell = &mut buf[(x, y)];
                        cell.set_bg(Color::Rgb(0, 40, 0));
                    }
                }
                UnifiedDiffLine::Removed(_) => {
                    for x in inner.x..inner.x + inner.width {
                        let cell = &mut buf[(x, y)];
                        cell.set_bg(Color::Rgb(40, 0, 0));
                    }
                }
                _ => {}
            }
        }
    }

    /// Move cursor down by one line, auto-scrolling viewport if needed.
    pub fn cursor_down(&mut self) {
        let max = self.total_lines().saturating_sub(1);
        if self.cursor_line < max {
            self.cursor_line += 1;
            self.ensure_cursor_visible();
        }
    }

    /// Move cursor up by one line, auto-scrolling viewport if needed.
    pub fn cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.ensure_cursor_visible();
        }
    }

    /// Get the path of the currently loaded file, if any.
    pub fn current_file(&self) -> Option<&Path> {
        match &self.content {
            ViewerContent::File { path, .. } => Some(path),
            _ => None,
        }
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
                self.cursor_down();
                Ok(Action::None)
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                self.cursor_up();
                Ok(Action::None)
            }
            (KeyCode::Char('c'), KeyModifiers::NONE) => Ok(Action::StartComment),
            (KeyCode::Char('x'), KeyModifiers::NONE) => Ok(Action::DeleteComment),
            (KeyCode::Char('V'), KeyModifiers::SHIFT) => Ok(Action::StartLineSelect),
            (KeyCode::Char('e'), KeyModifiers::NONE) => Ok(Action::ExportComments),
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                // Toggle diff mode (only when unified diff data is available)
                if self.unified_diff.is_some() {
                    self.diff_mode = !self.diff_mode;
                    self.scroll_offset = 0;
                    self.cursor_line = 0;
                }
                Ok(Action::None)
            }
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, _) => {
                self.scroll_left();
                Ok(Action::None)
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Right, _) => {
                self.scroll_right();
                Ok(Action::None)
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                let half = self.visible_height / 2;
                self.scroll_down(half);
                // Move cursor with the scroll
                let max = self.total_lines().saturating_sub(1);
                self.cursor_line = (self.cursor_line + half).min(max);
                self.ensure_cursor_visible();
                Ok(Action::None)
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                let half = self.visible_height / 2;
                self.scroll_up(half);
                // Move cursor with the scroll
                self.cursor_line = self.cursor_line.saturating_sub(half);
                self.ensure_cursor_visible();
                Ok(Action::None)
            }
            (KeyCode::Tab, _) => Ok(Action::SwitchFocus),
            (KeyCode::Char('q'), KeyModifiers::NONE) => Ok(Action::Quit),
            _ => Ok(Action::None),
        }
    }
}

/// Drop the first `skip` characters across a sequence of spans, preserving styles.
/// Characters that fall entirely within skipped spans are removed; a span that is
/// partially skipped retains only the remaining suffix with its original style.
fn skip_chars_in_spans(spans: Vec<Span<'_>>, skip: usize) -> Vec<Span<'static>> {
    if skip == 0 {
        return spans
            .into_iter()
            .map(|s| Span::styled(s.content.into_owned(), s.style))
            .collect();
    }

    let mut remaining = skip;
    let mut result = Vec::new();

    for span in spans {
        let char_count = span.content.chars().count();
        if remaining >= char_count {
            remaining -= char_count;
            continue;
        }
        if remaining > 0 {
            // Find byte offset of the `remaining`-th character to avoid
            // slicing inside a multi-byte character (e.g. →, CJK).
            let byte_offset = span
                .content
                .char_indices()
                .nth(remaining)
                .map(|(i, _)| i)
                .unwrap_or(span.content.len());
            let sliced = &span.content[byte_offset..];
            result.push(Span::styled(sliced.to_owned(), span.style));
            remaining = 0;
        } else {
            result.push(Span::styled(span.content.into_owned(), span.style));
        }
    }

    result
}

/// Check if data is binary by looking for null bytes in the first 512 bytes.
fn is_binary(bytes: &[u8]) -> bool {
    let check_len = bytes.len().min(512);
    bytes[..check_len].contains(&0)
}

/// Calculate the width needed for line numbers.
fn line_number_width(total_lines: usize) -> usize {
    if total_lines == 0 {
        1
    } else {
        total_lines.to_string().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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

        viewer.handle_event(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(viewer.cursor_line, 1);
    }

    #[test]
    fn k_moves_cursor_up() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.cursor_line = 5;

        viewer.handle_event(key(KeyCode::Char('k'))).unwrap();
        assert_eq!(viewer.cursor_line, 4);
    }

    #[test]
    fn cursor_clamped_at_end() {
        let (_tmp, path) = tmp_file("short.txt", b"line1\nline2");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.cursor_line = 1; // last line

        viewer.handle_event(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(viewer.cursor_line, 1); // stays at max
    }

    #[test]
    fn cursor_clamped_at_beginning() {
        let (_tmp, path) = tmp_file("short.txt", b"line1\nline2");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        viewer.handle_event(key(KeyCode::Char('k'))).unwrap();
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
        viewer.handle_event(key(KeyCode::Char('j'))).unwrap();
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
        viewer.handle_event(key(KeyCode::Char('k'))).unwrap();
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

        viewer.handle_event(key(KeyCode::Down)).unwrap();
        assert_eq!(viewer.cursor_line, 1);

        viewer.handle_event(key(KeyCode::Up)).unwrap();
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

    // Line number width
    #[test]
    fn line_number_width_for_various_sizes() {
        assert_eq!(line_number_width(1), 1);
        assert_eq!(line_number_width(9), 1);
        assert_eq!(line_number_width(10), 2);
        assert_eq!(line_number_width(99), 2);
        assert_eq!(line_number_width(100), 3);
        assert_eq!(line_number_width(1000), 4);
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
        viewer.render_to_buffer(area, &mut buf, true);
        // Should not panic; content should be rendered
    }

    #[test]
    fn render_plain_text_file_does_not_panic() {
        let (_tmp, path) = tmp_file("notes.xyz999", b"line 1\nline 2\n");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true);
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
        viewer.render_to_buffer(area, &mut buf, true);
    }

    #[test]
    fn empty_file_unaffected_by_highlighting() {
        let (_tmp, path) = tmp_file("empty.rs", b"");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        assert!(matches!(viewer.content, ViewerContent::Empty(_)));
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true);
    }

    #[test]
    fn placeholder_unaffected_by_highlighting() {
        let mut viewer = FileViewer::new();
        assert_eq!(viewer.content, ViewerContent::Placeholder);
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, false);
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
        assert!(viewer.diff_highlighted_lines.is_empty());

        // Trigger lazy computation
        viewer.ensure_diff_highlighted();

        assert_eq!(viewer.diff_highlighted_lines.len(), 5);

        // Hunk header should have no syntax highlighting (empty spans)
        assert!(viewer.diff_highlighted_lines[0].is_empty());

        // Code lines should have syntax-highlighted spans with RGB colors
        let code_spans = &viewer.diff_highlighted_lines[1]; // "fn main() {"
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
        viewer.diff_mode = true;

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true);
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
        viewer.diff_mode = true;

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true);

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
        viewer.diff_mode = true;

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true);

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
        viewer.diff_mode = true;

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        viewer.render_to_buffer(area, &mut buf, true);

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
        viewer.diff_mode = false;
        viewer.render_to_buffer(area, &mut buf_preview, true);

        let mut buf_diff = Buffer::empty(area);
        viewer.diff_mode = true;
        viewer.scroll_offset = 0;
        viewer.render_to_buffer(area, &mut buf_diff, true);

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
        viewer.diff_mode = true;

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
        assert!(!viewer.diff_highlighted_lines.is_empty());

        // Reload a file — diff highlights should be cleared
        viewer.load_file(&path);
        assert!(viewer.diff_highlighted_lines.is_empty());
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
    fn skip_chars_in_spans_single_span() {
        let spans = vec![Span::raw("Hello World")];
        let result = skip_chars_in_spans(spans, 6);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.as_ref(), "World");
    }

    #[test]
    fn skip_chars_in_spans_multi_span_preserves_style() {
        let spans = vec![
            Span::styled("Hello", Style::default().fg(Color::Red)),
            Span::styled(" World", Style::default().fg(Color::Blue)),
        ];
        let result = skip_chars_in_spans(spans, 7);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.as_ref(), "orld");
        assert_eq!(result[0].style, Style::default().fg(Color::Blue));
    }

    #[test]
    fn skip_chars_in_spans_skip_exceeding_total_returns_empty() {
        let spans = vec![Span::raw("Hello")];
        let result = skip_chars_in_spans(spans, 10);
        assert!(result.is_empty());
    }

    #[test]
    fn skip_chars_in_spans_multibyte_chars() {
        // "→" is 3 bytes but 1 character; skip should count characters, not bytes
        let spans = vec![Span::raw(" → hello")];
        let result = skip_chars_in_spans(spans, 3);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.as_ref(), "hello");
    }

    #[test]
    fn skip_chars_in_spans_skip_zero_returns_unchanged() {
        let spans = vec![Span::raw("Hello")];
        let result = skip_chars_in_spans(spans, 0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.as_ref(), "Hello");
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
    fn left_arrow_scrolls_left_in_viewer() {
        let mut viewer = FileViewer::new();
        viewer.h_scroll = 8;
        viewer.handle_event(key(KeyCode::Left)).unwrap();
        assert_eq!(viewer.h_scroll, 4);
    }

    #[test]
    fn right_arrow_scrolls_right_in_viewer() {
        let (_tmp, path) = tmp_file("wide.txt", b"abcdefghijklmnopqrstuvwxyz");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.handle_event(key(KeyCode::Right)).unwrap();
        assert_eq!(viewer.h_scroll, H_SCROLL_AMOUNT);
    }

}
