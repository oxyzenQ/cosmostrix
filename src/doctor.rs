// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Doctor diagnostics: environment, locale, charset, and terminal compatibility check.

use std::env;

use crate::charset::{charset_from_str, Charset};
use crate::config::{Args, ColorBg};
use crate::diagnostics;
use crate::renderer_info;
use crate::report::Report;
use crate::runtime::ColorMode;

use super::{
    color_mode_label, default_to_ascii, detect_color_mode, detect_color_mode_auto,
    normalize_charset_preset_name,
};

#[cfg(target_os = "linux")]
use super::env_var_truthy;

pub fn print_doctor_report(args: &Args) {
    let lang = env::var("LANG").unwrap_or_default();
    let lc_all = env::var("LC_ALL").unwrap_or_default();
    let lc_ctype = env::var("LC_CTYPE").unwrap_or_default();
    let term = env::var("TERM").unwrap_or_default();
    let colorterm = env::var("COLORTERM").unwrap_or_default();

    let stdin_tty = std::io::IsTerminal::is_terminal(&std::io::stdin());
    let stdout_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());

    let locale_blob = format!("{}{}{}", lc_all, lc_ctype, lang);
    let locale_utf8 = locale_blob.to_ascii_uppercase().contains("UTF");

    let auto = detect_color_mode_auto();
    let effective = detect_color_mode(args);
    let def_ascii = default_to_ascii();

    // Environment detections
    let terminal_family = terminal_family(&term);
    let tmux = env::var("TMUX").is_ok() || terminal_family == "tmux";
    let ssh = env::var("SSH_CONNECTION").is_ok() || env::var("SSH_TTY").is_ok();
    let headless = !stdin_tty && !stdout_tty;

    let cpu = diagnostics::detect_cpu_info();
    let ri = renderer_info::renderer_info(effective);

    let mut r = Report::new("COSMOSTRIX DIAGNOSTICS REPORT");

    // SYSTEM section
    {
        let s = r.section("SYSTEM");
        s.field("stdin_tty", if stdin_tty { "yes" } else { "no" });
        s.field("stdout_tty", if stdout_tty { "yes" } else { "no" });
        s.field("variant", cpu.variant);
        s.field("optimization", env!("COSMOSTRIX_OPTIMIZATION"));
        s.field("build", cpu.build_variant);
    }

    // ENVIRONMENT section
    {
        let s = r.section("ENVIRONMENT");
        s.field("locale", if lang.is_empty() { "(unset)" } else { &lang });
        s.field("locale_utf8", if locale_utf8 { "yes" } else { "no" });
        s.field("tmux", if tmux { "yes" } else { "no" });
        s.field("ssh", if ssh { "yes" } else { "no" });
        s.field("headless", if headless { "yes" } else { "no" });
    }

    // TERMINAL section
    {
        let s = r.section("TERMINAL");
        s.field("TERM", if term.is_empty() { "(unset)" } else { &term });
        s.field("family", terminal_family);
        s.field(
            "COLORTERM",
            if colorterm.is_empty() {
                "(unset)"
            } else {
                &colorterm
            },
        );
        s.field("color_mode", ri.color_depth);

        #[cfg(target_os = "linux")]
        {
            let no_fork_guard = env_var_truthy("COSMOSTRIX_NO_FORK_GUARD");
            s.field(
                "fork_guard",
                if no_fork_guard { "disabled" } else { "enabled" },
            );
        }
        #[cfg(not(target_os = "linux"))]
        {
            s.field("fork_guard", "n/a (non-linux)");
        }

        s.field("color_auto_detected", color_mode_label(auto));
        if args.colormode.is_some() {
            s.field("color_forced", color_mode_label(effective));
        }
    }

    // COMPATIBILITY section
    {
        let s = r.section("COMPATIBILITY");
        s.field("terminal_class", terminal_family);
        s.field("color_capability", color_capability(effective));
        s.field("background", background_guidance(args.color_bg));
        s.field("normal_exit", "non-destructive mode/style restore");
        s.field(
            "reset_terminal",
            "explicit destructive recovery: clears visible screen and attempts scrollback purge",
        );
        s.field("signal_exit", "catchable cleanup (SIGINT/SIGTERM/SIGHUP)");
        s.field("sigkill", "cannot be caught or guaranteed");
        s.field("terminal_writer", "single-owner");
        s.field("mouse_mode", "opt-in only via --mouse");
        let hints = environment_hints(&term, &colorterm, locale_utf8, tmux, ssh, headless);
        let hint_text = if hints.is_empty() {
            "none".to_string()
        } else {
            hints.join(", ")
        };
        s.field("environment_hints", &hint_text);
    }

    // CHARSET section
    {
        let s = r.section("CHARSET");
        s.field(
            "preset",
            if args.charset.is_empty() {
                "(empty)"
            } else {
                &args.charset
            },
        );
        let charset_preset = normalize_charset_preset_name(&args.charset);
        if charset_preset != args.charset {
            s.field("preset_normalized", &charset_preset);
        }
        if let Some(spec) = &args.chars {
            s.field("chars_override", spec);
        }
        s.field("default_to_ascii", if def_ascii { "yes" } else { "no" });
    }

    // SAMPLE GLYPHS section (only if locale is UTF-8)
    if locale_utf8 {
        let cs = match charset_from_str(&normalize_charset_preset_name(&args.charset), def_ascii) {
            Ok(v) => v,
            Err(e) => {
                // Add parse error as a note
                let s = r.section("CHARSET");
                s.field("parse_error", &e);
                Charset::NONE
            }
        };

        let uses_katakana = cs.contains(Charset::KATAKANA);
        let uses_unicode = uses_katakana
            || cs.contains(Charset::GREEK)
            || cs.contains(Charset::CYRILLIC)
            || cs.contains(Charset::HEBREW)
            || cs.contains(Charset::BRAILLE)
            || cs.contains(Charset::RUNIC)
            || cs.contains(Charset::SYMBOLS)
            || cs.contains(Charset::ARROWS)
            || cs.contains(Charset::BLOCKS)
            || cs.contains(Charset::BOXDRAW)
            || cs.contains(Charset::MINIMAL);

        let s = r.section("SAMPLE GLYPHS");
        s.field("ascii", "01 ABC abc !@#");
        if uses_katakana {
            s.field(
                "katakana",
                "\u{FF71}\u{FF72}\u{FF73}\u{FF74}\u{FF75}\u{FF76}\u{FF77}\u{FF78}\u{FF79}\u{FF7A}",
            );
        }
        if cs.contains(Charset::GREEK) {
            s.field("greek", "\u{03A9}\u{03BB}\u{03C0}\u{0394}");
        }
        if cs.contains(Charset::CYRILLIC) {
            s.field("cyrillic", "\u{042F}\u{0416}\u{042E}\u{0428}");
        }
        if cs.contains(Charset::HEBREW) {
            s.field("hebrew", "\u{05D0}\u{05D1}\u{05D2}\u{05D3}");
        }
        if cs.contains(Charset::BRAILLE) {
            s.field("braille", "\u{28FF}\u{28F7}\u{28EF}\u{28DF}");
        }
        if cs.contains(Charset::RUNIC) {
            s.field("runic", "\u{16A0}\u{16A2}\u{16A6}\u{16A8}");
        }
        if cs.contains(Charset::SYMBOLS) {
            s.field("symbols", "\u{221E}\u{2211}\u{222B}\u{221A}\u{03C0}");
        }
        if cs.contains(Charset::ARROWS) {
            s.field("arrows", "\u{2190}\u{2192}\u{2191}\u{2193}");
        }
        if cs.contains(Charset::BLOCKS) {
            s.field("blocks", "\u{2591}\u{2592}\u{2593}\u{2588}");
        }
        if cs.contains(Charset::BOXDRAW) {
            s.field(
                "boxdraw",
                "\u{250C}\u{2510}\u{2514}\u{2518}\u{2500}\u{2502}",
            );
        }
        if cs.contains(Charset::MINIMAL) {
            s.field(
                "minimal",
                "\u{00B7}\u{2022}\u{25CB}\u{25CF}\u{25C7}\u{25C6}",
            );
        }

        // Re-borrow uses_unicode for ADVICE
        if false {
            let _ = uses_unicode;
        }
    }

    // ADVICE section
    {
        let s = r.section("ADVICE");

        if !stdin_tty || !stdout_tty {
            s.advice("headless/non-TTY detected; use --benchmark, --info, --doctor, or other redirect-safe commands");
        }
        if !locale_utf8 {
            s.advice("locale does not look like UTF-8; unicode charsets may render incorrectly");
            s.advice(
                "use a UTF-8 locale or choose an ASCII-safe charset such as --charset minimal",
            );
        }
        if should_advise_truecolor(&term, &colorterm, effective) {
            if terminal_family == "tmux" || terminal_family == "screen" || tmux {
                s.advice("256-color multiplexer detected; for truecolor, the outer terminal and tmux/screen config must both support RGB");
            } else {
                s.advice("256-color terminal detected; set COLORTERM=truecolor only if truecolor output is desired");
            }
        } else if effective == ColorMode::Color16 {
            s.advice("limited color terminal detected; try --colormode 256 or a truecolor-capable terminal");
        }
        if ssh {
            s.advice(
                "SSH detected; remote TERM/COLORTERM should match the local terminal capability",
            );
        }
        if tmux && effective == ColorMode::TrueColor {
            s.advice("tmux/screen detected; if colors look wrong, verify the outer terminal and multiplexer truecolor settings");
        }

        // Re-check unicode usage for advice
        let cs = match charset_from_str(&normalize_charset_preset_name(&args.charset), def_ascii) {
            Ok(v) => v,
            Err(_) => Charset::NONE,
        };
        let uses_katakana = cs.contains(Charset::KATAKANA);
        let uses_unicode = uses_katakana
            || cs.contains(Charset::GREEK)
            || cs.contains(Charset::CYRILLIC)
            || cs.contains(Charset::HEBREW)
            || cs.contains(Charset::BRAILLE)
            || cs.contains(Charset::RUNIC)
            || cs.contains(Charset::SYMBOLS)
            || cs.contains(Charset::ARROWS)
            || cs.contains(Charset::BLOCKS)
            || cs.contains(Charset::BOXDRAW)
            || cs.contains(Charset::MINIMAL);

        if uses_unicode {
            s.advice("selected charset uses unicode glyphs; if you see \u{25A1}\u{25A1}, change your terminal font");
            if uses_katakana {
                s.advice("font suggestions (CJK): Noto Sans CJK JP, Source Han Sans, IPAexGothic");
            } else {
                s.advice("font suggestions: Noto Sans Mono, DejaVu Sans Mono");
            }
        }

        #[cfg(target_os = "linux")]
        {
            if env_var_truthy("COSMOSTRIX_NO_FORK_GUARD") {
                s.advice("fork-based SIGKILL terminal guard is disabled; SIGKILL (-9) may leave your terminal broken");
            }
        }
        s.advice(
            "lifecycle contract: see docs/TERMINAL_LIFECYCLE_MATRIX.md for all 14 terminal paths",
        );

        // If no advice was added, add the all-clear
        if !s.has_advice() {
            s.advice("no issues detected");
        }
    }

    r.print();
}

