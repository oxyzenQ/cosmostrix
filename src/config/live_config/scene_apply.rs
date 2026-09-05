// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The `scene` config-key block of `rebuild_cloud_config`.
//!
//! v80.0.0-alpha.1 S-master-HUNT-6 extraction — keeps
//! live_config/mod.rs under the 800-LOC hard cap (see
//! src/RULES_LOC.md migration path). No semantic change: the body is
//! the verbatim block, moved one indent level out.
//!
//! Contract (the caller decides the key-ABSENT fallback):
//! a PRESENT `scene` key is a runtime trigger — the selected scene's
//! managed defaults (the complete bundle: color/charset/speed/density/
//! fps/glitch) outrank the CLI locks; the CLI lock survives as the
//! FALLBACK layer when the key is commented back out (restored by the
//! caller via `restore_locked_scene_family`).

use std::collections::HashMap;

use crate::app::CloudConfig;

/// Apply the `scene` key: resolve the named scene (builtin / custom /
/// unknown) onto `new`.
///
/// `user_set_color` / `user_set_charset`: whether the config's own
/// `color` / `charset` keys are present (applied EARLIER in
/// `rebuild_cloud_config` — they outrank the scene defaults).
pub(super) fn apply_scene_key(
    new: &mut CloudConfig,
    cfg: &HashMap<String, String>,
    user_set_color: bool,
    user_set_charset: bool,
) {
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
        crate::lr_trace!("apply scene='{}' (updating scene_name)", v);
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
                // S-master-HUNT-6 (owner mandate 2026-09-03): at RUNTIME a
                // present `scene` key outranks the CLI color locks (-c /
                // --colors-custom) — the config edit is the most recent
                // user intent, and a scene switch is a complete visual
                // statement. The ambient path
                // (`apply_builtin_scene_runtime`) already applies the full
                // bundle with no CLI gates; the rebuild path must match it
                // or the owner sees a half-applied scene (numeric fields
                // from the new scene, color/charset from the old CLI
                // flags). Only the config's OWN `color` key (applied
                // earlier in this function) still shields the scene
                // default. The CLI-locked palette is restored verbatim by
                // `restore_locked_scene_family` when the `scene` key is
                // commented back out — the lock is the FALLBACK layer, not
                // a runtime shield.
                if !user_set_color {
                    if let Ok(scheme) = crate::cli::parse_color_scheme(color) {
                        crate::lr_trace!(
                            "scene '{}' applies default color={:?} (over the CLI lock)",
                            v,
                            scheme
                        );
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
                        // The CLI-locked palette (startup --colors-custom)
                        // is restored verbatim by
                        // `restore_locked_scene_family` when the `scene` key
                        // is commented back out — the lock is the FALLBACK
                        // layer, not a runtime shield.
                        if new.custom_palette.is_some() {
                            crate::lr_trace!(
                                "clearing custom palette '{}' (scene switched to builtin '{}')",
                                new.custom_palette_name.as_deref().unwrap_or("?"),
                                v
                            );
                            new.custom_palette = None;
                            new.custom_palette_name = None;
                        }
                    }
                } else {
                    crate::lr_trace!("scene '{}' color skipped — config color key present", v);
                }
            }
            if let Some(charset_name) = scene_info.config.charset {
                // S-master-HUNT-6: same runtime precedence as color above —
                // the scene's charset beats the CLI `-C` lock at runtime;
                // only the config's own `charset` key (applied earlier)
                // shields it. Startup parity holds: at startup the CLI lock
                // wins (CLI > config > scene defaults), and commenting the
                // `scene` key back out restores the CLI charset via
                // `restore_locked_scene_family`.
                if !user_set_charset {
                    if let Some(custom_chars) =
                        crate::charset_custom::load_custom_charset_if_matches(cfg, charset_name)
                    {
                        crate::lr_trace!(
                            "scene '{}' applies default charset='{}' (custom)",
                            v,
                            charset_name
                        );
                        new.charset_preset = charset_name.to_string();
                        new.chars = custom_chars;
                    } else if let Ok(charset) =
                        crate::charset::charset_from_str(charset_name, false)
                    {
                        crate::lr_trace!(
                            "scene '{}' applies default charset='{}' (built-in)",
                            v,
                            charset_name
                        );
                        new.charset_preset = charset_name.to_string();
                        new.chars =
                            crate::charset::build_chars(charset, &new.user_ranges, new.def_ascii);
                    }
                } else {
                    crate::lr_trace!("scene '{}' charset skipped — config charset key present", v);
                }
            }
            // S-master-HUNT-6: speed/density/fps/glitch defaults beat the
            // CLI locks at runtime (same mandate as color/charset above).
            // The user-key blocks for these fields run AFTER this scene
            // block, so an explicit `speed`/`density`/`fps`/`glitch-level`
            // config key still wins over the scene default — no shielding
            // gate is needed here.
            if let Some(speed) = scene_info.config.speed {
                crate::lr_trace!(
                    "scene '{}' applies default speed={} (over the CLI lock)",
                    v,
                    speed
                );
                new.speed = speed;
            }
            if let Some(density) = scene_info.config.density {
                new.density = density;
                new.base_density = density;
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
                crate::lr_trace!(
                    "scene '{}' applies default fps={} (over the CLI lock)",
                    v,
                    fps
                );
                new.target_fps = fps;
            }
            if let Some(glitch) = scene_info.config.glitch_level {
                crate::lr_trace!("scene '{}' applies default glitch_level={:?}", v, glitch);
                crate::scene_custom::apply_glitch_level_preset_to_cloud_config(new, glitch);
            }
        } else if custom_scenes.contains_key(&normalized_scene) {
            // Custom scene: mark it active so the scene-custom tail
            // block applies the (complete) field layer, and resolve
            // rain_style here (construction-level field the tail
            // block applies via apply_scene_custom_field_to_cloud_config).
            // NIGHT-research-5 (owner-approved): custom scenes can now
            // pick any rain style via the block's `rain` field
            // (canonical label: glyph/monolith/vortex/ripple). The
            // resolve_rain_style helper consults the block; falls
            // back to Glyph when the field is missing or unrecognized.
            crate::lr_trace!(
                "apply scene='{}' (custom scene: resolving rain_style + field layer)",
                v
            );
            new.scene_custom_name = Some(v.clone());
            // v80.0.0-beta.2 (S-master-HUNT): the config `scene` key names
            // the custom scene — the block layer is CONFIG-OWNED at runtime
            // (the tail block may re-apply its fields over CLI locks).
            new.scene_custom_config_owned = true;
            new.rain_style = crate::scene_custom::resolve_rain_style(Some(v), cfg);
        } else {
            // Unknown scene — upstream strict validation rejects the
            // config before it reaches the render thread, so this is
            // defense-in-depth: keep the previous values.
            crate::lr_trace!("scene='{}' unknown — keeping previous scene values", v);
        }
    }
}
