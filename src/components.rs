use crossterm::event::KeyEvent;
use ratatui::style::{Color, Style};

/// Compute the border style for a pane: cyan when focused, dark gray otherwise.
pub fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

/// Action returned by a component's event handler.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// No action needed.
    None,
    /// Move cursor down one line in the viewer.
    CursorDown,
    /// Move cursor up one line in the viewer.
    CursorUp,
    /// Quit the application.
    Quit,
    /// Switch focus to the other pane.
    SwitchFocus,
    /// The selected file changed (path provided for viewer to load).
    FileSelected(std::path::PathBuf),
    /// Enter was pressed on a file — switch focus to viewer too.
    FileOpened(std::path::PathBuf),
    /// User pressed `c` to add/edit a comment on the cursor line.
    StartComment,
    /// User pressed `V` to enter line-select mode.
    StartLineSelect,
    /// User pressed `e` to export comments.
    ExportComments,
    /// User pressed `x` to delete comment on cursor line.
    DeleteComment,
    /// User pressed `B` to open bug report / feedback URL.
    BugReport,
    /// Enter comment list mode (tree → app → tree).
    EnterCommentList,
    /// A comment in the list was focused — preview file at line.
    CommentFocused {
        file: std::path::PathBuf,
        line: usize,
    },
    /// Enter was pressed on a comment — jump to file at line.
    CommentJumped {
        file: std::path::PathBuf,
        line: usize,
    },
    /// Delete a specific comment identified by file and line range.
    DeleteCommentAt {
        file: std::path::PathBuf,
        start_line: usize,
        end_line: usize,
    },
    /// User updated the content search query — App should spawn a worker.
    ContentSearchRequested,
}

/// Trait for TUI components.
pub trait Component {
    fn handle_event(&mut self, key: KeyEvent) -> anyhow::Result<Action>;
}
