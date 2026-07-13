<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmostrix Roadmap

## Release History

### v11.1.0 — Benchmark Depth & Theme Tuning (COMPLETE)

Closes the "real metrics, not gimmick" gap and pushes the benchmark to
S-tier (DeepSeek 9.8/10 → 10/10). The premium benchmark now reports RSS
memory, CPU usage, sub-component timing, long-run drift, build/environment
metadata, page faults + context switches, GPU-not-used declaration, and
JSON output. A live HUD overlay brings the same metrics into interactive
runs. `--color-tune` turns the 43 themes into 43 × ∞ variants. 5
near-duplicate themes tuned for visual distinctness.

| Batch | Description | Commit |
|-------|-------------|--------|
| P0-A | RSS memory tracking (peak + avg), MEMORY section | `34f22df` |
| P0-B | max_frame_time + p99.9_frame_time tail metrics | `3afac82` |
| P1-A | Sub-component timing (sim/render/io) — COMPONENT TIMING section | `6bc5035` |
| P1-B | `--bench-duration N` flag + DRIFT section | `9e94527` |
| P2 | Live HUD overlay (`i` toggle) — fps/p99/max/rss | `12a1d2f` |
| P3 | CPU usage % tracking — CPU section | `aeafdd3` |
| P2-fix | HUD toggle key changed from `?` to `i` (Android/Termux reliability) | v13.1.1 |
| Q2 | `--color-tune saturation/brightness` runtime adjustment | `ce0d191` |
| Audit | 5 near-duplicate themes tuned for distinctness | `304a07b` |
| Peak | Build metadata + CPU model + getrusage RESOURCE section | `7db64b9` |
| Peak | GPU-not-used declaration + BENCHMARK ENVIRONMENT + `--json` output | (pending) |
| CI fix | macOS libc + Mach API + time_value_t + sysctlbyname + codespell | `22fa131`–`4726d9a` |

Zero new runtime dependencies. Linux + macOS supported for RSS, CPU, and
RESOURCE tracking; Windows/other platforms emit `unsupported` honestly.
Theme count stays 43. See `CHANGELOG.md` for full details.

### v12.0.0 — Protocol Engine (COMPLETE)

Terminal protocol intelligence and color pipeline optimization.

| Feature | Module | Description |
|---------|--------|-------------|
| Terminal detection | `src/termdetect.rs` | Vendor detection + synchronized output |
| Color byte cache | `src/color_cache.rs` | Pre-formatted SGR, ~300-400 ops/frame saved |
| Unified error UX | `src/ux.rs` | Single source of truth, fixes double-print |
| DEFLATE compression | — | Investigated, skipped — zero terminal support |

`TerminalVendor` enum stored for future protocol-specific features
(kitty graphics protocol, foot damage tracking, etc.).

### v11.0.0 — Peak Performance & Stability (COMPLETE)

Major performance optimization and stability hardening release.
+70.3% FPS improvement over v5.0.3 baseline through three optimization
phases plus a brutal pre-release audit. Lightning feature removed per
user request. License enforced as GPL-3.0-only across all 171 files.

| Phase | Description | Gain |
|-------|-------------|------|
| Phase A | Hot-path optimization (7 fixes) | +73.8% FPS |
| Phase 2 | Structural (spawn free-list + flat dirty pairs) | +1.6% FPS |
| Phase 3 | Cell struct shrink | Cancelled (already 16 bytes via niche optimization) |
| Audit | Panic hook race, SIGQUIT, overflow guards, dead code | Stability hardening |

Cumulative: **+70.3% FPS** (31,445 → 53,561 avg_fps), **-40.6% frame time**.

### v4.9.0 — The Wolf: Release Guard + Terminal Runtime Contract (COMPLETE)

Hardens the release process so benchmark reports cannot be forgotten again.
Adds terminal lifecycle documentation, doctor/report polish, and release
gate guardrails. No renderer hot-path behavior changes.

| Phase | Description | Commit |
|-------|-------------|--------|
| Phase 1 | Release guard foundation | `cf63254` |
| Phase 2 | Benchmark report automation | `f3b6b63` |
| Phase 3 | Terminal lifecycle matrix | `294ad65` |
| Phase 4 | Doctor/report polish | `43e3dc9` |
| Phase 5 | Final release prep | `7aee3b2` |

v4.9 is complete. Terminal writer remains single-owner.
`compute_parallelism` remains `disabled`. `actual_execution` remains
`single-threaded-renderer`. 944 tests pass. 50k was not reached and is
not a release promise. No renderer behavior changes. No benchmark field
changes.

### v4.8.0 — Zactrix Integration + Terminal Cleanup Hardening (COMPLETE)

Integrated accepted zactrix color pipeline optimization with signal-exit
terminal cleanup hardening. No default visual behavior change and no active
parallel compute.

