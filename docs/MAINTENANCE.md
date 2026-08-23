# Maintenance Guide
<!-- SPDX-License-Identifier: GPL-3.0-only -->

Single reference for maintaining cosmostrix during dormant mode (5-10 year maintenance cycle). Covers build, test, dependency updates, security response, and health-check log.

Cosmostrix is built to survive. The owner may go dormant for 5-10 years. When returning, this file is the only document needed to bring the project back to a fully passing CI state. Every command, every check, every response procedure is here.

## 1. Quick Reference

| Task | Command |
|------|---------|
| Build (debug) | `cargo build` |
| Build (release) | `cargo build --release` |
| Build (optimized, AVX-512) | `cargo pro-native` |
| Build (PGO nitro) | `./scripts/build.sh pgo` |
| Test (full suite) | `cargo test --all --locked` |
| Gatekeeper (all checks) | `./scripts/build.sh check-all` |
| Format check | `cargo fmt --all --check` |
| Lint | `cargo clippy -- -D warnings` |
| Security audit | `cargo deny check all` |
| Validate config | `cosmostrix --testconf` |
| Diagnostics | `cosmostrix --doctor` |
| Install (user-local) | `./scripts/install.sh --user` |
| Uninstall | `./scripts/uninstall.sh --user` |
| Version bump | `./scripts/version-to.sh vX.Y.Z` |

## 2. Build Environments

**Pinned toolchain**: Rust 1.97.1 (`rust-toolchain.toml`), MSRV 1.97 (`Cargo.toml` `rust-version`), profile: minimal + rustfmt + clippy.

**Upgrading Rust**: (1) update `rust-toolchain.toml` channel; (2) update `Cargo.toml` `rust-version` if MSRV changed; (3) update all `.github/workflows/*.yml` `toolchain:` refs; (4) `./scripts/build.sh check-all`; (5) `cargo test --all --locked`; (6) commit `chore: bump Rust toolchain to X.Y.Z`.

**Dependencies**: `Cargo.lock` committed (reproducible builds), 64 direct deps / 98 total crates, `deny.toml` + CI `cargo deny check all` daily. To update: `cargo update` → `cargo deny check advisories` → `cargo test --all --locked` → `./scripts/build.sh check-all`. Commit `Cargo.lock` only if all checks pass.

## 3. CI/CD Pipeline

