// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal escape hardening for diagnostic output (S-night-R4, LTS
//! final audit 2026-09-11).
//!
//! Rust owns the render path end to end (cells are typed, colors go
//! through the chroma dragon SGR boundary), but the diagnostic surface
//! interpolates user-supplied strings into error, warning and verbose
//! text. A config value containing a raw escape byte (proved with a
//! scene-custom `rain = "glyph<ESC>[2Jx"` probe) used to reach the
//! terminal verbatim through the testconf and validation error echo.
//! On a shared-config threat model that is a terminal command
//! injection: OSC 52 clipboard writes, screen clears and DSR reply
//! spam all become reachable from a file the victim was told to
//! download.
//!
//! The fix is a sink guard, not a source hunt. Every line printed
//! through the labeled error/warning renderer, the suggestion line
//! helper and the verbose family passes through `escape_ctrl`, which
//! renders control characters as visible `\u00XX` literals. The
//! rendered text is also more debuggable than the raw byte was: the
//! user sees exactly which character in their config is corrupt.
//!
//! Charsets and the message overlay stay untouched: the charset
//! validator already rejects control chars outright (the set never
//! renders them), and `sanitize_message_text` strips them from
//! overlay text before layout (bug #11). This module only guards the
//! diagnostic echo of the rejected value.

use std::borrow::Cow;
use std::fmt::Write as _;

/// Render C0, DEL and C1 control characters as `\u00XX` literals.
///
/// Newline passes through: it is the line separator the labeled-block
/// renderer splits on, not a payload. Everything else in the Unicode
/// `Cc` class (tab, carriage return, bell, escape, the C1 block)
/// becomes a visible escape so no byte in a diagnostic line can
/// reprogram the victim terminal. Fast path borrows when the input is
/// already clean, which is every normal error and warning.
pub(crate) fn escape_ctrl(msg: &str) -> Cow<'_, str> {
    if !msg.chars().any(|c| c != '\n' && c.is_control()) {
        return Cow::Borrowed(msg);
    }
    let mut out = String::with_capacity(msg.len() + 8);
    for c in msg.chars() {
        if c != '\n' && c.is_control() {
            // Control codepoints are at most U+009F, so four hex digits
            // are always enough.
            let _ = write!(out, "\\u{:04x}", u32::from(c));
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::escape_ctrl;
    use std::borrow::Cow;

    /// Clean prose borrows unchanged — the fast path every normal
    /// diagnostic takes.
    #[test]
    fn clean_text_borrows_unchanged() {
        let msg = "error: invalid value 'glyphx' for 'rain'";
        assert!(matches!(escape_ctrl(msg), Cow::Borrowed(s) if s == msg));
    }

    /// Newline is structural, not payload — preserved for the
    /// labeled-block line splitter.
    #[test]
    fn newline_passes_through() {
        let msg = "first line\nsecond line";
        assert!(matches!(escape_ctrl(msg), Cow::Borrowed(s) if s == msg));
    }

    /// The proved vector: an escape byte in a config value renders as
    /// a visible literal instead of executing.
    #[test]
    fn escape_byte_is_neutralized() {
        let raw = "glyph\u{1b}[2Jx";
        let out = escape_ctrl(raw);
        assert_eq!(out, "glyph\\u001b[2Jx");
        assert!(!out.contains('\u{1b}'));
    }

    /// Tab, carriage return, bell, DEL and the C1 block are all in the
    /// Cc class and all get escaped.
    #[test]
    fn every_control_class_member_is_escaped() {
        let raw = "a\tb\rc\u{7}d\u{7f}e\u{85}f";
        let out = escape_ctrl(raw);
        assert_eq!(out, "a\\u0009b\\u000dc\\u0007d\\u007fe\\u0085f");
    }

    /// Non-control content around an escaped byte survives verbatim.
    #[test]
    fn surrounding_text_survives() {
        let out = escape_ctrl("\u{1b}]52;c;abc");
        assert_eq!(out, "\\u001b]52;c;abc");
    }
}
