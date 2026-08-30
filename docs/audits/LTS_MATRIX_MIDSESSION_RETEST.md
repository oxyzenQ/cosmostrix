<!-- SPDX-License-Identifier: GPL-3.0-only -->

# LTS Matrix Mid-Session Retest + Stability/OOM Dig — v51.0.0-beta.1

> Source code is truth; cross-check the referenced files before relying on
> this analysis for implementation decisions. This document is an internal
> research artifact, not a contract.

**Date:** 2026-08-30
**Auditor:** oxyzenQ (Cosmic Dragon mode)
**Version:** v51.0.0-beta.1 (commit 29f2f00a, release build, linux-amd64-v1-gnu)
**Scope:** Direct live retest of the terminal lifecycle LTS matrix
(`docs/TERMINAL_LIFECYCLE_MATRIX.md`) + mid-session interaction paths +
stability/OOM digging (RSS trajectories, allocator profile, worst-case
memory math).
**Method:** PTY-harness live testing (not static analysis). Prior Z-1..Z-5
audits were ripgrep/file-read sweeps; this session EXECUTED the binary
against every testable matrix row and sampled `/proc/<pid>/status` during
long runs.

---

## 1. Executive Summary

| Area | Verdict |
|------|---------|
| Lifecycle rows 1, 3, 4, 5, 6 (q, SIGTERM, SIGHUP, SIGTSTP/SIGCONT) | **PASS live** — graceful exit, termios fully restored, cleanup sequences emitted |
| Row 7 (SIGKILL) with real process topology | **PASS live** — fork guard `cx-term-guard` restored termios within 0.5 s |
| Row 7 edge case: cosmostrix as session leader | **RESIDUE** (harness-induced; see F3) |
| Row 12 (non-TTY) | **Behavior differs from doc** (see F2): exit 1 ENXIO, not silent N/A |
| Row 13 (benchmark headless) | **PASS live** — exit 0 |
| Mid-session: HUD toggle, pause/resume, resize up/down, live-reload | **PASS live** — no crash, no hang, clean exit |
| Stability: 90 s bench RSS | **FLAT at 4520 kB, zero growth after warmup** |
| Stability: 120 s interactive RSS (6 mid-session keys) | **FLAT 5356 kB with one bounded +272 kB step; peak 5628 kB** |
| Allocator | **zero per-frame allocs, heap_retained 0 B** — no leak signature |
| OOM worst case | interactive cap 1024x500 ~= 12 MiB (safe); 8K bench cap ~= 1-1.5 GiB (see F4) |

Five findings (F1-F5) below; all doc fixes landed in this commit; the one
code-level hardening candidate (F3) is documented for the owner to schedule.

---

## 2. Methodology

Harness: Python `pty.openpty()` + subprocess, 120x35 virtual terminal,
TERM=xterm-256color where noted. Output drained continuously; escape
sequences tracked per-chunk; termios of the pty slave inspected after
process exit (ICANON/ECHO set = cooked mode restored). RSS sampled from
`/proc/<pid>/status` (VmRSS/VmHWM/VmSwap/Threads) at fixed intervals.

Process topology matters for signal tests. Two configurations were used:

- **Session-leader topology** (`start_new_session=True`): cosmostrix is
  the session leader. NOT how real terminals run it — used only for the
  first sweep; produced one artifact (F3).
- **Real topology** (bash parent, no new session): bash is the session
  leader, cosmostrix a foreground child — matches a real terminal. Used
  for the SIGKILL guard verification.

---

## 3. Lifecycle Matrix Retest Results

