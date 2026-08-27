<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Dependency Audit — Post-Levenshtein Removal

**Audit date**: 2026-08-27
**Auditor**: cosmic-dragon session (v50.0.0-beta.7)
**Scope**: deep audit of all dependencies after the removal of the custom Levenshtein suggestion engine (`KNOWN_LONG_FLAGS` + `cli_edit_distance` + `suggest_cli_flag`). Verify zero stale deps, supply-chain hygiene, compile-time burden, and feature flag correctness.

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

## 1. Executive summary

**Levenshtein removal cleanup status: CLEAN.** The previous session's removal of `KNOWN_LONG_FLAGS` + `cli_edit_distance` + `suggest_cli_flag` left **zero stale dependencies**. The removed code was 100% pure Rust with no external crate dependency. The new `extract_clap_suggestion()` approach correctly leverages clap's built-in `suggestions` feature (which transitively pulls `strsim`) — this is intentional, documented, and actively used.

**Overall dependency health: GOOD.** 11 direct deps (12 with `proptest` dev-dep), all mainstream and actively maintained. No obscure crates. No `optional = true` deps. No `[features]` section. All enabled features map to verified usage sites. 65 unique production crates + 6 dev-only = 71 total — reasonable for a 95K-LOC terminal renderer with file watching, signal handling, SHA-512 hashing, and clap derive.

**Optimization opportunities (in priority order):**

1. Bump `notify` 6 → 7 to eliminate `mio` + `bitflags` duplication (~1s compile-time win, medium effort due to API migration)
2. Run `cargo audit` for RUSTSEC advisory check (not run in this audit — recommended for CI)
3. Relax `proptest = "1.8.0"` exact pin to `"1.8"` (consistency with other caret-style constraints)
4. Document the `notify` `<7` cap rationale in `Cargo.toml` (currently undocumented)

**No stale deps to remove. No features to drop. No urgent action required.**

---

## 2. Direct Dependencies Inventory

| # | Crate | Version Constraint | Resolved | Features | Target |
|---|---|---|---|---|---|
| 1 | `clap` | `>=4.5, <4.6` | 4.5.61 | `std, color, help, usage, error-context, derive, suggestions`, `default-features = false` | all |
| 2 | `crossterm` | `0.29` | 0.29.0 | `bracketed-paste, events, windows`, `default-features = false` | all |
| 3 | `rand` | `0.9` | 0.9.5 | (default features) | all |
| 4 | `bitvec` | `1` | 1.1.1 | (default features) | all |
| 5 | `smallvec` | `1` | 1.15.2 | (default features) | all |
| 6 | `unicode-width` | `0.2` | 0.2.2 | (default features; pulls `cjk`) | all |
| 7 | `notify` | `>=6.1, <7` | 6.1.1 | `default-features = false` (no features) | all |
| 7b | `notify` (macOS) | same | same | `macos_fsevent` | `cfg(target_os = "macos")` |
| 7c | `notify` (BSD/iOS) | same | same | `kqueue` | `cfg(any(freebsd, netbsd, openbsd, dragonfly, ios))` |
| 8 | `sha2` | `0.10` | 0.10.9 | (default features) | all |
| 9 | `signal-hook` | `0.3` | 0.3.18 | (default features) | `cfg(unix)` |
| 10 | `libc` | `0.2` | 0.2.189 | (default features) | `cfg(unix)` |
| 11 | `ctrlc` | `3.4` | (constraint) | (default features) | `cfg(windows)` |
| dev | `proptest` | `1.8.0` (exact pin) | 1.11.0 | `std`, `default-features = false` | dev only |

**No `[features]` section in Cargo.toml. No `optional = true` deps.** (Confirmed via grep — zero matches.)

---

## 3. Per-Dependency Usage Evidence (all still used)

Every direct dependency was verified via `rg` to have at least one production usage site in `src/`:

