// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunter-27 regression suite: 'r' is a FULL FRESH.
//!
//! Owner directive: pressing 'r' must wipe every runtime user change
//! (shortkey cycles c/C/s/S, Up/Down speed, '['/']' density, and
//! live-reloaded config keys) and return to the CURRENT scene's
//! builtin defaults — e.g. sorgonemous_intrascals comes back as
//! energy-zen + binary + speed 12 + density 0.55, with the rain
//! replayed from zero (NIGHT-lts-3's relaunch semantics preserved).
//!
//! The dispatch goes through the production `handle_keybinding` path
//! (the same KeybindingCtx the event loop builds), so these tests
//! exercise the exact scene-application + relaunch chain.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::cloud::Cloud;
use crate::frame::Frame;
use crate::platform::TermReinit;
use crate::CloudConfig;

use crate::interactive::input::{handle_keybinding, KeyOutcome, KeybindingCtx};

// ── fixtures (mirrored from tests.rs — kept local so this suite
// stays self-contained) ─────────────────────────────────────────

fn hunter27_config() -> CloudConfig {
    use crate::intro_style::IntroType;
    use crate::msg_fill_style::MsgFillStyle;
    use crate::rain_style::RainStyle;
    use crate::runtime::{BoldMode, ColorMode, ColorScheme, MonolithSize, ShadingMode};

    CloudConfig {
        color_mode: ColorMode::TrueColor,
        shading_mode: ShadingMode::Random,
        bold_mode: BoldMode::Random,
        async_mode: true,
        default_bg: true,
        color_scheme: ColorScheme::NeonPurple,
        custom_palette: None,
        custom_palette_name: None,
        rain_style: RainStyle::Glyph,
        glitch_enabled: true,
        glitch_level: crate::config::GlitchLevel::Default,
        glitch_pct: 10.0,
        glitch_low: 300,
        glitch_high: 400,
        linger_low: 400,
        linger_high: 600,
        short_pct: 50.0,
        die_early_pct: 33.0,
        max_dpc: 5,
        density: 0.75,
        speed: 9.0,
        monolith_size: MonolithSize::Normal,
        chars: vec!['0', '1'],
        message: None,
        message_border: false,
        msg_fill_style: MsgFillStyle::Typewriter,
        target_fps: 60.0,
        xtermjs_host: false,
        default_fps_cap: 240.0,
        duration_s: None,
        bench_frames: None,
        benchmark: false,
        bench_duration: None,
        screen_size: None,
        color_tune: crate::color_tune::ColorTune::IDENTITY,
        json: false,
        save_baseline: None,
        compare_baseline: None,
        bench_io: false,
        bench_all: false,
        bench_scene: None,
        verbose: false,
        density_auto: true,
        base_density: 0.75,
        perf_stats: false,
        screensaver: false,
        intro: IntroType::None,
        intro_color: None,
        mouse: false,
        charset_preset: "binary".to_string(),
        user_ranges: vec![],
        def_ascii: false,
        crystal_dragon: false,
        power_dragon: true,
        msg_mode: true,
        effects_enabled: true,
        config_path_for_watcher: None,
        scene_name: "sorgonemous_intrascals".to_string(),
        scene_custom_name: None,
        scene_custom_config_owned: false,
        cli_explicit: crate::app::CliExplicit::default(),
        ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
        ambient_snapback_secs: None,
        crystal_dragon_secs: None,
    }
}

fn key(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
}

fn default_term_reinit() -> TermReinit {
    TermReinit::default()
}

/// A cloud seeded on the flagship scene (its builtin fields applied),
/// ready for the user to mutate with shortkeys.
fn flagship_cloud() -> Cloud {
    let cfg = hunter27_config();
    let mut cloud = cfg.create_cloud(0.55);
    // Apply the scene's builtin layer exactly like a scene launch does.
    let charset = cloud.apply_scene_runtime(
        "sorgonemous_intrascals",
        &cfg.charset_preset.clone(),
        &cfg.user_ranges,
        cfg.def_ascii,
    );
    cloud.set_scene_label("sorgonemous_intrascals");
    let _ = charset;
    cloud
}

