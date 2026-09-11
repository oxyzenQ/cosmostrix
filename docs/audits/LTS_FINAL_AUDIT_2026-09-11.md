<!-- SPDX-License-Identifier: GPL-3.0-only -->

# LTS Final Audit — S-night-R1 to R8 (combined pass)

Date: 2026-09-11
Scope: stability, performance, security, code health, dragon engine
harmony at commit 3df685f (HEAD after the R4 hardening commit; audit
baseline captured at d187afd, the NIGHT-hunt-32 HEAD).
Method: the eight S-night audit rounds merged into one pass, run in
order, with overlapping work merged where the same evidence serves
two rounds. Owner rules honored: 10 s A/B benchmark before and after
any change, no visual/performance regression beyond 1 percent, gate
keepers after each sub-task, micro-commit per logical change, skip
over-engineering when the code is already at peak.

## Verdict

One real defect found and fixed (R4 terminal escape injection in
diagnostic sinks, commit 3df685f). Everything else verified at peak
or already covered by documented contracts — zero over-engineering
applied. Full suite 2846 passed / 0 failed / 2 ignored (was 2841 at
the audit baseline; +7 tests: 5 escape_ctrl unit tests, 2
render_labeled_block regression locks, minus none). A/B 10 s flat on
every visual metric (max delta 0.13 percent, well inside the 1
percent contract). All three dragon engines re-locked with
signatures (see the three KEY.md files).

## R1 — Stability and Crash

- Full test suite: 2846 passed, 0 failed, 2 ignored (59 s).
- All 20 built-in scenes 2 s-benched on the release binary: zero
  panics, zero non-zero exits (matrix, vortex, flux, lorenz,
  cosmic_dragon, physarum, sorgonemous_intrascals, aeolian,
  solar_flare, dna_helix, murmuration, quasar, neural, monolith,
  signal, classic, cinematic, calm, storm, cosmos).
- Extreme geometries: 1x1, 2x2, 240x1, 1x80, 500x200, 2000x500 all
  exit clean — no index panics, no divide-by-zeros, no allocator
  blowups.
- Adversarial CLI values (density 999999, fps 1000, speed 100000,
  negative speeds, glitch 100, color-shift 5000, rain-length 0 and
  99999): every one rejected at clap parse time with the documented
  range message — parse-time bounds are complete.
- Signals: SIGTERM in interactive mode exits 0 with the full restore
  sequence (alt-screen leave, cursor show, SGR reset) verified by
  PTY byte capture. SIGINT dies with the OS default disposition
  WITHOUT restore — this is the documented bug #15 design decision
  (TERMINAL_KILL_CLEANUP.md, Ctrl-C deprecated section), not a
  defect; the muscle-memory protection lives in raw mode (ISIG
  disabled), and kill(1) users are told to use TERM.
- No coredumps produced by any probe.

## R2 — Code Hygiene

- scripts/stale-hunt.py: 24 stale references, all of them
  intentional historical documentation (rename history like
  "--disable-effects renamed to --no-effects in v50.0.0-beta"),
  plus two false positives where the tool does not model hidden or
  aliased flags (--charset-custom is live and verified working on
  the real binary).
- Duplicate comment groups: 218, all in the mirrored test tree
  (shared fixtures across the test/ mirror — the known, accepted
  pattern; the zombie audits A1-A5 and S1 already swept the src
  tree).
- dead_code allowances: 9 sites, each annotated with an owner
  directive or a test-reference justification (deprecated dragon
  constants, GLYPH_ENTRY_RAMP_DURATION_MS referenced by regression
  tests). Zero undocumented zombies.
- Verdict: already clean. No changes applied — removing documented
  history or mirrored fixtures would be churn without benefit.

## R3 — Optimization

- Frame budget at the probes: cinematic sim 0.019 ms + render
  0.015 ms; sorgonemous_intrascals sim 0.039 ms + render 0.005 ms;
  aeolian 0.007 ms total. Stability field reads "excellent" on all
  probes, p99.9 frame time 0.064 ms / 0.064 ms / 0.014 ms.
- Allocator: 563-564 alloc calls across an entire 10 s run
  (0.002 per frame), dealloc/realloc matching, heap retained 45 KiB,
  virtual 656 KiB — the steady-state zero-allocation contract holds.
