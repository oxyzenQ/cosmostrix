<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Security Audit — Cosmostrix "Dragon Hunt Security Mode"

**Audit date**: 2026-08-05 · **Subject**: `cosmostrix` · **Scope**: full source tree (`src/`, `build.rs`, `scripts/`, `.github/workflows/`, `aur/`) · **Methodology**: automated `rg` sweeps for every sensitive capability class + manual review.

## Verdict

**Cosmostrix is a safe "digital art" program, not malware.** Every sensitive capability (network, subprocess, terminal mutation) is either opt-in (`--update`, `--reset-terminal`) or strictly bounded (path whitelist, FFI-only `unsafe` with soundness audits, signal handlers that only set atomic flags). The codebase demonstrates a mature security posture: documented `unsafe` policy, exhaustive path-traversal tests, license-policy enforcement, daily `cargo audit`, two-phase privilege separation for dep updates, and a self-audit document covering every `unsafe` site.

## 1. `unsafe` Usage — 15 Sites, All Sound

Documented **"no new unsafe in renderer/core paths"** policy (`docs/SIMD_FEASIBILITY.md` §1, §2.3; `docs/RULES.md`). Every `unsafe` site is FFI into `libc`/Mach and carries a `// SAFETY:` comment. **No `unsafe` in the renderer hot path** (verified by `info.rs:303`). **Total**: 15 distinct sites + 1 `unsafe fn` definition.

**By category**: macOS Mach `task_info` (2 sites: `memstat.rs:111`, `cpustat.rs:137`); Linux `libc::stat`/`fstat` (2: `main.rs:218, 221`); Linux fork-based SIGKILL guard (1: `main.rs:245` — `prctl(PR_SET_PDEATHSIG)` + `sigwait`, opt-out `COSMOSTRIX_NO_FORK_GUARD=1`); `perf_event_open` Linux bench-only (3: `bench_perf.rs:86, 124, 138`); `/dev/null` write test-only (2: `cosmic_dragon_engine/egg/io_uring_rejected.rs:68, 84` — `#[cfg(test)]`); custom allocator (1: `alloc_trace.rs:46` — thin atomic-counter wrapper over `System`, ~2ns overhead, Miri-verified); macOS `sysctlbyname` (1: `diagnostics.rs:111`); POSIX `getrusage` (2: `usagestat.rs:85`, `cpustat.rs:198`); POSIX `uname` (1: `envstat.rs:95`); POSIX `time`/`localtime_r` (3: `interactive/adaptive.rs:136, 145, 150` — thread-safe, `MaybeUninit`+`assume_init` after non-NULL); Linux `madvise(MADV_DONTNEED)` (1 fn + 2 calls: `interactive/adaptive.rs:218`, `event_loop.rs:604, 1239` — best-effort, null/zero-length guarded).

`docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md` (2026-08-04) confirms manual + Miri review of all 15 sites: **0 unsound, 0 fixes needed**. **Custom allocator note**: `TraceAlloc` (`src/alloc_trace.rs:46`) is wired as the global allocator (`src/main.rs:42-43`) and is always active in production builds. Thin wrapper around `std::alloc::System` that only adds `AtomicU64::fetch_add(.., Ordering::Relaxed)` calls — ~2 ns per allocation, no synchronization, no I/O, no syscalls beyond what `System` already does. Counters only read by the benchmark subsystem; if no benchmark runs, they accumulate harmlessly. Not a security concern.

## 2. Network Access — Opt-in Only, No Telemetry

**No network dependencies in `Cargo.toml`.** The **only** network code is `src/update.rs`: triggered by `cosmostrix --update` flag only (never from startup, background timer, or interactive event loop); shells out to system `curl` binary with `--silent --max-time 15` and `User-Agent: cosmostrix`; `GET https://api.github.com/repos/oxyzenQ/cosmostrix/releases/latest` with `Accept: application/vnd.github+json` header. **Outbound data**: NONE — no query parameters, no body, no cookies, no auth tokens, no client identifiers beyond the literal string "cosmostrix". Response handling parses `tag_name` from JSON, prints up-to-date or update-available status. No download, no auto-update. `docs/SYSTEM_REQUIREMENTS.md:254` documents: "Network — fully offline, no telemetry or update checks by default".

