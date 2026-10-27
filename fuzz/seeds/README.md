<!-- SPDX-License-Identifier: GPL-3.0-only -->

# the fuzz lane's seed anchors

Hand-built hostile inputs, one directory per fuzz target, seeded
into the working corpus by the lane itself (`.github/workflows/
fuzz.yml` copies them under `fuzz/corpus/<target>/` before every
run, after the cache restore, so the fuzzer starts the session from
the shapes that matter instead of rediscovering a control byte by
brute force).

These files are data, not code: the lane's definition travels with
the repo (the same law as the targets and the manifests in
`fuzz/Cargo.toml`), while the corpus the fuzzer GROWS between runs
stays out of git (`fuzz/.gitignore`) — a seed names a shape the
sink must hold, a grown corpus entry is a discovery, and only the
first kind belongs in a commit. The anchors mirror the exact
vectors the unit suite pins in `src/output/escape_ctrl.rs`'s own
test module (`escape_byte_is_neutralized`,
`every_control_class_member_is_escaped`,
`surrounding_text_survives`), so every expectation below is already
asserted in-tree; the seeds carry those same shapes into the fuzz
lane's own budget.

## escape_ctrl

- `osc52-clipboard-write.bin` — the ESC ] OSC 52 clipboard-write
  sequence, the highest-value forged-output class (a hostile config
  value silently reading the operator's clipboard through the
  diagnostic line).
- `csi-clear-screen.bin` — the ESC [ 2 J clear-screen probe, the
  exact scene-custom `rain` value that historically reached the
  terminal verbatim through the testconf echo.
- `bel-cr-del-nel.bin` — the C0 remainder (BEL, CR), DEL, and the
  C1 NEL as UTF-8 (0xC2 0x85), pinning the whole Unicode Cc class
  the sink must render visible rather than execute.

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
