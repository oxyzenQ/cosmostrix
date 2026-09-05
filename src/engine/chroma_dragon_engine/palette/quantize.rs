// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Emission-boundary color quantization for non-truecolor modes.
//!
//! ## Why this module exists (task-17, owner-approved Step 1)
//!
//! The rain renderer computes colors in RGB (OKLab gradient engine,
//! shading, phosphor decay) and carries `Color::Rgb` values all the way
//! to the SGR emission boundary. Before task-17 that boundary formatted
//! every RGB as `\x1b[38;2;R;G;Bm` regardless of the session's resolved
//! color mode. Terminals that resolve Color16 or Color256 (linux console,
//! old VTE, tmux without Tc) drop `38;2` sequences — palette identity was
//! lost on the wire and the PTY probe measured 12,470 truecolor SGRs in
//! a 2.5s `--color-mode 16` session with 0 classic emissions.
//!
//! This module quantizes at exactly that boundary:
//!
//! - Ansi256: `Color::Rgb` → `Color::AnsiValue(n)` with `n` the
//!   exact OKLab-nearest entry of the xterm-256 palette (indices
//!   16..=255; 0..15 are host-configurable and skipped by convention).
//! - Classic16: `Color::Rgb` → nearest of the 16 canonical xterm
//!   base colors as a crossterm named `Color`, which `sgr_format`
//!   then emits as classic `30-37`/`90-97` (fg) and `40-47`/`100-107`
//!   (bg) codes — the wire format the capability table in
//!   `output/mod.rs` documents for Color16.
//! - Mono: any color collapses to `White` (fg) / default (bg) so a
//!   `--color-mode mono` session never leaks RGB escapes.
//! - TrueColor: passthrough (a quantizer is never constructed for
//!   truecolor sessions — zero hot-path overhead).
//!
//! ## Why OKLab nearest (not the old cube+gray heuristic)
//!
//! The legacy `rgb_to_ansi256` picked the 6x6x6 cube cell by rounded
//! channel division and compared cube-vs-gray in RGB Euclidean distance.
//! Two known failure classes: hue skew near cube-cell boundaries and the
//! dim-blue collapse — `(0,0,100)` is RGB-Euclidean-nearest to Black, so
//! dim blues died on black backgrounds. OKLab distance fixes both: the
//! search is exact over all 240 candidates and perceptual, so dim blues
//! resolve to `DarkBlue` and hue is preserved. See
//! `docs/research/COLOR_SPACE_MASTER_RESEARCH.md` for why OKLab is this
//! codebase's canonical perceptual space.
//!
//! ## Anti-collapse floor (readability guard)
//!
//! A visibly-lit input (OKLab L ≥ 0.15) must never quantize to `Black`
//! in Classic16 mode — that would render an invisible glyph on the
//! black canvas the 16-color palettes paint. Near-black inputs (L below
//! the floor) still legitimately map to `Black`.
//!
//! ## Memoization
//!
//! `SgrQuantizer` memoizes RGB→quantized results in a flat hash map
//! keyed by the packed `(r<<16 | g<<8 | b | is_bg<<24)` integer. Rain
//! shading produces bounded distinct RGB values per session (a few
//! thousand at most: palette colors × brightness levels), so the
//! 240-candidate OKLab scan runs only on first sight of each color;
//! steady-state cost is one hash lookup per style change.
//!
//! NIGHT-hunter-12 (emission-path memo audit) tightened the map on the
//! hot path:
//!
//! - Integer hasher instead of std's SipHash. The default `RandomState`
//!   defends against hash-flooding attackers, but the memo's keys are
//!   internal packed integers, not attacker-controlled — each
//!   steady-state emission lookup paid ~15-25ns of SipHash work for a
//!   key that is already dense. [`PackedKeyHasher`] is the splitmix64
//!   finalizer (2 multiplies + 2 shifts, still ~1-2ns) whose downward
//!   avalanche matters because ladder keys share low-bit structure —
//!   see its doc — and the spread is pinned by test
//!   (`packed_key_hasher_spreads_clustered_ladder_keys`).
//! - Mono short-circuits before the map. The Mono result is a constant
//!   (fg→`White`, bg→`Reset`) — memoizing a constant both wasted a hash
//!   insert per first-sight color and grew the map without bound in
//!   phosphor-decay sessions. Mono now returns directly.
//! - Warm-start capacity. `with_capacity_and_hasher(256)` covers the
//!   typical warm-set (palette × brightness ladder ≈ 112-640 entries),
//!   so warm-up frames pay no grow+rehash chain.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::OnceLock;

