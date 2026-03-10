## Why

When reviewing files with long lines or complex diffs, the file tree pane takes up screen space that would be better used by the viewer. Users need a way to maximize the viewer to full width for focused reading, and return to the two-pane layout when they need to navigate again.

## What Changes

- Add focus mode: hide the file tree pane so the viewer occupies the full terminal width
- Toggle focus mode with Ctrl+Shift+B keyboard shortcut
- Automatically enter focus mode when mouse-dragging the pane border below the minimum width threshold
- Automatically exit focus mode when mouse-dragging the border outward from the collapsed state
- Remember the previous tree width so it can be restored when exiting focus mode

## Capabilities

### New Capabilities

_(none — this extends an existing capability)_

### Modified Capabilities

- `tui-layout`: The two-pane layout gains a focus mode where the tree pane is hidden and the viewer fills the screen, toggled by keyboard shortcut or by dragging the border past the minimum width

## Impact

- `src/app.rs`: Add `focus_mode: bool` and `saved_tree_width_percent: u16` to App state, modify `draw()` to conditionally hide tree pane, modify `handle_mouse()` to trigger focus mode transitions, add Ctrl+Shift+B handler in event loop
- No new crate dependencies required
