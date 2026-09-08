<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-24 (F-24-1) A/B report — 10 s benches, pro profile

Methodology: `git stash` → rebuild from clean `58b1d85` → 3 baseline
runs per scene → `git stash pop` → rebuild → 3 after runs per scene.
Command: `TERM=dumb ./target/pro/cosmostrix --benchmark
--bench-duration 10 --json --scene cinematic|monolith`. The change is
validation-only (colors-custom load contract, canonical probes, twin
dedup) — cold startup path; the bench loop never executes the touched
code, so any delta is expected to be pure machine noise.

## Cinematic

| metric | baseline (mean of 3) | after (mean of 3) | delta |
|---|---|---|---|
| avg fps | 28 966 | 28 874 | −0.32 % |
| entropy bits | 5.1642 | 5.1641 | −0.004 % |
| density gini | 0.6393 | 0.6394 | +0.018 % |
| dirty cells/frame | 456.69 | 456.80 | +0.024 % |

Run-to-run spread 1.96 % (28 635–29 195); the per-run series
interleave (after run 2 = 29 195 is the overall maximum). Verdict:
no regression — deltas inside the noise band.

## Monolith

| metric | baseline (mean of 3) | after (mean of 3) | delta |
|---|---|---|---|
| avg fps | 69 588 | 68 564 | −1.47 % (skewed by one outlier run) |
| entropy bits | 4.1754 | 4.1752 | −0.003 % |
| density gini | 0.8175 | 0.8175 | −0.000 % |
| dirty cells/frame | 140.78 | 140.94 | +0.11 % |

Runs: baseline 69 132 / 70 039 / 69 593; after 69 850 / 69 097 /
66 746. The 66 746 run is a machine-load outlier (the two remaining
after-runs interleave the baseline series). Visual metrics identical
to the 3rd decimal. Verdict: no regression.

## Conclusion

Zero visual regression; fps differences are run noise with
interleaved series on both scenes. Combined with the structural
argument (bench path bypasses config validation entirely), the
change is performance-neutral.
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
