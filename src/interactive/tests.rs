// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

#[cfg(test)]
mod cases {
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    use std::sync::atomic::AtomicBool;
    #[cfg(unix)]
    use std::sync::Arc;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::cloud::Cloud;
    use crate::constants::*;
    use crate::frame::Frame;

    use crate::interactive::activity::{idle_resync_due, is_runtime_idle, register_activity};
    use crate::interactive::input::{handle_keybinding, runtime_speed_clamp, PasteBurstGuard};
    use crate::CloudConfig;

    fn key(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
    }

    #[test]
    fn idle_resync_uses_wall_clock_time() {
        let start = Instant::now();
        let due = start + Duration::from_secs_f64(IDLE_REDRAW_RESYNC_INTERVAL_SECS + 0.1);
        let early = start + Duration::from_secs_f64(IDLE_REDRAW_RESYNC_INTERVAL_SECS - 0.1);

        assert!(!idle_resync_due(true, start, early));
        assert!(idle_resync_due(true, start, due));
        assert!(!idle_resync_due(false, start, due));
    }

    #[test]
    fn idle_to_active_activity_schedules_resync() {
        let start = Instant::now();
        let activity_time = start + Duration::from_secs(60);
        let mut last_input_time = start;
        let mut last_resync_time = start;

        assert!(register_activity(
            &mut last_input_time,
            &mut last_resync_time,
            activity_time,
            true,
            false,
        ));
        assert_eq!(last_input_time, activity_time);
        assert_eq!(last_resync_time, activity_time);
    }

    #[test]
    fn active_mouse_activity_does_not_force_resync_every_frame() {
        let start = Instant::now();
        let activity_time = start + Duration::from_secs(1);
        let mut last_input_time = start;
        let mut last_resync_time = start;

        assert!(!register_activity(
            &mut last_input_time,
            &mut last_resync_time,
            activity_time,
            false,
            false,
        ));
        assert_eq!(last_input_time, activity_time);
        assert_eq!(last_resync_time, start);
    }

    #[test]
    fn focus_activity_can_force_resync_while_active() {
        let start = Instant::now();
        let activity_time = start + Duration::from_secs(1);
        let mut last_input_time = start;
        let mut last_resync_time = start;

        assert!(register_activity(
            &mut last_input_time,
            &mut last_resync_time,
            activity_time,
            false,
            true,
        ));
        assert_eq!(last_resync_time, activity_time);
    }

    #[test]
    fn idle_state_stays_idle_until_activity_resets_timer() {
        let start = Instant::now();
        let idle_now = start + Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 0.1);
        let later_idle_now = idle_now + Duration::from_secs(5);
        let active_now = start + Duration::from_secs(1);

