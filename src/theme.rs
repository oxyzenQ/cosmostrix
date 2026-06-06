// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Canonical color theme catalog.
//!
//! This module is the single source for color theme names, display order,
//! aliases, categories, and short descriptions. Palette construction remains
//! in `palette.rs`; this catalog does not tune visual output.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::runtime::ColorScheme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeCategory {
    Classic,
    Primary,
    Cinematic,
    Nature,
    Space,
    Planetary,
    Cosmic,
}

impl ThemeCategory {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Classic => "CLASSIC",
            Self::Primary => "PRIMARY",
            Self::Cinematic => "CINEMATIC",
            Self::Nature => "NATURE",
            Self::Space => "SPACE",
            Self::Planetary => "PLANETARY",
            Self::Cosmic => "COSMIC",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ThemeInfo {
    pub name: &'static str,
    pub scheme: ColorScheme,
    pub category: ThemeCategory,
    pub description: &'static str,
    pub aliases: &'static [&'static str],
}

pub const THEME_COUNT: usize = 43;

pub const THEME_CATEGORIES: &[ThemeCategory] = &[
    ThemeCategory::Classic,
    ThemeCategory::Primary,
    ThemeCategory::Cinematic,
    ThemeCategory::Nature,
    ThemeCategory::Space,
    ThemeCategory::Planetary,
    ThemeCategory::Cosmic,
];

