// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-alpha.2 (S-master-HUNT-4) tests: the owner's CLI-fallback bug
//! quartet + the human-duration contract.
//!
//! 1. Zero-key config delivery — commenting out EVERY config key must
//!    reach the render thread as the empty map so the rebuild restores
//!    the CLI-locked startup values (owner repro: `--scene cosmos`,
//!    config `scene = cinematic` + `ambient.12-00 = monolith`, both
//!    commented back out → engine stayed cinematic/monolith).
//! 2. Human duration forms (`45`, `45s`, `1m`, `1h30m`) on the two
//!    `-secs` config keys — one vocabulary with the CLI flags.
//! 3. Routing contracts (verbose-only diag channel for the watcher
//!    liveness line; the deferral-flag restore after a rebuild).

use super::tests::minimal_cloud_config;
use super::watcher::zero_key_is_deliberate;
use super::*;

use crate::configfile;
use std::sync::atomic::Ordering;

// ── 1. Zero-key classification (watcher-level) ──────────────────────

#[test]
fn zero_key_all_commented_content_is_deliberate() {
    // The owner's exact final config state: every key commented out.
    assert!(zero_key_is_deliberate(
        "# scene = cinematic\n# ambient.12-00 = monolith\n"
    ));
    // Comment header only — still a deliberate empty config.
    assert!(zero_key_is_deliberate("# cosmostrix config\n"));
    // Blank lines + comments mixed.
    assert!(zero_key_is_deliberate("\n\n# disabled everything\n\n"));
}

#[test]
fn zero_key_whitespace_only_is_transient() {
    // Truly empty / whitespace-only file: a non-atomic editor save in
    // progress (truncate → write). The old skip behavior is kept.
    assert!(!zero_key_is_deliberate(""));
    assert!(!zero_key_is_deliberate("   "));
    assert!(!zero_key_is_deliberate("\n\n\t  \n"));
}

// ── 2. Zero-key map passes validation + delivery (watcher-level) ────

#[test]
fn validate_and_send_accepts_deliberate_empty_map() {
    // The render thread must RECEIVE the empty map (Ok({})) for a
    // zero-key config: the rebuild's CLI-lock fallback depends on the
    // map arriving. Pre-alpha.2 the watcher dropped the event entirely.
    // NOTE: no rejection-log drain here (nothing is rejected) — draining
    // in parallel would break the locked log-assertion tests.
    let (tx, rx) = std::sync::mpsc::sync_channel(64);
    let parsed = configfile::parse_config_text("# scene = cinematic\n# ambient.12-00 = monolith\n");
    assert!(parsed.values.is_empty(), "fixture must be zero-key");
    assert!(validate_and_send(&parsed, &tx).is_ok());
    let delivered = rx
        .try_recv()
        .expect("empty map must be DELIVERED to the render thread");
    let map = delivered.expect("empty config is valid — Ok(map), not Err");
    assert!(
        map.is_empty(),
        "delivered map must be empty (all keys commented out)"
    );
    assert_eq!(
        LIVE_RELOAD_EXIT_CODE.load(Ordering::Acquire),
        0,
        "a deliberate empty config must not be treated as an error"
    );
}

// ── 3. Rebuild-level fallback: empty map → CLI-locked values ────────

#[test]
fn rebuild_empty_map_falls_back_to_cli_locked_scene() {
    // rebuild_cloud_config(base, {}) with a CLI-locked scene keeps the
    // base values verbatim (the fallback the zero-key delivery now
    // reaches). The owner's exact scenario: --scene cosmos locked,
    // config scene/ambient commented out → cosmos.
    let mut base = minimal_cloud_config();
    base.scene_name = "cosmos".to_string();
    base.cli_explicit.scene = true;

    let empty = HashMap::new();
    let after = rebuild_cloud_config(&base, &empty);
    assert_eq!(
        after.scene_name, "cosmos",
        "empty config map must fall back to the CLI-locked startup scene"
    );
}

// ── 4. Human duration forms on the -secs config keys ────────────────

#[test]
fn rebuild_applies_human_crystal_dragon_secs() {
    let base = minimal_cloud_config();
    // One form per family: suffixed, compound, bare, fractional.
    for (input, expect) in [
        ("15s", 15.0),
        ("1m", 60.0),
        ("1h30m", 5400.0),
        ("45", 45.0),
        ("45.5s", 45.5),
    ] {
        let mut cfg = HashMap::new();
        cfg.insert("crystal-dragon-secs".to_string(), input.to_string());
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(
            new.crystal_dragon_secs,
            Some(expect),
            "crystal-dragon-secs = {input} must resolve to {expect}s on live-reload"
        );
    }
}

