use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
    Frame,
};

use super::{App, Focus, InputMode, FLASH_DURATION};

/// Width of the help dialog in columns.
const HELP_DIALOG_WIDTH: u16 = 42;

pub(super) fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Main layout: tree + viewer above, hint bar at bottom
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let pane_area = main_chunks[0];
    let hint_area = main_chunks[1];

    // Split panes: dynamic ratio from app state, or full-width viewer in focus mode
    let pane_chunks = if app.focus_mode {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(0), Constraint::Percentage(100)])
            .split(pane_area)
    } else {
        let tree_pct = app.tree_width_percent;
        let viewer_pct = 100u16.saturating_sub(tree_pct);
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(tree_pct),
                Constraint::Percentage(viewer_pct),
            ])
            .split(pane_area)
    };

    let tree_area = pane_chunks[0];
    let viewer_area = pane_chunks[1];

    // Cache the border column for mouse hit-testing (0 in focus mode for drag-to-restore)
    app.border_column = if app.focus_mode { 0 } else { tree_area.right() };
    // Cache tree inner area top for click-to-select (top border = +1)
    app.tree_inner_y = tree_area.y + 1;

    // Set visible height so ensure_visible in event handlers uses the correct value
    // (render_to_buffer also sets this, but we need it before the first render)
    app.file_tree.visible_height = tree_area.height.saturating_sub(2) as usize;

    // Compute render-time data (business logic separated from draw)
    let render_data = app.prepare_for_render();
    let viewer_ctx = crate::file_viewer::ViewerRenderContext {
        comments: &render_data.viewer_comments,
        comment_edit: render_data.comment_edit.as_ref(),
        line_select_range: render_data.line_select_range,
    };

    // Render panes
    let buf = frame.buffer_mut();
    app.file_tree
        .render_to_buffer(tree_area, buf, app.focus == Focus::Tree, &render_data.commented_files);
    app.file_viewer
        .render_to_buffer(viewer_area, buf, app.focus == Focus::Viewer, &viewer_ctx);

    // Highlight border when resizing
    if app.resizing && tree_area.right() > 0 {
        let border_x = tree_area.right() - 1;
        for y in tree_area.top()..tree_area.bottom() {
            if let Some(cell) = buf.cell_mut((border_x, y)) {
                cell.set_style(Style::default().fg(Color::Yellow));
            }
        }
    }

    // Clear expired flash messages
    if let Some(flash) = &app.flash_message
        && flash.created.elapsed() >= FLASH_DURATION {
            app.flash_message = None;
        }

    // Render key hint bar (or comment input)
    let (hints, hint_color): (String, Color) = match &app.input_mode {
        InputMode::CommentInput { start_line, end_line, .. } => {
            let range = if start_line == end_line {
                format!("L{}", start_line)
            } else {
                format!("L{}-{}", start_line, end_line)
            };
            (format!("Editing comment on {}  Enter save  Esc cancel", range), Color::DarkGray)
        }
        InputMode::LineSelect { .. } => {
            ("j/k extend  c comment  Esc cancel".into(), Color::DarkGray)
        }
        InputMode::Normal => {
            if let Some(flash) = &app.flash_message {
                (flash.text.clone(), flash.color)
            } else if let Some(hints) = super::comment_ops::comment_list_hints(app) {
                (hints, Color::DarkGray)
            } else if app.file_tree.search.is_some() {
                ("Enter confirm  Esc cancel  ↑/↓ navigate".into(), Color::DarkGray)
            } else {
                ("? help  q quit".into(), Color::DarkGray)
            }
        }
    };

    let hint_line = Line::from(Span::styled(&hints, Style::default().fg(hint_color)));
    buf.set_line(hint_area.x, hint_area.y, &hint_line, hint_area.width);

    // Help dialog overlay
    if app.show_help {
        draw_help_dialog(buf, area);
    }
}

fn draw_help_dialog(buf: &mut ratatui::buffer::Buffer, area: Rect) {
    let help_lines = [
        " Navigation",
        "   j/k          cursor up/down",
        "   h/l          scroll left/right",
        "   Ctrl-d/u     half-page scroll",
        "   Tab           switch pane",
        "",
        " File Tree",
        "   Enter         open file",
        "   h/l           fold/unfold",
        "   /             search",
        "   g             changed files",
        "   c             comment list",
        "",
        " Viewer",
        "   d             toggle diff",
        "   c             add comment",
        "   V             line select",
        "   e             export comments",
        "   b             toggle focus mode",
        "",
        " Other",
        "   B             report bug / feedback",
        "",
        " Press ? or Esc to close",
    ];

    let dialog_w: u16 = HELP_DIALOG_WIDTH;
    let dialog_h: u16 = (help_lines.len() as u16) + 2; // +2 for borders
    let w = dialog_w.min(area.width.saturating_sub(2));
    let h = dialog_h.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let dialog_rect = Rect::new(x, y, w, h);

    // Clear background
    for row in dialog_rect.top()..dialog_rect.bottom() {
        for col in dialog_rect.left()..dialog_rect.right() {
            if let Some(cell) = buf.cell_mut((col, row)) {
                cell.set_char(' ');
                cell.set_style(Style::default());
            }
        }
    }

    // Draw border
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .style(Style::default().fg(Color::White));
    let inner = block.inner(dialog_rect);
    // Render block border manually into buffer
    ratatui::widgets::Widget::render(block, dialog_rect, buf);

    // Draw content lines
    let style = Style::default().fg(Color::White);
    let section_style = Style::default().fg(Color::Yellow);
    for (i, line_text) in help_lines.iter().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        let st = if line_text.starts_with(' ') && !line_text.starts_with("   ") && !line_text.is_empty() {
            section_style
        } else {
            style
        };
        let line = Line::from(Span::styled(*line_text, st));
        buf.set_line(inner.x, inner.y + i as u16, &line, inner.width);
    }
}
