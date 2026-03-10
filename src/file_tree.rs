use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use crate::components::{Action, Component};
use crate::git_status::{FileStatus, GitStatus};
use crate::tree::{self, FileTreeModel, TreeEntry};

/// Search mode state.
pub struct SearchState {
    pub query: String,
    saved_entries: Vec<TreeEntry>,
    saved_selected: usize,
    saved_scroll_offset: usize,
    /// Root directory for recursive search.
    root: std::path::PathBuf,
}

pub struct FileTree {
    pub model: FileTreeModel,
    /// Scroll offset for the tree view.
    pub scroll_offset: usize,
    /// Active search state (None when not searching).
    pub search: Option<SearchState>,
}

impl FileTree {
    pub fn new(root: &Path, git_status: Option<GitStatus>) -> anyhow::Result<Self> {
        let model = FileTreeModel::from_dir(root, git_status)?;
        Ok(Self {
            model,
            scroll_offset: 0,
            search: None,
        })
    }

    pub fn render_to_buffer(&self, area: Rect, buf: &mut Buffer, focused: bool) {
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let title = if let Some(ref search) = self.search {
            format!(" Files [/{}] ", search.query)
        } else if self.model.filter_changed {
            " Changed Files ".to_string()
        } else {
            " Files ".to_string()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title.as_str());

        let inner = block.inner(area);
        block.render(area, buf);

        let visible_height = inner.height as usize;

        for (i, entry) in self
            .model
            .entries
            .iter()
            .skip(self.scroll_offset)
            .take(visible_height)
            .enumerate()
        {
            let global_idx = self.scroll_offset + i;
            let is_selected = global_idx == self.model.selected;

            let indent = "  ".repeat(entry.depth);
            let icon = if entry.is_directory {
                if entry.is_expanded {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };

            let style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::White)
            } else {
                Style::default()
            };

            let name_text = format!("{indent}{icon}{}", entry.name());
            let mut spans = vec![Span::styled(name_text, style)];

            // Git status marker
            let marker_info = if entry.is_directory {
                if self.model.dir_has_changes(&entry.path) {
                    Some((" [●]", Color::Yellow))
                } else {
                    None
                }
            } else {
                entry.git_status.map(|fs| match fs {
                    FileStatus::Modified => (" [M]", Color::Yellow),
                    FileStatus::Added => (" [A]", Color::Green),
                    FileStatus::Deleted => (" [D]", Color::Red),
                    FileStatus::Renamed => (" [R]", Color::Blue),
                    FileStatus::Untracked => (" [?]", Color::Green),
                })
            };

            if let Some((marker, color)) = marker_info {
                let marker_style = if is_selected {
                    Style::default().fg(color).bg(Color::White)
                } else {
                    Style::default().fg(color)
                };
                spans.push(Span::styled(marker, marker_style));
            }

            let line = Line::from(spans);

            let y = inner.y + i as u16;
            if y < inner.y + inner.height {
                buf.set_line(inner.x, y, &line, inner.width);
            }
        }

        // Show "No matches" when search is active and entries are empty
        if self.search.is_some() && self.model.entries.is_empty() {
            let msg = Line::from(Span::styled(
                "No matches",
                Style::default().fg(Color::DarkGray),
            ));
            buf.set_line(inner.x, inner.y, &msg, inner.width);
        }
    }

    /// Ensure the selected item is visible by adjusting scroll_offset.
    pub fn ensure_visible(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        if self.model.selected < self.scroll_offset {
            self.scroll_offset = self.model.selected;
        } else if self.model.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.model.selected - visible_height + 1;
        }
    }

    /// Move selection, returning the action (FileSelected if a file is now under cursor).
    fn move_selection(&mut self, delta: isize) -> Action {
        let len = self.model.entries.len();
        if len == 0 {
            return Action::None;
        }

        let new_idx = if delta > 0 {
            (self.model.selected + delta as usize).min(len - 1)
        } else {
            self.model.selected.saturating_sub((-delta) as usize)
        };

        self.model.selected = new_idx;

        if let Some(entry) = self.model.selected_entry() {
            if !entry.is_directory {
                return Action::FileSelected(entry.path.clone());
            }
        }
        Action::None
    }

    /// Find the index of the parent directory entry for the currently selected entry.
    /// The parent is the nearest preceding entry with depth one less than the current entry.
    fn find_parent_index(&self) -> Option<usize> {
        let selected = self.model.selected;
        let current_depth = self.model.entries.get(selected)?.depth;
        if current_depth == 0 {
            return None;
        }
        let target_depth = current_depth - 1;
        (0..selected)
            .rev()
            .find(|&i| self.model.entries[i].depth == target_depth && self.model.entries[i].is_directory)
    }
}

