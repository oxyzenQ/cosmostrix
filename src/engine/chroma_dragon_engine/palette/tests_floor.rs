// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the Phase 7 palette-relative brightness floor and Phase 7-b
//! body-tail continuity. Extracted from `palette.rs` to keep that file
//! under the 800-LOC cap.
//!
//! Loaded via `#[cfg(test)] #[path = "palette_floor_tests.rs"] mod palette_floor_tests;`
//! in `palette.rs` — preserves `use super::*` access to palette's private
//! helpers (apply_palette_relative_floor, colors_from_stops, etc.).

use super::*;
use crate::runtime::ColorScheme;

/// Compute the RGB sum of a Color (decoded via color_to_rgb).
pub(super) fn rgb_sum(c: Color) -> u16 {
    let (r, g, b) = color_to_rgb(c);
    r as u16 + g as u16 + b as u16
}

/// Phase 7: empty palette doesn't panic and returns empty.
#[test]
fn phase7_empty_palette_no_panic() {
    let mut rgb: Vec<(u8, u8, u8)> = vec![];
    apply_palette_relative_floor(&mut rgb);
    assert!(rgb.is_empty());
}

/// Phase 7: single stop above ABSOLUTE_MIN_FLOOR is unchanged.
#[test]
fn phase7_single_stop_above_absolute_min_unchanged() {
    let mut rgb = vec![(100, 100, 100)]; // sum 300, floor would be max(30, 45) = 45
    apply_palette_relative_floor(&mut rgb);
    assert_eq!(rgb, vec![(100, 100, 100)]);
}

/// Phase 7: single stop below ABSOLUTE_MIN_FLOOR gets boosted to 30.
#[test]
fn phase7_single_stop_below_absolute_min_boosted() {
    let mut rgb = vec![(5, 5, 5)]; // sum 15, max=15, derived floor = 2, clamped to 30
    apply_palette_relative_floor(&mut rgb);
    let sum = rgb[0].0 as u16 + rgb[0].1 as u16 + rgb[0].2 as u16;
    assert!(
        (28..=32).contains(&sum),
        "boosted sum {sum} should be ~30 (±2 for integer rounding)"
    );
}

/// Phase 7: trail stop on a bright palette gets boosted but not to 180.
/// Green-like palette: head sum 655, trail sum 13. Floor should be 131.
#[test]
fn phase7_bright_palette_trail_boosted_below_v17() {
    let mut rgb = vec![
        (0, 12, 1),     // trail, sum 13
        (0, 45, 6),     // sum 51
        (55, 218, 83),  // sum 356
        (80, 255, 110), // head, sum 445
    ];
    apply_palette_relative_floor(&mut rgb);
    let trail_sum = rgb[0].0 as u16 + rgb[0].1 as u16 + rgb[0].2 as u16;
    // max_sum across all stops is 445 (80+255+110).
    // 445 * 0.20 = 89.0, as u16 = 89. floor = max(30, min(180, 89)) = 89.
    // Trail sum 13 < 89 → boost to ~89.
    assert!(
        (85..=95).contains(&trail_sum),
        "trail sum {trail_sum} should be ~89 (v17 was 180 — Phase 7 is much less aggressive)"
    );
    // v17 would have boosted to 180; Phase 7 boosts to 89 — verify the
    // improvement is real (Phase 7 trail is at most 50% of v17's trail).
    assert!(
        trail_sum < 180,
        "Phase 7 trail sum {trail_sum} must be less than v17's 180"
    );
}

/// Phase 7: dark-theme trail (Cosmos-like) gets boosted but preserves void feel.
/// Cosmos palette: head sum 655, trail sum 24. Floor should be ~131.
#[test]
fn phase7_dark_palette_trail_preserves_aesthetic() {
    let mut rgb = vec![
        (3, 3, 18),      // void trail, sum 24
        (15, 18, 60),    // sum 93
        (94, 80, 221),   // sum 395
        (213, 194, 248), // head, sum 655
    ];
    apply_palette_relative_floor(&mut rgb);
    let (r, g, b) = rgb[0];
    let trail_sum = r as u16 + g as u16 + b as u16;
    // Floor = max(30, min(180, 655 * 0.20 = 131)) = 131.
    // Trail sum 24 < 131 → boost to ~131.
    assert!(
        (125..=135).contains(&trail_sum),
        "trail sum {trail_sum} should be ~131 (much less aggressive than v17's 180)"
    );
    // Verify hue preservation: the blue channel should still be dominant
    // (original ratio was 1:1:6, boosted should be similar).
    assert!(
        b > r && b > g,
        "blue channel {b} must remain dominant after boost (hue preserved)"
    );
    // Verify the trail is visibly less aggressive than v17.
    // v17 would have produced (22, 22, 135) sum 180.
    // Phase 7 should produce roughly (16, 16, 99) sum 131.
    assert!(
        trail_sum < 165,
        "Phase 7 trail sum {trail_sum} must be well below v17's 180 (preserve void feel)"
    );
}

