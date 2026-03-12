/// Return the byte position of the last complete character that fits within
/// `max_bytes` bytes from the start of `s`.
///
/// If `max_bytes >= s.len()`, returns `s.len()`.
/// This is equivalent to `str::floor_char_boundary` (nightly-only as of Rust 1.80),
/// provided here for stable Rust compatibility.
pub fn floor_char_boundary(s: &str, max_bytes: usize) -> usize {
    if max_bytes >= s.len() {
        return s.len();
    }
    // Walk backwards from max_bytes until we hit a char boundary.
    let mut pos = max_bytes;
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

/// Return the byte offset reached after skipping `skip_chars` characters from
/// the start of `s`.
///
/// If `skip_chars` exceeds the number of characters in `s`, returns `s.len()`.
pub fn char_skip_byte_offset(s: &str, skip_chars: usize) -> usize {
    s.char_indices()
        .nth(skip_chars)
        .map(|(byte_pos, _)| byte_pos)
        .unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- floor_char_boundary ---

    #[test]
    fn floor_char_boundary_ascii_within_bounds() {
        let s = "hello";
        assert_eq!(floor_char_boundary(s, 3), 3);
    }

    #[test]
    fn floor_char_boundary_ascii_at_len() {
        let s = "hello";
        assert_eq!(floor_char_boundary(s, 5), 5);
    }

    #[test]
    fn floor_char_boundary_ascii_beyond_len() {
        let s = "hello";
        assert_eq!(floor_char_boundary(s, 100), 5);
    }

    #[test]
    fn floor_char_boundary_empty_string() {
        assert_eq!(floor_char_boundary("", 0), 0);
        assert_eq!(floor_char_boundary("", 5), 0);
    }

    #[test]
    fn floor_char_boundary_zero_max() {
        assert_eq!(floor_char_boundary("hello", 0), 0);
        assert_eq!(floor_char_boundary("日本語", 0), 0);
    }

    #[test]
    fn floor_char_boundary_japanese_full_chars() {
        // Each Japanese character is 3 bytes in UTF-8.
        let s = "日本語"; // 9 bytes total
        assert_eq!(floor_char_boundary(s, 9), 9); // all 3 chars
        assert_eq!(floor_char_boundary(s, 6), 6); // "日本"
        assert_eq!(floor_char_boundary(s, 3), 3); // "日"
    }

    #[test]
    fn floor_char_boundary_japanese_mid_char() {
        let s = "日本語"; // each char is 3 bytes
        // 4 bytes: can only fit "日" (3 bytes), not the start of "本"
        assert_eq!(floor_char_boundary(s, 4), 3);
        assert_eq!(floor_char_boundary(s, 5), 3);
        // 7 bytes: can fit "日本" (6 bytes)
        assert_eq!(floor_char_boundary(s, 7), 6);
        assert_eq!(floor_char_boundary(s, 8), 6);
    }

    #[test]
    fn floor_char_boundary_emoji() {
        // Most emoji are 4 bytes in UTF-8.
        let s = "\u{1f600}\u{1f601}"; // 😀😁 — 8 bytes total
        assert_eq!(floor_char_boundary(s, 8), 8);
        assert_eq!(floor_char_boundary(s, 4), 4); // first emoji
        assert_eq!(floor_char_boundary(s, 5), 4); // mid-second-emoji rounds down
        assert_eq!(floor_char_boundary(s, 3), 0); // can't fit even one emoji
        assert_eq!(floor_char_boundary(s, 1), 0);
    }

    #[test]
    fn floor_char_boundary_mixed_ascii_and_multibyte() {
        let s = "aあb"; // a(1) + あ(3) + b(1) = 5 bytes
        assert_eq!(floor_char_boundary(s, 5), 5);
        assert_eq!(floor_char_boundary(s, 4), 4); // "aあ"
        assert_eq!(floor_char_boundary(s, 3), 1); // mid-"あ" rounds back to "a"
        assert_eq!(floor_char_boundary(s, 2), 1); // mid-"あ" rounds back to "a"
        assert_eq!(floor_char_boundary(s, 1), 1); // "a"
    }

    // --- char_skip_byte_offset ---

    #[test]
    fn char_skip_byte_offset_ascii() {
        let s = "hello";
        assert_eq!(char_skip_byte_offset(s, 0), 0);
        assert_eq!(char_skip_byte_offset(s, 2), 2);
        assert_eq!(char_skip_byte_offset(s, 5), 5);
    }

    #[test]
    fn char_skip_byte_offset_beyond_len() {
        let s = "hello";
        assert_eq!(char_skip_byte_offset(s, 10), 5);
    }

    #[test]
    fn char_skip_byte_offset_empty_string() {
        assert_eq!(char_skip_byte_offset("", 0), 0);
        assert_eq!(char_skip_byte_offset("", 5), 0);
    }

    #[test]
    fn char_skip_byte_offset_japanese() {
        let s = "日本語"; // 3 chars, each 3 bytes
        assert_eq!(char_skip_byte_offset(s, 0), 0);
        assert_eq!(char_skip_byte_offset(s, 1), 3); // skip "日"
        assert_eq!(char_skip_byte_offset(s, 2), 6); // skip "日本"
        assert_eq!(char_skip_byte_offset(s, 3), 9); // skip all
    }

    #[test]
    fn char_skip_byte_offset_emoji() {
        let s = "\u{1f600}\u{1f601}"; // 2 emoji, each 4 bytes
        assert_eq!(char_skip_byte_offset(s, 0), 0);
        assert_eq!(char_skip_byte_offset(s, 1), 4);
        assert_eq!(char_skip_byte_offset(s, 2), 8);
        assert_eq!(char_skip_byte_offset(s, 3), 8); // beyond end
    }

    #[test]
    fn char_skip_byte_offset_mixed() {
        let s = "aあb"; // a(1) + あ(3) + b(1) = 5 bytes, 3 chars
        assert_eq!(char_skip_byte_offset(s, 0), 0);
        assert_eq!(char_skip_byte_offset(s, 1), 1); // skip 'a'
        assert_eq!(char_skip_byte_offset(s, 2), 4); // skip 'a' + 'あ'
        assert_eq!(char_skip_byte_offset(s, 3), 5); // skip all
    }
}
