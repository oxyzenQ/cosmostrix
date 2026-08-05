<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Atmosphere Subsystem — Archival Record

**Archival date**: 2026-08-05
**Decision**: Dragon Hunt v2 Phase 6 Tier E, Item 31 — Option C (archive as
reservation) **with code deletion** of the truly dead/test-only modules.
**Supersedes**: live maintenance of the frozen atmosphere scaffolding.
**Canonical design spec**: `docs/ATMOSPHERE_ENGINE.md` (still live, still
authoritative for the wired-in atmosphere features).

---

## 1. Why This Document Exists

The atmosphere subsystem was conceived in v4.0.0 as a nine-phase plan to
give cosmostrix a slow-moving "visual climate" layer that could modulate
rain speed, density, brightness, and (eventually) color according to a
regime state machine (calm / pulse / signal / compression / void /
monolith-pressure / storm-rejected / adaptive). The plan called for a
verifier layer, a controlled-live application path, a visual whisper
adapter, an A/B smoke harness, and a regime probe — all to be landed
incrementally and gated behind explicit opt-in.

Across 26 versions (v4.0.0 → v30.0.0) the plan was **partially delivered**:
the regime enum, the verifier, the controlled-live modulation, the adaptive
hour-driven path, the custom time map, and the shadow metrics were wired
into production. The A/B smoke harness and the regime probe were landed
but **never connected** to the live render path — they remained
`#![cfg_attr(not(test), allow(dead_code))]` and were only exercised by
tests.

Dragon Hunt v2 (2026-08-04) audited the subsystem and surfaced 29 hidden
dead-code warnings behind the module-level allow attributes. Phase 2 of
the audit triaged those warnings and silenced the legitimate ones with
fine-grained reasons. Phase 6 Tier E item 31 then asked the owner to
decide: graduate, slim, or archive.

The owner picked **Option C with deletion**: document the design, then
delete the code that is genuinely dead. This file is the documentation.
The deletions are listed in §4 below.

---

## 2. What the Atmosphere Subsystem Was Designed to Be

The atmosphere engine models the visual terminal as a **regime state
machine** that breathes slowly. Each regime maps to a bounded set of
multipliers on rendering parameters:

| Regime | Speed mult | Density mult | Brightness bias | Glitch mult |
|--------|-----------|--------------|-----------------|-------------|
| `calm` | 1.0 | 1.0 | 0.0 | 1.0 |
| `pulse` | 1.0–1.06 | 1.0 | 0.0–0.03 | 1.0 |
| `signal` | bounded | bounded | bounded | bounded |
| `compression` | bounded | bounded | bounded | bounded |
| `void` | <1.0 | 0.95–1.0 | 0.0 | 1.0 |
| `monolith-pressure` | bounded | bounded | bounded | bounded |
| `storm` | **rejected** | **rejected** | **rejected** | **rejected** |
| `adaptive` | time-driven | time-driven | time-driven | time-driven |

The regime state machine enforces:

- **Minimum dwell time** (`REGIME_MIN_DWELL_SECS >= 5.0`) — a regime cannot
  transition away until it has been active for at least 5 seconds.
- **Transition ramp** (`REGIME_TRANSITION_RAMP_SECS >= 0.5`) — regime
  changes are interpolated over at least 0.5 seconds to avoid visual jumps.
- **Color lock** — color modification is always stripped regardless of
  input. Color changes are opt-in only via `--auto-color-drift` or the
  adaptive engine, never via a regime.
- **Terminal effect lock** — terminal behavior is never affected by
  atmosphere presets.
- **Storm rejection** — `storm` does not exist as a regime and must never
  be added. The `atmosphere-storm` preset is rejected at every layer.

The application modes gate how (and whether) the regime is wired into the
runtime:

- `disabled` (default) — modulation is always identity. Zero visual change
  from v3.9.0.
- `controlled-live` — regime modulation is applied with conservative
  bounds (±4% speed, ±4% density, ±3% brightness).
- `adaptive` — modulation is seeded from the local hour via
  `atmosphere_adaptive::adaptive_params()` and snapped at startup.

The **visual whisper** is a tighter adapter (±2% speed, ±2% density,
±1.5% brightness) that was intended as the first controlled visual
modulation path. It converts a verified `AtmosphereRuntimeModulation`
into ultra-subtle, tightly bounded visual-safe values. The whisper is
identity by default and never allows color change or terminal effects.

