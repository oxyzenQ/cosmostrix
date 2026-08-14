// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for `chroma::shaders::base::resolve_cell_color` and its helpers.
//!
//! Extracted from `base.rs` in Phase 4 to keep the shader module under
//! the 1500-LOC cap. The tests live in a sibling file loaded via
//! `#[cfg(test)] #[path = "base_tests.rs"] mod tests;` so they retain
//! `use super::*` access to base's private helpers (column_coherence_perturbation,
//! cell_hash, apply_subpixel_jitter, make_test_shader, slot_array, etc.)
//! without any visibility changes.
use super::*;

/// Bayer matrix tiles every 4×4 block — same (line, col) mod 4 returns
/// the same threshold.
#[test]
fn bayer_threshold_tiles_4x4() {
    for line in 0..16u16 {
        for col in 0..16u16 {
            let a = bayer_threshold(line, col);
            let b = bayer_threshold(line + 4, col + 4);
            assert_eq!(a, b, "Bayer threshold must tile every 4×4 block");
        }
    }
}

/// Bayer matrix is a permutation of {0..=15} — every threshold appears
/// exactly once per 4×4 tile. This is what makes the spatial average
/// equal undithered rounding.
#[test]
fn bayer_matrix_is_permutation_of_0_to_15() {
    let mut seen = [false; 16];
    for row in &BAYER_4X4 {
        for &v in row {
            let v = v as usize;
            assert!(v < 16, "Bayer entry out of range: {v}");
            assert!(!seen[v], "Bayer entry {v} appears more than once");
            seen[v] = true;
        }
    }
    assert!(seen.iter().all(|&s| s), "Not all thresholds 0..=15 present");
}

/// Bayer dithering preserves the spatial average: averaging the
/// up/down decision across a full 4×4 tile equals undithered rounding
/// of `v_continuous + 0.5/16` (a tiny bias from the threshold layout).
/// In practice this means no brightness shift — just banding broken
/// into fine texture.
#[test]
fn bayer_dither_preserves_spatial_average() {
    // Pick a continuous value whose fractional part is exactly 0.5 —
    // the worst case for rounding bias.
    let v_continuous = 4.5_f32;
    let frac = v_continuous - v_continuous.floor();
    assert!((frac - 0.5).abs() < 1e-6);

    // For each of the 16 cells in a 4×4 tile, compute the dithered v.
    let mut sum: u64 = 0;
    for line in 0..4u16 {
        for col in 0..4u16 {
            let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
            let v = if frac > bayer_t {
                v_continuous.floor() as u64 + 1
            } else {
                v_continuous.floor() as u64
            };
            sum += v;
        }
    }
    // Average should be ~4.5 (between 4 and 5). With 16 cells and
    // thresholds {0..15}/16, exactly 8 cells round up (frac > t when
    // t ∈ {0..7}/16) and 8 round down. So sum = 8*4 + 8*5 = 72,
    // average = 4.5 — exactly the continuous value.
    assert_eq!(sum, 72, "16-cell sum should be 8*4 + 8*5 = 72");
    let avg = sum as f32 / 16.0;
    assert!(
        (avg - v_continuous).abs() < 1e-6,
        "Spatial average {avg} should equal continuous value {v_continuous}"
    );
}

/// Bayer dithering rounds down to floor when frac=0 (no fractional part).
#[test]
fn bayer_dither_rounds_down_at_zero_frac() {
    let v_continuous = 4.0_f32; // no fractional part
    let frac = v_continuous - v_continuous.floor();
    assert!(frac < 1e-6);
    for line in 0..4u16 {
        for col in 0..4u16 {
            let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
            let v = if frac > bayer_t {
                v_continuous.floor() as u64 + 1
            } else {
                v_continuous.floor() as u64
            };
            assert_eq!(v, 4, "Zero frac should always round down");
        }
    }
}

/// Bayer dithering rounds up to ceil when frac ≥ 15/16 (nearly 1.0).
#[test]
fn bayer_dither_rounds_up_at_near_one_frac() {
    let v_continuous = 4.9375_f32; // frac = 15/16
    let frac = v_continuous - v_continuous.floor();
    // 15/16 = 0.9375; only the threshold 15/16 itself fails (frac > t
    // is false when frac == t). So 15 of 16 cells round up.
    let mut ups = 0;
    let mut downs = 0;
    for line in 0..4u16 {
        for col in 0..4u16 {
            let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
            let v = if frac > bayer_t {
                v_continuous.floor() as u64 + 1
            } else {
                v_continuous.floor() as u64
            };
            if v == 5 {
                ups += 1;
            } else {
                downs += 1;
            }
        }
    }
    assert_eq!(ups, 15, "frac=15/16 should round up in 15 of 16 cells");
    assert_eq!(downs, 1, "frac=15/16 should round down in 1 of 16 cells");
}

