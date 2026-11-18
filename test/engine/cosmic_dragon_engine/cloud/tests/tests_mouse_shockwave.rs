// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! Mouse-click shockwave tests (night-frontier-Z4b).
//!
//! The click ripple is a solved 2D wave equation on the cell grid: a
//! click injects an impulse into `Cloud::wave_field`, the solver
//! (leapfrog FDTD + Mur absorbing edges + time-centered damping)
//! advances it once per frame, and the renderer reads the solved
//! amplitudes. These tests cover the Cloud-level wiring (injection,
//! superposition, settle, the --no-effects gate); the solver's
//! physics pins (CFL, speed, damping life, zero-alloc, stability)
//! live in `tests_wave_field.rs`.

use super::super::Cloud;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

/// Build a TrueColor cloud so palette body colors are distinct per scheme.
fn make_truecolor_cloud(scheme: ColorScheme) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        scheme,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    cloud
}

/// A single click arms the field with energy at the clicked cell.
#[test]
fn mouse_shockwave_single_click_injects_energy() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    assert!(
        cloud.wave_field.is_settled(),
        "a fresh cloud's field must start settled"
    );
    cloud.set_mouse_click(5, 5);
    assert!(
        !cloud.wave_field.is_settled(),
        "one click must arm the field (not settled)"
    );
    let (cols, lines) = cloud.wave_field.dims();
    assert_eq!((cols, lines), (20, 10), "the field must match the grid");
    let amp = cloud.wave_field.amplitudes();
    let idx = 5 * cols + 5;
    assert!(
        amp[idx] > 0.0,
        "the clicked cell must carry injected energy (got {})",
        amp[idx]
    );
    assert!(
        amp[idx] <= 2.0,
        "the injection must stay inside the amplitude cap (got {})",
        amp[idx]
    );
}

/// A double click superposes: BOTH origins carry energy. The wave
/// equation is linear — no slot eviction, no reset-on-second-click
/// (the regression the old bounded pool existed for, now obsolete).
#[test]
fn mouse_shockwave_double_click_superposes() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    cloud.set_mouse_click(5, 5);
    cloud.set_mouse_click(15, 7);
    let (cols, _) = cloud.wave_field.dims();
    let amp = cloud.wave_field.amplitudes();
    assert!(
        amp[5 * cols + 5] > 0.0,
        "first click origin must hold energy"
    );
    assert!(
        amp[7 * cols + 15] > 0.0,
        "second click origin must hold energy (superposition, not eviction)"
    );
    assert!(
        !cloud.wave_field.is_settled(),
        "superposed clicks keep the field armed"
    );
}

/// A click storm saturates instead of blowing up: many clicks keep the
/// amplitude capped, and the field still decays to settled under the
/// damping physics within the documented life window.
#[test]
fn mouse_shockwave_click_storm_caps_and_decays() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    for i in 0..50u16 {
        cloud.set_mouse_click(i % 20, (i / 20) % 10);
    }
    let peak: f32 = cloud
        .wave_field
        .amplitudes()
        .iter()
        .fold(0.0f32, |m, &v| m.max(v.abs()));
    assert!(
        peak <= 2.0 + 1e-3,
        "the click storm must saturate at the inject cap, not blow up (peak {peak})"
    );
    // Step the field at 60 fps for the documented damping life
    // (2*ln(A0/eps)/k from the inject cap A0=2 is ~125 frames; the
    // storm's spatial spread lags the peak slightly — 200 frames is
    // the documented-window margin).
    for _ in 0..200 {
        cloud.wave_field.step(1.0 / 60.0);
    }
    assert!(
        cloud.wave_field.is_settled(),
        "the field must settle below the visible floor inside the damping window"
    );
}

/// The solver never destabilizes: long runs of steps on an armed field
/// keep the amplitude bounded (the CFL margin in action).
#[test]
fn mouse_shockwave_solver_stays_bounded() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    cloud.set_mouse_click(10, 5);
    let mut peak = 0.0f32;
    for _ in 0..600 {
        cloud.wave_field.step(1.0 / 60.0);
        let p = cloud
            .wave_field
            .amplitudes()
            .iter()
            .fold(0.0f32, |m, &v| m.max(v.abs()));
        peak = peak.max(p);
        assert!(
            p <= 2.0 + 1e-3,
            "amplitude escaped the inject cap mid-run ({p}) — CFL violation"
        );
    }
    assert!(
        peak > 0.5,
        "the wave must have been clearly visible (peak {peak})"
    );
}

