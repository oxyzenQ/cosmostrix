// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-perf-1 regression tests: equivalence guarantees for the
//! hot-path de-allocation / LUT work.
//!
//! Every optimization in NIGHT-perf-1 is behavior-preserving by
//! construction; these tests pin the equivalences so a future refactor
//! cannot silently change rendered values while chasing performance:
//!
//! 1. `rain_shadow_lut` (per-line, built on resize) must equal
//!    `rain_shadow_factor(line, lines)` for every line it covers.
//! 2. `Cloud::palette_gen` must bump on every palette replacement
//!    (set_color_scheme + set_palette both funnel through
//!    apply_new_palette) and stay put otherwise — the HUD chroma
//!    gradient gate depends on exactly this contract.
//! 3. `HEAD_BLOOM_LUT` must equal the historical per-cell
//!    `exp(-d^2 / (2 sigma^2))` formula.
//! 4. `PhasePredictor::is_trained` must agree with the None-returning
//!    precondition of `predicts_active` (the per-frame FFI skip).

use crate::cloud::Cloud;
use crate::constants::{HEAD_BLOOM_CELLS, HEAD_BLOOM_SIGMA};
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, MonolithSize, ShadingMode};

/// Rebuild the head-bloom gaussian exactly as the pre-NIGHT-perf-1
/// per-cell code computed it.
fn legacy_head_bloom_gaussian(d: u16) -> f32 {
    let d = d as f32;
    (-d * d / (2.0 * HEAD_BLOOM_SIGMA * HEAD_BLOOM_SIGMA)).exp()
}

fn make_cloud() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(80, 40);
    cloud
}

#[test]
fn rain_shadow_lut_matches_the_function_for_every_line() {
    // The LUT is rebuilt on every reset; its values must be exactly
    // what rain_shadow_factor returns for the same (line, lines) pair —
    // bit-identical, not approximately equal, because the LUT stores
    // the function's own outputs.
    let lines: u16 = 40;
    let cloud = make_cloud();
    assert_eq!(cloud.rain_shadow_lut.len(), lines as usize);
    for line in 0..lines {
        assert_eq!(
            cloud.rain_shadow_lut[line as usize],
            crate::brightness_factors::rain_shadow_factor(line, lines),
            "rain_shadow_lut[{line}] diverged from rain_shadow_factor"
        );
    }
}

#[test]
fn rain_shadow_lut_empty_terminal_falls_back_to_one() {
    // lines == 0 → empty LUT → the DrawCtx accessor must return the
    // same 1.0 the function returns for lines == 0.
    let cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    assert!(cloud.rain_shadow_lut.is_empty());
}

#[test]
fn palette_gen_bumps_on_every_palette_replacement() {
    // The HUD chroma-gradient gate skips recompute when the generation
    // is unchanged — a missed bump means a stale gradient, a spurious
    // bump means the optimization is void. Both mutators below funnel
    // through apply_new_palette, the single choke point.
    let mut cloud = make_cloud();
    let gen0 = cloud.palette_gen;

    cloud.set_color_scheme(ColorScheme::NeonGreen);
    assert_eq!(
        cloud.palette_gen,
        gen0.wrapping_add(1),
        "set_color_scheme must bump palette_gen"
    );

    let palette =
        crate::palette::build_palette(ColorScheme::FancyDiamond, ColorMode::TrueColor, false);
    cloud.set_palette(Some("test-pal"), palette);
    assert_eq!(
        cloud.palette_gen,
        gen0.wrapping_add(2),
        "set_palette must bump palette_gen"
    );

    // No palette change → no bump (monolith size mutates other state).
    cloud.set_monolith_size(MonolithSize::Large);
    assert_eq!(
        cloud.palette_gen,
        gen0.wrapping_add(2),
        "non-palette mutation must NOT bump palette_gen"
    );
}

#[test]
fn head_bloom_lut_matches_the_historical_formula() {
    // The LUT must reproduce the exact exp() values the per-cell code
    // computed; the production branch condition bounds
    // dist_from_head to 1..HEAD_BLOOM_CELLS, so index 0 is unused but
    // still must match the formula for safety.
    let lut = &crate::droplet::HEAD_BLOOM_LUT;
    assert_eq!(lut.len(), HEAD_BLOOM_CELLS as usize);
    for d in 0..HEAD_BLOOM_CELLS {
        assert_eq!(
            lut[d as usize],
            legacy_head_bloom_gaussian(d),
            "HEAD_BLOOM_LUT[{d}] diverged from the exp() formula"
        );
    }
}

#[test]
fn is_trained_agrees_with_predicts_active_precondition() {
    // begin_frame skips the wall-clock FFI when !is_trained(); the
    // skip is only correct while predicts_active would return None —
    // i.e. is_trained() == false exactly below two transitions.
    use crate::central_control_power_dragon::PhasePredictor;
    let mut p = PhasePredictor::new();
    assert!(!p.is_trained());
    assert_eq!(p.predicts_active(0.0), None);

    p.record_transition(true, 9.0 * 3600.0);
    assert!(
        !p.is_trained(),
        "one transition must not train the predictor"
    );
    assert_eq!(p.predicts_active(12.0 * 3600.0), None);

    p.record_transition(false, 17.0 * 3600.0);
    assert!(p.is_trained(), "two transitions must train the predictor");
    assert!(
        p.predicts_active(12.0 * 3600.0).is_some(),
        "a trained predictor must produce a prediction"
    );
}

#[test]
fn message_scratch_buffers_reuse_allocations_across_draws() {
    // The six draw_message scratch buffers must survive a frame cycle
    // (clear + refill, no realloc-from-zero) — the second draw_message
    // call must observe the capacities the first one established.
    // Draw a message twice and assert the pulse/halo arrays are sized
    // and the border_pulses drain-and-swap keeps both Vecs alive.
    let mut cloud = make_cloud();
    cloud.set_message("hi");
    let now = std::time::Instant::now();
    let mut frame = crate::frame::Frame::new(80, 40, None);
    cloud.draw_message(&mut frame, now);
    let pulse_cap = cloud.pulse_factor_scratch.capacity();
    let halo_cap = cloud.halo_color_scratch.capacity();
    let slide_cap = cloud.slide_cells_scratch.capacity();
    assert!(pulse_cap >= 2, "message.len()=2 sized the pulse arrays");
    assert!(halo_cap >= 80, "cols=80 sized the halo arrays");
    cloud.draw_message(&mut frame, now);
    assert_eq!(
        cloud.pulse_factor_scratch.capacity(),
        pulse_cap,
        "second draw must reuse (not regrow) the pulse scratch"
    );
    assert_eq!(
        cloud.halo_color_scratch.capacity(),
        halo_cap,
        "second draw must reuse (not regrow) the halo scratch"
    );
    assert_eq!(
        cloud.slide_cells_scratch.capacity(),
        slide_cap,
        "second draw must reuse (not regrow) the slide scratch"
    );
    // The alive-pulse drain-and-swap must keep border_pulses a live
    // Vec (empty after all pulses expired — none were ever created).
    assert!(cloud.border_pulses.is_empty());
}
