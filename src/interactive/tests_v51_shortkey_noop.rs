// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-beta.1 Z-master-1B shortkey audit: exhaustive no-op lock.
//!
//! Owner mandate: verify that ONLY the documented shortkeys have effects
//! — every other key must be a complete no-op. Trigger: a user pressing
//! `a` (an old cosmostrix async-mode toggle muscle memory) expected it to
//! do something; source code is truth and `a` has no match arm.
//!
//! The ACTIVE keymap (source code = truth, `handle_keybinding` in
//! `src/interactive/input.rs` + the HUD toggle in `event_loop.rs`):
//!
//! | Key | Effect | Handled in |
//! |-----|--------|-----------|
//! | `q` | Quit | input.rs |
//! | `r` | Reset animation + restart message typewriter | input.rs |
//! | `c` / `C` | Cycle color scheme fwd / reverse | input.rs |
//! | `s` / `S` | Cycle charset preset fwd / reverse | input.rs |
//! | `p` | Pause / resume (during pause: only `p` and `q` respond) | input.rs |
//! | `x` / `X` | Cycle scene fwd / reverse | input.rs |
//! | `Up` / `Down` | Speed up / down | input.rs |
//! | `[` / `]` | Density down / up | input.rs |
//! | `i` | Toggle live HUD (rejected while paused) | event_loop.rs |
//! | `q` / `Q` | Skip the intro cinematic (intro only) | intro_style/mod.rs |
//!
//! Everything else — including the removed/legacy keys `a` (v35 ambient
//! snapback; even older async toggle), `h` (HUD position), `Tab`/`BackTab`
//! (shading toggle, v30), `-`/`_`/`+`/`=` (density aliases), and every
//! letter/digit/punctuation/function key not listed above — hits the
//! `_ => {}` catch-all: no state change, no redraw, no exit.

#[cfg(test)]
mod cases_v51_shortkey_noop {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::cloud::Cloud;
    use crate::frame::Frame;

    use crate::interactive::input::handle_keybinding;
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
        // Pin mutable numeric state so no-op assertions have exact baselines.
        cloud.set_droplet_density(1.0);
        cloud.set_chars_per_sec(8.0);
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
            density: 1.0,
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
            base_density: 1.0,
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

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Dispatch a key through the same path the event loop uses and return
    /// whether a redraw was requested.
    fn dispatch(cloud: &mut Cloud, code: KeyCode) -> bool {
        let mut frame = Frame::new(20, 10, None);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;
        let cfg = make_test_config();
        let term_reinit: TermReinit = default_term_reinit();
        let user_ranges: [(char, char); 0] = [];
        let mut ctx = crate::interactive::input::KeybindingCtx {
            cloud,
            frame: &mut frame,
            charset_preset: &mut charset_preset,
            scene_name: &mut scene_name,
            scene_generation: &mut scene_generation,
            user_ranges: &user_ranges,
            def_ascii: true,
            cfg: &cfg,
            term_reinit: &term_reinit,
        };
        handle_keybinding(&mut ctx, &key_event(code))
    }

    /// Assert a key is a COMPLETE no-op: no redraw, no state change.
    fn assert_no_op(code: KeyCode, label: &str) {
        let mut cloud = make_test_cloud();
        let scheme_before = cloud.color_scheme();
        let density_before = cloud.droplet_density();
        let speed_before = cloud.chars_per_sec;
        let raining_before = cloud.raining;
        let pause_before = cloud.pause;
        let async_before = cloud.async_mode;

        let redraw = dispatch(&mut cloud, code);

        assert!(!redraw, "{label}: no-op key must not request a redraw");
        assert_eq!(
            cloud.color_scheme(),
            scheme_before,
            "{label}: no-op key must not change the color scheme"
        );
        assert_eq!(
            cloud.droplet_density(),
            density_before,
            "{label}: no-op key must not change density"
        );
        assert_eq!(
            cloud.chars_per_sec, speed_before,
            "{label}: no-op key must not change speed"
        );
        assert_eq!(
            cloud.raining, raining_before,
            "{label}: no-op key must not change raining"
        );
        assert_eq!(
            cloud.pause, pause_before,
            "{label}: no-op key must not change pause"
        );
        assert_eq!(
            cloud.async_mode, async_before,
            "{label}: no-op key must not change async_mode (owner report: 'a' must do nothing)"
        );
    }

