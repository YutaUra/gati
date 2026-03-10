use std::path::{Path, PathBuf};

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

use crate::diff::{DiffLineKind, LineDiff, UnifiedDiff, UnifiedDiffLine};
use crate::highlight::Highlighter;

/// Columns to scroll per horizontal scroll tick.
pub const H_SCROLL_AMOUNT: usize = 4;

/// Extra padding (in columns) added beyond the longest line for horizontal scroll.
const H_SCROLL_PADDING: usize = 2;
/// Extra padding (in lines) added beyond the last line for vertical scroll.
const V_SCROLL_PADDING: usize = 1;

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
    /// Pre-computed highlighted spans for each line (populated on file load).
    /// Avoids re-highlighting on every render, which was O(scroll_offset).
    highlighted_lines: Vec<Vec<Span<'static>>>,
    /// Per-line diff info for gutter markers in normal mode.
    pub line_diff: Option<LineDiff>,
    /// Parsed unified diff for diff mode.
    pub unified_diff: Option<UnifiedDiff>,
    /// Whether the viewer is currently in diff mode.
    pub diff_mode: bool,
    /// Comments for the currently viewed file (set before each render by App).
    pub comments: Vec<Comment>,
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
            line_diff: None,
            unified_diff: None,
            diff_mode: false,
            comments: Vec::new(),
        }
    }

    /// Set diff data for the currently loaded file.
    pub fn set_diff(&mut self, line_diff: Option<LineDiff>, unified_diff: Option<UnifiedDiff>) {
        self.line_diff = line_diff;
        self.unified_diff = unified_diff;
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

        // Pre-compute highlighted spans for all lines so rendering is O(visible_lines)
        // instead of O(scroll_offset) on every frame.
        let mut hl_state = self.highlighter.new_highlight_state(syntax);
        self.highlighted_lines.reserve(lines.len());
        for line in &lines {
            let spans = self.highlighter.highlight_line(&mut hl_state, &format!("{line}\n"));
            self.highlighted_lines.push(spans);
        }

        self.content = ViewerContent::File {
            path: path.to_path_buf(),
            lines,
            syntax_name,
        };
    }

    fn total_lines(&self) -> usize {
        if self.diff_mode {
            return self.unified_diff.as_ref().map_or(0, |d| d.lines.len());
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

        self.visible_height = inner.height as usize;

        // Diff mode rendering
        if self.diff_mode {
            self.render_diff_mode(inner, buf);
            return;
        }

        match &self.content {
            ViewerContent::Placeholder => {
                let msg = "Select a file to preview";
                let line = Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)));
                buf.set_line(inner.x, inner.y, &line, inner.width);
            }
            ViewerContent::Binary(_) => {
                let msg = "Binary file — cannot display";
                let line = Line::from(Span::styled(msg, Style::default().fg(Color::Yellow)));
                buf.set_line(inner.x, inner.y, &line, inner.width);
            }
            ViewerContent::Empty(_) => {
                let msg = "Empty file";
                let line = Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)));
                buf.set_line(inner.x, inner.y, &line, inner.width);
            }
            ViewerContent::Error(msg) => {
                let line = Line::from(Span::styled(msg.as_str(), Style::default().fg(Color::Red)));
                buf.set_line(inner.x, inner.y, &line, inner.width);
            }
            ViewerContent::File {
                lines,
                ..
            } => {
                let gutter_width = line_number_width(lines.len());
                let has_diff = self.line_diff.is_some();
                // gutter = diff marker (1 if present) + line number digits + 1 space
                let gutter_cols = gutter_width + 1 + if has_diff { 1 } else { 0 };
                self.visible_content_width = (inner.width as usize).saturating_sub(gutter_cols);

                let mut render_row: u16 = 0;
                let max_rows = inner.height;

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

                    let y = inner.y + render_row;
                    buf.set_line(inner.x, y, &line, inner.width);

                    // Highlight cursor line with subtle background
                    if focused && code_line_idx == self.cursor_line {
                        for x in inner.x..inner.x + inner.width {
                            let cell = &mut buf[(x, y)];
                            cell.set_bg(Color::DarkGray);
                        }
                    }

                    render_row += 1;

                    // Render inline comment block after this line if applicable
                    if let Some(comment) = self.comment_at_end_line(line_num) {
                        if render_row < max_rows {
                            self.render_comment_block(
                                comment, inner, buf, &mut render_row, max_rows,
                            );
                        }
                    }
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

    /// Render unified diff content.
    fn render_diff_mode(&self, inner: Rect, buf: &mut Buffer) {
        let Some(ref diff) = self.unified_diff else {
            let msg = "No changes";
            let line = Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)));
            buf.set_line(inner.x, inner.y, &line, inner.width);
            return;
        };

        if diff.lines.is_empty() {
            let msg = "No changes";
            let line = Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)));
            buf.set_line(inner.x, inner.y, &line, inner.width);
            return;
        }

        for (i, diff_line) in diff
            .lines
            .iter()
            .skip(self.scroll_offset)
            .take(self.visible_height)
            .enumerate()
        {
            let (text, style) = match diff_line {
                UnifiedDiffLine::Added(s) => (format!("+{s}"), Style::default().fg(Color::Green)),
                UnifiedDiffLine::Removed(s) => (format!("-{s}"), Style::default().fg(Color::Red)),
                UnifiedDiffLine::Context(s) => (format!(" {s}"), Style::default()),
                UnifiedDiffLine::HunkHeader(s) => {
                    (s.clone(), Style::default().fg(Color::Cyan))
                }
            };

            let line = Line::from(Span::styled(text, style));
            let y = inner.y + i as u16;
            if y < inner.y + inner.height {
                buf.set_line(inner.x, y, &line, inner.width);
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
