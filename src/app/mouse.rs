use std::time::Instant;

use ratatui::style::Color;

use super::{App, FlashMessage, Focus, InputMode, DEFAULT_TREE_WIDTH_PERCENT};

/// Minimum pane width in columns (absolute floor).
const MIN_PANE_COLS: u16 = 10;
/// Minimum pane width as percentage.
const MIN_PANE_PERCENT: u16 = 10;
/// Maximum tree pane width as percentage.
const MAX_TREE_PERCENT: u16 = 70;
/// Lines to scroll per mouse wheel tick.
pub(super) const MOUSE_SCROLL_LINES: usize = 5;

/// Compute clamped tree width percentage from a desired column position.
/// Returns a percentage in [min_percent, MAX_TREE_PERCENT] ensuring both panes
/// are at least max(MIN_PANE_PERCENT%, MIN_PANE_COLS columns) wide.
pub fn clamp_tree_percent(desired_cols: u16, terminal_width: u16) -> u16 {
    if terminal_width == 0 {
        return DEFAULT_TREE_WIDTH_PERCENT;
    }
    let min_cols = (terminal_width * MIN_PANE_PERCENT / 100).max(MIN_PANE_COLS);
    let max_tree_cols = terminal_width * MAX_TREE_PERCENT / 100;
    // Viewer also needs min_cols, so tree max is also terminal_width - min_cols
    let max_tree_cols = max_tree_cols.min(terminal_width.saturating_sub(min_cols));
    let clamped = desired_cols.clamp(min_cols, max_tree_cols);
    (clamped as u32 * 100 / terminal_width as u32) as u16
}

pub(super) fn toggle_focus_mode(app: &mut App) {
    if app.focus_mode {
        // Exit focus mode: restore saved tree width
        app.focus_mode = false;
        app.tree_width_percent = app.saved_tree_width_percent;
    } else {
        // Enter focus mode: save current tree width, force viewer focus
        app.saved_tree_width_percent = app.tree_width_percent;
        app.focus_mode = true;
        app.focus = Focus::Viewer;
    }
}

/// Enter CharSelect mode anchored at the current cursor position.
/// Called on mouse-down in the viewer content area.
fn start_mouse_char_select(app: &mut App) {
    if let Some(file) = app.file_viewer.current_file()
        && let Some(line) = app.file_viewer.cursor_file_line() {
            let file = file.to_path_buf();
            let col = app.file_viewer.cursor_col.unwrap_or(0);
            app.input_mode = InputMode::CharSelect {
                file,
                anchor_line: line,
                anchor_col: col,
            };
        }
}

pub(super) fn handle_mouse(app: &mut App, mouse: crossterm::event::MouseEvent, terminal_width: u16) {
    use crossterm::event::{MouseButton, MouseEventKind};

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => handle_mouse_click(app, mouse),
        MouseEventKind::Drag(MouseButton::Left) => handle_mouse_drag(app, mouse, terminal_width),
        MouseEventKind::Up(MouseButton::Left) => {
            app.resizing = false;
            // If char-select is zero-width (no drag movement), cancel selection
            if let InputMode::CharSelect { anchor_line, anchor_col, .. } = &app.input_mode {
                let current_line = app.file_viewer.cursor_file_line().unwrap_or(*anchor_line);
                let current_col = app.file_viewer.cursor_col.unwrap_or(0);
                if *anchor_line == current_line && *anchor_col == current_col {
                    app.input_mode = InputMode::Normal;
                }
            }
        }
        MouseEventKind::ScrollDown => {
            if app.focus_mode || mouse.column > app.border_column {
                if mouse.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                    app.file_viewer.scroll_right();
                } else {
                    app.file_viewer.scroll_down(MOUSE_SCROLL_LINES);
                }
            } else {
                app.file_tree.scroll_down(MOUSE_SCROLL_LINES);
            }
        }
        MouseEventKind::ScrollUp => {
            if app.focus_mode || mouse.column > app.border_column {
                if mouse.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                    app.file_viewer.scroll_left();
                } else {
                    app.file_viewer.scroll_up(MOUSE_SCROLL_LINES);
                }
            } else {
                app.file_tree.scroll_up(MOUSE_SCROLL_LINES);
            }
        }
        MouseEventKind::ScrollLeft => {
            if app.focus_mode || mouse.column > app.border_column {
                app.file_viewer.scroll_left();
            }
        }
        MouseEventKind::ScrollRight => {
            if app.focus_mode || mouse.column > app.border_column {
                app.file_viewer.scroll_right();
            }
        }
        _ => {}
    }
}

