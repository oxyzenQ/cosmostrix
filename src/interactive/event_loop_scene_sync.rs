// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Runtime scene synchronization for live-reload — extracted from
//! `event_loop.rs` to keep that file under the 800-LOC cap.
//!
//! Owns the scene-family base resolution used before every
//! `rebuild_cloud_config` call:
//!
//! - `resolve_scene_base_action()` — pure decision from the config map
//!   delta (key present / key just removed / key never present);
//! - `ambient_removed_between_maps()` — v80.0.0-beta.1 pure detector for the
//!   ambient overlay lifting (all `ambient.*` keys removed between the
//!   previously applied map and the new one);
//! - `sync_base_cfg_with_runtime_scene()` — re-applies a runtime scene's
//!   managed defaults onto the base (shortkey `x`/`X` cycles, ambient
//!   fires — the runtime scene must survive unrelated config edits);
//! - `restore_locked_scene_family()` — v80.0.0-beta.1 CLI-locked fallback: rolls
//!   the scene family back to the pristine startup snapshot when the
//!   config `scene` override is removed, and v80.0.0-beta.1: when the ambient
//!   schedule is removed while it owned the visual state (owner
//!   contract, 2026-09-01).

use std::collections::HashMap;

use crate::CloudConfig;

/// v80.0.0-beta.1 masterclass: what the rebuild base should do with the scene
/// family, derived from the config map delta.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum SceneBaseAction {
    /// Config `scene` key present — `rebuild_cloud_config`'s scene block
    /// applies it (config wins: the key is the most recent user intent).
    ApplyConfig,
    /// The config `scene` override was just REMOVED (present in the
    /// previously applied map, absent now — e.g. commented back out).
    /// The scene family reverts to the LOCKED startup snapshot so the
    /// rebuild falls back to the CLI-locked (startup-effective) scene.
    RestoreLocked,
    /// No scene key before or now — preserve the runtime scene (shortkey
    /// `x`/`X` cycles, ambient fires, startup scene) by syncing its
    /// managed defaults into the base.
    SyncRuntime,
}

/// Resolve the scene-base action from the config map delta.
///
/// Pure function (no I/O) — unit-testable in isolation:
/// - new map has `scene` → [`SceneBaseAction::ApplyConfig`];
/// - new map lacks `scene` but the previous map had it →
///   [`SceneBaseAction::RestoreLocked`] (the override disappeared:
///   fall back to the CLI lock);
/// - neither map has `scene` → [`SceneBaseAction::SyncRuntime`]
///   (keep whatever runtime scene is active).
pub(super) fn resolve_scene_base_action(
    new_map: &HashMap<String, String>,
    prev_map: Option<&HashMap<String, String>>,
) -> SceneBaseAction {
    if new_map.contains_key("scene") {
        SceneBaseAction::ApplyConfig
    } else if prev_map.is_some_and(|m| m.contains_key("scene")) {
        SceneBaseAction::RestoreLocked
    } else {
        SceneBaseAction::SyncRuntime
    }
}

/// v80.0.0-beta.1 ambient overlay rule (owner contract extension): did the config
/// map delta REMOVE the ambient schedule (all `ambient.*` keys commented
/// out)?
///
/// Pure function on the raw config maps — deliberately NOT on the
/// runtime `last_ambient_schedule`, which the ground-truth nuke may have
/// already cleared before a rebuild arrives (the applied-map pair is the
/// durable record of what the engine last agreed to).
///
/// Used by `apply_config_rebuild`: when the ambient keys disappear while
/// the ambient phase owns the visual state, the SyncRuntime arm is
/// upgraded to RestoreLocked — the ambient overlay lifts and the scene
/// family falls back to the locked startup values, mirroring the plain
/// `scene`-key contract (config present wins, absent reverts to the CLI
/// lock). Without this, commenting out `ambient.*` left the engine stuck
/// on the ambient-applied scene (the same "last value sticks" defect
/// family v80.0.0-beta.1 fixed for the scene key).
pub(super) fn ambient_removed_between_maps(
    new_map: &HashMap<String, String>,
    prev_map: Option<&HashMap<String, String>>,
) -> bool {
    let prev_had_ambient = prev_map.is_some_and(|m| {
        !crate::crystal_dragon_engine::ambient::collect_ambient_schedule(m)
            .entries
            .is_empty()
    });
    prev_had_ambient
        && crate::crystal_dragon_engine::ambient::collect_ambient_schedule(new_map)
            .entries
            .is_empty()
}

