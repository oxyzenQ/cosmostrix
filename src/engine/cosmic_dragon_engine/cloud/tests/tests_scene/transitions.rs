// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Scene transition tests — monolith↔glyph switches, dirty-frame behavior,
//! semantic invalidation, spawn debt clearing, force draw.

use std::time::{Duration, Instant};

use super::{has_dirty_cells, make_glyph_cloud, make_monolith_cloud};
use crate::frame::Frame;
use crate::rain_style::RainStyle;

#[test]
fn monolith_to_matrix_changes_rain_style_to_glyph() {
    let mut cloud = make_monolith_cloud();
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
}

#[test]
fn matrix_to_monolith_changes_rain_style_to_monolith() {
    let mut cloud = make_glyph_cloud();
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
}

#[test]
fn switching_from_monolith_clears_draw_history() {
    let mut cloud = make_monolith_cloud();
    // Simulate some monolith draw activity
    cloud.monolith_rain.reset(40);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    // Draw history should be empty after switching away from monolith
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
}

#[test]
fn switching_into_monolith_initializes_state_cleanly() {
    let mut cloud = make_glyph_cloud();
    cloud.droplets.clear();
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    // Monolith should be reset and ready
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    assert_eq!(cloud.active_scene(), "monolith");
}

#[test]
fn scene_switch_requests_semantic_invalidate() {
    let mut cloud = make_monolith_cloud();
    cloud.clear_redraw_flags_for_test();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert!(cloud.is_semantic_invalidate());
}

#[test]
fn scene_switch_triggers_force_draw() {
    let mut cloud = make_monolith_cloud();
    cloud.clear_redraw_flags_for_test();
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    assert!(cloud.is_force_draw_everything());
}

/// Scene switch must request semantic invalidation for safe redraw sync.
#[test]
fn scene_switch_glyph_requests_semantic_sync() {
    let mut cloud = make_monolith_cloud();
    cloud.clear_redraw_flags_for_test();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert!(
        cloud.is_semantic_invalidate(),
        "glyph scene switch must request semantic invalidation"
    );
}

#[test]
fn scene_switch_drops_spawn_debt() {
    let mut cloud = make_monolith_cloud();
    cloud.spawn_remainder = 100.0;
    cloud.last_spawn_time = Instant::now() - Duration::from_secs(5);
    // Switching to monolith resets spawn debt
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    assert!(
        cloud.spawn_remainder < 1.0,
        "monolith scene switch should drop spawn debt"
    );
}

/// After switching monolith → matrix, the first rain frame must produce
/// visible dirty cells — no blank black intermediate screen.
#[test]
fn monolith_to_matrix_produces_dirty_glyph_frame() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    let now = Instant::now();
    cloud.last_spawn_time = now;
    cloud.rain_at(&mut frame, now + Duration::from_millis(16));
    assert!(
        has_dirty_cells(&frame),
        "monolith→matrix: first frame must have dirty glyph cells"
    );
}

/// After switching monolith → signal, the first rain frame must produce
/// visible dirty cells — no blank black intermediate screen.
#[test]
fn monolith_to_signal_produces_dirty_glyph_frame() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    let now = Instant::now();
    cloud.last_spawn_time = now;
    cloud.rain_at(&mut frame, now + Duration::from_millis(16));
    assert!(
        has_dirty_cells(&frame),
        "monolith→signal: first frame must have dirty glyph cells"
    );
}

/// After switching signal → monolith, the monolith scene should render
/// correctly (monolith has its own draw path, not glyph droplets).
#[test]
fn signal_to_monolith_produces_visible_frame() {
    let mut cloud = make_glyph_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("monolith", "braille", &[], false);
    let now = Instant::now();
    cloud.last_spawn_time = now;
    cloud.rain_at(&mut frame, now + Duration::from_millis(16));
    assert!(
        has_dirty_cells(&frame),
        "signal→monolith: first frame must have dirty cells"
    );
}

/// Switching monolith → matrix must clear monolith draw history so no
/// monolith segmented residue persists in the glyph scene.
#[test]
fn monolith_to_matrix_clears_monolith_history_no_blank() {
    let mut cloud = make_monolith_cloud();
    cloud.monolith_rain.reset(40);
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    // Monolith history must be empty
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    // Glyph pool must be populated (not blank)
    assert!(!cloud.droplets.is_empty());
    let alive_count = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert!(
        alive_count > 0,
        "warm-start should seed at least 1 active droplet (got {alive_count})"
    );
    // First frame must render visible content
    cloud.last_spawn_time = Instant::now();
    cloud.rain(&mut frame);
    assert!(has_dirty_cells(&frame));
}