impl FileTree {
    /// Activate search mode.
    fn enter_search(&mut self) {
        let root = self
            .model
            .entries
            .first()
            .map(|e| {
                // Derive root from the first entry's parent
                e.path.parent().unwrap_or(&e.path).to_path_buf()
            })
            .unwrap_or_default();

        self.search = Some(SearchState {
            query: String::new(),
            saved_entries: self.model.entries.clone(),
            saved_selected: self.model.selected,
            saved_scroll_offset: self.scroll_offset,
            root,
        });
    }

    /// Exit search mode, keeping or restoring state.
    fn exit_search(&mut self, confirm: bool) {
        if let Some(search) = self.search.take() {
            if confirm {
                // Keep current entries and selection — the selected file stays
            } else {
                // Restore saved state
                self.model.entries = search.saved_entries;
                self.model.selected = search.saved_selected;
                self.scroll_offset = search.saved_scroll_offset;
            }
        }
    }

    /// Update search results based on current query.
    fn update_search_results(&mut self) -> anyhow::Result<()> {
        let Some(ref search) = self.search else {
            return Ok(());
        };

        if search.query.is_empty() {
            // Empty query: restore original entries
            self.model.entries = search.saved_entries.clone();
            self.model.selected = 0;
            self.scroll_offset = 0;
            return Ok(());
        }

        let mut entries = tree::search_files(&search.root, &search.query)?;

        // Annotate with git status if available
        if let Some(ref gs) = self.model.git_status_ref() {
            for entry in entries.iter_mut() {
                if !entry.is_directory {
                    entry.git_status = gs.file_status(&entry.path);
                }
            }
        }

        self.model.entries = entries;
        self.model.selected = 0;
        self.scroll_offset = 0;
        Ok(())
    }

    fn handle_search_event(&mut self, key: KeyEvent) -> anyhow::Result<Action> {
        match key.code {
            KeyCode::Enter => {
                // Confirm: if a file is selected, open it
                let action = if let Some(entry) = self.model.selected_entry() {
                    if !entry.is_directory {
                        Action::FileOpened(entry.path.clone())
                    } else {
                        Action::None
                    }
                } else {
                    Action::None
                };
                self.exit_search(true);
                Ok(action)
            }
            KeyCode::Esc => {
                self.exit_search(false);
                Ok(Action::None)
            }
            KeyCode::Char(c) => {
                if let Some(ref mut search) = self.search {
                    search.query.push(c);
                }
                self.update_search_results()?;
                Ok(Action::None)
            }
            KeyCode::Backspace => {
                if let Some(ref mut search) = self.search {
                    search.query.pop();
                }
                self.update_search_results()?;
                Ok(Action::None)
            }
            KeyCode::Down => {
                let action = self.move_selection(1);
                self.ensure_visible(20);
                Ok(action)
            }
            KeyCode::Up => {
                let action = self.move_selection(-1);
                self.ensure_visible(20);
                Ok(action)
            }
            _ => Ok(Action::None),
        }
    }
}

