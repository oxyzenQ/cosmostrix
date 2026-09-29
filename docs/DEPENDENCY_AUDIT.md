<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Dependency Update Audit

Owner confusion (2026-09-02): "if update latest version can break, but if
not update outdate/deprecated." This audit resolves that confusion with a
clear framework: which deps are safe to update now, which need migration
work, and which to hold.

## The framework (masterclass)

### Semver crash course for Rust crates

Rust crates follow semver with a 0.x caveat:

| Version pattern | cargo meaning | Breaking? |
|-----------------|---------------|-----------|
| `"1"` | >=1.0.0, <2.0.0 | 1.x → 2.x is breaking; 1.5 → 1.6 is safe |
| `"1.5"` | >=1.5.0, <2.0.0 | same as above (5 is just a floor) |
| `"0.10"` | >=0.10.0, <0.11.0 | 0.10 → 0.11 IS breaking (0.x minor = major) |
| `"0.3"` | >=0.3.0, <0.4.0 | 0.3 → 0.4 IS breaking |
| `">=4.5, <4.6"` | >=4.5.0, <4.6.0 | explicit pin to 4.5.x only |

**Key insight**: `cargo update` (without `-p`) only applies semver-compatible
updates (within the same allowed range). It will NEVER cross a major
boundary (1.x → 2.x, 0.10 → 0.11) or an explicit upper bound (`<4.6`).
Those require a `Cargo.toml` change.

### The three buckets

1. **UPDATE NOW** — semver-compatible (patch/minor within the same allowed
   range). Guaranteed safe by semver contract. Apply via `cargo update -p
   <crate>`. Zero migration work.

2. **AUDIT THEN UPDATE** — major version bump (crosses a semver boundary).
   Requires `Cargo.toml` constraint change + code audit + migration work.
   Each dep needs its own PR with testing.

3. **HOLD** — major version bump where the migration cost outweighs the
   benefit (no CVEs, current version well-maintained, breaking API is
   high-impact). Revisit quarterly or when a CVE appears.

### The "deprecated" fear is overblown

Rust crates do NOT "deprecate" in the traditional sense. A crate at
v0.10.x is still fully functional even if v0.11 exists. The only real
deprecation is when a crate is **yanked** (removed from crates.io) —
check with `cargo yank --list` or the crates.io page. None of cosmostrix's
deps are yanked.

The real signals to act on:
- **CVE in current version** → update immediately (monitored by
  `gitbot-audit.yml` daily `cargo audit`)
- **Current version unmaintained** (no commits for 2+ years AND a newer
  major exists) → plan a migration
- **New version has a feature you need** → update for that feature

None of cosmostrix's deps are unmaintained or have open CVEs as of
2026-09-02. The available updates are "nice to have", not "must have".

## RUSTSEC-2024-0384: `instant` 0.1.13 unmaintained (transitive, accepted risk)

Owner question (2026-09-12): "is this cargo audit warning dangerous?"

Verdict: **informational, not a vulnerability — accepted and tracked.**

- Advisory class is `unmaintained`, not a CVE: no exploit path, no
  input parsing, no unsafe surface exposed through the chain. The
  crate is a wasm-era wrapper around `std::time::Instant`; on every
  native target cosmostrix builds (gnu/musl/darwin/msvc) it is a
  zero-cost passthrough to the standard library.
- Dependency chain: cosmostrix → notify v7.0.0 (file watcher for
  config live-reload) → notify-types v1.0.1 → instant 0.1.13. No
  direct usage anywhere in `src/` (the `instant` module under
  `src/msg_fill_style/` is an unrelated internal text-reveal style
  that shares the name).
- Mitigations already in place: `deny.toml` suppresses the advisory
  with the full rationale (since v50.0.0-beta.7), and the watcher
  does not rely on `instant` for correctness — its change detection
  is mtime/size/SHA-512 snapshots plus native inotify events.
