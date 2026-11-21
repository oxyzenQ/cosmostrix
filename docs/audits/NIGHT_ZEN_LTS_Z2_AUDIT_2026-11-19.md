<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# Sharpest-skill depth audit Z-2: the emission path after the certificate seam (NIGHT-zen-lts-Z-2)

Date: 2026-11-19
Scope: the one capability cosmostrix cannot lose without losing its
identity - the diff-based render path - audited at its NEW shape: since
NIGHT-zen-lts-1, night-frontier-Z9-C moved the shipped steady-state
solver out of `Terminal::draw` into a pure seam module
(`src/engine/cosmic_dragon_engine/terminal/emit_diff.rs`) so the
emission-minimality certificate (INNOVATIONS entry 57, INV-22) could
bind to the code the binary actually ships. The audit covers the seam
module in full, the certificate suite's teeth
(`test/cosmic_dragon_incubator/tests/emission_ledger.rs`, 699 lines),
and the no-regression proof across the twenty-eight-commit delta that
built it. Audit baseline: 258a415 (HEAD of main). Method: full read of
the seam module, line-level and structural verbatim analysis of the
extraction commit (fbcae17), a cross-commit replay-digest A/B against
a worktree at the pre-delta commit, and the hostile battery at HEAD.

## Verdict

**The sharpest skill is at peak at today's HEAD - zero defects, zero
code changes. The extraction is proven verbatim by three independent
proofs, and the certificate is proven to have teeth by its rejection
tests.** Per the owner's audit-if-peak-skip rule this is a record-only
audit: the evidence is written down so the next session does not
re-audit the same surface.

## 1 - The seam module read: pure computation, one owner of the byte stream

`encode_diff_emission` is pure over its inputs: no I/O, no terminal,
no clock (the module header states it and the signature proves it -
`&Frame`, `&mut [Cell]`, buffers in, `DiffEmissionStats` out). It owns
exactly the byte stream: the flat sorted dirty iteration (with a
debug-only bounds assert guarding the Frame::set invariant), the
unchanged-cell skip, the contiguous same-style runs, the deferred
`MoveTo`, the cached SGR bridges through `emit_sgr` (the task-17
quantizer at the emission boundary), the BOLT bold table, and the
trailing attribute reset. The caller (`draw.rs`, now 204 lines
lighter) keeps only the flush and the dirty-clear. The three named
laws the solver optimizes within are restated in the module docs: no
overpaint (HUNT-27 byte-silence), row-boundary cursor resync (bug-12),
and the bridge grammar. `DiffEmissionStats` is exhaustive by
construction: payload + moves + sgrs + bolds, with `bytes` the total
the emission ledger asserts.

## 2 - The verbatim extraction: proven three independent ways

1. **Line-level**: the commit diff's removed-from-draw lines versus
   added-to-emit_diff lines match 78/139 exact-normalized; every
   residual line is exactly one of the classes the module docs declare
   (the `DiffEmissionStats` declaration, the parameterized signatures,
   and the `stats.` byte-class counters - which never touch the
   emitted stream).
2. **Structural**: the flat dirty-index structure pre-existed the
   extraction - `dirty_flat`/`sort_unstable` appear 15 times in
   `fbcae17^:draw.rs` - so the PERF comment moved with code that was
   already shipped; the extraction did not introduce it.
3. **Empirical (the decisive one)**: a worktree at the pre-delta
   commit 1565607 was built with the same pinned toolchain and both
   binaries ran the replay oracle twice each. All four
   `replay-digest:` lines are identical:
   `b3:7e938efa5efcb659dc214a9bf1fc84afdbbe8d6b8e0aab9f67b0b3606f1578d7`
   - also equal to the value NIGHT-depth-hardening-4 recorded. The
   shipped byte stream did not move across the twenty-eight commits
   that extracted the solver, built the certificate, and landed
   entries 55-58. No visual regression exists in the pinned sequence;
   nothing stronger than this can be measured.

## 3 - The certificate's teeth: the law reference and the rejections

The INV-22 suite binds the shipped solver to a reference derived from
the LAW LIST, not from the solver's code: `law_reference` walks the
changed cells in the scanline order the row-resync law forces and
charges exactly what the grammar charges (`cup_len`, `bridge_len` with
the session quantizer, the BOLD table lengths, the glyph's UTF-8 bytes,
plus the trailing reset). The approved band `[reference, 1.05 x
reference]` is evaluated in u64 integer arithmetic (`shipped * DEN <=
reference * NUM`) - no float enters the assertion. The suite's teeth:

| Test | What it proves |
|---|---|
| `diff_solver_is_byte_optimal_on_adversarial_grids` | shipped stays inside the band against the law reference on hostile grids across wire variants (truecolor/16/256 with cache) |
| `diff_solver_matches_the_law_reference_on_the_rig_window` | the rig window scenario agrees with the reference walk |
| `lean_writer_accounting_is_byte_exact_across_styles` | the layer-1 identity: bytes = payload + furniture, exact across style combinations |
| `certificate_rejects_per_cell_cursor_pessimization` | an injected per-cell MoveTo plan is CAUGHT |
| `certificate_rejects_per_cell_bridge_pessimization` | an injected per-cell SGR plan is CAUGHT |
| `certificate_rejects_overpaint_pessimization` | an injected overpaint plan is CAUGHT |

The three rejection tests are the house pattern ("a ledger that cannot
see a violation cannot enforce one"): the certificate demonstrably
detects the pessimization classes it exists to prevent, so a future
solver edit that silently wastes bytes fails CI naming the class.

## 4 - Empirical half: the hostile battery at this commit

`torture.py --binary target/debug/cosmostrix` (debug profile:
overflow checks on, debug assertions on) at HEAD:

| Leg | Result |
|---|---|
| resize-storm | PASS |
| paste-wedge (the Z5a leg) | PASS quit-through-wedge rc=0 |
| signal-storm (SIGINT death, SIGTERM graceful) | PASS (rc -2 / rc 0, restore tails clean) |
| config-churn (22 rapid edits) | PASS (rc 0) |
| restart-storm | PASS (12/12 clean with restore tails) |
| suspend-resume | PASS (silenced stream, resumed repaint, deadline exit, four restore-tail expectations) |

Verdict line: `torture: verdict 7/7 legs passed`. CI-side corroboration
at the same SHA: the emission-floor leg drives the real pty stream
against the same certificate identity, the wakeup-floor leg pins the
one-wake-one-write budget, and the syscall-floor ledger closes the
whole output surface.

## 5 - Skipped follow-ups and why

- Layer-3 exhaustive optimality on live-size grids: entry 57 scopes
  the adversarial search to the 20x6 rig by design; exhaustive search
  over full screens is exponential and adds no teeth beyond the band
  the reference walk already enforces per push.
- The INV-22 label shared with entry 44 (afterglow): family-scoped
  INV convention, documented in entry 50's text; renumbering would
  churn four suites and the CI filter semantics for zero behavioral
  gain (see the hardening-5 report's skip rationale).
- Row-RLE batching internals and a concurrent resize+SIGSTOP leg:
  unchanged from zen-lts-1's rationale (the lock suite asserts the
  byte shapes; the axes are covered and Loom bounds the interleavings).

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