The **regime probe** was a deterministic pure function that mapped
observable runtime facts (dirty cell ratio, active streams, p99 frame
time) to candidate regimes. It was intended to drive automatic regime
selection. It was landed but never wired into the production render path.

The **A/B smoke harness** was a deterministic, test-only comparison layer
that captured baseline (identity) vs candidate (whisper) samples and
produced pass/reject verdicts. It was intended to prove the whisper path
was bounded and safe before any public activation. It was landed but
never wired into anything outside tests.

---

## 3. What Was Actually Wired In (Kept)

These modules have live production callers and **cannot be deleted**
without a major refactor that would remove user-facing features. They
remain in `src/` and are still maintained.

| File | LOC | Live callers | Purpose |
|------|-----|--------------|---------|
| `src/atmosphere.rs` | 406 | `main.rs`, `live_config.rs`, `config_apply_tests` | `AtmosphereRegime` enum, `RegimeParams`, `AtmosphereState`, `AtmosphereController` |
| `src/atmosphere_adaptive.rs` | 474 | `main.rs` | Hour-driven `adaptive_params()`, `update_modulation()`, `current_hour()` |
| `src/atmosphere_apply.rs` | 249 | `main.rs`, `live_config.rs`, `app.rs`, `bench_report.rs` | `AtmosphereRuntimeModulation`, `AtmosphereApplicationMode`, re-exports `derive_effective_runtime` |
| `src/atmosphere_controlled_live.rs` | 117 | `main.rs` | `controlled_live_modulation_from_regime()` |
| `src/atmosphere_custom.rs` | 976 | `interactive/event_loop.rs`, `testconf.rs` | `CustomTimeMap`, `reparse_if_changed()`, `parse_custom_time_map()` |
| `src/atmosphere_presets.rs` | 278 | `profile.rs` | `all_atmosphere_presets()` for `--list-profiles` |
| `src/atmosphere_runtime.rs` | 107 | `app.rs`, `bench_report.rs` (via re-export) | `AtmosphereEffectiveRuntime`, `derive_effective_runtime()` |
| `src/atmosphere_shadow.rs` | 519 | `bench_report.rs`, `config_apply_tests` | `shadow_metrics_from_mode_and_regime()`, `AtmosphereShadowRisk` |
| `src/atmosphere_verifier.rs` | 543 | `bench_report.rs`, `atmosphere.rs`, `atmosphere_controlled_live.rs`, `atmosphere_shadow.rs` | `verify_application()`, `AtmosphereApplication`, `AtmosphereBounds` |
| `src/atmosphere_visual.rs` | 607 | `atmosphere_shadow.rs` | `AtmosphereVisualWhisper`, `VisualWhisperBounds`, `visual_whisper_from_regime()` |
| `src/atmosphere_tests/atmosphere_apply.rs` | 643 | test runner | Tests for `atmosphere_apply` + `derive_effective_runtime` |
| `src/atmosphere_tests/atmosphere_apply_cl.rs` | 236 | test runner | Tests for controlled-live modulation |
| `src/atmosphere_tests/atmosphere_expansion.rs` | 812 | test runner | Expansion / property tests for atmosphere invariants |
| `src/atmosphere_tests/mod.rs` | 347 | test runner | Umbrella module + regime/state/controller unit tests |
| `src/chroma/post/atmosphere.rs` | 547 | `chroma::post::apply_atmospheric()` (render path) | Live atmospheric post-FX (luminance/saturation/instability) — **separate subsystem**, not part of the v4.0.0 plan |

**Total kept**: ~6,255 LOC across 15 files.

The `verify_application()` call in `bench_report.rs:795` discards its
return value (`let _ = ...`), which is the "discarded result" pattern
the Dragon Hunt v2 audit flagged. This is intentional: `verify_application`
takes `&mut AtmosphereApplication` and performs in-place clamping and
validation. The discarded return is the verification verdict; the side
effect on the mutable application is what the bench report reads. The
verifier is therefore **not** dead — it just has a non-obvious contract.

---

## 4. What Is Being Deleted (Truly Dead / Test-Only)

These modules have **zero production callers**. They are only referenced
by themselves and by test files that exist solely to test them. Deleting
them removes ~1,071 LOC of frozen scaffolding with no behavioral change.

