use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
use crate::unicode;

use crate::comments::CommentListEntry;
use crate::tree::ContentMatch;

/// State for the comment list view.
struct CommentListState {
    entries: Vec<CommentListEntry>,
    selected: usize,
    scroll_offset: usize,
}

/// Search mode state.
pub struct SearchState {
    pub query: String,
    saved_entries: Vec<TreeEntry>,
    saved_selected: usize,
    saved_scroll_offset: usize,
    /// Root directory for recursive search.
    root: std::path::PathBuf,
}

impl SearchState {
    /// Create a minimal SearchState for testing.
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self {
            query: String::new(),
            saved_entries: Vec::new(),
            saved_selected: 0,
            saved_scroll_offset: 0,
            root: std::path::PathBuf::new(),
        }
    }
}

/// An entry in the content search results list.
#[derive(Debug, Clone)]
pub enum ContentSearchEntry {
    Header {
        display_name: String,
    },
    Match {
        file: PathBuf,
        line_number: usize,
        text_preview: String,
    },
}

/// State for the cross-file content search view.
pub struct ContentSearchState {
    pub query: String,
    pub entries: Vec<ContentSearchEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub match_count: usize,
    pub searching: bool,
    saved_entries: Vec<TreeEntry>,
    saved_selected: usize,
    saved_scroll_offset: usize,
    root: PathBuf,
}

/// Build spans for a content search result line, highlighting all occurrences
/// of `query` (case-insensitive) in `text` with `highlight_style`.
/// The `prefix` (e.g. "  :42 ") is rendered with `base_style`.
fn build_highlighted_spans<'a>(
    prefix: &str,
    text: &str,
    query: &str,
    base_style: Style,
    highlight_style: Style,
) -> Vec<Span<'a>> {
    let mut spans = vec![Span::styled(prefix.to_string(), base_style)];

    if query.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
        return spans;
    }

    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();
    let mut pos = 0;

    while let Some(match_start) = text_lower[pos..].find(&query_lower) {
        let abs_start = pos + match_start;
        let abs_end = abs_start + query.len();

        // Text before the match
        if abs_start > pos {
            spans.push(Span::styled(text[pos..abs_start].to_string(), base_style));
        }
        // The matched portion (use original casing from text)
        spans.push(Span::styled(
            text[abs_start..abs_end].to_string(),
            highlight_style,
        ));
        pos = abs_end;
    }

    // Remaining text after last match
    if pos < text.len() {
        spans.push(Span::styled(text[pos..].to_string(), base_style));
    }

    spans
}

/// Time window for double-tap detection.
const DOUBLE_TAP_THRESHOLD: Duration = Duration::from_millis(400);

pub struct FileTree {
    pub model: FileTreeModel,
    /// Scroll offset for the tree view.
    pub scroll_offset: usize,
    /// Active search state (None when not searching).
    pub search: Option<SearchState>,
    /// Cached visible height from last render, used for scroll calculations.
    pub visible_height: usize,
    /// Timestamp of last expand action (for double-tap recursive expand).
    pub last_expand_time: Option<Instant>,
    /// Timestamp of last collapse action (for double-tap fold-to-root).
    pub last_collapse_time: Option<Instant>,
    /// Comment list view state (None when in normal file tree mode).
    comment_list: Option<CommentListState>,
    /// Content search state (None when not searching file contents).
    pub content_search: Option<ContentSearchState>,
}

impl FileTree {
    pub fn new(root: &Path, git_status: Option<GitStatus>) -> anyhow::Result<Self> {
        let model = FileTreeModel::from_dir(root, git_status)?;
        Ok(Self {
            model,
            scroll_offset: 0,
            search: None,
            visible_height: 0,
            last_expand_time: None,
            last_collapse_time: None,
            comment_list: None,
            content_search: None,
        })
    }

    /// Enter comment list mode with the given entries.
    pub fn enter_comment_list(&mut self, entries: Vec<CommentListEntry>) {
        // Find the first non-header entry to select
        let first_comment = entries.iter().position(|e| !e.is_header()).unwrap_or(0);
        self.comment_list = Some(CommentListState {
            entries,
            selected: first_comment,
            scroll_offset: 0,
        });
    }

    /// Exit comment list mode, returning to the file tree.
    pub fn exit_comment_list(&mut self) {
        self.comment_list = None;
    }

    /// Whether the file tree is currently in comment list mode.
    pub fn is_comment_list_mode(&self) -> bool {
        self.comment_list.is_some()
    }

    /// Get the currently selected comment entry (non-header), if any.
    pub fn selected_comment(&self) -> Option<&CommentListEntry> {
        self.comment_list.as_ref().and_then(|cl| {
            cl.entries.get(cl.selected).filter(|e| !e.is_header())
        })
    }

