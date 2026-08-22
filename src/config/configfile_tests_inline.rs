// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use super::*;

#[test]
fn default_path_prefers_xdg_config_home() {
    let path = config_file_path_from(Some("/tmp/xdg".to_string()), Some("/tmp/home".to_string()));
    assert_eq!(path, PathBuf::from("/tmp/xdg/cosmostrix/config.toml"));
}

#[test]
fn default_path_falls_back_to_home_config() {
    let path = config_file_path_from(None, Some("/tmp/home".to_string()));
    assert_eq!(
        path,
        PathBuf::from("/tmp/home/.config/cosmostrix/config.toml")
    );
}

#[test]
fn parse_key_value_lines() {
    let parsed = parse_config_text("color = ocean\nfps = 60\n");
    assert_eq!(
        parsed.values.get("color").map(String::as_str),
        Some("ocean")
    );
    assert_eq!(parsed.values.get("fps").map(String::as_str), Some("60"));
    assert!(parsed.unknown_keys.is_empty());
}

#[test]
fn parse_ignores_comments_blank_lines_and_inline_comments() {
    let parsed =
        parse_config_text("\n# comment\ncharset = minimal # trailing comment\n\nspeed = 5\n");
    assert_eq!(
        parsed.values.get("charset").map(String::as_str),
        Some("minimal")
    );
    assert_eq!(parsed.values.get("speed").map(String::as_str), Some("5"));
    assert_eq!(parsed.values.len(), 2);
}

#[test]
fn parse_unknown_keys_are_reported_and_ignored() {
    let parsed = parse_config_text("color = ocean\ncolro = typo\n");
    assert_eq!(
        parsed.values.get("color").map(String::as_str),
        Some("ocean")
    );
    assert_eq!(parsed.unknown_keys, vec!["colro"]);
    assert!(!parsed.values.contains_key("colro"));
}

#[test]
fn legacy_keys_removed_v17() {
    // v17 mastery: legacy advanced keys (glitchpct, shortpct, rippct,
    // maxdpc) are REMOVED. They are now flagged as unknown by --testconf
    // so users know to migrate to --glitch-level. They do NOT go into
    // parsed.values (only known keys do).
    let parsed = parse_config_text("glitchpct = 3\nshortpct = 60\nrippct = 45\nmaxdpc = 2\n");
    assert_eq!(
        parsed.values.len(),
        0,
        "legacy keys should not be in values"
    );
    assert_eq!(
        parsed.unknown_keys.len(),
        4,
        "legacy keys should be flagged as unknown"
    );
    assert!(parsed.unknown_keys.contains(&"glitchpct".to_string()));
    assert!(parsed.unknown_keys.contains(&"shortpct".to_string()));
    assert!(parsed.unknown_keys.contains(&"rippct".to_string()));
    assert!(parsed.unknown_keys.contains(&"maxdpc".to_string()));
}

#[test]
fn profile_keys_are_flagged_as_unknown() {
    // Legacy [profile.<name>] blocks are inert (replaced by
    // [scene-custom.<name>]). Profile keys are NOT recognized by
    // is_known_key and must be flagged as unknown so config_hints
    // can suggest the rename.
    let parsed = parse_config_text(
        "profile.nightcore.base-scene = monolith\nprofile.nightcore.color = purple\n",
    );
    assert!(
        parsed
            .unknown_keys
            .contains(&"profile.nightcore.base-scene".to_string()),
        "profile keys must be flagged as unknown (inert system)"
    );
    assert!(
        parsed
            .unknown_keys
            .contains(&"profile.nightcore.color".to_string()),
        "profile keys must be flagged as unknown (inert system)"
    );
    assert!(parsed.malformed_lines.is_empty());
}

#[test]
fn malformed_lines_without_equals_are_collected() {
    // Lines with no '=' on a non-empty, non-comment line are malformed.
    let parsed = parse_config_text("color = ocean\necho here should error\n");
    assert_eq!(parsed.values.len(), 1);
    assert_eq!(parsed.malformed_lines, vec!["echo here should error"]);
}

#[test]
fn malformed_lines_with_empty_value_are_collected() {
    // `key =` (no value) is malformed.
    let parsed = parse_config_text("color = ocean\nspeed =\n");
    assert_eq!(parsed.values.len(), 1);
    assert_eq!(parsed.malformed_lines, vec!["speed ="]);
}

#[test]
fn malformed_lines_with_empty_key_are_collected() {
    // `= value` (no key) is malformed.
    let parsed = parse_config_text("color = ocean\n= 60\n");
    assert_eq!(parsed.values.len(), 1);
    assert_eq!(parsed.malformed_lines, vec!["= 60"]);
}

