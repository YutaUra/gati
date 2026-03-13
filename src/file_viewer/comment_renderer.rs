use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
};

use crate::comments::Comment;
use crate::unicode;

use super::render_utils::fill_row_bg;
use super::CommentEditState;

/// Render an inline comment block (2 rows: header + separator).
pub fn render_comment_block(
    comment: &Comment,
    is_stale: bool,
    inner: Rect,
    buf: &mut Buffer,
    render_row: &mut u16,
    max_rows: u16,
) {
    let (icon, text_color, sep_color) = if is_stale {
        ("\u{26a0}\u{fe0f}", Color::Yellow, Color::Yellow)
    } else {
        ("\u{1f4ac}", Color::Cyan, Color::DarkGray)
    };
    let comment_style = Style::default().fg(text_color).bg(Color::Black);

    // Row 1: range + comment text
    let range_str = if comment.start_line == comment.end_line {
        format!("  {icon} L{}: {}", comment.start_line, comment.text)
    } else {
        format!(
            "  {icon} L{}-{}: {}",
            comment.start_line, comment.end_line, comment.text
        )
    };

    let y = inner.y + *render_row;
    let line = Line::from(Span::styled(&range_str, comment_style));
    buf.set_line(inner.x, y, &line, inner.width);
    fill_row_bg(buf, inner.x, y, inner.width, Color::Black);
    *render_row += 1;

    render_separator(buf, inner, render_row, max_rows, sep_color);
}

/// Render the inline comment editor (single row: prefix + text + cursor).
///
/// If the text is wider than the available space, truncate from the left
/// so the cursor end is always visible.
pub fn render_comment_editor(
    edit: &CommentEditState,
    inner: Rect,
    buf: &mut Buffer,
    render_row: &mut u16,
    max_rows: u16,
) {
    if *render_row >= max_rows {
        return;
    }
    let style = Style::default().fg(Color::Cyan).bg(Color::Black);
    let range = if edit.start_line == edit.target_line {
        format!("L{}", edit.target_line)
    } else {
        format!("L{}-{}", edit.start_line, edit.target_line)
    };
    let prefix = format!("  ✏️ {}: ", range);
    let prefix_width = prefix.chars().count();
    let available = (inner.width as usize).saturating_sub(prefix_width + 1); // +1 for cursor

    // Truncate text from the left if it exceeds available width.
    // Use char_skip_byte_offset to avoid splitting multi-byte characters.
    let text_char_count = edit.text.chars().count();
    let display_text = if text_char_count > available {
        let skip_chars = text_char_count - available;
        let byte_offset = unicode::char_skip_byte_offset(&edit.text, skip_chars);
        &edit.text[byte_offset..]
    } else {
        &edit.text
    };

    let content = format!("{prefix}{display_text}█");
    let y = inner.y + *render_row;
    let line = Line::from(Span::styled(&content, style));
    buf.set_line(inner.x, y, &line, inner.width);
    fill_row_bg(buf, inner.x, y, inner.width, Color::Black);
    *render_row += 1;

    render_separator(buf, inner, render_row, max_rows, Color::DarkGray);
}

/// Render a horizontal separator line ("─") if there is room.
///
/// Shared by `render_comment_block` and `render_comment_editor` to avoid
/// duplicating the same separator rendering code.
pub fn render_separator(
    buf: &mut Buffer,
    inner: Rect,
    render_row: &mut u16,
    max_rows: u16,
    color: Color,
) {
    if *render_row < max_rows {
        let y = inner.y + *render_row;
        let sep = "─".repeat(inner.width as usize);
        let line = Line::from(Span::styled(sep, Style::default().fg(color)));
        buf.set_line(inner.x, y, &line, inner.width);
        *render_row += 1;
    }
}
