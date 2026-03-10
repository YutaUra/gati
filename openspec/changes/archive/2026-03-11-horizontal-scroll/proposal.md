## Why

The file viewer currently truncates long lines at the viewport width. When reviewing files with long lines (wide tables, minified code, long strings), users cannot see the truncated content. Horizontal scrolling lets users pan the viewport left and right to read full lines.

## What Changes

- Add horizontal scroll offset (`h_scroll: usize`) to FileViewer
- Support keyboard horizontal scroll: `H` (left) and `L` (right) in viewer, and left/right arrow keys
- Support mouse horizontal scroll: Shift+ScrollUp/ScrollDown (standard horizontal scroll gesture in many terminals) for the viewer pane
- Apply horizontal offset when rendering file content: skip the first `h_scroll` characters of each line
- Reset horizontal scroll on file change

## Capabilities

### New Capabilities

_(none — this extends an existing capability)_

### Modified Capabilities

- `file-viewer`: The file viewer gains horizontal scrolling via keyboard (H/L, arrow keys) and mouse (Shift+wheel)

## Impact

- `src/file_viewer.rs`: Add `h_scroll: usize` field, modify rendering to apply horizontal offset, add H/L/arrow key handlers, reset on file load
- `src/app.rs`: Handle `MouseEventKind::ScrollLeft`/`ScrollRight` or Shift+ScrollUp/ScrollDown for horizontal scroll in `handle_mouse()`
- No new crate dependencies required
