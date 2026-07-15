// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Configuration file support for Cosmostrix.
//!
//! Reads an explicit `--config <PATH>` file or the default
//! `~/.config/cosmostrix/config` (or `$XDG_CONFIG_HOME/cosmostrix/config`).
//!
//! ## Philosophy
//!
//! The config file exposes daily-driver settings and a small compatibility set
//! of legacy advanced keys. It stays intentionally flat and predictable.
//!
//! ## Format
//!
//! ```text
//! key = value          # one per line
//! # comments           # blank lines ignored
//! ```
//!
//! Config file values serve as defaults; presets and explicit CLI args are
//! applied later by `config_apply`.

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use crate::constants::{CONFIG_DIR_NAME, CONFIG_FILE_NAME, CONFIG_FILE_NAME_LEGACY};
use crate::profile::is_profile_config_key;
use crate::scene_custom::is_scene_custom_config_key;

pub const USER_CONFIG_KEYS: &[&str] = &[
    "scene",
    "preset",
    "profile",
    "scene-custom",
    "color",
    "charset",
    "fps",
    "speed",
    "density",
    "monolith-size",
    "glitch-level",
    "bold",
    "shadingmode",
    "color-bg",
    "low-power",
    "mouse",
    "fullwidth",
    "auto-color-drift",
    "async-mode",
    "atmosphere-mode",
    "atmosphere-regime",
];

pub const LEGACY_CONFIG_KEYS: &[&str] = &["glitchpct", "shortpct", "rippct", "maxdpc"];

const PROFILE_CONFIG_KEY_HINT: &str = "profile.<name>.<base|scene|preset|color|charset|fps|speed|density|glitch-level|monolith-size|color-bg|atmosphere-mode|atmosphere-regime>";
const SCENE_CUSTOM_CONFIG_KEY_HINT: &str = "scene-custom.<name>.<base|scene|preset|color|charset|fps|speed|density|glitch-level|monolith-size|color-bg|atmosphere-mode|atmosphere-regime>";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ParsedConfig {
    pub values: HashMap<String, String>,
    pub unknown_keys: Vec<String>,
    /// Non-empty, non-comment lines that do not match `key = value` syntax.
    ///
    /// Tracked so `--testconf` can report them as errors and `load_config_file`
    /// can warn on stderr. A line lands here when it has no `=` at all, or when
    /// either side of `=` is empty after trimming.
    pub malformed_lines: Vec<String>,
}

/// Load config file and return a HashMap of key → value pairs.
/// Returns empty HashMap if file doesn't exist or can't be read.
/// Warns on stderr for unrecognized keys (likely typos).
///
/// Search order when no explicit path is given:
/// 1. `$XDG_CONFIG_HOME/cosmostrix/config.toml` (or `~/.config/cosmostrix/config.toml`)
/// 2. Legacy `config` filename (pre-v10 backward compat)
/// 3. `/etc/cosmostrix/config.toml` (system-wide default, installed by AUR/package manager)
///
/// This means AUR users get a working default config out of the box —
/// the package installs `/etc/cosmostrix/config.toml`, and cosmostrix
/// reads it automatically if no user-level config exists.
#[must_use]
pub fn load_config_file(path_override: Option<&Path>) -> HashMap<String, String> {
    let path = path_override
        .map(Path::to_path_buf)
        .unwrap_or_else(default_config_file_path);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            // Fallback: try system-wide config at /etc/cosmostrix/config.toml.
            // This is installed by AUR/PKGBUILD and other package managers.
            // Only used when no user-level config exists and no explicit
            // --config path was given.
            if path_override.is_none() {
                let system_path = PathBuf::from("/etc/cosmostrix/config.toml");
                if let Ok(sys_content) = std::fs::read_to_string(&system_path) {
                    sys_content
                } else {
                    return HashMap::new();
                }
            } else {
                return HashMap::new();
            }
        }
    };

    let parsed = parse_config_text(&content);
    // No warnings printed here — startup validation (config_apply.rs) and
    // live-reload (live_config.rs) handle malformed_lines + unknown_keys
    // with strict errors. Printing warnings here caused duplicate output.
    parsed.values
}

