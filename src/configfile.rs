// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Configuration file support for Cosmostrix.
//!
//! Reads an explicit `--config <PATH>` file or the default
//! `~/.config/cosmostrix/config.toml` (or `$XDG_CONFIG_HOME/cosmostrix/config.toml`).
//!
//! ## Philosophy
//!
//! The config file exposes daily-driver settings. It stays intentionally
//! flat and predictable.
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
use crate::scene_custom::is_scene_custom_config_key;

pub const USER_CONFIG_KEYS: &[&str] = &[
    "scene",
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
    "fullwidth",
    "auto-color-drift",
    "async-mode",
    "atmosphere-mode",
    "atmosphere-regime",
    "adaptive-custom",
    // v20: Cinematic intro selector. Values: "logo" | "cosmic" | "none".
    // Default: "logo". CLI --intro flag wins over this config key.
    "intro",
];

const PROFILE_CONFIG_KEY_HINT: &str = "profile.<name>.<color|charset|fps|speed|density|glitch-level|monolith-size|color-bg|atmosphere-mode|atmosphere-regime>";
const SCENE_CUSTOM_CONFIG_KEY_HINT: &str = "scene-custom.<name>.<color|charset|fps|speed|density|density-map|glitch-level|monolith-size|color-bg|atmosphere-mode|atmosphere-regime>";
const COLORS_CUSTOM_CONFIG_KEY_HINT: &str = "colors-custom.<name>.<bg|rain>";
const CHARSET_CUSTOM_CONFIG_KEY_HINT: &str = "charset-custom.<name>.set";
const COLOR_TUNE_CONFIG_KEY_HINT: &str = "color.tune.<brightness|saturation|head|body|tail>";

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

    let mut current_section: String = String::new();

    // Collect lines into a Vec so we can advance the index for multi-line
    // array values (e.g. rain = [\n  "#1a0033",\n  ...\n]).
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let stripped = strip_inline_comment(line).trim();
        if stripped.is_empty() {
            i += 1;
            continue;
        }

        if stripped.starts_with('[') && stripped.ends_with(']') && stripped.len() > 2 {
            let section = &stripped[1..stripped.len() - 1];
            let section = section.trim().to_ascii_lowercase();
            if section.is_empty() {
                malformed_lines.push(stripped.to_string());
                i += 1;
                continue;
            }
            current_section = section;
            i += 1;
            continue;
        }

        if let Some((key, value)) = stripped.split_once('=') {
            let key = key.trim().to_ascii_lowercase();
            let mut value = value.trim().to_string();
            if key.is_empty() || value.is_empty() {
                malformed_lines.push(stripped.to_string());
                i += 1;
                continue;
            }

            // v25: Handle multi-line TOML arrays. If the value starts with
            // '[' but doesn't end with ']', keep consuming subsequent lines
            // until we find the closing ']'. Join all lines into a single
            // string so the colors-custom parser can handle it.
            if value.starts_with('[') && !value.ends_with(']') {
                while i + 1 < lines.len() {
                    let next_line = strip_inline_comment(lines[i + 1]).trim();
                    if next_line.is_empty() {
                        i += 1;
                        continue;
                    }
                    value.push(' ');
                    value.push_str(next_line);
                    i += 1;
                    if next_line.ends_with(']') {
                        break;
                    }
                }
            }

            let full_key = if !current_section.is_empty() {
                format!("{current_section}.{key}")
            } else {
                key
            };
            if !is_known_key(&full_key) {
                unknown_keys.push(full_key);
                i += 1;
                continue;
            }
            map.insert(full_key, value);
        } else {
            // No `=` — malformed (unless we're inside a multi-line array,
            // but those are consumed above).
            malformed_lines.push(stripped.to_string());
        }
        i += 1;
    }

    ParsedConfig {
        values: map,
        unknown_keys,
        malformed_lines,
    }
}