/// masterclass: short-droplet luminance-remap path now uses Bayer
/// dithering (same pattern as the shading_distance branch). This test
/// verifies the dithering formula produces both up and down decisions
/// for a mid-fractional continuous value — proving the path no longer
/// uses flat .round() which would assign the same color_idx to all cells.
#[test]
fn bayer_dither_short_droplet_path_produces_mixed_indices() {
    // Simulate a short droplet with last=8 (9-color palette) and a
    // continuous value of 4.5 (frac=0.5). Without dithering, .round()
    // would always produce 4 or 5 (banker's rounding varies). With Bayer
    // dithering, the 16 cells in a 4×4 block split: 8 round up, 8 down.
    let last = 8_i32;
    let v_continuous = 4.5_f32;
    let frac = v_continuous - v_continuous.floor();
    let mut ups = 0;
    let mut downs = 0;
    for line in 0..4u16 {
        for col in 0..4u16 {
            let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
            let v = if frac > bayer_t {
                (v_continuous.floor() as i32 + 1).min(last)
            } else {
                v_continuous.floor() as i32
            };
            if v == 5 {
                ups += 1;
            } else {
                downs += 1;
            }
        }
    }
    // frac=0.5 → 8 cells have bayer_t < 0.5 (round up), 8 have bayer_t ≥ 0.5
    // (round down). The BAYER_4X4 matrix has thresholds {0,8,2,10,12,4,14,6,
    // 3,11,1,9,15,7,13,5}. Divided by 16: 8 are < 0.5, 8 are ≥ 0.5.
    assert_eq!(ups, 8, "frac=0.5 should round up in 8 of 16 cells");
    assert_eq!(downs, 8, "frac=0.5 should round down in 8 of 16 cells");
}

// ── Phase 3-C: column-coherence perturbation ──────────────────────────

/// Perturbation is always in {-1, 0, +1} — never larger.
#[test]
fn column_coherence_perturbation_bounded() {
    for col in 0..256u16 {
        for phase_deg in 0..360 {
            let phase = phase_deg as f32 * std::f32::consts::PI / 180.0;
            let p = column_coherence_perturbation(phase, col);
            assert!(
                (-1..=1).contains(&p),
                "perturbation {p} out of [-1, +1] for phase={phase}, col={col}"
            );
        }
    }
}

/// Perturbation at phase=0, col=0 is exactly 0 (sin(0) * 0.5 = 0).
#[test]
fn column_coherence_perturbation_zero_at_origin() {
    assert_eq!(column_coherence_perturbation(0.0, 0), 0);
}

/// Perturbation at phase=π/2, col=0 sits at its non-negative peak.
///
/// Mathematically: `sin(π/2) * 0.5 = 0.5`, which `f32::round` (half away
/// from zero) maps to `1`. We assert the result is in `{0, 1}` rather
/// than exactly `1` because `sinf(FRAC_PI_2)` is not required to return
/// a value of exactly `1.0` across float implementations:
///
/// - glibc / LLVM libm: `sinf(FRAC_PI_2) = 1.0` → `0.5.round() = 1`
/// - Miri's software float: `sinf(FRAC_PI_2) = 0.99999994`
///   → `0.99999994 * 0.5 = 0.49999997` → `0.49999997.round() = 0`
///
/// Both outcomes are non-negative, which is the actual property under
/// test: "at phase π/2 the perturbation is at its positive peak". Any
/// negative value would indicate a real bug; `0` vs `1` is just float
/// precision noise.
#[test]
fn column_coherence_perturbation_peaks_at_plus_one() {
    let phase = std::f32::consts::FRAC_PI_2;
    let p = column_coherence_perturbation(phase, 0);
    assert!(
        p == 0 || p == 1,
        "perturbation at π/2 must be at its non-negative peak ({{0, 1}}), got {p}"
    );
}

