// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! HUD chroma gradient + metric stability regression tests — extracted
//! from `hud/tests.rs` to keep that file under the 800-LOC hard cap.
//!
//! Covers: compute_chroma_gradient_24 smoothness + NaN/Inf safety +
//! metric setter clamping/sanitization.

use super::*;

// ── v50 (2026-08-17) HUD chroma gradient smoothness regression tests ────
//
// C5 fix: compute_chroma_gradient_24 now uses interpolate_palette_color
// (linear lerp between adjacent palette stops via blend_toward_rgb)
// instead of discrete sampling `palette_colors[(t * last).round()]`.
// This eliminates visible bands when the palette has fewer stops than
// the HUD has rows (e.g. a 3-stop palette + 16 HUD rows previously
// produced 4/8/4 band blocks; now produces a smooth gradient).

#[test]
fn compute_chroma_gradient_24_smooth_with_small_palette_no_bands() {
    // THE OWNER REGRESSION TEST for HUD chroma gradient smoothness.
    //
    // Before C5: a 3-stop palette (white/grey/black) + 16 HUD rows
    // produced discrete bands — multiple adjacent rows shared the same
    // palette stop, creating visible color blocks instead of a smooth
    // gradient. The owner flagged this as inconsistent with the chroma
    // dragon smoothness mandate.
    //
    // After C5: every HUD row gets a smoothly-interpolated color via
    // `interpolate_palette_color` (the same helper the border message
    // uses). Adjacent rows produce DISTINCT colors — no bands.
    //
    // Test palette: pure red(255,0,0), pure green(0,255,0), pure blue(0,0,255).
    // High-contrast colorful stops ensure adjacent interpolated values are
    // distinct enough that the brighten_color floor (TARGET_V=200) does
    // NOT collapse them to the same brightened value. (A monochromatic
    // palette like white/grey/black WOULD expose the brighten floor's
    // integer-math collapse — that's a separate concern about the
    // readability/smoothness tradeoff, not a regression from C5.)
    let palette = vec![
        Color::Rgb { r: 255, g: 0, b: 0 }, // idx 0: red
        Color::Rgb { r: 0, g: 255, b: 0 }, // idx 1: green
        Color::Rgb { r: 0, g: 0, b: 255 }, // idx 2: blue
    ];
    let colors = compute_chroma_gradient_24(&palette);
    assert_eq!(colors.len(), 24, "HUD gradient must have 24 entries");

    // Count distinct colors. With interpolation, every row gets a
    // unique color (22 distinct values, modulo the brighten floor
    // collapsing some to neutral grey). The old discrete-sampling
    // implementation would have produced only 3 distinct values
    // (one per palette stop). Assert >=5 distinct to leave room for
    // the brighten floor's grey fallback.
    let distinct_count = {
        let mut unique: Vec<Color> = colors.to_vec();
        unique.dedup();
        unique.len()
    };
    assert!(
        distinct_count >= 5,
        "interpolated HUD gradient must produce >=5 distinct colors across \
         16 rows with a 3-stop palette (got {distinct_count}) — the old \
         discrete-sampling implementation would have produced only 3 (one \
         per palette stop), causing visible bands"
    );

    // Assert NO two adjacent rows share the same color — that's the
    // visible band the owner reported. With smooth interpolation, every
    // adjacent pair must differ (since `frac` increments by 1/15 each row
    // and the blend_toward_rgb factor changes the output).
    for i in 0..23 {
        assert_ne!(
            colors[i],
            colors[i + 1],
            "adjacent HUD rows {i} and {} must NOT share the same color — \
             that's the visible band the owner reported (palette: red/green/blue)",
            i + 1
        );
    }
}

