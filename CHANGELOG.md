# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - 2026-03-12

### Added

- Syntax highlighting for 100+ additional languages (TypeScript, TSX, TOML, Nix, and more) via two-face
- Background git status computation for faster startup on large repositories

### Fixed

- Global keybindings (`b`, `?`) no longer intercept typed characters during file tree search

## [0.5.0] - 2026-03-12

### Added

- Show dotfiles (`.github/`, `.gitignore`, `.claude/`, etc.) in the file tree, hiding only `.git`

## [0.4.2] - 2026-03-12

### Added

- `--version` flag to display installed version (`gati --version`)

## [0.4.1] - 2026-03-12

### Added

- `Ctrl+C` to quit from any mode (help dialog, comment input, line select)
- Homebrew install via `brew install YutaUra/tap/gati`
- Nix flake install via `nix profile install github:YutaUra/gati`
- Homebrew formula auto-update workflow (daily polling for new releases)

### Fixed

- Terminal size reported as 0x0 in multi-layer PTY setups (e.g. zellij → kubectl exec → container); now retries for up to 2 seconds before giving up
- Terminal resize events (`Event::Resize`) are now explicitly handled in the event loop

## [0.4.0] - 2026-03-12

### Added

- Mouse wheel scrolling for file tree and file viewer
- Horizontal scrolling in file viewer (`H`/`L` keys, Shift+wheel)
- Click-to-focus pane switching
- Click-to-select, fold, and unfold in file tree
- Double-tap `l` to recursively expand all subdirectories
- Double-tap `h` to fold to root-level ancestor
- Full-context unified diff with syntax highlighting and line numbers
- Preview minimap with viewport position, diff markers, and comment indicators
- Click-to-scroll navigation on minimap
- Click-to-position cursor in file viewer
- Mouse drag line selection (reuses V-mode LineSelect)
- Help dialog overlay (`?` key) with grouped keybinding reference
- Flash message feedback for comment export (success/empty/error)
- Bug report and feedback pipeline: panic hook with pre-filled GitHub issue URL, `--bug-report` CLI flag, `B` key to open bug report in browser
- README with project overview, features, keybindings, and demo GIFs
- VHS tape files for demo GIF generation

### Changed

- Hint bar simplified to `? help  q quit` in Normal mode (full reference moved to help dialog)
- Diff view shows file content as context lines instead of "No changes" for unchanged files
- Preserve scroll position and cursor when toggling diff/preview
- File selection optimized (~63ms → ~2ms) via single-pass diff computation and deferred syntax highlighting

### Fixed

- Stale git gutter indicators after filesystem changes
- Click line mismatch when inline comments are present
- File tree visible height calculation (was hardcoded to 20)

### Dependencies

- `open` for opening URLs in browser

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
