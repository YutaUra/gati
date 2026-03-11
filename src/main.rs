mod app;
mod bug_report;
mod comments;
mod components;
mod diff;
mod file_tree;
mod file_viewer;
mod git_status;
mod highlight;
mod tree;
mod watcher;

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "gati", about = "A terminal tool for reviewing code, not writing it")]
struct Cli {
    /// Path to open (directory or file). Defaults to current directory.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Print a pre-filled GitHub issue URL for bug reporting and exit.
    #[arg(long)]
    bug_report: bool,
}

/// Resolved startup target from CLI arguments.
#[derive(Debug, PartialEq)]
pub struct StartupTarget {
    /// Directory to display in the file tree.
    pub dir: PathBuf,
    /// File to select in the tree (if the user passed a file path).
    pub selected_file: Option<PathBuf>,
}

/// Resolve the CLI path argument into a directory and optional selected file.
///
/// - If the path is a directory, use it directly.
/// - If the path is a file, use its parent directory and mark the file as selected.
fn resolve_target(path: &std::path::Path) -> anyhow::Result<StartupTarget> {
    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    if path.is_file() {
        let dir = path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        Ok(StartupTarget {
            dir,
            selected_file: Some(path.to_path_buf()),
        })
    } else {
        Ok(StartupTarget {
            dir: path.to_path_buf(),
            selected_file: None,
        })
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.bug_report {
        let url = bug_report::build_url("Bug report", "");
        println!("{}", url);
        bug_report::open_or_print(&url);
        return Ok(());
    }

    let target = resolve_target(&cli.path)?;
    app::run(&target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_is_current_directory() {
        let cli = Cli::try_parse_from(["gati"]).unwrap();
        assert_eq!(cli.path, PathBuf::from("."));
    }

    #[test]
    fn accepts_directory_argument() {
        let cli = Cli::try_parse_from(["gati", "src/"]).unwrap();
        assert_eq!(cli.path, PathBuf::from("src/"));
    }

    #[test]
    fn accepts_file_argument() {
        let cli = Cli::try_parse_from(["gati", "src/main.rs"]).unwrap();
        assert_eq!(cli.path, PathBuf::from("src/main.rs"));
    }

    #[test]
    fn resolve_target_with_directory_returns_dir_and_no_selected_file() {
        let target = resolve_target(std::path::Path::new("src")).unwrap();
        assert_eq!(target.dir, PathBuf::from("src"));
        assert_eq!(target.selected_file, None);
    }

    #[test]
    fn resolve_target_with_file_returns_parent_dir_and_selected_file() {
        let target = resolve_target(std::path::Path::new("src/main.rs")).unwrap();
        assert_eq!(target.dir, PathBuf::from("src"));
        assert_eq!(target.selected_file, Some(PathBuf::from("src/main.rs")));
    }

    #[test]
    fn resolve_target_with_nonexistent_path_returns_error() {
        let result = resolve_target(std::path::Path::new("/nonexistent/path"));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("does not exist"),
            "Expected 'does not exist' in error message, got: {err_msg}"
        );
    }

    #[test]
    fn bug_report_flag_parses() {
        let cli = Cli::try_parse_from(["gati", "--bug-report"]).unwrap();
        assert!(cli.bug_report);
    }

    #[test]
    fn bug_report_flag_defaults_to_false() {
        let cli = Cli::try_parse_from(["gati"]).unwrap();
        assert!(!cli.bug_report);
    }
}
