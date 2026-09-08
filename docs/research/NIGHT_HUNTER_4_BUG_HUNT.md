<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-4 — hidden-bug hunt: panic-hook worker containment + phosphor full-grid guard

> Owner mandate (2026-09-08): free cosmostrix from potential hidden
> bugs, premature logic, and other problems via a depth audit. Two
> genuine defects were found, root-caused, fixed, and pinned with
> regression tests. The verified-safe catalog below records what was
> hunted and found clean, so the next hunt does not re-till this
> ground.

## Bug 1 — the panic hook fired for panics the code had already designed to survive

**Severity: genuine hidden bug (premature logic, exactly the owner's
suspected class).**

**Root cause.** Rust runs the global panic hook BEFORE unwinding —
including for panics that a worker thread's `catch_unwind` is about to
catch and recover from. cosmostrix's recovery design is explicit and
layered: the config watcher, the config poller, and the ambient
scheduler all wrap their bodies in `catch_unwind`, the poller even
restarts after a 1-second backoff, and the diagnostics are buffered to
the post-exit runtime-warning log (the AB-10 contract: never leak a
stderr line into the rain matrix). The v25 panic-hook doc comment
shows the author believed the hook stayed out of the way of that
recovery ("any panic in worker threads ... is cleanly caught by
`catch_unwind` instead of escalating to abort") — the escalation part
was true, but the hook still RAN for those caught panics.

**Failure mechanism, step by step.**

1. A worker thread panics (e.g. notify's internal FDs on terminal
   close — the documented trigger the watcher's catch_unwind exists
   for).
2. The global hook fires first: it restores the terminal
   (`restore_terminal_best_effort()` — leaving the alternate screen
   and disabling raw mode) while the main loop is still mid-rain, and
   it prints the panic message to stderr.
3. `catch_unwind` catches the panic; the poller restarts; the main
   loop keeps rendering — ANSI rain now spews into the user's normal
   shell screen on a de-raw-moded terminal.
4. The hook also armed `TERMINAL_RESTORED_BY_PANIC` — a flag with
   exactly one store site and no reset. After any later SIGTSTP /
   SIGCONT cycle re-creates the Terminal, the final `Terminal::drop`
   sees the sticky flag and SKIPS `cleanup_terminal()` — leaking raw
   mode / the alternate screen at process exit.

**Fix.** `src/platform/panic_hook.rs` now captures the installing
(main) thread's id and performs terminal teardown only for
main-thread panics — the only panics that escape to process death.
Worker-thread panics return early from the hook and keep their
designed recovery path (catch_unwind + buffered diagnostics + poller
restart). The module documentation was rewritten to state the
hook-before-unwind semantics correctly.

**Tests.** Three contracts pinned in `panic_hook.rs`'s test module:
the installing thread classifies as main; a spawned worker does not;
and a real worker-thread panic under the installed hook leaves
`TERMINAL_RESTORED_BY_PANIC` clear (the hook is swapped in and out
around the worker panic so parallel tests are unaffected).

## Bug 2 — the phosphor full-grid scan indexed FRAME space with CLOUD dimensions, unguarded

**Severity: latent out-of-bounds panic (defense-in-depth gap; not
reachable on any current code path).**

**Root cause.** The full-grid branch of `phosphor_decay_pass` (taken
when `dirty_all` is set AND the dirty list is empty — the
`clear_with_bg` / semantic-invalidation path) iterates
`0..self.lines x 0..self.cols` (cloud dimensions) but computes
`fidx = line * frame_width + col` and passes it to
`frame.cell_written_this_frame(fidx)` / `frame.cell_at_index_ref(fidx)`
— direct-indexed FRAME buffers with no internal bounds check. The
sibling dirty-index branch of the same pass has guarded the
mirror-image divergence since the HUNT-25 era
(`if line >= lines || col >= self.cols { continue; }`); the full-grid
branch simply lacked the inverse guard.

**Why it matters.** Every current construction site pairs cloud and
frame dimensions (setup, resize, rebuild, ambient, intro, and the
bench harness all rebuild both with identical clamps), so the
invariant holds today and no panic is reproducible. But the invariant
is implicit — one future refactor that mutates only one side (cloud
without frame, or vice versa) converts this scan into a per-frame
panic on the semantic-invalidation path. The repo's own history
(HUNT-25, HUNT-26, HUNT-27) shows exactly this class of
cross-subsystem invariant drift recurring.

**Fix.** `cloud/phosphor.rs`: the full-grid scan now guards
`line >= frame.height` (break — rows ascend) and
`col >= frame_width` (break — columns ascend), mirroring the sibling
branch's tolerance with zero per-cell overhead (one compare per row
plus one per column-leading-cell).

**Tests.** Four contracts pinned in `tests_phosphor.rs`: the
divergence tolerance on both axes independently (shorter frame,
narrower frame), both axes at once (smaller frame), and the control —
paired dimensions still capture fresh cells through the full-grid
branch (guards must not over-skip).

## Verified safe (hunted clean — do not re-audit without new signal)

- **Divisions**: `point_system` (total_weight guard), `pool_lifecycle`
  (cps max(0.001) + seed max(1)), `sensor` (wall_delta zero guard),
  flux P2G (weight epsilon guard) — all guarded.
- **Unsigned underflow / OOB**: `black_hole` and `physarum_helpers`
  zero-dim guards, `aeolian` string indexing guards,
  `rain_post` / `brightness_factors` (lines < 2H guard),
  spawn-scan empty-pool guards, `hud` index-1 zero guard — all
  guarded. Note `overflow-checks = false` in both cargo profiles:
  the guards are the protection, wraps surface as OOB not panics.
- **clamp(min,max)**: every clamp pair checked — all literal-ordered
  constants, zero computed-bound pairs found.
- **Production unwraps/expects**: all 126 candidates triaged — all are
  either invariant-backed `Uniform::new_inclusive` with constant
  ranges (documented at each site) or inside `#[cfg(test)]` modules.
- **Atomics**: `GRACEFUL_SHUTDOWN` Release/Acquire pairs correct;
  `FRAME_COUNTER` Relaxed load is correct (monotonic progress);
  `TermReinit` swap AcqRel; ambient generation SeqCst paired with
  lock+condvar.
- **Channels**: all mpsc are bounded `sync_channel(64)` with
  `try_send` (watcher, poller, ambient deliver with Full vs
  Disconnected discrimination) — no unbounded growth, no
  send-blocking after exit.
- **Terminal lifecycle pairing**: no skipped-restore exit path (9
  restore sites cover panic, SIGTSTP, Windows Ctrl-Break, watchdog,
  /dev/tty recovery, run_interactive Err, fork guard x3).
- **Exit codes**: 0/1/2/130 coherent and matrix-documented.

## A/B verification

Owner rule applied (10 s dry benches, before/after, pro build):
cinematic (the phosphor-owning glyph path) and monolith
(structured-family control). Zero visual regression: dirty-cell
populations, entropy, and gini match to the third decimal. FPS deltas
(+0.23% / +1.11%) are inside the documented same-tree noise band.
Full data: `benchmark/bench-labs/night_hunter4/AB_REPORT.md`.
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
