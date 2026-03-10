# Changed Files Filter — Design

## Approach

Add `filter_changed: bool` to `FileTreeModel`. When active, rebuild the tree entries to only include files with `git_status.is_some()` and directories where `dir_has_changes()` is true. On `toggle_expand`, filter children before inserting.

## Key Decisions

### Decision: Rebuild tree on toggle rather than maintaining two entry lists

**Chosen**: Rescan root directory and apply filter on toggle.
**Why not dual lists**: Simpler, avoids state synchronization, and expand/collapse state is reset cleanly.

### Decision: `g` key for filter toggle

Confirmed by spec. Does not conflict with existing bindings.
