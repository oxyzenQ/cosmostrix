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
| `miri.yml` | weekly cron (Sun 03:00 UTC) | Undefined behavior detection |
| `docs-ci.yml` | push | Verify doc links + code blocks |

## Release channels (tag conventions)

- `vX.Y.Z-alpha.N` / `vX.Y.Z-beta.N` / `vX.Y.Z-rc.N` → GitHub **prerelease**
- `vX.Y.Z` → GitHub **normal release** (eligible for Latest)

## Build matrix (release.yml)

Linux x86_64 (`v3`, `v4`, `musl`), Linux aarch64, macOS aarch64, Windows x86_64, Windows aarch64, Android aarch64 (cross-compiled).

Linux x86_64 baselines: `v3` = `-C target-cpu=x86-64-v3`, `v4` = `-C target-cpu=x86-64-v4`, `musl` = `x86-64-v3` static. v1/v2 were dropped in v10.0.0 (modern CPUs support v3).

## Packaging output

Each build produces `cosmostrix-bin-<tag>-<platform>.tar.gz` + `.sha512sum` (+ `.zip` + `.sha512sum` for Windows). Archive contains `cosmostrix` binary, `README.md`, `LICENSE`.

Verify a download:

```bash
sha512sum -c cosmostrix-bin-vX.Y.Z-linux-amd64-v3.tar.gz.sha512sum
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

Note: `version-to.sh` accepts stable SemVer only (`X.Y.Z`). Pre-release versions (`-alpha.N` / `-beta.N` / `-rc.N`) must be set manually in `Cargo.toml` and `Cargo.lock` — the script will reject them.

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