## 3. Filesystem Access — Strict Whitelist

`src/safepath.rs:101` `is_safe_path()` is the **only** path-validation primitive, applied uniformly to every CLI flag that reads or writes files.

**Reads (production)**: `~/.config/cosmostrix/config.toml` (user config), `/etc/cosmostrix/config.toml` (system fallback), `/sdcard/cosmostrix/config.toml` (Termux), `/proc/self/status` + `/proc/self/stat` (RSS + CPU sampling), `/proc/cpuinfo` (CPU model for `--doctor`), `/sys/devices/system/cpu/...` (benchmark env metadata), `/sys/class/power_supply/...` (energy benchmark), `/dev/tty` (one-shot stdout fallback for SSH disconnect, `O_WRONLY` only), `/dev/null` (benchmark sink), `.git/HEAD` + `.git/packed-refs` + `Cargo.toml` (build-time only, never at runtime). All read-only, none user-controlled.

**Writes (production)**: `--dump-config <path>` writes example TOML (`is_safe_path` whitelist, `.toml` extension required, refuses overwrite, refuses shell redirection). `--save-baseline <path>` writes benchmark JSON (same whitelist). **No writes outside `~/.config/cosmostrix/` or `XDG_CONFIG_HOME`.** No log file, no cache directory, no state file, no PID file, no socket file.

**Path-traversal hardening** (`safepath.rs:101-225`): strict whitelist-only validator. Rejects relative paths, unexpanded `~/` if `HOME` is unset. Lexically normalizes `..` and `.` segments before prefix matching — so `/etc/cosmostrix/../../../tmp/leak.toml` resolves to `/tmp/leak.toml` and is rejected. Whitelist: `~/.config/cosmostrix/`, `~/Library/Application Support/cosmostrix/` (macOS), `/etc/cosmostrix/`, `/sdcard/cosmostrix/` (Termux), `%APPDATA%\cosmostrix\` + `%ProgramData%\cosmostrix\` (Windows). Test coverage is exhaustive (`safepath.rs:366-595`): `/etc/passwd`, `/etc/shadow`, `~/.ssh/id_rsa`, `~/.aws/credentials`, `~/.bashrc`, `~/.bash_history`, `~/.netrc`, `~/.env`, `/proc/self/environ`, `/var/log/auth.log`, `/root/.bashrc`, `/opt/...`, `/usr/...`, `/home/other-user/...` — all explicitly rejected.

## 4. Process Spawning — 4 Sites, All Defensive

`src/update.rs:98` spawns `curl` (`--silent --max-time 15`) for `--update` flag only — no shell, explicit argv. `src/terminal/:1112`/`:1118`/`:1123` spawn `stty sane`/`reset`/`tput reset` for `--reset-terminal` flag only — best-effort recovery. `scripts/pgo-runner/src/main.rs:58` spawns `bash scripts/build.sh pgo --auto` — dev convenience alias, not part of shipped binary.

**No `sh -c`, no `bash -c`, no `shell=true`** anywhere. Every spawn uses explicit argv with no shell interpolation. The Linux-only `fork()` inside `main.rs:245` is NOT `process::Command` — it is a raw `libc::fork()` that immediately calls `prctl(PR_SET_PDEATHSIG)` and `sigwait()` in the child, never executing any external program. It exists solely to restore terminal modes if the parent is SIGKILLed.

## 5. Environment Variables + Terminal Escape Sequences

**Env vars**: reads only standard env vars — `HOME`, `XDG_CONFIG_HOME`, `TERM`, `COLORTERM`, `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`, `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, `SHELL`, `LANG`, `WT_SESSION`, `TERMUX_VERSION`, `PREFIX`, plus project-prefixed `COSMOSTRIX_*` tunables. **No production env writes** — all `std::env::set_var` / `env::remove_var` calls are inside `#[cfg(test)]` modules.