#[must_use]
fn terminal_family(term: &str) -> &'static str {
    let term = term.to_ascii_lowercase();
    if term.is_empty() || term == "dumb" {
        "dumb/unknown"
    } else if term.contains("tmux") {
        "tmux"
    } else if term.contains("screen") {
        "screen"
    } else if term == "xterm-direct" || term.ends_with("-direct") {
        "xterm-direct"
    } else if term.contains("256color") {
        "xterm-256color"
    } else {
        "dumb/unknown"
    }
}

#[must_use]
fn color_capability(mode: ColorMode) -> &'static str {
    match mode {
        ColorMode::TrueColor => "truecolor",
        ColorMode::Color256 => "256-color",
        ColorMode::Color16 => "16-color/mono",
        ColorMode::Mono => "16-color/mono",
    }
}

#[must_use]
fn background_guidance(color_bg: ColorBg) -> &'static str {
    match color_bg {
        ColorBg::Black => "black paints solid black",
        ColorBg::Transparent => "transparent follows terminal emulator background",
        ColorBg::DefaultBackground => "default-background uses terminal default background",
    }
}

#[must_use]
fn environment_hints(
    term: &str,
    colorterm: &str,
    locale_utf8: bool,
    tmux: bool,
    ssh: bool,
    headless: bool,
) -> Vec<&'static str> {
    let mut hints = Vec::new();
    let family = terminal_family(term);
    if tmux || family == "tmux" {
        hints.push("tmux detected");
    }
    if family == "screen" {
        hints.push("screen detected");
    }
    if ssh {
        hints.push("ssh detected");
    }
    if headless {
        hints.push("headless/non-TTY detected");
    }
    if colorterm.trim().is_empty() {
        hints.push("COLORTERM missing");
    }
    if !locale_utf8 {
        hints.push("locale not UTF-8");
    }
    hints
}