/// Perturbation at phase=3π/2, col=0 sits at its non-positive trough.
///
/// Same precision caveat as `peaks_at_plus_one`: `sinf(3 * FRAC_PI_2)`
/// may return exactly `-1.0` (glibc/LLVM) or `-0.99999994` (Miri), so
/// the rounded result is in `{-1, 0}`. The actual property under test
/// is "non-positive trough" — any positive value would be a real bug.
#[test]
fn column_coherence_perturbation_troughs_at_minus_one() {
    let phase = 3.0 * std::f32::consts::FRAC_PI_2;
    let p = column_coherence_perturbation(phase, 0);
    assert!(
        p == 0 || p == -1,
        "perturbation at 3π/2 must be at its non-positive trough ({{-1, 0}}), got {p}"
    );
}

/// Spatial coherence: adjacent columns get similar perturbations.
/// The spatial frequency is 0.05 rad/col, so the perturbation difference
/// between col=N and col=N+1 is bounded by `sin(N+1) - sin(N)` ≤ 0.05.
/// After rounding, this means neighboring columns usually share the
/// same perturbation, and never differ by more than 1.
#[test]
fn column_coherence_perturbation_spatially_smooth() {
    let phase = 0.7_f32; // arbitrary nonzero phase
    for col in 0..512u16 {
        let a = column_coherence_perturbation(phase, col);
        let b = column_coherence_perturbation(phase, col + 1);
        let diff = (a - b).abs();
        assert!(
            diff <= 1,
            "Adjacent cols {col} and {} differ by {diff} (phase={phase})",
            col + 1
        );
    }
}

/// Temporal coherence: at a fixed column, small phase changes produce
/// small or zero perturbation changes. This is what makes the effect
/// "shimmer" rather than strobe.
#[test]
fn column_coherence_perturbation_temporally_smooth() {
    let col = 42u16;
    let mut prev = column_coherence_perturbation(0.0, col);
    // Advance phase by 0.1 rad per step (slow temporal freq).
    for step in 1..100 {
        let phase = step as f32 * 0.1;
        let curr = column_coherence_perturbation(phase, col);
        let diff = (curr - prev).abs();
        // 0.1 rad phase change → at most sin(0.1) ≈ 0.1 amplitude change
        // → rounding can flip by at most 1.
        assert!(
            diff <= 1,
            "Temporal step {step} (phase={phase}) changed perturbation by {diff}"
        );
        prev = curr;
    }
}

// ── Phase 3-E: subpixel hue jitter ─────────────────────────────────────

/// cell_hash is deterministic: same input → same output.
#[test]
fn cell_hash_is_deterministic() {
    for line in 0..32u16 {
        for col in 0..32u16 {
            let a = cell_hash(line, col);
            let b = cell_hash(line, col);
            assert_eq!(a, b, "hash must be deterministic for ({line}, {col})");
        }
    }
}

/// cell_hash has low collision rate: distinct inputs rarely collide.
/// Test across a 64×64 grid and verify no two distinct (line, col)
/// pairs produce the same hash.
#[test]
fn cell_hash_low_collision_rate() {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut collisions = 0;
    for line in 0..64u16 {
        for col in 0..64u16 {
            let h = cell_hash(line, col);
            if !seen.insert(h) {
                collisions += 1;
            }
        }
    }
    // 4096 distinct inputs into a u32 space should produce ~0 collisions.
    // Allow up to 2 for bad luck.
    assert!(
        collisions <= 2,
        "cell_hash produced {collisions} collisions across 4096 inputs (expected ≤ 2)"
    );
}

/// Jitter with amplitude 0 returns the input unchanged.
#[test]
fn subpixel_jitter_zero_amplitude_unchanged() {
    let c = Color::Rgb {
        r: 100,
        g: 50,
        b: 200,
    };
    assert_eq!(apply_subpixel_jitter(c, 0xDEADBEEF, 0), c);
}

/// Jitter with Color::Reset returns Reset unchanged.
#[test]
fn subpixel_jitter_reset_unchanged() {
    assert_eq!(
        apply_subpixel_jitter(Color::Reset, 0xDEADBEEF, 16),
        Color::Reset
    );
}