/// Switching monolith → signal must clear monolith draw history and
/// produce visible glyph content on the first frame.
#[test]
fn monolith_to_signal_clears_monolith_history_no_blank() {
    let mut cloud = make_monolith_cloud();
    cloud.monolith_rain.reset(40);
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    assert!(!cloud.droplets.is_empty());
    cloud.last_spawn_time = Instant::now();
    cloud.rain(&mut frame);
    assert!(has_dirty_cells(&frame));
}

/// Repeated forward cycling (x key) through all scenes never yields
/// a blank frame. Each scene transition must produce dirty cells.
#[test]
fn repeated_forward_cycle_never_blank() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    let scenes = ["matrix", "signal", "monolith"];
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        frame.clear_dirty();
        cloud.last_spawn_time = Instant::now();
        cloud.rain(&mut frame);
        assert!(
            has_dirty_cells(&frame),
            "forward cycle: scene '{scene}' must produce dirty frame"
        );
    }
}

/// Repeated uppercase X cycling forward through all scenes never yields
/// a blank frame. Each scene transition must produce dirty cells.
#[test]
fn repeated_uppercase_forward_cycle_never_blank() {
    let mut cloud = make_monolith_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);
    let scenes = ["matrix", "signal", "monolith"];
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        frame.clear_dirty();
        cloud.last_spawn_time = Instant::now();
        cloud.rain(&mut frame);
        assert!(
            has_dirty_cells(&frame),
            "uppercase forward cycle: scene '{scene}' must produce dirty frame"
        );
    }
}

// ── apply_ambient_entry regressions ──
//
// ambient entries are now scene-name-only. The previous regression
// (color/charset/speed/density being silently lost) is impossible by
// construction — the scene IS the spec, so when ambient fires `signal`,
// all of signal's defaults (color/charset/speed/density/glitch) are applied
// atomically via apply_scene_runtime_with_cfg. There is no override layer
// to lose.
//
// These tests verify the  contract:
// 1. apply_ambient_entry with a built-in scene name applies that scene's
//    managed defaults.
// 2. apply_ambient_entry with a custom scene name looks up the
//    [scene-custom.<name>] block, applies base-scene defaults first, then
//    the block's own overrides.

use crate::cloud::Cloud;
use crate::crystal_dragon_engine::ambient::AmbientEntry;
use crate::runtime::ColorScheme;
use std::collections::HashMap;

/// Helper: build a glyph cloud whose starting state mirrors the cinematic
/// scene's defaults (color=energy-zen, charset=zen, speed=9.0, density=0.75).
fn make_cinematic_like_cloud() -> Cloud {
    let mut cloud = make_glyph_cloud();
    // Apply cinematic scene to mirror real startup state.
    cloud.apply_scene_runtime("cinematic", "zen", &[], false);
    // Verify our baseline assumptions — if these fail, the test setup is
    // wrong, not the bug.
    assert_eq!(cloud.color_scheme(), ColorScheme::EnergyZen);
    assert!((cloud.chars_per_sec - 9.0).abs() < 0.01);
    assert!((cloud.droplet_density - 0.75).abs() < 0.01);
    cloud
}

#[test]
fn apply_ambient_entry_builtin_scene_applies_scene_defaults() {
    // ambient entry with a built-in scene name applies that scene's
    // managed defaults atomically. No override layer — the scene IS the spec.
    let mut cloud = make_cinematic_like_cloud();
    let entry = AmbientEntry {
        hour: 13,
        minute: 0,
        scene: "signal".to_string(),
    };
    let cfg = HashMap::new();
    let charset_preset = cloud.apply_ambient_entry(&entry, "zen", &[], false, &cfg);

    // signal scene's defaults: color=aurora, charset=retro, speed=14.0,
    // density=0.55. All should be applied.
    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::Aurora,
        "signal scene should set color=aurora"
    );
    assert_eq!(
        charset_preset, "retro",
        "signal scene should set charset=retro"
    );
    assert!(
        (cloud.chars_per_sec - 14.0).abs() < 0.01,
        "signal scene should set speed=14.0, got {}",
        cloud.chars_per_sec
    );
    assert!(
        (cloud.droplet_density - 0.55).abs() < 0.01,
        "signal scene should set density=0.55, got {}",
        cloud.droplet_density
    );
}

