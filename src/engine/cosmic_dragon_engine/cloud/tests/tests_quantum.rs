// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Quantum Ripple dynamic-color tests.
//!
//! Verifies that each spawned QuantumParticle snapshots the active
//! palette's **body** color (middle index) at birth, and that this
//! snapshot is preserved across palette switches — producing a natural
//! crossfade where the old cohort fades out in its original body color
//! while new clicks spawn particles in the new body color.
//!
//! We snapshot the body stop (`colors[len/2]`) rather than the head
//! stop (`colors.last()`) because the head is intentionally near-white
//! across most schemes (Green head = `(201, 244, 210)`, Blue head =
//! `(190, 223, 242)`) to give droplets their bright leading edge.
//! Using the head made every click look white; the body stop is the
//! saturated hue the eye reads as "the rain color".
//!
//! These tests use `ColorMode::TrueColor` because the default `make_cloud()`
//! helper uses `ColorMode::Mono`, which degrades every palette to a single
//! white stop — making cross-scheme color differences invisible. Quantum
//! ripple color capture is only meaningful under TrueColor.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use super::super::Cloud;
// v50 (2026-08-17) enhanced color cycling: import the interpolation
// helper + a brighten_color test helper so tests can compute the
// expected brightened cycled color at a given life_frac.
use crate::constants::{
    COLOR_TRANSITION_DURATION_MS, QUANTUM_BODY_TONE_DOWN, QUANTUM_BRAND_PURPLE_B,
    QUANTUM_BRAND_PURPLE_G, QUANTUM_BRAND_PURPLE_R, QUANTUM_RIPPLE_LIFETIME_SECS,
    QUANTUM_RIPPLE_PARTICLE_COUNT,
};
use crate::frame::Frame;
use crate::palette::decode_color;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

/// Build a TrueColor cloud so palette body colors are distinct per scheme.
pub(super) fn make_truecolor_cloud(scheme: ColorScheme) -> Cloud {
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

/// Decode the body color (middle stop) of a cloud's active palette.
/// Returns the brand-purple fallback if the body is not decodable.
fn palette_body_rgb(cloud: &Cloud) -> (u8, u8, u8) {
    let idx = cloud.palette.colors.len() / 2;
    cloud
        .palette
        .colors
        .get(idx)
        .and_then(|c| decode_color(*c))
        .unwrap_or((
            QUANTUM_BRAND_PURPLE_R,
            QUANTUM_BRAND_PURPLE_G,
            QUANTUM_BRAND_PURPLE_B,
        ))
}

/// Count currently-active quantum particles in the pool.
pub(super) fn count_active_particles(cloud: &Cloud) -> usize {
    cloud.quantum_particles.iter().filter(|p| p.active).count()
}

/// Force-complete any pending color transition by rolling the transition
/// start time far enough into the past and running one frame.
pub(super) fn force_complete_transition(cloud: &mut Cloud) {
    let now = Instant::now();
    cloud.transition_start =
        Some(now - Duration::from_millis(COLOR_TRANSITION_DURATION_MS as u64 + 1));
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.rain_at(&mut frame, now);
    assert!(
        cloud.transition_start.is_none(),
        "color transition must complete before assertions"
    );
}

#[test]
fn quantum_pool_init_seeds_brand_purple_default() {
    // Inactive pool entries must have a valid default color so the
    // struct is never in an indeterminate state. The default matches
    // the brand-purple fallback used when a palette has no decodable
    // body stop.
    let cloud = make_truecolor_cloud(ColorScheme::Green);
    for p in &cloud.quantum_particles {
        assert!(!p.active, "pool entries must start inactive");
        assert_eq!(
            p.r, QUANTUM_BRAND_PURPLE_R,
            "default r must be brand purple"
        );
        assert_eq!(
            p.g, QUANTUM_BRAND_PURPLE_G,
            "default g must be brand purple"
        );
        assert_eq!(
            p.b, QUANTUM_BRAND_PURPLE_B,
            "default b must be brand purple"
        );
    }
}

#[test]
fn quantum_particle_snapshots_palette_body_color_at_spawn() {
    // Clicking must spawn particles whose r/g/b match the active
    // palette's body color (middle index). This is the core invariant:
    // the snapshot is taken once at spawn time and stored per-particle.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let expected = palette_body_rgb(&cloud);
    assert_ne!(
        expected,
        (
            QUANTUM_BRAND_PURPLE_R,
            QUANTUM_BRAND_PURPLE_G,
            QUANTUM_BRAND_PURPLE_B
        ),
        "test setup: Green TrueColor body should not equal the brand-purple fallback"
    );

    cloud.set_mouse_click(5, 5);

    let active_count = count_active_particles(&cloud);
    assert_eq!(
        active_count, QUANTUM_RIPPLE_PARTICLE_COUNT,
        "click must spawn exactly QUANTUM_RIPPLE_PARTICLE_COUNT particles"
    );

    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        assert_eq!(
            (p.r, p.g, p.b),
            expected,
            "active particle must snapshot palette body color at spawn"
        );
    }
}

