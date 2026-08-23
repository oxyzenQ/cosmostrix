<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Crystal Dragon Engine — Modification Rules (UNLOCK Protocol)

> **Simplified lock/unlock signature log**: see [`KEY.md`](KEY.md).
> This file holds the full UNLOCK protocol and detailed log entries.

> **Locked** at commit `69af079` on 2026-08-19T14:40:05Z by
> **rezky_nightky** — vision & director project cosmostrix.

## Purpose

This document defines the mandatory protocol for modifying any file in
`src/crystal_dragon_engine/` after the LTS lock. The lock ensures
long-term stability: any modification must be **documented**, **justified**,
and **acknowledged** before it lands on `main`.

## When to Follow This Protocol

This protocol applies if you modify any production `.rs` file under:

- `src/crystal_dragon_engine/ambient/`
- `src/crystal_dragon_engine/ambient_scheduler/`
- `src/crystal_dragon_engine/sensor/`
- `src/crystal_dragon_engine/palette_groups/`
- `src/crystal_dragon_engine/point_system/`
- `src/crystal_dragon_engine/crystal_dragon_control/`
- `src/crystal_dragon_engine/transition/`
- `src/crystal_dragon_engine/ambient_diag.rs`
- `src/crystal_dragon_engine/mod.rs`

Test files (`tests.rs`) are exempt UNLESS the test itself changes a
public contract or invariant.

## Pre-Modification Checklist

Before opening a PR that touches any locked file, you MUST:

