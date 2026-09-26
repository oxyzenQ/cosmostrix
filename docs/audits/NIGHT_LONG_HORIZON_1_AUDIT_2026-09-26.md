<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-long-horizon-1 audit — the five-phase long-horizon depth audit: stability, hygiene, optimization, security, LTS

Owner brief (2026-09-26): "depth audit focus for LTS and keep be
honest. Cross-check the whole root repo; if a phase is already at
peak, skip it and record why." Scope: the five requested phases over
the full active corpus (306 `.rs` files under `src/`, 218 live `.md`
files, all gate tooling), executed per-stage with targeted scans
instead of full-repo sweeps (the 181K-LOC tree makes blind scans a
context hazard).

Commit: this round (audit + report only — zero engine surface
touched; no benchmark applies per the docs-only rule).

## Methodology

Every phase combines three evidence classes: (1) fresh scripted
scans run this round — a brace-aware production panic-site analyzer
(tracks `#[cfg(test)]` scopes through brace depth, so a site only
counts as production when it is provably outside every test scope),
a normalized-body cross-file duplicate detector (comment-stripped,
whitespace-collapsed function bodies, 8-line minimum so trivial
getters are excluded by design), and the repo's own standing audit
suite (docs-audit, stale-hunt, visual-mode-audit, emoji-audit,
language audit); (2) targeted code reads of the flagged sites —
every candidate finding was opened in context before any verdict;
(3) the prior audit record (LTS_FINAL, NIGHT_DEPTH_HUNT_1,
NIGHT_ULTIMATE_1, OVERFLOW_ENDURANCE, DOCS_LTS_COMPLETENESS) to
avoid re-proving settled questions.

One methodological note recorded for future auditors: a suspected
syntax error in `chroma_dragon_engine/legacy.rs` (`#ust_use]`,
apparently missing `[m`) was debunked by byte-level verification —
`od -c` shows the file contains the correct `#[must_use]`; the
corruption was a display-layer artifact, not file content. The
lesson stands: never report a byte-level defect without a
byte-level check (hexdump before verdict).

## Verdict by phase

| phase | verdict | evidence summary |
|---|---|---|
| 1. Stability & crash | AT PEAK | zero raw production `unwrap()` (brace-aware, 306 files); 27 `expect()` sites all carry documented invariant reasoning; 49 `unsafe` sites all SAFETY-documented; 123 defensive-arithmetic sites vs 38 deliberate wrapping sites (hash mixing, generation counters); every `cols-1`-class subtraction sits inside `.min()`/`.clamp()` guards; 69 `saturating_sub` sites in the cloud engine alone; `panic = "unwind"` keeps terminal restore reachable; repo audit suite green (docs 0/0/0, stale 0, emoji 0, visual pass) |
| 2. Code hygiene | NEAR-PEAK (by design) | zero commented-out zombie blocks (5+ line code-comment scan: 0 hits); 19 `allow(dead_code)` all documented (platform-conditional, test-referenced, or owner-directed deprecated constants); 9 cross-file duplicate groups all consistent with the self-contained-subsystem architecture (`src/RULES.md` §Maintainability) |
| 3. Optimization | AT PEAK — skip | zero per-frame heap allocations in the two hottest files (frame.rs, terminal/draw.rs: no `String::new`/`format!`/`Vec::new`/`to_string`/`to_owned` in the render loop); pre-sized `ByteWindow::with_capacity` encoding windows; documented 0.002 allocs/frame with the alloc tripwire; PGO runner infra; per-change 10 s A/B discipline; prior verdicts "early-return paths already at peak" |
| 4. Security hardening | AT PEAK | zero env-var writes; process execution limited to unix terminal-restore and one test-only simulation; whitelist-only path security (`src/safepath/mod.rs`); strict config validation (exit 2 on unknown/duplicate/malformed keys, no silent fallback); locked `Cargo.lock` + `deny.toml` in CI; GPG-signed releases with three checksums; 49 SAFETY-documented `unsafe` + the Miri soundness methodology |
| 5. LTS stability | AT PEAK | ignored-result sites are exit-path stderr/stdout writes (correct by design — nothing to recover when stderr is dead); watchdog + self-healer (P1 auto-downgrade, P2 full redraw); endurance health subsystem; degenerate-viewport tests (20x6); live-reload limitations documented honestly (§8 breakdown); API freeze from v100; pinned toolchain with dormancy guide |

## Findings considered and deliberately not changed

Honesty requires recording what was found and why it stays:

- **Four deprecated constants** (`DRAGON_ACTIVE_BASE`,
  `DRAGON_ACTIVE_DENSITY_MULT`, `DRAGON_ACTIVE_MAX` in
  `central_control_rains/style_rain.rs`; `LOGO_COLOR_RGB` in
  `chroma_dragon_engine/intro_colors.rs`) are `#[allow(dead_code)]`
  by explicit owner directive ("DEPRECATED by NIGHT-research-5 ...
  kept for compatibility", "Historical reference ... kept for
  reference"). They are `pub(crate)` internals with zero external
  API impact, so removal is safe in principle — but they are the
  owner's documented keeps. Registered here as candidates for the
  next owner-approved cleanup window (v101) rather than removed
  unilaterally by an agent audit.
- **Scene-family helper duplication** (`step_down_level` in 8
  scene families, `find_inactive_mote` x3, `find_inactive_drop` x3,
  `adopt_palette_slot` x3, `rank_level`/`level_rank`/`cell_of` x2
  each) is the cost of the self-contained-subsystem rule: each
  `type_rain/<scene>/` module is independently maintainable with
  no cross-scene trait coupling. The copies are currently
  byte-identical (verified by normalized hashing), the drift risk
  is real but bounded (pure 9-line match helpers, each covered by
  its scene's test family), and consolidation would trade ~80
  lines for new cross-scene coupling in a frozen-visual era. The
  dual `boost_rgb` (legacy.rs + palette/mod.rs) is separately
  documented as a P11 migration decision with the unification path
  (OKLab L lift) explicitly deferred to owner approval.
- **`overflow-checks = false` in release profiles** is standard
  release practice; the overflow-sensitive surfaces use
  saturating/checked/wrapping calls explicitly (see phase 1
  counts), so the profile setting is not a latent hazard.

## Tooling

The audit is reproducible: the two fresh analyzers (production
panic-site scanner with brace-aware test-scope tracking; normalized
cross-file duplicate detector) were persisted as session scripts
and the repo's own standing suite
(`scripts/audit/docs-audit.py`, `stale-hunt.py`,
`visual-mode-audit.py`, `emoji-audit.py`) ran green this round.
The full `build.sh check-all -q` gate completed locally within
the owner's two-minute budget (exit 0, unpiped; warm target cache
— fmt, clippy, the test suite, and every gate stage green);
cargo-audit was not installed locally per the owner's
tooling-budget rule and stays owned by CI.

Gates for this commit: gate-keepers 19/19, build.sh check-all -q
exit 0, cargo fmt --check clean.
Doc-only change: zero engine surface, zero shipped-binary impact;
no benchmark applies.
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
