// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cinematic intro selection sequence — extracted from `event_loop.rs`
//! to keep that file under the 800-LOC cap.
//!
//! Owns the resolution chain for `cfg.intro_color`:
//!   1. `intro_color` unset → brand EnergyZen intro cloud (the signature
//!      purple-neon palette; `-c`/`--color`/`--colors-custom` never
//!      repaint the intro)
//!   2. CLI/config-set builtin theme name → build intro cloud with that scheme
//!   3. Custom palette name (from `[colors-custom.<name>]`) → build intro cloud
//!      with that palette
//!   4. Invalid name (validation failed silently at startup) → fallback to the
//!      brand EnergyZen intro cloud so the intro still plays
//!
//! Owner bug report (2026-08-30): `cosmostrix -c neon-green` repainted the
//! intro LOGO neon-green. Root cause: the unset-path passed the LIVE rain
//! cloud to the intro, and `logo_stage_colors()` samples the cloud's
//! palette stops — so the logo followed whatever `-c` selected. The intro
//! logo is a brand mark: it stays EnergyZen (the same scheme the cinematic
//! scene applies by default) unless `--intro-color` explicitly overrides
//! it. Only `--intro-color` / config `intro-color` can repaint the intro.
//!
//! After the intro animation completes, this module also re-reads the terminal
//! size (bug #10: user may have resized during the intro) and resets the
//! Cloud + Frame if dimensions changed. This keeps the render loop's first
//! frame consistent with the post-intro terminal geometry.

use crate::cloud::Cloud;
use crate::frame::Frame;
use crate::intro_style::IntroType;
use crate::runtime::ColorScheme;
use crate::terminal::Terminal;
use crate::CloudConfig;

use super::super::effective_density;
use crate::constants::{
    MAX_TERMINAL_COLS, MAX_TERMINAL_LINES, MIN_TERMINAL_COLS, MIN_TERMINAL_LINES,
};

/// Brand scheme for the intro cinematic when `--intro-color` is unset (or
/// invalid): EnergyZen — the signature purple-neon palette, the same scheme
/// the cinematic scene applies to the rain by default. This is why a
/// no-flags run and a `-c <any-theme>` run now show the same brand intro.
const INTRO_BRAND_SCHEME: ColorScheme = ColorScheme::EnergyZen;

/// Run the cinematic intro sequence (if enabled in `cfg.intro`).
///
/// Behavior:
/// - If `cfg.intro == IntroType::None`, returns immediately (no-op).
/// - Otherwise resolves `cfg.intro_color` per the chain documented at the
///   module level, runs `crate::intro_style::run_intro(...)`, then post-processes:
///     * `cloud.force_draw_everything()` + `frame.clear_with_bg(bg)` so the
///       first render-loop frame starts from a clean slate.
///     * If `cfg.screen_size` is unset (dynamic sizing) and the terminal was
///       resized during the intro, resets `cloud` + rebuilds `frame` with
///       the new clamped dimensions (bug #10 fix).
///
/// ## v52 message-reveal lead (owner bug report)
///
/// The 6 s message lead is armed HERE — right before the cinematic
/// starts, the only place that knows one is about to play — and CUT when
/// the intro did not run to completion (q skip, shutdown signal,
/// terminal below the intro floor). Pre-v52 the lead was armed
/// unconditionally inside `set_message`, so `--intro none`, an 'r'
/// restart, and live-reload cloud rebuilds all showed a dead 6 s wait
/// for the message with nothing hiding it.
///
/// Parameters are passed by `&mut` because the intro mutates the terminal
/// (writes animation frames), the frame buffer (clears + draws), the cloud
/// (force-draws after intro), and the live `w`/`h` (post-intro resize).
///
/// Cloud does not derive Clone (intentionally — it holds large buffers +
/// ghost event state), so every intro path builds a scoped temporary
/// `intro_cloud` from `cfg.create_cloud(density)` and then overrides its
/// palette (brand EnergyZen, the `--intro-color` scheme, or the custom
/// palette). The temporary is dropped at the end of this function — it is
/// NOT propagated to the loop; the live rain `cloud` keeps the user's
/// `-c`/`--colors-custom` palette for the rain itself.
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
    // The message hides behind the intro for MESSAGE_INTRO_LEAD, so the
    // text lands shortly after the cinematic ends (the tuned feel).
    cloud.hold_message_behind_intro();
    let outcome = run_intro_with_color_resolution(term, frame, cfg, *w, *h, density)?;
    if outcome == crate::intro_style::IntroOutcome::CutShort {
        // Intro cut short or never played: nothing is hiding the message
        // anymore — pull a still-future reveal start to now.
        cloud.cut_message_intro_lead();
    }
    cloud.force_draw_everything();
    frame.clear_with_bg(cloud.palette.bg);
    re_read_terminal_size_after_intro(term, cloud, frame, w, h, cfg);
    Ok(())
}

/// Pure decision: which palette source does the intro use for a given
/// `intro-color` value? Exposed separately from the I/O-heavy runner so
/// the resolution contract is unit-testable without a `Terminal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntroPaletteSource {
    /// Unset → brand EnergyZen scheme (never the rain palette).
    Brand,
    /// Builtin theme name → that scheme.
    Scheme(ColorScheme),
    /// Custom palette name → caller loads it from the config file.
    Custom,
}

fn resolve_intro_palette_source(intro_color: Option<&str>) -> IntroPaletteSource {
    match intro_color {
        None => IntroPaletteSource::Brand,
        Some(name) => match crate::theme::lookup_theme(name) {
            Some(scheme) => IntroPaletteSource::Scheme(scheme),
            None => IntroPaletteSource::Custom,
        },
    }
}

