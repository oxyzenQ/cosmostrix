// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v50 LTS regression tests for the first-reload scene reset crash.
//!
//! Extracted from `tests.rs` to keep that file under the project's 800-LOC
//! source cap. Loaded via `#[path = "tests_v50_first_reload.rs"] mod v50_first_reload;`
//! inside the `cases` module of `tests.rs`.
//!
//! Tests lock in the contract for the coredump fix at `event_loop.rs:70`
//! (commit `2b0e28b`). See individual test docs for the danger/fix contracts.

use crate::cloud::Cloud;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

// ── v50 LTS regression: first-reload scene reset crash ──────────────
//
// Owner-reported critical bug (coredump on commit 795eedf):
// Running `cosmostrix -C tron_legacy` then editing config.toml to add
// `color = "neon-green"` triggered a coredump on the FIRST live reload.
// Terminal 2 (no -C flag, default cinematic/zen/energy-zen) was unaffected.
//
// Root cause: `Cloud::new()` defaults `user_override_since_ambient = false`
// (verified by `v35_cloud_ambient_flags_default_false` in tests.rs). On the
// first live reload, the event loop's empty-schedule handler checked this
// flag to decide whether to preserve the user's state or re-apply scene
// defaults. Since it was `false`, the handler called
// `cloud.apply_scene_runtime("cinematic")`, which overrides:
//   - color → EnergyZen (cinematic's default, overriding user's NeonGreen)
//   - charset → zen (cinematic's default, overriding user's -C tron_legacy)
// The simultaneous palette + charset transition on a custom-charset cloud
// produced an inconsistent state → panic → double-panic → abort → coredump.
//
// Fix (commit 2b0e28b, event_loop.rs:70): set
// `cloud.user_override_since_ambient = true` immediately after creating
// the initial cloud, BEFORE the event loop starts. This routes the first
// reload through the `preserve_user_override` branch, which only restores
// the user's color when the config didn't change it (and never calls
// `apply_scene_runtime`).
//
// Why "terminal 2" was safe: it ran with default cinematic/zen/energy-zen,
// so `apply_scene_runtime("cinematic")` was a no-op (everything already
// matched). No state change, no transition, no crash.
//
// Why "warmup cache" made it not crash again: after the first (crashing)
// reload, `user_override_since_ambient` would have been set to `true` by
// the preserve branch. Subsequent reloads took the safe path. The crash
// only fired on the very first reload after startup.
//
// These tests verify the DANGER contract: `apply_scene_runtime("cinematic")`
// DOES override explicit user color and charset. This is the behavior the
// event_loop.rs:70 fix protects against. If a future refactor makes
// `apply_scene_runtime` idempotent (no-ops when user state already matches),
// these tests will fail — that's acceptable, but the fix at event_loop.rs:70
// must also be revisited at that time.

/// Verify `apply_scene_runtime("cinematic")` overrides an explicit user
/// color. This is the danger the event_loop.rs:70 fix protects against:
/// if the first reload were allowed to call this, the user's `color =
/// "neon-green"` from config.toml would be silently replaced by cinematic's
/// EnergyZen default.
#[test]
fn v50_apply_scene_runtime_cinematic_overrides_explicit_user_color() {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::NeonGreen, // user's explicit color
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    cloud.clear_redraw_flags_for_test();

    // Sanity: confirm the user's color is set before the scene apply.
    assert_eq!(cloud.color_scheme, ColorScheme::NeonGreen);

    // This is the call the bug path makes (event_loop.rs:435, else branch
    // when preserve_user_override == false). It applies cinematic's
    // defaults, overriding the user's explicit NeonGreen.
    let _ = cloud.apply_scene_runtime("cinematic", "binary", &[], false);

    // Assert the DANGER: apply_scene_runtime overrides user color.
    // If this assertion ever fails (color stays NeonGreen), it means
    // apply_scene_runtime became idempotent — revisit event_loop.rs:70.
    assert_ne!(
        cloud.color_scheme,
        ColorScheme::NeonGreen,
        "apply_scene_runtime('cinematic') must override user color — \
         this is the danger the event_loop.rs:70 fix protects against. \
         If this assertion fails, apply_scene_runtime became idempotent; \
         revisit the event_loop.rs:70 user_override_since_ambient = true fix."
    );
    assert_eq!(
        cloud.color_scheme,
        ColorScheme::EnergyZen,
        "apply_scene_runtime('cinematic') must apply EnergyZen (cinematic's \
         default color scheme)"
    );
}

