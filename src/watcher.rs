use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebouncedEventKind};

/// A file-system watcher that sets a flag when changes are detected.
pub struct FsWatcher {
    /// Flag: true when file changes have been detected since last check.
    changed: Arc<AtomicBool>,
    /// Keep the debouncer alive; dropping it stops the watcher.
    _debouncer: notify_debouncer_mini::Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>,
}

impl FsWatcher {
    /// Start watching `dir` recursively. Changes are debounced over `debounce` duration.
    /// Returns `None` if the watcher cannot be created (non-fatal).
    pub fn new(dir: &Path, debounce: Duration) -> Option<Self> {
        let changed = Arc::new(AtomicBool::new(false));
        let flag = changed.clone();

        let mut debouncer = new_debouncer(debounce, move |res: notify_debouncer_mini::DebounceEventResult| {
            if let Ok(events) = res {
                // Only signal on actual content changes, not just access
                if events.iter().any(|e| e.kind == DebouncedEventKind::Any) {
                    flag.store(true, Ordering::Relaxed);
                }
            }
        })
        .ok()?;

        debouncer
            .watcher()
            .watch(dir, RecursiveMode::Recursive)
            .ok()?;

        Some(Self {
            changed,
            _debouncer: debouncer,
        })
    }

    /// Check if changes were detected since last call. Resets the flag.
    pub fn has_changed(&self) -> bool {
        self.changed.swap(false, Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn detects_file_modification() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("file.txt"), "original").unwrap();

        let watcher = FsWatcher::new(tmp.path(), Duration::from_millis(100)).unwrap();

        // Initially no changes
        assert!(!watcher.has_changed());

        // Modify a file
        fs::write(tmp.path().join("file.txt"), "modified").unwrap();

        // Wait for debounce + processing
        std::thread::sleep(Duration::from_millis(300));

        assert!(watcher.has_changed(), "Should detect file modification");
        // Flag should be reset after checking
        assert!(!watcher.has_changed(), "Flag should reset after has_changed");
    }

    #[test]
    fn detects_new_file_creation() {
        let tmp = TempDir::new().unwrap();

        let watcher = FsWatcher::new(tmp.path(), Duration::from_millis(100)).unwrap();

        // Create a new file
        fs::write(tmp.path().join("new.txt"), "hello").unwrap();

        std::thread::sleep(Duration::from_millis(300));

        assert!(watcher.has_changed(), "Should detect new file creation");
    }
}
