# Diff View

## What

Add line-level diff visualization to the file viewer:
1. **Gutter markers** in normal (full file) mode showing which lines changed relative to HEAD
2. **Unified diff mode** toggled with `d` key showing additions/removals with context

## Why

Users reviewing code need to quickly identify what changed. Gutter markers provide at-a-glance change visibility in context, while unified diff mode provides focused change review similar to `git diff`.

## Capabilities

- **New capability: diff-view** — Line-level diff computation and unified diff rendering
- **Modified capability: file-viewer** — Gutter change markers, diff mode toggle, title/hint bar updates
