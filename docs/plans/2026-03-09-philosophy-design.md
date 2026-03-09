# gati — Philosophy & Concept Design

## One-liner

```
gati — a terminal tool for reviewing code, not writing it.
```

## Belief

In the age of AI coding, the developer's value shifts from **writing** to **understanding**. AI generates changes, but humans decide whether to accept them.

Today's tools are optimized for writing. Editors are heavy. Diff tools are narrow. File explorers aren't designed for review. The tool for **understanding changes** is missing.

gati fills that gap.

## Three Principles

### 1. Low cognitive load

Changed files, diffs, and surrounding code appear in a natural layout. You never have to go looking for information.

### 2. Low learning cost

You don't get lost the first time you use it. Vim-like keybindings as the foundation, with on-screen hints that make everything discoverable. No barrier for developers coming from VSCode.

### 3. No interruption of thought

See the diff. Read the surrounding code. Fix what you find. This flow completes within a single tool, through obvious operations.

## What gati is

- A code review tool that also lets you edit
- Git aware, but not a git client
- Built for terminal-native workflows (tmux, ghostty, AI coding tools)

## What gati is not

- An editor — editing is possible, but always in the context of review
- An IDE — no LSP, no debugger, no build system
- A git client — no commit, no push, no rebase

## Editing Philosophy

Editing exists within the context of review. When you notice something to fix during review, you fix it without leaving the tool. gati is not for writing code from scratch.

Editing capabilities will expand incrementally, but gati will never be called an editor.

> Write less, understand more. But fix what you find.

## Review Comment Vision

gati is a place for reviewing code, and also a place for **expressing the results of that review**.

### Inline comments

You can leave inline comments on any line — or a range of lines — of any file. Not just changed lines in a diff. Changes often affect surrounding code that wasn't modified. This is the terminal-native equivalent of GitHub PR review, completing the review workflow for AI coding.

Comments appear inline between code lines, just like GitHub PR review:

```
  4 │▎fn main() -> Result {
  5 │▎    let cfg = config::load();
  ┌─ Comment (L4-5) ──────────────────────┐
  │ This function should handle the error  │
  │ case when config file is missing.      │
  └────────────────────────────────────────┘
  6 │     println!("hello");
```

### Export

Comments are stored as structured data internally. Exported as plain text for maximum compatibility — paste directly into Claude Code, a GitHub issue, or a Slack message.

```
## src/main.rs

L4-5: This function should handle the error case when config file is missing.

L12: Use `?` operator instead of unwrap() here.

## src/config.rs

L8: This path should be configurable via environment variable.
```

### Workflow

```
review in gati → leave comments → export → send to AI (or human)
```

Instead of communicating issues one at a time, you communicate an entire review at once.

### Roadmap

- **Phase 1**: Leave inline comments in gati and export them
- **Phase 2**: Direct integration with AI coding tools (e.g. Claude Code)

## Target User

Developers who work in the terminal and use AI coding daily. This includes developers transitioning from VSCode to a terminal-native workflow with AI coding tools.

## Position

```
AI        → change
human     → review      (with gati)
human     → fix in place (with gati)
heavy edit → existing editor
git ops    → CLI or AI
```

## UI Design

### Launch

```
gati          # open current directory
gati src/     # open specific directory
```

### Default view (two-pane)

```
┌─ File Tree ─────────┬─ Preview ──────────────────────────┐
│ 📁 src/             │  1 │ use std::io;                   │
│   📄 main.rs    [M] │  2 │ use crate::config;             │
│   📄 config.rs      │  3 │                                │
│   📄 diff.rs    [A] │  4 │▎fn main() {                    │
│ 📁 tests/           │  5 │▎    let cfg = config::load();  │
│   📄 main_test.rs   │  6 │     println!("hello");         │
│                      │  7 │ }                              │
│                      │                                     │
├──────────────────────┴─────────────────────────────────────┤
│ [j/k] navigate  [Enter] open  [d] diff mode  [c] comment  │
└────────────────────────────────────────────────────────────┘
```

- **Left pane**: File tree with git status markers (`[M]` modified, `[A]` added, `[D]` deleted). Toggle between all files and changed-files-only filter.
- **Right pane**: Syntax-highlighted full file view. Changed lines marked with gutter indicators (▎).
- **Bottom bar**: Discoverable key hints — visible at all times, context-sensitive.

### Preview modes (toggle)

| Mode | Description |
|------|-------------|
| **Full file** (default) | Syntax-highlighted code, changed lines marked in gutter |
| **Unified diff** | Standard unified diff view of changes |
| **Side-by-side diff** | Old and new side by side |

Cycle through modes with a single key.

## Technology

### Language: Rust

Chosen for its mature TUI ecosystem. The goal is to focus on the product experience, not on building foundational UI components from scratch.

### Stack

| Component | Library | Rationale |
|-----------|---------|-----------|
| TUI framework | ratatui + crossterm | Industry standard, largest ecosystem |
| Syntax highlighting | syntect | Battle-tested, used by bat/delta |
| Git operations | git2 | Mature diff/blame/status API, used by gitui |
| Text editing (comments) | tui-textarea | Purpose-built for ratatui |
| File tree | tui-tree-widget (or custom) | Starting point, likely needs customization |
| CLI args | clap | De facto standard |
| Async runtime | tokio | Non-blocking git operations |
| Error handling | color-eyre | Nice panic reports for TUI apps |
| Config | serde + toml | User configuration |
| Distribution | cargo-dist | Automated binary releases + Homebrew |

## Why Not

Design decisions and the alternatives we considered but rejected.

### Why not Zig?

Zig was a strong candidate — excellent C interop, fast compile times, small binaries. However, the TUI ecosystem is immature. The only viable framework (libvaxis) has a bus factor of 1, and most UI components (file tree, diff viewer, editor widget, vim keybindings) would need to be built from scratch. Rust's ratatui ecosystem lets us focus on the review experience instead of building foundational UI infrastructure.

### Why not a full editor?

gati could grow into an editor, but that changes what the tool _is_. Editors compete with Neovim, Helix, and Zed — a crowded space with established players. A code review tool with editing capability competes with nothing. The "review tool that also edits" framing keeps the scope focused and the identity clear.

### Why not a git client?

Git operations (commit, push, rebase) are well-served by the CLI and AI coding tools like Claude Code. Adding git operations to gati would duplicate existing tools and blur the tool's purpose. gati reads git state but never writes to it.

### Why not support non-git workflows?

Focusing on git simplifies the initial design significantly. Git is the de facto standard for the target audience. Supporting other VCS (Mercurial, Jujutsu, etc.) could be considered later, but abstracting over VCS from day one would add complexity without clear benefit.

### Why not a web-based tool?

GitHub PR review already exists in the browser. gati's value is being terminal-native — it lives where the developer already works (tmux, ghostty, alongside Claude Code). A web tool would compete with GitHub directly and break the terminal-native workflow.

### Why not VSCode extension?

The target user is moving _away_ from VSCode toward a terminal-native workflow. Building a VSCode extension would serve the old workflow, not the new one. gati is part of the answer to "what replaces VSCode when AI writes the code."

## Inspirations

- yazi — file navigation UX
- lazygit — single-pane overview of git state
- delta — beautiful diff rendering
- bat — syntax-highlighted file viewing
- GitHub PR review — inline comment workflow

## Name

**gati** — from Japanese ガチ (serious, hardcore) and Sanskrit gati (movement, progress, path).

Four characters. Fits naturally in TUI tool culture alongside yazi, broot, helix.
