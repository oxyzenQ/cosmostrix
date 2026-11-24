// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The emission conformance law (night-netrunner-Z2, the zelynic
//! night-frontier-Z10 shape adapted to this estate's diff grammar).
//!
//! INV-22 (emission_ledger.rs) certifies the shipped diff solver's
//! BYTE ECONOMY: payload equals the changed cells' glyphs and the
//! total lands inside the law-reference band. A solver that emitted
//! garbage bytes inside that budget would still pass — byte economy
//! is not semantic correctness. This module closes exactly that gap
//! with the zelynic Z10 shape: a minimal strict VT state machine is
//! checked AGAINST the shipped solver's emission stream, and the
//! conformance law states that applying the emitted bytes to a model
//! of the previous screen RECONSTRUCTS the target screen — for every
//! frame pair the property lane can generate.
//!
//! The lemma decomposition (the zelynic night-frontier-Z12 lesson):
//! one monolithic session property over multi-frame streams is the
//! wall class that killed zelynic's first kani seat — the solver
//! circuit grows with the stream length and shrinks nothing. The
//! law is therefore split into three lemmas, each cheap, each
//! independently falsifiable, each pinning one contract:
//!
//! - L1 (the reconstruction lemma): for every (prev, target) pair,
//!   `apply_stream(vt(prev), encode_diff_emission(target)) == target`
//!   — the semantic theorem proper. The equality is on the FULL cell
//!   tuple (glyph, fg, bg, bold), not just the glyph: a style byte
//!   that fails to land is a visible corruption the same way a wrong
//!   glyph is.
//! - L2 (the idle identity lemma): an unchanged frame emits ZERO
//!   bytes — the HUNT-27 invisibility law restated semantically
//!   (INV-22 pins it on hand-built grids; here it is quantified over
//!   every generated prev screen, so a future solver that "helpfully"
//!   resyncs an idle screen dies on every input).
//! - L3 (the payload accounting lemma): the solver's payload counter
//!   equals the UTF-8 length of exactly the cells whose full tuple
//!   changed — the paint identity, quantified over generated screens
//!   instead of hand-built scenarios.
//!
//! Honest scope (the zelynic golden-vectors rule): the model is a
//! MODEL. It implements exactly the emission grammar the truecolor
//! diff path owns — CUP MoveTo, combined fg+bg SGR bridges
//! (`write_sgr_colors_buf` with no quantizer, no cache), the BOLT
//! bold toggles, glyph payloads, and the trailing attribute reset.
//! Truecolor-only by design: the quantized wire modes (Color256,
//! Color16, Mono) transform colors at the emission boundary, and
//! their conformance target is the QUANTIZED screen — that is the
//! task-17 contract's own surface, covered by INV-22's wire variants.
//! Any byte outside the grammar panics the model, so a future token
//! the model does not know fails this lane the same way it would
//! fail a real terminal: loudly.

use crossterm::style::Color;
use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence};

use crate::bolt::BOLD_ESCAPE_LENS;
use crate::cell::Cell;
use crate::engine::cosmic_dragon_engine::terminal::emit_diff::encode_diff_emission;
use crate::frame::Frame;

/// The conformance grid: small enough that proptest explores deep
/// multi-cell scenarios (wide runs, row boundaries, style churn),
/// large enough to exercise every structural branch of the solver:
/// contiguous runs, row-boundary resyncs, cursor reuse, and idle
/// clears back to blank.
const GRID_COLS: u16 = 12;
const GRID_LINES: u16 = 4;

/// The two-symbol alphabet (the zelynic domain shape): enough to form
/// multi-glyph runs that stress the RLE batching, small enough that
/// the generator spends its budget on LAYOUT, not glyphs.
const GLYPHS: [char; 3] = ['a', 'b', 'c'];

fn regression_store() -> Config {
    Config {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "test/cosmic_dragon_incubator/tests/emission_conformance-regressions.txt",
        ))),
        ..Config::default()
    }
}

// ── The model: a strict one-screen VT state machine ────────────────────

/// The modeled terminal: a cell grid, a cursor, and the current SGR
/// state. Strict by construction: `apply_token` panics on any byte
/// sequence outside the diff grammar, so the conformance law doubles
/// as a grammar pin — a new emission token must teach the model
/// deliberately, never silently.
struct VtModel {
    cells: Vec<Cell>,
    cols: u16,
    lines: u16,
    cursor: Option<(u16, u16)>,
    fg: Option<Color>,
    bg: Option<Color>,
    bold: bool,
}

