## Context

Mouse capture is already enabled (EnableMouseCapture in init_terminal). The `handle_mouse()` function handles Down/Drag/Up for pane resizing. FileViewer already has `scroll_up(amount)` and `scroll_down(amount)` methods. The viewer pane area is tracked via `border_column` (tree's right edge) — columns to the right of the border are the viewer.

## Goals / Non-Goals

**Goals:**
- Mouse wheel scrolls the file viewer content
- Only scroll when mouse is over the viewer pane
- Standard scroll speed (3 lines per tick)

**Non-Goals:**
- Mouse wheel scrolling in the file tree (can be added later)
- Configurable scroll speed
- Smooth/pixel-level scrolling

## Decisions

### 1. Handle ScrollUp/ScrollDown in handle_mouse()

Add `MouseEventKind::ScrollUp` and `MouseEventKind::ScrollDown` arms to the existing match in `handle_mouse()`. These need access to the file viewer, so `handle_mouse` will need a reference to the viewer (or the full App).

The function already takes `&mut App`, so we can call `app.file_viewer.scroll_up(3)` / `app.file_viewer.scroll_down(3)` directly.

### 2. Hit-test: only scroll when mouse is over viewer pane

Check `mouse.column > app.border_column` (or `mouse.column >= app.border_column` in focus mode since border_column is 0). In focus mode, the viewer fills the entire width so all scroll events should apply.

### 3. Scroll amount: 3 lines per tick

Use a constant `MOUSE_SCROLL_LINES = 3`. This matches the standard scroll speed in most terminals and TUI applications.

## Risks / Trade-offs

- **Scroll conflicts with resize drag**: ScrollUp/ScrollDown are separate event kinds from Drag, so no conflict. If the user is dragging to resize and accidentally scrolls, the scroll will be ignored because the column check will fail (border area is not the viewer).