/// Build the brand intro cloud: a fresh cloud from `cfg` (keeps charset,
/// density, glitch, color-tune re-application, etc.) with the palette
/// forced to the EnergyZen brand scheme. `set_color_scheme` clears
/// `custom_palette_active`, so a `--colors-custom` palette cannot leak
/// into the intro either. `pub(super)` for the v80.0.0-beta.1 regression tests
/// (tests_v51_intro_brand_pause.rs).
pub(super) fn brand_intro_cloud(cfg: &CloudConfig, density: f32) -> Cloud {
    let mut intro_cloud = cfg.create_cloud(density);
    intro_cloud.set_color_scheme(INTRO_BRAND_SCHEME);
    intro_cloud
}

/// Resolve `cfg.intro_color` and run the intro animation.
///
/// Per the priority chain:
/// 1. `intro_color` unset → brand EnergyZen intro cloud (NOT the rain
///    cloud — the intro logo must not follow `-c`/`--color`/
///    `--colors-custom`).
/// 2. `intro_color` matches a builtin theme → fresh intro_cloud with that
///    scheme + that scheme's palette RGB.
/// 3. `intro_color` matches a custom palette → fresh intro_cloud with that
///    palette + that palette's RGB.
/// 4. `intro_color` invalid → fallback to the brand EnergyZen intro cloud
///    (validation failed silently at startup; user sees the intro anyway).
///
/// Propagates the [`crate::intro_style::IntroOutcome`] so the caller can
/// decide whether the message-reveal lead stands.
///
/// `w` + `h` are passed by value (copy) because the intro does not mutate
/// them — the post-intro resize is handled separately by
/// `re_read_terminal_size_after_intro`.
fn run_intro_with_color_resolution(
    term: &mut Terminal,
    frame: &mut Frame,
    cfg: &CloudConfig,
    w: u16,
    h: u16,
    density: f32,
) -> std::io::Result<crate::intro_style::IntroOutcome> {
    match resolve_intro_palette_source(cfg.intro_color.as_deref()) {
        IntroPaletteSource::Brand => {
            // Cases 1 + 4: brand EnergyZen intro cloud. logo_color = the
            // palette head (bright lavender), consistent with cases 2/3.
            let intro_cloud = brand_intro_cloud(cfg, density);
            let logo_color = crate::intro_style::palette_target_rgb(&intro_cloud);
            crate::intro_style::run_intro(term, frame, &intro_cloud, w, h, cfg.intro, logo_color)
        }
        IntroPaletteSource::Scheme(scheme) => {
            let mut intro_cloud = cfg.create_cloud(density);
            intro_cloud.set_color_scheme(scheme);
            let logo_color = crate::intro_style::palette_target_rgb(&intro_cloud);
            crate::intro_style::run_intro(term, frame, &intro_cloud, w, h, cfg.intro, logo_color)
        }
        IntroPaletteSource::Custom => {
            let name = cfg.intro_color.as_deref().unwrap_or_default();
            let cfg_map = crate::configfile::load_config_file(None);
            match crate::colors_custom::load_custom_palette(&cfg_map, name) {
                Ok(palette) => {
                    let mut intro_cloud = cfg.create_cloud(density);
                    intro_cloud.palette = palette;
                    let logo_color = crate::intro_style::palette_target_rgb(&intro_cloud);
                    crate::intro_style::run_intro(
                        term,
                        frame,
                        &intro_cloud,
                        w,
                        h,
                        cfg.intro,
                        logo_color,
                    )
                }
                // Case 4: invalid name — validation failed silently at
                // startup; fall back to the brand cloud so the intro
                // still plays.
                Err(_) => {
                    let intro_cloud = brand_intro_cloud(cfg, density);
                    let logo_color = crate::intro_style::palette_target_rgb(&intro_cloud);
                    crate::intro_style::run_intro(
                        term,
                        frame,
                        &intro_cloud,
                        w,
                        h,
                        cfg.intro,
                        logo_color,
                    )
                }
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Unset intro_color → Brand source (the owner bug: this path used to
    /// borrow the rain cloud, so `-c neon-green` repainted the logo).
    #[test]
    fn resolve_unset_is_brand() {
        assert_eq!(
            resolve_intro_palette_source(None),
            IntroPaletteSource::Brand
        );
    }

    /// Any builtin theme name (incl. aliases) resolves to that scheme.
    #[test]
    fn resolve_builtin_theme_names() {
        assert_eq!(
            resolve_intro_palette_source(Some("neon-green")),
            IntroPaletteSource::Scheme(ColorScheme::NeonGreen)
        );
        assert_eq!(
            resolve_intro_palette_source(Some("energy-zen")),
            IntroPaletteSource::Scheme(ColorScheme::EnergyZen)
        );
        assert_eq!(
            resolve_intro_palette_source(Some("ez")),
            IntroPaletteSource::Scheme(ColorScheme::EnergyZen)
        );
    }

    /// A name that is neither builtin nor resolvable at this layer routes
    /// to the Custom arm — the caller then loads it from the config file
    /// and soft-fails to Brand when it is not a custom palette either.
    #[test]
    fn resolve_unknown_name_is_custom() {
        assert_eq!(
            resolve_intro_palette_source(Some("definitely-not-a-theme")),
            IntroPaletteSource::Custom
        );
    }

    /// The brand scheme constant is EnergyZen — locks the owner contract:
    /// the intro logo defaults to the signature purple-neon palette.
    #[test]
    fn brand_scheme_is_energyzen() {
        assert_eq!(INTRO_BRAND_SCHEME, ColorScheme::EnergyZen);
    }
}
