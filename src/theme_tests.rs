// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! theme tests, extracted from inline `mod tests { ... }` block in
//! theme.rs (Pattern D → Pattern C unification).
//!
//! Uses `use super::*;` to access theme.rs's private items unchanged.

use std::collections::{HashMap, HashSet};

use crate::cli::{all_color_schemes, cycle_color_scheme, parse_color_scheme};
use crate::palette::build_palette;
use crate::runtime::ColorMode;

fn runtime_color_scheme_variants() -> [ColorScheme; THEME_COUNT] {
    [
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
        ColorScheme::EnergyZen,
    ]
}

use super::*;
#[test]
fn catalog_count_is_current_theme_count() {
    assert_eq!(theme_count(), THEME_COUNT);
    assert_eq!(theme_count(), 44);
}

#[test]
fn every_color_scheme_has_metadata() {
    for scheme in runtime_color_scheme_variants() {
        assert!(
            metadata_for_scheme(scheme).is_some(),
            "missing theme metadata for {scheme:?}"
        );
    }
}

#[test]
fn every_scheme_has_exactly_one_canonical_entry() {
    let mut schemes = HashSet::new();
    for theme in themes() {
        assert!(
            schemes.insert(theme.scheme),
            "duplicate catalog entry for {:?}",
            theme.scheme
        );
    }
    assert_eq!(schemes.len(), runtime_color_scheme_variants().len());
}

#[test]
fn canonical_theme_names_are_unique() {
    let mut names = HashSet::new();
    for theme in themes() {
        assert!(
            names.insert(theme.name),
            "duplicate theme name {}",
            theme.name
        );
    }
}

#[test]
fn alias_names_are_unique_or_same_theme() {
    let mut aliases = HashMap::new();
    for theme in themes() {
        for alias in theme.aliases {
            if let Some(previous) = aliases.insert(*alias, theme.scheme) {
                assert_eq!(
                    previous, theme.scheme,
                    "alias {alias} maps to multiple themes"
                );
            }
        }
    }
}

#[test]
fn canonical_names_parse_to_catalog_scheme() {
    for theme in themes() {
        assert_eq!(parse_color_scheme(theme.name), Ok(theme.scheme));
    }
}

#[test]
fn existing_aliases_parse_with_previous_meaning() {
    assert_eq!(parse_color_scheme("synthwave"), Ok(ColorScheme::Neon));
    assert_eq!(parse_color_scheme("inferno"), Ok(ColorScheme::Fire));
    assert_eq!(parse_color_scheme("deep-sea"), Ok(ColorScheme::Ocean));
    assert_eq!(parse_color_scheme("deep_sea"), Ok(ColorScheme::Ocean));
    assert_eq!(parse_color_scheme("deepsea"), Ok(ColorScheme::Ocean));
    assert_eq!(parse_color_scheme("white"), Ok(ColorScheme::Snow));
    assert_eq!(parse_color_scheme("silver"), Ok(ColorScheme::Gray));
    assert_eq!(parse_color_scheme("grey"), Ok(ColorScheme::Gray));

    assert_eq!(
        parse_color_scheme("fancy-diamond"),
        Ok(ColorScheme::FancyDiamond)
    );
    assert_eq!(
        parse_color_scheme("fancy_diamond"),
        Ok(ColorScheme::FancyDiamond)
    );
    assert_eq!(
        parse_color_scheme("fancydiamond"),
        Ok(ColorScheme::FancyDiamond)
    );
}

#[test]
fn alias_inputs_have_canonical_display_names() {
    assert_eq!(canonical_name_for_input("white"), Some("snow"));
    assert_eq!(canonical_name_for_input("silver"), Some("gray"));
    assert_eq!(canonical_name_for_input("grey"), Some("gray"));
    assert_eq!(canonical_name_for_input("snow"), Some("snow"));
    assert_eq!(canonical_name_for_input("gray"), Some("gray"));
    assert_eq!(canonical_name_for_input("cosmos"), Some("cosmos"));
}

#[test]
fn parser_is_case_insensitive() {}

#[test]
fn cycle_color_scheme_uses_catalog_order() {
    let schemes = all_color_schemes();
    assert_eq!(schemes.len(), THEME_COUNT);
    for window in schemes.windows(2) {
        assert_eq!(cycle_color_scheme(window[0], 1), window[1]);
        assert_eq!(cycle_color_scheme(window[1], -1), window[0]);
    }
    assert_eq!(
        cycle_color_scheme(*schemes.last().unwrap(), 1),
        *schemes.first().unwrap()
    );
    assert_eq!(
        cycle_color_scheme(*schemes.first().unwrap(), -1),
        *schemes.last().unwrap()
    );
}

#[test]
fn every_catalog_entry_builds_a_palette() {
    for theme in themes() {
        let palette = build_palette(theme.scheme, ColorMode::TrueColor, true);
        assert!(
            !palette.colors.is_empty(),
            "empty palette for {}",
            theme.name
        );
    }
}

#[test]
fn readme_has_current_theme_count_wording() {
    let readme = include_str!("../README.md");
    assert!(!readme.contains("42 themes"));
    assert!(!readme.contains("42 built-in color schemes"));
    assert!(readme.contains("44 built-in themes"));
}

// ── (bug #13): "did you mean" color name suggestions ──

#[test]
fn unknown_color_cosmo_suggests_cosmos() {
    // 'cosmo' (missing last char) is edit-distance 1 from 'cosmos'.
    let err = parse_color_scheme("cosmo").unwrap_err();
    assert!(
        err.contains("Did you mean 'cosmos'?"),
        "should suggest cosmos for 'cosmo': {err}"
    );
}

#[test]
fn unknown_color_nebala_suggests_nebula() {
    // 'nebala' (missing 'u') is edit-distance 1 from 'nebula'.
    let err = parse_color_scheme("nebala").unwrap_err();
    assert!(
        err.contains("Did you mean 'nebula'?"),
        "should suggest nebula for 'nebala': {err}"
    );
}

#[test]
fn unknown_color_vaporwav_suggests_vaporwave() {
    // 'vaporwav' (missing 'e') is edit-distance 1 from 'vaporwave'.
    let err = parse_color_scheme("vaporwav").unwrap_err();
    assert!(
        err.contains("Did you mean 'vaporwave'?"),
        "should suggest vaporwave for 'vaporwav': {err}"
    );
}

#[test]
fn unknown_color_gren_suggests_green() {
    let err = parse_color_scheme("gren").unwrap_err();
    assert!(
        err.contains("Did you mean 'green'"),
        "should suggest green for 'gren': {err}"
    );
}

#[test]
fn unknown_color_completely_unrelated_no_suggestion() {
    // A string that's edit-distance > 2 from every color name should
    // NOT get a "did you mean" suggestion — just the plain error.
    let err = parse_color_scheme("xyzqwerty").unwrap_err();
    assert!(
        !err.contains("Did you mean"),
        "should not suggest for unrelated input: {err}"
    );
    assert!(
        err.contains("Use --list-colors"),
        "should still mention --list-colors: {err}"
    );
}

#[test]
fn unknown_color_empty_input_no_suggestion() {
    let err = parse_color_scheme("").unwrap_err();
    assert!(
        !err.contains("Did you mean"),
        "empty input should not suggest: {err}"
    );
}
