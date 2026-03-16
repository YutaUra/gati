use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::git_status::{FileStatus, GitStatus};

/// A single entry in the file tree (file or directory).
#[derive(Debug, Clone, PartialEq)]
pub struct TreeEntry {
    pub path: PathBuf,
    pub depth: usize,
    pub is_directory: bool,
    pub is_expanded: bool,
    /// Git status for this file (None if clean or not in a git repo).
    pub git_status: Option<FileStatus>,
    /// Whether this entry is ignored by .gitignore.
    pub is_gitignored: bool,
}

impl TreeEntry {
    pub fn file(path: PathBuf, depth: usize) -> Self {
        Self {
            path,
            depth,
            is_directory: false,
            is_expanded: false,
            git_status: None,
            is_gitignored: false,
        }
    }

    pub fn directory(path: PathBuf, depth: usize) -> Self {
        Self {
            path,
            depth,
            is_directory: true,
            is_expanded: false,
            git_status: None,
            is_gitignored: false,
        }
    }

    /// Display name (file/directory name only, not full path).
    pub fn name(&self) -> &str {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
    }
}

/// Scan a directory and return its immediate children as TreeEntries.
/// Shows dotfiles but always hides `.git`.
/// Files/directories ignored by .gitignore are included with `is_gitignored = true`.
pub fn scan_dir(dir: &Path, depth: usize) -> anyhow::Result<Vec<TreeEntry>> {
    use ignore::WalkBuilder;

    // Pass 1: Walk with gitignore rules to collect non-ignored paths.
    let walker = WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();

    let mut non_ignored: HashSet<PathBuf> = HashSet::new();
    for result in walker {
        let entry = result?;
        let path = entry.path().to_path_buf();
        if path != dir {
            non_ignored.insert(path);
        }
    }

    // Pass 2: Read all entries from disk (excluding .git) and mark ignored ones.
    let mut entries = Vec::new();
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(entries),
    };

    for result in read_dir {
        let dir_entry = result?;
        let path = dir_entry.path();

        // Always hide .git
        if dir_entry.file_name() == ".git" {
            continue;
        }

        let is_dir = dir_entry.file_type().is_ok_and(|ft| ft.is_dir());
        let is_gitignored = !non_ignored.contains(&path);

        let mut tree_entry = if is_dir {
            TreeEntry::directory(path, depth)
        } else {
            TreeEntry::file(path, depth)
        };
        tree_entry.is_gitignored = is_gitignored;
        entries.push(tree_entry);
    }

    Ok(entries)
}

/// Sort entries: directories first, then files. Alphabetical (case-insensitive) within each group.
pub fn sort_entries(entries: &mut [TreeEntry]) {
    entries.sort_by(|a, b| {
        // Directories first
        b.is_directory
            .cmp(&a.is_directory)
            .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
    });
}

/// The file tree model manages the full tree state (all entries, selection, expansion).
pub struct FileTreeModel {
    /// All entries currently in the tree (flat list reflecting expanded state).
    pub entries: Vec<TreeEntry>,
    /// Index of the currently selected entry.
    pub selected: usize,
    /// Root directory being displayed.
    root: PathBuf,
    /// Git status for the repository (None if not inside a git repo).
    git_status: Option<GitStatus>,
    /// Whether the changed-files-only filter is active.
    pub filter_changed: bool,
}

impl FileTreeModel {
    /// Build a new tree model from a root directory. The root is expanded by default.
    pub fn from_dir(root: &Path, git_status: Option<GitStatus>) -> anyhow::Result<Self> {
        let mut entries = scan_dir(root, 0)?;
        prepare_entries(&mut entries, git_status.as_ref(), root, 0, false);
        Ok(Self {
            entries,
            selected: 0,
            root: root.to_path_buf(),
            git_status,
            filter_changed: false,
        })
    }

