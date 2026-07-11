// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Verbose diagnostic output for --verbose flag.
//!
//! Extracted from main.rs to keep that file under 1000 LOC.
//! Prints comprehensive runtime configuration to stderr for
//! power users / hackers debugging config and loading issues.

use crate::atmosphere_apply::{AtmosphereApplicationMode, AtmosphereRuntimeModulation};
use crate::color_tune::ColorTune;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, MonolithSize, ShadingMode};
use crate::{configfile, is_safe_path};

#[allow(clippy::too_many_arguments)]
pub(crate) fn print_verbose(
    version: &str,
    scene: Option<&str>,
    rain_style: RainStyle,
    color_scheme: crate::runtime::ColorScheme,
    color_mode: ColorMode,
    color_tune: ColorTune,
    default_bg: bool,
    charset_preset: &str,
    chars: &[char],
    fullwidth: bool,
    target_fps: f64,
    speed: f32,
    base_density: f32,
    density_auto: bool,
    monolith_size: MonolithSize,
    async_mode: bool,
    bold_mode: BoldMode,
    shading_mode: ShadingMode,
    noglitch: bool,
    glitch_pct: f32,
    glitch_low: u16,
    glitch_high: u16,
    glitch_level: &str,
    mouse: bool,
    low_power: bool,
    screensaver: bool,
    auto_drift: bool,
    atmosphere_mode: AtmosphereApplicationMode,
    atmosphere_modulation: &AtmosphereRuntimeModulation,
    message: Option<&str>,
    message_border: bool,
    duration: Option<f64>,
    charset_file: Option<&str>,
    screen_size: Option<(u16, u16)>,
) {
    eprintln!("[verbose] ════════════════════════════════════════════════════");
    eprintln!("[verbose]  cosmostrix v{version} — runtime configuration");
    eprintln!("[verbose] ════════════════════════════════════════════════════");
    eprintln!("[verbose]  scene:        {:?}", scene.unwrap_or("default"));
    eprintln!("[verbose]  rain_style:   {rain_style:?}");
    eprintln!("[verbose]  color_scheme: {color_scheme:?}");
    eprintln!("[verbose]  color_mode:   {color_mode:?}");
    eprintln!(
        "[verbose]  color_tune:   sat={:.2} bright={:.2}",
        color_tune.saturation, color_tune.brightness
    );
    eprintln!("[verbose]  color_bg:     {default_bg:?}");
    eprintln!(
        "[verbose]  charset:      {charset_preset} ({} glyphs)",
        chars.len()
    );
    eprintln!("[verbose]  fullwidth:    {fullwidth}");
    eprintln!("[verbose]  fps:          {target_fps:.1}");
    eprintln!("[verbose]  speed:        {speed:.1}");
    eprintln!("[verbose]  density:      {base_density:.2} (auto: {density_auto})");
    eprintln!("[verbose]  monolith:     {monolith_size:?}");
    eprintln!("[verbose]  async_mode:   {async_mode} (variable column speeds)");
    eprintln!("[verbose]  bold:         {bold_mode:?}");
    eprintln!("[verbose]  shading:      {shading_mode:?}");
    eprintln!(
        "[verbose]  glitch:       {} ({glitch_pct}%, {glitch_low}-{glitch_high}ms)",
        !noglitch
    );
    eprintln!("[verbose]  glitch_level: {glitch_level:?}");
    eprintln!("[verbose]  mouse:        {mouse}");
    eprintln!("[verbose]  low_power:    {low_power}");
    eprintln!("[verbose]  screensaver:  {screensaver}");
    eprintln!("[verbose]  auto_drift:   {auto_drift}");
    eprintln!("[verbose]  atmosphere:   {atmosphere_mode:?} / {atmosphere_modulation:?}");
    // Screen size: fixed (--screen-size) or dynamic (terminal-detected)
    let (sw, sh, size_mode) = match screen_size {
        Some((w, h)) => (w, h, "fixed"),
        None => {
            // Detect actual terminal size for display
            let (tw, th) = crossterm::terminal::size().unwrap_or((0, 0));
            (tw, th, "auto")
        }
    };
    eprintln!("[verbose]  screen_size:  {sw}x{sh} ({size_mode})");
    if let Some(msg) = message {
        eprintln!(
            "[verbose]  message:      \"{msg}\" ({} chars, border: {message_border})",
            msg.chars().count()
        );
    }
    if let Some(d) = duration {
        eprintln!("[verbose]  duration:     {d:.1}s");
    }
    eprintln!("[verbose] ──────────────────────────────────────────────────");
    let term = std::env::var("TERM").unwrap_or_else(|_| "(unset)".into());
    let colorterm = std::env::var("COLORTERM").unwrap_or_else(|_| "(unset)".into());
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "(unset)".into());
    let term_version = std::env::var("TERM_PROGRAM_VERSION").unwrap_or_else(|_| "(unset)".into());
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "(unset)".into());
    let lang = std::env::var("LANG").unwrap_or_else(|_| "(unset)".into());
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let is_stdout_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());
    eprintln!("[verbose]  TERM:         {term}");
    eprintln!("[verbose]  COLORTERM:    {colorterm}");
    eprintln!("[verbose]  TERM_PROGRAM: {term_program}");
    eprintln!("[verbose]  TERM_VERSION: {term_version}");
    eprintln!("[verbose]  SHELL:        {shell}");
    eprintln!("[verbose]  LANG:         {lang}");
    eprintln!("[verbose]  isatty(stderr): {is_tty}");
    eprintln!("[verbose]  isatty(stdout): {is_stdout_tty}");
    let config_path = configfile::default_config_file_path();
    eprintln!("[verbose]  config_path:  {}", config_path.display());
    eprintln!("[verbose]  config exists: {}", config_path.exists());
    if let Some(cf) = charset_file {
        eprintln!("[verbose]  charset_file: {cf} (safe: {})", is_safe_path(cf));
    }
    let is_android = std::env::var("TERMUX_VERSION").is_ok()
        || std::env::var("PREFIX").is_ok_and(|p| p.contains("com.termux"));
    eprintln!("[verbose]  android:      {is_android}");
    eprintln!("[verbose] ════════════════════════════════════════════════════");
}
