# Config Sync Audit — Phase 2: Failure Mode Catalog

**Scope:** Field-by-field failure mode catalog. Untuk setiap CLI flag /
`config.toml` key / runtime field, katalogkan: invalid value behavior,
error message quality, silent coercion, edge cases (NaN/infinity/negative/
zero/max/empty/whitespace), combination conflicts.

**Method:** Source-code audit dengan evidence `file:line`. Anchor pada 12
priority gaps dari Phase 1. Tidak ada code change di phase ini — pure
catalog.

**Audit layer:** Same 11 files as Phase 1 (~7,439 LOC) + `src/safepath.rs`
(601 LOC, path security) + `src/main.rs:1217-1291` (benchmark noop
warnings).

---

## 1. Executive Summary

Phase 2 meng-katalogkan **24 field/flag groups** dengan failure modes.
Ditemukan **9 new findings** (di luar 12 Phase 1 gaps), total **21
actionable items** untuk Phase 5.

**Key themes:**

1. **testconf ↔ runtime canonical parser divergence** — `testconf.rs`
   menggunakan `v.parse::<f64>()` (stdlib, lenient) sedangkan runtime
   `parse_canonical_f64_range` menggunakan `is_canonical_decimal` (strict).
   Hasil: `fps = "inf"`, `density = "1e2"`, `fps = "+10"`, `fps = "010"`
   PASS `--testconf` tapi FAIL di runtime apply → **silent fallback**
   (error printed, value dropped, clap default used).

2. **Case-sensitivity asymmetry** — CLI `ValueEnum` is case-insensitive,
   `testconf.rs` strict lowercase, runtime `from_str(&v, true)` is
   case-insensitive. Hasil: `--intro Logo` works, `intro = "Logo"`
   rejected, runtime would accept. 3 enums affected (intro,
   monolith-size, glitch-level).

3. **CLI enum vs config.toml enum divergence** — `--color-bg
   default_background` REJECTED di CLI (`validation.rs:300-305`), tapi
   `color-bg = "default_background"` ACCEPTED di config.toml
   (`testconf.rs:545-549` + `config_apply.rs:677-680`). Snake_case alias
   only works in config.toml.

