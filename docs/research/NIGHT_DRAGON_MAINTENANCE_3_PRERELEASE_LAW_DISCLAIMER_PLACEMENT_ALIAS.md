<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Night Dragon Maintenance 3 — Prerelease Law, Disclaimer Placement, Alias Law, eBPF Depth Audit

Owner: oxyzenQ | Session: night-dragon-maintenance-3 | Date: 2026-10-27
Reference: github.com/oxyzenQ/zejtron (mature infra lineage)

## Scope

Five directives, one commit each, pushed in order:

| # | Directive | Commit | Result |
|---|-----------|--------|--------|
| 1 | Remove `install.sh` | `c0feddb` | Removed; all live references updated |
| 2 | Hardcoding-free prerelease detection | `f651d7c` | One law: hyphen in version = pre-release |
| 3 | Fix the disclaimer injector | `d51aa6e` | Placement law ported; 135 files normalized |
| 4 | No basic `cargo release` in scripts/tests | `3932092` | Alias dispatch + alias-aware discovery |
| 5 | eBPF depth audit | this doc | N/A by architecture — evidence below |

## 1. install.sh removal

`scripts/package/install.sh` (395 lines) removed. The from-source install
path is `scripts/build.sh release`, which now auto-detects the host CPU
level and dispatches to the matching named alias (directive 4). The
install story becomes: GitHub Releases prebuilt tarballs (BLAKE3
verified), `cargo install cosmostrix --locked`, or the alias dispatch +
manual copy. `uninstall.sh` stays: it never referenced install.sh and
serves prebuilt-tarball installs. `docs/archive/CHANGELOG-V100.0.6-ERA.md`
keeps its historical mention — frozen era records are never rewritten.

## 2. Prerelease law

Every release surface now classifies with a single rule: the first
hyphen after the version makes the tag a pre-release. No channel
enumeration survives. Evidence of the dead weight removed: the
release.yml classifier carried `alpha.*|beta.*|rc.*|pre.*|nightly.*`
arms whose catch-all already classified every suffix as pre-release —
pure maintenance surface. Hardening wins from the unification:

- `aur.yml`'s old suffix regex `-[a-zA-Z]+\.[0-9]+$` silently missed
  suffix shapes it was never tested against; the hyphen law rejects
  every suffixed tag with one clear message.
- `version-to.sh` rejected legal suffixes like `example.1` (its regex
  demanded an enumerated channel); now any `X.Y.Z-<suffix>` bumps.
- Tag validation regex widened to `vX.Y.Z(-[A-Za-z0-9.]+)?` — the
  owner's `v1.0.0-example.1` example now validates end to end.

Verified with a 28-case harness mirroring the exact deployed code
shapes (stable stays stable, every suffix classifies as pre-release,
`-stable.N` still dies, bare hyphen rejected).

## 3. Disclaimer placement law

The injector read marker PRESENCE as the done verdict, so any content
appended after the block left it mid-file forever. Measured defect at
audit time: `docs/LIVE_RELOAD_BEHAVIOR.md` carried the block 435 lines
above its real bottom, five A/B reports carried it above the
verification footer, eleven docs/research files carried truncated
marker-only blocks. Ported from zejtron's night-dragon-2S injector:
PLACEMENT is the verdict — normalize() strips every house-marker block
from wherever it sits and re-appends the canonical block as the file's
final content. Idempotency proven on the full tree (run 2: 0 changed).
Frozen records (CHANGELOG.md, docs/archive/**) and generated sweep
reports stay excluded per the zejtron law. One byte-level find: the old
injector's `read -d ''` IFS-trimmed the heredoc's leading newline, so
135 of 152 living files lacked the canonical blank separator above the
block — the normalize run restored it everywhere.

## 4. Alias law

`build.sh release` no longer runs a bare `cargo build --release`. It
dispatches through `pick_release_alias`: AVX-512 ->
`pro-linux-amd64-v4-gnu`, AVX2 -> `pro-linux-amd64-v3-gnu`,
Apple Silicon -> `pro-macos-aarch64-native`, FreeBSD ->
`pro-freebsd-amd64`, everything else -> `pro-native`. RUSTFLAGS compose
at step level (CPU pin + hardened flags — the exact release.yml
composition) so the env deterministically wins over the alias
`build.rustflags` config; an ambient RUSTFLAGS would otherwise silently
drop the target-cpu pin (the env-shadow bug class supermassive-kernel.yml
fixed). The depth/bench harness family now probes the alias output
paths first, so a post-dispatch binary is found without env overrides.
`maintenance.yml` builds `cargo pro-linux-amd64-v3-gnu --locked` for
dependency validation; the retired 2026-08-23 plain-pro note is
superseded by the 2026-10-27 directive. pgo.sh's plain release baseline
lane is intentionally untouched: it is the benchmark control group, not
a shipped build path.

## 5. eBPF depth audit — verdict: NOT APPLICABLE (skip, do not add)

The directive: make sure the project is supercharged eBPF; if already
peak, skip — no over-engineering. Evidence gathered fresh this session:

| Surface | Scan | Result |
|---------|------|--------|
| `Cargo.toml` / `Cargo.lock` | aya / libbpf / ebpf tokens | 0 matches |
| `src/` + `test/` (69k+ LOC) | bpf token scan | 0 matches |
| `[features]` section | feature surface | does not exist — nothing to pin on |
| `build.rs` | codegen / artifact writes | metadata only (git SHA, rustc version, profile); no generated prebuilt artifacts, no checksum manifest |
| `ebpf/` / `ebpf-prebuilt/` | directories | do not exist |

Two in-repo authorities already settled the question:

1. The architecture contract (`docs/workflow/ABOUT_CI.md`): the
   supermassive battery has "no eBPF leg — the runtime contract of a
   renderer is byte sequences on a tty".
2. The historical exploration record (`docs/archive/cosmic_dragon/
   FINDINGS.md`): "No amount of SIMD, eBPF, or C supercharger can fix
   that — the terminal is a separate process cosmostrix cannot
   control." The render bottleneck lives in the terminal emulator's
   ANSI parser, outside the process eBPF could reach.

Verdict: adding an eBPF leg to a pure-userspace tty renderer would be
over-engineering against the project's own contract and its own
recorded findings — the skip the directive itself orders. The
"permanent supercharged" intent is already satisfied by the build
surface that does exist: the pro alias family (v3/v4 gnu+musl, macOS,
FreeBSD, Android, PGO nitro) is default-on for every alias build with
no feature gate to disable. The zejtron unkillable-eBPF law (unconditional
aya, ebpf-prebuilt manifest + blake3 regeneration on any `ebpf/` edit)
remains the portable pattern to port if cosmostrix ever grows a real
kernel-side leg — recorded here so the port is a lookup, not a re-derivation.
No regenerate hook was wired: there are no generated artifacts to go
stale, and inventing a manifest for none would be exactly the burden
the owner's rule forbids.

## Verification

No cargo in the sandbox — light gates ran instead of `build.sh
check-all` (per the standing fallback): shellcheck + bash -n on every
touched shell file, py_compile on every touched harness, yamllint on
every touched workflow, emoji audit (0), language audit (0 findings),
SPDX headers (837 files OK), file modes preserved (injector stays
755). The prerelease harness (28/28) and the stubbed-cargo dispatch
dry run prove the two logic-bearing changes end to end. Benchmark A/B
skipped honestly: zero `.rs` runtime changes this session (one doc
comment in `root_guard.rs` only) — no render-loop surface touched, so
a 10s density/entropy/fps A/B would measure nothing but noise.

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
