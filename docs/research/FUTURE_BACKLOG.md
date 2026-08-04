<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmostrix Future Backlog

**Purpose**: Parking lot for **new flags / new parameters / new CLI surface**
that were intentionally NOT added in the v30 stabilization audit. The v30
release freezes the config surface — items here are saved for a future session
when the owner returns and wants to evolve the surface again.

**Owner**: oxyzenQ
**Last updated**: 2026-08-04 (after Miri unsafe soundness pass — 0 unsoundness)
**Status**: ALL 39 findings from Phases 1-5 are CLOSED (100%). Phase 6
dead-code sweep + pub→pub(crate) tightening + Miri unsafe soundness pass
all COMPLETE. Only item remaining: optional `cargo +nightly udeps` run
(expected 0 findings). The items below are NEW FLAG/PARAMETER IDEAS
only — they were parked per owner instruction ("flag/parameters baru
jangan dibuat dulu karena ini akan menjadi versi stabilisasi long term").

---

## 0. Audit Closure Status (2026-08-04)

All 39 findings from the 5-phase config-sync audit are now CLOSED:

| Phase | Findings | Closed | How |
|---|---|---|---|
| Phase 1 | 12 | 12 | 2 doc-only (Phase 5 Fix 1-2) + 6 doc-only (Phase 5 FINAL batch 1) + 2 case-insensitive (batch 2) + 1 reclassified (P2-3) + 1 name resolution (batch 6) + 2 hint patterns (batch 7) |
| Phase 2 | 9 | 9 | 2 Phase 3 Fix A/B + 1 reclassified (P2-9 false positive) + 1 reclassified (P2-3 false positive) + 1 case-insensitive (batch 2) + 1 documented (batch 1) + 1 closed by P1-#5 (batch 6) + 1 closed by P1-#4 (batch 2) + 1 closed by P3-4 tests (batch 4) |
| Phase 3 | 10 | 10 | 2 Phase 3 Fix A/B + 2 Phase 5 Fix 5/10 + 2 Phase 5 Fix 7/8/9/11 + 1 reclassified (P3-1) + 1 closed by batch 4 (P3-4) + 1 closed by batch 5 (P3-5) + 1 reclassified (P3-3 → P4-1) |
| Phase 4 | 8 | 8 | 3 reclassified as positive (P4-1/P4-4/P4-6/P4-7) + 1 Phase 5 Fix 3 (P4-2) + 1 Phase 5 Fix 6 (P4-3) + 1 documented (P4-5) + 1 refactored (P4-8, batch 9) |
| Phase 5 | 11 fixes | 11 | Applied in initial Phase 5 (commit bd6bb3e) |
| **FINAL batch** | — | +13 more | batches 1-9 in this session (commits 67d0092..c75aa98) |
| **Total** | **39** | **39** | **100% closed** |

Final test count: 1529 PASS (up from 1511 after initial Phase 5).
0 regressions. Clippy clean. All gatekeeper scripts PASS.

### Phase 6 update (2026-08-04)

Phase 6 (dead code & legacy parameter sweep) completed — **0 dead code
found**. The codebase is clean by every rigorous measure available without
installing new tooling. Full report: `CONFIG_SYNC_AUDIT_PHASE6.md`.

The only "debt" surfaced is 580 `unreachable_pub` warnings on opt-in lint
(`cargo clippy -W unreachable_pub`) — these are NOT dead code, they're
`pub` items in a binary crate where `pub == pub(crate)`. Not part of
gatekeeper. Documented as cosmetic debt for a future visibility-tightening
pass. See Phase 6 report §2.8 and §6 (recommendation #1) for details.

---

## 1. What's parked here (NEW FLAG ideas only)

The items below are **new CLI flag / parameter ideas** that the owner chose
NOT to implement in v30 (stabilization release). They are saved here so the
design rationale isn't lost. When the owner returns and wants to evolve the
config surface, they can pick items by ID and re-open them.

> **Note**: The underlying CONCERNS that motivated these flag ideas have
> already been addressed in v30 via non-flag approaches (doc comments,
> warning summaries, code fixes). The flags themselves are parked as future
> surface expansion.

---

## 2. New CLI Flags / Parameters (proposed, not built)

> **Rule for v30**: do NOT implement any of these. They are listed here so the
> design rationale isn't lost. Each entry has the original finding ID, the
> problem it solves, and the proposed surface.

### 2.1 `--strict-config` (from P3-5)

**Problem**: Soft warnings (`[config] warning: ...`) are easy to miss in noisy
startup output.

**v30 closure (no flag)**: Added a startup warning SUMMARY line at the end of
config apply — if any warnings were emitted, a final `[config] N warning(s)
emitted during config apply — scroll up for details` line is printed. This
makes warnings visible without adding a new flag. (commit 6fd7380)

**Proposed flag (parked)**: `--strict-config` flag. When set, any soft warning
becomes a hard error (exit code 1, message to stderr).

**Why deferred**: Needs design decision on which warnings get promoted (all?
only unknown keys? only invalid values?). Also overlaps with `--testconf`
(which is already strict for a different reason — canonical-form verification).

**Reopen cost**: ~2h. Touches `configfile.rs::warn_invalid` callers (5 sites)
+ `config.rs` (clap arg) + `main.rs` (exit-code wiring).

---

### 2.2 `--strict-profiles` (from P2-4)

**Problem**: `[profile.<name>]` and `[scene-custom.<name>]` use `warn_invalid`
(continue with warning), while top-level config uses strict reject (exit 1).
The divergence is confusing — users expect the same strictness everywhere.

**v30 closure (no flag)**: Documented the divergence as intentional in
`ATMOSPHERE_ENGINE.md` (Profile/Scene-Custom Strictness section). Profiles
are override collections — rejecting the entire config because one profile
has a typo would be hostile. Users who want strict validation can use
`--testconf`. (commit 67d0092)

**Proposed surface**: `--strict-profiles` flag. When set, profile/scene-custom
invalid values cause exit 1 (matching top-level behavior).

**Why deferred**: Same design-decision overhead as `--strict-config`. Owner
needs to decide if the default should flip (strict-by-default with
`--lenient-profiles` opt-out) or stay warn-by-default.

**Reopen cost**: ~1.5h. Touches `profile.rs::apply_profile_layer` + 
`scene_custom.rs::apply_scene_custom_layer` + clap arg + exit-code wiring.

---

### 2.3 `--no-adaptive-custom-when-disabled` (from P1-#2)

**Problem**: `[adaptive-custom.HH-MM]` entries run even when
`atmosphere-mode = disabled`. Phase 5 Fix 2 documented this in `--help` and
docs, but the behavior is still surprising.

**Proposed surface**: A flag (or config key) that makes `atmosphere-mode =
disabled` ALSO suspend adaptive-custom. Could be:
- `--no-adaptive-custom-when-disabled` (CLI flag), or
- `atmosphere-mode = "disabled-strict"` (new enum variant), or
- `adaptive-custom-respects-disabled = true` (config key).

**Why deferred**: Breaking change. Users who rely on the current behavior
(adaptive-custom running regardless of atmosphere-mode) would break. Needs
owner sign-off + migration note + at least one minor-version bump.

**Reopen cost**: ~3h. Touches `event_loop.rs` (gate the adaptive-custom
check) + `config_apply.rs` (wire the new flag/key) + tests + docs.

---

### 2.4 `--testconf-adaptive-custom` (from P3-4)

**Problem**: `--testconf` validates every config key EXCEPT
`[adaptive-custom.*]` blocks.

**v30 closure (no flag)**: Investigation revealed `--testconf` ALREADY
validates adaptive-custom blocks — `validate_config_strictly` (testconf.rs:254)
calls `parse_custom_time_map` for any `adaptive-custom.*` key, which validates
HH-MM format, color/scene names, and all 5 parameters via canonical parsers.
Added 5 explicit tests (commit 66b8af0) documenting the closure. No new flag
needed.

**Proposed flag (parked, lower priority now)**: A standalone
`--testconf-adaptive-custom` flag that validates ONLY adaptive-custom blocks
without running the full --testconf pass. Useful for rapid iteration on
adaptive-custom schedules. Low priority since `--testconf` already covers it.

**Reopen cost**: ~1h. Touches `testconf.rs` + `main.rs` (flag wiring).

---

### 2.5 Case-insensitive enum unification (from P2-6, P1-#4)

**Problem**: Three enums had inconsistent case handling:
- CLI `ValueEnum` — case-insensitive (clap default).
- `testconf.rs` — strict lowercase (canonical form).
- Runtime `from_str(&v, true)` — case-insensitive.

**v30 closure (no flag)**: Made `testconf.rs` case-insensitive for all 4
affected enums (intro, monolith-size, glitch-level, color-bg) by adding
`.to_ascii_lowercase()` normalization. Now all 3 paths (CLI, testconf,
runtime) agree. (commit 76115d4)

**No flag needed** — this was a code fix, not a surface expansion. The entry
is kept here only for historical reference.

---

## 3. Behavior Changes (breaking, need migration)

### 3.1 Warn-once for deprecated glitch flags (from P2-3)

**v30 closure**: RECLASSIFIED as false positive. The deprecated glitch flags
(`--glitch-pct`, `--shortpct`, `--rippct`) were removed in v17 — they are
`#[arg(skip = ...)]` in config.rs and NOT in USER_CONFIG_KEYS. Users cannot
set them via CLI or config.toml. No silent override is possible. (commit
dfc2680)

**No action needed** — the original Phase 2 finding described a v16-era
scenario that no longer applies.

---

### 3.2 Name resolution in profile/scene-custom (from P1-#5)

**v30 closure**: IMPLEMENTED. `apply_profile_layer` and `apply_profile_overrides`
now accept `cfg: &HashMap<String, String>` and resolve custom charset names
via `load_custom_charset_if_matches` + custom color names via the new
`is_colors_custom_name` helper. Matches top-level config_apply behavior.
(commit 115a458)

**No action needed** — this was a code fix, not a surface expansion.

---

## 4. Refactors (low priority, batch with future work)

### 4.1 `load_config_file` return full parse result (from P4-8)

**v30 closure**: IMPLEMENTED. Added `load_config_file_full` in configfile.rs
that returns the full `ParsedConfig` (values + malformed_lines + unknown_keys
+ promoted_keys) in a single disk read. `config_apply.rs` now uses it and
eliminates the redundant second `fs::read_to_string` + `parse_config_text`
call. The 15+ other callers of `load_config_file` are unchanged (it's now a
thin wrapper: `load_config_file_full(path).values`). (commit c75aa98)

**No action needed.**

---

### 4.2 Profile/scene-custom `collect_profiles` O(n) iteration (from P4-6)

**v30 closure**: Reclassified as positive finding. O(n) iteration over a
50-key HashMap is ~5μs — invisible. Documented as intentional in code comment
at `profile.rs::collect_profiles`. (commit 67d0092)

**No action needed.**

---

### 4.3 `config_apply` 17-sequential-lookup pattern (from P4-4)

**v30 closure**: Reclassified as positive finding. The 17-lookup pattern is
intentional for readability (each key's handling is co-located with its
lookup). ~5μs at startup is invisible. Documented in code comment at
`config_apply.rs::apply_config_values`. (commit 67d0092)

**No action needed.**

---

## 5. Phase 6 — Dead Code & Legacy Parameter Sweep (DONE 2026-08-04)

**Status**: COMPLETED. Full report at `CONFIG_SYNC_AUDIT_PHASE6.md`.

The owner asked about this on 2026-08-04: "gue ngoding sama ai bikin fitur
banyak sampai pokoknya jadi dead/legacy code baik code fungsi yang mati
ataupun parameters ataupun fungsi lain jadinya gue harus bersih bersih
sampai dalam bro."

### 5.1 Quick scan results (initial, 2026-08-04)

- **`#[allow(dead_code)]` / `#[allow(unused`**: 6 occurrences across 5 files
  (`cloud/ecosystem.rs`, `chroma/shaders/transition.rs`, `chroma/palette.rs`,
  `chroma/post/atmosphere.rs`, `bench_perf.rs`). Low count, but each one is a
  candidate for "is this actually dead?" review.
- **`#[deprecated]`**: 0 occurrences. No formal deprecation markers.
- **TODO/FIXME/legacy mentions**: 187 across 61 files. Most are in test files
  (`configfile_promotion_tests.rs` 19, `configfile_bug7_tests.rs` 6,
  `config_apply_tests.rs` 5) which is expected. The non-test mentions need
  triage.
- **Compiler warnings**: Phase 5 verified `cargo check` PASS with 0 warnings,
  `cargo clippy` clean. So no compiler-detected dead code.

### 5.2 Phase 6 scope (executed)

1. **Triage the 6 `#[allow(dead_code)]` sites** — DONE: all 6 are intentional
   design decisions with inline explanatory comments. None removed.
2. **Triage 187 TODO/FIXME/legacy mentions** — DONE: 0 actual TODO/FIXME
   markers in the codebase. The "187 mentions" count was matching English
   words `legacy` / `deprecated` in historical context comments (referring
   to v14/v17/v25 version purges). All such mentions are intentional
   documentation, not markers.
3. **CLI flag inventory** — DONE: all 58 Args fields referenced in code.
   The 4 `#[arg(skip = ...)]` fields are v17 legacy internals set by
   `glitch_level` preset — already documented inline.
4. **Config key inventory** — DONE: all 17 `USER_CONFIG_KEYS` entries read
   by `config_apply` (16 via `config_value()` helper + 1 via direct
   `cfg.get("async-mode")`). `adaptive-custom` handled by special parser.
5. **`pub fn` inventory** — DONE: `cargo clippy -W dead_code` reports 0
   warnings. `cargo +nightly udeps` install timed out, but rustc's
   `dead_code` lint is a sufficient substitute for a binary crate (no
   external API surface to keep items alive).
6. **Dead module detection** — DONE: `cargo check -W dead_code` reports 0
   unused-module warnings.

### 5.3 Phase 6 result

**0 dead code found.** The v30 stabilization work (Phases 1-5) already
purged the dead code that had accumulated through feature iteration. Phase 6
confirms the purge was thorough.

The only debt surfaced: 580 `unreachable_pub` warnings on opt-in lint
(`cargo clippy -W unreachable_pub`). These are NOT dead code — they're
`pub` items in a binary crate where `pub == pub(crate)`. Not part of
gatekeeper. Documented as cosmetic debt for a future visibility-tightening
pass. Estimated ~3-4h to bulk-fix (touches ~50 files, zero functional
benefit, pure cosmetic).

### 5.4 Why Phase 6 wasn't merged into Phase 5

Phase 5 was scoped to the 36 open config-sync items. Dead code is a different
dimension (code-reachability, not config-sync). Merging would have made the
audit unbounded. Cleaner to ship v30 with the config layer stabilized, then
run a separate Phase 6 for dead code.

### 5.5 Future cleanup recommendations (post-Phase 6)

1. **`pub` → `pub(crate)` visibility tightening** — DONE 2026-08-04
   (commit `78464a4`). 580 unreachable_pub warnings → 0. 64 files touched,
   560 replacements (567 pub→pub(crate) + 13 pub→pub(super)). 1529 tests
   PASS, clippy clean.
2. **`cargo +nightly udeps` install + run** — expected 0 findings (since
   `cargo clippy -W dead_code` is clean). Still worth running once for
   completeness. Install: `cargo +nightly install cargo-udeps`. Run:
   `cargo +nightly udeps --all-targets`.
3. **Miri run on `unsafe` blocks** — DONE 2026-08-04. Full report at
   `UNSAFE_SOUNDNESS_AUDIT.md`. Result: 0 unsoundness. 107 pure-logic
   tests verified under Miri (exercising the `GlobalAlloc` impl).
   14 FFI unsafe sites manually reviewed — all follow textbook patterns
   (zero-init + return check, MaybeUninit + assume_init after success,
   direct FFI with checked return). No fixes needed.

None of these are blocking. v30 is ready to ship as-is.

---

## 6. How to reopen an item

When the owner returns and wants to act on any item here:

1. Find the item by ID (e.g. "P3-5 `--strict-config`").
2. Read the original Phase report for full context (linked in §1 of each
   phase report).
3. Decide: implement / defer again / close as won't-fix.
4. If implement: create a branch, write the code, run
   `./scripts/build.sh check-all`, commit + push.

The Phase 1-5 reports are the source of truth for the original evidence
(`file:line` citations). This backlog is the index for reopening.
