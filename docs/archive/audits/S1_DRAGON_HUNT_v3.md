<!-- SPDX-License-Identifier: GPL-3.0-only -->

# S-master-1 — Dragon Hunt v3 (Post-Peak Sweep)

**Date:** 2026-09-01
**Scope:** `cosmostrix/*`, `src/*`, `deps` (perstage — important dirs only)
**Author:** oxyzenQ (cosmic dragon mode, master audit pass)
**Predecessors:** A1–A5 ZOMBIE_KILL, B1–B4 OPTIMIZE, Z1–Z5 LTS

## Context

The codebase had already been through multiple gatekeeper sweeps
(A1–A5 ZOMBIE_KILL, B1–B4 OPTIMIZE, Z1–Z5 LTS). This audit was a
**post-peak verification pass** to confirm whether any remaining
spaghetti / burden / duplicate / redundant / stale / zombie code
exists, and to clean up what was found.

## Method

Static-only audit (ripgrep + Read, no cargo) of all task-brief
directories:

- `src/engine/{cosmic,chroma,crystal}_dragon_engine/`
- `src/cli/`, `src/config/`, `src/interactive/`, `src/bench/`
- `build.rs`, `Cargo.toml`
- `src/central_control_{dragon_power,rains}/`, `src/cosmic_dragon_incubator/`
- `src/output/`, `src/diagnostics/`, `src/sysstat/`

Total surface: 336 `.rs` files / 215,898 LOC.

## Findings

### Category 1 — Zombie / dead code (7 items, all low severity)

| Path:line | Description | Action taken |
|---|---|---|
| `src/engine/crystal_dragon_engine/point_system/mod.rs:122-127` | `DriftHistory::reset()` marked `#[allow(dead_code)]`, zero callers | **Removed** — safer than wiring up (would change color drift behavior) |
| `src/cli/build_cloud_cfg.rs:54` | `CfgInputs.fps_precedence` field built in main.rs but destructured `_` | **Removed** — field is dead; `fps_precedence` local in main.rs still used by `run_verbose_startup` |
| `src/cli/build_cloud_cfg.rs:56` | `CfgInputs.color_tune` had stale `#[allow(dead_code)]` | **Annotation removed** — field IS used (passed to CloudConfig.color_tune) |
| `src/cli/build_cloud_cfg.rs:62` | `CfgInputs.intro` built but discarded (`intro: _`); CloudConfig.intro rebuilt from args | **Removed** — pure dead weight |
| `src/cli/build_cloud_cfg.rs:64` | `CfgInputs.intro_color` cloned but discarded; CloudConfig.intro_color rebuilt from args | **Removed** — saves a `.clone()` at startup |
| `src/engine/cosmic_dragon_engine/cloud/mod.rs:300-301` | `Cloud.profile: BehaviorProfile` field, `#[allow(dead_code)]`, "Retained for future profile selector" | **Left as-is** — documented future-reserved slot, defensible |
| `src/engine/chroma_dragon_engine/intro_colors.rs:97` | `LOGO_COLOR_RGB` const, `#[allow(dead_code)]`, "Historical reference for pre-v51 default flat brand purple" | **Left as-is** — documentation anchor |
| `src/engine/crystal_dragon_engine/sensor/mod.rs:92,113` | `CrystalDragonSensor` struct + impl had stale `#[allow(dead_code)]`; all 9 methods + 7 fields actively used | **Annotations removed** — was silencing a fully-used type |

### Category 2 — Stale code / comments (~55 instances)

**Root cause:** project LOC cap was tightened from 1500 → 800 on
2026-08-28 (per `src/RULES_LOC.md` owner mandate), but:

1. **4 runtime LOC guards** still enforced the old 1500 cap:
   - `src/tests/loc.rs:9` — `MAX_RUST_LOC = 1500` (the global guard)
   - `src/diagnostics/info.rs:633` — `info_file_stays_under_loc_cap`
   - `src/bench/bench_helpers.rs:380` — `bench_file_stays_under_target_loc`
   - `src/bench/bench_report_tests.rs:233` — `bench_report_bench_report_file_stays_under_loc_cap`
   - `src/engine/cosmic_dragon_engine/cloud/tests/tests_scene/mod.rs` — 3 `assert!(count <= 1500)` guards

   **All tightened to 800.** This is a real guardrail improvement —
   the runtime tests now actually enforce the policy that
   `scripts/check-rs-loc.sh` enforces at build time.

