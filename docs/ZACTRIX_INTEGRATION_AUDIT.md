<!-- SPDX-License-Identifier: MIT -->

# Zactrix Integration Audit

v4.8.0 Phase 1 is the Zactrix lab integration audit. It starts from the
released v4.7.0 mainline and prepares a safe path for render efficiency
work without changing runtime visuals, benchmark semantics, or terminal
ownership.

This is not a direct lab branch merge. This is not a 50k FPS promise.

## Baselines

| Track | Branch / Commit | Role |
|-------|-----------------|------|
| Main baseline | `main` / `52a558b` | Released v4.7.0 stability baseline |
| Accepted candidate source | `zactrix-20k-lab` / `e7253e7` | Reduce redundant color pipeline work |
| Boundary evidence | `zactrix-50k-lab` / `23c97a6` | 50k ceiling research closure document |

## Accepted Candidate

The accepted optimization candidate is:

- reduce redundant color pipeline work

The candidate appears in `e7253e7` and changes these relevant files:

- `src/palette.rs`
- `src/droplet.rs`
- `src/frame.rs`
- `src/cloud/monolith.rs`
- `src/cloud/phosphor.rs`
- `src/cloud/rain.rs`
- `src/cloud/render.rs`
- related depth and documentation guard tests

The safe-looking pieces are small and scalar:

- decode terminal colors once, then reuse RGB tuples through brightness and
  blend steps
- use fixed-point RGB helpers for brightness and white/core blending
- cache whether the current character pool is binary inside draw context
- provide a forced frame-cell write helper for known cleanup paths

These are Phase 2 adaptation candidates. They still require visual parity,
depth regression, benchmark honesty, and current-main conflict review before
they can land.

## Mainline Adaptation Notes

The v4.7.0 mainline includes profile ecosystem and release-candidate guard
work that was not the focus of the performance lab. Integration should adapt
individual changes onto current main instead of merging either lab branch.

Known adaptation pressure:

- `src/docs_tests/zactrix.rs` is already large, so new guards should use a
  separate test module.
- benchmark fields must remain stable and honest.
- terminal execution remains single-threaded for rendering.
- no worker-thread or parallel terminal write experiment is part of Phase 1.
- AUR metadata, version files, and release tags are out of scope.

## 50k Lab Closure

`zactrix-50k-lab` is documentation evidence only for this integration branch.
It records that 50k FPS was not reached and that extra attempts beyond the
accepted color-pipeline candidate were neutral or slower.

Rejected 50k attempts stay rejected:

- Frame dirty epoch stamps
- Monolith stale-only cleanup
- Edge-fade line cache
- Non-TTY benchmark progress elapsed gate

The closure document is kept in `docs/ZACTRIX_50K_LAB.md` so future work can
see the boundary evidence without replaying rejected experiments.

## Invariants

Phase 1 and later integration work must preserve these benchmark and runtime
invariants:

- dirty ratio roughly 6.8%-7.6%
- active_streams_avg roughly 40-42
- active_frame_ratio 100%
- actual_execution single-threaded-renderer
- terminal_writer single-owner
- compute_parallelism disabled

Optimization must not reduce visual density, reduce active streams, fake
dirty-cell ratios, remove benchmark fields, spawn worker threads, or parallelize
terminal writes.

## Integration Rules

- no direct merge from lab branches
- cherry-pick or adapt only clean changes
- visual equivalence required
- benchmark honesty required
- no version bump during Phase 1
- no v4.8.0 tag or release during Phase 1
- no generated benchmark dumps, logs, or videos in git

## Phase 1 Decision

Code integration is deferred to Phase 2. Phase 1 records the audit, imports the
50k closure evidence, and installs documentation guards so the v4.8 path stays
honest before any render-path optimization lands.

## Phase 2A — Code Integration (COMPLETE)

Commit: `ce8dc81 perf(v4.8): integrate zactrix color pipeline optimization`

Source commit: `e7253e7` (on `zactrix-20k-lab`)

Integration method: manual adaptation of individual safe color-pipeline
optimizations onto the v4.7.0 mainline. No direct lab branch merge.

### Accepted Optimizations

- **Single RGB decode per cell** — terminal colors are decoded once, then the
  resulting RGB tuple is reused through brightness and blend steps instead of
  being re-decoded.
- **Integer fixed-point blend/brightness math** — RGB helpers use fixed-point
  integer arithmetic for brightness scaling and white/core blending, avoiding
  per-component float conversion.
- **Combined `layer_brightness * glyph_dim`** — brightness and glyph dimming
  are multiplied once and reused, eliminating a redundant multiply per cell.
- **`set_force` for known-dirty monolith cleanup cells** — cells that are
  known to be dirty during monolith cleanup use a forced write helper instead
  of the general conditional-dirty path.
