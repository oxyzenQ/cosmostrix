<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmostrix night-total-lts-1 audit — the cosmostrix skill (terminal reset + restore system)

Audit date: 2026-10-11
Scope: the "cosmostrix skill" — the emergency terminal reset / restore
system that this project originated and that zelynic imported as the
`--reset-terminal` skill transfer (zelynic NIGHT-hunt-31, hardened in
NIGHT-improve-30). This is the origin-side depth audit: every layer the
skill is built from, as it exists in cosmostrix today.

Peer audit: night-total-lts-2 (all-infra: scripts, CI, build.rs, supply
chain) closed at peak with zero changes needed — see
[NIGHT_TOTAL_LTS_2_AUDIT_2026-10-11.md](NIGHT_TOTAL_LTS_2_AUDIT_2026-10-11.md).
That report explicitly deferred the render-path and skill surface to this
one.

## What was read (the skill layer stack, origin side)

- `src/engine/cosmic_dragon_engine/terminal/restore.rs` —
  `restore_terminal_best_effort()`, `reset_terminal_emergency()` (the
  5-layer nuclear reset), `force_exit_terminal_restored()`,
  `restore_terminal_best_effort_nonblocking()`, the
  `TERMINAL_RESTORE_SEQUENCE` / `TERMINAL_RESET_SEQUENCE` constants.
- `src/engine/cosmic_dragon_engine/terminal/terminal_tty.rs` — the raw-fd
  primitives (`set_fd_nonblocking`, `restore_fd_flags`,
  `write_fd_best_effort`), the io-error classifiers
  (`is_recoverable_io_error`, `is_terminal_gone`), and the EAGAIN
  contract tests.
- `src/engine/cosmic_dragon_engine/terminal/cleanup.rs` —
  `cleanup_terminal()` + `mark_externally_restored()` (the SIGCONT
  double-Drop neutralization).
- `src/engine/cosmic_dragon_engine/terminal/io_recovery.rs` —
  `flush_ansi()`, `write_with_recovery()`, `recover_to_tty()`
  (/dev/tty fallback), the Tier-2 RIS + backpressure machinery.
- `src/platform/fork_guard.rs` — the SIGKILL terminal guard
  (Linux fork + `prctl(PR_SET_PDEATHSIG)`, non-Linux getppid poller,
  Windows no-op) and `termios_still_broken()`.
- `src/platform/panic_hook.rs` — main-thread-vs-worker panic
  classification, double-panic (coredump) guard.
- `src/interactive/signal_handlers.rs` — SIGTERM/SIGHUP/SIGQUIT graceful
  path with the 3 s force-exit guarantee, SIGTSTP/SIGCONT suspend/resume.
- `src/cli/early_returns.rs` — the `--reset-terminal` dispatch.
- `src/interactive/event_loop.rs` — guard install ordering.

## 1. Stability and crash — at peak

Stress-hunted failure modes and where each is already answered:

- Fork-vs-prctl race (parent dies between `fork()` and
  `PR_SET_PDEATHSIG`): captured `orig_ppid` liveness check, not a magic
  `ppid == 1` (NIGHT-hunt-47-depthbore). The Linux guard's restore is
  gated by `termios_still_broken()` so a graceful exit is never polluted
  with a redundant escape burst.
- Fork-in-threaded-parent malloc-lock risk (the classic
  `fork()` + non-async-signal-safe UB): verified the Linux guard forks at
  `event_loop.rs:56`, BEFORE any thread exists — signal threads,
  watchdog, live-config watcher, and the ambient scheduler all spawn
  after it. The fork is single-threaded by construction.
- Reparent latency window (kernel takes up to ~200 ms to finish
  reparenting after PDEATHSIG): the 300 x 20 ms patience loop reads
  liveness, then restores only if the termios is still broken.
- Wedged-PTY deadlock (Termux screen lock): every escape hatch is
  lock-free — `force_exit_terminal_restored()` and
  `restore_terminal_best_effort_nonblocking()` never touch the
  `std::io::stdout()` ReentrantMutex; writes go raw-fd with O_NONBLOCK
  and EAGAIN-drops are documented as cosmetic because SIGCONT reinit
  repaints the full frame.
- Double-panic coredump (panic inside the panic hook): the hook uses
  `write_fmt` with the error discarded; worker-thread panics are
  classified by captured thread id and left to their `catch_unwind`
  recovery (NIGHT-hunter-4), so the sticky
  `TERMINAL_RESTORED_BY_PANIC` flag only arms on real main-thread death.
- SIGTERM zombie window: the signal thread waits 3 s (30 x 100 ms) for
  `SHUTDOWN`, then force-exits with the raw-fd restore — the owner's
  reported `pkill` no-op cannot recur.
- stdout death mid-rain: `recover_to_tty()` routes through /dev/tty,
  caps recoveries at `STDOUT_FALLBACK_MAX_RECOVERIES`, signals graceful
  shutdown, and leaves the alt screen before the stderr notice.
- Install-failure degradation: both signal sources
  (`Signals::new(...)`) surface failures on stderr pre-alt-screen AND
  buffer an AB-10 runtime warning; the fork guard + watchdog backstops
  stay active.
- Every behavior above is pinned by tests where testable
  (`terminal_tty.rs` EAGAIN/round-trip/bad-fd tests, panic hook
  classification tests, `io_recovery` classifiers).

No crash, coredump, hang, or foundation-breakage vector found that is
not already covered in code. Zero changes needed.

## 2. Code hygiene — one dragon hunted and removed

