// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    Mono,
    Color16,
    Color256,
    TrueColor,
}

/// Which color pipeline is active for the current run.
///
/// The Chroma Dragon engine (`src/chroma_dragon_engine/`) is the primary coloring
/// authority — it owns palette construction (OKLab gradient since v30),
/// per-cell base shader (`resolve_cell_color`), atmospheric post-FX
/// (`apply_climate`), palette-aware ghost color, and palette-aware
/// anomaly halos. When the terminal cannot represent truecolor output,
/// the same code paths fall back to legacy sRGB-linear math: identical
/// per-channel brightness/blend equations, but without OKLab palette
/// construction, perceptual blending, or atmospheric drift.
///
/// Detection rule (owner directive: "all color -> chroma dragon first
/// -> fallback legacy rgb/srgb"):
/// - `ColorMode::TrueColor` -> `ChromaDragon`
/// - `ColorMode::{Color256, Color16, Mono}` -> `LegacyRgb`
///
/// The active pipeline is disclosed in `cosmostrix -v`,
/// `cosmostrix --doctor`, and the benchmark CONFIG block so the user
/// can verify which path is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorPipeline {
    /// Chroma Dragon engine: OKLab gradient, perceptual blend,
    /// climate post-FX, head halo, L-smoothing, subpixel jitter.
    /// Active when `ColorMode == TrueColor`.
    ChromaDragon,

    /// Legacy sRGB-linear pipeline: raw per-channel RGB math, no
    /// OKLab, no climate post-FX. Active when `ColorMode` is
    /// `Color256`, `Color16`, or `Mono`. This is NOT a separate code
    /// path -- it is the SAME call sites with the chroma helpers
    /// swapped for their raw-RGB equivalents in `chroma::legacy`.
    LegacyRgb,
}

impl ColorPipeline {
    /// Resolve the active pipeline from the terminal color mode.
    ///
    /// The chroma engine requires truecolor output to express the
    /// OKLab-constructed palette stops. On `Color256`/`Color16`/`Mono`
    /// the palette would be quantized away, so the legacy sRGB-linear
    /// math is used directly -- it produces the same per-channel
    /// brightness/blend result without the wasted OKLab round-trip.
    #[must_use]
    pub const fn detect(color_mode: ColorMode) -> Self {
        match color_mode {
            ColorMode::TrueColor => Self::ChromaDragon,
            ColorMode::Color256 | ColorMode::Color16 | ColorMode::Mono => Self::LegacyRgb,
        }
    }

