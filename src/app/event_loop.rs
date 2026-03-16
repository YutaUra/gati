use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::DefaultTerminal;

use crate::watcher::FsWatcher;

use super::comment_ops::{handle_char_select, handle_comment_input, handle_line_select};
use super::mouse::{handle_mouse, toggle_focus_mode};
use super::render::draw;
use crate::components::Component;

use super::{App, Focus, InputMode};

const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 10;

pub fn run(target: &crate::StartupTarget) -> anyhow::Result<()> {
    install_panic_hook();
    let mut terminal = init_terminal()?;

    // Wait for a valid terminal size.
    // In multi-layer PTY setups (e.g. zellij -> kubectl exec -> container),
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

/// Enter CharSelect mode if not already in it, using current cursor position as anchor.
fn ensure_char_select(app: &mut App) {
    if matches!(app.input_mode, InputMode::CharSelect { .. }) {
        return;
    }
    let Some(file) = app.file_viewer.current_file() else {
        return;
    };
    let Some(line) = app.file_viewer.cursor_file_line() else {
        return;
    };
    app.input_mode = InputMode::CharSelect {
        file: file.to_path_buf(),
        anchor_line: line,
        anchor_col: app.file_viewer.cursor_col.unwrap_or(0),
    };
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> anyhow::Result<()> {
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

                    // Ctrl+C: copy in selection modes, quit otherwise
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        match &app.input_mode {
                            InputMode::CharSelect { .. } => {
                                super::comment_ops::copy_char_selection(app);
                                continue;
                            }
                            InputMode::LineSelect { .. } => {
                                super::comment_ops::copy_line_selection(app);
                                continue;
                            }
                            _ => return Ok(()),
                        }
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
                    if let InputMode::CommentInput { .. } = &app.input_mode {
                        handle_comment_input(app, key);
                        continue;
                    }

                    // Shift+arrow: start/extend character selection
                    if key.modifiers.contains(KeyModifiers::SHIFT)
                        && matches!(key.code, KeyCode::Left | KeyCode::Right)
                        && app.focus == Focus::Viewer
                    {
                        ensure_char_select(app);
                        if key.modifiers.contains(KeyModifiers::ALT) {
                            // Shift+Alt: word-level selection
                            match key.code {
                                KeyCode::Left => app.file_viewer.cursor_word_left(),
                                KeyCode::Right => app.file_viewer.cursor_word_right(),
                                _ => {}
                            }
                        } else {
                            // Shift only: char-level selection
                            match key.code {
                                KeyCode::Left => app.file_viewer.cursor_left(),
                                KeyCode::Right => app.file_viewer.cursor_right(),
                                _ => {}
                            }
                        }
                        continue;
                    }

                    match &app.input_mode {
                        InputMode::CharSelect { .. } => {
                            if handle_char_select(app, key) {
                                continue;
                            }
                            // If not handled, fall through to normal
                        }
                        InputMode::LineSelect { .. } => {
                            if handle_line_select(app, key) {
                                continue;
                            }
                            // If not handled, fall through to normal
                        }
                        InputMode::Normal | InputMode::CommentInput { .. } => {}
                    }

                    // Handle viewer search input when active
                    if app.file_viewer.search.is_some() && app.focus == Focus::Viewer {
                        match key.code {
                            KeyCode::Esc => {
                                app.file_viewer.search = None;
                            }
                            KeyCode::Enter | KeyCode::Down => {
                                if let Some(ref mut s) = app.file_viewer.search {
                                    s.next_match();
                                }
                                jump_to_search_match(app);
                            }
                            KeyCode::Up => {
                                if let Some(ref mut s) = app.file_viewer.search {
                                    s.prev_match();
                                }
                                jump_to_search_match(app);
                            }
                            KeyCode::Char(c) => {
                                if let Some(ref mut s) = app.file_viewer.search {
                                    s.query.push(c);
                                }
                                app.file_viewer.refresh_search();
                                // Jump to nearest match from current cursor
                                if let Some(ref mut s) = app.file_viewer.search {
                                    s.jump_to_nearest(app.file_viewer.cursor_line);
                                }
                                jump_to_search_match(app);
                            }
                            KeyCode::Backspace => {
                                if let Some(ref mut s) = app.file_viewer.search {
                                    s.query.pop();
                                }
                                app.file_viewer.refresh_search();
                                if let Some(ref mut s) = app.file_viewer.search {
                                    s.jump_to_nearest(app.file_viewer.cursor_line);
                                }
                                jump_to_search_match(app);
                            }
                            _ => {}
                        }
                        continue;
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

/// Jump the viewer cursor and scroll to the current search match position.
fn jump_to_search_match(app: &mut App) {
    let Some(ref search) = app.file_viewer.search else {
        return;
    };
    let Some((line, col, _)) = search.current_match() else {
        return;
    };
    app.file_viewer.cursor_line = line;
    app.file_viewer.cursor_col = Some(col);
    app.file_viewer.cursor_on_comment = None;
    app.file_viewer.ensure_cursor_visible();
    let content_w = app.file_viewer.visible_content_width;
    if content_w > 0 && col < app.file_viewer.h_scroll {
        app.file_viewer.h_scroll = col;
    } else if content_w > 0 && col >= app.file_viewer.h_scroll + content_w {
        app.file_viewer.h_scroll = col.saturating_sub(content_w) + 1;
    }
}
