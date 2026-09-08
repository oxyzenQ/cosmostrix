<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-3 — full-process flow audit (start to end): the verdict

> Owner mandate (2026-09-08): after a hidden-bug fix, verify that
> cosmostrix has an elegant master flow from process start to process
> end — not a spaghetti flow. This document is the audit deliverable:
> the method, the flow map, the per-segment verdict, and the warts
> cataloged for future work. No behavior changed in this task.

## Method

The codebase is heavy for any reviewer: 438 `.rs` files, ~148,712 LOC,
with `engine/` alone at 104 files / 36,467 LOC. A blanket read is neither
possible nor useful, so the audit used three passes:

1. Entry-point tracing: `main.rs` read in full (800 lines), then
   `interactive::run_interactive` and its setup/sim-draw/finalize
   extractions read in full, following every call that carries state
   across a stage boundary.
2. Lifecycle pairing audit: every terminal-enablement (raw mode, alt
   screen, mouse capture, bracketed paste, kitty keyboard) checked for
   a paired disable on every exit path (normal, error, panic, signal,
   SIGKILL, stuck loop).
3. Targeted pattern hunting (rg) for the classic spaghetti tells:
   TODO/FIXME debt markers, unguarded `unwrap`/`expect` on reachable
   paths, duplicated loading of the same resource across stages,
   dual-field state that can drift, and unbounded cross-module mutable
   state.

## The flow map (as verified)

```text
main()
 |- start_time (monotonic, first statement)
 |- CPU feature check (x86_64 only, before any v3/v4 instruction)
 |- install_panic_hook()                    [main-thread gated, NIGHT-hunter-4]
 |- clap Command build + styles + help template
 |- prevalidate_cli_args (raw argv, before -mb expansion)
 |- argv_expand ( -mb / -mfs shorthands -> long flags )
 |- clap parse -> Args
 |- handle_pre_config_returns (--help, --dump-config, ...)
 |- bench scene default (monolith) + power_dragon/msg_mode defaults
 |- apply_config_and_runtime_defaults (config.toml + scene + runtime)
 |- canonicalize_runtime_args
 |- handle_post_config_returns (--doctor, --docs, --version, ...)
 |- arg validators (shading, bold, fps precedence chain, xterm.js cap,
 |  duration, crystal-dragon-secs)
 |- color resolution (custom wins -> builtin -> error with suggestions)
 |- color_tune, rain_style, glitch/linger validators, speed
 |- charset resolution (custom wins -> builtin)
 |- density, screen_size, async
 |- verbose startup report
 |- build_cloud_cfg -> CloudConfig (one struct, one owner)
 |- bench dispatch (early return if bench mode)
 '- interactive::run_interactive(&cloud_cfg)
    |- warn_if_stdout_not_terminal (frame zero)
    |- set_interactive_session_active (AB-10 warning buffering begins)
    |- spawn_kill9_terminal_guard
    |- setup_terminal_cloud_frame
    |  |- install_signal_handlers (signal threads + watchdog)
    |  |- Terminal::with_signal_exit (raw mode + alt screen + pairs)
    |  |- Cloud::create + reset + tuning + user_override arm
    |  |- ColorCache + Frame + bg fill
    |- run_intro_sequence (skippable, 'q' honored)
    |- ambient startup decision (CLI-wins deferral contract)
    '- while cloud.raining
       |- GRACEFUL_SHUTDOWN check
       |- drain_config_events (try_recv, non-blocking)
       |- apply_config_rebuild (pending config -> full rebuild)
       |- poll_ambient_events (scheduler rx + ground truth)
       |- run_adaptive_throttle (power manager pacing)
       |- end_time check
       |- SIGCONT terminal reinit swap
       |- event wait loop (poll/spin hybrid, dead-PTY guards)
       |- handle_resize (debounced)
       |- frame_period from effective_fps
       |- update_hud_state
       |- run_sim_and_draw (cloud.rain_at -> HUD -> term.draw)
       |- post_draw_accounting (overshoot, utilization)
       |- effects_auto_gate observe
       |- sample_p5_health
       |- update_perf_stats
       |- run_self_healer
       '- next_frame schedule (single-reschedule)
    '- finalize_session (stats, terminal drop, final-state handoff)
 '- post-exit verbose + warning drain (fatal exits code 1)
```