use crossterm::style::Color;

use crate::engine::chroma_dragon_engine::gradient::srgb_to_oklab;

/// Wire-format mode of an SGR emission boundary.
///
/// Derived from the palette a `ColorCache` was built from (see
/// [`SgrMode::from_palette`]) — the palette's construction already
/// encodes the session's color mode, so no extra state needs to flow
/// through the event loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SgrMode {
    /// Passthrough — emit `38;2;R;G;B` exactly as computed.
    TrueColor,
    /// Emit `38;5;N` / `48;5;N` with the OKLab-nearest xterm-256 index.
    Ansi256,
    /// Emit classic `30-37`/`90-97` (fg) and `40-47`/`100-107` (bg).
    Classic16,
    /// Emit default fg / default bg — no color escapes at all.
    Mono,
}

impl SgrMode {
    /// Infer the emission mode from a built palette.
    ///
    /// Sound because palette construction is mode-driven
    /// (`catalog::build_colors` / `colors_from_rgb`):
    /// - Mono palettes are exactly `[Color::White]`;
    /// - TrueColor palettes carry `Color::Rgb` entries;
    /// - Color256 palettes carry `Color::AnsiValue(16..=255)`;
    /// - Color16 palettes carry named colors (hand-tuned c16 arrays or
    ///   `rgb_to_color16` output).
    ///
    /// Mixed palettes resolve to the most conservative mode that still
    /// produces valid escapes (any `Rgb` wins TrueColor).
    #[must_use]
    pub(crate) fn from_palette(palette: &crate::palette::Palette) -> Self {
        let colors = &palette.colors;
        if colors.is_empty() {
            return SgrMode::TrueColor;
        }
        if colors.len() == 1 && colors[0] == Color::White {
            return SgrMode::Mono;
        }
        if colors.iter().any(|c| matches!(c, Color::Rgb { .. })) {
            return SgrMode::TrueColor;
        }
        if colors
            .iter()
            .any(|c| matches!(c, Color::AnsiValue(v) if *v >= 16))
        {
            return SgrMode::Ansi256;
        }
        SgrMode::Classic16
    }
}

// ── Canonical xterm palettes ────────────────────────────────────────────────

/// Canonical xterm base-16 RGB values (indices 0..=15).
///
/// These match the xterm-256 palette's first 16 entries, so `38;5;N`
/// (N < 16) and the classic codes below render identically on
/// xterm-compatible terminals. Terminals remap these slots freely
/// (that is why the 256-nearest search skips 0..15), but the xterm
/// defaults are the best neutral reference table.
const XTERM16_RGB: [(u8, u8, u8); 16] = [
    (0, 0, 0),       // 0 black
    (205, 0, 0),     // 1 dark red
    (0, 205, 0),     // 2 dark green
    (205, 205, 0),   // 3 dark yellow
    (0, 0, 238),     // 4 dark blue
    (205, 0, 205),   // 5 dark magenta
    (0, 205, 205),   // 6 dark cyan
    (229, 229, 229), // 7 grey
    (127, 127, 127), // 8 dark grey
    (255, 0, 0),     // 9 red
    (0, 255, 0),     // 10 green
    (255, 255, 0),   // 11 yellow
    (92, 92, 255),   // 12 blue
    (255, 0, 255),   // 13 magenta
    (0, 255, 255),   // 14 cyan
    (255, 255, 255), // 15 white
];

