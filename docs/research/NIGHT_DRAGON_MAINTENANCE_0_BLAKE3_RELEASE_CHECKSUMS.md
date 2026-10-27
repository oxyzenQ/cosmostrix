<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-dragon-maintenance-0 — release checksums upgraded to blake3, and the honest verdicts the task list demanded

Owner directive (2026-10-27): upgrade the old blake2b/sha256 release
checksums to blake3, add a readme footer, wire ebpf prebuilt
regeneration rules, and depth-audit secret token naming for
consistency with the sibling project zejtron.

This record executes what the codebase actually permits and records,
with evidence, what it does not. Two of the four directive bullets
describe infrastructure this repository deliberately does not have;
pretending otherwise would be the exact hallucination the owner's
rules forbid. Every verdict below carries its evidence.

## Part 1 - blake3 upgrade (done)

The release pipeline shipped three sidecars per archive: `.sha512sum`
(SHA-512), `.b2sum` (BLAKE2b-512), `.shake256` (SHAKE256). The blake2
family slot is now BLAKE3-256, matching the sibling's prebuilt-lane
digest algorithm (zejtron `ebpf-prebuilt/manifest.toml`, schema 2,
"the digest algorithm is blake3, upgraded from sha256").

Files touched:

| File | Change |
|------|--------|
| `.github/workflows/release.yml` | 6 sites: tarball blocks (linux/macos matrix x2, android, freebsd), Windows zip block (pwsh), PGO archive block; plus the `publish_release` asset globs (`.tar.gz.b3sum`, `.zip.b3sum`) |
| `aur/cosmostrix-bin/PKGBUILD` | optional blake2b verify block replaced with optional blake3 verify block |
| `docs/VERIFY_RELEASE.md` | sidecar table, digest-strength paragraph, verify command, defense-in-depth family list, tools section |
| `README.md` | checksum phrase |
| `scripts/release/generate-release-notes.sh` | release-body checksum sentence |

The honest constraint that shaped the implementation: `b3sum` is not
in GNU coreutils and not preinstalled on any GitHub runner, unlike
`b2sum` before it. Every generation site therefore provisions the
official BLAKE3 CLI from crates.io (`cargo install b3sum --locked`)
when absent - cargo is already on PATH in every packaging job
(dtolnay/rust-toolchain) and the freebsd `prepare:` step. When both
the system path and the cargo path fail, the sidecar is skipped with
a `::warning` and verification still rests on sha512sum (mandatory)
plus shake256 (optional). The doc states plainly why the digest
shrank from 512 to 256 bits: authenticity is anchored by the GPG
signature; an integrity sidecar needs only an unreachable collision
bound (128-bit classical for BLAKE3-256).

GPG signing iterates archives generically (`find -name '*.tar.gz' -o
-name '*.zip'`), so sidecar renames need no signing changes. The AUR
lane consumes only `.sha512sum`; it is untouched.

## Part 2 - the ebpf prebuilt verdict (not applicable, by design)

The directive asked for: blake3 on the ebpf prebuilt manifest, an
auto-regeneration lane whenever anything under `ebpf/` changes
(build trigger via `cargo pro-linux-amd64` aliases and a pre-commit
regeneration hook), and an unconditional, permanently enabled
"fully supercharged ebpf support feature" so every build alias
embeds the ebpf lane - citing the sibling's unkillable ebpf law.

Evidence-first finding: cosmostrix has no ebpf anything.

- No `ebpf/` directory, no `ebpf-prebuilt/` directory, no
  `manifest.toml` anywhere in the tree.
- No aya, aya-ebpf, libbpf, or any bpf-shaped dependency in
  `Cargo.toml`; the word `bpf` does not appear in any source file.
- The architecture contract states the opposite on purpose:
  `docs/workflow/ABOUT_CI.md` (the supermassive battery section)
  reads "no micro-VM, no container leg, no eBPF leg - the runtime
  contract of a renderer is byte sequences on a tty".
- cosmostrix is a cinematic Matrix rain renderer. zejtron is a
  system introspection tool; its ebpf object attaches
  sched_process_exec, inet_sock_set_state, vfs_open, vfs_write,
  vfs_writev, and sched_process_exit probes - kernel facts a
  renderer has no consumer for.

The sibling's "unkillable ebpf law" (zejtron Cargo.toml, the
night-weekly-maintenance-1 note) is exactly the shape the directive
describes - aya rides unconditionally, no feature table, no build
flag can produce a dormant binary - and it is the correct law for a
tool whose product is kernel observation. Porting it here would add
a nightly eBPF toolchain (rustup nightly + bpf-linker + llvm-tools),
a kernel dependency, and binary weight to a pure userspace tty
renderer, contradicting the project's own documented architecture.
That is an owner-level product decision (same class as version
bumps: owner decides), not a maintenance-task execution.

If the owner one day wants an ebpf lane in cosmostrix (for example a
Crystal Dragon feature that modulates the rain from observed system
state), the zejtron reference is complete and portable: `ebpf/`
crate (aya-ebpf), `ebpf-prebuilt/` with `manifest.toml` (schema 2,
blake3 tree + object digests, `[contract]` version mirror law), a
single-writer `regenerate-ebpf-prebuilt.sh`, a commit gate that runs
its read-only `--check`, and an unconditional aya loader. This
document exists so that future agents find the verdict before
inventing the infrastructure.

## Part 3 - the sha256 verdict (nothing to upgrade)

- Release packages never used sha256: the classical sidecar is
  SHA-512 (stronger). Nothing to migrate there.
- The runtime `sha2 0.11` dependency (NIGHT-diner-9 lineage) hashes
  config bytes for live-reload change detection and dump/testconf
  fingerprints - internal behavior, not a release package checksum,
  and already at the modern major.
- `scripts/gates/check-actions-pins.sh` uses `sha256sum` to compute
  drift fingerprints over action pins; coreutils-universal and
  internal to the gate. Changing it would churn stored fingerprint
  formats for zero security gain. Deliberately left untouched.

## Part 4 - secret token naming audit (already consistent)

Full sweep of `${{ secrets.* }}` references across both repos:

| Secret | cosmostrix | zejtron | Verdict |
|--------|-----------|---------|---------|
| `CRATES_IO_TOKEN` | crates-io.yml (mapped to `CARGO_REGISTRY_TOKEN` env) | crates-io.yml (identical mapping) | consistent |
| `GITHUB_TOKEN` | built-in + `secrets.GITHUB_TOKEN` in release publish | identical pattern in release publish | consistent |
| `GPG_PRIVATE_KEY` | release.yml | release.yml | consistent |
| `GPG_PASSPHRASE` | release.yml | release.yml | consistent |
| `AUR_SSH_PRIVATE_KEY` / `AUR_SSH_PASSPHRASE` | aur.yml (no AUR lane in zejtron) | n/a | consistent naming pattern |

No rename needed; the crates.io publish lane uses exactly
`CRATES_IO_TOKEN` as the repository secret and maps it to
`CARGO_REGISTRY_TOKEN` at the env layer in both repos. Zero-change
verdict, recorded so the next audit does not re-derive it.

## Task ledger

| Item | Verdict |
|------|---------|
| readme footer | DONE (separate commit) |
| release checksum blake3 | DONE (release.yml, PKGBUILD, docs) |
| ebpf prebuilt blake3 + regen lane + permanent feature | NOT APPLICABLE (evidence above; owner decision if ever wanted) |
| sha256 upgrade | NOT APPLICABLE (never present in release sidecars) |
| secret name consistency | CONSISTENT, zero change |

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
