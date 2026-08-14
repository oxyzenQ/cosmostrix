// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Apply config file values, scene defaults, scene-custom, and glitch-level
//! cross-cutting merge to parsed CLI args.
//!
//! Precedence (highest wins — actual 5-level chain as wired in
//! `apply_config_and_runtime_defaults`):
//! 1. Built-in clap defaults (the floor — every field has one)
//! 2. Config file values (always wins over scene defaults for user-set keys)
//! 3. Default scene values (only fills keys NOT set in config — scene is a
//!    template for unset keys, not an override for user-set keys)
//! 4. CLI scene / scene-custom (only fills unset keys; respects config-set
//!    keys). `[profile.<name>]` config blocks are surfaced inside
//!    `scene_custom.rs`'s available-scenes list, but are NOT applied at
//!    runtime — users must rename the prefix to `scene-custom`.
//! 5. `--glitch-level` cross-cutting merge: applies the preset's
//!    glitch_pct/shortpct/rippct/glitch_ms values, overriding any clap
//!    defaults. `max_droplets_per_column` is NOT derived from glitch-level
//!    (stays at the clap default 3).
//!
//! Key rule: a value explicitly set in config.toml ALWAYS wins over a scene's
//! hardcoded default. Scenes are templates for *unset* keys, not overrides for
//! user-set keys. This prevents the surprise where `speed = 30` in config gets
//! silently overwritten by a scene's `speed = 8`.
//!
//! Historical note: v14/v17/v20 purges removed `--preset`, `--profile` (as a
//! standalone CLI flag), and `--low-power`. Their behavior was absorbed into
//! `--scene` and `--scene-custom`. Old doc comments listing 10 precedence
//! levels (with separate "config preset", "CLI preset", "CLI profile",
//! "low-power" layers) were stale — those layers no longer exist as separate
//! functions. This comment was rewritten in the Phase 5 config-sync audit to
//! match the actual wiring.

use std::collections::{HashMap, HashSet};

use clap::parser::ValueSource;
use clap::ValueEnum;

use crate::charset::charset_from_str;
use crate::cli::parse_color_scheme;
use crate::config::{Args, ColorBg, GlitchLevel, IntroType};
use crate::constants::{DENSITY_CLAMP_MAX, SPEED_MAX, SPEED_MIN};
use crate::runtime::MonolithSize;
use crate::scene::{get_scene, validate_scene_name, DEFAULT_SCENE};
use crate::scene_custom::apply_scene_custom_layer;
use crate::validation::{
    parse_canonical_f32_range, parse_canonical_f64_range, parse_canonical_speed,
    parse_canonical_u8_range,
};

