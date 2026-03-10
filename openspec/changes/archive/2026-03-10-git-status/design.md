## Context

gati currently displays a file tree with no awareness of version control state. The tree renders file/directory names with expand/collapse icons but provides no indication of which files have been modified, added, or deleted. This is the first step toward making gati a code review tool — before showing diffs, users need to know *where* changes exist.

## Goals / Non-Goals

**Goals:**

- Display per-file git status markers in the file tree
- Show directory-level change indicators for directories containing changed files
- Graceful degradation outside git repositories
- Compute status once on startup (no file watching)

**Non-Goals:**

- Diff computation (separate feature: diff-view)
- Staging/unstaging files (gati is read-only for git)
- File watching or auto-refresh on external changes
- Submodule status

## Decisions

### 1. Use `git2` crate for git operations

**Decision**: Use `git2` (libgit2 bindings) to read repository status.

**Why not shell out to `git status`?** Parsing `git status --porcelain` output would work but is fragile — depends on git being installed, introduces subprocess overhead, and requires output parsing. `git2` provides typed APIs and is the standard approach in Rust TUI tools (used by gitui, lazygit-rs).

**Why not `gix`?** `gix` is a pure-Rust git implementation and avoids the libgit2 C dependency. However, `git2` has a more stable API, better documentation, and wider adoption. `gix` could replace `git2` later if the C dependency becomes problematic.

### 2. Status data as a `HashMap<PathBuf, FileStatus>`

**Decision**: Compute a flat `HashMap<PathBuf, FileStatus>` mapping absolute file paths to their status. Pass this to the file tree for rendering.

This keeps git logic decoupled from tree rendering. The tree doesn't need to know about git internals — it just looks up paths in the map.

### 3. Directory indicators via propagation

**Decision**: After computing file statuses, propagate a generic "has changes" flag up to ancestor directories. Directories display `[●]` rather than trying to aggregate specific statuses.

**Why not aggregate statuses (e.g., show `[M]` on directory)?** Aggregation is ambiguous — a directory with both added and modified files would need complex priority rules. A simple `[●]` clearly communicates "something changed in here" without information overload.

### 4. Status computation timing

**Decision**: Compute git status once at application startup. Store the result and use it for the lifetime of the session.

Refresh on demand (e.g., when user presses a key) is a future enhancement. For the initial implementation, a single computation at startup is sufficient since gati is primarily a read-only review tool opened after changes are made.

### 5. Status enum design

```
enum FileStatus {
    Modified,    // [M] — tracked file with changes (staged or unstaged)
    Added,       // [A] — new file staged for commit
    Deleted,     // [D] — tracked file deleted from working tree
    Renamed,     // [R] — file renamed
    Untracked,   // [?] — new file not yet staged
}
```

Working tree and index statuses are merged into a single status per file. When a file has both staged and unstaged changes, the working tree status takes priority for display purposes (this is what the user cares about during review).

## Risks / Trade-offs

- **[Risk] libgit2 C dependency increases build complexity** → Acceptable trade-off for API quality. If problematic, can migrate to `gix` later.
- **[Risk] Large repositories may have slow status computation** → git2's `Repository::statuses()` is efficient (same underlying implementation as `git status`). Not a concern for typical projects.
- **[Trade-off] Single status per file loses staged vs. unstaged distinction** → Simplifies the UI. The distinction can be added later if users need it.
- **[Trade-off] No auto-refresh** → Acceptable for initial implementation. Users restart gati to see updated status.
