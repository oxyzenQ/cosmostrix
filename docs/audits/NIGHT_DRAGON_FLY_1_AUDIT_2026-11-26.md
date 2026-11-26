<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# night-dragon-fly-1 — UX CLI total depth audit (2026-11-26)

Owner brief: end-to-end master depth audit of the UX CLI surface
(source code = truth), hunting print-json / UX / CLI / wording /
suggestion problems before LTS landing, plus a copy-paste probe manual
covering every surface advertised in `-h/--help` so the owner can
capture real local output for the follow-up improve round.

Companion document: [NIGHT_DRAGON_FLY_1_PROBE_MANUAL_2026-11-26.md](NIGHT_DRAGON_FLY_1_PROBE_MANUAL_2026-11-26.md)
(80 probes, sections 0-8; every `-h/--help` surface covered, safe
probes scriptable, interactive probes hand-run).

Prior art: [INTERNAL_RESEARCH_LTS_CLI_E2E_AUDIT_2026-11-01.md](INTERNAL_RESEARCH_LTS_CLI_E2E_AUDIT_2026-11-01.md)
covered the panic/unsafe/fd/memory lattice and 32 headless probes; its
three findings were already fixed. Everything below is NEW relative to
that audit. Verification tags: `live` = reproduced on the debug binary
(rustc 1.98.1, commit 68a4800); `code` = pinned by file:line reading.

## Findings

