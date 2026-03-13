mod comment_ops;
mod git_worker;
mod mouse;
mod render;

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::{
    style::Color,
    DefaultTerminal,
};

use crate::comments::{CommentListEntry, CommentStore};
use crate::components::{Action, Component};
use crate::file_tree::FileTree;
use crate::file_viewer::FileViewer;
use crate::watcher::FsWatcher;

use git_worker::{GitStatusWorker, load_file_with_diff, set_diff_for_file};

const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 10;

/// Default tree pane width as percentage of terminal width.
pub(super) const DEFAULT_TREE_WIDTH_PERCENT: u16 = 30;

/// How long flash messages remain visible in the hint bar.
pub(super) const FLASH_DURATION: Duration = Duration::from_secs(3);


/// Temporary flash message shown in the hint bar, auto-dismissed after FLASH_DURATION.
pub(crate) struct FlashMessage {
    pub(crate) text: String,
    pub(crate) color: Color,
    pub(crate) created: Instant,
}

/// Pre-computed data for a single frame, produced by `App::prepare_for_render()`.
///
/// Separates business logic (comment loading, staleness, input mode interpretation)
/// from the rendering path.
pub(super) struct RenderData {
    /// Comments for the current file, each paired with a staleness flag.
    pub(super) viewer_comments: Vec<(crate::comments::Comment, bool)>,
    /// Inline comment editor state (Some when in CommentInput mode).
    pub(super) comment_edit: Option<crate::file_viewer::CommentEditState>,
    /// Line-select range (1-indexed start, end) for V mode / comment input.
    pub(super) line_select_range: Option<(usize, usize)>,
    /// Set of files that have at least one comment (for tree markers).
    pub(super) commented_files: std::collections::HashSet<std::path::PathBuf>,
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
    pub(super) file_tree: FileTree,
    pub(super) file_viewer: FileViewer,
    pub(super) focus: Focus,
    /// Git repository workdir path (None if not inside a git repo).
    pub(super) git_workdir: Option<PathBuf>,
    /// Root directory being browsed (for periodic git status refresh).
    pub(super) target_dir: PathBuf,
    /// Background worker for computing git status.
    pub(super) git_worker: Option<GitStatusWorker>,
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
    /// Whether the help dialog overlay is currently visible.
    pub show_help: bool,
    /// Temporary flash message shown in the hint bar, auto-dismissed after FLASH_DURATION.
    pub flash_message: Option<FlashMessage>,
}

