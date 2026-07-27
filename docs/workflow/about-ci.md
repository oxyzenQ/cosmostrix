# GitHub Actions workflows
<!-- SPDX-License-Identifier: GPL-3.0-only -->

This repository uses GitHub Actions for CI and releases.

Workflow files live under:

- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `.github/workflows/maintenance.yml`

## Overview

### CI (`.github/workflows/ci.yml`)

#### Triggers

- `push` to `main`
- `pull_request` targeting `main`

#### What it does

- **Security audit**: runs `cargo-audit` using `cargo +stable` to avoid MSRV breakage when `cargo-audit` bumps its required Rust version.
- **MSRV**: runs `cargo test --all` on Rust `1.81.0`.
- **Test + Build (debug)**: runs `cargo test --all` and `cargo build --scene-custom dev`.
- **Release variant sanity**: builds optimized Linux/macOS/Windows/Android targets, verifies embedded build metadata, and runs `cosmostrix -i` whenever the artifact can safely execute on the runner.
- **Format + Clippy**: runs `cargo fmt -- --check` and `cargo clippy ... -D warnings`.
- **Dependency policy**: installs `cargo-deny` and runs `cargo +stable deny check all`.

#### Notes

- This workflow is meant to keep `main` green and enforce formatting/lints/policy.

### Release (`.github/workflows/release.yml`)

#### Triggers

- `push` tag matching `v*` (recommended)

#### Tag conventions (release channel)

- `vX.Y.Z-alpha.N`, `vX.Y.Z-beta.N`, `vX.Y.Z-rc.N` => published as **prerelease** (not Latest)
- `vX.Y.Z` => published as a **normal release** (eligible to become **Latest**)

#### What it builds

- Linux x86_64 (v1/v2/v3/v4) (runner host build)
- Linux aarch64 native (runner host build)
- macOS aarch64 native (runner host build)
- Windows x86_64 (universal) (runner host build)
- Windows aarch64 native (runner host build)
- Android aarch64 native (cross-compiled): `aarch64-linux-android`

#### Build steps (per platform job)

- Tests: `cargo test --all --locked`
- Builds:
  - `cargo build --scene-custom dev --locked`
  - `cargo build --scene-custom release --locked`
- Checks:
  - `cargo fmt -- --check`
  - `cargo clippy --locked --all-targets --all-features -- -D warnings`
  - `cargo +stable deny check all`
  - `cosmostrix -i` metadata checks for runnable artifacts:
    - expected `variant`
    - `dispatch: static optimized build`
    - expected `cpu_baseline`
    - compile-time `target_features` required for the claimed CPU tier
    - `lto: fat`
    - `panic: unwind`
    - `strip: yes`
  - embedded metadata scan for cross-built or unsafe-to-run artifacts
  - Unix stripped-binary check with a clear failure if metadata says stripped but the artifact is not stripped

Linux x86_64 release artifacts are built with explicit baselines:

- `v3`: `-C target-cpu=x86-64-v3`
- `v4`: `-C target-cpu=x86-64-v4`
- `musl`: `-C target-cpu=x86-64-v3` (statically linked, max portability)

> **Note:** v1/v2 were dropped in v10.0.0. Modern CPUs (2013+) support v3.
> Use `musl` for Alpine/containers/minimal base images.

`target-cpu=native` is reserved for local/native non-x86_64 release jobs and
developer aliases; it is not used for distributed Linux x86_64 artifacts.
The build script fails official Linux x86_64 tier builds when the claimed
variant and Cargo's compile-time `CARGO_CFG_TARGET_FEATURE` set disagree.

#### Packaging output

Each build produces:

- `cosmostrix-bin-<tag>-<platform>.tar.gz`
- `cosmostrix-bin-<tag>-<platform>.tar.gz.sha512sum`
- `cosmostrix-bin-<tag>-<platform>.zip`
- `cosmostrix-bin-<tag>-<platform>.zip.sha512sum`

Where `<tag>` is a git tag like `v1.0.0`.

Where `<platform>` is one of:

- `linux-amd64-v3`
- `linux-amd64-v4`
- `linux-amd64-musl`
- `linux-aarch64`
- `darwin-aarch64-native`
- `windows-x86_64`
- `windows-aarch64-native`
- `android-aarch64-native`

The archive contains:

- `cosmostrix` binary
- `README.md`
- `LICENSE`

#### Checksums

Checksum files are generated using:

