# Flags Audit: `--bench-frames`, `--chars`, `--bold`

<!-- SPDX-License-Identifier: GPL-3.0-only -->

> **STATUS: EXECUTED** (commit `69ca2c6`, 2026-08-04).
>
> All three recommendations were applied:
>
> - **`--bench-frames`**: KEPT + 2 fixes (docs bug in `benchmark/README.md:339-356`,
>   warn-matrix bug in `main.rs::collect_bench_noop_warnings` case 4).
> - **`--chars`**: REMOVED. clap arg, parsing block, `parse_user_hex_chars` fn + test,
>   doctor display, help block all deleted. `user_ranges: Vec<(char,char)>` plumbing
>   kept (always-empty Vec) for runtime signature stability. Migration entry added
>   to `REMOVED_FLAGS` in `src/validation.rs`.
> - **`--bold`**: KEPT + 1 bug fix (silent error-swallowing — wildcard
>   `_ => BoldMode::Random` was eating `Err` from `validate_u8_range`. Wrapped in
>   `ux::or_exit(...)`. Same fix applied to `--shadingmode` which had the identical bug.)
>
> Test delta: 1533 → 1532 pass (`parse_user_hex_chars_parses_hex_codepoints` deleted).
>
> The original audit findings are preserved below for historical reference.

> **Original status (pre-execution)**: Owner-decision pending. No code changes
> performed — this was a read-only audit. The previous removal commit (`9598f37`)
> was reverted in `3f733ae` to restore the flags to their pre-removal state so the
> owner could decide per-flag what to do.
>
> **Scope**: For each flag, document (1) what it does, (2) every call site
> and value-flow, (3) what alternatives already exist (config file, CLI
> equivalents, scene presets), (4) blast radius of removal, (5) keep vs
> remove recommendation with rationale.

## Summary table

| Flag | CLI | Config key? | Documented? | Live-reload? | Used by tooling | Recommendation |
|------|-----|-------------|-------------|--------------|-----------------|----------------|
| `--bench-frames N` | `hide = true`, in `print_help()` block | No | Yes (BENCHMARKING.md §2, benchmark/README.md, benchmark.sh) | n/a (one-shot mode) | `benchmark/benchmark.sh` (5 call sites), hyperfine.md, README.md examples | **KEEP** — distinct use case, CI infrastructure depends on it |
| `--chars <ranges>` | `hide = true`, in `print_help()` block | No (not in `USER_CONFIG_KEYS`) | Yes (help_detail.rs:333, bench_report.rs:65) | No | None (no script uses it) | **REMOVE** — fully superseded by `[charset-custom.<name>]` block, redundant plumbing |
| `-b, --bold <0\|1\|2>` | `hide = true`, in `print_help()` block | Yes (`bold = 1` in USER_CONFIG_KEYS) | Yes (help_detail.rs:319, configfile.rs:610, BENCHMARKING.md) | Yes | None (no script uses it) | **KEEP** — has short letter `-b`, is a USER_CONFIG_KEY, live-reloadable, distinct semantic (0/1/2 enum, not duplicable via scene) |

---

## 1. `--bench-frames N`

### 1.1 What it does

Legacy CI/regression benchmark. Runs N headless frames in a tight loop and
prints compact parseable `BENCH:` output. Distinct from `--benchmark`
(premium 5s user-facing benchmark with full Report-engine output) and
`--bench-all` (scaling sweep across screen sizes).

### 1.2 Call sites & value flow

