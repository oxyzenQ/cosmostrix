<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# Sharpest-skill depth audit: the diff render path and the output law (NIGHT-zen-lts-1)

Date: 2026-11-19
Scope: the one capability cosmostrix cannot lose without losing its
identity — the diff-based render path and the output law behind it:
per-frame byte emission (`src/engine/cosmic_dragon_engine/terminal/`:
`draw.rs`, `io_recovery.rs`, `terminal_tty.rs`, `last_frame.rs`,
`mod.rs`), the shadow state machine that decides "which cells changed",
and the recovery ladder that keeps a broken stdout from corrupting the
diff. Audit baseline: 15656078 (HEAD of main, post night-frontier-Z4).
Method: full invariant read of the five emission modules, every
`force_full_emit` arming site traced to its consumer, the io error
classification compared against the syscall-floor ledger, plus the
empirical half — the six-leg hostile torture battery
(`scripts/supermassive/torture.py`) executed locally against the debug
build at this exact commit.

## Verdict

**The sharpest skill is at peak at today's HEAD — zero defects, zero
code changes.** Per the owner's audit-if-peak-skip rule this is a
record-only audit: the evidence is written down so the next session
does not re-audit the same surface.

## 1 — The resync invariant is closed at every desync source

A diff renderer has exactly one failure class: emitting "unchanged"
bytes for cells whose real terminal state differs from the shadow.
Every known desync source must arm `force_full_emit`; the audit traced
all of them:

| Desync source | Arming site | Consumed / cleared |
|---|---|---|
| Terminal construction (no shadow yet) | `last_frame.rs:58` (initial `true`) | `draw.rs:121` |
| Resize / new dimensions | `last_frame.rs:101` (both resize paths, NIGHT-hunter-34) | `draw.rs:175` |
| Shadow reset after corruption (HUNT-27) | `io_recovery.rs:117`, `io_recovery.rs:175` | `draw.rs:175` |
| `/dev/tty` fallback recovery (P3) | `io_recovery.rs:231` | `draw.rs:175` |
| After the forced row is emitted | cleared | `draw.rs:326` |

No code path writes the shadow without either going through the diff
pass or arming the full emit; no arming site lacks a consumer. The
invariant is a closed graph.

## 2 — The io error ladder matches the kernel's actual behavior

`terminal_tty.rs:34` classifies stdout write failures. The audit
re-derived the law from the syscall side:

- **Interrupted (EINTR)** is deliberately NOT routed to the `/dev/tty`
  fallback — it is transient, and `std::io::Write::write_all` already
  retries it internally on the same fd. The Z2a syscall-floor parser
  (night-frontier-Z4 gate fix) documents exactly what the kernel emits
  for this pair in a strace log, and the PTY lock suite asserts the
  steady state stays at one `write()` per frame.
- **WriteZero** is excluded: it means the buffer itself is wrong, so
  retrying through `/dev/tty` would mask a real bug.
- The recoverable set is `BrokenPipe`, `PermissionDenied`, `Other`,
  plus raw `EBADF (9)`, `ENXIO (6)`, `EIO (5)` — the three codes a
  dead terminal emulator, a revoked controlling terminal or a pty
  hangup actually produce.
- The fallback is capped (`STDOUT_FALLBACK_MAX_RECOVERIES`), flips to
  graceful shutdown instead of unwinding, restores the terminal BEFORE
  the stderr notice (AB-10 rain-screen cleanliness), and uses
  `write_fmt` because `eprintln!` panics on a broken stderr.

The conservative direction of the classification is correct: false
negatives propagate to the watchdog, false positives would hide real
breakage behind a lucky tty write.

## 3 — Empirical half: the hostile battery at this commit

`python3 scripts/supermassive/torture.py --binary
target/debug/cosmostrix` (debug profile: overflow checks on, debug
assertions on — the strictest arithmetic lens):

| Leg | Result |
|---|---|
| resize-storm | PASS |
| input-flood | PASS |
| signal-storm (SIGINT death, SIGTERM graceful) | PASS (rc -2 / rc 0, instant death, restore tails clean) |
| config-churn (22 rapid edits) | PASS (rc 0) |
| restart-storm (12 sessions) | PASS (12/12 clean with restore tails) |
| suspend-resume (SIGTSTP + SIGCONT) | PASS (silenced stream, resumed repaint, deadline exit) |

Verdict line: `torture: verdict 6/6 legs passed`. CI-side corroboration
at the same SHA: the syscall-floor ledger, zero-alloc ledger, cargo-fuzz
(nightly, 60 s bounded target), Miri, Loom bounded preemptions and Kani
harnesses all green.

## 4 — Skipped follow-ups and why

- Row-RLE batching internals: audited in cbg34/p5 lock tests and the
  reader-aware emission contract (#39); no new probe added — the
  existing lock suite already asserts the byte shapes.
- A new stress leg (e.g. concurrent resize+SIGSTOP interleave): the
  signal-storm and suspend-resume legs already cover each axis, and the
  Loom lane proves the interleavings that matter are bounded; adding a
  combinatorial leg would exceed the two-minute CI law for marginal
  new coverage.
