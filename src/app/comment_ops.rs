use std::time::Instant;

use ratatui::style::Color;

use crate::comments::{CommentListEntry, CommentStore};

use super::{App, FlashMessage, InputMode};

/// Start a comment at the current cursor position.
pub(super) fn start_comment(app: &mut App) {
    if let Some(file) = app.file_viewer.current_file() {
        let file = file.to_path_buf();
        if let Some((start, end)) = app.file_viewer.cursor_on_comment {
            // Cursor is on a comment row -> edit that specific comment
            let existing_text = app
                .comment_store
                .find_exact(&file, start, end)
                .map(|c| c.text.clone())
                .unwrap_or_default();
            app.input_mode = InputMode::CommentInput {
                file,
                start_line: start,
                end_line: end,
                text: existing_text,
                previous: None,
            };
        } else if let Some(line) = app.file_viewer.cursor_file_line() {
            // Cursor is on a code line -> always new comment
            app.input_mode = InputMode::CommentInput {
                file,
                start_line: line,
                end_line: line,
                text: String::new(),
                previous: None,
            };
        }
        // If cursor_file_line() is None (Removed line in diff), do nothing
    }
}

/// Delete the comment at the current cursor position.
pub(super) fn delete_comment_at_cursor(app: &mut App) {
    if let Some(file) = app.file_viewer.current_file() {
        let file = file.to_path_buf();
        if let Some((start, end)) = app.file_viewer.cursor_on_comment {
            // Cursor is on a comment row -> delete that specific comment
            app.comment_store.delete(&file, start, end);
            app.file_viewer.cursor_on_comment = None;
        } else if let Some(line) = app.file_viewer.cursor_file_line() {
            // Cursor is on a code line -> delete comment at that line
            if let Some(comment) = app.comment_store.find_at_line(&file, line) {
                let start = comment.start_line;
                let end = comment.end_line;
                app.comment_store.delete(&file, start, end);
            }
        }
    }
    refresh_comment_list(app);
}

pub(super) fn export_comments(app: &mut App) {
    let text = app.comment_store.export();
    if text.is_empty() {
        app.flash_message = Some(FlashMessage {
            text: "No comments to export".into(),
            color: Color::Yellow,
            created: Instant::now(),
        });
        return;
    }
    match cli_clipboard::set_contents(text) {
        Ok(_) => {
            let count = app.comment_store.len();
            app.flash_message = Some(FlashMessage {
                text: format!("Copied {} comment(s) to clipboard", count),
                color: Color::Green,
                created: Instant::now(),
            });
        }
        Err(_) => {
            app.flash_message = Some(FlashMessage {
                text: "Failed to copy to clipboard".into(),
                color: Color::Red,
                created: Instant::now(),
            });
        }
    }
}

/// Rebuild the comment list entries if currently in comment list mode.
pub(super) fn refresh_comment_list(app: &mut App) {
    if app.file_tree.is_comment_list_mode() {
        let entries = build_comment_list_entries(&app.comment_store, &app.target_dir);
        if entries.is_empty() {
            app.file_tree.exit_comment_list();
        } else {
            app.file_tree.enter_comment_list(entries);
        }
    }
}

/// Handle hint bar text when in comment list mode.
pub(super) fn comment_list_hints(app: &App) -> Option<String> {
    if app.file_tree.is_comment_list_mode() {
        Some("j/k navigate  Enter jump  c/Esc back".into())
    } else {
        None
    }
}

pub(super) fn extract_code_context(
    content: &crate::file_viewer::ViewerContent,
    file: &std::path::Path,
    start_line: usize,
    end_line: usize,
) -> Vec<String> {
    if let crate::file_viewer::ViewerContent::File { path, lines, .. } = content
        && path == file {
            let start = start_line.saturating_sub(1).min(lines.len());
            let end = end_line.min(lines.len());
            return lines[start..end].to_vec();
        }
    vec![]
}

pub(super) fn handle_comment_input(app: &mut App, key: crossterm::event::KeyEvent) {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Enter => {
            // Save the comment
            if let InputMode::CommentInput {
                ref file,
                start_line,
                end_line,
                ref text,
                ..
            } = app.input_mode
                && !text.is_empty() {
                    let code_context =
                        extract_code_context(&app.file_viewer.content, file, start_line, end_line);
                    app.comment_store
                        .add(file, start_line, end_line, text.clone(), code_context);
                }
            // Enter always goes to Normal (comment saved, selection done)
            app.input_mode = InputMode::Normal;
            refresh_comment_list(app);
        }
        KeyCode::Esc => {
            // Restore previous selection mode if we came from one
            let prev = if let InputMode::CommentInput { ref mut previous, .. } = app.input_mode {
                previous.take()
            } else {
                None
            };
            app.input_mode = prev.map_or(InputMode::Normal, |p| *p);
        }
        KeyCode::Backspace => {
            if let InputMode::CommentInput { ref mut text, .. } = app.input_mode {
                text.pop();
            }
        }
        KeyCode::Char(c) => {
            if let InputMode::CommentInput { ref mut text, .. } = app.input_mode {
                text.push(c);
            }
        }
        _ => {}
    }
}