    /// Short stable label for verbose / doctor / benchmark output.
    /// Example: `chroma_dragon`, `legacy_rgb`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ChromaDragon => "chroma_dragon",
            Self::LegacyRgb => "legacy_rgb",
        }
    }

    /// One-line human-readable description of the active pipeline.
    /// Surfaced next to `label()` in `-v` and `--doctor`.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::ChromaDragon => {
                "oklab gradient, perceptual blend, climate post-fx, head halo, l-smoothing"
            }
            Self::LegacyRgb => {
                "sRGB-linear fallback (color mode lacks truecolor; no OKLab, no climate post-fx)"
            }
        }
    }

    /// True when the Chroma Dragon engine is the active pipeline.
    /// Used by hot-path call sites to branch between the chroma
    /// helper and the legacy helper without a runtime string compare.
    #[must_use]
    pub const fn is_chroma(self) -> bool {
        matches!(self, Self::ChromaDragon)
    }

    /// Human-readable reason when the pipeline is `LegacyRgb`.
    /// Returns `None` when the pipeline is `ChromaDragon` (no
    /// fallback reason to disclose).
    #[must_use]
    pub const fn disable_reason(self, color_mode: ColorMode) -> Option<&'static str> {
        match (self, color_mode) {
            (Self::ChromaDragon, _) => None,
            (Self::LegacyRgb, ColorMode::Color256) => {
                Some("color_mode=Color256 -- chroma needs truecolor; legacy sRGB-linear in effect")
            }
            (Self::LegacyRgb, ColorMode::Color16) => {
                Some("color_mode=Color16 -- chroma needs truecolor; legacy sRGB-linear in effect")
            }
            (Self::LegacyRgb, ColorMode::Mono) => {
                Some("color_mode=Mono -- chroma needs truecolor; legacy sRGB-linear in effect")
            }
            // Defensive: LegacyRgb with TrueColor should not occur via
            // `detect()`, but the function is public so a caller could
            // force it. Disclose the state honestly.
            (Self::LegacyRgb, ColorMode::TrueColor) => {
                Some("color_mode=TrueColor but pipeline forced to legacy_rgb")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadingMode {
    Random,
    DistanceFromHead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoldMode {
    Off,
    Random,
    All,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonolithSize {
    #[value(name = "small")]
    Small,
    #[value(name = "normal")]
    Normal,
    #[value(name = "large")]
    Large,
}

impl MonolithSize {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Normal => "normal",
            Self::Large => "large",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorScheme {
    Green,
    Green2,
    Green3,
    NeonGreen,
    NeonPurple,
    NeonWhite,
    NeonBlue,
    NeonRed,
    NeonOrange,
    NeonYellow,
    NeonCyan,
    Carbon,
    Yellow,
    Orange,
    Red,
    Blue,
    Cyan,
    Gold,
    Rainbow,
    Purple,
    Neon,
    Fire,
    Ocean,
    Forest,
    Vaporwave,
    Gray,
    Snow,
    Aurora,
    FancyDiamond,
    Cosmos,
    Nebula,
    Spectrum20,
    Stars,
    Mars,
    Venus,
    Mercury,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
    Pluto,
    Moon,
    Sun,
    /// Premium exclusive purple-neon palette — the "energy-zen" rarity.
    /// Honors the cosmostrix + oxyzenQ journey. Default for monolith +
    /// cinematic scenes. Distinct from NeonPurple via deeper saturation,
    /// brighter head, and a crystal-edge magenta lift in the mid stops.
    EnergyZen,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_truecolor_routes_to_chroma_dragon() {
        assert_eq!(
            ColorPipeline::detect(ColorMode::TrueColor),
            ColorPipeline::ChromaDragon
        );
    }

    #[test]
    fn detect_non_truecolor_routes_to_legacy_rgb() {
        assert_eq!(
            ColorPipeline::detect(ColorMode::Color256),
            ColorPipeline::LegacyRgb
        );
        assert_eq!(
            ColorPipeline::detect(ColorMode::Color16),
            ColorPipeline::LegacyRgb
        );
        assert_eq!(
            ColorPipeline::detect(ColorMode::Mono),
            ColorPipeline::LegacyRgb
        );
    }

    #[test]
    fn label_is_stable_machine_readable() {
        // These strings appear in -v / --doctor / --benchmark output.
        // Lock them so a refactor cannot silently change the disclosed
        // pipeline identifier.
        assert_eq!(ColorPipeline::ChromaDragon.label(), "chroma_dragon");
        assert_eq!(ColorPipeline::LegacyRgb.label(), "legacy_rgb");
    }

    #[test]
    fn description_is_non_empty_for_both_pipelines() {
        // Both descriptions must be non-empty and contain a keyword
        // the user can grep for ("oklab" for chroma, "fallback" for
        // legacy).
        let chroma = ColorPipeline::ChromaDragon.description();
        let legacy = ColorPipeline::LegacyRgb.description();
        assert!(!chroma.is_empty());
        assert!(!legacy.is_empty());
        assert!(chroma.contains("oklab"));
        assert!(legacy.contains("fallback"));
    }

    #[test]
    fn is_chroma_only_for_chroma_dragon() {
        assert!(ColorPipeline::ChromaDragon.is_chroma());
        assert!(!ColorPipeline::LegacyRgb.is_chroma());
    }

    #[test]
    fn disable_reason_none_for_chroma_dragon() {
        // When chroma is active there is no fallback to explain.
        for mode in [
            ColorMode::Mono,
            ColorMode::Color16,
            ColorMode::Color256,
            ColorMode::TrueColor,
        ] {
            assert_eq!(
                ColorPipeline::ChromaDragon.disable_reason(mode),
                None,
                "chroma_dragon should not emit a disable_reason for {:?}",
                mode
            );
        }
    }

    #[test]
    fn disable_reason_some_for_legacy_rgb() {
        // Every LegacyRgb state must disclose why.
        for mode in [
            ColorMode::Mono,
            ColorMode::Color16,
            ColorMode::Color256,
            ColorMode::TrueColor,
        ] {
            let reason = ColorPipeline::LegacyRgb.disable_reason(mode);
            assert!(
                reason.is_some(),
                "legacy_rgb should always disclose a disable_reason for {:?}",
                mode
            );
            assert!(
                reason.unwrap().contains("legacy"),
                "disable_reason for {:?} should mention legacy, got: {}",
                mode,
                reason.unwrap()
            );
        }
    }

    #[test]
    fn detect_is_const_evaluable() {
        // `detect` is `const fn` so it can be used in const context
        // (e.g. array sizes, const initializer for Config). Verify
        // by constructing a const value.
        const PIPELINE: ColorPipeline = ColorPipeline::detect(ColorMode::TrueColor);
        assert_eq!(PIPELINE, ColorPipeline::ChromaDragon);
    }
}
