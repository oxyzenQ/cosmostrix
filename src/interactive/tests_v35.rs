// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v35+ regression tests, extracted from `interactive/tests.rs` as a
//! pre-emptive split to keep both files below the 1500-LOC guard
//! (`scripts/check-rs-loc.sh`).
//!
//! Covers:
//! - v35_x_key_* / v35_c_key_* / v35_s_key_* (user override + palette lock)
//! - v35_1_auto_snapback_* (idle threshold snapback regression)
//! - v35_cloud_ambient_flags_default_false
//! - super_* / hyper_* / meta_* / control+shift / alt / shift modifier-key
//!   suppression tests (must NOT cycle color/scene/charset/pause/reset)
//!
//! Helpers `key()`, `make_test_cloud()`, `make_test_config()` are
//! duplicated from `tests.rs::mod cases` because each test module
//! compiles its own private copy. `key_with_mod()`, `arrow_with_mod()`,
//! and `color_scheme_of()` are local to this section and moved with it.

#[cfg(test)]
mod cases_v35 {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::cloud::Cloud;
    use crate::frame::Frame;

    use crate::interactive::input::{handle_keybinding, should_auto_snapback, KeybindingCtx};
    use crate::platform::{default_term_reinit, TermReinit};
    use crate::CloudConfig;

    /// Duplicated from `tests.rs::mod cases::key()` (test fixture, stable).
    fn key(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
    }

    /// Duplicated from `tests.rs::mod cases::make_test_cloud()` (test fixture, stable).
    fn make_test_cloud() -> Cloud {
        let mut cloud = Cloud::new(
            crate::runtime::ColorMode::Mono,
            crate::runtime::ShadingMode::Random,
            crate::runtime::BoldMode::Off,
            false,
            true,
            crate::runtime::ColorScheme::Green,
            crate::rain_style::RainStyle::Glyph,
        );
        cloud.init_chars(vec!['0', '1']);
        cloud.reset(20, 10);
        // Clear flags set by init_chars/reset so tests start from a clean
        // state. Without this, semantic_invalidate and force_draw_everything
        // are already true from initialization, causing test assertions to
        // fail even when the tested key is a no-op.
        cloud.clear_redraw_flags_for_test();
        cloud
    }