/// Crossterm named colors indexed by xterm base-16 slot.
///
/// `NAMED16[n]` is what slot `n` looks like as a `Color` variant; it is
/// the value `classic16_nearest` returns and the representation
/// `sgr_format` formats as classic `3x`/`9x` codes.
const NAMED16: [Color; 16] = [
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
];

/// OKLab coordinates of [`XTERM16_RGB`] (lazy, computed once).
fn oklab16() -> &'static [(f32, f32, f32); 16] {
    static TABLE: OnceLock<[(f32, f32, f32); 16]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [(0.0f32, 0.0f32, 0.0f32); 16];
        for (i, &(r, g, b)) in XTERM16_RGB.iter().enumerate() {
            t[i] = srgb_to_oklab(r, g, b);
        }
        t
    })
}

/// RGB values of the xterm-256 palette entries `16..=255`
/// (6x6x6 cube + 24-step grayscale ramp).
fn xterm256_rgb_at(idx: usize) -> (u8, u8, u8) {
    const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    debug_assert!((0..240).contains(&idx), "idx must be 0..240");
    if idx < 216 {
        let r = CUBE_LEVELS[idx / 36];
        let g = CUBE_LEVELS[(idx % 36) / 6];
        let b = CUBE_LEVELS[idx % 6];
        (r, g, b)
    } else {
        // Grayscale ramp: palette index 232+v has value 8 + 10*v.
        let v = 8 + 10 * (idx - 216) as u8;
        (v, v, v)
    }
}

/// OKLab coordinates of xterm-256 entries `16..=255` (lazy, computed
/// once — the exact-nearest search scans all 240 of these).
fn oklab256() -> &'static [(f32, f32, f32); 240] {
    static TABLE: OnceLock<[(f32, f32, f32); 240]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [(0.0f32, 0.0f32, 0.0f32); 240];
        for (i, slot) in t.iter_mut().enumerate() {
            let (r, g, b) = xterm256_rgb_at(i);
            *slot = srgb_to_oklab(r, g, b);
        }
        t
    })
}

/// OKLab L below which mapping to `Black` is legitimate (near-black
/// inputs). Inputs at or above this luminance are visibly lit and must
/// not collapse to an invisible black-on-black glyph.
const BLACK_COLLAPSE_FLOOR_L: f32 = 0.15;

// ── Nearest searches ────────────────────────────────────────────────────────

