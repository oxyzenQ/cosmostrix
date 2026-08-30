# Supply Chain Security
<!-- SPDX-License-Identifier: GPL-3.0-only -->

Policies, tooling, and procedures that govern the integrity of the cosmostrix software supply chain — from dependency selection through release distribution.

## 1. Dependency Policy

Every dependency that ships with cosmostrix must be auditable. The CI pipeline enforces this contract by running `cargo audit` (known CVE scanning) and `cargo deny check all` (license, source, and duplicate-policy enforcement) on every pull request and release build. No dependency enters the lockfile without passing both gates.

### Direct Dependencies

| Crate | Version Constraint | Purpose | Network? | Crypto? |
|---|---|---|---|---|
| `clap` | `>=4.5, <4.6` (derive feature) | CLI argument parsing via derive macros | No | No |
| `crossterm` | `0.29` | Cross-platform terminal manipulation (colors, cursor, events) | No | No |
| `rand` | `0.9` | Cryptographic-quality RNG for rain columns | No | No |
| `bitvec` | `1` | Compact bit-vector storage for per-column state | No | No |
| `smallvec` | `1` | Stack-allocated small vectors — avoids heap allocation in hot paths | No | No |
| `unicode-width` | `0.2` | Correct character width calculation for CJK and wide glyphs | No | No |
| `signal-hook` | `0.3` (Unix only) | Graceful shutdown on SIGTERM/SIGHUP/SIGQUIT (v25.13: SIGINT deprecated) | No | No |
| `libc` | `0.2` (Linux only) | Low-level syscall bindings for terminal size queries | No | No |
| `ctrlc` | `3.4` (Windows only) | Graceful shutdown handler for Windows console Ctrl-C/SIGBREAK | No | No |
| `notify` | `>=6.1, <7` (default-features = false) | Cross-platform filesystem watcher for live config reload | No | No |
| `sha2` | `0.10` | SHA-512 for config.toml hashing (live-reload change detection, dump/testconf fingerprints) | No | No |

### Policy Rules

1. **No new dependencies without explicit justification.** Any PR that introduces a new crate (direct or transitive) must describe why the crate is necessary, what alternatives were evaluated, and why they were rejected. Justification must appear in the PR description.
2. **License compliance.** `deny.toml` allow-list permits only OSI-approved and widely-adopted licenses: Apache-2.0, MIT, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib, Unicode-DFS-2016, MPL-2.0, CC0-1.0. Any dependency carrying a license outside this set will cause CI to fail.
3. **Source restriction.** All crates must originate from the official crates.io registry. Git dependencies and unknown registries are denied at the CI level via `cargo deny` source checks.
4. **Lockfile discipline.** `Cargo.lock` is committed. All CI builds use `--locked` to guarantee exact dependency versions verified in the lockfile are used at compile time, preventing supply-chain drift between audit and build.

## 2. Release Verification

### SHA-512 Sidecar Checksums

Every release binary published to GitHub Releases is accompanied by a `.sha512sum` sidecar file, generated during the release workflow immediately after the tarball or zip archive is created, using `sha512sum` or `shasum -a 512`. The model is straightforward and deterministic: (1) **Build** — binary compiled with the appropriate profile using fat LTO, single codegen unit, `strip = true`. (2) **Package** — binary + `LICENSE` + `README.md` placed into a flat archive (`cosmostrix-vX.Y.Z-<platform>.tar.gz` or `.zip`); flat layout maintains compatibility with the AUR PKGBUILD `prepare()` function. (3) **Hash** — archive hashed with SHA-512, hex digest written to a same-named `.sha512sum` file in the format `<digest>  <filename>`, uploaded alongside the archive as a release asset.

Verify any downloaded artifact:

```bash
sha512sum --check cosmostrix-vX.Y.Z-linux-amd64-v3.tar.gz.sha512sum
```

### AUR Package Verification

The AUR package (`cosmostrix-bin`) is published automatically via `aur.yml`, triggered after a successful GitHub Release. PKGBUILD uses `sha512sums` verification to ensure the archive downloaded from GitHub Releases matches the expected hash; `prepare()` dynamically selects the correct platform asset and verifies its SHA-512 checksum before extracting. The AUR sync workflow is **deterministic** (same tag -> identical `pkgver`, `_tag`, `.SRCINFO`), **idempotent** (re-running for an already-published tag is a no-op, detected via `git diff --quiet`), and **retry-safe** (transient SSH failures to `aur.archlinux.org` retried up to 3 times with 10-second backoff). AUR host key pinned to a known Ed25519 fingerprint, preventing MITM attacks during the SSH push.

### Binary Reproducibility Goals

