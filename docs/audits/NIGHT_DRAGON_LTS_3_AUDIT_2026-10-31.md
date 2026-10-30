<!-- SPDX-License-Identifier: GPL-3.0-only -->

# night-dragon-lts-3 / lts-4 audit — sharpest-skill depth, all-infra coverage: stability, hygiene, optimization, security, LTS

Owner brief (2026-10-31): "night-dragon-lts-4: depth audit focus for all
infra. night-dragon-lts-3: depth audit focus for sharpest skill.
Ultimate total LTS, reliable all round and keep be honest. Cross-check
audit all root repo focus; if peak skip and continue the next step."
Scope: the five-rubric program over the active corpus (308 `.rs` files
under `src/`, ~110K LOC; engine-heavy: `cosmic_dragon_engine` alone is
1.9 MB), executed per-stage with targeted scans instead of blind
full-repo sweeps.

This round CHANGED code — three fix commits plus this report. Prior
LTS rounds that found nothing proved the estate was already hardened;
this round pushed past the estate into the residuals the earlier
verdicts left standing, and removed them.

## Method

Three independent evidence sweeps, every candidate finding opened in
context before a verdict, prior audit record consulted to avoid
re-proving settled questions (LTS_FINAL, NIGHT_DEPTH_HUNT_1,
NIGHT_ULTIMATE_1, OVERFLOW_ENDURANCE, UNSAFE_SOUNDNESS,
NIGHT_LONG_HORIZON_1, NIGHT_DRAGON_LTS_1/2):

1. Panic-surface classifier — brace-aware `#[cfg(test)]`-region
   tracking over unwrap/expect/panic/unreachable/assert/division/
   recursion candidates across engine, config, platform, output, cli,
   scene, interactive.
2. Duplicate/zombie detector — normalized function-body hashing across
   the three engines, every `#[allow(dead_code)]` read in context, all
   62 CLI fields and all config keys consumer-checked, 86 zero-ref
   candidates individually caller-verified.
3. Security/LTS surface — full unsafe inventory, input-surface
   validation audit (CLI, config, env, /proc, terminal), command-exec
   and write-path review, concurrency and failure-mode sweep.

## What changed because of this audit (4 commits, pushed)

1. `Internal research: night-dragon-lts-3 defuse the last production
   panic foot-guns` — the `(None, None) => unreachable!()` in
   `config/live_config_trace.rs` (the only production panic-family
   macro in the audited tree) defused to a silent skip;
   `crystal_dragon_engine/point_system::cdf_select` contract pinned
   with a `debug_assert` plus `saturating_sub` replacing the
   underflow-capable `len() - 1` belt.
2. `night-dragon-lts-3 dragon hunt removes verified zombies and stale
   claims` — `Cloud::set_async` deleted (repo-wide zero-caller proof;
   `async_mode` is constructor-fed and read once, so the setter was
   dead wiring by construction); the zero-referenced
   `DRAGON_ACTIVE_BASE/_DENSITY_MULT/_MAX` trio deleted (the owner
   remove-zombie directive is the mandate NIGHT_LONG_HORIZON_1 was
   waiting on); `theme_edit_distance` consolidated into the shared
   `cli::suggestion::edit_distance` (byte-identical copy, stale
   justification comment); the Cargo.toml lint-policy comment no
   longer points at the deleted `cloud/monolith_tests.rs`; the sRGB
   transfer twins gained mirror notes instead of a forced merge.
3. `night-dragon-lts-3 white-hat hardening closes the last input
   amplifiers` — 128-byte input cap with `usize::MAX` sentinel in the
   shared Levenshtein engine (the O(m*n) DP previously ran over raw
   config strings; all four call sites compare `dist <= 2`, so the
   sentinel degrades to "no suggestion");
   `write_config_atomic` creates its temp with O_EXCL (symlink-follow
   on the predictable `<name>.tmp.<pid>` closed; stale-temp pid-recycle
   handled by unlink-and-retry; fsync now rides the written handle);
   `--compare-baseline` routed through the shared 1 MiB `Read::take`
   cap (the last unbounded file read);
   `COSMOSTRIX_BENCH_WARMUP_SECS` clamped to 3600 s.
4. This report.

Verification per commit: `cargo check --workspace --all-targets` clean,
`cargo fmt` clean, `cargo clippy --workspace --all-targets --
-D warnings` clean, 110 targeted unit tests pass (0 failures), full
`gate-keepers.sh` 20 passed / 0 failed, and a 10 s A/B benchmark
(base `c69b7ac` binary vs HEAD binary, 80x24 lean/blocks): density
0.85 vs 0.85, avg_fps 7744.6 vs 7736.6 (run noise at ~7.7K fps),
dirty cells 94.4 vs 94.2, p99 frame time 0.185 vs 0.169 ms,
peak_rss 9.02 vs 8.95 MiB, `frame_time_stability: excellent` both,
exit 0 both. No visual or performance regression.

## The five rubric dimensions

### 1. Stability and crash — production panic surface verified empty; two latent foot-guns defused

Aggregate over production code (test regions excluded by brace
tracking, which cuts raw grep noise ~20x): 0 runtime-failing
`unwrap()`, 26 `expect()` all provably-safe (24 `Uniform::new*` on
constant or guard-validated ranges, 2 proven-`Some` invariants),
1 `unreachable!` (defused this round), 0 `todo!`/`unimplemented!`,
all 23 production `assert!`s are compile-time `const _: () =`
checks that fail the build, never the runtime. Every integer
div/mod site in the hot paths is guarded (`max(1)`, `clamp`,
`is_empty` early-returns); the only unguarded divisions are three
f64 exit-report stats that cannot panic. Zero unbounded recursion
(168 heuristic candidates all read and cleared). The render hot path
is allocation-free: 11 `.clone()` in 47K LOC of engine, none
per-frame; `dirty_indices()` returns a borrowed slice; ANSI buffers
are reused across frames. **Engine verdict: PEAK.** Stress evidence:
10 s bench clean with excellent frame-time stability; the fuzz lane
proves the escape sink; the remaining residual risk class is a
future copy-paste of the `(scan_idx + step) % len` free-slot template
without its `len == 0` early-return — the template is now documented
in this report so reviewers grep for it.

