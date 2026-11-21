// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The loading lane's tests (night-ux-orion-1): the helix table, the
//! style ladder, the Unicode probe, the lazy slot clock, and the
//! machine's byte-level draw contract - carriage return, grey line,
//! stable-width padding, and the full clear - pinned through the
//! test sink so the battery stays silent and the pins hold on any
//! terminal. The expected bytes ride the same
//! [`crate::output::grey_err`] wrapper the machine paints through,
//! so the pins are environment-independent (the palette itself has
//! its own pins in the output tests).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use super::{
    ASCII, HELIX, SWEEP, Spinner, Style, WAVE, frame_slot, frames_for, render_slot, spinner_line,
    unicode_ok,
};

// ── The loading lane (night-ux-orion-1) ─────────────────────────────

/// The helix table is the pin, not a computation: ten frames, every
/// frame pairing the sweep's i-th cell with its (9-i)-th - two
/// strands running the same walk in opposite directions - and the
/// strands never collide at any frame. Every frame in every table
/// shares one width, the stable-overwrite law.
#[test]
fn the_helix_table_is_ten_counter_phase_strands() {
    assert_eq!(HELIX.len(), 10);
    assert_eq!(WAVE.len(), 10);
    for i in 0..10 {
        let expected = format!("{}{}", SWEEP[i], SWEEP[9 - i]);
        assert_eq!(HELIX[i], expected, "frame {i}: forward x backward strand");
        let cells: Vec<char> = HELIX[i].chars().collect();
        assert_eq!(cells.len(), 2, "two strands, one cell each");
        assert_ne!(cells[0], cells[1], "frame {i}: the strands never collide");
    }
    assert!(HELIX.iter().all(|frame| frame.len() == HELIX[0].len()));
    assert!(WAVE.iter().all(|frame| frame.len() == WAVE[0].len()));
    assert!(ASCII.iter().all(|frame| frame.len() == ASCII[0].len()));
}

/// The style ladder: NO_COLOR or a locale without UTF-8 lands on the
/// ASCII floor, a UTF-8 terminal gets the seat the site claims - and
/// NO_COLOR wins over everything, because the unpainted dash is
/// still a visible wait, and a hang looks like a hang with or
/// without color.
#[test]
fn the_style_ladder_lands_on_the_ascii_floor() {
    let utf8 = Some("en_US.UTF-8");
    for locale in [None, Some("C"), Some("POSIX"), utf8] {
        for chosen in [Style::Helix, Style::Wave] {
            assert_eq!(
                frames_for(true, locale, chosen),
                &ASCII[..],
                "NO_COLOR: the dash, always"
            );
        }
    }
    assert_eq!(frames_for(false, None, Style::Helix), &ASCII[..]);
    assert_eq!(frames_for(false, Some("C"), Style::Helix), &ASCII[..]);
    assert_eq!(frames_for(false, Some("C.UTF-8"), Style::Helix), &HELIX[..]);
    assert_eq!(frames_for(false, utf8, Style::Helix), &HELIX[..]);
    assert_eq!(frames_for(false, utf8, Style::Wave), &WAVE[..]);
}

/// The Unicode probe's spellings: the three usual UTF-8 forms pass,
/// the plain locales fail, and absence fails - an exotic spelling
/// degrades to the ASCII floor, never to garbage bytes.
#[test]
fn the_unicode_probe_knows_the_utf8_spellings() {
    assert!(unicode_ok(Some("en_US.UTF-8")));
    assert!(unicode_ok(Some("C.UTF-8")));
    assert!(unicode_ok(Some("id_ID.utf8")));
    assert!(!unicode_ok(Some("C")));
    assert!(!unicode_ok(Some("POSIX")));
    assert!(!unicode_ok(None));
}

