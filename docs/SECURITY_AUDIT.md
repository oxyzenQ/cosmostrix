<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Security Audit — Cosmostrix "Dragon Hunt Security Mode"

**Audit date**: 2026-08-05
**Auditor**: Super Z (main agent) + 1 parallel Explore agent
**Subject**: `cosmostrix` v30.0.0-alpha.1
**Commit**: `ebca662` (post-CI-fix) + this commit
**Scope**: full source tree (`src/`, `build.rs`, `scripts/`, `.github/workflows/`, `aur/`)
**Methodology**: automated `rg` sweeps for every sensitive capability class + manual review of every match

---

## Verdict

**Cosmostrix is a safe "digital art" program, not malware.** Every sensitive
capability (network, subprocess, terminal mutation) is either opt-in
(`--update`, `--reset-terminal`) or strictly bounded (path whitelist,
FFI-only `unsafe` with soundness audits, signal handlers that only set
atomic flags). The codebase demonstrates a mature security posture:
documented `unsafe` policy, exhaustive path-traversal tests, license-policy
enforcement, daily `cargo audit`, two-phase privilege separation for dep
updates, and a self-audit document covering every `unsafe` site.

---

## 1. `unsafe` Usage — 15 Sites, All Sound

The codebase has a documented **"no new unsafe in renderer/core paths"**
policy (`docs/SIMD_FEASIBILITY.md` §1, §2.3; `docs/RULES.md`). Every
`unsafe` site is FFI into `libc`/Mach and carries a `// SAFETY:` comment.
There is **no `unsafe` in the renderer hot path** (verified by
`info.rs:303`).

### Summary by category

| Category | Count | Locations | Assessment |
|----------|-------|-----------|------------|
| macOS Mach `task_info` | 2 | `memstat.rs:111`, `cpustat.rs:137` | Kernel writes into our zeroed struct; macOS-only |
| Linux `libc::stat`/`fstat` | 2 | `main.rs:218, 221` | Read-only metadata on stdout fd |
| Linux fork-based SIGKILL guard | 1 | `main.rs:245` | Defensive; `prctl(PR_SET_PDEATHSIG)` + `sigwait`; opt-out via `COSMOSTRIX_NO_FORK_GUARD=1` |
| `perf_event_open` (Linux, bench-only) | 3 | `bench_perf.rs:86, 124, 138` | Returns `Option`; fd checked |
| `/dev/null` write (test-only) | 2 | `cosmic_dragon/egg/io_uring_rejected.rs:68, 84` | `#[cfg(test)]` — never compiled into release |
| Custom allocator | 1 | `alloc_trace.rs:46` | Thin atomic-counter wrapper over `System`; ~2ns overhead; Miri-verified |
| macOS `sysctlbyname` | 1 | `diagnostics.rs:111` | Two-pass length query + buffer write |
| POSIX `getrusage` | 2 | `usagestat.rs:85`, `cpustat.rs:198` | Read-only syscall into zeroed struct |
| POSIX `uname` | 1 | `envstat.rs:95` | Reads fixed char array |
| POSIX `time`/`localtime_r` | 3 | `interactive/adaptive.rs:136, 145, 150` | Thread-safe time APIs; `MaybeUninit`+`assume_init` after non-NULL |
| Linux `madvise(MADV_DONTNEED)` | 1 fn + 2 calls | `interactive/adaptive.rs:218`, `event_loop.rs:604, 1239` | Best-effort; null/zero-length guarded |

**Total**: 15 distinct sites + 1 `unsafe fn` definition.

A `docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md` (2026-08-04) confirms a
manual + Miri review of all 15 sites, finding **0 unsound, 0 fixes needed**.

### Custom allocator note

`TraceAlloc` (`src/alloc_trace.rs:46`) is wired as the global allocator
(`src/main.rs:42-43`) and is **always active** in production builds (not
benchmark-gated). However, the implementation is a thin wrapper around
`std::alloc::System` that only adds `AtomicU64::fetch_add(..,
Ordering::Relaxed)` calls — overhead is ~2 ns per allocation, no
synchronization, no I/O, no syscalls beyond what `System` already does.
The counters are only **read** by the benchmark subsystem; if no
benchmark runs, the counters just accumulate harmlessly. This is not a
security concern.

