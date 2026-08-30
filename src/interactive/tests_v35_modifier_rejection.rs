// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v50 alpha.3+ modifier-key rejection tests.
//!
//! Extracted from `tests_v35.rs` (which holds `mod cases_v35`) as a
//! separate `mod cases_modifier_rejection` to keep `tests_v35.rs`
//! under the 800-LOC cap. Pure code motion — no behavior change.
//!
//! Covers:
//! - Super/Hyper/Meta/Control/Alt + key rejection (must NOT cycle
//!   color/scene/charset/pause/reset)
//! - Bare key + Shift+key allowlist (the only allowed modifier states)
//! - `is_unmodified` / `is_unmodified_or_shift` predicate contracts

#[cfg(test)]
mod cases_modifier_rejection {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::cloud::Cloud;
    use crate::frame::Frame;

    use crate::interactive::input::{
        handle_keybinding, is_unmodified, is_unmodified_or_shift, KeybindingCtx,
    };
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
        // v50.0.0-beta.7 Z-master-1B: Shift+c/s/x produce uppercase
        // C/S/X which DO cycle (reverse). Other keys with Shift are no-ops.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut cp = String::from("binary");
        let cfg = make_test_config(); let tri = default_term_reinit();
        let density0 = cloud.droplet_density; let pause0 = cloud.pause;
        call_handle_keybinding(&mut cloud, &mut frame, &key_with_mod('q', KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert!(cloud.raining, "Shift+q must NOT quit");
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
        // v50.0.0-beta.7 Z-master-1B: Shift+c/s/x all produce uppercase
        // C/S/X which DO cycle (reverse). Only Super/Hyper/Meta/Ctrl/Alt
        // + key are no-ops. This test now verifies Super+X is still rejected.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut cp = String::from("binary"); let mut sn = String::from("monolith"); let mut sg: u64 = 0;
        let cfg = make_test_config(); let tri = default_term_reinit();
        call_handle_keybinding_with_scene(&mut cloud, &mut frame, &key_with_mod('x', KeyModifiers::SUPER), &mut cp, &mut sn, &mut sg, &cfg, &tri);
        assert_eq!(sn, "monolith", "Super+X must NOT cycle scene");
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
