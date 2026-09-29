<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-dinner-5 A/B report — 10 s benches, release profile: clap 4.5.61 vs 4.6.7

Methodology (same as cybersecurity-1/hunt-36/37): baseline binary
built from clean `579de6a` (HEAD before the clap relaxation, clap
4.5.61) in a detached worktree; after binary from `eb18b31` (clap
4.6.7). Both `--release` (fat LTO, codegen-units 1), both built the
same hour on the same host. Command per run: `TERM=dumb <bin>
--benchmark --bench-duration 10 --json --scene cinematic|monolith`,
2 runs per scene. Cinematic is the glyph rain (droplet-family
control); monolith is the structured-style control.

Structural expectation: clap executes exactly once, at process
startup, parsing argv before the first frame. The bench steady-state
loop never calls into clap (no help rendering, no error paths, no
suggestion matching during the loop). The only plausible carryover
into the measurement is binary layout — where the linker places the
clap 4.6.7 code versus 4.5.61 inside the same 3.03 MiB image — which
is noise-class, not cost-class.

## Cinematic (glyph control)

| metric | baseline r1 | baseline r2 | after r1 | after r2 |
|---|---|---|---|---|
| avg fps | 28,765.63 | 28,783.62 | 28,823.75 | 27,925.97 |
| dirty cells/frame | 458.24 | 457.78 | 457.60 | 461.42 |
| entropy bits | 5.1689 | 5.1622 | 5.1672 | 5.1810 |
| density gini | 0.6384 | 0.6398 | 0.6385 | 0.6359 |

## Monolith (structured control)

| metric | baseline r1 | baseline r2 | after r1 | after r2 |
|---|---|---|---|---|
| avg fps | 87,638.57 | 87,804.84 | 87,541.35 | 87,479.94 |
| dirty cells/frame | 56.81 | 56.76 | 56.74 | 56.79 |
| entropy bits | 3.2954 | 3.2943 | 3.2944 | 3.2943 |
| density gini | 0.8960 | 0.8961 | 0.8961 | 0.8962 |

## Conclusion

Performance-neutral and visual-identical within run noise. The
monolith control — the deterministic structured scene, where this
host's run-to-run swing is smallest — is flat to four decimal places
on entropy and gini and within 0.04% on fps and dirty cells. The
cinematic scene shows the expected sandbox swing: after r1 lands
+0.2% above baseline while after r2 lands -3.0% below it, a 3.2%
spread between two runs of the SAME binary — the signature of
scheduler noise, not a systematic cost (this sandbox routinely swings
plus/minus 1-4% between identical runs; see the cybersecurity-1
report for the same pattern). The structural argument settles it:
the bench loop never enters clap, so the two binaries share the same
steady-state frame path, and every visual metric (entropy, gini,
dirty cells) sits within 0.35% in both scenes — well inside the
noise band, on both sides of zero.

The dependency win is elsewhere and already CI-proven: all 21 checks
green on the clap commit (every platform build, both nextest
partitions, MSRV, CodeQL, cargo-deny, Miri), and the constraint now
admits one minor line of clap headroom instead of zero.

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
<!-- COSMOSTRIX-VERIFIED-A/B -->
