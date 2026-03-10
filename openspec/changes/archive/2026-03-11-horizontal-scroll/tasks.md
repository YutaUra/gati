## 1. State

- [x] 1.1 Add `h_scroll: usize` field to FileViewer (default 0)
- [x] 1.2 Add `H_SCROLL_AMOUNT: usize = 4` constant
- [x] 1.3 Reset `h_scroll = 0` in `load_file()`

## 2. Rendering

- [x] 2.1 Implement `skip_chars_in_spans(spans: Vec<Span>, skip: usize) -> Vec<Span>` helper that drops the first `skip` characters across spans while preserving styles
- [x] 2.2 Apply `skip_chars_in_spans` to highlighted code spans (after gutter) in the File rendering branch of `render_to_buffer()`

## 3. Keyboard

- [x] 3.1 Add `scroll_left()` and `scroll_right()` methods to FileViewer
- [x] 3.2 Handle `h` / `Left` arrow → `scroll_left()` and `l` / `Right` arrow → `scroll_right()` in viewer's `handle_event()`

## 4. Mouse

- [x] 4.1 Handle `MouseEventKind::ScrollLeft` / `ScrollRight` in `handle_mouse()` for viewer pane
- [x] 4.2 Handle Shift+ScrollUp (left) / Shift+ScrollDown (right) in `handle_mouse()` for viewer pane

## 5. Tests

- [x] 5.1 Test `skip_chars_in_spans` with single span, multi-span, and skip exceeding total length
- [x] 5.2 Test `h_scroll` resets to 0 on `load_file()`
- [x] 5.3 Test `scroll_left` and `scroll_right` adjust `h_scroll` correctly (including floor at 0)
- [x] 5.4 Test mouse horizontal scroll over viewer pane changes `h_scroll`
