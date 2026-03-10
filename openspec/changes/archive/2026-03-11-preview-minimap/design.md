## Context

gati's preview pane (`FileViewer`) renders file contents with syntax highlighting, line numbers, and diff markers. The rendering is done via direct `Buffer` writes in `render_to_buffer()`. The layout is a simple horizontal split between tree and viewer panes, with no sub-columns within the viewer.

When reviewing large files, users have no way to see where they are relative to the whole file or where changes are concentrated. VSCode and other editors solve this with a minimap—a narrow vertical strip showing a compressed view of the file.

## Goals / Non-Goals

**Goals:**
- Add a minimap column on the right edge of the preview pane
- Show viewport position indicator (which portion of the file is currently visible)
- Show diff markers (added/modified/removed lines) at their relative positions
- Show comment indicators at their relative positions
- Support click-to-scroll navigation on the minimap
- Work in both normal and diff view modes

**Non-Goals:**
- Rendering actual code content in the minimap (too narrow at 2-3 columns; markers are sufficient)
- Minimap in the file tree pane
- Configurable minimap width or toggle keybinding (keep it simple for v1)
- Drag-to-scroll on the viewport indicator

## Decisions

### 1. Minimap as internal sub-layout within FileViewer

**Decision**: Split the viewer's `Rect` internally into `[content_area, minimap_area]` within `render_to_buffer()`, rather than adding a new top-level pane in `App::draw()`.

**Rationale**: The minimap is tightly coupled to the viewer's scroll state, line count, diff data, and comments. Keeping it within `FileViewer` avoids exposing internal state to `App` and keeps the component self-contained. The alternative—adding a third horizontal pane in `App`—would require passing viewer state up and break the current component encapsulation.

### 2. Fixed 2-column width

**Decision**: The minimap occupies exactly 2 terminal columns on the right edge.

**Rationale**: 1 column is too narrow for distinguishable markers. 3+ columns wastes horizontal space that is valuable in terminal UIs. 2 columns provide enough space for a viewport indicator block and colored diff/comment markers while minimizing content area reduction.

### 3. Block-based rendering (not text-based)

**Decision**: Use block characters (`▐`, `█`, `┃`, or half-block `▀▄`) and background colors to represent file regions, rather than rendering miniaturized text.

**Rationale**: At 2 columns wide, text is unreadable. Block characters with colors effectively communicate position and change density. This approach is also simpler to implement and performs well since it's just colored cells.

### 4. Proportional mapping from file lines to minimap rows

**Decision**: Map file lines to minimap rows proportionally. Each minimap row represents `ceil(total_lines / minimap_height)` lines. The viewport indicator highlights the rows corresponding to the currently visible range.

**Rationale**: This ensures the minimap always fills its height and provides consistent spatial mapping regardless of file length. For short files (fewer lines than minimap height), each row maps to one line with empty rows below.

### 5. Mouse click handling delegated from App to FileViewer

**Decision**: `App` detects clicks in the minimap area (using the viewer's stored `minimap_rect`) and forwards them to `FileViewer` via a new method like `handle_minimap_click(row)`. The viewer translates the minimap row to a file line and adjusts `scroll_offset`.

**Rationale**: `App` already owns mouse event routing and knows pane boundaries. Adding a minimap hit-test is consistent with existing click handling for the border drag and tree pane. The viewer owns the line-mapping logic since it knows `total_lines` and `minimap_height`.

### 6. Color scheme for markers

**Decision**:
- Viewport indicator: bright block on a dim background (e.g., `DarkGray` background for minimap, `Gray` or `White` for viewport region)
- Added lines: `Green` marker
- Modified lines: `Yellow` marker
- Removed lines: `Red` marker
- Comment indicators: `Cyan` marker

**Rationale**: Reuses the existing color conventions from diff markers in the gutter (`Green` for added, `Yellow` for modified). `Cyan` for comments differentiates them from diff markers. The viewport indicator uses brightness contrast rather than color to avoid confusion with diff markers.

## Risks / Trade-offs

- **[Narrow terminals]** → The 2-column minimap reduces content width. On very narrow terminals (< 40 cols), this may be noticeable. Mitigation: The minimap can be conditionally hidden when viewer width is below a threshold (e.g., < 30 columns).
- **[Performance on huge files]** → Computing marker positions for files with thousands of lines requires iterating diff data each frame. Mitigation: The computation is O(n) where n = total lines, and only produces a small array of minimap-height booleans. This is negligible compared to syntax highlighting costs.
- **[Diff mode complexity]** → In diff mode, the line count differs from the file's actual line count (filtered hunk headers, added/removed lines). Mitigation: Use the diff mode's visible line count for mapping, not the raw file line count.