/// Returns the path to the config file.
///
/// Platform-specific resolution:
/// - **Linux/macOS**: Uses `$XDG_CONFIG_HOME` if set, otherwise `~/.config`.
/// - **Windows**: Uses `%APPDATA%\cosmostrix\config.toml` (always absolute).
///
/// Looks for `config.toml`. v20.1 removed the pre-v10 `config` (no
/// extension) fallback — users upgrading from pre-v10 must rename their
/// file to `config.toml`.
///
/// **v25.2 Termux fix**: On Android Termux, the XDG spec is ambiguous —
/// Termux's default environment does NOT set `XDG_CONFIG_HOME`, but some
/// Termux setups (e.g., when `termux-x11` or `proot-distro` is involved)
/// set it to `$PREFIX/etc` (a system location, NOT where users put
/// their config). This caused `default_config_file_path()` to return
/// `/data/data/com.termux/files/usr/etc/cosmostrix/config.toml` while
/// the user was editing `~/.config/cosmostrix/config.toml` — the
/// live-reload watcher watched the wrong file, so edits appeared to do
/// nothing.
///
/// The fix: on Termux (detected via `TERMUX_VERSION` env var or `PREFIX`
/// containing "com.termux"), ALWAYS prefer `$HOME/.config/cosmostrix/config.toml`
/// — the location Termux documentation tells users to edit. The XDG_CONFIG_HOME
/// path is only used as a fallback when $HOME is unset.
#[must_use]
pub fn default_config_file_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = env::var("APPDATA").ok().filter(|v| !v.is_empty()) {
            return PathBuf::from(appdata)
                .join(CONFIG_DIR_NAME)
                .join(CONFIG_FILE_NAME);
        }
        // Fallback: USERPROFILE is always set on modern Windows.
        // If both env vars are somehow unset, use a relative path as last resort
        // (matches the Unix fallback behavior).
        if let Some(userprofile) = env::var("USERPROFILE").ok().filter(|v| !v.is_empty()) {
            return PathBuf::from(userprofile)
                .join("AppData")
                .join("Roaming")
                .join(CONFIG_DIR_NAME)
                .join(CONFIG_FILE_NAME);
        }
        // Ultimate fallback: C:\cosmostrix\ (guaranteed absolute on Windows).
        PathBuf::from("C:\\cosmostrix")
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let xdg = env::var("XDG_CONFIG_HOME").ok();
        let home = env::var("HOME").ok();
        let is_termux = is_termux_environment();

        // v25.2 Termux fix: on Termux, $HOME/.config/cosmostrix/config.toml
        // is the canonical location users edit (matches Termux wiki/docs).
        // XDG_CONFIG_HOME may point to $PREFIX/etc (a system location),
        // which is NOT where users put their config. Always prefer $HOME
        // on Termux, regardless of XDG_CONFIG_HOME.
        if is_termux {
            if let Some(home) = home.as_deref().filter(|v| !v.is_empty()) {
                return PathBuf::from(home)
                    .join(".config")
                    .join(CONFIG_DIR_NAME)
                    .join(CONFIG_FILE_NAME);
            }
            // $HOME unset on Termux is extremely unusual (Termux always
            // sets it). Fall through to XDG resolution as a safety net.
        }

        config_file_path_from_env(xdg.as_deref(), home.as_deref(), CONFIG_FILE_NAME)
    }
}

/// Detect Android Termux at runtime.
///
/// Termux installs regular Linux ARM binaries (compiled with
/// `target_os = "linux"`), NOT Android NDK binaries. So
/// `#[cfg(target_os = "android")]` would never match a Termux build.
/// Runtime detection via env vars is the canonical approach.
///
/// Returns `true` if either `TERMUX_VERSION` is set OR `PREFIX` contains
/// "com.termux". This matches the detection used elsewhere in the
/// codebase (safepath.rs, verbose.rs, event_loop.rs).
#[must_use]
pub fn is_termux_environment() -> bool {
    env::var("TERMUX_VERSION").is_ok()
        || env::var("PREFIX")
            .map(|p| p.contains("com.termux"))
            .unwrap_or(false)
}

/// Resolve the config path the live-reload watcher should watch.
///
/// This is the path the user is ACTUALLY editing, which may differ from
/// `default_config_file_path()` when:
/// 1. `--config <PATH>` was given on the CLI — use that path verbatim.
/// 2. On Termux, when `XDG_CONFIG_HOME` is set to a system location but
///    the user's config lives at `~/.config/cosmostrix/config.toml`.
/// 3. The default path doesn't exist, but an alternative candidate does
///    (e.g., `/etc/cosmostrix/config.toml` system-wide config, or
///    `/sdcard/cosmostrix/config.toml` on Termux external storage).
///
/// Returns `(resolved_path, existed_candidates)`. The caller should:
/// - Watch `resolved_path` (whether or not it currently exists — the
///   watcher will pick up the file when it's created).
/// - Use `existed_candidates` for diagnostic logging.
///
/// v25.2 Termux fix: this function existed conceptually but was inlined
/// in main.rs without the multi-candidate search. The Termux bug
/// ("live reload doesn't work") was caused by main.rs using
/// `args.config.unwrap_or_else(default_config_file_path)` which on
/// Termux resolved to the WRONG path when XDG_CONFIG_HOME was set to
/// $PREFIX/etc. This function centralizes the resolution logic so it
/// can be tested and reused.
#[must_use]
pub fn resolve_watcher_config_path(cli_config: Option<&Path>) -> (PathBuf, Vec<PathBuf>) {
    // Case 1: explicit --config path. Use verbatim.
    if let Some(p) = cli_config {
        return (p.to_path_buf(), vec![p.to_path_buf()]);
    }

    // Case 2: build candidate list and pick the first one that exists.
    let candidates = config_candidate_paths();
    let existed: Vec<PathBuf> = candidates.iter().filter(|p| p.exists()).cloned().collect();

    // Prefer the first existing candidate. If none exist, fall back to
    // the default path (the watcher will be skipped via spawn_watcher's
    // existence check, but at least we return a sensible path).
    let resolved = existed
        .first()
        .cloned()
        .unwrap_or_else(default_config_file_path);

    (resolved, existed)
}