| File | Lines | Role |
|------|-------|------|
| `src/config.rs:614-625` | clap arg definition | `Option<u64>` with `value_parser!(u64).range(1..)` (rejects 0 at parse time — Phase 5 P3-6) |
| `src/app.rs:57-58, 284-285` | `CloudConfig` field + clone_config plumbing | Carried in `CloudConfig`, propagated through `clone_config` |
| `src/main.rs:1003` | Args → CloudConfig | `bench_frames: args.bench_frames` |
| `src/main.rs:1099-1102` | Entry point dispatch | `if let Some(_bench_frames) = args.bench_frames { warn_bench_noop_flags(&args, fps_user_set); return bench::run_benchmark(&cloud_cfg); }` — note: `--benchmark` and `--bench-all` take precedence (see warn table below) |
| `src/main.rs:1264-1272` | Conflict-warning matrix | Warns when `--bench-frames` is combined with `--bench-all`, `--benchmark`, or `--bench-duration` |
| `src/bench.rs:179-243` | `run_benchmark` impl | Consumes `cfg.bench_frames.expect(...)`, runs warmup + measurement loop, prints `BENCH:` block |
| `src/bench.rs:208-209` | Warmup derivation | `warmup_frames = (bench_frames / BENCH_WARMUP_DIVISOR).clamp(MIN, MAX)` |
| `src/constants.rs:393-413` | Warmup constants | `BENCH_WARMUP_DIVISOR=10`, `MIN=10`, `MAX=200` |
| `src/live_config.rs:1088` | Test fixture | `bench_frames: None` (default for non-bench tests) |
| `src/atmosphere_ab_tests.rs:318` | Test fixture | `bench_frames: None` |
| `src/interactive/intro.rs:125-127` | Doc comment | Notes that intro is skipped in bench mode |
| `src/cli_parse.rs:14` | Doc comment | Notes `--duration` is noop in `--bench-frames` mode |
| `src/bolt.rs:16-17` | Doc comment | Notes that BOLT gains are not measurable via `--bench-frames` (lean path already table-driven) |
| `src/interactive/tests.rs` | Test fixture | `bench_frames: None` |

### 1.3 External dependencies

`benchmark/benchmark.sh` (CRITICAL CI INFRASTRUCTURE) — 5 call sites:

| Line | Use |
|------|-----|
| 51 | Calibration: `--bench-frames $CALIB_FRAMES` (CALIB_FRAMES=10000 default) to derive fps, then compute BENCH_FRAMES = fps × target_secs |
| 75 | Hyperfine: release binary `--bench-frames $BENCH_FRAMES` |
| 76 | Hyperfine: pro-native binary `--bench-frames $BENCH_FRAMES` |
| 91, 93 | `/usr/bin/time -v` measurements (release + pro-native) |
| 101, 103 | `perf stat -d` measurements (release + pro-native) |
| 115, 122 | Valgrind massif (using `MASSIF_FRAMES`, derived from BENCH_FRAMES) |

`benchmark/README.md:22-24, 1119` — documents the flag as the
frame-count-based CI/regression benchmark.

`benchmark/hyperfine.md` — generated by `benchmark.sh`, references the flag.

`docs/BENCHMARKING.md:60` — table row: "Legacy CI benchmark. Runs N
headless frames, prints compact `BENCH:` output. Use when you want
frame-count-based measurement."

### 1.4 Documentation bug found during audit

`benchmark/README.md:339-356` shows 4 example commands that combine
`--benchmark` WITH `--bench-frames 30` (or 60). Per `src/main.rs:1267`:

```rust
if args.benchmark && args.bench_frames.is_some() {
    warns.push("--bench-frames ignored (--benchmark takes precedence)");
}
```

→ In all 4 examples, `--bench-frames` is **silently ignored**. The
examples work (they run `--benchmark`) but the `--bench-frames 30` token
is dead weight that misleads readers into thinking both flags combine.
**Not a flag-removal issue** — it's a docs fix that should be done
independently of the keep/remove decision.

### 1.5 Alternatives if removed

`--benchmark --bench-duration Ns` can replace time-based uses, but:

- Frame-count-based measurement (the whole point of `--bench-frames`)
  has no equivalent. `--bench-duration` is time-based.
- Output format differs: `--bench-frames` emits compact `BENCH:` block
  (grep-friendly); `--benchmark` emits multi-section Report-engine
  output (human-friendly but harder to parse in shell).
- `benchmark/benchmark.sh` parses `frames_per_s:` from the `BENCH:`
  block at line 52 — switching to `--benchmark` requires rewriting the
  parser to consume JSON (`--json`) and rewriting 5 call sites.
