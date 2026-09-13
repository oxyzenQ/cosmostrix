// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Custom color palette definitions from config.toml.
//!
//! One mode: `rain` gradient stops + `bg` — BOTH required since
//! NIGHT-hunt-37 (the owner complete-fields mandate: a block missing
//! either field is a hard error on every surface).
//!
//! ```toml
//! [colors-custom.sunset]
//! bg = "#0a0a12"
//! rain = "#1a0033", "#4d0080", "#9933ff", "#cc66ff", "#e6b3ff", "#f2ccff", "#ffffff"
//! ```
//!
//! Load with `--colors-custom sunset`, or reference it as
//! `color = sunset` (top-level or inside a scene-custom block — the
//! form ambient phases use, since `ambient.<HH-MM>` names a scene,
//! never a palette directly).

use std::collections::{BTreeMap, HashMap};

use crossterm::style::Color;

use crate::chroma_dragon_engine::palette::colors_from_stops;
use crate::palette::Palette;
use crate::runtime::ColorMode;

/// Number of perceptual samples produced from the user's `rain` stops via
/// the OKLab polar gradient engine. Matches the expansion applied to
/// built-in themes (`catalog.rs:933` routes through `colors_from_stops` with
/// `steps = 9`). Without this, colors-custom palettes would only carry the
/// raw user stops (typically 3-5 entries), producing visible banding on long
/// rain trails — every built-in theme avoids this by expanding to 9
/// perceptually-uniform OKLab samples.
const COLORS_CUSTOM_PALETTE_STEPS: usize = 9;

/// NIGHT-hunt-37 (owner mandate 2026-09-13): maximum number of rain
/// stops accepted in a single `[colors-custom.<name>]` block. Was 64
/// with a SILENT collector cap — the owner's 67-stop repro passed
/// every gate. Now 9, matching `COLORS_CUSTOM_PALETTE_STEPS` (the
/// count the OKLab engine resamples every palette to), and
/// over-limit blocks are a HARD error on all three validation
/// surfaces (startup, --testconf, live-reload watcher) — see
/// `strictness.rs::validate_rain_stop_contract`.
pub(crate) const COLORS_CUSTOM_MAX_RAIN_STOPS: usize = 9;

/// v50.0.0-beta.6 LTS: maximum number of custom palette blocks accepted
/// in a single config.toml. Aligned with charset-custom and scene-custom
/// (all 3 systems use 100). Bounds the BTreeMap size + iteration cost in
/// `collect_colors_custom`. 100 blocks is far beyond any realistic use
/// case (built-in themes are ~44); the cap prevents a config typo from
/// spawning hundreds of empty blocks.
pub(crate) const COLORS_CUSTOM_MAX_BLOCKS: usize = 100;

/// v50.0.0-beta.6 LTS: maximum length of a custom palette block name.
/// Bounds BTreeMap key allocation. 64 chars is generous (built-in names
/// are ≤16 chars like "fancy_diamond"); longer names are likely typos.
pub(crate) const COLORS_CUSTOM_MAX_NAME_LEN: usize = 64;

/// A parsed custom color palette definition.
#[derive(Debug, Clone, Default)]
pub(crate) struct CustomPaletteDef {
    /// Background color (required — NIGHT-hunt-37 complete-fields
    /// contract; `to_palette` rejects a definition without it).
    pub bg: Option<Color>,
    /// Gradient stops for the rain trail (tail → head order).
    pub rain: Vec<Color>,
}

