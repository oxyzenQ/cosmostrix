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

**No UNLOCK entries yet — engine is at locked state `69af079`.**
