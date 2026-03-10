## 1. Mouse wheel scroll handling

- [x] 1.1 Add `MOUSE_SCROLL_LINES` constant (value: 3)
- [x] 1.2 Handle `MouseEventKind::ScrollDown` in `handle_mouse()`: if mouse column is over viewer pane, call `app.file_viewer.scroll_down(MOUSE_SCROLL_LINES)`
- [x] 1.3 Handle `MouseEventKind::ScrollUp` in `handle_mouse()`: if mouse column is over viewer pane, call `app.file_viewer.scroll_up(MOUSE_SCROLL_LINES)`
- [x] 1.4 Make `file_viewer` field accessible from `handle_mouse` (it already is via `&mut App`)

## 2. Tests

- [x] 2.1 Test scroll down over viewer pane moves scroll offset
- [x] 2.2 Test scroll up over viewer pane moves scroll offset
- [x] 2.3 Test scroll over tree pane does not affect viewer scroll offset
