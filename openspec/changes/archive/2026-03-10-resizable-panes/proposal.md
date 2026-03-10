## Why

The file tree and file viewer panes are currently fixed at a 30%/70% split. When reviewing files with long lines, users need more viewer space; when navigating deep directory structures, users need more tree space. Mouse-draggable pane resizing gives users control over their layout without keyboard shortcuts.

## What Changes

- Add mouse-draggable pane border: click and drag the vertical border between file tree and viewer to resize
- Enforce minimum pane width: each pane must be at least max(10% of terminal width, 10 columns) to prevent unusable layouts
- Enforce maximum tree pane width: tree pane can expand up to 70% of terminal width
- Store pane ratio in App state so it persists during the session
- Enable crossterm mouse capture in the terminal

## Capabilities

### New Capabilities

_(none — this extends an existing capability)_

### Modified Capabilities

- `tui-layout`: The two-pane layout becomes resizable via mouse drag on the pane border, with min/max constraints replacing the fixed 30%/70% split

## Impact

- `src/app.rs`: Store pane ratio, enable mouse events, handle mouse drag events in event loop, use dynamic constraints in `draw()`
- `crossterm` dependency: Enable mouse capture (`EnableMouseCapture`/`DisableMouseCapture`)
- No new crate dependencies required
