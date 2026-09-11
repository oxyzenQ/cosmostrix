<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-depthtest-3 — the 64-char block-name blind spot + the config.toml line-1-to-end depth stress test

Owner report (2026-09-11, verbatim intent): a complete
`[scene-custom.tesssssssssssssss...]` block with a very long name
passed `--testconf` (expected: error at the 64-char limit) and never
appeared in `--list-scenes`, while a 4-char name listed fine — and
the owner suspected the same for the other custom-block systems.
The task brief then widened to a full end-to-end depth stress test of
config.toml live reloading, from line 1 (`scene = cinematic`) to the
end of the config.

Commit: `164d37d` (fix + unit tests), plus the e2e harness
`scripts/depthtest3_config_e2e.py` and this evidence trail.

## The defect (owner repro)

All three custom-block collectors cap block names at 64 chars to
bound BTreeMap key allocation (v50.0.0-beta.6 LTS bounds):

- `scene_custom::collect_custom_scenes` — `SCENE_CUSTOM_MAX_NAME_LEN`
- `colors_custom::collect_colors_custom` — `COLORS_CUSTOM_MAX_NAME_LEN`
- `charset_custom::collect_charset_custom` — `CHARSET_CUSTOM_MAX_NAME_LEN`

The cap was a **silent `continue`**. That made a complete, valid
block with a 65+-char name invisible to every consumer of the
collected map — and the validation layer iterates exactly those
maps:

| surface | verdict before the fix |
|---|---|
| `--testconf` | **PASS** — the block-level validators (`validate_scene_custom_completeness`, `validate_colors_custom_blocks`) iterate the COLLECTED map, which never contained the block |
| `--list-scenes` / `--list-colors` / `--list-charsets` | section absent — the listing iterates the same collected map; zero signal |
| startup `scene = <oversized>` reference | fatal exit 2 — but with a MISLEADING "unknown scene" (the scene lookup uses the collected map) |
| `--testconf` value scan for `scene = <oversized>` | PASS — `validate_field_value_with_cfg`'s scene arm scans RAW keys (`k.split('.').nth(1)`), so the reference was blessed |
| live-reload watcher | same as startup Layer 3 — silent for an unreferenced block |

That is the split-verdict species (testconf PASS / startup fatal /
listing invisible) the project hunts — with the extra twist that the
KEY itself was classified "known" (`is_valid_profile_name` checks
charset but not length), so no unknown-key error fired either. The
block was simply nowhere.

The hunt-24 round had already documented this exact edge as "known,
deliberately out of scope"
(docs/research/NIGHT_HUNTER_24_VALIDATION_CONTRACT.md — "Known edge").
The owner's repro promoted it to a defect.

## The fix (uniform-rejection contract)

Raw-key pre-scans front-load the three block-level validators, so
every surface that already called them rejects in lockstep:

- `scene_custom::validate_scene_custom_name_len` — pre-scan inside
  `validate_scene_custom_completeness` (extracted to
  `src/scene_custom/name_len.rs`, 800-LOC cap rule). Reached from
  `testconf::run` AND `validate_config_strictly` — covering
  `--testconf`, startup Layer 3, and the live-reload watcher.
- `colors_custom::validate_colors_custom_name_len` — pre-scan inside
  `validate_colors_custom_blocks` (extracted to
  `src/engine/chroma_dragon_engine/colors_custom/name_len.rs`).
  Same two entry points.
- `charset_custom::validate_charset_custom_name_len` — new validator
  wired into both entry points directly.

Shape discipline: only keys the collector would accept are scanned
(`scene-custom.<name>.<known-field>`, `colors-custom.<name>.<bg|rain
|stops>`, `charset-custom.<name>.set`); malformed keys and unknown
fields keep their existing error paths. Iteration is sorted +
deduplicated, so the reported name is deterministic across hash
seeds (the same first-error contract as every validator in the
layer). Exactly 64 chars stays legal — boundary pinned by tests.

CLI dead-ends now name the limit instead of a generic "unknown":

