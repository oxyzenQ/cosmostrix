<!-- SPDX-License-Identifier: GPL-3.0-only -->

S-master-1 Dragon Hunt A/B Comparison
Scene: monolith | Sizes: 6
Baseline A: pre-cleanup (stale 1500-LOC guards + dead CfgInputs fields + dead DriftHistory::reset + stale #[allow(dead_code)])
After B:    post-cleanup (800-LOC guards + dead weight removed)

Size     Metric                 A (before)       B (after)        Delta        Verdict
-------- ---------------------- ---------------- ---------------- ------------ ----------
6x6      avg_fps                1578626.1365     1569251.5226     -0.59%      stable
6x6      entropy                0.0000           0.0043           +0.00%      stable
6x6      gini                   0.8333           0.8319           -0.17%      stable
6x6      avg_dirty_cells        0.6675           0.6677           +0.03%      stable
20x20    avg_fps                500805.7045      492781.2564      -1.60%      stable
20x20    entropy                0.7536           0.7526           -0.14%      stable
20x20    gini                   0.9165           0.9166           +0.00%      stable
20x20    avg_dirty_cells        7.9345           7.9295           -0.06%      stable
40x20    avg_fps                302626.0497      305198.2175      +0.85%      stable
40x20    entropy                1.4372           1.4364           -0.06%      stable
40x20    gini                   0.9358           0.9359           +0.00%      stable
40x20    avg_dirty_cells        14.2378          14.2195          -0.13%      stable
80x24    avg_fps                92637.9134       90934.8122       -1.84%      stable
80x24    entropy                3.2959           3.2935           -0.07%      stable
80x24    gini                   0.8960           0.8962           +0.02%      stable
80x24    avg_dirty_cells        56.7817          56.7969          +0.03%      stable
120x40   avg_fps                53575.6479       53701.8289       +0.24%      stable
120x40   entropy                3.9257           3.9242           -0.04%      stable
120x40   gini                   0.8942           0.8943           +0.01%      stable
120x40   avg_dirty_cells        107.4414         107.3961         -0.04%      stable
200x60   avg_fps                29265.4037       29619.2066       +1.21%      stable
200x60   entropy                4.7106           4.7157           +0.11%      stable
200x60   gini                   0.8907           0.8899           -0.09%      stable
200x60   avg_dirty_cells        205.1405         204.9715         -0.08%      stable

Verdict: All metrics within natural variance band (<=3%).
S-master-1 changes are pure dead-code/comment cleanup — zero
visual or performance regression, as required by task brief.
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
