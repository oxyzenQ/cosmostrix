// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Scene + glitch level config application — extracted from
//! `config_apply.rs` to keep that file under the 800-LOC cap.

use std::collections::HashSet;

use super::is_explicit;
use crate::config::{Args, GlitchLevel};
use crate::scene::get_scene;

pub(crate) fn apply_scene_values(
    matches: &clap::ArgMatches,
    args: &mut Args,
    config_touched: &HashSet<&'static str>,
) -> Result<HashSet<&'static str>, String> {
    let mut scene_modified = HashSet::new();
    let Some(ref scene_name) = args.scene else {
        return Ok(scene_modified);
    };

    // v50.0.0-beta.6 Option D: accept custom scene names (not just builtin).
    // Previously validate_scene_name() would reject any name not in the
    // builtin scene list, forcing users to use --scene-custom for custom
    // scenes. Now we just normalize the name — builtin defaults are only
    // applied if get_scene() finds a match (the if-let guard below).
    // Custom scenes get their values applied via apply_scene_custom_layer.
    let name = scene_name.trim().to_ascii_lowercase();
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

pub(crate) fn apply_glitch_level_values(
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
    // (CLI-V-4): corrected flag names (--glitchpct not --glitch-pct) and
    // added --maxdpc to the list.

    match args.glitch_level {
        GlitchLevel::None => {
            // (Glitch-BUG8): explicitly reset all 5 preset fields to the
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