- Fix path: upstream only. notify-types must migrate to `web-time`
  (the advisory's recommended replacement). notify v8 was evaluated
  in the table below and remains an owner decision per project rules
  (version bumps are never agent-initiated).
- Re-check at each LTS release: if notify-types publishes the
  migration, drop the `deny.toml` ignore entry and the warning
  disappears with a routine `cargo update`.

## Current state (cargo update --verbose, 2026-09-29)

```
 Locking 0 packages to latest compatible versions
 Unchanged clap v4.5.61 (available: v4.6.7)
 Unchanged clap_builder v4.5.61 (available: v4.6.7)
 Unchanged clap_derive v4.5.61 (available: v4.6.7)
 Unchanged generic-array v0.14.7 (available: v0.14.9)
 Unchanged notify v7.0.0 (available: v8.2.0)
 Unchanged rand v0.9.5 (available: v0.10.3)
 Unchanged sha2 v0.10.9 (available: v0.11.0)
 Unchanged signal-hook v0.3.18 (available: v0.4.4)
```

NIGHT-dinner-5 (2026-09-29) decoded this list into three classes and
acted on it: the clap pin was the only self-imposed over-strictness
and is now relaxed to `>=4.5, <4.7` (lockfile at 4.6.7, verified:
cargo check + clippy -D warnings clean, gate-keepers 16/16, full suite
delegated to CI); generic-array is upstream-pinned, not ours (see its
section below); the four major bumps stay boundary-pinned on purpose.
Full policy: [NIGHT_DINNER_5_DEPENDENCY_STRICTNESS.md](research/NIGHT_DINNER_5_DEPENDENCY_STRICTNESS.md).

## Per-dependency analysis

### UPDATE NOW (semver-compatible, blocked by constraint pin)

#### clap 4.5.61 → 4.6.7 — DONE in NIGHT-dinner-5 (2026-09-29)

| Field | Value |
|-------|-------|
| Cargo.toml constraint | `>=4.5, <4.7` (relaxed from `<4.6` in NIGHT-dinner-5) |
| Locked version | 4.6.7 (was 4.5.61) |
| Type | Minor (same major 4.x) |
| Usage depth | Deep — CLI argument parsing via derive macros (`src/cli/`) |
| Breaking changes | None observed (derive surface compiles + clippy clean under -D warnings) |
| Migration | Zero code changes. Lockfile delta: clap family 3 bumps + syn 3.0.6 added (clap_derive 4.6 uses syn 3) |
| Risk | Minimal — verified locally with cargo check --locked, clippy --all-targets --all-features -D warnings, gate-keepers 16/16; full test suite delegated to CI |
| Status | **APPLIED** — Cargo.toml relaxed, `cargo update -p clap` run, lockfile committed |

#### generic-array 0.14.7 → 0.14.9 — NOT OURS TO MOVE (upstream exact-pin)

| Field | Value |
|-------|-------|
| Cargo.toml constraint | Transitive (via sha2 0.10 → digest 0.10.7 → crypto-common 0.1.7) |
| Available | 0.14.9 (not yanked — verified via crates.io API, 2026-09-29) |
| Root cause | crypto-common 0.1.7 declares `generic-array = "=0.14.7"` — an exact upstream pin (RustCrypto damage control around the 0.14.8/0.14.9 releases) |
| Migration | Impossible from this repo: `cargo update -p generic-array` refuses, `--precise 0.14.9` refuses (resolver names the crypto-common requirement). The only path is the sha2 0.11 migration (digest 0.11 → hybrid-array), classified HOLD below |
| Risk | None in staying — 0.14.7 is the version RustCrypto itself pins |
| Status | **UPSTREAM-PINNED** — corrects this audit's 2026-09-02 recommendation ("UPDATE NOW via cargo update -p generic-array"): that command cannot succeed under the sha2 0.10 line |

### AUDIT THEN UPDATE (major version, needs migration)

#### notify 7.0.0 → 8.2.0

| Field | Value |
|-------|-------|
| Cargo.toml constraint | `>=7, <8` (explicit pin to 7.x) |
| Available | 8.2.0 |
| Type | Major (7.x → 8.x) |
| Usage depth | Medium — `src/config/live_config/watcher.rs` (1 file, ~10 call sites) |
| Breaking changes | notify 8.0 reworked the `Event` API: `EventKind` variants changed, `ModifyKind::Data` renamed, `Config` struct restructured. The `Watcher::new` trait method signature changed (takes `Config` by value instead of separate args). |
| Migration | Medium — rewrite `watcher.rs` to use the new `Config` API + verify `EventKind` matching in `handle_notify_event`. ~2-4 hours work + testing. |
| Risk | Medium — live-reload is a core feature; a regression here breaks config hot-reload. Need PTY live-reload test proof. |
| Recommendation | **AUDIT THEN UPDATE** — do this in a dedicated PR. Test with the live PTY reload script (`scripts/harness/cli_config_stresstest.sh`). |

#### rand 0.9.5 → 0.10.2

| Field | Value |
|-------|-------|
| Cargo.toml constraint | `0.9` (= >=0.9.0, <0.10.0) |
| Available | 0.10.2 |
| Type | Major (0.9 → 0.10, 0.x minor = major) |
| Usage depth | Deep — rain droplet RNG across `src/engine/`, `src/msg_fill_style/`, tests (~15 call sites) |
| Breaking changes | rand 0.10 reworked the `Rng` trait, `distr` module (renamed from `distributions`), `SeedableRng` API. `rand::rngs::StdRng` API changed. |
| Migration | Medium-high — update all `use rand::distr::Distribution` → `use rand::distr::Distribution` (may be same), `rand::rngs::StdRng::seed_from_u64` may change signature. Need to audit each call site. ~4-6 hours work + testing. |
| Risk | Medium — RNG is used in visual rendering; a subtle change could alter rain patterns without breaking tests. Need visual A/B comparison. |
| Recommendation | **AUDIT THEN UPDATE** — do this AFTER notify. Run the visual A/B benchmark to verify rain patterns are unchanged. |

#### signal-hook 0.3.18 → 0.4.4

| Field | Value |
|-------|-------|
| Cargo.toml constraint | `0.3` (= >=0.3.0, <0.4.0) |
| Available | 0.4.4 |
| Type | Major (0.3 → 0.4, 0.x minor = major) |
| Usage depth | Low-medium — `src/interactive/signal_handlers.rs` + `src/bench/bench_progress.rs` (~5 call sites) |
| Breaking changes | signal-hook 0.4 changed the `iterator::Signals` API and `low_level` module. The `flag::register` signature may have changed. |
| Migration | Low-medium — signal handling is isolated to 2 files. ~1-2 hours work + testing (need to test SIGINT/SIGTERM/SIGHUP/SIGQUIT handling manually). |
| Risk | Medium — signal handling is critical for clean terminal cleanup. A regression here could leave the terminal in a broken state on Ctrl-C. |
| Recommendation | **AUDIT THEN UPDATE** — do this in a dedicated PR. Test signal handling manually (Ctrl-C, kill -TERM, kill -HUP). |

### HOLD (breaking + low ROI)

#### sha2 0.10.9 → 0.11.0

| Field | Value |
|-------|-------|
| Cargo.toml constraint | `0.10` (= >=0.10.0, <0.11.0) |
| Available | 0.11.0 |
| Type | Major (0.10 → 0.11, 0.x minor = major) |
| Usage depth | Low — `src/config/configfile/configfile_dump.rs` + tests (~4 call sites) |
| Breaking changes | sha2 0.11 is a major API rework: `Digest` trait restructured, `Sha512::new()` → `Sha512::new_with_prefix()`, output API changed. |
| Migration | Medium — update 4 call sites. BUT sha2 is used in security-critical paths (config.toml content hashing for live-reload change detection). |
| Risk | High — a subtle hashing change could cause live-reload to miss config changes (false negative) or fire spuriously (false positive). Hard to test exhaustively. |
| Recommendation | **HOLD** — sha2 0.10.9 is well-maintained (no CVEs, regular patch releases). The 0.11 migration cost (security-critical path, API rework) outweighs the benefit (no new features needed). Revisit if a CVE is reported in 0.10.x or if 0.11 stabilizes for 2+ years. |

## Action plan

### Step 1: Apply safe updates now — DONE (NIGHT-dinner-5, 2026-09-29)

Applied exactly as planned: the clap pin relaxed to `>=4.5, <4.7`, the
lockfile moved to 4.6.7 (clap + clap_builder + clap_derive + syn 3.0.6),
verified with cargo check --locked + clippy -D warnings + gate-keepers
16/16, and pushed with the NIGHT-dinner-5 commit. The generic-array
half of the old step 1 is impossible from this repo — upstream
exact-pin, see its table above. Next in-range updates land
automatically via the weekly maintenance.yml cron.

### Step 2: Plan major version updates (one PR per dep)

| Priority | Dep | Est. effort | Dependency |
|----------|-----|-------------|------------|
| 1 | notify 8 | 2-4 hours | None (independent; note: notify 9.0.0-rc.5 exists as a pre-release — release candidates never enter the lockfile) |
| 2 | signal-hook 0.4 | 1-2 hours | None (independent) |
| 3 | rand 0.10 | 4-6 hours | After notify (to isolate visual regressions) |
| — | sha2 0.11 | HOLD | Revisit quarterly |

Each major update PR MUST:
1. Change the `Cargo.toml` constraint.
2. Run `cargo update -p <crate>`.
3. Fix all compilation errors (migration work).
4. Run `cargo test --all --locked` (all tests pass).
5. Run `cargo clippy -- -D warnings` (no new lints).
6. For notify: run `scripts/harness/cli_config_stresstest.sh` (live-reload PTY proof).
7. For rand: run visual A/B benchmark (rain patterns unchanged).
8. For signal-hook: manually test Ctrl-C / kill -TERM / kill -HUP.

### Step 3: Ongoing maintenance

The existing `maintenance.yml` weekly cron already runs
`cargo update` + `cargo audit` + `cargo deny check all`. This handles:
- Semver-compatible updates (applied automatically, CI verifies).
- CVE monitoring (cargo audit, daily via `gitbot-audit.yml`).
- License compliance (cargo deny, daily).

No changes needed to the maintenance workflow — it's already correct.

## Why not just update everything to latest?

The "latest = best" assumption is wrong for LTS/dormant-mode projects:

1. **Stability over freshness**: cosmostrix targets 5-10 year maintenance
   cycles (see `docs/MAINTENANCE.md`). A stable, well-tested dep at v0.10
   is better than a fresh v0.11 that might have regressions.

2. **Semver is a contract, not a guarantee**: a "semver-compatible" minor
   bump CAN introduce subtle behavior changes (e.g. a hash function
   changing its internal buffer size). The risk is low but non-zero.

3. **Migration cost is real**: every major bump requires code changes,
   testing, and review. For a solo-maintained project, this time is
   scarce. Spend it on deps that have CVEs or blocking issues, not on
   "nice to have" updates.

4. **The lockfile is the truth**: `Cargo.lock` pins exact versions. Even
   if a new version exists, the lockfile guarantees reproducible builds.
   "Outdated" lockfile ≠ "broken" build.

## Monitoring

| Signal | Tool | Frequency | Action |
|--------|------|-----------|--------|
| CVE in a dep | `cargo audit` | Daily (`gitbot-audit.yml`) | Update the affected dep immediately |
| License violation | `cargo deny check` | Daily (`gitbot-audit.yml`) | Replace or obtain license |
| New major version available | `cargo update --verbose` | Quarterly manual check | Audit per this doc's framework |
| Dep unmaintained (2+ years no commits) | crates.io page | Annual check | Plan migration |

## Cross-references

- `docs/SUPPLY_CHAIN.md` — dependency policy, license allow-list, lockfile discipline
- `docs/MAINTENANCE.md` — dormant-mode maintenance guide (Section 4: dependency updates)
- `.github/workflows/maintenance.yml` — weekly `cargo update` cron
- `.github/workflows/gitbot-audit.yml` — daily `cargo audit` + `cargo deny`
- `deny.toml` — license/source/duplicate policy
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
