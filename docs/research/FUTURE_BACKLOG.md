<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmostrix Future Backlog

**Purpose**: Parking lot for design decisions, new flags, new parameters, and
behavior changes that were intentionally **NOT** implemented in the v30
stabilization audit (Phases 1-5). This is a long-term-stability release — the
config surface is frozen. Items here are saved for a future session when the
owner returns and wants to evolve the surface again.

**Owner**: oxyzenQ
**Last updated**: 2026-08-04 (after Phase 5 close, commit `bd6bb3e`)
**Source**: Distilled from `CONFIG_SYNC_AUDIT_PHASE5.md` §4.1 (15 deferred
items) + Phase 6 recommendation (dead code sweep).

---

## 1. Why these are parked (not done in v30)

The v30 release is a **stabilization release**. The 5-phase config-sync audit
closed 13 of 39 findings and produced 11 verified fixes (1511 tests PASS,
clippy clean, 0 regressions). The 24 remaining items fall into 3 buckets:

1. **New CLI surface** (`--strict-config`, `--strict-profiles`, etc.) — adds
   flags, which would expand the surface that v30 is trying to freeze.
2. **Behavior changes** (warn-once for deprecated flags, case-insensitivity
   unification) — breaking changes that need migration notes.
3. **Refactors with non-trivial blast radius** (`load_config_file` signature
   change touches ~10 call sites) — better batched with future work.

All 3 buckets are parked here so v30 ships clean. When the owner returns, they
can pick items by ID and re-open them.

---

## 2. New CLI Flags / Parameters (proposed, not built)

> **Rule for v30**: do NOT implement any of these. They are listed here so the
> design rationale isn't lost. Each entry has the original finding ID, the
> problem it solves, and the proposed surface.

### 2.1 `--strict-config` (from P3-5)

**Problem**: Soft warnings (`[config] warning: ...`) are easy to miss in noisy
startup output. Users can run with a typo'd key for weeks without noticing.

**Proposed surface**: `--strict-config` flag. When set, any soft warning
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
`[adaptive-custom.*]` blocks. The comma-separated `HH-MM = color, scene,
key=value, ...` format is never validated by `--testconf`, only by the runtime
parser at `event_loop.rs:251`.

**Proposed surface**: Extend `testconf.rs::validate_field_value` to handle
adaptive-custom lines, OR add a new `validate_adaptive_custom_line` function
called from the `--testconf` flow.

**Why deferred**: Requires writing a comma-separated parser that mirrors
`atmosphere_custom.rs::parse_custom_time_map` exactly. Risk of divergence
between the two parsers is high. Better to refactor
`parse_custom_time_map` into a shared validator + parser pair first.

**Reopen cost**: ~4h. Touches `testconf.rs` + `atmosphere_custom.rs` (extract
shared validator) + tests.

---

### 2.5 Case-insensitive enum unification (from P2-6, P1-#4)

**Problem**: Three enums have inconsistent case handling:
- CLI `ValueEnum` — case-insensitive (clap default).
- `testconf.rs` — strict lowercase (canonical form).
- Runtime `from_str(&v, true)` — case-insensitive.

Result: `--intro Logo` works, `intro = "Logo"` is rejected by `--testconf`,
runtime would accept it.

**Proposed surface**: Unify on case-insensitive everywhere. Either:
- Make `testconf.rs` case-insensitive (relax canonical form), OR
- Make CLI case-sensitive (force canonical form).

**Why deferred**: Owner needs to decide which direction. Relaxing testconf
loses the canonical-form verification property. Forcing CLI canonical breaks
user muscle memory (`--intro Logo` would stop working).

**Reopen cost**: ~2h whichever direction. Touches `testconf.rs` (3 enums) +
tests.

---

## 3. Behavior Changes (breaking, need migration)

### 3.1 Warn-once for deprecated glitch flags (from P2-3)

**Problem**: `--glitch-pct`, `--shortpct`, `--rippct` are silently overridden
by `--glitch-level`. Users setting these flags get the glitch-level value
instead of their flag value, with no warning.

**Proposed behavior**: Emit a warn-once stderr line when both a deprecated
flag AND `--glitch-level` are set.

**Why deferred**: The flags are deprecated — warning would be noisy for users
who haven't migrated. Owner needs to decide: warn-once, warn-always, or
remove the flags entirely (true breaking change).

**Reopen cost**: ~1h. Touches `config_apply.rs` glitch-level merge site + a
static `AtomicBool` for warn-once.

---

### 3.2 Name resolution in profile/scene-custom (from P1-#5)

**Problem**: `[profile.foo]` and `[scene-custom.foo]` can specify
`charset = "my_custom_charset"` but the name is never resolved against
`[charset-custom.my_custom_charset]`. The user gets a silent fallback to the
default charset.

**Proposed behavior**: Resolve custom charset/color/glitch names at apply
time, with a stderr error if the name doesn't exist.