pub const THEMES: &[ThemeInfo] = &[
    ThemeInfo {
        name: "green",
        scheme: ColorScheme::Green,
        category: ThemeCategory::Classic,
        description: "Classic Matrix green",
        aliases: &[],
    },
    ThemeInfo {
        name: "green2",
        scheme: ColorScheme::Green2,
        category: ThemeCategory::Classic,
        description: "Brighter green variant",
        aliases: &[],
    },
    ThemeInfo {
        name: "green3",
        scheme: ColorScheme::Green3,
        category: ThemeCategory::Classic,
        description: "Deep green variant",
        aliases: &[],
    },
    ThemeInfo {
        name: "yellow",
        scheme: ColorScheme::Yellow,
        category: ThemeCategory::Primary,
        description: "Warm yellow signal glow",
        aliases: &[],
    },
    ThemeInfo {
        name: "orange",
        scheme: ColorScheme::Orange,
        category: ThemeCategory::Primary,
        description: "Amber-orange terminal glow",
        aliases: &[],
    },
    ThemeInfo {
        name: "red",
        scheme: ColorScheme::Red,
        category: ThemeCategory::Primary,
        description: "High-alert red palette",
        aliases: &[],
    },
    ThemeInfo {
        name: "blue",
        scheme: ColorScheme::Blue,
        category: ThemeCategory::Primary,
        description: "Clean electric blue",
        aliases: &[],
    },
    ThemeInfo {
        name: "cyan",
        scheme: ColorScheme::Cyan,
        category: ThemeCategory::Primary,
        description: "Cool cyan terminal glow",
        aliases: &[],
    },
    ThemeInfo {
        name: "gold",
        scheme: ColorScheme::Gold,
        category: ThemeCategory::Primary,
        description: "Polished gold highlights",
        aliases: &[],
    },
    ThemeInfo {
        name: "rainbow",
        scheme: ColorScheme::Rainbow,
        category: ThemeCategory::Primary,
        description: "Full-spectrum color cycling",
        aliases: &[],
    },
    ThemeInfo {
        name: "purple",
        scheme: ColorScheme::Purple,
        category: ThemeCategory::Primary,
        description: "Saturated violet rain",
        aliases: &[],
    },
    ThemeInfo {
        name: "neon",
        scheme: ColorScheme::Neon,
        category: ThemeCategory::Cinematic,
        description: "Synthwave neon magenta/cyan",
        aliases: &["synthwave"],
    },
    ThemeInfo {
        name: "fire",
        scheme: ColorScheme::Fire,
        category: ThemeCategory::Cinematic,
        description: "Hot ember and flame tones",
        aliases: &["inferno"],
    },
    ThemeInfo {
        name: "ocean",
        scheme: ColorScheme::Ocean,
        category: ThemeCategory::Nature,
        description: "Deep sea blue-green palette",
        aliases: &["deep-sea", "deep_sea", "deepsea"],
    },
    ThemeInfo {
        name: "forest",
        scheme: ColorScheme::Forest,
        category: ThemeCategory::Nature,
        description: "Moss and canopy greens",
        aliases: &["jungle"],
    },
    ThemeInfo {
        name: "vaporwave",
        scheme: ColorScheme::Vaporwave,
        category: ThemeCategory::Cinematic,
        description: "Retro pink and cyan haze",
        aliases: &[],
    },
    ThemeInfo {
        name: "gray",
        scheme: ColorScheme::Gray,
        category: ThemeCategory::Classic,
        description: "Neutral monochrome gray",
        aliases: &["grey", "silver"],
    },
    ThemeInfo {
        name: "snow",
        scheme: ColorScheme::Snow,
        category: ThemeCategory::Cinematic,
        description: "Cold white-blue shimmer",
        aliases: &["white"],
    },
    ThemeInfo {
        name: "aurora",
        scheme: ColorScheme::Aurora,
        category: ThemeCategory::Cinematic,
        description: "Northern-light green and violet",
        aliases: &[],
    },
    ThemeInfo {
        name: "fancy-diamond",
        scheme: ColorScheme::FancyDiamond,
        category: ThemeCategory::Cinematic,
        description: "Prismatic diamond sparkle",
        aliases: &["fancy_diamond", "fancydiamond"],
    },
    ThemeInfo {
        name: "cosmos",
        scheme: ColorScheme::Cosmos,
        category: ThemeCategory::Space,
        description: "Cosmic blue/purple palette",
        aliases: &[],
    },
    ThemeInfo {
        name: "nebula",
        scheme: ColorScheme::Nebula,
        category: ThemeCategory::Space,
        description: "Nebula violet/cyan palette",
        aliases: &[],
    },
    ThemeInfo {
        name: "spectrum20",
        scheme: ColorScheme::Spectrum20,
        category: ThemeCategory::Cinematic,
        description: "Expanded twenty-stop spectrum",
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
        category: ThemeCategory::Space,
        description: "Bright starfield whites",
        aliases: &["star"],
    },
    ThemeInfo {
        name: "mars",
        scheme: ColorScheme::Mars,
        category: ThemeCategory::Planetary,
        description: "Rust-red Martian dust",
        aliases: &[],
    },
    ThemeInfo {
        name: "venus",
        scheme: ColorScheme::Venus,
        category: ThemeCategory::Planetary,
        description: "Cream and sulfur cloud tones",
        aliases: &[],
    },
    ThemeInfo {
        name: "mercury",
        scheme: ColorScheme::Mercury,
        category: ThemeCategory::Planetary,
        description: "Cool rocky silver palette",
        aliases: &[],
    },
    ThemeInfo {
        name: "jupiter",
        scheme: ColorScheme::Jupiter,
        category: ThemeCategory::Planetary,
        description: "Storm-band ochre and cream",
        aliases: &[],
    },
    ThemeInfo {
        name: "saturn",
        scheme: ColorScheme::Saturn,
        category: ThemeCategory::Planetary,
        description: "Soft ringed-planet golds",
        aliases: &[],
    },
    ThemeInfo {
        name: "uranus",
        scheme: ColorScheme::Uranus,
        category: ThemeCategory::Planetary,
        description: "Pale icy cyan",
        aliases: &[],
    },
    ThemeInfo {
        name: "neptune",
        scheme: ColorScheme::Neptune,
        category: ThemeCategory::Planetary,
        description: "Deep planetary blue",
        aliases: &[],
    },
    ThemeInfo {
        name: "pluto",
        scheme: ColorScheme::Pluto,
        category: ThemeCategory::Planetary,
        description: "Dim ice and umber tones",
        aliases: &[],
    },
    ThemeInfo {
        name: "moon",
        scheme: ColorScheme::Moon,
        category: ThemeCategory::Planetary,
        description: "Lunar gray-white palette",
        aliases: &[],
    },
    ThemeInfo {
        name: "sun",
        scheme: ColorScheme::Sun,
        category: ThemeCategory::Planetary,
        description: "Solar yellow-white heat",
        aliases: &[],
    },
    ThemeInfo {
        name: "comet",
        scheme: ColorScheme::Comet,
        category: ThemeCategory::Cosmic,
        description: "Icy tail blue-white streaks",
        aliases: &[],
    },
    ThemeInfo {
        name: "galaxy",
        scheme: ColorScheme::Galaxy,
        category: ThemeCategory::Cosmic,
        description: "Wide galactic purple/blue",
        aliases: &[],
    },
    ThemeInfo {
        name: "supernova",
        scheme: ColorScheme::Supernova,
        category: ThemeCategory::Cosmic,
        description: "Explosive stellar color burst",
        aliases: &["super-nova", "super_nova"],
    },
    ThemeInfo {
        name: "blackhole",
        scheme: ColorScheme::BlackHole,
        category: ThemeCategory::Cosmic,
        description: "Dark accretion-disk palette",
        aliases: &["black-hole", "black_hole"],
    },
    ThemeInfo {
        name: "andromeda",
        scheme: ColorScheme::Andromeda,
        category: ThemeCategory::Cosmic,
        description: "Andromeda blue-gold haze",
        aliases: &[],
    },
    ThemeInfo {
        name: "stardust",
        scheme: ColorScheme::Stardust,
        category: ThemeCategory::Cosmic,
        description: "Soft stellar dust shimmer",
        aliases: &["star-dust", "star_dust"],
    },
    ThemeInfo {
        name: "meteor",
        scheme: ColorScheme::Meteor,
        category: ThemeCategory::Cosmic,
        description: "Fast orange-white trail",
        aliases: &[],
    },
    ThemeInfo {
        name: "eclipse",
        scheme: ColorScheme::Eclipse,
        category: ThemeCategory::Cosmic,
        description: "Shadowed corona palette",
        aliases: &[],
    },
    ThemeInfo {
        name: "deepspace",
        scheme: ColorScheme::DeepSpace,
        category: ThemeCategory::Space,
        description: "Deep blue-black space palette",
        aliases: &[
            "deep-space",
            "deep_space",
            "deepblue",
            "deep-blue",
            "deep_blue",
        ],
    },
];

