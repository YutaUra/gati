## 1. Dependencies and Module Setup

- [x] 1.1 Add `open` crate to `Cargo.toml`
- [x] 1.2 Create `src/bug_report.rs` module and register in `src/main.rs`

## 2. Core URL Construction

- [x] 2.1 Implement `BugReportEnv` struct and `gather_env()` to collect gati version, OS, arch
- [x] 2.2 Implement `build_url(title, body) -> String` with URL encoding and 2000-char truncation
- [x] 2.3 Implement `open_or_print(url)` — try `open::that()`, fall back to stderr print
- [x] 2.4 Add unit tests for `build_url` (env info present, truncation, URL encoding)

## 3. Panic Hook Integration

- [x] 3.1 Modify `install_panic_hook()` in `src/app.rs` to extract panic message/location and call `bug_report::open_or_print(bug_report::build_url(...))`
- [x] 3.2 Add test verifying panic info is included in generated URL

## 4. CLI Flag

- [x] 4.1 Add `--bug-report` boolean flag to `Cli` struct in `src/main.rs`
- [x] 4.2 Handle `--bug-report` in `main()` — generate URL, open/print, exit before TUI
- [x] 4.3 Add CLI test for `--bug-report` flag parsing

## 5. In-App Keybinding

- [x] 5.1 Add `Action::BugReport` variant to `components::Action`
- [x] 5.2 Handle `KeyCode::Char('B')` in Normal mode to dispatch `Action::BugReport`
- [x] 5.3 Handle `Action::BugReport` in `App::handle_action()` — call `bug_report::open_or_print`
- [x] 5.4 Add "B report bug / feedback" entry to `draw_help_dialog()`

## 6. Verification

- [x] 6.1 `cargo build` with zero warnings
- [x] 6.2 `cargo test` — all tests pass
- [ ] 6.3 Manual: `gati --bug-report` prints/opens URL with correct env info
- [ ] 6.4 Manual: Press `B` in Normal mode opens/prints bug report URL
