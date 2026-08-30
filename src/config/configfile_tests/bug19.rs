// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! (bug #19) regression + single-glyph depth stress tests.
//!
//! Bug summary (owner-found 2026-08-30): a quoted value whose CONTENT is
//! a single `[` — `set = "["` inside a `[charset-custom.<name>]` block —
//! was quote-stripped to `[` BEFORE array detection, so the multi-line
//! array consumer mistook the bare bracket for an unterminated array.
//! Depending on what followed, the parser either rejected the line
//! ("array never closed") or silently absorbed following lines into a
//! bogus value. The owner hit it while previewing single-glyph charset
//! candidates; the exact repro is the first test below.
//!
//! Fix: `raw_is_quoted` snapshot taken before quote-stripping; a quoted
//! value is NEVER an array (see `parse_config_text`).
//!
//! The stress battery exists so no single-glyph charset — or any quoted
//! value built from syntax-looking characters — can regress the parser
//! again: every ASCII punctuation char, the four config metachars
//! (`[ ] # =`), multi-glyph combinations, unicode single glyphs, comment
//! interplay, and the owner's exact config shape. Owner mandate: depth
//! stress-test ANY single glyph so this bug class never returns.

#![cfg(test)]

use crate::configfile::parse_config_text;

/// Helper: parse `[charset-custom.s] set = "<pool>"` (+ optional trailing
/// comment) and assert the pool is stored verbatim with zero malformed
/// lines and zero unknown keys.
fn assert_pool_parses_verbatim(pool: &str, trailing_comment: Option<&str>) {
    let line = match trailing_comment {
        Some(c) => format!("set = \"{pool}\"  {c}"),
        None => format!("set = \"{pool}\""),
    };
    let content = format!("[charset-custom.s]\n{line}\n");
    let parsed = parse_config_text(&content);
    assert!(
        parsed.malformed_lines.is_empty(),
        "pool {pool:?} (comment {trailing_comment:?}): malformed: {:?}",
        parsed.malformed_lines
    );
    assert!(
        parsed.unknown_keys.is_empty(),
        "pool {pool:?}: unknown keys: {:?}",
        parsed.unknown_keys
    );
    assert_eq!(
        parsed
            .values
            .get("charset-custom.s.set")
            .map(String::as_str),
        Some(pool),
        "pool {pool:?} must be stored verbatim"
    );
}

#[test]
fn owners_exact_repro_quoted_bracket_with_hostile_comment_parses() {
    // The EXACT line from the owner's config (2026-08-30): the trailing
    // comment contains BOTH `']'` and `'#'` — every scanning heuristic
    // has something to trip on. Before bug #19 this single line failed
    // strict startup validation with:
    //   "malformed line(s): 'set = "["' (expected 'key = value' syntax)"
    // even though the line is perfectly valid.
    let parsed = parse_config_text(
        "[charset-custom.test]\nset = \"[\"  # ERROR: array never closed (missing ']') or '#' truncated the value\n",
    );
    assert!(
        parsed.malformed_lines.is_empty(),
        "got: {:?}",
        parsed.malformed_lines
    );
    assert!(
        parsed.unknown_keys.is_empty(),
        "got: {:?}",
        parsed.unknown_keys
    );
    assert_eq!(
        parsed
            .values
            .get("charset-custom.test.set")
            .map(String::as_str),
        Some("[")
    );
}

#[test]
fn quoted_bracket_does_not_eat_following_lines() {
    // Before the fix, `set = "["` entered the multi-line array consumer
    // and absorbed every following line up to a `]` or a section header —
    // silently corrupting sibling keys. After the fix, the line is stored
    // verbatim and everything after it parses normally (a root key placed
    // BEFORE the block and a full section placed AFTER both survive).
    let parsed = parse_config_text(
        "msg-fill-style = bubble\n\
         \n\
         [charset-custom.test]\n\
         set = \"[\"\n\
         \n\
         [colors-custom.zen]\n\
         bg = \"#0a0a12\"\n",
    );
    assert!(
        parsed.malformed_lines.is_empty(),
        "got: {:?}",
        parsed.malformed_lines
    );
    assert!(
        parsed.unknown_keys.is_empty(),
        "got: {:?}",
        parsed.unknown_keys
    );
    assert_eq!(
        parsed.values.get("msg-fill-style").map(String::as_str),
        Some("bubble")
    );
    assert_eq!(
        parsed
            .values
            .get("charset-custom.test.set")
            .map(String::as_str),
        Some("[")
    );
    assert_eq!(
        parsed
            .values
            .get("colors-custom.zen.bg")
            .map(String::as_str),
        Some("#0a0a12")
    );
}

#[test]
fn every_ascii_punctuation_single_glyph_parses() {
    // Every ASCII punctuation character as the SOLE glyph of a quoted
    // value. The bracket/hash/quote family used to reject or corrupt;
    // the rest of the sweep locks them so a future refactor of the
    // comment stripper or quote stripper cannot regress any of them.
    // (`"` is included deliberately: `set = """` stores a lone quote at
    // the parser layer — the charset-custom validator rejects it later
    // with a clear error; see charset_custom tests.)
    let glyphs = r##"!"#$%&'()*+,-./:;<=>?@[\]^_`{|}~"##;
    for ch in glyphs.chars() {
        assert_pool_parses_verbatim(&ch.to_string(), None);
    }
}

