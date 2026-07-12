// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Preset system: named parameter profiles that map to existing CLI options.
//!
//! Presets provide curated visual profiles combining color, charset, speed,
//! density, and glitch intensity into a single `--preset <name>` invocation.
//! Explicit CLI flags always override preset values.
//!
//! ## Precedence
//!
//! 1. Built-in defaults (clap `default_value` / `default_value_t`)
//! 2. Config file values
//! 3. Preset values (override config-managed preset fields)
//! 4. `--low-power` for fields not touched by preset or explicit CLI
//! 5. Explicit CLI flags (always win)

use crate::config::color_enabled_stdout;
use crate::config::GlitchLevel;

/// A named preset mapping to existing CLI options.
#[derive(Debug, Clone)]
pub struct PresetConfig {
    pub color: &'static str,
    pub charset: &'static str,
    pub fps: f64,
    pub speed: f32,
    pub density: f32,
    pub glitch_level: GlitchLevel,
}

/// All preset names in definition order.
pub const PRESET_NAMES: &[&str] = &[
    "classic",
    "cinematic",
    "calm",
    "monolith",
    "storm",
    "cosmos",
    "neon",
    "hacker",
    "low-power",
];

/// Preset descriptions aligned with [`PRESET_NAMES`].
const PRESET_DESCRIPTIONS: &[&str] = &[
    "The original green-on-black Matrix rain",
    "Cosmic binary with cinematic feel",
    "Gentle ocean tones with reduced density",
    "Dark and heavy binary rain",
    "Fast and intense purple cyberpunk",
    "Cosmic binary with rich cosmos palette",
    "Vibrant cyberpunk with neon colors",
    "Green hacker aesthetic at high speed",
    "Power-saving mode (30 FPS, reduced density/speed)",
];

/// Look up a preset by case-insensitive name.
#[must_use]
pub fn get_preset(name: &str) -> Option<PresetConfig> {
    match name.trim().to_ascii_lowercase().as_str() {
        "classic" => Some(PresetConfig {
            color: "green",
            charset: "matrix",
            fps: 60.0,
            speed: 8.0,
            density: 1.0,
            glitch_level: GlitchLevel::Default,
        }),
        "cinematic" => Some(PresetConfig {
            color: "cosmos",
            charset: "binary",
            fps: 60.0,
            speed: 8.0,
            density: 1.0,
            glitch_level: GlitchLevel::Default,
        }),
        "calm" => Some(PresetConfig {
            color: "ocean",
            charset: "minimal",
            fps: 60.0,
            speed: 5.0,
            density: 0.65,
            glitch_level: GlitchLevel::Subtle,
        }),
        "monolith" => Some(PresetConfig {
            color: "cosmos",
            charset: "binary",
            fps: 60.0,
            speed: 4.0,
            density: 0.75,
            glitch_level: GlitchLevel::Subtle,
        }),
        "storm" => Some(PresetConfig {
            color: "purple",
            charset: "cyberpunk",
            fps: 120.0,
            speed: 24.0,
            density: 1.35,
            glitch_level: GlitchLevel::Intense,
        }),
        "cosmos" => Some(PresetConfig {
            color: "cosmos",
            charset: "binary",
            fps: 60.0,
            speed: 9.0,
            density: 1.05,
            glitch_level: GlitchLevel::Default,
        }),
        "neon" => Some(PresetConfig {
            color: "neon",
            charset: "cyberpunk",
            fps: 60.0,
            speed: 10.0,
            density: 1.1,
            glitch_level: GlitchLevel::Default,
        }),
        "hacker" => Some(PresetConfig {
            color: "green",
            charset: "hacker",
            fps: 60.0,
            speed: 11.0,
            density: 1.2,
            glitch_level: GlitchLevel::Default,
        }),
        "low-power" => Some(PresetConfig {
            color: "green",
            charset: "binary",
            fps: 30.0,
            speed: 5.0,
            density: 0.5,
            glitch_level: GlitchLevel::Default,
        }),
        _ => None,
    }
}

