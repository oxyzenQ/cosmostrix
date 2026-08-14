# Maintenance Guide

<!-- SPDX-License-Identifier: GPL-3.0-only -->

> **Purpose**: This document is the single reference for maintaining cosmostrix
> during dormant mode (5-10 year maintenance cycle). It covers build, test,
> dependency updates, security response, and a health-check log.

---

## 1. Quick Reference

| Task | Command |
|------|---------|
| **Build (debug)** | `cargo build` |
| **Build (release)** | `cargo build --release` |
| **Build (optimized, AVX-512)** | `cargo pro-native` |
| **Build (PGO nitro)** | `./scripts/build.sh pgo` |
| **Test (full suite)** | `cargo test --all --locked` |
| **Gatekeeper (all checks)** | `./scripts/build.sh check-all` |
| **Format check** | `cargo fmt --all --check` |
| **Lint** | `cargo clippy -- -D warnings` |
| **Security audit** | `cargo deny check all` |
| **Validate config** | `cosmostrix --testconf` |
| **Diagnostics** | `cosmostrix --doctor` |
| **Install (user-local)** | `./scripts/install.sh --user` |
| **Uninstall** | `./scripts/uninstall.sh --user` |
| **Version bump** | `./scripts/version-to.sh vX.Y.Z` |

---

## 2. Build Environments

### Pinned toolchain

- **Rust**: 1.97.1 (pinned in `rust-toolchain.toml`)
- **MSRV**: 1.81 (declared in `Cargo.toml` `rust-version`)
- **Profile**: minimal + rustfmt + clippy

When upgrading Rust:
1. Update `rust-toolchain.toml` channel to the new version
2. Update `Cargo.toml` `rust-version` to the new MSRV (if changed)
3. Run `./scripts/build.sh check-all` — all 11 checks must pass
4. Run `cargo test --all --locked` — all 1,476 tests must pass
5. Commit with message: `chore: bump Rust toolchain to X.Y.Z`

### Dependencies

- **Cargo.lock**: committed — builds are reproducible
- **Direct deps**: 64 (in `Cargo.toml`)
- **Total crates** (incl. transitive): 98
- **Security policy**: `deny.toml` + CI runs `cargo deny check all` daily

To update dependencies:
```bash
cargo update                    # update all to latest compatible
cargo deny check advisories     # verify no security advisories
cargo test --all --locked       # verify no breakage
./scripts/build.sh check-all    # full gatekeeper
```

Only commit `Cargo.lock` if all checks pass.

---

## 3. CI/CD Pipeline

| Workflow | File | Trigger | Purpose |
|----------|------|---------|---------|
| CI | `.github/workflows/ci.yml` | push + PR | Build + test + clippy + fmt + deny on Linux/macOS/Windows |
| Release | `.github/workflows/release.yml` | tag push | Build 8 platform binaries + checksums + GPG sign |
| Miri | `.github/workflows/miri.yml` | weekly cron (Sun 03:00 UTC) | Undefined behavior detection |
| Docs CI | `.github/workflows/docs-ci.yml` | push | Verify doc links + code blocks |
| Gitbot audit | `.github/workflows/gitbot-audit.yml` | daily cron | Security advisory + dependency policy |
| AUR | `.github/workflows/aur.yml` | release | Update AUR package |
| Maintenance | `.github/workflows/maintenance.yml` | schedule | Stale issue/PR management |

---

## 4. Security Advisory Response

If `cargo deny check advisories` or GitHub Dependabot reports a vulnerability:

1. **Assess severity**: Is it in a direct dep or transitive?
2. **Check if cosmostrix is affected**: Does the vulnerable code path execute?
3. **Update the dependency**:
   ```bash
   cargo update -p <crate-name>
   cargo deny check advisories
   cargo test --all --locked
   ./scripts/build.sh check-all
   ```
4. **Commit**: `security: update <crate> for CVE-XXXX-XXXXX`
5. **Tag release** if the fix is user-facing: `./scripts/version-to.sh vX.Y.Z`

### 4.1 Symlink Handling