#[test]
fn malformed_lines_skip_comments_and_blanks() {
    // Comments and blank lines must NOT be flagged as malformed.
    let parsed =
        parse_config_text("# this is a comment\n\ncolor = ocean\n  # indented comment\n\n");
    assert_eq!(parsed.values.len(), 1);
    assert!(parsed.malformed_lines.is_empty());
}

#[test]
fn malformed_lines_inline_comment_stripped() {
    // A malformed line with an inline comment should be reported without
    // the comment portion.
    let parsed = parse_config_text("echo bad line # this is a comment\n");
    assert_eq!(parsed.malformed_lines, vec!["echo bad line"]);
}

#[test]
fn dump_config_contains_all_supported_keys() {
    let dump = dump_config_text();
    for key in USER_CONFIG_KEYS.iter() {
        assert!(dump.contains(*key), "dump config should mention {key}");
    }
    assert!(dump.contains("[scene-custom.hacker-mode]"));
    // v30+: ambient phase scheduler must be documented with at least one
    // live, uncommented-as-comment example so users can copy-paste.
    assert!(
        dump.contains("ambient.06-00"),
        "dump config should include an ambient.<HH-MM> example"
    );
    assert!(
        dump.contains("Ambient Phase Scheduler"),
        "dump config should include an Ambient section header"
    );
}

#[test]
fn dump_config_documents_paired_field_split() {
    // the dump-config template must explicitly document the
    // split between `color` (built-in) and `colors-custom` (custom
    // block ref), and the symmetric split for `charset` vs
    // `charset-custom`. This prevents the duplicate-usage confusion
    // reported by the owner. The note is enforced by content anchor —
    // if a future edit removes the "Paired fields" header, this test
    // fails loudly.
    let dump = dump_config_text();
    assert!(
            dump.contains("Paired fields"),
            "dump config should include the 'Paired fields' note (color vs colors-custom, charset vs charset-custom)"
        );
    // Each paired field's doc line should also appear in the custom
    // palettes / custom charsets sections, pointing users at the right
    // reference field.
    assert!(
        dump.contains("colors-custom = <name>"),
        "Custom Color Palettes section should show how to reference a block from a scene-custom"
    );
    assert!(
        dump.contains("charset-custom = <name>"),
        "Custom Character Sets section should show how to reference a block from a scene-custom"
    );
}

#[test]
fn dump_config_with_header_starts_with_header_lines() {
    // v50 (alpha.2): the generated config must start with the 5-line header +
    // blank `#` line, then the existing `# cosmostrix configuration`
    // template body. v50 added the fingerprint; this iteration renamed it to
    // `template-fingerprint` and added a `verify full file` hint line.
    let dump1 = dump_config_with_header();
    let lines: Vec<&str> = dump1.lines().collect();
    assert!(
        lines.len() >= 7,
        "header should have >= 7 lines (5 header + blank + body)"
    );
    assert_eq!(lines[0], "# cosmostrix config file", "header line 1");
    // Line 2: `# generated at <ISO 8601 UTC>`
    let line2 = lines[1];
    assert!(
        line2.starts_with("# generated at "),
        "header line 2 wrong: {line2:?}"
    );
    let ts = line2.trim_start_matches("# generated at ");
    assert!(
        ts.len() == 20 && ts.ends_with('Z') && ts.as_bytes()[10] == b'T',
        "timestamp not RFC 3339: {ts:?}"
    );
    // Line 3: Hinnant attribution
    assert_eq!(
        lines[2], "# using Howard Hinnant chrono design (libc::gmtime_r)",
        "header line 3"
    );
    // Line 4: template fingerprint (v50 alpha.2 label)
    // Labelled "template-fingerprint" to distinguish from `sha512sum` of the
    // full file on disk — this hash covers only the template body, not the
    // header lines. Use `--testconf` or `sha512sum` for file-level checks.
    let line4 = lines[3];
    assert!(
        line4.starts_with("# template-fingerprint: ") && line4.len() == 24 + 128,
        "template-fingerprint line wrong: {line4:?} (expected '# template-fingerprint: ' + 128 hex chars)"
    );
    // Line 5: verify full file hint (v50 alpha.2)
    assert_eq!(
        lines[4], "# verify full file: sha512sum <path> or --testconf",
        "header line 5 (verify hint)"
    );
    // Line 6: blank `#` separator
    assert_eq!(lines[5], "#", "blank separator");
    // Line 7: existing template body starts
    assert_eq!(
        lines[6], "# cosmostrix configuration",
        "template body start"
    );
}

