## 1. App state

- [x] 1.1 Add `focus_mode: bool` field to App (default false)
- [x] 1.2 Add `saved_tree_width_percent: u16` field to App (default 30)

## 2. Draw layout in focus mode

- [x] 2.1 In `draw()`, when `focus_mode` is true, use `Constraint::Length(0)` for tree and `Constraint::Percentage(100)` for viewer
- [x] 2.2 Set `border_column = 0` in focus mode so drag-to-restore can detect left edge clicks

## 3. Keyboard toggle

- [x] 3.1 Handle Ctrl+Shift+B in event loop: toggle `focus_mode`, save/restore `tree_width_percent`, force focus to viewer when entering focus mode

## 4. Drag-to-collapse

- [x] 4.1 In `handle_mouse()` during Drag, when mouse x is below `min_cols`, enter focus mode (save tree width, set `focus_mode = true`)

## 5. Drag-to-restore

- [x] 5.1 In `handle_mouse()` during Down, when `focus_mode` is true and mouse column ≤ 1, set `resizing = true`
- [x] 5.2 In `handle_mouse()` during Drag, when `focus_mode` is true and `resizing` and mouse x > `min_cols`, exit focus mode and set `tree_width_percent` from mouse position

## 6. Hint bar update

- [x] 6.1 Update hint bar to show Ctrl+Shift+B toggle hint; in focus mode show "Ctrl+Shift+B restore tree"

## 7. Tests

- [x] 7.1 Test default focus_mode is false and saved_tree_width_percent is 30
- [x] 7.2 Test toggle focus mode: entering saves tree width and sets focus_mode true
- [x] 7.3 Test toggle focus mode: exiting restores saved tree width and sets focus_mode false
- [x] 7.4 Test drag below minimum enters focus mode
- [x] 7.5 Test drag from left edge in focus mode exits focus mode
