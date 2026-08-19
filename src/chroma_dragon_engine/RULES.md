<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Chroma Dragon Engine — Modification Rules (UNLOCK Protocol)

> **Locked** at commit `69af079` on 2026-08-19T14:40:05Z by
> **rezky_nightky** — vision & director project cosmostrix.

## Purpose

This document defines the mandatory protocol for modifying any file in
`src/chroma_dragon_engine/` after the LTS lock. The lock ensures
long-term stability: any modification must be **documented**, **justified**,
and **acknowledged** before it lands on `main`.

## When to Follow This Protocol

This protocol applies if you modify any production `.rs` file under:

- `src/chroma_dragon_engine/palette/`
- `src/chroma_dragon_engine/shaders/`
- `src/chroma_dragon_engine/gradient/`
- `src/chroma_dragon_engine/post/`
- `src/chroma_dragon_engine/catalog.rs`
- `src/chroma_dragon_engine/legacy.rs`
- `src/chroma_dragon_engine/tuning.rs`
- `src/chroma_dragon_engine/color_cache.rs`
- `src/chroma_dragon_engine/mod.rs`

Test files (`tests.rs`, `tests/` subdirs) are exempt UNLESS the test
itself changes a public contract or invariant — in which case the
lock suite (`tests/lock.rs`) MUST be updated to assert the new contract.

## Pre-Modification Checklist

Before opening a PR that touches any locked file, you MUST:

1. **Run the gatekeeper**: `./scripts/build.sh check-all` (or, if
   that's unavailable in the dev env: `cargo fmt + cargo clippy --tests
   + cargo test --quiet`). All must pass before AND after your change.

2. **Run the lock suite explicitly**:
   ```bash
   cargo test --quiet chroma::tests::lock
   ```
   All 18 invariants must pass before AND after your change. If your
   change intentionally modifies a public contract, you MUST update the
   lock suite in the same commit.

3. **Run an A/B benchmark**:
   ```bash
   cargo build --release --quiet
   ./target/release/cosmostrix --benchmark --bench-io --bench-duration 10s > /tmp/before.txt
   # apply your change
   cargo build --release --quiet
   ./target/release/cosmostrix --benchmark --bench-io --bench-duration 10s > /tmp/after.txt
   python3 scripts/ab_compare.py  # if available, or compare manually
   ```

4. **Verify no regression**: avg_fps, peak_rss, and alloc_calls must
   stay within ±5% of the locked baseline. The visual identity metrics
   (`density_gini`, `color_transition_delta`, `frame_entropy_bits`)
   must stay within ±2% — color changes are HIGH-RISK because they
   directly affect what the user sees.

5. **Verify visual identity unchanged**: Run `python3 scripts/visual-mode-audit.py`
   and confirm the masterclass brightness profile is preserved
   (top=0.533, bot=0.369 within ±5%).

6. **Update this README's UNLOCK section** (below) with:
   - Your commit SHA (after merge, or use `pending` if pre-merge)
   - Date-time (ISO 8601 UTC)
   - Reason for modification (1-2 sentences, why not what)
   - Files changed (paths)
   - A/B delta summary (FPS / RSS / alloc_calls / visual identity metrics)
   - Your name/handle

## Acceptable Reasons for UNLOCK

The lock is intentionally hard to break. Acceptable reasons include:

- **Bug fix** — a correctness issue that produces wrong colors, panics
  on valid input, or violates a lock-suite invariant. Must include a
  regression test that fails before the fix.
- **Security fix** — a vulnerability (e.g., panic on malformed
  `[palette.<name>]` config). Must include a fuzzing run if input
  parsing is touched.
- **Performance improvement** — a measurable, repeatable gain (>5% on
  the A/B benchmark) with no visual regression. The improvement must
  be attributable to this engine's code, not external factors.
- **New theme addition** — adding a new `ColorScheme` variant + matching
  `ThemeDef` to `THEMES`. This is an additive change and does NOT
  require modifying existing code paths — only the registry.
- **New palette stop count** — increasing `MAX_STOPS` for a new
  chroma feature. Requires updating the lock suite to assert the new
  stop count invariant.

**NOT acceptable** as sole reason:

- Color tuning preferences (use `--color-tune` CLI flag instead)
- "Modernization" without measurable benefit
- Refactoring the shader pipeline (Phase 9-D is the locked final form)
- Removing the OKLab gradient path (Phase 9-C removed the sRGB-linear
  fallback; reverting would be a regression)

## UNLOCK Log

This section is appended every time a locked file is modified after
the lock commit. Newest entries go at the TOP.

### Template

```markdown
### UNLOCK chroma-dragon at commit <SHA>, <ISO 8601 UTC>

**Author**: <name/handle>
**Reason**: <1-2 sentences — why this modification was necessary>
**Files changed**:
- <path>
- <path>

**A/B delta** (vs locked baseline `69af079`):
- avg_fps: <before> → <after> (Δ <+/-%>)
- peak_rss: <before> → <after> (Δ <+/-%>)
- alloc_calls: <before> → <after> (Δ <+/-%>)
- density_gini: <before> → <after> (Δ <+/-%>)
- color_transition_delta: <before> → <after> (Δ <+/-%>)
- stability signals: <MATCH or list any changes>

**Lock suite**: <N>/18 invariants pass (must be 18/18 unless the
contract intentionally changed, in which case update the count)

**Visual audit**: <PASS / FAIL — masterclass brightness profile preserved?>

**Tests**: <N>/1587 pass (must be 1587/1587 or new total if tests added)
```

### Example (hypothetical, to be deleted once the first real UNLOCK lands)

```markdown
### UNLOCK chroma-dragon at commit def5678, 2026-10-20T11:15:00Z

**Author**: oxyzenQ
**Reason**: Add 45th builtin color scheme "Aurora Borealis" to support
the v51 "Northern Lights" theme pack. Additive change only — no existing
code path modified.

**Files changed**:
- src/runtime.rs (added `ColorScheme::AuroraBorealis` variant)
- src/chroma_dragon_engine/catalog.rs (added `ThemeDef` to `THEMES`)
- src/chroma_dragon_engine/tests/lock.rs (updated invariant: 44 → 45 themes)

**A/B delta** (vs locked baseline `69af079`):
- avg_fps: 85,555 → 85,549 (Δ -0.01%)
- peak_rss: 4.32 MiB → 4.32 MiB (Δ 0%)
- alloc_calls: 563 → 563 (Δ 0%)
- density_gini: 0.8961 → 0.8961 (Δ 0%)
- color_transition_delta: 0.00 → 0.00 (Δ 0%)
- stability signals: MATCH

**Lock suite**: 18/18 invariants pass (1 invariant updated: theme count
44 → 45)

**Visual audit**: PASS — masterclass brightness profile preserved.

**Tests**: 1590/1590 pass (added 3 tests for new theme).
```

---

### UNLOCK chroma-dragon at commit 809a897, 2026-08-19T16:36:02Z

**Author**: oxyzenQ
**Reason**: Deeper audit (task 1/6) found stale path references and
outdated theme counts across chroma_dragon_engine/ source files.
Specifically: `all_schemes()` test helper was missing `EnergyZen` (the
v50 masterclass theme) — INV-2 lock test asserted `schemes.len() == 43`
and passed, but silently skipped EnergyZen in every theme-sweep
invariant. This was a REAL BUG: a future regression in EnergyZen's
palette construction would NOT have been caught by the lock suite.
Also fixed 15+ stale path refs (src/cloud/ → src/cosmic_dragon_engine/cloud/,
chroma::legacy → chroma_dragon_engine::legacy, etc.) and outdated
"43 themes" → "44 themes" + "Phase 9-B" → "Phase 9-D" + "18 invariants"
→ "19 invariants" in doc comments.

**Files changed**:
- src/chroma_dragon_engine/mod.rs (1 stale path ref: src/cloud/ → src/cosmic_dragon_engine/cloud/)
- src/chroma_dragon_engine/palette/tests_floor.rs (6 "43 themes" → "44 themes" refs)
- src/chroma_dragon_engine/tests/lock.rs (10+ stale refs + EnergyZen added to all_schemes() + INV-2 assertion 43 → 44 + git log path ref updated)
- src/chroma_dragon_engine/tuning.rs (2 "43 themes" → "44 themes" refs)

**A/B delta** (vs locked baseline `69af079`):
- avg_fps: 85,555 → 84,457 (Δ -1.28% — within ±5% tolerance, hardware variance)
- peak_rss: 4.32 MiB → 4.32 MiB (Δ 0%)
- alloc_calls: 563 → 288 (Δ -48.8% — different bench duration 5s vs 10s, per-frame allocs unchanged at 0.0)
- density_gini: 0.8961 → 0.8958 (Δ -0.03%)
- color_transition_delta: 0.00 → 0.00 (Δ 0%)
- stability signals: MATCH (frame_jitter=low, frame_time_stability=excellent, drift=stable)

**Lock suite**: 19/19 invariants pass (INV-2 updated: theme count 43 → 44,
EnergyZen now included in all theme-sweep invariants)

**Visual audit**: PASS — masterclass brightness profile preserved
(visual-mode-audit.py: top=0.533, bot=0.369 unchanged)

**Tests**: 1594/1594 pass (was 1590 at lock; +4 tests from task 3
auto-color-drift hints, unrelated to this unlock)

**Notes**:
- This unlock was RETROACTIVELY documented (commit 809a897 landed before
  the UNLOCK entry was added). The lock protocol requires the UNLOCK
  entry to be added in the SAME commit as the modification; this was
  missed because the modification was part of a broader 6-task audit.
  Future unlocks MUST include the UNLOCK entry in the same commit.
- The EnergyZen bug was caught by this audit — without it, the lock
  suite was providing false confidence (INV-2 "passed" but didn't
  actually test all 44 themes).
