<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmic Dragon Engine — Modification Rules (UNLOCK Protocol)

> **Simplified lock/unlock signature log**: see [`KEY.md`](KEY.md).
> This file holds the full UNLOCK protocol and detailed log entries.

> **Locked** at commit `69af079` on 2026-08-19T14:40:05Z by
> **rezky_nightky** — vision & director project cosmostrix.

## Purpose

This document defines the mandatory protocol for modifying any file in
`src/cosmic_dragon_engine/` after the LTS lock. The lock ensures
long-term stability: any modification must be **documented**, **justified**,
and **acknowledged** before it lands on `main`.

## When to Follow This Protocol

This protocol applies if you modify any production `.rs` file under:

- `src/cosmic_dragon_engine/cloud/`
- `src/cosmic_dragon_engine/frame.rs`
- `src/cosmic_dragon_engine/terminal/`
- `src/cosmic_dragon_engine/runtime.rs`
- `src/cosmic_dragon_engine/mod.rs`

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
- src/cosmic_dragon_engine/cloud/spawn.rs (reset() funnels into
  reset_with_bounds() which shadows the raw parameters with the clamped
  values for the whole function body; new reset_bench() mirrors
  Frame::new_bench with BENCH_MAX bounds)
- src/bench/mod.rs (all 3 benchmark call sites switched to reset_bench)
- src/cosmic_dragon_engine/cloud/tests/mod.rs (3 dimension-consistency
  tests: oversized clamp coherence, degenerate zero-size coherence,
  reset_bench vs reset contrast)

**A/B delta** (vs locked baseline `24fa1be`):
- avg_fps: 90,819 → 86,520 / 86,615 (two runs; Δ -4.7% vs baseline —
  cross-session hardware variance, same-session run-to-run variance is
  ±0.1%; reset() runs on resize only and has zero per-frame surface)
- peak_rss: 4.23 MiB → 4.42 / 4.33 MiB (Δ within ±5%)
- alloc_calls: 563 → 563 (Δ 0% — exact match, 0.0 allocs/frame)
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
- avg_fps: <before> → <after> (Δ <+/-%>)
- peak_rss: <before> → <after> (Δ <+/-%>)
- alloc_calls: <before> → <after> (Δ <+/-%>)
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
- src/cosmic_dragon_engine/frame.rs (added early-init guard at line 142)

**A/B delta** (vs locked baseline `69af079`):
- avg_fps: 85,555 → 85,612 (Δ +0.07%)
- peak_rss: 4.32 MiB → 4.32 MiB (Δ 0%)
- alloc_calls: 563 → 563 (Δ 0%)
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
