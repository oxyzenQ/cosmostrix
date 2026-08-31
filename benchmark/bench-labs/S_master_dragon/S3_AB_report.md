<!-- SPDX-License-Identifier: GPL-3.0-only -->

S-master-3 Security LTS Harden A/B Comparison
Scene: monolith | Sizes: 6
Baseline A: pre-harden (post-S2 state)
After B:    post-harden (live-reload message sanitize + length cap)

Size     Metric                 A (before)       B (after)        Delta        Verdict
-------- ---------------------- ---------------- ---------------- ------------ ----------
6x6      avg_fps                1567654.9370     1553249.7615     -0.92%      stable
6x6      entropy                0.0000           0.0000           +0.00%      stable
6x6      gini                   0.8333           0.8333           +0.00%      stable
6x6      avg_dirty_cells        0.6674           0.6677           +0.03%      stable
20x20    avg_fps                496723.2013      499501.2625      +0.56%      stable
20x20    entropy                0.7535           0.7537           +0.03%      stable
20x20    gini                   0.9165           0.9165           -0.00%      stable
20x20    avg_dirty_cells        7.9329           7.9350           +0.03%      stable
40x20    avg_fps                302880.6716      303401.4769      +0.17%      stable
40x20    entropy                1.4355           1.4358           +0.02%      stable
40x20    gini                   0.9359           0.9354           -0.05%      stable
40x20    avg_dirty_cells        14.2220          14.2204          -0.01%      stable
80x24    avg_fps                93823.6759       93152.6420       -0.72%      stable
80x24    entropy                3.2939           3.2947           +0.03%      stable
80x24    gini                   0.8962           0.8961           -0.01%      stable
80x24    avg_dirty_cells        56.7571          56.8263          +0.12%      stable
120x40   avg_fps                54411.5807       53598.1878       -1.49%      stable
120x40   entropy                3.9244           3.9249           +0.01%      stable
120x40   gini                   0.8943           0.8943           -0.01%      stable
120x40   avg_dirty_cells        107.2830         107.1332         -0.14%      stable
200x60   avg_fps                29992.1210       29618.7942       -1.24%      stable
200x60   entropy                4.7128           4.7126           -0.00%      stable
200x60   gini                   0.8905           0.8905           +0.00%      stable
200x60   avg_dirty_cells        205.0553         204.9525         -0.05%      stable

Verdict: All metrics within +-2% natural variance. Security fix
(message sanitize + length cap) only affects live-reload path,
not bench mode (no config file). Zero visual/perf regression.
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
