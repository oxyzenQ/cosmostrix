<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Dragon Hunt v2 — Deep Bore Audit for Cleanup

**Repo**: cosmostrix
**Audit date**: 2026-08-04
**Methodology**: 5 parallel Explore agents covering distinct dimensions
**Supersedes**: Dragon Hunt v1 (commit `f326ac1`, Phase 6 dead-code sweep)

## Execution Status

| Phase | Tier | Status | Commits | LOC recovered |
|-------|------|--------|---------|---------------|
| 1 | S (batch 1) — dead code deletions | ✅ DONE | `841ebd4` | -701 LOC |
| 1 | S (batch 2) — doc accuracy | ✅ DONE | `d6cb36f` | 13 doc fixes |
| 2 | A — atmosphere triage | ✅ DONE | `e74b71b` | -28 LOC, 29 warnings surfaced + silenced |
| 2 | A — file consolidation | ✅ DONE | `38506b4` | -123 LOC, -2 files |
| 2 | A — related_schemes() deletion | ✅ DONE | `3feb68f` | -71 LOC |
| 3 | B (batch 1) — archive 11 closed docs | ✅ DONE | `dde68de` | -11 docs to archive/ |
| 3 | B (batch 2) — delete stale benchmark artifacts | ✅ DONE | `f509d3c` | -7 regenerated files |
| 3 | B (batch 3) — archive CHANGELOG pre-v13 | ✅ DONE | `4e5c55f` | -630 LOC live, +703 archive |
| 3 | B (batch 4) — condense FUTURE_BACKLOG.md | ✅ DONE | `d2ed7a4` | -259 LOC (341→82) |
| 4 | C (item 25) — drop crossterm derive-more | ✅ DONE | `4879585` | -53 LOC Cargo.lock, ~0.8s |
| 4 | C (item 26) — drop clap suggestions | ✅ DONE | `5d40a9d` | -7 LOC Cargo.lock, ~0.6s |
| 4 | C (item 27) — drop build-dep chrono | ✅ DONE | `efea502` | -3 LOC Cargo.toml, ~1.3s |
| 4 | C (item 28) — add [lints.clippy] table | ✅ DONE | `80c0a80` | -1 file-level allow |
| 5 | D (item 29) — precompute column_coherence sinf LUT | ✅ DONE | `96d2213` | -65-130M cycles/sec (architectural), visual parity verified |
| 5 | D (item 30) — replace per-frame scene_name clone with u64 counter | ✅ DONE | `8805f26` | -60 heap allocs/sec |
| 6 | E (item 31) — atmosphere subsystem archival + dead code deletion | ✅ DONE | (this commit) | -1,071 LOC (3 files), design knowledge preserved in `docs/archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md` |
| 6 | E (item 32) — Bold default | ✅ DONE (earlier) | — | Closed with Option B (Random), no code change |
| 6 | E (item 33) — vignette/rain_shadow LUTs | ⏸ DEFERRED | — | Visual shift risk, needs owner approval |
| 6 | E (item 34) — `bitvec` dep replacement | ✅ DONE (SKIP) | — | Owner keeps `bitvec`, no comparison branch, no code change |

**Item 16** (`cosmic_dragon_incubator/egg/io_uring_rejected.rs` demotion) was
**INTENTIONALLY SKIPPED** — the `egg/mod.rs` has an explicit policy:
"When an experiment concludes, its findings are documented in
`docs/COSMIC_DRAGON_FINDINGS.md` and the benchmark itself stays here as
a reproducible record." File is `#[cfg(test)]` so zero production cost;
demoting to markdown would lose reproducibility.

**Phase 2 result**: release build now ZERO warnings (was 29 hidden).
Test build ZERO warnings (was 3). All 1543 tests pass on every commit.

---

## 0. Why v1 Found "0 Dead Code" — The Structural Blind Spot

Dragon Hunt v1 ran `cargo clippy -W dead_code` and reported "0 dead code".
The owner was not satisfied. v2 confirms the owner's instinct was correct.

