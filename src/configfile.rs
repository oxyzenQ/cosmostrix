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

pub(crate) const USER_CONFIG_KEYS: &[&str] = &[
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
    "auto-color-drift",
    "async-mode",
    // v35.2 (CLI-D-1 fix): `adaptive-custom` removed from this whitelist.
    // The atmosphere engine was eliminated at commit 07b44b5 (2026-08-05),
    // and `config_hints.rs` + `testconf.rs` both explicitly reject
    // `adaptive-custom.*` keys with "have been removed" messages. But the
    // bare key was still classified as "known" here, so a stale
    // `adaptive-custom = "10-00, neon-purple, signal"` line was silently
    // stored and never applied at runtime — zero warning at startup, only
    // `--testconf` caught it. Now the bare key falls into `unknown_keys`
    // and triggers the startup rejection at `config_apply.rs:149-163`.
    // v20: Cinematic intro selector. Values: "logo" | "cosmic" | "none".
    // Default: "logo". CLI --intro flag wins over this config key.
    "intro",
];

const PROFILE_CONFIG_KEY_HINT: &str =
    "profile.<name>.<base-scene|color|charset|fps|speed|density|glitch-level|monolith-size|color-bg>";
const SCENE_CUSTOM_CONFIG_KEY_HINT: &str = "scene-custom.<name>.<base-scene|color|charset|bold|colors-custom|charset-custom|shadingmode|glitch-level|fps|speed|density|density-map|async>";
const COLORS_CUSTOM_CONFIG_KEY_HINT: &str = "colors-custom.<name>.<bg|rain|stops>";
const CHARSET_CUSTOM_CONFIG_KEY_HINT: &str = "charset-custom.<name>.set";
const COLOR_TUNE_CONFIG_KEY_HINT: &str = "color.tune.<brightness|saturation|head|body|tail>";
/// Ambient phase scheduler: `ambient.<HH-MM> = <scene-name>`.
///
/// v30.2: simplified — value is a single scene name (built-in OR custom).
/// Config-only (no CLI flag). Time-of-day phase entries that switch the
/// active scene at scheduled times. Instant switch (no blend window).
/// Dynamic idle/wake scheduler thread — zero CPU between phase boundaries.
/// See `src/ambient.rs` and `src/ambient_scheduler.rs`.
const AMBIENT_CONFIG_KEY_HINT: &str = "ambient.<HH-MM> = <scene-name>";

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ParsedConfig {
    pub values: HashMap<String, String>,
    pub unknown_keys: Vec<String>,
    /// Non-empty, non-comment lines that do not match `key = value` syntax.
    ///
    /// Tracked so `--testconf` can report them as errors and `load_config_file`
    /// can warn on stderr. A line lands here when it has no `=` at all, or when
    /// either side of `=` is empty after trimming.
    pub malformed_lines: Vec<String>,
    /// v25.7: keys that were auto-promoted from a nested section to root scope.
    ///
    /// Each tuple is `(original_nested_key, promoted_root_key)`. Populated when
    /// the user writes a top-level key (e.g. `fps = 30`) AFTER a
    /// `[scene-custom.<name>]` table header — TOML parses it as
    /// `scene-custom.<name>.fps`, but we detect the un-prefixed form is a
    /// known top-level key and silently re-home it. This lets top-level keys
    /// and `[scene-custom.<name>]` blocks coexist in the same file without
    /// forcing the user to learn TOML scope rules.
    pub promoted_keys: Vec<(String, String)>,
}

/// Load config file and return a HashMap of key → value pairs.
/// Returns empty HashMap if file doesn't exist or can't be read.
/// Warns on stderr for unrecognized keys (likely typos).
///
/// Search order when no explicit path is given:
/// 1. `$XDG_CONFIG_HOME/cosmostrix/config.toml` (or `~/.config/cosmostrix/config.toml`)
/// 2. `/etc/cosmostrix/config.toml` (system-wide default, installed by AUR/package manager)
///
/// This means AUR users get a working default config out of the box —
/// the package installs `/etc/cosmostrix/config.toml`, and cosmostrix
/// reads it automatically if no user-level config exists.
#[must_use]
pub(crate) fn load_config_file(path_override: Option<&Path>) -> HashMap<String, String> {
    load_config_file_full(path_override).values
}

