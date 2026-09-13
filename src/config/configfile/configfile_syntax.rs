// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Line-level config syntax scanners — extracted from `configfile.rs`
//! (NIGHT-hunt-38-supermassive) to keep the parser file under the 800-LOC
//! hard cap (see `src/RULES_LOC.md`), following the same pattern as
//! `configfile_dump.rs` / `configfile_promote.rs` /
//! `configfile_parsed.rs`.
//!
//! Owns:
//! - `strip_inline_comment`: comment stripping that respects quotes.
//! - `unquoted_hash_inside_array`: the bug #7 array-`#` detector.
//! - `double_equals_note` / `colon_separator_note`: NIGHT-hunt-38-supermassive
//!   separator-typo diagnostics.
//!
//! The owner's fatal report (commit 6198431, manual testing) showed the
//! forgiving parser blessing two typo classes that real TOML rejects:
//! `set == "x"` — `split_once('=')` stores `= "x"` as the VALUE, so a
//! charset silently becomes garbage glyphs and an ambient entry misfires
//! the "legacy multi-field format" migration essay; and `set : "x"` — the
//! YAML/JSON habit, already malformed but without a diagnostic that
//! pointed at the colon.
//!
//! Both now land in `malformed_lines` with a targeted `# ERROR:` note, so
//! all three validation surfaces (--testconf, startup, live-reload
//! watcher) reject them with the same root-cause message.

/// NIGHT-hunt-38-supermassive: the `key == value` typo guard.
///
/// Returns the `# ERROR:` note when the VALUE side of the first `=`
/// starts with another `=` — i.e. the line was `key == ...` or
/// `key = = ...`. TOML has no `==` operator; the old parser silently
/// stored the remainder (e.g. `= "x"`) as the value.
///
/// The check runs on the RAW trimmed value BEFORE quote-stripping, so a
/// legitimate quoted value whose content starts with `=` (e.g.
/// `set = "=x"`) is unaffected — only a genuine second separator
/// matches. Unquoted leading-`=` values are rejected, mirroring the
/// bug #19 asymmetry (unquoted `[` is an array opener; quoted `[` is a
/// glyph). Owners of `=`-leading charset pools must quote them.
pub(crate) fn double_equals_note(value: &str) -> Option<&'static str> {
    value
        .starts_with('=')
        .then_some("  # ERROR: double '=' — TOML uses a single 'key = value' separator")
}

/// NIGHT-hunt-38-supermassive: the `key : value` typo note.
///
/// For lines with NO `=` at all: returns the `# ERROR:` note when the
/// line has the `key : value` shape (a key-ish token, a colon, a
/// non-empty rest) — the YAML/JSON habit. Other `=`-less lines keep the
/// generic malformed diagnostic.
///
/// The LHS token must look like a config key (ASCII alphanumeric plus
/// `-`/`_`/`.`) so arbitrary stray text (a pasted URL, prose) does not
/// get the colon hint — it is a hint, and it only fires for the shape
/// that actually IS the typo class.
pub(crate) fn colon_separator_note(line: &str) -> Option<&'static str> {
    let (lhs, rest) = line.split_once(':')?;
    if lhs.is_empty() || rest.trim().is_empty() {
        return None;
    }
    let key_shaped = lhs
        .trim()
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
    key_shaped.then_some("  # ERROR: ':' is not a TOML separator — write 'key = value'")
}

/// Strip inline comments (`# ...`) from a config line, respecting quoted strings.
///
/// A `#` inside a double-quoted or single-quoted string is NOT treated as a
/// comment — it's part of the value. This is critical for hex color values
/// like `red = "#ff0000"` where `#` is the standard hex prefix.
///
/// Example:
///   `color = green # my favorite`     → `color = green`
///   `red = "#ff0000" # comment`       → `red = "#ff0000"`
///   `msg = "it's #1" # note`          → `msg = "it's #1"`
///
/// Unquoted `#` still works as before for backward compatibility.
#[inline]
pub(super) fn strip_inline_comment(line: &str) -> &str {
    let mut in_dquote = false;
    let mut in_squote = false;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' if !in_squote => in_dquote = !in_dquote,
            '\'' if !in_dquote => in_squote = !in_squote,
            '#' if !in_dquote && !in_squote => {
                return &line[..i];
            }
            _ => {}
        }
    }
    line
}

/// (bug #7): Detect unquoted '#' INSIDE an array value.
/// Returns `Some(byte_idx)` if the line has an unquoted '#' while bracket
/// depth > 0. Catches `rain = [#ff0000, #00ff00]` (user mistake — should
/// quote hex). Returns `None` for legitimate cases: quoted '#' inside
/// strings, or '#' AFTER the closing ']' (trailing comment).
#[inline]
pub(crate) fn unquoted_hash_inside_array(line: &str) -> Option<usize> {
    let mut in_dquote = false;
    let mut in_squote = false;
    let mut bracket_depth: i32 = 0;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' if !in_squote => in_dquote = !in_dquote,
            '\'' if !in_dquote => in_squote = !in_squote,
            '[' if !in_dquote && !in_squote => bracket_depth += 1,
            ']' if !in_dquote && !in_squote => bracket_depth -= 1,
            '#' if !in_dquote && !in_squote && bracket_depth > 0 => {
                return Some(i);
            }
            _ => {}
        }
    }
    None
}