---

## 2. Network Access — Opt-in Only, No Telemetry

**No network dependencies in `Cargo.toml`.** Searched for `std::net`,
`reqwest`, `ureq`, `hyper`, `tokio::net`, `TcpStream`, `UdpSocket`,
`http::`, `HttpClient`, `fetch`, `telemetry`, `analytics`. Zero hits in
source.

The **only** network code is `src/update.rs`:

- **Trigger**: `cosmostrix --update` flag only. Never called from any
  startup path, background timer, or interactive event loop.
- **Mechanism**: shells out to system `curl` binary with `--silent
  --max-time 15` and `User-Agent: cosmostrix`.
- **Request**: `GET https://api.github.com/repos/oxyzenQ/cosmostrix/releases/latest`
  with `Accept: application/vnd.github+json` header.
- **Outbound data**: NONE. No query parameters, no body, no cookies, no
  auth tokens, no client identifiers beyond the literal string "cosmostrix".
- **Response handling**: parses `tag_name` from JSON, prints up-to-date
  or update-available status. No download, no auto-update.

`docs/SYSTEM_REQUIREMENTS.md:254` documents: "Network — fully offline, no
telemetry or update checks by default".

---

## 3. Filesystem Access — Strict Whitelist

`src/safepath.rs:101` `is_safe_path()` is the **only** path-validation
primitive, applied uniformly to every CLI flag that reads or writes files.

### Reads (production)

| Path | Purpose | Risk |
|------|---------|------|
| `~/.config/cosmostrix/config.toml` | User config | None — user-owned |
| `/etc/cosmostrix/config.toml` | System config fallback | None — read-only |
| `/sdcard/cosmostrix/config.toml` | Termux config | None — read-only |
| `/proc/self/status`, `/proc/self/stat` | RSS + CPU sampling (Linux) | None — read-only OS introspection |
| `/proc/cpuinfo` | CPU model for `--doctor` (Linux) | None — read-only |
| `/sys/devices/system/cpu/...` | Benchmark env metadata (Linux) | None — read-only |
| `/sys/class/power_supply/...` | Energy benchmark (Linux) | None — read-only |
| `/dev/tty` | One-shot stdout fallback (SSH disconnect) | None — `O_WRONLY` only |
| `/dev/null` | Benchmark sink | None |
| `.git/HEAD`, `.git/packed-refs` | Build-time git SHA (`build.rs`) | None — never at runtime |
| `Cargo.toml` | Build-time profile detection (`build.rs`) | None — never at runtime |

### Writes (production)

| Path | Trigger | Conditions |
|------|---------|------------|
| `--dump-config <path>` | Writes example TOML | `is_safe_path` whitelist; `.toml` extension required; refuses overwrite; refuses shell redirection |
| `--save-baseline <path>` | Writes benchmark JSON | Same `is_safe_path` whitelist |

### Path-traversal hardening

`safepath.rs:101-225` is a strict **whitelist-only** validator:

- Rejects relative paths (`safepath.rs:129`).
- Rejects unexpanded `~/` if `HOME` is unset (`safepath.rs:112`).
- Lexically normalizes `..` and `.` segments before prefix matching
  (`safepath.rs:147`), so `/etc/cosmostrix/../../../tmp/leak.toml`
  resolves to `/tmp/leak.toml` and is rejected (`safepath.rs:512-521`).
