# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-03-10

### Added

- Git status markers in file tree: `[M]` modified, `[A]` staged, `[D]` deleted, `[R]` renamed, `[?]` untracked
- Deleted file virtual entries in tree (tracked files removed from disk appear with `[D]` marker)
- Real-time file tree refresh via filesystem watcher (`notify` crate with FSEvents on macOS)
- Diff view: per-line gutter markers (green added, red deleted) in file viewer
- Unified diff toggle with `d` key
- Changed files filter (`g` key) to show only files with git changes
- Incremental file search (`/` key) with case-insensitive filename matching
- Inline comments: cursor-line model with `j`/`k` navigation and highlight
- Single-line comment (`c` key) and range-select comment (`V` then `j`/`k` then `c`)
- Comment editing (re-press `c` on commented line) and deletion (`x` key)
- Comment export to clipboard (`e` key) with structured plain text format
- Half-page scroll (`Ctrl-d`/`Ctrl-u`) moves cursor with viewport
- Friendly error message for deleted files ("File has been deleted from disk")
- Path canonicalization fallback for macOS symlink handling (`/tmp` → `/private/tmp`)

### Dependencies

- `git2` for git status computation and diff generation
- `cli-clipboard` for comment export
- `notify-debouncer-mini` for filesystem change detection

## [0.2.0] - 2026-03-10

### Added

- Syntax highlighting in file viewer using syntect (base16-eighties theme)
- Language detection from file extension (Rust, Python, JavaScript, Markdown, TOML, etc.)
- First-line fallback detection for shebang lines (e.g., `#!/bin/bash`)
- Plain text fallback for files with unrecognized extensions

## [0.1.0] - 2026-03-10

### Added

- Two-pane TUI layout: file tree (30% left) and file viewer (70% right)
- File tree with vim-style navigation (j/k, Up/Down arrows)
- Directory expand/collapse with h/l and Left/Right arrows
- h on child entry collapses parent directory and moves cursor to parent
- Instant file preview on cursor movement (yazi-style)
- Enter to open file and switch focus to viewer
- Tab to toggle focus between panes
- File viewer with line numbers and vertical scrolling (j/k, Ctrl-d/Ctrl-u)
- .gitignore-aware file tree filtering via the `ignore` crate
- Binary file detection (null byte check in first 512 bytes)
- Graceful handling of empty files and permission errors
- Key hint bar at the bottom updating based on focused pane
- Minimum terminal size enforcement (40x10)
- Panic hook for terminal restoration on crash
- CLI: `gati [path]` with support for directory and file path arguments