**Terminal escape sequences**: all emitted sequences (verified by grep for `\x1b`) are standard, write-only — `\x1b[?2026h/l` (synced output, disabled for VSCode + Linux console), `\x1b[?1049h/l` (alt screen), `\x1b[?25h/l` (cursor show/hide), `\x1b[?7h/l` (auto-wrap), `\x1b[?2004h/l` (bracketed paste), `\x1b[?1000-1006h/l` (mouse reporting), `\x1b[?1004h/l` (focus events), `\x1b[2J`/`\x1b[3J` (clear screen/scrollback), `\x1b[<row>;<col>H` (cursor positioning), `\x1b[38;2;R;G;Bm` (truecolor SGR), `\x1b[1m`/`\x1b[22m` (bold on/off), `\x1b[0m` (SGR reset). **No DCS, no OSC, no DA1/DA2/DA3 queries, no DECRQM, no clipboard access, no working-directory queries.** The renderer is **write-only** to the terminal.

## 6. Signal Handlers + Dependency Audit

**Signal handlers** (`src/interactive/signal_handlers.rs`): Unix `SIGTERM`/`SIGHUP`/`SIGQUIT` → set `GRACEFUL_SHUTDOWN` and `signal_exit` atomic flags, wait up to 3 s for main loop to clean up. `SIGTSTP`/`SIGCONT`: disable mouse capture, restore terminal, raise `SIGSTOP` for proper Ctrl+Z suspend. `SIGINT` deliberately NOT handled — only `q` exits cosmostrix. Windows: `ctrlc::set_handler` for CTRL_C_EVENT + CTRL_BREAK_EVENT. Cleanup on exit: RAII `Terminal::drop` + 2-second watchdog thread + panic hook that restores terminal BEFORE printing the panic message.

**Dependencies** — 11 direct deps, all mainstream. No crypto crates, no HTTP client crates, no TLS crates, no filesystem-walk crates, no subprocess-management crates, no async runtimes. Every direct dependency (clap, crossterm, rand, bitvec, smallvec, unicode-width, chrono, notify, signal-hook, libc, ctrlc) is mainstream and matches the stated purpose of a terminal renderer. Feature-flag minimality (Dragon Hunt v2 Phase 4): `clap`/`crossterm`/`chrono`/`notify` all have `default-features = false` with only the required features enabled. License policy: `deny.toml` enforces an allowlist (Apache-2.0, MIT, GPL-3.0-only, BSD-2/3-Clause, ISC, Unicode-3.0, Zlib, MPL-2.0, CC0-1.0) and `cargo deny check all` runs in CI.

## 7. External Scripts + CI/CD + build.rs

**External scripts** (14 in `scripts/`): `install.sh` runs `cargo build` then `install -Dm755` to `~/.local/bin/` or `/usr/bin/` (sudo only with `--system`); refuses to run as root. `uninstall.sh` removes binary; `--purge` removes config dirs. `build.sh` runs `cargo` + optional `cargo audit`. All others read-only or write to `target/`, `benchmark/`, `logs/`, or in-repo files. **No script downloads binaries, no script curls to bash, no script pipes network output to a shell.**

**CI/CD** (6 workflows in `.github/workflows/`): network calls are `ci.yml:462` `curl https://sh.rustup.rs | sh` (official rustup install for FreeBSD VM, TLS 1.2 enforced) and `release.yml:1032` `curl -X POST .../repository_dispatches` (GitHub API to same repo, authenticated, no third-party endpoint). All third-party actions are first-party GitHub Actions or widely-used community actions with pinned major versions. AUR SSH deployment uses pinned host key (`aur.yml:258`), StrictHostKeyChecking=yes, IdentitiesOnly=yes, SSH key cleaned up in `always()` step. Two-phase privilege separation (`maintenance.yml`): `validate` job uses read-only token; `commit` job uses write token but is restricted to modifying only `Cargo.lock`.

**build.rs** (791 lines): reads `.git/HEAD`, `.git/packed-refs`, `Cargo.toml` only to extract build metadata (git SHA, rustc version, profile). Writes only `cargo:rustc-env=...` and `cargo:rerun-if-changed=...` directives. **No network calls. No file writes outside `OUT_DIR`. No subprocess spawns.**

## 8. VSCode/Electron Crash Fix (Tier 1 + Tier 2)

**Problem**: After running cosmostrix for hours inside VSCode's integrated terminal, the `code-oss` (Electron) process hangs, CPU goes to 100%, then crashes with Signal 5 (SIGTRAP). Root cause: cosmostrix had zero awareness of running inside VSCode; it enabled mode 2026 (synchronized output) unconditionally and pumped ANSI bytes at 60 FPS (0.3-13.7 MB/sec) into node-pty → xterm.js, whose in-memory buffer grows without bound over multi-hour runs until V8 hits an OOM assertion → SIGTRAP.

