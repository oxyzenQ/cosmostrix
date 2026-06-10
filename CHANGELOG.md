# Changelog

Cosmostrix uses [SemVer](https://semver.org/) for package versions (e.g. `4.0.0`).
Git tags and GitHub Releases use a leading `v` (e.g. `v4.0.0`).
Stable releases do not use `-stable.N` suffixes.

All notable changes to this project are documented in this file.

---

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