pub(crate) fn apply_config_and_runtime_defaults(
    matches: &clap::ArgMatches,
    args: &mut Args,
) -> Result<(), String> {
    // Phase 5 closure (P3-5): reset the startup warning counter at the start
    // of config apply. Individual warnings (from profile/scene-custom
    // warn_invalid, etc.) increment it via eprintln_warn_labeled. We emit a
    // summary line at the end if any warnings were emitted, so users don't
    // miss them in noisy startup output.
    crate::output::reset_startup_warning_count();

    let mut config_touched = HashSet::new();

    // Security: validate --config path is in a safe location AND has .toml extension.
    // Centralized in safepath::validate_config_path so testconf, --show-scene,
    // --colors-custom, and --scene-custom all apply the same check consistently.
    // On Windows, also resolve %APPDATA% etc. so the expanded path is used for
    // file I/O (the OS doesn't understand %VAR% in paths).
    if let Some(ref config_path) = args.config {
        let path_str = config_path.to_string_lossy();
        let resolved = crate::validate_config_path(&path_str, args.verbose)?;
        // On Windows, override args.config with the resolved path so
        // load_config_file_full reads from the expanded %APPDATA% path.
        // On non-Windows, resolved == path_str (no-op).
        if resolved != path_str {
            args.config = Some(std::path::PathBuf::from(&resolved));
        }
    }

    // Phase 5 closure (P4-8): use load_config_file_full to get the full
    // ParsedConfig (values + malformed_lines + unknown_keys) in ONE disk
    // read. Previously this used load_config_file (which drops malformed/
    // unknown) and then re-read + re-parsed the file at line 200 to recover
    // them — a redundant ~200μs disk read on every startup.
    let parsed_cfg = crate::configfile::load_config_file_full(args.config.as_deref());
    let cfg = parsed_cfg.values;
    if args.verbose {
        // Show the ACTUALLY-RESOLVED config path (with system fallback),
        // not just the default user path. After a --system install where
        // only /etc/cosmostrix/config.toml exists, the default user path
        // doesn't exist — showing it would be misleading.
        let config_path_display = args
            .config
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                let (resolved, _) =
                    crate::configfile::resolve_watcher_config_path(args.config.as_deref());
                resolved.to_string_lossy().into_owned()
            });
        crate::output::eprintln_verbose_raw(&format!(
            "config loaded from: {config_path_display} ({} keys)",
            cfg.len()
        ));
        // List the actual keys so the user can see exactly what is set.
        // This is critical for debugging config issues — without this list,
        // the user only sees "(N keys)" and has to manually re-read the
        // config file to figure out which keys are active.
        if !cfg.is_empty() {
            let mut keys: Vec<&str> = cfg.keys().map(String::as_str).collect();
            keys.sort();
            crate::output::eprintln_verbose_raw(&format!("config keys: {}", keys.join(", ")));
        }
    }

    // Strict startup validation: if config has ANY error (malformed lines,
    // unknown keys, or invalid values), exit. This matches --testconf
    // behavior: invalid config = exit code 2, not silent fallback.
    //
    // Phase 5 closure (P4-8): we now have malformed_lines + unknown_keys from
    // the single load_config_file_full call above — no redundant re-read.
    //
    // Test bypass: COSMOSTRIX_SKIP_STARTUP_VALIDATION=1 skips this check
    // so existing tests that verify apply/fallback logic with invalid values
    // still work. Production builds never set this env var.
    if !cfg.is_empty() && std::env::var("COSMOSTRIX_SKIP_STARTUP_VALIDATION").is_err() {
        // Layer 1: malformed lines (stray text without 'key = value')
        if !parsed_cfg.malformed_lines.is_empty() {
            let lines: Vec<&str> = parsed_cfg
                .malformed_lines
                .iter()
                .take(3)
                .map(String::as_str)
                .collect();
            return Err(format!(
                "error: invalid config — malformed line(s): '{}' (expected 'key = value' syntax)\n\n  Fix the error above, or run 'cosmostrix --testconf' for details.",
                lines.join(", ")
            ));
        }

        // Layer 2: unknown keys (typos)
        if !parsed_cfg.unknown_keys.is_empty() {
            let keys: Vec<&str> = parsed_cfg
                .unknown_keys
                .iter()
                .take(3)
                .map(String::as_str)
                .collect();
            // v25.6 depth-test fix: targeted "did you mean" hints for
            // structural TOML mistakes (e.g. bold under [color.tune]).
            let hints = crate::config_hints::format_hints_block(&parsed_cfg.unknown_keys);
            return Err(format!(
                "error: invalid config — unknown key(s): '{}' (run 'cosmostrix --testconf' for known keys){hints}\n\n  Fix the error above, or run 'cosmostrix --testconf' for details.",
                keys.join(", ")
            ));
        }

        // Layer 3: invalid values (out of range, unknown enum, etc.)
        if let Err(msg) = crate::testconf::validate_config_strictly(&cfg) {
            return Err(format!(
                "error: invalid config — {msg}\n\n  Fix the error above, or run 'cosmostrix --testconf' for details."
            ));
        }
    }

    if !cfg.is_empty() {
        apply_config_values(matches, args, &cfg, &mut config_touched);
    }

    let scene_is_cli = is_explicit(matches, "scene");
    let scene_custom_is_cli = is_explicit(matches, "scene_custom");
    let scene_is_default = args.scene.is_none();
    if scene_is_default {
        args.scene = Some(DEFAULT_SCENE.to_string());
        apply_default_scene_values(matches, args, &config_touched)?;
    }

    let mut curated_modified = HashSet::new();
    if !scene_is_cli && !scene_is_default {
        curated_modified.extend(apply_scene_values(matches, args, &config_touched)?);
    }
    if !scene_custom_is_cli {
        if let Some(scene_custom_name) = args.scene_custom.clone() {
            curated_modified.extend(apply_scene_custom_layer(
                matches,
                args,
                &cfg,
                &scene_custom_name,
                false,
            )?);
        }
    }
    if scene_is_cli {
        curated_modified.extend(apply_scene_values(matches, args, &config_touched)?);
    }
    if scene_custom_is_cli {
        if let Some(scene_custom_name) = args.scene_custom.clone() {
            curated_modified.extend(apply_scene_custom_layer(
                matches,
                args,
                &cfg,
                &scene_custom_name,
                true,
            )?);
        }
    }

    apply_glitch_level_values(matches, args, &config_touched, &curated_modified);

    // Phase 5 closure (P3-5): emit a startup warning summary if any soft
    // warnings were emitted during config apply. This makes warnings visible
    // even in noisy startup output (e.g. when stderr scrolls past quickly).
    let warning_count = crate::output::startup_warning_count();
    if warning_count > 0 {
        use std::io::Write;
        let _ = std::io::stderr().write_fmt(format_args!(
            "[config] {warning_count} warning(s) emitted during config apply — scroll up for details, or run 'cosmostrix --testconf' for strict validation.\n"
        ));
    }

    Ok(())
}

