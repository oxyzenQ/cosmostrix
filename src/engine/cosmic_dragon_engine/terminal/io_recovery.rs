// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal ANSI flush + write recovery — extracted from `terminal/mod.rs`
//! to keep that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns the I/O recovery methods on `Terminal`:
//! - `flush_ansi()`: write the pending ANSI buffer to stdout, optionally
//!   wrapped in BSU/ESU sync markers for tear-free rendering.
//! - `emit_ris_reset()`: emit RIS (reset to initial state) escape sequence.
//! - `write_with_recovery()`: write with P3 broken-pipe recovery.
//! - `recover_to_tty()`: recover a broken stdout by routing through
//!   `/dev/tty` (Unix) or propagate the error (non-Unix).

// Bring the tty fallback helpers into scope so recover_to_tty can call
// them by bare name (matching the original in mod.rs). The crate-level
// re-exports (crate::terminal::is_terminal_gone etc) stay in mod.rs.
#[cfg(unix)]
use crate::terminal_tty::{is_recoverable_io_error, open_tty_fallback};

// Imports shared with terminal/mod.rs (kept in sync). These are the same
// paths the original methods used when they lived inline in mod.rs.
use std::io::Result;
use std::io::Write as _;

use crate::tier2::{should_backpressure, should_ris_reset};

// Re-exported from terminal/restore.rs so recover_to_tty can call it.
use super::restore::restore_terminal_best_effort;

impl super::Terminal {
    /// Flush the ANSI buffer to stdout via a single write_all call.
    /// When synchronized output is supported, wraps the frame in
    /// `ESC[?2026h` / `ESC[?2026l` markers for tear-free rendering.
    ///
    /// Also accumulates `ansi_buf.len()` into `total_ansi_bytes` and
    /// increments `flush_count` for `--perf-stats` reporting. The sync
    /// wrapper bytes (12 bytes total when sync_output is enabled) are
    /// NOT counted — only the actual frame content.
    ///
    /// P3 optimization: when sync_output is enabled, combine the 3
    /// write_all calls (SYNC_START + ansi_buf + SYNC_END) into a single
    /// write_all via a combined buffer. This reduces syscalls from 3 to 1
    /// per frame. At 60 FPS, saves 120 syscalls/second (3→1 × 60).
    /// Each syscall is ~1µs, so saves ~120µs/second of syscall overhead.
    ///
    /// The combined buffer is reused across frames (self.combined_flush_buf)
    /// to avoid per-frame allocation. It grows as needed but never shrinks.
    #[inline]
    pub(super) fn flush_ansi(&mut self) -> Result<()> {
        if self.ansi_buf.is_empty() {
            return Ok(());
        }
        // Accumulate encoding stats BEFORE clearing the buffer.
        // Only count frame content, not sync wrappers.
        let frame_bytes = self.ansi_buf.len() as u64;
        self.total_ansi_bytes += frame_bytes;
        self.flush_count += 1;

        // Tier 2 -- RIS reset (xterm.js hosts only).
        //
        // Check BEFORE backpressure: a RIS emission is itself ~20 bytes
        // (RIS + re-enter-alternate-screen + cursor hide + SGR mouse
        // mode), negligible vs the threshold, so even if we are about
        // to suppress this flush for backpressure, the RIS still fires
        // and resets xterm.js's buffer.
        if self.term_caps.xtermjs_host {
            let cumulative = self.bytes_since_ris + frame_bytes;
            if should_ris_reset(cumulative, self.bytes_since_ris) {
                self.emit_ris_reset()?;
                // bytes_since_ris was reset inside emit_ris_reset; the
                // upcoming flush's frame_bytes will be added below.
            }
        }

        // Tier 2 -- byte-budget backpressure (xterm.js hosts only).
        // If the rolling window exceeds the budget, suppress this flush.
        // Rain state still advances (event loop calls cloud.rain_at()
        // BEFORE term.draw()), so the user sees a brief stutter rather
        // than a permanent desync. The 0 byte count is pushed into the
        // window so the budget recovers as old frames age out.
        if self.term_caps.xtermjs_host {
            let window_sum = self.byte_window.sum();
            if should_backpressure(window_sum, self.bytes_since_ris) {
                self.backpressure_skips += 1;
                self.byte_window.push(0);
                // signal backpressure so the event loop injects
                // a synthetic write_overshoot (otherwise suppression
                // masks itself: no write → stale latency → no
                // perf_pressure → self-healer never fires).
                self.last_flush_suppressed = true;
                self.ansi_buf.clear();
                return Ok(());
            }
        }

        // Extract ansi_buf so write_with_recovery can borrow `*self`
        // mutably for the recovery path. The Vec's allocation is preserved
        // across take + restore -- zero per-frame alloc cost.
        let mut ansi_buf = std::mem::take(&mut self.ansi_buf);

        let write_result = if self.term_caps.sync_output {
            // P3: combine SYNC_START + ansi_buf + SYNC_END into one write.
            // Reuse the combined buffer to avoid per-frame allocation.
            let mut combined = std::mem::take(&mut self.combined_flush_buf);
            combined.clear();
            combined.extend_from_slice(crate::termdetect::SYNC_START);
            combined.extend_from_slice(&ansi_buf);
            combined.extend_from_slice(crate::termdetect::SYNC_END);
            let r = self.write_with_recovery(&combined);
            // Restore the combined buffer for reuse next frame.
            self.combined_flush_buf = combined;
            r
        } else {
            self.write_with_recovery(&ansi_buf)
        };

        match write_result {
            Ok(()) => {
                // Success: clear ansi_buf (preserves allocation for reuse).
                ansi_buf.clear();
                self.ansi_buf = ansi_buf;
                // Tier 2: record this frame's byte contribution.
                if self.term_caps.xtermjs_host {
                    self.byte_window.push(frame_bytes);
                    self.bytes_since_ris += frame_bytes;
                }
                // clear backpressure flag (this flush went through).
                self.last_flush_suppressed = false;
                Ok(())
            }
            Err(e) => {
                // Failure: restore ansi_buf so the next flush retries the
                // same data (matches pre-P3 semantics).
                self.ansi_buf = ansi_buf;
                Err(e)
            }
        }
    }

