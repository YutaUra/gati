## Context

The TUI uses a fixed 30%/70% layout split via `Constraint::Percentage(30)` and `Constraint::Percentage(70)` in `draw()`. crossterm already supports mouse events but they are not currently enabled. ratatui layouts accept dynamic constraints, so switching from fixed percentages to a stored ratio is straightforward.

## Goals / Non-Goals

**Goals:**
- Mouse drag on pane border to resize
- Minimum pane width: `max(10% of terminal width, 10 columns)`
- Maximum tree pane width: 70% of terminal width
- Default ratio remains 30% tree / 70% viewer

**Non-Goals:**
- Keyboard-based resize (can be added later)
- Persistent ratio across sessions (file-based config)
- Vertical splitting or additional panes

## Decisions

### 1. Store ratio as `tree_width_percent: u16` in App

Store the tree pane width as an integer percentage (0–100) in App state. Default is 30. The viewer gets the remainder. This is simpler than storing pixel counts because ratatui's `Constraint::Percentage` works directly with it, and it adapts naturally to terminal resizes.

Alternative: store absolute column count. Rejected because it requires recalculation on terminal resize and doesn't map cleanly to ratatui constraints.

### 2. Detect drag on the border column

The border between panes is at `tree_area.right()` (or equivalently `viewer_area.left() - 1`). On `MouseEventKind::Drag(MouseButton::Left)`, check if the drag started near the border (within ±1 column). Update `tree_width_percent` based on the mouse x position relative to terminal width.

Track drag state with a boolean `resizing: bool` in App. Set to true on `MouseDown` near the border, false on `MouseUp`.

### 3. Clamp to min/max bounds

On each resize update:
```
min_cols = max(terminal_width * 10 / 100, 10)
max_tree_cols = terminal_width * 70 / 100
tree_cols = clamp(mouse_x, min_cols, max_tree_cols)
tree_width_percent = tree_cols * 100 / terminal_width
```
The viewer minimum is implicitly enforced: if tree is at max 70%, viewer gets 30% which satisfies the same minimum constraint.

### 4. Enable mouse capture

Add `EnableMouseCapture` to `init_terminal()` and `DisableMouseCapture` to `restore_terminal()`. Handle `Event::Mouse` in the event loop alongside `Event::Key`.

## Risks / Trade-offs

- **Mouse events may interfere with terminal selection**: Users who want to select text with the mouse will find it captured by the app. This is standard for TUI apps with mouse support (e.g., lazygit, btop). → No mitigation needed; this is expected behavior.
- **Drag precision on narrow terminals**: On very small terminals, 1-column drag = large percentage change. → The min/max clamp prevents unusable layouts.
