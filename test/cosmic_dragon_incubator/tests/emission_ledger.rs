// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The emission-minimality certificate (INV-22, night-frontier-Z9-C).
//!
//! The terminal module's own docs claim the differential render path
//! "produces minimal cursor movement and style-change overhead" — that
//! was prose. This module turns it into a CONTRACT with three layers,
//! all driving the EXACT shipped code (no copy, no re-implementation):
//!
//! 1. The paint accounting identity: the shipped diff solver paints
//!    EXACTLY the cells whose content changed — its payload is the
//!    changed cells' UTF-8 glyph bytes, no more, no less (asserted
//!    exact, every frame, every scenario).
//! 2. The law-reference bound: an INDEPENDENT cost walker derived from
//!    the solver's named law set — (a) no overpaint (HUNT-27: a cell
//!    whose content already matches the screen is never emitted), (b)
//!    row-boundary cursor resync (bug-12: the cursor position drops at
//!    every row change), (c) the bridge grammar (pre-formatted
//!    fg+bg SGR forms through the task-17 emission-boundary quantizer,
//!    BOLT bold escapes) — computes the byte minimum any plan within
//!    those laws can achieve, cell by cell, from the same two screen
//!    states. The shipped solver must land in
//!    `[reference, 1.05 x reference]` on every adversarial grid, every
//!    wire mode, and every real rig frame. The reference is derived
//!    from the LAW LIST, not from the solver's code, so a solver
//!    regression that leaves the optimum moves the measured bytes away
//!    from an independently computed floor and CI goes red.
//! 3. The teeth: deliberately pessimized emitters (per-cell cursor
//!    addresses, per-cell style bridges, law-violating overpaint) must
//!    FAIL the same bound the shipped solver passes — a tripwire that
//!    cannot fire is decoration.
//!
//! The lean writer (the bench-parity emission, `--bench-io`) carries
//! its own byte-exact accounting identity across the 14-style ledger
//! rig: the writer's counter must equal an independent recomposition
//! of its deterministic emission shape, frame by frame.
//!
//! Honest scope: the certificate binds the shipped solver to optimality
//! WITHIN the named law set. The laws are correctness contracts — the
//! no-overpaint law is what keeps an idle resync byte-silent (the
//! HUNT-27 glitch fix), so plans that paint unchanged cells are not
//! legal for this solver regardless of their byte count. Residuals
//! outside the named grammar (a 4-byte `ESC[0m` bridge to the default
//! style where the grammar emits `ESC[39;49m`; mono wire-redundant
//! bridges whose wire state did not move) are named in the ledger, not
//! exploited: adopting one is a deliberate solver retune that
//! re-derives this certificate in the same commit.

use crossterm::style::Color;

use crate::bolt::BOLD_ESCAPE_LENS;
use crate::cell::Cell;
use crate::color_cache::ColorCache;
use crate::engine::cosmic_dragon_engine::terminal::emit_diff::{
    DiffEmissionStats, encode_diff_emission,
};
use crate::frame::Frame;
use crate::palette::{Palette, SgrMode, SgrQuantizer};
use crate::rain_style::RainStyle;
use crate::sgr_format::write_sgr_colors_buf;

use super::alloc_ledger::{LEDGER_FRAMES, LEDGER_WARMUP_FRAMES, build_ledger_rig_for_style};

/// The adversarial certificate grid: 20x6 — big enough to exercise row
/// boundaries, cursor reuse, and multi-run structure; small enough that
/// every scenario is hand-checkable. The approved certificate shape.
const GRID_COLS: u16 = 20;
const GRID_LINES: u16 = 6;

/// The approved band: shipped <= 1.05 x reference.
const BAND_NUM: u64 = 105;
const BAND_DEN: u64 = 100;

/// Decimal digit count of `n` (the CUP cost model needs the exact
/// wire length of `push_u16`).
fn dec_len(mut n: u16) -> usize {
    let mut d = 1;
    while n >= 10 {
        n /= 10;
        d += 1;
    }
    d
}

