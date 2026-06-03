# Supply Chain Security

This document describes the policies, tooling, and procedures that govern the integrity
of the Cosmostrix software supply chain — from dependency selection through release
distribution.

---

## 1. Dependency Policy

Every dependency that ships with Cosmostrix must be auditable. The CI pipeline enforces
this contract by running `cargo audit` (known CVE scanning) and `cargo deny check all`
(license, source, and duplicate-policy enforcement) on every pull request and release
build. No dependency enters the lockfile without passing both gates.

### Direct Dependencies

| Crate | Version Constraint | Purpose | Last Audit Date |
|---|---|---|---|
| `clap` | `>=4.5, <4.6` (derive feature) | CLI argument parsing via derive macros | _YYYY-MM-DD_ |
| `crossterm` | `0.29` | Cross-platform terminal manipulation (colors, cursor, events) | _YYYY-MM-DD_ |
| `rand` | `0.9` | Cryptographic-quality random number generation for rain columns | _YYYY-MM-DD_ |
| `bitvec` | `1` | Compact bit-vector storage for per-column state tracking | _YYYY-MM-DD_ |
| `smallvec` | `1` | Stack-allocated small vectors — avoids heap allocation in hot paths | _YYYY-MM-DD_ |
| `unicode-width` | `0.2` | Correct character width calculation for CJK and wide glyphs | _YYYY-MM-DD_ |
| `signal-hook` | `0.3` (Unix only) | Graceful shutdown on SIGINT/SIGTERM via POSIX signal handlers | _YYYY-MM-DD_ |
| `libc` | `0.2` (Linux only) | Low-level syscall bindings for terminal size queries on Linux | _YYYY-MM-DD_ |
| `ctrlc` | `3.4` (Windows only) | Graceful shutdown handler for Windows console Ctrl-C/SIGBREAK events | _YYYY-MM-DD_ |

### Policy Rules

1. **No new dependencies without explicit justification.** Any pull request that
   introduces a new crate — whether direct or transitive — must include a
   description of why the crate is necessary, what alternatives were evaluated, and
   why they were rejected. This justification must appear in the PR description.

2. **License compliance.** The `deny.toml` allow-list permits only OSI-approved and
   widely-adopted licenses: Apache-2.0, MIT, BSD-2-Clause, BSD-3-Clause, ISC,
   Unicode-3.0, Zlib, Unicode-DFS-2016, MPL-2.0, and CC0-1.0. Any dependency
   carrying a license outside this set will cause CI to fail.

3. **Source restriction.** All crates must originate from the official crates.io
   registry (`https://github.com/rust-lang/crates.io-index`). Git dependencies
   and unknown registries are denied at the CI level via `cargo deny` source
   checks.

4. **Lockfile discipline.** `Cargo.lock` is committed to version control. All CI
   builds use `--locked` to guarantee that the exact dependency versions verified
   in the lockfile are used at compile time, preventing supply-chain drift between
   audit and build.

---

## 2. Release Verification

### SHA-512 Sidecar Checksums

Every release binary published to GitHub Releases is accompanied by a `.sha512`
sidecar file. These checksums are generated during the release workflow
(`release.yml`) immediately after the tarball or zip archive is created, using
the host system's `sha512sum` or `shasum -a 512` utility.

The model is straightforward and deterministic:

1. **Build** — The binary is compiled with the appropriate profile
   (`pro-linux-vN`, `pro-macos-aarch64-native`, `pro-win-x86_64`, etc.) using
   fat LTO, a single codegen unit, and `strip = true` for a minimal, reproducible
   artifact.
2. **Package** — The binary, `LICENSE`, and `README.md` are placed into a flat
   archive (`cosmostrix-bin-vX.Y.Z-<platform>.tar.gz` or `.zip`). The archive
   layout is intentionally flat (no directory nesting) to maintain compatibility
   with the AUR PKGBUILD `prepare()` function.
3. **Hash** — The archive file is hashed with SHA-512. The hex digest is written
   to a same-named `.sha512` file in the format `<digest>  <filename>`, uploaded
   alongside the archive as a release asset.

Users can verify any downloaded artifact with a single command:

```bash
sha512sum --check cosmostrix-bin-v2.1.0-linux-x86_64-v1.tar.gz.sha512
```

### AUR Package Verification

The AUR package (`cosmostrix-bin`) is published automatically via the `aur.yml`
workflow, which triggers after a successful GitHub Release. The PKGBUILD uses
`sha512sums` verification to ensure the archive downloaded from GitHub Releases
matches the expected hash. The `prepare()` function in the PKGBUILD dynamically
selects the correct platform asset and verifies its SHA-512 checksum before
extracting.