Full deterministic reproduction (bit-for-bit identical binaries across different build environments) is a long-term goal. Current release process already enforces: fixed toolchain version (rustc 1.98.0, pinned in `rust-toolchain.toml` and every workflow via `dtolnay/rust-toolchain`); profile standardization (`opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `panic = "unwind"`, `strip = true`, `incremental = false`); `--locked` flag (guarantees exact dependency tree from `Cargo.lock`); embedded build metadata verification (each release build self-reports variant, LTO mode, panic strategy, strip status via `cosmostrix --doctor`, which CI asserts against expected values). Future work: `cargo +nightly -Z build-std` for fully deterministic standard library builds and `sha512sum` digest comparison across independent build machines.

## 3. GitHub Actions Hardening

**Action References Float on Major Tags (owner decision 2026-08-30)**: every third-party action in CI and release workflows is referenced by its major version tag (`actions/checkout@v6`, `Swatinem/rust-cache@v2`, `dtolnay/rust-toolchain@stable`), and CI-installed deps (shfmt, codespell, ruff, cargo-audit, cargo-deny, Android NDK, ...) resolve their latest upstream release at run time. This "dynamic latest, minimal maintenance, boring but strong" policy deliberately replaced the earlier SHA-pinning migration plan: exact-pinning every action and tool multiplies maintenance work (dozens of pins to review and bump forever) while the remaining attack surface — a maintainer moving a major tag — is a low-likelihood event already mitigated by the controls below. Compensating controls: all workflows run with minimal permissions (`contents: read` everywhere except release/maintenance writers), `release.yml`/`crates-io.yml` only trigger on owner-pushed `v*` tags, CodeQL scans every push, and `cargo audit` + `cargo deny` run daily. If a higher-assurance posture is ever needed, re-introduce SHA pins via Dependabot automation (which removes the manual bump burden) rather than hand-maintained hashes. See `docs/workflow/ABOUT_CI.md` (Dependency version policy).

**Minimal Permissions**: `ci.yml` `contents: read` (builds and audits only); `gitbot-audit.yml` `contents: read`, `actions: read` (observation-only); `aur.yml` `contents: read` (no write to this repo; SSH key handles AUR push); `release.yml` `contents: write`, `actions: write` (creates GitHub Releases; write is necessary); `maintenance.yml` `contents: write` (commits validated lockfile updates directly to `main`). No workflow requests `attestations: write`. If binary attestation via GitHub's Sigstore integration is adopted in the future, that permission will be scoped exclusively to `release.yml` and pinned to a single job.

**Branch Protection**: `ci.yml` triggers on both `push` to `main` and `pull_request` against `main`. `maintenance.yml` pushes directly to `main` when scheduled weekly — acceptable because the automated commit only modifies `Cargo.lock`, is preceded by a full validation pipeline (audit + deny + fmt + build + test + clippy), and failed validation stops the workflow before any commit. For all human-authored changes, enable GitHub branch protection rules on `main` that require at least one approving review before merge.

## 4. Toolchain Requirements

**`cargo audit`** (Rust Security Advisory working group): scans the project's dependency tree against the RustSec Advisory Database for known CVEs, advisory withdrawals, unmaintained crates. Installed via `taiki-e/install-action` in CI; runs as the first gate in the `security` job. Daily `gitbot-audit.yml` workflow runs `cargo audit` at 00:00 UTC as observation-only; if vulnerabilities detected, weekly `maintenance.yml` resolves them during next scheduled run (Monday 00:00 UTC) by `cargo update`, re-auditing, pushing updated lockfile.

**`cargo deny`** enforces organizational policies across four dimensions, configured in `deny.toml`: advisories (cross-references RustSec, zero ignored); licenses (permits only the license set in §1, confidence threshold 0.8); bans (warns on duplicate crate versions, wildcard deps allowed for legitimate use); sources (restricts all crate origins to official crates.io registry, Git sources and unknown registries produce warnings). Both tools run in CI (`ci.yml` deny job, `release.yml` audit+deny) and in `maintenance.yml`.

**MSRV**: Rust 1.98.0, declared in `Cargo.toml` as `rust-version = "1.98"`, pinned in `rust-toolchain.toml` (`channel = "1.98.0"`), enforced in every CI workflow via `dtolnay/rust-toolchain` action with `toolchain: 1.98.0`. CI includes a dedicated MSRV job that compiles and tests under this exact version. Developers: `rustup install 1.98.0 && rustup default 1.98.0`.

## 5. Update Process

**Routine Dependency Updates** (weekly `maintenance.yml`): (1) `cargo update --workspace` bumps all deps in `Cargo.lock` to latest compatible versions; (2) no-op detection — if `Cargo.lock` unchanged, exit early; (3) `cargo audit` verifies no known vulnerabilities; (4) `cargo deny check all` confirms continued policy compliance; (5) `cargo fmt --all -- --check`; (6) `cargo build` (dev, release, pro-linux-v3 profiles); (7) `cargo test --all --locked`; (8) `cargo clippy --locked --all-targets --all-features -- -D warnings`; (9) commit and push to `main` if all checks pass.

**Security Advisory Response** (when `cargo audit` or daily `gitbot-audit.yml` detects a vulnerability):

| Severity | Response Time | Action |
|---|---|---|
| **Critical** (CVSS >= 9.0) | Immediate | Emergency `cargo update` targeting the affected crate, full CI validation, direct push to `main`. New patch release if main is clean. |
| **High** (CVSS 7.0–8.9) | Within 24 hours | Next scheduled `maintenance.yml` run resolves automatically. Maintainers can trigger manually via `workflow_dispatch`. |
| **Medium** (CVSS 4.0–6.9) | Next release cycle | Addressed during next regular dep update cycle (weekly) or next feature release. |
| **Low** (CVSS < 4.0) | Next minor release | Tracked and resolved at project's discretion during normal maintenance. |

For advisories that cannot be resolved by a simple `cargo update` (e.g., no patched version available), the project will evaluate whether the affected functionality can be disabled, the dependency replaced, or a temporary RustSec ignore entry (documented with rationale and a deadline for removal) added to `deny.toml`.
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
