<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Security Audit — cosmostrix "Dragon Hunt Security Mode"

> **SUPERSEDED (2026-08-23)**: This audit's "15 unsafe sites, all sound"
> verdict was built on an incorrect OS-semantics premise: it justified the
> madvise(MADV_DONTNEED) site as a "non-destructive hint" (§1, quoting the
> archived soundness audit). madvise(2) specifies zero-fill-on-demand for
> private anonymous mappings, and the raw-range call was a real cross-object
> heap-corruption hazard — found and fixed on 2026-08-23 as DPD-01 (commit
> `2210276`, interior-page confinement). Some file references are also stale
> (the adaptive.rs time FFI consolidated into clock/posix_time.rs). The
> capability-class analysis (network, filesystem, subprocess, environment)
> remains valid. The current, corrected reference is
> [`docs/archive/audits/SECURITY_VULNERABILITY_AUDIT.md`](archive/audits/SECURITY_VULNERABILITY_AUDIT.md).

**Audit date**: 2026-08-05 · **Subject**: `cosmostrix` · **Scope**: full source tree (`src/`, `build.rs`, `scripts/`, `.github/workflows/`, `aur/`) · **Methodology**: automated `rg` sweeps for every sensitive capability class + manual review.

## Verdict

**cosmostrix is a safe "digital art" program, not malware.** Every sensitive capability (network, subprocess, terminal mutation) is either opt-in (`--check-update`, `--reset-terminal`) or strictly bounded (path whitelist, FFI-only `unsafe` with soundness audits, signal handlers that only set atomic flags). The codebase demonstrates a mature security posture: documented `unsafe` policy, exhaustive path-traversal tests, license-policy enforcement, daily `cargo audit`, two-phase privilege separation for dep updates, and a self-audit document covering every `unsafe` site.

## 1. `unsafe` Usage — 15 Sites, All Sound

Documented **"no new unsafe in renderer/core paths"** policy (`docs/archive/SIMD_FEASIBILITY.md` §1, §2.3; `docs/RULES.md`). Every `unsafe` site is FFI into `libc`/Mach and carries a `// SAFETY:` comment. **No `unsafe` in the renderer hot path** (verified by `src/diagnostics/info.rs:300`). **Total**: 15 distinct sites + 1 `unsafe fn` definition.

**By category**: macOS Mach `task_info` (2 sites: `sysstat/memstat.rs:111`, `sysstat/cpustat.rs:137`); Linux `libc::stat`/`fstat` (2: `main.rs:218, 221`); Linux fork-based SIGKILL guard (1: `main.rs:245` — `prctl(PR_SET_PDEATHSIG)` + `sigwait`, opt-out `COSMOSTRIX_NO_FORK_GUARD=1`); `perf_event_open` Linux bench-only (3: `bench/bench_perf.rs:86, 124, 138`); `/dev/null` write test-only (2: `cosmic_dragon_incubator/egg/io_uring_rejected.rs:68, 84` — `#[cfg(test)]`); custom allocator (1: `diagnostics/alloc_trace.rs:46` — thin atomic-counter wrapper over `System`, ~2ns overhead, Miri-verified); macOS `sysctlbyname` (1: `diagnostics/info.rs:111`); POSIX `getrusage` (2: `sysstat/usagestat.rs:85`, `sysstat/cpustat.rs:198`); POSIX `uname` (1: `sysstat/envstat.rs:95`); POSIX `time`/`localtime_r` (3 sites, moved to `clock/posix_time.rs:76, 80, 89` by the Hinnant-style clock refactor — thread-safe, `MaybeUninit`+`assume_init` after non-NULL); Linux `madvise(MADV_DONTNEED)` (1 unsafe fn + 2 production call sites; the unsafe fn lives at `central_control_power_dragon/reclaim_state.rs:120`, invoked only from the `reclaim_frame_cells` helper at `reclaim_state.rs:238` — the madvise + `normalize_reclaimed_cells` + cooldown-mark bundle of NIGHT-hunt-43 — with the two event-loop callers at `interactive/event_loop_self_heal.rs:134` and `interactive/event_loop_adaptive.rs:71` — best-effort, null/zero-length guarded). (NIGHT-docs-audit 2026-09-12: paths re-pointed to the post-refactor locations; the historical `interactive/adaptive.rs:NNN` line refs moved with the clock and reclaim refactors. NIGHT-hunt-42 2026-09-14: refs re-pointed again after hunt-43 centralized both callers through `reclaim_frame_cells`.)

