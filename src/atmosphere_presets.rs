// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Controlled atmosphere profile preset registry for v4.6.0 Phase 2.
//!
//! Defines a small, deterministic set of named atmosphere presets that map
//! to already-allowed mode/regime pairs. These presets are documentation
//! and test ground truth — they do NOT change default visual behavior,
//! do NOT enable live atmosphere by default, and do NOT include a storm preset.
//!
//! ## Invariants
//!
//! - No preset is default. Users must explicitly opt in.
//! - Every preset maps to an allowed mode/regime combination only.
//! - No preset maps to storm or any unknown mode/regime.
//! - No preset enables color change or terminal effects.
//! - `atmosphere-calm` always produces identity shadow.
//! - Non-calm presets always produce whisper shadow under controlled-live.

// Module-level allow is needed because the preset registry items are
// consumed in test modules (atmosphere_expansion_tests, docs_tests/zactrix)
// but not in the non-test binary path — consistent with atmosphere_shadow.rs,
// atmosphere_visual.rs, and other atmosphere modules.
#![allow(dead_code)]

/// A controlled atmosphere profile preset definition.
///
/// Each preset defines a specific mode/regime combination and documents
/// the expected runtime behavior. Presets are pure data — they do not
/// execute any code or mutate any state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AtmospherePreset {
    /// Preset name (matches config/profile usage).
    pub name: &'static str,
    /// Atmosphere mode for this preset.
    pub mode: &'static str,
    /// Atmosphere regime for this preset.
    pub regime: &'static str,
    /// Expected shadow risk label when applied.
    pub expected_shadow: &'static str,
    /// Human-readable description.
    pub description: &'static str,
}

impl AtmospherePreset {
    /// Whether this preset expects identity shadow (no visual impact).
    pub(crate) fn expects_identity(&self) -> bool {
        self.expected_shadow == "identity"
    }

    /// Whether this preset expects whisper shadow (imperceptible modulation).
    pub(crate) fn expects_whisper(&self) -> bool {
        self.expected_shadow == "whisper"
    }
}

/// All controlled atmosphere preset names in definition order.
pub(crate) const ATMOSPHERE_PRESET_NAMES: &[&str] = &[
    "atmosphere-calm",
    "atmosphere-pulse",
    "atmosphere-signal",
    "atmosphere-compression",
    "atmosphere-void",
    "atmosphere-monolith-pressure",
];

/// Look up a controlled atmosphere preset by name.
///
/// Returns `None` for unknown names, including "atmosphere-storm"
/// (storm preset does not exist and must never exist).
#[must_use]
pub(crate) fn get_atmosphere_preset(name: &str) -> Option<AtmospherePreset> {
    match name.trim().to_ascii_lowercase().as_str() {
        "atmosphere-calm" => Some(AtmospherePreset {
            name: "atmosphere-calm",
            mode: "disabled",
            regime: "calm",
            expected_shadow: "identity",
            description: "Disabled mode + calm regime. Zero visual impact. Default-safe.",
        }),
        "atmosphere-pulse" => Some(AtmospherePreset {
            name: "atmosphere-pulse",
            mode: "controlled-live",
            regime: "pulse",
            expected_shadow: "whisper",
            description: "Controlled-live + pulse. Periodic intensity waves, whisper-bounded.",
        }),
        "atmosphere-signal" => Some(AtmospherePreset {
            name: "atmosphere-signal",
            mode: "controlled-live",
            regime: "signal",
            expected_shadow: "whisper",
            description:
                "Controlled-live + signal. Focused directional convergence, whisper-bounded.",
        }),
        "atmosphere-compression" => Some(AtmospherePreset {
            name: "atmosphere-compression",
            mode: "controlled-live",
            regime: "compression",
            expected_shadow: "whisper",
            description:
                "Controlled-live + compression. Gradually increasing density, whisper-bounded.",
        }),
        "atmosphere-void" => Some(AtmospherePreset {
            name: "atmosphere-void",
            mode: "controlled-live",
            regime: "void",
            expected_shadow: "whisper",
            description:
                "Controlled-live + void. Minimal activity, sparse streams, whisper-bounded.",
        }),
        "atmosphere-monolith-pressure" => Some(AtmospherePreset {
            name: "atmosphere-monolith-pressure",
            mode: "controlled-live",
            regime: "monolith-pressure",
            expected_shadow: "whisper",
            description:
                "Controlled-live + monolith-pressure. Enhanced monolith presence, whisper-bounded.",
        }),
        _ => None,
    }
}