impl VtModel {
    fn from_screen(prev: &[Cell], cols: u16, lines: u16) -> Self {
        Self {
            cells: prev.to_vec(),
            cols,
            lines,
            // The solver's emission assumes no cursor position at
            // frame start (its `cur_pos` starts None), no carried
            // style, bold off — the state the trailing reset of the
            // PREVIOUS frame left behind.
            cursor: None,
            fg: None,
            bg: None,
            bold: false,
        }
    }

    /// Write one glyph at the cursor and advance. Panics if the
    /// cursor is off-screen — the solver never moves past the last
    /// column without a fresh MoveTo (the run flush sets cursor None
    /// at end-of-line), so an off-screen write is a grammar violation.
    fn put_glyph(&mut self, ch: char) {
        let (x, y) = self
            .cursor
            .expect("glyph written with no cursor position — the solver must MoveTo first");
        assert!(
            x < self.cols && y < self.lines,
            "glyph written off-screen at ({x},{y})"
        );
        let idx = y as usize * self.cols as usize + x as usize;
        self.cells[idx] = Cell {
            ch,
            fg: self.fg,
            bg: self.bg,
            bold: self.bold,
        };
        self.cursor = if x + 1 < self.cols {
            Some((x + 1, y))
        } else {
            None
        };
    }

    fn move_to(&mut self, y: u16, x: u16) {
        assert!(
            y < self.lines && x < self.cols,
            "MoveTo off-screen ({y},{x})"
        );
        self.cursor = Some((x, y));
    }
}

/// Apply one emission stream to the model. Returns the final screen.
/// The parser is strict: it accepts exactly the tokens the truecolor
/// diff grammar emits and panics on anything else.
fn apply_stream(mut model: VtModel, stream: &[u8]) -> Vec<Cell> {
    let mut i = 0usize;
    while i < stream.len() {
        if stream[i] == 0x1b {
            match stream.get(i + 1) {
                Some(b'[') => {}
                other => panic!("escape with unknown introducer {other:?}"),
            }
            // Find the final byte of the CSI sequence.
            let rest = &stream[i + 2..];
            let end = rest
                .iter()
                .position(|b| (0x40..=0x7e).contains(b))
                .expect("unterminated CSI sequence in emission stream");
            let params = &rest[..end];
            let final_byte = rest[end];
            match final_byte {
                b'H' => {
                    // CUP: ESC[y;xH — exactly two decimal params.
                    let text =
                        std::str::from_utf8(params).expect("CUP params must be ASCII decimal");
                    let (y, x) = text.split_once(';').expect("CUP must carry row;col params");
                    let y: u16 = y.parse().expect("CUP row must be decimal");
                    let x: u16 = x.parse().expect("CUP col must be decimal");
                    model.move_to(y - 1, x - 1);
                }
                b'm' => {
                    // SGR: the combined fg+bg bridge, the bold toggles,
                    // or the trailing reset. The parameter list must
                    // match one of the grammar's shapes exactly.
                    let text =
                        std::str::from_utf8(params).expect("SGR params must be ASCII decimal");
                    if text == "0" {
                        // The trailing attribute reset: back to the
                        // same state a fresh frame assumes.
                        model.fg = None;
                        model.bg = None;
                        model.bold = false;
                    } else if text == "1" {
                        model.bold = true;
                    } else if text == "22" {
                        model.bold = false;
                    } else {
                        // The combined color bridge. Parse it with the
                        // SAME writer the solver uses (round-trip
                        // through the grammar's own formatter), then
                        // compare bytes — the model never re-derives
                        // color parsing by hand, it matches the
                        // canonical serialization.
                        let (fg, bg) = parse_sgr_bridge(text);
                        model.fg = fg;
                        model.bg = bg;
                    }
                }
                other => panic!("unknown CSI final byte {other:?}"),
            }
            i += 2 + end + 1;
        } else {
            // A UTF-8 glyph payload byte run: decode one char and put
            // it. The grammar emits complete glyphs, so the boundary
            // always lands between chars.
            let s = std::str::from_utf8(&stream[i..])
                .expect("glyph payload must be valid UTF-8 to end-of-stream or next ESC");
            let ch = s.chars().next().expect("empty glyph run");
            let len = ch.len_utf8();
            model.put_glyph(ch);
            i += len;
        }
    }
    model.cells
}