    pub fn render_to_buffer(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        focused: bool,
        commented_files: &HashSet<PathBuf>,
    ) {
        let border_style = crate::components::border_style(focused);

        // Content search mode renders differently
        if self.content_search.is_some() {
            self.render_content_search(area, buf, border_style);
            return;
        }

        // Comment list mode renders differently
        if self.comment_list.is_some() {
            self.render_comment_list(area, buf, border_style);
            return;
        }

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
        self.visible_height = visible_height;

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
            let line = self.render_tree_entry(entry, is_selected, commented_files);

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

    /// Render the comment list overlay.
    fn render_comment_list(&self, area: Rect, buf: &mut Buffer, border_style: Style) {
        let cl = self.comment_list.as_ref().unwrap();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Comments ");
        let inner = block.inner(area);
        block.render(area, buf);

        let visible_height = inner.height as usize;

        for (i, entry) in cl
            .entries
            .iter()
            .skip(cl.scroll_offset)
            .take(visible_height)
            .enumerate()
        {
            let global_idx = cl.scroll_offset + i;
            let is_selected = global_idx == cl.selected;

            let style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::White)
            } else if entry.is_header() {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            let text = match entry {
                CommentListEntry::Header { display_name, .. } => {
                    let prefix = "\u{25bc} ";
                    let max_name = (inner.width as usize).saturating_sub(prefix.len());
                    if display_name.chars().count() > max_name {
                        let char_count = display_name.chars().count();
                        let skip_chars = char_count.saturating_sub(max_name.saturating_sub(1));
                        let byte_offset = unicode::char_skip_byte_offset(display_name, skip_chars);
                        format!("{prefix}\u{2026}{}", &display_name[byte_offset..])
                    } else {
                        format!("{prefix}{display_name}")
                    }
                }
                CommentListEntry::Comment { start_line, end_line, text, .. } => {
                    let line_str = if start_line == end_line {
                        format!(":{}", start_line)
                    } else {
                        format!(":{}-{}", start_line, end_line)
                    };
                    // Truncate comment text to fit
                    let max_text = (inner.width as usize).saturating_sub(line_str.len() + 4);
                    let truncated = if text.len() > max_text {
                        let end = unicode::floor_char_boundary(text, max_text.saturating_sub(3));
                        format!("{}...", &text[..end])
                    } else {
                        text.clone()
                    };
                    format!("  {line_str} {truncated}")
                }
            };

            let line = Line::from(Span::styled(text, style));
            let y = inner.y + i as u16;
            if y < inner.y + inner.height {
                buf.set_line(inner.x, y, &line, inner.width);
            }
        }

        if cl.entries.is_empty() {
            let msg = Line::from(Span::styled(
                "No comments",
                Style::default().fg(Color::DarkGray),
            ));
            buf.set_line(inner.x, inner.y, &msg, inner.width);
        }
    }

