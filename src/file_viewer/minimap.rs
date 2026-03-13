use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
};

use crate::comments::Comment;
use crate::diff::{DiffLineKind, UnifiedDiffLine};

use super::diff_state::DiffState;

/// Background color for the minimap area.
const MINIMAP_BG: Color = Color::Rgb(30, 30, 30);
/// Color for the viewport indicator in the minimap.
const MINIMAP_VIEWPORT: Color = Color::Rgb(80, 80, 80);

/// Minimum inner width (in columns) below which the minimap is hidden.
pub const MINIMAP_MIN_WIDTH: u16 = 30;
/// Width of the minimap in terminal columns.
pub const MINIMAP_WIDTH: u16 = 2;

/// Color of a minimap marker for a given row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MinimapMarker {
    Added,
    Modified,
    Removed,
    Comment,
    StaleComment,
}

/// Compute minimap markers for each row of the minimap.
/// Returns a Vec of length `minimap_height` where each entry is the most
/// important marker for lines mapped to that row (Comment > Diff).
pub fn compute_markers(
    total_lines: usize,
    minimap_height: usize,
    diff: &DiffState,
    comments: &[(Comment, bool)],
) -> Vec<Option<MinimapMarker>> {
    if total_lines == 0 || minimap_height == 0 {
        return vec![None; minimap_height];
    }

    let mut markers = vec![None; minimap_height];

    if diff.diff_mode {
        // Diff mode: use unified diff lines (excluding hunk headers)
        if let Some(ref ud) = diff.unified_diff {
            let displayable: Vec<&UnifiedDiffLine> = ud
                .lines
                .iter()
                .filter(|l| !matches!(l, UnifiedDiffLine::HunkHeader(_)))
                .collect();
            for (i, line) in displayable.iter().enumerate() {
                let row = i * minimap_height / total_lines.max(1);
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
                    if !matches!(markers[row], Some(MinimapMarker::Comment | MinimapMarker::StaleComment)) {
                        markers[row] = Some(m);
                    }
                }
            }
        }
    } else {
        // Normal mode: use line diff data
        if let Some(ref ld) = diff.line_diff {
            for line_num in 1..=total_lines {
                let row = (line_num - 1) * minimap_height / total_lines;
                if row >= minimap_height {
                    break;
                }
                let kind = ld.line_kind(line_num);
                let marker = match kind {
                    DiffLineKind::Added => Some(MinimapMarker::Added),
                    DiffLineKind::Modified => Some(MinimapMarker::Modified),
                    DiffLineKind::Unchanged => None,
                };
                if let Some(m) = marker
                    && !matches!(markers[row], Some(MinimapMarker::Comment | MinimapMarker::StaleComment)) {
                        markers[row] = Some(m);
                    }
            }
        }
    }

    // Comment markers (highest priority, overwrite diff markers)
    for (comment, is_stale) in comments {
        let marker = if *is_stale {
            MinimapMarker::StaleComment
        } else {
            MinimapMarker::Comment
        };
        for line_num in comment.start_line..=comment.end_line {
            let idx = line_num.saturating_sub(1);
            let row = idx * minimap_height / total_lines.max(1);
            if row < minimap_height {
                markers[row] = Some(marker);
            }
        }
    }

    markers
}

/// Translate a minimap row to the corresponding file line index (0-based).
pub fn row_to_line(row: u16, minimap_height: u16, total_lines: usize) -> usize {
    if minimap_height == 0 || total_lines == 0 {
        return 0;
    }
    (row as usize * total_lines / minimap_height as usize).min(total_lines.saturating_sub(1))
}

