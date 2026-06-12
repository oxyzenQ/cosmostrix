<!-- SPDX-License-Identifier: MIT -->

# Cosmostrix Roadmap

## Release History

### v4.7.0 — Renderer Ergonomics + Profile Ecosystem (COMPLETE)

Improved profile configuration, preset management, validation UX, and release
candidate smoke coverage without changing core render behavior.

| Phase | Description |
|-------|-------------|
| Phase 1 | Profile Ecosystem Audit + Contract |
| Phase 2 | Profile Examples + Config Dump Polish |
| Phase 3 | Profile Validation UX + Error Message Polish |
| Phase 4 | Profile RC Smoke + Closure Prep |

v4.7 is complete. Runtime and visual behavior remain stable. Terminal writer
remains single-owner. `compute_parallelism` remains `disabled`.

### v4.6.0 — Controlled Atmosphere Expansion (COMPLETE)

Controlled atmosphere expansion with contracts, docs, presets, and tests.
All atmosphere features remain **opt-in only**. No default visual output
change. Terminal writer remains single-owner. Storm unavailable.

| Phase | Description |
|-------|-------------|
| Phase 1 | Controlled Atmosphere Expansion Contract |
| Phase 2 | Controlled Atmosphere Profile Presets (6 presets) |
| Phase 3 | Preset UX / Config Examples + Pressure-aware Tests |
| Phase 4 | Preset CLI/Profile Discoverability (`--list-profiles`) |
| Phase 5 | Atmosphere RC Smoke + v4.6 Closure |

### v4.5.0 — Zactrix Foundation + Depth Regression (COMPLETE)

Architecture and regression foundation. Complete.

| Phase | Description |
|-------|-------------|
| Phase 1 | Zactrix Engine Architecture Split |
| Phase 2 | Docs Guard Split + ZACTRIX SYSTEM Diagnostics |
| Phase 3 | Depth Regression Lab |
| Phase 4 | Monolith Test Pressure Relief |
| Phase 5 | Scene Test Pressure Relief |
| Phase 6 | Closure Prep |

v4.5 is complete. No active parallel compute was added. The terminal
writer remains single-owner. ZACTRIX SYSTEM diagnostics report honest
policy values. Visual behavior is identical to v4.0.1.

### v4.0.1 — Stable Patch

Production-grade cinematic Matrix rain renderer. Includes
Monolith Rain visual identity, warm-start scene transitions, phosphor decay
system, and sparse density enforcement. All visual behavior is locked down
by the Depth Regression Lab.

---

## Active Development

### v4.8.0 — Zactrix Render Efficiency Integration [ACTIVE]

Phase 4B (current): Signal-Exit Visible Residue Cleanup. Fix fork-guard
stdout race and add viewport clear before alternate screen switch on
signal exit. v4.8 merge blocked until owner-side visual smoke confirms.

Phase 4 (complete): Terminal mode sanity hardening. Signal handler
race fixed. Commit `c56b4d7`.

Phase 3 (complete): Main Merge Prep / Conflict Audit.

Phase 2B (complete): Validation Lock. Commit `fa2c995`. 875 tests pass.
5-run benchmark mean ~27,900 FPS. All invariants stable.

Phase 2A (complete): Color pipeline optimization integrated from `e7253e7`
(`zactrix-20k-lab`) via manual adaptation. No direct lab merge. Commit
`ce8dc81`.

Phase 1 (complete): Zactrix Lab Integration Audit.
- `docs/ZACTRIX_INTEGRATION_AUDIT.md` records the v4.7.0 main baseline,
  lab commits, accepted candidate source, rejected 50k attempts, and
  integration invariants.
- `zactrix-20k-lab` is the optimization candidate source. The accepted
  candidate is reducing redundant color pipeline work.
- `zactrix-50k-lab` is ceiling and boundary evidence. 50k was not reached,
  and rejected attempts stay rejected. 50k is not a release promise.
- No direct merge from lab branches. Future work must cherry-pick or adapt
  only clean changes.
- No fake benchmark progress. Benchmark counters and field names stay honest.
- Terminal writer remains single-owner. `compute_parallelism` remains
  `disabled`.

Any optimization that touches the renderer, cloud module, monolith module,
phosphor system, or droplet lifecycle must pass all depth regression tests.
If an optimization cannot pass these tests, it must be redesigned.

### v4.9.0 — Optional RC Freeze / Endurance / Release Prep

Endurance testing, resource monitoring validation, and release candidate
freeze. Stabilization only — no new features.

### v5.0.0 — Zactrix Engine Stable Default + Precision/Efficiency

Only when the runtime planner is real, proven, and stable. Requires a
completed v4.8.0 with confirmed stability across extended endurance runs.

---

## Invariants (all versions)

These invariants apply to every release. They must never regress:

- **Terminal writer remains single-owner.** No parallel terminal writes.
- **No unbounded thread pools.** Worker budgets are always bounded.
- **Visual identity must remain identical to v3.9.0.** Depth, brightness
  hierarchy, empty-space ratio, and residue bounds are locked.
- **Scene cycling semantics unchanged.** x/X cycle behavior is stable.
- **Color stability behavior unchanged.** Explicit choices remain sticky.
- **`auto_color_drift` remains default `false`.** Opt-in only.
- **`compute_parallelism` remains `disabled`** unless a future release
  explicitly activates it through the Zactrix runtime planner.
- **Benchmark field names unchanged.** No renaming of diagnostic labels.

## CPU Targets

- Calm/idle target: < 1-3% realistic CPU usage.
- Benchmark/stress can use dynamic high CPU.
- Paused should remain near 0%.
