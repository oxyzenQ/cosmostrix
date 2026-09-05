// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Validation-order regression tests (v80.0.0-beta.2, S-master-HUNT-2).
//!
//! The owner's cp77x bug (2026-09-02): `validate_config_strictly` used
//! to `break` out of its ENTIRE per-key loop after validating the first
//! `ambient.*` key it happened to reach. HashMap iteration order is
//! seed-randomized per instance, so a config pairing `ambient.12-00`
//! with an invalid `scene-custom.<name>.color` was rejected on some
//! parses and silently blessed on others (measured: 11 reject /
//! 9 silent over 20 startups). The live-reload watcher parses on its
//! own thread — a different seed — so the same file validated
//! differently there, which is why the owner's error only surfaced
//! after a second config touch (the "need 2x trigger" symptom).
//!
//! These tests pin the fix: full coverage regardless of key order, and
//! a deterministic first-error selection (ambient pre-pass + sorted
//! per-key iteration).

use std::collections::HashMap;

use super::validate_config_strictly;

/// The owner's exact config shape (2026-09-02 report): a complete v2
/// [scene-custom.cp77] block (builtin-pair form) with its custom palette
/// and charset blocks plus an ambient entry. `color` is parameterized so
/// the same builder covers the valid ("cp77") and typo'd ("cp77x")
/// forms.
fn owner_cp77_config(color: &str) -> HashMap<String, String> {
    let mut cfg = HashMap::new();
    for (k, v) in [
        ("scene-custom.cp77.rain", "lorenz"),
        ("scene-custom.cp77.color", color),
        ("scene-custom.cp77.charset", "ascii"),
        ("scene-custom.cp77.fps", "90"),
        ("scene-custom.cp77.speed", "12"),
        ("scene-custom.cp77.density", "0.90"),
        ("scene-custom.cp77.glitch-level", "none"),
        ("colors-custom.cp77.bg", "#0A0008"),
        (
            "colors-custom.cp77.rain",
            "#FFE100,#FF6B00,#FF0066,#FF00CC,#CC00FF,#00FFFF,#E0E0E0",
        ),
        ("charset-custom.cp77.set", "012"),
        ("ambient.12-00", "cp77"),
    ] {
        cfg.insert(k.to_string(), v.to_string());
    }
    cfg
}

#[test]
fn strict_validation_rejects_invalid_block_color_with_ambient_present() {
    // Owner bug: `color = "cp77x"` was silently ignored whenever the
    // HashMap iteration reached `ambient.12-00` before the scene-custom
    // keys (the loop `break`ed, skipping them). The rain then kept the
    // startup scene's default color (HUD `clr: carbon`) instead of
    // exiting 2. The ambient pre-pass + full-coverage loop make the
    // rejection deterministic: EVERY parse rejects, from ANY thread.
    let cfg = owner_cp77_config("cp77x");
    let err = validate_config_strictly(&cfg)
        .expect_err("invalid scene-custom color must be rejected with ambient keys present");
    assert!(
        err.contains("invalid value 'cp77x' for 'scene-custom.cp77.color'"),
        "error must name the block field: {err}"
    );
    assert!(
        err.contains("unknown color 'cp77x'"),
        "error must be the unknown-color rejection: {err}"
    );
}

#[test]
fn strict_validation_accepts_custom_palette_color_with_ambient_present() {
    // The valid twin of the owner's config: `color = "cp77"` resolves
    // to the [colors-custom.cp77] block. Ambient entries + a complete
    // block must still pass — the pre-pass must not over-reject.
    let cfg = owner_cp77_config("cp77");
    assert!(
        validate_config_strictly(&cfg).is_ok(),
        "valid config with ambient + complete block must pass"
    );
}

#[test]
fn strict_validation_is_deterministic_across_hash_seeds() {
    // Same content, two separately-seeded HashMaps (std RandomState
    // bumps its per-thread seed per instance), reversed insertion order
    // in the second. Multiple invalid keys compete for the first-error
    // slot: the sorted iteration must pick the SAME one every time,
    // from every map instance — this is what makes startup (main
    // thread) and live-reload (watcher thread) agree on one file.
    let mut a = owner_cp77_config("cp77x");
    a.insert("speed".to_string(), "999".to_string());
    let mut b = HashMap::new();
    let pairs: Vec<(&String, &String)> = a.iter().collect();
    for (k, v) in pairs.into_iter().rev() {
        b.insert(k.clone(), v.clone());
    }
    let err_a = validate_config_strictly(&a).expect_err("a must be invalid");
    let err_b = validate_config_strictly(&b).expect_err("b must be invalid");
    assert_eq!(
        err_a, err_b,
        "same config must produce the same first error"
    );
    // Sorted order: "scene-custom.cp77.color" < "speed", so the block
    // color error wins deterministically.
    assert!(
        err_a.contains("invalid value 'cp77x'"),
        "sorted order must pick the block-color error first: {err_a}"
    );
}

#[test]
fn strict_validation_reports_ambient_error_first_regardless_of_order() {
    // An invalid ambient entry + an invalid top-level key: the ambient
    // pre-pass runs before the per-key loop, so the ambient error wins
    // deterministically (before the fix, whichever key the HashMap
    // reached first won — and the `break` could hide the loser
    // entirely).
    let mut cfg = owner_cp77_config("cp77");
    cfg.insert("ambient.23-59".to_string(), "no-such-scene".to_string());
    cfg.insert("fps".to_string(), "999".to_string());
    let err = validate_config_strictly(&cfg)
        .expect_err("invalid ambient entry + invalid fps must be rejected");
    assert!(
        err.contains("no-such-scene") && err.contains("ambient.23-59"),
        "ambient error must surface first: {err}"
    );
}

// ── v80.0.0-alpha.1: crystal-dragon-secs strict validation ────────────────

#[test]
fn strict_validation_rejects_out_of_range_crystal_dragon_secs() {
    // The new harmony knob must validate on ALL surfaces (startup,
    // --testconf, live-reload) exactly like ambient-snapback-secs —
    // a typo'd magnitude (999999) is a hard rejection, not a silent
    // fallback to the 60s default.
    let mut cfg = owner_cp77_config("cp77");
    cfg.insert("crystal-dragon-secs".to_string(), "999999".to_string());
    let err = validate_config_strictly(&cfg)
        .expect_err("out-of-range crystal-dragon-secs must be rejected");
    assert!(
        err.contains("crystal-dragon-secs") && err.contains("86400"),
        "error must name the key and the range: {err}"
    );
}

#[test]
fn strict_validation_rejects_non_numeric_crystal_dragon_secs() {
    let mut cfg = owner_cp77_config("cp77");
    cfg.insert("crystal-dragon-secs".to_string(), "fast".to_string());
    let err = validate_config_strictly(&cfg)
        .expect_err("non-numeric crystal-dragon-secs must be rejected");
    assert!(
        err.contains("crystal-dragon-secs"),
        "error must name the key: {err}"
    );
}

#[test]
fn strict_validation_accepts_crystal_dragon_secs_bounds() {
    for v in ["0", "45.5", "86400"] {
        let mut cfg = owner_cp77_config("cp77");
        cfg.insert("crystal-dragon-secs".to_string(), v.to_string());
        assert!(
            validate_config_strictly(&cfg).is_ok(),
            "crystal-dragon-secs={v} must be accepted (0.0..=86400.0)"
        );
    }
}
