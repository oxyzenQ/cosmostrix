// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-perf-2: tests for the --bench-cosmetics harness — the
//! dedicated measurement of the render paths the Z-6 bench-mode
//! contract skips (message overlay + per-frame HUD block).
//!
//! Coverage contract:
//! - the clap `requires` wiring (the flag is meaningless without
//!   --benchmark and must fail fast at parse time, not warn later);
//! - the harness actually measures what it claims (mode + message
//!   active + hud timings populated + hud_frames == total_frames);
//! - the zero-alloc tripwire: the cosmetics path must stay under
//!   1 alloc/frame — the visible_border_scratch Z-5 fix (NIGHT-perf-2)
//!   removed the last per-frame Vec allocation (measured 1.0006
//!   allocs/frame before, 0.0006 after in the dev profile);
//! - non-cosmetics runs keep the new report fields idle (default
//!   bench behavior is byte-for-byte unchanged — Z-6 untouched);
//! - the JSON schema carries the new component_timing fields in
//!   both modes (uniform schema per the stability contract).

use std::sync::Mutex;

use clap::Parser;

use super::*;

/// Env-mutating tests serialize on this lock (the safepath/tests.rs
/// convention): COSMOSTRIX_BENCH_WARMUP_SECS is process-global, and
/// the harness reads it inside bench_warmup_secs().
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Minimal bench-shaped CloudConfig. Same literal shape as the
/// interactive tests' make_test_config, tuned for the harness:
/// 1-second duration, fixed 120x40 size, cinematic scene.
fn make_bench_config(cosmetics: bool) -> CloudConfig {
    CloudConfig {
        color_mode: crate::runtime::ColorMode::Mono,
        shading_mode: crate::runtime::ShadingMode::Random,
        bold_mode: crate::runtime::BoldMode::Off,
        async_mode: false,
        default_bg: true,
        color_scheme: crate::runtime::ColorScheme::Green,
        custom_palette: None,
        custom_palette_name: None,
        rain_style: crate::rain_style::RainStyle::Glyph,
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
        density: 0.8,
        speed: 8.0,
        monolith_size: crate::runtime::MonolithSize::Normal,
        chars: vec!['0', '1'],
        message: None,
        message_border: true,
        msg_fill_style: crate::msg_fill_style::MsgFillStyle::Engrave,
        target_fps: 60.0,
        xtermjs_host: false,
        default_fps_cap: 240.0,
        duration_s: None,
        bench_frames: None,
        benchmark: true,
        bench_duration: Some(1),
        save_baseline: None,
        compare_baseline: None,
        bench_io: false,
        bench_all: false,
        bench_scene: None,
        bench_cosmetics: cosmetics,
        screen_size: Some((120, 40)),
        color_tune: crate::color_tune::ColorTune::IDENTITY,
        json: false,
        verbose: false,
        density_auto: false,
        base_density: 0.8,
        perf_stats: false,
        screensaver: false,
        intro: crate::intro_style::IntroType::None,
        intro_color: None,
        mouse: false,
        charset_preset: String::from("binary"),
        user_ranges: vec![],
        def_ascii: true,
        crystal_dragon: false,
        power_dragon: true,
        msg_mode: true,
        effects_enabled: true,
        config_path_for_watcher: None,
        scene_name: "cinematic".to_string(),
        scene_custom_name: None,
        scene_custom_config_owned: false,
        cli_explicit: crate::app::CliExplicit::default(),
        ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
        ambient_snapback_secs: None,
        crystal_dragon_secs: None,
    }
}

#[test]
fn bench_cosmetics_requires_benchmark_at_parse_time() {
    // clap `requires = "benchmark"`: the flag alone must fail fast with
    // the missing-argument error naming --benchmark, never reach the
    // warn machinery or the interactive loop.
    let err = crate::config::Args::try_parse_from(["cosmostrix", "--bench-cosmetics"])
        .expect_err("--bench-cosmetics alone must not parse");
    let msg = format!("{err}");
    assert!(
        msg.contains("--benchmark"),
        "error must name the required flag: {msg}"
    );

    // With --benchmark it parses and the flag survives.
    let args =
        crate::config::Args::try_parse_from(["cosmostrix", "--benchmark", "--bench-cosmetics"])
            .expect("--benchmark --bench-cosmetics must parse");
    assert!(args.bench_cosmetics);
    assert!(args.benchmark);
}