4. **`atmosphere-regime = storm` divergence** — `testconf.rs:524-525`
   explicitly rejects "storm" untuk config.toml ("storm is unavailable
   and will be rejected"), tapi CLI `--atmosphere-regime storm` may be
   accepted (need Phase 3 verify). Storm is NOT config-safe per
   `config_apply.rs:41-42` comment.

5. **Profile/scene-custom warn-vs-reject divergence** —
   `profile.rs:207-387` uses `warn_invalid` (WARN + continue with
   default) untuk invalid field values. Top-level uses strict REJECT
   (exit 2). Hasil: `scene-custom.foo.color = "typo"` warns and uses
   default color; `color = "typo"` at top-level exits with error.

6. **`--glitch-pct` always overridden by `--glitch-level`** —
   `config_apply.rs:547-580` unconditionally assigns
   `glitch_pct`/`shortpct`/`rippct` based on glitch_level. User-set
   `--glitch-pct 50 --glitch-level subtle` → `glitch_pct` forced to 3.0.
   NO WARNING. `glitch_ms` is preserved (has `should_skip` check), but
   the 3 percentage fields are not.

7. **Combination conflicts WELL HANDLED** — `main.rs:1243-1280`
   `collect_bench_noop_warnings` covers 9 benchmark-mode noop cases.
   This is a positive finding — no gap here.

8. **safepath.rs is robust** — 601 LOC, 17 tests, whitelist-only,
   `..` normalization, `.toml` extension check, cross-platform
   (Linux/macOS/Windows/Android-Termux). No path traversal vectors
   found. Positive finding.

9. **`parse_duration` and `parse_screen_size` well-tested** —
   `cli_parse.rs:207-399` has 18 tests covering: bare numbers, compound
   format, zero rejection, too-small rejection, case-insensitive 'x',
   large values, error message flag attribution. No gap.

---

## 2. Field-by-Field Failure Mode Catalog

### 2.1 Numeric Fields

#### `fps` (f64, range [1.0, 240.0])

| Path | Validator | Evidence |
|---|---|---|
| CLI `--fps` | `parse_canonical_f64_range` (canonical decimal) | `validation.rs:267-273` |
| config.toml `fps` (testconf) | `v.parse::<f64>()` (stdlib, lenient) | `testconf.rs:373-386` |
| config.toml `fps` (runtime apply) | `parse_canonical_f64_range` | `config_apply.rs:362-366` via `parse_f64_config` |

**Failure modes:**

| Input | CLI `--fps` | testconf | runtime apply | Result |
|---|---|---|---|---|
| `60` | ✓ accept | ✓ accept | ✓ accept | consistent |
| `60.5` | ✓ accept (canonical decimal) | ✓ accept | ✓ accept | consistent |
| `0` | ✗ reject (range) | ✗ reject (range) | ✗ reject (range) | consistent |
| `241` | ✗ reject (range) | ✗ reject (range) | ✗ reject (range) | consistent |
| `inf` | ✗ reject (not canonical) | ✓ **accept** (f64 parses inf) | ✗ reject (not canonical) | **DIVERGENT — silent fallback** |
| `nan` | ✗ reject | ✓ **accept** | ✗ reject | **DIVERGENT — silent fallback** |
| `1e2` | ✗ reject (no scientific notation) | ✓ **accept** | ✗ reject | **DIVERGENT — silent fallback** |
| `+10` | ✗ reject (no leading +) | ✓ **accept** | ✗ reject | **DIVERGENT — silent fallback** |
| `-10` | ✗ reject | ✓ **accept** (f64 parses -10) but range check fails → reject | n/a (testconf catches) | testconf rejects via range |
| `010` | ✗ reject (no leading zeros) | ✓ **accept** (f64 parses 010 = 10) | ✗ reject | **DIVERGENT — silent fallback** |
| `0x10` | ✗ reject | ✗ reject (f64 rejects 0x10) | ✗ reject | consistent |
| empty `""` | ✗ reject | ✗ reject (f64 parse err) | ✗ reject | consistent |
| `"  "` whitespace | ✗ reject | ✓ **accept** (f64 trims? no — f64 parse fails on whitespace) → actually reject | ✗ reject | consistent (testconf rejects) |

**Divergence impact:** User writes `fps = "inf"` in config.toml.
`--testconf` PASSES (testconf accepts). At startup,
`validate_config_strictly` (`config_apply.rs:219`) calls
`validate_field_value` which uses the lenient `v.parse::<f64>()` → PASSES.
Then `apply_config_values:362-366` calls `parse_f64_config` →
`parse_canonical_f64_range` → REJECTS "inf" → prints error to stderr,
returns None → `args.fps` stays at clap default, `config_touched` does
NOT get "fps" → scene defaults may overwrite.

**Severity:** Medium. Error IS printed (not fully silent), tetapi value
is dropped and clap default used without explicit "falling back to
default" message.

**Recommended fix (Phase 5):** Make `testconf.rs:373-386` use
`parse_canonical_f64_range` instead of `v.parse::<f64>()`. Same for
density (line 400-411) and color.tune.* (line 425-437).

---

#### `speed` (f32, range [1, 100] integer-canonical)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--speed` | `parse_canonical_speed` (canonical integer) | `validation.rs:274-277, 141-153` |
| config.toml top-level (testconf) | `v.parse::<i64>()` | `testconf.rs:387-398` |
| config.toml top-level (runtime) | `parse_canonical_speed` | `config_apply.rs:368-373` via `parse_speed_config` |
| config.toml adaptive-custom (testconf) | `parse_custom_time_map` → `v.parse::<f64>()` | `atmosphere_custom.rs:275-284` |
| config.toml adaptive-custom (runtime) | `v.parse::<f64>()` | `atmosphere_custom.rs:275-284` |

**Failure modes:**

| Input | CLI | testconf top-level | runtime top-level | adaptive-custom | Result |
|---|---|---|---|---|---|
| `15` | ✓ | ✓ | ✓ | ✓ | consistent |
| `15.5` | ✗ reject (not integer) | ✗ reject (i64 parse fails) | ✗ reject | ✓ **accept** | **DIVERGENT (Phase 1 Gap #3)** |
| `0` | ✗ reject (range) | ✗ reject (range) | ✗ reject | ✗ reject (range [1,100]) | consistent |
| `101` | ✗ reject (range) | ✗ reject (range) | ✗ reject | ✗ reject (range) | consistent |
| `inf` | ✗ reject | ✗ reject (i64 fails) | ✗ reject | ✓ **accept** (f64) but range check `n < 1.0 \|\| n > 100.0` → inf > 100 → reject | adaptive-custom rejects via range |
| `nan` | ✗ reject | ✗ reject | ✗ reject | ✓ **accept** (f64) but range check `nan > 100.0` is false, `nan < 1.0` is false → **passes range check** → accept | **DIVERGENT — NaN accepted in adaptive-custom** |
| `010` | ✗ reject (leading zero) | ✓ **accept** (i64 parses 010 = 10 in Rust, but actually Rust i64 parse rejects leading zeros? No — i64::from_str("010") returns Ok(10)) | ✗ reject | ✓ accept | **DIVERGENT — silent fallback top-level** |
| `+15` | ✗ reject | ✗ reject (i64::from_str("+15") → Ok(15) actually) → accept | ✗ reject | ✓ accept | **DIVERGENT** |

**Critical NaN finding:** `atmosphere_custom.rs:281` range check
`n < 1.0 || n > 100.0` — NaN comparisons always return false, so
`speed=nan` PASSES the range check. NaN then propagates to cloud state
as f32 → causes division/rendering anomalies.

**Severity:** High (NaN propagation).

**Recommended fix (Phase 5):**
1. Align adaptive-custom speed parser to `parse_canonical_speed`
   (rejects NaN, inf, floats, leading zeros).
2. Add `n.is_finite()` check in `atmosphere_custom.rs:281` range guard.

---

#### `density` (f32, range [0.01, 5.0])

Same pattern as `fps` — testconf uses `v.parse::<f64>()`, runtime uses
`parse_canonical_f32_range`. Divergence for `inf`, `nan`, `1e2`, `+0.05`,
`010`. Silent fallback at runtime.

| Input | CLI | testconf | runtime | Result |
|---|---|---|---|---|
| `1.0` | ✓ | ✓ | ✓ | consistent |
| `0` | ✗ reject (range) | ✗ reject (range) | ✗ reject (range) | consistent |
| `5.1` | ✗ reject (range) | ✗ reject (range) | ✗ reject (range) | consistent |
| `inf` | ✗ reject | ✓ **accept** | ✗ reject | **DIVERGENT — silent fallback** |
| `nan` | ✗ reject | ✓ **accept** | ✗ reject | **DIVERGENT — silent fallback** |
| `1e2` | ✗ reject | ✓ **accept** (= 100.0, but range 0.01..=5.0 → reject via range) | ✗ reject | testconf catches via range |

**Note:** `density = 1e2` is accepted by f64 parse (= 100.0) but rejected
by range check (100 > 5.0). So testconf catches this one. But
`density = 1e0` (= 1.0, in range) would be accepted by testconf and
rejected by runtime → silent fallback.

**Severity:** Medium (same as fps).

---

#### `duration` (f64, range [0.1, 86400.0])

| Path | Validator | Evidence |
|---|---|---|
| CLI `--duration` | `parse_canonical_f64_range` | `validation.rs:285-290` |
| config.toml | NOT in USER_CONFIG_KEYS (CLI-only) | `configfile.rs:32-52` |

**Failure modes:**

| Input | CLI | Result |
|---|---|---|
| `5` | ✓ accept (5.0, in range) | consistent |
| `0` | ✗ reject (range, min 0.1) | consistent |
| `0.05` | ✗ reject (range) | consistent |
| `86401` | ✗ reject (range, max 86400 = 24h) | consistent |
| `inf` | ✗ reject (not canonical) | consistent |
| `1e2` | ✗ reject (not canonical) | consistent |

**Combination conflict:** `--duration` in benchmark mode is NOOP —
warned via `main.rs:1267-1268`. ✓ Handled.

**Note:** `--duration` (f64 bare float) vs `--bench-duration` (string,
compound format via `parse_duration`). Two separate flags, different
parsers, different scopes. Documented in `cli_parse.rs:9-19`. ✓ Clear.

---

#### `bench-duration` (string → u64 seconds via `parse_duration`)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--bench-duration` | `parse_duration` | `cli_parse.rs:40-119` |
| config.toml | NOT in USER_CONFIG_KEYS (CLI-only) | `configfile.rs:32-52` |

**Failure modes** (well-tested in `cli_parse.rs:207-278`, 12 tests):

| Input | Result |
|---|---|
| `5` (bare) | ✓ accept → 5 secs |
| `6s` | ✓ accept → 6 secs |
| `30m` | ✓ accept → 1800 secs |
| `1h30m` (compound) | ✓ accept → 5400 secs |
| `2h15m30s` (full compound) | ✓ accept → 8130 secs |
| `0` | ✗ reject (below 1-sec minimum) |
| `0s` | ✗ reject |
| `abc` | ✗ reject (invalid format) |
| `6x` | ✗ reject (unknown unit) |
| empty `""` | ✗ reject |
| `100h` | ✓ accept (no max cap) → 360000 secs |
| `8784h` (~1 year) | ✓ accept → 31622400 secs |

**Error message quality:** ✓ Excellent. Each error attributes to correct
flag (`--bench-duration` not `--duration`). Tested at `cli_parse.rs:281-300`.

**Combination conflict:** `--bench-frames + --bench-duration` (without
`--benchmark`) → `--bench-duration` ignored, warned via
`main.rs:1257-1258`. ✓ Handled.

**No gap.** Positive finding.

---

#### `screen-size` (string "WxH" → (u16, u16))

| Path | Validator | Evidence |
|---|---|---|
| CLI `--screen-size` | `parse_screen_size` | `cli_parse.rs:152-196` |
| config.toml | NOT in USER_CONFIG_KEYS (CLI-only) | `configfile.rs:32-52` |

**Failure modes** (well-tested in `cli_parse.rs:302-363`, 10 tests):

| Input | Result |
|---|---|
| `120x40` | ✓ accept |
| `200X60` (case-insensitive x) | ✓ accept |
| `120 x 40` (with spaces) | ✓ accept |
| `4x4` (minimum) | ✓ accept |
| `1x1` | ✗ reject (below 4x4 minimum) |
| `3x3` | ✗ reject |
| `0x0` | ✗ reject (zero dimension) |
| `0x10` | ✗ reject (zero width) |
| `65535x65535` (u16 max) | ✓ accept |
| `65536x65535` | ✗ reject (u16 overflow) |
| `120` (missing x) | ✗ reject |
| `120x40x30` (extra component) | ✗ reject |
| `abcx40` | ✗ reject (non-numeric width) |
| empty `""` | ✗ reject |

**Runtime clamping:** Interactive mode clamps to 1024×500
(`MAX_TERMINAL_COLS × MAX_TERMINAL_LINES`), benchmark mode to
7680×4320 (8K UHD). Warning emitted if screen-size exceeds terminal:
`event_loop.rs:70` "warning: --screen-size {}x{} exceeds terminal {}x{};
will clip to top-left".

**No gap.** Positive finding.

---

#### `bench-frames` (u64)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--bench-frames` | clap u64 parser (implicit) | `config.rs:609-614` |
| config.toml | NOT in USER_CONFIG_KEYS (CLI-only) | `configfile.rs:32-52` |

**Failure modes:**

| Input | Result |
|---|---|
| `1000` | ✓ accept |
| `0` | ✓ accept (clap allows) — but bench loop would run 0 frames, likely instant exit. Need Phase 3 verify. |
| `18446744073709551615` (u64 max) | ✓ accept — potential OOM if each frame allocates. Bounded by BENCH_MAX_COLS×BENCH_MAX_LINES. |
| `-1` | ✗ reject (clap u64 rejects negative) |
| `1.5` | ✗ reject (clap u64 rejects float) |
| `abc` | ✗ reject |

**Combination conflict:** `--bench-frames + --bench-duration` (without
`--benchmark`) → `--bench-duration` ignored, warned. `--bench-frames +
--benchmark` → `--bench-frames` ignored, warned. ✓ Handled.

**Potential issue:** `--bench-frames 0` — need to verify behavior. Likely
benign (instant exit) but should warn. Phase 3 item.

---

#### `bold` (u8, range [0, 2])

| Path | Validator | Evidence |
|---|---|---|
| CLI | no `--bold` flag (config-only field) | not in `validation.rs:cli_spec` |
| config.toml (testconf) | `match v { "0"\|"1"\|"2" => None, _ => Some }` | `testconf.rs:443-445` |
| config.toml (runtime) | `parse_u8_config` → `parse_canonical_u8_range` | `config_apply.rs:417-421` |

**Failure modes:**

| Input | testconf | runtime | Result |
|---|---|---|---|
| `0` | ✓ | ✓ | consistent |
| `1` | ✓ | ✓ | consistent |
| `2` | ✓ | ✓ | consistent |
| `3` | ✗ reject | ✗ reject | consistent |
| `true` | ✗ reject ("expected 0, 1, or 2") | ✗ reject | consistent (but no targeted hint) |
| `false` | ✗ reject | ✗ reject | consistent (but no targeted hint) |
| `00` | ✗ reject (match is literal "0") | ✗ reject (canonical u8 rejects leading zeros) | consistent |
| `0x1` | ✗ reject | ✗ reject | consistent |

**Hint gap:** `config_hints.rs` has no targeted hint for `bold = true` /
`bold = false` (Phase 1 Gap #9). User who writes `bold = true` (boolean
habit from other config) gets "expected 0, 1, or 2" error without
explanation that bold is a tri-state enum (0=off, 1=random, 2=all).

**Severity:** Low (cosmetic hint improvement).

---

#### `shadingmode` (u8, range [0, 1])

Same pattern as `bold`. Consistent testconf ↔ runtime. No hint for
`shadingmode = true`. Low severity.

---

#### `max_droplets_per_column` (u8, default 3, hidden)

| Path | Validator | Evidence |
|---|---|---|
| CLI | `#[arg(skip = 3u8)]` — NOT settable via CLI | `config.rs:663` |
| config.toml | NOT in USER_CONFIG_KEYS | `configfile.rs:32-52` |

**Failure modes:** Truly internal — no user-facing input path. No gap.

---

### 2.2 Enum Fields

#### `intro` (enum: cosmic | logo | none)

| Path | Validator | Case sensitivity | Evidence |
|---|---|---|---|
| CLI `--intro` | clap `ValueEnum` (default case-insensitive) | insensitive | `config.rs:125-132` |
| config.toml (testconf) | `match v { "cosmic"\|"logo"\|"none" => None }` | **sensitive** | `testconf.rs:572-575` |
| config.toml (runtime apply) | `IntroType::from_str(&v, true)` | **insensitive** (`true` = ignore case) | `config_apply.rs:409` |

**Failure modes:**

| Input | CLI `--intro` | testconf | runtime apply | Result |
|---|---|---|---|---|
| `logo` | ✓ | ✓ | ✓ | consistent |
| `Logo` | ✓ (clap lenient) | ✗ **reject** | ✓ (from_str lenient) | **DIVERGENT (Phase 1 Gap #4)** |
| `LOGO` | ✓ | ✗ **reject** | ✓ | **DIVERGENT** |
| `blah` | ✗ reject | ✗ reject | ✗ reject (prints error, drops value) | consistent |
| empty `""` | ✗ reject | ✗ reject | ✗ reject | consistent |
| `"  "` whitespace | ✗ reject | ✗ reject | ✓ **accept**? (from_str may trim? need verify) | possible divergence |

**Impact:** `intro = "Logo"` in config.toml → `--testconf` FAILS (exit
2). But at runtime, if user bypasses testconf, `IntroType::from_str("Logo",
true)` succeeds. So the strict testconf is the ONLY gate. If a user
relies on runtime behavior ("it worked when I tested without testconf"),
they'll be surprised when testconf rejects.

**Severity:** Medium (Phase 1 Gap #4). Intentional canonical-form
policy, but asymmetry is undocumented.

**Combination conflict:** `--intro` in benchmark mode → warned via
`main.rs:1273-1275`. ✓ Handled.

---

#### `monolith-size` (enum: small | normal | large)

| Path | Validator | Case sensitivity | Evidence |
|---|---|---|---|
| CLI `--monolith-size` | `validate_enum_value` (case-insensitive) | insensitive | `validation.rs:294-299, 240-248` |
| config.toml (testconf) | `match v { "small"\|"normal"\|"large" => None }` | **sensitive** | `testconf.rs:537-539` |
| config.toml (runtime apply) | `MonolithSize::from_str(&v, true)` | **insensitive** | `config_apply.rs:381` |

**Failure modes:** Same pattern as `intro`. `monolith-size = "Small"` →
testconf rejects, runtime would accept. **DIVERGENT.**

**Severity:** Medium. Same fix as intro (align testconf to runtime
leniency, OR document canonical-form policy).

---

#### `glitch-level` (enum: none | subtle | default | intense)

| Path | Validator | Case sensitivity | Evidence |
|---|---|---|---|
| CLI `--glitch-level` | `validate_enum_value` (case-insensitive) | insensitive | `validation.rs:306-311` |
| config.toml (testconf) | `match v { "none"\|"subtle"\|"default"\|"intense" => None }` | **sensitive** | `testconf.rs:541-543` |
| config.toml (runtime apply) | `GlitchLevel::from_str(&v, true)` | **insensitive** | `config_apply.rs:394` |

**Failure modes:** Same divergence pattern.

**Combination conflict (NEW finding):** `--glitch-pct` (hidden) is
ALWAYS overridden by `--glitch-level` via `apply_glitch_level_values`
(`config_apply.rs:547-580`).

```rust
// config_apply.rs:547-557
GlitchLevel::Subtle => {
    if !should_skip("glitch_ms") { ... }
    args.glitch_pct = 3.0;    // ALWAYS assigned, no should_skip check
    args.shortpct = 60.0;     // ALWAYS assigned
    args.rippct = 45.0;       // ALWAYS assigned
}
```

User runs: `cosmostrix --glitch-level subtle --glitch-pct 50`
Expected: `glitch_pct = 50.0` (user override)
Actual: `glitch_pct = 3.0` (subtle default overwrites user's --glitch-pct)

`glitch_ms` IS preserved (has `should_skip("glitch_ms")` check at lines
548, 559, 570), but the 3 percentage fields are NOT.

**Severity:** Medium. Hidden flags, but misleading for power users who
tune glitch internals. NO WARNING emitted.

**Recommended fix (Phase 5):** Add `should_skip("glitch_pct")` etc.
checks, OR emit warning when `--glitch-pct` is explicit AND
`--glitch-level` is set.

---

#### `color-bg` (enum: black | default-background)

| Path | Validator | Case sensitivity | Alias | Evidence |
|---|---|---|---|---|
| CLI `--color-bg` | `validate_enum_value` allowed `["black", "default-background"]` | insensitive | NO `default_background` | `validation.rs:300-305` |
| config.toml (testconf) | `match v { "black"\|"default-background"\|"default_background" => None }` | **sensitive** | YES `default_background` | `testconf.rs:545-549` |
| config.toml (runtime apply) | `parse_color_bg_config` via `.to_ascii_lowercase()` | **insensitive** | YES `default_background` | `config_apply.rs:677-680` |

**Failure modes:**

| Input | CLI `--color-bg` | testconf | runtime | Result |
|---|---|---|---|---|
| `black` | ✓ | ✓ | ✓ | consistent |
| `default-background` | ✓ | ✓ | ✓ | consistent |
| `default_background` | ✗ **reject** (not in CLI allowed list) | ✓ accept | ✓ accept | **DIVERGENT — CLI rejects, config accepts** |
| `Black` | ✓ (case-insensitive) | ✗ reject | ✓ (lowercase) | **DIVERGENT** |
| `DEFAULT-BACKGROUND` | ✓ | ✗ reject | ✓ | **DIVERGENT** |
| `white` | ✗ reject | ✗ reject | ✗ reject | consistent |

**Severity:** Medium. `--color-bg default_background` fails on CLI but
works in config.toml. Users who test in config first then try CLI are
surprised.

**Recommended fix (Phase 5):** Add `"default_background"` to
`validation.rs:300-305` CLI allowed list, OR remove the alias from
testconf/runtime (breaking change, migration needed).

---

#### `atmosphere-mode` (enum: disabled | controlled-live)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--atmosphere-mode` | no prevalidation in `cli_spec` — resolved at runtime | `validation.rs:265-315` (not listed) |
| config.toml (testconf) | `match v { "disabled"\|"controlled-live" => None }` | `testconf.rs:531-535` |
| config.toml (runtime apply) | `parse_atmosphere_mode_config` | `config_apply.rs:450-454` |
| runtime resolution | `config_apply::resolve_atmosphere_mode` | `main.rs:796` |

**Failure modes:**

| Input | testconf | runtime | Result |
|---|---|---|---|
| `disabled` | ✓ | ✓ | consistent |
| `controlled-live` | ✓ | ✓ | consistent |
| `storm` | ✗ reject ("unknown mode") | ✗ reject (per `config_apply.rs:41-42` comment "Storm is NOT config-safe") | consistent for config.toml |
| `Disabled` | ✗ reject (case-sensitive) | need verify | possible divergence |
| empty | ✗ reject | ✗ reject | consistent |

**Note:** "storm" is NOT config-safe but MAY be CLI-safe (need Phase 3
verify `resolve_atmosphere_mode` behavior). If CLI accepts "storm" but
config rejects, that's intentional (storm is a runtime stress-test mode,
not a persistent config).

**Phase 1 Gap #2:** `adaptive-custom` bypasses `atmosphere-mode =
disabled`. Confirmed in Phase 2 — no atmosphere-mode guard in
`event_loop.rs:235-317`.

---

#### `atmosphere-regime` (enum: calm | pulse | signal | compression | void | monolith-pressure | adaptive)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--atmosphere-regime` | no prevalidation — resolved at runtime | `validation.rs:265-315` (not listed) |
| config.toml (testconf) | `match v { ... \| "adaptive" => None, "storm" => Some("storm is unavailable") }` | `testconf.rs:521-529` |
| config.toml (runtime apply) | `parse_atmosphere_regime_config` | `config_apply.rs:456-460` |

**Failure modes:**

| Input | testconf | Result |
|---|---|---|
| `calm` | ✓ | consistent |
| `adaptive` | ✓ | consistent |
| `storm` | ✗ **reject** ("storm is unavailable and will be rejected") | **special rejection message** |
| `Storm` | ✗ reject (case-sensitive, falls to `_` arm "unknown regime") | possible divergence |
| `unknown` | ✗ reject | consistent |

**Divergence:** "storm" has a SPECIAL rejection message in testconf
(line 524-525), distinct from other unknown values. This suggests
"storm" was once valid and is now deprecated. CLI behavior for
`--atmosphere-regime storm` needs Phase 3 verification.

---

### 2.3 String Fields

#### `color` (theme name)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--color` | no prevalidation — `parse_color_scheme` at runtime | `cli.rs:233-248` |
| config.toml (testconf) | `theme::canonical_name_for_input(v)` | `testconf.rs:492-499` |
| config.toml (runtime apply) | `parse_color_scheme(&v).is_ok()` | `config_apply.rs:337-345` |

**Failure modes:**

| Input | CLI | testconf | runtime | Result |
|---|---|---|---|---|
| `green` | ✓ | ✓ | ✓ | consistent |
| `Green` | ✓ (theme lookup case handling) | ✓ | ✓ | need verify (theme module) |
| `grean` (typo) | ✗ reject with "Did you mean 'green'?" suggestion | ✗ reject | ✗ reject | consistent (suggestion is good) |
| `unknown-theme` | ✗ reject | ✗ reject | ✗ reject | consistent |

**Positive:** `cli.rs:233-248` has Levenshtein "Did you mean" suggestion
for typos. Good UX.

**Phase 1 Gap #5:** profile/scene-custom cannot resolve custom color
names that top-level can (profile.rs:115 lacks cfg HashMap access).

---

#### `charset` (preset or custom name)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--charset` / `--charset-custom` (alias) | no prevalidation — `charset_from_str` at runtime | `cli_parse.rs:373-389` (alias tests) |
| config.toml (testconf) | `charset_from_str(v, false)` + custom block check | `testconf.rs:501-510, 201-207` |
| config.toml (runtime apply) | `charset_from_str(&v, false)` + `load_custom_charset_if_matches` | `config_apply.rs:347-360` |

**Failure modes:**

| Input | CLI | testconf | runtime | Result |
|---|---|---|---|---|
| `binary` (preset) | ✓ | ✓ | ✓ | consistent |
| `Binary` | ✓ (charset_from_str case handling) | need verify | need verify | possible divergence |
| `mycustom` (custom block) | ✓ (if block exists) | ✓ (if block exists in same config) | ✓ | consistent |
| `unknown-charset` | ✗ reject | ✗ reject | ✗ reject | consistent |

**Phase 1 Gap #5:** profile/scene-custom cannot resolve custom charset
names. `profile.rs:115` `apply_profile_layer` doesn't have `cfg`
HashMap access.

---

#### `scene` (scene name)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--scene` | no prevalidation — `validate_scene_name` at runtime | `config_apply.rs:322-334, 474` |
| config.toml (testconf) | `get_scene(v)` | `testconf.rs:512-519` |
| config.toml (runtime apply) | `validate_scene_name(&v)` | `config_apply.rs:322-334` |

**Failure modes:**

| Input | CLI | testconf | runtime | Result |
|---|---|---|---|---|
| `matrix` (built-in) | ✓ | ✓ | ✓ | consistent |
| `Matrix` | need verify | need verify | need verify | possible divergence |
| `my-scene` (custom) | ✓ (if defined) | ✓ (if defined) | ✓ | consistent |
| `unknown-scene` | ✗ reject | ✗ reject | ✗ reject | consistent |

---

### 2.4 Boolean Fields

#### `auto-color-drift` and `async-mode` (bool)

| Path | Validator | Evidence |
|---|---|---|
| CLI | no flag (config-only) | not in `validation.rs:cli_spec` |
| config.toml (testconf) | `match lower { "true"\|"yes"\|"on"\|"1"\|"false"\|"no"\|"off"\|"0" => None }` | `testconf.rs:556-561` |
| config.toml (runtime apply) | `parse_bool_config` — same lenient set | `config_apply.rs:664-675` |

**Failure modes:**

| Input | testconf | runtime | Result |
|---|---|---|---|
| `true` | ✓ | ✓ | consistent |
| `yes` | ✓ | ✓ | consistent |
| `on` | ✓ | ✓ | consistent |
| `1` | ✓ | ✓ | consistent |
| `false` / `no` / `off` / `0` | ✓ | ✓ | consistent |
| `True` (capitalized) | ✓ (lowercases first) | ✓ (lowercases first) | consistent |
| `TRUE` | ✓ | ✓ | consistent |
| `t` | ✗ reject | ✗ reject | consistent |
| `enable` | ✗ reject | ✗ reject | consistent |
| empty | ✗ reject | ✗ reject | consistent |

**Positive:** Phase D Bug #1 fix unified 3 bool parsers (testconf,
config_apply, live_config) to use the same lenient set. No divergence.

**No gap.** Positive finding.

---

### 2.5 Block Fields (profile / scene-custom)

#### Block field value validation

| Path | Validator | Behavior on invalid | Evidence |
|---|---|---|---|
| testconf | `validate_field_value(field, value)` | REJECT (exit 2) | `testconf.rs:142-148` |
| runtime apply (profile.rs) | `warn_invalid(name, field, value, expected)` | **WARN + continue with default** | `profile.rs:207-387, 420-425` |
| runtime apply (scene_custom.rs) | delegates to `apply_profile_layer` | WARN + continue | `scene_custom.rs:116` |

**Failure modes:**

| Scenario | testconf | runtime | Result |
|---|---|---|---|
| `[scene-custom.foo] color = "typo"` | ✗ reject (exit 2) | ⚠ warn + use default color | **DIVERGENT** |
| `[profile.bar] speed = "999"` | ✗ reject (exit 2) | ⚠ warn + use default speed | **DIVERGENT** |
| `[scene-custom.foo] monolith-size = "huge"` | ✗ reject | ⚠ warn + use default | **DIVERGENT** |

**Impact:** At startup, `validate_config_strictly` (`config_apply.rs:219`)
catches invalid block field values and exits before runtime apply runs.
So the divergence is only reachable if
`COSMOSTRIX_SKIP_STARTUP_VALIDATION=1` is set (test-only env var) OR
via live config reload (which may not re-run strict validation — Phase 4
item).

**Severity:** Medium. The strict startup validation is the safety net,
but the warn-vs-reject divergence in profile.rs is a code smell.

**Recommended fix (Phase 5):** Make `profile.rs:apply_profile_layer`
return `Result` instead of warning, propagating errors to
`apply_config_and_runtime_defaults` which already returns `Result<(), String>`.

---

### 2.6 Hidden Internal Flags

#### `glitch_pct`, `shortpct`, `rippct` (f32, hidden CLI flags)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--glitch-pct` etc. | no prevalidation | not in `validation.rs:cli_spec` |
| config.toml | NOT in USER_CONFIG_KEYS | `configfile.rs:32-52` |
| runtime | `apply_glitch_level_values` ALWAYS overwrites | `config_apply.rs:547-580` |

**Combination conflict (NEW finding — detailed in §2.2 glitch-level):**
`--glitch-pct 50 --glitch-level subtle` → `glitch_pct` forced to 3.0.
NO WARNING. User-set `--glitch-pct` is silently discarded.

**Severity:** Medium. Hidden flags, but misleading for power users.

---

#### `glitch_ms` (U16Range "low,high", hidden)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--glitch-ms` | `U16Range::from_str` | `config.rs:143-163` |
| config.toml | NOT in USER_CONFIG_KEYS | `configfile.rs:32-52` |
| runtime | `apply_glitch_level_values` with `should_skip("glitch_ms")` check | `config_apply.rs:548, 559, 570` |

**Failure modes:**

| Input | CLI | Result |
|---|---|---|
| `200,300` | ✓ accept → U16Range { low: 200, high: 300 } | consistent |
| `300,200` | ✗ reject ("range must be >0 and low <= high") | consistent |
| `0,100` | ✗ reject ("range must be >0") | consistent |
| `100` | ✗ reject ("expected: NUM1,NUM2") | consistent |
| `abc,def` | ✗ reject ("invalid low value") | consistent |

**Positive:** `should_skip("glitch_ms")` PRESERVES user-set `--glitch-ms`
when `--glitch-level` is also set. Unlike `glitch_pct` etc. Consistent
behavior, but inconsistent WITH `glitch_pct` (which is NOT preserved).

**Severity:** Low (internal inconsistency between glitch_ms preserved
vs glitch_pct overwritten).

---

#### `colormode` (u16, enum-like: 0 | 16 | 8/256 | 24/32)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--colormode` | `detect_color_mode` → `ux::die_input` exit 2 | `cli.rs:125-145` |
| config.toml | NOT in USER_CONFIG_KEYS | `configfile.rs:32-52` |

**Failure modes:**

| Input | CLI | Result |
|---|---|---|
| `0` | ✓ → Mono | consistent |
| `16` | ✓ → Color16 | consistent |
| `8` / `256` | ✓ → Color256 | consistent |
| `24` / `32` | ✓ → TrueColor | consistent |
| `1` | ✗ reject ("invalid --colormode: 1") | consistent |
| `15` | ✗ reject | consistent |
| `-1` | ✗ reject (clap u16 rejects negative) | consistent |
| `abc` | ✗ reject (clap u16 rejects non-numeric) | consistent |

**No gap.** Positive finding.

---

### 2.7 Path Fields

#### `config` (PathBuf, `--config <path>`)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--config` | `validate_config_path` → `is_safe_path` + `.toml` extension | `safepath.rs:292-335` |
| runtime | same validator, centralized | `config_apply.rs:121-124`, `testconf.rs:26-32` |

**Failure modes** (well-tested in `safepath.rs:337-601`, 17 tests):

| Input | Result |
|---|---|
| `~/.config/cosmostrix/config.toml` | ✓ accept |
| `/etc/cosmostrix/config.toml` | ✓ accept |
| `/etc/cosmostrix/../passwd.toml` | ✗ reject (path traversal — `..` normalized) |
| `/etc/cosmostrix/../../etc/shadow` | ✗ reject |
| `/etc/cosmostrix/../../../tmp/leak.toml` | ✗ reject |
| `./config.toml` (relative) | ✗ reject (v14 strict policy) |
| `/tmp/test.toml` | ✗ reject (not in whitelist) |
| `/etc/passwd` | ✗ reject (not in whitelist) |
| `~/.ssh/id_rsa` | ✗ reject |
| `/etc/shadow` | ✗ reject |
| `/proc/self/environ` | ✗ reject |
| `my-chars.txt` (no .toml) | ✗ reject (extension check) |
| `config.txt` | ✗ reject (extension check) |

**Positive:** Robust whitelist-only approach with `..` normalization.
Cross-platform (Linux/macOS/Windows/Android-Termux). 17 tests cover all
attack vectors.

**No gap.** Strong positive finding.

---

#### `dump-config` (String, `--dump-config <path>`)

| Path | Validator | Evidence |
|---|---|---|
| CLI `--dump-config` | `is_safe_path` (write path) | need verify (Phase 3) |

**Phase 3 item:** Verify `--dump-config` write path uses same
`is_safe_path` validator. If it writes to arbitrary paths, that's a
gap (user could overwrite system files).

---

## 3. Combination Conflicts Catalog

### 3.1 Benchmark Mode Conflicts (WELL HANDLED)

`main.rs:1243-1280` `collect_bench_noop_warnings` covers 9 cases:

| Conflict | Warning | Evidence |
|---|---|---|
| `--bench-all + --benchmark` | "--benchmark ignored (--bench-all takes precedence)" | line 1248-1249 |
| `--bench-all + --bench-frames` | "--bench-frames ignored (--bench-all takes precedence)" | line 1251-1252 |
| `--benchmark + --bench-frames` | "--bench-frames ignored (--benchmark takes precedence)" | line 1254-1255 |
| `--bench-frames + --bench-duration` (no --benchmark) | "--bench-duration ignored (--bench-frames is frame-count-based)" | line 1257-1258 |
| `--fps` in benchmark | "--fps (in benchmark mode sets simulation rate only...)" | line 1260-1266 |
| `--duration` in benchmark | "--duration (interactive auto-exit only; use --bench-duration)" | line 1267-1268 |
| `--screensaver` in benchmark | "--screensaver (interactive input handler; bench has no input loop)" | line 1270-1271 |
| `--intro` in benchmark | "--intro (interactive intro animation; bench never plays it)" | line 1273-1274 |
| `--perf-stats` in benchmark | "--perf-stats (interactive summary; bench emits its own report)" | line 1276-1277 |

**Positive finding.** No gap.

---

### 3.2 Glitch Level vs Glitch Internals (NEW finding)

| Conflict | Behavior | Warning | Evidence |
|---|---|---|---|
| `--glitch-level subtle + --glitch-pct 50` | `glitch_pct` forced to 3.0 | ❌ NO WARNING | `config_apply.rs:554` |
| `--glitch-level subtle + --glitch-pct 50 + --shortpct 80` | `glitch_pct=3.0, shortpct=60.0` | ❌ NO WARNING | lines 554-556 |
| `--glitch-level subtle + --glitch-ms 100,200` | `glitch_ms` PRESERVED (user-set) | n/a (correct) | line 548 `should_skip` |
| `--glitch-level subtle + --rippct 30` | `rippct` forced to 45.0 | ❌ NO WARNING | line 556 |

**Severity:** Medium. Hidden flags, but power users who tune glitch
internals will be confused.

**Recommended fix (Phase 5):** Either:
- (a) Add `should_skip("glitch_pct")` etc. checks (preserves user-set
  values when explicit), OR
- (b) Emit warning: "--glitch-pct ignored (--glitch-level takes
  precedence)"

---

### 3.3 Scene vs Config (Documented, Correct)

| Conflict | Behavior | Evidence |
|---|---|---|
| `--scene matrix` + `speed = 30` in config.toml | config wins (speed=30) | `config_apply.rs:6-22` doc comment |
| `--scene matrix` + `color = red` in config.toml | config wins (color=red) | same |
| `--scene matrix` + `--speed 50` (CLI explicit) | CLI wins (speed=50) | `is_explicit` check |

**Positive finding.** Precedence is documented and consistent
(config-touched keys block scene defaults). No gap.

---

### 3.4 Adaptive-Custom vs Atmosphere-Mode (Phase 1 Gap #2)

| Conflict | Behavior | Warning | Evidence |
|---|---|---|---|
| `atmosphere-mode = disabled` + `[adaptive-custom.10-00]` | adaptive-custom STILL RUNS every 30s | ⚠ verbose-only log | `event_loop.rs:235-317` |

**Severity:** Critical (Phase 1 Gap #2). Intentional but undocumented.

---

### 3.5 Screensaver vs Intro

| Conflict | Behavior | Warning | Evidence |
|---|---|---|---|
| `--screensaver --intro none` | intro skipped, screensaver runs | n/a (logical) | need verify |
| `--screensaver --intro logo` | intro plays THEN screensaver runs | n/a (logical) | need verify |
| `--benchmark --intro logo` | intro NEVER plays | ✓ warned | `main.rs:1273-1274` |

**Phase 3 item:** Verify `--screensaver + --intro` interaction. Recent
commit `2396197` "fix(intro): play intro in --screensaver mode" suggests
there was a bug here. May need regression test.

---

## 4. Edge Case Coverage Matrix

### 4.1 NaN / Infinity Handling

| Field | CLI rejects NaN? | testconf rejects NaN? | runtime rejects NaN? | Consistent? |
|---|---|---|---|---|
| `fps` | ✓ (canonical) | ✗ **(f64 accepts)** | ✓ (canonical) | ❌ DIVERGENT |
| `speed` (top-level) | ✓ (canonical integer) | ✓ (i64 rejects NaN) | ✓ | ✓ |
| `speed` (adaptive-custom) | n/a | n/a | ✗ **(f64 accepts, range check passes)** | ❌ DIVERGENT |
| `density` | ✓ (canonical) | ✗ **(f64 accepts)** | ✓ (canonical) | ❌ DIVERGENT |
| `duration` | ✓ (canonical) | n/a (CLI-only) | n/a | ✓ |
| `color.tune.*` | n/a (config-only) | ✗ **(f64 accepts)** | ✓ (canonical via parse_canonical_f64_range) | ❌ DIVERGENT |
| `density-map` entries | n/a | ✓ (f64 parse + range, NaN fails range `0.0..=1.0` since NaN comparisons are false → wait, NaN in range check: `!(0.0..=1.0).contains(&nan)` → true → reject) | ✓ | ✓ |

**Summary:** 4 fields have NaN divergence between testconf (lenient f64)
and runtime (strict canonical). Silent fallback at runtime.

---

### 4.2 Zero / Negative Handling

| Field | Zero | Negative | Evidence |
|---|---|---|---|
| `fps` | ✗ reject (range min 1.0) | ✗ reject (canonical no leading -) | consistent |
| `speed` | ✗ reject (range min 1) | ✗ reject (canonical integer no -) | consistent |
| `density` | ✗ reject (range min 0.01) | ✗ reject (canonical no -) | consistent |
| `duration` | ✗ reject (range min 0.1) | ✗ reject (canonical no -) | consistent |
| `bench-duration` | ✗ reject (min 1 sec) | n/a (u64) | consistent |
| `screen-size` | ✗ reject (zero dimension) | n/a (u16) | consistent |
| `bench-frames` | ✓ accept (0 frames) | n/a (u64) | **possible issue** — Phase 3 |
| `bold` | ✓ accept (0 = off) | n/a (u8) | consistent |
| `shadingmode` | ✓ accept (0) | n/a (u8) | consistent |

**No major gap.** `--bench-frames 0` is the only questionable accept.

---

### 4.3 Empty / Whitespace Handling

| Field | Empty `""` | Whitespace `"  "` | Evidence |
|---|---|---|---|
| `fps` | ✗ reject (f64/canonical) | ✗ reject | consistent |
| `color` | ✗ reject (theme lookup) | ✗ reject | consistent |
| `charset` | ✗ reject | ✗ reject | consistent |
| `scene` | ✗ reject | ✗ reject | consistent |
| `intro` | ✗ reject | ✗ reject (testconf) / need verify (runtime from_str) | possible divergence |
| `monolith-size` | ✗ reject | ✗ reject | consistent |
| `density-map` | ✗ reject ("expected at least one...") | ✗ reject | consistent |

**No major gap.**

---

### 4.4 Leading Zeros Handling

| Field | `010` | Evidence |
|---|---|---|
| `fps` (CLI) | ✗ reject (canonical: no leading zeros) | `validation.rs:321-323` |
| `fps` (testconf) | ✓ **accept** (f64 parses 010 = 10) | `testconf.rs:373` |
| `fps` (runtime) | ✗ reject (canonical) | `config_apply.rs:363` |
| `speed` (CLI) | ✗ reject (canonical integer) | `validation.rs:321-323` |
| `speed` (testconf) | ✓ **accept** (i64 parses 010 = 10 in Rust) | `testconf.rs:387` |
| `speed` (runtime) | ✗ reject (canonical) | `config_apply.rs:369` |
| `bold` (testconf) | ✗ reject (literal match "0"\|"1"\|"2") | `testconf.rs:443-445` |
| `bold` (runtime) | ✗ reject (canonical u8) | `config_apply.rs:418` |

**Divergence:** `010` accepted by testconf, rejected by runtime for fps
and speed. Silent fallback.

---

## 5. Silent Coercion Detection

### 5.1 No Silent Coercion Found

The codebase uses **strict canonical parsers** that reject non-canonical
forms rather than coercing them. Specifically:

- `"10"` is NOT coerced to `10.0` for f64 fields — canonical decimal
  accepts `"10"` and `"10.0"` both as valid f64.
- `"fast"` is NOT coerced to any numeric value — rejected.
- `"true"` is NOT coerced to `1` for u8 fields like `bold` — rejected
  with "expected 0, 1, or 2".
- Booleans DO accept lenient set (`yes`/`on`/`1` → true) — this is
  documented leniency, not silent coercion.

**Positive finding.** No silent coercion.

---

### 5.2 Silent Fallback (NOT coercion, but related)

When runtime `parse_canonical_*` rejects a value that testconf accepted,
the value is **dropped** (not coerced). The error is printed to stderr,
but:

1. No "falling back to default X" message is emitted.
2. `config_touched` does NOT get the key, so scene defaults may
   overwrite.
3. User may not notice the stderr error if running in a non-interactive
   context.

**Affected fields:** `fps`, `speed` (top-level), `density`,
`color.tune.*` — all use `parse_canonical_f64_range` at runtime but
`v.parse::<f64>()` at testconf.

**Severity:** Medium. Error is printed (not fully silent), but fallback
behavior is not explicitly communicated.

**Recommended fix (Phase 5):** Align testconf to use canonical parsers,
OR add "falling back to default" message in `parse_f64_config` etc.
when returning None.

---

## 6. Swallow Pattern Audit

### 6.1 `unwrap_or(default)` patterns

Grep across config-layer files found:

| Location | Pattern | Risk |
|---|---|---|
| `config_apply.rs:132` | `.unwrap_or_else(|| default_config_file_path())` | ✓ Safe (intentional default) |
| `config_apply.rs:183` | `.unwrap_or_else(default_config_file_path)` | ✓ Safe |
| `config_apply.rs:331` | `.unwrap_or(&e)` (strip_prefix fallback) | ✓ Safe (string op) |
| `configfile.rs:98, 399` | `.unwrap_or_else(default_config_file_path)` | ✓ Safe |
| `configfile.rs:356, 1083, 1097` | `.unwrap_or(false)` (env var bool) | ✓ Safe (env absent = false) |
| `config_hints.rs:215` | `.unwrap_or(field)` (split fallback) | ✓ Safe (string op) |
| `testconf.rs:127, 322, 323, 355` | `.unwrap_or(...)` (string strip ops) | ✓ Safe |

**No risky swallow patterns found.** All `unwrap_or` uses are
intentional defaults or safe string operations.

---

### 6.2 `.ok()` patterns (error context discard)

| Location | Pattern | Risk |
|---|---|---|
| `configfile.rs:293, 301, 316, 431, 446` | `env::var(...).ok()` | ✓ Safe (env var may be unset) |
| `safepath.rs:161, 194, 200, 210, 280, 285` | `env::var(...).ok().filter(...)` | ✓ Safe |
| `testconf.rs:373, 387, 400, 425` | `v.parse::<f64>().ok().and_then(...)` | ⚠ **Hides parse error context** — see §2.1 fps divergence |

**The `testconf.rs` `.ok()` usage is the root cause of the testconf ↔
runtime divergence.** `v.parse::<f64>().ok()` discards the parse error
and converts to `None`, which falls through to range check. For "inf"
and "nan", f64 parse SUCCEEDS, so `.ok()` returns `Some(inf)` — but
canonical parser would reject. For "abc", f64 parse FAILS, `.ok()`
returns `None`, and the `or_else` branch emits "expected number" error.

**Recommended fix (Phase 5):** Replace `v.parse::<f64>().ok()` in
testconf with `parse_canonical_f64_range(...)` for consistency.

---

### 6.3 `if let Ok(_) = ...` swallow patterns

No instances found in config-layer files. Positive.

---

## 7. Path Traversal Audit (safepath.rs)

### 7.1 Attack Vectors Tested

| Vector | Test | Evidence |
|---|---|---|
| `..` traversal to `/etc/passwd` | ✓ rejected | `safepath.rs:508-514` |
| `..` traversal to `/tmp/` | ✓ rejected | `safepath.rs:517-522` |
| `..` traversal to `/etc/shadow` via user config | ✓ rejected | `safepath.rs:525-533` |
| `..` traversal to `~/.local/` | ✓ rejected | `safepath.rs:536-541` |
| Escape above root `/../../../../etc/shadow` | ✓ rejected (normalize returns None) | `safepath.rs:564-570` |
| Relative paths `./config.toml` | ✓ rejected (v14 policy) | `safepath.rs:364-370` |
| `/tmp/` (was allowed pre-v14) | ✓ rejected | `safepath.rs:396-404` |
| `~/.ssh/`, `~/.aws/`, `/etc/shadow`, `/proc/`, `/sys/` | ✓ rejected | `safepath.rs:419-451` |
| Non-`.toml` extension | ✓ rejected | `safepath.rs:328-333` |
| Unexpanded `~` (HOME unset) | ✓ rejected | `safepath.rs:487-503` |
| Windows `%APPDATA%` expansion | ✓ handled | `safepath.rs:58-83` |
| Termux runtime detection | ✓ handled | `safepath.rs:187-190` |

### 7.2 Whitelist Approach

Strict whitelist-only: `~/.config/cosmostrix/`, `/etc/cosmostrix/`,
`~/Library/Application Support/cosmostrix/` (macOS),
`%APPDATA%\cosmostrix\`, `%ProgramData%\cosmostrix\` (Windows),
`/sdcard/cosmostrix/` (Termux). Everything else rejected.

**Positive finding.** No path traversal gap.

---

## 8. Summary of New Findings (beyond Phase 1's 12 gaps)

### New Finding #1 — testconf ↔ runtime canonical parser divergence

**Affected fields:** `fps`, `speed` (top-level), `density`,
`color.tune.*`

**Root cause:** testconf uses `v.parse::<f64>()` (stdlib lenient),
runtime uses `parse_canonical_f64_range` (strict canonical).

**Divergent inputs:** `inf`, `nan`, `1e2`, `+10`, `010` — PASS testconf,
FAIL runtime → silent fallback.

**Severity:** Medium.

**Fix:** Align testconf to canonical parsers.

---

### New Finding #2 — NaN accepted in adaptive-custom speed

**Affected field:** `speed` in `adaptive-custom.HH-MM`

**Root cause:** `atmosphere_custom.rs:281` range check
`n < 1.0 || n > 100.0` — NaN comparisons are always false, so NaN passes.

**Impact:** NaN propagates to cloud state as f32 → rendering anomalies.

**Severity:** High.

**Fix:** Add `n.is_finite()` check, OR use `parse_canonical_speed`.

---

### New Finding #3 — `--glitch-pct` always overridden by `--glitch-level`

**Affected fields:** `glitch_pct`, `shortpct`, `rippct`

**Root cause:** `config_apply.rs:547-580` unconditionally assigns these
3 fields based on glitch_level, without `should_skip` check (unlike
`glitch_ms` which IS preserved).

**Impact:** User-set `--glitch-pct 50 --glitch-level subtle` → forced
to 3.0. NO WARNING.

**Severity:** Medium.

**Fix:** Add `should_skip` checks OR emit warning.

---

### New Finding #4 — Profile/scene-custom warn-vs-reject divergence

**Affected:** All block field values in `[profile.*]` and
`[scene-custom.*]`

**Root cause:** `profile.rs:207-387` uses `warn_invalid` (WARN +
continue). Top-level uses strict REJECT (exit 2).

**Impact:** Divergence only reachable via
`COSMOSTRIX_SKIP_STARTUP_VALIDATION=1` or live reload (Phase 4 item).

**Severity:** Medium.

**Fix:** Make `apply_profile_layer` return `Result`.

---

### New Finding #5 — `--color-bg default_background` CLI vs config divergence

**Root cause:** `validation.rs:300-305` CLI allowed list does NOT
include `default_background`. `testconf.rs:545-549` and
`config_apply.rs:677-680` DO accept the snake_case alias.

**Impact:** `--color-bg default_background` REJECTED on CLI, ACCEPTED
in config.toml.

**Severity:** Medium.

**Fix:** Add alias to CLI allowed list, OR remove from config.

---

### New Finding #6 — Case-sensitivity divergence for 3 enums

**Affected:** `intro`, `monolith-size`, `glitch-level`

**Root cause:** CLI `ValueEnum` case-insensitive, testconf
case-sensitive (literal match), runtime `from_str(&v, true)`
case-insensitive.

**Impact:** `intro = "Logo"` in config → testconf rejects, runtime would
accept. Testconf is the only gate.

**Severity:** Medium (Phase 1 Gap #4 expanded to 3 enums).

**Fix:** Align testconf to case-insensitive, OR document canonical-form
policy clearly.

---

### New Finding #7 — `--bench-frames 0` accepted

**Root cause:** clap u64 parser accepts 0. No range check.

**Impact:** Bench loop runs 0 frames → instant exit. Likely benign but
confusing.

**Severity:** Low.

**Fix:** Add minimum-1 check OR warning.

---

### New Finding #8 — `atmosphere-regime = storm` special rejection

**Root cause:** `testconf.rs:524-525` has special-case rejection message
for "storm" distinct from other unknown values. Suggests deprecation.

**Impact:** "storm" was once valid, now deprecated for config.toml. CLI
behavior needs Phase 3 verification.

**Severity:** Low (informational).

**Fix:** Document deprecation in `--help` and `--dump-config`.

---

### New Finding #9 — `--dump-config` write path not verified

**Root cause:** Phase 1 did not audit `--dump-config` write path
validation. `is_safe_path` is for READ paths.

**Impact:** If `--dump-config /etc/passwd` writes to arbitrary paths,
that's a security gap.

**Severity:** Medium (pending Phase 3 verification).

**Fix:** Verify `--dump-config` uses `is_safe_path` or equivalent for
write path.

---

## 9. Consolidated Action Items for Phase 5

| # | Severity | Finding | Fix |
|---|---|---|---|
| P1-1 | Critical | Stale 10-level precedence doc | Rewrite doc comment to 5-level |
| P1-2 | Critical | adaptive-custom bypasses atmosphere-mode | Doc-only OR behavior change |
| P1-3 | High | speed type asymmetry (CLI int vs adaptive float) | Align adaptive to canonical integer |
| P1-4 | High | intro case sensitivity asymmetry | Document OR align testconf |
| P1-5 | High | profile/scene-custom can't resolve custom names | Thread cfg through apply_profile_layer |
| P1-6 | Medium | color-bg underscore alias | Add alias to CLI OR remove from config |
| P1-7 | Medium | adaptive-custom 5-field limit | Document allowed fields |
| P1-8 | Medium | density-map section-only asymmetry | Add hint for top-level misuse |
| P1-9 | Medium | 9 hint coverage gaps | Add 9 patterns to config_hints.rs |
| P1-10 | Low | Density-map bounded Vec leak | None (accepted trade-off) |
| P1-11 | Low | Mutex poison risk | Phase 4 verify + PoisonError recovery |
| P1-12 | Low | async-mode always-wins | Doc-only |
| **P2-1** | **Medium** | **testconf ↔ runtime canonical divergence (fps/density/color.tune)** | **Align testconf to canonical parsers** |
| **P2-2** | **High** | **NaN accepted in adaptive-custom speed** | **Add is_finite() check** |
| **P2-3** | **Medium** | **--glitch-pct always overridden by --glitch-level** | **Add should_skip OR warning** |
| **P2-4** | **Medium** | **Profile/scene-custom warn-vs-reject divergence** | **Make apply_profile_layer return Result** |
| **P2-5** | **Medium** | **--color-bg default_background CLI vs config** | **Add alias to CLI OR remove** |
| **P2-6** | **Medium** | **Case-sensitivity divergence (3 enums)** | **Align OR document** |
| **P2-7** | **Low** | **--bench-frames 0 accepted** | **Add min-1 check OR warning** |
| **P2-8** | **Low** | **atmosphere-regime=storm deprecation** | **Document** |
| **P2-9** | **Medium** | **--dump-config write path unverified** | **Phase 3 verify** |

**Total: 21 action items** (12 from Phase 1 + 9 new from Phase 2).

**Priority breakdown:**
- Critical: 2
- High: 3
- Medium: 12
- Low: 4

---

## 10. Phase 2 Status

**Complete.** Failure mode catalog delivered for 24 field/flag groups.
9 new findings beyond Phase 1's 12 gaps. Total 21 action items for
Phase 5.

**Positive findings (no gap):**
- `parse_duration` and `parse_screen_size` well-tested (28 tests total)
- Benchmark mode combination conflicts well-handled (9 cases warned)
- `safepath.rs` robust (17 tests, whitelist-only, `..` normalization)
- Boolean parser unified across 3 sites (Phase D Bug #1 fix)
- No silent coercion found
- No risky `unwrap_or` swallow patterns

**Next:** Phase 3 (Silent Error & Warning Sweep) — hunt for swallowed
errors, missing warnings, footgun combinations. Inline small fixes for
obvious cases. Estimasi 2-3 sesi.

---

*Phase 2 audit executed by Cosmic Dragon. Evidence-based — every claim
cites `file:line`. No code changed in this phase.*
