<!-- SPDX-License-Identifier: GPL-3.0-only -->

# S-master-v2 (1/2/3/4) Audit — Dragon Hunt / Optimize / Security / Stability LTS (2026-09-01)

Owner mandate across all four tasks: master deeper audit of deps,
cosmostrix/*, and src/* (staged — important dirs first, not a full
scan), no 99% visual/performance changes, 10s A/B benchmark
verification, all docs/reference synced, skip if already peak.

Benchmark protocol (all tasks): monolith 80x24 dry,
`--benchmark --json --bench-duration 10s`, identical to the
Z-master-v2 precedent. Baseline A captured at 8617360 (HEAD before
any S-master-v2 change). Task 4 baseline captured at 22b9417
(HEAD before any S-master-4-v2 change).

## Task 1 — S-master-1-v2 dragon hunt (spaghetti/burden/duplicate/stale/zombie)

### Method (staged)

1. Deps (Cargo.toml): 12 production deps — usage counted by import
   scan. All 12 used (clap 21 files, crossterm 77, rand 21, bitvec 7,
   smallvec 5, unicode-width 6, notify 3, sha2 3, signal-hook 2,
   libc 17, ctrlc 2, proptest 1 dev). Zero zombie deps.
2. cosmostrix/* root: 28 scripts — single-pass reference scan (CI
   workflows, docs, cross-script calls). All referenced. Verdicts on
   the two near-orphans: `emoji-audit.py` is the enforcement tool for
   the owner's standing cold/zen no-emoji directive (kept, it IS the
   policy); `inject-disclaimer.sh` is called by gate-keepers.sh (kept).
   build.rs / deny.toml / pgo-runner/ / aur/ all referenced (8-12
   refs each).
3. Source, important dirs only (cloud, chroma shaders, interactive,
   config, central_control_*): dead-code allow inventory, cross-file
   duplicate helper scan, stale file-pointer sweep (comments referencing
   files that no longer exist at that path).

### Findings and fixes (4 real)

| # | Class | Finding | Fix |
|---|-------|---------|-----|
| S1-1 | stale comment | `crystal_dragon_control` enum doc claimed calc-v2 "NOT YET IMPLEMENTED, reserved for future" — wrong since merge d55442d (calc-v2 is implemented AND the default) | Doc rewritten; `allow(dead_code)` on the enum retained with a justification comment (legacy `Calc` variant matched in production, constructed only in tests) |
| S1-2 | redundant duplicate source-of-truth | `CrystalDragonControl.drift_chance` + `.cpu_ema_alpha` were zombie fields: runtime read the consts (`CRYSTAL_DRAGON_DRIFT_CHANCE`, `CRYSTAL_DRAGON_CPU_EMA_ALPHA`) directly, so the documented "future config override" contract could never work; doctor displayed the field value as if it were live | Wired through: `crystal_dragon_tick` now reads `control.drift_chance`; the sensor copies `control.cpu_ema_alpha` at construction and uses it in the EMA. Consts now only seed the defaults. Struct-level `#[allow(dead_code)]` removed (all 6 fields live). Zero behavior change — field defaults equal the consts |
| S1-3 | burden / dead imports | monolith.rs imported 9 helper names behind `#[allow(unused_imports)]` that are never called from monolith.rs (build_segments, draw_spine_cell, layer_from_roll, segment_gap, segment_len, segment_level, spine_envelope, varied_span, varied_speed_mult) | Import list trimmed to the 8 live names; allow dropped; the doc comment lists what was masked and why it was safe |
| S1-4 | stale pointers (10 sites) | Comments pointing at pre-LOC-refactor paths: `chroma/shaders/transition.rs` (cloud/mod.rs), `tests/chroma_legacy_parity.rs` (legacy.rs — the parity tests actually live in legacy.rs's inline `mod tests`), `palette_floor_tests.rs` x3 (tuning.rs x2, lock_inv13_19.rs — actual home `palette/tests_floor_audit.rs`), `src/validation.rs` (config/mod.rs — now `src/validation/mod.rs`), `safepath.rs`/`verbose.rs` (configfile.rs — now `safepath/mod.rs`/`output/verbose.rs`), `chroma/post/climate.rs` (phosphor_anomaly.rs), `chroma/gradient.rs`+`chroma/shaders/base.rs` namespace pointers (shaders/mod.rs), stale `#[path=...]` load-mechanism doc (palette/tests_floor.rs) | All 10 pointers corrected to current paths. Historical-provenance comments ("was previously in X.rs") intentionally preserved — they describe history accurately |

### Verified NON-findings (audited, deliberately kept)

- Dual theme catalogs (`theme/mod.rs` ThemeInfo display data vs
  `chroma_dragon_engine/catalog/themes.rs` ThemeDef color data) —
  different data, both guarded by count/sweep tests
  (`catalog_count_is_current_theme_count`, `every_scheme_has_a_theme`).
- `legacy.rs` vs `palette/mod.rs` blend function twins — the documented
  chroma-first / legacy-fallback dual path (owner architecture), parity
  asserted by inline bit-exact tests.
- `sgr_format::push_u8/u16` delegating to `bolt::` — documented BOLT
  inline shims.
- LOC-split `pub(crate) use` re-exports with `allow(unused_imports)`
  (bench/mod.rs, testconf/mod.rs, monolith.rs test re-export) —
  documented Pattern-D→C convention keeping call sites stable.
- cfg-gated platform stubs (platform/mod.rs, bench_perf.rs,
  endurance_health.rs non-Linux arms).
- GLYPH_ENTRY_RAMP_DURATION_MS — test-referenced constant, doc rationale.

### A/B benchmark (Task 1)

| Metric | A (8617360) | B (S-master-1) | Delta |
|--------|-------------|----------------|-------|
| avg_fps | 91805.00 | 92770.83 | +1.05% |
| frame_entropy_bits | 3.2958 | 3.2962 | +0.01% |
| density_gini | 0.8960 | 0.8960 | -0.00% |
| dirty_cells_per_frame | 56.78 | 56.78 | -0.01% |
| active_streams_avg | 23 | 23 | 0 |
| alloc_calls | 563 | 563 | 0 |
| total_ns_per_cell | 191.82 | 189.85 | -1.03% |

Verdict: visual bit-parity (identical to RNG noise level); performance
within the natural variance band. The touched code (crystal control
wiring, import lists, comments) is not on the monolith bench hot path.

## Task 2 — S-master-2-v2 optimize code

### Method (staged, important dirs first)

Hot-path inventory of the per-frame pipeline (event loop -> rain_at ->
shader -> bolt/ansi writer):

1. Per-cell shader path: already LUT-backed end to end —
   TRAIL_EXPONENTIAL LUT (shaders/base), column_coherence_lut,
   precomputed phosphor decay exp factors (6/frame), BOLT branchless
   digit tables (U8_PADDED + U8_LEN), FlashWaveCtx with precomputed
   radii and fade and squared-distance early-out.
2. Allocation profile: 563 alloc calls over 922,938 frames
   (0.0006/frame) — steady state is zero-alloc; the remaining allocs
   are construction/resize-time Vec::with_capacity (monolith
   streams/previous_cells/current_cells).
3. Remaining transcendentals (all bounded, all deliberate):
   - droplet/draw.rs head-bloom gaussian exp — per Middle char within
     HEAD_BLOOM_CELLS of a head (max ~4 cells x ~23 streams), the
     visual signature itself;
   - droplet/mod.rs turbulence sin + startup-ease exp — per droplet,
     ~23 calls/frame, transitional (3 tau window);
   - rain_at.rs pause/resume/glyph-entry easing exp — once per
     transition event, not per frame;
   - flash-wave sqrt — early-out bounding circle skips ~75%.
   LUT-ing any of these would change output bits (visual regression,
   violating the no-99%-visual-change mandate) for sub-microsecond
   gains. Peak-constrained by design.
4. TODO/FIXME scan: zero markers repo-wide.

### Verdict: ALREADY AT PEAK — skipped per task brief

Past rounds already landed the measurable wins (S_master_dragon S2:
const-gate fog + direct-index vignette LUT, measured <1% = below the
noise floor; BOLT; PGO; zero-alloc steady state). Evidence for the
skip decision, control A/B on the identical tree (055a69f, zero code
change, 10s monolith 80x24 x2 runs):

| Metric | Run 1 | Run 2 | Delta (noise floor) |
|--------|-------|-------|---------------------|
| avg_fps | 92770.83 | 92789.49 | +0.02% |
| frame_entropy_bits | 3.2962 | 3.2937 | -0.08% |
| density_gini | 0.8960 | 0.8962 | +0.02% |
| dirty_cells_per_frame | 56.7773 | 56.7876 | +0.02% |
| total_ns_per_cell | 189.85 | 189.78 | -0.04% |
| alloc_calls | 563 | 563 | 0.00% (bit-stable) |

The run-to-run band (<=0.1% visual, ~1% fps including cross-build
variance) is narrower than any remaining optimization could measure.
Per the brief: skip, do not over-engineer. Raw JSON:
benchmark/bench-labs/S_master_v2/S2_control.json.

## Task 3 — S-master-3-v2 security LTS hardening

### Method (staged, important files first)

1. unsafe inventory: 35 sites / 14 files — all libc FFI with per-site
   SAFETY comments (sysinfo, fstat, localtime_r, madvise, perf
   counters, fork guard, io_uring in a rejected experiment file).
   No unsafe touches user-controlled memory layouts. No action.
2. Path security: safepath strict-whitelist (v14 policy, v16 traversal
   hardening, symlink-aware) — already LTS-grade. No action.
3. Input validation: charset-custom rejects control chars + non-1-width
   chars at parse (hard error); message text sanitized at BOTH entry
   points (CLI build + live-reload) via sanitize_message_text with
   MESSAGE_MAX_LEN=200; speed/fps/density ranges validated. Verified
   — the terminal-injection surfaces I probed were already closed.
4. Config read paths: FOUND THE GAP.

### Finding and fix (S3-1: unbounded config reads — OOM DoS vector)

Four read paths loaded the FULL config file into a String with no
size bound, even though the path is safepath-whitelisted:

- `configfile::load_config_file_full` (startup, called by main.rs
  multiple times) — user config + the /etc fallback;
- `live_config/watcher.rs` reparse (fires on every file-change event);
- `event_loop_ambient.rs` ground-truth check — re-reads the file every
  5 s while an ambient entry is applied, so a multi-GB file (runaway
  append, log-rotation mistake, or hostile file planted in a
  whitelisted dir on a shared machine) would thrash/OOM the process
  repeatedly, not just once.

Fix: `config_io::read_config_capped()` — a shared reader enforcing
`CONFIG_FILE_MAX_BYTES` (1 MiB, ~100x the typical 1-10 KB config, and
above the documented sha2 hashing window expectations). The cap is
enforced via `Read::take` (bounded syscalls, no metadata-then-read
TOCTOU window — a concurrently growing file stays bounded). Oversized
files map to the existing unreadable-file semantics: startup falls
back to defaults, the watcher skips the reparse, the ambient check
skips. All four call sites converted. Also fixed en route: the
stale live_config_poll doc comment claiming each poll does a full
`read_to_string` (the poll path hashes an 8 KiB prefix only), and a
`&PathBuf` -> `&Path` signature fix the new reader surfaced
(clippy ptr_arg).

### A/B benchmark (Task 3)

| Metric | A (baseline) | B (S-master-3) | Delta |
|--------|--------------|----------------|-------|
| avg_fps | 91805.00 | 92376.52 | +0.62% |
| frame_entropy_bits | 3.2958 | 3.2949 | -0.03% |
| density_gini | 0.8960 | 0.8961 | +0.01% |
| dirty_cells_per_frame | 56.78 | 56.80 | +0.03% |
| active_streams_avg | 23 | 23 | 0 |
| alloc_calls | 563 | 564 | +0.18% (startup, +1 alloc) |
| total_ns_per_cell | 191.82 | 190.57 | -0.65% |

Verdict: visual bit-parity; performance within the natural variance
band (render_ns_per_cell +4.96% is single-sub-metric jitter — the
stable aggregates total_ns_per_cell and avg_fps both improved
slightly; the config-cap code is not on the per-frame path). Raw
JSON: benchmark/bench-labs/S_master_v2/after_S3.json.

## Task 4 — S-master-4-v2 stability / LTS potential-gain audit

### Method (staged, important dirs first)

Audit axis for this task: LONG-RUN ROBUSTNESS — what keeps the
process alive and the user's terminal intact across months of LTS
deployment. Deps stage, then src stability-critical subsystems.

1. **Panic-path inventory (src/*, production code only).**
   Script-scoped scan (comments + `#[cfg(test)]` blocks stripped,
   test-only files excluded) for `unwrap`/`expect`/`panic!`/
   `unreachable!`/`todo!`/`unimplemented!`/`assert*!`: 33 sites
   across 15 production files. Every site read and classified:
   - 24x `Uniform::new*` / constructor expects with constant or
     guarded arguments (swap-before-use, `max(1)` clamps,
     `saturating_sub`) — safe by construction;
   - 4x `scene_custom` split-expects behind `is_profile_config_key`/
     `is_scene_custom_config_key` predicates that perform the exact
     same split and return `false` on failure — unreachable on
     unvalidated input;
   - 3x checked-Option patterns (`expect("checked is_err above")`,
     `expect("set above")`, `expect("just pushed")`);
   - 1x `live_config_trace.rs` `(None, None) => unreachable!()` in a
     diff visitor where the entry exists by iteration contract;
   - 1x `theme/mod.rs` `assert_eq!` on duplicate alias registration
     of a static table — fires only on a table-authoring mistake,
     caught by the test suite before release.
   ZERO genuine runtime panic risks. No fix warranted.
2. **Panic hook / terminal restore.** `platform/panic_hook.rs` is
   v25-hardened: restores the terminal BEFORE printing (Windows
   silent-exit fix), uses `write_fmt` with errors discarded so the
   hook itself can never double-panic -> abort -> coredump (the
   documented journal-entry root cause). `Terminal::drop` honors
   `TERMINAL_RESTORED_BY_PANIC` to skip double cleanup. No action.
3. **Signal coverage.** SIGTERM/SIGHUP/SIGQUIT -> graceful shutdown
   with a bounded 3s signal-thread wait (calibrated to the 2s
   watchdog + 1s grace); SIGTSTP/SIGCONT -> suspend/resume with
   mouse-capture release and terminal reinit; SIGINT intentionally
   not an exit (only 'q'); Windows ctrlc path; `fork_guard.rs`
   PDEATHSIG protects forked children. No action.
4. **Thread lifecycle.** All production threads are named
   (`cx-shutdown-guard`), wrapped in `catch_unwind`
   (live-reload watcher, ambient scheduler) or flag-bounded
   (signal threads, watchdog with dead-PTY probe, shutdown guard
   with 10s force-exit). Watcher/ambient schedulers spawn ONCE at
   event-loop entry — no per-scene-switch thread accumulation. No
   action.
5. **Lock discipline.** Every shared-state lock site uses a
   poison-tolerant pattern: `if let Ok(guard) = ...lock()` (skip on
   poison), `unwrap_or_else(|e| e.into_inner())` (recover on
   poison), or `unwrap()` on a test-only mutex. Zero
   poisoning-propagation paths. No action.
6. **Channel bounds.** All mpsc usage is `sync_channel(64)` —
   bounded queues with `try_send` + `Disconnected`/`Full`
   backpressure handling (watcher, ambient scheduler). Zero
   unbounded channels. No action.
7. **Constructor-vs-resize window.** `Cloud`'s constructor seeds
   80x24-shaped `Uniform` defaults then immediately calls
   `reset_with_bounds(cols, lines, ...)` — no frame is ever sampled
   against the hardcoded defaults. No action.
8. **Deps / toolchain LTS health.** All 12 production deps pinned
   (clap `>=4.5,<4.6`, notify `>=7,<8` with per-OS feature matrix,
   crossterm 0.29 ...); the single RUSTSEC suppression
   (RUSTSEC-2024-0384 `instant` via notify-types) is documented with
   rationale; `cargo audit` + `cargo deny` run in CI; toolchain
   pinned 1.98.0 with an MSRV-sync gatekeeper. No action.
9. **Open-issue cross-check.** KNOWN_ISSUES.md lists only platform
   event-delivery quirks (Windows/Termux 'i'-key exit — partial
   mitigation v30, root cause upstream); FUTURE_BACKLOG.md items
   are closed/obsolete/parked. No stability-critical open item.

### Verdict: ALREADY AT PEAK — zero code changes, skipped per brief

The stability architecture is comprehensively hardened across all
nine staged axes. Every candidate "gain" I probed (the 33 panic
sites, the constructor-default uniforms, the signal-thread bounds)
documents its own invariant and would return churn without a
measurable robustness delta — the definition of over-engineering
the owner's brief forbids.

### A/B benchmark (Task 4 — control pair on identical tree)

Baseline A captured at 22b9417 BEFORE the audit; B run AFTER on the
byte-identical tree (audit made zero changes — the control pair
proves it):

| Metric | A (22b9417) | B (control) | Delta |
|--------|-------------|--------------|-------|
| avg_fps | 93068.98 | 92995.68 | -0.08% |
| frame_entropy_bits | 3.2953 | 3.2960 | +0.02% |
| density_gini | 0.8961 | 0.8956 | -0.05% |
| dirty_cells_per_frame | 56.76 | 56.83 | +0.14% |
| active_streams_avg | 23 | 23 | 0 |
| alloc_calls | 563 | 563 | 0.00% (bit-stable) |
| dealloc_calls | 553 | 553 | 0.00% (bit-stable) |
| total_ns_per_cell | 189.32 | 189.21 | -0.06% |
| frame_time_stability | excellent | excellent | stable |

Verdict: all deltas inside the established run-to-run noise band
(<=0.1% visual metrics, <=1% fps); allocator counts bit-stable.
Raw JSON: benchmark/bench-labs/S_master_v2_v2/baseline_S4.json and
after_S4_control.json.

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