pub(super) fn handle_line_select(app: &mut App, key: crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            let comments = app.viewer_comments();
            app.file_viewer.cursor_down(&comments);
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let comments = app.viewer_comments();
            app.file_viewer.cursor_up(&comments);
            true
        }
        KeyCode::Char('y') => {
            copy_line_selection(app);
            true
        }
        KeyCode::Char('c') => {
            // Open comment input for the selected range, saving selection for restore on Esc
            if let InputMode::LineSelect { ref file, anchor } = app.input_mode {
                let current = app.file_viewer.cursor_file_line().unwrap_or(anchor);
                let start = anchor.min(current);
                let end = anchor.max(current);
                let file = file.clone();
                let prev = app.input_mode.clone();
                // Only pre-fill when editing the exact same range;
                // overlapping or contained ranges should start empty.
                let existing_text = app
                    .comment_store
                    .find_exact(&file, start, end)
                    .map(|c| c.text.clone())
                    .unwrap_or_default();
                app.input_mode = InputMode::CommentInput {
                    file,
                    start_line: start,
                    end_line: end,
                    text: existing_text,
                    previous: Some(Box::new(prev)),
                };
            }
            true
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            true
        }
        _ => false,
    }
}

/// Handle keyboard input in CharSelect mode.
/// Returns true if the key was consumed.
pub(super) fn handle_char_select(app: &mut App, key: crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Char('y') => {
            copy_char_selection(app);
            true
        }
        KeyCode::Char('c') => {
            // Convert char selection to line range and open comment input,
            // saving selection for restore on Esc
            if let InputMode::CharSelect { ref file, anchor_line, .. } = app.input_mode {
                let current = app.file_viewer.cursor_file_line().unwrap_or(anchor_line);
                let start = anchor_line.min(current);
                let end = anchor_line.max(current);
                let file = file.clone();
                let prev = app.input_mode.clone();
                let existing_text = app
                    .comment_store
                    .find_exact(&file, start, end)
                    .map(|c| c.text.clone())
                    .unwrap_or_default();
                app.input_mode = InputMode::CommentInput {
                    file,
                    start_line: start,
                    end_line: end,
                    text: existing_text,
                    previous: Some(Box::new(prev)),
                };
            }
            true
        }
        KeyCode::Char('j') | KeyCode::Down => {
            let comments = app.viewer_comments();
            app.file_viewer.cursor_down(&comments);
            // Clamp cursor_col to new line length
            if let Some(col) = app.file_viewer.cursor_col {
                let max_col = app.file_viewer.line_char_count(app.file_viewer.cursor_line);
                app.file_viewer.cursor_col = Some(col.min(max_col));
            }
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let comments = app.viewer_comments();
            app.file_viewer.cursor_up(&comments);
            if let Some(col) = app.file_viewer.cursor_col {
                let max_col = app.file_viewer.line_char_count(app.file_viewer.cursor_line);
                app.file_viewer.cursor_col = Some(col.min(max_col));
            }
            true
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            true
        }
        _ => false,
    }
}

/// Extract text from a character range across lines.
/// Lines are 0-indexed slices, start_col/end_col are 0-indexed char offsets.
pub fn extract_char_range(
    lines: &[String],
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col: usize,
) -> String {
    if lines.is_empty() || start_line > end_line || start_line >= lines.len() {
        return String::new();
    }

    let end_line = end_line.min(lines.len() - 1);

    if start_line == end_line {
        // Single line selection
        let line = &lines[start_line];
        line.chars().skip(start_col).take(end_col.saturating_sub(start_col)).collect()
    } else {
        // Multi-line selection
        let mut result = String::new();
        // First line: from start_col to end
        let first = &lines[start_line];
        result.extend(first.chars().skip(start_col));
        result.push('\n');
        // Middle lines: full content
        for line in &lines[start_line + 1..end_line] {
            result.push_str(line);
            result.push('\n');
        }
        // Last line: from start to end_col
        let last = &lines[end_line];
        result.extend(last.chars().take(end_col));
        result
    }
}

/// Copy the current char selection to clipboard.
pub(super) fn copy_char_selection(app: &mut App) {
    if let InputMode::CharSelect { anchor_line, anchor_col, .. } = app.input_mode {
        let cursor_line = app.file_viewer.cursor_file_line().unwrap_or(anchor_line);
        let cursor_col = app.file_viewer.cursor_col.unwrap_or(0);

        // Normalize so start <= end
        let (start_line, start_col, end_line, end_col) =
            if (cursor_line, cursor_col) < (anchor_line, anchor_col) {
                (cursor_line, cursor_col, anchor_line, anchor_col)
            } else {
                (anchor_line, anchor_col, cursor_line, cursor_col)
            };

        let lines = app.file_viewer.current_lines();
        // Convert 1-indexed lines to 0-indexed
        let text = extract_char_range(
            lines,
            start_line.saturating_sub(1),
            start_col,
            end_line.saturating_sub(1),
            end_col,
        );

        copy_to_clipboard(app, &text);
    }
}