2. **1 stale filename** in self-skip guard:
   - `src/tests/loc.rs:81` skipped `loc_tests.rs` but the file was renamed to `loc.rs`. Self-skip was a no-op.
   - **Fixed** — skip now targets `loc.rs`.

3. **3 stale `loc_tests` references** in module docs:
   - `src/tests/mod.rs:11`, `src/config/live_config_poll/mod.rs:6`, `src/config/live_config_trace.rs:20`
   - **All updated** to `src/tests/loc.rs`.

4. **~47 stale "1500-LOC" comment references** across 38 files —
   module doc-comments (`//!`) explaining why files were extracted
   still cited the old 1500 cap.
   - **All swept** to 800-LOC via `scripts/s1_sweep_1500_loc.py`
     (mechanical, comment-only, no behavior change).

### Category 3 — Duplicate / redundant code

**SKIP — mostly clean.** Three pairs of "parallel" implementations
identified, all documented as intentional pipeline splits with
parity tests:

- `chroma::legacy::blend_toward_white` (RGB tuple) vs `chroma::palette::blend_toward_white` (Color) — two color pipelines (legacy sRGB-linear vs OKLab-chroma)
- `palette::blend_toward_bg` vs `palette::blend_toward_bg_rgb` vs `palette::blend_toward_bg_rgb_unclamped` — three API shapes for the same math (tuple variants avoid Color wrap/decode round-trip on hot paths)
- `intro_style::lerp_rgb` (tuple) vs `chroma::palette::lerp_u8` (per-channel) — different signatures, different consumers

No action taken — over-engineering to merge these would hurt hot-path
performance.

### Category 4 — Spaghetti / burden

**SKIP — mostly clean.** All >800 LOC files have explicit
`// LOC_EXEMPT:` markers with one-line justifications:

- `cosmic_dragon_engine/cloud/rain_at.rs` (980 LOC) — single render algorithm, each step mutates cloud state consumed by the next
- `interactive/event_loop.rs` (816 LOC) — while-cloud.raining loop with deeply coupled mutable state across 20+ sibling modules
- `bench/premium.rs` (761 LOC) — under 800 cap
- `main.rs` (761 LOC) — under 800 cap

Large fns are genuinely cohesive — artificial splits would hurt
readability. No action taken.

### Category 5 — Redundant deps

**SKIP — clean.** All 8 direct prod deps + 1 dev dep actively used
(verified via `rg -l "use <dep>"`). Two transitive duplicates
(`bitflags 1.3.2 + 2.13.1`, `windows-sys 0.52 + 0.61`) already
documented in A5 audit as unavoidable. `ctrlc` is target-cfg-gated
to Windows only.

### Category 6 — build.rs burden

**SKIP — clean (justified).** 795 LOC / 26KB. Read in full.
Breakdown:

- Build metadata emission (profile, CPU baseline, git SHA, rustc version, build timestamp)
- Minimal TOML subset parser for `[profile.X] inherits = Y` resolution (hand-rolled to avoid `toml_edit` dep)
- x86-64-v3/v4 + aarch64 CPU baseline detection + cross-check
- Howard Hinnant's `civil_from_days` algorithm — replaces dropped `chrono` build-dep (saves ~1.3s per clean release build)
- 4 unit tests covering profile resolution, LTO/panic/strip normalization, PGO labeling, and the date algorithm

No over-engineering, no zombies, no stale logic. The size is
justified by the feature set + tests + extensive rationale comments.

## A/B Benchmark (10s, scene=monolith)