- Hyperfine comparisons rely on equal-work invocations; frame-count
  based is the natural unit for that.

### 1.6 Blast radius of removal

- **High**: `benchmark/benchmark.sh` breaks (5 call sites) — needs
  rewrite to use `--benchmark --json` + jq parsing.
- **Medium**: `benchmark/README.md` examples break.
- **Medium**: `docs/BENCHMARKING.md` table row + examples break.
- **Low**: `src/main.rs:1099-1102` entry-point dispatch deleted.
- **Low**: `src/bench.rs:179-243` `run_benchmark` function deleted
  (~65 LOC + warmup constants become orphaned).
- **Low**: `src/main.rs:1264-1272` conflict-warning matrix simplified.
- **Low**: Test fixtures (3 files) lose the `bench_frames: None` line.

### 1.7 Recommendation: **KEEP**

`--bench-frames` serves a distinct, well-defined use case
(frame-count-based CI benchmarking) that `--benchmark` cannot replace
without significant tooling rewrite. It is `hide = true` so it doesn't
clutter `--help` for end users, but is documented for CI authors. The
compact `BENCH:` output format is the contract that `benchmark.sh` and
external CI pipelines depend on. Removing it would break the project's
own benchmark script.

**Action items if KEPT** (independent fixes):

1. Fix `benchmark/README.md:339-356` — remove `--bench-frames 30` from
   examples that already use `--benchmark` (the flag is ignored there).

---

## 2. `--chars <ranges>`

### 2.1 What it does

Custom character pool override as hex Unicode ranges
(e.g. `--chars "0x30-0x39,0x41-0x5A"`). Pairs must be even count.
Feeds `user_ranges: Vec<(char, char)>` into `build_chars`.

### 2.2 Call sites & value flow

| File | Lines | Role |
|------|-------|------|
| `src/config.rs:692-693` | clap arg definition | `Option<String>`, `hide = true` |
| `src/main.rs:188` | Import | `use crate::charset::{build_chars, charset_from_str, parse_user_hex_chars};` |
| `src/main.rs:728-743` | Parsing block | Parses hex ranges → `Vec<(char, char)>` via `parse_user_hex_chars` |
| `src/main.rs:773` | Plumbing into `build_chars` | `build_chars(charset, &user_ranges, def_ascii)` |
| `src/main.rs:1022` | CloudConfig field | `user_ranges` propagated into cloud config |
| `src/charset.rs:44-89` | `parse_user_hex_chars` impl | Parses hex codepoints, rejects control chars + wide chars (Cosmic Dragon principle) |
| `src/charset.rs:265-269` | Unit test | `parse_user_hex_chars_parses_hex_codepoints` |
| `src/doctor.rs:341-342` | Diagnostic display | `s.field("chars_override", spec)` — shows the override string in `--doctor` output |
| `src/help_detail.rs:333-335` | Manual help text | Documents `--chars <ranges>` in `--help` output (despite clap `hide = true`) |
| `src/bench_report.rs:65-66` | Doc comment | References `--chars` as one of the charset override mechanisms |
| `src/cosmic_dragon_engine/cloud/scene_runtime.rs` | Plumbing (read-only) | Carries `user_ranges` through scene runtime |
| `src/interactive/event_loop.rs` | Plumbing (read-only) | Carries `user_ranges` through event loop |
| `src/interactive/input.rs` | Plumbing (read-only) | Carries `user_ranges` through input handler |
| `src/live_config.rs` | Plumbing (read-only) | Carries `user_ranges` through live config |
| `src/profile.rs` | Plumbing (read-only) | Carries `user_ranges` through profile system |
| `src/atmosphere_ab_tests.rs` | Test fixture | `user_ranges: Vec::new()` (empty) |
| `src/cli_parse.rs` | Doc reference | Notes that `--charset-file` was replaced by config-file flow |

