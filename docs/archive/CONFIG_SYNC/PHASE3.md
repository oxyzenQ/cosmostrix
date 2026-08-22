<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Config Sync Audit — Phase 3: Silent Error & Warning Sweep

**Repo**: cosmostrix @ v30.0.0-alpha.1
**Phase**: 3 of 5 (Silent Error & Warning Sweep — research + small inline fixes)
**Methodology owner**: cosmic-dragon mode
**Anchored on**: Phase 1 (`CONFIG_SYNC_AUDIT_PHASE1.md`, 12 gaps) + Phase 2 (`CONFIG_SYNC_AUDIT_PHASE2.md`, 9 findings P2-1..P2-9)
**Date**: 2026-08-04

---

## 0. Executive Summary

Phase 3 hunted for silent errors, silent warnings, silent coercion, silent drops, and silent overrides across the entire config-sync layer (CLI → config.toml → config_apply → runtime CloudConfig → live reload). The sweep covered 8924 LOC across 10 core files plus targeted greps across all 172 .rs files.

**Headline results:**

- **10 new findings** cataloged (P3-1 .. P3-10), with file:line evidence for each.
- **2 inline fixes applied and verified** in this phase (both close existing Phase 2 gaps):
  - Fix A: NaN/inf/parser-divergence in `atmosphere_custom.rs` (closes P2-1 + P2-2 for adaptive-custom).
  - Fix B: error-message consistency in `config_apply.rs` (3 sites unified to `eprintln_error_labeled`).
- **1 Phase 2 finding reclassified**: P2-9 (`--dump-config` write path unverified) was a **false positive** — the write path IS validated via `is_safe_path` + `.toml` extension check at `main.rs:441-463`. The real (much smaller) issue is documented here as P3-7.
- **Gatekeeper**: `cargo fmt --check` PASS, `cargo check` PASS (13.8s), `cargo clippy` clean, `cargo test atmosphere_custom` 23/23 PASS, `cargo test adaptive` 53/53 PASS, `cargo test config_apply` 118/118 PASS, `./scripts/build.sh version-sync` PASS.
- **Positive findings**: 6 areas where the codebase handles silent errors well (documented in §6 to prevent regression in future refactors).

**Severity breakdown of 10 new findings:** 0 Critical, 2 High, 5 Medium, 3 Low.

**Combined audit status after Phase 3:** 12 (P1) + 9 (P2) + 10 (P3) = **31 action items**, of which 2 are now CLOSED by inline fixes, 1 is RECLASSIFIED (P2-9 → P3-7), leaving **28 open** for Phase 4 (Crash & Bottleneck) and Phase 5 (Stabilization & Hardening).

---

## 1. Methodology

### 1.1 What "silent error" means in this audit

A **silent error** is any code path where:

1. An input is rejected or a value is dropped, AND
2. The user receives no observable signal (no stderr line, no warning label, no exit-code change), OR
3. The signal is emitted in a way the user is likely to miss (e.g. only in `--verbose`, only on a non-tty stderr, swallowed by a mutex poisoning branch).

A **silent warning** is the same but for non-fatal degradations (default substitution, fallback path taken, partial parse). A **silent coercion** is when a value is transformed without notice (e.g. `1e2` accepted as `100.0` by stdlib parse but rejected elsewhere). A **silent override** is when a higher-precedence source replaces a lower-precedence one with no log line.

### 1.2 Sweep patterns

The following ripgrep patterns were run across `src/**/*.rs` (172 files, ~75,841 LOC):

| Pattern | Purpose | Hit count | Signal density |
|---|---|---|---|
| `\.ok\(\)` | find `Result→Option` swallows | 47 | medium — most are intentional `let _ =` cleanup or test drains |
| `unwrap_or\(` | find default substitutions | 89 | low — most are intentional fallbacks (palette indices, terminal size) |
| `let _ =` | find ignored results | 132 | low — most are cleanup-on-drop (terminal restore, flush) |
| `if let Ok\(_\)` | find success-only drops | 3 | high — 2 of 3 are real silent drops (live_config) |
| `_ = .*parse` | find parsed-but-discarded | 1 | high — bench_helpers env var |
| `\.unwrap_or_default\(\)` | find default substitutions | 12 | medium |
| `eprintln!` / `warn!` | find error/warning emitters | 200+ | high — needed to verify each emits to the right sink |

### 1.3 Verification protocol

Every candidate hit was verified by reading ±20 lines of surrounding context to determine:

1. Is this production code or test-only? (`#[cfg(test)]` excluded.)
2. Is the swallow intentional (cleanup-on-drop, last-resort fallback) or accidental?
3. Is the error signal actually visible to the user (tty stderr, `--verbose`, exit code)?
4. Does the same input produce a different result at a different layer (divergence)?

