# CLI Flags Audit — Dead Weight Sweep (Task `flags-audit-3`)

**Agent**: flags-audit-3 (general-purpose sub-agent)
**Scope**: every `#[arg(long = "...")]` entry in `src/config.rs`, with deep-dive on the 13 suspected-dead flags listed in the task brief.
**Mode**: read-only audit. No source modified.
**Prior context**: builds on `FLAGS_AUDIT_bench-frames_chars_bold.md` (which led to `--chars` removal + `--bench-frames`/`--bold` keep + bug fixes).

---

## Summary Table

| # | Flag | Line in `config.rs` | Hidden? | Consumed? | Config equiv? | Verdict | 1-line rationale |
|---|------|---------------------|---------|-----------|---------------|---------|------------------|
| 1 | `--colors-custom` | 197–204 | no | ✅ `main.rs:773`, `interactive/event_loop.rs:337` | `[colors-custom.<name>]` block | **KEEP** | CLI gives the *name*, config gives the *palette data* — same dual-path pattern as `--charset`/`--scene-custom`. Not redundant. |
| 2 | `--color-tune` | 206–212 | no | ✅ `main.rs:659–666` (CLI string parsed; falls back to `[color.tune]`) | `[color.tune]` block | **KEEP** | CLI takes precedence over config (verified `main.rs:659`); both paths feed the same `ColorTune` struct. |
| 3 | `--auto-color-drift` | 627–632 | yes | ✅ `main.rs:879–882, 926, 1014`, `config_apply.rs:471`, `cloud/rain.rs:843` | `auto-color-drift = true` | **KEEP** | Deeply integrated (live-reload CLI-intent guard at `app.rs:157–163`, opt-in gating, doctor.rs:169, SYSTEM_FEELING.md). Hidden is correct (advanced). |
| 4 | `--colormode` | 692–697 | yes | ✅ `cli.rs:126`, `main.rs:597, 884`, `doctor.rs:291` | (none — auto-detection only) | **KEEP** | Explicit override of auto-detection; required when `COLORTERM`/`TERM` lie. Referenced in `doctor.rs:440`, `bench_report.rs:822` advice. |
| 5 | `--message-border` | 666–671 | yes | ✅ `main.rs:930, 989`, `app.rs:247`, `cloud/mod.rs:424–425, 659` | (none) | **KEEP** | Does NOT duplicate `--message`. The `-mb` shorthand is real — `main.rs:343–367` pre-expands `-mb "text"` → `--message-border --message "text"` before clap parses. |
| 6 | `--atmosphere-mode` | 708–713 | yes | ✅ `main.rs:799`, `config_apply.rs:486–490`, `profile.rs:326–330` | `atmosphere-mode = "controlled-live"` | **KEEP** (help text bug) | Help string says "config only" but the flag IS CLI-parseable and consumed. Misleading label, not dead. |
| 7 | `--atmosphere-regime` | 715–720 | yes | ✅ `main.rs:801`, `config_apply.rs:492–496`, `profile.rs:336–340` | `atmosphere-regime = "pulse"` | **KEEP** (help text bug) | Same as #6 — help says "config only" but CLI works. |
| 8 | `--duration` | 601–606 | yes | ✅ `main.rs:644, 931, 991, 1285`, `interactive/event_loop.rs:143–147` | (none) | **KEEP** | Does NOT duplicate `--bench-duration`. `--duration N` = interactive auto-exit after N s; `--bench-duration` = how long `--benchmark` runs. Distinct code paths, distinct help text. |
| 9 | `--perf-stats` | 608–613 | yes | ✅ `main.rs:1007, 1294`, `interactive/event_loop.rs:89, 515, 1108, 1215, 1333`, `cloud/phosphor.rs:680` | (none) | **KEEP** | Does NOT duplicate `--verbose` (startup info) or `--benchmark` (headless report). Enables per-component timing + interactive exit summary (FPS, work_ms, pressure, encoding stats). Documented in `benchmark/README.md:192`, `RENDER_ENGINE.md:259,466,478`. |
| 10 | `--check-bitcolor` | 699–704 | yes | ✅ `main.rs:577–602` | (none) | **NEEDS_OWNER_DECISION** | Subset of `--doctor` output (`doctor.rs:264–293` already prints `color_auto_detected` + `color_forced` + COLORTERM + TERM). Only 2 doc references; no tests/scripts use it. Either remove (route users to `--doctor`) or keep as a parseable one-liner for scripts. |
| 11 | `--reset-terminal` | 489–495 | no | ✅ `main.rs:387–390` (calls `reset_terminal_emergency()`), `terminal.rs:1082` | (none) | **KEEP** | One-shot utility, but heavily documented (`TERMINAL_KILL_CLEANUP.md`, `TERMINAL_LIFECYCLE_MATRIX.md`, `COSMIC_DRAGON_ARCHITECTURE.md`, README:130,147,149,377,492,495, CHANGELOG). Bundling into cosmostrix binary is intentional — "kill -9 broke my terminal, run cosmostrix --reset-terminal" is the documented user model. |
| 12 | `--uniform` | 282–288 | no | ✅ `main.rs:832` (`args.async_mode && !args.uniform`) | (none — `async-mode = false` in config is the config-side equivalent) | **KEEP** | Disables variable column pacing. The `--async` flag was removed in v17 (always on); `--uniform` is the only CLI way to opt out. Documented in config.rs:578–579 comment. |
| 13 | `--screensaver` | 290–297 | no | ✅ `main.rs:925, 1008, 1288`, `interactive/event_loop.rs:808` (gates input to "only q exits") | (none) | **KEEP** | Real mode with distinct behavior — input handler ignores all keys except `q` (and a small allowlist for HUD/intro toggles). |
| 14 | `--intro` | 299–308 | no | ✅ `main.rs:889, 1009, 1291`, `interactive/event_loop.rs:107–108` (plays `intro::run_intro` before rain) | `intro = "logo"` in config | **KEEP** | Real feature (cosmic / logo / none). Verbose output, CloudConfig, and event_loop all consume it. |