/// Wire length of the solver's absolute cursor address `ESC[y;xH`.
fn cup_len(y: u16, x: u16) -> usize {
    2 + dec_len(y + 1) + 1 + dec_len(x + 1) + 1
}

/// Wire length of the bridge grammar for (fg, bg) under `mode`:
/// `write_sgr_colors_buf` after the mode's emission-boundary
/// quantization — the byte shape both the cache hit and the miss path
/// produce (the cache pre-formats through the same function).
fn bridge_len(
    mode: Option<SgrMode>,
    quant: &mut Option<SgrQuantizer>,
    scratch: &mut Vec<u8>,
    fg: Option<Color>,
    bg: Option<Color>,
) -> usize {
    let (fg, bg) = match (mode, quant.as_mut()) {
        (Some(_), Some(q)) => (q.quantize_fg(fg), q.quantize_bg(bg)),
        _ => (fg, bg),
    };
    scratch.clear();
    write_sgr_colors_buf(scratch, fg, bg);
    scratch.len()
}

/// Build the mode-specific quantizer the emission boundary would carry
/// (`None` on truecolor sessions — the hot path stays byte-identical
/// to the pre-task-17 wire format).
fn quantizer_for(mode: Option<SgrMode>) -> Option<SgrQuantizer> {
    mode.map(SgrQuantizer::new)
}

/// The law reference: the byte minimum any plan within the solver's
/// named law set can achieve for (prev -> frame). Derived from the law
/// list, NOT from the solver's code: walk the changed cells in the
/// scanline order the row-resync law forces, and charge every changed
/// cell exactly what the grammar charges — a cursor address when the
/// cursor is not already there, a bridge when the raw style differs,
/// a bold toggle when the flag differs, the glyph's UTF-8 bytes — plus
/// the trailing attribute reset every frame carries.
///
/// Returns (total bytes, payload bytes) so the paint accounting
/// identity (layer 1) and the byte bound (layer 2) share one walk.
fn law_reference(prev: &[Cell], frame: &Frame, mode: Option<SgrMode>) -> (usize, usize) {
    let w = frame.width as usize;
    let total_cells = w * frame.height as usize;
    let mut quant = quantizer_for(mode);
    let mut scratch = Vec::with_capacity(32);

    let mut cur_fg: Option<Color> = None;
    let mut cur_bg: Option<Color> = None;
    let mut cur_bold = false;
    let mut cur_pos: Option<(u16, u16)> = None;
    let mut last_row: u16 = u16::MAX;
    let mut total = 0usize;
    let mut payload = 0usize;

    for (idx, prev_cell) in prev.iter().enumerate().take(total_cells) {
        let cell = frame.cell_at_index_ref(idx);
        // Law (a): no overpaint — a cell whose content already matches
        // the screen is not part of any legal plan.
        if *cell == *prev_cell {
            continue;
        }
        let y = (idx / w) as u16;
        let x = (idx % w) as u16;
        // Law (b): the cursor position drops at every row change.
        if y != last_row {
            cur_pos = None;
            last_row = y;
        }
        if cur_pos != Some((x, y)) {
            total += cup_len(y, x);
        }
        if cell.fg != cur_fg || cell.bg != cur_bg {
            total += bridge_len(mode, &mut quant, &mut scratch, cell.fg, cell.bg);
            cur_fg = cell.fg;
            cur_bg = cell.bg;
        }
        if cell.bold != cur_bold {
            total += BOLD_ESCAPE_LENS[cell.bold as usize];
            cur_bold = cell.bold;
        }
        let glyph = cell.ch.len_utf8();
        total += glyph;
        payload += glyph;
        cur_pos = if x + 1 < frame.width {
            Some((x + 1, y))
        } else {
            None
        };
    }
    // The trailing attribute reset every frame carries.
    (total + 4, payload)
}

