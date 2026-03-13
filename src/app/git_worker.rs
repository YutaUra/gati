use std::path::PathBuf;
use std::sync::mpsc;

use crate::diff;
use crate::file_viewer::FileViewer;
use crate::git_status::GitStatus;

/// Computes git status on a background thread and sends the result via a channel.
pub(crate) struct GitStatusWorker {
    receiver: mpsc::Receiver<Option<GitStatus>>,
}

impl GitStatusWorker {
    /// Spawn a background thread to compute git status for `dir`.
    pub(super) fn spawn(dir: PathBuf) -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let status = GitStatus::from_dir(&dir);
            // Ignore send error — the receiver may have been dropped if the app quit.
            let _ = sender.send(status);
        });
        Self { receiver }
    }

    /// Non-blocking check for a completed git status result.
    pub(super) fn try_recv(&self) -> Option<Option<GitStatus>> {
        self.receiver.try_recv().ok()
    }
}

/// Load a file and its diff data into the viewer.
///
/// Used by `App::new` (before `self` is constructed) and via
/// `App::handle_file_action` / `App::load_diff_for_file`.
pub(super) fn load_file_with_diff(
    viewer: &mut FileViewer,
    path: &std::path::Path,
    git_workdir: &Option<PathBuf>,
) {
    viewer.load_file(path);
    set_diff_for_file(viewer, path, git_workdir);
}

/// Compute and set diff data for a file in the viewer.
pub(super) fn set_diff_for_file(
    viewer: &mut FileViewer,
    path: &std::path::Path,
    git_workdir: &Option<PathBuf>,
) {
    if let Some(workdir) = git_workdir {
        if let Some((line_diff, unified_diff)) = diff::compute_diffs(workdir, path) {
            viewer.set_diff(Some(line_diff), Some(unified_diff));
        } else {
            viewer.set_diff(None, None);
        }
    }
}