/// Phase 5 closure (P4-8): load config file and return the FULL parse result
/// (including `malformed_lines` and `unknown_keys` vectors).
///
/// `load_config_file` discards these vectors (it only returns `values`).
/// Callers that need malformed/unknown detection (e.g. startup validation in
/// `config_apply.rs`) previously had to re-read + re-parse the file from disk
/// to recover them. This function eliminates the redundant disk read by
/// returning the full `ParsedConfig` in one pass.
///
/// Most callers should use `load_config_file` (which returns just the values
/// HashMap). Use this function only when you need the malformed/unknown vectors.
#[must_use]
pub(crate) fn load_config_file_full(path_override: Option<&Path>) -> ParsedConfig {
    let path = path_override
        .map(Path::to_path_buf)
        .unwrap_or_else(default_config_file_path);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            // Fallback: try system-wide config at /etc/cosmostrix/config.toml.
            if path_override.is_none() {
                let system_path = PathBuf::from("/etc/cosmostrix/config.toml");
                if let Ok(sys_content) = std::fs::read_to_string(&system_path) {
                    sys_content
                } else {
                    return ParsedConfig {
                        values: HashMap::new(),
                        unknown_keys: Vec::new(),
                        malformed_lines: Vec::new(),
                        promoted_keys: Vec::new(),
                    };
                }
            } else {
                return ParsedConfig {
                    values: HashMap::new(),
                    unknown_keys: Vec::new(),
                    malformed_lines: Vec::new(),
                    promoted_keys: Vec::new(),
                };
            }
        }
    };

    parse_config_text(&content)
}

#[must_use]
pub(crate) fn parse_config_text(content: &str) -> ParsedConfig {
    let mut map = HashMap::new();
    let mut unknown_keys = Vec::new();
    let mut malformed_lines = Vec::new();
    let mut promoted_keys: Vec<(String, String)> = Vec::new();

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

            // v25.9 (bug #7): Detect unquoted '#' inside an array value.
            // strip_inline_comment strips at first unquoted '#'. If that
            // happened while bracket depth > 0, the user wrote e.g.
            // `rain = [#ff0000, #00ff00]` — '#' inside array was treated
            // as comment, silently truncating value to '[' and triggering
            // the multi-line consumer to eat subsequent lines. Reject
            // explicitly. We check the ORIGINAL line (not `stripped`)
            // because `stripped` is already truncated.
            if value.starts_with('[')
                && unquoted_hash_inside_array(line).is_some()
                && !value.ends_with(']')
            {
                malformed_lines.push(format!(
                    "{stripped}  # ERROR: unquoted '#' inside array — quote hex values (e.g. \"#ff0000\") or remove the '#'"
                ));
                i += 1;
                continue;
            }

            // v25: Handle multi-line TOML arrays. If the value starts with
            // '[' but doesn't end with ']', consume subsequent lines until
            // we find the closing ']'. v25.9 (bug #7) hardening: do NOT
            // consume [section] headers — those were previously mistaken
            // for the closing ']' of the array, corrupting subsequent
            // block definitions.
            if value.starts_with('[') && !value.ends_with(']') {
                while i + 1 < lines.len() {
                    let raw_next = lines[i + 1];
                    let next_line = strip_inline_comment(raw_next).trim();
                    if next_line.is_empty() {
                        i += 1;
                        continue;
                    }
                    // [section] header is never an array element. Stop.
                    if next_line.starts_with('[') && next_line.ends_with(']') && next_line.len() > 2
                    {
                        break;
                    }
                    value.push(' ');
                    value.push_str(next_line);
                    i += 1;
                    if next_line.ends_with(']') {
                        break;
                    }
                }
                // Still no closing ']' → genuinely malformed (user forgot
                // to close, or '#' truncated). Reject explicitly.
                if !value.ends_with(']') {
                    malformed_lines.push(format!(
                        "{stripped}  # ERROR: array never closed (missing ']') or '#' truncated the value"
                    ));
                    i += 1;
                    continue;
                }
            }

            let full_key = if !current_section.is_empty() {
                format!("{current_section}.{key}")
            } else {
                key.clone()
            };
            if !is_known_key(&full_key) {
                // v25.7: Auto-promote forgiving parser. If the un-prefixed
                // key is itself a known top-level key, the user accidentally
                // nested it under a [section] header (very common when
                // mixing [scene-custom.<name>] with top-level keys like
                // `fps` or `speed`). Silently re-home it to root scope and
                // record the promotion so --testconf can warn the user
                // about the structural fix.
                if !current_section.is_empty() && is_known_key(&key) {
                    promoted_keys.push((full_key.clone(), key.clone()));
                    // Don't overwrite an explicit root-scope value — first
                    // writer wins (matches TOML semantics for duplicate keys).
                    map.entry(key).or_insert(value);
                } else {
                    unknown_keys.push(full_key);
                }
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
        promoted_keys,
    }
}