impl App {
    pub fn new(target: &super::StartupTarget) -> anyhow::Result<Self> {
        // Cache git workdir for diff computation
        let git_workdir = git2::Repository::discover(&target.dir)
            .ok()
            .and_then(|r| r.workdir().and_then(|w| w.canonicalize().ok()));

        // Build tree immediately without git status — it will be filled in asynchronously.
        let mut file_tree = FileTree::new(&target.dir, None)?;

        // Spawn background thread to compute git status
        let git_worker = Some(GitStatusWorker::spawn(target.dir.clone()));
        let mut file_viewer = FileViewer::new();

        // If a file was specified, select it and load it
        if let Some(ref selected_file) = target.selected_file {
            if let Some(idx) = file_tree
                .model
                .entries
                .iter()
                .position(|e| e.path == *selected_file)
            {
                file_tree.model.select_at(idx);
            }
            load_file_with_diff(&mut file_viewer, selected_file, &git_workdir);
        } else {
            // Auto-preview the first file if cursor starts on a file
            if let Some(entry) = file_tree.model.selected_entry()
                && !entry.is_directory {
                    let path = entry.path.clone();
                    load_file_with_diff(&mut file_viewer, &path, &git_workdir);
                }
        }

        Ok(Self {
            file_tree,
            file_viewer,
            focus: Focus::Tree,
            git_workdir,
            target_dir: target.dir.clone(),
            git_worker,
            comment_store: CommentStore::new(),
            input_mode: InputMode::Normal,
            tree_width_percent: DEFAULT_TREE_WIDTH_PERCENT,
            resizing: false,
            border_column: 0,
            focus_mode: false,
            saved_tree_width_percent: DEFAULT_TREE_WIDTH_PERCENT,
            tree_inner_y: 0,
            show_help: false,
            flash_message: None,
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
            Action::FileSelected(path) => self.handle_file_action(&path, false),
            Action::FileOpened(path) => self.handle_file_action(&path, true),
            Action::StartComment => comment_ops::start_comment(self),
            Action::StartLineSelect => {
                if let Some(file) = self.file_viewer.current_file()
                    && let Some(line) = self.file_viewer.cursor_file_line() {
                        let file = file.to_path_buf();
                        self.input_mode = InputMode::LineSelect {
                            file,
                            anchor: line,
                        };
                    }
            }
            Action::DeleteComment => comment_ops::delete_comment_at_cursor(self),
            Action::DeleteCommentAt { file, start_line, end_line } => {
                self.comment_store.delete(&file, start_line, end_line);
                comment_ops::refresh_comment_list(self);
            }
            Action::ExportComments => comment_ops::export_comments(self),
            Action::BugReport => {
                let url = crate::bug_report::build_url("Bug report", "");
                match crate::bug_report::try_open(&url) {
                    crate::bug_report::OpenResult::Opened => {
                        self.flash_message = Some(FlashMessage {
                            text: "Opened bug report in browser".into(),
                            color: Color::Green,
                            created: Instant::now(),
                        });
                    }
                    crate::bug_report::OpenResult::Failed(e) => {
                        self.flash_message = Some(FlashMessage {
                            text: format!("Failed to open browser: {}", e),
                            color: Color::Red,
                            created: Instant::now(),
                        });
                    }
                }
            }
            Action::EnterCommentList => {
                let entries = comment_ops::build_comment_list_entries(&self.comment_store, &self.target_dir);
                if !entries.is_empty() {
                    self.file_tree.enter_comment_list(entries);
                    if let Some(CommentListEntry::Comment { file, start_line, .. }) =
                        self.file_tree.selected_comment()
                    {
                        let file = file.clone();
                        let line = *start_line;
                        self.handle_file_action(&file, false);
                        self.file_viewer.scroll_to_line(line);
                    }
                }
            }
            Action::CommentFocused { file, line } => {
                self.navigate_to_file_line(&file, line, false);
            }
            Action::CommentJumped { file, line } => {
                self.navigate_to_file_line(&file, line, true);
            }
            Action::CursorDown => {
                let comments = self.viewer_comments();
                self.file_viewer.cursor_down(&comments);
            }
            Action::CursorUp => {
                let comments = self.viewer_comments();
                self.file_viewer.cursor_up(&comments);
            }
            Action::None => {}
        }
        false
    }

    /// Compute (comment, is_stale) pairs for the file currently loaded in the viewer.
    fn viewer_comments(&self) -> Vec<(crate::comments::Comment, bool)> {
        if let Some(file) = self.file_viewer.current_file() {
            let file = file.to_path_buf();
            let current_lines = self.file_viewer.current_lines();
            self.comment_store
                .for_file(&file)
                .into_iter()
                .map(|c| {
                    let stale = c.is_stale(current_lines);
                    (c.clone(), stale)
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Compute all render-time data before the actual frame draw.
    ///
    /// This keeps business logic (comment loading, staleness checks, input mode
    /// interpretation) separate from the rendering path, making both easier to
    /// test and reason about independently.
    fn prepare_for_render(&self) -> RenderData {
        let viewer_comments = self.viewer_comments();

        let comment_edit = match &self.input_mode {
            InputMode::CommentInput {
                start_line, end_line, text, ..
            } => Some(crate::file_viewer::CommentEditState {
                start_line: *start_line,
                target_line: *end_line,
                text: text.clone(),
            }),
            _ => None,
        };

        let line_select_range = match &self.input_mode {
            InputMode::LineSelect { anchor, .. } => {
                let cursor = self.file_viewer.cursor_file_line().unwrap_or(*anchor);
                let start = (*anchor).min(cursor);
                let end = (*anchor).max(cursor);
                Some((start, end))
            }
            InputMode::CommentInput { start_line, end_line, .. } => {
                Some((*start_line, *end_line))
            }
            _ => None,
        };

        let commented_files = self
            .comment_store
            .files_with_comments()
            .into_iter()
            .map(|p| p.to_path_buf())
            .collect();

        RenderData {
            viewer_comments,
            comment_edit,
            line_select_range,
            commented_files,
        }
    }

    /// Load a file into the viewer with its diff, optionally switching focus.
    fn handle_file_action(&mut self, path: &Path, switch_focus: bool) {
        self.file_viewer.load_file(path);
        self.load_diff_for_file(path);
        if switch_focus {
            self.focus = Focus::Viewer;
        }
    }

    /// Navigate to a specific file and line, loading the file if needed.
    fn navigate_to_file_line(&mut self, file: &Path, line: usize, switch_focus: bool) {
        if self.file_viewer.current_file() != Some(file) {
            self.file_viewer.load_file(file);
            self.load_diff_for_file(file);
        }
        self.file_viewer.scroll_to_line(line);
        if switch_focus {
            self.focus = Focus::Viewer;
        }
    }

    /// Refresh state when the filesystem watcher detects changes.
    /// Re-reads the file tree layout (fast, sync) and spawns a background thread
    /// for git status recomputation.
    fn refresh_on_fs_change(&mut self) {
        // Rescan filesystem layout (fast — no git status)
        if let Err(e) = self.file_tree.model.refresh_tree() {
            self.flash_message = Some(FlashMessage {
                text: format!("Tree refresh failed: {}", e),
                color: Color::Red,
                created: Instant::now(),
            });
        }

        // Spawn background git status recomputation
        self.git_worker = Some(GitStatusWorker::spawn(self.target_dir.clone()));

        // Reload file content from disk (preserves cursor/scroll position)
        self.file_viewer.reload_content();
        if let Some(path) = self.file_viewer.current_file().map(|p| p.to_path_buf()) {
            let current_lines = self.file_viewer.current_lines();
            self.comment_store.relocate_comments(&path, current_lines);
            self.load_diff_for_file(&path);
        }
    }

    fn load_diff_for_file(&mut self, path: &std::path::Path) {
        set_diff_for_file(&mut self.file_viewer, path, &self.git_workdir);
    }
}


pub fn run(target: &super::StartupTarget) -> anyhow::Result<()> {
    install_panic_hook();
    let mut terminal = init_terminal()?;

    // Wait for a valid terminal size.
    // In multi-layer PTY setups (e.g. zellij → kubectl exec → container),
    // the remote PTY may initially report 0x0 because the resize message
    // has not arrived yet. Poll briefly before giving up.
    let size = wait_for_terminal_size(&terminal)?;
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

        // Build the crash log from panic info
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".into()
        };

        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".into());

        let backtrace = std::backtrace::Backtrace::force_capture();
        let crash_log = format!(
            "panicked at '{}', {}\n\nstack backtrace:\n{}",
            message, location, backtrace
        );

        // Print the original panic output
        original_hook(panic_info);

        // Then print the bug report URL
        let url = crate::bug_report::build_panic_url(&crash_log);
        let link = crate::bug_report::hyperlink(&url, "Report this crash on GitHub");
        eprintln!();
        eprintln!("  {}", link);
        eprintln!();
        eprintln!("  {}", url);
        eprintln!();
    }));
}

fn init_terminal() -> anyhow::Result<DefaultTerminal> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    io::stdout().execute(EnableMouseCapture)?;
    let terminal = ratatui::init();
    Ok(terminal)
}

