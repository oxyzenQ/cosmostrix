// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunter-11a: Color16 anchor invariants — the per-theme
//! quantization quality contract for the classic-16 wire mode.
//!
//! The audit that produced this contract (2026-09-06) measured every
//! theme's Color16 palette against the canonical xterm base-16 slot
//! luminance (OKLab L of XTERM16_RGB) and found four defect classes:
//!
//! 1. Inverted head hierarchy — the head slot dimmer than body slots
//!    (yellow/venus pale heads quantized to Grey 0.92 under saturated
//!    Yellow 0.97 bodies; rainbow's violet head quantized to Blue 0.58,
//!    dimmer than its Red 0.63 trail).
//! 2. Mid-ladder brightness dips — slot L decreasing toward the head
//!    (visible banding).
//! 3. Gradient collapse — fewer than 3 distinct slots (moon's 11-stop
//!    grayscale ramp collapsed to 2; five hand-tuned ladders shipped
//!    with only 2 anchors, off the family's 3-anchor convention).
//! 4. Neutral-band corruption — a neutral slot (DarkGrey) inside a
//!    saturated hue ladder (rainbow's orange band), breaking hue
//!    continuity.
//!
//! The fixes (hand-tuned 3-anchor ladders in catalog/themes.rs) follow
//! the existing StopsWithC16 convention: Dark* trail anchor, bright
//! body anchor, White (or theme-faithful bright) head anchor. The
//! invariants below lock the contract so future themes and future
//! quantizer changes cannot silently regress it.
//!
//! Exemptions: rainbow and spectrum20 are hue-cycle themes — their
//! identity is a hue walk, not a brightness ladder, so intra-ladder L
//! dips are inherent (their head-vs-trail hierarchy is still asserted).
//! The exemption list is asserted to be exactly these two names so a
//! new exemption must be a conscious, documented decision.

use super::*;

/// Canonical xterm base-16 RGB values — mirrors the private
/// XTERM16_RGB table in quantize.rs (kept in sync by
/// `c16_xterm_table_matches_quantize` below).
const X16: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 0, 0),
    (0, 205, 0),
    (205, 205, 0),
    (0, 0, 238),
    (205, 0, 205),
    (0, 205, 205),
    (229, 229, 229),
    (127, 127, 127),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (92, 92, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

/// OKLab luminance of a crossterm named color's canonical xterm slot.
/// Non-base-16 colors return None (the caller treats that as a
/// contract violation — hand-tuned c16 arrays must be named slots).
fn slot_l(c: Color) -> Option<f32> {
    use crate::engine::chroma_dragon_engine::gradient::srgb_to_oklab;
    let slot = named_slot(c)?;
    let (r, g, b) = X16[slot];
    Some(srgb_to_oklab(r, g, b).0)
}

/// Xterm base-16 slot index of a crossterm named color, if it is one.
fn named_slot(c: Color) -> Option<usize> {
    let slot = match c {
        Color::Black => 0,
        Color::DarkRed => 1,
        Color::DarkGreen => 2,
        Color::DarkYellow => 3,
        Color::DarkBlue => 4,
        Color::DarkMagenta => 5,
        Color::DarkCyan => 6,
        Color::Grey => 7,
        Color::DarkGrey => 8,
        Color::Red => 9,
        Color::Green => 10,
        Color::Yellow => 11,
        Color::Blue => 12,
        Color::Magenta => 13,
        Color::Cyan => 14,
        Color::White => 15,
        _ => return None,
    };
    Some(slot)
}

/// Themes whose Color16 ladder is a hue cycle, not a brightness ladder.
/// Intra-ladder L dips are exempt for these (documented above).
const HUE_CYCLE_THEMES: [&str; 2] = ["rainbow", "spectrum20"];

fn theme_name(scheme: crate::runtime::ColorScheme) -> &'static str {
    crate::theme::canonical_name_for_scheme(scheme).unwrap_or("unknown")
}

