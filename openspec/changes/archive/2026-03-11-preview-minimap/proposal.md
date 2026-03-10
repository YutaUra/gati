## Why

The preview pane currently lacks a way to understand where you are within a file and where changes exist at a glance. When reviewing large files, users must scroll through the entire file to find modified regions. A minimap—similar to the one in VSCode—provides spatial orientation and quick navigation to changed areas, significantly improving code review efficiency.

## What Changes

- Add a minimap column on the right edge of the preview pane that shows:
  - A viewport indicator representing which portion of the file is currently visible
  - Colored markers for changed/added/removed lines (from git diff data)
  - Comment indicators for lines with inline comments
- The minimap is clickable: clicking a position scrolls the preview to that location
- The minimap works in both normal and diff view modes
- The minimap width is fixed (a narrow column, ~2-3 terminal columns)

## Capabilities

### New Capabilities
- `preview-minimap`: A vertical minimap element on the right edge of the preview pane showing viewport position, diff markers, and comment indicators with click-to-scroll navigation

### Modified Capabilities
- `file-viewer`: The preview pane layout changes to accommodate the minimap column on the right side, reducing the content area width slightly

## Impact

- `src/file_viewer.rs`: Layout changes to split the preview area into content + minimap columns; new rendering logic for the minimap
- `src/app.rs`: Mouse click handling for minimap area (click-to-scroll)
- No new dependencies required—uses ratatui's existing `Buffer` API for direct rendering
- No breaking changes to keybindings or existing behavior