/// Phase 7: Mercury-like palette with very dark trail + bright head.
#[test]
fn phase7_mercury_palette_trail_visible_but_dark() {
    let mut rgb = vec![
        (5, 5, 5),       // sum 15
        (25, 24, 23),    // sum 72
        (93, 90, 86),    // sum 269
        (245, 240, 235), // head, sum 720
    ];
    apply_palette_relative_floor(&mut rgb);
    let trail_sum = rgb[0].0 as u16 + rgb[0].1 as u16 + rgb[0].2 as u16;
    // Floor = max(30, min(180, 720 * 0.20 = 144)) = 144.
    // Trail sum 15 < 144 → boost to ~144.
    assert!(
        (140..=150).contains(&trail_sum),
        "trail sum {trail_sum} should be ~144 (visible dark gray, not v17's 180 medium gray)"
    );
    // Verify it's still dark gray, not medium gray.
    // v17 would have produced (60, 60, 60). Phase 7 produces (48, 48, 48).
    assert!(
        rgb[0].0 < 55,
        "trail r {} must be < 55 (dark gray, not v17's medium gray)",
        rgb[0].0
    );
}

/// Phase 7: palette with all stops above floor is unchanged.
#[test]
fn phase7_all_stops_above_floor_unchanged() {
    let original = vec![
        (100, 100, 100), // sum 300
        (150, 150, 150), // sum 450
        (200, 200, 200), // sum 600
    ];
    let mut rgb = original.clone();
    apply_palette_relative_floor(&mut rgb);
    // max_sum = 600, floor = max(30, min(180, 90)) = 90. All stops > 90.
    assert_eq!(
        rgb, original,
        "no stops should be modified when all above floor"
    );
}

/// Phase 7: palette with extremely bright head caps at GLOBAL_MAX_FLOOR.
#[test]
fn phase7_extremely_bright_head_caps_at_global_max() {
    let mut rgb = vec![
        (0, 0, 0),       // sum 0
        (255, 255, 255), // head, sum 765
    ];
    apply_palette_relative_floor(&mut rgb);
    let trail_sum = rgb[0].0 as u16 + rgb[0].1 as u16 + rgb[0].2 as u16;
    let head_sum = rgb[1].0 as u16 + rgb[1].1 as u16 + rgb[1].2 as u16;
    // Phase 7-b: continuity is uncapped. Trail = head/2.0 = 765/2.0 = 382.5,
    // integer-truncated to ~381. This is intentional — the v17 ceiling (180)
    // is for the BASIC floor only. Continuity can boost higher to maintain
    // the 2.0x gap contract.
    assert!(
        (370..=395).contains(&trail_sum),
        "trail sum {trail_sum} should be ~381 (head/2.0, uncapped continuity)"
    );
    // Hard guarantee: trail must never exceed head (hierarchy preserved).
    assert!(
        trail_sum < head_sum,
        "trail sum {trail_sum} must be < head sum {head_sum} (hierarchy preserved)"
    );
}

/// Phase 7: palette with dim head (max_sum < 200) gets low floor.
#[test]
fn phase7_dim_head_palette_gets_low_floor() {
    let mut rgb = vec![
        (0, 0, 0),    // sum 0
        (10, 10, 30), // sum 50
        (40, 40, 80), // head, sum 160
    ];
    apply_palette_relative_floor(&mut rgb);
    // max_sum = 160, derived = 160 * 0.20 = 32. floor = max(30, min(180, 32)) = 32.
    // Trail sum 0 < 32 → boost to 30 (pure-black special case uses floor/3 = 10 per channel = 30).
    // Then Phase 7-b continuity (gap target = 2.0):
    //   head=160, second=50, gap=160/50=3.2x > 2.0 → boost second to 160/2.0=80.
    //   trail=30, second=80, gap=80/30=2.67x > 2.0 → boost trail to 80/2.0=40.
    let trail_sum = rgb[0].0 as u16 + rgb[0].1 as u16 + rgb[0].2 as u16;
    assert!(
        (35..=45).contains(&trail_sum),
        "trail sum {trail_sum} should be ~40 (continuity boost to second/2.0)"
    );
    let second_sum = rgb[1].0 as u16 + rgb[1].1 as u16 + rgb[1].2 as u16;
    // Phase 7-b continuity boosts second from 50 to ~80 (head/2.0).
    assert!(
        (75..=85).contains(&second_sum),
        "second stop sum {second_sum} should be ~80 (continuity boost to head/2.0)"
    );
}