/// Returns the path to the config file.
///
/// Platform-specific resolution:
/// - **Linux, macOS, FreeBSD, Android (Termux)**: Uses `$XDG_CONFIG_HOME`
///   if set, otherwise `~/.config`. On Termux specifically,
///   `XDG_CONFIG_HOME` is deliberately IGNORED (see "v25.2 Termux fix"
///   below) because it may point to `$PREFIX/etc`, a system location
///   users don't edit.
/// - **Windows**: Uses `%APPDATA%\cosmostrix\config.toml` (always absolute).
///
/// System-wide fallback locations (consulted by `resolve_config_path`
/// when the user-specific path doesn't exist):
/// - **Linux**: `/etc/cosmostrix/config.toml`
/// - **macOS**: `/Library/Application Support/cosmostrix/config.toml`
/// - **FreeBSD**: `/usr/local/etc/cosmostrix/config.toml`
///   (FreeBSD uses `/usr/local/etc` for ports/packages, not `/etc`)
/// - **Android (Termux)**: `$PREFIX/etc/cosmostrix/config.toml`
///   (typically `/data/data/com.termux/files/usr/etc/cosmostrix/...`)
/// - **Windows**: `%ProgramData%\cosmostrix\config.toml`
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
pub(crate) fn default_config_file_path() -> PathBuf {
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
/// "com.termux". This is the single source of truth for Termux detection
/// in the codebase — `safepath.rs` and `verbose.rs` both call this
/// function instead of inlining their own env-var checks.
#[must_use]
pub(crate) fn is_termux_environment() -> bool {
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
pub(crate) fn resolve_watcher_config_path(cli_config: Option<&Path>) -> (PathBuf, Vec<PathBuf>) {
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
pub(crate) fn config_candidate_paths() -> Vec<PathBuf> {
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
#[cfg(test)]
#[must_use]
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
pub(crate) fn dump_config_text() -> &'static str {
    r##"# cosmostrix configuration

# Override priority: CLI flags > config.toml > scene defaults.
# Examples:
#   cosmostrix -c neon-green --speed 20    # CLI overrides config
#   cosmostrix --scene-custom hacker-mode  # user custom scene
#   cosmostrix --testconf                  # validate this config
#   cosmostrix --doctor                    # diagnose terminal issues

# File Location:
#   Linux:     ~/.config/cosmostrix/config.toml (or $XDG_CONFIG_HOME)
#              /etc/cosmostrix/config.toml (system-wide)
#   macOS:     ~/.config/cosmostrix/config.toml (or $XDG_CONFIG_HOME)
#              /Library/Application Support/cosmostrix/config.toml (system-wide)
#   FreeBSD:   ~/.config/cosmostrix/config.toml
#              /usr/local/etc/cosmostrix/config.toml (system-wide)
#   Termux:    ~/.config/cosmostrix/config.toml (XDG_CONFIG_HOME ignored)
#              $PREFIX/etc/cosmostrix/config.toml (system-wide)
#   Windows:   %APPDATA%\cosmostrix\config.toml
#              %ProgramData%\cosmostrix/config.toml (system-wide)

# Format:
#   key = value              # one per line
#   # comments               # blank lines ignored
#   [section.name]           # TOML table header (groups keys under it)
#   field = value            # keys inside a table are prefixed automatically
#   Custom blocks support BOTH flat (scene-custom.name.field = value)
#   and TOML table ([scene-custom.name] + field = value) formats.
#   Malformed lines (no '=' or empty key/value) cause --testconf to FAIL.

# All keys below are commented out. Uncomment the ones you want to
# customize. Built-in defaults are shown for reference. Run
# `cosmostrix --testconf` to validate your config after editing.

# ── Standard Settings ──

# Scene — built-in atmospheric template.
#   cinematic (default) | matrix | monolith | signal | classic | calm |
#   storm | cosmos | neon | hacker | low-power | matrix_film | cosmic-dragon |
#   carbonic | dragon-crystal | orange-cat | north-stars | curiosity
# scene = cinematic

# Color scheme (palette). See: cosmostrix --list-colors
# color = neon-purple

# Character set for rain glyphs. See: cosmostrix --list-charsets
# charset = zen

# Background mode: default-background (follow terminal) | black (solid #000000)
# color-bg = default-background

# Cinematic intro animation: logo | cosmic | none (default: logo)
# intro = "logo"

# ── Motion ──

# Target FPS (1-240). Loop sleeps to maintain this cap in interactive mode.
# Press 'i' to see it as `tgt:` in the HUD. In --benchmark mode this sets
# the simulation rate only.
# When unset, the default is dynamic: 60 FPS on standard terminals, 144 FPS
# on high-refresh terminals (Alacritty, kitty, WezTerm, etc.) — see
# `cosmostrix --verbose` fps_precedence line for which layer resolved.
# fps = 60

# Rain fall speed (1-100). Default depends on scene (cinematic=9).
# speed = 9

# Rain density (0.01-5.0). Default depends on scene (cinematic=0.75).
# density = 0.75

# Variable column speeds for organic rain (default: on).
# async-mode = true

# Monolith pillar size: small | normal | large — only for monolith scene.
# monolith-size = normal

# ── Behavior ──

# Glitch intensity: none | subtle | default | intense.
# The preset fully controls glitch percent, stream decay, fragmented stream
# chance, and stream layering automatically — no separate keys.
# glitch-level = subtle

# Auto color drift (default: off).
# auto-color-drift = false

# ── Advanced Style ──

# Bold style: 0=off, 1=random (default), 2=all.
# bold = 1

# Shading mode: 0=random, 1=cinematic (default — distance from head).
# shadingmode = 1

# Color tuning (0.0-3.0 each, default 1.0 = no change):
# [color.tune]
# brightness = 1.0   # global brightness (dim-rain: use < 1.0)
# saturation = 1.0   # color saturation (0.0 = grayscale)
# head = 1.0         # head segment brightness
# body = 1.0         # body segment brightness
# tail = 1.0         # tail segment brightness

# ── Custom Scenes ──
# Define named custom scenes and load with: cosmostrix --scene-custom <name>.
# v30.3 field allowlist (the ONLY keys accepted inside [scene-custom.<name>]):
#   base-scene, color, charset, bold, colors-custom, charset-custom,
#   shadingmode, glitch-level, fps, speed, density, density-map, async.
# Forbidden keys (rejected as unknown by --testconf): ambient,
# auto-color-drift, color.tune, monolith-size, intro, color-bg.
# Missing fields fall back to cinematic's defaults (or to base-scene's
# defaults if base-scene is set). Custom scenes are listed in --list-scenes.
#
# Ordering: once you write a [section] header, every flat key AFTER it
# belongs to that section until the next header. Prefer writing top-level
# keys BEFORE any [section] block.
#
# Paired fields (don't mix them up — `--testconf` will hint if you do):
#   color          — built-in color name only (`cosmostrix --list-colors`)
#   colors-custom  — name of a [colors-custom.<name>] block (see below)
#   charset        — built-in charset preset only (`cosmostrix --list-charsets`)
#   charset-custom — name of a [charset-custom.<name>] block (see below)
# Inside [scene-custom.<name>] blocks, the `color` and `charset` fields
# NEVER accept custom-block names. If you write `color = mypalette` where
# `mypalette` is a `[colors-custom.mypalette]` block, --testconf rejects
# it with a hint pointing at `colors-custom = mypalette` (and the same
# symmetric hint for `charset` → `charset-custom`).

# [scene-custom.hacker-mode]
# base-scene = matrix       # v30.2: inherit matrix's rain_style + defaults
# color = green             # override matrix's neon-green with plain green
# charset = hacker          # override matrix's matrix charset
# bold = 1                  # v30.3: 0=off, 1=on, 2=double-width
# colors-custom = zen       # v30.3: reference a [colors-custom.<name>] block
# charset-custom = pipes    # v30.3: reference a [charset-custom.<name>] block
# shadingmode = 1           # v30.3: 0=off, 1=on
# speed = 28                # override matrix's speed=18
# density = 1.2             # override matrix's density=0.65
# glitch-level = intense    # override matrix's glitch=Subtle
# fps = 60                  # override cinematic's fps=60
# async = false             # v30.3: true enables async render path
#
# v30.2: base-scene is the inheritance anchor. When set, the custom scene
# inherits ALL scene-managed defaults from the named built-in scene before
# applying its own overrides. Without base-scene, missing fields fall back
# to cinematic's defaults.
#
# Without base-scene (legacy v20.1+ behavior):
# [scene-custom.hacker-mode]
# color = green
# charset = hacker
# speed = 28
# density = 1.2
# glitch-level = intense
#
# v30.3: `ambient.<HH-MM>` MUST live at the top level (NEVER inside a
# [scene-custom.<name>] block). Putting it inside a scene-custom section
# makes TOML parse it as `scene-custom.<name>.ambient.<HH-MM>`, which is
# rejected as an unknown key. Define the scene first, then reference it
# by name from a top-level `ambient.<HH-MM> = <scene-name>` entry.

# Density Map: per-column spawn weights (0.0=never, 1.0=always). Maps
# shorter than terminal width treat missing columns as 1.0. Both quoted
# and unquoted forms work.
#   density-map = 0.05,0.3,1.0           (unquoted — standard)
#   density-map = "0.05,0.3,1.0"         (quoted — also valid)

# ── Custom Color Palettes ──
# Define named custom palettes and load with: cosmostrix --colors-custom <name>.
# Reference from a [scene-custom.<name>] block via: colors-custom = <name>
# (NOT `color = <name>` — that field is for built-in colors only).
# Hex values use #rrggbb notation. ALWAYS quote hex strings: "#ff0000"
# (unquoted # is treated as a TOML comment, silently truncating the value).
# rain = 7 hex gradient stops (tail → head order). Minimum 2, 7 recommended.

# [colors-custom.zen]
# bg = "#0a0a0a"
# rain = [
#  "#111111",  # tail dimmer
#  "#2a2a2a",  # tail dim
#  "#4a4a4a",  # semi-body dark
#  "#6a6a6a",  # body peak
#  "#8a8a8a",  # semi-body light
#  "#b0b0b0",  # semi-white
#  "#d0d0d0",  # head glow
# ]

# ── Custom Character Sets ──
# Define named custom charsets and load with: cosmostrix --charset <name>
# (or: top-level `charset = "name"` in config — custom names take precedence
# over built-in presets with the same name).
# Reference from a [scene-custom.<name>] block via: charset-custom = <name>
# (NOT `charset = <name>` — inside scene-custom blocks, that field is for
# built-in presets only and --testconf will reject a custom name with a hint).
# Fields:
#   set — literal string of characters to use as the rain glyph pool.
#   Whitespace (except ASCII space) skipped. Control chars rejected.
#   Wide/zero-width chars (emoji, CJK fullwidth) auto-filtered.

# [charset-custom.zen]
# set = "|"

# ── Ambient Phase Scheduler ──
# Schedule time-of-day phase transitions. Config-only (no CLI flag).
# Each entry switches the active scene at the specified wall-clock minute
# and stays active until the next entry's boundary. Instant switch (no
# blend window). Dynamic idle/wake scheduler thread — zero CPU between
# phase boundaries.
#
# v30.2 format (simplified — breaking change from v30.1):
#   ambient.<HH-MM> = <scene-name>
#
# The value is a SINGLE scene name — either a built-in scene (cinematic,
# signal, monolith, etc.) OR a custom scene defined via [scene-custom.<name>].
# All parameters (color, charset, speed, density, fps, glitch-level, rain_style)
# live inside the scene itself. This eliminates the v30.1 override-precedence
# bugs where speed=50 was silently overridden by the scene's default speed.
#
# - HH-MM: 24-hour time, zero-padded (00-00 to 23-59).
# - scene-name: built-in OR [scene-custom.<name>] block.
# - Wrap-around: if now is 0:30 and the earliest entry is 6:00, the
#   "current" phase is the LAST entry of the previous day (carried over).
# - Live reload: edits take effect immediately on save.
# - Max 256 entries (a healthy schedule has 2-6).
#
# Migration from v30.1 multi-field format:
#   v30.1: ambient.15-00 = neon-purple, signal, speed=50, density=0.65
#   v30.3: define the scene, then reference it at the TOP LEVEL:
#         [scene-custom.afternoon]
#         base-scene = "signal"
#         color = "neon-purple"
#         speed = "50"
#         density = "0.65"
#
#         ambient.15-00 = afternoon    # top-level — NEVER inside the block
#
# Working Example: 3-phase day/night cycle (v30.2)
#   ambient.06-00 = signal
#   ambient.12-00 = monolith
#   ambient.20-00 = cinematic
#
# Custom-scene Example: define once, reference by name.
# Define the scene in its own [scene-custom.<name>] block, then reference
# it from a TOP-LEVEL `ambient.<HH-MM> = <name>` entry. NEVER place the
# ambient entry inside the [scene-custom.<name>] block — that produces
# `scene-custom.<name>.ambient.<HH-MM>` and is rejected as unknown.
#   [scene-custom.afternoon]
#   base-scene = "signal"
#   color = "neon-purple"
#   speed = "50"
#   density = "0.65"
#
#   ambient.15-00 = afternoon    # top-level — switches to afternoon at 15:00
#
# Minimal Example: 2-phase day/night
#   ambient.07-00 = matrix
#   ambient.19-00 = monolith

# ambient.06-00 = signal
# ambient.12-00 = monolith
# ambient.20-00 = cinematic

# ── Removed Keys (rejected by --testconf) ──
# adaptive-custom.*  — atmosphere engine eliminated; use ambient.* instead.
"##
}

/// Build the full dump-config output with a generated header prepended.
///
/// The header is 3 comment lines:
///   ```text
///   # cosmostrix config file
///   # generated at <ISO 8601 UTC>
///   # using Howard Hinnant chrono design (libc::gmtime_r)
///   ```
/// followed by a blank `#` line, then the existing curated `# cosmostrix
/// configuration` template from `dump_config_text()`.
///
/// v30 (Hinnant-style): the timestamp is produced by `clock::now_iso_utc()`
/// which uses direct `libc::gmtime_r` on Unix — no `chrono` dependency. The
/// "Howard Hinnant chrono design" attribution honors the algorithm
/// (civil-from-days + minimal abstraction) without claiming the chrono crate
/// is in use (it was dropped in v30 to eliminate 8 transitive deps).
///
/// Returns a `String` (allocates) instead of `&'static str` because the
/// timestamp is runtime-generated. Callers: `--dump-config` stdout path and
/// `--dump-config <path>` file-write path in `main.rs`.
#[must_use]
pub(crate) fn dump_config_with_header() -> String {
    let ts = crate::clock::now_iso_utc();
    format!(
        "# cosmostrix config file\n# generated at {ts}\n# using Howard Hinnant chrono design (libc::gmtime_r)\n#\n{}",
        dump_config_text()
    )
}

#[must_use]
pub(crate) fn known_keys() -> Vec<&'static str> {
    USER_CONFIG_KEYS
        .iter()
        .chain(std::iter::once(&PROFILE_CONFIG_KEY_HINT))
        .chain(std::iter::once(&SCENE_CUSTOM_CONFIG_KEY_HINT))
        .chain(std::iter::once(&COLORS_CUSTOM_CONFIG_KEY_HINT))
        .chain(std::iter::once(&CHARSET_CUSTOM_CONFIG_KEY_HINT))
        .chain(std::iter::once(&COLOR_TUNE_CONFIG_KEY_HINT))
        .chain(std::iter::once(&AMBIENT_CONFIG_KEY_HINT))
        .copied()
        .collect()
}

#[inline]
fn is_known_key(key: &str) -> bool {
    USER_CONFIG_KEYS.contains(&key)
        || is_profile_config_key(key)
        || is_scene_custom_config_key(key)
        || is_colors_custom_key(key)
        || is_charset_custom_key(key)
        || is_color_tune_key(key)
        || crate::ambient::is_ambient_config_key(key)
}

/// v17: Check if key matches `color.tune.<field>` pattern.
#[inline]
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

/// Check if `key` matches `colors-custom.<name>.<field>` pattern.
/// Recognized fields: `bg`, `rain` (canonical), `stops` (deprecated alias for `rain`).
/// Invalid fields surface as `unknown_keys` so `config_hints` can attach a hint.
/// Name must be non-empty, ASCII alphanumeric + `-`/`_` only.
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
///
/// v25.10 (bug #8): tightened from `bg | background | rain` to
/// `bg | rain | stops`. `background` (undocumented alias) was removed —
/// use `bg`. `stops` is a deprecated alias for `rain` (still accepted,
/// `--testconf` emits a deprecation warning). Brings the key-checker in
/// sync with `validate_colors_custom_value`, which already handled `.stops`.
#[inline]
fn is_valid_colors_custom_field(field: &str) -> bool {
    matches!(field, "bg" | "rain" | "stops")
}

/// Check if `key` matches `charset-custom.<name>.set` pattern.
/// v25: replaces legacy `--charset-file <PATH>` CLI flag. Only `set`
/// field is recognized. Name must be non-empty, ASCII alphanumeric +
/// `-`/`_` only (same rule as `colors-custom`).
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

/// v25.9 (bug #7): Detect unquoted '#' INSIDE an array value.
/// Returns `Some(byte_idx)` if the line has an unquoted '#' while bracket
/// depth > 0. Catches `rain = [#ff0000, #00ff00]` (user mistake — should
/// quote hex). Returns `None` for legitimate cases: quoted '#' inside
/// strings, or '#' AFTER the closing ']' (trailing comment).
#[inline]
pub(crate) fn unquoted_hash_inside_array(line: &str) -> Option<usize> {
    let mut in_dquote = false;
    let mut in_squote = false;
    let mut bracket_depth: i32 = 0;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' if !in_squote => in_dquote = !in_dquote,
            '\'' if !in_dquote => in_squote = !in_squote,
            '[' if !in_dquote && !in_squote => bracket_depth += 1,
            ']' if !in_dquote && !in_squote => bracket_depth -= 1,
            '#' if !in_dquote && !in_squote && bracket_depth > 0 => {
                return Some(i);
            }
            _ => {}
        }
    }
    None
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
        // v30.2: `base-scene` is restored as a recognized profile/scene-custom
        // field (with cleaner inheritance semantics — see profile.rs). It
        // must be stored in values, NOT flagged as unknown.
        let parsed = parse_config_text(
            "profile.nightcore.base-scene = monolith\nprofile.nightcore.color = purple\n",
        );
        // base-scene is recognized and stored.
        assert_eq!(
            parsed
                .values
                .get("profile.nightcore.base-scene")
                .map(String::as_str),
            Some("monolith")
        );
        assert!(
            !parsed
                .unknown_keys
                .contains(&"profile.nightcore.base-scene".to_string()),
            "base-scene must NOT be flagged as unknown in v30.2"
        );
        // color is also recognized and stored.
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
        // v30+: ambient phase scheduler must be documented with at least one
        // live, uncommented-as-comment example so users can copy-paste.
        assert!(
            dump.contains("ambient.06-00"),
            "dump config should include an ambient.<HH-MM> example"
        );
        assert!(
            dump.contains("Ambient Phase Scheduler"),
            "dump config should include an Ambient section header"
        );
    }

    #[test]
    fn dump_config_documents_paired_field_split() {
        // v30.3: the dump-config template must explicitly document the
        // split between `color` (built-in) and `colors-custom` (custom
        // block ref), and the symmetric split for `charset` vs
        // `charset-custom`. This prevents the duplicate-usage confusion
        // reported by the owner. The note is enforced by content anchor —
        // if a future edit removes the "Paired fields" header, this test
        // fails loudly.
        let dump = dump_config_text();
        assert!(
            dump.contains("Paired fields"),
            "dump config should include the 'Paired fields' note (color vs colors-custom, charset vs charset-custom)"
        );
        // Each paired field's doc line should also appear in the custom
        // palettes / custom charsets sections, pointing users at the right
        // reference field.
        assert!(
            dump.contains("colors-custom = <name>"),
            "Custom Color Palettes section should show how to reference a block from a scene-custom"
        );
        assert!(
            dump.contains("charset-custom = <name>"),
            "Custom Character Sets section should show how to reference a block from a scene-custom"
        );
    }

    #[test]
    fn dump_config_with_header_starts_with_header_lines() {
        // v30: the generated config must start with the 3-line header +
        // blank `#` line, then the existing `# cosmostrix configuration`
        // template body.
        let dump1 = dump_config_with_header();
        let lines: Vec<&str> = dump1.lines().collect();
        assert!(lines.len() >= 5, "header should have >= 5 lines");
        assert_eq!(lines[0], "# cosmostrix config file", "header line 1");
        // Line 2: `# generated at <ISO 8601 UTC>`
        let line2 = lines[1];
        assert!(
            line2.starts_with("# generated at "),
            "header line 2 wrong: {line2:?}"
        );
        let ts = line2.trim_start_matches("# generated at ");
        assert!(
            ts.len() == 20 && ts.ends_with('Z') && ts.as_bytes()[10] == b'T',
            "timestamp not RFC 3339: {ts:?}"
        );
        // Line 3: Hinnant attribution
        assert_eq!(
            lines[2], "# using Howard Hinnant chrono design (libc::gmtime_r)",
            "header line 3"
        );
        // Line 4: blank `#` separator
        assert_eq!(lines[3], "#", "blank separator");
        // Line 5: existing template body starts
        assert_eq!(
            lines[4], "# cosmostrix configuration",
            "template body start"
        );
    }

    #[test]
    fn dump_config_with_header_includes_all_keys() {
        // The header prepended must not break the existing key-coverage test.
        let dump = dump_config_with_header();
        for key in USER_CONFIG_KEYS.iter() {
            assert!(
                dump.contains(*key),
                "header'd dump should still mention {key}"
            );
        }
        assert!(dump.contains("[scene-custom.hacker-mode]"));
        // v30+: ambient example must survive the header prepend.
        assert!(
            dump.contains("ambient.06-00"),
            "header'd dump should still include ambient.<HH-MM> example"
        );
    }

    #[test]
    fn parse_multiline_array_joins_correctly() {
        let content = "[colors-custom.zen]\nbg = \"#0a0a12\"\nrain = [\n  \"#1a0033\",\n  \"#4d0080\",\n  \"#9933ff\",\n  \"#cc66ff\",\n  \"#e6b3ff\",\n  \"#f2ccff\",\n  \"#ffffff\",\n]\n";
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
        let rain = parsed.values.get("colors-custom.zen.rain");
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