#[test]
fn quantum_body_color_differs_from_head_color() {
    // Regression guard: the body color used for ripple snapshots must
    // NOT equal the head color. If it does, the snapshot logic has
    // regressed back to `colors.last()`. The head stop is intentionally
    // near-white across most schemes; the body stop is the saturated
    // rain hue. They must differ for every scheme we ship.
    for &scheme in &[
        ColorScheme::Green,
        ColorScheme::Red,
        ColorScheme::Blue,
        ColorScheme::Cyan,
        ColorScheme::Orange,
    ] {
        let cloud = make_truecolor_cloud(scheme);
        let body = palette_body_rgb(&cloud);
        let head = cloud
            .palette
            .colors
            .last()
            .and_then(|c| decode_color(*c))
            .unwrap_or((0, 0, 0));
        assert_ne!(
            body, head,
            "{scheme:?}: body color {body:?} must differ from head color {head:?} \
             — if they match, ripple particles will look white again"
        );
    }
}

#[test]
fn quantum_particle_retains_snapshot_after_palette_switch() {
    // The crossfade invariant: when the user switches palette mid-flight,
    // existing particles must KEEP their original snapshot color. Only
    // newly-spawned particles (from clicks after the switch) pick up the
    // new color. This is what produces the natural crossfade.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let old_body = palette_body_rgb(&cloud);

    cloud.set_mouse_click(5, 5);

    cloud.set_color_scheme(ColorScheme::Red);
    force_complete_transition(&mut cloud);

    let new_body = palette_body_rgb(&cloud);
    assert_ne!(
        new_body, old_body,
        "test setup requires Green and Red TrueColor palettes to have different body colors"
    );

    // The particles spawned BEFORE the switch must still carry the old color.
    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        assert_eq!(
            (p.r, p.g, p.b),
            old_body,
            "particles spawned before palette switch must retain their original snapshot"
        );
    }
}

#[test]
fn quantum_particle_after_switch_new_clicks_use_new_color() {
    // After a palette switch completes, subsequent clicks must spawn
    // particles with the NEW palette body color. This complements the
    // retain-snapshot test: old particles keep old color, new particles
    // get new color.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let old_body = palette_body_rgb(&cloud);

    cloud.set_color_scheme(ColorScheme::Red);
    force_complete_transition(&mut cloud);

    let new_body = palette_body_rgb(&cloud);
    assert_ne!(new_body, old_body, "palettes must differ for this test");

    cloud.set_mouse_click(5, 5);

    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        assert_eq!(
            (p.r, p.g, p.b),
            new_body,
            "particles spawned after palette switch must use the new body color"
        );
    }
}

#[test]
fn quantum_crossfade_two_cohorts_coexist_with_distinct_colors() {
    // The full crossfade scenario: click under palette A, switch to
    // palette B mid-flight, then click again. Both cohorts must coexist
    // with their respective snapshot colors until the old one expires.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let body_a = palette_body_rgb(&cloud);

    // First click — cohort A born.
    cloud.set_mouse_click(2, 2);
    let cohort_a_count = count_active_particles(&cloud);
    assert_eq!(cohort_a_count, QUANTUM_RIPPLE_PARTICLE_COUNT);

    // Switch palette mid-flight (well within the particle lifespan).
    cloud.set_color_scheme(ColorScheme::Blue);
    force_complete_transition(&mut cloud);

    let body_b = palette_body_rgb(&cloud);
    assert_ne!(body_b, body_a, "palettes must differ for crossfade test");

    // Second click — cohort B born. The pool must have enough free slots
    // to accommodate both cohorts (pool size = 64, each click = 20).
    cloud.set_mouse_click(15, 8);

    let mut cohort_a_particles = 0usize;
    let mut cohort_b_particles = 0usize;
    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        let rgb = (p.r, p.g, p.b);
        if rgb == body_a {
            cohort_a_particles += 1;
        } else if rgb == body_b {
            cohort_b_particles += 1;
        } else {
            panic!(
                "active particle has unexpected color {rgb:?}, expected either {body_a:?} or {body_b:?}"
            );
        }
    }

    assert_eq!(
        cohort_a_particles, QUANTUM_RIPPLE_PARTICLE_COUNT,
        "cohort A particles must still be active with the old color"
    );
    assert_eq!(
        cohort_b_particles, QUANTUM_RIPPLE_PARTICLE_COUNT,
        "cohort B particles must be active with the new color"
    );
}

