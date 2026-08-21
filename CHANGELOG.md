# Changelog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

Cosmostrix uses [SemVer](https://semver.org/). Git tags use a leading `v` (e.g. `v50.0.0`).

Pre-v13 history is archived in [`docs/archive/CHANGELOG_PRE_V13.md`](docs/archive/CHANGELOG_PRE_V13.md).

---

## v50.0.0-beta.2 — Three Dragon Engines Stabilization

Current release. 230 Rust source files, ~89K LOC, 860+ tests.

Cosmostrix v50 is the "zero to hero" culmination — from a simple terminal
rain demo to a professional-grade cinematic renderer with three independent
dragon engines, each owning a distinct concern.

### The Three Dragon Engines

- **Cosmic Dragon** (`src/cosmic_dragon_engine/`, `src/cosmic_dragon_incubator/`)
  — Simulation core. Droplet lifecycle, spawn physics, atmospheric evolution,
  cinematic behaviors, self-healer, phase predictor, and reclaim state.
  Never touches palette — reads colors produced by Chroma Dragon.
- **Chroma Dragon** (`src/chroma_dragon_engine/`)
  — Coloring engine. OKLab gradient palettes, per-cell shader pipeline,
  climate post-FX (luminance/saturation/hue drift), L-smoothing, and
  300ms top-to-bottom wave transitions on every color-change path.
- **Crystal Dragon** (`src/crystal_dragon_engine/`)
  — Ambient intelligence. CPU/CLOCK-driven palette drift (44 themes in
  Cold/Medium/Hot groups, probabilistic weighted selection, 60s polling,
  12% drift chance, 60s dwell hysteresis). Time-of-day ambient scheduler
  for automatic scene+palette switching via `config.toml`.

### Architecture & Quality

- Module-directory source layout (12 module dirs), extracted from flat `src/`.
- MSRV 1.97, Clippy `-D warnings` CI gate, Miri nightly validation.
- PGO (Profile-Guided Optimization) two-stage build via `./scripts/build.sh pgo`.
- Fat LTO, single codegen-unit release profile with platform-specific PGO profiles.
- Live config reload with SHA-512 fingerprinting and OKLab smooth transitions.
- Central Control Dragon Power: thermal sampling, endurance health, power management.
- Performance self-healer, phase predictor, adaptive resync.
- Terminal protocol detection (kitty, wezterm, alacritty, iTerm2, Windows Terminal, tmux).
- Synchronized output (`ESC[?2026h`) for tear-free frame delivery.
- Configurable via CLI flags, `config.toml`, and profiles (CLI > profile > config > defaults).
- `--doctor` diagnostics, `--benchmark` with JSON output, `--testconf` validation.
- Cross-platform: Linux, macOS, Windows, FreeBSD, Android.
- AUR package: `cosmostrix-bin`.

### Scenes & Themes

- 5 scenes: monolith (default), matrix, cinematic, hacker, digital-rain.
- 44+ builtin color themes with OKLab gradients and climate post-FX.
- Custom charset support via `--charset-file`.
- `--color-tune` for runtime saturation/brightness adjustment.

### Interactive

- Keybindings: `q` quit, `Space` pause, `c`/`C` cycle colors, `s`/`S` cycle scenes,
  `p` profile, `x` random scene, `i` info HUD, `[`/`]` speed, `Up`/`Down` density.
- Live HUD with FPS, RSS memory, pressure, endurance score.
- Mouse-click effects (opt-in). Bracketed-paste safe.

---

## v50.0.0-alpha.6 — Crystal Dragon Engine + Legacy Purge

- Crystal Dragon Engine: ambient palette drift via CPU/CLOCK → temperature groups.
- Total removal of old auto-color-drift engine. `--crystal-dragon` promoted to first-class.
- 44 builtin themes partitioned: 14 Cold + 14 Medium + 14 Hot + 2 Reserved.

## v50.0.0-alpha.5 — Mouse-Click Effects + Chroma Dragon Sync

- Mouse-click ripple effects (opt-in). OKLab 300ms wave transitions on all palette changes.
- Smooth OKLab transition on live config reload while running.

## v50.0.0-alpha.4 — HUD Expansion + Metric Stability

- HUD: scene name, charset, color scheme, uptime, pressure, endurance.
- Purged redundant `h` shortkey (superseded by `i` toggle).

## v50.0.0-alpha.1 — Cosmic Dragon Stability + Rain Cleanliness

- Cosmic Dragon stability fixes, rain-screen cleanliness audit, IP surface tightening.

## v25.0.0-alpha.7 … v25.0.0-alpha.2 — Dragon Hunt v2 Dead-Code Sweep

- Systematic dead-code removal across the full codebase in 5 phases:
  cloud, config, interactive subsystem, then full-codebase sweep.
- Legacy `--fullwidth` purge (superseded by auto-detection).
- Cross-scene performance baselines, monolith-style optimizations.

## v20.1.0 — Legacy / Backward-Compat Purge

- Removed deprecated CLI flags and backward-compatibility shims.

## v20.0.0 — Temporal-Prediction Milestone

- Cosmic Dragon phase predictor (P1), adaptive resync (P2), reclaim state (P4).

## v15.0.0 — Cosmic Dragon Pre-Release Polish

- Cosmic Dragon cinematic behaviors, atmospheric evolution, self-healer.

## v14.0.0 — Scene-Custom Migration (Breaking CLI)

- Breaking: `--scene-custom` migrated to TOML config. New CLI structure.

## v13.6.0 — CLI Simplification

- CLI flag simplification, background mode cleanup.

## v13.4.0 — Screen Size + Duration

- Added `--size` and `--duration` flags.

## v13.3.1 — Cosmic Dragon Performance

- 18 Dragon Eggs, P1/P2/P3 adaptive layers.

## v13.3.0 — Encoding Instrumentation

- SGR cache hit-rate tracking, ANSI bytes/frame metrics.

## v13.2.0 — Render Engine Specification

- Diff engine specification, competitor benchmark comparison.

## v13.1.0 — Shell Completions + Verbose + Help Polish

## v13.0.0 — Alive Rain + Depth-of-Field + Security

---

## v4.0.0 — Atmosphere Engine + Monolith Rain

- Signature Monolith Rain as the production default (sparse data pillars, segmented blocks).
- Cosmic Dragon Core/Engine/Cache groundwork for adaptive rendering.
- Atmosphere engine, terminal compatibility lab, doctor diagnostics.
- Profile ecosystem, config discoverability, benchmark hardening.
- Canonical metadata alignment across Cargo, README, AUR.

## v3.9.0 — v4 Ground-Work

- Atmosphere visual whisper engine, cosmic dragon architecture discipline.
- Phase 10.5: atmosphere config honesty + profile smoke hardening.