impl CustomPaletteDef {
    /// Build a cosmostrix `Palette` from this definition.
    ///
    /// The user's `rain` stops are expanded through the same OKLab polar
    /// gradient engine (`chroma::gradient::gradient_from_stops_oklab`) that
    /// built-in themes use. This produces `COLORS_CUSTOM_PALETTE_STEPS`
    /// perceptually-uniform samples from the raw stops, eliminating banding
    /// on long rain trails and bringing colors-custom palettes onto equal
    /// footing with built-in themes like `synthwave`, `cosmos`, etc.
    ///
    /// Before this change, `to_palette()` returned the raw `rain` stops
    /// verbatim — a 3-stop palette stayed 3 entries while every built-in
    /// theme expanded to 9 OKLab-interpolated entries. That asymmetry was
    /// the only place colors-custom diverged from the chroma engine.
    pub(crate) fn to_palette(&self) -> Result<Palette, String> {
        if self.rain.is_empty() {
            return Err("custom palette needs 'rain' field with at least 2 hex colors".to_string());
        }
        if self.rain.len() < 2 {
            return Err("rain needs at least 2 hex colors for a gradient".to_string());
        }
        // NIGHT-hunt-37 (owner mandate 2026-09-13): bg is REQUIRED —
        // a complete [colors-custom.<name>] block defines BOTH fields.
        if self.bg.is_none() {
            return Err(
                "custom palette needs 'bg' field (hex color) — a [colors-custom.<name>] block must define BOTH bg and rain"
                    .to_string(),
            );
        }
        // Convert the user's Color stops to RGB tuples for the gradient
        // engine. All colors-custom stops are parsed from hex by
        // `parse_hex_color`, so they are always `Color::Rgb`. The
        // `color_to_rgb` helper handles the AnsiValue fallback path too.
        let stops_rgb: Vec<(u8, u8, u8)> = self
            .rain
            .iter()
            .map(|c| crate::chroma_dragon_engine::palette::color_to_rgb(*c))
            .collect();
        // TrueColor is the only mode that makes sense for colors-custom:
        // user-supplied hex colors are by definition 24-bit. Mono mode would
        // collapse everything to white, defeating the purpose.
        let colors = colors_from_stops(
            ColorMode::TrueColor,
            &stops_rgb,
            COLORS_CUSTOM_PALETTE_STEPS,
        );
        Ok(Palette {
            colors,
            bg: self.bg,
        })
    }
}

/// Parse a hex color string to a crossterm Color.
///
/// Accepts: `#rrggbb`, `rrggbb`, `#rgb`, `rgb`, `"#rrggbb"` (quoted).
pub(crate) fn parse_hex_color(s: &str) -> Result<Color, String> {
    let s = s.trim().trim_matches('"').trim();
    let s = s.strip_prefix('#').unwrap_or(s);

    if s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?;
        let g = u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?;
        let b = u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?;
        Ok(Color::Rgb { r, g, b })
    } else if s.len() == 3 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&s[0..1].repeat(2), 16).map_err(|e| e.to_string())?;
        let g = u8::from_str_radix(&s[1..2].repeat(2), 16).map_err(|e| e.to_string())?;
        let b = u8::from_str_radix(&s[2..3].repeat(2), 16).map_err(|e| e.to_string())?;
        Ok(Color::Rgb { r, g, b })
    } else {
        Err(format!(
            "invalid hex color '{s}' (expected #rrggbb or rrggbb)"
        ))
    }
}

