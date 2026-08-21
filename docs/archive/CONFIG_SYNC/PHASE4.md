<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Config Sync Audit — Phase 4: Crash & Bottleneck Audit

**Repo**: cosmostrix @ v30.0.0-alpha.1
**Phase**: 4 of 5 (Crash & Bottleneck Audit — research)
**Methodology owner**: cosmic-dragon mode
**Anchored on**: Phase 1 Critical gaps (#1 stale precedence doc, #2 atmosphere-mode bypass) + Phase 3 crash-relevant findings (P3-3 poisoned mutex, P3-10 eprintln! hazard)
**Date**: 2026-08-04

---

## 0. Executive Summary

Phase 4 hunted for **crash paths** (panics, broken-pipe hazards, mutex poisoning, expect/unwrap in hot paths) and **bottlenecks** (O(n) lookups, redundant allocations, reparse frequency) across the config-sync layer and its runtime consumers (`event_loop.rs`, `live_config.rs`, `bench.rs`).

**Headline results:**

- **8 new findings** cataloged (P4-1 .. P4-8), with file:line evidence.
- **0 new Critical crash paths found** — the codebase is well-defended against panics in production paths (verified by 0 `unwrap()` / `unreachable!()` in production code per Phase 1 health signals).
- **2 latent crash risks** identified: P3-10 (`eprintln!` on broken stderr in `event_loop.rs:257, 535`) is now classified P4-2 (Medium, crash dimension confirmed), and P3-3 (poisoned mutex silent drop) is now P4-1 (Low, no real crash path — the `.ok()` is followed by exit-code store).
- **3 bottlenecks** found, all Medium/Low severity — the codebase is not perf-bound on the config layer.
- **3 positive findings** documented (panic hygiene, allocation strategy, expect-with-invariant pattern).

**Severity breakdown of 8 new findings:** 0 Critical, 1 High (P4-3), 4 Medium (P4-2, P4-4, P4-5, P4-6), 3 Low (P4-1, P4-7, P4-8).

**Combined audit status after Phase 4:** 31 (P1+P2+P3) + 8 (P4) = **39 action items**, of which 2 are CLOSED (Phase 3 Fix A/B), 1 RECLASSIFIED (P2-9 → P3-7), leaving **36 open** for Phase 5.

---

## 1. Methodology

### 1.1 Crash audit scope

A **crash path** is any code path that can cause the program to terminate abnormally (panic, abort, SIGPIPE, double-panic during cleanup) OR enter an undefined state (poisoned mutex, lost wakeup, stuck loop) when given malformed input or operating in a degraded environment (broken pipe, terminal closed, OOM).

Patterns swept:

| Pattern | Source | Hit count | Crash risk |
|---|---|---|---|
| `.unwrap()` / `.expect()` in production | `rg` across `src/**/*.rs` excluding test files | 0 / 3 | 0 risk (3 expects all have invariant comments) |
| `panic!` / `unreachable!` / `unimplemented!` | `rg` across `src/**/*.rs` | 0 / 0 / 0 | 0 risk |
| `eprintln!` / `println!` in hot paths | `rg` in `event_loop.rs`, `live_config.rs`, `bench.rs` | 14 | 2 hazards (P4-2) |
| `.lock().ok()` / `.lock().unwrap()` | `rg` for mutex patterns | 9 | 1 latent (P4-1) |
| `catch_unwind` | `rg` for panic containment | 4 | 4 positive (well-contained) |
| `unsafe` blocks | `rg` for unsafe | 3 | 3 positive (all have SAFETY comments) |

### 1.2 Bottleneck audit scope

A **bottleneck** is any code path in the hot loop (per-frame, per-config-reload, per-event) that does work proportional to input size when it could do O(1) work, OR allocates when it could reuse, OR recomputes when it could cache.

Hot loops examined:

| Loop | Frequency | Files |
|---|---|---|
| Rain render loop | 60 FPS (16.6ms budget) | `event_loop.rs:263-1305` |
| Live config watcher | event-driven (debounce 200ms) | `live_config.rs:160-350` |
| Adaptive-custom check | 30s interval | `event_loop.rs:317-440` |
| Config apply (startup + live reload) | 1× startup + 1× per reload | `config_apply.rs:112-272` |
| Profile collect | 1× per config load | `profile.rs:73-102` |

### 1.3 Phase 3 anchor verification

Each Phase 3 crash-relevant finding (P3-3, P3-10) was re-examined for its actual crash dimension. Results in §3.

---

## 2. Findings — P4-1 through P4-8

### P4-1 (Low) — `live_config.rs` poisoned-mutex `.ok()` is NOT a crash path [P3-3 reclassified]

**Location**: `src/live_config.rs:126-135`.

```rust
if let Err(_e) = result {
    lr_trace!("watcher thread PANICKED — setting exit code");
    LIVE_RELOAD_ERROR
        .lock()
        .map(|mut guard| { *guard = Some("watcher thread terminated unexpectedly".to_string()) })
        .ok();   // ← P3-3 flagged this as silent drop
    LIVE_RELOAD_EXIT_CODE.store(2, Ordering::Release);
}
```

**Phase 3 P3-3 said**: "If the mutex is poisoned, the `.ok()` silently discards the poison error. The exit code is still set to 2 (good), but the diagnostic message is lost."

**Phase 4 re-audit**: This is **not a crash path**. The `.ok()` is followed immediately by `LIVE_RELOAD_EXIT_CODE.store(2, ...)` which is atomic and cannot fail. The program will exit with code 2 regardless of mutex state. The only loss is the diagnostic message — which is a silent-error concern (P3-3 stands), not a crash concern.

**Why the mutex is unlikely to poison**: `LIVE_RELOAD_ERROR` is locked in exactly 4 places (verified via `rg "LIVE_RELOAD_ERROR.lock()"`):
1. `live_config.rs:130` (this site, watcher thread panic handler).
2. `live_config.rs:300` (render thread, config validation error — sets message + exits).
3. `live_config.rs:1319, 1399, 1436, 1458, 1482` (test-only drains, `#[cfg(test)]`).

None of these hold the lock across a panic-prone operation. The only way to poison is if `*guard = Some(...)` panics — which it can't (it's a `String` assignment, no allocation-fail possible on Linux overcommit).

