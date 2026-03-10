## Why

gati is a code review tool, but currently has no awareness of git state. Without git status markers, users cannot tell which files have been modified, added, or deleted — the fundamental starting point of any code review workflow.

## What Changes

- Add a `git_status` module that reads working tree and index status using `git2`
- Annotate file tree entries with status markers (`[M]`, `[A]`, `[D]`, `[R]`, `[?]`)
- Display directory-level change indicators (`[●]`) when descendants have changes
- Gracefully degrade when outside a git repository (no markers shown)

## Capabilities

### New Capabilities

- `git-status`: Git repository detection and working tree status computation, providing per-file status data to the file tree

### Modified Capabilities

- `file-tree`: File tree entries display git status markers next to file/directory names

## Impact

- New dependency: `git2` crate
- New module: `src/git_status.rs`
- Modified: `src/tree.rs` (TreeEntry gains optional status field)
- Modified: `src/file_tree.rs` (rendering includes status markers with colors)
- Modified: `src/app.rs` (git status computed on startup, passed to tree)
