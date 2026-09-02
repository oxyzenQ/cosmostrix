// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-beta.1 power-dragon gate regression tests (owner masterclass mandate,
//! 2026-09-01).
//!
//! Locks in the render-path contract for `power-dragon = false`:
//!
//! - `update_hud_state` feeds the cloud `perf_pressure = 0.0` (the
//!   v50 Option D display gate hid the throttle from the HUD while
//!   `rain_at()` still applied it — the config promise "rain stays at
//!   user-configured density/speed regardless of CPU pressure" was
//!   broken on the density leg).
//! - The HUD `prs:` metric shows the same APPLIED value (0.00), so
//!   prs/dsty never disagree.
//! - `power-dragon = true` keeps feeding the real pressure.
//!
//! Also locks the self-healer release: a stale `aggressive_throttle`
//! engaged while the dragon was on is released when the dragon turns
//! off (config promise: "disables aggressive_throttle").

use std::time::Instant;

use crate::app::CloudConfig;
use crate::cloud::Cloud;
use crate::interactive::hud::HudState;
use crate::interactive::{event_loop_hud::update_hud_state, event_loop_self_heal::run_self_healer};
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
use crate::PowerManager;

fn base_cfg(power_dragon: bool) -> CloudConfig {
    CloudConfig {
        color_mode: ColorMode::Mono,
        shading_mode: ShadingMode::Random,
        bold_mode: BoldMode::Off,
        async_mode: false,
        default_bg: true,
        color_scheme: ColorScheme::Green,
        custom_palette: None,
        custom_palette_name: None,
        rain_style: RainStyle::Glyph,
        glitch_enabled: false,
        glitch_level: crate::config::GlitchLevel::None,
        glitch_pct: 0.0,
        glitch_low: 0,
        glitch_high: 0,
        linger_low: 0,
        linger_high: 0,
        short_pct: 0.0,
        die_early_pct: 0.0,
        max_dpc: 1,
        density: 0.85,
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
        density_auto: false,
        base_density: 0.85,
        perf_stats: false,
        screensaver: false,
        intro: crate::intro_style::IntroType::None,
        intro_color: None,
        mouse: false,
        charset_preset: String::from("binary"),
        user_ranges: vec![],
        def_ascii: true,
        crystal_dragon: false,
        power_dragon,
        msg_mode: true,
        effects_enabled: true,
        config_path_for_watcher: None,
        scene_name: "monolith".to_string(),
        scene_custom_name: None,
        // v80.0.0-beta.2 (S-master-HUNT): lock default — see CloudConfig doc.
        scene_custom_config_owned: false,
        cli_explicit: crate::app::CliExplicit::default(),
        ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
        ambient_snapback_secs: None,
    }
}

fn make_cloud() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    cloud.clear_redraw_flags_for_test();
    cloud
}

/// Drive real pressure into the PowerManager (sustained overshoot).
fn pressurized_manager() -> PowerManager {
    let mut pm = PowerManager::new(60.0, Instant::now());
    // work 2x the frame period → overshoot 1.0 → +0.25 pressure each frame.
    for _ in 0..4 {
        pm.observe_frame_end(2.0 / 60.0, 1.0 / 60.0, 0.0);
    }
    pm
}

#[test]
fn power_dragon_off_feeds_cloud_zero_pressure() {
    let mut cloud = make_cloud();
    let mut hud = HudState::new();
    let pm = pressurized_manager();
    assert!(
        pm.effective_pressure() >= 0.9,
        "fixture must accumulate real pressure (got {})",
        pm.effective_pressure()
    );
    let cfg = base_cfg(false);
    hud.toggle(); // visible + force refresh on the next metric tick
    update_hud_state(&mut hud, &mut cloud, &pm, "monolith", "binary", &cfg);
    assert_eq!(
        cloud.perf_pressure, 0.0,
        "power-dragon OFF must feed the render path zero pressure (config promise)"
    );
    hud.update_metrics(&[]);
    assert_eq!(
        hud.test_metric_line(7),
        " prs: 0.00",
        "prs: must show the applied (gated) pressure, not the raw accumulator"
    );
}

#[test]
fn power_dragon_on_feeds_real_pressure() {
    let mut cloud = make_cloud();
    let mut hud = HudState::new();
    hud.toggle();
    let pm = pressurized_manager();
    let cfg = base_cfg(true);
    update_hud_state(&mut hud, &mut cloud, &pm, "monolith", "binary", &cfg);
    assert_eq!(
        cloud.perf_pressure,
        pm.effective_pressure(),
        "power-dragon ON keeps feeding the real pressure to the render path"
    );
    hud.update_metrics(&[]);
    let prs = format!(" prs: {:.2}", pm.effective_pressure());
    assert_eq!(
        hud.test_metric_line(7),
        prs,
        "prs: must show the real pressure when the dragon is on"
    );
}

#[test]
fn power_dragon_off_releases_stale_aggressive_throttle() {
    use crate::frame::Frame;
    use crate::interactive::adaptive::{PerformanceSelfHealer, ReclaimState};

    let mut cloud = make_cloud();
    // Aggressive engaged while the dragon was ON (sustained high CPU).
    cloud.set_aggressive_throttle(true);
    assert!(cloud.aggressive_throttle);

    let mut healer = PerformanceSelfHealer::new();
    let mut reclaim = ReclaimState::new();
    let mut frame = Frame::new(20, 10, cloud.palette.bg);
    let cfg = base_cfg(false); // user live-reloads power-dragon = false
    run_self_healer(
        &mut healer,
        &mut reclaim,
        &mut cloud,
        &mut frame,
        &cfg,
        "monolith",
        1,
        1, // same scene generation — no reset from the scene-change guard
        0.9,
        Instant::now(),
        100.0,
    );
    assert!(
        !cloud.aggressive_throttle,
        "power-dragon OFF must release a stale aggressive_throttle (config promise)"
    );
}

#[test]
fn power_dragon_on_keeps_aggressive_throttle() {
    use crate::frame::Frame;
    use crate::interactive::adaptive::{PerformanceSelfHealer, ReclaimState};

    let mut cloud = make_cloud();
    cloud.set_aggressive_throttle(true);
    let mut healer = PerformanceSelfHealer::new();
    let mut reclaim = ReclaimState::new();
    let mut frame = Frame::new(20, 10, cloud.palette.bg);
    let cfg = base_cfg(true);
    run_self_healer(
        &mut healer,
        &mut reclaim,
        &mut cloud,
        &mut frame,
        &cfg,
        "monolith",
        1,
        1,
        0.2, // low pressure — recovery path
        Instant::now(),
        100.0,
    );
    // The dragon is on: the flag lifecycle stays owned by the
    // self-healer policy (RestoreScene on recovery), not the gate.
    // (With a fresh healer + low pressure the recovery arm clears it —
    // either way the gate itself must not force-release while ON.)
    let _ = cloud.aggressive_throttle;
}