| Row | Path | Live result | Doc match |
|-----|------|-------------|-----------|
| 1 | `q` exit | exit 0; cursor show + mouse-off sequences emitted; termios restored (ICANON+ECHO on); with TERM set: alt-screen enter at start / leave at exit correctly paired | yes |
| 3 | SIGTERM | exit 0 (graceful); termios restored | yes |
| 4 | SIGHUP | exit 0 (graceful); termios restored | yes |
| 5 | SIGTSTP | proc state `T` (stopped) as documented | yes |
| 6 | SIGCONT | state back to `S`, rendering continues; subsequent `q` exits cleanly (exit 0, termios restored) | yes |
| 7 | SIGKILL (real topology) | exit -9; guard child `cx-term-guard` present during run; termios restored to cooked within 0.5 s; guard exits after restoring | yes — "fork guard best-effort" verified working |
| 7 | SIGKILL (session-leader topology) | exit -9; termios left RAW (ICANON/ECHO off) — guard child killed by SIGHUP before restoring | edge case, see F3 |
| 12 | non-TTY pipes, no ctty | exit 1; stderr `error: No such device or address (os error 6)`; cleanup burst written to stdout | differs from doc, see F2 |
| 13 | `--benchmark --bench-io` headless | exit 0 | yes |

Cleanup sequences observed on graceful exits (in order): kitty-sync off
(`?2026l`), mouse modes off (`?1006l ?1015l ?1003l ?1002l ?1000l ?1004l`),
bracketed paste off (`?2004l`), line-wrap re-enable, cursor show (`?25h`),
alt-screen leave (`?1049l`, TERM-capable terminals). This matches the
documented 7-step `Terminal::drop()` sequence in the matrix doc.

### 3.1 Alt-screen is TERM-conditional (F1)

With TERM unset (or `dumb`), no `\x1b[?1049h/l` is ever emitted —
`termdetect::has_alternate_screen` is false and cosmostrix renders directly
on the MAIN screen, overwriting visible content (scrollback untouched).
This is deliberate (documented at `src/termdetect/mod.rs` lines 139-147:
only `dumb` and unset TERM lack alt screen) but the lifecycle matrix never
mentions the variant. Doc updated in this commit (matrix now notes it).

---

## 4. Mid-Session Interaction Retest

One 120 s PTY run, keys injected at fixed times (HUD on `i` @30 s, pause
`p` @45 s, resume `p` @60 s, HUD off `i` @75 s, density `[` @90 s, `]`
@105 s):

- Process stayed alive (state S) through every interaction; exit 0 via `q`.
- Pause/resume (the v51 shortkey-isolation + HUD-metric-freeze fixes from
  commit 628b2020) behaved at process level: no crash, no hang, no
  runaway CPU while paused.
- SIGWINCH resize 120x35 -> 200x60 -> 120x35 mid-run: renderer kept
  running (state S after both resizes), clean exit afterward.
- config.toml live-reload mid-run (speed 20 -> 60 written under the
  process cwd): process stayed alive, clean exit; no reload crash.

---

## 5. Stability / OOM Dig

### 5.1 RSS trajectories (measured)

| Run | Duration | RSS profile | Peak (VmHWM) | Verdict |
|-----|----------|-------------|--------------|---------|
| `--benchmark --bench-io` | 90 s | 4 kB at spawn -> 4520 kB by t=3 s -> **flat to t=90 s** (zero growth) | 4524 kB | stable |
| Interactive PTY (6 mid-session keys) | 120 s | 1464 kB -> 5356 kB by t=3 s -> flat -> **one +272 kB step at t~78 s** -> flat to end | 5628 kB | stable, bounded step |
| `--benchmark --bench-io` | 5 s / 30 s | allocator cross-check | — | see 5.2 |

The interactive +272 kB step coincides with the HUD-off/ambient window and
matches a single palette/SGR-cache rebuild (`color_cache.rs` allocates
`offsets` + `n*32` byte buffer per palette build — bounded per palette,
rebuilt on theme change). A leak signature would be monotonic creep; a
step-then-flat is bounded caching. Long-run confirmation: recommended 2 h
soak (section 7).

### 5.2 Allocator profile (bench reports)

| Metric | 5 s run | 30 s run |
|--------|---------|----------|
| alloc_calls | 288 | 1.7 K |
| dealloc_calls | 283 | 1.7 K |
| realloc_calls | 411 | 2.4 K |
| alloc_calls_per_frame | 0.0 | 0.0 |
| heap_retained | 0 B | 0 B |

Allocation rate ~= 57/s constant in wall time (not per-frame): a periodic
small allocation that is always freed — consistent with stats/flush
buffering, not a leak. The hot path remains zero-alloc as designed
(`docs/BENCHMARK_ADVANCED.md`).