### 2. Code hygiene — dragon hunt: three verified zombies and one drifted claim removed; the rest is intentional mirroring

TODO/FIXME/XXX/HACK/WIP: zero in `src/`, `scripts/`, `build.rs`.
Commented-out code blocks (3+ lines): zero. `#[deprecated]`: zero.
All 62 clap fields and all bare + namespaced config keys have live
consumers; removed keys are enforced rejections with migration
messages, not silent zombies. Duplication: the large near-identical
families (`draw_*_cell` x9, `step_down_level` x8, `push_trail` x7,
the chroma legacy/palette bit-identical twins) carry explicit
mirror/parity-contract comments and are intentional per-family
contracts — flagged in the report, not merged. The one real drift
(a byte-identical copy with a false "it's private" justification)
was consolidated this round. `CrystalDragonCalcMethod::Calc` (legacy
calc-v1) and `Cloud.profile` (future-reserved slot) are documented
deliberate keeps — adjudicated, left alone. Verdicts: cosmic engine
and chroma OK (now cleaner), crystal engine and output and scripts
PEAK.

### 3. Optimization — skipped as peak; no profiling-backed finding justifies touching the hot path

The engine's draw path already rides reused run buffers, borrowed
dirty lists, and clamped frame bounds (1024x500); the A/B shows
identical density and dirty-cell counts with fps differences inside
run-to-run noise at ~7.7K fps (debug build, 80x24). The repo's own
bench records (BENCH_LABS, BENCHMARK_ADVANCED) show the release
shapes far from any user-observable ceiling, and the PGO recipe
exists as a local capability for owners who want the last percent.
Micro-optimizing without a profiler trace pointing at a real
regression would be churn risk with no measurable payoff — the
explicit LTS trade the owner asked for (stability over performance
edge) is already embodied in the shipped matrix. **Verdict: PEAK by
discipline, not by exhaustion; revisit only with a profile showing a
hot loop.**

### 4. Security hardening (white-hat pass) — unsafe inventory 100 percent justified; four amplifiers closed; one agent suggestion rejected on documented evidence

Every production `unsafe` site (29 across fork_guard, root_guard,
bench_perf, reclaim_state, config_io, sysstat, watchdog, terminal_tty,
posix_time, info, alloc_trace) carries a SAFETY comment and a
non-replaceable reason (libc-only FFI policy; no datetime crate; the
anti-deadlock raw-write contract). No transmute, no raw-pointer
tricks, no stack escapes, no `/tmp` writes anywhere in the tree.
Input surfaces: CLI (clap + range validation), config (1 MiB cap,
whitelist paths, traversal-normalized), env (clamped numerics),
`/proc` (fixed-buffer, prefix-strip, never panics), terminal input
(clamped dims, paste-burst suppression) — all PEAK after this
round's caps landed. Command execution: fixed argv everywhere, no
shell interpolation. **One finding from the security sweep was
rejected after verification**: replacing the raw `libc::isatty(1)`
in the watchdog with `std::io::IsTerminal` — the NIGHT-termux-hang
comment at the site documents that std's probe futex-wedged the
watchdog on the stdout mutex during a jammed-PTY write, which is
exactly the failure the watchdog exists to escape. The unsafe there
is the fix, not the bug. Fuzz lane: 1 of 3 identified hostile-input
surfaces covered; extending it needs a lib-target extraction (a
structural decision that belongs to the owner) — noted, not done.

### 5. LTS stability — the classic killers are each engineered against; verified, not trusted

Mutex poisoning: zero `lock().unwrap()` in production; the pattern is
uniformly poison-tolerant (`if let Ok`, `into_inner`, degrade-and-
skip). Channels: bounded `sync_channel(64)` with `try_send` +
deadline-bounded retry-then-drop. Threads: fixed startup population,
every loop has a shutdown flag, signal thread bounded to a 3 s grace.
Backpressure: single-threaded fps-paced loop, adaptive load shedding,
O_NONBLOCK raw-write force-exit plus 1 Hz dead-PTY probe; the
watchdog kill threshold scales with fps so slow settings are not
false-killed. Terminal restore on panic/kill: five independent
layers (panic hook, Drop guard, fork PDEATHSIG/getppid, watchdog,
`--reset-terminal`). Clock math: `saturating_duration_since`
pervasive; durations validated before construction. This round
removed the last two LTS-relevant residuals (unbounded baseline
read, uncapped warmup sleep). **Verdict: PEAK.**

## What would falsify this verdict

- A new `unwrap`/`expect` on genuinely fallible IO or external input
  in production paths, or a `%`/`/` over a caller-supplied divisor
  without a guard.
- A new engine code path that clones or allocates per frame (the
  allocation-free claim is the foundation of the perf verdict).
- Any `Command` usage that interpolates user data into argv, or a
  file write to a predictable `/tmp` name (both absent today).
- A copy of the free-slot `% len` scan template (or any indexed
  helper) landing without the empty-guard the siblings carry.
- Fuzz targets added without first resolving the bin-only crate
  constraint would silently cover nothing — the fuzz-lane gap is
  known; half-fixing it must not read as fixed.

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
