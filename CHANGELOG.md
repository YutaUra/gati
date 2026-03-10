# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
