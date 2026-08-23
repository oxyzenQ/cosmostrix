# Triple Engine LTS Audit — v50.0.0-beta.4

<!-- SPDX-License-Identifier: GPL-3.0-only -->

> Internal independent QA session covering the three rendering engines on the
> road to stable LTS: Cosmic Dragon (diff-based render), Chroma Dragon (OKLab
> color pipeline), and Crystal Dragon (ambient adaptive color). Conducted on
> 2026-08-23 at commit `d102ba6`. Focus areas requested: memory safety / UB,
> race conditions / deadlock potential, performance bottlenecks in hot paths,
> white-box security, invariant violations, and edge-case handling (resize,
> zero-size, malformed config).

---

## 1. Audit Scope and Method

Three engines, ~37.3 K production+test LOC, audited with a surgical sweep
strategy (ripgrep pattern scans followed by targeted full reads of hot paths):

| Engine | Path | Total LoC | Role |
|--------|------|-----------|------|
| Cosmic Dragon | `src/cosmic_dragon_engine/` | 21 387 | Rain simulation, frame buffer, diff-based terminal renderer |
| Chroma Dragon | `src/chroma_dragon_engine/` | 13 208 | OKLab color pipeline, palettes, gradients, shaders, post-FX |
| Crystal Dragon | `src/crystal_dragon_engine/` | 2 738 | Ambient adaptive color: CPU/clock sensor, point system, phase scheduler |

Method, per engine:

1. ripgrep sweeps for `unsafe`, threading primitives
   (`thread::spawn`, `Mutex`, `Condvar`, `mpsc`, `Atomic`), panic vectors
   (`unwrap`, `expect`, `panic!`, `assert!`), cast sites, and edge-case
   guards (`== 0`, `is_empty`, `max(1)`, `saturating_*`).
2. Full reads of every hot or risk-critical file: `frame.rs`, `terminal/draw.rs`,
   `terminal/last_frame.rs`, `cloud/render.rs`, `cloud/spawn.rs` (reset),
   `chroma/gradient/mod.rs` (OKLab core), `chroma/colors_custom.rs`,
   `chroma/color_cache.rs`, `crystal/sensor/mod.rs`,
   `crystal/ambient/mod.rs`, `crystal/ambient_scheduler/mod.rs`,
   `crystal/point_system/mod.rs`.
3. Regression verification of every fix recorded in the 2026-08-17
   nightly.1 audit and the archived ambient-scheduler audit.

Relationship to prior audits: the 2026-08-17 LTS Depth Audit covered Cosmic +
Chroma but explicitly did **not** cover Crystal. This audit closes that gap,
re-verifies all prior fixes against beta.4, and re-runs the six focus areas
with fresh eyes across all three engines.

---

## 2. Executive Summary

| Engine | Critical | High | Medium | Low (new) | Verdict |
|--------|----------|------|--------|-----------|---------|
| Cosmic Dragon | 0 | 0 | 0 | 1 | **LTS-ready** — diff renderer invariants hold under resize |
| Chroma Dragon | 0 | 0 | 0 | 0 | **LTS-ready** — OKLab math is NaN/Inf-safe, config parsing is panic-free |
| Crystal Dragon | 0 | 0 | 0 | 1 | **LTS-ready** — first full audit; sensor/scheduler hardened well |

Zero unsafe blocks exist in any of the three engines. Zero new medium or
higher findings. The two new low findings are informational (theoretical
only, unreachable in practice) and do not block the stable release. All nine
prior fixes from earlier audits were verified still present at their exact
locations — no regressions since nightly.1.

---

## 3. Regression Check — Prior Fixes Verified Present

Every fix from the 2026-08-17 audit and the archived ambient-scheduler audit
was located in the beta.4 tree:

