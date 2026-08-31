// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Configuration file support for cosmostrix.
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
    "crystal-dragon",
    // v50: Power Dragon toggle. When false, disables aggressive_throttle
    // and idle FPS reduction — rain stays at user-configured density/speed
    // regardless of CPU pressure. Default: true (protection enabled).
    "power-dragon",
    "async-mode",
    // (CLI-D-1 fix): `adaptive-custom` removed from this whitelist.
    // The atmosphere engine was eliminated at commit 07b44b5 (2026-08-05),
    // and `config_hints.rs` + `testconf.rs` both explicitly reject
    // `adaptive-custom.*` keys with "have been removed" messages. But the
    // bare key was still classified as "known" here, so a stale
    // `adaptive-custom = "10-00, energy-zen, signal"` line was silently
    // stored and never applied at runtime — zero warning at startup, only
    // `--testconf` caught it. Now the bare key falls into `unknown_keys`
    // and triggers the startup rejection at `config_apply.rs:149-163`.
    // v20: Cinematic intro selector. Values: "logo" | "cosmic" | "none".
    // Default: "logo". CLI --intro flag wins over this config key.
    "intro",
    // v50: Intro color override. Allows the intro animation to use a
    // different color theme than the rain. Values: any builtin theme
    // name (e.g. "energy-zen", "neon-green") or custom palette name
    // (e.g. "cyberpunk_2077"). Default: same as --color (rain color).
    // Use --list-colors to see available builtin themes.
    "intro-color",
    // v50: Overlay message text. Two config keys mirror the CLI flags:
    //   message         = "text"  → message WITHOUT border (matches -m)
    //   message-border  = "text"  → message WITH border    (matches -mb)
    // If both are present in config, `message-border` wins (border=true).
    // CLI -m / -mb wins over either config key. When none of CLI/config
    // provides a message, interactive mode defaults to a bordered overlay
    // showing the project name (see main.rs CloudConfig construction).
    // Benchmark mode never shows a message overlay.
    "message",
    "message-border",
    // v50-beta.3: msg-mode master switch for the message overlay subsystem.
    // When false, disables BOTH the default message AND any message/
    // message-border config key (config-only message is suppressed).
    // CLI -m / -mb always wins over msg-mode=false (CLI precedence).
    // Default: true (message overlay active).
    "msg-mode",
    // v51 msg-fill-style: message overlay reveal style. Values:
    // "typewriter" | "fade" | "words" | "slide" | "pulse" | "instant"
    // | "engrave" | "hologram" | "glitch".
    // Default: "typewriter". CLI -mfs/--msg-fill-style wins over this
    // config key. Case-insensitive here (config surface is forgiving;
    // the CLI flag itself is case-sensitive, like every other enum key).
    "msg-fill-style",
    // v50.0.0-beta.7: Config-tunable ambient auto-snapback delay (seconds).
    // After the user presses x/c/s (manual override) and is then idle for
    // this many seconds, the event loop re-applies the current ambient
    // phase. Range: 0.0..=86400.0. Default: 30.0 (when unset). Setting to
    // 86400 (24h) effectively disables snapback; 0.0 means instant.
    "ambient-snapback-secs",
];

