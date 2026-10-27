<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-dragon-lts-1 audit — the sharpest skill: the full terminal reset/restore stack, including the render-path terminal files every prior audit deferred

Owner brief (2026-10-27): "night-dragon-lts-1. depth audit focus for
sharpest skill. ultimate total lts, realiable all round and keep be
honest. cross check audit all root repo focus, if peak skip and
continue the next step" — with the five-dimension rubric (stability
and crash, code hygiene, optimization, security hardening, LTS
stability) and the standing rule: if a sub-task is already at peak,
skip and note why.

## Audit scope and method

The sharpest skill is the layer that owns the user's terminal: the
reset/restore guarantee. This audit closes the surface the 2026-10-11
night-total-lts-1 report deliberately narrowed — that pass read the
restore primitives; this one reads the WHOLE stack: all nine
`src/engine/cosmic_dragon_engine/terminal/` files (mod, draw, tier2,
terminal_tty, io_recovery, restore, cleanup, sgr_format, last_frame)
plus the platform guards (`src/platform/fork_guard.rs`,
`src/platform/panic_hook.rs`) — ~3,400 lines, five of which (draw,
mod, tier2, sgr_format, last_frame) no prior audit had covered. Method:
full reads, pattern hunts (panic/unwrap/index/unsafe/signal/Drop),
cross-module invariant verification (frame.rs contracts, event_loop
call sites, spawn ordering), cross-target compile checks
(x86_64-linux + aarch64-apple-darwin for the cfg'd branches), and a
release-binary 10 s benchmark A/B against the pre-audit head 2d16779.

## What changed because of this audit (3 commits, all pushed)

- `ee8d4dd` — the non-Linux fork guard (macOS/BSD/Termux poller) still
  decided liveness with `getppid() == 1`, the exact magic-value check
  the Linux path's own NIGHT-hunt-47-depthbore comment bans: an orphan
  lands on the SUBREAPER in containers, so the probe never fires and
  the SIGKILL guarantee silently degrades on every non-Linux platform.
  A textbook fix-not-propagated to the sibling. The poller now captures
  the renderer's ppid once and fires on any move away from it, closing
  both directions (missed subreaper orphans AND false restores in
  legitimate ppid==1 sessions). Verified with cargo check on linux and
  the darwin target (the changed cfg arm).
- `d449d7b` — the fourth shadow-honesty trigger. NIGHT-hunter-34 closed
  the class for backpressure suppression and RIS reset; the plain
  write-error branch of `flush_ansi` still left every changed cell
  believed-written but never emitted, under a comment promising a retry
  the draw loop never performs (it clears `ansi_buf` at frame start).
  The error branch now arms `force_full_emit`, symmetric with the two
  reachable sibling arms, with the reachability stated honestly: every
  current caller propagates the error and unwinds, so the arm is
  latent-correctness that keeps the class closed by construction.
- `e90e537` — the hygiene dragon hunt (details in rubric 2 below).

## The five rubric dimensions

### 1. Stability and crash — two real findings, both fixed; the rest at peak

Hunts performed: every `panic!`/`unwrap()`/`expect()`/`unreachable!()`
(two `expect`s in draw.rs, both logically unreachable and commented as
such; zero other hits outside tests), every direct index into
`last.cells` (five sites, all under the dimension invariant below),
every `unsafe` block (table in rubric 4), loop termination (diff-path
walk strictly advances; `write_fd_best_effort` terminates on
`n <= 0`; the fork-guard reparent wait is bounded at 300 x 20 ms),
integer overflow (all hot arithmetic saturating/wrapping-with-reset or
clamp-bounded by the 1024x500 terminal-size clamp — a zeroed/garbage
winsize cannot OOM), and Drop/reentrancy (double-Drop prevented by the
`cleaned_up` guard + `mark_externally_restored`; the SIGCONT
assignment-ordering hazard is explicitly handled).

SKIP RATIONALE (diff-path direct indexing, draw.rs:375 et al — left as
is): the invariant `last.cells.len() == w*h` is enforced by
construction (`can_reuse_last` requires equal dimensions; resize
reallocates and forces full redraw) and `debug_assert!`ed; adding
per-cell bounds checks to the hottest loop in the engine to guard a
structurally guaranteed invariant would tax every frame to protect
against a bug that cannot be reached without breaking the allocator
first. Release-mode indexing panics loudly at the same site the
debug_assert names, so diagnosability is not lost.

Residual, documented not fixed (LOW): if the Drop shutdown-guard
thread spawn fails (`let _ =` at mod.rs:623-644), the 2 s local
force-exit is absent for that session. Backstopped by the session
watchdog's stuck-frame arm (only when >= 1 frame rendered) and the
fork guard. Thread spawn failure is OOM-territory; the added code and
a new failure surface were judged worse than the residual.

### 2. Code hygiene — one batch shipped, one honest retraction

Fixed in `e90e537`, all zero-behavior-change: the module doc claiming
a "64 KiB" writer against its own 256 KiB constant; stale line-number
references to the moved `recover_to_tty`; a `// SAFETY:` mislabel on
safe error-discarding code in panic_hook.rs; a vestigial
`allow(unused_variables)` on `force_exit_terminal_restored` (both
params used in both cfg arms); the watchdog doc citing
`FD_HEALTH_PROBE_INTERVAL_FRAMES` as 3600 when the constant is 600
(the doc now defers to the constant instead of re-deriving seconds);
`blank_cell` byte-duplicating `Cell::blank_with_bg` (now delegated);
and the fork-before-any-thread ordering invariant pinned in source
where the risk lives, not just in prose. Docs synced to the orig-ppid
semantics (architecture ASCII box + Layer 2 steps, lifecycle matrix
SIGKILL section — whose macOS/BSD bullet now states honestly that the
polling thread covers parent death but not a SIGKILL of the renderer
itself).

The honest retraction: this audit's pattern pass flagged the
`TERMINAL_*_SEQUENCE` re-exports in terminal/mod.rs as dead (masked by
`allow(unused_imports)`) and the fix commit initially removed them.
`cargo check --all-targets` caught the real consumer —
test/tests/terminal.rs, 22 usages via `crate::terminal::` — that a
`src/`-only grep misses. The re-exports were restored and the comment
now names the actual test-only consumer. Lesson recorded in source:
repo-wide means repo-wide, and the compiler outranks the grep.

Sequence-drift dedup (deriving `TERMINAL_RESET_SEQUENCE` from
`TERMINAL_RESTORE_SEQUENCE`) — SKIP RATIONALE: the superset
relationship is documented as intentional, both constants are const
contexts (a `format!`-derived reset would break const usage), and
touching the nuclear reset path to save six duplicated bytes is risk
without payoff; the sequence-content test suite already pins both.

### 3. Optimization — at peak for this pass, with numbers

No actionable micro-optimization was found in the stack: the draw path
is already RLE-diff with flat dirty-index buffers and single-sort
grouping, every buffer is reused across frames (take/restore, zero
per-frame alloc), tier2's ByteWindow is a fixed 600-slot ring, and
memory stays frame-bounded (<= 2x frame transiently during combined
flush) even under permanently failing writes. The benchmark A/B
(release build, `--benchmark` dry mode, ~7 s per run, pre-audit head
2d16779 vs audit head e90e537): avg_fps 89,886 vs 89,407 with the
identical-binary repeat band spanning 89,406-89,886 (the delta is
inside run-to-run noise), p99 frame time 15.3 vs 15.1 us, dirty
cells/frame 56.8 vs 56.8, frame entropy 3.29 vs 3.29, density gini
0.8961 vs 0.8962 — visual metrics identical, performance parity. SKIP
RATIONALE for deeper micro-opt: no profiling data available beyond the
benchmark's own counters, and the only candidate surfaces (the
diff-path inner loop) are guarded by the same rubric-1 skip — measure
first, and the project's own PGO/bench-labs infrastructure is the
owner's instrument for that.

