<!-- SPDX-License-Identifier: GPL-3.0-only -->

# The Three Dragon Engines of cosmostrix v50

> v50.0.0-alpha.6 — 2026-08-19

cosmostrix runs three independent dragon engines, each owning a distinct
rendering concern. They never share mutable state; they communicate only
through the immutable `Cloud` snapshot each frame.

```
Cloud (frame state)

  COSMIC Dragon      CHROMA Dragon      CRYSTAL Dragon
  - simulation       - color            - palette
  - physics          - palette          - drift +
  - behavior         - OKLab            - ambient
```

## 1. Cosmic Dragon — `src/engine/cosmic_dragon_engine/`

The simulation core. Owns droplet lifecycle, spawn physics, atmospheric
evolution, cinematic behavior profiles, and the self-healer. Reads
palette colors produced by Chroma Dragon; never writes palette state.

Dragon Engine v2 additions: the self-healer is now predictive (EMA
trend with alpha 0.3 fires PreemptiveThrottle when pressure rises
>0.05/tick inside the warning zone — before the 30s reactive downgrade),
ghost events are pressure-scaled (ghosts as a living system health
indicator: frequent at calm, none near the perf gate), and phosphor
decay is adaptive (trails ~20% longer at idle, shorter under load —
"the rain breathes with your CPU").

## 2. Chroma Dragon — `src/engine/chroma_dragon_engine/`

The coloring engine. Owns palette construction (OKLab gradients since
v30), per-cell shader pipeline, climate post-FX (luminance/saturation/
hue drift), L-smoothing, and the 300 ms top-to-bottom wave transition.
Every color-change path (keypress, Crystal Dragon, scene runtime, live
reload) delegates to `set_color_scheme()` -> `apply_new_palette()` which
advances the circular buffer and activates the wave.

## 3. Crystal Dragon — `src/engine/crystal_dragon_engine/`

The ambient intelligence engine. Two subsystems working in harmony:

### 3a. Palette drift (CPU/CLOCK -> theme)

```
CPU% ──> point (1-99) ──> group ──> weighted theme selection
  │                          │
  │   1-33 = Cold (14)       │   calc-v2 (DEFAULT): CDF + recency
  │   34-66 = Medium (14)    │   crystal-dragon-secs cadence (60s default),
  │   67-99 = Hot (14)       │   deterministic boundary fire (HUNT-7),
  │                          │   dwell floor = min(60s, cadence) (HUNT-3)
  │                          │   calc-v1: legacy, no memory
  └── CPU unsupported? ──> CLOCK fallback (UTC hour -> point)
```

The builtin themes: roughly even Cold / Medium / Hot groups + reserved.
Low CPU -> Snow/Moon/Ocean (Cold). High CPU -> Sun/Fire/Red (Hot).
Transitions delegate to Chroma Dragon for smooth 300 ms OKLab waves.

### 3b. Ambient scheduler (time-of-day -> scene)

Time-of-day scene switches via `ambient.HH-MM = <scene>` in config.toml.
Fires at scheduled times, applies scene+palette. Crystal Dragon wins
(drift overrides the palette), but ambient snapback reverts after
`ambient-snapback-secs` of idle — the two systems cooperate, and since
v80.0.0-alpha.1 both timing knobs are tunable (keep snapback <
`crystal-dragon-secs` for a clean take-turns rhythm).

### File architecture

| File | Role |
|------|------|
| `crystal_dragon_control/mod.rs` | Config: polling (60s default — `crystal-dragon-secs` tunable, v80.0.0-alpha.1), calc-v2 (default) / calc-v1 (legacy), CPU/CLOCK mode |
| `sensor/mod.rs` | CPU sampling (sysinfo/procfs) + CLOCK fallback |
| `palette_groups/mod.rs` | The builtin themes -> Cold/Medium/Hot partition |
| `point_system/mod.rs` | calc-v2 (default): weighted CDF + DriftHistory recency ring buffer (8 entries, prevents A->B->A oscillation); calc-v1 (legacy): no-memory CDF |
| `ambient/mod.rs` | Schedule types, parsing, validation, startup apply |
| `ambient_scheduler/mod.rs` | Background thread: fire entries on schedule |
| `ambient_diag.rs` | Diagnostics counters (exit summary) |

## Lock status + commit history (v100 LTS)

All three dragons are LOCK-protocol engines: each engine's
`KEY.md` (simplified signature log) and `RULES.md` (full unlock
detail) record every lock/unlock round, signed oxyzenQ. Any commit
that touches a locked engine folder after its lock boundary MUST
carry an UNLOCK entry in the same commit (the `c1c7779` and
depthtest-3 retroactive entries document the failure mode when it
does not).

Current lock round (2026-09-12, locked tree `1007714`):

| Engine | Path | Status | Lock entry |
|---|---|---|---|
| Cosmic | `src/engine/cosmic_dragon_engine/` | LOCKED (hunter-34 unlock + re-lock: terminal shadow honesty) | `KEY.md` top |
| Chroma | `src/engine/chroma_dragon_engine/` | LOCKED (lock intact; retroactive depthtest-3 unlock noted) | `KEY.md` top |
| Crystal | `src/engine/crystal_dragon_engine/` | LOCKED (lock intact, zero commits since S-night-R8) | `KEY.md` top |

### The simple history method (owner request 2026-09-12)

One git command shows every commit that ever touched the dragon
engine folders:

```bash
git log --oneline -- \
  src/engine/chroma_dragon_engine \
  src/engine/cosmic_dragon_engine \
  src/engine/crystal_dragon_engine
```

The convenience wrapper (recommended — it also carries the lock
boundary and the audit view):

```bash
./scripts/dragon-history.sh                # full history, all engines (newest first)
./scripts/dragon-history.sh --since-lock   # engine commits since LOCK_AT — each needs an UNLOCK entry
./scripts/dragon-history.sh --per-engine   # per-engine last commit + count
./scripts/dragon-history.sh 9c36a049..HEAD # any commit range
```

Update `LOCK_AT` in `scripts/dragon-history.sh` every time a new
lock round is signed, so `--since-lock` stays the authoritative
audit trail for the frozen core.

---

*Rezky / oxyzenQ — 2026*
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
