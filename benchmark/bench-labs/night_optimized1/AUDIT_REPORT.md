<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-optimized-1 master optimization audit — peak verified, no change warranted

Audit date: 2026-09-19 · Subject: the full production tree at `f70be1b`
(HEAD before this session's commits; the same-session
NIGHT-cybersecurity-1 commit `fa5c2a7` is A/B-verified
performance-neutral, so the verdict carries forward) · Methodology:
targeted dimension-by-dimension sweeps with evidence captured on this
host (2 vCPU Xeon, 4.1 GiB, Debian 13, Rust 1.98.1, release profile).

The owner's standing rule applies: if the tree is already at peak,
skip — do not over-engineer. This audit was built to FAIL that test
if possible: every dimension below was probed for a >5 % measurable
gain (the cosmic-dragon UNLOCK bar) or a zero-cost redundancy
removal. None qualified. The evidence:

## Dimension-by-dimension verdict

| # | Dimension | Verdict | Evidence |
|---|-----------|---------|----------|
| 1 | Hot-path allocation discipline | PEAK | Steady-state 0 heap allocs/frame (alloc_calls 563 total, all startup — the locked UNLOCK logs' standing figure); report-family sinks are exit-time only |
| 2 | Per-frame clock discipline | PEAK | Every `Instant::now()` in the cloud tick path is per-frame or per-event; the historical hidden per-cell now() was already hunted out (rain_at.rs fix note) |
| 3 | Frame pacing | PEAK | Hybrid spin-sleep (poll bulk + ~500 µs spin for sub-ms deadline accuracy) with dead-PTY, clock-jump and resize-debounce guards — no busy-spin, no oversleep |
| 4 | Release profile | PEAK | opt-level 3, lto fat, codegen-units 1, strip, overflow-checks off; `panic = unwind` is deliberate (catch_unwind worker containment documented in the security audit) — abort would break that contract |
| 5 | PGO | PRESENT | Two-stage instrument/use infra (`build.sh pgo`); first PGO A/B on record: +4.5 % median FPS, −35 % worst-case frame time (PGO_AB_20260823.md) |
| 6 | Dead code | MINIMAL + DELIBERATE | 13 `#[allow(dead_code)]` in the whole tree, each a documented platform-cfg or test-only-API marker; `#[allow(unused_imports)]` sites are LOC-split re-export patterns with explanatory comments — removal would be pure churn |
| 7 | Redundant functions | INTENTIONAL PARALLELISM | Duplicate fn names (reset/advance/draw/spawn ×13 scenes, msg-fill style family ×12) are the adjudicated parallel-scene architecture (FUTURE_BACKLOG + hunt-4 records), not redundancy |
| 8 | Dependency surface | PEAK | 11 direct deps, every one production-used (verified per-crate: bitvec render.rs, smallvec rain_at.rs, unicode_width ghost.rs, sha2 configfile_dump.rs, signal_hook/ctrlc signal_handlers.rs, notify/crossterm/clap/rand obvious); minimal feature flags; chrono already removed once for 8 transitive crates |
| 9 | Cold start | PEAK | `cosmostrix -V` exits in ~1 ms on this host; no lazy heavy init on the version path |
| 10 | Long-run stability | PEAK | Bench drift −3.1 % (cinematic, second half FASTER — warm-up, not decay); endurance machinery (endurance_probe.py, reclaim_state madvise family, RIS/byte-budget xterm.js guards) all in place; the last long-run bug class (black_hole spin phase) was fixed in hunt-1 with a 13-scene audit |

## Fresh HEAD measurements (audit evidence, 10 s benches)

Baseline capture for this session's cybersecurity A/B doubles as the
current-head hardware verification (2 runs/scene, release profile,
`TERM=dumb`):

| metric | cinematic | monolith |
|---|---|---|
| avg fps | 28 902.76 / 29 218.29 * | 85 926.51 / 85 347.27 * |
| entropy bits | 5.1675 / 5.1584 | 3.2965 / 3.2950 |
| density gini | 0.6386 / 0.6408 | 0.8958 / 0.8961 |
| dirty cells/frame | 456.58 / 453.49 | 56.77 / 56.77 |

\* first value = `f70be1b` baseline, second = `fa5c2a7` (cyber-1) —
the deltas are inside the documented ±1–4 % run-noise band with the
scenes disagreeing on the sign; see
[`../night_cybersecurity1/AB_REPORT.md`](../night_cybersecurity1/AB_REPORT.md)
for the full A/B.

The numbers sit on the historical regression line for this hardware
(H37-era cinematic ≈ 28.4–28.7 K fps, monolith ≈ 85.3–86.2 K fps) —
no drift since the last LTS round.

## Conclusion

The engine core is under the LTS UNLOCK lock and every candidate
change found this pass is below its bar (a few ns/frame of clock
calls, HUD-on format! allocations behind an opt-in diagnostic
surface, comment hygiene). Touching locked files for any of these
would violate both the lock protocol and the no-over-engineering
rule. The optimization investment for this tree is already banked
(0 alloc/frame, PGO, hybrid pacing, minimal deps); the correct LTS
action is to keep the lock and re-run this audit after any future
feature lands in the frame path.

<!-- COSMOSTRIX-DISCLAIMER -->
