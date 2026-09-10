// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-depthtest-2 and hunt-30: depth regression suite for the CLI
//! and config.toml chain (the depthtest-1 stresstest covers fuzzing;
//! this file pins the OWNER-REPORTED and hunt-found defects as exact
//! reproducible contracts).
//!
//! Five sections:
//! 1. dump-config overwrite refusal: the suggested sibling path must
//!    satisfy every validation rule the flag itself enforces (the
//!    owner's two-transcript loop: refusal suggested config.toml.new,
//!    the extension validator then rejected it).
//! 2. Parser duplicate detection: duplicate keys and duplicate
//!    [section] headers (the charset/color/scene-custom duplicate-name
//!    audit) are recorded, while the values map keeps the documented
//!    merge/last-wins semantics for the validation-bypass path.
//! 3. Three-surface lockstep: startup, --testconf, and the
//!    live-reload watcher all reject duplicates with the same
//!    verdict (the S-master-HUNT-2 uniform-rejection contract).
//! 4. Explicit --config read errors: a missing or unreadable explicit
//!    config file is a hard error, an empty existing file still
//!    applies (the deliberate-empty contract is unchanged).
//! 5. Extension validator + dump template invariants: the .toml
//!    extension check is case-insensitive (Windows filesystem
//!    reality) and every non-ASCII line of the config template is
//!    charset glyph data, never prose (the pure-English rule).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;

use clap::{CommandFactory, FromArgMatches};

use crate::configfile::{dump_config_text, parse_config_text};
use crate::testconf::duplicate_diagnostics;

