// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Bug #11 regression guards — width=1 invariant for the frame buffer.
//!
//! Cosmostrix's frame buffer holds one `char` per cell with no width
//! metadata. The terminal serializer writes chars verbatim, advancing
//! the cursor by 1 column per char. If a width=2 char (CJK ideograph,
//! emoji, fullwidth punctuation) ever enters the frame buffer:
//!
//!   1. The terminal advances the cursor by 2 columns.
//!   2. The renderer thinks it advanced by 1.
//!   3. Every subsequent cell in that row shifts right by 1.
//!   4. The rain appears "glitched" / "shifted" for the duration of
//!      the wide char's presence, then self-corrects on the next
//!      force_draw_everything (every 5 minutes by default).
//!
//! This is exactly Bug #11 (commit c1843fe, originally for -m message
//! wide chars) and its recurrence in GHOST_CHARS. Cosmostrix
//! will NEVER support emoji — only pure text/data. These tests guard
//! that invariant at the static-array and charset-preset layer.
//!
//! The runtime `debug_assert!` in `Frame::set` / `Frame::set_force`
//! is the primary defense — it catches ALL frame-write regressions
//! at the moment of the write. These tests cover the static sources
//! that may not be exercised by render tests (e.g. GHOST_CHARS only
//! fires at runtime when ghost events spawn).

#![cfg(test)]

use unicode_width::UnicodeWidthChar;

use crate::charset::{build_chars, Charset};
use crate::cloud::events::ghost::GHOST_CHARS;
use crate::interactive::intro_cosmic::BURST_CHARS;

/// Verify every entry in `GHOST_CHARS` is width=1. The previous bug
/// held fullwidth CJK ideographs (雨雷電風雲闇光)
/// which caused the entire row to the right of a ghost to shift right
/// by 1 for the ghost's 2-4 second lifetime.
#[test]
fn ghost_chars_all_width_one() {
    for (i, &ch) in GHOST_CHARS.iter().enumerate() {
        assert_eq!(
            UnicodeWidthChar::width(ch),
            Some(1),
            "GHOST_CHARS[{}] = {:?} (U+{:04X}) is not width=1 — Bug #11 regression",
            i,
            ch,
            ch as u32
        );
    }
}

/// Verify every entry in `BURST_CHARS` (cosmic intro particle glyphs).
#[test]
fn burst_chars_all_width_one() {
    for (i, &ch) in BURST_CHARS.iter().enumerate() {
        assert_eq!(
            UnicodeWidthChar::width(ch),
            Some(1),
            "BURST_CHARS[{}] = {:?} (U+{:04X}) is not width=1",
            i,
            ch,
            ch as u32
        );
    }
}

/// Verify every built-in charset preset produces only width=1 chars.
/// This catches future regressions if anyone adds a preset range that
/// includes wide codepoints (e.g. accidentally using fullwidth Katakana
/// 0x30A0-0x30FF instead of halfwidth 0xFF66-0xFF9D).
#[test]
fn all_charset_presets_produce_only_width_one() {
    let presets: &[Charset] = &[
        Charset::BINARY,
        Charset::HEX,
        Charset::ENGLISH_LETTERS,
        Charset::ENGLISH_DIGITS,
        Charset::ENGLISH_PUNCTUATION,
        Charset::KATAKANA,
        Charset::GREEK,
        Charset::CYRILLIC,
        Charset::HEBREW,
        Charset::BRAILLE,
        Charset::RUNIC,
        Charset::SYMBOLS,
        Charset::ARROWS,
        Charset::BLOCKS,
        Charset::BOXDRAW,
        Charset::MINIMAL,
        Charset::DNA,
        Charset::ZEN,
        Charset::DEFAULT,
        Charset::EXTENDED_DEFAULT,
        Charset::ASCII_SAFE,
        Charset::MATRIX,
    ];
    for &preset in presets {
        let chars = build_chars(preset, &[], false);
        assert!(!chars.is_empty(), "preset {:?} produced empty pool", preset);
        for (i, &ch) in chars.iter().enumerate() {
            assert_eq!(
                UnicodeWidthChar::width(ch),
                Some(1),
                "preset {:?} produced width!=1 char at index {}: {:?} (U+{:04X})",
                preset,
                i,
                ch,
                ch as u32
            );
        }
    }
}

/// Verify the same for `default_to_ascii = true` mode (forces
/// `Charset::NONE` to fall back to ASCII-safe `DEFAULT`).
#[test]
fn default_to_ascii_mode_produces_only_width_one() {
    let chars = build_chars(Charset::NONE, &[], true);
    assert!(!chars.is_empty(), "default_to_ascii produced empty pool");
    for (i, &ch) in chars.iter().enumerate() {
        assert_eq!(
            UnicodeWidthChar::width(ch),
            Some(1),
            "default_to_ascii char at index {} is not width=1: {:?} (U+{:04X})",
            i,
            ch,
            ch as u32
        );
    }
}

/// Verify `default_to_ascii = false` mode (extended default — adds
/// Katakana, Greek, Cyrillic, etc.).
#[test]
fn extended_default_mode_produces_only_width_one() {
    let chars = build_chars(Charset::NONE, &[], false);
    assert!(!chars.is_empty(), "extended default produced empty pool");
    for (i, &ch) in chars.iter().enumerate() {
        assert_eq!(
            UnicodeWidthChar::width(ch),
            Some(1),
            "extended default char at index {} is not width=1: {:?} (U+{:04X})",
            i,
            ch,
            ch as u32
        );
    }
}
