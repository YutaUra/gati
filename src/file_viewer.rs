use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use crate::components::{Action, Component};
use crate::highlight::Highlighter;

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
    highlighter: Highlighter,
}

impl FileViewer {
    pub fn new() -> Self {
        Self {
            content: ViewerContent::Placeholder,
            scroll_offset: 0,
            visible_height: 20,
            highlighter: Highlighter::new(),
        }
    }

    /// Load a file into the viewer.
    pub fn load_file(&mut self, path: &Path) {
        self.scroll_offset = 0;

        // Try to read the file
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
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
        self.content = ViewerContent::File {
            path: path.to_path_buf(),
            lines,
            syntax_name,
        };
    }

    fn total_lines(&self) -> usize {
        match &self.content {
            ViewerContent::File { lines, .. } => lines.len(),
            _ => 0,
        }
    }

    fn max_scroll(&self) -> usize {
        self.total_lines().saturating_sub(1)
    }

    fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = (self.scroll_offset + amount).min(self.max_scroll());
    }

    fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn render_to_buffer(&mut self, area: Rect, buf: &mut Buffer, focused: bool) {
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Preview ");

        let inner = block.inner(area);
        block.render(area, buf);

        self.visible_height = inner.height as usize;

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
                syntax_name,
                ..
            } => {
                let gutter_width = line_number_width(lines.len());

                // Look up syntax by name and create highlight state
                let syntax = self
                    .highlighter
                    .syntax_set
                    .find_syntax_by_name(syntax_name)
                    .unwrap_or_else(|| self.highlighter.syntax_set.find_syntax_plain_text());
                let mut hl_state = self.highlighter.new_highlight_state(syntax);

                // Advance highlight state past lines before scroll_offset
                for line_text in lines.iter().take(self.scroll_offset) {
                    // Feed lines to keep parse state correct, discard results
                    let _ = self.highlighter.highlight_line(&mut hl_state, &format!("{line_text}\n"));
                }

                for (i, line_text) in lines
                    .iter()
                    .skip(self.scroll_offset)
                    .take(self.visible_height)
                    .enumerate()
                {
                    let line_num = self.scroll_offset + i + 1;
                    let num_str = format!("{:>width$} ", line_num, width = gutter_width);

                    let mut spans = vec![Span::styled(num_str, Style::default().fg(Color::DarkGray))];
                    let highlighted = self.highlighter.highlight_line(&mut hl_state, &format!("{line_text}\n"));
                    spans.extend(highlighted);

                    let line = Line::from(spans);

                    let y = inner.y + i as u16;
                    if y < inner.y + inner.height {
                        buf.set_line(inner.x, y, &line, inner.width);
                    }
                }
            }
        }
    }
}

impl Component for FileViewer {
    fn handle_event(&mut self, key: KeyEvent) -> anyhow::Result<Action> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                self.scroll_down(1);
                Ok(Action::None)
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                self.scroll_up(1);
                Ok(Action::None)
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                let half = self.visible_height / 2;
                self.scroll_down(half);
                Ok(Action::None)
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                let half = self.visible_height / 2;
                self.scroll_up(half);
                Ok(Action::None)
            }
            (KeyCode::Tab, _) => Ok(Action::SwitchFocus),
            (KeyCode::Char('q'), KeyModifiers::NONE) => Ok(Action::Quit),
            _ => Ok(Action::None),
        }
    }
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

    // 4.3: Scrolling
    #[test]
    fn scroll_down_line_by_line() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        assert_eq!(viewer.scroll_offset, 0);

        viewer.handle_event(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(viewer.scroll_offset, 1);
    }

    #[test]
    fn scroll_up_line_by_line() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.scroll_offset = 5;

        viewer.handle_event(key(KeyCode::Char('k'))).unwrap();
        assert_eq!(viewer.scroll_offset, 4);
    }

    #[test]
    fn scroll_down_half_page_with_ctrl_d() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.visible_height = 20;
        viewer.load_file(&path);

        viewer.handle_event(ctrl_key('d')).unwrap();
        assert_eq!(viewer.scroll_offset, 10); // half of 20
    }

    #[test]
    fn scroll_up_half_page_with_ctrl_u() {
        let content: Vec<u8> = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("long.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.visible_height = 20;
        viewer.load_file(&path);
        viewer.scroll_offset = 20;

        viewer.handle_event(ctrl_key('u')).unwrap();
        assert_eq!(viewer.scroll_offset, 10);
    }

    #[test]
    fn scroll_clamped_at_end() {
        let (_tmp, path) = tmp_file("short.txt", b"line1\nline2");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);
        viewer.scroll_offset = 1; // max for 2 lines

        viewer.handle_event(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(viewer.scroll_offset, 1); // stays at max
    }

    #[test]
    fn scroll_clamped_at_beginning() {
        let (_tmp, path) = tmp_file("short.txt", b"line1\nline2");
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        viewer.handle_event(key(KeyCode::Char('k'))).unwrap();
        assert_eq!(viewer.scroll_offset, 0); // stays at 0
    }

    #[test]
    fn scroll_with_arrow_keys() {
        let content: Vec<u8> = (0..10).map(|i| format!("line {i}\n")).collect::<String>().into();
        let (_tmp, path) = tmp_file("file.txt", &content);
        let mut viewer = FileViewer::new();
        viewer.load_file(&path);

        viewer.handle_event(key(KeyCode::Down)).unwrap();
        assert_eq!(viewer.scroll_offset, 1);

        viewer.handle_event(key(KeyCode::Up)).unwrap();
        assert_eq!(viewer.scroll_offset, 0);
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
}