    /// Tier 2: emit an RIS (Reset to Initial State) sequence -- ESC c.
    ///
    /// This forces xterm.js to clear its in-memory scrollback buffer,
    /// preventing the unbounded growth that leads to V8 OOM (SIGTRAP)
    /// over multi-hour runs. After the RIS, we re-enter the alternate
    /// screen and re-hide the cursor -- RIS in some terminals exits the
    /// alternate screen and resets cursor visibility, which would leave
    /// the user with a scrambled display.
    ///
    /// Cost: ~20 bytes of ANSI (RIS + alternate screen + cursor hide +
    /// SGR mouse mode). Fires at most every ~7 seconds under sustained
    /// max load (50 MB threshold / 7 MB/sec rate), so the per-frame
    /// amortized cost is ~3 bytes -- negligible.
    ///
    /// After emission, `bytes_since_ris` and `byte_window` are reset
    /// since the buffer they were tracking has been nuked.
    fn emit_ris_reset(&mut self) -> Result<()> {
        // Build the reset sequence: RIS + re-enter alternate screen +
        // re-hide cursor. RIS alone is sufficient for xterm.js (its
        // implementation preserves alternate screen state), but the
        // extra bytes are cheap insurance against stricter terminals
        // (e.g., a future xterm.js host that fully resets on RIS).
        //
        // Sequence breakdown:
        //   ESC c            -- RIS (Reset to Initial State)
        //   ESC [ ? 1049 h   -- Enter alternate screen (DECSET 1049)
        //   ESC [ ? 25 l     -- Hide cursor (DECTCEM off)
        //   ESC [ ? 1006 h   -- Re-enable SGR mouse mode (in case RIS
        //                      reset it -- xterm.js preserves this, but
        //                      other hosts might not)
        const RIS_RECOVERY: &[u8] = b"\x1bc\x1b[?1049h\x1b[?25l\x1b[?1006h";

        self.write_with_recovery(RIS_RECOVERY)?;
        self.ris_resets += 1;
        self.bytes_since_ris = 0;
        self.byte_window.reset();
        Ok(())
    }