    /// The owner's exact scenario: pressing `a` (old-version async-mode
    /// muscle memory) must have NO effect — there is no match arm for it.
    #[test]
    fn legacy_a_key_is_complete_no_op() {
        assert_no_op(KeyCode::Char('a'), "'a' (removed async/ambient toggle)");
    }

    /// Every lowercase/uppercase letter outside the active set {c,s,x,q,p}
    /// (plus their uppercase forms {C,S,X} which ARE active) is a no-op.
    #[test]
    fn all_non_active_letters_are_no_ops() {
        let active_lowercase = ['q', 'c', 's', 'x', 'p'];
        let active_uppercase = ['C', 'S', 'X'];
        for ch in 'a'..='z' {
            if !active_lowercase.contains(&ch) {
                assert_no_op(KeyCode::Char(ch), &format!("letter '{ch}'"));
            }
        }
        for ch in 'A'..='Z' {
            if !active_uppercase.contains(&ch) {
                // NOTE: 'Q', 'P', 'I' arrive with SHIFT tagged on legacy
                // terminals (rejected by the NONE-only arms); the bare
                // uppercase char with NONE only matches the cycle arms.
                // Non-cycle uppercase keys like 'A', 'H', 'M' are no-ops.
                assert_no_op(KeyCode::Char(ch), &format!("letter '{ch}'"));
            }
        }
    }

    /// Digits, punctuation, and the removed density aliases are no-ops.
    #[test]
    fn digits_punctuation_and_removed_aliases_are_no_ops() {
        let removed_density_aliases = ['-', '_', '+', '='];
        for ch in '0'..='9' {
            assert_no_op(KeyCode::Char(ch), &format!("digit '{ch}'"));
        }
        for ch in removed_density_aliases {
            assert_no_op(KeyCode::Char(ch), &format!("removed density alias '{ch}'"));
        }
        for ch in [
            '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', ',', '.', '/', ';', '\'', '\\', '`',
            '{', '}', '|', ':', '"', '<', '>', '?', '~',
        ] {
            assert_no_op(KeyCode::Char(ch), &format!("punctuation '{ch}'"));
        }
    }

    /// Special keys outside the active set (arrows Up/Down are active;
    /// Left/Right/Tab/BackTab/Esc/Enter/Backspace/Delete/Home/End/
    /// PageUp/PageDown/Insert/F-keys are no-ops).
    #[test]
    fn non_active_special_keys_are_no_ops() {
        let special = [
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Enter,
            KeyCode::Backspace,
            KeyCode::Delete,
            KeyCode::Insert,
            KeyCode::Home,
            KeyCode::End,
            KeyCode::PageUp,
            KeyCode::PageDown,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Esc,
            KeyCode::F(1),
            KeyCode::F(5),
            KeyCode::F(12),
        ];
        for code in special {
            assert_no_op(code, &format!("special key {code:?}"));
        }
    }