The AUR sync workflow (`aur.yml`) is:
- **Deterministic** — the same tag always produces identical `pkgver`, `_tag`, and
  `.SRCINFO` output.
- **Idempotent** — re-running for an already-published tag is a no-op (detected
  via `git diff --quiet`).
- **Retry-safe** — transient SSH failures to `aur.archlinux.org` are retried up
  to three times with a ten-second backoff delay.

The AUR host key is pinned in the workflow to a known Ed25519 fingerprint,
preventing MITM attacks during the SSH push.

### Binary Reproducibility Goals

While full deterministic reproduction (bit-for-bit identical binaries across
different build environments) is a long-term goal, the current release process
already enforces several reproducibility-friendly properties:

- **Fixed toolchain version** — all release builds use `rustc 1.81.0`, pinned in
  every workflow via `dtolnay/rust-toolchain`.
- **Profile standardization** — `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`,
  `panic = "unwind"`, `strip = true`, and `incremental = false` ensure consistent
  codegen output across builds.
- **`--locked` flag** — guarantees that the exact dependency tree from
  `Cargo.lock` is used, preventing variation from registry drift.
- **Embedded build metadata verification** — each release build self-reports its
  variant, LTO mode, panic strategy, and strip status via the `cosmostrix -i`
  diagnostic flag, which the CI workflow asserts against expected values.

Future work includes investigating `cargo +nightly -Z build-std` for fully
deterministic standard library builds and comparing `sha512sum` digests across
independent build machines.

---

## 3. GitHub Actions Hardening

### Pin All Action References to SHA Commits

Every third-party action used in CI and release workflows is referenced by its
full SHA commit hash rather than a mutable tag. Tags can be reassigned by
repository owners without notice, which would allow an attacker to inject
arbitrary code into a workflow run by pushing a new commit to a previously-used
tag.

**Current status and required migration:** At the time of writing, several workflows
reference actions by version tag (e.g., `actions/checkout@v6.0.2`,
`dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2.9.1`). While these
are semver tags from trusted maintainers, the hardened posture requires pinning
each to an immutable SHA. A pending migration will replace every `@tag` reference
with `@<sha-commit>` across all five workflow files (`ci.yml`, `release.yml`,
`gitbot-deps.yml`, `gitbot-audit.yml`, `aur.yml`). Each pin will be accompanied
by a comment noting the tag and date for maintainability.

### Minimal Permissions

GitHub Actions permissions follow the principle of least privilege:

| Workflow | `permissions` | Rationale |
|---|---|---|
| `ci.yml` | `contents: read` | Builds and audits only — no write access needed |
| `gitbot-audit.yml` | `contents: read`, `actions: read` | Observation-only security scan |
| `aur.yml` | `contents: read` | No write to this repo; SSH key handles AUR push |
| `release.yml` | `contents: write`, `actions: write` | Creates GitHub Releases; write is necessary |
| `gitbot-deps.yml` | `contents: write` | Commits validated lockfile updates directly to `main` |

No workflow requests `attestations: write` at present. If binary attestation via
GitHub's Sigstore integration is adopted in the future, that permission will be
scoped exclusively to the `release.yml` workflow and pinned to a single job.

### Branch Protection

The CI workflow (`ci.yml`) triggers on both `push` to `main` and `pull_request`
against `main`. However, the `gitbot-deps.yml` automated dependency update workflow
pushes directly to `main` when scheduled weekly. This is acceptable because:

- The automated commit only modifies `Cargo.lock` — no source code changes.
- The commit is preceded by a full validation pipeline: `cargo audit`,
  `cargo deny check all`, `cargo fmt --check`, `cargo build`, `cargo test`,
  and `cargo clippy`.
- Failed validation stops the workflow before any commit is pushed.

For all human-authored changes, the recommendation is to enable GitHub branch
protection rules on `main` that require at least one approving review before
merge, ensuring that no unreviewed code reaches the default branch.

---

## 4. Toolchain Requirements

### `cargo audit` — Known Vulnerability Scanning

`cargo audit` (maintained by the Rust Security Advisory working group) scans the
project's dependency tree against the RustSec Advisory Database. It checks for
known CVEs, advisory withdrawals, and unmaintained crates. The tool is installed
via `taiki-e/install-action` in the CI pipeline and runs as the first gate in the
`security` job.

