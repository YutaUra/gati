## 1. FileTreeModel filter support

- [x] 1.1 Add `filter_changed: bool` to `FileTreeModel`
- [x] 1.2 Implement `toggle_filter()` that rebuilds entries with filter applied
- [x] 1.3 Filter children in `toggle_expand()` when filter is active
- [x] 1.4 Preserve selection across filter toggle when possible

## 2. FileTree integration

- [x] 2.1 Handle `g` key in `FileTree::handle_event` to call `toggle_filter`
- [x] 2.2 Update tree title: "Files" vs "Changed Files"
- [x] 2.3 Filter is no-op outside git

## 3. App integration

- [x] 3.1 Update hint bar to include `g changed` when inside git repo
