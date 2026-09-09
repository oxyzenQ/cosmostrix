// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Template themed-preset regression tests (owner directive
//! 2026-09-10: return the cyberpunk_2077 / quantum / tron_legacy
//! presets the NIGHT-quality-1 template rewrite had cut).
//!
//! Two halves are pinned:
//! - The dump-config template text carries the three themed blocks
//!   (scene-custom, colors-custom, charset-custom) verbatim — a
//!   future template sweep that drops them fails loudly here.
//! - The preset values actually LOAD: the scene blocks pass the
//!   seven-dimension contract end-to-end through --scene-custom,
//!   and each charset set parses to its full glyph list (the
//!   single-width filter keeps every glyph of all three sets).

#![cfg(test)]

use super::args_with_config;
use crate::configfile::dump_config_text;
use crate::scene::charset_custom::parse_charset_value;

const QUANTUM_SET: &str = "∀∃∄∅∈∉∋∌∏∑∫∂∆∇√∞≈≠≤≥±∓×÷⊕⊗⊘⊙⊚⊛⊜⊝⊞⊟⊠⊡⊢⊣⊤⊥⊦⊧⊨⊩⊪⊫⊬⊭⊮⊯";
const CYBERPUNK_SET: &str = "0123456789ABCDEF<>{}[]|=+*ｱｲｳｴｵﾊﾋﾌﾍﾎﾏ";
const TRON_SET: &str = "0123456789ABCDEF←→↑↓█▌▐░▒▓│─┤├┬┴┼";

const TEMPLATE_PRESETS_CONFIG: &str = "\
[scene-custom.cyberpunk_2077]
rain = \"monolith\"
colors-custom = \"cyberpunk_2077\"
charset-custom = \"cyberpunk_2077\"
fps = 90
speed = 12
density = 0.90
glitch-level = \"none\"

[scene-custom.tron_legacy]
rain = \"flux\"
colors-custom = \"tron_legacy\"
charset-custom = \"tron_legacy\"
fps = 75
speed = 8
density = 0.70
glitch-level = \"subtle\"

[colors-custom.cyberpunk_2077]
bg = \"#0A0008\"
rain = [\"#FFE100\", \"#FF6B00\", \"#FF0066\", \"#FF00CC\", \"#CC00FF\", \"#00FFFF\", \"#E0E0E0\"]

[colors-custom.tron_legacy]
bg = \"#02080C\"
rain = [\"#002B4D\", \"#0066AA\", \"#00BBEE\", \"#22DDFF\", \"#88EEFF\", \"#CCF4FF\", \"#FFFFFF\"]

[charset-custom.quantum]
set = \"∀∃∄∅∈∉∋∌∏∑∫∂∆∇√∞≈≠≤≥±∓×÷⊕⊗⊘⊙⊚⊛⊜⊝⊞⊟⊠⊡⊢⊣⊤⊥⊦⊧⊨⊩⊪⊫⊬⊭⊮⊯\"

[charset-custom.cyberpunk_2077]
set = \"0123456789ABCDEF<>{}[]|=+*ｱｲｳｴｵﾊﾋﾌﾍﾎﾏ\"

[charset-custom.tron_legacy]
set = \"0123456789ABCDEF←→↑↓█▌▐░▒▓│─┤├┬┴┼\"
";

#[test]
fn dump_config_carries_the_three_themed_presets() {
    let dump = dump_config_text();
    for anchor in [
        "[scene-custom.cyberpunk_2077]",
        "[scene-custom.tron_legacy]",
        "[colors-custom.cyberpunk_2077]",
        "[colors-custom.tron_legacy]",
        "[charset-custom.quantum]",
        "[charset-custom.cyberpunk_2077]",
        "[charset-custom.tron_legacy]",
    ] {
        assert!(
            dump.contains(anchor),
            "the dump-config template must carry {anchor}"
        );
    }
}

#[test]
fn template_cyberpunk_2077_scene_loads_end_to_end() {
    // The block is a COMPLETE profile (the seven-dimension contract):
    // loading it through --scene-custom must resolve every field —
    // the palette reference, the charset reference, and the numeric
    // quartet. A preset that only LOOKS complete fails at load.
    let args = args_with_config(
        TEMPLATE_PRESETS_CONFIG,
        &["--scene-custom", "cyberpunk_2077"],
    );
    assert_eq!(args.fps, 90.0);
    assert_eq!(args.speed, 12.0);
    assert_eq!(args.density, 0.90);
    assert_eq!(args.charset, "cyberpunk_2077");
    assert_eq!(args.colors_custom.as_deref(), Some("cyberpunk_2077"));
}

#[test]
fn template_tron_legacy_scene_loads_end_to_end() {
    let args = args_with_config(TEMPLATE_PRESETS_CONFIG, &["--scene-custom", "tron_legacy"]);
    assert_eq!(args.fps, 75.0);
    assert_eq!(args.speed, 8.0);
    assert_eq!(args.density, 0.70);
    assert_eq!(args.charset, "tron_legacy");
    assert_eq!(args.colors_custom.as_deref(), Some("tron_legacy"));
}

#[test]
fn template_charset_sets_parse_to_their_glyphs() {
    // The single-width filter must keep every glyph of all three
    // sets (math symbols, half-width katakana, box-drawing) — the
    // wide/zero-width skip is the only legal way for a set to shrink,
    // and none of these glyphs are wide.
    let quantum = parse_charset_value(QUANTUM_SET).expect("the quantum math-symbol set must parse");
    assert_eq!(quantum.len(), QUANTUM_SET.chars().count());
    assert!(quantum.contains(&'∀'));
    assert!(quantum.contains(&'√'));

    let cyberpunk =
        parse_charset_value(CYBERPUNK_SET).expect("the cyberpunk_2077 hex+katakana set must parse");
    assert_eq!(cyberpunk.len(), CYBERPUNK_SET.chars().count());
    assert!(cyberpunk.contains(&'ｱ'));

    let tron =
        parse_charset_value(TRON_SET).expect("the tron_legacy hex+box-drawing set must parse");
    assert_eq!(tron.len(), TRON_SET.chars().count());
    assert!(tron.contains(&'█'));
    assert!(tron.contains(&'┼'));
}