**(NIGHT-cybersecurity-1 re-inventory, 2026-09-19)**: the v100-era refactors — the terminal-module split (`terminal_tty.rs`, the Termux non-blocking write family), the `fork_guard.rs` extraction, the posix-time clock consolidation, `config_io.rs`'s stdout-fstat, and `watchdog.rs`'s isatty — grew the tree well past the 2026-08-05 snapshot above. Current truth: **47 `unsafe` occurrences across 14 files — 35 production + 12 test-gated** (classified by an occurrence-level sweep with test-module boundary detection; per-file production counts: `terminal_tty.rs` 3, `posix_time.rs` 7, `fork_guard.rs` 6, `alloc_trace.rs` 5, `cpustat.rs` 4, `reclaim_state.rs` 1 unsafe fn, `bench_perf.rs` 3, `config_io.rs` 2, `memstat.rs`/`envstat.rs`/`usagestat.rs`/`watchdog.rs`/`diagnostics/mod.rs` 1 each). Every production site was re-reviewed this pass and carries either an inline `// SAFETY:` comment or a `# Safety` doc section; the one gap found — `utc_tm()`'s `time`/`gmtime_r`/`assume_init` calls missing the `SAFETY` twins their `local_tm()` counterparts have — was restored in the same commit. No soundness issue was found in any reviewed site; the "15 sites" figure below is preserved as the historical snapshot it was. Future re-inventories should re-run the classification sweep rather than extend this paragraph by hand.

`docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md` (2026-08-04) confirms manual + Miri review of all 15 sites: **0 unsound, 0 fixes needed**. **Custom allocator note**: `TraceAlloc` (`src/diagnostics/alloc_trace.rs:46`) is wired as the global allocator (`src/main.rs:42-43`) and is always active in production builds. Thin wrapper around `std::alloc::System` that only adds `AtomicU64::fetch_add(.., Ordering::Relaxed)` calls — ~2 ns per allocation, no synchronization, no I/O, no syscalls beyond what `System` already does. Counters only read by the benchmark subsystem; if no benchmark runs, they accumulate harmlessly. Not a security concern.

## 2. Network Access — Opt-in Only, No Telemetry

**No network dependencies in `Cargo.toml`.** The **only** network code is `src/platform/update.rs`: triggered by `cosmostrix --check-update` flag only (never from startup, background timer, or interactive event loop); shells out to the system `curl` binary with `--silent --max-time 15` and `User-Agent: cosmostrix`, falling back to `wget -q -O - -T 15` when curl is not installed (NIGHT-hunter-7: curl-less systems such as Alpine/busybox or minimal containers); `GET https://api.github.com/repos/oxyzenQ/cosmostrix/releases/latest` with `Accept: application/vnd.github+json` header. **Outbound data**: NONE — no query parameters, no body, no cookies, no auth tokens, no client identifiers beyond the literal string "cosmostrix". Response handling parses `tag_name` from JSON, prints up-to-date or update-available status. No download, no auto-update. `docs/SYSTEM_REQUIREMENTS.md:254` documents: "Network — fully offline, no telemetry or update checks by default".

## 3. Filesystem Access — Strict Whitelist

`src/safepath/mod.rs` `is_safe_path()` is the **only** path-validation primitive, applied uniformly to every CLI flag that reads or writes files.

**Reads (production)**: `~/.config/cosmostrix/config.toml` (user config), `/etc/cosmostrix/config.toml` (system fallback), `/sdcard/cosmostrix/config.toml` (Termux), `/proc/self/status` + `/proc/self/stat` (RSS + CPU sampling), `/proc/cpuinfo` (CPU model for `--doctor`), `/sys/devices/system/cpu/...` (benchmark env metadata), `/sys/class/power_supply/...` (energy benchmark), `/dev/tty` (one-shot stdout fallback for SSH disconnect, `O_WRONLY` only), `/dev/null` (benchmark sink), `.git/HEAD` + `.git/packed-refs` + `Cargo.toml` (build-time only, never at runtime). All read-only, none user-controlled.

