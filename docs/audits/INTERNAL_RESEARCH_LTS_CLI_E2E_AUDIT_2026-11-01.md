<!-- SPDX-License-Identifier: GPL-3.0-only -->

# internal-research LTS pre-landing audit — end-to-end real-usage verification (CLI surface, panic surface, unsafe/leak/growth)

Owner brief (2026-11-01): "master audit end to end real usage this
project to verify free from bug/leaks/problems before landing on LTS
production, if find a problems documented and fix. Include real usage
all existing cli/skill this project, e.g usage print json, and etc."

Scope: the non-interactive CLI/skill surface exercised as a real user
(headless binary, no TTY), plus two evidence sweeps over the input
reachability and resource-lifecycle surfaces (308 `.rs` files under
`src/`, ~110K LOC). Built with the pinned toolchain (rustc 1.98.1,
`cargo build --locked`, clean).

## Method

1. **Real-usage battery** (headless, exit codes + output shape checked
   for every probe): `--version`, `-h`, `--list-colors/--list-charsets/
   --list-scenes` (with and without `--json`), `--dump-config` (stdout
   pipe, whitelisted path, overwrite guard, outside-whitelist guard),
   `--config X --testconf` (valid template + out-of-whitelist
   rejection), `--doctor`, `--docs`, `--show-scene <name>` (valid +
   unknown-name hint), `--check-update`, `--benchmark --json` /
   `--benchmark` text (2s and 10s runs), human-duration forms
   (`--duration 90s`), and 11 invalid-input probes (`--fps 0`,
   `--density 99`, unknown color/charset/scene, `--screen-size 0x0`,
   `100000x100000`, unknown flag, bad `--color-tune` key, numeric on a
   boolean flag).
2. **Panic-surface sweep** (input-reachable only): every `unwrap`/
   `expect`/`panic!`/`unreachable!`/index/slice/underflow/division/
   `parse` site in `cli/`, `config/` (+ live-reload), `validation/`,
   `scene/`, `scene_custom/`, `termdetect/`, `safepath/`, and
   `bench/bench_json.rs`, triaged by reading the actual guard code.
3. **Resource-lifecycle sweep**: all 19 `unsafe` sites judged for
   soundness; fd-open cadences across `sysstat/`, `central_control_
   power_dragon/`, `interactive/hud/`, live-reload polling; terminal
   restore lattice (q/SIGTERM/SIGHUP/SIGQUIT/panic/watchdog/Ctrl-Z/
   fork guard); unbounded-growth scan over every buffer touched by the
   event loop, HUD, cloud, and diagnostics.

## Findings and dispositions