**Important**: `user_ranges: Vec<(char, char)>` is plumbing that
persists even if `--chars` is removed. It flows through ~6 modules and
is always an empty Vec for users who don't pass `--chars`. Removing
the Vec entirely would touch ~15 call sites with zero functional
benefit — that's why the previous removal commit kept the Vec and
just nuked the CLI surface.

### 2.3 External dependencies

None. No script in `benchmark/` or `docs/` uses `--chars`. The flag is
purely a user-facing CLI affordance.

### 2.4 Alternatives that already exist

The `[charset-custom.<name>]` block in `config.toml` is the
**replacement** for `--chars`. From `src/configfile.rs:736-752`:

```toml
# [charset-custom.zen]
# set = "|"

# [charset-custom.greek-letters]
# set = "αβγδεζηθικλμνξοπρστυφχψω"
```

Loaded via `--charset <name>` or `charset = "name"` in config.toml.
Custom names take precedence over built-in presets with the same name.
Live-reloadable.

`--chars` is functionally redundant: anything achievable via
`--chars "0x30-0x39"` is achievable via:

```toml
[charset-custom.my-range]
set = "0123456789"
```

then `cosmostrix --charset my-range`. The config-file flow is also
strictly more powerful (supports literal characters, live-reload,
named presets, shareable across machines).

### 2.5 Documentation note

The `[charset-custom]` block was added specifically to replace
`--chars`. The previous `--charset-file` flag (which loaded chars from
a file) was removed with a migration message in
`src/validation.rs:68-71`:

```
error: --charset-file has been removed in v25.0.0.
  Custom charsets now live in config.toml under [charset-custom.<name>]
  and are loaded via --charset <name>.
```

`--chars` was left in place during migration as a "soft
deprecation" — but the migration message explicitly tells users
to move to `[charset-custom]`. The only reason `--chars` was kept was
that it accepted *hex Unicode ranges* (e.g. `0x30-0x39`) rather than
literal characters, which is a slightly different input format.
However, users can type the literal characters directly in the TOML
`set` field — Unicode is native to TOML.

### 2.6 Blast radius of removal

- **Low**: `src/config.rs:692-693` clap arg definition deleted (2 LOC).
- **Low**: `src/main.rs:188` import — drop `parse_user_hex_chars` from the use list.
- **Low**: `src/main.rs:728-743` parsing block deleted (16 LOC).
- **Low**: `src/charset.rs:44-89` `parse_user_hex_chars` function deleted (46 LOC).
- **Low**: `src/charset.rs:265-269` unit test deleted (5 LOC).
- **Low**: `src/doctor.rs:341-342` `chars_override` display deleted (2 LOC).
- **Low**: `src/help_detail.rs:333-335` `--chars` help block deleted (3 LOC).
- **Low**: `src/bench_report.rs:65-66` doc-comment updated to remove `--chars` reference (1 LOC).
- **Zero**: `user_ranges: Vec<(char, char)>` plumbing STAYS (always empty Vec). Removing it would touch ~15 call sites with zero functional benefit.
- **Low**: Add `REMOVED_FLAGS` entry in `src/validation.rs:23` with migration instructions pointing to `[charset-custom]`.
- **Zero**: Test impact — `parse_user_hex_chars_parses_hex_codepoints` is the only test that touches this code path.

Total LOC delta: ~75 lines deleted, ~5 lines added (REMOVED_FLAGS entry).

### 2.7 Recommendation: **REMOVE**

`--chars` is fully superseded by `[charset-custom.<name>]` in
config.toml. The config-file flow is strictly more powerful (literal
chars, live-reload, named presets, shareable). `--chars` was a "soft
deprecation" leftover from migration — the release
notes already tell users to move to `[charset-custom]` via the
`--charset-file` removal message.

The hex-range input format (`0x30-0x39`) is a minor convenience that
doesn't justify maintaining duplicate parsing infrastructure (46 LOC
in `parse_user_hex_chars` + its unit test). Users can type literal
characters in TOML directly.

Removal is clean: ~75 LOC deleted, no test breakage beyond the
deleted unit test, no script breakage (no script uses it), no
config-file breakage (the `[charset-custom]` flow is unaffected).