        assert!(!is_runtime_idle(start, active_now));
        assert!(is_runtime_idle(start, idle_now));
        assert!(is_runtime_idle(start, later_idle_now));
    }

    #[test]
    fn plain_shortcut_key_is_not_ignored_without_burst() {
        let now = Instant::now();
        let mut guard = PasteBurstGuard::default();

        // No bracketed paste signal armed → plain keys must pass through.
        // This is the critical case for printable shortcuts like L (storm
        // mode), C (color cycle), S (charset), P (pause) on terminals that
        // emit Press+Release pairs — previously the queue-ready heuristic
        // would drop the Press because the Release was already queued.
        assert!(!guard.ignore_plain_key(&key('p'), now));
        assert!(!guard.ignore_plain_key(&key('l'), now));
        assert!(!guard.ignore_plain_key(&key('c'), now));
        assert!(!guard.ignore_plain_key(&key('s'), now));
    }

    #[test]
    fn paste_burst_ignores_shortcut_letters() {
        // Bracketed paste arms the suppression window; subsequent plain
        // keys within the window must be dropped so pasted text does not
        // trigger shortcuts like c/s/p.
        let now = Instant::now();
        let mut guard = PasteBurstGuard::default();

        guard.note_bracketed_paste(now);
        assert!(guard.ignore_plain_key(&key('p'), now + Duration::from_millis(1)));
        assert!(guard.ignore_plain_key(&key('c'), now + Duration::from_millis(2)));
        assert!(guard.ignore_plain_key(&key('s'), now + Duration::from_millis(3)));
    }

    #[test]
    fn paste_burst_suppression_expires() {
        let now = Instant::now();
        let mut guard = PasteBurstGuard::default();

        guard.note_bracketed_paste(now);
        assert!(guard.ignore_plain_key(&key('p'), now + Duration::from_millis(1)));
        // After PASTE_BURST_SUPPRESS_MS (50ms) elapses, plain keys must
        // pass through again.
        assert!(!guard.ignore_plain_key(&key('p'), now + Duration::from_millis(52)));
    }

    #[test]
    fn bracketed_paste_starts_printable_suppression_window() {
        let now = Instant::now();
        let mut guard = PasteBurstGuard::default();

        guard.note_bracketed_paste(now);

        assert!(guard.ignore_plain_key(&key('p'), now + Duration::from_millis(1)));
    }

    #[test]
    fn runtime_speed_control_clamps_to_safe_limits() {
        assert_eq!(
            runtime_speed_clamp(f32::NAN, crate::rain_style::RainStyle::Glyph),
            RUNTIME_SPEED_MIN
        );
        assert_eq!(
            runtime_speed_clamp(-10.0, crate::rain_style::RainStyle::Glyph),
            RUNTIME_SPEED_MIN
        );
        assert_eq!(
            runtime_speed_clamp(9999.0, crate::rain_style::RainStyle::Glyph),
            RUNTIME_SPEED_MAX
        );
        assert_eq!(
            runtime_speed_clamp(9999.0, crate::rain_style::RainStyle::Monolith),
            MONOLITH_EFFECTIVE_SPEED_MAX
        );
    }

    #[test]
    fn paste_suppression_does_not_trigger_shortcut_actions() {
        // Verify that paste events go through the Paste branch, not Key,
        // so they never trigger 'c', 's', 'p', or other shortcuts.
        let now = Instant::now();
        let mut guard = PasteBurstGuard::default();

        // Simulate a bracketed paste event
        guard.note_bracketed_paste(now);

        // Printable keys during the suppression window should be silently
        // ignored — they must not reach the keybinding handler.
        assert!(guard.ignore_plain_key(&key('c'), now + Duration::from_millis(1)));
        assert!(guard.ignore_plain_key(&key('s'), now + Duration::from_millis(1)));
        assert!(guard.ignore_plain_key(&key('p'), now + Duration::from_millis(1)));
    }

    // --- Tab key safety tests ---
    // These tests verify that Tab and BackTab are safely ignored and do not
    // cause ghost background artifacts, state mutations, or visual flicker.

    fn make_test_cloud() -> Cloud {
        let mut cloud = Cloud::new(
            crate::runtime::ColorMode::Mono,
            false,
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

    fn make_test_config() -> CloudConfig {
        CloudConfig {
            color_mode: crate::runtime::ColorMode::Mono,
            fullwidth: false,
            shading_mode: crate::runtime::ShadingMode::Random,
            bold_mode: crate::runtime::BoldMode::Off,
            async_mode: false,
            default_bg: true,
            color_scheme: crate::runtime::ColorScheme::Green,
            rain_style: crate::rain_style::RainStyle::Glyph,
            noglitch: true,
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
            target_fps: 60.0,
            duration: None,
            duration_s: None,
            bench_frames: None,
            benchmark: false,
            bench_duration: None,
            color_tune: crate::color_tune::ColorTune::IDENTITY,
            density_auto: false,
            base_density: 0.8,
            perf_stats: false,
            screensaver: false,
            mouse: false,
            charset_preset: String::from("binary"),
            user_ranges: vec![],
            def_ascii: true,
            auto_color_drift: false,
            atmosphere_modulation: crate::atmosphere_apply::AtmosphereRuntimeModulation::identity(),
            atmosphere_mode: crate::atmosphere_apply::AtmosphereApplicationMode::Disabled,
        }
    }

    fn tab_key() -> KeyEvent {
        KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)
    }

    fn backtab_key() -> KeyEvent {
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)
    }

    fn call_handle_keybinding(
        cloud: &mut Cloud,
        frame: &mut Frame,
        key: &KeyEvent,
        charset_preset: &mut String,
        cfg: &CloudConfig,
        #[cfg(unix)] term_reinit: &Arc<AtomicBool>,
    ) -> bool {
        let mut scene_name = String::from("monolith");
        call_handle_keybinding_with_scene(
            cloud,
            frame,
            key,
            charset_preset,
            &mut scene_name,
            cfg,
            #[cfg(unix)]
            term_reinit,
        )
    }

    fn call_handle_keybinding_with_scene(
        cloud: &mut Cloud,
        frame: &mut Frame,
        key: &KeyEvent,
        charset_preset: &mut String,
        scene_name: &mut String,
        cfg: &CloudConfig,
        #[cfg(unix)] term_reinit: &Arc<AtomicBool>,
    ) -> bool {
        let user_ranges: [(char, char); 0] = [];
        handle_keybinding(
            cloud,
            frame,
            key,
            charset_preset,
            scene_name,
            &user_ranges,
            true,
            cfg,
            #[cfg(unix)]
            term_reinit,
        )
    }

    #[test]
    fn lowercase_x_cycles_scene_forward() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");

        call_handle_keybinding_with_scene(
            &mut cloud,
            &mut frame,
            &key('x'),
            &mut charset_preset,
            &mut scene_name,
            &make_test_config(),
            #[cfg(unix)]
            &Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(scene_name, "matrix");
        assert_eq!(cloud.active_scene(), "matrix");
    }

    #[test]
    fn uppercase_x_cycles_scene_forward() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");

        call_handle_keybinding_with_scene(
            &mut cloud,
            &mut frame,
            &key('X'),
            &mut charset_preset,
            &mut scene_name,
            &make_test_config(),
            #[cfg(unix)]
            &Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(scene_name, "matrix");
        assert_eq!(cloud.active_scene(), "matrix");
    }

    #[test]
    fn uppercase_x_repeated_uses_forward_scene_order() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut visited = Vec::new();

        for _ in 0..3 {
            call_handle_keybinding_with_scene(
                &mut cloud,
                &mut frame,
                &key('X'),
                &mut charset_preset,
                &mut scene_name,
                &make_test_config(),
                #[cfg(unix)]
                &Arc::new(AtomicBool::new(false)),
            );
            visited.push(scene_name.clone());
        }

        assert_eq!(visited, ["matrix", "signal", "monolith"]);
        assert_eq!(cloud.active_scene(), "monolith");
    }

    #[test]
    fn tab_key_is_ignored() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");

        let shading_before = cloud.shading_distance;
        let pause_before = cloud.pause;
        let color_before = cloud.color_scheme();

        let result = call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &tab_key(),
            &mut charset_preset,
            &make_test_config(),
            #[cfg(unix)]
            &Arc::new(AtomicBool::new(false)),
        );

        assert!(!result, "Tab should not signal a keybinding action");
        assert_eq!(
            cloud.shading_distance, shading_before,
            "Tab should not toggle shading mode"
        );
        assert_eq!(cloud.pause, pause_before, "Tab should not toggle pause");
        assert_eq!(
            cloud.color_scheme(),
            color_before,
            "Tab should not change color scheme"
        );
    }

    #[test]
    fn backtab_key_is_ignored() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");

        let shading_before = cloud.shading_distance;

        let result = call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &backtab_key(),
            &mut charset_preset,
            &make_test_config(),
            #[cfg(unix)]
            &Arc::new(AtomicBool::new(false)),
        );

        assert!(!result, "BackTab should not signal a keybinding action");
        assert_eq!(
            cloud.shading_distance, shading_before,
            "BackTab should not toggle shading mode"
        );
    }

    #[test]
    fn tab_does_not_toggle_pause() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");

        assert!(!cloud.pause, "cloud should start unpaused");

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &tab_key(),
            &mut charset_preset,
            &make_test_config(),
            #[cfg(unix)]
            &Arc::new(AtomicBool::new(false)),
        );

        assert!(!cloud.pause, "Tab should not pause the rain");
    }

    #[test]
    fn tab_does_not_change_color_or_charset() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");

        let color_before = cloud.color_scheme();

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &tab_key(),
            &mut charset_preset,
            &make_test_config(),
            #[cfg(unix)]
            &Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(
            cloud.color_scheme(),
            color_before,
            "Tab should not change color scheme"
        );
        assert_eq!(
            charset_preset, "binary",
            "Tab should not change charset preset"
        );
    }

    #[test]
    fn tab_does_not_force_ghost_background_redraw() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &tab_key(),
            &mut charset_preset,
            &make_test_config(),
            #[cfg(unix)]
            &Arc::new(AtomicBool::new(false)),
        );

        assert!(
            !cloud.is_semantic_invalidate(),
            "Tab should not set semantic_invalidate"
        );
        assert!(
            !cloud.is_force_draw_everything(),
            "Tab should not set force_draw_everything"
        );
    }

    #[test]
    fn repeated_tab_is_stable() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");

        let shading_before = cloud.shading_distance;
        let pause_before = cloud.pause;

        for _ in 0..10 {
            call_handle_keybinding(
                &mut cloud,
                &mut frame,
                &tab_key(),
                &mut charset_preset,
                &make_test_config(),
                #[cfg(unix)]
                &Arc::new(AtomicBool::new(false)),
            );
        }

        assert_eq!(
            cloud.shading_distance, shading_before,
            "10 Tab presses should not change shading mode"
        );
        assert_eq!(
            cloud.pause, pause_before,
            "10 Tab presses should not change pause state"
        );
        assert!(
            !cloud.is_semantic_invalidate(),
            "10 Tab presses should not set semantic_invalidate"
        );
        assert!(
            !cloud.is_force_draw_everything(),
            "10 Tab presses should not set force_draw_everything"
        );
    }
}
