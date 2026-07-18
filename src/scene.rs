// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Scene catalog and scene-to-runtime mappings.
//!
//! Scenes map curated runtime knobs and internal rain style selection while
//! preserving palette override behavior.
//!
//! ## Catalog
//!
//! Built-in scenes combine the three core runtime atmospheres (`matrix`,
//! `monolith`, `signal`) with eight curated visual scenes (`classic`,
//! `cinematic`, `calm`, `storm`, `cosmos`, `neon`, `hacker`, `low-power`).
//! The interactive cycle (`SCENE_ORDER`) keeps the three original entries
//! to preserve runtime cycling behavior.

use crate::config::GlitchLevel;
use crate::rain_style::RainStyle;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneConfig {
    pub color: Option<&'static str>,
    pub charset: Option<&'static str>,
    pub fps: Option<f64>,
    pub speed: Option<f32>,
    pub density: Option<f32>,
    pub glitch_level: Option<GlitchLevel>,
    pub rain_style: RainStyle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub config: SceneConfig,
}

pub const DEFAULT_SCENE: &str = "monolith";

/// Ordered scene cycle: monolith -> matrix -> signal -> monolith.
pub const SCENE_ORDER: &[&str] = &["monolith", "matrix", "signal"];

pub const SCENES: &[SceneInfo] = &[
    // --- Original runtime scenes (interactive cycle entries) ---
    SceneInfo {
        name: "matrix",
        description: "Classic Matrix glyph rain behavior",
        config: SceneConfig {
            color: None,
            charset: None,
            fps: None,
            speed: None,
            density: None,
            glitch_level: None,
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "monolith",
        description: "Signature structured segmented rain with premium pacing",
        config: SceneConfig {
            color: Some("cosmos"),
            charset: Some("binary"),
            fps: Some(60.0),
            speed: Some(30.0),
            density: Some(0.85),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Monolith,
        },
    },
    SceneInfo {
        name: "signal",
        description: "Digital transmission feel with code-like cyan rain",
        config: SceneConfig {
            color: Some("aurora"),
            charset: Some("retro"),
            fps: Some(60.0),
            speed: Some(10.0),
            density: Some(0.95),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Glyph,
        },
    },
    // --- Curated visual scenes ---
    SceneInfo {
        name: "classic",
        description: "The original green-on-black Matrix rain",
        config: SceneConfig {
            color: Some("green"),
            charset: Some("matrix"),
            fps: Some(60.0),
            speed: Some(8.0),
            density: Some(1.0),
            glitch_level: Some(GlitchLevel::Default),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "cinematic",
        description: "Cosmic binary with cinematic feel",
        config: SceneConfig {
            color: Some("cosmos"),
            charset: Some("binary"),
            fps: Some(60.0),
            speed: Some(8.0),
            density: Some(1.0),
            glitch_level: Some(GlitchLevel::Default),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "calm",
        description: "Gentle ocean tones with reduced density",
        config: SceneConfig {
            color: Some("ocean"),
            charset: Some("minimal"),
            fps: Some(60.0),
            speed: Some(5.0),
            density: Some(0.65),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "storm",
        description: "Fast and intense purple cyberpunk",
        config: SceneConfig {
            color: Some("purple"),
            charset: Some("cyberpunk"),
            fps: Some(120.0),
            speed: Some(24.0),
            density: Some(1.35),
            glitch_level: Some(GlitchLevel::Intense),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "cosmos",
        description: "Cosmic binary with rich cosmos palette",
        config: SceneConfig {
            color: Some("cosmos"),
            charset: Some("binary"),
            fps: Some(60.0),
            speed: Some(9.0),
            density: Some(1.05),
            glitch_level: Some(GlitchLevel::Default),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "neon",
        description: "Vibrant cyberpunk with neon colors",
        config: SceneConfig {
            color: Some("neon"),
            charset: Some("cyberpunk"),
            fps: Some(60.0),
            speed: Some(10.0),
            density: Some(1.1),
            glitch_level: Some(GlitchLevel::Default),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "hacker",
        description: "Green hacker aesthetic at high speed",
        config: SceneConfig {
            color: Some("green"),
            charset: Some("hacker"),
            fps: Some(60.0),
            speed: Some(11.0),
            density: Some(1.2),
            glitch_level: Some(GlitchLevel::Default),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "low-power",
        description: "Power-saving mode (30 FPS, reduced density/speed)",
        config: SceneConfig {
            color: Some("green"),
            charset: Some("binary"),
            fps: Some(30.0),
            speed: Some(5.0),
            density: Some(0.5),
            glitch_level: Some(GlitchLevel::Default),
            rain_style: RainStyle::Glyph,
        },
    },
];

#[must_use]
pub fn all_scene_names() -> &'static [&'static str] {
    &[
        "calm",
        "cinematic",
        "classic",
        "cosmos",
        "hacker",
        "low-power",
        "matrix",
        "monolith",
        "neon",
        "signal",
        "storm",
    ]
}

/// Cycle to the next or previous scene in the ordered cycle.
/// Returns the next scene name.
/// Forward:  monolith -> matrix -> signal -> monolith
/// Backward: monolith -> signal -> matrix -> monolith
#[must_use]
pub fn cycle_scene(current: &str, dir: i32) -> &'static str {
    let Some(pos) = SCENE_ORDER.iter().position(|&n| n == current) else {
        return DEFAULT_SCENE;
    };
    let n = SCENE_ORDER.len() as i32;
    let mut idx = pos as i32 + dir;
    idx = ((idx % n) + n) % n;
    SCENE_ORDER[idx as usize]
}

#[must_use]
pub fn get_scene(name: &str) -> Option<&'static SceneInfo> {
    let normalized = name.trim().to_ascii_lowercase();
    SCENES.iter().find(|scene| scene.name == normalized)
}

#[must_use]
pub fn rain_style_for_scene(name: &str) -> Option<RainStyle> {
    get_scene(name).map(|scene| scene.config.rain_style)
}

pub fn validate_scene_name(name: &str) -> Result<String, String> {
    let normalized = name.trim().to_ascii_lowercase();
    if get_scene(&normalized).is_some() {
        Ok(normalized)
    } else {
        Err(format!(
            "error: unknown scene '{name}'\n\n  Use --list-scenes to see available scenes."
        ))
    }
}

#[must_use]
pub fn list_scenes_text() -> String {
    let mut out = String::new();
    for scene in SCENES {
        out.push_str(&format!("  {:10} {}\n", scene.name, scene.description));
    }
    out
}

/// Render a detailed, human-readable description of a single built-in scene.
///
/// Output is intended for `--show-scene <name>` when `<name>` matches a
/// built-in scene. Each field line is only printed when the scene actually
/// sets that field (i.e. it is `Some(_)`), so partial scenes do not show
/// misleading "default" placeholders.
#[must_use]
pub fn show_scene_text(info: &SceneInfo) -> String {
    let mut out = String::new();
    out.push_str(&format!("SCENE: {}\n\n", info.name));
    out.push_str(&format!("  Description: {}\n\n", info.description));
    out.push_str("  Configuration:\n");

    let cfg = info.config;
    if let Some(color) = cfg.color {
        out.push_str(&format!("    color        = {color}\n"));
    }
    if let Some(charset) = cfg.charset {
        out.push_str(&format!("    charset      = {charset}\n"));
    }
    if let Some(fps) = cfg.fps {
        out.push_str(&format!("    fps          = {fps}\n"));
    }
    if let Some(speed) = cfg.speed {
        out.push_str(&format!("    speed        = {speed}\n"));
    }
    if let Some(density) = cfg.density {
        out.push_str(&format!("    density      = {density}\n"));
    }
    if let Some(glitch) = cfg.glitch_level {
        out.push_str(&format!("    glitch-level = {}\n", glitch_label(glitch)));
    }
    // rain_style is always set (it's not an Option), so always show it.
    out.push_str(&format!("    rain-style   = {}\n", cfg.rain_style.as_str()));

    out.push_str("\n  Use: cosmostrix --scene ");
    out.push_str(info.name);
    out.push('\n');
    out
}

/// Map a `GlitchLevel` to its lowercase CLI string label.
fn glitch_label(level: crate::config::GlitchLevel) -> &'static str {
    use crate::config::GlitchLevel;
    match level {
        GlitchLevel::None => "none",
        GlitchLevel::Subtle => "subtle",
        GlitchLevel::Default => "default",
        GlitchLevel::Intense => "intense",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_scene_forward_order() {
        assert_eq!(cycle_scene("monolith", 1), "matrix");
        assert_eq!(cycle_scene("matrix", 1), "signal");
        assert_eq!(cycle_scene("signal", 1), "monolith");
    }

    #[test]
    fn cycle_scene_backward_order() {
        assert_eq!(cycle_scene("monolith", -1), "signal");
        assert_eq!(cycle_scene("signal", -1), "matrix");
        assert_eq!(cycle_scene("matrix", -1), "monolith");
    }

    #[test]
    fn cycle_scene_unknown_returns_default() {
        assert_eq!(cycle_scene("nonexistent", 1), DEFAULT_SCENE);
        assert_eq!(cycle_scene("nonexistent", -1), DEFAULT_SCENE);
    }

    #[test]
    fn cycle_scene_wraps_around() {
        // Double forward from monolith
        assert_eq!(cycle_scene(cycle_scene("monolith", 1), 1), "signal");
        // Double backward from monolith
        assert_eq!(cycle_scene(cycle_scene("monolith", -1), -1), "matrix");
    }

    #[test]
    fn scene_names_are_present() {
        assert_eq!(DEFAULT_SCENE, "monolith");
        assert_eq!(
            all_scene_names(),
            &[
                "calm",
                "cinematic",
                "classic",
                "cosmos",
                "hacker",
                "low-power",
                "matrix",
                "monolith",
                "neon",
                "signal",
                "storm",
            ]
        );
        for name in all_scene_names() {
            assert!(get_scene(name).is_some(), "missing scene {name}");
        }
    }

    #[test]
    fn scene_catalog_has_eleven_entries() {
        assert_eq!(SCENES.len(), 11, "catalog must contain 11 built-in scenes");
    }

    #[test]
    fn scene_cycle_order_is_preserved() {
        // SCENE_ORDER stays three-entry to keep interactive cycling stable.
        assert_eq!(SCENE_ORDER, &["monolith", "matrix", "signal"]);
    }

    #[test]
    fn classic_scene_uses_matrix_scene_values() {
        let s = get_scene("classic").expect("classic scene");
        assert_eq!(s.config.color, Some("green"));
        assert_eq!(s.config.charset, Some("matrix"));
        assert_eq!(s.config.fps, Some(60.0));
        assert_eq!(s.config.speed, Some(8.0));
        assert_eq!(s.config.density, Some(1.0));
        assert_eq!(s.config.glitch_level, Some(GlitchLevel::Default));
        assert_eq!(s.config.rain_style, RainStyle::Glyph);
    }

    #[test]
    fn storm_scene_uses_intense_cyberpunk_values() {
        let s = get_scene("storm").expect("storm scene");
        assert_eq!(s.config.color, Some("purple"));
        assert_eq!(s.config.charset, Some("cyberpunk"));
        assert_eq!(s.config.fps, Some(120.0));
        assert_eq!(s.config.speed, Some(24.0));
        assert_eq!(s.config.density, Some(1.35));
        assert_eq!(s.config.glitch_level, Some(GlitchLevel::Intense));
    }

    #[test]
    fn low_power_scene_caps_fps_at_thirty() {
        let s = get_scene("low-power").expect("low-power scene");
        assert_eq!(s.config.fps, Some(30.0));
        assert_eq!(s.config.speed, Some(5.0));
        assert_eq!(s.config.density, Some(0.5));
        assert_eq!(s.config.color, Some("green"));
        assert_eq!(s.config.charset, Some("binary"));
    }

    #[test]
    fn hacker_scene_uses_hacker_charset() {
        let s = get_scene("hacker").expect("hacker scene");
        assert_eq!(s.config.charset, Some("hacker"));
        assert_eq!(s.config.speed, Some(11.0));
        assert_eq!(s.config.density, Some(1.2));
    }

    #[test]
    fn calm_scene_uses_ocean_and_subtle_glitch() {
        let s = get_scene("calm").expect("calm scene");
        assert_eq!(s.config.color, Some("ocean"));
        assert_eq!(s.config.charset, Some("minimal"));
        assert_eq!(s.config.glitch_level, Some(GlitchLevel::Subtle));
    }

    #[test]
    fn scenes_select_expected_rain_style() {
        assert_eq!(rain_style_for_scene("matrix"), Some(RainStyle::Glyph));
        assert_eq!(rain_style_for_scene("signal"), Some(RainStyle::Glyph));
        assert_eq!(rain_style_for_scene("monolith"), Some(RainStyle::Monolith));
    }

    #[test]
    fn monolith_scene_uses_premium_motion_defaults() {
        let monolith = get_scene("monolith").expect("monolith scene");
        assert_eq!(monolith.config.speed, Some(30.0));
        assert_eq!(monolith.config.density, Some(0.85));
    }

    #[test]
    fn scene_parser_is_case_insensitive() {
        assert_eq!(validate_scene_name("SIGNAL").unwrap(), "signal");
        assert_eq!(validate_scene_name(" Monolith ").unwrap(), "monolith");
    }

    #[test]
    fn invalid_scene_error_mentions_discovery() {
        let err = validate_scene_name("nonexistent").unwrap_err();
        assert!(
            err.contains("error: unknown scene"),
            "scene error must use 'unknown' terminology: {err}"
        );
        assert!(
            err.contains("--list-scenes"),
            "scene error must reference --list-scenes: {err}"
        );
    }

    #[test]
    fn list_scenes_output_includes_all_scenes() {
        let text = list_scenes_text();
        for name in all_scene_names() {
            assert!(text.contains(name), "missing scene {name}");
        }
        assert!(
            text.contains("low-power"),
            "list must include low-power scene"
        );
        assert!(text.contains("storm"), "list must include storm scene");
    }

    #[test]
    fn show_scene_text_includes_header_and_usage() {
        let info = get_scene("storm").expect("storm scene");
        let text = show_scene_text(info);
        assert!(text.starts_with("SCENE: storm"), "header missing: {text}");
        assert!(
            text.contains("Description:"),
            "description label missing: {text}"
        );
        assert!(
            text.contains("Configuration:"),
            "config label missing: {text}"
        );
        assert!(
            text.contains("color        = purple"),
            "color field missing: {text}"
        );
        assert!(
            text.contains("fps          = 120"),
            "fps field missing: {text}"
        );
        assert!(
            text.contains("rain-style   = glyph"),
            "rain-style missing: {text}"
        );
        assert!(
            text.contains("cosmostrix --scene storm"),
            "usage hint missing: {text}"
        );
    }

    #[test]
    fn show_scene_text_handles_partial_scene() {
        // matrix scene has all fields set to None except rain_style.
        let info = get_scene("matrix").expect("matrix scene");
        let text = show_scene_text(info);
        assert!(text.contains("SCENE: matrix"), "header missing: {text}");
        assert!(
            text.contains("rain-style   = glyph"),
            "rain-style missing: {text}"
        );
        // Should not contain field labels for unset fields
        assert!(
            !text.contains("color        ="),
            "color should be absent: {text}"
        );
        assert!(
            !text.contains("fps          ="),
            "fps should be absent: {text}"
        );
    }
}