/// Validate a preset name. Returns the normalized (lowercased) name or an error.
pub fn validate_preset_name(name: &str) -> Result<String, String> {
    let lower = name.trim().to_ascii_lowercase();
    if get_preset(&lower).is_some() {
        Ok(lower)
    } else {
        Err(format!(
            "error: unknown preset '{name}'\n\n  Use --list-presets to see available presets."
        ))
    }
}

/// Default values that presets are compared against to show only overrides.
const DEFAULTS: PresetConfig = PresetConfig {
    color: "green",
    charset: "binary",
    fps: 60.0,
    speed: 8.0,
    density: 1.0,
    glitch_level: GlitchLevel::Default,
};

/// Print detailed information about a single preset.
///
/// Returns `Ok(())` on success, `Err(message)` if the preset is not found.
pub fn print_show_preset(name: &str) -> Result<(), String> {
    let lower = name.trim().to_ascii_lowercase();
    let Some(p) = get_preset(&lower) else {
        return Err(format!(
            "error: unknown preset '{name}'\n\n  Use --list-presets to see available presets."
        ));
    };

    // Find the description for this preset.
    let desc = PRESET_NAMES
        .iter()
        .position(|&n| n == lower)
        .map(|i| PRESET_DESCRIPTIONS[i])
        .unwrap_or("");

    println!("PRESET: {lower}");
    println!();
    println!("  Description: {desc}");
    println!();
    println!("  Overrides:");

    let mut has_override = false;
    if p.color != DEFAULTS.color {
        println!("    color   = {}", p.color);
        has_override = true;
    }
    if p.charset != DEFAULTS.charset {
        println!("    charset = {}", p.charset);
        has_override = true;
    }
    if p.fps != DEFAULTS.fps {
        println!("    fps     = {}", p.fps);
        has_override = true;
    }
    if p.speed != DEFAULTS.speed {
        println!("    speed   = {}", p.speed);
        has_override = true;
    }
    if (p.density - DEFAULTS.density).abs() > f32::EPSILON {
        println!("    density = {}", p.density);
        has_override = true;
    }
    if p.glitch_level != DEFAULTS.glitch_level {
        let label = match p.glitch_level {
            GlitchLevel::None => "none",
            GlitchLevel::Subtle => "subtle",
            GlitchLevel::Default => "default",
            GlitchLevel::Intense => "intense",
        };
        println!("    glitch  = {label}");
        has_override = true;
    }

    if !has_override {
        println!("    (matches defaults — no overrides)");
    }

    println!();
    println!("  Use: cosmostrix --preset {lower}");
    Ok(())
}

