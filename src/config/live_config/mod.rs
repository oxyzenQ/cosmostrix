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

/// Rebuild a CloudConfig from base + new config values. CLI-only fields
/// preserved from base. Per-field CLI flags (`cli_explicit`) immutable.
#[must_use]
pub(crate) fn rebuild_cloud_config(
    base: &crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
) -> crate::app::CloudConfig {
    let mut new = base.clone();
    // Snapshot CLI-explicit tracker — preserved across reloads.
    // CliExplicit derives Copy, so this is a cheap field copy, not a heap clone.
    let cli = new.cli_explicit;

    lr_trace!(
        "rebuild_cloud_config: cli_explicit = {{color:{}, charset:{}, speed:{}, density:{}, fps:{}, scene:{}, glitch:{}}}",
        cli.color, cli.charset, cli.speed, cli.density, cli.fps, cli.scene, cli.glitch_level
    );

    // depth-test fix: user-set color/charset must win over scene defaults.
    let user_set_color = cfg.contains_key("color");
    let user_set_charset = cfg.contains_key("charset");

    // Color scheme — skip if CLI --color was explicit.
    // v51 (owner audit 2026-08-30): custom-palette parity with the startup
    // path (main.rs checks `colors-custom` FIRST — v50.0.0-beta.6 Option D).
    // The old live-reload block only parsed BUILTIN scheme names, so:
    //   - switching `color` TO a custom palette name was a silent no-op;
    //   - switching `color` AWAY from an active custom palette left the
    //     stale palette loaded, and create_cloud's `set_palette` kept
    //     overriding the builtin scheme the user just switched to.
    // Now: custom name → load the palette (custom wins on collision,
    // mirroring startup); builtin name → clear the palette so the scheme
    // actually takes effect; absent key → keep current state.
    // Z-master-2-v2: `--colors-custom <name>` is also an explicit CLI color
    // intent — a config `color` key must NOT clear the CLI-owned palette on
    // reload (startup checks args.colors_custom FIRST and never drops it).
    if !cli.color && !cli.colors_custom {
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
    } else {
        lr_trace!("skip color (CLI explicit) — keeping {:?}", new.color_scheme);
    }

    // v16: Custom color palette live reload (if active at startup).
    // Z-master-2-v2 note: this fires even when cli.colors_custom is set —
    // that is CORRECT: it re-loads the CLI-owned palette from the (edited)
    // [colors-custom.<name>] block, which is the documented live-edit
    // feature for custom palettes. The cli.colors_custom guard above only
    // prevents the plain `color` key from switching/clearing the palette.
    if let Some(ref name) = new.custom_palette_name {
        if let Ok(palette) = crate::colors_custom::load_custom_palette(cfg, name) {
            new.custom_palette = Some(palette);
        }
    }

    // Charset — skip if CLI --charset
    if !cli.charset {
        if let Some(v) = cfg.get("charset") {
            // v25: charset-custom.<name> takes precedence over built-in.
            if let Some(custom_chars) =
                crate::charset_custom::load_custom_charset_if_matches(cfg, v)
            {
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
    } else {
        lr_trace!(
            "skip charset (CLI explicit) — keeping '{}'",
            new.charset_preset
        );
    }

    // Scene — skip if CLI --scene explicit. scene color/charset are
    // defaults; user config values win.
    // v51 (owner audit 2026-08-30): custom-scene parity. The old block
    // only resolved BUILTIN scenes (get_scene), so switching `scene` to
    // a custom scene name updated scene_name but left rain_style/color/
    // charset/speed/density at the PREVIOUS scene's values — a visual
    // no-op whenever the ambient-schedule scene-change branch did not
    // fire. Custom scenes are now resolved here: rain_style comes from
    // the base-scene (mirroring startup's rain_style_for_custom_scene
    // construction path), and the field layer is applied by the
    // scene-custom tail block below (same layer the startup path uses).
    // Z-master-2-v2: `--scene-custom <name>` is also explicit CLI scene
    // intent — a config `scene` key must NOT replace the CLI-selected
    // custom scene on reload (startup applies the CLI scene-custom layer
    // LAST, so it wins over the config scene key; the guard preserves
    // that outcome). The tail block below still re-applies the custom
    // scene's fields, so live-editing [scene-custom.<name>] keeps working.
    if !cli.scene && !cli.scene_custom {
        if let Some(v) = cfg.get("scene") {
            // v50 fix: update new.scene_name to match the config's scene
            // value. Without this, the live-reload path left scene_name at
            // base.scene_name (the previous value), so the HUD 'scn:' line
            // showed the old scene even though the rain style had already
            // switched. The event_loop.rs schedule-empty preserve/else
            // branch compares new_cfg.scene_name against preserved_scene_name
            // to decide whether to re-apply scene runtime — both values MUST
            // reflect the config's scene for that branch to fire correctly.
            // This is the source-of-truth fix; the event_loop.rs else branch
            // (commit 51ccafe) is the consumer-side mirror.
            //
            // Normalization: scene names are case-insensitive at lookup
            // (scene::get_scene lowercases internally), but the HUD displays
            // the exact string the user typed. We preserve the original
            // casing from config for display, matching the startup path in
            // main.rs (args.scene.as_deref().unwrap_or(DEFAULT_SCENE)).
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
                            new.chars = crate::charset::build_chars(
                                charset,
                                &new.user_ranges,
                                new.def_ascii,
                            );
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
                // v51 (owner audit 2026-08-30): startup-parity — the startup
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
                // block applies base-scene defaults + field overrides,
                // and resolve rain_style here (construction-level field
                // the tail block does not touch).
                lr_trace!(
                    "apply scene='{}' (custom scene: resolving rain_style + field layer)",
                    v
                );
                new.scene_custom_name = Some(v.clone());
                new.rain_style =
                    crate::scene_custom::rain_style_for_custom_scene(cfg, &normalized_scene)
                        .unwrap_or(crate::rain_style::RainStyle::Glyph);
            } else {
                // Unknown scene — upstream strict validation rejects the
                // config before it reaches the render thread, so this is
                // defense-in-depth: keep the previous values.
                lr_trace!("scene='{}' unknown — keeping previous scene values", v);
            }
        }
    }

    // Speed — skip if CLI --speed was explicit
    if !cli.speed {
        if let Some(v) = cfg.get("speed") {
            if let Ok(n) = crate::validation::parse_canonical_speed("speed", v) {
                lr_trace!("apply speed='{}' -> {}", v, n);
                new.speed = n;
            } else {
                lr_trace!("speed='{}' failed to parse — keeping {}", v, new.speed);
            }
        }
    } else {
        lr_trace!("skip speed (CLI explicit) — keeping {}", new.speed);
    }

    // Density — skip if CLI --density was explicit
    if !cli.density {
        if let Some(v) = cfg.get("density") {
            if let Ok(n) = crate::validation::parse_canonical_f32_range("density", v, 0.01, 5.0) {
                lr_trace!("apply density='{}' -> {}", v, n);
                new.density = n;
                new.base_density = n;
            } else {
                lr_trace!("density='{}' failed to parse — keeping {}", v, new.density);
            }
        }
    } else {
        lr_trace!("skip density (CLI explicit) — keeping {}", new.density);
    }

    // FPS — skip if CLI --fps was explicit
    if !cli.fps {
        if let Some(v) = cfg.get("fps") {
            if let Ok(n) = crate::validation::parse_canonical_f64_range("fps", v, 1.0, 240.0) {
                lr_trace!("apply fps='{}' -> {}", v, n);
                new.target_fps = n;
            } else {
                lr_trace!("fps='{}' failed to parse — keeping {}", v, new.target_fps);
            }
        }
    } else {
        lr_trace!("skip fps (CLI explicit) — keeping {}", new.target_fps);
    }

    // Glitch level — skip if CLI --glitch-level was explicit.
    // (CLI-P-3): re-derive ALL preset values on live reload.
    // (Glitch-BUG3): None arm now resets all 5 preset fields too.
    // BL-01 (Dragon Hunt v3): dedup — delegate to the shared helper in
    // scene_custom.rs (bit-identical preset values, was inlined here).
    // max_dpc is NOT touched — never set by glitch_level presets at startup.
    if !cli.glitch_level {
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
    } else {
        lr_trace!(
            "skip glitch-level (CLI explicit) — glitch_enabled={}",
            new.glitch_enabled
        );
    }

    // color-bg live reload (true = terminal default; false = solid black).
    // Z-master-2-v2: `--color-bg` CLI wins over config on live-reload (was:
    // config-only path, no intent guard — the CLI flag was overridden by a
    // config edit on next reload; startup gates via config_value).
    if !cli.color_bg {
        if let Some(v) = cfg.get("color-bg") {
            new.default_bg = match v.trim().to_ascii_lowercase().as_str() {
                "black" => false,
                "default-background" | "default_background" => true,
                _ => new.default_bg,
            };
            lr_trace!("apply color-bg='{}' → default_bg={}", v, new.default_bg);
        }
    } else {
        lr_trace!(
            "skip color-bg (CLI explicit) — keeping default_bg={}",
            new.default_bg
        );
    }

    // Monolith size — v50.0.0-alpha.7: CLI intent guard added (Issue #4).
    // CLI --monolith-size wins over config on live-reload (was: config-only
    // path, no guard → CLI flag overridden by config edit on next reload).
    if !cli.monolith_size {
        if let Some(v) = cfg.get("monolith-size") {
            use clap::ValueEnum;
            if let Ok(size) = crate::runtime::MonolithSize::from_str(v, true) {
                new.monolith_size = size;
            }
        }
    }

    // Crystal Dragon Engine — intent preservation: CLI --crystal-dragon
    // wins over config.toml on live reload.
    if !cli.crystal_dragon {
        if let Some(v) = cfg.get("crystal-dragon") {
            if let Some(b) = crate::config_apply::parse_bool_config("crystal-dragon", v) {
                new.crystal_dragon = b;
            }
        }
    }

    // v50.0.0-alpha.7: Power Dragon live reload. CLI --power-dragon wins
    // over config (was: config-only path, no intent guard — CLI flag was
    // overridden by config edit on next reload). Now mirrors crystal-dragon.
    if !cli.power_dragon {
        if let Some(v) = cfg.get("power-dragon") {
            if let Some(b) = crate::config_apply::parse_bool_config("power-dragon", v) {
                new.power_dragon = b;
            }
        }
    }

    // (CLI-P-1): live-reload bold/shading-mode/async-mode (previously
    // silently ignored). Mirrors startup parsers with range validation.
    // Z-master-2-v2: `--bold` CLI wins over config on live-reload (was:
    // config-only path, no intent guard — the CLI flag was overridden by a
    // config edit on next reload; startup gates via config_value).
    if !cli.bold {
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
    } else {
        lr_trace!("skip bold (CLI explicit) — keeping {:?}", new.bold_mode);
    }
    // Z-master-2-v2: `--shading-mode` CLI wins over config on live-reload
    // (was: config-only path, no intent guard — same bug class as bold).
    if !cli.shading_mode {
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
    } else {
        lr_trace!(
            "skip shading-mode (CLI explicit) — keeping {:?}",
            new.shading_mode
        );
    }
    // v50.0.0-alpha.7: --async-mode CLI flag now exists. CLI wins over
    // config (was: config-only path, no intent guard — CLI flag was
    // overridden by config edit on next reload). Now mirrors crystal-dragon.
    if !cli.async_mode {
        if let Some(v) = cfg.get("async-mode") {
            if let Some(b) = crate::config_apply::parse_bool_config("async-mode", v) {
                new.async_mode = b;
            }
        }
    }

    // v20: scene-custom live reload — re-apply fields if the custom scene
    // is STILL the active scene. v51 (owner audit 2026-08-30): the tracker
    // is now `new.scene_custom_name` (updated by the scene block above when
    // the user switches scenes) instead of the immutable startup
    // `base.scene_custom_name` — otherwise the stale startup layer kept
    // overriding every builtin scene the user switched to at runtime.
    if let Some(custom_name) = new.scene_custom_name.clone() {
        let still_active = new
            .scene_name
            .trim()
            .eq_ignore_ascii_case(custom_name.trim());
        if still_active {
            crate::scene_custom::apply_scene_custom_to_cloud_config(&mut new, cfg, &custom_name);
        }
    }

    // (bug #9): color.tune.* live reload — re-parse from cfg HashMap
    // (same path as startup).
    // v50.0.0-alpha.7 fix: was gated on `has_tune_keys` — when all
    // color.tune.* keys were commented out, the parser didn't see them,
    // so `has_tune_keys` was false, and the base tune (e.g. brightness=0.0)
    // was preserved instead of resetting to identity (1.0). Now: always
    // re-parse (color_tune_from_config returns IDENTITY when no keys
    // present — that's the correct "reset to default" behavior).
    // CLI --color-tune is preserved via `cli.color_tune` guard: when CLI
    // is explicit, config absence does NOT reset (CLI wins).
    if !cli.color_tune {
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
    } else {
        lr_trace!("color.tune: CLI --color-tune explicit — preserving base tune");
    }

    // v50.0.0-alpha.7: Live-reload for message / message-border / msg-mode.
    // Previously these 3 keys were NOT handled in rebuild_cloud_config —
    // editing config.toml mid-run had no effect until restart. This was
    // the primary source of owner/user confusion (see
    // docs/LIVE_RELOAD_BEHAVIOR.md Issue #2).
    //
    // Precedence (highest wins):
    //   1. CLI -m / -mb (always wins — cli.message guard skips config read)
    //   2. msg-mode=false → suppress config message (gate fires)
    //   3. config `message-border` (wins over `message` when both present)
    //   4. config `message` (no border)
    //   5. default fallback (only at startup, not here — main.rs handles)
    //
    // The msg-mode gate mirrors config_apply.rs: when msg-mode=false AND
    // message came from config (not CLI), clear it. CLI -m/-mb is unaffected.
    let msg_mode_on = if !cli.msg_mode {
        // CLI --msg-mode not explicit → read from config (default true).
        cfg.get("msg-mode")
            .and_then(|v| crate::config_apply::parse_bool_config("msg-mode", v))
            .unwrap_or(true)
    } else {
        // CLI --msg-mode explicit → keep the startup value.
        new.msg_mode
    };
    new.msg_mode = msg_mode_on;

    if !cli.message {
        // CLI -m / -mb not explicit → read message from config.
        let msg_from_config: Option<(String, bool)> = cfg
            .get("message-border")
            .map(|v| (v.clone(), true))
            .or_else(|| cfg.get("message").map(|v| (v.clone(), false)));
        if let Some((text, border)) = msg_from_config {
            // Apply msg-mode gate: if msg-mode=false, suppress config message.
            // CLI -m/-mb always wins (handled by cli.message guard above).
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
        } else {
            // No message in config. Mirror the startup default-fallback
            // logic (main.rs:1239-1258): when args.message is None AND
            // !bench_mode AND msg_mode_on, use default_message_text() with
            // border. Without this, base.message (which may carry a stale
            // config value like "hey") would be preserved instead of
            // reverting to the default "Experience a masterpiece with
            // cosmostrix v{}". Follows the same "reset-on-comment" pattern
            // as color.tune (LIVE_RELOAD_BEHAVIOR.md Limitation C, fixed
            // v50.0.0-alpha.7).
            //
            // Live-reload only fires in interactive mode (benchmarks exit
            // immediately, no watcher), so the !bench_mode guard from
            // main.rs is implicitly satisfied here.
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
    } else {
        lr_trace!(
            "skip message (CLI -m/-mb explicit) — keeping '{}'",
            new.message.as_deref().unwrap_or("(none)")
        );
    }

    // v50.0.0-alpha.7: Live-reload intro-color (was missing).
    // CLI --intro-color wins over config (cli.intro_color guard).
    // Validates theme name on reload — invalid themes are logged and
    // cleared (mirrors startup behavior, but soft-fail on live-reload
    // to avoid crashing a running session).
    if !cli.intro_color {
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
    }

    // Ambient: re-collect schedule. Event loop pushes to scheduler thread.
    new.ambient_schedule = crate::crystal_dragon_engine::ambient::collect_ambient_schedule(cfg);
    if !new.ambient_schedule.is_empty() {
        lr_trace!(
            "ambient: reloaded {} entries",
            new.ambient_schedule.entries.len()
        );
    }

    // v51 msg-fill-style: live-reload the message overlay reveal style.
    // CLI -mfs/--msg-fill-style wins over config (cli.msg_fill_style
    // guard). Invalid values are logged and skipped (soft-fail, mirrors
    // intro-color live-reload policy — don't crash a running session).
    // When the key is absent, the startup style is preserved (unlike
    // color.tune there is no "reset-on-comment" semantics for enums —
    // an absent key simply means "unchanged").
    if !cli.msg_fill_style {
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
    } else {
        lr_trace!(
            "skip msg-fill-style (CLI -mfs/--msg-fill-style explicit) — keeping '{}'",
            new.msg_fill_style.as_str()
        );
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
mod tests_cli_priority;

#[cfg(test)]
mod tests_msg_fill_style;

#[cfg(test)]
mod tests_rejection_msg;