    /// Toggle expand/collapse on the selected entry. No-op if the selection is a file.
    pub fn toggle_expand(&mut self) -> anyhow::Result<()> {
        let Some(entry) = self.entries.get(self.selected) else {
            return Ok(());
        };

        if !entry.is_directory {
            return Ok(());
        }

        if entry.is_expanded {
            // Collapse: remove all children (entries with depth > this entry's depth
            // that follow it contiguously)
            let depth = entry.depth;
            let remove_start = self.selected + 1;
            let mut remove_end = remove_start;
            while remove_end < self.entries.len() && self.entries[remove_end].depth > depth {
                remove_end += 1;
            }
            self.entries.drain(remove_start..remove_end);
            self.entries[self.selected].is_expanded = false;
        } else {
            // Expand: scan the directory and insert children after this entry
            let path = entry.path.clone();
            let child_depth = entry.depth + 1;
            let mut children = scan_dir(&path, child_depth)?;
            prepare_entries(
                &mut children,
                self.git_status.as_ref(),
                &path,
                child_depth,
                self.filter_changed,
            );

            self.entries[self.selected].is_expanded = true;
            let insert_pos = self.selected + 1;
            for (i, child) in children.into_iter().enumerate() {
                self.entries.insert(insert_pos + i, child);
            }
        }

        Ok(())
    }

    /// Return the currently selected entry, if any.
    pub fn selected_entry(&self) -> Option<&TreeEntry> {
        self.entries.get(self.selected)
    }

    /// Select the entry at `idx` and return a reference to it, or None if out of bounds.
    pub fn select_at(&mut self, idx: usize) -> Option<&TreeEntry> {
        if idx < self.entries.len() {
            self.selected = idx;
            Some(&self.entries[idx])
        } else {
            None
        }
    }

    /// Return the path of the currently selected entry, if any.
    #[allow(dead_code)]
    pub fn selected_path(&self) -> Option<&Path> {
        self.entries.get(self.selected).map(|e| e.path.as_path())
    }

    /// Get a reference to the git status data.
    pub fn git_status_ref(&self) -> Option<&GitStatus> {
        self.git_status.as_ref()
    }

    /// Check if a directory has git changes among its descendants.
    pub fn dir_has_changes(&self, path: &Path) -> bool {
        self.git_status
            .as_ref()
            .is_some_and(|gs| gs.dir_has_changes(path))
    }

    /// Update git status annotations without rescanning the filesystem.
    /// Re-annotates all entries, injects deleted files, and reapplies the filter.
    pub fn update_git_status(&mut self, git_status: Option<GitStatus>) {
        self.git_status = git_status;

        // Remember expanded dirs and selection for restoration
        let expanded: HashSet<PathBuf> = self
            .entries
            .iter()
            .filter(|e| e.is_directory && e.is_expanded)
            .map(|e| e.path.clone())
            .collect();
        let selected_path = self.entries.get(self.selected).map(|e| e.path.clone());

        // Remove previously injected deleted-file entries (they may have changed)
        self.entries
            .retain(|e| e.git_status != Some(FileStatus::Deleted) || e.path.exists());

        // Clear all file annotations first
        for entry in &mut self.entries {
            if !entry.is_directory {
                entry.git_status = None;
            }
        }

        if let Some(ref gs) = self.git_status {
            // Re-inject deleted files at root level
            inject_deleted_files(&mut self.entries, gs, &self.root, 0);

            // Re-inject deleted files into expanded directories
            for dir_path in &expanded {
                if let Some(dir_entry) = self.entries.iter().find(|e| &e.path == dir_path) {
                    let child_depth = dir_entry.depth + 1;
                    inject_deleted_files(&mut self.entries, gs, dir_path, child_depth);
                }
            }

            // Re-annotate all entries
            annotate_entries(&mut self.entries, gs);

            if self.filter_changed {
                filter_changed_entries(&mut self.entries, gs);
            }
        }

        // Restore selection
        self.selected = selected_path
            .and_then(|p| self.entries.iter().position(|e| e.path == p))
            .unwrap_or(self.selected.min(self.entries.len().saturating_sub(1)));
    }