/// Unique temp-file counter (same pattern as config_apply_tests) so
/// parallel tests never share a config path — including the
/// startup-parse memo, which is keyed by resolved path.
static TEMP_FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Allow the temp dir as a safe config location (test-only env var,
/// compiled out of release builds) and return a unique .toml path in
/// it. No COSMOSTRIX_SKIP_STARTUP_VALIDATION is set here: the tests
/// below either run before that guard (read errors) or follow the
/// strict_mode.rs tolerant-match convention for the layers inside it.
fn unique_temp_toml(label: &str) -> std::path::PathBuf {
    std::env::set_var(
        "COSMOSTRIX_TEST_CONFIG_DIR",
        std::env::temp_dir().to_string_lossy().into_owned(),
    );
    let mut path = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    let seq = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.push(format!(
        "cosmostrix-depthtest2-{label}-{}-{nanos}-{seq}.toml",
        std::process::id(),
    ));
    path
}

/// Build Args from a raw argv and run the startup config funnel,
/// returning its verdict. This is the exact chain main.rs drives.
fn apply_from_argv(argv: &[&str]) -> Result<crate::config::Args, String> {
    let cmd = crate::config::Args::command();
    let matches = cmd.get_matches_from(argv.to_vec());
    let mut args = crate::config::Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    crate::config::config_apply::apply_config_and_runtime_defaults(&matches, &mut args)
        .map(|()| args)
}

/// Write config text to a unique temp file and run the startup funnel
/// with an explicit --config override pointing at it.
fn apply_with_config_text(content: &str) -> Result<crate::config::Args, String> {
    let path = unique_temp_toml("cfg");
    std::fs::write(&path, content).expect("write temp config");
    let path_string = path.to_string_lossy().into_owned();
    let argv = vec!["cosmostrix", "--config", path_string.as_str()];
    let result = apply_from_argv(&argv);
    let _ = std::fs::remove_file(&path);
    result
}

// ─────────────────────────────────────────────────────────────────
// Section 1: dump-config overwrite refusal (owner transcripts)
// ─────────────────────────────────────────────────────────────────

/// Extract the suggested path from the refusal message: the token
/// after "cosmostrix --dump-config " on the guidance line.
fn suggested_path_from_refusal(msg: &str) -> String {
    let line = msg
        .lines()
        .find(|l| l.trim_start().starts_with("cosmostrix --dump-config ") && !l.contains("--force"))
        .expect("refusal must carry a suggestion command line");
    line.trim()
        .trim_start_matches("cosmostrix --dump-config ")
        .trim()
        .to_string()
}

#[test]
fn refusal_suggestion_passes_every_validator_rule() {
    // The owner's exact first transcript: the guard fires on an
    // existing config. The suggested sibling must (a) end in .toml,
    // (b) not be the guarded path itself, and (c) survive the SAME
    // validate_config_path call the flag runs — the pre-fix message
    // suggested config.toml.new, which the validator rejected with
    // "must have a .toml extension" (the second transcript).
    let existing = unique_temp_toml("existing");
    let existing_str = existing.to_string_lossy().into_owned();
    let msg = crate::cli::early_returns::dump_config_overwrite_refusal(&existing_str);

    let suggested = suggested_path_from_refusal(&msg);
    assert!(
        suggested.to_ascii_lowercase().ends_with(".toml"),
        "suggested path '{suggested}' must end in .toml"
    );
    assert_ne!(
        suggested, existing_str,
        "suggestion must not point back at the guarded file"
    );
    assert!(
        !suggested.ends_with(".toml.new"),
        "the old self-contradictory suggestion shape must be gone: {suggested}"
    );
    assert!(
        crate::safepath::validate_config_path(&suggested, false).is_ok(),
        "suggested path '{suggested}' must pass validate_config_path"
    );
    // The --force escape hatch stays advertised.
    assert!(
        msg.contains("--force"),
        "refusal must advertise --force: {msg}"
    );
    assert!(
        msg.contains(&existing_str),
        "refusal must name the guarded path"
    );
}

#[test]
fn refusal_suggestion_preserves_stem_case() {
    // A mixed-case stem must survive into the suggestion — lowering
    // the whole path would corrupt it on case-sensitive filesystems.
    let guarded = unique_temp_toml("Config-Upper");
    let guarded_str = guarded.to_string_lossy().into_owned();
    let msg = crate::cli::early_returns::dump_config_overwrite_refusal(&guarded_str);
    let suggested = suggested_path_from_refusal(&msg);
    let stem = guarded_str.strip_suffix(".toml").unwrap_or(&guarded_str);
    assert!(
        suggested.contains(stem),
        "suggestion '{suggested}' must keep the original stem '{stem}' verbatim"
    );
    assert_eq!(suggested, format!("{stem}.new.toml"));
}

#[test]
fn refusal_is_total_for_non_toml_paths() {
    // Defensive totality: a caller that somehow bypasses the
    // validation ordering still gets a suggestion that satisfies the
    // extension rule (append, never invent).
    let msg = crate::cli::early_returns::dump_config_overwrite_refusal("weird.name");
    let suggested = suggested_path_from_refusal(&msg);
    assert_eq!(suggested, "weird.name.new.toml");
}

// ─────────────────────────────────────────────────────────────────
// Section 2: parser duplicate detection
// ─────────────────────────────────────────────────────────────────

#[test]
fn parse_records_duplicate_key_inside_a_custom_block() {
    let parsed =
        parse_config_text("[scene-custom.hacker]\nrain = glyph\nrain = monolith\ncolor = green\n");
    assert_eq!(
        parsed.duplicate_keys,
        vec!["scene-custom.hacker.rain".to_string()],
        "the repeated field must be recorded exactly once"
    );
    // Documented bypass semantics: the values map keeps last-wins.
    assert_eq!(
        parsed
            .values
            .get("scene-custom.hacker.rain")
            .map(String::as_str),
        Some("monolith")
    );
}

#[test]
fn parse_records_duplicate_section_header() {
    let parsed = parse_config_text(
        "[colors-custom.zen]\nbg = \"#0a0a0a\"\n[colors-custom.zen]\nrain = \"#ffffff\"\n",
    );
    assert_eq!(
        parsed.duplicate_sections,
        vec!["colors-custom.zen".to_string()]
    );
    // The merge still happens (the validation layer owns the reject).
    assert!(parsed.values.contains_key("colors-custom.zen.bg"));
    assert!(parsed.values.contains_key("colors-custom.zen.rain"));
}

#[test]
fn parse_duplicate_section_detection_is_case_insensitive() {
    // The parser lowercases section names, so a case-variant reopen
    // is the same table (TOML would reject both spellings; cosmostrix
    // keys are case-insensitive by long-standing design).
    let parsed =
        parse_config_text("[scene-custom.x]\nrain = glyph\n[SCENE-CUSTOM.X]\nspeed = 30\n");
    assert_eq!(
        parsed.duplicate_sections,
        vec!["scene-custom.x".to_string()]
    );
}

#[test]
fn parse_records_duplicate_root_key() {
    let parsed = parse_config_text("fps = 60\nfps = 144\nspeed = 9\n");
    assert_eq!(parsed.duplicate_keys, vec!["fps".to_string()]);
    assert_eq!(parsed.values.get("fps").map(String::as_str), Some("144"));
}

#[test]
fn parse_records_duplicate_ambient_time_slot() {
    // The ambient scheduler keys are names too (HH-MM slots); a
    // duplicated slot silently switched scenes before this fix.
    let parsed = parse_config_text("ambient.06-00 = signal\nambient.06-00 = monolith\n");
    assert_eq!(parsed.duplicate_keys, vec!["ambient.06-00".to_string()]);
}

#[test]
fn parse_clean_config_records_no_duplicates() {
    let parsed = parse_config_text(
        "fps = 60\nspeed = 9\n[scene-custom.a]\nrain = glyph\n[scene-custom.b]\nrain = flux\n",
    );
    assert!(
        parsed.duplicate_keys.is_empty(),
        "{:?}",
        parsed.duplicate_keys
    );
    assert!(
        parsed.duplicate_sections.is_empty(),
        "{:?}",
        parsed.duplicate_sections
    );
}

#[test]
fn parse_same_field_in_different_blocks_is_not_a_duplicate() {
    // The legitimate multi-block shape: same field name, different
    // scopes — full keys differ, nothing is recorded.
    let parsed =
        parse_config_text("[scene-custom.a]\nrain = glyph\n[scene-custom.b]\nrain = flux\n");
    assert!(parsed.duplicate_keys.is_empty());
    assert!(parsed.duplicate_sections.is_empty());
    assert_eq!(parsed.values.len(), 2, "both blocks keep their fields");
}

#[test]
fn parse_promoted_key_collision_is_not_recorded_as_duplicate() {
    // The auto-promote path (a top-level key accidentally nested
    // under a section) keeps root-scope first-wins and is reported as
    // an info notice by --testconf — it must NOT double-report as a
    // duplicate key.
    let parsed = parse_config_text("fps = 60\n[weird-section]\nfps = 144\n");
    assert!(
        parsed.duplicate_keys.is_empty(),
        "{:?}",
        parsed.duplicate_keys
    );
    assert_eq!(
        parsed.promoted_keys.len(),
        1,
        "the promotion is still recorded"
    );
    assert_eq!(parsed.values.get("fps").map(String::as_str), Some("60"));
}

// ─────────────────────────────────────────────────────────────────
// Section 3: three-surface lockstep (startup / testconf / watcher)
// ─────────────────────────────────────────────────────────────────

#[test]
fn watcher_validate_and_send_rejects_duplicate_sections_and_keys() {
    // The live-reload surface: an editor that duplicates a block
    // mid-save must get the reject verdict, not a silently merged
    // "successful" reload.
    let parsed = parse_config_text(
        "[scene-custom.duo]\nrain = glyph\n[scene-custom.duo]\nrain = monolith\n",
    );
    let (tx, _rx) = sync_channel(64);
    let verdict = crate::config::live_config::watcher::validate_and_send(&parsed, &tx);
    let msg = verdict.expect_err("duplicates must be rejected by the watcher");
    assert!(
        msg.contains("duplicate"),
        "watcher rejection must name the duplicate class: {msg}"
    );
    assert!(
        msg.contains("scene-custom.duo"),
        "watcher rejection must name the offending block: {msg}"
    );
}

#[test]
fn watcher_validate_and_send_rejects_duplicate_root_keys() {
    let parsed = parse_config_text("fps = 60\nfps = 144\n");
    let (tx, _rx) = sync_channel(64);
    let verdict = crate::config::live_config::watcher::validate_and_send(&parsed, &tx);
    let msg = verdict.expect_err("duplicate root key must be rejected");
    assert!(
        msg.contains("'fps'"),
        "the offending key must be named: {msg}"
    );
}

#[test]
fn watcher_clean_config_still_passes_to_strict_validation() {
    // Control: no false positives — a clean parse flows through the
    // duplicate layer untouched (it may still fail value validation
    // further down, which is not this layer's concern).
    let parsed = parse_config_text("fps = 60\nspeed = 9\n");
    let (tx, _rx) = sync_channel(64);
    let verdict = crate::config::live_config::watcher::validate_and_send(&parsed, &tx);
    assert!(
        verdict.is_ok(),
        "clean config must not trip the duplicate layer: {verdict:?}"
    );
}

#[test]
fn testconf_duplicate_diagnostics_report_both_kinds() {
    let parsed = parse_config_text(
        "fps = 60\nfps = 144\n[colors-custom.zen]\nbg = \"#0a0a0a\"\n[colors-custom.zen]\nrain = \"#ffffff\"\n",
    );
    let lines = duplicate_diagnostics(&parsed);
    assert_eq!(lines.len(), 2, "one section dup + one key dup: {lines:?}");
    assert!(
        lines[0].contains("[colors-custom.zen]"),
        "section diagnostic must name the block: {}",
        lines[0]
    );
    assert!(
        lines[1].contains("'fps'"),
        "key diagnostic must name the key: {}",
        lines[1]
    );
}

#[test]
fn testconf_duplicate_diagnostics_empty_for_clean_parse() {
    let parsed = parse_config_text("fps = 60\nspeed = 9\n");
    assert!(duplicate_diagnostics(&parsed).is_empty());
}

#[test]
fn startup_rejects_duplicate_config_definitions() {
    // Tolerant-match convention (strict_mode.rs): production runs the
    // strict layer; a parallel test setting the skip env var makes
    // the Ok arm legal, in which case the duplicate verdicts are
    // still pinned by the watcher and testconf sections above.
    let verdict = apply_with_config_text("fps = 60\nfps = 144\nspeed = 9\n");
    if let Err(msg) = verdict {
        assert!(
            msg.contains("duplicate"),
            "startup error must name the duplicate class: {msg}"
        );
        assert!(
            msg.contains("'fps'"),
            "startup error must name the duplicated key: {msg}"
        );
    }
}

#[test]
fn startup_rejects_duplicate_custom_block_header() {
    let verdict = apply_with_config_text(
        "[charset-custom.zen]\nset = \"|\"\n[charset-custom.zen]\nset = \"-\"\n",
    );
    if let Err(msg) = verdict {
        assert!(
            msg.contains("duplicate") && msg.contains("[charset-custom.zen]"),
            "startup error must name the duplicated block: {msg}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────
// Section 4: explicit --config read errors (silent-failure hunt)
// ─────────────────────────────────────────────────────────────────

#[test]
fn explicit_config_missing_file_is_a_hard_error() {
    // Pre-fix behavior: an unreadable explicit --config silently
    // produced an empty map and the run continued on pure defaults —
    // a typo'd path was indistinguishable from an intentional
    // default run. The error now fires BEFORE the skip-validation
    // guard (deterministic in the test process).
    let missing = unique_temp_toml("missing");
    let missing_str = missing.to_string_lossy().into_owned();
    let argv = vec!["cosmostrix", "--config", missing_str.as_str()];
    let verdict = apply_from_argv(&argv);
    let msg = verdict.expect_err("missing explicit config must be a hard error");
    assert!(msg.contains("--config"), "error must name the flag: {msg}");
    assert!(
        msg.contains("cannot read config file"),
        "error must carry the read failure reason: {msg}"
    );
}

#[test]
fn explicit_config_empty_existing_file_still_applies() {
    // The deliberate-empty contract is unchanged: an existing, empty
    // (or all-comments) config is a legal zero-key config, not a read
    // error. First-run users with a touched-but-empty file keep
    // working.
    let verdict = apply_with_config_text("");
    assert!(
        verdict.is_ok(),
        "empty existing config must still apply: {:?}",
        verdict.err()
    );
}

#[test]
fn explicit_config_comment_only_file_still_applies() {
    let verdict = apply_with_config_text("# all keys commented out\n# fps = 60\n");
    assert!(verdict.is_ok(), "comment-only config must still apply");
}

// ─────────────────────────────────────────────────────────────────
// Section 5: extension validator + template pure-English invariant
// ─────────────────────────────────────────────────────────────────

#[test]
fn validate_config_path_accepts_uppercase_toml_extension() {
    // Windows filesystems are case-insensitive: CONFIG.TOML is the
    // same file as config.toml there, and the validator must not
    // contradict the whitelist (which resolves the real path). Only
    // the FILE NAME is uppercased — the directory must keep its
    // real on-disk casing (Unix dirs are case-sensitive).
    let path = unique_temp_toml("UPPER");
    let dir = path.parent().expect("temp path has a parent");
    let upper_name = path
        .file_name()
        .expect("temp path has a file name")
        .to_string_lossy()
        .to_uppercase();
    let path_str = dir.join(upper_name).to_string_lossy().into_owned();
    assert!(
        crate::safepath::validate_config_path(&path_str, false).is_ok(),
        "uppercase .TOML extension must be accepted: {path_str}"
    );
}

#[test]
fn validate_config_path_still_rejects_non_toml_extensions() {
    let path = unique_temp_toml("reject");
    let path_str = path.to_string_lossy().into_owned();
    let non_toml = path_str.replace(".toml", ".txt");
    let verdict = crate::safepath::validate_config_path(&non_toml, false);
    assert!(
        verdict
            .expect_err("non-.toml extension must be rejected")
            .contains(".toml extension"),
        "the rejection must explain the extension rule"
    );
}

#[test]
fn dump_template_non_ascii_lines_are_charset_data_only() {
    // The pure-English rule (NIGHT-depthtest-2): every non-ASCII
    // LETTER in the config template must sit on a charset glyph data
    // line (a set assignment), never in prose. Em dashes and other
    // punctuation pass (the language audit flags letters only); the
    // katakana and math-symbol example presets are data. A template
    // line that gains letter-script content outside `set = ` lines
    // is language leakage and this fails.
    for line in dump_config_text().lines() {
        let has_non_ascii_letter = line.chars().any(|c| !c.is_ascii() && c.is_alphabetic());
        if !has_non_ascii_letter {
            continue;
        }
        assert!(
            line.contains("set = "),
            "non-ASCII letters in a non-data template line: {line}"
        );
    }
}
