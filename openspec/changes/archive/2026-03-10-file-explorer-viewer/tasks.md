## 1. Project Scaffolding

- [x] 1.1 Initialize Rust binary crate with `cargo init` and add dependencies (ratatui, crossterm, clap, ignore, anyhow)
- [x] 1.2 Set up CLI argument parsing with clap: `gati [path]` with path defaulting to "."; if path is a file, use its parent directory and select the file
- [x] 1.3 Validate path on startup: exit with descriptive error for non-existent paths or permission errors
- [x] 1.4 Set up basic application loop: initialize terminal, run event loop, restore terminal on exit (including panic hook for terminal restoration)

## 2. File Tree Model

- [x] 2.1 Create file tree data structure: Vec of entries with path, depth, is_directory, is_expanded
- [x] 2.2 Implement directory scanning using the `ignore` crate to respect .gitignore and hide dotfiles
- [x] 2.3 Implement sorting: directories first, then files, alphabetical within each group (case-insensitive)
- [x] 2.4 Implement expand/collapse: toggle directory expanded state and rebuild visible entries; handle empty directories gracefully

## 3. File Tree Rendering

- [x] 3.1 Create FileTree component with render() and handle_event() methods
- [x] 3.2 Render tree entries with indentation based on depth, directory/file indicators
- [x] 3.3 Implement selection cursor with j/k and Up/Down arrow navigation, clamped to visible entries
- [x] 3.4 Implement tree scrolling to keep the selected entry visible when it moves beyond the viewport
- [x] 3.5 Implement Enter key: expand/collapse directories, switch focus to viewer for files
- [x] 3.6 Trigger file preview on cursor movement (yazi-style instant preview)

## 4. File Viewer

- [x] 4.1 Create FileViewer component that reads a file into Vec<String> on selection
- [x] 4.2 Render file contents with right-aligned line numbers (gutter width adjusts based on total lines)
- [x] 4.3 Implement vertical scrolling: j/k and Up/Down (line), Ctrl-d/Ctrl-u (half page), clamped to file bounds
- [x] 4.4 Detect binary files (null bytes in first 512 bytes) and show placeholder message
- [x] 4.5 Show placeholder message for empty files and when no file is selected
- [x] 4.6 Handle permission errors gracefully: display error message instead of crashing

## 5. Layout and Focus

- [x] 5.1 Implement two-pane layout: file tree (30%) left, file viewer (70%) right, with border separator
- [x] 5.2 Enforce minimum terminal size (40 columns x 10 rows); show error and exit if too small
- [x] 5.3 Implement focus state: Tab to toggle between panes, Enter on file to switch to viewer
- [x] 5.4 Highlight the focused pane's border
- [x] 5.5 Implement key hint bar at the bottom, updating based on focused pane

## 6. Integration and Polish

- [x] 6.1 Wire file tree cursor movement to file viewer: moving cursor to a file instantly loads it in the viewer
- [x] 6.2 Wire Enter on file: load file in viewer and switch focus to viewer
- [x] 6.3 Implement q to quit and restore terminal
- [x] 6.4 Handle edge cases: empty directories, permission errors, symlinks, deeply nested trees