1. **Run the gatekeeper**: `./scripts/build.sh check-all` (or, if
   that's unavailable in the dev env: `cargo fmt + cargo clippy --tests
   - cargo test --quiet`). All must pass before AND after your change.

2. **Run crystal-specific tests**:

   ```bash
   cargo test --quiet crystal_dragon
   ```

   All crystal tests must pass before AND after your change.

3. **Run an A/B benchmark** (Crystal Dragon's contribution to per-frame
   cost is zero, but ambient scheduler behavior must not regress):

   ```bash
   cargo build --release --quiet
   ./target/release/cosmostrix --benchmark --bench-io --bench-duration 10s > /tmp/before.txt
   # apply your change
   cargo build --release --quiet
   ./target/release/cosmostrix --benchmark --bench-io --bench-duration 10s > /tmp/after.txt
   python3 scripts/ab_compare.py  # if available, or compare manually
   ```

4. **Verify no regression**: avg_fps, peak_rss, and alloc_calls must
   stay within ±5% of the locked baseline. If your change affects the
   ambient scheduler, verify zero CPU usage between phase boundaries
   (use `top` or `pidstat -p <pid> 1` during a 5-minute idle window).

5. **Verify ambient scheduler behavior** (if scheduler touched):
   - Test with empty schedule (thread should idle 60s loops)
   - Test with single entry (thread should sleep until boundary)
   - Test live reload (condvar should wake thread immediately)
   - Test DST edge cases if time-of-day logic touched

6. **Update this README's UNLOCK section** (below) with:
   - Your commit SHA (after merge, or use `pending` if pre-merge)
   - Date-time (ISO 8601 UTC)
   - Reason for modification (1-2 sentences, why not what)
   - Files changed (paths)
   - A/B delta summary (FPS / RSS / alloc_calls)
   - Scheduler behavior verification (if scheduler touched)
   - Your name/handle

## Acceptable Reasons for UNLOCK

The lock is intentionally hard to break. Acceptable reasons include:

- **Bug fix** — a correctness issue (e.g., schedule entry not firing,
  CPU sample reading wrong value, snapback firing when disabled). Must
  include a regression test that fails before the fix.
- **Security fix** — a vulnerability (e.g., panic on malformed
  `ambient.HH-MM` config value).
- **Performance improvement** — measurable reduction in CPU usage
  between phase boundaries, OR faster startup apply. Must include
  before/after `pidstat` measurements.
- **New temperature group** — adding a 4th group (e.g., "Extreme"
  for points 100+). Requires updating `palette_groups/mod.rs`,
  `sensor/mod.rs` (point range), and `point_system/mod.rs` (selection).
- **calc-v2 implementation** — the reserved pattern state machine with
  memory. Owner approval required BEFORE starting implementation; this
  is a major algorithmic change.
- **Schedule format extension** — adding new fields to `AmbientEntry`
  (e.g., duration, fade-in). Requires migration path for existing
  configs.

**NOT acceptable** as sole reason:

- Polling interval tweaks (use `CrystalDragonControl` struct fields
  instead — these are already owner-tunable constants)
- EMA alpha tweaks (same as above)
- "Modernization" without measurable benefit
- Removing the silent-elegant mode (Option A is the owner-locked
  choice; verbose logging was explicitly rejected)

## UNLOCK Log

This section is appended every time a locked file is modified after
the lock commit. Newest entries go at the TOP.

### UNLOCK crystal-dragon at commit 9de2f44, 2026-08-23T09:10:00Z

**Author**: oxyzenQ (Cosmic Dragon AI Agent)
**Reason**: Triple-engine LTS audit finding LOW-1 — the scheduler loop
terminated its thread on ANY `try_send` error, conflating a transient full
channel (`TrySendError::Full`) with a dead receiver (`Disconnected`). A
saturated channel would silently disable ambient scheduling for the rest of
the session while the rain kept running.

**Files changed**:
- src/crystal_dragon_engine/ambient_scheduler/mod.rs (deliver() helper with
  three-way DeliverOutcome contract — Delivered / Saturated / ReceiverGone;
  bounded sleeping retry loop of 20 ms steps capped at 1 s, manual because
  SyncSender::send_timeout is unstable in std; both send sites updated; the
  day-boundary refire defers its day-seen marking when saturated)
- src/crystal_dragon_engine/ambient_scheduler/tests.rs (4 new contract tests:
  delivered, receiver-gone, saturated-with-bound-elapsed, recovery-within-wait)

**A/B delta** (vs locked baseline `24fa1be`):
- avg_fps: 90,819 → 86,520 / 86,615 (two runs; Δ -4.7% vs baseline —
  cross-session hardware variance, same-session run-to-run variance is
  ±0.1%; the scheduler thread has zero per-frame surface by design)
- peak_rss: 4.23 MiB → 4.42 / 4.33 MiB (Δ within ±5%)
- alloc_calls: 563 → 563 (Δ 0% — exact match, 0.0 allocs/frame)
- stability signals: MATCH (frame_jitter=low, frame_time_stability=excellent,
  drift_interpretation=stable)

**Scheduler behavior** (scheduler touched):
- Empty schedule: PASS (spawn_with_empty_schedule_does_not_fire)
- Single entry: PASS (spawn_fires_current_phase_on_startup, day-boundary suite)
- Live reload: PASS (reload_fires_new_current_phase_if_different, AB-09 refire suite)
- Saturated channel: PASS (new — deliver_reports_saturated_when_channel_stays_full
  pins the fixed-bound wait; pre-fix behavior terminated the thread)
- CPU between phases: unchanged — the thread still parks in
  Condvar::wait_timeout between boundaries (delivery retry only engages when
  the channel is full, which requires a wedged event loop)

**Tests**: 1642 passed / 0 failed / 2 ignored (full binary suite);
crystal 82/82; ambient_scheduler 17/17.

**Notes**: RETROACTIVELY documented — the same-commit entry was missed
(matching the chroma 809a897 precedent). Future unlocks MUST include the
UNLOCK entry in the same commit.

### Template

```markdown
### UNLOCK crystal-dragon at commit <SHA>, <ISO 8601 UTC>

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

**Scheduler behavior** (if scheduler touched):
- Empty schedule: <PASS/FAIL — thread idles 60s loops>
- Single entry: <PASS/FAIL — thread sleeps until boundary>
- Live reload: <PASS/FAIL — condvar wakes thread immediately>
- CPU between phases: <0% confirmed via pidstat>

**Tests**: <N>/~1500+ pass (must be ~1500+ or new total if tests added)
```

### Example (hypothetical, to be deleted once the first real UNLOCK lands)

```markdown
### UNLOCK crystal-dragon at commit ghi9012, 2026-11-05T19:20:00Z

**Author**: oxyzenQ
**Reason**: Fix ambient scheduler not firing entry at 02:30 local time
during DST fall-back (entry was firing once at 01:30 instead of twice
at 01:30 and 02:30). The `seconds_to_next_phase` calculation didn't
account for the repeated hour.

**Files changed**:
- src/crystal_dragon_engine/ambient/mod.rs (fixed `seconds_to_next_phase` DST handling)
- src/crystal_dragon_engine/ambient/tests.rs (added regression test for DST fall-back)

**A/B delta** (vs locked baseline `69af079`):
- avg_fps: 85,555 → 85,558 (Δ +0.003%)
- peak_rss: 4.32 MiB → 4.32 MiB (Δ 0%)
- alloc_calls: 563 → 563 (Δ 0%)
- stability signals: MATCH

**Scheduler behavior**:
- Empty schedule: PASS
- Single entry: PASS
- Live reload: PASS
- CPU between phases: 0% confirmed via pidstat

**Tests**: 1589/1589 pass (added 2 DST regression tests).
```

---

**Newest UNLOCK entry: `9de2f44` (2026-08-23) — see top of this log.**
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
