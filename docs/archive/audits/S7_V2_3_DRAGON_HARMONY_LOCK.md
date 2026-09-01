<!-- SPDX-License-Identifier: GPL-3.0-only -->

# S-master-7-v2 — 3-Dragon Harmony Audit + Lock Signature (LTS, deeper)

**Date:** 2026-09-01
**Scope:** crystal + chroma + cosmic 3-dragon integration surface
**Author:** oxyzenQ (cosmic dragon mode, master audit pass v2)
**Task:** Deeper audit that the 3 dragons reach peak harmony and work
together; specialized, efficient, strong-foundation, stable LTS; no
visual/performance downgrade; hidden vulnerabilities if found;
potential gain if possible; 10s A/B; lock with signature.
**Predecessor:** v1 3-dragon harmony lock at `1dd2ce2`
(docs/archive/audits/S6_3_DRAGON_HARMONY_LOCK.md).

## What v2 adds over v1

v1 verified the harmony architecture (immutable snapshot, delegation
chain, lock tests, 6-size A/B) and fixed one thread-spawn gap. v2
re-verifies the chain AT HEAD — necessary because S-master-1-v2
(055a69f) subsequently rewired the crystal→cosmic control surface
(drift_chance/cpu_ema_alpha became runtime fields) and d55442d's
calc-v2 became the default — then adds dynamic 3-dragon execution
proof and a stale-data purge of the crystal dragon doc.

## 1. Delegation chain re-verified at HEAD (post-S-master-1 wiring)

Read end to end at `bf6ef18`:

1. **Crystal decision** — `runtime_controls.rs:321
   crystal_dragon_tick`: polling gate (60 s), dwell hysteresis (60 s),
   probabilistic gate reading `control.drift_chance` (the FIELD, not
   the const — S-master-1-v2 S1-2 wiring live), theme selection via
   `calc_v2_select` (default) with 8-entry `DriftHistory` recency
   ring, or `calc_v1_select` (legacy, test-constructed only).
