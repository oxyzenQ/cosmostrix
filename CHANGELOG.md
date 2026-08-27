# Changelog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

cosmostrix uses [SemVer](https://semver.org/). Git tags use a leading `v` (e.g. `v50.0.0`).

Pre-v13 history is archived in [`docs/archive/CHANGELOG_PRE_V13.md`](docs/archive/CHANGELOG_PRE_V13.md). The summary below covers the full journey from the first public release to the current beta, condensed so users can follow the evolution without wading through per-release minutiae.

---

## Unreleased

### Bug Fixes

- **Live-reload of `message` / `message-border` now reverts to default when commented out**: previously, when config.toml had `message = "hey"` at startup and the user commented it out (`# message = "hey"`), the renderer kept showing the stale "hey" instead of reverting to the default `"Experience a masterpiece with cosmostrix v{}"`. Root cause: `rebuild_cloud_config` in `src/config/live_config/mod.rs` preserved `base.message` from `base.clone()` when no config key was present — a stale-value carryover. The else branch now resets to `default_message_text()` + border, mirroring the startup fallback at `main.rs:1239-1258` and following the same "reset-on-comment" pattern as `color.tune` (Limitation C in `docs/LIVE_RELOAD_BEHAVIOR.md`, fixed in v50.0.0-alpha.7). Two lock tests added: `live_reload_no_config_message_reverts_to_default` and `live_reload_no_config_message_clears_when_msg_mode_false`.

### Docs

- **Ambient scheduler + Crystal Dragon interaction documented**: `docs/AMBIENT_SCHEDULER.md` now has a dedicated section explaining the 30-second auto-snapback behavior (user keypress overrides via `x`/`c`/`s` revert to the ambient phase after 30s of keyboard idle), the `cinematic`/`monolith` shared-color gotcha (pressing `x` from `monolith` may show no visible color change because both default to `neon-purple`), and a summary table of override behavior. This is the documentation baseline — the owner is reviewing options for a future config-tunable snapback delay (see `docs/archive/audits/AMBIENT_SCHEDULER_AUDIT.md` §3 for the deferred enhancement).

### Performance

- **PERF-1-Supreme: benchmark mode = critical path only**: the two last cosmetic workloads still running during `--benchmark` measurement frames are now gated on `!bench_mode`: (1) the cinematic CRT vignette post-process (dims top/bottom edge rows — pure retro-CRT look, zero critical-path value) and (2) the emergent storytelling engine (LuminanceSwell / DensityPulse / TemporalDilation "moments" that perturb spawn density, luminance and speed mid-run). Benchmark mode now measures exactly the rain simulation + the 3 dragon engines (cosmic render, chroma color, crystal climate) with no barriers: every power-management system (idle FPS throttle, self-healer, perf_pressure clamps, aggressive throttle, madvise, xterm.js cap) is interactive-only and never engages in bench paths — verified by call-site trace, documented in `docs/audits/PERF_SUPREME_bench_max_power_config_keys.md`. Measured A/B (release profile, 5 s run): avg_fps 91,096.90 → 94,211.97 (+3.4%). Two lock tests (`bench_mode_storytelling_moments_stay_empty`, `bench_cosmetics_gates_exist_in_rain_source`) prevent future refactors from silently reintroducing cosmetic work into the bench hot path.
- **Stale comment fix (honesty)**: the droplet-advance loop comment claimed bench runs with `max_sim_delta = 0` (tight path). Reality: both bench entry points set `max_sim_delta = target_period`, so bench takes the cap path — behaviorally inert under uniform bench stepping (the clamp never fires), but the comment now describes actual behavior.

### Features

- **`ambient-snapback-secs` config key (Option A — config-tunable snapback delay)**: the 30-second auto-snapback delay (which reverts user `x`/`c`/`s` overrides to the ambient phase after keyboard idle) is now configurable via `ambient-snapback-secs` in `config.toml`. Range `0.0..=86400.0` (0 = instant, 86400 = 24h = effectively disabled). Default 30s when unset (preserves existing behavior — no breaking change). The key is live-reloadable; editing it takes effect on the next frame. Closes the deferred enhancement listed in `docs/archive/audits/AMBIENT_SCHEDULER_AUDIT.md` §3. 5 lock tests added: `live_reload_ambient_snapback_secs_from_config`, `live_reload_ambient_snapback_secs_defaults_none_when_unset`, `live_reload_ambient_snapback_secs_invalid_falls_back_to_none`, `live_reload_ambient_snapback_secs_zero_is_valid`, `live_reload_ambient_snapback_secs_86400_is_valid`.
- **`--no-effects` CLI flag (rename + strengthening)**: renamed from `--disable-effects` to `--no-effects` for CLI ergonomics — mirrors the established `--no-*` convention (`--no-color`, `--no-border`). Typing the old `--disable-effects` now triggers clap's built-in "did you mean?" hint (the `suggestions` clap feature was added in `Cargo.toml`). Coverage strengthened from "quantum ripple + border spark" to **ALL** particle subsystems: quantum ripple, border spark, mouse-click flash waves (dual-ring expanding rings), and anomaly zones (LuminanceSurge / GlyphCorruption / PulseWave phosphor post-process). Previously `set_mouse_click` and `spawn_anomaly` continued to spawn under `--disable-effects` — a partial-disable leak. Both are now gated with an early-return; existing in-flight particles/waves/zones fade out naturally on their next update tick. CLI-only (no config needed). Default: effects on.
- **`--benchmark` auto-enables `--no-effects`**: particle effects are input-driven (mouse clicks, border touches) and never spawn during a benchmark run. `cosmostrix --benchmark` is now equivalent to `cosmostrix --benchmark --no-effects` — the user no longer needs to pass `--no-effects` explicitly. The bench CONFIG report's `no_effects` field always shows `true` for any bench mode (`--benchmark`, `--bench-all`, `--bench-frames`). This is a zero-cost auto-enable: no behavior change, no perf impact, just cleaner UX.
- **CLI did-you-mean consistency**: the custom Levenshtein-based suggestion engine (`KNOWN_LONG_FLAGS` + `cli_edit_distance` in `src/validation/mod.rs`) was removed and replaced with `extract_clap_suggestion()` in `src/main.rs`, which reads clap's own "tip:" line and reformats it as "Did you mean --<flag>?". This fixes an inconsistency where `--no-effecs` (typo) showed only the "tip:" line but NOT "Did you mean?" (because `no-effects` was missing from the hand-maintained flag list after the rename). It also fixes a disagreement where `--clr` showed "tip: --color-bg" (clap's jaro) but "Did you mean --color?" (custom Levenshtein) — now both lines always agree.
- **PERF-2-Supreme: benchmark CONFIG completeness**: the `--benchmark` text report CONFIG section now includes the owner-requested `no_effects` key (`true` when `--no-effects` is set; pure transparency — particles are mouse/click-driven and never spawn during a benchmark). The `--json` output gained `power_dragon`, `crystal_dragon`, `msg_mode`, and `no_effects` in its `config` object for CI/script parity (previously these keys existed only in the text report). The `cosmetics_skipped` disclosure line now lists the full set: message border + anomaly zones + CRT vignette + emergent storytelling.
- **`--disable-effects` CLI flag** (historical, superseded by `--no-effects` above): original introduction — disabled quantum ripple mouse-click burst + border-touch splash crown spark. Useful for VTE terminals (Konsole, GNOME) where particle effects cause fullscreen lag. CLI-only (no config needed). Default: effects on. See `INSIGHTS.md` for the origin story.

