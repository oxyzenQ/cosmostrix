// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI presentation helpers: help templates, clap styling, color/charset scheme
//! parsing, cycling, and terminal color detection.

use std::env;

use crate::config::Args;
use crate::runtime::{ColorMode, ColorScheme};
use crate::theme;

// --- Help template builder ---
//
// `--help` intentionally omits the `{name} {version}` and `{about-with-newline}`
// header lines. The header is reserved for `-V` / `--version` only, so the
// help output opens straight with `USAGE:` for a clean first impression.
//
// The `USAGE:` label is rendered in brand purple (bold) with capability-aware
// escapes: truecolor RGB on modern terminals, 256-color palette index on
// older 256-color terminals, basic 16-color magenta on legacy terminals,
// and plain text when piped or NO_COLOR is set.

/// Build the clap `help_template` string.
///
/// `color` should be true when stdout is a TTY (and `NO_COLOR` is unset).
#[must_use]
pub(crate) fn help_template(color: bool) -> String {
    if color {
        format!(
            "{}USAGE:{}\n  {{usage}}\n\n{{all-args}}{{after-help}}",
            crate::output::brand_bold_open(),
            crate::output::reset(),
        )
    } else {
        "USAGE:\n  {usage}\n\n{all-args}{after-help}".to_string()
    }
}

// --- Clap styling ---

#[cfg(unix)]
use clap::builder::styling::{Color as ClapColor, RgbColor as ClapRgbColor};
#[cfg(unix)]
use clap::builder::styling::{Effects as ClapEffects, Style as ClapStyle};
#[cfg(unix)]
use clap::builder::Styles as ClapStyles;

#[must_use]
#[cfg(unix)]
pub(crate) fn clap_styles() -> ClapStyles {
    // Purple brand identity: section headings (USAGE, COMMON OPTIONS,
    // ADVANCED, CONFIG, DIAGNOSTICS, HELP) rendered in bold truecolor
    // purple #A855F7 (168, 85, 247). This matches the BRAND_BOLD constant
    // in crate::output, so every purple element in --help / -V / verbose /
    // errors uses the exact same RGB value.
    //
    // Literals and placeholders use default terminal color (white) — no
    // yellow, cyan, or green.
    //
    // Why truecolor and not Ansi(Magenta): the basic 16-color ANSI
    // palette is mapped by the terminal emulator to whatever shade of
    // magenta it prefers (usually #FF00FF or similar hot pink), which
    // does NOT match #A855F7. Truecolor emits the exact RGB bytes.
    ClapStyles::styled()
        .header(
            ClapStyle::new()
                .effects(ClapEffects::BOLD)
                .fg_color(Some(ClapColor::Rgb(ClapRgbColor(168, 85, 247)))),
        )
        .usage(
            ClapStyle::new()
                .effects(ClapEffects::BOLD)
                .fg_color(Some(ClapColor::Rgb(ClapRgbColor(168, 85, 247)))),
        )
        .literal(ClapStyle::new().effects(ClapEffects::BOLD))
        .placeholder(ClapStyle::new())
}

// --- Charset helpers ---

#[must_use]
pub fn default_to_ascii() -> bool {
    let lang = env::var("LANG").unwrap_or_default();
    !lang.to_ascii_uppercase().contains("UTF")
}

// --- Color mode detection ---

#[must_use]
pub(crate) fn detect_color_mode_from_terms(colorterm: &str, term: &str) -> ColorMode {
    let colorterm = colorterm.to_ascii_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return ColorMode::TrueColor;
    }

    let term = term.to_ascii_lowercase();
    if term == "dumb" {
        return ColorMode::Mono;
    }
    if term.contains("-truecolor") || term.ends_with("-direct") {
        return ColorMode::TrueColor;
    }
    if term.contains("256color") {
        return ColorMode::Color256;
    }

    ColorMode::Color16
}

#[must_use]
pub fn detect_color_mode_auto() -> ColorMode {
    #[cfg(windows)]
    {
        if env::var_os("WT_SESSION").is_some() {
            return ColorMode::TrueColor;
        }
    }

    let colorterm = env::var("COLORTERM").unwrap_or_default();
    let term = env::var("TERM").unwrap_or_default();
    detect_color_mode_from_terms(&colorterm, &term)
}

pub fn detect_color_mode(args: &Args) -> ColorMode {
    if let Some(m) = args.colormode {
        return match m {
            0 => ColorMode::Mono,
            16 => ColorMode::Color16,
            8 | 256 => ColorMode::Color256,
            24 | 32 => ColorMode::TrueColor,
            _ => {
                // Route through ux::die_input so the error message and exit
                // code (2) match every other CLI input error in the codebase.
                // Previously this used process::exit(1) + eprintln_error_labeled,
                // which bypassed the ux module and used the wrong exit code.
                crate::ux::die_input(format!(
                    "invalid --colormode: {m} (allowed: 0,16,8/256,24/32)"
                ));
            }
        };
    }

    detect_color_mode_auto()
}

