// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! colors-custom unit tests — moved from the inline `mod tests` /
//! `suggestion_tests` / `precheck_tests` in colors_custom.rs to the
//! test/ mirror tree (NIGHT-hunter-1 convention) by NIGHT-hunt-37,
//! keeping colors_custom.rs under the 800-LOC hard cap while the
//! strictness contract grew. `super` is the colors_custom module —
//! the same resolution the inline mods had.

use super::*;

#[test]
fn parse_hex_color_full_with_hash() {
    let c = parse_hex_color("#ff0000").unwrap();
    assert_eq!(c, Color::Rgb { r: 255, g: 0, b: 0 });
}

#[test]
fn parse_hex_color_full_without_hash() {
    let c = parse_hex_color("00ff00").unwrap();
    assert_eq!(c, Color::Rgb { r: 0, g: 255, b: 0 });
}

#[test]
fn parse_hex_color_short_with_hash() {
    let c = parse_hex_color("#0f0").unwrap();
    assert_eq!(c, Color::Rgb { r: 0, g: 255, b: 0 });
}

#[test]
fn parse_hex_color_quoted() {
    let c = parse_hex_color("\"#4488ff\"").unwrap();
    assert_eq!(
        c,
        Color::Rgb {
            r: 68,
            g: 136,
            b: 255
        }
    );
}

#[test]
fn parse_hex_color_invalid() {
    assert!(parse_hex_color("#gg0000").is_err());
    assert!(parse_hex_color("xyz").is_err());
    assert!(parse_hex_color("").is_err());
}

#[test]
fn collect_colors_custom_rain_mode() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "colors-custom.mytheme.rain".to_string(),
        "#1a0033, #4d0080, #9933ff, #cc66ff, #ffffff".to_string(),
    );
    cfg.insert(
        "colors-custom.mytheme.bg".to_string(),
        "#0a0a12".to_string(),
    );

    let palettes = collect_colors_custom(&cfg);
    assert!(palettes.contains_key("mytheme"));
    let def = &palettes["mytheme"];
    assert_eq!(def.rain.len(), 5);
    assert_eq!(
        def.bg,
        Some(Color::Rgb {
            r: 10,
            g: 10,
            b: 18
        })
    );
}

#[test]
fn to_palette_rain_mode() {
    let def = CustomPaletteDef {
        rain: vec![
            Color::Rgb { r: 0, g: 0, b: 0 },
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
        ],
        bg: Some(Color::Rgb {
            r: 10,
            g: 10,
            b: 18,
        }),
    };
    let palette = def.to_palette().unwrap();
    // masterclass: 2 raw stops expand to 9 OKLab-polar samples.
    assert_eq!(palette.colors.len(), COLORS_CUSTOM_PALETTE_STEPS);
    assert_eq!(
        palette.bg,
        Some(Color::Rgb {
            r: 10,
            g: 10,
            b: 18
        })
    );
}

#[test]
fn to_palette_empty_fails() {
    let def = CustomPaletteDef::default();
    assert!(def.to_palette().is_err());
}

#[test]
fn to_palette_single_color_fails() {
    let def = CustomPaletteDef {
        rain: vec![Color::Rgb { r: 0, g: 0, b: 0 }],
        ..Default::default()
    };
    assert!(def.to_palette().is_err());
}

#[test]
fn load_custom_palette_not_found() {
    let cfg = HashMap::new();
    assert!(load_custom_palette(&cfg, "nonexistent").is_err());
}

#[test]
fn load_custom_palette_found() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "colors-custom.mytheme.rain".to_string(),
        "#000000, #ffffff".to_string(),
    );
    // NIGHT-hunt-37: bg is required for a loadable palette.
    cfg.insert(
        "colors-custom.mytheme.bg".to_string(),
        "#0a0a12".to_string(),
    );
    let palette = load_custom_palette(&cfg, "mytheme").unwrap();
    // masterclass: 2 CSV stops expand to 9 OKLab-polar samples.
    assert_eq!(palette.colors.len(), COLORS_CUSTOM_PALETTE_STEPS);
}

