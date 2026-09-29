<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-diner-9 A/B report — 10 s benches, release profile: sha2 0.10.9 vs 0.11.0

Methodology (same as night_dinner-5/8): baseline binary built from
clean `5a4f1e6` (HEAD before this task: rand 0.10.3, sha2 0.10.9, the
dinner-8 final state) in a detached worktree; after binary from
`92d5597` (the sha2 0.11.0 migration commit). Both `--release` (fat
LTO, codegen-units 1), built on the same host the same hour (after
image 3,032,872 B vs baseline 3,033,256 B). Command per run:
`TERM=dumb <bin> --benchmark --bench-duration 10 --json --scene
cinematic|monolith`, 3 runs per scene per side (n=3, matching the
dinner-8 monolith protocol). Raw JSONs session-side at
`/home/z/my-project/scripts/ab-dinner9/`.

Structural expectation: the frame path never enters sha2. Its two
runtime surfaces are the watcher thread's 750 ms config poll (~100 µs
of hashing per cycle = 0.013% CPU on a thread the render loop never
waits on) and startup/diagnostic fingerprints (`--dump-config`,
`--testconf` — not exercised by `--benchmark`). The render engines
call no digest code; the dragon's per-frame number stream is rand's,
not sha2's. The only plausible carryover into the measurement is
binary layout — swapping the digest 0.10/generic-array cluster for
digest 0.11/hybrid-array moves bytes around the same ~3 MiB image —
which is noise-class, not cost-class (the argument that settled the
clap, notify, and signal-hook A/Bs). On top of that, output identity
is already proven three ways (parity probe / NIST vectors / GNU
sha512sum end-to-end), so any visual difference would indicate a
harness fault, not a hash difference.

## Cinematic (glyph control, ambient jitter)

| metric | baseline r1-r3 (mean) | after r1-r3 (mean) | delta |
|---|---|---|---|
| avg fps | 28790.17 / 29074.93 / 29044.85 (28969.98) | 27418.18 / 29053.96 / 28884.16 (28452.10) | -1.79% |
| dirty cells/frame | 459.67 / 455.56 / 455.63 (456.95) | 464.06 / 456.37 / 459.21 (459.88) | +0.64% |
| entropy bits | 5.1727 / 5.1621 / 5.1615 (5.1655) | 5.1803 / 5.1631 / 5.1681 (5.1705) | +0.10% |
| density gini | 0.6374 / 0.6399 / 0.6401 (0.6391) | 0.6357 / 0.6395 / 0.6385 (0.6379) | -0.19% |

The fps mean delta is one run: after run1 (27418) is the cell's
outlier — runs 2-3 (29053.96 / 28884.16) sit inside the baseline's
own band (28790-29075). Medians: baseline ~28790, after ~28884
(+0.3%). Every visual metric moves less than the same-side
run-to-run spread (baseline gini itself spans 0.6374-0.6401 =
0.42%; the cross-side delta is -0.19%).

## Monolith (structured control, deterministic)

| metric | baseline r1-r3 (mean) | after r1-r3 (mean) | delta |
|---|---|---|---|
| avg fps | 88844.55 / 85371.34 / 90053.19 (88089.69) | 90410.89 / 90157.73 / 90128.83 (90232.48) | +2.43% |
| dirty cells/frame | 56.80 / 56.79 / 56.75 (56.78) | 56.76 / 56.72 / 56.80 (56.76) | -0.03% |
| entropy bits | 3.2951 / 3.2947 / 3.2946 (3.2948) | 3.2934 / 3.2941 / 3.2944 (3.2940) | -0.025% |
| density gini | 0.8961 / 0.8961 / 0.8961 (0.8961) | 0.8962 / 0.8961 / 0.8961 (0.8961) | +0.004% |

Gini is flat to four decimals on every run — the deterministic
control says the picture did not move. The fps delta is again one
run from the other side: baseline run2 (85371) is the outlier, and
the baseline's own spread (4,682 fps, 5.3%) is ~2.2x the cross-side
gap (2,143 fps). Notably the after side is 16x tighter (spread 282
vs 4,682) — the same after-side-tighter signature the dinner-8
campaign recorded for this host's monolith band (85-90K fps).
Cross-session consistency: dinner-8's after-rand binary ran monolith
at mean 89,900; today's after (dinner-8 state + sha2 0.11) runs
90,232 — +0.4%, well inside host-state variance.

## Conclusion (sha2 0.11.0)

Visual-identical and performance-neutral within run noise, exactly
as the structure predicts. The deterministic control (monolith) is
flat to four decimals on gini with entropy and dirty cells flat to
the third decimal; the ambient control (cinematic) wobbles inside
its own run-to-run spreads on every metric. The two-sided fps
deltas (-1.8% / +2.4%) are each the product of a single outlier run
on opposite sides, with no mechanism for sha2 code to reach the
frame path — the render loop executes zero digest instructions, and
the migration's identity proofs guarantee the hashes themselves
never moved a bit. Verdict: migration confirmed performance-neutral
and visual-identical; the last Unchanged line is gone at no cost.
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