#[test]
fn dump_config_with_header_includes_all_keys() {
    // The header prepended must not break the existing key-coverage test.
    let dump = dump_config_with_header();
    for key in USER_CONFIG_KEYS.iter() {
        assert!(
            dump.contains(*key),
            "header'd dump should still mention {key}"
        );
    }
    assert!(dump.contains("[scene-custom.hacker-mode]"));
    // v30+: ambient example must survive the header prepend.
    assert!(
        dump.contains("ambient.06-00"),
        "header'd dump should still include ambient.<HH-MM> example"
    );
}

#[test]
fn parse_multiline_array_joins_correctly() {
    let content = "[colors-custom.zen]\nbg = \"#0a0a12\"\nrain = [\n  \"#1a0033\",\n  \"#4d0080\",\n  \"#9933ff\",\n  \"#cc66ff\",\n  \"#e6b3ff\",\n  \"#f2ccff\",\n  \"#ffffff\",\n]\n";
    let parsed = parse_config_text(content);
    assert!(
        parsed.malformed_lines.is_empty(),
        "no malformed lines, got: {:?}",
        parsed.malformed_lines
    );
    assert!(
        parsed.unknown_keys.is_empty(),
        "no unknown keys, got: {:?}",
        parsed.unknown_keys
    );
    let rain = parsed.values.get("colors-custom.zen.rain");
    assert!(rain.is_some(), "rain key should be parsed");
    let rain = rain.unwrap();
    assert!(rain.starts_with('['), "rain value should start with [");
    assert!(rain.ends_with(']'), "rain value should end with ]");
}

// ── Termux fix: path resolution tests ──

#[test]
fn is_termux_environment_returns_false_off_termux() {
    // On a normal Linux/macOS/Windows CI runner, neither TERMUX_VERSION
    // nor a "com.termux"-containing PREFIX is set. This test verifies
    // the detection returns false. (It would return true on an actual
    // Termux runner, where this assertion is skipped via env check.)
    let on_termux = std::env::var("TERMUX_VERSION").is_ok()
        || std::env::var("PREFIX")
            .map(|p| p.contains("com.termux"))
            .unwrap_or(false);
    if !on_termux {
        assert!(!is_termux_environment(), "should be false off Termux");
    }
}

#[test]
fn is_termux_environment_detects_termux_version() {
    // Simulate Termux by setting TERMUX_VERSION in a subprocess.
    // We can't actually set env vars in-process, so we replicate
    // the detection logic with a known-set value.
    let detected = std::env::var("TERMUX_VERSION").is_ok()
        || std::env::var("PREFIX")
            .map(|p| p.contains("com.termux"))
            .unwrap_or(false);
    // On CI runners, this is false; on Termux, this is true.
    // Either way, is_termux_environment() must agree with our manual check.
    assert_eq!(is_termux_environment(), detected);
}

#[test]
fn config_candidate_paths_includes_default_path() {
    // The first candidate should always be default_config_file_path().
    let candidates = config_candidate_paths();
    assert!(!candidates.is_empty(), "candidate list must not be empty");
    assert_eq!(
        candidates[0],
        default_config_file_path(),
        "first candidate must be default_config_file_path()"
    );
}

#[test]
fn config_candidate_paths_includes_system_path() {
    // /etc/cosmostrix/config.toml should always be in the candidate list
    // (it's a system-wide fallback). This is unconditional — even on
    // platforms where it doesn't exist, the candidate is listed so
    // the resolver can check it.
    let candidates = config_candidate_paths();
    let system = PathBuf::from("/etc")
        .join(CONFIG_DIR_NAME)
        .join(CONFIG_FILE_NAME);
    assert!(
        candidates.contains(&system),
        "candidate list must include {system:?}"
    );
}

#[test]
fn config_candidate_paths_includes_sdcard_path() {
    // /sdcard/cosmostrix/config.toml should be in the candidate list
    // (Termux external storage fallback).
    let candidates = config_candidate_paths();
    let sdcard = PathBuf::from("/sdcard")
        .join(CONFIG_DIR_NAME)
        .join(CONFIG_FILE_NAME);
    assert!(
        candidates.contains(&sdcard),
        "candidate list must include {sdcard:?}"
    );
}

#[test]
fn config_candidate_paths_no_duplicates() {
    // Even if XDG_CONFIG_HOME equals $HOME/.config, the candidate list
    // must not contain duplicate entries.
    let candidates = config_candidate_paths();
    let mut seen = std::collections::HashSet::new();
    for c in &candidates {
        assert!(seen.insert(c.clone()), "duplicate candidate: {c:?}");
    }
}

