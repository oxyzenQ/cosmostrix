// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The loading lane (night-ux-orion-1): the one voice a silent wait
//! is allowed.
//!
//! One concern, one module: the indeterminate operations - today the
//! one network fetch behind `--check-update` - where there is no
//! window to count down, only an answer that may take seconds. A
//! silent fetch is indistinguishable from a hang, and the Ctrl+C a
//! frozen-looking terminal invites is a bug of perception, not of
//! behavior. The lane arms a background redrawing thread whose first
//! frame lands only after the operation has actually crossed the
//! lazy threshold: a fast path never draws and its finish erases
//! nothing - the fast path's stderr stays byte for byte silent - and
//! a slow one shows a DNA helix - two braille strands running the
//! same sweep in opposite directions, the two anti-parallel streams
//! the metaphor wants (the request out, the answer back).
//!
//! ```text
//! [Fetching latest release info... ⠋⠏]
//! ```
//!
//! - shown only when stderr is an interactive terminal (piped or
//!   redirected output stays pure bytes — the same pollution law the
//!   benchmark spinner follows);
//! - the whole line rides the calm grey tier through
//!   [`crate::output::grey_err`] — subordinate progress, never a
//!   headline (the zelynic spinner law, night-cafe-2 parity);
//! - the fallbacks are detection, never flags: a terminal without
//!   color or without a Unicode locale drops to the rotating ASCII
//!   dash automatically, and the paint rides the same capability
//!   ladder — bright black on a Color16 palette, the theme's own
//!   grey, visible by construction on dark and light backgrounds
//!   alike;
//! - the frame rides the same braille set the benchmark lane renders,
//!   shared via [`SWEEP`] so the two lanes can never drift apart.
//!
//! The overwrite and the clear ride carriage returns and spaces only
//! (no CSI erase), so they degrade honestly on any terminal that
//! honors \r; the width law is cell granularity (every frame in a
//! table is the same cell width, so the overwrite is always stable).
//! Labels are the crate's own ASCII constants; nothing external ever
//! reaches this lane.
//!
//! One noise law: a failed narration write is swallowed, never a
//! panic (write_raw) - the indicator is decoration, and a stderr that
//! cannot take the line must not cost the run its report. The test
//! lane swaps the sink for a thread-local buffer, so the byte
//! contract is pinned exactly and the battery stays silent; the
//! Spinner's pure half (frame tables, style selection, the lazy slot
//! clock) is pinned directly, and its thread half stays thin glue on
//! purpose - the bytes it writes are the pinned pure bytes.

use std::io::IsTerminal;
#[cfg(not(test))]
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The lazy threshold: an operation that lands inside 100 ms never
/// draws - the fast paths keep their byte-for-byte silence, and the
/// indicator appears only when the wait is real. The owner's
/// auto-detect law, one constant.
const LAZY: Duration = Duration::from_millis(100);

/// The frame period: ten frames per second, the braille sweep's
/// smooth cadence - fast enough to read as motion, slow enough that
/// a slow fetch's redraw cost is noise.
const FRAME_PERIOD: Duration = Duration::from_millis(100);

/// The stop-flag poll slice: the redraw thread never sleeps longer
/// than one slice, so a `finish` that lands mid-sleep joins within
/// a slice instead of a whole sleep leg. The fast path keeps its
/// speed (the join adds at most one slice, never the remaining
/// pre-threshold fraction or a full frame period), and the clear
/// never lags the verdict.
const POLL: Duration = Duration::from_millis(25);

/// The braille sweep both helix strands ride: the classic ten-frame
/// dot walk, one cell wide, clockwise. Single source for every
/// loading lane (benchmark + this one), so the tables never drift.
pub(crate) const SWEEP: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// The DNA helix (night-ux-orion-1): two strands, counter-phase.
/// `HELIX[i]` pairs `SWEEP[i]` with `SWEEP[9 - i]` - strand one runs
/// the sweep forward while strand two runs it backward, the two
/// anti-parallel streams the metaphor wants - and the strands never
/// collide: the pair is distinct at every frame because `i = 9 - i`
/// has no integer answer. Written out as literals so the table is
/// the pin, not a computation.
const HELIX: [&str; 10] = ["⠋⠏", "⠙⠇", "⠹⠧", "⠸⠦", "⠼⠴", "⠴⠼", "⠦⠸", "⠧⠹", "⠇⠙", "⠏⠋"];

