## 1. Search infrastructure

- [x] 1.1 Add `search_files(root, query)` function to tree.rs that recursively walks and filters by file name
- [x] 1.2 Add `SearchState` struct to FileTree with query, saved entries, saved selection

## 2. Search mode behavior

- [x] 2.1 Handle `/` key to activate search mode (save state, show input)
- [x] 2.2 Handle character input to update query and re-filter incrementally
- [x] 2.3 Handle Backspace to delete characters from query
- [x] 2.4 Handle Enter to confirm search (exit search, keep selection)
- [x] 2.5 Handle Escape to cancel search (restore saved state)
- [x] 2.6 Handle j/k navigation in search results

## 3. Rendering

- [x] 3.1 Show search input line at bottom of tree pane when search active
- [x] 3.2 Update tree title to indicate search mode (e.g., "Files [/query]")
- [x] 3.3 Update hint bar for search mode

## 4. Edge cases

- [x] 4.1 Empty query shows all files (original tree)
- [x] 4.2 No matches shows "No matches" or empty tree
- [x] 4.3 Annotate search results with git status when available