/// Collect all custom color palette definitions from the config HashMap.
///
/// v50.0.0-beta.6 LTS: bounded by `COLORS_CUSTOM_MAX_BLOCKS` (100) and
/// `COLORS_CUSTOM_MAX_RAIN_STOPS` (64) to prevent config typos from
/// bloating memory or stalling startup. Names longer than
/// `COLORS_CUSTOM_MAX_NAME_LEN` (64 chars) are skipped by the
/// collector — and since NIGHT-depthtest-3 the skip is fronted by a
/// HARD validation error (`validate_colors_custom_name_len`), so an
/// oversized name can no longer pass `--testconf` silently.
#[must_use]
pub(crate) fn collect_colors_custom(
    cfg: &HashMap<String, String>,
) -> BTreeMap<String, CustomPaletteDef> {
    let mut palettes: BTreeMap<String, CustomPaletteDef> = BTreeMap::new();

    for (key, value) in cfg {
        let Some(rest) = key.strip_prefix("colors-custom.") else {
            continue;
        };
        let Some((name, field)) = rest.split_once('.') else {
            continue;
        };
        // v50.0.0-beta.6 LTS: skip oversized names early (before
        // to_ascii_lowercase allocates). 64 chars is generous.
        if name.len() > COLORS_CUSTOM_MAX_NAME_LEN {
            continue;
        }
        // v50.0.0-beta.6 LTS: skip if we already hit the block cap.
        // Prevents a config with hundreds of [colors-custom.X] blocks
        // from bloating the BTreeMap.
        let name_lower = name.to_ascii_lowercase();
        if palettes.len() >= COLORS_CUSTOM_MAX_BLOCKS && !palettes.contains_key(&name_lower) {
            continue;
        }
        let palette = palettes.entry(name_lower).or_default();

        match field {
            "bg" => {
                if let Ok(color) = parse_hex_color(value) {
                    palette.bg = Some(color);
                }
            }
            // (bug #8): `stops` is a deprecated alias for `rain`.
            // The validator still accepts it (with a --testconf deprecation
            // warning); the runtime parser treats it identically to `rain`.
            // (CLI-D-2): emit a one-time deprecation warning at runtime
            // too — previously only --testconf warned, so users who never
            // ran --testconf used the deprecated alias indefinitely with no
            // signal.
            // AB-10 (rain-screen cleanliness): buffer the warning to
            // `LIVE_RELOAD_RUNTIME_WARNINGS` instead of eprintln. This
            // function runs on every config save via the live-reload path,
            // and the eprintln fired while the alt screen was active,
            // leaking into the rain matrix. main.rs drains the buffer
            // AFTER Terminal::drop restores the main screen.
            "rain" | "stops" => {
                if field == "stops" {
                    crate::live_config::push_runtime_warning(
                        "colors-custom: '.stops' is deprecated — rename to '.rain' (alias removed in a future release)",
                    );
                }
                // v25 masterclass: support both CSV string and TOML array
                // format via the shared splitter (NIGHT-hunt-37: the
                // collector, --testconf's value checker, and the
                // strictness validator all split through this ONE
                // definition — the F-23-1 no-twin policy).
                let stops = strictness::split_rain_stop_entries(value);
                for stop in stops {
                    // v50.0.0-beta.6 LTS defense-in-depth cap (the
                    // validation layer hard-errors over-limit blocks
                    // first; this only fires when validation is
                    // bypassed via COSMOSTRIX_SKIP_STARTUP_VALIDATION).
                    if palette.rain.len() >= COLORS_CUSTOM_MAX_RAIN_STOPS {
                        crate::live_config::push_runtime_warning(&format!(
                            "colors-custom: rain stops capped at {COLORS_CUSTOM_MAX_RAIN_STOPS} (extra stops ignored)"
                        ));
                        break;
                    }
                    if let Ok(color) = parse_hex_color(stop) {
                        palette.rain.push(color);
                    }
                }
            }
            _ => {}
        }
    }

    palettes
}

/// Look up a custom palette by name and convert it to a cosmostrix Palette.
pub(crate) fn load_custom_palette(
    cfg: &HashMap<String, String>,
    name: &str,
) -> Result<Palette, String> {
    let palettes = collect_colors_custom(cfg);
    let normalized = name.trim().to_ascii_lowercase();
    let def = palettes.get(&normalized).ok_or_else(|| {
        let mut available: Vec<String> = palettes.keys().cloned().collect();
        available.sort();
        let list = if available.is_empty() {
            "<none defined>".to_string()
        } else {
            available.join(", ")
        };
        // v80.0.0-beta.1 did-you-mean audit: suggest the closest defined palette
        // (edit-distance <= 2, same policy as every other value surface).
        let tip = crate::cli::suggestion::closest_value_match(
            name,
            &available.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        )
        .map(|s| crate::cli::ux::format_value_suggestion(&s))
        .unwrap_or_default();
        // NIGHT-depthtest-3: a >64-char name can never match a block
        // (the collector drops oversized names) — surface the limit
        // instead of a bare "not found" that reads as a missing block.
        let limit_note = if normalized.len() > COLORS_CUSTOM_MAX_NAME_LEN {
            format!(
                "\n  note: the name is {} chars — over the {COLORS_CUSTOM_MAX_NAME_LEN}-char limit, so no [colors-custom.<name>] block can ever define it",
                normalized.chars().count()
            )
        } else {
            String::new()
        };
        format!(
            "custom color '{name}' not found in config{tip}{limit_note}\nexpected one of: {list}\n\n  Use --list-colors to see built-in and custom palettes."
        )
    })?;
    def.to_palette()
}

/// Phase 5 closure (P1-#5): check whether `name` refers to a defined
/// `[colors-custom.<name>]` block in `cfg`. Used by profile/scene-custom
/// layers to resolve custom color names (matching top-level config_apply
/// behavior which resolves via `parse_color_scheme || colors-custom lookup`).
///
/// v80.0.0-beta.1 killer-features hardening: cheap pre-check (mirrors
/// charset_custom's `contains_key` probe) — if the config defines no
/// `[colors-custom.*]` blocks at all, skip building the full BTreeMap.
/// This probe runs on every scene change and live reload via the
/// scene-custom layers, and most configs define no custom palettes.
#[must_use]
pub(crate) fn is_colors_custom_name(cfg: &HashMap<String, String>, name: &str) -> bool {
    if !cfg.keys().any(|k| k.starts_with("colors-custom.")) {
        return false;
    }
    let palettes = collect_colors_custom(cfg);
    palettes.contains_key(&name.trim().to_ascii_lowercase())
}

