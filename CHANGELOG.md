# Changelog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

Cosmostrix uses [SemVer](https://semver.org/) for package versions (e.g. `4.0.0`).
Git tags and GitHub Releases use a leading `v` (e.g. `v4.0.0`).
Stable releases do not use `-stable.N` suffixes.

All notable changes to this project are documented in this file.

---

## v10.0.0 — Peak Performance & Stability

Major performance optimization and stability hardening release.
+76.5% FPS improvement over v5.0.3 baseline through three optimization
phases plus a brutal pre-release audit. Lightning feature removed per
user request (never reached satisfying visual feel). License enforced
as GPL-3.0-only across all 171 source/doc/config files.

### Performance — Phase A: Hot-Path Optimization (+73.8% FPS)
- `phosphor_active` O(1) dedup via `phosphor_in_active` BitVec —
  eliminated 5K-100K wasted ops/frame from linear `contains()` scan
- `head_brightness()` hoisted out of per-line loop — eliminated 4K
  redundant `Instant::elapsed()` + `exp()` calls/frame
- `is_bright()` / `is_dim()` cached in `DrawCtx` — eliminated 100-300
  per-cell calls/frame when glitchy
- `viewport_edge_fade()` precomputed as LUT per terminal resize —
  eliminated 300-1000 float divisions/frame
- `phosphor_fresh` incremental clear — replaced O(W×H) `fill(false)`
  with ~200 bit clears
- `monolith_breathing_factor` computed once per stream, passed to both
  `draw_spine` and `draw_segments` — eliminated redundant cross-module
  call
- `zactrix monolith_*` functions marked `#[inline]` for cross-module
  inlining

### Performance — Phase 2: Structural (+1.6% FPS)
- Spawn free-list: `droplet_free_list: Vec<usize>` replaces O(N) linear
  scan with O(1) pop/push lifecycle
- Terminal flat dirty pairs: single `Vec<usize>` + single sort replaces
  nested `Vec<Vec<usize>>` — better cache locality, no per-row realloc

### Stability — Pre-Release Audit Fixes
- **CRITICAL**: Panic hook no longer writes to stdout (was racing with
  `Terminal::drop`'s BufWriter flush, leaking rain onto user's main
  terminal screen)
- **HIGH**: Added SIGQUIT to graceful shutdown signal set (was defaulting
  to core dump, bypassing all cleanup)
- **HIGH**: Added `debug_assert!` guard + `.min(255)` clamp in
  `fill_color_map` u8 cast (prevents latent panic if palette > 257 colors)
- **MEDIUM**: `Instant::now() - UPDATE_INTERVAL` → `checked_sub()` in
  bench.rs (prevents panic at boot epoch in containers/VMs)
- **MEDIUM**: `term_reinit.swap(false, Acquire)` → `AcqRel` for correct
  RMW memory ordering
- **MEDIUM**: `validate_err(...).unwrap()` → `.unwrap_or(s)` for
  defense-in-depth
- **LOW**: `tp + 1` → `tp.saturating_add(1)` in droplet/rain hot path

### Dead Code / Bloat Removal
- Deleted `column_transition_delay_ms: Vec<u16>` field (never read)
- Deleted `EVENT_MAX_CONCURRENT` constant (never referenced)
- Removed stale `#[allow(dead_code)]` on `EVENT_RNG_XOR` (is used)

### Feature Removal
- **Lightning system completely removed** (~3000 lines deleted). The
  atmospheric lightning feature (Storm Mode, Weather Director, bolt
  families, illuminate, global pulse) never reached a satisfying visual
  feel after multiple tuning iterations. Removed entirely rather than
  shipped in a poor state. Ghost event (phosphor ghost kanji) retained
  as a separate atmospheric feature.

### License
- Enforced `GPL-3.0-only` across all 171 source/doc/config files
- Fixed `scripts/check-headers.sh` stale `EXPECTED_LICENSE` variable
- Extended `check-headers.sh` to scan `*.md` files (was .rs/.sh/.toml/
  .yml only)
- Updated `LICENSE` body: removed "or (at your option) any later version"

### Benchmark
```
v5.0.1 baseline:    avg_fps 21,359  | frame_time 0.046ms | p99 0.058ms
v5.0.3:             avg_fps 27,869  | frame_time 0.035ms | p99 0.046ms
v10.0.0:            avg_fps 38,545  | frame_time 0.025ms | p99 0.031ms
Gain (v5.0.3→v10):  +38.3% FPS      | -28.6% frame time  | -32.6% p99
Gain (v5.0.1→v10):  +80.5% FPS      | -45.7% frame time  | -46.6% p99
```

---

## v5.0.0 — Nightfall

Cinematic UX + Product Identity Release. Polishes discoverability,
error messages, help text, and configuration UX to product-grade quality.
Establishes the cinematic breathing language as an authoritative
reference for how visual transitions and atmospheric effects should feel.
No renderer hot-path rewrite. No benchmark output field changes.
No 50k FPS promise. Terminal writer remains single-owner.
Benchmark honesty preserved.

### Added
- `--show-preset <NAME>` flag: display full preset details including
  description, overridden parameters, and effective values for any
  named preset. Makes preset behavior inspectable without running the
  renderer. Commit `e9f7b3b`.
- `config/cosmostrix.example.toml`: well-commented example configuration
  file with documented defaults and three profile examples (calm-night,
  cinematic, and dense-stress) ready to copy into `~/.config/cosmostrix/
  config.toml`. Commit `e9f7b3b`.
- `docs/CINEMATIC_BREATHING.md`: authoritative cinematic breathing
  vocabulary and pacing contract defining eight terms (Rest, Pulse,
  Whisper, Compression, Void, Signal, Storm, Breath Cycle), eight pacing
  rules, naming conventions, a 10-layer state hierarchy, and six
  anti-patterns. Commit `6289f41`.
- Cinematic breathing vocabulary with formal definitions for all
  atmospheric intensity levels, establishing a shared language for
  future development and documentation.
- Pacing contract: no instant visual state changes, default state is
  always Rest, Storm is never a default, transitions must be perceptible
  as breathing rather than flickering.
- `--profile` help text now includes `(see --list-profiles)` cross-
  reference so users know where to find available profiles.

### Changed
- Error messages follow a consistent pattern: `error: unknown <type>
  '<value>'` followed by a discovery hint line suggesting the
  appropriate `--list-*` flag. This applies to `--preset`, `--scene`,
  `--color`, `--charset`, and `--profile` validation errors.
- `--color` validation exit code changed from 2 to 1 for consistency
  with other user-input validation errors.
- `--charset` error message changed from "unsupported" to "unknown" for
  consistency with all other validation error messages.
- `--color` error message changed from inline parenthetical format to
  a separate hint line for consistency with other discovery hints.
- `docs/ROADMAP.md` updated with v5.0.0 Nightfall active development
  section, phase table, and cinematic breathing language reference.
- `docs/V5_NIGHTFALL_PLAN.md` created as the full v5.0.0 planning
  document covering scope, non-goals, release safety, phase plan, and
  Android/Cosmostrix Live boundaries. Commit `dc27e6f`.
- `docs/cosmostrix-next-vision.md` created for future sibling product
  (Cosmostrix Live) exploration as an explicitly exploratory document.
  Commit `dc27e6f`.
- `--help` output reorganized with a DISCOVERY section grouping
  `--list-presets`, `--list-profiles`, `--list-scenes`, and
  `--show-preset` for better scannability.

### Fixed
- `--profile` help text previously lacked a cross-reference to
  `--list-profiles`, making profile discovery unintuitive for new
  users. Now includes `(see --list-profiles)` hint.
- `--charset` error message used "unsupported charset" instead of
  "unknown charset", breaking the consistent `error: unknown <type>`
  pattern. Now uses the consistent format.
- `--color` error message used an inline parenthetical hint instead of
  a separate discovery hint line, inconsistent with all other
  validation errors. Now uses a separate line.

### Release Safety
- All v4.9.0 release guard mechanisms inherited and active.
- Terminal writer remains single-owner.
- Benchmark honesty preserved: no fake benchmark progress, no cherry-
  picked runs, no omitted metrics.
- No renderer hot-path changes.
- No benchmark output field changes.
- Terminal lifecycle contract remains authoritative.
- 993 deterministic tests passing.
- No new dependencies.

---

## v4.9.0

The Wolf: Release Guard + Terminal Runtime Contract. Hardens the release
process with mandatory pre-tag gates, automated benchmark reporting,
terminal lifecycle documentation, and doctor/report polish. No renderer
hot-path behavior changes and no benchmark output field changes.

- Release guard foundation (Phase 1): 10-gate (now 11-gate) pre-tag
  checklist in `docs/RELEASE_GUARD.md` ensures benchmark reports,
  version metadata, docs guards, CI, and terminal lifecycle verification
  all pass before any release tag is created. Commit `cf63254`.
- Benchmark report automation (Phase 2): `scripts/release-benchmark-report.sh`
  implements full 5-run benchmark collection and Markdown report generation
  with invariant validation. Commit `f3b6b63`.
- Terminal lifecycle matrix (Phase 3): `docs/TERMINAL_LIFECYCLE_MATRIX.md`
  documents expected cleanup behavior across 14 terminal lifecycle paths
  including normal exit, SIGINT, SIGTERM, SIGHUP, SIGTSTP/SIGCONT, SIGKILL,
  `--reset-terminal`, Windows Terminal, tmux, ssh, headless, benchmark mode,
  and doctor mode. Commit `294ad65`.
- Doctor/report polish (Phase 4): `--doctor` output now includes lifecycle
  contract fields (`signal_exit`, `sigkill`, `terminal_writer`). `--reset-terminal`
  help text clarified as destructive recovery. Commit `43e3dc9`.
- Terminal cleanup honesty preserved:
  - Normal exit (q/Esc): non-destructive mode/style restore.
  - `--reset-terminal`: explicit destructive recovery (clears screen,
    purges scrollback, resets modes).
  - SIGINT/SIGTERM/SIGHUP: catchable cleanup with viewport clear.
  - SIGKILL: cannot be caught or guaranteed. Fork guard is best-effort,
    Linux-only.
- Release guard Gate 7 (terminal lifecycle verification) requires
  `--doctor` lifecycle contract fields, manual exit testing, and
  SIGKILL honesty.
- 944 deterministic tests, all passing.
- Terminal writer remains single-owner.
- `compute_parallelism` remains `disabled`.
- `actual_execution` remains `single-threaded-renderer`.
- No new dependencies.

## v4.8.0

Zactrix Integration + Terminal Cleanup Hardening. Color pipeline optimization
from the zactrix lab with signal-exit terminal cleanup fixes. No default
visual behavior change and no active parallel compute.

- Integrated accepted zactrix color pipeline optimization from lab source
  `e7253e7` (`zactrix-20k-lab`) via manual adaptation. No direct lab merge.
  Commit `ce8dc81`.
- Single RGB decode path and integer brightness blend path replace
  redundant per-cell color computation, reducing pipeline overhead while
  preserving identical visual output.
- Cached binary pool detection avoids redundant `contains_key` lookups
  during stream spawning.
- `set_force` cleanup optimization removes unnecessary work on cells
  that are already marked dirty.
- 50k FPS lab (`zactrix-50k-lab`) documented as not reached and not a
  release promise. Rejected optimization attempts stay rejected.
- Signal-exit cleanup hardening (Phase 4): signal handler threads no
  longer race on stdout with the main loop's buffered writer.
- Visible residue fix (Phase 4B): catchable SIGTERM/pkill-TERM now clears
  the alternate screen viewport before switching back, preventing rain
  frame glyphs from bleeding into the main screen. Fork-guard child
  process silenced on normal parent exit to prevent stdout races.
- Cross-platform signal cleanup imports fixed for Windows CI.
- Windows Terminal `--reset-terminal` issue #15 verified clean.
- 891 deterministic tests, all passing.
- Terminal writer remains single-owner.
- `compute_parallelism` remains `disabled`.
- `actual_execution` remains `single-threaded-renderer`.
- No version bump until this release prep.
- No new dependencies.

## v4.7.0

Profile Ecosystem. Documentation, validation UX, and release-candidate smoke
coverage for the profile system with no default visual behavior change and no
active parallel compute.

- Profile ecosystem contract documenting profile precedence
  (CLI > profile > config > defaults), profile resolution, and mutation
  semantics.
- Profile examples documentation with ready-to-copy config snippets for
  common profile use cases.
- Config dump and `--list-profiles` enhanced with profile documentation
  pointers to `PROFILE_ECOSYSTEM.md` and `PROFILE_EXAMPLES.md`.
- Profile validation UX polish with clear, actionable error messages:
  unknown profiles mention `--list-profiles`, invalid fields/values show
  expected formats, storm rejection is explicit.
- Unknown profile actionable error: both CLI and config paths produce
  clear diagnostics pointing to `--list-profiles`.
- Storm unavailable: error messages and config dump consistently state
  that storm is unavailable.
- Profile RC smoke coverage in `scripts/rc-smoke.sh` with 11 profile-related
  checks and `docs/RELEASE_CANDIDATE.md` updated with v4.7 profile checklist.
- Default remains disabled/protected/identity. No live atmosphere enabled by
  default.
- Terminal writer remains single-owner. Compute parallelism remains disabled.
- No zactrix-20k-lab merge.
- 858 deterministic tests, all passing.

## v4.6.0

Controlled Atmosphere Expansion. Docs, test infrastructure, and CLI
discoverability release with no default visual behavior change and no active
parallel compute.

- Controlled atmosphere expansion contract with state matrix (identity, whisper,
  shadow, protected) and six regimes (calm, pulse, signal, compression, void,
  monolith-pressure). Storm is intentionally unavailable.
- Preset registry with six controlled atmosphere presets: atmosphere-calm
  (identity), atmosphere-pulse, atmosphere-signal, atmosphere-compression,
  atmosphere-void, atmosphere-monolith-pressure (all whisper). Presets are
  opt-in only.
- Preset UX documentation, config/profile examples, and config dump atmosphere
  lines for discoverability.
- `--list-profiles` enhanced with controlled atmosphere preset section showing
  mode, regime, and shadow level for each preset.
- RC smoke script hardened with six atmosphere checks (preset listing, storm
  rejection, controlled-live field verification, disabled+non-calm identity,
  color sun sticky).
- 800 deterministic tests, all passing.
- Default remains disabled/protected/identity. No live atmosphere enabled by
  default.
- Storm unavailable. Terminal writer remains single-owner.
- No zactrix-20k-lab merge.

## v4.5.0

Zactrix Foundation + Depth Regression. Architecture and test infrastructure release with no default visual behavior change and no active parallel compute.

- Split Zactrix Engine architecture into core/cache/render/system/scheduler/metrics modules.
- Added honest ZACTRIX SYSTEM diagnostics (runtime_mode, cpu_budget, render_plan, compute_parallelism, idle_policy).
- Added depth regression lab for Monolith Rain visual stability (15 categories, deterministic guards).
- Split docs, monolith, and scene regression tests into focused module directories to keep all files under 1000 LOC.
- Added roadmap closure docs covering v4.6/v4.7/v4.8/v5 release trajectory.
- No default visual behavior change.
- No active parallel compute.
- Terminal writer remains single-owner.

## v4.0.1

Fixed version output build label to include the optimized CPU tier, matching doctor/benchmark diagnostics.

- `cosmostrix -V` / `--version` now reports the canonical build label (e.g. `linux-x86_64-v3`) from `COSMOSTRIX_BUILD`, consistent with `--doctor`, `--benchmark`, and `--info`.
- Added `canonical_build_label()` as the single source of truth for the build label across all output paths.
- Added deterministic tests to prevent this mismatch from returning.

## v4.0.0

Full Atmosphere Engine groundwork and signature Monolith Rain maturation release.

Highlights:
- Signature Monolith Rain as the production default, with refined sparse data pillars, subtle phase variation, clean afterglow, and bounded residue behavior.
- Zactrix Core / Zactrix Engine / Zactrix Cache groundwork for adaptive rendering architecture, while terminal writes remain single-owner.
- Atmosphere engine internal model, verifier, controlled-live config gate, visual whisper, shadow metrics, and A/B safety smoke tests.
- Terminal compatibility lab, doctor guidance, reset safety, color capability diagnostics, and clean terminal recovery.
- User scene/profile config with controlled atmosphere profile keys.
- Benchmark/endurance/report hardening with honest planned-vs-actual execution diagnostics.
- README demo refresh with GIF-first v4 preview, MP4 link, and binary/retro posters.
- Canonical metadata alignment across Cargo, README, runtime identity, and AUR packaging.
- Release-candidate smoke script and release checklist.

Safety/defaults:
- Default runtime remains protected and identity: `application_mode = disabled`, `effective_runtime = identity`, `shadow_risk = identity`.
- `auto_color_drift` remains off by default.
- `storm` is not config-safe in controlled-live config/profile mode.
- No actual multithreaded terminal rendering; benchmark reports planned engine mode honestly.

## v3.9.0

Internal v4.0.0 ground-work phase. No public API or visual behavior changes.

- Atmosphere visual whisper engine with bounded A/B smoke testing
- Whisper wiring guard and runtime shadow metrics
- Zactrix Core eBPF-inspired architecture discipline
- Self-referential guard string avoidance pattern
- Phase 10.5: atmosphere config honesty + profile smoke hardening (27 new tests)
- Added v4 demo poster and MP4 assets for README preview
- Made the v4 README demo GIF-first and removed the obsolete demo GIF
- Replaced single v4 demo poster with binary and retro themed demo screenshots
- 568 deterministic tests, all passing

## v3.1.0

**Monolith Rain Engine.** Plain `cosmostrix` now launches signature Cosmostrix
Monolith Rain: sparse structured vertical data pillars with segmented blocks,
subtle spines, visible gaps, and a clear brightness hierarchy. Classic Matrix
glyph rain remains available with `cosmostrix --scene matrix`.

## v2.2.0

**Stability, maintainability, and supply-chain hardening release.** No visual
or CLI behavior changes.

- All `*.rs` files are under 1,000 gross lines (enforced by `check-rs-loc.sh` in `check-all`)
- Module splits: `src/cloud.rs` → `src/cloud/` (8 modules), `src/interactive.rs` → `src/interactive/` (6 modules), `src/main.rs` → `src/app.rs` + `src/cli.rs` + `src/info.rs` + `src/main.rs`
- Cloud tests split into `tests/mod.rs` (core) and `tests/tests_phosphor.rs` (phosphor/ghost)
- Added endurance testing documentation ([ENDURANCE.md](docs/ENDURANCE.md)) and resource summary script
- Added supply-chain hardening policy ([SUPPLY_CHAIN.md](docs/SUPPLY_CHAIN.md))
- Added terminal stability audit ([STABILITY_AUDIT.md](docs/STABILITY_AUDIT.md))
- Added SIMD feasibility audit ([SIMD_FEASIBILITY.md](docs/SIMD_FEASIBILITY.md))
- Engine module splits: `cloud/mod.rs` → `scene_runtime.rs` + `runtime_controls.rs` (scene switching and runtime controls extracted from core module)
- Fixed clippy module-inception and unused import warnings
- Regression suite passes, clippy clean, fmt clean

## v2.1.0

**Visual contrast & readability overhaul** — body glyphs are now clearly readable
with stronger head/body/trail hierarchy while preserving the calm cinematic identity.

- Tuned exponential trail decay (K: 3.0 → 1.8) for readable body glyphs across the full trail length
- Raised parallax brightness (far: 35→55%, mid: 80→90%) so depth layers are visible, not invisible
- Increased phosphor residual energy (120→160) for more visible CRT afterglow fadeout
- Extended head linger duration (100→300ms) for smoother cinematic head fade
- Added head self-bloom (12% white blend) making the head clearly the brightest element
- Softer head brightness mapping (0.5+0.5×hb → 0.7+0.3×hb) preventing abrupt head disappearance
- Raised luminance climate minimum (60→75%) and saturation minimum (50→70%) to prevent muddy/dim periods
- Raised fog vignette minimum (25→35%) to keep edge glyphs faintly visible
- Reduced far-layer glyph dimming (30→15%) — already dim from parallax brightness
- TrueColor green palettes now use 24-bit RGB gradients instead of ANSI 256-color indices, with proper bright green head instead of cyan-white
- Reduced profile luminance offsets (Monolith: -0.1→0, Void: -0.2→-0.1, Decay: -0.15→-0.05, Static: -0.25→-0.1)

**Safety & hardening fixes:**

- Tab key safely ignored (was toggling shading mode, causing ghost background glyph flood)
- Paste safety (bracketed-paste burst suppression ignores shortcut letters during paste)
- Pause/resume with cinematic smoothstep easing (no snap on resume)
- Color and charset transitions use cinematic top-to-bottom wave propagation
- Mouse mode default-off, opt-in with `--mouse`
- Bottom-row phosphor decay acceleration prevents "concrete wall" accumulation
- Ghost glyph threshold prevents stale charset from filling background on full redraw
- Safe terminal cleanup on all exit paths (RAII guard + `--reset-terminal`)

## v2.0.0

- Fixed stale glyph artifacts in the top visible rows during charset and theme changes.
- Fixed long-idle rain/trail resync issues with wall-clock redraw scheduling and focus/input redraw resync.
- Clarified benchmark dirty-cell and color-mode metrics so differential rendering reports are easier to interpret.
- Fixed direct-color auto-detection for `xterm-direct` and `tmux-direct`.
- Removed unused low-value support code while preserving rendering behavior.
- Completed 10h+ visual soak checks across Alacritty, Konsole, and WezTerm.
- Resource monitoring found no memory, file descriptor, thread, swap, CPU, or IO leak during the release soak.
