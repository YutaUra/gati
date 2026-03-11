## Why

The current comment input uses the bottom hint bar as a text input area. This is disconnected from the code context — users type at the bottom of the screen while the comment target line is elsewhere. Moving the input field inline (directly below the target line) provides immediate visual context, matching the UX pattern of GitHub PR review comments and IDE inline editors.

## What Changes

- Replace the bottom-bar comment input with an inline text input widget rendered directly below the comment target line (or range) in the file viewer
- The inline editor appears in the same visual style as existing comment blocks (cyan/black) with a text cursor
- Existing keybindings (c to start, Enter to save, Esc to cancel, Backspace to delete) remain unchanged
- The hint bar shows a simplified prompt during comment input instead of the full input text

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `inline-comments`: The comment input area moves from the hint bar to an inline editor widget rendered below the target line in the file viewer

## Impact

- `src/file_viewer.rs`: Add rendering logic for an inline text input widget below the target line during comment input mode
- `src/app.rs`: Pass input mode state to `FileViewer` for rendering; remove comment text from hint bar display; adjust scroll to keep the editor visible
- No new dependencies; no API changes; no breaking changes to comment data model
