<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# The Proof Atlas

One page, every innovation, its proof. The [Innovation
Ledger](INNOVATIONS.md) is the map of WHAT the machine does; this
atlas is the map of HOW EACH CLAIM IS PROVEN — the test file, the CI
lane, the bench-labs record, or the audit doc that pins it. A buyer,
an acquirer, or an AI evaluator reads this page and walks the whole
evidence chain in one sitting; every link below is alive (the
docs-audit gate sweeps relative refs on every push) and every claim
without a live artifact is MARKED, never inflated — an atlas row
inherits its number's truth from
[INNOVATIONS.md](INNOVATIONS.md), and the source tree outranks both.

## The honesty law

- Every row names at least one artifact that exists in the tree
  today; the docs-audit gate fails the push if a link rots.
- A row whose strongest evidence is a CI lane names the lane and the
  verdict line to look for.
- Where evidence is historical (a dated research doc, a bench-labs
  A/B record), the row says so — a snapshot is evidence of the past,
  labeled as the past.
- The atlas is docs-only by design: no version bump rides it, no A/B
  campaign applies, and it must never become the truth itself —
  `src/` is the truth; the atlas only routes to it.

## The atlas (entries 1-47 and 49-59)

| # | The innovation | Proof artifact(s) | The verifier |
|---|---|---|---|
| 1 | The diff-based render path (the Cosmic Dragon) | [frame.rs](../src/engine/cosmic_dragon_engine/frame.rs), [draw.rs](../src/engine/cosmic_dragon_engine/terminal/draw.rs), [last_frame.rs](../src/engine/cosmic_dragon_engine/terminal/last_frame.rs) | `cargo test cosmic_dragon_incubator::tests::lock` |
| 2 | The output path (row-RLE + pre-formatted SGR) | [draw.rs](../src/engine/cosmic_dragon_engine/terminal/draw.rs), [bolt/](../src/bolt/), the `escape_ctrl` fuzz target | Chroma lock INV-14/15 + the [fuzz lane](../.github/workflows/fuzz.yml) |
| 3 | The three-layer parallax depth system | [droplet/draw.rs](../src/droplet/draw.rs), [central_control_rains/](../src/central_control_rains/) | the `test/central_control_rains/` seat |
| 4 | Phosphor persistence (CRT afterglow) | [phosphor.rs](../src/engine/cosmic_dragon_engine/cloud/phosphor.rs), [phosphor_anomaly.rs](../src/engine/cosmic_dragon_engine/cloud/phosphor_anomaly.rs) | the `tests_phosphor*.rs` cloud seats |
| 5 | The living rain field (value-noise density, gusts) | [living_rain.rs](../src/engine/cosmic_dragon_engine/cloud/living_rain.rs), [rain_at.rs](../src/engine/cosmic_dragon_engine/cloud/rain_at.rs) | [RENDER_ENGINE.md](RENDER_ENGINE.md) + the PTY lanes |
| 6 | The border-touch family (glow pulse) | [border_touch.rs](../src/engine/cosmic_dragon_engine/cloud/border_touch.rs), [tests_border_gradient.rs](../test/engine/cosmic_dragon_engine/cloud/tests/tests_border_gradient.rs) | the border-touch cloud seats |
| 7 | The quantum particle pool (removed) | the design records: [archive/specs/QUANTUM_RIPPLE_MOUSE_CLICK_BURST.md](archive/specs/QUANTUM_RIPPLE_MOUSE_CLICK_BURST.md) + [archive/specs/BORDER_SPARK_SPLASH_CROWN.md](archive/specs/BORDER_SPARK_SPLASH_CROWN.md) | removed in night-moonlight-1 — no live verifier; the mouse click now drives only the wave-equation shockwave |
| 8 | Ghost-kanji cinematic events | [ghost_events.rs](../src/engine/cosmic_dragon_engine/cloud/ghost_events.rs) | [RENDER_ENGINE.md](RENDER_ENGINE.md) + the PTY lanes |
| 9 | The entropy drift ecosystem | [ecosystem.rs](../src/engine/cosmic_dragon_engine/cloud/ecosystem.rs) | [RENDER_ENGINE.md](RENDER_ENGINE.md) + [ENDURANCE.md](ENDURANCE.md) |
| 10 | Smooth pause (the exponential-decay easing family) | [pause.rs](../src/engine/cosmic_dragon_engine/cloud/pause.rs), [tests_pause_smooth_hunt8.rs](../test/engine/cosmic_dragon_engine/cloud/tests/tests_pause_smooth_hunt8.rs) | the pause/resume cloud seats |
| 11 | The Chroma Dragon OKLab pipeline (Phase 9-D) | [palette/mod.rs](../src/engine/chroma_dragon_engine/palette/mod.rs), [lock.rs](../test/engine/chroma_dragon_engine/tests/lock.rs) | `cargo test chroma` (INV-01..INV-19) |
| 12 | The central color theme registry | [catalog.rs](../src/engine/chroma_dragon_engine/catalog.rs), [themes.rs](../src/engine/chroma_dragon_engine/catalog/themes.rs) | Chroma lock full-catalog sweep (INV-02) |
| 13 | Color tune (the per-channel multipliers) | [tuning.rs](../src/engine/chroma_dragon_engine/tuning.rs) | the `--color-tune` contract ([FAQ.md](FAQ.md)) |
| 14 | Crystal Dragon ambient drift | [crystal_dragon_engine/](../src/engine/crystal_dragon_engine/), [ambient/tests.rs](../test/engine/crystal_dragon_engine/ambient/tests.rs) | `cargo test crystal` |
| 15 | The ambient scheduler (time-of-day scenes) | [ambient_scheduler/](../src/engine/crystal_dragon_engine/ambient_scheduler/), [tests.rs](../test/engine/crystal_dragon_engine/ambient_scheduler/tests.rs) | `cargo test ambient_scheduler` + [AMBIENT_SCHEDULER.md](AMBIENT_SCHEDULER.md) |
| 16 | The Power Dragon (adaptive throttle, byte budgets) | [power_manager/](../src/central_control_power_dragon/power_manager/), [tests.rs](../test/central_control_power_dragon/power_manager/tests.rs) | `cargo test central_control_power_dragon` |
| 17 | The self-healing stack (P1, P2, Endurance Health Score) | [self_healer/](../src/central_control_power_dragon/self_healer/), [endurance_health.rs](../src/central_control_power_dragon/endurance_health.rs) | the self-healer + audit seats |
| 18 | Live config reload (the hybrid watcher) | [watcher.rs](../src/config/live_config/watcher.rs), [live_config_poll/](../src/config/live_config_poll/) | `cargo test live_config live_config_poll` + [LIVE_RELOAD_BEHAVIOR.md](LIVE_RELOAD_BEHAVIOR.md) |
| 19 | The CLI suggestion engine (did-you-mean) | [suggestion.rs](../src/cli/suggestion.rs), [clap_suggestion.rs](../test/tests/clap_suggestion.rs), [cli_suggestion_stresstest.sh](../scripts/depth/cli_suggestion_stresstest.sh) | `cargo test clap_suggestion` |
| 20 | Message fill styles (eleven cinematic reveals) | [msg_fill_style/](../src/msg_fill_style/), [tests_msg_fill_style.rs](../test/config/live_config/tests_msg_fill_style.rs) | the per-style `tests_msg_fill_*.rs` seats |
| 21 | Terminal recovery and the fork guard | [io_recovery.rs](../src/engine/cosmic_dragon_engine/terminal/io_recovery.rs), [cbg34_tests.rs](../test/engine/cosmic_dragon_engine/terminal/cbg34_tests.rs), [p5_tests.rs](../test/engine/cosmic_dragon_engine/terminal/p5_tests.rs) | `cargo test cbg34 p5` |
| 22 | Terminal tier detection (the xterm.js OOM guard) | [termdetect/](../src/termdetect/), [TERMINAL_COMPATIBILITY.md](TERMINAL_COMPATIBILITY.md) | `cargo test termdetect` |
| 23 | The zero-dependency benchmark stack | [bench/](../src/bench/), [bench-runner.py](../scripts/bench/bench-runner.py) | the `test/bench/` seat + [BENCHMARKING.md](BENCHMARKING.md) |
| 24 | The fourteen rain styles (six motion-math systems) | [type_rain/](../src/engine/cosmic_dragon_engine/cloud/type_rain/) | the style research records + the Chroma color-cap pins |
| 25 | The Cosmic Dragon Incubator | [cosmic_dragon_incubator/](../src/cosmic_dragon_incubator/), [lock.rs](../test/cosmic_dragon_incubator/tests/lock.rs) | `cargo test cosmic_dragon_incubator` |
| 26 | The live HUD | [hud/](../src/interactive/hud/), [HUD.md](HUD.md) | the `test/interactive/` seat |
| 27 | The cinematic intro | [intro_style/](../src/intro_style/) | the `test/intro_style/` seat |
| 28 | The safepath gate | [safepath/](../src/safepath/) | the `test/safepath/` seat |
| 29 | Doctor and diagnostics | [doctor/](../src/doctor/), [diagnostics/](../src/diagnostics/) | the `test/doctor/` seat |
| 30 | The verification machinery | [gates/](../scripts/gates/), [gate-keepers.sh](../scripts/gate-keepers.sh), [supermassive/](../scripts/supermassive/) | `./scripts/gate-keepers.sh` |
| 31 | The provenance depth matrix (masked signature) | [signature.rs](../src/diagnostics/signature.rs), [info.rs](../src/diagnostics/info.rs), [generate-provenance-words.py](../scripts/release/generate-provenance-words.py) | `cosmostrix -V | grep Depth-matrix` |
| 32 | The deterministic replay oracle (byte-identical sequences) | [frame_digest.rs](../src/bench/frame_digest.rs), [bench_replay.rs](../src/bench/bench_replay.rs) | `cargo test bench::bench_replay` + the CI replay-oracle step |
| 33 | The reproducible release chain (two-host builds) | [ci.yml](../.github/workflows/ci.yml) (`reproducible_build`, `reproducible_verify`), [repro-build.sh](../scripts/release/repro-build.sh) | the CI `REPRODUCIBLE RELEASE CHAIN: PASS` line |
| 34 | The embedded engine oracle (capability depth D4) | [engine_oracle.rs](../src/bench/engine_oracle.rs), [sig_report.rs](../src/diagnostics/sig_report.rs) | `cosmostrix sig` ends `SIGNATURE: AUTHENTIC (6/6 depths verified)` |
| 35 | The syscall-floor ledger (the kernel-floor proof) | [syscall_floor.py](../scripts/supermassive/syscall_floor.py), [supermassive.yml](../.github/workflows/supermassive.yml) | the CI `MASS-VERDICT: PASS` line |
| 36 | The zero-alloc ledger (0.0 allocs/frame contract) | [alloc_trace.rs](../src/diagnostics/alloc_trace.rs), [alloc_ledger.rs](../test/cosmic_dragon_incubator/tests/alloc_ledger.rs) | `cargo test alloc_ledger` |
| 37 | The exact-state suspend/resume contract | [termios_snapshot.rs](../src/platform/termios_snapshot.rs), [torture.py](../scripts/supermassive/torture.py) | the `torture-suspend-resume` PASS rows |
| 38 | The synthetic terminal-gone quarantine | [terminal_gone.rs](../src/interactive/terminal_gone.rs) | `cargo test interactive::terminal_gone` |
| 39 | Reader-aware emission (the consumer-side cost model) | [reader_cost.rs](../src/central_control_power_dragon/reader_cost.rs) | `cargo test central_control_power_dragon::reader_cost` |
| 40 | The measured colorimetry budgets (the Delta-E-OK moat) | [lock_inv20_z4a.rs](../test/engine/chroma_dragon_engine/tests/lock_inv20_z4a.rs) | `cargo test lock_inv20_z4a` |
| 41 | The calibrated phosphor decay (P1-class curve) | [phosphor_calibration.rs](../src/central_control_rains/phosphor_calibration.rs), [tests_phosphor_calibration.rs](../test/central_control_rains/tests_phosphor_calibration.rs) | `cargo test z4c_` |
| 42 | The wave-equation click shockwave (the solved ripple) | [wave_field.rs](../src/engine/cosmic_dragon_engine/cloud/wave_field.rs), [tests_wave_field.rs](../test/engine/cosmic_dragon_engine/cloud/tests/tests_wave_field.rs), [tests_mouse_shockwave.rs](../test/engine/cosmic_dragon_engine/cloud/tests/tests_mouse_shockwave.rs) | `cargo test wave_field mouse_shockwave` |
| 43 | The paste fuse (the structural wedge kill) | [torture.py](../scripts/supermassive/torture.py) (`paste-wedge` leg), the Z5a research record + [AB_REPORT.md](../benchmark/bench-labs/night_frontier_z5a/AB_REPORT.md) | the torture battery's `paste-wedge` rows |
| 44 | The afterglow temporal colorimetry law (INV-22) | [lock_inv22_z5b.rs](../test/engine/chroma_dragon_engine/tests/lock_inv22_z5b.rs), the Z5b research record | `cargo test inv22` |
| 45 | The unsafe law (the compiler-forbidden render core) | [unsafe-census.py](../scripts/gates/unsafe-census.py), the Z5c research record + [AB_REPORT.md](../benchmark/bench-labs/night_frontier_z5c/AB_REPORT.md) | the gate-keepers 23-check sweep |
| 46 | The wave-field energy ledger (the named Lyapunov bound) | [wave_field.rs](../src/engine/cosmic_dragon_engine/cloud/wave_field.rs) (EnergyLedger + the quantum clock), the Z6 [AB_REPORT.md](../benchmark/bench-labs/night_frontier_z6/AB_REPORT.md) | `cargo test wave_field` (16 pins: 10 Z4b + 6 Z6) |
| 47 | The determinism-anchored soak (the 24h canary lane) | [soak_determinism.sh](../scripts/depth/soak_determinism.sh), [soak.yml](../.github/workflows/soak.yml), [ENDURANCE.md](ENDURANCE.md) | the Sunday lane + the smoke run (`--minutes 2`) |
| 49 | The never-be scope contract (the enforced non-goals) | [scope-census.py](../scripts/gates/scope-census.py), [README.md](../README.md) (the contract table) | the gate-keepers `scope-census: OK` verdict line |
| 50 | The lock census (the pre-advisory graph tripwire) | [lock-census.py](../scripts/gates/lock-census.py), [deny.toml](../deny.toml) (yanked deny + the attack-family bans) | the gate-keepers `lock-census: OK` verdict line |
| 51 | The fault-floor ledger (the kernel-side memory mirror, INV-19) | [fault_ledger.rs](../test/cosmic_dragon_incubator/tests/fault_ledger.rs), [alloc_ledger.rs](../test/cosmic_dragon_incubator/tests/alloc_ledger.rs) (the shared pinned rig) | `cargo test --locked lock_inv19` — the zero window and the fresh-page control |
| 52 | The cross-architecture oracle parity (one rain, two silicons) | [supermassive.yml](../.github/workflows/supermassive.yml) (the `sig_parity` + `arm_lock_suite` legs), [engine_oracle.rs](../src/bench/engine_oracle.rs) (the pinned vector) | the supermassive `sig-parity-aarch64 exit=0 D4=pass` relay row + the arm64 musl lock suite `exit=0` |
| 53 | The conservation-ledger family (six named contracts) | [tests_conservation_z8.rs](../test/engine/cosmic_dragon_engine/cloud/tests/tests_conservation_z8.rs), [wave_field.rs](../src/engine/cosmic_dragon_engine/cloud/wave_field.rs) (entry 46, the family's seventh member) | `cargo test --locked lock_inv20` — the six contracts and the family teeth |
| 54 | The control-loop resonance census (the no-limit-cycle proof) | [tests_resonance_z8.rs](../test/central_control_power_dragon/power_manager/tests_resonance_z8.rs), [power_manager/mod.rs](../src/central_control_power_dragon/power_manager/mod.rs) (the coupled axes) | `cargo test --locked lock_resonance` — boundedness, release, non-amplification, and the load-bearing clamp |
| 55 | The information ledger (the field's named entropy and Gini bands) | [info_ledger.rs](../test/cosmic_dragon_incubator/tests/info_ledger.rs), [bench_visual.rs](../src/bench/bench_visual.rs) (the production math the port syncs to) | `cargo test --locked lock_inv21` — the 14-style band sweep, the port-sync cross-check, the collapse control, the PRNG horizon law |
| 56 | The wakeup-floor ledger (the whole-frame syscall budget) | [wakeup_budget.py](../scripts/supermassive/wakeup_budget.py), [event_loop.rs](../src/interactive/event_loop.rs) (the spin-sleep pacing it measures) | the CI `wakeup_floor` job - `MASS-VERDICT: PASS` with writes/frame=1.000, blocking-waits in bounds, zero sleeps/yields/inputs |
| 57 | The emission-minimality certificate (the shipped solver bound to the law reference) | [emit_diff.rs](../src/engine/cosmic_dragon_engine/terminal/emit_diff.rs) (the shipped solver, the seam), [emission_ledger.rs](../test/cosmic_dragon_incubator/tests/emission_ledger.rs) (INV-22), [emission_floor.py](../scripts/supermassive/emission_floor.py) | `cargo test lock_inv22` + the CI `emission_floor` job - `MASS-VERDICT: PASS` with every frame inside the identity floor and the calibrated overhead bands, both injections rejected |
| 58 | The in-process state-digest canary (the soaked process's own anchor) | [premium.rs](../src/bench/premium.rs) (the two anchors, the canary line), [soak_determinism.sh](../scripts/depth/soak_determinism.sh) (pass-law legs 1b/5), [replay_tests.rs](../test/bench/replay_tests.rs) (the chain test) | `cargo test soak_state_digest` + the soak lane's four-way anchor chain and leg 5 (t0 == end inside the soaked process) |
| 59 | The emission conformance law (the diff grammar's machine-checked semantics) | [emission_conformance.rs](../test/cosmic_dragon_incubator/tests/emission_conformance.rs) (the strict VT model, lemmas L1/L2/L3), [emission_conformance-regressions.txt](../test/cosmic_dragon_incubator/tests/emission_conformance-regressions.txt) (the committed witness ledger), [property-regressions.txt](../test/tests/property-regressions.txt) (the config lane's store) | `cargo test emission_conformance` + `cargo test prop_` — the stored seeds replay on every run |

## The standing lanes (the always-on half of the chain)

| Lane | Cadence | What it pins |
|---|---|---|
| [ci.yml](../.github/workflows/ci.yml) | every push | the full suite partitions, the replay-oracle double run, the two-host reproducible release chain, fmt + clippy, cargo-audit, cargo-deny, MSRV, the multi-OS builds, the property lanes with their committed regression stores (entry 59) |
| [supermassive.yml](../.github/workflows/supermassive.yml) | push + scheduled | the PTY contract on a real terminal: byte-log assertions, the syscall-floor ledger, the wakeup-floor ledger, the emission-floor ledger (the stream identity), the torture battery, the arch-parity legs (sig oracle parity on both silicons, the arm64 musl lock suite) |
| [soak.yml](../.github/workflows/soak.yml) | Sunday + dispatch | the determinism-anchored soak (entry 47 + entry 58): digest drift, the four-way anchor chain, the in-process state digest, RSS plateau, fd census |
| [miri.yml](../.github/workflows/miri.yml) | push | no undefined behavior in the audited pointer/alloc families |
| [kani.yml](../.github/workflows/kani.yml) | push | bounded model checking of the audited proof harnesses |
| [fuzz.yml](../.github/workflows/fuzz.yml) | push | the fuzz targets (emission stays safe under adversarial input) |
| [codeql.yml](../.github/workflows/codeql.yml) + [gitbot-audit.yml](../.github/workflows/gitbot-audit.yml) | push | static security analysis + the security audit lane |
| [loom.yml](../.github/workflows/loom.yml) | push | the concurrency proof lane |
| [gate-keepers.sh](../scripts/gate-keepers.sh) | every push (via CI) | every non-Rust quality gate: headers, permissions, LOC caps, language purity, emoji sweep, hype vocabulary, docs truth audit, unsafe census |

## Auditing the chain in one sitting

1. `cargo test` — the whole invariant suite (the entry rows above
   resolve into it).
2. `./scripts/gate-keepers.sh` — every non-Rust gate, including the
   docs-audit that keeps THIS page's links alive.
3. `cosmostrix --bench-replay | grep replay-digest` twice — the
   offline re-verification a buyer can run without the repo (entry
   32's contract).
4. Spot-check any row: follow its artifact links, run its verifier,
   expect the documented verdict line.

A claim in [INNOVATIONS.md](INNOVATIONS.md) or the README that cannot
trace to a row here is a finding, not a fact — file it, and the atlas
grows a row or the claim shrinks.

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
