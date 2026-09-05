// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! ambient tests, extracted from inline `mod tests { ... }` block in
//! ambient.rs (Pattern D → Pattern C unification).
//!
//! Uses `use super::*;` to access ambient.rs's private items unchanged.

// ── is_ambient_config_key ──

use super::*;
#[test]
fn recognizes_valid_keys() {
    assert!(is_ambient_config_key("ambient.00-00"));
    assert!(is_ambient_config_key("ambient.12-30"));
    assert!(is_ambient_config_key("ambient.23-59"));
}

#[test]
fn rejects_invalid_time_suffix() {
    assert!(!is_ambient_config_key("ambient.24-00")); // HH out of range
    assert!(!is_ambient_config_key("ambient.12-60")); // MM out of range
    assert!(!is_ambient_config_key("ambient.midnight")); // not HH-MM
    assert!(!is_ambient_config_key("ambient.1-00")); // not zero-padded
    assert!(!is_ambient_config_key("ambient.12:00")); // colon not dash
    assert!(!is_ambient_config_key("ambient.1200")); // missing dash
}

#[test]
fn rejects_wrong_namespace() {
    assert!(!is_ambient_config_key("scene-custom.12-00"));
    assert!(!is_ambient_config_key("colors-custom.12-00"));
    assert!(!is_ambient_config_key("adaptive-custom.12-00")); // archived
    assert!(!is_ambient_config_key("ambient")); // no suffix
}

// ── parse_ambient_value ( single scene name) ──

#[test]
fn parses_single_builtin_scene_name() {
    let e = parse_ambient_value("signal").unwrap();
    assert_eq!(e.scene, "signal");
}

#[test]
fn parses_single_custom_scene_name() {
    let e = parse_ambient_value("afternoon").unwrap();
    assert_eq!(e.scene, "afternoon");
}

#[test]
fn parses_name_with_surrounding_whitespace() {
    let e = parse_ambient_value("  signal  ").unwrap();
    assert_eq!(e.scene, "signal");
}

#[test]
fn parses_name_with_underscores_and_dashes() {
    let e = parse_ambient_value("night_mode").unwrap();
    assert_eq!(e.scene, "night_mode");
    let e = parse_ambient_value("night-mode").unwrap();
    assert_eq!(e.scene, "night-mode");
}

#[test]
fn rejects_empty_value() {
    assert!(parse_ambient_value("").is_err());
    assert!(parse_ambient_value("   ").is_err());
}

// ── migration: legacy multi-field format must produce a
//    helpful migration error, NOT silently drop fields. ──

#[test]
fn rejects_legacy_multifield_format_with_migration_message() {
    // User's exact  config from the bug report — must surface
    // a migration error pointing to [scene-custom.*] + base-scene.
    let err = parse_ambient_value("neon-purple, signal, speed=50, density=0.65")
        .expect_err("legacy format must be rejected");
    assert!(
        err.contains("legacy multi-field format no longer supported"),
        "missing migration header: {err}"
    );
    assert!(
        err.contains("[scene-custom.<name>]"),
        "missing scene-custom hint: {err}"
    );
    assert!(err.contains("base-scene"), "missing base-scene hint: {err}");
    assert!(
        err.contains("ambient.<HH-MM> = <name>"),
        "missing new format example: {err}"
    );
}

#[test]
fn rejects_legacy_color_scene_positional_only() {
    // Even just `cosmos, monolith` (no kv pairs) is  format.
    let err =
        parse_ambient_value("cosmos, monolith").expect_err("comma must trigger migration error");
    assert!(err.contains("legacy multi-field format"));
}

#[test]
fn rejects_legacy_kv_only_format() {
    // `speed=15, density=1.2` (no positionals) is also  format.
    let err = parse_ambient_value("speed=15, density=1.2")
        .expect_err("equals sign must trigger migration error");
    assert!(err.contains("legacy multi-field format"));
}

#[test]
fn migration_message_includes_user_repro_example() {
    // The user's exact bug-report config should appear in the message
    // so they can copy-paste the migration target.
    let err = parse_ambient_value("neon-purple, signal, speed=50, density=0.65").unwrap_err();
    assert!(
        err.contains("ambient.15-00 = neon-purple, signal, speed=50, density=0.65"),
        "migration message should include the user's repro example: {err}"
    );
    assert!(
        err.contains("[scene-custom.afternoon]"),
        "migration message should include the afternoon example: {err}"
    );
}

// ── AmbientSchedule helpers (current_phase / next_phase / seconds_to_next_phase) ──

/// Helper: build a minimal entry for schedule tests.
fn entry(h: u32, m: u32, scene: &str) -> AmbientEntry {
    AmbientEntry {
        hour: h,
        minute: m,
        scene: scene.to_string(),
    }
}

