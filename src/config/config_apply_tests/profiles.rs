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
    // `base-scene` is restored with cleaner inheritance semantics.
    // Custom scenes can opt into a built-in scene's defaults via
    // `base-scene = <name>`, then override individual fields. Without
    // `base-scene`, custom scenes fall back to cinematic's defaults
    // (matching behavior).
    // The config namespace is `[scene-custom.<name>]` (the [profile.<name>]
    // fallback was removed — users must rename the prefix).
    //
    // `monolith-size` removed from scene-custom (forbidden per owner
    // contract — collides with ambient simplification). It's now a top-level
    // / scene-managed field only.
    "scene-custom.nightcore.color = purple\n\
     scene-custom.nightcore.charset = binary\n\
     scene-custom.nightcore.speed = 24\n\
     scene-custom.nightcore.density = 0.70\n\
     scene-custom.nightcore.glitch-level = subtle\n"
}

#[test]
fn cli_profile_loads_user_profile_from_config() {
    //  `--scene-custom` resolves ONLY `[scene-custom.<name>]` blocks.
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
    // `monolith-size` is forbidden in scene-custom blocks per owner
    // contract. args.monolith_size retains its default (Normal) — users who
    // want a different monolith-size must set it as a top-level config key.
    assert_eq!(args.monolith_size, MonolithSize::Normal);
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
fn profile_base_scene_applies_inherited_defaults() {
    // `base-scene` is restored. A custom scene with
    // `base-scene = monolith` + `color = green` should inherit monolith's
    // defaults (speed=30, density=0.85, charset=zen, glitch=Subtle) and
    // then override only color to green.
    let args = args_with_config(
        "scene-custom.nightcore.base-scene = monolith\n\
         scene-custom.nightcore.color = green\n",
        &["--scene-custom", "nightcore"],
    );
    assert_eq!(args.scene.as_deref(), Some("nightcore"));
    // color override wins.
    assert_eq!(args.color, "green");
    // Inherited from monolith: speed=30.0, density=0.85, charset=zen.
    assert_eq!(args.speed, 30.0);
    assert!((args.density - 0.85).abs() < f32::EPSILON);
    assert_eq!(args.charset, "zen");
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
    // `monolith-size` is forbidden in scene-custom blocks per owner
    // contract. The nightcore scene no longer sets it, so args.monolith_size
    // retains its default (Normal) regardless of CLI overrides.
    assert_eq!(args.monolith_size, MonolithSize::Normal);
}

#[test]
fn config_profile_applies_after_config_scene() {
    //  --scene-custom CLI flag wins over `scene = signal` config key.
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
    //  --scene-custom wins over --scene for the args.scene value
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
    //  --scene-custom resolves only [scene-custom.<name>] blocks.
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
    // `base-scene = monolith` is now recognized and applies
    // monolith's defaults (speed=30, density=0.85, charset=zen). The
    // remaining invalid fields (color=not-a-color, speed=0, density=nope)
    // fail to parse and are ignored — monolith's inherited values survive
    // for those fields. Only color falls back to monolith's color
    // (energy-zen) because the override failed.
    let config = "scene-custom.bad.base-scene = monolith\n\
                  scene-custom.bad.color = not-a-color\n\
                  scene-custom.bad.speed = 0\n\
                  scene-custom.bad.density = nope\n";
    let args = args_with_config(config, &["--scene-custom", "bad"]);
    assert_eq!(args.scene.as_deref(), Some("bad"));
    // color=not-a-color failed → monolith's energy-zen inherited.
    assert_eq!(args.color, "energy-zen");
    // speed=0 failed → monolith's 30.0 inherited.
    assert_eq!(args.speed, 30.0);
    // density=nope failed → monolith's 0.85 inherited.
    assert!((args.density - 0.85).abs() < f32::EPSILON);
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
    assert_eq!(args.color, "energy-zen");
    assert_eq!(args.speed, 9.0);
}

// Color precedence vs Crystal Dragon drift clarity tests

#[test]
fn config_color_overridden_by_config_preset_is_precedence_not_drift() {
    // v14.0.0: `preset = cinematic` redirects to scene = cinematic, but
    // `scene = monolith` (also in config) wins because the scene handler
    // runs after the preset redirect. Scenes only fill UNSET keys, so
    // config `color = sun` is preserved (config wins over scene default).
    // color is from config, not drift.
    let args = args_with_config("color = sun\npreset = cinematic\nscene = monolith\n", &[]);
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
    //  [profile.<name>] fallback removed; use [scene-custom.<name>].
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
    // removed fields: no assertion needed.
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
