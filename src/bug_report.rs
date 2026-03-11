use std::fmt::Write;

const REPO_URL: &str = "https://github.com/YutaUra/gati";
const MAX_URL_LEN: usize = 8000;

pub struct BugReportEnv {
    pub gati_version: String,
    pub os: String,
    pub term: String,
    pub shell: String,
    pub locale: String,
    pub colorterm: String,
    pub terminal_size: String,
    pub is_ssh: bool,
}

pub fn gather_env() -> BugReportEnv {
    let env_var = |key: &str| std::env::var(key).unwrap_or_default();

    let terminal_size = crossterm::terminal::size()
        .map(|(w, h)| format!("{}x{}", w, h))
        .unwrap_or_else(|_| "unknown".into());

    BugReportEnv {
        gati_version: env!("CARGO_PKG_VERSION").into(),
        os: format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH),
        term: env_var("TERM"),
        shell: env_var("SHELL"),
        locale: env_var("LANG"),
        colorterm: env_var("COLORTERM"),
        terminal_size,
        is_ssh: std::env::var("SSH_CONNECTION").is_ok(),
    }
}

fn format_env_section(env: &BugReportEnv) -> String {
    let mut s = String::from("## Environment\n\n");
    s.push_str(&format!("- **gati version**: {}\n", env.gati_version));
    s.push_str(&format!("- **OS**: {}\n", env.os));
    s.push_str(&format!("- **Terminal**: {}\n", if env.term.is_empty() { "unknown" } else { &env.term }));
    s.push_str(&format!("- **Shell**: {}\n", if env.shell.is_empty() { "unknown" } else { &env.shell }));
    s.push_str(&format!("- **Terminal size**: {}\n", env.terminal_size));
    if !env.colorterm.is_empty() {
        s.push_str(&format!("- **Color support**: {}\n", env.colorterm));
    }
    if !env.locale.is_empty() {
        s.push_str(&format!("- **Locale**: {}\n", env.locale));
    }
    if env.is_ssh {
        s.push_str("- **SSH**: yes\n");
    }
    s
}

const USER_TEMPLATE: &str = "\
## What happened?\n\
\n\
<!-- A clear description of the bug or feedback -->\n\
\n\
\n\
## Steps to reproduce\n\
\n\
<!-- How can we reproduce this? -->\n\
1. \n\
2. \n\
3. \n\
\n\
## Expected behavior\n\
\n\
<!-- What did you expect to happen? -->\n\
\n\
\n\
## Additional context\n\
\n\
<!-- Screenshots, error messages, or anything else that might help -->\n\
\n";

/// Build a pre-filled GitHub issue URL for a user-initiated bug report.
pub fn build_url(title: &str, body: &str) -> String {
    let env = gather_env();
    let env_section = format_env_section(&env);

    let full_body = if body.is_empty() {
        format!("{}\n{}", USER_TEMPLATE, env_section)
    } else {
        format!("## Description\n\n{}\n\n{}", body, env_section)
    };

    let base = format!("{}/issues/new?labels=bug", REPO_URL);
    build_url_with_truncation(&base, title, &full_body)
}

/// Build a pre-filled GitHub issue URL specifically for panic reports.
/// Uses a crash-specific template instead of the user template.
/// `crash_log` is the full panic output (message + backtrace if available).
pub fn build_panic_url(crash_log: &str) -> String {
    let env = gather_env();
    let env_section = format_env_section(&env);

    let full_body = format!(
        "## Crash Report\n\n\
         gati crashed unexpectedly. The information below has been \
         automatically collected to help diagnose the issue.\n\n\
         ### Crash log\n\n\
         ```\n{}\n```\n\n\
         ## Steps to reproduce\n\n\
         <!-- What were you doing when the crash occurred? -->\n\
         1. \n\
         2. \n\
         3. \n\n\
         ## Additional context\n\n\
         <!-- Any other details that might help -->\n\n\
         {}",
        crash_log, env_section
    );

    // Extract a short title from the first line of the crash log
    let first_line = crash_log.lines().next().unwrap_or("unknown panic");
    let title = format!("crash: {}", &first_line[..first_line.len().min(60)]);
    let base = format!("{}/issues/new?labels=bug%2Ccrash", REPO_URL);
    build_url_with_truncation(&base, &title, &full_body)
}

fn build_url_with_truncation(base: &str, title: &str, body: &str) -> String {
    let url = format_issue_url(base, title, body);
    if url.len() <= MAX_URL_LEN {
        return url;
    }

    // Truncate body to fit within limit.
    // Each body char may expand to 3 chars (%XX) in the worst case,
    // so we iteratively trim until the URL fits.
    let mut truncated = body.to_string();
    while format_issue_url(base, title, &truncated).len() > MAX_URL_LEN && !truncated.is_empty() {
        let new_len = truncated.len() * 9 / 10;
        truncated.truncate(new_len);
    }
    if !truncated.is_empty() && truncated.len() < body.len() {
        truncated.push_str("...(truncated)");
    }

    format_issue_url(base, title, &truncated)
}