- Zombie/stale-reference sweep across the skill stack and its callers
  found 14 live code references to `terminal.rs` — a file that no longer
  exists (the former 2187-line monolith, split into
  `engine/cosmic_dragon_engine/terminal/` during the LTS round). The
  stale pointers spanned the architecture map (`main.rs`), cross-module
  re-export rationales with rotted line numbers
  (`interactive/mod.rs` "terminal.rs:508"/"terminal.rs:425"),
  visibility rationale (`interactive/watchdog.rs`), bench mirror
  pointer (`bench/bench_io.rs`), split-history headers
  (`terminal_tty.rs`, `sgr_format.rs`), and the threshold-sweep
  incubator's runtime verdict strings (`cosmic_dragon_incubator/egg/
  threshold_sweep.rs`) — a future maintainer following any of them lands
  on a nonexistent file. All 14 now point at the real post-split
  locations (`terminal/mod.rs`, `terminal/draw.rs`,
  `terminal/io_recovery.rs`); the two remaining `terminal.rs` mentions
  are deliberate historical narrative ("the former terminal.rs
  monolith"), which stays accurate.
- `TERMINAL_RESET_SEQUENCE` re-states the restore prefix byte-for-byte
  instead of concatenating the const — verified deliberate: `concat!`
  accepts literals only, a const path cannot compose; the duplication is
  documented in both doc comments. Not a dragon.
- No TODO/FIXME/XXX/HACK markers anywhere in the skill stack.
- Docs `docs/archive/**` mentions of `terminal.rs` are frozen history by
  design and were left untouched.

## 3. Optimization — not applicable by domain (peak by vacuity)

The skill surface is cold-path by construction: panic hooks, signal
threads, exit paths, and rescue commands run at most once per session
and never inside the frame budget. The project's measured hot path
(fps, dirty cells, entropy, gini, alloc counts — the owner's benchmark
rubric) lives in `draw.rs` / the render loop, which this audit does not
touch. Micro-optimizing a once-per-process exit path (e.g. batching the
eight crossterm restore commands into one write) would trade crossterm's
internal state cleanup for sub-microsecond savings on a path whose cost
nobody measures — churn, not gain. The one measured-syscall guard that
exists here (the P3 3-to-1 combined sync flush) is already shipped in
`io_recovery.rs`.

## 4. Security hardening — at peak (white-hat pass)

- `reset_terminal_emergency()` spawns exactly three external programs
  (`stty sane`, `reset`, `tput reset`) with fixed argument vectors, no
  interpolation, no shell, and only when stdin or stdout is a real TTY.
  No user-shaped input reaches any command line; PATH is the user's own
  session privilege with no elevation boundary, so there is nothing to
  inject into.
- Raw-fd writes are bounds-checked slices into caller-owned buffers
  (`bytes[off..]`, `len - off`); `write_fd_best_effort` treats
  `n <= 0` as terminal — no spin, no overflow, no uninitialized reads.
  All unsafe blocks carry SAFETY comments naming the invariants.
- The fork guard's termios snapshot/restore touches only
  `STDIN_FILENO` after an `is_terminal()` gate; `MaybeUninit<termios>`
  is fully initialized by `tcgetattr` before `assume_init` in every
  path (verified all three platform branches).
- No env-var-to-Command flow exists in the skill stack; the only env
  reads (`TERM`, `COSMOSTRIX_NO_FORK_GUARD`) gate behavior, never
  construct commands or paths.
- The nuclear sequence is destructive by design (clears screen +
  scrollback) but gated behind an explicit `--reset-terminal` flag with
  a documented contract — the destructive surface is opt-in, which is
  the correct white-hat posture for a rescue tool.

No injection, memory-safety, or privilege vector found. Zero changes
needed.

## 5. LTS stability — at peak

- Platform coverage is complete and honest: Linux gets the gold-standard
  fork+prctl guard; macOS/BSD/Android/Termux get the getppid poller
  (their kernels reparent orphans to PID 1 — the subreaper trap that
  forced NIGHT-hunt-47-depthbore on Linux does not exist there, so the
  `ppid == 1` check is correct per-platform, not a copy-paste bug);
  Windows is a documented no-op because ConPTY auto-restores.
- Hidden failure modes hunted and not found: EAGAIN-dropped restore
  bytes are repainted by the SIGCONT TermReinit path; the SIGTSTP
  handler raises SIGSTOP only after the nonblocking restore; the old
  Drop-after-replacement hazard is neutralized by
  `mark_externally_restored()`; the shutdown-guard thread spawn in
  `Drop` is skipped for neutralized terminals; kitty-protocol pops are
  gated on the push flag and are stack no-ops on compliant terminals.
- Stale-documentation risk (the LTS killer for a 100+-file project) is
  the one dimension where this audit shipped a change — the 14 dead
  `terminal.rs` pointers above, fixed in the same round.

## Verdict

The skill is at peak: five layers, three platforms, every exit path
deadlock-proof, every documented bug class pinned by a test or a
SAFETY comment. The only work this audit shipped is the stale-reference
dragon hunt in dimension 2 — a comment/doc-pointer fix with zero
behavioral surface, so per the house rule no A/B benchmark is run for
it (the benchmark rubric measures density/entropy/fps/dirty-cell
behavior, none of which comments can move).

## What would falsify this verdict

- A new thread spawn introduced between process start and
  `run_interactive()` would break the single-threaded-fork invariant the
  Linux guard stands on — the guard must keep its position as the first
  thread-creating call in the interactive path.
- A terminal mode newly enabled at init (a new DECSET push) that is not
  added to both `TERMINAL_RESTORE_SEQUENCE` and `cleanup_terminal()`
  would leak that mode into the user's shell at exit.
- A new platform target whose orphan-reparent semantics differ from both
  the prctl and PID-1 models would need a third guard strategy.
<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
