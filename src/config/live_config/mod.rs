// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Live config reload — "The Cosmic Dragon's true Awakening".
//!
//! Watches config.toml for changes, validates strictly, and sends the
//! validated config HashMap to the render thread for a full Cloud rebuild.
//!
//! ## Architecture
//!
//! ```text
//! config.toml → notify watcher thread → mpsc channel → render thread
//!               (parse + validate)      (try_recv/frame)  (rebuild Cloud)
//! ```
//!
//! - Watcher thread: blocks on filesystem events, reparses config on change.
//!   Strict validation — any invalid value rejects the entire config.
//! - Render thread: `try_recv()` each frame; rebuilds Cloud on update.
//!
//! ## Strict validation
//!
//! Uses the same `validate_field_value` rules as `--testconf`. Invalid
//! values reject the entire config with a clear error message.
//!
//! ## S3 (internal independent QA): parse race with non-atomic editor writes
//!
//! If the editor writes the config file non-atomically (truncate + write,
//! e.g. `echo > config.toml` or `tee`), the watcher may read a half-written
//! file mid-save. The strict parser will see malformed lines and reject the
//! entire config, setting LIVE_RELOAD_EXIT_CODE=2 and exiting cosmostrix.
//!
//! Editors that write atomically (temp file + rename: vim, emacs, nano,
//! VSCode, most modern editors) are safe — the watcher sees either the
//! old file or the complete new file, never a partial write.
//!
//! This is a known limitation of file-watcher systems. The  design
//! (exit on validation error) makes this more visible than the old "silently
//! keep last valid config" behavior, but it is the honest choice: a
//! malformed config should not be silently ignored.

use std::collections::HashMap;

use clap::ValueEnum;

use crate::types::constants::MESSAGE_MAX_LEN;

// Polling heartbeat + snapshot dedup live in live_config_poll.rs.

// AB-10: session-wide buffered state lives in live_config_state.rs.
// Re-export everything so existing `live_config::LIVE_RELOAD_*` and
// `live_config::push_*` references continue to resolve unchanged.
#[cfg(test)]
pub(crate) use crate::live_config_state::drain_validation_rejections;
#[cfg(test)]
pub(crate) use crate::live_config_state::MAX_REJECTION_LOG;
pub(crate) use crate::live_config_state::{
    drain_runtime_warnings, interactive_session_active, push_runtime_warning,
    push_validation_rejection, set_interactive_session_active, LIVE_RELOAD_ERROR,
    LIVE_RELOAD_EXIT_CODE,
};

/// Live config event sent from watcher to render thread.
/// Ok = valid config, rebuild Cloud. Err = invalid, exit cosmostrix.
pub(crate) type LiveConfigEvent = Result<HashMap<String, String>, String>;

