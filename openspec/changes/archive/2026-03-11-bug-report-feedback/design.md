## Context

gati is a terminal-based code review tool. When panics occur, the panic hook restores the terminal but provides no guidance on reporting the issue. There is no `--bug-report` flag or in-app feedback link. The project is hosted at `github.com/YutaUra/gati`.

Currently:
- `install_panic_hook()` in `src/app.rs` calls `restore_terminal()` then delegates to the default panic handler
- `main()` uses `anyhow::Result` — non-panic errors are printed by anyhow's default handler
- The help dialog (`?`) lists keybindings but has no feedback/report entry
- `cli-clipboard` is available; no URL-opening crate is present

## Goals / Non-Goals

**Goals:**
- Provide a pre-filled GitHub issue URL on panic with OS, gati version, and panic location
- Add `gati --bug-report` CLI flag to generate the same URL on demand
- Show a "Report bug" entry in the `?` help dialog
- Attempt to open the URL in the default browser via the `open` crate; fall back to printing the URL

**Non-Goals:**
- Telemetry or automatic crash reporting
- Custom issue templates on the GitHub repo (use query-parameter pre-fill only)
- Collecting logs or core dumps

## Decisions

### 1. New module `src/bug_report.rs`

Centralise URL construction and environment gathering in a single module.

**`gather_env() -> BugReportEnv`** collects:
- `gati_version`: from `env!("CARGO_PKG_VERSION")`
- `os`: from `std::env::consts::OS` / `std::env::consts::ARCH`
- `rust_version`: from the `rustc_version` build script, or compile-time `rustc --version` output via `env!("RUSTC_VERSION")` (set via `build.rs`)

**`build_url(title, body) -> String`** constructs:
```
https://github.com/YutaUra/gati/issues/new?title=<url-encoded>&body=<url-encoded>&labels=bug
```

**`open_or_print(url: &str)`**: try `open::that(url)`; on failure, print the URL to stderr.

**Alternative considered**: Using `webbrowser` crate instead of `open`. `open` is lighter (no extra features) and widely used. Chose `open` for minimal dependency footprint.

### 2. Panic hook enhancement

Extend `install_panic_hook()` to:
1. Restore terminal (existing)
2. Call original hook (existing)
3. Extract panic location and message from `PanicInfo`
4. Call `bug_report::build_url()` with the panic info
5. Call `bug_report::open_or_print()` with the URL

The URL is printed to stderr so it doesn't interfere with stdout piping.

### 3. `--bug-report` CLI flag

Add a boolean flag to `Cli` struct. When set, gather env info, build URL, open/print, and exit immediately (before entering TUI).

### 4. Help dialog entry

Add a line to `draw_help_dialog()`: `"   B           report bug / feedback"` under a new "Other" section. Pressing `B` (shift+b) in Normal mode triggers the same `open_or_print` flow. This reuses the existing `Action` dispatch pattern.

**Alternative considered**: Adding to the hint bar instead. The help dialog is better since hint bar space is limited and the action is infrequent.

## Risks / Trade-offs

- **[`open` crate may fail on headless/SSH]** → Fallback to printing the URL to stderr handles this gracefully.
- **[URL length limits]** → GitHub issue URLs have practical limits (~8KB). Truncate panic body if necessary to stay under 2000 chars.
- **[Version info accuracy]** → `CARGO_PKG_VERSION` is set at compile time, which is correct for release builds. For dev builds it shows `0.1.0` which is acceptable.