fn apply_default_scene_values(
    matches: &clap::ArgMatches,
    args: &mut Args,
    config_touched: &HashSet<&'static str>,
) -> Result<(), String> {
    let Some(scene) = get_scene(DEFAULT_SCENE) else {
        return Ok(());
    };
    let cfg = scene.config;
    if let Some(color) = cfg.color {
        if !is_explicit(matches, "color") && !config_touched.contains("color") {
            args.color = color.to_string();
        }
    }
    if let Some(charset) = cfg.charset {
        if !is_explicit(matches, "charset") && !config_touched.contains("charset") {
            args.charset = charset.to_string();
        }
    }
    if let Some(fps) = cfg.fps {
        if !is_explicit(matches, "fps") && !config_touched.contains("fps") {
            args.fps = fps;
        }
    }
    if let Some(speed) = cfg.speed {
        if !is_explicit(matches, "speed") && !config_touched.contains("speed") {
            args.speed = speed;
        }
    }
    if let Some(density) = cfg.density {
        if !is_explicit(matches, "density") && !config_touched.contains("density") {
            args.density = density;
        }
    }
    if let Some(glitch_level) = cfg.glitch_level {
        if !is_explicit(matches, "glitch_level") && !config_touched.contains("glitch_level") {
            args.glitch_level = glitch_level;
        }
    }
    Ok(())
}

