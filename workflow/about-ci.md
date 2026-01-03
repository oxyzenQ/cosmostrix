# GitHub Actions workflows

This repository uses GitHub Actions for CI and releases.

Workflow files live under:

- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `.github/workflows/gitbot-deps.yml`

## Overview

### CI (`.github/workflows/ci.yml`)

**Triggers**

- `push` to `main`
- `pull_request` targeting `main`

**What it does**

- **Security audit**: runs `cargo-audit` using `cargo +stable` to avoid MSRV breakage when `cargo-audit` bumps its required Rust version.
- **MSRV**: runs `cargo test --all` on Rust `1.81.0`.
- **Test + Build (debug)**: runs `cargo test --all` and `cargo build --profile dev`.
- **Format + Clippy**: runs `cargo fmt -- --check` and `cargo clippy ... -D warnings`.
- **Dependency policy**: installs `cargo-deny` and runs `cargo +stable deny check all`.

**Notes**

- This workflow is meant to keep `main` green and enforce formatting/lints/policy.

### Release (`.github/workflows/release.yml`)

**Triggers**

- `push` tag matching `v*` (recommended)

**Tag conventions (release channel)**

- `vX.Y.Z-alpha.N`, `vX.Y.Z-beta.N`, `vX.Y.Z-rc.N` => published as **prerelease** (not Latest)
- `vX.Y.Z-stable.N` => published as a **normal release** (eligible to become **Latest**)
- `vX.Y.Z` => published as a **normal release** (eligible to become **Latest**)

**What it builds**

- Linux x86_64 (v1/v2/v3/v4) (runner host build)
- Linux aarch64 native (runner host build)
- macOS aarch64 native (runner host build)
- Windows x86_64 (universal) (runner host build)
- Windows aarch64 native (runner host build)
- Android aarch64 native (cross-compiled): `aarch64-linux-android`

**Build steps (per platform job)**

- Tests: `cargo test --all --locked`
- Builds:
  - `cargo build --profile dev --locked`
  - `cargo build --profile release --locked`
- Checks:
  - `cargo fmt -- --check`
  - `cargo clippy --locked --all-targets --all-features -- -D warnings`
  - `cargo +stable deny check all`

**Packaging output**

Each build produces:

- `cosmostrix-bin-<tag>-<platform>.tar.gz`
- `cosmostrix-bin-<tag>-<platform>.tar.gz.sha512`
- `cosmostrix-bin-<tag>-<platform>.zip`
- `cosmostrix-bin-<tag>-<platform>.zip.sha512`

Where `<tag>` is a git tag like `v1.0.0`.

Where `<platform>` is one of:

- `linux-x86_64-v1`
- `linux-x86_64-v2`
- `linux-x86_64-v3`
- `linux-x86_64-v4`
- `linux-aarch64-native`
- `darwin-aarch64-native`
- `windows-x86_64`
- `windows-aarch64-native`
- `android-aarch64-native`

The archive contains:

- `cosmostrix` binary
- `README.md`
- `LICENSE`

**Checksums**

Checksum files are generated using:

- `sha512sum` when available, else
- `shasum -a 512`

Verification examples:

```bash
# Linux
sha512sum -c cosmostrix-bin-v1.0.0-linux-x86_64-v1.tar.gz.sha512

# macOS
shasum -a 512 -c cosmostrix-bin-v1.0.0-darwin-aarch64-native.tar.gz.sha512
```

**Release publishing**

The `publish_release` job:

- downloads all build artifacts
- generates release notes from git history (since previous `v*` tag)
- creates a GitHub Release and uploads all `*.tar.gz`, `*.tar.gz.sha512`, `*.zip`, and `*.zip.sha512` files

## Typical release flow

```bash
# 1) Update Cargo.toml version
# 2) Commit the version bump
git commit -am "release: 1.0.1-stable.1"

# 3) Create an annotated tag
git tag -a v1.0.1-stable.1 -m v1.0.1-stable.1

# 4) Push the tag (this triggers the Release workflow)
git push origin v1.0.1-stable.1
```

- The **Release** workflow will run on that tag.
- `-alpha.*` / `-beta.*` / `-rc.*` tags are marked as prerelease.
- `-stable.*` tags are treated as normal releases.

### Dependency update bot (`.github/workflows/gitbot-deps.yml`)

**Triggers**

- `schedule` daily at **01:00 UTC (08:00 WIB)**
- `workflow_dispatch` manual run

**Manual inputs**

- `strategy`: `direct` (commit to `main`) or `pr` (open a PR)
- `force_update`: continue even if tests fail

**What it does**

- Runs `cargo update`
- Runs `cargo +stable audit` and `cargo +stable deny check all`
- Runs `cargo fmt -- --check` and basic build/test/clippy on toolchain `1.81.0`

**Notes**

- GitHub cron uses UTC; adjust the schedule if you want a different local time.
