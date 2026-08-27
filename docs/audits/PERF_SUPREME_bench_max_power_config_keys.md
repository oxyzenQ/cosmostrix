<!-- SPDX-License-Identifier: GPL-3.0-only -->

# PERF Supreme — Benchmark Mode Maximum Power Audit + CONFIG Completeness

**Date:** 2026-08-27
**Auditor:** oxyzenQ (Cosmic Dragon mode)
**Version:** v50.0.0-beta.6
**Scope:** PERF-1-Supreme (benchmark mode = critical path only, maximum power, no barriers) + PERF-2-Supreme (benchmark CONFIG section completeness: `power_dragon`, `crystal_dragon`, `no_effects`).

---

## 0. Executive Summary

**PERF-1-Supreme: VERIFIED + HARDENED.** The benchmark path was already free of every
power-management barrier (idle FPS throttle, self-healer, perf_pressure clamps, madvise,
xterm.js cap — all interactive-only). Two cosmetic leftovers were still running during
measurement frames and are now gated: the cinematic CRT vignette post-process and the
emergent storytelling engine. Measured A/B on release build (same machine, 5 s run):
**avg_fps 91,096.90 → 94,211.97 (+3.4%)**, median 94,786.73 → 97,318.87 (+2.6%). The gain
is honest and modest — these were the only two remaining non-critical-path workloads.

**PERF-2-Supreme: FIXED.** The benchmark CONFIG section now reports `power_dragon`,
`crystal_dragon`, `msg_mode`, and the owner-requested `no_effects` key (true when
`--no-effects` is set). The `--json` output gained the same four keys for CI/script
parity (they previously existed only in the text report).

**Honest answer to "do power-dragon / crystal-dragon throttle the benchmark?": NO.**
Neither system reduces benchmark throughput. Details in §2.

---

## 1. What "rain + 3 dragon engines" means (audit vocabulary)

| Component | Engine | In bench? |
|---|---|---|
| Droplet spawn/advance/linger/glitch simulation | rain core | YES |
| Diff-based renderer, phosphor afterglow, RLE, SGR cache | cosmic dragon engine | YES |
| Color resolution pipeline, climate shader (atmospheric ctx) | chroma dragon engine | YES |
| Ambient climate drift (deterministic luminance/saturation/hue modulation) | crystal dragon engine (climate half) | YES |
| Crystal dragon palette drift (rebuilds palette) | crystal dragon engine (drift half) | NO — forced off for determinism |
| HUD, intro, message overlay, terminal I/O interaction | interactive shell | NO — bench paths return before `run_interactive` |

---

## 2. PERF-1-Supreme — the full barrier audit (honest)

### 2.1 Barriers that NEVER apply in benchmark mode (verified by call-site trace)

| Barrier | Where it lives | Why bench is immune |
|---|---|---|
| Idle FPS reduction (×0.5) | `PowerManager::effective_fps` | Only called from `interactive/event_loop.rs` — bench drives `cloud.rain_at()` directly |
| Self-healer scene downgrade | `interactive/adaptive.rs` | Interactive-only |
| `perf_pressure` spawn clamp + glitch/vignette/phosphor/ghost gates | set only via `cloud.set_perf_pressure()` from the event loop | Bench leaves it at initial 0.0 — all gates stay OPEN (full workload) |
| `aggressive_throttle` steeper clamp | set only by the self-healer | Initial `false`, never set in bench |
| madvise reclaim + endurance health | interactive adaptive stack | Never constructed in bench path |
| xterm.js 30 FPS cap | main.rs tier-2 | Explicitly skipped in ALL bench modes (`!in_bench_mode &&`) |
| Dynamic default FPS | main.rs | Skipped when `args.benchmark` |
| Ghost events (cinematic event engine) | `ghost_events.rs` | Opt-in via `enable_events()` — only the interactive loop calls it |
| Anomaly zones spawn/apply | `rain.rs` | Gated `!self.bench_mode` (PERF-1, prior session) |
| Border-cross cosmetics + border spark | `rain.rs` / `spawn.rs` | Gated `!self.bench_mode` (Z-6, prior session) |
| Message overlay draw (8 heap allocs/frame) | `rain.rs` | Gated `!self.bench_mode` (Z-6, prior session) |
| Quantum ripple / border spark particles | `spawn.rs` | Input-driven (mouse clicks) — cannot fire in bench |

### 2.2 The two leftover cosmetics found by this audit (now fixed)

1. **Cinematic CRT vignette** (`apply_crt_vignette`) — its own documentation calls it
   "cinematic CRT vignette post-process": dims the top/bottom edge rows for a retro CRT
   look. O(dirty cells in the 2×`CRT_VIGNETTE_HEIGHT` band) per frame of pure cosmetic
   work. **Fix:** gated on `!self.bench_mode`.
2. **Emergent storytelling engine** (`StorytellingState::tick` / `active_effects`) —
   spawns "emotionally resonant moments" (LuminanceSwell / DensityPulse /
   TemporalDilation) that perturb spawn density, luminance and speed mid-run.
   Cinematic by design, workload variance by effect. **Fix:** the tick, the per-frame
   effects evaluation, and moment expiry are all skipped in bench mode;
   `emergent_effects` short-circuits to the zero-boost default.

### 2.3 power_dragon — honest verdict