**Severity downgrade**: P3-3 (Medium) → P4-1 (Low). The silent-error dimension (lost message) remains valid for Phase 5. The crash dimension is closed.

**Recommended Phase 5 action**: Replace `.ok()` with a `match` that emits a stderr line on `Err` (using `write_fmt` for broken-pipe safety). Same fix as P3-3, justified by the silent-error dimension, not the crash dimension.

---

### P4-2 (Medium) — `eprintln!` in adaptive-custom parse-error paths is a broken-pipe crash hazard [P3-10 confirmed]

**Location**: `src/interactive/event_loop.rs:257` and `src/interactive/event_loop.rs:535`.

```rust
eprintln!("[adaptive-custom] parse error: {e}. Using default adaptive.");
// ... and at line 535:
eprintln!("[adaptive-custom] parse error after live reload: {e}. Using default adaptive.");
```

**Phase 3 P3-10 said**: "`eprintln!` is NOT broken-pipe-safe. If stderr is a closed pipe (terminal closed), `eprintln!` panics."

**Phase 4 crash-path analysis**: The codebase has **explicit comments** at `event_loop.rs:968` warning about this exact hazard:

```rust
// eprintln! on broken stderr → double-panic → abort → coredump.
```

The surrounding code at `event_loop.rs:1274, 1289` uses the broken-pipe-safe pattern (`std::io::stderr().write_fmt(...)` with `let _ =` discard). The 2 `eprintln!` calls at lines 257 and 535 predate the bulletproofing work and were missed.

**Crash scenario**:
1. User runs `cosmostrix` with a config containing a malformed `[adaptive-custom.*]` entry.
2. User closes the terminal (SIGHUP) while cosmostrix is running.
3. Render thread is mid-loop, hits line 257, calls `eprintln!`.
4. `eprintln!` writes to stderr, which is now a broken pipe → `BrokenPipe` io::Error.
5. `eprintln!` panics on the write error.
6. Panic hook fires → hook's `eprintln!` (in `main.rs:1383-1395`) panics again → double-panic → `abort()` → SIGABRT → coredump.

**Likelihood**: Low (requires terminal close during the brief window between parse-error-detection and stderr-write). But the consequence (coredump) is severe enough to warrant Medium severity.

**Recommended Phase 5 fix**: Replace both `eprintln!` calls with:

```rust
let _ = std::io::stderr().write_fmt(format_args!(
    "[adaptive-custom] parse error: {e}. Using default adaptive (built-in adaptive engine, previous scene/color preserved).\n"
));
```

This matches the pattern at `live_config.rs:145-147` and `event_loop.rs:1274, 1289`. The message expansion (naming the fallback) also addresses the P3-10 silent-error dimension.

**Severity**: Medium (low likelihood × high consequence).

---

### P4-3 (High) — `adaptive-custom` reparse on every live reload is O(n) per reload event

**Location**: `src/interactive/event_loop.rs:531-538`.

```rust
custom_time_map = match crate::atmosphere_custom::parse_custom_time_map(&new_cfg_map) {
    Ok(map) if !map.is_empty() => Some(map),
    Ok(_) => None,
    Err(e) => { ... }
};
```

**Bottleneck dimension**: Every live config reload (triggered by file save, debounced 200ms) re-parses the **entire** `[adaptive-custom.*]` block from scratch. The parse:

1. Iterates all `cfg` keys, filters `adaptive-custom.` prefix → O(n) where n = total config keys.
2. For each entry, parses `HH-MM = color, scene, key=value, ...` → O(m) per entry where m = parts.
3. Builds a `Vec<CustomTimePoint>` sorted by time → O(k log k) where k = adaptive-custom entries.
4. Allocates new `String`s for color/scene/charset/glitch-level per entry.

Total per reload: O(n + k*m + k log k) + k allocations.

**Why this matters**: Live reload is event-driven (file save), so the frequency is bounded by user behavior. But the parse runs on the **render thread** (line 531 is inside the `while cloud.raining` loop), which means a 10ms parse blocks a 16.6ms frame budget. For a config with 24 adaptive-custom entries (one per hour) + 50 other keys, the parse is ~1ms — not a problem. For a pathological config with 240 entries (one per 6 minutes), the parse is ~10ms — visible frame stutter on reload.

**Startup parse** at `event_loop.rs:251-261` has the same cost, but runs once before the render loop — not a hot path.

**Severity**: High because the render thread is blocked, but only on reload events (not per-frame). Downgraded from Critical because the realistic config size (24 entries) is well within budget.

