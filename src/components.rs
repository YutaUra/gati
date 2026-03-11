use crossterm::event::KeyEvent;

/// Action returned by a component's event handler.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// No action needed.
    None,
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
}

/// Trait for TUI components.
pub trait Component {
    fn handle_event(&mut self, key: KeyEvent) -> anyhow::Result<Action>;
}