`power_dragon = true` (the default) is a **protection stack for interactive mode**: idle
FPS reduction, perf_pressure spawn clamps, phosphor skip, glitch disable, vignette
disable, self-healer downgrade, madvise. In benchmark mode **none of it exists** — the
benchmark constructs the Cloud and calls `rain_at()` directly; no event loop, no
PowerManager, no pressure feeding. The CONFIG line `power_dragon: true` in a benchmark
report therefore describes your config, not an active throttle. This is now stated in
the report comments and in this audit. (If you want the same unlimited behavior
interactively, set `power_dragon = false` in config.toml — v50-beta.3 Option D.)

### 2.4 crystal_dragon — honest verdict

The bench entry points force `cloud.crystal_dragon = false` (palette drift OFF). This is
**not a throttle on the rain** — it stops palette *rebuilds*, which inject one-off timing
spikes that would corrupt p99/max determinism. The deterministic half of the crystal
engine (climate drift: luminance/saturation/hue modulation via the chroma climate
shader) **still runs** in bench. The `chroma_in_benchmark` CONFIG line discloses exactly
this. Verdict: real engine, honestly split — drift off for measurement integrity,
climate on because it is deterministic.

### 2.5 `max_sim_delta` stale-comment fix (honesty housekeeping)

The old comment claimed bench runs the droplet-advance loop with `max_sim_delta = 0`
(tight path). Reality: both bench entry points call `set_max_sim_delta(target_period)`,
so bench takes the cap path — but the cap is behaviorally inert under uniform bench
stepping (`last_time + target_period == now` for every droplet; the clamp never fires).
Comment corrected to describe actual behavior. No code-behavior change.

### 2.6 A/B measurement (release profile, no LTO, same container)

| Metric | Before (no gates) | After (vignette + storytelling gated) | Delta |
|---|---|---|---|
| avg_fps | 91,096.90 | 94,211.97 | **+3.4%** |
| median_fps | 94,786.73 | 97,318.87 | +2.6% |
| peak_fps | 124,579.54 | 126,742.71 | +1.7% |

The improvement is real but bounded — these were the last two cosmetic workloads left in
the bench hot path. Claiming more would be dishonest.

### 2.7 Lock tests (LTS)

- `bench_mode_storytelling_moments_stay_empty` — behavioral: 120 sim-seconds of bench
  stepping must never spawn an emergent moment or set a cooldown.
- `bench_cosmetics_gates_exist_in_rain_source` — structural: the `!self.bench_mode`
  guards around `apply_crt_vignette` and `storytelling.tick` must exist in rain.rs.

---

## 3. PERF-2-Supreme — CONFIG section completeness

### 3.1 Owner complaint

Benchmark output under CONFIG was missing state keys, e.g. `power_dragon`,
`crystal_dragon`, `no_effects true/false`. (power_dragon/crystal_dragon/msg_mode were
added by the immediately preceding session; this commit completes the set with
`no_effects` and extends the same keys to `--json`.)

### 3.2 Fix

Text report CONFIG section (and JSON `config` object) now include:

```text
power_dragon:    true   # config state (not a bench throttle — see §2.3)
crystal_dragon:  false  # config state (drift forced OFF in bench — see §2.4)
msg_mode:        true   # config state (messages never render in bench — Z-6)
no_effects:      false  # true when --no-effects set (particles are
                        # input-driven; never changes bench numbers)
```

`no_effects` naming follows the owner's requested semantics (true = effects OFF) and is
the inversion of the internal `effects_enabled` flag.

### 3.3 Honesty notes carried in the report itself

- `cosmetics_skipped` now lists the full set: message border + anomaly zones + CRT
  vignette + emergent storytelling (bench mode = rain + 3 dragons only).
- `chroma_in_benchmark` continues to disclose the crystal-drift force-off.

### 3.4 Verification

- Text: `cosmostrix --benchmark` shows all four keys; `--no-effects` flips
  `no_effects` to `true`.
- JSON: `cosmostrix --benchmark --json --no-effects` → `config.no_effects == true`
  plus `power_dragon` / `crystal_dragon` / `msg_mode` present.
- Tests: 1712 passed / 0 failed / 2 ignored (includes 2 new lock tests).

---

## 4. Files changed

- `src/cosmic_dragon_engine/cloud/rain.rs` — PERF-1 gates (vignette, storytelling ×2),
  stale max_sim_delta comment fix
- `src/cosmic_dragon_engine/cloud/tests/mod.rs` — 2 lock tests
- `src/cosmic_dragon_engine/cloud/mod.rs` — clippy doc-list fix (pre-existing, stricter
  clippy in 1.98)
- `src/bench/bench_config_enrichment.rs` — `no_effects` field + derivation
- `src/bench/bench_report.rs` — `no_effects` struct field + CONFIG render +
  cosmetics_skipped text
- `src/bench/bench_report_tests.rs` — test struct updated + field-count doc
- `src/bench/mod.rs` — wiring at both report construction sites
- `src/bench/bench_json.rs` — `power_dragon`/`crystal_dragon`/`msg_mode`/`no_effects`
  in JSON config section
- `CHANGELOG.md` — Unreleased entry

## 5. Lock status

- Cosmic Dragon: extended (bench-only gates in rain_at; interactive path identical)
- Chroma Dragon: untouched (climate shader still runs in bench)
- Crystal Dragon: untouched (drift already forced off in bench; climate still runs)
- Bench reporting: extended (additive fields only, no behavior change)

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
