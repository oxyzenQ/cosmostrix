# Depth Bore (LTS Verification)

<!-- SPDX-License-Identifier: GPL-3.0-only -->

The depth bore is the pre-LTS drill for the bug classes that survive every
normal hunt. As the bug count approaches zero, the remaining bugs hide in
places a unit test or a happy-path smoke run cannot reach: races that fire
only at the right timing, platform-specific paths, extreme inputs, config
combinations nobody imagined, and drift that only shows up after hours.
The bore attacks all five classes against the real binary, from a
clean-slate zero state to a fully loaded hero state.

Harness: `scripts/depthbore/depthbore.py` (Python 3, stdlib only, Unix-only
like every harness in this repo). Binary resolution: `--bin` flag, then
`COSMOSTRIX_BIN`, then `target/release`, then `target/debug`.

## Usage

```bash
python3 scripts/depthbore/depthbore.py             # full bore (~3 min)
python3 scripts/depthbore/depthbore.py --quick     # smoke bore (~2 min)
python3 scripts/depthbore/depthbore.py --parts 15  # only RACE + DRIFT parts
python3 scripts/depthbore/depthbore.py --soak-secs 120 --seed 7
```

**Pre-push drill (owner rule, 2026-09-14):** before every big push, run
`--parts 15 --quick` — the RACE-STORM and DRIFT-SOAK bores against the
release binary, ~75 s. These are the two classes that a silent local
regression is most likely to reopen (timing races and slow drift), and
they are cheap enough to drill on every push. A push with a red drill
is a push that ships the bug; full-bore runs stay the deeper
pre-release checkpoint.

Exit code 0 = every assertion passed; 1 = at least one FAIL. SKIPs are
honest non-claims (a Linux run never claims to have verified the FreeBSD
ports path or the Windows `%APPDATA%` expansion) and are never failures.
The final line is machine-readable:
`DEPTHBORE RESULT: pass=N fail=M skip=K | <secs>s | verdict=CLEAN|BUGS FOUND`.

## The Five Bores

### Part 0 — Zero (clean-slate bootstrap)

The zero-to-hero thread starts at a machine that has never run cosmostrix:
no config directory, no system-wide config, missing or empty `HOME`. The
binary must boot on pure defaults, quit clean via `q` with the terminal
restored, and every static surface (`--version`, `--docs`, `--list-scenes`,
`--list-colors`, `--list-charsets`, `--config-path`) must answer. The hero
endpoint boots the fully loaded config (custom palette + custom scene +
crystal-dragon + ambient overlay) and verifies the `ambient_diag`
observability counters in the `-v` exit summary.

### Part 1 — Race-Storm (the timing class)

Signals mid-render (SIGTERM/SIGHUP/SIGQUIT graceful, SIGINT default-kill
with fork-guard restore, SIGKILL with guard restore), a spawn/kill storm
with random delays and signals where every process must die within 3 s
and every fork guard must follow within 7 s, a 35-resize SIGWINCH storm
survived with a clean quit, a config-write race at the startup read (20 ms
alternation between two valid files — the parse must boot a coherent
snapshot or reject the torn read cleanly, never hang), and a live-reload
storm alternating valid/broken configs every 70 ms ending on the valid
file. Invariants across all of it: no hang, no panic-class exit code, the
terminal always restored, no zombies, no leftover processes.

### Part 2 — Platform-Matrix (honest coverage)

FreeBSD and Windows-only surfaces are SKIPped with the reason recorded —
coverage claims are only made where they were actually tested. Termux
(simulated via `TERMUX_VERSION`/`PREFIX`), dumb/linux/empty `TERM`,
`NO_COLOR=1`, `LANG=C`, tmux, screen and ssh environments must all boot
and quit clean. Byte counts per environment are recorded as observational
data (the color-depth downgrade shows in the output volume).

### Part 3 — Edge-Crusher (extremes)