/// The approved band: shipped within [reference, 1.05 x reference].
fn within_band(shipped: usize, reference: usize) -> bool {
    shipped >= reference && (shipped as u64) * BAND_DEN <= (reference as u64) * BAND_NUM
}

fn cell_of(ch: char, fg: Option<Color>, bg: Option<Color>, bold: bool) -> Cell {
    Cell { ch, fg, bg, bold }
}

/// Build one scenario on ONE persistent Frame object - the real loop
/// renders successive frames into the same buffer, so a cell cleared
/// back to blank is charged to the dirty list by the equality check
/// against the real previous content (a fresh-frame construction would
/// silently lose exactly those clears). Returns the previous screen's
/// cell snapshot and the target frame with its real dirty list.
fn build_scenario(
    prev_paints: &[(u16, u16, Cell)],
    target_paints: &[(u16, u16, Cell)],
) -> (Vec<Cell>, Frame) {
    let mut frame = Frame::new(GRID_COLS, GRID_LINES, None);
    // A fresh frame is dirty_all=true by construction (the first frame
    // is a full redraw): establish the previous screen under that flag
    // (content lands, the list stays empty), then open the steady-state
    // dirty tracking and advance to the target screen.
    for &(x, y, c) in prev_paints {
        frame.set(x, y, c);
    }
    frame.clear_dirty();
    let prev: Vec<Cell> = (0..(GRID_COLS as usize * GRID_LINES as usize))
        .map(|i| *frame.cell_at_index_ref(i))
        .collect();
    for &(x, y, c) in target_paints {
        frame.set(x, y, c);
    }
    (prev, frame)
}

/// Drive the shipped diff solver over one (prev -> target) diff and
/// return its stats. The shadow starts as the previous screen; the
/// solver consumes the target frame's dirty set exactly as draw() does.
fn drive_solver(
    prev: &[Cell],
    target: &Frame,
    mode: Option<SgrMode>,
    cache: Option<&ColorCache>,
) -> DiffEmissionStats {
    eprintln!(
        "DEBUG drive_solver: dirty={} prev_len={}",
        target.dirty_indices().len(),
        prev.len()
    );
    let mut shadow = prev.to_vec();
    let mut quant = quantizer_for(mode);
    let mut out = Vec::with_capacity(4096);
    let mut run_buf = String::new();
    let mut dirty_flat = Vec::new();
    encode_diff_emission(
        target,
        &mut shadow,
        quant.as_mut(),
        cache,
        &mut out,
        &mut run_buf,
        &mut dirty_flat,
    )
}

// ── The scenario palette ────────────────────────────────────────────────
// Real Rgb colors (the matrix-rain hot case), so every quantizer mode
// does real work; the cached variant builds a real ColorCache over the
// same palette the production path would carry.
const SC_A: Color = Color::Rgb {
    r: 0,
    g: 255,
    b: 70,
};
const SC_B: Color = Color::Rgb {
    r: 90,
    g: 160,
    b: 255,
};
const SC_C: Color = Color::Rgb {
    r: 255,
    g: 120,
    b: 40,
};

fn scenario_palette() -> Palette {
    Palette {
        colors: vec![SC_A, SC_B, SC_C],
        bg: Some(Color::Black),
    }
}

