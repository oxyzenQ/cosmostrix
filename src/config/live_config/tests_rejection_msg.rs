// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for rejection-log (bug #14), validate_and_send error path,
//! live-reload message/msg-mode, and ambient snapback.
//!
//! Extracted from `live_config/tests.rs` to keep that source file under
//! the 800-LOC cap. Pure code motion — no behavior change.

use super::tests::minimal_cloud_config;
use super::*;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

use crate::configfile;

/// (bug #16): Serialize every test that touches the global
/// `LIVE_RELOAD_VALIDATION_REJECTIONS` log (directly or indirectly via
/// `validate_and_send`). Without this lock, cargo test's default
/// thread-pool runs these tests in parallel and one test drains another
/// test's expected rejection — `assert_eq!(rejections.len(), 1)` then
/// sees 0 or 2+ and fails spuriously.
static TEST_REJECTION_LOCK: Mutex<()> = Mutex::new(());

/// FIX D: validate_and_send returns Err on bad config, but the
/// render thread NO LONGER sets LIVE_RELOAD_EXIT_CODE — only true
/// watcher-thread panics do. FIX E: error includes a hint.
#[test]
fn validate_and_send_returns_err_without_setting_exit_code() {
    let _guard = TEST_REJECTION_LOCK.lock().unwrap();
    let _ = drain_validation_rejections();
    let (tx, _rx) = std::sync::mpsc::sync_channel(64);
    let mut parsed = configfile::ParsedConfig::default();
    parsed.unknown_keys.push("color.tune.bold".to_string());
    let result = validate_and_send(&parsed, &tx);
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(msg.contains("color.tune.bold"));
    assert!(msg.contains("top-level"), "need structural hint: {msg}");
    assert!(msg.contains("[color.tune]"), "need section ref: {msg}");
    assert_eq!(LIVE_RELOAD_EXIT_CODE.load(Ordering::Acquire), 0);
}

/// (bug #9): color.tune.* changes must propagate via live reload.
/// Before the fix, `rebuild_cloud_config` never touched `color_tune`,
/// so editing `brightness = 0.0` while running had zero effect until
/// restart. Verify brightness/saturation/head/body/tail all flow through.
#[test]
fn rebuild_applies_color_tune_live_reload_brightness() {
    let base = minimal_cloud_config();
    assert_eq!(
        base.color_tune.brightness, 1.0,
        "base config should start at identity brightness"
    );
    let mut cfg = HashMap::new();
    cfg.insert("color.tune.brightness".to_string(), "0.5".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        (new.color_tune.brightness - 0.5).abs() < 1e-6,
        "brightness should propagate to live-reloaded config (got {})",
        new.color_tune.brightness
    );
}

/// (bug #9): all 5 color.tune.* fields propagate, not just brightness.
#[test]
fn rebuild_applies_color_tune_live_reload_all_fields() {
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("color.tune.brightness".to_string(), "1.5".to_string());
    cfg.insert("color.tune.saturation".to_string(), "0.7".to_string());
    cfg.insert("color.tune.head".to_string(), "2.0".to_string());
    cfg.insert("color.tune.body".to_string(), "1.2".to_string());
    cfg.insert("color.tune.tail".to_string(), "0.8".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert!((new.color_tune.brightness - 1.5).abs() < 1e-6);
    assert!((new.color_tune.saturation - 0.7).abs() < 1e-6);
    assert!((new.color_tune.head - 2.0).abs() < 1e-6);
    assert!((new.color_tune.body - 1.2).abs() < 1e-6);
    assert!((new.color_tune.tail - 0.8).abs() < 1e-6);
}

/// (bug #9): when no color.tune.* keys are in config, the tune
/// stays at the base value (identity by default). This protects users
/// v50.0.0-alpha.7: was `rebuild_without_color_tune_keys_keeps_base_tune`.
/// OLD behavior: no color.tune.* keys → keep base tune (brightness=2.0
/// preserved). NEW behavior: no color.tune.* keys → reset to IDENTITY
/// (brightness=1.0). This is the fix for the color.tune reset-on-comment
/// bug: when user comments out all color.tune.* keys, the rain should
/// return to normal (identity), not stay at the old value.
#[test]
fn rebuild_without_color_tune_keys_resets_to_identity() {
    let mut base = minimal_cloud_config();
    base.color_tune.brightness = 2.0; // set at startup via config
    base.cli_explicit.color_tune = false; // NOT CLI --color-tune
    let cfg = HashMap::new(); // no color.tune.* keys (all commented out)
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.color_tune.brightness, 1.0,
        "no color.tune.* in config + no CLI → reset to identity (1.0)"
    );
}