#[test]
fn apply_ambient_entry_custom_scene_applies_base_scene_then_overrides() {
    // ambient entry with a custom scene name looks up the
    // [scene-custom.<name>] block, applies base-scene defaults first,
    // then the block's own overrides.
    //
    // Setup: custom scene "afternoon" with base-scene=signal, color=cosmos,
    // speed=12.0. Should result in:
    //   - rain_style = signal's (Glyph)
    //   - color = cosmos (override)
    //   - charset = retro (from signal base)
    //   - speed = 12.0 (override)
    //   - density = 0.55 (from signal base)
    //   - glitch = signal's default (Default)
    let mut cloud = make_cinematic_like_cloud();
    let entry = AmbientEntry {
        hour: 15,
        minute: 0,
        scene: "afternoon".to_string(),
    };
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.afternoon.base-scene".to_string(),
        "signal".to_string(),
    );
    cfg.insert(
        "scene-custom.afternoon.color".to_string(),
        "cosmos".to_string(),
    );
    cfg.insert(
        "scene-custom.afternoon.speed".to_string(),
        "12.0".to_string(),
    );
    let charset_preset = cloud.apply_ambient_entry(&entry, "zen", &[], false, &cfg);

    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::Cosmos,
        "custom scene color=cosmos must override signal base's aurora"
    );
    assert_eq!(
        charset_preset, "retro",
        "custom scene with no charset field must inherit signal base's retro"
    );
    assert!(
        (cloud.chars_per_sec - 12.0).abs() < 0.01,
        "custom scene speed=12.0 must override signal base's 14.0, got {}",
        cloud.chars_per_sec
    );
    assert!(
        (cloud.droplet_density - 0.55).abs() < 0.01,
        "custom scene with no density must inherit signal base's 0.55, got {}",
        cloud.droplet_density
    );
}

#[test]
fn apply_ambient_entry_custom_scene_without_base_scene_uses_glyph_rain() {
    // a custom scene with no base-scene falls back to Glyph rain
    // style and applies only the block's own overrides. Missing fields
    // retain the cloud's current state (no reset to defaults).
    let mut cloud = make_cinematic_like_cloud();
    let entry = AmbientEntry {
        hour: 18,
        minute: 0,
        scene: "minimal".to_string(),
    };
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.minimal.color".to_string(),
        "neon-green".to_string(),
    );
    // No base-scene, no speed/density — those retain current values.
    let _ = cloud.apply_ambient_entry(&entry, "zen", &[], false, &cfg);

    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::NeonGreen,
        "custom scene color=neon-green must be applied"
    );
    // Speed and density should NOT change (no base-scene, no override).
    assert!(
        (cloud.chars_per_sec - 9.0).abs() < 0.01,
        "custom scene with no base-scene and no speed must retain current 9.0, got {}",
        cloud.chars_per_sec
    );
    assert!(
        (cloud.droplet_density - 0.75).abs() < 0.01,
        "custom scene with no base-scene and no density must retain current 0.75, got {}",
        cloud.droplet_density
    );
}

#[test]
fn apply_ambient_entry_unknown_scene_is_noop() {
    // an unknown scene name (not built-in, no [scene-custom.<name>]
    // block) is a no-op — current state is preserved. This matches the
    // apply_scene_runtime contract for unknown scenes.
    let mut cloud = make_cinematic_like_cloud();
    let entry = AmbientEntry {
        hour: 20,
        minute: 0,
        scene: "nonexistent-scene".to_string(),
    };
    let cfg = HashMap::new();
    let charset_preset = cloud.apply_ambient_entry(&entry, "zen", &[], false, &cfg);

    // Nothing should change.
    assert_eq!(cloud.color_scheme(), ColorScheme::EnergyZen);
    assert_eq!(charset_preset, "zen");
    assert!((cloud.chars_per_sec - 9.0).abs() < 0.01);
    assert!((cloud.droplet_density - 0.75).abs() < 0.01);
}

// ── apply_startup_ambient regression ( hotfix) ──
//
// Bug: `apply_startup_ambient` originally passed `&HashMap::new()` (empty
// cfg) to `apply_ambient_entry`. For custom-scene ambient targets, this
// silently broke the lookup: `apply_custom_scene_runtime` calls
// `collect_custom_scenes(cfg)` which returns an empty map → no custom block
// found → no-op. The function STILL returned `Some(entry)`, which made the
// event loop's dedup check skip the scheduler's first real fire. Net
// result: ambient never applied at startup until the user touched
// config.toml (which triggered live-reload, which DOES pass the real cfg).

