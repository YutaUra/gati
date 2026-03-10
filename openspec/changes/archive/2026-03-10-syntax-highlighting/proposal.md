## Why

The file viewer currently displays code as plain white text, making it difficult to scan and understand code structure at a glance. Syntax highlighting is a baseline expectation for any code review tool — without it, gati feels incomplete compared to bat, delta, or any modern editor. Adding syntax highlighting directly improves the core "understanding code" experience.

## What Changes

- Add syntax highlighting to the file viewer using the `syntect` crate
- Highlight file contents based on file extension / first-line detection
- Fall back to plain text rendering when no syntax definition matches
- Preserve existing line number gutter behavior (line numbers remain unhighlighted)

## Capabilities

### New Capabilities

(none — this modifies an existing capability)

### Modified Capabilities

- `file-viewer`: Add syntax-highlighted rendering of file contents. The viewer currently displays plain text; it will now render with language-aware colors using a bundled theme.

## Impact

- **Dependencies**: Add `syntect` crate
- **Code**: `src/file_viewer.rs` — rendering logic changes from plain `Span::raw` to styled spans per token
- **Performance**: syntect parses on file load; acceptable for typical source files (<10K lines). No async needed for v0.2.
- **Theme**: Bundle a single dark terminal theme (e.g., base16-eighties or similar). No user-configurable themes in this change.