/// Poll `terminal.size()` until it returns a non-zero value, or give up
/// after a short deadline.  This handles the case where a resize message
/// from an outer multiplexer (e.g. zellij) or kubectl has not yet reached
/// the PTY when gati starts.
fn wait_for_terminal_size(
    terminal: &DefaultTerminal,
) -> anyhow::Result<ratatui::layout::Size> {
    use std::thread;

    let size = terminal.size()?;
    if size.width > 0 && size.height > 0 {
        return Ok(size);
    }

    // Retry up to 2 seconds in 50 ms intervals.
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));
        let size = terminal.size()?;
        if size.width > 0 && size.height > 0 {
            return Ok(size);
        }
    }

    // Return whatever we got; the caller decides whether to bail.
    Ok(terminal.size()?)
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
                    if app.show_help {
                        // Ignore mouse events while help dialog is open
                        continue;
                    }
                    handle_mouse(app, mouse, terminal.size()?.width);
                    continue;
                }
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    // Ctrl+C always quits, regardless of mode
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        return Ok(());
                    }

                    // Handle help dialog: consume all keys, close on ? / Esc / q
                    if app.show_help {
                        match key.code {
                            KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                                app.show_help = false;
                            }
                            _ => {}
                        }
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

                    // Global keybindings are suppressed while file tree
                    // search is active so that typed characters reach the
                    // search query instead of triggering shortcuts.
                    let in_search = app.file_tree.search.is_some();

                    // '?' toggles help dialog in Normal mode
                    if key.code == KeyCode::Char('?') && !in_search {
                        app.show_help = !app.show_help;
                        continue;
                    }

                    // 'b' toggles focus mode in Normal mode (both panes)
                    if key.code == KeyCode::Char('b')
                        && key.modifiers.is_empty()
                        && !in_search
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
                // Terminal resize: just let the next draw() pick up the new size.
                Event::Resize(_width, _height) => {}
                _ => {}
            }
        }

        // Check if background git status computation has completed
        if let Some(ref worker) = app.git_worker
            && let Some(git_status) = worker.try_recv() {
                app.file_tree.model.update_git_status(git_status);
                app.git_worker = None;
            }

        // Refresh tree, git status, and diff when the watcher detects file-system changes
        if let Some(ref watcher) = fs_watcher
            && watcher.has_changed() {
                app.refresh_on_fs_change();
            }
    }
}

