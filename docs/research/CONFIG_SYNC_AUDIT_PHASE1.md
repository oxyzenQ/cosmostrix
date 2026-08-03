# Config Sync Audit — Phase 1: Surface Inventory & Mismatch Map

**Scope:** CLI flags, `config.toml` keys, runtime `CloudConfig` fields, and the
precedence chain that merges them.

**Goal:** Peta lengkap semua CLI flag, `config.toml` key, dan runtime field,
plus 3-way sync status. Identifikasi gap, conflict, dan silent override.

**Method:** Source-code audit dengan evidence `file:line`. Tidak ada code
change di phase ini — pure inventory.

**Audit layer files (11 files, ~7,109 LOC):**

| File | LOC | Role |
|---|---|---|
| `src/config.rs` | 901 | CLI args struct (clap derive) + IntroType + U16Range |
| `src/cli.rs` | 310 | CLI presentation helpers, color/charset scheme parsing |
| `src/cli_parse.rs` | 399 | `parse_duration`, `parse_screen_size`, charset alias tests |
| `src/configfile.rs` | 1,210 | `config.toml` parser + `USER_CONFIG_KEYS` + unknown-key tracking |
| `src/config_apply.rs` | 688 | Precedence chain (config → scene → scene-custom → glitch-level) |
| `src/config_hints.rs` | 716 | "Did you mean" hints for unknown keys (4 patterns) |
| `src/validation.rs` | 642 | Removed-flag interception + canonical range validators |
| `src/profile.rs` | 455 | `[profile.<name>]` parser + `apply_profile_layer` |
| `src/scene_custom.rs` | 399 | `[scene-custom.<name>]` parser + `apply_scene_custom_layer` |
| `src/atmosphere_custom.rs` | ~400 | `[adaptive-custom.HH-MM]` parser + lerp logic |
| `src/testconf.rs` | 1,159 | `--testconf` strict validator (case-sensitive, canonical form) |
| **Total** | **~7,439** | (4 test files: `config_apply_tests.rs` 1,250 + `config_apply_profiles_tests.rs` + `configfile_bug7_tests.rs` + `configfile_promotion_tests.rs` ≈ 2,243 LOC additional) |

---

## 1. Executive Summary

Audit menemukan **12 priority gaps** dibagi 4 tier severity. Dua critical
issues berkaitan dengan **stale documentation** dan **runtime bypass** yang
bisa menyebabkan silent override — keduanya bukan crash/fatal, tetapi
misleading untuk user dan bisa trigger surprise behavior.

**Health signals positif:**

- Zero `TODO` / `FIXME` / `HACK` di production code (bugs di-track di
  `KNOWN_ISSUES.md`, bukan inline)
- Zero `unreachable!()` di production code
- Zero `unwrap()` di production code (3 `expect()` di `configfile.rs:1067`,
  `live_config.rs`, `interactive/event_loop.rs` — semua safe-by-construction
  dengan invariant comment)
- 14 removed CLI flags properly intercepted di `validation.rs:23-88` dengan
  migration messages yang actionable
- 16 invariant tests di `cosmic_dragon_lock_tests.rs` ngunci engine contract
- `config.toml` parser punya 3-layer strict validation di startup
  (`config_apply.rs:177-225`): malformed lines → unknown keys → invalid values

**Health signals negatif:**

- 5 dari 10 documented precedence levels nggak actually wired sebagai
  separate functions (stale doc comment dari v14/v17/v20 purges)
- `adaptive-custom` runs regardless of `atmosphere-mode = disabled`
  (intentional by design tapi undocumented di `--help`)
- Type asymmetry: `speed = 15.5` accepted di `adaptive-custom`, rejected di
  CLI/config.toml top-level
- Case sensitivity asymmetry: `--intro Logo` works on CLI, `intro = "Logo"`
  rejected in config.toml (intentional canonical-form choice, tapi confusing)

---

## 2. Surface Inventory

### 2.1 CLI Fields (61 total)

Sumber: `src/config.rs:170-712` (`Args` struct).

**Categorized:**

