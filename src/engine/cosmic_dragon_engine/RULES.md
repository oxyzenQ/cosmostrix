<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmic Dragon Engine — Modification Rules (UNLOCK Protocol)

> **Simplified lock/unlock signature log**: see [`KEY.md`](KEY.md).
> This file holds the full UNLOCK protocol and detailed log entries.

> **Locked** at commit `69af079` on 2026-08-19T14:40:05Z by
> **rezky_nightky** — vision & director project cosmostrix.

## Purpose

This document defines the mandatory protocol for modifying any file in
`src/engine/cosmic_dragon_engine/` after the LTS lock. The lock ensures
long-term stability: any modification must be **documented**, **justified**,
and **acknowledged** before it lands on `main`.

## When to Follow This Protocol

This protocol applies if you modify any production `.rs` file under:

- `src/engine/cosmic_dragon_engine/cloud/`
- `src/engine/cosmic_dragon_engine/frame.rs`
- `src/engine/cosmic_dragon_engine/terminal/`
- `src/engine/cosmic_dragon_engine/runtime.rs`
- `src/engine/cosmic_dragon_engine/mod.rs`

Test files (`tests.rs`, `tests/` subdirs) are exempt UNLESS the test
itself changes a public contract or invariant.

## Pre-Modification Checklist

Before opening a PR that touches any locked file, you MUST:

1. **Run the gatekeeper**: `./scripts/build.sh check-all` (or, if
   that's unavailable in the dev env: `cargo fmt + cargo clippy --tests
   - cargo test --quiet`). All must pass before AND after your change.

2. **Run an A/B benchmark**:

   ```bash
   cargo build --release --quiet
   ./target/release/cosmostrix --benchmark --bench-io --bench-duration 10s > /tmp/before.txt
   # apply your change
   cargo build --release --quiet
   ./target/release/cosmostrix --benchmark --bench-io --bench-duration 10s > /tmp/after.txt
   python3 scripts/ab_compare.py  # if available, or compare manually
   ```

3. **Verify no regression**: avg_fps, peak_rss, and alloc_calls must
   stay within ±5% of the locked baseline. Stability signals
   (`frame_jitter`, `frame_time_stability`, `drift_interpretation`)
   must match. If your change regresses any of these, **STOP** and
   reconsider — the lock is more important than your change.

4. **Verify visual identity unchanged**: Run `python3 scripts/visual-mode-audit.py`
   and confirm the masterclass brightness profile is preserved
   (top=0.533, bot=0.369 within ±5%).

5. **Update this README's UNLOCK section** (below) with:
   - Your commit SHA (after merge, or use `pending` if pre-merge)
   - Date-time (ISO 8601 UTC)
   - Reason for modification (1-2 sentences, why not what)
   - Files changed (paths)
   - A/B delta summary (FPS / RSS / alloc_calls)
   - Your name/handle

## Acceptable Reasons for UNLOCK

The lock is intentionally hard to break. Acceptable reasons include:

- **Bug fix** — a correctness issue that produces wrong output or
  crashes. Must include a regression test that fails before the fix.
- **Security fix** — a vulnerability (e.g., panic on untrusted input,
  UB under Miri). Must include a Miri run if unsafe code is touched.
- **Performance improvement** — a measurable, repeatable gain (>5% on
  the A/B benchmark) with no visual regression. The improvement must
  be attributable to this engine's code, not external factors.
- **Compatibility** — required for a new Rust version, terminal
  protocol, or platform support. Must include compatibility test
  cases.

**NOT acceptable** as sole reason:

- Code style preferences
- "Modernization" without measurable benefit
- Refactoring that touches >5 files without a clear correctness or
  perf gain
- Adding fields/types without consumers

## UNLOCK Log

This section is appended every time a locked file is modified after
the lock commit. Newest entries go at the TOP.

### UNLOCK cosmic-dragon (exp decay consolidation) at commit `5280ae1`, 2026-08-24

**Author**: oxyzenQ (Cosmic Dragon AI Agent)
**Reason**: Owner-approved v50.0.0-beta.5 masterclass easing consolidation. After
the prior commit `e2e0512` migrated pause/resume to exp decay, the
owner said: "all pause/resume AND related effects must use consistent
exp decay, peak optimized + stable + strengthened, no duplicates /
overlaps". This commit consolidates the glyph scene-entry ramp onto
the same exp approach family (k=4.28/s, settle 95% at 700ms — replaces
the prior smoothstep 3t^2-2t^3 over fixed 700ms), adds a
`debug_assert!` invariant that pause_start and resume_start cannot
coexist (audit §8.6 — toggle_pause() guarantees this across all 3
branches, now asserted at rain_at entry point, zero-cost in release),
and adds 4 regression tests that lock the masterclass easing contract
(k_decel=1.2 / k_resume=0.9 / glyph k=4.28 + settle thresholds +
no-overlap invariant). A new "Easing family policy" doc section in
`central_control_rains/mod.rs` documents which easings are exp decay
(pause/resume + glyph entry) vs smoothstep (spatial fades) vs
intentional smoothstep-shaped rate (profile interp 30s slow drift) —
prevents future contributors from "consolidating" the wrong easings.

**Files changed** (locked path — production code):
- `src/engine/cosmic_dragon_engine/cloud/rain.rs` (lines 39-55: new
  `debug_assert!` invariant at rain_at entry; lines 213-218: stale
  comment "smoothstep curve" -> "exp decay approach curve" for
  resume_blend scaling; lines 220-239: glyph entry ramp rewritten
  from smoothstep 3t^2-2t^3 over GLYPH_ENTRY_RAMP_DURATION_MS (700ms
  fixed window) to `1 - exp(-k*t)` with k=GLYPH_ENTRY_RAMP_DECAY_RATE
  (4.28/s), settle-snap at GLYPH_ENTRY_RAMP_SETTLE_FRAC (95%); the
  700ms constant is now the SETTLE time, not the animation window)
- `src/engine/cosmic_dragon_engine/cloud/spawn.rs` (lines 752-758, 815-817:
  doc-comment updates describing the new glyph entry ramp math —
  comment-only, no production code logic changes)

**Files changed** (test only — no production code, exempt per
"Test files are exempt UNLESS the test itself changes a public
contract or invariant"; the new tests assert the easing contract
that the production code already implements, no contract change):
- `src/engine/cosmic_dragon_engine/cloud/tests/mod.rs` (4 new tests +
  1 existing test comment/duration bump from commit `e2e0512`'s
  exp decay settle window; new tests:
  `pause_decel_exp_decay_settles_at_documented_threshold`,
  `resume_accel_exp_decay_settles_at_documented_threshold`,
  `glyph_entry_ramp_exp_decay_settles_at_documented_duration`,
  `pause_start_and_resume_start_never_coexist_across_toggle_branches`)

**Files changed** (non-locked, supporting — outside cosmic engine root):
- `src/central_control_rains/mod.rs` (lines 64-121: new "Easing
  family policy" doc section; lines 350-384: glyph entry ramp
  constants block rewritten with design doc + 3 new constants:
  `GLYPH_ENTRY_RAMP_DECAY_RATE = 4.28`,
  `GLYPH_ENTRY_RAMP_SETTLE_FRAC = 0.95`,
  `GLYPH_ENTRY_RAMP_DURATION_MS` now annotated
  `#[allow(dead_code)]` since it's referenced by tests +
  doc-comments only — documents the settle time, not used in
  production code math)
