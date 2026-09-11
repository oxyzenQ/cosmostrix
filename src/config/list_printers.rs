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
use crate::output::println_safe;
use std::collections::HashMap;

// List printers — clean, no alias noise

/// NIGHT-depthtest-3 (owner repro 2026-09-11): collect the block
/// names a custom-block collector silently drops for exceeding its
/// name-length cap (64 chars, all three namespaces).
///
/// The list printers are non-strict by design (they must list even
/// when unrelated keys are broken), but silent invisibility is the
/// defect the owner hit: a complete `[scene-custom.<name>]` block
/// with a 65+-char name never listed AND `--testconf` passed. Each
/// printer now appends a visible warning line naming the hidden
/// blocks so the user gets a signal without running --testconf.
///
/// Only keys shaped like the collector accepts are scanned (known
/// field under the namespace); malformed keys keep their own error
/// paths. Sorted + deduplicated: stable output across hash seeds.
fn oversized_custom_block_names(
    cfg: &HashMap<String, String>,
    prefix: &str,
    fields: &[&str],
    limit: usize,
) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for key in cfg.keys() {
        let Some(rest) = key.strip_prefix(prefix) else {
            continue;
        };
        let Some((name, field)) = rest.rsplit_once('.') else {
            continue;
        };
        if fields.contains(&field) && name.len() > limit {
            let name = name.to_string();
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names.sort();
    names
}

/// NIGHT-depthtest-3: render the hidden-block warning lines shared by
/// all three list printers (one per oversized name).
fn hidden_block_warning_lines(prefix: &str, hidden: &[String], limit: usize) -> Vec<String> {
    hidden
        .iter()
        .map(|name| {
            let display: String = name.chars().take(24).collect();
            let suffix = if name.chars().count() > 24 { "..." } else { "" };
            format!(
                "  hidden: {prefix}'{display}{suffix}' is {} chars — exceeds the {limit}-char name limit; the block is dropped. Shorten the name (run --testconf).",
                name.chars().count()
            )
        })
        .collect()
}

pub(crate) fn print_list_charsets() {
    if color_enabled_stdout() {
        println_safe!(
            "{}AVAILABLE CHARSET PRESETS:{}",
            crate::output::brand_bold_open(),
            crate::output::reset()
        );
    } else {
        println_safe!("AVAILABLE CHARSET PRESETS:");
    }
    println_safe!();
    println_safe!("  auto         Auto-select (ASCII_SAFE when non-UTF, otherwise matrix)");
    println_safe!("  matrix       Letters + digits + katakana");
    println_safe!("  ascii        Letters + digits + punctuation");
    println_safe!("  extended     Digits + punctuation + katakana");
    println_safe!("  english      Letters only");
    println_safe!("  digits       Digits only");
    println_safe!("  punc         Punctuation only");
    println_safe!("  binary       0 and 1");
    println_safe!("  hex          0-9 and A-F");
    println_safe!("  katakana     Katakana");
    println_safe!("  greek        Greek");
    println_safe!("  cyrillic     Cyrillic");
    println_safe!("  hebrew       Hebrew");
    println_safe!("  blocks       Block elements");
    println_safe!("  symbols      Math / technical symbols");
    println_safe!("  arrows       Arrow symbols");
    println_safe!("  retro        Box-drawing characters");
    println_safe!("  cyberpunk    Katakana + hex + symbols");
    println_safe!("  hacker       Letters + hex + punctuation + symbols");
    println_safe!("  minimal      Single nabla glyph (∇) — one shape, pure gradient rain");
    println_safe!("  code         Letters + digits + punctuation + symbols");
    println_safe!("  dna          DNA bases (ACGT)");
    println_safe!("  braille      Braille");
    println_safe!("  runic        Runic");
    println_safe!("  zen          Pipe character only (default for cinematic & monolith)");
    println_safe!();
    println_safe!("  Or define a custom charset in config.toml via [charset-custom.<name>] (see --dump-config).");

    // v25: Show custom charsets from config (if any).
    let cfg = crate::configfile::load_config_file(None);
    let custom_charsets = crate::charset_custom::collect_charset_custom(&cfg);
    // NIGHT-depthtest-3: charset blocks hidden by the name-length cap
    // (the collector drops them before this listing ever runs).
    let hidden = oversized_custom_block_names(
        &cfg,
        "charset-custom.",
        &["set"],
        crate::charset_custom::CHARSET_CUSTOM_MAX_NAME_LEN,
    );
    if !custom_charsets.is_empty() || !hidden.is_empty() {
        println_safe!();
        if color_enabled_stdout() {
            println_safe!(
                "{}CUSTOM CHARACTER SETS (from config):{}",
                crate::output::brand_bold_open(),
                crate::output::reset()
            );
        } else {
            println_safe!("CUSTOM CHARACTER SETS (from config):");
        }
        println_safe!();
        for (name, def) in &custom_charsets {
            println_safe!("  {name:<20} {} chars", def.chars.len());
        }
        for line in hidden_block_warning_lines(
            "charset-custom.",
            &hidden,
            crate::charset_custom::CHARSET_CUSTOM_MAX_NAME_LEN,
        ) {
            println_safe!("{line}");
        }
        println_safe!();
        println_safe!("  Load with: cosmostrix -C/--charset/--charset-custom <name>");
        println_safe!("  Or set in config: charset = \"<name>\"");
    }
}

pub(crate) fn print_list_colors() {
    if color_enabled_stdout() {
        println_safe!(
            "{}AVAILABLE COLOR THEMES:{}",
            crate::output::brand_bold_open(),
            crate::output::reset()
        );
    } else {
        println_safe!("AVAILABLE COLOR THEMES:");
    }
    println_safe!();
    print!("{}", crate::theme::compact_list_text());
    println_safe!();
    println_safe!("{} built-in themes.", crate::theme::theme_count());

    // v16: Show custom color palettes from config (if any).
    let cfg = crate::configfile::load_config_file(None);
    let custom_palettes = crate::colors_custom::collect_colors_custom(&cfg);
    // NIGHT-depthtest-3: palette blocks hidden by the name-length cap.
    let hidden = oversized_custom_block_names(
        &cfg,
        "colors-custom.",
        &["bg", "rain", "stops"],
        crate::colors_custom::COLORS_CUSTOM_MAX_NAME_LEN,
    );
    if !custom_palettes.is_empty() || !hidden.is_empty() {
        println_safe!();
        if color_enabled_stdout() {
            println_safe!(
                "{}CUSTOM COLOR PALETTES (from config):{}",
                crate::output::brand_bold_open(),
                crate::output::reset()
            );
        } else {
            println_safe!("CUSTOM COLOR PALETTES (from config):");
        }
        println_safe!();
        for name in custom_palettes.keys() {
            println_safe!("  {name:<20} custom palette");
        }
        for line in hidden_block_warning_lines(
            "colors-custom.",
            &hidden,
            crate::colors_custom::COLORS_CUSTOM_MAX_NAME_LEN,
        ) {
            println_safe!("{line}");
        }
        println_safe!();
        println_safe!("  Load with: cosmostrix -c/--color/--colors-custom <name>");
        println_safe!("  Use in ambient: ambient.HH-MM = <name>");
    }
}

pub(crate) fn print_list_scenes() {
    if color_enabled_stdout() {
        println_safe!(
            "{}AVAILABLE SCENES:{}",
            crate::output::brand_bold_open(),
            crate::output::reset()
        );
    } else {
        println_safe!("AVAILABLE SCENES:");
    }
    println_safe!();
    print!("{}", crate::scene::list_scenes_text());

    // Append custom scenes from config (if any) under a separate heading.
    let cfg = crate::configfile::load_config_file(None);
    let custom_scenes = crate::scene_custom::collect_custom_scenes(&cfg);
    // NIGHT-depthtest-3: scene blocks hidden by the name-length cap —
    // the owner's --list-scenes repro: a 65+-char name never listed
    // while 4-char names listed fine. The section now prints (with a
    // warning) even when the collected map is empty and every defined
    // block is hidden.
    let hidden = oversized_custom_block_names(
        &cfg,
        "scene-custom.",
        crate::scene_custom::SCENE_CUSTOM_FIELDS,
        crate::scene_custom::SCENE_CUSTOM_MAX_NAME_LEN,
    );
    if !custom_scenes.is_empty() || !hidden.is_empty() {
        println_safe!();
        if color_enabled_stdout() {
            println_safe!(
                "{}CUSTOM SCENES (from config):{}",
                crate::output::brand_bold_open(),
                crate::output::reset()
            );
        } else {
            println_safe!("CUSTOM SCENES (from config):");
        }
        println_safe!();
        print!(
            "{}",
            crate::scene_custom::list_custom_scenes_text(&custom_scenes)
        );
        for line in hidden_block_warning_lines(
            "scene-custom.",
            &hidden,
            crate::scene_custom::SCENE_CUSTOM_MAX_NAME_LEN,
        ) {
            println_safe!("{line}");
        }
        println_safe!();
        println_safe!("  Load with: cosmostrix --scene <name> or --scene-custom <name>");
        println_safe!("  Or set in config: scene = \"<name>\"");
    }
}

/// Print details for a single scene by name. Looks up built-in scenes first,
/// then custom scenes from config. Returns `Ok(())` on success or an error
/// message suitable for `ux::die_input` (CLI value error: footer family).
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
    // v100.0.0-nightly.1 footer-consistency sweep (owner report
    // 2026-09-04): the unknown-scene error now carries the same
    // did-you-mean tip the --scene path renders (shared
    // scene_suggestion_tip engine) — before, --show-scene cosmosm
    // dead-ended with the bare list while --scene cosmosm suggested
    // 'cosmos'.
    let tip = super::config_apply::scene_suggestion_tip(&normalized, cfg);
    // NIGHT-depthtest-3: over-limit names can never match a custom
    // block — surface the limit (same note the --scene path renders).
    let limit_note = super::config_apply::scene_length_limit_note(&normalized);
    Err(format!(
        "error: unknown scene '{name}'{tip}{limit_note}\n\n  Available: {list}\n  Use --list-scenes to see all scenes."
    ))
}

