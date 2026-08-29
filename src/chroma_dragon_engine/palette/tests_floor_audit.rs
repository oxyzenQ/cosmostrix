// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Informational audit tests for the Phase 7/7-b palette floor tuning.
//!
//! These tests are diagnostic tools (NOT gates) — they print body-tail
//! gap ratios and continuity sweep data for all themes. Run with:
//!   cargo test --release phase7_print_... -- --nocapture
//!
//! Extracted from `palette/tests_floor.rs` to keep that source file
//! under the 800-LOC cap. Pure code motion — no behavior change.

use super::tests_floor::rgb_sum;
use super::*;
use crate::runtime::ColorScheme;

/// Phase 7 audit (informational, not a gate): print body-tail gap ratios
/// for all 44 themes. Run with:
///   cargo test --release phase7_print_body_tail_gap_audit -- --nocapture
///
/// Identifies themes where the body-tail brightness jump is large enough
/// to cause a horizontal-line illusion at high rain speed. Such gaps
/// occur when trail stops are very dark (post-Phase-7 floor) and the
/// next body stop is much brighter — the eye perceives two distinct
/// brightness bands instead of a continuous gradient.
///
/// This test NEVER fails — it's a diagnostic tool for tuning
/// PALETTE_FLOOR_RATIO. If too many themes show HIGH risk, raise the
/// ratio (e.g. 0.15 → 0.20). If trails are too bright (washout),
/// lower it.
#[test]
fn phase7_print_body_tail_gap_audit() {
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

    struct Row {
        name: String,
        n: usize,
        trail_max: u16,
        body_min: u16,
        head_max: u16,
        max_adj_gap: f32,
        risk: &'static str,
    }
    let mut results: Vec<Row> = Vec::new();

    for &scheme in &schemes {
        let p = build_palette(scheme, ColorMode::TrueColor, true);
        if p.colors.is_empty() {
            continue;
        }
        let sums: Vec<u16> = p.colors.iter().map(|&c| rgb_sum(c)).collect();
        let n = sums.len();
        if n < 4 {
            continue;
        }
        // Trail = first 2 stops (typically lowest sum)
        // Body = middle stops
        // Head = last 2 stops (typically highest sum)
        let trail_max = *sums[..2.min(n)].iter().max().unwrap_or(&0);
        let body: &[u16] = &sums[2.min(n)..(n - 2).max(2)];
        let body_min = *body.iter().min().unwrap_or(&0);
        let head_max = *sums[n - 2..].iter().max().unwrap_or(&0);
        // Max adjacent gap (what continuity actually enforces).
        // Phase 7-b targets BODY_TAIL_MAX_GAP_RATIO=2.0; integer rounding
        // may push actual gap to 2.01-2.05. Threshold of 2.1x = "at target".
        let max_adj_gap: f32 = (0..n - 1)
            .map(|i| {
                let cur = sums[i];
                let next = sums[i + 1];
                if cur == 0 {
                    0.0
                } else {
                    next as f32 / cur as f32
                }
            })
            .fold(0.0_f32, f32::max);
        // HIGH = structural issue (cap-limited, body too bright for trail to catch up)
        // MED = above continuity target but within rounding slack
        // low = at or below continuity target
        let risk = if max_adj_gap > 2.1 {
            "HIGH"
        } else if max_adj_gap > 2.05 {
            "MED"
        } else {
            "low"
        };
        results.push(Row {
            name: format!("{:?}", scheme),
            n,
            trail_max,
            body_min,
            head_max,
            max_adj_gap,
            risk,
        });
    }

    results.sort_by(|a, b| b.max_adj_gap.partial_cmp(&a.max_adj_gap).unwrap());

    eprintln!();
    eprintln!("=== Phase 7-b body-tail gap audit (44 themes) ===");
    eprintln!(
        "{:<14} {:>3} {:>10} {:>9} {:>9} {:>10} {:>5}",
        "THEME", "N", "TRAIL_MAX", "BODY_MIN", "HEAD_MAX", "MAX_ADJ", "RISK"
    );
    eprintln!("{}", "-".repeat(64));
    for r in &results {
        eprintln!(
            "{:<14} {:>3} {:>10} {:>9} {:>9} {:>9.2}x {:>5}",
            r.name, r.n, r.trail_max, r.body_min, r.head_max, r.max_adj_gap, r.risk
        );
    }
    eprintln!();
    let high: Vec<_> = results.iter().filter(|r| r.risk == "HIGH").collect();
    let med: Vec<_> = results.iter().filter(|r| r.risk == "MED").collect();
    eprintln!(
        "HIGH risk (max_adj_gap > 2.1x): {} themes — structural cap-limited",
        high.len()
    );
    for r in &high {
        eprintln!(
            "  - {}: trail_max={}, body_min={}, max_adj_gap={:.2}x (cap=180 limits trail boost)",
            r.name, r.trail_max, r.body_min, r.max_adj_gap
        );
    }
    eprintln!(
        "MED risk (2.05x-2.1x): {} themes — within rounding slack of continuity target",
        med.len()
    );
    for r in &med {
        eprintln!("  - {}: max_adj_gap={:.2}x", r.name, r.max_adj_gap);
    }
    eprintln!();
    eprintln!(
        "Continuity target: BODY_TAIL_MAX_GAP_RATIO = {:.2}x",
        super::super::tuning::BODY_TAIL_MAX_GAP_RATIO
    );
    eprintln!("Tuning guidance:");
    eprintln!("  - HIGH count > 5: raise GLOBAL_MAX_FLOOR (e.g. 180 → 220) — at cost of some v17-style washout");
    eprintln!("  - All low + trails look washed out: lower PALETTE_FLOOR_RATIO or BODY_TAIL_MAX_GAP_RATIO");
}