#[test]
fn current_phase_finds_latest_before_now() {
    let s = AmbientSchedule {
        entries: vec![entry(0, 0, "cinematic"), entry(12, 0, "signal")],
    };
    // 12:30 → current is 12:00
    assert_eq!(s.current_phase(12 * 60 + 30).unwrap().hour, 12);
    // 11:59 → current is 00:00 (12:00 not yet fired)
    assert_eq!(s.current_phase(11 * 60 + 59).unwrap().hour, 0);
    // 13:00 → current is 12:00 (last entry <= 13:00)
    assert_eq!(s.current_phase(13 * 60).unwrap().hour, 12);
}

#[test]
fn current_phase_wraps_to_last_entry_before_first() {
    // 2 entries: 06:00, 18:00. now=03:00 → no entry has fired today,
    // wrap to last entry (18:00 from yesterday).
    let s = AmbientSchedule {
        entries: vec![entry(6, 0, "matrix"), entry(18, 0, "monolith")],
    };
    let cur = s.current_phase(3 * 60).unwrap();
    assert_eq!(cur.hour, 18);
}

#[test]
fn current_phase_empty_schedule_returns_none() {
    let s = AmbientSchedule::default();
    assert!(s.current_phase(0).is_none());
}

#[test]
fn next_phase_finds_earliest_after_now() {
    let s = AmbientSchedule {
        entries: vec![entry(0, 0, "cinematic"), entry(12, 0, "signal")],
    };
    // 11:00 → next is 12:00
    assert_eq!(s.next_phase(11 * 60).unwrap().hour, 12);
    // 12:30 → next is 00:00 (tomorrow)
    assert_eq!(s.next_phase(12 * 60 + 30).unwrap().hour, 0);
}

#[test]
fn next_phase_empty_schedule_returns_none() {
    let s = AmbientSchedule::default();
    assert!(s.next_phase(0).is_none());
}

#[test]
fn seconds_to_next_phase_normal_case() {
    let s = AmbientSchedule {
        entries: vec![entry(12, 0, "signal")],
    };
    // now = 11:00:00 (660 min, 0 sec). next = 12:00:00 (720 min). diff = 60*60 = 3600 sec.
    assert_eq!(s.seconds_to_next_phase(660, 0), Some(3600));
    // now = 11:59:30 (719 min, 30 sec). next = 12:00:00. diff = 30 sec.
    assert_eq!(s.seconds_to_next_phase(719, 30), Some(30));
}

#[test]
fn seconds_to_next_phase_wraps_midnight() {
    let s = AmbientSchedule {
        entries: vec![entry(6, 0, "matrix")],
    };
    // now = 23:00:00 (1380 min). next = 06:00:00 tomorrow (360 min).
    // diff = (24*60 - 1380 + 360) * 60 = 420 * 60 = 25200 sec.
    // Capped at 3600.
    assert_eq!(s.seconds_to_next_phase(1380, 0), Some(3600));
}

#[test]
fn seconds_to_next_phase_empty_returns_none() {
    let s = AmbientSchedule::default();
    assert!(s.seconds_to_next_phase(0, 0).is_none());
}

// ── collect_ambient_schedule ──

#[test]
fn collect_sorts_entries_by_time() {
    let mut cfg = HashMap::new();
    cfg.insert("ambient.18-00".into(), "monolith".into());
    cfg.insert("ambient.06-00".into(), "matrix".into());
    cfg.insert("ambient.12-00".into(), "signal".into());
    let s = collect_ambient_schedule(&cfg);
    assert_eq!(s.entries.len(), 3);
    assert_eq!(s.entries[0].hour, 6);
    assert_eq!(s.entries[1].hour, 12);
    assert_eq!(s.entries[2].hour, 18);
    // Each entry's scene is preserved.
    assert_eq!(s.entries[0].scene, "matrix");
    assert_eq!(s.entries[1].scene, "signal");
    assert_eq!(s.entries[2].scene, "monolith");
}

#[test]
fn collect_skips_legacy_format_entries() {
    // legacy multi-field entries fail to parse and are silently
    // dropped from the runtime schedule (strict --testconf still errors).
    // This matches the live-reload contract: a half-edited config must
    // not crash the runtime.
    let mut cfg = HashMap::new();
    cfg.insert("ambient.12-00".into(), "signal".into());
    cfg.insert("ambient.18-00".into(), "neon, monolith, speed=15".into());
    let s = collect_ambient_schedule(&cfg);
    assert_eq!(s.entries.len(), 1);
    assert_eq!(s.entries[0].hour, 12);
    assert_eq!(s.entries[0].scene, "signal");
}

#[test]
fn collect_returns_empty_when_no_ambient_keys() {
    let mut cfg = HashMap::new();
    cfg.insert("color".into(), "neon-green".into());
    cfg.insert("scene".into(), "monolith".into());
    let s = collect_ambient_schedule(&cfg);
    assert!(s.is_empty());
}

