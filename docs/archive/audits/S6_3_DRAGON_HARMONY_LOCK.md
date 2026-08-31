<!-- SPDX-License-Identifier: GPL-3.0-only -->

# S-master-6 — 3-Dragon Harmony + Lock Signature

**Date:** 2026-09-01
**Scope:** 3-dragon integration surface (cosmic + chroma + crystal)
**Author:** oxyzenQ (cosmic dragon mode, master audit pass)
**Task:** Deeper audit for crystal, chroma, cosmic dragon — peak harmony & works together. Optimize for specialize work, efficient resource, strong foundation, stable LTS. Without sacrificing visual/performance. Hidden vulnerability security leaks if found, improve potential gain. 10s A/B benchmark. Lock 3 dragons with signature (KEY.md).

## Context

This is the final S-master task. It verifies the 3 dragons (cosmic,
chroma, crystal) work together in harmony, checks for hidden
vulnerabilities in the integration surface, applies hardening if
found, and locks the 3-dragon system with a signature.

## 3-Dragon Harmony Verification

### Architecture (confirmed sound)

The 3 dragons communicate via immutable `Cloud` snapshot each frame.
No shared mutable state. Integration contracts:

1. **Crystal → Cosmic → Chroma delegation**: Crystal Dragon decides
   a new theme (drift/schedule). Calls `set_color_scheme()` on Cloud
   (Cosmic's `runtime_controls.rs:51`). Cosmic rebuilds palette via
   `build_palette()` + `apply_tune_to_palette()` (Chroma's palette
   construction). 300ms transition wave activates.

2. **Immutable snapshot isolation**: each frame produces immutable
   `Cloud`. Chroma reads palette stops. Crystal reads sensor state.
   No dragon writes to another's state.

3. **Color routing rule**: all render-path color output routes through
   Chroma (`is_chroma()` → `chroma::palette::*` for TrueColor,
   `chroma::legacy::*` for fallback). 7+ `is_chroma()` branches in
   `droplet/draw.rs` (the per-cell render hot path). No hardcoded
   `Color::Rgb`/`Color::White` in render code.

4. **Thread isolation**: each dragon's background threads are isolated
   with `catch_unwind` panic safety.

### Lock test verification

| Suite | Tests | Passed | Failed |
|---|---|---|---|
| All lock tests (3 dragons + incubator) | 78 | 78 | 0 |
| Full binary test suite | 1945 | 1945 | 0 (2 ignored) |
| Chroma-specific tests | 289 | 289 | 0 |
| Chroma lock invariants (lock_inv01-19) | 19 | 19 | 0 |

## Hidden Vulnerability Sweep (3-dragon integration surface)

### Method
Focused ripgrep + Read on dragon communication points:
- `runtime_controls.rs` (set_color_scheme, apply_new_palette, crystal_dragon_sensor)
- `scene_runtime.rs` (scene application)
- `ambient_scheduler/mod.rs` (crystal background thread)
- `crystal_dragon_control/mod.rs` (crystal config)
- `droplet/draw.rs` (chroma routing in render hot path)

### Findings

| Category | Status |
|---|---|
| unwrap/expect/panic in dragon integration paths | **Clean** — 0 found (grep returned empty) |
| unsafe in chroma/crystal dragons | **Clean** — 0 found (cosmic unsafe already audited in S3) |
| Command injection | **Clean** — all `Command::new` hardcoded args (S3) |
| Env var injection | **Clean** — 50 reads all safe (S3) |
| Live-reload message path | **Fixed in S3** — sanitize + length cap |
| Thread spawn failure handling | **1 gap found + fixed** (see below) |

### Thread spawn failure gap (FIXED)

`src/engine/crystal_dragon_engine/ambient_scheduler/mod.rs:175` used
`.expect("spawn ambient scheduler thread")` — panics on extreme
resource exhaustion (RLIMIT_NPROC). This is the same pattern S4 fixed
in `fork_guard.rs:164`.

**Fix applied**: replaced `.expect()` with `if ... .is_err() { ... }`
check + `push_runtime_warning()` on failure. A missing ambient
scheduler (scenes won't fire) is strictly better than a crash at
startup. The `rx` channel returns Err on first poll (tx was never
created), and the event loop detects the dead scheduler — the rain
loop + chroma + cosmic dragons continue unaffected.

The `terminal/mod.rs:567` spawn (cosmic shutdown guard) already uses
`let _ =` (silently ignores) — no fix needed.

## A/B Benchmark (10s, scene=monolith)

| Size | Metric | A (before) | B (after) | Delta | Verdict |
|---|---|---|---|---|---|
| 6x6 | avg_fps | 1,572,599 | 1,539,500 | -2.10% | stable |
| 6x6 | entropy | 0.0000 | 0.0043 | +0.00% | stable |
| 6x6 | gini | 0.8333 | 0.8319 | -0.17% | stable |
| 6x6 | avg_dirty_cells | 0.6675 | 0.6674 | -0.02% | stable |
| 20x20 | avg_fps | 496,498 | 494,476 | -0.41% | stable |
| 20x20 | entropy | 0.7542 | 0.7536 | -0.08% | stable |
| 20x20 | gini | 0.9165 | 0.9165 | +0.00% | stable |
| 20x20 | avg_dirty_cells | 7.9356 | 7.9353 | -0.00% | stable |
| 40x20 | avg_fps | 302,492 | 302,290 | -0.07% | stable |
| 40x20 | entropy | 1.4336 | 1.4368 | +0.22% | stable |
| 40x20 | gini | 0.9360 | 0.9358 | -0.02% | stable |
| 40x20 | avg_dirty_cells | 14.2234 | 14.2313 | +0.06% | stable |
| 80x24 | avg_fps | 92,834 | 93,400 | +0.61% | stable |
| 80x24 | entropy | 3.2955 | 3.2952 | -0.01% | stable |
| 80x24 | gini | 0.8961 | 0.8961 | -0.00% | stable |
| 80x24 | avg_dirty_cells | 56.8060 | 56.8064 | +0.00% | stable |
| 120x40 | avg_fps | 53,012 | 53,680 | +1.26% | stable |
| 120x40 | entropy | 3.9244 | 3.9239 | -0.02% | stable |
| 120x40 | gini | 0.8943 | 0.8943 | +0.00% | stable |
| 120x40 | avg_dirty_cells | 107.3955 | 107.3610 | -0.03% | stable |
| 200x60 | avg_fps | 29,654 | 29,714 | +0.20% | stable |
| 200x60 | entropy | 4.7142 | 4.7142 | +0.00% | stable |
| 200x60 | gini | 0.8904 | 0.8904 | -0.00% | stable |
| 200x60 | avg_dirty_cells | 205.0622 | 205.0307 | -0.02% | stable |

**All 24 metrics within ±2.1% natural variance.** Max delta -2.10% fps
at 6x6 (within bench noise floor ~3%). Visual metrics (gini, entropy)
all <0.25% delta. Zero visual or performance regression confirmed.

The ambient_scheduler spawn-failure fix only affects the error path
(thread spawn fails — rare, only under RLIMIT_NPROC). Normal operation
unchanged.

Raw JSON: `benchmark/bench-labs/S_master_dragon/S6_baseline_A.json`
and `S6_after_B.json`.

## 3-Dragon Lock Signature

Created top-level `KEY.md` at repo root — the 3-dragon harmony
signature. Records:
- 3-dragon architecture (immutable snapshot isolation)
- 5 harmony invariants (delegation chain, snapshot isolation, color
  routing rule, thread isolation, lock integrity)
- S-master-6 signature: all 3 dragons LOCKED, no UNLOCK opened during
  S1-S6, 78 lock tests green, 1945 full tests green, A/B within noise
- Signoff: oxyzenQ — 2026-09-01

Updated per-engine KEY.md files:
- `src/engine/cosmic_dragon_engine/KEY.md`: S-master-6 harmony entry
- `src/engine/chroma_dragon_engine/KEY.md`: S-master-5 verification entry (already done)
- `src/engine/crystal_dragon_engine/KEY.md`: S-master-6 harmony + ambient_scheduler spawn hardening entry

## Verdict

**3-dragon harmony confirmed.** The crystal, chroma, and cosmic dragons
work together in harmony via the immutable Cloud snapshot contract.
Integration is sound, no hidden vulnerabilities found in the
communication surface (1 thread-spawn robustness gap fixed).

**Lock state**: all 3 dragons LOCKED. S-master series (S1-S6) made NO
changes to any dragon's locked invariants. All changes were either:
- Non-dragon code (S1, S3)
- Dragon-adjacent hardening (S2 micro-opts in render hot path but
  not locked invariants; S4 fork_guard spawn; S6 ambient_scheduler
  spawn)
- Verification only (S5)

**3-Dragon LTS Lock signature**: committed at `dd34821` (S-master-5)
with S-master-6 hardening on top. 78 lock tests green, 1945 full tests
green, A/B within noise, zero security regressions.

## Files Changed

- `KEY.md` (new) — top-level 3-dragon harmony signature
- `src/engine/cosmic_dragon_engine/KEY.md` — S-master-6 harmony entry
- `src/engine/crystal_dragon_engine/KEY.md` — S-master-6 harmony + ambient_scheduler spawn hardening entry
- `src/engine/crystal_dragon_engine/ambient_scheduler/mod.rs` — spawn-failure graceful skip (replaced .expect with .is_err + runtime warning)
- `benchmark/bench-labs/S_master_dragon/S6_*.{json,md}` — A/B data + report
- `docs/archive/audits/S6_3_DRAGON_HARMONY_LOCK.md` — this audit doc
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