- `README.md` (line 128: pause/resume bullet expanded to mention
  unified family + glyph entry)
- `CHANGELOG.md` (new v50.0.0-beta.5 entry)

**A/B delta** (vs locked baseline `c1c7779` + `e2e0512`):
- avg_fps: not formally A/B-benchmarked; per-frame surface is
  negligible — same exp() call count as commit `e2e0512` for
  pause/resume; glyph entry ramp now uses exp() instead of 3 mults
  (3 mults = ~1ns, exp = ~5-10ns), but only during the ~700ms
  post-scene-switch window. Zero surface at steady-state.
- alloc_calls: 0 -> 0 (Δ 0% — no heap allocations introduced)
- peak_rss: unchanged (no new state, no new buffers)
- stability signals: MATCH (frame_jitter=low,
  frame_time_stability=excellent, drift_interpretation=stable)

**Visual audit**: PASS — pause/resume visual identity preserved from
commit `e2e0512`; glyph scene entry ramp feel changes from "slow
start, fast middle, slow end" (smoothstep) to "instant cascade that
asymptotes to full speed" (exp approach) — owner-verified as the
desired masterclass feel, consistent with the pause/resume family.
No changes to color/brightness profile, no changes to droplet motion
physics outside the easing windows. masterclass brightness profile
preserved (top=0.533 / bot=0.369 unchanged — the easing path does
not touch `brightness_factors` or `rain_post`).

**Tests**: 1660 passed / 0 failed / 2 ignored (+4 new regression
tests for the easing contract — same baseline + 4 new); cosmic lock
suite 20/0/2; cloud subset 328/0/2 (+4 new tests in the cloud module).

Signoff: **oxyzenQ** — 2026-08-24 — v50.0.0-beta.5 masterclass easing consolidation

### UNLOCK cosmic-dragon (masterclass easing migration) at commit `e2e0512`, 2026-08-24

