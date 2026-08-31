// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use std::char;
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Charset(u32);

impl Charset {
    pub(crate) const NONE: Charset = Charset(0);
    pub(crate) const ENGLISH_LETTERS: Charset = Charset(0x1);
    pub(crate) const ENGLISH_DIGITS: Charset = Charset(0x2);
    pub(crate) const ENGLISH_PUNCTUATION: Charset = Charset(0x4);
    pub(crate) const KATAKANA: Charset = Charset(0x8);
    pub(crate) const GREEK: Charset = Charset(0x10);
    pub(crate) const CYRILLIC: Charset = Charset(0x20);
    pub(crate) const HEBREW: Charset = Charset(0x80);
    pub(crate) const BINARY: Charset = Charset(0x100);
    pub(crate) const HEX: Charset = Charset(0x200);
    pub(crate) const BRAILLE: Charset = Charset(0x800);
    pub(crate) const RUNIC: Charset = Charset(0x1000);
    pub(crate) const SYMBOLS: Charset = Charset(0x2000);
    pub(crate) const ARROWS: Charset = Charset(0x4000);
    pub(crate) const BLOCKS: Charset = Charset(0x8000);
    pub(crate) const BOXDRAW: Charset = Charset(0x10000);
    /// Minimal charset: a single nabla glyph (U+2207). Owner-picked
    /// 2026-08-30 after the charset-minimal masterclass research
    /// (cffd549) — the 17-glyph junk-drawer pool was replaced by ONE
    /// mathematically elegant glyph. Second single-glyph preset after
    /// zen. See docs/research/CHARSET_MINIMAL_MASTERCLASS_RESEARCH.md.
    pub(crate) const MINIMAL: Charset = Charset(0x20000);
    pub(crate) const DNA: Charset = Charset(0x40000);
    /// Zen charset: a single `|` pipe character. The minimalist's
    /// minimalist — one glyph, infinite rain. Default for cinematic
    /// and monolith scenes on the Cosmic Dragon journey.
    pub(crate) const ZEN: Charset = Charset(0x80000);

    pub(crate) const DEFAULT: Charset = Charset(0x7);
    pub(crate) const EXTENDED_DEFAULT: Charset = Charset(0xE);
    pub(crate) const ASCII_SAFE: Charset = Charset(0x3);
    pub(crate) const MATRIX: Charset = Charset(0xB);

    pub(crate) fn contains(self, other: Charset) -> bool {
        (self.0 & other.0) != 0
    }
}

/// v51 did-you-mean audit: canonical preset-name list. Kept in lockstep
/// with the `charset_from_str` match arms and the `--list-charsets`
/// printer (single source of truth for suggestions; the printer keeps
/// its own formatted descriptions).
pub(crate) const CHARSET_PRESET_NAMES: &[&str] = &[
    "auto",
    "matrix",
    "ascii",
    "extended",
    "english",
    "digits",
    "punc",
    "binary",
    "hex",
    "katakana",
    "greek",
    "cyrillic",
    "hebrew",
    "blocks",
    "symbols",
    "arrows",
    "retro",
    "cyberpunk",
    "hacker",
    "minimal",
    "code",
    "dna",
    "braille",
    "runic",
    "zen",
];