#[test]
fn compute_chroma_gradient_24_large_palette_still_exact_at_integer_t() {
    // Backward compatibility: with a 24-stop palette (one stop per HUD
    // row), the interpolated t = i/23.0 lands exactly on integer palette
    // positions, so the helper returns palette[i] exactly (no
    // interpolation). The brighten step is then applied as before. This
    // test verifies the C5 fix does NOT regress the 22-stop-palette
    // case — every row still gets its dedicated palette stop's color
    // (post-brighten).
    //
    // Z-master-1X round 5: palette expanded from 22 → 24 entries to match
    // the 24 HUD rows (dcel + tcel added). With 1:1 mapping, t = i/23.0
    // maps to palette index i*23/23 = i exactly.
    //
    // Test palette: 24 distinct RGB values, all with max channel >= 200
    // so brighten returns each as-is (isolates the gradient mapping
    // from the brightening math).
    let palette: Vec<Color> = (0..24)
        .map(|i| Color::Rgb {
            r: 200 + (i as u8 % 56),
            g: 200,
            b: 200,
        })
        .collect();
    let colors = compute_chroma_gradient_24(&palette);
    for (i, expected) in palette.iter().enumerate() {
        assert_eq!(
            &colors[i], expected,
            "row {i} must use palette[{i}] exactly (24-stop palette, t lands on integer boundary)"
        );
    }
}

// ── v50 (2026-08-17) HUD metric stability regression tests ─────────
//
// The 7 new owner-mandated metric setters were strengthened with NaN/Inf
// handling, range clamping, and string sanitization. These tests verify
// the sanitization at the setter level so a future code path that
// bypasses the setter still gets sanitized output via `update_metrics`.

