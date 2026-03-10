## 1. Dependencies and Setup

- [x] 1.1 Add `git2` dependency to Cargo.toml
- [x] 1.2 Create `src/git_status.rs` module with `FileStatus` enum and `GitStatus` struct

## 2. Git Status Computation

- [x] 2.1 Implement `GitStatus::from_dir()` that discovers the git repository and computes file statuses
- [x] 2.2 Handle non-git directories gracefully (return empty/None status)
- [x] 2.3 Map git2 status flags to `FileStatus` enum (Modified, Added, Deleted, Renamed, Untracked)
- [x] 2.4 Compute directory-level change indicators by propagating file statuses up to ancestor directories

## 3. File Tree Integration

- [x] 3.1 Add optional `FileStatus` field to `TreeEntry`
- [x] 3.2 Pass git status data to `FileTreeModel` and annotate entries on construction and expand
- [x] 3.3 Render status markers (`[M]`, `[A]`, `[D]`, `[R]`, `[?]`, `[●]`) with colors in `FileTree::render_to_buffer`

## 4. App Integration

- [x] 4.1 Compute git status in `App::new()` and pass to file tree
- [x] 4.2 Verify non-git directories work without markers
- [x] 4.3 Verify markers appear correctly for modified, added, deleted, renamed, and untracked files
