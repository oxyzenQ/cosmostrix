// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::atomic::{AtomicU64, Ordering};

use clap::{CommandFactory, FromArgMatches};

use crate::config::{Args, GlitchLevel};
use crate::config_apply::apply_config_and_runtime_defaults;
use crate::rain_style::RainStyle;
use crate::runtime::MonolithSize;

/// Global counter for unique temp file names. Prevents collisions when
/// multiple tests run in parallel and `SystemTime::now()` returns the
/// same nanosecond on fast CI runners.
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Set COSMOSTRIX_TEST_CONFIG_DIR so is_safe_path allows the actual temp
/// directory during tests. Uses `std::env::temp_dir()` instead of hardcoding
/// `/tmp` because on some CI runners (macOS Arch packaging) temp_dir()
/// returns `/var/cache/makepkg-build/tmp/` rather than `/tmp`, causing
/// path validation to reject config files in the "wrong" temp location.
fn ensure_test_config_dir_allowed() {
    std::env::set_var("COSMOSTRIX_SKIP_STARTUP_VALIDATION", "1");
    std::env::set_var(
        "COSMOSTRIX_TEST_CONFIG_DIR",
        std::env::temp_dir().to_string_lossy().into_owned(),
    );
}

pub(crate) fn args_with_config_result(config: &str, cli: &[&str]) -> Result<Args, String> {
    ensure_test_config_dir_allowed();
    let mut path = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    let seq = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.push(format!(
        "cosmostrix-profile-test-{}-{nanos}-{seq}.toml",
        std::process::id(),
    ));
    std::fs::write(&path, config).expect("write temp config");

    let path_string = path.to_string_lossy().into_owned();
    let mut argv = vec!["cosmostrix", "--config", path_string.as_str()];
    argv.extend_from_slice(cli);

    let cmd = Args::command();
    let matches = cmd.get_matches_from(argv);
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    let result = apply_config_and_runtime_defaults(&matches, &mut args).map(|()| args);

    let _ = std::fs::remove_file(path);
    result
}

pub(crate) fn args_with_config(config: &str, cli: &[&str]) -> Args {
    args_with_config_result(config, cli).expect("apply profile config")
}

fn nightcore_config() -> &'static str {
    // v20.1: `base-scene` and `preset` are removed entirely. Custom scenes
    // stand on their own; missing fields fall back to cinematic's defaults.
    // The config namespace is `[scene-custom.<name>]` (the [profile.<name>]
    // fallback was also removed in v20.1 — users must rename the prefix).
    "scene-custom.nightcore.color = purple\n\
     scene-custom.nightcore.charset = binary\n\
     scene-custom.nightcore.speed = 24\n\
     scene-custom.nightcore.density = 0.70\n\
     scene-custom.nightcore.glitch-level = subtle\n\
     scene-custom.nightcore.monolith-size = large\n"
}

#[test]
fn cli_profile_loads_user_profile_from_config() {
    // v20.1: `--scene-custom` resolves ONLY `[scene-custom.<name>]` blocks.
    // The [profile.<name>] fallback was removed. Custom scenes stand on
    // their own — missing fields fall back to cinematic's defaults.
    // args.scene is set to the custom scene name ("nightcore").
    let args = args_with_config(nightcore_config(), &["--scene-custom", "nightcore"]);
    assert_eq!(args.scene_custom.as_deref(), Some("nightcore"));
    assert_eq!(args.scene.as_deref(), Some("nightcore"));
    assert_eq!(args.color, "purple");
    assert_eq!(args.charset, "binary");
    assert_eq!(args.speed, 24.0);
    assert!((args.density - 0.70).abs() < f32::EPSILON);
    assert_eq!(args.glitch_level, GlitchLevel::Subtle);
    assert_eq!(args.monolith_size, MonolithSize::Large);
    // v20: args.scene is now the custom scene name, so rain_style_for_scene
    // returns None (custom scenes are not built-in) and falls back to Glyph.
    assert_eq!(
        crate::scene::rain_style_for_scene(args.scene.as_deref().unwrap_or("")),
        None,
        "custom scene name should not resolve to a built-in rain_style"
    );
    let _ = RainStyle::Glyph; // ensure RainStyle import is used
}

#[test]
fn profile_base_monolith_is_unknown_key() {
    // v20.1: `base-scene` is no longer a recognized field. A config that
    // sets `base-scene` plus one valid field results in a scene-custom
    // block with just that one field; args falls back to DEFAULT_SCENE
    // (cinematic) for everything else. args.scene is set to the custom
    // scene name.
    let args = args_with_config(
        "scene-custom.nightcore.base-scene = monolith\n\
         scene-custom.nightcore.color = green\n",
        &["--scene-custom", "nightcore"],
    );
    assert_eq!(args.scene.as_deref(), Some("nightcore"));
    // color is set by the custom scene (the only recognized field).
    assert_eq!(args.color, "green");
    // Cinematic defaults for the rest: speed=9, density=0.75, glitch=Subtle.
    assert_eq!(args.speed, 9.0);
    assert!((args.density - 0.75).abs() < f32::EPSILON);
    assert_eq!(args.glitch_level, GlitchLevel::Subtle);
}

#[test]
fn explicit_cli_flags_override_profile_values() {
    let args = args_with_config(
        nightcore_config(),
        &[
            "--scene-custom",
            "nightcore",
            "--speed",
            "30",
            "--color",
            "green",
        ],
    );
    assert_eq!(args.color, "green");
    assert_eq!(args.speed, 30.0);
    assert!((args.density - 0.70).abs() < f32::EPSILON);
    assert_eq!(args.monolith_size, MonolithSize::Large);
}

