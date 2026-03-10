use std::path::{Path, PathBuf};

/// A single entry in the file tree (file or directory).
#[derive(Debug, Clone, PartialEq)]
pub struct TreeEntry {
    pub path: PathBuf,
    pub depth: usize,
    pub is_directory: bool,
    pub is_expanded: bool,
}

impl TreeEntry {
    pub fn file(path: PathBuf, depth: usize) -> Self {
        Self {
            path,
            depth,
            is_directory: false,
            is_expanded: false,
        }
    }

    pub fn directory(path: PathBuf, depth: usize) -> Self {
        Self {
            path,
            depth,
            is_directory: true,
            is_expanded: false,
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
/// Respects .gitignore and hides dotfiles. Always hides `.git`.
pub fn scan_dir(dir: &Path, depth: usize) -> anyhow::Result<Vec<TreeEntry>> {
    use ignore::WalkBuilder;

    let mut entries = Vec::new();

    let walker = WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(true) // skip dotfiles
        .build();

    for result in walker {
        let entry = result?;
        let path = entry.path().to_path_buf();

        // Skip the root directory itself
        if path == dir {
            continue;
        }

        let is_dir = entry.file_type().map_or(false, |ft| ft.is_dir());
        if is_dir {
            entries.push(TreeEntry::directory(path, depth));
        } else {
            entries.push(TreeEntry::file(path, depth));
        }
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
}

impl FileTreeModel {
    /// Build a new tree model from a root directory. The root is expanded by default.
    pub fn from_dir(root: &Path) -> anyhow::Result<Self> {
        let mut entries = scan_dir(root, 0)?;
        sort_entries(&mut entries);
        Ok(Self {
            entries,
            selected: 0,
            root: root.to_path_buf(),
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
            sort_entries(&mut children);

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a temp directory with given structure.
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
            fs::write(&path, "").unwrap();
        }
        tmp
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
    fn scan_dir_hides_dotfiles() {
        let tmp = setup_dir(&[".hidden", "visible.txt"], &[".secret"]);
        let entries = scan_dir(tmp.path(), 0).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name()).collect();
        assert!(!names.contains(&".hidden"));
        assert!(!names.contains(&".secret"));
        assert!(names.contains(&"visible.txt"));
    }

    #[test]
    fn scan_dir_hides_gitignored_files() {
        // ignore crate requires .git directory to recognize .gitignore
        let tmp = setup_dir(&["keep.rs", "build.log"], &[".git"]);
        fs::write(tmp.path().join(".gitignore"), "*.log\n").unwrap();
        let entries = scan_dir(tmp.path(), 0).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name()).collect();
        assert!(names.contains(&"keep.rs"));
        assert!(!names.contains(&"build.log"));
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
        let model = FileTreeModel::from_dir(tmp.path()).unwrap();
        let names: Vec<&str> = model.entries.iter().map(|e| e.name()).collect();
        // directories first, then files, alphabetical
        assert_eq!(names, vec!["src", "a.rs", "b.rs"]);
    }

    #[test]
    fn model_from_dir_selects_first_entry() {
        let tmp = setup_dir(&["file.txt"], &[]);
        let model = FileTreeModel::from_dir(tmp.path()).unwrap();
        assert_eq!(model.selected, 0);
    }

    #[test]
    fn toggle_expand_expands_collapsed_directory() {
        let tmp = setup_dir(&["sub/child.rs"], &["sub"]);
        let mut model = FileTreeModel::from_dir(tmp.path()).unwrap();
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
        let mut model = FileTreeModel::from_dir(tmp.path()).unwrap();
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
        let mut model = FileTreeModel::from_dir(tmp.path()).unwrap();
        let len_before = model.entries.len();
        model.selected = 0;
        model.toggle_expand().unwrap();
        assert_eq!(model.entries.len(), len_before);
    }

    #[test]
    fn toggle_expand_on_empty_directory() {
        let tmp = setup_dir(&[], &["empty"]);
        let mut model = FileTreeModel::from_dir(tmp.path()).unwrap();
        assert_eq!(model.entries[0].name(), "empty");
        model.selected = 0;
        model.toggle_expand().unwrap();
        assert!(model.entries[0].is_expanded);
        // No children added, length stays the same
        assert_eq!(model.entries.len(), 1);
    }
}
