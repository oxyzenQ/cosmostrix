// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v50.0.0-beta.7 Z-master-1B: kitty CSI-u Shift+letter shape tests.
//!
//! Extracted from `tests_v35_modifier_rejection.rs` to keep that file
//! under the 800-LOC cap (repo convention: split, don't exempt).
//!
//! Covers:
//! - Kitty-keyboard-protocol terminals (kitty, Alacritty, WezTerm,
//!   ghostty, foot, konsole) report Shift+letter as the BASE lowercase
//!   codepoint plus the SHIFT modifier bit (`CSI 120;2u` for Shift+X
//!   arrives as `Char('x') + SHIFT`), while legacy terminals report the
//!   shifted uppercase char (`Char('X') + SHIFT`).
//! - `normalize_shifted_char()` maps both shapes onto the same
//!   reverse-cycle arms, so Shift+X/C/S work identically on both
//!   terminal families (owner-reported bug: Shift+X was a no-op).
//! - Non-cycle keys must reject their Shift variants in the kitty shape
//!   too (owner policy: Shift is the only accepted modifier, and only
//!   the cycle keys c/s/x have uppercase bindings).

#[cfg(test)]
mod cases_kitty_shift {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::cloud::Cloud;
    use crate::frame::Frame;

    use crate::interactive::input::{handle_keybinding, normalize_shifted_char, KeybindingCtx};
    use crate::platform::{default_term_reinit, TermReinit};
    use crate::CloudConfig;

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
            // v80.0.0-beta.2 msg-fill-style: pinned to Typewriter explicitly so these
            // style-agnostic tests never depend on the champion default (engrave
            // since v80.0.0-beta.2 - the default contract is locked separately in
            // tests_msg_fill_style.rs and clap_suggestion.rs).
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

    fn key_with_mod(ch: char, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), mods)
    }

    fn arrow_with_mod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    /// Duplicated from `tests.rs::mod cases::color_scheme_of()` (test fixture, stable).
    fn color_scheme_of(cloud: &Cloud) -> crate::runtime::ColorScheme {
        cloud.color_scheme()
    }

    #[test]
    fn kitty_shift_x_cycles_scene_reverse() {
        // CSI 120;2u -> Char('x') + SHIFT.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("matrix");
        let mut scene_generation: u64 = 0;

        call_handle_keybinding_with_scene(
            &mut cloud,
            &mut frame,
            &key_with_mod('x', KeyModifiers::SHIFT),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_eq!(
            scene_name, "monolith",
            "kitty CSI-u Shift+X must cycle scene in reverse (matrix -> monolith)"
        );
    }

    #[test]
    fn kitty_shift_c_cycles_color_reverse() {
        // CSI 99;2u -> Char('c') + SHIFT.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let before = color_scheme_of(&cloud);

        call_handle_keybinding(
            &mut cloud,
            &mut frame,
            &key_with_mod('c', KeyModifiers::SHIFT),
            &mut charset_preset,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_ne!(
            color_scheme_of(&cloud),
            before,
            "kitty CSI-u Shift+C must cycle colors in reverse"
        );
    }

    #[test]
    fn kitty_shift_s_cycles_charset_reverse() {
        // CSI 115;2u -> Char('s') + SHIFT.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;
        let before = charset_preset.clone();

        call_handle_keybinding_with_scene(
            &mut cloud,
            &mut frame,
            &key_with_mod('s', KeyModifiers::SHIFT),
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            &make_test_config(),
            &default_term_reinit(),
        );

        assert_ne!(
            charset_preset, before,
            "kitty CSI-u Shift+S must cycle charset in reverse"
        );
    }

    #[test]
    #[rustfmt::skip]
    fn kitty_shift_non_cycle_keys_are_no_ops() {
        // CSI-u base+SHIFT forms of non-cycle keys must stay no-ops,
        // matching the owner policy: only bare lowercase q/c/s/x/i/p,
        // '[', ']', arrows, and 'r' respond; Shift variants are
        // rejected. ('i' is handled in the event loop HUD branch, not
        // in handle_keybinding — no-op here by design.)
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let mut cp = String::from("binary");
        let cfg = make_test_config(); let tri = default_term_reinit();
        let density0 = cloud.droplet_density; let pause0 = cloud.pause;
        let raining0 = cloud.raining;
        call_handle_keybinding(&mut cloud, &mut frame, &key_with_mod('q', KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.raining, raining0, "kitty Shift+q must NOT quit");
        call_handle_keybinding(&mut cloud, &mut frame, &key_with_mod('p', KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.pause, pause0, "kitty Shift+p no-op");
        call_handle_keybinding(&mut cloud, &mut frame, &key_with_mod('[', KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.droplet_density, density0, "kitty Shift+[ no-op");
        call_handle_keybinding(&mut cloud, &mut frame, &key_with_mod(']', KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.droplet_density, density0, "kitty Shift+] no-op");
        cloud.force_draw_everything(); let fd0 = cloud.is_force_draw_everything();
        call_handle_keybinding(&mut cloud, &mut frame, &key_with_mod('r', KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.is_force_draw_everything(), fd0, "kitty Shift+r no-op");
        let speed0 = cloud.chars_per_sec;
        call_handle_keybinding(&mut cloud, &mut frame, &arrow_with_mod(KeyCode::Up, KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.chars_per_sec, speed0, "kitty Shift+Up no-op");
        call_handle_keybinding(&mut cloud, &mut frame, &arrow_with_mod(KeyCode::Down, KeyModifiers::SHIFT), &mut cp, &cfg, &tri);
        assert_eq!(cloud.chars_per_sec, speed0, "kitty Shift+Down no-op");
    }

    #[test]
    fn normalize_shifted_char_maps_kitty_base_to_uppercase() {
        // Unit tests for the normalization helper contract.
        assert!(matches!(
            normalize_shifted_char(KeyCode::Char('x'), KeyModifiers::SHIFT),
            KeyCode::Char('X')
        ));
        assert!(matches!(
            normalize_shifted_char(KeyCode::Char('c'), KeyModifiers::SHIFT),
            KeyCode::Char('C')
        ));
        assert!(matches!(
            normalize_shifted_char(KeyCode::Char('s'), KeyModifiers::SHIFT),
            KeyCode::Char('S')
        ));
        // Non-SHIFT modifier states pass through untouched.
        assert!(matches!(
            normalize_shifted_char(KeyCode::Char('x'), KeyModifiers::NONE),
            KeyCode::Char('x')
        ));
        assert!(matches!(
            normalize_shifted_char(KeyCode::Char('x'), KeyModifiers::SUPER),
            KeyCode::Char('x')
        ));
        // Already-uppercase, non-letter, and non-Char keys pass through.
        assert!(matches!(
            normalize_shifted_char(KeyCode::Char('X'), KeyModifiers::SHIFT),
            KeyCode::Char('X')
        ));
        assert!(matches!(
            normalize_shifted_char(KeyCode::Char('['), KeyModifiers::SHIFT),
            KeyCode::Char('[')
        ));
        assert!(matches!(
            normalize_shifted_char(KeyCode::Up, KeyModifiers::SHIFT),
            KeyCode::Up
        ));
    }
}