#[test]
fn load_custom_palette_case_insensitive() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "colors-custom.MyTheme.rain".to_string(),
        "#000000, #ffffff".to_string(),
    );
    // NIGHT-hunt-37: bg required; mixed-case key proves the collector
    // lowercases BOTH fields before matching.
    cfg.insert(
        "colors-custom.MYTHEME.bg".to_string(),
        "#0a0a12".to_string(),
    );
    let palette = load_custom_palette(&cfg, "mytheme").unwrap();
    // masterclass: 2 CSV stops expand to 9 OKLab-polar samples.
    assert_eq!(palette.colors.len(), COLORS_CUSTOM_PALETTE_STEPS);
}

/// v25 masterclass: TOML array format for rain field.
#[test]
fn rain_array_format_parses_7_stops() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "colors-custom.mythme.rain".to_string(),
        "[\"#1a0033\", \"#4d0080\", \"#9933ff\", \"#cc66ff\", \"#e6b3ff\", \"#f2ccff\", \"#ffffff\"]"
            .to_string(),
    );
    cfg.insert("colors-custom.mythme.bg".to_string(), "#0a0a12".to_string());
    // Verify the 7 stops were parsed correctly by inspecting the raw
    // CustomPaletteDef before to_palette() expands them.
    let collected = collect_colors_custom(&cfg);
    let raw = collected.get("mythme").expect("palette must be collected");
    assert_eq!(raw.rain.len(), 7, "array format must parse 7 raw stops");
    // After to_palette(), the 7 stops expand to COLORS_CUSTOM_PALETTE_STEPS
    // via the OKLab polar gradient engine (same as built-in themes).
    let palette = load_custom_palette(&cfg, "mythme").unwrap();
    assert_eq!(
        palette.colors.len(),
        COLORS_CUSTOM_PALETTE_STEPS,
        "expanded palette must have COLORS_CUSTOM_PALETTE_STEPS entries"
    );
}

/// v25 masterclass: CSV format still works (backward compat).
#[test]
fn rain_csv_format_still_works() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "colors-custom.oldstyle.rain".to_string(),
        "#000000, #ffffff".to_string(),
    );
    cfg.insert(
        "colors-custom.oldstyle.bg".to_string(),
        "#0a0a12".to_string(),
    );
    let palette = load_custom_palette(&cfg, "oldstyle").unwrap();
    // masterclass: 2 CSV stops expand to 9 OKLab-polar samples.
    assert_eq!(
        palette.colors.len(),
        COLORS_CUSTOM_PALETTE_STEPS,
        "CSV format must still work"
    );
}

/// masterclass: colors-custom must flow through the same OKLab
/// polar gradient engine as built-in themes. This integration test
/// asserts the two properties that prove the bypass is fixed:
///
/// 1. Expansion: 2 raw stops produce COLORS_CUSTOM_PALETTE_STEPS
///    palette entries (not the raw 2).
/// 2. Midpoint saturation: the middle palette entry is distinct
///    from both endpoints (after the palette-relative floor is
///    applied). This proves the polar OKLab engine is producing
///    interpolated colors, not just clamping to one endpoint.
///
/// Note: exact endpoint preservation is NOT asserted here. The
/// palette-relative floor (`apply_palette_relative_floor` in
/// `palette.rs:383`) intentionally boosts dark endpoints to prevent
/// total black crush on long rain trails. Every built-in theme
/// undergoes the same floor — so endpoint equality is not a property
/// of the chroma pipeline. The `to_palette_matches_builtin_gradient_path`
/// test below is the authoritative byte-match proof.
#[test]
fn to_palette_routes_through_oklab_polar_engine() {
    let black = Color::Rgb { r: 0, g: 0, b: 0 };
    let white = Color::Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let def = CustomPaletteDef {
        rain: vec![black, white],
        // NIGHT-hunt-37: bg is required for to_palette to build.
        bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
    };
    let palette = def.to_palette().expect("palette must build");

    // (1) Expansion: 2 raw stops -> 9 OKLab-polar samples.
    assert_eq!(
        palette.colors.len(),
        COLORS_CUSTOM_PALETTE_STEPS,
        "2 raw stops must expand to COLORS_CUSTOM_PALETTE_STEPS samples"
    );

    // (2) Midpoint saturation: the middle entry must not be equal to
    // either endpoint (after floor). A black-to-white gradient should
    // produce a mid-gray at the midpoint, distinct from both endpoints
    // even after the floor boosts black.
    let mid = palette
        .colors
        .get(COLORS_CUSTOM_PALETTE_STEPS / 2)
        .expect("midpoint must exist");
    let floor_first = palette.colors.first().expect("first entry must exist");
    let floor_last = palette.colors.last().expect("last entry must exist");
    assert_ne!(
        mid, floor_first,
        "midpoint must be interpolated, not clamped to first endpoint"
    );
    assert_ne!(
        mid, floor_last,
        "midpoint must be interpolated, not clamped to last endpoint"
    );
}

