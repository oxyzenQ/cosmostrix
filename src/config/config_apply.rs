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
//!    keys). `[scene-custom.<name>]` config blocks are the active
//!    custom scene mechanism.
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
use crate::config::{Args, ColorBg, GlitchLevel};
use crate::constants::{DENSITY_CLAMP_MAX, SPEED_MAX, SPEED_MIN};
use crate::intro_style::IntroType;
use crate::msg_fill_style::MsgFillStyle;
use crate::runtime::MonolithSize;
use crate::scene::{get_scene, DEFAULT_SCENE};
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
    // of config apply. Individual warnings (from scene-custom
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
            // depth-test fix: targeted "did you mean" hints for
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

    // v50-beta.3: intro-color validation runs unconditionally (even when cfg
    // is empty) because the value may come from the CLI flag --intro-color.
    // apply_config_values is gated on `!cfg.is_empty()`, so we cannot rely
    // on it to fire when only the CLI flag is set. Validation lives here.
    //
    // CLI flag --intro-color <name> populates args.intro_color via clap; the
    // config key intro-color = "name" is read from cfg. Either way,
    // validation must fire to reject unknown theme names. Unlike other
    // config values that warn-and-continue, intro-color is a hard error:
    // the renderer cannot build a palette for a non-existent theme, so we
    // exit early with a clear message + "did you mean" suggestion.
    let intro_color_value: Option<String> = args
        .intro_color
        .clone()
        .or_else(|| cfg.get("intro-color").cloned());
    if let Some(v) = intro_color_value {
        let theme_ok = crate::theme::lookup_theme(&v).is_some();
        let custom_ok = cfg.contains_key(&format!("colors-custom.{v}.bg"));
        if theme_ok || custom_ok {
            args.intro_color = Some(v);
            config_touched.insert("intro-color");
        } else {
            // Hard error: unknown theme name. Surface a "tip: a similar
            // value exists" suggestion if a builtin theme is close
            // (edit distance ≤ 2).
            let suggestion = crate::theme::suggest_closest_theme(&v);
            let hint = match suggestion {
                Some(name) => format!(
                    "{}\n  Use --list-colors to see all available themes.",
                    crate::cli::suggestion::format_value_suggestion(name)
                ),
                None => String::from("\n  Use --list-colors to see all available themes."),
            };
            return Err(format!(
                "error: invalid intro-color='{v}' — not a builtin theme or custom palette.{hint}"
            ));
        }
    }

    if !cfg.is_empty() {
        apply_config_values(matches, args, &cfg, &mut config_touched);
    }

    // v50.0.0-beta.6 Option D: scene name collision resolution.
    // If args.scene matches a [scene-custom.<name>] block, auto-set
    // args.scene_custom so the custom layer gets applied (custom wins).
    // If the name also matches a builtin scene, emit a collision warning.
    // This lets `--scene <custom>` work (previously required --scene-custom).
    if let Some(ref scene_name) = args.scene {
        let normalized = scene_name.trim().to_ascii_lowercase();
        let custom_scenes = crate::scene_custom::collect_custom_scenes(&cfg);
        if custom_scenes.contains_key(&normalized) {
            // Custom block exists — auto-set scene_custom if not already set.
            if args.scene_custom.is_none() {
                args.scene_custom = Some(normalized.clone());
            }
            // Warn if builtin also exists (collision).
            if crate::scene::get_scene(&normalized).is_some() {
                crate::output::warn_name_collision(
                    "scene",
                    &normalized,
                    "builtin scene (see --list-scenes)",
                    "custom scene from [scene-custom.*]",
                );
            }
        }
    }

    let scene_is_cli = is_explicit(matches, "scene");
    let scene_custom_is_cli = is_explicit(matches, "scene_custom");
    let scene_is_default = args.scene.is_none();
    if scene_is_default {
        args.scene = Some(DEFAULT_SCENE.to_string());
        apply_default_scene_values(matches, args, &config_touched)?;
    }

    // v50.0.0-beta.6 Option D: validate that the scene name is either a
    // builtin scene or a [scene-custom.<name>] block. If neither, error
    // with a clear message. This catches typos like `--scene nonexistnt`
    // while still accepting custom scene names (which were previously
    // rejected by validate_scene_name).
    if let Some(scene_name) = args.scene.as_ref() {
        let normalized = scene_name.trim().to_ascii_lowercase();
        let is_builtin = crate::scene::get_scene(&normalized).is_some();
        let custom_scenes = crate::scene_custom::collect_custom_scenes(&cfg);
        let is_custom = custom_scenes.contains_key(&normalized);
        if !is_builtin && !is_custom {
            return Err(format!(
                "error: unknown scene '{scene_name}'{}\n\n  Use --list-scenes to see available scenes.",
                scene_suggestion_tip(&normalized, &cfg)
            ));
        }
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
        // v50.0.0-beta.6 Option D: accept custom scene names from config
        // (not just builtin). Previously validate_scene_name() rejected any
        // name not in the builtin list. Now we accept the name if it matches
        // EITHER a builtin scene OR a [scene-custom.<name>] block. The
        // collision resolution + warning happens later in
        // apply_config_and_runtime_defaults.
        let normalized = v.trim().to_ascii_lowercase();
        let is_builtin = crate::scene::get_scene(&normalized).is_some();
        let custom_scenes = crate::scene_custom::collect_custom_scenes(cfg);
        let is_custom = custom_scenes.contains_key(&normalized);
        if is_builtin || is_custom {
            args.scene = Some(normalized);
            config_touched.insert("scene");
        } else {
            crate::output::eprintln_error_labeled(&format!(
                "unknown scene '{v}'{}\n\n  Use --list-scenes to see available scenes.",
                scene_suggestion_tip(&normalized, cfg)
            ));
        }
    }

    if let Some(v) = config_value(matches, cfg, "color", "color") {
        // v50.0.0-beta.6 Option D: color may be a builtin theme OR a
        // [colors-custom.<name>] block. Check both — custom wins on
        // collision (handled by main.rs color resolution). The collision
        // warning is emitted in main.rs at the unified resolution point.
        if parse_color_scheme(&v).is_ok() || crate::colors_custom::is_colors_custom_name(cfg, &v) {
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
    // (intro-color validation moved to apply_config_and_runtime_defaults
    // — it runs unconditionally, even when cfg is empty, so CLI flag
    // --intro-color gets validated without needing a config file.)
    // v50: Overlay message config keys. Two keys mirror the CLI flags:
    //   message         = "text"  → message WITHOUT border (matches -m)
    //   message-border  = "text"  → message WITH border    (matches -mb)
    // CLI -m / -mb wins over either config key (handled by `config_value`'s
    // `is_explicit` check returning None when the CLI flag is present).
    // When both config keys are present, `message-border` wins (border=true).
    // Default fallback (no CLI, no config) is applied later in main.rs.
    if let Some(v) = config_value(matches, cfg, "message", "message-border") {
        // `message-border` config key — wins over `message` config key.
        args.message = Some(v);
        args.message_border = true;
        config_touched.insert("message-border");
    } else if let Some(v) = config_value(matches, cfg, "message", "message") {
        // `message` config key — message WITHOUT border.
        args.message = Some(v);
        // message_border stays at its clap default (false) unless -mb was
        // passed on the CLI (handled by `is_explicit` returning None for
        // the `message-border` config_value above, so we never reach here
        // when -mb is on the CLI).
        args.message_border = false;
        config_touched.insert("message");
    }
    // v50-beta.3: msg-mode gate runs at end of this function (after
    // msg-mode itself is parsed) — see `apply_msg_mode_gate(...)` call.
    if let Some(v) = config_value(matches, cfg, "bold", "bold") {
        if let Some(n) = parse_u8_config("bold", &v, 0, 2) {
            args.bold = n;
            config_touched.insert("bold");
        }
    }
    if let Some(v) = config_value(matches, cfg, "shading_mode", "shading-mode") {
        if let Some(n) = parse_u8_config("shading-mode", &v, 0, 1) {
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
    // Crystal Dragon Engine config.
    // v50-beta.3: CLI --crystal-dragon=true|false wins over config.
    // Previously config-only; now both paths set args.crystal_dragon: Option<bool>.
    if let Some(v) = config_value(matches, cfg, "crystal_dragon", "crystal-dragon") {
        if let Some(b) = parse_bool_config("crystal-dragon", &v) {
            args.crystal_dragon = Some(b);
            config_touched.insert("crystal_dragon");
        }
    }
    // v50-beta.3: power-dragon CLI flag now exists (--power-dragon=true|false).
    // CLI wins over config (handled by config_value's is_explicit check).
    // Default when neither CLI nor config provides a value: true (main.rs).
    if let Some(v) = config_value(matches, cfg, "power_dragon", "power-dragon") {
        if let Some(b) = parse_bool_config("power-dragon", &v) {
            args.power_dragon = Some(b);
            config_touched.insert("power-dragon");
        }
    }
    // v50-beta.3: msg-mode CLI flag now exists (--msg-mode=true|false).
    // CLI wins over config. Default when neither provides a value: true (main.rs).
    // msg-mode=false disables BOTH default message AND any message/message-border
    // config key; CLI -m/-mb always wins (handled in main.rs).
    if let Some(v) = config_value(matches, cfg, "msg_mode", "msg-mode") {
        if let Some(b) = parse_bool_config("msg-mode", &v) {
            args.msg_mode = Some(b);
            config_touched.insert("msg-mode");
        }
    }
    // v51 msg-fill-style: message overlay reveal style. Parsed with clap's
    // ValueEnum machinery so the accepted values stay in sync with the
    // -mfs/--msg-fill-style CLI flag. Case-insensitive (config surface is
    // forgiving, mirroring `intro` and `glitch-level`). CLI flag wins over
    // this config key (handled by `config_value`'s `is_explicit` check).
    if let Some(v) = config_value(matches, cfg, "msg_fill_style", "msg-fill-style") {
        match MsgFillStyle::from_str(&v, true) {
            Ok(style) => {
                args.msg_fill_style = style;
                config_touched.insert("msg-fill-style");
            }
            Err(_) => crate::output::eprintln_error_labeled(
                "invalid msg-fill-style='{v}' (allowed: typewriter, fade, words, slide, instant, engrave, hologram, glitch, scorch, cascade)",
            ),
        }
    }
    // v50-beta.3: --async-mode CLI flag now exists (replaces --uniform).
    // CLI flag wins over config key (handled by config_value's is_explicit).
    // Default: true (async variable pacing on) — applied in main.rs.
    if let Some(v) = config_value(matches, cfg, "async_mode", "async-mode") {
        if let Some(b) = parse_bool_config("async-mode", &v) {
            args.async_mode = Some(b);
            config_touched.insert("async_mode");
        }
    }

    // v50-beta.3: msg-mode gate (runs AFTER all message/msg-mode parsing).
    // Rule: if msg-mode=false AND message came from config (not CLI -m/-mb),
    // clear it. CLI -m/-mb always wins (we don't touch message when it was
    // set via CLI).
    // Detection: if config_touched has `message` or `message-border`, the
    // message came from config. If neither is in config_touched but args.message
    // is Some, it came from CLI (or pre-existing default) — leave it alone.
    let msg_from_config =
        config_touched.contains("message") || config_touched.contains("message-border");
    let msg_mode_on = args.msg_mode.unwrap_or(true); // default true
    if !msg_mode_on && msg_from_config {
        // msg-mode=false + config message: clear it. User must set msg-mode=true
        // to use config message/message-border. CLI -m/-mb is unaffected.
        args.message = None;
        args.message_border = false;
        config_touched.remove("message");
        config_touched.remove("message-border");
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

/// v51 did-you-mean audit: suggestion tip for an unknown scene name.
///
/// Candidates = every builtin scene name + every `[scene-custom.<name>]`
/// block defined in the config. Uses the shared edit-distance <= 2 policy
/// (same as colors / charsets / enum values), so `--scene cinemtic`
/// suggests 'cinematic' instead of the bare "use --list-scenes" dead end.
fn scene_suggestion_tip(normalized: &str, cfg: &HashMap<String, String>) -> String {
    let mut candidates: Vec<&str> = crate::scene::SCENES.iter().map(|s| s.name).collect();
    let custom: Vec<String> = crate::scene_custom::collect_custom_scenes(cfg)
        .keys()
        .cloned()
        .collect();
    let custom_refs: Vec<&str> = custom.iter().map(|s| s.as_str()).collect();
    candidates.extend(custom_refs);
    crate::cli::suggestion::closest_value_match(normalized, &candidates)
        .map(|s| crate::cli::suggestion::format_value_suggestion(&s))
        .unwrap_or_default()
}

pub(super) fn is_explicit(matches: &clap::ArgMatches, key: &str) -> bool {
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

pub(crate) fn parse_f64_config(name: &str, value: &str, min: f64, max: f64) -> Option<f64> {
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

// v50.0.0-beta.7 LOC refactor: apply_scene_values + apply_glitch_level_values
// extracted to config_apply_scene_glitch.rs.
mod config_apply_scene_glitch;
pub(crate) use config_apply_scene_glitch::{apply_glitch_level_values, apply_scene_values};

/// v51 did-you-mean audit: scene name typo suggestions.
#[cfg(test)]
mod scene_suggestion_tests {
    use super::*;

    #[test]
    fn scene_suggestion_tip_suggests_builtin() {
        let cfg = HashMap::new();
        assert_eq!(
            scene_suggestion_tip("cinemtic", &cfg),
            "\n  tip: a similar value exists: 'cinematic'"
        );
    }

    #[test]
    fn scene_suggestion_tip_includes_custom_scenes() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "scene-custom.afternoon.base-scene".to_string(),
            "cinematic".to_string(),
        );
        assert_eq!(
            scene_suggestion_tip("afternon", &cfg),
            "\n  tip: a similar value exists: 'afternoon'"
        );
    }

    #[test]
    fn scene_suggestion_tip_distant_name_is_empty() {
        let cfg = HashMap::new();
        assert_eq!(scene_suggestion_tip("zzzzzzzz", &cfg), "");
    }
}