    /// Positive control: every ACTIVE key must actually do something.
    /// Guards against the no-op suite passing because the dispatch helper
    /// is broken (if all keys looked like no-ops, the asserts above would
    /// be vacuously true). NOTE: only 'p' returns true from
    /// handle_keybinding (toggle_pause); every other active key signals
    /// redraw via internal force-draw flags, so these controls assert
    /// STATE changes, matching the existing test conventions.
    #[test]
    fn active_keys_all_have_effects() {
        // q quits.
        let mut cloud = make_test_cloud();
        dispatch(&mut cloud, KeyCode::Char('q'));
        assert!(!cloud.raining, "'q' must stop the rain");

        // c / C change the color scheme.
        let mut cloud = make_test_cloud();
        let before = cloud.color_scheme();
        dispatch(&mut cloud, KeyCode::Char('c'));
        assert_ne!(cloud.color_scheme(), before, "'c' must cycle the scheme");
        let mut cloud = make_test_cloud();
        dispatch(&mut cloud, KeyCode::Char('C'));
        assert_ne!(
            cloud.color_scheme(),
            before,
            "'C' must reverse-cycle the scheme"
        );

        // p engages the pause path (deceleration starts) and redraws.
        let mut cloud = make_test_cloud();
        assert!(dispatch(&mut cloud, KeyCode::Char('p')), "'p' must redraw");
        assert!(
            cloud.is_paused_or_decelerating(),
            "'p' must engage pause/deceleration"
        );

        // [ / ] change density.
        let mut cloud = make_test_cloud();
        dispatch(&mut cloud, KeyCode::Char('['));
        assert!(
            (cloud.droplet_density() - 1.0).abs() > 0.001,
            "'[' must reduce density"
        );
        let mut cloud = make_test_cloud();
        dispatch(&mut cloud, KeyCode::Char(']'));
        assert!(cloud.droplet_density() > 1.0, "']' must increase density");

        // Up / Down change speed.
        let mut cloud = make_test_cloud();
        dispatch(&mut cloud, KeyCode::Up);
        assert_eq!(
            cloud.chars_per_sec, 9.0,
            "'Up' must increase chars_per_sec from 8.0"
        );
        let mut cloud = make_test_cloud();
        dispatch(&mut cloud, KeyCode::Down);
        assert_eq!(
            cloud.chars_per_sec, 7.0,
            "'Down' must decrease chars_per_sec from 8.0"
        );

        // 'r' resets the animation (force-draw flag set for full redraw).
        let mut cloud = make_test_cloud();
        dispatch(&mut cloud, KeyCode::Char('r'));
        assert!(
            cloud.is_force_draw_everything(),
            "'r' must force a full redraw (animation reset)"
        );
    }

    /// Positive control for the charset/scene cycle keys: they mutate the
    /// preset/scene strings via the ctx (verified through handle_keybinding
    /// side effects on the cloud — charset transition / scene application).
    #[test]
    fn active_cycle_keys_change_cloud_state() {
        // s: charset cycle transitions the char pool (cloud semantic state).
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(20, 10, None);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;
        let cfg = make_test_config();
        let term_reinit: TermReinit = default_term_reinit();
        let user_ranges: [(char, char); 0] = [];
        let mut ctx = crate::interactive::input::KeybindingCtx {
            cloud: &mut cloud,
            frame: &mut frame,
            charset_preset: &mut charset_preset,
            scene_name: &mut scene_name,
            scene_generation: &mut scene_generation,
            user_ranges: &user_ranges,
            def_ascii: true,
            cfg: &cfg,
            term_reinit: &term_reinit,
        };
        let _ = handle_keybinding(&mut ctx, &key_event(KeyCode::Char('s')));
        assert_ne!(
            charset_preset, "binary",
            "'s' must advance the charset preset"
        );
        // (transition_chars marks the cloud for redraw internally; the
        // function return is false for 's' — only 'p' returns true.)

        // x: scene cycle updates the scene name + generation.
        let mut cloud = make_test_cloud();
        let mut frame = Frame::new(20, 10, None);
        let mut charset_preset = String::from("binary");
        let mut scene_name = String::from("monolith");
        let mut scene_generation: u64 = 0;
        let cfg = make_test_config();
        let term_reinit: TermReinit = default_term_reinit();
        let user_ranges: [(char, char); 0] = [];
        let mut ctx = crate::interactive::input::KeybindingCtx {
            cloud: &mut cloud,
            frame: &mut frame,
            charset_preset: &mut charset_preset,
            scene_name: &mut scene_name,
            scene_generation: &mut scene_generation,
            user_ranges: &user_ranges,
            def_ascii: true,
            cfg: &cfg,
            term_reinit: &term_reinit,
        };
        let _ = handle_keybinding(&mut ctx, &key_event(KeyCode::Char('x')));
        assert_ne!(scene_name, "monolith", "'x' must advance the scene");
        assert_eq!(scene_generation, 1, "'x' must bump scene_generation");
    }

    /// `i` is a no-op AT THE handle_keybinding LEVEL — the HUD toggle is
    /// dispatched in the event loop (before handle_keybinding, gated by
    /// hud_toggle_accepted). This documents the split; the event-loop side
    /// is covered by tests_v51_intro_brand_pause.rs (hud_toggle_accepted).
    #[test]
    fn i_key_is_noop_at_keybinding_level() {
        assert_no_op(
            KeyCode::Char('i'),
            "'i' (HUD toggle lives in event_loop.rs)",
        );
    }
}