**Writes (production)**: `--dump-config <path>` writes example TOML (`is_safe_path` whitelist, `.toml` extension required, refuses overwrite, refuses shell redirection). `--save-baseline <path>` writes benchmark JSON (same whitelist). **No writes outside `~/.config/cosmostrix/` or `XDG_CONFIG_HOME`.** No log file, no cache directory, no state file, no PID file, no socket file.

**Path-traversal hardening** (`src/safepath/mod.rs:126` — `is_safe_path`, with `validate_config_path` at `:493`): strict whitelist-only validator. Rejects relative paths, unexpanded `~/` if `HOME` is unset. Lexically normalizes `..` and `.` segments before prefix matching — so `/etc/cosmostrix/../../../tmp/leak.toml` resolves to `/tmp/leak.toml` and is rejected. Whitelist: `~/.config/cosmostrix/`, `~/Library/Application Support/cosmostrix/` (macOS), `/etc/cosmostrix/`, `/sdcard/cosmostrix/` (Termux), `%APPDATA%\cosmostrix\` + `%ProgramData%\cosmostrix\` (Windows). Test coverage is exhaustive (`test/safepath/tests.rs`, 536 lines, wired from `src/safepath/mod.rs:535`): `/etc/passwd`, `/etc/shadow`, `~/.ssh/id_rsa`, `~/.aws/credentials`, `~/.bashrc`, `~/.bash_history`, `~/.netrc`, `~/.env`, `/proc/self/environ`, `/var/log/auth.log`, `/root/.bashrc`, `/opt/...`, `/usr/...`, `/home/other-user/...` — all explicitly rejected. (NIGHT-hunt-42 2026-09-14: refs re-pointed after the safepath split into `src/safepath/mod.rs` + `test/safepath/tests.rs`.)

## 4. Process Spawning — 4 Sites, All Defensive

`src/platform/update.rs` spawns `curl` (`--silent --max-time 15`), falling back to `wget` (`-q -O - -T 15`, busybox/GNU-compatible flags) only when curl is absent from PATH — both for the `--check-update` flag only, no shell, explicit argv, and an actionable error naming the manual releases URL when neither tool exists (NIGHT-hunter-7). `src/engine/cosmic_dragon_engine/terminal/restore.rs:259`/`:265`/`:270` spawn `stty sane`/`reset`/`tput reset` for `--reset-terminal` flag only — best-effort recovery. `pgo-runner/src/main.rs:58` spawns `bash scripts/build.sh pgo --auto` — dev convenience alias, not part of shipped binary. (NIGHT-hunt-42 2026-09-14: spawn-site refs re-pointed to `terminal/restore.rs` after the terminal-module split.)

**No `sh -c`, no `bash -c`, no `shell=true`** anywhere. Every spawn uses explicit argv with no shell interpolation. The Linux-only `fork()` inside `main.rs:245` is NOT `process::Command` — it is a raw `libc::fork()` that immediately calls `prctl(PR_SET_PDEATHSIG)` and `sigwait()` in the child, never executing any external program. It exists solely to restore terminal modes if the parent is SIGKILLed.

## 5. Environment Variables + Terminal Escape Sequences

**Env vars**: reads only standard env vars — `HOME`, `XDG_CONFIG_HOME`, `TERM`, `COLORTERM`, `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`, `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, `SHELL`, `LANG`, `WT_SESSION`, `TERMUX_VERSION`, `PREFIX`, plus project-prefixed `COSMOSTRIX_*` tunables. **No production env writes** — all `std::env::set_var` / `env::remove_var` calls are inside `#[cfg(test)]` modules.