/// One adversarial scenario: (name, previous-screen paints, target
/// paints).
type Scenario = (&'static str, Vec<(u16, u16, Cell)>, Vec<(u16, u16, Cell)>);

/// The adversarial scenario set. The target paints the FULL screen content, so cells
/// that persist from prev ride dirty-but-unchanged.
fn scenarios() -> Vec<Scenario> {
    let blank = cell_of(' ', None, None, false);
    let a = cell_of('0', Some(SC_A), None, false);
    let a_b = cell_of('1', Some(SC_A), None, true);
    let b = cell_of('0', Some(SC_B), None, false);
    let c = cell_of('#', Some(SC_C), None, false);
    let mut v = Vec::new();

    // Sparse single cell: one CUP, one bridge, one glyph.
    v.push(("sparse_single", vec![], vec![(10, 2, a)]));

    // Contiguous same-style run: one CUP, one bridge, five glyphs.
    v.push((
        "contiguous_run",
        vec![],
        (5..10).map(|x| (x, 3, a)).collect(),
    ));

    // Same-style runs separated by an unchanged gap: the cursor law.
    v.push(("gap_same_style", vec![], vec![(5, 3, a), (7, 3, a)]));

    // Alternating styles: the bridge cadence.
    v.push((
        "alternating_styles",
        vec![],
        (2..9)
            .map(|x| (x, 1, if x % 2 == 0 { a } else { b }))
            .collect(),
    ));

    // Row boundary crossing: the resync law (cursor drop at row change).
    v.push(("row_boundary", vec![], vec![(18, 0, a), (1, 1, a)]));

    // Style sweep: twenty distinct colors on one row.
    v.push((
        "style_sweep_row",
        vec![],
        (0..GRID_COLS)
            .map(|x| {
                (
                    x,
                    4,
                    cell_of(
                        'x',
                        Some(Color::Rgb {
                            r: (x as u8) * 12,
                            g: 40 + (x as u8) * 9,
                            b: 255 - (x as u8) * 11,
                        }),
                        None,
                        false,
                    ),
                )
            })
            .collect(),
    ));

    // Multibyte glyphs: the payload is the UTF-8 length.
    v.push((
        "multibyte_glyphs",
        vec![],
        vec![
            (4, 2, cell_of('\u{e9}', Some(SC_A), None, false)),
            (5, 2, cell_of('\u{fc}', Some(SC_A), None, false)),
        ],
    ));

    // Bold toggles on one color: the BOLT cadence.
    v.push((
        "bold_toggles",
        vec![],
        vec![(3, 5, a), (4, 5, a_b), (5, 5, a), (6, 5, a_b)],
    ));

    // Prev content that persists AND changes: mixed screen.
    v.push((
        "mixed_persist_and_change",
        vec![(2, 2, a), (10, 2, b), (16, 2, c)],
        vec![
            (2, 2, a),      // persists (dirty-but-unchanged)
            (10, 2, c),     // changes color
            (16, 2, blank), // cleared
            (6, 4, a_b),    // new bold cell
        ],
    ));

    // Dirty-only no-change: the skip law must emit nothing but the
    // frame's trailing reset.
    v.push(("dirty_only_no_change", vec![(8, 2, a)], vec![(8, 2, a)]));

    v
}

/// The wire variants every scenario runs under: truecolor (no
/// quantizer, no cache — the raw grammar), mono, 256-index, classic-16,
/// and truecolor through a real ColorCache (the palette-hit path).
fn wire_variants() -> Vec<(&'static str, Option<SgrMode>, Option<ColorCache>)> {
    vec![
        ("truecolor", None, None),
        ("mono", Some(SgrMode::Mono), None),
        ("ansi256", Some(SgrMode::Ansi256), None),
        ("classic16", Some(SgrMode::Classic16), None),
        (
            "truecolor_cached",
            None,
            Some(ColorCache::new(&scenario_palette())),
        ),
    ]
}

/// INV-22 layer 1 + 2: on every adversarial scenario, under every wire
/// variant, the shipped diff solver paints EXACTLY the changed cells
/// (payload identity) and lands inside [reference, 1.05 x reference]
/// against the independent law reference.
#[test]
fn lock_inv22_diff_solver_is_byte_optimal_on_adversarial_grids() {
    for (name, prev_paints, target_paints) in scenarios() {
        let (prev_cells, target_frame) = build_scenario(&prev_paints, &target_paints);
        for (wire, mode, cache) in wire_variants() {
            let stats = drive_solver(&prev_cells, &target_frame, mode, cache.as_ref());
            let (reference, reference_payload) = law_reference(&prev_cells, &target_frame, mode);

            assert_eq!(
                stats.payload_bytes, reference_payload,
                "scenario {name} wire {wire}: the solver painted {} payload bytes \
                 where the changed cells carry {reference_payload} - the paint \
                 accounting identity is broken",
                stats.payload_bytes
            );
            assert!(
                within_band(stats.bytes, reference),
                "scenario {name} wire {wire}: the shipped diff solver emitted {} \
                 bytes where the law reference computes {reference} - outside the \
                 [reference, 1.05 x reference] band",
                stats.bytes
            );
        }
    }
}

/// INV-22 layer 1 on the real rain window: through the pinned ledger
/// rig (the INV-18 shape, 200 warmup + 180 anchored frames), the
/// shipped solver's payload equals the changed cells' glyph bytes every
/// frame, and its byte total stays inside the law band every frame —
/// the certificate runs on REAL frames, not only hand-built grids.
#[test]
fn lock_inv22_diff_solver_matches_the_law_reference_on_the_rig_window() {
    let (mut cloud, mut frame, _writer) = build_ledger_rig_for_style(RainStyle::Monolith);
    let target_period = std::time::Duration::from_secs_f64(1.0 / 60.0);

    // The emission boundary the production path carries: the rig's
    // palette mode decides whether a quantizer arms (task-17).
    let mode = match ColorCache::new(&cloud.palette).sgr_mode() {
        SgrMode::TrueColor => None,
        m => Some(m),
    };

    let mut sim_now = std::time::Instant::now();
    for _ in 0..LEDGER_WARMUP_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        frame.clear_dirty();
    }

    let mut checked = 0u64;
    for _ in 0..LEDGER_FRAMES {
        sim_now += target_period;
        let prev: Vec<Cell> = (0..frame.cells.len())
            .map(|i| *frame.cell_at_index_ref(i))
            .collect();
        cloud.rain_at(&mut frame, sim_now);

        let mut quant = quantizer_for(mode);
        let mut out = Vec::with_capacity(8192);
        let mut run_buf = String::new();
        let mut dirty_flat = Vec::new();
        let stats = encode_diff_emission(
            &frame,
            &mut shadow_cell_sink(&prev),
            quant.as_mut(),
            None,
            &mut out,
            &mut run_buf,
            &mut dirty_flat,
        );
        let (reference, reference_payload) = law_reference(&prev, &frame, mode);

        assert_eq!(
            stats.payload_bytes, reference_payload,
            "rig frame {checked}: the solver painted {} payload bytes where the \
             changed cells carry {reference_payload}",
            stats.payload_bytes
        );
        assert!(
            within_band(stats.bytes, reference),
            "rig frame {checked}: solver {} bytes vs law reference {reference} - \
             outside the band",
            stats.bytes
        );
        frame.clear_dirty();
        checked += 1;
    }
    assert_eq!(checked, LEDGER_FRAMES);
}