    /// Rescan the file tree from disk, preserving expanded directories and selection.
    /// Does NOT refresh git status — call `update_git_status()` separately if needed.
    pub fn refresh_tree(&mut self) -> anyhow::Result<()> {
        // Remember which directories are expanded
        let expanded: std::collections::HashSet<PathBuf> = self
            .entries
            .iter()
            .filter(|e| e.is_directory && e.is_expanded)
            .map(|e| e.path.clone())
            .collect();

        // Remember current selection path
        let selected_path = self.entries.get(self.selected).map(|e| e.path.clone());

        // Rescan root (git status is NOT refreshed here; use update_git_status() separately)
        let mut entries = scan_dir(&self.root, 0)?;
        prepare_entries(
            &mut entries,
            self.git_status.as_ref(),
            &self.root,
            0,
            self.filter_changed,
        );

        // Re-expand previously expanded directories (depth-first)
        let mut i = 0;
        while i < entries.len() {
            if entries[i].is_directory && expanded.contains(&entries[i].path) {
                let path = entries[i].path.clone();
                let child_depth = entries[i].depth + 1;
                let mut children = scan_dir(&path, child_depth)?;
                prepare_entries(
                    &mut children,
                    self.git_status.as_ref(),
                    &path,
                    child_depth,
                    self.filter_changed,
                );
                entries[i].is_expanded = true;
                let insert_pos = i + 1;
                for (j, child) in children.into_iter().enumerate() {
                    entries.insert(insert_pos + j, child);
                }
            }
            i += 1;
        }

        self.entries = entries;

        // Restore selection
        self.selected = selected_path
            .and_then(|p| self.entries.iter().position(|e| e.path == p))
            .unwrap_or(self.selected.min(self.entries.len().saturating_sub(1)));

        Ok(())
    }

    /// Toggle the changed-files-only filter. No-op if not inside a git repo.
    pub fn toggle_filter(&mut self) -> anyhow::Result<()> {
        if self.git_status.is_none() {
            return Ok(());
        }

        self.filter_changed = !self.filter_changed;

        // Save current selection path for restoration
        let selected_path = self.entries.get(self.selected).map(|e| e.path.clone());

        // Rebuild root-level entries
        let mut entries = scan_dir(&self.root, 0)?;
        prepare_entries(
            &mut entries,
            self.git_status.as_ref(),
            &self.root,
            0,
            self.filter_changed,
        );
        self.entries = entries;

        // Restore selection or reset to first entry
        self.selected = selected_path
            .and_then(|p| self.entries.iter().position(|e| e.path == p))
            .unwrap_or(0);

        Ok(())
    }
}

/// Search for files matching the query (case-insensitive, file name only).
/// Returns tree entries with matching files and their ancestor directories (all expanded).
pub fn search_files(root: &Path, query: &str) -> anyhow::Result<Vec<TreeEntry>> {
    use ignore::WalkBuilder;

    let query_lower = query.to_lowercase();

    // First pass: walk to find matching file paths and their ancestors
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    let mut matching_set: HashSet<PathBuf> = HashSet::new();

    for result in walker {
        let entry = result?;
        let path = entry.path().to_path_buf();
        if path == root {
            continue;
        }
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if name.to_lowercase().contains(&query_lower) {
            matching_set.insert(path.clone());
            // Add ancestor directories
            let mut parent = path.parent();
            while let Some(p) = parent {
                if p == root {
                    break;
                }
                matching_set.insert(p.to_path_buf());
                parent = p.parent();
            }
        }
    }

    // Second pass: walk in sorted order, keeping only matching entries
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .filter_entry(|e| e.file_name() != ".git")
        .sort_by_file_name(|a: &OsStr, b: &OsStr| {
            let a_lower = a.to_ascii_lowercase();
            let b_lower = b.to_ascii_lowercase();
            a_lower.cmp(&b_lower)
        })
        .build();

    let mut entries = Vec::new();
    for result in walker {
        let entry = result?;
        let path = entry.path().to_path_buf();
        if path == root {
            continue;
        }
        if !matching_set.contains(&path) {
            continue;
        }

        let depth = path
            .strip_prefix(root)
            .map(|rel| rel.components().count() - 1)
            .unwrap_or(0);

        let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());
        if is_dir {
            let mut e = TreeEntry::directory(path, depth);
            e.is_expanded = true;
            entries.push(e);
        } else {
            entries.push(TreeEntry::file(path, depth));
        }
    }

    Ok(entries)
}

/// A single match from searching file contents.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentMatch {
    pub file: PathBuf,
    pub line_number: usize, // 1-indexed
    pub line_text: String,  // trimmed line content
}