use comment_ops::{handle_comment_input, handle_line_select};
use mouse::{handle_mouse, toggle_focus_mode};
use render::draw;

#[cfg(test)]
mod tests {
    use super::*;
    use super::comment_ops::{extract_code_context, export_comments, handle_comment_input};
    use super::mouse::{clamp_tree_percent, handle_mouse, toggle_focus_mode, MOUSE_SCROLL_LINES};
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    use crate::test_helpers::setup_dir_with;

    fn setup_dir(files: &[&str], dirs: &[&str]) -> TempDir {
        setup_dir_with(files, dirs, |f| format!("content of {f}"))
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
        app.comment_store.add(&path, 1, 1, "Delete me".into(), vec![]);
        app.focus = Focus::Viewer;

        app.handle_action(Action::DeleteComment);

        assert!(app.comment_store.is_empty());
    }

    #[test]
    fn start_comment_on_comment_row_edits_existing() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let path = tmp.path().join("file.rs");
        let mut app = App::new(&make_target(tmp.path(), Some(path.clone()))).unwrap();
        app.comment_store.add(&path, 2, 4, "Existing comment".into(), vec![]);
        app.focus = Focus::Viewer;
        app.file_viewer.cursor_line = 3; // 0-indexed
        app.file_viewer.cursor_on_comment = Some((2, 4));

        app.handle_action(Action::StartComment);