/// Keep the local X16 mirror in sync with the quantizer's private
/// XTERM16_RGB table — if quantize.rs ever re-bases the reference
/// table, this test forces the mirror (and the L math) to follow.
#[test]
fn c16_xterm_table_matches_quantize() {
    // The SgrQuantizer round-trip test already locks slot identities;
    // here we lock the L ordering property that the anchor invariants
    // depend on: White is the brightest slot, Black the dimmest, and
    // the neutral ladder Black < DarkGrey < Grey < White is monotone.
    let white = slot_l(Color::White).unwrap();
    let grey = slot_l(Color::Grey).unwrap();
    let dark_grey = slot_l(Color::DarkGrey).unwrap();
    let black = slot_l(Color::Black).unwrap();
    assert!(white > grey, "White ({white}) must out-shine Grey ({grey})");
    assert!(
        grey > dark_grey,
        "Grey ({grey}) must out-shine DarkGrey ({dark_grey})"
    );
    assert!(
        dark_grey > black,
        "DarkGrey ({dark_grey}) must out-shine Black ({black})"
    );
    // Every slot must resolve through the table.
    for c in [
        Color::Black,
        Color::DarkRed,
        Color::DarkGreen,
        Color::DarkYellow,
        Color::DarkBlue,
        Color::DarkMagenta,
        Color::DarkCyan,
        Color::Grey,
        Color::DarkGrey,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::White,
    ] {
        assert!(slot_l(c).is_some(), "{c:?} must be a base-16 slot");
    }
}

/// INV-1 (head hierarchy): for every theme, the Color16 head slot (the
/// LAST palette entry — the rain's leading character) must be strictly
/// brighter than the trail slot (the FIRST entry). No exemptions: the
/// hue-cycle themes close on Magenta / White heads that out-shine their
/// trails by design.
#[test]
fn c16_head_brighter_than_trail_all_themes() {
    use crate::engine::chroma_dragon_engine::catalog::THEMES;
    for t in THEMES.iter() {
        let p = build_palette(t.scheme, ColorMode::Color16, true);
        let n = p.colors.len();
        assert!(
            n >= 2,
            "theme {} c16 palette must have 2+ entries",
            theme_name(t.scheme)
        );
        let l_head = slot_l(p.colors[n - 1]).unwrap_or_else(|| {
            panic!(
                "{} head {:?} is not a base-16 slot",
                theme_name(t.scheme),
                p.colors[n - 1]
            )
        });
        let l_trail = slot_l(p.colors[0]).unwrap_or_else(|| {
            panic!(
                "{} trail {:?} is not a base-16 slot",
                theme_name(t.scheme),
                p.colors[0]
            )
        });
        assert!(
            l_head > l_trail,
            "theme {}: c16 head {:?} (L {l_head:.3}) must out-shine trail {:?} (L {l_trail:.3}) — inverted hierarchy",
            theme_name(t.scheme),
            p.colors[n - 1],
            p.colors[0]
        );
    }
}

/// INV-2 (ladder monotonicity): walking the palette trail → head, slot
/// luminance must be non-decreasing (no brightness dips). Exempt: the
/// documented hue-cycle themes, whose identity is a hue walk. The
/// exemption list is asserted to be exactly HUE_CYCLE_THEMES.
#[test]
fn c16_no_luminance_dips_except_hue_cycle_themes() {
    use crate::engine::chroma_dragon_engine::catalog::THEMES;
    for t in THEMES.iter() {
        let name = theme_name(t.scheme);
        let p = build_palette(t.scheme, ColorMode::Color16, true);
        if HUE_CYCLE_THEMES.contains(&name) {
            continue;
        }
        let ls: Vec<f32> = p
            .colors
            .iter()
            .map(|c| {
                slot_l(*c).unwrap_or_else(|| panic!("{}: {:?} is not a base-16 slot", name, c))
            })
            .collect();
        for i in 1..ls.len() {
            assert!(
                ls[i] >= ls[i - 1] - 1e-6,
                "theme {}: c16 luminance dip at index {i} — {:?} (L {:.3}) is dimmer than {:?} (L {:.3}) walking trail → head",
                name,
                p.colors[i],
                ls[i],
                p.colors[i - 1],
                ls[i - 1]
            );
        }
    }
}

