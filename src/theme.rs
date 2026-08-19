// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Canonical color theme catalog.
//!
//! This module is the single source for color theme names, display order,
//! aliases, categories, and short descriptions. Palette construction remains
//! in `palette.rs`; this catalog does not tune visual output.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::runtime::ColorScheme;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ThemeInfo {
    pub name: &'static str,
    pub scheme: ColorScheme,
    pub aliases: &'static [&'static str],
}

pub(crate) const THEME_COUNT: usize = 44;

pub(crate) const THEMES: &[ThemeInfo] = &[
    ThemeInfo {
        name: "green",
        scheme: ColorScheme::Green,
        aliases: &[],
    },
    ThemeInfo {
        name: "green2",
        scheme: ColorScheme::Green2,
        aliases: &[],
    },
    ThemeInfo {
        name: "green3",
        scheme: ColorScheme::Green3,
        aliases: &[],
    },
    ThemeInfo {
        name: "neon-green",
        scheme: ColorScheme::NeonGreen,
        aliases: &["neon_green", "neongreen"],
    },
    ThemeInfo {
        name: "neon-purple",
        scheme: ColorScheme::NeonPurple,
        aliases: &["neon_purple", "neonpurple"],
    },
    ThemeInfo {
        name: "neon-white",
        scheme: ColorScheme::NeonWhite,
        aliases: &["neon_white", "neonwhite"],
    },
    ThemeInfo {
        name: "neon-blue",
        scheme: ColorScheme::NeonBlue,
        aliases: &["neon_blue", "neonblue"],
    },
    ThemeInfo {
        name: "neon-red",
        scheme: ColorScheme::NeonRed,
        aliases: &["neon_red", "neonred"],
    },
    ThemeInfo {
        name: "neon-orange",
        scheme: ColorScheme::NeonOrange,
        aliases: &["neon_orange", "neonorange"],
    },
    ThemeInfo {
        name: "neon-yellow",
        scheme: ColorScheme::NeonYellow,
        aliases: &["neon_yellow", "neonyellow"],
    },
    ThemeInfo {
        name: "neon-cyan",
        scheme: ColorScheme::NeonCyan,
        aliases: &["neon_cyan", "neoncyan"],
    },
    ThemeInfo {
        name: "carbon",
        scheme: ColorScheme::Carbon,
        aliases: &[],
    },
    ThemeInfo {
        name: "yellow",
        scheme: ColorScheme::Yellow,
        aliases: &[],
    },
    ThemeInfo {
        name: "orange",
        scheme: ColorScheme::Orange,
        aliases: &[],
    },
    ThemeInfo {
        name: "red",
        scheme: ColorScheme::Red,
        aliases: &[],
    },
    ThemeInfo {
        name: "blue",
        scheme: ColorScheme::Blue,
        aliases: &[],
    },
    ThemeInfo {
        name: "cyan",
        scheme: ColorScheme::Cyan,
        aliases: &[],
    },
    ThemeInfo {
        name: "gold",
        scheme: ColorScheme::Gold,
        aliases: &[],
    },
    ThemeInfo {
        name: "rainbow",
        scheme: ColorScheme::Rainbow,
        aliases: &[],
    },
    ThemeInfo {
        name: "purple",
        scheme: ColorScheme::Purple,
        aliases: &[],
    },
    ThemeInfo {
        name: "neon",
        scheme: ColorScheme::Neon,
        aliases: &["synthwave"],
    },
    ThemeInfo {
        name: "fire",
        scheme: ColorScheme::Fire,
        aliases: &["inferno"],
    },
    ThemeInfo {
        name: "ocean",
        scheme: ColorScheme::Ocean,
        aliases: &["deep-sea", "deep_sea", "deepsea"],
    },
    ThemeInfo {
        name: "forest",
        scheme: ColorScheme::Forest,
        aliases: &["jungle"],
    },
    ThemeInfo {
        name: "vaporwave",
        scheme: ColorScheme::Vaporwave,
        aliases: &[],
    },
    ThemeInfo {
        name: "gray",
        scheme: ColorScheme::Gray,
        aliases: &["grey", "silver"],
    },
    ThemeInfo {
        name: "snow",
        scheme: ColorScheme::Snow,
        aliases: &["white"],
    },
    ThemeInfo {
        name: "aurora",
        scheme: ColorScheme::Aurora,
        aliases: &[],
    },
    ThemeInfo {
        name: "fancy-diamond",
        scheme: ColorScheme::FancyDiamond,
        aliases: &["fancy_diamond", "fancydiamond"],
    },
    ThemeInfo {
        name: "cosmos",
        scheme: ColorScheme::Cosmos,
        aliases: &[],
    },
    ThemeInfo {
        name: "nebula",
        scheme: ColorScheme::Nebula,
        aliases: &[],
    },
    ThemeInfo {
        name: "spectrum20",
        scheme: ColorScheme::Spectrum20,
        aliases: &[
            "spectrum-20",
            "spectrum_20",
            "theme20",
            "theme-20",
            "theme_20",
        ],
    },
    ThemeInfo {
        name: "stars",
        scheme: ColorScheme::Stars,
        aliases: &["star"],
    },
    ThemeInfo {
        name: "mars",
        scheme: ColorScheme::Mars,
        aliases: &[],
    },
    ThemeInfo {
        name: "venus",
        scheme: ColorScheme::Venus,
        aliases: &[],
    },
    ThemeInfo {
        name: "mercury",
        scheme: ColorScheme::Mercury,
        aliases: &[],
    },
    ThemeInfo {
        name: "jupiter",
        scheme: ColorScheme::Jupiter,
        aliases: &[],
    },
    ThemeInfo {
        name: "saturn",
        scheme: ColorScheme::Saturn,
        aliases: &[],
    },
    ThemeInfo {
        name: "uranus",
        scheme: ColorScheme::Uranus,
        aliases: &[],
    },
    ThemeInfo {
        name: "neptune",
        scheme: ColorScheme::Neptune,
        aliases: &[],
    },
    ThemeInfo {
        name: "pluto",
        scheme: ColorScheme::Pluto,
        aliases: &[],
    },
    ThemeInfo {
        name: "moon",
        scheme: ColorScheme::Moon,
        aliases: &[],
    },
    ThemeInfo {
        name: "sun",
        scheme: ColorScheme::Sun,
        aliases: &[],
    },
    // ── Premium exclusive rarity ────────────────────────────────────────
    // energy-zen: the signature purple-neon palette honoring the cosmostrix
    // + oxyzenQ journey. Deeper saturation than NeonPurple, brighter head
    // with a crystal-edge magenta lift in the mid stops. Default for the
    // monolith + cinematic scenes.
    ThemeInfo {
        name: "energy-zen",
        scheme: ColorScheme::EnergyZen,
        aliases: &["energy_zen", "energyzen", "ez"],
    },
];

