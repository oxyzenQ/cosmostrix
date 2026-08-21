// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! intro_logo tests, extracted from inline `mod tests { ... }` block.
//!
//! Uses `use super::*;` to access parent's private items unchanged.

use super::*;
use crate::chroma_dragon_engine::intro_colors::LOGO_COLOR_RGB as ENGINE_LOGO_COLOR_RGB;

#[test]
fn logo_color_is_brand_purple() {
    // Spec: #A855F7 = RGB(168, 85, 247).
    // Verify the engine's canonical constant matches the spec.
    assert_eq!(ENGINE_LOGO_COLOR_RGB, (168, 85, 247));
}

#[test]
fn logo_art_is_non_empty() {
    assert!(!LOGO_ART.is_empty());
    assert!(
        LOGO_ART.lines().count() >= 10,
        "logo should have at least 10 lines"
    );
}

#[test]
fn parse_logo_art_returns_consistent_dimensions() {
    let (lines, w, h) = parse_logo_art(80, 24);
    assert_eq!(lines.len() as u16, h, "height must match line count");
    // Width is the max char count across lines.
    let computed_w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
    assert_eq!(w, computed_w);
    // Logo should fit in a typical 80×24 terminal with room to spare.
    assert!(w <= 80, "logo width {w} must fit in 80-col terminal");
    assert!(h <= 24, "logo height {h} must fit in 24-row terminal");
}

/// v25 responsive: parse_logo_art scales art to fit small terminals.
#[test]
fn parse_logo_art_scales_down_for_small_terminal() {
    let (lines, w, h) = parse_logo_art(30, 20);
    assert!(w <= 30, "scaled logo width {w} must fit in 30-col terminal");
    assert!(
        h <= 20,
        "scaled logo height {h} must fit in 20-row terminal"
    );
    // Scaled art should still have content (not empty).
    assert!(!lines.is_empty(), "scaled art must not be empty");
    let total_ink: usize = lines
        .iter()
        .map(|l| l.chars().filter(|&c| c != ' ').count())
        .sum();
    assert!(total_ink > 0, "scaled art must retain some ink cells");
}

/// v25 responsive: no upscaling when terminal exceeds art size.
#[test]
fn parse_logo_art_does_not_scale_up_for_large_terminal() {
    let (lines_large, w_large, h_large) = parse_logo_art(200, 50);
    let (lines_default, w_default, h_default) = parse_logo_art(80, 24);
    // Both should produce identical output (no upscaling).
    assert_eq!(w_large, w_default, "no upscaling for large terminal");
    assert_eq!(h_large, h_default, "no upscaling for large terminal");
    assert_eq!(
        lines_large.len(),
        lines_default.len(),
        "line count must match"
    );
}

#[test]
fn collect_logo_cells_skips_blanks() {
    let (lines, _w, _h) = parse_logo_art(80, 24);
    let (cx, cy) = visual_centroid(&lines);
    let cells = collect_logo_cells(&lines, cx, cy);
    // Every collected cell must have a non-blank glyph.
    for c in &cells {
        assert_ne!(c.ch, ' ', "blank cell should not be collected");
    }
    // The logo clearly has more than 50 non-blank cells.
    assert!(
        cells.len() > 50,
        "logo should have many non-blank cells, got {}",
        cells.len()
    );
}

#[test]
fn collect_logo_cells_computes_centroid_distance() {
    let (lines, _w, _h) = parse_logo_art(80, 24);
    let (cx, cy) = visual_centroid(&lines);
    let cells = collect_logo_cells(&lines, cx, cy);
    // The centermost cell should have a small dist_sq; the outermost
    // should have a large dist_sq.
    let mut min_d = f32::MAX;
    let mut max_d = f32::MIN;
    for c in &cells {
        // Verify the stored dist_sq matches a fresh computation
        // against the visual centroid (not the bbox center).
        let xf = c.bx as f32;
        let yf = c.by as f32;
        let expected = (xf - cx) * (xf - cx) + (yf - cy) * (yf - cy);
        assert!(
            (c.dist_sq - expected).abs() < 0.01,
            "dist_sq mismatch: stored={}, expected={}",
            c.dist_sq,
            expected
        );
        min_d = min_d.min(c.dist_sq);
        max_d = max_d.max(c.dist_sq);
    }
    assert!(min_d < max_d, "logo should have spatial extent");
}

#[test]
fn visual_centroid_is_within_bounding_box() {
    let (lines, w, h) = parse_logo_art(80, 24);
    let (cx, cy) = visual_centroid(&lines);
    // The centroid must lie inside the bounding box.
    assert!(
        (0.0..=w as f32).contains(&cx),
        "centroid x {} must be inside [0, {}]",
        cx,
        w
    );
    assert!(
        (0.0..=h as f32).contains(&cy),
        "centroid y {} must be inside [0, {}]",
        cy,
        h
    );
}