2. **Cosmic hand-off** — `runtime_controls.rs:51 set_color_scheme`:
   scheme store, `custom_palette_active` clear (Color-#1 contract),
   `build_palette` + `apply_tune_to_palette` re-apply (v30 Bug-#5
   contract — tune survives drift).
3. **Chroma execution** — `apply_new_palette`: palette slot rotation
   (cross-fade buffer), color-map regeneration, column slot reset,
   transition-wave timing (300 ms cascade + TransitionLTable
   L-smoothing).
4. **Drift lifecycle** — `post_rain.rs:181` gate +
   Z-master-1X round-2 60 s drift-cycle self-reset; ambient
   snapback path clears the same flags — no deadlock between the
   two reset paths.

Chain intact and improved since v1. The typed `ColorScheme` enum
crosses every boundary — no user-controlled string reaches palette
construction.

## 2. Cosmic v2 features confirmed live at HEAD

- Predictive self-heal: `SelfHealAction::PreemptiveThrottle`
  (self_healer/mod.rs:90, handled at event_loop_self_heal.rs:116),
  EMA-slope spike prediction with the 8617360 `reset()` fix that
  preserves v2 state across scene switches.
- Pressure-scaled ghost spawn (ghost_events.rs, v2 chance shaping).
- Adaptive phosphor: `0.8 + (perf_pressure / 0.30) * 0.2` ramp
  (phosphor.rs:340) — calm 0.8x decay, full 1.0x at the perf gate.

## 3. Dynamic 3-dragon execution proof

10s monolith 80x24 dry, `--crystal-dragon true --color-mode 24` —
all three dragons active in one process: crystal sensor + drift
machinery armed, chroma truecolor pipeline flowing
(color_transition_delta 94.76, entropy 4.2132), cosmic simulation +
self-heal steady. 62326 fps, stability excellent, 565 allocs
bit-stable. Drift correctly does NOT fire in a 10 s window (60 s
dwell hysteresis — by design, the short-run quietness IS the
harmony contract working).

## 4. Stale-data purge (the real fix of this pass)

`docs/CRYSTAL_DRAGON_ENGINE.md` was stale since `d55442d`: it still
described **calc-v1 as active** and **calc-v2 as "NOT YET
IMPLEMENTED — reserved"** — the exact mirror of the code-comment bug
S-master-1-v2 fixed at 055a69f (which missed this doc). Corrected
(10 sections):

| Section | Before (stale) | After (current) |
|---|---|---|
| Subsystem map | point_system "calc-v1 weighted selection", 126 LOC | calc-v2 default + legacy, 268 LOC; all LOC counts refreshed (137/300/524/506/129/75) |
| Struct code block | `calc_method, // Calc (calc-v1)` | `// CalcV2 (calc-v2, default)` |
| Enum code block | `Calc, (active)` / `CalcV2, (NOT YET IMPLEMENTED — reserved)` | `Calc, legacy (test-constructed)` / `CalcV2, implemented + default since Dragon Engine v2` |
| Decision table | "calc-v1 ... calc-v2 reserved for future release" | "calc-v2 (DriftHistory, default) ... calc-v1 retained for A/B" |
| §6 heading intro | implied production algorithm | legacy framing + calc-v2 recency-ring description |
| Test table | "calc-v1 distribution properties" | calc-v2 DriftHistory + calc-v1 parity |
| Constants table | const-as-truth | drift_chance/cpu_ema_alpha FIELD is runtime source of truth (S-master-1 wiring noted) |
| File layout | 12 stale LOC values, total 2,738 | actual 3,820 total |

THREE_DRAGON_ENGINES.md and COSMIC_DRAGON_ARCHITECTURE.md checked —
already current (synced at 8617360).

## 5. A/B benchmark (10s, monolith 80x24, standard protocol)

Control pair — A at 22b9417 (pre-series baseline), B at bf6ef18
(docs-only diff; binary source byte-identical):

| Metric | A | B | Delta |
|---|---|---|---|
| avg_fps | 93068.98 | 93353.03 | +0.31% (noise) |
| frame_entropy_bits | 3.2953 | 3.2941 | -0.04% |
| density_gini | 0.8961 | 0.8961 | +0.00% |
| dirty_cells_per_frame | 56.76 | 56.75 | -0.01% |
| active_streams_avg | 23 | 23 | 0 |
| alloc_calls | 563 | 563 | 0.00% (bit-stable) |
| dealloc_calls | 553 | 553 | 0.00% (bit-stable) |
| total_ns_per_cell | 189.32 | 189.39 | +0.04% |

Visual bit-parity, allocator counts bit-stable. Raw JSON:
benchmark/bench-labs/S_master_v2_v2/S7_after_control.json (+
baseline_S4.json for A).

## 6. Test verification at HEAD

81 lock tests (0 fail) — chroma lock_inv01-19, cosmic 17, crystal
locks, incubator locks. 1995 full binary tests (0 fail, 2 ignored).
289 chroma-filtered tests. clippy clean. gate-keepers 8/8.

## Verdict

**3-dragon harmony confirmed at HEAD — deeper than v1.** The chain
survived the S-master-1 control-field rewiring and the v2
calc-method flip without losing a single contract; all three
dragons run together dynamically; every dragon remains LOCKED. The
one real defect found this pass was documentation staleness (the
crystal doc contradicting the shipped engine) — fixed. No source
changes; nothing to unlock; no over-engineering.

## Files Changed

- `docs/CRYSTAL_DRAGON_ENGINE.md` — 10 stale sections corrected.
- `KEY.md` (top level) — S-master-7-v2 signature section.
- `src/engine/crystal_dragon_engine/KEY.md`,
  `src/engine/cosmic_dragon_engine/KEY.md`,
  `src/engine/chroma_dragon_engine/KEY.md` — S-master-7-v2 harmony
  entries (one per dragon leg).
- `CHANGELOG.md` — Unreleased entry.
- `benchmark/bench-labs/S_master_v2_v2/S7_after_control.json` —
  raw control benchmark JSON.
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