/// v50.0.0-alpha.7: CLI --color-tune explicit → config absence does NOT
/// reset to identity (CLI wins). This preserves the "set once, keep
/// forever" semantics for CLI users.
#[test]
fn rebuild_color_tune_cli_explicit_preserves_base() {
    let mut base = minimal_cloud_config();
    base.color_tune.brightness = 2.0;
    base.cli_explicit.color_tune = true; // CLI --color-tune bright=2.0
    let cfg = HashMap::new(); // no color.tune.* keys in config
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.color_tune.brightness, 2.0,
        "CLI --color-tune explicit → config absence must NOT reset"
    );
}

/// v50.0.0-alpha.7: color.tune.brightness=0.0 set at startup, then user
/// comments it out → rain should return to normal (brightness=1.0).
/// This is the primary bug the owner reported.
#[test]
fn rebuild_color_tune_reset_on_comment_bug_fix() {
    let mut base = minimal_cloud_config();
    base.color_tune.brightness = 0.0; // user set this at startup
    base.cli_explicit.color_tune = false;
    let cfg = HashMap::new(); // all color.tune.* commented out
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.color_tune.brightness, 1.0,
        "commenting out color.tune.brightness must reset to 1.0"
    );
}

/// v50.0.0-alpha.7: color.tune.brightness=0.0 set at startup, then user
/// changes to brightness=2.0 → rain updates to 2.0 (normal live-reload).
#[test]
fn rebuild_color_tune_live_reload_change() {
    let mut base = minimal_cloud_config();
    base.color_tune.brightness = 0.0;
    base.cli_explicit.color_tune = false;
    let mut cfg = HashMap::new();
    cfg.insert("color.tune.brightness".to_string(), "2.0".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.color_tune.brightness, 2.0);
}

/// (bug #14): `validate_and_send` must push every rejection to
/// the session log so the post-exit verbose summary can surface silent
/// rejections. Before the fix, an OOR value like `color.tune.tail = 5.0`
/// got silently rejected by `validate_config_strictly` — the watcher
/// kept watching, the rain kept running on the last valid config, and
/// the user had no idea their edit was rejected.
#[test]
fn validate_and_send_pushes_oor_rejection_to_session_log() {
    // (bug #16): hold the serialization lock so parallel tests
    // cannot drain our rejection mid-test.
    let _guard = TEST_REJECTION_LOCK.lock().unwrap();
    // Drain any prior rejections from earlier tests in this process.
    let _ = drain_validation_rejections();

    let (tx, _rx) = std::sync::mpsc::sync_channel(64);
    let mut parsed = configfile::ParsedConfig::default();
    parsed
        .values
        .insert("color.tune.tail".to_string(), "5.0".to_string());
    let result = validate_and_send(&parsed, &tx);
    assert!(result.is_err(), "OOR color.tune.tail must be rejected");

    let rejections = drain_validation_rejections();
    assert_eq!(
        rejections.len(),
        1,
        "exactly one rejection should be in the session log"
    );
    let entry = &rejections[0];
    assert!(
        entry.contains("color.tune.tail"),
        "rejection must name the bad field: {entry}"
    );
    assert!(
        entry.contains("out of range"),
        "rejection must mention range: {entry}"
    );

    // Drain must empty the log — next call returns empty Vec.
    let again = drain_validation_rejections();
    assert!(again.is_empty(), "drain must empty the log");
}