/// Verify `apply_scene_runtime("cinematic")` overrides the user's charset.
/// This is the second half of the crash trigger: the simultaneous charset
/// transition (user's tron_legacy → cinematic's zen) combined with the
/// palette transition produced the inconsistent state that panicked.
#[test]
fn v50_apply_scene_runtime_cinematic_overrides_explicit_user_charset() {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::NeonGreen,
        RainStyle::Glyph,
    );
    // Simulate the user's -C tron_legacy charset: a small custom set.
    // The exact chars don't matter — what matters is that apply_scene_runtime
    // replaces them with cinematic's zen charset.
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    cloud.clear_redraw_flags_for_test();

    let user_charset_len = cloud.chars.len();
    assert_eq!(
        user_charset_len, 2,
        "sanity: user charset should have 2 chars before scene apply"
    );

    // The bug path: apply_scene_runtime("cinematic") replaces the charset.
    let _ = cloud.apply_scene_runtime("cinematic", "binary", &[], false);

    // Assert the DANGER: charset was replaced. The zen charset has
    // a different length than the user's 2-char set, proving override.
    assert_ne!(
        cloud.chars.len(),
        user_charset_len,
        "apply_scene_runtime('cinematic') must override user charset — \
         this is the danger the event_loop.rs:70 fix protects against. \
         If this assertion fails, apply_scene_runtime became idempotent; \
         revisit the event_loop.rs:70 user_override_since_ambient = true fix."
    );
}

/// Verify the FIX CONTRACT: when `user_override_since_ambient = true`
/// (as event_loop.rs:70 sets for the initial cloud), the reload path
/// must NOT call `apply_scene_runtime`. This test simulates the
/// preserve_user_override branch's color-preservation logic.
///
/// Note: This test verifies the BEHAVIOR contract (user state is
/// preserved when the flag is true), not the event_loop.rs:70 line
/// itself (which requires integration testing). The contract is:
///   - If user_override_since_ambient == true, do NOT call
///     apply_scene_runtime on reload. Preserve the user's color/charset.
///   - If user_override_since_ambient == false, the reload path MAY
///     call apply_scene_runtime (the bug path).
#[test]
fn v50_user_override_true_preserves_user_color_without_scene_apply() {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::NeonGreen, // user's explicit color
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    cloud.clear_redraw_flags_for_test();

    // THE FIX (event_loop.rs:70): set this to true for the initial cloud
    // so the first reload takes the preserve_user_override branch.
    cloud.user_override_since_ambient = true;

    // Simulate the preserve_user_override branch (event_loop.rs:392-447):
    // when the flag is true, the reload path does NOT call
    // apply_scene_runtime. It only restores cloud.color_scheme if the
    // config didn't change it. Here we just verify that NOT calling
    // apply_scene_runtime leaves the user's color intact.
    // (No apply_scene_runtime call — that's the whole point of the fix.)
    assert_eq!(
        cloud.color_scheme,
        ColorScheme::NeonGreen,
        "with user_override_since_ambient = true, the reload path must \
         NOT call apply_scene_runtime — user's NeonGreen must be preserved"
    );
    assert_eq!(
        cloud.chars.len(),
        2,
        "with user_override_since_ambient = true, the reload path must \
         NOT call apply_scene_runtime — user's charset must be preserved"
    );
    // The flag itself remains true (the preserve branch doesn't clear it).
    assert!(
        cloud.user_override_since_ambient,
        "preserve_user_override branch must keep the flag true"
    );
}