**Verdict tally**: 13 KEEP, 1 NEEDS_OWNER_DECISION (`--check-bitcolor`), 0 REMOVE.

---

## Detailed Per-Flag Analysis

### 1. `--colors-custom` (config.rs:197–204)

**Help text**: "Load a user-defined custom color palette from config (see --list-colors)"

**What it claims to do**: Same dual-path pattern as `--charset` — the CLI accepts a *name*, the actual palette data is loaded from `[colors-custom.<name>]` in `config.toml`.

**Consumed at**:
- `main.rs:773` — `if let Some(ref name) = args.colors_custom { colors_custom::load_custom_palette(&cfg_map, name) ... }`
- `interactive/event_loop.rs:337` — runtime `c/C` theme-cycle reads custom palettes from config
- `profile.rs:225` — profile `color = <name>` field accepts a custom-palette name
- `colors_custom.rs:162` — `is_colors_custom_name()` for validation

**Config equivalent**: `[colors-custom.<name>]` block in config.toml — but this is the *data source*, not a replacement. The CLI flag is the only way to *select* a named palette at startup.

**Verdict**: **KEEP**. Direct analog of `--charset` (which was kept in audit-1) and `--scene-custom`. Not dead weight.

---

### 2. `--color-tune` (config.rs:206–212)

**Help text**: "Tune theme colors (keys: sat=, bright=, head=, body=, tail=; range 0.0-3.0)"

**What it claims to do**: Inline CLI tuner that adjusts saturation/brightness/per-segment multipliers on the active theme palette.