/// (bug #14): malformed lines and unknown keys must ALSO push to
/// the session log, not just strict value validation failures. All three
/// rejection paths in `validate_and_send` must be visible under `-v`.
#[test]
fn validate_and_send_pushes_unknown_key_to_session_log() {
    let _guard = TEST_REJECTION_LOCK.lock().unwrap();
    let _ = drain_validation_rejections();

    let (tx, _rx) = std::sync::mpsc::sync_channel(64);
    let mut parsed = configfile::ParsedConfig::default();
    parsed.unknown_keys.push("collor".to_string());
    let result = validate_and_send(&parsed, &tx);
    assert!(result.is_err());

    let rejections = drain_validation_rejections();
    assert_eq!(rejections.len(), 1);
    assert!(
        rejections[0].contains("collor"),
        "unknown-key rejection must be in session log: {}",
        rejections[0]
    );
}

/// (bug #14): cap at MAX_REJECTION_LOG (64) to avoid unbounded
/// growth on a misbehaving editor that saves 1000 times per second.
#[test]
fn rejection_log_caps_at_max() {
    let _guard = TEST_REJECTION_LOCK.lock().unwrap();
    let _ = drain_validation_rejections();

    for _ in 0..100 {
        push_validation_rejection("test rejection");
    }
    let rejections = drain_validation_rejections();
    assert_eq!(
        rejections.len(),
        MAX_REJECTION_LOG,
        "log must cap at MAX_REJECTION_LOG (64), got {}",
        rejections.len()
    );

    // Drain must reset — fresh log after drain.
    let again = drain_validation_rejections();
    assert!(again.is_empty());
}

/// (bug #14): valid config does NOT push to the session log.
/// Only rejections are logged; valid reloads are silent (the rebuild
/// trace already covers the success path).
#[test]
fn validate_and_send_does_not_log_valid_config() {
    let _guard = TEST_REJECTION_LOCK.lock().unwrap();
    let _ = drain_validation_rejections();

    let (tx, _rx) = std::sync::mpsc::sync_channel(64);
    let mut parsed = configfile::ParsedConfig::default();
    parsed
        .values
        .insert("color.tune.brightness".to_string(), "1.5".to_string());
    let result = validate_and_send(&parsed, &tx);
    assert!(result.is_ok(), "1.5 is in range [0.0, 3.0]");

    let rejections = drain_validation_rejections();
    assert!(
        rejections.is_empty(),
        "valid config must not push to rejection log, got: {rejections:?}"
    );
}

// ── v50.0.0-alpha.7: live-reload message / message-border / msg-mode ──

#[test]
fn live_reload_message_border_from_config() {
    // Config `message-border = "hello"` → new.message = "hello", border=true.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("message-border".to_string(), "hello".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.message.as_deref(), Some("hello"));
    assert!(
        new.message_border,
        "message-border config must set border=true"
    );
}

#[test]
fn live_reload_message_bare_from_config() {
    // Config `message = "hello"` (no border) → new.message = "hello", border=false.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("message".to_string(), "hello".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.message.as_deref(), Some("hello"));
    assert!(!new.message_border, "message config must keep border=false");
}

#[test]
fn live_reload_message_border_wins_over_message() {
    // Both keys present → message-border wins (border=true).
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("message".to_string(), "plain".to_string());
    cfg.insert("message-border".to_string(), "boxed".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.message.as_deref(), Some("boxed"));
    assert!(new.message_border);
}

#[test]
fn live_reload_msg_mode_false_suppresses_config_message() {
    // msg-mode=false + config message-border → message suppressed.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("msg-mode".to_string(), "false".to_string());
    cfg.insert("message-border".to_string(), "hello".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.message, None,
        "msg-mode=false must suppress config message"
    );
    assert!(!new.message_border);
    assert!(!new.msg_mode, "msg_mode field must reflect false");
}

#[test]
fn live_reload_msg_mode_true_keeps_config_message() {
    // msg-mode=true + config message → preserved.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("msg-mode".to_string(), "true".to_string());
    cfg.insert("message-border".to_string(), "hello".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.message.as_deref(), Some("hello"));
    assert!(new.message_border);
    assert!(new.msg_mode);
}