    /// P3: write a buffer to stdout, attempting a /dev/tty fallback when
    /// the primary fd is broken mid-run (SSH disconnect, terminal crash,
    /// parent death). On a recoverable error:
    ///
    ///   1. Lazily open `/dev/tty` (Unix) or `CONOUT$` (Windows).
    ///   2. Write the buffer to the fallback handle.
    ///   3. Set `GRACEFUL_SHUTDOWN` so the main loop exits cleanly via
    ///      the normal shutdown path (Terminal::drop still runs).
    ///   4. Bump `tty_recoveries`. If it exceeds
    ///      `STDOUT_FALLBACK_MAX_RECOVERIES`, propagate the original
    ///      error — /dev/tty itself is likely broken too.
    ///
    /// Zero per-frame overhead in the steady state: the happy path is a
    /// single `write_all` on the BufWriter. Fallback only fires on error.
    #[inline]
    fn write_with_recovery(&mut self, buf: &[u8]) -> Result<()> {
        // Time the write so the event loop can detect slow downstream
        // terminals. Instant::now() is ~20ns — negligible vs the write.
        let start = std::time::Instant::now();
        let result = self.stdout.write_all(buf);
        self.last_write_ns = start.elapsed().as_nanos() as u64;
        match result {
            Ok(()) => Ok(()),
            Err(e) => self.recover_to_tty(buf, e),
        }
    }

    /// P3 helper: attempt to recover a failed stdout write by routing the
    /// buffer through /dev/tty. See `write_with_recovery` for the full
    /// contract. Returns the original error if recovery is not possible.
    #[cfg(unix)]
    pub(super) fn recover_to_tty(
        &mut self,
        buf: &[u8],
        original_err: std::io::Error,
    ) -> Result<()> {
        use crate::constants::STDOUT_FALLBACK_MAX_RECOVERIES;
        // Non-recoverable errors (e.g., WriteZero, Interrupted) should
        // propagate unchanged. We only attempt /dev/tty when the primary
        // fd is observably broken.
        if !is_recoverable_io_error(&original_err) {
            return Err(original_err);
        }
        // Defensive cap: stop trying after N consecutive recoveries.
        if self.tty_recoveries >= STDOUT_FALLBACK_MAX_RECOVERIES {
            return Err(original_err);
        }
        // Lazily open /dev/tty on first recovery; cache for reuse.
        if self.tty_fallback.is_none() {
            self.tty_fallback = open_tty_fallback();
        }
        let Some(tty) = self.tty_fallback.as_mut() else {
            // No controlling terminal (e.g., `setsid` sandbox). Propagate
            // the original stdout error so the event loop exits normally.
            return Err(original_err);
        };
        // Best-effort write to /dev/tty. If this fails too, propagate the
        // original stdout error (more diagnostic than the tty error).
        if tty.write_all(buf).is_err() {
            return Err(original_err);
        }
        let _ = tty.flush();
        self.tty_recoveries += 1;
        // Signal the main loop to exit cleanly via the normal shutdown
        // path. This avoids racing on the broken stdout fd during cleanup.
        crate::interactive::request_graceful_shutdown();
        // AB-10 (rain-screen cleanliness): leave the alt screen BEFORE the
        // stderr write below. Without this, the notice leaks into the
        // rain matrix because the alt screen is still active (raw mode +
        // EnterAlternateScreen were sent at Terminal construction, and
        // stdout being broken doesn't unwind them). restore_terminal_best_effort
        // is idempotent — calling it here AND again later in the panic hook
        // or Terminal::drop is a no-op for the second call.
        restore_terminal_best_effort();
        // Broken-pipe-safe stderr notice — eprintln! would panic if stderr
        // is also broken (e.g., terminal fully gone), so use write_fmt.
        use std::io::Write as _;
        let _ = std::io::stderr().write_fmt(format_args!(
            "[terminal] stdout write failed ({}) — recovered via /dev/tty, exiting gracefully\n",
            original_err
        ));
        let _ = std::io::stderr().flush();
        Ok(())
    }

    /// P3 helper (Windows): /dev/tty fallback is Unix-only. On Windows,
    /// stdout corruption is rarer (the console API is more robust) and
    /// `CONOUT$` reopening requires unsafe Win32 calls. Propagate the
    /// original error for now; the watchdog still catches stuck loops.
    #[cfg(not(unix))]
    pub(super) fn recover_to_tty(
        &mut self,
        _buf: &[u8],
        original_err: std::io::Error,
    ) -> Result<()> {
        Err(original_err)
    }
}