/// Handle left-click: start resize drag, select tree entry, click minimap, or click content.
fn handle_mouse_click(app: &mut App, mouse: crossterm::event::MouseEvent) {
    let border = app.border_column;
    if app.focus_mode {
        // In focus mode, clicking near left edge (col 0-1) starts drag-to-restore
        if mouse.column <= 1 {
            app.resizing = true;
        }
    } else if mouse.column.abs_diff(border) <= 1 {
        app.resizing = true;
    } else if mouse.column < border {
        app.focus = Focus::Tree;
        // Click-to-select tree entry
        if mouse.row >= app.tree_inner_y {
            let inner_row = (mouse.row - app.tree_inner_y) as usize;
            let entry_idx = app.file_tree.scroll_offset + inner_row;
            if let Some(entry) = app.file_tree.model.select_at(entry_idx) {
                if entry.is_directory {
                    if let Err(e) = app.file_tree.model.toggle_expand() {
                        app.flash_message = Some(FlashMessage {
                            text: format!("Toggle expand failed: {}", e),
                            color: Color::Red,
                            created: Instant::now(),
                        });
                    }
                } else {
                    let path = entry.path.clone();
                    app.handle_file_action(&path, false);
                }
            }
        }
    } else if let Some(mr) = app.file_viewer.minimap_rect {
        if mouse.column >= mr.x
            && mouse.column < mr.x + mr.width
            && mouse.row >= mr.y
            && mouse.row < mr.y + mr.height
        {
            // Click on minimap: scroll to corresponding file position
            app.focus = Focus::Viewer;
            let row_in_minimap = mouse.row - mr.y;
            app.file_viewer
                .scroll_to_minimap_row(row_in_minimap, mr.height);
        } else {
            app.focus = Focus::Viewer;
            if app.file_viewer.click_line(mouse.row, mouse.column) {
                start_mouse_char_select(app);
            }
        }
    } else {
        app.focus = Focus::Viewer;
        if app.file_viewer.click_line(mouse.row, mouse.column) {
            start_mouse_char_select(app);
        }
    }
}

/// Handle left-drag: resize pane border or extend line selection.
fn handle_mouse_drag(app: &mut App, mouse: crossterm::event::MouseEvent, terminal_width: u16) {
    let min_cols = (terminal_width * MIN_PANE_PERCENT / 100).max(MIN_PANE_COLS);

    if app.resizing {
        if app.focus_mode {
            // Drag-to-restore: exit focus mode when dragged past min_cols
            if mouse.column >= min_cols {
                app.focus_mode = false;
                app.tree_width_percent = clamp_tree_percent(mouse.column, terminal_width);
            }
        } else {
            // Drag-to-collapse: enter focus mode when dragged below min_cols
            if mouse.column < min_cols {
                app.saved_tree_width_percent = app.tree_width_percent;
                app.focus_mode = true;
                app.focus = Focus::Viewer;
            } else {
                app.tree_width_percent = clamp_tree_percent(mouse.column, terminal_width);
            }
        }
    } else if matches!(app.input_mode, InputMode::CharSelect { .. }) {
        // Extend char selection by moving cursor to dragged position
        app.file_viewer.click_line(mouse.row, mouse.column);
    }
}