    /// Build a Line with icon, name, git status marker, and comment indicator for one entry.
    fn render_tree_entry(
        &self,
        entry: &TreeEntry,
        is_selected: bool,
        commented_files: &HashSet<PathBuf>,
    ) -> Line<'static> {
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
        } else if entry.is_gitignored {
            Style::default().fg(Color::DarkGray)
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
            } else if entry.is_gitignored {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(color)
            };
            spans.push(Span::styled(marker, marker_style));
        }

        // Comment indicator
        let has_comments = if entry.is_directory {
            commented_files.iter().any(|f| f.starts_with(&entry.path))
        } else {
            commented_files.contains(&entry.path)
        };
        if has_comments {
            let comment_style = if is_selected {
                Style::default().fg(Color::Cyan).bg(Color::White)
            } else if entry.is_gitignored {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Cyan)
            };
            spans.push(Span::styled(" [C]", comment_style));
        }

        Line::from(spans)
    }

    /// Scroll the active view (content search, comment list, or tree) down.
    pub fn scroll_down(&mut self, lines: usize) {
        if let Some(ref mut cs) = self.content_search {
            let total = cs.entries.len();
            let max = total.saturating_sub(self.visible_height);
            cs.scroll_offset = (cs.scroll_offset + lines).min(max);
        } else if let Some(ref mut cl) = self.comment_list {
            let total = cl.entries.len();
            let max = total.saturating_sub(self.visible_height);
            cl.scroll_offset = (cl.scroll_offset + lines).min(max);
        } else {
            let total = self.model.entries.len();
            let max = total.saturating_sub(self.visible_height);
            self.scroll_offset = (self.scroll_offset + lines).min(max);
        }
    }

    /// Scroll the active view (content search, comment list, or tree) up.
    pub fn scroll_up(&mut self, lines: usize) {
        if let Some(ref mut cs) = self.content_search {
            cs.scroll_offset = cs.scroll_offset.saturating_sub(lines);
        } else if let Some(ref mut cl) = self.comment_list {
            cl.scroll_offset = cl.scroll_offset.saturating_sub(lines);
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        }
    }

    /// Handle a mouse click on a row within the tree pane inner area.
    /// Returns an Action if the click triggers navigation.
    pub fn click_entry(&mut self, inner_row: usize) -> Action {
        if let Some(ref mut cs) = self.content_search {
            let entry_idx = cs.scroll_offset + inner_row;
            if entry_idx < cs.entries.len() {
                // Skip headers — select the clicked entry only if it's a Match
                if let ContentSearchEntry::Match { file, line_number, .. } = &cs.entries[entry_idx] {
                    cs.selected = entry_idx;
                    return Action::CommentJumped {
                        file: file.clone(),
                        line: *line_number,
                    };
                }
            }
            Action::None
        } else if let Some(ref mut cl) = self.comment_list {
            let entry_idx = cl.scroll_offset + inner_row;
            if entry_idx < cl.entries.len()
                && let CommentListEntry::Comment { file, start_line, .. } = &cl.entries[entry_idx]
            {
                cl.selected = entry_idx;
                return Action::CommentJumped {
                    file: file.clone(),
                    line: *start_line,
                };
            }
            Action::None
        } else {
            // Normal tree mode — handled by mouse.rs directly
            Action::None
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

    /// Recursively expand all subdirectories under the currently selected directory.
    fn expand_all(&mut self) -> anyhow::Result<()> {
        let start = self.model.selected;
        let start_depth = self.model.entries[start].depth;
        let mut i = start;
        while i < self.model.entries.len() {
            // Stop before acting when we leave the subtree
            if i > start && self.model.entries[i].depth <= start_depth {
                break;
            }
            if self.model.entries[i].is_directory && !self.model.entries[i].is_expanded {
                let saved = self.model.selected;
                self.model.selected = i;
                self.model.toggle_expand()?;
                self.model.selected = saved;
            }
            i += 1;
        }
        Ok(())
    }

    /// Find the root-level (depth 0) ancestor of the currently selected entry.
    fn find_root_ancestor_index(&self) -> Option<usize> {
        let selected = self.model.selected;
        if self.model.entries.get(selected)?.depth == 0 {
            return Some(selected);
        }
        (0..selected)
            .rev()
            .find(|&i| self.model.entries[i].depth == 0)
    }

    /// Fold (collapse) up to the root-level ancestor and select it.
    fn fold_to_root(&mut self) -> anyhow::Result<()> {
        if let Some(root_idx) = self.find_root_ancestor_index() {
            // Collapse the root ancestor (which collapses everything under it)
            if self.model.entries[root_idx].is_directory && self.model.entries[root_idx].is_expanded {
                self.model.selected = root_idx;
                self.model.toggle_expand()?;
            } else {
                self.model.selected = root_idx;
            }
            self.ensure_visible(self.visible_height);
        }
        Ok(())
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

        if let Some(entry) = self.model.selected_entry()
            && !entry.is_directory {
                return Action::FileSelected(entry.path.clone());
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
        if let Some(gs) = self.model.git_status_ref() {
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
                self.ensure_visible(self.visible_height);
                Ok(action)
            }
            KeyCode::Up => {
                let action = self.move_selection(-1);
                self.ensure_visible(self.visible_height);
                Ok(action)
            }
            _ => Ok(Action::None),
        }
    }
}

impl FileTree {
    /// Handle events when in comment list mode.
    fn handle_comment_list_event(&mut self, key: KeyEvent) -> anyhow::Result<Action> {
        let cl = self.comment_list.as_mut().unwrap();
        match (key.code, key.modifiers) {
            (KeyCode::Char('j'), KeyModifiers::NONE)
            | (KeyCode::Down, _)
            | (KeyCode::Char('k'), KeyModifiers::NONE)
            | (KeyCode::Up, _) => {
                let going_down = matches!(key.code, KeyCode::Char('j') | KeyCode::Down);
                let len = cl.entries.len();
                if len == 0 {
                    return Ok(Action::None);
                }
                // Move to next/prev non-header entry
                let mut idx = cl.selected;
                loop {
                    if going_down {
                        if idx + 1 >= len {
                            break;
                        }
                        idx += 1;
                    } else {
                        if idx == 0 {
                            break;
                        }
                        idx -= 1;
                    }
                    if let CommentListEntry::Comment { file, start_line, .. } = &cl.entries[idx] {
                        cl.selected = idx;
                        // Ensure visible
                        if cl.selected < cl.scroll_offset {
                            cl.scroll_offset = cl.selected;
                        } else if cl.selected >= cl.scroll_offset + self.visible_height.max(1) {
                            cl.scroll_offset = cl.selected - self.visible_height.max(1) + 1;
                        }
                        return Ok(Action::CommentFocused {
                            file: file.clone(),
                            line: *start_line,
                        });
                    }
                }
                Ok(Action::None)
            }
            (KeyCode::Enter, _) => {
                if let Some(CommentListEntry::Comment { file, start_line, .. }) = cl.entries.get(cl.selected) {
                    return Ok(Action::CommentJumped {
                        file: file.clone(),
                        line: *start_line,
                    });
                }
                Ok(Action::None)
            }
            (KeyCode::Char('x'), KeyModifiers::NONE)
            | (KeyCode::Delete, _)
            | (KeyCode::Backspace, _) => {
                if let Some(CommentListEntry::Comment { file, start_line, end_line, .. }) = cl.entries.get(cl.selected) {
                    return Ok(Action::DeleteCommentAt {
                        file: file.clone(),
                        start_line: *start_line,
                        end_line: *end_line,
                    });
                }
                Ok(Action::None)
            }
            (KeyCode::Char('c'), KeyModifiers::NONE) | (KeyCode::Esc, _) => {
                self.exit_comment_list();
                Ok(Action::None)
            }
            (KeyCode::Tab, _) => Ok(Action::SwitchFocus),
            (KeyCode::Char('q'), KeyModifiers::NONE) => Ok(Action::Quit),
            _ => Ok(Action::None),
        }
    }
}

impl FileTree {
    /// Activate content search mode (Cmd+Shift+F / Ctrl+Shift+F).
    pub fn enter_content_search(&mut self) {
        // Close any existing search or comment list
        self.exit_search(false);
        self.exit_comment_list();

        let root = self
            .model
            .entries
            .first()
            .map(|e| e.path.parent().unwrap_or(&e.path).to_path_buf())
            .unwrap_or_default();

        self.content_search = Some(ContentSearchState {
            query: String::new(),
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            match_count: 0,
            searching: false,
            saved_entries: self.model.entries.clone(),
            saved_selected: self.model.selected,
            saved_scroll_offset: self.scroll_offset,
            root,
        });
    }

    /// Exit content search mode, restoring tree state.
    pub fn exit_content_search(&mut self) {
        if let Some(cs) = self.content_search.take() {
            self.model.entries = cs.saved_entries;
            self.model.selected = cs.saved_selected;
            self.scroll_offset = cs.saved_scroll_offset;
        }
    }

    /// Convert ContentMatch results into grouped ContentSearchEntry list.
    pub fn apply_content_search_results(&mut self, results: Vec<ContentMatch>) {
        let Some(ref mut cs) = self.content_search else {
            return;
        };

        cs.searching = false;
        cs.match_count = results.len();
        cs.entries.clear();

        let mut current_file: Option<PathBuf> = None;

        for m in &results {
            if current_file.as_ref() != Some(&m.file) {
                let display_name = m
                    .file
                    .strip_prefix(&cs.root)
                    .unwrap_or(&m.file)
                    .to_string_lossy()
                    .to_string();
                cs.entries.push(ContentSearchEntry::Header { display_name });
                current_file = Some(m.file.clone());
            }
            cs.entries.push(ContentSearchEntry::Match {
                file: m.file.clone(),
                line_number: m.line_number,
                text_preview: m.line_text.clone(),
            });
        }

        // Select first non-header entry
        cs.selected = cs
            .entries
            .iter()
            .position(|e| matches!(e, ContentSearchEntry::Match { .. }))
            .unwrap_or(0);
        cs.scroll_offset = 0;
    }

    /// Handle events while in content search mode.
    fn handle_content_search_event(&mut self, key: KeyEvent) -> anyhow::Result<Action> {
        let cs = self.content_search.as_mut().unwrap();
        match key.code {
            KeyCode::Esc => {
                self.exit_content_search();
                Ok(Action::None)
            }
            KeyCode::Enter => {
                if let Some(entry) = cs.entries.get(cs.selected) {
                    match entry {
                        ContentSearchEntry::Match {
                            file, line_number, ..
                        } => {
                            let file = file.clone();
                            let line = *line_number;
                            return Ok(Action::CommentJumped { file, line });
                        }
                        ContentSearchEntry::Header { .. } => {}
                    }
                }
                Ok(Action::None)
            }
            KeyCode::Backspace => {
                cs.query.pop();
                Ok(Action::ContentSearchRequested)
            }
            KeyCode::Down => {
                self.content_search_move(true);
                Ok(Action::None)
            }
            KeyCode::Up => {
                self.content_search_move(false);
                Ok(Action::None)
            }
            KeyCode::Char(c) => {
                cs.query.push(c);
                Ok(Action::ContentSearchRequested)
            }
            _ => Ok(Action::None),
        }
    }

    /// Move selection in content search results, skipping headers.
    fn content_search_move(&mut self, down: bool) {
        let cs = self.content_search.as_mut().unwrap();
        let len = cs.entries.len();
        if len == 0 {
            return;
        }
        let mut idx = cs.selected;
        loop {
            if down {
                if idx + 1 >= len {
                    break;
                }
                idx += 1;
            } else {
                if idx == 0 {
                    break;
                }
                idx -= 1;
            }
            if matches!(cs.entries[idx], ContentSearchEntry::Match { .. }) {
                cs.selected = idx;
                // Ensure visible
                if cs.selected < cs.scroll_offset {
                    cs.scroll_offset = cs.selected;
                } else if cs.selected >= cs.scroll_offset + self.visible_height.max(1) {
                    cs.scroll_offset = cs.selected - self.visible_height.max(1) + 1;
                }
                break;
            }
        }
    }

    /// Render the content search view.
    #[allow(clippy::too_many_lines)]
    fn render_content_search(&self, area: Rect, buf: &mut Buffer, border_style: Style) {
        let cs = self.content_search.as_ref().unwrap();

        let title = if cs.searching {
            format!(" Search [{}] (searching...) ", cs.query)
        } else if cs.query.len() < 2 {
            format!(" Search [{}] ", cs.query)
        } else if cs.match_count > 0 {
            format!(" Search [{}] ({} matches) ", cs.query, cs.match_count)
        } else {
            format!(" Search [{}] (no matches) ", cs.query)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title.as_str());
        let inner = block.inner(area);
        block.render(area, buf);

        let visible_height = inner.height as usize;

        if cs.query.len() < 2 && !cs.searching {
            let msg = Line::from(Span::styled(
                "Type 2+ chars to search",
                Style::default().fg(Color::DarkGray),
            ));
            buf.set_line(inner.x, inner.y, &msg, inner.width);
            return;
        }

        if cs.entries.is_empty() && !cs.searching {
            let msg = Line::from(Span::styled(
                "No matches",
                Style::default().fg(Color::DarkGray),
            ));
            buf.set_line(inner.x, inner.y, &msg, inner.width);
            return;
        }

        for (i, entry) in cs
            .entries
            .iter()
            .skip(cs.scroll_offset)
            .take(visible_height)
            .enumerate()
        {
            let global_idx = cs.scroll_offset + i;
            let is_selected = global_idx == cs.selected;

            let line = match entry {
                ContentSearchEntry::Header { display_name, .. } => {
                    let prefix = "\u{25bc} ";
                    let max_name = (inner.width as usize).saturating_sub(prefix.len());
                    let display = if display_name.chars().count() > max_name {
                        let char_count = display_name.chars().count();
                        let skip_chars = char_count.saturating_sub(max_name.saturating_sub(1));
                        let byte_offset = unicode::char_skip_byte_offset(display_name, skip_chars);
                        format!("{prefix}\u{2026}{}", &display_name[byte_offset..])
                    } else {
                        format!("{prefix}{display_name}")
                    };
                    let st = if is_selected {
                        Style::default().fg(Color::Black).bg(Color::White)
                    } else {
                        Style::default().fg(Color::Yellow)
                    };
                    Line::from(Span::styled(display, st))
                }
                ContentSearchEntry::Match {
                    line_number,
                    text_preview,
                    ..
                } => {
                    let line_str = format!(":{}", line_number);
                    let max_text =
                        (inner.width as usize).saturating_sub(line_str.len() + 4);
                    let truncated = if text_preview.len() > max_text {
                        let end =
                            unicode::floor_char_boundary(text_preview, max_text.saturating_sub(3));
                        format!("{}...", &text_preview[..end])
                    } else {
                        text_preview.clone()
                    };

                    let base_style = if is_selected {
                        Style::default().fg(Color::Black).bg(Color::White)
                    } else {
                        Style::default()
                    };
                    let highlight_style =
                        Style::default().fg(Color::Black).bg(Color::Yellow);

                    let prefix = format!("  {line_str} ");
                    let spans = build_highlighted_spans(
                        &prefix, &truncated, &cs.query, base_style, highlight_style,
                    );
                    Line::from(spans)
                }
            };

            let y = inner.y + i as u16;
            if y < inner.y + inner.height {
                buf.set_line(inner.x, y, &line, inner.width);
            }
        }
    }
}

impl Component for FileTree {
    fn handle_event(&mut self, key: KeyEvent) -> anyhow::Result<Action> {
        // Content search mode handles its own events
        if self.content_search.is_some() {
            return self.handle_content_search_event(key);
        }

        // Comment list mode handles its own events
        if self.comment_list.is_some() {
            return self.handle_comment_list_event(key);
        }

        // Search mode handles its own events
        if self.search.is_some() {
            return self.handle_search_event(key);
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                let action = self.move_selection(1);
                self.ensure_visible(self.visible_height);
                Ok(action)
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                let action = self.move_selection(-1);
                self.ensure_visible(self.visible_height);
                Ok(action)
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Right, _) => {
                if let Some(entry) = self.model.selected_entry()
                    && entry.is_directory {
                        let is_double_tap = self
                            .last_expand_time
                            .is_some_and(|t| t.elapsed() < DOUBLE_TAP_THRESHOLD);
                        if !entry.is_expanded {
                            self.model.toggle_expand()?;
                            self.last_expand_time = Some(Instant::now());
                        } else if is_double_tap {
                            // Double-tap on expanded dir: recursively expand all
                            self.expand_all()?;
                            self.last_expand_time = None;
                        } else {
                            // Already expanded, record time for potential double-tap
                            self.last_expand_time = Some(Instant::now());
                        }
                    }
                Ok(Action::None)
            }
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, _) => {
                if let Some(entry) = self.model.selected_entry() {
                    let is_double_tap = self
                        .last_collapse_time
                        .is_some_and(|t| t.elapsed() < DOUBLE_TAP_THRESHOLD);
                    if entry.is_directory && entry.is_expanded {
                        self.model.toggle_expand()?;
                        self.last_collapse_time = Some(Instant::now());
                    } else if is_double_tap {
                        // Double-tap: fold to root-level ancestor
                        self.fold_to_root()?;
                        self.last_collapse_time = None;
                    } else if let Some(parent_idx) = self.find_parent_index() {
                        self.model.selected = parent_idx;
                        self.model.toggle_expand()?;
                        self.ensure_visible(self.visible_height);
                        self.last_collapse_time = Some(Instant::now());
                    }
                }
                Ok(Action::None)
            }
            (KeyCode::Enter, _) => {
                if let Some(entry) = self.model.selected_entry()
                    && !entry.is_directory {
                        let path = entry.path.clone();
                        return Ok(Action::FileOpened(path));
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
                self.ensure_visible(self.visible_height);
                Ok(Action::None)
            }
            (KeyCode::Char('c'), KeyModifiers::NONE) => Ok(Action::EnterCommentList),
            (KeyCode::Tab, _) => Ok(Action::SwitchFocus),
            (KeyCode::Char('q'), KeyModifiers::NONE) => Ok(Action::Quit),
            _ => Ok(Action::None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use crate::test_helpers::setup_dir_with;

    fn setup_dir(files: &[&str], dirs: &[&str]) -> TempDir {
        setup_dir_with(files, dirs, |_| "content".into())
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

    #[test]
    fn scroll_down_increases_scroll_offset() {
        let files: Vec<String> = (0..30).map(|i| format!("file{i:02}.rs")).collect();
        let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        let tmp = setup_dir(&file_refs, &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.visible_height = 20;
        assert_eq!(tree.scroll_offset, 0);

        tree.scroll_down(3);
        assert_eq!(tree.scroll_offset, 3);
        // Selection should NOT move — viewport-only scroll
        assert_eq!(tree.model.selected, 0);
    }

    #[test]
    fn scroll_up_decreases_scroll_offset() {
        let files: Vec<String> = (0..30).map(|i| format!("file{i:02}.rs")).collect();
        let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        let tmp = setup_dir(&file_refs, &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.visible_height = 20;
        tree.scroll_offset = 10;
        tree.model.selected = 15;

        tree.scroll_up(3);
        assert_eq!(tree.scroll_offset, 7);
        // Selection should NOT move
        assert_eq!(tree.model.selected, 15);
    }

    #[test]
    fn scroll_down_clamps_at_max() {
        let tmp = setup_dir(&["a.rs", "b.rs", "c.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.visible_height = 10;

        tree.scroll_down(100);
        // 3 items, visible_height 10 → max scroll is 0 (all fit)
        assert_eq!(tree.scroll_offset, 0);
    }

    #[test]
    fn scroll_up_floors_at_zero() {
        let tmp = setup_dir(&["a.rs", "b.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.visible_height = 10;
        tree.scroll_offset = 0;

        tree.scroll_up(5);
        assert_eq!(tree.scroll_offset, 0);
    }

    #[test]
    fn keyboard_nav_brings_offscreen_selection_into_view() {
        let files: Vec<String> = (0..30).map(|i| format!("file{i:02}.rs")).collect();
        let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        let tmp = setup_dir(&file_refs, &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.visible_height = 10;

        // Selection at 0, scroll viewport so selection is off-screen
        tree.scroll_offset = 15;
        assert_eq!(tree.model.selected, 0); // off-screen (above viewport)

        // Press j — selection moves to 1, ensure_visible should bring viewport back
        tree.handle_event(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(tree.model.selected, 1);
        assert!(tree.scroll_offset <= tree.model.selected,
            "viewport should scroll to show selection: offset={}, selected={}",
            tree.scroll_offset, tree.model.selected);
    }

    #[test]
    fn no_scroll_when_all_items_fit_in_visible_height() {
        // 5 files in a tree with visible_height=40 — should never scroll
        let tmp = setup_dir(&["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.visible_height = 40;

        // Move to the last item
        for _ in 0..4 {
            tree.handle_event(key(KeyCode::Char('j'))).unwrap();
        }
        assert_eq!(tree.model.selected, 4);
        assert_eq!(tree.scroll_offset, 0, "should not scroll when all items fit");
    }

    // Double-tap expand/collapse tests
    #[test]
    fn double_tap_l_expands_all_subdirectories() {
        // Structure: parent/ -> child/ -> grandchild/ -> file.rs
        let tmp = setup_dir(
            &["parent/child/grandchild/file.rs"],
            &["parent", "parent/child", "parent/child/grandchild"],
        );
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.visible_height = 40;
        // First entry is "parent" directory
        assert!(tree.model.entries[0].is_directory);
        assert!(!tree.model.entries[0].is_expanded);

        // First l: expand parent (normal)
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();
        assert!(tree.model.entries[0].is_expanded);
        // Only parent's immediate children should be visible, child/ not expanded
        let child = tree.model.entries.iter().find(|e| e.name() == "child").unwrap();
        assert!(!child.is_expanded);

        // Simulate double-tap: set last_expand_time to now
        tree.last_expand_time = Some(std::time::Instant::now());
        // Second l: should recursively expand all subdirectories
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();

        // All directories should be expanded
        for entry in &tree.model.entries {
            if entry.is_directory {
                assert!(entry.is_expanded, "directory '{}' should be expanded", entry.name());
            }
        }
        // file.rs should be present
        assert!(tree.model.entries.iter().any(|e| e.name() == "file.rs"));
    }

    #[test]
    fn double_tap_l_does_not_expand_sibling_folder() {
        // Structure: alpha/ -> inner/ -> file.rs
        //            beta/  -> other.rs
        // Double-tap on alpha should NOT expand beta
        let tmp = setup_dir(
            &["alpha/inner/file.rs", "beta/other.rs"],
            &["alpha", "alpha/inner", "beta"],
        );
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.visible_height = 40;
        // Entries sorted: alpha/ (dir), beta/ (dir)
        assert_eq!(tree.model.entries[0].name(), "alpha");
        assert_eq!(tree.model.entries[1].name(), "beta");

        // Expand alpha
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();
        assert!(tree.model.entries[0].is_expanded);

        // Double-tap: recursive expand alpha
        tree.last_expand_time = Some(std::time::Instant::now());
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();

        // alpha and its children should be expanded
        let alpha = tree.model.entries.iter().find(|e| e.name() == "alpha").unwrap();
        assert!(alpha.is_expanded);
        let inner = tree.model.entries.iter().find(|e| e.name() == "inner").unwrap();
        assert!(inner.is_expanded);

        // beta must NOT be expanded
        let beta = tree.model.entries.iter().find(|e| e.name() == "beta").unwrap();
        assert!(!beta.is_expanded, "sibling folder 'beta' should not be expanded by double-tap on 'alpha'");
    }

    #[test]
    fn single_l_on_expanded_dir_is_noop_without_double_tap() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.visible_height = 40;

        // Expand sub
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();
        assert!(tree.model.entries[0].is_expanded);
        let count = tree.model.entries.len();

        // Clear timing so it's not a double-tap
        tree.last_expand_time = None;

        // Second l without double-tap: should be noop
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();
        assert_eq!(tree.model.entries.len(), count);
    }

    #[test]
    fn double_tap_l_on_already_expanded_dir_expands_all() {
        // Structure: parent/ -> child/ -> file.rs
        let tmp = setup_dir(
            &["parent/child/file.rs"],
            &["parent", "parent/child"],
        );
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.visible_height = 40;

        // Expand parent normally (not via double-tap)
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();
        assert!(tree.model.entries[0].is_expanded);
        // child/ is visible but not expanded
        let child = tree.model.entries.iter().find(|e| e.name() == "child").unwrap();
        assert!(!child.is_expanded);

        // Clear timing to simulate time passing
        tree.last_expand_time = None;

        // First l on already-expanded parent: should record time
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();

        // Second l quickly (simulated by last_expand_time being recent):
        // last_expand_time should have been set by previous l
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();

        // All subdirectories should now be expanded
        for entry in &tree.model.entries {
            if entry.is_directory {
                assert!(entry.is_expanded, "directory '{}' should be expanded", entry.name());
            }
        }
        assert!(tree.model.entries.iter().any(|e| e.name() == "file.rs"));
    }

    #[test]
    fn double_tap_h_folds_to_root_parent() {
        // Structure: parent/ -> child/ -> file.rs
        let tmp = setup_dir(
            &["parent/child/file.rs"],
            &["parent", "parent/child"],
        );
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.visible_height = 40;

        // Expand parent and child
        tree.handle_event(key(KeyCode::Char('l'))).unwrap(); // expand parent
        tree.model.selected = 1; // select child
        tree.handle_event(key(KeyCode::Char('l'))).unwrap(); // expand child
        // Move to file.rs
        tree.model.selected = 2;
        assert_eq!(tree.model.entries[2].name(), "file.rs");

        // First h: normal fold — goes to parent dir "child" and collapses it
        tree.handle_event(key(KeyCode::Char('h'))).unwrap();
        assert_eq!(tree.model.entries[tree.model.selected].name(), "child");
        assert!(!tree.model.entries[tree.model.selected].is_expanded);

        // Simulate double-tap
        tree.last_collapse_time = Some(std::time::Instant::now());
        // Second h: should fold all the way to root parent ("parent") and collapse
        tree.handle_event(key(KeyCode::Char('h'))).unwrap();
        assert_eq!(tree.model.selected, 0);
        assert_eq!(tree.model.entries[0].name(), "parent");
        assert!(!tree.model.entries[0].is_expanded);
    }

    #[test]
    fn single_h_without_double_tap_only_folds_to_parent() {
        // Structure: parent/ -> child/ -> file.rs
        let tmp = setup_dir(
            &["parent/child/file.rs"],
            &["parent", "parent/child"],
        );
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.visible_height = 40;

        // Expand parent
        tree.handle_event(key(KeyCode::Char('l'))).unwrap();
        // Select child (collapsed)
        tree.model.selected = 1;
        assert_eq!(tree.model.entries[1].name(), "child");

        // Clear timing
        tree.last_collapse_time = None;

        // h: normal — goes to parent and collapses
        tree.handle_event(key(KeyCode::Char('h'))).unwrap();
        assert_eq!(tree.model.selected, 0);
        assert_eq!(tree.model.entries[0].name(), "parent");
        assert!(!tree.model.entries[0].is_expanded);
    }

    // ── Content search mode tests ──

    fn make_content_search_tree() -> FileTree {
        let tmp = setup_dir(&["a.rs", "b.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        tree.enter_content_search();

        // Simulate results from two files
        let matches = vec![
            crate::tree::ContentMatch {
                file: tmp.path().join("a.rs"),
                line_number: 10,
                line_text: "hello world".into(),
            },
            crate::tree::ContentMatch {
                file: tmp.path().join("a.rs"),
                line_number: 20,
                line_text: "hello again".into(),
            },
            crate::tree::ContentMatch {
                file: tmp.path().join("b.rs"),
                line_number: 5,
                line_text: "hello there".into(),
            },
        ];
        tree.apply_content_search_results(matches);
        tree
    }

    #[test]
    fn content_search_apply_creates_grouped_entries() {
        let tree = make_content_search_tree();
        let cs = tree.content_search.as_ref().unwrap();
        assert_eq!(cs.match_count, 3);
        // 2 headers + 3 matches = 5 entries
        assert_eq!(cs.entries.len(), 5);
        assert!(matches!(cs.entries[0], ContentSearchEntry::Header { .. }));
        assert!(matches!(cs.entries[1], ContentSearchEntry::Match { .. }));
        assert!(matches!(cs.entries[2], ContentSearchEntry::Match { .. }));
        assert!(matches!(cs.entries[3], ContentSearchEntry::Header { .. }));
        assert!(matches!(cs.entries[4], ContentSearchEntry::Match { .. }));
        // Selected should be first Match (index 1)
        assert_eq!(cs.selected, 1);
    }

    #[test]
    fn content_search_click_on_match_returns_jump_action() {
        let mut tree = make_content_search_tree();
        // Click row 0 = Header → no action
        let action = tree.click_entry(0);
        assert_eq!(action, Action::None);

        // Click row 1 = first Match
        let action = tree.click_entry(1);
        assert!(matches!(action, Action::CommentJumped { line: 10, .. }));

        // Click row 4 = third Match (b.rs:5)
        let action = tree.click_entry(4);
        assert!(matches!(action, Action::CommentJumped { line: 5, .. }));
    }

    #[test]
    fn content_search_click_updates_selected() {
        let mut tree = make_content_search_tree();
        tree.click_entry(4); // Click on last Match
        let cs = tree.content_search.as_ref().unwrap();
        assert_eq!(cs.selected, 4);
    }

    #[test]
    fn content_search_scroll_down_updates_scroll_offset() {
        let mut tree = make_content_search_tree();
        tree.visible_height = 2;
        tree.scroll_down(1);
        let cs = tree.content_search.as_ref().unwrap();
        assert_eq!(cs.scroll_offset, 1);
    }

    #[test]
    fn content_search_scroll_up_updates_scroll_offset() {
        let mut tree = make_content_search_tree();
        tree.visible_height = 2;
        tree.scroll_down(3);
        tree.scroll_up(1);
        let cs = tree.content_search.as_ref().unwrap();
        assert_eq!(cs.scroll_offset, 2);
    }

    #[test]
    fn content_search_scroll_clamps_at_boundaries() {
        let mut tree = make_content_search_tree();
        tree.visible_height = 2;
        tree.scroll_up(100); // Should clamp at 0
        assert_eq!(tree.content_search.as_ref().unwrap().scroll_offset, 0);
        tree.scroll_down(100); // Should clamp at max
        let cs = tree.content_search.as_ref().unwrap();
        assert_eq!(cs.scroll_offset, cs.entries.len() - tree.visible_height);
    }

    #[test]
    fn content_search_keyboard_nav_skips_headers() {
        let mut tree = make_content_search_tree();
        tree.visible_height = 10;
        let cs = tree.content_search.as_ref().unwrap();
        assert_eq!(cs.selected, 1); // First Match

        // Move down: should go to second Match (index 2), not Header (index 3)
        tree.handle_event(key(KeyCode::Down)).unwrap();
        assert_eq!(tree.content_search.as_ref().unwrap().selected, 2);

        // Move down again: should skip Header (3) and go to Match (4)
        tree.handle_event(key(KeyCode::Down)).unwrap();
        assert_eq!(tree.content_search.as_ref().unwrap().selected, 4);
    }

    #[test]
    fn content_search_esc_restores_tree_state() {
        let tmp = setup_dir(&["a.rs", "b.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        let original_entries_len = tree.model.entries.len();
        let original_selected = tree.model.selected;

        tree.enter_content_search();
        assert!(tree.content_search.is_some());

        tree.handle_event(key(KeyCode::Esc)).unwrap();
        assert!(tree.content_search.is_none());
        assert_eq!(tree.model.entries.len(), original_entries_len);
        assert_eq!(tree.model.selected, original_selected);
    }

    #[test]
    fn content_search_enter_on_match_returns_jump() {
        let mut tree = make_content_search_tree();
        tree.visible_height = 10;
        let action = tree.handle_event(key(KeyCode::Enter)).unwrap();
        assert!(matches!(action, Action::CommentJumped { line: 10, .. }));
    }

    // ── Comment list mouse tests (pre-existing gap) ──

    #[test]
    fn comment_list_click_on_comment_returns_jump_action() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        let entries = vec![
            CommentListEntry::Header {
                file: tmp.path().join("file.rs"),
                display_name: "file.rs".into(),
            },
            CommentListEntry::Comment {
                file: tmp.path().join("file.rs"),
                start_line: 42,
                end_line: 42,
                text: "a comment".into(),
            },
        ];
        tree.enter_comment_list(entries);

        // Click on Header (row 0) → no action
        let action = tree.click_entry(0);
        assert_eq!(action, Action::None);

        // Click on Comment (row 1) → jump
        let action = tree.click_entry(1);
        assert!(matches!(action, Action::CommentJumped { line: 42, .. }));
    }

    #[test]
    fn comment_list_scroll_down_updates_scroll_offset() {
        let tmp = setup_dir(&["file.rs"], &[]);
        let mut tree = FileTree::new(tmp.path(), None).unwrap();
        let entries = vec![
            CommentListEntry::Header {
                file: tmp.path().join("file.rs"),
                display_name: "file.rs".into(),
            },
            CommentListEntry::Comment {
                file: tmp.path().join("file.rs"),
                start_line: 1,
                end_line: 1,
                text: "c1".into(),
            },
            CommentListEntry::Comment {
                file: tmp.path().join("file.rs"),
                start_line: 2,
                end_line: 2,
                text: "c2".into(),
            },
        ];
        tree.enter_comment_list(entries);
        tree.visible_height = 1;
        tree.scroll_down(1);
        assert_eq!(tree.comment_list.as_ref().unwrap().scroll_offset, 1);
    }

    // ── Highlight spans tests ──

    #[test]
    fn highlight_spans_marks_single_match() {
        let base = Style::default();
        let hl = Style::default().fg(Color::Black).bg(Color::Yellow);
        let spans = build_highlighted_spans("  :10 ", "hello world", "world", base, hl);
        // prefix + "hello " + "world"
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "  :10 ");
        assert_eq!(spans[1].content, "hello ");
        assert_eq!(spans[2].content, "world");
        assert_eq!(spans[2].style, hl);
    }

    #[test]
    fn highlight_spans_case_insensitive() {
        let base = Style::default();
        let hl = Style::default().fg(Color::Black).bg(Color::Yellow);
        let spans = build_highlighted_spans("", "Hello WORLD", "hello", base, hl);
        // prefix("") + "Hello" (highlighted) + " WORLD"
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].content, "Hello"); // preserves original casing
        assert_eq!(spans[1].style, hl);
        assert_eq!(spans[2].content, " WORLD");
    }

    #[test]
    fn highlight_spans_multiple_occurrences() {
        let base = Style::default();
        let hl = Style::default().fg(Color::Black).bg(Color::Yellow);
        let spans = build_highlighted_spans("", "abcabc", "abc", base, hl);
        // prefix("") + "abc" (hl) + "abc" (hl)
        assert_eq!(spans.len(), 3);
        assert!(spans[1..].iter().all(|s| s.style == hl));
    }

    #[test]
    fn highlight_spans_no_match_returns_full_text() {
        let base = Style::default();
        let hl = Style::default().fg(Color::Black).bg(Color::Yellow);
        let spans = build_highlighted_spans("", "hello", "xyz", base, hl);
        // prefix("") + "hello"
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].content, "hello");
        assert_eq!(spans[1].style, base);
    }

    #[test]
    fn highlight_spans_empty_query_no_highlight() {
        let base = Style::default();
        let hl = Style::default().fg(Color::Black).bg(Color::Yellow);
        let spans = build_highlighted_spans("P ", "text", "", base, hl);
        assert_eq!(spans.len(), 2); // prefix + text
        assert_eq!(spans[0].content, "P ");
        assert_eq!(spans[1].content, "text");
    }
}
