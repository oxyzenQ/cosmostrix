# Cosmostrix Documentation Index
<!-- SPDX-License-Identifier: GPL-3.0-only -->

Master index of all cosmostrix documentation. Use this as your map when returning to the project after a long break.

## Quick Navigation

| I want to... | Go to |
|-------------|-------|
| Understand what cosmostrix is | [README.md](../README.md) |
| Run a benchmark | [BENCHMARKING.md](BENCHMARKING.md) |
| Tune the rain visuals | [CENTRAL_CONTROL_RAINS_USAGE.md](CENTRAL_CONTROL_RAINS_USAGE.md) |
| Understand the render engine | [RENDER_ENGINE.md](RENDER_ENGINE.md) |
| Understand the architecture | [COSMIC_DRAGON_ARCHITECTURE.md](COSMIC_DRAGON_ARCHITECTURE.md) |
| Build from source | [README.md § Installation](../README.md#installation) |
| Release a new version | [workflow/ABOUT_CI.md](workflow/ABOUT_CI.md) |
| Recover a broken terminal | `cosmostrix --reset-terminal` or [TERMINAL_KILL_CLEANUP.md](TERMINAL_KILL_CLEANUP.md) |
| Report a bug / contribute | [RULES.md](RULES.md) |

## Architecture & Engine

| Doc | Covers |
|-----|--------|
| [RENDER_ENGINE.md](RENDER_ENGINE.md) | Diff-based rendering engine spec (`src/frame.rs`, `src/terminal/`, `src/terminal_tty.rs`) |
| [COSMIC_DRAGON_ARCHITECTURE.md](COSMIC_DRAGON_ARCHITECTURE.md) | Full architecture deep-dive (`src/`) |
| [PHILOSOPHY.md](PHILOSOPHY.md) | Why cosmostrix exists, design principles |
| [SIMD_FEASIBILITY.md](SIMD_FEASIBILITY.md) | SIMD optimization feasibility (rejected; `src/bolt.rs`) |

**Two cooperating engines**: the **Cosmic Dragon** diff-based rendering engine (owns *what cells changed* — `src/frame.rs`, `src/terminal/`, `src/runtime.rs`) and the **Chroma Dragon** coloring engine (owns *what color a cell becomes* — `src/chroma/`).

## Benchmarking & Performance

| Doc | Covers |
|-----|--------|
| [BENCHMARKING.md](BENCHMARKING.md) | **Start here.** Independent benchmarking guide |
| [BENCHMARK_ADVANCED.md](BENCHMARK_ADVANCED.md) | MICROARCHITECTURE + ENERGY metrics (Linux `perf_event_open` + RAPL) |
| [PERFORMANCE_ACROSS_SCALES.md](PERFORMANCE_ACROSS_SCALES.md) | FPS scaling with screen size (6×6 → 200×60) |
| [ENDURANCE.md](ENDURANCE.md) | Long-run endurance testing, memory leak detection |
| [RELEASE_GUARD.md](RELEASE_GUARD.md) | Performance regression gates for releases |
| [RAPL_ACCESS.md](RAPL_ACCESS.md) | Granting RAPL read access for ENERGY metrics |

## Rain Visuals & Tuning

| Doc | Covers |
|-----|--------|
| [CENTRAL_CONTROL_RAINS_USAGE.md](CENTRAL_CONTROL_RAINS_USAGE.md) | **The tuning bible** — every rain visual knob (`src/central_control_rains.rs`) |
| [RAIN_DEPTH_AUDIT.md](RAIN_DEPTH_AUDIT.md) | Visual-audit methodology for rain depth stack |

The atmosphere engine subsystem was eliminated at commit `07b44b5` (2026-08-05). Historical spec at [archive/specs/ATMOSPHERE_ENGINE.md](archive/specs/ATMOSPHERE_ENGINE.md); elimination record at [archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md](archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md). Subsystems still sharing the "atmosphere" name (`src/chroma/post/climate.rs`, `AtmosphericEvolution` in `src/cloud/ecosystem.rs`) are separate and remain live.

## Color & Theming (Chroma Dragon)

| Doc | Covers |
|-----|--------|
| [../README.md § Chroma Dragon](../README.md#the-chroma-dragon-coloring-engine) | High-level overview, Phase 9-D lock (`src/chroma/`) |
| [src/chroma/catalog.rs](../src/chroma/catalog.rs) | **Central color theme registry** — single source of truth |
| [src/chroma/palette.rs](../src/chroma/palette.rs) | Palette construction, OKLab interpolation |
| [src/chroma/tuning.rs](../src/chroma/tuning.rs) | `--color-tune` key=value tuning |

**Adding a new color theme**: add a variant to `ColorScheme` in `src/runtime.rs`, then add one `ThemeDef` to `THEMES` in `src/chroma/catalog.rs`. `--list-colors`, `--color <name>`, and `build_palette()` auto-discover from the registry.

## Terminal Compatibility & Recovery

| Doc | Covers |
|-----|--------|
| [TERMINAL_COMPATIBILITY.md](TERMINAL_COMPATIBILITY.md) | Terminal behavior matrix, tmux/SSH, known quirks |
| [TERMINAL_KILL_CLEANUP.md](TERMINAL_KILL_CLEANUP.md) | Kill/crash recovery |
| [TERMINAL_LIFECYCLE_MATRIX.md](TERMINAL_LIFECYCLE_MATRIX.md) | Full terminal lifecycle (init, alt screen, raw mode, cleanup) |
| [STABILITY_AUDIT.md](STABILITY_AUDIT.md) | Terminal stability stress tests |
| [HUD.md](HUD.md) | Live HUD overlay reference |

**Emergency recovery**: `cosmostrix --reset-terminal` — 5-layer recovery (ANSI + crossterm + stty + reset). Restores the terminal from any state, including after `kill -9`.

## Build, Release & CI

| Doc | Covers |
|-----|--------|
| [../README.md § Installation](../README.md#installation) | Build instructions, PGO nitro build |
| [workflow/ABOUT_CI.md](workflow/ABOUT_CI.md) | CI pipeline + release process |
| [RELEASE_CANDIDATE.md](RELEASE_CANDIDATE.md) | Release candidate checklist |
| [VERIFY_RELEASE.md](VERIFY_RELEASE.md) | Post-release artifact verification |
| [SUPPLY_CHAIN.md](SUPPLY_CHAIN.md) | Supply-chain hardening (cargo-deny, audit, MSRV) |
| [SYSTEM_REQUIREMENTS.md](SYSTEM_REQUIREMENTS.md) | Kernel, glibc/musl, CPU, terminal matrix |

Other meta docs: [RULES.md](RULES.md) (conventions), [BRANDING.md](BRANDING.md) (brand identity), [MAINTENANCE.md](MAINTENANCE.md) (dormant-mode guide), [../KNOWN_ISSUES.md](../KNOWN_ISSUES.md), [../CHANGELOG.md](../CHANGELOG.md), [../CONTRIBUTING.md](../CONTRIBUTING.md).

## Coming Back After a Long Break?

Read in order: [README.md](../README.md) → [CHANGELOG.md](../CHANGELOG.md) → this index → [RULES.md](RULES.md) → [workflow/ABOUT_CI.md](workflow/ABOUT_CI.md). Sanity check:

```bash
git pull origin main && cargo build --release && cargo test --all --locked
cargo fmt --all -- --check && cargo clippy --locked --all-targets --all-features -- -D warnings
cosmostrix --doctor && cosmostrix --benchmark --bench-duration 5s
```
## Key Invariants & Doc Maintenance

**Invariants**: honesty contract (every flag in `--help`, strict validation); single-threaded (`planned_worker_budget: 0`); CPU-only (no GPU context); zero-alloc hot path; diff-based rendering (never full-screen redraw in interactive mode); lock tests (`src/cosmic_dragon/lock_tests.rs`, `src/chroma/lock_tests.rs`) must pass on every commit.

**Adding a doc**: place in `docs/` (or `docs/workflow/`), add to this index, add to README Documentation list, add SPDX header, cross-link from related docs. **Removing/renaming**: grep for old filename, update all cross-references, remove from this index and README list.
