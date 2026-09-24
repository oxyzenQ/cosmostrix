<!-- SPDX-License-Identifier: GPL-3.0-only -->

# scripts/ Layout

Every script lives in a category directory — nothing sits flat at the
`scripts/` root. Categories group scripts by the job they do, so the
directory name tells you what a script is for before you open it. All
scripts are invoked from the repository root unless a script's own
header says otherwise (each header also carries the platform note:
UNIX-only — Linux, macOS, BSD).

| Directory | Purpose | Scripts |
|---|---|---|
| `build/` | Build pipeline: the orchestrator entry point, the strict CI cargo wrapper, and the Android NDK resolver | `build.sh`, `ci-strict-build.sh`, `resolve-latest-ndk.py` |
| `gates/` | Quality gates: the pre-commit gatekeeper and every `check-*` guard plus the disclaimer injector it invokes | `gate-keepers.sh`, `check-headers.sh`, `check-permissions.sh`, `check-rs-loc.sh`, `check-scripts-loc.sh`, `check-symbol-only-output.sh`, `check-version-anti-patterns.sh`, `check-rust-version-sync.sh`, `check-ci-path-filters.py`, `check-comment-style.py`, `inject-disclaimer.sh` |
| `release/` | Version and release flow: version bumping (project and Rust toolchain), release-notes generation, release-build verification | `version-to.sh`, `rust-version-to.sh`, `bump-rust-to.sh`, `generate-release-notes.sh`, `verify-release-build.sh` |
| `audit/` | One-shot corpus audits: docs truth audit, stale-comment hunt, language audit, emoji sweep, visual-mode audit | `docs-audit.py`, `stale-hunt.py`, `language_audit.py`, `emoji-audit.py`, `visual-mode-audit.py` |
| `bench/` | Benchmark tooling: the benchmark runner and the terminal-size scaling sweep | `bench-runner.py`, `run_scaling_benchmarks.py` |
| `harness/` | Stress, end-to-end and preset-battle harnesses that drive the built binary | `cli_config_stresstest.sh`, `cli_suggestion_stresstest.sh`, `night_cbg34_e2e.py`, `nh2_shift_harness.py`, `termux_hang_harness.py`, `apply-visual-preset.sh` |
| `setup/` | User-facing install and uninstall | `install.sh`, `uninstall.sh` |
| `depthbore/` | The LTS depth bore: long-horizon bug-class probe suite (pre-existing category, unchanged) | `depthbore.py` |

## Entry points

- `./scripts/build/build.sh help` — the build/check/pgo/miri/version-sync
  command list (canonical local verification entry point:
  `./scripts/build/build.sh check-all`).
- `./scripts/gates/gate-keepers.sh` — every non-Rust quality gate in one
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