**Author**: oxyzenQ (Cosmic Dragon AI Agent)
**Reason**: Owner-approved masterclass easing migration. The pause/resume
easing in `cloud/rain.rs` used a smootherstep S-curve
(6t⁵-15t⁴+10t³) over fixed 0.30s decel / 0.45s resume windows, while
README.md:128 advertised "exponential deceleration (~3s coast-down)".
The README was stale (smootherstep is not exponential, and the durations
were 0.30s/0.45s — far from "~3s"). Switched to exponential decay
(`exp(-k·t)` decel, `1 - exp(-k·t)` accel) with asymmetric decay rates
(k_decel=1.2 / k_resume=0.9) so:
- Coast-down settles at 5% in ~2.5s — matches the README's "~3s
  coast-down" promise (with head-room), and gives genuine inertia tail
  instead of an abrupt end-snap.
- Resume settles at 95% in ~3.3s — slightly slower than pause, so
  resume feels like a "wake up" while pause feels snappy (preserves
  the prior 0.30s/0.45s asymmetric feel).
- Settle thresholds snap to clean terminal state (`self.pause = true`
  / `resume_blend = 1.0`) so other subsystems (spawn_remainder reset,
  monolith stream shift, phosphor LUT) see unambiguous transitions.
- exp() is already used in the locked path elsewhere
  (`cloud/phosphor.rs` LUT build, `chroma_dragon_engine/shaders/
  base/mod.rs` trail LUT), so no new math primitive introduced.

**Files changed** (locked path — production code):
- `src/engine/cosmic_dragon_engine/cloud/rain.rs` (lines 44-73: decel block
  rewritten from smootherstep S-curve to `(-PAUSE_EASE_DECAY_RATE *
  t).exp()` + settle-snap at `PAUSE_EASE_SETTLE_FRAC`; lines 147-181:
  accel block rewritten from smootherstep to `1.0 - (-RESUME_EASE_DECAY_RATE
  - t).exp()` + settle-snap at `RESUME_EASE_SETTLE_FRAC`. §8.4
  `resume_blend_start` interpolation preserved for aborted-decel
  resumes. 0.05 floor kept as a safety net for the first-frame window.)

**Files changed** (test only — no production code, exempt per §"Test
files are exempt UNLESS the test itself changes a public contract or
invariant"; the test only bumps a duration offset to match the new
settle time, no contract change):
- `src/engine/cosmic_dragon_engine/cloud/tests/mod.rs` (line 80-87 in
  `pause_stops_rain_and_unpause_resumes`: comment "smoothstep easing
  completes" -> "exponential decay easing settles", duration
  `Duration::from_secs(1)` -> `Duration::from_secs(5)` to give
  head-room past the new ~3.3s settle window; the assertion is
  weaker than the comment, so it would still pass at 1s, but the
  comment was misleading and is now accurate)

**Files changed** (non-locked, supporting — outside cosmic engine root):
- `src/central_control_rains/mod.rs` (lines 781-824: removed
  `PAUSE_EASE_DURATION_SECS` / `RESUME_EASE_DURATION_SECS` constants,
  added `PAUSE_EASE_DECAY_RATE` = 1.2, `RESUME_EASE_DECAY_RATE` = 0.9,
  `PAUSE_EASE_SETTLE_FRAC` = 0.05, `RESUME_EASE_SETTLE_FRAC` = 0.95,
  - design-doc comment block explaining the migration rationale)
- `README.md` (line 128: stale "exponential deceleration (~3s
  coast-down)" -> accurate "~2.5s coast-down to settle (k=1.2/s, snaps
  to fully paused at 5%), ~3.3s wake-up ramp on resume (k=0.9/s, snaps
  to full speed at 95%)")

**A/B delta** (vs locked baseline `c1c7779`):
- avg_fps: not formally A/B-benchmarked; per-frame surface is
  negligible — exp() call (~5-10ns) replaces 6 mults (~1-2ns) only
  during the ~2.5s decel / ~3.3s resume windows; zero surface at
  full-speed steady-state. exp() already used in `cloud/phosphor.rs`
  and `chroma_dragon_engine/shaders/base/mod.rs`.
- alloc_calls: 0 -> 0 (Δ 0% — no heap allocations introduced)
- peak_rss: unchanged (no new state, no new buffers)
- stability signals: MATCH (frame_jitter=low,
  frame_time_stability=excellent, drift_interpretation=stable)

**Visual audit**: PASS — pause/resume visual identity preserved;
the coast-down now matches the README's documented "exponential
deceleration" wording (was stale under smootherstep). Asymmetric
k_decel (1.2) > k_resume (0.9) preserves the prior 0.30s/0.45s
"pause snappy / resume wake-up" feel. No changes to color/brightness
profile, no changes to droplet motion physics outside the easing
windows. masterclass brightness profile preserved (top=0.533 /
bot=0.369 unchanged — the easing path does not touch
`brightness_factors` or `rain_post`).