The daily `gitbot-audit.yml` workflow runs `cargo audit` at 00:00 UTC every day
as an observation-only check. If vulnerabilities are detected, the weekly
`gitbot-deps.yml` workflow will resolve them during its next scheduled run
(Saturday 23:00 UTC) by performing `cargo update`, re-auditing, and pushing the
updated lockfile.

### `cargo deny` — Policy Enforcement

`cargo deny` enforces organizational policies across four dimensions, configured
in the project's `deny.toml` file:

- **Advisories** — cross-references the same RustSec database as `cargo audit`
  with zero ignored advisories, ensuring no known vulnerability is suppressed.
- **Licenses** — permits only the license set enumerated in Section 1 above.
  A confidence threshold of 0.8 ensures that license detection is reliable before
  blocking a build.
- **Bans** — warns on duplicate crate versions (common in large dependency
  trees) to encourage deduplication over time. Wildcard dependencies are allowed
  to support legitimate use cases.
- **Sources** — restricts all crate origins to the official crates.io registry.
  Git sources and unknown registries produce warnings.

Both tools are run in CI (`ci.yml` deny job, `release.yml` audit+deny during the
checks matrix) and in the automated dependency update workflow (`gitbot-deps.yml`).

### `rustup` — Minimum Supported Rust Version (MSRV)

The project's MSRV is **Rust 1.81.0**, declared in `Cargo.toml` as
`rust-version = "1.81"` and enforced in every CI workflow via the
`dtolnay/rust-toolchain` action with `toolchain: 1.81.0`. The CI pipeline
includes a dedicated MSRV job that compiles and tests the project under this
exact toolchain version, ensuring compatibility is not accidentally broken by
newer Rust features.

Developers must use `rustup` to install and manage the toolchain:

```bash
rustup install 1.81.0
rustup default 1.81.0
```

---

## 5. Update Process

### Routine Dependency Updates

The project uses a weekly automated update cycle powered by the T-800 bot
(`gitbot-deps.yml`):

1. **`cargo update --workspace`** — bumps all dependencies in `Cargo.lock` to the
   latest compatible versions according to `Cargo.toml` version constraints.
2. **No-op detection** — if `Cargo.lock` is unchanged, the pipeline exits early
   to avoid unnecessary CI resource consumption.
3. **`cargo audit`** — verifies that the updated dependency tree introduces no
   known vulnerabilities. A baseline pre-update audit is also performed to detect
   whether the update itself resolves existing advisories.
4. **`cargo deny check all`** — confirms continued compliance with license,
   source, and duplicate-version policies.
5. **`cargo fmt --all -- --check`** — ensures no formatting drift was introduced
   by any dependency update affecting macro expansion.
6. **`cargo build`** (dev, release, and `pro-linux-v1` profiles) — validates that
   the project compiles cleanly with the updated crates.
7. **`cargo test --all --locked`** — runs the full test suite to catch any
   behavioral regressions.
8. **`cargo clippy --locked --all-targets --all-features -- -D warnings`** — lints
   for correctness and style issues introduced by the update.
9. **Commit and push** — if all checks pass, the updated `Cargo.lock` is committed
   with a descriptive message and pushed directly to `main`.

### Security Advisory Response

When `cargo audit` or the daily `gitbot-audit.yml` workflow detects a
vulnerability, the response depends on severity:

| Severity | Response Time | Action |
|---|---|---|
| **Critical** (RustSec CVSS >= 9.0) | Immediate | Emergency `cargo update` targeting the affected crate, full CI validation, and direct push to `main`. A new patch release is published if the main branch is clean. |
| **High** (CVSS 7.0–8.9) | Within 24 hours | The next scheduled `gitbot-deps.yml` run will resolve it automatically. If it runs before the weekly window, maintainers can trigger it manually via `workflow_dispatch`. |
| **Medium** (CVSS 4.0–6.9) | Next release cycle | The vulnerability is addressed during the next regular dependency update cycle (weekly) or the next feature release, whichever comes first. |
| **Low** (CVSS < 4.0) | Next minor release | Low-severity advisories are tracked and resolved at the project's discretion during normal maintenance. |

For any advisory that cannot be resolved by a simple `cargo update` (e.g., no
patched version is available), the project will evaluate whether the affected
functionality can be disabled, the dependency can be replaced, or a temporary
RustSec ignore entry (documented with rationale and a deadline for removal) must
be added to `deny.toml`.

---

*Last updated: Cosmostrix 2.1.0*