/// v80.0.0-beta.1: full scene-base decision including the ambient overlay rule.
///
/// Composes [`resolve_scene_base_action`] with the ambient overlay lift:
/// when the plain `scene`-key delta resolves to `SyncRuntime` (no scene
/// key before or now), the ambient keys disappeared between the applied
/// maps, AND the ambient phase owns the visual state (no user override
/// since the last ambient apply + the live scene is the one ambient
/// applied), the decision upgrades to `RestoreLocked` — the overlay lifts
/// and the scene family falls back to the locked startup values.
///
/// Pure function over its inputs — the ambient-ownership state
/// (`user_override_since_ambient` + the last applied ambient entry's
/// scene) is passed in by the caller from the live cloud state BEFORE
/// any ambient clearing runs.
pub(super) fn resolve_scene_base_with_ambient(
    new_map: &HashMap<String, String>,
    prev_map: Option<&HashMap<String, String>>,
    runtime_scene: &str,
    user_override_since_ambient: bool,
    last_applied_ambient_scene: Option<&str>,
) -> SceneBaseAction {
    let action = resolve_scene_base_action(new_map, prev_map);
    if action != SceneBaseAction::SyncRuntime {
        // The plain scene-key delta already decided (config key present
        // wins / key removed restores) — ambient ownership cannot
        // override those arms.
        return action;
    }
    let ambient_owns_visual =
        !user_override_since_ambient && last_applied_ambient_scene == Some(runtime_scene);
    if ambient_removed_between_maps(new_map, prev_map) && ambient_owns_visual {
        return SceneBaseAction::RestoreLocked;
    }
    SceneBaseAction::SyncRuntime
}

/// Restore the locked startup scene family onto the rebuild base.
///
/// v80.0.0-beta.1 masterclass (owner contract, 2026-09-01): mirrors
/// `sync_base_cfg_with_runtime_scene`'s managed-field set (plus `density`,
/// which travels with `base_density`) — exactly the fields the sync may
/// have contaminated while a config-driven scene was active. Fields
/// outside the scene family (fps, glitch, bold, message, …) are never
/// touched: only the sync writes them into the base, and it does not
/// write those.
///
/// `startup_cfg` is the pristine startup snapshot (CLI >
/// config@startup resolution baked in, never mutated for the whole
/// session — `run_interactive` clones it next to `base_cfg`).
pub(super) fn restore_locked_scene_family(base_cfg: &mut CloudConfig, startup_cfg: &CloudConfig) {
    // The revert EVENT is traced by the caller (event_loop_config_rebuild.rs)
    // so it always appears in the debug log; here the field copies are
    // silent — the "Cloud rebuilt" summary line shows the resulting values.
    base_cfg.scene_name = startup_cfg.scene_name.clone();
    base_cfg.color_scheme = startup_cfg.color_scheme;
    base_cfg.charset_preset = startup_cfg.charset_preset.clone();
    base_cfg.chars = startup_cfg.chars.clone();
    base_cfg.speed = startup_cfg.speed;
    base_cfg.density = startup_cfg.density;
    base_cfg.base_density = startup_cfg.base_density;
    base_cfg.rain_style = startup_cfg.rain_style;
}

