## Context

The file viewer's comment system currently uses `InputMode::CommentInput` in `app.rs`. When activated, the hint bar at the bottom of the screen becomes a single-line text input showing `Comment: <text>█`. The comment target line is visible in the viewer above, but the input is physically disconnected at the bottom.

Existing comment blocks are rendered inline by `FileViewer::render_comment_block()` — a 2-row block (text + separator) inserted below the target line's end. The rendering loop in `render_to_buffer()` checks `comment_at_end_line()` after each code line.

## Goals / Non-Goals

**Goals:**
- Render comment input as an inline widget below the target line in the file viewer, replacing the bottom-bar input
- Reuse the existing comment block visual style (cyan on black) with a text cursor
- Auto-scroll the viewport to keep the inline editor visible
- Support all existing keybindings unchanged (c, Enter, Esc, Backspace, character input)

**Non-Goals:**
- Multi-line comment editing (stays single-line as today)
- Rich text or markdown preview in the editor
- Mouse-click positioning within the editor text

## Decisions

### 1. Pass editing state to FileViewer via a dedicated struct

Rather than passing the full `InputMode` enum (which lives in `app.rs`), introduce a lightweight `CommentEditState { target_line: usize, text: String }` that `App` sets on `FileViewer` before each render when in `CommentInput` mode. This keeps the data flow one-directional and avoids coupling `FileViewer` to `App`'s input mode.

**Alternative**: Pass `InputMode` directly — rejected because it couples the viewer to app-level state and requires the viewer to know about `LineSelect` and `Normal` modes.

### 2. Render inline editor in the same render loop as comment blocks

The editor widget is rendered at the same position as a saved comment would be — after the target line's end_line. The render loop already handles inserting extra rows after code lines (for existing comments). The editor simply takes priority over any existing comment on the same line (since editing replaces viewing).

**Alternative**: Render the editor as an overlay — rejected because it would require z-ordering logic and could obscure adjacent code.

### 3. Auto-scroll to keep editor visible

When `CommentEditState` is set and the target line is near the bottom of the viewport, adjust `scroll_offset` so the editor row (target_line + 1) is visible. This is a simple bounds check at the start of the File render path.

### 4. Hint bar shows mode indicator only

During comment input, the hint bar changes from showing the full input text to a simple mode indicator like `Editing comment on L{n}  Enter save  Esc cancel`. The actual text is visible inline.

## Risks / Trade-offs

- [Cursor visibility in narrow panes] Long comment text may exceed the pane width. → Truncate display with the cursor always visible at the end (horizontal scroll not needed for single-line input).
- [Scroll interaction during editing] User might scroll away from the editor. → The editor row is anchored to the target line and scrolls with it; if scrolled out of view the input is still active but not visible, which matches existing behavior.
