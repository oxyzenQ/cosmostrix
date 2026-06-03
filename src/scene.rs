// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Scene catalog and scene-to-runtime mappings.
//!
//! Scenes are a thin atmosphere selection layer over existing runtime knobs.
//! They do not introduce renderer internals or change palette behavior.

use crate::config::GlitchLevel;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneConfig {
    pub color: Option<&'static str>,
    pub charset: Option<&'static str>,
    pub fps: Option<f64>,
    pub speed: Option<f32>,
    pub density: Option<f32>,
    pub glitch_level: Option<GlitchLevel>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub config: SceneConfig,
}

pub const DEFAULT_SCENE: &str = "matrix";

pub const SCENES: &[SceneInfo] = &[
    SceneInfo {
        name: "matrix",
        description: "Default Matrix rain; preserves the classic v2 behavior",
        config: SceneConfig {
            color: None,
            charset: None,
            fps: None,
            speed: None,
            density: None,
            glitch_level: None,
        },
    },
    SceneInfo {
        name: "monolith",
        description: "Dark, calm, heavy atmosphere with premium pacing",
        config: SceneConfig {
            color: Some("blackhole"),
            charset: Some("binary"),
            fps: Some(60.0),
            speed: Some(4.0),
            density: Some(0.75),
            glitch_level: Some(GlitchLevel::Subtle),
        },
    },
    SceneInfo {
        name: "signal",
        description: "Digital transmission feel with code-like cyan rain",
        config: SceneConfig {
            color: Some("cyan"),
            charset: Some("code"),
            fps: Some(60.0),
            speed: Some(10.0),
            density: Some(0.95),
            glitch_level: Some(GlitchLevel::Subtle),
        },
    },
];

#[must_use]
#[cfg(test)]
pub fn all_scene_names() -> &'static [&'static str] {
    &["matrix", "monolith", "signal"]
}

#[must_use]
pub fn get_scene(name: &str) -> Option<&'static SceneInfo> {
    let normalized = name.trim().to_ascii_lowercase();
    SCENES.iter().find(|scene| scene.name == normalized)
}

pub fn validate_scene_name(name: &str) -> Result<String, String> {
    let normalized = name.trim().to_ascii_lowercase();
    if get_scene(&normalized).is_some() {
        Ok(normalized)
    } else {
        Err(format!("invalid scene: {} (see --list-scenes)", name))
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
    fn scene_names_are_present() {
        assert_eq!(all_scene_names(), &["matrix", "monolith", "signal"]);
        for name in all_scene_names() {
            assert!(get_scene(name).is_some(), "missing scene {name}");
        }
    }

    #[test]
    fn scene_parser_is_case_insensitive() {
        assert_eq!(validate_scene_name("SIGNAL").unwrap(), "signal");
        assert_eq!(validate_scene_name(" Monolith ").unwrap(), "monolith");
    }

    #[test]
    fn invalid_scene_error_mentions_discovery() {
        let err = validate_scene_name("nonexistent").unwrap_err();
        assert_eq!(err, "invalid scene: nonexistent (see --list-scenes)");
    }

    #[test]
    fn list_scenes_output_includes_all_scenes() {
        let text = list_scenes_text();
        for name in all_scene_names() {
            assert!(text.contains(name), "missing scene {name}");
        }
    }
}
