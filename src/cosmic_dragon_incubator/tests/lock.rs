// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Cosmic Dragon Engine Lock — v30
//!
//! Comprehensive invariant suite that **locks** the Cosmic Dragon diff-based
//! rendering engine at its v30 peak. Mirrors the Chroma Dragon lock suite
//! (`src/chroma_dragon_engine/tests/lock.rs`) — every public contract the render engine
//! guarantees is asserted here, so any future change that silently regresses
//! an invariant fails CI before it reaches `main`.
//!
//! ## What "lock" means
//!
//! The engine is at its peak: the double-buffered generation-based dirty
//! tracking (`dirty_gen` + `dirty_cell_gen`) eliminates the O(N) per-frame
//! memset, the `semantic_gen` invalidation counter eliminates stale-glyph
//! residue, the `/dev/tty` fallback recovers from broken stdout mid-run,
//! and the single-syscall flush + `ColorCache` SGR byte cache keep I/O tight.
//! Nothing else is on the roadmap for the render engine.
//!
//! "Lock" means: every invariant below MUST keep holding. If a future
//! commit changes the dirty-tracking contract, the diff threshold, the
//! semantic-gen protocol, or the SGR cache format in a way that breaks
//! one of these invariants, this module's tests fail and the commit is
//! rejected at review. The engine is **deliberately done**.
//!
//! ## Invariant inventory
//!
//! | ID    | Scope                | Assertion                                              |
//! |-------|----------------------|--------------------------------------------------------|
//! | INV-1 | Engine version       | `COSMIC_DRAGON_ENGINE_VERSION` matches the locked tag  |
//! | INV-2 | Frame clamp          | `Frame::new` clamps to `[4,1024]×[4,500]`              |
//! | INV-3 | Bench clamp          | `Frame::new_bench` clamps to `[4,7680]×[4,4320]`       |
//! | INV-4 | Dirty skip on equal  | `Frame::set` does NOT push to dirty when cell unchanged|
//! | INV-5 | Dirty push on change | `Frame::set` pushes exactly once when cell changes     |
//! | INV-6 | No duplicate dirty   | `Frame::set` does not push twice for same cell/frame   |
//! | INV-7 | clear_dirty is O(1)  | `clear_dirty` empties dirty list + bumps dirty_gen     |
//! | INV-8 | dirty_all flag       | `clear_with_bg` sets `dirty_all=true` + clears dirty   |
//! | INV-9 | Semantic invalidation| `invalidate_semantic` bumps `semantic_gen`             |
//! | INV-10| set_force bypasses   | `set_force` writes without equality check              |
//! | INV-11| gen overflow safety  | `clear_with_bg` u32 wrap resets `cell_gen` to 0        |
//! | INV-12| dirty_gen overflow   | `clear_dirty` u32 wrap resets `dirty_cell_gen` to 0    |
//! | INV-13| blank fallback       | `cell_at_index` returns blank when gen doesn't match   |
//! | INV-14| ColorCache Rgb       | `ColorCache` produces non-empty SGR bytes for Rgb      |
//! | INV-15| ColorCache miss      | `ColorCache::sgr_for_cell` returns None for unknown idx|
//! | INV-16| Lock report          | Sentinel test prints the engine report                 |
//! | INV-17| Idle-frame fast path | `clear_dirty` on empty frame advances `dirty_gen`      |
//!
//! ## Adding a new invariant
//!
//! If a future commit lands a new render engine feature (v31+), add a
//! new `INV-XX` test below following the existing pattern:
//!
//! 1. Document the invariant in the table above.
//! 2. Add a `#[test] fn lock_invXX_<short_name>()` function.
//! 3. Assert the contract with a synthetic Frame/ColorCache fixture.
//! 4. Bump `COSMIC_DRAGON_ENGINE_VERSION` if the invariant changes the
//!    engine's public contract.

use crossterm::style::Color;

