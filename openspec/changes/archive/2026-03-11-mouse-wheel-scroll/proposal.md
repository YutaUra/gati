## Why

The file viewer pane currently only supports keyboard-based scrolling (j/k for line, Ctrl-d/Ctrl-u for page). Mouse capture is already enabled for pane resizing, but mouse wheel events are ignored. Users expect mouse wheel scrolling to work when mouse capture is active.

## What Changes

- Handle `MouseEventKind::ScrollUp` and `MouseEventKind::ScrollDown` in the event loop
- Scroll the file viewer content by 3 lines per wheel tick (standard scroll speed)
- Only scroll when the mouse is over the viewer pane area

## Capabilities

### New Capabilities

_(none — this extends an existing capability)_

### Modified Capabilities

- `file-viewer`: The file viewer gains mouse wheel scrolling support

## Impact

- `src/app.rs`: Add `ScrollUp`/`ScrollDown` handling in `handle_mouse()`, using the existing `scroll_up()`/`scroll_down()` methods on FileViewer
- No new crate dependencies required
