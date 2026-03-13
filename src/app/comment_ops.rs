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
            };
        } else if let Some(line) = app.file_viewer.cursor_file_line() {
            // Cursor is on a code line -> always new comment
            app.input_mode = InputMode::CommentInput {
                file,
                start_line: line,
                end_line: line,
                text: String::new(),
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
            } = app.input_mode
                && !text.is_empty() {
                    let code_context =
                        extract_code_context(&app.file_viewer.content, file, start_line, end_line);
                    app.comment_store
                        .add(file, start_line, end_line, text.clone(), code_context);
                }
            app.input_mode = InputMode::Normal;
            refresh_comment_list(app);
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
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
        KeyCode::Char('c') => {
            // Open comment input for the selected range
            if let InputMode::LineSelect { ref file, anchor } = app.input_mode {
                let current = app.file_viewer.cursor_file_line().unwrap_or(anchor);
                let start = anchor.min(current);
                let end = anchor.max(current);
                let file = file.clone();
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
