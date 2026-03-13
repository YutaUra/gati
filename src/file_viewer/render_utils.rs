use ratatui::{
    buffer::Buffer,
    style::{Color, Style},
    text::Span,
};

use crate::diff::{DiffLineKind, LineDiff};

/// Skip the first `skip` characters from a sequence of styled spans.
///
/// Characters are counted (not bytes), so multi-byte characters are handled
/// correctly. Returns owned spans with 'static lifetime.
pub fn skip_chars_in_spans(spans: Vec<Span<'_>>, skip: usize) -> Vec<Span<'static>> {
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

/// Fill an entire row with the given background color.
pub fn fill_row_bg(buf: &mut Buffer, x: u16, y: u16, width: u16, bg: Color) {
    for col in x..x + width {
        buf[(col, y)].set_bg(bg);
    }
}

/// Calculate the width needed for line numbers.
pub fn line_number_width(total_lines: usize) -> usize {
    if total_lines == 0 {
        1
    } else {
        total_lines.to_string().len()
    }
}

/// Build gutter spans (diff marker + line number) for a single code line.
///
/// When `line_diff` is `Some`, the first span is a colored diff marker (▎);
/// otherwise the gutter starts directly with the line number.
pub fn gutter_spans(
    line_num: usize,
    gutter_width: usize,
    line_diff: Option<&LineDiff>,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if let Some(diff) = line_diff {
        let kind = diff.line_kind(line_num);
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
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn line_number_width_for_various_sizes() {
        assert_eq!(line_number_width(1), 1);
        assert_eq!(line_number_width(9), 1);
        assert_eq!(line_number_width(10), 2);
        assert_eq!(line_number_width(99), 2);
        assert_eq!(line_number_width(100), 3);
        assert_eq!(line_number_width(1000), 4);
    }
}