- Whitelist (`safepath.rs:158-202`):
  - `~/.config/cosmostrix/`
  - `~/Library/Application Support/cosmostrix/` (macOS)
  - `/etc/cosmostrix/`
  - `/sdcard/cosmostrix/` (Termux)
  - `%APPDATA%\cosmostrix\` and `%ProgramData%\cosmostrix\` (Windows)

Test coverage is exhaustive (`safepath.rs:366-595`): `/etc/passwd`,
`/etc/shadow`, `~/.ssh/id_rsa`, `~/.aws/credentials`, `~/.bashrc`,
`~/.bash_history`, `~/.netrc`, `~/.env`, `/proc/self/environ`,
`/var/log/auth.log`, `/root/.bashrc`, `/opt/...`, `/usr/...`,
`/home/other-user/...` — all explicitly rejected.

**No writes outside `~/.config/cosmostrix/` or `XDG_CONFIG_HOME`.** No
log file, no cache directory, no state file, no PID file, no socket file.

---

## 4. Process Spawning — 4 Sites, All Defensive

| File:line | What it spawns | Why | Risk |
|-----------|---------------|-----|------|
| `src/update.rs:98` | `curl` (`--silent --max-time 15`) | `--update` flag only | None — no shell, explicit argv |
| `src/terminal.rs:1112` | `stty sane` | `--reset-terminal` flag only | None — best-effort recovery |
| `src/terminal.rs:1118` | `reset` | Same | None |
| `src/terminal.rs:1123` | `tput reset` | Same | None |
| `scripts/pgo-runner/src/main.rs:58` | `bash scripts/build.sh pgo --auto` | Dev convenience alias | None — not part of shipped binary |

**No `sh -c`, no `bash -c`, no `shell=true`** anywhere. Every spawn uses
an explicit argv with no shell interpolation.

The Linux-only `fork()` inside `main.rs:245` is **not**
`process::Command` — it is a raw `libc::fork()` that immediately calls
`prctl(PR_SET_PDEATHSIG)` and `sigwait()` in the child, never executing
any external program. It exists solely to restore terminal modes if the
parent is SIGKILLed.

---

## 5. Environment Variables — Read-only at Runtime

Reads only standard env vars: `HOME`, `XDG_CONFIG_HOME`, `TERM`,
`COLORTERM`, `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`, `NO_COLOR`,
`CLICOLOR`, `CLICOLOR_FORCE`, `SHELL`, `LANG`, `WT_SESSION`,
`TERMUX_VERSION`, `PREFIX`, plus project-prefixed `COSMOSTRIX_*` tunables.

**No production env writes.** All `std::env::set_var` / `env::remove_var`
calls are inside `#[cfg(test)]` modules. The project does **not** mutate
the parent environment.

---

## 6. Terminal Escape Sequences — Standard, Write-only

All emitted sequences (verified by grep for `\x1b`):

| Sequence | Meaning | Frequency |
|----------|---------|-----------|
| `\x1b[?2026h`/`l` | Synchronized output (disabled for VSCode + Linux console) | Per frame (when enabled) |
| `\x1b[?1049h`/`l` | Alternate screen | Start/end only |
| `\x1b[?25h`/`l` | Cursor show/hide | Start/end only |
| `\x1b[?7h`/`l` | Auto-wrap | Start/end only |
| `\x1b[?2004h`/`l` | Bracketed paste | Start/end only |
| `\x1b[?1000-1006h`/`l` | Mouse reporting | Start/end only |
| `\x1b[?1004h`/`l` | Focus events | Start/end only |
| `\x1b[2J`, `\x1b[3J` | Clear screen/scrollback | Reset only |
| `\x1b[<row>;<col>H` | Cursor positioning | Per dirty row |
| `\x1b[38;2;R;G;Bm` | Truecolor SGR | Per style change (RLE-batched) |
| `\x1b[1m`/`\x1b[22m` | Bold on/off | Per bold change |
| `\x1b[0m` | SGR reset | End of every frame |

**No DCS, no OSC, no DA1/DA2/DA3 queries, no DECRQM, no clipboard access,
no working-directory queries.** The renderer is **write-only** to the
terminal — it never asks the terminal for state.

---

## 7. Signal Handlers — Defensive, Well-scoped

`src/interactive/signal_handlers.rs`:

- **Unix**: `SIGTERM`/`SIGHUP`/`SIGQUIT` → set `GRACEFUL_SHUTDOWN` and
  `signal_exit` atomic flags, wait up to 3 s for main loop to clean up.
- **SIGTSTP/SIGCONT**: disable mouse capture, restore terminal, raise
  `SIGSTOP` for proper Ctrl+Z suspend.
- **SIGINT deliberately NOT handled** — only `q` exits cosmostrix
  (documented v25.13 policy).
