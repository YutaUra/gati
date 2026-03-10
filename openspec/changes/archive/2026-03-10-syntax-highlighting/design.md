## Context

gati v0.1 renders file contents as plain monochrome text. The file viewer reads files into `Vec<String>` and renders each line as `Span::raw`. The philosophy document specifies syntect for syntax highlighting, and the target audience expects colored code as a baseline.

## Goals / Non-Goals

**Goals:**

- Syntax-highlight file contents in the viewer using syntect
- Detect language from file extension (with first-line fallback, e.g., `#!/bin/bash`)
- Use a bundled dark terminal-friendly theme
- Maintain current rendering performance for typical source files

**Non-Goals:**

- User-configurable themes or color schemes
- Custom syntax definitions
- Treesitter-based highlighting
- Highlighting line numbers (gutter stays dim gray)

## Decisions

### Highlighting library: syntect

Use syntect with its bundled default syntax set and theme set. syntect is battle-tested (used by bat, delta, xi-editor) and provides both syntax detection and themed highlighting out of the box.

_Why not tree-sitter_: tree-sitter provides better accuracy for some languages but requires per-language parsers (native C libraries). syntect is pure Rust, bundles everything, and is sufficient for read-only code display.

### Theme: base16-eighties (bundled)

Use a single bundled theme from syntect's default theme set. base16-eighties works well on dark terminals, which is the primary target. No theme selection in this change.

_Why not user-configurable themes_: YAGNI for v0.2. Adding theme configuration requires config file support, which is a separate concern. A single good default is sufficient to validate the feature.

### Integration point: highlight at render time

Store the raw `Vec<String>` lines as before. On each render call, use syntect to produce styled spans. Cache the syntax reference and theme to avoid re-parsing the syntax set on every frame.

_Why not highlight on file load_: Highlighting at load time would store pre-styled data, coupling the data model to the rendering library. Keeping raw lines means the data model stays simple and testable, and the rendering can change independently.

_Why not cache highlighted lines_: For v0.2, re-highlighting visible lines each frame is fast enough (syntect processes ~10K lines/sec). Caching can be added later if profiling shows a need.

### Syntax detection: file extension with first-line fallback

Use syntect's `SyntaxSet::find_syntax_for_file()` which checks extension first, then falls back to first-line matching (shebangs, modelines). If no syntax matches, render as plain text.

### Color mapping: syntect Color → ratatui Color

Map syntect's `Color { r, g, b, a }` directly to `ratatui::style::Color::Rgb(r, g, b)`. This requires a true-color terminal, which is standard for the target audience (ghostty, iTerm2, kitty, WezTerm).

_Why not 256-color fallback_: The target users (terminal-native developers) overwhelmingly use true-color terminals. Adding a 256-color fallback adds complexity for a shrinking edge case.

## Risks / Trade-offs

- **[True-color assumption]** → Terminals without true-color support will see garbled or missing colors. Mitigation: acceptable for target audience; can add detection later.
- **[Render performance]** → Re-highlighting visible lines each frame could be slow for very large files. Mitigation: only visible lines (viewport height) are highlighted per frame; profiling can guide caching if needed.
- **[Binary size]** → syntect bundles syntax definitions and themes, adding ~5-10MB to the binary. Mitigation: acceptable for a developer tool; can use `syntect::dumps` for compressed assets if needed.