#[must_use]
pub fn color_mode_label(m: ColorMode) -> &'static str {
    match m {
        ColorMode::TrueColor => "24-bit truecolor",
        ColorMode::Color256 => "8-bit (256-color)",
        ColorMode::Mono => "mono",
        ColorMode::Color16 => "16-color",
    }
}

// --- Color scheme helpers ---

#[must_use]
pub(crate) fn all_color_schemes() -> &'static [ColorScheme] {
    theme::SCHEME_ORDER.as_slice()
}

#[must_use]
pub fn cycle_color_scheme(current: ColorScheme, dir: i32) -> ColorScheme {
    let list = all_color_schemes();
    let Some(pos) = list.iter().position(|&c| c == current) else {
        return ColorScheme::Green;
    };

    let n = list.len() as i32;
    let mut idx = pos as i32 + dir;
    idx = ((idx % n) + n) % n;
    list[idx as usize]
}

// --- Charset preset helpers ---

#[must_use]
pub(crate) fn all_charset_presets() -> &'static [&'static str] {
    // IMPORTANT: "auto" is intentionally EXCLUDED from this list.
    //
    // "auto" is a meta-value (not a real charset) that means "let
    // cosmostrix pick a default based on terminal capabilities" — see
    // `charset_from_str("auto", ...)` which resolves to `Charset::MATRIX`
    // or `Charset::ASCII_SAFE`. It is valid as a CLI/config input, but
    // it must NOT appear in the cycle list used by `cycle_charset_preset`.
    //
    // Bug history ("charset drift" user report):
    //   When "auto" was index 0 and "zen" was the last entry, pressing
    //   's' (cycle forward) from "zen" wrapped around to index 0 =
    //   "auto". The `charset_from_str("auto", false)` call in
    //   `input.rs::handle_keybinding` then silently replaced the user's
    //   custom charset (e.g. `[charset-custom.zen.set]`) with the
    //   built-in Matrix glyph pool — destroying their 2-glyph custom
    //   config without any indication. The post-exit verbose log
    //   reported `charset: auto (was zen)`, which looked like an
    //   uninvited "drift" event but was actually this cycle bug.
    //
    // Fix: exclude "auto" from the cycle candidate list. CLI/config
    // parsing still accepts "auto" via `charset_from_str`; only the
    // interactive 's'/'S' cycle is sanitized. If the user explicitly
    // starts with `--charset auto` and presses 's', the cycle falls
    // through to the `None` branch in `cycle_charset_preset` and lands
    // on the `"binary"` fallback — which is a real, renderable charset.
    &[
        "matrix",
        "ascii",
        "extended",
        "english",
        "digits",
        "punc",
        "binary",
        "hex",
        "katakana",
        "greek",
        "cyrillic",
        "hebrew",
        "blocks",
        "symbols",
        "arrows",
        "retro",
        "cyberpunk",
        "hacker",
        "minimal",
        "code",
        "dna",
        "braille",
        "runic",
        "zen",
    ]
}

#[must_use]
pub fn normalize_charset_preset_name(s: &str) -> String {
    match s.trim().to_ascii_lowercase().as_str() {
        "bin" | "01" => "binary".to_string(),
        "dec" | "decimal" => "digits".to_string(),
        "hexadecimal" => "hex".to_string(),
        other => other.to_string(),
    }
}

#[must_use]
pub fn cycle_charset_preset(current: &str, dir: i32) -> &'static str {
    let list = all_charset_presets();
    let Some(pos) = list.iter().position(|&c| c == current) else {
        return "binary";
    };

    let n = list.len() as i32;
    let mut idx = pos as i32 + dir;
    idx = ((idx % n) + n) % n;
    list[idx as usize]
}

pub fn parse_color_scheme(s: &str) -> Result<ColorScheme, String> {
    theme::lookup_theme(s).ok_or_else(|| {
        // (bug #13): add "did you mean" suggestion for close matches.
        // Catches approximate names and typos, suggesting the closest current
        // theme when edit distance ≤ 2 (e.g. a slightly-misspelled color name
        // gets nudged toward the nearest valid theme).
        let suggestion = closest_color_name(s);
        if let Some(name) = suggestion {
            format!(
                "error: unknown color '{s}'\n\n  Did you mean '{name}'?\n  Use --list-colors to see all available colors."
            )
        } else {
            format!(
                "error: unknown color '{s}'\n\n  Use --list-colors to see available colors."
            )
        }
    })
}