- **Windows**: `ctrlc::set_handler` for CTRL_C_EVENT + CTRL_BREAK_EVENT.

**Cleanup on exit**: RAII `Terminal::drop` + 2-second watchdog thread +
panic hook that restores terminal BEFORE printing the panic message.

---

## 8. Dependency Audit — 11 Direct Deps, All Mainstream

| Crate | Version | Purpose | Network? | Crypto? |
|-------|---------|---------|----------|---------|
| `clap` | ≥4.5, <4.6 | CLI parser | No | No |
| `crossterm` | 0.29 | Terminal abstraction | No | No |
| `rand` | 0.9 | RNG | No | No |
| `bitvec` | 1 | Bit-slice for dirty-cell map | No | No |
| `smallvec` | 1 | Inline Vec | No | No |
| `unicode-width` | 0.2 | Glyph width | No | No |
| `chrono` | ≥0.4.38, <0.5 | Wall-clock hour | No | No |
| `notify` | ≥6.1, <7 | Config live-reload watcher | No | No |
| `signal-hook` (Unix) | 0.3 | Signal handling | No | No |
| `libc` (Unix) | 0.2 | FFI for syscalls | No | No |
| `ctrlc` (Windows) | 3.4 | Ctrl-C handling | No | No |

**No crypto crates. No HTTP client crates. No TLS crates. No
filesystem-walk crates. No subprocess-management crates. No async
runtimes.** Every direct dependency is mainstream and matches the stated
purpose of a terminal renderer.

### Feature-flag minimality (already optimized in Dragon Hunt v2 Phase 4)

- `clap`: `default-features = false`, only `std`, `color`, `help`,
  `usage`, `error-context`, `derive` (item 26, commit `5d40a9d`).
- `crossterm`: `default-features = false`, only `bracketed-paste`,
  `events`, `windows` (item 25, commit `4879585`).
- `chrono`: `default-features = false`, only `clock` (no `serde`).
- `notify`: `default-features = false`; macOS `macos_fsevent`, BSD `kqueue`.

### License policy

`deny.toml` enforces a license allowlist (Apache-2.0, MIT, GPL-3.0-only,
BSD-2/3-Clause, ISC, Unicode-3.0, Zlib, MPL-2.0, CC0-1.0) and
`cargo deny check all` runs in CI (`maintenance.yml:104`).

---

## 9. External Scripts — No Suspicious Behavior

14 scripts in `scripts/`. Audited each for network and filesystem
operations:

- **`install.sh`**: `cargo build` then `install -Dm755` to
  `~/.local/bin/` or `/usr/bin/` (sudo only with `--system`); refuses to
  run as root.
- **`uninstall.sh`**: removes binary; `--purge` removes config dirs.
- **`build.sh`**: runs `cargo` + optional `cargo audit`; `target/` only.
- All others: read-only or write to `target/`, `benchmark/`, `logs/`,
  or in-repo files.

**No script downloads binaries, no script curls to bash, no script pipes
network output to a shell.**

---

## 10. CI/CD — First-party Actions, No curl-pipe-bash

6 workflows in `.github/workflows/`.

### Network calls in CI

| Workflow | Call | Assessment |
|----------|------|------------|
| `ci.yml:462` | `curl https://sh.rustup.rs \| sh` | Official rustup install for FreeBSD VM; TLS 1.2 enforced |
| `release.yml:1032` | `curl -X POST .../repository_dispatches` | GitHub API to same repo; authenticated; no third-party endpoint |

### Third-party actions

All are first-party GitHub Actions or widely-used community actions with
pinned major versions: `actions/checkout@v6.0.2`,
`actions/upload-artifact@v7.0.1`, `dtolnay/rust-toolchain@stable`,
`Swatinem/rust-cache@v2.9.1`, `taiki-e/install-action@v2` (installs
`cargo-audit`/`cargo-deny` — official RustSec tools),
`vmactions/freebsd-vm@v1`, `softprops/action-gh-release@v3.0.0`.

### AUR SSH deployment