**Why deferred**: Requires name-resolution logic in `apply_profile_layer` and
`apply_scene_custom_layer`. Needs a clear error message + tests for each
combo (charset/color/glitch × profile/scene-custom).

**Reopen cost**: ~4h. Touches `profile.rs` + `scene_custom.rs` +
`charset_custom.rs` (lookup) + `colors_custom.rs` (lookup) + tests.

---

## 4. Refactors (low priority, batch with future work)

### 4.1 `load_config_file` return full parse result (from P4-8)

**Problem**: `config_apply.rs:126, 184` reads the config file from disk 2
times. `load_config_file` returns only the parsed `HashMap`, discarding
`malformed_lines` and `unknown_keys` vectors. The second read at line 184
re-parses to recover those vectors.

**Proposed refactor**: Change `load_config_file` signature to return the full
`parse_config_text` result. Update ~10 call sites.

**Why deferred**: 200μs saving is invisible. Refactor touches ~10 call sites,
including test helpers. Better batched with future `configfile.rs` work.

**Reopen cost**: ~2h. Touches `configfile.rs::load_config_file` + ~10 callers
+ test helpers.

---

### 4.2 Profile/scene-custom `collect_profiles` O(n) iteration (from P4-6)

**Problem**: `profile.rs::collect_profiles` iterates ALL config keys to find
profile keys. O(n) where n = total keys. For 50-key config, ~5μs.

**Proposed refactor**: Maintain a `profile_keys` subset during config load.
Iterate only that subset in `collect_profiles`.

**Why deferred**: 5μs saving is invisible. Refactor requires touching the
config-load path to maintain the subset, which adds complexity for no
user-visible benefit.

**Reopen cost**: ~1.5h. Touches `configfile.rs` (maintain subset) +
`profile.rs::collect_profiles`.

---

### 4.3 `config_apply` 17-sequential-lookup pattern (from P4-4)

**Problem**: `apply_config_values` calls `config_value(...)` 17 times, once
per supported key. Each call does 2 HashMap lookups + may allocate.

**Proposed refactor**: Single iteration over `cfg` with a match arm per key.

**Why deferred**: Code-smell, not perf-critical. The current design is more
readable (each key's handling is co-located with its lookup). Reclassified as
a positive finding in Phase 5.

**Reopen cost**: ~3h. Touches `config_apply.rs::apply_config_values` (large
function, needs careful refactor to preserve all 17 behaviors).

---

## 5. Phase 6 Recommendation — Dead Code & Legacy Parameter Sweep

**NOT done in Phases 1-5.** This is the next audit dimension the owner asked
about (2026-08-04 chat): "gue ngoding sama ai bikin fitur banyak sampai
pokoknya jadi dead/legacy code baik code fungsi yang mati ataupun parameters
ataupun fungsi lain jadinya gue harus bersih bersih sampai dalam bro."

### 5.1 Quick scan results (2026-08-04)

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

### 5.2 Proposed Phase 6 scope

1. **Triage the 6 `#[allow(dead_code)]` sites** — decide for each: remove,
   document why it's kept, or wire it up.
2. **Triage 187 TODO/FIXME/legacy mentions** — most are in tests, but the
   non-test ones (especially in `scene.rs` 9, `testconf.rs` 7,
   `atmosphere_apply.rs` 6, `configfile.rs` 17, `config_hints.rs` 17,
   `scene_custom.rs` 14) need review.
3. **CLI flag inventory** — Phase 1 inventoried 60+ CLI flags. Phase 6 should
   cross-reference each flag against: (a) is it wired through
   `config_apply`? (b) is it tested? (c) is it documented in `--help`? Any
   flag failing all 3 is a dead flag candidate.
4. **Config key inventory** — Phase 1 listed `USER_CONFIG_KEYS` (the
   allow-list). Phase 6 should cross-reference each key against: (a) is it
   read by `config_apply`? (b) is it tested? Any key failing both is a dead
   key candidate.
5. **`pub fn` inventory** — run `cargo +nightly udeps` (or equivalent) to
   find unused public functions. Note: this requires installing `cargo-udeps`
   which the owner previously said is OK (the "don't install" rule was for
   `cargo-deny` / `cargo-audit` only).
6. **Dead module detection** — are there entire modules that are no longer
   reached from `main.rs`? Phase 6 should build a call graph from `main` and
   flag unreachable modules.

### 5.3 Why Phase 6 wasn't merged into Phase 5

Phase 5 was scoped to the 36 open config-sync items. Dead code is a different
dimension (code-reachability, not config-sync). Merging would have made the
audit unbounded. Cleaner to ship v30 with the config layer stabilized, then
run a separate Phase 6 for dead code.

### 5.4 Reopen cost estimate

~6-8h for a thorough Phase 6 sweep. Produces
`docs/research/CONFIG_SYNC_AUDIT_PHASE6.md` (or rename to
`DEAD_CODE_SWEEP.md`) + commits removing dead code.

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