- `--scene-custom <65+>` — `apply_scene_custom_layer` errors with
  the char count and the 64-char limit (previously "unknown custom
  scene", hiding why the name can never match).
- `--scene <65+>` and `--show-scene <65+>` — the unknown-scene error
  carries a note (built by
  `config_apply_diagnostics::scene_length_limit_note`).
- `--colors-custom <65+>` and the `--charset` built-in path — same
  note shape.

List-printer visibility: `--list-scenes` / `--list-colors` /
`--list-charsets` append a `hidden: <name> is N chars — exceeds the
64-char name limit` warning line for collector-dropped blocks, and
the CUSTOM sections now print even when every defined block is
hidden (the owner's exact case: only the oversized block exists).
The list printers stay non-strict by design — they must list even
when unrelated keys are broken.

The collectors themselves are unchanged: the runtime caps, the
BTreeMap bounds, and the collector-skip contract tests all stay
intact. The fix is a gate IN FRONT of the skip, not a new
resolution policy.

## The e2e depth stress test (the flagship deliverable)

`scripts/depthtest3_config_e2e.py` (755, ruff clean) runs 44
expectations on the real binary through a PTY. Sources of truth: the
ANSI 24-bit SGR stream, the `--verbose` startup dump, and the
`--verbose` **final runtime state** (the post-live-reload truth at
exit — the strongest oracle available: a key that appears in the
FINAL state but not the startup dump proves the watcher applied the
edit).

- **Phase A (full-key soak)**: a canonical config with EVERY
  top-level user key plus all four block namespaces, line 1 =
  `scene = "cinematic"` (the owner's line-1-to-end brief).
  `--testconf` PASS + 14 applied-value assertions from the startup
  dump + exit 0.
- **Phase B (name-length contract)**: 3 namespaces x (testconf
  reject + startup reject) + live-reload reject + boundary-64 PASS +
  CLI limit error + `--list-scenes` hidden warning.
- **Phase C (live-reload depth)**: staggered mid-run edits; the
  final runtime state must show the EDITED values for the numeric
  group (fps 42→12, density 0.40→1.20, speed 30→8), the enum group
  (color aurora→gold + hue-classified SGR shift, charset
  binary→katakana + half-width-katakana census in the decoded
  stream, msg-fill-style engrave→radar, glitch subtle→none), and
  the overlay/scene group (message text swap + scene
  cinematic→matrix, rendered text verified in the stream).
- **Phase D (reject depth)**: mid-run out-of-range fps edit →
  watcher reject exit 2; mid-run oversized-name edit → watcher
  reject exit 2 with the limit message (the OLD binary kept running
  and silently ignored it).

Result: **44/44 PASS**.

### Harness pitfalls found while building it (methodology notes)

- cosmostrix's katakana pool is HALF-WIDTH katakana (U+FF66–U+FF9D,
  `build_chars` pushes 0xFF66..0xFF9D), not full-width U+30A0–30FF.
  A census keyed on the full-width range reports zero glyphs and
  looks like a renderer bug. Pinned in the harness regex.
- Top-level keys written AFTER a `[section]` header land INSIDE that
  section (TOML scoping) — the forgiving parser auto-promotes or
  flags unknown keys, and Layer 2 (unknown keys) fires BEFORE Layer
  3 (strict values), masking the length error under test. The
  harness writes top-level keys first.
- `ambient.HH-MM = "<scene>"` is a top-level KEY, not a
  `[ambient.HH-MM]` section.
- The `--verbose` final-state report only renders in a PTY session
  (headless runs exit before the report; `bold` is a 0/1/2 enum, not
  a TOML boolean — `bold = true` correctly rejects).
- The live-reload debug trace (`COSMOSTRIX_LIVE_RELOAD_DEBUG=1`)
  proved the charset edit path end-to-end: `apply charset='katakana'
  (built-in)` → `Cloud rebuilt` → `config diff: charset: binary →
  katakana` — used to separate a harness bug (wrong regex) from a
  product bug (none).

## Verification

- 17 new unit tests (owner repro shape, boundary 64, multi-block
  counting, unknown-field non-interference, CLI dead-end, strict
  lockstep x3, list helper x2, collector-vs-gate separation); full
  suite **2863/2863**.
- `build.sh check -q` exit 0 (light path — check-all exceeds the
  2-minute local kill limit on this machine; CI owns the heavy
  matrix), `cargo clippy --all-targets --all-features -D warnings`
  clean, `cargo fmt` clean, gate-keepers **16/16** (shellcheck
  installed this session).
- A/B 10 s benchmark (same-pipeline methodology: baseline 9c36a04
  rebuilt via `cargo build --release` in a separate worktree vs HEAD
  164d37d; scenes cinematic + sorgonemous_intrascals; evidence in
  `benchmark/bench-labs/depthtest3_ab/`): **FLAT** — dirty cells
  ±0.22%, frame entropy ±0.11%, density gini ±0.27% (contract 1%,
  reproducibility floor ~0.13%); fps within the same-commit rebuild
  noise band (same-binary repeats swing p95 by up to 9% on the
  cinematic tail, so the single-run p95 deltas are noise, and the
  3-run B-side table is preserved in the JSON set). Expected: the
  change is config-validation cold path, zero per-frame code.

## References

- docs/RULES.md — the unified bounds table's name-length row now
  documents the hard-error semantics.
- docs/research/NIGHT_HUNTER_24_VALIDATION_CONTRACT.md — the "known
  edge" section is marked RESOLVED with this round.
- The collector skip-vs-gate separation is pinned by the
  `collect_*_skips_oversized_names` tests (collector contract) and
  the `completeness_validation_rejects_oversized_name` family
  (validation contract).
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