| Crate | Used? | Evidence (file:line) | # files |
|---|---|---|---|
| `clap` | YES | `src/main.rs:414` (`cmd.styles(cli::clap_styles())`), `src/cli/mod.rs:43-47`, `src/scene_custom/mod.rs:48-49` | 11 files, 53 hits |
| `crossterm` | YES | `src/interactive/input.rs`, `src/cosmic_dragon_engine/frame.rs`, `src/cosmic_dragon_engine/terminal/mod.rs` | 54 files, 114 hits |
| `rand` | YES | `src/cosmic_dragon_engine/cloud/living_rain.rs`, `src/crystal_dragon_engine/point_system/mod.rs` | 15 files, 29 hits |
| `bitvec` | YES | `src/chroma_dragon_engine/shaders/base/mod.rs:22`, `src/cosmic_dragon_engine/cloud/tests/tests_monolith/depth.rs:143` | 7 files, 11 hits |
| `smallvec` | YES | `src/cosmic_dragon_engine/cloud/rain.rs:25`, `src/cosmic_dragon_engine/frame.rs:37` | 5 files, 6 hits |
| `unicode-width` | YES | `src/tests/width_guard.rs:6`, `src/cosmic_dragon_engine/frame.rs:5`, `src/scene/charset.rs:1` | 6 files, 17 hits |
| `notify` | YES | `src/config/live_config/mod.rs:48`, `src/config/live_config_poll/mod.rs:5` | 3 files, 16 hits |
| `sha2` | YES | `src/config/configfile.rs:30` (`use sha2::{Digest, Sha512}`), `src/config/live_config_poll/mod.rs:1` | 3 files, 4 hits |
| `signal-hook` | YES | `src/interactive/signal_handlers.rs:32,34,36`, `src/bench/bench_progress.rs:1` | 2 files, 4 hits |
| `libc` | YES | `src/clock/posix_time.rs:4-5`, `src/diagnostics/mod.rs:106`, `src/sysstat/cpustat.rs` | 17 files, 101 hits |
| `ctrlc` | YES | `src/bench/bench_progress.rs:82`, `src/interactive/signal_handlers.rs:1` | 2 files, 2 hits |
| `proptest` (dev) | YES | `src/tests/property.rs:12` (`use proptest::prelude::*`), `src/tests/property.rs:14` (`proptest!`) | 1 file, 2 hits |

**Verdict: ZERO stale dependencies.**

---

## 4. Levenshtein Removal Impact — Clean

The removed symbols were 100% pure Rust:

- `KNOWN_LONG_FLAGS` (static slice of `&str`)
- `cli_edit_distance` (Levenshtein algorithm — `Vec<char>` + `Vec<usize>`)
- `suggest_cli_flag` (scanner over `KNOWN_LONG_FLAGS`)

**No external crate was used by these symbols.** The removal could not have left a stale dep.

**Search for `strsim` / `levenshtein` / `edit-distance` crate in Cargo.toml**: NEGATIVE — no such crate is declared as a direct dep.

**Search in Cargo.lock**:

- `strsim v0.11.1` IS present at `Cargo.lock:699` — but as a **transitive** dep of `clap_builder` (via the `suggestions` feature), NOT a direct dep. This is the engine that powers clap's "tip:" suggestion line, which `extract_clap_suggestion()` in `src/main.rs:377` now parses. **NOT stale — actively used.**
- `levenshtein` / `edit-distance` crates: ABSENT from Cargo.lock.

