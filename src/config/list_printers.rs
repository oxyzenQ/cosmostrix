// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! List-printer helpers — extracted from `config/mod.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns 4 CLI discovery output functions:
//! - `print_list_charsets()`: `--list-charsets` output.
//! - `print_list_colors()`: `--list-colors` output.
//! - `print_list_scenes()`: `--list-scenes` output.
//! - `print_show_scene(name)`: `--show-scene <NAME>` output.
//!
//! Re-exported from `config/mod.rs` via `pub(crate) use` so all
//! existing `crate::config::{print_list_*, print_show_scene}` call
//! sites resolve unchanged.

use super::color_enabled_stdout;

// List printers — clean, no alias noise

pub(crate) fn print_list_charsets() {
    if color_enabled_stdout() {
        println!(
            "{}AVAILABLE CHARSET PRESETS:{}",
            crate::output::brand_bold_open(),
            crate::output::reset()
        );
    } else {
        println!("AVAILABLE CHARSET PRESETS:");
    }
    println!();
    println!("  auto         Auto-select (ASCII_SAFE when non-UTF, otherwise matrix)");
    println!("  matrix       Letters + digits + katakana");
    println!("  ascii        Letters + digits + punctuation");
    println!("  extended     Digits + punctuation + katakana");
    println!("  english      Letters only");
    println!("  digits       Digits only");
    println!("  punc         Punctuation only");
    println!("  binary       0 and 1");
    println!("  hex          0-9 and A-F");
    println!("  katakana     Katakana");
    println!("  greek        Greek");
    println!("  cyrillic     Cyrillic");
    println!("  hebrew       Hebrew");
    println!("  blocks       Block elements");
    println!("  symbols      Math / technical symbols");
    println!("  arrows       Arrow symbols");
    println!("  retro        Box-drawing characters");
    println!("  cyberpunk    Katakana + hex + symbols");
    println!("  hacker       Letters + hex + punctuation + symbols");
    println!("  minimal      Single nabla glyph (∇) — one shape, pure gradient rain");
    println!("  code         Letters + digits + punctuation + symbols");
    println!("  dna          DNA bases (ACGT)");
    println!("  braille      Braille");
    println!("  runic        Runic");
    println!("  zen          Pipe character only (default for cinematic & monolith)");
    println!();
    println!("  Or define a custom charset in config.toml via [charset-custom.<name>] (see --dump-config).");

    // v25: Show custom charsets from config (if any).
    let cfg = crate::configfile::load_config_file(None);
    let custom_charsets = crate::charset_custom::collect_charset_custom(&cfg);
    if !custom_charsets.is_empty() {
        println!();
        if color_enabled_stdout() {
            println!(
                "{}CUSTOM CHARACTER SETS (from config):{}",
                crate::output::brand_bold_open(),
                crate::output::reset()
            );
        } else {
            println!("CUSTOM CHARACTER SETS (from config):");
        }
        println!();
        for (name, def) in &custom_charsets {
            println!("  {name:<20} {} chars", def.chars.len());
        }
        println!();
        println!("  Load with: cosmostrix -C/--charset/--charset-custom <name>");
        println!("  Or set in config: charset = \"<name>\"");
    }
}

pub(crate) fn print_list_colors() {
    if color_enabled_stdout() {
        println!(
            "{}AVAILABLE COLOR THEMES:{}",
            crate::output::brand_bold_open(),
            crate::output::reset()
        );
    } else {
        println!("AVAILABLE COLOR THEMES:");
    }
    println!();
    print!("{}", crate::theme::compact_list_text());
    println!();
    println!("{} built-in themes.", crate::theme::theme_count());

    // v16: Show custom color palettes from config (if any).
    let cfg = crate::configfile::load_config_file(None);
    let custom_palettes = crate::colors_custom::collect_colors_custom(&cfg);
    if !custom_palettes.is_empty() {
        println!();
        if color_enabled_stdout() {
            println!(
                "{}CUSTOM COLOR PALETTES (from config):{}",
                crate::output::brand_bold_open(),
                crate::output::reset()
            );
        } else {
            println!("CUSTOM COLOR PALETTES (from config):");
        }
        println!();
        for name in custom_palettes.keys() {
            println!("  {name:<20} custom palette");
        }
        println!();
        println!("  Load with: cosmostrix -c/--color/--colors-custom <name>");
        println!("  Use in ambient: ambient.HH-MM = <name>");
    }
}

pub(crate) fn print_list_scenes() {
    if color_enabled_stdout() {
        println!(
            "{}AVAILABLE SCENES:{}",
            crate::output::brand_bold_open(),
            crate::output::reset()
        );
    } else {
        println!("AVAILABLE SCENES:");
    }
    println!();
    print!("{}", crate::scene::list_scenes_text());

    // Append custom scenes from config (if any) under a separate heading.
    let cfg = crate::configfile::load_config_file(None);
    let custom_scenes = crate::scene_custom::collect_custom_scenes(&cfg);
    if !custom_scenes.is_empty() {
        println!();
        if color_enabled_stdout() {
            println!(
                "{}CUSTOM SCENES (from config):{}",
                crate::output::brand_bold_open(),
                crate::output::reset()
            );
        } else {
            println!("CUSTOM SCENES (from config):");
        }
        println!();
        print!(
            "{}",
            crate::scene_custom::list_custom_scenes_text(&custom_scenes)
        );
        println!();
        println!("  Load with: cosmostrix --scene-custom <name>");
    }
}

/// Print details for a single scene by name. Looks up built-in scenes first,
/// then custom scenes from config. Returns `Ok(())` on success or an error
/// message suitable for `ux::die_config`.
pub(crate) fn print_show_scene(
    name: &str,
    cfg: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    // 1. Built-in scene lookup.
    if let Some(info) = crate::scene::get_scene(name) {
        print!("{}", crate::scene::show_scene_text(info));
        return Ok(());
    }

    // 2. Custom scene lookup (scene-custom namespace only — removed
    //    the [profile.<name>] fallback; users must rename the prefix).
    let custom_scenes = crate::scene_custom::collect_custom_scenes(cfg);
    let normalized = name.trim().to_ascii_lowercase();
    if let Some(custom) = custom_scenes.get(&normalized) {
        print!(
            "{}",
            crate::scene_custom::show_custom_scene_text(&normalized, custom)
        );
        return Ok(());
    }

    // 3. Not found.
    let mut available: Vec<String> = crate::scene::all_scene_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    available.extend(custom_scenes.keys().cloned());
    available.sort();
    available.dedup();
    let list = if available.is_empty() {
        "<none defined>".to_string()
    } else {
        available.join(", ")
    };
    Err(format!(
        "error: unknown scene '{name}'\n\n  Available: {list}\n  Use --list-scenes to see all scenes."
    ))
}

// --help: curated full reference manual
//
// Design principle: guide, don't dump. No embedded catalogs, no schema dumps,
// no verbose alias disclosures. Discovery commands handle discovery.
//
// print_help() lives in src/cli/help_detail.rs.