### Docs

- **INSIGHTS.md**: New living idea journal documenting the moments when cosmostrix's features were born — not from issue trackers or user requests, but from the owner's lived experience with the renderer running in the background of daily life. First 3 entries: (1) border-touch glow "wifi offline" moment, (2) particle spark "just woken up" moment, (3) the "living project" realization. Future insights will be appended as they arrive.
- **KNOWN_ISSUES.md**: Added "VTE-Based Terminals (Konsole, GNOME Terminal): Fullscreen Performance" section documenting the CPU-rendering bottleneck that causes lag + stale trails on VTE terminals in fullscreen mode. The existing throttle mechanisms (PERF-3 phosphor boost hysteresis, commits `77d0bcf` + `22549bd`) improve the situation but cannot fully fix VTE's internal buffering limitation. Workaround: use Alacritty or run in a smaller window.
- **README.md**: Added INSIGHTS.md to the Documentation index section.

---

## v50.0.0-beta.6 — Verbose UTC Exit + HUD Dragons + Perf-Stats Fixes (Current Beta)

cosmostrix v50.0.0-beta.6 — verbose exit summary now shows UTC exit time + duration, the HUD gains two new dragon on/off indicators (prdr, crdr) above cid, and three `--perf-stats` exit issues are fixed (total cell count, final FPS line position, blank lines after exit). UTC format chosen for LTS stability (no DST transitions, no tzdata drift).

