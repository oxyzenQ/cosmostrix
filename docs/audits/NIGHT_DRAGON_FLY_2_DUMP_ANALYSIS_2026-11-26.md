<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# night-dragon-fly-2 — probe dump analysis and LTS verdict (2026-11-26)

Owner brief: the owner ran the night-dragon-fly-1 probe battery on the
local machine and pasted the raw terminal capture (5155 lines). This
document analyzes that dump, cross-checks the 18 round-1 findings, and
answers the owner's question: is the UX CLI surface already peak
masterclass LTS?

STATUS 2026-11-26 (same day): the fix batch below was APPLIED —
commits `459d921` (input-contract: bare `-mb`, attached-short
prevalidation, retired-short hints, `--bench-frames` 1e9 ceiling),
`74f281f` (baseline honesty + single error label + shared whitelist
summary + spinner clear + interactive no-op warnings), `6799f4b`
(help manual accuracy + doctor advice). Verified green: cargo
fmt/clippy/build, 3124 tests, the 35-check regression battery, and a
pre/post A/B benchmark at 80x24 (max metric delta 3.37%, visual gini
-0.06% / entropy +0.12% — render path untouched). The two QUESTION
items (EHS calibration, SGR cache hit rate) remain owner calls.

Capture context: local build `v100.1.3-dragon.1` (commit 11a4ecd,
local-pro-linux-amd64-v3-musl, build-time 2026-11-26 09:48Z) on CachyOS
6.18 LTS, Ryzen 7 5800HS, 88x33 xterm-direct truecolor terminal,
`sig` verdict AUTHENTIC (6/6 depths, GPG-signed HEAD). Sandbox
re-verification used the debug build of commit 68a4800 where needed.

## Dump coverage

Ran and captured: sections 0 (help + sig), 2 (config), 3 (diagnostics),
4 (benchmark + print-json + baselines), 5 (all 66 invalid-input
probes), 7 (20 interactive runs by hand), and the python JSON deep
check. Known gaps, none blocking: the first section-1 paste failed
(`probe: command not found` — the helper was never defined) and the
immediate rerun left no captured output; the section-6 env variants
(NO_COLOR/CLICOLOR/CLICOLOR_FORCE on piped help) produced no captured
output either; and the bulk paste interleaved echoes of later commands
into outputs of earlier ones in the 4130-4160 zone. The interleaving
is a terminal-capture artifact, not a product bug — verified by
re-running the two suspicious probes (`sig extra_token`,
`--bench-cosmetics`) on the sandbox binary, where both print exactly
as designed. Next round should run the one-shot capture script (file
redirect) instead of pasting into the interactive shell.

## What came back clean (masterclass-grade, live-evidenced)

- Signature: AUTHENTIC 6/6, exit 0, honest note about what the
  self-hash can and cannot prove.
- Suggestion engine: `--colour` -> `--color`, `--list-colours` ->
  `--list-colors`, `--LIS` -> `--list-scenes`, `-mfss` ->
  `--msg-fill-style (short form: -mfs)`, `--bench-scene leanax` ->
  value tip `'lean'` — every one fired.
- Removed-flag long forms all print version + rename hints; exit 2.
- Exit contract: every fatal CLI probe exited 2; `sig` 0; `--testconf`
  PASS 0; JSON single line, 19 sections all present, null serializer
  working.
- The benchmark text report's meaning/basis/caveat discipline held on
  every section; drift stable (+1.94%), state-digest IDENTICAL; the
  bench-all scaling table confirmed ns/cell O(n) (1129 -> 136 ns/cell
  from 6x6 to 200x60) with honest insights.
- NOOP warnings fired exactly as the help claims (`--duration` and
  `--bench-frames ignored (--benchmark takes precedence)`).
- Duration grammar strict and honest (`1h30` rejected with the
  `m/h/d/w — or a bare number` recovery line; `25h` hits the
  courteous-guest ceiling message).
- `--dump-config` overwrite guard reads masterclass (refuses,
  suggests `.new.toml`, offers `--force`); redirect refusal exit 2.
- `--check-update` honest on the prerelease-newer-than-latest state.
- Interactive: 144 fps pinned cadence over 12 s (avg 144.076, 1729
  frames, 0 overshoot), budget-pressure section explains scheduler
  overshoot vs renderer work, verbose dump complete (fps_source,
  fps_precedence, chroma_features, config candidates).
- Live evidence corrected two round-1 notes: `-m ""` is rejected by
  clap ("a value is required"), no empty overlay is possible; and
  `--screen-size 5000x3000` interactive is NOT silently clamped — it
  prints `size 5000x3000 exceeds terminal 88x33; will clip to
  top-left` and clips.

## Round-1 findings re-confirmed on the owner machine

| Round-1 # | Evidence in dump |
|---|---|
| 3 (double error prefix, baseline) | 3262: `error: error: cannot read baseline ...` |
| 4 (corrupt baseline false-pass) | 3029-3037: all N/A rows + `OK: no significant changes detected.` |
| 5 (broken relative example) | 2797: `--save-baseline 'base.json' is outside allowed directories` |
| 7 (13 sections claim) | 2097 + python check: 19 sections |
| 8 (PASS/FAIL claim) | 2342 `OK: no significant changes detected.`, 3494 `+ BETTER`, 3501 `+ 1 improvement(s) detected. No regressions.` |
| 11 (whitelist summaries disagree) | 1436-1441 full six-platform list (dump-config path form) vs 1447-1448 two-root (redirect refusal) vs baseline two-root |
| 12 (short-form hints dead) | 4093-4116: `-G/-r/-i/-a` get generic clap tips, no migration hint |
| 13 (RENDERER section name) | 1494: heading is `Renderer`; help says `See --doctor RENDERER` |