impl Component for FileTree {
    fn handle_event(&mut self, key: KeyEvent) -> anyhow::Result<Action> {
        // Search mode handles its own events
        if self.search.is_some() {
            return self.handle_search_event(key);
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                let action = self.move_selection(1);
                self.ensure_visible(20);
                Ok(action)
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                let action = self.move_selection(-1);
                self.ensure_visible(20);
                Ok(action)
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Right, _) => {
                if let Some(entry) = self.model.selected_entry() {
                    if entry.is_directory && !entry.is_expanded {
                        self.model.toggle_expand()?;
                    }
                }
                Ok(Action::None)
            }
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, _) => {
                if let Some(entry) = self.model.selected_entry() {
                    if entry.is_directory && entry.is_expanded {
                        self.model.toggle_expand()?;
                    } else if let Some(parent_idx) = self.find_parent_index() {
                        self.model.selected = parent_idx;
                        self.model.toggle_expand()?;
                        self.ensure_visible(20);
                    }
                }
                Ok(Action::None)
            }
            (KeyCode::Enter, _) => {
                if let Some(entry) = self.model.selected_entry() {
                    if !entry.is_directory {
                        let path = entry.path.clone();
                        return Ok(Action::FileOpened(path));
                    }
                }
                Ok(Action::None)
            }
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                self.enter_search();
                Ok(Action::None)
            }
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                self.model.toggle_filter()?;
                self.scroll_offset = 0;
                self.ensure_visible(20);
                Ok(Action::None)
            }
            (KeyCode::Tab, _) => Ok(Action::SwitchFocus),
            (KeyCode::Char('q'), KeyModifiers::NONE) => Ok(Action::Quit),
            _ => Ok(Action::None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
            fs::write(&path, "content").unwrap();
        }
        tmp
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    // Task 3.3: Navigation tests
    #[test]
    fn move_down_with_j_selects_next_entry() {
        let tmp = setup_dir(&["a.rs", "b.rs", "c.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        assert_eq!(tree.model.selected, 0);

        let action = tree.handle_event(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(tree.model.selected, 1);
        assert!(matches!(action, Action::FileSelected(_)));
    }

    #[test]
    fn move_up_with_k_selects_previous_entry() {
        let tmp = setup_dir(&["a.rs", "b.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.model.selected = 1;

        tree.handle_event(key(KeyCode::Char('k'))).unwrap();
        assert_eq!(tree.model.selected, 0);
    }

    #[test]
    fn move_down_with_arrow_key() {
        let tmp = setup_dir(&["a.rs", "b.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.handle_event(key(KeyCode::Down)).unwrap();
        assert_eq!(tree.model.selected, 1);
    }

    #[test]
    fn move_up_with_arrow_key() {
        let tmp = setup_dir(&["a.rs", "b.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.model.selected = 1;
        tree.handle_event(key(KeyCode::Up)).unwrap();
        assert_eq!(tree.model.selected, 0);
    }

    #[test]
    fn selection_clamped_at_bottom() {
        let tmp = setup_dir(&["a.rs", "b.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.model.selected = 1; // last entry
        tree.handle_event(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(tree.model.selected, 1); // stays at last
    }

    #[test]
    fn selection_clamped_at_top() {
        let tmp = setup_dir(&["a.rs", "b.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.handle_event(key(KeyCode::Char('k'))).unwrap();
        assert_eq!(tree.model.selected, 0); // stays at first
    }

    // Task 3.4: Scroll tests
    #[test]
    fn ensure_visible_scrolls_down() {
        let tmp = setup_dir(&["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.model.selected = 4; // beyond visible area of height 3
        tree.ensure_visible(3);
        assert_eq!(tree.scroll_offset, 2); // 4 - 3 + 1
    }

    #[test]
    fn ensure_visible_scrolls_up() {
        let tmp = setup_dir(&["a.rs", "b.rs", "c.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.scroll_offset = 2;
        tree.model.selected = 0;
        tree.ensure_visible(3);
        assert_eq!(tree.scroll_offset, 0);
    }

    // Task 3.5: h/l for directory expand/collapse
    #[test]
    fn l_expands_collapsed_directory() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        assert!(tree.model.entries[0].is_directory);
        assert!(!tree.model.entries[0].is_expanded);

        let action = tree.handle_event(key(KeyCode::Char('l'))).unwrap();
        assert_eq!(action, Action::None);
        assert!(tree.model.entries[0].is_expanded);
    }

    #[test]
    fn right_arrow_expands_collapsed_directory() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        assert!(!tree.model.entries[0].is_expanded);

        tree.handle_event(key(KeyCode::Right)).unwrap();
        assert!(tree.model.entries[0].is_expanded);
    }

    #[test]
    fn h_collapses_expanded_directory() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        // Expand first
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();
        assert!(tree.model.entries[0].is_expanded);

        let action = tree.handle_event(key(KeyCode::Char('h'))).unwrap();
        assert_eq!(action, Action::None);
        assert!(!tree.model.entries[0].is_expanded);
    }

    #[test]
    fn left_arrow_collapses_expanded_directory() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();
        assert!(tree.model.entries[0].is_expanded);

        tree.handle_event(key(KeyCode::Left)).unwrap();
        assert!(!tree.model.entries[0].is_expanded);
    }

    #[test]
    fn h_on_root_level_collapsed_directory_is_noop() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        // "sub" is at depth 0, collapsed — no parent to collapse
        assert!(!tree.model.entries[0].is_expanded);

        let action = tree.handle_event(key(KeyCode::Char('h'))).unwrap();
        assert_eq!(action, Action::None);
        assert!(!tree.model.entries[0].is_expanded);
        assert_eq!(tree.model.selected, 0);
    }

    #[test]
    fn h_on_root_level_file_is_noop() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        // "file.rs" is at depth 0 — no parent to collapse
        let action = tree.handle_event(key(KeyCode::Char('h'))).unwrap();
        assert_eq!(action, Action::None);
        assert_eq!(tree.model.selected, 0);
    }

    #[test]
    fn h_on_child_file_collapses_parent_and_moves_cursor() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        // Expand "sub" directory
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();
        assert!(tree.model.entries[0].is_expanded);
        // entries: [sub(expanded), child.rs]
        // Move cursor to child.rs
        tree.model.selected = 1;

        let action = tree.handle_event(key(KeyCode::Char('h'))).unwrap();
        assert_eq!(action, Action::None);
        // Parent "sub" should be collapsed
        assert!(!tree.model.entries[0].is_expanded);
        // Cursor should move to parent "sub"
        assert_eq!(tree.model.selected, 0);
    }

    #[test]
    fn h_on_nested_collapsed_directory_collapses_parent() {
        let tmp = setup_dir(&["parent/child/file.rs"], &["parent", "parent/child"]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        // Expand "parent"
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();
        assert!(tree.model.entries[0].is_expanded);
        // entries: [parent(expanded), child(collapsed)]
        // Cursor on "child" (collapsed directory inside parent)
        tree.model.selected = 1;
        assert!(tree.model.entries[1].is_directory);
        assert!(!tree.model.entries[1].is_expanded);

        tree.handle_event(key(KeyCode::Char('h'))).unwrap();
        // Parent "parent" should be collapsed
        assert!(!tree.model.entries[0].is_expanded);
        // Cursor should move to parent
        assert_eq!(tree.model.selected, 0);
    }

    #[test]
    fn left_arrow_on_child_file_collapses_parent() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.handle_event(key(KeyCode::Right)).unwrap();
        tree.model.selected = 1;

        tree.handle_event(key(KeyCode::Left)).unwrap();
        assert!(!tree.model.entries[0].is_expanded);
        assert_eq!(tree.model.selected, 0);
    }

    #[test]
    fn l_on_file_is_noop() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        assert!(!tree.model.entries[0].is_directory);

        let action = tree.handle_event(key(KeyCode::Char('l'))).unwrap();
        assert_eq!(action, Action::None);
    }

    #[test]
    fn enter_on_directory_is_noop() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        assert!(tree.model.entries[0].is_directory);

        let action = tree.handle_event(key(KeyCode::Enter)).unwrap();
        assert_eq!(action, Action::None);
        // Directory should NOT be expanded by Enter
        assert!(!tree.model.entries[0].is_expanded);
    }

    #[test]
    fn enter_on_file_returns_file_opened() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        let action = tree.handle_event(key(KeyCode::Enter)).unwrap();
        assert!(matches!(action, Action::FileOpened(_)));
    }

    // Task 3.6: Preview on cursor movement
    #[test]
    fn moving_to_file_returns_file_selected() {
        let tmp = setup_dir(&["a.rs", "b.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        let action = tree.handle_event(key(KeyCode::Char('j'))).unwrap();
        assert!(matches!(action, Action::FileSelected(_)));
    }

    #[test]
    fn moving_to_directory_returns_none() {
        let tmp = setup_dir(&["a.rs"], &["sub"]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        // entries: [sub(dir), a.rs(file)] — sorted dirs first
        // selected = 0 (sub), move down to a.rs
        // Actually first is sub (dir), so moving down goes to a.rs (file)
        // Let's set selected to 1 (a.rs) and move up to sub (dir)
        tree.model.selected = 1;
        let action = tree.handle_event(key(KeyCode::Char('k'))).unwrap();
        assert_eq!(action, Action::None); // moved to directory
    }

    // Tab and q
    #[test]
    fn tab_returns_switch_focus() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        let action = tree.handle_event(key(KeyCode::Tab)).unwrap();
        assert_eq!(action, Action::SwitchFocus);
    }

    #[test]
    fn q_returns_quit() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        let action = tree.handle_event(key(KeyCode::Char('q'))).unwrap();
        assert_eq!(action, Action::Quit);
    }
}
