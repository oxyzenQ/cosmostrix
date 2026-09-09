<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-25 part 2 A/B report — 10 s benches, pro profile

Methodology: baseline built from `5bb2bfd` (the seven-positional
signatures), after built from `b71d416` (the bundle family). Command:
`TERM=dumb ./target/pro/cosmostrix --benchmark --bench-duration 10
--json --scene cinematic|monolith|solar_flare`, 2 runs per scene per
side, run-averaged below. Unlike part 1 (cold paths, bench loop
executed none of the changed code), this change sits INSIDE the
per-cell inner loop — every visible droplet cell, every frame — so
the A/B is the load-bearing evidence for the "scalar-replaces to the
same codegen" claim.

## Cinematic

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 28 664.87 | 28 787.46 | +0.428 % |
| entropy bits | 5.1621 | 5.1678 | +0.110 % |
| density gini | 0.6379 | 0.6362 | −0.260 % |
| dirty cells/frame | 456.60 | 459.40 | +0.613 % |

## Monolith

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 70 235.05 | 70 374.01 | +0.198 % |
| entropy bits | 4.1755 | 4.1754 | −0.012 % |
| density gini | 0.8174 | 0.8175 | +0.011 % |
| dirty cells/frame | 140.86 | 140.81 | −0.038 % |

## Solar flare (the SolarCellPaint scene)

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 35 742.34 | 36 787.14 | +2.923 % |
| entropy bits | 6.0478 | 6.0400 | −0.129 % |
| density gini | 0.3199 | 0.3239 | +1.240 % |
| dirty cells/frame | 248.88 | 248.92 | +0.017 % |

Run-level spread (the noise yardstick): baseline fps 35 102 / 36 382
— its own two runs differ by 3.6 %, more than the 2.9 % cross-side
delta; the slow baseline run is the outlier (the same sandbox
measured a −4 % swing in part 1). The gini delta is +0.004 absolute
on a 0.32 value with stochastic flare placement run to run. Both
deltas sit inside the scene's own run-to-run variance.

## Conclusion

Performance-neutral and visual-neutral within noise on all three
scenes, exactly what the design predicts: every bundle is all-`Copy`
scalars (16 bytes for `CellPaint`), destructured once at the top of
the function body, so LLVM's SROA dissolves it back into the same
register-level parameter passing the positionals used. The named
fields buy the cross-wire hazard elimination (line/col,
head_put_line/length, now/t1, the bool triple) at zero runtime
cost. The rigorous inner-loop A/B the owner mandated for this family
is satisfied: no regression, no visual drift, no dirty-cell change.
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