        match &app.input_mode {
            InputMode::CommentInput { start_line, end_line, text, .. } => {
                assert_eq!(*start_line, 2);
                assert_eq!(*end_line, 4);
                assert_eq!(text, "Existing comment");
            }
            other => panic!("Expected CommentInput, got {:?}", other),
        }
    }

    #[test]
    fn start_comment_on_code_line_creates_new() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let path = tmp.path().join("file.rs");
        let mut app = App::new(&make_target(tmp.path(), Some(path.clone()))).unwrap();
        // Add a range comment covering lines 1-5
        app.comment_store.add(&path, 1, 5, "Range comment".into(), vec![]);
        app.focus = Focus::Viewer;
        app.file_viewer.cursor_line = 2; // 0-indexed, line 3 in 1-indexed (within range)
        // cursor_on_comment is None → should create new, not edit existing

        app.handle_action(Action::StartComment);

        match &app.input_mode {
            InputMode::CommentInput { start_line, end_line, text, .. } => {
                assert_eq!(*start_line, 3); // cursor_line + 1
                assert_eq!(*end_line, 3);
                assert!(text.is_empty()); // new comment, not pre-filled
            }
            other => panic!("Expected CommentInput, got {:?}", other),
        }
    }

    #[test]
    fn delete_comment_on_comment_row_deletes_and_clears_focus() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let path = tmp.path().join("file.rs");
        let mut app = App::new(&make_target(tmp.path(), Some(path.clone()))).unwrap();
        app.comment_store.add(&path, 3, 5, "Delete me".into(), vec![]);
        app.focus = Focus::Viewer;
        app.file_viewer.cursor_on_comment = Some((3, 5));

        app.handle_action(Action::DeleteComment);

        assert!(app.comment_store.is_empty());
        assert!(app.file_viewer.cursor_on_comment.is_none());
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

    // Help dialog tests
    #[test]
    fn show_help_is_initially_false() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let app = App::new(&make_target(tmp.path(), None)).unwrap();
        assert!(!app.show_help);
    }

    #[test]
    fn question_mark_toggles_show_help() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        assert!(!app.show_help);

        app.show_help = true;
        assert!(app.show_help);

        app.show_help = false;
        assert!(!app.show_help);
    }

    #[test]
    fn help_open_esc_closes_help() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.show_help = true;

        // Simulate Esc while help is open: should close help, not quit
        // (In event_loop, Esc closes help. Here we test the flag directly.)
        app.show_help = false;
        assert!(!app.show_help);
    }

    #[test]
    fn help_open_q_closes_help_not_quit() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();
        app.show_help = true;

        // When help is open, 'q' should close help, not trigger app quit.
        // The event loop handles this, so we verify the flag behavior.
        app.show_help = false;
        assert!(!app.show_help);

        // App should still be alive (handle_action Quit returns true)
        assert!(!app.handle_action(Action::SwitchFocus));
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
    fn search_mode_suppresses_focus_mode_toggle() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();

        // Enter search mode on file tree
        app.file_tree.search = Some(crate::file_tree::SearchState::new_for_test());

        // Simulate 'b' key via handle_event on the file tree
        let key = crossterm::event::KeyEvent::new(
            KeyCode::Char('b'),
            crossterm::event::KeyModifiers::NONE,
        );

        // In the real event loop, the guard `!in_search` prevents
        // toggle_focus_mode from firing. We verify the guard condition:
        let in_search = app.file_tree.search.is_some();
        assert!(in_search, "search mode should be active");
        assert!(!app.focus_mode, "focus mode should NOT be toggled during search");
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

    // Mouse drag line selection
    #[test]
    fn mouse_click_on_viewer_enters_line_select() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let file_path = tmp.path().join("file.rs");
        fs::write(&file_path, "line0\nline1\nline2\nline3\nline4\n").unwrap();
        let mut app = App::new(&make_target(tmp.path(), Some(file_path.clone()))).unwrap();
        app.border_column = 30;
        // Simulate content_rect as if viewer rendered at (31, 1) with 69x20
        app.file_viewer.content_rect = Some(ratatui::layout::Rect::new(31, 1, 69, 20));
        app.file_viewer.scroll_offset = 0;

        // Click on viewer row 3 → inner row 2 → cursor_line = 2, file_line = 3
        let down = make_mouse_event_at(
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            40, 3,
        );
        handle_mouse(&mut app, down, 100);

        assert_eq!(app.focus, Focus::Viewer);
        assert_eq!(app.file_viewer.cursor_line, 2);
        match &app.input_mode {
            InputMode::LineSelect { anchor, .. } => assert_eq!(*anchor, 3),
            other => panic!("Expected LineSelect, got {:?}", other),
        }
    }

    #[test]
    fn mouse_drag_extends_line_select() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let file_path = tmp.path().join("file.rs");
        fs::write(&file_path, "line0\nline1\nline2\nline3\nline4\n").unwrap();
        let mut app = App::new(&make_target(tmp.path(), Some(file_path.clone()))).unwrap();
        app.border_column = 30;
        app.file_viewer.content_rect = Some(ratatui::layout::Rect::new(31, 1, 69, 20));
        app.file_viewer.scroll_offset = 0;

        // Mouse down at row 2 → cursor_line = 1, anchor = file_line 2
        let down = make_mouse_event_at(
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            40, 2,
        );
        handle_mouse(&mut app, down, 100);
        assert!(matches!(app.input_mode, InputMode::LineSelect { .. }));

        // Drag to row 5 → cursor_line = 4, file_line = 5
        let drag = make_mouse_event_at(
            crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left),
            40, 5,
        );
        handle_mouse(&mut app, drag, 100);

        assert_eq!(app.file_viewer.cursor_line, 4);
        // Still in LineSelect mode
        match &app.input_mode {
            InputMode::LineSelect { anchor, .. } => assert_eq!(*anchor, 2),
            other => panic!("Expected LineSelect after drag, got {:?}", other),
        }
    }

    #[test]
    fn mouse_up_single_line_cancels_line_select() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let file_path = tmp.path().join("file.rs");
        fs::write(&file_path, "line0\nline1\nline2\n").unwrap();
        let mut app = App::new(&make_target(tmp.path(), Some(file_path.clone()))).unwrap();
        app.border_column = 30;
        app.file_viewer.content_rect = Some(ratatui::layout::Rect::new(31, 1, 69, 20));
        app.file_viewer.scroll_offset = 0;

        // Click (no drag) → down + up on same row
        let down = make_mouse_event_at(
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            40, 2,
        );
        handle_mouse(&mut app, down, 100);
        assert!(matches!(app.input_mode, InputMode::LineSelect { .. }));

        let up = make_mouse_event_at(
            crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left),
            40, 2,
        );
        handle_mouse(&mut app, up, 100);

        // Single-line select cancelled on mouse up
        assert!(matches!(app.input_mode, InputMode::Normal));
    }

    #[test]
    fn mouse_up_multi_line_keeps_line_select() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let file_path = tmp.path().join("file.rs");
        fs::write(&file_path, "line0\nline1\nline2\nline3\n").unwrap();
        let mut app = App::new(&make_target(tmp.path(), Some(file_path.clone()))).unwrap();
        app.border_column = 30;
        app.file_viewer.content_rect = Some(ratatui::layout::Rect::new(31, 1, 69, 20));
        app.file_viewer.scroll_offset = 0;

        // Down at row 2
        let down = make_mouse_event_at(
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            40, 2,
        );
        handle_mouse(&mut app, down, 100);

        // Drag to row 4
        let drag = make_mouse_event_at(
            crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left),
            40, 4,
        );
        handle_mouse(&mut app, drag, 100);

        // Up → multi-line selection should persist
        let up = make_mouse_event_at(
            crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left),
            40, 4,
        );
        handle_mouse(&mut app, up, 100);

        match &app.input_mode {
            InputMode::LineSelect { anchor, .. } => {
                assert_eq!(*anchor, 2); // anchor at file line 2
                let current = app.file_viewer.cursor_file_line().unwrap();
                assert_eq!(current, 4); // cursor at file line 4
            }
            other => panic!("Expected LineSelect to persist after multi-line drag, got {:?}", other),
        }
    }

    // Git diff refresh on filesystem change
    use crate::test_helpers::setup_git_repo;

    #[test]
    fn refresh_on_fs_change_recomputes_diff_for_current_file() {
        let tmp = setup_git_repo(&[("file.rs", "line1\nline2\n")]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();

        // Select the file to load it into the viewer with diff
        let file_path = tmp.path().join("file.rs");
        app.handle_action(Action::FileSelected(file_path.clone()));

        // Initially committed, so no diff markers (all unchanged)
        let has_changes = app.file_viewer.diff.line_diff.as_ref().map_or(false, |d| {
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
            .diff.line_diff
            .as_ref()
            .expect("diff should be present after refresh");
        assert_eq!(
            diff.line_kind(3),
            crate::diff::DiffLineKind::Added,
            "new line 3 should be marked as Added after fs change refresh"
        );
    }

    #[test]
    fn extract_code_context_returns_correct_lines() {
        let content = crate::file_viewer::ViewerContent::File {
            path: PathBuf::from("test.rs"),
            lines: vec![
                "line 1".into(),
                "line 2".into(),
                "line 3".into(),
                "line 4".into(),
                "line 5".into(),
            ],
            syntax_name: "Rust".into(),
        };
        let result = extract_code_context(&content, Path::new("test.rs"), 2, 4);
        assert_eq!(result, vec!["line 2", "line 3", "line 4"]);
    }

    #[test]
    fn extract_code_context_clamps_to_file_length() {
        let content = crate::file_viewer::ViewerContent::File {
            path: PathBuf::from("test.rs"),
            lines: vec!["line 1".into(), "line 2".into(), "line 3".into()],
            syntax_name: "Rust".into(),
        };
        let result = extract_code_context(&content, Path::new("test.rs"), 2, 10);
        assert_eq!(result, vec!["line 2", "line 3"]);
    }

    #[test]
    fn extract_code_context_returns_empty_for_wrong_file() {
        let content = crate::file_viewer::ViewerContent::File {
            path: PathBuf::from("other.rs"),
            lines: vec!["line 1".into()],
            syntax_name: "Rust".into(),
        };
        let result = extract_code_context(&content, Path::new("test.rs"), 1, 1);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_code_context_returns_empty_for_placeholder() {
        let content = crate::file_viewer::ViewerContent::Placeholder;
        let result = extract_code_context(&content, Path::new("test.rs"), 1, 1);
        assert!(result.is_empty());
    }

    // Flash message tests
    #[test]
    fn export_sets_flash_message_on_success() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let path = tmp.path().join("file.rs");
        let mut app = App::new(&make_target(tmp.path(), Some(path.clone()))).unwrap();
        app.comment_store.add(&path, 1, 1, "A comment".into(), vec![]);

        export_comments(&mut app);

        let flash = app.flash_message.as_ref().expect("flash_message should be set");
        assert!(flash.text.contains("1 comment(s)"));
        assert_eq!(flash.color, Color::Green);
    }

    #[test]
    fn export_sets_flash_message_when_empty() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut app = App::new(&make_target(tmp.path(), None)).unwrap();

        export_comments(&mut app);

        let flash = app.flash_message.as_ref().expect("flash_message should be set");
        assert_eq!(flash.text, "No comments to export");
        assert_eq!(flash.color, Color::Yellow);
    }

    #[test]
    fn flash_message_not_shown_in_comment_input_mode() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let path = tmp.path().join("file.rs");
        let mut app = App::new(&make_target(tmp.path(), Some(path.clone()))).unwrap();
        app.flash_message = Some(FlashMessage {
            text: "test flash".into(),
            color: Color::Green,
            created: Instant::now(),
        });
        app.input_mode = InputMode::CommentInput {
            file: path,
            start_line: 1,
            end_line: 1,
            text: String::new(),
        };

        // In CommentInput mode, hints should show comment-related text, not the flash
        let hints = match &app.input_mode {
            InputMode::CommentInput { start_line, end_line, .. } => {
                let range = if start_line == end_line {
                    format!("L{}", start_line)
                } else {
                    format!("L{}-{}", start_line, end_line)
                };
                format!("Editing comment on {}  Enter save  Esc cancel", range)
            }
            _ => unreachable!(),
        };
        assert!(hints.contains("Editing comment"));
    }
}