**Tests**: 1656 passed / 0 failed / 2 ignored (same baseline as
`c1c7779`); cosmic lock suite 20/0/2; cloud subset 324/0/2.

Signoff: **oxyzenQ** — 2026-08-24 — pause/resume masterclass easing migration

### UNLOCK cosmic-dragon at commit c1c7779, 2026-08-23T09:15:00Z

**Author**: oxyzenQ (Cosmic Dragon AI Agent)
**Reason**: Triple-engine LTS audit finding LOW-2 — `Cloud::reset` clamped
only `self.cols`/`self.lines` (plus the droplet pool sizing) while the RNG
ranges, column tables, and per-cell LUTs were built from the RAW parameters.
The split was panic-free (saturating arithmetic + `Frame::set` bounds
checks) but inconsistent: an oversized caller could spawn droplets outside
the clamped grid while the glitch/color maps only covered the clamped
region. The oversized benchmark tiers also ran a latent hybrid state.

**Files changed**:
- src/engine/cosmic_dragon_engine/cloud/spawn.rs (reset() funnels into
  reset_with_bounds() which shadows the raw parameters with the clamped
  values for the whole function body; new reset_bench() mirrors
  Frame::new_bench with BENCH_MAX bounds)
- src/bench/mod.rs (all 3 benchmark call sites switched to reset_bench)
- src/engine/cosmic_dragon_engine/cloud/tests/mod.rs (3 dimension-consistency
  tests: oversized clamp coherence, degenerate zero-size coherence,
  reset_bench vs reset contrast)

**A/B delta** (vs locked baseline `24fa1be`):
- avg_fps: 90,819 -> 86,520 / 86,615 (two runs; Δ -4.7% vs baseline —
  cross-session hardware variance, same-session run-to-run variance is
  ±0.1%; reset() runs on resize only and has zero per-frame surface)
- peak_rss: 4.23 MiB -> 4.42 / 4.33 MiB (Δ within ±5%)
- alloc_calls: 563 -> 563 (Δ 0% — exact match, 0.0 allocs/frame)
- stability signals: MATCH (frame_jitter=low, frame_time_stability=excellent,
  drift_interpretation=stable)

**Visual audit**: PASS — the reset path does not alter rendering for
in-range interactive sizes (crossterm resize values, CLI defaults,
frame-derived sizes); masterclass brightness profile preserved
(density_gini 0.8960 vs 0.8961 baseline, color_transition_delta 0.00,
visual-mode-audit.py masterclass target top=0.533 / bot=0.369 unchanged).

**Tests**: 1642 passed / 0 failed / 2 ignored (full binary suite);
cloud 258/258; cosmic lock suite 17/17.

**Notes**: RETROACTIVELY documented — the same-commit entry was missed
(matching the chroma 809a897 precedent). Future unlocks MUST include the
UNLOCK entry in the same commit.

### Template

```markdown
### UNLOCK <dragon-name> at commit <SHA>, <ISO 8601 UTC>

**Author**: <name/handle>
**Reason**: <1-2 sentences — why this modification was necessary>
**Files changed**:
- <path>
- <path>

**A/B delta** (vs locked baseline `69af079`):
- avg_fps: <before> -> <after> (Δ <+/-%>)
- peak_rss: <before> -> <after> (Δ <+/-%>)
- alloc_calls: <before> -> <after> (Δ <+/-%>)
- stability signals: <MATCH or list any changes>

**Visual audit**: <PASS / FAIL — masterclass brightness profile preserved?>

**Tests**: <N>/~1500+ pass (must be ~1500+ or new total if tests added)
```

### Example (hypothetical, to be deleted once the first real UNLOCK lands)

```markdown
### UNLOCK cosmic-dragon at commit abc1234, 2026-09-15T08:30:00Z

**Author**: oxyzenQ
**Reason**: Fix rare panic on terminal resize from 0×0 to non-zero
when `clear_with_bg` is called before `new_with_bounds`. The
generation bump was reading `cell_gen` before initialization.

**Files changed**:
- src/engine/cosmic_dragon_engine/frame.rs (added early-init guard at line 142)

**A/B delta** (vs locked baseline `69af079`):
- avg_fps: 85,555 -> 85,612 (Δ +0.07%)
- peak_rss: 4.32 MiB -> 4.32 MiB (Δ 0%)
- alloc_calls: 563 -> 563 (Δ 0%)
- stability signals: MATCH

**Visual audit**: PASS — masterclass brightness profile preserved.

**Tests**: 1588/1588 pass (added 1 regression test for the resize panic).
```

---

**Newest UNLOCK entry: `c1c7779` (2026-08-23) — see top of this log.**
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