#[test]
fn cosmetics_harness_measures_overlay_and_hud() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let previous = std::env::var("COSMOSTRIX_BENCH_WARMUP_SECS").ok();
    std::env::set_var("COSMOSTRIX_BENCH_WARMUP_SECS", "0");

    let cfg = make_bench_config(true);
    let data = run_premium_benchmark_silent(&cfg).expect("cosmetics bench must run");

    if let Some(prev) = previous {
        std::env::set_var("COSMOSTRIX_BENCH_WARMUP_SECS", prev);
    } else {
        std::env::remove_var("COSMOSTRIX_BENCH_WARMUP_SECS");
    }

    // The harness reports itself: mode on, message overlay active.
    assert!(data.cosmetics.mode, "cosmetics.mode must be true");
    assert!(
        data.cosmetics.message_active,
        "the default message must be active (start time rewound past \
         the intro lead + reveal)"
    );

    // HUD work was measured for every frame.
    assert!(
        data.cosmetics.hud_frames == data.total_frames,
        "hud_frames ({}) must equal total_frames ({})",
        data.cosmetics.hud_frames,
        data.total_frames
    );
    assert!(
        data.cosmetics.hud_avg_ms > 0.0,
        "hud_avg_ms must be positive (two Instant::now calls per frame \
         alone guarantee nonzero)"
    );
    assert!(
        data.cosmetics.hud_max_ms >= data.cosmetics.hud_avg_ms,
        "hud_max_ms must dominate hud_avg_ms"
    );

    // The zero-alloc tripwire (Z-5 contract, now actually measurable):
    // pre-fix the BN-01/02 visible-border Vec allocated 1.0x per frame;
    // the 1 Hz HUD metric tick's transient format! allocs amortize to
    // well under 1 per frame on any plausible frame rate.
    let allocs_per_frame = data
        .allocator
        .as_ref()
        .map(|m| m.alloc_calls_per_frame)
        .unwrap_or(0.0);
    assert!(
        allocs_per_frame < 1.0,
        "cosmetics path must stay zero-alloc per frame (measured \
         {allocs_per_frame:.4} allocs/frame; > 1.0 means a per-frame \
         heap allocation returned to draw_message or the HUD block)"
    );

    // The JSON schema carries the harness fields (uniform schema:
    // same keys in both modes, per the stability contract).
    let json = crate::bench_json::build_json_string(&data);
    assert!(json.contains("\"hud_avg_ms\""));
    assert!(json.contains("\"hud_max_ms\""));
    assert!(json.contains("\"hud_frames\""));
    assert!(json.contains("\"cosmetics_mode\":true"));
    assert!(json.contains("\"message_active\":true"));
}

#[test]
fn plain_bench_keeps_cosmetics_fields_idle() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let previous = std::env::var("COSMOSTRIX_BENCH_WARMUP_SECS").ok();
    std::env::set_var("COSMOSTRIX_BENCH_WARMUP_SECS", "0");

    let cfg = make_bench_config(false);
    let data = run_premium_benchmark_silent(&cfg).expect("plain bench must run");

    if let Some(prev) = previous {
        std::env::set_var("COSMOSTRIX_BENCH_WARMUP_SECS", prev);
    } else {
        std::env::remove_var("COSMOSTRIX_BENCH_WARMUP_SECS");
    }

    // Default Z-6 behavior is untouched: no cosmetics measurement, no
    // HUD work, no message overlay, idle report fields.
    assert!(!data.cosmetics.mode);
    assert!(!data.cosmetics.message_active);
    assert_eq!(data.cosmetics.hud_avg_ms, 0.0);
    assert_eq!(data.cosmetics.hud_max_ms, 0.0);
    assert_eq!(data.cosmetics.hud_frames, 0);

    let json = crate::bench_json::build_json_string(&data);
    assert!(json.contains("\"cosmetics_mode\":false"));
    assert!(json.contains("\"message_active\":false"));
}