**v1 was structurally blind** because:

1. **Lint suppression attributes** — `#![allow(dead_code)]` appears at module
   level in **6 atmosphere files** and 4 other files. These blanket-suppress
   the lint, so `cargo clippy` literally cannot see the dead items inside.
   v2 temporarily stripped the attributes and re-ran clippy:
   **29 hidden dead-code warnings surfaced** (2 survive even with `--tests`).

2. **Compiler lint scope** — `cargo clippy` only scans `.rs` files. It cannot
   see orphan Python scripts, stale markdown, bloated GIFs, broken doc
   references, or feature-flag bloat in `Cargo.toml`.

3. **"Used" != "Earns place"** — v1 verified each dep is imported, but did
   not ask "is the dep's enabled feature set minimal?" or "is this 71-LOC
   file justified when it's only re-exported once?"

4. **Forward-compat scaffolding frozen mid-flight** — the `atmosphere_*`
   subsystem has been `#![allow(dead_code)]` since Phase 3/4 (26
   versions ago) because it was "not yet wired into the hot render path".
   Production callers do `let _ = verify_application(...)` — **discarding
   the result**. This is the largest single cluster of attribute-suppressed
   dead code in the project.

---

## 1. Audit Dimensions & Headline Findings

| Task ID | Dimension | Findings | Recoverable LOC |
|---------|-----------|----------|-----------------|
| 6-a | Module & file redundancy | 10 targets | ~1,500 LOC |
| 6-b | Docs / assets / folder bloat | 10 targets + 4 bonus | ~5,850 LOC + 13 MB GIF |
| 6-c | Build system & dep bloat | 5 targets + 5 optional | ~2.7s build + 22KB binary |
| 6-d | Hot-path / runtime bottlenecks | 26 findings (3 HIGH, 12 MED) | 60-130M cycles/sec |
| 6-e | Legacy / version-marker / compat-shim | 26 findings | ~200 LOC + 6 doc fixes |

---

## 2. Consolidated Top-30 Cleanup Targets (Prioritized)

### Tier S — Zero-risk quick wins (≤30 min each, zero build risk)

| # | Target | Action | Impact |
|---|--------|--------|--------|
| 1 | `src/bench_tune.rs` (227 LOC) | DELETE — implements removed `--tune-visual` (v14.0.0) | -227 LOC |
| 2 | `scripts/smooth_body_tail_gap.py` + `normalize_head_stops.py` (239 LOC) | DELETE — both target deleted `src/central_colors.rs`, will crash if run | -239 LOC |
| 3 | `README.md:137-148` duplicate `## Philosophy` section | DELETE — pure subset of lines 89-135 | -12 LOC |
| 4 | `README.md:132` wrong `--duration 1h30m` doc | FIX — `--duration` is bare-float-only; compound format is `--bench-duration`'s feature | doc accuracy |
| 5 | `src/configfile.rs:88` stale "Legacy `config` filename" comment | FIX — `CONFIG_FILE_NAME_LEGACY` was removed | doc accuracy |
| 6 | `docs/research/FLAGS_AUDIT_dead_weight.md:10-19` stale "NOT YET FIXED" status | FIX — bug was fixed in commit `295a725` | doc accuracy |
| 7 | 3× stale README refs to `src/cosmic_dragon_lock_tests.rs` (Task 4 fallout) | FIX — path is now `src/cosmic_dragon_incubator/lock_tests.rs` | doc accuracy |
| 8 | `Cargo.toml` `[profile.bench]` (5 lines) | DELETE — 0 `#[bench]` tests, 0 `cargo bench` invocations | -5 LOC |
| 9 | `cloud/tests/tests_architecture.rs` (210 LOC) | DELETE or wire into `cloud/tests/mod.rs` — currently NOT mod-declared, never runs | -210 LOC or fix |
| 10 | `cloud/ecosystem.rs:221-263` `related_schemes()` (43 LOC) | DELETE — "preserved as documentation", family-view already captures same clustering disjointly | -43 LOC |

### Tier A — Safe refactors (≤1 hr each, low risk)