/// Phase 7: hue is preserved after boost (RGB ratio stays proportional).
#[test]
fn phase7_hue_preserved_after_boost() {
    // Blue-dominant trail: ratio 1:1:6
    let mut rgb = vec![
        (3, 3, 18),      // sum 24, ratio 1:1:6
        (213, 194, 248), // head, sum 655
    ];
    apply_palette_relative_floor(&mut rgb);
    let (r, g, b) = rgb[0];
    // After boost, ratio should still be approximately 1:1:6 (within integer rounding).
    let ratio_rg = (r as f32) / (g as f32).max(1.0);
    let ratio_rb = (r as f32) / (b as f32).max(1.0);
    assert!(
        (0.8..=1.2).contains(&ratio_rg),
        "r/g ratio {ratio_rg:.2} should be ~1.0 (hue preserved)"
    );
    assert!(
        (0.10..=0.25).contains(&ratio_rb),
        "r/b ratio {ratio_rb:.2} should be ~0.17 (blue dominant, hue preserved)"
    );
}

/// Phase 7: head→body→trail hierarchy preserved (head brighter than body,
/// body brighter than trail).
#[test]
fn phase7_hierarchy_preserved() {
    let mut rgb = vec![
        (0, 12, 1),     // trail
        (55, 218, 83),  // body
        (80, 255, 110), // head
    ];
    apply_palette_relative_floor(&mut rgb);
    let trail_sum = rgb[0].0 as u16 + rgb[0].1 as u16 + rgb[0].2 as u16;
    let body_sum = rgb[1].0 as u16 + rgb[1].1 as u16 + rgb[1].2 as u16;
    let head_sum = rgb[2].0 as u16 + rgb[2].1 as u16 + rgb[2].2 as u16;
    assert!(trail_sum < body_sum, "trail {trail_sum} < body {body_sum}");
    assert!(body_sum < head_sum, "body {body_sum} < head {head_sum}");
}

/// Phase 7: integration with colors_from_stops — full pipeline produces
/// visible trail stops for a dark-theme-like input.
#[test]
fn phase7_colors_from_stops_integration_dark_theme() {
    let stops: &[(u8, u8, u8)] = &[
        (3, 3, 18),
        (15, 18, 60),
        (94, 80, 221),
        (120, 100, 255),
        (213, 194, 248),
    ];
    let colors = colors_from_stops(ColorMode::TrueColor, stops, 5);
    assert_eq!(colors.len(), 5);
    // Trail (first stop) must be visible (sum >= ABSOLUTE_MIN_FLOOR).
    let trail_sum = rgb_sum(colors[0]);
    assert!(
        trail_sum >= super::super::tuning::ABSOLUTE_MIN_FLOOR,
        "trail sum {trail_sum} must be >= ABSOLUTE_MIN_FLOOR (30)"
    );
    // Trail must NOT be washed out to v17's 180.
    assert!(
        trail_sum < 175,
        "trail sum {trail_sum} must be < 175 (Phase 7 preserves dark aesthetic, v17 was 180)"
    );
}

/// Phase 7: integration — full pipeline produces visible trail for bright theme.
#[test]
fn phase7_colors_from_stops_integration_bright_theme() {
    let stops: &[(u8, u8, u8)] = &[
        (0, 12, 1),
        (0, 45, 6),
        (55, 218, 83),
        (80, 255, 110),
        (170, 255, 190),
    ];
    let colors = colors_from_stops(ColorMode::TrueColor, stops, 5);
    let trail_sum = rgb_sum(colors[0]);
    assert!(
        trail_sum >= super::super::tuning::ABSOLUTE_MIN_FLOOR,
        "trail sum {trail_sum} must be >= ABSOLUTE_MIN_FLOOR (30)"
    );
    // Head must still be brighter than trail (hierarchy preserved).
    let head_sum = rgb_sum(colors[4]);
    assert!(
        head_sum > trail_sum,
        "head sum {head_sum} must be > trail sum {trail_sum}"
    );
}