/// Helper: the solver takes the shadow as `&mut [Cell]`; give it a
/// fresh copy per frame (the rig test's shadow must track the frame,
/// and the law reference reads the pre-frame snapshot independently).
fn shadow_cell_sink(prev: &[Cell]) -> Vec<Cell> {
    prev.to_vec()
}

/// The independent recomposition of the lean writer's per-frame byte
/// shape: dirty cells in push order (all cells on dirty-all), one
/// style cursor across the frame, a bridge when the raw style differs,
/// a bold toggle when the flag differs, the glyph's UTF-8 bytes, and
/// the trailing reset - EXCEPT on idle frames (empty dirty list with
/// dirty_all=false), where the writer early-returns and emits nothing
/// at all.
fn lean_frame_expected(
    frame: &Frame,
    mode: Option<SgrMode>,
    quant: &mut Option<SgrQuantizer>,
    scratch: &mut Vec<u8>,
) -> u64 {
    let dirty_all = frame.is_dirty_all();
    let dirty = frame.dirty_indices();
    if dirty.is_empty() && !dirty_all {
        return 0;
    }
    let mut expected = 0u64;
    let mut cur_fg: Option<Color> = None;
    let mut cur_bg: Option<Color> = None;
    let mut cur_bold = false;
    if dirty_all {
        for idx in 0..frame.cells.len() {
            let cell = frame.cell_at_index_ref(idx);
            if cell.fg != cur_fg || cell.bg != cur_bg {
                expected += bridge_len(mode, quant, scratch, cell.fg, cell.bg) as u64;
                cur_fg = cell.fg;
                cur_bg = cell.bg;
            }
            if cell.bold != cur_bold {
                expected += BOLD_ESCAPE_LENS[cell.bold as usize] as u64;
                cur_bold = cell.bold;
            }
            expected += cell.ch.len_utf8() as u64;
        }
    } else {
        for &idx in dirty {
            let cell = frame.cell_at_index_ref(idx);
            if cell.fg != cur_fg || cell.bg != cur_bg {
                expected += bridge_len(mode, quant, scratch, cell.fg, cell.bg) as u64;
                cur_fg = cell.fg;
                cur_bg = cell.bg;
            }
            if cell.bold != cur_bold {
                expected += BOLD_ESCAPE_LENS[cell.bold as usize] as u64;
                cur_bold = cell.bold;
            }
            expected += cell.ch.len_utf8() as u64;
        }
    }
    expected + 4 // the trailing attribute reset
}