`--config <path>` enforces a directory whitelist via `validate_config_path()`
(see `src/safepath.rs`). The path the user passes must resolve inside an
allowed directory; symlinks pointing **outside** the whitelist are rejected
at the validation layer.

**What is not explicitly tested**: symlinks that resolve *inside* the
whitelisted directory but were created by another user (TOCTOU between the
whitelist check and the file read). This is acceptable because:

- The configfile parser reads the target strictly as **TOML text**. It does
  not `eval`, `include`, or recursively resolve `[include]` directives — the
  file content cannot escape the parser's value-typed surface.
- No environment variables, secrets, or shell expansion are performed during
  config parsing. A symlink swap can at most feed the user *different TOML
  content* than they expected, which the type-checked parser rejects loudly
  via `--testconf`.
- An attacker needs **filesystem write access inside the whitelisted
  directory** to plant a symlink — at which point they already have the
  same read/write authority as the cosmostrix process for that directory.

**Mitigations already in place**:
1. `validate_config_path()` rejects `..` traversal and absolute paths
   outside the whitelist.
2. `--testconf` is the recommended pre-flight check before any production
   run — a swapped symlink will produce different TOML and be caught.
3. The watcher thread (`src/live_config.rs`) re-validates the path on every
   reload, so a swap mid-session is detected.

**Future hardening (not required for v50 stable)**: if a real-world threat
model emerges, switch `validate_config_path()` to `fstatat` with
`AT_SYMLINK_NOFOLLOW` and reject any path that crosses a symlink boundary.
This requires `std::os::unix::fs::symlink_metadata` plus a path-component
walk — non-trivial portability work for Windows, hence deferred.

---

## 5. Periodic Health Check

**Schedule**: Every 6 months (or when returning from dormant period).

### Procedure

1. **Clean clone** on a fresh machine:
   ```bash
   git clone https://github.com/oxyzenQ/cosmostrix.git
   cd cosmostrix
   ```

2. **Build from source**:
   ```bash
   cargo build --release
   ```

3. **Run full test suite**:
   ```bash
   cargo test --all --locked
   ```

4. **Run gatekeeper**:
   ```bash
   ./scripts/build.sh check-all
   ```

5. **Run security audit**:
   ```bash
   cargo deny check all
   ```

6. **Run benchmark** (optional — verify no regression):
   ```bash
   ./target/release/cosmostrix --benchmark --scene monolith --bench-duration 5
   ```

7. **Log the result** in the table below.

### Health Check Log

| Date | Rust Version | Result | Notes |
|------|-------------|--------|-------|
| 2026-08-13 | 1.97.1 | ✅ PASS | Full audit session — 1,476 tests, all quality gates green |

---

## 6. API Stability Promise

From v50.0.0 onward, the following are **frozen** (no breaking changes
without a major version bump):

- **CLI flags**: all flags in `--help` (names, short/long forms, value types)
- **Config format**: `config.toml` keys, value types, and TOML structure
- **Scene names**: all 18 built-in scene names
- **Color scheme names**: all 44 built-in color scheme names
- **Charset preset names**: all 25 built-in charset names
- **Runtime controls**: all keyboard shortcuts (q, p, c/C, s/S, x, [/], etc.)
- **Output format**: `--json` benchmark output schema, `--doctor` report format

Breaking changes to any of the above require a major version bump
(e.g. v51.0.0). Minor versions (v50.1.0) may add new features but must
not change or remove existing API surface.

---

## 7. Architecture Reference

For the full architecture audit, see:
- [`docs/audits/COSMIC_DRAGON_AUDIT.md`](audits/COSMIC_DRAGON_AUDIT.md) — comprehensive audit covering visual quality, stability, power management, and competitive depth
- [`docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md`](archive/audits/UNSAFE_SOUNDNESS_AUDIT.md) — unsafe block soundness audit + Miri methodology
- [`docs/README.md`](README.md) — docs index with source module map

---

Copyright (C) 2026 rezky_nightky (oxyzenQ). All rights reserved.