#[test]
fn bracket_family_pools_parse_verbatim() {
    // Pools built ONLY from brackets — the adversarial cases for the
    // array-detection logic. All quoted, all verbatim, none of them may
    // enter the multi-line array consumer.
    for pool in ["[]", "[][]", "[a]", "[[]]", "][", "[#]", "[=]", "[,]"] {
        assert_pool_parses_verbatim(pool, None);
    }
}

#[test]
fn quoted_hash_glyphs_are_values_not_comments() {
    // `#` inside quotes must never start a comment (strip_inline_comment
    // is quote-aware) — including a pool that is ONLY hash marks.
    for pool in ["#", "##", "#=#", "a#b"] {
        assert_pool_parses_verbatim(pool, None);
    }
    // ...and a quoted hash glyph followed by a REAL trailing comment.
    assert_pool_parses_verbatim("#", Some("# my favorite glyph"));
}

#[test]
fn quoted_equals_glyphs_survive_first_equals_split() {
    // split_once('=') splits at the FIRST '='; everything after belongs
    // to the value. `=`-only pools and embedded equals must parse.
    for pool in ["=", "==", "a=b", "key=value"] {
        assert_pool_parses_verbatim(pool, None);
    }
}

#[test]
fn quoted_bracket_with_assorted_trailing_comments() {
    // The owner's bug was maximally hostile comments; this sweep covers
    // the realistic comment shapes a user writes next to a bracket glyph.
    for comment in [
        "# plain",
        "# ]",
        "# [",
        "# ']'",
        "# \"quoted\" words",
        "# don't panic",
        "# a ']' and a '#' inside",
    ] {
        assert_pool_parses_verbatim("[", Some(comment));
    }
}

#[test]
fn unicode_single_glyphs_parse_verbatim() {
    // Single unicode glyphs — including the new `minimal` preset glyph
    // (nabla), the owner's preview pair, and one WIDE char (CJK 漢):
    // the parser layer stores ALL of them verbatim (width filtering is
    // charset-custom's job, not the parser's).
    for pool in [
        "∇", "∀", "∂", "∑", "∫", "√", "π", "∆", "Ω", "±", "×", "÷", "░", "█", "─", "○", "∀∇", "漢",
    ] {
        assert_pool_parses_verbatim(pool, None);
    }
}

#[test]
fn unquoted_open_bracket_still_reports_unclosed_array() {
    // Contrast lock (bug #7 semantics preserved): WITHOUT quotes, a `[`
    // value IS an array opener — the genuine "array never closed"
    // rejection must keep firing. The bug #19 fix narrows the array
    // branches to UNQUOTED values only.
    let parsed = parse_config_text("[charset-custom.s]\nset = [\n");
    assert!(
        !parsed.malformed_lines.is_empty(),
        "unquoted unterminated array must stay malformed"
    );
    assert!(!parsed.values.contains_key("charset-custom.s.set"));
}

#[test]
fn quoted_bracket_pool_loads_through_charset_custom_layer() {
    // End-to-end: the stored `[` value must survive the charset-custom
    // validation layer as a single-glyph pool (it is a width-1 char).
    // This is the full path the owner exercised with `-C test`.
    let parsed = parse_config_text("[charset-custom.test]\nset = \"[\"\n");
    let cfg = parsed.values;
    let pool = crate::charset_custom::load_custom_charset(&cfg, "test")
        .unwrap_or_else(|e| panic!("single-bracket pool must load: {e}"));
    assert_eq!(pool, vec!['[']);
}

#[test]
fn owners_full_config_shape_end_to_end() {
    // The owner's actual config shape: a top-level key + TWO
    // [charset-custom.test] blocks. Duplicate section headers are
    // last-wins in the forgiving parser (the second block's `set`
    // overwrites the first). Before bug #19 the second block's quoted
    // `[` killed strict startup validation for the ENTIRE file.
    let content = "msg-fill-style = bubble\n\
                   \n\
                   [charset-custom.test]\n\
                   set = \"∀∇\"\n\
                   \n\
                   [charset-custom.test]\n\
                   set = \"[\"\n";
    let parsed = parse_config_text(content);
    assert!(
        parsed.malformed_lines.is_empty(),
        "got: {:?}",
        parsed.malformed_lines
    );
    // Duplicate section: last writer wins.
    assert_eq!(
        parsed
            .values
            .get("charset-custom.test.set")
            .map(String::as_str),
        Some("[")
    );
    assert_eq!(
        parsed.values.get("msg-fill-style").map(String::as_str),
        Some("bubble")
    );
    // And the winning pool loads through the charset-custom layer.
    let pool = crate::charset_custom::load_custom_charset(&parsed.values, "test")
        .unwrap_or_else(|e| panic!("owner config must load: {e}"));
    assert_eq!(pool, vec!['[']);
}

#[test]
fn quoted_value_still_strips_outer_quotes_for_plain_strings() {
    // Regression guard for the refactor itself: plain quoted values keep
    // the long-standing Option-1 quote-stripping behavior (both quoted
    // and unquoted forms produce the same stored value).
    for (line, expected) in [
        ("intro = \"logo\"", "logo"),
        ("intro = logo", "logo"),
        ("charset = \"minimal\"", "minimal"),
    ] {
        let parsed = parse_config_text(&format!("{line}\n"));
        assert!(
            parsed.malformed_lines.is_empty(),
            "line {line:?}: {:?}",
            parsed.malformed_lines
        );
        assert_eq!(
            parsed
                .values
                .get("intro")
                .or_else(|| parsed.values.get("charset"))
                .map(String::as_str),
            Some(expected),
            "line {line:?} must store {expected:?}"
        );
    }
}
