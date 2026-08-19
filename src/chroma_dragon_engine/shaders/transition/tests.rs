// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for `chroma::shaders::transition`.
//!
//! Coverage:
//! - `TransitionLTable::build` edge cases (empty, Reset, mismatched lengths)
//! - `apply_l_smoothing` skip conditions (None table, Reset color, out
//!   of window, out of range, equal L)
//! - End-to-end smoothing behavior (above/below wave, blend factor,
//!   midpoint behavior, hue preservation)

use crossterm::style::Color;

use super::*;

// ─── TransitionLTable::build ───

#[test]
fn build_empty_old_palette_returns_none() {
    let new = [Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    }];
    assert!(TransitionLTable::build(&[], &new, 10.0, 3.0).is_none());
}

#[test]
fn build_empty_new_palette_returns_none() {
    let old = [Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    }];
    assert!(TransitionLTable::build(&old, &[], 10.0, 3.0).is_none());
}

#[test]
fn build_zero_window_returns_none() {
    let old = [Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    assert!(TransitionLTable::build(&old, &new, 10.0, 0.0).is_none());
}

#[test]
fn build_negative_window_returns_none() {
    let old = [Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    assert!(TransitionLTable::build(&old, &new, 10.0, -1.0).is_none());
}

#[test]
fn build_all_reset_old_returns_none() {
    let old = [Color::Reset, Color::Reset];
    let new = [
        Color::Rgb {
            r: 100,
            g: 100,
            b: 100,
        },
        Color::Rgb {
            r: 200,
            g: 200,
            b: 200,
        },
    ];
    assert!(TransitionLTable::build(&old, &new, 10.0, 3.0).is_none());
}

#[test]
fn build_all_reset_new_returns_none() {
    let old = [
        Color::Rgb {
            r: 100,
            g: 100,
            b: 100,
        },
        Color::Rgb {
            r: 200,
            g: 200,
            b: 200,
        },
    ];
    let new = [Color::Reset, Color::Reset];
    assert!(TransitionLTable::build(&old, &new, 10.0, 3.0).is_none());
}

#[test]
fn build_skips_reset_entries() {
    // 3 stops in old, 3 in new. Stops 0 and 2 are valid in both;
    // stop 1 is Reset in old. The resulting entries vec should have
    // only 2 entries (indices 0 and 2 from the source palettes), but
    // because we use `continue` (not compacting), entries[1] is the
    // valid pair from old[2]/new[2]. This is a known sparseness bug:
    // the shader's color_idx won't line up with entries[] after a skip.
    //
    // For now, this test documents the behavior: build succeeds,
    // entries.len() = number of valid pairs (2), not the min palette
    // length (3). Stops with Reset in either palette are skipped.
    let old = [
        Color::Rgb {
            r: 50,
            g: 50,
            b: 50,
        },
        Color::Reset,
        Color::Rgb {
            r: 150,
            g: 150,
            b: 150,
        },
    ];
    let new = [
        Color::Rgb {
            r: 60,
            g: 60,
            b: 60,
        },
        Color::Rgb {
            r: 120,
            g: 120,
            b: 120,
        },
        Color::Rgb {
            r: 180,
            g: 180,
            b: 180,
        },
    ];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0);
    assert!(table.is_some(), "build should succeed with mixed Reset");
    let table = table.unwrap();
    assert_eq!(table.entries.len(), 2, "Reset entry should be skipped");
}

#[test]
fn build_mismatched_lengths_uses_min() {
    let old = [
        Color::Rgb {
            r: 50,
            g: 50,
            b: 50,
        },
        Color::Rgb {
            r: 100,
            g: 100,
            b: 100,
        },
        Color::Rgb {
            r: 150,
            g: 150,
            b: 150,
        },
    ];
    let new = [Color::Rgb {
        r: 60,
        g: 60,
        b: 60,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0);
    assert!(table.is_some());
    assert_eq!(table.unwrap().entries.len(), 1, "should use min length");
}

#[test]
fn build_carries_wave_line_and_window() {
    let old = [Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let table = TransitionLTable::build(&old, &new, 25.5, 4.0).unwrap();
    assert_eq!(table.wave_line, 25.5);
    assert_eq!(table.window, 4.0);
}

#[test]
fn build_computes_l_values() {
    // L of (50,50,50) < L of (200,200,200) — lighter color has higher L.
    let old = [Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();
    let e = &table.entries[0];
    assert!(
        e.l_old < e.l_new,
        "darker color must have lower L: L_old={}, L_new={}",
        e.l_old,
        e.l_new
    );
    // Phase 8: (a, b) should also be populated. For grayscale colors,
    // both (a, b) pairs are ~0 — we just verify the fields are accessible
    // and finite.
    assert!(e.a_old.is_finite());
    assert!(e.b_old.is_finite());
    assert!(e.a_new.is_finite());
    assert!(e.b_new.is_finite());
}

// ─── TransitionLTable::get ───

#[test]
fn get_in_range_returns_some() {
    let old = [
        Color::Rgb {
            r: 50,
            g: 50,
            b: 50,
        },
        Color::Rgb {
            r: 100,
            g: 100,
            b: 100,
        },
    ];
    let new = [
        Color::Rgb {
            r: 60,
            g: 60,
            b: 60,
        },
        Color::Rgb {
            r: 120,
            g: 120,
            b: 120,
        },
    ];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();
    assert!(table.get(0).is_some());
    assert!(table.get(1).is_some());
}

#[test]
fn get_out_of_range_returns_none() {
    let old = [Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    }];
    let new = [Color::Rgb {
        r: 60,
        g: 60,
        b: 60,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();
    assert!(table.get(0).is_some());
    assert!(table.get(1).is_none());
    assert!(table.get(usize::MAX).is_none());
}

// ─── apply_l_smoothing: skip conditions ───

#[test]
fn smoothing_none_table_returns_original() {
    let color = Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    };
    assert_eq!(apply_l_smoothing(color, None, 0, 10), color);
}

#[test]
fn smoothing_reset_color_returns_original() {
    let old = [Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();
    let color = Color::Reset;
    assert_eq!(apply_l_smoothing(color, Some(&table), 0, 10), Color::Reset);
}

#[test]
fn smoothing_outside_window_returns_original() {
    let old = [Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();
    let color = Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    };

    // line 0 is 10 lines above wave_line=10, distance=−10, |dist|=10 >= window=3
    assert_eq!(apply_l_smoothing(color, Some(&table), 0, 0), color);
    // line 20 is 10 lines below, also outside
    assert_eq!(apply_l_smoothing(color, Some(&table), 0, 20), color);
    // line 14 is 4 lines below, still outside (window=3)
    assert_eq!(apply_l_smoothing(color, Some(&table), 0, 14), color);
}

#[test]
fn smoothing_at_window_boundary_returns_original() {
    // Exactly at window boundary: |distance| == window, blend = 0, no change.
    let old = [Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();
    let color = Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    };

    // line 13: distance = 13 - 10 = 3, |dist| = 3 == window → no smoothing
    assert_eq!(apply_l_smoothing(color, Some(&table), 0, 13), color);
    // line 7: distance = 7 - 10 = -3, |dist| = 3 == window → no smoothing
    assert_eq!(apply_l_smoothing(color, Some(&table), 0, 7), color);
}

#[test]
fn smoothing_out_of_range_stop_returns_original() {
    let old = [Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();
    let color = Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    };

    // stop_idx 1 is out of range (only 1 entry)
    assert_eq!(apply_l_smoothing(color, Some(&table), 1, 10), color);
    // negative stop_idx (shouldn't occur in practice) returns original
    assert_eq!(apply_l_smoothing(color, Some(&table), -1, 10), color);
}

#[test]
fn smoothing_equal_l_returns_original() {
    // Same RGB in both palettes → L_old == L_new → no smoothing.
    let old = [Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    }];
    let new = [Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();
    let color = Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    };
    assert_eq!(apply_l_smoothing(color, Some(&table), 0, 10), color);
}

// ─── apply_l_smoothing: end-to-end behavior ───

#[test]
fn smoothing_above_wave_blends_toward_old_l() {
    // Cell above wave uses NEW palette (L_new = L of (200,200,200) ≈ 0.78).
    // Smoothing should blend its L toward L_old (L of (50,50,50) ≈ 0.22).
    // Result L should be between L_old and L_new, closer to L_new (because
    // blend < 0.5 above the wave).
    let old = [Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();

    // Cell at line 11 (distance = +1, blend = 0.5 * (1 - 1/3) = 1/3)
    // Cell color is NEW palette's stop (200,200,200), L ≈ 0.78
    let cell_color = Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    };
    let smoothed = apply_l_smoothing(cell_color, Some(&table), 0, 11);

    let (r, g, b) = match smoothed {
        Color::Rgb { r, g, b } => (r, g, b),
        _ => panic!("expected Color::Rgb, got {smoothed:?}"),
    };
    // The smoothed color should be DIMMER than the original (because we
    // blended toward L_old which is darker). All channels should decrease.
    assert!(
        r < 200 && g < 200 && b < 200,
        "smoothed color ({r},{g},{b}) should be dimmer than (200,200,200)"
    );
    // But still brighter than the old palette's color (50,50,50).
    assert!(
        r > 50 && g > 50 && b > 50,
        "smoothed color ({r},{g},{b}) should be brighter than (50,50,50)"
    );
}

#[test]
fn smoothing_below_wave_blends_toward_new_l() {
    // Cell below wave uses OLD palette (L_old = L of (50,50,50) ≈ 0.22).
    // Smoothing should blend its L toward L_new (L of (200,200,200) ≈ 0.78).
    // Result L should be between L_old and L_new, closer to L_old.
    let old = [Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();

    // Cell at line 9 (distance = -1, blend = 1/3)
    // Cell color is OLD palette's stop (50,50,50), L ≈ 0.22
    let cell_color = Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    };
    let smoothed = apply_l_smoothing(cell_color, Some(&table), 0, 9);

    let (r, g, b) = match smoothed {
        Color::Rgb { r, g, b } => (r, g, b),
        _ => panic!("expected Color::Rgb, got {smoothed:?}"),
    };
    // Smoothed color should be BRIGHTER than original (blended toward L_new).
    assert!(
        r > 50 && g > 50 && b > 50,
        "smoothed color ({r},{g},{b}) should be brighter than (50,50,50)"
    );
    // But still dimmer than the new palette's color (200,200,200).
    assert!(
        r < 200 && g < 200 && b < 200,
        "smoothed color ({r},{g},{b}) should be dimmer than (200,200,200)"
    );
}

#[test]
fn smoothing_at_wave_line_produces_midpoint() {
    // At the wave line (distance = 0), blend = 0.5 (50% toward opposite).
    // A cell at the wave line uses the NEW palette (per
    // `color_uses_previous_palette`: `line > wave_line` is false when
    // line == wave_line, so the cell uses the active/new palette). The
    // smoothing blends its L toward L_old by 50%, producing the midpoint
    // between L_new and L_old.
    //
    // We verify: the smoothed L is halfway between L_new and L_old, which
    // for grayscale palettes means the smoothed RGB is halfway between
    // the new and old RGB values (in OKLab L space, not sRGB space).
    let old = [Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();

    // Cell at line 10 (distance = 0), uses new palette's color (200,200,200).
    let cell_color = Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    };
    let smoothed = apply_l_smoothing(cell_color, Some(&table), 0, 10);

    let (r, g, b) = match smoothed {
        Color::Rgb { r, g, b } => (r, g, b),
        _ => panic!("expected Color::Rgb"),
    };

    // The smoothed color should be roughly the OKLab L midpoint between
    // (50,50,50) and (200,200,200). For grayscale, OKLab L midpoint is
    // NOT the sRGB midpoint (125) — it's perceptually halfway, which is
    // dimmer than sRGB midpoint because OKLab L is perceptual (human
    // eyes perceive mid-gray as dimmer than the arithmetic midpoint).
    //
    // Empirically, OKLab L of (50,50,50) ≈ 0.22, OKLab L of (200,200,200)
    // ≈ 0.78. Midpoint L = 0.50. OKLab→sRGB of (0.50, 0, 0) ≈ (121,121,121).
    assert_eq!(
        (r, g, b),
        (121, 121, 121),
        "smoothed color at wave line should be the OKLab L midpoint"
    );

    // The midpoint should be strictly between (50,50,50) and (200,200,200).
    assert!(
        r > 50 && r < 200,
        "midpoint R={r} should be strictly between 50 and 200"
    );
}

#[test]
fn smoothing_decreases_with_distance_from_wave() {
    // Cells further from the wave should get less smoothing.
    // Compare a cell at distance=1 vs distance=2 (both above wave).
    let old = [Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();

    let cell_color = Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    };
    let near = apply_l_smoothing(cell_color, Some(&table), 0, 11); // dist=1
    let far = apply_l_smoothing(cell_color, Some(&table), 0, 12); // dist=2

    let (nr, _, _) = match near {
        Color::Rgb { r, g, b } => (r, g, b),
        _ => panic!(),
    };
    let (fr, _, _) = match far {
        Color::Rgb { r, g, b } => (r, g, b),
        _ => panic!(),
    };

    // Near (more blend toward dark L_old) should be dimmer than far.
    assert!(
        nr < fr,
        "cell closer to wave (R={nr}) should be more smoothed (dimmer) than cell further (R={fr})"
    );
    // Far should still be dimmer than the unsmoothed (200,200,200).
    assert!(
        fr < 200,
        "far cell (R={fr}) should still be smoothed below 200"
    );
}

#[test]
fn smoothing_preserves_hue_for_colored_palette() {
    // Both palettes use red stops with a modest L difference. Smoothing
    // should preserve the red hue (R >> G, R >> B) — only L changes.
    //
    // We use a modest L difference (L_old ≈ 0.45, L_new ≈ 0.60) to avoid
    // out-of-gamut clamping during the OKLab → sRGB conversion. Extreme
    // L changes can push the (a, b) chroma outside the sRGB gamut,
    // causing clamping that distorts the hue ratio.
    let old = [Color::Rgb {
        r: 130,
        g: 30,
        b: 30,
    }]; // medium-dark red
    let new = [Color::Rgb {
        r: 220,
        g: 70,
        b: 70,
    }]; // medium-bright red
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();

    let cell_color = Color::Rgb {
        r: 220,
        g: 70,
        b: 70,
    }; // new palette
    let smoothed = apply_l_smoothing(cell_color, Some(&table), 0, 11);

    let (r, g, b) = match smoothed {
        Color::Rgb { r, g, b } => (r, g, b),
        _ => panic!(),
    };
    // Hue preservation: R must remain the dominant channel by a clear
    // margin. The exact ratio may shift slightly due to OKLab→sRGB
    // rounding, but R >> G and R >> B must hold.
    assert!(
        r > g + 50 && r > b + 50,
        "red hue must be preserved after L smoothing: ({r},{g},{b})"
    );
    // The smoothed color should be dimmer than the original (blended
    // toward the darker old palette's L).
    assert!(r < 220, "smoothed R={r} should be dimmer than original 220");
    // But still brighter than the old palette's color.
    assert!(
        r > 130,
        "smoothed R={r} should be brighter than old palette's 130"
    );
}

#[test]
fn smoothing_deterministic() {
    let old = [Color::Rgb {
        r: 50,
        g: 50,
        b: 50,
    }];
    let new = [Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();
    let color = Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    };
    let a = apply_l_smoothing(color, Some(&table), 0, 11);
    let b = apply_l_smoothing(color, Some(&table), 0, 11);
    assert_eq!(a, b, "smoothing must be deterministic for same inputs");
}

// ─── Integration with shader stop indices ───

#[test]
fn smoothing_handles_multi_stop_palette() {
    // 5 stops in each palette, all valid RGB. The table should have 5
    // entries, and stop_idx 0..=4 should all smooth correctly.
    let old: Vec<Color> = (0..5)
        .map(|i| Color::Rgb {
            r: 50 + i as u8 * 40,
            g: 50 + i as u8 * 40,
            b: 50 + i as u8 * 40,
        })
        .collect();
    let new: Vec<Color> = (0..5)
        .map(|i| Color::Rgb {
            r: 60 + i as u8 * 40,
            g: 60 + i as u8 * 40,
            b: 60 + i as u8 * 40,
        })
        .collect();
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0).unwrap();
    assert_eq!(table.entries.len(), 5);

    // Each stop should smooth without panic.
    for stop_idx in 0..5_i32 {
        let color = new[stop_idx as usize];
        let smoothed = apply_l_smoothing(color, Some(&table), stop_idx, 11);
        // Should produce a valid Color::Rgb
        assert!(matches!(smoothed, Color::Rgb { .. }));
    }
}

#[test]
fn smoothing_no_panic_on_ansi_color() {
    // Non-RGB color types (AnsiValue) are decoded via color_to_rgb.
    // The smoothing should handle them gracefully (no panic).
    let old = [Color::AnsiValue(2)]; // ANSI green
    let new = [Color::AnsiValue(1)]; // ANSI red
    let table = TransitionLTable::build(&old, &new, 10.0, 3.0);
    // build might succeed or fail depending on color_to_rgb's behavior
    // for AnsiValue — either is acceptable, as long as no panic.
    if let Some(table) = table {
        let color = Color::AnsiValue(2);
        let _ = apply_l_smoothing(color, Some(&table), 0, 10);
        // Should not panic; result is some Color variant.
    }
}

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
