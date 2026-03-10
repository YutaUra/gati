# Diff View — Design

## Architecture

### New module: `src/diff.rs`

Provides diff computation using `git2`:
- `LineDiff` struct: stores per-line change status (Added, Modified, Unchanged) for gutter markers
- `UnifiedDiff` struct: stores parsed unified diff lines (Added, Removed, Context, HunkHeader) for diff mode
- `compute_line_diff(repo_path, file_path) -> Option<LineDiff>`: compares working tree vs HEAD
- `compute_unified_diff(repo_path, file_path) -> Option<UnifiedDiff>`: generates unified diff

### Modified module: `src/file_viewer.rs`

- Add `diff_mode: bool` toggle state
- Add `line_diff: Option<LineDiff>` for gutter markers in normal mode
- Add `unified_diff: Option<UnifiedDiff>` for diff mode content
- Handle `d` key to toggle between normal and diff mode
- Render gutter markers (`▎` in yellow/green) when `line_diff` is present
- Render unified diff lines with colored prefixes when in diff mode

### Modified module: `src/app.rs`

- Store repository path for diff computation
- When a file is loaded, compute both `LineDiff` and `UnifiedDiff`
- Pass diff data to file viewer
- Update hint bar to include `d diff` when inside a git repo

## Key Decisions

### Decision: Use git2 blob diff, not shell `git diff`

**Chosen**: Use `git2::Diff` API to compute diffs programmatically.
**Why not shell**: Avoids spawning processes, keeps dependency on already-used git2 crate, no PATH dependency.

### Decision: Compute diff on file load, not on toggle

**Chosen**: Compute both LineDiff and UnifiedDiff when a file is loaded/selected.
**Why not on toggle**: Avoids noticeable delay when pressing `d`. Pre-computation is fast for single files.

### Decision: Store repo workdir path in App for diff context

**Chosen**: Store `Option<PathBuf>` for the git workdir in App.
**Why not discover each time**: `Repository::discover` is not free; caching the workdir avoids repeated lookups.