/// Rebuild a CloudConfig from the locked startup base + new config values.
///
/// v80.0.0-beta.2 masterclass precedence (owner contract, S-master-LOGIC-3):
///
/// ```text
/// Startup:  CLI > config.toml > scene defaults > built-in defaults
/// Runtime:  user shortkeys > ambient scene > config keys (incl. the
///           [scene-custom.<name>] block fields) > CLI lock > scene defaults
/// ```
///
/// The runtime chain is the temporal-intent chain: whatever the user
/// touched LAST wins, and the ambient overlay outranks plain config
/// keys for the scene-family fields while a phase is active. The CLI
/// never blocks a present config value at runtime — it only survives
/// as the locked fallback underneath (see event_loop_scene_sync's
/// RestoreLocked arm).
///
/// The runtime layering is TEMPORAL: an explicit config key is the most
/// recent user intent, so it wins over the CLI flag — but the CLI value
/// stays LOCKED underneath (held in `base`, the pristine startup snapshot
/// the caller keeps for the whole session). When the key is commented out
/// or removed, the rebuild falls back to the locked startup value: a
/// `--scene crystal-dragon` run returns to crystal-dragon after the
/// config `scene` override is commented back out — no exit, no rerun.
///
/// Key-present arms below apply the config value. Key-absent arms do
/// nothing (base already carries the locked value). Scene-managed default
/// arms (the scene block's inner gates and the scene-custom / base-scene
/// layers) still gate on `cli_explicit.*` — a CLI lock also beats
/// scene-level defaults, mirroring the startup chain.
#[must_use]
pub(crate) fn rebuild_cloud_config(
    base: &crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
) -> crate::app::CloudConfig {
    let mut new = base.clone();
    // Snapshot CLI-explicit tracker — the LOCKED layer (alive for the whole
    // session; the v50.0.0-beta.6 zeroing in the caller was removed in v80.0.0-beta.1
    // because it destroyed every CLI lock on the first reload).
    // CliExplicit derives Copy, so this is a cheap field copy, not a heap clone.
    let cli = new.cli_explicit;

    // depth-test fix: user-set color/charset must win over scene defaults.
    let user_set_color = cfg.contains_key("color");
    let user_set_charset = cfg.contains_key("charset");

    // Color scheme — config key present wins over the CLI lock (v80.0.0-beta.1
    // temporal precedence); when the key is absent, base keeps the locked
    // startup value (CLI color first, then config@startup), so no fallback
    // arm is needed here.
    // v80.0.0-beta.1 (owner audit 2026-08-30): custom-palette parity with the startup
    // path (main.rs checks `colors-custom` FIRST — v50.0.0-beta.6 Option D).
    // The old live-reload block only parsed BUILTIN scheme names, so:
    //   - switching `color` TO a custom palette name was a silent no-op;
    //   - switching `color` AWAY from an active custom palette left the
    //     stale palette loaded, and create_cloud's `set_palette` kept
    //     overriding the builtin scheme the user just switched to.
    // Now: custom name → load the palette (custom wins on collision,
    // mirroring startup); builtin name → clear the palette so the scheme
    // actually takes effect; absent key → keep current state.
    // v80.0.0-beta.1: the `!cli.color && !cli.colors_custom` guard is GONE — it
    // encoded the pre-beta.6 "CLI blocks config" model. A runtime config
    // `color` key now overrides a CLI color (and may clear a CLI-owned
    // palette — the owner's contract: the key is the most recent intent);
    // commenting the key back out falls back to the locked startup color
    // AND the locked palette (base carries both).
    if let Some(v) = cfg.get("color") {
        let is_custom = crate::colors_custom::is_colors_custom_name(cfg, v);
        if is_custom {
            match crate::colors_custom::load_custom_palette(cfg, v) {
                Ok(palette) => {
                    lr_trace!("apply color='{}' -> custom palette", v);
                    new.custom_palette = Some(palette);
                    new.custom_palette_name = Some(v.clone());
                }
                Err(e) => {
                    lr_trace!(
                        "color='{}' custom palette failed to load ({}) — keeping current",
                        v,
                        e
                    );
                }
            }
        } else if let Ok(scheme) = crate::cli::parse_color_scheme(v) {
            lr_trace!("apply color='{}' -> {:?}", v, scheme);
            new.color_scheme = scheme;
            // Explicit switch to a builtin: drop any active custom
            // palette — create_cloud applies custom_palette AFTER
            // the scheme, so a lingering palette would silently
            // override the scheme the user just selected.
            if new.custom_palette.is_some() {
                lr_trace!(
                    "clearing custom palette '{}' (color switched to builtin '{}')",
                    new.custom_palette_name.as_deref().unwrap_or("?"),
                    v
                );
                new.custom_palette = None;
                new.custom_palette_name = None;
            }
        } else {
            lr_trace!(
                "color='{}' failed to parse — keeping {:?}",
                v,
                new.color_scheme
            );
        }
    }

    // v16: Custom color palette live reload (if active at startup).
    // Note: this fires even when the palette is CLI-owned (`--colors-custom`)
    // — that is CORRECT: it re-loads the palette from the (edited)
    // [colors-custom.<name>] block, which is the documented live-edit
    // feature for custom palettes.
    if let Some(ref name) = new.custom_palette_name {
        if let Ok(palette) = crate::colors_custom::load_custom_palette(cfg, name) {
            new.custom_palette = Some(palette);
        }
    }

    // Charset — config key present wins over the CLI lock (v80.0.0-beta.1); when
    // the key is absent, base keeps the locked startup charset.
    if let Some(v) = cfg.get("charset") {
        // v25: charset-custom.<name> takes precedence over built-in.
        if let Some(custom_chars) = crate::charset_custom::load_custom_charset_if_matches(cfg, v) {
            lr_trace!(
                "apply charset='{}' (custom, {} chars)",
                v,
                custom_chars.len()
            );
            new.charset_preset = v.clone();
            new.chars = custom_chars;
        } else if let Ok(charset) = crate::charset::charset_from_str(v, false) {
            lr_trace!("apply charset='{}' (built-in)", v);
            new.charset_preset = v.clone();
            new.chars = crate::charset::build_chars(charset, &new.user_ranges, new.def_ascii);
        } else {
            lr_trace!(
                "charset='{}' failed to parse — keeping '{}'",
                v,
                new.charset_preset
            );
        }
    }

    // Scene — config key present wins over the CLI-locked scene (v80.0.0-beta.1
    // temporal precedence: the key is the most recent user intent). When
    // the key is ABSENT, the fallback is decided by the caller
    // (event_loop_config_rebuild.rs); no fallback arm here.
    // v80.0.0-beta.1 (owner audit 2026-08-30): custom-scene parity — the old
    // block only resolved BUILTIN scenes, so switching `scene` to a custom
    // name updated scene_name but left the visual fields at the PREVIOUS
    // scene's values. Custom scenes now resolve here (rain_style always
    // Glyph; the complete field layer applies via the scene-custom tail
    // block below, same layer startup uses).
    // v80.0.0-beta.1: the old `!cli.scene && !cli.scene_custom` outer guard is
    // GONE — the CLI lock survives as the FALLBACK layer (commenting out the
    // config `scene` key returns the `--scene-custom`/`--scene` selection).
    if let Some(v) = cfg.get("scene") {
        // v50 fix: update new.scene_name to match the config's scene value —
        // the HUD 'scn:' line and the event_loop schedule-empty branch both
        // compare against this field. Preserve the user's casing for
        // display (lookup is case-insensitive; matches startup).
        lr_trace!("apply scene='{}' (updating scene_name)", v);
        new.scene_name = v.clone();
        let normalized_scene = v.trim().to_ascii_lowercase();
        let custom_scenes = crate::scene_custom::collect_custom_scenes(cfg);
        if let Some(scene_info) = crate::scene::get_scene(v) {
            // Builtin scene: the startup custom-scene layer no longer
            // applies — clear the tracker so the stale layer is not
            // re-applied by the tail block below (owner audit: switching
            // scene away from a custom scene used to leave the custom
            // layer overriding the builtin the user switched to).
            new.scene_custom_name = None;
            // v80.0.0-beta.2 (S-master-HUNT): a builtin selection is never
            // custom-scene-owned — keep the flag coherent with the None
            // tracker so a later restore/sync starts from clean state.
            new.scene_custom_config_owned = false;
            new.rain_style = scene_info.config.rain_style;
            if let Some(color) = scene_info.config.color {
                // Z-master-2-v2: `--colors-custom` is explicit CLI color
                // intent — the scene color default (and the palette clear
                // below) must not touch a CLI-owned palette.
                if !cli.color && !cli.colors_custom && !user_set_color {
                    if let Ok(scheme) = crate::cli::parse_color_scheme(color) {
                        lr_trace!("scene '{}' applies default color={:?}", v, scheme);
                        new.color_scheme = scheme;
                        // (Z1-4): clear any stale custom palette when the
                        // scene switch actually applies the builtin color
                        // default. create_cloud applies custom_palette
                        // AFTER the scheme, so a palette left over from a
                        // palette-owning custom scene (colors-custom field)
                        // would silently shadow the scheme the scene
                        // switch just set — making the switch a visual
                        // no-op for color. Startup parity: startup
                        // resolution re-evaluates the palette from scratch
                        // (main.rs), so no stale palette can survive there.
                        if new.custom_palette.is_some() {
                            lr_trace!(
                                "clearing custom palette '{}' (scene switched to builtin '{}')",
                                new.custom_palette_name.as_deref().unwrap_or("?"),
                                v
                            );
                            new.custom_palette = None;
                            new.custom_palette_name = None;
                        }
                    }
                } else {
                    lr_trace!("scene '{}' color skipped — user/CLI set", v);
                }
            }
            if let Some(charset_name) = scene_info.config.charset {
                if !cli.charset && !user_set_charset {
                    if let Some(custom_chars) =
                        crate::charset_custom::load_custom_charset_if_matches(cfg, charset_name)
                    {
                        lr_trace!(
                            "scene '{}' applies default charset='{}' (custom)",
                            v,
                            charset_name
                        );
                        new.charset_preset = charset_name.to_string();
                        new.chars = custom_chars;
                    } else if let Ok(charset) =
                        crate::charset::charset_from_str(charset_name, false)
                    {
                        lr_trace!(
                            "scene '{}' applies default charset='{}' (built-in)",
                            v,
                            charset_name
                        );
                        new.charset_preset = charset_name.to_string();
                        new.chars =
                            crate::charset::build_chars(charset, &new.user_ranges, new.def_ascii);
                    }
                } else {
                    lr_trace!("scene '{}' charset skipped — user/CLI set", v);
                }
            }
            if let Some(speed) = scene_info.config.speed {
                if !cli.speed {
                    new.speed = speed;
                }
            }
            if let Some(density) = scene_info.config.density {
                if !cli.density {
                    new.density = density;
                    new.base_density = density;
                }
            }
            // v80.0.0-beta.1 (owner audit 2026-08-30): startup-parity — the startup
            // path (apply_default_scene_values) also applies the scene's
            // fps and glitch_level defaults; the live-reload block never
            // did, so switching scenes via config.toml silently kept the
            // previous scene's fps cap and glitch preset. Applied here
            // BEFORE the user-key blocks below, so an explicit `fps` or
            // `glitch-level` key in config still wins (same layering as
            // startup: CLI > config > scene defaults).
            if let Some(fps) = scene_info.config.fps {
                if !cli.fps {
                    lr_trace!("scene '{}' applies default fps={}", v, fps);
                    new.target_fps = fps;
                }
            }
            if let Some(glitch) = scene_info.config.glitch_level {
                if !cli.glitch_level {
                    lr_trace!("scene '{}' applies default glitch_level={:?}", v, glitch);
                    crate::scene_custom::apply_glitch_level_preset_to_cloud_config(
                        &mut new, glitch,
                    );
                }
            }
        } else if custom_scenes.contains_key(&normalized_scene) {
            // Custom scene: mark it active so the scene-custom tail
            // block applies the (complete) field layer, and resolve
            // rain_style here (construction-level field the tail
            // block does not touch). v80.0.0-beta.2: base-scene
            // inheritance is removed — custom scenes always render
            // glyph rain.
            lr_trace!(
                "apply scene='{}' (custom scene: resolving rain_style + field layer)",
                v
            );
            new.scene_custom_name = Some(v.clone());
            // v80.0.0-beta.2 (S-master-HUNT): the config `scene` key names
            // the custom scene — the block layer is CONFIG-OWNED at runtime
            // (the tail block may re-apply its fields over CLI locks).
            new.scene_custom_config_owned = true;
            new.rain_style = crate::rain_style::RainStyle::Glyph;
        } else {
            // Unknown scene — upstream strict validation rejects the
            // config before it reaches the render thread, so this is
            // defense-in-depth: keep the previous values.
            lr_trace!("scene='{}' unknown — keeping previous scene values", v);
        }
    }

    // Speed — config key present wins over the CLI lock (v80.0.0-beta.1 temporal
    // precedence); absent key falls back to the locked startup value in
    // base (CLI speed first), so no fallback arm is needed.
    if let Some(v) = cfg.get("speed") {
        if let Ok(n) = crate::validation::parse_canonical_speed("speed", v) {
            lr_trace!("apply speed='{}' -> {}", v, n);
            new.speed = n;
        } else {
            lr_trace!("speed='{}' failed to parse — keeping {}", v, new.speed);
        }
    }

    // Density — config key present wins (v80.0.0-beta.1); absent key falls back to
    // the locked startup value in base.
    if let Some(v) = cfg.get("density") {
        if let Ok(n) = crate::validation::parse_canonical_f32_range("density", v, 0.01, 5.0) {
            lr_trace!("apply density='{}' -> {}", v, n);
            new.density = n;
            new.base_density = n;
        } else {
            lr_trace!("density='{}' failed to parse — keeping {}", v, new.density);
        }
    }

    // FPS — config key present wins (v80.0.0-beta.1); absent key falls back to the
    // locked startup value in base.
    if let Some(v) = cfg.get("fps") {
        if let Ok(n) = crate::validation::parse_canonical_f64_range("fps", v, 1.0, 240.0) {
            lr_trace!("apply fps='{}' -> {}", v, n);
            new.target_fps = n;
        } else {
            lr_trace!("fps='{}' failed to parse — keeping {}", v, new.target_fps);
        }
    }

    // Glitch level — config key present wins (v80.0.0-beta.1); absent key falls
    // back to the locked startup value in base.
    // (CLI-P-3): re-derive ALL preset values on live reload.
    // (Glitch-BUG3): None arm now resets all 5 preset fields too.
    // BL-01 (Dragon Hunt v3): dedup — delegate to the shared helper in
    // scene_custom.rs (bit-identical preset values, was inlined here).
    // max_dpc is NOT touched — never set by glitch_level presets at startup.
    if let Some(v) = cfg.get("glitch-level") {
        lr_trace!("apply glitch-level='{}'", v);
        use clap::ValueEnum;
        match crate::config::GlitchLevel::from_str(v, true) {
            Ok(level) => {
                crate::scene_custom::apply_glitch_level_preset_to_cloud_config(&mut new, level);
            }
            // Unrecognized: flip enable bool only (old fallback).
            // Startup clap rejects bad values, so this shouldn't fire.
            Err(_) => {
                new.glitch_enabled = !v.trim().eq_ignore_ascii_case("none");
            }
        }
    }

    // color-bg live reload (true = terminal default; false = solid black).
    // v80.0.0-beta.1: config key present wins over the CLI lock; absent key falls
    // back to the locked startup value in base.
    if let Some(v) = cfg.get("color-bg") {
        new.default_bg = match v.trim().to_ascii_lowercase().as_str() {
            "black" => false,
            "default-background" | "default_background" => true,
            _ => new.default_bg,
        };
        lr_trace!("apply color-bg='{}' → default_bg={}", v, new.default_bg);
    }

    // Monolith size — v50.0.0-alpha.7 tracked CLI intent (Issue #4);
    // v80.0.0-beta.1: config key present wins over the CLI lock, absent key falls
    // back to the locked startup value in base.
    if let Some(v) = cfg.get("monolith-size") {
        use clap::ValueEnum;
        if let Ok(size) = crate::runtime::MonolithSize::from_str(v, true) {
            new.monolith_size = size;
        }
    }

    // Crystal Dragon Engine — v80.0.0-beta.1: config key present wins over the CLI
    // lock; absent key falls back to the locked startup value in base.
    if let Some(v) = cfg.get("crystal-dragon") {
        if let Some(b) = crate::config_apply::parse_bool_config("crystal-dragon", v) {
            new.crystal_dragon = b;
        }
    }

    // v80.0.0-alpha.1: crystal-dragon-secs live-reload — the online harmony
    // knob. Config key present (valid) wins over the CLI lock; absent key
    // keeps the locked startup value in base (mirrors crystal-dragon).
    // Out-of-range values are rejected upstream by validate_config_strictly
    // before rebuild runs, so the parse_f64_config fallback (None → keep
    // base) is defense-in-depth only. The new value reaches the Cloud via
    // create_cloud (CloudConfig -> crystal_dragon_control.polling_secs);
    // inherit_ecosystem_state no longer copies the old cloud's control.
    if let Some(v) = cfg.get("crystal-dragon-secs") {
        if let Some(secs) =
            crate::config_apply::parse_f64_config("crystal-dragon-secs", v, 0.0, 86400.0)
        {
            new.crystal_dragon_secs = Some(secs);
            crate::lr_trace!("crystal-dragon-secs: {}", secs);
        }
    }

    // v50.0.0-alpha.7: Power Dragon live reload; v80.0.0-beta.1: config key present
    // wins over the CLI lock, absent key falls back to the locked startup
    // value in base. Mirrors crystal-dragon.
    if let Some(v) = cfg.get("power-dragon") {
        if let Some(b) = crate::config_apply::parse_bool_config("power-dragon", v) {
            new.power_dragon = b;
        }
    }

    // (CLI-P-1): live-reload bold/shading-mode/async-mode (previously
    // silently ignored). Mirrors startup parsers with range validation.
    // v80.0.0-beta.1: config key present wins over the CLI lock; absent key falls
    // back to the locked startup value in base.
    if let Some(v) = cfg.get("bold").and_then(|s| s.trim().parse::<u8>().ok()) {
        // Range-gate to match startup parse_u8_config("bold", ..., 0, 2).
        // Upstream validate_config_strictly catches out-of-range before this
        // runs, but defense-in-depth prevents silent mis-parsing if that
        // validation ever has a regression.
        new.bold_mode = match v {
            0 => crate::runtime::BoldMode::Off,
            2 => crate::runtime::BoldMode::All,
            _ => crate::runtime::BoldMode::Random,
        };
        if v > 2 {
            // Out-of-range: log and let validate_config_strictly handle
            // rejection on next cycle. Do not apply the parsed value.
            new.bold_mode = base.bold_mode;
        }
    }
    // v80.0.0-beta.1: config key present wins over the CLI lock; absent key falls
    // back to the locked startup value in base.
    if let Some(v) = cfg
        .get("shading-mode")
        .and_then(|s| s.trim().parse::<u8>().ok())
    {
        new.shading_mode = match v {
            1 => crate::runtime::ShadingMode::DistanceFromHead,
            _ => crate::runtime::ShadingMode::Random,
        };
        if v > 1 {
            new.shading_mode = base.shading_mode;
        }
    }
    // v50.0.0-alpha.7: --async-mode CLI flag; v80.0.0-beta.1: config key present
    // wins over the CLI lock, absent key falls back to the locked startup
    // value in base. Mirrors crystal-dragon.
    if let Some(v) = cfg.get("async-mode") {
        if let Some(b) = crate::config_apply::parse_bool_config("async-mode", v) {
            new.async_mode = b;
        }
    }

    // v20: scene-custom live reload — re-apply fields if the custom scene
    // is STILL the active scene. v80.0.0-beta.1 (owner audit 2026-08-30): the tracker
    // is now `new.scene_custom_name` (updated by the scene block above when
    // the user switches scenes) instead of the immutable startup
    // `base.scene_custom_name` — otherwise the stale startup layer kept
    // overriding every builtin scene the user switched to at runtime.
    //
    // v80.0.0-beta.2 (S-master-HUNT) ownership gate: the tail block fires
    // only when the layer is CONFIG-OWNED (`scene_custom_config_owned`) —
    // selected at runtime by the config `scene` key (the scene block above
    // sets it) or by the ambient scheduler (the runtime-scene sync in
    // event_loop_scene_sync.rs). When the tracker reflects the LOCKED
    // startup resolution (startup construction or
    // `restore_locked_scene_family` — e.g. the config `scene` key was just
    // commented out), the startup snapshot ALREADY resolved the block
    // layer correctly (explicit CLI flags shadow block fields) and the tail
    // block must NOT re-derive the fields over the lock. Re-applying there
    // stomped CLI-shadowed values and kept a REMOVED config scene's profile
    // alive after the overlay lifted (owner bug: `--scene tron_legacy
    // -c test -C test` + comment out `scene`/`ambient.*` -> charset/color
    // stuck on the block values instead of returning to the CLI setup).
    if let Some(custom_name) = new.scene_custom_name.clone() {
        let still_active = new
            .scene_name
            .trim()
            .eq_ignore_ascii_case(custom_name.trim());
        if new.scene_custom_config_owned && still_active {
            crate::scene_custom::apply_scene_custom_to_cloud_config(
                &mut new,
                cfg,
                &custom_name,
                true,
            );
        } else if still_active {
            // Lock-owned tracker: the block layer still re-applies, but
            // per-field CLI locks are respected (block EDITS to
            // non-shadowed dimensions keep working — the v20 feature).
            crate::scene_custom::apply_scene_custom_to_cloud_config(
                &mut new,
                cfg,
                &custom_name,
                false,
            );
        }
    }

    // (bug #9): color.tune.* live reload — re-parse from cfg HashMap
    // (same path as startup) whenever any tune key is present.
    // v50.0.0-alpha.7 fix: when all color.tune.* keys are commented out the
    // parser returns IDENTITY — the correct "reset to default" behavior
    // for a run with no CLI lock.
    // v80.0.0-beta.1 (owner contract): when CLI --color-tune is explicit, an absent
    // [color.tune] block falls back to the LOCKED startup tune instead of
    // resetting to identity — the CLI lock survives the key's removal.
    if cfg.keys().any(|k| k.starts_with("color.tune.")) {
        let new_tune = crate::color_tune::color_tune_from_config(cfg);
        if new_tune.brightness != new.color_tune.brightness
            || new_tune.saturation != new.color_tune.saturation
            || new_tune.head != new.color_tune.head
            || new_tune.body != new.color_tune.body
            || new_tune.tail != new.color_tune.tail
        {
            lr_trace!(
                "apply color.tune live reload: sat={} bright={} head={} body={} tail={} (was sat={} bright={} head={} body={} tail={})",
                new_tune.saturation, new_tune.brightness, new_tune.head, new_tune.body, new_tune.tail,
                new.color_tune.saturation, new.color_tune.brightness, new.color_tune.head, new.color_tune.body, new.color_tune.tail
            );
            new.color_tune = new_tune;
        } else {
            lr_trace!("color.tune: present but unchanged");
        }
    } else if cli.color_tune {
        lr_trace!("color.tune: no keys, CLI --color-tune locked — keeping base tune");
    } else {
        lr_trace!("color.tune: no keys, no CLI lock — resetting to identity");
        new.color_tune = crate::color_tune::ColorTune::IDENTITY;
    }

    // v50.0.0-alpha.7: Live-reload for message / message-border / msg-mode.
    // Previously these 3 keys were NOT handled in rebuild_cloud_config —
    // editing config.toml mid-run had no effect until restart. This was
    // the primary source of owner/user confusion (see
    // docs/LIVE_RELOAD_BEHAVIOR.md Issue #2).
    //
    // Precedence (highest wins):
    //   1. config `msg-mode` key (v80.0.0-beta.1: present key wins over the CLI lock)
    //   2. CLI --msg-mode (locked — survives key absence)
    //   3. default true (reset-on-comment, no CLI lock)
    //
    // The msg-mode gate mirrors config_apply.rs: when msg-mode=false AND
    // message came from config (not CLI), clear it. CLI -m/-mb messages
    // are unaffected by the gate (applied below).
    let msg_mode_on = if let Some(v) = cfg.get("msg-mode") {
        crate::config_apply::parse_bool_config("msg-mode", v).unwrap_or(true)
    } else if cli.msg_mode {
        // CLI --msg-mode locked — key absent, keep the startup value.
        new.msg_mode
    } else {
        // No key, no CLI lock — default true.
        true
    };
    new.msg_mode = msg_mode_on;

    // v80.0.0-beta.1: config `message`/`message-border` key present → wins over the
    // CLI -m/-mb lock (temporal precedence — the key is the most recent
    // intent). Absent → the CLI lock keeps the locked startup message
    // (base.message; the CLI -m text returns when the key is commented
    // out), else the default fallback (v50.0.0-alpha.7 reset-on-comment).
    let msg_from_config: Option<(String, bool)> = cfg
        .get("message-border")
        .map(|v| (v.clone(), true))
        .or_else(|| cfg.get("message").map(|v| (v.clone(), false)));
    if let Some((text, border)) = msg_from_config {
        // Apply msg-mode gate: if msg-mode=false, suppress config message.
        if msg_mode_on {
            // S3 (security harden): mirror the startup path's sanitization
            // + length cap. Without this, a live-reloaded `message` /
            // `message-border` value would (a) bypass sanitize_message_text
            // and allow ANSI/control-char injection into the terminal, and
            // (b) bypass the MESSAGE_MAX_LEN=200 cap and allow unbounded
            // memory allocation from a multi-MB config line. The startup
            // path (cli/build_cloud_cfg.rs:158-170) already enforces both;
            // this closes the live-reload inconsistency.
            if text.len() > MESSAGE_MAX_LEN {
                lr_trace!(
                    "reject live-reload message: {} chars exceeds {} limit",
                    text.len(),
                    MESSAGE_MAX_LEN
                );
                push_validation_rejection(&format!(
                    "message: value length {} exceeds {} character limit (live-reload rejected)",
                    text.len(),
                    MESSAGE_MAX_LEN
                ));
            } else {
                let sanitized = crate::output::message::sanitize_message_text(&text);
                new.message = Some(sanitized);
                new.message_border = border;
                lr_trace!(
                    "apply message='{}' border={} (from config, msg-mode=true, sanitized)",
                    new.message.as_deref().unwrap_or(""),
                    border
                );
            }
        } else {
            // msg-mode=false + config message → suppress.
            new.message = None;
            new.message_border = false;
            lr_trace!(
                "suppress config message (msg-mode=false) — text was '{}'",
                text
            );
        }
    } else if cli.message {
        // v80.0.0-beta.1: CLI -m/-mb locked — the config key's absence reveals
        // the locked startup message (base.message). No reset-to-default
        // here: the owner contract keeps the CLI value as the fallback.
        lr_trace!(
            "keep message (CLI -m/-mb locked, no config key) — '{}'",
            new.message.as_deref().unwrap_or("(none)")
        );
    } else {
        // No message in config, no CLI lock. Mirror the startup
        // default-fallback: msg_mode_on → default_message_text() with
        // border; else clear. Reset-on-comment semantics (Limitation C).
        // Live-reload only fires in interactive mode, so the !bench_mode
        // guard from main.rs is implicitly satisfied.
        if !msg_mode_on {
            new.message = None;
            new.message_border = false;
            lr_trace!("clear message (no config + msg-mode=false)");
        } else {
            new.message = Some(crate::constants::default_message_text());
            new.message_border = true;
            lr_trace!("revert message to default fallback (no config key, msg-mode=true)");
        }
    }

    // v50.0.0-alpha.7: Live-reload intro-color (was missing).
    // v80.0.0-beta.1: config key present wins over the CLI lock; absent key keeps
    // the locked startup intro color in base. Validates theme name on
    // reload — invalid themes are logged and cleared (soft-fail to avoid
    // crashing a running session).
    if let Some(v) = cfg.get("intro-color") {
        let theme_ok = crate::theme::lookup_theme(v).is_some();
        let custom_ok = cfg.contains_key(&format!("colors-custom.{v}.bg"));
        if theme_ok || custom_ok {
            new.intro_color = Some(v.clone());
            lr_trace!("apply intro-color='{}'", v);
        } else {
            // Soft-fail on live-reload: log + clear. Don't crash the
            // running session with a hard error (unlike startup which
            // exits). User can fix the config and save again.
            lr_trace!(
                "intro-color='{}' is invalid on live-reload — clearing (theme_ok={}, custom_ok={})",
                v,
                theme_ok,
                custom_ok
            );
            new.intro_color = None;
        }
    }

    // Ambient: re-collect schedule. Event loop pushes to scheduler thread.
    new.ambient_schedule = crate::crystal_dragon_engine::ambient::collect_ambient_schedule(cfg);
    if !new.ambient_schedule.is_empty() {
        lr_trace!(
            "ambient: reloaded {} entries",
            new.ambient_schedule.entries.len()
        );
    }

    // v80.0.0-beta.1 msg-fill-style: live-reload the message overlay reveal style.
    // v80.0.0-beta.1: config key present wins over the CLI lock; absent key keeps
    // the locked startup style in base (there is no "reset-on-comment"
    // semantics for enums — an absent key simply means "unchanged").
    // Invalid values are logged and skipped (soft-fail, mirrors
    // intro-color live-reload policy — don't crash a running session).
    if let Some(v) = cfg.get("msg-fill-style") {
        match crate::msg_fill_style::MsgFillStyle::from_str(v, true) {
            Ok(style) => {
                if style != new.msg_fill_style {
                    lr_trace!("apply msg-fill-style='{}'", style.as_str());
                }
                new.msg_fill_style = style;
            }
            Err(_) => {
                lr_trace!(
                    "msg-fill-style='{}' is invalid on live-reload — keeping '{}'",
                    v,
                    new.msg_fill_style.as_str()
                );
            }
        }
    }

    // v50.0.0-beta.7: ambient-snapback-secs live-reload (config-only).
    // When the key is absent (commented out), fall back to None (default
    // 30s via AUTO_SNAPBACK_DELAY_SECS). Mirrors the color.tune
    // reset-on-comment pattern (LIVE_RELOAD_BEHAVIOR.md Limitation C).
    new.ambient_snapback_secs = cfg.get("ambient-snapback-secs").and_then(|v| {
        crate::config_apply::parse_f64_config("ambient-snapback-secs", v, 0.0, 86400.0)
    });
    if let Some(secs) = new.ambient_snapback_secs {
        lr_trace!("ambient-snapback-secs: {}", secs);
    }

    new
}

// v50.0.0-beta.7 LOC refactor: watcher thread functions extracted to
// watcher.rs. Re-exported here so all call sites (including tests via
// 'use super::*' glob) resolve unchanged.
mod watcher;
pub(crate) use watcher::spawn_watcher;
#[cfg(test)]
pub(crate) use watcher::validate_and_send;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_cli_fallback;
#[cfg(test)]
mod tests_cli_priority;

#[cfg(test)]
mod tests_msg_fill_style;

#[cfg(test)]
mod tests_rejection_msg;

#[cfg(test)]
mod tests_crystal_dragon_secs;
