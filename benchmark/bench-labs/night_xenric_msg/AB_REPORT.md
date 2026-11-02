<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# NIGHT-xenric A/B report — default overlay message wording change

Change under test: `default_message_text()` (src/types/constants.rs)
rewritten from `"Experience cosmostrix v<CARGO_PKG_VERSION>"` to
`"Experience with cosmostrix v<CARGO_PKG_VERSION>"` (owner request:
the default overlay message must read "Experience with cosmostrix").
Comment and doc references to the old phrasing synced in the same
commit (configfile dump template, USER_CONFIG_KEYS comment,
phosphor_anomaly history note, live-reload rejection test comment,
LIVE_RELOAD_BEHAVIOR.md precedence list, hunt-36 module doc).

Methodology (night_dinner-8 shape): baseline binary built from clean
HEAD 1589276; after binary from the working tree with only this
change. Both `--release` (fat LTO, codegen-units 1, rustc 1.98.1),
built the same hour on the same 2-core host. Command per run:
`TERM=dumb <bin> --benchmark --bench-duration 10 --json --scene
cinematic|monolith`, 2 runs per scene. Raw JSONs committed beside
this report.

## Structural expectation: an exact tie

Benchmark mode never renders the message overlay
(`component_timing.message_active = false` on all eight runs, both
binaries), so the changed string is never drawn, measured, or
allocated per frame in this path — the wording lives only on the
interactive startup path (one allocation per process). Any delta is
run-to-run noise by construction; the runs below confirm it.

## Cinematic (glyph control)

| metric | baseline r1 | baseline r2 | after r1 | after r2 | avg delta |
|---|---|---|---|---|---|
| fps | 28167.3 | 28723.3 | 28689.0 | 28807.9 | +1.07% |
| dirty cells/frame | 462.3 | 458.6 | 458.4 | 457.3 | -0.56% |
| density gini | 0.6354 | 0.6388 | 0.6391 | 0.6386 | +0.28% |
| frame entropy (bits) | 5.1813 | 5.1683 | 5.1661 | 5.1677 | -0.15% |

## Monolith (structured control)

| metric | baseline r1 | baseline r2 | after r1 | after r2 | avg delta |
|---|---|---|---|---|---|
| fps | 89721.7 | 89453.1 | 89506.7 | 89575.8 | -0.05% |
| dirty cells/frame | 56.75 | 56.79 | 56.76 | 56.76 | -0.01% |
| density gini | 0.8961 | 0.8962 | 0.8962 | 0.8961 | -0.00% |
| frame entropy (bits) | 3.2950 | 3.2939 | 3.2936 | 3.2956 | +0.00% |

## Verdict

**PASS — statistical tie on every metric, as the structural
expectation predicts.** The +1.07% cinematic fps spread is inside the
observed run-to-run variance for this host (the two baseline runs
alone differ by +2.0%); monolith, the deterministic control, is flat
to three decimal places. `message_active` is false on every run of
both binaries. The wording change is rendered-output-only and lands.

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