use crate::cell::Cell;
use crate::color_cache::ColorCache;
use crate::constants::{
    BENCH_MAX_COLS, BENCH_MAX_LINES, MAX_TERMINAL_COLS, MAX_TERMINAL_LINES, MIN_TERMINAL_COLS,
    MIN_TERMINAL_LINES,
};
use crate::frame::Frame;
use crate::palette::Palette;

/// The Cosmic Dragon diff-based rendering engine version tag.
///
/// Bumped whenever a commit changes the engine's public contract
/// (new invariant, new dirty-tracking protocol, retuned threshold that
/// shifts render behavior). The lock test asserts this matches the
/// locked value — bumping it is the explicit "I know what I'm doing" signal.
///
/// History:
/// - `"1 (locked)"` — v30: engine locked. All invariants asserted.
///   The double-buffered generation dirty system, the semantic_gen
///   invalidation counter, the /dev/tty fallback, the single-syscall
///   flush, and the ColorCache SGR byte cache are all frozen contracts.
pub const COSMIC_DRAGON_ENGINE_VERSION: &str = "1 (locked)";

// ═══════════════════════════════════════════════════════════════════════════
// INV-1: Engine version sentinel
// ═══════════════════════════════════════════════════════════════════════════

/// INV-1: the engine version matches the locked tag.
///
/// This test exists so that any commit bumping the version must also touch
/// this file — making the contract change visible in `git blame` and
/// forcing the author to update the invariant inventory above.
#[test]
fn lock_inv01_engine_version_sentinel() {
    assert_eq!(
        COSMIC_DRAGON_ENGINE_VERSION, "1 (locked)",
        "Cosmic Dragon engine version drifted. If this was intentional, update the \
         COSMIC_DRAGON_ENGINE_VERSION history comment and the INV-1 sentinel together."
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-2 & INV-3: Frame constructor clamps
// ═══════════════════════════════════════════════════════════════════════════

/// INV-2: `Frame::new` clamps width/height to the interactive safety range
/// `[MIN_TERMINAL_COLS, MAX_TERMINAL_COLS] × [MIN_TERMINAL_LINES, MAX_TERMINAL_LINES]`.
///
/// This prevents OOM from absurd terminal sizes (e.g. 65535×65535 reported
/// by a buggy terminal emulator). 0×0 must be clamped to 4×4 (the minimum).
#[test]
fn lock_inv02_frame_new_clamps_to_interactive_bounds() {
    // 0×0 clamps to MIN×MIN.
    let f = Frame::new(0, 0, None);
    assert_eq!(f.width, MIN_TERMINAL_COLS);
    assert_eq!(f.height, MIN_TERMINAL_LINES);

    // u16::MAX × u16::MAX clamps to MAX × MAX.
    let f = Frame::new(u16::MAX, u16::MAX, None);
    assert_eq!(f.width, MAX_TERMINAL_COLS);
    assert_eq!(f.height, MAX_TERMINAL_LINES);

    // A normal size is unchanged.
    let f = Frame::new(80, 24, None);
    assert_eq!(f.width, 80);
    assert_eq!(f.height, 24);

    // A size just below MAX is unchanged.
    let f = Frame::new(MAX_TERMINAL_COLS - 1, MAX_TERMINAL_LINES - 1, None);
    assert_eq!(f.width, MAX_TERMINAL_COLS - 1);
    assert_eq!(f.height, MAX_TERMINAL_LINES - 1);
}

/// INV-3: `Frame::new_bench` clamps width/height to the benchmark range
/// `[MIN_TERMINAL_COLS, BENCH_MAX_COLS] × [MIN_TERMINAL_LINES, BENCH_MAX_LINES]`.
///
/// Benchmark mode allows larger frames (8K UHD = 7680×4320) for stress
/// testing, but still clamps to prevent OOM from genuinely absurd values.
#[test]
fn lock_inv03_frame_new_bench_clamps_to_bench_bounds() {
    let f = Frame::new_bench(0, 0, None);
    assert_eq!(f.width, MIN_TERMINAL_COLS);
    assert_eq!(f.height, MIN_TERMINAL_LINES);

    let f = Frame::new_bench(u16::MAX, u16::MAX, None);
    assert_eq!(f.width, BENCH_MAX_COLS);
    assert_eq!(f.height, BENCH_MAX_LINES);

    // 8K UHD is allowed.
    let f = Frame::new_bench(7680, 4320, None);
    assert_eq!(f.width, 7680);
    assert_eq!(f.height, 4320);
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-4, INV-5, INV-6: Dirty tracking contract
// ═══════════════════════════════════════════════════════════════════════════

/// INV-4: `Frame::set` does NOT push to the dirty list when the new cell
/// equals the current cell. This is the load-bearing equality-skip — without
/// it, every `set()` call would push to dirty and the diff renderer would
/// do unnecessary work.
#[test]
fn lock_inv04_set_skips_dirty_when_cell_unchanged() {
    let mut f = Frame::new(10, 10, None);
    f.clear_dirty();

    let cell = Cell {
        ch: 'x',
        fg: None,
        bg: None,
        bold: false,
    };
    f.set(5, 5, cell);
    assert_eq!(f.dirty_indices().len(), 1, "first set must push to dirty");

    // Same cell again — must NOT push a duplicate.
    f.set(5, 5, cell);
    assert_eq!(
        f.dirty_indices().len(),
        1,
        "set with unchanged cell must NOT push to dirty (equality-skip)"
    );
}

/// INV-5: `Frame::set` pushes exactly one entry to the dirty list when the
/// cell changes. Combined with INV-4, this gives the diff renderer a clean
/// "one dirty entry per changed cell" contract.
#[test]
fn lock_inv05_set_pushes_once_when_cell_changes() {
    let mut f = Frame::new(10, 10, None);
    f.clear_dirty();

    let cell_a = Cell {
        ch: 'a',
        fg: None,
        bg: None,
        bold: false,
    };
    let cell_b = Cell {
        ch: 'b',
        fg: None,
        bg: None,
        bold: false,
    };

    f.set(3, 3, cell_a);
    assert_eq!(f.dirty_indices(), &[33]);

    f.set(3, 3, cell_b);
    assert_eq!(
        f.dirty_indices(),
        &[33],
        "second set on same cell with new value must NOT add a duplicate dirty entry \
         (the cell is already marked dirty this frame)"
    );
}

/// INV-6: `Frame::set` does not push the same cell index twice in one frame
/// even if called many times with many different values. This is the
/// double-buffered dirty stamp at work: once `dirty_cell_gen[i] == dirty_gen`,
/// subsequent sets skip the push.
#[test]
fn lock_inv06_set_does_not_duplicate_dirty_entries() {
    let mut f = Frame::new(10, 10, None);
    f.clear_dirty();

    // Write 5 different values to the same cell — dirty list must still
    // contain exactly 1 entry for that cell.
    for i in 0..5u8 {
        f.set(
            2,
            2,
            Cell {
                ch: char::from(b'a' + i),
                fg: None,
                bg: None,
                bold: false,
            },
        );
    }
    assert_eq!(
        f.dirty_indices(),
        &[22],
        "5 writes to the same cell in one frame must produce exactly 1 dirty entry"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-7 & INV-8: clear_dirty + dirty_all
// ═══════════════════════════════════════════════════════════════════════════

/// INV-7: `Frame::clear_dirty` is O(1) — it bumps `dirty_gen` and clears the
/// dirty SmallVec's len. It does NOT iterate over `dirty_cell_gen`. This is
/// the load-bearing double-buffer trick: old stamps become stale instantly
/// without a memset.
///
/// We can't directly assert "O(1)" but we can assert the observable contract:
/// after `clear_dirty`, the dirty list is empty AND a subsequent write to a
/// previously-dirtied cell (with a NEW value) pushes it again — proving the
/// stamp was invalidated (otherwise the equality-skip would not fire because
/// the cell value differs, but the duplicate-dirty-skip WOULD fire if the
/// stamp still matched).
#[test]
fn lock_inv07_clear_dirty_empties_list_and_invalidates_stamps() {
    let mut f = Frame::new(10, 10, None);
    f.clear_dirty();

    // Dirty some cells with NON-blank values (blank==blank would be skipped
    // by the equality check in `set`).
    let live = Cell {
        ch: 'x',
        fg: None,
        bg: None,
        bold: false,
    };
    f.set(1, 1, live);
    f.set(2, 2, live);
    f.set(3, 3, live);
    assert_eq!(f.dirty_indices().len(), 3);

    // Clear — dirty list must be empty.
    f.clear_dirty();
    assert!(
        f.dirty_indices().is_empty(),
        "dirty list must be empty after clear_dirty"
    );

    // Writing a DIFFERENT value to a previously-dirtied cell must push it
    // again. If the dirty stamp had NOT been invalidated by clear_dirty,
    // the duplicate-dirty-skip (`dirty_cell_gen[i] != dirty_gen`) would
    // prevent the push even though the cell value changed. The push
    // succeeding proves the stamp was invalidated.
    let live2 = Cell {
        ch: 'y',
        fg: None,
        bg: None,
        bold: false,
    };
    f.set(1, 1, live2);
    assert_eq!(
        f.dirty_indices().len(),
        1,
        "after clear_dirty, writing a new value to a previously-dirtied cell must push it again \
         (stamp was invalidated by the generation bump)"
    );
}

/// INV-8: `Frame::clear_with_bg` sets `dirty_all=true` and clears the dirty
/// list. This is the semantic-reset path — the next render MUST do a full
/// redraw regardless of cell-level diff.
#[test]
fn lock_inv08_clear_with_bg_sets_dirty_all_and_clears_dirty() {
    let mut f = Frame::new(10, 10, None);
    f.clear_dirty();

    // Dirty some cells first (non-blank to bypass equality skip).
    let live = Cell {
        ch: 'x',
        fg: None,
        bg: None,
        bold: false,
    };
    f.set(1, 1, live);
    f.set(2, 2, live);
    assert!(!f.is_dirty_all());
    assert_eq!(f.dirty_indices().len(), 2);

    // clear_with_bg must set dirty_all + clear dirty.
    f.clear_with_bg(None);
    assert!(f.is_dirty_all(), "clear_with_bg must set dirty_all=true");
    assert!(
        f.dirty_indices().is_empty(),
        "clear_with_bg must clear the dirty list (full redraw makes per-cell dirty tracking moot)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-9: Semantic invalidation
// ═══════════════════════════════════════════════════════════════════════════

/// INV-9: `Frame::invalidate_semantic` bumps `semantic_gen` AND calls
/// `clear_with_bg` (which sets dirty_all). The Terminal's LastFrame cache
/// tracks `semantic_gen` — a mismatch forces a full redraw, eliminating
/// stale glyph residue from charset/theme/shading mutations.
#[test]
fn lock_inv09_invalidate_semantic_bumps_gen_and_sets_dirty_all() {
    let mut f = Frame::new(10, 10, None);
    f.clear_dirty();
    let gen_before = f.semantic_gen;
    assert!(!f.is_dirty_all());

    f.invalidate_semantic(None);

    assert_eq!(
        f.semantic_gen,
        gen_before.wrapping_add(1),
        "invalidate_semantic must bump semantic_gen by exactly 1"
    );
    assert!(
        f.is_dirty_all(),
        "invalidate_semantic must set dirty_all=true via clear_with_bg"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-10: set_force bypasses equality check
// ═══════════════════════════════════════════════════════════════════════════

/// INV-10: `Frame::set_force` writes the cell without doing the equality
/// comparison. This is used by callers who KNOW the cell changed (e.g.
/// monolith cleanup, quantum particles) and want to skip the 16-byte Cell
/// `==` check.
///
/// Test: set a cell, clear dirty, then `set_force` the SAME cell — the
/// force-set must still push to dirty (no equality-skip).
#[test]
fn lock_inv10_set_force_bypasses_equality_check() {
    let mut f = Frame::new(10, 10, None);
    f.clear_dirty();

    let cell = Cell {
        ch: 'x',
        fg: None,
        bg: None,
        bold: false,
    };
    f.set(5, 5, cell);
    f.clear_dirty();

    // set_force with the same cell — must still push to dirty.
    f.set_force(5, 5, cell);
    assert_eq!(
        f.dirty_indices(),
        &[55],
        "set_force must push to dirty even when the cell value is unchanged (no equality check)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-11 & INV-12: Generation overflow safety
// ═══════════════════════════════════════════════════════════════════════════

/// INV-11: When `Frame::clear_with_bg` causes `gen` to wrap past u32::MAX,
/// the overflow handler resets `cell_gen` to all-zeros and bumps `gen` to 1.
/// This prevents the case where a cell's `cell_gen` happens to equal the
/// wrapped `gen` and falsely appears "live".
///
/// At 60 FPS this fires once every ~2 years. Cost: one O(N) memset.
#[test]
fn lock_inv11_clear_with_bg_gen_overflow_resets_cell_gen() {
    let mut f = Frame::new(4, 4, None);
    // Force gen to u32::MAX so the next clear_with_bg wraps to 0.
    f.gen = u32::MAX;

    f.clear_with_bg(None);

    assert_eq!(f.gen, 1, "after wrap, gen must restart at 1 (not 0)");
    assert!(
        f.cell_gen.iter().all(|&g| g == 0),
        "after wrap, all cell_gen entries must be 0 (so no cell falsely matches gen=1)"
    );
    assert!(f.is_dirty_all());
}

/// INV-12: When `Frame::clear_dirty` causes `dirty_gen` to wrap past u32::MAX,
/// the overflow handler resets `dirty_cell_gen` to all-zeros and bumps
/// `dirty_gen` to 1. Same rationale as INV-11.
#[test]
fn lock_inv12_clear_dirty_gen_overflow_resets_dirty_cell_gen() {
    let mut f = Frame::new(4, 4, None);
    // Force dirty_gen to u32::MAX so the next clear_dirty wraps to 0.
    f.dirty_gen = u32::MAX;

    f.clear_dirty();

    assert_eq!(
        f.dirty_gen, 1,
        "after wrap, dirty_gen must restart at 1 (not 0)"
    );
    assert!(
        f.dirty_cell_gen.iter().all(|&g| g == 0),
        "after wrap, all dirty_cell_gen entries must be 0 (so no cell falsely appears dirty)"
    );
    assert!(!f.is_dirty_all());
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-13: Blank fallback when gen doesn't match
// ═══════════════════════════════════════════════════════════════════════════

/// INV-13: `cell_at_index` (and `cell_at_index_ref`) returns the blank cell
/// when the cell's `cell_gen` doesn't match the current `gen`. This is the
/// "logically cleared" property — after `clear_with_bg`, all cells appear
/// blank without being physically overwritten.
#[test]
fn lock_inv13_cell_at_index_returns_blank_when_gen_mismatches() {
    let mut f = Frame::new(4, 4, None);
    let live = Cell {
        ch: 'x',
        fg: None,
        bg: None,
        bold: false,
    };
    f.set(2, 2, live);

    // Live cell — should return the written cell.
    let idx = f.index(2, 2).unwrap();
    let read = f.cell_at_index(idx);
    assert_eq!(read.ch, 'x', "live cell must be returned when gen matches");

    // Bump gen via clear_with_bg — cell is now "logically cleared".
    f.clear_with_bg(None);

    // Reading the same index must return the blank cell, not the stale 'x'.
    let read = f.cell_at_index(idx);
    assert_eq!(
        read.ch, ' ',
        "after clear_with_bg, cell_at_index must return blank (gen mismatch)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-14 & INV-15: ColorCache contract
// ═══════════════════════════════════════════════════════════════════════════

/// INV-14: `ColorCache` produces non-empty SGR byte slices for every palette
/// color × background combination. The cache is pre-built at palette
/// construction; the hot path is `extend_from_slice` — zero `format!()` calls.
#[test]
fn lock_inv14_color_cache_produces_nonempty_sgr_bytes() {
    let palette = Palette {
        colors: vec![
            Color::Rgb { r: 0, g: 255, b: 0 },
            Color::Rgb {
                r: 128,
                g: 128,
                b: 128,
            },
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
        ],
        bg: Some(Color::Rgb {
            r: 10,
            g: 10,
            b: 10,
        }),
    };
    let cache = ColorCache::new(&palette);

    for i in 0..palette.colors.len() {
        let sgr = cache.sgr(i);
        assert!(
            !sgr.is_empty(),
            "ColorCache::sgr({i}) must produce non-empty bytes for TrueColor + bg"
        );
        // SGR sequences start with ESC [.
        assert_eq!(sgr[0], 0x1b, "SGR bytes must start with ESC (0x1b)");
        assert_eq!(sgr[1], b'[', "SGR bytes must start with ESC [");
    }
}

/// INV-15: `ColorCache::sgr_for_cell` returns `None` for fg/bg combinations
/// not in the palette. This is the safety hatch — a cell whose fg color is
/// not one of the palette colors (e.g. an anomaly halo color, or a ghost
/// color) gets a graceful `None` (caller falls back to inline formatting)
/// instead of a panic.
#[test]
fn lock_inv15_color_cache_sgr_for_cell_returns_none_for_unknown_color() {
    let palette = Palette {
        colors: vec![Color::Rgb { r: 0, g: 255, b: 0 }],
        bg: Some(Color::Rgb {
            r: 10,
            g: 10,
            b: 10,
        }),
    };
    let cache = ColorCache::new(&palette);

    // Palette fg + palette bg → Some.
    assert!(
        cache
            .sgr_for_cell(Some(palette.colors[0]), palette.bg)
            .is_some(),
        "palette fg + palette bg must return Some"
    );

    // Non-palette fg → None.
    assert!(
        cache
            .sgr_for_cell(
                Some(Color::Rgb {
                    r: 99,
                    g: 99,
                    b: 99,
                }),
                palette.bg,
            )
            .is_none(),
        "non-palette fg must return None (no panic, caller falls back to inline formatting)"
    );

    // Palette fg + non-palette bg → None.
    assert!(
        cache
            .sgr_for_cell(
                Some(palette.colors[0]),
                Some(Color::Rgb {
                    r: 99,
                    g: 99,
                    b: 99,
                }),
            )
            .is_none(),
        "palette fg + non-palette bg must return None"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-17: Idle-frame fast path — clear_dirty on empty frame preserves stamps
// ═══════════════════════════════════════════════════════════════════════════

/// INV-17: the idle-frame fast path in `Terminal::draw()` calls `clear_dirty()`
/// even when the dirty list is empty. This advances `dirty_gen`, which is
/// load-bearing for the next frame's `set()` calls — without it, cells that
/// were dirty in the last non-idle frame would still have
/// `dirty_cell_gen[i] == dirty_gen`, and the next frame's `set()` would
/// skip the dirty push (the duplicate-dirty-skip at frame.rs:296).
///
/// The fast path itself lives in `Terminal::draw()` and isn't directly
/// testable without a stdout handle. This test verifies the underlying
/// contract the fast path relies on: an idle `clear_dirty()` between two
/// active frames preserves the dirty-tracking correctness.
///
/// Scenario:
/// 1. Frame N: set cell (1,1) to 'x' → dirty list has 1 entry.
/// 2. End of frame N: clear_dirty() → dirty list empty, dirty_gen bumped.
/// 3. Frame N+1 (idle): no set() calls → dirty list stays empty.
/// 4. End of frame N+1: clear_dirty() → dirty_gen bumped again (this is
///    the call the idle-frame fast path makes).
/// 5. Frame N+2: set cell (1,1) to 'y' → MUST push to dirty list.
///
/// If step 4's clear_dirty were skipped (the bug the fast path must avoid),
/// step 5's set() would see `dirty_cell_gen[1*10+1] == dirty_gen` (still
/// matching from frame N) and skip the push — losing the cell change.
#[test]
fn lock_inv17_idle_frame_clear_dirty_preserves_dirty_tracking() {
    let mut f = Frame::new(10, 10, None);
    f.clear_dirty(); // Initial state — frame N setup.

    // Step 1: Frame N — dirty cell (1,1).
    let cell_x = Cell {
        ch: 'x',
        fg: None,
        bg: None,
        bold: false,
    };
    f.set(1, 1, cell_x);
    assert_eq!(f.dirty_indices().len(), 1, "frame N must have 1 dirty cell");

    // Step 2: End of frame N — clear_dirty.
    f.clear_dirty();
    assert!(f.dirty_indices().is_empty(), "dirty list empty after clear");

    // Step 3: Frame N+1 (idle) — no set() calls.
    // (Simulates the idle-frame fast path's pre-condition: dirty_count == 0.)
    assert!(f.dirty_indices().is_empty());

    // Step 4: End of frame N+1 — idle clear_dirty (the call the fast path makes).
    // This is the load-bearing call: without it, dirty_gen doesn't advance.
    f.clear_dirty();
    assert!(f.dirty_indices().is_empty());

    // Step 5: Frame N+2 — set the SAME cell to a NEW value.
    let cell_y = Cell {
        ch: 'y',
        fg: None,
        bg: None,
        bold: false,
    };
    f.set(1, 1, cell_y);
    assert_eq!(
        f.dirty_indices().len(),
        1,
        "frame N+2 must push cell (1,1) to dirty — idle clear_dirty advanced dirty_gen, \
         so the duplicate-dirty-skip must NOT fire"
    );
    assert_eq!(
        f.dirty_indices()[0],
        11,
        "dirty cell must be (1,1) → index 11"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// INV-16: Lock report sentinel
// ═══════════════════════════════════════════════════════════════════════════

/// INV-16: sentinel test that prints the engine lock report when run with
/// `--nocapture`. Useful for `git log` archaeology — running this test
/// after a checkout shows what the engine was locked at that commit.
#[test]
fn lock_inv16_engine_lock_report() {
    eprintln!("\n═══════════════════════════════════════════════════════════════");
    eprintln!("  Cosmic Dragon Engine Lock Report");
    eprintln!("═══════════════════════════════════════════════════════════════");
    eprintln!("  Version:            {COSMIC_DRAGON_ENGINE_VERSION}");
    eprintln!("  Invariants:         17 (INV-01 through INV-17)");
    eprintln!("  Frame clamp:        [{MIN_TERMINAL_COLS},{MAX_TERMINAL_COLS}] × [{MIN_TERMINAL_LINES},{MAX_TERMINAL_LINES}]");
    eprintln!("  Bench clamp:        [{MIN_TERMINAL_COLS},{BENCH_MAX_COLS}] × [{MIN_TERMINAL_LINES},{BENCH_MAX_LINES}]");
    eprintln!("  Dirty tracking:     double-buffered generation (O(1) clear_dirty)");
    eprintln!("  Idle-frame fast path: skip render body when dirty_count==0 & can_reuse_last");
    eprintln!("  Semantic invalidation: semantic_gen counter (stale-glyph guard)");
    eprintln!("  Color cache:        pre-formatted SGR bytes (no format!() in hot path)");
    eprintln!("═══════════════════════════════════════════════════════════════\n");
}