#[test]
fn live_reload_msg_mode_defaults_true_when_unset() {
    // No msg-mode in config → default true.
    let base = minimal_cloud_config();
    let cfg = HashMap::new();
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(new.msg_mode, "msg_mode must default to true when unset");
}

#[test]
fn live_reload_no_config_message_reverts_to_default() {
    // Bug fix v50.0.0-beta.7: when config has `message = "hey"` at
    // startup, base.message = Some("hey"). User comments it out
    // (# message = "hey"). Live-reload MUST revert to the default
    // "Experience a masterpiece with cosmostrix v{}" with border=true
    // — NOT preserve the stale "hey". Mirrors the color.tune
    // reset-on-comment pattern (LIVE_RELOAD_BEHAVIOR.md Limitation C,
    // fixed v50.0.0-alpha.7).
    let mut base = minimal_cloud_config();
    base.message = Some("hey".to_string()); // stale config value
    base.message_border = false; // stale border state
    base.cli_explicit.message = false; // no CLI -m/-mb
    let cfg = HashMap::new(); // no message keys (commented out)
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.message.as_deref(),
        Some(crate::constants::default_message_text().as_str()),
        "commented-out config message must revert to default fallback, not preserve stale value"
    );
    assert!(
        new.message_border,
        "default fallback message must have border=true (mirrors main.rs startup fallback)"
    );
}

#[test]
fn live_reload_no_config_message_clears_when_msg_mode_false() {
    // When msg-mode=false and no config message, message must be None
    // (not the default fallback). This was already correct before the
    // fix — this test locks the behavior to prevent regression.
    let mut base = minimal_cloud_config();
    base.message = Some("stale".to_string());
    base.message_border = true;
    base.cli_explicit.message = false;
    base.msg_mode = false;
    base.cli_explicit.msg_mode = true; // CLI --msg-mode false explicit
    let cfg = HashMap::new();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.message, None,
        "msg-mode=false + no config → message must be None"
    );
    assert!(!new.message_border);
}

#[test]
fn live_reload_cli_message_locked_falls_back_when_config_absent() {
    // v80.0.0-beta.1: config message key PRESENT wins over the CLI -m lock
    // (temporal precedence) — the rewrite of the old "CLI wins over
    // config" test. The CLI value becomes the fallback: commenting the
    // key back out keeps "from-cli" (pinned in tests_cli_fallback.rs,
    // fallback_message_key_absent_keeps_cli_lock).
    let mut base = minimal_cloud_config();
    base.message = Some("from-cli".to_string());
    base.message_border = false;
    base.cli_explicit.message = true;
    let mut cfg = HashMap::new();
    cfg.insert("message-border".to_string(), "from-config".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.message.as_deref(),
        Some("from-config"),
        "config message-border key present must override the CLI -m lock (v80.0.0-beta.1)"
    );
    assert!(
        new.message_border,
        "message-border key implies border=true (msg-mode gate passes: default true)"
    );
}

#[test]
fn live_reload_config_msg_mode_overrides_cli_lock_when_present() {
    // v80.0.0-beta.1: config `msg-mode=true` key PRESENT wins over the CLI
    // --msg-mode false lock — so the config message shows. The CLI lock
    // returns when the key is commented back out (fallback path pinned
    // in tests_cli_fallback.rs).
    let mut base = minimal_cloud_config();
    base.msg_mode = false;
    base.cli_explicit.msg_mode = true;
    let mut cfg = HashMap::new();
    cfg.insert("msg-mode".to_string(), "true".to_string());
    cfg.insert("message-border".to_string(), "hello".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        new.msg_mode,
        "config msg-mode key present must override the CLI lock (v80.0.0-beta.1)"
    );
    assert_eq!(
        new.message.as_deref(),
        Some("hello"),
        "msg-mode=true + config message must show the message"
    );
}

