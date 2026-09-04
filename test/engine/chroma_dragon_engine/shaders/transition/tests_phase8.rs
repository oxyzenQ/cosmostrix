// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for Phase 8 hue-preserving chroma smoothing.
//!
//! Covers `apply_l_smoothing` + `TransitionLTable` Oklab population.
//! Extracted from `transition/tests.rs` to keep the source file under
//! the 800-LOC cap. Pure code motion — no behavior change.

use crossterm::style::Color;

use super::*;

// ─── Phase 8: hue-preserving chroma smoothing ───

#[test]
fn phase8_build_populates_full_oklab_entry() {
    // Phase 8 extends the entry to carry (L, a, b) per side, not just L.
    // Verify all 6 fields are populated and finite for a colored stop.
    let old = [Color::Rgb {
        r: 220,
        g: 30,
        b: 30,
    }]; // red
    let new = [Color::Rgb {
        r: 30,
        g: 220,
        b: 30,
    }]; // green
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();
    let e = &table.entries[0];

    // L: red and green have similar L in OKLab (both ~0.45).
    assert!(e.l_old.is_finite());
    assert!(e.l_new.is_finite());

    // a: red has positive a (≈+0.25), green has negative a (≈-0.25).
    assert!(
        e.a_old > 0.0,
        "red a_old must be positive (red is on +a axis), got {}",
        e.a_old
    );
    assert!(
        e.a_new < 0.0,
        "green a_new must be negative (green is on -a axis), got {}",
        e.a_new
    );

    // b: both red and green have small positive b (slight yellow bias).
    assert!(e.b_old.is_finite());
    assert!(e.b_new.is_finite());
}

#[test]
fn phase8_chroma_smoothing_avoids_gray_midpoint_for_opposing_hues() {
    // Phase 8's key win: red → cyan should rotate through magenta or
    // yellow (whichever is shorter), NOT pass through gray.
    //
    // Cartesian (a, b) lerp of red (a=+0.25, b=+0.05) → cyan (a=-0.25,
    // b=-0.05) passes through (0, 0) — gray. Polar lerp keeps the
    // chroma magnitude high throughout, so the midpoint stays saturated.
    //
    // Red in OKLab:    (L≈0.45, a≈+0.25, b≈+0.05) — chroma ≈ 0.255
    // Cyan in OKLab:   (L≈0.79, a≈-0.30, b≈-0.07) — chroma ≈ 0.308
    // Polar midpoint:  chroma ≈ 0.282, hue rotates through magenta/yellow
    // Cartesian mid:   chroma ≈ 0.025 — desaturated, near-gray
    let old = [Color::Rgb {
        r: 220,
        g: 30,
        b: 30,
    }]; // red
    let new = [Color::Rgb {
        r: 30,
        g: 220,
        b: 220,
    }]; // cyan
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();

    // Cell at the wave line (distance = 0, blend = 0.5) using NEW palette.
    let cell_color = Color::Rgb {
        r: 30,
        g: 220,
        b: 220,
    };
    let smoothed = apply_l_smoothing(cell_color, Some(&table), 0, 10);

    let (r, g, b) = match smoothed {
        Color::Rgb { r, g, b } => (r, g, b),
        _ => panic!("expected Color::Rgb"),
    };

    // Saturation proxy: max channel - min channel.
    // Gray has saturation 0. A saturated hue has saturation >= 50.
    let max_c = r.max(g).max(b) as i32;
    let min_c = r.min(g).min(b) as i32;
    let sat = max_c - min_c;

    // Phase 8 polar smoothing should keep the midpoint saturated.
    // Cartesian lerp would produce sat ≈ 0 (gray).
    // Polar lerp produces sat ≥ 50 (typically ~80+).
    assert!(
        sat >= 50,
        "Phase 8 polar chroma smoothing must keep midpoint saturated: \
         ({r},{g},{b}) saturation = {sat}, expected ≥ 50 (gray would be ~0)"
    );
}