#[must_use]
pub fn parse_config_text(content: &str) -> ParsedConfig {
    let mut map = HashMap::new();
    let mut unknown_keys = Vec::new();
    let mut malformed_lines = Vec::new();

    for line in content.lines() {
        let stripped = strip_inline_comment(line).trim();
        // Skip comments and blank lines
        if stripped.is_empty() {
            continue;
        }
        // Parse key = value
        if let Some((key, value)) = stripped.split_once('=') {
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if key.is_empty() || value.is_empty() {
                // Malformed: `= value` (no key) or `key =` (no value).
                malformed_lines.push(stripped.to_string());
                continue;
            }
            if !is_known_key(&key) {
                unknown_keys.push(key);
                continue;
            }
            map.insert(key, value);
        } else {
            // No `=` at all on a non-empty, non-comment line — malformed.
            malformed_lines.push(stripped.to_string());
        }
    }

    ParsedConfig {
        values: map,
        unknown_keys,
        malformed_lines,
    }
}

/// Returns the path to the config file.
/// Uses `$XDG_CONFIG_HOME` if set, otherwise `~/.config`.
///
/// Looks for `config.toml` first (v10+), falls back to `config` (pre-v10)
/// for backward compatibility with users upgrading from older versions.
#[must_use]
pub fn default_config_file_path() -> PathBuf {
    let xdg = env::var("XDG_CONFIG_HOME").ok();
    let home = env::var("HOME").ok();
    let new_path = config_file_path_from_env(xdg.as_deref(), home.as_deref(), CONFIG_FILE_NAME);
    // Backward compat: if config.toml doesn't exist, check for legacy "config"
    if new_path.exists() {
        return new_path;
    }
    let legacy_path =
        config_file_path_from_env(xdg.as_deref(), home.as_deref(), CONFIG_FILE_NAME_LEGACY);
    if legacy_path.exists() {
        return legacy_path;
    }
    // Neither exists — return the new path (for --config-path display + --testconf)
    new_path
}

#[must_use]
#[allow(dead_code)]
pub fn config_file_path_from(xdg_config_home: Option<String>, home: Option<String>) -> PathBuf {
    config_file_path_from_env(
        xdg_config_home.as_deref(),
        home.as_deref(),
        CONFIG_FILE_NAME,
    )
}

fn config_file_path_from_env(
    xdg_config_home: Option<&str>,
    home: Option<&str>,
    file_name: &str,
) -> PathBuf {
    if let Some(xdg) = xdg_config_home.filter(|v| !v.is_empty()) {
        PathBuf::from(xdg).join(CONFIG_DIR_NAME).join(file_name)
    } else if let Some(home) = home.filter(|v| !v.is_empty()) {
        PathBuf::from(home)
            .join(".config")
            .join(CONFIG_DIR_NAME)
            .join(file_name)
    } else {
        PathBuf::from(".config")
            .join(CONFIG_DIR_NAME)
            .join(file_name)
    }
}

#[must_use]
pub fn dump_config_text() -> &'static str {
    r#"# Cosmostrix Configuration

# Location:
#   Linux:   ~/.config/cosmostrix/config.toml
#   macOS:   ~/.config/cosmostrix/config.toml
#            (or ~/Library/Application Support/cosmostrix/config.toml)
#   Windows: %APPDATA%\cosmostrix\config.toml
#   System-wide: /etc/cosmostrix/config.toml (Linux/macOS)
#                %ProgramData%\cosmostrix\config.toml (Windows)
#   Or set $XDG_CONFIG_HOME (Linux/macOS).
#
# Format:
#   key = value          # one per line
#   # comments           # blank lines ignored
#   Custom scene blocks use scene-custom.<name>.<field> = <value> syntax.
#   Malformed lines (no '=' or empty key/value) cause --testconf to FAIL.
#
# Precedence (highest wins):
#   built-in defaults
#   < scene defaults (fills unset keys only)
#   < config values (always wins over scene defaults for user-set keys)
#   < config scene-custom
#   < CLI scene
#   < CLI scene-custom
#   < explicit CLI flags
#
# Key rule: a value set in config.toml ALWAYS wins over a scene's
# hardcoded default. Scenes only fill keys the user did NOT set.
# This prevents surprises like `speed = 30` in config being silently
# overwritten by a scene's `speed = 8`.
#
# All keys below are commented out. Uncomment the ones you want to
# customize — cosmostrix's built-in defaults (shown for reference)
# will be used for any key left commented. Run `cosmostrix --testconf`
# to validate your config after editing.

