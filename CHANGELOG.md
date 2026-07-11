# Changelog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

Cosmostrix uses [SemVer](https://semver.org/) for package versions (e.g. `4.0.0`).
Git tags and GitHub Releases use a leading `v` (e.g. `v4.0.0`).
Stable releases do not use `-stable.N` suffixes.

All notable changes to this project are documented in this file.

---

## v13.4.0 — Screen Size + Duration Features

New feature release. Adds `--screen-size WxH` for fixed virtual screen
size and `--duration` for human-readable benchmark duration. HUD now
shows screen size on the 6th line.

### Features

**`--screen-size WxH`** (e.g. `--screen-size 120x40`, `--screen-size 12x12`):
- **Benchmark mode**: override terminal size. Replaces
  `COSMOSTRIX_BENCH_COLS`/`COSMOSTRIX_BENCH_LINES` env vars.
  Example: `cosmostrix --benchmark --screen-size 12x12 --json`
- **Interactive mode**: render to fixed virtual size. Ignores terminal
  resize events. If screen-size exceeds terminal, prints warning and
  clips to top-left.
- **Without `--screen-size`**: dynamic (current behavior — follows
  terminal resize).
- Minimum: `1x1`. Maximum: `65535x65535` (u16 range).
- Case-insensitive: `120x40` or `120X40` both work.

**`--duration` compound format** (e.g. `--duration 6s`, `--duration 1h30m`):
- Human-readable duration: `6s`, `30m`, `1h`, `1h30m`, `2h15m30s`
- Long forms: `6sec`, `30mins`, `1hour` also accepted
- Bare number: `--duration 90` = 90 seconds (backward compat)
- Minimum: `1s`. No maximum cap (user responsibility for long runs).
- Alias for `--bench-duration` — `--duration` takes precedence if both
  are specified.
- `--bench-duration` 600s max cap REMOVED — unlimited endurance runs.

**HUD screen size line**:
- 6th HUD line shows current screen size: `120x40` (dynamic) or
  `120x40*` (fixed via `--screen-size`).
- Updates on terminal resize (dynamic mode) or stays constant (fixed mode).

### Implementation

- `src/cli_parse.rs` (new, 175 LOC): `parse_duration()` + `parse_screen_size()`
  with 16 unit tests covering all formats + edge cases.
- `src/config.rs`: added `--duration` + `--screen-size` CLI args.
- `src/app.rs`: added `screen_size: Option<(u16, u16)>` to CloudConfig.
- `src/main.rs`: `resolve_bench_duration_args()` — `--duration` takes
  precedence over `--bench-duration`.
- `src/bench.rs`: `bench_dimensions()` accepts CLI screen-size;
  removed 600s max cap from `resolve_bench_duration()`.
- `src/interactive/event_loop.rs`: fixed vs dynamic size logic;
  resize events ignored in fixed mode; warning on size > terminal.
- `src/interactive/hud.rs`: 6th HUD line for screen size;
  `set_screen_size()` method.

### Usage Examples

```bash
# Benchmark at fixed 12x12 size (tiny terminal = max FPS)
cosmostrix --benchmark --screen-size 12x12 --json

# Benchmark for 10 minutes
cosmostrix --benchmark --duration 10m --json

# Benchmark for 1h30m (endurance test)
cosmostrix --benchmark --duration 1h30m --json

# Interactive at fixed 80x24 (ignores terminal resize)
cosmostrix --screen-size 80x24

# Interactive dynamic (current behavior — follows terminal)
cosmostrix
```

All 747 tests pass (731 existing + 16 new cli_parse tests). Clippy + fmt clean.

---

## v13.3.1 — Dragon Performance Merge (18 Dragon Eggs + P1/P2/P3)

Performance-only patch release. Merges the `dragon-experimental`
branch: 8 commits containing 18 "dragon egg" micro-optimizations
plus 3 P-tier optimizations (P1/P2/P3). No color/render quality
changes — all commits are pure performance work.

### P1: Gate Component Timing Behind Flag

`cloud.rain_at()` skips 2 `Instant::now()` calls (t1, t2) when
`enable_component_timing` is false. Interactive mode leaves it off;
`--benchmark` and `--perf-stats` enable it. Saves ~40ns/frame.

### P2: Halve spin_wait Instant::now() Calls

`activity.spin_wait()` cached `now` for both deadline + limit checks.
Saves ~250µs/frame in interactive mode (50% reduction in spin timing).

### P3: Combined flush_ansi + io_uring Dead End

- `terminal.flush_ansi()` combines `SYNC_START + ansi_buf + SYNC_END`
  into a single `write_all` via reusable buffer. Reduces syscalls 3→1.
- `dragon_egg_io_uring.rs` proves io_uring NOT worth it: `write()`
  syscall is 306ns/call = 0.0018% CPU at 60 FPS. Dead end.

### 18 Dragon Eggs: Eliminate Redundant Bounds Checks + Option Allocs

All 18 eggs: `.get(i)` → direct `[]` indexing when `i` was already
bounds-checked. 2-3 cycles faster per call.

| Eggs | File | Pattern |
|------|------|---------|
| #1-5 | frame.rs, terminal.rs | set(), set_force(), diff path, dirty_map, clear_dirty() |
| #6-7 | frame.rs | cell_gen_at_index(), get() test accessor |
| #8-10 | phosphor.rs | phosphor_fresh, phosphor_in_active |
| #11-13 | render.rs | get_char(), col_stat, edge_fade LUT |
| #14-18 | render.rs, spawn.rs, monolith.rs | glitch_map, color_map, palette_slices |

Eggs #19-21 attempted but caused regression — compiler was already
optimal in those paths. Reverted.

### Performance Gains

| Size | v13.3.0 avg FPS | v13.3.1 avg FPS | Delta |
|------|----------------:|----------------:|------:|
| 4×4 | 765,235 | 790,770 | +3.3% |
| 80×24 | 105,256 | 108,572 | +3.2% |
| 120×40 | 51,236 | 51,865 | +1.2% |
| 200×60 | 28,138 | 28,582 | +1.6% |

Peak 4×4: 1,082,251 → 1,122,334 FPS (+3.7%)

### What Did NOT Change

- No color palette changes
- No rendering logic changes
- No visual quality changes
- 731 tests pass (729 existing + 2 dragon egg tests)
- `supercharger.c`/`supercharger.rs` are research artifacts, gated
  behind feature flag, NOT compiled by default

---

## v13.3.0 — Encoding Instrumentation (SGR cache hit-rate + ANSI bytes/frame)

Adds empirical measurement instrumentation to the diff-based rendering
engine. The `--perf-stats` exit report now includes an ENCODING section
showing actual measured ANSI bytes per frame, total bandwidth, and SGR
cache hit rate — replacing the previous estimate-based numbers.

### New Metrics (ENCODING section in --perf-stats report)

- **total_ansi_bytes**: cumulative ANSI bytes flushed to stdout across
  all frames. Measured in `Terminal::flush_ansi()` by summing
  `ansi_buf.len()` before each clear.
- **frames_flushed**: number of `flush_ansi()` calls (= number of frames
  actually drawn to the terminal).
- **avg_bytes_per_frame**: `total_ansi_bytes / frames_flushed`. Replaces
  the previous `ANSI_BYTES_PER_CELL_ESTIMATE` heuristic with actual
  measurement.
- **bandwidth**: `total_ansi_bytes / elapsed_seconds` in KiB/s. Shows
  real terminal I/O load.
- **sgr_cache_hits / sgr_cache_misses**: atomic counters in `ColorCache`
  incremented on every `sgr_for_cell()` call. Hit = palette color found
  in cache; miss = fell back to on-the-fly `write_sgr_colors_buf`.
- **sgr_cache_hit_rate**: `hits / (hits + misses) * 100%`. High rate
  (>90%) confirms the cache is effective.

### Implementation

**Option A — SGR cache counters** (`src/color_cache.rs`):
- Added `sgr_hits: AtomicU64` and `sgr_misses: AtomicU64` fields to
  `ColorCache`.
- `sgr_for_cell()` increments the appropriate counter on every call.
  Uses `Ordering::Relaxed` (~2ns overhead on x86) — eventual accuracy
  is sufficient for the perf report.
- New `cache_stats()` method returns `(hits, misses)`.
- 6 new unit tests covering: zero-initialization, hit on palette color,
  miss on non-palette color, miss on non-palette bg, hit on reset/blank,
  accumulation across calls.

**Option B — ANSI bytes/frame counter** (`src/terminal.rs`):
- Added `total_ansi_bytes: u64` and `flush_count: u64` fields to
  `Terminal`.
- `flush_ansi()` accumulates `ansi_buf.len()` into `total_ansi_bytes`
  and increments `flush_count` BEFORE clearing the buffer. Sync wrapper
  bytes (12 bytes when `sync_output` is enabled) are NOT counted —
  only actual frame content.
- New `encoding_stats()` method returns
  `(total_ansi_bytes, flush_count, sgr_hits, sgr_misses)`.
- Called from the `--perf-stats` exit path in `event_loop.rs`, captured
  BEFORE `drop(term)` to avoid losing the stats.

### Code Refactor

- Extracted `push_u8`, `push_u16`, `write_sgr_colors_buf` from
  `terminal.rs` into new `src/sgr_format.rs` (106 LOC). These are pure
  SGR formatting functions with no dependency on the `Terminal` struct.
  This keeps `terminal.rs` under its 1000-LOC guard (now 947 LOC).
- Updated `docs/RENDER_ENGINE.md` future-work section: SGR cache
  instrumentation marked as DONE.

### Why This Release

The RENDER_ENGINE.md spec claimed "~95% SGR cache hit rate" without
empirical evidence. v13.3.0 makes that claim **measurable and
defensible**. Run `cosmostrix --perf-stats`, interact for a few seconds,
press `q`, and the ENCODING section shows the actual hit rate and bytes
per frame.

This also replaces the `ANSI_BYTES_PER_CELL_ESTIMATE` heuristic in the
benchmark report with actual measured bytes per frame — the estimate
was ~19 bytes/cell, but with RLE batching the real number is typically
much lower (often 1-5 bytes/cell for stable rain).

All 729 tests pass (723 existing + 6 new counter tests). Clippy + fmt
clean.

---

## v13.2.0 — Render Engine Formal Specification + Competitor Benchmark

Documentation release formalizing cosmostrix's position as the
definitive diff-based terminal rendering engine. No runtime behavior
changes — purely additive documentation and tooling.

### Documentation

**Formal render engine specification** (`docs/RENDER_ENGINE.md`):
- 9-section formal architecture document covering: problem statement,
  strategy (differential rendering + RLE), data structures, complexity
  analysis, output encoding details, alternative-engine comparison,
  measured performance, failure modes, and future work.
- Includes BibTeX citation block for academic reference.
- Documents the existing `terminal.rs` `draw()` implementation:
  - Cell equality fast path (24-byte derived `==`, ~4 cycles/cell)
  - Dirty tracking via BitVec + dirty queue (O(1) mark, O(dirty) flush)
  - Run-length encoding on both full-redraw and diff-redraw paths
  - SGR state tracking across runs (`cur_fg`/`cur_bg`/`cur_bold`)
  - `ColorCache` pre-computed SGR bytes per `(fg, bg)` pair
  - `semantic_gen` counter for charset/theme invalidation
  - `force_draw_everything()` escape hatch for overlay cleanup
- Compares cosmostrix's diff-based engine against 5 alternatives:
  full redraw (cmatrix), per-droplet cursor targeting, ANSI scroll
  regions, Sixel/graphics protocol, PTY multiplexer — with explicit
  trade-off analysis for each.

**Competitor benchmark script** (`scripts/bench-compare.sh`):
- Side-by-side resource comparison: cosmostrix vs cmatrix vs unimatrix.
- Uses `/usr/bin/time -v` inside a PTY (`script`) to measure CPU time
  and peak RSS under identical terminal conditions.
- Outputs a Markdown table suitable for pasting into `benchmark/README.md`.
- Honest about limitations: terminal-bound renderers cannot be
  benchmarked for FPS via subprocess (FPS is determined by the terminal
  emulator, not the process). The script measures **resource
  efficiency** — the defensible axis for diff-based vs full-redraw
  engine evaluation.
- Gracefully handles missing competitors (cmatrix/unimatrix) with
  clear install instructions.

### README / Docs Cross-References

- `README.md` Documentation section: added link to RENDER_ENGINE.md.
- `benchmark/README.md`: added "Competitor Comparison" section
  pointing to `scripts/bench-compare.sh` and `docs/RENDER_ENGINE.md`.

### Why This Release

Cosmostrix's `terminal.rs` `draw()` function has been at masterclass
level since v10.x — RLE on both paths, SGR state tracking, color
cache, direct ANSI byte buffer, no-heap integer formatting. But this
was implicit knowledge scattered across code comments. v13.2.0 makes
it **explicit and defensible**:

1. **RENDER_ENGINE.md** makes the design citation-worthy — downstream
   TUI authors can reference cosmostrix as a reference implementation
   of diff-based terminal rendering.
2. **bench-compare.sh** provides empirical evidence — without
   competitor data, claims of "masterclass" are marketing, not
   engineering.
3. The formal spec also serves as onboarding for new contributors:
   instead of reverse-engineering `terminal.rs`, they read one
   document that explains the why behind every design choice.

---

## v13.1.2 — HUD Toggle-Off Residue Fix

Bug-fix release addressing a visual residue issue: when toggling the
live HUD off (pressing `i` again), stale HUD text + black background
cells remained visible in regions where the rain didn't actively write
this frame.

### Bug Fixes

**HUD toggle-off now clears residue via force_draw_everything()**:
- The rain renderer is diff-based (`frame.set()` skips cells whose
  content matches the previously-sent state). When HUD turns off, the
  frame buffer still contains the 5×15 HUD cells (text + black bg). On
  the next frame, only cells the rain actively writes get refreshed —
  cells in dead zones (no active droplet, no glitch, no phosphor decay
  this frame) keep their stale HUD content, leaving visible "residue".
- The fix calls `cloud.force_draw_everything()` when toggling OFF. This
  triggers `frame.clear_with_bg()` on the next rain update, which:
  1. Sets `dirty_all = true` (forces every cell to be re-sent)
  2. Resets all cells to the bg color
  3. The rain then redraws active cells on the clean canvas
- Net effect: HUD cells are guaranteed to be cleared, regardless of
  whether the rain happens to write them this frame. The user sees a
  clean toggle-off with no leftover text.
- Toggling ON does not need force_draw — the HUD writes via `set()`
  which marks cells dirty because content differs from rain. Toggle ON
  was already working correctly.

### Code Changes

- `src/interactive/event_loop.rs`: HUD toggle handler now captures the
  return value of `hud_state.toggle()` (new visibility state) and calls
  `cloud.force_draw_everything()` only when turning OFF. Added detailed
  comment explaining the residue mechanism and why force_draw is needed.

---

## v13.1.1 — Android HUD Toggle Fix

Bug-fix release addressing a critical Android/Termux regression: pressing
the HUD toggle key caused cosmostrix to self-exit instead of showing the
live metrics overlay.

### Bug Fixes

**HUD toggle key changed from `?` to `i`**:
- On Android/Termux soft keyboards, the `?` character may arrive with
  unexpected modifier bits or as a different keycode entirely. When the
  event did not match the HUD toggle arms, it fell through to the
  screensaver exit path (`if cfg.screensaver { cloud.raining = false;
  break; }`), causing cosmostrix to quit instead of toggling the HUD.
- The fix replaces `?` (and the previous `/`-with-Shift fallback arms)
  with a simple lowercase printable letter `i` (uppercase `I` also
  accepted). Every Android keyboard sends simple printable letters
  reliably; the modifier-bit ambiguity is eliminated entirely.
- All docs, help text (`--help-detail`), README, ROADMAP, and
  RELEASE_CANDIDATE updated to reflect the new key.

### Documentation

- `docs/ROADMAP.md`: added `P2-fix` row noting the v13.1.1 key change.
- `docs/RELEASE_CANDIDATE.md`: HUD smoke-test steps updated to press `i`.
- `src/help_detail.rs`: RUNTIME CONTROLS table updated.
- `README.md`: keyboard shortcuts table + benchmark section updated.

---

## v13.1.0 — Shell Completions + Verbose + Help-Detail Polish

UX polish release. Adds shell completions, a verbose diagnostic flag,
strict .toml enforcement for `--config`, and clearer help text.

### Features

**Shell completions** (bash, zsh, fish, elvish):
- New `--completions <shell>` flag generates a shell completion script
  on stdout. Pipe to your shell's completions directory.
- AUR `PKGBUILD` and `scripts/install.sh` auto-install bash + zsh
  completions during package install.
- Built with `clap_complete = "4"`.

**Verbose diagnostic output** (`--verbose`):
- Prints 30+ diagnostic fields to stderr before launching: config path,
  resolved values, terminal detection, atmosphere state, color tune,
  charset source, profile, etc.
- For power users debugging config/loading issues.

### Bug Fixes / Behavior Changes

**Strict .toml extension check for `--config`**:
- Previously `--config` would silently accept non-.toml files. Now it
  enforces the .toml extension and exits with a clear error.

**Invalid config values now say "error:" not "warning:"**:
- Invalid config values are no longer silently ignored. They now print
  `error: invalid <field>='<value>' (allowed: ...)` to stderr so users
  immediately know cosmostrix didn't load their custom config.

**Help-detail DIAGNOSTICS section**:
- `--verbose` and `--completions <shell>` added to `--help-detail`
  DIAGNOSTICS section with clear usage examples.
- `--dump-config` text updated: "warn cleanly and are ignored" →
  "error: messages are printed to stderr" (reflects the actual behavior).
- `--config` text: removed "Falls back to legacy 'config' (no extension)"
  since we now require .toml.

### Documentation

- `docs/ROADMAP.md`: removed future roadmap sections (secret — kept
  private until features ship). Only completed history remains.
- Test suite reduced from 882 to 723 by removing 159 doc-content tests
  (-2178 LOC). The docs-tests module now only verifies asset integrity
  and metadata, not prose content.

---

## v13.0.0 — Alive Rain + Depth-of-Field + Security

Visual quality + security hardening release. The rain now feels alive
throughout the trail (not just at the head), background rain appears
out-of-focus like film Matrix depth-of-field, the message typewriter
glows in per character, and file-reading CLI flags are restricted to
safe paths.

### Visual Quality

**Character cycling** (alive rain):
- Trail characters now have a 2% chance per decay step to mutate to a
  new random glyph from the char pool. At 60fps with ~1000 active trail
  cells, ~20 characters change per second — subtle enough to feel
  organic, frequent enough to make the rain feel "alive" throughout.
- Previously only the head character cycled (every 100ms); the trail
  was static after spawn. Now matches the film Matrix effect where
  background characters subtly shift.
- New constant: `TRAIL_CYCLE_PROBABILITY = 0.02` in constants.rs.

**Depth-of-field** (perceptual blur):
- Layer 0 (background) foreground color is blended 35% toward black,
  creating a "foggy/out-of-focus" look. The terminal equivalent of
  depth-of-field: instead of blurring pixels (impossible in text), we
  reduce fg-bg contrast so background rain reads as "behind a haze".
- Layers 1-2 stay sharp. 3-tier depth hierarchy: sharp foreground →
  clear midground → hazy background.
- New constant: `PARALLAX_CONTRAST_REDUCTION = [0.35, 0.0, 0.0]`.

**Typewriter fade-in glow** (masterclass upgrade):
- Each newly revealed message character now fades in from 30% to 100%
  brightness over 100ms (3 frames at 30ms/char reveal rate). Creates a
  premium "glow-in" effect — characters appear to illuminate rather
  than snap into existence.
- Previously characters popped in at full brightness (hard pop-in).

**Space key restarts typewriter**:
- When the user presses Space to reseed the rain, the message typewriter
  also restarts from the beginning. Rain reseed + message types out
  from scratch — consistent cinematic replay on every restart.
- New method: `cloud::restart_message_typewriter()`.

### Security

**Safe path validation** (`--config` and `--charset-file`):
- Prevents cosmostrix from being used as an arbitrary file reader.
  Before: `--charset-file /etc/shadow` would read and display shadow
  file contents as charset characters. `--config /proc/self/environ`
  would parse environment variables as config.
- Now: `is_safe_path()` validates the path before reading. Allowed:
  home directory (`~`), current directory (`.`), `/etc/cosmostrix/`,
  `/tmp/` (for testing/scripts). Rejected: `/etc/shadow`, `/proc/*`,
  `/sys/*`, `/root/*`, `/var/log/*`, etc.
- New file: `src/safepath.rs` (117 LOC) with 6 unit tests.

**System-wide config fallback**:
- `load_config_file()` now falls back to `/etc/cosmostrix/config.toml`
  when no user-level config exists. Search order: user config → legacy
  filename → /etc system default.

**Message length limit**:
- `--message` text is now limited to 200 characters. Prevents layout
  overflow from excessively long messages. Clear error message on
  violation.
- New constant: `MESSAGE_MAX_LEN = 200`.

### PKGBUILD Cleanup

- Removed hardcoded config.toml from AUR package. Clean install: only
  binary + license + docs. No config files installed — cosmostrix
  ships sensible built-in defaults and generates a config on demand
  via `cosmostrix --dump-config`.

---

## v11.1.0 — Benchmark Depth & Theme Tuning

Closes the "real metrics, not gimmick" gap and pushes the benchmark to
S-tier (ChatGPT 9.8/10 → 10/10). The premium benchmark (`--benchmark`)
now reports RSS memory, CPU usage, sub-component timing, long-run drift,
build/environment metadata, page faults + context switches, and an
explicit GPU-not-used declaration. A live HUD overlay brings the same
metrics into interactive runs. JSON output mode enables CI parsing.
Theme tuning makes the 43 built-in palettes more visually distinct.

### New Features

**RSS memory tracking** (P0-A, commit 34f22df):
- `--benchmark` now emits a `MEMORY` section with `peak_rss`, `avg_rss`,
  `rss_samples`, `rss_basis`, and `rss_caveat`.
- Zero new dependencies. Linux samples `/proc/self/status`; macOS uses
  `mach_task_basic_info` via `libc`. Other platforms emit `unsupported`.
- The benchmark report honestly states "RSS includes shared pages; treat
  as order-of-magnitude footprint" so users do not over-interpret.

**Tail frame-time metrics** (P0-B, commit 3afac82):
- Added `p99_9_frame_time` (1-in-1000 worst frames) and `max_frame_time`
  (single worst spike) to the `PERFORMANCE` section, plus
  `max_frame_time_meaning`.
- `max_frame_time` captures what users perceive as jank — page faults,
  OS scheduling glitches — that p99 smooths over.
- PERFORMANCE section reordered for monotonic display:
  avg → p95 → p99 → p99.9 → max.

**Sub-component timing** (P1-A, commit 6bc5035):
- New `COMPONENT TIMING` section: `avg_sim_ms`, `avg_render_ms`,
  `avg_io_ms`, plus maxes and `sim/render/io_share_percent`.
- `sim_ms` = atmosphere events + spawn rate + droplet physics.
- `render_ms` = phosphor decay + anomaly zones + atmospheric fx.
- `io_ms` = dirty checks + clear_dirty + bookkeeping. Honestly labeled
  "NO terminal write in benchmark mode" — not real IO.
- Distinguishes "benchmark mainan" from "profiling tool".

**`--bench-duration N` flag + drift detection** (P1-B, commit 9e94527):
- New `--bench-duration <1-600>` flag (default 5s). Use with `--benchmark`
  for long-run drift / leak / thermal-throttle detection.
- New `DRIFT` section: `first_half_fps`, `second_half_fps`,
  `fps_drift_percent`, `drift_interpretation`, `drift_basis`.
- Interpretation: `> +10%` = degraded; `< -10%` = improved (warmup
  insufficient); otherwise `stable`.

**Live HUD overlay** (P2, commit 12a1d2f):
- Press `?` during any interactive run to toggle a top-right overlay
  showing `fps`, `avg`, `p99`, `max`, `rss` in real time.
- Zero cost when off (all methods short-circuit on `visible == false`).
- 4 Hz render rate, 1 Hz RSS sampling. ANSI-only output bypasses the
  frame buffer to keep rain renderer's dirty tracking clean.

**CPU usage % tracking** (P3, commit aeafdd3):
- New `CPU` section in `--benchmark`: `avg_cpu_percent`,
  `peak_cpu_percent`, `cpu_samples`, `cpu_basis`, `cpu_caveat`.
- Linux samples `/proc/self/stat` (utime + stime); macOS uses
  `mach_task_basic_info` (`time_value_t` seconds + microseconds).
  Other platforms emit `unsupported`.
- `cpu_caveat` honestly states "~100% = one core saturated; >100% would
  indicate multi-threading or measurement error" (single-thread by design).

**`--color-tune` runtime theme adjustment** (Q2, commit ce0d191):
- New `--color-tune saturation=X,brightness=Y` flag. Keys: `saturation`/
  `sat`, `brightness`/`bright`. Range 0.0–3.0 (1.0 = identity).
- Linear-RGB transforms (no HSL round-trip): saturation scales distance
  from Rec. 601 luminance; brightness multiplies each channel.
- Turns the 43 built-in themes into 43 × infinite variants without adding
  new presets. Background color is also tuned for visual consistency.

**Build metadata + CPU model** (peak, commit 7db64b9):
- `SYSTEM` section expanded from 3 to 12 fields: now includes
  `rustc_version`, `git_sha`, `cpu_baseline`, `target_features`, `lto`,
  `panic`, `strip`, `pgo`, and `cpu_model` (runtime-detected chip name).
- `cpu_model` reads `/proc/cpuinfo` (Linux) or `machdep.cpu.brand_string`
  via sysctl (macOS). Lets users compare reports across machines.

**Resource usage via getrusage** (peak, commit 7db64b9):
- New `RESOURCE` section: `minor_faults`, `major_faults`,
  `voluntary_ctxt`, `involuntary_ctxt` + `*_meaning` fields.
- Cross-platform via `getrusage(RUSAGE_SELF)` — no permissions required.
- Covers the scheduling-pressure story without `perf_event_open` (which
  is Linux-only and permission-gated).

**GPU-not-used declaration** (peak, commit 7db64b9):
- New `gpu_usage: not_applicable` + `gpu_basis` fields in the RENDERER
  section. Explicitly declares that cosmostrix creates no GPU context
  (no OpenGL/Vulkan/Metal/DirectX/WebGPU). Closes the "does cosmostrix
  use GPU?" question definitively in the report itself.

**Benchmark environment (reproducibility)** (peak, commit 7db64b9):
- New `BENCHMARK ENVIRONMENT` section: `kernel_version`, `libc_variant`,
  `term`, `term_program`, `term_version`, `cpu_governor`, `smt_active`
  + `env_basis` + `env_caveat`.
- Cross-platform: kernel via `uname`, libc variant from build-time,
  terminal from env vars. Linux-only: CPU governor + SMT from `/sys`.
- Lets users compare reports across machines knowing the OS/governor/
  terminal context. Two machines with the same CPU can produce
  different results if the governor differs.

**JSON output mode** (peak, commit 7db64b9):
- New `--json` flag. Use with `--benchmark` for machine-readable JSON
  output (single line, parseable by CI/scripts).
- Manual JSON serializer — zero new dependencies (no serde).
- Mirrors the text report's 13 sections: status, system, renderer,
  config, environment, performance, memory, cpu, resource,
  component_timing, drift, throughput, timing.
- Option fields emit `null` when None; NaN/Inf emit `null` defensively.

**Async mode now default ON + improved distribution** (organic, commit pending):
- `--async` default flipped from `off` → `on`. The rain now feels
  organic out of the box — columns fall at desynchronized speeds
  instead of uniform pacing.
- Speed distribution improved from flat `uniform[0.33, 1.0]` (mean 0.665)
  to `max(two uniforms)` — a triangular distribution skewed toward 1.0
  (mean ~0.78). Most columns run near full speed with occasional slow
  streams, which feels more natural than the previous flat distribution.
- Naming clarified in `--help-detail`: "async" means "asynchronous
  column pacing", NOT Rust async/await. Cosmostrix remains single-threaded.
- `async-mode = true` now appears in `--dump-config` and `config.toml`
  with a clarification comment.
- Config file key `async-mode` added to `USER_CONFIG_KEYS` so it's
  recognized by the parser (previously only settable via CLI flag).
- `config_apply.rs` now reads `async-mode` from config files.
- Runtime toggle `a` still works for A/B comparison.

### Theme Audit

**5 near-duplicate themes tuned** (commit 304a07b):
- Programmatic audit (pairwise average per-stop RGB distance < 30)
  identified 5 pairs that were too similar. All tuned to be visually
  distinct:
  - `green3`: deep teal-shifted forest green (was nearly identical to `green`)
  - `saturn`: amber-gold (was too close to `venus`)
  - `comet`: deep-blue ion trail to cyan-white head (was too close to `uranus`)
  - `meteor`: burning rock with ionized plasma tail (was too close to `sun`)
  - `pluto`: nitrogen-ice blue dwarf (was too close to `mercury`)
- Theme descriptions in `--list-colors-detail` updated to match.
- Audit runs as an informational test (`palette::audit_tests::
  audit_near_duplicate_themes`) — future theme additions can re-run it.
- After tuning: no pairs remain under threshold. Closest is
  `galaxy` ↔ `andromeda` at 30.0 (both purple-cosmic, meant to be related).

### CI Fixes

- `libc` moved from `cfg(target_os = "linux")` to `cfg(unix)` so macOS
  builds can use `mach_task_basic_info` (commit 22fa131).
- macOS Mach API migrated from removed `task_basic_info` /
  `TASK_BASIC_INFO` to modern `mach_task_basic_info` /
  `MACH_TASK_BASIC_INFO` (commit 4e76fda).
- `cpustat.rs` macOS branch fixed: `time_value_t` is a struct
  `{seconds, microseconds}`, not `u32` — removed incorrect
  `mach_timebase_info` conversion (commit 58ebedb).
- `diagnostics.rs` macOS `sysctlbyname` fixed: `null()` → `null_mut()`
  for `*mut c_void` params + removed unused `c_char` import (commit 4726d9a).
- `.codespellrc` added `numer`, `denom` to ignore-words-list (legitimate
  Mach timebase field names, not typos).

### Internal

- 7 new source files: `memstat.rs`, `cpustat.rs`, `usagestat.rs`,
  `envstat.rs`, `bench_mem.rs`, `bench_cpu.rs`, `bench_comp.rs`,
  `bench_progress.rs`, `bench_meta.rs`, `bench_json.rs`, `color_tune.rs`,
  `interactive/hud.rs`.
- `bench.rs` extracted `BenchProgress` + `ComponentTimer` + `RssTracker`
  + `CpuTracker` to keep the file under its 900-LOC guard.
- `bench_report.rs` extracted meaning constants + helpers to `bench_meta.rs`
  + BENCHMARK ENVIRONMENT rendering to `envstat.rs` to keep under 1000 LOC.
- `FrameTimeTracker` gained `p99_ms()` accessor for the live HUD.
- `cloud/rain.rs` instrumented with 2 `Instant::now()` markers per frame
  for sim/render split (~40ns overhead, negligible).
- 845 → 864 tests (clippy + fmt clean on every commit).
- Zero new runtime dependencies.

---

## v12.0.0 — Protocol Engine

**Released: 2026-07-08**

Major release introducing terminal protocol intelligence and color pipeline
optimization. The engine now detects the terminal emulator at startup and
adapts its output strategy accordingly.

### New Modules

- **`src/termdetect.rs`** — Terminal vendor detection (kitty, wezterm, alacritty,
  foot, iTerm2, Windows Terminal, tmux, Rio) via environment variables.
  Enables synchronized output (`ESC[?2026h` / `ESC[?2026l`) for tear-free
  frame delivery. Safe on all terminals — unsupported ones ignore the sequences.
- **`src/color_cache.rs`** — Pre-formatted ANSI SGR byte cache for palette colors.
  Eliminates ~300-400 per-cell encoding calls per full-redraw frame.
  Linear-scan lookup optimized for small palettes (7-20 colors).
- **`src/ux.rs`** — Unified CLI user-experience output. Single source of truth
  for error/warning formatting. Fixes double-print bug on validation errors.

### Performance

- Synchronized output: terminal buffers entire frame, flushes atomically
- Color byte cache: `extend_from_slice` replaces `push_u8` arithmetic
- Zero regression on benchmark throughput (engine already at 50K+ FPS)

### UX Improvements

- All error messages: single clean line with `error:` prefix
- All warnings: consistent `warning:` prefix (was mixed `config:` / `warning:`)
- Exit codes: 2 for invalid input, 1 for config/runtime failure

---

## v11.0.0 — Cinematic Peak

Visual quality push to peak cinematic Matrix rain. Pure tuning — no
architecture changes, no new dependencies. Every change is a constant
value adjustment or small feature addition.

### Visual Quality Improvements

**Cosmos palette brightened** (v10.0.0):
- Old: `[17,18,19,54,55,56,57,93,129,189,225]` — avg 30.3% luminance
- New: `[20,27,33,57,63,93,99,129,141,189,225]` — avg 45.5% luminance
- Replaced 3 darkest entries with vibrant blue/purple mid-range colors.

**Head white blend raised 12% → 45%** (v10.0.0):
- Glyph mode (droplet.rs): HEAD_WF 31 → 115
- Monolith mode (monolith.rs): CORE_WF 26 → 115
- Head is now OBVIOUSLY brighter than body — film-quality head pop.

**Parallax layer 0 raised 0.55 → 0.70** (v10.0.0):
- Background rain now visible (was near-invisible after dimming).
- 3-tier depth hierarchy: bright head → mid body → dim-but-visible background.

**Phosphor decay faster** (v11.0.0):
- PHOSPHOR_DECAY_RATE: 3.0 → 5.0 (afterglow 1094ms → ~400ms)
- PHOSPHOR_TAIL_RESIDUAL: 160 → 120 (63% → 47% initial brightness)
- Trail is now crisp and energetic — matches film Matrix energy.

**EdgeFade bottom min raised 0.20 → 0.45** (v11.0.0):
- Bottom border brightness: 7% → 16% (visible, was near-invisible).
- Cinema framing preserved without over-aggressive dimming.

**Fog min factor raised 0.35 → 0.45** (v11.0.0):
- Border rows brighter, less aggressive vignette.
- Combined with EdgeFade: bottom border now ~20% brightness (was 7%).

**Monolith Ghost/Dim level raised** (v11.0.0):
- Old: `first_visible` (index 1, 4-33% luminance)
- New: `last/5` (index 2, ~42% luminance for cosmos)
- Ghost trace now visible after dimming (~25% perceptual brightness).

**Default density raised 0.75 → 0.85** (v11.0.0):
- More columns active, denser rain — matches film Matrix density.
- Updated: scene.rs, config.toml, dump_config_text, all test assertions.

**Head shimmer period 0.12s → 0.10s** (v11.0.0):
- Character changes 10/sec (was 8.3/sec) — more chaotic, film-like.

### New Features

**`--charset-file PATH`** (v11.0.0):
- Load custom characters from a file. Overrides `--charset` preset.
- File format: one char per line, or single line of characters.
- UTF-8 supported (kanji, Latin, symbols).
- Wide/zero-width characters (emoji, CJK fullwidth) are automatically
  filtered with a warning — prevents screen corruption.
- Usage: `cosmostrix --charset-file ~/my-chars.txt`

### Bug Fixes

**`--charset-file` wide-char crash** (v11.0.0):
- Emoji (🐺) and CJK fullwidth characters caused screen corruption
  (jitter, column misalignment) because the renderer is column-based
  and assumes 1 cell per character.
- Fix: filter wide/zero-width characters using `unicode_width` crate
  (same filter as built-in charset presets). Warns on stderr with
  skipped character codepoints.

---

## v10.0.0 — Peak Performance & Stability

Major performance optimization and stability hardening release.
+76.5% FPS improvement over v5.0.3 baseline through three optimization
phases plus a brutal pre-release audit. Lightning feature removed per
user request (never reached satisfying visual feel). License enforced
as GPL-3.0-only across all 171 source/doc/config files.

### Performance — Phase A: Hot-Path Optimization (+73.8% FPS)
- `phosphor_active` O(1) dedup via `phosphor_in_active` BitVec —
  eliminated 5K-100K wasted ops/frame from linear `contains()` scan
- `head_brightness()` hoisted out of per-line loop — eliminated 4K
  redundant `Instant::elapsed()` + `exp()` calls/frame
- `is_bright()` / `is_dim()` cached in `DrawCtx` — eliminated 100-300
  per-cell calls/frame when glitchy
- `viewport_edge_fade()` precomputed as LUT per terminal resize —
  eliminated 300-1000 float divisions/frame
- `phosphor_fresh` incremental clear — replaced O(W×H) `fill(false)`
  with ~200 bit clears
- `monolith_breathing_factor` computed once per stream, passed to both
  `draw_spine` and `draw_segments` — eliminated redundant cross-module
  call
- `zactrix monolith_*` functions marked `#[inline]` for cross-module
  inlining

### Performance — Phase 2: Structural (+1.6% FPS)
- Spawn free-list: `droplet_free_list: Vec<usize>` replaces O(N) linear
  scan with O(1) pop/push lifecycle
- Terminal flat dirty pairs: single `Vec<usize>` + single sort replaces
  nested `Vec<Vec<usize>>` — better cache locality, no per-row realloc

### Stability — Pre-Release Audit Fixes
- **CRITICAL**: Panic hook no longer writes to stdout (was racing with
  `Terminal::drop`'s BufWriter flush, leaking rain onto user's main
  terminal screen)
- **HIGH**: Added SIGQUIT to graceful shutdown signal set (was defaulting
  to core dump, bypassing all cleanup)
- **HIGH**: Added `debug_assert!` guard + `.min(255)` clamp in
  `fill_color_map` u8 cast (prevents latent panic if palette > 257 colors)
- **MEDIUM**: `Instant::now() - UPDATE_INTERVAL` → `checked_sub()` in
  bench.rs (prevents panic at boot epoch in containers/VMs)
- **MEDIUM**: `term_reinit.swap(false, Acquire)` → `AcqRel` for correct
  RMW memory ordering
- **MEDIUM**: `validate_err(...).unwrap()` → `.unwrap_or(s)` for
  defense-in-depth
- **LOW**: `tp + 1` → `tp.saturating_add(1)` in droplet/rain hot path

### Dead Code / Bloat Removal
- Deleted `column_transition_delay_ms: Vec<u16>` field (never read)
- Deleted `EVENT_MAX_CONCURRENT` constant (never referenced)
- Removed stale `#[allow(dead_code)]` on `EVENT_RNG_XOR` (is used)

### Feature Removal
- **Lightning system completely removed** (~3000 lines deleted). The
  atmospheric lightning feature (Storm Mode, Weather Director, bolt
  families, illuminate, global pulse) never reached a satisfying visual
  feel after multiple tuning iterations. Removed entirely rather than
  shipped in a poor state. Ghost event (phosphor ghost kanji) retained
  as a separate atmospheric feature.

### License
- Enforced `GPL-3.0-only` across all 171 source/doc/config files
- Fixed `scripts/check-headers.sh` stale `EXPECTED_LICENSE` variable
- Extended `check-headers.sh` to scan `*.md` files (was .rs/.sh/.toml/
  .yml only)
- Updated `LICENSE` body: removed "or (at your option) any later version"

### Benchmark
```
v5.0.1 baseline:    avg_fps 21,359  | frame_time 0.046ms | p99 0.058ms
v5.0.3:             avg_fps 27,869  | frame_time 0.035ms | p99 0.046ms
v10.0.0:            avg_fps 39,147  | frame_time 0.025ms | p99 0.030ms
Gain (v5.0.3→v10):  +40.5% FPS      | -28.6% frame time  | -34.8% p99
Gain (v5.0.1→v10):  +83.3% FPS      | -45.7% frame time  | -48.3% p99
```

---

## v5.0.0 — Nightfall

Cinematic UX + Product Identity Release. Polishes discoverability,
error messages, help text, and configuration UX to product-grade quality.
Establishes the cinematic breathing language as an authoritative
reference for how visual transitions and atmospheric effects should feel.
No renderer hot-path rewrite. No benchmark output field changes.
No 50k FPS promise. Terminal writer remains single-owner.
Benchmark honesty preserved.

### Added
- `--show-preset <NAME>` flag: display full preset details including
  description, overridden parameters, and effective values for any
  named preset. Makes preset behavior inspectable without running the
  renderer. Commit `e9f7b3b`.
- `config/cosmostrix.example.toml`: well-commented example configuration
  file with documented defaults and three profile examples (calm-night,
  cinematic, and dense-stress) ready to copy into `~/.config/cosmostrix/
  config.toml`. Commit `e9f7b3b`.
- `docs/CINEMATIC_BREATHING.md`: authoritative cinematic breathing
  vocabulary and pacing contract defining eight terms (Rest, Pulse,
  Whisper, Compression, Void, Signal, Storm, Breath Cycle), eight pacing
  rules, naming conventions, a 10-layer state hierarchy, and six
  anti-patterns. Commit `6289f41`.
- Cinematic breathing vocabulary with formal definitions for all
  atmospheric intensity levels, establishing a shared language for
  future development and documentation.
- Pacing contract: no instant visual state changes, default state is
  always Rest, Storm is never a default, transitions must be perceptible
  as breathing rather than flickering.
- `--profile` help text now includes `(see --list-profiles)` cross-
  reference so users know where to find available profiles.

### Changed
- Error messages follow a consistent pattern: `error: unknown <type>
  '<value>'` followed by a discovery hint line suggesting the
  appropriate `--list-*` flag. This applies to `--preset`, `--scene`,
  `--color`, `--charset`, and `--profile` validation errors.
- `--color` validation exit code changed from 2 to 1 for consistency
  with other user-input validation errors.
- `--charset` error message changed from "unsupported" to "unknown" for
  consistency with all other validation error messages.
- `--color` error message changed from inline parenthetical format to
  a separate hint line for consistency with other discovery hints.
- `docs/ROADMAP.md` updated with v5.0.0 Nightfall active development
  section, phase table, and cinematic breathing language reference.
- `docs/V5_NIGHTFALL_PLAN.md` created as the full v5.0.0 planning
  document covering scope, non-goals, release safety, phase plan, and
  Android/Cosmostrix Live boundaries. Commit `dc27e6f`.
- `docs/cosmostrix-next-vision.md` created for future sibling product
  (Cosmostrix Live) exploration as an explicitly exploratory document.
  Commit `dc27e6f`.
- `--help` output reorganized with a DISCOVERY section grouping
  `--list-presets`, `--list-profiles`, `--list-scenes`, and
  `--show-preset` for better scannability.

### Fixed
- `--profile` help text previously lacked a cross-reference to
  `--list-profiles`, making profile discovery unintuitive for new
  users. Now includes `(see --list-profiles)` hint.
- `--charset` error message used "unsupported charset" instead of
  "unknown charset", breaking the consistent `error: unknown <type>`
  pattern. Now uses the consistent format.
- `--color` error message used an inline parenthetical hint instead of
  a separate discovery hint line, inconsistent with all other
  validation errors. Now uses a separate line.

### Release Safety
- All v4.9.0 release guard mechanisms inherited and active.
- Terminal writer remains single-owner.
- Benchmark honesty preserved: no fake benchmark progress, no cherry-
  picked runs, no omitted metrics.
- No renderer hot-path changes.
- No benchmark output field changes.
- Terminal lifecycle contract remains authoritative.
- 993 deterministic tests passing.
- No new dependencies.

---

## v4.9.0

The Wolf: Release Guard + Terminal Runtime Contract. Hardens the release
process with mandatory pre-tag gates, automated benchmark reporting,
terminal lifecycle documentation, and doctor/report polish. No renderer
hot-path behavior changes and no benchmark output field changes.

- Release guard foundation (Phase 1): 10-gate (now 11-gate) pre-tag
  checklist in `docs/RELEASE_GUARD.md` ensures benchmark reports,
  version metadata, docs guards, CI, and terminal lifecycle verification
  all pass before any release tag is created. Commit `cf63254`.
- Benchmark report automation (Phase 2): `scripts/release-benchmark-report.sh`
  implements full 5-run benchmark collection and Markdown report generation
  with invariant validation. Commit `f3b6b63`.
- Terminal lifecycle matrix (Phase 3): `docs/TERMINAL_LIFECYCLE_MATRIX.md`
  documents expected cleanup behavior across 14 terminal lifecycle paths
  including normal exit, SIGINT, SIGTERM, SIGHUP, SIGTSTP/SIGCONT, SIGKILL,
  `--reset-terminal`, Windows Terminal, tmux, ssh, headless, benchmark mode,
  and doctor mode. Commit `294ad65`.
- Doctor/report polish (Phase 4): `--doctor` output now includes lifecycle
  contract fields (`signal_exit`, `sigkill`, `terminal_writer`). `--reset-terminal`
  help text clarified as destructive recovery. Commit `43e3dc9`.
- Terminal cleanup honesty preserved:
  - Normal exit (q/Esc): non-destructive mode/style restore.
  - `--reset-terminal`: explicit destructive recovery (clears screen,
    purges scrollback, resets modes).
  - SIGINT/SIGTERM/SIGHUP: catchable cleanup with viewport clear.
  - SIGKILL: cannot be caught or guaranteed. Fork guard is best-effort,
    Linux-only.
- Release guard Gate 7 (terminal lifecycle verification) requires
  `--doctor` lifecycle contract fields, manual exit testing, and
  SIGKILL honesty.
- 944 deterministic tests, all passing.
- Terminal writer remains single-owner.
- `compute_parallelism` remains `disabled`.
- `actual_execution` remains `single-threaded-renderer`.
- No new dependencies.

## v4.8.0

Zactrix Integration + Terminal Cleanup Hardening. Color pipeline optimization
from the zactrix lab with signal-exit terminal cleanup fixes. No default
visual behavior change and no active parallel compute.

- Integrated accepted zactrix color pipeline optimization from lab source
  `e7253e7` (`zactrix-20k-lab`) via manual adaptation. No direct lab merge.
  Commit `ce8dc81`.
- Single RGB decode path and integer brightness blend path replace
  redundant per-cell color computation, reducing pipeline overhead while
  preserving identical visual output.
- Cached binary pool detection avoids redundant `contains_key` lookups
  during stream spawning.
- `set_force` cleanup optimization removes unnecessary work on cells
  that are already marked dirty.
- 50k FPS lab (`zactrix-50k-lab`) documented as not reached and not a
  release promise. Rejected optimization attempts stay rejected.
- Signal-exit cleanup hardening (Phase 4): signal handler threads no
  longer race on stdout with the main loop's buffered writer.
- Visible residue fix (Phase 4B): catchable SIGTERM/pkill-TERM now clears
  the alternate screen viewport before switching back, preventing rain
  frame glyphs from bleeding into the main screen. Fork-guard child
  process silenced on normal parent exit to prevent stdout races.
- Cross-platform signal cleanup imports fixed for Windows CI.
- Windows Terminal `--reset-terminal` issue #15 verified clean.
- 891 deterministic tests, all passing.
- Terminal writer remains single-owner.
- `compute_parallelism` remains `disabled`.
- `actual_execution` remains `single-threaded-renderer`.
- No version bump until this release prep.
- No new dependencies.

## v4.7.0

Profile Ecosystem. Documentation, validation UX, and release-candidate smoke
coverage for the profile system with no default visual behavior change and no
active parallel compute.

- Profile ecosystem contract documenting profile precedence
  (CLI > profile > config > defaults), profile resolution, and mutation
  semantics.
- Profile examples documentation with ready-to-copy config snippets for
  common profile use cases.
- Config dump and `--list-profiles` enhanced with profile documentation
  pointers to `PROFILE_ECOSYSTEM.md` and `PROFILE_EXAMPLES.md`.
- Profile validation UX polish with clear, actionable error messages:
  unknown profiles mention `--list-profiles`, invalid fields/values show
  expected formats, storm rejection is explicit.
- Unknown profile actionable error: both CLI and config paths produce
  clear diagnostics pointing to `--list-profiles`.
- Storm unavailable: error messages and config dump consistently state
  that storm is unavailable.
- Profile RC smoke coverage in `scripts/rc-smoke.sh` with 11 profile-related
  checks and `docs/RELEASE_CANDIDATE.md` updated with v4.7 profile checklist.
- Default remains disabled/protected/identity. No live atmosphere enabled by
  default.
- Terminal writer remains single-owner. Compute parallelism remains disabled.
- No zactrix-20k-lab merge.
- 858 deterministic tests, all passing.

## v4.6.0

Controlled Atmosphere Expansion. Docs, test infrastructure, and CLI
discoverability release with no default visual behavior change and no active
parallel compute.

- Controlled atmosphere expansion contract with state matrix (identity, whisper,
  shadow, protected) and six regimes (calm, pulse, signal, compression, void,
  monolith-pressure). Storm is intentionally unavailable.
- Preset registry with six controlled atmosphere presets: atmosphere-calm
  (identity), atmosphere-pulse, atmosphere-signal, atmosphere-compression,
  atmosphere-void, atmosphere-monolith-pressure (all whisper). Presets are
  opt-in only.
- Preset UX documentation, config/profile examples, and config dump atmosphere
  lines for discoverability.
- `--list-profiles` enhanced with controlled atmosphere preset section showing
  mode, regime, and shadow level for each preset.
- RC smoke script hardened with six atmosphere checks (preset listing, storm
  rejection, controlled-live field verification, disabled+non-calm identity,
  color sun sticky).
- 800 deterministic tests, all passing.
- Default remains disabled/protected/identity. No live atmosphere enabled by
  default.
- Storm unavailable. Terminal writer remains single-owner.
- No zactrix-20k-lab merge.

## v4.5.0

Zactrix Foundation + Depth Regression. Architecture and test infrastructure release with no default visual behavior change and no active parallel compute.

- Split Zactrix Engine architecture into core/cache/render/system/scheduler/metrics modules.
- Added honest ZACTRIX SYSTEM diagnostics (runtime_mode, cpu_budget, render_plan, compute_parallelism, idle_policy).
- Added depth regression lab for Monolith Rain visual stability (15 categories, deterministic guards).
- Split docs, monolith, and scene regression tests into focused module directories to keep all files under 1000 LOC.
- Added roadmap closure docs covering v4.6/v4.7/v4.8/v5 release trajectory.
- No default visual behavior change.
- No active parallel compute.
- Terminal writer remains single-owner.

## v4.0.1

Fixed version output build label to include the optimized CPU tier, matching doctor/benchmark diagnostics.

- `cosmostrix -V` / `--version` now reports the canonical build label (e.g. `linux-x86_64-v3`) from `COSMOSTRIX_BUILD`, consistent with `--doctor`, `--benchmark`, and `--info`.
- Added `canonical_build_label()` as the single source of truth for the build label across all output paths.
- Added deterministic tests to prevent this mismatch from returning.

## v4.0.0

Full Atmosphere Engine groundwork and signature Monolith Rain maturation release.

Highlights:
- Signature Monolith Rain as the production default, with refined sparse data pillars, subtle phase variation, clean afterglow, and bounded residue behavior.
- Zactrix Core / Zactrix Engine / Zactrix Cache groundwork for adaptive rendering architecture, while terminal writes remain single-owner.
- Atmosphere engine internal model, verifier, controlled-live config gate, visual whisper, shadow metrics, and A/B safety smoke tests.
- Terminal compatibility lab, doctor guidance, reset safety, color capability diagnostics, and clean terminal recovery.
- User scene/profile config with controlled atmosphere profile keys.
- Benchmark/endurance/report hardening with honest planned-vs-actual execution diagnostics.
- README demo refresh with GIF-first v4 preview, MP4 link, and binary/retro posters.
- Canonical metadata alignment across Cargo, README, runtime identity, and AUR packaging.
- Release-candidate smoke script and release checklist.

Safety/defaults:
- Default runtime remains protected and identity: `application_mode = disabled`, `effective_runtime = identity`, `shadow_risk = identity`.
- `auto_color_drift` remains off by default.
- `storm` is not config-safe in controlled-live config/profile mode.
- No actual multithreaded terminal rendering; benchmark reports planned engine mode honestly.

## v3.9.0

Internal v4.0.0 ground-work phase. No public API or visual behavior changes.

- Atmosphere visual whisper engine with bounded A/B smoke testing
- Whisper wiring guard and runtime shadow metrics
- Zactrix Core eBPF-inspired architecture discipline
- Self-referential guard string avoidance pattern
- Phase 10.5: atmosphere config honesty + profile smoke hardening (27 new tests)
- Added v4 demo poster and MP4 assets for README preview
- Made the v4 README demo GIF-first and removed the obsolete demo GIF
- Replaced single v4 demo poster with binary and retro themed demo screenshots
- 568 deterministic tests, all passing

## v3.1.0

**Monolith Rain Engine.** Plain `cosmostrix` now launches signature Cosmostrix
Monolith Rain: sparse structured vertical data pillars with segmented blocks,
subtle spines, visible gaps, and a clear brightness hierarchy. Classic Matrix
glyph rain remains available with `cosmostrix --scene matrix`.

## v2.2.0

**Stability, maintainability, and supply-chain hardening release.** No visual
or CLI behavior changes.

- All `*.rs` files are under 1,000 gross lines (enforced by `check-rs-loc.sh` in `check-all`)
- Module splits: `src/cloud.rs` → `src/cloud/` (8 modules), `src/interactive.rs` → `src/interactive/` (6 modules), `src/main.rs` → `src/app.rs` + `src/cli.rs` + `src/info.rs` + `src/main.rs`
- Cloud tests split into `tests/mod.rs` (core) and `tests/tests_phosphor.rs` (phosphor/ghost)
- Added endurance testing documentation ([ENDURANCE.md](docs/ENDURANCE.md)) and resource summary script
- Added supply-chain hardening policy ([SUPPLY_CHAIN.md](docs/SUPPLY_CHAIN.md))
- Added terminal stability audit ([STABILITY_AUDIT.md](docs/STABILITY_AUDIT.md))
- Added SIMD feasibility audit ([SIMD_FEASIBILITY.md](docs/SIMD_FEASIBILITY.md))
- Engine module splits: `cloud/mod.rs` → `scene_runtime.rs` + `runtime_controls.rs` (scene switching and runtime controls extracted from core module)
- Fixed clippy module-inception and unused import warnings
- Regression suite passes, clippy clean, fmt clean

## v2.1.0

**Visual contrast & readability overhaul** — body glyphs are now clearly readable
with stronger head/body/trail hierarchy while preserving the calm cinematic identity.

- Tuned exponential trail decay (K: 3.0 → 1.8) for readable body glyphs across the full trail length
- Raised parallax brightness (far: 35→55%, mid: 80→90%) so depth layers are visible, not invisible
- Increased phosphor residual energy (120→160) for more visible CRT afterglow fadeout
- Extended head linger duration (100→300ms) for smoother cinematic head fade
- Added head self-bloom (12% white blend) making the head clearly the brightest element
- Softer head brightness mapping (0.5+0.5×hb → 0.7+0.3×hb) preventing abrupt head disappearance
- Raised luminance climate minimum (60→75%) and saturation minimum (50→70%) to prevent muddy/dim periods
- Raised fog vignette minimum (25→35%) to keep edge glyphs faintly visible
- Reduced far-layer glyph dimming (30→15%) — already dim from parallax brightness
- TrueColor green palettes now use 24-bit RGB gradients instead of ANSI 256-color indices, with proper bright green head instead of cyan-white
- Reduced profile luminance offsets (Monolith: -0.1→0, Void: -0.2→-0.1, Decay: -0.15→-0.05, Static: -0.25→-0.1)

**Safety & hardening fixes:**

- Tab key safely ignored (was toggling shading mode, causing ghost background glyph flood)
- Paste safety (bracketed-paste burst suppression ignores shortcut letters during paste)
- Pause/resume with cinematic smoothstep easing (no snap on resume)
- Color and charset transitions use cinematic top-to-bottom wave propagation
- Mouse mode default-off, opt-in with `--mouse`
- Bottom-row phosphor decay acceleration prevents "concrete wall" accumulation
- Ghost glyph threshold prevents stale charset from filling background on full redraw
- Safe terminal cleanup on all exit paths (RAII guard + `--reset-terminal`)

## v2.0.0

- Fixed stale glyph artifacts in the top visible rows during charset and theme changes.
- Fixed long-idle rain/trail resync issues with wall-clock redraw scheduling and focus/input redraw resync.
- Clarified benchmark dirty-cell and color-mode metrics so differential rendering reports are easier to interpret.
- Fixed direct-color auto-detection for `xterm-direct` and `tmux-direct`.
- Removed unused low-value support code while preserving rendering behavior.
- Completed 10h+ visual soak checks across Alacritty, Konsole, and WezTerm.
- Resource monitoring found no memory, file descriptor, thread, swap, CPU, or IO leak during the release soak.