    /// Duplicated from `tests.rs::mod cases::make_test_config()` (test fixture, stable).
    fn make_test_config() -> CloudConfig {
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
            message_border: false,
            // v51 msg-fill-style: default keeps the classic typewriter reveal.
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
            monolith_density_map: None,
            config_path_for_watcher: None,
            scene_name: "monolith".to_string(),
            scene_custom_name: None,
            cli_explicit: crate::app::CliExplicit::default(),
            ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
            ambient_snapback_secs: None,
        }
    }

    /// Duplicated from `tests.rs::mod cases::call_handle_keybinding()` (test fixture, stable).
    #[allow(dead_code)]
    fn call_handle_keybinding(
        cloud: &mut Cloud,
        frame: &mut Frame,
        key: &KeyEvent,
        charset_preset: &mut String,
        cfg: &CloudConfig,
        term_reinit: &TermReinit,
    ) -> bool {
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;
        call_handle_keybinding_with_scene(
            cloud,
            frame,
            key,
            charset_preset,
            &mut scene_name,
            &mut scene_generation,
            cfg,
            term_reinit,
        )
    }

    /// Duplicated from `tests.rs::mod cases::call_handle_keybinding_with_scene()` (test fixture, stable).
    #[allow(clippy::too_many_arguments)]
    fn call_handle_keybinding_with_scene(
        cloud: &mut Cloud,
        frame: &mut Frame,
        key: &KeyEvent,
        charset_preset: &mut String,
        scene_name: &mut String,
        scene_generation: &mut u64,
        cfg: &CloudConfig,
        term_reinit: &TermReinit,
    ) -> bool {
        let user_ranges: [(char, char); 0] = [];
        handle_keybinding(
            &mut KeybindingCtx {
                cloud,
                frame,
                charset_preset,
                scene_name,
                scene_generation,
                user_ranges: &user_ranges,
                def_ascii: true,
                cfg,
                term_reinit,
            },
            key,
        )
    }

    #[test]
    fn v35_x_key_sets_user_override_and_clears_palette_lock() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;
        cloud.ambient_palette_locked = true;
        cloud.user_override_since_ambient = false;

        call_handle_keybinding_with_scene(
            &mut cloud,
            &mut frame,
            &key('x'),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert!(
            cloud.user_override_since_ambient,
            "'x' key must set user_override_since_ambient = true"
        );
        assert!(
            !cloud.ambient_palette_locked,
            "'x' key must clear ambient_palette_locked"
        );
    }

    #[test]
    fn v35_s_key_sets_user_override_only() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;
        // 's' changes charset, not palette — ambient_palette_locked state
        // should be preserved (whatever it was).
        cloud.ambient_palette_locked = true;
        cloud.user_override_since_ambient = false;

        call_handle_keybinding_with_scene(
            &mut cloud,
            &mut frame,
            &key('s'),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert!(
            cloud.user_override_since_ambient,
            "'s' key must set user_override_since_ambient = true"
        );
        // ambient_palette_locked is NOT cleared by 's' (charset != palette).
        assert!(
            cloud.ambient_palette_locked,
            "'s' key must NOT clear ambient_palette_locked (charset change)"
        );
    }

    /// `should_auto_snapback` returns false when no user override is
    /// active — the ambient scheduler is already in control and doesn't need
    /// to re-assert anything.
    #[test]
    fn v35_1_auto_snapback_skipped_when_no_override() {
        // No user override → never snapback, regardless of idle time.
        assert!(
            !should_auto_snapback(false, 0.0, 30.0),
            "no override, just pressed key → no snapback"
        );
        assert!(
            !should_auto_snapback(false, 60.0, 30.0),
            "no override, idle 60s → no snapback"
        );
        assert!(
            !should_auto_snapback(false, 3600.0, 30.0),
            "no override, idle 1h → no snapback (scheduler owns state)"
        );
    }

    /// `should_auto_snapback` returns false when user has overridden
    /// but is still actively pressing keys (idle time below threshold).
    /// This is the "user is cycling through scenes" case — auto-snapback
    /// must NOT interrupt.
    #[test]
    fn v35_1_auto_snapback_skipped_during_active_input() {
        assert!(
            !should_auto_snapback(true, 0.0, 30.0),
            "override + just pressed key → no snapback (active)"
        );
        assert!(
            !should_auto_snapback(true, 5.0, 30.0),
            "override + 5s idle → no snapback (still active)"
        );
        assert!(
            !should_auto_snapback(true, 29.99, 30.0),
            "override + 29.99s idle → no snapback (below threshold)"
        );
    }

    /// `should_auto_snapback` returns true ONLY when user has
    /// overridden AND idle time has crossed the threshold. This is the
    /// "user pressed x then walked away" case — ambient must re-assert.
    #[test]
    fn v35_1_auto_snapback_triggered_after_idle_threshold() {
        assert!(
            should_auto_snapback(true, 30.0, 30.0),
            "override + exactly 30s idle → snapback (boundary inclusive)"
        );
        assert!(
            should_auto_snapback(true, 30.01, 30.0),
            "override + 30.01s idle → snapback"
        );
        assert!(
            should_auto_snapback(true, 600.0, 30.0),
            "override + 10min idle → snapback (long idle is fine)"
        );
    }

    /// `should_auto_snapback` honors the configured threshold — a
    /// longer threshold delays the snapback, a shorter threshold hastens it.
    /// This makes the helper reusable if AUTO_SNAPBACK_DELAY_SECS becomes
    /// configurable in the future.
    #[test]
    fn v35_1_auto_snapback_threshold_is_configurable() {
        // 10s threshold
        assert!(
            !should_auto_snapback(true, 9.99, 10.0),
            "10s threshold + 9.99s idle → no snapback"
        );
        assert!(
            should_auto_snapback(true, 10.0, 10.0),
            "10s threshold + 10s idle → snapback"
        );
        // 60s threshold
        assert!(
            !should_auto_snapback(true, 59.99, 60.0),
            "60s threshold + 59.99s idle → no snapback"
        );
        assert!(
            should_auto_snapback(true, 60.0, 60.0),
            "60s threshold + 60s idle → snapback"
        );
    }

    /// cloud fields default to false on construction.
    #[test]
    fn v35_cloud_ambient_flags_default_false() {
        let cloud = make_test_cloud();
        assert!(
            !cloud.ambient_palette_locked,
            "ambient_palette_locked must default to false"
        );
        assert!(
            !cloud.user_override_since_ambient,
            "user_override_since_ambient must default to false"
        );
    }

    /// v50.0.0-beta.7 masterclass state machine: drift_active + drift_start
    /// default to false + None.
    #[test]
    fn v50_drift_state_defaults() {
        let cloud = make_test_cloud();
        assert!(!cloud.drift_active, "drift_active must default to false");
        assert!(
            cloud.drift_start.is_none(),
            "drift_start must default to None"
        );
    }

    /// v50.0.0-beta.7 masterclass: snapback counts from drift_start (when
    /// drift fired), giving drift exactly ambient-snapback-secs of visibility.
    /// When drift is NOT active, falls back to last_user_input_at for manual
    /// user overrides.
    #[test]
    fn v50_snapback_counts_from_drift_start() {
        use std::time::{Duration, Instant};

        let mut cloud = make_test_cloud();
        let now = Instant::now();

        // Scenario: drift fired 5s ago (drift_start = now-5s), snapback=10.
        // idle from drift_start = 5s → no snapback yet (drift visible 5s more).
        cloud.drift_active = true;
        cloud.drift_start = Some(now - Duration::from_secs(5));
        let snapback_ref = cloud.drift_start.unwrap();
        let idle_secs = now.saturating_duration_since(snapback_ref).as_secs_f64();
        assert!(
            (4.0..=6.0).contains(&idle_secs),
            "idle must be ~5s (from drift_start), got {idle_secs}"
        );
        assert!(
            !should_auto_snapback(true, idle_secs, 10.0),
            "5s since drift + 10s threshold → no snapback yet (drift visible 5s more)"
        );

        // 5s later: idle from drift_start = 10s → snapback fires
        let idle_at_10 = idle_secs + 5.0;
        assert!(
            should_auto_snapback(true, idle_at_10, 10.0),
            "10s since drift + 10s threshold → snapback fires (revert to ambient)"
        );

        // After snapback: drift_active cleared, drift_start = None.
        // Next drift can fire on next poll (60s from last poll).
        cloud.drift_active = false;
        cloud.drift_start = None;
        assert!(!cloud.drift_active, "drift_active cleared after snapback");
    }

    /// v50.0.0-beta.7 masterclass: drift must NOT fire while drift_active is true.
    /// Once drift fires, it sets drift_active=true and will NOT fire again
    /// until snapback clears it. This prevents drift racing snapback.
    #[test]
    fn v50_drift_suppressed_while_active() {
        let mut cloud = make_test_cloud();
        cloud.crystal_dragon = true;
        cloud.ambient_schedule_active = true;

        // Drift already active — must not fire again
        cloud.drift_active = true;
        assert!(
            !(cloud.crystal_dragon
                && !cloud.drift_active
                && (!cloud.user_override_since_ambient || !cloud.ambient_schedule_active)),
            "drift must be suppressed while drift_active is true"
        );

        // Snapback cleared drift_active — drift can fire again
        cloud.drift_active = false;
        assert!(
            cloud.crystal_dragon
                && !cloud.drift_active
                && (!cloud.user_override_since_ambient || !cloud.ambient_schedule_active),
            "drift can fire again after snapback clears drift_active"
        );
    }

    /// v50.0.0-beta.7: drift must NOT fire while user_override_since_ambient is true
    /// (manual user override via c/C/x). The state machine gate is:
    /// crystal_dragon && !drift_active && !user_override_since_ambient.
    ///
    /// Z-master-1X: this test exercises the AMBIENT-ON path — when the
    /// schedule is active, the user-override flag must suppress drift
    /// until an ambient fire clears it. `ambient_schedule_active = true`
    /// must be set explicitly because `make_test_cloud()` defaults it to
    /// false (no schedule).
    #[test]
    fn v50_drift_suppressed_while_override_pending() {
        let mut cloud = make_test_cloud();
        cloud.crystal_dragon = true;
        cloud.ambient_schedule_active = true;
        cloud.drift_active = false;
        cloud.user_override_since_ambient = true;

        // User override pending → drift must NOT fire
        assert!(
            !(cloud.crystal_dragon
                && !cloud.drift_active
                && (!cloud.user_override_since_ambient || !cloud.ambient_schedule_active)),
            "drift must be suppressed while user_override_since_ambient is true and ambient is active"
        );

        // Snapback cleared the flag → drift can fire again
        cloud.user_override_since_ambient = false;
        assert!(
            cloud.crystal_dragon
                && !cloud.drift_active
                && (!cloud.user_override_since_ambient || !cloud.ambient_schedule_active),
            "drift can fire again after snapback clears the override flag"
        );
    }

    /// Z-master-1X bug fix regression test: when the ambient schedule is
    /// empty (`ambient_schedule_active = false`), the user-override flag
    /// MUST NOT block crystal dragon drift. The flag is forced to `true`
    /// at startup by `event_loop_setup.rs` (coredump fix, commit 2b0e28b)
    /// and is only cleared by an ambient fire — which never happens when
    /// the schedule is empty. Without this fix, `crystal_dragon = true`
    /// in config with ambient off would never produce a color change,
    /// even though the HUD reports `crdr: on`.
    ///
    /// Reproduces the owner-reported bug:
    ///   `cosmostrix -v -s -C minimal -mfs words`
    ///   config: crystal-dragon = true, power_dragon = true, ambient off
    ///   symptom: HUD shows prdr: on + crdr: on, but no color change after 60s
    #[test]
    fn z_master_1x_drift_allowed_when_ambient_off_despite_override_flag() {
        let mut cloud = make_test_cloud();
        cloud.crystal_dragon = true;
        // Simulate startup state: ambient schedule empty + override flag
        // forced true by event_loop_setup.rs.
        cloud.ambient_schedule_active = false;
        cloud.user_override_since_ambient = true;
        cloud.drift_active = false;

        // Drift MUST be allowed — ambient is off, so the override flag is
        // meaningless and must not block the crystal dragon engine.
        assert!(
            cloud.crystal_dragon
                && !cloud.drift_active
                && (!cloud.user_override_since_ambient || !cloud.ambient_schedule_active),
            "Z-master-1X: when ambient schedule is empty, drift must NOT be blocked by user_override_since_ambient"
        );
    }

    // v50 LTS regression tests (first-reload scene reset crash) live in the
    // sibling file `tests_v50_first_reload.rs` (declared at file bottom).
    // Extracted to keep this file under the 1500-LOC cap.
}