use crate::crystal_dragon_engine::ambient::{apply_startup_ambient, AmbientSchedule};

#[test]
fn apply_startup_ambient_with_empty_cfg_is_noop_for_custom_scene() {
    // Regression proof: with an EMPTY cfg map, a custom-scene ambient target
    // cannot resolve. The cloud stays in its starting state. The function
    // still returns Some(entry) — this is the trap that caused the dedup
    // check to skip the scheduler's first real fire.
    let mut cloud = make_cinematic_like_cloud();
    let schedule = AmbientSchedule {
        entries: vec![AmbientEntry {
            hour: 0,
            minute: 0,
            scene: "afternoon".to_string(),
        }],
    };
    let empty_cfg = HashMap::new();
    let (charset_preset, entry) =
        apply_startup_ambient(&mut cloud, &schedule, "zen", &[], false, &empty_cfg);

    // The bug: entry is Some (claimed applied) but the cloud is unchanged.
    assert!(entry.is_some(), "entry should be returned even for no-op");
    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::EnergyZen,
        "empty cfg must NOT resolve custom scene — cloud stays cinematic"
    );
    assert_eq!(charset_preset, "zen");
}

#[test]
fn apply_startup_ambient_with_real_cfg_applies_custom_scene() {
    // Fix proof: with the REAL cfg map (containing the [scene-custom.<name>]
    // block), the custom scene resolves correctly at startup.
    let mut cloud = make_cinematic_like_cloud();
    let schedule = AmbientSchedule {
        entries: vec![AmbientEntry {
            hour: 0,
            minute: 0,
            scene: "afternoon".to_string(),
        }],
    };
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.afternoon.base-scene".to_string(),
        "signal".to_string(),
    );
    cfg.insert(
        "scene-custom.afternoon.color".to_string(),
        "cosmos".to_string(),
    );
    let (charset_preset, entry) =
        apply_startup_ambient(&mut cloud, &schedule, "zen", &[], false, &cfg);

    // The fix: entry is Some AND the cloud actually changes.
    assert!(
        entry.is_some(),
        "entry must be returned for an active phase"
    );
    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::Cosmos,
        "real cfg must resolve custom scene — cloud switches to cosmos"
    );
    assert_eq!(
        charset_preset, "retro",
        "base-scene=signal must inherit retro charset"
    );
}

// ── Quantum ripple consistency: same-palette scene cycle ──
//
// Bug: pressing `x` to cycle cinematic → monolith (both energy-zen, both
// zen charset) triggered a spurious 300ms palette transition wave because
// set_color_scheme and transition_chars were called unconditionally. During
// that 300ms window, apply_quantum_ripple's blend with cell.fg (which could
// be mid-transition old/new palette mix) produced inconsistent click effect
// colors — the "snow ice vs spark fire" bug.
//
// Fix: set_color_scheme has an internal no-op guard for same-scheme calls;
// transition_chars is guarded at the scene-runtime call site. Together they
// ensure same-palette + same-charset scene cycles do NOT start transition
// waves, keeping the quantum ripple blend base stable.

#[test]
fn same_palette_scene_cycle_does_not_trigger_color_transition() {
    let mut cloud = make_cinematic_like_cloud();
    // make_cinematic_like_cloud applies cinematic which starts a color
    // transition (Green → EnergyZen). Clear it so we start from a clean
    // settled state.
    cloud.transition_start = None;
    cloud.charset_transition_start = None;

    // Cycle cinematic → monolith. Both use color=energy-zen, charset=zen.
    // The only real change is rain_style (Glyph → Monolith).
    cloud.apply_scene_runtime("monolith", "zen", &[], false);

    // Color transition must NOT have started — palette is identical.
    assert!(
        cloud.transition_start.is_none(),
        "same-palette scene cycle must NOT trigger color transition \
         (cinematic and monolith both use energy-zen)"
    );
    // Charset transition must NOT have started — charset is identical.
    assert!(
        cloud.charset_transition_start.is_none(),
        "same-charset scene cycle must NOT trigger charset transition \
         (cinematic and monolith both use zen)"
    );
    // Color scheme is still EnergyZen.
    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::EnergyZen,
        "color scheme must remain energy-zen after cycling to monolith"
    );
}