/// Exact OKLab-nearest xterm-256 palette index for an sRGB color.
///
/// Returns an index in `16..=255`. Entries `0..=15` are skipped: they
/// alias the host-configurable base-16 slots, so the cube+gray region
/// is the only stable part of the palette.
///
/// Cost: 240 squared-distance evaluations (~2-4 µs) — callers on the
/// emission hot path go through [`SgrQuantizer`], which memoizes.
#[must_use]
pub(crate) fn xterm256_nearest(r: u8, g: u8, b: u8) -> u8 {
    let (l0, a0, b0) = srgb_to_oklab(r, g, b);
    let table = oklab256();
    let mut best = 0usize;
    let mut best_d = f32::INFINITY;
    for (i, &(l, a, b)) in table.iter().enumerate() {
        let dl = l - l0;
        let da = a - a0;
        let db = b - b0;
        let d = dl * dl + da * da + db * db;
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    16 + best as u8
}

/// OKLab-nearest of the 16 canonical base colors, as a named `Color`.
///
/// Applies the anti-collapse readability floor: a visibly-lit input
/// (L ≥ [`BLACK_COLLAPSE_FLOOR_L`]) never resolves to `Black`.
#[must_use]
pub(crate) fn classic16_nearest(r: u8, g: u8, b: u8) -> Color {
    let (l0, a0, b0) = srgb_to_oklab(r, g, b);
    let best = nearest16_slot(l0, a0, b0, 0..16);
    if best == 0 && l0 >= BLACK_COLLAPSE_FLOOR_L {
        // Visibly-lit input would die as Black — re-run over the
        // non-Black slots. This only ever moves the result upward in
        // luminance, so it cannot invert monotone gradients.
        NAMED16[nearest16_slot(l0, a0, b0, 1..16)]
    } else {
        NAMED16[best]
    }
}

/// Squared-OKLab-distance argmin over `range` of the base-16 table.
fn nearest16_slot(l0: f32, a0: f32, b0: f32, range: std::ops::Range<usize>) -> usize {
    let table = oklab16();
    let mut best = range.start;
    let mut best_d = f32::INFINITY;
    for i in range {
        let (l, a, b) = table[i];
        let dl = l - l0;
        let da = a - a0;
        let db = b - b0;
        let d = dl * dl + da * da + db * db;
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

/// xterm base-16 slot of a crossterm named color, if it is one of the 16.
#[must_use]
pub(crate) fn named16_slot(color: Color) -> Option<u8> {
    let slot = match color {
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

// ── Memoized boundary quantizer ─────────────────────────────────────────────

/// Integer hasher for the memo's packed RGB keys.
///
/// std's default hasher (SipHash-1-3, randomly seeded) exists to defeat
/// hash-flooding; the memo's keys are internal packed u32s
/// (`r<<16 | g<<8 | b | is_bg<<24`), not attacker-controlled input.
/// Hashing such a key with SipHash costs ~15-25ns per steady-state
/// emission lookup — measurable on the style-change path of every
/// non-truecolor frame.
///
/// The mix is the splitmix64 finalizer (golden-ratio add, then two
/// multiply+xorshift stages). A plain multiply is NOT enough here:
/// ladder keys share low-bit structure — a green brightness sweep
/// `(0, g, 0)` packs to `g<<8`, which keeps 8 trailing zero bits under
/// any multiply, and hashbrown's bucket index is the hash's LOW bits,
/// so the whole ladder would collide into one bucket. The finalizer
/// avalanches high bits into low bits (pinned by test — see
/// `packed_key_hasher_spreads_clustered_ladder_keys`), and every stage
/// is a bijection, so distinct keys keep distinct full hashes.
#[derive(Default)]
struct PackedKeyHasher(u64);

impl Hasher for PackedKeyHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    /// Never hit for u32 keys (`<u32 as Hash>::hash` calls `write_u32`),
    /// but kept correct so the hasher stays valid if the key type ever
    /// widens — mixes FNV-style over the bytes.
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
        }
    }

    #[inline]
    fn write_u32(&mut self, v: u32) {
        let mut z = u64::from(v).wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        self.0 = z ^ (z >> 31);
    }
}

/// The quantizer memo's map type: std SwissTable probing over a
/// hash-free integer key. `BuildHasherDefault<PackedKeyHasher>` means
/// `Default` — no per-instance state to construct.
type MemoMap = HashMap<u32, Color, BuildHasherDefault<PackedKeyHasher>>;

/// Initial memo capacity.
///
/// Covers the typical warm-set (7-20 palette colors × ~16-32 brightness
/// ladder steps ≈ 112-640 entries) so the first frames after palette
/// build pay no grow-and-rehash chain.
const MEMO_INITIAL_CAPACITY: usize = 256;

/// Mode-aware RGB quantizer for an SGR emission boundary, with a flat
/// memo so each distinct input color pays the OKLab scan exactly once.
///
/// One instance lives in `Terminal` (interactive renderer) and one in
/// `BenchIoWriter` (benchmark I/O path) whenever the session resolved a
/// non-truecolor mode; `None` (no quantizer) means truecolor and keeps
/// the hot path byte-identical to the pre-task-17 wire format.
pub(crate) struct SgrQuantizer {
    mode: SgrMode,
    /// Packed `(r<<16 | g<<8 | b)` keyed results. Bit 24 marks the
    /// background direction (fg and bg quantize differently in Mono).
    /// Integer-hashed ([`PackedKeyHasher`]) — see the module-level
    /// Memoization notes.
    memo: MemoMap,
}

impl SgrQuantizer {
    /// Build a quantizer for `mode`. `SgrMode::TrueColor` instances are
    /// never constructed on hot paths — callers skip the quantizer.
    pub(crate) fn new(mode: SgrMode) -> Self {
        SgrQuantizer {
            mode,
            memo: MemoMap::with_capacity_and_hasher(
                MEMO_INITIAL_CAPACITY,
                BuildHasherDefault::default(),
            ),
        }
    }

    /// Quantize a cell's foreground for the wire.
    ///
    /// `None` (blank cell) and `Color::Reset` (explicit default) pass
    /// through untouched — they are semantic defaults, not colors.
    pub(crate) fn quantize_fg(&mut self, fg: Option<Color>) -> Option<Color> {
        fg.map(|c| self.quantize_one(c, false))
    }

    /// Quantize a cell's background for the wire.
    pub(crate) fn quantize_bg(&mut self, bg: Option<Color>) -> Option<Color> {
        bg.map(|c| self.quantize_one(c, true))
    }

    /// Quantize one non-Option color in the given direction.
    fn quantize_one(&mut self, color: Color, is_bg: bool) -> Color {
        match color {
            Color::Reset => color,
            Color::Rgb { r, g, b } => self.quantize_rgb(r, g, b, is_bg),
            Color::AnsiValue(v) => match self.mode {
                // Already palette-space for 256; decode for the rest.
                SgrMode::TrueColor | SgrMode::Ansi256 => color,
                SgrMode::Classic16 => {
                    if v < 16 {
                        NAMED16[v as usize]
                    } else {
                        let (r, g, b) = crate::palette::color_to_rgb(color);
                        self.quantize_rgb(r, g, b, is_bg)
                    }
                }
                SgrMode::Mono => mono_wire_color(is_bg),
            },
            // Named colors: classic-space for Classic16; indexed for
            // Ansi256; decoded to RGB for TrueColor (palette build path
            // only — the Terminal boundary never holds a TrueColor
            // quantizer).
            named => match self.mode {
                SgrMode::Classic16 => named,
                SgrMode::Ansi256 => named16_slot(named).map(Color::AnsiValue).unwrap_or(named),
                SgrMode::Mono => mono_wire_color(is_bg),
                SgrMode::TrueColor => {
                    let (r, g, b) = crate::palette::color_to_rgb(named);
                    Color::Rgb { r, g, b }
                }
            },
        }
    }

    /// Memoized RGB → wire color for the active mode.
    ///
    /// Mono short-circuits before the map: the result is a constant of
    /// `is_bg` alone (`White` fg / `Reset` bg), so a memo lookup (and a
    /// per-distinct-color insert, growing the map without bound across
    /// phosphor-decay gradients) would be pure overhead for a constant
    /// function.
    fn quantize_rgb(&mut self, r: u8, g: u8, b: u8, is_bg: bool) -> Color {
        if let SgrMode::Mono = self.mode {
            return mono_wire_color(is_bg);
        }
        let key = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32) | ((is_bg as u32) << 24);
        if let Some(&cached) = self.memo.get(&key) {
            return cached;
        }
        let quantized = match self.mode {
            SgrMode::Ansi256 => Color::AnsiValue(xterm256_nearest(r, g, b)),
            SgrMode::Classic16 => classic16_nearest(r, g, b),
            SgrMode::Mono => mono_wire_color(is_bg),
            SgrMode::TrueColor => Color::Rgb { r, g, b },
        };
        self.memo.insert(key, quantized);
        quantized
    }

    /// Number of memoized results (test observability only).
    #[cfg(test)]
    pub(crate) fn memo_len(&self) -> usize {
        self.memo.len()
    }
}

/// The Mono wire color: `White` for fg (a single honest color), `Reset`
/// for bg (default background — never paint on dumb terminals).
fn mono_wire_color(is_bg: bool) -> Color {
    if is_bg {
        Color::Reset
    } else {
        Color::White
    }
}

#[cfg(test)]
#[path = "../../../../test/engine/chroma_dragon_engine/palette/tests_quantize.rs"]
mod tests_quantize;