/// Phase 7 ratio sweep audit (informational). For each of the 44 themes,
/// prints the trail brightness + max adjacent gap that would result from
/// each candidate `PALETTE_FLOOR_RATIO` in [0.15, 0.20, 0.25, 0.30].
///
/// Run with:
///   cargo test --release phase7_print_ratio_sweep_audit -- --nocapture
///
/// This test NEVER fails — it's a diagnostic for picking the next value
/// of `PALETTE_FLOOR_RATIO`. The current production value is 0.15 (the
/// first column), and the user-reported issue is "tail too dark". The
/// sweep shows the trade-off: higher ratio → brighter trails, but more
/// themes hit the GLOBAL_MAX_FLOOR cap (potential v17-style washout).
///
/// Columns:
///   THEME       — ColorScheme name
///   HEAD_SUM    — brightest stop sum (palette ceiling, constant across ratios)
///   ORIG_TRAIL  — last stop sum pre-floor (the "intended" trail brightness)
///   For each ratio r ∈ {0.15, 0.20, 0.25, 0.30}:
///     TRAIL@r     — final trail (last stop) sum after floor + continuity
///     GAP@r       — max adjacent gap (next/cur) post-continuity; lower = smoother
///     CAP_HIT@r   — "*" if GLOBAL_MAX_FLOOR=180 was the binding constraint
///                   (i.e. derived floor would have been > 180)
#[test]
fn phase7_print_ratio_sweep_audit() {
    use crate::chroma_dragon_engine::catalog::{ThemeColors, THEMES};

    let ratios: [f32; 4] = [0.15, 0.20, 0.25, 0.30];
    let abs_min = super::super::tuning::ABSOLUTE_MIN_FLOOR;
    let global_max = super::super::tuning::GLOBAL_MAX_FLOOR;
    let max_gap_prod = super::super::tuning::BODY_TAIL_MAX_GAP_RATIO;

    eprintln!();
    eprintln!("=== Phase 7 PALETTE_FLOOR_RATIO sweep audit (44 themes) ===");
    eprintln!(
        "Constants: ABSOLUTE_MIN_FLOOR={}, GLOBAL_MAX_FLOOR={}, BODY_TAIL_MAX_GAP_RATIO={:.2}",
        abs_min, global_max, max_gap_prod
    );
    eprintln!();
    eprintln!(
        "{:<14} {:>8} {:>10}  {:>10} {:>6} {:>1}  {:>10} {:>6} {:>1}  {:>10} {:>6} {:>1}  {:>10} {:>6} {:>1}",
        "THEME", "HEAD_SUM", "ORIG_TRL",
        "TRL@.15", "GAP", "C",
        "TRL@.20", "GAP", "C",
        "TRL@.25", "GAP", "C",
        "TRL@.30", "GAP", "C",
    );

    let mut n_capped_per_ratio = [0usize; 4];
    let mut n_worst_gap_above_3_per_ratio = [0usize; 4];

    for theme in THEMES {
        // Extract the raw RGB stops (pre-floor) by replaying the same
        // gradient_from_stops call that colors_from_stops makes.
        let raw_stops: Vec<(u8, u8, u8)> = match theme.def {
            ThemeColors::Stops { stops, steps }
            | ThemeColors::StopsWithC16 { stops, steps, .. } => gradient_from_stops(stops, steps),
            ThemeColors::RgbWithC16 { rgb, .. } => rgb.to_vec(),
        };
        if raw_stops.len() < 4 {
            continue;
        }
        let head_sum: u16 = raw_stops
            .iter()
            .map(|&(r, g, b)| r as u16 + g as u16 + b as u16)
            .max()
            .unwrap_or(0);
        let orig_trail_sum: u16 = {
            let (r, g, b) = raw_stops[0];
            r as u16 + g as u16 + b as u16
        };

        // For each ratio, clone the raw stops and apply floor + continuity.
        let mut cells: Vec<String> = Vec::with_capacity(ratios.len());
        for (idx, &ratio) in ratios.iter().enumerate() {
            let mut rgb = raw_stops.clone();
            apply_palette_relative_floor_with(rgb.as_mut_slice(), ratio, abs_min, global_max);
            apply_body_tail_continuity_with(rgb.as_mut_slice(), max_gap_prod);

            // Trail = first stop (after sorting by palette order, the dim end).
            let trail_sum = {
                let (r, g, b) = rgb[0];
                r as u16 + g as u16 + b as u16
            };

            // Max adjacent gap (post-continuity).
            let n = rgb.len();
            let max_adj_gap: f32 = (0..n - 1)
                .map(|i| {
                    let cur = rgb[i].0 as u16 + rgb[i].1 as u16 + rgb[i].2 as u16;
                    let next = rgb[i + 1].0 as u16 + rgb[i + 1].1 as u16 + rgb[i + 1].2 as u16;
                    if cur == 0 {
                        0.0
                    } else {
                        next as f32 / cur as f32
                    }
                })
                .fold(0.0_f32, f32::max);

            // Did the basic floor hit the global_max cap?
            let derived = (head_sum as f32 * ratio) as u16;
            let cap_hit = derived > global_max;
            if cap_hit {
                n_capped_per_ratio[idx] += 1;
            }
            if max_adj_gap > 3.0 {
                n_worst_gap_above_3_per_ratio[idx] += 1;
            }

            cells.push(format!(
                "{:>10} {:>6.2} {:>1}",
                trail_sum,
                max_adj_gap,
                if cap_hit { "*" } else { " " }
            ));
        }

        eprintln!(
            "{:<14} {:>8} {:>10}  {}  {}  {}  {}",
            format!("{:?}", theme.scheme),
            head_sum,
            orig_trail_sum,
            cells[0],
            cells[1],
            cells[2],
            cells[3]
        );
    }

    eprintln!();
    eprintln!("Legend: TRAIL = sum of dimmest stop after floor + continuity");
    eprintln!("        GAP   = max(next/cur) across adjacent stops — lower = smoother");
    eprintln!(
        "        C=*   = GLOBAL_MAX_FLOOR={} was the binding cap (derived floor > {})",
        global_max, global_max
    );
    eprintln!("        (would have produced a brighter floor if uncapped)");
    eprintln!();
    eprintln!("Summary per ratio:");
    for (idx, &r) in ratios.iter().enumerate() {
        eprintln!(
            "  ratio={:.2}: {} themes hit GLOBAL_MAX_FLOOR cap, {} themes had max_adj_gap > 3.0x",
            r, n_capped_per_ratio[idx], n_worst_gap_above_3_per_ratio[idx]
        );
    }
    eprintln!();
    eprintln!("Tuning guidance:");
    eprintln!("  - Pick the lowest ratio where GAP>3.0x count drops to ~0 (kills horizontal-line illusion)");
    eprintln!("  - Watch cap_hit count: if it grows significantly, dark themes may wash out (v17 regression)");
    eprintln!(
        "  - Production current: PALETTE_FLOOR_RATIO = {:.2}",
        super::super::tuning::PALETTE_FLOOR_RATIO
    );
}

