## Context

The TUI has a two-pane layout (tree + viewer) with mouse-draggable resizing. The tree width is stored as `tree_width_percent: u16` in App. The `draw()` function uses `Constraint::Percentage` for layout splitting, and `handle_mouse()` manages drag resize with `clamp_tree_percent()`. We need to add a mode where the tree pane is completely hidden.

## Goals / Non-Goals

**Goals:**
- Keyboard toggle (Ctrl+Shift+B) for focus mode
- Drag-to-collapse: dragging border below minimum enters focus mode
- Drag-to-restore: dragging from collapsed state exits focus mode
- Remember previous tree width for keyboard toggle restoration

**Non-Goals:**
- Persisting focus mode across sessions
- Hiding the viewer pane (only tree can be hidden)
- Animations or transitions

## Decisions

### 1. Add `focus_mode: bool` and `saved_tree_width_percent: u16` to App

When focus mode activates, save the current `tree_width_percent` to `saved_tree_width_percent` and set `focus_mode = true`. When deactivating via keyboard, restore `tree_width_percent` from the saved value.

Alternative: use `Option<u16>` where `Some(saved)` means focus mode is active. Rejected because a separate bool is clearer and the saved width is always meaningful (defaults to 30).

### 2. In `draw()`, use `Constraint::Length(0)` for tree pane in focus mode

When `focus_mode` is true, render the layout with `Constraint::Length(0)` for the tree and `Constraint::Percentage(100)` for the viewer. The tree pane gets zero width so nothing is rendered. Set `border_column = 0` so mouse handler can detect drags from the left edge.

Alternative: skip rendering the tree widget entirely. Rejected because using zero-width constraint is simpler and the layout logic remains uniform.

### 3. Drag-to-collapse: detect when mouse goes below minimum

In `handle_mouse()` during `MouseEventKind::Drag`, if `resizing` is true and the mouse x position is less than `min_cols` (the minimum pane width), enter focus mode instead of clamping. Save the current `tree_width_percent` before collapsing.

### 4. Drag-to-restore: detect drag from left edge in focus mode

In `handle_mouse()`, when `focus_mode` is true:
- On `MouseEventKind::Down(Left)` at column 0 or 1, set `resizing = true`
- On `MouseEventKind::Drag(Left)` while `resizing`, if mouse x > `min_cols`, exit focus mode and set `tree_width_percent` from `clamp_tree_percent(mouse.column, terminal_width)`

### 5. Ctrl+Shift+B toggles focus mode

Handle `KeyCode::Char('B')` (uppercase B, meaning Shift is held) with `KeyModifiers::CONTROL` in the event loop. Toggle `focus_mode`. When entering, save tree width. When exiting, restore saved tree width. This key is handled before focus-specific dispatch so it works in both panes.

Note: crossterm reports Ctrl+Shift+B as `KeyCode::Char('B')` with `CONTROL` modifier (uppercase because Shift). No need to check SHIFT separately.

## Risks / Trade-offs

- **Ctrl+Shift+B may conflict with terminal emulators**: Some terminals intercept Ctrl+Shift+B (e.g., tmux). This is acceptable since the feature is also accessible via mouse drag. → If needed, the keybinding can be changed later.
- **Focus mode with tree focused**: If tree is focused when entering focus mode, focus must move to viewer since the tree is hidden. → Handled in toggle logic.