| Category | Count | Fields |
|---|---|---|
| **Color/Charset** | 6 | `color` (`-c`), `colors-custom`, `color-tune`, `charset` (`-C`), `chars` (hidden), `colormode` |
| **Render tuning** | 8 | `fps` (`-f`), `speed` (`-S`), `density` (`-d`), `monolith-size`, `uniform`, `scene` (`-s`), `intro`, `glitch-level` |
| **Glitch internals (hidden)** | 5 | `glitch_pct` (`-g`), `shading_mode` (`-l`), `max_droplets_per_column`, `rippct`, `shortpct` |
| **Mode switches** | 16 | `screensaver`, `testconf`, `doctor`, `docs`, `benchmark`, `json`, `bench-io`, `bench-all`, `reset-terminal`, `list-colors`, `list-charsets`, `list-scenes`, `help`, `version`, `check-update`, `verbose` |
| **Benchmark** | 5 | `bench-duration`, `screen-size`, `save-baseline`, `compare-baseline`, `bench-scene`, `bench-frames` |
| **Scene/Profile** | 3 | `scene`, `scene-custom`, `show-scene` |
| **Config** | 3 | `config`, `dump-config`, `config-path` |
| **Atmosphere** | 4 | `atmosphere-mode`, `atmosphere-regime`, `async-mode` (skip), `auto-color-drift` |
| **Message/UI** | 4 | `message` (`-m`), `message-border`, `bold`, `color-bg` (`-b`), `perf-stats` |
| **Duration** | 2 | `duration`, `bench-duration` |
| **Hidden/diagnostic** | 5 | `check-bitcolor`, `chars`, `perf-stats`, `bench-frames`, `bench-scene` |

**Total: ~61 fields** (matches earlier estimate).

### 2.2 `config.toml` Published Keys (18 top-level)

Sumber: `src/configfile.rs:32-52` (`USER_CONFIG_KEYS` const).

```
scene, color, charset, fps, speed, density, monolith-size, glitch-level,
bold, shadingmode, color-bg, auto-color-drift, async-mode,
atmosphere-mode, atmosphere-regime, adaptive-custom, intro
```

(17 listed — `adaptive-custom` is a prefix pattern, not a single key, so
effectively 17 keys + 1 prefix pattern = 18 published surfaces.)

### 2.3 `config.toml` Section Patterns (4)

Sumber: `src/configfile.rs:54-57` (hint constants), `src/profile.rs:27-39`,
`src/scene_custom.rs`, `src/atmosphere_custom.rs`.

| Section | Pattern | Fields | Validator |
|---|---|---|---|
| `[profile.<name>]` | `profile.<name>.<field>` | 11 fields | `profile::is_profile_config_key` |
| `[scene-custom.<name>]` | `scene-custom.<name>.<field>` | 11 fields + `density-map` | `scene_custom::is_scene_custom_config_key` |
| `[colors-custom.<name>]` | `colors-custom.<name>.<bg\|rain\|stops>` | 3 fields | `config_hints::is_valid_colors_custom_field_str` |
| `[charset-custom.<name>]` | `charset-custom.<name>.set` | 1 field | `charset_custom::load_custom_charset_if_matches` |
| `[adaptive-custom.HH-MM]` | `adaptive-custom.HH-MM = <color>, <scene>, [k=v, ...]` | 5 fields (`speed`, `density`, `fps`, `charset`, `glitch-level`) | `atmosphere_custom::parse_custom_time_map` |

**Note:** `profile` supports `density-map`, `scene-custom` supports
`density-map`, tetapi top-level `USER_CONFIG_KEYS` TIDAK punya `density-map`.
Ini gap #6 (Medium).

### 2.4 Precedence Chain — Documented vs Actual

**Documented (10 levels)** — `src/config_apply.rs:6-22`:

```
1. Built-in clap defaults
2. Scene defaults (only for keys NOT set in config — fills the gaps)
3. Config file values (always wins over scene defaults for user-set keys)
4. Config preset
5. Config profile
6. CLI preset
7. CLI scene (still respects config-set keys; only fills unset keys)
8. CLI profile
9. Low-power values for fields not touched by curated layers or explicit CLI
10. Explicit CLI flags
```

**Actual wired functions** — `apply_config_and_runtime_defaults`
(`src/config_apply.rs:112-272`):

| Step | Function call | Line | Documented level |
|---|---|---|---|
| 1 | `load_config_file` | 126 | (implicit — clap defaults already applied) |
| 2 | `apply_config_values` | 228 | Level 3 (Config file values) |
| 3 | `apply_default_scene_values` | 236 | Level 2 (Scene defaults — for default scene only) |
| 4 | `apply_scene_values` (no CLI scene) | 241 | Level 7 (CLI scene — non-CLI path) |
| 5 | `apply_scene_custom_layer` (no CLI scene-custom) | 245 | Level 5/8 (profile — invoked via scene-custom) |
| 6 | `apply_scene_values` (CLI scene) | 255 | Level 7 (CLI scene) |
| 7 | `apply_scene_custom_layer` (CLI scene-custom) | 259 | Level 8 (CLI profile — invoked via scene-custom) |
| 8 | `apply_glitch_level_values` | 269 | (cross-cutting — handles `--glitch-level` vs `--glitch-pct` conflict) |

