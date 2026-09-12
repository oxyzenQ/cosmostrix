// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunter-34 regression tests: color-bg default-background
//! residue (owner report 2026-09-12, v100.0.0-beta.1).
//!
//! Symptom: with `color-bg = "default-background"` the physical screen
//! kept residue through every semantic event — intro rain glyphs stuck
//! after the logo cinematic, old-scene glyphs stuck after 'x'/'X',
//! pre-restart glyphs stuck after 'r', and black/custom background
//! cells stuck behind the moving rain after live-reload bg changes.
//! Under `color-bg = "black"` all four scenarios were clean.
//!
//! Root cause: `LastFrame::reuse_or_new` resets the shadow to
//! `Cell::blank_with_bg(None)` whenever the shadow is discarded
//! (semantic_gen mismatch, resize, first draw). The HUNT-27 cell-skip
//! in the full-redraw path then treats "frame blank == shadow blank"
//! as "nothing to emit" — but the physical screen still holds the old
//! content. `color-bg = "black"` masked the bug (frame blank
//! `bg = Some(black)` differs from the reset cell's `bg = None`, so
//! the full redraw repainted everything by accident); under
//! default-background the frame blank is also `bg = None`, the skip
//! fires, and the residue survives.
//!
//! Fix: the shadow carries `force_full_emit: bool` — every constructor
//! path sets it (the physical screen state is UNKNOWN after a reset),
//! `Terminal::draw` emits every cell once and then clears it. The
//! HUNT-27 idle-resync zero-emit optimization (force_repaint without a
//! semantic change — the shadow is preserved) is untouched.
//!
//! These unit tests pin the shadow-side flag contract; the four
//! end-to-end owner scenarios are verified by
//! `scripts/night_cbg34_e2e.py` (PTY + mini terminal emulator).

use super::LastFrame;
use crate::cell::Cell;

/// A fresh shadow (first draw after Terminal creation) must be in the
/// unknown state — the alternate screen may already hold content
/// (e.g. the intro drawing before the main loop) that the shadow has
/// never tracked.
#[test]
fn cbg34_fresh_shadow_is_unknown() {
    let shadow = LastFrame::new(80, 24);
    assert!(
        shadow.force_full_emit,
        "LastFrame::new must mark the physical screen state as unknown"
    );
}

/// The reuse path (same-or-smaller dimensions, capacity fits) discards
/// the old cell contents — the shadow no longer matches the physical
/// screen, so it must return to the unknown state. This is the exact
/// scene-switch / restart / live-reload path the owner's bugs hit.
#[test]
fn cbg34_reused_shadow_is_unknown() {
    let mut old = LastFrame::new(80, 24);
    // Simulate a tracked screen: paint a glyph into the shadow.
    old.cells[0] = Cell {
        ch: '|',
        fg: None,
        bg: None,
        bold: false,
    };
    old.semantic_gen = 7;
    old.force_full_emit = false;
    let reused = LastFrame::reuse_or_new(Some(old), 80, 24);
    assert!(
        reused.force_full_emit,
        "reuse_or_new must re-arm force_full_emit after discarding cell contents"
    );
    assert_eq!(
        reused.cells[0],
        Cell::blank_with_bg(None),
        "reuse path must reset cells to blank (pre-existing contract)"
    );
    assert_eq!(reused.semantic_gen, 0, "semantic_gen resets on reuse");
}

/// The fresh-allocation path (terminal grew beyond capacity) must also
/// arm the unknown flag — a resize leaves stale content at the new
/// edges that the blank shadow does not know about.
#[test]
fn cbg34_grown_shadow_is_unknown() {
    let old = LastFrame::new(80, 24);
    let grown = LastFrame::reuse_or_new(Some(old), 200, 60);
    assert!(
        grown.force_full_emit,
        "grown (fresh-alloc) shadow must mark the physical screen state as unknown"
    );
    assert_eq!(grown.width, 200);
    assert_eq!(grown.height, 60);
}

/// No-shadow fallback (`existing = None`) goes through `new` — same
/// unknown contract.
#[test]
fn cbg34_absent_shadow_is_unknown() {
    let fresh = LastFrame::reuse_or_new(None, 80, 24);
    assert!(fresh.force_full_emit);
}
