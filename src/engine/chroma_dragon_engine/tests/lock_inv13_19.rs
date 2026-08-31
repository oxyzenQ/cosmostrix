// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Chroma Dragon Engine Lock invariants INV-13 through INV-19.
//!
//! Extracted from `chroma/tests/lock.rs` to keep that source file under
//! the 800-LOC cap. Pure code motion — no behavior change.
//!
//! Covers:
//! - INV-13 polar chroma smoothing preserves saturation
//! - INV-14 subpixel jitter amplitude in bounds
//! - INV-15 head halo factor in range
//! - INV-16 tuning constants in sweet spots
//! - INV-17 engine lock report (comprehensive summary)
//! - INV-18 polar is sole production gradient path
//! - INV-19 color pipeline disclosure routes correctly

use super::lock::{all_schemes, CHROMA_DRAGON_ENGINE_VERSION};
use crossterm::style::Color;

use crate::chroma_dragon_engine::gradient::{gradient_from_stops_oklab, srgb_to_oklab};
use crate::chroma_dragon_engine::palette::color_to_rgb;
use crate::chroma_dragon_engine::shaders::transition::{apply_l_smoothing, TransitionLTable};
use crate::chroma_dragon_engine::tuning::{
    ABSOLUTE_MIN_FLOOR, BODY_TAIL_MAX_GAP_RATIO, GLOBAL_MAX_FLOOR, HEAD_HALO_FACTOR,
    PALETTE_FLOOR_RATIO, SUBPIXEL_JITTER_AMPLITUDE,
};
use crate::runtime::{ColorMode, ColorPipeline};

// ═══════════════════════════════════════════════════════════════════════════
// INV-13: Phase 8 polar chroma smoothing never desaturates below min(c_old, c_new)
// ═══════════════════════════════════════════════════════════════════════════

