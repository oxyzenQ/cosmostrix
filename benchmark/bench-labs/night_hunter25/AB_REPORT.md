<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-25 A/B report — 10 s benches, pro profile

Methodology: `git stash` → rebuild from clean `3bb7953` → 2 baseline
runs per scene → `git stash pop` → rebuild → 2 after runs per scene.
Command: `TERM=dumb ./target/pro/cosmostrix --benchmark
--bench-duration 10 --json --scene cinematic|monolith`. The change
deletes four stale `#[allow]` attributes (compile-time only) and
bundles two COLD-path signatures (`--verbose` pre-launch dump,
`--perf-stats` exit report) — the bench loop executes none of the
touched code.

## Cinematic

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 28 369 | 28 685 | +1.11 % |
| entropy bits | 5.1752 | 5.1686 | −0.129 % |
| density gini | 0.6369 | 0.6385 | +0.251 % |
| dirty cells/frame | 459.16 | 458.92 | −0.052 % |

## Monolith

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 68 821 | 69 966 | +1.66 % |
| entropy bits | 4.1755 | 4.1757 | +0.005 % |
| density gini | 0.8174 | 0.8173 | −0.022 % |
| dirty cells/frame | 140.83 | 140.92 | +0.065 % |

## Conclusion

The after side measured FASTER on both scenes — machine noise
swinging the other way (the same sandbox measured a −4 % outlier
run the previous day). Visual metrics identical within run noise;
the structural argument (bench path bypasses both cold paths)
settles it: performance-neutral, zero visual regression.
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