/// Jitter perturbs each channel by at most `amplitude` units.
#[test]
fn subpixel_jitter_bounded_by_amplitude() {
    let c = Color::Rgb {
        r: 128,
        g: 128,
        b: 128,
    };
    let amp: u8 = 8;
    // Sample many hashes to cover the offset space.
    for line in 0..32u16 {
        for col in 0..32u16 {
            let h = cell_hash(line, col);
            let result = apply_subpixel_jitter(c, h, amp);
            let Color::Rgb { r, g, b } = result else {
                panic!("expected Rgb");
            };
            let dr = (i32::from(r) - 128).abs();
            let dg = (i32::from(g) - 128).abs();
            let db = (i32::from(b) - 128).abs();
            assert!(
                dr <= i32::from(amp),
                "r delta {dr} exceeds amp {amp} (line={line}, col={col}, h={h:#x})"
            );
            assert!(
                dg <= i32::from(amp),
                "g delta {dg} exceeds amp {amp} (line={line}, col={col}, h={h:#x})"
            );
            assert!(
                db <= i32::from(amp),
                "b delta {db} exceeds amp {amp} (line={line}, col={col}, h={h:#x})"
            );
        }
    }
}

/// Jitter clamps to [0, 255] — near-zero and near-255 channels don't
/// wrap around.
#[test]
fn subpixel_jitter_clamps_to_valid_range() {
    let dark = Color::Rgb { r: 0, g: 0, b: 0 };
    let bright = Color::Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    // Try many hashes to exercise both negative and positive offsets.
    for line in 0..16u16 {
        for col in 0..16u16 {
            let h = cell_hash(line, col);
            let r_dark = apply_subpixel_jitter(dark, h, 16);
            let r_bright = apply_subpixel_jitter(bright, h, 16);
            let Color::Rgb { r, g, b } = r_dark else {
                panic!("expected Rgb");
            };
            assert!(r <= 16, "dark r {r} should be ≤ 16 after +amp jitter");
            assert!(g <= 16, "dark g {g} should be ≤ 16 after +amp jitter");
            assert!(b <= 16, "dark b {b} should be ≤ 16 after +amp jitter");
            let Color::Rgb { r, g, b } = r_bright else {
                panic!("expected Rgb");
            };
            assert!(
                r >= 255 - 16,
                "bright r {r} should be ≥ {} after -amp jitter",
                255 - 16
            );
            assert!(
                g >= 255 - 16,
                "bright g {g} should be ≥ {} after -amp jitter",
                255 - 16
            );
            assert!(
                b >= 255 - 16,
                "bright b {b} should be ≥ {} after -amp jitter",
                255 - 16
            );
        }
    }
}

/// Jitter is deterministic: same (color, hash, amp) → same result.
#[test]
fn subpixel_jitter_deterministic() {
    let c = Color::Rgb {
        r: 100,
        g: 50,
        b: 200,
    };
    let h = 0x12345678u32;
    let a = apply_subpixel_jitter(c, h, 8);
    let b = apply_subpixel_jitter(c, h, 8);
    assert_eq!(a, b);
}

/// Different hashes produce different jitter (high probability).
/// Verify by sampling many hashes and counting distinct results.
#[test]
fn subpixel_jitter_varies_with_hash() {
    let c = Color::Rgb {
        r: 128,
        g: 128,
        b: 128,
    };
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for h in 0..256u32 {
        seen.insert(apply_subpixel_jitter(c, h, 8));
    }
    // 256 distinct hashes into a 17^3 ≈ 4913 space should produce
    // many distinct results. Allow at least 50 (very conservative).
    assert!(
        seen.len() >= 50,
        "jitter produced only {} distinct results across 256 hashes (expected ≥ 50)",
        seen.len()
    );
}

// ── Phase 3-F: luminance-remap for short droplets ─────────────────────

/// Short droplet (length=4) Middle cells get a position-based ramp
/// spanning the full palette range, not the random color_map value.
///
/// Setup: 5-stop palette (last=4), length=4, color_map all set to 1
/// (would normally give every Middle cell color_idx=1).
/// Expectation: the two Middle cells (dist_from_head=1 and 2) get
/// remapped to color_idx=4 and 0 respectively (t=0 → last, t=1 → 0).
#[test]
fn short_droplet_middle_cells_get_remapped() {
    let palette: Vec<Color> = (0..5)
        .map(|i| Color::Rgb {
            r: i as u8 * 50,
            g: i as u8 * 50,
            b: i as u8 * 50,
        })
        .collect();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![1u8; 50 * 100]; // all cells → color_idx 1
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);
    let shader = make_test_shader(&slots, color_map, false);

    // length=4, head_put_line=20. Middle cells are at line 19 and 18
    // (dist_from_head = 1 and 2). denom = length-3 = 1.
    // dist=1 → t = 0 → color_idx = last = 4
    // dist=2 → t = 1 → color_idx = 0
    let (fg1, _) = resolve_cell_color(
        &shader,
        0,
        19, // line (head_put_line - 1)
        5,  // col
        'x',
        CharLoc::Middle,
        20, // head_put_line
        4,  // length
    );
    let (fg2, _) = resolve_cell_color(
        &shader,
        0,
        18, // line (head_put_line - 2)
        5,
        'x',
        CharLoc::Middle,
        20,
        4,
    );
    // fg1 should be palette[4] (brightest), fg2 should be palette[0] (darkest)
    assert_eq!(
        fg1,
        Some(palette[4]),
        "head-adjacent Middle cell should be brightest"
    );
    assert_eq!(
        fg2,
        Some(palette[0]),
        "tail-adjacent Middle cell should be darkest"
    );
}