/// Apply top-level `config.toml` values to `args`.
///
/// **Design note (Phase 4 P4-4 — positive finding, intentional pattern):**
/// This function calls `config_value(matches, cfg, snake_key, kebab_key)`
/// once per supported config key (17 sequential lookups). An alternative
/// single-iteration design (`for (key, value) in cfg { match key { ... } }`)
/// would reduce 34 HashMap lookups to 1 iteration + 17 match arms, saving
/// ~3μs per startup. The current design is kept because:
/// 1. Startup runs once — ~5μs total is invisible.
/// 2. Co-locating each key's handling with its lookup is more readable.
/// 3. The 17-lookup pattern makes it trivial to add/remove a key (one
///    block per key, no shared match arm to keep in sync).
fn apply_config_values(
    matches: &clap::ArgMatches,
    args: &mut Args,
    cfg: &HashMap<String, String>,
    config_touched: &mut HashSet<&'static str>,
) {
    if let Some(v) = config_value(matches, cfg, "scene", "scene") {
        match validate_scene_name(&v) {
            Ok(name) => {
                args.scene = Some(name);
                config_touched.insert("scene");
            }
            Err(e) => {
                // Strip the "error: " prefix from validate_scene_name's message
                // since eprintln_error_labeled adds its own "error:" label.
                let msg = e.strip_prefix("error: ").unwrap_or(&e);
                crate::output::eprintln_error_labeled(msg);
            }
        }
    }

    if let Some(v) = config_value(matches, cfg, "color", "color") {
        if parse_color_scheme(&v).is_ok() {
            args.color = v;
            config_touched.insert("color");
        } else {
            crate::output::eprintln_error_labeled(&format!(
                "invalid color='{v}' (see --list-colors)"
            ));
        }
    }
    if let Some(v) = config_value(matches, cfg, "charset", "charset") {
        // v25: charset may be a built-in preset OR a [charset-custom.<name>]
        // block. Check both — `validate_config_strictly` already accepted
        // the value, so we should not silently reject a custom name here.
        if charset_from_str(&v, false).is_ok()
            || crate::charset_custom::load_custom_charset_if_matches(cfg, &v).is_some()
        {
            args.charset = v;
            config_touched.insert("charset");
        } else {
            crate::output::eprintln_error_labeled(&format!(
                "invalid charset='{v}' (see --list-charsets)"
            ));
        }
    }
    if let Some(v) = config_value(matches, cfg, "fps", "fps") {
        if let Some(f) = parse_f64_config("fps", &v, 1.0, 240.0) {
            args.fps = f;
            config_touched.insert("fps");
        }
    }
    if let Some(v) = config_value(matches, cfg, "speed", "speed") {
        if let Some(f) = parse_speed_config("speed", &v) {
            args.speed = f;
            config_touched.insert("speed");
        }
    }
    if let Some(v) = config_value(matches, cfg, "density", "density") {
        if let Some(f) = parse_f32_config("density", &v, 0.01, DENSITY_CLAMP_MAX) {
            args.density = f;
            config_touched.insert("density");
        }
    }
    if let Some(v) = config_value(matches, cfg, "monolith_size", "monolith-size") {
        match MonolithSize::from_str(&v, true) {
            Ok(size) => {
                args.monolith_size = size;
                config_touched.insert("monolith_size");
            }
            Err(_) => {
                crate::output::eprintln_error_labeled(&format!(
                    "invalid monolith-size='{v}' (allowed: small, normal, large)"
                ));
            }
        }
    }
    if let Some(v) = config_value(matches, cfg, "glitch_level", "glitch-level") {
        match GlitchLevel::from_str(&v, true) {
            Ok(level) => {
                args.glitch_level = level;
                config_touched.insert("glitch_level");
            }
            Err(_) => crate::output::eprintln_error_labeled(
                "invalid glitch-level='{v}' (allowed: none, subtle, default, intense)",
            ),
        }
    }
    if let Some(v) = config_value(matches, cfg, "intro", "intro") {
        // Parse the intro type using clap's ValueEnum machinery so the
        // accepted values stay in sync with the --intro CLI flag.
        // Precedence: CLI --intro flag wins over this config key (handled
        // by `config_value` returning None when the flag is explicit).
        match IntroType::from_str(&v, true) {
            Ok(t) => {
                args.intro = Some(t);
                config_touched.insert("intro");
            }
            Err(_) => crate::output::eprintln_error_labeled(
                "invalid intro='{v}' (allowed: cosmic, logo, none)",
            ),
        }
    }
    if let Some(v) = config_value(matches, cfg, "bold", "bold") {
        if let Some(n) = parse_u8_config("bold", &v, 0, 2) {
            args.bold = n;
            config_touched.insert("bold");
        }
    }
    if let Some(v) = config_value(matches, cfg, "shading_mode", "shadingmode") {
        if let Some(n) = parse_u8_config("shadingmode", &v, 0, 1) {
            args.shading_mode = n;
            config_touched.insert("shading_mode");
        }
    }
    if let Some(v) = config_value(matches, cfg, "color_bg", "color-bg") {
        if let Some(bg) = parse_color_bg_config(&v) {
            args.color_bg = bg;
            config_touched.insert("color_bg");
        }
    }
    if let Some(v) = config_value(matches, cfg, "auto_color_drift", "auto-color-drift") {
        if let Some(b) = parse_bool_config("auto-color-drift", &v) {
            args.auto_color_drift = b;
            config_touched.insert("auto_color_drift");
        }
    }
    // v17: --async flag removed (always on). Config key 'async-mode' still
    // respected for users who want to disable it via config. No is_explicit
    // check needed since the CLI flag is gone.
    if let Some(v) = cfg.get("async-mode") {
        if let Some(b) = parse_bool_config("async-mode", v) {
            args.async_mode = b;
            config_touched.insert("async_mode");
        }
    }
}