| File | LOC | Why it's safe to delete |
|------|-----|-------------------------|
| `src/atmosphere_ab.rs` | 368 | Internal A/B smoke model (Phase 9). Module header explicitly says: "Test-only: the A/B smoke functions are only called from tests. No public CLI flag, no config key, no runtime default change." Only caller is `src/atmosphere_tests/atmosphere_ab.rs`. |
| `src/atmosphere_probe.rs` | 221 | Regime probe and selection. Module header explicitly says: "Test-only API surface: probe types are consumed by atmosphere_tests/* but not yet wired into the production render path." Has `#![cfg_attr(not(test), allow(dead_code))]`. Zero `use crate::atmosphere_probe` statements anywhere in `src/`. |
| `src/atmosphere_tests/atmosphere_ab.rs` | 482 | Tests for `atmosphere_ab`. Must be deleted together with its parent module. |

**Total deleted**: 1,071 LOC across 3 files.

### Module declarations removed

- `src/main.rs:48` — `mod atmosphere_ab;`
- `src/main.rs:54` — `mod atmosphere_probe;`
- `src/atmosphere_tests/mod.rs:15` — `mod atmosphere_ab;`

### Files updated

- `src/atmosphere_visual.rs:38` — comment reference to `atmosphere_ab.rs`
  updated to past tense ("was consumed by the now-deleted
  `atmosphere_ab.rs` test module").
- `src/atmosphere.rs:35` — comment reference to `RegimeProbe` updated to
  note the probe module has been deleted.

---

## 5. Design Knowledge Preserved (from Deleted Modules)

### 5.1 Regime Probe Design (from `atmosphere_probe.rs`)

The regime probe was designed to map observable runtime facts to a
candidate regime. The probe fields were:

- `dirty_cell_ratio: f64` — fraction of dirty cells (0.0..1.0)
- `active_streams: usize` — number of active droplet streams
- `p99_frame_time_ms: f64` — p99 frame time in milliseconds (0.0 if unknown)

The selection function `select_regime_from_probe()` was a deterministic
pure function that mapped these facts to a candidate regime. The mapping
was never finalized — the probe was landed as scaffolding, and the
selection logic was deferred until the probe was actually wired in.

If a future owner wants to revive automatic regime selection, the probe
fields above are the right starting point. The selection logic should be
bounded by the same `REGIME_MIN_DWELL_SECS` and `REGIME_TRANSITION_RAMP_SECS`
invariants that govern explicit transitions.

### 5.2 A/B Smoke Model Design (from `atmosphere_ab.rs`)

The A/B smoke model compared a baseline (identity / Calm / Disabled) path
against a candidate (controlled whisper from a specific regime under
ControlledLive mode) and produced a pass/reject verdict. The safety
checks were:

- No color change is allowed in the candidate.
- No terminal effect is allowed in the candidate.
- Density does not collapse (remains >= 0.98).
- Brightness does not spike (remains <= 1.015).

Each smoke sample captured:

- `baseline_whisper: AtmosphereVisualWhisper`
- `candidate_whisper: AtmosphereVisualWhisper`
- `baseline_shadow: AtmosphereShadowRisk`
- `candidate_shadow: AtmosphereShadowRisk`
- `risk_label: &'static str`
- `passed: bool`
- `reason: String`

The A/B smoke was never wired into a production comparison loop. If a
future owner wants to revive it, the safety checks above are the right
invariants, and the `AtmosphereVisualWhisper` and `AtmosphereShadowRisk`
types (still live in `atmosphere_visual.rs` and `atmosphere_shadow.rs`)
are the right comparison subjects.

---

## 6. Why Option C (Archive + Delete) Was Chosen

The owner considered three options in Dragon Hunt v2 Phase 6 Tier E:

- **(a) Graduate** — wire `verify_application` result into the render
  path. This would make the verifier's verdict (not just its side
  effects) influence rendering. Rejected: the verifier's side effects
  (clamping, validation) are already applied in place; the verdict is
  diagnostic only and has no natural consumer in the render path.

- **(b) Slim** — delete the unused half. This is effectively what
  Option C with deletion does: the unused half (probe, A/B smoke) is
  deleted, and the used half (regime, verifier, controlled-live,
  adaptive, custom, presets, runtime, shadow, visual) is kept.