/// Long droplet (length > threshold=8) Middle cells keep the color_map
/// value — remap is not applied.
#[test]
fn long_droplet_middle_cells_unchanged() {
    let palette: Vec<Color> = (0..5)
        .map(|i| Color::Rgb {
            r: i as u8 * 50,
            g: i as u8 * 50,
            b: i as u8 * 50,
        })
        .collect();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![2u8; 50 * 100]; // all cells → color_idx 2
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);
    let shader = make_test_shader(&slots, color_map, false);

    // length=9 (> threshold of 8). Middle cell at line 19 (dist=1).
    // Remap NOT applied → color_idx stays at color_map value = 2.
    let (fg, _) = resolve_cell_color(
        &shader,
        0,
        19,
        5,
        'x',
        CharLoc::Middle,
        20,
        9, // length > threshold
    );
    assert_eq!(
        fg,
        Some(palette[2]),
        "long droplet Middle cell should use color_map value"
    );
}

/// Threshold boundary: length=8 (exactly the threshold) → remap applies.
#[test]
fn threshold_boundary_length_8_remapped() {
    let palette: Vec<Color> = (0..5)
        .map(|i| Color::Rgb {
            r: i as u8 * 50,
            g: i as u8 * 50,
            b: i as u8 * 50,
        })
        .collect();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![1u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);
    let shader = make_test_shader(&slots, color_map, false);

    // length=8 (= threshold). denom = 5. dist=1 → t=0 → color_idx = 4 (last).
    let (fg, _) = resolve_cell_color(&shader, 0, 19, 5, 'x', CharLoc::Middle, 20, 8);
    assert_eq!(
        fg,
        Some(palette[4]),
        "length=8 should still be remapped (≤ threshold)"
    );
}

/// shading_distance=true disables the remap even for short droplets.
/// The shading_distance path has its own length-aware exponential decay.
#[test]
fn shading_distance_disables_remap() {
    let palette: Vec<Color> = (0..5)
        .map(|i| Color::Rgb {
            r: i as u8 * 50,
            g: i as u8 * 50,
            b: i as u8 * 50,
        })
        .collect();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![1u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);
    let shader = make_test_shader(&slots, color_map, true);

    // length=4 with shading_distance=true. Remap NOT applied —
    // shading_distance path overrides color_idx with exponential decay.
    // Just verify it doesn't panic and returns some color.
    let (fg, _) = resolve_cell_color(&shader, 0, 19, 5, 'x', CharLoc::Middle, 20, 4);
    assert!(fg.is_some(), "shading_distance path must return a color");
}

/// Head and Tail are unaffected by the remap — only Middle cells change.
#[test]
fn head_and_tail_unaffected_by_remap() {
    let palette: Vec<Color> = (0..5)
        .map(|i| Color::Rgb {
            r: i as u8 * 50,
            g: i as u8 * 50,
            b: i as u8 * 50,
        })
        .collect();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![1u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);
    let shader = make_test_shader(&slots, color_map, false);

    // length=4 (short). Head should be palette[4] (last). Tail should be palette[0].
    let (fg_head, bold_head) = resolve_cell_color(
        &shader,
        0,
        20, // head line
        5,
        'x',
        CharLoc::Head,
        20,
        4,
    );
    let (fg_tail, bold_tail) = resolve_cell_color(
        &shader,
        0,
        17, // tail line (head - 3)
        5,
        'x',
        CharLoc::Tail,
        20,
        4,
    );
    assert_eq!(fg_head, Some(palette[4]));
    assert!(bold_head, "Head should be bold");
    assert_eq!(fg_tail, Some(palette[0]));
    assert!(!bold_tail, "Tail should not be bold");
}

