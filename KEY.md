<!-- SPDX-License-Identifier: GPL-3.0-only -->

# cosmostrix 3-Dragon Signature Lock — LTS KEY

> This file is the top-level 3-dragon harmony signature. It locks
> the integrated state of all three dragon engines as a single
> production LTS unit. For per-engine lock detail, see:
> - [Cosmic Dragon KEY](src/engine/cosmic_dragon_engine/KEY.md)
> - [Chroma Dragon KEY](src/engine/chroma_dragon_engine/KEY.md)
> - [Crystal Dragon KEY](src/engine/crystal_dragon_engine/KEY.md)

## 3-Dragon Architecture

```
Cloud (immutable per-frame snapshot)

  COSMIC Dragon      CHROMA Dragon      CRYSTAL Dragon
  - simulation       - color            - palette drift
  - physics          - OKLab            - ambient scheduler
  - behavior         - palette          - sensor (CPU/Clock)
```

The three dragons never share mutable state. They communicate only
through the immutable `Cloud` snapshot each frame:

- **Cosmic Dragon** (`src/engine/cosmic_dragon_engine/`): owns droplet
  lifecycle, spawn physics, atmospheric evolution, cinematic behavior,
  self-healer. Reads palette colors produced by Chroma Dragon.
- **Chroma Dragon** (`src/engine/chroma_dragon_engine/`): owns palette
  construction (OKLab gradients), per-cell shader pipeline, climate
  post-FX, L-smoothing, 300ms transition wave. All color output routes
  through here (Chroma Dragon Routing Rule).
- **Crystal Dragon** (`src/engine/crystal_dragon_engine/`): owns
  ambient intelligence — CPU/clock sensor → point (1-99) → theme
  selection (44 themes: 14 Cold + 14 Medium + 14 Hot + 2 Reserved).
  Triggers `set_color_scheme()` → `apply_new_palette()` which delegates
  to Chroma Dragon for smooth OKLab wave transitions.

## Harmony Invariants

The 3 dragons work together via these integration contracts:

1. **Crystal → Cosmic → Chroma delegation**: Crystal Dragon decides a
   new theme is needed (drift/schedule). It calls `set_color_scheme()`
   on Cloud (Cosmic Dragon's runtime_controls). Cosmic Dragon rebuilds
   the palette via `build_palette()` + `apply_tune_to_palette()` (Chroma
   Dragon's palette construction). The palette transition wave (300ms
   top-to-bottom cascade) activates. No user-controlled strings reach
   palette construction — `ColorScheme` is a typed enum.

2. **Immutable snapshot isolation**: each frame, Cosmic Dragon produces
   an immutable `Cloud` snapshot. Chroma Dragon reads palette stops from
   it. Crystal Dragon reads sensor state from it. No dragon writes to
   another dragon's state — communication is read-only via snapshot.

3. **Color routing rule**: all color output in render paths MUST route
   through Chroma Dragon (`is_chroma()` → `chroma::palette::*` for
   TrueColor, `chroma::legacy::*` for fallback). No hardcoded
   `Color::Rgb` or `Color::White` in render code. Exception: diagnostic
   output (`--doctor`, `--benchmark`, verbose stderr).

4. **Thread isolation**: each dragon's background threads are isolated:
   - Cosmic: `cx-shutdown-guard` (terminal restore watchdog)
   - Chroma: no background threads (synchronous color math)
   - Crystal: `ambient-scheduler` (time-of-day scene switches) +
     `cx-term-guard` (fork guard, macOS/BSD only)
   All use `catch_unwind` for panic safety + graceful degradation on
   spawn failure (S4/S6 harden: missing thread → runtime warning, not
   crash).

5. **Lock integrity**: each dragon has its own KEY.md with lock
   invariants (Cosmic: 17, Chroma: 19, Crystal: documented in
   `c1c7779` re-seal). S-master-6 did NOT unlock any dragon — all
   changes were additive hardening (ambient scheduler spawn-failure
   graceful skip).

## S-master-6 Signature (2026-09-01)

**Lock state**: all 3 dragons LOCKED. No UNLOCK opened during S-master
series (S1-S6). All changes were either:
- Non-dragon code (S1: dead code/stale comments in cli/config/tests;
  S3: live_config message sanitize; S4: fork_guard spawn robustness)
- Dragon-adjacent hardening (S6: ambient_scheduler spawn-failure
  graceful skip — matches S4 fork_guard pattern, no invariant change)
- Verification only (S5: chroma integrated verify, no code change)

**Test verification**:
- 78 lock tests across all 3 dragons pass (0 fail)
- 1945 full binary tests pass (0 fail, 2 ignored)
- 289 chroma-specific tests pass
- 19 chroma lock invariants pass (lock_inv01-19)
- clippy clean, gate-keepers 8/8

**A/B benchmark** (10s, scene=monolith, 6 sizes × 4 metrics = 24 data
points): all within ±2.1% natural variance. Max delta -2.10% fps at
6x6 (within bench noise floor ~3%). Visual metrics (gini, entropy)
all <0.25% delta. Zero visual or performance regression.

**Security verification** (S3 + S6):
- No unwrap/expect/panic in dragon integration paths
- No unsafe in chroma or crystal dragons (cosmic unsafe already audited)
- No command injection (all Command::new hardcoded args)
- No env var injection (50 reads, all safe)
- Live-reload message path sanitized + length-capped (S3 fix)
- All thread spawns gracefully degrade on failure (S4/S6 fixes)

**3-Dragon Harmony Verdict**: the three dragons work together in
harmony. Integration contracts are sound. No hidden vulnerabilities
found in the communication surface. Each dragon is independently
locked with its own invariants; the integrated system is stable
production LTS.

## Signature

> **3-Dragon LTS Lock** — committed at `dd34821` (S-master-5) with
> S-master-6 hardening on top. All 3 dragons confirmed in harmony:
> Crystal decides, Cosmic delegates, Chroma colors. 78 lock tests
> green, 1945 full tests green, A/B within noise, zero security
> regressions.
>
> Signoff: **oxyzenQ** — 2026-09-01 — S-master-6 3-dragon harmony lock
> (additive hardening only, no dragon unlocked)

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