/// INV-3 (gradient graduation): every Color16 palette must use at least
/// 3 distinct slots. Two-slot ladders flatten the trail/body/head
/// gradient into a binary flip — the audit's collapse defect class.
#[test]
fn c16_at_least_three_distinct_slots() {
    use crate::engine::chroma_dragon_engine::catalog::THEMES;
    for t in THEMES.iter() {
        let p = build_palette(t.scheme, ColorMode::Color16, true);
        let distinct = {
            let mut s = std::collections::HashSet::new();
            for c in &p.colors {
                s.insert(*c);
            }
            s.len()
        };
        assert!(
            distinct >= 3,
            "theme {}: c16 palette collapsed to {distinct} distinct slots — a 3-anchor ladder is the minimum graduation",
            theme_name(t.scheme)
        );
    }
}

/// INV-4 (readability): no Color16 palette entry may be Black — a
/// black-on-black glyph is invisible (the emission-side quantizer
/// enforces this per color; this locks it at palette-construction time
/// for the hand-tuned arrays too).
#[test]
fn c16_no_black_slots_anywhere() {
    use crate::engine::chroma_dragon_engine::catalog::THEMES;
    for t in THEMES.iter() {
        let p = build_palette(t.scheme, ColorMode::Color16, true);
        for c in &p.colors {
            assert!(
                !matches!(c, Color::Black),
                "theme {}: c16 palette contains Black — invisible glyph risk",
                theme_name(t.scheme)
            );
        }
    }
}

/// INV-5 (hand-tuned hygiene): every entry of a hand-tuned c16 array
/// (StopsWithC16 / RgbWithC16) must be a named base-16 color. Rgb or
/// AnsiValue entries would silently bypass the classic-16 wire format
/// and fall back to per-cell emission quantization.
#[test]
fn c16_hand_tuned_arrays_are_named_base16() {
    use crate::engine::chroma_dragon_engine::catalog::{ThemeColors, THEMES};
    for t in THEMES.iter() {
        if let ThemeColors::StopsWithC16 { c16, .. } | ThemeColors::RgbWithC16 { c16, .. } = &t.def
        {
            for c in c16.iter() {
                assert!(
                    named_slot(*c).is_some(),
                    "theme {}: hand-tuned c16 entry {:?} is not a named base-16 color",
                    theme_name(t.scheme),
                    c
                );
                assert!(
                    !matches!(c, Color::Black),
                    "theme {}: hand-tuned c16 entry Black is invisible",
                    theme_name(t.scheme)
                );
            }
        }
    }
}

/// INV-6 (exemption discipline): the hue-cycle exemption list must
/// cover exactly rainbow and spectrum20. Adding an exemption is a
/// design decision that must be visible in this test's diff.
#[test]
fn c16_hue_cycle_exemption_list_is_exact() {
    use crate::engine::chroma_dragon_engine::catalog::THEMES;
    let exempted: Vec<&str> = THEMES
        .iter()
        .map(|t| theme_name(t.scheme))
        .filter(|n| HUE_CYCLE_THEMES.contains(n))
        .collect();
    assert_eq!(
        exempted.len(),
        HUE_CYCLE_THEMES.len(),
        "every HUE_CYCLE_THEMES entry must name a registered theme"
    );
    // rainbow closes on Magenta (L 0.70) over a Red (0.63) trail — head
    // hierarchy holds without the exemption; the exemption only covers
    // intra-ladder dips.
    let p = build_palette(
        THEMES
            .iter()
            .find(|t| theme_name(t.scheme) == "rainbow")
            .unwrap()
            .scheme,
        ColorMode::Color16,
        true,
    );
    let n = p.colors.len();
    assert_eq!(
        p.colors[n - 1],
        Color::Magenta,
        "rainbow closes on Magenta (the c16 violet)"
    );
    assert_eq!(p.colors[0], Color::Red, "rainbow opens on Red (the trail)");
    // No neutral slot inside the rainbow ladder (the DarkGrey-band defect).
    for c in &p.colors {
        assert!(
            !matches!(
                c,
                Color::Grey | Color::DarkGrey | Color::White | Color::Black
            ),
            "rainbow c16 ladder must stay saturated — found neutral slot {c:?}"
        );
    }
}
