<!-- SPDX-License-Identifier: MIT -->

# Cosmostrix Roadmap

## Release History

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

### v4.7.0 — Renderer Ergonomics + Profile Ecosystem [ACTIVE]

Improved profile configuration, preset management, and renderer tuning
options. Focus on user-facing ergonomics without touching the core render
pipeline.

Phase 1 (complete): Profile Ecosystem Audit + Contract.
- `docs/PROFILE_ECOSYSTEM.md` — profile syntax, precedence chain, behavior
  matrix, color stickiness rules, storm unavailability, single-owner
  invariant, zactrix-20k-lab parked for v4.8.
- Profile behavior matrix: 14 documented scenarios covering no profile,
  known/unknown profiles, CLI > profile > config precedence, color
  stickiness, atmosphere opt-in, storm rejection, terminal writer
  single-owner, compute parallelism disabled.
- `--list-profiles` output adds pointer to `docs/PROFILE_ECOSYSTEM.md`.
- Deterministic doc guard tests in new `docs_tests/profile.rs`.
- No runtime or visual behavior change.
- No version bump.

Phase 2 (current): Profile Examples + Config Dump Polish.
- `docs/PROFILE_EXAMPLES.md` — 9 concise profile examples (minimal,
  color-only, scene foundation, 4 atmosphere presets, CLI override,
  config precedence). All examples use real supported profile syntax.
- Config dump adds pointer to `docs/PROFILE_EXAMPLES.md`.
- `--list-profiles` adds pointer to `docs/PROFILE_EXAMPLES.md`.
- 14 deterministic doc guard tests for examples doc, config dump, and
  list-profiles pointers.
- No runtime or visual behavior change.
- No version bump.

### v4.8.0 — Zactrix Render Efficiency Finishing

Review and merge `zactrix-20k-lab` performance experiments. May introduce
controlled parallel compute for **non-terminal buffer preparation** only,
gated by the runtime planner. Terminal writes remain single-owner. Must
pass the full Depth Regression Lab before merge.

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