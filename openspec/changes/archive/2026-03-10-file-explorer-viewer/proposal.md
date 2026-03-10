## Why

gati is a terminal-native code review tool. Before it can review anything, it needs the ability to navigate and display files. This is the foundation — a two-pane TUI where users can browse a file tree and read file contents. Without this, no subsequent feature (git diff, inline comments, editing) can exist.

## What Changes

- Add a Rust binary crate (`gati`) with a two-pane TUI layout
- Left pane: file tree with directory expand/collapse and file selection
- Right pane: plain text file viewer showing the selected file's contents
- CLI entry point: `gati [path]` where path defaults to current directory
- Keyboard navigation (j/k, arrows, Enter, Tab) with on-screen key hints
- No syntax highlighting, no git integration, no editing — just navigate and read

## Capabilities

### New Capabilities

- `file-tree`: Navigable file tree with directory expand/collapse, respecting .gitignore
- `file-viewer`: Plain text file viewer displaying the selected file's content with line numbers
- `tui-layout`: Two-pane layout with file tree on the left and file viewer on the right, with a bottom key hint bar

### Modified Capabilities

_None — this is the initial implementation._

## Impact

- New Rust binary crate with dependencies: ratatui, crossterm, clap
- No existing code is affected (greenfield)
- Produces a single `gati` binary
