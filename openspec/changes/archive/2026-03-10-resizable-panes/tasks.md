## 1. Mouse capture

- [x] 1.1 Enable `EnableMouseCapture` in `init_terminal()` and `DisableMouseCapture` in `restore_terminal()` and panic hook
- [x] 1.2 Handle `Event::Mouse` in event loop (pass-through for now, no crash on mouse events)

## 2. App state for pane ratio

- [x] 2.1 Add `tree_width_percent: u16` field to App (default 30)
- [x] 2.2 Add `resizing: bool` field to App for drag state tracking
- [x] 2.3 Replace fixed `Constraint::Percentage(30/70)` in `draw()` with dynamic values from `tree_width_percent`

## 3. Mouse drag resize

- [x] 3.1 On `MouseEventKind::Down(Left)`, set `resizing = true` if mouse x is within ±1 of the pane border column
- [x] 3.2 On `MouseEventKind::Drag(Left)` while `resizing`, compute new `tree_width_percent` from mouse x position
- [x] 3.3 Clamp `tree_width_percent` to min/max bounds: min = max(10%, 10 columns), max = 70%
- [x] 3.4 On `MouseEventKind::Up(Left)`, set `resizing = false`

## 4. Visual feedback

- [x] 4.1 Change border style or cursor when hovering over the draggable border area (optional polish)

## 5. Tests

- [x] 5.1 Test default tree_width_percent is 30
- [x] 5.2 Test clamp logic: values below minimum are clamped up, values above maximum are clamped down
- [x] 5.3 Test resize state: resizing flag set/cleared correctly