/// Search file contents under `root` for lines matching `query` (case-insensitive substring).
/// Respects .gitignore, skips binary files (null byte in first 512 bytes) and files > 1MB.
/// Returns at most `max_matches` results, sorted by file path.
pub fn search_file_contents(
    root: &Path,
    query: &str,
    max_matches: usize,
) -> anyhow::Result<Vec<ContentMatch>> {
    use ignore::WalkBuilder;
    use std::io::{BufRead, BufReader};

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(false)
        .filter_entry(|e| e.file_name() != ".git")
        .sort_by_file_name(|a, b| {
            let a_lower = a.to_ascii_lowercase();
            let b_lower = b.to_ascii_lowercase();
            a_lower.cmp(&b_lower)
        })
        .build();

    for result in walker {
        if matches.len() >= max_matches {
            break;
        }

        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();

        // Skip files > 1MB
        if let Ok(meta) = path.metadata()
            && meta.len() > 1_048_576
        {
            continue;
        }

        // Read the file; skip binary files (null byte in first 512 bytes)
        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let reader = BufReader::new(file);
        let mut is_first_chunk = true;

        for (line_idx, line_result) in reader.lines().enumerate() {
            if matches.len() >= max_matches {
                break;
            }

            let line = match line_result {
                Ok(l) => l,
                Err(_) => break, // likely binary or encoding issue
            };

            // Binary check on first line content
            if is_first_chunk {
                is_first_chunk = false;
                let check_len = line.len().min(512);
                if line.as_bytes()[..check_len].contains(&0) {
                    break;
                }
            }

            if line.to_lowercase().contains(&query_lower) {
                matches.push(ContentMatch {
                    file: path.to_path_buf(),
                    line_number: line_idx + 1,
                    line_text: line.trim().to_string(),
                });
            }
        }
    }

    Ok(matches)
}

/// Post-process scanned entries: inject deleted files, sort, annotate git status,
/// and optionally filter to changed-only. This is the shared pipeline used by
/// `from_dir`, `toggle_expand`, `refresh_tree`, and `toggle_filter`.
fn prepare_entries(
    entries: &mut Vec<TreeEntry>,
    gs: Option<&GitStatus>,
    dir: &Path,
    depth: usize,
    filter_changed: bool,
) {
    if let Some(gs) = gs {
        inject_deleted_files(entries, gs, dir, depth);
    }
    sort_entries(entries);
    if let Some(gs) = gs {
        annotate_entries(entries, gs);
        if filter_changed {
            filter_changed_entries(entries, gs);
        }
    }
}

/// Filter entries to only include files with git changes and directories containing changes.
fn filter_changed_entries(entries: &mut Vec<TreeEntry>, gs: &GitStatus) {
    entries.retain(|e| {
        if e.is_directory {
            gs.dir_has_changes(&e.path)
        } else {
            e.git_status.is_some()
        }
    });
}

/// Annotate tree entries with git status information.
fn annotate_entries(entries: &mut [TreeEntry], gs: &GitStatus) {
    for entry in entries.iter_mut() {
        if !entry.is_directory {
            entry.git_status = gs.file_status(&entry.path);
        }
    }
}