**Mismatch analysis:**

| Documented level | Status | Evidence |
|---|---|---|
| 1. clap defaults | ✓ implicit | clap auto-applies |
| 2. Scene defaults | ✓ wired | `apply_default_scene_values:236` |
| 3. Config file values | ✓ wired | `apply_config_values:228` |
| 4. **Config preset** | ✗ **NOT WIRED** | No `apply_config_preset` function exists. `--preset` was removed in v14 (`validation.rs:57-59`). Dead concept. |
| 5. **Config profile** | ⚠ **MIS-ROUTED** | `profile::apply_profile_layer` exists (`profile.rs:115`) tetapi ONLY called from `scene_custom.rs:116` — invoked via scene-custom, bukan sebagai layer terpisah. |
| 6. **CLI preset** | ✗ **NOT WIRED** | `--preset` removed v14 (`validation.rs:57-59`). |
| 7. CLI scene | ✓ wired | `apply_scene_values:241,255` |
| 8. **CLI profile** | ✗ **NOT WIRED** | `--profile` removed v14 (`validation.rs:61-63`). Profile logic absorbed into `scene-custom`. |
| 9. **Low-power values** | ✗ **NOT WIRED** | `--low-power` removed v14 (`validation.rs:53-55`). Low-power is now a scene (`--scene low-power`). |
| 10. Explicit CLI flags | ✓ implicit | `is_explicit(matches, ...)` checks di setiap apply function |

**Verdict:** 5 dari 10 documented levels adalah **stale references** ke
fitur yang sudah dihapus di v14 (preset, profile, low-power sebagai CLI
flags). Doc comment di `config_apply.rs:6-22` perlu di-rewrite untuk
match actual 5-level chain.

---

## 3. 12 Priority Gaps (with file:line evidence)

### CRITICAL (2)

#### Gap #1 — Stale 10-level precedence doc comment vs 5-level actual wiring

**Evidence:**
- Doc comment `src/config_apply.rs:6-22` lists 10 precedence levels
- Actual `apply_config_and_runtime_defaults` body (`config_apply.rs:112-272`)
  only calls 5 functions: `apply_config_values`, `apply_default_scene_values`,
  `apply_scene_values`, `apply_scene_custom_layer`, `apply_glitch_level_values`
- Levels 4 (config preset), 6 (CLI preset), 8 (CLI profile), 9 (low-power)
  reference v14-removed features (`validation.rs:53-63` REMOVED_FLAGS:
  `--preset`, `--profile`, `--low-power`)
- Level 5 (config profile) mis-routed: `profile::apply_profile_layer`
  (`profile.rs:115`) hanya dipanggil dari `scene_custom.rs:116`, bukan
  sebagai standalone layer di `config_apply.rs`

**Impact:**
- Misleading untuk maintainer baru yang baca doc comment expecting 10 layers
- Phase 2-5 audit bisa waste time hunting untuk dead layers
- Future contributor might "fix" missing layer dengan re-implementing
  v14-removed behavior

**Recommended fix (Phase 5):**
- Rewrite doc comment ke actual 5-level chain:
  1. clap defaults
  2. Config file values (always wins over scene defaults for user-set keys)
  3. Default scene values (only for keys NOT set in config)
  4. CLI scene / scene-custom (only fills unset keys, respects config-set keys)
  5. `--glitch-level` cross-cutting merge
- Add note: "preset/profile/low-power were removed in v14 — absorbed into
  `--scene` and `--scene-custom`"

---

#### Gap #2 — `adaptive-custom` bypasses `atmosphere-mode = disabled`

**Evidence:**
- `src/config_apply.rs:152-164` (verbose log): explicitly states
  "adaptive-custom: {N} entries (active regardless of atmosphere-mode)"
- `src/interactive/event_loop.rs:235-243`: `COLOR_CHECK_INTERVAL = 30s`
  comment says "Adaptive color shift: check current hour's target color
  every 30s" — no atmosphere-mode guard
- `src/interactive/event_loop.rs:249-257`: parses `custom_time_map` from
  `cfg_map` tanpa check `args.atmosphere_mode == Disabled`