Pushes to `aur.archlinux.org` using `AUR_SSH_PRIVATE_KEY` secret. Host
key is **pinned** (`aur.yml:258`). StrictHostKeyChecking=yes,
IdentitiesOnly=yes. SSH key cleaned up in `always()` step. Only
`PKGBUILD` and `.SRCINFO` are committed.

### Two-phase privilege separation (maintenance.yml)

`validate` job uses read-only token; `commit` job uses write token but
is restricted to modifying only `Cargo.lock` (`maintenance.yml:91, 166`).

---

## 11. `build.rs` — Pure Metadata

791 lines. Reads `.git/HEAD`, `.git/packed-refs`, `Cargo.toml` only to
extract build metadata (git SHA, rustc version, profile). Writes only
`cargo:rustc-env=...` and `cargo:rerun-if-changed=...` directives.
**No network calls. No file writes outside `OUT_DIR`. No subprocess
spawns.**

---

## 12. VSCode/Electron Crash Fix (This Commit)

**Problem**: After running cosmostrix for hours inside VSCode's
integrated terminal, the `code-oss` (Electron) process hangs, CPU goes
to 100%, then crashes with Signal 5 (SIGTRAP). Coredump from
2026-08-04 23:48 WIB.

**Root cause**: cosmostrix had zero awareness that it was running inside
VSCode. It enabled mode 2026 (synchronized output) unconditionally and
pumped ANSI bytes at 60 FPS (0.3-13.7 MB/sec) into node-pty → xterm.js,
whose in-memory buffer grows without bound over multi-hour runs until
V8 hits an OOM assertion → SIGTRAP.

**Tier 1 Fix** (3 layers, defense-in-depth):

1. **VSCode detection** (`src/termdetect.rs`): read `TERM_PROGRAM=vscode`,
   set `vscode_integrated: bool` on `TerminalCaps`.
2. **Disable sync_output for VSCode**: xterm.js's mode 2026 buffer
   amplifies memory pressure.
3. **FPS cap**: VSCode gets 30 FPS max (vs 240 for native terminals).
   The cap is disclosed via warning + verbose output, not silently
   applied. Benchmark mode skips the cap.
4. **Write-latency backpressure** (`src/terminal.rs` + `src/interactive/event_loop.rs`):
   time each `write_all` call; if a write takes >50% of the frame
   period, feed it into `perf_pressure` so the self-healer downgrades
   the scene before the consumer OOMs.

**Verification**: build clean (zero warnings), 1511 tests pass, clippy
clean. The fix is transparent to native terminals (Alacritty, Kitty,
etc.) — they get the same 240 FPS cap and sync_output enabled as before.

### 12a. Tier 2 Extension — xterm.js Host Generalization

**Problem**: Tier 1 only covered VSCode. The same xterm.js OOM failure
mode applies to every Electron-based terminal that embeds xterm.js as
its renderer: Hyper, WaveTerminal, Tabby, WarpTerminal. Users running
cosmostrix inside these hosts were silently unprotected.

**Tier 2 Fix** (4 layers, extends Tier 1):

1. **Multi-host detection** (`src/termdetect.rs`): `vscode_integrated`
   becomes a back-compat alias; new primary signal is `xtermjs_host: bool`
   which is true for ANY of: `vscode`, `Hyper`, `WaveTerminal`, `Tabby`,
   `WarpTerminal`. The `XTERMJS_HOSTS` const list is the single source
   of truth — adding a future host is a one-line change.

2. **Byte-budget backpressure** (`src/terminal.rs::flush_ansi` + new
   `ByteWindow` ring buffer): Tier 1's FPS cap bounds the
   *instantaneous* byte rate but not the *cumulative* bytes that
   accumulate in xterm.js's scrollback buffer. Tier 2 adds a rolling
   window (`XTERMJS_BYTE_BUDGET_WINDOW_FRAMES` = 600 frames ≈ 20 s at
   the 30 FPS cap) with a per-window budget
   (`XTERMJS_BYTE_BUDGET_PER_WINDOW` = 40 MB). When the window sum
   exceeds the budget, `flush_ansi` suppresses the next flush entirely
   (state still advances externally, so the rain animation continues
   internally — only the ANSI write is suppressed). Suppressed frames
   push a 0-byte entry into the window, aging out old high-byte entries
   so the budget naturally recovers.