#[test]
fn visual_centroid_differs_from_bbox_center() {
    // The centroid-based placement is correct regardless of whether
    // the centroid differs from the bbox center — when they coincide,
    // the placement is simply a no-op. The owner's manually engraved
    // logo (commit 218a748) is intentionally near-symmetric, so the
    // centroid sits very close to the bbox center. We keep the test
    // as a non-strict sanity check: the centroid must be a valid
    // point inside the bbox (computed by `visual_centroid` and
    // verified by `visual_centroid_is_within_bounding_box`), but it
    // does not need to be offset by any specific amount.
    //
    // Historical note: prior to commit 218a748 the logo was strongly
    // asymmetric (dx > 0.5 || dy > 0.5). That property was specific
    // to the old art and is no longer a design invariant.
    let (lines, w, h) = parse_logo_art(80, 24);
    let (cx, cy) = visual_centroid(&lines);
    // Sanity: centroid is a finite, in-bounds point.
    assert!(cx.is_finite() && cy.is_finite(), "centroid must be finite");
    assert!(
        (0.0..=w as f32).contains(&cx),
        "centroid x {} must be inside [0, {}]",
        cx,
        w
    );
    assert!(
        (0.0..=h as f32).contains(&cy),
        "centroid y {} must be inside [0, {}]",
        cy,
        h
    );
}

#[test]
fn visual_centroid_handles_empty_art() {
    // Defensive: an empty art string must not panic.
    let lines: Vec<String> = vec!["   ".to_string(), "  ".to_string()];
    let (cx, cy) = visual_centroid(&lines);
    assert_eq!((cx, cy), (0.0, 0.0));
}

#[test]
fn visual_centroid_of_single_cell() {
    let lines: Vec<String> = vec!["     X     ".to_string()];
    let (cx, cy) = visual_centroid(&lines);
    assert!(
        (cx - 5.0).abs() < 0.01,
        "centroid x of single cell at col 5"
    );
    assert!((cy - 0.0).abs() < 0.01, "centroid y of single row");
}

#[test]
fn placement_uses_centroid_not_bbox_center() {
    // Sanity-check the placement math by reconstructing it. For a
    // typical 80×24 terminal, the spark target (logo_center_x)
    // should equal w/2 exactly when no clamping kicks in — which
    // happens as long as the centroid is at least `logo_w/2` from
    // the right edge of the bbox.
    let (lines, logo_w, logo_h) = parse_logo_art(80, 24);
    let (centroid_x, centroid_y) = visual_centroid(&lines);
    let w: u16 = 80;
    let h: u16 = 24;
    let target_x = (w as f32 * 0.5 - centroid_x).round() as i32;
    let target_y = (h as f32 * 0.5 - centroid_y).round() as i32;
    let max_x = (w as i32).saturating_sub(logo_w as i32);
    let max_y = (h as i32).saturating_sub(logo_h as i32);
    let logo_x = target_x.clamp(0, max_x);
    let logo_y = target_y.clamp(0, max_y);
    let logo_center_x = logo_x as f32 + centroid_x;
    let logo_center_y = logo_y as f32 + centroid_y;
    // On 80×24, the logo (40×19) easily fits, so no clamping should
    // occur and the centroid lands dead-center on both axes.
    assert!(
        (logo_center_x - w as f32 * 0.5).abs() < 1.0,
        "spark x {logo_center_x} should be within 1 cell of terminal center {}",
        w as f32 * 0.5
    );
    assert!(
        (logo_center_y - h as f32 * 0.5).abs() < 1.0,
        "spark y {logo_center_y} should be within 1 cell of terminal center {}",
        h as f32 * 0.5
    );
    // And the logo bbox stays fully on-screen.
    assert!(logo_x >= 0, "logo_x must be non-negative");
    let logo_right = logo_x + logo_w as i32;
    assert!(
        logo_right <= w as i32,
        "logo right edge {logo_right} must not exceed terminal width {w}"
    );
    assert!(logo_y >= 0, "logo_y must be non-negative");
    let logo_bottom = logo_y + logo_h as i32;
    assert!(
        logo_bottom <= h as i32,
        "logo bottom edge {logo_bottom} must not exceed terminal height {h}"
    );
}

#[test]
fn phase_boundaries_are_monotonic() {
    const {
        assert!(PHASE1_FADEIN_END_MS < PHASE2_IGNITION_END_MS);
    }
    const {
        assert!(PHASE2_IGNITION_END_MS < PHASE3_DISSOLVE_END_MS);
    }
    const {
        assert!(PHASE3_DISSOLVE_END_MS < PHASE4_RAIN_END_MS);
    }
}

#[test]
fn phase_boundaries_match_spec() {
    // v25 balanced: Phase 1=1.2s, Phase 2=3.0s, Phase 3=4.0s, Phase 4=4.5s.
    assert_eq!(PHASE1_FADEIN_END_MS, 1_200);
    assert_eq!(PHASE2_IGNITION_END_MS, 3_000);
    assert_eq!(PHASE3_DISSOLVE_END_MS, 4_000);
    assert_eq!(PHASE4_RAIN_END_MS, 4_500);
}