/// INV-22: the lean writer's byte accounting is EXACT across the
/// 14-style ledger rig — the writer's counter must equal an independent
/// recomposition of its deterministic emission shape (per dirty cell:
/// a bridge when the raw style differs from the cursor, a bold toggle
/// when the flag differs, the glyph's UTF-8 bytes; plus the trailing
/// reset), frame by frame. A writer-side byte drift of ANY size is a
/// named finding, not a silent change.
#[test]
fn lock_inv22_lean_writer_accounting_is_byte_exact_across_styles() {
    const STYLES: &[RainStyle] = &[
        RainStyle::Glyph,
        RainStyle::Monolith,
        RainStyle::Vortex,
        RainStyle::Flux,
        RainStyle::Lorenz,
        RainStyle::Dragon,
        RainStyle::Physarum,
        RainStyle::BlackHole,
        RainStyle::Aeolian,
        RainStyle::SolarFlare,
        RainStyle::DnaHelix,
        RainStyle::Murmuration,
        RainStyle::Quasar,
        RainStyle::Neural,
    ];

    for style in STYLES {
        let (mut cloud, mut frame, mut writer) = build_ledger_rig_for_style(*style);
        let target_period = std::time::Duration::from_secs_f64(1.0 / 60.0);

        // Mirror BenchIoWriter::build: the cache derives the wire mode
        // from the palette; a non-truecolor mode arms the quantizer.
        let mode = match ColorCache::new(&cloud.palette).sgr_mode() {
            SgrMode::TrueColor => None,
            m => Some(m),
        };
        let mut quant = quantizer_for(mode);
        let mut scratch = Vec::with_capacity(32);

        // The writer's counter is private and accumulates from
        // construction, so the certificate mirrors EVERY write_frame
        // call (warmup included) with the exact per-frame expectation
        // and compares one window total: an exact sum of exact frames
        // must equal the counter exactly - any per-frame drift of any
        // size moves the total.
        let mut sim_now = std::time::Instant::now();
        let mut expected_total = 0u64;
        for _ in 0..LEDGER_WARMUP_FRAMES + LEDGER_FRAMES {
            sim_now += target_period;
            cloud.rain_at(&mut frame, sim_now);
            expected_total += lean_frame_expected(&frame, mode, &mut quant, &mut scratch);
            writer.write_frame(&frame);
            frame.clear_dirty();
        }

        let metrics = writer.finalize(1.0);
        assert_eq!(
            metrics.bytes_written, expected_total,
            "style {style:?}: the lean writer's counter moved {} bytes where the \
             independent recomposition computes {expected_total} - the emission \
             shape drifted; re-derive the accounting in the same commit",
            metrics.bytes_written
        );
    }
}

