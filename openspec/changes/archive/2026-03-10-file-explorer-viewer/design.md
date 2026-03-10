## Context

gati is a new Rust TUI application with no existing code. This is the v0.1 foundation: a two-pane file explorer and viewer. The chosen stack is ratatui + crossterm for TUI rendering, and clap for CLI argument parsing. The target users are terminal-native developers familiar with tools like yazi and lazygit.

## Goals / Non-Goals

**Goals:**

- Render a responsive two-pane layout (file tree + file viewer)
- Navigate the file tree with vim-like keybindings (j/k) and arrow keys
- Display file contents with line numbers in the viewer pane
- Instant file preview on cursor movement (yazi-style)
- Launch with `gati [path]`, defaulting to current directory; file paths open the parent directory with the file selected
- Show discoverable key hints at the bottom of the screen
- Graceful error handling for permissions, binary files, empty directories

**Non-Goals:**

- Syntax highlighting
- Git integration (status markers, diff)
- File editing
- Async file loading or streaming for large files
- Configuration file support
- Plugin or extension system

## Decisions

### Application architecture: Component-based with event loop

Use a main event loop that reads crossterm events and dispatches to focused components. Each pane (FileTree, FileViewer, KeyHints) is a struct implementing a `Component` trait with `handle_event()` and `render()` methods.

_Why not a framework like `ratatui-templates`_: The app is simple enough that a hand-rolled event loop is clearer and avoids unnecessary abstraction.

### File tree: Custom implementation over tui-tree-widget

Build a simple tree model (Vec of entries with depth/expanded state) rather than pulling in `tui-tree-widget`. The v0.1 tree only needs expand/collapse and selection — no drag-and-drop, no filtering.

_Why not tui-tree-widget_: It adds a dependency for functionality we only partially need. A flat Vec with indentation is simpler to reason about and customize later (e.g., git status markers in v0.2).

### .gitignore filtering: Use the `ignore` crate

Filter hidden files and .gitignore patterns using the `ignore` crate (from ripgrep). This gives users a clean file tree by default.

_Why not walk the tree manually_: Reimplementing .gitignore parsing is error-prone and already well-solved by the `ignore` crate.

### File viewer: Read entire file into memory

For v0.1, read the selected file fully into a `Vec<String>`. Support scrolling with j/k and page up/down.

_Why not streaming or mmap_: Premature optimization. Most source files are under 10K lines. If large file support is needed later, it can be added without changing the viewer API.

### Layout: Fixed 30/70 split

Left pane takes 30% width, right pane takes 70%. No dynamic resizing in v0.1.

_Why not configurable split_: YAGNI. Resizable panes add complexity (mouse handling, drag state). A fixed split is good enough to validate the concept.

### Pane focus: Tab toggle + Enter to enter viewer

Tab toggles focus between the two panes. Enter on a file in the tree also switches focus to the viewer. This keeps the mental model simple — one key (Tab) for toggling, plus the natural "enter a file" action.

_Why not Shift+Tab for returning to tree_: With only two panes, a toggle is simpler. Shift+Tab also has terminal compatibility issues (some terminals send different escape sequences). One key to learn = lowest cognitive load.

### Directory expand/collapse: h/l and arrow keys (vim-style spatial navigation)

l (Right arrow) expands a directory, h (Left arrow) collapses it. This follows the spatial metaphor of "right = go deeper into tree, left = go back up" used by ranger, yazi, and other vim-style file managers. Enter is reserved exclusively for opening files (switching focus to the viewer).

_Why not Enter for expand/collapse_: Using Enter for both expand/collapse (directories) and open (files) is context-dependent and overloaded. Separating the actions makes each key's behavior consistent regardless of what's selected. h/l also aligns with the vim mental model of horizontal movement = tree depth navigation.

When h is pressed on a file or collapsed directory inside an expanded parent, the parent directory is collapsed and the cursor moves to the parent entry. This matches ranger/yazi behavior where h means "go up one level". At root level (depth 0), h is a no-op since there is no parent to collapse.

### Preview: Cursor movement triggers preview (yazi-style)

Moving the selection cursor in the file tree immediately updates the file viewer. No explicit action needed to preview a file.

_Why not require Enter to preview_: Instant preview reduces friction and matches yazi's UX, which is familiar to the target audience. Enter is reserved for the intentional "I want to focus on this file" action.

## Risks / Trade-offs

- **[Large files]** → Reading entire files into memory could be slow for very large files. Mitigation: acceptable for v0.1; add size limit or lazy loading in a future version.
- **[Binary files]** → Displaying binary files as text will show garbage. Mitigation: detect binary files (check for null bytes in first 512 bytes) and show a placeholder message.
- **[Terminal compatibility]** → Some terminals may render differently. Mitigation: crossterm handles most compatibility; test on ghostty and iTerm2.
- **[Terminal too small]** → Very small terminal windows could break the layout. Mitigation: enforce a minimum terminal size (40x10) and show an error if too small.
