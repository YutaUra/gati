## 1. Cursor line in file viewer

- [x] 1.1 Add `cursor_line` field to FileViewer (0-indexed line position within the file)
- [x] 1.2 Change j/k from scroll to cursor movement, auto-scroll viewport to keep cursor visible
- [x] 1.3 Clamp cursor to viewport edges on Ctrl-d/Ctrl-u half-page scroll
- [x] 1.4 Render cursor line with subtle background highlight

## 2. Comment data model

- [x] 2.1 Create `src/comments.rs` with Comment struct and CommentStore
- [x] 2.2 Add CommentStore to App, pass comments to FileViewer for rendering

## 3. Comment creation

- [x] 3.1 Handle `c` key to open comment input on cursor line (or edit existing)
- [x] 3.2 Add comment input mode with text entry, Enter to save, Esc to cancel
- [x] 3.3 Add line-select mode with `V` key, j/k to extend selection, `c` to comment range, Esc to cancel

## 4. Comment display

- [x] 4.1 Render inline comment blocks between code lines (bordered, visually distinct)
- [x] 4.2 Adjust scroll/total_lines calculations to account for comment blocks

## 5. Comment editing and deletion

- [x] 5.1 `c` on a line with existing comment opens it for editing
- [x] 5.2 Add delete comment keybinding (`x` key on commented line)

## 6. Export

- [x] 6.1 Add `cli-clipboard` dependency
- [x] 6.2 Implement export command (`e` key) that formats and copies all comments to clipboard

## 7. Hint bar and integration

- [x] 7.1 Update hint bar for viewer: add `c comment`, `V select`, `e export`
- [x] 7.2 Update hint bar for comment input mode: `Enter save  Esc cancel`
- [x] 7.3 Update hint bar for line-select mode: `j/k extend  c comment  Esc cancel`