/// Dispatch one key through the production path with an optional
/// config map (None = no live-reload state, the unit-test default).
fn dispatch(
    cloud: &mut Cloud,
    frame: &mut Frame,
    key_event: &KeyEvent,
    charset_preset: &mut String,
    scene_name: &mut String,
    scene_generation: &mut u64,
    cfg_map: Option<&std::collections::HashMap<String, String>>,
) -> KeyOutcome {
    let cfg = hunter27_config();
    let user_ranges: [(char, char); 0] = [];
    let term_reinit = default_term_reinit();
    handle_keybinding(
        &mut KeybindingCtx {
            cloud,
            frame,
            charset_preset,
            scene_name,
            scene_generation,
            user_ranges: &user_ranges,
            def_ascii: false,
            cfg: &cfg,
            term_reinit: &term_reinit,
            cfg_map,
        },
        key_event,
    )
}

// ── The owner's exact scenario ──────────────────────────────────

#[test]
fn r_returns_flagship_scene_builtin_after_shortkey_mutations() {
    let mut cloud = flagship_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let mut charset_preset = String::from("binary");
    let mut scene_name = String::from("sorgonemous_intrascals");
    let mut scene_generation: u64 = 0;

    // Snapshot the pristine builtin state.
    let builtin_scheme = cloud.color_scheme();
    let builtin_speed = cloud.chars_per_sec;
    let builtin_density = cloud.droplet_density();

    // ── the user mutates everything 'r' must wipe ──
    // color: cycle away from energy-zen (the owner's "change color
    // green or other using shortkey c").
    for _ in 0..7 {
        let _ = dispatch(
            &mut cloud,
            &mut frame,
            &key('c'),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            None,
        );
    }
    // charset: cycle away from binary.
    for _ in 0..3 {
        let _ = dispatch(
            &mut cloud,
            &mut frame,
            &key('s'),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            None,
        );
    }
    // speed: pump Up.
    for _ in 0..10 {
        let _ = dispatch(
            &mut cloud,
            &mut frame,
            &KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            None,
        );
    }
    // density: pump ']'.
    for _ in 0..5 {
        let _ = dispatch(
            &mut cloud,
            &mut frame,
            &key(']'),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            None,
        );
    }

    // The mutations actually landed.
    assert_ne!(
        cloud.color_scheme(),
        builtin_scheme,
        "precondition: color was mutated"
    );
    assert_ne!(
        cloud.chars_per_sec, builtin_speed,
        "precondition: speed was mutated"
    );
    assert_ne!(
        cloud.droplet_density(),
        builtin_density,
        "precondition: density was mutated"
    );
    assert_ne!(
        charset_preset, "binary",
        "precondition: charset was mutated"
    );

    // ── 'r': the full fresh ──
    let outcome = dispatch(
        &mut cloud,
        &mut frame,
        &key('r'),
        &mut charset_preset,
        &mut scene_name,
        &mut scene_generation,
        None,
    );

    // Outcome contract: FreshScene (wakes renderer + fps intent).
    assert!(
        matches!(outcome, KeyOutcome::FreshScene),
        "'r' must return FreshScene, got {outcome:?}"
    );
    // The scene's builtin values are back.
    assert_eq!(
        cloud.color_scheme(),
        builtin_scheme,
        "'r' must restore the scene's builtin color (owner: energy-zen)"
    );
    assert_eq!(
        charset_preset, "binary",
        "'r' must restore the scene's builtin charset"
    );
    assert_eq!(
        cloud.chars_per_sec, builtin_speed,
        "'r' must restore the scene's builtin speed (owner: 12)"
    );
    assert_eq!(
        cloud.droplet_density(),
        builtin_density,
        "'r' must restore the scene's builtin density (owner: 0.55)"
    );
    // The rain style stays the scene's.
    assert_eq!(cloud.active_scene(), "sorgonemous_intrascals");
    // The scene-family generation bumped (self-healer reset parity
    // with 'x').
    assert_eq!(scene_generation, 1, "'r' must bump scene_generation");
}

