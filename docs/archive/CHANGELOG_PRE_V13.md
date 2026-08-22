<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Changelog — Pre-v13 Archive (Era Summary)

This file summarizes every cosmostrix release before `v13.0.0` in era form.
It was originally a verbatim per-release changelog (714 LOC); it has been
compressed to an era-grouped summary for consistency with the live
[`CHANGELOG.md`](../../CHANGELOG.md).

For per-release detail older than `v13.0.0`, walk the git history:

```bash
git log --oneline v13.0.0 ^v2.0.0
```

## Ordering

Entries are in reverse-chronological order. The top of this file is `v12.0.0`
(the entry immediately below `v13.0.0` in the live `CHANGELOG.md`); the bottom
is `v2.0.0` (the oldest entry in the project's history).

## Tripwire note

`v4.0.0` and `v3.9.0` also appear in the live `CHANGELOG.md` because
`src/docs_tests/metadata.rs` enforces their presence via
`include_str!("../../CHANGELOG.md")`. The two copies describe the same
release; the live file is the source of truth.

---

## v12.0.0 — Protocol Engine (2026-07-08)

Major release introducing terminal protocol intelligence and color pipeline
optimization. The engine detects the terminal emulator at startup and adapts
its output strategy accordingly.

### New Modules

- `src/termdetect.rs` — Terminal vendor detection (kitty, wezterm, alacritty,
  foot, iTerm2, Windows Terminal, tmux, Rio) via environment variables.
  Enables synchronized output (`ESC[?2026h` / `ESC[?2026l`) for tear-free
  frame delivery. Safe on all terminals — unsupported ones ignore the sequences.
- `src/color_cache.rs` — Pre-formatted ANSI SGR byte cache for palette colors.
  Eliminates ~300–400 per-cell encoding calls per full-redraw frame.
  Linear-scan lookup optimized for small palettes (7–20 colors).
- `src/ux.rs` — Unified CLI user-experience output. Single source of truth
  for branding, error formatting, and exit codes.

### Headlines

- Auto-detects terminal capabilities at startup (truecolor / direct-color /
  synchronized-output / bracketed-paste / in-band resize reports).
- Synchronized output for tear-free frame delivery on capable terminals.
- `--benchmark` now emits JSON output via `--json` for CI parsing.
- Cross-terminal compatibility lab — verified across 8 major terminals.

---

## v11.x — Cinematic Peak & Benchmark Depth

The renderer reaches its cinematic peak and the benchmark reaches S-tier.

### v11.1.0 — Benchmark Depth & Theme Tuning

Closes the "real metrics, not gimmick" gap and pushes the benchmark to
S-tier (DeepSeek 9.8/10 → 10/10).

- **RSS memory tracking** in `--benchmark` (Linux `/proc/self/status`,
  macOS `mach_task_basic_info`). Zero new dependencies.
- **Tail frame-time metrics**: `p99_9_frame_time` (1-in-1000 worst frames)
  and `max_frame_time` (single worst spike). Captures jank that p99
  smooths over.
- **Sub-component timing**: `sim_ms` / `render_ms` / `io_ms` plus maxes
  and shares — separates atmosphere+spawn+physics from render+post-FX
  from dirty-check+IO.
- **JSON output mode** for CI parsing.
- **Live HUD overlay** brings the same metrics into interactive runs.
- **Theme tuning**: 43 builtin palettes made more visually distinct.

### v11.0.0 — Cinematic Peak

Pure tuning release — no architecture changes, no new dependencies. Every
change is a constant value adjustment or small feature addition.

- Cosmos palette brightened (avg luminance 30.3% → 45.5%).
- Head white blend raised 12% → 45% for stronger head presence.
- Phosphor decay curve retuned for longer readable trails.
- Smoothstep easing on pause/resume (no snap on resume).
- Top-to-bottom wave color transitions.
- Mouse-click ripple effects (opt-in via `--mouse`).
- Bracketed-paste safety (burst suppression ignores shortcut letters).

---

## v10.0.0 — Peak Performance & Stability

Major performance optimization and stability hardening release. +76.5% FPS
improvement over v5.0.3 baseline through three optimization phases plus a
brutal pre-release audit. Lightning feature removed per user request (never
reached satisfying visual feel). License enforced as GPL-3.0-only across
all 171 source/doc/config files.

### Performance — Phase A: Hot-Path Optimization (+73.8% FPS)

- `phosphor_active` O(1) dedup via `phosphor_in_active` BitVec — eliminated
  5K–100K wasted ops/frame from linear `contains()` scan.
- `head_brightness()` hoisted out of per-line loop — eliminated 4K redundant
  calls per frame.
- Dirty-cell bookkeeping collapsed into a single `SmallVec` pass.

### Performance — Phase B: Allocation Audit

- Per-frame allocation dropped from ~12 KB → ~1.5 KB.
- Hot-path `Vec` calls replaced with stack-allocated `SmallVec`.
- `String` formatting in render loop replaced with `&'static str` matches.

### Performance — Phase C: Stability Soak

- 10h+ visual soak checks across Alacritty, Konsole, and WezTerm.
- Zero memory, FD, thread, swap, CPU, or IO leaks confirmed.
- Long-idle rain resync via wall-clock redraw scheduling.

---

## v5.0.0 — Nightfall

Cinematic UX + Product Identity Release. Polishes discoverability, error
messages, help text, and configuration UX to product-grade quality.
Establishes the cinematic breathing language as an authoritative reference
for how visual transitions and atmospheric effects should feel. No renderer
hot-path rewrite. No benchmark output field changes. No 50k FPS promise.
Terminal writer remains single-owner. Benchmark honesty preserved.

### Added

- `--show-preset <NAME>` flag: display full preset details including
  description, overridden parameters, and effective values for any
  named preset.
- `config/cosmostrix.example.toml`: well-commented example configuration
  file with documented defaults and three profile examples.
- Help text rewrite — every CLI flag now has a usage example and a
  "common mistakes" hint.

---

## v4.x — Atmosphere Polish Iterations

Iterative atmosphere work spanning v4.0.0 → v4.9.0. Each release raised the
visual floor without changing the architecture established in v4.0.0.

### v4.0.0 — Atmosphere Engine + Monolith Rain (foundation)

- Signature Monolith Rain as the production default (sparse data pillars,
  segmented blocks, subtle spines, visible gaps, brightness hierarchy).
- Classic Matrix glyph rain remains available with `cosmostrix --scene matrix`.
- Cosmic Dragon Core/Engine/Cache groundwork for adaptive rendering.
- Atmosphere engine, terminal compatibility lab, doctor diagnostics.
- Profile ecosystem, config discoverability, benchmark hardening.
- Canonical metadata alignment across Cargo, README, AUR.

### v4.0.1 → v4.9.0 — Tuning iterations

- v4.0.1: hot-fix for profile luminance offset on Void profile.
- v4.5.0: fog vignette tuning (edge glyph visibility floor raised).
- v4.6.0: parallax brightness calibration (far: 35→55%, mid: 80→90%).
- v4.7.0: head self-bloom (12% white blend) — head becomes clearly brightest.
- v4.8.0: climate luminance minimum raised (60→75%) to prevent muddy periods.
- v4.9.0: climate saturation minimum raised (50→70%) to prevent dim periods.

---

## v3.9.0 — v4 Ground-Work

- Atmosphere visual whisper engine, cosmic dragon architecture discipline.
- Phase 10.5: atmosphere config honesty + profile smoke hardening.

---

## v3.1.0 — Monolith Rain Engine

Plain `cosmostrix` now launches signature Cosmostrix Monolith Rain: sparse
structured vertical data pillars with segmented blocks, subtle spines,
visible gaps, and a clear brightness hierarchy. Classic Matrix glyph rain
remains available with `cosmostrix --scene matrix`.

---

## v2.x — Soak & Stability Era

### v2.2.0 — Stability, Maintainability, Supply-Chain Hardening

No visual change. CI hardening, supply-chain policy (cargo-vet baseline),
and dead-code removal.

### v2.1.0 — Visual Contrast & Readability Overhaul

Body glyphs now clearly readable with stronger head/body/trail hierarchy
while preserving the calm cinematic identity.

- Tuned exponential trail decay (K: 3.0 → 1.8) for readable body glyphs
  across the full trail length.
- Raised parallax brightness (far: 35→55%, mid: 80→90%) so depth layers
  are visible, not invisible.
- Increased phosphor residual energy (120→160) for more visible CRT
  afterglow fadeout.
- Extended head linger duration (100→300ms) for smoother cinematic head fade.
- Added head self-bloom (12% white blend) — head is clearly the brightest.
- Softer head brightness mapping (0.5+0.5×hb → 0.7+0.3×hb) preventing
  abrupt head disappearance.
- Raised luminance climate minimum (60→75%) and saturation minimum (50→70%)
  to prevent muddy/dim periods.
- Raised fog vignette minimum (25→35%) to keep edge glyphs faintly visible.
- Reduced far-layer glyph dimming (30→15%) — already dim from parallax brightness.
- TrueColor green palettes now use 24-bit RGB gradients instead of ANSI
  256-color indices, with proper bright green head instead of cyan-white.
- Reduced profile luminance offsets.

**Safety & hardening fixes:**

- Tab key safely ignored (was toggling shading mode, causing ghost background glyph flood).
- Paste safety (bracketed-paste burst suppression ignores shortcut letters during paste).
- Pause/resume with cinematic smoothstep easing (no snap on resume).
- Color and charset transitions use cinematic top-to-bottom wave propagation.
- Mouse mode default-off, opt-in with `--mouse`.
- Bottom-row phosphor decay acceleration prevents "concrete wall" accumulation.
- Ghost glyph threshold prevents stale charset from filling background on full redraw.
- Safe terminal cleanup on all exit paths (RAII guard + `--reset-terminal`).

### v2.0.0 — First Public-Stability Release

- Fixed stale glyph artifacts in the top visible rows during charset and theme changes.
- Fixed long-idle rain/trail resync issues with wall-clock redraw scheduling and
  focus/input redraw resync.
- Clarified benchmark dirty-cell and color-mode metrics so differential rendering
  reports are easier to interpret.
- Fixed direct-color auto-detection for `xterm-direct` and `tmux-direct`.
- Removed unused low-value support code while preserving rendering behavior.
- Completed 10h+ visual soak checks across Alacritty, Konsole, and WezTerm.
- Resource monitoring found no memory, file descriptor, thread, swap, CPU, or IO
  leak during the release soak.
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
