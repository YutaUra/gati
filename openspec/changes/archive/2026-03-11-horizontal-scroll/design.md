## Context

The file viewer renders lines with syntax highlighting via `Highlighter::highlight_line()`, which returns `Vec<Span>`. Each line is rendered as `Line::from(spans)` and written to the buffer with `buf.set_line()`. The gutter (line numbers, diff markers) is prepended as spans. Vertical scroll uses `scroll_offset` to skip lines. We need an analogous horizontal offset for character-level panning.

## Goals / Non-Goals

**Goals:**
- Horizontal scroll with keyboard (H/L, arrow keys) and mouse (Shift+wheel)
- Gutter stays fixed — only the code content scrolls horizontally
- Reset h_scroll on file load
- Scroll amount: 4 columns per tick (comfortable for code reading)

**Non-Goals:**
- Word-wrap mode (alternative to horizontal scroll)
- Horizontal scroll for diff mode (can be added later)
- Horizontal scroll bar indicator

## Decisions

### 1. Store `h_scroll: usize` in FileViewer

Simple character offset. Default 0, reset on `load_file()`. The offset is in characters, not display columns (tabs are not expanded, matching the existing line storage).

### 2. Apply h_scroll during rendering by slicing highlighted spans

After `highlight_line()` returns spans, skip the first `h_scroll` characters worth of span content. This preserves syntax highlighting across the scroll boundary. Implement a helper `skip_chars_in_spans(spans, skip)` that walks through spans, dropping characters from the front.

Alternative: slice the raw string before highlighting. Rejected because it would break syntax highlighting state (partial tokens would be mis-highlighted).

### 3. Gutter is not affected by h_scroll

Line numbers and diff markers are prepended before the highlighted content. Only the code spans after the gutter are subject to horizontal offset. This keeps navigation context visible.

### 4. Keyboard: H/L and Left/Right arrows

In the viewer's `handle_event`:
- `H` or `Left`: decrease `h_scroll` by `H_SCROLL_AMOUNT` (min 0)
- `L` or `Right`: increase `h_scroll` by `H_SCROLL_AMOUNT`

Note: `H` and `L` are currently unused in the viewer. Arrow keys (Left/Right) are also unused.

### 5. Mouse: Shift+ScrollUp/Down for horizontal scroll

crossterm reports `MouseEventKind::ScrollLeft` / `ScrollRight` for native horizontal scroll (supported by some terminals). Also handle Shift+ScrollUp as left and Shift+ScrollDown as right (common convention). Both handled in `handle_mouse()` in app.rs.

### 6. Constant: H_SCROLL_AMOUNT = 4

4 columns per keyboard press or mouse tick. Balances speed and precision.

## Risks / Trade-offs

- **Character vs display width**: CJK characters and tabs occupy multiple display columns but are counted as 1 character. The offset may not align perfectly with visual columns for these cases. → Acceptable for v1; can be improved with unicode-width crate later.
- **Highlight state**: Highlighting entire lines then slicing means we do full highlighting work even for scrolled-away content. → Performance impact is negligible since we already highlight visible lines.