**Terminal escape sequences**: all emitted sequences (verified by grep for `\x1b`) are standard, write-only — `\x1b[?2026h/l` (synced output, disabled for VSCode + Linux console), `\x1b[?1049h/l` (alt screen), `\x1b[?25h/l` (cursor show/hide), `\x1b[?7h/l` (auto-wrap), `\x1b[?2004h/l` (bracketed paste), `\x1b[?1000-1006h/l` (mouse reporting), `\x1b[?1004h/l` (focus events), `\x1b[2J`/`\x1b[3J` (clear screen/scrollback), `\x1b[<row>;<col>H` (cursor positioning), `\x1b[38;2;R;G;Bm` (truecolor SGR), `\x1b[1m`/`\x1b[22m` (bold on/off), `\x1b[0m` (SGR reset). **No DCS, no OSC, no DA1/DA2/DA3 queries, no DECRQM, no clipboard access, no working-directory queries.** The renderer is **write-only** to the terminal.

**(NIGHT-cybersecurity-1, 2026-09-19)**: the `--list-*`/`--show-scene` report family now carries the same sink discipline as the diagnostics. Proven with a hostile-config PoC on the pre-fix binary: `--show-scene` echoed a raw `ESC [2J` (od-verified `033` byte) from an unvalidated `scene-custom.<name>.rain` VALUE — config VALUES are not charset-gated at collection (source validation happens later, at cloud-config build time), and the report printers interpolated them raw. The custom charset/palette name loops and the hidden-block warning lines held the same class (their collectors gate length and key shape, not name charset). Fixed by routing the report builders through `escape_ctrl`: `show_custom_scene_text` / `list_custom_scenes_text` (whole-string sink in `scene_custom/display.rs`), the custom charset + palette name loops and `hidden_block_warning_lines` (`config/list_printers.rs`). Post-fix the same PoC renders as the visible `glyph\u001b[2Jx` literal. Scene NAMES were already source-gated (`is_valid_profile_name` rejects them at collection), and the config parser rejects control bytes in KEYS (name-vector probes did not pass end-to-end) — the name-side guards are therefore defense-in-depth, pinned by unit tests at the sink.

## 6. Signal Handlers + Dependency Audit

