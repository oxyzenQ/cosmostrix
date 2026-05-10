// Copyright (c) 2026 rezky_nightky

use std::env;

use crate::charset::{charset_from_str, Charset};
use crate::config::Args;
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

    println!("DOCTOR REPORT:");
    println!("  stdin_is_tty: {}", if stdin_tty { "yes" } else { "no" });
    println!("  stdout_is_tty: {}", if stdout_tty { "yes" } else { "no" });

    println!(
        "  LANG: {}",
        if lang.is_empty() { "(unset)" } else { &lang }
    );
    println!(
        "  LC_ALL: {}",
        if lc_all.is_empty() {
            "(unset)"
        } else {
            &lc_all
        }
    );
    println!(
        "  LC_CTYPE: {}",
        if lc_ctype.is_empty() {
            "(unset)"
        } else {
            &lc_ctype
        }
    );
    println!("  locale_utf8: {}", if locale_utf8 { "yes" } else { "no" });

    println!(
        "  TERM: {}",
        if term.is_empty() { "(unset)" } else { &term }
    );
    println!(
        "  COLORTERM: {}",
        if colorterm.is_empty() {
            "(unset)"
        } else {
            &colorterm
        }
    );

    #[cfg(target_os = "linux")]
    {
        let no_fork_guard = env_var_truthy("COSMOSTRIX_NO_FORK_GUARD");
        println!(
            "  fork_guard: {}",
            if no_fork_guard {
                "disabled (COSMOSTRIX_NO_FORK_GUARD)"
            } else {
                "enabled"
            }
        );
    }

    println!("  color_auto_detected: {}", color_mode_label(auto));
    if args.colormode.is_some() {
        println!("  color_forced: {}", color_mode_label(effective));
    }
    println!("  color_effective: {}", color_mode_label(effective));

    let def_ascii = default_to_ascii();
    println!(
        "  default_to_ascii: {}",
        if def_ascii { "yes" } else { "no" }
    );

    let charset_preset = normalize_charset_preset_name(&args.charset);
    println!(
        "  charset: {}",
        if args.charset.is_empty() {
            "(empty)"
        } else {
            &args.charset
        }
    );
    if charset_preset != args.charset {
        println!("  charset_normalized: {}", charset_preset);
    }
    if let Some(spec) = &args.chars {
        println!("  chars_override: {}", spec);
    }

    let cs = match charset_from_str(&charset_preset, def_ascii) {
        Ok(v) => v,
        Err(e) => {
            println!("  charset_parse_error: {}", e);
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

    if locale_utf8 {
        println!();
        println!("SAMPLE GLYPHS:");
        println!("  ascii: 01 ABC abc !@#");
        if uses_katakana {
            println!("  katakana: \u{FF71}\u{FF72}\u{FF73}\u{FF74}\u{FF75}\u{FF76}\u{FF77}\u{FF78}\u{FF79}\u{FF7A}");
        }
        if cs.contains(Charset::GREEK) {
            println!("  greek: \u{03A9}\u{03BB}\u{03C0}\u{0394}");
        }
        if cs.contains(Charset::CYRILLIC) {
            println!("  cyrillic: \u{042F}\u{0416}\u{042E}\u{0428}");
        }
        if cs.contains(Charset::HEBREW) {
            println!("  hebrew: \u{05D0}\u{05D1}\u{05D2}\u{05D3}");
        }
        if cs.contains(Charset::BRAILLE) {
            println!("  braille: \u{28FF}\u{28F7}\u{28EF}\u{28DF}");
        }
        if cs.contains(Charset::RUNIC) {
            println!("  runic: \u{16A0}\u{16A2}\u{16A6}\u{16A8}");
        }
        if cs.contains(Charset::SYMBOLS) {
            println!("  symbols: \u{221E}\u{2211}\u{222B}\u{221A}\u{03C0}");
        }
        if cs.contains(Charset::ARROWS) {
            println!("  arrows: \u{2190}\u{2192}\u{2191}\u{2193}");
        }
        if cs.contains(Charset::BLOCKS) {
            println!("  blocks: \u{2591}\u{2592}\u{2593}\u{2588}");
        }
        if cs.contains(Charset::BOXDRAW) {
            println!("  boxdraw: \u{250C}\u{2510}\u{2514}\u{2518}\u{2500}\u{2502}");
        }
        if cs.contains(Charset::MINIMAL) {
            println!("  minimal: \u{00B7}\u{2022}\u{25CB}\u{25CF}\u{25C7}\u{25C6}");
        }
    }

    println!();
    println!("ADVICE:");
    let mut printed = false;
    if !stdin_tty || !stdout_tty {
        println!("  - run cosmostrix directly in a terminal (avoid piping/redirect)");
        printed = true;
    }
    if !locale_utf8 {
        println!("  - locale does not look like UTF-8; unicode charsets may render incorrectly");
        println!("    try: export LANG=en_US.UTF-8");
        printed = true;
    }
    if effective != ColorMode::TrueColor {
        println!("  - for best colors use a truecolor terminal (COLORTERM=truecolor)");
        printed = true;
    }
    if uses_unicode {
        println!(
            "  - selected charset uses unicode glyphs; if you see \u{25A1}\u{25A1}, change your terminal font"
        );
        if uses_katakana {
            println!("    font suggestions (CJK): Noto Sans CJK JP, Source Han Sans, IPAexGothic");
        } else {
            println!("    font suggestions: Noto Sans Mono, DejaVu Sans Mono");
        }
        printed = true;
    }

    #[cfg(target_os = "linux")]
    {
        if env_var_truthy("COSMOSTRIX_NO_FORK_GUARD") {
            println!(
                "  - fork-based SIGKILL terminal guard is disabled; SIGKILL (-9) may leave your terminal broken"
            );
            printed = true;
        }
    }
    if !printed {
        println!("  - no issues detected");
    }
}
