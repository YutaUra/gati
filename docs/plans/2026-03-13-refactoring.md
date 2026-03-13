# gati Refactoring Plan

## Context

A parallel review based on 6 design principles (SOLID, KISS/YAGNI/DRY, Coupling/Cohesion, Error Handling/Robustness, Unix Philosophy/Separation of Concerns, Naming/Readability/Maintainability) identified the following issues:

- **UTF-8 byte boundary panics**: 5 locations that will reliably crash on multi-byte characters
- **DRY violations**: Duplication in comment rendering, diff loading, file reading, etc.
- **God Objects**: `file_viewer.rs` (2911 lines) and `app.rs` (2326 lines) with too many responsibilities
- **Lack of encapsulation**: Excessive pub fields, Law of Demeter violations
- **Silent error suppression**: FS operation errors not reported to users

Refactoring is split into 4 phases. Each phase can be committed independently.

---

## Phase 1: Critical Bug Fixes (UTF-8 Panics)

Fix 5 locations that **will panic** on multi-byte characters (Japanese filenames, comments, etc.).

### Step 1-1: Add UTF-8 safe slicing utilities

**New file**: `src/unicode.rs`

```rust
/// Return byte position of the last complete character within max_bytes
pub fn floor_char_boundary(s: &str, max_bytes: usize) -> usize

/// Return byte offset after skipping skip_chars characters from the start
pub fn char_skip_byte_offset(s: &str, skip_chars: usize) -> usize
```

- Add `mod unicode;` to `lib.rs`

### Step 1-2: `file_tree.rs:225` — Filename truncation

```rust
// Before
let skip = name.len() - max_name.saturating_sub(1);
format!("{prefix}\u{2026}{}", &name[skip..])

// After: calculate in char units
let char_count = name.chars().count();
let skip_chars = char_count.saturating_sub(max_name.saturating_sub(1));
let byte_offset = unicode::char_skip_byte_offset(name, skip_chars);
format!("{prefix}\u{2026}{}", &name[byte_offset..])
```

### Step 1-3: `file_tree.rs:238` — Comment text truncation

```rust
// Before
format!("{}...", &entry.text[..max_text.saturating_sub(3)])

// After
let end = unicode::floor_char_boundary(&entry.text, max_text.saturating_sub(3));
format!("{}...", &entry.text[..end])
```

### Step 1-4: `file_viewer.rs:997-998` — Comment editor display text

```rust
// Before
&edit.text[edit.text.len() - available..]

// After
let offset = edit.text.len().saturating_sub(available);
let offset = unicode::char_skip_byte_offset(
    &edit.text,
    edit.text[..offset].chars().count()
); // or use is_char_boundary loop
&edit.text[offset..]
```

### Step 1-5: `bug_report.rs:137` — String::truncate()

```rust
// Before
truncated.truncate(new_len);

// After
let new_len = unicode::floor_char_boundary(&truncated, new_len);
truncated.truncate(new_len);
```

### Step 1-6: `bug_report.rs:110` — Title byte position slicing

```rust
// Before
let title = format!("crash: {}", &first_line[..first_line.len().min(60)]);

// After
let end = unicode::floor_char_boundary(first_line, 60);
let title = format!("crash: {}", &first_line[..end]);
```

### Step 1-7: Add tests

Add tests with multi-byte characters (Japanese, emoji) for each fix location.

**Target files**: `src/unicode.rs`, `src/file_tree.rs`, `src/file_viewer.rs`, `src/bug_report.rs`, `src/lib.rs`

---

## Phase 2: Quick Wins (Small, Independent Improvements)

### Step 2-1: Remove redundant branching

**File**: `src/file_viewer.rs:568-573`

```rust
// Before: both branches are identical
let idx = if self.diff_mode {
    line_num.saturating_sub(1)
} else {
    line_num.saturating_sub(1)
};

// After
let idx = line_num.saturating_sub(1);
```

### Step 2-2: `HashMap<PathBuf, bool>` → `HashSet<PathBuf>`

**File**: `src/git_status.rs`