| Phase | Description | Commit |
|-------|-------------|--------|
| Phase 1 | Zactrix Lab Integration Audit | `9c39b3f` |
| Phase 2A | Color Pipeline Code Integration | `ce8dc81` |
| Phase 2B | Validation Lock | `fa2c995` |
| Phase 3 | Main Merge Prep / Conflict Audit | `8e9f3f8` |
| Phase 4 | Terminal Mode Sanity Hardening | `c56b4d7` |
| Phase 4B | Signal-Exit Visible Residue Cleanup | `df671fb` |
| Phase 5 | Release Prep Metadata + RC Gate | `ec1214b` |
| Phase 5B | Release Benchmark Report | `08dc6f5` |

v4.8 is complete. Terminal writer remains single-owner.
`compute_parallelism` remains `disabled`. `actual_execution` remains
`single-threaded-renderer`. 891 tests pass. 50k was not reached and is
not a release promise. No fake benchmark progress.

### v4.7.0 — Renderer Ergonomics + Profile Ecosystem (COMPLETE)

Improved profile configuration, preset management, validation UX, and release
candidate smoke coverage without changing core render behavior.

| Phase | Description |
|-------|-------------|
| Phase 1 | Profile Ecosystem Audit + Contract |
| Phase 2 | Profile Examples + Config Dump Polish |
| Phase 3 | Profile Validation UX + Error Message Polish |
| Phase 4 | Profile RC Smoke + Closure Prep |

v4.7 is complete. Runtime and visual behavior remain stable. Terminal writer
remains single-owner. `compute_parallelism` remains `disabled`.

### v4.6.0 — Controlled Atmosphere Expansion (COMPLETE)

Controlled atmosphere expansion with contracts, docs, presets, and tests.
All atmosphere features remain **opt-in only**. No default visual output
change. Terminal writer remains single-owner. Storm unavailable.

| Phase | Description |
|-------|-------------|
| Phase 1 | Controlled Atmosphere Expansion Contract |
| Phase 2 | Controlled Atmosphere Profile Presets (6 presets) |
| Phase 3 | Preset UX / Config Examples + Pressure-aware Tests |
| Phase 4 | Preset CLI/Profile Discoverability (`--list-scenes`) |
| Phase 5 | Atmosphere RC Smoke + v4.6 Closure |

### v4.5.0 — Zactrix Foundation + Depth Regression (COMPLETE)

Architecture and regression foundation. Complete.

| Phase | Description |
|-------|-------------|
| Phase 1 | Zactrix Engine Architecture Split |
| Phase 2 | Docs Guard Split + ZACTRIX SYSTEM Diagnostics |
| Phase 3 | Depth Regression Lab |
| Phase 4 | Monolith Test Pressure Relief |
| Phase 5 | Scene Test Pressure Relief |
| Phase 6 | Closure Prep |

v4.5 is complete. No active parallel compute was added. The terminal
writer remains single-owner. ZACTRIX SYSTEM diagnostics report honest
policy values. Visual behavior is identical to v4.0.1.

### v4.0.1 — Stable Patch

Production-grade cinematic Matrix rain renderer. Includes
Monolith Rain visual identity, warm-start scene transitions, phosphor decay
system, and sparse density enforcement. All visual behavior is locked down
by the Depth Regression Lab.

---

## Release History (older)

### v5.0.0 — Nightfall: Cinematic UX + Product Identity Release (COMPLETE)

A medium major release focused on polish, discoverability, and
product-grade feel. All 6 phases shipped. Terminal writer remains
single-owner. Benchmark honesty preserved.

| Phase | Description | Commit |
|-------|-------------|--------|
| Phase 1 | Roadmap + product identity foundation | `dc27e6f` |
| Phase 2 | Preset/profile discoverability polish | `e9f7b3b` |
| Phase 3 | Cinematic breathing language + docs contract | `6289f41` |
| Phase 4 | Help/config UX polish | `20552f1` |
| Phase 5 | Release candidate prep | shipped |
| Phase 6 | Signed tag / release / AUR | shipped |

Cinematic breathing vocabulary and pacing contract are defined in
`docs/CINEMATIC_BREATHING.md`.

---

## Invariants (all versions)

These invariants apply to every release. They must never regress:

- **Terminal writer remains single-owner.** No parallel terminal writes.
- **No unbounded thread pools.** Worker budgets are always bounded.
- **Visual identity must remain identical to v3.9.0.** Depth, brightness
  hierarchy, empty-space ratio, and residue bounds are locked.
- **Scene cycling semantics unchanged.** x/X cycle behavior is stable.
- **Color stability behavior unchanged.** Explicit choices remain sticky.
- **`auto_color_drift` remains default `false`.** Opt-in only.
- **`compute_parallelism` remains `disabled`** unless a future release
  explicitly activates it through the Zactrix runtime planner.
- **Benchmark field names unchanged.** No renaming of diagnostic labels.
- **New benchmark sections (v11.1.0) are additive — must not be removed
  without a contract change.** MEMORY, CPU, COMPONENT TIMING, and DRIFT
  sections are part of the `--benchmark` output contract. Linux/macOS-only
  fields emit `unsupported` on Windows rather than being silently absent.

## CPU Targets

- Calm/idle target: < 1-3% realistic CPU usage. Verified via the `CPU`
  section of `--benchmark` (`avg_cpu_percent`, `peak_cpu_percent`;
  Linux/macOS only).
- Benchmark/stress can use dynamic high CPU.
- Paused should remain near 0%.