**Signal handlers** (`src/interactive/signal_handlers.rs`): Unix `SIGTERM`/`SIGHUP`/`SIGQUIT` -> set `GRACEFUL_SHUTDOWN` and `signal_exit` atomic flags, wait up to 3 s for main loop to clean up. `SIGTSTP`/`SIGCONT`: disable mouse capture, restore terminal, raise `SIGSTOP` for proper Ctrl+Z suspend. `SIGINT` deliberately NOT handled — only `q` exits cosmostrix. Windows: `ctrlc::set_handler` for CTRL_C_EVENT + CTRL_BREAK_EVENT. Cleanup on exit: RAII `Terminal::drop` + 2-second watchdog thread + panic hook that restores terminal BEFORE printing the panic message (NIGHT-hunter-4: only for main-thread panics — worker-thread panics are contained by each thread's `catch_unwind` + buffered diagnostics, so the hook no longer tears down the terminal mid-rain for recovered panics).

**Anti-copy / interaction surface (NIGHT-improve-8 + follow-up)**: the renderer exposes zero copy paths — no clipboard crate, no OSC 52 writes (the escape sink gate in `src/output/escape_ctrl.rs` rejects OSC 52 in untrusted strings; SECURITY section 5), and mouse capture is held for the entire session: enabled at startup (`event_loop_setup.rs`), re-asserted after SIGCONT terminal re-init (`event_loop.rs`), released only on exit/suspend paths (`terminal/cleanup.rs`, `terminal/restore.rs`, SIGTSTP handler — the suspend restore also leaves the alternate screen, so no frozen frame remains visible or selectable while the process is SIGSTOPped; SIGCONT re-inits and repaints). Bracketed paste is enabled purely as an input-safety wrapper: pasted content is structurally discarded (`Event::Paste(_)` is never read) while the burst guard (`PasteBurstGuard`, 50 ms window) keeps paste floods from reaching the keybinding dispatch. Modified mouse events — shift+click and every other modifier combination — are the one selection vector terminals keep local: most never forward them, and the minority that do are handled gesture-level by `is_selection_bypass_event()` (`src/interactive/input.rs`): the full selection motion (Down anchor, Drag extension, Up release, plus the modified Moved pre-gesture hover) receives zero visual acknowledgment — the hover glow is frozen, not just the click wave suppressed; the predicate runs before the hover-position update in the mouse arm — plus a full-frame redraw per forwarded bypass event, which erases freshly painted native selection highlights in terminals that clear selection state on grid updates and keeps the grid churning under the whole gesture so position-anchored selection copies (xterm-style, where copy reads the CURRENT cell content) capture moving rain glyphs instead of the text the user highlighted. Modified scroll is deliberately excluded: the wheel is not a selection primitive, and modifier bits on scroll kinds must not trigger spurious full redraws.

**Trust boundary (honest limits — the direct answer to "why is text still copyable")**: terminal emulators split into two classes, and only one of them is reachable by any TUI application. *Bypass terminals* (the mainstream default: xterm, VTE/GNOME Terminal and derivatives, kitty, WezTerm, Alacritty, iTerm2, Windows Terminal, foot) intercept modified clicks THEMSELVES for their local selection engine and never deliver the events to the application — no escape sequence exists that revokes a terminal's own selection engine, its select-all shortcut (Cmd/Ctrl+Shift+A), Ctrl+Shift+C copy, middle-click primary paste, screenshot tools, or a multiplexer's copy-mode (tmux/screen sit between the app and the terminal and own their own pane selection). In that class, "still able to copy" is terminal physics, not a cosmostrix gap; the only counters live outside the application (OS kiosk policies, screenshot-permission controls) or mean not rendering the text at all (graphics-protocol pixel output — deliberately out of scope for a text renderer). *Forwarding terminals* (a minority; the legacy Windows console input path is the notable case) deliver modified mouse events to the app, and there cosmostrix actively degrades every selection attempt as described above. What cosmostrix guarantees is everything inside its own boundary: mouse capture held for the whole run, no app-offered copy path, pasted content discarded, and zero visual acknowledgment plus grid churn for every selection gesture that actually reaches the application.

**Dependencies** — 11 direct deps, all mainstream. No crypto crates, no HTTP client crates, no TLS crates, no filesystem-walk crates, no subprocess-management crates, no async runtimes. Every direct dependency (clap, crossterm, rand, bitvec, smallvec, unicode-width, notify, sha2, signal-hook, libc, ctrlc) is mainstream and matches the stated purpose of a terminal renderer. Feature-flag minimality (Dragon Hunt v2 Phase 4): `clap`/`crossterm`/`notify` all have `default-features = false` with only the required features enabled. License policy: `deny.toml` enforces an allowlist (Apache-2.0, MIT, GPL-3.0-only, BSD-2/3-Clause, ISC, Unicode-3.0, Zlib, MPL-2.0, CC0-1.0) and `cargo deny check all` runs in CI.

## 7. External Scripts + CI/CD + build.rs

**External scripts** (14 in `scripts/`): `install.sh` runs `cargo build` then `install -Dm755` to `~/.local/bin/` or `/usr/bin/` (sudo only with `--system`); refuses to run as root. `uninstall.sh` removes binary; `--purge` removes config dirs. `build.sh` runs `cargo` + optional `cargo audit`. All others read-only or write to `target/`, `benchmark/`, `logs/`, or in-repo files. **No script downloads binaries, no script curls to bash, no script pipes network output to a shell.**

**CI/CD** (the workflows in `.github/workflows/`): network calls are `ci.yml` `curl https://sh.rustup.rs | sh` (official rustup install for FreeBSD VM, TLS 1.2 enforced) and `release.yml:1032` `curl -X POST .../repository_dispatches` (GitHub API to same repo, authenticated, no third-party endpoint). All third-party actions are first-party GitHub Actions or widely-used community actions with pinned major versions. AUR SSH deployment uses pinned host key (`aur.yml`), StrictHostKeyChecking=yes, IdentitiesOnly=yes, SSH key cleaned up in `always()` step. Two-phase privilege separation (`maintenance.yml`): `validate` job uses read-only token; `commit` job uses write token but is restricted to modifying only `Cargo.lock`.

**build.rs**: reads `.git/HEAD`, `.git/packed-refs`, `Cargo.toml` only to extract build metadata (git SHA, rustc version, profile). Writes only `cargo:rustc-env=...` and `cargo:rerun-if-changed=...` directives. **No network calls. No file writes outside `OUT_DIR`. No subprocess spawns.**

## 8. VSCode/Electron Crash Fix (Tier 1 + Tier 2)

**Problem**: After running cosmostrix for hours inside VSCode's integrated terminal, the `code-oss` (Electron) process hangs, CPU goes to 100%, then crashes with Signal 5 (SIGTRAP). Root cause: cosmostrix had zero awareness of running inside VSCode; it enabled mode 2026 (synchronized output) unconditionally and pumped ANSI bytes at 60 FPS (0.3-13.7 MB/sec) into node-pty -> xterm.js, whose in-memory buffer grows without bound over multi-hour runs until V8 hits an OOM assertion -> SIGTRAP.

**Tier 1 Fix** (3 layers): (1) VSCode detection (`src/termdetect/mod.rs` + `src/termdetect/hosts.rs`) reads `TERM_PROGRAM=vscode`, sets `vscode_integrated: bool` on `TerminalCaps`. (2) Disable sync_output for VSCode — xterm.js's mode 2026 buffer amplifies memory pressure. (3) FPS cap: VSCode gets 30 FPS max (vs 1–240 cap range on native terminals). Cap disclosed via warning + verbose output, not silently applied. Benchmark mode skips the cap. (4) Write-latency backpressure (`src/engine/cosmic_dragon_engine/terminal/` + `src/interactive/event_loop.rs`): time each `write_all` call; if a write takes >50% of the frame period, feed it into `perf_pressure` so the self-healer downgrades the scene before the consumer OOMs.

**Tier 2 Extension** (xterm.js host generalization — applies to Hyper, WaveTerminal, Tabby, WarpTerminal): (1) Multi-host detection — `vscode_integrated` becomes a back-compat alias; new primary signal is `xtermjs_host: bool` (true for any of the listed hosts; `XTERMJS_HOSTS` const list is the single source of truth — adding a future host is a one-line change). (2) Byte-budget backpressure (`flush_ansi` + new `ByteWindow` ring buffer) — Tier 1's FPS cap bounds the instantaneous byte rate but not the cumulative bytes that accumulate in xterm.js's scrollback buffer. Tier 2 adds a rolling window (600 frames ≈ 20 s at 30 FPS cap) with a per-window budget (40 MB). When exceeded, `flush_ansi` suppresses the next flush entirely (state still advances, so the rain animation continues internally — only the ANSI write is suppressed). Suppressed frames push a 0-byte entry, aging out old high-byte entries so the budget naturally recovers. (3) Periodic RIS reset — when cumulative bytes since the last reset cross 50 MB, emit `ESC c` (RIS — Reset to Initial State) which forces xterm.js to clear its in-memory scrollback buffer. The RIS sequence is followed by re-entering the alternate screen, re-hiding the cursor, and re-enabling SGR mouse mode — defensive against stricter terminals that fully reset on RIS. (4) Hard ceiling (200 MB) — defensive last-resort; should never fire in practice (RIS at 50 MB fires first) but exists as a belt-and-suspenders bound against pathological cases.

`--perf-stats` integration: Tier 2 stats reported in a new `TIER2_XTERMJS` section — `backpressure_skips` (number of flushes suppressed), `ris_resets` (number of ESC c emissions), `bytes_since_last_ris` (cumulative bytes since last RIS). All three are 0 on native terminals; nonzero only inside xterm.js hosts. Verification: build clean (zero warnings), tests pass (Tier 2 added 4 termdetect tests + 8 ByteWindow/flush tests). Native terminals see zero behavioral change — all Tier 2 paths are gated on `term_caps.xtermjs_host`.

**Threshold sizing** (all sized for the 30 FPS Tier 1 cap, ~7 MB/sec worst case): `XTERMJS_BYTE_BUDGET_PER_WINDOW` = 40 MB (fires ~5 s sustained max load, then suppresses); `XTERMJS_RIS_RESET_BYTES` = 50 MB (fires ~7 s sustained max load); `XTERMJS_HARD_CEILING_BYTES` = 200 MB (never — RIS at 50 MB fires first); `XTERMJS_BYTE_BUDGET_WINDOW_FRAMES` = 600 frames (20 s rolling window at 30 FPS).

## 9. Time-Scale Input Ceiling — 24h Hard Limit (S-master-HUNT-5)

**Owner security mandate (2026-09-03):** every flag that accepts a
time-scale value is hard-capped at **24 hours (86,400s)**. Rationale:
cosmostrix is a courteous guest on the host OS — a flag-requested run
longer than a day would hold CPU and terminal resources indefinitely
(performance leakage). Before this cap, `--bench-duration 222h`
launched a benchmark with no wall-clock bound (verified empirically —
the run had to be timeout-killed), and `--bench-frames 999999999999`
could hold the CPU for ~190 years.

| Surface | Enforcement | Behavior at the ceiling |
|---------|-------------|--------------------------|
| `--bench-duration` | `cli_parse::parse_duration` → `validate_secs` | `24h`/`1d` valid (exactly 86400); anything above rejected with the policy reason |
| `--duration` | `cli_parse::parse_secs_f64` (structurally) + prevalidator range 0.0..=86400 (0 = disable sentinel) | Same; `0` keeps its documented "disables auto-exit" meaning (a prevalidator regression that rejected `--duration 0` was fixed alongside) |
| `--crystal-dragon-secs` | `parse_secs_f64` (clap value_parser) + post-parse gate 0..=86400 | Same; day-unit values work below the cap (`0.5d` = 12h) |
| `ambient-snapback-secs` (config key) | `parse_secs_config(0.0, 86400.0)` — unchanged range, now double-enforced by the parser ceiling | Same |
| `--bench-frames` (frame count, not a duration) | 24h wall-clock watchdog in the frames loop (checked every 4096 frames, ~ns amortized) | Loop stops at 24h with a disclosed `watchdog:` report line; the FPS denominator uses the frames actually run |

The ceiling lives INSIDE both duration parsers (not at call sites), so
no future flag or config key can accidentally bypass it. Day (`d`) and
week (`w`) units are parseable so over-limit inputs like `2d`/`1w` are
rejected with the real reason (the 24h policy) instead of a misleading
"unknown unit" — while sub-ceiling day values remain expressible.
Calendar units (`mo`/`y`) are deliberately NOT input grammar (their
lengths are not fixed elapsed-time units; the HUD renders them on the
display side via `clock::format_uptime_tiered`).

Interactive sessions WITHOUT `--duration` are user-supervised (any key
quits) and remain unbounded by design — the owner's server-class
multi-day uptime use case (`up: 1mo:1d:22h:10m`) is a supported display
tier, not a policy violation. The cap governs flag-REQUESTED timed
behavior only.

## 10. Recommended Ongoing Security Practices

1. Run `cargo audit` weekly (already automated in `gitbot-audit.yml` daily run).
2. Run `cargo deny check all` before each release (already in `maintenance.yml`).
3. Pin transitive deps when upstream `notify` v7 lands (currently 3 duplicate-version warnings, all Windows-only, documented in `deny.toml:47-64`).
4. Consider replacing `curl` subprocess in `--update` with `ureq` (compiled-out by default) so users don't need to trust whatever `curl` binary is on `PATH`. Defense-in-depth, not a vulnerability.
5. Re-audit `unsafe` sites when adding new FFI (the policy forbids new `unsafe` in renderer/core paths).

## 11. Running as Root — Wrong Use Case (NIGHT-security-4)

**Owner report (2026-09-21)**: `sudo cosmostrix -vV` and `sudo cosmostrix
--check-update` both ran silently — the config path switched to
`/root/.config/cosmostrix/config.toml` and the update check performed its
network fetch with uid 0 privileges, with zero indication that anything
about the trust boundary had changed.

**Policy**: cosmostrix is designed for regular (non-root) users. Running
it as root (`sudo`, `su`, setuid) is an **unsupported, high-risk wrong
use case** — it is not part of any documented workflow, and the runtime
guard below makes the mistake loud on every invocation. This section is
the canonical policy text (NIGHT-docs-8 tell-once rule: other docs cite
this section, they do not re-tell it).

**Why root execution is high-risk here** — every item below is attack
surface that only exists at euid 0:

1. **Root-owned config trust**: the path whitelist still applies, but the
   process now parses config as uid 0 — a root-private hostile
   `/root/.config/cosmostrix/config.toml` (planted by any earlier root
   compromise, invisible to user-level audits) drives charset, scene,
   and message values straight into a root-privileged process.
2. **Network as root**: `--check-update` shells out to `curl`/`wget`
   with uid 0 — PATH resolution, the TLS stack, and response parsing
   all run inside the root trust domain instead of the user's. CLOSED
   since the NIGHT-security-4 follow-up: the command now hard-refuses
   at euid 0 before any fetcher spawns (see the runtime guard below).
3. **Terminal escape output as root**: the renderer's ANSI byte stream
   is write-only and audited (section 5), but on a shared or forwarded
   root session every escape-handling surface becomes a root-level
   surface.
4. **Root-owned artifacts**: `--dump-config`/`--save-baseline` writes
   create root-owned files — the exact ownership-corruption class that
   makes `scripts/install.sh` refuse to run as root (section 7).

**Runtime guard** (`src/platform/root_guard.rs`): `libc::geteuid()` FFI —
the same libc-FFI family as `clock/posix_time.rs`, SAFETY-commented, no
new dependency (libc is already the unix target-gated dependency in
`Cargo.toml`). Effective UID is the ground truth, so `sudo -u <user>`
targets correctly do NOT warn. On every invocation with euid 0, after
argument parsing (clap error output stays clean) and before any command
output, one warning block is emitted to **stderr** — covering
`--version`, `--check-update`, `--doctor`, `--help`, benchmark, and the
interactive loop. stdout is never touched, so piped output stays clean.

The guard is **two-tier** (NIGHT-security-4 follow-up, 2026-09-21):

- **LOCAL surfaces** (interactive loop, config, `--version`, `--doctor`,
  `--help`, benchmark) stay advisory, warn-and-continue: container
  defaults legitimately run as euid 0, and a hard refusal would break
  them.
- **NETWORK egress** — the one root surface with no legitimate
  container case — hard-refuses: `--check-update` at euid 0 emits one
  stderr refusal block and exits 2 (the `cli/ux.rs` fatal-CLI
  contract) BEFORE any curl/wget spawn. The gate sits in the
  `--check-update` dispatch arm (`cli/early_returns.rs`, the sole
  caller of `platform/update.rs::check_update`), so no route to the
  network fetch can skip it. No override exists — no flag, no env
  var. Forced-root environments check releases from a user shell or
  the manual releases URL
  (`https://github.com/oxyzenQ/cosmostrix/releases/latest`).

**If you are forced to run as root anyway** (documented mitigation, in
order of preference):

1. Don't — drop back to a regular user first (`sudo -u <user>
   cosmostrix`, or run inside the user session).
2. Contain it — container/sandbox with dropped capabilities, read-only
   root filesystem, isolated `HOME`.
3. Never run `--check-update` as root — enforced since the follow-up:
   the command hard-refuses at euid 0 with exit 2. Check releases from
   a user shell or the manual releases URL instead.
4. Never share the root session or terminal with other users.
5. Treat root-owned config artifacts as suspect — audit
   `/root/.config/cosmostrix/` and `/etc/cosmostrix/` before relying
   on them.

**Honest limits**: the guard is unix-only — Windows has no euid (the
Administrator elevation model is a different trust boundary and out of
scope). It warns on local surfaces and deliberately does not block
them; the single network surface (`--check-update`) hard-refuses with
no override. A root run inside a container is indistinguishable from
a root run on a workstation, which is exactly why local warnings stay
advisory text rather than an exit — and why the network refusal is
unconditional: an update check has no container-workflow case that a
refusal could break.

## Cross-References

- `docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md` — detailed `unsafe` review
- `docs/archive/SIMD_FEASIBILITY.md` — "no new unsafe" policy rationale
- `docs/RULES.md` — project rules including unsafe policy
- `docs/SUPPLY_CHAIN.md` — supply chain security notes
- `docs/archive/STABILITY_AUDIT.md` — four-layer cleanup audit
- `docs/ENDURANCE.md` — long-running stability methodology
- `docs/TERMINAL_COMPATIBILITY.md` — terminal compatibility matrix
- `deny.toml` — license + advisory policy
- `.github/workflows/gitbot-audit.yml` — daily `cargo audit` + `cargo deny`
- `.github/workflows/maintenance.yml` — two-phase dep auto-update
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