| Workflow | File | Trigger | Purpose |
|----------|------|---------|---------|
| CI | `ci.yml` | push + PR (src/**) | Build + test + clippy + fmt + deny + MSRV |
| Cosmic Dragon Guard - Gate-keepers | `cosmic-dragon-guard.yml` | push + PR | Gate-keepers: shellcheck, yamllint, actionlint, markdownlint, codespell, SPDX, LOC |
| Workflow CI | `workflow-ci.yml` | push + PR (.github/**) | Validate workflow YAML syntax + actionlint |
| Miri | `miri.yml` | weekly cron (Sun 03:00 UTC) | Undefined behavior detection (6 audited modules) |
| Security Audit | `gitbot-audit.yml` | daily cron | Security advisory + dependency policy |
| CodeQL | `codeql.yml` | weekly cron (Mon 03:00 UTC) | GitHub CodeQL semantic analysis |
| AUR | `aur.yml` | release tag | Update AUR package |
| Release | `release.yml` | tag push (v*) | Build 10 platform binaries + PGO + checksums + GPG sign |
| Maintenance | `maintenance.yml` | weekly cron (Mon 07:00 WIB) | Dependency update + validate + commit |

## 4. Security Advisory Response

If `cargo deny check advisories` or GitHub Dependabot reports a vulnerability:

1. **Assess severity**: direct dep or transitive? Does the vulnerable code path execute?
2. **Update the dependency**: `cargo update -p <crate-name>` → `cargo deny check advisories` → `cargo test --all --locked` → `./scripts/build.sh check-all`.
3. **Commit**: `security: update <crate> for CVE-XXXX-XXXXX`.
4. **Tag release** if user-facing: `./scripts/version-to.sh vX.Y.Z`.

### Symlink Handling

`--config <path>` enforces a directory whitelist via `validate_config_path()` (see `src/safepath/mod.rs`). Symlinks pointing outside the whitelist are rejected at the validation layer. The configfile parser reads the target strictly as TOML text — no `eval`, `include`, or recursive resolution; no env vars, secrets, or shell expansion. A symlink swap can at most feed different TOML content, which `--testconf` catches. The watcher (`src/config/live_config_poll/`) re-validates on every reload. **Future hardening** (not required for v50 stable): switch to `fstatat` with `AT_SYMLINK_NOFOLLOW` and reject any path crossing a symlink boundary.

## 5. Periodic Health Check

**Schedule**: every 6 months (or when returning from dormant period).

If returning after 5-10 years of dormancy, follow this exact sequence. No steps may be skipped.

1. **Clean clone**: `git clone https://github.com/oxyzenQ/cosmostrix.git && cd cosmostrix`
2. **Install toolchain**: `rustup install 1.97.1 && rustup default 1.97.1` (or whatever `rust-toolchain.toml` says)
3. **Build**: `cargo build --release`
4. **Test**: `cargo test --all --locked`
5. **Gatekeeper**: `./scripts/build.sh check-all -q`
6. **Security audit**: `cargo deny check all`
7. **Dependency update** (if any CVEs): `cargo update` → repeat steps 4-6
8. **Rust toolchain upgrade** (if current Rust is EOL): see Section 2 "Upgrading Rust"
9. **Benchmark** (optional): `./target/release/cosmostrix --benchmark --scene monolith --bench-duration 5`
10. **Log result** in the table below.

### Health Check Log

| Date | Rust Version | Result | Notes |
|------|-------------|--------|-------|
| 2026-08-13 | 1.97.1 | PASS | Full audit session — ~1649 tests (then), all quality gates green. Current: ~1500+ pass. |

## 6. Dormant Mode Contract

Cosmostrix is designed for long-term stability. The owner may go dormant for 5-10 years without touching the codebase. When returning, the project must compile and pass all tests on the pinned toolchain with zero intervention beyond `cargo build && cargo test`.

### What "dormant" means

- No commits, no releases, no dependency updates during the dormant period.
- CI continues running automatically: daily security audits, weekly Miri, weekly CodeQL, weekly dependency maintenance.
- The AUR package remains available at the last published version.
- Issues and PRs may accumulate; the maintenance workflow handles stale management.
- The `Cargo.lock` is committed and immutable during dormancy — it is the single source of truth for dependency versions.

### What "returning from dormancy" means

- Follow the Periodic Health Check (Section 5) exactly. No steps may be skipped.
- If the pinned Rust toolchain is EOL or unavailable: upgrade per Section 2.
- If CVEs exist in dependencies: update per Section 4.
- If CI is red: fix it before any feature work.
- The project must be at a fully green CI state before any new development.
- Do not batch unrelated changes with the return-from-dormancy commit. One commit per concern.

### Offline build resilience

After 5-10 years of dormancy, external services may be unavailable. The pinned toolchain and locked dependencies provide a survival baseline:

- **rustup**: the toolchain installer caches locally (`~/.rustup/toolchains/`). If rustup.rs is unreachable, an existing local installation still works. As a backup, Rust toolchain archives are mirrored on GitHub Releases (`rust-lang/rust`), and many Linux distros ship Rust in their package repositories.
- **crates.io**: `Cargo.lock` pins exact dependency versions. If crates.io is unreachable, the global cargo cache (`~/.cargo/registry/`) from the last successful build still contains the sources. For long-term archival, archive the `~/.cargo/registry/` directory alongside the repo.
- **Cargo deny**: if `cargo-deny` is unavailable or its advisory database is unreachable, skip `cargo deny check all` and proceed with the remaining checks. Re-enable once connectivity is restored.

### Dormant-mode invariants (must hold after any maintenance session)

- `cargo build --release` compiles with zero warnings on the pinned toolchain.
- `cargo test --all --locked` passes all tests.
- `./scripts/build.sh check-all -q` passes all quality gates.
- `cargo deny check all` reports no advisories (or is skipped if offline).
- `cosmostrix --testconf` validates the default config without errors.
- `cosmostrix --doctor` reports no hard failures (warnings are acceptable).
- No new dependencies were added without owner approval.
- No existing CLI flag, config key, scene name, color theme, or charset preset was removed or renamed.
- All CI workflows pass on push to main.
- No behavioral regression: `--benchmark` throughput on the pinned toolchain must not decrease by more than 5% from the last logged Health Check result.

## 7. API Stability Promise

From v50.0.0 onward, the following are **frozen** (no breaking changes without a major version bump):

- **CLI flags**: all flags in `--help` (names, short/long forms, value types)
- **Config format**: `config.toml` keys, value types, and TOML structure
- **Scene names**: all 18 built-in scene names
- **Color scheme names**: all 44 built-in color scheme names (`THEME_COUNT` in `src/theme/mod.rs`)
- **Charset preset names**: all 25 built-in charset names
- **Runtime controls**: all keyboard shortcuts (q, Space, c/C, s/S, p, x, i, [/], Up/Down)
- **Output format**: `--json` benchmark output schema, `--doctor` report format

Breaking changes require a major version bump (e.g. v51.0.0). Minor versions (v50.1.0) may add new features but must not change or remove existing API surface.

## 8. Architecture Reference

- [`docs/audits/COSMIC_DRAGON_AUDIT.md`](audits/COSMIC_DRAGON_AUDIT.md) — comprehensive audit (visual quality, stability, power management, competitive depth)
- [`docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md`](archive/audits/UNSAFE_SOUNDNESS_AUDIT.md) — unsafe block soundness audit + Miri methodology
- [`docs/README.md`](README.md) — docs index with source module map

Copyright (C) 2026 rezky_nightky (oxyzenQ). All rights reserved.
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