fn format_issue_url(base: &str, title: &str, body: &str) -> String {
    format!(
        "{}&title={}&body={}",
        base,
        url_encode(title),
        url_encode(body)
    )
}

/// Result of attempting to open a URL in the browser.
pub enum OpenResult {
    /// Browser opened successfully.
    Opened,
    /// Browser failed to open; URL is available for display.
    Failed(String),
}

/// Attempt to open URL in the default browser without blocking.
pub fn try_open(url: &str) -> OpenResult {
    // that_detached() spawns the browser without waiting for it to exit,
    // so the TUI event loop is not blocked.
    match open::that_detached(url) {
        Ok(_) => OpenResult::Opened,
        Err(e) => OpenResult::Failed(format!("{}", e)),
    }
}

/// Attempt to open URL in the default browser; print to stderr on failure.
/// Used outside the TUI (e.g. --bug-report flag, panic hook).
pub fn open_or_print(url: &str) {
    if let OpenResult::Failed(e) = try_open(url) {
        eprintln!("\nFailed to open browser ({}). Open this URL manually:\n  {}\n", e, url);
    }
}

/// Format a URL as a clickable terminal hyperlink using OSC 8.
/// Uses BEL (0x07) as the string terminator for broader compatibility.
/// Terminals that don't support OSC 8 will just show the display text.
pub fn hyperlink(url: &str, text: &str) -> String {
    format!("\x1b]8;;{}\x07{}\x1b]8;;\x07", url, text)
}

/// Minimal percent-encoding for URL query parameters.
fn url_encode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 2);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                let _ = write!(encoded, "%{:02X}", byte);
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_url_contains_env_info() {
        let url = build_url("Test bug", "Something broke");
        let decoded = url_decode_rough(&url);
        assert!(decoded.contains("gati version"));
        assert!(decoded.contains(env!("CARGO_PKG_VERSION")));
        assert!(decoded.contains(std::env::consts::OS));
    }

    #[test]
    fn build_url_contains_terminal_info() {
        let url = build_url("Test", "");
        let decoded = url_decode_rough(&url);
        assert!(decoded.contains("Terminal"));
        assert!(decoded.contains("Shell"));
        assert!(decoded.contains("Terminal size"));
    }

    #[test]
    fn build_url_empty_body_has_user_template() {
        let url = build_url("Bug report", "");
        let decoded = url_decode_rough(&url);
        assert!(decoded.contains("What happened?"));
        assert!(decoded.contains("Steps to reproduce"));
        assert!(decoded.contains("Expected behavior"));
    }

    #[test]
    fn build_url_with_body_shows_description() {
        let url = build_url("My title", "Detailed description");
        let decoded = url_decode_rough(&url);
        assert!(decoded.contains("Detailed description"));
        assert!(decoded.contains("Description"));
    }

    #[test]
    fn build_url_under_max_length() {
        let long_body = "x".repeat(20000);
        let url = build_url("Bug", &long_body);
        assert!(url.len() <= MAX_URL_LEN);
    }

    #[test]
    fn build_url_starts_with_github_issues() {
        let url = build_url("title", "body");
        assert!(url.starts_with("https://github.com/YutaUra/gati/issues/new"));
    }

    #[test]
    fn panic_url_contains_crash_info() {
        let crash_log = "panicked at 'index out of bounds: len 5, index 10', src/app.rs:123:5\n\nstack backtrace:\n  0: main";
        let url = build_panic_url(crash_log);
        let decoded = url_decode_rough(&url);
        assert!(decoded.contains("Crash Report"));
        assert!(decoded.contains("index out of bounds"));
        assert!(decoded.contains("src/app.rs:123:5"));
    }

    #[test]
    fn panic_url_has_crash_label() {
        let url = build_panic_url("panicked at 'oops', main.rs:1:1");
        assert!(url.contains("labels=bug%2Ccrash"));
    }

    #[test]
    fn panic_url_includes_backtrace_in_body() {
        let crash_log = "panicked at 'fail', src/lib.rs:10:5\n\nstack backtrace:\n  0: std::panicking\n  1: app::run";
        let url = build_panic_url(crash_log);
        let decoded = url_decode_rough(&url);
        assert!(decoded.contains("stack backtrace"));
        assert!(decoded.contains("app::run"));
    }

    #[test]
    fn url_encode_handles_special_chars() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
    }

    /// Rough URL decode for test assertions (handles + and %XX).
    fn url_decode_rough(s: &str) -> String {
        let s = s.replace('+', " ");
        let mut result = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '%' {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                }
            } else {
                result.push(c);
            }
        }
        result
    }
}