/// All defined atmosphere presets as a slice.
#[must_use]
pub(crate) fn all_atmosphere_presets() -> Vec<AtmospherePreset> {
    ATMOSPHERE_PRESET_NAMES
        .iter()
        .filter_map(|&name| get_atmosphere_preset(name))
        .collect()
}

/// Check whether a name refers to a storm-related preset.
///
/// Storm presets must never exist. This function is used in tests
/// to ensure no storm preset is accidentally introduced.
#[must_use]
pub(crate) fn is_storm_preset_name(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    lower.contains("storm")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_preset_names_match_registry() {
        for &name in ATMOSPHERE_PRESET_NAMES {
            assert!(
                get_atmosphere_preset(name).is_some(),
                "preset '{name}' must exist in registry"
            );
        }
        assert_eq!(
            ATMOSPHERE_PRESET_NAMES.len(),
            all_atmosphere_presets().len(),
            "name list length must match registry length"
        );
    }

    #[test]
    fn no_storm_preset_exists() {
        assert!(
            get_atmosphere_preset("atmosphere-storm").is_none(),
            "atmosphere-storm must not exist"
        );
        assert!(
            get_atmosphere_preset("storm").is_none(),
            "storm must not exist as atmosphere preset"
        );
    }

    #[test]
    fn unknown_preset_returns_none() {
        assert!(get_atmosphere_preset("").is_none());
        assert!(get_atmosphere_preset("nonexistent").is_none());
        assert!(get_atmosphere_preset("atmosphere-aggressive").is_none());
    }

    #[test]
    fn calm_preset_is_identity() {
        let preset = get_atmosphere_preset("atmosphere-calm").unwrap();
        assert!(preset.expects_identity());
        assert!(!preset.expects_whisper());
        assert_eq!(preset.mode, "disabled");
        assert_eq!(preset.regime, "calm");
    }

    #[test]
    fn non_calm_presets_are_whisper() {
        for &name in &[
            "atmosphere-pulse",
            "atmosphere-signal",
            "atmosphere-compression",
            "atmosphere-void",
            "atmosphere-monolith-pressure",
        ] {
            let preset = get_atmosphere_preset(name).unwrap();
            assert!(
                preset.expects_whisper(),
                "{name} must expect whisper shadow"
            );
            assert!(!preset.expects_identity());
            assert_eq!(preset.mode, "controlled-live");
        }
    }

    #[test]
    fn every_preset_maps_to_allowed_mode() {
        for preset in all_atmosphere_presets() {
            assert!(
                preset.mode == "disabled" || preset.mode == "controlled-live",
                "preset '{}' has invalid mode '{}'",
                preset.name,
                preset.mode
            );
        }
    }

    #[test]
    fn every_preset_maps_to_allowed_regime() {
        let allowed = [
            "calm",
            "pulse",
            "signal",
            "compression",
            "void",
            "monolith-pressure",
        ];
        for preset in all_atmosphere_presets() {
            assert!(
                allowed.contains(&preset.regime),
                "preset '{}' has invalid regime '{}'",
                preset.name,
                preset.regime
            );
        }
    }

    #[test]
    fn no_preset_maps_to_storm() {
        for preset in all_atmosphere_presets() {
            assert_ne!(
                preset.regime, "storm",
                "preset '{}' must not map to storm",
                preset.name
            );
            assert!(
                !is_storm_preset_name(preset.name),
                "preset '{}' must not contain 'storm' in name",
                preset.name
            );
        }
    }

    #[test]
    fn all_presets_have_descriptions() {
        for preset in all_atmosphere_presets() {
            assert!(
                !preset.description.is_empty(),
                "preset '{}' must have a description",
                preset.name
            );
            assert!(
                preset.description.len() > 10,
                "preset '{}' description must be meaningful",
                preset.name
            );
        }
    }

    #[test]
    fn preset_count_is_six() {
        assert_eq!(all_atmosphere_presets().len(), 6);
        assert_eq!(ATMOSPHERE_PRESET_NAMES.len(), 6);
    }
}
