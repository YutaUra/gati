use std::io;
use std::path::PathBuf;

use crossterm::{
    event::{self, Event, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
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

    fn load_diff_for_file(&mut self, path: &std::path::Path) {
        if let Some(ref workdir) = self.git_workdir {
            let line_diff = diff::compute_line_diff(workdir, path);
            let unified_diff = diff::compute_unified_diff(workdir, path);
            self.file_viewer.set_diff(line_diff, unified_diff);
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
    let terminal = ratatui::init();
    Ok(terminal)
}

fn restore_terminal() -> anyhow::Result<()> {
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
            if let Event::Key(key) = event::read()? {
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

                let action = match app.focus {
                    Focus::Tree => app.file_tree.handle_event(key)?,
                    Focus::Viewer => app.file_viewer.handle_event(key)?,
                };

                if app.handle_action(action) {
                    return Ok(());
                }
            }
        }

        // Refresh tree and git status when the watcher detects file-system changes
        if let Some(ref watcher) = fs_watcher {
            if watcher.has_changed() {
                let _ = app.file_tree.model.refresh_tree();
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

fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Main layout: tree + viewer above, hint bar at bottom
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let pane_area = main_chunks[0];
    let hint_area = main_chunks[1];

    // Split panes: 30% tree, 70% viewer
    let pane_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(pane_area);

    let tree_area = pane_chunks[0];
    let viewer_area = pane_chunks[1];

    // Update visible height for scroll calculations
    let tree_inner_height = tree_area.height.saturating_sub(2) as usize; // minus borders
    app.file_tree.ensure_visible(tree_inner_height);

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

    // Render key hint bar (or comment input)
    let hints: String = match &app.input_mode {
        InputMode::CommentInput { text, .. } => {
            format!("Comment: {}█  (Enter save  Esc cancel)", text)
        }
        InputMode::LineSelect { .. } => {
            "j/k extend  c comment  Esc cancel".into()
        }
        InputMode::Normal => match app.focus {
            Focus::Tree => {
                if app.file_tree.search.is_some() {
                    "Enter confirm  Esc cancel  ↑/↓ navigate".into()
                } else if app.git_workdir.is_some() {
                    "j/k navigate  h/l fold/unfold  Enter open  / search  g changed  Tab switch  q quit".into()
                } else {
                    "j/k navigate  h/l fold/unfold  Enter open  / search  Tab switch pane  q quit".into()
                }
            }
            Focus::Viewer => {
                let mut h = String::from("j/k cursor  Ctrl-d/Ctrl-u page  c comment  V select  e export");
                if app.git_workdir.is_some() {
                    h.push_str("  d diff");
                }
                h.push_str("  Tab switch  q quit");
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
}