- Drift: first-half vs second-half fps within 1.3 percent (stable).
- The engines are bit-stable locked (chroma at S-master-6-v2
  visual peak: "any further gain would change output bits").
  Verdict: at peak. Optimization changes would either alter output
  bits (lock violation) or be unmeasurable churn — skipped per the
  owner's explicit over-engineering rule.

## R4 — Security Hardening (the one real fix)

- Proved vector: a config value carrying a raw ESC byte (probe:
  scene-custom rain = "glyph<ESC>[2Jx") reached the terminal
  verbatim through the testconf and validation error echo
  ("testconf: scene-custom.probe.rain = glyph<raw-ESC>[2Jx"). On
  the shared-config threat model (a victim told to download a
  config.toml) this is terminal command injection: OSC 52 clipboard
  writes, screen clears, DSR reply spam.
- Sink inventory walked end to end: message overlay text is
  sanitize_message_text-guarded on all three intake paths (CLI
  -m/-mb, startup config, live-reload with the S3 mirror);
  charset-custom values reject control chars outright; custom block
  names are grammar-locked (ASCII alphanumeric plus dash/underscore)
  by is_known_key at parse; config paths are safepath-restricted to
  the five allowed directories; config reads are TOCTOU-safe and
  1 MiB-capped; unsafe code is confined to FFI boundaries already
  covered by UNSAFE_SOUNDNESS_AUDIT.md.
- Fix (commit 3df685f): new src/output/escape_ctrl.rs renders C0,
  DEL and C1 control characters as visible \u00XX literals (newline
  passes through as the line separator). Wired at five cold-path
  sinks: the labeled error/warning renderer (covers all 40 labeled
  call sites plus the die_input family), the suggestion line
  helper, the three verbose emitters, the live-reload fatal error
  echo and the post-exit debug trace drain. Clean input borrows
  unchanged on a fast path — every normal diagnostic line renders
  byte-identical to before.
- Also restored the SPDX header on scripts/nh32_crown_blink_audit.py
  (pre-existing header-check gap from the hunt-32 commit).