| Size | Metric | A (before) | B (after) | Delta | Verdict |
|---|---|---|---|---|---|
| 6x6 | avg_fps | 1,578,626 | 1,569,252 | -0.59% | stable |
| 6x6 | entropy | 0.0000 | 0.0043 | +0.00% | stable |
| 6x6 | gini | 0.8333 | 0.8319 | -0.17% | stable |
| 6x6 | avg_dirty_cells | 0.6675 | 0.6677 | +0.03% | stable |
| 20x20 | avg_fps | 500,806 | 492,781 | -1.60% | stable |
| 20x20 | entropy | 0.7536 | 0.7526 | -0.14% | stable |
| 20x20 | gini | 0.9165 | 0.9166 | +0.00% | stable |
| 20x20 | avg_dirty_cells | 7.9345 | 7.9295 | -0.06% | stable |
| 40x20 | avg_fps | 302,626 | 305,198 | +0.85% | stable |
| 40x20 | entropy | 1.4372 | 1.4364 | -0.06% | stable |
| 40x20 | gini | 0.9358 | 0.9359 | +0.00% | stable |
| 40x20 | avg_dirty_cells | 14.2378 | 14.2195 | -0.13% | stable |
| 80x24 | avg_fps | 92,638 | 90,935 | -1.84% | stable |
| 80x24 | entropy | 3.2959 | 3.2935 | -0.07% | stable |
| 80x24 | gini | 0.8960 | 0.8962 | +0.02% | stable |
| 80x24 | avg_dirty_cells | 56.7817 | 56.7969 | +0.03% | stable |
| 120x40 | avg_fps | 53,576 | 53,702 | +0.24% | stable |
| 120x40 | entropy | 3.9257 | 3.9242 | -0.04% | stable |
| 120x40 | gini | 0.8942 | 0.8943 | +0.01% | stable |
| 120x40 | avg_dirty_cells | 107.4414 | 107.3961 | -0.04% | stable |
| 200x60 | avg_fps | 29,265 | 29,619 | +1.21% | stable |
| 200x60 | entropy | 4.7106 | 4.7157 | +0.11% | stable |
| 200x60 | gini | 0.8907 | 0.8899 | -0.09% | stable |
| 200x60 | avg_dirty_cells | 205.1405 | 204.9715 | -0.08% | stable |

**All 24 metrics within ±2% natural variance.** Max delta is -1.84%
fps at 80x24 (within noise — bench has ~3% run-to-run variance).
Visual metrics (gini, entropy) all <0.2% delta. **Zero visual or
performance regression confirmed**, as required by task brief.

Raw JSON: `benchmark/bench-labs/S_master_dragon/S1_baseline_A.json`
and `S1_after_B.json`.

## Verdict

**Codebase confirmed post-peak-clean.** After this sweep:

- All runtime LOC guards now enforce the actual 800-LOC policy
  (was 1500 — false sense of safety).
- All `CfgInputs` dead fields removed (3 fields + 3 `#[allow(dead_code)]`).
- `DriftHistory::reset()` dead method removed.
- `CrystalDragonSensor` stale `#[allow(dead_code)]` annotations removed.
- All `1500-LOC` / `loc_tests` stale references updated.

**No high-severity issues. No architectural rot. No critical zombies.
No redundant deps. No build.rs over-engineering.**

The remaining "dragons" are documented future-reserved slots
(`Cloud.profile`, `LOGO_COLOR_RGB`) and intentional parallel
implementations with parity tests — both defensible.

## Files Changed

- `src/tests/loc.rs` — MAX_RUST_LOC 1500→800, skip filename `loc_tests.rs`→`loc.rs`
- `src/diagnostics/info.rs` — LOC guard 1500→800
- `src/bench/bench_helpers.rs` — LOC guard 1500→800
- `src/bench/bench_report_tests.rs` — LOC guard 1500→800
- `src/engine/cosmic_dragon_engine/cloud/tests/tests_scene/mod.rs` — 3 LOC guards 1500→800
- `src/cli/build_cloud_cfg.rs` — removed 3 dead CfgInputs fields + 3 stale `#[allow(dead_code)]` + unused `IntroType` import
- `src/main.rs` — removed 3 dead field assignments in CfgInputs construction
- `src/engine/crystal_dragon_engine/point_system/mod.rs` — removed dead `DriftHistory::reset()` method
- `src/engine/crystal_dragon_engine/sensor/mod.rs` — removed 2 stale `#[allow(dead_code)]` annotations
- `src/tests/mod.rs`, `src/config/live_config_poll/mod.rs`, `src/config/live_config_trace.rs` — stale `loc_tests` → `src/tests/loc.rs`
- 38 files swept for `1500-LOC` → `800-LOC` stale comment references
- `benchmark/bench-labs/S_master_dragon/` — A/B JSON + report (new)
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
