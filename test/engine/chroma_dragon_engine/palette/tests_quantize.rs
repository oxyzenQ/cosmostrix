// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! task-17 contract tests: emission-boundary quantization.
//!
//! Covers `palette::quantize` (OKLab-nearest xterm-256 / canonical
//! base-16, anti-collapse floor, memoized `SgrQuantizer`, `SgrMode`
//! palette inference) plus the wire-format contracts of
//! `sgr_format::write_sgr_colors_buf` driven through the quantizer —
//! the property the task exists for: **no `38;2` truecolor sequence may
//! leave the emission boundary on a Color16/Color256/Mono session**.
//!
//! Behavior expectations were pre-verified against an independent
//! Python OKLab probe (task17_quantize_probe.py) so the fixed asserts
//! below encode measured behavior, not guesses.

use super::*;
use crate::palette::build_palette;
use crate::runtime::{ColorMode, ColorScheme};
use crate::sgr_format::write_sgr_colors_buf;

/// Extract the SGR parameter bytes (between `ESC [` and `m`).
fn sgr_params(bytes: &[u8]) -> String {
    let s = std::str::from_utf8(bytes).expect("SGR bytes are ASCII");
    let body = s
        .strip_prefix("\x1b[")
        .and_then(|b| b.strip_suffix('m'))
        .expect("SGR sequence shape \\x1b[...m");
    body.to_string()
}

// ── xterm256_nearest ────────────────────────────────────────────────────────

/// Every xterm-256 palette color (16..=255) is its own exact nearest —
/// distance 0 beats everything, so the table walk must round-trip.
#[test]
fn xterm256_exact_palette_colors_round_trip() {
    for i in 0..240usize {
        let (r, g, b) = xterm256_rgb_at(i);
        let idx = xterm256_nearest(r, g, b);
        assert_eq!(idx, (16 + i) as u8, "palette entry {i} {r},{g},{b}");
    }
}

/// Measured anchor points (from the OKLab probe):
/// black→16, white→231, mid-gray→244 (grayscale ramp), dim blue→17,
/// cube entry (95,135,175)→67, (0,200,0)→40.
#[test]
fn xterm256_known_anchors() {
    assert_eq!(xterm256_nearest(0, 0, 0), 16);
    assert_eq!(xterm256_nearest(255, 255, 255), 231);
    assert_eq!(xterm256_nearest(128, 128, 128), 244);
    assert_eq!(xterm256_nearest(0, 0, 100), 17);
    assert_eq!(xterm256_nearest(95, 135, 175), 67);
    assert_eq!(xterm256_nearest(0, 200, 0), 40);
}

/// All results live in the stable palette region 16..=255 — never the
/// host-configurable base-16 slots.
#[test]
fn xterm256_results_stay_in_stable_region() {
    for r in (0..=255u8).step_by(17) {
        for g in (0..=255u8).step_by(17) {
            for b in (0..=255u8).step_by(17) {
                let idx = xterm256_nearest(r, g, b);
                assert!((16..=255).contains(&idx), "({r},{g},{b}) -> {idx}");
            }
        }
    }
}

// ── classic16_nearest ───────────────────────────────────────────────────────

/// Exact table entries resolve to themselves (distance 0).
#[test]
fn classic16_exact_entries_resolve_to_themselves() {
    assert_eq!(classic16_nearest(0, 0, 0), Color::Black);
    assert_eq!(classic16_nearest(255, 0, 0), Color::Red);
    assert_eq!(classic16_nearest(0, 255, 0), Color::Green);
    assert_eq!(classic16_nearest(229, 229, 229), Color::Grey);
    assert_eq!(classic16_nearest(127, 127, 127), Color::DarkGrey);
    assert_eq!(classic16_nearest(255, 255, 255), Color::White);
    assert_eq!(classic16_nearest(0, 0, 238), Color::DarkBlue);
}

/// THE pre-task-17 defect case: dim blue (0,0,100) was
/// RGB-Euclidean-nearest to Black (invisible on the black canvas).
/// OKLab distance resolves it to the blue family instead.
#[test]
fn classic16_dim_blue_does_not_collapse_to_black() {
    assert_ne!(classic16_nearest(0, 0, 100), Color::Black);
    assert_eq!(classic16_nearest(0, 0, 100), Color::DarkBlue);
}

