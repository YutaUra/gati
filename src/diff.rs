use std::path::Path;

/// The kind of change for a single line in the working tree relative to HEAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    /// Line is unchanged.
    Unchanged,
    /// Line was added (new line not in HEAD).
    Added,
    /// Line was modified (content differs from HEAD).
    Modified,
}

/// Per-line diff information for gutter markers in normal (full file) mode.
#[derive(Debug, Clone)]
pub struct LineDiff {
    /// One entry per line in the working tree file, indicating its change status.
    pub lines: Vec<DiffLineKind>,
}

impl LineDiff {
    /// Get the diff kind for a specific line number (1-indexed).
    pub fn line_kind(&self, line_number: usize) -> DiffLineKind {
        if line_number == 0 || line_number > self.lines.len() {
            return DiffLineKind::Unchanged;
        }
        self.lines[line_number - 1]
    }
}

/// A single line in a unified diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnifiedDiffLine {
    /// Context line (unchanged).
    Context(String),
    /// Added line.
    Added(String),
    /// Removed line.
    Removed(String),
    /// Hunk header (e.g., @@ -10,5 +10,7 @@).
    HunkHeader(String),
}

/// Parsed unified diff for diff mode display.
#[derive(Debug, Clone)]
pub struct UnifiedDiff {
    pub lines: Vec<UnifiedDiffLine>,
}

/// Compute per-line diff information for a file (working tree vs HEAD).
///
/// Returns `None` if the path is not inside a git repository or an error occurs.
pub fn compute_line_diff(repo_path: &Path, file_path: &Path) -> Option<LineDiff> {
    let repo = git2::Repository::discover(repo_path).ok()?;
    let workdir = repo.workdir()?.canonicalize().ok()?;

    let rel_path = file_path.canonicalize().ok()?;
    let rel_path = rel_path.strip_prefix(&workdir).ok()?;

    // Read working tree file content
    let working_content = std::fs::read_to_string(file_path).ok()?;
    let working_lines: Vec<&str> = working_content.lines().collect();

    // Try to get HEAD blob content
    let head_content = get_head_blob_content(&repo, rel_path);

    let old_content = match head_content {
        Some(ref c) => c.as_str(),
        None => {
            // Untracked or new file: all lines are Added
            let lines = vec![DiffLineKind::Added; working_lines.len()];
            return Some(LineDiff { lines });
        }
    };

    // Use git2::Patch to compute line-level diff
    let mut result = vec![DiffLineKind::Unchanged; working_lines.len()];
    let patch = git2::Patch::from_buffers(
        old_content.as_bytes(),
        None,
        working_content.as_bytes(),
        None,
        None,
    )
    .ok()?;

    let num_hunks = patch.num_hunks();
    for hunk_idx in 0..num_hunks {
        let (_, num_lines) = patch.hunk(hunk_idx).ok()?;
        let mut removals_in_hunk = 0u32;
        let mut additions_in_hunk: Vec<u32> = Vec::new();

        for line_idx in 0..num_lines {
            let line = patch.line_in_hunk(hunk_idx, line_idx).ok()?;
            match line.origin() {
                '+' => {
                    if let Some(lineno) = line.new_lineno() {
                        additions_in_hunk.push(lineno);
                    }
                }
                '-' => {
                    removals_in_hunk += 1;
                }
                _ => {}
            }
        }

        // Lines that replace removed lines are Modified; rest are Added
        let modified_count = (removals_in_hunk as usize).min(additions_in_hunk.len());
        for (i, &lineno) in additions_in_hunk.iter().enumerate() {
            let idx = lineno as usize - 1;
            if idx < result.len() {
                result[idx] = if i < modified_count {
                    DiffLineKind::Modified
                } else {
                    DiffLineKind::Added
                };
            }
        }
    }

    Some(LineDiff { lines: result })
}