/// Copy the current line selection to clipboard.
pub(super) fn copy_line_selection(app: &mut App) {
    if let InputMode::LineSelect { anchor, .. } = app.input_mode {
        let current = app.file_viewer.cursor_file_line().unwrap_or(anchor);
        let start = anchor.min(current);
        let end = anchor.max(current);

        let lines = app.file_viewer.current_lines();
        let start_idx = start.saturating_sub(1).min(lines.len());
        let end_idx = end.min(lines.len());
        let text = lines[start_idx..end_idx].join("\n");

        copy_to_clipboard(app, &text);
    }
}

/// Copy text to the system clipboard and show a flash message.
fn copy_to_clipboard(app: &mut App, text: &str) {
    if text.is_empty() {
        return;
    }
    match cli_clipboard::set_contents(text.to_string()) {
        Ok(_) => {
            app.flash_message = Some(FlashMessage {
                text: "Yanked to clipboard".into(),
                color: Color::Green,
                created: Instant::now(),
            });
        }
        Err(_) => {
            app.flash_message = Some(FlashMessage {
                text: "Failed to copy to clipboard".into(),
                color: Color::Red,
                created: Instant::now(),
            });
        }
    }
}

/// Build a flat list of comment entries grouped by file for the comment list view.
pub(super) fn build_comment_list_entries(
    store: &CommentStore,
    root: &std::path::Path,
) -> Vec<CommentListEntry> {
    use std::collections::BTreeMap;

    let mut by_file: BTreeMap<&std::path::Path, Vec<&crate::comments::Comment>> = BTreeMap::new();
    for file_path in store.files_with_comments() {
        let comments = store.for_file(file_path);
        by_file.insert(file_path, comments);
    }

    let mut entries = Vec::new();
    for (file, mut comments) in by_file {
        comments.sort_by_key(|c| c.start_line);
        let display_name = file
            .strip_prefix(root)
            .unwrap_or(file)
            .display()
            .to_string();
        entries.push(CommentListEntry::Header {
            file: file.to_path_buf(),
            display_name,
        });
        for c in comments {
            let first_line = c.text.lines().next().unwrap_or("").to_string();
            entries.push(CommentListEntry::Comment {
                file: c.file.clone(),
                start_line: c.start_line,
                end_line: c.end_line,
                text: first_line,
            });
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::extract_char_range;

    #[test]
    fn single_line_selection() {
        let lines = vec!["hello world".to_string()];
        assert_eq!(extract_char_range(&lines, 0, 6, 0, 11), "world");
    }

    #[test]
    fn single_line_partial() {
        let lines = vec!["abcdefgh".to_string()];
        assert_eq!(extract_char_range(&lines, 0, 2, 0, 5), "cde");
    }

    #[test]
    fn multi_line_selection() {
        let lines = vec![
            "first line".to_string(),
            "second line".to_string(),
            "third line".to_string(),
        ];
        let result = extract_char_range(&lines, 0, 6, 2, 5);
        assert_eq!(result, "line\nsecond line\nthird");
    }

    #[test]
    fn multi_line_two_lines() {
        let lines = vec![
            "hello world".to_string(),
            "goodbye world".to_string(),
        ];
        let result = extract_char_range(&lines, 0, 6, 1, 7);
        assert_eq!(result, "world\ngoodbye");
    }

    #[test]
    fn empty_lines() {
        let lines: Vec<String> = vec![];
        assert_eq!(extract_char_range(&lines, 0, 0, 0, 5), "");
    }

    #[test]
    fn empty_line_content() {
        let lines = vec!["".to_string(), "abc".to_string()];
        let result = extract_char_range(&lines, 0, 0, 1, 2);
        assert_eq!(result, "\nab");
    }

    #[test]
    fn multibyte_characters() {
        let lines = vec!["こんにちは世界".to_string()];
        // Select chars 3..5 = "ち" "は"
        assert_eq!(extract_char_range(&lines, 0, 3, 0, 5), "ちは");
    }

    #[test]
    fn col_beyond_line_length() {
        let lines = vec!["abc".to_string()];
        // end_col exceeds line length, should clamp via .take()
        assert_eq!(extract_char_range(&lines, 0, 1, 0, 100), "bc");
    }

    #[test]
    fn zero_width_selection() {
        let lines = vec!["abc".to_string()];
        assert_eq!(extract_char_range(&lines, 0, 2, 0, 2), "");
    }
}