// ─── PERF-4 strengthen: --no-effects full-disable lock tests ───────────────
//
// The mouse-click shockwave and the anomaly zones must stay no-ops under
// --no-effects. These lock tests prevent future refactors from silently
// reintroducing the leak.

/// --no-effects must suppress shockwave injection on mouse click.
/// Regression: before the strengthen, `set_mouse_click` unconditionally
/// activated the wave — the click overlay continued to render even with
/// `--no-effects` set.
#[test]
fn no_effects_disables_mouse_click_shockwave() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    cloud.set_effects_enabled(false);
    cloud.set_mouse_click(5, 5);
    assert!(
        cloud.wave_field.is_settled(),
        "no shockwave energy should be injected under --no-effects"
    );
    // Quantum ripple pool must also stay empty (separate gate on
    // spawn_quantum_ripple, but click path now early-returns before
    // even calling it — so this is a defense-in-depth check).
    let particles = cloud.quantum_particles.iter().filter(|p| p.active).count();
    assert_eq!(
        particles, 0,
        "no quantum particles should be active under --no-effects (got {particles})"
    );
}

/// --no-effects must suppress border spark activation.
/// Pre-existing gate (PERF-4 original) — lock test prevents regression.
#[test]
fn no_effects_disables_border_spark() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    cloud.set_effects_enabled(false);
    cloud.spawn_border_spark(5, 0, (255, 255, 255));
    let particles = cloud.quantum_particles.iter().filter(|p| p.active).count();
    assert_eq!(
        particles, 0,
        "no border-spark particles should be active under --no-effects (got {particles})"
    );
}

/// --no-effects must suppress anomaly zone spawning.
/// Regression: before the strengthen, `spawn_anomaly` was unguarded —
/// so LuminanceSurge / GlyphCorruption / PulseWave zones continued to
/// appear under --no-effects.
#[test]
fn no_effects_disables_anomaly_zones() {
    use std::time::Instant;
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    cloud.set_effects_enabled(false);
    let now = Instant::now();
    // Hammer spawn_anomaly — even with generous randomness, no zone
    // should ever be pushed while effects are disabled.
    for _ in 0..100 {
        cloud.spawn_anomaly(now);
    }
    assert!(
        cloud.anomaly_zones.is_empty(),
        "anomaly_zones must stay empty under --no-effects (got {} zones)",
        cloud.anomaly_zones.len()
    );
}

/// Effects default ON: a fresh Cloud must have effects_enabled == true
/// so the CLI default (no --no-effects flag) keeps the visual identity.
#[test]
fn effects_enabled_default_is_true() {
    let cloud = make_truecolor_cloud(ColorScheme::Green);
    assert!(
        cloud.effects_enabled,
        "effects_enabled must default to true (CLI default = effects on)"
    );
}

/// v80.0.0-alpha.1 (--no-effects audit closure): the cursor hover glow
/// must be suppressed under --no-effects. set_mouse_position stores the
/// u16::MAX "no cursor" sentinel when effects are off, so the droplet
/// draw path's `ctx.mouse_col != u16::MAX` gate never fires (the
/// per-cell elliptical-brightening math near the cursor was the last
/// un-gated cosmetic found by the audit, together with the CRT vignette).
#[test]
fn no_effects_stores_mouse_sentinel_suppressing_hover_glow() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    cloud.set_effects_enabled(false);
    cloud.set_mouse_position(7, 3);
    assert_eq!(
        cloud.mouse_col,
        u16::MAX,
        "mouse_col must hold the no-cursor sentinel under --no-effects"
    );
    assert_eq!(
        cloud.mouse_line,
        u16::MAX,
        "mouse_line must hold the no-cursor sentinel under --no-effects"
    );

    // Effects re-enabled → position tracking works again (the sentinel
    // is written per-event, not latched).
    cloud.set_effects_enabled(true);
    cloud.set_mouse_position(7, 3);
    assert_eq!(cloud.mouse_col, 7);
    assert_eq!(cloud.mouse_line, 3);
}