pub static THEME_LOOKUP: LazyLock<HashMap<&'static str, ColorScheme>> = LazyLock::new(|| {
    let mut lookup = HashMap::new();
    for theme in THEMES {
        insert_lookup(&mut lookup, theme.name, theme.scheme);
        for alias in theme.aliases {
            insert_lookup(&mut lookup, alias, theme.scheme);
        }
    }
    lookup
});

pub static SCHEME_ORDER: LazyLock<Vec<ColorScheme>> =
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
pub fn themes() -> &'static [ThemeInfo] {
    THEMES
}

#[must_use]
pub fn theme_count() -> usize {
    debug_assert_eq!(THEMES.len(), THEME_COUNT);
    THEME_COUNT
}

#[must_use]
pub fn lookup_theme(name: &str) -> Option<ColorScheme> {
    let key = name.trim().to_ascii_lowercase();
    THEME_LOOKUP.get(key.as_str()).copied()
}

#[must_use]
pub fn metadata_for_scheme(scheme: ColorScheme) -> Option<&'static ThemeInfo> {
    THEMES.iter().find(|theme| theme.scheme == scheme)
}

#[must_use]
pub fn canonical_name_for_scheme(scheme: ColorScheme) -> Option<&'static str> {
    metadata_for_scheme(scheme).map(|theme| theme.name)
}

