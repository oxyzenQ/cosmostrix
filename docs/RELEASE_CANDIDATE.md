# Release Candidate Checklist
<!-- SPDX-License-Identifier: GPL-3.0-only -->

cosmostrix follows [SemVer](https://semver.org/) for package versions. Git tags and GitHub Releases use a leading `v` (e.g. `v50.0.0`). Stable releases do not use `-stable.N` suffixes. Do not bump the version or create a tag until the release phase is explicitly authorized.

## Required Commands

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --all --locked
./scripts/build.sh check-all
cargo pro-linux-v3
./scripts/version-to.sh --check <version>
```

All must pass with zero errors before a release candidate is considered.

## Runtime Smoke

```bash
BIN="target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix"
"$BIN" -V
"$BIN" --doctor
"$BIN" --benchmark
"$BIN" --benchmark --bench-duration 3
"$BIN" --color red --color-tune saturation=1.5 --benchmark
```

Expected defaults: `application_mode: disabled`, `effective_runtime: identity`, `shadow_metrics: identity`, `shadow_risk: identity`, `config_gate: disabled`, `visual_runtime: protected`, `runtime_application: identity`, `actual_execution: single-threaded-renderer`. v50+ benchmark output must include six section headers — `"$BIN" --benchmark 2>&1 | grep -E "^(BENCHMARK ENVIRONMENT|MEMORY|CPU|COMPONENT TIMING|DRIFT|RESOURCE):"` — all six must appear. On Linux/macOS, `MEMORY`, `CPU`, and `RESOURCE` must report real numbers (not `unsupported`). The `RENDERER` section must contain `gpu_usage: not_applicable`.

### JSON output smoke

```bash
"$BIN" --benchmark --json | python3 -c "import json,sys; json.load(sys.stdin); print('valid JSON')"
```

Must print `valid JSON`. The JSON object must contain 13 top-level keys: status, system, renderer, config, environment, performance, memory, cpu, resource, component_timing, drift, throughput, timing.

## Benchmark & HUD RC Checklist

### `--bench-duration` + `--color-tune` validation

```bash
"$BIN" --benchmark --bench-duration 3   # exits 0, DRIFT section present
"$BIN" --benchmark --bench-duration 600 # exits 0 (no max cap)
"$BIN" --benchmark --bench-duration 0   # "below the 1-second minimum"
"$BIN" --color green --color-tune "saturation=1.5,brightness=0.9" --benchmark   # valid
"$BIN" --color aurora --color-tune "sat=0.0" --benchmark                        # grayscale
"$BIN" --color-tune "hue=30"          # "unknown key 'hue'"
"$BIN" --color-tune "saturation=4.0"  # "out of range [0, 3]"
"$BIN" --color-tune ""                # "value is empty"
```

### Live HUD overlay (manual interactive smoke)

```bash
"$BIN"
```

Then press `i` and verify: a top-left overlay appears showing the full HUD row set (fps, tgt, max, p99, cpu, rss, ehs, prs, scn, chr, clr, sped, dsty, dcel, tcel, prdr, crdr, ambt, glth, ctun, mnst, cid, up, screensize — see docs/HUD.md); the overlay updates ~1 time per second without flickering; press `i` again — the overlay disappears cleanly; press `q` — clean exit, terminal restored.

Note: `i` is lowercase-only (uppercase `I` is a no-op — see `docs/HUD.md` and `docs/RULES.md`).

## AUR Metadata Check

Verify `aur/cosmostrix-bin/PKGBUILD` and `aur/cosmostrix-bin/.SRCINFO` have matching `pkgver`, `pkgdesc`, and repository URL. Run `./scripts/version-to.sh --check <version>` to automate this.

## README / CHANGELOG Guard + Benchmark Interpretation

- README must link to CHANGELOG.md and must not contain release notes sections or old version-history headings.
- README must stay scannable (no hard line cap — three-engine narrative, full CLI reference, scenes, configuration, multi-platform install, GPG verification, benchmarking).
- CHANGELOG is the dedicated release history document.
- Canonical tagline must be aligned across Cargo.toml, README.md, clap about, runtime identity, and AUR pkgdesc.

Benchmark FPS is synthetic uncapped throughput measured in a headless simulation. The actual runtime target is the configured FPS (dynamic default: 60 on standard terminals, 144 on high-refresh terminals; override with `--fps`). Do not chase raw FPS; frame-time stability and p99 latency matter more. See [benchmark/HIST_BENCH.md](../benchmark/HIST_BENCH.md) for detailed metric definitions.

## Manual Visual Smoke

Run these interactively and verify clean exit with `q`:

```bash
"$BIN"
"$BIN" --color sun
"$BIN" -mb "one world first seriously matrix rain"
"$BIN" --color green --color-tune "saturation=1.5,brightness=1.2"
```

For the last command, verify the rain renders with visibly boosted saturation + brightness compared to `--color green` alone. Verify: terminal restored cleanly on exit (no raw mode, no alternate screen residue); no visual regressions compared to the previous release; color, charset, and scene transitions are smooth.

## Rollback Notes & Release Workflow Authentication

- Use `git revert` to undo a release commit if issues are found post-push.
- GitHub Releases can be deleted if no users have downloaded the asset.
- AUR package can be reset by bumping `pkgrel` and publishing a fix.
- Do not force-push to `main`; use revert or fix-forward.
- The release workflow (`release.yml`) requires `contents: write` for the `publish_release` job. The `GITHUB_TOKEN` is passed explicitly to `softprops/action-gh-release` via `env`. If it fails with HTTP 401, verify repository/org settings have not restricted the default `GITHUB_TOKEN` permissions.
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