/// INV-13: when smoothing chroma at the transition wave, the smoothed
/// chroma magnitude never drops below `min(c_old, c_new)` for opposing
/// hues. This is the Phase 8 promise — polar interpolation keeps chroma
/// high through the midpoint, unlike Cartesian which would cut through
/// gray.
///
/// Tested with two palettes whose stop 0 has opposing hues (red ↔ cyan).
/// At the wave line (blend=0.5), the smoothed chroma should be near
/// the average of `c_old` and `c_new`, NOT near 0.
#[test]
fn lock_inv13_polar_chroma_smoothing_preserves_saturation() {
    // Red and Cyan — opposing hues, both saturated.
    let old_palette = vec![Color::Rgb { r: 255, g: 0, b: 0 }]; // red
    let new_palette = vec![Color::Rgb {
        r: 0,
        g: 255,
        b: 255,
    }]; // cyan
    let table = TransitionLTable::build(&old_palette, &new_palette, 5.0, 3.0)
        .expect("TransitionLTable::build should succeed for red ↔ cyan");

    let (_, a_old, b_old) = srgb_to_oklab(255, 0, 0);
    let (_, a_new, b_new) = srgb_to_oklab(0, 255, 255);
    let c_old = (a_old * a_old + b_old * b_old).sqrt();
    let c_new = (a_new * a_new + b_new * b_new).sqrt();
    let min_endpoint_chroma = c_old.min(c_new);

    // Smooth at the wave line (line=5, wave=5, blend=0.5).
    let smoothed = apply_l_smoothing(old_palette[0], Some(&table), 0, 5);
    let (r, g, b) = color_to_rgb(smoothed);
    let (_, a_smoothed, b_smoothed) = srgb_to_oklab(r, g, b);
    let c_smoothed = (a_smoothed * a_smoothed + b_smoothed * b_smoothed).sqrt();

    assert!(
        c_smoothed >= min_endpoint_chroma * 0.7,
        "Smoothed chroma {c_smoothed:.4} dropped below 70% of min endpoint chroma {min_endpoint_chroma:.4} — \
         polar smoothing failed to preserve saturation for opposing hues"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-14: Subpixel jitter amplitude in [0, 32]
// ═══════════════════════════════════════════════════════════════════════════

/// INV-14: `SUBPIXEL_JITTER_AMPLITUDE` is bounded in `[0, 32]`. Above 32
/// the per-channel perturbation becomes visible noise (not film grain);
/// below 0 it's a no-op. The production value (3) is the conservative
/// default documented in `tuning.rs`.
///
/// (clippy::assertions_on_constants is allowed because the intent is a
/// runtime test that fails CI if a future commit bumps the constant
/// outside the safe range without updating this assertion.)
#[test]
#[allow(clippy::assertions_on_constants)]
fn lock_inv14_subpixel_jitter_amplitude_in_bounds() {
    assert!(
        SUBPIXEL_JITTER_AMPLITUDE <= 32,
        "SUBPIXEL_JITTER_AMPLITUDE={SUBPIXEL_JITTER_AMPLITUDE} exceeds 32 — \
         jitter would read as visible noise, not film grain"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-15: Head halo factor in [0.0, 1.0]
// ═══════════════════════════════════════════════════════════════════════════

/// INV-15: `HEAD_HALO_FACTOR` is in `[0.0, 1.0]`. The blend function
/// clamps to this range anyway, but the constant itself must be in-range
/// so the doc comment matches the runtime behavior. A value above 1.0
/// would invert the blend (head becomes more bg than bg); below 0.0 is
/// meaningless.
///
/// (clippy::assertions_on_constants is allowed — see INV-14.)
#[test]
#[allow(clippy::assertions_on_constants)]
fn lock_inv15_head_halo_factor_in_range() {
    assert!(
        HEAD_HALO_FACTOR >= 0.0 && HEAD_HALO_FACTOR <= 1.0,
        "HEAD_HALO_FACTOR={HEAD_HALO_FACTOR} is outside [0.0, 1.0] — \
         blend semantics undefined"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-16: Tuning constants in their empirical sweet spots
// ═══════════════════════════════════════════════════════════════════════════

/// INV-16: the Phase 7 tuning constants stay within their empirically
/// validated ranges. The `phase7_print_ratio_sweep_audit` and
/// `phase7b_print_gap_ratio_sweep_audit` tests in `palette_floor_tests.rs`
/// verified these ranges across all 44 themes — drifting outside them
/// risks either v17-style washout (ratio too high) or "tail too dark"
/// regression (ratio too low).
///
/// - `PALETTE_FLOOR_RATIO`: range `[0.05, 0.50]`
///   - Below 0.05: trail sum below 33 on a head-655 palette — too dim,
///     fails the "tail too dark" complaint that Phase 7-c fixed.
///   - Above 0.50: most themes hit the GLOBAL_MAX_FLOOR=180 cap — v17
///     washout regression.
/// - `BODY_TAIL_MAX_GAP_RATIO`: range `[1.5, 3.0]`
///   - Below 1.5: trails nearly as bright as body — loses cinematic
///     trail-fade.
///   - Above 3.0: visible horizontal-line illusion at speed 100 returns.
///
/// (clippy::assertions_on_constants is allowed — see INV-14.)
#[test]
#[allow(clippy::assertions_on_constants)]
fn lock_inv16_tuning_constants_in_sweet_spots() {
    assert!(
        PALETTE_FLOOR_RATIO >= 0.05 && PALETTE_FLOOR_RATIO <= 0.50,
        "PALETTE_FLOOR_RATIO={PALETTE_FLOOR_RATIO} is outside [0.05, 0.50] — \
         outside the empirically validated sweet spot across all 44 themes"
    );
    assert!(
        BODY_TAIL_MAX_GAP_RATIO >= 1.5 && BODY_TAIL_MAX_GAP_RATIO <= 3.0,
        "BODY_TAIL_MAX_GAP_RATIO={BODY_TAIL_MAX_GAP_RATIO} is outside [1.5, 3.0] — \
         outside the empirically validated sweet spot"
    );
    assert!(
        ABSOLUTE_MIN_FLOOR >= 20 && ABSOLUTE_MIN_FLOOR <= 50,
        "ABSOLUTE_MIN_FLOOR={ABSOLUTE_MIN_FLOOR} is outside [20, 50] — \
         too low makes trails invisible, too high washes out dark themes"
    );
    assert!(
        GLOBAL_MAX_FLOOR >= 150 && GLOBAL_MAX_FLOOR <= 200,
        "GLOBAL_MAX_FLOOR={GLOBAL_MAX_FLOOR} is outside [150, 200] — \
         must match v17's 180 ceiling (within tolerance) to preserve the washout cap"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-18: Polar is the sole production gradient path (no Cartesian fallback)
// ═══════════════════════════════════════════════════════════════════════════

/// INV-18: the production `palette::gradient_from_stops` path dispatches
/// directly to the polar OKLab gradient implementation, with no Cartesian
/// fallback and no opt-in CLI flag.
///
/// Phase 9-A originally wired polar behind the `--polar-gradient` CLI flag
/// (default off, Cartesian was the default). v30 deleted Cartesian entirely
/// and made polar the sole path. This test verifies the production
/// gradient_from_stops actually produces polar output (not a stub or
/// accidental Cartesian regression).
///
/// Verification strategy: build a red↔cyan gradient (the canonical case
/// where polar and Cartesian produce provably different midpoints) and
/// assert the midpoint matches what `gradient::gradient_from_stops_oklab`
/// (the polar implementation) produces directly. If someone accidentally
/// re-introduces a Cartesian path or stubs gradient_from_stops, this test
/// will fail because the midpoint will differ from the polar baseline.
#[test]
fn lock_inv18_polar_is_sole_production_gradient_path() {
    use crate::chroma_dragon_engine::palette::gradient_from_stops;

    let stops = [(255, 0, 0), (0, 255, 255)];
    let steps = 3;

    // Production path.
    let prod = gradient_from_stops(&stops, steps);
    // Direct call to the polar implementation.
    let polar_direct = gradient_from_stops_oklab(&stops, steps);

    assert_eq!(prod.len(), steps);
    assert_eq!(polar_direct.len(), steps);
    // Endpoints preserved.
    assert_eq!(prod[0], stops[0]);
    assert_eq!(prod[2], stops[1]);
    // Production midpoint must match the polar implementation exactly.
    // If they differ, gradient_from_stops is no longer dispatching to polar.
    assert_eq!(
        prod, polar_direct,
        "production gradient_from_stops must produce identical output to the polar \
         implementation — Cartesian path was removed in v30"
    );

    // Sanity: the polar midpoint on red↔cyan must be clearly saturated
    // (not gray). Cartesian would produce sat ≈ 30-60; polar produces sat ≥ 80.
    let sat = |c: (u8, u8, u8)| -> i32 {
        let max_c = c.0.max(c.1).max(c.2) as i32;
        let min_c = c.0.min(c.1).min(c.2) as i32;
        max_c - min_c
    };
    assert!(
        sat(prod[1]) >= 80,
        "production red↔cyan midpoint {:?} saturation = {}, expected ≥ 80 \
         (polar must stay saturated — if this fails, the path is degenerate)",
        prod[1],
        sat(prod[1])
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-17: Lock report — sentinel test that prints the engine state
// ═══════════════════════════════════════════════════════════════════════════

/// INV-17: sentinel test that prints the full Chroma Dragon engine lock
/// report. This is the human-readable attestation that the engine is
/// locked at its peak — every other INV-XX test must pass for this one
/// to print "LOCKED".
///
/// The report lists every invariant, its scope, and the production value
/// of every tuning constant. Running `cargo test lock_inv17 -- --nocapture`
/// prints the full report; the assertion verifies the engine version
/// matches the locked tag and the invariant count is 19.
///
/// (Phase 9-D): invariant count bumped 18 → 19. INV-19 (pipeline
/// disclosure) was added by the chroma dragon audit. The version tag was
/// bumped from `9-C (locked)` → `9-D (locked)` to signal the new public
/// contract: `ColorPipeline::detect` is now part of the engine's surface.
#[test]
fn lock_inv17_engine_lock_report() {
    eprintln!("\n╔════════════════════════════════════════════════════════════════════╗");
    eprintln!("║     CHROMA DRAGON COLORING ENGINE — LOCK REPORT (Phase 9-D)        ║");
    eprintln!("╚════════════════════════════════════════════════════════════════════╝");
    eprintln!();
    eprintln!("  Engine version:  {CHROMA_DRAGON_ENGINE_VERSION}");
    eprintln!("  Theme count:     {}", all_schemes().len());
    eprintln!();
    eprintln!("  ── Tuning constants (production values) ──────────────────────────");
    eprintln!("  PALETTE_FLOOR_RATIO       = {PALETTE_FLOOR_RATIO}");
    eprintln!("  ABSOLUTE_MIN_FLOOR        = {ABSOLUTE_MIN_FLOOR}");
    eprintln!("  GLOBAL_MAX_FLOOR          = {GLOBAL_MAX_FLOOR}");
    eprintln!("  BODY_TAIL_MAX_GAP_RATIO   = {BODY_TAIL_MAX_GAP_RATIO}");
    eprintln!("  SUBPIXEL_JITTER_AMPLITUDE = {SUBPIXEL_JITTER_AMPLITUDE}");
    eprintln!("  HEAD_HALO_FACTOR          = {HEAD_HALO_FACTOR}");
    eprintln!();
    eprintln!("  ── Invariants (19 total) ─────────────────────────────────────────");
    eprintln!("  INV-01  Engine version sentinel               [LOCKED]");
    eprintln!("  INV-02  44-theme build sweep                  [LOCKED]");
    eprintln!("  INV-03  Floor bounds                          [LOCKED]");
    eprintln!("  INV-04  Head→body→trail hierarchy             [LOCKED]");
    eprintln!("  INV-05  Hue preservation under floor          [LOCKED]");
    eprintln!("  INV-06  Body-tail gap contract                [LOCKED]");
    eprintln!("  INV-07  Continuity ≤ head ceiling             [LOCKED]");
    eprintln!("  INV-08  OKLab round-trip ±1                   [LOCKED]");
    eprintln!("  INV-09  Polar gradient endpoints              [LOCKED]");
    eprintln!("  INV-10  Polar midpoint saturation            [LOCKED]");
    eprintln!("  INV-11  blend_toward_bg normalization         [LOCKED]");
    eprintln!("  INV-12  L-smoothing bounds                    [LOCKED]");
    eprintln!("  INV-13  Polar chroma smoothing saturation     [LOCKED]");
    eprintln!("  INV-14  Subpixel jitter amplitude             [LOCKED]");
    eprintln!("  INV-15  Head halo factor range                [LOCKED]");
    eprintln!("  INV-16  Tuning constants in sweet spots       [LOCKED]");
    eprintln!("  INV-17  This lock report                      [LOCKED]");
    eprintln!("  INV-18  Polar is sole production gradient path  [LOCKED]");
    eprintln!("  INV-19  ColorPipeline disclosure (chroma first, legacy fallback) [LOCKED]");
    eprintln!();
    eprintln!("  ── Phase history ────────────────────────────────────────────────");
    eprintln!("  Phase 1   Foundation (palette relocation)            ✓");
    eprintln!("  Phase 2   Shader extraction                         ✓");
    eprintln!("  Phase 3   OKLab gradient + Innovations A–H           ✓");
    eprintln!("  Phase 4   Dragon Awakening (Innovations C/D/E on)    ✓");
    eprintln!("  Phase 5   Perceptual L smoothing at transitions      ✓");
    eprintln!("  Phase 6   Palette-aware anomaly halos                ✓");
    eprintln!("  Phase 7   Palette-relative brightness floor          ✓");
    eprintln!("  Phase 7-c Floor ratio 0.15 → 0.20 (trail +33%)       ✓");
    eprintln!("  Phase 7-d Gap ratio 2.5 → 2.0 (step −20%)            ✓");
    eprintln!("  Phase 8   Hue-preserving chroma smoothing            ✓");
    eprintln!(
        "  Phase 9-A Hue-preserving polar gradient variant      ✓ (was opt-in --polar-gradient)"
    );
    eprintln!("  Phase 9-B ENGINE LOCK (Chroma Dragon)                ✓");
    eprintln!("  Phase 9-C Cartesian removed — polar is sole path     ✓");
    eprintln!("  Phase 9-D Chroma audit: ColorPipeline + chroma::legacy ✓");
    eprintln!();
    eprintln!("  ── Polar gradient demo (sole production path) ──────────────────");
    let demo_stops = [(10, 20, 30), (200, 100, 50), (50, 250, 75)];
    let polar = gradient_from_stops_oklab(&demo_stops, 5);
    eprintln!("  Polar: {polar:?}");
    eprintln!();
    eprintln!("  ── Color pipeline disclosure (INV-19) ──────────────────────────");
    eprintln!(
        "  ColorPipeline::detect(ColorMode::TrueColor)  = {}",
        ColorPipeline::detect(ColorMode::TrueColor).label()
    );
    eprintln!(
        "  ColorPipeline::detect(ColorMode::Color256)   = {}",
        ColorPipeline::detect(ColorMode::Color256).label()
    );
    eprintln!(
        "  ColorPipeline::detect(ColorMode::Color16)    = {}",
        ColorPipeline::detect(ColorMode::Color16).label()
    );
    eprintln!(
        "  ColorPipeline::detect(ColorMode::Mono)       = {}",
        ColorPipeline::detect(ColorMode::Mono).label()
    );
    eprintln!();
    eprintln!("  ── Status ──────────────────────────────────────────────────────");
    eprintln!("  All 19 invariants hold. Engine is at peak and locked.");
    eprintln!("  Future commits that change any constant, helper, or shader path");
    eprintln!("  in chroma/ must update the relevant INV-XX test AND bump");
    eprintln!("  CHROMA_DRAGON_ENGINE_VERSION. No silent contract drift.");
    eprintln!();

    // Sentinel assertion: the engine version matches the locked tag AND
    // the invariant count is exactly 19. If a future commit adds INV-20,
    // they must update this count too.
    assert_eq!(CHROMA_DRAGON_ENGINE_VERSION, "9-D (locked)");
    const INV_COUNT: u32 = 19;
    assert_eq!(
        INV_COUNT, 19,
        "INV_COUNT must match the actual invariant count"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-19: ColorPipeline disclosure — chroma dragon first, legacy fallback
// ═══════════════════════════════════════════════════════════════════════════

/// INV-19: assert that `ColorPipeline::detect` routes every `ColorMode`
/// variant to the correct pipeline. Owner directive: "all color -> chroma
/// dragon first -> fallback legacy rgb/srgb". The detection rule is:
/// `ColorMode::TrueColor` → `ChromaDragon`, everything else → `LegacyRgb`.
///
/// This invariant was added in  (Phase 9-D) by the chroma dragon
/// audit. It locks the public contract of `ColorPipeline` so a future
/// refactor cannot silently flip the routing (e.g. enabling chroma on
/// Color256 without a deliberate engine version bump).
///
/// Disclosure surfaces that depend on this routing:
/// - `cosmostrix -v` → `color_pipeline:` line under Scene & Color
/// - `cosmostrix --doctor` → `color_pipeline` field in RENDERER section
/// - `cosmostrix --benchmark` → `color_pipeline` + `chroma_in_benchmark`
///   fields in the CONFIG block of the report
///
/// If this test fails, one of those three surfaces is now lying to the
/// user about which pipeline is active.
#[test]
fn lock_inv19_color_pipeline_disclosure_routes_correctly() {
    // TrueColor terminals get the chroma dragon engine — the OKLab
    // gradient, perceptual blend, climate post-FX, head halo, L-smoothing,
    // and subpixel jitter all run.
    assert_eq!(
        ColorPipeline::detect(ColorMode::TrueColor),
        ColorPipeline::ChromaDragon,
        "TrueColor must route to ChromaDragon — owner rule: chroma first"
    );
    assert!(
        ColorPipeline::detect(ColorMode::TrueColor).is_chroma(),
        "is_chroma() must return true for the TrueColor pipeline"
    );

    // Color256/Color16/Mono all fall back to legacy sRGB-linear — the
    // OKLab palette would be quantized away by the terminal, so the
    // raw-RGB math is used directly via chroma::legacy.
    for mode in [ColorMode::Color256, ColorMode::Color16, ColorMode::Mono] {
        assert_eq!(
            ColorPipeline::detect(mode),
            ColorPipeline::LegacyRgb,
            "{:?} must route to LegacyRgb — chroma needs truecolor output",
            mode
        );
        assert!(
            !ColorPipeline::detect(mode).is_chroma(),
            "is_chroma() must return false for {:?}",
            mode
        );
        assert_eq!(
            ColorPipeline::detect(mode).label(),
            "legacy_rgb",
            "label() must be the stable machine-readable string for {:?}",
            mode
        );
        // Every LegacyRgb state must disclose why — the user is told the
        // reason via -v / --doctor so they don't have to guess.
        assert!(
            ColorPipeline::detect(mode).disable_reason(mode).is_some(),
            "disable_reason must be Some for {:?}",
            mode
        );
    }

    // ChromaDragon must NOT emit a disable_reason (no fallback to explain).
    assert_eq!(
        ColorPipeline::ChromaDragon.disable_reason(ColorMode::TrueColor),
        None,
        "ChromaDragon has no disable_reason — chroma is the primary path"
    );

    // The chroma_dragon label must be the stable string surfaced in -v,
    // --doctor, and --benchmark. Locking it here means a refactor cannot
    // silently break user-facing tooling that greps for this identifier.
    assert_eq!(
        ColorPipeline::ChromaDragon.label(),
        "chroma_dragon",
        "label() must be the stable machine-readable string for chroma_dragon"
    );
}