| # | Finding | Severity | Evidence | Status |
|---|---------|----------|----------|--------|
| 1 | Attached short values bypass the canonical-integer gate: `--speed 12.5` and `-S 12.5` are rejected ("canonical integer in range 1..=100") but `-S12.5` is accepted and runs at 12.5. `cli_spec` matches only exact tokens, so the clap attached-short path skips prevalidation. Same class applies to `-f1e2` (fps accepted at 100) and siblings | MEDIUM | src/validation/mod.rs:442-445 + main.rs:613; live (A2/A3) | Proposed fix: route attached-short values through the same prevalidator, or normalize them in argv_expand |
| 2 | Bare trailing `-mb` silently becomes the message text `b`: expansion pushes `--message-border` then falls through instead of `continue`, leaving the raw `-mb` token for clap to parse as `-m` with attached value `b`. The sibling `-mfs` bare case was handled deliberately; this one was missed. Live-verified: no parse error, interactive launch proceeds | MEDIUM | src/cli/argv_expand.rs:131-149 (fall-through), 95-107 (sibling), 30-31 (the module doc calls this class "a silent footgun"); live (A1) | Proposed fix: `continue` after the bare `-mb` push, mirroring `-mfs` |
| 3 | Double `error:` prefix on baseline failures: `bench_baseline.rs` embeds `error: ` in the Err string (lines 32, 45) and `bench_cosmetics.rs:107,120` forwards it through `eprintln_error_labeled`, which adds its own `error:` label. Rendered: `error: error: cannot read baseline '...'` | LOW | src/bench/bench_baseline.rs:32,45 + src/bench/bench_cosmetics.rs:107,120 + src/output/mod.rs:596-598; live (D1, D2, regression probe) | Proposed fix: strip the embedded prefix at the two call sites (or from the map_err strings) |
| 4 | Corrupt/malformed baseline JSON silently false-passes: `parse_json_flat` never errors, every metric row renders `N/A`, and the verdict prints `OK: no significant changes detected.` — a CI gate comparing against a corrupted baseline gets a green verdict. Missing-file errors also leave exit code 0 (bench runs anyway), so scripts cannot gate on compare failures | MEDIUM | src/bench/bench_baseline.rs:146-168, :43-45; live (D2: `{"broken":` → `OK: no significant changes detected.`) | Proposed fix: fail the comparison (or at minimum a distinct verdict line + nonzero exit) when a metric key is absent/unparseable in the baseline |
| 5 | The help example `cosmostrix --benchmark --save-baseline base.json` (relative path) is guaranteed rejected by the safepath absolute-path rule; the bench still runs and exits 0 after printing `--save-baseline 'base.json' is outside allowed directories`. Same relative example in the `bench_baseline.rs` module doc | LOW (doc bug) | src/cli/help_text.rs:622; src/bench/bench_baseline.rs:9-10; src/safepath/mod.rs:144-159; live (A6) | Proposed fix: document the whitelisted absolute form in both places |
| 6 | `--bench-duration` claims `0.5d` is accepted (help + clap help), but the u64 duration grammar cannot parse fractional numbers: `0.5d` → `missing unit after 0`. `0.5d` only works on `--duration` / `--crystal-dragon-secs` (the f64 parser) | LOW-MEDIUM (doc promises false) | src/cli/help_text.rs:583; src/config/mod.rs:368; src/cli/cli_parse.rs:252-326, 303-306; live (A4/A5) | Proposed fix: either state the integer-only grammar for --bench-duration or route it through parse_secs_f64 |
| 7 | The JSON claim "Mirrors the text report's 13 sections" is stale: the JSON emits 19 top-level sections (live-counted), the text report 20 always-on + 3 conditional; and `--bench-all --json` emits a different schema entirely (scaling ladder), undocumented. `bench_helpers.rs:276` comment carries the same stale 13 | LOW | src/cli/help_text.rs:594-595; src/bench/bench_json.rs:71-76; live (C2: 19 sections) | Proposed fix: reword to "mirrors the text report's section set" or pin the real counts; document the bench-all schema |
| 8 | The compare claim "Marks >5% FPS regressions with a clear PASS/FAIL verdict" matches no real string: actual per-row statuses are `OK` / `+ BETTER` / `- WORSE` and verdicts are `! N REGRESSION(S) detected` / `+ N improvement(s) detected. No regressions.` / `OK: no significant changes detected.`; the threshold fires at >= on absolute percentage and covers 7 metrics, not only FPS | LOW (wording) | src/cli/help_text.rs:625; src/bench/bench_baseline.rs:20-28, 123-129, 163-167; live (forced-regression probe: `! 2 REGRESSION(S) detected`) | Proposed fix: reword the help line to the real vocabulary |
| 9 | Save/compare-baseline failures exit 0 and the bench still runs to completion (state-digest printed). Honest output, but scripts cannot gate on it and nothing documents the policy | LOW | src/bench/bench_cosmetics.rs:98-121; live (A6, D1) | Proposed fix: document, or exit 2 on baseline I/O failure (owner call) |
| 10 | `--save-baseline` silently overwrites an existing file (plain truncate-write; no refuse-with-suggestion guard like `--dump-config`, no `--force` requirement). The `--force` help says other writes "have their own per-option overwrite policy" without stating what it is | LOW | src/bench/bench_baseline.rs:31-33; src/cli/help_text.rs:526-527 | Proposed fix: state the always-overwrite policy in --save-baseline help (or adopt the dump-config guard; owner call) |
| 11 | Baseline rejection message under-lists the whitelist: `Allowed: ~/.config/cosmostrix/, /etc/cosmostrix/` while the real safepath set also includes XDG, `/usr/local/etc/`, Termux, macOS, Windows roots. The `--dump-config` refusal + its help section carry the same two-root summary while the `--config` help section lists the full set — the help disagrees with itself | LOW (wording) | src/bench/bench_cosmetics.rs:100-103; src/safepath/mod.rs:190-300; src/cli/early_returns.rs:295-296; src/cli/help_text.rs:464-471 vs 485-486; live (A6) | Proposed fix: route all three summaries through one safepath-provided string |
| 12 | Removed-flag hints advertise short forms (`--glitchpct / -G`, `--rippct / -r`, `--info / -i`, `--async / -a`) but the matcher only inspects `--`-prefixed tokens, so `-G 5`, `-r`, `-i`, `-a` never trigger the migration hint — users get clap's generic suggestion (`-G` → "did you mean --msg-mode") instead | LOW | src/validation/mod.rs:157,165,153,129 vs :211; live (A7/A8) | Proposed fix: add short-form tokens to the matcher or drop the short forms from the hint texts |
| 13 | "See --doctor RENDERER for the field-level declaration" — the doctor renders the section as `Renderer` (exact-case grep finds nothing), and the `--check-bitcolor` hint points at a `COLOR` section that does not exist (color fields live under Renderer/Terminal) | LOW | src/cli/help_text.rs:751; src/validation/mod.rs:119; src/doctor/mod.rs:112; live (D3: RENDERER=0 hits) | Proposed fix: reference `Renderer` or make the section names match the help |
| 14 | `--json` without any bench flag is a silent no-op: interactive rain starts with no warning (only the `--bench-frames` combination warns). Same silent-drop applies to `--save-baseline`/`--compare-baseline` under `--bench-all` and `--bench-frames` | LOW | src/bench/bench_helpers.rs:245-307 (no entry for these combos); src/bench/bench_scale.rs:94-95; live (A9) | Proposed fix: add noop-warning entries, matching the codebase's own no-silent-failure contract |
| 15 | `--screen-size 5000x3000` in interactive mode is silently clamped to 1024x500 with no stderr notice (bench mode uses values under its 8K cap as-is). UPDATE night-dragon-fly-2: the live owner capture shows the run is NOT silent — it prints `size 5000x3000 exceeds terminal 88x33; will clip to top-left` and clips to the terminal; the remaining issue is the help's `Max 1024x500 in interactive mode` wording misdescribing a clip rule as a cap | LOW | src/interactive/event_loop_setup.rs:83-86; owner dump line 5129 (NIGHT_DRAGON_FLY_2_DUMP_ANALYSIS_2026-11-26.md) | Proposed fix: reword the help line to the clip behavior |
| 16 | `--colors-custom` (a visible, non-hidden arg in the "Common options" clap heading) has no entry in the curated `--help` manual — only an oblique mention inside `--intro-color`. Same gap for `--bench-cosmetics` (visible, "Diagnostics" heading). Both contradict the manual's own honest-disclosure posture | LOW (doc) | src/config/mod.rs:108-115, 448-459; src/cli/help_text.rs (no matches) | Proposed fix: add both entries (Common options / Diagnostics) |
| 17 | Hygiene cluster — stale comments and dead branches: `resolve_bench_duration` claims "No max cap" while the 24h ceiling lives in the parser and its below-minimum arm is unreachable (bench_helpers.rs:360-368); screen-size parser docstring says "4x4" while MIN is 1x1 and the too-small branch is dead (cli_parse.rs:358-359, 403-409); `bench_baseline.rs` docstring promises dot-flattening that is not implemented and the `config.cols`/`config.lines` lookups can never match bare keys (bench_baseline.rs:59-64, 189-190, 225); early_returns header omits the sig exit-1 (unsigned) code (early_returns.rs:17-18); `--color-mode` clap value hint `<0|16|256|24>` omits accepted `8`/`32` (config/mod.rs:764) | LOW (hygiene) | file:line as listed | Proposed fix: one hygiene sweep |
| 18 | Verified-intentional, documented only: `--crystal-dragon yes` accepted via the 8-alias bool vocabulary while the value hint says `<true|false>` (live; prior audit's disposition stands); `--bench-scene lean` without `--bench-io` is a silent no-op while `production-draw` without it errors (asymmetric strictness); `--bench-replay` outranks `--bench-frames` in dispatch but the help precedence ladder omits it (hidden flag); the replay canary digests run even on the `--json` path where their output is suppressed; `-m ""` renders an empty overlay with no guard — UPDATE night-dragon-fly-2: the live owner capture corrects this item, clap rejects the empty value with `a value is required for '-m <MESSAGE>'`, so no empty overlay is possible | LOW (notes) | src/cli/ux.rs / argv_expand.rs; src/bench/premium.rs:102,416, 784-786; src/bench/dispatch.rs:65-73; live (C6) | No action without an owner directive (no over-engineering) |

## What came back clean (live or code-verified)

- `--help` manual: layout contract intact, piped output ANSI-free, `NO_COLOR`/`CLICOLOR`/`CLICOLOR_FORCE` gating correct.
- Exit-code policy holds everywhere probed: fatal CLI errors exit 2 (invalid values, whitelist, removed flags, `sig` extra token, bench-scene typo), `--testconf` 0/2, `sig` 0/1/2, headless interactive fail exit 1 with the alternatives tip.
- JSON contract: single line, 19 sections present, NaN/Inf -> null serializer correct.
- Suggestion engine: `-mfss` -> `--msg-fill-style` tip, unknown long-flag case-insensitive rescue, value suggestions for enum/scene/charset/color names — all fire as documented.
- Removed-flag table: all 34 long-form names produce their migration hints (only the advertised short forms are dead, finding 12).
- Duration grammar: `5`, `6s`, `30m`, `1h30m`, `90m`, `1w`, `-5`, `NaN`, `1e9` all handled with precise, honest error wording; only the `0.5d` claim for `--bench-duration` is false (finding 6).

## Verdict

No LTS blocker found: no crash, no leak, no exit-contract breach in the
probed surface. Two MEDIUM input-contract bugs (findings 1-2), one
MEDIUM false-pass CI hazard (finding 4), and a cluster of
wording/doc-drift items that matter for the "masterclass honest help"
bar the project sets for itself. All fixes are small and local; the
probe manual is the regression harness for the improve round.

## Verification commands

```
cargo build
target/debug/cosmostrix --benchmark --bench-duration 2 --screen-size 80x24 --json   # 19 sections, single line
target/debug/cosmostrix -S12.5          # accepted (bug 1)
target/debug/cosmostrix --speed 12.5    # rejected (contract)
target/debug/cosmostrix -mb             # accepted silently (bug 2)
target/debug/cosmostrix --bench-duration 0.5d   # rejected despite help claim (bug 6)
```

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
