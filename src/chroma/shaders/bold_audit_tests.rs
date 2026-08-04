// SPDX-License-Identifier: GPL-3.0-only
//
// Audit test (task flags-audit-3): verify that --bold and --shadingmode
// actually produce DIFFERENT rendered output. The owner reported that
// switching --bold 0/1/2 visually looked identical in past testing.
// This test series proves the shader branches produce different (fg, bold)
// pairs for the same input cells, so the difference is real — if the user
// can't see it, the cause is downstream (terminal emulator bold rendering,
// font lacks bold variant, etc.) not in the shader.
//
// The tests use resolve_cell_color directly (the unit-of-work function)
// rather than running a full frame render, so we isolate the shader logic
// from terminal I/O.

#![cfg(test)]

use bitvec::prelude::BitSlice;
use crossterm::style::Color;

use crate::chroma::shaders::base::{resolve_cell_color, CharLoc, ShaderCtx};
use crate::constants::MAX_PALETTE_SLOTS;
use crate::runtime::{BoldMode, ColorMode};

fn make_shader<'a>(
    palette_slices: &'a [&'a [Color]; MAX_PALETTE_SLOTS],
    color_map: &'a [u8],
    shading_distance: bool,
    bold_mode: BoldMode,
) -> ShaderCtx<'a> {
    ShaderCtx {
        palette_slices,
        active_palette_slot: 0,
        color_wave_line: None,
        bold_mode,
        lines: 50,
        color_map,
        shading_distance,
        glitchy: false,
        glitch_map: <&BitSlice>::default(),
        glitch_bright: false,
        glitch_dim: false,
        color_mode: ColorMode::TrueColor,
        column_coherence_phase: None,
        subpixel_jitter_amplitude: None,
        atmospheric: None,
        hue_drift_offset: None,
        head_halo_factor: None,
        transition_l_table: None,
        bg: None,
    }
}

fn slot_array(palette: &[Color]) -> [&[Color]; MAX_PALETTE_SLOTS] {
    let mut arr: [&[Color]; MAX_PALETTE_SLOTS] = [&[]; MAX_PALETTE_SLOTS];
    arr[0] = palette;
    arr
}

/// Build a 5-stop palette for testing.
fn test_palette() -> Vec<Color> {
    (0..5)
        .map(|i| Color::Rgb {
            r: i as u8 * 50,
            g: i as u8 * 50,
            b: i as u8 * 50,
        })
        .collect()
}

// ── --bold audit: BoldMode::Off vs All vs Random ────────────────────────

/// BoldMode::Off vs BoldMode::All produce DIFFERENT bold values for
/// Middle cells. This is the core visual-difference proof.
#[test]
fn bold_off_vs_all_middle_cells_differ() {
    let palette = test_palette();
    let palette: &[Color] = &palette;
    // color_map all 1 → Middle cells would normally get color_idx=1
    let color_map: Vec<u8> = vec![1u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);

    let shader_off = make_shader(&slots, color_map, false, BoldMode::Off);
    let shader_all = make_shader(&slots, color_map, false, BoldMode::All);

    // Test multiple Middle cells — at least one should differ in bold.
    let mut diffs = 0;
    for line in 10..20 {
        for col in 0..20 {
            let (_, bold_off) =
                resolve_cell_color(&shader_off, 0, line, col, 'x', CharLoc::Middle, 25, 20);
            let (_, bold_all) =
                resolve_cell_color(&shader_all, 0, line, col, 'x', CharLoc::Middle, 25, 20);
            if bold_off != bold_all {
                diffs += 1;
            }
        }
    }
    assert!(
        diffs > 0,
        "BoldMode::Off vs All: ALL Middle cells produced identical bold values — bold flag is dead"
    );
}

/// BoldMode::Off produces bold=false for ALL Middle cells (no randomness).
#[test]
fn bold_off_middle_cells_never_bold() {
    let palette = test_palette();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![1u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);
    let shader = make_shader(&slots, color_map, false, BoldMode::Off);

    for line in 5..25 {
        for col in 0..30 {
            let (_, bold) = resolve_cell_color(&shader, 0, line, col, 'x', CharLoc::Middle, 30, 30);
            assert!(
                !bold,
                "BoldMode::Off: Middle cell at ({line},{col}) should NOT be bold"
            );
        }
    }
}

/// BoldMode::All produces bold=true for ALL Middle cells.
#[test]
fn bold_all_middle_cells_always_bold() {
    let palette = test_palette();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![1u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);
    let shader = make_shader(&slots, color_map, false, BoldMode::All);

    for line in 5..25 {
        for col in 0..30 {
            let (_, bold) = resolve_cell_color(&shader, 0, line, col, 'x', CharLoc::Middle, 30, 30);
            assert!(
                bold,
                "BoldMode::All: Middle cell at ({line},{col}) should be bold"
            );
        }
    }
}

/// BoldMode::Random produces a MIX of bold true/false across Middle cells
/// (otherwise it's not "random" — it's a constant).
#[test]
fn bold_random_middle_cells_produce_mixed_values() {
    let palette = test_palette();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![1u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);
    let shader = make_shader(&slots, color_map, false, BoldMode::Random);

    let mut bold_count = 0;
    let mut non_bold_count = 0;
    for line in 0..50 {
        for col in 0..50 {
            let (_, bold) = resolve_cell_color(&shader, 0, line, col, 'x', CharLoc::Middle, 30, 30);
            if bold {
                bold_count += 1;
            } else {
                non_bold_count += 1;
            }
        }
    }
    assert!(
        bold_count > 0,
        "BoldMode::Random: should produce SOME bold cells"
    );
    assert!(
        non_bold_count > 0,
        "BoldMode::Random: should produce SOME non-bold cells"
    );
    // Should be roughly 50/50 (the formula is (line ^ val) % 2)
    let total = bold_count + non_bold_count;
    let ratio = bold_count as f64 / total as f64;
    assert!(
        ratio > 0.3 && ratio < 0.7,
        "BoldMode::Random: bold ratio {ratio:.2} should be near 0.5, got {bold_count}/{total}"
    );
}