# Scene
# Built-in atmospheric template bundling sensible defaults.
#   monolith   — premium motion, cosmos palette, binary glyphs (default)
#   matrix     — classic green Matrix rain
#   signal     — aurora palette, retro glyphs, slow & dense
#   classic    — original green-on-black Matrix rain
#   cinematic  — cosmic binary with cinematic feel
#   calm       — gentle ocean tones, reduced density
#   storm      — fast and intense purple cyberpunk
#   cosmos     — cosmic binary, rich cosmos palette
#   neon       — vibrant cyberpunk with neon colors
#   hacker     — green hacker aesthetic, high speed
#   low-power  — power-saving (30 FPS, reduced density/speed)
# See: cosmostrix --list-scenes
# scene = monolith

# Custom Scene (optional)
# User-defined custom scene to apply by default. Custom scenes use the
# scene-custom.<name>.<field> = <value> syntax (see bottom of this file).
# See: cosmostrix --list-scenes (lists built-in and custom scenes together)
# scene-custom = nightcore

# Appearance
# Color scheme (palette). See: cosmostrix --list-colors
# color = cosmos

# Character set for rain glyphs. See: cosmostrix --list-charsets
# Custom characters from file (CLI only, overrides charset):
#   cosmostrix --charset-file ~/.config/cosmostrix/my-chars.txt
# charset = binary

# Background mode:
#   default-background — follow terminal emulator bg (default; saves ANSI bytes)
#   black              — force solid #000000 behind rain
# color-bg = default-background

# Motion
# Target frames-per-second. Adaptive pacing may reduce under load.
# fps = 60

# Rain fall speed (1–100). Higher = faster rain.
# speed = 30

# Rain density (0.01–5.0). Higher = more columns active.
# density = 0.85

# Variable column speeds for organic rain (default: on).
# Each column gets a random speed multiplier (33%-100% of base).
# Disable with: cosmostrix --uniform
# async-mode = true

# Monolith
# Pillar size (only applies when scene=monolith or rain_style=monolith):
#   small | normal (default) | large
# monolith-size = normal

# Behavior
# Glitch intensity: none | subtle | default | intense
# glitch-level = subtle

# Mouse capture for interactive controls (default: off).
# mouse = false

# Full-width CJK glyph rendering (default: off).
# fullwidth = false

# Auto color drift: cycle color scheme over time (default: off).
# auto-color-drift = false

# Advanced Style
# Bold style: 0=off, 1=random, 2=all
# bold = 1

# Shading mode: 0=random, 1=cinematic (distance-from-head brightness)
# shadingmode = 1

# Atmosphere Engine (opt-in only)
# atmosphere-mode: disabled (default) | controlled-live
# atmosphere-regime: calm | pulse | signal | compression | void | monolith-pressure | adaptive
#   adaptive — time-driven: modulates rain based on local hour (v14).
#              Deep Void at midnight, Pulse in the morning, Calm at noon,
#              Signal at dusk. Smoothstep transitions, no jumps.
# Note: storm is unavailable and will be rejected.
# These keys are opt-in; setting atmosphere-mode without controlled-live has no effect.
# atmosphere-mode = disabled
# atmosphere-regime = calm

# Controlled atmosphere example (opt-in only):
# atmosphere-mode = controlled-live
# atmosphere-regime = adaptive
# See docs/ATMOSPHERE_PRESETS.md for all 6 preset examples.

# Legacy Advanced Keys (kept for compatibility)
# Prefer glitch-level for normal use.
# glitchpct = 10
# shortpct = 50
# rippct = 33.33333
# maxdpc = 3

# Custom Scene Definitions
# Define named custom scenes and load with: cosmostrix --scene-custom <name>
# Fields: base, scene, preset, color, charset, fps, speed, density,
#         density-map, glitch-level, monolith-size, color-bg,
#         atmosphere-mode, atmosphere-regime
# Custom scenes are listed alongside built-in scenes in --list-scenes output.
# See docs/PROFILE_EXAMPLES.md for more examples.
# scene-custom.hacker-mode.base = storm
# scene-custom.hacker-mode.color = green
# scene-custom.hacker-mode.charset = hacker
# scene-custom.hacker-mode.speed = 24
# scene-custom.hacker-mode.density = 1.2
# scene-custom.hacker-mode.glitch-level = intense