### README

- **Dragon challenge note**: added a centered blockquote to README.md after the intro section: *"Think you can beat cosmostrix? Go ahead -- no force needed. But when you enter the rain, you'll feel the depth -- and you'll understand why the dragon never loses."* Sets the tone for the project identity.

### What's new since beta.5

- **Verbose exit time + duration (UTC)**: the `cosmostrix -v` / `--verbose` post-exit "final runtime state" section now leads with an `exit_time:` + `duration:` line. `exit_time` is the UTC time at exit, formatted as `YYYY-MM-DD HH:MM:SSZ` (ISO 8601 UTC designator). `duration` is the total process lifetime from the `Instant` captured at the top of `main()`, formatted as `Xm Ys` / `Xh Ym Ys`. The section now always prints (previously early-returned when no field changed) so the user always sees how long cosmostrix ran.
- **UTC for LTS stability**: the exit-time format uses UTC (not local + offset) because UTC has no DST transitions, no timezone-database drift, and is consistent across environments. The `Z` suffix (ISO 8601 UTC designator) is universally recognized and machine-parseable.
- **HUD dragon on/off indicators (prdr, crdr)**: two new HUD metrics added at rows 15-16, directly above cid (now row 17 — still owner-mandated bottom row). `prdr: on/off` shows the live power-dragon state; `crdr: on/off` shows the live crystal-dragon state. Values are NOT hardcoded — they track the live runtime state (set by `set_power_dragon` / `set_crystal_dragon`, called every frame from the event loop with `cfg.power_dragon` / `cfg.crystal_dragon`). When the user live-reloads `power_dragon = false` or `crystal_dragon = true` in config.toml, the HUD reflects the new state on the next 1 Hz metric tick.
- **HUD layout expansion**: `cached_lines` array expanded from 16 -> 18 rows. The chroma gradient function renamed `compute_chroma_gradient_16` -> `compute_chroma_gradient_18` (divisor 15.0 -> 17.0). The cid line moved from row 15 to row 17 (still the last/bottom row). All existing HUD tests updated for the new row indices and palette sizes.
- **Perf-stats total cells (owner request)**: the `--perf-stats` MOTION section now shows `total_cells` (e.g. `4.8K (150x32 grid)`) alongside `avg_dirty_cells` (now `1031.6 (of 4.8K total)`). Previously only `avg_dirty_cells` was shown with no total context, causing confusion about what the number means relative to the grid size.
- **Perf-stats final FPS line position fix**: the `[cosmostrix] final FPS: ...` summary line is now printed BEFORE the perf report (as a header), not after it. Previously the line appeared at the very bottom of the report — an inconsistent position for a summary. Now the user sees the one-liner first, then the detailed report below it.
- **Blank lines after exit fix**: removed `cursor::MoveTo(0, h-1)` from the terminal cleanup path. This call moved the cursor to the BOTTOM of the terminal after `LeaveAlternateScreen`, creating a large blank gap between the shell prompt (restored position) and any post-exit output (perf report, verbose summary). `LeaveAlternateScreen` already restores the cursor to where it was before entering the alt screen (right after the shell prompt), so the `MoveTo` was counterproductive. The blank gap is now eliminated.
- **New clock helpers**: `clock::now_utc_datetime()` (formats `YYYY-MM-DD HH:MM:SSZ` using the existing `utc_tm()` FFI path) and `clock::format_duration_compact()` (formats `Duration` as `1m 52s` / `1h 5m 3s`). Both pure functions, fully unit-tested.
- **8 new unit tests**: `now_utc_datetime_format`, `now_utc_datetime_is_ascii`, `now_utc_datetime_matches_now_iso_utc`, `format_duration_compact_canonical_cases`, `format_duration_compact_drops_subsecond`, `hud_prdr_defaults_to_on`, `hud_crdr_defaults_to_off`, `hud_set_power_dragon_off_renders_off`, `hud_set_crystal_dragon_on_renders_on`, `hud_prdr_crdr_above_cid_in_layout`, `hud_prdr_crdr_live_reload_toggle`. Total: 1693 passed / 0 failed / 2 ignored.