| # | Finding | Severity | Disposition |
|---|---------|----------|-------------|
| 1 | `config_io.rs` `write_config_atomic` early-returned on `write_all`/`sync_all`/`rename` errors with the `<target>.tmp.<pid>` temp left on disk, contradicting its own "best-effort cleanup on error" doc — each failed `--dump-config` stranded a temp file | LOW (real bug) | **Fixed** — error paths now unlink the temp; pinned by 2 regression tests (`test/config/config_io_tests/`, rename-failure via file-to-dir EISDIR + success path). Commit: temp-file cleanup. Benchmark A/B (10s, 80x24, debug): avg_fps +0.5%, density_gini 0.8962→0.8960, frame_entropy 3.294→3.296, dirty cells/frame +0.3% — parity, error-path-only change confirmed |
| 2 | `fork_guard.rs` module header, entry doc, and S4 note oversold the non-Linux Unix guard as covering cosmostrix's own SIGKILL/crash/OOM. The getppid poller is a same-process thread: it dies with the process and only recovers PARENT death. Contradicted `docs/TERMINAL_KILL_CLEANUP.md` ("macOS/BSD are not covered") | MEDIUM (doc truth, safety-relevant) | **Fixed** — three comment sites rescoped to per-platform reality, `--reset-terminal` named as the escape hatch, cross-referenced to the authoritative doc. Commit: fork guard doc scoping |
| 3 | `TERMINAL_KILL_CLEANUP.md`, `COSMIC_DRAGON_ARCHITECTURE.md`, `TERMINAL_LIFECYCLE_MATRIX.md` still promised the Phase 4B signal-exit viewport clear (`MoveTo(0,0)` + `Clear(All)` + flush before `LeaveAlternateScreen`) that the code removed to protect VTE scrollback (`with_signal_exit` no longer stores the flag; `signal_exit` has zero consumers in the cleanup path) | MEDIUM (doc drift, safety-relevant) | **Fixed** — five doc sites rewritten to the current behavior (signal-exit cleanup = byte-identical to `q` exit), cross-referenced. Commit: signal-exit docs sync. `docs_tests` 21/21 pass |
| 4 | Panic surface reachable from CLI args / config.toml / terminal responses | — | **Clean bill, 0 findings.** Every candidate site guarded (verified by reading each guard: section-header slice bounds, quote-strip bounds, Levenshtein caps, safepath ASCII-boundary slicing, live-reload strict revalidation before the render thread, `catch_unwind` + restart backoff on watcher threads). Five former live-panic sites are documented as defused in-code |
| 5 | `unsafe` inventory (19 sites) | — | **All sound.** Notables re-verified: `madvise(MADV_DONTNEED)` reclaim confined to interior full pages (unit-tested incl. overflow), `GlobalAlloc` wrapper re-entrancy-free with wrapping counters, `perf_event_open` Drop closes all fds, MaybeUninit init-before-`assume_init` discipline holds throughout |
| 6 | fd leaks / reopen cadence | — | **No leaks.** All `/proc`/sysfs reads open-and-drop per call at conservative rates (1-4 Hz max at the 240 FPS cap; thermal 0.1 Hz; live-reload steady state = one `stat()` per ~750 ms, hash only on mtime/size change). Only kept fd is the capped TTY fallback (`STDOUT_FALLBACK_MAX_RECOVERIES`) |
| 7 | Unbounded memory growth | — | **All capped.** Live-reload warning/diag buffers (64, deduped), debug traces (1000), HUD ring buffers ([60]), endurance RSS ring (60), droplet pool recycled, ghost events (`GHOST_MAX_ACTIVE`), ANSI buffers plateau at max frame size (grid clamped 1024x500) |
| 8 | Terminal restore lattice | — | **Every catchable exit path covered** (drop, signal, panic hook + skip-reenter flag, watchdog 1-3s raw-fd force-exit, Ctrl-Z/SIGCONT with double-cleanup race closed, constructor-failure pre-return cleanup, wedged-drop 2s shutdown guard). Remaining gaps are the documented ones: SIGKILL on non-Linux Unix (finding 2 doc truth now matches), Windows forced-termination (KNOWN_ISSUES) |
| 9 | `--benchmark --json` machine-readable contract | — | **Valid strict JSON** (jq-verified): NaN/Inf emit null, control chars escaped, user-derived strings (scene/charset/custom names) cannot break parseability. All 19 report sections present |
| 10 | UX nits, verified intentional, no action: `--json` silently ignored by `--list-*` (documented benchmark-only flag); `--crystal-dragon` help shows `<true|false>` while the parser accepts the 8-alias bool vocabulary (`1|0|yes|no|on|off` — help's "numeric rejected with a hint" promise verified live for `--crystal-dragon 10`); fork-while-single-threaded ordering invariant enforced by comment only (verified holding: guard spawn precedes all thread installs in `event_loop_setup`); one bounded intentional `String::leak()` at startup (default message text, once per process) | LOW | Documented here; no change (owner peak-directive: no over-engineering) |

## Real-usage battery results (32 probes)

All invalid-input probes rejected cleanly with exit 2 + specific hints
(scene unknown-name probe lists every valid scene; dump-config
redirect/outside-whitelist/overwrite guards all fire; numeric on a
bool flag fires the `--crystal-dragon-secs` hint). `--benchmark` warns
when `--duration` is a no-op in bench mode. Headless interactive
attempt fails fast with the ENXIO tip pointing at `--benchmark`/
`--doctor`/`--dump-config`. `--testconf` requires `--config <path>`
(the flag takes no inline path — resolved-path model).

## Verdict

The estate is LTS-ready. Three real items were found and fixed in this
series (temp-file stranding, two doc-truth drifts); everything else
came back clean at evidence-verified depth. No code behavior changed
beyond the error-path temp cleanup; no benchmark-visible regression
(visual objective metrics at parity).

## Verification commands

```
cargo build --locked
cargo test config_io_tests          # 2 passed
cargo test docs_tests               # 21 passed
cargo clippy                        # clean
cosmostrix --benchmark --bench-duration 10 --screen-size 80x24 --json
```