# Density Map: sculpt monolith pillar formation per-column.
# Comma-separated weights (0.0..1.0). 0.0 = never spawn, 1.0 = always spawn.
# Maps shorter than terminal width treat missing columns as 1.0.
#
# Three cinematic presets (120 columns each) — uncomment to use:
#
# Twin Towers
# Two dense pillar clusters at cols 20-35 and 85-100, sparse canyon between.
# Symmetric, majestic — like twin monuments guarding a silent valley.
# scene-custom.twin-towers.base = monolith
# scene-custom.twin-towers.density-map = 0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.7,0.7,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,0.7,0.7,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.7,0.7,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,0.7,0.7,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08
#
# Cascade
# Smooth linear gradient: dense on left (col 0), sparse on right (col 119).
# Creates a waterfall effect — rain pours from one side and fades into mist.
# scene-custom.cascade.base = monolith
# scene-custom.cascade.density-map = 1.0,0.992,0.984,0.976,0.968,0.96,0.952,0.944,0.936,0.928,0.92,0.912,0.904,0.896,0.888,0.88,0.872,0.864,0.856,0.848,0.84,0.832,0.824,0.816,0.808,0.8,0.792,0.784,0.776,0.768,0.761,0.753,0.745,0.737,0.729,0.721,0.713,0.705,0.697,0.689,0.681,0.673,0.665,0.657,0.649,0.641,0.633,0.625,0.617,0.609,0.601,0.593,0.585,0.577,0.569,0.561,0.553,0.545,0.537,0.529,0.521,0.513,0.505,0.497,0.489,0.481,0.473,0.465,0.457,0.449,0.441,0.433,0.425,0.417,0.409,0.401,0.393,0.385,0.377,0.369,0.361,0.353,0.345,0.337,0.329,0.321,0.313,0.305,0.297,0.289,0.282,0.274,0.266,0.258,0.25,0.242,0.234,0.226,0.218,0.21,0.202,0.194,0.186,0.178,0.17,0.162,0.154,0.146,0.138,0.13,0.122,0.114,0.106,0.098,0.09,0.082,0.074,0.066,0.058,0.05
#
# Throne
# One massive pillar at center (cols 50-70), ringed by sparse court and wilderness.
# All eyes drawn to the middle — the seat of power in a quiet kingdom.
# scene-custom.throne.base = monolith
# scene-custom.throne.density-map = 0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.3,0.3,0.3,0.3,0.3,0.8,0.8,0.8,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,0.8,0.8,0.8,0.3,0.3,0.3,0.3,0.3,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05

# Quick Start
# cosmostrix                                       # run with defaults
# cosmostrix --scene storm                         # built-in scene
# cosmostrix --scene-custom hacker-mode            # user-defined custom scene
# cosmostrix --list-scenes                         # list all scenes
# cosmostrix --show-scene hacker-mode              # preview a scene
# cosmostrix --testconf                            # validate this config
# cosmostrix --doctor                              # diagnose terminal issues
"#
}

#[must_use]
pub fn known_keys() -> Vec<&'static str> {
    USER_CONFIG_KEYS
        .iter()
        .chain(LEGACY_CONFIG_KEYS.iter())
        .chain(std::iter::once(&PROFILE_CONFIG_KEY_HINT))
        .chain(std::iter::once(&SCENE_CUSTOM_CONFIG_KEY_HINT))
        .copied()
        .collect()
}

#[inline]
fn is_known_key(key: &str) -> bool {
    USER_CONFIG_KEYS.contains(&key)
        || LEGACY_CONFIG_KEYS.contains(&key)
        || is_profile_config_key(key)
        || is_scene_custom_config_key(key)
}

