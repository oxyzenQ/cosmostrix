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
    let (l_old, l_new) = table.entries[0];
    assert!(
        l_old < l_new,
        "darker color must have lower L: L_old={l_old}, L_new={l_new}"
    );
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