- **(c) Archive** — document as "intentional reservation" and stop
  maintaining. The owner chose this **with the deletion twist**: instead
  of leaving the dead code in place with a "do not maintain" sign, the
  dead code is removed entirely and its design knowledge is preserved
  here.

The rationale for the deletion twist:

1. **Dead code rots.** Leaving 1,071 LOC of `#![allow(dead_code)]` in the
   tree invites future drift — a refactor might accidentally wire it in,
   or a contributor might "fix" a warning by deleting a test that
   actually guards an invariant.
2. **The design knowledge is already captured.** `docs/ATMOSPHERE_ENGINE.md`
   (465 lines) is the canonical design spec. This archive doc captures
   the additional knowledge from the deleted modules (probe fields, A/B
   safety checks). Nothing is lost.
3. **The build gets cleaner.** Removing the probe and A/B smoke modules
   removes two `#![cfg_attr(not(test), allow(dead_code))]` attributes
   and their associated test files, reducing the test surface by ~1,071
   LOC and slightly improving compile time.
4. **Revival is still possible.** If a future owner wants to revive the
   probe or A/B smoke, the design knowledge is here. The revival would
   be a net-new feature, not a "re-wiring" of frozen code.

---

## 7. Impact on the Live Codebase

- **No behavioral change.** The deleted modules had zero production
  callers. The wired-in atmosphere features (regime enum, controlled-live
  modulation, adaptive hour-driven path, custom time map, shadow metrics,
  verifier in-place clamping) continue to work exactly as before.
- **No test count change.** The deleted test file (`atmosphere_tests/atmosphere_ab.rs`)
  contained 25 tests that exercised only the deleted `atmosphere_ab`
  module. The remaining atmosphere tests (in `atmosphere_tests/mod.rs`,
  `atmosphere_apply.rs`, `atmosphere_apply_cl.rs`, `atmosphere_expansion.rs`)
  continue to run.
- **No documentation links broken.** The deleted modules were not
  referenced by any live doc except `docs/ATMOSPHERE_ENGINE.md`, which
  still accurately describes the wired-in features.
- **Compile time slightly improved.** Two fewer modules to compile, two
  fewer `allow(dead_code)` attributes to enforce.

---

## 8. Cross-References

- `docs/ATMOSPHERE_ENGINE.md` — canonical v20 design spec (still live)
- `docs/research/DRAGON_HUNT_V2_AUDIT.md` — Dragon Hunt v2 audit, Phase 6
  Tier E item 31 (this archival closes it)
- `docs/research/MATRIX_BOLD_AUDIT.md` — item 32 (Bold default, closed
  with Option B Random)
- Git history: the deleted files remain accessible via `git log` and
  `git show <hash>:src/atmosphere_ab.rs` for any future reference.

---

## 9. Full Elimination (2026-08-05 — Dragon Hunt v2 Phase 6 Tier E item 31 final)

Following the partial archival in §4 (which deleted only the truly dead
probe + A/B smoke modules), the owner decided the atmosphere engine is
**not used in the future** and directed a complete elimination of all
remaining atmosphere subsystem code.

### What was eliminated

**Source files deleted (6,520 LOC across 14 files):**
- `src/atmosphere.rs` (411 LOC) — regime enum, controller, state
- `src/atmosphere_adaptive.rs` (474 LOC) — hour-driven modulation
- `src/atmosphere_apply.rs` (249 LOC) — application mode + modulation
- `src/atmosphere_controlled_live.rs` (117 LOC) — controlled-live adapter
- `src/atmosphere_custom.rs` (976 LOC) — CustomTimeMap (adaptive-custom.*)
- `src/atmosphere_presets.rs` (278 LOC) — preset list for --list-profiles
- `src/atmosphere_runtime.rs` (107 LOC) — AtmosphereEffectiveRuntime
- `src/atmosphere_shadow.rs` (519 LOC) — shadow risk metrics
- `src/atmosphere_verifier.rs` (543 LOC) — verify_application bounds
- `src/atmosphere_visual.rs` (608 LOC) — visual whisper adapter
- `src/atmosphere_tests/atmosphere_apply.rs` (643 LOC)
- `src/atmosphere_tests/atmosphere_apply_cl.rs` (236 LOC)
- `src/atmosphere_tests/atmosphere_expansion.rs` (812 LOC)
- `src/atmosphere_tests/mod.rs` (352 LOC)