pub(crate) fn charset_from_str(spec: &str, default_to_ascii: bool) -> Result<Charset, String> {
    let spec = spec.trim().to_ascii_lowercase();
    match spec.as_str() {
        "auto" => Ok(if default_to_ascii {
            Charset::ASCII_SAFE
        } else {
            Charset::MATRIX
        }),
        "matrix" => Ok(Charset::MATRIX),
        "ascii" => Ok(Charset::DEFAULT),
        "extended" => Ok(Charset::EXTENDED_DEFAULT),
        "english" => Ok(Charset::ENGLISH_LETTERS),
        "digits" | "dec" | "decimal" => Ok(Charset::ENGLISH_DIGITS),
        "punc" => Ok(Charset::ENGLISH_PUNCTUATION),
        "bin" | "binary" | "01" => Ok(Charset::BINARY),
        "hex" | "hexadecimal" => Ok(Charset::HEX),
        "katakana" => Ok(Charset::KATAKANA),
        "greek" => Ok(Charset::GREEK),
        "cyrillic" => Ok(Charset::CYRILLIC),
        "hebrew" => Ok(Charset::HEBREW),
        "blocks" => Ok(Charset::BLOCKS),
        "symbols" => Ok(Charset::SYMBOLS),
        "arrows" => Ok(Charset::ARROWS),
        "retro" => Ok(Charset::BOXDRAW),
        "cyberpunk" => Ok(Charset(
            Charset::ENGLISH_LETTERS.0 | Charset::HEX.0 | Charset::KATAKANA.0 | Charset::SYMBOLS.0,
        )),
        "hacker" => Ok(Charset(
            Charset::ENGLISH_LETTERS.0
                | Charset::HEX.0
                | Charset::ENGLISH_PUNCTUATION.0
                | Charset::SYMBOLS.0,
        )),
        "minimal" => Ok(Charset::MINIMAL),
        "code" => Ok(Charset(
            Charset::ENGLISH_LETTERS.0
                | Charset::ENGLISH_DIGITS.0
                | Charset::ENGLISH_PUNCTUATION.0
                | Charset::SYMBOLS.0,
        )),
        "dna" => Ok(Charset::DNA),
        "braille" => Ok(Charset::BRAILLE),
        "runic" => Ok(Charset::RUNIC),
        "zen" => Ok(Charset::ZEN),
        _ => Err({
            // v51 did-you-mean audit: suggest the closest preset (same
            // edit-distance <= 2 policy as colors/scenes). Custom
            // [charset-custom.<name>] blocks are not suggested here —
            // charset_from_str has no config access; --list-charsets
            // lists them.
            let tip = crate::cli::suggestion::closest_value_match(&spec, CHARSET_PRESET_NAMES)
                .map(|s| crate::cli::suggestion::format_value_suggestion(&s))
                .unwrap_or_default();
            format!(
                "error: unknown charset '{spec}'{tip}\n\n  Use --list-charsets to see available charsets."
            )
        }),
    }
}

fn push_range(out: &mut Vec<char>, start: u32, end: u32) {
    for v in start..=end {
        if let Some(ch) = char::from_u32(v) {
            // Cosmic Dragon principle: only single-width chars pass. Wide
            // (CJK fullwidth) and zero-width chars are excluded to prevent
            // glyph alignment corruption in the renderer. Permanent design.
            if ch.width() == Some(1) {
                out.push(ch);
            }
        }
    }
}