/// Anti-collapse floor: a visibly-lit achromatic-ish input (L ≥ 0.15)
/// never resolves to Black, even when Black is the raw nearest.
#[test]
fn classic16_floor_keeps_lit_inputs_visible() {
    assert_ne!(classic16_nearest(40, 40, 60), Color::Black);
    // Near-black (L < 0.15) legitimately stays Black.
    assert_eq!(classic16_nearest(8, 8, 8), Color::Black);
}

/// Luminance monotonicity contract (task-17): along hue-stable ramps —
/// how every shipped gradient is constructed — quantized luminance is
/// non-decreasing. A quantizer that inverted gradient order would make
/// rain heads dimmer than tails on 16-color terminals.
#[test]
fn classic16_luminance_monotone_along_hue_stable_ramps() {
    let table = oklab16();
    let luminance_of = |c: Color| -> f32 {
        let slot = named16_slot(c).expect("quantized color must be named");
        table[slot as usize].0
    };
    type RampFn = fn(u8) -> (u8, u8, u8);
    let ramps: [RampFn; 4] = [
        |k| (0, k, 0), // green
        |k| (0, 0, k), // blue
        |k| (k, 0, 0), // red
        |k| (k, k, k), // gray
    ];
    for ramp in ramps {
        let mut prev_l = -1.0f32;
        for k in (0..=255u8).step_by(8) {
            let (r, g, b) = ramp(k);
            let l = luminance_of(classic16_nearest(r, g, b));
            assert!(
                l >= prev_l,
                "monotonicity violated at ramp({r},{g},{b}): L={l} < prev {prev_l}"
            );
            prev_l = l;
        }
    }
}

// ── SgrMode inference ───────────────────────────────────────────────────────

/// `from_palette` recovers the palette's construction mode for all four
/// runtime color modes (the production inference path — the quantizer
/// is never handed an explicit mode).
#[test]
fn sgr_mode_from_palette_recovers_all_four_modes() {
    for (mode, expected) in [
        (ColorMode::TrueColor, SgrMode::TrueColor),
        (ColorMode::Color256, SgrMode::Ansi256),
        (ColorMode::Color16, SgrMode::Classic16),
        (ColorMode::Mono, SgrMode::Mono),
    ] {
        for scheme in [
            ColorScheme::Green,
            ColorScheme::Blue,
            ColorScheme::Spectrum20,
        ] {
            let palette = build_palette(scheme, mode, false);
            let inferred = SgrMode::from_palette(&palette);
            assert_eq!(
                inferred, expected,
                "scheme={scheme:?} mode={mode:?} inferred {inferred:?}"
            );
        }
    }
}

/// Degenerate palettes: empty → TrueColor (no quantizer, no-op).
#[test]
fn sgr_mode_empty_palette_defaults_to_truecolor() {
    let palette = crate::palette::Palette {
        colors: vec![],
        bg: None,
    };
    assert_eq!(SgrMode::from_palette(&palette), SgrMode::TrueColor);
}

// ── SgrQuantizer ────────────────────────────────────────────────────────────

/// Ansi256: RGB → indexed; memoized lookups return the same index.
#[test]
fn quantizer_ansi256_maps_rgb_to_stable_index() {
    let mut q = SgrQuantizer::new(SgrMode::Ansi256);
    let first = q.quantize_fg(Some(Color::Rgb { r: 0, g: 200, b: 0 }));
    let second = q.quantize_fg(Some(Color::Rgb { r: 0, g: 200, b: 0 }));
    assert_eq!(first, second);
    match first {
        Some(Color::AnsiValue(v)) => assert!((16..=255).contains(&v), "v={v}"),
        other => panic!("expected AnsiValue, got {other:?}"),
    }
}

/// Classic16: RGB → a named base-16 color.
#[test]
fn quantizer_classic16_maps_rgb_to_named() {
    let mut q = SgrQuantizer::new(SgrMode::Classic16);
    match q.quantize_fg(Some(Color::Rgb { r: 0, g: 200, b: 0 })) {
        Some(c) => assert!(named16_slot(c).is_some(), "got {c:?}"),
        None => panic!("expected Some(named)"),
    }
}