// --help: curated full reference manual
//
// Design principle: guide, don't dump. No embedded catalogs, no schema dumps,
// no verbose alias disclosures. Discovery commands handle discovery.
//
// print_help() lives in src/cli/help_detail.rs.

#[cfg(test)]
mod tests {
    use super::print_show_scene;

    /// v100.0.0-nightly.1 footer-consistency sweep: an unknown
    /// `--show-scene` name must carry the SAME did-you-mean tip the
    /// `--scene` path renders (shared scene_suggestion_tip engine), so
    /// one typo shape covers both surfaces.
    #[test]
    fn show_scene_unknown_name_carries_suggestion_tip() {
        let cfg = std::collections::HashMap::new();
        let err = print_show_scene("cosmosm", &cfg).unwrap_err();
        assert!(
            err.contains("error: unknown scene 'cosmosm'"),
            "must name the unknown scene: {err}"
        );
        assert!(
            err.contains("tip: a similar value exists: 'cosmos'"),
            "must suggest 'cosmos' for 'cosmosm': {err}"
        );
        assert!(
            err.contains("Use --list-scenes to see all scenes."),
            "must keep the --list-scenes guidance: {err}"
        );
    }

    /// Distant names stay tip-free (edit-distance policy) but keep the
    /// available-scenes list, which is the query surface's value.
    #[test]
    fn show_scene_distant_name_keeps_list_without_tip() {
        let cfg = std::collections::HashMap::new();
        let err = print_show_scene("zzzzzzzz", &cfg).unwrap_err();
        assert!(!err.contains("tip:"), "no tip for a distant name: {err}");
        assert!(err.contains("Available:"), "list must survive: {err}");
    }

