## 1. Diff Computation Module

- [x] 1.1 Create `src/diff.rs` module with `DiffLineKind` enum and `LineDiff` struct
- [x] 1.2 Implement `compute_line_diff()` using git2 to compare working tree vs HEAD
- [x] 1.3 Create `UnifiedDiffLine` enum and `UnifiedDiff` struct for diff mode
- [x] 1.4 Implement `compute_unified_diff()` to generate parsed unified diff lines
- [x] 1.5 Handle edge cases: untracked files (all lines as added), files not in git, binary files

## 2. Gutter Markers in Normal Mode

- [x] 2.1 Add `line_diff: Option<LineDiff>` to `FileViewer`
- [x] 2.2 Render gutter change markers (`▎` yellow for modified, green for added) in `render_to_buffer`
- [x] 2.3 Compute and pass `LineDiff` when loading a file in App

## 3. Unified Diff Mode

- [x] 3.1 Add `diff_mode: bool` and `unified_diff: Option<UnifiedDiff>` to `FileViewer`
- [x] 3.2 Handle `d` key to toggle diff mode on/off
- [x] 3.3 Render unified diff content with colored lines (green for +, red for -, cyan for @@)
- [x] 3.4 Show "No changes" message when file has no diff
- [x] 3.5 Scrolling works in diff mode (reuse existing scroll logic)

## 4. UI Integration

- [x] 4.1 Update viewer pane title: "Preview" in normal mode, "Diff" in diff mode
- [x] 4.2 Update hint bar to include `d diff` when inside a git repo
- [x] 4.3 Store git workdir path in App for diff computation
- [x] 4.4 Diff mode unavailable outside git (d key is no-op)