#[must_use]
fn should_advise_truecolor(term: &str, colorterm: &str, effective: ColorMode) -> bool {
    effective == ColorMode::Color256
        && colorterm.trim().is_empty()
        && terminal_family(term) != "xterm-direct"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_family_detects_common_terms() {
        assert_eq!(terminal_family("xterm-direct"), "xterm-direct");
        assert_eq!(terminal_family("xterm-256color"), "xterm-256color");
        assert_eq!(terminal_family("tmux-256color"), "tmux");
        assert_eq!(terminal_family("screen-256color"), "screen");
        assert_eq!(terminal_family("dumb"), "dumb/unknown");
    }

    #[test]
    fn doctor_guidance_distinguishes_truecolor_and_256_color() {
        assert_eq!(color_capability(ColorMode::TrueColor), "truecolor");
        assert_eq!(color_capability(ColorMode::Color256), "256-color");
        assert!(should_advise_truecolor(
            "xterm-256color",
            "",
            ColorMode::Color256
        ));
        assert!(!should_advise_truecolor(
            "xterm-direct",
            "",
            ColorMode::TrueColor
        ));
    }

    #[test]
    fn doctor_background_guidance_mentions_modes() {
        assert_eq!(
            background_guidance(ColorBg::Transparent),
            "transparent follows terminal emulator background"
        );
        assert_eq!(
            background_guidance(ColorBg::Black),
            "black paints solid black"
        );
        assert_eq!(
            background_guidance(ColorBg::DefaultBackground),
            "default-background uses terminal default background"
        );
    }

    #[test]
    fn doctor_environment_hints_are_actionable() {
        let hints = environment_hints("tmux-256color", "", false, true, true, true);
        assert!(hints.contains(&"tmux detected"));
        assert!(hints.contains(&"ssh detected"));
        assert!(hints.contains(&"headless/non-TTY detected"));
        assert!(hints.contains(&"COLORTERM missing"));
        assert!(hints.contains(&"locale not UTF-8"));
    }
}
