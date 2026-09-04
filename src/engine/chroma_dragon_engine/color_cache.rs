// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Color byte cache — pre-formatted ANSI SGR escape sequences for
//! palette colors, eliminating the per-style-change formatting overhead
//! in the hot render path.
//!
//! ## How it works
//!
//! At startup, the palette's color list is scanned. For each color we
//! pre-compute the exact ANSI SGR byte sequence and store it in a flat
//! byte buffer with an index table.
//!
//! During rendering, instead of calling `write_sgr_colors_buf` (which
//! encodes integer→ASCII digits, semicolons, and branch logic per call),
//! we do a single `extend_from_slice` from the precomputed cache.
//!
//! ## Impact
//!
//! For a typical 120×40 frame with ~7 palette colors, each dirty cell
//! run triggers one style change. The cache eliminates ~300-400 write_sgr
//! calls per frame (each ~40-50 bytes of formatting + push_u8 arithmetic),
//! replacing them with memcpy-like slices.
//!
//! The cache also pre-formats the "reset to bg" combination (fg=palette
//! color, bg=terminal bg) — the most common SGR pattern in full redraws.
//!
//! ## task-17: wire-space entries
//!
//! Entries are built through `palette::quantize::SgrQuantizer` in the
//! palette's own color mode, so cached bytes carry the session's wire
//! format (`38;5;N` for Color256, classic `3x`/`9x` codes for Color16,
//! default-only for Mono) instead of the pre-task-17 always-truecolor
//! `38;2;R;G;B`. The stored `bg` is likewise the wire-space palette bg —
//! the same space the emission boundary quantizes cell colors into, so
//! a quantized cell (fg, bg) that lands on a palette entry hits the
//! cache instead of falling through to on-the-fly formatting.

use std::sync::atomic::{AtomicU64, Ordering};

use crossterm::style::Color;

use crate::palette::quantize::SgrMode;
use crate::palette::{Palette, SgrQuantizer};
use crate::sgr_format::write_sgr_colors_buf;

/// Pre-formatted ANSI SGR byte sequences for palette colors.
///
/// Storage layout:
/// ```text
/// [sgr0][sgr1][sgr2]...[sgrN][bg_only]
///   ^     ^     ^          ^     ^
///   |     |     |          |     +-- Background-only sequence (39;49m)
///   |     |     |          +-- SGR for palette color N (fg) + bg
///   +-offsets[0]           +-- offsets[N]
/// ```
///
/// Each entry is a complete escape sequence like `\x1b[38;2;0;145;30;48;2;0;0;0m`
/// ready to be spliced directly into the ANSI output buffer.
pub(crate) struct ColorCache {
    /// Original palette colors for lookup by Color value.
    colors: Vec<Color>,
    /// The palette's background color in wire space — the space the
    /// emission boundary quantizes cell backgrounds into, so cache-hit
    /// comparison succeeds for quantized (fg, bg) pairs.
    bg: Option<Color>,
    /// Wire-format mode the entries were built for (task-17).
    sgr_mode: SgrMode,
    /// Single allocation holding all pre-formatted SGR byte sequences
    /// concatenated together.
    buf: Vec<u8>,
    /// Start offset of each palette color's SGR in `buf`.
    /// `offsets[i]` is the byte index of the SGR for palette color `i`.
    /// `offsets.len() - 1` is the "bg reset" entry.
    offsets: Vec<usize>,
    /// Number of palette colors (== offsets.len() - 1).
    num_colors: usize,
    /// SGR cache hit counter — incremented each time `sgr_for_cell()`
    /// returns `Some`. Used by `--perf-stats` to report cache hit rate.
    /// Atomic for thread-safety (though cosmostrix is single-threaded,
    /// this future-proofs the API).
    sgr_hits: AtomicU64,
    /// SGR cache miss counter — incremented each time `sgr_for_cell()`
    /// returns `None` (cell has non-palette color or non-palette bg).
    sgr_misses: AtomicU64,
}