3. **Periodic RIS reset** (`src/terminal.rs::emit_ris_reset`): the
   SIGHUP-like recovery. When cumulative bytes since the last reset
   cross `XTERMJS_RIS_RESET_BYTES` (50 MB), emit `ESC c` (RIS — Reset
   to Initial State) which forces xterm.js to clear its in-memory
   scrollback buffer. The RIS sequence is followed by re-entering the
   alternate screen, re-hiding the cursor, and re-enabling SGR mouse
   mode — defensive against stricter terminals that fully reset on
   RIS. After emission, both `bytes_since_ris` and `byte_window` are
   reset since the buffer they were tracking has been nuked.

4. **Hard ceiling** (`XTERMJS_HARD_CEILING_BYTES` = 200 MB): a
   defensive last-resort. If the RIS reset fails to fire (e.g., a
   single 250 MB full-redraw frame skips the cumulative check), the
   hard ceiling forces a RIS regardless of window-budget state. Should
   never fire in practice — RIS at 50 MB fires first — but exists as
   a belt-and-suspenders bound against pathological cases.

**`--perf-stats` integration**: Tier 2 stats are reported in a new
`TIER2_XTERMJS` section of the perf-stats exit summary:
- `backpressure_skips`: number of flushes suppressed by the byte budget.
- `ris_resets`: number of ESC c emissions.
- `bytes_since_last_ris`: cumulative bytes since the last RIS.

All three are 0 on native terminals; nonzero only inside xterm.js hosts.
Useful for diagnosing whether the multi-hour OOM crash mode is actually
being mitigated.

**Verification**: build clean (zero warnings), tests pass (Tier 2 added
4 termdetect tests + 8 ByteWindow/flush tests). Native terminals see
zero behavioral change — all Tier 2 paths are gated on
`term_caps.xtermjs_host`.

**Threshold sizing rationale** (all sized for the 30 FPS Tier 1 cap,
~7 MB/sec worst case):

| Constant | Value | Fires roughly every |
| --- | --- | --- |
| `XTERMJS_BYTE_BUDGET_PER_WINDOW` | 40 MB | 5 s sustained max load (then suppresses) |
| `XTERMJS_RIS_RESET_BYTES` | 50 MB | 7 s sustained max load |
| `XTERMJS_HARD_CEILING_BYTES` | 200 MB | never (RIS at 50 MB fires first) |
| `XTERMJS_BYTE_BUDGET_WINDOW_FRAMES` | 600 frames | 20 s rolling window at 30 FPS |

---

## 13. Recommended Ongoing Security Practices

1. **Run `cargo audit` weekly** (already automated in
   `gitbot-audit.yml` daily run).
2. **Run `cargo deny check all`** before each release (already in
   `maintenance.yml`).
3. **Pin transitive deps** when upstream `notify` v7 lands (currently
   3 duplicate-version warnings, all Windows-only, documented in
   `deny.toml:47-64`).
4. **Consider replacing `curl` subprocess in `--update`** with `ureq`
   (compiled-out by default) so users don't need to trust whatever
   `curl` binary is on `PATH`. Defense-in-depth, not a vulnerability.
5. **Re-audit `unsafe` sites** when adding new FFI (the policy forbids
   new `unsafe` in renderer/core paths).

---

## Cross-References

- `docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md` — detailed `unsafe` review
- `docs/SIMD_FEASIBILITY.md` — "no new unsafe" policy rationale
- `docs/RULES.md` — project rules including unsafe policy
- `docs/SUPPLY_CHAIN.md` — supply chain security notes
- `docs/STABILITY_AUDIT.md` — four-layer cleanup audit
- `docs/ENDURANCE.md` — long-running stability methodology
- `docs/TERMINAL_COMPATIBILITY.md` — terminal compatibility matrix
- `deny.toml` — license + advisory policy
- `.github/workflows/gitbot-audit.yml` — daily `cargo audit` + `cargo deny`
- `.github/workflows/maintenance.yml` — two-phase dep auto-update