**Consumed at**:
- `main.rs:659–666` — `match args.color_tune.as_deref() { Some(s) => parse_color_tune(s), None => color_tune_from_config(&cfg_map) }`
- `app.rs:210–213, 244, 289` — `apply_tune_to_palette()` runs on every cloud rebuild
- `cloud/runtime_controls.rs:46–49` — re-applied after palette swap (Bug #5 fix)
- `live_config.rs:839–868` — live-reload watches `[color.tune]` and re-applies
- `bench.rs:130–136` — emits `color_tune_summary` in JSON/text bench reports

**Config equivalent**: `[color.tune]` block (`color.tune.saturation`, `color.tune.brightness`, `color.tune.head`, `color.tune.body`, `color.tune.tail`).

**Verdict**: **KEEP**. CLI takes precedence (`main.rs:659` Some-arm wins). The v30 simplify comment at config.rs:214–217 explicitly calls out this exact pattern as the *intended* design — the v17 `--brightness`/`--saturation` ghost skip-fields were removed in favor of this single flag. Removing `--color-tune` would re-introduce the very fragmentation that the v30 simplify step cleaned up.

---

### 3. `--auto-color-drift` (config.rs:627–632, hidden)

**Help text**: "Enable autonomous palette drift (default: off)"

**What it claims to do**: Opt-in palette-scheme drift (3% chance per 3 s tick, 30 s cooldown). Climate drift (luminance/saturation/hue) is always-on regardless.

**Consumed at**:
- `main.rs:879–882` — tracks CLI-explicit intent (Phase D Bug #10 fix) so live-reload doesn't silently override
- `main.rs:926, 1014` — passed into CloudConfig and then into the Cloud
- `config_apply.rs:471–475` — config-side loader (snake_case + kebab-case keys)
- `cloud/rain.rs:843` — `if self.auto_color_drift && !self.custom_palette_active { ... }` (the actual drift trigger)
- `cloud/ecosystem.rs:270, 427, 556` — ecosystem consults the drift flag
- `live_config.rs:821–828` — live-reload respects CLI intent
- `doctor.rs:169–179` — surfaces drift state in `--doctor`
- `bench.rs:161, 173, 201, 346, 1044` — force-disabled in all bench modes (keeps p99/max clean)
- `bench_report.rs:136, 365` — exposed in JSON output for CI verification
- `bench_json.rs:93` — emitted to JSON

**Config equivalent**: `auto-color-drift = true` in config.toml (configfile.rs:44 lists it as a known key; configfile.rs:599 dumps it in the example).

**Verdict**: **KEEP**. Pervasive integration; the CLI-vs-config intent tracking (Phase D Bug #10) is a real, tested behavior. Hidden status is correct — it's an advanced opt-in.

---

### 4. `--colormode` (config.rs:692–697, hidden)

**Help text**: "Force color mode (allowed: 0,16,8/256,24/32). Default: 24-bit if supported (COLORTERM), else 8-bit (TERM=...256color), else 16-color"

**What it claims to do**: Force-override the auto-detected color depth.

**Consumed at**:
- `cli.rs:126–138` — `validate_color_mode()` parses `0/16/8/256/24/32`
- `main.rs:597` — feeds `detect_color_mode(&args)`
- `doctor.rs:291–293` — only prints `color_forced` when `--colormode` is set
- `doctor.rs:440` — advice text suggests `--colormode 256` for limited-color terminals
- `bench_report.rs:822` — same advice in bench-compare report
- `help_detail.rs:329` — documented under ADVANCED APPEARANCE

**Config equivalent**: None — auto-detection is the default; `--colormode` is the explicit override. (No `colormode = ...` config key exists.)

**Verdict**: **KEEP**. Auto-detection is the default, but terminals that lie about COLORTERM/TERM need this override. doctor.rs and bench_report.rs both recommend it. Hidden is correct (advanced diagnostic override).

---

### 5. `--message-border` (config.rs:666–671, hidden)

**Help text**: "Draw message box with border (use with --message; shorthand: -mb)"

**What it claims to do**: Enable border rendering on the `--message` overlay box.

**Consumed at**:
- `main.rs:930` — passed into `verbose::print_verbose`
- `main.rs:989` — passed into `CloudConfig.message_border`
- `app.rs:247` — `cloud.set_message_border(self.message_border)`
- `cloud/mod.rs:424–425` — `set_message_border()` setter
- `cloud/mod.rs:659` — `let border: u16 = if self.message_border { 1 } else { 0 };` (rendering branch)
- `verbose.rs:89, 189` — verbose output shows border state

**`-mb` shorthand verification**: The shorthand is REAL. `main.rs:343–367` pre-expands argv: `["-mb", "hello"]` → `["--message-border", "--message", "hello"]` *before* clap sees it. Same for `-mb=hello` form. (clap cannot natively combine `-m` (value-taking) with `-b` (existing short for `--bold`), so the pre-expansion is necessary.)

**Config equivalent**: None.

**Verdict**: **KEEP**. Does NOT duplicate `--message` — it's a complementary modifier. The `-mb` shorthand is real and tested by the pre-expansion logic.

---

### 6. `--atmosphere-mode` (config.rs:708–713, hidden)

**Help text**: "Atmosphere mode (config only: disabled, controlled-live)"

**What it claims to do**: Selects whether the adaptive atmosphere engine is active.

**Consumed at**:
- `main.rs:799` — `resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref())`
- `main.rs:808` — `if atmosphere_mode.allows_modulation() { ... }` gates the entire modulation pipeline
- `config_apply.rs:486–490` — config-side loader (validates values)
- `profile.rs:326–330` — profile blocks can set `atmosphere-mode`
- 20+ tests in `config_apply_tests.rs`, `atmosphere_expansion_tests.rs`, `config_apply_profiles_tests.rs`

**Config equivalent**: `atmosphere-mode = "controlled-live"` in config.toml. Both paths converge on the same field.

**Verdict**: **KEEP** (but the inline help string is misleading). The flag IS CLI-parseable and IS consumed — the "(config only: ...)" parenthetical is a UX hint (recommending config as the primary path), not a hard restriction. `help_detail.rs:360–367` documents the CLI flag honestly under ADVANCED. Suggest owner update the inline `help =` string to drop the "config only" wording (e.g., `"Atmosphere mode (disabled, controlled-live). Config: atmosphere-mode = \"controlled-live\""` to match the convention used by `--auto-color-drift`).

---

### 7. `--atmosphere-regime` (config.rs:715–720, hidden)

**Help text**: "Atmosphere regime (config only: calm, pulse, signal, compression, void, monolith-pressure, adaptive)"

**What it claims to do**: Selects the modulation profile applied when `atmosphere-mode = controlled-live`.

**Consumed at**: same call sites as `--atmosphere-mode` (`main.rs:801`, `config_apply.rs:492–496`, `profile.rs:336–340`) plus 30+ tests.

**Config equivalent**: `atmosphere-regime = "pulse"` in config.toml.

**Verdict**: **KEEP** (same help-text bug as #6). Same fix suggested: drop "config only" from the inline help.

---

### 8. `--duration` (config.rs:601–606, hidden)

**Help text**: "Stop after N seconds (min 0.1 max 86400; <=0 disables)"

**What it claims to do**: Interactive auto-exit timer.

**Consumed at**:
- `main.rs:644–653` — parsed as `duration_s` (bare float only)
- `main.rs:931, 991` — passed into CloudConfig as both `duration` (raw Option<f64>) and `duration_s` (validated f64)
- `interactive/event_loop.rs:143–147` — `let end_time = cfg.duration_s.and_then(|s| { ... });` — used as the auto-exit deadline
- `main.rs:1285–1286` — warns "interactive auto-exit only; use --bench-duration" when combined with `--benchmark`

**Config equivalent**: None.

**Does it duplicate `--bench-duration`?** **No.** `--bench-duration` (config.rs:411–417) accepts compound format (`5s, 30m, 1h30m`) and is consumed only by `--benchmark`/`--bench-all`/`--bench-frames` to set how long the bench runs. `--duration` is bare-float-only and consumed by the *interactive* event loop to auto-exit. Different code paths, different value parsers, different consumers, different help text — `main.rs:1285` even warns when they're conflated.

**Verdict**: **KEEP**. Distinct from `--bench-duration` (verified by warn matrix at `main.rs:1285–1286`).

---

### 9. `--perf-stats` (config.rs:608–613, hidden)

**Help text**: "Print performance statistics summary on exit"

**What it claims to do**: Enables per-component timing collection + prints a perf summary when the interactive loop exits.

**Consumed at**:
- `main.rs:1007` — stored in CloudConfig
- `main.rs:1294–1296` — warns "interactive summary; bench emits its own report" when combined with `--benchmark`
- `interactive/event_loop.rs:89` — `cloud.set_component_timing(cfg.perf_stats)` — turns on per-component `Instant::now()` calls
- `interactive/event_loop.rs:515` — live-reload re-applies
- `interactive/event_loop.rs:1108–1124` — accumulates per-frame work_ms / pressure / dirty / utilization / overshoot
- `interactive/event_loop.rs:1215` — feeds endurance_health score
- `interactive/event_loop.rs:1333–1354+` — prints the final summary (FPS, work_ms, pressure class, encoding stats, RSS)
- `cloud/phosphor.rs:680, 696` — gates the legacy sweep (kept for backwards compat)
- `color_cache.rs:62, 157, 205` — SGR cache hit/miss counters feed `--perf-stats` reporting
- `terminal.rs:188, 309, 532` — encoding stats counters feed `--perf-stats`
- `bench_helpers.rs:80` — backpressure formatter used in `--perf-stats` exit report
- `cloud/rain.rs:757` — skips `Instant::now()` when perf-stats is off (zero-cost)
- `interactive/activity.rs:82` — zero-cost when off
- `report.rs:7` — module docstring names `--perf-stats`, `--benchmark` as the two diagnostics

**Does it duplicate `--verbose` or `--benchmark`?** **No.**
- `--verbose` (`main.rs:884–940`, `verbose.rs`) prints *startup* info to stderr (config path, resolved values, terminal detection) *before* rain starts. No per-frame timing.
- `--benchmark` (`bench.rs`, `main.rs:1243+`) runs a *headless* bench loop with its own `BenchReportData` JSON/text emission.
- `--perf-stats` runs the *interactive* loop normally and adds a perf summary on exit — the only way to get honest interactive-mode timing.

**Verdict**: **KEEP**. Documented in `CHANGELOG.md:1341, 1396, 1400, 1440, 1456`, `RENDER_ENGINE.md:259, 466, 478, 587`, `benchmark/README.md:140, 192, 194, 1056`, `COSMIC_DRAGON_EXPLORATION.md:86, 371`. Distinct role from both `--verbose` and `--benchmark`.

---

### 10. `--check-bitcolor` (config.rs:699–704, hidden) — NEEDS_OWNER_DECISION

**Help text**: "Print detected terminal color capability and exit"

**What it claims to do**: One-shot diagnostic — prints COLORTERM, TERM, auto-detected mode, forced mode (if `--colormode` set), effective mode, then exits.

**Consumed at**:
- `main.rs:577–602` — full implementation (5 println's, then `return Ok(())`)

**Total references in repo** (excluding the audit doc that mentions it): exactly **2** — `config.rs:700` (definition) and `help_detail.rs:333` (manual documentation). No tests, no scripts, no CHANGELOG entries, no README mention.

**Does it duplicate `--doctor`?** **Mostly yes.** `doctor.rs:264–293` (TERMINAL section) already prints:
- `TERM`
- `COLORTERM`
- `color_auto_detected` (same value as `--check-bitcolor`'s `auto_detected`)
- `color_forced` (only when `--colormode` is set — same condition as `--check-bitcolor`)
- And the RENDERER section above it prints `color_depth` (same as `--check-bitcolor`'s `effective`)

So `--check-bitcolor`'s 5-line output is a strict subset of `--doctor`'s output. The only differences:
1. `--check-bitcolor` is a 5-line plain print (potentially script-parseable).
2. `--doctor` is a multi-section human-readable report.

**Verdict**: **NEEDS_OWNER_DECISION**. Three options:
- (a) **Remove** — route users to `--doctor` (which already includes everything `--check-bitcolor` prints, plus much more). Add to `REMOVED_FLAGS` in `validation.rs` with migration message.
- (b) **Keep** — if any script in the wild pipes `cosmostrix --check-bitcolor` for CI color-capability gating. (No evidence of such use in this repo, but the binary is shipped publicly.)
- (c) **Keep but document** — add a one-line note in `--doctor` advice pointing to `--check-bitcolor` as the script-friendly alternative.

Owner's call. Lowest-risk option is (c) (no behavior change); most-cleanup option is (a).

---

### 11. `--reset-terminal` (config.rs:489–495)

**Help text**: "Destructive terminal recovery: clears screen, purges scrollback, resets modes"

**What it claims to do**: 5-layer defense-in-depth terminal recovery (ANSI restore, ANSI reset, crossterm, stty sane, external reset/tput). Use after SIGKILL or crash.

**Consumed at**:
- `main.rs:387–390` — `if args.reset_terminal { reset_terminal_emergency(); return Ok(()); }`
- `terminal.rs:1021, 1034, 1061, 1082` — `reset_terminal_emergency()` implementation
- `doctor.rs:304–306` — references it in COMPATIBILITY section: `"reset_terminal": "explicit destructive recovery: ..."`
- `terminal_tests.rs:120, 147, 163` — 3 unit tests on the reset sequence

**Could it be a separate utility?** In principle yes (e.g., `cosmostrix-reset`), but the design choice to bundle it is intentional and well-documented:
- `README.md:130, 147, 149, 377, 492, 495` — user-facing docs
- `docs/README.md:23, 152` — docs index
- `docs/COSMIC_DRAGON_ARCHITECTURE.md:42, 48, 60, 134, 163, 166` — architecture
- `docs/TERMINAL_KILL_CLEANUP.md:128, 131, 141, 220` — kill cleanup
- `docs/TERMINAL_LIFECYCLE_MATRIX.md:21, 128, 130, 139, 202` — lifecycle matrix
- `docs/TERMINAL_COMPATIBILITY.md:30, 50, 63, 73, 108, 111` — compatibility
- `docs/RELEASE_GUARD.md:148` — release checklist item
- `KNOWN_ISSUES.md:103, 111` — known-issues remedy
- `CHANGELOG.md:2181, 2184, 2188, 2227, 2394` — version history

The user model "kill -9 broke my terminal → run `cosmostrix --reset-terminal`" is the documented recovery contract. Forcing users to remember a second binary name would regress UX.

**Verdict**: **KEEP**. One-shot utility, yes, but intentionally bundled and heavily documented.

---

### 12. `--uniform` (config.rs:282–288)

**Help text**: "Uniform column speeds (disables variable pacing for organic rain)"

**What it claims to do**: Disable the variable-pacing async mode so all columns advance at the same speed.

**Consumed at**:
- `main.rs:832` — `let effective_async = args.async_mode && !args.uniform;`
  - `args.async_mode` is `#[arg(skip = true)]` (always-true internal field — the `--async` flag was removed in v17, see config.rs:578–581 comment)
  - So `effective_async = !args.uniform`
- This `effective_async` flows into `CloudConfig.async_mode` (`main.rs:961`) and is consumed throughout the renderer.

**Config equivalent**: `async-mode = false` in config (config_apply.rs:480–484). But the `--async` CLI flag was removed in v17, so `--uniform` is the only CLI way to opt out.

**Verdict**: **KEEP**. The v17 comment at config.rs:578–579 explicitly says "use --uniform to disable" — this flag is the designated CLI replacement for `--async`. Without it, users would have no CLI knob for variable-pacing.

---

### 13. `--screensaver` (config.rs:290–297)

**Help text**: "Screensaver mode: only q exits (all other input ignored)"

**What it claims to do**: Lock down input handling to "only q exits" (with a small documented allowlist for HUD/intro toggles that don't break the screensaver aesthetic).

**Consumed at**:
- `main.rs:925` — passed into `verbose::print_verbose`
- `main.rs:1008` — passed into CloudConfig
- `main.rs:1288–1289` — warns "interactive input handler; bench has no input loop" when combined with `--benchmark`
- `interactive/event_loop.rs:808` — `if cfg.screensaver { ... }` — the actual gate (drops events except the allowlist)
- `interactive/event_loop.rs:684, 710, 794, 823, 827, 861` — extensive comments documenting the screensaver input contract
- `verbose.rs:84, 183` — verbose output

**Config equivalent**: None.

**Verdict**: **KEEP**. Real mode, distinct behavior, used in production (kiosk / screen-lock replacement scenarios per `constants.rs:103`).

---

### 14. `--intro` (config.rs:299–308)

**Help text**: "Show cinematic intro before rain begins (cosmic|logo|none, default: logo)"

**What it claims to do**: Play a cinematic intro animation (cosmic flythrough or logo reveal) before the rain starts.

**Consumed at**:
- `main.rs:889, 1009` — `args.intro.unwrap_or(IntroType::Logo)` (default-to-logo resolution)
- `main.rs:1291–1292` — warns "interactive intro animation; bench never plays it" when combined with `--benchmark`
- `interactive/event_loop.rs:107–108` — `if cfg.intro != IntroType::None { super::intro::run_intro(...) }`
- `config_apply.rs:445` — config-side loader (`intro = "cosmic"` in config.toml)
- `config_apply_tests.rs:1152–1195` — 5 tests covering all 3 values + config interaction

**Config equivalent**: `intro = "logo"` in config.toml. Both paths converge on the same `IntroType` enum.

**Verdict**: **KEEP**. Real feature, dual-path (CLI + config) like `--color-tune`. CLI takes precedence (config_apply.rs:445 only sets when CLI didn't).

---

## Cross-Cutting Observations

### Help-text inconsistency on `--atmosphere-mode` / `--atmosphere-regime`

The inline `help = "..."` strings on these two flags say "config only" (config.rs:711, 718), but the flags ARE CLI-parseable, ARE consumed in `main.rs:799–801`, and ARE documented as ADVANCED CLI flags in `help_detail.rs:360–371` with explicit "Config: ..." lines. The "config only" wording is misleading and inconsistent with the convention used by every other dual-path flag (`--auto-color-drift`, `--color-tune`, `--bold`, `--shadingmode`, `--color-bg` all use "Config: <key> = <value>" suffix or no parenthetical at all).

**Suggested fix** (owner decision, not in scope of this audit):
```rust
// config.rs:711
help = "Atmosphere mode (disabled, controlled-live). Config: atmosphere-mode = \"controlled-live\""
// config.rs:718
help = "Atmosphere regime (calm, pulse, signal, compression, void, monolith-pressure, adaptive). Config: atmosphere-regime = \"pulse\""
```

### Patterns confirmed (no action needed)

- **Dual-path flags** (`--color-tune`, `--colors-custom`, `--intro`, `--auto-color-drift`, `--atmosphere-mode`, `--atmosphere-regime`): all use the `config_value(matches, cfg, "<snake>", "<kebab>")` helper at `config_apply.rs` which checks CLI-explicit first, then config. This is the consistent design pattern, not dead weight.
- **`#[arg(skip = ...)]` fields** (`async_mode`, `glitch_pct`, `max_droplets_per_column`, `rippct`, `shortpct`): NOT CLI flags (no `long = "..."`). These are v17/v30 internal-only fields kept for runtime use. Correctly excluded from this audit's scope.
- **Hidden-vs-removed distinction**: hidden flags (`hide = true`) are intentionally-undocumented-but-supported; removed flags live in `validation.rs::REMOVED_FLAGS` with migration messages. None of the audited flags belong in the latter.

---

## Owner Action Items

1. **Decide on `--check-bitcolor`** (option a/b/c above). This is the only flag in this audit that qualifies as a removal candidate.
2. **(Optional, low priority)** Fix the misleading "config only" help strings on `--atmosphere-mode` and `--atmosphere-regime` to match the convention used by other dual-path flags.

No other action items. The remaining 12 audited flags are all clearly alive, consumed, and serve distinct purposes.