/// Mono: fg collapses to White, bg collapses to Reset (default).
#[test]
fn quantizer_mono_defaults() {
    let mut q = SgrQuantizer::new(SgrMode::Mono);
    let fg = q.quantize_fg(Some(Color::Rgb {
        r: 10,
        g: 200,
        b: 90,
    }));
    let bg = q.quantize_bg(Some(Color::Rgb {
        r: 10,
        g: 200,
        b: 90,
    }));
    assert_eq!(fg, Some(Color::White));
    assert_eq!(bg, Some(Color::Reset));
}

/// Semantic defaults pass through untouched in every mode: `None`
/// (blank cell) stays None, `Reset` stays Reset.
#[test]
fn quantizer_reset_and_none_passthrough_all_modes() {
    for mode in [
        SgrMode::TrueColor,
        SgrMode::Ansi256,
        SgrMode::Classic16,
        SgrMode::Mono,
    ] {
        let mut q = SgrQuantizer::new(mode);
        assert_eq!(q.quantize_fg(None), None, "mode={mode:?}");
        assert_eq!(q.quantize_bg(None), None, "mode={mode:?}");
        assert_eq!(q.quantize_fg(Some(Color::Reset)), Some(Color::Reset));
        assert_eq!(q.quantize_bg(Some(Color::Reset)), Some(Color::Reset));
    }
}

/// TrueColor: exact passthrough — the palette build path must stay
/// byte-identical to the pre-task-17 wire format.
#[test]
fn quantizer_truecolor_passthrough() {
    let mut q = SgrQuantizer::new(SgrMode::TrueColor);
    let c = Color::Rgb { r: 1, g: 2, b: 3 };
    assert_eq!(q.quantize_fg(Some(c)), Some(c));
    assert_eq!(q.quantize_bg(Some(c)), Some(c));
}

/// Named colors arriving in Ansi256 mode map to their 0-15 indexed
/// equivalent (same rendering on 256-color terminals).
#[test]
fn quantizer_ansi256_maps_named_to_indexed_slot() {
    let mut q = SgrQuantizer::new(SgrMode::Ansi256);
    assert_eq!(
        q.quantize_fg(Some(Color::Green)),
        Some(Color::AnsiValue(10))
    );
}

/// Memo: repeated colors return cached results and the memo stays
/// bounded by the number of DISTINCT colors seen.
#[test]
fn quantizer_memo_is_stable_and_bounded() {
    let mut q = SgrQuantizer::new(SgrMode::Classic16);
    let mut results = Vec::new();
    for k in 0..100u8 {
        let c = q.quantize_fg(Some(Color::Rgb { r: 0, g: k, b: 0 }));
        // Ask twice — second must be memo-identical.
        let again = q.quantize_fg(Some(Color::Rgb { r: 0, g: k, b: 0 }));
        assert_eq!(c, again, "k={k}");
        results.push(c);
    }
    // 100 distinct inputs → at most 100 memo entries (fg direction only).
    assert!(q.memo_len() <= 100, "memo_len={}", q.memo_len());
    // Distinct inputs may map to the same output — but at least two
    // distinct named colors must appear across a 0..100 green sweep.
    let distinct_outputs: std::collections::HashSet<_> = results.into_iter().collect();
    assert!(distinct_outputs.len() >= 2);
}

// ── NIGHT-hunter-12: emission-path memo contracts ───────────────────────────

