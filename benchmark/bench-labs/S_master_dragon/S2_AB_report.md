<!-- SPDX-License-Identifier: GPL-3.0-only -->

S-master-2 Optimize Code A/B Comparison
Scene: monolith | Sizes: 6
Baseline A: pre-optimize (post-S1 state)
After B:    post-optimize (const-gate fog + direct-index vignette LUT)

Size     Metric                 A (before)       B (after)        Delta        Verdict
-------- ---------------------- ---------------- ---------------- ------------ ----------
6x6      avg_fps                1551023.5800     1554558.2178     +0.23%      stable
6x6      entropy                0.0000           0.0000           +0.00%      stable
6x6      gini                   0.8333           0.8333           -0.00%      stable
6x6      avg_dirty_cells        0.6675           0.6678           +0.05%      stable
6x6      total_ns_per_cell      965.9638         963.2729         -0.28%      stable
20x20    avg_fps                500712.8699      493393.6664      -1.46%      stable
20x20    entropy                0.7536           0.7521           -0.19%      stable
20x20    gini                   0.9165           0.9166           +0.01%      stable
20x20    avg_dirty_cells        7.9254           7.9348           +0.12%      stable
20x20    total_ns_per_cell      251.9932         255.4305         +1.36%      stable
40x20    avg_fps                305345.4000      304194.1301      -0.38%      stable
40x20    entropy                1.4367           1.4351           -0.11%      stable
40x20    gini                   0.9358           0.9359           +0.01%      stable
40x20    avg_dirty_cells        14.2344          14.2319          -0.02%      stable
40x20    total_ns_per_cell      230.0754         230.9858         +0.40%      stable
80x24    avg_fps                93224.7801       93348.2637       +0.13%      stable
80x24    entropy                3.2943           3.2975           +0.10%      stable
80x24    gini                   0.8961           0.8955           -0.07%      stable
80x24    avg_dirty_cells        56.8090          56.8250          +0.03%      stable
80x24    total_ns_per_cell      188.8214         188.5187         -0.16%      stable
120x40   avg_fps                54083.6875       53428.9339       -1.21%      stable
120x40   entropy                3.9252           3.9245           -0.02%      stable
120x40   gini                   0.8943           0.8943           +0.01%      stable
120x40   avg_dirty_cells        107.2915         107.3006         +0.01%      stable
120x40   total_ns_per_cell      172.3330         174.4300         +1.22%      stable
200x60   avg_fps                29836.4478       29906.3501       +0.23%      stable
200x60   entropy                4.7155           4.7143           -0.03%      stable
200x60   gini                   0.8903           0.8904           +0.01%      stable
200x60   avg_dirty_cells        204.9873         205.1464         +0.08%      stable
200x60   total_ns_per_cell      163.5031         162.9944         -0.31%      stable

Verdict: All metrics within natural variance band (<=3%).
S-master-2 changes are const-gate + direct-index micro-opts —
zero visual regression, perf neutral (gains <1%, below bench
noise floor). Codebase confirmed post-peak-optimized.
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
