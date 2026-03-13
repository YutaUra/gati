# God Object Decomposition Plan

## Problem

Two files concentrate too many responsibilities:
- `src/file_viewer.rs` (2948 lines): file I/O, syntax highlighting, diff, minimap, comments, rendering, navigation, input handling
- `src/app.rs` (2376 lines): orchestration, comment workflow, git/FS, event loop, layout/rendering, mouse handling

Moving functions to other files does not solve this. The goal is to identify **distinct responsibility domains**, define **clean interfaces** between them, and place **state ownership** correctly.

## Design Principles

1. **Single Responsibility**: Each module has one reason to change
2. **Borrow Checker Compatibility**: Design interfaces that work WITH Rust's ownership model, not against it
3. **No Premature Abstraction**: Use traits only where polymorphism is needed (the `Component` trait already exists; don't add more)
4. **Parameter Passing over Shared State**: Sub-modules receive explicit inputs, not hidden `&self` access to unrelated fields
5. **Preserve the Elm Pattern**: `Action` enum dispatching in `handle_action` stays as the integration point

## Key Architectural Decisions

### FileViewer Strategy: Extract Pure Sub-systems

FileViewer stays as the integration struct. Sub-modules are extracted as **pure functions** or **self-contained structs** that take explicit parameters. This avoids borrow checker issues because Rust allows borrowing different struct fields simultaneously.

```
// This compiles because highlight_cache and content are separate fields:
if let ViewerContent::File { lines, syntax_name, .. } = &self.content {
    self.highlight_cache.ensure_up_to(end, lines, syntax_name, &self.highlighter);
}
```

### App Strategy: Free Functions Taking `&mut App`

Do NOT split App into multiple sub-structs with disjoint ownership. The borrow checker would prevent `comment_ops` from reading `file_viewer` while writing `comment_store` (both owned by different sub-structs borrowed from the same parent).

Instead, use free functions:
```rust
// comment_ops.rs
pub fn start_comment(app: &mut App) { ... }

// mod.rs (handle_action)
Action::StartComment => comment_ops::start_comment(self),
```

This achieves file-level separation without fighting the borrow checker.

---

## FileViewer Decomposition

### Responsibility Domains Identified

| Domain | Responsibility | Fields | Est. Lines |
|--------|---------------|--------|------------|
| Content Loading | Read files, classify (text/binary/empty/error) | `content`, `highlighter` | ~100 |
| Highlight Cache | Incremental syntax highlighting with syntect state | `highlight_cache` | ~100 |
| Diff State | Diff data, line mappings, diff-mode highlighting | `diff` | ~200 |
| Minimap | Compute markers, render minimap, click translation | `minimap_rect` (write) | ~150 |
| Comment Rendering | Render comment blocks and inline editor | None (stateless) | ~120 |
| Render Utilities | fill_row_bg, skip_chars_in_spans, line_number_width, gutter_spans | None (stateless) | ~80 |
| Navigation & Viewport | scroll, cursor, click, ensure_cursor_visible | scroll/cursor fields | Stays in mod.rs |
| Render Orchestration | render_to_buffer, render_file_content, render_diff_mode | layout fields | Stays in mod.rs |
| Event Handling | Component::handle_event, key dispatch | None | Stays in mod.rs |

### Target Structure

```
src/file_viewer/
  mod.rs               (~1200 lines)  ← FileViewer struct, navigation, render orchestration, Component impl
  content.rs            (~100 lines)   ← ViewerContent enum, load(), is_binary()
  highlight_cache.rs    (~100 lines)   ← HighlightCache struct + ensure_up_to()
  diff_state.rs         (~200 lines)   ← DiffState struct + all diff methods
  minimap.rs            (~150 lines)   ← MinimapMarker, compute_markers(), render(), row_to_line()
  comment_renderer.rs   (~120 lines)   ← render_block(), render_editor(), render_separator()
  render_utils.rs       (~80 lines)    ← skip_chars_in_spans(), fill_row_bg(), line_number_width(), gutter_spans()
```

### Module Interfaces

#### `content.rs`
```rust
pub enum ViewerContent {
    Placeholder,
    File { path: PathBuf, lines: Vec<String>, syntax_name: String },
    Binary(PathBuf),
    Empty(PathBuf),
    Error(String),
}

pub fn load(path: &Path, highlighter: &Highlighter) -> ViewerContent
pub(crate) fn is_binary(bytes: &[u8]) -> bool
```

#### `highlight_cache.rs`
```rust
pub(crate) struct HighlightCache { ... }
impl HighlightCache {
    pub fn new() -> Self
    pub fn clear(&mut self)
    pub fn ensure_up_to(&mut self, end: usize, lines: &[String], syntax_name: &str, highlighter: &Highlighter)
    pub fn get(&self, idx: usize) -> Option<&[Span<'static>]>
}
```
Key design: accepts `lines` and `syntax_name` as parameters (not via `&self`), enabling field-level borrow splitting at call sites.

#### `diff_state.rs`
```rust
pub struct DiffState { ... }
impl DiffState {
    pub fn new() -> Self
    pub fn clear(&mut self)
    pub fn set(&mut self, line_diff: Option<LineDiff>, unified_diff: Option<UnifiedDiff>)
    pub fn ensure_highlighted(&mut self, syntax_name: &str, highlighter: &Highlighter)
    pub fn compute_line_numbers(&mut self)
    pub fn total_lines(&self) -> usize
    pub fn file_line_at(&self, display_idx: usize) -> Option<usize>
    pub fn display_index_for_file_line(&self, file_line: usize) -> usize
    pub fn resolve_nearest_file_line(&self, cursor: usize) -> Option<usize>
}
```

#### `minimap.rs`
```rust
pub enum MinimapMarker { Added, Modified, Removed, Comment, StaleComment }

pub fn compute_markers(total_lines: usize, height: usize, diff: &DiffState, comments: &[(Comment, bool)]) -> Vec<Option<MinimapMarker>>
pub fn render(area: Rect, buf: &mut Buffer, markers: &[Option<MinimapMarker>], scroll_offset: usize, visible_height: usize, total_lines: usize)
pub fn row_to_line(row: u16, minimap_height: u16, total_lines: usize) -> usize
```
All pure functions — no state, no `&self`. Currently forced into methods only because they live in the same file.

#### `comment_renderer.rs`
```rust
pub fn render_block(comment: &Comment, is_stale: bool, inner: Rect, buf: &mut Buffer, render_row: &mut u16, max_rows: u16)
pub fn render_editor(edit: &CommentEditState, inner: Rect, buf: &mut Buffer, render_row: &mut u16, max_rows: u16)
fn render_separator(buf: &mut Buffer, inner: Rect, render_row: &mut u16, max_rows: u16, color: Color)
```

#### `render_utils.rs`
```rust
pub fn skip_chars_in_spans(spans: Vec<Span<'_>>, skip: usize) -> Vec<Span<'static>>
pub fn fill_row_bg(buf: &mut Buffer, x: u16, y: u16, width: u16, bg: Color)
pub fn line_number_width(total_lines: usize) -> usize
pub fn gutter_spans(line_num: usize, gutter_width: usize, line_diff: Option<&LineDiff>) -> Vec<Span<'static>>
```
`gutter_spans` changes from `&self` method to free function (currently uses only `self.diff.line_diff`).

---

## App Decomposition

### Responsibility Domains Identified

| Domain | Responsibility | Key Fields | Est. Lines |
|--------|---------------|------------|------------|
| Core / Orchestration | Struct def, new(), handle_action, prepare_for_render | All (owns everything) | ~350 |
| Comment Workflow | Enter/save/delete/export comments, input mode transitions | comment_store, input_mode | ~300 |
| Git & Diff Workers | Background git status, diff loading | git_workdir, git_worker | ~80 |
| Event Loop | Terminal lifecycle, event dispatch loop | None (drives App) | ~220 |
| Mouse Handling | Click/drag/scroll dispatch, resize, focus toggle | resizing, focus_mode, layout fields | ~180 |
| Layout & Rendering | Pane layout, draw dispatch, help dialog, hint bar | border_column, tree_inner_y | ~200 |
| Tests | All test functions | — | ~1100 |

### Target Structure

```
src/app/
  mod.rs               (~350 lines)   ← App struct, new(), handle_action, prepare_for_render
  comment_ops.rs       (~300 lines)   ← start_comment, delete, export, handle_comment_input, build_list
  git_worker.rs        (~80 lines)    ← GitStatusWorker, load_file_with_diff, set_diff_for_file
  event_loop.rs        (~220 lines)   ← run, install_panic_hook, init/restore terminal, event_loop
  mouse.rs             (~180 lines)   ← handle_mouse, handle_mouse_click/drag, toggle_focus_mode
  render.rs            (~200 lines)   ← draw, draw_help_dialog
  tests.rs             (~1100 lines)  ← all #[cfg(test)] tests
```

### Module Interfaces

#### `comment_ops.rs`
```rust
pub fn start_comment(app: &mut App)
pub fn delete_comment_at_cursor(app: &mut App)
pub fn export_comments(app: &mut App)
pub fn refresh_comment_list(app: &mut App)
pub fn comment_list_hints(app: &App) -> Option<String>
pub fn handle_comment_input(app: &mut App, key: KeyEvent)
pub fn handle_line_select(app: &mut App, key: KeyEvent) -> bool
pub fn extract_code_context(content: &ViewerContent, file: &Path, start: usize, end: usize) -> Vec<String>
pub fn build_comment_list_entries(store: &CommentStore, root: &Path) -> Vec<CommentListEntry>
```
Free functions taking `&mut App` — no trait, no sub-struct. Clean and borrow-checker-safe.

#### `git_worker.rs`
```rust
pub struct GitStatusWorker { ... }
impl GitStatusWorker {
    pub fn spawn(dir: PathBuf) -> Self
    pub fn try_recv(&self) -> Option<Option<GitStatus>>
}
pub fn load_file_with_diff(viewer: &mut FileViewer, path: &Path, git_workdir: &Option<PathBuf>)
pub fn set_diff_for_file(viewer: &mut FileViewer, path: &Path, git_workdir: &Option<PathBuf>)
```
Zero coupling to App. Takes exact types needed.

#### `event_loop.rs`
```rust
pub fn run(target: &StartupTarget) -> anyhow::Result<()>
fn install_panic_hook()
fn init_terminal() -> anyhow::Result<DefaultTerminal>
fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> anyhow::Result<()>
```

#### `mouse.rs`
```rust
pub fn handle_mouse(app: &mut App, mouse: MouseEvent, terminal_width: u16)
fn handle_mouse_click(app: &mut App, mouse: MouseEvent)
fn handle_mouse_drag(app: &mut App, mouse: MouseEvent, terminal_width: u16)
pub fn toggle_focus_mode(app: &mut App)
pub fn start_mouse_line_select(app: &mut App)
```

#### `render.rs`
```rust
pub fn draw(frame: &mut Frame, app: &mut App)
fn draw_help_dialog(buf: &mut Buffer, area: Rect)
```

---

## Migration Plan

### Phase A: FileViewer Decomposition (6 steps)

Each step: extract → build → test → commit.

**A-1: Extract render_utils.rs + comment_renderer.rs** (Zero Risk)
- Pure stateless functions, no borrow complexity
- Move: skip_chars_in_spans, fill_row_bg, line_number_width, gutter_spans (change &self to explicit params)
- Move: render_comment_block, render_comment_editor, render_separator (remove unused &self)
- Move existing tests for skip_chars_in_spans

**A-2: Extract content.rs** (Low Risk)
- Move: ViewerContent enum, read_and_classify(), is_binary()
- Re-export ViewerContent from mod.rs
- load_file() and reload_content() call content::load()

**A-3: Extract highlight_cache.rs** (Medium Risk)
- Change ensure_highlighted_up_to() signature: accept (lines, syntax_name, highlighter) explicitly
- Borrow split at call sites: `&self.content` + `&mut self.highlight_cache` compiles

**A-4: Extract diff_state.rs** (Medium Risk)
- Move DiffState and all diff methods
- FileViewer delegates: self.diff.compute_line_numbers(), self.diff.file_line_at(), etc.
- Move diff-related tests

**A-5: Extract minimap.rs** (Low Risk)
- All functions already near-stateless, change &self methods to free functions
- Move MinimapMarker enum

**A-6: Convert to module directory**
- `git mv src/file_viewer.rs src/file_viewer/mod.rs`
- Add mod declarations and re-exports

### Phase B: App Decomposition (5 steps)

**B-1: Extract git_worker.rs** (Zero Risk)
- Zero App dependency, cleanest separation
- Move GitStatusWorker struct + impl, load_file_with_diff, set_diff_for_file

**B-2: Extract render.rs** (Low Risk)
- Move draw() and draw_help_dialog()
- Pure read-aggregator + 3 layout cache writes

**B-3: Extract mouse.rs** (Low Risk)
- Move handle_mouse, handle_mouse_click, handle_mouse_drag, toggle_focus_mode, start_mouse_line_select
- Takes &mut App — same coupling as current, just different file

**B-4: Extract comment_ops.rs** (Medium Risk)
- Convert App methods → free functions taking &mut App
- Replace self.field with app.field throughout
- Update handle_action match arms to call comment_ops::*

**B-5: Extract event_loop.rs + tests.rs**
- Move run, install_panic_hook, init/restore terminal, event_loop
- Move #[cfg(test)] mod tests to tests.rs
- Update test imports to reference sub-modules

### Phase C: Convert app.rs to module directory
- `git mv src/app.rs src/app/mod.rs`
- Add mod declarations

---

## Field Visibility Strategy

After decomposition, App fields need visibility for sibling modules:

```rust
pub struct App {
    pub(super) comment_store: CommentStore,   // for comment_ops
    pub(super) input_mode: InputMode,         // for comment_ops, mouse, event_loop
    pub(super) file_viewer: FileViewer,       // for render, mouse, comment_ops
    pub(super) file_tree: FileTreeWidget,     // for render, mouse, comment_ops
    pub(super) focus: Focus,                  // for render, mouse
    pub(super) flash_message: Option<FlashMessage>,  // for comment_ops, render
    // ... etc
}
```

Use `pub(super)` for fields needed within `app/` but not outside. Fields needed by external code (e.g., integration tests) stay `pub(crate)`.

---

## Verification

After each step:
```bash
cargo build
cargo test
cargo clippy -- -D warnings
```

After complete decomposition:
- file_viewer/mod.rs: ~1200 lines (was 2948)
- app/mod.rs: ~350 lines (was 2376)
- Manual TUI testing for rendering correctness

## Anti-patterns to Avoid

1. **Don't introduce traits** for App sub-behaviors — no polymorphism needed
2. **Don't split App into owned sub-structs** — borrow checker will fight back
3. **Don't use channels/events between sub-modules** — unnecessary indirection
4. **Don't use `dyn Trait`** in render paths — performance matters for 60fps TUI
5. **Don't move render_inline_comments / row_map** — their coupling to render state is intentional

## Expected Outcome

| Metric | Before | After |
|--------|--------|-------|
| file_viewer.rs | 2948 lines, 9 responsibilities | mod.rs ~1200 lines, 3 responsibilities (navigation + render orchestration + events) |
| app.rs | 2376 lines, 6 responsibilities | mod.rs ~350 lines, 1 responsibility (orchestration) |
| New modules | 0 | 11 focused modules |
| Max file size | 2948 lines | ~1200 lines |
| Test count | No change | No change |