/// The wave (one strand, the linear shape): the bare sweep. Kept
/// for the estate's next linear site - the loading lane's one
/// current site (the update fetch) arms the helix, whose two
/// anti-parallel strands ARE the fetch: the request out, the answer
/// back.
const WAVE: [&str; 10] = SWEEP;

/// The ASCII floor: the rotating dash. NO_COLOR terminals and
/// terminals without a Unicode locale land here automatically -
/// the fallback is detection, never a flag, which is what separates
/// a CLI that carries its aesthetics from one that forces them.
const ASCII: [&str; 4] = ["-", "\\", "|", "/"];

/// The style seat a loading site claims: the helix for the
/// parallel-looking operations, the wave for the linear ones.
#[derive(Clone, Copy)]
pub(crate) enum Style {
    /// Two counter-phase braille strands - the DNA helix.
    Helix,
    /// One braille strand - the linear wave.
    #[allow(dead_code)]
    Wave,
}

impl Style {
    fn frames(self) -> &'static [&'static str] {
        match self {
            Style::Helix => &HELIX,
            Style::Wave => &WAVE,
        }
    }
}

/// The Unicode probe, pure: a locale that names UTF-8 in any of its
/// usual spellings. Best-effort by design - an exotic locale
/// spelling degrades to the ASCII floor, never to garbage bytes.
fn unicode_ok(locale: Option<&str>) -> bool {
    locale.is_some_and(|l| {
        let lower = l.to_ascii_lowercase();
        lower.contains("utf-8") || lower.contains("utf8")
    })
}

/// The locale probe: LC_ALL, then LC_CTYPE, then LANG - the
/// standard precedence, first set value wins.
fn locale() -> Option<String> {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .find_map(|key| std::env::var(key).ok())
}

/// The style ladder, pure: NO_COLOR or a non-Unicode locale lands on
/// the ASCII floor; otherwise the site's own seat. The loading
/// lane's gate deliberately keeps the indicator alive on a NO_COLOR
/// terminal (as the unpainted dash) because a hang looks like a hang
/// with or without color.
fn frames_for(no_color: bool, locale: Option<&str>, chosen: Style) -> &'static [&'static str] {
    if no_color || !unicode_ok(locale) {
        &ASCII
    } else {
        chosen.frames()
    }
}

/// The loading line's shape: the bracket grammar, the frame riding
/// where a countdown's seconds would ride - one grammar, every
/// surface, no drift.
fn spinner_line(label: &str, frame: &str) -> String {
    format!("[{label}... {frame}]")
}

/// The rendered line for a slot: the table wraps, the line rides
/// the bracket grammar. Pure - the thread's draw is exactly this
/// string behind a carriage return, so the pins hold the bytes the
/// terminal sees.
fn render_slot(label: &str, frames: &[&str], slot: usize) -> String {
    spinner_line(label, frames[slot % frames.len()])
}

/// The lazy slot clock, pure: an operation younger than the lazy
/// threshold has no frame yet (None - nothing has ever been drawn),
/// and an older one names the frame slot the elapsed time pins.
/// Deterministic: the slot is a pure function of elapsed time, so a
/// redraw that lands late repeats a frame instead of skipping the
/// sequence.
fn frame_slot(elapsed: Duration) -> Option<usize> {
    if elapsed < LAZY {
        None
    } else {
        Some(((elapsed - LAZY).as_millis() / FRAME_PERIOD.as_millis()) as usize)
    }
}

/// One narration write, errors swallowed: `eprint!` panics when the
/// write fails, and the indicator must never be the reason a run
/// dies. stderr is unbuffered - one call, one landing.
#[cfg(not(test))]
fn write_raw(bytes: &str) {
    let _ = std::io::stderr().write_all(bytes.as_bytes());
}

/// The test lane's sink: the machine's exact bytes, per test thread.
/// The production path above is the only other writer - the tests
/// pin the draw contract without touching the battery's stderr.
#[cfg(test)]
fn write_raw(bytes: &str) {
    DRAWN.with(|buffer| buffer.borrow_mut().extend_from_slice(bytes.as_bytes()));
}

#[cfg(test)]
thread_local! {
    /// The bytes this test thread's machine drew since the last
    /// clear.
    static DRAWN: std::cell::RefCell<Vec<u8>> = const {
        std::cell::RefCell::new(Vec::new())
    };
}

/// The drawn bytes of this test thread, for the shape pins. The
/// machine's grammar is ASCII, so the bytes are always valid UTF-8.
#[cfg(test)]
fn test_drawn_bytes() -> String {
    DRAWN.with(|buffer| String::from_utf8_lossy(&buffer.borrow()).into_owned())
}