const SCENE_CUSTOM_CONFIG_KEY_HINT: &str = "scene-custom.<name>.<base-scene|color|charset|bold|colors-custom|charset-custom|shadingmode|glitch-level|fps|speed|density|density-map|async-mode>";
const COLORS_CUSTOM_CONFIG_KEY_HINT: &str = "colors-custom.<name>.<bg|rain|stops>";
const CHARSET_CUSTOM_CONFIG_KEY_HINT: &str = "charset-custom.<name>.set";
const COLOR_TUNE_CONFIG_KEY_HINT: &str = "color.tune.<brightness|saturation|head|body|tail>";
/// Ambient phase scheduler: `ambient.<HH-MM> = <scene-name>`.
///
/// simplified — value is a single scene name (built-in OR custom).
/// Config-only (no CLI flag). Time-of-day phase entries that switch the
/// active scene at scheduled times. Instant switch (no blend window).
/// Dynamic idle/wake scheduler thread — zero CPU between phase boundaries.
/// See `src/crystal_dragon_engine/ambient/mod.rs` and `src/crystal_dragon_engine/ambient_scheduler/mod.rs`.
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
    /// keys that were auto-promoted from a nested section to root scope.
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

            // (bug #19, owner-found 2026-08-30): a quoted value is NEVER
            // an array. `set = "["` (single-bracket charset glyph) used to
            // quote-strip to `[` and get mistaken by the multi-line array
            // consumer below for an unterminated array. Snapshotting the
            // quoted form BEFORE stripping and guarding both array
            // branches with it closes the whole class: quoted values may
            // contain `[`, `]`, `#`, `=` — strings, not syntax.
            let raw_is_quoted = value.len() >= 2 && value.starts_with('"') && value.ends_with('"');

            // Option 1 (internal independent QA): strip surrounding double
            // quotes from string values. Standard TOML requires string
            // values to be quoted ("value"), but cosmostrix's custom parser
            // previously stored the raw value INCLUDING the quotes. This
            // caused `intro = "logo"` to fail because the consumer saw
            // `"logo"` (with quotes) instead of `logo`. Now we strip
            // matching leading/trailing double quotes so both
            // `intro = "logo"` and `intro = logo` produce the same stored
            // value `logo`. This also fixes the template inconsistency
            // where some values were quoted (intro, bg, set) and others
            // were not (scene, color, charset).
            //
            // Only strip if the value starts AND ends with a double quote
            // (both must be present — a lone leading quote is not stripped).
            // Arrays (raw values starting with `[`) never match: they are
            // not quoted strings, and their internal quotes are handled by
            // the array consumer (hex colors). Stripping runs AFTER the
            // raw_is_quoted snapshot so the array branches can tell a
            // quoted string whose CONTENT starts with `[` from an array.
            if raw_is_quoted {
                value = value[1..value.len() - 1].to_string();
            }

            // (bug #7): Detect unquoted '#' inside an array value.
            // strip_inline_comment strips at first unquoted '#'. If that
            // happened while bracket depth > 0, the user wrote e.g.
            // `rain = [#ff0000, #00ff00]` — '#' inside array was treated
            // as comment, silently truncating value to '[' and triggering
            // the multi-line consumer to eat subsequent lines. Reject
            // explicitly. We check the ORIGINAL line (not `stripped`)
            // because `stripped` is already truncated. Quoted values are
            // exempt (bug #19): a quoted string is never an array.
            if !raw_is_quoted
                && value.starts_with('[')
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
            // we find the closing ']'. (bug #7) hardening: do NOT
            // consume [section] headers — those were previously mistaken
            // for the closing ']' of the array, corrupting subsequent
            // block definitions. (bug #19): quoted values never enter
            // this branch (a quote-stripped set = "[" is a STRING).
            if !raw_is_quoted && value.starts_with('[') && !value.ends_with(']') {
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
                // Auto-promote forgiving parser. If the un-prefixed
                // key is itself a known top-level key, the user accidentally
                // nested it under a [section] header (very common when
                // mixing [scene-custom.<name>] with top-level keys like
                // `fps` or `speed`). Silently re-home it to root scope and
                // record the promotion so --testconf can warn the user
                // about the structural fix.
                //
                // v50.0.0-beta.6 FATAL FIX: do NOT auto-promote when inside
                // a custom block (charset-custom.*, colors-custom.*,
                // scene-custom.*). These blocks have a strict field
                // allowlist — unknown fields must be rejected as
                // unknown_keys, NOT promoted to root scope. Previously,
                // `color = green` inside `[charset-custom.quantum]` was
                // promoted to root `color = green`, silently changing the
                // global color scheme. Now it surfaces as an unknown key
                // so config_hints can attach a helpful error message.
                let is_custom_block = current_section.starts_with("charset-custom.")
                    || current_section.starts_with("colors-custom.")
                    || current_section.starts_with("scene-custom.");
                if !current_section.is_empty() && !is_custom_block && is_known_key(&key) {
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
///   `XDG_CONFIG_HOME` is deliberately IGNORED (see "Termux fix"
///   below) because it may point to `$PREFIX/etc`, a system location
///   users don't edit.
/// - **Windows**: Uses `%APPDATA%\cosmostrix\config.toml` (always absolute).
///
/// System-wide fallback locations (consulted by `resolve_config_path`
/// when the user-specific path doesn't exist):
/// - **Linux**: `/etc/cosmostrix/config.toml`
/// - **macOS**: `~/Library/Application Support/cosmostrix/config.toml`
/// - **FreeBSD**: `/usr/local/etc/cosmostrix/config.toml`
///   (FreeBSD uses `/usr/local/etc` for ports/packages, not `/etc`)
/// - **Android (Termux)**: `$PREFIX/etc/cosmostrix/config.toml`
///   (typically `/data/data/com.termux/files/usr/etc/cosmostrix/...`)
/// - **Windows**: `%ProgramData%\cosmostrix\config.toml`
///
/// Looks for `config.toml`. removed the pre-v10 `config` (no
/// extension) fallback — users upgrading from pre-v10 must rename their
/// file to `config.toml`.
///
/// **Termux fix**: On Android Termux, the XDG spec is ambiguous —
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

        // Termux fix: on Termux, $HOME/.config/cosmostrix/config.toml
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
/// Termux fix: this function existed conceptually but was inlined
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

// Cross-platform path resolution helper.
// v50.0.0-beta.4+: removed `#[cfg(not(target_os = "windows"))]` guard so that
// inline tests (`configfile_tests_inline.rs`) can compile and run on all
// platforms including Windows (MSVC). The underlying logic uses
// platform-agnostic PathBuf operations that work correctly on Windows.
#[cfg(test)]
#[must_use]
pub fn config_file_path_from(xdg_config_home: Option<String>, home: Option<String>) -> PathBuf {
    config_file_path_from_env(
        xdg_config_home.as_deref(),
        home.as_deref(),
        CONFIG_FILE_NAME,
    )
}

// Cross-platform implementation (works on Windows, Linux, macOS).
// Uses PathBuf operations that are platform-agnostic.
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
pub(crate) fn known_keys() -> Vec<&'static str> {
    USER_CONFIG_KEYS
        .iter()
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
        || is_scene_custom_config_key(key)
        || is_colors_custom_key(key)
        || is_charset_custom_key(key)
        || is_color_tune_key(key)
        || crate::crystal_dragon_engine::ambient::is_ambient_config_key(key)
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
/// (bug #8): tightened from `bg | background | rain` to
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

/// (bug #7): Detect unquoted '#' INSIDE an array value.
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

// v50.0.0-beta.7 LOC refactor: dump_config_text + dump_config_with_header
// + sha512_hex + extract_template_fingerprint extracted to
// configfile_dump.rs. Re-exported here so all call sites resolve unchanged.
mod configfile_dump;
pub(crate) use configfile_dump::{
    dump_config_text, dump_config_with_header, extract_template_fingerprint, sha512_hex,
};

#[cfg(test)]
#[path = "configfile_tests_inline.rs"]
mod configfile_tests_inline;