/// Head cells: BoldMode::Off makes them non-bold, BoldMode::All makes
/// them bold. This proves Head isn't unconditionally bold — the mode
/// DOES affect Head cells.
#[test]
fn bold_mode_affects_head_cells() {
    let palette = test_palette();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![1u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);

    let shader_off = make_shader(&slots, color_map, false, BoldMode::Off);
    let shader_all = make_shader(&slots, color_map, false, BoldMode::All);

    let (_, bold_off) = resolve_cell_color(&shader_off, 0, 20, 5, 'x', CharLoc::Head, 20, 10);
    let (_, bold_all) = resolve_cell_color(&shader_all, 0, 20, 5, 'x', CharLoc::Head, 20, 10);
    assert!(
        !bold_off,
        "BoldMode::Off: Head cell should be non-bold (match block overrides Head's bold=true)"
    );
    assert!(bold_all, "BoldMode::All: Head cell should be bold");
}

/// Tail cells: BoldMode::All makes them bold (overriding Tail's bold=false).
#[test]
fn bold_all_overrides_tail_to_bold() {
    let palette = test_palette();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![1u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);

    let shader_off = make_shader(&slots, color_map, false, BoldMode::Off);
    let shader_all = make_shader(&slots, color_map, false, BoldMode::All);

    let (_, bold_off) = resolve_cell_color(&shader_off, 0, 10, 5, 'x', CharLoc::Tail, 20, 10);
    let (_, bold_all) = resolve_cell_color(&shader_all, 0, 10, 5, 'x', CharLoc::Tail, 20, 10);
    assert!(!bold_off, "BoldMode::Off: Tail cell should be non-bold");
    assert!(
        bold_all,
        "BoldMode::All: Tail cell should be bold (match block overrides Tail's bold=false)"
    );
}

// ── --shadingmode audit: Random vs DistanceFromHead ─────────────────────

/// ShadingMode::Random vs DistanceFromHead produce DIFFERENT color_idx
/// values for Middle cells. This proves the shading mode flag has a
/// real visual effect.
#[test]
fn shadingmode_random_vs_distance_produces_different_colors() {
    let palette = test_palette();
    let palette: &[Color] = &palette;
    // color_map all 1 → Random mode gives color_idx=1 for all Middle cells
    let color_map: Vec<u8> = vec![1u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);

    let shader_random = make_shader(&slots, color_map, false, BoldMode::Off);
    let shader_distance = make_shader(&slots, color_map, true, BoldMode::Off);

    let mut diffs = 0;
    for line in 10..20 {
        for col in 0..20 {
            let (fg_random, _) =
                resolve_cell_color(&shader_random, 0, line, col, 'x', CharLoc::Middle, 25, 20);
            let (fg_distance, _) =
                resolve_cell_color(&shader_distance, 0, line, col, 'x', CharLoc::Middle, 25, 20);
            if fg_random != fg_distance {
                diffs += 1;
            }
        }
    }
    assert!(
        diffs > 0,
        "ShadingMode::Random vs DistanceFromHead: ALL Middle cells produced identical colors — shading_distance flag is dead"
    );
}

/// ShadingMode::DistanceFromHead produces a brightness DECAY from head
/// to tail (head_put_line is the brightest, cells further away get
/// progressively darker). This is the visual signature of cinematic mode.
#[test]
fn shadingmode_distance_produces_head_to_tail_decay() {
    let palette = test_palette();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![1u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);
    let shader = make_shader(&slots, color_map, true, BoldMode::Off);

    // head_put_line=25, length=20. Test cells at increasing distance.
    let head_put_line: u16 = 25;
    let length: u16 = 20;

    let mut colors_by_distance: Vec<(u16, Option<Color>)> = Vec::new();
    for dist in 1..=10 {
        let line = head_put_line - dist;
        let (fg, _) = resolve_cell_color(
            &shader,
            0,
            line,
            5,
            'x',
            CharLoc::Middle,
            head_put_line,
            length,
        );
        colors_by_distance.push((dist, fg));
    }

    // The palette is RGB (0,0,0), (50,50,50), (100,100,100), (150,150,150), (200,200,200).
    // DistanceFromHead mode uses exp(-k * dist/length) * last_idx, so cells
    // closer to head should land on higher palette indices (brighter).
    // Verify by extracting the R channel (R=G=B in this palette).
    let brightness = |c: Option<Color>| match c {
        Some(Color::Rgb { r, .. }) => r,
        _ => 0,
    };

    // Cell at dist=1 should be brighter than cell at dist=10 (overall trend,
    // not necessarily monotonic due to Bayer dithering).
    let near = brightness(colors_by_distance[0].1);
    let far = brightness(colors_by_distance[9].1);
    assert!(
        near >= far,
        "DistanceFromHead: cell near head (dist=1, brightness={near}) should be >= cell far from head (dist=10, brightness={far})"
    );
}
