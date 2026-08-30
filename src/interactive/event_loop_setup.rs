// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Pre-loop terminal + cloud + frame setup for `run_interactive()`.
//!
//! Extracted from `event_loop.rs` to keep that file under the 800-LOC cap.
//! Pure code motion — no behavior change.
//!
//! Owns the startup sequence:
//! - signal handler installation
//! - alt-screen terminal construction + mouse capture
//! - cloud creation + reset + phosphor tuning + component timing
//! - color cache build + frame construction
//! - pre-frame alt-screen bg fill
//!
//! The cinematic intro sequence is owned by `event_loop_intro.rs` (already
//! extracted). This function sets up everything the intro needs, then
//! the caller invokes the intro and the main loop separately.

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::color_cache::ColorCache;
use crate::frame::Frame;
use crate::terminal::Terminal;

use super::super::{effective_density, CloudConfig};
use super::watchdog::MOUSE_CAPTURE_ACTIVE;
use crate::cloud::Cloud;
use crate::platform::TermReinit;

/// Terminal + cloud + frame triple returned by [`setup_terminal_cloud_frame`].
pub(crate) struct LoopSetup {
    pub term: Terminal,
    pub cloud: Cloud,
    pub frame: Frame,
    pub w: u16,
    pub h: u16,
    pub signal_exit: Arc<AtomicBool>,
    pub term_reinit: TermReinit,
}

/// Install signal handlers, build the alt-screen Terminal, create + tune
/// the Cloud, build the color cache + Frame, and fill the alt-screen bg.
///
/// Returns the ([`Terminal`], [`Cloud`], [`Frame`], w, h) tuple plus the
/// signal-exit handle and TermReinit token. Caller is responsible for
/// invoking `event_loop_intro::run_intro_sequence` next, then the main
/// `while cloud.raining` loop.
pub(crate) fn setup_terminal_cloud_frame(cfg: &CloudConfig) -> std::io::Result<LoopSetup> {
    // term_reinit is TermReinit (Arc<AtomicBool> on Unix, () on Windows).
    // On Windows, TermReinit=() so all swap() calls are eliminated at
    // compile time — no #[cfg] needed at the usage sites.
    let (signal_exit, term_reinit) = super::signal_handlers::install_signal_handlers();

    // AB-10: emit pre-alt-screen warnings BEFORE Terminal::with_signal_exit()
    // enters the alt screen. Otherwise they leak into the rain matrix.
    let fixed_size = cfg.screen_size;
    super::emit_pre_alt_screen_warnings(
        fixed_size,
        cfg.intro != crate::intro_style::IntroType::None,
    );

    let mut term = Terminal::with_signal_exit(signal_exit.clone())?;
    if term.enable_mouse_capture().is_ok() {
        MOUSE_CAPTURE_ACTIVE.store(true, Ordering::Release);
    }
    let (w, h) = if let Some(fixed) = fixed_size {
        fixed
    } else {
        term.size()?
    };

    let density = effective_density(cfg.base_density, w, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);
    cloud.enable_events();
    // P1: per-component timing only when --perf-stats (skips 2 Instant::now()
    // per frame when off, ~40ns saved).
    cloud.set_component_timing(cfg.perf_stats);
    cloud.set_effects_enabled(cfg.effects_enabled);
    let caps = term.phosphor_tuning();
    cloud.set_phosphor_tuning(caps.0, caps.1, caps.2);
    // Bug-fix: no ambient phase has fired yet, so the user's CLI/config
    // choices ARE the authoritative state. Without this, the first live
    // reload would incorrectly re-apply scene defaults (cinematic/zen/
    // energyzen) via the empty-schedule handler, overriding the user's
    // explicit --charset, --color, and config.toml values. With
    // user_override_since_ambient = true, the preserve_user_override
    // branch correctly restores the user's state after each rebuild.
    cloud.user_override_since_ambient = true;

    // Build color byte cache so the draw hot path emits pre-formatted SGR.
    term.set_color_cache(ColorCache::new(&cloud.palette));

    let frame = Frame::new(w, h, cloud.palette.bg);

    // v16: fill alt screen with palette bg before first frame (no edge gaps).
    super::fill_terminal_bg(cloud.palette.bg);

    Ok(LoopSetup {
        term,
        cloud,
        frame,
        w,
        h,
        signal_exit,
        term_reinit,
    })
}