// ── The teeth ───────────────────────────────────────────────────────────
// Three deliberately pessimized emitters must FAIL the same bound the
// shipped solver passes. Each pessimizer violates exactly one named
// law or grammar rule; if the certificate accepts any of them, it is
// decoration.

/// Per-cell cursor addresses (the cur_pos reuse law disabled): on the
/// contiguous-run scenario this adds a CUP per cell.
#[test]
fn lock_inv22_certificate_rejects_per_cell_cursor_pessimization() {
    let (prev_cells, target_frame) = build_scenario(
        &[],
        &(5..10)
            .map(|x| (x, 3, cell_of('0', Some(SC_A), None, false)))
            .collect::<Vec<_>>(),
    );
    let (reference, _) = law_reference(&prev_cells, &target_frame, None);

    // The pessimizer: a CUP before EVERY changed cell.
    let mut pessimized = 4; // the trailing reset
    for x in 5..10 {
        pessimized += cup_len(3, x);
        pessimized += bridge_len(None, &mut None, &mut Vec::new(), Some(SC_A), None);
        pessimized += 1;
    }
    assert!(
        !within_band(pessimized, reference),
        "per-cell CUP pessimizer ({pessimized} bytes vs reference {reference}) was \
         ACCEPTED - the band cannot catch cursor regressions"
    );
}

/// Per-cell style bridges (the style-cursor law disabled): on the
/// contiguous-run scenario this adds a full SGR per cell.
#[test]
fn lock_inv22_certificate_rejects_per_cell_bridge_pessimization() {
    let (prev_cells, target_frame) = build_scenario(
        &[],
        &(5..10)
            .map(|x| (x, 3, cell_of('0', Some(SC_A), None, false)))
            .collect::<Vec<_>>(),
    );
    let (reference, _) = law_reference(&prev_cells, &target_frame, None);

    // The pessimizer: the cursor law holds, but every cell emits a full
    // bridge regardless of the style state.
    let mut pessimized = 4;
    for x in 5..10 {
        if x == 5 {
            pessimized += cup_len(3, x);
        }
        pessimized += bridge_len(None, &mut None, &mut Vec::new(), Some(SC_A), None);
        pessimized += 1;
    }
    assert!(
        !within_band(pessimized, reference),
        "per-cell SGR pessimizer ({pessimized} bytes vs reference {reference}) was \
         ACCEPTED - the band cannot catch style-bridge regressions"
    );
}

/// Overpaint (the HUNT-27 no-overpaint law disabled): repaint the whole
/// screen regardless of changed cells - byte-flooding on sparse diffs.
#[test]
fn lock_inv22_certificate_rejects_overpaint_pessimization() {
    let (prev_cells, target_frame) =
        build_scenario(&[], &[(10, 2, cell_of('0', Some(SC_A), None, false))]);
    let (reference, _) = law_reference(&prev_cells, &target_frame, None);

    // The pessimizer: the full-redraw plan (every cell of the grid,
    // row-homed like the production full path) on a one-cell diff.
    let mut pessimized = 4;
    for y in 0..GRID_LINES {
        pessimized += cup_len(y, 0);
        for _x in 0..GRID_COLS {
            pessimized += bridge_len(None, &mut None, &mut Vec::new(), Some(SC_A), None);
            pessimized += 1;
        }
    }
    assert!(
        !within_band(pessimized, reference),
        "overpaint pessimizer ({pessimized} bytes vs reference {reference}) was \
         ACCEPTED - the band cannot catch law violations"
    );
}