---

## 3. `-b, --bold <0|1|2>`

### 3.1 What it does

Bold style: 0=off, 1=random (default), 2=all. Controls whether the
renderer emits bold SGR escape sequences per glyph. Affects glyph
weight, which has a small but measurable impact on terminal rendering
throughput (bold glyphs use a different SGR sequence).

### 3.2 Call sites & value flow

| File | Lines | Role |
|------|-------|------|
| `src/config.rs:583-590` | clap arg definition | `u8`, `short = 'b'`, `default_value_t = 1`, `hide = true` |
| `src/configfile.rs:41` | USER_CONFIG_KEYS | `"bold"` is a recognized top-level config key |
| `src/configfile.rs:610-611` | Dump-config template | `# bold = 1` shown as commented-out default |
| `src/config_apply.rs:453-458` | Config → Args | `args.bold = parse_u8_config("bold", &v, 0, 2)` — config value flows into CLI args |
| `src/main.rs:635-639` | Args → BoldMode | `validate_u8_range("--bold", args.bold, 0, 2)` → `BoldMode::Off/All/Random` |
| `src/cosmic_dragon_engine/runtime.rs:18-23` | BoldMode enum | `Off`, `Random`, `All` |
| `src/cosmic_dragon_engine/cloud/render.rs:67, 333` | DrawCtx field | `bold_mode: BoldMode` carried in render context |
| `src/cosmic_dragon_engine/cloud/rain.rs:603` | RainCtx field | `bold_mode` carried in rain context |
| `src/chroma_dragon_engine/shaders/base.rs:79` | Shader field | `bold_mode: BoldMode` |
| `src/chroma_dragon_engine/shaders/base.rs:449-450, 621-625` | Shader logic | `match shader.bold_mode { Off => bold = false, All => bold = true, Random => bold = ((line ^ val) % 2) == 1 }` |
| `src/cosmic_dragon_engine/cloud/monolith.rs` | Monolith rendering | Uses `bold_mode` |
| `src/cosmic_dragon_engine/cloud/phosphor.rs` | Phosphor decay | Uses `bold_mode` |
| `src/terminal.rs` | Terminal output | Uses `bold_mode` for SGR emission |
| `src/bench_io.rs` | Bench I/O | Uses `bold_mode` for SGR emission in benchmark mode |
| `src/bench_report.rs:81-84, 358` | Bench report | Reports `bold_mode` as a CONFIG field (affects throughput) |
| `src/bench_json.rs` | JSON output | Includes `bold_mode` in JSON report |
| `src/verbose.rs` | Verbose dump | Shows `bold_mode` in `--verbose` |
| `src/config_hints.rs:16, 43-56, 284-286, 295-499` | Hints | Special hint for `color.tune.bold` (wrong location + wrong value type — `bold` is 0/1/2 enum, not boolean) |
| `src/live_config.rs:1063, 1319-1330` | Live reload + tests | `bold` is live-reloadable via USER_CONFIG_KEYS; test fixture uses `color.tune.bold` to verify hint system |
| `src/help_detail.rs:319-321` | Manual help text | `-b, --bold <0|1|2>` documented in `--help` |
| `docs/CENTRAL_CONTROL_RAINS_USAGE.md` | Usage doc | Bold mode row in the config table |
| `src/cosmic_dragon_engine/cloud/tests/*` (8 files) | Test fixtures | Tests pass `bold_mode: BoldMode::Random` (or specific values) to verify rendering behavior |
| `src/interactive/tests.rs` | Test fixtures | Uses `bold_mode` |
| `src/atmosphere_ab_tests.rs` | Test fixtures | Uses `bold_mode` |
| `src/app.rs` | CloudConfig field | `bold_mode` propagated |
| `src/main.rs:194` | Import | `use crate::runtime::{BoldMode, ShadingMode};` |

### 3.3 External dependencies

None for scripts. `--bold` is a user-facing CLI flag + config key
combination.

### 3.4 Distinct from `--chars`: `--bold` has 3 integration surfaces