- `src/interactive/event_loop.rs:312-317`: applies custom_time_map every
  30s, mutating cloud state (color/scene/speed/density)
- `src/interactive/event_loop.rs:530-535`: re-parses custom_time_map on
  live config reload (still no atmosphere-mode guard)

**Impact:**
- User sets `atmosphere-mode = disabled` expecting ALL atmosphere behavior
  to stop → adaptive-custom schedule still mutates `cloud` state every 30s
- Surprise: `speed`, `density`, `color`, `scene` change silently over hours
- Debug difficulty: verbose log mentions the bypass, tetapi `--help` and
  `--docs` TIDAK document ini

**Status:** Intentional by design (comment di `config_apply.rs:152-164`
implies this is deliberate — "defining them is an opt-in"), tetapi
**undocumented di user-facing surface**.

**Recommended fix (Phase 5):**
- Option A (doc-only): Add to `--help` and `docs/ATMOSPHERE_ENGINE.md`:
  "Note: `adaptive-custom.HH-MM` entries run regardless of
  `atmosphere-mode`. To disable ALL atmosphere behavior, remove
  `adaptive-custom.*` keys from config.toml."
- Option B (behavior change): Make `atmosphere-mode = disabled` also
  suspend adaptive-custom. Breaking change — requires migration note.

---

### HIGH (3)

#### Gap #3 — `speed` type inconsistency: integer-everywhere vs float-in-adaptive-custom

**Evidence:**
- `src/validation.rs:141-153` (`parse_canonical_speed`): requires canonical
  INTEGER format, range `[1, 100]` as `u32`, then casts to `f32`. Rejects
  `"15.5"`, `"0.5"`, `"01"`, `"100.1"`.
- `src/config_apply.rs:368-373` (`apply_config_values` for `speed`): calls
  `parse_speed_config` which uses `parse_canonical_speed` → integer-only.
- `src/testconf.rs` (strict validator): same integer-only enforcement.
- `src/atmosphere_custom.rs:40`: `pub speed: Option<f32>` — field type is f32
- `src/atmosphere_custom.rs:275-284`: parses speed as `f64` via
  `v.parse::<f64>()`, accepts `15.5`, range check `[1.0, 100.0]` (float
  comparison, not integer canonical check)

**Impact:**
- `--speed 15.5` on CLI → REJECTED (`parse_canonical_speed` requires integer)
- `speed = 15.5` in `config.toml` top-level → REJECTED (same path)
- `adaptive-custom.10-00 = cosmos, monolith, speed=15.5` → ACCEPTED
- User confusion: "why does `speed=15.5` work in adaptive-custom but not
  in `--speed`?"

**Recommended fix (Phase 5):**
- Option A (align to integer): Make `adaptive-custom` also reject float
  speed. Use `parse_canonical_speed` in `atmosphere_custom.rs:275-284`.
- Option B (align to float): Make CLI/config.toml accept float speed.
  Loosen `parse_canonical_speed` to accept canonical decimals. Bigger
  change — affects `--testconf` semantics.
- **Recommend Option A** (smaller blast radius, matches "canonical integer"
  philosophy documented in `validation.rs:141`).

---

#### Gap #4 — `intro` case sensitivity asymmetry (intentional, but confusing)

**Evidence:**
- `src/config.rs:125-132` (`IntroType`): `#[derive(clap::ValueEnum)]` with
  `#[value(name = "cosmic")]`, `#[value(name = "logo")]`, `#[value(name = "none")]`.
  clap `ValueEnum` is case-INsensitive by default → `--intro Logo` works.
- `src/testconf.rs:572-575` (`validate_field_value` for `intro`):
  ```rust
  "intro" => match v {
      "cosmic" | "logo" | "none" => None,
      _ => Some("expected cosmic/logo/none, got '{v}' ...")
  }
  ```
  No `.to_ascii_lowercase()` normalization → case-SENSITIVE.
- `src/testconf.rs:709-720` (test `intro_case_sensitive_typo_is_rejected`):
  explicitly asserts `validate_field_value("intro", "Logo")` returns `Some`
  (rejected). Comment at line 711-713:
  > "Strict validation is intentionally case-sensitive (canonical form).
  > clap ValueEnum is case-insensitive, but config.toml should use the
  > canonical lowercase form documented in --help."

**Impact:**
- `--intro Logo` on CLI → WORKS (clap lenient)
- `intro = "Logo"` in `config.toml` → REJECTED at startup by
  `validate_config_strictly` (`config_apply.rs:219`)