#[test]
fn phase8_chroma_smoothing_red_to_green_stays_saturated() {
    // Red → green is the classic Cartesian-midpoint-is-gray case.
    // Phase 8 should keep the midpoint saturated (yellow-ish, since
    // red→green shorter-arc goes through yellow).
    let old = [Color::Rgb {
        r: 220,
        g: 30,
        b: 30,
    }]; // red
    let new = [Color::Rgb {
        r: 30,
        g: 220,
        b: 30,
    }]; // green
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();

    let cell_color = Color::Rgb {
        r: 30,
        g: 220,
        b: 30,
    }; // new (green)
    let smoothed = apply_l_smoothing(cell_color, Some(&table), 0, 10);

    let (r, g, b) = match smoothed {
        Color::Rgb { r, g, b } => (r, g, b),
        _ => panic!("expected Color::Rgb"),
    };

    let max_c = r.max(g).max(b) as i32;
    let min_c = r.min(g).min(b) as i32;
    let sat = max_c - min_c;

    // Polar midpoint of red→green rotates through yellow (high R and G,
    // low B). Cartesian midpoint is gray (R≈G≈B≈127, sat ≈ 0).
    assert!(
        sat >= 40,
        "Phase 8 red→green midpoint must stay saturated (yellow-ish): \
         ({r},{g},{b}) saturation = {sat}, expected ≥ 40"
    );
}

#[test]
fn phase8_grayscale_falls_back_to_cartesian() {
    // When either palette's stop is grayscale (chroma = 0), the hue is
    // undefined. Phase 8 falls back to Cartesian (a, b) lerp. The
    // midpoint is gray, which is the visually correct answer.
    //
    // This test documents that behavior: a red → gray transition produces
    // a desaturated red-ish midpoint (NOT a hue rotation through the
    // chroma ring).
    let old = [Color::Rgb {
        r: 220,
        g: 30,
        b: 30,
    }]; // red
    let new = [Color::Rgb {
        r: 128,
        g: 128,
        b: 128,
    }]; // gray
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();

    let cell_color = Color::Rgb {
        r: 128,
        g: 128,
        b: 128,
    }; // new (gray)
    let smoothed = apply_l_smoothing(cell_color, Some(&table), 0, 10);

    let (r, g, b) = match smoothed {
        Color::Rgb { r, g, b } => (r, g, b),
        _ => panic!("expected Color::Rgb"),
    };

    // The midpoint should be a desaturated red — R slightly higher than
    // G and B, but not the saturated red-yellow-green rotation that polar
    // would produce. Most importantly, it should NOT panic and should
    // produce a finite, in-gamut color. (u8 is always ≤ 255 by type, so
    // no explicit bounds check needed — we just verify the hue leans red.)
    // R should be slightly elevated (blended toward red's a > 0).
    assert!(
        r >= g,
        "R={r} should be >= G={g} (red-leaning midpoint for gray→red)"
    );
    assert!(
        r >= b,
        "R={r} should be >= B={b} (red-leaning midpoint for gray→red)"
    );
}

#[test]
fn phase8_same_chroma_no_hue_rotation() {
    // When both palettes have the same hue (just different L), polar
    // smoothing should be a no-op for chroma — the hue angle doesn't
    // change, only the magnitude (and that's already the same).
    //
    // This test guards against a regression where polar interpolation
    // might introduce spurious hue rotation due to floating-point noise.
    let old = [Color::Rgb {
        r: 100,
        g: 30,
        b: 30,
    }]; // dim red
    let new = [Color::Rgb {
        r: 220,
        g: 70,
        b: 70,
    }]; // bright red
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();

    let cell_color = Color::Rgb {
        r: 220,
        g: 70,
        b: 70,
    }; // new
    let smoothed = apply_l_smoothing(cell_color, Some(&table), 0, 11);

    let (r, g, b) = match smoothed {
        Color::Rgb { r, g, b } => (r, g, b),
        _ => panic!("expected Color::Rgb"),
    };

    // Both palettes are red. Smoothing should keep R dominant by a
    // clear margin. Cartesian lerp would produce R >> G ≈ B.
    // Polar lerp should also produce R >> G ≈ B (no spurious rotation).
    assert!(
        r > g + 50 && r > b + 50,
        "red hue must stay dominant (no spurious rotation): ({r},{g},{b})"
    );
}