| # | Target | Action | Impact |
|---|--------|--------|--------|
| 11 | 6× `#![allow(dead_code)]` in atmosphere files | REMOVE attrs + triage the 29 surfaced warnings one-by-one | surfaces real cleanup |
| 12 | `src/branding.rs` (52 LOC) | INLINE into `info.rs` (1 fn, 1 caller) | -52 LOC, -1 file |
| 13 | `src/quantum_constants.rs` (71 LOC) | MERGE into `constants.rs` (only re-exported once) | -71 LOC, -1 file |
| 14 | `atmosphere_probe.rs` (223 LOC) `evaluate_probe` method | DELETE — test-only API that never shipped | -223 LOC |
| 15 | `atmosphere_presets.rs` — 4 dead fns (`expects_identity`, `expects_whisper`, `is_storm_preset_name`, ...) | DELETE | ~80 LOC |
| 16 | `cosmic_dragon_incubator/egg/io_uring_rejected.rs` (161 LOC) | DEMOTE to `docs/research/io_uring_rejected.md` — concluded experiment, kept as historical note | -1 .rs file |
| 17 | `assets/cosmostrix-v30-demo.gif` (16 MB) | RE-ENCODE to ~3 MB (saves 13 MB per clone) | -13 MB |

### Tier B — Doc archival (low risk, high LOC recovery)

| # | Target | Action | Impact |
|---|--------|--------|--------|
| 18 | 7× `CONFIG_SYNC_AUDIT_PHASE{1..6,5_FINAL}.md` (3,715 LOC) | ARCHIVE to `docs/archive/CONFIG_SYNC/` — closed-phase reports, zero live consumers | -3,715 LOC from live tree |
| 19 | `docs/COSMIC_DRAGON_EXPLORATION.md` (389 LOC) | ARCHIVE — references obsolete version numbers; conclusions already in PHILOSOPHY.md | -389 LOC |
| 20 | `docs/COSMIC_DRAGON_FINDINGS.md` (242 LOC) | ARCHIVE — measurements superseded by PERFORMANCE_ACROSS_SCALES.md | -242 LOC |
| 21 | `UNSAFE_SOUNDNESS_AUDIT.md` + `FLAGS_AUDIT_dead_weight.md` (873 LOC) | ARCHIVE — closed reports with zero live refs (except 1 user-facing error msg in `validation.rs:74`) | -873 LOC |
| 22 | `benchmark/hyperfine.md` + 4× `perf|time-*.txt` + `cloud-xeon/` (33 KB) | DELETE + ARCHIVE — 1-shot artifacts with hardcoded dead paths | -33 KB |
| 23 | `CHANGELOG.md` (112 KB, 2,404 LOC, 36 versions) | ARCHIVE pre-v13 (~700 LOC). MUST preserve v3.9.0 + v4.0.0 + "568 deterministic tests" (test-locked by `docs_tests/metadata.rs`) | -700 LOC from live tree |
| 24 | `docs/research/FUTURE_BACKLOG.md` (341 LOC) | CONDENSE to ~50 LOC — closure-status table duplicated in PHASE5_FINAL | -291 LOC |

### Tier C — Build system optimization (medium risk, compile-time win)

| # | Target | Action | Impact |
|---|--------|--------|--------|
| 25 | crossterm default `derive-more` feature | `default-features = false, features = ["bracketed-paste", "events", "windows"]` | ~0.8s build |
| 26 | clap default `suggestions` feature | `default-features = false, features = ["std", "color", "help", "usage", "error-context", "derive"]` | ~0.6s build |
| 27 | `[build-dependencies] chrono` | Replace `build.rs:72-73` with `std::time::SystemTime` + 30-LOC formatter | ~1.3s build, -1 build-dep |
| 28 | Add `[lints]` table to Cargo.toml | Centralize `#![allow(...)]` decisions; removes need for file-level attrs | Lint-time consistency |

### Tier D — Hot-path optimizations (medium risk, perf win)