#[must_use]
pub fn canonical_name_for_input(name: &str) -> Option<&'static str> {
    lookup_theme(name).and_then(canonical_name_for_scheme)
}

#[must_use]
pub fn compact_list_text() -> String {
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

#[must_use]
pub fn detail_list_text() -> String {
    let mut out = String::new();
    for category in THEME_CATEGORIES {
        out.push_str(category.label());
        out.push('\n');
        for theme in themes().iter().filter(|theme| theme.category == *category) {
            if theme.aliases.is_empty() {
                out.push_str(&format!("  {:<15} {}\n", theme.name, theme.description));
            } else {
                out.push_str(&format!(
                    "  {:<15} {} (aliases: {})\n",
                    theme.name,
                    theme.description,
                    theme.aliases.join(", ")
                ));
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;
    use crate::cli::{all_color_schemes, cycle_color_scheme, parse_color_scheme};
    use crate::palette::build_palette;
    use crate::runtime::ColorMode;

    fn runtime_color_scheme_variants() -> [ColorScheme; THEME_COUNT] {
        [
            ColorScheme::Green,
            ColorScheme::Green2,
            ColorScheme::Green3,
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
            ColorScheme::Comet,
            ColorScheme::Galaxy,
            ColorScheme::Supernova,
            ColorScheme::BlackHole,
            ColorScheme::Andromeda,
            ColorScheme::Stardust,
            ColorScheme::Meteor,
            ColorScheme::Eclipse,
            ColorScheme::DeepSpace,
        ]
    }

    #[test]
    fn catalog_count_is_current_theme_count() {
        assert_eq!(theme_count(), THEME_COUNT);
        assert_eq!(theme_count(), 43);
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
        assert_eq!(parse_color_scheme("deepblue"), Ok(ColorScheme::DeepSpace));
        assert_eq!(parse_color_scheme("deep-blue"), Ok(ColorScheme::DeepSpace));
        assert_eq!(parse_color_scheme("deep_blue"), Ok(ColorScheme::DeepSpace));
        assert_eq!(parse_color_scheme("black-hole"), Ok(ColorScheme::BlackHole));
        assert_eq!(parse_color_scheme("black_hole"), Ok(ColorScheme::BlackHole));
        assert_eq!(parse_color_scheme("super-nova"), Ok(ColorScheme::Supernova));
        assert_eq!(parse_color_scheme("super_nova"), Ok(ColorScheme::Supernova));
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
        assert_eq!(canonical_name_for_input("deepblue"), Some("deepspace"));
        assert_eq!(canonical_name_for_input("deep-blue"), Some("deepspace"));
        assert_eq!(canonical_name_for_input("deep_blue"), Some("deepspace"));
        assert_eq!(canonical_name_for_input("snow"), Some("snow"));
        assert_eq!(canonical_name_for_input("gray"), Some("gray"));
        assert_eq!(canonical_name_for_input("deepspace"), Some("deepspace"));
    }

    #[test]
    fn parser_is_case_insensitive() {
        assert_eq!(parse_color_scheme("deepSpace"), Ok(ColorScheme::DeepSpace));
        assert_eq!(parse_color_scheme("BLACK-HOLE"), Ok(ColorScheme::BlackHole));
    }

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
    fn detailed_color_list_includes_categories_and_canonical_themes() {
        let detail = detail_list_text();
        for category in THEME_CATEGORIES {
            assert!(detail.contains(category.label()));
        }
        for theme in themes() {
            assert!(detail.contains(theme.name), "missing {}", theme.name);
        }
        assert!(detail.contains("aliases: white"));
        assert!(detail.contains("aliases: grey, silver"));
        assert!(detail.contains("deepblue, deep-blue, deep_blue"));
    }

    #[test]
    fn readme_has_current_theme_count_wording() {
        let readme = include_str!("../README.md");
        assert!(!readme.contains("42 themes"));
        assert!(!readme.contains("42 built-in color schemes"));
        assert!(readme.contains("43 built-in themes"));
    }
}