Geometry from 1x1 to 10000x10000 through `--screen-size` (bench mode) plus
real PTY ioctl sizes (1x1, 100x400, 200x100); the maximum VALID
custom-block config (24 blocks per family — the LTS bound from
NIGHT-hunt-40, so a "10,000-entry" config is impossible by design; the
25th block of every family must be rejected with the named cap); the
200-character message boundary; screen-size validation rejects; and the
config byte edges — invalid UTF-8, empty file, permission denied,
config-as-directory, past the 1 MiB read cap, a 100 KB key, 1000 brackets.
The byte-edge family pins the two config-read fixes from this hunt: the
doctor `CONFIG FILE` status section and the testconf cap alignment.

### Part 4 — Config-Fuzz (deterministic mutants)

A fixed, seeded table of 38 mutations of a rich valid baseline: type
swaps, out-of-range values, `nan`/`inf`, duplicate and unknown keys,
malformed lines, invalid hex, unterminated headers, incomplete
scene-custom blocks, bad ambient times, CRLF endings, BOM prefixes. Every
mutant must be classified cleanly — rc=2 with the diagnostic on stderr
and a clean stdout, or rc=0 (accepted) — and accepted mutants get sampled
PTY boots. Panics, hangs, timeouts and garbage stdout are bugs at any
depth. A CLI flag-combination matrix re-pins the precedence ladder at the
same time.

### Part 5 — Drift-Soak (the bounded 24 h proxy)

A still soak and a live-reload churn soak (default 36 s, `--soak-secs` to
extend toward the real 24 h regime), sampling `/proc` (RSS, threads, fds)
every 2 s and the PTY output rate in two windows. The RSS slope is fitted
over the second half of the window only — the warmup curve is bounded and
converging (measured 120 s: steps +436/+188/+24/+12 KB, flat after ~30 s)
— and windows shorter than 30 s judge only runaway growth, not slope.
Thresholds are smoke-grade by intent (still 20 KB/s, churn 60 KB/s, fd
drift 6-8, thread drift 2-3, output growth x5): the bore catches leak
smells, a microscope analysis still belongs to
[ENDURANCE.md](ENDURANCE.md). The exit must stay clean (`q`, rc=0,
terminal restored) after a long run, and nothing may survive the soaks.

## Inaugural Run Findings (NIGHT-hunt-47-depthbore, 2026-09-14)

The first full bore found and fixed three bugs, then passed
119/119 (2 platform skips):

1. **Doctor was blind to a present-but-unreadable config** — the runtime
   loader treats an unreadable default config as "no config" (invalid
   UTF-8, EACCES, past the 1 MiB cap) and `--testconf` reports it, but
   `--doctor` printed a fully healthy report while the user's settings
   were silently ignored. Fix: the `CONFIG FILE` report section (status +
   effective source + testconf hint; exit codes unchanged — see
   `src/doctor/mod.rs::config_file_status`).
2. **`--testconf` bypassed the 1 MiB size cap** — the one config read
   path not funneling through `read_config_capped` (the S-master-3-v2
   invariant): a 1.2 MB file read unbounded AND reported rc=0 "valid"
   while the runtime refused the same file. Fix: capped read, both
   surfaces agree.
3. **The fork guard lost the kernel reparent race** — `cx-term-guard`
   decided with `getppid() == 1`, but PDEATHSIG wakes the guard while
   the kernel can still take ~200 ms to finish reparenting, so the guard
   read the DEAD parent's pid and silently skipped the restore (the bore
   measured 25-75% restore rates on SIGKILL/SIGINT; subreaper containers
   lose the same check permanently). Fix: liveness polling on the pid
   captured at fork time, plus a termios-still-broken gate so a graceful
   exit stays byte-identical. Measured after: 8/8 restores. See
   [TERMINAL_KILL_CLEANUP.md](TERMINAL_KILL_CLEANUP.md).

## Relationship to the Other Harnesses

The depthtest-5/6/7/8 harnesses pin specific contracts (config error
streams, CLI flag error streams, reference graphs, ambient/crystal PTY
harmony) at assertion depth. The depthbore is the breadth-first drill
across the five hidden-bug classes — it re-pins a few of their contracts
as it passes (the doctor strict ladder, the version rescue, the
deferred-error ordering under reload churn) but does not replace them.
Run the bore before an LTS release candidate; run the depthtest series
when touching the surfaces they pin.
<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying
  on any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
