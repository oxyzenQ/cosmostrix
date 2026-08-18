# Maintenance Guide
<!-- SPDX-License-Identifier: GPL-3.0-only -->

Single reference for maintaining cosmostrix during dormant mode (5-10 year maintenance cycle). Covers build, test, dependency updates, security response, and health-check log.

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
| CI | `ci.yml` | push + PR | Build + test + clippy + fmt + deny |
| Release | `release.yml` | tag push | Build 8 platform binaries + checksums + GPG sign |
| Miri | `miri.yml` | weekly cron (Sun 03:00 UTC) | Undefined behavior detection |
| Docs CI | `docs-ci.yml` | push | Verify doc links + code blocks |
| Gitbot audit | `gitbot-audit.yml` | daily cron | Security advisory + dependency policy |
| AUR | `aur.yml` | release | Update AUR package |
| Maintenance | `maintenance.yml` | weekly cron | Stale issue/PR management |

## 4. Security Advisory Response

If `cargo deny check advisories` or GitHub Dependabot reports a vulnerability:

1. **Assess severity**: direct dep or transitive? Does the vulnerable code path execute?
2. **Update the dependency**: `cargo update -p <crate-name>` → `cargo deny check advisories` → `cargo test --all --locked` → `./scripts/build.sh check-all`.
3. **Commit**: `security: update <crate> for CVE-XXXX-XXXXX`.
4. **Tag release** if user-facing: `./scripts/version-to.sh vX.Y.Z`.

### Symlink Handling

`--config <path>` enforces a directory whitelist via `validate_config_path()` (see `src/safepath.rs`). Symlinks pointing outside the whitelist are rejected at the validation layer. The configfile parser reads the target strictly as TOML text — no `eval`, `include`, or recursive resolution; no env vars, secrets, or shell expansion. A symlink swap can at most feed different TOML content, which `--testconf` catches. The watcher (`src/live_config.rs`) re-validates on every reload. **Future hardening** (not required for v50 stable): switch to `fstatat` with `AT_SYMLINK_NOFOLLOW` and reject any path crossing a symlink boundary.

## 5. Periodic Health Check

**Schedule**: every 6 months (or when returning from dormant period).

1. **Clean clone**: `git clone https://github.com/oxyzenQ/cosmostrix.git && cd cosmostrix`
2. **Build**: `cargo build --release`
3. **Test**: `cargo test --all --locked`
4. **Gatekeeper**: `./scripts/build.sh check-all`
5. **Security audit**: `cargo deny check all`
6. **Benchmark** (optional): `./target/release/cosmostrix --benchmark --scene monolith --bench-duration 5`
7. **Log result** in the table below.

### Health Check Log

| Date | Rust Version | Result | Notes |
|------|-------------|--------|-------|
| 2026-08-13 | 1.97.1 | PASS | Full audit session — 1,476 tests, all quality gates green |

## 6. API Stability Promise

From v50.0.0 onward, the following are **frozen** (no breaking changes without a major version bump):

- **CLI flags**: all flags in `--help` (names, short/long forms, value types)
- **Config format**: `config.toml` keys, value types, and TOML structure
- **Scene names**: all 18 built-in scene names
- **Color scheme names**: all 44 built-in color scheme names
- **Charset preset names**: all 25 built-in charset names
- **Runtime controls**: all keyboard shortcuts (q, p, c/C, s/S, x, [/], etc.)
- **Output format**: `--json` benchmark output schema, `--doctor` report format

Breaking changes require a major version bump (e.g. v51.0.0). Minor versions (v50.1.0) may add new features but must not change or remove existing API surface.

## 7. Architecture Reference

- [`docs/audits/COSMIC_DRAGON_AUDIT.md`](audits/COSMIC_DRAGON_AUDIT.md) — comprehensive audit (visual quality, stability, power management, competitive depth)
- [`docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md`](archive/audits/UNSAFE_SOUNDNESS_AUDIT.md) — unsafe block soundness audit + Miri methodology
- [`docs/README.md`](README.md) — docs index with source module map

Copyright (C) 2026 rezky_nightky (oxyzenQ). All rights reserved.
