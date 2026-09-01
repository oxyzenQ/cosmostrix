<!-- SPDX-License-Identifier: GPL-3.0-only -->

# v51.2 evidence — power-dragon banded density + ambient overlay lift

<!-- COSMOSTRIX-DISCLAIMER -->

## 10s monolith 80x24 A/B (same machine, same day)

- `A*.json` — baseline control pair, pre-change tree at 018920b
  (A/A2 primary; A1-A4 the same-tree control rerun used to establish
  the allocator-variance baseline).
- `B*.json` — post-change tree (v51.2).
- `live_pty_trace.log` — PTY live proof of the ambient overlay-lift
  contract (real binary, `COSMOSTRIX_LIVE_RELOAD_DEBUG=1`, graceful
  `q` exit): ambient applies at 2s (snapback), ALL `ambient.*` keys
  commented out at 5s (trace: "schedule emptied — reverting
  ambient-owned scene 'monolith' to the locked startup scene
  'crystal-dragon'"), ambient uncommented at 10s (scheduler refires,
  final scene monolith). Command shape:
  `cosmostrix -v -s --config <cfg> --scene crystal-dragon -mfs words`
  with `ambient.<HH-MM> = monolith` + `ambient-snapback-secs = 2`.

Summary (identical invocation, zero-pressure bench — the banded curve
is behaviorally identical in the dead zone):

| run | fps | entropy | gini | dirty | allocs |
| --- | ---: | ---: | ---: | ---: | ---: |
| A   | 98,641 | 3.2944 | 0.8957 | 56.77 | 563 |
| A2  | 96,156 | 3.2954 | 0.8960 | 56.74 | 563 |
| A3 (control rerun) | — | — | — | — | 564 (1 of 4) |
| B   | 98,899 | 3.2938 | 0.8962 | 56.77 | 563 |
| B2  | 98,853 | 3.2949 | 0.8961 | 56.78 | 564 |
| B3  | 98,996 | 3.2936 | 0.8962 | 56.77 | 563 |
| B4  | 98,573 | 3.2942 | 0.8961 | 56.76 | 563 |

Verdict: visual parity (entropy/gini within noise, streams 23
identical, dirty cells 56.74-56.78 both sides), fps within the
session noise band (the A/A2 spread alone was 2.5%), deallocs
bit-stable 553 everywhere. The occasional 564th allocation is
PRE-EXISTING nondeterministic variance (it appears on the pre-change
control binary too, 1 of 4 runs) — not a v51.2 regression.

Detail: `docs/research/V51_2_POWER_DRAGON_AMBIENT_CONTRACT.md`.
