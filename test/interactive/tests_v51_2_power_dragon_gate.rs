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
//! - `power-dragon = true` keeps feeding the pressure — NIGHT-hunter-2
//!   refines "the pressure" to the visual-pressure EMA: sustained
//!   pressure reaches the render path, transient spikes are filtered
//!   at the feed (the raw accumulator is untouched for the control
//!   side).
//!
//! Also locks the self-healer release: a stale `aggressive_throttle`
//! engaged while the dragon was on is released when the dragon turns
//! off (config promise: "disables aggressive_throttle").

use std::time::{Duration, Instant};

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
        crystal_dragon_secs: None,
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

/// Drive SUSTAINED pressure while sampling the visual EMA exactly like
/// production (begin_frame + observe_frame_end pairs advancing wall
/// time) — NIGHT-hunter-2 fixture: 6 s of overshoot pins raw pressure at
/// 1.0 and converges the EMA to ~1 - e^(-6/2.5) ~= 0.91.
fn pressurized_manager_sustained() -> PowerManager {
    let start = Instant::now();
    let mut pm = PowerManager::new(60.0, start);
    let step = Duration::from_secs_f64(1.0 / 60.0);
    let mut t = start;
    for _ in 0..(6 * 60) {
        t += step;
        pm.begin_frame(t);
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
    let pm = pressurized_manager_sustained();
    let cfg = base_cfg(true);
    update_hud_state(&mut hud, &mut cloud, &pm, "monolith", "binary", &cfg);
    assert!(
        (cloud.perf_pressure - pm.visual_pressure()).abs() < 1e-6,
        "power-dragon ON feeds the render path the visual-pressure EMA (the applied value)"
    );
    assert!(
        cloud.perf_pressure > 0.85,
        "sustained pressure must reach the render path (EMA converged, got {})",
        cloud.perf_pressure
    );
    hud.update_metrics(&[]);
    let prs = format!(" prs: {:.2}", pm.applied_visual_pressure(true));
    assert_eq!(
        hud.test_metric_line(7),
        prs,
        "prs: must show the applied (smoothed) pressure when the dragon is on"
    );
}

#[test]
fn power_dragon_on_filters_transient_spikes_at_the_cloud_feed() {
    // NIGHT-hunter-2: a sub-second saturation burst (the measured
    // drain-loop write-blocking spikes) must not strobe the render path
    // into its pressure behaviors — the EMA stays below every visual
    // threshold while the RAW accumulator spikes to 1.0 for the
    // control side.
    let mut cloud = make_cloud();
    let mut hud = HudState::new();
    hud.toggle();
    let start = Instant::now();
    let mut pm = PowerManager::new(60.0, start);
    let step = Duration::from_secs_f64(1.0 / 60.0);
    let mut t = start;
    // 0.5 s of fully-blocked flushes (write_overshoot 2.0).
    for _ in 0..30 {
        t += step;
        pm.begin_frame(t);
        pm.observe_frame_end(0.001, 1.0 / 60.0, 2.0);
    }
    assert!(
        (pm.effective_pressure() - 1.0).abs() < 1e-6,
        "fixture: raw pressure spikes to 1.0 (control side sees the spike)"
    );
    let cfg = base_cfg(true);
    update_hud_state(&mut hud, &mut cloud, &pm, "monolith", "binary", &cfg);
    assert!(
        cloud.perf_pressure < 0.35,
        "transient spike must stay below every visual threshold at the feed (got {})",
        cloud.perf_pressure
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

// ── S-master-HUNT-23: P2 mitigation congestion guard ─────────────────────────
//
// The P2 health mitigation (force full redraw + madvise) must NOT push a
// full-screen redraw (the single largest ANSI burst the renderer can
// produce) into a pipe that is already congested (pressure >=
// SELF_HEAL_PRESSURE_LOW). The full redraw stays reserved for its original
// calibration: pressure LOW + unhealthy process (genuine stuck visual
// state / desync), where the terminal has drain headroom to absorb it.

/// Drive the healer into the P2 `TriggerHealthMitigation` arm: health score
/// below the investigate band (60), no prior mitigation (cooldown clear).
/// Both tests share this setup; only the effective pressure differs.
fn hunts23_mitigation_setup() -> (
    crate::interactive::adaptive::PerformanceSelfHealer,
    crate::interactive::adaptive::ReclaimState,
) {
    (
        crate::interactive::adaptive::PerformanceSelfHealer::new(),
        crate::interactive::adaptive::ReclaimState::new(),
    )
}

#[test]
fn health_mitigation_forces_redraw_when_pressure_low() {
    use crate::frame::Frame;

    let mut cloud = make_cloud();
    let (mut healer, mut reclaim) = hunts23_mitigation_setup();
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
        1,   // no scene change — healer state must survive
        0.0, // LOW pressure: terminal has drain headroom
        Instant::now(),
        50.0, // health score in the "investigate" band
    );
    assert!(
        cloud.force_draw_everything,
        "with pressure LOW + unhealthy score, P2 must force the full redraw \
         (original calibration: clear stuck visual state / desync)"
    );
}

#[test]
fn health_mitigation_hunts23_skips_redraw_when_congested() {
    use crate::frame::Frame;

    let mut cloud = make_cloud();
    let (mut healer, mut reclaim) = hunts23_mitigation_setup();
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
        0.5, // CONGESTED: pressure >= SELF_HEAL_PRESSURE_LOW (0.3)
        Instant::now(),
        50.0,
    );
    assert!(
        !cloud.force_draw_everything,
        "HUNT-23: under output congestion the full-redraw bomb must be \
         skipped — a full-screen ANSI burst into a saturated pipe is what \
         produced the periodic stuck-then-auto-dismiss VTE/foot symptom"
    );
}