/// The memo's integer hasher must spread clustered keys across buckets.
///
/// Rain ladders produce keys that cluster catastrophically under plain
/// identity hashing: a green brightness sweep is `(0, g, 0)` → packed
/// keys `g<<8` — and since hashbrown's bucket index is the hash's LOW
/// bits, a hash without downward avalanche (e.g. a bare multiply — see
/// the `PackedKeyHasher` doc) puts the whole ladder in one bucket.
/// This test pins the spread so a future hasher swap cannot silently
/// regress the SwissTable probe into linear chains.
///
/// Threshold calibration: 256 keys scattered into 12-bit buckets
/// (capacity-4096 table) by a uniform hash yield ~248 distinct (the
/// birthday bound leaves ~8 collisions); a degenerate hash yields ~1.
/// The 200 floor sits far below the uniform expectation and far above
/// any degenerate pattern.
#[test]
fn packed_key_hasher_spreads_clustered_ladder_keys() {
    let bucket_of = |key: u32| -> usize {
        let mut h = PackedKeyHasher::default();
        h.write_u32(key);
        h.finish() as usize & 0xFFF // capacity-4096 bucket index
    };
    // Family 1: green ramp (0, g, 0) — keys g<<8.
    let green_buckets: std::collections::HashSet<usize> =
        (0u32..256).map(|g| bucket_of(g << 8)).collect();
    // Family 2: gray ramp (k, k, k) — keys k<<16 | k<<8 | k.
    let gray_buckets: std::collections::HashSet<usize> = (0u32..256)
        .map(|k| bucket_of((k << 16) | (k << 8) | k))
        .collect();
    // Family 3: bg-direction ladder (0, g, 0, bg) — keys g<<8 | 1<<24.
    let bg_buckets: std::collections::HashSet<usize> = (0u32..256)
        .map(|g| bucket_of((g << 8) | (1 << 24)))
        .collect();
    assert!(
        green_buckets.len() >= 200,
        "green ladder collapsed into {} buckets",
        green_buckets.len()
    );
    assert!(
        gray_buckets.len() >= 200,
        "gray ladder collapsed into {} buckets",
        gray_buckets.len()
    );
    assert!(
        bg_buckets.len() >= 200,
        "bg-direction ladder collapsed into {} buckets",
        bg_buckets.len()
    );
}

/// Distinct keys must keep distinct full hashes — every stage of the
/// splitmix64 finalizer (add, odd multiply, xorshift) is a bijection on
/// u64, so the composition is too. A lossy hash would mean someone
/// swapped a stage for a non-invertible mix.
#[test]
fn packed_key_hasher_distinct_keys_hash_distinct() {
    let mut seen = std::collections::HashSet::with_capacity(1000);
    for k in 0..1000u32 {
        let mut h = PackedKeyHasher::default();
        h.write_u32(k);
        seen.insert(h.finish());
    }
    assert_eq!(seen.len(), 1000, "hash is not injective over 0..1000");
}

/// Mono never touches the memo: the result is a constant of the
/// direction alone, so feeding hundreds of distinct RGBs must leave
/// the map empty (no lookup work, no unbounded growth) while the wire
/// results stay fg=White / bg=Reset.
#[test]
fn quantizer_mono_memo_stays_empty() {
    let mut q = SgrQuantizer::new(SgrMode::Mono);
    for k in 0..500u16 {
        let v = k as u8;
        assert_eq!(
            q.quantize_fg(Some(Color::Rgb { r: v, g: v, b: v })),
            Some(Color::White),
            "k={k}"
        );
        assert_eq!(
            q.quantize_bg(Some(Color::Rgb {
                r: v,
                g: 255 - v,
                b: v.wrapping_mul(3)
            })),
            Some(Color::Reset),
            "k={k}"
        );
    }
    assert_eq!(q.memo_len(), 0, "Mono must not memoize a constant function");
}

/// Ansi256/Classic16 keep memoizing: first sight computes, second sight
/// hits the map (observable via memo_len growth bounded by distinct
/// inputs, across BOTH directions — fg and bg keys differ by bit 24).
#[test]
fn quantizer_memo_tracks_both_directions_independently() {
    let mut q = SgrQuantizer::new(SgrMode::Ansi256);
    let c = Color::Rgb {
        r: 10,
        g: 20,
        b: 30,
    };
    q.quantize_fg(Some(c));
    q.quantize_fg(Some(c)); // memo hit — no new entry
    assert_eq!(q.memo_len(), 1);
    q.quantize_bg(Some(c)); // different direction — new entry
    assert_eq!(q.memo_len(), 2, "fg and bg must occupy distinct memo keys");
    // Results are direction-correct for Ansi256 (both indexed).
    assert!(matches!(q.quantize_bg(Some(c)), Some(Color::AnsiValue(_))));
}

// ── Wire-format contracts through write_sgr_colors_buf ─────────────────────

