## Why

When gati crashes or users encounter issues, there is no guided way to report bugs or provide feedback. The panic hook currently restores the terminal but provides no actionable next step. Adding a feedback pipeline lowers the barrier for bug reports and enriches them with automatic environment info, helping maintainers triage faster.

## What Changes

- On panic/crash, print a pre-filled GitHub issue URL containing environment details (OS, gati version, panic message)
- Add a `--bug-report` CLI flag that prints a pre-filled GitHub issue URL with environment info
- Add a bug report / feedback entry in the `?` help dialog so users know how to report issues
- If the `open` crate is available on the platform, attempt to open the URL in the default browser; otherwise just print it

## Capabilities

### New Capabilities
- `bug-report`: Pre-filled GitHub issue URL generation with environment info, panic hook integration, and CLI flag

### Modified Capabilities

## Impact

- `src/main.rs`: Add `--bug-report` flag via clap
- `src/app.rs`: Modify `install_panic_hook()` to print issue URL after terminal restore; add bug report entry to help dialog
- New module `src/bug_report.rs`: URL construction with environment info gathering
- `Cargo.toml`: Add `open` crate dependency (optional, for browser launch)