#[test]
fn quantum_apply_does_not_mutate_snapshot_after_palette_switch() {
    // End-to-end render check: render a frame after a palette switch
    // and verify that a still-active particle's stored snapshot is
    // unchanged by the render pass — i.e. apply_quantum_ripple is a
    // pure reader of p.r/g/b and never writes back to those fields.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let old_body = palette_body_rgb(&cloud);

    cloud.set_mouse_click(3, 3);
    let snapshot_before: Vec<(u8, u8, u8)> = cloud
        .quantum_particles
        .iter()
        .filter(|p| p.active)
        .map(|p| (p.r, p.g, p.b))
        .collect();
    assert!(!snapshot_before.is_empty());

    cloud.set_color_scheme(ColorScheme::Red);
    force_complete_transition(&mut cloud);

    let new_body = palette_body_rgb(&cloud);
    assert_ne!(new_body, old_body);

    // Render one more frame — apply_quantum_ripple must run and must
    // NOT mutate any particle's snapshot color.
    let render_now = Instant::now();
    cloud.last_phosphor_time = render_now;
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.rain_at(&mut frame, render_now);

    let snapshot_after: Vec<(u8, u8, u8)> = cloud
        .quantum_particles
        .iter()
        .filter(|p| p.active)
        .map(|p| (p.r, p.g, p.b))
        .collect();

    assert_eq!(
        snapshot_before, snapshot_after,
        "render pass must not mutate particle snapshot colors"
    );
    // And the snapshot must still be the OLD body, not the new one.
    for rgb in &snapshot_after {
        assert_eq!(
            *rgb, old_body,
            "particles must retain old snapshot after render under new palette"
        );
    }
}

#[test]
fn quantum_particle_expires_within_documented_lifespan() {
    // Sanity check the lifespan constant is wired correctly: a particle
    // spawned now must still be active immediately, and must be deactivated
    // once `now` advances past QUANTUM_RIPPLE_LIFETIME_SECS.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let spawn_time = Instant::now();

    cloud.set_mouse_click(5, 5);
    assert_eq!(
        count_active_particles(&cloud),
        QUANTUM_RIPPLE_PARTICLE_COUNT,
        "all spawned particles must be active immediately after click"
    );

    // Advance just past the lifespan and run apply_quantum_ripple
    // (called internally by rain_at when particles are active).
    let expire_time =
        spawn_time + Duration::from_millis(((QUANTUM_RIPPLE_LIFETIME_SECS * 1000.0) as u64) + 50);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.last_phosphor_time = spawn_time;
    cloud.rain_at(&mut frame, expire_time);

    assert_eq!(
        count_active_particles(&cloud),
        0,
        "all particles must be deactivated once age exceeds QUANTUM_RIPPLE_LIFETIME_SECS"
    );
    assert_eq!(
        cloud.quantum_active_count, 0,
        "active_count counter must be decremented to zero alongside the pool"
    );
}

#[test]
fn quantum_active_count_counter_tracks_pool_state() {
    // The O(1) early-out in apply_quantum_ripple relies on
    // quantum_active_count being an accurate reflection of the number
    // of active particles in the pool. Verify the counter is updated
    // correctly across multiple clicks.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);

    assert_eq!(cloud.quantum_active_count, 0, "starts at zero");

    cloud.set_mouse_click(1, 1);
    assert_eq!(
        cloud.quantum_active_count, QUANTUM_RIPPLE_PARTICLE_COUNT,
        "counter must match active count after first click"
    );

    // Second click while the first cohort is still alive — the pool
    // must have room (64 slots, 20 per click) and the counter must
    // reflect the sum.
    cloud.set_mouse_click(2, 2);
    assert_eq!(
        cloud.quantum_active_count,
        QUANTUM_RIPPLE_PARTICLE_COUNT * 2,
        "counter must accumulate across overlapping clicks"
    );

    // Force-expire all particles by advancing well past the lifespan.
    let now = Instant::now();
    let expire = now + Duration::from_secs(10);
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.last_phosphor_time = now;
    cloud.rain_at(&mut frame, expire);

    assert_eq!(
        cloud.quantum_active_count, 0,
        "counter must return to zero once all particles expire"
    );
}

