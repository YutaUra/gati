/// Shared test helpers used across multiple modules.
///
/// Import via `use crate::test_helpers::*;` in test modules.

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a temp directory with the given files and subdirectories.
///
/// Each file receives `content_fn(file_name)` as its content, allowing
/// callers to customise what gets written.  Intermediate parent dirs are
/// created automatically.
pub fn setup_dir_with<F>(files: &[&str], dirs: &[&str], content_fn: F) -> TempDir
where
    F: Fn(&str) -> String,
{
    let tmp = TempDir::new().unwrap();
    for d in dirs {
        fs::create_dir_all(tmp.path().join(d)).unwrap();
    }
    for f in files {
        let path = tmp.path().join(f);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content_fn(f)).unwrap();
    }
    tmp
}

/// Create a git repository in a temp directory with an initial commit.
///
/// `files` is a list of `(name, content)` tuples.  Returns the TempDir
/// (keep it alive for the test's duration).
pub fn setup_git_repo(files: &[(&str, &str)]) -> TempDir {
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

/// Canonical path of a TempDir (resolves symlinks like /tmp -> /private/tmp on macOS).
pub fn canonical_tmp_path(tmp: &TempDir) -> PathBuf {
    tmp.path().canonicalize().unwrap()
}