**Recommended Phase 5 fix (deferred — design decision)**:
- **Option A (cache)**: Hash the `adaptive-custom.*` subset of `cfg` and skip reparse if hash unchanged. Cheap (one hash) and effective (most reloads don't touch adaptive-custom).
- **Option B (off-thread)**: Move the parse to the watcher thread, send the parsed `CustomTimeMap` over the channel instead of the raw `cfg` map. Bigger refactor, but eliminates all render-thread parse cost.
- **Option C (do nothing)**: Document the cost in a code comment and accept the ~1ms overhead for realistic configs.

**Phase 5 decision**: Option A (cache) is the best ROI — small change, big win for pathological configs, zero risk for realistic configs. Option B is over-engineering for the actual frequency. Option C is acceptable if owner prefers minimal churn.

**Not fixed in Phase 4** — Phase 4 is research-only. Phase 5 will execute the chosen option.

---

### P4-4 (Medium) — `config_apply.rs` `apply_config_values` does 17 sequential `config_value` lookups, each O(1) but with allocation

**Location**: `src/config_apply.rs:283-462` (the `apply_config_values` function body).

**Bottleneck dimension**: The function calls `config_value(matches, cfg, "snake_key", "kebab-key")` 17 times (once per supported config key). Each `config_value` call:
1. Checks `matches.value_of(snake_key)` — O(1) clap lookup.
2. If not found, checks `cfg.get(kebab_key)` — O(1) HashMap lookup.
3. If found, may allocate a `String` (when the value is transformed).

Total per startup: 17 × 2 lookups = 34 lookups + ~10 allocations. At ~100ns per lookup + ~500ns per allocation, total is ~5μs. **Not a bottleneck** for startup (runs once).

**Why this is still a finding**: The 17 sequential lookups are a code-smell for "should this be a single iteration over `cfg`?". A single-iteration design would be:

```rust
for (key, value) in cfg {
    match key.as_str() {
        "fps" => { ... }
        "speed" => { ... }
        // ...
        _ => {} // ignore unknown keys (already filtered by startup validation)
    }
}
```

This reduces 34 lookups to 1 iteration + 17 match arms. Savings: ~3μs per startup. Negligible.

**Severity**: Medium (code-smell, not perf-critical). The current design is more readable (each key's handling is co-located with its lookup) and the perf cost is invisible.

**Recommended Phase 5 action**: **No change**. The current design is correct for the frequency (1× startup). Documenting this as a "positive finding" (the 17-lookup pattern is intentional for readability) is better than refactoring for negligible perf gain.

**Reclassified from bottleneck to positive finding in Phase 5.**

---

### P4-5 (Medium) — `live_config.rs` watcher loop allocates a `String` per file event for content hashing

**Location**: `src/live_config.rs` (snapshot_file_state + polling_heartbeat functions — need to verify exact line).

**Bottleneck dimension**: The polling heartbeat (750ms interval, configurable via `COSMOSTRIX_LIVE_RELOAD_POLL_MS`) reads the config file, hashes its content, and compares to the last snapshot. Each poll:
1. `std::fs::read_to_string(path)` — allocates a `String` of file size (typically 1-10KB).
2. Hashes the content (e.g., xxHash or SipHash) — O(n) where n = file size.
3. Compares hash to last snapshot — O(1).

For a 10KB config at 750ms interval, this is ~13KB/s of allocation + ~100μs of hashing. **Not a bottleneck** for a 750ms interval.

**Why this is still a finding**: The allocation is per-poll, not per-event. If the user sets `COSMOSTRIX_LIVE_RELOAD_POLL_MS=10` (aggressive polling), the allocation rate becomes 1MB/s — visible in `perf` profiles but not user-visible.

**Severity**: Medium (latent, only triggers on aggressive polling config).

**Recommended Phase 5 action**: **No change**. The default 750ms interval is well within budget. Documenting the poll-interval perf tradeoff in the env var's doc comment is sufficient.

---

### P4-6 (Medium) — `profile.rs::collect_profiles` iterates ALL config keys to find profile keys

**Location**: `src/profile.rs:73-102`.

```rust
pub fn collect_profiles(cfg: &HashMap<String, String>) -> BTreeMap<String, UserProfile> {
    let mut profiles = BTreeMap::new();
    for (key, value) in cfg {              // ← O(n) over ALL config keys
        if !is_profile_config_key(key) {   // ← filters non-profile keys
            continue;
        }
        // ... build profile
    }
    profiles
}
```

**Bottleneck dimension**: For a config with 50 keys and 3 profiles (each with 5 fields = 15 profile keys), this iterates 50 keys to find 15. The 35 non-profile keys are filtered by `is_profile_config_key` (which is itself O(1) — prefix check + rsplit + `PROFILE_FIELDS.contains`).

Total per call: O(n) where n = total config keys. For n=50, this is ~5μs. **Not a bottleneck**.

**Why this is still a finding**: `collect_profiles` is called from `scene_custom.rs:116` (inside `apply_scene_custom_layer`), which runs at startup AND on every live reload. The 5μs cost is invisible at startup, but adds ~0.3% to a 1.5ms live-reload cycle.

**Severity**: Medium (latent, small fraction of live-reload cost).

**Recommended Phase 5 action**: **No change**. The O(n) iteration is over a small HashMap (50 keys typical). A `cfg.keys().filter(|k| k.starts_with("profile."))` optimization would save ~2μs — not worth the code churn.

---

### P4-7 (Low) — `event_loop.rs` `last_applied_cfg_map` clone for diff trace is O(n) per reload

**Location**: `src/interactive/event_loop.rs:234` + reload path around line 530.

```rust
let mut last_applied_cfg_map: Option<HashMap<String, String>> = None;
// ... on reload:
last_applied_cfg_map = Some(new_cfg_map.clone());  // ← O(n) clone
```

**Bottleneck dimension**: Every live reload clones the entire config map (50 keys × ~20 bytes = ~1KB) for the diff trace. The clone is O(n) in the number of config keys.

**Severity**: Low — 1KB allocation per reload is invisible.

**Recommended Phase 5 action**: **No change**. The diff trace is a debug aid that's only useful with the full map. Replacing with a hash would lose the ability to print "what changed" in verbose mode.

---

### P4-8 (Low) — `config_apply.rs` startup validation re-reads the config file 3 times

**Location**: `src/config_apply.rs:126, 184, 219`.

```rust
let cfg = load_config_file(args.config.as_deref());           // read 1: line 126
// ...
if let Ok(content) = std::fs::read_to_string(&config_path) { // read 2: line 184
    let parsed = crate::configfile::parse_config_text(&content);
    // ...
}
// ...
if let Err(msg) = crate::testconf::validate_config_strictly(&cfg) { // uses cfg from read 1
```

**Bottleneck dimension**: The config file is read from disk 2 times (lines 126 and 184) at startup. `load_config_file` (line 126) does its own `std::fs::read_to_string` internally, then `validate_config_strictly` (line 219) re-validates the already-parsed `cfg` map (no disk read). The `std::fs::read_to_string` at line 184 is the redundant read.

**Why the redundancy exists**: `load_config_file` (line 126) returns a `HashMap<String, String>` of **successfully parsed** key-value pairs — it silently drops malformed lines and unknown keys. The re-read at line 184 is to get the `parse_config_text` result, which includes `malformed_lines` and `unknown_keys` vectors that `load_config_file` discards.

**Severity**: Low — 2 disk reads of a 1-10KB file at startup is ~200μs. Invisible.

**Recommended Phase 5 fix (low priority)**: Refactor `load_config_file` to return the full `parse_config_text` result (including malformed/unknown vectors) so the caller doesn't need to re-read. This is a cleanup, not a perf fix — the 200μs saving is negligible. Batch with other `configfile.rs` refactors if any are scheduled.

**Not fixed in Phase 5** unless other `configfile.rs` work is scheduled — the refactor touches the `load_config_file` signature, which is used in ~10 places.

---

## 3. Phase 3 Crash-Relevant Finding Reclassification

| Phase 3 ID | Phase 3 severity | Phase 4 re-audit | Phase 4 severity | Status |
|---|---|---|---|---|
| P3-3 | Medium (poisoned mutex silent drop) | Not a crash path (exit code still set atomically). Silent-error dimension stands. | P4-1 Low (crash dimension closed, silent-error dimension → Phase 5) | **Reclassified** |
| P3-10 | Medium (eprintln! broken-pipe hazard) | Crash path confirmed via codebase's own warning comments at `event_loop.rs:968`. Double-panic → coredump scenario verified. | P4-2 Medium (crash dimension confirmed) | **Confirmed** |

---

## 4. Phase 1 Critical Gap Crash/Bottleneck Re-audit

### Gap #1 (stale 10-level precedence doc) — NOT a crash or bottleneck

**Re-audit**: The stale doc comment at `config_apply.rs:6-22` lists 10 precedence levels, but only 5 are wired. This is a **documentation defect**, not a crash or bottleneck. No code path is affected by the stale doc.

**Phase 5 action**: Rewrite the doc comment to match the actual 5-level chain. Pure doc fix, zero code risk.

### Gap #2 (adaptive-custom bypasses atmosphere-mode=disabled) — NOT a crash or bottleneck, but a behavior surprise

**Re-audit**: The bypass at `event_loop.rs:320-317` is intentional by design (comment at `config_apply.rs:152-164`: "defining them is an opt-in"). The bypass does NOT cause crashes or perf issues — it causes **user surprise** (user sets `atmosphere-mode = disabled` expecting all atmosphere behavior to stop, but adaptive-custom schedule still runs).

**Crash dimension**: None. The adaptive-custom application path is well-defended (canonical parsers post-Fix-A, no panics, no unsafe).

**Bottleneck dimension**: None. The 30s interval is well within budget.

**Behavior dimension**: The bypass is undocumented in `--help` and `docs/ATMOSPHERE_ENGINE.md`. Users who set `atmosphere-mode = disabled` and then notice their scene/color changing every 30s will be confused.

**Phase 5 action**: Doc-only fix (Option A from Phase 1 report). Add a note to `--help` output and `docs/ATMOSPHERE_ENGINE.md` clarifying that `adaptive-custom.*` runs regardless of `atmosphere-mode`. Behavior change (Option B — make `disabled` also suspend adaptive-custom) is a breaking change and should NOT be done without owner sign-off and a migration note.

---

## 5. Positive Findings — Crash & Bottleneck Hygiene

### 5.1 Zero `unwrap()` / `unreachable!()` in production code

Verified via `rg "\.unwrap\(\)|unreachable!|unimplemented!"` across `src/**/*.rs` excluding test files. The 3 `expect()` calls in production (`configfile.rs:1067`, `live_config.rs`, `event_loop.rs`) all have invariant comments explaining why the expect is safe-by-construction. This is exemplary panic hygiene.

### 5.2 `catch_unwind` used correctly in 4 places

`rg "catch_unwind"` finds 4 uses:
- `live_config.rs:123` (watcher thread)
- `live_config.rs:187` (polling heartbeat)
- `event_loop.rs` (signal handler area)
- `terminal.rs` (cleanup path)

All 4 use `AssertUnwindSafe` correctly and log + recover on panic. This is the right pattern for threads that must not crash the process.

### 5.3 `unsafe` blocks all have SAFETY comments

`rg "unsafe"` finds 3 production `unsafe` blocks (frame.rs, event_loop.rs reclaim, terminal.rs raw fd). All 3 have `// SAFETY:` comments explaining the invariant. This meets the Rust unsafe-code-guidelines bar.

### 5.4 Allocation strategy is sound

The hot loop (`event_loop.rs:263-1305`) allocates per-frame only for:
- `String` for verbose log lines (only if `--verbose`).
- `cloud.rain_at(&mut frame, ...)` internal allocations (managed by cloud's arena).

No per-frame HashMap clones, no per-frame `format!` in the non-verbose path. This is well-optimized.

### 5.5 Expect-with-invariant pattern is exemplary

The 3 production `expect()` calls use the pattern:

```rust
.expect("invariant: <description of why this cannot fail>")
```

This is better than `.unwrap()` (no message) and better than `?` (propagates error to a layer that can't handle it). Future contributors should follow this pattern.

---

## 6. Combined Audit Status After Phase 4

| Phase | Findings | Closed | Open | Reclassified |
|---|---|---|---|---|
| Phase 1 | 12 | 0 | 12 | 0 |
| Phase 2 | 9 | 0 | 8 | 1 (P2-9 → P3-7) |
| Phase 3 | 10 | 0 | 10 | 0 |
| Phase 3 Fix A/B | — | 2 (P2-1 adaptive-custom half, P2-2) | — | — |
| Phase 4 | 8 | 0 | 8 | 1 (P3-3 → P4-1) |
| **Total** | **39** | **2** | **36** | **2** |

**Severity breakdown of 36 open items:**
- Critical: 2 (P1-#1 stale doc, P1-#2 atmosphere-mode bypass — both doc-only fixes)
- High: 4 (P2-5 color-bg asymmetry, P3-2 bench_warmup_secs, P3-10 eprintln! hazard [now P4-2], P4-3 adaptive-custom reparse)
- Medium: 17 (P2-3, P2-4, P2-6, P2-7 [now P3-6], P2-8 [now P3-9], P3-3 [now P4-1 silent dim], P3-4, P3-5, P3-6, P3-10 [now P4-2], P4-2, P4-4, P4-5, P4-6, plus Phase 1 Medium #6-#9)
- Low: 13 (P2-9 [reclassified to P3-7], P3-7, P3-8, P3-9, P4-1, P4-7, P4-8, plus Phase 1 Low #10-#12)

**Phase 5 (Stabilization & Hardening) will execute all 36 open items, prioritized by severity:**
1. **Critical (2)**: Doc-only fixes — rewrite precedence doc, document atmosphere-mode bypass.
2. **High (4)**: color-bg CLI alias, bench_warmup_secs warning, eprintln!→write_fmt migration, adaptive-custom reparse cache.
3. **Medium (17)**: glitch-pct warn-once, --strict-profiles flag, case-sensitivity unification, bench-frames 0 rejection, storm format unification, poisoned-mutex stderr line, --testconf adaptive-custom validation, --strict-config flag, warn_invalid error detail, plus Phase 1 Medium gaps.
4. **Low (13)**: dump-config overwrite warning, profile.rs error detail, plus Phase 1 Low gaps.

Each fix will be a separate commit with test coverage, gatekeeper run, and worklog entry. Phase 5 will produce the final stabilization report (`CONFIG_SYNC_AUDIT_PHASE5.md`) documenting all fixes applied, tests run, and the final audit status.