#[test]
fn phase8_shortest_arc_picks_shorter_direction() {
    // Two hues 270° apart should rotate through the 90° gap, not the
    // 270° gap. We can verify this by checking that the midpoint hue
    // is closer to BOTH endpoints than the opposite direction would be.
    //
    // We use red (h≈30° in OKLab) and a hue ~270° away (h≈300°). The
    // shorter arc is 90° (going through magenta/red region), the longer
    // is 270° (going through yellow/green/cyan/blue).
    //
    // Constructing precise OKLab hues via sRGB is tricky, so we use
    // approximate sRGB inputs and just verify the midpoint is finite
    // and saturated (i.e. not gray, which would indicate the longer
    // arc passed through the desaturated center).
    let old = [Color::Rgb { r: 255, g: 0, b: 0 }]; // red, h≈29°
    let new = [Color::Rgb {
        r: 128,
        g: 0,
        b: 255,
    }]; // violet, h≈-80°
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();

    let cell_color = Color::Rgb {
        r: 128,
        g: 0,
        b: 255,
    };
    let smoothed = apply_l_smoothing(cell_color, Some(&table), 0, 10);

    let (r, g, b) = match smoothed {
        Color::Rgb { r, g, b } => (r, g, b),
        _ => panic!("expected Color::Rgb"),
    };

    // Shortest-arc from red (h≈29°) to violet (h≈280°) is -109° (going
    // backward through magenta). The midpoint hue should be around
    // (29° + (-109°)*0.5) = -25° → roughly magenta/red-violet.
    // Cartesian midpoint of (255,0,0)→(128,0,255) is (191,0,127) —
    // also magenta-ish, so this isn't a perfect discriminator.
    //
    // The key assertion is: the result is saturated (not gray) and
    // doesn't pass through cyan (the longer arc would).
    let max_c = r.max(g).max(b) as i32;
    let min_c = r.min(g).min(b) as i32;
    let sat = max_c - min_c;
    assert!(
        sat >= 50,
        "shortest-arc midpoint must be saturated: ({r},{g},{b}) sat={sat}"
    );
    // The longer arc would pass through green (high G). Verify G is low.
    assert!(
        g < 60,
        "shortest-arc midpoint should NOT pass through green (longer arc): G={g}"
    );
}

#[test]
fn phase8_full_oklab_equality_skips_smoothing() {
    // Phase 8 widens the skip condition from L-only to full OKLab
    // equality. A stop where (L, a, b) are identical in both palettes
    // should be a no-op even if other stops in the same palette differ.
    //
    // Construct a 2-stop palette where stop 0 is identical and stop 1
    // differs. Verify stop 0 returns the original color unchanged.
    let old = [
        Color::Rgb {
            r: 100,
            g: 50,
            b: 50,
        }, // stop 0: identical red
        Color::Rgb {
            r: 200,
            g: 30,
            b: 30,
        }, // stop 1: dimmer red
    ];
    let new = [
        Color::Rgb {
            r: 100,
            g: 50,
            b: 50,
        }, // stop 0: identical red
        Color::Rgb {
            r: 240,
            g: 20,
            b: 20,
        }, // stop 1: brighter red
    ];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();

    let cell_color = Color::Rgb {
        r: 100,
        g: 50,
        b: 50,
    };
    let smoothed = apply_l_smoothing(cell_color, Some(&table), 0, 10);
    // Stop 0 is identical → no smoothing → original color returned.
    assert_eq!(
        smoothed, cell_color,
        "Phase 8 full-OKLab equality must skip smoothing for identical stops"
    );

    // Stop 1 differs → smoothing applies.
    let cell_color_1 = Color::Rgb {
        r: 240,
        g: 20,
        b: 20,
    };
    let smoothed_1 = apply_l_smoothing(cell_color_1, Some(&table), 1, 10);
    assert_ne!(
        smoothed_1, cell_color_1,
        "Phase 8 must smooth stops where (L, a, b) differs"
    );
}

#[test]
fn phase8_smoothing_deterministic_across_calls() {
    // Polar interpolation with trig functions must be deterministic.
    // Same inputs → same outputs (no floating-point nondeterminism).
    let old = [Color::Rgb {
        r: 220,
        g: 30,
        b: 30,
    }];
    let new = [Color::Rgb {
        r: 30,
        g: 220,
        b: 30,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();

    let color = Color::Rgb {
        r: 30,
        g: 220,
        b: 30,
    };
    let a = apply_l_smoothing(color, Some(&table), 0, 11);
    let b = apply_l_smoothing(color, Some(&table), 0, 11);
    assert_eq!(a, b, "Phase 8 smoothing must be deterministic");
}
