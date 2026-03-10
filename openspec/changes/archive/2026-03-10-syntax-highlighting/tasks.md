## 1. Dependencies and Setup

- [x] 1.1 Add `syntect` dependency to Cargo.toml
- [x] 1.2 Create a `highlight` module (`src/highlight.rs`) that initializes and exposes a shared `SyntaxSet` and `Theme`

## 2. Syntax Detection

- [x] 2.1 Implement syntax detection from file path (extension-based via `SyntaxSet::find_syntax_for_file`)
- [x] 2.2 Implement first-line fallback detection (shebang/modeline via `SyntaxSet::find_syntax_by_first_line`)
- [x] 2.3 Return plain text syntax reference when no match is found

## 3. Highlighted Rendering

- [x] 3.1 Implement a function that takes lines + syntax reference + theme and returns styled ratatui `Span`s per line
- [x] 3.2 Map syntect `Color { r, g, b, a }` to `ratatui::style::Color::Rgb(r, g, b)` for foreground and background
- [x] 3.3 Integrate highlighting into `FileViewer::render_to_buffer`: replace `Span::raw(line_text)` with highlighted spans for visible lines

## 4. Integration and Polish

- [x] 4.1 Store detected `SyntaxReference` in `ViewerContent::File` on file load to avoid re-detection each frame
- [x] 4.2 Verify plain text fallback renders identically to current behavior for unknown file types
- [x] 4.3 Verify binary/empty/error/placeholder content types are unaffected by highlighting changes