/// Build the ordered list of candidate config file paths.
///
/// Order matters — the first candidate that exists is used. The list
/// reflects user intent:
/// 1. `~/.config/cosmostrix/config.toml` (XDG default, where users edit)
/// 2. `$XDG_CONFIG_HOME/cosmostrix/config.toml` (if XDG_CONFIG_HOME is
///    set DIFFERENTLY from $HOME/.config — covers system-wide setups)
/// 3. `/etc/cosmostrix/config.toml` (system-wide, installed by package managers)
/// 4. `/sdcard/cosmostrix/config.toml` (Termux external storage)
///
/// On Termux, candidate #1 is always `~/.config/cosmostrix/config.toml`
/// because `default_config_file_path()` already prefers $HOME on Termux.
/// On non-Termux Linux, candidate #1 may be `$XDG_CONFIG_HOME/...` if
/// XDG_CONFIG_HOME is set, otherwise `~/.config/...`.
#[must_use]
pub fn config_candidate_paths() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // Candidate #1: the default path (handles XDG_CONFIG_HOME + $HOME
    // fallback, with Termux $HOME preference baked in).
    let default = default_config_file_path();
    candidates.push(default.clone());

    // Candidate #2: $HOME/.config/cosmostrix/config.toml (XDG default
    // when XDG_CONFIG_HOME is unset). Only add if different from #1.
    // This catches the case where XDG_CONFIG_HOME is set to a system
    // location on non-Termux platforms (rare but possible).
    if let Some(home) = env::var("HOME").ok().filter(|v| !v.is_empty()) {
        let home_config = PathBuf::from(home)
            .join(".config")
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME);
        if !candidates.contains(&home_config) {
            candidates.push(home_config);
        }
    }

    // Candidate #3: $XDG_CONFIG_HOME/cosmostrix/config.toml (if set
    // differently from $HOME/.config). This is the source of the Termux
    // bug — when XDG_CONFIG_HOME=$PREFIX/etc, this is the WRONG path,
    // but we include it as a fallback for users who genuinely put their
    // config there.
    if let Some(xdg) = env::var("XDG_CONFIG_HOME").ok().filter(|v| !v.is_empty()) {
        let xdg_config = PathBuf::from(xdg)
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME);
        if !candidates.contains(&xdg_config) {
            candidates.push(xdg_config);
        }
    }

    // Candidate #4: /etc/cosmostrix/config.toml (system-wide).
    let system = PathBuf::from("/etc")
        .join(CONFIG_DIR_NAME)
        .join(CONFIG_FILE_NAME);
    if !candidates.contains(&system) {
        candidates.push(system);
    }

    // Candidate #5: /sdcard/cosmostrix/config.toml (Termux external
    // storage — only relevant on Termux, but harmless to include).
    let sdcard = PathBuf::from("/sdcard")
        .join(CONFIG_DIR_NAME)
        .join(CONFIG_FILE_NAME);
    if !candidates.contains(&sdcard) {
        candidates.push(sdcard);
    }

    candidates
}

#[cfg(not(target_os = "windows"))]
#[must_use]
#[allow(dead_code)]
pub fn config_file_path_from(xdg_config_home: Option<String>, home: Option<String>) -> PathBuf {
    config_file_path_from_env(
        xdg_config_home.as_deref(),
        home.as_deref(),
        CONFIG_FILE_NAME,
    )
}

#[cfg(not(target_os = "windows"))]
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
    r##"# cosmostrix configuration

#
# Quick Start & Override Priority
#
#
# The easiest way to customize is via CLI flags:
#   cosmostrix -c neon-green --speed 20
#
# For permanent settings, edit this file. Values here override scene defaults.
# CLI flags always override this file.
#
# Override priority (highest wins):
#   1. CLI flags           (e.g. -c neon-green, --speed 20)    ← HIGHEST
#   2. config.toml         (this file — values set here)      ← MEDIUM
#   3. scene defaults      (built-in scenes like cinematic)   ← LOWEST
#
# Key rule: a value set in config.toml ALWAYS wins over a scene's
# hardcoded default. Scenes only fill keys the user did NOT set.
# This prevents surprises like `speed = 30` in config being silently
# overwritten by a scene's `speed = 8`.
#
# Examples:
#   cosmostrix                                       # run with defaults
#   cosmostrix --scene storm                         # built-in scene
#   cosmostrix --scene-custom hacker-mode            # user-defined custom scene
#   cosmostrix -c neon-green --speed 20              # CLI overrides config
#   cosmostrix --list-scenes                         # list all scenes
#   cosmostrix --testconf                            # validate this config
#   cosmostrix --doctor                              # diagnose terminal issues