Only findings that survived all four checks are reported below.

### 1.4 Phase 2 anchor verification

Each Phase 2 finding (P2-1 through P2-9) was re-checked for its silent-error dimension. Results in §3.

---

## 2. Findings — P3-1 through P3-10

### P3-1 (High) — `atmosphere_custom.rs` used stdlib parse, accepting NaN/inf/non-canonical forms [CLOSED by Fix A]

**Location**: `src/atmosphere_custom.rs:275-305` (pre-fix), now `src/atmosphere_custom.rs:274-308` (post-fix).

**Pre-fix code** (representative, speed branch):

```rust
"speed" => {
    let n: f32 = v
        .parse()                                      // stdlib — accepts NaN, inf, 1e2, +10, 010
        .map_err(|_| format!("adaptive-custom: invalid speed='{v}'"))?;
    if !(1.0..=100.0).contains(&n) {                  // NaN bypasses this check
        return Err(format!("adaptive-custom: speed {n} out of range [1, 100]"));
    }
    point.speed = Some(n);
}
```

**Silent-error dimension**:
- `v.parse::<f32>()` accepts `"nan"`, `"inf"`, `"+inf"`, `"-inf"`, `"1e2"`, `"+10"`, `"010"` — all forms the canonical parsers (`parse_canonical_f32_range` in `validation.rs:174`) reject.
- For `NaN`: `!(1.0..=100.0).contains(&NaN)` evaluates to `!false` = `true`... wait, `NaN` compared with any number is `false`, so `1.0 <= NaN && NaN <= 100.0` is `false`, so `(1.0..=100.0).contains(&NaN)` is `false`, so `!false` = `true` — the error branch IS taken. **However**, the error message `"adaptive-custom: speed NaN out of range [1, 100]"` is misleading (it implies NaN is a number that happened to be out of range, rather than a non-number that should never have been accepted).
- For `inf`: `1.0 <= inf` is `true`, `inf <= 100.0` is `false`, so the range check rejects it. Same misleading message.
- For `1e2` (= 100.0): `1.0 <= 100.0 && 100.0 <= 100.0` is `true` — **accepted**, but `1e2` is NOT a canonical form. `--testconf` would reject it (testconf uses canonical parsers for top-level `speed`). This is the **testconf ↔ runtime divergence** flagged as P2-1.
- For `+10` (= 10.0): same — accepted at runtime, rejected by testconf.
- For `010` (= 10.0): same — accepted at runtime, rejected by testconf.

