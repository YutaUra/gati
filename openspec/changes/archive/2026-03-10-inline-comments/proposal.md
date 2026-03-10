## Why

Code review tools need inline commenting to allow reviewers to leave contextual feedback on specific lines. Without this, review notes exist separately from the code they reference, making it harder to communicate precise feedback.

## What Changes

- Add cursor line tracking in the file viewer (prerequisite for line-specific actions)
- Add inline comment creation on single lines (`c` key) and line ranges (`V` select + `c`)
- Display comments inline between code lines in visually distinct blocks
- Support editing and deleting existing comments
- Export all comments as structured plain text to clipboard
- Session-only persistence (comments stored in memory, lost on exit)

## Capabilities

### New Capabilities

_(none — this builds on the existing `inline-comments` spec)_

### Modified Capabilities

- `inline-comments`: Implementing the full spec — cursor line, comment CRUD, inline display, export to clipboard
- `file-viewer`: Adding cursor line tracking and comment input mode

## Impact

- `src/file_viewer.rs`: Major changes — cursor line, comment input, inline comment rendering
- `src/app.rs`: Comment store, export action, hint bar updates
- `src/components.rs`: New actions for comment workflow
- New module `src/comments.rs`: Comment data model and storage