**Plus surgical removals in 30+ non-atmosphere files:**
- CLI flags `--atmosphere-mode`, `--atmosphere-regime` removed from `config.rs`
- Config keys `atmosphere-mode`, `atmosphere-regime` removed from `configfile.rs` USER_CONFIG_KEYS
- Profile fields `atmosphere_mode`, `atmosphere_regime` removed from `profile.rs::UserProfile`
- Scene-custom fields `atmosphere_mode`, `atmosphere_regime` removed from `scene_custom.rs`
- `adaptive-custom.*` config key support removed (was feeding `atmosphere_custom::CustomTimeMap`)
- `is_adaptive_custom_key()` function deleted from `configfile.rs`
- Atmosphere diagnostic section removed from `bench_report.rs::build_premium_report`
- `BenchReportData.atmosphere_mode` + `atmosphere_regime` fields removed
- `CloudConfig.atmosphere_modulation/mode/regime` fields removed from `app.rs`
- Atmosphere status banner block removed from `verbose.rs::print_verbose`
- Atmosphere resolution + modulation block removed from `main.rs` (~40 LOC)
- Atmosphere block (~150 LOC) removed from `interactive/event_loop.rs`
  (custom_time_map scheduled scene change + adaptive color target)
- `parse_atmosphere_mode_config/regime_config` + `resolve_atmosphere_mode/regime`
  functions deleted from `config_apply.rs`
- `parse_atmosphere_mode_profile/regime_profile` functions deleted from `profile.rs`
- `atmosphere_presets_section()` + `list_profiles_text()` deleted from `profile.rs`
- Atmosphere help section removed from `help_detail.rs`
- Atmosphere engine info section removed from `info.rs`
- ~447 lines of atmosphere tests removed from `config_apply_tests/mod.rs`
- ~279 lines of atmosphere tests removed from `config_apply_tests/profiles.rs`
- ~115 lines of atmosphere/adaptive-custom tests removed from `testconf.rs`
- `atmosphere_adaptive::current_hour()` inlined as `system_feeling::current_local_hour()`
  (kept the chrono::Local::now() utility for system_feeling + doctor diagnostics)

**Net change**: 8,058 LOC deleted, 183 LOC inserted → 7,875 LOC net removal.

### What was KEPT (separate subsystems, NOT atmosphere engine)

- `src/chroma/post/atmosphere.rs` (547 LOC) — **Chroma Dragon post-FX**
  (AtmosphericCtx, apply_atmospheric — luminance/saturation/instability shader).
  Used by `chroma::shaders::base::resolve_cell_color` for every cell render.
  This is a SEPARATE visual post-FX subsystem that happens to share the name.
- `AtmosphericEvolution` struct in `src/cloud/ecosystem.rs` — cloud drift/gust
  events (entropy_phase, density_offset, luminance_offset, anomaly_offset,
  cycle_speed). Separate simulation subsystem, NOT atmosphere engine.

### Backward compatibility

Users with `atmosphere-mode`, `atmosphere-regime`, or `adaptive-custom.*`
keys in their config.toml will get clear rejection messages from
`--testconf`:

- `atmosphere-mode` / `atmosphere-regime`: "config keys have been removed
  — the atmosphere engine subsystem was eliminated"
- `adaptive-custom.*`: flagged as unknown key (likely typo)
- `adaptive-custom` (bare): "keys have been removed — the atmosphere
  engine subsystem was eliminated"

The default behavior (atmosphere-mode = disabled = identity modulation)
was already the production default, so existing user runs are visually
unchanged — only the diagnostic fields in `--benchmark` output are gone.

### Verification

- `cargo check` (no tests): ✓
- `cargo check --tests`: ✓
- `cargo test`: 1211 passed, 0 failed
- `cargo clippy --all-targets`: ✓ (0 warnings)
- `cargo fmt --check`: ✓
- `codespell src/ scripts/`: ✓
- `shellcheck scripts/build.sh`: ✓

### Design knowledge preserved

The original `docs/ATMOSPHERE_ENGINE.md` (475 lines) is the canonical
design spec. This archive doc captures the additional knowledge from
the deleted modules (probe fields, A/B safety checks, regime state
machine, controlled-live bounds, visual whisper adapter). If a future
owner wants to revive any part of the atmosphere engine, the design
knowledge is here + in git history (`git show <hash>:src/atmosphere_*.rs`).