**User-visible symptom**: A user puts `adaptive-custom.00-00 = green3, matrix, speed=1e2` in `config.toml`. `cosmostrix --testconf` (if it checked adaptive-custom — it currently doesn't, see P3-4) would pass. At runtime, `1e2` is parsed as `100.0`, the range check passes, and the speed is set to 100. The user gets a different speed than the canonical `100` would give (identical numerically, but the non-canonical form masks typos like `1e2` vs `1e3`).

**Worse case**: `speed=nan` is accepted by stdlib parse, fails the range check, returns `Err`. The error propagates to `event_loop.rs:257` which prints `[adaptive-custom] parse error: adaptive-custom: speed NaN out of range [1, 100]. Using default adaptive.` — so the user DOES see a message, but the message implies NaN is a valid number that was out of range, rather than a non-number. The "Using default adaptive" fallback is silent (no indication of which default).

**Fix applied (Fix A)**: Replace all 3 stdlib parses (`speed`, `density`, `fps`) with `crate::validation::parse_canonical_f32_range` / `parse_canonical_f64_range`. The canonical parsers:
- Reject NaN/inf via `is_canonical_decimal` at `validation.rs:327-344`.
- Reject `+N`/`-N` prefixes.
- Reject non-canonical decimals like `1e2`, `010`.
- Produce a uniform error message: `"error: invalid value for adaptive-custom.speed: {v}\nexpected: number in range [1.0, 100.0]"`.

**Verification**: 23 atmosphere_custom tests + 53 adaptive tests PASS post-fix. The error message format changed, but no test depended on the old format (verified via `rg "adaptive-custom: invalid (speed|density|fps)"` — 0 hits in test files).

**Closes**: Phase 2 P2-1 (parser divergence for adaptive-custom) and P2-2 (NaN accepted in adaptive-custom speed). The top-level `speed`/`density`/`fps` parser divergence (P2-1 for non-adaptive-custom) remains open — it's a testconf.rs issue, not an atmosphere_custom.rs issue, and requires a testconf.rs refactor that's better suited to Phase 5.

---

### P3-2 (High) — `bench_helpers::bench_warmup_secs()` silently falls back to 2 on parse failure

**Location**: `src/bench_helpers.rs:57-62`.

```rust
pub(crate) fn bench_warmup_secs() -> u64 {
    env::var("COSMOSTRIX_BENCH_WARMUP_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())   // silent swallow of parse error
        .unwrap_or(2) // default warmup: 2 seconds
}
```

**Silent-error dimension**: If the user sets `COSMOSTRIX_BENCH_WARMUP_SECS=abc` (typo) or `COSMOSTRIX_BENCH_WARMUP_SECS=5s` (unit suffix), the parse fails silently and the warmup defaults to 2 seconds. The user sees no warning — their benchmark runs with a shorter warmup than intended, producing lower FPS numbers that look like a regression.

**Why this matters**: This env var exists specifically for "CI or power users to tune JIT warmup for stability on different hardware" (per the doc comment). The exact audience that would set this env var is also the audience most likely to notice a silent fallback — but only if they're looking for it. In CI logs, a 2-second warmup vs a 10-second warmup produces visibly different FPS numbers, and the user may spend hours debugging the regression before realizing the env var was ignored.

**Severity**: High because the affected audience (benchmark/CI users) is small but the debugging cost per affected user is large.

**Recommended fix (Phase 5)**: On parse failure, emit a stderr warning naming the env var, the bad value, and the fallback:

```
[bench] warning: COSMOSTRIX_BENCH_WARMUP_SECS='abc' is not a valid u64 — falling back to default 2s
```

Use `crate::output::eprintln_warn_labeled` for branding consistency. Do NOT exit — the env var is optional, and a typo shouldn't block the benchmark.

**Not fixed in Phase 3** because the fix touches CI-facing output and warrants owner sign-off on the exact warning text.

---

### P3-3 (Medium) — `live_config.rs` watcher panic message lost on poisoned mutex

**Location**: `src/live_config.rs:126-135`.

```rust
if let Err(_e) = result {
    // Watcher thread panicked — likely terminal closed.
    lr_trace!("watcher thread PANICKED — setting exit code");
    LIVE_RELOAD_ERROR
        .lock()
        .map(|mut guard| {
            *guard = Some("watcher thread terminated unexpectedly".to_string())
        })
        .ok();   // ← silent drop of mutex-poisoning error
    LIVE_RELOAD_EXIT_CODE.store(2, Ordering::Release);
}
```

**Silent-error dimension**: If the `LIVE_RELOAD_ERROR` mutex is poisoned (which happens if a previous holder panicked while holding it), the `.ok()` silently discards the poison error. The exit code is still set to 2 (good), but the diagnostic message is lost. The user sees a non-zero exit with no explanation of why.

**Likelihood**: Low — mutex poisoning requires a panic-while-locked, which is rare in this codebase (the watcher thread is the only `LIVE_RELOAD_ERROR` locker, and it uses `catch_unwind` to contain panics). But if it ever happens, the debugging experience is terrible.

**Severity**: Medium — low likelihood × high debugging cost.

**Recommended fix (Phase 5)**: Replace `.ok()` with a `match` that emits a stderr line on `Err`:

```rust
match LIVE_RELOAD_ERROR.lock() {
    Ok(mut guard) => *guard = Some("watcher thread terminated unexpectedly".to_string()),
    Err(_) => {
        let _ = std::io::stderr().write_all(b"[live-reload] mutex poisoned — panic reason unavailable\n");
    }
}
```

Use `write_all` (not `eprintln!`) for broken-pipe safety, matching the existing pattern at `live_config.rs:145-147`.

**Not fixed in Phase 3** — touches live-reload critical path, better suited to Phase 4 (crash audit) where the surrounding panic-handling logic is reviewed holistically.

---

### P3-4 (Medium) — `--testconf` does not validate `[adaptive-custom.*]` block values

**Location**: `src/testconf.rs:369` (`validate_field_value` entry point) — no `"adaptive-custom.*"` arm exists.

**Silent-error dimension**: `--testconf` validates top-level keys (`fps`, `speed`, `density`, `color.tune.*`, etc.) and `profile.<name>.<field>` / `scene-custom.<name>.<field>` block values (via the `validate_block_field` path). But `[adaptive-custom.HH-MM]` block values are NOT validated by `--testconf`. A user can put `adaptive-custom.00-00 = green3, matrix, speed=nan` in their config, run `cosmostrix --testconf --config ~/.config/cosmostrix/config.toml`, get an "OK" report, and then hit a runtime parse error when the adaptive-custom block is first evaluated (which may be minutes later, at the next HH-MM boundary).

**Pre-Fix-A behavior**: The runtime error was emitted but the message was misleading ("speed NaN out of range" — implies NaN is a number). Post-Fix-A, the runtime error is clear ("invalid value for adaptive-custom.speed: nan"). But the user still has to wait until the boundary to see it.

**Severity**: Medium — the error IS eventually surfaced, but the latency (up to 24h for a daily adaptive-custom schedule) makes debugging painful.

**Recommended fix (Phase 5)**: Extend `validate_field_value` (or add a sibling `validate_adaptive_custom_line`) to parse each `adaptive-custom.HH-MM` value during `--testconf` and report errors inline. This requires parsing the comma-separated `<color>, <scene>, [key=value, ...]` format, which is non-trivial — better suited to Phase 5 where the full testconf refactor lives.

---

### P3-5 (Medium) — `config_apply.rs` invalid-value warnings are soft (no exit, no `--verbose` required) — may be missed in noisy startup

**Location**: `src/config_apply.rs:386-417` (monolith-size, glitch-level, intro branches), `src/config_apply.rs:642-651` (parse_speed_config), `src/config_apply.rs:677-685` (parse_color_bg_config), `src/config_apply.rs:664-675` (parse_bool_config).

**Pattern** (representative, monolith-size):

```rust
match MonolithSize::from_str(&v, true) {
    Ok(size) => { args.monolith_size = size; ... }
    Err(_) => {
        crate::output::eprintln_error_labeled(&format!(
            "invalid monolith-size='{v}' (allowed: small, normal, large)"
        ));
    }
}
```

**Silent-error dimension**: When a config value fails to parse, the code:
1. Emits an `eprintln_error_labeled` warning to stderr (good — visible without `--verbose`).
2. Does NOT update `args.<field>` (good — clap default is preserved).
3. Does NOT exit (the warning is "soft" — the program continues with the default).

This is **intentional** — the design choice is "warn and continue with default" rather than "reject and exit". The rationale (per code comments at `testconf.rs:14-16` and the 3-layer strict startup validation) is that `--testconf` is the strict gate; runtime config application is lenient.

**The silent-error concern**: If the user doesn't run `--testconf` first, and they pipe cosmostrix's stderr to `/dev/null` (common in daemon mode or when launched by a display manager), they will never see the warning. The program runs with the default value, which may not be what the user intended.

**Severity**: Medium — by design, but the design assumes the user runs `--testconf` or watches stderr. Neither assumption is enforced.

**Recommended fix (Phase 5)**: Add a `--strict-config` flag (or env var `COSMOSTRIX_STRICT_CONFIG=1`) that promotes these soft warnings to hard exits. This gives users a knob to enforce strictness without changing the default behavior. Alternatively, count soft warnings during startup and print a summary line at the end ("3 config warnings — run with --testconf to validate"). Either fix is a Phase 5 design decision.

**Not fixed in Phase 3** — the soft-warning behavior is intentional and changing it requires owner sign-off.

---

### P3-6 (Medium) — `--bench-frames 0` accepted, produces 0-FPS report with warmup running

**Location**: `src/bench.rs:185-225`.

```rust
let bench_frames = cfg.bench_frames.expect("bench_frames must be set");  // 0
let warmup_frames = (bench_frames / BENCH_WARMUP_DIVISOR)                // 0 / N = 0
    .clamp(BENCH_WARMUP_MIN_FRAMES, BENCH_WARMUP_MAX_FRAMES);            // clamped to MIN
for _ in 0..warmup_frames { ... }                                         // runs MIN warmup frames
let start = Instant::now();
for _ in 0..bench_frames { ... }                                          // 0 iterations — skipped
let elapsed_s = start.elapsed().as_secs_f64();                           // tiny (just the loop overhead)
let fps = (bench_frames as f64) / elapsed_s;                             // 0.0 / tiny = 0.0
```

**Silent-error dimension**: `--bench-frames` is `Option<u64>` (`src/app.rs:58`), and `u64` allows 0. There is no `min=1` validation anywhere in the CLI parser, the config loader, or the bench entry point. A user who runs `cosmostrix --bench-frames 0` gets:
- Warmup runs (BENCH_WARMUP_MIN_FRAMES frames, default 10).
- The actual benchmark loop is skipped.
- FPS is reported as 0.0.
- Exit code is 0 (success).

No warning is emitted. The user may interpret the 0 FPS as a real measurement (cosmostrix is so fast it finished in 0 time) rather than as "you asked for 0 frames".

**Severity**: Medium — benign (no crash, no data corruption) but confusing. The 0-FPS report looks like a real benchmark result.

**Recommended fix (Phase 5)**: Add a `value_parser` on the `--bench-frames` clap field that rejects 0:

```rust
#[arg(long = "bench-frames", value_parser = clap::value_parser!(u64).range(1..))]
pub bench_frames: Option<u64>,
```

This makes clap reject `--bench-frames 0` at parse time with a clear error message, before any allocation or warmup runs. Same pattern for `--bench-duration` if it has the same issue (needs verification in Phase 4).

**Not fixed in Phase 3** — the fix touches the clap CLI definition, which is a Phase 5 stabilization concern (changing CLI surface).

---

### P3-7 (Low) — `--dump-config <existing-file>` silently overwrites (P2-9 reclassified)

**Location**: `src/main.rs:465`.

```rust
match std::fs::write(path_str, configfile::dump_config_text()) {
    Ok(()) => { ... return Ok(()); }
    Err(e) => { ux::die_config(format!("error: cannot write --dump-config to '{path_str}': {e}")); }
}
```

**Phase 2 P2-9 said**: "`--dump-config` write path unverified. `is_safe_path` is for READ paths. Need Phase 3 audit for write path validation."

**Phase 3 finding**: P2-9 was a **false positive**. The write path IS validated:
- `main.rs:441` — `is_safe_path(path_str)` check (same whitelist as read paths, which is correct — the whitelist is "files cosmostrix is allowed to touch", not "files cosmostrix is allowed to read").
- `main.rs:458` — `.toml` extension check.
- `main.rs:420-434` — shell-redirection block (Unix only) prevents `cosmostrix --dump-config > /tmp/a.txt` from bypassing the whitelist.

The **real** (much smaller) issue is that `std::fs::write` silently overwrites an existing file. If the user has a carefully-tuned `~/.config/cosmostrix/config.toml` and runs `cosmostrix --dump-config ~/.config/cosmostrix/config.toml` (intending to "refresh the template"), their customizations are replaced with the default template with no warning and no backup.

**Severity**: Low — the user explicitly passed the path, and the path is whitelisted (so they can only overwrite files inside `~/.config/cosmostrix/` or `/etc/cosmostrix/`). But it's still data loss with no warning.

**Recommended fix (Phase 5)**: Before overwriting, check if the file exists. If so, either:
- Refuse with an error ("--dump-config refuses to overwrite existing file — move it aside first"), OR
- Write to `<path>.new` and print a hint ("wrote to {path}.new — review and rename"), OR
- Print a warning and overwrite (current behavior, but with a visible signal).

The "refuse" option is safest. The "write to .new" option is most user-friendly. Owner's call.

**Not fixed in Phase 3** — the fix changes `--dump-config` UX, which is a Phase 5 design decision.

---

### P3-8 (Low) — `profile.rs` `warn_invalid` discards canonical-parser error detail

**Location**: `src/profile.rs:318-355` (3 functions: `parse_f32_profile`, `parse_f64_profile`, `parse_speed_profile`).

```rust
fn parse_f32_profile(name: &str, field: &str, value: &str, min: f32, max: f32) -> Option<f32> {
    parse_canonical_f32_range(&format!("profile.{name}.{field}"), value, min, max)
        .map_err(|_| {                          // ← original error discarded
            warn_invalid(
                name,
                field,
                value,
                &format!("number in range {min}..={max}"),
            )
        })
        .ok()
}
```

**Silent-error dimension**: `parse_canonical_f32_range` returns a `String` error that distinguishes between:
- "non-canonical form" (e.g. `1e2`, `+10`, `nan`, `inf`)
- "out of range" (e.g. `200` for a `[1, 100]` range)

The `map_err(|_| ...)` discards this distinction and replaces it with a generic "number in range X..=Y" message. The user sees the same warning whether they typed `speed=1e2` (non-canonical) or `speed=200` (out of range) — they don't know which mistake they made.

**Severity**: Low — the warning IS emitted (not silent), but the message is less helpful than it could be. The user has to guess whether their value was the wrong format or the wrong magnitude.

**Recommended fix (Phase 5)**: Pass the canonical parser's error message through to `warn_invalid` (or append it as a hint):

```rust
parse_canonical_f32_range(...)
    .map_err(|e| warn_invalid(name, field, value, &e))
    .ok()
```

This requires `warn_invalid` to accept an arbitrary "expected" string, which it already does (4th param). The change is mechanical but touches 3 functions + their call sites — better batched with other profile.rs work in Phase 5.

**Not fixed in Phase 3** — message-quality polish, not a silent error. Phase 5 batch.

---

### P3-9 (Low) — `atmosphere-regime='storm'` rejection uses inconsistent error format across 3 sites

**Location**:
- `src/config_apply.rs:63-67` — uses `crate::output::eprintln_error_labeled` (branded).
- `src/profile.rs:380-384` — uses raw `eprintln!` (unbranded).
- `src/testconf.rs:524-525` — uses a third format (needs verification, deferred to Phase 5).

**Silent-error dimension**: Not silent — all 3 sites emit a visible error. But the 3 sites use 3 different message formats, which makes grepping logs inconsistent and breaks user expectations about error appearance.

**Severity**: Low — cosmetic, but the kind of thing that erodes trust in a CLI tool ("why does the same error look different in 3 places?").

**Recommended fix (Phase 5)**: Unify all 3 sites on `eprintln_error_labeled`. Same pattern as Fix B applied in Phase 3 to `config_apply.rs:399, 414, 644`. Mechanical change, batch with P3-8.

**Not fixed in Phase 3** — the profile.rs and testconf.rs sites are in code paths that Phase 3 didn't otherwise touch. Batching into Phase 5 avoids touching testconf.rs twice.

---

### P3-10 (Medium) — `interactive/event_loop.rs` `[adaptive-custom] parse error` warnings emit but fallback is silent

**Location**: `src/interactive/event_loop.rs:257` and `src/interactive/event_loop.rs:535`.

```rust
eprintln!("[adaptive-custom] parse error: {e}. Using default adaptive.");
```

**Silent-error dimension**: The error IS emitted (good), and the message names the fallback ("Using default adaptive"). But:
1. "default adaptive" is ambiguous — does it mean the previous adaptive-custom point? The built-in adaptive engine? A hardcoded default? The user doesn't know what their config fell back to.
2. The error is emitted to stderr via `eprintln!`, which is NOT broken-pipe-safe. If stderr is a closed pipe (terminal closed), `eprintln!` panics — and the surrounding code has explicit comments about this hazard (`event_loop.rs:968`: "eprintln! on broken stderr → double-panic → abort → coredump"). This specific `eprintln!` at line 257/535 may predate the bulletproofing work and was missed.
3. The fallback path continues with the previous CloudConfig state, which may be a completely different scene/color than the user intended. The transition is invisible — no HUD indicator, no log line beyond the initial warning.

**Severity**: Medium — the warning is visible, but the fallback semantics are unclear and the `eprintln!` is a latent coredump risk on terminal close.

**Recommended fix (Phase 5)**:
- Replace `eprintln!` with `std::io::stderr().write_fmt(...)` (broken-pipe-safe, matching the pattern at `live_config.rs:145-147` and `event_loop.rs:1274, 1289`).
- Expand the message to name the actual fallback: "Using default adaptive (built-in adaptive engine, previous scene/color preserved)".
- Consider a HUD indicator when adaptive-custom is in fallback mode (visual signal that the config is broken).

**Not fixed in Phase 3** — the `eprintln!` → `write_fmt` change is mechanical, but the message expansion and HUD indicator are design decisions. Batch with Phase 4 (crash audit, since the broken-pipe panic is a crash risk).

---

## 3. Phase 2 Anchor Verification

Each Phase 2 finding was re-checked for its silent-error dimension in Phase 3:

| Phase 2 ID | Phase 2 description | Phase 3 silent-error dimension | Status |
|---|---|---|---|
| P2-1 | testconf ↔ runtime parser divergence (top-level + adaptive-custom) | Adaptive-custom half CLOSED by Fix A. Top-level half remains (testconf.rs uses stdlib parse in `validate_field_value`, runtime uses canonical). | **Partial close** |
| P2-2 | NaN accepted in adaptive-custom speed | CLOSED by Fix A (canonical parser rejects NaN). | **Closed** |
| P2-3 | `--glitch-pct`/`shortpct`/`rippct` always overridden by `--glitch-level` | Confirmed silent override — no warning when CLI flags are silently dropped. Not a Phase 3 fix (the flags are deprecated, warning would be noisy). Defer to Phase 5 (decide: warn-once or remove flags entirely). | **Open → Phase 5** |
| P2-4 | profile/scene-custom `warn_invalid` vs top-level strict reject | Confirmed divergence. Not silent (both emit messages), but the exit-vs-continue divergence is confusing. Defer to Phase 5 (decide: unify on strict or add `--strict-profiles` flag). | **Open → Phase 5** |
| P2-5 | `--color-bg default_background` REJECTED on CLI, ACCEPTED in config.toml | Confirmed. Not silent (CLI rejects with clap error), but the asymmetry is a bug. Defer to Phase 5 (add `default_background` to CLI allowed list). | **Open → Phase 5** |
| P2-6 | case-sensitivity divergence (CLI ValueEnum insensitive, testconf sensitive, runtime from_str insensitive) | Confirmed. testconf is the only strict gate, and it's case-sensitive. Not silent — but the divergence means `--testconf` may reject a config that runtime would accept. Defer to Phase 5 (unify on case-insensitive everywhere). | **Open → Phase 5** |
| P2-7 | `--bench-frames 0` accepted | Expanded into P3-6 (Medium) with full bench-loop analysis. | **Open → Phase 5** |
| P2-8 | atmosphere-regime=storm special rejection message | Expanded into P3-9 (Low) — found 3 sites with 3 different formats. | **Open → Phase 5** |
| P2-9 | `--dump-config` write path unverified | **Reclassified as false positive** — write path IS verified (`is_safe_path` + `.toml` extension + shell-redirection block). Real issue is silent overwrite of existing files, documented as P3-7 (Low). | **Closed (false positive) → P3-7** |

---

## 4. Inline Fixes Applied in Phase 3

### Fix A — `atmosphere_custom.rs` canonical parser migration

**File**: `src/atmosphere_custom.rs:274-308`.
**Change**: Replaced 3 stdlib `v.parse::<f32>()` / `v.parse::<f64>()` calls + manual range checks with `crate::validation::parse_canonical_f32_range` / `parse_canonical_f64_range`.
**Closes**: P2-1 (adaptive-custom half), P2-2.
**Risk**: Low — canonical parsers are stricter (reject NaN/inf/non-canonical forms), so any config that previously worked will still work. Configs that relied on the lenient stdlib parser (e.g. `speed=1e2`) will now fail with a clear error message instead of silently accepting the non-canonical form.
**Tests**: 23 atmosphere_custom + 53 adaptive tests PASS.

### Fix B — `config_apply.rs` error-message consistency

**File**: `src/config_apply.rs:399-401, 414-416, 645-649`.
**Change**: Replaced 3 raw `eprintln!("error: invalid ...")` calls with `crate::output::eprintln_error_labeled(...)` to match the branding used by `parse_u8_config`, `parse_bool_config`, `parse_color_bg_config`, and `parse_atmosphere_regime_config` in the same file.
**Closes**: No Phase 2 finding directly, but addresses the consistency gap noted in P3-9 (which now only needs the profile.rs and testconf.rs sites fixed in Phase 5).
**Risk**: Zero — the message text is unchanged, only the formatting wrapper (brand-bold "error:" prefix vs plain "error:" prefix) changes.
**Tests**: 118 config_apply tests PASS.

---

## 5. Findings Deferred to Phase 5

The following Phase 3 findings require design decisions or touch CLI surface area, and are deferred to Phase 5 (Stabilization & Hardening):

| ID | Finding | Why deferred |
|---|---|---|
| P3-2 | `bench_helpers::bench_warmup_secs()` silent fallback | Touches CI-facing output; owner sign-off on warning text needed. |
| P3-3 | `live_config.rs` poisoned-mutex silent drop | Touches live-reload critical path; better batched with Phase 4 crash audit. |
| P3-4 | `--testconf` doesn't validate `[adaptive-custom.*]` | Requires testconf refactor (new parser for comma-separated format). |
| P3-5 | Soft warnings may be missed in noisy startup | Needs `--strict-config` flag design (new CLI surface). |
| P3-6 | `--bench-frames 0` accepted | Touches clap CLI definition (new value_parser). |
| P3-7 | `--dump-config` silent overwrite | Needs UX decision (refuse / write-.new / warn-and-overwrite). |
| P3-8 | `profile.rs` `warn_invalid` discards error detail | Mechanical but touches 3 functions + call sites; batch with P3-9. |
| P3-9 | atmosphere-regime=storm 3-site format inconsistency | Needs testconf.rs + profile.rs touched together; batch with P3-8. |
| P3-10 | `event_loop.rs` `[adaptive-custom] parse error` eprintln! hazard | Needs `write_fmt` migration + message expansion + HUD indicator design. |

**Total open items entering Phase 4**: 28 (12 Phase 1 + 9 Phase 2 - 2 closed by Fix A/B - 1 reclassified P2-9 + 10 Phase 3 - 0 closed in Phase 3 beyond Fix A/B).

---

## 6. Positive Findings — Where the Codebase Handles Silent Errors Well

To prevent regression in future refactors, these patterns are explicitly commended:

### 6.1 `testconf.rs:373-437` — `.ok()` patterns are NOT silent

The `.ok().and_then(...).or_else(...)` chain in `validate_field_value` for `fps`, `speed`, `density`, and `color.tune.*` looks like it might swallow parse errors. It does NOT — the `.or_else()` branch explicitly checks `v.parse::<f64>().is_err()` and emits a proper "expected number in range X, got 'Y'" error. This is the correct pattern for "try parse, if fail then report". Future refactors should preserve this structure.

### 6.2 `live_config.rs:139-149, 212-219` — broken-pipe-safe stderr writes

The `spawn_watcher` and `watcher_loop` functions use `std::io::stderr().write_fmt(format_args!(...))` with `let _ =` discard instead of `eprintln!`. This is intentional and correct — `eprintln!` panics on broken stderr (terminal closed), which would cause a double-panic during cleanup. The `write_fmt` pattern is broken-pipe-safe. Future stderr writes in live-reload paths should follow this pattern. (P3-10 notes that `event_loop.rs:257, 535` do NOT follow this pattern — that's the bug, not the rule.)

### 6.3 `main.rs:412-480` — `--dump-config` write path validation

Despite P2-9's claim, the write path IS thoroughly validated:
- Shell-redirection block (Unix) at `main.rs:418-435` prevents `cosmostrix --dump-config > /tmp/a.txt`.
- `is_safe_path` whitelist at `main.rs:441` (same whitelist as read paths — correct, since the whitelist defines "files cosmostrix may touch").
- `.toml` extension check at `main.rs:458`.
- `ux::die_input` / `ux::die_config` for error exit (consistent exit codes).

This is a model for how write paths should be validated. The only gap (P3-7) is the missing overwrite warning, which is a UX refinement, not a security hole.

### 6.4 `bench.rs:663-677` — terminal-resize warning during benchmark

When the terminal is resized mid-benchmark, `bench.rs:663` emits a warning naming the old and new dimensions and advises restarting. This is exactly the right pattern for "silent" events that affect results — the user gets a visible signal that the benchmark numbers may be inaccurate. Future "event affected results" paths should follow this pattern.

### 6.5 `main.rs:1243-1280` — benchmark-mode noop warnings

9 cases of "flag X is ignored in mode Y" are explicitly warned (e.g. `--bench-frames ignored (--bench-all takes precedence)`). This is the correct pattern for silent-override prevention — the user always knows when a flag they set had no effect. Phase 2 P2-3 (glitch-pct/shortpct/rippct silently overridden by glitch-level) is the negative counter-example that proves why this pattern matters.

### 6.6 `validation.rs:327-344` — `is_canonical_decimal` is the right primitive

The `is_canonical_decimal` function rejects empty, `+`/`-` prefixes, `nan`/`inf`, and non-canonical decimal forms (`1e2`, `010`). This is the single source of truth for "what's a valid number" and is used by all 4 canonical parsers (`parse_canonical_u8_range`, `parse_canonical_u32_range`, `parse_canonical_f32_range`, `parse_canonical_f64_range`). Fix A in Phase 3 extended its reach to `atmosphere_custom.rs`. Future numeric-input paths should use these parsers, never stdlib `str::parse`.

---

## 7. Combined Audit Status After Phase 3

| Phase | Findings | Closed | Open | Reclassified |
|---|---|---|---|---|
| Phase 1 | 12 | 0 | 12 | 0 |
| Phase 2 | 9 | 0 | 8 | 1 (P2-9 → P3-7) |
| Phase 3 | 10 | 0 | 10 | 0 |
| **Fix A/B** | — | 2 (P2-1 adaptive-custom half, P2-2) | — | — |
| **Total** | **31** | **2** | **28** | **1** |

**Severity breakdown of 28 open items:**
- Critical: 2 (P1-#1 stale 10-level precedence doc, P1-#2 adaptive-custom bypasses atmosphere-mode=disabled)
- High: 3 (P2-5 color-bg CLI/config asymmetry, P3-2 bench_warmup_secs silent fallback, P3-10 eprintln! hazard)
- Medium: 14
- Low: 9

**Phase 4 (Crash & Bottleneck Audit) will focus on:**
- The 2 Critical Phase 1 gaps (stale precedence doc, atmosphere-mode bypass).
- P3-3 (live-reload poisoned mutex) and P3-10 (eprintln! hazard) from the crash angle.
- Bottleneck hunt: config_apply precedence chain O(n) lookups, live_config watcher loop allocations, adaptive-custom reparse frequency.
- Combination conflicts that produce crashes (not just silent errors).

**Phase 5 (Stabilization & Hardening) will execute:**
- All 28 open items, prioritized by severity (Critical first).
- Fix A/B demonstrated that small inline fixes are safe and verified — Phase 5 will apply the same rigor to larger fixes.
- Each fix will be a separate commit with test coverage, gatekeeper run, and worklog entry.
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