/// Parse one combined fg+bg SGR bridge (`38;2;r;g;b;48;2;r;g;b` and
/// its Reset/None sub-shapes) into the (fg, bg) pair. Strict: any
/// other parameter shape panics — the truecolor diff grammar emits
/// exactly these forms through `write_sgr_colors_buf`.
fn parse_sgr_bridge(text: &str) -> (Option<Color>, Option<Color>) {
    let mut buf = Vec::new();
    // Route through the writer the solver itself uses, then compare
    // against the canonical forms for each (fg, bg) candidate class.
    // The bridge grammar is: fg-part [; bg-part] where each part is
    // 38;2;r;g;b (truecolor), 39/49 (reset), or absent.
    let parts: Vec<&str> = text.split(';').collect();
    let mut fg = None;
    let mut bg = None;
    let mut i = 0usize;
    let mut saw_fg = false;
    let mut saw_bg = false;
    while i < parts.len() {
        match parts[i] {
            "38" => {
                assert!(
                    parts.get(i + 1) == Some(&"2"),
                    "only truecolor fg bridges ship"
                );
                let r: u8 = parts[i + 2].parse().expect("fg r");
                let g: u8 = parts[i + 3].parse().expect("fg g");
                let b: u8 = parts[i + 4].parse().expect("fg b");
                assert!(!saw_fg, "double fg in bridge");
                fg = Some(Color::Rgb { r, g, b });
                saw_fg = true;
                i += 5;
            }
            "48" => {
                assert!(
                    parts.get(i + 1) == Some(&"2"),
                    "only truecolor bg bridges ship"
                );
                let r: u8 = parts[i + 2].parse().expect("bg r");
                let g: u8 = parts[i + 3].parse().expect("bg g");
                let b: u8 = parts[i + 4].parse().expect("bg b");
                assert!(!saw_bg, "double bg in bridge");
                bg = Some(Color::Rgb { r, g, b });
                saw_bg = true;
                i += 5;
            }
            "39" => {
                assert!(!saw_fg, "double fg in bridge");
                saw_fg = true;
                i += 1;
            }
            "49" => {
                assert!(!saw_bg, "double bg in bridge");
                saw_bg = true;
                i += 1;
            }
            other => panic!("bridge parameter outside the truecolor grammar: {other:?}"),
        }
    }
    // Canonical round-trip: the writer must serialize this pair back
    // to the exact bytes the stream carried — the model and the
    // grammar share one definition.
    crate::sgr_format::write_sgr_colors_buf(&mut buf, fg, bg);
    let mut expected = Vec::new();
    expected.extend_from_slice(b"\x1b[");
    expected.extend_from_slice(text.as_bytes());
    expected.push(b'm');
    assert_eq!(
        buf, expected,
        "bridge {text:?} does not round-trip through the grammar's own writer"
    );
    (fg, bg)
}

// ── The generators ──────────────────────────────────────────────────────

/// A cell strategy over the conformance alphabet: three glyphs, the
/// truecolor palette's fg/bg arms, and both bold states.
fn cell_strategy() -> impl Strategy<Value = Cell> {
    (
        proptest::sample::select(GLYPHS.to_vec()),
        proptest::option::of(proptest::sample::select(vec![
            Color::Rgb {
                r: 0,
                g: 255,
                b: 70,
            },
            Color::Rgb {
                r: 90,
                g: 160,
                b: 255,
            },
            Color::Rgb {
                r: 255,
                g: 120,
                b: 40,
            },
        ])),
        proptest::option::of(proptest::sample::select(vec![
            Color::Rgb {
                r: 10,
                g: 10,
                b: 20,
            },
            Color::Rgb {
                r: 20,
                g: 12,
                b: 30,
            },
        ])),
        proptest::bool::ANY,
    )
        .prop_map(|(ch, fg, bg, bold)| Cell { ch, fg, bg, bold })
}

/// A full-screen paint list: 0..=GRID area cells placed by index.
/// Placing by index (not by strategy-unique coords) keeps duplicates
/// natural — later paints win, exactly like overlapping rain writes.
fn paint_list_strategy() -> impl Strategy<Value = Vec<(u16, u16, Cell)>> {
    proptest::collection::vec(
        (0u16..GRID_COLS, 0u16..GRID_LINES, cell_strategy()),
        0..(GRID_COLS as usize * GRID_LINES as usize),
    )
}