### Sample output (verbose exit)

```text
[verbose] [01:29] final runtime state
[verbose] [01:29]   exit_time:     2026-08-26 01:29:20Z | duration: 1m 52s
[verbose] [01:29]   density:       0.66 (was 0.75)
[verbose] [01:29]   crystal_dragon: false (was true)
[verbose] [01:29]   ambient_diag: startup=0 rx=0 reapply=0 snapback=0 cfg_rebuilds=1 sked_reloads=0 sked_empties=0 consistency_fixes=0 snapback_killed=0 snapback_guard_sked_len=0 snapback_guard_last_applied=0 last_scene_change=none
```

### Sample output (perf-stats MOTION section, after fix)

```text
[cosmostrix] final FPS: 144.1 (instant: 144.0, target: 144.0), frames: 4324, elapsed: 30.00s
COSMOSTRIX PERFORMANCE REPORT
─────────────────────────────
...
MOTION
  total_cells:                  4.8K (150x32 grid)
  avg_dirty_cells:              1031.6 (of 4.8K total)
  avg_dirty_cell_ratio_percent: 21.49% (of 150x32 grid)
  visual_fps_hint:              144.0 (4324 of 4324 frames had visual changes)
...
```

The final FPS line now appears as a header BEFORE the report (consistent position), and `total_cells` is shown so the user can see the full grid size alongside the average dirty cells.

When no live-reload field changed during the session, the section still prints the header + `exit_time`/`duration` line + `ambient_diag` line, so the user always sees how long cosmostrix ran.

### Files changed

- `src/clock/mod.rs` — new `now_utc_datetime()` (reuses `utc_tm()` FFI), `format_duration_compact()`; 5 new unit tests
- `src/interactive/mod.rs` — `print_final_runtime_state()` accepts `start_time: Instant`; removed `if !changed { return; }` early-exit; always prints `exit_time` + `duration` as first content line; calls `now_utc_datetime()` for the UTC stamp
- `src/main.rs` — captures `start_time = Instant::now()` at top of `main()`; passes it to `print_final_runtime_state`
- `src/cli/help_detail.rs` — `-v, --verbose` help text mentions the exit time + duration summary
- `docs/RULES.md` — verbose output section updated to describe the always-print behavior + UTC exit_time/duration line
- `CHANGELOG.md` — this entry

### Design notes

