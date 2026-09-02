// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-beta.1 Z-master-1B regression tests: intro brand color + pause isolation.
//!
//! Covers two owner bug reports from 2026-08-30:
//!
//! 1. **Intro logo color override** — `cosmostrix -c neon-green` repainted
//!    the intro logo neon-green. Root cause: the unset-`intro_color` path
//!    passed the LIVE rain cloud to the intro, and `logo_stage_colors()`
//!    samples the cloud's palette stops. Fix: unset/invalid paths build a
//!    brand EnergyZen intro cloud (`event_loop_intro::brand_intro_cloud`)
//!    — `-c`/`--color`/`--colors-custom` never repaint the intro logo.
//!
//! 2. **Pause shortkey isolation** — while paused, `i` still toggled the
//!    HUD because `i` is dispatched in the event loop BEFORE
//!    `handle_keybinding()`, so the pause guard inside it never saw the
//!    key. Fix: `input::hud_toggle_accepted()` applies the same predicate
//!    the guard uses (`is_paused_or_decelerating`).

#[cfg(test)]
mod cases_v51_intro_brand_pause {
    use crate::cloud::Cloud;
    use crate::palette::build_palette;
    use crate::runtime::{ColorMode, ColorScheme};

    use crate::interactive::event_loop_intro::brand_intro_cloud;
    use crate::interactive::input::hud_toggle_accepted;
    use crate::CloudConfig;

    /// Duplicated from `tests.rs::mod cases::make_test_config()` (test fixture, stable).
    fn make_test_config() -> CloudConfig {
        CloudConfig {
            color_mode: ColorMode::TrueColor,
            shading_mode: crate::runtime::ShadingMode::Random,
            bold_mode: crate::runtime::BoldMode::Off,
            async_mode: false,
            default_bg: true,
            // The user's explicit choice — exactly the owner bug scenario
            // (`cosmostrix -c neon-green`).
            color_scheme: ColorScheme::NeonGreen,
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
            scene_name: "monolith".to_string(),
            scene_custom_name: None,
            // v80.0.0-beta.2 (S-master-HUNT): lock default — see CloudConfig doc.
            scene_custom_config_owned: false,
            cli_explicit: crate::app::CliExplicit::default(),
            ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
            ambient_snapback_secs: None,
        }
    }

    // ── Bug 1: intro brand color ────────────────────────────────────────

    /// The brand intro cloud must use the EnergyZen scheme even when the
    /// config (from `-c neon-green`) says NeonGreen — the intro logo is a
    /// brand mark and only `--intro-color` may repaint it.
    #[test]
    fn brand_intro_cloud_ignores_user_scheme() {
        let cfg = make_test_config();
        assert_eq!(cfg.color_scheme, ColorScheme::NeonGreen); // fixture sanity

        let intro_cloud = brand_intro_cloud(&cfg, 0.8);
        assert_eq!(
            intro_cloud.color_scheme,
            ColorScheme::EnergyZen,
            "intro cloud must be the brand EnergyZen scheme, not the rain scheme"
        );

        // The palette itself must differ from the NeonGreen palette the
        // logo gradient would otherwise have sampled (the owner bug).
        let neon = build_palette(ColorScheme::NeonGreen, ColorMode::TrueColor, true);
        assert_ne!(
            intro_cloud.palette.colors, neon.colors,
            "intro palette must not be the -c neon-green palette"
        );
    }

    /// A `--colors-custom` palette in the config must not leak into the
    /// intro either: `set_color_scheme` clears `custom_palette_active`.
    #[test]
    fn brand_intro_cloud_clears_custom_palette_flag() {
        let mut cfg = make_test_config();
        cfg.custom_palette = Some(build_palette(
            ColorScheme::FancyDiamond,
            ColorMode::TrueColor,
            true,
        ));
        cfg.custom_palette_name = Some("diamondish".to_string());

        let intro_cloud = brand_intro_cloud(&cfg, 0.8);
        assert!(
            !intro_cloud.custom_palette_active,
            "custom palette must be cleared on the brand intro cloud"
        );
        assert_eq!(intro_cloud.color_scheme, ColorScheme::EnergyZen);
    }

    /// The brand cloud keeps the user's charset — the intro's dissolve
    /// rain glyphs come from the configured pool, matching the
    /// `--intro-color` paths (which also build from `cfg.create_cloud`).
    /// `char_pool` itself is a randomly-sampled buffer, so assert on the
    /// source charset (`chars`) + that every pool glyph comes from it.
    #[test]
    fn brand_intro_cloud_keeps_charset() {
        let cfg = make_test_config();
        let intro_cloud = brand_intro_cloud(&cfg, 0.8);
        assert_eq!(intro_cloud.chars, vec!['0', '1']);
        assert!(
            intro_cloud.char_pool.iter().all(|&c| c == '0' || c == '1'),
            "intro rain glyphs must come from the configured charset"
        );
        assert!(!intro_cloud.char_pool.is_empty());
    }

    // ── Bug 2: pause shortkey isolation ─────────────────────────────────

    /// Duplicated from `tests.rs::mod cases::make_test_cloud()` (test fixture, stable).
    fn make_test_cloud() -> Cloud {
        let mut cloud = Cloud::new(
            ColorMode::Mono,
            crate::runtime::ShadingMode::Random,
            crate::runtime::BoldMode::Off,
            false,
            true,
            ColorScheme::Green,
            crate::rain_style::RainStyle::Glyph,
        );
        cloud.init_chars(vec!['0', '1']);
        cloud.reset(20, 10);
        cloud.clear_redraw_flags_for_test();
        cloud
    }

    /// While running, `i` is accepted (HUD toggles).
    #[test]
    fn hud_toggle_accepted_while_running() {
        let cloud = make_test_cloud();
        assert!(hud_toggle_accepted(&cloud));
    }

    /// While decelerating toward pause (first `p` press), `i` must already
    /// be rejected — the freeze window opens at the keypress, matching the
    /// `is_paused_or_decelerating` predicate used by the keybinding guard.
    #[test]
    fn hud_toggle_rejected_during_deceleration() {
        let mut cloud = make_test_cloud();
        assert!(cloud.toggle_pause()); // BRANCH 3: starts deceleration
        assert!(
            !hud_toggle_accepted(&cloud),
            "'i' must be rejected from the moment pause begins decelerating"
        );
    }

    /// While fully paused, `i` must be rejected (owner bug: it still
    /// toggled the HUD because it bypassed the handle_keybinding guard).
    /// Reaches the fully-paused state the same way
    /// `cloud::tests::is_paused_or_decelerating_catches_both_states`
    /// does (direct field set — the deceleration easing is time-based
    /// and would need a ~2.5 s sleep to settle naturally).
    #[test]
    fn hud_toggle_rejected_while_paused() {
        let mut cloud = make_test_cloud();
        cloud.pause = true;
        cloud.pause_start = None;
        cloud.pause_time = Some(std::time::Instant::now());
        assert!(cloud.pause, "fixture sanity: cloud must be fully paused");
        assert!(
            !hud_toggle_accepted(&cloud),
            "'i' must be rejected while paused (only 'p' and 'q' respond)"
        );
    }
}