1. **CLI** (`-b`, `--bold`): The `-b` short letter is one of only two
   short-letter flags in the entire CLI (the other is `-c` for `--color`).
   This is premium "ergonomic" positioning — short letters are reserved
   for the most-used flags. Removing `-b` without a replacement would
   be a UX regression for users who actually use bold-mode tuning.

2. **Config key** (`bold = <0|1|2>` in config.toml): In
   `USER_CONFIG_KEYS` list (line 41), so it's a first-class config
   key, not just a CLI affordance. `--dump-config` includes it as a
   commented-out default. Live-reloadable.

3. **Scene presets**: Not directly settable via scene presets, but the
   `BoldMode` enum is deeply integrated into the renderer (shaders,
   monolith, phosphor, terminal, bench_io). The enum STAYS even if
   the CLI/config surface is removed (the renderer still needs to know
   whether to emit bold SGR codes — it just defaults to Random
   permanently).

### 3.5 Alternatives if removed

**There is no equivalent config-key or scene-preset replacement for
bold-mode control.** Unlike `--chars` (which has `[charset-custom]`),
`--bold` is the ONLY way to control glyph boldness. Removing it means:

- `BoldMode::Random` becomes the permanent default (no off, no all).
- Users lose the ability to disable bold for terminals where bold
  rendering is slow or visually noisy.
- Users lose the ability to force all-bold for accessibility
  (low-vision users who want maximum glyph weight).
- The config-key `bold = 1` becomes a `REMOVED_FLAGS` entry.
- The `color.tune.bold` hint in `src/config_hints.rs` becomes dead
  code (no real key to hint about).
- ~8 test fixtures that pass `bold_mode: BoldMode::Off` or `All` for
  coverage would need to be rewritten to only test `Random`.

### 3.6 Blast radius of removal

- **Medium**: `src/config.rs:583-590` clap arg deleted (8 LOC).
- **Medium**: `src/configfile.rs:41` `bold` removed from USER_CONFIG_KEYS.
- **Medium**: `src/configfile.rs:610-611` `# bold = 1` removed from dump-config.
- **Medium**: `src/config_apply.rs:453-458` config-apply block deleted (6 LOC).
- **Medium**: `src/main.rs:635-639` `validate_u8_range` + match deleted, replaced with `let bold_mode = BoldMode::Random;` (5 LOC → 1 LOC).
- **Medium**: `src/help_detail.rs:319-321` `-b, --bold` block deleted (3 LOC).
- **Medium**: `docs/CENTRAL_CONTROL_RAINS_USAGE.md` Bold mode row removed.
- **Medium**: Add `REMOVED_FLAGS` entry with migration instructions.
- **Low**: `src/config_hints.rs:54-55` special-case `if suffix == "bold"` branch deleted — the hint becomes "key not found" generic message.
- **Low**: `src/config_hints.rs:298-320, 478-499` migrate tests that use `bold` as a USER_CONFIG_KEYS fixture to `shadingmode` (another 0/1/2 enum with same semantics).
- **Low**: `src/live_config.rs:1319` test fixture migrated.
- **Low**: `src/config_apply_tests.rs:679` dump-config test fixture migrated.
- **Low**: `src/configfile_promotion_tests.rs:118` test fixture migrated.
- **Zero**: `BoldMode` enum STAYS (renderer still needs it). Only the CLI/config surface is removed.
- **Zero**: `src/cosmic_dragon_engine/cloud/render.rs`, `rain.rs`, `chroma_dragon_engine/shaders/base.rs`, etc. — unchanged (they consume `BoldMode`, not `args.bold`).

Total LOC delta: ~30 lines deleted, ~10 lines added (REMOVED_FLAGS + test fixture migrations).

### 3.7 The previous removal commit's approach (9598f37)

The reverted commit `9598f37` took exactly this approach: delete the
CLI/config surface, keep the `BoldMode` enum, migrate test fixtures
to `shadingmode`, simplify `main.rs:635-639` to `let bold_mode =
BoldMode::Random`. The commit message documents:

> `--bold` removal (medium complexity — has config-key equivalent):
>
> - Delete Args::bold field (u8 with -b short) in config.rs
> - Delete bold = 1 from USER_CONFIG_KEYS in configfile.rs
> - Delete config_apply.rs block that set args.bold from config
> - Delete bold = 1 from dump-config template in configfile.rs
> - Replace main.rs:635-639 (validate_u8_range + match) with
>   let bold_mode = BoldMode::Random (the permanent default)
> - ...
> - BoldMode enum (Off/All/Random) STAYS in runtime.rs — still used
>   by renderer + tests. Only the CLI/config surface is gone.
>
> Verification:
>
> - 1533 → 1531 tests PASS (-2: parse_user_hex_chars_parses_hex_codepoints,
>   color_tune_bold_hint_warns_about_value_type; both tests' fixtures
>   became invalid after their target flag/key was removed)
> - clippy clean, fmt clean, LOC cap clean, headers clean, version-sync clean

Note: the commit message says "has config-key equivalent" — but
**that's misleading**. The config key `bold = 1` IS the same surface
as `--bold 1`, just spelled differently. There is no *separate*
mechanism that replaces bold-mode control. Removing both means users
lose the ability to control bold entirely. The commit message
conflated "has a config-key alias" with "has a replacement".

### 3.8 Recommendation: **KEEP**

`--bold` is the **only** way to control glyph boldness. Unlike
`--chars` (which has `[charset-custom]` as a strict superset
replacement), `--bold` has no replacement. Removing it permanently
locks users into `BoldMode::Random` with no recourse for:

- Accessibility: low-vision users who want `BoldMode::All` for
  maximum glyph weight.
- Performance: users on slow terminals who want `BoldMode::Off` to
  skip bold SGR overhead.
- Aesthetics: users who want consistent non-bold rendering for
  screenshot/screen-recording purposes.

The `-b` short letter is also premium CLI real estate (one of only
two short letters in the entire CLI). Removing it without a
replacement is a UX regression.

**The `BoldMode` enum is deeply integrated** (8+ renderer files, 27
test fixtures) — keeping the CLI/config surface costs only ~30 LOC
and preserves user control. The cost of removal is high (lost user
control + test fixture migration + docs updates), the benefit is
minimal (30 LOC savings).

**Action items if KEPT** (independent improvements):

1. Consider promoting `--bold` out of `hide = true` so it appears in
   clap's auto-generated help (currently only in the manual
   `print_help()` text). Same applies to `--chars` if kept.
2. The `color.tune.bold` special-case hint in `config_hints.rs:54-55`
   is good UX — keep it.

---

## 4. Cross-flag observations

### 4.1 `hide = true` + manual `print_help()` is a pattern

All three flags use `hide = true` in clap AND appear in the manual
`print_help()` text in `help_detail.rs`. This is intentional: clap's
auto-generated help is bypassed entirely (the project uses a custom
`--help` flag that calls `help_detail::print_help()`), so `hide =
true` is technically a no-op for end-user display. The `hide = true`
is kept as a documentation marker that says "this flag is advanced,
don't promote it in future tooling".

### 4.2 The previous removal commit's misleading framing

Commit `9598f37`'s message framed both `--chars` and `--bold` as
"has config-key equivalent" / "has config-key equivalent" — implying
both have replacements. In reality:

- `--chars` → `[charset-custom.<name>]` is a **true replacement**
  (strict superset of functionality).
- `--bold` → `bold = 1` in config is **the same surface, just
  spelled differently** (no replacement, just an alias).

This conflation led to the premature removal of `--bold`. The
revert in `3f733ae` restores both flags so the owner can decide
per-flag based on this audit's findings.

### 4.3 `--bench-frames` is in a different category

`--bench-frames` is **CI infrastructure**, not a user-facing
affordance. It has no config-key equivalent (and shouldn't —
benchmark modes are one-shot CLI invocations, not persistent
settings). It is `hide = true` precisely because it's not for end
users — it's for `benchmark/benchmark.sh` and external CI pipelines.
Removing it would break the project's own benchmark script.