## New findings (round 2)

| # | Finding | Severity | Evidence | Status |
|---|---------|----------|----------|--------|
| 1 | Double `error: error: --config '/tmp/probe_dump.toml' is outside allowed directories` on the `--testconf` + bad `--config` path: `validate_config_path` embeds `error: ` in the Err string (src/safepath/mod.rs:530-537) and the testconf caller wraps it again (src/testconf/mod.rs:47-49, `eprintln_error_labeled`). The plain `--config` run prints the same message with a SINGLE prefix — one message, two printers, two shapes | LOW-MEDIUM | dump 1453 vs 1580; source sites above | Proposed fix: strip the embedded prefix in safepath and label exactly once at each printer |
| 2 | `--bench-frames` has no ceiling: the `--bench-frames 0` rejection exposes the parse range `1..=18446744073709551615` (full u64). A typo'd huge frame count would run unbounded — inconsistent with the courteous-guest 24h ceiling every time-based knob enforces | MEDIUM-LOW | dump 4146 fragment; sandbox re-run: `0 is not in 1..18446744073709551615` | Proposed fix: cap the value_parser range (or add a frame ceiling equivalent to 24h) + one help line |
| 3 | Piped `--doctor` prints `headless/non-TTY detected; use --benchmark, --doctor, ...` — recommending `--doctor` while running `--doctor` | LOW | dump 1706; src/doctor/mod.rs:501 | Proposed fix: reword or suppress for the doctor surface itself |
| 4 | Help default-overlay wording drift: help says the default overlay shows `cosmostrix v<VERSION>`; the actual default is `Experience with cosmostrix v<version>` | LOW | dump 318-319 (help) vs 4873/5049 (verbose) vs 1232-1234 (dump-config template) | Proposed fix: align help_text.rs with the shipped string |
| 5 | `--screen-size` help says `Max 1024x500 in interactive mode`, but live behavior accepts 5000x3000 with an exceeds-terminal warning and clips to the terminal top-left — the limit is a clip rule, not a cap | LOW (help accuracy) | dump 5129; src/interactive/event_loop_setup.rs:83-86 | Proposed fix: reword the help line; round-1 finding 15 is thereby REFINED (not silent) |
| 6 | EHS calibration question: two identical healthy 12 s / 144 fps sessions scored 59.6/100 (`investigate`) then 97.6/100 (`healthy`) | QUESTION (owner call) | dump 4990-4992 vs 5121-5123 | No verdict — needs the EHS formula's input diff before any change |
| 7 | SGR cache hit rate 12.1% (14K hits vs 101K misses) on a zen (1 glyph) truecolor session | QUESTION (owner call) | dump 5000-5002 | One look at the cache key granularity before LTS; may be by design |
| 8 | Error text printed while the bench spinner line is still active: `Running COSMOSTRIX BENCHMARK...error: ...` glued on one line | LOW (cosmetic) | dump 3970 | Proposed fix: clear the spinner line before emitting the error |
| 9 | Capture hygiene (not product bugs): bulk-paste echo interleaving hid the sig-extra tip and polluted the 4130-4160 zone; `sig extra_token` and `--bench-cosmetics` verified correct on the sandbox binary (tip + `required arguments were not provided: --benchmark`, both exit 2) | NOTE | dump 4827-4828; sandbox re-runs | Use the capture script next round |

## Verdict: already peak masterclass LTS?

Honest answer: **not yet — one fix batch away, no re-architecture
needed.** The core is genuinely peak-grade: the report discipline,
suggestion engine, guard UX, exit contract, signature, and interactive
telemetry all behaved exactly as documented across 80+ probes on the
owner's machine, and the two candidate anomalies from the dump
resolved to capture artifacts, not product bugs. What keeps the
surface below its own honest-help bar is a small, all-local cluster:
the three round-1 MEDIUMs (`-S12.5` bypass, bare `-mb` -> message `b`,
corrupt-baseline false-pass), the round-2 double-prefix and
bench-frames-ceiling items, and four help wording drifts. Every fix is
small and local; none touches the engine. After that batch plus the
round-1 hygiene sweep and a benchmark A/B (mandatory once code
changes), the UX CLI surface is defensibly peak LTS.

## Proposed fix batch order (next task)

1. argv_expand bare `-mb` fall-through -> `continue` (mirror `-mfs`).
2. Attached-short values (`-S12.5`) through the same prevalidation as
   long forms (or normalize them in argv_expand).
3. Baseline errors: strip the embedded `error: ` at the two
   `eprintln_error_labeled` call sites; make compare FAIL (distinct
   verdict + exit 2) when the baseline is unreadable or has no
   parseable metric keys.
4. Safepath/testconf: single-source the error label (strip the prefix
   in safepath, label once at each printer).
5. bench-frames ceiling + help line.
6. Help corrections: 13 -> real count, PASS/FAIL -> real vocabulary,
   drop `0.5d` from the bench-duration claim, absolute save-baseline
   example, default-overlay string, screen-size clip wording,
   `--doctor Renderer`, `--colors-custom` and `--bench-cosmetics`
   entries.
7. doctor/mod.rs:501 wording.
8. Round-1 hygiene sweep (stale comments, dead branches).
9. `cargo fmt` + `build.sh check-all -q` (CI parity) + 10 s benchmark
   A/B (density gini, frame_entropy, fps, dirty cells) — mandatory
   because this batch changes code.

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