#[test]
fn different_palette_scene_cycle_does_trigger_color_transition() {
    // Control test: cycling cinematic (energy-zen) → matrix (neon-green)
    // MUST trigger a color transition because the palette actually changes.
    // This verifies the guard is selective — it only skips same-scheme calls.
    let mut cloud = make_cinematic_like_cloud();
    cloud.transition_start = None;
    cloud.charset_transition_start = None;

    cloud.apply_scene_runtime("matrix", "matrix", &[], false);

    assert!(
        cloud.transition_start.is_some(),
        "different-palette scene cycle MUST trigger color transition \
         (cinematic=energy-zen, matrix=neon-green)"
    );
    assert_eq!(
        cloud.color_scheme(),
        ColorScheme::NeonGreen,
        "color scheme must switch to neon-green after applying matrix scene"
    );
}

#[test]
fn set_color_scheme_same_scheme_still_clears_stale_state() {
    // Direct callers of set_color_scheme (tests, `c` key, live config reload)
    // ALWAYS get the full behavior — palette rebuild, transition wave, stale
    // state cleanup — even when the scheme is unchanged. The same-scheme
    // no-op guard lives at the scene-runtime call site, NOT inside
    // set_color_scheme itself. This verifies the contract: the residue
    // cleanup test (monolith_color_and_charset_transitions_clear_stale_residue)
    // depends on set_color_scheme always clearing draw history.
    let mut cloud = make_cinematic_like_cloud();
    cloud.transition_start = None;
    let slot_before = cloud.active_palette_slot;

    cloud.set_color_scheme(ColorScheme::EnergyZen); // same as current

    // set_color_scheme unconditionally advances the palette slot and starts
    // a transition — even for same-scheme calls.
    assert!(
        cloud.transition_start.is_some(),
        "direct set_color_scheme call must start a transition even for same scheme"
    );
    assert_ne!(
        cloud.active_palette_slot, slot_before,
        "direct set_color_scheme call must advance palette slot even for same scheme"
    );
}

// ME-02 regression test for the mouse-effect state-leak bug.
//
// Owner report: "click mouse effect inconsistency after scene switch via 'x'"
// Run cosmostrix → click → spark/fire effect (correct). Switch cinematic →
// monolith via 'x' → click → slow / ice-crystal / stain (BUG). Restart via
// Space → click → spark/fire again (correct).
//
// Root cause: `reset_phosphor_state()` cleared the Vec-backed phosphor
// fields but not the two BitVecs (`phosphor_fresh`, `phosphor_in_active`).
// `Cloud::reset()` (Space key) DID clear them. So scene switch left stale
// `true` bits in `phosphor_in_active` → freshly-drawn cells in the new
// scene failed the `if !self.phosphor_in_active[pidx]` check in
// `phosphor_decay_pass` → never pushed onto `phosphor_active` → Pass 3
// never decayed them → cells kept their last-drawn color → visible stain.
//
// This test pre-populates the BitVecs via rain_at (any scene), then performs
// the exact owner repro path (cinematic → monolith) and asserts the BitVecs
// are cleared alongside the Vec-backed state.

#[test]
fn scene_switch_clears_phosphor_bitvecs() {
    let mut cloud = make_cinematic_like_cloud();
    let mut frame = Frame::new(40, 20, cloud.palette.bg);

    // Run a few frames so phosphor_decay_pass populates phosphor_fresh /
    // phosphor_in_active with `true` bits. Pre-condition: bits must be set
    // or the test is meaningless.
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    for i in 0..5 {
        let now = start + Duration::from_millis(i * 33);
        cloud.rain_at(&mut frame, now);
        frame.clear_dirty();
    }
    assert!(
        cloud.phosphor_fresh.any(),
        "precondition: phosphor_fresh must have set bits after rain_at"
    );
    assert!(
        cloud.phosphor_in_active.any(),
        "precondition: phosphor_in_active must have set bits after rain_at"
    );

    // Owner repro: scene switch cinematic → monolith via apply_scene_runtime.
    cloud.apply_scene_runtime("monolith", "zen", &[], false);

    // The fix: BitVecs must be cleared alongside the Vec-backed state.
    // Before ME-01, this assertion failed — the BitVecs retained their
    // pre-switch `true` bits, causing the visible "noda"/stain symptom.
    assert!(
        !cloud.phosphor_fresh.any(),
        "phosphor_fresh must be cleared on scene switch (ME-01 root cause of mouse-effect state leak)"
    );
    assert!(
        !cloud.phosphor_in_active.any(),
        "phosphor_in_active must be cleared on scene switch (ME-01 root cause of mouse-effect state leak)"
    );
}