---

## 5. Final recommendations

| Flag | Recommendation | Confidence | Rationale |
|------|----------------|------------|----------|
| `--bench-frames` | **KEEP** | High | Distinct use case (frame-count CI benchmarking), no replacement, `benchmark/benchmark.sh` depends on it (5 call sites) |
| `--chars` | **REMOVE** | High | Fully superseded by `[charset-custom.<name>]` (strict superset), no script uses it, clean removal (~75 LOC) |
| `--bold` | **KEEP** | Medium-High | No replacement exists (only an alias), `-b` short letter is premium real estate, BoldMode enum deeply integrated, accessibility/perf use cases |

### 5.1 If the owner decides to remove `--chars` only

Re-apply the `--chars` portions of reverted commit `9598f37`:

- `src/config.rs:692-693` — delete clap arg
- `src/main.rs:188, 728-743` — delete import + parsing block
- `src/charset.rs:44-89, 265-269` — delete `parse_user_hex_chars` + test
- `src/doctor.rs:341-342` — delete `chars_override` display
- `src/help_detail.rs:333-335` — delete help block
- `src/bench_report.rs:65-66` — update doc comment
- `src/validation.rs` — add REMOVED_FLAGS entry

### 5.2 If the owner decides to remove `--bold` only

Re-apply the `--bold` portions of reverted commit `9598f37` (see
§3.7 above for the full change list). Note that this locks
`BoldMode::Random` as the permanent default with no user override.

### 5.3 If the owner decides to keep all three

No code changes needed. Independent improvements that could be done:

1. Fix `benchmark/README.md:339-356` docs bug (dead `--bench-frames`
   tokens in `--benchmark` examples).
2. Consider promoting `--bold` out of `hide = true` (it's a
   legitimate user-facing flag with a short letter).
3. Consider promoting `--chars` out of `hide = true` OR adding a
   deprecation warning when it's used (pointing users to
   `[charset-custom]`).

---

## Appendix A: Verification commands

```bash
# Re-verify the revert is clean
# (run from the cosmostrix repo root)
git log --oneline -3
# Expected:
# 3f733ae Revert "refactor: remove --chars and --bold CLI flags (owner instruction)"
# 9598f37 refactor: remove --chars and --bold CLI flags (owner instruction)
# 7fbdba3 fix: mouse-click wave pool — double-click no longer resets in-flight wave

# Verify compile
cargo check

# Verify all 1533 tests pass (the 2 tests deleted in 9598f37 should be back)
cargo test --workspace 2>&1 | tail -5
```

## Appendix B: File reference

| File | Section(s) |
|------|------------|
| `src/config.rs` | §1.2, §2.2, §3.2 |
| `src/main.rs` | §1.2, §2.2, §3.2 |
| `src/app.rs` | §1.2, §3.2 |
| `src/bench.rs` | §1.2 |
| `src/constants.rs` | §1.2 |
| `src/charset.rs` | §2.2 |
| `src/doctor.rs` | §2.2 |
| `src/help_detail.rs` | §2.2, §3.2 |
| `src/bench_report.rs` | §2.2, §3.2 |
| `src/configfile.rs` | §3.2 |
| `src/config_apply.rs` | §3.2 |
| `src/cosmic_dragon_engine/runtime.rs` | §3.2 |
| `src/cosmic_dragon_engine/cloud/render.rs` | §3.2 |
| `src/cosmic_dragon_engine/cloud/rain.rs` | §3.2 |
| `src/chroma_dragon_engine/shaders/base.rs` | §3.2 |
| `src/config_hints.rs` | §3.2 |
| `src/live_config.rs` | §1.2, §3.2 |
| `src/validation.rs` | §2.6, §3.6 |
| `benchmark/benchmark.sh` | §1.3 |
| `benchmark/README.md` | §1.3, §1.4 |
| `docs/BENCHMARKING.md` | §1.3 |
| `docs/CENTRAL_CONTROL_RAINS_USAGE.md` | §3.2 |