- `changed_dirs: HashMap<PathBuf, bool>` → `changed_dirs: HashSet<PathBuf>` (line 34)
- Change `propagate_to_dirs()` return type to `HashSet<PathBuf>` (lines 145-158)
  - `dirs.insert(p.to_path_buf(), true)` → `dirs.insert(p.to_path_buf())`
- `dir_has_changes()`: `contains_key` → `contains` (lines 101-107)

### Step 2-3: `highlight.rs:22` — Safe HashMap indexing

**File**: `src/highlight.rs:22`

```rust
// Before
let theme = theme_set.themes["base16-eighties.dark"].clone();

// After
let theme = theme_set
    .themes
    .remove("base16-eighties.dark")
    .unwrap_or_else(|| {
        theme_set.themes.into_values().next()
            .expect("syntect ThemeSet contains no themes")
    });
```

### Step 2-4: Make `flash_message` a named struct

**File**: `src/app.rs`

```rust
// New struct
struct FlashMessage {
    text: String,
    color: Color,
    created: Instant,
}

// Field change
flash_message: Option<FlashMessage>,
```

- Update setter locations (lines 235, 242, 347, 353, 360) and reader locations (lines 1002, 1021)

### Step 2-5: Extract magic numbers to constants

**File**: `src/file_viewer.rs` (add at top)

```rust
const BINARY_CHECK_BYTES: usize = 512;
const COMMENT_EDITOR_ROWS: usize = 2;
const STALE_COMMENT_BG: Color = Color::Indexed(52);
const COMMENT_RANGE_BG: Color = Color::Indexed(236);
const DIFF_ADDED_BG: Color = Color::Rgb(0, 40, 0);
const DIFF_REMOVED_BG: Color = Color::Rgb(40, 0, 0);
const MINIMAP_BG: Color = Color::Rgb(30, 30, 30);
const MINIMAP_VIEWPORT: Color = Color::Rgb(80, 80, 80);
```

**File**: `src/app.rs`

```rust
const DEFAULT_TREE_WIDTH_PERCENT: u16 = 30;
const HELP_DIALOG_WIDTH: u16 = 42;
```

- Replace each literal with the corresponding constant

### Step 2-6: Error handling improvements

**File**: `src/app.rs`

(a) `refresh_tree()` error notification (line 395):
```rust
// Before
let _ = self.file_tree.model.refresh_tree();
// After
if let Err(_e) = self.file_tree.model.refresh_tree() {
    self.flash_message = Some(FlashMessage::new(
        "Failed to refresh file tree", Color::Red,
    ));
}
```

(b) `toggle_expand()` error notification (line 829):
```rust
// Similarly notify via flash message
```

**File**: `src/file_viewer.rs`

(c) `reload_content` non-NotFound errors (line 353):
```rust
// Before
Err(_) => return false,
// After
Err(e) => {
    self.content = ViewerContent::Error(format!(
        "{} — {}", path.display(), e,
    ));
    return true;
}
```

**Target files**: `src/file_viewer.rs`, `src/app.rs`, `src/highlight.rs`, `src/git_status.rs`

---

## Phase 3: DRY Improvements (Consolidate Duplicate Code)

### Step 3-1: Unify file reading logic

**File**: `src/file_viewer.rs`

Extract common parts from `load_file` (lines 273-333) and `reload_content` (lines 338-389):

```rust
fn read_and_classify(path: &Path, highlighter: &Highlighter) -> ViewerContent {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return ViewerContent::Error(format!("{} — File has been deleted from disk", path.display()));
        }
        Err(e) => {
            return ViewerContent::Error(format!("{} — {}", path.display(), e));
        }
    };
    if is_binary(&bytes) { return ViewerContent::Binary(path.to_path_buf()); }
    let text = String::from_utf8_lossy(&bytes);
    if text.is_empty() { return ViewerContent::Empty(path.to_path_buf()); }
    let lines: Vec<String> = text.lines().map(String::from).collect();
    let first_line = lines.first().map(|s| s.as_str()).unwrap_or("");
    let syntax_name = highlighter.detect_syntax(path, first_line);
    ViewerContent::File { path: path.to_path_buf(), lines, syntax_name }
}
```

`load_file` and `reload_content` call this function.