- **Cached `pool_is_binary`** — whether the current character pool is binary
  is cached once per draw context instead of being recomputed per cell.
- **Head self-bloom precomputed constant** — the head cell's self-bloom factor
  is precomputed as a constant instead of being derived per frame.

### Rejected (from 50k lab, remain rejected)

- Frame dirty epoch stamps
- Monolith stale-only cleanup
- Edge-fade line cache
- Non-TTY benchmark progress elapsed gate

50k FPS is not a release promise. See `docs/ZACTRIX_50K_LAB.md`.

### Phase 2A Benchmark (3-run, previous session)

| Run | avg_fps | median_fps | p95 ms | p99 ms | dirty % | streams | stability |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 1 | ~26900 | ~27200 | ~0.041 | ~0.042 | ~7.21 | 41 | excellent |
| 2 | ~27800 | ~28100 | ~0.039 | ~0.040 | ~7.21 | 41 | excellent |
| 3 | ~27200 | ~27500 | ~0.041 | ~0.042 | ~7.21 | 41 | excellent |

Invariant labels (all runs):

- `actual_execution`: `single-threaded-renderer`
- `terminal_writer`: `single-owner`
- `compute_parallelism`: `disabled`
- `active_frame_ratio`: `100.0%`

### Integration Safety

- No direct merge from `zactrix-20k-lab` or `zactrix-50k-lab`.
- No version bump, tag, release, or AUR metadata change.
- All 869 tests passed after integration.
- Visual parity not yet confirmed by interactive smoke (deferred to Phase 2B).

## Phase 2B — Validation Lock (CURRENT)

Commit: pending

Goal: lock Phase 2A with full validation, 5-run benchmark, documentation
update, and honest visual-smoke status. No new optimization work unless fixing
a validation failure.

### Phase 2B 5-Run Benchmark

| Run | avg_fps | median_fps | p95 ms | p99 ms | dirty % | streams | stability |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 1 | 27861.3 | 28144.4 | 0.039 | 0.040 | 7.21 | 41 | excellent |
| 2 | 28034.5 | 28287.3 | 0.039 | 0.041 | 7.21 | 41 | excellent |
| 3 | 28008.1 | 28297.3 | 0.039 | 0.041 | 7.21 | 41 | excellent |
| 4 | 27634.9 | 28282.5 | 0.039 | 0.041 | 7.21 | 41 | excellent |
| 5 | 27963.0 | 28290.9 | 0.039 | 0.041 | 7.21 | 41 | excellent |

5-run mean avg_fps: `27900.4`

Invariant labels (all runs):

- `actual_execution`: `single-threaded-renderer`
- `terminal_writer`: `single-owner`
- `compute_parallelism`: `disabled`
- `active_frame_ratio`: `100.0%`

### Visual Smoke

manual visual smoke not run because session is non-interactive.

### Validation Results

- `cargo fmt --all -- --check`: PASS
- `cargo test --all --locked`: 869 passed, 0 failed
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: PASS
- `./build.sh check-all`: PASS (skipped: sccache, mold/lld, cargo-nextest, cargo-audit, cargo-deny)
- `./version-to.sh --check 4.7.0`: PASS
- `cargo pro-linux-v3`: PASS
- `./scripts/rc-smoke.sh`: PASS
- `bash -n scripts/monitor-cosmostrix.sh`: PASS

## Phase 3 — Main Merge Prep / Conflict Audit (CURRENT)

Commit: pending

Goal: prepare `v48-zactrix-integration` for eventual merge into `main` by
auditing conflicts, diff scope, docs, tests, and release-readiness. Do not
perform the merge.

### Merge-Prep Status

- Branch: `v48-zactrix-integration`
- Commits ahead of main: 3 (Phase 1, 2A, 2B)
- Changed files: 17 (docs, color pipeline, tests — no release metadata)
- `origin/main` is an ancestor of HEAD (fast-forward eligible)
- `git merge-tree` conflict check: zero conflicts
- No AUR metadata touched
- No version bump — v4.8 remains unbumped until release prep
- No tag or release created
- No direct lab branch merge
- 50k FPS not reached and not a release promise
- Locked integration benchmark: 27,900.4 FPS (5-run mean)

### Merge-Readiness Summary

The branch is a clean fast-forward candidate onto `main`. All changes are
limited to:

- Color pipeline optimizations (src/palette.rs, src/droplet.rs, src/frame.rs,
  src/cloud/monolith.rs, src/cloud/phosphor.rs, src/cloud/rain.rs,
  src/cloud/render.rs)
- Documentation (docs/ZACTRIX_INTEGRATION_AUDIT.md, docs/ZACTRIX_50K_LAB.md,
  docs/ROADMAP.md)