**Tier 1 Fix** (3 layers): (1) VSCode detection (`src/termdetect/mod.rs` + `src/termdetect/hosts.rs`) reads `TERM_PROGRAM=vscode`, sets `vscode_integrated: bool` on `TerminalCaps`. (2) Disable sync_output for VSCode — xterm.js's mode 2026 buffer amplifies memory pressure. (3) FPS cap: VSCode gets 30 FPS max (vs 1–240 cap range on native terminals). Cap disclosed via warning + verbose output, not silently applied. Benchmark mode skips the cap. (4) Write-latency backpressure (`src/terminal/` + `src/interactive/event_loop.rs`): time each `write_all` call; if a write takes >50% of the frame period, feed it into `perf_pressure` so the self-healer downgrades the scene before the consumer OOMs.

**Tier 2 Extension** (xterm.js host generalization — applies to Hyper, WaveTerminal, Tabby, WarpTerminal): (1) Multi-host detection — `vscode_integrated` becomes a back-compat alias; new primary signal is `xtermjs_host: bool` (true for any of the listed hosts; `XTERMJS_HOSTS` const list is the single source of truth — adding a future host is a one-line change). (2) Byte-budget backpressure (`flush_ansi` + new `ByteWindow` ring buffer) — Tier 1's FPS cap bounds the instantaneous byte rate but not the cumulative bytes that accumulate in xterm.js's scrollback buffer. Tier 2 adds a rolling window (600 frames ≈ 20 s at 30 FPS cap) with a per-window budget (40 MB). When exceeded, `flush_ansi` suppresses the next flush entirely (state still advances, so the rain animation continues internally — only the ANSI write is suppressed). Suppressed frames push a 0-byte entry, aging out old high-byte entries so the budget naturally recovers. (3) Periodic RIS reset — when cumulative bytes since the last reset cross 50 MB, emit `ESC c` (RIS — Reset to Initial State) which forces xterm.js to clear its in-memory scrollback buffer. The RIS sequence is followed by re-entering the alternate screen, re-hiding the cursor, and re-enabling SGR mouse mode — defensive against stricter terminals that fully reset on RIS. (4) Hard ceiling (200 MB) — defensive last-resort; should never fire in practice (RIS at 50 MB fires first) but exists as a belt-and-suspenders bound against pathological cases.

`--perf-stats` integration: Tier 2 stats reported in a new `TIER2_XTERMJS` section — `backpressure_skips` (number of flushes suppressed), `ris_resets` (number of ESC c emissions), `bytes_since_last_ris` (cumulative bytes since last RIS). All three are 0 on native terminals; nonzero only inside xterm.js hosts. Verification: build clean (zero warnings), tests pass (Tier 2 added 4 termdetect tests + 8 ByteWindow/flush tests). Native terminals see zero behavioral change — all Tier 2 paths are gated on `term_caps.xtermjs_host`.

**Threshold sizing** (all sized for the 30 FPS Tier 1 cap, ~7 MB/sec worst case): `XTERMJS_BYTE_BUDGET_PER_WINDOW` = 40 MB (fires ~5 s sustained max load, then suppresses); `XTERMJS_RIS_RESET_BYTES` = 50 MB (fires ~7 s sustained max load); `XTERMJS_HARD_CEILING_BYTES` = 200 MB (never — RIS at 50 MB fires first); `XTERMJS_BYTE_BUDGET_WINDOW_FRAMES` = 600 frames (20 s rolling window at 30 FPS).

## 9. Recommended Ongoing Security Practices

1. Run `cargo audit` weekly (already automated in `gitbot-audit.yml` daily run).
2. Run `cargo deny check all` before each release (already in `maintenance.yml`).
3. Pin transitive deps when upstream `notify` v7 lands (currently 3 duplicate-version warnings, all Windows-only, documented in `deny.toml:47-64`).
4. Consider replacing `curl` subprocess in `--update` with `ureq` (compiled-out by default) so users don't need to trust whatever `curl` binary is on `PATH`. Defense-in-depth, not a vulnerability.
5. Re-audit `unsafe` sites when adding new FFI (the policy forbids new `unsafe` in renderer/core paths).

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
