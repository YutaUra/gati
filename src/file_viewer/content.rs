use std::path::{Path, PathBuf};

use crate::highlight::Highlighter;

/// Number of bytes to check for null bytes when detecting binary files.
const BINARY_CHECK_BYTES: usize = 512;

/// Content to display in the file viewer.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewerContent {
    /// No file selected yet.
    Placeholder,
    /// File loaded successfully.
    File {
        path: PathBuf,
        lines: Vec<String>,
        /// Name of the detected syntax (e.g., "Rust", "Python", "Plain Text").
        /// Stored as a String to avoid lifetime issues with SyntaxReference.
        syntax_name: String,
    },
    /// Binary file detected.
    Binary(PathBuf),
    /// Empty file.
    Empty(PathBuf),
    /// Error reading file.
    Error(String),
}

/// Read a file and classify its content for display.
///
/// Shared by `load_file` and `reload_content` to avoid duplicating the
/// read -> binary check -> empty check -> syntax detect pipeline.
pub(crate) fn read_and_classify(path: &Path, highlighter: &Highlighter) -> ViewerContent {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return ViewerContent::Error(format!(
                "{} — File has been deleted from disk",
                path.display(),
            ));
        }
        Err(e) => {
            return ViewerContent::Error(format!(
                "Cannot read {}: {}",
                path.display(),
                e,
            ));
        }
    };

    if is_binary(&bytes) {
        return ViewerContent::Binary(path.to_path_buf());
    }

    let text = String::from_utf8_lossy(&bytes);
    if text.is_empty() {
        return ViewerContent::Empty(path.to_path_buf());
    }

    let lines: Vec<String> = text.lines().map(String::from).collect();
    let first_line = lines.first().map(|s| s.as_str()).unwrap_or("");
    let syntax = highlighter.detect_syntax(path, first_line);
    let syntax_name = syntax.name.clone();

    ViewerContent::File {
        path: path.to_path_buf(),
        lines,
        syntax_name,
    }
}

/// Check if data is binary by looking for null bytes in the first N bytes.
fn is_binary(bytes: &[u8]) -> bool {
    let check_len = bytes.len().min(BINARY_CHECK_BYTES);
    bytes[..check_len].contains(&0)
}