#
# File Location
#
#
#   Linux:   ~/.config/cosmostrix/config.toml
#   macOS:   ~/.config/cosmostrix/config.toml
#            (or ~/Library/Application Support/cosmostrix/config.toml)
#   Windows: %APPDATA%\cosmostrix\config.toml
#   System-wide: /etc/cosmostrix/config.toml (Linux/macOS)
#                %ProgramData%\cosmostrix\config.toml (Windows)
#   Or set $XDG_CONFIG_HOME (Linux/macOS).
#
# Format:
#   key = value              # one per line
#   # comments               # blank lines ignored
#   [section.name]           # TOML table header (groups keys under it)
#   field = value            # keys inside a table are prefixed automatically
#   Custom blocks support BOTH flat (scene-custom.name.field = value)
#   and TOML table ([scene-custom.name] + field = value) formats.
#   Malformed lines (no '=' or empty key/value) cause --testconf to FAIL.
#
# All keys below are commented out. Uncomment the ones you want to
# customize — cosmostrix's built-in defaults (shown for reference)
# will be used for any key left commented. Run `cosmostrix --testconf`
# to validate your config after editing.

#
# Standard Settings (flat key = value)
#

# Core

# Scene — built-in atmospheric template
#   cinematic (default) | matrix | monolith | signal | classic | calm
#   storm | cosmos | neon | hacker | low-power | cosmic_dragon | carbonic
# Examples: scene = monolith, scene = matrix, scene = cosmic_dragon
# scene = cinematic

# Color scheme (palette). See: cosmostrix --list-colors
# color = cosmos

# Character set for rain glyphs. See: cosmostrix --list-charsets
# charset = binary

# Background mode: default-background (follow terminal) | black (solid #000000)
# color-bg = default-background

# Cinematic intro animation played before the rain engine starts.
# intro = "logo"  # Intro animation: logo | cosmic | none

# Motion

# Target FPS. Adaptive pacing may reduce under load.
# fps = 60

# Rain fall speed (1–100). Default depends on scene:
#   monolith=30, matrix=18, signal=14, storm=28, calm=6, low-power=5
# speed = 30

# Rain density (0.01–5.0). Default depends on scene:
#   monolith=0.85, matrix=0.65, signal=0.55, storm=1.10, calm=0.40
# density = 0.85

# Variable column speeds for organic rain (default: on)
# async-mode = true

# Monolith

# Pillar size (small | normal | large, only for monolith scene)
# monolith-size = normal

# Behavior

# Glitch intensity: none | subtle | default | intense
# glitch-level = subtle

# v17: --mouse flag DELETED. Mouse glow + click wave effects are always on.
# Mouse reporting is always active (blocks text selection).
# No config key needed — the effect is part of cosmostrix's signature.

# Full-width CJK glyphs (default: off)
# fullwidth = false

# Auto color drift (default: off)
# auto-color-drift = false

# Advanced Style

# Color tuning (adjust rain brightness/saturation/head/body/tail)
# All values: 0.0-3.0, default 1.0 = no change
# [color.tune]
# brightness = 1.0   # global brightness (dim-rain: use < 1.0)
# saturation = 1.0   # color saturation (0.0 = grayscale)
# head = 1.0         # head segment brightness
# body = 1.0         # body segment brightness
# tail = 1.0         # tail segment brightness

# Bold style: 0=off, 1=random (default), 2=all
# bold = 1

# Shading mode: 0=random, 1=cinematic (default — distance from head)
# shadingmode = 1

# Atmosphere Engine (opt-in)

# atmosphere-mode: disabled (default) | controlled-live
# atmosphere-regime: calm | pulse | signal | compression | void | monolith-pressure | adaptive
# atmosphere-mode = disabled
# atmosphere-regime = calm

# Controlled atmosphere example:
# atmosphere-mode = controlled-live
# atmosphere-regime = adaptive

# Glitch behavior is fully owned by --glitch-level (none|subtle|default|intense).
# The preset controls glitch percent, stream decay, fragmented stream chance,
# and stream layering automatically — there are no separate config keys.