| # | Target | Action | Impact |
|---|--------|--------|--------|
| 29 | `chroma_dragon_engine/shaders/base.rs:298` per-cell `sinf` | Precompute `Vec<i32>` of length `cols` once per frame; pass through `DrawCtx` | -65-130M cycles/sec |
| 30 | `interactive/event_loop.rs:558` per-frame `String::clone` for scene_name | Replace with `u64` generation counter | -60 heap allocs/sec |

### Tier E — Decisions requiring owner input

| # | Target | Decision needed |
|---|--------|----------------|
| 31 | Atmosphere subsystem (12 files, ~4,894 LOC) frozen since Phase 3/4 | Owner decides: **(a) graduate** — wire `verify_application` result into render path; **(b) slim** — delete the unused half; **(c) archive** — document as "intentional reservation" and stop maintaining |
| 32 | Bold default (per MATRIX_BOLD_AUDIT.md) | Owner already picked **Option B** (keep `BoldMode::Random` default). Closed. |
| 33 | `droplet.rs:913+882` per-cell `vignette_factor` + `rain_shadow_factor` (sqrt+smoothstep) LUTs | Visual output may shift slightly — owner approval needed before changing |
| 34 | `bitvec` dep (1.3MB compile cost) | Replace with hand-rolled `Vec<u64>` bitset? Saves ~4s compile, medium risk |

---

## 3. Phased Execution Plan

### Phase 1 — Quick wins (Tier S, items 1-10)
**Risk**: Zero. **Time**: ~2 hrs. **LOC recovered**: ~750.
- All changes are deletions or doc fixes.
- Each item committed separately with clear message.
- `cargo build --release` + `cargo test` must pass after each commit.

### Phase 2 — Safe refactors (Tier A, items 11-17)
**Risk**: Low (atmosphere triage is the only non-trivial part). **Time**: ~4 hrs.
- Item 11 (atmosphere `#![allow(dead_code)]` triage) is the highest-value
  item in the entire audit. Each of the 29 surfaced warnings needs a
  triage decision: delete / wire-in / re-document.
- Items 12-16 are mechanical (inline / merge / delete).
- Item 17 (GIF re-encode) requires `ffmpeg` or `gifsicle`.

### Phase 3 — Doc archival (Tier B, items 18-24)
**Risk**: Low (files moved, not deleted). **Time**: ~1 hr.
- Create `docs/archive/` structure.
- Move (not delete) closed-phase docs.
- Update `docs/README.md` index.
- Verify `src/docs_tests/assets.rs` + `src/docs_tests/metadata.rs` test-lock
  tripwires still pass (these enforce CHANGELOG content + asset presence).

### Phase 4 — Build optimization (Tier C, items 25-28)
**Risk**: Medium (feature-flag changes can break compilation). **Time**: ~2 hrs.
- Each Cargo.toml change is 1-line, but must `cargo build` after each.
- `[lints]` table addition is mechanical.

### Phase 5 — Hot-path optimization (Tier D, items 29-30)
**Risk**: Medium (visual output may shift). **Time**: ~3 hrs.
- Item 29 (sinf LUT): should be visually identical (same math, just cached).
- Item 30 (String::clone → u64): pure refactor, no visual change.
- Both need `cargo bench` before/after comparison.

### Phase 6 — Owner decisions (Tier E, items 31, 33, 34)
**Risk**: High. **Time**: Owner-dependent.
- Pause and present findings to owner.

**Item 31 — CLOSED 2026-08-05** with **Option C (archive) + code deletion**.
The owner chose to archive the atmosphere subsystem as an intentional
reservation AND delete the truly dead/test-only modules (1,071 LOC across
3 files: `atmosphere_ab.rs`, `atmosphere_probe.rs`, `atmosphere_tests/atmosphere_ab.rs`).
The wired-in modules (~6,255 LOC across 15 files) remain in `src/` because
they have live production callers in `main.rs`, `app.rs`, `bench_report.rs`,
`profile.rs`, `event_loop.rs`, `testconf.rs`, `live_config.rs`. Full
archival record (design knowledge, deleted module contents, rationale)
preserved in `docs/archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md`.

