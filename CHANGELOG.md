# Changelog

Cosmostrix uses [SemVer](https://semver.org/) for package versions (e.g. `4.0.0`).
Git tags and GitHub Releases use a leading `v` (e.g. `v4.0.0`).
Stable releases do not use `-stable.N` suffixes.

All notable changes to this project are documented in this file.

---

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
