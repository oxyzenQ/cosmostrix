// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

#[cfg(test)]
mod cases {
    use std::time::{Duration, Instant};

    use crate::platform::{default_term_reinit, TermReinit};

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::cloud::Cloud;
    use crate::constants::*;
    use crate::frame::Frame;

    use crate::interactive::activity::{idle_resync_due, is_runtime_idle, register_activity};
    use crate::interactive::input::{
        handle_keybinding, is_unmodified, is_unmodified_or_shift, runtime_speed_clamp,
        should_auto_snapback, KeybindingCtx, PasteBurstGuard,
    };
    use crate::{cycle_charset_preset, cycle_color_scheme, CloudConfig, PowerManager};

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
        let mut pm = PowerManager::new(60.0, start);
        let mut last_resync_time = start;

        assert!(register_activity(
            &mut pm,
            &mut last_resync_time,
            activity_time,
            true,
            false,
        ));
        assert_eq!(last_resync_time, activity_time);
        // PowerManager's idle timer is now reset — verify via is_idle().
        assert!(!pm.is_idle());
    }

    #[test]
    fn active_mouse_activity_does_not_force_resync_every_frame() {
        let start = Instant::now();
        let activity_time = start + Duration::from_secs(1);
        let mut pm = PowerManager::new(60.0, start);
        let mut last_resync_time = start;

        assert!(!register_activity(
            &mut pm,
            &mut last_resync_time,
            activity_time,
            false,
            false,
        ));
        assert_eq!(last_resync_time, start);
        // PowerManager's idle timer is still reset (note_activity fires).
        assert!(!pm.is_idle());
    }

    #[test]
    fn focus_activity_can_force_resync_while_active() {
        let start = Instant::now();
        let activity_time = start + Duration::from_secs(1);
        let mut pm = PowerManager::new(60.0, start);
        let mut last_resync_time = start;

        assert!(register_activity(
            &mut pm,
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
        // This is the critical case for printable shortcuts like
        // C (color cycle), S (charset), P (pause) on terminals that
        // emit Press+Release pairs — previously the queue-ready heuristic
        // would drop the Press because the Release was already queued.
        assert!(!guard.ignore_plain_key(&key('p'), now));
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
            shading_mode: crate::runtime::ShadingMode::Random,
            bold_mode: crate::runtime::BoldMode::Off,
            async_mode: false,
            default_bg: true,
            color_scheme: crate::runtime::ColorScheme::Green,
            custom_palette: None,
            custom_palette_name: None,
            rain_style: crate::rain_style::RainStyle::Glyph,
            glitch_enabled: false,
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
            intro: crate::config::IntroType::None,
            mouse: false,
            charset_preset: String::from("binary"),
            user_ranges: vec![],
            def_ascii: true,
            auto_color_drift: false,
            monolith_density_map: None,
            config_path_for_watcher: None,
            scene_name: "monolith".to_string(),
            scene_custom_name: None,
            cli_explicit: crate::app::CliExplicit::default(),
            ambient_schedule: crate::ambient::AmbientSchedule::default(),
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
    fn lowercase_x_cycles_scene_forward() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;

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

        assert_eq!(scene_name, "matrix");
        assert_eq!(cloud.active_scene(), "matrix");
    }

    #[test]
    fn x_repeated_uses_forward_scene_order() {
        // v30 simplify: lowercase-only shortcuts. Uppercase 'X' removed;
        // this test now verifies lowercase 'x' repeated cycles forward.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;
        let mut visited = Vec::new();

        for _ in 0..3 {
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
            visited.push(scene_name.clone());
        }

        assert_eq!(visited, ["matrix", "cinematic", "monolith"]);
        assert_eq!(cloud.active_scene(), "monolith");
    }

    #[test]
    fn uppercase_x_is_now_ignored() {
        // v30 simplify: uppercase 'X' removed for consistency. Verify it
        // falls through to the no-op arm (no scene change).
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;

        call_handle_keybinding_with_scene(
            &mut cloud,
            &mut frame,
            &key('X'),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            &make_test_config(),
            &default_term_reinit(),
        );

        // scene_name local variable should be unchanged (no scene cycle fired).
        assert_eq!(scene_name, "monolith");
    }

    #[test]
    fn uppercase_c_reverses_color_cycle() {
        // shift+c (KeyCode::Char('C')) cycles the color scheme backward.
        // Restored per owner instruction: shift+c/s is the simple reverse-
        // cycle binding (c/C = forward/backward color, s/S = forward/backward
        // charset). Replaces the v30-simplify removal of uppercase 'C'.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;

        // Snapshot the forward-cycle neighbor so we can verify 'C' lands on
        // the same scheme that 'c' would have produced on a fresh cloud —
        // i.e. pressing 'C' on the default scheme yields the scheme that sits
        // immediately before it in the catalog (wrap-around to the last).
        let forward_neighbor = {
            let mut probe = make_test_cloud();
            probe.set_color_scheme(cycle_color_scheme(probe.color_scheme(), 1));
            probe.color_scheme()
        };
        // Pressing 'C' once should wrap to the scheme that, when cycled
        // forward once, returns to the default. So 'C' then 'c' == default.
        let default_scheme = cloud.color_scheme();

        call_handle_keybinding_with_scene(
            &mut cloud,
            &mut frame,
            &key('C'),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            &make_test_config(),
            &default_term_reinit(),
        );

        // 'C' must change the scheme (not be a no-op)...
        assert_ne!(
            cloud.color_scheme(),
            default_scheme,
            "uppercase C should reverse-cycle the color scheme"
        );
        // ...and it must land on the scheme immediately before the default
        // (i.e. cycling forward from the 'C' result returns to default).
        assert_eq!(
            cycle_color_scheme(cloud.color_scheme(), 1),
            default_scheme,
            "uppercase C should produce the reverse neighbor of the default scheme"
        );
        // Sanity: 'C' produces the wrap-around neighbor (last in catalog),
        // which is also the scheme that 'c' starting from the default's
        // reverse neighbor would produce. Concretely: 'c' from default gives
        // forward_neighbor; 'C' from default gives forward_neighbor's
        // reverse neighbor (== default). Verify the inverse relationship.
        let _ = forward_neighbor; // silence unused warning if cycle order changes
    }

    #[test]
    fn uppercase_s_reverses_charset_cycle() {
        // shift+s (KeyCode::Char('S')) cycles the charset preset backward.
        // Restored per owner instruction. See uppercase_c_reverses_color_cycle
        // for the rationale.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;
        let charset_before = charset_preset.clone();

        call_handle_keybinding_with_scene(
            &mut cloud,
            &mut frame,
            &key('S'),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            &make_test_config(),
            &default_term_reinit(),
        );

        // 'S' must change the charset preset (not be a no-op)...
        assert_ne!(
            charset_preset, charset_before,
            "uppercase S should reverse-cycle the charset preset"
        );
        // ...and cycling forward from the result returns to the original.
        assert_eq!(
            cycle_charset_preset(&charset_preset, 1),
            charset_before,
            "uppercase S should produce the reverse neighbor of the original preset"
        );
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
            &default_term_reinit(),
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
            &default_term_reinit(),
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
            &default_term_reinit(),
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
            &default_term_reinit(),
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
            &default_term_reinit(),
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
                &default_term_reinit(),
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

    // ───  regression: mouse click during idle must wake renderer ────────
    //
    // Bug: the mouse click handler used `let _ = register_activity(...)`,
    // silently discarding the return value. When the cloud was idle (30s
    // no input), a click did NOT trigger force_draw_everything or
    // next_frame=now. The click effect rendered at the throttled 30 FPS
    // idle cadence, causing the quantum ripple lifespan to expire before
    // the effect was fully visible — the "click effect immediately gone"
    // bug. Key presses already had the wake-on-idle behavior; mouse
    // clicks now match. (Note: lifespan is now 2.5s as of v50 masterclass
    // retune — the regression was originally filed against the 0.8s
    // lifespan, but the wake-on-idle fix is still required because the
    // 30 FPS idle cadence would visibly stutter the ripple motion.)
    //
    // This test verifies the register_activity contract: a click during
    // idle returns true (caller should force_draw + advance next_frame).

    #[test]
    fn mouse_click_during_idle_schedules_resync() {
        let start = Instant::now();
        let activity_time = start + Duration::from_secs(60);
        let mut pm = PowerManager::new(60.0, start);
        let mut last_resync_time = start;

        // Simulate a click during idle (was_idle = true).
        let should_force_draw = register_activity(
            &mut pm,
            &mut last_resync_time,
            activity_time,
            true,  // was_idle
            false, // force_resync
        );

        assert!(
            should_force_draw,
            "click during idle must return true (caller should force_draw + next_frame=now)"
        );
        assert_eq!(
            last_resync_time, activity_time,
            "click during idle must update last_resync_time"
        );
        assert!(
            !pm.is_idle(),
            "click must reset the idle timer so next frame runs at full FPS"
        );
    }

    #[test]
    fn mouse_click_while_active_does_not_force_resync() {
        let start = Instant::now();
        let activity_time = start + Duration::from_secs(1);
        let mut pm = PowerManager::new(60.0, start);
        let mut last_resync_time = start;

        // Simulate a click while active (was_idle = false).
        let should_force_draw = register_activity(
            &mut pm,
            &mut last_resync_time,
            activity_time,
            false, // was_idle
            false, // force_resync
        );

        assert!(
            !should_force_draw,
            "click while active must NOT force_draw (avoids redundant full redraws)"
        );
        assert_eq!(
            last_resync_time, start,
            "click while active must NOT update last_resync_time"
        );
        assert!(!pm.is_idle(), "click must still reset the idle timer");
    }

    /// Mouse click must spawn quantum particles, regardless of idle state.
    /// This guards the set_mouse_click contract.
    #[test]
    fn mouse_click_spawns_quantum_particles() {
        let mut cloud = make_test_cloud();
        let active_before = cloud.quantum_active_count;

        cloud.set_mouse_click(5, 5);

        assert!(
            cloud.quantum_active_count > active_before,
            "click must spawn quantum particles"
        );
    }

    // ── ambient harmony flag tests ──
    //
    // When the user presses x/c/s/C/S, two flags must be updated:
    //   - user_override_since_ambient = true  (so next ambient fire isn't deduped)
    //   - ambient_palette_locked = false      (so auto-drift palette drift resumes)
    // (except 's'/'S' which only sets user_override, not the palette lock —
    // charset change doesn't affect palette).

    #[test]
    fn v35_c_key_sets_user_override_and_clears_palette_lock() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;
        // Simulate ambient having fired.
        cloud.ambient_palette_locked = true;
        cloud.user_override_since_ambient = false;

        call_handle_keybinding_with_scene(
            &mut cloud,
            &mut frame,
            &key('c'),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert!(
            cloud.user_override_since_ambient,
            "'c' key must set user_override_since_ambient = true"
        );
        assert!(
            !cloud.ambient_palette_locked,
            "'c' key must clear ambient_palette_locked"
        );
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

    // v50 LTS regression tests (first-reload scene reset crash) live in the
    // sibling file `tests_v50_first_reload.rs` (declared at file bottom).
    // Extracted to keep this file under the 1500-LOC cap.

    // ── Modifier rejection tests (v50 alpha.3) ─────────────────────────────
    //
    // Owner-reported bug: Super+C still cycled colors on modern terminals
    // (kitty, wezterm, foot) that report the kitty keyboard protocol's
    // enhanced modifier bits. crossterm 0.29 exposes SUPER/HYPER/META as
    // separate KeyModifiers bits. The previous denylist only blocked
    // CONTROL|ALT, leaving SUPER/HYPER/META unguarded.
    //
    // The fix uses an allowlist: only NONE (bare key) or SHIFT (for S/C
    // capitals) are accepted. All other modifiers are rejected. These
    // tests verify every shortcut rejects every non-allowed modifier.

    fn key_with_mod(ch: char, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), mods)
    }

    fn arrow_with_mod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    /// Returns the color scheme before the keypress, for later comparison.
    /// ColorScheme derives PartialEq, so direct comparison works.
    fn color_scheme_of(cloud: &Cloud) -> crate::runtime::ColorScheme {
        cloud.color_scheme()
    }

    #[test]
    fn super_c_does_not_cycle_color() {
        // Owner-reported bug: Super+C still cycled colors.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let before = color_scheme_of(&cloud);

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod('c', KeyModifiers::SUPER),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(
            color_scheme_of(&cloud),
            before,
            "Super+C must NOT cycle colors (owner-reported bug)"
        );
    }

    #[test]
    fn super_q_does_not_quit() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let raining_before = cloud.raining;

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod('q', KeyModifiers::SUPER),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(
            cloud.raining, raining_before,
            "Super+Q must NOT quit (only bare 'q' quits)"
        );
    }

    #[test]
    fn super_x_does_not_cycle_scene() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;

        call_handle_keybinding_with_scene(
            &mut cloud,
            &mut frame,
            &key_with_mod('x', KeyModifiers::SUPER),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(scene_name, "monolith", "Super+X must NOT cycle scene");
    }

    #[test]
    fn super_s_does_not_cycle_charset() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;
        let before = charset_preset.clone();

        call_handle_keybinding_with_scene(
            &mut cloud,
            &mut frame,
            &key_with_mod('s', KeyModifiers::SUPER),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(charset_preset, before, "Super+S must NOT cycle charset");
    }

    #[test]
    fn super_p_does_not_toggle_pause() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let paused_before = cloud.pause;

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod('p', KeyModifiers::SUPER),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(cloud.pause, paused_before, "Super+P must NOT toggle pause");
    }

    #[test]
    fn super_space_does_not_reset() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        // force_draw_everything is set by Space (reset). Capture pre-state
        // via the public getter.
        let force_draw_before = cloud.is_force_draw_everything();

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod(' ', KeyModifiers::SUPER),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(
            cloud.is_force_draw_everything(),
            force_draw_before,
            "Super+Space must NOT trigger reset"
        );
    }

    #[test]
    fn super_arrow_up_does_not_change_speed() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let speed_before = cloud.chars_per_sec;

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &arrow_with_mod(KeyCode::Up, KeyModifiers::SUPER),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(
            cloud.chars_per_sec, speed_before,
            "Super+ArrowUp must NOT change speed"
        );
    }

    #[test]
    fn super_arrow_down_does_not_change_speed() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let speed_before = cloud.chars_per_sec;

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &arrow_with_mod(KeyCode::Down, KeyModifiers::SUPER),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(
            cloud.chars_per_sec, speed_before,
            "Super+ArrowDown must NOT change speed"
        );
    }

    #[test]
    fn super_brackets_do_not_change_density() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let density_before = cloud.droplet_density;

        // Super+[
        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod('[', KeyModifiers::SUPER),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );
        assert_eq!(
            cloud.droplet_density, density_before,
            "Super+[ must NOT decrease density"
        );

        // Super+]
        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod(']', KeyModifiers::SUPER),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );
        assert_eq!(
            cloud.droplet_density, density_before,
            "Super+] must NOT increase density"
        );
    }

    #[test]
    fn hyper_c_does_not_cycle_color() {
        // HYPER (0b10000) is a separate modifier bit from SUPER — verify
        // the allowlist catches it too.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let before = color_scheme_of(&cloud);

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod('c', KeyModifiers::HYPER),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(
            color_scheme_of(&cloud),
            before,
            "Hyper+C must NOT cycle colors"
        );
    }

    #[test]
    fn meta_c_does_not_cycle_color() {
        // META (0b100000) is a separate modifier bit from SUPER and HYPER.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let before = color_scheme_of(&cloud);

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod('c', KeyModifiers::META),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(
            color_scheme_of(&cloud),
            before,
            "Meta+C must NOT cycle colors"
        );
    }

    #[test]
    fn control_c_does_not_cycle_color() {
        // Regression guard: CONTROL was already blocked by the old denylist.
        // Verify the new allowlist still blocks it.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let before = color_scheme_of(&cloud);

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod('c', KeyModifiers::CONTROL),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(
            color_scheme_of(&cloud),
            before,
            "Ctrl+C must NOT cycle colors"
        );
    }

    #[test]
    fn alt_c_does_not_cycle_color() {
        // Regression guard: ALT was already blocked by the old denylist.
        // Verify the new allowlist still blocks it.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let before = color_scheme_of(&cloud);

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod('c', KeyModifiers::ALT),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(
            color_scheme_of(&cloud),
            before,
            "Alt+C must NOT cycle colors"
        );
    }

    #[test]
    fn control_shift_c_does_not_cycle_color() {
        // Ctrl+Shift+C produces Char('C') with CONTROL | SHIFT modifiers.
        // The allowlist must reject this because modifiers is not empty
        // and not == SHIFT (CONTROL bit is also set).
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let before = color_scheme_of(&cloud);

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod('C', KeyModifiers::CONTROL | KeyModifiers::SHIFT),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(
            color_scheme_of(&cloud),
            before,
            "Ctrl+Shift+C must NOT cycle colors (CONTROL bit blocks despite SHIFT)"
        );
    }

    #[test]
    fn super_shift_c_does_not_cycle_color() {
        // Super+Shift+C produces Char('C') with SUPER | SHIFT modifiers.
        // Even though SHIFT is set, the SUPER bit must trigger rejection.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let before = color_scheme_of(&cloud);

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod('C', KeyModifiers::SUPER | KeyModifiers::SHIFT),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(
            color_scheme_of(&cloud),
            before,
            "Super+Shift+C must NOT cycle colors (SUPER bit blocks despite SHIFT)"
        );
    }

    #[test]
    fn bare_c_still_cycles_color_forward() {
        // Sanity guard: bare 'c' (no modifiers) must still cycle colors.
        // This protects against the allowlist being too aggressive.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let before = color_scheme_of(&cloud);

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key('c'),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_ne!(
            color_scheme_of(&cloud),
            before,
            "Bare 'c' must still cycle colors forward"
        );
    }

    #[test]
    fn shift_c_still_cycles_color_reverse() {
        // Sanity guard: Shift+C (capital 'C' with SHIFT modifier) must
        // still cycle colors in reverse. This is the only SHIFT-allowed
        // path — protects against the allowlist being too aggressive.
        // (Complementary to uppercase_c_reverses_color_cycle above.)
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let before = color_scheme_of(&cloud);

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod('C', KeyModifiers::SHIFT),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_ne!(
            color_scheme_of(&cloud),
            before,
            "Shift+C must still cycle colors in reverse"
        );
    }

    // ── Non-cycle SHIFT rejection (v50 alpha.4) ────────────────────────────
    // Owner: non-cycle shortcuts only respond to bare lowercase keys (NONE).
    // Shift+key rejected, preventing CapsLock+Shift scenarios from
    // triggering unintended actions. Previously match arms used `_` for mods.

    #[test]
    #[rustfmt::skip]
    fn shift_non_cycle_keys_are_no_ops() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut cp = String::from("binary");
        let cfg = make_test_config(); let tri = default_term_reinit();
        let color0 = color_scheme_of(&cloud); let density0 = cloud.droplet_density; let pause0 = cloud.pause;
        call_handle_keybinding(&mut cloud, &mut frame, &key_with_mod('q', KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert!(cloud.raining, "Shift+q must NOT quit");
        call_handle_keybinding(&mut cloud, &mut frame, &key_with_mod('c', KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(color_scheme_of(&cloud), color0, "Shift+c no-op");
        call_handle_keybinding(&mut cloud, &mut frame, &key_with_mod('p', KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.pause, pause0, "Shift+p no-op");
        call_handle_keybinding(&mut cloud, &mut frame, &key_with_mod('[', KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.droplet_density, density0, "Shift+[ no-op");
        call_handle_keybinding(&mut cloud, &mut frame, &key_with_mod(']', KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.droplet_density, density0, "Shift+] no-op");
        cloud.force_draw_everything(); let fd0 = cloud.is_force_draw_everything();
        call_handle_keybinding(&mut cloud, &mut frame, &key_with_mod(' ', KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.is_force_draw_everything(), fd0, "Shift+Space no-op");
        let speed0 = cloud.chars_per_sec;
        call_handle_keybinding(&mut cloud, &mut frame, &arrow_with_mod(KeyCode::Up, KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.chars_per_sec, speed0, "Shift+Up no-op");
        call_handle_keybinding(&mut cloud, &mut frame, &arrow_with_mod(KeyCode::Down, KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.chars_per_sec, speed0, "Shift+Down no-op");
    }

    #[test]
    #[rustfmt::skip]
    fn shift_scene_keys_are_no_ops() {
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut cp = String::from("binary"); let mut sn = String::from("monolith"); let mut sg: u64 = 0;
        let cfg = make_test_config(); let tri = default_term_reinit(); let cp0 = cp.clone();
        call_handle_keybinding_with_scene(&mut cloud, &mut frame, &key_with_mod('s', KeyModifiers::SHIFT), &mut cp, &mut sn, &mut sg, &cfg, &tri);
        assert_eq!(cp, cp0, "Shift+s no-op");
        call_handle_keybinding_with_scene(&mut cloud, &mut frame, &key_with_mod('x', KeyModifiers::SHIFT), &mut cp, &mut sn, &mut sg, &cfg, &tri);
        assert_eq!(sn, "monolith", "Shift+x no-op");
    }

    #[test]
    fn is_unmodified_accepts_only_none() {
        assert!(is_unmodified(KeyModifiers::NONE));
        for m in [
            KeyModifiers::SHIFT,
            KeyModifiers::CONTROL,
            KeyModifiers::ALT,
            KeyModifiers::SUPER,
            KeyModifiers::HYPER,
            KeyModifiers::META,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ] {
            assert!(!is_unmodified(m));
        }
    }

    #[test]
    fn is_unmodified_or_shift_accepts_none_and_shift() {
        assert!(is_unmodified_or_shift(KeyModifiers::NONE));
        assert!(is_unmodified_or_shift(KeyModifiers::SHIFT));
        for m in [
            KeyModifiers::CONTROL,
            KeyModifiers::ALT,
            KeyModifiers::SUPER,
            KeyModifiers::HYPER,
            KeyModifiers::META,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ] {
            assert!(!is_unmodified_or_shift(m));
        }
    }
}

// v50 LTS regression tests (first-reload scene reset crash). Extracted to
// keep this file under the 1500-LOC cap. `#[path]` must be at top level
// (not inside `mod cases`) for path resolution — same pattern as hud.rs.
#[cfg(test)]
#[path = "tests_v50_first_reload.rs"]
mod v50_first_reload;