### 4. Security hardening (white-hat / kernel-hacker pass) — at peak

Every `unsafe` block in the stack was enumerated with the invariant it
assumes: fcntl flag flips, raw `write`, tcgetattr/tcsetattr, fork/
prctl/sigwait — all consume process-owned fds and return-checked libc
values; no block's memory safety depends on terminal CONTENT (a
hostile terminal with garbage ioctl returns and a zeroed winsize
reaches, at worst, the covered hang/exit paths). The engine never
reads from the terminal: no DA/cursor-position queries are sent, the
/dev/tty fallback handle is opened write-only, and the only readers
are crossterm's bounded event calls — there is no cosmostrix-owned
unbounded read to exploit. No C signal handlers exist (signal_hook
threads + `raise()` syscall only), so async-signal-safety violations
are structurally impossible. No `static mut`; all cross-thread state
is atomics with consistent orderings. Residual (documented, outside
this stack): crossterm's `read()` after `poll()==true` can block on a
partial escape sequence from a hostile TTY — bounded operationally by
the session watchdog's stuck-frame force-exit, not by a read timeout.
Acceptable; recorded for the next audit.

### 5. LTS stability — the real debt is named, and one piece of it is now in source

The stack's correctness rests on cross-module invariants that lived
only in prose: shadow-before-flush, frame/LastFrame dimension
agreement, fork-before-any-thread. This audit moved the sharpest of
them (fork ordering) into the source next to the `fork()` call, closed
the one un-armed trigger of the shadow-honesty class, and ported the
liveness rule to the sibling platform so the guard family behaves
identically everywhere Unix does. What remains for the next auditor,
honestly: the Drop spawn-failure residual (rubric 1), the tier-2
cosmetic windows (RIS bytes delayed one suppressed flush; /dev/tty
fallback can duplicate already-flushed bytes), the probe consuming the
shared recovery budget (harmless today — any successful recovery ends
the loop anyway), and the prose-invariant debt itself (dimension
agreement and shadow ordering are still comment-documented, enforced
by debug_assert). None of these is a hidden failure mode today; all
are named so they cannot become one quietly.

## What would falsify this verdict

- A thread spawned before `interactive/event_loop.rs:56` invalidates
  the fork-guard child path (the invariant is now pinned in source).
- A draw caller that tolerates a failed `flush_ansi` instead of
  unwinding becomes the first live consumer of the new arm — and the
  `flush_stdout_timed` failure sites (draw.rs:294 and the diff-path
  tail) would need the same arm if such a caller ever appears.
- A platform where an orphan's ppid does NOT move away from the
  original parent (pids reused across a 500 ms window in a
  pid_namespace with aggressive recycling) would weaken the poller —
  the Linux PDEATHSIG path is immune by construction.

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
