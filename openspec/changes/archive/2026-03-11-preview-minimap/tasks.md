## 1. Minimap Data Model

- [x] 1.1 Add minimap state fields to `FileViewer` struct: `minimap_rect` (stored for mouse hit-testing), and a helper to compute line-to-row mapping
- [x] 1.2 Implement `compute_minimap_markers()` method that produces a `Vec<MinimapMarker>` from line diff, unified diff, and comments data — each entry maps a minimap row to its marker color (green/yellow/red/cyan) or none

## 2. Minimap Rendering

- [x] 2.1 Split the viewer's render area into `[content_area, minimap_area]` within `render_to_buffer()` — minimap is 2 columns wide on the right edge, hidden when inner width < 30
- [x] 2.2 Implement `render_minimap()` method that draws the minimap: background fill, viewport indicator (bright region), and colored diff/comment markers using block characters
- [x] 2.3 Integrate minimap rendering into both normal mode and diff mode render paths

## 3. Click-to-Scroll Navigation

- [x] 3.1 Store the minimap `Rect` after rendering for mouse hit-testing in `App`
- [x] 3.2 Add minimap click detection in `App::handle_mouse()` — when a click lands in the minimap rect, call a new `FileViewer::scroll_to_minimap_row(row)` method
- [x] 3.3 Implement `scroll_to_minimap_row()` that translates a minimap row to a file line and centers it in the viewport

## 4. Placeholder State Handling

- [x] 4.1 Skip minimap rendering when `ViewerContent` is `Placeholder`, `Binary`, `Empty`, or `Error` — only render for `File` content