/// Clear the test sink between draws, per test.
#[cfg(test)]
fn test_clear_drawn() {
    DRAWN.with(|buffer| buffer.borrow_mut().clear());
}

/// The loading lane's machine: a background thread redraws one
/// stderr line while the operation blocks, the caller's `finish`
/// stops it and erases the line before any verdict prints. The
/// thread never draws before the lazy threshold, the frames are the
/// pure tables above, and every write failure is swallowed by the
/// shared noise law. A non-terminal stderr arms the inert machine -
/// a piped run pays one gate probe and nothing else, byte for byte.
pub(crate) struct Spinner {
    /// The thread's stop flag: set by `finish`, checked every cycle.
    stop: Arc<AtomicBool>,
    /// The thread's drawn flag: set before the first frame's write.
    /// The clear rides this truth - an operation that landed inside
    /// the lazy threshold drew nothing, so its finish writes no
    /// bytes at all (a drawn flag, mirrored from a width that only
    /// grows on a draw).
    drawn: Arc<AtomicBool>,
    /// The redraw thread, when the gate allowed one.
    handle: Option<JoinHandle<()>>,
    /// The drawn line's CELL width: the bracket prefix's bytes plus
    /// the frame's cell count (the tables are width-stable, so the
    /// first frame names it; a braille strand is three bytes per
    /// cell, and the clear blanks cells, not bytes).
    width: usize,
}

impl Spinner {
    /// Arm the loading lane for an indeterminate operation. The
    /// frames come from the style ladder (the helix seat unless the
    /// terminal drops to the ASCII floor); the first draw happens
    /// only when the operation crosses the lazy threshold, inside
    /// the thread.
    pub(crate) fn arm(label: &'static str, style: Style) -> Spinner {
        // The piped-run law: a captured stderr never sees narration,
        // no matter what the color environment promises.
        if !std::io::stderr().is_terminal() {
            return Spinner {
                stop: Arc::new(AtomicBool::new(false)),
                drawn: Arc::new(AtomicBool::new(false)),
                handle: None,
                width: 0,
            };
        }
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        let frames = frames_for(no_color, locale().as_deref(), style);
        // The width law: the tables are width-stable, so the first
        // frame names the line's CELL width.
        let width = spinner_line(label, "").len() + frames[0].chars().count();
        let stop = Arc::new(AtomicBool::new(false));
        let drawn = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread_drawn = Arc::clone(&drawn);
        let handle = std::thread::spawn(move || {
            let armed = Instant::now();
            let mut drawn_slot: Option<usize> = None;
            loop {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
                let elapsed = armed.elapsed();
                if let Some(slot) = frame_slot(elapsed) {
                    if drawn_slot != Some(slot) {
                        thread_drawn.store(true, Ordering::Relaxed);
                        write_raw(&format!(
                            "\r{}",
                            crate::output::grey_err(&render_slot(label, frames, slot))
                        ));
                        drawn_slot = Some(slot);
                    }
                    // Sleep toward the next slot boundary, never past
                    // the stop flag's next check.
                    let in_slot = (elapsed - LAZY).as_millis() % FRAME_PERIOD.as_millis();
                    let rest = FRAME_PERIOD.as_millis() - in_slot;
                    std::thread::sleep(Duration::from_millis(rest.max(1) as u64).min(POLL));
                } else {
                    // Below the lazy threshold: sleep toward it in
                    // poll slices, so the stop flag's check keeps its
                    // cadence and a fast operation's join never waits
                    // out the whole remaining fraction.
                    std::thread::sleep((LAZY - elapsed).min(POLL));
                }
            }
        });
        Spinner {
            stop,
            drawn,
            handle: Some(handle),
            width,
        }
    }

    /// Stop the redraw thread and erase the line entirely: the
    /// drawn CELL width blanked with spaces, the cursor back at
    /// column zero, before any verdict prints. The join guarantees
    /// the thread's last write lands before the clear does. The
    /// clear rides the drawn truth: an operation that landed inside
    /// the lazy threshold never drew, so its finish is byte-silent -
    /// the fast path's stderr is untouched, byte for byte.
    pub(crate) fn finish(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        if self.drawn.load(Ordering::Relaxed) && self.width > 0 {
            write_raw(&format!("\r{}\r", " ".repeat(self.width)));
        }
    }
}

#[cfg(test)]
#[path = "../../test/output/spinner_tests.rs"]
mod tests;