#[test]
fn quantum_particle_snapshot_matches_an_actual_palette_stop() {
    // Guard against a regression where the snapshot accidentally
    // stores Color::Reset (which decode_color returns None for) or
    // some synthetic color not present in the palette. The body stop
    // of a real palette is always a concrete RGB color, so the snapshot
    // must always match one of the palette's actual stops exactly.
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let palette_stops: Vec<(u8, u8, u8)> = cloud
        .palette
        .colors
        .iter()
        .filter_map(|c| decode_color(*c))
        .collect();
    assert!(
        !palette_stops.is_empty(),
        "test palette must have at least one decodable stop"
    );

    cloud.set_mouse_click(7, 3);

    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        let rgb = (p.r, p.g, p.b);
        assert!(
            palette_stops.contains(&rgb),
            "particle snapshot {rgb:?} must match one of the palette's actual stops {palette_stops:?}"
        );
        // The snapshot must equal the BODY stop (middle index), not the
        // head (last) or any other stop.
        let body_idx = palette_stops.len() / 2;
        assert_eq!(
            rgb,
            palette_stops[body_idx],
            "particle snapshot must equal the body (mid-index) stop, not the head or any other stop"
        );
        // Reset color must never appear as a snapshot.
        assert_ne!(
            Color::from(rgb),
            Color::Reset,
            "snapshot must not be Color::Reset"
        );
    }
}