- Verification: full suite 2846/2846; end-to-end PTY probe on the
  release binary shows the probe value echoing as
  glyph\u001b[2Jx (visible literal, no escape execution);
  build.sh check-all -q and gate-keepers.sh 16/16 clean.

## R5 — LTS Stability

- Config lifecycle robustness (PTY probes on the real binary):
  config deleted mid-watch, deleted then recreated, replaced with
  an empty file — all three take the documented live-reload error
  path (exit 2, rejection message), zero panics, zero hangs.
- Live-reload stress (the hunt-31 harness, typo injection mid-run):
  documented reject-and-exit behavior, stable.
- Config writes are atomic (temp file + fsync + rename), readers
  can never see a partial file.
- Watcher thread termination and mutex poisoning both handled with
  runtime warnings, not silent loss.
- Panic hook: restore-before-print ordering, double-panic guard on
  closed terminals, worker-thread containment (notify watcher,
  heartbeat) — bulletproof by construction and by the hunter-4
  tests.
- Every time-scale input is capped at 24 h (the S-master-HUNT-5
  owner mandate, including the --bench-frames watchdog).
- Verdict: no hidden failure modes found. No changes needed.

## R6 — Chroma Dragon Integration

- 350 chroma tests green (0 failed), 184 lock-suite tests green,
  32 sweep-audit tests green.
- --doctor on a forced-truecolor terminal (TERM=xterm-256color,
  COLORTERM=truecolor) discloses the production pipeline:
  color_pipeline: chroma_dragon, detail "oklab gradient, perceptual
  blend, climate post-fx, head halo, l-smoothing".
- git log confirms zero chroma engine source changes this session;
  the last engine change remains the research-26 in-hue self-bloom
  cap round, which the 2026-09-02 data retune lock already covers.
- Verdict: integration real, working, production-ready at HEAD.

## R7 — Visual Impact (Chroma Dragon Focus)

- Re-verified at HEAD, zero code changes: 12/12 tuning constants at
  sweep-audit-verified sweet spots; all six dragon-engine-v2
  innovations live; zero steady-state allocations; A/B flat (see
  the comparison table below).
- Lock entry added to src/engine/chroma_dragon_engine/KEY.md
  (S-night-R7, signed oxyzenQ 2026-09-11).
- Verdict: the dragon stays LOCKED at visual peak. Further gain
  would change output bits or add unmeasurable churn.

## R8 — Three Dragons Harmony

- 117 crystal tests green; the cosmic side covered by the full
  suite and the 20-scene stability sweep.
- Dynamic 3-dragon probe (10 s PTY, truecolor, 120x40, all engines
  at defaults): exit 0, no panic, full terminal restore, 1.6 MiB
  ANSI stream, 9,381 distinct 24-bit SGR fg colors with zero
  256-color fallback — cosmic rain, chroma pipeline and crystal
  drift demonstrably running together with no interference.
- Harmony chain wiring unchanged since the S-master-7-v2
  verification.
- Lock entries added to all three KEY.md files (S-night-R8, signed
  oxyzenQ 2026-09-11).
- Verdict: harmony intact, no wasted resources, no conflicts.

## A/B Benchmark (10 s, the critical metrics)

Method note: the A/B sides must come from the same build pipeline.
The session's first AFTER comparison used the hunt-32 session's
build.sh binary as BEFORE versus a cargo build --release AFTER and
showed a 19 percent dirty-cell delta on sorgonemous_intrascals —
reproduced as a build-pipeline artifact, not a code change: a
fresh rebuild of the BEFORE commit (d187afd) via cargo build
--release in a separate worktree reproduces the AFTER numbers
exactly (dirty 113.1, entropy 5.431, gini 0.5601). The
build.sh-profile binary runs the black hole's phase cycle at a
different frame rate, so the 10 s window samples a different phase
mix. All numbers below compare same-pipeline builds (cargo build
--release, worktree d187afd vs HEAD 3df685f); raw JSON in
benchmark/bench-labs/lts_final_r1r8/.

| scene | side | avg_fps | p95_ms | dirty/fr | entropy_bits | gini |
|-------|------|---------|--------|----------|--------------|------|
| cinematic | A | 28,218 | 0.0490 | 458.98 | 5.1723 | 0.6373 |
| cinematic | B | 28,516 | 0.0505 | 458.40 | 5.1728 | 0.6372 |
| sorgonemous_intrascals | A | 23,270 | 0.0470 | 113.09 | 5.4310 | 0.5601 |
| sorgonemous_intrascals | B | 23,245 | 0.0472 | 113.08 | 5.4315 | 0.5600 |
| aeolian | A | 165,284 | 0.0075 | 35.32 | 4.7676 | 0.6836 |
| aeolian | B | 165,838 | 0.0074 | 35.33 | 4.7701 | 0.6829 |

Deltas: dirty cells max 0.13 percent, frame entropy max 0.05
percent, density gini max 0.10 percent — all far inside the 1
percent contract and at the reproducibility floor (same-binary
repeats show the same spread). fps deltas (+1.06 / -0.11 / +0.34
percent) sit inside the same-commit rebuild noise band (same-commit
rebuild pairs observed swinging 2-4 percent; p95 swings 4 percent
between builds of the identical commit). Allocator calls 563-564
both sides. Visual regression: none. Performance regression: none.

## Changed files (this audit)

- src/output/escape_ctrl.rs — new: the control-character sink guard
  with 5 unit tests.
- src/output/mod.rs — wired escape_ctrl into render_labeled_block,
  the suggestion line helper and the three verbose emitters.
- src/output/post_exit.rs — wired escape_ctrl into the live-reload
  fatal error echo and the post-exit trace drain.
- test/output/output_tests.rs — 2 regression locks (escape byte
  neutralized in rendered blocks; plain lines verbatim).
- scripts/nh32_crown_blink_audit.py — SPDX header restored.
- src/engine/chroma_dragon_engine/KEY.md,
  src/engine/cosmic_dragon_engine/KEY.md,
  src/engine/crystal_dragon_engine/KEY.md — S-night-R7/R8 lock
  entries.
- benchmark/bench-labs/lts_final_r1r8/ — A/B evidence JSONs.
- docs/audits/LTS_FINAL_AUDIT_2026-09-11.md — this document.

## Gates

- cargo fmt --check: clean.
- cargo clippy -- -D warnings: clean.
- ./scripts/build.sh check-all -q: exit 0 (all sub-checks pass;
  cargo-audit intentionally not installed per session policy — CI
  owns it).
- ./scripts/gate-keepers.sh: 16 passed, 0 failed.
- cargo test: 2846 passed, 0 failed, 2 ignored.

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
