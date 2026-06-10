<!-- SPDX-License-Identifier: MIT -->

# Cosmostrix Roadmap

## Release History

### v4.0.1 — Stable Patch

Current release. Production-grade cinematic Matrix rain renderer. Includes
Monolith Rain visual identity, warm-start scene transitions, phosphor decay
system, and sparse density enforcement. All visual behavior is locked down
by the Depth Regression Lab.

### v4.1.x — Hygiene (if needed)

Reserved for point fixes only: documentation corrections, CI adjustments,
or supply-chain updates. No feature work.

---

## v4.5.0 — Zactrix Foundation + Depth Regression

Architecture and regression foundation only. No visual redesign, no runtime
behavior change, no active parallel compute.

| Phase | Description |
|-------|-------------|
| Phase 1 | Zactrix Engine Architecture Split — modular directory structure under `src/zactrix_engine/` |
| Phase 2 | Docs Guard Split + ZACTRIX SYSTEM Diagnostics — split 993 LOC docs tests, add system diagnostics |
| Phase 3 | Depth Regression Lab — 15 categories of deterministic visual identity regression tests |
| Phase 4 | Monolith Test Pressure Relief — split 999 LOC monolith tests into focused modules |
| Phase 5 | Scene Test Pressure Relief — split 959 LOC scene tests into focused modules |
| Phase 6 | Closure Prep — roadmap docs, benchmark baseline, foundation closure note |

v4.5 is complete. No active parallel compute was added. The terminal
writer remains single-owner. ZACTRIX SYSTEM diagnostics report honest
policy values. All 679 tests pass. Visual behavior is identical to v4.0.1.

---

## Future Releases

### v4.6.0 — Controlled Atmosphere Expansion (opt-in only)

Gradual regime transitions for the Atmosphere Engine. All atmosphere features
remain **opt-in only** — the default calm regime is never overridden without
explicit user configuration. No forced visual changes.

### v4.7.0 — Renderer Ergonomics + Profile Ecosystem

Improved profile configuration, preset management, and renderer tuning
options. Focus on user-facing ergonomics without touching the core render
pipeline.

### v4.8.0 — Zactrix Render Efficiency Finishing

May introduce controlled parallel compute for **non-terminal buffer
preparation** only, gated by the runtime planner. Terminal writes remain
single-owner. Must pass the full Depth Regression Lab before merge.

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
