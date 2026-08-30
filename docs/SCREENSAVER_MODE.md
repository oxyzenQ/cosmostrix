<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Screensaver Mode (`-s` / `--screensaver`) — Behavioral Audit

Owner-requested Z-master-1B LTS audit: "what is actually different when
running with `--screensaver` vs without?" This document is the verified
answer, traced through the source. It replaces several stale claims that
previously lived in `--help`, the README, and inline comments (see
§6 Stale texts fixed by this audit).

## TL;DR verdict

`--screensaver` is **functionally near-identical to the default mode** in
the current codebase. The name and the old help text suggest "lock the
input, only `q` exits" — but the v17+ "only-`q`-quits" policy was applied
to **both** modes, and every interactive key works in **both** modes. The
flag survives as a compatibility/labeling switch: exactly two micro-level
scheduling differences remain (§2), plus informational output (§3).

## 1. Where the flag lives and where it is read

| Location | Role |
|---|---|
| `src/config/mod.rs` (clap arg `-s`/`--screensaver`) | CLI definition; also settable in `config.toml` |
| `src/cli/app.rs` / `src/cli/build_cloud_cfg.rs` | Threaded into `CloudConfig.screensaver` |
| `src/interactive/event_loop.rs` (`if cfg.screensaver`) | **The only functional read in the entire codebase** |
| `src/output/verbose.rs` / `src/output/startup_verbose.rs` | Verbose reporting (`screensaver: true/false`) |
| `src/bench/bench_helpers.rs` | Warns that bench mode has no input loop |

Everything below diverges from that single read in `event_loop.rs`.

## 2. The two real behavioral differences (both micro-scale)

Both live in the key-event branch of the event loop:

1. **Post-`q` event drain.** When `q` sets `cloud.raining = false`, the
   inner event-drain loop `break`s **immediately** in screensaver mode —
   any still-queued key events are discarded. In normal mode the drain
   loop keeps consuming the already-queued events until the queue is
   empty, and only then the outer loop exits. User-visible impact: none
   (the app is exiting either way); it only changes how many queued
   events are processed during the exit window.

2. **Pause-toggle fast redraw.** In normal mode, when
   `handle_keybinding()` reports `redraw_needed` (returned by
   `Cloud::toggle_pause()`), the loop sets `next_frame = Instant::now()`
   to render the new pause state on the very next frame. In screensaver
   mode this fast-path is skipped and the pause state renders on the
   regular frame cadence instead. At the default frame rate this is at
   most one frame of extra latency (typically <=16 ms; up to the idle
   cadence if the renderer is idle-throttled). This is an incidental
   structural consequence of the `if/else` shape, not a designed
   screensaver behavior.

## 3. What is IDENTICAL in both modes (verified, not assumed)

| Behavior | Evidence |
|---|---|
| All runtime keys (`q`, `c`/`C`, `s`/`S`, `x`/`X`, `p`, `i`, `[`, `]`, `Space`, `Up`/`Down`) | `handle_keybinding()` runs before the `cfg.screensaver` branch; it contains no screensaver logic |
| Only `q` quits (Esc, Ctrl+C, Tab, unknown keys are no-ops) | Same quit policy arm in both modes |
| Mouse click never exits (v17 policy) | Mouse branch has no screensaver check; clicks only drive glow/click-wave |
| Mouse capture (blocks drag-select) | Always on in both modes |
| Cinematic intro plays (and `q`/`Q` skips it) | `run_intro_sequence()` has no screensaver check; auto-skip happens only below `MIN_INTRO_COLS x MIN_INTRO_LINES` (10x5) |
| `--duration` auto-exit, resize handling, adaptive throttling, ambient scheduler, live reload, self-heal | No `cfg.screensaver` reads in those modules |

## 4. Why the flag still exists (LTS rationale)

Removing the flag would break every script, alias, config.toml, and
downstream packaging (AUR unit files, dotfiles) that passes
`--screensaver`. Keeping it costs one `bool` field and one branch. The
LTS-correct decision is: keep the flag, document its true (near-no-op)
behavior, and keep the help text honest — which is what this audit does.

## 5. Historical note

Early cosmostrix (pre-v17) treated `--screensaver` as a strict input
lock: mouse click exited the app in normal mode but not in screensaver
mode, and the help text advertised "all other input ignored". v17 removed
mouse-click exit everywhere ("only-q-quits" policy). The interactive key
handler was later unified so all keys work in both modes. The stale help
text outlived those changes — classic doc-drift: the source was refactored
faster than `--help`, README, and comments were resynced.

## 6. Stale texts fixed by this audit

| Location | Was | Now |
|---|---|---|
| `src/config/mod.rs` short help | "only q exits (all other input ignored)" | "only 'q' exits; interactive keys still work" |
| `src/cli/help_detail.rs` `-s` block | "all other keys are silently ignored" + stale key list | accurate behavior + full key set |
| `src/cli/help_detail.rs` `--intro` block | "Auto-skipped in `--screensaver` mode and on terminals smaller than 80x24" | intro is NOT auto-skipped in screensaver mode; threshold is 10x5 (`MIN_INTRO_COLS x MIN_INTRO_LINES`) |
| `README.md` Quickstart + CLI Reference | "all other keys ignored" | matches actual behavior |
| `src/interactive/event_loop.rs` comment | key list missing `X`/`x` uppercase reverse-cycle | full list |

## 7. Maintenance rule

If you touch the `cfg.screensaver` branch in `event_loop.rs`, update
§2/§3 of this document in the same commit. The table in §3 is a
verified claim list — do not weaken it to "probably unchanged".

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