fn apply_scene_values(
    matches: &clap::ArgMatches,
    args: &mut Args,
    config_touched: &HashSet<&'static str>,
) -> Result<HashSet<&'static str>, String> {
    let mut scene_modified = HashSet::new();
    let Some(ref scene_name) = args.scene else {
        return Ok(scene_modified);
    };

    let name = validate_scene_name(scene_name)?;
    args.scene = Some(name.clone());

    if let Some(scene) = get_scene(&name) {
        let cfg = scene.config;
        // Scene defaults only apply to keys NOT explicitly set by the user
        // in config.toml. This mirrors the apply_default_scene_values
        // pattern: config-set keys win over scene defaults. CLI flags
        // still win over both (checked via is_explicit).
        if let Some(color) = cfg.color {
            if !is_explicit(matches, "color") && !config_touched.contains("color") {
                args.color = color.to_string();
                scene_modified.insert("color");
            }
        }
        if let Some(charset) = cfg.charset {
            if !is_explicit(matches, "charset") && !config_touched.contains("charset") {
                args.charset = charset.to_string();
                scene_modified.insert("charset");
            }
        }
        if let Some(fps) = cfg.fps {
            if !is_explicit(matches, "fps") && !config_touched.contains("fps") {
                args.fps = fps;
                scene_modified.insert("fps");
            }
        }
        if let Some(speed) = cfg.speed {
            if !is_explicit(matches, "speed") && !config_touched.contains("speed") {
                args.speed = speed;
                scene_modified.insert("speed");
            }
        }
        if let Some(density) = cfg.density {
            if !is_explicit(matches, "density") && !config_touched.contains("density") {
                args.density = density;
                scene_modified.insert("density");
            }
        }
        if let Some(glitch_level) = cfg.glitch_level {
            if !is_explicit(matches, "glitch_level") && !config_touched.contains("glitch_level") {
                args.glitch_level = glitch_level;
                scene_modified.insert("glitch_level");
            }
        }
    }

    Ok(scene_modified)
}

fn apply_glitch_level_values(
    matches: &clap::ArgMatches,
    args: &mut Args,
    config_touched: &HashSet<&'static str>,
    curated_modified: &HashSet<&'static str>,
) {
    let high_precedence_glitch_level =
        is_explicit(matches, "glitch_level") || curated_modified.contains("glitch_level");

    let should_skip = |arg_id: &'static str| {
        is_explicit(matches, arg_id)
            || (config_touched.contains(arg_id) && !high_precedence_glitch_level)
    };

    // v30 simplify: --noglitch CLI flag removed. glitch_enabled is now derived
    // directly from glitch_level (None => false, anything else => true) at
    // CloudConfig construction time. The `should_skip("noglitch")` calls are
    // gone because there's no `args.noglitch` to assign anymore.

    // Phase 5 closure (P2-3): RECLASSIFIED as false positive. The deprecated
    // glitch flags (--glitchpct / --shortpct / --rippct / --maxdpc) were
    // removed in v17 — they are `#[arg(skip = ...)]` in config.rs and NOT in
    // USER_CONFIG_KEYS, so users cannot set them via CLI or config.toml. The
    // glitch_pct/shortpct/rippct fields are internal-only, set exclusively by
    // glitch_level presets below. No silent override is possible — the
    // original Phase 2 P2-3 finding described a v16-era scenario that no
    // longer applies.
    // v35.3 (CLI-V-4): corrected flag names (--glitchpct not --glitch-pct) and
    // added --maxdpc to the list.

    match args.glitch_level {
        GlitchLevel::None => {
            // v35.3 (Glitch-BUG8): explicitly reset all 5 preset fields to the
            // None preset (matching apply_glitch_level_runtime) so the stored
            // CloudConfig is honest — startup-None now matches
            // runtime-scene-switch-None. The clap defaults happen to match for
            // shortpct/rippct (50.0/33.33333) but glitch_pct clap default is
            // 10.0 while None preset is 0.0; setting it explicitly removes the
            // disagreement. glitchy=false means these are unused for rendering,
            // but shortpct/rippct ARE read by build_droplet_spec regardless.
            args.glitch_pct = 0.0;
            args.glitch_ms = crate::config::U16Range {
                low: 300,
                high: 400,
            };
            args.shortpct = 50.0;
            args.rippct = 33.33333;
        }
        GlitchLevel::Subtle => {
            if !should_skip("glitch_ms") {
                args.glitch_ms = crate::config::U16Range {
                    low: 200,
                    high: 300,
                };
            }
            args.glitch_pct = 3.0;
            args.shortpct = 60.0;
            args.rippct = 45.0;
        }
        GlitchLevel::Default => {
            if !should_skip("glitch_ms") {
                args.glitch_ms = crate::config::U16Range {
                    low: 300,
                    high: 400,
                };
            }
            args.glitch_pct = 10.0;
            args.shortpct = 50.0;
            args.rippct = 33.33333;
        }
        GlitchLevel::Intense => {
            if !should_skip("glitch_ms") {
                args.glitch_ms = crate::config::U16Range {
                    low: 500,
                    high: 800,
                };
            }
            args.glitch_pct = 25.0;
            args.shortpct = 30.0;
            args.rippct = 20.0;
        }
    }
}