impl ColorCache {
    /// Build the cache from a palette.
    ///
    /// Pre-formats one SGR sequence per palette color (fg + palette bg)
    /// plus a terminal-reset entry for blank/empty cells (index N),
    /// all in the palette mode's wire format via `SgrQuantizer`.
    pub(crate) fn new(palette: &Palette) -> Self {
        let num_colors = palette.colors.len();
        let colors = palette.colors.clone();
        let sgr_mode = SgrMode::from_palette(palette);
        // Build entries through the quantizer so cached bytes use the
        // same wire format the emission boundary's miss path emits.
        let mut quantizer = SgrQuantizer::new(sgr_mode);
        let bg = quantizer.quantize_bg(palette.bg);
        // +1 for the "bg-only" terminal-reset entry
        let n = num_colors + 1;
        let mut offsets = Vec::with_capacity(n);
        let mut buf = Vec::with_capacity(n * 32);

        for fg in &palette.colors {
            offsets.push(buf.len());
            write_sgr_colors_buf(&mut buf, quantizer.quantize_fg(Some(*fg)), bg);
        }

        // Terminal-reset entry: no fg, bg=palette bg (used for blank cells)
        offsets.push(buf.len());
        write_sgr_colors_buf(&mut buf, None, bg);

        ColorCache {
            colors,
            bg,
            sgr_mode,
            buf,
            offsets,
            num_colors,
            sgr_hits: AtomicU64::new(0),
            sgr_misses: AtomicU64::new(0),
        }
    }

    /// The wire-format mode this cache's entries were built for.
    /// `Terminal::set_color_cache` reads this to decide whether the
    /// emission boundary needs a quantizer (TrueColor → none).
    #[must_use]
    pub(crate) fn sgr_mode(&self) -> SgrMode {
        self.sgr_mode
    }

    /// Look up the pre-formatted SGR bytes for a palette color index.
    /// `idx` must be in `0..num_colors`; panics otherwise (debug only).
    /// For `None` / blank / reset cells, use `reset_sgr()`.
    #[inline]
    pub(crate) fn sgr(&self, idx: usize) -> &[u8] {
        debug_assert!(idx < self.num_colors);
        let start = self.offsets[idx];
        let end = if idx + 1 < self.offsets.len() {
            self.offsets[idx + 1]
        } else {
            self.buf.len()
        };
        &self.buf[start..end]
    }

    /// Look up the reset/blank SGR (no fg, palette bg).
    #[inline]
    pub(crate) fn reset_sgr(&self) -> &[u8] {
        let start = self.offsets[self.num_colors];
        let end = self.buf.len();
        &self.buf[start..end]
    }

