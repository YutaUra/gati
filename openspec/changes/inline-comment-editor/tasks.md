## 1. Data Model

- [x] 1.1 Add `CommentEditState` struct (or tuple) with `target_line: usize` and `text: String` to `FileViewer`, and a setter method `set_comment_edit` / `clear_comment_edit`
- [x] 1.2 In `App::render`, set `file_viewer.comment_edit` from `InputMode::CommentInput` state before calling `render_to_buffer`, and clear it otherwise

## 2. Inline Editor Rendering

- [x] 2.1 Add `render_comment_editor()` method to `FileViewer` that renders a single-row inline text input (cyan on black, with `█` cursor) at a given position in the buffer
- [x] 2.2 In the File render loop of `render_to_buffer()`, after rendering a code line, check if `comment_edit.target_line` matches and render the inline editor — taking priority over any existing saved comment on the same line
- [x] 2.3 Auto-scroll: if the editor target line + 1 would be below the visible viewport, adjust `scroll_offset` before rendering to keep the editor row visible

## 3. Hint Bar Update

- [x] 3.1 Change the `CommentInput` hint bar text from `Comment: {text}█` to `Editing comment on L{n}  Enter save  Esc cancel` (the actual text is now visible inline)

## 4. Edge Cases

- [x] 4.1 Handle long comment text that exceeds pane width — truncate display from the left so the cursor end is always visible
- [x] 4.2 Ensure the inline editor does not shift file line numbers (rendered rows are extra, not replacing code lines)