/// masterclass: colors-custom must produce the SAME output as a
/// built-in theme that uses the same raw stops. This is the strongest
/// possible proof that the bypass is fixed — colors-custom and built-in
/// themes now share the identical code path.
#[test]
fn to_palette_matches_builtin_gradient_path() {
    use crate::chroma_dragon_engine::palette::{color_to_rgb, colors_from_stops};
    use crate::runtime::ColorMode;

    let stops = vec![
        Color::Rgb { r: 26, g: 0, b: 51 }, // #1a0033
        Color::Rgb {
            r: 77,
            g: 0,
            b: 128,
        }, // #4d0080
        Color::Rgb {
            r: 153,
            g: 51,
            b: 255,
        }, // #9933ff
    ];
    let def = CustomPaletteDef {
        rain: stops.clone(),
        // NIGHT-hunt-37: bg is required for to_palette to build.
        bg: Some(stops[0].clone()),
    };
    let palette = def.to_palette().expect("palette must build");

    // Build the same palette directly through the chroma engine, exactly
    // like catalog.rs:933 does for built-in themes.
    let stops_rgb: Vec<(u8, u8, u8)> = stops.iter().map(|c| color_to_rgb(*c)).collect();
    let expected = colors_from_stops(
        ColorMode::TrueColor,
        &stops_rgb,
        COLORS_CUSTOM_PALETTE_STEPS,
    );

    assert_eq!(
        palette.colors, expected,
        "colors-custom palette must byte-match the chroma engine output for the same stops"
    );
}

// ── v50.0.0-beta.6 LTS: bounds enforcement tests ─────────────────

#[test]
fn collect_caps_rain_stops_at_max() {
    // A config with >COLORS_CUSTOM_MAX_RAIN_STOPS stops should cap
    // the palette.rain vec, not allocate unbounded memory.
    let mut cfg = HashMap::new();
    // Generate 100 stops (well over the 64 cap).
    let stops: Vec<String> = (0..100)
        .map(|i| format!("#{:02x}{:02x}{:02x}", i, i, i))
        .collect();
    cfg.insert("colors-custom.big.rain".to_string(), stops.join(", "));
    let map = collect_colors_custom(&cfg);
    let def = &map["big"];
    assert!(
        def.rain.len() <= COLORS_CUSTOM_MAX_RAIN_STOPS,
        "rain stops must be capped at {}, got {}",
        COLORS_CUSTOM_MAX_RAIN_STOPS,
        def.rain.len()
    );
}

#[test]
fn collect_caps_total_blocks_at_max() {
    // A config with >COLORS_CUSTOM_MAX_BLOCKS blocks keeps at most
    // MAX_BLOCKS entries (which ones survive is unspecified — HashMap
    // iteration order), never allocating unbounded.
    let mut cfg = HashMap::new();
    for i in 0..(COLORS_CUSTOM_MAX_BLOCKS + 10) {
        cfg.insert(
            format!("colors-custom.palette{i}.rain"),
            "#000000, #ffffff".to_string(),
        );
    }
    let map = collect_colors_custom(&cfg);
    assert!(
        map.len() <= COLORS_CUSTOM_MAX_BLOCKS,
        "total blocks must be capped at {}, got {}",
        COLORS_CUSTOM_MAX_BLOCKS,
        map.len()
    );
}

#[test]
fn collect_skips_oversized_names() {
    // A name longer than COLORS_CUSTOM_MAX_NAME_LEN should be
    // silently skipped (no allocation, no BTreeMap entry).
    // NIGHT-depthtest-3: the COLLECTOR still skips (this contract);
    // the config gate now hard-errors via
    // validate_colors_custom_name_len (colors_custom/name_len.rs).
    let mut cfg = HashMap::new();
    let long_name = "x".repeat(COLORS_CUSTOM_MAX_NAME_LEN + 1);
    cfg.insert(
        format!("colors-custom.{long_name}.rain"),
        "#000000, #ffffff".to_string(),
    );
    let map = collect_colors_custom(&cfg);
    assert!(
        map.is_empty(),
        "oversized name must be skipped, got {} entries",
        map.len()
    );
}