/// Render the minimap with half-block characters for 2x vertical resolution.
///
/// Column 1 (left): change markers -- colored half-blocks showing where diffs/comments are.
/// Column 2 (right): viewport indicator -- shows which portion of the file is visible.
///
/// Half-block rendering doubles the effective vertical resolution by using upper half
/// and lower half characters, where each terminal row encodes two virtual rows.
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    total_lines: usize,
    scroll_offset: usize,
    visible_height: usize,
    diff: &DiffState,
    comments: &[(Comment, bool)],
) {
    let minimap_h = area.height as usize;

    // 2x virtual resolution: each terminal row maps to 2 virtual rows
    let virtual_h = minimap_h * 2;
    let markers = compute_markers(total_lines, virtual_h, diff, comments);

    // Viewport range in virtual rows (ceiling division for end)
    let (vp_start, vp_end) = if total_lines > 0 {
        let start = scroll_offset * virtual_h / total_lines;
        let visible = visible_height.min(total_lines);
        let end_numer = (scroll_offset + visible) * virtual_h;
        let end = end_numer.div_ceil(total_lines).min(virtual_h);
        (start, end.max(start + 1))
    } else {
        (0, virtual_h)
    };

    let bg_dim = MINIMAP_BG;
    let vp_color = MINIMAP_VIEWPORT;

    fn marker_color(m: MinimapMarker) -> Color {
        match m {
            MinimapMarker::Added => Color::Green,
            MinimapMarker::Modified => Color::Yellow,
            MinimapMarker::Removed => Color::Red,
            MinimapMarker::Comment => Color::Cyan,
            MinimapMarker::StaleComment => Color::Yellow,
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
                    ("\u{2588}", tc, bg_dim)
                } else {
                    // upper half: fg = top pixel, bg = bottom pixel
                    ("\u{2580}", tc, bc)
                }
            }
            (Some(t), None) => ("\u{2580}", marker_color(t), bg_dim),
            (None, Some(b)) => ("\u{2584}", marker_color(b), bg_dim),
            (None, None) => (" ", bg_dim, bg_dim),
        };
        let line1 = Line::from(Span::styled(ch1, Style::default().fg(fg1).bg(bg1)));
        buf.set_line(area.x, y, &line1, 1);

        // Column 2: viewport indicator (half-block, 2x resolution)
        if area.width > 1 {
            let top_vp = vr_top >= vp_start && vr_top < vp_end;
            let bot_vp = vr_bot >= vp_start && vr_bot < vp_end;

            let (ch2, fg2, bg2) = match (top_vp, bot_vp) {
                (true, true) => ("\u{2588}", vp_color, bg_dim),
                (true, false) => ("\u{2580}", vp_color, bg_dim),
                (false, true) => ("\u{2584}", vp_color, bg_dim),
                (false, false) => (" ", bg_dim, bg_dim),
            };
            let line2 = Line::from(Span::styled(ch2, Style::default().fg(fg2).bg(bg2)));
            buf.set_line(area.x + 1, y, &line2, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_comment(path: &std::path::Path, start: usize, end: usize) -> Comment {
        Comment {
            file: path.to_path_buf(),
            start_line: start,
            end_line: end,
            text: "test comment".into(),
            code_context: vec![],
        }
    }

    #[test]
    fn stale_comment_minimap_marker() {
        let path = PathBuf::from("test.txt");

        // Fresh comment
        let fresh = Comment {
            file: path.clone(),
            start_line: 1,
            end_line: 1,
            text: "ok".into(),
            code_context: vec!["line1".into()],
        };
        // Stale comment
        let stale = Comment {
            file: path.clone(),
            start_line: 2,
            end_line: 2,
            text: "old".into(),
            code_context: vec!["original_line2".into()],
        };
        let comments = vec![(fresh, false), (stale, true)];

        let diff = DiffState::new();
        // total_lines=3, minimap_height=6
        let markers = compute_markers(3, 6, &diff, &comments);
        // Line 1 (fresh) -> MinimapMarker::Comment (Cyan)
        assert_eq!(markers[0], Some(MinimapMarker::Comment));
        // Line 2 (stale) -> MinimapMarker::StaleComment (Yellow)
        assert_eq!(markers[2], Some(MinimapMarker::StaleComment));
    }

    #[test]
    fn row_to_line_basic() {
        // 10 total lines, 5 row minimap
        assert_eq!(row_to_line(0, 5, 10), 0);
        assert_eq!(row_to_line(2, 5, 10), 4);
        assert_eq!(row_to_line(4, 5, 10), 8);
    }

    #[test]
    fn row_to_line_zero_height_returns_zero() {
        assert_eq!(row_to_line(0, 0, 10), 0);
    }

    #[test]
    fn row_to_line_zero_lines_returns_zero() {
        assert_eq!(row_to_line(3, 5, 0), 0);
    }

    #[test]
    fn compute_markers_empty() {
        let diff = DiffState::new();
        let markers = compute_markers(0, 5, &diff, &[]);
        assert_eq!(markers.len(), 5);
        assert!(markers.iter().all(|m| m.is_none()));
    }

    #[test]
    fn compute_markers_comment_overwrites_diff() {
        let path = PathBuf::from("test.txt");
        let comments = vec![(make_comment(&path, 1, 1), false)];
        let mut diff = DiffState::new();
        diff.line_diff = Some(crate::diff::LineDiff { lines: vec![crate::diff::DiffLineKind::Added] });
        // total_lines=1, minimap_height=1 => single row
        let markers = compute_markers(1, 1, &diff, &comments);
        // Comment marker should override diff marker
        assert_eq!(markers[0], Some(MinimapMarker::Comment));
    }
}