- **Why `Instant` not `SystemTime` for duration**: `Instant` is monotonic — NTP jumps, manual `date` changes, or DST transitions cannot make the duration negative or jump. `SystemTime::now().duration_since(start)` can panic on clock rollback; `Instant::elapsed()` is always sound.
- **Why UTC not local + offset**: UTC is LTS-stable. Local time depends on the system timezone database (tzdata), which can drift or be unavailable on minimal containers. DST transitions can make a wall-clock stamp ambiguous (the 2am->3am fall-back produces two identical local stamps for different UTC moments). UTC has none of these issues — it is the same everywhere, always monotonically increasing, and never ambiguous. A user comparing logs across servers in different timezones can do so without mental conversion.
- **Why `Z` suffix not `+00:00`**: `Z` (Zulu) is the standard ISO 8601 UTC designator — shorter, universally recognized, and unambiguous. `+00:00` is valid but verbose; `Z` is the conventional choice for UTC stamps in logs and timestamps.
- **Why always print (not conditional on `changed`)**: the owner's explicit ask was "user can see how long cosmostrix run if user using verbose mode". Suppressing the section when nothing changed would hide the duration — defeating the feature's purpose. The per-field `if final_X != startup_X` guards still suppress unchanged fields, keeping the section scannable.

### Lock status

- Cosmic Dragon: untouched (no cosmic paths modified)
- Chroma Dragon: untouched (no chroma paths modified)
- Crystal Dragon: untouched (no crystal paths modified)
- Clock subsystem: extended (additive change — new functions, no behavior change to existing callers)

---

## v50.0.0-beta.5 — Exp Decay Easing Consolidation (Current Beta)

cosmostrix v50.0.0-beta.5 — masterclass easing consolidation. All **temporal** easing in the rain simulation now uses the unified **exponential decay** family. Owner-approved, owner-verified feel. 227 source files, ~89K LOC, ~1500+ tests pass (1656/0/2 — 4 new regression tests added).

### What's new since beta.4

- **Pause/resume -> exp decay** (commit `e2e0512`): replaced the prior smootherstep S-curve (6t⁵-15t⁴+10t³ over fixed 0.30s decel / 0.45s resume) with asymmetric exponential decay — `exp(-k·t)` decel (k=1.2/s, settle 5% @ ~2.5s) + `1 - exp(-k·t)` accel (k=0.9/s, settle 95% @ ~3.3s). The asymmetric k_decel > k_resume preserves the prior "pause snappy / resume wake-up" feel. Settle thresholds snap to clean terminal state so other subsystems (spawn_remainder reset, monolith stream shift, phosphor LUT) see unambiguous transitions. Restores the README's previously-stale "exponential deceleration (~3s coast-down)" promise (smootherstep is not exponential — the README was wrong under the prior implementation).
- **Glyph scene entry -> exp decay** (this beta): migrated the scene-entry ramp from smoothstep (3t²-2t³ over 700ms) to the same exp approach family — `1 - exp(-k·t)` with k=4.28/s (derived so settle 95% lands at the documented 700ms). Now all temporal easing in the rain path uses the same physical-drag model — pause, resume, and scene entry all coast under the same math primitive. exp() was already in use in the cosmic locked path (`cloud/phosphor.rs:307` LUT build) and chroma shaders/base LUT (`shaders/base/mod.rs:237`), so no new math primitive introduced.
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
- **`--uniform` removed** -> replaced by `--async-mode false`. `--check-updated` alias removed -> `--check-update` is canonical.
- **Verbose honesty**: "final runtime state" section now tracks ALL live-reload fields (12 total) — shows EFFECTIVE runtime values, not startup values.
- **Border gradient fix**: triangle wave eliminates sharp white->black gap on left border. All color output routes through Chroma Dragon (routing rule codified).
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

- Introduced Crystal Dragon Engine: ambient palette drift via CPU/CLOCK -> temperature groups.
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