#[test]
fn live_reload_power_dragon_key_overrides_cli_lock_when_present() {
    // v80.0.0-beta.1: config `power-dragon=true` key PRESENT wins over the CLI
    // --power-dragon false lock. The CLI value is the fallback on key
    // absence (pinned in tests_cli_fallback.rs).
    let mut base = minimal_cloud_config();
    base.power_dragon = false;
    base.cli_explicit.power_dragon = true;
    let mut cfg = HashMap::new();
    cfg.insert("power-dragon".to_string(), "true".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        new.power_dragon,
        "config power-dragon key present must override the CLI lock (v80.0.0-beta.1)"
    );
}

#[test]
fn live_reload_async_mode_key_overrides_cli_lock_when_present() {
    // v80.0.0-beta.1: config `async-mode=true` key PRESENT wins over the CLI
    // --async-mode false lock. The CLI value is the fallback on key
    // absence (pinned in tests_cli_fallback.rs).
    let mut base = minimal_cloud_config();
    base.async_mode = false;
    base.cli_explicit.async_mode = true;
    let mut cfg = HashMap::new();
    cfg.insert("async-mode".to_string(), "true".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        new.async_mode,
        "config async-mode key present must override the CLI lock (v80.0.0-beta.1)"
    );
}

#[test]
fn live_reload_intro_color_from_config() {
    // Config intro-color = "energy-zen" (valid) → new.intro_color set.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("intro-color".to_string(), "energy-zen".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.intro_color.as_deref(), Some("energy-zen"));
}

#[test]
fn live_reload_intro_color_invalid_soft_fails() {
    // Config intro-color = "not-a-color" (invalid) → soft-fail: clear field.
    // Unlike startup (hard error + exit), live-reload soft-fails to avoid
    // crashing a running session. User can fix config and save again.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("intro-color".to_string(), "not-a-color".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.intro_color, None,
        "invalid intro-color must soft-fail (clear)"
    );
}

#[test]
fn live_reload_intro_color_key_overrides_cli_lock_when_present() {
    // v80.0.0-beta.1: config `intro-color` key PRESENT wins over the CLI
    // --intro-color lock. The CLI value is the fallback on key absence.
    let mut base = minimal_cloud_config();
    base.intro_color = Some("green".to_string());
    base.cli_explicit.intro_color = true;
    let mut cfg = HashMap::new();
    cfg.insert("intro-color".to_string(), "energy-zen".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.intro_color.as_deref(),
        Some("energy-zen"),
        "config intro-color key present must override the CLI lock (v80.0.0-beta.1)"
    );
}

#[test]
fn live_reload_monolith_size_key_overrides_cli_lock_when_present() {
    // v80.0.0-beta.1: config `monolith-size` key PRESENT wins over the CLI lock
    // (temporal precedence — the key is the most recent user intent).
    // The CLI value is the fallback on key absence.
    use crate::runtime::MonolithSize;
    let mut base = minimal_cloud_config();
    base.monolith_size = MonolithSize::Large;
    base.cli_explicit.monolith_size = true;
    let mut cfg = HashMap::new();
    cfg.insert("monolith-size".to_string(), "small".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.monolith_size,
        MonolithSize::Small,
        "config monolith-size key present must override the CLI lock (v80.0.0-beta.1)"
    );
}

#[test]
fn live_reload_monolith_size_from_config_when_no_cli() {
    // No CLI flag → config monolith-size=small applied.
    use crate::runtime::MonolithSize;
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("monolith-size".to_string(), "small".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.monolith_size, MonolithSize::Small);
}

#[test]
fn live_reload_ambient_snapback_secs_from_config() {
    // v50.0.0-beta.7: ambient-snapback-secs config key (config-only).
    // When set in config, live-reload applies it.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("ambient-snapback-secs".to_string(), "120".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.ambient_snapback_secs,
        Some(120.0),
        "ambient-snapback-secs=120 must be applied on live-reload"
    );
}

#[test]
fn live_reload_ambient_snapback_secs_defaults_none_when_unset() {
    // When ambient-snapback-secs is not in config, it stays None
    // (event loop falls back to AUTO_SNAPBACK_DELAY_SECS = 30.0).
    let base = minimal_cloud_config();
    let cfg = HashMap::new();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.ambient_snapback_secs, None,
        "ambient-snapback-secs must be None when unset (default 30s)"
    );
}