/// THE task-17 contract: a Classic16 quantized (fg, bg) pair formats as
/// classic codes only — the full parameter string is
/// `3N[;4N]` / `9N[;10N]`, never `38;2`/`38;5`/`48;2`/`48;5`.
#[test]
fn write_sgr_classic16_emits_only_classic_codes() {
    let mut q = SgrQuantizer::new(SgrMode::Classic16);
    for k in (0..=255u8).step_by(7) {
        for bg in [None, Some(Color::Rgb { r: 0, g: 0, b: 0 })] {
            let fg = q.quantize_fg(Some(Color::Rgb { r: 0, g: k, b: 0 }));
            let bg = q.quantize_bg(bg);
            let mut buf = Vec::new();
            write_sgr_colors_buf(&mut buf, fg, bg);
            let params = sgr_params(&buf);
            assert!(
                !params.contains("38;2") && !params.contains("48;2"),
                "truecolor leaked in Classic16: {params:?}"
            );
            assert!(
                !params.contains("38;5") && !params.contains("48;5"),
                "indexed leaked in Classic16: {params:?}"
            );
            let fg_ok = params
                .split(';')
                .next()
                .is_some_and(|p| p.len() == 2 && (p.starts_with('3') || p.starts_with('9')));
            assert!(fg_ok, "fg not a classic code: {params:?}");
            if let Some(bg_code) = params.split(';').nth(1) {
                assert!(
                    (bg_code.starts_with('4') && bg_code.len() == 2)
                        || (bg_code.starts_with("10") && bg_code.len() == 3),
                    "bg not a classic code: {params:?}"
                );
            }
        }
    }
}

/// Same contract for Ansi256: only `38;5;N[;48;5;N]` on the wire.
#[test]
fn write_sgr_ansi256_emits_only_indexed_codes() {
    let mut q = SgrQuantizer::new(SgrMode::Ansi256);
    for k in (0..=255u8).step_by(7) {
        let fg = q.quantize_fg(Some(Color::Rgb { r: k, g: 40, b: 90 }));
        let bg = q.quantize_bg(Some(Color::Rgb { r: 0, g: 0, b: 0 }));
        let mut buf = Vec::new();
        write_sgr_colors_buf(&mut buf, fg, bg);
        let params = sgr_params(&buf);
        assert!(
            !params.contains("38;2") && !params.contains("48;2"),
            "truecolor leaked in Ansi256: {params:?}"
        );
        assert!(params.starts_with("38;5;"), "fg not indexed: {params:?}");
        if let Some(rest) = params.strip_prefix("38;5;") {
            // rest is "N" or "N;48;5;M" — the bg half must be indexed too.
            if let Some((_, bg_part)) = rest.split_once(';') {
                assert!(bg_part.starts_with("48;5;"), "bg not indexed: {params:?}");
            }
        }
    }
}

/// Mono: only `97` (bright-white fg — the single honest color) with
/// default bg — no RGB escapes at all.
#[test]
fn write_sgr_mono_emits_default_only() {
    let mut q = SgrQuantizer::new(SgrMode::Mono);
    let fg = q.quantize_fg(Some(Color::Rgb {
        r: 90,
        g: 20,
        b: 200,
    }));
    let bg = q.quantize_bg(Some(Color::Rgb {
        r: 90,
        g: 20,
        b: 200,
    }));
    let mut buf = Vec::new();
    write_sgr_colors_buf(&mut buf, fg, bg);
    let params = sgr_params(&buf);
    assert_eq!(
        params, "97;49",
        "mono wire must be bright-white-on-default: {params:?}"
    );
}

/// Named colors format directly as classic codes (the sgr_format task-17
/// arms) — fg and bg, normal and bright slots, including the
/// previously-emitted-nothing case.
#[test]
fn write_sgr_named_colors_emit_classic_fg_and_bg() {
    let cases: [(Color, &str, Option<Color>, Option<&str>); 5] = [
        (Color::Green, "92", None, Some("49")),
        (Color::DarkGreen, "32", Some(Color::Black), Some("40")),
        (Color::White, "97", Some(Color::DarkGrey), Some("100")),
        (Color::DarkBlue, "34", Some(Color::Grey), Some("47")),
        (Color::Yellow, "93", Some(Color::Cyan), Some("106")),
    ];
    for (fg, want_fg, bg, want_bg) in cases {
        let mut buf = Vec::new();
        write_sgr_colors_buf(&mut buf, Some(fg), bg);
        let params = sgr_params(&buf);
        assert_eq!(params.split(';').next(), Some(want_fg), "fg of {params:?}");
        assert_eq!(params.split(';').nth(1), want_bg, "bg of {params:?}");
    }
}