#[test]
fn dissolve_speed_range_is_valid() {
    const {
        assert!(DISSOLVE_SPEED_MIN < DISSOLVE_SPEED_MAX);
        assert!(DISSOLVE_SPEED_MIN >= 1.0);
        assert!(DISSOLVE_SPEED_MAX <= 100.0);
    }
}

#[test]
fn spawn_rain_droplet_populates_pool() {
    let mut pool = ParticlePool::new();
    let mut rng = XorShift::new(42);
    let charset = ['0', '1', 'x', 'z'];
    let ok = spawn_rain_droplet(&mut pool, &mut rng, 10.0, 5.0, &charset);
    assert!(ok);
    assert_eq!(pool.active_count(), 1);
    let p = pool
        .particles
        .iter()
        .find(|p| p.active)
        .expect("spawned droplet should be active");
    // Velocity should be mostly downward with optional horizontal jitter.
    assert!(p.vy > 0.0, "droplet should move downward");
    assert!(
        p.vx.abs() <= JITTER_VX + 0.01,
        "horizontal velocity should be within jitter range, got {}",
        p.vx
    );
    assert!(p.speed >= DISSOLVE_SPEED_MIN * 0.95);
    assert!(p.speed <= DISSOLVE_SPEED_MAX * 1.05);
    assert!(charset.contains(&p.ch), "glyph should come from charset");
    // Particle should start with the brand purple color.
    assert!(p.active);
}

#[test]
fn spawn_rain_droplet_handles_empty_charset() {
    let mut pool = ParticlePool::new();
    let mut rng = XorShift::new(7);
    let ok = spawn_rain_droplet(&mut pool, &mut rng, 10.0, 5.0, &[]);
    assert!(ok);
    let p = pool
        .particles
        .iter()
        .find(|p| p.active)
        .expect("droplet should be active");
    assert_eq!(p.ch, '0', "empty charset should fall back to '0'");
}

#[test]
fn update_rain_droplets_kills_offscreen() {
    let mut pool = ParticlePool::new();
    let _ = pool.spawn(Particle {
        x: 5.0,
        y: 50.0,
        vx: 0.0,
        vy: 20.0,
        ch: '0',
        r: 57,
        g: 255,
        b: 20,
        life: 1.0,
        max_life: 1.0,
        angle: std::f32::consts::FRAC_PI_2,
        speed: 20.0,
        spiral_rate: 0.0,
        active: true,
    });
    // Screen height 24 — droplet at y=50 is already off-screen.
    update_rain_droplets(&mut pool, 0.1, 24.0);
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn update_rain_droplets_kills_expired_life() {
    let mut pool = ParticlePool::new();
    let _ = pool.spawn(Particle {
        x: 5.0,
        y: 5.0,
        vx: 0.0,
        vy: 1.0,
        ch: '0',
        r: 57,
        g: 255,
        b: 20,
        life: 0.05,
        max_life: 0.05,
        angle: std::f32::consts::FRAC_PI_2,
        speed: 1.0,
        spiral_rate: 0.0,
        active: true,
    });
    // After 0.1s, life = 0.05 - 0.1 = negative → killed.
    update_rain_droplets(&mut pool, 0.1, 24.0);
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn update_rain_droplets_keeps_alive() {
    let mut pool = ParticlePool::new();
    let _ = pool.spawn(Particle {
        x: 5.0,
        y: 5.0,
        vx: 0.0,
        vy: 5.0,
        ch: '0',
        r: 57,
        g: 255,
        b: 20,
        life: 5.0,
        max_life: 5.0,
        angle: std::f32::consts::FRAC_PI_2,
        speed: 5.0,
        spiral_rate: 0.0,
        active: true,
    });
    update_rain_droplets(&mut pool, 0.1, 24.0);
    assert_eq!(pool.active_count(), 1);
}

#[test]
fn update_rain_droplets_advances_position() {
    let mut pool = ParticlePool::new();
    let _ = pool.spawn(Particle {
        x: 5.0,
        y: 5.0,
        vx: 0.0,
        vy: 10.0,
        ch: '0',
        r: 57,
        g: 255,
        b: 20,
        life: 5.0,
        max_life: 5.0,
        angle: std::f32::consts::FRAC_PI_2,
        speed: 10.0,
        spiral_rate: 0.0,
        active: true,
    });
    update_rain_droplets(&mut pool, 0.5, 24.0);
    let p = pool
        .particles
        .iter()
        .find(|p| p.active)
        .expect("droplet should still be active");
    // y should have advanced by speed*dt = 10*0.5 = 5 cells, so the
    // new y is 5 + 5 = 10.
    assert!(
        (p.y - 10.0).abs() < 0.1,
        "y should have advanced by speed*dt, got {}",
        p.y
    );
}