- `sha512sum` when available, else
- `shasum -a 512`

Verification examples:

```bash
# Linux
sha512sum -c cosmostrix-bin-v10.0.0-linux-amd64-v3.tar.gz.sha512sum

# macOS
shasum -a 512 -c cosmostrix-bin-v1.0.0-darwin-aarch64-native.tar.gz.sha512sum
```

#### Release publishing

The `publish_release` job:

- downloads all build artifacts
- generates release notes from git history (since previous `v*` tag)
- creates a GitHub Release and uploads all `*.tar.gz`, `*.tar.gz.sha512sum`, `*.zip`, and `*.zip.sha512sum` files

## Typical release flow

```bash
# 1) Update Cargo.toml version
# 2) Commit the version bump
git commit -am "release: 4.0.0"

# 3) Create an annotated tag
git tag -a v4.0.0 -m v4.0.0

# 4) Push the tag (this triggers the Release workflow)
git push origin v4.0.0

# 5) if want to delete/repush
git tag -d v4.0.0
git push origin :refs/tags/v4.0.0
git tag -a v4.0.0 -m v4.0.0
git push origin v4.0.0
```

- The **Release** workflow will run on that tag.
- `-alpha.*` / `-beta.*` / `-rc.*` tags are marked as prerelease.
- Stable tags use the simple `vX.Y.Z` format (no `-stable.N` suffix).

### Maintenance deps weekly (`.github/workflows/maintenance.yml`)

#### Triggers

- `schedule` weekly at **00:00 UTC Monday (07:00 WIB Monday)**
- `workflow_dispatch` manual run

#### Manual inputs

- None. Updates are always validated and committed directly to `main` by `github-actions[bot]`.

#### What it does

- Runs `cargo update`
- Runs `cargo +stable audit` and `cargo +stable deny check all`
- Runs `cargo fmt -- --check` and basic build/test/clippy on toolchain `1.81.0`
- Commits and pushes to `main` only after validation passes

#### Notes

- GitHub cron uses UTC; adjust the schedule if you want a different local time.

## Version bump

The single source of truth for the package version is `Cargo.toml`'s `[package] version` field. Every other active version reference in the repo is derived from it — either at compile time via `env!("CARGO_PKG_VERSION")` in Rust source, or by `./scripts/version-to.sh` for files that must contain a literal version string (PKGBUILD, .SRCINFO, README install example, docs/workflow/about-ci.md).

### Bump + build (recommended)

Bump the repo with `./scripts/version-to.sh`, then trigger a build separately with `./scripts/build.sh`:

```bash
./scripts/version-to.sh v25.0.0         # bump to v25.0.0 across all active files
./scripts/build.sh release              # then build a release binary
./scripts/build.sh pgo --auto           # or a PGO nitro build
./scripts/build.sh version-sync         # verify all version refs agree (no build)
```

If the repo is already at the requested version, `version-to.sh` is a no-op (verification only, no writes).

### What version-to.sh updates

```bash
./scripts/version-to.sh 25.0.0
git diff
git commit -m "chore: bump version to v25.0.0"
git tag v25.0.0
git push origin main v25.0.0
```

The script updates:
- `Cargo.toml` (package version)
- `Cargo.lock` (root package version only, no dependency changes)
- `aur/cosmostrix-bin/PKGBUILD` (`pkgver=`, `_tag=`)
- `aur/cosmostrix-bin/.SRCINFO` (regenerated from PKGBUILD)
- `README.md` (active version examples)
- `docs/workflow/about-ci.md` (active version examples)

It skips changelog headings (e.g. `### v25.0.0`) to preserve historical release notes, and audits workflow files for hardcoded versions (workflows should derive versions dynamically from `GITHUB_REF_NAME`).

Verify the current version without making changes:

```bash
./scripts/version-to.sh --check 25.0.0
```

### CI fail-fast guard

CI runs `./scripts/build.sh version-sync` as a dedicated job (`Dragon - Version sync guard`) BEFORE any Rust build, so a desync fails the pipeline in seconds rather than after a full test job. The `scripts/check-version-anti-patterns.sh` guard also runs there to block re-introduction of hardcoded version assertions in `src/`. The compile-time test guard in `src/docs_tests/metadata.rs` provides a third layer of defense: it asserts that `Cargo.toml`, `PKGBUILD`, `.SRCINFO`, and the README install tag all agree with `env!("CARGO_PKG_VERSION")`.