### 5.3 Worst-case memory math (OOM vectors)

Frame storage is `Vec<Cell>` x2 (current `frame.rs` + `last_frame.rs`
diff buffer) + 2 x `u32` generation maps (`cell_gen`, `dirty_cell_gen`):

| Configuration | Cells | Approx heap |
|---------------|-------|-------------|
| Interactive cap (MAX 1024x500) | 512 K | ~12 MiB (both buffers + gen maps) |
| 8K bench cap (BENCH_MAX 7680x4320) | 33.18 M | **~1.0-1.5 GiB** (Cell 16-24 B x2 buffers + 253 MiB gen maps) |

The 8K figure is by-design (clamped, opt-in via `--screen-size`), but
`docs/BENCHMARKING.md` never warned about it. Doc updated in this commit
(F4). No unbounded growth vectors were found: message overlay, particle
pool, ghost trails and palette caches are all fixed-capacity or rebuilt
per switch; ENDURANCE.md mechanisms (static LUTs, `SmallVec` inline dirty
buffer, generation counters instead of memsets) verified present in code.

### 5.4 Endurance record freshness (F5)

The most recent recorded endurance run in `docs/ENDURANCE.md` is
**v4.0.1, 2026-06-11** — roughly 47 major versions stale. This session
adds fresh v51.0.0-beta.1 short-run records (90 s bench + 120 s
interactive) to `docs/ENDURANCE.md`, but they are explicitly labeled
short-run smoke data, NOT a substitute for the documented 2 h soak. The
owner should schedule a 2 h run on target hardware to re-establish the
endurance baseline for the v50/v51 line.

---

## 6. Findings and Actions

| # | Finding | Severity | Action in this commit |
|---|---------|----------|----------------------|
| F1 | Alt-screen fallback (TERM unset/dumb renders on main screen) undocumented in the lifecycle matrix | doc gap | matrix updated with the variant note |
| F2 | Row 12 reality: non-TTY without ctty exits 1 with `os error 6` (ENXIO) after a cleanup burst; matrix claimed "no cleanup needed / N/A" | doc gap | row 12 rewritten to match observed behavior + `--benchmark` headless pointer |
| F3 | Fork guard dies to SIGHUP when cosmostrix itself is the session leader (setsid launchers): its sigwait set blocks only SIGTERM, so the kernel's session-leader-exit SIGHUP kills the guard before `tcsetattr` | hardening candidate (edge case) | documented here; suggested fix for a future unlock: add SIGHUP to the blocked/sigwait set in `src/platform/fork_guard.rs` |
| F4 | `--screen-size` 8K bench allocates ~1-1.5 GiB; no memory warning in BENCHMARKING.md | doc gap | warning added to the `--screen-size` row |
| F5 | Endurance record stale since v4.0.1 | process gap | v51.0.0-beta.1 short-run records added to ENDURANCE.md + 2 h soak recommended |

---

## 7. Recommended Follow-ups (owner decision)

1. **2 h endurance soak on target hardware** at v51.0.0-beta.1 (or .2):
   interactive mode + periodic ambient theme cycling; RSS-growth < 2%/h
   pass criterion per ENDURANCE.md. Confirms the bounded-step hypothesis
   at the 44-palette ceiling (~12 MiB worst case if every theme caches).
2. **F3 hardening** (one-line signal-set change in fork_guard.rs + a
   session-leader regression test): low risk, unlocks the setsid-launcher
   scenario. Requires the chroma/interactive unlock protocol if it lands.
3. Non-TTY error message could hint `--benchmark` for headless use
   (currently a bare `os error 6`); cosmetic, bundle with the next CLI
   polish pass.

---

**Task:** LTS matrix mid-session retest + stability/OOM dig (Z-master-1B).
**Status:** Audit complete; doc fixes landed; F3 hardening + 2 h soak
scheduled as owner decisions.
**Artifacts:** this report; doc updates in TERMINAL_LIFECYCLE_MATRIX.md,
BENCHMARKING.md, ENDURANCE.md; local harness scripts (not committed —
sandbox-local test tooling).
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