/// Inject deleted files as virtual entries into the tree.
/// Deleted files exist in git but not on disk, so `scan_dir` cannot find them.
/// `dir` is the directory being listed, `depth` is the depth of entries in that directory.
fn inject_deleted_files(entries: &mut Vec<TreeEntry>, gs: &GitStatus, dir: &Path, depth: usize) {
    let existing: HashSet<PathBuf> = entries.iter().map(|e| e.path.clone()).collect();

    for deleted_path in gs.files_with_status(crate::git_status::FileStatus::Deleted) {
        // Only include files whose parent matches this directory
        let parent = deleted_path.parent();
        // dir may not be canonical; compare both canonical and raw
        let dir_matches = parent == Some(dir)
            || dir
                .canonicalize()
                .ok()
                .is_some_and(|canon| parent == Some(canon.as_path()));

        if dir_matches && !existing.contains(deleted_path) {
            let mut entry = TreeEntry::file(deleted_path.to_path_buf(), depth);
            entry.git_status = Some(crate::git_status::FileStatus::Deleted);
            entries.push(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    use crate::test_helpers::setup_dir_with;

    fn setup_dir(files: &[&str], dirs: &[&str]) -> TempDir {
        setup_dir_with(files, dirs, |_| String::new())
    }

    #[test]
    fn file_entry_is_not_directory_and_not_expanded() {
        let entry = TreeEntry::file(PathBuf::from("src/main.rs"), 1);
        assert!(!entry.is_directory);
        assert!(!entry.is_expanded);
        assert_eq!(entry.depth, 1);
    }

    #[test]
    fn directory_entry_is_directory_and_not_expanded() {
        let entry = TreeEntry::directory(PathBuf::from("src"), 0);
        assert!(entry.is_directory);
        assert!(!entry.is_expanded);
    }

    #[test]
    fn name_returns_file_name_only() {
        let entry = TreeEntry::file(PathBuf::from("src/main.rs"), 1);
        assert_eq!(entry.name(), "main.rs");
    }

    #[test]
    fn name_returns_directory_name_only() {
        let entry = TreeEntry::directory(PathBuf::from("src/components"), 1);
        assert_eq!(entry.name(), "components");
    }

    #[test]
    fn scan_dir_returns_files_and_directories() {
        let tmp = setup_dir(&["hello.txt", "world.rs"], &["subdir"]);
        let entries = scan_dir(tmp.path(), 0).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name()).collect();
        assert!(names.contains(&"hello.txt"));
        assert!(names.contains(&"world.rs"));
        assert!(names.contains(&"subdir"));
    }

    #[test]
    fn scan_dir_shows_dotfiles_but_hides_git() {
        let tmp = setup_dir(&[".hidden", "visible.txt"], &[".git", ".github"]);
        let entries = scan_dir(tmp.path(), 0).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name()).collect();
        assert!(names.contains(&".hidden"));
        assert!(names.contains(&".github"));
        assert!(names.contains(&"visible.txt"));
        assert!(!names.contains(&".git"));
    }

    #[test]
    fn scan_dir_includes_gitignored_files_with_flag() {
        // ignore crate requires .git directory to recognize .gitignore
        let tmp = setup_dir(&["keep.rs", "build.log"], &[".git"]);
        fs::write(tmp.path().join(".gitignore"), "*.log\n").unwrap();
        let entries = scan_dir(tmp.path(), 0).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name()).collect();
        assert!(names.contains(&"keep.rs"));
        assert!(names.contains(&"build.log"), "Gitignored file should be included");

        let ignored = entries.iter().find(|e| e.name() == "build.log").unwrap();
        assert!(ignored.is_gitignored, "Gitignored file should have is_gitignored = true");

        let kept = entries.iter().find(|e| e.name() == "keep.rs").unwrap();
        assert!(!kept.is_gitignored, "Non-ignored file should have is_gitignored = false");
    }

    #[test]
    fn scan_dir_includes_gitignored_directory_with_flag() {
        let tmp = setup_dir(&["dist/bundle.js"], &[".git", "dist", "src"]);
        fs::write(tmp.path().join(".gitignore"), "dist/\n").unwrap();
        let entries = scan_dir(tmp.path(), 0).unwrap();

        let dist = entries.iter().find(|e| e.name() == "dist").unwrap();
        assert!(dist.is_gitignored, "Gitignored directory should have is_gitignored = true");
        assert!(dist.is_directory);

        let src = entries.iter().find(|e| e.name() == "src").unwrap();
        assert!(!src.is_gitignored, "Non-ignored directory should have is_gitignored = false");
    }

    #[test]
    fn scan_dir_sets_correct_depth() {
        let tmp = setup_dir(&["file.txt"], &[]);
        let entries = scan_dir(tmp.path(), 3).unwrap();
        assert!(entries.iter().all(|e| e.depth == 3));
    }

    #[test]
    fn sort_entries_puts_directories_before_files() {
        let mut entries = vec![
            TreeEntry::file(PathBuf::from("b.rs"), 0),
            TreeEntry::directory(PathBuf::from("src"), 0),
            TreeEntry::file(PathBuf::from("a.rs"), 0),
        ];
        sort_entries(&mut entries);
        assert!(entries[0].is_directory, "First entry should be a directory");
        assert!(!entries[1].is_directory);
        assert!(!entries[2].is_directory);
    }

    #[test]
    fn sort_entries_alphabetical_case_insensitive() {
        let mut entries = vec![
            TreeEntry::file(PathBuf::from("README.md"), 0),
            TreeEntry::file(PathBuf::from("api.rs"), 0),
            TreeEntry::file(PathBuf::from("Build.rs"), 0),
        ];
        sort_entries(&mut entries);
        let names: Vec<&str> = entries.iter().map(|e| e.name()).collect();
        assert_eq!(names, vec!["api.rs", "Build.rs", "README.md"]);
    }

    #[test]
    fn model_from_dir_lists_root_contents_sorted() {
        let tmp = setup_dir(&["b.rs", "a.rs"], &["src"]);
        let model = FileTreeModel::from_dir(tmp.path(), None).unwrap();
        let names: Vec<&str> = model.entries.iter().map(|e| e.name()).collect();
        // directories first, then files, alphabetical
        assert_eq!(names, vec!["src", "a.rs", "b.rs"]);
    }

    #[test]
    fn model_from_dir_selects_first_entry() {
        let tmp = setup_dir(&["file.txt"], &[]);
        let model = FileTreeModel::from_dir(tmp.path(), None).unwrap();
        assert_eq!(model.selected, 0);
    }

    #[test]
    fn toggle_expand_expands_collapsed_directory() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut model = FileTreeModel::from_dir(tmp.path(), None).unwrap();
        // First entry should be the "sub" directory
        assert!(model.entries[0].is_directory);
        assert!(!model.entries[0].is_expanded);

        model.selected = 0;
        model.toggle_expand().unwrap();

        assert!(model.entries[0].is_expanded);
        // Should now have child entries after "sub"
        assert!(model.entries.len() > 1);
        assert_eq!(model.entries[1].name(), "child.rs");
    }

    #[test]
    fn toggle_expand_collapses_expanded_directory() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut model = FileTreeModel::from_dir(tmp.path(), None).unwrap();
        model.selected = 0;
        model.toggle_expand().unwrap(); // expand
        let expanded_len = model.entries.len();

        model.selected = 0;
        model.toggle_expand().unwrap(); // collapse

        assert!(!model.entries[0].is_expanded);
        assert!(model.entries.len() < expanded_len);
    }

    #[test]
    fn toggle_expand_on_file_is_noop() {
        let tmp = setup_dir(&["file.txt"], &[]);
        let mut model = FileTreeModel::from_dir(tmp.path(), None).unwrap();
        let len_before = model.entries.len();
        model.selected = 0;
        model.toggle_expand().unwrap();
        assert_eq!(model.entries.len(), len_before);
    }

    #[test]
    fn toggle_expand_on_empty_directory() {
        let tmp = setup_dir(&[], &["empty"]);
        let mut model = FileTreeModel::from_dir(tmp.path(), None).unwrap();
        assert_eq!(model.entries[0].name(), "empty");
        model.selected = 0;
        model.toggle_expand().unwrap();
        assert!(model.entries[0].is_expanded);
        // No children added, length stays the same
        assert_eq!(model.entries.len(), 1);
    }

    /// Helper: create a git repo with initial commit and return (TempDir, canonical root).
    fn setup_git_repo(files: &[(&str, &str)]) -> (TempDir, PathBuf) {
        let tmp = crate::test_helpers::setup_git_repo(files);
        let root = crate::test_helpers::canonical_tmp_path(&tmp);
        (tmp, root)
    }

    #[test]
    fn update_git_status_annotates_modified_file() {
        let (_tmp, root) = setup_git_repo(&[("file.txt", "hello")]);

        // Start with no git status
        let mut model = FileTreeModel::from_dir(&root, None).unwrap();
        let file_entry = model.entries.iter().find(|e| e.name() == "file.txt").unwrap();
        assert_eq!(file_entry.git_status, None, "Initially no git status");

        // Modify file externally
        fs::write(root.join("file.txt"), "modified").unwrap();

        // Update git status separately (simulates background worker completing)
        let gs = GitStatus::from_dir(&root);
        model.update_git_status(gs);

        // Now file should show Modified
        let file_entry = model.entries.iter().find(|e| e.name() == "file.txt").unwrap();
        assert_eq!(
            file_entry.git_status,
            Some(FileStatus::Modified),
            "After update_git_status, modified file should have Modified status"
        );
    }

    #[test]
    fn refresh_tree_then_update_git_status() {
        let (_tmp, root) = setup_git_repo(&[("file.txt", "hello")]);

        let gs = GitStatus::from_dir(&root);
        let mut model = FileTreeModel::from_dir(&root, gs).unwrap();
        let file_entry = model.entries.iter().find(|e| e.name() == "file.txt").unwrap();
        assert_eq!(file_entry.git_status, None, "Clean file should have no status");

        // Modify file externally
        fs::write(root.join("file.txt"), "modified").unwrap();

        // refresh_tree rescans FS layout but does not update git status
        model.refresh_tree().unwrap();
        let file_entry = model.entries.iter().find(|e| e.name() == "file.txt").unwrap();
        assert_eq!(file_entry.git_status, None, "refresh_tree alone does not update git status");

        // update_git_status fills in the annotations
        let gs = GitStatus::from_dir(&root);
        model.update_git_status(gs);
        let file_entry = model.entries.iter().find(|e| e.name() == "file.txt").unwrap();
        assert_eq!(
            file_entry.git_status,
            Some(FileStatus::Modified),
            "update_git_status should annotate the modified file"
        );
    }

    #[test]
    fn refresh_tree_picks_up_new_root_file() {
        let tmp = setup_dir(&["a.rs"], &[]);
        let mut model = FileTreeModel::from_dir(tmp.path(), None).unwrap();
        assert_eq!(model.entries.len(), 1);

        // Create a new file externally
        fs::write(tmp.path().join("b.rs"), "new").unwrap();

        model.refresh_tree().unwrap();

        let names: Vec<&str> = model.entries.iter().map(|e| e.name()).collect();
        assert!(names.contains(&"b.rs"), "New file should appear after refresh");
        assert!(names.contains(&"a.rs"), "Existing file should still be present");
    }

    #[test]
    fn refresh_tree_removes_deleted_file() {
        let tmp = setup_dir(&["a.rs", "b.rs"], &[]);
        let mut model = FileTreeModel::from_dir(tmp.path(), None).unwrap();
        assert_eq!(model.entries.len(), 2);

        fs::remove_file(tmp.path().join("b.rs")).unwrap();

        model.refresh_tree().unwrap();

        let names: Vec<&str> = model.entries.iter().map(|e| e.name()).collect();
        assert!(names.contains(&"a.rs"));
        assert!(!names.contains(&"b.rs"), "Deleted file should be gone after refresh");
    }

    #[test]
    fn refresh_tree_preserves_expanded_directories() {
        let tmp = setup_dir(&["sub/child.rs", "other.rs"], &["sub"]);
        let mut model = FileTreeModel::from_dir(tmp.path(), None).unwrap();

        // Expand "sub"
        model.selected = 0; // sub directory
        model.toggle_expand().unwrap();
        assert!(model.entries[0].is_expanded);
        assert_eq!(model.entries[1].name(), "child.rs");

        // Add a new root file externally
        fs::write(tmp.path().join("new.rs"), "").unwrap();

        model.refresh_tree().unwrap();

        // "sub" should still be expanded with child visible
        let sub = model.entries.iter().find(|e| e.name() == "sub").unwrap();
        assert!(sub.is_expanded, "Expanded directory should stay expanded after refresh");
        let names: Vec<&str> = model.entries.iter().map(|e| e.name()).collect();
        assert!(names.contains(&"child.rs"), "Children of expanded dir should be present");
        assert!(names.contains(&"new.rs"), "New file should appear");
    }

    #[test]
    fn refresh_tree_preserves_selection() {
        let tmp = setup_dir(&["a.rs", "b.rs", "c.rs"], &[]);
        let mut model = FileTreeModel::from_dir(tmp.path(), None).unwrap();

        // Select b.rs
        let b_idx = model.entries.iter().position(|e| e.name() == "b.rs").unwrap();
        model.selected = b_idx;

        // Add a new file
        fs::write(tmp.path().join("d.rs"), "").unwrap();

        model.refresh_tree().unwrap();

        // Selection should still be on b.rs
        assert_eq!(
            model.entries[model.selected].name(),
            "b.rs",
            "Selection should be preserved after refresh"
        );
    }

    #[test]
    fn deleted_file_appears_in_tree_with_marker() {
        let (_tmp, root) = setup_git_repo(&[("file.txt", "hello"), ("keep.txt", "keep")]);

        // Delete a tracked file
        fs::remove_file(root.join("file.txt")).unwrap();

        let gs = GitStatus::from_dir(&root);
        let model = FileTreeModel::from_dir(&root, gs).unwrap();

        // Deleted file should appear in tree with [D] status
        let deleted = model.entries.iter().find(|e| e.name() == "file.txt");
        assert!(deleted.is_some(), "Deleted tracked file should appear in tree");
        assert_eq!(
            deleted.unwrap().git_status,
            Some(FileStatus::Deleted),
            "Deleted file should have Deleted status"
        );

        // Non-deleted file should still be present
        assert!(model.entries.iter().any(|e| e.name() == "keep.txt"));
    }

    #[test]
    fn deleted_file_in_subdirectory_appears_when_expanded() {
        let (_tmp, root) = setup_git_repo(&[("sub/child.rs", "fn main() {}")]);

        // Delete the tracked file
        fs::remove_file(root.join("sub/child.rs")).unwrap();

        let gs = GitStatus::from_dir(&root);
        let mut model = FileTreeModel::from_dir(&root, gs).unwrap();

        // Expand "sub"
        model.selected = 0;
        model.toggle_expand().unwrap();

        let deleted = model.entries.iter().find(|e| e.name() == "child.rs");
        assert!(deleted.is_some(), "Deleted file should appear when parent is expanded");
        assert_eq!(deleted.unwrap().git_status, Some(FileStatus::Deleted));
    }

    #[test]
    fn deleted_file_appears_in_changed_filter() {
        let (_tmp, root) = setup_git_repo(&[("file.txt", "hello"), ("keep.txt", "keep")]);

        fs::remove_file(root.join("file.txt")).unwrap();

        let gs = GitStatus::from_dir(&root);
        let mut model = FileTreeModel::from_dir(&root, gs).unwrap();

        model.toggle_filter().unwrap();

        let names: Vec<&str> = model.entries.iter().map(|e| e.name()).collect();
        assert!(names.contains(&"file.txt"), "Deleted file should appear in changed filter");
        assert!(!names.contains(&"keep.txt"), "Clean file should not appear in changed filter");
    }

    #[test]
    fn search_file_contents_finds_matching_lines() {
        let tmp = setup_dir_with(
            &["hello.txt", "world.rs"],
            &[],
            |name| match name {
                "hello.txt" => "line one\nfind me here\nline three".into(),
                "world.rs" => "fn main() {}\nfind me too".into(),
                _ => String::new(),
            },
        );

        let results = search_file_contents(tmp.path(), "find me", 100).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|m| m.line_text.contains("find me")));
    }

    #[test]
    fn search_file_contents_case_insensitive() {
        let tmp = setup_dir_with(
            &["test.txt"],
            &[],
            |_| "Hello World\nhello world\nHELLO WORLD".into(),
        );

        let results = search_file_contents(tmp.path(), "hello", 100).unwrap();
        assert_eq!(results.len(), 3, "Case-insensitive search should match all three lines");
    }

    #[test]
    fn search_file_contents_respects_max_matches() {
        let tmp = setup_dir_with(
            &["many.txt"],
            &[],
            |_| (0..100).map(|i| format!("match line {i}")).collect::<Vec<_>>().join("\n"),
        );

        let results = search_file_contents(tmp.path(), "match", 5).unwrap();
        assert_eq!(results.len(), 5, "Should stop at max_matches");
    }

    #[test]
    fn search_file_contents_skips_binary_files() {
        let tmp = setup_dir_with(&["text.txt"], &[], |_| "searchable text".into());
        // Create a binary file manually
        fs::write(tmp.path().join("binary.bin"), b"\x00\x01\x02searchable\x03").unwrap();

        let results = search_file_contents(tmp.path(), "searchable", 100).unwrap();
        assert_eq!(results.len(), 1, "Should only find match in text file, not binary");
        assert!(results[0].file.ends_with("text.txt"));
    }

    #[test]
    fn search_file_contents_returns_correct_line_numbers() {
        let tmp = setup_dir_with(
            &["lines.txt"],
            &[],
            |_| "alpha\nbeta\ngamma\nbeta again".into(),
        );

        let results = search_file_contents(tmp.path(), "beta", 100).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].line_number, 2);
        assert_eq!(results[1].line_number, 4);
    }

    #[test]
    fn search_file_contents_trims_line_text() {
        let tmp = setup_dir_with(
            &["spaces.txt"],
            &[],
            |_| "  leading spaces  \n\ttabbed\t".into(),
        );

        let results = search_file_contents(tmp.path(), "leading", 100).unwrap();
        assert_eq!(results[0].line_text, "leading spaces");

        let results = search_file_contents(tmp.path(), "tabbed", 100).unwrap();
        assert_eq!(results[0].line_text, "tabbed");
    }
}
