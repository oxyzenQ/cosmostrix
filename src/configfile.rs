// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

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

use crate::constants::{CONFIG_DIR_NAME, CONFIG_FILE_NAME};
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
#[must_use]
pub fn load_config_file(path_override: Option<&Path>) -> HashMap<String, String> {
    let path = path_override
        .map(Path::to_path_buf)
        .unwrap_or_else(default_config_file_path);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };

    let parsed = parse_config_text(&content);
    for key in &parsed.unknown_keys {
        eprintln!(
            "config: ignoring unknown key '{}' (known: {})",
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
#[must_use]
pub fn default_config_file_path() -> PathBuf {
    config_file_path_from(env::var("XDG_CONFIG_HOME").ok(), env::var("HOME").ok())
}

#[must_use]
pub fn config_file_path_from(xdg_config_home: Option<String>, home: Option<String>) -> PathBuf {
    if let Some(xdg) = xdg_config_home.filter(|v| !v.is_empty()) {
        PathBuf::from(xdg)
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME)
    } else if let Some(home) = home.filter(|v| !v.is_empty()) {
        PathBuf::from(home)
            .join(".config")
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME)
    } else {
        PathBuf::from(".config")
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME)
    }
}

#[must_use]
pub fn dump_config_text() -> &'static str {
    r#"# Cosmostrix config
# Location:
#   $XDG_CONFIG_HOME/cosmostrix/config
#   or ~/.config/cosmostrix/config
#
# Format:
#   key = value
# Invalid values warn cleanly and are ignored.
#
# Precedence:
#   built-in defaults < config values < config preset < config scene
#   < config profile < CLI preset < CLI scene < CLI profile
#   < low-power < explicit CLI flags

# Scene atmosphere. See: cosmostrix --list-scenes
scene = monolith

# Curated preset. See: cosmostrix --list-presets
preset = cinematic

# User-defined profile to apply by default. See: cosmostrix --list-profiles
# profile = nightcore

# Appearance
color = cosmos
charset = binary
color-bg = black

# Motion
fps = 60
speed = 20
density = 0.75
monolith-size = normal

# Behavior
glitch-level = subtle
low-power = false
mouse = false
fullwidth = false
auto-color-drift = false

# Advanced style
bold = 1
shadingmode = 1

# Atmosphere engine (gated/internal-first — Phase 10).
# atmosphere-mode: disabled (default) | controlled-live
# atmosphere-regime: calm (default) | pulse | signal | compression | void | monolith-pressure
# Note: storm is NOT config-safe and will be rejected.
# These keys are opt-in; setting atmosphere-mode without controlled-live has no effect.
# atmosphere-mode = disabled
# atmosphere-regime = calm

# Controlled atmosphere examples (opt-in only):
# atmosphere-mode = controlled-live
# atmosphere-regime = pulse
# See docs/ATMOSPHERE_PRESETS.md for all 6 profile preset examples.

# Legacy advanced keys kept for compatibility.
# Prefer glitch-level for normal use.
# glitchpct = 10
# shortpct = 50
# rippct = 33.33333
# maxdpc = 3

# User scene/profile config.
# Load with: cosmostrix --profile nightcore
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
        assert_eq!(path, PathBuf::from("/tmp/xdg/cosmostrix/config"));
    }

    #[test]
    fn default_path_falls_back_to_home_config() {
        let path = config_file_path_from(None, Some("/tmp/home".to_string()));
        assert_eq!(path, PathBuf::from("/tmp/home/.config/cosmostrix/config"));
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
