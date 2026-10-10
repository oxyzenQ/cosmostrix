# Changelog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

cosmostrix uses [SemVer](https://semver.org/). Git tags use a leading `v` (e.g. `v50.0.0`).

The changelog is split into per-era files so this one stays navigable
(first pass plus completion, both in NIGHT-docs-1; the former
monolith carried every entry since v50.0.0-beta.6 inside a single
Unreleased section, and the stable bump of 2026-09-14 never cut it):

- Unreleased (post-v100.0.6 work, 2026-10-10 onward) — this file, below.
- [CHANGELOG-V100.0.6-ERA.md](docs/archive/CHANGELOG-V100.0.6-ERA.md) — the v100.0.x patch line that carried v100.0.0 stable through v100.0.6: the post-stable maintenance arc (2026-09-14 to 2026-10-10).
- [CHANGELOG-V100-ERA.md](docs/archive/CHANGELOG-V100-ERA.md) — the v100 line that built v100.0.0 stable: the nightly.1 hunts (2026-09-04 to 2026-09-10), the beta.1 long-horizon hardening (2026-09-10 to 2026-09-14), and the rc.1 candidate (2026-09-14).
- [CHANGELOG-V80-ERA.md](docs/archive/CHANGELOG-V80-ERA.md) — the v80 line: S-master hunts, the crystal dragon, and the Z-master harmony campaigns (2026-08-30 to 2026-09-03).
- [CHANGELOG-V50-ERA.md](docs/archive/CHANGELOG-V50-ERA.md) — the v50.0.0 pre-release line plus the condensed v13-v25 releases.
- [Pre-v13 archive](docs/archive/CHANGELOG_PRE_V13.md) — pre-v13 history (v2 to v12).

All four era files moved under `docs/archive/` (the first three in
NIGHT-dinner-4, the v100.0.6 patch era in night-improve-5 — the root
stays a single live `CHANGELOG.md`, exactly like every other
historical record).

The condensed origin story stays at the bottom of this file. The
`## v4.0.0` and `## v3.9.0` headings are tripwire-locked by
`test/docs_tests/metadata.rs` and must remain in this file (see the
tripwire note in the pre-v13 archive).

---

## Unreleased

The next release cycle starts here: entries land in this section as
they merge, and move to a per-era archive file under `docs/archive/`
when the version line closes.

### build: night-improve-4 — the Rust edition upgrades 2021 to 2024: the language bump lands across both crates with zero behavior change

- Both Cargo.toml manifests (root crate and pgo-runner) move to
  `edition = "2024"`. MSRV stays at 1.98 — the edition 2024 floor is
  1.85, so the pinned toolchain and the rust-version sync gate are
  untouched.
- The `gen` reserved keyword collision: Frame's content-generation
  field (`gen`), its local aliases, and the thirteen type_rain engine
  consumers become the raw identifier `r#gen` (18 files, 80 tokens).
  Compound names (`cell_gen`, `drawn_gen`, `gen_counter`) are
  untouched by construction.
- Edition 2024 marks `env::set_var` / `env::remove_var` unsafe: all
  160 call sites in the test suite are wrapped in `unsafe` blocks with
  house-style SAFETY comments (69 blocks across 10 test files). No
  production call site exists.
- Edition 2024 pattern ergonomics: the phosphor tests' tuple filters
  take explicit reference patterns (`|&(&energy, &ch)|`), the config
  drain drops the `ref` modifier, and two `contains(i)` calls gain the
  now-explicit borrow.
- `unsafe_op_in_unsafe_fn` hygiene: `hint_reclaim_pages` wraps its
  `madvise` call and the `TraceAlloc` GlobalAlloc forwarding wraps its
  `INNER` calls in inner `unsafe` blocks with SAFETY comments; the
  `tzset` extern block becomes `unsafe extern "C"`.
- Clippy's `collapsible_if` (if-let chains are stable in edition
  2024) auto-collapsed across src and tests via `cargo clippy --fix`.
- Verification: `cargo check --all-targets` green, `cargo clippy
  --all-targets` zero warnings, `cargo fmt --all` clean, and the
  standalone build-script suite passes 10/10 under
  `rustc --edition 2024 --test build.rs`. Compiler-driven migration
  end to end — no behavior change intended; the A/B benchmark pair
  stays the next-task gate.

### cli: night-improve-2 — the calm grey grammar tier for the -h/--help output: every flag spelling (definition rows and inline prose mentions) rides the zelynic grey law

- Flags are grammar — "context, not content" (the zelynic flag-row
  law): every flag spelling in the help now renders in the calm grey
  #8B8B8B tier. Definition rows (`-c, --color <name>` — the whole
  spelling including its value hint rides inside the span, the indent
  never does) and inline prose mentions ("See --list-colors.",
  "(e.g. -mfss) is rejected") share one tier, so both help surfaces
  (the clap-rendered short help and the colorize_help full reference)
  agree: clap's literal style joins the same tier.
- New `src/output/grey.rs` module (one concern, one module —
  `output/mod.rs` sits 7 lines under its hard LOC cap): the
  four-rung capability ladder pins truecolor #8B8B8B, 256-color
  index 245 (the same rung zelynic pins), ANSI bright black (90) on
  16-color terminals, and plain text on mono/piped (piped bytes stay
  plain — verified: the piped --help carries zero escape bytes).
- The inline matcher is a hand-rolled scanner, not a regex (no new
  dependency): word-boundary anchored, letter required after the
  dashes — kill -9, ranges like 0.0-3.0, and undashed names never
  grey. Splice indices land on ASCII boundaries, so the em-dash-rich
  prose splices UTF-8-safe.
- Section headings keep bold brand purple, runnable examples keep
  bold Matrix green (NIGHT-boost-2) — one tier per semantic, no
  mixed inline paint.
- Verification: cargo check --all-targets green, clippy --all-targets
  zero warnings, fmt clean; 8 colorize_help + 2 grey + 4 help_detail
  honesty + 36 output tests pass; check-rs-loc / check-comment-style
  / check-headers / emoji / language audits green; render-path sanity
  bench unchanged (fps in band, alloc_calls 563 identical, entropy
  and gini identical — the change never touches the rain path).

### docs: night-improve-5 — the v100.0.6 era trimmed to its archive file: the live CHANGELOG.md restarts fresh for the upcoming version

- The post-stable maintenance arc (2026-09-14 to 2026-10-10: the
  NIGHT-dinner release-craft series, the NIGHT-security / NIGHT-lts /
  NIGHT-perf hardening lines, the NIGHT-boost campaigns, and the
  post-v100 hunt, audit, docs, and cybersecurity rounds) moved
  verbatim to
  [CHANGELOG-V100.0.6-ERA.md](docs/archive/CHANGELOG-V100.0.6-ERA.md)
  — original order preserved, zero content edits, the same
  immutable-record convention the other era files follow.
- This header's era list now links the four archive files (v100.0.6,
  v100, v80, v50) plus the pre-v13 archive, and the "all three era
  files" note became "all four" with the night-improve-5 move
  recorded.
- The `## v4.0.0` / `## v3.9.0` condensed origin story stays in this
  file, tripwire-locked by `test/docs_tests/metadata.rs` — untouched.
- Docs-only change: no source, build, or test surface touched, so no
  A/B benchmark per house rule.

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