/// Build a (prev_screen, target_frame) pair the way the production
/// loop does: paint prev under the fresh frame's dirty_all, clear,
/// snapshot, then paint the target through the steady-state dirty
/// tracking (the same shape as emission_ledger::build_scenario).
fn build_pair(
    prev_paints: &[(u16, u16, Cell)],
    target_paints: &[(u16, u16, Cell)],
) -> (Vec<Cell>, Frame) {
    let mut frame = Frame::new(GRID_COLS, GRID_LINES, None);
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

/// Drive the shipped solver once and return (stats, stream, shadow).
fn solve(
    prev: &[Cell],
    target: &Frame,
) -> (
    crate::engine::cosmic_dragon_engine::terminal::emit_diff::DiffEmissionStats,
    Vec<u8>,
    Vec<Cell>,
) {
    let mut shadow = prev.to_vec();
    let mut out = Vec::with_capacity(4096);
    let mut run_buf = String::new();
    let mut dirty_flat = Vec::new();
    let stats = encode_diff_emission(
        target,
        &mut shadow,
        None,
        None,
        &mut out,
        &mut run_buf,
        &mut dirty_flat,
    );
    (stats, out, shadow)
}

proptest! {
    #![proptest_config(regression_store())]

    /// L1 — the reconstruction lemma: applying the emitted stream to
    /// a model of the previous screen yields EXACTLY the target
    /// screen, full cell tuple, for every generated frame pair.
    #[test]
    fn emission_stream_reconstructs_the_target_screen(
        prev_paints in paint_list_strategy(),
        target_paints in paint_list_strategy(),
    ) {
        let (prev, frame) = build_pair(&prev_paints, &target_paints);
        let (_stats, stream, _shadow) = solve(&prev, &frame);
        let target: Vec<Cell> = (0..(GRID_COLS as usize * GRID_LINES as usize))
            .map(|i| *frame.cell_at_index_ref(i))
            .collect();
        let model = VtModel::from_screen(&prev, GRID_COLS, GRID_LINES);
        let after = apply_stream(model, &stream);
        prop_assert_eq!(after, target);
    }

    /// L2 — the idle identity lemma, decomposed to the SOLVER layer
    /// (the Z12 shape: the caller's idle fast path and the solver's
    /// own emission are separate obligations). draw.rs's v30 fast path
    /// (INV-17, lock.rs) returns before the solver when the dirty
    /// list is empty, so the zero-BYTE claim lives there. This lemma
    /// pins what the SOLVER itself owes on an unchanged screen: the
    /// payload is empty, no cursor moves, no style bridges, no bold
    /// toggles — the only bytes are the 4-byte trailing attribute
    /// reset the grammar mandates every emission carries. For every
    /// generated previous screen (the HUNT-27 no-overpaint law,
    /// quantified over all inputs).
    #[test]
    fn idle_frame_emits_only_the_trailing_reset(
        prev_paints in paint_list_strategy(),
    ) {
        let (prev, _frame) = build_pair(&prev_paints, &[]);
        // Re-paint the exact same screen: same paints, same order.
        let (prev2, frame2) = build_pair(&prev_paints, &prev_paints);
        prop_assert_eq!(prev, prev2.clone());
        let (stats, stream, _shadow) = solve(&prev2, &frame2);
        prop_assert_eq!(
            stream,
            b"\x1b[0m".to_vec(),
            "idle solver emission must be exactly the trailing reset"
        );
        prop_assert_eq!(stats.payload_bytes, 0);
        prop_assert_eq!(stats.moves, 0);
        prop_assert_eq!(stats.sgrs, 0);
        prop_assert_eq!(stats.bolds, 0);
        prop_assert_eq!(stats.bytes, 4);
    }

    /// L3 — the payload accounting lemma: the solver's payload counter
    /// equals the UTF-8 byte length of exactly the cells whose full
    /// tuple changed (the paint identity, quantified).
    #[test]
    fn payload_counter_equals_changed_glyph_bytes(
        prev_paints in paint_list_strategy(),
        target_paints in paint_list_strategy(),
    ) {
        let (prev, frame) = build_pair(&prev_paints, &target_paints);
        let (stats, _stream, _shadow) = solve(&prev, &frame);
        let expected: usize = (0..(GRID_COLS as usize * GRID_LINES as usize))
            .filter(|&i| *frame.cell_at_index_ref(i) != prev[i])
            .map(|i| frame.cell_at_index_ref(i).ch.len_utf8())
            .sum();
        prop_assert_eq!(stats.payload_bytes, expected);
    }
}

// ── The BOLT table pin (shared with INV-22, restated for the model) ────

#[test]
fn bolt_bold_tables_match_the_model_grammar() {
    // The model's bold arms are the literal sequences; BOLD_ESCAPES is
    // what the solver emits. Pin them together so a table edit fails
    // the conformance lane, not a random terminal.
    assert_eq!(BOLD_ESCAPE_LENS[0], b"\x1b[22m".len());
    assert_eq!(BOLD_ESCAPE_LENS[1], b"\x1b[1m".len());
}
