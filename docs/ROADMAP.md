<!-- SPDX-License-Identifier: MIT -->

# Cosmostrix Roadmap

## Release History

### v4.8.0 — Zactrix Integration + Terminal Cleanup Hardening (COMPLETE)

Integrated accepted zactrix color pipeline optimization with signal-exit
terminal cleanup hardening. No default visual behavior change and no active
parallel compute.

| Phase | Description | Commit |
|-------|-------------|--------|
| Phase 1 | Zactrix Lab Integration Audit | `9c39b3f` |
| Phase 2A | Color Pipeline Code Integration | `ce8dc81` |
| Phase 2B | Validation Lock | `fa2c995` |
| Phase 3 | Main Merge Prep / Conflict Audit | `8e9f3f8` |
| Phase 4 | Terminal Mode Sanity Hardening | `c56b4d7` |
| Phase 4B | Signal-Exit Visible Residue Cleanup | `df671fb` |
| Phase 5 | Release Prep Metadata + RC Gate | `ec1214b` |
| Phase 5B | Release Benchmark Report | `08dc6f5` |

v4.8 is complete. Terminal writer remains single-owner.
`compute_parallelism` remains `disabled`. `actual_execution` remains
`single-threaded-renderer`. 891 tests pass. 50k was not reached and is
not a release promise. No fake benchmark progress.

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

### v4.9.0 — The Wolf: Release Guard + Terminal Runtime Contract

Hardens the release process so benchmark reports cannot be forgotten again.
Adds terminal lifecycle documentation and release gate guardrails.

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 1 | Release guard foundation | current |
| Phase 2 | Benchmark report automation | pending |
| Phase 3 | Terminal lifecycle matrix | pending |
| Phase 4 | Doctor/report polish | pending |
| Phase 5 | Final release prep | pending |

v4.9 is not a 50k FPS promise. v4.9 does not claim parallel
renderer execution. Renderer honesty invariants are preserved:

- `actual_execution: single-threaded-renderer`
- `terminal_writer: single-owner`
- `compute_parallelism: disabled`

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