| ID | Fix | Verified at |
|----|-----|-------------|
| C-1 | Flash-wave head color precomputed once per wave (not per cell) | `cloud/rain.rs:653` writes `head_rgb`; `droplet/mod.rs:821-822` reads it |
| C-2 | Ambient ground-truth config re-read rate-limited to 1 per 5 s | `interactive/event_loop.rs:259,553-555` |
| C-3 | Termux detection cached via `OnceLock` (no per-keypress env locks) | `interactive/event_loop.rs:762-765` |
| S-1/S-2 | `debug_assert!` guards on `as u8` truncations in monolith | `cloud/monolith.rs:602,748` |
| H1 | Resize handler refreshes the SGR color cache | `interactive/event_loop.rs:1005-1011` |
| S4 | Ambient scheduler thread wrapped in `catch_unwind` | `crystal/ambient_scheduler/mod.rs:164-173` |
| AB-09 | Empty-schedule clears `last_applied` so re-added entries refire | `crystal/ambient_scheduler/mod.rs:265-279` |
| Defect A | Day-boundary refire for single-entry schedules | `crystal/ambient_scheduler/mod.rs:281-326` |
| Scene-name refire | Full-entry comparison (hour, minute, scene) | `crystal/ambient_scheduler/mod.rs:202-207` |

No regressions found. The deferred items (S-3, S-4, S-5, M-1, C-5) remain
open exactly as documented in the nightly.1 audit (see section 6).

---

## 4. New Findings

### 4.1 LOW-1 — Ambient scheduler treats a full channel as a dead receiver

Location: `src/crystal_dragon_engine/ambient_scheduler/mod.rs:258` and
`:317`.

```rust
if tx.try_send(entry.clone()).is_err() {
    // Receiver dropped (event loop exited). Terminate.
    return;
}
```

`SyncSender::try_send` fails for two distinct reasons: the receiver was
dropped (`Disconnected`), or the bounded channel (capacity 64) is full
(`Full`). The code terminates the scheduler thread on either error. If the
channel ever filled, the scheduler would die for the rest of the session
while the rain keeps running — ambient phases would silently stop applying.

Reachability analysis: the event loop drains `rx` via non-blocking
`try_recv` every frame (60+ Hz). The scheduler sends at most two entries per
wake (identity fire + day-boundary refire), and wakes at most once per minute
per boundary. Filling 64 slots would require dozens of phase fires within a
single frame period, which the schedule model (deduplicated `HH-MM` keys,
max 256 entries, at most one boundary per minute) cannot produce.

Impact: theoretical only. Recommendation for a future refactor: match on the
error kind and treat `Full` as "retry after a short sleep" instead of
terminating. Not a release blocker; the current behavior is safe under every
realistic schedule shape.

### 4.2 LOW-2 — `Cloud::reset` builds RNG ranges from raw dimensions

Location: `src/cosmic_dragon_engine/cloud/spawn.rs:48-54`.

`Cloud::reset` clamps its parameters into `self.cols` / `self.lines`
(lines 29-30) and uses the clamped values for every buffer allocation, but
then constructs `rand_line`, `rand_len`, and `rand_col` from the **raw**
`cols` / `lines` parameters. If a caller ever passed a size larger than
`MAX_TERMINAL_COLS/LINES`, droplets could be spawned beyond the visible
frame.

Safety analysis: this is panic-free and memory-safe. `saturating_sub` plus
`max(1)` keep every `Uniform::new_inclusive` range valid even for
degenerate zero sizes (single-value ranges are legal), and `Frame::set`
drops out-of-bounds writes via its `index()` bounds check, so an
over-sized spawn coordinate becomes a silent no-op rather than a panic.
Current callers pass crossterm `Resize(nw, nh)` values, which real terminals
keep within the clamp bounds.

Impact: defense-in-depth inconsistency only — the clamp discipline
documented at spawn.rs:26-28 is not applied uniformly. Recommendation:
derive the RNG ranges from `self.cols` / `self.lines` in a future refactor.
Not a release blocker.

---

## 5. Focus-Area Matrix

### 5.1 Memory Safety / Undefined Behavior

