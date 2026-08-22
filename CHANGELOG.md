# Changelog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

Cosmostrix uses [SemVer](https://semver.org/). Git tags use a leading `v` (e.g. `v50.0.0`).

Pre-v13 history is archived in [`docs/archive/CHANGELOG_PRE_V13.md`](docs/archive/CHANGELOG_PRE_V13.md). The summary below covers the full journey from the first public release to the current beta, condensed so users can follow the evolution without wading through per-release minutiae.

---

## v50.0.0-beta.4 — Three Dragon Engines (Current Beta)

Cosmostrix v50.0.0-beta.4 — production-LTS-grade stability after full audit pass. 226 source files, ~89K LOC, ~1500+ tests pass. All 3 dragon engines locked with A/B benchmark signature.

### What's new since beta.3

- **Live-reload masterclass** (Option D): message, message-border, msg-mode, intro-color now live-reload. CLI intent guards for power-dragon, async-mode, monolith-size, color-tune. color.tune reset-on-comment bug fixed.
- **New CLI flags**: `--intro-color`, `--power-dragon`, `--msg-mode`, `--crystal-dragon`, `--async-mode` (all `<true|false>` or `<name>` with value_parser — no silent-toggle).
- **`--uniform` removed** → replaced by `--async-mode false`. `--check-updated` alias removed → `--check-update` is canonical.
- **Verbose honesty**: "final runtime state" section now tracks ALL live-reload fields (12 total) — shows EFFECTIVE runtime values, not startup values.
- **Border gradient fix**: triangle wave eliminates sharp white→black gap on left border. All color output routes through Chroma Dragon (routing rule codified).
- **Disclaimer injector**: auto-injects "source code = truth" disclaimer to all `*.md` files. Wired into gate-keepers.sh.
- **Dynamic default message**: `"cosmostrix v<CARGO_PKG_VERSION>"` — version from Cargo.toml at compile time, never hardcoded.
- **Did-you-mean**: strengthened for all 5 new CLI flags + `--intro-color` hard error for unknown themes (was silent ignore).

---

## v50.0.0-beta.3 — Three Dragon Engines

Cosmostrix v50 is the "zero to hero" culmination — from a simple terminal rain demo to a professional-grade cinematic renderer with three independent dragon engines, each owning a distinct concern. 220+ source files, ~89K LOC, ~1500+ tests pass.

### The Three Dragon Engines

- **Cosmic Dragon** (`src/cosmic_dragon_engine/`) — Simulation core. Droplet lifecycle, spawn physics, atmospheric evolution, cinematic behaviors, self-healer, phase predictor, reclaim state. Never touches palette.
- **Chroma Dragon** (`src/chroma_dragon_engine/`) — Coloring engine. OKLab gradient palettes, per-cell shader pipeline, climate post-FX (luminance/saturation/hue drift), L-smoothing, 300ms top-to-bottom wave transitions on every color-change path.
- **Crystal Dragon** (`src/crystal_dragon_engine/`) — Ambient intelligence. CPU/CLOCK-driven palette drift (44 themes in Cold/Medium/Hot groups, probabilistic weighted selection, 60s polling, 12% drift chance, 60s dwell hysteresis). Time-of-day ambient scheduler for automatic scene+palette switching via `config.toml`.

### Highlights Since v13

- Module-directory source layout (12 module dirs), extracted from flat `src/`.
- MSRV 1.97, Clippy `-D warnings` CI gate, Miri nightly validation.
- PGO (Profile-Guided Optimization) two-stage build via `./scripts/build.sh pgo`.
- Fat LTO, single codegen-unit release profile with platform-specific PGO profiles.
- Live config reload with SHA-512 fingerprinting and OKLab smooth transitions.
- Central Control Dragon Power: thermal sampling, endurance health, power management.
- Terminal protocol detection (kitty, wezterm, alacritty, iTerm2, Windows Terminal, tmux).
- Synchronized output (`ESC`) for tear-free frame delivery.
- 18 scenes: monolith (default), matrix, signal, classic, cinematic, calm, storm, cosmos, neon, hacker, matrix_film, low-power, cosmic-dragon, carbonic, dragon-crystal, orange-cat, north-stars, curiosity.
- 44+ builtin color themes with OKLab gradients and climate post-FX.
- `--doctor` diagnostics, `--benchmark` with JSON output, `--testconf` validation.
- Cross-platform: Linux, macOS, Windows, FreeBSD, Android. AUR package: `cosmostrix-bin`.

### Interactive Controls

`q` quit · `Space` pause · `c`/`C` cycle colors · `s`/`S` cycle scenes · `x`/`X` random scene · `p` profile · `i` info HUD · `[`/`]` speed · `Up`/`Down` density

---

## v50.0.0-alpha.6 — Crystal Dragon Engine + Legacy Purge

- Introduced Crystal Dragon Engine: ambient palette drift via CPU/CLOCK → temperature groups.
- Removed old auto-color-drift engine entirely. `--crystal-dragon` promoted to first-class.

## v50.0.0-alpha.5 — Mouse-Click Effects + Chroma Dragon Sync

- Mouse-click ripple effects (opt-in).
- OKLab 300ms wave transitions on all palette changes, including live config reload.

## v50.0.0-alpha.4 — HUD Expansion

- HUD now shows scene name, charset, color scheme, uptime, pressure, endurance score.
- Purged redundant `h` shortkey (superseded by `i` toggle).

## v50.0.0-alpha.1 — Cosmic Dragon Stability

- Cosmic Dragon stability fixes, rain-screen cleanliness audit, IP surface tightening.

## v25.0.0 — Dragon Hunt v2 Dead-Code Sweep

- Systematic dead-code removal across the full codebase in 5 phases (cloud, config, interactive, full sweep).
- Legacy `--fullwidth` purge (superseded by auto-detection).
- Cross-scene performance baselines, monolith-style optimizations.

## v20.x — Temporal Prediction & Legacy Purge

- v20.1.0: removed deprecated CLI flags and backward-compatibility shims.
- v20.0.0: Cosmic Dragon phase predictor (P1), adaptive resync (P2), reclaim state (P4) — the temporal-prediction milestone that gave the renderer self-awareness of long-running drift.

## v15.0.0 — Cosmic Dragon Pre-Release Polish

- Cosmic Dragon cinematic behaviors, atmospheric evolution, self-healer — the renderer becomes a director rather than a feed.

## v14.0.0 — Scene-Custom Migration (Breaking CLI)

- **Breaking**: `--scene-custom` migrated to TOML config. New CLI structure.

## v13.x — Cosmic Dragon Engine Birth

The era that turned cosmostrix from "a Matrix rain toy" into "a cinematic renderer". Key milestones:

- v13.0.0: Alive rain + depth-of-field + security hardening.
- v13.1.0: Shell completions, verbose mode, help polish.
- v13.2.0: Diff-based render engine specification, competitor benchmark comparison.
- v13.3.0: SGR cache hit-rate tracking, ANSI bytes/frame metrics.
- v13.3.1: 18 Dragon Eggs, P1/P2/P3 adaptive layers.
- v13.4.0: Added `--size` and `--duration` flags.
- v13.6.0: CLI flag simplification, background mode cleanup.

---

## v4.0.0 — Atmosphere Engine + Monolith Rain

The "real renderer" era. Cosmostrix found its identity here.

- Signature Monolith Rain as the production default (sparse data pillars, segmented blocks).
- Cosmic Dragon Core/Engine/Cache groundwork for adaptive rendering.
- Atmosphere engine, terminal compatibility lab, doctor diagnostics.
- Profile ecosystem, config discoverability, benchmark hardening.
- Canonical metadata alignment across Cargo, README, AUR.

## v3.9.0 — v4 Ground-Work

- Atmosphere visual whisper engine, cosmic dragon architecture discipline.
- Phase 10.5: atmosphere config honesty + profile smoke hardening.

---

## Pre-v13 Era — The Journey From v2 to v12

These releases are documented in detail in [`docs/archive/CHANGELOG_PRE_V13.md`](docs/archive/CHANGELOG_PRE_V13.md). The summary below captures the arc.

### v12.0.0 — Protocol Engine

Terminal protocol detection (kitty keyboard, synchronized output, in-band resize reports). Render path respects each terminal's capabilities instead of falling back to lowest-common-denominator.

### v11.x — Cinematic Peak & Benchmark Depth

- v11.1.0: Benchmark reaches S-tier — RSS memory tracking, p99.9 / max frame-time metrics, sub-component timing (sim/render/io), JSON output mode, live HUD overlay. Theme tuning makes the 43 builtin palettes visually distinct.
- v11.0.0: Cinematic peak. Smoothstep easing on pause/resume, top-to-bottom wave color transitions, mouse-click effects, bracketed-paste safety.

### v10.0.0 — Peak Performance & Stability

Diff-based cell renderer reaches steady state. All known frame-time regressions resolved. Long-run soak tests (10h+) confirm zero leaks in memory, FDs, threads, CPU.

### v5.0.0 — Nightfall

Visual identity overhaul. TrueColor gradients become the default on capable terminals; ANSI 256-color mode remains as a fallback. CRT phosphor decay model replaced with physics-based exponential curve.

### v4.x — Atmosphere Polish

Iterative atmosphere work across v4.5–v4.9: fog vignette tuning, parallax brightness calibration, head self-bloom, climate luminance/saturation minimums, profile luminance offsets. Each release raised the visual floor without changing the architecture from v4.0.0.

### v3.x — The Foundational Era

- v3.9.0: ground-work for v4 (above).
- v3.1.0: first appearance of droplet physics and the rain-style lifecycle.
- v3.0.0: initial public release — basic rain rendering, single color, no scenes, no profiles.

### v2.x — Soak & Stability

- v2.1.0: visual contrast & readability overhaul — readable body glyphs, depth-layer visibility, CRT afterglow, pause/resume easing, mouse mode default-off, safe terminal cleanup on all exit paths.
- v2.0.0: first public-stability release. Stale glyph artifacts fixed, long-idle resync, direct-color auto-detection for `xterm-direct` / `tmux-direct`. 10h+ visual soak checks confirmed no leaks.
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
