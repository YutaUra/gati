use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Git status for a single file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
}

impl FileStatus {
    /// Short marker string for display in the file tree.
    #[cfg(test)]
    pub fn marker(&self) -> &'static str {
        match self {
            FileStatus::Modified => "[M]",
            FileStatus::Added => "[A]",
            FileStatus::Deleted => "[D]",
            FileStatus::Renamed => "[R]",
            FileStatus::Untracked => "[?]",
        }
    }
}

/// Git status data for a repository.
#[derive(Clone)]
pub struct GitStatus {
    /// Per-file status, keyed by absolute path.
    file_statuses: HashMap<PathBuf, FileStatus>,
    /// Directories that contain changed files (absolute paths).
    changed_dirs: HashSet<PathBuf>,
}

impl GitStatus {
    /// Compute git status for the repository containing `dir`.
    /// Returns `None` if `dir` is not inside a git repository.
    pub fn from_dir(dir: &Path) -> Option<Self> {
        let repo = git2::Repository::discover(dir).ok()?;
        // Canonicalize workdir to resolve symlinks (e.g., macOS /tmp → /private/tmp)
        let workdir = repo.workdir()?.canonicalize().ok()?;

        let statuses = repo
            .statuses(Some(
                git2::StatusOptions::new()
                    .include_untracked(true)
                    .recurse_untracked_dirs(true)
                    .renames_head_to_index(true),
            ))
            .ok()?;

        let mut file_statuses = HashMap::new();

        for entry in statuses.iter() {
            let Some(rel_path) = entry.path() else {
                continue;
            };
            let abs_path = workdir.join(rel_path);
            let status = entry.status();

            let file_status = map_git2_status(status);
            if let Some(fs) = file_status {
                file_statuses.insert(abs_path, fs);
            }
        }

        // Propagate to ancestor directories
        let changed_dirs = propagate_to_dirs(&file_statuses);

        Some(Self {
            file_statuses,
            changed_dirs,
        })
    }

    /// Get the status of a file by its absolute path.
    /// Falls back to canonicalized path to handle symlinks (e.g., macOS /tmp → /private/tmp).
    pub fn file_status(&self, path: &Path) -> Option<FileStatus> {
        if let Some(fs) = self.file_statuses.get(path) {
            return Some(*fs);
        }
        path.canonicalize()
            .ok()
            .and_then(|canonical| self.file_statuses.get(&canonical).copied())
    }

    /// Return all files with the given status.
    pub fn files_with_status(&self, status: FileStatus) -> Vec<&Path> {
        self.file_statuses
            .iter()
            .filter(|(_, s)| **s == status)
            .map(|(p, _)| p.as_path())
            .collect()
    }

    /// Check if a directory contains any changed files.
    /// Falls back to canonicalized path to handle symlinks.
    pub fn dir_has_changes(&self, path: &Path) -> bool {
        if self.changed_dirs.contains(path) {
            return true;
        }
        path.canonicalize()
            .ok()
            .is_some_and(|canonical| self.changed_dirs.contains(&canonical))
    }
}

/// Map git2 status flags to our FileStatus enum.
/// Working tree status takes priority over index status.
fn map_git2_status(status: git2::Status) -> Option<FileStatus> {
    // Working tree statuses (take priority)
    if status.intersects(git2::Status::WT_MODIFIED | git2::Status::WT_TYPECHANGE) {
        return Some(FileStatus::Modified);
    }
    if status.contains(git2::Status::WT_DELETED) {
        return Some(FileStatus::Deleted);
    }
    if status.contains(git2::Status::WT_RENAMED) {
        return Some(FileStatus::Renamed);
    }
    if status.contains(git2::Status::WT_NEW) {
        return Some(FileStatus::Untracked);
    }

    // Index statuses
    if status.intersects(git2::Status::INDEX_MODIFIED | git2::Status::INDEX_TYPECHANGE) {
        return Some(FileStatus::Modified);
    }
    if status.contains(git2::Status::INDEX_NEW) {
        return Some(FileStatus::Added);
    }
    if status.contains(git2::Status::INDEX_DELETED) {
        return Some(FileStatus::Deleted);
    }
    if status.contains(git2::Status::INDEX_RENAMED) {
        return Some(FileStatus::Renamed);
    }

    None
}