#[test]
fn config_profile_applies_after_config_scene() {
    // v20.1: --scene-custom CLI flag wins over `scene = signal` config key.
    // args.scene is set to the custom scene name ("nightcore"), and the
    // custom scene's own fields override the cinematic defaults.
    let config = format!("scene = signal\n{}", nightcore_config());
    let args = args_with_config(&config, &["--scene-custom", "nightcore"]);
    assert_eq!(args.scene.as_deref(), Some("nightcore"));
    assert_eq!(args.color, "purple");
    assert_eq!(args.speed, 24.0);
}

#[test]
fn cli_profile_overrides_cli_scene_for_profile_foundation() {
    // v20.1: --scene-custom wins over --scene for the args.scene value
    // (custom scenes are first-class).
    let args = args_with_config(
        nightcore_config(),
        &["--scene", "signal", "--scene-custom", "nightcore"],
    );
    assert_eq!(args.scene.as_deref(), Some("nightcore"));
    assert_eq!(args.color, "purple");
    assert_eq!(args.speed, 24.0);
}

#[test]
fn unknown_cli_profile_has_clear_error() {
    // v20.1: --scene-custom resolves only [scene-custom.<name>] blocks.
    // Unknown custom scene must produce a clear error mentioning the
    // available names.
    let err =
        args_with_config_result(nightcore_config(), &["--scene-custom", "unknown"]).unwrap_err();
    assert!(
        err.contains("error: unknown custom scene 'unknown'"),
        "error must mention unknown custom scene: {err}"
    );
    assert!(
        err.contains("nightcore"),
        "error must list available names: {err}"
    );
}

#[test]
fn invalid_profile_values_are_ignored_cleanly() {
    // v20.1: `base-scene` is unknown and filtered out. The remaining
    // color/speed/density values fail to parse, so args retains the
    // DEFAULT_SCENE (cinematic) defaults.
    let config = "scene-custom.bad.base-scene = monolith\n\
                  scene-custom.bad.color = not-a-color\n\
                  scene-custom.bad.speed = 0\n\
                  scene-custom.bad.density = nope\n";
    let args = args_with_config(config, &["--scene-custom", "bad"]);
    assert_eq!(args.scene.as_deref(), Some("bad"));
    // Cinematic defaults: color=neon-purple, speed=9, density=0.75.
    assert_eq!(args.color, "neon-purple");
    assert_eq!(args.speed, 9.0);
    assert!((args.density - 0.75).abs() < f32::EPSILON);
}

#[test]
fn existing_config_without_profiles_still_works() {
    let args = args_with_config("scene = signal\ncolor = aurora\n", &[]);
    assert_eq!(args.scene_custom, None);
    assert_eq!(args.scene.as_deref(), Some("signal"));
    assert_eq!(args.color, "aurora");
}

#[test]
fn default_plain_runtime_profile_remains_cinematic() {
    let args = args_with_config("", &[]);
    assert_eq!(args.scene.as_deref(), Some("cinematic"));
    assert_eq!(args.color, "neon-purple");
    assert_eq!(args.speed, 9.0);
}

// Color precedence vs auto-drift clarity tests

#[test]
fn config_color_overridden_by_config_preset_is_precedence_not_drift() {
    // v14.0.0: `preset = cinematic` redirects to scene = cinematic, but
    // `scene = monolith` (also in config) wins because the scene handler
    // runs after the preset redirect. Scenes only fill UNSET keys, so
    // config `color = sun` is preserved (config wins over scene default).
    // auto_color_drift must remain false — the color is from config, not drift.
    let args = args_with_config(
        "color = sun\npreset = cinematic\nscene = monolith\nauto-color-drift = false\n",
        &[],
    );
    assert!(
        !args.auto_color_drift,
        "auto_color_drift must remain false; color is from config, not drift"
    );
    // v14.0.0: config color=sun wins over scene default (scenes only fill
    // unset keys, they do not override config-set keys).
    assert_eq!(
        args.color, "sun",
        "config color=sun must be preserved; scenes do not override config-set keys in v14"
    );
    assert_eq!(
        args.scene.as_deref(),
        Some("monolith"),
        "scene=monolith must win over preset=cinematic redirect (scene handler runs second)"
    );
}

#[test]
fn profile_color_resolves_sun_after_preset_and_scene() {
    // v20.1: [profile.<name>] fallback removed; use [scene-custom.<name>].
    // 'scene = monolith' is the config-level default, but --scene-custom
    // nightcore wins (custom scenes are first-class) and sets color = sun.
    let args = args_with_config(
        "scene = monolith\n\
         scene-custom.nightcore.color = sun\n\
         scene-custom.nightcore.charset = binary\n\
         scene-custom.nightcore.speed = 24\n\
         scene-custom.nightcore.density = 0.70\n\
         scene-custom.nightcore.glitch-level = subtle\n\
         scene-custom.nightcore.monolith-size = large\n",
        &["--scene-custom", "nightcore"],
    );
    assert_eq!(
        args.color, "sun",
        "custom scene color must override config scene color per precedence"
    );
    assert!(
        !args.auto_color_drift,
        "auto_color_drift must default false"
    );
}

#[test]
fn cli_color_wins_over_config_preset_and_scene() {
    // CLI --color (step 10) is the highest precedence and always wins.
    let args = args_with_config(
        "preset = cinematic\nscene = monolith\n",
        &["--color", "sun"],
    );
    assert_eq!(
        args.color, "sun",
        "CLI --color must override config preset/scene"
    );
}
