use std::collections::HashSet;
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
    /// Snapshot of code lines at comment creation time (empty = no context).
    pub code_context: Vec<String>,
}

impl Comment {
    /// Search for code_context block in current_lines, returning the 0-indexed
    /// start position nearest to the original location.
    fn find_context_in(&self, current_lines: &[String]) -> Option<usize> {
        let ctx = &self.code_context;
        if ctx.is_empty() || ctx.len() > current_lines.len() {
            return None;
        }
        let original = self.start_line.saturating_sub(1);
        let mut best: Option<(usize, usize)> = None; // (distance, position)
        for i in 0..=(current_lines.len() - ctx.len()) {
            if current_lines[i..i + ctx.len()] == *ctx.as_slice() {
                let dist = (i as isize - original as isize).unsigned_abs();
                if best.map_or(true, |(d, _)| dist < d) {
                    best = Some((dist, i));
                }
            }
        }
        best.map(|(_, pos)| pos)
    }

    /// Check if this comment's code context no longer matches the current file content.
    /// Returns false for legacy comments with empty code_context.
    pub fn is_stale(&self, current_lines: &[String]) -> bool {
        if self.code_context.is_empty() {
            return false;
        }
        let start = self.start_line.saturating_sub(1);
        let end = self.end_line.min(current_lines.len());
        // Fast path: exact position match
        if start < end
            && end - start == self.code_context.len()
            && current_lines[start..end] == *self.code_context.as_slice()
        {
            return false;
        }
        // Fallback: search for the block elsewhere in the file
        self.find_context_in(current_lines).is_none()
    }
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
    pub fn add(
        &mut self,
        file: &Path,
        start_line: usize,
        end_line: usize,
        text: String,
        code_context: Vec<String>,
    ) {
        // Replace existing comment on same range
        if let Some(existing) = self.comments.iter_mut().find(|c| {
            c.file == file && c.start_line == start_line && c.end_line == end_line
        }) {
            existing.text = text;
            existing.code_context = code_context;
            return;
        }
        self.comments.push(Comment {
            file: file.to_path_buf(),
            start_line,
            end_line,
            text,
            code_context,
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

    /// Find a comment at the exact (start_line, end_line) range.
    pub fn find_exact(&self, file: &Path, start_line: usize, end_line: usize) -> Option<&Comment> {
        self.comments
            .iter()
            .find(|c| c.file == file && c.start_line == start_line && c.end_line == end_line)
    }

    /// Delete a comment at the exact range.
    pub fn delete(&mut self, file: &Path, start_line: usize, end_line: usize) -> bool {
        let len_before = self.comments.len();
        self.comments.retain(|c| {
            !(c.file == file && c.start_line == start_line && c.end_line == end_line)
        });
        self.comments.len() < len_before
    }

    /// Relocate comments whose code_context has moved to a different position in the file.
    /// Updates start_line/end_line to follow the code. If the context is not found,
    /// the comment stays at its original position (is_stale() will flag it).
    pub fn relocate_comments(&mut self, file: &Path, current_lines: &[String]) {
        for comment in &mut self.comments {
            if comment.file != file || comment.code_context.is_empty() {
                continue;
            }
            // Skip if already at correct position
            let start = comment.start_line.saturating_sub(1);
            let end = comment.end_line.min(current_lines.len());
            if start < end
                && end - start == comment.code_context.len()
                && current_lines[start..end] == *comment.code_context.as_slice()
            {
                continue;
            }
            // Try to relocate
            if let Some(new_start) = comment.find_context_in(current_lines) {
                let range_len = comment.end_line - comment.start_line;
                comment.start_line = new_start + 1;
                comment.end_line = new_start + 1 + range_len;
            }
        }
    }

    /// Return the set of file paths that have at least one comment.
    pub fn files_with_comments(&self) -> HashSet<&Path> {
        self.comments.iter().map(|c| c.file.as_path()).collect()
    }

    /// Return the number of comments.
    pub fn len(&self) -> usize {
        self.comments.len()
    }

    /// Check if there are any comments.
    #[cfg(test)]
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
                    output.push_str(&format!("L{}: {}\n", c.start_line, c.text));
                } else {
                    output.push_str(&format!(
                        "L{}-{}: {}\n",
                        c.start_line, c.end_line, c.text
                    ));
                }
                if !c.code_context.is_empty() {
                    output.push_str("```\n");
                    for (i, line) in c.code_context.iter().enumerate() {
                        output.push_str(&format!("{} | {}\n", c.start_line + i, line));
                    }
                    output.push_str("```\n");
                }
                output.push('\n');
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
        store.add(&file, 5, 5, "Fix this".into(), vec![]);

        let comments = store.for_file(&file);
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].start_line, 5);
        assert_eq!(comments[0].text, "Fix this");
    }

    #[test]
    fn add_and_retrieve_range_comment() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("/tmp/test.rs");
        store.add(&file, 10, 15, "Refactor this block".into(), vec![]);

        let comments = store.for_file(&file);
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].start_line, 10);
        assert_eq!(comments[0].end_line, 15);
    }

    #[test]
    fn replace_comment_on_same_range() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("/tmp/test.rs");
        store.add(&file, 5, 5, "Original".into(), vec![]);
        store.add(&file, 5, 5, "Updated".into(), vec![]);

        let comments = store.for_file(&file);
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "Updated");
    }

    #[test]
    fn find_at_line_within_range() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("/tmp/test.rs");
        store.add(&file, 10, 15, "Range comment".into(), vec![]);

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
        store.add(&file, 5, 5, "Delete me".into(), vec![]);

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
        store.add(&file, 20, 20, "Second".into(), vec![]);
        store.add(&file, 5, 5, "First".into(), vec![]);

        let comments = store.for_file(&file);
        assert_eq!(comments[0].start_line, 5);
        assert_eq!(comments[1].start_line, 20);
    }

    #[test]
    fn for_file_filters_by_file() {
        let mut store = CommentStore::new();
        let file_a = PathBuf::from("/tmp/a.rs");
        let file_b = PathBuf::from("/tmp/b.rs");
        store.add(&file_a, 1, 1, "A comment".into(), vec![]);
        store.add(&file_b, 1, 1, "B comment".into(), vec![]);

        assert_eq!(store.for_file(&file_a).len(), 1);
        assert_eq!(store.for_file(&file_b).len(), 1);
    }

    #[test]
    fn export_single_line_format() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("src/main.rs");
        store.add(&file, 5, 5, "Fix this".into(), vec![]);

        let output = store.export();
        assert!(output.contains("## src/main.rs"));
        assert!(output.contains("L5: Fix this"));
    }

    #[test]
    fn export_range_format() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("src/main.rs");
        store.add(&file, 10, 15, "Refactor".into(), vec![]);

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
        store.add(Path::new("f"), 1, 1, "x".into(), vec![]);
        assert!(!store.is_empty());
    }

    #[test]
    fn export_single_line_with_code_context() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("src/main.rs");
        store.add(
            &file,
            5,
            5,
            "Fix this".into(),
            vec!["fn example() {".into()],
        );

        let output = store.export();
        assert!(output.contains("L5: Fix this\n"));
        assert!(output.contains("```\n5 | fn example() {\n```"));
    }

    #[test]
    fn export_range_with_code_context() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("src/main.rs");
        store.add(
            &file,
            10,
            12,
            "Refactor this".into(),
            vec![
                "fn long_function() {".into(),
                "    let x = 1;".into(),
                "    let y = 2;".into(),
            ],
        );

        let output = store.export();
        assert!(output.contains("L10-12: Refactor this\n"));
        assert!(output.contains("10 | fn long_function() {"));
        assert!(output.contains("11 |     let x = 1;"));
        assert!(output.contains("12 |     let y = 2;"));
    }

    #[test]
    fn is_stale_returns_false_for_empty_code_context() {
        let comment = Comment {
            file: PathBuf::from("test.rs"),
            start_line: 1,
            end_line: 1,
            text: "note".into(),
            code_context: vec![],
        };
        let lines = vec!["anything".to_string()];
        assert!(!comment.is_stale(&lines));
    }

    #[test]
    fn is_stale_returns_false_when_lines_match() {
        let comment = Comment {
            file: PathBuf::from("test.rs"),
            start_line: 2,
            end_line: 3,
            text: "note".into(),
            code_context: vec!["line2".into(), "line3".into()],
        };
        let lines = vec!["line1".into(), "line2".into(), "line3".into()];
        assert!(!comment.is_stale(&lines));
    }

    #[test]
    fn is_stale_returns_true_when_lines_differ() {
        let comment = Comment {
            file: PathBuf::from("test.rs"),
            start_line: 1,
            end_line: 1,
            text: "note".into(),
            code_context: vec!["original".into()],
        };
        let lines = vec!["modified".to_string()];
        assert!(comment.is_stale(&lines));
    }

    #[test]
    fn is_stale_returns_true_when_lines_removed() {
        let comment = Comment {
            file: PathBuf::from("test.rs"),
            start_line: 3,
            end_line: 5,
            text: "note".into(),
            code_context: vec!["a".into(), "b".into(), "c".into()],
        };
        // File now only has 2 lines — comment range exceeds file length
        let lines = vec!["x".into(), "y".into()];
        assert!(comment.is_stale(&lines));
    }

    #[test]
    fn files_with_comments_returns_correct_set() {
        let mut store = CommentStore::new();
        let file_a = PathBuf::from("/tmp/a.rs");
        let file_b = PathBuf::from("/tmp/b.rs");
        store.add(&file_a, 1, 1, "comment a".into(), vec![]);
        store.add(&file_a, 5, 5, "comment a2".into(), vec![]);
        store.add(&file_b, 2, 2, "comment b".into(), vec![]);

        let files = store.files_with_comments();
        assert_eq!(files.len(), 2);
        assert!(files.contains(file_a.as_path()));
        assert!(files.contains(file_b.as_path()));
    }

    #[test]
    fn export_no_code_context_omits_fenced_block() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("src/main.rs");
        store.add(&file, 5, 5, "Fix this".into(), vec![]);

        let output = store.export();
        assert!(output.contains("L5: Fix this"));
        assert!(!output.contains("```"));
    }

    #[test]
    fn is_stale_returns_false_when_lines_shifted() {
        // Comment on L10, then 3 lines inserted before it → code moves to L13
        let comment = Comment {
            file: PathBuf::from("test.rs"),
            start_line: 10,
            end_line: 10,
            text: "note".into(),
            code_context: vec!["target_line".into()],
        };
        let mut lines: Vec<String> = (1..=9).map(|i| format!("line{i}")).collect();
        // Insert 3 new lines before the target
        lines.push("inserted1".into());
        lines.push("inserted2".into());
        lines.push("inserted3".into());
        lines.push("target_line".into()); // now at index 12 (L13)
        assert!(!comment.is_stale(&lines));
    }

    #[test]
    fn is_stale_returns_true_when_content_actually_changed() {
        let comment = Comment {
            file: PathBuf::from("test.rs"),
            start_line: 2,
            end_line: 3,
            text: "note".into(),
            code_context: vec!["old_a".into(), "old_b".into()],
        };
        // The original code is nowhere in the file
        let lines = vec![
            "line1".into(),
            "new_a".into(),
            "new_b".into(),
            "line4".into(),
        ];
        assert!(comment.is_stale(&lines));
    }

    #[test]
    fn relocate_updates_line_numbers_on_shift() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("test.rs");
        store.add(&file, 5, 6, "note".into(), vec!["aa".into(), "bb".into()]);

        // 3 lines inserted at the beginning → original content shifted by 3
        let lines = vec![
            "new1".into(),
            "new2".into(),
            "new3".into(),
            "xx".into(),
            "yy".into(),
            "zz".into(),
            "aa".into(), // index 6 → L7
            "bb".into(), // index 7 → L8
        ];
        store.relocate_comments(&file, &lines);

        let comments = store.for_file(&file);
        assert_eq!(comments[0].start_line, 7);
        assert_eq!(comments[0].end_line, 8);
    }

    #[test]
    fn relocate_picks_nearest_match() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("test.rs");
        // Comment at L5 with context "dup"
        store.add(&file, 5, 5, "note".into(), vec!["dup".into()]);

        // "dup" appears at index 1 (L2) and index 6 (L7); L7 is closer to L5
        let lines = vec![
            "x".into(),
            "dup".into(), // index 1, distance |1-4|=3
            "y".into(),
            "z".into(),
            "w".into(),
            "v".into(),
            "dup".into(), // index 6, distance |6-4|=2
            "u".into(),
        ];
        store.relocate_comments(&file, &lines);

        let comments = store.for_file(&file);
        assert_eq!(comments[0].start_line, 7); // index 6 + 1
    }

    #[test]
    fn relocate_noop_when_already_at_correct_position() {
        let mut store = CommentStore::new();
        let file = PathBuf::from("test.rs");
        store.add(&file, 2, 3, "note".into(), vec!["aa".into(), "bb".into()]);

        let lines = vec!["xx".into(), "aa".into(), "bb".into(), "yy".into()];
        store.relocate_comments(&file, &lines);

        let comments = store.for_file(&file);
        assert_eq!(comments[0].start_line, 2);
        assert_eq!(comments[0].end_line, 3);
    }
}
