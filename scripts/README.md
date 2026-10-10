<!-- SPDX-License-Identifier: GPL-3.0-only -->

# scripts/ Layout

Two entry points sit flat at the `scripts/` root — `build.sh` and
`gate-keepers.sh` — and every other script lives in a category
directory (the zelynic-shaped layout, night-improve-1). Categories
group scripts by the job they do, so the directory name tells you what
a script is for before you open it. All scripts are invoked from the
repository root unless a script's own header says otherwise (each
header also carries the platform note: UNIX-only — Linux, macOS, BSD).

| Entry point (root) | Purpose | Script |
|---|---|---|
| `build.sh` | Build pipeline orchestrator: check/build/pgo/miri/version-sync (sources the `lib/` modules: common, builds, quality, help, version-sync, miri, pgo — the NIGHT-lts-2 split of the former monolith) | `build.sh` |
| `gate-keepers.sh` | Every non-Rust quality gate in one run | `gate-keepers.sh` |

| Directory | Purpose | Scripts |
|---|---|---|
| `lib/` | Build pipeline modules sourced by `build.sh` | `common.sh`, `builds.sh`, `quality.sh`, `help.sh`, `version-sync.sh`, `miri.sh`, `pgo.sh` |
| `gates/` | Quality gates: every `check-*` guard plus the audit corpus (docs truth audit, stale-comment hunt, language audit, emoji sweep, visual-mode audit) and the disclaimer injector | `check-headers.sh`, `check-permissions.sh`, `check-rs-loc.sh`, `check-scripts-loc.sh`, `check-symbol-only-output.sh`, `check-version-anti-patterns.sh`, `check-rust-version-sync.sh`, `check-ci-path-filters.py`, `check-comment-style.py`, `check-name-case.py`, `check-release-notes-shapes.sh`, `inject-disclaimer.sh`, `docs-audit.py`, `stale-hunt.py`, `language_audit.py`, `emoji-audit.py`, `visual-mode-audit.py` |
| `dev/` | Version and visual dev tools: version bumping (project and Rust toolchain) and the visual-preset battle tool | `version-to.sh`, `rust-version-to.sh`, `bump-rust-to.sh`, `apply-visual-preset.sh` |
| `ci/` | CI helper scripts: the strict CI cargo wrapper, the Android NDK resolver, and the tag-pipeline CI waiter | `ci-strict-build.sh`, `resolve-latest-ndk.py`, `wait-for-ci.sh` |
| `depth/` | The LTS depth bore and the stress, end-to-end and hang harnesses that drive the built binary | `depthbore.py`, `cli_config_stresstest.sh`, `cli_suggestion_stresstest.sh`, `night_cbg34_e2e.py`, `nh2_shift_harness.py`, `signal_smoke.py`, `termux_hang_harness.py` |
| `package/` | User-facing install and uninstall | `install.sh`, `uninstall.sh` |
| `bench/` | Benchmark tooling: the benchmark runner and the terminal-size scaling sweep | `bench-runner.py`, `run_scaling_benchmarks.py` |
| `release/` | Release flow: release-notes generation and release-build verification | `generate-release-notes.sh`, `verify-release-build.sh` |

## Entry points

- `./scripts/build.sh help` — the build/check/pgo/miri/version-sync
  command list (canonical local verification entry point:
  `./scripts/build.sh check-all`).
- `./scripts/gate-keepers.sh` — every non-Rust quality gate in one
  run (`--fix-all` auto-fixes what can be auto-fixed).

## Size policy

Shell and Python scripts here stay at or below 1000 gross lines
(enforced by `gates/check-scripts-loc.sh`, gate-keepers check 17);
oversized scripts self-declare with a `# LOC_EXEMPT:` marker — the
policy is stated once in `docs/RULES.md` ("Scripts file size").
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