## Per-segment verdict

| Segment | Verdict | Notes |
|---------|---------|-------|
| Startup chain (main) | Disciplined | Every stage has a documented contract and a dated rationale comment; early returns are centralized; validation dies once per value with consistent UX shape (ux.rs contract). |
| Config resolution | Disciplined, one wart | The custom-wins precedence chain is uniform across color/charset/scene; the wart is that `load_config_file` re-reads and re-parses the file at each resolution site (see Wart 1). |
| Terminal lifecycle | Near-airtight | Single RAII owner; every enablement has a paired state flag; Drop + idempotent restore covers normal/error/panic/signal/SIGKILL/stuck exits. No skipped-restore exit path found. |
| Thread + channel topology | Clean | Bounded `sync_channel(64)` with `try_send` everywhere (watcher, poller, ambient); signal threads set flags and never race the terminal; single-writer shutdown. |
| The rain loop | Master flow, one acknowledged debt | The loop body is decomposed into 20+ single-purpose modules (`event_loop_*`), each with a stated contract; the mutable state threaded through them is the acknowledged debt (Wart 3). |
| Teardown | Disciplined | `finalize_session` owns the single exit funnel; exit codes are coherent (0/1/2/130) and documented in TERMINAL_LIFECYCLE_MATRIX.md. |

## Warts cataloged (documented, not fixed in this task)

1. **Config re-load multiplicity (elegance, not performance).**
   `load_config_file` performs a fresh disk read + full parse per call,
   with no caching layer. One interactive startup parses the file 9
   times (main.rs x4, canonicalize, build_cloud_cfg x2,
   event_loop initial map), 12 with `--verbose`. Quantified cost is
   ~30-150 microseconds per parse on a page-cached <5 KB file — under
   1.4 ms total, less than 0.1% of time-to-first-frame. A one-shot
   parse threaded through the resolution chain would be more elegant,
   but the current shape also buys freshness (each site reads the
   file as of its own moment). Verdict: maintainability wart, safe to
   leave; a `ParsedConfig` threaded through main is the natural fix
   if ever wanted.
2. **`duration` / `duration_s` dual field.** `CloudConfig` carries both
   the raw `args.duration` and the validated `duration_s`;
   `event_loop.rs`'s end-time computation validates one and then reads
   the other via `cfg.duration.unwrap_or(s)`. The values coincide by
   construction today (`duration_s` is `args.duration.map(validated)`
   and the validator is a pass-through in-range check), so this is
   redundancy, not a bug — but it is exactly the shape that becomes a
   bug the day one field gains another writer. A future cleanup should
   delete the raw field from `CloudConfig` and keep only the
   validated one.
3. **The rain loop's coupled mutable state (acknowledged).**
   `event_loop.rs` carries an explicit `LOC_EXEMPT`: the
   `while cloud.raining` loop threads ~20 mutable borrows through the
   extracted sibling modules because they all operate on the same
   session state. The file documents that a context-struct refactor
   is the prerequisite for further splitting. This is the largest
   structural debt in the flow — and it is acknowledged, bounded, and
   documented rather than hidden, which is the difference between
   debt and spaghetti.

## Verdict

**Master flow, not spaghetti.** The start-to-end path is a linear,
single-owner pipeline with one entry funnel (main), one state owner
per concern (CloudConfig for configuration, Cloud for simulation,
Frame for the buffer, Terminal for I/O, PowerManager for pacing), one
exit funnel (finalize_session + Terminal::drop), and defense layers
that are idempotent and independently documented. The tells that would
indicate spaghetti — hidden cross-module mutation, duplicated state
with drift, missing teardown pairs, silent error swallowing — were
each specifically hunted and not found beyond the three cataloged
warts, all of which are already documented in-tree. The owner's
"premature flow" suspicion is answered: the flow holds up under a
start-to-end trace after the hidden-bug fixes.

Related: NIGHT-hunter-4 (same session) fixed the one genuine pairing
hole this audit surfaced in the panic-hook layer — see
`docs/research/NIGHT_HUNTER_4_BUG_HUNT.md` and the panic-hook module
documentation.
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