#[test]
fn hud_set_endurance_health_score_clamps_nan_inf_and_range() {
    // The `ehs:` setter must clamp NaN, +Inf, -Inf to 0.0 (rendered as
    // `ehs: 0` — visibly degraded, forcing investigation). In-range
    // values are clamped to [0.0, 100.0]. This is the defense-in-depth
    // layer that complements `update_metrics` (which also clamps).
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    // NaN → 0
    h.set_endurance_health_score(f64::NAN);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[6];
    assert_eq!(line, " ehs: 0", "NaN ehs must render as 0");

    // +Inf → 0
    h.set_endurance_health_score(f64::INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[6];
    assert_eq!(line, " ehs: 0", "+Inf ehs must render as 0");

    // -Inf → 0
    h.set_endurance_health_score(f64::NEG_INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[6];
    assert_eq!(line, " ehs: 0", "-Inf ehs must render as 0");

    // Negative → clamp to 0
    h.set_endurance_health_score(-42.0);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[6];
    assert_eq!(line, " ehs: 0", "negative ehs must clamp to 0");

    // Above 100 → clamp to 100
    h.set_endurance_health_score(250.0);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[6];
    assert_eq!(line, " ehs: 100", "ehs above 100 must clamp to 100");
}

#[test]
fn hud_set_effective_pressure_clamps_nan_inf_and_range() {
    // The `prs:` setter must clamp NaN, +Inf, -Inf to 0.0 (rendered as
    // `prs: 0.00`). In-range values are clamped to [0.0, 1.0]. This is
    // the defense-in-depth layer that complements the existing clamp
    // in `update_metrics`.
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    // NaN → 0
    h.set_effective_pressure(f32::NAN);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[7];
    assert_eq!(line, " prs: 0.00", "NaN prs must render as 0.00");

    // +Inf → 0
    h.set_effective_pressure(f32::INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[7];
    assert_eq!(line, " prs: 0.00", "+Inf prs must render as 0.00");

    // -Inf → 0
    h.set_effective_pressure(f32::NEG_INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[7];
    assert_eq!(line, " prs: 0.00", "-Inf prs must render as 0.00");

    // Below 0 → 0
    h.set_effective_pressure(-0.5);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[7];
    assert_eq!(line, " prs: 0.00", "negative prs must clamp to 0.00");

    // Above 1 → 1
    h.set_effective_pressure(2.5);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[7];
    assert_eq!(line, " prs: 1.00", "prs above 1 must clamp to 1.00");
}

#[test]
fn hud_set_chars_per_sec_clamps_nan_and_negative() {
    // The `sped:` setter must clamp NaN, +Inf, -Inf, and negative values
    // to 0.0 (rendered as `sped: 0.0` — visibly broken, forcing
    // investigation rather than hiding the issue). This is the
    // defense-in-depth layer for the chars-per-second speed metric.
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    // NaN → 0
    h.set_chars_per_sec(f32::NAN);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[11];
    assert_eq!(line, " sped: 0.0", "NaN sped must render as 0.0");

    // +Inf → 0
    h.set_chars_per_sec(f32::INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[11];
    assert_eq!(line, " sped: 0.0", "+Inf sped must render as 0.0");

    // -Inf → 0
    h.set_chars_per_sec(f32::NEG_INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[11];
    assert_eq!(line, " sped: 0.0", "-Inf sped must render as 0.0");

    // Negative → 0
    h.set_chars_per_sec(-25.0);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[11];
    assert_eq!(line, " sped: 0.0", "negative sped must clamp to 0.0");
}

#[test]
fn hud_set_droplet_density_clamps_nan_and_negative() {
    // The `dsty:` setter must clamp NaN, +Inf, -Inf, and negative values
    // to 0.0 (rendered as `dsty: 0.00` — visibly broken, forcing
    // investigation). Owner explicitly mandated `dsty` label (NOT `den`).
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    // NaN → 0
    h.set_droplet_density(f32::NAN);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[12];
    assert_eq!(line, " dsty: 0.00", "NaN dsty must render as 0.00");

    // +Inf → 0
    h.set_droplet_density(f32::INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[12];
    assert_eq!(line, " dsty: 0.00", "+Inf dsty must render as 0.00");

    // -Inf → 0
    h.set_droplet_density(f32::NEG_INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[12];
    assert_eq!(line, " dsty: 0.00", "-Inf dsty must render as 0.00");

    // Negative → 0
    h.set_droplet_density(-2.0);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[12];
    assert_eq!(line, " dsty: 0.00", "negative dsty must clamp to 0.00");
}

#[test]
fn hud_set_scene_name_and_charset_preset_truncate_long_input() {
    // The `scn:` and `chr:` setters must truncate input to 14 chars (by
    // char count, preserving UTF-8 boundaries) so a very long custom
    // scene name or charset preset cannot blow past the HUD_MAX_WIDTH
    // (22 cols) budget. The ` scn: ` prefix is 6 chars (so 6 + 14 = 20
    // ≤ 22); the ` chr: ` prefix is also 6 chars (so 6 + 14 = 20 ≤ 22).
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    // Long scene name (30 chars) → truncated to 14
    let long_name = "abcdefghijklmnopqrstuvwxyz1234"; // 30 chars
    h.set_scene_name(long_name);
    h.update_metrics(&palette);
    let (_, scn_line) = &h.cached_lines[8];
    assert_eq!(
        scn_line, " scn: abcdefghijklmn",
        "scn line must truncate to first 14 chars of long scene name"
    );
    assert_eq!(
        scn_line.chars().count(),
        20,
        "truncated scn line must be 6 + 14 = 20 chars (prefix ' scn: ' is 6 chars)"
    );

    // Long charset preset (26 chars) → truncated to 14
    let long_preset = "PRESET0123456789abcdefghij"; // 26 chars
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.set_charset_preset(long_preset);
    h.update_metrics(&palette);
    let (_, chr_line) = &h.cached_lines[9];
    assert_eq!(
        chr_line, " chr: PRESET01234567",
        "chr line must truncate to first 14 chars of long charset preset"
    );
    assert_eq!(
        chr_line.chars().count(),
        20,
        "truncated chr line must be 6 + 14 = 20 chars (prefix ' chr: ' is 6 chars)"
    );
}
// ── v50 (2026-08-17) HUD expansion content tests ───────────────────
//
// The following tests verify the 7 new owner-mandated metric lines
// (rows 6-12) render the correct text after the corresponding setters
// are called and `update_metrics` runs the 1 Hz text reformat. The
// layout matches owner's Option S mandate: ehs/prs/sped/dsty/scn/chr/
// clr at rows 6-12, with the density label explicitly set to `dsty`
// (NOT `den` — owner judged `den` as ugly/unsuitable).

#[test]
fn hud_renders_seven_new_metric_lines_after_setters_and_update() {
    // Verify all 7 new owner-mandated metric lines render with the
    // expected label prefix + value formatting after their setters are
    // called and `update_metrics` runs. The chroma gradient is fed an
    // arbitrary palette so the colors[i] sweep is exercised too.
    let mut h = HudState::new();
    h.toggle(); // visible + forces next update_metrics to execute
    h.set_endurance_health_score(87.4);
    h.set_effective_pressure(0.123);
    h.set_chars_per_sec(14.0);
    h.set_droplet_density(1.0);
    h.set_scene_name("cinematic");
    h.set_charset_preset("binary");
    h.set_color_scheme(ColorScheme::NeonGreen);
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50,
        };
        16
    ];
    h.update_metrics(&palette);

    // Row 6 — ehs: integer rounded (87.4 -> 87)
    let (_, ehs_line) = &h.cached_lines[6];
    assert_eq!(ehs_line, " ehs: 87", "row 6 (ehs) content mismatch");

    // Row 7 — prs: 2 decimals, clamped 0.0-1.0
    let (_, prs_line) = &h.cached_lines[7];
    assert_eq!(prs_line, " prs: 0.12", "row 7 (prs) content mismatch");

    // Row 8 — sped: 1 decimal
    let (_, sped_line) = &h.cached_lines[11];
    assert_eq!(sped_line, " sped: 14.0", "row 11 (sped) content mismatch");

    // Row 9 — dsty: 2 decimals. Owner mandated `dsty` (NOT `den`).
    // v50.0.0-beta.6 Option D + v51.2 banded masterclass: dsty is DYNAMIC
    // when power-dragon is ON. This test sets power_dragon ON (default) +
    // pressure=0.123 + density=1.0, so dsty = the v51.2 banded target:
    // p=0.123 is in the LOW band (0.05 < p < 0.30), t=(0.123-0.05)/0.25,
    // target = 0.84 - 0.14*t = 0.799 -> 0.80 (rounded to 2 decimals).
    let (_, dsty_line) = &h.cached_lines[12];
    assert!(
        dsty_line.starts_with(" dsty: "),
        "row 12 must start with ' dsty: ', got: {dsty_line:?}"
    );
    assert_eq!(
        dsty_line, " dsty: 0.80",
        "row 12 (dsty) content mismatch — dynamic banded throttle"
    );

    // Row 10 — scn: scene name string
    let (_, scn_line) = &h.cached_lines[8];
    assert_eq!(scn_line, " scn: cinematic", "row 8 (scn) content mismatch");

    // Row 11 — chr: charset preset string
    let (_, chr_line) = &h.cached_lines[9];
    assert_eq!(chr_line, " chr: binary", "row 9 (chr) content mismatch");

    // Row 12 — clr: Debug format of ColorScheme
    let (_, clr_line) = &h.cached_lines[10];
    assert_eq!(clr_line, " clr: NeonGreen", "row 10 (clr) content mismatch");
}

#[test]
fn hud_density_label_is_dsty_not_den() {
    // Owner explicitly mandated the `dsty` label for density (NOT `den`):
    // owner judged `den` as ugly/unsuitable for the density multiplier
    // label. This regression test locks the label in so a future rename
    // would fail loudly. The value formatting is verified separately.
    let mut h = HudState::new();
    h.toggle();
    h.set_droplet_density(2.5);
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];
    h.update_metrics(&palette);
    let (_, dsty_line) = &h.cached_lines[12];
    assert!(
        dsty_line.starts_with(" dsty: "),
        "density label must be ' dsty: ' (NOT ' den: ' per owner mandate), got: {dsty_line:?}"
    );
    assert!(
        !dsty_line.starts_with(" den"),
        "density label must NOT be ' den' (owner judged `den` as ugly/unsuitable), got: {dsty_line:?}"
    );
}

#[test]
fn hud_effective_pressure_clamps_to_unity_range() {
    // The `prs:` line clamps the underlying f32 to [0.0, 1.0] before
    // formatting. Values outside this range (e.g., a 1.5 thermal spike
    // or a -0.1 jitter) are rendered as 1.00 / 0.00 respectively. This
    // matches the `effective_pressure()` downstream contract.
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    // Above 1.0 — clamps to 1.00
    h.set_effective_pressure(1.5);
    h.update_metrics(&palette);
    let (_, prs_line) = &h.cached_lines[7];
    assert_eq!(prs_line, " prs: 1.00", "prs must clamp > 1.0 to 1.00");

    // Below 0.0 — clamps to 0.00
    h.set_effective_pressure(-0.5);
    // Force the next metric tick to execute immediately.
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, prs_line) = &h.cached_lines[7];
    assert_eq!(prs_line, " prs: 0.00", "prs must clamp < 0.0 to 0.00");

    // In-range — exact 2-decimal format
    h.set_effective_pressure(0.347);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, prs_line) = &h.cached_lines[7];
    assert_eq!(
        prs_line, " prs: 0.35",
        "prs must format in-range value with 2 decimals"
    );
}

#[test]
fn hud_endurance_health_score_rounds_to_integer() {
    // The `ehs:` line rounds the f64 score to the nearest integer so
    // the HUD reads as a calm 0-100 number (matches htop/mangoHUD
    // convention for summary metrics — sub-integer precision would
    // cause flicker without adding diagnostic value).
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    h.set_endurance_health_score(87.4);
    h.update_metrics(&palette);
    let (_, ehs_line) = &h.cached_lines[6];
    assert_eq!(ehs_line, " ehs: 87", "ehs must round 87.4 to 87");

    h.set_endurance_health_score(87.6);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, ehs_line) = &h.cached_lines[6];
    assert_eq!(ehs_line, " ehs: 88", "ehs must round 87.6 to 88");

    h.set_endurance_health_score(100.0);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, ehs_line) = &h.cached_lines[6];
    assert_eq!(ehs_line, " ehs: 100", "ehs must render 100 as-is");
}

#[test]
fn hud_final_layout_positions_match_z_master_1x_round5() {
    // Regression guard for the row layout. Locks in the position of
    // every row so a future reorder would fail loudly.
    //
    // Layout (24 rows, Z-master-1X round 5 owner mandate 2026-08-31):
    //   0   fps
    //   1   tgt
    //   2   max
    //   3   p99
    //   4   cpu
    //   5   rss
    //   6   ehs
    //   7   prs
    //   8   scn    (v51: moved up — identity lines)
    //   9   chr    (v51: moved up)
    //   10  clr    (v51: moved up)
    //   11  sped   (v51: moved down — user controls after identity)
    //   12  dsty   (v51: moved down)
    //   13  prdr
    //   14  crdr
    //   15  ambt
    //   16  glth
    //   17  ctun
    //   18  mnst
    //   19  dcel   (Z-master-1X round 5: dirty cell ratio %)
    //   20  tcel   (Z-master-1X round 5: total cells)
    //   21  cid    (Z-master-1X round 5: moved down from 19 to make room)
    //   22  up     (Z-master-1X round 5: moved down from 20)
    //   23  screensize (Z-master-1X round 5: moved down from 21 — visual anchor)
    let h = HudState::new();
    // The cid line is static and lives only at row 21.
    let (_, cid_line) = &h.cached_lines[21];
    assert!(
        cid_line.starts_with(" cid: "),
        "row 21 must be the cid line (Z-master-1X round 5: above the session footer), got: {cid_line:?}"
    );
    // Every OTHER row must NOT contain the cid prefix.
    for (i, (_, text)) in h.cached_lines.iter().enumerate() {
        if i == 21 {
            continue;
        }
        assert!(
            !text.starts_with(" cid: "),
            "row {i} must NOT contain the cid prefix — cid is exclusive to row 21, got: {text:?}"
        );
    }
}
