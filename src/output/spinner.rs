// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The shared single-line loading indicator (night-dragon-maintenance-4).
//!
//! One concern, one module: any blocking step that would otherwise sit
//! silent — the `--check-update` network fetch today — shows the same
//! calm single-line spinner the benchmark lane renders, so the binary
//! never looks unresponsive during a wait window. Grammar:
//!
//! ```text
//! ⠋ Fetching latest release info... 2.3s
//! ```
//!
//! - shown only when stderr is an interactive terminal (piped or
//!   redirected output stays pure bytes — the same pollution law the
//!   benchmark spinner follows);
//! - the frame rides the bold ok green tier (alive), the label and the
//!   elapsed seconds ride the calm grey tier (output color law,
//!   night-dragon-maintenance-4);
//! - braille frames — the same premium set the benchmark lane renders,
//!   shared via [`SPINNER_FRAMES`] so the two lanes can never drift
//!   apart (the symbol-only guard targets emoji-class bytes, not
//!   braille);
//! - a background ticker thread repaints at 4 Hz (250 ms — calm, not
//!   spammy, the benchmark cadence); dropping the guard stops the
//!   thread, erases the line, and joins, so early-error returns clean
//!   up through ordinary RAII.

use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::{color_capability, grey_open, ok_bold_open, reset, ColorCapability};

/// Braille spinner frames — subtle, modern, premium. Single source for
/// every loading lane (benchmark + the blocking-fetch indicator).
pub(crate) const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Repaint cadence (4 Hz — the benchmark spinner's silent cadence).
const TICK: Duration = Duration::from_millis(250);

/// Compose one spinner line, capability-parameterized (unit-testable
/// without env sniffing — the same four-rung ladder as grey.rs).
/// Frame green (alive), label + seconds grey (context).
fn render_line_for(cap: ColorCapability, frame: char, label: &str, secs: f64) -> String {
    match cap {
        ColorCapability::Mono => format!("{frame} {label}... {secs:.1}s"),
        _ => format!(
            "{}{frame}{} {}{label}... {secs:.1}s{}",
            ok_bold_open(),
            reset(),
            grey_open(),
            reset()
        ),
    }
}

/// A scoped loading indicator for a blocking step.
///
/// Start it before the blocking call, let it fall out of scope after —
/// the terminal line it borrowed is erased either way, and errors
/// propagating through `?` clean up for free.
pub(crate) struct Spinner {
    stop: Option<Arc<AtomicBool>>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    /// Show the spinner for `label` until dropped. No-op on piped or
    /// redirected stderr (the caller's output stays pure bytes).
    pub(crate) fn start(label: &str) -> Self {
        if !io::stderr().is_terminal() {
            return Self {
                stop: None,
                handle: None,
            };
        }
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let label = label.to_string();
        let handle = std::thread::Builder::new()
            .name("loading-indicator".to_string())
            .spawn(move || {
                let start = Instant::now();
                let mut idx = 0usize;
                let cap = color_capability();
                let mut err = io::stderr();
                while !stop_flag.load(Ordering::Relaxed) {
                    let frame = SPINNER_FRAMES[idx % SPINNER_FRAMES.len()];
                    idx += 1;
                    let secs = start.elapsed().as_secs_f64();
                    let line = render_line_for(cap, frame, &label, secs);
                    // Broken pipe or any write hiccup must never panic the
                    // ticker — the indicator is cosmetic by contract.
                    let _ = write!(err, "\r\x1b[2K{line}");
                    let _ = err.flush();
                    std::thread::sleep(TICK);
                }
            })
            .ok();
        Self {
            stop: Some(stop),
            handle,
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if let Some(stop) = &self.stop {
            stop.store(true, Ordering::Relaxed);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        if io::stderr().is_terminal() {
            let mut err = io::stderr();
            let _ = write!(err, "\r\x1b[2K");
            let _ = err.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frame set is the house braille identity: ten calm frames,
    /// shared with the benchmark lane by construction (single const).
    #[test]
    fn frames_are_the_house_braille_set() {
        assert_eq!(SPINNER_FRAMES.len(), 10);
        assert!(SPINNER_FRAMES.iter().all(|c| !c.is_alphanumeric()));
    }

    /// The mono ladder renders plain text — no ANSI escapes anywhere.
    #[test]
    fn mono_line_is_plain() {
        let line = render_line_for(ColorCapability::Mono, '⠋', "Fetching", 2.3);
        assert_eq!(line, "⠋ Fetching... 2.3s");
        assert!(!line.contains('\x1b'));
    }

    /// The colored ladder carries both tiers: the green frame open and
    /// the grey label open, with the elapsed seconds included.
    #[test]
    fn colored_line_carries_both_tiers() {
        let line = render_line_for(ColorCapability::TrueColor, '⠋', "Fetching", 2.3);
        assert!(line.contains("Fetching... 2.3s"));
        assert!(line.contains(ok_bold_open()));
        assert!(line.contains(grey_open()));
    }
}