**Residual `edit_distance` symbols in src/** (these are NOT the removed code — they are separate pure-Rust implementations for unrelated features):

- `src/cli/mod.rs:315` — `fn edit_distance(a, b)` — used for **theme name matching**
- `src/config/config_hints/mod.rs:260` — `fn edit_distance(a, b)` — used for **config key suggestions**
- `src/theme/mod.rs:347` — `fn theme_edit_distance(a, b)` — used for **theme alias matching** (comment explicitly notes it's a self-contained duplicate)

These could potentially be deduplicated (the `theme/mod.rs:345` comment explicitly notes the duplication), but that is a code-quality issue, NOT a dependency issue.

---

## 5. Supply-Chain Risk Assessment

| Crate | Risk | Notes |
|---|---|---|
| `clap` | LOW | Top-20 Rust crate. Maintained by clap-rs org. Pinned `>=4.5, <4.6` (allows patch upgrades only). |
| `crossterm` | LOW | Top-50 Rust crate. Maintained by crossterm-rs org. Pinned to exact minor `0.29`. |
| `rand` | LOW | Foundational crate, maintained by rust-random org. Constraint `0.9` is loose (allows 0.9.x patches). |
| `bitvec` | LOW | Maintained by ferrilab. Constraint `1` is loose. |
| `smallvec` | LOW | Servo heritage, maintained by servo team. |
| `unicode-width` | LOW | Maintained by unicode-rs org. |
| `notify` | LOW-MEDIUM | Maintained by notify-rs org. v6.1.1 is older (v7.x current). The `<7` cap is explicit but undocumented. |
| `sha2` | LOW | RustCrypto org. Pure-Rust, no `unsafe`. |
| `signal-hook` | LOW | Maintained by vorner. |
| `libc` | LOW | Foundational crate, rust-lang org. |
| `ctrlc` | LOW | Maintained by gnzlbg. |
| `proptest` (dev) | LOW | Maintained by altsysrq. Exact-pinned `1.8.0` — slightly unusual. |

**Transitive dep of note:**

- `strsim v0.11.1` — pulled in via `clap_builder`. Small (~1KB source), well-known crate maintained by Strainu. **LOW risk.** This is the expected consequence of enabling clap's `suggestions` feature.

**Overall supply-chain risk: LOW.** All direct deps are mainstream, actively-maintained crates. No obscure or single-maintainer crates. No crates with known advisories (would need `cargo audit` to confirm — see §7).

---

## 6. Compile-Time Burden Analysis

Full clean `cargo build --timings` (debug profile, fresh cache):

- **Wall-clock: 25.16s** (parallel, 8+ cores)
- **Sum of per-crate durations: 43.82s** (serial equivalent)

**Top 20 slowest-compiling crates (debug profile):**

| Rank | Crate | Duration (s) | Source |
|---|---|---|---|
| 1 | `cosmostrix` (project) | 6.13 | Project code |
| 2 | `clap_builder` | 4.64 | clap parser machinery |
| 3 | `syn` | 3.48 | Proc-macro engine (clap_derive) |
| 4 | `zerocopy` | 3.15 | rand → rand_chacha → ppv-lite86 → zerocopy |
| 5 | `bitvec` | 1.92 | Direct dep |
| 6 | `notify` | 1.72 | Direct dep |
| 7 | `clap_derive` | 1.59 | Proc-macro for #[derive(Parser)] |
| 8 | `crossterm` | 1.44 | Direct dep |
| 9 | `cosmostrix` build script | 0.99 | build.rs (subprocess calls) |
| 10 | `libc` | 0.94 | Direct dep — large FFI surface |
| 11 | `rustix` | 0.93 | crossterm (replaces some libc calls on Linux) |
| 12 | `mio` v1.2.2 | 0.72 | crossterm |
| 13 | `rand` | 0.71 | Direct dep |
| 14 | `typenum` | 0.62 | sha2 → digest → generic-array → typenum |
| 15 | `funty` | 0.59 | bitvec |
| 16 | `parking_lot` | 0.57 | crossterm |
| 17 | `strsim` | 0.54 | clap_builder (via `suggestions` feature) |
| 18 | `rand_chacha` | 0.53 | rand |
| 19 | `proc-macro2` | 0.52 | syn / clap_derive |
| 20 | `generic-array` | 0.50 | sha2 → digest |

**Heaviest dependency clusters:**

- **clap stack** (clap + clap_builder + clap_derive + clap_lex + syn + proc-macro2 + quote + heck + unicode-ident + anstream + anstyle + anstyle-parse + anstyle-query + colorchoice + utf8parse + is_terminal_polyfill + strsim + litrs via document-features): **~14s combined**
- **rand stack** (rand + rand_chacha + rand_core + ppv-lite86 + zerocopy + getrandom): **~7s combined** (zerocopy alone is 3.15s)
- **crossterm stack** (crossterm + mio + rustix + bitflags + parking_lot + lock_api + scopeguard + parking_lot_core + signal-hook-mio + linux-raw-sys + errno + document-features + litrs + log): **~5s combined**
- **notify stack** (notify + mio 0.8 + filetime + inotify + inotify-sys + walkdir + same-file + bitflags 1 + log): **~3s combined**
- **sha2 stack** (sha2 + digest + block-buffer + generic-array + typenum + crypto-common + cpufeatures + version_check + cfg-if): **~2s combined**

**Specifically addressed:**

- **Does `notify` pull many transitive deps?** Yes — 9 transitive crates including a duplicate `mio v0.8.11` and `bitflags v1.3.2`. Compile time: 1.72s for notify itself, ~3s total for the cluster.
- **Does `sha2` pull many transitive deps?** Yes — 9 transitive crates. Compile time: 0.5s for sha2 itself, ~2s for the cluster.
- **Does `clap` with the `suggestions` feature pull in `strsim`?** YES — confirmed via `cargo tree --invert strsim`. `strsim` adds **0.54s** compile time. This is intentional and now actively used by `extract_clap_suggestion()`. **NOT a stale dep.**

**Duplicate crates (compile-time waste):**

| Crate | v1 (source) | v2 (source) | Impact |
|---|---|---|---|
| `mio` | v0.8.11 ← `notify` 6.1.1 | v1.2.2 ← `crossterm` 0.29 + `signal-hook-mio` | 2 versions compiled |
| `bitflags` | v1.3.2 ← `notify` → `inotify` | v2.13.1 ← `crossterm`, `rustix` | 2 versions compiled |

Both duplicates originate from `notify 6.1.1` using the older `mio 0.8` / `bitflags 1` ecosystem, while `crossterm 0.29` uses the newer `mio 1.x` / `bitflags 2` ecosystem. **Upgrading notify to 7.x would unify both** (~1s compile-time win).

---

## 7. Feature Flag Review

### `clap` features: `std, color, help, usage, error-context, derive, suggestions`

| Feature | Used? | Evidence |
|---|---|---|
| `std` | YES | All clap usage assumes std |
| `color` | YES | `src/cli/mod.rs:51-78` defines `clap_styles()` returning `ClapStyles` with truecolor purple RGB (168, 85, 247). Applied in `src/main.rs:414`. |
| `help` | YES | Project ships `--help` (referenced throughout `src/cli/` and `src/main.rs`) |
| `usage` | YES | Required for `--help` usage line |
| `error-context` | YES | Required for clap's rich error messages including the "tip:" suggestion line |
| `derive` | YES | `src/config/mod.rs` uses `#[derive(Parser)]` |
| `suggestions` | YES (CRITICAL) | This is the feature that enables clap's built-in did-you-mean. `extract_clap_suggestion()` in `src/main.rs:377` parses the "tip:" line this feature emits. **Removing this feature would break the entire CLI suggestion system.** |

**All clap features are justified. None can be dropped.**

### `crossterm` features: `bracketed-paste, events, windows`

| Feature | Used? | Evidence |
|---|---|---|
| `bracketed-paste` | YES | `src/interactive/event_loop.rs`, `src/interactive/input.rs`, `src/interactive/tests.rs`, `src/tests/terminal.rs`, `src/cosmic_dragon_engine/terminal/mod.rs` |
| `events` | YES | All input handling in `src/interactive/` |
| `windows` | YES (cross-compile) | Enables Windows-specific event sources — needed when cross-compiling to `x86_64-pc-windows-msvc` / `aarch64-pc-windows-msvc`. On non-Windows hosts it compiles to a no-op. |

**All crossterm features are justified.**

### `notify` features (per-target)

| Target | Features | Justified? |
|---|---|---|
| default (Linux) | none (only `default-features = false`) | YES — Linux uses inotify backend (auto-selected) |
| macOS | `macos_fsevent` | YES — explicit comment: FSEvents is more efficient than kqueue on macOS |
| BSD/iOS | `kqueue` | YES — explicit comment: "Without these, notify falls back to a no-op backend on BSDs" |

**All notify features are justified.**

### `proptest` features: `std`

`std` is the minimum feature for any non-`no_std` test. Used in `src/tests/property.rs`. **Justified.**

**Feature audit verdict: No unused features detected. All enabled features map to a verified usage site.**

---

## 8. Recommendations

### Priority 1 — Investigate `notify` 7.x upgrade (compile-time win)

**Action:** Test bumping `notify = ">=6.1, <7"` to `notify = ">=7, <8"` in Cargo.toml.

**Rationale:** notify 6.1.1 is the source of both duplicate-crate problems:

- `mio v0.8.11` (notify) vs `mio v1.2.2` (crossterm) — notify 7.x migrated to mio 1.0
- `bitflags v1.3.2` (notify → inotify) vs `bitflags v2.13.1` (crossterm/rustix) — notify 7.x uses bitflags 2

**Expected compile-time savings:** ~0.7-1.0s (eliminating the duplicate mio + bitflags builds) plus reduced incremental cache pressure.

**Risk:** notify 7.x has API changes (the `Watcher::new` API was reworked). The two call sites to migrate are `src/config/live_config/mod.rs:48` and `src/config/live_config_poll/mod.rs`. The macOS `macos_fsevent` and BSD `kqueue` feature flags may have been renamed in v7 — needs verification.

**Owner decision needed:** The current `<7` cap is deliberate but undocumented. Either bump to v7 or document why v6 is pinned.

### Priority 2 — Run `cargo audit` (supply-chain hygiene)

**Action:** Install and run `cargo audit` to check for known CVEs in the locked versions.

**Rationale:** This audit confirmed all crates are mainstream and low-risk, but did not check the RUSTSEC advisory database. Specific versions to verify:

- `libc v0.2.189` (recent)
- `mio v0.8.11` (older — notify 6.x constraint)
- `bitflags v1.3.2` (very old — notify 6.x constraint)
- `zerocopy v0.8.56` (recent)

### Priority 3 — Relax `proptest` exact pin (consistency)

**Action:** Change `proptest = { version = "1.8.0", ... }` to `proptest = { version = "1.8", ... }` (or `^1.8`).

**Rationale:** `1.8.0` is the only exact-pinned version constraint in the file — inconsistent with the caret-style used everywhere else. Exact pinning a dev-dep provides no supply-chain benefit (dev-deps don't ship in the binary) and blocks patch upgrades that may include bug fixes.

### Priority 4 — Document the `notify` `<7` cap

**Action:** Add a one-line comment next to `notify = ">=6.1, <7"` explaining why v7 is excluded. If the reason is "haven't audited v7's API changes yet", say so. The current comment block only documents the *features* (macos_fsevent/kqueue), not the *version cap*.

### NOT Recommended — Do NOT remove `strsim`

`strsim v0.11.1` is in Cargo.lock as a transitive dep of `clap_builder` (via the `suggestions` feature). It is the engine that powers clap's "tip:" suggestion line, which `extract_clap_suggestion()` now parses. **Removing the `suggestions` feature would re-break the did-you-mean system.** The 0.54s compile cost is the price of admission.

### NOT Recommended — Do NOT remove the remaining `edit_distance` symbols

The three remaining `edit_distance` functions (`src/cli/mod.rs:315`, `src/config/config_hints/mod.rs:260`, `src/theme/mod.rs:347`) are pure-Rust helpers for theme name matching and config key suggestions — they do NOT use any external crate. They could be deduplicated, but that is a code-quality issue, not a dependency issue.

---

## 9. Verification matrix

| check | method | result |
|---|---|---|
| `cargo build` (debug) succeeds | `cargo build --quiet` | clean |
| `cargo test --bins` passes | `cargo test --bin cosmostrix` | 1716 passed / 0 failed / 2 ignored |
| `cargo fmt --check` clean | `cargo fmt -- --check` | clean |
| `cargo clippy -- -D warnings` clean | `cargo clippy -- -D warnings` | clean |
| `./scripts/gate-keepers.sh` | gatekeeper | 8/8 PASS |
| `./scripts/check-version-anti-patterns.sh` | anti-pattern check | 227 files clean |
| Direct deps usage verified | `rg` per crate | all 11 + 1 dev-dep used |
| Transitive dep count | `cargo tree` | 65 production + 6 dev = 71 total |
| Duplicate crates identified | `cargo tree` | mio (2 versions), bitflags (2 versions) — both from notify 6.x |
