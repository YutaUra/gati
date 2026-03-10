use std::io;
use std::path::PathBuf;

use crossterm::{
    event::{self, Event, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    DefaultTerminal, Frame,
};

use crate::comments::CommentStore;
use crate::components::{Action, Component};
use crate::diff;
use crate::file_tree::FileTree;
use crate::file_viewer::FileViewer;
use crate::git_status::GitStatus;
use crate::watcher::FsWatcher;

const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 10;

/// Minimum pane width in columns (absolute floor).
const MIN_PANE_COLS: u16 = 10;
/// Minimum pane width as percentage.
const MIN_PANE_PERCENT: u16 = 10;
/// Maximum tree pane width as percentage.
const MAX_TREE_PERCENT: u16 = 70;
/// Lines to scroll per mouse wheel tick.
const MOUSE_SCROLL_LINES: usize = 5;

/// Compute clamped tree width percentage from a desired column position.
/// Returns a percentage in [min_percent, MAX_TREE_PERCENT] ensuring both panes
/// are at least max(MIN_PANE_PERCENT%, MIN_PANE_COLS columns) wide.
pub fn clamp_tree_percent(desired_cols: u16, terminal_width: u16) -> u16 {
    if terminal_width == 0 {
        return 30;
    }
    let min_cols = (terminal_width * MIN_PANE_PERCENT / 100).max(MIN_PANE_COLS);
    let max_tree_cols = terminal_width * MAX_TREE_PERCENT / 100;
    // Viewer also needs min_cols, so tree max is also terminal_width - min_cols
    let max_tree_cols = max_tree_cols.min(terminal_width.saturating_sub(min_cols));
    let clamped = desired_cols.clamp(min_cols, max_tree_cols);
    (clamped as u32 * 100 / terminal_width as u32) as u16
}

/// Which pane is currently focused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    Tree,
    Viewer,
}

/// Active input mode for the application.
#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    /// Normal navigation mode.
    Normal,
    /// Typing a comment. Stores: file path, start_line, end_line, current text.
    CommentInput {
        file: PathBuf,
        start_line: usize,
        end_line: usize,
        text: String,
    },
    /// Visual line-select mode. Stores: file path, anchor line (1-indexed).
    LineSelect {
        file: PathBuf,
        anchor: usize,
    },
}

pub struct App {
    file_tree: FileTree,
    file_viewer: FileViewer,
    focus: Focus,
    /// Git repository workdir path (None if not inside a git repo).
    git_workdir: Option<PathBuf>,
    /// Root directory being browsed (for periodic git status refresh).
    target_dir: PathBuf,
    /// In-memory comment store for the session.
    pub comment_store: CommentStore,
    /// Current input mode.
    pub input_mode: InputMode,
    /// Tree pane width as percentage of terminal width (default 30).
    pub tree_width_percent: u16,
    /// Whether the user is currently dragging the pane border.
    pub resizing: bool,
    /// Cached right-edge column of the tree pane (set during draw).
    /// Used by mouse handler to detect clicks on the pane border.
    pub border_column: u16,
    /// Whether focus mode is active (tree pane hidden, viewer full width).
    pub focus_mode: bool,
    /// Saved tree width percentage to restore when exiting focus mode.
    pub saved_tree_width_percent: u16,
    /// Cached top Y coordinate of tree pane inner area (after top border).
    pub tree_inner_y: u16,
}

impl App {
    pub fn new(target: &super::StartupTarget) -> anyhow::Result<Self> {
        let git_status = GitStatus::from_dir(&target.dir);

        // Cache git workdir for diff computation
        let git_workdir = git2::Repository::discover(&target.dir)
            .ok()
            .and_then(|r| r.workdir().and_then(|w| w.canonicalize().ok()));

        let mut file_tree = FileTree::new(&target.dir, git_status)?;
        let mut file_viewer = FileViewer::new();

        // If a file was specified, select it and load it
        if let Some(ref selected_file) = target.selected_file {
            if let Some(idx) = file_tree
                .model
                .entries
                .iter()
                .position(|e| e.path == *selected_file)
            {
                file_tree.model.selected = idx;
            }
            file_viewer.load_file(selected_file);
            if let Some(ref workdir) = git_workdir {
                let line_diff = diff::compute_line_diff(workdir, selected_file);
                let unified_diff = diff::compute_unified_diff(workdir, selected_file);
                file_viewer.set_diff(line_diff, unified_diff);
            }
        } else {
            // Auto-preview the first file if cursor starts on a file
            if let Some(entry) = file_tree.model.selected_entry() {
                if !entry.is_directory {
                    let path = entry.path.clone();
                    file_viewer.load_file(&path);
                    if let Some(ref workdir) = git_workdir {
                        let line_diff = diff::compute_line_diff(workdir, &path);
                        let unified_diff = diff::compute_unified_diff(workdir, &path);
                        file_viewer.set_diff(line_diff, unified_diff);
                    }
                }
            }
        }

        Ok(Self {
            file_tree,
            file_viewer,
            focus: Focus::Tree,
            git_workdir,
            target_dir: target.dir.clone(),
            comment_store: CommentStore::new(),
            input_mode: InputMode::Normal,
            tree_width_percent: 30,
            resizing: false,
            border_column: 0,
            focus_mode: false,
            saved_tree_width_percent: 30,
            tree_inner_y: 0,
        })
    }