    /// Number of cached palette entries. Test-only — production rendering
    /// uses the pre-built lookup tables directly without querying length.
    #[inline]
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.num_colors
    }

    /// Find a cached SGR byte slice for a given foreground color.
    /// Returns `None` if `fg` is not one of the palette colors.
    ///
    /// Uses linear scan — palette is small (typically 7-20 colors),
    /// making this cheaper than a HashMap for the common case.
    #[inline]
    pub(crate) fn sgr_for(&self, fg: Color) -> Option<&[u8]> {
        for (i, c) in self.colors.iter().enumerate() {
            if *c == fg {
                return Some(self.sgr(i));
            }
        }
        None
    }

    /// Try to look up a cached SGR for a (fg, bg) cell pair.
    /// Returns `None` when `bg` doesn't match the palette background
    /// (meaning the cell has a non-standard background) or `fg` is not
    /// a cached palette color.
    ///
    /// Increments internal hit/miss counters for `--perf-stats` reporting.
    /// The counters are atomic, so the increment cost is ~2ns on x86
    /// (relaxed ordering is sufficient — we only need eventual accuracy
    /// for the perf report, not strict synchronization).
    #[inline]
    pub(crate) fn sgr_for_cell(&self, fg: Option<Color>, bg: Option<Color>) -> Option<&[u8]> {
        if bg != self.bg {
            self.sgr_misses.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        let result = match fg {
            Some(c) => self.sgr_for(c),
            None => Some(self.reset_sgr()),
        };
        if result.is_some() {
            self.sgr_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.sgr_misses.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    /// Silent variant of `sgr_for_cell` — same lookup logic, but does NOT
    /// touch the atomic hit/miss counters. Used by the benchmark wet I/O
    /// hot path (`BenchIoWriter::write_frame`), where matrix rain workloads
    /// generate a unique (fg, bg) per cell and the cache always misses.
    ///
    /// At 46K FPS × 235 cells = 10.8M atomic RMW ops/sec, the silent variant
    /// saves ~5-10% of the per-cell SGR cost. The bench writer has its own
    /// local counters in `TerminalIoMetrics` if it ever needs to report
    /// hit/miss rate (currently unused — kept for future use).
    #[inline]
    pub(crate) fn sgr_for_cell_silent(
        &self,
        fg: Option<Color>,
        bg: Option<Color>,
    ) -> Option<&[u8]> {
        if bg != self.bg {
            return None;
        }
        match fg {
            Some(c) => self.sgr_for(c),
            None => Some(self.reset_sgr()),
        }
    }

    /// Return cumulative SGR cache hit/miss counters as `(hits, misses)`.
    ///
    /// Used by the `--perf-stats` exit report to compute cache hit rate.
    /// A high hit rate (>90%) indicates the palette colors dominate the
    /// frame — the cache is doing its job. A low hit rate suggests many
    /// non-palette colors (glitch, anomaly, atmospheric post-FX) are
    /// triggering the on-the-fly `write_sgr_colors_buf` path.
    #[must_use]
    pub(crate) fn cache_stats(&self) -> (u64, u64) {
        (
            self.sgr_hits.load(Ordering::Relaxed),
            self.sgr_misses.load(Ordering::Relaxed),
        )
    }
}

// task-17: the build-time formatters (`push_u8`, `push_sgr_fg_bg`,
// `push_sgr_reset_bg`) were removed — entry construction now delegates
// to `sgr_format::write_sgr_colors_buf` after quantization, giving one
// source of truth for the wire format. The old copies decoded named
// 16-colors back to `38;2` truecolor (the defect task-17 fixes) and
// mapped Black fg/bg to default (39/49) instead of their classic codes.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::build_palette;
    use crate::runtime::{ColorMode, ColorScheme};

    #[test]
    fn cache_builds_for_all_schemes() {
        for scheme in &[
            ColorScheme::Green,
            ColorScheme::Blue,
            ColorScheme::Red,
            ColorScheme::Cyan,
            ColorScheme::Neon,
            ColorScheme::Fire,
            ColorScheme::Rainbow,
            ColorScheme::Spectrum20,
            ColorScheme::Cosmos,
        ] {
            let palette = build_palette(*scheme, ColorMode::TrueColor, false);
            let cache = ColorCache::new(&palette);
            assert_eq!(cache.len(), palette.colors.len());
            // Every cached SGR must start with ESC
            for i in 0..cache.len() {
                let sgr = cache.sgr(i);
                assert!(
                    sgr.starts_with(b"\x1b["),
                    "scheme={scheme:?} idx={i} does not start with ESC[: {:?}",
                    std::str::from_utf8(sgr).unwrap_or("<invalid utf8>")
                );
                assert!(
                    sgr.ends_with(b"m"),
                    "scheme={scheme:?} idx={i} does not end with 'm'"
                );
            }
            // Reset entry must be 39 (default fg)
            let reset = cache.reset_sgr();
            assert!(reset.starts_with(b"\x1b[39"), "reset must set default fg");
        }
    }

    #[test]
    fn cache_entries_are_non_overlapping() {
        let palette = build_palette(ColorScheme::Spectrum20, ColorMode::TrueColor, false);
        let cache = ColorCache::new(&palette);
        for i in 0..cache.len() {
            for j in (i + 1)..cache.len() {
                let a = cache.sgr(i);
                let b = cache.sgr(j);
                // Different entries may produce identical SGR if palette has
                // duplicate colors, but they must be distinct slices in memory.
                let a_ptr = a.as_ptr() as usize;
                let b_ptr = b.as_ptr() as usize;
                let a_end = a_ptr + a.len();
                assert!(
                    b_ptr >= a_end || b_ptr + b.len() <= a_ptr,
                    "entries {i} and {j} overlap in the cache buffer"
                );
            }
        }
    }

    #[test]
    fn cache_with_bg_none_formats_correctly() {
        let palette = Palette {
            colors: vec![Color::Rgb { r: 0, g: 255, b: 0 }],
            bg: None,
        };
        let cache = ColorCache::new(&palette);
        let sgr = std::str::from_utf8(cache.sgr(0)).unwrap();
        assert!(sgr.contains("38;2;0;255;0"), "missing fg: {sgr}");
        assert!(sgr.contains("49"), "missing default bg: {sgr}");
    }

    #[test]
    fn cache_with_rgb_bg_formats_correctly() {
        let palette = Palette {
            colors: vec![Color::Rgb { r: 0, g: 255, b: 0 }],
            bg: Some(Color::Rgb {
                r: 10,
                g: 10,
                b: 10,
            }),
        };
        let cache = ColorCache::new(&palette);
        let sgr = std::str::from_utf8(cache.sgr(0)).unwrap();
        assert!(sgr.contains("48;2;10;10;10"), "missing bg rgb: {sgr}");
    }

    #[test]
    fn cache_stats_start_at_zero() {
        let palette = Palette {
            colors: vec![Color::Rgb { r: 0, g: 255, b: 0 }],
            bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        };
        let cache = ColorCache::new(&palette);
        let (hits, misses) = cache.cache_stats();
        assert_eq!(hits, 0, "hits must start at 0");
        assert_eq!(misses, 0, "misses must start at 0");
    }

    #[test]
    fn cache_stats_counts_hits_on_palette_color_lookup() {
        let palette = Palette {
            colors: vec![Color::Rgb { r: 0, g: 255, b: 0 }],
            bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        };
        let cache = ColorCache::new(&palette);
        // Lookup a palette fg color with matching bg → should be a hit.
        let _ = cache.sgr_for_cell(
            Some(Color::Rgb { r: 0, g: 255, b: 0 }),
            Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        );
        let (hits, misses) = cache.cache_stats();
        assert_eq!(hits, 1, "palette color lookup must count as hit");
        assert_eq!(misses, 0, "no misses expected");
    }

    #[test]
    fn cache_stats_counts_miss_on_non_palette_color() {
        let palette = Palette {
            colors: vec![Color::Rgb { r: 0, g: 255, b: 0 }],
            bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        };
        let cache = ColorCache::new(&palette);
        // Lookup a color NOT in the palette → miss.
        let _ = cache.sgr_for_cell(
            Some(Color::Rgb { r: 255, g: 0, b: 0 }),
            Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        );
        let (hits, misses) = cache.cache_stats();
        assert_eq!(hits, 0, "non-palette fg must not be a hit");
        assert_eq!(misses, 1, "non-palette fg must count as miss");
    }

    #[test]
    fn cache_stats_counts_miss_on_non_palette_bg() {
        let palette = Palette {
            colors: vec![Color::Rgb { r: 0, g: 255, b: 0 }],
            bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        };
        let cache = ColorCache::new(&palette);
        // Lookup with a bg that doesn't match palette bg → miss.
        let _ = cache.sgr_for_cell(
            Some(Color::Rgb { r: 0, g: 255, b: 0 }),
            Some(Color::Rgb {
                r: 99,
                g: 99,
                b: 99,
            }),
        );
        let (hits, misses) = cache.cache_stats();
        assert_eq!(hits, 0, "non-palette bg must not be a hit");
        assert_eq!(misses, 1, "non-palette bg must count as miss");
    }

    #[test]
    fn cache_stats_counts_reset_as_hit() {
        let palette = Palette {
            colors: vec![Color::Rgb { r: 0, g: 255, b: 0 }],
            bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        };
        let cache = ColorCache::new(&palette);
        // Lookup with fg=None (blank cell) → reset SGR → hit.
        let _ = cache.sgr_for_cell(None, Some(Color::Rgb { r: 0, g: 0, b: 0 }));
        let (hits, misses) = cache.cache_stats();
        assert_eq!(hits, 1, "reset SGR lookup must count as hit");
        assert_eq!(misses, 0, "no misses expected");
    }

    #[test]
    fn cache_stats_accumulate_across_calls() {
        let palette = Palette {
            colors: vec![Color::Rgb { r: 0, g: 255, b: 0 }],
            bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        };
        let cache = ColorCache::new(&palette);
        // 3 hits + 2 misses
        let _ = cache.sgr_for_cell(
            Some(Color::Rgb { r: 0, g: 255, b: 0 }),
            Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        );
        let _ = cache.sgr_for_cell(None, Some(Color::Rgb { r: 0, g: 0, b: 0 }));
        let _ = cache.sgr_for_cell(
            Some(Color::Rgb { r: 0, g: 255, b: 0 }),
            Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        );
        let _ = cache.sgr_for_cell(
            Some(Color::Rgb { r: 1, g: 1, b: 1 }),
            Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        );
        let _ = cache.sgr_for_cell(
            Some(Color::Rgb { r: 0, g: 255, b: 0 }),
            Some(Color::Rgb { r: 9, g: 9, b: 9 }),
        );
        let (hits, misses) = cache.cache_stats();
        assert_eq!(hits, 3, "expected 3 hits");
        assert_eq!(misses, 2, "expected 2 misses");
    }

    #[test]
    fn silent_variant_does_not_touch_counters() {
        // Strategy C: sgr_for_cell_silent must NOT increment the atomic
        // hit/miss counters. This is the contract that lets the bench
        // writer skip ~10.8M atomic RMW ops/sec in the matrix rain hot path.
        let palette = Palette {
            colors: vec![Color::Rgb { r: 0, g: 255, b: 0 }],
            bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        };
        let cache = ColorCache::new(&palette);

        // Mix of would-be hits and would-be misses through the silent path.
        let _ = cache.sgr_for_cell_silent(
            Some(Color::Rgb { r: 0, g: 255, b: 0 }),
            Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        );
        let _ = cache.sgr_for_cell_silent(None, Some(Color::Rgb { r: 0, g: 0, b: 0 }));
        let _ = cache.sgr_for_cell_silent(
            Some(Color::Rgb { r: 1, g: 1, b: 1 }),
            Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        );
        let _ = cache.sgr_for_cell_silent(
            Some(Color::Rgb { r: 0, g: 255, b: 0 }),
            Some(Color::Rgb { r: 9, g: 9, b: 9 }),
        );

        let (hits, misses) = cache.cache_stats();
        assert_eq!(hits, 0, "silent variant must not increment hit counter");
        assert_eq!(misses, 0, "silent variant must not increment miss counter");
    }

    #[test]
    fn silent_variant_returns_same_result_as_loud_variant() {
        // The silent variant must return the exact same Option<&[u8]> as the
        // regular sgr_for_cell for every (fg, bg) input — it only differs in
        // whether the atomic counters are touched.
        let palette = Palette {
            colors: vec![Color::Rgb { r: 0, g: 255, b: 0 }],
            bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        };
        let cache = ColorCache::new(&palette);

        let cases: [(Option<Color>, Option<Color>); 4] = [
            (
                Some(Color::Rgb { r: 0, g: 255, b: 0 }),
                Some(Color::Rgb { r: 0, g: 0, b: 0 }),
            ),
            (None, Some(Color::Rgb { r: 0, g: 0, b: 0 })),
            (
                Some(Color::Rgb { r: 1, g: 1, b: 1 }),
                Some(Color::Rgb { r: 0, g: 0, b: 0 }),
            ),
            (
                Some(Color::Rgb { r: 0, g: 255, b: 0 }),
                Some(Color::Rgb { r: 9, g: 9, b: 9 }),
            ),
        ];

        for (fg, bg) in cases {
            let loud = cache.sgr_for_cell(fg, bg).map(|s| s.to_vec());
            let silent = cache.sgr_for_cell_silent(fg, bg).map(|s| s.to_vec());
            assert_eq!(
                loud, silent,
                "silent variant must match loud variant for fg={fg:?} bg={bg:?}"
            );
        }
    }

    // ── task-17: wire-format contracts ─────────────────────────────────────

    /// Helper: assert every cached SGR entry (including the reset entry)
    /// satisfies the predicate, and that all entries are well-formed
    /// escape sequences.
    fn assert_every_entry(cache: &ColorCache, pred: impl Fn(&str) -> bool, label: &str) {
        for i in 0..=cache.len() {
            let sgr = if i == cache.len() {
                cache.reset_sgr()
            } else {
                cache.sgr(i)
            };
            let s = std::str::from_utf8(sgr).expect("SGR bytes are ASCII");
            assert!(s.starts_with("\x1b[") && s.ends_with('m'), "shape: {s:?}");
            let params = &s[2..s.len() - 1];
            assert!(pred(params), "{label} violated by entry {i}: {s:?}");
        }
    }

    /// Color16 palettes cache classic-code entries — no truecolor, no
    /// indexed. This is the wire the linux console (and every terminal
    /// that resolved Color16) actually honors.
    #[test]
    fn cache_color16_entries_are_classic_wire() {
        for scheme in [ColorScheme::Green, ColorScheme::Blue, ColorScheme::Rainbow] {
            let palette = build_palette(scheme, ColorMode::Color16, false);
            let cache = ColorCache::new(&palette);
            assert_eq!(cache.sgr_mode(), crate::palette::SgrMode::Classic16);
            assert_every_entry(
                &cache,
                |p| {
                    !p.contains("38;2")
                        && !p.contains("48;2")
                        && !p.contains("38;5")
                        && !p.contains("48;5")
                },
                "Color16 cache must be classic-only",
            );
            // Palette bg is Black (slot 0) → bg code 40, matching the
            // truecolor path's 48;2;0;0;0 black-canvas semantics.
            let reset = std::str::from_utf8(cache.reset_sgr()).unwrap();
            assert!(
                reset.contains("39;40"),
                "reset entry must be default-fg on black bg: {reset:?}"
            );
        }
    }

    /// Color256 palettes cache indexed entries — no truecolor.
    #[test]
    fn cache_color256_entries_are_indexed_wire() {
        for scheme in [
            ColorScheme::Green,
            ColorScheme::Blue,
            ColorScheme::Spectrum20,
        ] {
            let palette = build_palette(scheme, ColorMode::Color256, false);
            let cache = ColorCache::new(&palette);
            assert_eq!(cache.sgr_mode(), crate::palette::SgrMode::Ansi256);
            assert_every_entry(
                &cache,
                |p| {
                    (p.starts_with("38;5;") || p.starts_with("39"))
                        && !p.contains("38;2")
                        && !p.contains("48;2")
                },
                "Color256 cache must be indexed-only",
            );
        }
    }

    /// TrueColor palettes keep the historical truecolor wire — the
    /// emission fix must not regress the default experience.
    #[test]
    fn cache_truecolor_entries_stay_truecolor_wire() {
        let palette = build_palette(ColorScheme::Green, ColorMode::TrueColor, false);
        let cache = ColorCache::new(&palette);
        assert_eq!(cache.sgr_mode(), crate::palette::SgrMode::TrueColor);
        let mut found_truecolor = false;
        for i in 0..cache.len() {
            let s = std::str::from_utf8(cache.sgr(i)).unwrap();
            if s.contains("38;2;") {
                found_truecolor = true;
            }
        }
        assert!(found_truecolor, "truecolor cache must emit 38;2 entries");
    }

    /// Mono palettes cache default-only entries (bright-white fg on
    /// default bg).
    #[test]
    fn cache_mono_entries_are_default_wire() {
        let palette = build_palette(ColorScheme::Green, ColorMode::Mono, false);
        let cache = ColorCache::new(&palette);
        assert_eq!(cache.sgr_mode(), crate::palette::SgrMode::Mono);
        assert_every_entry(
            &cache,
            |p| p == "97;49" || p == "39;49",
            "Mono cache must be default-only",
        );
    }
}
