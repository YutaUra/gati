use std::path::{Path, PathBuf};

/// A single inline comment on a file line or range of lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    pub file: PathBuf,
    /// 1-indexed start line.
    pub start_line: usize,
    /// 1-indexed end line (same as start_line for single-line comments).
    pub end_line: usize,
    pub text: String,
}

/// In-memory store for all comments in the session.
#[derive(Debug, Default)]
pub struct CommentStore {
    comments: Vec<Comment>,
}

impl CommentStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a comment. If a comment already exists on the exact same range, replace it.
    pub fn add(&mut self, file: &Path, start_line: usize, end_line: usize, text: String) {
        // Replace existing comment on same range
        if let Some(existing) = self.comments.iter_mut().find(|c| {
            c.file == file && c.start_line == start_line && c.end_line == end_line
        }) {
            existing.text = text;
            return;
        }
        self.comments.push(Comment {
            file: file.to_path_buf(),
            start_line,
            end_line,
            text,
        });
    }

    /// Get all comments for a given file, sorted by start_line.
    pub fn for_file(&self, file: &Path) -> Vec<&Comment> {
        let mut result: Vec<&Comment> = self.comments.iter().filter(|c| c.file == file).collect();
        result.sort_by_key(|c| c.start_line);
        result
    }

    /// Find a comment that covers the given line in the given file.
    pub fn find_at_line(&self, file: &Path, line: usize) -> Option<&Comment> {
        self.comments
            .iter()
            .find(|c| c.file == file && line >= c.start_line && line <= c.end_line)
    }

    /// Delete a comment at the exact range.
    pub fn delete(&mut self, file: &Path, start_line: usize, end_line: usize) -> bool {
        let len_before = self.comments.len();
        self.comments.retain(|c| {
            !(c.file == file && c.start_line == start_line && c.end_line == end_line)
        });
        self.comments.len() < len_before
    }

    /// Check if there are any comments.
    pub fn is_empty(&self) -> bool {
        self.comments.is_empty()
    }

    /// Export all comments as structured plain text.
    pub fn export(&self) -> String {
        if self.comments.is_empty() {
            return String::new();
        }

        // Group by file
        let mut by_file: std::collections::BTreeMap<&Path, Vec<&Comment>> =
            std::collections::BTreeMap::new();
        for c in &self.comments {
            by_file.entry(&c.file).or_default().push(c);
        }

        let mut output = String::new();
        for (file, mut comments) in by_file {
            comments.sort_by_key(|c| c.start_line);
            output.push_str(&format!("## {}\n\n", file.display()));
            for c in comments {
                if c.start_line == c.end_line {
                    output.push_str(&format!("L{}: {}\n\n", c.start_line, c.text));
                } else {
                    output.push_str(&format!(
                        "L{}-{}: {}\n\n",
                        c.start_line, c.end_line, c.text
                    ));
                }
            }
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_retrieve_single_line_comment() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("/tmp/test.rs");
        store.add(&file, 5, 5, "Fix this".into());

        let comments = store.for_file(&file);
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].start_line, 5);
        assert_eq!(comments[0].text, "Fix this");
    }

    #[test]
    fn add_and_retrieve_range_comment() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("/tmp/test.rs");
        store.add(&file, 10, 15, "Refactor this block".into());

        let comments = store.for_file(&file);
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].start_line, 10);
        assert_eq!(comments[0].end_line, 15);
    }

    #[test]
    fn replace_comment_on_same_range() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("/tmp/test.rs");
        store.add(&file, 5, 5, "Original".into());
        store.add(&file, 5, 5, "Updated".into());

        let comments = store.for_file(&file);
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "Updated");
    }

    #[test]
    fn find_at_line_within_range() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("/tmp/test.rs");
        store.add(&file, 10, 15, "Range comment".into());

        assert!(store.find_at_line(&file, 12).is_some());
        assert!(store.find_at_line(&file, 10).is_some());
        assert!(store.find_at_line(&file, 15).is_some());
        assert!(store.find_at_line(&file, 9).is_none());
        assert!(store.find_at_line(&file, 16).is_none());
    }

    #[test]
    fn delete_comment() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("/tmp/test.rs");
        store.add(&file, 5, 5, "Delete me".into());

        assert!(store.delete(&file, 5, 5));
        assert!(store.for_file(&file).is_empty());
    }

    #[test]
    fn delete_nonexistent_returns_false() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("/tmp/test.rs");
        assert!(!store.delete(&file, 5, 5));
    }

    #[test]
    fn for_file_returns_sorted_by_start_line() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("/tmp/test.rs");
        store.add(&file, 20, 20, "Second".into());
        store.add(&file, 5, 5, "First".into());

        let comments = store.for_file(&file);
        assert_eq!(comments[0].start_line, 5);
        assert_eq!(comments[1].start_line, 20);
    }

    #[test]
    fn for_file_filters_by_file() {
        let mut store = CommentStore::new();
        let file_a = PathBuf::from("/tmp/a.rs");
        let file_b = PathBuf::from("/tmp/b.rs");
        store.add(&file_a, 1, 1, "A comment".into());
        store.add(&file_b, 1, 1, "B comment".into());

        assert_eq!(store.for_file(&file_a).len(), 1);
        assert_eq!(store.for_file(&file_b).len(), 1);
    }

    #[test]
    fn export_single_line_format() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("src/main.rs");
        store.add(&file, 5, 5, "Fix this".into());

        let output = store.export();
        assert!(output.contains("## src/main.rs"));
        assert!(output.contains("L5: Fix this"));
    }

    #[test]
    fn export_range_format() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("src/main.rs");
        store.add(&file, 10, 15, "Refactor".into());

        let output = store.export();
        assert!(output.contains("L10-15: Refactor"));
    }

    #[test]
    fn export_empty_returns_empty_string() {
        let store = CommentStore::new();
        assert!(store.export().is_empty());
    }

    #[test]
    fn is_empty_works() {
        let mut store = CommentStore::new();
        assert!(store.is_empty());
        store.add(Path::new("f"), 1, 1, "x".into());
        assert!(!store.is_empty());
    }
}
