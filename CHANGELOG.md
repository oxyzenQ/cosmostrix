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

### infra: night-improve-1 — the scripts/ tree adopts the zelynic shape: root entry points, category directories, zero reference drift

- The scripts/ tree reorganizes to the zelynic layout (git mv, history
  and exec-bit preserved): `build.sh` and `gate-keepers.sh` become the
  two flat root entry points; `build/lib/` rises to `lib/`; the strict
  CI wrapper, the NDK resolver, and the tag-pipeline CI waiter group
  under `ci/`; version bumping (`version-to.sh`,
  `rust-version-to.sh`, `bump-rust-to.sh`) and the visual-preset
  battle tool group under `dev/`; `depthbore/` and the stress/e2e/hang
  harnesses merge under `depth/`; `setup/` becomes `package/`
  (install/uninstall — zelynic's package shape); the five corpus
  audits join their checker siblings in `gates/`. The zelynic folders
  with no cosmostrix counterpart (`sandbox/`, `supermassive/`,
  `setup.sh`) are deliberately not invented.
- Reference drift: zero. 64 live files rewired (5 workflows, build
  pipeline internals, gates, src doc comments, test path pins, and
  20+ live docs) via ordered longest-first replacements with a
  post-pass stale scan — archive changelogs, docs/audits, and
  docs/research keep their historical paths untouched by design.
- scripts/README.md rewritten for the new tree (root entry points +
  nine category directories, every file listed and accounted for).
- Verification: build.sh help + version-sync green from the new
  location; check-ci-path-filters (9 workflows), check-scripts-loc,
  check-name-case, check-headers, check-rs-loc, comment-style, emoji,
  language, and disclaimer gates all green; docs_tests 21 passed; a
  follow-up gate repair carried the documentation disclaimer into the
  v100.0.6 era archive file (the one .md the disclaimer gate caught).
  Render-path sanity bench unchanged (fps in band, alloc_calls 563
  identical, entropy and gini identical — no runtime surface moved).
- Known-local-only: check-permissions reports the cloud checkout's
  umask artifact (664/775 vs 644/755) across the whole tree; git blob
  modes remain canonical 644/755 and CI runners verify them.

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

### docs: night-improve-3 — the README demo assets ride the zelynic round-border law: the video loop and the scene still wear the 24 px radius

- The two demo embeds at the top of the README — the animated
  `cosmostrix-video.webp` loop and the `cosmostrix-4-scene.png` still
  — render centered with
  `border-radius:24px; box-shadow:0 4px 20px rgba(0,0,0,0.15)`,
  byte-identical to the zelynic demo-asset styling the task pins as
  the reference: rounded corners, one soft ambient shadow, no
  hard-edged screenshots above the fold.
- The logo embed stays un-rounded (zelynic's does too — the radius is
  a demo-asset law, not a brand law), the YouTube link row keeps its
  markup, and nothing below the fold moves: pure README HTML
  attribute styling, zero content or asset changes.
- Docs-only change: no source, build, or test surface touched, so no
  A/B benchmark per house rule.

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

### docs: night-total-lts-2 — the all-infra depth audit closes at peak: every rubric dimension verified, zero changes needed

- The full infra surface read under the five-dimension rubric
  (stability and crash, code hygiene, optimization, security
  hardening, LTS stability): all 30 `scripts/**/*.sh` files, all 9
  CI workflows, `build.rs`, `.cargo/config.toml`, and the
  supply-chain decision docs. The honest verdict: at peak, zero code
  changes shipped — every candidate finding was already handled in
  code, deliberate by design, or an owner-documented decision.
- The hunt that came back clean: no `pull_request_target` anywhere
  (the fork-PR secret-exfiltration vector does not exist); secrets
  scoped to owner-pushed `v*` tags and `workflow_dispatch` only;
  `build.rs` executes two fixed-argument programs (`git rev-parse`,
  `rustc --version`) with no user-shaped command lines; strict
  `set -euo pipefail` at every shell entry point with sourced
  `lib/` modules inheriting by design; the two `depth/` stress
  harnesses deliberately run without `-e` (failing commands are
  their test cases — forcing `-e` would kill them mid-run); the
  zombie-script hunt found zero orphans (the seven unreferenced
  files are the dynamically sourced `lib/` modules); action tag
  floating re-verified as the recorded owner decision with active
  compensating controls, not an oversight.
- Full rationale, evidence line numbers, and the three trip wires
  that would reopen the verdict:
  [NIGHT_TOTAL_LTS_2_AUDIT_2026-10-11.md](docs/audits/NIGHT_TOTAL_LTS_2_AUDIT_2026-10-11.md).
- Docs-only change: no source, build, or test surface touched, so no
  A/B benchmark per house rule.

### fix+docs: night-total-lts-1 — the cosmostrix skill depth audit: the origin-side terminal reset/restore stack closes at peak, one stale-reference dragon removed

- The full skill layer stack read under the five-dimension rubric
  (stability and crash, code hygiene, optimization, security
  hardening, LTS stability): `terminal/restore.rs`,
  `terminal/terminal_tty.rs`, `terminal/cleanup.rs`,
  `terminal/io_recovery.rs`, `platform/fork_guard.rs`,
  `platform/panic_hook.rs`, `interactive/signal_handlers.rs`, the
  `--reset-terminal` dispatch in `cli/early_returns.rs`, and the guard
  install ordering in `interactive/event_loop.rs`. This is the origin
  side of the skill zelynic imported as the `--reset-terminal` transfer
  (NIGHT-hunt-31).
- The hunt that came back clean: the Linux fork guard forks before any
  thread exists (single-threaded fork by construction, verified against
  every spawn site); the fork-vs-prctl race, the ~200 ms reparent
  window, the Termux wedged-PTY deadlock, the double-panic coredump,
  the SIGTERM zombie window, and the stdout-death mid-rain path are
  each answered in code with tests or SAFETY comments; the three
  external rescue programs run fixed argument vectors with no
  user-shaped input (nothing to inject); the raw-fd writes are
  bounds-checked with `n <= 0` termination.
- The one dragon found and removed (the fix half of this entry): 14
  live code references to `terminal.rs` — a file that no longer exists
  since the monolith split — repointed to their real post-split homes
  (`terminal/mod.rs`, `terminal/draw.rs`, `terminal/io_recovery.rs`)
  across seven files, including the incubator's runtime verdict
  strings. The two remaining `terminal.rs` mentions are deliberate
  historical narrative and stay accurate.
- Zero code-behavior changes, so no A/B benchmark per house rule; the
  comment-only fix verified `cargo fmt --all --check` clean and
  `cargo clippy --all-targets` zero warnings.
- Full rationale, the per-dimension evidence, and the three trip wires
  that would reopen the verdict:
  [NIGHT_TOTAL_LTS_1_AUDIT_2026-10-11.md](docs/audits/NIGHT_TOTAL_LTS_1_AUDIT_2026-10-11.md).

### fix: night-improve-6 — the CLI output color audit closes with three consistency fixes, plus the Business/Company price raise

- The Business and Company tiers are raised to track the compounding
  engineering value (Business $5,000 to $5,199/year, Company $50,199 to
  $100,199/year; Personal and Individual untouched), recorded in the
  Pricing Philosophy section with the sharpened unauthorized-use
  warning, across COMMERCIAL_LICENSE.md, README.md, and
  docs/LICENSING_FAQ.md (the pricing commit of this round).
- The depth audit of every error/failed/unknown/typo output surface
  (referenced against zelynic's identical brand palette) verified the
  two canonical paths are already consistent — `cli/ux.rs` renders
  every fatal CLI path through the branded labeled-block renderer, and
  `clap_styles()` aligns clap's own error/valid/invalid hues with the
  crate palette (brand red 255,90,90 errors, white tips, warn yellow
  invalid values) — then found three surfaces that bypassed the
  contract:
  1. the bench baseline safepath rejections and baseline save/compare
     failures printed plain "error:" text (bench_cosmetics.rs) — all
     four sites now render through `eprintln_error_labeled` like every
     other error, wording preserved;
  2. `clap_styles()` was `#[cfg(unix)]`-gated, so Windows clap-side
     errors (unknown flag, missing value, invalid value) fell back to
     clap's default three-hue styling that the v100.0.0-nightly.1
     mandate had eliminated on Unix — the gate is lifted (clap builder
     styles are platform-neutral ANSI strings; anstream strips them
     where the stream cannot take color), and the change is verified
     by a local `cargo check --target x86_64-pc-windows-gnu
     --all-targets` green run;
  3. the Windows Ctrl-Break install failure printed plain text — now
     error-labeled like its Unix siblings.
- Bonus hunt from the Windows verification build: four
  `std::env::set_var`/`remove_var` call sites in the
  `#[cfg(windows)]` `windows_tests` module of `test/safepath/tests.rs`
  were missed by the night-improve-4 edition-2024 migration (invisible
  to Linux CI, which never compiles that module) — the Windows test
  target did not compile before this round. All four are wrapped in
  unsafe blocks with the house SAFETY comments, and the windows-gnu
  test target now compiles warning-free.
- No render-path or frame-pipeline surface touched, so no A/B
  benchmark per house rule (the benchmark rubric measures rain
  density/entropy/fps behavior, not stderr diagnostics).

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