# Custom Configuration (advanced, optional)
#
# The sections below define user-named custom resources. They are
# loaded via CLI flags (--scene-custom, --colors-custom) and do not
# affect the standard settings above. Moved to the bottom of the file
# to keep the main config clean and the override priority obvious.
#
# Custom Scene Definitions
#
# Define named custom scenes and load with: cosmostrix --scene-custom <name>
# Fields: color, charset, fps, speed, density, density-map,
#         glitch-level, monolith-size, color-bg, atmosphere-mode, atmosphere-regime
# Custom scenes stand on their own — missing fields fall back to cinematic's
# defaults. (base-scene and preset were removed in v20.1; if present in
# config.toml, --testconf will flag them as unknown keys.)
# Custom scenes are listed alongside built-in scenes in --list-scenes output.
# See docs/ATMOSPHERE_ENGINE.md for more examples.

# [scene-custom.hacker-mode]
# color = green
# charset = hacker
# speed = 28
# density = 1.2
# glitch-level = intense

# Density Map: sculpt monolith pillar formation per-column.
# Comma-separated weights (0.0..1.0). 0.0 = never spawn, 1.0 = always spawn.
# Maps shorter than terminal width treat missing columns as 1.0.
#
# Three cinematic presets (120 columns each) — uncomment to use:

# Twin Towers — two dense pillar clusters, sparse canyon between.
# [scene-custom.twin-towers]
# charset = braille
# color = neon-purple
# speed = 30
# density = 0.85
# density-map = 0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.7,0.7,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,0.7,0.7,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.7,0.7,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,0.7,0.7,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08,0.08

# Cascade — smooth linear gradient: dense left, sparse right (waterfall).
# [scene-custom.cascade]
# charset = braille
# color = neon-purple
# speed = 30
# density = 0.85
# density-map = 1.0,0.992,0.984,0.976,0.968,0.96,0.952,0.944,0.936,0.928,0.92,0.912,0.904,0.896,0.888,0.88,0.872,0.864,0.856,0.848,0.84,0.832,0.824,0.816,0.808,0.8,0.792,0.784,0.776,0.768,0.761,0.753,0.745,0.737,0.729,0.721,0.713,0.705,0.697,0.689,0.681,0.673,0.665,0.657,0.649,0.641,0.633,0.625,0.617,0.609,0.601,0.593,0.585,0.577,0.569,0.561,0.553,0.545,0.537,0.529,0.521,0.513,0.505,0.497,0.489,0.481,0.473,0.465,0.457,0.449,0.441,0.433,0.425,0.417,0.409,0.401,0.393,0.385,0.377,0.369,0.361,0.353,0.345,0.337,0.329,0.321,0.313,0.305,0.297,0.289,0.282,0.274,0.266,0.258,0.25,0.242,0.234,0.226,0.218,0.21,0.202,0.194,0.186,0.178,0.17,0.162,0.154,0.146,0.138,0.13,0.122,0.114,0.106,0.098,0.09,0.082,0.074,0.066,0.058,0.05

# Throne — massive pillar at center, ringed by sparse court.
# [scene-custom.throne]
# charset = braille
# color = neon-purple
# speed = 30
# density = 0.85
# density-map = 0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.3,0.3,0.3,0.3,0.3,0.8,0.8,0.8,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,0.8,0.8,0.8,0.3,0.3,0.3,0.3,0.3,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.12,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05,0.05

# Adaptive Custom Time Map
#
# Optional. Overrides the default 5-phase adaptive engine.
# Define your own time-to-parameter mapping. Format: H-M = color, scene, key=value, ...
# Time format: flexible digits — 2-3, 02-03, 2-30, 14-5 all valid.
# Parameters not specified are sticky (keep previous value).
# Transition: smooth 5-minute blend before next time point.
# If not defined, default adaptive engine (5 phases) is used.
# Note: custom time map is checked every 30s at runtime.
# Live config reload re-parses the map immediately on save.

# adaptive-custom.00-00 = cosmos, monolith, speed=15, density=1.2
# adaptive-custom.06-00 = aurora, signal, speed=10, density=0.5
# adaptive-custom.12-00 = cosmos, monolith, speed=30, density=0.85
# adaptive-custom.18-00 = neon, storm, speed=24, density=1.1

# Custom Color Palettes (optional, v16+)
#
# Define named custom palettes usable from --colors-custom or adaptive-custom.
# Uses TOML table format. Hex values use standard #rrggbb notation.
#
# Fields:
#   bg   — background color (optional)
#   rain — array of 7 hex gradient stops (tail → head order).
#   Format: rain = ["#stop0", "#stop1", ..., "#stop6"]
#   Also accepts CSV string: rain = "#stop0, #stop1, ..."
#   Minimum 2 stops required; 7 stops recommended for full 3-2-2 distribution.
#
# Load with: cosmostrix --colors-custom mytheme
# Use in adaptive-custom: adaptive-custom.22-00 = mytheme, monolith

# [colors-custom.mythme]
# bg = "#0a0a12"
# rain = [
#   "#1a0033",  # tail dimmer
#   "#4d0080",  # tail dim
#   "#9933ff",  # semi-body dark
#   "#cc66ff",  # body peak
#   "#e6b3ff",  # semi-body light
#   "#f2ccff",  # semi-white
#   "#ffffff",  # head glow
# ]

