# gati

A terminal tool for reviewing code, not writing it.

![gati demo](demo/basic.gif)

**gati** is a TUI code reviewer that lives in your terminal. Navigate your codebase with vim-style keybindings, view syntax-highlighted files with git diffs, and leave inline comments — all without leaving the command line.

## Features

- File tree with vim-style navigation
- Syntax highlighting powered by [syntect](https://github.com/trishume/syntect)
- Git status markers & inline diff view
- Inline comments with clipboard export
- Incremental file search
- Real-time file watching (auto-reload on changes)
- Bug report / feedback pipeline

## Installation

### Homebrew

```sh
brew install YutaUra/tap/gati
```

### Nix

```sh
nix profile install github:YutaUra/gati --accept-flake-config
```

The `--accept-flake-config` flag enables the binary cache so you get a pre-built binary instead of compiling from source. Without it, Nix will prompt you to approve the cache.

Or with flakes in your configuration:

```nix
{
  inputs.gati.url = "github:YutaUra/gati";
}
```

### From source

```sh
cargo install --path .
```

Requires Rust 1.85+ (edition 2024).

## Quick Start

```sh
gati              # open current directory
gati src/         # open specific directory
gati src/main.rs  # open with a specific file selected
```

## Keybindings

Press `?` to open the help dialog inside gati.

### Navigation

| Key | Action |
|-----|--------|
| `j` / `k` | Cursor up / down |
| `h` / `l` | Scroll left / right |
| `Ctrl-d` / `Ctrl-u` | Half-page scroll |
| `Tab` | Switch pane |

### File Tree

| Key | Action |
|-----|--------|
| `Enter` | Open file |
| `h` / `l` | Fold / unfold directory |
| `/` | Search |
| `g` | Changed files filter |

### Viewer

| Key | Action |
|-----|--------|
| `d` | Toggle diff |
| `c` | Add comment |
| `V` | Line select |
| `e` | Export comments |
| `b` | Toggle focus mode |

### Other

| Key | Action |
|-----|--------|
| `B` | Report bug / feedback |
| `?` | Help |
| `q` | Quit |

## Demo

<!-- Generated with VHS: https://github.com/charmbracelet/vhs -->
<!-- To regenerate, install VHS and run: vhs < demo/<name>.tape -->

### Basic navigation & file preview

![Basic navigation](demo/basic.gif)

### Git diff view & changed files filter

![Git diff](demo/git-diff.gif)

### Inline comments & export

![Comments](demo/comments.gif)

## Bug Report

From the command line:

```sh
gati --bug-report
```

Or press `B` inside the viewer to open a pre-filled GitHub issue.

## Regenerating Demo GIFs

Demo GIFs are generated with [VHS](https://github.com/charmbracelet/vhs). To regenerate:

```sh
# Install VHS (macOS)
brew install charmbracelet/tap/vhs

# Generate all demos
vhs demo/basic.tape
vhs demo/git-diff.tape
vhs demo/comments.tape
```

## License

MIT