#[test]
fn r_replays_the_rain_from_zero() {
    // The from-zero replay is a GLYPH-family property (the droplet
    // pool): use the matrix scene, whose builtin style is Glyph.
    // Structured families (like the flagship's BlackHole) own their
    // mote pools and are covered by their own restart tests.
    let cfg = hunter27_config();
    let mut cloud = cfg.create_cloud(0.65);
    let charset = cloud.apply_scene_runtime(
        "matrix",
        &cfg.charset_preset.clone(),
        &cfg.user_ranges,
        cfg.def_ascii,
    );
    cloud.set_scene_label("matrix");

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let mut charset_preset = charset;
    let mut scene_name = String::from("matrix");
    let mut scene_generation: u64 = 0;

    // Let the rain run for a while (spawn + advance).
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(200) {
        cloud.rain_at(&mut frame, std::time::Instant::now());
    }
    let alive_before = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert!(alive_before > 0, "precondition: rain is running");

    let _ = dispatch(
        &mut cloud,
        &mut frame,
        &key('r'),
        &mut charset_preset,
        &mut scene_name,
        &mut scene_generation,
        None,
    );

    // From zero: the pool is vacant (fresh launch state — glyph rain
    // refills through natural spawn).
    let alive_after = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert_eq!(
        alive_after, 0,
        "'r' must replay from zero (NIGHT-lts-3 preserved), found {alive_after} alive"
    );
    // And the pause family is clean (fresh launch runs unpaused).
    assert!(!cloud.pause && cloud.pause_start.is_none());
    // The matrix scene's own builtin speed is restored too (the
    // from-zero + defaults contract composes).
    assert_eq!(
        cloud.chars_per_sec, 18.0,
        "'r' restores matrix's builtin speed 18"
    );
}

#[test]
fn r_does_not_change_the_scene() {
    let mut cloud = flagship_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let mut charset_preset = String::from("binary");
    let mut scene_name = String::from("sorgonemous_intrascals");
    let mut scene_generation: u64 = 0;

    let _ = dispatch(
        &mut cloud,
        &mut frame,
        &key('r'),
        &mut charset_preset,
        &mut scene_name,
        &mut scene_generation,
        None,
    );

    assert_eq!(
        scene_name, "sorgonemous_intrascals",
        "'r' is a restart of the CURRENT scene, not a cycle"
    );
    assert_eq!(cloud.active_scene(), "sorgonemous_intrascals");
}

#[test]
fn r_resets_ambient_override_and_drift_state() {
    let mut cloud = flagship_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let mut charset_preset = String::from("binary");
    let mut scene_name = String::from("sorgonemous_intrascals");
    let mut scene_generation: u64 = 0;

    // Simulate a stale drift + palette lock (mid-session Crystal
    // Dragon state) — 'r' must retire both (fresh-launch parity).
    cloud.ambient_palette_locked = true;
    cloud.drift_active = true;
    cloud.drift_start = Some(std::time::Instant::now());

    let _ = dispatch(
        &mut cloud,
        &mut frame,
        &key('r'),
        &mut charset_preset,
        &mut scene_name,
        &mut scene_generation,
        None,
    );

    // Mirrors the 'x' ownership contract + the snapback's drift
    // clearing.
    assert!(
        cloud.user_override_since_ambient,
        "'r' marks the user as owner (snapback timer re-arms)"
    );
    assert!(!cloud.ambient_palette_locked, "'r' clears the palette lock");
    assert!(!cloud.drift_active, "'r' retires mid-flight drift");
    assert!(cloud.drift_start.is_none(), "'r' clears drift_start");
    assert!(
        cloud.crystal_dragon_last_poll.is_some(),
        "'r' re-arms the drift poll cycle"
    );
}

#[test]
fn r_is_ignored_while_paused() {
    let mut cloud = flagship_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let mut charset_preset = String::from("binary");
    let mut scene_name = String::from("sorgonemous_intrascals");
    let mut scene_generation: u64 = 0;

    // Enter the deceleration window (pause pressed once).
    let _ = dispatch(
        &mut cloud,
        &mut frame,
        &key('p'),
        &mut charset_preset,
        &mut scene_name,
        &mut scene_generation,
        None,
    );
    // Mutate state so we can prove 'r' did NOT run.
    let scheme_before = cloud.color_scheme();

    let outcome = dispatch(
        &mut cloud,
        &mut frame,
        &key('r'),
        &mut charset_preset,
        &mut scene_name,
        &mut scene_generation,
        None,
    );

    assert!(
        matches!(outcome, KeyOutcome::None),
        "'r' during pause/decel must be suppressed, got {outcome:?}"
    );
    assert_eq!(
        scene_generation, 0,
        "'r' during pause must not bump generation"
    );
    assert_eq!(
        cloud.color_scheme(),
        scheme_before,
        "'r' during pause is a no-op"
    );
}