/// NIGHT-hunter-24 (F-24-1): the load contract for a referenced
/// `[colors-custom.<name>]` block, asked of the loader itself.
///
/// `to_palette` (the constructor every load path funnels through —
/// startup `--colors-custom`/`color =`, `intro-color =`, live reload,
/// scene-runtime ambient) hard-errors when a block carries fewer than
/// 2 parseable rain stops. Before this helper the validation layer
/// (`--testconf`, startup, live-reload watcher) checked only hex FORMAT,
/// never the stop COUNT: a bg-only / single-stop / empty-array block
/// passed every gate, then died at load (startup exit) or silently
/// no-opped (intro brand fallback, live-reload "keeping current").
/// Validation now asks the runtime constructor directly, so the two
/// layers cannot drift apart again.
///
/// Returns `None` when `name` is not a colors-custom block (the caller's
/// unknown-name path owns that case) or when the block builds a valid
/// palette; `Some(err)` carries the exact `to_palette` error.
#[must_use]
pub(crate) fn colors_custom_load_error(
    cfg: &HashMap<String, String>,
    name: &str,
) -> Option<String> {
    if !cfg.keys().any(|k| k.starts_with("colors-custom.")) {
        return None;
    }
    let normalized = name.trim().to_ascii_lowercase();
    collect_colors_custom(cfg)
        .get(&normalized)
        .and_then(|def| def.to_palette().err())
}

/// NIGHT-hunter-24 (F-24-1): every DEFINED `[colors-custom.<name>]`
/// block must satisfy the load contract — the colors-custom analogue of
/// the scene-custom completeness mandate. A block that cannot build a
/// palette is dead weight unreferenced, and a split-verdict defect when
/// referenced (testconf PASS / startup fatal / live-reload silent
/// no-op), so deficient blocks are rejected whether or not a reference
/// exists. BTreeMap iteration is sorted — the first reported block is
/// deterministic across hash seeds (same contract as every validator
/// in the layer).
#[must_use]
pub(crate) fn validate_colors_custom_blocks(cfg: &HashMap<String, String>) -> Option<String> {
    if !cfg.keys().any(|k| k.starts_with("colors-custom.")) {
        return None;
    }
    // NIGHT-depthtest-3 (owner repro 2026-09-11): oversized block
    // names are a HARD error, not a silent skip — the pre-scan lives
    // in name_len.rs (LOC cap extraction). Same blind-spot class as
    // scene-custom: the collector cap dropped the block before any
    // validation or listing ever saw it.
    if let Some(msg) = name_len::validate_colors_custom_name_len(cfg) {
        return Some(msg);
    }
    // NIGHT-hunt-37 (owner mandate 2026-09-13): rain-stop ceiling
    // (max 9 — was 64 with a silent cap), rain/stops field overload,
    // and the block-count ceiling (the last silent-skip in the
    // collector). Raw-value checks live in strictness.rs.
    if let Some(msg) = strictness::validate_rain_stop_contract(cfg) {
        return Some(msg);
    }
    if let Some(msg) = strictness::validate_block_count(cfg) {
        return Some(msg);
    }
    for (name, def) in collect_colors_custom(cfg) {
        if let Err(e) = def.to_palette() {
            return Some(format!("colors-custom.{name}: {e}"));
        }
    }
    None
}

// NIGHT-depthtest-3 LOC extraction: the oversized-name pre-scan
// lives in colors_custom/name_len.rs (800-LOC hard cap rule).
// NIGHT-hunt-37 LOC + policy extraction: the rain-stop ceiling,
// rain/stops overload and block-count checks live in
// colors_custom/strictness.rs (same cap rule).
mod name_len;
mod strictness;

pub(crate) use strictness::split_rain_stop_entries;

#[cfg(test)]
#[path = "../../../test/engine/chroma_dragon_engine/colors_custom/tests.rs"]
mod tests;
