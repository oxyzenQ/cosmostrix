# GitHub Actions workflows
<!-- SPDX-License-Identifier: GPL-3.0-only -->

CI and release pipeline reference. Workflow files live under `.github/workflows/`.

## Workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `ci.yml` | push + PR to `main` | fmt, clippy, test, build, security audit, version sync |
| `release.yml` | tag push `v*` | 8-platform binaries + checksums + GPG sign + GitHub Release |
| `maintenance.yml` | weekly cron (Mon 00:00 UTC) | `cargo update` + audit + commit if validation passes |
| `gitbot-audit.yml` | daily cron | `cargo audit` + `cargo deny` (observation-only) |
| `aur.yml` | release | Update AUR `cosmostrix-bin` package |
| `crates-io.yml` | tag push `v*` (stable + pre-release) | Publish the crate to crates.io (`cargo publish --locked`, idempotent) |
| `miri.yml` | weekly cron (Sun 03:00 UTC) | Undefined behavior detection |
| `codeql.yml` | push + PR (path-filtered) + weekly cron | CodeQL static analysis, auto-detected languages |
| `cosmic-dragon-guard.yml` | push + PR to `main` | `gate-keepers.sh`: shell triad, yamllint, actionlint, TOML, markdownlint, codespell, ruff, naming, SPDX, LOC, version sync, disclaimer |
| `workflow-ci.yml` | push + PR to `main` (`.github/**` paths) | actionlint + yamllint + YAML syntax for every workflow file |

## Dependency version policy (owner decision 2026-08-30)

Zero hardcoded dependency versions in `.github/*`. The rule is
"dynamic latest, minimal maintenance, boring but strong":

- **GitHub Actions** float on their major tag (`actions/checkout@v6`,
  `Swatinem/rust-cache@v2`). Minor and patch releases arrive
  automatically; a new major requires a one-character review.
- **CI-installed deps** (shfmt, codespell, ruff, yamllint, shellcheck,
  markdownlint-cli, actionlint, cargo-audit, cargo-deny, Android NDK)
  resolve their latest upstream release at run time. shfmt is fetched
  from the `mvdan/sh` GitHub releases API; pip/npm/go/apt installs are
  unpinned; `nttld/setup-ndk` uses `ndk-version: latest`.
- **Rust toolchain is the deliberate exception**: it is LTS-locked, not
  floating. `rust-toolchain.toml` is the single source of truth; CI jobs
  that pass an explicit version use the `RUST_VERSION` env, which gate
  check 9 (`check-rust-version-sync.sh`) keeps in lockstep. Bumping is
  one command: `./scripts/rust-version-to.sh X.Y.Z`. A floating Rust
  toolchain can silently break the build the day a new stable ships;
  the lock is what makes CI boring.

Trade-off accepted by the owner: a future tool release with new default
rules (e.g. ruff, shfmt formatting) can turn the gate red. The fix is a
one-commit tree refresh (`shfmt -w scripts/*.sh`, fix new lint
findings), which is cheaper than carrying version pins for every tool
and bumping them forever.

## Release channels (tag conventions)

- `vX.Y.Z-alpha.N` / `vX.Y.Z-beta.N` / `vX.Y.Z-rc.N` -> GitHub **prerelease** + crates.io publish
- `vX.Y.Z` -> GitHub **normal release** (eligible for Latest) + crates.io publish

## crates.io publishing

The crate is published by `crates-io.yml` on every owner-pushed `v*` tag
(stable and pre-release both trigger). One-time setup: create a
crates.io API token (Account Settings -> API Tokens; the
`publish-new` scope is enough) and add it as the `CRATES_IO_TOKEN`
repository secret (Settings -> Secrets and variables -> Actions).

Workflow safety properties: it fails fast if the tag does not match
`Cargo.toml`'s version, skips the upload when the version is already on
the registry (re-pushed tags / re-runs stay green), and publishes with
`--locked` so the shipped dependency tree is exactly the tagged
`Cargo.lock`. Users install with `cargo install cosmostrix` (see README
-> Installation).

## Build matrix (release.yml)

Linux x86_64 (`v3`, `v4`, `musl`), Linux aarch64, macOS aarch64, Windows x86_64, Windows aarch64, Android aarch64 (cross-compiled).

Linux x86_64 baselines: `v3` = `-C target-cpu=x86-64-v3`, `v4` = `-C target-cpu=x86-64-v4`, `musl` = `x86-64-v3` static. v1/v2 were dropped in v10.0.0 (modern CPUs support v3).

## Packaging output

Each build produces `cosmostrix-<tag>-<platform>.tar.gz` + `.sha512sum` (+ `.zip` + `.sha512sum` for Windows). Archive contains `cosmostrix` binary, `README.md`, `LICENSE`.

Verify a download:

```bash
sha512sum -c cosmostrix-vX.Y.Z-linux-amd64-v3.tar.gz.sha512sum
```

## Version bump

Single source of truth: `Cargo.toml` `[package] version`. Every other active version reference is derived from it — via `env!("CARGO_PKG_VERSION")` at compile time, or `./scripts/version-to.sh` for files that must contain a literal version string.

### Bump + build

```bash
./scripts/version-to.sh X.Y.Z        # bump across all active files
./scripts/build.sh release           # build a release binary
./scripts/build.sh pgo --auto        # or a PGO nitro build
./scripts/build.sh version-sync      # verify all version refs agree (no build)
```

If the repo is already at the requested version, `version-to.sh` is a no-op.

### What `version-to.sh` updates

```bash
./scripts/version-to.sh X.Y.Z
git diff
git commit -m "chore: bump version to vX.Y.Z"
git tag vX.Y.Z
git push origin main vX.Y.Z
```

The script updates:

- `Cargo.toml` (package version)
- `Cargo.lock` (root package version only)
- `aur/cosmostrix-bin/PKGBUILD` (`pkgver=`, `_tag=`)
- `aur/cosmostrix-bin/.SRCINFO` (regenerated from PKGBUILD)
- `README.md` (the `TAG="vX.Y.Z"` install snippet)
- `docs/workflow/ABOUT_CI.md` (the `TAG="vX.Y.Z"` install snippet, if present)

It skips CHANGELOG headings (historical record) and audits workflow files for hardcoded versions (workflows should derive versions from `GITHUB_REF_NAME`).

`version-to.sh` accepts both stable (`X.Y.Z`) and pre-release
(`X.Y.Z-alpha.N` / `-beta.N` / `-rc.N` / `-pre.N` / `-nightly.N`)
versions; the tag for a release is created manually by the owner
(`git tag vX.Y.Z && git push origin vX.Y.Z`).

Verify the current version without changes:

```bash
./scripts/version-to.sh --check X.Y.Z
```

## CI fail-fast guard

CI runs `./scripts/build.sh version-sync` as a dedicated job **before** any Rust build, so a version desync fails the pipeline in seconds. `scripts/check-version-anti-patterns.sh` blocks re-introduction of hardcoded version assertions in `src/`. The compile-time guard in `src/docs_tests/metadata.rs` asserts `Cargo.toml`, `PKGBUILD`, `.SRCINFO`, and the README install tag all agree with `env!("CARGO_PKG_VERSION")`.
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