/// Compute unified diff for a file (working tree vs HEAD).
///
/// Returns `None` if not inside a git repository or an error occurs.
pub fn compute_unified_diff(repo_path: &Path, file_path: &Path) -> Option<UnifiedDiff> {
    let repo = git2::Repository::discover(repo_path).ok()?;
    let workdir = repo.workdir()?.canonicalize().ok()?;

    let rel_path = file_path.canonicalize().ok()?;
    let rel_path = rel_path.strip_prefix(&workdir).ok()?;

    let working_content = std::fs::read_to_string(file_path).ok()?;
    let head_content = get_head_blob_content(&repo, rel_path);
    let old_content = head_content.unwrap_or_default();

    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3);

    let patch = git2::Patch::from_buffers(
        old_content.as_bytes(),
        None,
        working_content.as_bytes(),
        None,
        Some(&mut opts),
    )
    .ok()?;

    let mut lines = Vec::new();
    let num_hunks = patch.num_hunks();

    for hunk_idx in 0..num_hunks {
        let (hunk, num_lines) = patch.hunk(hunk_idx).ok()?;
        let header = String::from_utf8_lossy(hunk.header()).trim().to_string();
        lines.push(UnifiedDiffLine::HunkHeader(header));

        for line_idx in 0..num_lines {
            let line = patch.line_in_hunk(hunk_idx, line_idx).ok()?;
            let content = String::from_utf8_lossy(line.content())
                .trim_end_matches('\n')
                .to_string();
            match line.origin() {
                '+' => lines.push(UnifiedDiffLine::Added(content)),
                '-' => lines.push(UnifiedDiffLine::Removed(content)),
                ' ' => lines.push(UnifiedDiffLine::Context(content)),
                _ => {}
            }
        }
    }

    Some(UnifiedDiff { lines })
}