/// Propagate file statuses up to ancestor directories.
fn propagate_to_dirs(file_statuses: &HashMap<PathBuf, FileStatus>) -> HashSet<PathBuf> {
    let mut dirs = HashSet::new();
    for path in file_statuses.keys() {
        let mut parent = path.parent();
        while let Some(p) = parent {
            if dirs.contains(p) {
                break; // Already propagated from another file
            }
            dirs.insert(p.to_path_buf());
            parent = p.parent();
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Canonical path of a TempDir (resolves symlinks like /tmp → /private/tmp on macOS).
    fn canonical_tmp_path(tmp: &TempDir) -> PathBuf {
        tmp.path().canonicalize().unwrap()
    }

    /// Create a git repository in a temp directory with an initial commit.
    fn setup_git_repo(files: &[(&str, &str)]) -> TempDir {
        let tmp = TempDir::new().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();

        // Create files and make initial commit
        for (name, content) in files {
            let file_path = tmp.path().join(name);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&file_path, content).unwrap();
        }

        // Stage all files
        let mut index = repo.index().unwrap();
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();

        // Create initial commit
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("test", "test@test.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
            .unwrap();

        tmp
    }

    #[test]
    fn from_dir_returns_none_outside_git() {
        let tmp = TempDir::new().unwrap();
        let status = GitStatus::from_dir(tmp.path());
        assert!(status.is_none());
    }

    #[test]
    fn from_dir_returns_some_inside_git() {
        let tmp = setup_git_repo(&[("file.txt", "hello")]);
        let status = GitStatus::from_dir(tmp.path());
        assert!(status.is_some());
    }

    #[test]
    fn clean_repo_has_no_statuses() {
        let tmp = setup_git_repo(&[("file.txt", "hello")]);
        let root = canonical_tmp_path(&tmp);
        let status = GitStatus::from_dir(&root).unwrap();
        assert_eq!(status.file_status(&root.join("file.txt")), None);
    }

    #[test]
    fn modified_file_detected() {
        let tmp = setup_git_repo(&[("file.txt", "hello")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("file.txt"), "modified").unwrap();

        let status = GitStatus::from_dir(&root).unwrap();
        assert_eq!(
            status.file_status(&root.join("file.txt")),
            Some(FileStatus::Modified)
        );
    }

    #[test]
    fn untracked_file_detected() {
        let tmp = setup_git_repo(&[("file.txt", "hello")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("new.txt"), "new file").unwrap();

        let status = GitStatus::from_dir(&root).unwrap();
        assert_eq!(
            status.file_status(&root.join("new.txt")),
            Some(FileStatus::Untracked)
        );
    }

    #[test]
    fn staged_new_file_detected_as_added() {
        let tmp = setup_git_repo(&[("file.txt", "hello")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("new.txt"), "new file").unwrap();

        // Stage the new file
        let repo = git2::Repository::open(&root).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("new.txt")).unwrap();
        index.write().unwrap();

        let status = GitStatus::from_dir(&root).unwrap();
        assert_eq!(
            status.file_status(&root.join("new.txt")),
            Some(FileStatus::Added)
        );
    }

    #[test]
    fn deleted_file_detected() {
        let tmp = setup_git_repo(&[("file.txt", "hello")]);
        let root = canonical_tmp_path(&tmp);
        fs::remove_file(root.join("file.txt")).unwrap();

        let status = GitStatus::from_dir(&root).unwrap();
        assert_eq!(
            status.file_status(&root.join("file.txt")),
            Some(FileStatus::Deleted)
        );
    }

    #[test]
    fn dir_has_changes_for_parent_of_modified_file() {
        let tmp = setup_git_repo(&[("src/main.rs", "fn main() {}")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("src/main.rs"), "fn main() { changed }").unwrap();

        let status = GitStatus::from_dir(&root).unwrap();
        assert!(status.dir_has_changes(&root.join("src")));
    }

    #[test]
    fn dir_has_no_changes_for_clean_directory() {
        let tmp = setup_git_repo(&[("src/main.rs", "fn main() {}")]);
        let root = canonical_tmp_path(&tmp);

        let status = GitStatus::from_dir(&root).unwrap();
        assert!(!status.dir_has_changes(&root.join("src")));
    }

    #[test]
    fn file_status_matches_non_canonical_path() {
        let tmp = setup_git_repo(&[("file.txt", "hello")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("file.txt"), "modified").unwrap();

        let status = GitStatus::from_dir(tmp.path()).unwrap();
        // tmp.path() may not be canonical (e.g., /var/folders/... vs /private/var/...)
        // The tree uses paths from WalkBuilder which may be non-canonical
        let non_canonical = tmp.path().join("file.txt");
        assert_eq!(
            status.file_status(&non_canonical),
            Some(FileStatus::Modified),
            "file_status should match non-canonical paths too. \
             Non-canonical: {:?}, Canonical: {:?}",
            non_canonical,
            root.join("file.txt"),
        );
    }

    #[test]
    fn dir_has_changes_matches_non_canonical_path() {
        let tmp = setup_git_repo(&[("src/main.rs", "fn main() {}")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("src/main.rs"), "fn main() { changed }").unwrap();

        let status = GitStatus::from_dir(tmp.path()).unwrap();
        let non_canonical = tmp.path().join("src");
        assert!(
            status.dir_has_changes(&non_canonical),
            "dir_has_changes should match non-canonical paths too. \
             Non-canonical: {:?}, Canonical: {:?}",
            non_canonical,
            root.join("src"),
        );
    }

    #[test]
    fn marker_strings_are_correct() {
        assert_eq!(FileStatus::Modified.marker(), "[M]");
        assert_eq!(FileStatus::Added.marker(), "[A]");
        assert_eq!(FileStatus::Deleted.marker(), "[D]");
        assert_eq!(FileStatus::Renamed.marker(), "[R]");
        assert_eq!(FileStatus::Untracked.marker(), "[?]");
    }
}