pub(crate) static THEME_LOOKUP: LazyLock<HashMap<&'static str, ColorScheme>> =
    LazyLock::new(|| {
        let mut lookup = HashMap::new();
        for theme in THEMES {
            insert_lookup(&mut lookup, theme.name, theme.scheme);
            for alias in theme.aliases {
                insert_lookup(&mut lookup, alias, theme.scheme);
            }
        }
        lookup
    });

pub(crate) static SCHEME_ORDER: LazyLock<Vec<ColorScheme>> =
    LazyLock::new(|| THEMES.iter().map(|theme| theme.scheme).collect());

fn insert_lookup(
    lookup: &mut HashMap<&'static str, ColorScheme>,
    name: &'static str,
    scheme: ColorScheme,
) {
    if let Some(previous) = lookup.insert(name, scheme) {
        assert_eq!(
            previous, scheme,
            "conflicting color theme alias '{name}' maps to multiple schemes"
        );
    }
}

#[must_use]
pub(crate) fn themes() -> &'static [ThemeInfo] {
    THEMES
}

#[must_use]
pub(crate) fn theme_count() -> usize {
    debug_assert_eq!(THEMES.len(), THEME_COUNT);
    THEME_COUNT
}

#[must_use]
pub(crate) fn lookup_theme(name: &str) -> Option<ColorScheme> {
    let key = name.trim().to_ascii_lowercase();
    THEME_LOOKUP.get(key.as_str()).copied()
}

#[must_use]
fn metadata_for_scheme(scheme: ColorScheme) -> Option<&'static ThemeInfo> {
    THEMES.iter().find(|theme| theme.scheme == scheme)
}

#[must_use]
pub(crate) fn canonical_name_for_scheme(scheme: ColorScheme) -> Option<&'static str> {
    metadata_for_scheme(scheme).map(|theme| theme.name)
}

#[must_use]
pub(crate) fn canonical_name_for_input(name: &str) -> Option<&'static str> {
    lookup_theme(name).and_then(canonical_name_for_scheme)
}

#[must_use]
pub(crate) fn compact_list_text() -> String {
    let mut out = String::new();
    for row in themes().chunks(3) {
        out.push_str("  ");
        for theme in row {
            out.push_str(&format!("{:<15}", theme.name));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
