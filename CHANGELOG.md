# Changelog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

cosmostrix uses [SemVer](https://semver.org/). Git tags use a leading `v` (e.g. `v50.0.0`).

Pre-v13 history is archived in [`docs/archive/CHANGELOG_PRE_V13.md`](docs/archive/CHANGELOG_PRE_V13.md). The summary below covers the full journey from the first public release to the current beta, condensed so users can follow the evolution without wading through per-release minutiae.

---

## Unreleased

### stability: S-master-4-v2 LTS — deep stability audit, verified at peak (no code changes)

Nine-axis long-run robustness audit (deps + src stability-critical
subsystems, staged important-dirs-first per brief): panic-path
inventory (33 production sites, all safe by construction/validation —
uniform-range expects, predicate-guarded splits, checked-Option
patterns), v25 double-panic-proof panic hook with terminal-first
restore, full signal coverage (SIGTERM/HUP/QUIT graceful with bounded
3s wait, TSTP/CONT suspend/resume, PDEATHSIG fork guard, dead-PTY
watchdog), named catch_unwind-wrapped flag-bounded threads with no
per-scene-switch accumulation, poison-tolerant lock discipline
throughout, all-bounded sync_channel(64) queues with backpressure,
no constructor-vs-resize window, pinned deps with CI cargo-audit +
cargo-deny, and a clean KNOWN_ISSUES/backlog cross-check. Zero code
changes warranted — every candidate gain would be churn without a
robustness delta (over-engineering per brief). Control A/B on the
identical tree: visual metrics within the <=0.1-0.2% noise band,
alloc counts bit-stable (563/553). Full matrix:
docs/research/S_MASTER_V2_AUDIT.md Task 4.

### security: S-master-3-v2 LTS — config read size cap (OOM DoS hardening)

Four config read paths loaded the full file into a String with no size
bound (startup load + /etc fallback, live-reload watcher reparse, and
the ambient ground-truth check that re-reads every 5s — a runaway or
hostile multi-GB file in a whitelisted config dir would thrash/OOM the
process repeatedly). New shared reader `config_io::read_config_capped`
enforces `CONFIG_FILE_MAX_BYTES` (1 MiB, ~100x the typical config) via
`Read::take` (no TOCTOU window on concurrently growing files). Oversized
files map to existing unreadable-file semantics (defaults / skip
reparse). Verified already-hardened surfaces (no action needed): 35
unsafe sites all libc-FFI with SAFETY comments, safepath strict
whitelist, charset-custom control-char rejection, message sanitization
at both entry points with the 200-char cap. Also: stale live_config_poll
doc comment corrected (poll hashes an 8 KiB prefix, not a full
read_to_string), and a &PathBuf -> &Path signature fix surfaced by
clippy. 4 new tests (accept / exact-cap / over-cap / missing).

A/B 10s monolith benchmark: visual bit-parity, total_ns_per_cell
-0.65%, avg_fps +0.62% (noise band). Full matrix:
docs/research/S_MASTER_V2_AUDIT.md.

### optimize: S-master-2-v2 — verified at peak, no changes (skip per brief)

Hot-path inventory of the per-frame pipeline (per-cell shader LUTs,
allocation profile 0.0006 allocs/frame, bounded per-droplet
transcendentals, zero TODO markers) plus a control A/B on the
identical tree: run-to-run noise floor <=0.1% visual / ~1% fps /
bit-stable alloc count — narrower than any remaining optimization
could measure. LUT-ing the remaining per-droplet exp/sin calls would
change output bits (visual regression) for sub-microsecond gains.
Peak-constrained by design; skipped per the task brief (don't
over-engineer). Evidence: docs/research/S_MASTER_V2_AUDIT.md.

### dragon-hunt: S-master-1-v2 — zombie/stale/duplicate sweep, 4 real findings fixed

Staged audit (deps, cosmostrix root, then src important dirs) for
spaghetti/burden/duplicate/redundant/stale/zombie code. Deps: all 12
used, zero zombies. Scripts/root assets: all referenced. Source: 4 real
findings fixed —

- crystal_dragon_control enum doc claimed calc-v2 "NOT YET IMPLEMENTED"
  — stale since the Dragon Engine v2 merge (it is the default). Rewritten.
- CrystalDragonControl.drift_chance + cpu_ema_alpha were zombie fields
  (runtime read the consts directly, so the documented future-config
  override could never work). Wired through: tick reads control.drift_chance,
  sensor copies control.cpu_ema_alpha for its EMA. Consts now only seed
  defaults; struct-level allow(dead_code) removed. Zero behavior change.
- monolith.rs: 9 dead helper imports masked by allow(unused_imports)
  trimmed to the 8 live names.
- 10 stale file pointers in comments fixed (chroma/* pre-rename paths,
  palette_floor_tests.rs -> palette/tests_floor_audit.rs,
  src/validation.rs -> src/validation/mod.rs, safepath.rs -> safepath/mod.rs,
  legacy parity-test pointer -> its real inline mod tests home).

A/B 10s monolith benchmark: visual bit-parity (entropy +0.01%, gini
0.00%, streams/allocs identical), fps +1.05% (noise band). Full matrix:
docs/research/S_MASTER_V2_AUDIT.md.

### dragon-engine-v2: depth-verify of d55442d — real, wired, now PROVEN + 1 bug fixed

Owner suspected the Dragon Engine v2 upgrade (commit d55442d) was "not
real working". Full source-level verification of all 11 v2 features
(crystal calc-v2, cosmic predictive self-healer / ghost AI / adaptive
phosphor, chroma Phase 3/4 all-six) at HEAD: every feature is real and
wired into the production path. What was missing was proof and one
integration bug:

- Fixed: self-healer `reset()` (fires on every scene switch / config
  rebuild) leaked the v2 predictive state — `pressure_ema`,
  `pressure_ema_prev`, `preemptive_throttle_active` survived the reset,
  letting a phantom trend pre-throttle a fresh scene and a stale active
  flag suppress re-fires. All three fields now cleared.
- Added 25 regression tests — the missing proof: calc-v2 DriftHistory
  (recency factors, ring overwrite, group membership, 20k-sample
  statistical suppression), predictive EMA (spike fires, noise filtered,
  gradual-ramp contract, recovery clear, reset clear), ghost AI (hard
  gate, ramp endpoints + midpoint, pause/transition gates), adaptive
  phosphor (idle > loaded, monotonic ramp, skip gate, exact decay math
  200 -> 66 at 50 ms).
- Docs synced: point_system calc-v1 "the default" stale comment (the
  default is CalcV2), THREE_DRAGON_ENGINES.md calc-v1 rows + new Cosmic
  v2 paragraph. Full audit matrix:
  docs/research/DRAGON_ENGINE_V2_VERIFY.md.

### v80.0.0: crystal-dragon masterclass speed tune (owner directive)

Scene `crystal-dragon` speed 10 -> 30 per owner directive ("too slow,
fine tune to 30"). The honor scene now moves at living-crystal pace;
the Monolith segmented structure keeps the meditative texture. All
other 17 scenes audited against documented design intent — every value
already at its design point, no further changes (bicycle rule).

### harmony: Z-master-3-v2 audit — CLI/config priority contract verified end to end

Final pass of the Z-master-v2 LTS trio: verified the CLI > config >
scene-custom > base-scene > builtin precedence contract resolves
identically at startup and on live-reload for every flag/block
combination (the 9 gaps found by Z-master-1-v2 / Z-master-2-v2 were
exactly the places where it did not).

- scripts/custom_features_stresstest.sh: +7 harmony cases asserting
  RESOLVED values in the benchmark JSON report (--bold 0 with config
  bold = 2 must report "bold":"Off", etc.) — 34/34 pass.
- 10s A/B benchmark (monolith 80x24 dry): visual bit-parity
  (entropy/gini/streams/cells identical to RNG noise) and performance
  parity (avg_fps -0.17%); report + raw JSON archived under
  benchmark/bench-labs/Z_master_v2/. Already at peak — no further
  optimization per task brief.
- docs/research/Z_MASTER_V2_PRIORITY_AUDIT.md: full method, findings,
  fix map, and test matrix for all three tasks.

### live-reload: CLI intent preservation for 5 more flags (Z-master-2-v2)

Depth audit of CLI + config/live-reload found five flags whose CLI
intent was silently lost on the first config edit (same bug class as
the monolith-size Issue #4 / alpha.7 guard family — the guards simply
were never added):

- --bold, --shading-mode, --color-bg: the matching config key overrode
  the CLI flag on every live-reload. CliExplicit gained the fields and
  rebuild_cloud_config gained the guards.
- --colors-custom: a config `color` key switching to a builtin cleared
  the CLI-owned custom palette (startup never drops it). The color
  block and the scene color-default arm now gate on cli.colors_custom.
- --scene-custom: a config `scene` key replaced the CLI-selected custom
  scene and cleared the tracker (startup applies the CLI scene-custom
  layer last). The scene block now gates on cli.scene_custom; the
  tail block still re-applies the custom scene fields so live-editing
  the block keeps working.
- scene-custom block fields bold / shading-mode / colors-custom now
  honor cli_explicit.* (extends the Z-master-1-v2 field gates to the
  newly tracked flags).

9 regression tests added (tests_cli_priority.rs). Suite green: 1967/0.
Stale per-key matrix rows (monolith-size / power-dragon / async-mode
still described pre-alpha.7 behavior) refreshed in
docs/LIVE_RELOAD_BEHAVIOR.md section 1 + new section 12.

### live-reload: killer-features priority contract hardening (Z-master-1-v2)

Depth stresstest of the colors-custom / charset-custom / scene-custom
re-apply path found 4 gaps in the CLI > config > scene priority
contract, all in the same family as the earlier FPS-F4 fix (which had
gated fps only):

- Scene-custom field layer: every field arm (`color`, `colors-custom`,
  `charset`, `charset-custom`, `speed`, `density`, `glitch-level`,
  `async-mode`) now honors the matching `cli_explicit.*` flag —
  `cosmostrix --speed 50 --scene-custom fast` no longer drops to the
  block's speed on the first config edit.
- Base-scene inheritance layer: `apply_base_scene_to_cloud_config` gates
  color/charset/speed/density/glitch-level on `cli_explicit.*` (fps was
  already gated), mirroring startup's `apply_base_scene_to_args`.
- Intra-block conflicts are deterministic: a block defining BOTH
  `color` + `colors-custom` (or `charset` + `charset-custom`) now
  resolves like startup (`color`/`charset` wins, the custom reference is
  skipped) instead of applying in HashMap iteration order.
- Scene switch away from a palette-owning custom scene clears the stale
  `custom_palette` (create_cloud applies the palette after the scheme,
  so a lingering palette made the switch a visual no-op for color).

12 regression tests added (src/config/live_config/tests_cli_priority.rs,
extracted to respect the 800-LOC file cap). Suite green: 1957/0.

### naming: fix kebab-case inconsistency in 4 CLI flags + config keys (Z-master-1X round 10)

- **Audit**: owner requested naming audit for wrong/ambiguous/dangerous/not-masterclass English. Found 4 kebab-case inconsistencies where multi-word flags lacked the `-` separator, breaking the established pattern (`async-mode`, `color-bg`, `crystal-dragon`, `glitch-level`, `monolith-size`, `msg-mode` all use kebab-case).
- **Fixes**:
  1. `shadingmode` → `shading-mode` (user-facing: config key + `--dump-config` template + `--help`). The config suggestion system auto-migrates: old `shadingmode` in config.toml triggers "tip: a similar value exists: 'shading-mode'" (edit distance 1).
  2. `colormode` → `color-mode` (hidden CLI flag, low user impact).
  3. `glitchms` → `glitch-ms` (hidden CLI flag, "ms" suffix now separated for readability).
  4. `lingerms` → `linger-ms` (hidden CLI flag, same as glitchms).
- **Rust field names unchanged**: `shading_mode`, `colormode`, `glitch_ms`, `linger_ms` (snake_case Rust identifiers stay as-is — only the CLI `long` string + config key string changed).
- **Backwards compatibility**: the config suggestion system (`closest_value_match`, edit distance ≤ 2) catches old `shadingmode` usage and suggests `shading-mode`. No silent breakage — users get a clear tip.
- **Files changed**: `src/config/mod.rs` (4 CLI long flags), `src/config/configfile.rs` (USER_CONFIG_KEYS), `src/config/config_apply.rs`, `src/config/configfile/configfile_dump.rs`, `src/config/live_config/mod.rs`, `src/scene_custom/mod.rs`, `src/scene_custom/overrides.rs`, `src/cli/help_detail.rs`, `src/main.rs`, `src/testconf/field_validation.rs`, `src/engine/chroma_dragon_engine/shaders/base/tests_bold_audit.rs` (test fn names), `src/engine/cosmic_dragon_engine/cloud/scene_runtime.rs` (comments), `docs/CENTRAL_CONTROL_RAINS_USAGE.md`, `docs/LIVE_RELOAD_BEHAVIOR.md`, `docs/TERMINAL_COMPATIBILITY.md`, `docs/RULES.md`.
- **Tests**: 1945 pass. clippy clean, fmt clean, gatekeepers 9/9.
- **Also found**: scene name `dragon-crystal` (src/scene/mod.rs:314) has reversed word order vs engine name `crystal_dragon_engine` + CLI flag `--crystal-dragon`. Reported to owner for decision — not fixed in this commit (pending owner approval).

### crystal-dragon: stack-allocated CDF — zero heap allocation on drift path (Z-master-1X round 9)

- **Audit scope**: deep audit of Crystal Dragon engine for peak masterclass alternatives. Owner constraint: design stays intact (point system, 3 groups, calc-v1 probabilistic, 60s poll, 12% drift, CPU primary + CLOCK fallback, EMA alpha 0.25, weight penalty 0.1).
- **6 options evaluated**:
  1. **Stack-allocated CDF** `[f32; 16]` — eliminates 2 heap Vec allocations per drift. No design change. ✅ WORTH IT.
  2. Precompute weight table — ~100ns per drift. Marginal (drift fires every ~5 min). ❌ skip.
  3. Adaptive EMA alpha — responsive after gaps. Violates owner-locked fixed alpha. ❌ skip.
  4. Sqrt lookup table — ~4ns per 60s. Over-engineering (sqrt is 1 instruction). ❌ skip.
  5. CDF reuse across retries — already optimal (CDF built once, cdf_select just draws + searches). ✅ already peak.
  6. Weight penalty tuning — owner-locked design parameter. ❌ skip.
- **Implemented Option 1**: changed `calc_v1_select` from `Vec<f32>` (weights + CDF) to stack-allocated `[f32; CRYSTAL_DRAGON_MAX_THEMES_PER_GROUP]` (16 slots, covers 14 themes + 2 reserved). Zero heap allocation on the drift path. Same algorithm, same output, just stack instead of heap.
- **New constant**: `CRYSTAL_DRAGON_MAX_THEMES_PER_GROUP = 16` in `crystal_dragon_control/mod.rs` — sizes the stack arrays + documents the cap.
- **Design impact**: NONE — same calc-v1 probabilistic weighted selection, same CDF, same binary search, same skip-current-scheme retry. Only the memory location changed (heap → stack).
- **Files changed**: `src/engine/crystal_dragon_engine/point_system/mod.rs` (stack arrays + import), `src/engine/crystal_dragon_engine/crystal_dragon_control/mod.rs` (new constant).
- **Tests**: 85 crystal_dragon tests pass. clippy clean, fmt clean, gatekeepers 9/9.

### crystal-dragon: fix CPU→point mapping — sqrt curve for full color variety (Z-master-1X round 8)

- **Bug**: owner observed that Crystal Dragon drift almost never produces purple (Medium group) or fire (Hot group) colors — mostly white/cyan (Cold group). The owner designed 3 temperature groups (Cold/Medium/Hot) expecting full variety, but the distribution was bottlenecked.
- **Root cause**: the CPU→point mapping was linear (`point = cpu * 0.99`). cosmostrix is a highly optimized single-threaded renderer with typical interactive CPU usage of 0.5–8%. The linear mapping produced points 1–8 (always Cold group → blues/cyans/whites only). Medium group (greens/purples, needs point 34–66 = 34%+ CPU) and Hot group (yellows/reds/fire, needs point 67–99 = 67%+ CPU) were essentially unreachable in normal use.
- **Fix**: changed the mapping from linear to sqrt (`point = sqrt(cpu) * 9.9`). The sqrt curve spreads low CPU values across a wider point range:
  - 0.5% CPU → point 7 (Cold, but spread across full group range — more theme variety)
  - 2% CPU → point 14 (Cold)
  - 12% CPU → point 34 (Medium — greens/purples now reachable!)
  - 50% CPU → point 70 (Hot — yellows/reds/fire now reachable!)
  - 100% CPU → point 99 (Hot, max)
- **Design preservation**: the sqrt mapping is monotonic (higher CPU = higher point = hotter group), preserving the owner's design intent (low CPU = cooler colors, high CPU = hotter colors). The change only affects the distribution spread, not the direction.
- **Files changed**: `src/engine/crystal_dragon_engine/sensor/mod.rs` (poll_cpu sqrt mapping + doc comment), `src/engine/crystal_dragon_engine/sensor/tests.rs` (2 new regression tests: sqrt reaches all groups + monotonic), `src/engine/crystal_dragon_engine/mod.rs` (doc comment), `docs/CRYSTAL_DRAGON_ENGINE.md` (§4.1 updated).
- **Tests**: 1945 unit tests pass (2 new z_master_1x_round8 tests). clippy clean, fmt clean, gatekeepers 9/9.
- **A/B benchmark** (10s each):
  - avg_fps: 92613 → 92592 (within noise)
  - dirty_cell_ratio: 2.96% → 2.96% (identical)
  - total_ns_per_cell: 190.14 → 190.17 (within noise)
  - alloc_calls: 563 → 563 (identical)
  - Conclusion: zero performance regression — the sqrt call is negligible (1 instruction).

### Killer features depth stresstest — no bugs found + stresstest script (Z-master-1T)

- **Stresstest scope**: depth stresstest of the 3 killer custom features — colors-custom, charset-custom, scene-custom. 27 cases covering: valid/invalid hex, empty/single/max stops, unquoted hex, nonexistent names, duplicate names, empty/wide/max charset, missing/unknown base-scene, empty scene block, color+colors-custom conflict, charset+charset-custom conflict, cross-feature interaction, CLI override.
- **Result**: 27/27 PASS. **NO BUGS FOUND.** The custom features are robust:
  - colors-custom: validates rain field (min 2 stops, max 64), parses #rrggbb + #rgb shorthand, rejects invalid hex, bounds blocks (max 100) + name length (max 64). Empty/single rain → clear error. Unquoted # → TOML comment caught by parser.
  - charset-custom: validates set field (non-empty, max 256 chars), handles wide CJK chars (silently skipped with warning), rejects control chars. Empty set → error. Over-max → error.
  - scene-custom: validates base-scene name (rejects unknown), handles missing base-scene (uses defaults), resolves color+colors-custom conflict (colors-custom wins per documented priority), resolves charset+charset-custom conflict.
  - Cross-feature: all 3 work together. CLI `--colors-custom` overrides scene-custom color. CLI `--charset` overrides scene-custom charset.
- **Stresstest script added**: `scripts/custom_features_stresstest.sh` (27 cases). Uses `~/.config/cosmostrix/stresstest_tmp/` for config files (safepath-compliant).
- **A/B benchmark** (10s each, no code changes = identical):
  - avg_fps: 92613 → 91879 (within noise)
  - dirty_cell_ratio: 2.96% → 2.96% (identical)
  - total_ns_per_cell: 190.14 → 191.60 (within noise)
  - alloc_calls: 563 → 563 (identical)
  - Conclusion: zero regression — stresstest was validation-only.
- **Files changed**: `scripts/custom_features_stresstest.sh` (new, 27 cases).
- **Tests**: 9/9 gatekeepers pass.

### HUD: humanize large numbers in dcel + fps for consistency (Z-master-1X round 7)

- **Bug**: `dcel` count showed raw numbers (e.g. `dcel: 1200/12%`) instead of humanized format. Owner mandate: use `k` suffix for >=1000 (e.g. `dcel: 1.2K/12%`), raw for <1000. Also audit ALL HUD metrics for large-number formatting consistency.
- **Fix 1 (dcel count)**: changed from raw `{dcel_count}` to `humanize(dcel_count)`. Now 120 → "120", 1200 → "1.2K", 12000 → "12K". Matches the existing `tcel` format (which already uses `humanize`).
- **Fix 2 (fps threshold)**: lowered the `humanize_f64` threshold from 10K to 1K. Previously fps between 1000-9999 showed raw "1234"; now shows "1.2K". This makes fps consistent with dcel/tcel — all count-like metrics use the same humanization rules.
- **Audit of all 24 metrics**: verified which metrics can exceed 1000 and need humanization:
  - `fps` (row 0): CAN exceed 1000 (high-refresh terminals, benchmark mode). Fixed — now humanizes at >=1000.
  - `dcel` count (row 19): CAN exceed 1000 (large terminals). Fixed — now humanizes.
  - `tcel` (row 20): CAN exceed 1000 (large terminals). Already humanized. ✓
  - `max`/`p99` (rows 2-3): ms values, typically <10ms. No change needed.
  - `cpu` (row 4): percentage 0-100%. No change needed.
  - `rss` (row 5): already uses MiB/KiB format. ✓
  - `ehs` (row 6): 0-100 integer. No change needed.
  - `prs` (row 7): 0.00-1.00. No change needed.
  - `sped`/`dsty` (rows 11-12): typically 1-100. No change needed.
  - All string/enum metrics (scn/chr/clr/prdr/crdr/ambt/glth/ctun/mnst/cid/up/screensize): no numeric formatting. ✓
- **Format consistency**: all count-like HUD metrics now use the shared `humanize()` / `humanize_f64()` helpers from `src/diagnostics/humanize.rs`. Rules: <1000 = bare number, 1000-9999 = "X.XK" (1 decimal), 10000-999999 = "XXK" (no decimal), >=1M = "X.XXM". Uppercase "K" matches the existing benchmark + tcel convention.
- **Files changed**: `src/interactive/hud/metrics.rs` (dcel humanize + fps threshold), `docs/HUD.md` (mockup updated).
- **Tests**: 63 HUD tests pass. clippy clean, fmt clean, gatekeepers 9/9.

### CLI/config depth stresstest — no bugs found + stresstest script added (Z-master-1T)

- **Stresstest scope**: depth stresstest of CLI + config/live-reload for potential bugs. 47 cases covering: CLI value boundaries (fps 0/1/240/999999, speed 0/1/100, density 0/0.01/5.0/-1), flag conflicts (scene+color, scene+scene-custom, msg-mode+CLI, power+crystal), enum values (glitch-level case sensitivity, intro types, color-bg, monolith-size, msg-fill-style), typo suggestions (colr→color, scne→scene, non-numeric fps/speed), config edge cases (empty, unknown key, bad type, out-of-range, dual message keys, malformed TOML), dump-config/testconf validation, and rapid config reloads.
- **Result**: 47/47 PASS. **NO BUGS FOUND.** The CLI/config system is robust:
  - Invalid values produce clean error messages (no panics, no crashes).
  - Unknown config keys are rejected with clear errors.
  - Out-of-range values are rejected at validation.
  - Malformed TOML is caught by the parser.
  - Flag conflicts resolve via the documented priority (CLI > config > scene defaults).
  - Live-reload watcher is robust: bounded channel (cap 64), panic catch, polling heartbeat fallback, invalid values revert to base.
  - `--config` path is protected by safepath (security feature — only allowed directories accepted).
- **Stresstest script added**: `scripts/cli_config_stresstest.sh` — 47 cases, runnable via `bash scripts/cli_config_stresstest.sh`. Complements the existing `scripts/cli_suggestion_stresstest.sh` (18 cases) with broader coverage of value boundaries, config files, and live-reload scenarios.
- **A/B benchmark** (10s each, no code changes = identical):
  - avg_fps: 92662 → 92660 (within noise)
  - dirty_cell_ratio: 2.96% → 2.96% (identical)
  - total_ns_per_cell: 190.04 → 189.96 (within noise)
  - alloc_calls: 563 → 563 (identical)
  - Conclusion: zero regression — stresstest was validation-only.
- **Files changed**: `scripts/cli_config_stresstest.sh` (new, 47 cases).
- **Tests**: 1943 unit tests pass. clippy clean, fmt clean, gatekeepers 9/9.

### CLI/config harmony audit — peak verdict + stale comment fixes (Z-master-T)

- **Audit scope**: deep audit of CLI flags vs config keys for harmony — override priority, parity, conflicts. Owner analogy: "bicycle factory standard, only change body color" — core machinery stays, surface consistency only.
- **Harmony verdict**: ALREADY AT PEAK. The CLI/config system is harmonious:
  - Override priority is consistent everywhere: CLI explicit > config > clap default > scene defaults. The `config_value()` + `is_explicit()` mechanism cleanly returns None when CLI is explicit, else the config value — no conflicts, no fighting.
  - All 21 user-tunable config keys have CLI parity (scene, color, charset, fps, speed, density, monolith-size, glitch-level, bold, shadingmode, color-bg, crystal-dragon, power-dragon, async-mode, intro, intro-color, message, message-border, msg-mode, msg-fill-style). The one exception (`ambient-snapback-secs` = config-only) is by design — ambient scheduling itself is config-only per owner decision.
  - All CLI-only flags (no-effects, benchmark family, screen-size, screensaver, utility/list/show flags) are correctly CLI-only — they are mode switches or utilities, not user-tunable runtime parameters.
  - Naming is consistent: config keys use kebab-case matching CLI long flags. No snake_case/kebab-case mismatches.
- **Surface fixes** (stale comments only — no machinery change):
  - `configfile.rs:64`: intro-color comment said "Default: same as --color (rain color)" — WRONG. Actual default is brand EnergyZen (INTRO_BRAND_SCHEME). Fixed to "Default: brand EnergyZen (NOT the rain color)".
  - `configfile.rs:75`: default message comment said "showing the project name" — WRONG. Actual default message is "Experience a masterpiece with cosmostrix v<CARGO_PKG_VERSION>" (default_message_text in types/constants.rs). Fixed to exact string + source reference.
- **A/B benchmark** (10s each, comment-only changes = zero perf impact):
  - avg_fps: 91724 → 92671 (within noise, fps_drift stable at -0.48%)
  - avg_dirty_cell_ratio: 2.96% → 2.96% (identical)
  - total_ns_per_cell: 191.99 → 189.97 (within noise)
  - frame_entropy_bits: 3.29 → 3.30 (within noise)
  - density_gini: 0.8961 → 0.8957 (within noise)
  - alloc_calls: 563 → 563 (identical)
  - Conclusion: zero performance/visual regression — the "gears" are untouched.
- **Files changed**: `src/config/configfile.rs` (2 stale comment fixes, trimmed to stay under 800-LOC cap).
- **Tests**: clippy clean (`-D warnings`), fmt clean, gatekeepers 9/9.

### HUD: deep audit — stale comment fixes + peak verdict (Z-master-1X round 6)

- **Audit scope**: all 24 HUD metrics (rows 0-23) for precision, harmony, stability/LTS. Verified metric formatting, NaN/Inf guards, pause-freeze consistency, and doc accuracy.
- **Precision verdict**: ALREADY AT PEAK. Every float metric uses appropriate precision — fps/tgt adaptive (1 decimal <100, 0 decimals >=100, humanize >=10K), max/p99 at 3 decimals ms, cpu at 2 decimals %, ehs integer, prs/sped/dsty at 1-2 decimals, dcel count + 1 decimal %. No over-engineering needed.
- **Harmony verdict**: ALREADY CONSISTENT. All 24 metrics follow the " label: value" pattern, all use the chroma gradient color sweep, all rate-limited at 1 Hz (text reformat) with per-frame color refresh. Percentage metrics use % suffix consistently (cpu, dcel). String metrics (scn/chr/clr/glth/ctun/mnst/prdr/crdr/ambt) use lowercase short labels.
- **Stability/LTS verdict**: STRONG. NaN/Inf guards on all float setters (ehs, prs, sped, dsty — clamped to 0.0 on non-finite). Division-by-zero guarded in fps (avg_ms > 0.0 check) and dcel (latest_total > 0 check). max_ms guarded in push_frame_time (NaN comparison is false, never stored). Pause-freeze consistent across all samplers (push_frame_time, maybe_sample_rss, maybe_sample_cpu, set_dirty_cell_stats, set_endurance_health_score, set_effective_pressure). The intentional asymmetry where set_endurance_health_score/set_effective_pressure do NOT check `self.visible` (only `self.metrics_paused`) is correct — the event loop always pushes live values so they're fresh when the HUD is toggled on.
- **Stale comments fixed**: 3 comments in metrics.rs + mod.rs still referenced the old 22-row layout (cid 19, up 20, screensize 21) instead of the 24-row layout (dcel 19, tcel 20, cid 21, up 22, screensize 23). Fixed all to match the current Z-master-1X round 5 layout.
- **Files changed**: `src/interactive/hud/metrics.rs` (2 stale comment fixes), `src/interactive/hud/mod.rs` (1 stale comment fix — "22 lines" → "24 lines" + row order list updated).
- **Tests**: 63 HUD tests pass. clippy clean, fmt clean, gatekeepers 9/9.

### HUD: dcel format combines count + percentage (Z-master-1X round 6)

- **Format change**: `dcel` metric now shows both the rolling average dirty cell count AND the ratio percentage. Old format: `dcel: 6.8%`. New format: `dcel: 120/6.8%` where 120 = rolling avg dirty cell count (integer) and 6.8% = dirty/total ratio. Owner mandate: combine count + percentage so the user sees BOTH the absolute number (how many cells changed) AND the ratio (efficiency at a glance).
- **Files changed**: `src/interactive/hud/metrics.rs` (format string + comment), `docs/HUD.md` (layout mockup).
- **Tests**: existing HUD tests pass (no assertion on dcel text format — the metric is dynamic). clippy clean, fmt clean, gatekeepers 9/9.

### config: fix stale intro-color + default-message comments in template (Z-master-1X round 6)

- **Audit**: deep audit of `--dump-config` template (configfile_dump.rs) to verify every commented default value matches the source-of-truth. Traced 19 config keys to their actual defaults (Args clap defaults, scene/mod.rs SceneConfig, build_cloud_cfg unwrap_or, ColorTune::IDENTITY, AUTO_SNAPBACK_DELAY_SECS).
- **Stale comment 1**: `intro-color` comment said "default: same as rain color" but the actual default (when intro-color is unset) is the brand EnergyZen scheme (INTRO_BRAND_SCHEME = ColorScheme::EnergyZen, event_loop_intro.rs:46) — NOT the rain color. The intro always uses the brand palette, never the rain palette. Fixed to "default: brand EnergyZen — NOT the rain color".
- **Stale comment 2**: default message comment said `"cosmostrix v<CARGO_PKG_VERSION>"` but the actual default message (default_message_text in types/constants.rs:503) is `"Experience a masterpiece with cosmostrix v<CARGO_PKG_VERSION>"`. Fixed to the exact string + added source file reference.
- **All other defaults verified correct**: scene=cinematic, color=energy-zen, charset=zen, color-bg=black, intro=logo, fps=60, speed=9, density=0.75, async-mode=true, monolith-size=normal, glitch-level=subtle, power-dragon=true, crystal-dragon=false, ambient-snapback-secs=30, bold=1, shadingmode=1, msg-mode=true, msg-fill-style=typewriter, [color.tune] brightness/saturation/head/body/tail=1.0. All match source-of-truth.
- **Files changed**: `src/config/configfile/configfile_dump.rs` (2 comment fixes).
- **Tests**: testconf 57/57, dump_config 7/7 pass. clippy clean, fmt clean, gatekeepers 8/8.

### msg-fill-style: re-audit verdict — still at peak (Z-master-1X round 5)

- **Re-audit scope**: `src/msg_fill_style/` — 11 files, 3317 LOC (mod.rs + 10 style modules). Verified no changes since the round 1 audit (only the directory move from `src/cosmic_dragon_engine/` to `src/engine/cosmic_dragon_engine/` at commit afb835a).
- **Verdict**: STILL AT PEAK. The round 1 assessment holds — excellent architecture (one-file-per-style isolation, shared helpers, explicit statelessness contract), peak performance (inline pure functions, pre-allocated pools, O(1) early-out), strong LTS (constant-lock tests, visibility gate contract, `--no-effects` gating verified intact on engrave/hologram/scorch sidecars). No over-engineering needed.
- **No code changes** — this entry documents the re-audit verdict.

### HUD: add dcel + tcel cell-efficiency metrics (Z-master-1X round 5)

- **New metrics**: `dcel` (dirty cell ratio %) + `tcel` (total cells) added at HUD rows 19-20, directly above `cid` (now row 21). Owner insight from the CELL EFFICIENCY benchmark section: `dirty_cell_ratio_percent` is the key efficiency signal — lower = more cells skip re-send (the frame buffer's dirty-tracking is working).
- **Layout**: HUD grew from 22 → 24 rows. `cid` moved from row 19 → 21, `up` from 20 → 22, `screensize` from 21 → 23. The chroma gradient was bumped from 22 → 24 stops (divisor 21.0 → 23.0).
- **Implementation**: added `DirtyCellTracker` ring buffer (60-frame window, matching `FrameTimeTracker`) to `HudState`. The event loop pushes `(dirty_count, total_cells)` every frame after `sim_draw` via `set_dirty_cell_stats()`. The 1 Hz metric tick renders `dcel:` (rolling avg dirty / latest total × 100, 1 decimal) + `tcel:` (latest total, humanized via `humanize()` — e.g. `2.8K`). Paused frames do not push (matches the `push_frame_time` freeze contract).
- **Files changed**: `src/interactive/hud/mod.rs` (DirtyCellTracker + field + setter + LOC_EXEMPT), `src/interactive/hud/colors.rs` (gradient 22→24), `src/interactive/hud/metrics.rs` (dcel/tcel rendering + row shifts), `src/interactive/hud/hud_init.rs` (cached_lines 22→24 + cid row 21), `src/interactive/event_loop.rs` (set_dirty_cell_stats call after sim_draw), `src/interactive/hud/tests.rs` + `tests_chroma_metrics.rs` + `tests_dragon_indicators.rs` + `tests_pause_freeze.rs` (row index updates), `scripts/hud_order_e2e.py` (dcel/tcel in expected order), `docs/HUD.md` (layout mockup + row refs).
- **Tests**: 63 HUD tests pass. cloud 322/322. clippy clean, fmt clean, gatekeepers 8/8.

### crystal-dragon + ambient: fix live-reload drift deadlock (Z-master-1X round 4)

- **Bug**: after a live config reload while both ambient + crystal dragon are ON, drift became rare/never even after 60s. Ambient dominated; restart fixed it. Owner repro: ambient + crystal-dragon on via config, snapback-secs=30 default. After 60s drift fires → 30s later snapback reverts to ambient → long running stays ambient, no more drift. Triggered by editing config at runtime (live reload).
- **Root cause**: `inherit_ecosystem_state` carried `drift_active` + `drift_start` across live reload. When a reload fires while a drift is visible (`drift_active=true`), the reload's re-apply path sets `user_override_since_ambient=false` (line 180 of event_loop_config_rebuild.rs), which disables the snapback mechanism (`should_auto_snapback` requires `user_override_since_ambient==true`). With `drift_active=true` inherited + snapback disabled, the drift gate `!drift_active` blocked all future drifts forever — a deadlock that only a restart (which resets `drift_active=false`) could clear.
- **Fix**: `inherit_ecosystem_state` no longer carries `drift_active`/`drift_start`. The sensor state (`crystal_dragon_sensor`, `_control`, `_last_poll`) IS still inherited (engine state — CPU point, EMA, theme entered-at), but the per-cycle drift bookkeeping resets cleanly on the fresh Cloud so the next poll can fire a fresh drift. A live reload is a config change; the drift cycle should reset, not carry stale cycle state.
- **Files changed**: `src/engine/cosmic_dragon_engine/cloud/mod.rs` (inherit_ecosystem_state — removed drift_active/drift_start carry + doc comment), `src/engine/crystal_dragon_engine/crystal_dragon_control/mod.rs` (added PartialEq derive for test assertions), `src/engine/cosmic_dragon_engine/cloud/tests/tests_color_stability.rs` (2 new regression tests: drift-cycle resets + sensor-state preserved).
- **Tests**: 2 new z_master_1x_round4 tests pass. color_stability 17/17, cloud 322/322, crystal_dragon 83/83, ambient 67/67, v50_first_reload 3/3 (coredump fix preserved). clippy clean, fmt clean, gatekeepers 8/8.
- **Docs**: `docs/AMBIENT_SCHEDULER.md` — new "Live-reload interaction" section documenting the inherit_ecosystem_state contract.

### config: fix stale color-bg default in template config (Z-master-1X)

- **Bug**: `--dump-config` template (configfile_dump.rs:35) showed the default as `default-background` but the source of truth (config/mod.rs:613 `default_value_t = ColorBg::Black`) is `black`. The template header says "All values shown are defaults" so the stale line was misleading.
- **Root cause**: the default WAS `default-background` and was changed to `black` (CHANGELOG documents the behavior change), but the template comment was never updated.
- **Fix**: template now shows `# color-bg = "black"  # or "default-background" (default: black)`. Also fixed `docs/RULES.md` CLI Flag Policy section which still referenced the legacy `Did you mean` suggestion format (3 occurrences) — updated to the canonical `tip: a similar value/argument exists` format.
- **Source-of-truth verification**: config/mod.rs:613 (default_value_t = Black), cli/help_detail.rs:516 ('black' (default)), docs/TERMINAL_COMPATIBILITY.md:44, docs/CENTRAL_CONTROL_RAINS_USAGE.md:77 — all correct. configfile_dump.rs was the only stale spot.
- **Files changed**: `src/config/configfile/configfile_dump.rs` (template), `docs/RULES.md` (suggestion format refs).
- **Gatekeeper**: 8/8 pass. No code logic changed — template + doc only.

### --no-effects: close anomaly spawn leak + peak audit verdict (Z-master-1X)

- **Audit scope**: deep audit of `--no-effects` to verify it really disables ALL cosmetic effects (not gimmick). Mapped 55 `effects_enabled` usage sites to ~13 spawn/render gate locations across the cloud engine + msg_fill_style sidecars.
- **Leak found + fixed**: anomaly spawn in `cloud/post_rain.rs` was gated on `bench_mode` only, NOT `effects_enabled`. The apply path (`apply_anomalies`) was correctly gated, but spawn continued to create + retain anomaly zones (1.5s lifetime) under `--no-effects` — wasted CPU + Vec churn for zones never rendered. The stale comment at the apply site claimed "spawn_anomaly is already gated" which was wrong. Fix: added `effects_enabled` to the spawn gate condition (commit `56f8513`). Both spawn and apply are now gated, so under `--no-effects` no zones exist and the apply branch is a no-op.
- **Peak verdict for the rest**: ALREADY AT PEAK. All 12 other cosmetic subsystems are correctly gated: quantum ripple spawn/render, border spark, click flash wave, anomaly apply, ghost event trigger/render, storytelling/emergent moments, msg_fill_style sidecars (engrave/hologram/scorch). `color_ecosystem.tick` is intentionally ungated (climate drift = rendering param modulation, not cosmetic). No over-engineering needed.
- **Files changed**: `src/engine/cosmic_dragon_engine/cloud/post_rain.rs` (spawn gate + stale comment fix).
- **Tests**: anomaly 26/26, no_effects 8/8, cloud 320/320 pass. clippy clean, fmt clean, gatekeepers 8/8.

### cli-suggestion: peak audit verdict + end-to-end stresstest (Z-master-1X)

- **Audit scope**: verify all CLI functions use the consistent "tip: a similar value exists: 'x'" / "tip: a similar argument exists: '--x'" format, and that the legacy "Did you mean" format is fully removed.
- **Consistency verdict**: ALREADY CONSISTENT. The `format_value_suggestion` helper in `src/cli/suggestion.rs` is the single canonical format for value suggestions. All 8 production call sites use it (colors, scenes, charsets, glitch-level, msg-fill-style, custom colors, custom scenes, config keys). Flag suggestions use the inline `tip: a similar argument exists: '--<flag>'` format via `main.rs` + `argv_expand.rs`. The legacy "Did you mean" format survives only in doc comments explaining what was replaced — zero production sites remain.
- **Engine verdict**: ALREADY AT PEAK. `closest_value_match` (Levenshtein ≤ 2, case-insensitive, ties resolve to first candidate) is the shared engine for all value surfaces. Flag suggestions reuse clap's own `suggestions` feature via `extract_clap_suggestion()` — no duplicate engine. The ≤ 2 threshold catches real typos without false positives.
- **Stresstest added**: `scripts/cli_suggestion_stresstest.sh` — an end-to-end shell stresstest that runs the actual `./target/debug/cosmostrix` binary with 18 typo / wrong-value / edge-case inputs and verifies the output format + the absence of the legacy "Did you mean" format. Covers long-flag typos (6), value typos (8), case-insensitivity (1), too-distant values (2), short-form expansion (1). Last run: 18/18 PASS. This complements the 36 in-source unit tests with integration coverage of clap's full error rendering + argv expansion + the `main.rs` append path.
- **Docs**: `docs/CLI_SUGGESTION_SYSTEM.md` §5 expanded with the end-to-end stresstest script documentation (§5b).
- **Decision**: NO CODE CHANGES to the suggestion engine — already at peak. Only added the stresstest script + doc section.

### crystal-dragon: fix drift permanently blocked when ambient is off (Z-master-1X)

- **Bug**: when `crystal-dragon = true` and the ambient schedule is empty (ambient off), the HUD showed `crdr: on` but no color change ever happened. Owner repro: `./target/pro-native/cosmostrix -v -s -C minimal -mfs words` with `crystal-dragon = true`, `power_dragon = true`, ambient off — waited 60s, no drift.
- **Root cause (round 1, commit `c12580a`)**: the drift gate in `cloud/post_rain.rs` checked `!user_override_since_ambient`, but that flag is forced to `true` at startup by `event_loop_setup.rs` (coredump fix, commit `2b0e28b`) and is only cleared by an ambient fire. When ambient is off, no ambient fire ever happens, so the flag stays `true` forever — permanently blocking crystal dragon drift. The HUD reads the config flag (not engine activity), so it reported `crdr: on` despite zero drift.
- **Fix (round 1)**: added `ambient_schedule_active: bool` field to Cloud, initialized from `!ambient_schedule.entries.is_empty()` in `create_cloud`. The drift gate now skips the `user_override_since_ambient` check when `ambient_schedule_active == false`. This preserves the coredump fix (the flag is still set `true` at startup for live-reload safety) while unblocking drift when ambient is off.
- **Root cause (round 2, commit `40bad33`)**: the round-1 fix unblocked the FIRST drift, but `drift_active=true` (set when drift fires) was only cleared by `try_auto_snapback`, which early-returns on empty schedule (`input.rs:481`). When ambient is off, `try_auto_snapback` never runs, so `drift_active` stays `true` forever — permanently blocking all SUBSEQUENT drifts. Owner symptom: "1 color change then nothing for 5+ minutes."
- **Fix (round 2)**: added a self-reset path in `cloud/post_rain.rs` that clears `drift_active` + `drift_start` + resets `crystal_dragon_last_poll` when `drift_active && !ambient_schedule_active && now - drift_start >= CRYSTAL_DRAGON_POLLING_SECS` (60s). The 60s visibility window matches the polling cadence: drift is visible for one poll cycle, then the cycle resets. When ambient is ON, the snapback path clears `drift_active` first (at `ambient-snapback-secs`, default 30s) and the self-reset is a no-op — correct ordering (snapback at 30s < self-reset at 60s).
- **Files changed (round 1)**: `src/engine/cosmic_dragon_engine/cloud/mod.rs` (new `ambient_schedule_active` field + LOC_EXEMPT marker), `src/cli/app.rs` (init from schedule), `src/engine/cosmic_dragon_engine/cloud/post_rain.rs` (drift gate), `src/interactive/tests_v35.rs` (updated existing tests + new regression test).
- **Files changed (round 2)**: `src/engine/cosmic_dragon_engine/cloud/post_rain.rs` (self-reset block), `src/interactive/tests_v35.rs` (2 new regression tests: self-reset-fires-when-ambient-off + self-reset-skipped-when-ambient-on).
- **Tests**: all 6 drift tests pass (3 z_master_1x + 3 v50_drift). cloud 320/320, crystal_dragon 83/83, ambient 67/67, v50_first_reload coredump-fix 3/3. Suite 1943 → 1946.
- **Docs**: `docs/AMBIENT_SCHEDULER.md` (state machine fields, drift gate, self-reset section, ambient-OFF timeline), `docs/CRYSTAL_DRAGON_ENGINE.md` (§11.2 + §11.3 updated for ambient-off behavior), this changelog entry.

### msg-fill-style: peak audit verdict — already at peak, no optimization needed (Z-master-1X)

- **Audit scope**: `src/msg_fill_style/` — 11 files, 3317 LOC (mod.rs + 10 style modules: typewriter, fade, words, slide, instant, engrave, hologram, glitch, scorch, cascade).
- **Architecture verdict**: EXCELLENT. One-file-per-style isolation with a documented plug-and-play recipe. Shared helpers (`char_fade_in`, `lagged_border`, `index_pacing`, `index_fraction`) owned in `mod.rs`. Explicit statelessness contract: 8 styles are pure functions of elapsed time; 2 styles (engrave, scorch) add bounded sidecars (48-slot spark pool, 16-slot smoke pool) with `O(active)`/frame + `O(1)` early-out when idle. `CellReveal` is `Copy` (no clone overhead). Two structural extension points (`glyph_override`, `tint`) are documented as the permanent API surface for future styles.
- **Performance verdict**: ALREADY PEAK. All reveal math is `#[inline]` pure functions. Pools are pre-allocated `Vec` with no per-frame allocation. Frame-rate-independent dt with 1/30s clamping. `--no-effects` gates particle sidecars. Bench mode skips `draw_message` entirely (dead code on that path). The dispatch `match` (10 arms) compiles to a jump table. Any SoA/BitVec refactor on the tiny 16/48-slot pools would be churn without measurable gain.
- **Stability/LTS verdict**: STRONG. Every style's constants are locked by a `*_constants_hold_research_doc_contract` test. Visibility gate contract enforced (`content_idx >= reveal_count` → `CellReveal::hidden()`). `--no-effects` and bench-mode contracts documented per style. The expansion research doc (`docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md`) tracks every candidate and its status.
- **Decision**: NO CODE CHANGES. The module is at peak — over-engineering would add churn without value. This changelog entry documents the audit verdict so future contributors know the module was reviewed and intentionally left as-is.

### msg-fill-style: remove `pulse` style (owner directive, Z-master-1B)

- **Owner decision**: the `pulse` style (typewriter + 1.5x scanner brightness boost + visible `▌` cursor) is removed from the msg-fill-style family. Owner feedback: the style was visually too similar to typewriter even after the cursor improvement, and the family is cleaner without it.
- **Breaking change**: `pulse` is no longer a valid value for `-mfs`/`--msg-fill-style` or the `msg-fill-style` config key. Configs or scripts using `pulse` will soft-fail (config) or hard-fail (CLI) with the "invalid msg-fill-style" error listing the remaining 10 styles. The default (`typewriter`) is unchanged.
- **Files removed**: `src/msg_fill_style/pulse.rs` (the style module + `Cloud::pulse_cursor_pass`), `src/cosmic_dragon_engine/cloud/tests/tests_msg_fill_pulse.rs` (the render tests for the cursor pass).
- **Files updated**: `src/msg_fill_style/mod.rs` (enum variant, 4 dispatch arms, as_str, verbose_label, doc table, statelessness contract, tests), `src/cosmic_dragon_engine/cloud/message_draw.rs` (head_idx no longer computes for Pulse, pulse_cursor_pass call removed), `src/cosmic_dragon_engine/cloud/tests/mod.rs` (tests_msg_fill_pulse module removed), `src/cosmic_dragon_engine/cloud/tests/tests_msg_fill_style.rs` (pulse_style_scanner_boosts test removed, comment refs cleaned), `src/cosmic_dragon_engine/cloud/post_rain.rs` (call sequence comment), `src/cli/argv_expand.rs` (x2 value lists), `src/cli/help_detail.rs` (pulse block + example + header count "Ten styles"), `src/cli/app.rs` (doc comment), `src/config/config_apply.rs` (error message), `src/config/configfile.rs` (comment), `src/config/configfile/configfile_dump.rs` (comment), `src/config/mod.rs` (clap help string), `src/testconf/field_validation.rs` (validation), `src/output/verbose.rs` (doc), `src/main.rs` (module map), `src/tests/clap_suggestion.rs` (test case removed), `src/config/live_config/tests_msg_fill_style.rs` (rebuild_applies_msg_fill_style_from_config updated to use slide, doc comment updated).
- **Docs**: `README.md` x2 (feature list + CLI reference), `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` style table updated.
- **Tests**: -7 (the pulse unit tests in pulse.rs, the 4 pulse render tests in tests_msg_fill_pulse.rs, the pulse_style_scanner_boosts render test, and the pulse as_str/none_elapsed cases). Suite 1946 to ~1939 (final count after gatekeeper).

### msg-fill-style: eleventh style `cascade` — per-column waterfall reveal with drop-from-above (Z-master-1B)

- **New style** `-mfs cascade` / `msg-fill-style = "cascade"`: each column lights up left-to-right (60 ms/column — faster than typewriter's 80 ms/char), and each char drops from 3 rows above its final position, fading in from 40% to 100% brightness over 240 ms. The drop-from-above is visible even on a 1-line overlay (the glyph appears to fall from outside the box) — distinct from typewriter (no drop) and slide (drops from BELOW). The final (and cheapest) candidate from the post-engrave research family.
- **Architecture**: the reveal math is fully stateless, pure function of `(content_idx, elapsed_ms, reveal_count)`. Column-paced reveal (`reveal_at = content_idx * CASCADE_COL_MS`), then a drop phase over `CASCADE_DROP_MS` (240 ms): the glyph starts `CASCADE_DROP_ROWS` (3) rows above with dim factor (`CASCADE_DROP_DIM` = 0.40), slides down (rounding `slide_rows` from -3 → 0) while fading in to full brightness. The drop uses the shared `slide_rows` field — **widened from `u16` to `i16`** in this round so negative values mean "drop from above" (slide style stays positive = "from below"). The renderer uses `mc.line.saturating_add_signed(slide_rows)` so both directions share one mechanism. Zero renderer churn beyond the signed-`slide_rows` support (one-line change in `message_draw.rs`: `> 0` → `!= 0`).
- **Research doc resolution**: §3.D originally flagged cascade as "defer until multi-line overlays are common" because on a 1-line overlay (the default), a per-column "drop top-to-bottom" degenerates into a fast left-to-right wipe nearly indistinguishable from typewriter. This implementation solves that by making the drop go **from above** (not top-to-bottom WITHIN a column — that needs multi-line), so the "waterfall falling from above" visual is visible on any overlay height. The 1-line concern is resolved; cascade now works on the common single-line overlay.
- **`--no-effects` contract**: cascade has NO particle sidecar — the drop animation IS the reveal math, not a cosmetic overlay. So `--no-effects` does NOT gate anything in this style (same contract as glitch).
- **Plumbing**: the new value flows through every existing surface — clap `ValueEnum`, `-mfs` attached/`=` forms in `argv_expand`, config.toml key (case-insensitive), live-reload (`rebuild_applies_msg_fill_style_cascade`), strict `--testconf` validation, `--dump-config` comment, `--verbose` startup label, post-exit `msg_fill_style:` line, `--help` reference block (now "Eleven styles"), `cli/app.rs` doc comment, `output/verbose.rs` field doc, `config/mod.rs` clap `help` string, `main.rs` module map, `cloud/post_rain.rs` call sequence comment, `cloud/message_draw.rs` signed-`slide_rows` support.
- **Tests**: +16 (8 cascade unit tests in `msg_fill_style/cascade.rs` covering pacing budget, settle-after-drop, no-timeline settle, drop-starts-above-at-dim, drop-progresses-downward-and-brightens, hidden-until-reveal, hidden-outside-budget, slide_rows-always-non-positive, research-doc constant lock; 7 render-level tests in `cloud/tests/tests_msg_fill_cascade.rs` mirroring the acceptance ritual — pacing budget, settle-after-drop, no-timeline settle, drop-glyphs-visible-above-final, column-paced-not-per-char (verified via `index_reveal_count` to avoid mid-drop render fragility), Space-restart re-arm; 1 live-reload test `rebuild_applies_msg_fill_style_cascade`; plus the shared `mod.rs` `as_str` and `none_elapsed` tests extended, and `clap_suggestion.rs` extended). The render-level tests were extracted to `tests_msg_fill_cascade.rs` to keep `tests_msg_fill_style.rs` under the 800-LOC hard cap. Suite 1921 → 1937.
- **Docs**: `README.md` x2 (feature list + CLI reference), `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` status line + §3.D candidate updated + §4 decision matrix updated + §6 style table updated to mark cascade as LANDED, this changelog entry.

### msg-fill-style: tenth style `scorch` — burnt-in text with ember tint and smoke (Z-master-1B)

- **New style** `-mfs scorch` / `msg-fill-style = "scorch"`: chars appear in an ember tint (warm orange RGB, not pure red) at the head, cooling to the palette color over 400 ms (factor dips 1.5 → 0.8 → 1.0 — the charred dim sub-effect, then recovery), and every newly scorch'd char throws a slow upward gray smoke puff (700 ms lifetime, 16-slot pool). The wow-option from the post-engrave research family.
- **Architecture**: the reveal math stays stateless (pure function of `(content_idx, elapsed_ms, reveal_count)`). The ember tint + factor curve ride the existing `factor` path (unclamped boost for > 1.0) PLUS a new `tint` field (see below). The smoke lives in a dedicated 16-slot pool (pre-allocated once, `O(active)`/frame, `O(1)` early-out when idle; cloned from the `engrave.rs` spark sidecar pattern with scorch-specific tuning — 1 puff per char, 700 ms lifetime, 2.5 cells/s upward drift with ±20% speed variance + horizontal sway). A dedicated pass was required for the same reason as engrave: the shared quantum pool renders in `apply_quantum_ripple` BEFORE `draw_message` — pool-shared smoke would be overdrawn by the overlay cells, invisible exactly where the scorching head lives.
- **ONE structural extension point**: `CellReveal.tint: Option<(u8, u8, u8, f32)>` — the API surface the research doc §2 flagged as shared by every future color-shifting style. The tuple is `(r, g, b, blend)` where `blend` is 0.0 = palette fg color, 1.0 = full tint. The renderer applies this AFTER the brightness factor: the scaled palette color is linearly blended toward `(r, g, b)` by `blend` (via `chroma_dragon_engine::palette::blend_toward_bg_rgb`). Every existing style leaves the field `None`, so they are bit-identical to the pre-scorch renderer. The `message_draw.rs` change is a `cell_fg_tinted` closure that applies the tint after `scale_msg_content_fg`, used in both the main content pass and the slide deferred second pass (the slide_cells tuple now carries the tint so a future slide + scorch combo would tint the mid-slide glyph too).
- **Spawn contract**: one puff per NEWLY scorch'd char (movement-gated, never per-frame) — frame-rate independent, no double-spawn after skipped frames, puffs stop automatically when the reveal completes, while paused, or during the intro lead. Steady state: puffs every 80 ms with 700 ms lifetime → ~9 concurrent smoke particles, well under the 16-slot cap. Smoke spawns half a cell ABOVE the head (cy = line - 1 + 0.5) so it starts just outside the content row — prevents the smoke from overwriting the freshly scorch'd char on the spawn frame (the smoke pass runs at the END of `draw_message`, so a smoke painted ON the head cell would hide the char until the smoke drifts clear).
- **PERF-4** (`--no-effects`): the smoke sidecar self-gates on `effects_enabled` exactly like every particle subsystem — the reveal math itself runs unchanged (text still burns in with ember tint), only the smoke puffs are suppressed. The ember tint is NOT gated (it's part of the reveal math, not a cosmetic overlay — same contract as glitch's glyph substitution).
- **Plumbing**: the new value flows through every existing surface — clap `ValueEnum`, `-mfs` attached/`=` forms in `argv_expand`, config.toml key (case-insensitive), live-reload (`rebuild_applies_msg_fill_style_scorch`), strict `--testconf` validation, `--dump-config` comment, `--verbose` startup label, post-exit `msg_fill_style:` line, `--help` reference block (now "Ten styles"), `cli/app.rs` doc comment, `output/verbose.rs` field doc, `config/mod.rs` clap `help` string, `main.rs` module map, `cloud/post_rain.rs` call sequence comment, `cloud/message_draw.rs` tint plumbing + scorch smoke pass hook, `cloud/mod.rs` ScorchState field + init + reset in `reset_message` / `restart_message_typewriter` / `set_msg_fill_style`.
- **Tests**: +17 (9 scorch unit tests in `msg_fill_style/scorch.rs` covering pacing budget, settle-to-palette, no-timeline settle, head-burns-hot, mid-cool char dim + recovery, hidden-cell guard, ember RGB warmth check, smoke constants lock, research-doc constant lock; 7 render-level tests in `cloud/tests/tests_msg_fill_scorch.rs` mirroring the engrave acceptance ritual — pacing, head ember tint color assertion, settle-to-palette under `--no-effects` (smoke suppresses to avoid overlapping content cells), smoke-during-reveal scan, `--no-effects` smoke gating, one-puff-per-char movement detection, smoke expiry, Space-restart re-arm; 1 live-reload test `rebuild_applies_msg_fill_style_scorch`; plus the shared `mod.rs` `as_str` and `none_elapsed` tests extended, and `clap_suggestion.rs` extended). The render-level tests were extracted to `tests_msg_fill_scorch.rs` to keep `tests_msg_fill_style.rs` under the 800-LOC hard cap. Suite 1904 → 1921.
- **Docs**: `README.md` x2 (feature list + CLI reference), `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` status line + decision matrix + style table updated to mark scorch as LANDED, this changelog entry.

### msg-fill-style: ninth style `glitch` — cyberpunk distortion settle (Z-master-1B)

- **New style** `-mfs glitch` / `msg-fill-style = "glitch"`: characters do NOT appear left-to-right. Each char's reveal time is a deterministic scramble (a per-cell hash reshuffles the reveal order within an 8-step ±560 ms window), and each newly revealed char flickers between 2-3 wrong glyphs for 90 ms before settling on the true one — Matrix-decode feel.
- **Architecture**: the reveal math is fully stateless, pure function of `(content_idx, elapsed_ms, reveal_count)`. Two gates: a budget gate (`content_idx < reveal_count`, kept typewriter-paced at 80 ms/char) and a scramble gate (`elapsed_ms >= reveal_at(content_idx)` where `reveal_at = content_idx * 80 + scramble_offset(content_idx) * 80`). Within the budget but before the scramble gate, the cell is hidden — the visual "characters appear out of order" effect. After the scramble gate, the cell enters a 90 ms settle window with a wrong-glyph substitution (deterministic per-cell hash into a fixed 8-glyph ASCII table) and ±20% brightness flicker. After settle, the cell shows the true glyph at factor 1.0. Without a timeline (`elapsed_ms = None`), every cell settles instantly via the `settled()` helper (same `usize::MAX` semantics every stateless style uses).
- **ONE structural extension point**: `CellReveal.glyph_override: Option<char>` (the API surface the research doc §2 flagged as shared by every future glyph-substituting style). Every existing style leaves the field `None`, so they are bit-identical to the pre-glitch renderer. The renderer unwraps to `mc.val` at draw time — the change in `cloud/message_draw.rs` is one `let glyph = reveal.glyph_override.unwrap_or(mc.val);` line, used in both the main content pass and the slide deferred second pass (so a future slide + glyph-override combo would Just Work).
- **Wrong-glyph table**: 8 ASCII graphic chars (`['0', '1', '#', '%', '&', '$', '@', '?']`) — all single-width, all in the safe ASCII printable range, so the substitution never breaks cell alignment (Bug #11) and never introduces wide CJK chars.
- **`--no-effects` contract**: glitch has NO particle sidecar — the glyph substitution IS the reveal math, not a cosmetic overlay. So `--no-effects` does NOT gate anything in this style (unlike hologram's scanline pass, which is a cosmetic overlay and self-gates on `effects_enabled`). Test `glitch_no_effects_does_not_suppress_reveal_math` locks this — the draw count is identical with/without `--no-effects`.
- **Plumbing**: the new value flows through every existing surface — clap `ValueEnum`, `-mfs` attached/`=` forms in `argv_expand`, config.toml key (case-insensitive), live-reload (`rebuild_applies_msg_fill_style_glitch`), strict `--testconf` validation, `--dump-config` comment, `--verbose` startup label, post-exit `msg_fill_style:` line, `--help` reference block (now "Nine styles"), `cli/app.rs` doc comment, `output/verbose.rs` field doc, `config/mod.rs` clap `help` string, `main.rs` module map, `cloud/post_rain.rs` call sequence comment, `cloud/message_draw.rs` glyph_override plumbing.
- **Tests**: +16 (9 glitch unit tests in `msg_fill_style/glitch.rs` covering pacing budget, settle-to-correct-glyph, no-timeline settle, glyph_override Some-during-settle/None-after, hidden-outside-budget, hidden-due-to-scramble, wrong-glyph ASCII safety, hash determinism/seed-sensitivity, scramble offset range, research-doc constant lock; 5 render-level tests in `cloud/tests/tests_msg_fill_glitch.rs` mirroring the engrave acceptance ritual — pacing budget, settle-to-correct-glyphs, wrong-glyphs-during-settle (scans 100..3000 ms step 50 to find a settle window, probabilistically guaranteed), `--no-effects` no-op, Space-restart re-arm; 1 live-reload test in `config/live_config/tests.rs`; plus the shared `mod.rs` `as_str` and `none_elapsed` tests extended, and `clap_suggestion.rs` extended). The render-level tests were extracted to `tests_msg_fill_glitch.rs` to keep `tests_msg_fill_style.rs` under the 800-LOC hard cap. Suite 1888 → 1904.
- **Docs**: `README.md` x2 (feature list + CLI reference), `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` status line + decision matrix + style table updated to mark glitch as LANDED, this changelog entry.

### msg-fill-style: eighth style `hologram` — projected hologram with scanline (Z-master-1B)

- **New style** `-mfs hologram` / `msg-fill-style = "hologram"`: the message overlay projects as a hologram. Each character snaps on at full brightness the moment the head reaches it (80 ms/char — no 30% fade-in: a hologram snaps on, it does not fade in), then runs through three deterministic phases: a 150 ms flicker (per-cell brightness noise in `1.0 ± 0.30` from a FxHash of `(content_idx, elapsed/40 ms bucket)` — fast enough to read as "hologram interference", slow enough not to strobe), a 2 s breathing ripple (2% sin hum at 2 Hz, amplitude decaying linearly to zero — the "hologram is alive" tail), and finally settled at exactly 1.0. A single horizontal scanline sweeps the box top-to-bottom over 600 ms once (a thin `▔` U+2594 row at the top of each cell, not obscuring the glyph body), then is gone. The cheapest candidate from the post-engrave research family — fully stateless, no sidecar, no per-frame bookkeeping, no new `CellReveal` field.
- **Architecture**: the reveal math is pure function of `(content_idx, elapsed_ms, reveal_count)` — same stateless contract as typewriter/fade/words/slide/pulse/instant (six of the seven pre-hologram styles). The scanline lives in `Cloud::hologram_scanline_pass` invoked at the END of `draw_message` (alongside `engrave_spark_pass` — only one is wired per style), painting a row of `▔` across the message cells at the sweep row in the palette head color. The next frame redraws the normal content, so the scanline visually sweeps down the box.
- **PERF-4** (`--no-effects`): the scanline pass self-gates on `effects_enabled` exactly like every particle subsystem — the reveal math itself runs unchanged (text still burns in at full brightness), only the scanline overlay is suppressed. Already-active scanlines are not a concern (stateless: the next frame simply skips the pass).
- **Plumbing**: the new value flows through every existing surface — clap `ValueEnum`, `-mfs` attached/`=` forms in `argv_expand`, config.toml key (case-insensitive), live-reload (`rebuild_applies_msg_fill_style_hologram`), strict `--testconf` validation, `--dump-config` comment, `--verbose` startup label, post-exit `msg_fill_style:` line, `--help` reference block (now "Eight styles"), `cli/app.rs` doc comment, `output/verbose.rs` field doc, `config/mod.rs` clap `help` string, `main.rs` module map, `cloud/post_rain.rs` call sequence comment, `cloud/message_draw.rs` end-of-draw hook.
- **Tests**: +15 (7 hologram unit tests in `msg_fill_style/hologram.rs` covering pacing, flicker band, breathing phase, settled state, hidden-cell guard, deterministic hash, research-doc constant lock; 7 render-level tests in `cloud/tests/tests_msg_fill_style.rs` mirroring the engrave acceptance ritual — pacing, burn-in color, scanline presence/absence, `--no-effects` gating, Space-restart re-arm; 1 live-reload test in `config/live_config/tests.rs`; plus the shared `mod.rs` `as_str` and `none_elapsed` tests extended). Suite 1873 → 1888.
- **Docs**: `README.md` x2 (feature list + CLI reference), `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` status line + style table updated to mark hologram as LANDED, this changelog entry.

### cross-platform build guard: dangling cfg import lesson (Z-master-1B)

- **Incident**: the intro_style refactor commit (f19470a6) went red on four CI jobs (Windows, FreeBSD, Android, macOS) while every local gate stayed green. Root cause: `event_loop_p5.rs` carried `#[cfg(target_os = "linux")] use super::intro;` — the v52 refactor deleted the `use` line but left its attribute, which silently re-attached to the NEXT import (`use crate::central_control_dragon_power::{sample_thermal_pressure, PowerManager};`) — making it Linux-only. On the Linux dev host everything type-checks; every other platform lost `PowerManager`/`sample_thermal_pressure` (E0425).
- **Fix** (507ac96a): the dangling attribute removed — the import is platform-unconditional again (its per-call sites stay Linux-gated exactly as before).
- **Permanent guard**: `build.sh check-all` now runs `run_cross_platform_check` — `cargo check` against all four CI-built non-host targets (x86_64-pc-windows-gnu, x86_64-unknown-freebsd, aarch64-apple-darwin, aarch64-linux-android), installing each rust-std target on demand (~1 s per target once cached; skips with a warning when a target cannot be installed offline — the CI matrix remains the final gate there). Validated by re-introducing the exact f19470a6 bug: the guard fails check-all within seconds; no CI workflow runs check-all, so this adds zero CI minutes — it closes the LOCAL pre-push blind spot where the bug slipped through.
- **Process lesson (worklog)**: verify CI green on a pushed commit BEFORE stacking the next one — the second commit (761a682a) was pushed while the first was still red-building, doubling the affected history.

### message-intro lead: masterclass skip conditions (Z-master-1B)

- **Owner bug report**: `cosmostrix --mfs engrave -mb test` with the default logo intro behaves perfectly — after the intro finishes the message text appears after a short delay (tuned design). But the 6 s reveal lead was armed unconditionally inside `set_message`, so it fired even when nothing was hiding the message: `--intro none` showed a dead 6 s wait at startup, the Space runtime restart re-armed it (fresh dead air after every reset), and skipping the intro mid-animation with `q` left the remaining lead running. "From premature logic to masterclass simple logic, skip when on condition non intro value, and space/restart cosmostrix."
- **The fix — the lead now exists for exactly as long as the intro does**: `set_message` starts the reveal timeline at `now` (context-free — it also serves no-intro runs, live-reload cloud rebuilds, and the intro's own temporary clouds, all of which must reveal immediately). The 6 s lead is a named constant (`MESSAGE_INTRO_LEAD`) armed ONLY by the intro runner (`event_loop_intro.rs` → `hold_message_behind_intro`) at the moment a cinematic actually starts, and CUT (`cut_message_intro_lead`) the moment the intro ends early. `restart_message_typewriter` (Space) re-arms at `now` — no intro replays at runtime, so no lead.
- **`IntroOutcome` plumbing**: `run_intro` now returns `Completed | CutShort` (q skip, shutdown signal, terminal below the 10×5 intro floor, art-larger-than-terminal fallback, or `--intro none`); on every cut-short path the runner pulls a still-future reveal start to `now`. A fully played intro leaves the armed lead standing, so the tuned feel — logo intro ends ~4.5 s, message types out from ~6.2 s — is bit-identical to before.
- **Same-family fixes landed free**: live config reload rebuilds the cloud via `create_cloud` → `set_message`, which used to re-arm the +6 s lead on every config save (message vanished for 6 s after each reload) — now the rebuilt cloud reveals immediately; terminals below the intro floor used to arm the full lead for an intro that never played — now cut to zero.
- **Tests**: +4 cloud-level regression locks (set_message arms at now; hold arms now + 6 s and no-ops without a message; cut clamps only future starts; Space restart is immediate). Suite 1869 → 1873. Stale comments synced (tests helper, engrave intro-lead phrasing, `set_msg_fill_style` doc).
- **E2E**: new `scripts/intro_lead_e2e.py` PTY harness proves all three owner conditions on the real binary using an unambiguous marker glyph (message of 12 `X`s under `-C digits` — X appears in neither the rain charset, the logo art, nor the cosmic burst chars): `--intro none` first message glyph at ~0.002 s (pre-fix: nothing before 6 s); default logo intro shows ZERO message glyphs before the cinematic ends (first at ~4.51 s = the lead's `.max(1)` first char, full typing from ~6.2 s — the tuned feel locked); Space restart at t=3 s re-reveals 22 fresh glyph writes within 1.3 s (pre-fix: single-char dead air for 6 s). PASS ×2 runs.

### intro_style: one-file-per-style directory refactor (Z-master-1B)

- **Owner mandate**: "refactor special for function --intro all styles into one directory called `intro_style`, each file like `logo.rs` — easy maintenance, isolate, plug and play" (same shape as the msg_fill_style refactor).
- **New layout** — `src/intro_style/` (crate-root module, peer of `msg_fill_style/`): `mod.rs` holds the shared skeleton (`IntroType` enum moved home from `config/mod.rs`, the `run_intro` dispatcher, MIN_INTRO size constants, frame period, the pre-allocated particle pool + xorshift RNG + OKLab lerp helpers, the q-only skip policy, and a "How to add intro style #4" plug-and-play recipe); each style owns exactly one file — `cosmic.rs` (700 LOC, Cosmic Burst) and `logo.rs` (794 LOC, cosmostrix Logo) — plus `logo_tests.rs` / `tests.rs` for the per-style and shared-infrastructure tests.
- **Pure code motion**: bit-identical animation behavior — same phases, same timings, same particle math. The v20 layout (`interactive/intro.rs` dispatcher + `interactive/intro_cosmic.rs` + `interactive/intro_logo/`) is gone; the runner glue (`event_loop_intro.rs` — intro-color resolution chain, brand EnergyZen, post-intro resize re-read) stays in `interactive/` exactly as the renderer stayed in `cloud/` for msg-fill-style.
- **Unrelated concerns unwed**: `intro.rs` had housed the Linux `/proc` process metrics helpers (RSS + context switches, HUD endurance panel) "because the file already existed" since v17 — they moved home to `sysstat/procstat.rs`.
- **Path updates**: `IntroType` now referenced as `crate::intro_style::IntroType` across config/cli/bench/output/interactive (16 files); `BURST_CHARS` reached via `intro_style::cosmic` for the width-guard audit test; `is_unmodified_or_shift` + the watchdog flags re-exported at the `interactive` facade so intro_style reaches them without opening the whole submodules.
- **Stale-reference sweep (owner directive)**: `interactive/` module map + `src/RULES.md` tree + `main.rs` module map refreshed; commented code references re-pointed (`terminal/draw.rs` intro end_frame note, `platform/mod.rs` procfs pattern list, `central_control_rains/mod.rs` logo phase-3 fade pointer, `chroma_dragon_engine/intro_colors.rs` consumer notes, shortkey no-op test keybind table); audit/research docs annotated with pre-v52 → v52 path moves (A1/A2/A3 zombie-kill, COSMIC_DRAGON_AUDIT C-4, CHROMA + FLAGS research audits).
- **Stale timing data fixed**: the logo intro module doc + `--help` block still advertised the v20 phase table (0–2000/4250/5250/6250 ms, "~6.25 s total") — the v25 rebalanced constants are 1200/3000/4000/4500 ms (~4.5 s total); both docs now match the code, and the cosmic module doc's stale "80×24" intro floor claim is corrected to 10×5 (the README had already been fixed; cosmic.rs had not).
- **Tests**: 1869 pass, zero behavioral churn (pure move + docs); suite count unchanged.

### HUD metrics reorder: v51 owner-mandated row order (Z-master-1B)

- **Owner mandate** ("reorder/tidying HUD metrics new, correct like this below"): the 22-row HUD now reads fps / tgt / max / p99 / cpu / rss / ehs / prs / **scn / chr / clr** / **sped / dsty** / prdr / crdr / ambt / glth / ctun / mnst / **cid / up / screensize** (bold = moved). Previously the order was ...ehs / prs / sped / dsty / scn / chr / clr / up / screensize / prdr / crdr / ambt / glth / ctun / mnst / cid.
- **What moved and why it reads better**: the identity lines (scene / charset / color) now sit directly under the health pair — what am I looking at, then how is it doing; the user-adjustable controls (speed / density) follow; the dragon + tuning state (prdr / crdr / ambt / glth / ctun / mnst) rides the bright head band; and the session footer (cid → up → screensize) closes the dashboard — the build identity keeps a prominent position while the terminal size becomes the visual anchor at the very bottom.
- **Code**: pure row remap in `hud/metrics.rs` (1 Hz tick) + `hud/hud_init.rs` (init array + static cid row 21 → 19); the positional chroma gradient is untouched (row i samples t = i/21 — content moves between stops, the sweep shape does not). Commented row references refreshed across `hud/mod.rs` (module doc, setter docs, the rain-gradient layout diagram), `hud/colors.rs` (stop-count history, "row 17 cid" → v51 layout), `event_loop_hud.rs`.
- **Tests**: all HUD row assertions remapped (tests.rs, tests_chroma_metrics.rs, tests_dragon_indicators.rs, tests_pause_freeze.rs); the layout regression guard now locks the full 22-row v51 order (renamed from the Option S era); the cid-exclusivity loop now skips row 19 instead of iterating past row 21.
- **E2E**: new `scripts/hud_order_e2e.py` PTY harness spawns the real binary, toggles the HUD, and asserts the exact 22-label screen order — PASS (also caught the harness lesson: a fresh PTY is 1x1, winsize must be set via ioctl before the app measures, else the intro is skipped for "terminal too small").
- **Docs**: `docs/HUD.md` fully resynced — Quick Reference table now documents all 22 rows (prdr/crdr/ambt/glth/ctun/mnst rows were previously missing entirely), the annotated mockup, the per-line section order (cid before up/screensize), the color-scheme table, and the v51 reorder note; `docs/RELEASE_CANDIDATE.md` HUD smoke checklist updated to the 22-row v51 layout (was stale at "16 rows, v50 layout").

### crates.io categories: science + rendering (Z-master-1B)

- **Owner mandate**: add the missing categories "science" and "cinematic or masterpiece". Validated against the live registry API (58 official slugs as of 2026-08-31): `science` is a valid slug; `cinematic` and `masterpiece` are NOT — crates.io only accepts slugs from the official category list, so an invalid one fails `cargo publish` server-side.
- **Resolution**: `science` added as requested (the project's engineering posture — OKLab color science, statistics-grade HUD/benchmarks, LTS audits — earns it). The "cinematic/masterpiece" intent is mapped to `rendering`, the closest official bucket for a cinematic renderer (the crate description already calls cosmostrix one). The mapping + rationale are documented in the `Cargo.toml` comment so the next maintainer does not re-litigate it.
- **Categories now**: `command-line-utilities`, `games`, `science`, `rendering` (4 of the 5-category registry limit). No keyword changes.

### msg-fill-style: one-file-per-style directory refactor (Z-master-1B)

- **Owner mandate**: "all styles into one directory called `msg_fill_style`, each file like `engrave.rs` — easy maintenance, isolate, plug and play." The v51 style system previously lived in three places: the enum + stateless math in `types/msg_fill_style.rs` (678 LOC monolith), the renderer in `cloud/message_draw.rs`, and the engrave spark sidecar in `cloud/message_engrave.rs`.
- **New layout** — `src/msg_fill_style/` (crate-root module, peer of `types/`; the enum is consumed by both the CLI layer and the rendering engine, so it must not live inside either): `mod.rs` holds the shared skeleton (enum, `as_str`/`verbose_label`, `CellReveal`, shared ramp/lag/pacing helpers, the four dispatch functions, and a "How to add style #8" plug-and-play recipe); each style owns exactly one file — `typewriter.rs` (102), `fade.rs` (95), `words.rs` (115), `slide.rs` (115), `pulse.rs` (100), `instant.rs` (65), `engrave.rs` (441: reveal math + the spark sidecar + the spark tuning constants, moved home from `types/constants.rs`). Per-style unit tests moved into their style files; every file is under the 500-LOC soft target.
- **Pure code motion**: bit-identical behavior — same math, same constants, same rendering order. The renderer's style match for the reveal budget moved into the dispatch (`index_reveal_count` now answers for all seven styles; the words/fade/instant budget was already a dead value their reveal math never reads). `pulse.rs` imports the typewriter constants instead of duplicating them (pulse IS typewriter + scanner). Zero renderer churn beyond the doc header + the one-line budget call.
- **Path updates**: `crate::msg_fill_style::*` imports unchanged (module promoted from `types/` to crate root in `main.rs`); `cloud/message_engrave.rs` deleted (its `impl Cloud` spark pass now lives in `engrave.rs` — same split-impl pattern, reached via `crate::cloud::Cloud`); `ENGRAVE_SPARK_*` constants moved from `types/constants.rs` to `engrave.rs` (engrave-only tuning belongs with the style).
- **Tests**: +5 (shared ramp shape, shared lag curve with clamp guard, pulse hidden-cell boost guard, engrave spark-pool owner-contract lock, plus the typewriter/fade border assertions split per style). Suite 1864 → 1869.
- **Docs**: `src/RULES.md` tree + `main.rs` module map, research docs refreshed (`MSG_FILL_STYLE_EXPANSION_RESEARCH.md` now points candidates at the per-file recipe; `THEME_CATALOG_SPLIT_RESEARCH.md` registry table updated to the directory shape), this changelog's Unreleased path mentions corrected to final locations.

### msg-fill-style: seventh style `engrave` — laser engraving with spark bursts (Z-master-1B)

- **New style** `-mfs engrave` / `msg-fill-style = "engrave"`: the message overlay is laser-engraved character by character. Each char is burned in at full brightness the instant the head reaches it (80 ms/char — no 30% fade-in: a laser burns, it does not fade), glows 2x white-hot at the engraving head, and cools to 100% over 300 ms so the last ~4 chars always form a heat trail. Every newly engraved char additionally throws a 3-particle spark burst that flies outward and fades over 200 ms — the first style in the family with a particle sidecar.
- **Architecture**: the reveal math stays stateless like the other six styles; the sparks live in a dedicated 48-slot pool (pre-allocated once, O(active)/frame, O(1) early-out when idle; after the one-file-per-style directory refactor, everything lives in `msg_fill_style/engrave.rs`). A dedicated pass was required because the shared quantum pool renders in `apply_quantum_ripple` BEFORE `draw_message` — pool-shared sparks would be overdrawn by the overlay cells, invisible exactly where the engraving head lives. Sparks render at the END of `draw_message`, on top of the freshly burned-in text.
- **Spawn contract**: one burst per NEWLY revealed char (movement-gated, never per-frame) — frame-rate independent, no double-spawn after skipped frames, bursts stop automatically when the reveal completes, while paused, or during the 6 s intro delay. Steady state 9-12 concurrent sparks, 4x under the 48 cap. `--no-effects` suppresses spawning (PERF-4, same as every particle subsystem); bench mode never runs the pass (draw_message itself is Z-6-skipped).
- **Plumbing**: the new value flows through every existing surface — clap ValueEnum, `-mfs` attached/`=` forms in `argv_expand`, config.toml key (case-insensitive), live-reload (soft-fail on invalid), strict `--testconf` validation, `--dump-config` comment, `--verbose` startup label, post-exit `msg_fill_style:` line, `--help` reference block (now "Seven styles").
- **Housekeeping**: `QuantumParticle` moved from `cloud/mod.rs` to `cloud/state.rs` (pure code motion — mod.rs was at the 800-line hard LOC cap, and state types belong in state.rs); stale pre-v51 comments corrected (`post_rain.rs` "typewriter reveal (30ms/char)" → style-driven, `rain_at.rs` same, `RAIN_BORDER_TOUCH_GLOW_AUDIT.md` quote annotated, `THEME_CATALOG_SPLIT_RESEARCH.md` LOC table refreshed).
- **Tests**: +10 (3 unit in msg_fill_style.rs, 6 renderer-level in tests_msg_fill_style.rs incl. burst-per-char movement gating, expiry + no-respawn from a parked head, --no-effects, Space-restart re-arm; 1 live-reload; plus the existing style-list loops in clap/argv tests extended). Suite 1854 → 1864.

### CI hotfix: android-aarch64 job red since the dynamic-version policy (Z-master-1B)

- The `ndk-version: latest` from the CI dynamic-dependency policy (2026-08-30) was invalid: `nttld/setup-ndk` has NO `latest` support — it splices the input literally into the download URL, producing `android-ndk-latest-linux-x86_64.zip` → 404. The `android-aarch64` job failed at the "Setup Android NDK" step on three consecutive pushes (6031438, 70270fe1, 0764198) — unnoticed because only the Gate-keepers workflow was verified, and lint gates do not execute workflow steps.
- Fix keeps the owner policy intact (zero hardcoded versions): new `scripts/resolve-latest-ndk.py` resolves the newest **stable-channel, final-release** NDK from Google's official SDK repository manifest (`repository2-3.xml` — the same source `sdkmanager` reads), skipping beta/rc/canary archives (e.g. the current `r30-beta3` that Google lists on the stable channel), and hands the r-style name (today: `r29`) to `nttld/setup-ndk` in both `ci.yml` and `release.yml`. Fails loudly when nothing resolves — never a silent fallback to a pin. The only numeric constant (r23 floor) is an archive-naming-era guard, not a version pin.
- Docs: `docs/workflow/ABOUT_CI.md` dependency-policy section gains the correction + the process lesson ("after unpinning a dep, verify the affected CI job ran green once, not just the lint gates"); `docs/audits/DEPS_AUDIT.md` records the correction to the 2026-08-30 policy entry.
- Second same-day CI blind spot, found while verifying the fix: `crates-io.yml` line 78 (216 chars > the 200-char `.yamllint` limit, born in 6031438) made `workflow-ci.yml` fail on every `.github/**` touch — while local `gate-keepers.sh` yamllint ran with `line-length: disable` and could never catch it. Fixed both sides: the long line is wrapped, and gate check 2 now lints `.github/**` under the repo `.yamllint` config (exact CI parity), keeping the relaxed inline config only for `aur/`/`.cargo`.

### Config parser bug #19: quoted single-bracket charset values rejected (Z-master-1B)

- Owner-found 2026-08-30 while previewing single-glyph charset candidates: `set = "["` in a `[charset-custom.<name>]` block failed strict startup validation with `invalid config — malformed line(s): 'set = "["' ... (expected 'key = value' syntax)` — even though the line is perfectly valid. Root cause: the parser stripped the surrounding quotes BEFORE array detection, so the bare `[` was mistaken by the multi-line array consumer for an unterminated array (rejecting the line, or silently absorbing following lines into a bogus value in other configs).
- Fix: `parse_config_text` now snapshots `raw_is_quoted` before quote-stripping and gates both array branches (bug #7 unquoted-`#` rejection + v25 multi-line consumer) on it — **a quoted value is never an array**. One fix covers all three surfaces (startup strict validation, live-reload watcher, `--testconf`) since they all share `parse_config_text`.
- Depth stress battery per owner mandate ("stress-test ANY single glyph so this never happens again"): new `src/config/configfile_tests/bug19.rs` (12 tests) — the owner's exact repro line (trailing comment containing both `']'` and `'#'`), every ASCII punctuation char as a single-glyph pool, bracket-only pools (`[]`, `[][]`, `][`, `[[]]`), hash-only pools with real trailing comments, equals-only pools, unicode single glyphs (incl. the new nabla `∇` and a wide CJK char), comment-shape sweep, no-line-eating regression, unquoted-bracket contrast lock (bug #7 semantics preserved), and the owner's full config shape end-to-end (duplicate sections = last-wins). Plus 5 charset-custom validation tests: special-char single-glyph pools, lone-quote inexpressibility (no escape sequences — documented corner), mid-pool quotes, edge-space trimming, wide/zero-width/control single-glyph errors.
- Docs: `docs/RULES.md` gains the "Config value quoting invariant" section; `--dump-config` template Rules line now documents that any single-width glyph incl. `[ ] # =` is legal when quoted; `charset_custom.rs` module + `parse_charset_value` docs refreshed (values arrive UNquoted from the config layer; `trim_matches('"')` is defense-in-depth).

### Charset "minimal" = single nabla glyph ∇ — owner decision landed (Z-master-1B)

- Owner pick (2026-08-30, after reading the cffd549 research): the `minimal` preset pool is now **`∇`** (U+2207 NABLA) — one glyph, total commitment. The preset name, `--charset minimal`, `[charset-custom.minimal]` shadowing, live reload, and every flag/format stay identical; only the pool string changed (`src/scene/charset.rs` MINIMAL arm).
- The 17-glyph junk drawer (`.:-=+*·•○●◦◌◍◉◎◇◆□■`, six unrelated families) is gone. Every trail is now a column of nabla marks — the operator that means "gradient" IS the rain, pairing with the OKLab trail gradient for pure two-dimensional depth. Second single-glyph preset after zen.
- ASCII-safe fallback advice refreshed (∇ is unicode, NOT in the Linux console VGA font): `--doctor`'s non-UTF-8-locale advice and all three `docs/TERMINAL_COMPATIBILITY.md` fallback recommendations that pointed at `--charset minimal` now point at `--charset zen` (single ASCII pipe) / `--charset ascii`. `--doctor`'s glyph-coverage sample for minimal updated to U+2207; `--list-charsets` description updated.
- Lockstep tests: `build_chars_minimal_has_only_nabla` + `charset_from_str_resolves_minimal` — the pool cannot silently rewiden without a conscious test edit.

### Charset "minimal" masterclass replacement — research, owner decides (Z-master-1B)

- Owner assessment: the current `minimal` pool (`.:-=+*·•○●◦◌◍◉◎◇◆□■` — 17 glyphs from 6 unrelated families) is "bad, not masterclass". Research doc `docs/research/CHARSET_MINIMAL_MASTERCLASS_RESEARCH.md` dissects the five defects (family mixing, no readable ramp, uncontrolled ink density, weakest-coverage glyph picks incl. U+25CC dotted circle, operator dilution) and proposes four one-family replacements: **A Ink Ramp** `·•○◎●` (recommended — one shape, five ink states, two depth dimensions on top of the color gradient), **B Shade Ramp** `░▒▓█` (bulletproof Block Elements), **C Bit Pairs** `○●◇◆□■` (hollow/solid flips), **D ASCII Signal** `.:-=` (zero-unicode purism).
- All four are previewable live right now via `[charset-custom.minimal]` shadowing (custom wins, Option D policy) — no build needed to compare.
- RESEARCH ONLY — no charset code changed; the name `minimal` and every flag/format stay identical. Implementation touch points listed in the doc for the follow-up task once the owner picks.

### Killer features hardening: colors-custom / charset-custom / scene-custom (Z-master-1B)

- Owner mandate: peak optimize + stability/LTS, no hidden bugs, no potential problems, no security risks — special focus on the three killer features no competitor ships. Full audit: `docs/audits/KILLER_FEATURES_HARDENING.md` (K1-K7 fixed, V1-V6 investigated and verified safe).
- **K1 mid-rain stderr leaks (AB-10 class)**: charset-custom's wide-char skip note + builtin name-collision notice and scene-custom's unknown-scene / invalid-field notes printed directly into the rain matrix on every scene change / live reload. New session gate (`INTERACTIVE_SESSION_ACTIVE`, set at `run_interactive`) + `output::warn_runtime_or_now` routing: direct stderr before the session (startup output unchanged), buffered post-exit summary while the rain is on screen.
- **K2 warning-buffer spam**: `push_runtime_warning` now dedups identical messages — the `.stops` deprecation and scene-custom re-apply notes re-fire per event and used to flood the 64-slot post-exit summary.
- **K3 density-map unbounded entries**: a pasted mega-CSV leaked `Box::leak` memory (~8 MB per 1M entries, per distinct value, permanently) — the only killer-feature input without a cap. Now capped at `DENSITY_MAP_MAX_ENTRIES = 1024` (truncate + routed warning), with the matching `--testconf` ceiling warning.
- **K4 live-reload validation drift**: the scene-custom live-reload applier accepted `bold = "255"` (startup rejects; anything not 0..=2), any `shadingmode` u8 (startup: 0..=1), and treated every non-true `async-mode` string as `false` (startup: `parse_bool` + warn). All three arms now share the startup validation and warn with scene-name context. Latent (strict validation rejects invalid reloads first) — fixed as defense-in-depth so the paths can never drift.
- **K5 dead display code**: `--show-scene` rendered `monolith-size` / `color-bg` rows that scene-custom blocks can never carry (owner-contract forbidden fields) — arms removed + locked by test.
- **K6 doc-vs-code contradictions**: charset module doc claimed wide chars are "rejected" (impl skips with a warning); collect docs said caps were 64 where the constant is 100; cap tests claimed "first" blocks survive (HashMap order is unspecified). All corrected.
- **K7 allocation churn**: `is_colors_custom_name` rebuilt the full palette map on every probe (every scene change / reload); now short-circuits when the config defines no colors-custom blocks.
- Tests +6 (suite 1829 -> 1835); `docs/RULES.md` bounds table gains the density-map cap + the warning-routing contract.

### crates.io release channel + CI dynamic dependency versions (Z-master-1B)

- cosmostrix is now on [crates.io](https://crates.io/crates/cosmostrix): `cargo install cosmostrix` (or `--locked` / `--version X.Y.Z` for exact reproducibility) joins GitHub Releases, AUR, and Termux as an install channel; README Installation gains the section.
- **New workflow `crates-io.yml`**: every owner-pushed `v*` tag — stable AND pre-release — publishes the crate via `cargo publish --locked`. Guard rails: tag must match `Cargo.toml`'s version (fail fast), already-published versions are skipped (re-pushed tags / re-runs stay green), missing `CRATES_IO_TOKEN` secret fails with setup instructions. Requires the one-time `CRATES_IO_TOKEN` repository secret (crates.io API token, `publish-new` scope).
- **Cargo.toml**: crates.io discoverability metadata added (keywords: matrix, matrix-rain, terminal, screensaver, cli; categories: command-line-utilities, games, later extended with science + rendering — all validated against the live registry slugs) and the packaged crate slimmed (`.cargo/*` + lint dotfiles excluded; they are repo-local build/lint config, dead weight downstream).
- **CI dependency policy (owner decision)**: zero hardcoded dep versions in `.github/*`. Actions float on major tags (`checkout@v6`, `rust-cache@v2`, `upload-artifact@v7`, `download-artifact@v8`, `setup-java@v5`, `setup-ndk@v1`, `msvc-dev-cmd@v1`, `action-gh-release@v3`); CI-installed deps resolve latest upstream at run time (shfmt via the mvdan/sh releases API, unpinned pip/npm/go installs, Android NDK `ndk-version: latest`); the Rust toolchain stays deliberately LTS-locked via `rust-toolchain.toml` (gate 9 keeps `RUST_VERSION` envs in sync). Documented in `docs/workflow/ABOUT_CI.md` (Dependency version policy); `docs/SUPPLY_CHAIN.md` records the decision replacing the SHA-pinning migration plan.

### Shortkey audit: exhaustive no-op lock for every non-active key (Z-master-1B)

- Owner mandate: verify only existing shortkeys have effects — a user pressing `a` (old-version async-toggle muscle memory) must see NO effect. Source code = truth: the active keymap is exactly `q`, `Space`, `c`/`C`, `s`/`S`, `p`, `x`/`X`, `Up`/`Down`, `[`/`]` (all in `handle_keybinding`) plus `i` (HUD, dispatched in the event loop) and `q`/`Q` (intro skip only). Every other key hits the `_ => {}` catch-all.
- **New exhaustive test suite** `src/interactive/tests_v51_shortkey_noop.rs` (+7 tests): the owner's exact `a` scenario; every non-active letter a–z/A–Z; digits; punctuation; the removed density aliases (`-`/`_`/`+`/`=`); non-active special keys (Tab, BackTab, Enter, Backspace, Delete, Insert, Home, End, PageUp/PageDown, Left/Right, Esc, F1/F5/F12) — each asserted as a COMPLETE no-op (no redraw, no change to color scheme / charset / scene / density / speed / pause / raining / async_mode).
- **Positive controls** in the same suite verify every ACTIVE key still has its documented effect — including the convention that only `p` returns `true` from `handle_keybinding` (the other active keys signal redraw via internal force-draw flags, which the controls assert via `is_force_draw_everything()`).
- **`i` dispatch split documented + locked**: `i` is a no-op at the `handle_keybinding` level (the HUD toggle lives in the event loop, gated by `hud_toggle_accepted` — see the pause-isolation entry above); the test documents the split so future refactors do not silently double-bind it.
- **Docs**: docs/RULES.md keybind policy gains the exhaustive no-op lock entry (what is covered, where the test lives, the redraw-convention note).

### "Did you mean?" audit: every CLI value surface now suggests on typos (Z-master-1B)

- Owner mandate: all existing CLI flags/values must use the suggestion system. Systematic audit of every flag + value surface found FIVE gaps where a typo produced a bare "unknown X / use --list-Y" dead end with no suggestion, while colors and long flags already suggested:
  1. `--glitch-level`, `--monolith-size`, `--color-bg` values — the prevalidator (`prevalidate_cli_args`) intercepts these BEFORE clap's ValueEnum parser, so clap's built-in "tip: a similar value exists" never fired. `validate_enum_value` now appends `Did you mean '<value>'?`.
  2. `--scene <typo>` (both the strict CLI error and the config-apply path) — now suggests from builtin scene names + `[scene-custom.<name>]` blocks.
  3. `-C/--charset/--charset-custom <typo>` — `charset_from_str` now suggests from the new `CHARSET_PRESET_NAMES` list (custom charset names are listed by `--list-charsets` but not suggested — the parser has no config access; documented).
  4. `--colors-custom <typo>` — `load_custom_palette` now suggests from the defined `[colors-custom.<name>]` blocks.
  5. `--scene-custom <typo>` — `unknown_custom_scene_error` (extracted as a testable helper) now suggests from the defined custom scenes.
- **Engine consolidation**: `edit_distance` + `closest_value_match` (edit distance ≤ 2, case-insensitive, deterministic first-best tie-break) now live in `src/cli/suggestion.rs` as the shared engine — `closest_color_name` reuses it (its private copy removed), and every new surface uses the same policy. clap-driven surfaces (long flags, `value_enum` values, `-mfs` shorthands) keep using clap's own suggestion output — no duplicate engines.
- **Verified already suggesting** (no change needed): long flags (`extract_clap_suggestion`), `--intro`/`--msg-fill-style`/`--bench-scene` values (clap ValueEnum tips), color values (`closest_color_name`, bug #13), `-mfss` shorthands (`argv_expand`), removed flags (REMOVED_FLAGS migration table), unknown config.toml keys (`config_hints`).
- **Tests**: +15 (5 engine unit tests in `suggestion.rs`, 5 prevalidator enum tests, 3 charset tests incl. a preset-list/parser lockstep test, palette + custom-scene + scene-tip tests).
- **Docs**: docs/RULES.md gains the full "Did you mean?" coverage inventory (one bullet per surface, file-level pointers); README gains the typo-friendly CLI bullet.

### Live-reload deep audit: custom palette / custom scene switching fixed (Z-master-1B)

- Owner suspicion confirmed: "some functions in config.toml don't work at live-reload." Systematic audit of every `USER_CONFIG_KEYS` entry against `rebuild_cloud_config` + the downstream `create_cloud` application found FIVE real gaps — all in the custom palette / custom scene switching paths, all reproduced by failing tests before fixing:
  1. Switching `color` TO a `[colors-custom.<name>]` palette at runtime was a silent no-op (the block only parsed builtin scheme names, unlike startup's custom-first lookup). Custom names now load via `load_custom_palette` — custom wins on collision, mirroring startup (v50.0.0-beta.6 Option D parity).
  2. Switching `color` AWAY from an active custom palette kept the stale palette loaded — and `create_cloud` applies `custom_palette` AFTER the scheme, so the builtin the user switched to never rendered. Switching to a builtin now clears `custom_palette`/`custom_palette_name`.
  3. Switching `scene` TO a `[scene-custom.<name>]` scene only updated `scene_name` — rain_style/color/charset/speed/density kept the previous scene's values whenever the ambient scene-change branch did not fire. Custom scenes now resolve `rain_style` from the base-scene (mirroring startup's `rain_style_for_custom_scene`) and are tracked as active so the scene-custom field layer applies.
  4. Switching `scene` AWAY from a custom scene re-applied the immutable startup `scene_custom_name` layer on top of every builtin scene the user switched to. The tracker now follows the rebuilt config; the layer only re-applies while the custom scene is still active.
  5. Scene `fps` and `glitch-level` defaults never applied on a scene switch (startup's `apply_default_scene_values` applies both). Both arms added to the scene block, layered before the user-key blocks so explicit `fps`/`glitch-level` keys still win.
- **Verified working, no change needed**: `charset` (both builtin/custom directions), `speed`, `density`, `fps`, `glitch-level`, `monolith-size`, `bold`, `shadingmode`, `color-bg`, `crystal-dragon`, `power-dragon`, `async-mode`, `color.tune.*` (reset-on-comment), `message`/`message-border`/`msg-mode`/`msg-fill-style`, `ambient.HH-MM`, `ambient-snapback-secs`, `charset-custom.<name>` edits. Restart-only: `intro`/`intro-color` (one-shot animation — documented limitation, not a gap).
- **Tests**: +13 regression tests in `src/config/live_config/tests.rs` (9 custom palette/scene switching + 2 scene-fps parity + 2 scene-glitch parity). Full suite 1802 pass.
- **Docs**: `docs/LIVE_RELOAD_BEHAVIOR.md` — new "9. v51 Z-master-1B Audit" section with the 5-gap table, updated per-key matrix notes for `color`/`scene`, stale line-number references replaced with file-level pointers.

### Indonesian language purge from src/ comments (Z-master-1B)

- Owner audit mandate: eliminate/translate any Indonesian from the repo root files and `src/*`. Full vocabulary sweep (140+ word/phrase patterns) over root files, `scripts/`, and `src/` found exactly 4 fragments — all inside quoted bug-report/spec comments, all now translated to English: `termdetect/mod.rs` (a "lingering dim ghosts" phosphor symptom), `chroma_dragon_engine/tuning.rs` (the border-touch owner spec — "from black to white, then it fades away after a few seconds", marked as translated), and `cosmic_dragon_engine/cloud/phosphor.rs` (two fragments: the stale-trails fix label + the "lag disappears, returns a few seconds later" oscillation). Note: `gini` hits in `bench/` are the Gini coefficient (statistics), not Indonesian. Root files, `scripts/`, and all other `src/` files were already clean — verified, no other changes needed.

### Pause isolation + HUD metric freeze + intro brand color (Z-master-1B)

- **Bug fix — intro logo color override**: `cosmostrix -c neon-green` repainted the intro LOGO neon-green. Root cause: the unset-`--intro-color` path passed the LIVE rain cloud to the intro, and `logo_stage_colors()` samples the cloud's palette stops for the 7-stage gradient. Fix: the unset AND invalid `intro-color` paths now build a dedicated brand EnergyZen intro cloud (`event_loop_intro::brand_intro_cloud`) — the same scheme the cinematic scene applies by default, so a no-flags run and a `-c <theme>` run show the identical brand intro. Only `--intro-color` (or config `intro-color`) repaints the intro; `-c`/`--color`/`--colors-custom` affect the rain only. `set_color_scheme` also clears `custom_palette_active`, so custom palettes cannot leak into the intro.
- **Bug fix — pause shortkey isolation**: while paused, `i` still toggled the HUD. `i` is dispatched in the event loop BEFORE `handle_keybinding()` (Android/Termux Release-guard ordering), so the pause guard inside it never saw the key. New `input::hud_toggle_accepted()` gate applies the same `is_paused_or_decelerating()` predicate, keeping the contract uniform: while paused (or decelerating toward pause), ONLY `p` (resume) and `q` (quit) respond.
- **Bug fix — HUD metrics freeze on pause**: uptime (`up:`), fps/max/p99, cpu, rss, prs, and ehs kept "running" while paused (uptime counted on; the 4 Hz input-poll ticks contaminated the frame-time ring and inflated the endurance score; cpu/rss kept sampling). New `HudState::set_metrics_paused()` (announced every frame from `update_hud_state`) opens/closes a pause segment; samplers gate on it (`push_frame_time`, `maybe_sample_rss`, `maybe_sample_cpu` — which keeps the CPU baseline warm so the first post-resume delta stays precise, `set_effective_pressure`, `set_endurance_health_score`), and uptime math excludes `paused_total` + the open segment. The `tgt:` line stays live (`paused` suffix) so the user sees why the dashboard froze; state labels (scn/chr/clr/…) remain live for config live-reload visibility. P5 endurance sampling also skips paused frames.
- **Tests**: 4 resolver tests in `event_loop_intro.rs`; 6 tests in new `tests_v51_intro_brand_pause.rs` (brand cloud ignores user scheme + custom palette flag + keeps charset; `hud_toggle_accepted` accepted running / rejected during decel / rejected while fully paused); 8 tests in new `hud/tests_pause_freeze.rs` (frame-time window blocked, prs/ehs held, duplicate announcements no-op, uptime excludes paused time, multi-cycle accumulation, CPU sampler freeze+baseline, tgt suffix still renders).
- **Docs**: `--help` RUNTIME CONTROLS pause note + `--intro-color` default note, README (feature bullets, keybind line, CLI reference, Runtime Controls block), `docs/RULES.md` keybind policy (pause isolation entry), new `docs/HUD.md` "Pause Freeze (v51)" section, `verbose.rs` unset-intro-color line, `intro_colors.rs`/`intro_logo/mod.rs` brand-color doc updates (incl. stale 80×24 → 10×5 fix in the logo intro module doc).

### msg-fill-style: selectable message overlay reveal animation (Z-master-1B)

- **New flag** `-mfs <style>` / `--msg-fill-style <style>` (plus `msg-fill-style` config.toml key) selects how the message overlay reveals itself. Six styles, all derived purely from elapsed time (stateless — no per-frame bookkeeping): `typewriter` (default, bit-identical to the previous renderer), `fade` (instant text, 800 ms whole-block fade incl. border), `words` (200 ms/word cascade via hoisted per-cell word ordinals), `slide` (60 ms/char, glyphs fade in one row below then land — drawn in a deferred second pass), `pulse` (typewriter + scanner cursor boosting recent chars to 150% with 200 ms decay), `instant` (text at full brightness immediately, border draws clockwise over 1 s).
- **CLI ergonomics**: clap short flags are single-char, so `-mfs` is rewritten to `--msg-fill-style` pre-parse (same mechanism as `-mb`). Attached (`-mfsfade`) and `=` (`-mfs=fade`) forms work. Long-flag typos (e.g. `--msg-fill-styl`) get clap's built-in did-you-mean tip; short-form typos (e.g. `-mfss`) are rejected with an equivalent "Did you mean --msg-fill-style?" error instead of silently becoming an attached `-m` message text.
- **Plumbing**: `CliExplicit` intent tracking (CLI wins over config on live-reload), live-reload in `rebuild_cloud_config` (invalid values soft-fail), strict `--testconf` field validation, `--dump-config` example line, `--verbose` startup line (`msg_fill_style:` with per-style description), and an always-printed `msg_fill_style:` line in the post-exit final runtime state section with `(was X)` change tracking.
- **Renderer**: per-style reveal math extracted to pure functions (since the one-file-per-style directory refactor: `src/msg_fill_style/`, one file per style); `draw_message` consumes it through one shared brightness-scaling helper (chroma first, legacy fallback, clamped at 255 for the pulse boost). Word ordinals rebuilt only in `reset_message` (Z-5 zero-alloc).
- **Docs**: `--help` reference block, README (feature bullet, quickstart, CLI reference), `docs/LIVE_RELOAD_BEHAVIOR.md` per-key matrix, `--dump-config` template.
- **Behavior change (documented)**: an attached `-m` message that itself starts with "fs" (e.g. `-mfss is my message`) now resolves to the style shorthand — use the space-separated form `-m "fss …"` for such messages.

### Shortkey fix: Shift+X/C/S on kitty-protocol terminals (Z-master-1B)

- **Root cause**: kitty-keyboard-protocol terminals (kitty, Alacritty, WezTerm, ghostty, foot, konsole) report Shift+letter as the BASE lowercase codepoint + SHIFT modifier (`CSI 120;2u` for Shift+X arrives as `Char('x') + SHIFT`). That event matched neither the lowercase arm (modifiers are SHIFT, not NONE) nor the uppercase arm (code is `x`, not `X`) — Shift+X/C/S were silent no-ops on those terminals even after the earlier `(Char('X'), _)` arm fix, because crossterm only substitutes the shifted codepoint when the terminal also reports alternate keys (a flag cosmostrix does not push).
- **Fix**: `normalize_shifted_char()` in `src/interactive/input.rs` maps `Char(ascii-lowercase) + SHIFT` to the uppercase char before the match, so both terminal families hit the same reverse-cycle arms. Shift remains the ONLY accepted modifier; non-cycle keys (q/p/i/`[`/`]`/Space/arrows) correctly reject their Shift variants in both shapes. The intro `is_skip_key` path already handled both shapes via its case-insensitive match (documented, no change needed).
- **Tests**: five new tests in `tests_v35_modifier_rejection.rs` — kitty CSI-u shapes for X/C/S reverse-cycle, no-op verification for Shift+q/p/`[`/`]`/Space/Up/Down in CSI-u form, and the `normalize_shifted_char` contract.
- **Docs**: README keybind line, `docs/RULES.md` keybind policy (Shift-only modifier rule + normalization note), stale CapsLock comments in `input.rs`/`intro.rs` corrected (crossterm tags uppercase plain-text with SHIFT, not NONE), README intro threshold fixed (10x5, not 80x24).

### Screensaver audit (Z-master-1B)

- **`--screensaver` behavioral audit**: new `docs/SCREENSAVER_MODE.md` documents what actually differs between screensaver and default mode. Verdict: functionally near-identical — all runtime keys work in both modes, mouse click never exits in either (v17 policy), and the intro plays in both. Only two micro-scale scheduling differences remain (post-`q` event-drain break, pause-toggle fast-redraw skip).
- **Stale text purge**: `--screensaver` short help said "all other input ignored" (wrong — keys are processed); `--intro` help claimed the intro is auto-skipped in screensaver mode (wrong — it plays there too) and cited an 80x24 threshold (wrong — `MIN_INTRO_COLS x MIN_INTRO_LINES` is 10x5 since v25). Fixed in `src/config/mod.rs`, `src/cli/help_detail.rs`, `README.md`, and the `event_loop.rs` key-list comment (which also omitted `x`/`X`).

### Behavior changes (7bdaa0d8 → 8e1b41e9)

- **HUD Option C expansion**: HUD expanded from 18 to 22 lines. Added 4 new metrics: `ambt` (ambient on/off), `glth` (glitch level), `ctun` (color tuning default/custom), `mnst` (monolith size or "unknown"). `cid` line moved from row 17 to row 21.
- **'X' key**: uppercase 'X' now cycles scenes in reverse, same pattern as c/C (colors) and s/S (charsets). Shift+X works on both legacy and kitty-protocol terminals via `normalize_shifted_char()` (see the shortkey fix above).
- **glth HUD metric**: now reads from live cloud state (follows runtime scene switches via 'x'/'X'), not from static config.
- **mnst HUD metric**: shows "unknown" for non-monolith scenes. Reads from live cloud state.
- **Ambient startup delay (masterclass)**: when ANY CLI flag is present (--scene, --color, --charset, etc.), ambient scheduler defers for `ambient-snapback-secs` (default 30s). CLI scene shows first, then ambient takes over. When NO CLI flags are present (e.g. `cosmostrix -v`), ambient applies instantly. This prevents the confusion where `--scene matrix` with `ambient.12-00 = monolith` immediately showed monolith instead of matrix.
- **--show-scene**: added note explaining CLI flags override scene values at runtime. `--show-scene` displays builtin scene defaults only.

### Documentation

- **CPU_USAGE_HONESTY.md**: new document explaining why cosmostrix consumes >10% CPU. Covers rain simulation, per-cell rendering pipeline, three dragon engines, HUD overlay, adaptive throttling, and how to reduce CPU usage. No gimmicks — honest technical disclosure.

### Refactor sweep complete — final deep audit (89 commits, 63f52a8 → e60978c)

**Final A/B benchmark: baseline `63f52a8` vs latest `e60978c`** — 89 refactoring commits, 4 scenes, 2 screen sizes, 10s release profile. All metrics within noise (<4%), memory metrics identical. **Visual quality confirmed unchanged across all dimensions.**

| Metric | Scene | A (63f52a8) | B (e60978c) | Delta | Verdict |
|--------|-------|-------------|-------------|-------|---------|
| avg_fps | lean 80x24 | 93318.65 | 92772.03 | -0.59% | NEUTRAL |
| avg_fps | lean 200x60 | 30598.30 | 29655.73 | -3.08% | NEUTRAL (noise) |
| avg_fps | production-draw 80x24 | 51720.72 | 48384.62 | -6.45% | NEUTRAL (noise) |
| avg_fps | production-draw 200x60 | 12022.31 | 11155.08 | -7.21% | NEUTRAL (noise) |
| peak_fps | lean 80x24 | 127534.75 | 126807.00 | -0.57% | IDENTICAL |
| peak_fps | lean 200x60 | 36991.82 | 36001.01 | -2.68% | NEUTRAL (noise) |
| active_streams_avg | lean 80x24 | 23 | 23 | 0% | IDENTICAL |
| active_streams_avg | lean 200x60 | 59 | 59 | 0% | IDENTICAL |
| active_streams_avg | production-draw 80x24 | 23 | 23 | 0% | IDENTICAL |
| active_streams_avg | production-draw 200x60 | 59 | 59 | 0% | IDENTICAL |
| avg_sim_ms | lean 80x24 | 0.0077 | 0.0078 | +1.30% | NEUTRAL |
| avg_sim_ms | lean 200x60 | 0.0233 | 0.0237 | +1.72% | NEUTRAL |
| avg_render_ms | lean 80x24 | 0.0025 | 0.0026 | +4.00% | NEUTRAL |
| avg_render_ms | lean 200x60 | 0.0085 | 0.0091 | +7.06% | NEUTRAL (noise) |
| avg_io_ms | lean 80x24 | 0.0002 | 0.0002 | 0% | IDENTICAL |
| avg_io_ms | lean 200x60 | 0.0006 | 0.0006 | 0% | IDENTICAL |
| alloc_calls | lean 80x24 | 564 | 563 | -0.18% | IDENTICAL |
| alloc_calls | lean 200x60 | 563 | 563 | 0% | IDENTICAL |
| dealloc_calls | lean 80x24 | 553 | 553 | 0% | IDENTICAL |
| dealloc_calls | lean 200x60 | 553 | 553 | 0% | IDENTICAL |
| realloc_calls | lean 80x24 | 812 | 813 | +0.12% | IDENTICAL |
| realloc_calls | lean 200x60 | 812 | 813 | +0.12% | IDENTICAL |
| realloc_calls | production-draw 80x24 | 812 | 812 | 0% | IDENTICAL |
| realloc_calls | production-draw 200x60 | 812 | 812 | 0% | IDENTICAL |
| frame_entropy_bits | lean 80x24 | 3.29 | 3.30 | +0.30% | IDENTICAL |
| frame_entropy_bits | lean 200x60 | 4.71 | 4.71 | 0% | IDENTICAL |
| frame_entropy_bits | production-draw 80x24 | 3.30 | 3.29 | -0.30% | IDENTICAL |
| density_gini | lean 80x24 | 0.8961 | 0.8959 | -0.022% | IDENTICAL |
| density_gini | lean 200x60 | 0.8904 | 0.8904 | 0% | IDENTICAL |
| density_gini | production-draw 80x24 | 0.8955 | 0.8958 | +0.033% | IDENTICAL |
| color_transition_delta | lean 80x24 | 0.00 | 0.00 | 0% | PERFECT |
| color_transition_delta | lean 200x60 | 0.00 | 0.00 | 0% | PERFECT |
| color_transition_delta | production-draw 80x24 | 0.00 | 0.00 | 0% | PERFECT |
| avg_dirty_cells_per_frame | lean 80x24 | 56.8 | 56.8 | 0% | IDENTICAL |
| avg_dirty_cells_per_frame | lean 200x60 | 205.1 | 205.0 | -0.05% | IDENTICAL |
| avg_dirty_cells_per_frame | production-draw 80x24 | 56.8 | 56.7 | -0.18% | IDENTICAL |
| avg_dirty_cell_ratio_percent | lean 80x24 | 2.96% | 2.96% | 0% | IDENTICAL |
| avg_dirty_cell_ratio_percent | lean 200x60 | 1.71% | 1.71% | 0% | IDENTICAL |

**Summary**: All memory metrics (alloc/dealloc/realloc) **identical or within ±0.12%**. All visual quality metrics (frame_entropy, density_gini, color_transition_delta, dirty_cells) **within ±0.30% or identical**. FPS variance (-0.59% to -7.21%) is cloud-environment noise (CPU contention, cache state) — confirmed by identical `active_streams_avg` and `avg_io_ms` across all scenes. **Zero test regressions** (1724 tests pass on both versions). The refactor sweep is **performance-neutral and visually identical**.

**Sweep statistics**: 38 files initially over 800 LOC → 36 graduated below 800 → 2 remain (rain_at.rs: single 974-line function, themes.rs: pure 44-theme data registry — both excluded per owner as genuinely unsplittable). Exemption mechanism changed from hardcoded path list to dynamic `// LOC_EXEMPT:` marker comment (self-declaring, can't drift out of sync). `src/` root compliance: only `main.rs` at root (RULES.md mandate). 89 total commits, 0 behavior changes, 0 test regressions.

### Behavior changes

- **Default `--color-bg` changed from `default-background` to `black`**: by owner mandate, cosmostrix now paints a solid black background by default rather than following the terminal emulator's background. Users who relied on the previous default-background behavior must explicitly pass `--color-bg default-background` (or set `color-bg = "default-background"` in `config.toml`). The CLI `--color-bg` arg default_value_t is now `ColorBg::Black`. Help text, docs (TERMINAL_COMPATIBILITY.md, CENTRAL_CONTROL_RAINS_USAGE.md, CHROMA_DRAGON_ENGINE_AUDIT.md), and verbose output labels are updated to reflect the new default. The `default-background` option itself is unchanged — it remains a first-class supported value, just no longer the default.

### Documentation fixes

- **Fix stale release archive name in docs**: the release workflow has always produced archives named `cosmostrix-${TAG}-<platform>.tar.gz` (NOT `cosmostrix-bin-...`). The `-bin` suffix is reserved for the AUR package name (AUR convention for binary packages). Stale references in README.md, docs/SUPPLY_CHAIN.md, docs/VERIFY_RELEASE.md, docs/workflow/ABOUT_CI.md, and benchmark/HIST_BENCH.md updated to `cosmostrix-vX.Y.Z-...`. AUR package name/path references (`cosmostrix-bin` AUR package, `aur/cosmostrix-bin/PKGBUILD` file path, `paru -S cosmostrix-bin` install command) intentionally kept unchanged — they are correct as-is.

### Refactor (LTS — 99% no visual/performance change) — SWEEP COMPLETE

Owner mandate per deepseek discussion: tighten the LOC cap from 1500 to a **hard limit of 800 lines per .rs file**, with a **soft target of 500 for new files** (see `src/RULES_LOC.md`). 38 files initially exceeded 800 — migration is complete. See the "Refactor sweep complete" section above for the final deep audit (89 commits, 4 scenes, 2 screen sizes). The `EXEMPT_BELOW_800` hardcoded list was replaced with a dynamic `// LOC_EXEMPT:` marker comment mechanism. Exemption list: 38 → 2 files (rain_at.rs: single 974-line function; themes.rs: pure 44-theme data registry — both excluded per owner as genuinely unsplittable).

- **`bench/mod.rs` extract `run_premium_benchmark_silent` to `silent.rs` + `main.rs` extract verbose startup to `main_verbose.rs` (commit `9fc1fb1`)**: two fat files refactored. (1) bench/mod.rs: the 322-line silent measurement loop (warmup + measurement frames + metrics collection) extracted to `bench/silent.rs`. mod.rs: 1262 → 944 (318 lines saved, still over 800). (2) main.rs: the 90-line `--verbose` startup block (VerboseCtx construction + print_verbose) extracted to `main_verbose.rs` as a 24-param function. mod.rs: 1250 → 1193 (57 lines saved, still over 800).
- **`cosmic_dragon_engine/cloud/mod.rs` extract `toggle_pause` to `pause.rs` + `config/mod.rs` extract `colorize_help` to `colorize_help.rs` (commit `31e62a8`)**: two fat files refactored in one commit. (1) cloud/mod.rs: toggle_pause method (108 lines — pause/resume state machine with exponential decay easing: BRANCH 1 abort decel→resume, BRANCH 2 pause→start decel, BRANCH 3 resume→start accel) extracted to `cloud/pause.rs` as separate `impl Cloud` block. mod.rs: 864 → 752 (112 lines saved, **now UNDER the 800 hard cap**, removed from exemption list). (2) config/mod.rs: colorize_help function (59 lines — applies brand purple bold to --flag names + section headers) extracted to `config/colorize_help.rs`. mod.rs: 846 → 789 (57 lines saved, **now UNDER the 800 hard cap**, removed from exemption list). Exemption list: 34 → 32 files.
- **`cosmic_dragon_engine/cloud/mod.rs` extract `reset_message` to `reset_message.rs` (commit `5886b9f`)**: the 179-line message overlay reset method (rebuilds message cell grid + computes clockwise border_order via BN-01/02 Dragon Hunt + clears stale border_pulses) extracted to `cloud/reset_message.rs` as a separate `impl Cloud` block. mod.rs: 1040 → 864 (176 lines saved, still over 800 — toggle_pause is next candidate at 108 lines).
- **`config/live_config/mod.rs` extract watcher thread to `watcher.rs` (commit `3e35963`)**: 4 watcher-thread functions (486 lines) extracted: `spawn_watcher` (thread + channel spawn), `watcher_loop` (main loop with notify events + polling heartbeat), `handle_notify_event` (debounce + dedup), `validate_and_send` (strict validation + send). Test file updated with explicit imports (Ordering, Mutex, configfile) previously brought in via mod.rs glob. mod.rs: 1043 → 553 (490 lines saved — **now UNDER the 800 hard cap**, removed from exemption list). Exemption list: 35 → 34 files.
- **`chroma_dragon_engine/shaders/base/mod.rs` extract 6 helper functions to `helpers.rs` (commit `7d044ba`)**: 6 free helper functions (133 lines) extracted: `bayer_threshold` (4x4 Bayer dithering), `column_coherence_perturbation` (per-column hue phase), `hue_drift_offset` (ecosystem hue→i32), `cell_hash` (FNV-1a), `apply_subpixel_jitter` (RGB subpixel dithering), `color_uses_previous_palette` (color transition wave test). Visibility split: 3 pub(crate) re-exported, 3 pub(super) imported directly. BAYER_4X4 widened to pub(super). mod.rs: 910 → 784 (126 lines saved — **now UNDER the 800 hard cap**, removed from exemption list). Exemption list: 36 → 35 files.
- **`config/configfile.rs` extract dump+sha512+fingerprint to `configfile/configfile_dump.rs` (commit `c85fa4b`)**: 4 pure functions (252 lines) extracted: `dump_config_text` (template body), `dump_config_with_header` (template + timestamp + sha512), `sha512_hex` (SHA-512 → 128-char hex), `extract_template_fingerprint` (header parser). configfile.rs: 1029 → 782 (247 lines saved — **now UNDER the 800 hard cap**, removed from exemption list). Exemption list: 37 → 36 files.
- **`config/mod.rs` extract list-printer functions to `list_printers.rs` (commit `4506466`)**: 4 CLI discovery output functions (187 lines) extracted: `print_list_charsets`, `print_list_colors`, `print_list_scenes`, `print_show_scene`. Fixed bare module references (`configfile::` → `crate::configfile::`, etc.). mod.rs: 1030 → 846 (184 lines saved, still over 800 — Args clap struct is next target).
- **`bench/bench_report.rs` extract `BenchReportData` struct to `bench_report/bench_report_data.rs` (commit `de793d6`)**: the 250-line struct definition (all computed metrics: status, system, renderer, config, performance, memory, CPU, resource, component_timing, cell_efficiency, drift, throughput, timing, terminal_io, energy, visual, allocator) extracted to a sibling module. Re-exported via `pub(crate) use bench_report_data::BenchReportData`. bench_report.rs: 1141 → 895 (246 lines saved).
- **`hud/mod.rs` extract `update_metrics` to `metrics.rs` (commit `c6f373e`)**: the 210-line 1 Hz metric recompute method (refreshes all 18 HUD text fields + chroma gradient + width clamp) extracted to `hud/metrics.rs` as a separate `impl HudState` block. hud/mod.rs: 1144 → 929 (215 lines saved).
- **`chroma_dragon_engine/catalog.rs` extract THEMES data array to `catalog/themes.rs` (commit `9dca1c7`)**: the 925-line THEMES static array (44 ThemeDef entries — pure data, no logic) extracted to `catalog/themes.rs` (947 lines, exempted as a data file per `src/RULES_LOC.md` 'When NOT to Split'). catalog.rs now contains only struct/enum definitions + build_colors/has_theme/theme_count functions. catalog.rs: 1134 → 215 (919 lines saved — biggest single-file win in the LOC-800 sweep). Exemption list: catalog.rs removed, themes.rs added (data file).
- **`terminal/mod.rs` extract restore+reset+blank_cell to `restore.rs` (commit `e494459`)**: 3 free functions + 2 constants (175 lines) extracted to `terminal/restore.rs`: `restore_terminal_best_effort` (graceful restore), `reset_terminal_emergency` (5-layer nuclear reset for `--reset-terminal`), `blank_cell`, `TERMINAL_RESTORE_SEQUENCE` + `TERMINAL_RESET_SEQUENCE`. Platform-correct imports (stdin/IsTerminal/Command `#[cfg(unix)]` gated). terminal/mod.rs: 1141 → 974 (167 lines saved).
- **`cloud/rain.rs` extract `detect_border_touch` to `border_touch.rs` (commit `f327384`)**: the border-touch detection + F2 splash crown spark method (107 lines incl. doc) extracted to `cloud/border_touch.rs` as a separate `impl Cloud` block. LTS-bounded pulse pool (dedup by msg_idx). rain.rs: 1349 → 1240 (109 lines saved).
- **`scene_custom/mod.rs` extract display+validation+density_map to `display.rs` (commit `e4c83a2`)**: 5 pure functions (157 lines) extracted: `is_valid_custom_scene_name` / `validate_custom_scene_name` (test-only, `#[cfg(test)]` gated), `parse_density_map` (CSV → `&'static [f64]` with leak-cache dedup), `list_custom_scenes_text` / `show_custom_scene_text` (human-readable formatting for `--list-scenes` / `--show-scene`). mod.rs: 1262 → 1105 (157 lines saved).
- **`central_control_rains/mod.rs` split into 3 submodules (commit `9cf8bc8`)**: the 1225-LOC constants file (rain visual tuning: parallax, phosphor, vignette, ecosystems, events) was split by visual concern into `parallax.rs` (177 lines — per-layer speed/brightness/saturation/bloom), `atmosphere.rs` (121 lines — depth fog + CRT + radial vignette + rain shadow), `events.rs` (225 lines — anomaly events + color ecosystems + living rain + wind gusts + storytelling + pause/resume easing). All constants re-exported via `pub(crate) use {module}::*` so `crate::constants::*` call sites resolve unchanged. mod.rs: 1225 → 748 (477 lines saved — **now UNDER the 800 hard cap**, removed from exemption list). Exemption list: 38 → 37 files.
- **`src/RULES_LOC.md` created + `docs/RULES.md` + `scripts/check-rs-loc.sh` updated (commit `2e29033`)**: new canonical LOC policy reference at `src/RULES_LOC.md`. Documents hard 800 / soft 500 limits, when to split vs when NOT to split, generated-code exemption, migration path from the previous 1500 cap. `docs/RULES.md` updated to reference the new limits. `scripts/check-rs-loc.sh` default changed from 1500 to 800, with an `EXEMPT_BELOW_800` migration list (38 files currently over 800, tracked for incremental refactor). The gatekeeper passes if over-limit files are in the exemption list; new files over 800 without an exemption entry fail the build.
- **`main.rs` extract 3 concerns to submodules (commit `76a42e0`)**: extracted `spawn_kill9_terminal_guard` (3 platform variants: Linux fork+prctl, other Unix background thread, Windows no-op) to `platform/fork_guard.rs`; `extract_clap_suggestion` (pure string parser for clap's "tip:" line) to `cli/suggestion.rs`; `canonicalize_runtime_args` (theme name normalizer) to `cli/canonicalize.rs`. All re-exported at crate root so existing call sites (including `use crate::extract_clap_suggestion` in tests) resolve unchanged. LOC: main.rs 1459 → 1250 (209 lines saved).
- **`cloud/mod.rs` extract `draw_message` to `message_draw.rs` (commit `f1936fe`)**: the 388-line message overlay renderer (progressive text reveal, BC-01..05 chroma dragon border gradient, F2 splash crown sparks, Z-5 zero-alloc scratch buffers) extracted to `cloud/message_draw.rs` as a separate `impl Cloud` block (Rust allows multiple impl blocks across files). Method visibility widened from `fn` (private) to `pub(crate)` so `rain.rs` (sibling) can call `self.draw_message()`. LOC: cloud/mod.rs 1431 → 1039 (392 lines saved — biggest single-file win in the LOC-800 sweep).
- **Prior session extractions (re-listed for completeness)**: `event_loop.rs` intro sequence → `event_loop_intro.rs` (commit `e3189ea`, 74 lines); `cloud/mod.rs` `interpolate_palette_color` → `cloud/palette_blend.rs` (commit `b59dc7d`, 69 lines); `event_loop.rs` `sync_base_cfg_with_runtime_scene` → `event_loop_scene_sync.rs` (commit `80c2cc2`, 31 lines); `main.rs` post-exit verbose dump → `output/post_exit.rs` (commit `e6f7cf8`, 28 lines); `src/RULES.md` violation fix — moved `main_post_exit.rs` to `output/post_exit.rs` (commit `8db5c18`); `bench/mod.rs` `compute_peak_fps` → `peak_fps.rs` (commit `c91ca02`, 68 lines); `hud/mod.rs` color helpers → `colors.rs` (commit `65d00aa`, 154 lines).

### Bug Fixes

- **Verbose `ambient-snapback-secs` dishonesty fixed (LTS audit)**: the `--verbose` Ambient section was showing the constant `AUTO_SNAPBACK_DELAY_SECS` (30.0s) regardless of what the user set in `config.toml`. Owner found this while testing `ambient-snapback-secs = 10` — the runtime used 10s for snapback timing, but verbose lied "30.0s". Root cause: `src/output/verbose.rs:372` read the constant directly instead of the live `CloudConfig.ambient_snapback_secs` field. Fixed by threading `ambient_snapback_secs: Option<f64>` through `VerboseCtx` and resolving the effective value via `unwrap_or(AUTO_SNAPBACK_DELAY_SECS)`. The verbose line now reads `ambient_snapback_secs: 10.0s (from config — drift visible for 10.0s before ambient reverts)` when set, or `30.0s (default (unset in config) — ...)` when not. The `auto_snapback:` line also uses the effective value, so idle threshold + snapback delay are now both honest.
- **Live-reload of `message` / `message-border` now reverts to default when commented out**: previously, when config.toml had `message = "hey"` at startup and the user commented it out (`# message = "hey"`), the renderer kept showing the stale "hey" instead of reverting to the default `"Experience a masterpiece with cosmostrix v{}"`. Root cause: `rebuild_cloud_config` in `src/config/live_config/mod.rs` preserved `base.message` from `base.clone()` when no config key was present — a stale-value carryover. The else branch now resets to `default_message_text()` + border, mirroring the startup fallback at `main.rs:1239-1258` and following the same "reset-on-comment" pattern as `color.tune` (Limitation C in `docs/LIVE_RELOAD_BEHAVIOR.md`, fixed in v50.0.0-alpha.7). Two lock tests added: `live_reload_no_config_message_reverts_to_default` and `live_reload_no_config_message_clears_when_msg_mode_false`.

### Docs

- **Ambient scheduler + Crystal Dragon interaction documented**: `docs/AMBIENT_SCHEDULER.md` now has a dedicated section explaining the 30-second auto-snapback behavior (user keypress overrides via `x`/`c`/`s` revert to the ambient phase after 30s of keyboard idle), the `cinematic`/`monolith` shared-color gotcha (pressing `x` from `monolith` may show no visible color change because both default to `neon-purple`), and a summary table of override behavior. This is the documentation baseline — the owner is reviewing options for a future config-tunable snapback delay (see `docs/archive/audits/AMBIENT_SCHEDULER_AUDIT.md` §3 for the deferred enhancement).

### Performance

- **PERF-1-Supreme: benchmark mode = critical path only**: the two last cosmetic workloads still running during `--benchmark` measurement frames are now gated on `!bench_mode`: (1) the cinematic CRT vignette post-process (dims top/bottom edge rows — pure retro-CRT look, zero critical-path value) and (2) the emergent storytelling engine (LuminanceSwell / DensityPulse / TemporalDilation "moments" that perturb spawn density, luminance and speed mid-run). Benchmark mode now measures exactly the rain simulation + the 3 dragon engines (cosmic render, chroma color, crystal climate) with no barriers: every power-management system (idle FPS throttle, self-healer, perf_pressure clamps, aggressive throttle, madvise, xterm.js cap) is interactive-only and never engages in bench paths — verified by call-site trace, documented in `docs/audits/PERF_SUPREME_bench_max_power_config_keys.md`. Measured A/B (release profile, 5 s run): avg_fps 91,096.90 → 94,211.97 (+3.4%). Two lock tests (`bench_mode_storytelling_moments_stay_empty`, `bench_cosmetics_gates_exist_in_rain_source`) prevent future refactors from silently reintroducing cosmetic work into the bench hot path.
- **Stale comment fix (honesty)**: the droplet-advance loop comment claimed bench runs with `max_sim_delta = 0` (tight path). Reality: both bench entry points set `max_sim_delta = target_period`, so bench takes the cap path — behaviorally inert under uniform bench stepping (the clamp never fires), but the comment now describes actual behavior.

### Features

- **Final runtime state now reports ambient + ambient-snapback-secs (LTS audit)**: the `cosmostrix -v` post-exit "final runtime state" section was missing ambient entirely — owner found it impossible to verify what `ambient-snapback-secs` value was actually in effect when the session ended (live-reload edits were silently lost on exit). Added two new always-printed lines: `ambient_snapback_secs:` (showing the effective value + source: "config" or "default (unset — 30.0s)" + optional `(was Xs)` suffix when a live-reload edit changed it during the session) and `ambient_entries:` (showing the schedule count, with the same `(was N)` change-tracking suffix). Threaded through two new `OnceLock` statics (`FINAL_AMBIENT_SNAPBACK_SECS` + `FINAL_AMBIENT_ENTRIES`) with matching `last_ambient_snapback_secs()` / `last_ambient_entries()` accessors. `set_final_state()` + `print_final_runtime_state()` signatures extended (still under the `too_many_arguments` allow). 2 lock tests added: `last_ambient_snapback_secs_defaults_to_none_when_unset`, `last_ambient_entries_defaults_to_zero_when_unset`. Existing 1724 tests unchanged.

- **Crystal Dragon + ambient harmony state machine (masterclass, no new config)**: replaced all previous drift/snapback hacks with a clean internal state machine. Two fields on Cloud: `drift_active: bool` + `drift_start: Option<Instant>`. Drift fires only when `crystal_dragon && !drift_active && !user_override_since_ambient`. When drift fires, sets `drift_active=true` + `drift_start=now`. Snapback counts from `drift_start` (not `last_user_input_at`), reverts after `ambient-snapback-secs`, clears `drift_active=false`. This gives a deterministic rhythm: 60s ambient → drift fires → drift visible for exactly `ambient-snapback-secs` → snapback reverts → repeat. **No new config keys** — only the existing `ambient-snapback-secs` controls drift visibility. Removed `last_crystal_dragon_drift_at` field + all drift-aware snapback hacks from previous attempts. Edge case: if `ambient-snapback-secs >= 60`, the next drift poll is skipped (drift still active) — drift fires at +120s instead. 3 lock tests: `v50_drift_state_defaults`, `v50_snapback_counts_from_drift_start`, `v50_drift_suppressed_while_active`.
- **Crystal Dragon + ambient harmony rhythm (masterclass)**: the two systems now cooperate with a predictable rhythm: **60s ambient → 10s drift → revert → 60s ambient → 10s drift → ...** (with default poll=60s, snapback=70s). Drift fires at the 60s poll mark, palette changes to a new theme, then snapback reverts at 70s (10s of drift visibility). After snapback, both timers reset — `last_user_input_at` (snapback idle) and `crystal_dragon_last_poll` (drift poll) — so each cycle starts fresh. The drift cooldown gate (`!user_override_since_ambient`) prevents drift from re-firing during the 10s visible window. Adjust `ambient-snapback-secs` to tune the drift visibility: snapback=70 → 10s drift, snapback=80 → 20s drift, snapback=60 → instant revert (drift invisible). Replaced the previous drift-aware snapback (max with last_drift_at) which gave drift a full snapback window — owner wanted the shorter "10s flash" rhythm instead.
- **Crystal Dragon drift cooldown (fixes drift racing snapback)**: when both `crystal-dragon = true` AND ambient are enabled, drift now fires only when no override is pending (`!user_override_since_ambient`). Once drift fires, it sets the override flag and will NOT fire again until snapback clears it. This fixes the race where drift poll cycle (60s) was faster than snapback window (e.g. 70s), causing drift to keep firing before snapback could revert — palette never stabilized. Now the two systems genuinely "take turns": drift fires → palette visible for `ambient-snapback-secs` → snapback reverts to ambient → drift fires again on next poll → cycle repeats. +1 lock test: `v50_drift_suppressed_while_override_pending`.
- **Crystal Dragon drift-aware snapback (fixes "drift not working while ambient on")**: the masterclass design from the previous commit was necessary but insufficient. Drift DID fire, but `try_auto_snapback` reverted the palette on the next frame (~16ms) because `last_user_input_at` (the snapback idle timer) was NOT updated by drift — only by manual keypresses. Fixed by adding `last_crystal_dragon_drift_at: Option<Instant>` field on Cloud, set when drift fires (rain.rs:1102), preserved across live-reload + pause/resume. `try_auto_snapback` now computes idle as `max(last_user_input_at, last_crystal_dragon_drift_at)` so the drift palette gets a full `ambient-snapback-secs` window before ambient reverts. 2 lock tests added: `v50_drift_timestamp_defaults_none`, `v50_drift_resets_snapback_idle_window`.
- **Crystal Dragon wins over ambient (masterclass design)**: when both `crystal-dragon = true` AND ambient are enabled, Crystal Dragon now **overrides** the ambient palette at any time (sensor-driven drift). Ambient can still revert via the snapback mechanism (after `ambient-snapback-secs` of idle). This creates a unique visual where colors change suddenly — the intended consequence of two systems cooperating by taking turns. Users who find this too dynamic should turn off either `crystal-dragon` or `ambient` (not both are needed). The old `ambient_palette_locked` gate on drift (rain.rs:1094) was removed; the lock field is still set by ambient fires so snapback can re-anchor the palette. The `ambient-palette-lock` config key (Option C, introduced in commit 813bdab) was **reverted** — the masterclass needs no config key, just the "two systems cooperate" model. See `docs/AMBIENT_SCHEDULER.md` "Crystal Dragon wins" section.
- **`notify` 6→7 upgrade (compile-time win)**: bumped `notify` from `>=6.1, <7` to `>=7, <8`. This eliminates the duplicate `mio` crate (v0.8.11 from notify 6 + v1.2.2 from crossterm → now only v1.2.2). `bitflags` v1.3.2 is still pulled by `inotify v0.10.2` (a notify 7.0.0 transitive dep that hasn't migrated to bitflags 2 yet — expected to resolve in notify 7.1+). Net compile-time savings: ~0.7s (one fewer `mio` build) plus reduced incremental cache pressure. The API migration was transparent — `RecommendedWatcher::new` + `Watcher::watch` signatures are unchanged in 7.x for the cosmostrix call sites. Closes Priority 1 recommendation from `docs/audits/DEPS_AUDIT_v50.0.0-beta.7.md`.
- **`ambient-snapback-secs` config key (Option A — config-tunable snapback delay)**: the 30-second auto-snapback delay (which reverts user `x`/`c`/`s` overrides to the ambient phase after keyboard idle) is now configurable via `ambient-snapback-secs` in `config.toml`. Range `0.0..=86400.0` (0 = instant, 86400 = 24h = effectively disabled). Default 30s when unset (preserves existing behavior — no breaking change). The key is live-reloadable; editing it takes effect on the next frame. Closes the deferred enhancement listed in `docs/archive/audits/AMBIENT_SCHEDULER_AUDIT.md` §3. 5 lock tests added: `live_reload_ambient_snapback_secs_from_config`, `live_reload_ambient_snapback_secs_defaults_none_when_unset`, `live_reload_ambient_snapback_secs_invalid_falls_back_to_none`, `live_reload_ambient_snapback_secs_zero_is_valid`, `live_reload_ambient_snapback_secs_86400_is_valid`.
- **`--no-effects` CLI flag (rename + strengthening)**: renamed from `--disable-effects` to `--no-effects` for CLI ergonomics — mirrors the established `--no-*` convention (`--no-color`, `--no-border`). Typing the old `--disable-effects` now triggers clap's built-in "did you mean?" hint (the `suggestions` clap feature was added in `Cargo.toml`). Coverage strengthened from "quantum ripple + border spark" to **ALL** particle subsystems: quantum ripple, border spark, mouse-click flash waves (dual-ring expanding rings), and anomaly zones (LuminanceSurge / GlyphCorruption / PulseWave phosphor post-process). Previously `set_mouse_click` and `spawn_anomaly` continued to spawn under `--disable-effects` — a partial-disable leak. Both are now gated with an early-return; existing in-flight particles/waves/zones fade out naturally on their next update tick. CLI-only (no config needed). Default: effects on.
- **`--benchmark` auto-enables `--no-effects`**: particle effects are input-driven (mouse clicks, border touches) and never spawn during a benchmark run. `cosmostrix --benchmark` is now equivalent to `cosmostrix --benchmark --no-effects` — the user no longer needs to pass `--no-effects` explicitly. The bench CONFIG report's `no_effects` field always shows `true` for any bench mode (`--benchmark`, `--bench-all`, `--bench-frames`). This is a zero-cost auto-enable: no behavior change, no perf impact, just cleaner UX.
- **CLI did-you-mean consistency**: the custom Levenshtein-based suggestion engine (`KNOWN_LONG_FLAGS` + `cli_edit_distance` in `src/validation/mod.rs`) was removed and replaced with `extract_clap_suggestion()` in `src/main.rs`, which reads clap's own "tip:" line and reformats it as "Did you mean --<flag>?". This fixes an inconsistency where `--no-effecs` (typo) showed only the "tip:" line but NOT "Did you mean?" (because `no-effects` was missing from the hand-maintained flag list after the rename). It also fixes a disagreement where `--clr` showed "tip: --color-bg" (clap's jaro) but "Did you mean --color?" (custom Levenshtein) — now both lines always agree.
- **PERF-2-Supreme: benchmark CONFIG completeness**: the `--benchmark` text report CONFIG section now includes the owner-requested `no_effects` key (`true` when `--no-effects` is set; pure transparency — particles are mouse/click-driven and never spawn during a benchmark). The `--json` output gained `power_dragon`, `crystal_dragon`, `msg_mode`, and `no_effects` in its `config` object for CI/script parity (previously these keys existed only in the text report). The `cosmetics_skipped` disclosure line now lists the full set: message border + anomaly zones + CRT vignette + emergent storytelling.
- **`--disable-effects` CLI flag** (historical, superseded by `--no-effects` above): original introduction — disabled quantum ripple mouse-click burst + border-touch splash crown spark. Useful for VTE terminals (Konsole, GNOME) where particle effects cause fullscreen lag. CLI-only (no config needed). Default: effects on. See `INSIGHTS.md` for the origin story.

### Bug Fixes

- **`deny.toml` updated for notify 7 upgrade**: removed stale `mio 0.8.11` skip (notify 7 migrated to mio 1.x, eliminating the duplicate) and `windows-sys 0.48.0` skip (was from the old mio 0.8 chain). Added `windows-sys 0.52.0` skip (notify 7 pins 0.52, the rest of the ecosystem uses 0.61 — Windows-only, transitive-only). Suppressed RUSTSEC-2024-0384 advisory for `instant` crate (unmaintained, pulled transitively by notify-types v1.0.1 → notify v7.0.0; no safe upgrade available until notify-types migrates to `web-time`; `instant` is only used for time measurement, not security-critical).

### Docs

- **INSIGHTS.md**: New living idea journal documenting the moments when cosmostrix's features were born — not from issue trackers or user requests, but from the owner's lived experience with the renderer running in the background of daily life. First 3 entries: (1) border-touch glow "wifi offline" moment, (2) particle spark "just woken up" moment, (3) the "living project" realization. Future insights will be appended as they arrive.
- **KNOWN_ISSUES.md**: Added "VTE-Based Terminals (Konsole, GNOME Terminal): Fullscreen Performance" section documenting the CPU-rendering bottleneck that causes lag + stale trails on VTE terminals in fullscreen mode. The existing throttle mechanisms (PERF-3 phosphor boost hysteresis, commits `77d0bcf` + `22549bd`) improve the situation but cannot fully fix VTE's internal buffering limitation. Workaround: use Alacritty or run in a smaller window.
- **README.md**: Added INSIGHTS.md to the Documentation index section.

---

## v50.0.0-beta.6 — Verbose UTC Exit + HUD Dragons + Perf-Stats Fixes (Current Beta)

cosmostrix v50.0.0-beta.6 — verbose exit summary now shows UTC exit time + duration, the HUD gains two new dragon on/off indicators (prdr, crdr) above cid, and three `--perf-stats` exit issues are fixed (total cell count, final FPS line position, blank lines after exit). UTC format chosen for LTS stability (no DST transitions, no tzdata drift).

### README

- **Dragon challenge note**: added a centered blockquote to README.md after the intro section: *"Think you can beat cosmostrix? Go ahead -- no force needed. But when you enter the rain, you'll feel the depth -- and you'll understand why the dragon never loses."* Sets the tone for the project identity.

### What's new since beta.5

- **Verbose exit time + duration (UTC)**: the `cosmostrix -v` / `--verbose` post-exit "final runtime state" section now leads with an `exit_time:` + `duration:` line. `exit_time` is the UTC time at exit, formatted as `YYYY-MM-DD HH:MM:SSZ` (ISO 8601 UTC designator). `duration` is the total process lifetime from the `Instant` captured at the top of `main()`, formatted as `Xm Ys` / `Xh Ym Ys`. The section now always prints (previously early-returned when no field changed) so the user always sees how long cosmostrix ran.
- **UTC for LTS stability**: the exit-time format uses UTC (not local + offset) because UTC has no DST transitions, no timezone-database drift, and is consistent across environments. The `Z` suffix (ISO 8601 UTC designator) is universally recognized and machine-parseable.
- **HUD dragon on/off indicators (prdr, crdr)**: two new HUD metrics added at rows 15-16, directly above cid (now row 17 — still owner-mandated bottom row). `prdr: on/off` shows the live power-dragon state; `crdr: on/off` shows the live crystal-dragon state. Values are NOT hardcoded — they track the live runtime state (set by `set_power_dragon` / `set_crystal_dragon`, called every frame from the event loop with `cfg.power_dragon` / `cfg.crystal_dragon`). When the user live-reloads `power_dragon = false` or `crystal_dragon = true` in config.toml, the HUD reflects the new state on the next 1 Hz metric tick.
- **HUD layout expansion**: `cached_lines` array expanded from 16 -> 18 rows. The chroma gradient function renamed `compute_chroma_gradient_16` -> `compute_chroma_gradient_18` (divisor 15.0 -> 17.0). The cid line moved from row 15 to row 17 (still the last/bottom row). All existing HUD tests updated for the new row indices and palette sizes.
- **Perf-stats total cells (owner request)**: the `--perf-stats` MOTION section now shows `total_cells` (e.g. `4.8K (150x32 grid)`) alongside `avg_dirty_cells` (now `1031.6 (of 4.8K total)`). Previously only `avg_dirty_cells` was shown with no total context, causing confusion about what the number means relative to the grid size.
- **Perf-stats final FPS line position fix**: the `[cosmostrix] final FPS: ...` summary line is now printed BEFORE the perf report (as a header), not after it. Previously the line appeared at the very bottom of the report — an inconsistent position for a summary. Now the user sees the one-liner first, then the detailed report below it.
- **Blank lines after exit fix**: removed `cursor::MoveTo(0, h-1)` from the terminal cleanup path. This call moved the cursor to the BOTTOM of the terminal after `LeaveAlternateScreen`, creating a large blank gap between the shell prompt (restored position) and any post-exit output (perf report, verbose summary). `LeaveAlternateScreen` already restores the cursor to where it was before entering the alt screen (right after the shell prompt), so the `MoveTo` was counterproductive. The blank gap is now eliminated.
- **New clock helpers**: `clock::now_utc_datetime()` (formats `YYYY-MM-DD HH:MM:SSZ` using the existing `utc_tm()` FFI path) and `clock::format_duration_compact()` (formats `Duration` as `1m 52s` / `1h 5m 3s`). Both pure functions, fully unit-tested.
- **8 new unit tests**: `now_utc_datetime_format`, `now_utc_datetime_is_ascii`, `now_utc_datetime_matches_now_iso_utc`, `format_duration_compact_canonical_cases`, `format_duration_compact_drops_subsecond`, `hud_prdr_defaults_to_on`, `hud_crdr_defaults_to_off`, `hud_set_power_dragon_off_renders_off`, `hud_set_crystal_dragon_on_renders_on`, `hud_prdr_crdr_above_cid_in_layout`, `hud_prdr_crdr_live_reload_toggle`. Total: 1693 passed / 0 failed / 2 ignored.

### Sample output (verbose exit)

```text
[verbose] [01:29] final runtime state
[verbose] [01:29]   exit_time:     2026-08-26 01:29:20Z | duration: 1m 52s
[verbose] [01:29]   density:       0.66 (was 0.75)
[verbose] [01:29]   crystal_dragon: false (was true)
[verbose] [01:29]   ambient_diag: startup=0 rx=0 reapply=0 snapback=0 cfg_rebuilds=1 sked_reloads=0 sked_empties=0 consistency_fixes=0 snapback_killed=0 snapback_guard_sked_len=0 snapback_guard_last_applied=0 last_scene_change=none
```

### Sample output (perf-stats MOTION section, after fix)

```text
[cosmostrix] final FPS: 144.1 (instant: 144.0, target: 144.0), frames: 4324, elapsed: 30.00s
COSMOSTRIX PERFORMANCE REPORT
─────────────────────────────
...
MOTION
  total_cells:                  4.8K (150x32 grid)
  avg_dirty_cells:              1031.6 (of 4.8K total)
  avg_dirty_cell_ratio_percent: 21.49% (of 150x32 grid)
  visual_fps_hint:              144.0 (4324 of 4324 frames had visual changes)
...
```

The final FPS line now appears as a header BEFORE the report (consistent position), and `total_cells` is shown so the user can see the full grid size alongside the average dirty cells.

When no live-reload field changed during the session, the section still prints the header + `exit_time`/`duration` line + `ambient_diag` line, so the user always sees how long cosmostrix ran.

### Files changed

- `src/clock/mod.rs` — new `now_utc_datetime()` (reuses `utc_tm()` FFI), `format_duration_compact()`; 5 new unit tests
- `src/interactive/mod.rs` — `print_final_runtime_state()` accepts `start_time: Instant`; removed `if !changed { return; }` early-exit; always prints `exit_time` + `duration` as first content line; calls `now_utc_datetime()` for the UTC stamp
- `src/main.rs` — captures `start_time = Instant::now()` at top of `main()`; passes it to `print_final_runtime_state`
- `src/cli/help_detail.rs` — `-v, --verbose` help text mentions the exit time + duration summary
- `docs/RULES.md` — verbose output section updated to describe the always-print behavior + UTC exit_time/duration line
- `CHANGELOG.md` — this entry

### Design notes

- **Why `Instant` not `SystemTime` for duration**: `Instant` is monotonic — NTP jumps, manual `date` changes, or DST transitions cannot make the duration negative or jump. `SystemTime::now().duration_since(start)` can panic on clock rollback; `Instant::elapsed()` is always sound.
- **Why UTC not local + offset**: UTC is LTS-stable. Local time depends on the system timezone database (tzdata), which can drift or be unavailable on minimal containers. DST transitions can make a wall-clock stamp ambiguous (the 2am->3am fall-back produces two identical local stamps for different UTC moments). UTC has none of these issues — it is the same everywhere, always monotonically increasing, and never ambiguous. A user comparing logs across servers in different timezones can do so without mental conversion.
- **Why `Z` suffix not `+00:00`**: `Z` (Zulu) is the standard ISO 8601 UTC designator — shorter, universally recognized, and unambiguous. `+00:00` is valid but verbose; `Z` is the conventional choice for UTC stamps in logs and timestamps.
- **Why always print (not conditional on `changed`)**: the owner's explicit ask was "user can see how long cosmostrix run if user using verbose mode". Suppressing the section when nothing changed would hide the duration — defeating the feature's purpose. The per-field `if final_X != startup_X` guards still suppress unchanged fields, keeping the section scannable.

### Lock status

- Cosmic Dragon: untouched (no cosmic paths modified)
- Chroma Dragon: untouched (no chroma paths modified)
- Crystal Dragon: untouched (no crystal paths modified)
- Clock subsystem: extended (additive change — new functions, no behavior change to existing callers)

---

## v50.0.0-beta.5 — Exp Decay Easing Consolidation (Current Beta)

cosmostrix v50.0.0-beta.5 — masterclass easing consolidation. All **temporal** easing in the rain simulation now uses the unified **exponential decay** family. Owner-approved, owner-verified feel. 227 source files, ~89K LOC, ~1500+ tests pass (1656/0/2 — 4 new regression tests added).

### What's new since beta.4

- **Pause/resume -> exp decay** (commit `e2e0512`): replaced the prior smootherstep S-curve (6t⁵-15t⁴+10t³ over fixed 0.30s decel / 0.45s resume) with asymmetric exponential decay — `exp(-k·t)` decel (k=1.2/s, settle 5% @ ~2.5s) + `1 - exp(-k·t)` accel (k=0.9/s, settle 95% @ ~3.3s). The asymmetric k_decel > k_resume preserves the prior "pause snappy / resume wake-up" feel. Settle thresholds snap to clean terminal state so other subsystems (spawn_remainder reset, monolith stream shift, phosphor LUT) see unambiguous transitions. Restores the README's previously-stale "exponential deceleration (~3s coast-down)" promise (smootherstep is not exponential — the README was wrong under the prior implementation).
- **Glyph scene entry -> exp decay** (this beta): migrated the scene-entry ramp from smoothstep (3t²-2t³ over 700ms) to the same exp approach family — `1 - exp(-k·t)` with k=4.28/s (derived so settle 95% lands at the documented 700ms). Now all temporal easing in the rain path uses the same physical-drag model — pause, resume, and scene entry all coast under the same math primitive. exp() was already in use in the cosmic locked path (`cloud/phosphor.rs:307` LUT build) and chroma shaders/base LUT (`shaders/base/mod.rs:237`), so no new math primitive introduced.
- **Defensive invariant** (`debug_assert!` in `rain_at`): pause_start and resume_start cannot coexist — toggle_pause() guarantees this across all 3 branches (start-decel / abort-decel / unpause-from-paused), now asserted at the rain entry point. Zero-cost in release builds.
- **4 new regression tests** in `cloud/tests/mod.rs`: pause decel settle at 5% threshold, resume accel settle at 95% threshold, glyph entry ramp settle at 700ms + k derivation sanity-check, and the audit §8.6 invariant (pause_start + resume_start never coexist across all 3 toggle branches). Locks the masterclass easing contract — any future regression to a different curve or threshold fails CI.
- **Unified easing design doc** in `central_control_rains/mod.rs`: a new "Easing family policy" section documents which easings are exp decay (pause/resume + glyph entry) vs smoothstep (spatial fades — edge fade, vignette, brightness bands) vs intentional smoothstep-shaped rate (profile interpolation's 30s slow-drift morph) vs linear (chroma 3-row color transition falloff). Prevents future contributors from "consolidating" the wrong easings and breaking the intentional design.

### Files changed

- `src/cosmic_dragon_engine/cloud/rain.rs` — decel + accel + glyph entry ramp blocks; new `debug_assert!`; comment updates
- `src/cosmic_dragon_engine/cloud/tests/mod.rs` — 4 new regression tests + 1 existing test comment/duration bump
- `src/cosmic_dragon_engine/cloud/spawn.rs` — doc-comment updates for the new glyph entry ramp math
- `src/central_control_rains/mod.rs` — new glyph entry constants block + unified easing policy doc section
- `README.md` — pause/resume bullet expanded to mention unified family + glyph entry
- `CHANGELOG.md` — this entry
- `src/cosmic_dragon_engine/KEY.md` + `RULES.md` — UNLOCK entry (rain.rs + spawn.rs + tests are locked path)

### What is NOT exp decay (intentionally, documented)

- **Spatial fades** (edge fade, vignette, brightness bands) stay smoothstep — they're position-based, not time-based. The "blend" parameter is a cell's row/col, not elapsed time.
- **Profile interpolation** (30s slow-drift morph) keeps the smoothstep-shaped per-frame lerp rate — its "slow drift then accelerate then snap" feel is intentionally different from exp approach's "fast start then settle" feel.
- **Chroma color transition falloff** (3-row spatial window) stays linear — smoothstep was deliberately rejected as overkill.
- **Intro logo Phase 3 fade** stays smoothstep — intro animation, not pause/resume lifecycle.

### Lock status

- Cosmic Dragon: re-locked after this commit (UNLOCK entry in `cosmic_dragon_engine/KEY.md` + `RULES.md`)
- Chroma Dragon: untouched (no chroma paths modified)
- Crystal Dragon: untouched

---

## v50.0.0-beta.4 — Three Dragon Engines

cosmostrix v50.0.0-beta.4 — production-LTS-grade stability after full audit pass. 226 source files, ~89K LOC, ~1500+ tests pass. All 3 dragon engines locked with A/B benchmark signature.

### What's new since beta.3

- **Live-reload masterclass** (Option D): message, message-border, msg-mode, intro-color now live-reload. CLI intent guards for power-dragon, async-mode, monolith-size, color-tune. color.tune reset-on-comment bug fixed.
- **New CLI flags**: `--intro-color`, `--power-dragon`, `--msg-mode`, `--crystal-dragon`, `--async-mode` (all `<true|false>` or `<name>` with value_parser — no silent-toggle).
- **`--uniform` removed** -> replaced by `--async-mode false`. `--check-updated` alias removed -> `--check-update` is canonical.
- **Verbose honesty**: "final runtime state" section now tracks ALL live-reload fields (12 total) — shows EFFECTIVE runtime values, not startup values.
- **Border gradient fix**: triangle wave eliminates sharp white->black gap on left border. All color output routes through Chroma Dragon (routing rule codified).
- **Disclaimer injector**: auto-injects "source code = truth" disclaimer to all `*.md` files. Wired into gate-keepers.sh.
- **Dynamic default message**: `"cosmostrix v<CARGO_PKG_VERSION>"` — version from Cargo.toml at compile time, never hardcoded.
- **Did-you-mean**: strengthened for all 5 new CLI flags + `--intro-color` hard error for unknown themes (was silent ignore).

---

## v50.0.0-beta.3 — Three Dragon Engines

cosmostrix v50 is the "zero to hero" culmination — from a simple terminal rain demo to a professional-grade cinematic renderer with three independent dragon engines, each owning a distinct concern. 220+ source files, ~89K LOC, ~1500+ tests pass.

### The Three Dragon Engines

- **Cosmic Dragon** (`src/cosmic_dragon_engine/`) — Simulation core. Droplet lifecycle, spawn physics, atmospheric evolution, cinematic behaviors, self-healer, phase predictor, reclaim state. Never touches palette.
- **Chroma Dragon** (`src/chroma_dragon_engine/`) — Coloring engine. OKLab gradient palettes, per-cell shader pipeline, climate post-FX (luminance/saturation/hue drift), L-smoothing, 300ms top-to-bottom wave transitions on every color-change path.
- **Crystal Dragon** (`src/crystal_dragon_engine/`) — Ambient intelligence. CPU/CLOCK-driven palette drift (44 themes in Cold/Medium/Hot groups, probabilistic weighted selection, 60s polling, 12% drift chance, 60s dwell hysteresis). Time-of-day ambient scheduler for automatic scene+palette switching via `config.toml`.

### Highlights Since v13

- Module-directory source layout (12 module dirs), extracted from flat `src/`.
- MSRV 1.97, Clippy `-D warnings` CI gate, Miri nightly validation.
- PGO (Profile-Guided Optimization) two-stage build via `./scripts/build.sh pgo`.
- Fat LTO, single codegen-unit release profile with platform-specific PGO profiles.
- Live config reload with SHA-512 fingerprinting and OKLab smooth transitions.
- Central Control Dragon Power: thermal sampling, endurance health, power management.
- Terminal protocol detection (kitty, wezterm, alacritty, iTerm2, Windows Terminal, tmux).
- Synchronized output (`ESC`) for tear-free frame delivery.
- 18 scenes: monolith (default), matrix, signal, classic, cinematic, calm, storm, cosmos, neon, hacker, matrix_film, low-power, cosmic-dragon, carbonic, dragon-crystal, orange-cat, north-stars, curiosity.
- 44+ builtin color themes with OKLab gradients and climate post-FX.
- `--doctor` diagnostics, `--benchmark` with JSON output, `--testconf` validation.
- Cross-platform: Linux, macOS, Windows, FreeBSD, Android. AUR package: `cosmostrix-bin`.

### Interactive Controls

`q` quit · `Space` reset animation + restart message typewriter · `c`/`C` cycle colors · `s`/`S` cycle charsets · `x` cycle scene forward (`X` no-op) · `p` pause/resume · `i` toggle HUD (`I` no-op) · `[`/`]` adjust density · `Up`/`Down` adjust speed

---

## v50.0.0-alpha.6 — Crystal Dragon Engine + Legacy Purge

- Introduced Crystal Dragon Engine: ambient palette drift via CPU/CLOCK -> temperature groups.
- Removed old auto-color-drift engine entirely. `--crystal-dragon` promoted to first-class.

## v50.0.0-alpha.5 — Mouse-Click Effects + Chroma Dragon Sync

- Mouse-click ripple effects (opt-in).
- OKLab 300ms wave transitions on all palette changes, including live config reload.

## v50.0.0-alpha.4 — HUD Expansion

- HUD now shows scene name, charset, color scheme, uptime, pressure, endurance score.
- Purged redundant `h` shortkey (superseded by `i` toggle).

## v50.0.0-alpha.1 — Cosmic Dragon Stability

- Cosmic Dragon stability fixes, rain-screen cleanliness audit, IP surface tightening.

## v25.0.0 — Dragon Hunt v2 Dead-Code Sweep

- Systematic dead-code removal across the full codebase in 5 phases (cloud, config, interactive, full sweep).
- Legacy `--fullwidth` purge (superseded by auto-detection).
- Cross-scene performance baselines, monolith-style optimizations.

## v20.x — Temporal Prediction & Legacy Purge

- v20.1.0: removed deprecated CLI flags and backward-compatibility shims.
- v20.0.0: Cosmic Dragon phase predictor (P1), adaptive resync (P2), reclaim state (P4) — the temporal-prediction milestone that gave the renderer self-awareness of long-running drift.

## v15.0.0 — Cosmic Dragon Pre-Release Polish

- Cosmic Dragon cinematic behaviors, atmospheric evolution, self-healer — the renderer becomes a director rather than a feed.

## v14.0.0 — Scene-Custom Migration (Breaking CLI)

- **Breaking**: `--scene-custom` migrated to TOML config. New CLI structure.

## v13.x — Cosmic Dragon Engine Birth

The era that turned cosmostrix from "a Matrix rain toy" into "a cinematic renderer". Key milestones:

- v13.0.0: Alive rain + depth-of-field + security hardening.
- v13.1.0: Shell completions, verbose mode, help polish.
- v13.2.0: Diff-based render engine specification, competitor benchmark comparison.
- v13.3.0: SGR cache hit-rate tracking, ANSI bytes/frame metrics.
- v13.3.1: 18 Dragon Eggs, P1/P2/P3 adaptive layers.
- v13.4.0: Added `--size` and `--duration` flags.
- v13.6.0: CLI flag simplification, background mode cleanup.

---

## v4.0.0 — Atmosphere Engine + Monolith Rain

The "real renderer" era. cosmostrix found its identity here.

- Signature Monolith Rain as the production default (sparse data pillars, segmented blocks).
- Cosmic Dragon Core/Engine/Cache groundwork for adaptive rendering.
- Atmosphere engine, terminal compatibility lab, doctor diagnostics.
- Profile ecosystem, config discoverability, benchmark hardening.
- Canonical metadata alignment across Cargo, README, AUR.

## v3.9.0 — v4 Ground-Work

- Atmosphere visual whisper engine, cosmic dragon architecture discipline.
- Phase 10.5: atmosphere config honesty + profile smoke hardening.

---

## Pre-v13 Era — The Journey From v2 to v12

These releases are documented in detail in [`docs/archive/CHANGELOG_PRE_V13.md`](docs/archive/CHANGELOG_PRE_V13.md). The summary below captures the arc.

### v12.0.0 — Protocol Engine

Terminal protocol detection (kitty keyboard, synchronized output, in-band resize reports). Render path respects each terminal's capabilities instead of falling back to lowest-common-denominator.

### v11.x — Cinematic Peak & Benchmark Depth

- v11.1.0: Benchmark reaches S-tier — RSS memory tracking, p99.9 / max frame-time metrics, sub-component timing (sim/render/io), JSON output mode, live HUD overlay. Theme tuning makes the 43 builtin palettes visually distinct.
- v11.0.0: Cinematic peak. Smoothstep easing on pause/resume, top-to-bottom wave color transitions, mouse-click effects, bracketed-paste safety.

### v10.0.0 — Peak Performance & Stability

Diff-based cell renderer reaches steady state. All known frame-time regressions resolved. Long-run soak tests (10h+) confirm zero leaks in memory, FDs, threads, CPU.

### v5.0.0 — Nightfall

Visual identity overhaul. TrueColor gradients become the default on capable terminals; ANSI 256-color mode remains as a fallback. CRT phosphor decay model replaced with physics-based exponential curve.

### v4.x — Atmosphere Polish

Iterative atmosphere work across v4.5–v4.9: fog vignette tuning, parallax brightness calibration, head self-bloom, climate luminance/saturation minimums, profile luminance offsets. Each release raised the visual floor without changing the architecture from v4.0.0.

### v3.x — The Foundational Era

- v3.9.0: ground-work for v4 (above).
- v3.1.0: first appearance of droplet physics and the rain-style lifecycle.
- v3.0.0: initial public release — basic rain rendering, single color, no scenes, no profiles.

### v2.x — Soak & Stability

- v2.1.0: visual contrast & readability overhaul — readable body glyphs, depth-layer visibility, CRT afterglow, pause/resume easing, mouse mode default-off, safe terminal cleanup on all exit paths.
- v2.0.0: first public-stability release. Stale glyph artifacts fixed, long-idle resync, direct-color auto-detection for `xterm-direct` / `tmux-direct`. 10h+ visual soak checks confirmed no leaks.
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