/// Sync `base_cfg` with the runtime scene's managed defaults.
///
/// v50.0.0-beta.6 masterclass: called before `rebuild_cloud_config` so
/// config overrides layer on top of scene defaults (user wins: editing
/// `color` changes only color, not the whole scene profile).
///
/// Behavior:
/// 1. If `base_cfg.scene_name == scene_name`, returns immediately (no-op
///    — already synced). This is the common case on the first rebuild
///    after startup (scene hasn't changed).
/// 2. Updates `base_cfg.scene_name` to the new scene.
/// 3. Looks up the scene via `crate::scene::get_scene`. If not found
///    (custom scene deleted from config mid-session), returns without
///    applying defaults — the previous values persist.
/// 4. For each managed field the scene defines (`color`, `charset`,
///    `speed`, `density`, `rain_style`), overwrites the corresponding
///    `base_cfg` field. Fields the scene leaves `None` are NOT touched
///    (preserves user config values for those dimensions).
///
/// Charset handling: when the scene defines `charset`, both
/// `base_cfg.charset_preset` (the name) and `base_cfg.chars` (the
/// resolved glyph Vec) are updated — the latter via
/// `charset::build_chars` using `base_cfg.user_ranges` + `base_cfg.def_ascii`
/// so the user's custom ranges + ASCII-fallback flag are respected.
///
/// Parameters:
/// - `base_cfg`: mutable reference to the CloudConfig used as the rebuild
///   base (cloned from the startup `cfg` at the top of `run_interactive`).
/// - `scene_name`: the runtime scene name (from the event loop's
///   `scene_name` local, which is updated by `x`/`X`/ambient fires).
///
/// v80.0.0-beta.1: the caller (event_loop_config_rebuild.rs) invokes this only on
/// the [`SceneBaseAction::SyncRuntime`] branch — never while a
/// config `scene` key is present, and never on the restore branch (the
/// restore rolls this sync's contamination back to the startup values).
pub(super) fn sync_base_cfg_with_runtime_scene(base_cfg: &mut CloudConfig, scene_name: &str) {
    if base_cfg.scene_name == scene_name {
        return;
    }
    base_cfg.scene_name = scene_name.to_string();
    let Some(scene_info) = crate::scene::get_scene(scene_name) else {
        return;
    };
    let sc = scene_info.config;
    if let Some(color) = sc.color {
        if let Ok(scheme) = crate::cli::parse_color_scheme(color) {
            base_cfg.color_scheme = scheme;
        }
    }
    if let Some(charset_name) = sc.charset {
        base_cfg.charset_preset = charset_name.to_string();
        if let Ok(charset) = crate::charset::charset_from_str(charset_name, base_cfg.def_ascii) {
            base_cfg.chars =
                crate::charset::build_chars(charset, &base_cfg.user_ranges, base_cfg.def_ascii);
        }
    }
    if let Some(speed) = sc.speed {
        base_cfg.speed = speed;
    }
    if let Some(density) = sc.density {
        base_cfg.base_density = density;
    }
    base_cfg.rain_style = sc.rain_style;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixture shaped like the owner's exact run:
    /// `cosmostrix -v -s -C minimal --scene crystal-dragon -mfs words`
    /// (scene CLI-locked to crystal-dragon; no --speed/--density/--color).
    fn locked_crystal_dragon_cfg() -> CloudConfig {
        CloudConfig {
            color_mode: crate::runtime::ColorMode::TrueColor,
            shading_mode: crate::runtime::ShadingMode::Random,
            bold_mode: crate::runtime::BoldMode::Random,
            async_mode: true,
            default_bg: true,
            color_scheme: crate::runtime::ColorScheme::EnergyZen,
            custom_palette: None,
            custom_palette_name: None,
            // crystal-dragon scene profile (see SCENES table):
            rain_style: crate::rain_style::RainStyle::Monolith,
            glitch_enabled: true,
            glitch_level: crate::config::GlitchLevel::Subtle,
            glitch_pct: 3.0,
            glitch_low: 300,
            glitch_high: 400,
            linger_low: 400,
            linger_high: 600,
            short_pct: 50.0,
            die_early_pct: 33.0,
            max_dpc: 5,
            density: 0.78,
            speed: 30.0,
            monolith_size: crate::runtime::MonolithSize::Normal,
            chars: vec!['0', '1'],
            message: None,
            message_border: false,
            msg_fill_style: crate::msg_fill_style::MsgFillStyle::Typewriter,
            target_fps: 60.0,
            xtermjs_host: false,
            default_fps_cap: 240.0,
            duration: None,
            duration_s: None,
            bench_frames: None,
            benchmark: false,
            bench_duration: None,
            save_baseline: None,
            compare_baseline: None,
            bench_io: false,
            bench_all: false,
            bench_scene: None,
            screen_size: None,
            color_tune: crate::color_tune::ColorTune::IDENTITY,
            json: false,
            verbose: false,
            density_auto: true,
            base_density: 0.78,
            perf_stats: false,
            screensaver: false,
            intro: crate::intro_style::IntroType::None,
            intro_color: None,
            mouse: false,
            charset_preset: "zen".to_string(),
            user_ranges: vec![],
            def_ascii: false,
            crystal_dragon: true,
            power_dragon: true,
            msg_mode: true,
            effects_enabled: true,
            monolith_density_map: None,
            config_path_for_watcher: None,
            scene_name: "crystal-dragon".to_string(),
            scene_custom_name: None,
            cli_explicit: crate::app::CliExplicit {
                scene: true,
                ..crate::app::CliExplicit::default()
            },
            ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
            ambient_snapback_secs: None,
        }
    }

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // ── resolve_scene_base_action: the config map delta rule ──────────

    #[test]
    fn resolver_key_present_applies_config_scene() {
        let new = map(&[("scene", "cinematic")]);
        let prev = map(&[]);
        assert_eq!(
            resolve_scene_base_action(&new, Some(&prev)),
            SceneBaseAction::ApplyConfig
        );
    }

    #[test]
    fn resolver_key_present_wins_even_when_prev_also_had_it() {
        let new = map(&[("scene", "neon")]);
        let prev = map(&[("scene", "cinematic")]);
        assert_eq!(
            resolve_scene_base_action(&new, Some(&prev)),
            SceneBaseAction::ApplyConfig
        );
    }

    #[test]
    fn resolver_key_removed_restores_locked() {
        // The owner's exact edit: `scene = cinematic` commented back out.
        let new = map(&[]);
        let prev = map(&[("scene", "cinematic")]);
        assert_eq!(
            resolve_scene_base_action(&new, Some(&prev)),
            SceneBaseAction::RestoreLocked
        );
    }

    #[test]
    fn resolver_key_never_present_syncs_runtime() {
        // Shortkey-cycled scene (never a config scene) + unrelated edit.
        let new = map(&[("fps", "60")]);
        let prev = map(&[]);
        assert_eq!(
            resolve_scene_base_action(&new, Some(&prev)),
            SceneBaseAction::SyncRuntime
        );
    }

    #[test]
    fn resolver_first_reload_no_prev_map_syncs_runtime() {
        let new = map(&[("fps", "60")]);
        assert_eq!(
            resolve_scene_base_action(&new, None),
            SceneBaseAction::SyncRuntime
        );
    }

    // ── restore_locked_scene_family: rolling back sync contamination ──

    #[test]
    fn restore_rolls_back_runtime_scene_sync_contamination() {
        let startup = locked_crystal_dragon_cfg();
        let mut base = startup.clone();
        // Simulate a config-driven cinematic phase: the runtime scene sync
        // copied cinematic's managed defaults into the base.
        sync_base_cfg_with_runtime_scene(&mut base, "cinematic");
        assert_eq!(base.scene_name, "cinematic");
        assert_eq!(base.speed, 9.0);
        assert_eq!(base.base_density, 0.75);
        assert_eq!(base.rain_style, crate::rain_style::RainStyle::Glyph);

        restore_locked_scene_family(&mut base, &startup);
        assert_eq!(base.scene_name, "crystal-dragon");
        assert_eq!(base.speed, 30.0);
        assert_eq!(base.density, 0.78);
        assert_eq!(base.base_density, 0.78);
        assert_eq!(base.rain_style, crate::rain_style::RainStyle::Monolith);
        assert_eq!(base.color_scheme, startup.color_scheme);
        assert_eq!(base.charset_preset, startup.charset_preset);
        assert_eq!(base.chars, startup.chars);
    }

    #[test]
    fn restore_is_noop_when_base_matches_startup() {
        let startup = locked_crystal_dragon_cfg();
        let mut base = startup.clone();
        restore_locked_scene_family(&mut base, &startup);
        assert_eq!(base.scene_name, startup.scene_name);
        assert_eq!(base.speed, startup.speed);
    }

    // ── sync: shortkey scene preservation (existing behavior locked) ──

    #[test]
    fn sync_applies_runtime_scene_managed_defaults() {
        let mut base = locked_crystal_dragon_cfg();
        sync_base_cfg_with_runtime_scene(&mut base, "cinematic");
        assert_eq!(base.scene_name, "cinematic");
        assert_eq!(base.speed, 9.0);
        assert_eq!(base.base_density, 0.75);
        assert_eq!(base.rain_style, crate::rain_style::RainStyle::Glyph);
    }

    // ── the owner's exact end-to-end scenario (v80.0.0-beta.1 contract) ────────

    /// Owner repro: run `--scene crystal-dragon`, edit config.toml to
    /// uncomment `scene = cinematic` (live-reload switches — good), then
    /// comment it back out. Before v80.0.0-beta.1 the engine STAYED on cinematic
    /// (the runtime scene sync contaminated the rebuild base + the CLI
    /// lock was zeroed at the first reload). The contract: the engine
    /// detects no config value to override the CLI and falls back to the
    /// locked crystal-dragon — no exit, no rerun.
    #[test]
    fn owner_scenario_scene_comment_out_reverts_to_cli_lock() {
        let startup = locked_crystal_dragon_cfg();
        let mut base = startup.clone(); // what run_interactive holds
        let mut prev_map: Option<HashMap<String, String>> = Some(map(&[]));

        // Phase 1 — config gains `scene = cinematic` (uncommented).
        let phase1 = map(&[("scene", "cinematic")]);
        let action = resolve_scene_base_action(&phase1, prev_map.as_ref());
        assert_eq!(action, SceneBaseAction::ApplyConfig);
        let cfg1 = crate::live_config::rebuild_cloud_config(&base, &phase1);
        assert_eq!(cfg1.scene_name, "cinematic");
        assert_eq!(cfg1.speed, 9.0);
        assert_eq!(cfg1.rain_style, crate::rain_style::RainStyle::Glyph);
        prev_map = Some(phase1);

        // Phase 2 — `# scene = cinematic` (commented back out).
        let phase2 = map(&[]);
        let action = resolve_scene_base_action(&phase2, prev_map.as_ref());
        assert_eq!(action, SceneBaseAction::RestoreLocked);
        restore_locked_scene_family(&mut base, &startup);
        let cfg2 = crate::live_config::rebuild_cloud_config(&base, &phase2);
        assert_eq!(
            cfg2.scene_name, "crystal-dragon",
            "commenting the scene key out must revert to the CLI-locked scene (owner contract)"
        );
        assert_eq!(cfg2.speed, 30.0, "crystal-dragon's speed returns");
        assert_eq!(cfg2.base_density, 0.78, "crystal-dragon's density returns");
        assert_eq!(
            cfg2.rain_style,
            crate::rain_style::RainStyle::Monolith,
            "crystal-dragon's rain style returns"
        );
        prev_map = Some(phase2);

        // Phase 3 — another unrelated edit: scene stays locked (the sync
        // branch is a no-op because the runtime scene is crystal-dragon
        // again, matching the base).
        let phase3 = map(&[("fps", "60")]);
        let action = resolve_scene_base_action(&phase3, prev_map.as_ref());
        assert_eq!(action, SceneBaseAction::SyncRuntime);
        sync_base_cfg_with_runtime_scene(&mut base, "crystal-dragon");
        let cfg3 = crate::live_config::rebuild_cloud_config(&base, &phase3);
        assert_eq!(cfg3.scene_name, "crystal-dragon");
        assert_eq!(cfg3.target_fps, 60.0);
    }

    // ── v80.0.0-beta.1: ambient overlay lift (ambient snapback contract) ──────

    #[test]
    fn ambient_removed_detector_pure_map_delta() {
        // Ambient keys present before, absent now → removed.
        let new = map(&[]);
        let prev = map(&[("ambient.09-00", "cinematic")]);
        assert!(ambient_removed_between_maps(&new, Some(&prev)));
        // Still present (edited, not removed) → not removed.
        let new_edited = map(&[("ambient.09-00", "monolith")]);
        assert!(!ambient_removed_between_maps(&new_edited, Some(&prev)));
        // Never present → not removed (nothing lifts).
        assert!(!ambient_removed_between_maps(
            &new,
            Some(&map(&[("fps", "60")]))
        ));
        // No previous map (first reload) → cannot be "removed".
        assert!(!ambient_removed_between_maps(&new, None));
        // Partial removal (one of two keys left) → schedule still active.
        let prev_two = map(&[
            ("ambient.09-00", "cinematic"),
            ("ambient.21-00", "monolith"),
        ]);
        let one_left = map(&[("ambient.21-00", "monolith")]);
        assert!(!ambient_removed_between_maps(&one_left, Some(&prev_two)));
    }

    #[test]
    fn ambient_removal_with_ambient_owned_scene_restores_locked() {
        // Ambient applied cinematic (no user override since) + all ambient
        // keys commented out → RestoreLocked (NOT SyncRuntime).
        let new = map(&[]);
        let prev = map(&[("ambient.09-00", "cinematic")]);
        assert_eq!(
            resolve_scene_base_with_ambient(
                &new,
                Some(&prev),
                "cinematic",
                false,             // user_override_since_ambient
                Some("cinematic"), // last applied ambient scene
            ),
            SceneBaseAction::RestoreLocked
        );
    }

    #[test]
    fn ambient_removal_with_user_override_keeps_runtime_scene() {
        // User pressed `x` after ambient applied (user_override=true) →
        // their scene survives the ambient removal.
        let new = map(&[]);
        let prev = map(&[("ambient.09-00", "cinematic")]);
        assert_eq!(
            resolve_scene_base_with_ambient(&new, Some(&prev), "signal", true, Some("cinematic")),
            SceneBaseAction::SyncRuntime
        );
    }

    #[test]
    fn ambient_removal_with_scene_key_present_applies_config() {
        // The scene key still wins: config scene applies even while the
        // ambient overlay lifts (plain-key delta outranks ownership).
        let new = map(&[("scene", "neon")]);
        let prev = map(&[("ambient.09-00", "cinematic")]);
        assert_eq!(
            resolve_scene_base_with_ambient(
                &new,
                Some(&prev),
                "cinematic",
                false,
                Some("cinematic")
            ),
            SceneBaseAction::ApplyConfig
        );
    }

    #[test]
    fn ambient_present_unrelated_edit_syncs_runtime() {
        // Ambient keys still present + unrelated edit → SyncRuntime (the
        // rebuild-reapply block re-applies the ambient entry anyway).
        let new = map(&[("fps", "60"), ("ambient.09-00", "cinematic")]);
        let prev = map(&[("ambient.09-00", "cinematic")]);
        assert_eq!(
            resolve_scene_base_with_ambient(
                &new,
                Some(&prev),
                "cinematic",
                false,
                Some("cinematic")
            ),
            SceneBaseAction::SyncRuntime
        );
    }

    #[test]
    fn drift_changed_scene_is_not_ambient_owned() {
        // Crystal Dragon drift moved the scene away from the ambient
        // entry's scene → the runtime scene is NOT ambient-owned → keep
        // it (SyncRuntime), even though ambient is being removed.
        let new = map(&[]);
        let prev = map(&[("ambient.09-00", "cinematic")]);
        assert_eq!(
            resolve_scene_base_with_ambient(&new, Some(&prev), "aurora", false, Some("cinematic")),
            SceneBaseAction::SyncRuntime
        );
    }

    /// Owner repro (ambient variant of the v80.0.0-beta.1 contract): run with
    /// `--scene crystal-dragon` + `ambient.09-00 = cinematic`. Ambient
    /// applies (snapback/rx), then the user comments out ALL ambient keys.
    /// Before v80.0.0-beta.1 the engine STAYED on cinematic (SyncRuntime synced the
    /// ambient scene into the rebuild base). The contract: the ambient
    /// overlay lifts → the engine falls back to the locked crystal-dragon
    /// — no exit, no rerun.
    #[test]
    fn owner_scenario_ambient_comment_out_reverts_to_cli_lock() {
        let startup = locked_crystal_dragon_cfg();
        let mut base = startup.clone(); // what run_interactive holds
        let mut prev_map: Option<HashMap<String, String>> =
            Some(map(&[("ambient.09-00", "cinematic")]));

        // Phase 1 — ambient phase applied cinematic (rx event / snapback).
        // Model the ambient apply: runtime scene is cinematic, ambient
        // owns the visual state.
        let runtime_scene = "cinematic";
        let user_override = false;
        let last_ambient_scene = Some("cinematic");

        // Phase 2 — all `ambient.*` keys commented out (live-reload fires).
        let phase2 = map(&[]);
        let action = resolve_scene_base_with_ambient(
            &phase2,
            prev_map.as_ref(),
            runtime_scene,
            user_override,
            last_ambient_scene,
        );
        assert_eq!(
            action,
            SceneBaseAction::RestoreLocked,
            "ambient comment-out must lift the overlay (restore the CLI lock)"
        );
        // The runtime scene sync contaminated the base first (pre-v80.0.0-beta.1
        // behavior) — model that, then the restore must roll it back.
        sync_base_cfg_with_runtime_scene(&mut base, runtime_scene);
        restore_locked_scene_family(&mut base, &startup);
        let cfg2 = crate::live_config::rebuild_cloud_config(&base, &phase2);
        assert_eq!(
            cfg2.scene_name, "crystal-dragon",
            "ambient removal must revert to the CLI-locked scene (owner contract)"
        );
        assert_eq!(cfg2.speed, 30.0, "crystal-dragon's speed returns");
        assert_eq!(cfg2.base_density, 0.78, "crystal-dragon's density returns");
        assert_eq!(
            cfg2.rain_style,
            crate::rain_style::RainStyle::Monolith,
            "crystal-dragon's rain style returns"
        );
        prev_map = Some(phase2);

        // Phase 3 — an unrelated edit afterwards: scene stays locked.
        let phase3 = map(&[("fps", "60")]);
        let action3 = resolve_scene_base_with_ambient(
            &phase3,
            prev_map.as_ref(),
            "crystal-dragon",
            false,
            None, // ambient tracker cleared by the removal
        );
        assert_eq!(action3, SceneBaseAction::SyncRuntime);
        sync_base_cfg_with_runtime_scene(&mut base, "crystal-dragon"); // no-op
        let cfg3 = crate::live_config::rebuild_cloud_config(&base, &phase3);
        assert_eq!(cfg3.scene_name, "crystal-dragon");
    }

    /// Lockless mirror: no CLI scene lock — startup scene resolution came
    /// from config/default. Ambient applied a phase, then ambient is
    /// removed → the overlay lifts back to the startup resolution (the
    /// lockless comment-out contract: revert to the startup-effective
    /// family, matching the alpha.7 reset-on-comment pattern).
    #[test]
    fn lockless_ambient_comment_out_reverts_to_startup_scene() {
        let mut startup = locked_crystal_dragon_cfg();
        // Lockless: no CLI scene flag — startup scene = the resolution
        // without any CLI override.
        startup.cli_explicit = crate::app::CliExplicit::default();
        let mut base = startup.clone();
        let prev = map(&[("ambient.09-00", "cinematic")]);

        let action = resolve_scene_base_with_ambient(
            &map(&[]),
            Some(&prev),
            "cinematic",
            false,
            Some("cinematic"),
        );
        assert_eq!(action, SceneBaseAction::RestoreLocked);
        sync_base_cfg_with_runtime_scene(&mut base, "cinematic");
        restore_locked_scene_family(&mut base, &startup);
        let cfg = crate::live_config::rebuild_cloud_config(&base, &map(&[]));
        assert_eq!(cfg.scene_name, "crystal-dragon");
    }
}