/// Phase 7-b gap ratio sweep audit (informational). For each of the 43
/// themes, prints the trail brightness + max adjacent gap that would result
/// from each candidate `BODY_TAIL_MAX_GAP_RATIO` in [2.5, 2.0, 1.8, 1.5].
///
/// Run with:
///   cargo test --release phase7b_print_gap_ratio_sweep_audit -- --nocapture
///
/// This test NEVER fails — it's a diagnostic for picking the next value
/// of `BODY_TAIL_MAX_GAP_RATIO`. The current production value is 2.0
/// (lowered from 2.5 in Phase 7-d to kill the user-reported
/// "horizontal-line illusion" at speed 100, where the eye perceived a
/// hard brightness step at the trail→body boundary). A 2.5x step was
/// still perceptible at high rain speed; the 2.0x step is 20% tighter
/// and below the perceptual threshold.
///
/// Columns:
///   THEME       — ColorScheme name
///   HEAD_SUM    — brightest stop sum (palette ceiling)
///   BODY_MIN    — dimmest non-trail stop sum (the body anchor continuity targets)
///   For each gap g ∈ {2.5, 2.0, 1.8, 1.5}:
///     TRAIL@g    — final trail (last stop) sum after floor (ratio=0.20) + continuity
///     MAX_GAP@g  — actual max adjacent gap post-continuity (should be ≤ g + rounding)
///     SAT@g      — "*" if any channel hit 255 (continuity capped by u8 max)
#[test]
fn phase7b_print_gap_ratio_sweep_audit() {
    use crate::chroma_dragon_engine::catalog::{ThemeColors, THEMES};

    let gaps: [f32; 4] = [2.5, 2.0, 1.8, 1.5];
    let ratio = super::super::tuning::PALETTE_FLOOR_RATIO;
    let abs_min = super::super::tuning::ABSOLUTE_MIN_FLOOR;
    let global_max = super::super::tuning::GLOBAL_MAX_FLOOR;

    eprintln!();
    eprintln!("=== Phase 7-b BODY_TAIL_MAX_GAP_RATIO sweep audit (44 themes) ===");
    eprintln!(
        "Constants: PALETTE_FLOOR_RATIO={:.2}, ABSOLUTE_MIN_FLOOR={}, GLOBAL_MAX_FLOOR={}",
        ratio, abs_min, global_max
    );
    eprintln!();
    eprintln!(
        "{:<14} {:>8} {:>8}  {:>10} {:>7} {:>1}  {:>10} {:>7} {:>1}  {:>10} {:>7} {:>1}  {:>10} {:>7} {:>1}",
        "THEME", "HEAD_SUM", "BODY_MIN",
        "TRL@2.5", "GAP", "S",
        "TRL@2.0", "GAP", "S",
        "TRL@1.8", "GAP", "S",
        "TRL@1.5", "GAP", "S",
    );

    let mut n_saturated_per_gap = [0usize; 4];
    let mut n_trail_above_head_per_gap = [0usize; 4];

    for theme in THEMES {
        let raw_stops: Vec<(u8, u8, u8)> = match theme.def {
            ThemeColors::Stops { stops, steps }
            | ThemeColors::StopsWithC16 { stops, steps, .. } => gradient_from_stops(stops, steps),
            ThemeColors::RgbWithC16 { rgb, .. } => rgb.to_vec(),
        };
        if raw_stops.len() < 4 {
            continue;
        }
        let head_sum: u16 = raw_stops
            .iter()
            .map(|&(r, g, b)| r as u16 + g as u16 + b as u16)
            .max()
            .unwrap_or(0);
        // Body = all stops except first 2 (trail) and last 2 (head).
        let n = raw_stops.len();
        let body_min: u16 = raw_stops
            .iter()
            .skip(2)
            .take(n.saturating_sub(4))
            .map(|&(r, g, b)| r as u16 + g as u16 + b as u16)
            .min()
            .unwrap_or(0);

        let mut cells: Vec<String> = Vec::with_capacity(gaps.len());
        for (idx, &gap_target) in gaps.iter().enumerate() {
            let mut rgb = raw_stops.clone();
            apply_palette_relative_floor_with(rgb.as_mut_slice(), ratio, abs_min, global_max);
            apply_body_tail_continuity_with(rgb.as_mut_slice(), gap_target);

            let trail_sum = {
                let (r, g, b) = rgb[0];
                r as u16 + g as u16 + b as u16
            };
            let max_adj_gap: f32 = (0..n - 1)
                .map(|i| {
                    let cur = rgb[i].0 as u16 + rgb[i].1 as u16 + rgb[i].2 as u16;
                    let next = rgb[i + 1].0 as u16 + rgb[i + 1].1 as u16 + rgb[i + 1].2 as u16;
                    if cur == 0 {
                        0.0
                    } else {
                        next as f32 / cur as f32
                    }
                })
                .fold(0.0_f32, f32::max);

            let saturated = rgb
                .iter()
                .any(|&(r, g, b)| r == 255 || g == 255 || b == 255);
            if saturated {
                n_saturated_per_gap[idx] += 1;
            }
            if trail_sum >= head_sum {
                n_trail_above_head_per_gap[idx] += 1;
            }

            cells.push(format!(
                "{:>10} {:>7.2} {:>1}",
                trail_sum,
                max_adj_gap,
                if saturated { "*" } else { " " }
            ));
        }

        eprintln!(
            "{:<14} {:>8} {:>8}  {}  {}  {}  {}",
            format!("{:?}", theme.scheme),
            head_sum,
            body_min,
            cells[0],
            cells[1],
            cells[2],
            cells[3]
        );
    }

    eprintln!();
    eprintln!("Legend: TRL = dimmest stop sum after floor + continuity");
    eprintln!("        GAP = max(next/cur) post-continuity — lower = smoother");
    eprintln!("        S=*  = at least one channel saturated at 255 (continuity capped by u8)");
    eprintln!();
    eprintln!("Summary per gap target:");
    for (idx, &g) in gaps.iter().enumerate() {
        eprintln!(
            "  gap={:.1}: {} themes had a channel saturate at 255, {} themes had trail ≥ head (hierarchy broken)",
            g, n_saturated_per_gap[idx], n_trail_above_head_per_gap[idx]
        );
    }
    eprintln!();
    eprintln!("Tuning guidance:");
    eprintln!("  - Lower gap = smoother body-tail transition (kills horizontal-line illusion)");
    eprintln!("  - Watch saturation: if many themes hit 255, continuity is being over-boosted");
    eprintln!("  - Watch trail≥head: if non-zero, hierarchy is broken (regression)");
    eprintln!(
        "  - Production current: BODY_TAIL_MAX_GAP_RATIO = {:.2}",
        super::super::tuning::BODY_TAIL_MAX_GAP_RATIO
    );
}