/// (bug #13): find the closest built-in color name to `input` using
/// edit distance. Returns `Some(name)` if the best match has distance ≤ 2,
/// or `None` if no color is close enough. Also checks theme aliases (e.g.
/// `deep-sea` is an alias for `ocean`), so a typo like `deap-sea` would
/// suggest `deep-sea` (the alias).
#[must_use]
fn closest_color_name(input: &str) -> Option<&'static str> {
    let input_lower = input.trim().to_ascii_lowercase();
    if input_lower.is_empty() {
        return None;
    }
    let mut best: Option<(&'static str, usize)> = None;
    for theme in theme::themes().iter() {
        // Check canonical name
        let dist = edit_distance(&input_lower, theme.name);
        if dist <= 2 {
            match best {
                None => best = Some((theme.name, dist)),
                Some((_, d)) if dist < d => best = Some((theme.name, dist)),
                _ => {}
            }
        }
        // Check aliases
        for &alias in theme.aliases {
            let dist = edit_distance(&input_lower, alias);
            if dist <= 2 {
                match best {
                    None => best = Some((theme.name, dist)),
                    Some((_, d)) if dist < d => best = Some((theme.name, dist)),
                    _ => {}
                }
            }
        }
    }
    best.map(|(name, _)| name)
}

// v51 did-you-mean audit: edit_distance moved to cli/suggestion.rs as the
// shared engine (closest_value_match + closest_color_name both use it).
use suggestion::edit_distance;

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: "auto" must never appear in the cycle candidate list.
    ///
    /// This is the root cause of the "charset drift" bug
    /// where pressing 's' from "zen" wrapped to "auto" and silently
    /// replaced the user's custom charset with the built-in Matrix pool.
    #[test]
    fn all_charset_presets_excludes_auto() {
        let list = all_charset_presets();
        assert!(
            !list.contains(&"auto"),
            "`auto` is a meta-value and must not be a cycle target, found: {list:?}"
        );
    }

    /// Regression: cycling forward from the last entry ("zen") must wrap to
    /// the first REAL preset ("matrix"), not to "auto".
    #[test]
    fn cycle_forward_from_zen_wraps_to_matrix_not_auto() {
        let next = cycle_charset_preset("zen", 1);
        assert_eq!(
            next, "matrix",
            "forward cycle from 'zen' must wrap to 'matrix', got '{next}'"
        );
        assert_ne!(next, "auto", "cycle must never return 'auto'");
    }

    /// Regression: cycling backward from the first entry ("matrix") must
    /// wrap to the last REAL preset ("zen"), not to "auto".
    #[test]
    fn cycle_backward_from_matrix_wraps_to_zen_not_auto() {
        let prev = cycle_charset_preset("matrix", -1);
        assert_eq!(
            prev, "zen",
            "backward cycle from 'matrix' must wrap to 'zen', got '{prev}'"
        );
        assert_ne!(prev, "auto", "cycle must never return 'auto'");
    }

    /// Sanity: forward then backward must return to the original preset.
    /// This verifies the cycle is a proper bijection on the real presets.
    #[test]
    fn cycle_forward_then_backward_is_identity() {
        for &preset in all_charset_presets() {
            let fwd = cycle_charset_preset(preset, 1);
            let back = cycle_charset_preset(fwd, -1);
            assert_eq!(
                back, preset,
                "forward then backward from '{preset}' should return to '{preset}', got '{back}'"
            );
        }
    }

    /// Edge case: if the user starts with `--charset auto` (still valid
    /// via `charset_from_str`), pressing 's' falls through to the
    /// `"binary"` fallback. This is acceptable — "binary" is a real,
    /// renderable charset, and the user can continue cycling from there.
    #[test]
    fn cycle_from_unknown_preset_falls_back_to_binary() {
        let next = cycle_charset_preset("auto", 1);
        assert_eq!(
            next, "binary",
            "cycle from unknown preset 'auto' must fall back to 'binary', got '{next}'"
        );
    }

    /// Exhaustive: no cycle direction or starting point should ever
    /// produce "auto". This is the core invariant of the bug fix.
    #[test]
    fn cycle_never_returns_auto_from_any_preset() {
        let list = all_charset_presets();
        for &preset in list {
            for dir in [-2, -1, 1, 2] {
                let result = cycle_charset_preset(preset, dir);
                assert_ne!(
                    result, "auto",
                    "cycle(preset='{preset}', dir={dir}) returned 'auto' — this is forbidden"
                );
            }
        }
    }
}

// Submodules (moved from src/ root for clean src/ layout)
pub(crate) mod app;
pub(crate) mod argv_expand;
pub(crate) mod build_cloud_cfg;
pub(crate) mod canonicalize;
pub(crate) mod cli_explicit;
pub(crate) mod cli_parse;
pub(crate) mod early_returns;
pub(crate) mod help_detail;
pub(crate) mod suggestion;