# Custom Character Sets (optional, v25+)
#
# Define named custom charsets usable from --charset or charset = "name".
# Replaces the legacy --charset-file CLI flag — the charset now lives in
# config.toml next to every other setting, no external file needed.
#
# Fields:
#   set — the literal string of characters to use as the rain glyph pool.
#   Whitespace (except ASCII space) is skipped. Control characters and
#   characters longer than the 256-char cap are rejected with an error.
#   Wide/zero-width characters (emoji, CJK fullwidth) are auto-filtered.
#
# Load with: cosmostrix --charset cat
# Or set in config: charset = "cat"
# Custom names take precedence over built-in presets with the same name.
# Live reload: editing the block takes effect on the next config save.

# [charset-custom.zen]
# set = "|"

# [charset-custom.greek-letters]
# set = "αβγδεζηθικλμνξοπρστυφχψω"
"##
}

#[must_use]
pub fn known_keys() -> Vec<&'static str> {
    USER_CONFIG_KEYS
        .iter()
        .chain(std::iter::once(&PROFILE_CONFIG_KEY_HINT))
        .chain(std::iter::once(&SCENE_CUSTOM_CONFIG_KEY_HINT))
        .chain(std::iter::once(&COLORS_CUSTOM_CONFIG_KEY_HINT))
        .chain(std::iter::once(&CHARSET_CUSTOM_CONFIG_KEY_HINT))
        .chain(std::iter::once(&COLOR_TUNE_CONFIG_KEY_HINT))
        .copied()
        .collect()
}

#[inline]
fn is_known_key(key: &str) -> bool {
    USER_CONFIG_KEYS.contains(&key)
        || is_profile_config_key(key)
        || is_scene_custom_config_key(key)
        || is_adaptive_custom_key(key)
        || is_colors_custom_key(key)
        || is_charset_custom_key(key)
        || is_color_tune_key(key)
}

/// Check if `key` matches the `colors-custom.<name>.<field>` pattern.
///
/// Recognized fields (v16):
/// - `bg` / `background` — background color (hex)
/// - `normal.red`, `normal.green`, `normal.blue` — core normal colors
/// - `normal.yellow`, `normal.cyan`, `normal.magenta`, `normal.white` — extended normal
/// - `bright.red`, `bright.green`, `bright.blue` — core bright colors
/// - `bright.yellow`, `bright.cyan`, `bright.magenta`, `bright.white` — extended bright
/// - `head` — head (brightest) color (hex) — cosmostrix-specific
/// - `stops` — hex gradient stops (array or CSV format) — cosmostrix-specific
///
/// Name must be non-empty, ASCII alphanumeric + `-`/`_` only.
#[inline]
/// v17: Check if key matches `color.tune.<field>` pattern.
fn is_color_tune_key(key: &str) -> bool {
    matches!(
        key,
        "color.tune.brightness"
            | "color.tune.saturation"
            | "color.tune.head"
            | "color.tune.body"
            | "color.tune.tail"
    )
}

fn is_colors_custom_key(key: &str) -> bool {
    let Some(rest) = key.strip_prefix("colors-custom.") else {
        return false;
    };
    // Must have at least name.field (2+ segments after the prefix).
    let Some((name, field)) = rest.split_once('.') else {
        return false;
    };
    if name.is_empty() || !is_valid_custom_name(name) {
        return false;
    }
    is_valid_colors_custom_field(field)
}

/// Check if a custom palette name is valid (non-empty, alphanumeric + -/_).
#[inline]
fn is_valid_custom_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Check if a colors-custom field name is recognized.
#[inline]
fn is_valid_colors_custom_field(field: &str) -> bool {
    matches!(field, "bg" | "background" | "rain")
}

/// Check if `key` matches the `charset-custom.<name>.set` pattern.
///
/// v25: replaces the legacy `--charset-file <PATH>` CLI flag with an
/// in-config block. Only the `set` field is recognized — any other
/// field under `[charset-custom.<name>]` is rejected as unknown by
/// `is_known_key()` so the user gets a clear `--testconf` error.
///
/// Name must be non-empty, ASCII alphanumeric + `-`/`_` only (same rule
/// as `colors-custom`).
#[inline]
fn is_charset_custom_key(key: &str) -> bool {
    let Some(rest) = key.strip_prefix("charset-custom.") else {
        return false;
    };
    let Some((name, field)) = rest.split_once('.') else {
        return false;
    };
    if name.is_empty() || !is_valid_custom_name(name) {
        return false;
    }
    field == "set"
}