#[test]
fn r_on_custom_scene_reapplies_block_fields() {
    // A custom scene block in the config map: the 'r' reset must
    // resolve it through apply_scene_runtime_with_cfg (the same map
    // the live-reload rebuild uses) and re-apply its field layer.
    let mut cfg_map = std::collections::HashMap::new();
    cfg_map.insert(
        "scene-custom.night_hunt.rain".to_string(),
        "black_hole".to_string(),
    );
    cfg_map.insert(
        "scene-custom.night_hunt.color".to_string(),
        "green".to_string(),
    );
    cfg_map.insert(
        "scene-custom.night_hunt.charset".to_string(),
        "runic".to_string(),
    );
    cfg_map.insert("scene-custom.night_hunt.fps".to_string(), "60".to_string());
    cfg_map.insert(
        "scene-custom.night_hunt.speed".to_string(),
        "15".to_string(),
    );
    cfg_map.insert(
        "scene-custom.night_hunt.density".to_string(),
        "0.4".to_string(),
    );
    cfg_map.insert(
        "scene-custom.night_hunt.glitch-level".to_string(),
        "none".to_string(),
    );

    let cfg = hunter27_config();
    let mut cloud = cfg.create_cloud(0.4);
    // Apply the custom scene once (the runtime ambient path).
    let charset = cloud.apply_scene_runtime_with_cfg(
        "night_hunt",
        &cfg.charset_preset.clone(),
        &cfg.user_ranges,
        cfg.def_ascii,
        &cfg_map,
    );
    cloud.set_scene_label("night_hunt");

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let mut charset_preset = charset;
    let mut scene_name = String::from("night_hunt");
    let mut scene_generation: u64 = 0;

    // User mutates speed away from the block's 15.
    for _ in 0..30 {
        let _ = dispatch(
            &mut cloud,
            &mut frame,
            &KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            Some(&cfg_map),
        );
    }
    assert_ne!(
        cloud.chars_per_sec, 15.0,
        "precondition: speed mutated away"
    );

    let _ = dispatch(
        &mut cloud,
        &mut frame,
        &key('r'),
        &mut charset_preset,
        &mut scene_name,
        &mut scene_generation,
        Some(&cfg_map),
    );

    assert_eq!(
        cloud.chars_per_sec, 15.0,
        "'r' on a custom scene must re-apply the block's speed"
    );
    assert_eq!(
        charset_preset, "runic",
        "'r' restores the custom scene's charset"
    );
}

#[test]
fn other_keys_still_return_none_wake_contract() {
    // The KeyOutcome migration must preserve the legacy wake contract:
    // only 'p' (pause toggle) and 'r' (fresh scene) wake the renderer
    // immediately; c/s/x/[/]/Up/Down return None (absorbed by the next
    // scheduled frame — the pre-NIGHT-hunter-27 behavior).
    let mut cloud = flagship_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let mut charset_preset = String::from("binary");
    let mut scene_name = String::from("sorgonemous_intrascals");
    let mut scene_generation: u64 = 0;

    for k in ['c', 's', 'x', '[', ']'] {
        let outcome = dispatch(
            &mut cloud,
            &mut frame,
            &key(k),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            None,
        );
        assert!(
            matches!(outcome, KeyOutcome::None),
            "'{k}' must keep the None (no-wake) contract, got {outcome:?}"
        );
    }
    for k in [KeyCode::Up, KeyCode::Down] {
        let outcome = dispatch(
            &mut cloud,
            &mut frame,
            &KeyEvent::new(k, KeyModifiers::NONE),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            None,
        );
        assert!(
            matches!(outcome, KeyOutcome::None),
            "'{k:?}' must keep the None (no-wake) contract, got {outcome:?}"
        );
    }
}