    fn handle_action(&mut self, action: Action) -> bool {
        match action {
            Action::Quit => return true,
            Action::SwitchFocus => {
                self.focus = match self.focus {
                    Focus::Tree => Focus::Viewer,
                    Focus::Viewer => Focus::Tree,
                };
            }
            Action::FileSelected(path) => {
                self.file_viewer.load_file(&path);
                self.load_diff_for_file(&path);
            }
            Action::FileOpened(path) => {
                self.file_viewer.load_file(&path);
                self.load_diff_for_file(&path);
                self.focus = Focus::Viewer;
            }
            Action::StartComment => {
                if let Some(file) = self.file_viewer.current_file() {
                    let file = file.to_path_buf();
                    let line = self.file_viewer.cursor_line + 1; // 1-indexed
                    // If existing comment on this line, pre-fill text
                    let existing_text = self
                        .comment_store
                        .find_at_line(&file, line)
                        .map(|c| c.text.clone())
                        .unwrap_or_default();
                    self.input_mode = InputMode::CommentInput {
                        file,
                        start_line: line,
                        end_line: line,
                        text: existing_text,
                    };
                }
            }
            Action::StartLineSelect => {
                if let Some(file) = self.file_viewer.current_file() {
                    let file = file.to_path_buf();
                    let line = self.file_viewer.cursor_line + 1; // 1-indexed
                    self.input_mode = InputMode::LineSelect {
                        file,
                        anchor: line,
                    };
                }
            }
            Action::DeleteComment => {
                if let Some(file) = self.file_viewer.current_file() {
                    let file = file.to_path_buf();
                    let line = self.file_viewer.cursor_line + 1;
                    if let Some(comment) = self.comment_store.find_at_line(&file, line) {
                        let start = comment.start_line;
                        let end = comment.end_line;
                        self.comment_store.delete(&file, start, end);
                    }
                }
            }
            Action::ExportComments => {
                self.export_comments();
            }
            Action::None => {}
        }
        false
    }

    fn export_comments(&self) {
        let text = self.comment_store.export();
        if text.is_empty() {
            return;
        }
        // Try to copy to clipboard; ignore errors silently
        let _ = cli_clipboard::set_contents(text);
    }

    /// Refresh state when the filesystem watcher detects changes.
    /// Re-reads the file tree and recomputes diff for the currently viewed file.
    fn refresh_on_fs_change(&mut self) {
        let _ = self.file_tree.model.refresh_tree();
        if let Some(path) = self.file_viewer.current_file().map(|p| p.to_path_buf()) {
            self.load_diff_for_file(&path);
        }
    }

    fn load_diff_for_file(&mut self, path: &std::path::Path) {
        if let Some(ref workdir) = self.git_workdir {
            if let Some((line_diff, unified_diff)) = diff::compute_diffs(workdir, path) {
                self.file_viewer.set_diff(Some(line_diff), Some(unified_diff));
            } else {
                self.file_viewer.set_diff(None, None);
            }
        }
    }
}

pub fn run(target: &super::StartupTarget) -> anyhow::Result<()> {
    install_panic_hook();
    let mut terminal = init_terminal()?;

    // Check minimum terminal size
    let size = terminal.size()?;
    if size.width < MIN_WIDTH || size.height < MIN_HEIGHT {
        restore_terminal()?;
        anyhow::bail!(
            "Terminal too small ({}x{}). Minimum size is {}x{}.",
            size.width,
            size.height,
            MIN_WIDTH,
            MIN_HEIGHT
        );
    }

    let mut app = App::new(target)?;
    let result = event_loop(&mut terminal, &mut app);
    restore_terminal()?;
    result
}

fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));
}

fn init_terminal() -> anyhow::Result<DefaultTerminal> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    io::stdout().execute(EnableMouseCapture)?;
    let terminal = ratatui::init();
    Ok(terminal)
}

