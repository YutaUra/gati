## Context

gati is a TUI code review tool. Currently the file viewer supports scroll-only navigation (j/k moves viewport). To support inline comments, we need a cursor line concept where j/k moves a line cursor within the file, and the viewport follows.

## Goals / Non-Goals

**Goals:**
- Cursor line in file viewer with visual highlight
- Comment CRUD on single lines and line ranges
- Inline comment display between code lines
- Export comments to clipboard as structured text
- Session-only in-memory persistence

**Non-Goals:**
- Persistent storage (file/database)
- Multi-line comment editing (single-line input only for v1)
- Comment threading or replies

## Decisions

### 1. Cursor line replaces scroll-only navigation

j/k now moves a cursor line instead of scrolling. The viewport auto-scrolls to keep the cursor visible. Ctrl-d/Ctrl-u still do half-page scroll but clamp the cursor to viewport edges.

**Alternative**: Keep scroll separate and add a separate cursor concept. Rejected because it creates confusing dual navigation modes.

### 2. Comment data model in separate `comments.rs` module

```rust
pub struct Comment {
    pub file: PathBuf,
    pub start_line: usize,  // 1-indexed
    pub end_line: usize,    // 1-indexed, same as start for single-line
    pub text: String,
}

pub struct CommentStore {
    comments: Vec<Comment>,
}
```

CommentStore lives in App, not FileViewer. This allows comments to persist across file navigation.

### 3. Line select mode with `V`

Pressing `V` enters line-select mode, j/k extends selection. Pressing `c` opens comment input for the selected range. Pressing Esc cancels selection.

### 4. Comment input as single-line text input

When `c` is pressed, a text input appears below the target line(s). Enter saves, Esc cancels. This keeps input simple. If an existing comment exists on the cursor line, it opens for editing.

### 5. Inline rendering approach

Comments are rendered as extra rows between code lines. They don't affect line numbering. Each comment block shows a bordered region with the comment text. The `total_lines()` calculation must account for comment blocks.

### 6. Export via `e` key in viewer

Pressing `e` collects all comments, formats them as structured text, and copies to clipboard using the `cli-clipboard` crate.

## Risks / Trade-offs

- **[Cursor change is behavioral]** → j/k behavior changes from scroll to cursor movement. This is intentional per the spec.
- **[Clipboard dependency]** → `cli-clipboard` adds a dependency but is the standard Rust approach. Falls back gracefully if clipboard unavailable.
- **[Comment rendering complexity]** → Inserting virtual rows between code lines adds complexity to scroll/line calculations. Mitigated by computing an expanded line map.