/// The lazy slot clock: silence below the owner's 100 ms threshold,
/// then one slot per frame period, deterministic - a late redraw
/// repeats a frame instead of skipping the sequence.
#[test]
fn the_lazy_slot_clock_stays_silent_below_the_threshold() {
    assert_eq!(frame_slot(Duration::from_millis(0)), None);
    assert_eq!(frame_slot(Duration::from_millis(99)), None);
    assert_eq!(frame_slot(Duration::from_millis(100)), Some(0));
    assert_eq!(frame_slot(Duration::from_millis(150)), Some(0));
    assert_eq!(frame_slot(Duration::from_millis(199)), Some(0));
    assert_eq!(frame_slot(Duration::from_millis(200)), Some(1));
    assert_eq!(frame_slot(Duration::from_millis(1234)), Some(11));
}

/// The loading line rides the bracket grammar - the frame where a
/// countdown's seconds would ride - and the slot render wraps the
/// table, so slot 12 of a ten-frame helix is frame 2.
#[test]
fn the_loading_line_rides_the_bracket_grammar() {
    assert_eq!(
        spinner_line("checking for updates", "⠋⠏"),
        "[checking for updates... ⠋⠏]"
    );
    assert_eq!(
        spinner_line("checking for updates", "-"),
        "[checking for updates... -]"
    );
    assert_eq!(
        render_slot("checking for updates", &HELIX, 12),
        spinner_line("checking for updates", HELIX[2]),
        "the table wraps"
    );
}

/// The inert machine: the test lane runs piped, so arm() takes the
/// gate's off arm - no thread, no width, and a finish that writes
/// nothing. A piped `--check-update` pays one gate probe and
/// nothing else, byte for byte.
#[test]
fn the_inert_spinner_never_draws_and_never_spawns() {
    super::test_clear_drawn();
    let spinner = Spinner::arm("checking for updates", Style::Helix);
    assert!(spinner.handle.is_none());
    assert_eq!(spinner.width, 0);
    spinner.finish();
    assert!(super::test_drawn_bytes().is_empty());
}

/// The drawn machine's clear, byte-pinned: the drawn CELL width
/// blanked with spaces, the cursor parked at column zero - the
/// clear grammar. The width is the screen footprint (the bracket
/// prefix's bytes plus the frame's cell count), not the byte width
/// a braille frame inflates.
#[test]
fn the_drawn_machine_clears_the_cell_width() {
    super::test_clear_drawn();
    let width = spinner_line("checking for updates", "").len() + HELIX[0].chars().count();
    let spinner = Spinner {
        stop: Arc::new(AtomicBool::new(false)),
        drawn: Arc::new(AtomicBool::new(true)),
        handle: None,
        width,
    };
    spinner.finish();
    assert_eq!(
        super::test_drawn_bytes(),
        format!("\r{}\r", " ".repeat(width))
    );
}

/// The undrawn machine's silence: an operation that landed inside
/// the lazy threshold drew nothing, so its finish writes no bytes
/// at all - the fast path's stderr stays untouched, byte for byte,
/// the way a piped run's machine is silent.
#[test]
fn the_undrawn_machine_finishes_without_a_byte() {
    super::test_clear_drawn();
    let spinner = Spinner {
        stop: Arc::new(AtomicBool::new(false)),
        drawn: Arc::new(AtomicBool::new(false)),
        handle: None,
        width: 31,
    };
    spinner.finish();
    assert!(super::test_drawn_bytes().is_empty());
}

/// The width law at cell granularity: every frame in a table is the
/// same CELL width, so the clear width is one computation - and the
/// braille table's cell count is a third of its byte count, the
/// three-bytes-per-cell fact the clear no longer inflates.
#[test]
fn the_frame_tables_are_cell_width_stable() {
    let tables: [&[&str]; 3] = [&HELIX, &WAVE, &ASCII];
    for table in tables {
        let cells: Vec<usize> = table.iter().map(|frame| frame.chars().count()).collect();
        assert!(
            cells.iter().all(|c| *c == cells[0]),
            "the table's cells drift"
        );
    }
    assert_eq!(HELIX[0].chars().count(), 2, "two strands, two cells");
    assert_eq!(HELIX[0].len(), 6, "braille is three bytes per cell");
}