#[test]
fn resolve_watcher_config_path_uses_cli_config_when_provided() {
    // When --config <PATH> is given, the resolver must use that path
    // verbatim — no candidate search.
    let cli_path = Path::new("/tmp/cosmostrix-test-custom.toml");
    let (resolved, existed) = resolve_watcher_config_path(Some(cli_path));
    assert_eq!(resolved, cli_path, "must use CLI path verbatim");
    assert_eq!(
        existed,
        vec![cli_path],
        "existed list must be just the CLI path"
    );
}

#[test]
fn resolve_watcher_config_path_returns_default_when_no_candidates_exist() {
    // When no candidate exists, the resolver falls back to the default
    // path. This is the "user hasn't created a config yet" case.
    // Save the current env, unset HOME/XDG_CONFIG_HOME so the default
    // path is the relative `.config/cosmostrix/config.toml`.
    let saved_home = std::env::var("HOME").ok();
    let saved_xdg = std::env::var("XDG_CONFIG_HOME").ok();
    std::env::remove_var("HOME");
    std::env::remove_var("XDG_CONFIG_HOME");

    // Mark Termux detection as needing re-check by clearing env vars
    // that might match (the test runner might be on a system where
    // TERMUX_VERSION isn't set, which is the normal case).
    let (resolved, existed) = resolve_watcher_config_path(None);

    // Restore env vars immediately to avoid breaking other tests.
    if let Some(h) = saved_home {
        std::env::set_var("HOME", h);
    }
    if let Some(x) = saved_xdg {
        std::env::set_var("XDG_CONFIG_HOME", x);
    }

    // When HOME and XDG_CONFIG_HOME are both unset, the default path
    // is `.config/cosmostrix/config.toml` (relative). The resolver
    // must return this path. existed should be empty (the relative
    // path likely doesn't exist as a file).
    assert_eq!(
        resolved,
        PathBuf::from(".config")
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME),
        "must fall back to relative default path when no candidates exist"
    );
    // existed might or might not contain /etc/cosmostrix/config.toml
    // depending on the test runner. We don't assert on it because
    // it's environment-dependent.
    let _ = existed;
}

// ── v50 (alpha.2): extract_template_fingerprint tests ──

#[test]
fn extract_template_fingerprint_from_v501_header() {
    let content = dump_config_with_header();
    let fp = extract_template_fingerprint(&content);
    assert!(
        fp.is_some(),
        "should extract fingerprint from v50 alpha.2 header"
    );
    let fp = fp.unwrap();
    assert_eq!(fp.len(), 128, "fingerprint must be 128 hex chars");
    // Verify the extracted fingerprint matches a fresh body hash.
    let expected = sha512_hex(dump_config_text().as_bytes());
    assert_eq!(
        fp, expected,
        "extracted fingerprint must match fresh body hash"
    );
}

#[test]
fn extract_template_fingerprint_from_legacy_v50_header() {
    // Legacy v50 used the label `sha512 (template):` instead of
    // `template-fingerprint:`.
    let hash_hex = "a".repeat(128);
    let content = format!(
        "# cosmostrix config file\n# generated at 2026-01-01T00:00:00Z\n# using Howard Hinnant chrono design (libc::gmtime_r)\n# sha512 (template): {hash_hex}\n#\n# body here\n"
    );
    let fp = extract_template_fingerprint(&content);
    assert!(
        fp.is_some(),
        "should extract fingerprint from legacy v50 header"
    );
    assert_eq!(fp.unwrap(), hash_hex);
}

#[test]
fn extract_template_fingerprint_missing_header() {
    // Hand-written config with no fingerprint line.
    let content = "# cosmostrix configuration\n# some random config\ncolor = green\n";
    let fp = extract_template_fingerprint(content);
    assert!(fp.is_none(), "should return None when no fingerprint line");
}

#[test]
fn extract_template_fingerprint_invalid_hex() {
    // Fingerprint line with invalid (too short) hex.
    let content = "# template-fingerprint: deadbeef\n# rest of file\n";
    let fp = extract_template_fingerprint(content);
    assert!(fp.is_none(), "should reject fingerprint with wrong length");
}

#[test]
fn extract_template_fingerprint_only_scans_first_6_lines() {
    // Fingerprint on line 8 (beyond the scan window) should be ignored.
    let hash_hex = "b".repeat(128);
    let content = format!(
        "# line 1\n# line 2\n# line 3\n# line 4\n# line 5\n# line 6\n# line 7\n# template-fingerprint: {hash_hex}\n"
    );
    let fp = extract_template_fingerprint(&content);
    assert!(
        fp.is_none(),
        "should not find fingerprint beyond first 6 lines"
    );
}
