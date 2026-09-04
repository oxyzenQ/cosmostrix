# Usage: Piping and Redirection — the Fatal-Usage Catalog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

This document catalogs every way `cosmostrix` can be invoked through a
pipe or redirection, what actually happens inside the process for each
one, and which invocation to use instead. It exists because these
failure modes are not obvious from the outside: cosmostrix is a
fullscreen terminal UI that renders ANSI escape-sequence frames to
stdout at full frame rate, and stdout has no obligation to be a
terminal. When it is not, the renderer does not stop — the frames go
wherever stdout points.

Owner hunt NIGHT-hunter-6 (2026-09-05) root-caused the three
invocations below live; every row marked *verified* was reproduced on
a real PTY, not inferred. The catalog also covers the additional fatal
scenarios found during the same hunt.

## The rule in one line

Interactive rain renders to stdout. If stdout is not a terminal, the
run still starts — the frames simply pour into your pipe or file until
the periodic stdout-health probe ends the session.

## Scenario catalog

| # | Invocation | What happens | Verdict |
|---|------------|--------------|---------|
| 1 | `cosmostrix \| less` | Rain renders inside the pager (the pager's window IS a terminal). Quitting the pager closes the pipe; the next frame write fails with EPIPE and the P3 recovery exits gracefully. *Verified.* | Works by accident; confusing. Use a real terminal. |
| 2 | `cosmostrix \| grep test` | Frames pour into the pipe while the reader lives. When the reader dies (Ctrl-C hits the whole foreground group, or the reader exits), the next write fails with EPIPE. Two panic-free outcomes depending on WHERE the reader dies: mid-loop (the common case) the P3 `/dev/tty` recovery fires and the process exits 0 with the stderr notice — *verified in the owner transcript (5 s)*; within the first moments (terminal setup / intro, before the recovery-wrapped main loop — e.g. a `head -c` reader that exits almost immediately) the error propagates as one branded `error: Broken pipe (os error 32)` render with exit 1 — *verified live.* | Fatal for the intent: grep can never usefully filter frame data. |
| 3 | `cosmostrix > file` | The full TUI runs at full frame rate writing raw ANSI frames into the file — megabytes of escape sequences with no line terminators. Only after `FD_HEALTH_PROBE_INTERVAL_FRAMES` (3600) frames — roughly 30-40 s at typical frame rates (the owner's transcript shows 29 s) — does the P5 isatty probe notice stdout is not a tty, synthesize a broken pipe, and end the run gracefully. *Verified (owner transcript).* | Fatal: unbounded garbage file plus a burned CPU core, and the user sees no rain at all. |
| 4 | `cat <the dump file from #3>` | The dump contains RIS/DECSET sequences, cursor addressing, alternate-screen enter/leave, SGR color resets. Dumping it into a live terminal can clear the screen, resize the viewport, flip the color palette, or leave the session in alternate-screen limbo. The file has no line terminators, so a single `cat` writes everything in a burst. | Fatal: do not `cat`/`less`/`grep` a frame dump. Inspect with `less -R` if you must, or regenerate it on purpose with `--benchmark`. |
| 5 | `cosmostrix \| tee log` | Same as #1/#2 plus the file side of #3: the terminal shows the rain through tee while tee ALSO writes the identical frame stream to the log file. | Fatal for the file side; double the write load. |
| 6 | `nohup cosmostrix &` | nohup redirects stdout to `nohup.out` when it is a tty — which silently converts this into scenario #3. The user sees no rain, the file fills with frames, and (with stdin still attached) the session is fully interactive but invisible. | Fatal: never background the interactive mode this way. |
| 7 | `setsid cosmostrix` / no ctty (cron, CI, `ssh -T`) | Terminal setup fails fast with ENXIO: a cleanup burst, one branded `error: os error 6` render with a headless tip pointing at `--benchmark` / `--doctor` / `--dump-config`, exit 1. *Verified (lifecycle matrix row 12).* | Handled by design. |
| 8 | `cosmostrix 2> file` (stdout still a tty) | Verbose stderr output is captured to the file; the rain renders normally on screen. | Safe — documented verbose-usage pattern. |
| 9 | `cosmostrix --benchmark \| jq` / `> report.txt` | Benchmark (and `--doctor`, `--dump-config`, `--docs`, `--testconf`, `--list-*`) print plain text to stdout and exit 0; they are the supported pipeline surface. | Safe — the correct tool for pipelines. |

## The three mechanisms behind the behavior

### P3: reactive broken-pipe recovery (`io_recovery.rs`)

`write_with_recovery()` wraps every frame write. On a recoverable
stdout error (EPIPE, EBADF, ENXIO, EIO, permission denied) it opens
`/dev/tty` (Unix) one-shot, routes the pending buffer through it,
requests graceful shutdown through the normal cleanup path, and prints
the stderr notice:

```
[terminal] stdout write failed (broken pipe) — recovered via /dev/tty, exiting gracefully
```

This is why scenarios #1 and #2 exit cleanly instead of dying with
SIGPIPE or panicking: Rust ignores SIGPIPE by design, and the recovery
turns the failed write into an orderly exit. Note the boundary: only
main-loop frame writes go through `write_with_recovery` — writes
issued during terminal setup and the intro sequence (before the loop
starts) propagate their error directly, which is why a reader that
dies within the first moments produces the branded
`error: Broken pipe (os error 32)` exit instead of the recovery
notice. Both layers are panic-free.

### P5: proactive stdout-health probe (`probe_stdout_health`)

The reactive path only catches failures during active rendering. The
P5 probe closes the idle window: every
`FD_HEALTH_PROBE_INTERVAL_FRAMES` (3600) frames it calls
`isatty(stdout)`; when that returns false it synthesizes a BrokenPipe
error and reuses the P3 recovery — which is exactly how scenario #3
terminates itself. The gap between start and probe (30-40 s of frame
dump) is the reason the NIGHT-hunter-6 startup warning exists.

### NIGHT-hunter-6: startup warning (frame zero)

`run_interactive()` now checks `isatty(stdout)` before entering the
alternate screen and, when stdout is piped or redirected, prints a
branded stderr warning that names the correct tool for each intent
(`--benchmark` for pipelines, `--doctor`/`--dump-config`/`--docs` for
text) and points at this document. The warning fires once, at frame
zero, before the AB-10 runtime-warning buffering engages — so it is
visible immediately even though the session itself continues until the
P5 probe ends it.

## What to use instead

| Intent | Invocation |
|--------|------------|
| Measure performance in a pipeline | `cosmostrix --benchmark` (plain text, exit 0) |
| Diagnostics report | `cosmostrix --doctor` |
| Machine-readable config dump | `cosmostrix --dump-config` |
| Full engine documentation | `cosmostrix --docs` |
| Watch the rain | run `cosmostrix` in a real terminal |
| Log verbose output while watching | `cosmostrix -v 2> verbose.log` |

## Exit-code summary

| Scenario | Exit code |
|----------|-----------|
| P3 recovery after mid-loop EPIPE (#1, #2 late death) | 0 (graceful shutdown path) |
| Branded `error: Broken pipe` when the reader dies during setup/intro (#2 early death) | 1 |
| P5 probe ends a redirected run (#3) | 0 (graceful shutdown path) |
| Headless ENXIO fast fail (#7) | 1 |
| Both fetch tools missing in `--check-update` | 2 (config-error family) |

All of them are panic-free and leave no core dump; the difference is
which layer caught the broken pipe (see the mechanisms below).

See `docs/TERMINAL_LIFECYCLE_MATRIX.md` for the complete terminal
lifecycle contract and `docs/TERMINAL_KILL_CLEANUP.md` for the cleanup
guarantees behind these exits.

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