fn config_value(
    matches: &clap::ArgMatches,
    cfg: &HashMap<String, String>,
    arg_id: &str,
    config_key: &str,
) -> Option<String> {
    if is_explicit(matches, arg_id) {
        None
    } else {
        cfg.get(config_key).cloned()
    }
}

#[inline]
fn is_explicit(matches: &clap::ArgMatches, key: &str) -> bool {
    !matches!(
        matches.value_source(key),
        None | Some(ValueSource::DefaultValue)
    )
}

fn parse_f32_config(name: &str, value: &str, min: f32, max: f32) -> Option<f32> {
    match parse_canonical_f32_range(&format!("config {name}"), value, min, max) {
        Ok(f) => Some(f),
        Err(_) => {
            crate::output::eprintln_error_labeled(&format!(
                "invalid {name}='{value}' (expected: number in range {min}..={max})"
            ));
            None
        }
    }
}

fn parse_f64_config(name: &str, value: &str, min: f64, max: f64) -> Option<f64> {
    match parse_canonical_f64_range(&format!("config {name}"), value, min, max) {
        Ok(f) => Some(f),
        Err(_) => {
            crate::output::eprintln_error_labeled(&format!(
                "invalid {name}='{value}' (expected: number in range {min}..={max})"
            ));
            None
        }
    }
}

fn parse_u8_config(name: &str, value: &str, min: u8, max: u8) -> Option<u8> {
    match parse_canonical_u8_range(&format!("config {name}"), value, min, max) {
        Ok(valid) => Some(valid),
        Err(_) => {
            crate::output::eprintln_error_labeled(&format!(
                "invalid {name}='{value}' (expected: number in range {min}..={max})"
            ));
            None
        }
    }
}

fn parse_speed_config(name: &str, value: &str) -> Option<f32> {
    match parse_canonical_speed(&format!("config {name}"), value) {
        Ok(valid) => Some(valid),
        Err(_) => {
            crate::output::eprintln_error_labeled(&format!(
                "invalid {name}='{value}' (expected: canonical integer in range {SPEED_MIN}..={SPEED_MAX})"
            ));
            None
        }
    }
}

/// Parse a bool config value, accepting the lenient set:
/// `true/yes/on/1` → true, `false/no/off/0` → false (case-insensitive, trims).
///
/// Phase D Bug #1 fix: this is the SINGLE canonical bool parser for config
/// values. Previously 3 sites had 3 different parsers:
/// - testconf.rs:543 — strict, only "true"/"false" (case-sensitive)
/// - config_apply.rs:652 — lenient (this fn)
/// - live_config.rs:815 — strictest, only `v.trim() == "true"`
///
/// Now all 3 sites use this function (testconf mirrors the accepted set,
/// live_config calls this directly). A config that passes `--testconf`
/// will behave identically at startup and live-reload.
pub(crate) fn parse_bool_config(name: &str, value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => {
            crate::output::eprintln_error_labeled(&format!(
                "invalid {name}='{value}' (expected true/false)"
            ));
            None
        }
    }
}

fn parse_color_bg_config(value: &str) -> Option<ColorBg> {
    match value.trim().to_ascii_lowercase().as_str() {
        "black" => Some(ColorBg::Black),
        "default-background" | "default_background" => Some(ColorBg::DefaultBackground),
        _ => {
            crate::output::eprintln_error_labeled(&format!(
                "invalid color-bg='{value}' (allowed: black, default-background)"
            ));
            None
        }
    }
}