#[test]
fn live_reload_ambient_snapback_secs_invalid_falls_back_to_none() {
    // Invalid value (out of range or non-numeric) → parse_f64_config
    // returns None, so ambient_snapback_secs stays None (default 30s).
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("ambient-snapback-secs".to_string(), "999999".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.ambient_snapback_secs, None,
        "out-of-range ambient-snapback-secs must fall back to None"
    );
}

#[test]
fn live_reload_ambient_snapback_secs_zero_is_valid() {
    // 0.0 is the lower bound — instant snapback. Must be accepted.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("ambient-snapback-secs".to_string(), "0".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.ambient_snapback_secs,
        Some(0.0),
        "ambient-snapback-secs=0 must be accepted (instant snapback)"
    );
}

#[test]
fn live_reload_ambient_snapback_secs_86400_is_valid() {
    // 86400.0 is the upper bound — 24h, effectively disables snapback.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("ambient-snapback-secs".to_string(), "86400".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.ambient_snapback_secs,
        Some(86400.0),
        "ambient-snapback-secs=86400 must be accepted (24h = disabled)"
    );
}

// ── v80.0.0-alpha.1: crystal-dragon-secs live-reload (harmony twin of the
// ambient-snapback-secs block above — same semantics, mirrored tests) ──

#[test]
fn live_reload_crystal_dragon_secs_from_config() {
    // Config key present (valid) → applied over the base lock.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("crystal-dragon-secs".to_string(), "120".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.crystal_dragon_secs,
        Some(120.0),
        "crystal-dragon-secs=120 must be applied on live-reload"
    );
}

#[test]
fn live_reload_crystal_dragon_secs_present_wins_over_cli_lock() {
    // CLI lock (--crystal-dragon-secs 90 baked into base) + a present
    // config key (45) → the key wins (most recent user intent), mirroring
    // the crystal-dragon bool contract.
    let mut base = minimal_cloud_config();
    base.crystal_dragon_secs = Some(90.0);
    let mut cfg = HashMap::new();
    cfg.insert("crystal-dragon-secs".to_string(), "45".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.crystal_dragon_secs,
        Some(45.0),
        "present config key must override the CLI lock"
    );
}

#[test]
fn live_reload_crystal_dragon_secs_absent_keeps_cli_lock() {
    // Key absent (commented out) → the CLI lock survives as the fallback.
    // (Unlike ambient-snapback-secs — config-only, reset-on-comment — this
    // knob has a CLI surface, so the lock contract applies.)
    let mut base = minimal_cloud_config();
    base.crystal_dragon_secs = Some(90.0);
    let cfg = HashMap::new();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.crystal_dragon_secs,
        Some(90.0),
        "absent key must keep the locked startup value"
    );
}

#[test]
fn live_reload_crystal_dragon_secs_invalid_keeps_base() {
    // Out-of-range → parse_f64_config returns None → base kept
    // (defense-in-depth; upstream strict validation rejects the file).
    let mut base = minimal_cloud_config();
    base.crystal_dragon_secs = Some(90.0);
    let mut cfg = HashMap::new();
    cfg.insert("crystal-dragon-secs".to_string(), "999999".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.crystal_dragon_secs,
        Some(90.0),
        "out-of-range crystal-dragon-secs must keep the base value"
    );
}

#[test]
fn live_reload_crystal_dragon_secs_bounds_are_valid() {
    // 0.0 (lower bound — per-tick poll) and 86400.0 (upper — once/24h)
    // must both be accepted.
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("crystal-dragon-secs".to_string(), "0".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.crystal_dragon_secs,
        Some(0.0),
        "crystal-dragon-secs=0 must be accepted"
    );
    let mut cfg = HashMap::new();
    cfg.insert("crystal-dragon-secs".to_string(), "86400".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.crystal_dragon_secs,
        Some(86400.0),
        "crystal-dragon-secs=86400 must be accepted"
    );
}