**Item 32 — CLOSED** earlier with Option B (keep `BoldMode::Random`
default). No code change.

**Item 33 — DEFERRED** (per-cell `vignette_factor` + `rain_shadow_factor`
sqrt+smoothstep LUTs). Visual shift risk requires owner approval before
changing. Not actionable in this session.

**Item 34 — CLOSED 2026-08-05** with **SKIP**. The owner decided to keep
using `bitvec` and NOT create a comparison branch. Rationale: `bitvec` is
stable, well-tested, and the ~4s compile-time saving from a hand-rolled
`Vec<u64>` bitset is not worth the medium risk of introducing a custom
data structure with different semantics. The `bitvec` dep remains in
`Cargo.toml` unchanged.

---

## 4. Test-Lock Tripwires (Critical Awareness)

Before any cleanup commit, be aware that the test suite actively enforces
some content presence:

1. **`src/docs_tests/assets.rs`** — 7 assertions enforcing all 7 demo
   assets exist + no v1-v29 assets survive. Any asset deletion/rename must
   update this file in the same commit.

2. **`src/docs_tests/metadata.rs`** — 5 assertions enforcing CHANGELOG
   content (specifically v3.9.0 + v4.0.0 + "568 deterministic tests"
   string). Any CHANGELOG archival must preserve these strings.

3. **`src/validation.rs:74`** — user-facing error message references
   `FLAGS_AUDIT_bench-frames_chars_bold.md` by name. Do NOT delete or
   rename that file without updating the error message.

4. **`aur/cosmostrix-bin/PKGBUILD` + `.SRCINFO`** — actively synced via
   `aur.yml` workflow. Any version bump must update both files together.

5. **`src/cli.rs:289`** — hand-rolled `edit_distance` function. If clap's
  `suggestions` feature is re-enabled, this becomes dead code.

---

## 5. Audit Methodology Notes

### Why v1 was structurally blind

| v1 check | v2 improvement |
|----------|----------------|
| `cargo clippy -W dead_code` | v2 also stripped `#![allow(dead_code)]` attrs to surface hidden warnings |
| `rg` for dep imports | v2 also checked feature-flag minimality (clap `suggestions`, crossterm `derive-more`) |
| `rg` for `TODO\|FIXME` | also checked version markers and matched against CHANGELOG to identify expired compat windows |
| File-by-file lint | v2 also did **reference-graph audit**: for each file, "who imports this?" — found orphan test files, orphan Python scripts, orphan docs |
| Compiler-only | v2 also audited non-`.rs` artifacts: 16 MB GIF, 112 KB CHANGELOG, 6 dead-dragon-ab JSON files |

### Dimensions NOT covered by v2 (deferred to v3 if needed)

- **Fuzzing / property-test coverage gaps** — not a cleanup dimension.
- **Cross-platform build verification** — only Linux verified; macOS/BSD
  build paths need separate audit.
- **Security advisory database scan** — `cargo deny check advisories` not
  run (cargo-deny not installed). 3 duplicate-version deps in Cargo.lock
  flagged but not actionable until upstream `notify` v7 lands.
- **Binary size profiling** — `cargo bloat` not run. 2.3 MB binary could
  likely shed 100-200 KB with `--strip` and LTO settings tuning.

---

## 6. Bottom Line

**v1's "0 dead code" was technically true at the compiler level, but
false at the project-hygiene level.** v2 found:

- **~1,500 LOC of dead/burden code** in `.rs` files (hidden behind lint suppression)
- **~5,850 LOC of stale docs/assets** that should be archived
- **~13 MB of binary bloat** in one GIF asset
- **~2.7s of compile-time waste** from over-broad dep features
- **~60-130M cycles/sec of runtime waste** in the render hot path

Phased execution starting with Tier S (zero-risk quick wins) can recover
~750 LOC in ~2 hrs with zero build risk. Deeper phases require owner
input on the atmosphere subsystem decision (Tier E, item 31).