- Guard tests (src/docs_tests/zactrix_integration.rs, src/docs_tests/zactrix.rs,
  src/docs_tests/mod.rs)
- Depth/visual regression tests (src/cloud/tests/)

No release metadata, AUR files, version bumps, or configuration defaults were
modified. The expected merge target is `main` after owner review. Manual
visual smoke is owner-side/local if the environment is non-interactive.

### Invariant Confirmation (Phase 3)

- `terminal_writer`: single-owner
- `compute_parallelism`: disabled
- `actual_execution`: single-threaded-renderer
- `avg_dirty_cell_ratio`: 7.21%
- `active_streams_avg`: 41
- `active_frame_ratio`: 100%
- No version bump until release prep

## Phase 4 — Terminal Kill Cleanup / Signal Exit Hardening (COMPLETE)

Commit: `a3ac896`

Owner-side visual smoke found that `pkill -f cosmostrix` left Matrix rain
glyph residue on the terminal screen and prompt lines. Investigation
revealed the signal handler fallback path (`restore_terminal_best_effort()`
+ `process::exit()` after 1-second timeout) raced on stdout with the main
loop's buffered writer and skipped `Terminal::drop()`.

Fix: signal handler threads (SIGINT, SIGTERM, SIGHUP) now set
`GRACEFUL_SHUTDOWN` and block until `SHUTDOWN` is observed. They no longer
call `restore_terminal_best_effort()` or `process::exit()` themselves. The
watchdog thread (20-second stuck-loop timeout) remains the sole hard
fallback.

SIGKILL (`kill -9`) cannot be caught by any process. On Linux, the
fork-based guard (`cx-term-guard`) provides best-effort recovery. See
`docs/TERMINAL_KILL_CLEANUP.md` for full documentation.

v4.8 merged to main. Owner-side visual smoke confirmed clean.

## Phase 4B — Signal-Exit Visible Residue Cleanup (COMPLETE)

Commit: `0eaf691`

Owner-side visual smoke after Phase 4 found that `pkill -TERM` still left
visible Matrix rain residue on the terminal screen despite terminal modes
being restored correctly. Two root causes were identified:

### Root Cause 1: Fork Guard Stdout Race

The fork-based `cx-term-guard` child process (created via `libc::fork()`)
previously called `restore_terminal_best_effort()` on any received SIGTERM,
including when the parent received `pkill -TERM` and was handling cleanup
via `Terminal::drop()`. Since both processes share the same stdout file
descriptor, this produced a race: the child's ANSI escape sequences
interleaved with the parent's buffered writer output, causing garbled
escape sequences and glyph residue on the main screen.

Fix: the child now checks `getppid()` after receiving SIGTERM. If ppid
is not 1 (parent still alive), the child exits silently without writing
to stdout — the parent's `Terminal::drop()` handles all cleanup. Only
when the parent is already dead (ppid == 1, indicating SIGKILL or crash)
does the child perform terminal restoration.

### Root Cause 2: No Viewport Clear Before Alternate Screen Switch

The renderer uses the alternate screen buffer. On signal exit,
`Terminal::cleanup_terminal()` called `LeaveAlternateScreen` without
first clearing the visible viewport inside the alternate screen. Terminal
emulators can briefly show the last alternate screen content during the
buffer switch, leaving rain frame glyphs visible on the main screen.

Fix: `Terminal` now accepts a `signal_exit: Arc<AtomicBool>` flag. When
set (by signal handler threads for SIGINT/SIGTERM/SIGHUP),
`cleanup_terminal()` writes `MoveTo(0,0)` + `Clear(All)` + flush inside
the alternate screen before issuing `LeaveAlternateScreen`. Normal q/esc
exit does not set this flag, so normal exit remains non-destructive — the
alternate screen switch alone cleanly restores the original content.

### Changed Files

- `src/terminal.rs`: Added `signal_exit` field to `Terminal`, added
  `with_signal_exit()` constructor, updated `cleanup_terminal()` to
  clear viewport on signal exit, added tests.
- `src/interactive/event_loop.rs`: Signal handlers set `signal_exit`
  flag alongside `GRACEFUL_SHUTDOWN`. `Terminal::with_signal_exit()`
  used instead of `Terminal::new()`.
- `src/interactive/watchdog.rs`: Removed unused `SIGNAL_EXIT_REQUESTED`
  static (replaced by local `Arc<AtomicBool>`).
- `src/main.rs`: Fork guard child now checks `getppid()` before
  restoring terminal on SIGTERM.
- `docs/TERMINAL_KILL_CLEANUP.md`: Full rewrite with process model,
  signal-exit viewport clear documentation, fork guard race fix,
  manual test instructions.
- `docs/ROADMAP.md`: Updated to Phase 4B (current).
- `src/docs_tests/zactrix_integration.rs`: Added guards for Phase 4B.