    /// Known builtin scene resolves Ok (guards the tip change against
    /// breaking the success path).
    #[test]
    fn show_scene_known_builtin_is_ok() {
        let cfg = std::collections::HashMap::new();
        assert!(print_show_scene("cosmos", &cfg).is_ok());
    }
    // ── NIGHT-depthtest-3: hidden-block visibility helpers ─────────

    #[test]
    fn oversized_block_names_sorted_and_deduped() {
        use super::{hidden_block_warning_lines, oversized_custom_block_names};
        use std::collections::HashMap;
        let mut cfg = HashMap::new();
        let b = "b".repeat(70);
        let a = "a".repeat(70);
        // two keys under the same name (multiple fields) + one other
        cfg.insert(format!("scene-custom.{b}.rain"), "glyph".to_string());
        cfg.insert(format!("scene-custom.{b}.fps"), "90".to_string());
        cfg.insert(format!("scene-custom.{a}.color"), "red".to_string());
        cfg.insert("scene-custom.ok.rain".to_string(), "glyph".to_string());
        let names = oversized_custom_block_names(
            &cfg,
            "scene-custom.",
            crate::scene_custom::SCENE_CUSTOM_FIELDS,
            crate::scene_custom::SCENE_CUSTOM_MAX_NAME_LEN,
        );
        assert_eq!(names, vec![a.clone(), b.clone()], "sorted + deduped");
        let lines = hidden_block_warning_lines("scene-custom.", &names, 64);
        assert_eq!(lines.len(), 2);
        assert!(
            lines[0].contains("hidden: scene-custom"),
            "line must be a hidden-block warning: {0}",
            lines[0]
        );
        assert!(
            lines[0].contains("exceeds the 64-char name limit"),
            "line must name the limit: {0}",
            lines[0]
        );
    }

    #[test]
    fn oversized_block_names_empty_when_all_within_limit() {
        use super::oversized_custom_block_names;
        use std::collections::HashMap;
        let cfg = HashMap::from([("scene-custom.ok.rain".to_string(), "glyph".to_string())]);
        assert!(oversized_custom_block_names(
            &cfg,
            "scene-custom.",
            crate::scene_custom::SCENE_CUSTOM_FIELDS,
            crate::scene_custom::SCENE_CUSTOM_MAX_NAME_LEN
        )
        .is_empty());
    }
}