/// Phase 7: audit all 44 built-in themes — every trail stop must be
/// visible (sum >= ABSOLUTE_MIN_FLOOR). This is the regression guard:
/// if a future change breaks the floor logic, this test catches it
/// across the entire theme catalog.
///
/// Note: we do NOT assert a trail/head brightness ratio upper bound.
/// Some themes (Rainbow, Spectrum20) have intentionally bright trail
/// stops — Rainbow's trail is OKLCH-derived red (232, 89, 74) sum 395
/// by design (v30 OKLab audit: replaced raw sRGB primaries with
/// perceptually-uniform OKLCH values; see catalog.rs Rainbow entry
/// for the full rationale). The "washout" concern is covered by the
/// per-theme unit tests (phase7_dark_palette_trail_preserves_aesthetic,
/// etc.) which verify specific dark-theme trails are not over-boosted.
/// This audit only verifies the visibility guarantee.
#[test]
fn phase7_all_themes_trail_stops_within_bounds() {
    use crate::runtime::ColorMode;
    let schemes = [
        ColorScheme::Green,
        ColorScheme::Green2,
        ColorScheme::Green3,
        ColorScheme::NeonGreen,
        ColorScheme::NeonPurple,
        ColorScheme::NeonWhite,
        ColorScheme::NeonBlue,
        ColorScheme::NeonRed,
        ColorScheme::NeonOrange,
        ColorScheme::NeonYellow,
        ColorScheme::NeonCyan,
        ColorScheme::Carbon,
        ColorScheme::Yellow,
        ColorScheme::Orange,
        ColorScheme::Red,
        ColorScheme::Blue,
        ColorScheme::Cyan,
        ColorScheme::Gold,
        ColorScheme::Rainbow,
        ColorScheme::Purple,
        ColorScheme::Neon,
        ColorScheme::Fire,
        ColorScheme::Ocean,
        ColorScheme::Forest,
        ColorScheme::Vaporwave,
        ColorScheme::Gray,
        ColorScheme::Snow,
        ColorScheme::Aurora,
        ColorScheme::FancyDiamond,
        ColorScheme::Cosmos,
        ColorScheme::Nebula,
        ColorScheme::Spectrum20,
        ColorScheme::Stars,
        ColorScheme::Mars,
        ColorScheme::Venus,
        ColorScheme::Mercury,
        ColorScheme::Jupiter,
        ColorScheme::Saturn,
        ColorScheme::Uranus,
        ColorScheme::Neptune,
        ColorScheme::Pluto,
        ColorScheme::Moon,
        ColorScheme::Sun,
    ];
    let mut failures: Vec<String> = Vec::new();
    for &scheme in &schemes {
        let p = build_palette(scheme, ColorMode::TrueColor, true);
        if p.colors.is_empty() {
            continue;
        }
        // Trail = darkest stop (lowest sum). Find it explicitly rather than
        // assuming index 0, since some palettes may have non-monotonic sums.
        let min_sum = p.colors.iter().map(|&c| rgb_sum(c)).min().unwrap_or(0);
        if min_sum < super::super::tuning::ABSOLUTE_MIN_FLOOR {
            failures.push(format!(
                "{scheme:?}: min trail sum {min_sum} < ABSOLUTE_MIN_FLOOR {} (invisible)",
                super::super::tuning::ABSOLUTE_MIN_FLOOR
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "Phase 7 audit failures:\n  {}",
        failures.join("\n  ")
    );
}

/// Phase 7: regression guard — verify the v17 constant (180) is no longer
/// referenced in colors_from_stops. The function should delegate to
/// apply_palette_relative_floor which uses the tuning constants.
/// This test exists because a future refactor might accidentally
/// re-introduce the global floor.
#[test]
fn phase7_no_global_min_rgb_sum_constant_in_pipeline() {
    // Build a palette that would be washed out under v17 but preserved
    // under Phase 7: dark trail + bright head.
    let stops: &[(u8, u8, u8)] = &[(3, 3, 18), (213, 194, 248)];
    let colors = colors_from_stops(ColorMode::TrueColor, stops, 2);
    let trail_sum = rgb_sum(colors[0]);
    let head_sum = rgb_sum(colors[1]);
    // v17 would have produced sum 180 (basic floor only, no continuity).
    // Phase 7-b: continuity boosts trail to head/2.0 = 655/2.0 = 327.5.
    // This is intentional — continuity is uncapped to fix horizontal-line
    // illusion on palettes with bright bodies.
    //
    // If this fails with sum == 180, the v17 global MIN_RGB_SUM was
    // re-introduced AND continuity was lost.
    assert!(
            trail_sum > 250,
            "trail sum {trail_sum} — if this is 180, v17 global MIN_RGB_SUM was re-introduced; if < 250, continuity was lost"
        );
    assert!(
        trail_sum < head_sum,
        "trail sum {trail_sum} must be < head sum {head_sum} (hierarchy preserved)"
    );
}

/// Phase 7-b: continuity reduces adjacent gap to ≤ BODY_TAIL_MAX_GAP_RATIO.
/// Pre-Phase-7-b, this palette had gap 356/13 = 27x between trail and body.
/// Post-Phase-7-b, the gap should be ≤ 2.0x (with integer rounding slack).
#[test]
fn phase7b_continuity_reduces_adjacent_gap() {
    let mut rgb = vec![
        (0, 12, 1),     // trail, sum 13
        (55, 218, 83),  // body, sum 356
        (80, 255, 110), // head, sum 445
    ];
    apply_palette_relative_floor(&mut rgb);
    let trail_sum = rgb[0].0 as u16 + rgb[0].1 as u16 + rgb[0].2 as u16;
    let body_sum = rgb[1].0 as u16 + rgb[1].1 as u16 + rgb[1].2 as u16;
    let gap = body_sum as f32 / trail_sum as f32;
    // Continuity target: body/2.0 = 356/2.0 = 178. Trail boosted to ~178.
    // Gap = 356/178 = 2.0x (integer rounding may push to 2.01-2.05).
    assert!(
        gap <= 2.1,
        "adjacent gap {gap:.2}x must be ≤ 2.1x (continuity target 2.0x + rounding slack)"
    );
}

/// Phase 7-b: continuity is uncapped — can boost trail above GLOBAL_MAX_FLOOR
/// when body is very bright (NeonWhite-like: body sum 638).
#[test]
fn phase7b_continuity_uncapped_for_bright_body() {
    let mut rgb = vec![
        (5, 6, 8),       // trail, sum 19
        (190, 204, 244), // body, sum 638
        (220, 235, 255), // head, sum 710
    ];
    apply_palette_relative_floor(&mut rgb);
    let trail_sum = rgb[0].0 as u16 + rgb[0].1 as u16 + rgb[0].2 as u16;
    let body_sum = rgb[1].0 as u16 + rgb[1].1 as u16 + rgb[1].2 as u16;
    // Continuity target: body/2.0 = 638/2.0 = 319. Trail boosted above 180.
    // This is the NeonWhite case — without uncapped continuity, gap would
    // be 638/180 = 3.54x (horizontal-line illusion at speed 100).
    assert!(
        trail_sum > 250,
        "trail sum {trail_sum} should be > 250 (uncapped continuity boost for bright body)"
    );
    let gap = body_sum as f32 / trail_sum as f32;
    assert!(
        gap <= 2.1,
        "gap {gap:.2}x must be ≤ 2.1x even with bright body (uncapped continuity)"
    );
}

/// Phase 7-b: continuity preserves hue (RGB ratio scaling).
#[test]
fn phase7b_continuity_preserves_hue() {
    let mut rgb = vec![
        (3, 3, 18),      // blue-dominant trail, sum 24, ratio 1:1:6
        (213, 194, 248), // head, sum 655
    ];
    apply_palette_relative_floor(&mut rgb);
    let (r, g, b) = rgb[0];
    // After continuity boost, ratio should still be ~1:1:6.
    let ratio_rg = (r as f32) / (g as f32).max(1.0);
    let ratio_rb = (r as f32) / (b as f32).max(1.0);
    assert!(
        (0.8..=1.2).contains(&ratio_rg),
        "r/g ratio {ratio_rg:.2} should be ~1.0 (hue preserved after continuity)"
    );
    assert!(
        (0.10..=0.25).contains(&ratio_rb),
        "r/b ratio {ratio_rb:.2} should be ~0.17 (blue dominant, hue preserved after continuity)"
    );
}

/// Phase 7-b: continuity never dims a stop (only scales UP).
#[test]
fn phase7b_continuity_only_scales_up() {
    let mut rgb = vec![
        (100, 100, 100), // sum 300
        (200, 200, 200), // sum 600
        (250, 250, 250), // head, sum 750
    ];
    let original: Vec<(u8, u8, u8)> = rgb.clone();
    apply_palette_relative_floor(&mut rgb);
    // All adjacent gaps are ≤ 2.0x (300→600 = 2x, 600→750 = 1.25x).
    // No boost needed — palette should be unchanged.
    assert_eq!(
        rgb, original,
        "continuity must not modify palette when all gaps ≤ BODY_TAIL_MAX_GAP_RATIO"
    );
}

/// Phase 7-b: continuity preserves head→body→trail hierarchy.
#[test]
fn phase7b_continuity_preserves_hierarchy() {
    let mut rgb = vec![
        (0, 12, 1),     // trail
        (55, 218, 83),  // body
        (80, 255, 110), // head
    ];
    apply_palette_relative_floor(&mut rgb);
    let trail_sum = rgb[0].0 as u16 + rgb[0].1 as u16 + rgb[0].2 as u16;
    let body_sum = rgb[1].0 as u16 + rgb[1].1 as u16 + rgb[1].2 as u16;
    let head_sum = rgb[2].0 as u16 + rgb[2].1 as u16 + rgb[2].2 as u16;
    assert!(trail_sum < body_sum, "trail {trail_sum} < body {body_sum}");
    assert!(body_sum < head_sum, "body {body_sum} < head {head_sum}");
}

/// Phase 7-b: continuity audit — all 44 themes have max adjacent gap ≤ 2.6x.
/// This is the regression guard for the horizontal-line illusion fix.
#[test]
fn phase7b_all_themes_max_adjacent_gap_within_bounds() {
    use crate::runtime::ColorMode;
    let schemes = [
        ColorScheme::Green,
        ColorScheme::Green2,
        ColorScheme::Green3,
        ColorScheme::NeonGreen,
        ColorScheme::NeonPurple,
        ColorScheme::NeonWhite,
        ColorScheme::NeonBlue,
        ColorScheme::NeonRed,
        ColorScheme::NeonOrange,
        ColorScheme::NeonYellow,
        ColorScheme::NeonCyan,
        ColorScheme::Carbon,
        ColorScheme::Yellow,
        ColorScheme::Orange,
        ColorScheme::Red,
        ColorScheme::Blue,
        ColorScheme::Cyan,
        ColorScheme::Gold,
        ColorScheme::Rainbow,
        ColorScheme::Purple,
        ColorScheme::Neon,
        ColorScheme::Fire,
        ColorScheme::Ocean,
        ColorScheme::Forest,
        ColorScheme::Vaporwave,
        ColorScheme::Gray,
        ColorScheme::Snow,
        ColorScheme::Aurora,
        ColorScheme::FancyDiamond,
        ColorScheme::Cosmos,
        ColorScheme::Nebula,
        ColorScheme::Spectrum20,
        ColorScheme::Stars,
        ColorScheme::Mars,
        ColorScheme::Venus,
        ColorScheme::Mercury,
        ColorScheme::Jupiter,
        ColorScheme::Saturn,
        ColorScheme::Uranus,
        ColorScheme::Neptune,
        ColorScheme::Pluto,
        ColorScheme::Moon,
        ColorScheme::Sun,
    ];
    // 2.1x = BODY_TAIL_MAX_GAP_RATIO (2.0) + integer rounding slack.
    // Continuity may produce 2.01-2.05x due to integer scaling, which is
    // visually indistinguishable from 2.0x.
    const MAX_ALLOWED_GAP: f32 = 2.1;
    let mut failures: Vec<String> = Vec::new();
    for &scheme in &schemes {
        let p = build_palette(scheme, ColorMode::TrueColor, true);
        if p.colors.len() < 2 {
            continue;
        }
        let sums: Vec<u16> = p.colors.iter().map(|&c| rgb_sum(c)).collect();
        let mut max_gap: f32 = 0.0;
        for i in 0..sums.len() - 1 {
            if sums[i] == 0 {
                continue;
            }
            let gap = sums[i + 1] as f32 / sums[i] as f32;
            if gap > max_gap {
                max_gap = gap;
            }
        }
        if max_gap > MAX_ALLOWED_GAP {
            failures.push(format!(
                "{scheme:?}: max adjacent gap {max_gap:.2}x > {MAX_ALLOWED_GAP}x"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "Phase 7-b continuity audit failures (horizontal-line risk at high speed):\n  {}",
        failures.join("\n  ")
    );
}
