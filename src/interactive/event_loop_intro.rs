// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cinematic intro selection sequence — extracted from `event_loop.rs`
//! to keep that file under the 1500-LOC cap.
//!
//! Owns the resolution chain for `cfg.intro_color`:
//!   1. CLI/config-set builtin theme name → build intro cloud with that scheme
//!   2. Custom palette name (from `[colors-custom.<name>]`) → build intro cloud
//!      with that palette
//!   3. Invalid name (validation failed silently at startup) → fallback to the
//!      rain cloud so the intro still plays
//!   4. `intro_color` unset → use the rain cloud with brand-purple logo color
//!
//! After the intro animation completes, this module also re-reads the terminal
//! size (bug #10: user may have resized during the intro) and resets the
//! Cloud + Frame if dimensions changed. This keeps the render loop's first
//! frame consistent with the post-intro terminal geometry.

use crate::cloud::Cloud;
use crate::config::IntroType;
use crate::frame::Frame;
use crate::terminal::Terminal;
use crate::CloudConfig;

use super::super::effective_density;
use crate::constants::{
    MAX_TERMINAL_COLS, MAX_TERMINAL_LINES, MIN_TERMINAL_COLS, MIN_TERMINAL_LINES,
};

/// Brand purple — the default intro logo color when no `intro-color` is set.
/// Mirrors the `output::BRAND_RGB` constant; duplicated as a const tuple here
/// to avoid pulling the whole `output` module into the intro path.
const DEFAULT_LOGO_COLOR: (u8, u8, u8) = (168, 85, 247);

/// Run the cinematic intro sequence (if enabled in `cfg.intro`).
///
/// Behavior:
/// - If `cfg.intro == IntroType::None`, returns immediately (no-op).
/// - Otherwise resolves `cfg.intro_color` per the chain documented at the
///   module level, runs `super::intro::run_intro(...)`, then post-processes:
///     * `cloud.force_draw_everything()` + `frame.clear_with_bg(bg)` so the
///       first render-loop frame starts from a clean slate.
///     * If `cfg.screen_size` is unset (dynamic sizing) and the terminal was
///       resized during the intro, resets `cloud` + rebuilds `frame` with
///       the new clamped dimensions (bug #10 fix).
///
/// Parameters are passed by `&mut` because the intro mutates the terminal
/// (writes animation frames), the frame buffer (clears + draws), the cloud
/// (force-draws after intro), and the live `w`/`h` (post-intro resize).
///
/// Cloud does not derive Clone (intentionally — it holds large buffers +
/// ghost event state), so the intro path uses a scoped temporary
/// `intro_cloud` for the builtin-theme / custom-palette cases and falls
/// back to a borrow of the live `cloud` for the unset / invalid cases.
/// The temporary `intro_cloud` is dropped at the end of this function —
/// it is NOT propagated to the loop.
pub(super) fn run_intro_sequence(
    term: &mut Terminal,
    frame: &mut Frame,
    cloud: &mut Cloud,
    w: &mut u16,
    h: &mut u16,
    cfg: &CloudConfig,
    density: f32,
) -> std::io::Result<()> {
    if cfg.intro == IntroType::None {
        return Ok(());
    }
    run_intro_with_color_resolution(term, frame, cloud, *w, *h, cfg, density)?;
    cloud.force_draw_everything();
    frame.clear_with_bg(cloud.palette.bg);
    re_read_terminal_size_after_intro(term, cloud, frame, w, h, cfg);
    Ok(())
}

/// Resolve `cfg.intro_color` and run the intro animation.
///
/// Per the priority chain:
/// 1. `intro_color` unset → run intro with the rain cloud + brand purple.
/// 2. `intro_color` matches a builtin theme → fresh intro_cloud with that
///    scheme + that scheme's palette RGB.
/// 3. `intro_color` matches a custom palette → fresh intro_cloud with that
///    palette + that palette's RGB.
/// 4. `intro_color` invalid → fallback to the rain cloud + brand purple
///    (validation failed silently at startup; user sees the intro anyway).
///
/// `w` + `h` are passed by value (copy) because the intro does not mutate
/// them — the post-intro resize is handled separately by
/// `re_read_terminal_size_after_intro`.
fn run_intro_with_color_resolution(
    term: &mut Terminal,
    frame: &mut Frame,
    cloud: &Cloud,
    w: u16,
    h: u16,
    cfg: &CloudConfig,
    density: f32,
) -> std::io::Result<()> {
    let Some(intro_color) = cfg.intro_color.as_deref() else {
        // Case 1: no intro_color → use the rain cloud + brand purple.
        return super::intro::run_intro(term, frame, cloud, w, h, cfg.intro, DEFAULT_LOGO_COLOR);
    };
    // Case 2: builtin theme lookup.
    if let Some(scheme) = crate::theme::lookup_theme(intro_color) {
        let mut intro_cloud = cfg.create_cloud(density);
        intro_cloud.set_color_scheme(scheme);
        let logo_color = super::intro::palette_target_rgb(&intro_cloud);
        return super::intro::run_intro(term, frame, &intro_cloud, w, h, cfg.intro, logo_color);
    }
    // Case 3: custom palette lookup.
    let cfg_map = crate::configfile::load_config_file(None);
    if let Ok(palette) = crate::colors_custom::load_custom_palette(&cfg_map, intro_color) {
        let mut intro_cloud = cfg.create_cloud(density);
        intro_cloud.palette = palette;
        let logo_color = super::intro::palette_target_rgb(&intro_cloud);
        return super::intro::run_intro(term, frame, &intro_cloud, w, h, cfg.intro, logo_color);
    }
    // Case 4: invalid name — fallback to rain cloud + brand purple.
    super::intro::run_intro(term, frame, cloud, w, h, cfg.intro, DEFAULT_LOGO_COLOR)
}

/// Bug #10: re-read terminal size after the intro animation completes.
///
/// The intro can take 1-3 seconds; users often resize the terminal during
/// that window. Without this re-read, the rain loop would render at the
/// pre-intro dimensions, producing visual artifacts (ghost columns,
/// misaligned glyphs) until the next resize event.
///
/// Only fires when `cfg.screen_size` is `None` (dynamic sizing). Fixed-size
/// mode (`--screen-size WxH`) ignores real terminal dimensions by design.
fn re_read_terminal_size_after_intro(
    term: &Terminal,
    cloud: &mut Cloud,
    frame: &mut Frame,
    w: &mut u16,
    h: &mut u16,
    cfg: &CloudConfig,
) {
    if cfg.screen_size.is_some() {
        return;
    }
    let Ok((nw, nh)) = term.size() else {
        return;
    };
    if nw == *w && nh == *h {
        return;
    }
    let cw = nw.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
    let ch = nh.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);
    *w = cw;
    *h = ch;
    cloud.reset(cw, ch);
    *frame = Frame::new(cw, ch, cloud.palette.bg);
    if cfg.density_auto {
        cloud.set_droplet_density(effective_density(cfg.base_density, cw, true));
    }
    cloud.force_draw_everything();
    super::fill_terminal_bg(cloud.palette.bg);
}