#[test]
fn collect_preserves_custom_scene_names() {
    // custom scene names are stored verbatim — validation that
    // they reference a defined [scene-custom.<name>] block happens in
    // validate_ambient_entries, not collect_ambient_schedule.
    let mut cfg = HashMap::new();
    cfg.insert("ambient.13-00".into(), "afternoon".into());
    let s = collect_ambient_schedule(&cfg);
    assert_eq!(s.entries.len(), 1);
    assert_eq!(s.entries[0].scene, "afternoon");
}

// ── validate_ambient_entries ──

#[test]
fn validate_accepts_builtin_scene_names() {
    let mut cfg = HashMap::new();
    cfg.insert("ambient.00-00".into(), "cinematic".into());
    cfg.insert("ambient.12-00".into(), "signal".into());
    cfg.insert("ambient.18-00".into(), "monolith".into());
    assert!(validate_ambient_entries(&cfg).is_ok());
}

#[test]
fn validate_accepts_custom_scene_names() {
    let mut cfg = HashMap::new();
    cfg.insert("scene-custom.afternoon.color".into(), "neon-green".into());
    cfg.insert("ambient.15-00".into(), "afternoon".into());
    assert!(validate_ambient_entries(&cfg).is_ok());
}

#[test]
fn validate_rejects_unknown_scene_name() {
    let mut cfg = HashMap::new();
    cfg.insert("ambient.00-00".into(), "nonexistent-scene".into());
    let err = validate_ambient_entries(&cfg).unwrap_err();
    assert!(
        err.contains("unknown scene 'nonexistent-scene'"),
        "got: {err}"
    );
    assert!(
        err.contains("[scene-custom.nonexistent-scene]"),
        "should hint at scene-custom block: {err}"
    );
}

#[test]
fn validate_rejects_legacy_format_with_migration_hint() {
    // a legacy multi-field entry must fail validation with the
    // full migration message. This is the primary user-facing error
    // path — when a user runs `--testconf` on an old config, they see
    // this and learn how to migrate.
    let mut cfg = HashMap::new();
    cfg.insert(
        "ambient.15-00".into(),
        "neon-purple, signal, speed=50, density=0.65".into(),
    );
    let err = validate_ambient_entries(&cfg).unwrap_err();
    assert!(err.contains("legacy multi-field format"), "got: {err}");
    assert!(err.contains("[scene-custom"), "got: {err}");
    assert!(err.contains("base-scene"), "got: {err}");
}

#[test]
fn validate_accepts_empty_schedule() {
    let cfg = HashMap::new();
    assert!(validate_ambient_entries(&cfg).is_ok());
}

#[test]
fn validate_case_insensitive_custom_scene_lookup() {
    // Custom scene names are stored lowercase by collect_custom_scenes;
    // validate_ambient_entries should match case-insensitively.
    let mut cfg = HashMap::new();
    cfg.insert("scene-custom.afternoon.color".into(), "neon-green".into());
    cfg.insert("ambient.15-00".into(), "AFTERNOON".into());
    assert!(validate_ambient_entries(&cfg).is_ok());
}

// ── wall-clock helpers ──

#[test]
fn current_minute_of_day_bounded() {
    let m = current_minute_of_day();
    assert!(m < 24 * 60, "minute of day out of range: {m}");
}

/// NIGHT-hunter-12: the scheduler's one-read snapshot replaces the old
/// separate second/yday helpers. Its fields must be bounded AND mutually
/// coherent — minute + second describe the same clock sample (that is the
/// whole point of the snapshot: the torn minute/second read was the
/// cadence audit's correctness defect).
#[test]
fn ambient_clock_snapshot_bounded_and_coherent() {
    use crate::crystal_dragon_engine::ambient::AmbientClockSnapshot;
    let snap = AmbientClockSnapshot::now();
    assert!(snap.minute_of_day < 24 * 60, "minute of day out of range");
    assert!(snap.second_of_minute < 60, "second of minute out of range");
    assert!(
        (0..=366).contains(&snap.yday),
        "yday out of range: {}",
        snap.yday
    );
    // Coherence: the snapshot's total seconds-of-day must round-trip
    // through a second read taken immediately after within one minute of
    // slack (wall clock advances between the two reads, never rewinds).
    let total = snap.minute_of_day * 60 + snap.second_of_minute;
    let next = AmbientClockSnapshot::now();
    let next_total = next.minute_of_day * 60 + next.second_of_minute;
    let drift = (next_total as i64 - total as i64).rem_euclid(86_400);
    assert!(
        drift < 120,
        "two back-to-back snapshots drifted {drift}s — not one consistent clock each"
    );
}
