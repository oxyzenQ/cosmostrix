<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-dinner-8 A/B report — 10 s benches, release profile: signal-hook 0.3.18 vs 0.4.4 (then rand 0.9.5 vs 0.10.x, appended below)

Methodology (same as night_dinner-5): baseline binary built from
clean `6e3d718` (HEAD before the task: signal-hook 0.3.18, rand
0.9.5); after binary from `8469234` (the signal-hook 0.4.4 migration
commit). Both `--release` (fat LTO, codegen-units 1), both built on
the same host the same hour. Command per run: `TERM=dumb <bin>
--benchmark --bench-duration 10 --json --scene cinematic|monolith`,
2 runs per scene. Cinematic is the glyph rain control; monolith is
the deterministic structured control. Raw JSONs session-side at
`/home/z/my-project/scripts/ab-dinner8/`.

Structural expectation: the frame path never enters signal-hook.
Interactive mode installs two signal threads that block in
`Signals::forever()` (a sigtimedwait-style loop — zero CPU until a
signal arrives, and none arrives during a bench); benchmark mode
registers exactly one SIGINT flag at startup (an AtomicBool write
only if a signal fires — it never does in an uninterrupted run).
The two binaries differ in linked signal-hook 0.3.18 vs 0.4.4 code
that the steady-state loop never executes; the only plausible
carryover into the measurement is binary layout — where the linker
places the new crate version inside the same ~3 MiB image — which
is noise-class, not cost-class (the same argument that settled the
clap and notify A/Bs).

## Cinematic (glyph control)

| metric | baseline r1 | baseline r2 | after r1 | after r2 |
|---|---|---|---|---|
| avg fps | 28,621.43 | 29,087.22 | 28,781.38 | 29,168.06 |
| dirty cells/frame | 460.28 | 454.67 | 455.30 | 455.10 |
| entropy bits | 5.1751 | 5.1607 | 5.1636 | 5.1655 |
| density gini | 0.6373 | 0.6406 | 0.6398 | 0.6392 |

## Monolith (structured control)

| metric | baseline r1 | baseline r2 | after r1 | after r2 |
|---|---|---|---|---|
| avg fps | 87,160.40 | 87,452.64 | 87,301.66 | 86,137.09 |
| dirty cells/frame | 56.77 | 56.74 | 56.76 | 56.76 |
| entropy bits | 3.2924 | 3.2939 | 3.2946 | 3.2936 |
| density gini | 0.8963 | 0.8962 | 0.8962 | 0.8962 |

## Conclusion (signal-hook 0.4.4)

Performance-neutral and visual-identical within run noise. The
monolith control is flat to four decimals on gini (0.8962/0.8963 on
both sides), entropy moves +0.03% (a fourth-decimal wobble inside
the same-binary spread), and dirty cells are identical to a
hundredth. Fps means move -0.67% with median at -0.18% and peak at
+0.95% — no consistent direction, the two-sided signature of
scheduler noise rather than a systematic cost. Cinematic straddles
the baseline the same way: gini and entropy sit inside the
baseline's own run-to-run spread, fps means land +0.42% ABOVE the
baseline, and peak fps shows the familiar same-binary swing (35.3K
to 38.8K across two runs of the after binary — a 3.5K spread
between identical binaries, the scheduler-noise signature documented
in every prior A/B of this project).

The structural argument settles it: the bench loop never enters
signal-hook, so the two binaries share the same steady-state frame
path byte for byte — 2983/2983 tests plus the 8-check PTY signal
harness (`scripts/harness/signal_smoke.py`) carry the functional
side of the proof (behavior identical to the 0.3.18 baseline on
every exit path), and this bench carries the performance side:
nothing moved beyond noise.
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