/// Print all available presets with one-line descriptions.
pub fn print_list_presets() {
    if color_enabled_stdout() {
        println!("\x1b[1;36mAVAILABLE PRESETS:\x1b[0m");
    } else {
        println!("AVAILABLE PRESETS:");
    }
    println!();
    for (i, name) in PRESET_NAMES.iter().enumerate() {
        println!("  {:12} {}", name, PRESET_DESCRIPTIONS[i]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_preset_names_present() {
        assert_eq!(PRESET_NAMES.len(), 9);
        assert_eq!(PRESET_DESCRIPTIONS.len(), 9);
    }

    #[test]
    fn get_preset_valid_names() {
        for &name in PRESET_NAMES {
            assert!(get_preset(name).is_some(), "preset '{}' should exist", name);
        }
    }

    #[test]
    fn get_preset_invalid_name() {
        assert!(get_preset("foobar").is_none());
        assert!(get_preset("").is_none());
        assert!(get_preset("Classic ").is_some()); // trimmed + case-insensitive
    }

    #[test]
    fn validate_preset_name_valid() {
        for &name in PRESET_NAMES {
            assert!(validate_preset_name(name).is_ok());
        }
    }

    #[test]
    fn validate_preset_name_invalid() {
        let err = validate_preset_name("nonexistent").unwrap_err();
        assert!(err.contains("unknown preset"));
        assert!(err.contains("nonexistent"));
        assert!(err.contains("--list-presets"));
    }

    #[test]
    fn validate_preset_name_case_insensitive() {
        assert_eq!(validate_preset_name("STORM").unwrap(), "storm");
        assert_eq!(validate_preset_name(" Calm ").unwrap(), "calm");
    }

    #[test]
    fn preset_values_classic() {
        let p = get_preset("classic").unwrap();
        assert_eq!(p.color, "green");
        assert_eq!(p.charset, "matrix");
        assert_eq!(p.fps, 60.0);
        assert_eq!(p.speed, 8.0);
        assert_eq!(p.density, 1.0);
        assert_eq!(p.glitch_level, GlitchLevel::Default);
    }

    #[test]
    fn preset_values_storm() {
        let p = get_preset("storm").unwrap();
        assert_eq!(p.color, "purple");
        assert_eq!(p.charset, "cyberpunk");
        assert_eq!(p.fps, 120.0);
        assert_eq!(p.speed, 24.0);
        assert!((p.density - 1.35).abs() < f32::EPSILON);
        assert_eq!(p.glitch_level, GlitchLevel::Intense);
    }

    #[test]
    fn preset_values_calm() {
        let p = get_preset("calm").unwrap();
        assert_eq!(p.color, "ocean");
        assert_eq!(p.charset, "minimal");
        assert_eq!(p.fps, 60.0);
        assert_eq!(p.speed, 5.0);
        assert!((p.density - 0.65).abs() < f32::EPSILON);
        assert_eq!(p.glitch_level, GlitchLevel::Subtle);
    }

    #[test]
    fn preset_values_monolith() {
        let p = get_preset("monolith").unwrap();
        assert_eq!(p.color, "cosmos");
        assert_eq!(p.charset, "binary");
        assert_eq!(p.fps, 60.0);
        assert_eq!(p.speed, 4.0);
        assert!((p.density - 0.75).abs() < f32::EPSILON);
        assert_eq!(p.glitch_level, GlitchLevel::Subtle);
    }

    #[test]
    fn preset_values_cinematic() {
        let p = get_preset("cinematic").unwrap();
        assert_eq!(p.color, "cosmos");
        assert_eq!(p.charset, "binary");
        assert_eq!(p.fps, 60.0);
        assert_eq!(p.speed, 8.0);
        assert_eq!(p.density, 1.0);
        assert_eq!(p.glitch_level, GlitchLevel::Default);
    }

    #[test]
    fn preset_values_cosmos() {
        let p = get_preset("cosmos").unwrap();
        assert_eq!(p.color, "cosmos");
        assert_eq!(p.charset, "binary");
        assert_eq!(p.fps, 60.0);
        assert_eq!(p.speed, 9.0);
        assert!((p.density - 1.05).abs() < f32::EPSILON);
        assert_eq!(p.glitch_level, GlitchLevel::Default);
    }

    #[test]
    fn preset_values_neon() {
        let p = get_preset("neon").unwrap();
        assert_eq!(p.color, "neon");
        assert_eq!(p.charset, "cyberpunk");
        assert_eq!(p.fps, 60.0);
        assert_eq!(p.speed, 10.0);
        assert!((p.density - 1.1).abs() < f32::EPSILON);
        assert_eq!(p.glitch_level, GlitchLevel::Default);
    }

    #[test]
    fn preset_values_hacker() {
        let p = get_preset("hacker").unwrap();
        assert_eq!(p.color, "green");
        assert_eq!(p.charset, "hacker");
        assert_eq!(p.fps, 60.0);
        assert_eq!(p.speed, 11.0);
        assert!((p.density - 1.2).abs() < f32::EPSILON);
        assert_eq!(p.glitch_level, GlitchLevel::Default);
    }

    #[test]
    fn preset_values_low_power() {
        let p = get_preset("low-power").unwrap();
        assert_eq!(p.color, "green");
        assert_eq!(p.charset, "binary");
        assert_eq!(p.fps, 30.0, "low-power must cap fps at 30");
        assert_eq!(p.speed, 5.0, "low-power must set speed to 5");
        assert!((p.density - 0.5).abs() < f32::EPSILON, "low-power must set density to 0.5");
        assert_eq!(p.glitch_level, GlitchLevel::Default);
    }
}