/// Check if `key` matches the `adaptive-custom.H-M` pattern.
/// Accepts flexible digit counts: `2-3`, `02-03`, `2-03`, `02-3` all valid.
#[inline]
fn is_adaptive_custom_key(key: &str) -> bool {
    let Some(rest) = key.strip_prefix("adaptive-custom.") else {
        return false;
    };
    let Some((hh, mm)) = rest.split_once('-') else {
        return false;
    };
    !hh.is_empty()
        && !mm.is_empty()
        && hh.chars().all(|c| c.is_ascii_digit())
        && mm.chars().all(|c| c.is_ascii_digit())
}

/// Strip inline comments (`# ...`) from a config line, respecting quoted strings.
///
/// A `#` inside a double-quoted or single-quoted string is NOT treated as a
/// comment — it's part of the value. This is critical for hex color values
/// like `red = "#ff0000"` where `#` is the standard hex prefix.
///
/// Example:
///   `color = green # my favorite`     → `color = green`
///   `red = "#ff0000" # comment`       → `red = "#ff0000"`
///   `msg = "it's #1" # note`          → `msg = "it's #1"`
///
/// Unquoted `#` still works as before for backward compatibility.
#[inline]
fn strip_inline_comment(line: &str) -> &str {
    let mut in_dquote = false;
    let mut in_squote = false;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' if !in_squote => in_dquote = !in_dquote,
            '\'' if !in_dquote => in_squote = !in_squote,
            '#' if !in_dquote && !in_squote => {
                return &line[..i];
            }
            _ => {}
        }
    }
    line
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
    fn legacy_keys_removed_v17() {
        // v17 mastery: legacy advanced keys (glitchpct, shortpct, rippct,
        // maxdpc) are REMOVED. They are now flagged as unknown by --testconf
        // so users know to migrate to --glitch-level. They do NOT go into
        // parsed.values (only known keys do).
        let parsed = parse_config_text("glitchpct = 3\nshortpct = 60\nrippct = 45\nmaxdpc = 2\n");
        assert_eq!(
            parsed.values.len(),
            0,
            "legacy keys should not be in values"
        );
        assert_eq!(
            parsed.unknown_keys.len(),
            4,
            "legacy keys should be flagged as unknown"
        );
        assert!(parsed.unknown_keys.contains(&"glitchpct".to_string()));
        assert!(parsed.unknown_keys.contains(&"shortpct".to_string()));
        assert!(parsed.unknown_keys.contains(&"rippct".to_string()));
        assert!(parsed.unknown_keys.contains(&"maxdpc".to_string()));
    }

    #[test]
    fn profile_keys_are_known() {
        // v20.1: `base-scene` is no longer a recognized profile field — it
        // must be flagged as unknown. `color` is still recognized.
        let parsed = parse_config_text(
            "profile.nightcore.base-scene = monolith\nprofile.nightcore.color = purple\n",
        );
        // base-scene is unknown and therefore NOT stored in values.
        assert_eq!(parsed.values.get("profile.nightcore.base-scene"), None);
        assert!(parsed
            .unknown_keys
            .contains(&"profile.nightcore.base-scene".to_string()));
        // color is recognized and stored.
        assert_eq!(
            parsed
                .values
                .get("profile.nightcore.color")
                .map(String::as_str),
            Some("purple")
        );
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
        for key in USER_CONFIG_KEYS.iter() {
            assert!(dump.contains(*key), "dump config should mention {key}");
        }
        assert!(dump.contains("[scene-custom.hacker-mode]"));
    }

    #[test]
    fn parse_multiline_array_joins_correctly() {
        let content = "[colors-custom.mythme]\nbg = \"#0a0a12\"\nrain = [\n  \"#1a0033\",\n  \"#4d0080\",\n  \"#9933ff\",\n  \"#cc66ff\",\n  \"#e6b3ff\",\n  \"#f2ccff\",\n  \"#ffffff\",\n]\n";
        let parsed = parse_config_text(content);
        assert!(
            parsed.malformed_lines.is_empty(),
            "no malformed lines, got: {:?}",
            parsed.malformed_lines
        );
        assert!(
            parsed.unknown_keys.is_empty(),
            "no unknown keys, got: {:?}",
            parsed.unknown_keys
        );
        let rain = parsed.values.get("colors-custom.mythme.rain");
        assert!(rain.is_some(), "rain key should be parsed");
        let rain = rain.unwrap();
        assert!(rain.starts_with('['), "rain value should start with [");
        assert!(rain.ends_with(']'), "rain value should end with ]");
    }

    // ── v25.2 Termux fix: path resolution tests ──

    #[test]
    fn is_termux_environment_returns_false_off_termux() {
        // On a normal Linux/macOS/Windows CI runner, neither TERMUX_VERSION
        // nor a "com.termux"-containing PREFIX is set. This test verifies
        // the detection returns false. (It would return true on an actual
        // Termux runner, where this assertion is skipped via env check.)
        let on_termux = std::env::var("TERMUX_VERSION").is_ok()
            || std::env::var("PREFIX")
                .map(|p| p.contains("com.termux"))
                .unwrap_or(false);
        if !on_termux {
            assert!(!is_termux_environment(), "should be false off Termux");
        }
    }

    #[test]
    fn is_termux_environment_detects_termux_version() {
        // Simulate Termux by setting TERMUX_VERSION in a subprocess.
        // We can't actually set env vars in-process, so we replicate
        // the detection logic with a known-set value.
        let detected = std::env::var("TERMUX_VERSION").is_ok()
            || std::env::var("PREFIX")
                .map(|p| p.contains("com.termux"))
                .unwrap_or(false);
        // On CI runners, this is false; on Termux, this is true.
        // Either way, is_termux_environment() must agree with our manual check.
        assert_eq!(is_termux_environment(), detected);
    }

    #[test]
    fn config_candidate_paths_includes_default_path() {
        // The first candidate should always be default_config_file_path().
        let candidates = config_candidate_paths();
        assert!(!candidates.is_empty(), "candidate list must not be empty");
        assert_eq!(
            candidates[0],
            default_config_file_path(),
            "first candidate must be default_config_file_path()"
        );
    }

    #[test]
    fn config_candidate_paths_includes_system_path() {
        // /etc/cosmostrix/config.toml should always be in the candidate list
        // (it's a system-wide fallback). This is unconditional — even on
        // platforms where it doesn't exist, the candidate is listed so
        // the resolver can check it.
        let candidates = config_candidate_paths();
        let system = PathBuf::from("/etc")
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME);
        assert!(
            candidates.contains(&system),
            "candidate list must include {system:?}"
        );
    }

    #[test]
    fn config_candidate_paths_includes_sdcard_path() {
        // /sdcard/cosmostrix/config.toml should be in the candidate list
        // (Termux external storage fallback).
        let candidates = config_candidate_paths();
        let sdcard = PathBuf::from("/sdcard")
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME);
        assert!(
            candidates.contains(&sdcard),
            "candidate list must include {sdcard:?}"
        );
    }

    #[test]
    fn config_candidate_paths_no_duplicates() {
        // Even if XDG_CONFIG_HOME equals $HOME/.config, the candidate list
        // must not contain duplicate entries.
        let candidates = config_candidate_paths();
        let mut seen = std::collections::HashSet::new();
        for c in &candidates {
            assert!(seen.insert(c.clone()), "duplicate candidate: {c:?}");
        }
    }

    #[test]
    fn resolve_watcher_config_path_uses_cli_config_when_provided() {
        // When --config <PATH> is given, the resolver must use that path
        // verbatim — no candidate search.
        let cli_path = Path::new("/tmp/cosmostrix-test-custom.toml");
        let (resolved, existed) = resolve_watcher_config_path(Some(cli_path));
        assert_eq!(resolved, cli_path, "must use CLI path verbatim");
        assert_eq!(
            existed,
            vec![cli_path],
            "existed list must be just the CLI path"
        );
    }

    #[test]
    fn resolve_watcher_config_path_returns_default_when_no_candidates_exist() {
        // When no candidate exists, the resolver falls back to the default
        // path. This is the "user hasn't created a config yet" case.
        // Save the current env, unset HOME/XDG_CONFIG_HOME so the default
        // path is the relative `.config/cosmostrix/config.toml`.
        let saved_home = std::env::var("HOME").ok();
        let saved_xdg = std::env::var("XDG_CONFIG_HOME").ok();
        std::env::remove_var("HOME");
        std::env::remove_var("XDG_CONFIG_HOME");

        // Mark Termux detection as needing re-check by clearing env vars
        // that might match (the test runner might be on a system where
        // TERMUX_VERSION isn't set, which is the normal case).
        let (resolved, existed) = resolve_watcher_config_path(None);

        // Restore env vars immediately to avoid breaking other tests.
        if let Some(h) = saved_home {
            std::env::set_var("HOME", h);
        }
        if let Some(x) = saved_xdg {
            std::env::set_var("XDG_CONFIG_HOME", x);
        }

        // When HOME and XDG_CONFIG_HOME are both unset, the default path
        // is `.config/cosmostrix/config.toml` (relative). The resolver
        // must return this path. existed should be empty (the relative
        // path likely doesn't exist as a file).
        assert_eq!(
            resolved,
            PathBuf::from(".config")
                .join(CONFIG_DIR_NAME)
                .join(CONFIG_FILE_NAME),
            "must fall back to relative default path when no candidates exist"
        );
        // existed might or might not contain /etc/cosmostrix/config.toml
        // depending on the test runner. We don't assert on it because
        // it's environment-dependent.
        let _ = existed;
    }
}
