use std::io::Write;

use base64::{Engine, engine::general_purpose::STANDARD};

/// Copy text to the clipboard.
///
/// Tries the OS clipboard (via cli-clipboard) first. If that fails
/// (e.g. no display server on a remote SSH/kubectl session), falls back
/// to the OSC 52 escape sequence which asks the terminal emulator to
/// write to the local clipboard. This works across SSH, kubectl exec,
/// docker exec, and tmux because the escape sequence travels through
/// stdout to the user's terminal.
pub fn copy(text: &str) -> Result<(), CopyError> {
    // Try OS-native clipboard first (works when a display server is available).
    if cli_clipboard::set_contents(text.to_string()).is_ok() {
        return Ok(());
    }

    // Fallback: OSC 52. Supported by iTerm2, WezTerm, Alacritty, kitty,
    // Windows Terminal, foot, and others.
    let mut stdout = std::io::stdout().lock();
    write_osc52(&mut stdout, text).map_err(|_| CopyError::Osc52Failed)
}

/// Write text to the clipboard using the OSC 52 escape sequence.
///
/// OSC 52 format: ESC ] 52 ; c ; <base64-payload> BEL
/// The terminal emulator intercepts this and copies the decoded payload
/// to the system clipboard. Accepts any `Write` impl so the output
/// destination is testable.
fn write_osc52(w: &mut impl Write, text: &str) -> std::io::Result<()> {
    let encoded = STANDARD.encode(text.as_bytes());
    write!(w, "\x1b]52;c;{}\x07", encoded)?;
    w.flush()
}

/// Error type for clipboard copy operations.
#[derive(Debug)]
pub enum CopyError {
    /// OSC 52 write to stdout failed (unlikely in practice).
    Osc52Failed,
}

impl std::fmt::Display for CopyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CopyError::Osc52Failed => write!(f, "Failed to write OSC 52 escape sequence"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc52_produces_correct_escape_sequence() {
        let mut buf = Vec::new();
        write_osc52(&mut buf, "hello").unwrap();
        let output = String::from_utf8(buf).unwrap();
        // ESC ] 52 ; c ; <base64("hello")> BEL
        assert_eq!(output, "\x1b]52;c;aGVsbG8=\x07");
    }

    #[test]
    fn osc52_empty_string() {
        let mut buf = Vec::new();
        write_osc52(&mut buf, "").unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "\x1b]52;c;\x07");
    }

    #[test]
    fn osc52_multibyte_round_trips() {
        let text = "こんにちは";
        let mut buf = Vec::new();
        write_osc52(&mut buf, text).unwrap();
        let output = String::from_utf8(buf).unwrap();

        // Extract base64 payload between "c;" and BEL
        let payload = output
            .strip_prefix("\x1b]52;c;")
            .unwrap()
            .strip_suffix("\x07")
            .unwrap();
        let decoded = STANDARD.decode(payload).unwrap();
        assert_eq!(std::str::from_utf8(&decoded).unwrap(), text);
    }

    #[test]
    fn osc52_special_characters() {
        let text = "line1\nline2\ttab";
        let mut buf = Vec::new();
        write_osc52(&mut buf, text).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let payload = output
            .strip_prefix("\x1b]52;c;")
            .unwrap()
            .strip_suffix("\x07")
            .unwrap();
        let decoded = STANDARD.decode(payload).unwrap();
        assert_eq!(std::str::from_utf8(&decoded).unwrap(), text);
    }
}