pub(crate) fn build_chars(
    mut charset: Charset,
    user_ranges: &[(char, char)],
    default_to_ascii: bool,
) -> Vec<char> {
    if charset == Charset::NONE && user_ranges.is_empty() {
        charset = if default_to_ascii {
            Charset::DEFAULT
        } else {
            Charset::EXTENDED_DEFAULT
        };
    }

    let mut out: Vec<char> = Vec::new();

    if charset.contains(Charset::BINARY) {
        push_range(&mut out, 0x30, 0x31);
    }
    if charset.contains(Charset::HEX) {
        push_range(&mut out, 0x30, 0x39);
        push_range(&mut out, 0x41, 0x46);
    }
    if charset.contains(Charset::ENGLISH_LETTERS) {
        push_range(&mut out, 0x41, 0x5A);
        push_range(&mut out, 0x61, 0x7A);
    }
    if charset.contains(Charset::ENGLISH_DIGITS) {
        push_range(&mut out, 0x30, 0x39);
    }
    if charset.contains(Charset::ENGLISH_PUNCTUATION) {
        push_range(&mut out, 0x21, 0x2F);
        push_range(&mut out, 0x3A, 0x40);
        push_range(&mut out, 0x5B, 0x60);
        push_range(&mut out, 0x7B, 0x7E);
    }
    if charset.contains(Charset::KATAKANA) {
        push_range(&mut out, 0xFF66, 0xFF9D);
    }
    if charset.contains(Charset::GREEK) {
        push_range(&mut out, 0x0370, 0x03FF);
    }
    if charset.contains(Charset::CYRILLIC) {
        push_range(&mut out, 0x0410, 0x044F);
    }
    if charset.contains(Charset::HEBREW) {
        push_range(&mut out, 0x0590, 0x05FF);
        push_range(&mut out, 0xFB1D, 0xFB4F);
    }
    if charset.contains(Charset::BRAILLE) {
        push_range(&mut out, 0x2800, 0x28FF);
    }
    if charset.contains(Charset::RUNIC) {
        push_range(&mut out, 0x16A0, 0x16FF);
    }
    if charset.contains(Charset::SYMBOLS) {
        out.extend(
            "∞∑∫√π∆Ωµλ≈≠≤≥×÷±∂∇∈∉∩∪⊂⊃⊆⊇⊕⊗"
                .chars()
                .filter(|&c| c.width() == Some(1)),
        );
    }
    if charset.contains(Charset::ARROWS) {
        out.extend("←→↑↓↔↕⇐⇒⇑⇓⇔↖↗↘↙".chars().filter(|&c| c.width() == Some(1)));
    }
    if charset.contains(Charset::BLOCKS) {
        push_range(&mut out, 0x2580, 0x259F);
    }
    if charset.contains(Charset::BOXDRAW) {
        push_range(&mut out, 0x2500, 0x257F);
    }
    if charset.contains(Charset::MINIMAL) {
        // Owner decision (2026-08-30, after the cffd549 research): the
        // pool is a single nabla glyph. Total commitment to one shape —
        // every trail is a column of nabla marks, so the rain reads as
        // falling gradients (the nabla IS the gradient operator) and
        // pairs with the OKLab trail gradient for pure two-dimensional
        // depth. U+2207 is East-Asian-Width Ambiguous -> width 1 under
        // this project's unicode-width config; the filter below guards
        // the pool regardless.
        out.extend("∇".chars().filter(|&c| c.width() == Some(1)));
    }
    if charset.contains(Charset::DNA) {
        out.extend("ACGTacgt".chars().filter(|&c| c.width() == Some(1)));
    }
    if charset.contains(Charset::ZEN) {
        out.push('|');
    }

    for &(a, b) in user_ranges {
        let start = a as u32;
        let end = b as u32;
        for v in start..=end {
            if let Some(ch) = char::from_u32(v) {
                // Cosmic Dragon principle: only single-width characters are
                // safe for the column-based renderer. Wide characters (e.g.,
                // CJK fullwidth) would corrupt glyph alignment. Permanent.
                if ch.width() == Some(1) {
                    out.push(ch);
                }
            }
        }
    }

    if out.is_empty() {
        out.push('0');
        out.push('1');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charset_auto_selects_ascii_safe_when_non_utf() {
        let cs = charset_from_str("auto", true).unwrap();
        assert_eq!(cs, Charset::ASCII_SAFE);
    }

    #[test]
    fn build_chars_binary_has_only_0_and_1() {
        let out = build_chars(Charset::BINARY, &[], true);
        assert_eq!(out, vec!['0', '1']);
    }

    #[test]
    fn build_chars_zen_has_only_pipe() {
        let out = build_chars(Charset::ZEN, &[], true);
        assert_eq!(out, vec!['|']);
    }

    #[test]
    fn build_chars_minimal_has_only_nabla() {
        // Owner decision 2026-08-30: minimal = one nabla glyph (U+2207).
        // Lockstep with the MINIMAL arm's pool string — a conscious edit
        // is required to change the preset (this test fails loudly if
        // someone silently rewidens the pool).
        let out = build_chars(Charset::MINIMAL, &[], true);
        assert_eq!(out, vec!['∇']);
    }

    #[test]
    fn charset_from_str_resolves_minimal() {
        assert_eq!(
            charset_from_str("minimal", false).unwrap(),
            Charset::MINIMAL
        );
        assert_eq!(
            charset_from_str("MINIMAL", false).unwrap(),
            Charset::MINIMAL
        );
    }

    #[test]
    fn charset_from_str_resolves_zen() {
        assert_eq!(charset_from_str("zen", false).unwrap(), Charset::ZEN);
        assert_eq!(charset_from_str("ZEN", false).unwrap(), Charset::ZEN);
    }
}

#[cfg(test)]
mod suggestion_tests {
    use super::*;

    /// v51 did-you-mean audit: unknown charset errors suggest the closest
    /// preset (edit-distance <= 2, same policy as colors).
    #[test]
    fn unknown_charset_typo_suggests_closest_preset() {
        let err = charset_from_str("binari", false).unwrap_err();
        assert!(
            err.contains("tip: a similar value exists: 'binary'"),
            "charset typo must suggest the closest preset, got: {err}"
        );
        let err = charset_from_str("katakan", false).unwrap_err();
        assert!(
            err.contains("tip: a similar value exists: 'katakana'"),
            "got: {err}"
        );
    }

    #[test]
    fn unknown_charset_distant_value_no_suggestion() {
        let err = charset_from_str("totally-not-a-charset", false).unwrap_err();
        assert!(
            !err.contains("tip: a similar"),
            "distant value must not suggest, got: {err}"
        );
    }

    #[test]
    fn preset_names_stay_in_lockstep_with_parser() {
        // Every preset name must parse; every parser arm must be listed.
        for name in CHARSET_PRESET_NAMES {
            assert!(
                charset_from_str(name, false).is_ok(),
                "CHARSET_PRESET_NAMES lists '{name}' but charset_from_str rejects it"
            );
        }
    }
}