**Zero `unsafe` blocks** across all three engines (the only textual matches
are documentation references and a Windows `CONOUT$` comment in
`terminal/mod.rs`). All memory operations go through safe Rust. The
direct-indexing patterns that replace `.get()` chains (the "Cosmic Dragon
egg" optimizations) were each traced to their bounding guarantee:

- `frame.rs` cell accessors (`cell_at_index`, `cell_gen_at_index`): callers
  derive indices exclusively from `Frame::index()` (bounds-checked) or
  `dirty_indices()` (populated only by `set`/`set_force` after `index()`
  returned `Some`). This is the documented S-3 contract.
- `terminal/draw.rs` `last.cells[idx]`: the differential path only runs when
  `can_reuse_last` holds, which requires `last.width == frame.width` and
  `last.height == frame.height` (any dim change forces a full redraw and a
  `LastFrame` rebuild first). A `debug_assert!` on the sorted-last dirty
  index (draw.rs:246-251) verifies the invariant in debug builds at O(1)
  cost.
- `terminal/last_frame.rs` `reuse_or_new`: clear + `resize_with` always
  produces `cells.len() == width * height`; capacity-short buffers fall back
  to a fresh allocation. The resize-storm Vec reuse cannot produce a
  length/dimension mismatch.
- `chroma/color_cache.rs` `sgr(idx)`: every production caller passes an
  index obtained from `enumerate` over the cached palette itself.

Generation counters (`gen`, `dirty_gen`) use `wrapping_add` with a reset
threshold at `u32::MAX - 50_000_000` (frame.rs:68) — roughly 2.1 years at
60 FPS — after which a full stamp reset runs. No overflow path exists.

### 5.2 Race Conditions / Deadlock

The three engines own exactly one background thread: the ambient scheduler.
Its concurrency design was re-derived from source:

- Schedule swap happens under the mutex; the condvar notify uses a
  `SeqCst` generation counter (ambient_scheduler/mod.rs:89-100) that
  correctly closes the lost-wakeup TOCTOU window between the scheduler's
  snapshot-unlock and its `wait_timeout` re-lock. Both sides use `SeqCst`
  and the counter is loaded inside the lock scope on the scheduler side.
- The lock is released **before** `tx.try_send`, so a blocked send can never
  deadlock against `reload` (documented at mod.rs:225-227).
- The channel is bounded (`sync_channel(64)`), preventing unbounded queue
  growth.
- The thread is wrapped in `catch_unwind`; mutex poisoning exits the thread
  silently instead of panicking into the alternate screen.
- `AmbientSchedulerHandle::reload` tolerates a poisoned lock (swap skipped,
  notify still fired) — consistent with the documented poison contract.
- The only other shared state, `ambient_diag.rs` counters, uses atomics:
  `Relaxed` on pure performance counters (documented as sufficient) and a
  `Mutex<Option<String>>` for the last scene-change label with no lock
  ordering concerns (single lock, no nesting).

The render path itself is single-threaded by construction — no data races
are possible in the frame/terminal/cloud pipeline.

### 5.3 Performance Bottlenecks (Hot Paths)

The hottest path — `Terminal::draw`, executed every frame — was read in
full:

- Two-path strategy with a 12.5% dirty crossover (`DIRTY_THRESHOLD_RATIO`)
  plus an idle-frame fast path that skips the entire render body when no
  cells changed (draw.rs:98-101).
- The differential path performs **zero heap allocations**: `run_buf`,
  `ansi_buf`, and `dirty_flat` are pre-owned buffers on `Terminal`; the
  dirty list is cleared and re-extended in place.
- The full-redraw path is a single-pass row-RLE with style-change flushing,
  one `write_all`, and one `flush`.
- `Frame` clears dirty state in O(1) via the generation bump; the dirty
  index list is a `SmallVec` with 256 inline slots, avoiding heap spills
  for terminals up to ~2000 cells.
- `DrawCtx` pre-computes every per-frame factor once (column-coherence LUT,
  hue drift as a pre-converted integer, edge-fade and vignette LUTs built
  on resize, flash-wave radii and head color per wave). The per-cell shader
  path is stack-only.
- Crystal's scheduler sleeps in `Condvar::wait_timeout` between phase
  boundaries — zero polling cost, matching the owner's dynamic idle/wake
  directive.

Carry-over perf notes (M-1, C-5) remain minor and time-boxed as documented
in section 6.

### 5.4 Security (White-Box)

User-controlled input surfaces were each traced end to end:

- `parse_hex_color` (colors_custom.rs): strict length + `is_ascii_hexdigit`
  validation; every malformed input returns `Err`. Slicing is byte-safe
  because all accepted inputs are pure ASCII. No panic path.
- Ambient `HH-MM` keys: 5-byte format check with sentinel fallbacks
  (`unwrap_or(24)` / `unwrap_or(60)` guarantee rejection), so invalid keys
  surface as unknown-key hints, never as parse panics. The validator's
  string slicing is boundary-safe even for multi-byte UTF-8 inputs.
- Malformed ambient **values** during live reload are dropped silently
  (collect_ambient_schedule, ambient/mod.rs:343-350) — a half-edited config
  cannot crash the running session; strict validation runs in the
  `--testconf` / live-reload validation path.
- `AMBIENT_MAX_ENTRIES = 256` bounds sort cost and memory for
  script-generated configs (documented DoS hardening).
- Scene-name resolution requires a built-in scene or an existing
  `[scene-custom.<name>]` block; unresolved custom targets degrade to a
  logged no-op rather than a crash.
- The Crystal sensor reads CPU time from `/proc`-backed `cpustat` and
  otherwise derives its point from the wall clock. It never parses terminal
  escape-sequence responses, which structurally eliminates the OSC-response
  injection surface that ambient-color implementations often carry.
- Config content hashing uses SHA-512 via the `sha2` crate (no custom
  crypto).

No command execution, no path handling beyond the previously audited
`safepath` tilde expansion, and no unsafe deserialization exist in the
three engines.

### 5.5 Invariant Violations

Invariants were checked, not assumed:

- Dirty-index bounds: guaranteed by construction (section 5.1) and asserted
  in debug builds at O(1) cost in the renderer.
- `LastFrame` dimension/length coherence: enforced by `reuse_or_new`
  (always exact `resize_with` to `width * height`).
- Differential-render precondition (`last` dims == frame dims): enforced by
  the dim-change detection at the top of `draw` before the differential
  path is reachable.
- Color pipeline routing (TrueColor -> Chroma Dragon, everything else ->
  Legacy RGB): locked by unit tests in `runtime.rs`.
- Crystal CDF selection: the cumulative distribution's last entry is forced
  to exactly 1.0 to neutralize float-summation drift, and the binary-search
  result is clamped with `idx.min(themes.len() - 1)`.
- Point mapping: `(raw.round() as u8).clamp(POINT_MIN, POINT_MAX)` on every
  sensor path (CPU and CLOCK); percent is clamped to `0.0..=999.9` before
  the EMA, and a zero wall-delta sample returns the previous EMA instead of
  dividing by zero.

### 5.6 Edge-Case Handling

- **Resize**: crossterm resize events are debounced and coalesced
  (pending_resize); the handler rebuilds `Frame` from scratch (fresh dirty
  state — no stale indices from the old geometry), calls
  `force_draw_everything`, refreshes the SGR color cache (H1 fix), and
  `LastFrame::reuse_or_new` reallocations are amortized across drag storms.
  The first post-resize `draw` detects the dimension change and takes the
  full-redraw path, rebuilding the diff baseline before any differential
  frame runs.
- **Zero / degenerate size**: `Frame::new_with_bounds` clamps to
  `MIN_TERMINAL_COLS/LINES`; `Cloud::reset` clamps independently
  (defense in depth); phosphor guards `cols == 0 || lines == 0`; monolith
  and ghost events use `max(1)` on coordinates; message-box layout uses
  `saturating_*` arithmetic with `max(1)` floors. All RNG range
  constructors stay valid for degenerate inputs (LOW-2 notes the raw-dims
  style inconsistency, which remains panic-free).
- **Malformed config**: every parser returns `Result`; ambient entries with
  legacy multi-field values produce a migration message; empty scene names
  are rejected; live reload drops bad entries instead of crashing; strict
  validation surfaces errors with exit code 2 through `--testconf`.
- **Clock edge cases**: `SystemTime` before epoch falls back via
  `unwrap_or_default`; POSIX `localtime` failure falls back to minute 0;
  DST spring-forward (skipped entries never fire) and fall-back (repeated
  hour fires twice, idempotent apply) are documented and accepted in the
  scheduler module docs.
- **u32 generation wraparound**: handled 2+ years ahead of reach via
  `GEN_RESET_THRESHOLD` (frame.rs:63-68).

---

## 6. Known Deferred Items (Carried Forward)

The five deferred items from the nightly.1 audit were re-checked and remain
open, documented, and safe (all rated Low):

| ID | Location | Item | Status |
|----|----------|------|--------|
| S-3 | frame.rs cell accessors | Direct indexing on a caller contract | Documented; renderer-side O(1) debug_assert added in draw.rs partially covers it |
| S-4 | spawn.rs:561 | `as u64` truncation of RNG seed nanos | Harmless (seed entropy loss only) |
| S-5 | living_rain.rs:196 | `as u32` overflow at ~13.6 years uptime | Documented LTS ceiling |
| M-1 | rain.rs:447 | Per-frame Vec alloc during 300 ms palette transitions (<= 20 entries) | Time-boxed and small |
| C-5 | ecosystem.rs:359 | `Uniform` construction per atmospheric tick | Rare cadence |

Additionally, the runtime.rs `assert!` sites flagged by the panic-vector
sweep were confirmed to be entirely inside `#[cfg(test)]` modules —
production code paths in all three engines are free of bare `assert!` /
`unwrap()` on non-constant inputs (the two remaining `expect` calls bind to
compile-time-validated constants or statically-valid ranges).

---

## 7. LTS Recommendation

All three engines are **ready for stable LTS promotion** from a correctness,
safety, and robustness standpoint:

1. No critical, high, or medium findings. No unsafe code. No reachable
   panic paths on user-controlled input.
2. The two new low findings (LOW-1 channel-error conflation, LOW-2 raw-dims
   RNG ranges) are theoretical, panic-free, and suitable for a post-release
   polish pass — neither blocks the stable tag.
3. The previously unaudited Crystal Dragon engine turns out to be the most
   defensively coded of the three (strict key validation, bounded queues,
   graceful degradation everywhere), which is consistent with its
   config-driven design.
4. Recommended (optional, non-blocking) follow-ups for the first maintenance
   release: distinguish `Full` from `Disconnected` in the ambient scheduler
   send path, and align `Cloud::reset` RNG ranges with the clamped
   dimensions.

---

## 8. Verification Performed During This Audit

- ripgrep sweeps: unsafe blocks, concurrency primitives, panic vectors,
  cast sites, zero-size guards across all three engines.
- Full-file reads: frame.rs, terminal/draw.rs, terminal/last_frame.rs,
  cloud/render.rs, cloud/spawn.rs (reset), chroma/gradient/mod.rs,
  chroma/colors_custom.rs, chroma/color_cache.rs, crystal/sensor/mod.rs,
  crystal/ambient/mod.rs, crystal/ambient_scheduler/mod.rs,
  crystal/point_system/mod.rs, crystal/crystal_dragon_control/mod.rs,
  crystal/palette_groups/mod.rs.
- Regression greps for all nine prior fixes (section 3).
- Byte-level hexdump verification of one suspected source anomaly
  (colors_custom.rs `#[must_use]` — confirmed intact; earlier display was a
  terminal artifact).
- No code was modified during this audit phase.

---

## 9. Post-Audit Fix Trail

Both LOW findings were fixed immediately after the audit (owner-approved,
same session):

| Finding | Fix | Commit |
|---------|-----|--------|
| LOW-1 (try_send Full/Disconnected conflation) | `deliver()` helper in `ambient_scheduler/mod.rs` with a three-way `DeliverOutcome` contract: `Disconnected` still terminates the thread; `Full` enters a bounded retry (20 ms steps, 1 s cap — manual loop because `SyncSender::send_timeout` is unstable in std); a persistent stall drops the entry WITHOUT marking it applied, so the next scheduler wake re-attempts delivery. The day-boundary refire path also defers its day-seen marking when saturated. 4 unit tests pin the contract. | `9de2f44` |
| LOW-2 (Cloud::reset raw-dims RNG ranges) | `reset()` funnels into `reset_with_bounds()`, which shadows the raw parameters with the clamped values for the entire function body — RNG ranges, column tables, and per-cell LUTs now all agree with the clamped grid. New `reset_bench()` (mirroring `Frame::new_bench`) keeps the stress benchmarks consistent at bench-bounded dimensions; previously they ran a hybrid state where rain spawned at raw bench width but glitch/color coverage stopped at the interactive cap. 3 dimension-consistency unit tests added. | this commit |

Verification for both fixes: `gate-keepers.sh` 6/6 PASS, `cargo fmt --check`
PASS, `cargo clippy --bin cosmostrix --all-targets -- -D warnings` PASS,
`cargo test --bin cosmostrix ambient_scheduler` 17/17 PASS,
`cargo test --bin cosmostrix cloud` 258/258 PASS (full regression —
the reset refactor is a no-op for in-range interactive sizes).

---

Copyright (C) 2026 rezky_nightky (oxyzenQ). All rights reserved.
cosmostrix and the cosmostrix logo are trademarks of rezky_nightky (oxyzenQ).
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
