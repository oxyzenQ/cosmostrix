<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-dragon-1 audit — the master usage verdict: server, desktop, or hybrid at ultra-massive scale, documented from source

Owner brief (2026-10-27): "master depth audit to already usage for
ultra massive scale. this project best use for server or desktop or
hybrid? need documented" — plus two concrete gaps the owner found
against the zelynic family: the missing
`scripts/package/install-on-my-machine.sh` (zejtron reference) and the
missing `persist-credentials: false` in release.yml (zejtron/zylaxion/
zelynic each carry it; cosmostrix carried zero). Both are fixed in this
audit's commits and recorded here as part of the scale-readiness
verdict.

## The verdict, stated once

**cosmostrix is a desktop-first interactive terminal renderer that is
hybrid-capable by design and server-usable only as a per-session
process or a headless instrument — it is not, and must not be run as,
a server daemon.** The honest scale statement: "ultra massive scale"
for this binary means thousands of CONCURRENT HUMAN TERMINAL SESSIONS
(each self-bounded), fleet-scale CI/benchmark runs (already proven by
this repository's own KVM lanes), and browser-hosted terminal farms —
not one central server process serving many users, because no such
process model exists in the source.

## Evidence: what the binary IS (the process model decides everything)

- One process per terminal session, no daemon path. Repo-wide grep for
  `TcpListener`/`UdpSocket`/`bind`/`listen`/`serve` in src/ returns
  zero network-binding code; the engine's only network-adjacent
  behavior is downloading nothing and listening for nothing. There is
  no service unit, no socket activation, no multiplexer: the
  installer's own doc block records "no critical service unit to
  install" as the family convention.
- The interactive path is TTY-gated by design: piped stdout into the
  interactive rain loop is a documented fatal-usage family
  (docs/USAGE_PIPE_REDIRECT.md is the catalog; event_loop warns at
  frame zero and teaches `--benchmark` / the text modes). The event
  loop consumes crossterm keyboard/mouse/resize events — a human on
  the other end is a structural assumption, not an accident.
- Memory and CPU are frame-bounded per process: the terminal size
  clamps to 1024x500 cells (LastFrame ~8 MB worst case), the 8K UHD
  benchmark bounds (BENCH_MAX 7680x4320) exist precisely to prove the
  renderer survives the largest terminal anyone will open, and the
  adaptive throttle (event_loop_adaptive.rs) drops to an adaptive idle
  resync interval when frames stop changing — an unattended session
  costs almost nothing; a rendering one is one core at most.

## Evidence: desktop is the designed home

The deepest engineering in this repository targets long, interactive,
local terminal sessions: the five-layer terminal restore family
(SIGKILL fork guard, panic hook with double-panic coredump guard,
session watchdog, Tier-2 backpressure), resize-drag storm handling
(reuse_or_new allocation reuse), per-host FPS caps for VSCode/xterm.js
and VTE-class renderers, the power subsystem (RAPL/thermal sampling
feeding perf_pressure into pacing), and the multi-hour endurance work
(periodic RIS resets preventing the xterm.js V8 OOM over multi-hour
runs). None of that machinery helps a daemon; all of it helps a human
who leaves rain running all day. The AUR package, the /usr/bin install
law, and the new install-on-my-machine.sh all describe the same
shape: one user, one machine, one terminal.

## Evidence: hybrid is genuinely engineered, not accidental

- SSH: the chroma pipeline survives the hop without COLORTERM
  forwarding for the truecolor-native TERM family (alacritty, kitty,
  ghostty, wezterm, foot, contour — NIGHT-research-1, documented in
  docs/TERMINAL_COMPATIBILITY.md "SSH Or Headless Usage"), and the
  /dev/tty fallback (io_recovery.rs recover_to_tty) keeps the session
  recoverable when the primary stdout dies mid-run — the SSH-disconnect
  case is a named, tested failure mode.
- Browser-hosted terminals: the entire Tier-2 subsystem (byte-budget
  backpressure + RIS scrollback resets) exists because xterm.js hosts
  buffer unboundedly — that is the web-terminal deployment shape.
- Mobile: Termux is a first-class archive in the release matrix
  (android-aarch64-native), with storage-path handling documented.
- Pipes and pagers: `cosmostrix | less` is a documented, deliberately
  supported lifecycle (TERMINAL_LIFECYCLE_MATRIX section 12), with the
  stdout-health probe exiting gracefully when the reader goes away.

## Evidence: server usage is real but narrow — and already proven at fleet scale

- Headless instrument modes work pipe-first and are the documented
  server answer: `--benchmark` (the full report this audit ran
  headless: 8/8 visual+perf sections), `--doctor`, and the text modes
  taught by the pipe gate.
- The repository's own CI IS the ultra-massive-scale proof for
  unattended runs: the Supermassive kernel lanes boot this binary's
  bench battery inside KVM micro-VMs on the real 2.6.27 floor kernel
  and the latest mainline kernel, on every push, green. A binary that
  runs its full battery inside a 2.6.27 guest on a 2026 host has no
  server-side boot problem.
- What a server must NOT host: a persistent rendering service. The
  interactive loop needs a PTY and an event consumer per instance;
  there is no shared-state, multi-tenant, or network-facing mode, and
  adding one would be a different product.

## The deployment matrix (documented verdict)

| Deployment | Verdict | Binding evidence |
| --- | --- | --- |
| Local desktop terminal (Linux/mac/BSD/Windows/Termux) | THE designed use; the deepest guarantees in the codebase live here (5-layer restore, multi-hour endurance) | restore family, adaptive pacing, power mgmt, endurance RIS |
| SSH session from a desktop to any server | Fully supported hybrid; disconnect-recoverable | TERMINAL_COMPATIBILITY SSH section, recover_to_tty |
| Web/browser terminal farm (xterm.js-class hosts) | Supported hybrid; Tier-2 exists for exactly this | backpressure + RIS, per-host FPS caps |
| Server, per-user interactive sessions at scale (jump hosts) | Supported; cost is per-session and self-bounded (idle resync ~0, one core max while rendering) | adaptive throttle, frame-bounded memory, TTY gates |
| Server, headless instruments (bench/doctor/art-text modes) | Supported; fleet-proven | KVM lanes green every push; 8K bounds |
| Server, as a daemon/service/shared renderer | NOT SUPPORTED — no listener, no multiplexer, no service unit in source; do not build one into this binary | zero network-binding code; installer doc block |

## Scale-readiness fixes shipped by this audit (both owner-found)

- `f3d0da1` — estate-wide `persist-credentials: false` on 34 of 36
  checkout steps (the zelynic-family law; the owner's grep showed
  release.yml at zero while all three siblings carry it). The runner
  token no longer sits in every job's .git/config — the supply-chain
  requirement for letting more CI lanes, more runners, and more
  third-party actions near the tree as the estate scales. The one
  exception is the weekly-deps checkout whose job pushes; preserved.
- `a8451de` — scripts/package/install-on-my-machine.sh, the
  zejtron-referenced installer, adapted to cosmostrix's tier law
  (v4/v3 musl detection, baseline-alias honesty, official-build
  signature verify, /usr/bin law, uninstall.sh pairing).

## What would falsify this verdict

- Any PR introducing a listener, socket activation, or a multi-tenant
  rendering path invalidates the process-model row of the matrix.
- A terminal-size path that escapes the 1024x500 clamp (or a frame
  buffer that grows with session count rather than frame size) would
  break the per-session cost bound that the jump-host row relies on.
- The idle-resync guarantee holds only while frames stop changing; a
  scene that never idles (max-density rain on an 8K terminal) is a
  deliberate one-core load — capacity planning for shared hosts must
  assume that worst case, not the idle average.

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