#[test]
fn scene_switch_clears_flash_waves_and_quantum_particles() {
    // ME-03 + ME-04: stale mouse-click state (flash_waves + quantum_particles)
    // must be cleared on scene switch so the new scene starts clean. Without
    // this, in-flight flash waves from the previous scene render in the new
    // scene with the previous palette's snapshot color for up to 1.8s.
    let mut cloud = make_cinematic_like_cloud();
    // Simulate a mouse click — populate flash_waves + quantum_particles.
    cloud.set_mouse_click(10, 5);
    assert!(
        cloud.flash_waves.iter().any(|w| w.active),
        "precondition: at least one flash wave must be active after click"
    );
    assert!(
        cloud.quantum_active_count > 0,
        "precondition: quantum particles must be active after click"
    );

    // Scene switch cinematic → monolith.
    cloud.apply_scene_runtime("monolith", "zen", &[], false);

    // The fix: flash_waves + quantum_particles cleared.
    assert!(
        !cloud.flash_waves.iter().any(|w| w.active),
        "flash_waves must be cleared on scene switch (ME-03)"
    );
    assert_eq!(
        cloud.quantum_active_count, 0,
        "quantum_active_count must be reset on scene switch (ME-04)"
    );
}

// ── v80.0.0-beta.2 HUD honesty: ambient-fired custom palette name ──
// Owner bug (2026-09-02): an ambient entry firing a [scene-custom.<name>]
// block with `colors-custom` rendered the custom palette while the HUD
// `clr:` line kept showing the base-scene scheme name ("clr: Purple" for
// a storm-based block). The palette name must now travel with the
// palette through set_palette so the HUD can follow it.

#[test]
fn apply_ambient_entry_custom_scene_colors_custom_tracks_palette_name() {
    // Exact owner scenario: ambient fires cp77 (base-scene=storm,
    // colors-custom=cyberpunk_2077). The base layer sets the storm
    // scheme (Purple); the field layer must then activate the custom
    // palette AND record its name for the HUD clr: line.
    let mut cloud = make_cinematic_like_cloud();
    let entry = AmbientEntry {
        hour: 21,
        minute: 0,
        scene: "cp77".to_string(),
    };
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.cp77.base-scene".to_string(),
        "storm".to_string(),
    );
    cfg.insert(
        "scene-custom.cp77.colors-custom".to_string(),
        "cyberpunk_2077".to_string(),
    );
    cfg.insert(
        "colors-custom.cyberpunk_2077.bg".to_string(),
        "#0a0a12".to_string(),
    );
    cfg.insert(
        "colors-custom.cyberpunk_2077.rain".to_string(),
        "#00fff7,#ff003c".to_string(),
    );
    let _charset_preset = cloud.apply_ambient_entry(&entry, "zen", &[], false, &cfg);

    assert!(
        cloud.custom_palette_active,
        "ambient-fired colors-custom must activate the custom palette"
    );
    assert_eq!(
        cloud.custom_palette_name.as_deref(),
        Some("cyberpunk_2077"),
        "the palette NAME must be tracked so the HUD clr: line follows it \
         (pre-beta.2 it kept showing the storm base scheme 'Purple')"
    );
}

#[test]
fn apply_ambient_entry_custom_scene_color_field_custom_palette_tracks_name() {
    // Variant: the block references the custom palette through the
    // `color` field (scene_runtime resolves non-builtin color names via
    // the custom palette path).
    let mut cloud = make_cinematic_like_cloud();
    let entry = AmbientEntry {
        hour: 22,
        minute: 0,
        scene: "nightshift".to_string(),
    };
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.nightshift.base-scene".to_string(),
        "signal".to_string(),
    );
    cfg.insert(
        "scene-custom.nightshift.color".to_string(),
        "cyberpunk_2077".to_string(),
    );
    cfg.insert(
        "colors-custom.cyberpunk_2077.bg".to_string(),
        "#0a0a12".to_string(),
    );
    cfg.insert(
        "colors-custom.cyberpunk_2077.rain".to_string(),
        "#00fff7,#ff003c".to_string(),
    );
    let _charset_preset = cloud.apply_ambient_entry(&entry, "zen", &[], false, &cfg);
    assert!(cloud.custom_palette_active);
    assert_eq!(
        cloud.custom_palette_name.as_deref(),
        Some("cyberpunk_2077"),
        "color = <custom palette> inside an ambient-fired block must track the name"
    );
}