#[inline]
fn strip_inline_comment(line: &str) -> &str {
    line.split_once('#').map_or(line, |(before, _)| before)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_prefers_xdg_config_home() {
        let path =
            config_file_path_from(Some("/tmp/xdg".to_string()), Some("/tmp/home".to_string()));
        assert_eq!(path, PathBuf::from("/tmp/xdg/cosmostrix/config.toml"));
    }

    #[test]
    fn default_path_falls_back_to_home_config() {
        let path = config_file_path_from(None, Some("/tmp/home".to_string()));
        assert_eq!(
            path,
            PathBuf::from("/tmp/home/.config/cosmostrix/config.toml")
        );
    }

    #[test]
    fn parse_key_value_lines() {
        let parsed = parse_config_text("color = ocean\nfps = 60\n");
        assert_eq!(
            parsed.values.get("color").map(String::as_str),
            Some("ocean")
        );
        assert_eq!(parsed.values.get("fps").map(String::as_str), Some("60"));
        assert!(parsed.unknown_keys.is_empty());
    }

    #[test]
    fn parse_ignores_comments_blank_lines_and_inline_comments() {
        let parsed =
            parse_config_text("\n# comment\ncharset = minimal # trailing comment\n\nspeed = 5\n");
        assert_eq!(
            parsed.values.get("charset").map(String::as_str),
            Some("minimal")
        );
        assert_eq!(parsed.values.get("speed").map(String::as_str), Some("5"));
        assert_eq!(parsed.values.len(), 2);
    }

    #[test]
    fn parse_unknown_keys_are_reported_and_ignored() {
        let parsed = parse_config_text("color = ocean\ncolro = typo\n");
        assert_eq!(
            parsed.values.get("color").map(String::as_str),
            Some("ocean")
        );
        assert_eq!(parsed.unknown_keys, vec!["colro"]);
        assert!(!parsed.values.contains_key("colro"));
    }

    #[test]
    fn legacy_keys_are_known() {
        let parsed = parse_config_text("glitchpct = 3\nshortpct = 60\nrippct = 45\nmaxdpc = 2\n");
        assert_eq!(parsed.values.len(), 4);
        assert!(parsed.unknown_keys.is_empty());
    }

    #[test]
    fn profile_keys_are_known() {
        let parsed = parse_config_text(
            "profile.nightcore.base = monolith\nprofile.nightcore.color = purple\n",
        );
        assert_eq!(
            parsed
                .values
                .get("profile.nightcore.base")
                .map(String::as_str),
            Some("monolith")
        );
        assert!(parsed.unknown_keys.is_empty());
        assert!(parsed.malformed_lines.is_empty());
    }

    #[test]
    fn malformed_lines_without_equals_are_collected() {
        // Lines with no '=' on a non-empty, non-comment line are malformed.
        let parsed = parse_config_text("color = ocean\necho here should error\n");
        assert_eq!(parsed.values.len(), 1);
        assert_eq!(parsed.malformed_lines, vec!["echo here should error"]);
    }

    #[test]
    fn malformed_lines_with_empty_value_are_collected() {
        // `key =` (no value) is malformed.
        let parsed = parse_config_text("color = ocean\nspeed =\n");
        assert_eq!(parsed.values.len(), 1);
        assert_eq!(parsed.malformed_lines, vec!["speed ="]);
    }

    #[test]
    fn malformed_lines_with_empty_key_are_collected() {
        // `= value` (no key) is malformed.
        let parsed = parse_config_text("color = ocean\n= 60\n");
        assert_eq!(parsed.values.len(), 1);
        assert_eq!(parsed.malformed_lines, vec!["= 60"]);
    }

    #[test]
    fn malformed_lines_skip_comments_and_blanks() {
        // Comments and blank lines must NOT be flagged as malformed.
        let parsed =
            parse_config_text("# this is a comment\n\ncolor = ocean\n  # indented comment\n\n");
        assert_eq!(parsed.values.len(), 1);
        assert!(parsed.malformed_lines.is_empty());
    }

    #[test]
    fn malformed_lines_inline_comment_stripped() {
        // A malformed line with an inline comment should be reported without
        // the comment portion.
        let parsed = parse_config_text("echo bad line # this is a comment\n");
        assert_eq!(parsed.malformed_lines, vec!["echo bad line"]);
    }

    #[test]
    fn dump_config_contains_all_supported_keys() {
        let dump = dump_config_text();
        // Check non-deprecated keys are mentioned. Deprecated config aliases
        // (preset, profile, low-power) are still valid config keys but are
        // intentionally omitted from the dump template since v14.0.0.
        let deprecated = ["preset", "profile", "low-power"];
        for key in USER_CONFIG_KEYS.iter().chain(LEGACY_CONFIG_KEYS.iter()) {
            if deprecated.contains(key) {
                continue;
            }
            assert!(dump.contains(*key), "dump config should mention {key}");
        }
        assert!(dump.contains("scene-custom.hacker-mode.base"));
    }
}
