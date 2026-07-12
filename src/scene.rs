// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Scene catalog and scene-to-runtime mappings.
//!
//! Scenes map curated runtime knobs and internal rain style selection while
//! preserving palette override behavior.

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
];

#[must_use]
#[cfg(test)]
pub fn all_scene_names() -> &'static [&'static str] {
    &["matrix", "monolith", "signal"]
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
        assert_eq!(all_scene_names(), &["matrix", "monolith", "signal"]);
        for name in all_scene_names() {
            assert!(get_scene(name).is_some(), "missing scene {name}");
        }
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
    }
}