### Step 3-2: Unify diff load pattern

**File**: `src/app.rs`

In `handle_mouse_click` (lines 832-836), instead of calling `compute_line_diff` + `compute_unified_diff` individually, dispatch `Action::FileSelected` or `Action::FileOpened` to unify through `handle_action`.

Specifically, lines 826-837 in `handle_mouse_click`:
```rust
app.file_tree.model.selected = entry_idx;
if app.file_tree.model.entries[entry_idx].is_directory {
    let _ = app.file_tree.model.toggle_expand();
} else {
    let path = app.file_tree.model.entries[entry_idx].path.clone();
    app.file_viewer.load_file(&path);
    app.load_diff_for_file(&path);  // ← call compute_diffs once
}
```

Due to borrowing constraints, `self.handle_file_action()` cannot be called directly (app is borrowed as `&mut`). Either inline the body of `load_diff_for_file` or make it a free function.

### Step 3-3: Unify comment rendering logic

**File**: `src/file_viewer.rs`

Extract duplication from `render_file_content` (lines 880-914) and `render_diff_mode` (lines 1247-1275):

```rust
fn render_inline_comments(
    &mut self,
    file_line_num: usize,
    inner: Rect,
    buf: &mut Buffer,
    render_row: &mut u16,
    max_rows: u16,
    focused: bool,
)
```

This function manages: editing check → editor rendering or comment block rendering → row_map registration → cursor highlight.

### Step 3-4: Unify auto-scroll logic

**File**: `src/file_viewer.rs`

Duplication between `render_file_content` (lines 783-794) and `render_diff_mode` (lines 1146-1155):

```rust
fn auto_scroll_for_editor(&mut self, viewport_height: usize) {
    if let Some(ref edit) = self.comment_edit {
        let target_idx = edit.target_line.saturating_sub(1);
        let need_visible = target_idx + 1 + COMMENT_EDITOR_ROWS;
        if need_visible > self.scroll_offset + viewport_height {
            self.scroll_offset = need_visible.saturating_sub(viewport_height);
        }
    }
}
```

### Step 3-5: Extract row background fill helper

**File**: `src/file_viewer.rs`

8 occurrences of `for x in rect.x..rect.x+rect.width { buf[(x,y)].set_bg(color) }` pattern:

```rust
fn fill_row_bg(buf: &mut Buffer, x: u16, y: u16, width: u16, bg: Color) {
    for col in x..x + width {
        buf[(col, y)].set_bg(bg);
    }
}
```

### Step 3-6: Unify separator rendering

**File**: `src/file_viewer.rs`

From `render_comment_block` (lines 961-968) and `render_comment_editor` (lines 1014-1021):

```rust
fn render_separator(
    buf: &mut Buffer, inner: Rect, render_row: &mut u16, max_rows: u16, color: Color,
)
```

### Step 3-7: Unify border style calculation

**File**: `src/file_viewer.rs`, `src/file_tree.rs`

```rust
// Add to components.rs
pub fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}
```

### Step 3-8: Consolidate test helpers

**New file**: `src/test_helpers.rs` (`#[cfg(test)]`)

Consolidate the following duplicate helpers:
- `setup_dir` (file_tree.rs:741, tree.rs:497, app.rs:1165)
- `setup_git_repo` (tree.rs:694, diff.rs:214, git_status.rs:172, app.rs:2166)
- `canonical_tmp_path` (diff.rs:209, git_status.rs:167)

Change `#[cfg(test)]` modules in each file to reference common helpers.

**Target files**: `src/file_viewer.rs`, `src/app.rs`, `src/components.rs`, `src/file_tree.rs`, `src/tree.rs`, `src/diff.rs`, `src/git_status.rs`, `src/test_helpers.rs` (new), `src/lib.rs`

---

## Phase 4: Structural Improvements (Encapsulation & Separation of Concerns)

### Step 4-1: Separate business logic from `draw()`

**File**: `src/app.rs`

Extract state preparation logic from `draw()` (lines 933-984) into `prepare_for_render()`:

