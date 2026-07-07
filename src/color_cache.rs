// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Color byte cache — pre-formatted ANSI SGR escape sequences for
//! palette colors, eliminating the per-style-change formatting overhead
//! in the hot render path.
//!
//! ## How it works
//!
//! At startup, the palette's color list is scanned. For each color we
//! pre-compute the exact ANSI SGR byte sequence (`\x1b[38;2;R;G;Bm`)
//! and store it in a flat byte buffer with an index table.
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

use crossterm::style::Color;

use crate::palette::Palette;

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
pub struct ColorCache {
    /// Original palette colors for lookup by Color value.
    colors: Vec<Color>,
    /// The palette's background color (cached SGR entries include this bg).
    bg: Option<Color>,
    /// Single allocation holding all pre-formatted SGR byte sequences
    /// concatenated together.
    buf: Vec<u8>,
    /// Start offset of each palette color's SGR in `buf`.
    /// `offsets[i]` is the byte index of the SGR for palette color `i`.
    /// `offsets.len() - 1` is the "bg reset" entry.
    offsets: Vec<usize>,
    /// Number of palette colors (== offsets.len() - 1).
    num_colors: usize,
}

impl ColorCache {
    /// Build the cache from a palette.
    ///
    /// Pre-formats two SGR sequences per palette color:
    /// 1. `fg=color, bg=palette.bg` (the common case in full redraws)
    /// 2. Also stores a terminal-reset entry for blank/empty cells (index N).
    pub fn new(palette: &Palette) -> Self {
        let num_colors = palette.colors.len();
        let colors = palette.colors.clone();
        let bg = palette.bg;
        // +1 for the "bg-only" terminal-reset entry
        let n = num_colors + 1;
        let mut offsets = Vec::with_capacity(n);
        let mut buf = Vec::with_capacity(n * 32);

        for fg in &palette.colors {
            offsets.push(buf.len());
            push_sgr_fg_bg(&mut buf, *fg, palette.bg);
        }

        // Terminal-reset entry: no fg, bg=palette.bg (used for blank cells)
        offsets.push(buf.len());
        push_sgr_reset_bg(&mut buf, palette.bg);

        ColorCache {
            colors,
            bg,
            buf,
            offsets,
            num_colors,
        }
    }

    /// Look up the pre-formatted SGR bytes for a palette color index.
    /// `idx` must be in `0..num_colors`; panics otherwise (debug only).
    /// For `None` / blank / reset cells, use `reset_sgr()`.
    #[inline]
    pub fn sgr(&self, idx: usize) -> &[u8] {
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
    pub fn reset_sgr(&self) -> &[u8] {
        let start = self.offsets[self.num_colors];
        let end = self.buf.len();
        &self.buf[start..end]
    }

    /// Number of cached palette entries.
    #[inline]
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.num_colors
    }

    /// Find a cached SGR byte slice for a given foreground color.
    /// Returns `None` if `fg` is not one of the palette colors.
    ///
    /// Uses linear scan — palette is small (typically 7-20 colors),
    /// making this cheaper than a HashMap for the common case.
    #[inline]
    pub fn sgr_for(&self, fg: Color) -> Option<&[u8]> {
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
    #[inline]
    pub fn sgr_for_cell(&self, fg: Option<Color>, bg: Option<Color>) -> Option<&[u8]> {
        if bg != self.bg {
            return None;
        }
        match fg {
            Some(c) => self.sgr_for(c),
            None => Some(self.reset_sgr()),
        }
    }
}

// ── Internal: ANSI byte formatters (build-time only) ────────────────────────

/// Push a u8 as ASCII decimal into buf.
#[inline]
fn push_u8(buf: &mut Vec<u8>, n: u8) {
    if n < 10 {
        buf.push(b'0' + n);
    } else if n < 100 {
        buf.push(b'0' + n / 10);
        buf.push(b'0' + n % 10);
    } else {
        buf.push(b'0' + n / 100);
        buf.push(b'0' + (n / 10) % 10);
        buf.push(b'0' + n % 10);
    }
}

/// Pre-format `\x1b[38;2;R;G;B;48;2;r;g;bm` for fg + bg into buf.
fn push_sgr_fg_bg(buf: &mut Vec<u8>, fg: Color, bg: Option<Color>) {
    buf.extend_from_slice(b"\x1b[");
    #[allow(clippy::needless_late_init)]
    let semi: bool;
    match fg {
        Color::Rgb { r, g, b } => {
            buf.extend_from_slice(b"38;2;");
            push_u8(buf, r);
            buf.push(b';');
            push_u8(buf, g);
            buf.push(b';');
            push_u8(buf, b);
            semi = true;
        }
        Color::AnsiValue(v) => {
            buf.extend_from_slice(b"38;5;");
            push_u8(buf, v);
            semi = true;
        }
        Color::Reset | Color::Black => {
            buf.extend_from_slice(b"39");
            semi = true;
        }
        _ => {
            // Named colors: decode to RGB and format
            let (r, g, b) = crate::palette::color_to_rgb(fg);
            buf.extend_from_slice(b"38;2;");
            push_u8(buf, r);
            buf.push(b';');
            push_u8(buf, g);
            buf.push(b';');
            push_u8(buf, b);
            semi = true;
        }
    }
    match bg {
        Some(Color::Rgb { r, g, b }) => {
            if semi {
                buf.push(b';');
            }
            buf.extend_from_slice(b"48;2;");
            push_u8(buf, r);
            buf.push(b';');
            push_u8(buf, g);
            buf.push(b';');
            push_u8(buf, b);
        }
        Some(Color::AnsiValue(v)) => {
            if semi {
                buf.push(b';');
            }
            buf.extend_from_slice(b"48;5;");
            push_u8(buf, v);
        }
        Some(Color::Reset) | None => {
            if semi {
                buf.push(b';');
            }
            buf.extend_from_slice(b"49");
        }
        _ => {
            let (r, g, b) = crate::palette::color_to_rgb(bg.unwrap_or(Color::Reset));
            if semi {
                buf.push(b';');
            }
            buf.extend_from_slice(b"48;2;");
            push_u8(buf, r);
            buf.push(b';');
            push_u8(buf, g);
            buf.push(b';');
            push_u8(buf, b);
        }
    }
    buf.extend_from_slice(b"m");
}

/// Pre-format `\x1b[39;49m` (or with specific bg) for terminal reset.
fn push_sgr_reset_bg(buf: &mut Vec<u8>, bg: Option<Color>) {
    buf.extend_from_slice(b"\x1b[39");
    match bg {
        Some(Color::Rgb { r, g, b }) => {
            buf.extend_from_slice(b";48;2;");
            push_u8(buf, r);
            buf.push(b';');
            push_u8(buf, g);
            buf.push(b';');
            push_u8(buf, b);
        }
        Some(Color::AnsiValue(v)) => {
            buf.extend_from_slice(b";48;5;");
            push_u8(buf, v);
        }
        _ => {
            buf.extend_from_slice(b";49");
        }
    }
    buf.extend_from_slice(b"m");
}

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
}
