# Changelog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

cosmostrix uses [SemVer](https://semver.org/). Git tags use a leading `v` (e.g. `v50.0.0`).

Pre-v13 history is archived in [`docs/archive/CHANGELOG_PRE_V13.md`](docs/archive/CHANGELOG_PRE_V13.md). The summary below covers the full journey from the first public release to the current beta, condensed so users can follow the evolution without wading through per-release minutiae.

---

## v50.0.0-rc.1 — Verbose Exit Time + Duration (Current RC)

cosmostrix v50.0.0-rc.1 — verbose exit summary now shows the local exit time and total run duration. Owner-identified gap: `cosmostrix -v` previously had no way to tell the user how long the process ran or at what wall-clock time it exited. The "final runtime state" section now always prints this information as its first content line, even when no live-reload field changed during the session.

### What's new since beta.5

- **Verbose exit time + duration** (this RC): the `cosmostrix -v` / `--verbose` post-exit "final runtime state" section now leads with an `exit_time:` + `duration:` line. `exit_time` is the local wall-clock at the moment of exit, formatted as `YYYY-MM-DD HH:MM:SS ±HH:MM` (timezone-aware via POSIX `tm_gmtoff`). `duration` is the total process lifetime measured from the `Instant` captured at the top of `main()`, formatted as `Xm Ys` / `Xh Ym Ys` for human scanning. The section previously early-returned when no live-reload field changed; it now always prints so the user can see how long cosmostrix ran regardless of session activity.
- **Clock module extended**: `LocalTm` struct in `src/clock/posix_time.rs` gained `year`, `month`, `day`, `gmtoff` fields (previously only `hour`/`minute`/`second`/`yday`). This avoids a second FFI round-trip when formatting the full local datetime stamp. Existing callers are unaffected — they continue to read only the fields they need.
- **New clock helpers**: `clock::now_local_datetime()` (formats `YYYY-MM-DD HH:MM:SS ±HH:MM`) and `clock::format_duration_compact()` (formats `Duration` as `1m 52s` / `1h 5m 3s`). Both are pure functions with no global state, fully unit-tested with canonical cases including the Asia/Jakarta UTC+7 example from the owner's spec.
- **5 new unit tests + 1 extended test** in `src/clock/mod.rs` and `src/clock/posix_time.rs`: `now_local_datetime_format`, `now_local_datetime_is_ascii`, `format_gmtoff_canonical_cases` (5 cases: +07:00, +00:00, -05:00, +05:30, -05:45), `format_duration_compact_canonical_cases` (6 cases: 0s/45s/1m52s/59m59s/1h0m0s/1h5m3s), `format_duration_compact_drops_subsecond`, and extended `local_tm_fields_bounded` covering the new Y/M/D/gmtoff fields. Total test count: 1687 passed / 0 failed / 2 ignored.

### Sample output

```text
[verbose] [01:29] final runtime state
[verbose] [01:29]   exit_time:     2026-08-26 01:29:20 +07:00 | duration: 1m 52s
[verbose] [01:29]   density:       0.66 (was 0.75)
[verbose] [01:29]   crystal_dragon: false (was true)
[verbose] [01:29]   ambient_diag: startup=0 rx=0 reapply=0 snapback=0 cfg_rebuilds=1 sked_reloads=0 sked_empties=0 consistency_fixes=0 snapback_killed=0 snapback_guard_sked_len=0 snapback_guard_last_applied=0 last_scene_change=none
```

When no live-reload field changed during the session, the section still prints the header + `exit_time`/`duration` line + `ambient_diag` line, so the user always sees how long cosmostrix ran.

### Files changed

- `src/clock/posix_time.rs` — `LocalTm` struct extended (year/month/day/gmtoff); `local_tm()` unix path reads `tm_gmtoff` directly; non-unix fallback derives Y/M/D from civil-from-days algorithm (gmtoff stays 0); tests extended
- `src/clock/mod.rs` — new `now_local_datetime()`, `format_gmtoff()`, `format_duration_compact()`; 6 new unit tests
- `src/interactive/mod.rs` — `print_final_runtime_state()` accepts `start_time: Instant`; removed `if !changed { return; }` early-exit; always prints `exit_time` + `duration` as first content line
- `src/main.rs` — captures `start_time: Instant::now()` at top of `main()`; passes it to `print_final_runtime_state`
- `src/cli/help_detail.rs` — `-v, --verbose` help text mentions the exit time + duration summary
- `docs/RULES.md` — verbose output section updated to describe the always-print behavior + new exit_time/duration line
- `CHANGELOG.md` — this entry

### Design notes

- **Why `Instant` not `SystemTime` for duration**: `Instant` is monotonic — NTP jumps, manual `date` changes, or DST transitions cannot make the duration negative or jump. `SystemTime::now().duration_since(start)` can panic on clock rollback; `Instant::elapsed()` is always sound.
- **Why local wall-clock for `exit_time`**: the user runs cosmostrix interactively in their local timezone. Showing UTC would force a mental conversion; showing local time matches what `date(1)` prints and what the user expects.
- **Why `±HH:MM` not `±HHMM` or named zone**: `±HH:MM` is the form `date(1)` and `timedatectl` use, instantly scannable. A named zone (e.g. `WIB`) requires a tzdata lookup and is ambiguous (CST = China/US Central/Cuba).
- **Why always print (not conditional on `changed`)**: the owner's explicit ask was "user can see how long cosmostrix run if user using verbose mode". Suppressing the section when nothing changed would hide the duration — defeating the feature's purpose. The per-field `if final_X != startup_X` guards still suppress unchanged fields, keeping the section scannable.

### Lock status

- Cosmic Dragon: untouched (no cosmic paths modified)
- Chroma Dragon: untouched (no chroma paths modified)
- Crystal Dragon: untouched (no crystal paths modified)
- Clock subsystem: extended (additive change — new fields, new functions, no behavior change to existing callers)

---

## v50.0.0-beta.5 — Exp Decay Easing Consolidation (Current Beta)

cosmostrix v50.0.0-beta.5 — masterclass easing consolidation. All **temporal** easing in the rain simulation now uses the unified **exponential decay** family. Owner-approved, owner-verified feel. 227 source files, ~89K LOC, ~1500+ tests pass (1656/0/2 — 4 new regression tests added).

### What's new since beta.4

- **Pause/resume → exp decay** (commit `e2e0512`): replaced the prior smootherstep S-curve (6t⁵-15t⁴+10t³ over fixed 0.30s decel / 0.45s resume) with asymmetric exponential decay — `exp(-k·t)` decel (k=1.2/s, settle 5% @ ~2.5s) + `1 - exp(-k·t)` accel (k=0.9/s, settle 95% @ ~3.3s). The asymmetric k_decel > k_resume preserves the prior "pause snappy / resume wake-up" feel. Settle thresholds snap to clean terminal state so other subsystems (spawn_remainder reset, monolith stream shift, phosphor LUT) see unambiguous transitions. Restores the README's previously-stale "exponential deceleration (~3s coast-down)" promise (smootherstep is not exponential — the README was wrong under the prior implementation).
- **Glyph scene entry → exp decay** (this beta): migrated the scene-entry ramp from smoothstep (3t²-2t³ over 700ms) to the same exp approach family — `1 - exp(-k·t)` with k=4.28/s (derived so settle 95% lands at the documented 700ms). Now all temporal easing in the rain path uses the same physical-drag model — pause, resume, and scene entry all coast under the same math primitive. exp() was already in use in the cosmic locked path (`cloud/phosphor.rs:307` LUT build) and chroma shaders/base LUT (`shaders/base/mod.rs:237`), so no new math primitive introduced.
- **Defensive invariant** (`debug_assert!` in `rain_at`): pause_start and resume_start cannot coexist — toggle_pause() guarantees this across all 3 branches (start-decel / abort-decel / unpause-from-paused), now asserted at the rain entry point. Zero-cost in release builds.
- **4 new regression tests** in `cloud/tests/mod.rs`: pause decel settle at 5% threshold, resume accel settle at 95% threshold, glyph entry ramp settle at 700ms + k derivation sanity-check, and the audit §8.6 invariant (pause_start + resume_start never coexist across all 3 toggle branches). Locks the masterclass easing contract — any future regression to a different curve or threshold fails CI.
- **Unified easing design doc** in `central_control_rains/mod.rs`: a new "Easing family policy" section documents which easings are exp decay (pause/resume + glyph entry) vs smoothstep (spatial fades — edge fade, vignette, brightness bands) vs intentional smoothstep-shaped rate (profile interpolation's 30s slow-drift morph) vs linear (chroma 3-row color transition falloff). Prevents future contributors from "consolidating" the wrong easings and breaking the intentional design.

### Files changed

- `src/cosmic_dragon_engine/cloud/rain.rs` — decel + accel + glyph entry ramp blocks; new `debug_assert!`; comment updates
- `src/cosmic_dragon_engine/cloud/tests/mod.rs` — 4 new regression tests + 1 existing test comment/duration bump
- `src/cosmic_dragon_engine/cloud/spawn.rs` — doc-comment updates for the new glyph entry ramp math
- `src/central_control_rains/mod.rs` — new glyph entry constants block + unified easing policy doc section
- `README.md` — pause/resume bullet expanded to mention unified family + glyph entry
- `CHANGELOG.md` — this entry
- `src/cosmic_dragon_engine/KEY.md` + `RULES.md` — UNLOCK entry (rain.rs + spawn.rs + tests are locked path)

### What is NOT exp decay (intentionally, documented)

- **Spatial fades** (edge fade, vignette, brightness bands) stay smoothstep — they're position-based, not time-based. The "blend" parameter is a cell's row/col, not elapsed time.
- **Profile interpolation** (30s slow-drift morph) keeps the smoothstep-shaped per-frame lerp rate — its "slow drift then accelerate then snap" feel is intentionally different from exp approach's "fast start then settle" feel.
- **Chroma color transition falloff** (3-row spatial window) stays linear — smoothstep was deliberately rejected as overkill.
- **Intro logo Phase 3 fade** stays smoothstep — intro animation, not pause/resume lifecycle.

### Lock status

- Cosmic Dragon: re-locked after this commit (UNLOCK entry in `cosmic_dragon_engine/KEY.md` + `RULES.md`)
- Chroma Dragon: untouched (no chroma paths modified)
- Crystal Dragon: untouched

---

## v50.0.0-beta.4 — Three Dragon Engines

cosmostrix v50.0.0-beta.4 — production-LTS-grade stability after full audit pass. 226 source files, ~89K LOC, ~1500+ tests pass. All 3 dragon engines locked with A/B benchmark signature.

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

cosmostrix v50 is the "zero to hero" culmination — from a simple terminal rain demo to a professional-grade cinematic renderer with three independent dragon engines, each owning a distinct concern. 220+ source files, ~89K LOC, ~1500+ tests pass.

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

`q` quit · `Space` reset animation + restart message typewriter · `c`/`C` cycle colors · `s`/`S` cycle charsets · `x` cycle scene forward (`X` no-op) · `p` pause/resume · `i` toggle HUD (`I` no-op) · `[`/`]` adjust density · `Up`/`Down` adjust speed

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

The "real renderer" era. cosmostrix found its identity here.

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