#[test]
fn rebuild_applies_human_ambient_snapback_secs() {
    let base = minimal_cloud_config();
    for (input, expect) in [("10s", 10.0), ("30m", 1800.0), ("90", 90.0)] {
        let mut cfg = HashMap::new();
        cfg.insert("ambient-snapback-secs".to_string(), input.to_string());
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(
            new.ambient_snapback_secs,
            Some(expect),
            "ambient-snapback-secs = {input} must resolve to {expect}s on live-reload"
        );
    }
}

#[test]
fn rebuild_rejects_out_of_range_human_secs() {
    let base = minimal_cloud_config();
    let mut cfg = HashMap::new();
    cfg.insert("crystal-dragon-secs".to_string(), "25h".to_string()); // 90000 > 86400
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.crystal_dragon_secs, None,
        "25h exceeds the 86400s cap — must fall back to the base value"
    );
}

#[test]
fn field_validation_accepts_human_secs_forms() {
    // The strict validation (watcher + --testconf) must ACCEPT the human
    // forms or live-reload edits with them would hard-exit the session.
    for key in ["ambient-snapback-secs", "crystal-dragon-secs"] {
        for good in ["30", "30s", "1m", "1h30m", "86400"] {
            let mut cfg = HashMap::new();
            cfg.insert(key.to_string(), good.to_string());
            assert!(
                crate::testconf::validate_config_strictly(&cfg).is_ok(),
                "{key} = {good} must pass strict validation"
            );
        }
    }
}

#[test]
fn field_validation_rejects_bad_human_secs() {
    for key in ["ambient-snapback-secs", "crystal-dragon-secs"] {
        for bad in ["25h", "abc", "-5s", "6x"] {
            let mut cfg = HashMap::new();
            cfg.insert(key.to_string(), bad.to_string());
            assert!(
                crate::testconf::validate_config_strictly(&cfg).is_err(),
                "{key} = {bad} must fail strict validation"
            );
        }
    }
}

// ── 5. Routing / state contracts (source-scan, house pattern) ───────

/// Source-scan helper: read a source file from the crate root.
fn src(rel: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"))
}

#[test]
fn watcher_silent_line_routes_verbose_only() {
    // v80.0.0-alpha.2 (owner regression report): the informational
    // "native watcher silent" line must drain ONLY with --verbose
    // (push_runtime_diag), never the always-drained warning channel.
    // Mirrors the self-heal routing test (live_config_state.rs).
    let w = src("src/config/live_config/watcher.rs");
    // The `{elapsed}` placeholder only exists in the ACTUAL format string
    // (the doc comments quote the owner's paste with a literal 33s) — so
    // this anchors on the real call site, not the comments.
    let idx = w
        .find("native watcher silent {elapsed}")
        .expect("watcher.rs must contain the liveness format string");
    let before = &w[idx.saturating_sub(600)..idx];
    assert!(
        before.contains("push_runtime_diag"),
        "the liveness diagnostic must route through push_runtime_diag (verbose-only), window:\n{before}"
    );
    assert!(
        !before.contains("push_runtime_warning(&format!("),
        "the always-drained call form must be gone (comment mentions are fine), window:\n{before}"
    );
}

#[test]
fn rebuild_restores_ambient_deferral_flag_after_cloud_swap() {
    // v80.0.0-alpha.2: the fresh Cloud resets user_override_since_ambient
    // to false; apply_config_rebuild must restore the pre-rebuild value
    // right after the swap or the startup CLI deferral dies on the first
    // config edit (the rx/snapback paths then apply ambient instantly).
    let r = src("src/interactive/event_loop_config_rebuild.rs");
    let swap_idx = r
        .find("*cloud = new_cloud")
        .expect("rebuild must swap the cloud");
    // 1200 chars covers the explanatory comment between the swap and the
    // restore line.
    let window = &r[swap_idx..swap_idx + 1200];
    assert!(
        window.contains("cloud.user_override_since_ambient = preserve_user_override"),
        "the flag restore must sit right after the cloud swap, window:\n{window}"
    );
}

#[test]
fn snapback_guard_is_rate_limited() {
    // v80.0.0-alpha.2 hunt-find: the ground-truth guard must gate its
    // per-frame file read on the shared rate-limit budget (it previously
    // re-read the config EVERY FRAME — I/O burn + inotify flood).
    let a = src("src/interactive/event_loop_ambient.rs");
    assert!(
        a.contains("GROUND_TRUTH_MIN_INTERVAL_SECS"),
        "the guard must use the shared ground-truth budget constant"
    );
    let idx = a
        .find("guard_budget_ok")
        .expect("guard must compute guard_budget_ok");
    let window = &a[idx..idx + 400];
    assert!(
        window.contains("last_ground_truth_check"),
        "guard_budget_ok must be derived from the shared timestamp"
    );
}