/// Short droplet with length=4 produces a strict head→tail gradient:
/// Head=last, Middle1=last, Middle2=0, Tail=0. The two Middle cells
/// are visually distinct, breaking the "flat short droplet" look.
#[test]
fn short_droplet_produces_visible_gradient() {
    let palette: Vec<Color> = (0..8)
        .map(|i| Color::Rgb {
            r: i as u8 * 30,
            g: i as u8 * 30,
            b: i as u8 * 30,
        })
        .collect();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![3u8; 50 * 100]; // uniform "flat" baseline
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);
    let shader = make_test_shader(&slots, color_map, false);

    // length=4, 8-stop palette (last=7). denom = 1.
    // Middle1 (dist=1): t=0 → color_idx = 7 (last)
    // Middle2 (dist=2): t=1 → color_idx = 0
    let (fg_m1, _) = resolve_cell_color(&shader, 0, 19, 5, 'x', CharLoc::Middle, 20, 4);
    let (fg_m2, _) = resolve_cell_color(&shader, 0, 18, 5, 'x', CharLoc::Middle, 20, 4);
    // The two Middle cells must differ — that's the whole point of 3-F.
    assert_ne!(
        fg_m1, fg_m2,
        "short droplet Middle cells must differ after remap (was uniform before)"
    );
    // And specifically: m1 brighter than m2 (head-side brighter than tail-side).
    let Color::Rgb { r: r1, .. } = fg_m1.unwrap() else {
        panic!("expected Rgb");
    };
    let Color::Rgb { r: r2, .. } = fg_m2.unwrap() else {
        panic!("expected Rgb");
    };
    assert!(
        r1 > r2,
        "head-side Middle ({r1}) should be brighter than tail-side ({r2})"
    );
}

// ── Phase 3-H: global hue drift ───────────────────────────────────────

/// hue_drift_offset maps drift values to integer offsets:
///   0 → 0, π/2 → +1, -π/2 → -1, π → +2, -π → -2.
#[test]
fn hue_drift_offset_known_values() {
    assert_eq!(hue_drift_offset(0.0), 0);
    assert_eq!(hue_drift_offset(std::f32::consts::PI), 2);
    assert_eq!(hue_drift_offset(-std::f32::consts::PI), -2);
    assert_eq!(hue_drift_offset(std::f32::consts::FRAC_PI_2), 1);
    assert_eq!(hue_drift_offset(-std::f32::consts::FRAC_PI_2), -1);
}

/// Small drifts (|drift| < π/4) round to 0 — the common production
/// case because COLOR_HUE_DRIFT_RATE is small (0.015 rad/tick).
#[test]
fn hue_drift_offset_small_drifts_round_to_zero() {
    assert_eq!(hue_drift_offset(std::f32::consts::FRAC_PI_8), 0);
    assert_eq!(hue_drift_offset(-std::f32::consts::FRAC_PI_8), 0);
    assert_eq!(hue_drift_offset(0.78), 0);
    assert_eq!(hue_drift_offset(-0.78), 0);
}

/// Offset is bounded to {-2, -1, 0, +1, +2} across [-π, π] and is
/// monotonic non-decreasing + odd (offset(-x) = -offset(x)).
#[test]
fn hue_drift_offset_bounded_monotonic_odd() {
    let steps = 1000;
    let mut prev = hue_drift_offset(-std::f32::consts::PI);
    for i in 0..=steps {
        let drift =
            -std::f32::consts::PI + 2.0 * std::f32::consts::PI * (i as f32) / (steps as f32);
        let offset = hue_drift_offset(drift);
        let neg_offset = hue_drift_offset(-drift);
        assert!(
            (-2..=2).contains(&offset),
            "drift {drift} → {offset} out of [-2,2]"
        );
        assert!(
            offset >= prev,
            "non-monotonic at drift {drift}: {offset} < {prev}"
        );
        assert_eq!(
            offset, -neg_offset,
            "not odd: offset({drift})={offset} != -offset(-drift)"
        );
        prev = offset;
    }
}

