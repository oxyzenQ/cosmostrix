# Cosmostrix Documentation Index

<!-- SPDX-License-Identifier: GPL-3.0-only -->

> Master index of all cosmostrix documentation. Use this as your map when
> you come back to the project after a long break — every doc, every key
> source module, and every "where do I find X?" question is answered here.
>
> **Last updated: v40.0.0-alpha.1 (commit `5e9cff8`, 2026-08-11)

## Quick Navigation

| I want to... | Go to |
|-------------|-------|
| Understand what cosmostrix is | [README.md](../README.md) (top-level overview) |
| Run a benchmark | [docs/BENCHMARKING.md](BENCHMARKING.md) |
| Tune the rain visuals | [docs/CENTRAL_CONTROL_RAINS_USAGE.md](CENTRAL_CONTROL_RAINS_USAGE.md) |
| Add a new color theme | [src/chroma/catalog.rs](../src/chroma/catalog.rs) (module doc comments) |
| Understand the render engine | [docs/RENDER_ENGINE.md](RENDER_ENGINE.md) |
| Understand the architecture | [docs/COSMIC_DRAGON_ARCHITECTURE.md](COSMIC_DRAGON_ARCHITECTURE.md) |
| Build from source | [README.md § Installation](../README.md#installation) |
| Release a new version | [docs/workflow/about-ci.md](workflow/about-ci.md) |
| Recover a broken terminal | `cosmostrix --reset-terminal` or [docs/TERMINAL_KILL_CLEANUP.md](TERMINAL_KILL_CLEANUP.md) |
| Report a bug / contribute | [docs/RULES.md](RULES.md) |

---

## 1. Architecture & Engine

These docs explain *how cosmostrix works internally*. Read these if you're
modifying the engine or want to understand the design philosophy.

| Doc | What it covers | Key source files |
|-----|---------------|-----------------|
| [RENDER_ENGINE.md](RENDER_ENGINE.md) | Formal spec of the diff-based rendering engine (complexity analysis, design rationale, comparison vs alternatives) | `src/frame.rs`, `src/terminal/`, `src/terminal_tty.rs` |
| [COSMIC_DRAGON_ARCHITECTURE.md](COSMIC_DRAGON_ARCHITECTURE.md) | Full architecture deep-dive — all subsystems, data flow, design decisions | `src/` (top-level) |
| [COSMIC_DRAGON_EXPLORATION.md (archived)](archive/cosmic_dragon/EXPLORATION.md) | Design explorations and rejected alternatives (GPU mode, multi-threading, SIMD) | — |
| [COSMIC_DRAGON_FINDINGS.md (archived)](archive/cosmic_dragon/FINDINGS.md) | Performance findings — engine ceiling analysis, where the bottlenecks are | — |
| [PHILOSOPHY.md](PHILOSOPHY.md) | Why cosmostrix exists, design principles, what it deliberately is NOT | — |
| [SIMD_FEASIBILITY.md](SIMD_FEASIBILITY.md) | SIMD optimization feasibility study (evaluated, partially applied via BOLT) | `src/bolt.rs` |

### The Two Engines

Cosmostrix is built on **two cooperating engines**:

1. **The Cosmic Dragon Diff-Based Rendering Engine** — owns *what cells
   changed*. Lives at the crate root: `src/frame.rs`, `src/terminal/`,
   `src/terminal_tty.rs`, `src/runtime.rs`. Invariant tests in
   `src/cosmic_dragon/lock_tests.rs` lock the engine's contract.

2. **The Chroma Dragon Coloring Engine** — owns *what color a cell
   becomes*. Lives under `src/chroma/` (`palette`, `catalog`,
   `gradient`, `shaders`, `post`, `tuning`). Locked at Phase 9-B.
   Invariant tests in `src/chroma/lock_tests.rs` lock the engine's
   contract.

---

## 2. Benchmarking & Performance

These docs cover *how to measure cosmostrix's performance* and interpret
the results.

| Doc | What it covers |
|-----|---------------|
| [BENCHMARKING.md](BENCHMARKING.md) | **Start here.** Full independent benchmarking guide — how to run, interpret, compare, strict `--bench-scene` validation, v30 reference results (peak 102K FPS) |
| [BENCHMARK_CLOUD_XEON.md](BENCHMARK_CLOUD_XEON.md) | Third-party hardware verification — same commit `c97ba87` built and benchmarked on a 2-core Intel Xeon cloud VM (116K avg FPS, 1.58× the owner's Ryzen) |
| [BENCHMARK_ADVANCED.md](BENCHMARK_ADVANCED.md) | Enabling MICROARCHITECTURE and ENERGY metrics (Linux `perf_event_open` + RAPL) |
| [PERFORMANCE_ACROSS_SCALES.md](PERFORMANCE_ACROSS_SCALES.md) | How FPS scales with screen size (6×6 → 200×60) |
| [ENDURANCE.md](ENDURANCE.md) | Long-run endurance testing and resource monitoring (memory leak detection) |
| [RELEASE_GUARD.md](RELEASE_GUARD.md) | Performance regression gates for releases |
| [RAPL_ACCESS.md](RAPL_ACCESS.md) | How to grant RAPL read access for ENERGY metrics |
| [../benchmark/README.md](../benchmark/README.md) | Reference benchmark results across versions (v15, v30) + comparison vs other Matrix rain tools |

---

## 3. Rain Visuals & Tuning

These docs cover *how to tune what the rain looks like*.

| Doc | What it covers | Key source files |
|-----|---------------|-----------------|
| [CENTRAL_CONTROL_RAINS_USAGE.md](CENTRAL_CONTROL_RAINS_USAGE.md) | **The tuning bible.** Every tunable knob in the rain visual stack — per-layer brightness, depth, speed, density, phosphor decay, parallax multipliers. | `src/central_control_rains.rs` |
| [RAIN_DEPTH_AUDIT.md](RAIN_DEPTH_AUDIT.md) | Visual-audit methodology for the rain depth stack (uses `--bench-scene production-draw`) | `src/central_control_rains.rs` |
| [CINEMATIC_BREATHING.md (archived)](archive/specs/CINEMATIC_BREATHING.md) | Cinematic breathing vocabulary (Rest / Pulse / Signal / Compression / Void / Monolith-Pressure) — archived 2026-08-05 alongside atmosphere engine elimination. Concepts preserved; `--atmosphere-mode` / `--atmosphere-regime` triggers are obsolete. | (historical) |

> **Note (v30 — 2026-08-05):** the atmosphere engine subsystem (former
> `src/atmosphere_*.rs` modules, `--atmosphere-mode` / `--atmosphere-regime`
> CLI flags, `atmosphere-mode` / `atmosphere-regime` / `adaptive-custom.*`
> config keys) was fully eliminated at commit `07b44b5` (net reduction in codebase size).
> The historical design spec is preserved at
> [archive/specs/ATMOSPHERE_ENGINE.md](archive/specs/ATMOSPHERE_ENGINE.md).
> The elimination record (file list, KEPT-vs-DELETED table, backward-compat
> notes, revival guidance) is at
> [archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md](archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md).
> Subsystems that still share the "atmosphere" name (`src/chroma/post/climate.rs`
> post-FX shader, `AtmosphericEvolution` in `src/cloud/ecosystem.rs`) are
> separate subsystems and remain live — they were never part of the v4.0.0
> atmosphere engine plan.

### `central_control_rains.rs` — The Rain Control Center

This is the single file that controls the rain's visual identity. Every
constant in it directly affects what you see on screen:

- Per-layer brightness/depth stack (far/mid/near)
- Phosphor tail residual + decay rate
- 3-layer parallax multipliers (speed, brightness, length, density, decay)
- Depth fog configuration
- Monolith formation parameters
- Glitch + anomaly zone tuning

To tune the rain, edit `src/central_control_rains.rs` directly. See
[CENTRAL_CONTROL_RAINS_USAGE.md](CENTRAL_CONTROL_RAINS_USAGE.md) for the
full guide.

---

## 4. Color & Theming (Chroma Dragon)

These docs cover *how color works* in cosmostrix.

| Doc | What it covers | Key source files |
|-----|---------------|-----------------|
| [../README.md § About — The Chroma Dragon Coloring Engine](../README.md#the-chroma-dragon-coloring-engine) | High-level overview of the Chroma Dragon, Phase 9-B lock, the 9 phases | `src/chroma/` |
| [src/chroma/mod.rs](../src/chroma/mod.rs) | Module doc — Chroma Dragon phase history, module map | `src/chroma/mod.rs` |
| [src/chroma/catalog.rs](../src/chroma/catalog.rs) | **Central color theme registry** — single source of truth for ALL color schemes. To add a new theme: add a variant to `ColorScheme` enum in `runtime.rs`, then add one `ThemeDef` entry to the `THEMES` array. `--list-colors`, `--color <name>`, and `build_palette()` all auto-discover from this registry. | `src/chroma/catalog.rs` |
| [src/chroma/palette.rs](../src/chroma/palette.rs) | Palette construction — RGB gradient stops, OKLab interpolation, Color16/ANSI fallbacks | `src/chroma/palette.rs` |
| [src/chroma/gradient.rs](../src/chroma/gradient.rs) | OKLab gradient interpolation (perceptually uniform) | `src/chroma/gradient.rs` |
| [src/chroma/tuning.rs](../src/chroma/tuning.rs) | `--color-tune` key=value tuning (sat, bright, head, body, tail) | `src/chroma/tuning.rs` |
| [src/chroma/shaders/](../src/chroma/shaders/) | Cell-color decision logic — `resolve_cell_color()`, `CharLoc` enum, `TRAIL_EXP_LUT` | `src/chroma/shaders/` |
| [src/chroma/post/](../src/chroma/post/) | Atmospheric post-processing + palette-aware anomaly halos | `src/chroma/post/` |
| [src/chroma/lock_tests.rs](../src/chroma/lock_tests.rs) | Invariant tests that lock the Chroma Dragon's contract on every commit | — |

### Adding a New Color Theme

1. Add a variant to the `ColorScheme` enum in `src/runtime.rs`.
2. Add one `ThemeDef` entry to the `THEMES` array in
   `src/chroma/catalog.rs`.

That's it. `--list-colors`, `--color <name>`, and `build_palette()` all
auto-discover the new theme from the registry. See the module doc at the
top of `src/chroma/catalog.rs` for the full guide.

---

## 5. Terminal Compatibility & Recovery

These docs cover *how cosmostrix interacts with terminals* and how to
recover from a broken terminal state.

| Doc | What it covers |
|-----|---------------|
| [TERMINAL_COMPATIBILITY.md](TERMINAL_COMPATIBILITY.md) | Terminal behavior matrix — which terminals work, tmux/SSH, known quirks |
| [TERMINAL_KILL_CLEANUP.md](TERMINAL_KILL_CLEANUP.md) | What happens when cosmostrix is killed (SIGKILL, Ctrl-C, close window) and how to recover |
| [TERMINAL_LIFECYCLE_MATRIX.md](TERMINAL_LIFECYCLE_MATRIX.md) | Full terminal lifecycle — init, alternate screen, raw mode, cleanup paths |
| [STABILITY_AUDIT.md](STABILITY_AUDIT.md) | Terminal stability audit — stress tests, edge cases, recovery verification |
| [HUD.md](HUD.md) | Live HUD overlay reference — what each line means, why `fps:` ≠ `--fps`, HUD vs `--benchmark` |

### Emergency Recovery

If your terminal is broken after a crash:

```bash
cosmostrix --reset-terminal    # 5-layer recovery: ANSI + crossterm + stty + reset
```

This is the nuclear option. It restores the terminal from any state,
including after `kill -9` (which no process can intercept).

---

## 6. Build, Release & CI

These docs cover *how to build, release, and ship cosmostrix*.

| Doc | What it covers |
|-----|---------------|
| [../README.md § Installation](../README.md#installation) | Build instructions — `cargo build`, `scripts/build.sh`, PGO nitro build |
| [../README.md § Release Process](../README.md#release-process) | Version bump + build + tag + CI release |
| [workflow/about-ci.md](workflow/about-ci.md) | CI pipeline — 14 jobs (lint, build, test, security audit, version sync, cross-platform builds) |
| [RELEASE_CANDIDATE.md](RELEASE_CANDIDATE.md) | Release candidate checklist |
| [VERIFY_RELEASE.md](VERIFY_RELEASE.md) | Post-release verification steps |
| [SUPPLY_CHAIN.md](SUPPLY_CHAIN.md) | Supply-chain hardening policy (cargo-deny, audit, MSRV) |
| [SYSTEM_REQUIREMENTS.md](SYSTEM_REQUIREMENTS.md) | Kernel, glibc/musl, CPU, terminal compatibility matrix |

### Build Profiles

| Profile | Target | Use case |
|---------|--------|----------|
| `debug` | host | Development, fast iteration |
| `release` | host | Optimized release build |
| `pro-linux-v3` | x86_64-unknown-linux-gnu (AVX2/BMI2/FMA) | Production Linux (most users) |
| `pro-linux-v4` | x86_64-unknown-linux-gnu (AVX-512) | Modern Linux (Zen 4+, Ice Lake+) |
| `pro-macos-aarch64-native` | aarch64-apple-darwin | Apple Silicon (M1/M2/M3) |
| `pro-linux-musl` | x86_64-unknown-linux-musl | Static musl build (Alpine, containers) |

---

## 7. Project Conventions & Meta

These docs cover *how the project is organized* and what rules govern
contributions.

| Doc | What it covers |
|-----|---------------|
| [RULES.md](RULES.md) | Project conventions — code style, commit messages, PR rules |
| [BRANDING.md](BRANDING.md) | Brand identity — name, logo, signature, visual language |
| [KNOWN_ISSUES.md](../KNOWN_ISSUES.md) | Platform-specific quirks, workarounds, planned fixes |
| [../CHANGELOG.md](../CHANGELOG.md) | Release history — every version, every change |
| [research/SELF_HEALING_AUDIT.md](research/SELF_HEALING_AUDIT.md) | Self-healing audit — how cosmostrix recovers from failures |
| [research/dead-dragon-ab/](research/dead-dragon-ab/) | A/B comparison: dead dragon (old) vs live dragon (current) — performance + visual |
| [research/dead-dragon-ab/comparison-report.md](research/dead-dragon-ab/comparison-report.md) | Full comparison report — dead dragon vs live dragon benchmarks across screen sizes |

---

## 8. Source Module Map

Every key source module and what it does. Use this when you need to find
where a feature lives.

### Cosmic Dragon (Rendering Engine)

| Source file | What it does |
|-------------|-------------|
| `src/frame.rs` | Frame buffer + dirty-cell tracking (O(1) `clear_dirty` via u32 bump) |
| `src/terminal/` | Terminal abstraction — alternate screen, raw mode, ANSI output, ColorCache |
| `src/terminal_tty.rs` | `/dev/tty` fallback — recovers from broken stdout mid-run |
| `src/runtime.rs` | Runtime enums — `ColorMode`, `ShadingMode`, `BoldMode`, `ColorScheme` |
| `src/bolt.rs` | BOLT — Branchless Optimized Lookup Tables. Project-wide branchless formatting for the hot render path. |
| `src/central_control_rains.rs` | **Rain control center** — every tunable knob for the rain visual stack. See [CENTRAL_CONTROL_RAINS_USAGE.md](CENTRAL_CONTROL_RAINS_USAGE.md). |

### Cloud (Droplet Simulation)

| Source file | What it does |
|-------------|-------------|
| `src/cloud/mod.rs` | Cloud module root — droplet cloud state, column management |
| `src/cloud/rain.rs` | Rain simulation — droplet physics, spawn, fall, collision |
| `src/cloud/spawn.rs` | Spawn logic — 3-layer parallax, density maps, monolith formations |
| `src/cloud/phosphor.rs` | Phosphor persistence — CRT afterglow, per-layer decay |
| `src/cloud/monolith.rs` | Monolith scene — density-sculpted pillar formations |
| `src/cloud/living_rain.rs` | Living rain — organic motion, gust-driven acceleration |
| `src/cloud/ecosystem.rs` | Ecosystem — droplet lifecycle, die-early, short-rain |
| `src/cloud/render.rs` | Render path — `emit_cell_lean` (fast) + `Terminal::draw` (production) |
| `src/cloud/scene_runtime.rs` | Scene runtime — scene switching, parameter application |
| `src/cloud/runtime_controls.rs` | Runtime controls — live parameter adjustment |

### Chroma Dragon (Coloring Engine)

| Source file | What it does |
|-------------|-------------|
| `src/chroma/mod.rs` | Module root — phase history, module map |
| `src/chroma/catalog.rs` | **Central color theme registry** — single source of truth for all 44 themes |
| `src/chroma/palette.rs` | Palette construction — RGB stops, OKLab interpolation, fallbacks |
| `src/chroma/gradient.rs` | OKLab gradient interpolation (perceptually uniform) |
| `src/chroma/tuning.rs` | `--color-tune` key=value tuning |
| `src/chroma/shaders/` | Cell-color decision logic (`resolve_cell_color`) |
| `src/chroma/post/` | Atmospheric post-processing, anomaly halos |
| `src/chroma/lock_tests.rs` | Invariant tests (Phase 9-B lock) |

### Atmosphere Engine (REMOVED 2026-08-05)

The atmosphere engine subsystem was fully eliminated at commit `07b44b5`
(Dragon Hunt v2 Phase 6 Tier E item 31 — final elimination). All
`src/atmosphere_*.rs` source files listed in historical revisions of this
table have been deleted; the same applies to the `--atmosphere-mode` /
`--atmosphere-regime` CLI flags and the `atmosphere-mode` /
`atmosphere-regime` / `adaptive-custom.*` config keys. See
[archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md](archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md)
for the full elimination record.

Subsystems that still share the "atmosphere" name (separate subsystems,
not the v4.0.0 atmosphere engine — KEPT):

| Source file | What it does |
|-------------|-------------|
| `src/chroma/post/climate.rs` | Chroma Dragon post-FX shader — luminance/saturation/instability. Used by `chroma::shaders::base::resolve_cell_color` for every cell render. |
| `src/cloud/ecosystem.rs::AtmosphericEvolution` | Cloud drift/gust events (entropy_phase, density_offset, luminance_offset, anomaly_offset, cycle_speed). |

### Benchmark Subsystem

| Source file | What it does |
|-------------|-------------|
| `src/bench.rs` | Main benchmark entry points (`run_benchmark`, `run_premium_benchmark`) + `--bench-scene` strict validation |
| `src/bench_io.rs` | `BenchIoWriter` — wet I/O writer (writes ANSI to /dev/null) |
| `src/bench_helpers.rs` | Benchmark helpers — dimensions, duration resolution |
| `src/bench_baseline.rs` | `--save-baseline` / `--compare-baseline` |
| `src/bench_comp.rs` | Baseline comparison logic |
| `src/bench_cpu.rs` | CPU usage sampling |
| `src/bench_energy.rs` | ENERGY section — RAPL powercap sysfs (Linux) |
| `src/bench_json.rs` | JSON output formatter |
| `src/bench_mem.rs` | MEMORY section — RSS sampling |
| `src/bench_meta.rs` | Benchmark metadata — SYSTEM, RENDERER, CONFIG sections |
| `src/bench_perf.rs` | MICROARCHITECTURE section — perf_event_open (Linux) |
| `src/bench_visual.rs` | VisualSampler — frame entropy, density Gini |

### App & Config

| Source file | What it does |
|-------------|-------------|
| `src/main.rs` | Entry point — arg parsing, dispatch, run loop |
| `src/app.rs` | `CloudConfig` struct — the central config object |
| `src/config.rs` | CLI arg definitions (clap) + config.toml parsing |
| `src/live_config.rs` | Live config reload — filesystem watch, strict validation |
| `src/help_detail.rs` | `--help` output (full curated reference manual) |
| `src/ux.rs` | UX helpers — `or_exit`, `die_input`, error formatting |

---

## 9. Coming Back After a Long Break?

If you're returning to cosmostrix after months or years of inactivity,
read these in order:

1. **[README.md](../README.md)** — re-orient on what cosmostrix is and
   what's new in the latest version.
2. **[CHANGELOG.md](../CHANGELOG.md)** — scan recent versions for
   breaking changes or new features.
3. **This index** — find the doc for whatever you need to work on.
4. **[docs/RULES.md](RULES.md)** — refresh on project conventions before
   making changes.
5. **[docs/workflow/about-ci.md](workflow/about-ci.md)** — check what CI
   expects before pushing.

### Quick Sanity Check

```bash
git pull origin main
cargo build --release                    # or: ./scripts/build.sh pro-linux-v3
cargo test --all --locked                # all tests pass?
cargo fmt --all -- --check               # formatting clean?
cargo clippy --locked --all-targets --all-features -- -D warnings
cosmostrix --doctor                      # diagnostics pass?
cosmostrix --benchmark --bench-duration 5s   # benchmark runs?
```

If all of these pass, the project is in a healthy state and you're ready
to make changes.

### Key Invariants to Preserve

- **Honesty contract**: no hidden flags, no hidden behavior. Every flag
  is documented in `--help`. Strict validation on
  all enum-like flags (see `--bench-scene` as the reference pattern).
- **Single-thread optimized**: `planned_worker_budget: 0` by design.
  Do not add multi-threading — it violates the architecture.
- **CPU-only**: no GPU context is ever created. GPU image-mode was
  evaluated and rejected (see [PHILOSOPHY.md](PHILOSOPHY.md)).
- **Zero-alloc hot path**: the render + I/O hot path must not allocate.
  Allocs are allowed in sim (atmospheric event bookkeeping) but not in
  the per-frame render/io loop.
- **Diff-based rendering**: the core innovation. Never fall back to
  full-screen redraw in interactive mode.
- **Lock tests**: `src/cosmic_dragon/lock_tests.rs` and
  `src/chroma/lock_tests.rs` must pass on every commit.
  They lock the engines' public contracts.

---

## 10. Doc Maintenance

When adding a new doc:

1. Place it in `docs/` (or `docs/workflow/` for CI/process docs).
2. Add it to the appropriate section in this index.
3. Add it to the Documentation list in [README.md](../README.md).
4. Add SPDX license header: `<!-- SPDX-License-Identifier: GPL-3.0-only -->`
5. Cross-link from related docs (see-also sections).

When removing or renaming a doc:

1. Update all cross-references (grep for the old filename).
2. Remove it from this index.
3. Remove it from the README.md Documentation list.

---

*This index is the map. The docs are the territory. When in doubt,
start here.*
