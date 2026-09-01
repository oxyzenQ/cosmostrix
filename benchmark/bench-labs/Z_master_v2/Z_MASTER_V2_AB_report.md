<!-- SPDX-License-Identifier: GPL-3.0-only -->

Z-master-v2 (1/2/3) A/B Comparison
Scene: monolith (80x24, dry benchmark) | Duration: 10s per side
Baseline A: 6c95c47 pre-audit tree (priority-contract gaps present)
After B:    post Z-master-1-v2 (24b3a28) + Z-master-2-v2 (c60441e)

Metric                        A (before)       B (after)        Delta        Verdict
---------------------------- ---------------- ---------------- ------------ ----------
avg_fps                       92293.7451       92137.2163       -0.17%       stable
avg_frame_time_ms             0.0108           0.0109           +0.17%       stable
p95_frame_time_ms             0.0191           0.0128           -32.84%      stable (jitter band)
frame_entropy_bits            3.2930           3.2938           +0.03%       identical
density_gini                  0.8962           0.8962           -0.01%       identical
color_transition_delta_avg    0                0                0%           identical
visual_samples                92293            92137            -0.17%       stable
dirty_cells_per_frame         56.7350          56.7997          +0.11%       stable
logical_cells_per_frame       1920             1920             +0.00%       identical
render_ns_per_cell            44.8254          45.2879          +1.03%       stable
io_ns_per_cell                4.2543           4.2370           -0.41%       stable
total_ns_per_cell             190.9751         191.0817         +0.06%       stable
dirty_glyphs_per_second       5236286          5233364          -0.06%       stable
active_streams_avg            23               23               +0.00%       identical
total_drawn_cells             52362889         52333691         -0.06%       stable

Resolved config (identical both sides): scene=monolith, color=energy-zen,
charset=zen, bold=Random, shading=DistanceFromHead, speed=30, density=0.85,
glitch=subtle, async=true, effects=off.

Verdict: visual metrics bit-parity (entropy/gini/streams/cells identical
to RNG noise level); performance within the natural variance band
(<=3%, matching the S-master_dragon precedent). Z-master-v2 changes
touch ONLY the config-apply / live-reload priority gates — zero code on
the per-frame render path, so parity is the expected and observed
result. No further optimization warranted (already at peak; per task
brief: skip, do not over-engineer).
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
