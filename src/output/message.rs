// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Message overlay text sanitization.
//!
//! `sanitize_message_text` enforces the 1-char-1-cell invariant required by
//! the `-m` and `-mb` overlay. Wide chars
//! (CJK, emoji) and zero-width chars (combining marks, ZWJ) are replaced with
//! `?` so the user sees that a char was dropped. C0/C1 control chars (except
//! `\n`) are stripped entirely.

use unicode_width::UnicodeWidthChar;

/// Sanitize message text for the `-m` overlay.
///
/// Wide chars (width ≥ 2: CJK, emoji) and zero-width chars (combining marks,
/// ZWJ) break the 1-char-1-cell invariant. Both are replaced with `?`.
/// C0/C1 control chars (except `\n`, which is preserved for multi-line
/// messages) are stripped. Unassigned chars (width `None`) are skipped.
///
/// A stderr warning is emitted if any chars were replaced or stripped, so
/// the user knows their input was modified.
pub(crate) fn sanitize_message_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut skipped_wide = 0u32;
    let mut skipped_ctrl = 0u32;
    for ch in input.chars() {
        if ch == '\n' {
            out.push(ch);
            continue;
        }
        // Reject C0/C1 control chars (except \n handled above).
        if ch.is_control() {
            skipped_ctrl += 1;
            continue;
        }
        match ch.width() {
            Some(1) => out.push(ch),
            Some(0) | Some(2) => {
                // Zero-width (combining marks, ZWJ) or wide (CJK, emoji) —
                // both break the 1-char-1-cell invariant. Cosmic Dragon
                // principle: these are PERMANENTLY rejected, never supported.
                // Replace with `?` so the user sees that a char was dropped.
                skipped_wide += 1;
                out.push('?');
            }
            // Some chars return None (e.g., unassigned) — skip entirely.
            None => {
                skipped_ctrl += 1;
            }
            // Chars with width >= 3 are extremely rare (some terminal
            // implementations reserve them for special glyphs). Treat
            // them as wide — replace with '?' to preserve alignment.
            // Same Cosmic Dragon principle: never render multi-cell chars.
            _ => {
                skipped_wide += 1;
                out.push('?');
            }
        }
    }
    if skipped_wide > 0 || skipped_ctrl > 0 {
        crate::output::eprintln_warn_labeled(&format!(
            "--message contained {} wide/zero-width char(s) (replaced with '?') and {} control char(s) (removed). Wide chars (CJK, emoji) break cell alignment — see Bug #11.",
            skipped_wide, skipped_ctrl
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::sanitize_message_text;

    /// (bug #11): ASCII-only messages pass through unchanged.
    #[test]
    fn sanitize_preserves_ascii_message() {
        let input = "Hello World! 0123 #hash $var";
        assert_eq!(sanitize_message_text(input), input);
    }

    /// (bug #11): newlines are preserved (needed for multi-line `-m`).
    #[test]
    fn sanitize_preserves_newlines() {
        let input = "Line1\nLine2\nLine3";
        assert_eq!(sanitize_message_text(input), input);
    }

    /// (bug #11): wide CJK chars replaced with '?'.
    /// Without this, "世界" (2 chars, 4 cells) breaks the 1-char-1-cell
    /// invariant in the message box layout, causing rain to the right
    /// of the box to glitch.
    #[test]
    fn sanitize_replaces_wide_cjk_chars() {
        let result = sanitize_message_text("Hello 世界");
        assert_eq!(result, "Hello ??");
    }

    /// (bug #11): emoji replaced with '?'.
    #[test]
    fn sanitize_replaces_emoji() {
        let result = sanitize_message_text("Galaxy 🌌 emoji");
        assert_eq!(result, "Galaxy ? emoji");
    }

    /// (bug #11): control chars (except \n) stripped.
    #[test]
    fn sanitize_strips_control_chars() {
        let result = sanitize_message_text("Tab\there\x07bell");
        assert_eq!(result, "Tabherebell");
    }

    /// (bug #11): mixed content — ASCII passes, wide/control filtered.
    #[test]
    fn sanitize_handles_mixed_content() {
        let result = sanitize_message_text("Hello 世界 🌌 αβγ #hash $var");
        // "Hello " (6) + "??" (世界) + " " + "?" (🌌) + " " + "αβγ" (3) + " #hash $var"
        assert_eq!(result, "Hello ?? ? αβγ #hash $var");
    }

    /// (bug #11): empty message stays empty.
    #[test]
    fn sanitize_handles_empty_message() {
        assert_eq!(sanitize_message_text(""), "");
    }
}
