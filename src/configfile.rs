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

pub const USER_CONFIG_KEYS: &[&str] = &[
    "scene",
    "preset",
    "profile",
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

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ParsedConfig {
    pub values: HashMap<String, String>,
    pub unknown_keys: Vec<String>,
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
    for key in &parsed.unknown_keys {
        eprintln!(
            "warning: ignoring unknown key '{}' (known: {})",
            key,
            known_keys().join(", ")
        );
    }
    parsed.values
}

#[must_use]
pub fn parse_config_text(content: &str) -> ParsedConfig {
    let mut map = HashMap::new();
    let mut unknown_keys = Vec::new();

    for line in content.lines() {
        let line = strip_inline_comment(line).trim();
        // Skip comments and blank lines
        if line.is_empty() {
            continue;
        }
        // Parse key = value
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if !key.is_empty() && !value.is_empty() {
                if !is_known_key(&key) {
                    unknown_keys.push(key);
                    continue;
                }
                map.insert(key, value);
            }
        }
    }

    ParsedConfig {
        values: map,
        unknown_keys,
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
# ─────────────────────────────────────────────────────────────────────────────
# Location:
#   Linux/macOS: ~/.config/cosmostrix/config.toml
#   Windows:     %APPDATA%\cosmostrix\config.toml
#   Or set $XDG_CONFIG_HOME (Linux/macOS).
#
# Format:
#   key = value
#   Flat keys — no TOML sections needed for globals.
#   Profile blocks use profile.<name>.<field> = <value> syntax.
#   Invalid values warn cleanly and are ignored.
#
# Precedence (highest wins):
#   built-in defaults
#   < scene defaults (fills unset keys only)
#   < config values (always wins over scene defaults for user-set keys)
#   < config preset
#   < config profile
#   < CLI preset
#   < CLI scene
#   < CLI profile
#   < low-power
#   < explicit CLI flags
#
# Key rule: a value set in config.toml ALWAYS wins over a scene's
# hardcoded default. Scenes only fill keys the user did NOT set.
# This prevents surprises like `speed = 30` in config being silently
# overwritten by a scene's `speed = 8`.

# ── Scene ────────────────────────────────────────────────────────────────────
# Atmospheric template bundling sensible defaults.
#   monolith  — premium motion, cosmos palette, binary glyphs (default)
#   matrix    — classic green Matrix rain, katakana glyphs
#   signal    — aurora palette, retro glyphs, slow & dense
# See: cosmostrix --list-scenes
scene = monolith

# ── Preset (optional) ────────────────────────────────────────────────────────
# Curated visual preset applied on top of config values.
# See: cosmostrix --list-presets, cosmostrix --show-preset cinematic
# preset = cinematic

# ── Profile (optional) ───────────────────────────────────────────────────────
# User-defined profile to apply by default.
# See: cosmostrix --list-profiles
# profile = nightcore

# ── Appearance ───────────────────────────────────────────────────────────────
# Color scheme (palette). See: cosmostrix --list-colors
color = cosmos

# Character set for rain glyphs. See: cosmostrix --list-charsets
# Custom characters from file (CLI only, overrides charset):
#   cosmostrix --charset-file ~/my-chars.txt
charset = binary

# Background mode:
#   default-background — follow terminal emulator bg (default; saves ANSI bytes)
#   black              — force solid #000000 behind rain
color-bg = default-background

# ── Motion ───────────────────────────────────────────────────────────────────
fps = 60
speed = 20
density = 0.85

# Variable column speeds for organic rain (default: on).
# Each column gets a random speed multiplier (33%-100% of base).
# Despite the name, this is NOT Rust async/await — cosmostrix remains
# single-threaded. "async" = "asynchronous column pacing".
async-mode = true

# ── Monolith ─────────────────────────────────────────────────────────────────
# Pillar size (only applies when scene=monolith or rain_style=monolith):
#   small | normal (default) | large
monolith-size = normal

# ── Behavior ─────────────────────────────────────────────────────────────────
# Glitch intensity: none | subtle | default | intense
glitch-level = subtle
low-power = false
mouse = false
fullwidth = false
auto-color-drift = false

# ── Advanced Style ───────────────────────────────────────────────────────────
# Bold style: 0=off, 1=random, 2=all
bold = 1
# Shading mode: 0=random, 1=cinematic (distance-from-head brightness)
shadingmode = 1

# ── Atmosphere Engine (opt-in only) ──────────────────────────────────────────
# atmosphere-mode: disabled (default) | controlled-live
# atmosphere-regime: calm | pulse | signal | compression | void | monolith-pressure
# Note: storm is unavailable and will be rejected.
# These keys are opt-in; setting atmosphere-mode without controlled-live has no effect.
# atmosphere-mode = disabled
# atmosphere-regime = calm

# Controlled atmosphere example (opt-in only):
# atmosphere-mode = controlled-live
# atmosphere-regime = pulse
# See docs/ATMOSPHERE_PRESETS.md for all 6 profile preset examples.

# ── Legacy Advanced Keys (kept for compatibility) ────────────────────────────
# Prefer glitch-level for normal use.
# glitchpct = 10
# shortpct = 50
# rippct = 33.33333
# maxdpc = 3

# ── User Profile Config ──────────────────────────────────────────────────────
# Define named profiles and load with: cosmostrix --profile <name>
# Invalid profile values warn cleanly and are ignored.
# See docs/PROFILE_EXAMPLES.md for more profile examples.
# profile.nightcore.base = monolith
# profile.nightcore.color = purple
# profile.nightcore.charset = binary
# profile.nightcore.speed = 24
# profile.nightcore.density = 0.70
# profile.nightcore.glitch-level = subtle
# profile.nightcore.monolith-size = large
"#
}

#[must_use]
pub fn known_keys() -> Vec<&'static str> {
    USER_CONFIG_KEYS
        .iter()
        .chain(LEGACY_CONFIG_KEYS.iter())
        .chain(std::iter::once(&PROFILE_CONFIG_KEY_HINT))
        .copied()
        .collect()
}

#[inline]
fn is_known_key(key: &str) -> bool {
    USER_CONFIG_KEYS.contains(&key)
        || LEGACY_CONFIG_KEYS.contains(&key)
        || is_profile_config_key(key)
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
    }

    #[test]
    fn dump_config_contains_all_supported_keys() {
        let dump = dump_config_text();
        for key in USER_CONFIG_KEYS.iter().chain(LEGACY_CONFIG_KEYS.iter()) {
            assert!(dump.contains(key), "dump config should mention {key}");
        }
        assert!(dump.contains("profile.nightcore.base"));
    }
}