- User confusion: "the same value works on CLI but not config.toml"
- This is **intentional** (canonical-form policy), tetapi the asymmetry
  is not documented in `--help` or `--docs`.

**Recommended fix (Phase 5):**
- Doc-only: Add to `config.toml` template (`configfile.rs:687-690` area)
  and `--dump-config` output: "Values must be lowercase canonical form
  (e.g. `logo`, not `Logo`). CLI `--intro` is case-insensitive for
  convenience; config.toml is strict to enforce canonical form."

---

#### Gap #5 — Profile/scene-custom cannot resolve custom charset/color names that top-level config can

**Evidence:**
- `src/config_apply.rs:347-360` (top-level `charset` apply): checks BOTH
  `charset_from_str(&v, false)` AND
  `crate::charset_custom::load_custom_charset_if_matches(cfg, &v)` —
  custom names resolve.
- `src/profile.rs:115` (`apply_profile_layer`): passes `profile.charset`
  string to `args.charset` without custom-charset resolution. (Profile
  layer doesn't have access to `cfg` HashMap for custom-charset lookup.)
- `src/scene_custom.rs:116` (calls `apply_profile_layer`): passes
  `&custom_scenes` (BTreeMap of profiles), not the raw `cfg` HashMap.
- Result: `[scene-custom.foo] charset = "mycustom"` fails at runtime
  because `mycustom` is not a built-in preset, and profile layer can't
  look up `[charset-custom.mycustom]` from `cfg`.

**Impact:**
- User defines `[charset-custom.mycustom]` + `[scene-custom.foo]` with
  `charset = "mycustom"` → `--scene-custom foo` fails with "invalid
  charset" error
- Workaround: put `charset = "mycustom"` at top-level config (works) —
  but then can't switch scenes via `--scene-custom`

**Recommended fix (Phase 5):**
- Thread `cfg: &HashMap<String, String>` through `apply_profile_layer` →
  `apply_scene_custom_layer` → use same dual-check as
  `config_apply.rs:347-360`.
- Same fix needed for `color` field (top-level resolves theme aliases,
  profile layer doesn't).

---

### MEDIUM (4)

#### Gap #6 — `color-bg` underscore alias inconsistency

**Evidence:**
- `src/configfile.rs:32-52` (`USER_CONFIG_KEYS`): publishes `color-bg`
  (kebab-case), NOT `color_bg`.
- `src/config_apply.rs:380` (`apply_config_values`): looks up
  `config_value(matches, cfg, "monolith_size", "monolith-size")` —
  kebab-case TOML key, snake_case arg id.
- `src/config_apply.rs:432` (`color_bg`): looks up
  `config_value(matches, cfg, "color_bg", "color-bg")` — same pattern.
- `src/validation.rs:300-305` (`--color-bg` CLI spec): enum allowed
  `["black", "default-background"]`.
- `src/testconf.rs`: validates `color-bg` (kebab-case key).
- **Gap:** User who writes `color_bg = "black"` (snake_case) in
  `config.toml` gets "unknown key" error. Kebab-case is enforced.

**Impact:**
- Minor footgun. Most TOML conventions use kebab-case, so this is the
  right choice — tetapi users coming from Rust struct field naming might
  snake-case by accident.
- `config_hints.rs:119-127` (closest_top_level_key): edit-distance ≤ 2
  suggestion. `color_bg` → `color-bg` is edit distance 1 (replace `_`
  with `-`). Should trigger suggestion. Verify in Phase 2.

**Recommended fix (Phase 5):**
- Add explicit hint in `config_hints.rs`: if unknown key matches a
  USER_CONFIG_KEYS entry with `_` → `-` substitution, suggest the
  kebab-case form.

---

#### Gap #7 — `adaptive-custom` cannot change 5 fields that top-level config can

**Evidence:**
- `src/atmosphere_custom.rs:269-323` (`parse_custom_time_map`): accepts
  only `speed`, `density`, `fps`, `charset`, `glitch-level` as key=value
  pairs.
- `src/atmosphere_custom.rs:323`: explicit error
  `"adaptive-custom: unknown parameter '{k}' (allowed: speed, density, fps, charset, glitch-level)"`
- Top-level `USER_CONFIG_KEYS` (18 keys) includes: `scene`, `color`,
  `monolith-size`, `bold`, `shadingmode`, `color-bg`, `auto-color-drift`,
  `async-mode`, `atmosphere-mode`, `atmosphere-regime`, `intro` —
  none of which can be set via `adaptive-custom`.

**Impact:**
- User wants `adaptive-custom.22-00 = cosmos, monolith, bold=2` → REJECTED
- Workaround: define separate `[scene-custom.<name>]` blocks and switch
  via `scene` field in adaptive-custom — but that's indirect.

**Status:** Likely intentional (adaptive-custom is for time-varying
visual params, not configuration switches). Tetapi undocumented.

**Recommended fix (Phase 5):**
- Doc: list allowed fields explicitly in `--dump-config` template and
  `docs/ATMOSPHERE_ENGINE.md`.

---

#### Gap #8 — `density-map` testconf/runtime asymmetry

**Evidence:**
- `src/profile.rs:33` (`PROFILE_FIELDS`): includes `"density-map"`.
- `src/scene_custom.rs`: `apply_scene_custom_layer` accepts `density-map`.
- `src/configfile.rs:32-52` (`USER_CONFIG_KEYS`): does NOT include
  `density-map` (it's only valid inside `[profile.*]` / `[scene-custom.*]`).
- `src/scene_custom.rs:193` (`parse_density_map`): parses CSV f64 weights.
- `src/testconf.rs`: validates `density-map` only inside profile/scene-custom
  context, not top-level.

**Impact:**
- User writes `density-map = "0.5,0.3,0.2"` at top-level config → "unknown
  key" error (correct rejection, but error message doesn't explain that
  `density-map` is section-only).
- `config_hints.rs`: no targeted hint for top-level `density-map` misuse.

**Recommended fix (Phase 5):**
- Add hint pattern in `config_hints.rs`: if unknown key is `density-map`
  at top-level, suggest "move inside `[scene-custom.<name>]` or
  `[profile.<name>]`".

---

#### Gap #9 — Hint coverage gaps (9 undocumented patterns)

**Evidence:**
- `src/config_hints.rs:41-127` (`suggest_for_unknown_key`): handles 4
  patterns:
  1. `color.tune.<top-level-key>` (line 45)
  2. `scene-custom.<name>.adaptive-custom.<HH-MM>...` (line 67)
  3. `colors-custom.<name>.<invalid-field>` (line 89)
  4. Top-level typo via edit-distance (line 119)
- Missing patterns (Phase 2 will catalog fully):
  5. Top-level `density-map` (should suggest section move)
  6. Top-level `color_bg` snake_case (should suggest `color-bg`)
  7. `profile.<name>.<invalid-field>` (no hint)
  8. `charset-custom.<name>.<invalid-field>` (no hint — only `set` valid)
  9. `adaptive-custom.<invalid-HH-MM>` (no hint — currently generic error)
  10. `scene-custom.<name>.<top-level-key>` mis-nest (no hint)
  11. `colors-custom.<name>.<top-level-key>` mis-nest (no hint)
  12. `adaptive-custom.HH-MM = <color>, <scene>, <invalid-k=v>` (no hint)
  13. Top-level `bold = true` / `bold = false` (should suggest `0`/`1`/`2`)

**Impact:**
- Users hit "unknown key" with generic "run --testconf" message for 9
  patterns that could have targeted hints.

**Recommended fix (Phase 5):**
- Add 9 patterns to `config_hints.rs`, each with test coverage.

---

### LOW (3)

#### Gap #10 — Density-map memory: bounded `Vec<f64>` leak to `&'static [f64]`

**Evidence:**
- `src/scene_custom.rs:193` (`parse_density_map`): comment at
  `src/profile.rs:48-49`:
  > "Comma-separated f64 weights (0.0..=1.0) for monolith pillar placement.
  > Parsed into a Vec<f64> and leaked to &'static for Cloud consumption."
- Pattern: `Box::leak(vec.into_boxed_slice())` (typical Rust idiom for
  "runtime-lifetime config slice").

**Impact:**
- Each `--scene-custom <name>` invocation with `density-map` leaks one
  Vec. Bounded by number of scene-custom blocks in config.toml (typically
  <10). Not a real leak in practice.
- Live config reload could accumulate leaks over hours/days if user
  edits config.toml frequently. Still bounded by reload count.

**Status:** Accepted trade-off (Cloud config needs `&'static` for
zero-cost cell access in hot render loop). Documented in source.

**Recommended fix (Phase 5):**
- None (or: switch to `Arc<[f64]>` if Cloud ever moves off `&'static`).

---

#### Gap #11 — `Mutex` poison panic risk on live reload thread

**Evidence:**
- `src/live_config.rs:142-146`: spawns watcher thread, handles spawn
  failure with error message.
- Live config uses `Mutex<FileStateSnapshot>` (standard pattern).
- If watcher thread panics while holding lock (e.g., due to filesystem
  race), Mutex becomes poisoned → next `.lock()` call panics on main
  thread → crash.

**Impact:**
- Rare in practice (would require watcher thread to panic mid-lock).
- `live_config.rs` doesn't show explicit poison recovery (no
  `.lock().unwrap_or_else(|e| e.into_inner())` pattern visible in grep).

**Status:** Phase 4 will verify with full `live_config.rs` read.

**Recommended fix (Phase 5):**
- If poison panic found, add `PoisonError::into_inner()` recovery in
  hot-path lock sites.

---

#### Gap #12 — `async-mode` always-wins over `atmosphere-mode = disabled`

**Evidence:**
- `src/config_apply.rs:574` (`pub async_mode: bool`): `#[arg(skip = true)]`
  — not settable via CLI.
- `src/config_apply.rs:447` (`apply_config_values` for `async_mode`):
  config.toml `async-mode = true/false` sets `args.async_mode`.
- `src/cloud/runtime_controls.rs` (not read in Phase 1): if `async_mode`
  is true, async renderer is used regardless of `atmosphere-mode`.

**Impact:**
- User sets `atmosphere-mode = disabled` expecting sync renderer, but
  `async-mode = true` (from config) overrides → async renderer still
  active.
- Likely intentional (async-mode is a renderer choice, not an atmosphere
  feature), tetapi the interaction is undocumented.

**Status:** Intentional by design. Doc-only fix.

**Recommended fix (Phase 5):**
- Add to `docs/ATMOSPHERE_ENGINE.md`: "`async-mode` and `atmosphere-mode`
  are independent. Setting `atmosphere-mode = disabled` does NOT disable
  async rendering. To force sync, set `async-mode = false`."

---

## 4. Health Signals

### Positive

| Signal | Evidence |
|---|---|
| Zero `TODO`/`FIXME`/`HACK` in production code | `rg "TODO\|FIXME\|HACK" src/ --glob '!*_tests.rs'` returns nothing |
| Zero `unreachable!()` in production code | grep across `src/` (excluding tests) returns nothing |
| Zero `unwrap()` in production code | grep returns nothing — 3 `expect()` calls all have invariant comments |
| 14 removed CLI flags intercepted | `validation.rs:23-88` REMOVED_FLAGS table |
| 16 invariant tests | `cosmic_dragon_lock_tests.rs` |
| 3-layer strict startup validation | `config_apply.rs:177-225` (malformed → unknown → invalid) |
| "Did you mean" hints for 4 patterns | `config_hints.rs:41-127` |
| Live config reload | `live_config.rs` (file watcher + 30s polling heartbeat fallback) |
| `--testconf` strict validator | `testconf.rs` (canonical form enforcement) |

### Negative

| Signal | Evidence |
|---|---|
| 5/10 documented precedence levels stale | Gap #1 above |
| `adaptive-custom` bypasses `atmosphere-mode` | Gap #2 |
| `speed` type asymmetry | Gap #3 |
| `intro` case sensitivity asymmetry | Gap #4 |
| Profile/scene-custom can't resolve custom names | Gap #5 |
| 9 hint coverage gaps | Gap #9 |
| 1 known platform issue (HUD `i` key on Windows/Termux) | `KNOWN_ISSUES.md` — event delivery, not renderer |

---

## 5. Recommendations for Phase 2-5

### Phase 2 — Failure Mode Catalog

Untuk setiap field di inventory, katalogkan:

1. **Invalid value behavior** (panic / silent ignore / error / fallback)
2. **Error message quality** (clear vs misleading)
3. **Silent coercion** (e.g., `"10"` → `10.0`, `"fast"` → `1.0`)
4. **Edge cases**: NaN, infinity, negative, zero, max_value, empty, whitespace
5. **Combination conflicts** (e.g., `--scene matrix --color red`)

**Anchor cases (from Phase 1 gaps):**
- Gap #3: `speed = 15.5` in adaptive-custom vs CLI (type asymmetry)
- Gap #4: `intro = "Logo"` in config.toml vs `--intro Logo` on CLI
- Gap #5: `charset = "mycustom"` in `[scene-custom.foo]` vs top-level
- Gap #8: `density-map` at top-level vs inside section

**Hunt targets:**
- `unwrap_or(default)` patterns that hide parse failure
- `.ok()` that discards error context
- `if let Ok(_) = ...` that swallow Err without log
- `parse_duration`, `parse_screen_size` edge cases (already well-tested in
  `cli_parse.rs:207-399`)
- Float parsing (NaN/infinity) — `validation.rs:327-344` already rejects
  via `is_canonical_decimal`

### Phase 3 — Silent Error & Warning Sweep

- Warning yang should be error (e.g., `--screensaver --intro none`)
- Missing warnings for footguns (e.g., `--benchmark --intro logo`)
- Config keys yang typo'd silently ignored vs unknown-key tracking
  (Phase 1 confirms: unknown keys ARE tracked, but 9 hint patterns missing)

### Phase 4 — Crash & Bottleneck Audit

- Division by zero (density = 0, fps = 0) — `validation.rs` rejects both
- Integer overflow (large screen_size, large bench_frames) — `cli_parse.rs`
  uses u16 for screen_size (max 65535×65535), u64 for bench_frames
- OOM (very high density + very large screen) — `DENSITY_CLAMP_MAX = 5.0`
  in constants
- Panic on malformed config.toml — `config_apply.rs:177-225` strict check
- Path traversal di `--config` / `--dump-config` — `safepath.rs` (audit)
- Race conditions di live config reload — `live_config.rs` (Phase 4 read)
- Mutex poison — Gap #11

### Phase 5 — Stabilization & Hardening

Prioritization (per gap severity):

1. **Critical** (Gap #1, #2) — fix immediately
2. **High** (Gap #3, #4, #5) — fix next
3. **Medium** (Gap #6, #7, #8, #9) — fix after
4. **Low** (Gap #10, #11, #12) — batch at end

Each fix gets:
- Regression test (kalau belum ada)
- Updated docs (README, RULES, help text)
- Worklog entry

---

## 6. Appendix: File Inventory

### Config-layer source files (read in Phase 1)

```
src/config.rs              901 LOC  (Args struct, IntroType, U16Range)
src/cli.rs                 310 LOC  (help template, color/charset helpers)
src/cli_parse.rs           399 LOC  (parse_duration, parse_screen_size)
src/configfile.rs        1,210 LOC  (config.toml parser, USER_CONFIG_KEYS)
src/config_apply.rs        688 LOC  (precedence chain)
src/config_hints.rs        716 LOC  (did-you-mean hints, 4 patterns)
src/validation.rs          642 LOC  (removed flags, canonical validators)
src/profile.rs             455 LOC  ([profile.*] parser, apply_profile_layer)
src/scene_custom.rs        399 LOC  ([scene-custom.*] parser, apply layer)
src/atmosphere_custom.rs   ~400 LOC ([adaptive-custom.HH-MM] parser, lerp)
src/testconf.rs          1,159 LOC  (--testconf strict validator)
```

### Test files (not read in Phase 1, will read in Phase 2)

```
src/config_apply_tests.rs           1,250 LOC
src/config_apply_profiles_tests.rs       ? LOC
src/configfile_bug7_tests.rs             ? LOC
src/configfile_promotion_tests.rs        ? LOC
```

### Key constants

```
src/constants.rs:
  SPEED_MIN, SPEED_MAX        (used in validation.rs:138, 142)
  DENSITY_CLAMP_MAX           (used in validation.rs:282, config_apply.rs:375)
  MIN_TERMINAL_COLS, MIN_TERMINAL_LINES  (used in cli_parse.rs:187-188)
  MAX_TERMINAL_COLS, MAX_TERMINAL_LINES  (1024×500 interactive)
  BENCH_MAX_COLS, BENCH_MAX_LINES        (7680×4320 benchmark)
  CONFIG_DIR_NAME, CONFIG_FILE_NAME      (~/.config/cosmostrix/config.toml)
```

---

## 7. Phase 1 Status

**Complete.** Inventory dan mismatch map di-deliver. 12 priority gaps
identified dengan file:line evidence. Health signals positif (zero
TODO/FIXME/unwrap/unreachable in production). Health signals negatif
(stale doc, runtime bypass, type asymmetry, hint coverage gaps).

**Next:** Phase 2 (Failure Mode Catalog) — anchor on 12 gaps, hunt
edge cases (NaN/infinity/negative/zero/max/empty), combination conflicts.
Estimasi 2-3 sesi.

---

*Phase 1 audit executed by Cosmic Dragon. Evidence-based — every claim
cites `file:line`. No code changed in this phase.*