#[test]
fn quantum_particle_snapshot_is_independent_of_other_schemes() {
    // Verify the snapshot is taken from the cloud's CURRENT palette at
    // spawn time, not from some global default. Build a cloud, snapshot
    // its body, then build a different-scheme cloud and verify the two
    // bodies differ — proving the snapshot would also differ.
    let green = make_truecolor_cloud(ColorScheme::Green);
    let red = make_truecolor_cloud(ColorScheme::Red);
    let blue = make_truecolor_cloud(ColorScheme::Blue);

    let g_body = palette_body_rgb(&green);
    let r_body = palette_body_rgb(&red);
    let b_body = palette_body_rgb(&blue);

    assert_ne!(g_body, r_body, "Green vs Red body must differ");
    assert_ne!(g_body, b_body, "Green vs Blue body must differ");
    assert_ne!(r_body, b_body, "Red vs Blue body must differ");

    // Now spawn particles in each and verify they carry scheme-correct snapshots.
    let mut green = green;
    let mut red = red;
    let mut blue = blue;
    green.set_mouse_click(1, 1);
    red.set_mouse_click(1, 1);
    blue.set_mouse_click(1, 1);

    for p in &green.quantum_particles {
        if p.active {
            assert_eq!(
                (p.r, p.g, p.b),
                g_body,
                "green-cloud particles must snapshot green body"
            );
        }
    }
    for p in &red.quantum_particles {
        if p.active {
            assert_eq!(
                (p.r, p.g, p.b),
                r_body,
                "red-cloud particles must snapshot red body"
            );
        }
    }
    for p in &blue.quantum_particles {
        if p.active {
            assert_eq!(
                (p.r, p.g, p.b),
                b_body,
                "blue-cloud particles must snapshot blue body"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// v30 masterclass: render-time tone-down contract
// ═══════════════════════════════════════════════════════════════════════════

/// v30 invariant: the snapshot stored on a spawned particle is the
/// palette body stop EXACTLY — the tone-down factor is applied only at
/// render time. This means the snapshot stays a clean copy of the body
/// stop for crossfade + regression-test purposes, while the rendered
/// pixel is dimmed by `QUANTUM_BODY_TONE_DOWN`.
///
/// If a future commit moves the tone-down into the spawn path (so
/// `p.r/g/b` already stores the dimmed value), this test fails —
/// forcing the author to either revert or explicitly relax the
/// "snapshot == body stop" contract by also updating
/// `quantum_particle_snapshot_matches_an_actual_palette_stop`.
#[test]
fn quantum_snapshot_unchanged_by_tone_down_factor() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    let body = palette_body_rgb(&cloud);
    assert_ne!(
        body,
        (
            QUANTUM_BRAND_PURPLE_R,
            QUANTUM_BRAND_PURPLE_G,
            QUANTUM_BRAND_PURPLE_B
        ),
        "test setup: Green body must differ from brand-purple fallback"
    );

    cloud.set_mouse_click(5, 5);

    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        assert_eq!(
            (p.r, p.g, p.b),
            body,
            "snapshot stored on the particle must equal the palette body stop EXACTLY — \
             the v30 tone-down is applied only at render time"
        );
    }
}

/// v30 invariant: the rendered pixel IS dimmed by `QUANTUM_BODY_TONE_DOWN`.
///
/// Spawns a particle on a blank cell (no fg, no rain), runs one render
/// pass, and verifies the resulting cell color equals the snapshot
/// scaled by the tone-down factor (within ±1 per channel for rounding).
/// This catches the inverse regression: a future commit that removes
/// the tone-down from `apply_quantum_ripple` but keeps the snapshot
/// logic intact — the snapshot tests would still pass but the visual
/// "too bright" complaint would return.
/// HSV value scaling to TARGET_V=200 (matches apply_quantum_ripple's brighten).
pub(super) fn brighten_color(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    const TARGET_V: u32 = 200;
    let max = r.max(g).max(b) as u32;
    if max >= TARGET_V {
        (r, g, b)
    } else if max == 0 {
        (120, 120, 120)
    } else {
        let scale = (TARGET_V * 100).saturating_div(max);
        (
            ((r as u32 * scale) / 100) as u8,
            ((g as u32 * scale) / 100) as u8,
            ((b as u32 * scale) / 100) as u8,
        )
    }
}

#[test]
fn quantum_rendered_pixel_is_dimmed_by_tone_down_factor() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);

    // v50 (2026-08-17) enhanced color cycling: expected = brighten(palette[0]) * tone_down.
    let palette_first = cloud
        .palette
        .colors
        .first()
        .and_then(|c| decode_color(*c))
        .unwrap_or_else(|| palette_body_rgb(&cloud));
    let brightened = brighten_color(palette_first.0, palette_first.1, palette_first.2);
    let expected = (
        (brightened.0 as f32 * QUANTUM_BODY_TONE_DOWN).round() as u8,
        (brightened.1 as f32 * QUANTUM_BODY_TONE_DOWN).round() as u8,
        (brightened.2 as f32 * QUANTUM_BODY_TONE_DOWN).round() as u8,
    );

    // Spawn a click at (5, 5).
    let spawn_time = Instant::now();
    cloud.set_mouse_click(5, 5);

    // Render one frame at the spawn instant (age ≈ 0, brightness ≈ 1).
    // On a blank cell with no fg and no bg, the blend collapses to
    // `nr = pr` (the dimmed snapshot), so the rendered cell's fg should
    // equal `expected` within rounding tolerance.
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.last_phosphor_time = spawn_time;
    cloud.rain_at(&mut frame, spawn_time);

    // Find the spawned particle's screen cell and verify the rendered color.
    let mut found = false;
    for p in &cloud.quantum_particles {
        if !p.active {
            continue;
        }
        let col = p.x as u16;
        let line = p.y as u16;
        if let Some(idx) = frame.index(col, line) {
            let cell = frame.cell_at_index(idx);
            if let Some(fg) = cell.fg {
                let rendered = decode_color(fg).unwrap_or((0, 0, 0));
                let dr = (rendered.0 as i16 - expected.0 as i16).unsigned_abs() as u8;
                let dg = (rendered.1 as i16 - expected.1 as i16).unsigned_abs() as u8;
                let db = (rendered.2 as i16 - expected.2 as i16).unsigned_abs() as u8;
                assert!(
                    dr <= 1 && dg <= 1 && db <= 1,
                    "rendered color {rendered:?} must equal brighten(palette[0])*tone_down \
                     {expected:?} within ±1 per channel (palette[0]={palette_first:?}, brightened={brightened:?}, tone_down={QUANTUM_BODY_TONE_DOWN})"
                );
                found = true;
            }
        }
    }
    assert!(
        found,
        "at least one rendered particle must be visible on a blank cell"
    );
}

/// v30 invariant: the tone-down factor itself stays in a perceptible range.
///
/// `0.5` would dim Green body `(0, 220, 0)` to `(0, 110, 0)` — below
/// the Phase 7 trail floor of 131 on bright themes, making ripples
/// disappear against the bg. `1.0` is no tone-down at all — restores
/// the "too bright" complaint that motivated the constant. The sweet
/// spot is `[0.6, 0.85]`.
#[test]
fn quantum_body_tone_down_factor_in_sweet_spot() {
    assert!(
        (0.6..=0.85).contains(&QUANTUM_BODY_TONE_DOWN),
        "QUANTUM_BODY_TONE_DOWN = {QUANTUM_BODY_TONE_DOWN} is outside the sweet spot [0.6, 0.85] \
         — if this was intentional, update the rationale in the constant's doc comment"
    );
}