fn restore_terminal() -> anyhow::Result<()> {
    io::stdout().execute(DisableMouseCapture)?;
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> anyhow::Result<()> {
    use std::time::Duration;

    // Start file-system watcher for live git status updates.
    // Debounce at 500ms so rapid saves don't cause excessive recomputation.
    let fs_watcher = FsWatcher::new(&app.target_dir, Duration::from_millis(500));

    // Short poll timeout to check watcher flag frequently
    let poll_timeout = Duration::from_millis(200);

    loop {
        terminal.draw(|frame| draw(frame, app))?;

        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Mouse(mouse) => {
                    handle_mouse(app, mouse, terminal.size()?.width);
                    continue;
                }
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    // Handle modal input modes first
                    match &app.input_mode {
                        InputMode::CommentInput { .. } => {
                            handle_comment_input(app, key);
                            continue;
                        }
                        InputMode::LineSelect { .. } => {
                            if handle_line_select(app, key) {
                                continue;
                            }
                            // If not handled, fall through to normal
                        }
                        InputMode::Normal => {}
                    }

                    // 'b' toggles focus mode in Normal mode (both panes)
                    if key.code == crossterm::event::KeyCode::Char('b')
                        && key.modifiers.is_empty()
                    {
                        toggle_focus_mode(app);
                        continue;
                    }

                    let action = match app.focus {
                        Focus::Tree => app.file_tree.handle_event(key)?,
                        Focus::Viewer => app.file_viewer.handle_event(key)?,
                    };

                    if app.handle_action(action) {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }

        // Refresh tree, git status, and diff when the watcher detects file-system changes
        if let Some(ref watcher) = fs_watcher {
            if watcher.has_changed() {
                app.refresh_on_fs_change();
            }
        }
    }
}

fn handle_comment_input(app: &mut App, key: crossterm::event::KeyEvent) {
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
            {
                if !text.is_empty() {
                    app.comment_store
                        .add(file, start_line, end_line, text.clone());
                }
            }
            app.input_mode = InputMode::Normal;
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

fn handle_line_select(app: &mut App, key: crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            app.file_viewer.cursor_down();
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.file_viewer.cursor_up();
            true
        }
        KeyCode::Char('c') => {
            // Open comment input for the selected range
            if let InputMode::LineSelect { ref file, anchor } = app.input_mode {
                let current = app.file_viewer.cursor_line + 1; // 1-indexed
                let start = anchor.min(current);
                let end = anchor.max(current);
                let file = file.clone();
                let existing_text = app
                    .comment_store
                    .find_at_line(&file, start)
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

fn toggle_focus_mode(app: &mut App) {
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

fn handle_mouse(app: &mut App, mouse: crossterm::event::MouseEvent, terminal_width: u16) {
    use crossterm::event::{MouseButton, MouseEventKind};

    let min_cols = (terminal_width * MIN_PANE_PERCENT / 100).max(MIN_PANE_COLS);

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
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
                    if entry_idx < app.file_tree.model.entries.len() {
                        app.file_tree.model.selected = entry_idx;
                        if app.file_tree.model.entries[entry_idx].is_directory {
                            let _ = app.file_tree.model.toggle_expand();
                        } else {
                            let path = app.file_tree.model.entries[entry_idx].path.clone();
                            app.file_viewer.load_file(&path);
                            if let Some(ref workdir) = app.git_workdir {
                                let line_diff = diff::compute_line_diff(workdir, &path);
                                let unified_diff = diff::compute_unified_diff(workdir, &path);
                                app.file_viewer.set_diff(line_diff, unified_diff);
                            }
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
                }
            } else {
                app.focus = Focus::Viewer;
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if !app.resizing {
                return;
            }
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
        }
        MouseEventKind::Up(MouseButton::Left) => {
            app.resizing = false;
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

fn draw(frame: &mut Frame, app: &mut App) {
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

    // Update file viewer's comments for current file
    if let Some(file) = app.file_viewer.current_file() {
        let file = file.to_path_buf();
        app.file_viewer.comments = app
            .comment_store
            .for_file(&file)
            .into_iter()
            .cloned()
            .collect();
    } else {
        app.file_viewer.comments.clear();
    }

    // Render panes
    let buf = frame.buffer_mut();
    app.file_tree
        .render_to_buffer(tree_area, buf, app.focus == Focus::Tree);
    app.file_viewer
        .render_to_buffer(viewer_area, buf, app.focus == Focus::Viewer);

    // Highlight border when resizing
    if app.resizing && tree_area.right() > 0 {
        let border_x = tree_area.right() - 1;
        for y in tree_area.top()..tree_area.bottom() {
            if let Some(cell) = buf.cell_mut((border_x, y)) {
                cell.set_style(Style::default().fg(Color::Yellow));
            }
        }
    }

    // Render key hint bar (or comment input)
    let hints: String = match &app.input_mode {
        InputMode::CommentInput { text, .. } => {
            format!("Comment: {}█  (Enter save  Esc cancel)", text)
        }
        InputMode::LineSelect { .. } => {
            "j/k extend  c comment  Esc cancel".into()
        }
        InputMode::Normal if app.focus_mode => {
            "j/k cursor  h/l scroll  Ctrl-d/Ctrl-u page  c comment  V select  e export  b restore tree  q quit".into()
        }
        InputMode::Normal => match app.focus {
            Focus::Tree => {
                if app.file_tree.search.is_some() {
                    "Enter confirm  Esc cancel  ↑/↓ navigate".into()
                } else if app.git_workdir.is_some() {
                    "j/k navigate  h/l fold/unfold  Enter open  / search  g changed  b focus  Tab switch  q quit".into()
                } else {
                    "j/k navigate  h/l fold/unfold  Enter open  / search  b focus  Tab switch pane  q quit".into()
                }
            }
            Focus::Viewer => {
                let mut h = String::from("j/k cursor  h/l scroll  Ctrl-d/Ctrl-u page  c comment  V select  e export");
                if app.git_workdir.is_some() {
                    h.push_str("  d diff");
                }
                h.push_str("  b focus  Tab switch  q quit");
                h
            }
        },
    };

    let hint_line = Line::from(Span::styled(
        &hints,
        Style::default().fg(Color::DarkGray),
    ));
    buf.set_line(hint_area.x, hint_area.y, &hint_line, hint_area.width);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_dir(files: &[&str], dirs: &[&str]) -> TempDir {
        let tmp = TempDir::new().unwrap();
        for d in dirs {
            fs::create_dir_all(tmp.path().join(d)).unwrap();
        }
        for f in files {
            let path = tmp.path().join(f);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, format!("content of {f}")).unwrap();
        }
        tmp
    }

    fn make_target(dir: &std::path::Path, file: Option<PathBuf>) -> crate::StartupTarget {
        crate::StartupTarget {
            dir: dir.to_path_buf(),
            selected_file: file,
        }
    }

    // 5.3: Focus switching
    #[test]
    fn initial_focus_is_tree() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let app = App::new(&make_target(tmp.path(), None)).unwrap();
        assert_eq!(app.focus, Focus::Tree);
    }

    #[test]
    fn switch_focus_toggles_between_panes() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();

        app.handle_action(Action::SwitchFocus);
        assert_eq!(app.focus, Focus::Viewer);

        app.handle_action(Action::SwitchFocus);
        assert_eq!(app.focus, Focus::Tree);
    }

    #[test]
    fn file_opened_switches_focus_to_viewer() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        assert_eq!(app.focus, Focus::Tree);

        let path = tmp.path().join("file.rs");
        app.handle_action(Action::FileOpened(path));
        assert_eq!(app.focus, Focus::Viewer);
    }

    // 6.1: Cursor movement triggers preview
    #[test]
    fn file_selected_loads_file_in_viewer() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();

        let path = tmp.path().join("file.rs");
        app.handle_action(Action::FileSelected(path));

        match &app.file_viewer.content {
            crate::file_viewer::ViewerContent::File { lines, .. } => {
                assert_eq!(lines[0], "content of file.rs");
            }
            other => panic!("Expected File content, got {:?}", other),
        }
    }

    // Startup with selected file
    #[test]
    fn startup_with_file_selects_and_previews() {
        let tmp = setup_dir(&["a.rs", "b.rs"], &[]);
        let file_path = tmp.path().join("b.rs");
        let app = App::new(&make_target(tmp.path(), Some(file_path.clone()))).unwrap();

        // b.rs should be loaded in viewer
        match &app.file_viewer.content {
            crate::file_viewer::ViewerContent::File { path, .. } => {
                assert_eq!(path, &file_path);
            }
            other => panic!("Expected File content, got {:?}", other),
        }
    }

    #[test]
    fn quit_action_returns_true() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        assert!(app.handle_action(Action::Quit));
    }

    // Comment workflow
    #[test]
    fn start_comment_enters_comment_input_mode() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let path = tmp.path().join("file.rs");
        let mut app = App::new(&make_target(tmp.path(), Some(path.clone()))).unwrap();
        app.focus = Focus::Viewer;

        app.handle_action(Action::StartComment);

        match &app.input_mode {
            InputMode::CommentInput {
                start_line,
                end_line,
                text,
                ..
            } => {
                assert_eq!(*start_line, 1); // cursor at line 0 → 1-indexed
                assert_eq!(*end_line, 1);
                assert!(text.is_empty());
            }
            other => panic!("Expected CommentInput, got {:?}", other),
        }
    }

    #[test]
    fn comment_input_saves_on_enter() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let path = tmp.path().join("file.rs");
        let mut app = App::new(&make_target(tmp.path(), Some(path.clone()))).unwrap();
        app.input_mode = InputMode::CommentInput {
            file: path.clone(),
            start_line: 1,
            end_line: 1,
            text: "Fix this".into(),
        };

        handle_comment_input(
            &mut app,
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            ),
        );

        assert_eq!(app.input_mode, InputMode::Normal);
        let comments = app.comment_store.for_file(&path);
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "Fix this");
    }

    #[test]
    fn comment_input_cancels_on_esc() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let path = tmp.path().join("file.rs");
        let mut app = App::new(&make_target(tmp.path(), Some(path.clone()))).unwrap();
        app.input_mode = InputMode::CommentInput {
            file: path.clone(),
            start_line: 1,
            end_line: 1,
            text: "Draft".into(),
        };

        handle_comment_input(
            &mut app,
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Esc,
                crossterm::event::KeyModifiers::NONE,
            ),
        );

        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.comment_store.is_empty());
    }

    #[test]
    fn comment_input_typing_appends_chars() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let path = tmp.path().join("file.rs");
        let mut app = App::new(&make_target(tmp.path(), Some(path.clone()))).unwrap();
        app.input_mode = InputMode::CommentInput {
            file: path,
            start_line: 1,
            end_line: 1,
            text: String::new(),
        };

        handle_comment_input(
            &mut app,
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('H'),
                crossterm::event::KeyModifiers::NONE,
            ),
        );
        handle_comment_input(
            &mut app,
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('i'),
                crossterm::event::KeyModifiers::NONE,
            ),
        );

        if let InputMode::CommentInput { ref text, .. } = app.input_mode {
            assert_eq!(text, "Hi");
        } else {
            panic!("Expected CommentInput mode");
        }
    }

    #[test]
    fn delete_comment_removes_comment_at_cursor() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let path = tmp.path().join("file.rs");
        let mut app = App::new(&make_target(tmp.path(), Some(path.clone()))).unwrap();
        app.comment_store.add(&path, 1, 1, "Delete me".into());
        app.focus = Focus::Viewer;

        app.handle_action(Action::DeleteComment);

        assert!(app.comment_store.is_empty());
    }

    #[test]
    fn start_line_select_enters_line_select_mode() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let path = tmp.path().join("file.rs");
        let mut app = App::new(&make_target(tmp.path(), Some(path.clone()))).unwrap();
        app.focus = Focus::Viewer;

        app.handle_action(Action::StartLineSelect);

        match &app.input_mode {
            InputMode::LineSelect { anchor, .. } => {
                assert_eq!(*anchor, 1);
            }
            other => panic!("Expected LineSelect, got {:?}", other),
        }
    }

    // Resizable panes
    #[test]
    fn default_tree_width_percent_is_30() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let app = App::new(&make_target(tmp.path(), None)).unwrap();
        assert_eq!(app.tree_width_percent, 30);
        assert!(!app.resizing);
    }

    #[test]
    fn clamp_tree_percent_enforces_minimum() {
        // Terminal 100 cols wide: min = max(10, 10) = 10 cols → 10%
        assert_eq!(clamp_tree_percent(5, 100), 10);
        // Terminal 80 cols wide: min = max(8, 10) = 10 cols → 12%
        assert_eq!(clamp_tree_percent(3, 80), 12);
    }

    #[test]
    fn clamp_tree_percent_enforces_maximum() {
        // Terminal 100 cols wide: max = min(70, 100-10) = 70 cols → 70%
        assert_eq!(clamp_tree_percent(90, 100), 70);
        // Terminal 40 cols wide: max = min(28, 40-10) = 28 cols → 70%
        assert_eq!(clamp_tree_percent(35, 40), 70);
    }

    #[test]
    fn clamp_tree_percent_allows_valid_values() {
        // 50 cols on 100 terminal → 50%
        assert_eq!(clamp_tree_percent(50, 100), 50);
        // 30 cols on 100 terminal → 30%
        assert_eq!(clamp_tree_percent(30, 100), 30);
    }

    #[test]
    fn resizing_flag_can_be_toggled() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        assert!(!app.resizing);
        app.resizing = true;
        assert!(app.resizing);
        app.resizing = false;
        assert!(!app.resizing);
    }

    // Mouse drag resize
    fn make_mouse_event(
        kind: crossterm::event::MouseEventKind,
        col: u16,
    ) -> crossterm::event::MouseEvent {
        crossterm::event::MouseEvent {
            kind,
            column: col,
            row: 5,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    #[test]
    fn mouse_down_near_border_starts_resizing() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.border_column = 30;

        // Click exactly on border
        let ev = make_mouse_event(crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left), 30);
        handle_mouse(&mut app, ev, 100);
        assert!(app.resizing);
    }

    #[test]
    fn mouse_down_away_from_border_does_not_start_resizing() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.border_column = 30;

        // Click 5 columns away from border
        let ev = make_mouse_event(crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left), 25);
        handle_mouse(&mut app, ev, 100);
        assert!(!app.resizing);
    }

    #[test]
    fn mouse_drag_while_resizing_updates_percent() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.resizing = true;

        // Drag to column 50 on 100-col terminal → 50%
        let ev = make_mouse_event(crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left), 50);
        handle_mouse(&mut app, ev, 100);
        assert_eq!(app.tree_width_percent, 50);
    }

    #[test]
    fn mouse_drag_without_resizing_does_nothing() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        assert!(!app.resizing);

        let ev = make_mouse_event(crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left), 50);
        handle_mouse(&mut app, ev, 100);
        assert_eq!(app.tree_width_percent, 30); // unchanged
    }

    #[test]
    fn mouse_up_stops_resizing() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.resizing = true;

        let ev = make_mouse_event(crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left), 50);
        handle_mouse(&mut app, ev, 100);
        assert!(!app.resizing);
    }

    // Focus mode tests
    #[test]
    fn default_focus_mode_is_false() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let app = App::new(&make_target(tmp.path(), None)).unwrap();
        assert!(!app.focus_mode);
        assert_eq!(app.saved_tree_width_percent, 30);
    }

    #[test]
    fn toggle_focus_mode_entering_saves_tree_width() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.tree_width_percent = 40;

        toggle_focus_mode(&mut app);

        assert!(app.focus_mode);
        assert_eq!(app.saved_tree_width_percent, 40);
        assert_eq!(app.focus, Focus::Viewer);
    }

    #[test]
    fn toggle_focus_mode_exiting_restores_tree_width() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.tree_width_percent = 40;

        toggle_focus_mode(&mut app); // enter
        assert!(app.focus_mode);

        toggle_focus_mode(&mut app); // exit
        assert!(!app.focus_mode);
        assert_eq!(app.tree_width_percent, 40);
    }

    #[test]
    fn drag_below_minimum_enters_focus_mode() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.resizing = true;
        app.tree_width_percent = 20;

        // Drag to column 5, well below min_cols (10) on 100-col terminal
        let ev = make_mouse_event(crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left), 5);
        handle_mouse(&mut app, ev, 100);

        assert!(app.focus_mode);
        assert_eq!(app.saved_tree_width_percent, 20);
        assert_eq!(app.focus, Focus::Viewer);
    }

    #[test]
    fn drag_from_left_edge_in_focus_mode_restores() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.focus_mode = true;
        app.saved_tree_width_percent = 30;

        // Click at left edge to start resize
        let down = make_mouse_event(crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left), 0);
        handle_mouse(&mut app, down, 100);
        assert!(app.resizing);

        // Drag past min_cols to restore
        let drag = make_mouse_event(crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left), 25);
        handle_mouse(&mut app, drag, 100);

        assert!(!app.focus_mode);
        assert_eq!(app.tree_width_percent, 25);
    }

    // Mouse wheel scroll tests
    fn create_long_file(dir: &std::path::Path) -> PathBuf {
        let path = dir.join("long.rs");
        let content: String = (0..100).map(|i| format!("line {i}\n")).collect();
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn scroll_down_over_viewer_moves_scroll_offset() {
        let tmp = setup_dir(&["placeholder"], &[]);
        let long_file = create_long_file(tmp.path());
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.file_viewer.load_file(&long_file);
        app.border_column = 30;
        let initial_offset = app.file_viewer.scroll_offset;

        // Scroll down at column 50 (over viewer pane)
        let ev = make_mouse_event(crossterm::event::MouseEventKind::ScrollDown, 50);
        handle_mouse(&mut app, ev, 100);

        assert!(app.file_viewer.scroll_offset > initial_offset);
        assert_eq!(app.file_viewer.scroll_offset, initial_offset + MOUSE_SCROLL_LINES);
    }

    #[test]
    fn scroll_up_over_viewer_moves_scroll_offset() {
        let tmp = setup_dir(&["placeholder"], &[]);
        let long_file = create_long_file(tmp.path());
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.file_viewer.load_file(&long_file);
        app.border_column = 30;
        app.file_viewer.scroll_offset = 20;

        let ev = make_mouse_event(crossterm::event::MouseEventKind::ScrollUp, 50);
        handle_mouse(&mut app, ev, 100);

        assert_eq!(app.file_viewer.scroll_offset, 20 - MOUSE_SCROLL_LINES);
    }

    #[test]
    fn scroll_over_tree_pane_does_not_affect_viewer() {
        let tmp = setup_dir(&["placeholder"], &[]);
        let long_file = create_long_file(tmp.path());
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.file_viewer.load_file(&long_file);
        app.border_column = 30;
        let initial_offset = app.file_viewer.scroll_offset;

        // Scroll at column 10 (over tree pane, before border)
        let ev = make_mouse_event(crossterm::event::MouseEventKind::ScrollDown, 10);
        handle_mouse(&mut app, ev, 100);

        assert_eq!(app.file_viewer.scroll_offset, initial_offset);
    }

    // Mouse horizontal scroll tests
    fn make_mouse_event_with_modifiers(
        kind: crossterm::event::MouseEventKind,
        col: u16,
        modifiers: crossterm::event::KeyModifiers,
    ) -> crossterm::event::MouseEvent {
        crossterm::event::MouseEvent {
            kind,
            column: col,
            row: 5,
            modifiers,
        }
    }

    #[test]
    fn shift_scroll_down_over_viewer_scrolls_right() {
        let tmp = setup_dir(&["placeholder"], &[]);
        let long_file = create_long_file(tmp.path());
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.file_viewer.load_file(&long_file);
        app.border_column = 30;

        let ev = make_mouse_event_with_modifiers(
            crossterm::event::MouseEventKind::ScrollDown,
            50,
            crossterm::event::KeyModifiers::SHIFT,
        );
        handle_mouse(&mut app, ev, 100);

        assert_eq!(app.file_viewer.h_scroll, crate::file_viewer::H_SCROLL_AMOUNT);
    }

    #[test]
    fn shift_scroll_up_over_viewer_scrolls_left() {
        let tmp = setup_dir(&["placeholder"], &[]);
        let long_file = create_long_file(tmp.path());
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.file_viewer.load_file(&long_file);
        app.file_viewer.h_scroll = 8;
        app.border_column = 30;

        let ev = make_mouse_event_with_modifiers(
            crossterm::event::MouseEventKind::ScrollUp,
            50,
            crossterm::event::KeyModifiers::SHIFT,
        );
        handle_mouse(&mut app, ev, 100);

        assert_eq!(app.file_viewer.h_scroll, 4);
    }

    #[test]
    fn shift_scroll_over_tree_does_not_h_scroll() {
        let tmp = setup_dir(&["placeholder"], &[]);
        let long_file = create_long_file(tmp.path());
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.file_viewer.load_file(&long_file);
        app.border_column = 30;

        let ev = make_mouse_event_with_modifiers(
            crossterm::event::MouseEventKind::ScrollDown,
            10,
            crossterm::event::KeyModifiers::SHIFT,
        );
        handle_mouse(&mut app, ev, 100);

        assert_eq!(app.file_viewer.h_scroll, 0);
    }

    // Mouse scroll over tree pane tests
    #[test]
    fn scroll_down_over_tree_pane_scrolls_tree() {
        // Create enough files so the tree can scroll
        let files: Vec<String> = (0..30).map(|i| format!("file{i:02}.rs")).collect();
        let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        let tmp = setup_dir(&file_refs, &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.border_column = 30;
        app.file_tree.visible_height = 20;
        let initial = app.file_tree.scroll_offset;

        // Scroll at column 10 (over tree pane)
        let ev = make_mouse_event(crossterm::event::MouseEventKind::ScrollDown, 10);
        handle_mouse(&mut app, ev, 100);

        assert!(app.file_tree.scroll_offset > initial);
    }

    #[test]
    fn scroll_up_over_tree_pane_scrolls_tree() {
        let files: Vec<String> = (0..30).map(|i| format!("file{i:02}.rs")).collect();
        let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        let tmp = setup_dir(&file_refs, &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.border_column = 30;
        app.file_tree.visible_height = 20;
        app.file_tree.scroll_offset = 10;
        app.file_tree.model.selected = 15;

        let ev = make_mouse_event(crossterm::event::MouseEventKind::ScrollUp, 10);
        handle_mouse(&mut app, ev, 100);

        assert!(app.file_tree.scroll_offset < 10);
    }

    fn make_mouse_event_at(
        kind: crossterm::event::MouseEventKind,
        col: u16,
        row: u16,
    ) -> crossterm::event::MouseEvent {
        crossterm::event::MouseEvent {
            kind,
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    // Mouse click on tree entry tests
    #[test]
    fn click_on_tree_file_entry_selects_it() {
        let tmp = setup_dir(&["a.rs", "b.rs", "c.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.border_column = 30;
        app.tree_inner_y = 1; // inner area starts at row 1 (after top border)
        app.file_tree.visible_height = 20;
        app.file_tree.scroll_offset = 0;
        assert_eq!(app.file_tree.model.selected, 0);

        // Click on row 3 → inner row 2 → entry index 2 (c.rs)
        let ev = make_mouse_event_at(
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            5, 3,
        );
        handle_mouse(&mut app, ev, 100);

        assert_eq!(app.file_tree.model.selected, 2);
    }

    #[test]
    fn click_on_tree_directory_toggles_expand() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.border_column = 30;
        app.tree_inner_y = 1;
        app.file_tree.visible_height = 20;
        app.file_tree.scroll_offset = 0;
        // Entry 0 is "sub" directory
        assert!(app.file_tree.model.entries[0].is_directory);
        assert!(!app.file_tree.model.entries[0].is_expanded);

        // Click on row 1 → inner row 0 → entry index 0 (sub/)
        let ev = make_mouse_event_at(
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            5, 1,
        );
        handle_mouse(&mut app, ev, 100);

        assert!(app.file_tree.model.entries[0].is_expanded, "clicking directory should expand it");
    }

    #[test]
    fn click_on_expanded_directory_collapses_it() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.border_column = 30;
        app.tree_inner_y = 1;
        app.file_tree.visible_height = 20;
        app.file_tree.scroll_offset = 0;

        // Expand sub first
        app.file_tree.model.selected = 0;
        app.file_tree.model.toggle_expand().unwrap();
        assert!(app.file_tree.model.entries[0].is_expanded);

        // Click on row 1 → entry 0 (sub/, expanded)
        let ev = make_mouse_event_at(
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            5, 1,
        );
        handle_mouse(&mut app, ev, 100);

        assert!(!app.file_tree.model.entries[0].is_expanded, "clicking expanded directory should collapse it");
    }

    #[test]
    fn click_on_tree_with_scroll_offset_selects_correct_entry() {
        let files: Vec<String> = (0..20).map(|i| format!("file{i:02}.rs")).collect();
        let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        let tmp = setup_dir(&file_refs, &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.border_column = 30;
        app.tree_inner_y = 1;
        app.file_tree.visible_height = 10;
        app.file_tree.scroll_offset = 5;

        // Click on row 1 → inner row 0 → scroll_offset + 0 = entry 5
        let ev = make_mouse_event_at(
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            5, 1,
        );
        handle_mouse(&mut app, ev, 100);

        assert_eq!(app.file_tree.model.selected, 5);
    }

    #[test]
    fn click_beyond_tree_entries_is_noop() {
        let tmp = setup_dir(&["a.rs", "b.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.border_column = 30;
        app.tree_inner_y = 1;
        app.file_tree.visible_height = 20;
        app.file_tree.scroll_offset = 0;
        app.file_tree.model.selected = 0;

        // Click on row 10 → inner row 9 → entry 9, but only 2 entries exist
        let ev = make_mouse_event_at(
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            5, 10,
        );
        handle_mouse(&mut app, ev, 100);

        assert_eq!(app.file_tree.model.selected, 0, "selection should not change");
    }

    // Click-to-focus tests
    #[test]
    fn click_on_tree_pane_focuses_tree() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.border_column = 30;
        app.focus = Focus::Viewer; // start focused on viewer

        let ev = make_mouse_event(crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left), 10);
        handle_mouse(&mut app, ev, 100);

        assert_eq!(app.focus, Focus::Tree);
    }

    #[test]
    fn click_on_viewer_pane_focuses_viewer() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.border_column = 30;
        app.focus = Focus::Tree; // start focused on tree

        let ev = make_mouse_event(crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left), 50);
        handle_mouse(&mut app, ev, 100);

        assert_eq!(app.focus, Focus::Viewer);
    }

    #[test]
    fn click_on_border_does_not_change_focus() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.border_column = 30;
        app.focus = Focus::Tree;

        // Click on border (within ±1 of border_column) starts resize, not focus change
        let ev = make_mouse_event(crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left), 30);
        handle_mouse(&mut app, ev, 100);

        assert_eq!(app.focus, Focus::Tree); // unchanged
    }

    // Git diff refresh on filesystem change
    fn setup_git_repo(files: &[(&str, &str)]) -> TempDir {
        let tmp = TempDir::new().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();

        for (name, content) in files {
            let path = tmp.path().join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, content).unwrap();
        }

        // Initial commit with all files
        let mut index = repo.index().unwrap();
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = git2::Signature::now("test", "test@test.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();

        tmp
    }

    #[test]
    fn refresh_on_fs_change_recomputes_diff_for_current_file() {
        let tmp = setup_git_repo(&[("file.rs", "line1\nline2\n")]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();

        // Select the file to load it into the viewer with diff
        let file_path = tmp.path().join("file.rs");
        app.handle_action(Action::FileSelected(file_path.clone()));

        // Initially committed, so no diff markers (all unchanged)
        let has_changes = app.file_viewer.line_diff.as_ref().map_or(false, |d| {
            d.lines.iter().any(|k| *k != crate::diff::DiffLineKind::Unchanged)
        });
        assert!(!has_changes, "all lines should be unchanged after initial commit");

        // Simulate external modification (adds a new line)
        fs::write(&file_path, "line1\nline2\nline3\n").unwrap();

        // Call the refresh method (simulating what watcher triggers)
        app.refresh_on_fs_change();

        // Diff should now show the new line as Added
        let diff = app
            .file_viewer
            .line_diff
            .as_ref()
            .expect("diff should be present after refresh");
        assert_eq!(
            diff.line_kind(3),
            crate::diff::DiffLineKind::Added,
            "new line 3 should be marked as Added after fs change refresh"
        );
    }
}