// ── did-you-mean suggestion tests ─────────────────────────────────

// v80.0.0-beta.1 did-you-mean audit (former mod suggestion_tests,
// now flat in this module): unknown palette names suggest the closest
// defined block.

#[test]
fn load_custom_palette_not_found_suggests_closest() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "colors-custom.cyberpunk_2077.rain".to_string(),
        "#000000, #ffffff".to_string(),
    );
    let err = load_custom_palette(&cfg, "cyberpunk_207").unwrap_err();
    assert!(
        err.contains("tip: a similar value exists: 'cyberpunk_2077'"),
        "palette typo must suggest the closest block, got: {err}"
    );
    // Distant name: no suggestion.
    let err = load_custom_palette(&cfg, "something-else").unwrap_err();
    assert!(!err.contains("tip: a similar"), "got: {err}");
}

// ── pre-check probe tests ─────────────────────────────────────────

// Former mod precheck_tests (v80.0.0-beta.1 killer-features hardening:
// pre-check probe behavior) — now flat in this module.

#[test]
fn is_colors_custom_name_false_when_no_blocks_defined() {
    let mut cfg = HashMap::new();
    cfg.insert("color".to_string(), "green".to_string());
    assert!(!is_colors_custom_name(&cfg, "sunset"));
    // Once a block exists, the probe falls through to the full lookup.
    cfg.insert(
        "colors-custom.sunset.rain".to_string(),
        "#000000, #ffffff".to_string(),
    );
    assert!(is_colors_custom_name(&cfg, "sunset"));
    assert!(!is_colors_custom_name(&cfg, "sunris"));
}

// ── NIGHT-hunter-24 (F-24-1): load-contract helpers ──

#[test]
fn load_error_none_for_valid_block_and_unknown_name() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "colors-custom.z.rain".to_string(),
        "#111111, #1ee460".to_string(),
    );
    // NIGHT-hunt-37: bg is required for a loadable palette.
    cfg.insert("colors-custom.z.bg".to_string(), "#0a0a0a".to_string());
    assert_eq!(colors_custom_load_error(&cfg, "z"), None);
    // Not a block: the caller's unknown-name path owns this case.
    assert_eq!(colors_custom_load_error(&cfg, "nope"), None);
    assert_eq!(colors_custom_load_error(&HashMap::new(), "z"), None);
}

#[test]
fn load_error_some_for_deficient_blocks() {
    // bg-only: entry exists, rain empty → to_palette error.
    let mut cfg = HashMap::new();
    cfg.insert("colors-custom.z.bg".to_string(), "#0a0a0a".to_string());
    let e = colors_custom_load_error(&cfg, "z").expect("bg-only must error");
    assert!(e.contains("at least 2"), "got: {e}");
    // Single valid stop.
    cfg.insert("colors-custom.z.rain".to_string(), "#111111".to_string());
    assert!(colors_custom_load_error(&cfg, "z").is_some());
    // Case-insensitive + trimmed lookup.
    assert!(colors_custom_load_error(&cfg, " Z ").is_some());
}

#[test]
fn validate_blocks_rejects_deficient_and_accepts_complete() {
    let mut cfg = HashMap::new();
    cfg.insert("colors-custom.bad.bg".to_string(), "#0a0a0a".to_string());
    cfg.insert(
        "colors-custom.good.rain".to_string(),
        "#111111, #1ee460".to_string(),
    );
    cfg.insert("colors-custom.good.bg".to_string(), "#0a0a0a".to_string());
    let e = validate_colors_custom_blocks(&cfg).expect("deficient block must error");
    assert!(e.contains("colors-custom.bad"), "got: {e}");
    cfg.remove("colors-custom.bad.bg");
    assert_eq!(validate_colors_custom_blocks(&cfg), None);
    assert_eq!(validate_colors_custom_blocks(&HashMap::new()), None);
}
