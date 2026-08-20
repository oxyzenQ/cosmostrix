# Changelog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

Cosmostrix uses [SemVer](https://semver.org/). Git tags use a leading `v` (e.g. `v50.0.0`).

---

## v50.0.0-alpha.6 — Crystal Dragon Engine + Legacy Purge

- **Crystal Dragon Engine**: ambient palette drift via system state (CPU/CLOCK) → point (1-99) → temperature group (Cold/Medium/Hot) → probabilistic weighted theme selection. 60s polling, 12% drift chance, 60s dwell hysteresis, 300ms OKLab transitions via Chroma Dragon.
- **Total removal** of old auto-color-drift engine (`control_color_drift.rs`, `system_feeling.rs`, all references). No legacy, no duplicate.
- `--crystal-dragon` CLI flag promoted to first-class (visible in `--help`).
- 44 builtin themes partitioned: 14 Cold + 14 Medium + 14 Hot + 2 Reserved.
- CPU sensor with EMA smoothing (alpha 0.25), CLOCK fallback (UTC hour → point).

## v50.0.0-alpha.5 — Mouse-Click Effects + Chroma Dragon Sync

- Mouse-click effects: spawn ripple bursts on click, configurable via `--mouse-effects`.
- Chroma Dragon: OKLab 300ms wave transition sync on all palette-change paths.
- Live config reload: smooth OKLab transition on `config.toml` edit while running.

## v50.0.0-alpha.4 — HUD Expansion + `h` Shortkey Purge + Metric Stability

- HUD: scene name, charset, color scheme, uptime, pressure, endurance score.
- Purged `h` shortkey (redundant with `i` toggle). HUD metric stability fixes.

## v50.0.0-alpha.1 — Cosmic Dragon Stability + Rain-Screen Cleanliness + IP Tightening

- Cosmic Dragon: stability fixes, rain-screen cleanliness audit.
- IP tightening: reduce attack surface on network-exposed paths.

## v25.0.0-alpha.7 — Full-Codebase Dead-Code Sweep

- Flat `src/` files dead-code sweep, removed unused functions/imports.

## v25.0.0-alpha.6 — Interactive Subsystem Dead-Code Audit

- Interactive subsystem dead-code removal, test coverage hardening.

## v25.0.0-alpha.5 — Config Subsystem Dead-Code Audit

- Config subsystem dead-code sweep, removed stale flags/fields.

## v25.0.0-alpha.4 — Cloud Dead-Code Audit

- Cloud struct dead-code removal, field pruning.

## v25.0.0-alpha.3 — Legacy `--fullwidth` Purge

- Removed `--fullwidth` parameter (superseded by auto-detection).

## v25.0.0-alpha.2 — Cross-Scene Performance Audit

- Monolith-style optimizations, per-scene benchmark baselines.

## v20.1.0 — Legacy / Backward-Compat Purge

- Removed deprecated CLI flags, backward-compat shims.

## v20.0.0 — Temporal-Prediction Milestone

- Cosmic Dragon: phase predictor (P1), adaptive resync (P2), reclaim state (P4).

## v15.0.0 — Cosmic Dragon Pre-Release Polish

- Cosmic Dragon: cinematic behaviors, atmospheric evolution, self-healer.

## v14.0.0 — Scene-Custom Migration (Breaking CLI)

- Breaking: `--scene-custom` → TOML config, new CLI structure.

## v13.6.0 — CLI Simplification + Background Cleanup

- CLI flag simplification, background mode cleanup.

## v13.4.0 — Screen Size + Duration

- `--size` and `--duration` flags.

## v13.3.1 — Cosmic Dragon Performance

- 18 Dragon Eggs, P1/P2/P3 adaptive layers.

## v13.3.0 — Encoding Instrumentation

- SGR cache hit-rate, ANSI bytes/frame.

## v13.2.0 — Render Engine Specification

- Diff engine spec, competitor benchmark.

## v13.1.2 — HUD Toggle-Off Residue Fix

## v13.1.1 — Android HUD Toggle Fix

## v13.1.0 — Shell Completions + Verbose + Help Polish

## v13.0.0 — Alive Rain + Depth-of-Field + Security

## v4.0.0 — Initial v4 Release

- Atmosphere whisper engine, Cosmic Dragon architecture.

## v3.9.0 — v4 Ground-Work