```rust
impl App {
    fn prepare_for_render(&mut self) {
        // Comment update + staleness check (former lines 936-950)
        // comment_edit setup (former lines 952-962)
        // line_select_range setup (former lines 964-976)
        // commented_files construction (former lines 979-984)
    }
}
```

Call it before `terminal.draw()` in `event_loop`.

### Step 4-2: Introduce RenderContext pattern

**File**: `src/file_viewer.rs`

Change `comments`, `comment_edit`, `line_select_range` from pub fields to render arguments:

```rust
pub struct ViewerRenderContext<'a> {
    pub comments: &'a [(Comment, bool)],
    pub comment_edit: Option<&'a CommentEditState>,
    pub line_select_range: Option<(usize, usize)>,
}

pub fn render_to_buffer(
    &mut self, area: Rect, buf: &mut Buffer, focused: bool, ctx: &ViewerRenderContext,
)
```

Remove corresponding pub fields (`comments`, `comment_edit`, `line_select_range`).

### Step 4-3: Improve `CommentListEntry`

**File**: `src/file_tree.rs` → `src/comments.rs`

(a) Move `CommentListEntry` to `comments.rs`
(b) Convert to enum:

```rust
pub enum CommentListEntry {
    Header { file: PathBuf, display_name: String },
    Comment { file: PathBuf, start_line: usize, end_line: usize, text: String },
}
```

Update references in `file_tree.rs`.

### Step 4-4: Law of Demeter improvement — Add methods to FileTreeModel

**File**: `src/tree.rs`

```rust
impl FileTreeModel {
    pub fn select_at(&mut self, idx: usize) -> Option<&TreeEntry> {
        if idx < self.entries.len() {
            self.selected = idx;
            Some(&self.entries[idx])
        } else {
            None
        }
    }

    pub fn selected_path(&self) -> Option<&Path> {
        self.entries.get(self.selected).map(|e| e.path.as_path())
    }
}
```

Replace direct access in `app.rs` (lines 826-831 etc.) with method calls.

### Step 4-5: FileViewer sub-structuring

**File**: `src/file_viewer.rs`

```rust
struct HighlightCache {
    highlighted_lines: Vec<Vec<Span<'static>>>,
    hl_parse_state: Option<ParseState>,
    hl_highlight_state: Option<HighlightState>,
}

struct DiffState {
    line_diff: Option<LineDiff>,
    unified_diff: Option<UnifiedDiff>,
    diff_highlighted_lines: Vec<Vec<Span<'static>>>,
    diff_mode: bool,
    diff_line_numbers: Vec<Option<usize>>,
}
```

Replace `FileViewer` fields with these sub-structs. Update internal method access paths.

**Target files**: `src/file_viewer.rs`, `src/app.rs`, `src/tree.rs`, `src/comments.rs`, `src/file_tree.rs`

---

## Verification

After each phase, run:

```bash
# Compile check
cargo build

# Run all tests
cargo test

# Static analysis with clippy
cargo clippy -- -D warnings

# Manual check: Launch TUI in directory with Japanese filenames
mkdir -p /tmp/test-gati/日本語ディレクトリ
echo "テスト" > /tmp/test-gati/日本語ディレクトリ/テスト.txt
cd /tmp/test-gati && cargo run --manifest-path <path>/Cargo.toml
```

For Phase 1, focus on verifying:
- Comment list display with Japanese filenames
- Japanese text comment input/display
- Bug report URL generation with multi-byte characters in panic messages

---

## File Change Matrix

| File | Phase 1 | Phase 2 | Phase 3 | Phase 4 |
|------|---------|---------|---------|---------|
| `src/unicode.rs` (new) | x | | | |
| `src/test_helpers.rs` (new) | | | x | |
| `src/lib.rs` | x | | x | |
| `src/file_viewer.rs` | x | x | x | x |
| `src/file_tree.rs` | x | | x | x |
| `src/bug_report.rs` | x | | | |
| `src/app.rs` | | x | x | x |
| `src/git_status.rs` | | x | | |
| `src/highlight.rs` | | x | | |
| `src/components.rs` | | | x | |
| `src/tree.rs` | | | x | x |
| `src/diff.rs` | | | x | |
| `src/comments.rs` | | | | x |