/// Get the content of a file at HEAD.
fn get_head_blob_content(repo: &git2::Repository, rel_path: &Path) -> Option<String> {
    let head = repo.head().ok()?;
    let tree = head.peel_to_tree().ok()?;
    let entry = tree.get_path(rel_path).ok()?;
    let blob = repo.find_blob(entry.id()).ok()?;
    if blob.is_binary() {
        return None;
    }
    String::from_utf8(blob.content().to_vec()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Canonical path of a TempDir (resolves symlinks like /tmp → /private/tmp on macOS).
    fn canonical_tmp_path(tmp: &TempDir) -> PathBuf {
        tmp.path().canonicalize().unwrap()
    }

    /// Create a git repository with an initial commit containing the given files.
    fn setup_git_repo(files: &[(&str, &str)]) -> TempDir {
        let tmp = TempDir::new().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();

        for (name, content) in files {
            let file_path = tmp.path().join(name);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&file_path, content).unwrap();
        }

        let mut index = repo.index().unwrap();
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("test", "test@test.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
            .unwrap();

        tmp
    }

    #[test]
    fn line_diff_returns_none_outside_git() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("file.txt");
        fs::write(&file, "hello").unwrap();
        assert!(compute_line_diff(tmp.path(), &file).is_none());
    }

    #[test]
    fn line_diff_all_unchanged_for_clean_file() {
        let tmp = setup_git_repo(&[("file.txt", "line1\nline2\nline3")]);
        let root = canonical_tmp_path(&tmp);
        let diff = compute_line_diff(&root, &root.join("file.txt")).unwrap();
        assert_eq!(diff.lines.len(), 3);
        assert!(diff.lines.iter().all(|k| *k == DiffLineKind::Unchanged));
    }

    #[test]
    fn line_diff_detects_modified_line() {
        let tmp = setup_git_repo(&[("file.txt", "line1\nline2\nline3")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("file.txt"), "line1\nchanged\nline3").unwrap();

        let diff = compute_line_diff(&root, &root.join("file.txt")).unwrap();
        assert_eq!(diff.line_kind(1), DiffLineKind::Unchanged);
        assert_eq!(diff.line_kind(2), DiffLineKind::Modified);
        assert_eq!(diff.line_kind(3), DiffLineKind::Unchanged);
    }

    #[test]
    fn line_diff_detects_added_lines() {
        // Trailing newlines to avoid EOF-newline-change artifacts
        let tmp = setup_git_repo(&[("file.txt", "line1\nline2\n")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("file.txt"), "line1\nline2\nnew_line\n").unwrap();

        let diff = compute_line_diff(&root, &root.join("file.txt")).unwrap();
        assert_eq!(diff.line_kind(1), DiffLineKind::Unchanged);
        assert_eq!(diff.line_kind(2), DiffLineKind::Unchanged);
        assert_eq!(diff.line_kind(3), DiffLineKind::Added);
    }

    #[test]
    fn line_diff_untracked_file_all_added() {
        let tmp = setup_git_repo(&[("existing.txt", "hello")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("new.txt"), "a\nb\nc").unwrap();

        let diff = compute_line_diff(&root, &root.join("new.txt")).unwrap();
        assert_eq!(diff.lines.len(), 3);
        assert!(diff.lines.iter().all(|k| *k == DiffLineKind::Added));
    }

    #[test]
    fn line_kind_out_of_bounds_returns_unchanged() {
        let diff = LineDiff {
            lines: vec![DiffLineKind::Added],
        };
        assert_eq!(diff.line_kind(0), DiffLineKind::Unchanged);
        assert_eq!(diff.line_kind(2), DiffLineKind::Unchanged);
    }

    #[test]
    fn unified_diff_returns_none_outside_git() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("file.txt");
        fs::write(&file, "hello").unwrap();
        assert!(compute_unified_diff(tmp.path(), &file).is_none());
    }

    #[test]
    fn unified_diff_empty_for_clean_file() {
        let tmp = setup_git_repo(&[("file.txt", "line1\nline2\nline3")]);
        let root = canonical_tmp_path(&tmp);
        let diff = compute_unified_diff(&root, &root.join("file.txt")).unwrap();
        assert!(diff.lines.is_empty());
    }

    #[test]
    fn unified_diff_contains_added_lines() {
        let tmp = setup_git_repo(&[("file.txt", "line1\nline2")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("file.txt"), "line1\nline2\nnew_line").unwrap();

        let diff = compute_unified_diff(&root, &root.join("file.txt")).unwrap();
        assert!(!diff.lines.is_empty());
        assert!(diff
            .lines
            .iter()
            .any(|l| matches!(l, UnifiedDiffLine::Added(s) if s == "new_line")));
    }

    #[test]
    fn unified_diff_contains_removed_lines() {
        let tmp = setup_git_repo(&[("file.txt", "line1\nline2\nline3")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("file.txt"), "line1\nline3").unwrap();

        let diff = compute_unified_diff(&root, &root.join("file.txt")).unwrap();
        assert!(diff
            .lines
            .iter()
            .any(|l| matches!(l, UnifiedDiffLine::Removed(s) if s == "line2")));
    }

    #[test]
    fn unified_diff_contains_hunk_header() {
        let tmp = setup_git_repo(&[("file.txt", "line1\nline2\nline3")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("file.txt"), "line1\nchanged\nline3").unwrap();

        let diff = compute_unified_diff(&root, &root.join("file.txt")).unwrap();
        assert!(diff
            .lines
            .iter()
            .any(|l| matches!(l, UnifiedDiffLine::HunkHeader(_))));
    }

    #[test]
    fn unified_diff_untracked_file_all_added() {
        let tmp = setup_git_repo(&[("existing.txt", "hello")]);
        let root = canonical_tmp_path(&tmp);
        fs::write(root.join("new.txt"), "a\nb\nc").unwrap();

        let diff = compute_unified_diff(&root, &root.join("new.txt")).unwrap();
        let added_count = diff
            .lines
            .iter()
            .filter(|l| matches!(l, UnifiedDiffLine::Added(_)))
            .count();
        assert_eq!(added_count, 3);
    }
}