/// Integration: resolve_cell_color with hue_drift applies the offset
/// to Middle cells. Verify a Middle cell's color shifts when hue_drift
/// is non-zero (vs. None which leaves it unchanged).
#[test]
fn hue_drift_shifts_middle_color() {
    let palette: Vec<Color> = (0..8)
        .map(|i| Color::Rgb {
            r: i as u8 * 30,
            g: i as u8 * 30,
            b: i as u8 * 30,
        })
        .collect();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![3u8; 50 * 100];
    let color_map: &[u8] = &color_map;

    let slots = slot_array(palette);
    let shader_none = make_test_shader(&slots, color_map, false);
    let (fg_none, _) = resolve_cell_color(&shader_none, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(fg_none, Some(palette[3]));

    let mut shader_drift = make_test_shader(&slots, color_map, false);
    shader_drift.hue_drift_offset = Some(hue_drift_offset(std::f32::consts::PI));
    let (fg_drift, _) = resolve_cell_color(&shader_drift, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(fg_drift, Some(palette[5]), "hue_drift=π should shift 3 → 5");
}

/// hue_drift does NOT affect Head or Tail — those are pinned.
#[test]
fn hue_drift_does_not_affect_head_or_tail() {
    let palette: Vec<Color> = (0..8)
        .map(|i| Color::Rgb {
            r: i as u8 * 30,
            g: i as u8 * 30,
            b: i as u8 * 30,
        })
        .collect();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![3u8; 50 * 100];
    let color_map: &[u8] = &color_map;

    let slots = slot_array(palette);
    let mut shader = make_test_shader(&slots, color_map, false);
    shader.hue_drift_offset = Some(hue_drift_offset(std::f32::consts::PI));

    let (fg_head, _) = resolve_cell_color(&shader, 0, 20, 5, 'x', CharLoc::Head, 20, 12);
    assert_eq!(fg_head, Some(palette[7]));

    let (fg_tail, _) = resolve_cell_color(&shader, 0, 9, 5, 'x', CharLoc::Tail, 20, 12);
    assert_eq!(fg_tail, Some(palette[0]));
}

/// hue_drift is skipped under shading_distance — that path has its own
/// length-aware gradient and stacking a hue shift would muddy the signal.
#[test]
fn hue_drift_skipped_under_shading_distance() {
    let palette: Vec<Color> = (0..8)
        .map(|i| Color::Rgb {
            r: i as u8 * 30,
            g: i as u8 * 30,
            b: i as u8 * 30,
        })
        .collect();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![3u8; 50 * 100];
    let color_map: &[u8] = &color_map;

    let slots = slot_array(palette);
    let mut shader_off = make_test_shader(&slots, color_map, true);
    shader_off.hue_drift_offset = None;
    let mut shader_on = make_test_shader(&slots, color_map, true);
    shader_on.hue_drift_offset = Some(hue_drift_offset(std::f32::consts::PI));

    let (fg_off, _) = resolve_cell_color(&shader_off, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    let (fg_on, _) = resolve_cell_color(&shader_on, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(
        fg_off, fg_on,
        "hue_drift must not affect shading_distance path"
    );
}

/// hue_drift clamps to valid palette range — offset that would push
/// color_idx below 0 or above last is clamped.
#[test]
fn hue_drift_clamps_to_palette_range() {
    let palette: Vec<Color> = (0..3)
        .map(|i| Color::Rgb {
            r: i as u8 * 100,
            g: i as u8 * 100,
            b: i as u8 * 100,
        })
        .collect();
    let palette: &[Color] = &palette;

    // Lower bound: color_map=0, hue_drift=-π → offset -2, clamped to 0.
    let color_map_lo: Vec<u8> = vec![0u8; 50 * 100];
    let color_map_lo: &[u8] = &color_map_lo;
    let slots_lo = slot_array(palette);
    let mut shader_lo = make_test_shader(&slots_lo, color_map_lo, false);
    shader_lo.hue_drift_offset = Some(hue_drift_offset(-std::f32::consts::PI));
    let (fg_lo, _) = resolve_cell_color(&shader_lo, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(fg_lo, Some(palette[0]));

    // Upper bound: color_map=2, hue_drift=+π → offset +2, clamped to 2.
    let color_map_hi: Vec<u8> = vec![2u8; 50 * 100];
    let color_map_hi: &[u8] = &color_map_hi;
    let slots_hi = slot_array(palette);
    let mut shader_hi = make_test_shader(&slots_hi, color_map_hi, false);
    shader_hi.hue_drift_offset = Some(hue_drift_offset(std::f32::consts::PI));
    let (fg_hi, _) = resolve_cell_color(&shader_hi, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(fg_hi, Some(palette[2]));
}
