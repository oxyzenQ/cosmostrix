# Changelog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

cosmostrix uses [SemVer](https://semver.org/). Git tags use a leading `v` (e.g. `v50.0.0`).

The changelog is split into per-era files so this one stays navigable
(first pass plus completion, both in NIGHT-docs-1; the former
monolith carried every entry since v50.0.0-beta.6 inside a single
Unreleased section, and the stable bump of 2026-09-14 never cut it):

- Unreleased (post-v100.0.0-stable work, 2026-09-14 onward) — this file, below.
- [CHANGELOG-V100-ERA.md](CHANGELOG-V100-ERA.md) — the v100 line that built v100.0.0 stable: the nightly.1 hunts (2026-09-04 to 2026-09-10), the beta.1 long-horizon hardening (2026-09-10 to 2026-09-14), and the rc.1 candidate (2026-09-14).
- [CHANGELOG-V80-ERA.md](CHANGELOG-V80-ERA.md) — the v80 line: S-master hunts, the crystal dragon, and the Z-master harmony campaigns (2026-08-30 to 2026-09-03).
- [CHANGELOG-V50-ERA.md](CHANGELOG-V50-ERA.md) — the v50.0.0 pre-release line plus the condensed v13-v25 releases.
- [Pre-v13 archive](docs/archive/CHANGELOG_PRE_V13.md) — pre-v13 history (v2 to v12).

The condensed origin story stays at the bottom of this file. The
`## v4.0.0` and `## v3.9.0` headings are tripwire-locked by
`test/docs_tests/metadata.rs` and must remain in this file (see the
tripwire note in the pre-v13 archive).

---

## Unreleased

### audit: NIGHT-improve-10 / security-3 — overflow & explode-data endurance audit: all surfaces verified at peak, zero code changes

- **Change**: owner request to mitigate overflow/explode-data when
  limits are reached, for long-endurance LTS operation. A
  surface-by-surface audit (record:
  `docs/audits/OVERFLOW_ENDURANCE_AUDIT_2026-09-21.md`) covered
  every counter, accumulator, buffer, time computation, and
  floating-point drift
  state a running session advances. Verdict: everything is already
  at peak — per-frame integer accumulators are u64 with saturating
  adds (shortest wrap horizon ~42,700 years at the worst documented
  byte rate) or u32 generation counters with the wraparound-safe
  `GEN_RESET_THRESHOLD` proactive reset (~2.1 years at 60 FPS) and a
  lock-test-verified invariant; every session-grown container is
  bounded (fixed 60-slot frame-time ring, `ANOMALY_MAX_ZONES` cap +
  expiry, phosphor BitVec dedup + swap_remove, moments expiry +
  cooldown, 64-slot deduped warning/diag logs, droplet free-list
  recycle); the 24 h duration ceiling is enforced inside the f64
  parser (is_finite per component and on the total — a hostile
  `99999999h` saturates to inf and is rejected, integer overflow
  structurally unreachable); every `Duration::from_secs_f64(1.0/fps)`
  site is floored upstream (base fps `max(1.0)` at construction and
  in the setter, constant pause branch, explicit drain floor,
  non-configurable 0.5 idle const) so the inf-panic path is
  unreachable; the entropy drift phase is wrapped (`%= 1.0`) before
  every sin evaluation and renderer-memory pressures are derived
  from bounded history averages, not accumulated — no f32 drift
  state can explode over months of uptime. Per the owner's
  audit-if-peak-skip rule, zero over-engineering was applied: no
  u128 widening, no checked arithmetic on the hot generation path,
  no ring-buffer restructuring of the capped warning log. Docs-only
  change — no benchmark run (owner rule).
- **Verification**: source-verified per surface with worst-case
  long-session math (24 h capped paths; indefinite screensaver runs
  elsewhere); no code paths touched; gate-keepers full suite green.

### security: NIGHT-improve-8 follow-up — gesture-level selection-bypass hardening: the whole Down/Drag/Up/Moved motion family now gets zero acknowledgment plus per-event selection-clearing churn

- **Change**: owner follow-up report — rendered text was still copyable
  ("include even shift+click and any"). The re-audit found the first
  pass covered only the anchor click of a selection gesture:
  `is_modifier_click()` classified modified Down events, but modified
  Drag/Up/Moved events fell through to the plain path — the hover glow
  tracked the bypass drag (`set_mouse_position` ran before the modifier
  check in the mouse arm), and no selection-clearing churn happened
  while the selection extended. The predicate is now
  `is_selection_bypass_event()` (`src/interactive/input.rs`): every
  modifier combination on the full selection-motion family (Down,
  Drag, Up, Moved) is a bypass attempt. The mouse arm in
  `event_loop_mouse.rs` checks the predicate BEFORE the hover-position
  update, so a forwarded shift+drag gesture gets zero visual
  acknowledgment end to end (hover glow frozen, no click wave) plus a
  full-frame redraw per bypass event — erasing freshly painted native
  selection highlights in terminals that clear selection state on grid
  updates, and keeping the grid churning under the whole gesture so
  position-anchored selection copies (xterm-style: copy reads CURRENT
  cell content) capture moving rain glyphs, not the text the user
  highlighted. Modified scroll deliberately stays on the plain path
  (the wheel is not a selection primitive; no spurious full redraws).
  The honest trust boundary in `docs/SECURITY_AUDIT.md` is
  strengthened with a two-class terminal matrix: bypass terminals
  (mainstream default) intercept modified clicks locally and never
  forward them — no escape sequence can revoke a terminal's own
  selection engine, select-all, Ctrl+Shift+C, screenshots, or
  multiplexer copy-mode, so "still able to copy" there is terminal
  physics, not an app gap; forwarding terminals (minority; legacy
  Windows console input is the notable case) are covered by the
  gesture-level policy above. `docs/SCREENSAVER_MODE.md` interaction
  table resynced to the new predicate.
- **Verification**: `test/interactive/tests_night_improve8.rs`
  rewritten to pin the gesture-level contract — every modifier bit and
  combination (9 combos) on every motion kind (Down/Drag/Up/Moved),
  a full shift+drag gesture sequence (Down -> Drag x3 -> Up), plain
  unmodified events on all three buttons plus Moved stay on the normal
  path, and all four scroll kinds with modifiers stay excluded;
  `cargo fmt --check` + `cargo clippy --all-targets` clean; interactive
  suite 221/221; gate-keepers 13/13 (fresh-clone 664 permission
  artifacts auto-normalized to the canonical 644/755, no tracked
  mode-bit changes); runtime surface is event-path only (render
  pipeline untouched, steady-state frame content identical), benchmark
  A/B recorded post-commit.

### security: NIGHT-improve-8 — modified-click selection-bypass hardening: shift+click and any modifier combination produce zero feedback plus a selection-clearing redraw

- **Change**: owner request to disable copy/paste — rendered text/info
  must not be extractable, including via shift+click. The audit found the
  copy surface already closed everywhere except the terminal's local
  selection bypass: mouse capture is held for the entire session
  (startup enable, SIGCONT re-assert, exit/suspend-only release), no
  clipboard crate or OSC 52 write exists, and pasted content is
  structurally discarded (`Event::Paste(_)` never read). The one gap:
  modified clicks (shift+click and any other modifier combination) are
  reserved by terminals for their local selection engine, and the
  minority of terminals that forward them previously triggered the same
  click-wave arm as plain clicks. Now `is_modifier_click()`
  (`src/interactive/input.rs`) classifies every modifier combination on
  a mouse Down as a selection-bypass attempt: zero visual acknowledgment
  (no click wave, no idle click wake) plus an immediate full-frame
  redraw that erases the freshly painted native selection highlight in
  terminals that clear selection state when grid content underneath
  updates. Plain unmodified clicks keep the exact hover/click-wave
  behavior; modifier bits on drag/move/up/scroll kinds do not trigger
  spurious redraws. The trust boundary is documented honestly in
  `docs/SECURITY_AUDIT.md` (new anti-copy/interaction-surface
  paragraph): terminal-side features (select-all shortcuts, Ctrl+Shift+C,
  screen capture, post-exit scrollback) are outside any TUI
  application's control — no escape sequence can revoke them.
- **Verification**: 4 new unit tests
  (`test/interactive/tests_night_improve8.rs`, wired in
  `src/interactive/mod.rs`) pin the classification contract — every
  modifier bit and combination on Down is a modifier click, plain Down
  on all three buttons stays on the normal path, non-Down kinds with
  SHIFT never classify; `cargo fmt --check` + `cargo clippy` clean;
  gate-keepers full suite green; runtime surface is event-path only
  (render pipeline untouched), benchmark run post-commit as A/B record.

### ci: unify all scheduled workflow crons at 00:00 UTC (07:00 WIB) — owner call closing the decision parked by NIGHT-hunt-6

- **Change**: `maintenance.yml` fired Monday 07:00 UTC (14:00 WIB) while
  the daily `gitbot-audit.yml` already fired at 00:00 UTC (07:00 WIB) —
  the two security bots ran 7 hours apart, and the weekly `codeql.yml`
  (Mon 03:00 UTC) + `miri.yml` (Sun 03:00 UTC) drifted further behind.
  All four scheduled workflows now share one clock: `maintenance.yml`
  `0 7 * * 1` -> `0 0 * * 1` (Mon 14:00 -> 07:00 WIB), `codeql.yml`
  `0 3 * * 1` -> `0 0 * * 1` (Mon 10:00 -> 07:00 WIB), `miri.yml`
  `0 3 * * 0` -> `0 0 * * 0` (Sun 10:00 -> 07:00 WIB); `gitbot-audit.yml`
  unchanged (`0 0 * * *`) with its header + cron annotation carrying the
  WIB offset. Doc resync: the "Monday 00:00 UTC" statements in
  `docs/workflow/ABOUT_CI.md` and `docs/SUPPLY_CHAIN.md` — left stale by
  the `b0a70ebd` cron rotation, flagged and parked (not silently fixed)
  by NIGHT-hunt-6 — are now true again, with the WIB offset annotated in
  both plus 4 trigger-table rows in `docs/MAINTENANCE.md` and the
  `miri.yml` / `codeql.yml` workflow headers.
- **Verification**: all four `cron:` expressions re-parsed via
  `yaml.safe_load` (`0 0 * * 1`, `0 0 * * 1`, `0 0 * * 0`, `0 0 * * *`);
  corpus re-sweep for the old schedule strings (`0 7 * * 1`, `0 3 * * 1`,
  `0 3 * * 0`, `Mon 03:00`, `Mon 07:00`, `Sun 03:00`, `14:00 WIB`) hits
  only framed-historical CHANGELOG text; actionlint + yamllint clean on
  the four edited workflows; gate-keepers full suite green; zero Rust
  surface touched (workflows + docs only, no benchmark run required).

### cleanup: NIGHT-hunt-6 (post v100.0.2) — second-sweep staleness hunt: key-details truth, CI-trigger table, dep counts, moved-doc citations

- **Change**: a second, independent audit pass over axes the repo's own
  tooling does not cover (key IDs, workflow triggers, dependency counts,
  non-`*.md` commented code) — source files were parsed as the truth column
  and every divergent doc line was rewritten to match reality.
  `docs/VERIFY_RELEASE.md`: the key-details line listed `56B96F3109F4B924`
  in the active signing set with no role; keyserver ground truth (live
  fetch) plus `gpg --list-packets` over every published release `.asc`
  shows it is the pre-rotation artifact subkey — it last signed the
  v50.0.0-rc.1 artifacts and was superseded by `3C9EB25BF0407781` from
  v80.0.0-beta.1 onward — now labeled as such (still carried on the
  master key, expires 2028-08-14, signs nothing in the current pipeline).
  The tag subkey role now also names maintainer git commits (verified
  Good on a84016ac, c110affc, 6c511477, 27f7d4f5).
  `docs/MAINTENANCE.md`: dependency line corrected from "64 direct deps /
  98 total crates" to the real 11 unique runtime direct deps (8
  cross-platform + `signal-hook`/`libc` unix + `ctrlc` windows; `proptest`
  dev-only) / 105 crates in the lock; the CI/CD table rows now state the
  real triggers (Miri/CodeQL/Security Audit also run on push/PR, not
  cron-only; `ci.yml` paths include `test/**` per its own standing rule;
  `aur.yml` is `repository_dispatch`-triggered by `release.yml`, not
  "release tag"; Release builds 11 archives across 7 OS/arch targets, not
  "10 platform binaries"; Maintenance cron is Mon 07:00 UTC + manual
  dispatch, not "Mon 07:00 WIB").
  `maintenance.yml`: both schedule comments still said "Monday 00:00 UTC"
  — stale since the deliberate cron change `0 0 * * 1` -> `0 7 * * 1` in
  b0a70ebd ("maintenance cron 7AM Monday"); now "Monday 07:00 UTC
  (14:00 WIB)" matching the expression. Behavior untouched: the cron
  itself was NOT changed — if the original intent was 07:00 WIB
  (= 00:00 UTC), the expression needs a separate one-character change
  and that decision stays with the owner.
  Moved-doc citations repaired: `.cargo/config.toml` still pointed at
  `docs/audits/LTS_BUILD_AUDIT_v50.0.0-beta.7.md` and `release.yml` at
  `docs/research/PLATFORM_EXPANSION_IOS_WIN_ARM64.md` — both live under
  `docs/archive/` since the docs restructure; paths updated.
  `scripts/visual-mode-audit.py`: the constants-mirror comment named
  `src/central_control_rains.rs` (a file that no longer exists — it is a
  module directory); now names the real homes (`atmosphere.rs` for
  `CRT_VIGNETTE_*`, `mod.rs` for `EDGE_FADE_*`).
- **Verification**: keyserver re-fetch of the master key + `gpg
  --list-packets` on the v50.0.0-rc.1 / v80.0.0-beta.1 / v100.0.0-
  nightly.1 / rc.1 / v100.0.0 / v100.0.1 artifact signatures (rotation
  boundary pinned between Aug 24-31); `git verify-commit` on four
  maintainer commits (all Good from the tag subkey); trigger blocks
  re-parsed from all 9 workflow files; dependency counts recounted from
  `Cargo.toml` sections + `Cargo.lock` packages; dead-script sweep found
  zero unreferenced scripts (33/33 referenced; `nh2_shift_harness.py` is
  a live shared library — `night_cbg34_e2e.py` imports `Screen` from
  it); repo-path existence check over the 52 non-`*.md` script/workflow/
  toml files found 4 unframed stale paths (the 2 fixed citations, the
  mirror comment, and one properly-framed `was previously` note left
  as-is); gate-keepers all green; docs-audit + stale-hunt still clean.

### cleanup: NIGHT-hunt-5 (post v100.0.2) — total staleness cross-audit of docs and commented code

- **Change**: live-corpus sweep (55 .md files at audit time; `docs/archive/`,
  `docs/research/`, `docs/audits/`, the CHANGELOG era files, bench-lab A/B
  artifacts, and `CHANGELOG.md` below `## Unreleased` are historical
  snapshots outside the sweep) eliminating stale data, dead how-to burden,
  and stale commented-code references. Fixed: `docs/ENDURANCE.md` dropped
  ~127 lines of present-tense how-to guidance for the removed
  `monitor-cosmostrix.sh`/`endurance-summary.sh` helpers (the historical
  record — CSV format spec, acceptance criteria, past results, run
  template — is kept, reframed past tense); the chroma lock-suite header
  still said "Phase 9-C" while `CHROMA_DRAGON_ENGINE_VERSION` is
  "9-D (locked)" — aligned; the chroma README 9-C phase row now names both
  removed variants (Cartesian + sRGB-linear) per commit `2e20f6cc`;
  `docs/THREE_DRAGON_ENGINES.md` lock-suite sentence pointed at a wrong
  glob — it now names the two real lock suites (19 + 17 invariants) and
  the Crystal per-subsystem suites; `docs/VERIFY_RELEASE.md` key-details
  line refreshed from the v100.0.1 to the v100.0.2 release signatures;
  the `aur.yml` `repository_dispatch` trigger comment described a
  superseded "backward compat, keep until" state — rewritten to describe
  the live mechanism (release.yml posts the event after a release
  publishes).
- **Tooling** (`scripts/docs-audit.py`): truth notes refreshed (2952
  `#[test]` fns = 2418 in `test/` + 534 in `src/`; was 2947), the corpus
  rules encoded (historical snapshots excluded per
  `docs/FUTURE_BACKLOG.md` section 1), and sections 1-3 gained
  negation/history context awareness (line window + enclosing-section
  markers, the same philosophy `scripts/stale-hunt.py` applies to Rust
  comments) so intentional-history records are no longer tool noise.
  `docs/FUTURE_BACKLOG.md` rewritten as the standing ledger: corpus
  rules, the intentional-history register, and current counts.
- **Verification**: `python3 scripts/docs-audit.py` — all four sections
  clean over the 55-file live corpus, and a scratch-file self-test
  proved both directions (an unframed stale ref is flagged;
  removal/move/example-framed refs are exempt); `python3
  scripts/stale-hunt.py` — 0 stale flags/paths/modules (the 223
  duplicate-comment groups are the heuristic mirror-test narration
  pattern, unchanged); `bash scripts/inject-disclaimer.sh --check` —
  212/212 files carry the marker; ruff + rustfmt clean; gate-keepers
  all green.

### fix: the release-note commit classifier never fired — case-sensitive prefix vs the capitalized history

- **Change** (scripts/generate-release-notes.sh): every classifier stage
  (`grep -qE '^internal research:...'` detection, verb extraction sed,
  `scan_text` prefix stripper, and the display-text prefix stripper) matched
  the prefix lowercase-only, while the entire commit history writes
  `Internal research:` (capital I; 7/7 such subjects in the log). Result: the
  verb table never fired and every prefixed commit fell into "others" — the
  published v100.0.1 note shows `others x 13` of 18 commits, and entry
  display kept the full `Internal research: ...` prefix. All six pattern
  sites now match case-tolerantly (`[Ii]nternal [Rr]esearch:`), portable
  POSIX classes only (no GNU-only operators, BSD-sed safe).
- **Proof**: regeneration over `v100.0.0..v100.0.1` reclassifies the visible
  commits into `ci x 1`, `chore x 1`, `docs x 1` (previously all "others");
  the remaining `others x 3` are by-design falls (verbs "retire"/"accelerate"
  are outside the verb table; the bare `release:` conventional type is
  unknown to the map). The `v100.0.1..main` preview with TAG=v100.0.2 now
  renders `fix x 1, chore x 1` with the process prefix stripped from the
  entry display, exactly as the design notes intend.
- **Verification**: bash -n + shellcheck + shfmt clean; release bodies
  generated locally and diffed against the published classification;
  gate-keepers all green.

### fix: the release-note GPG example shipped wrong asset names — missing `v` prefix + spurious `-gnu` (caught in the published v100.0.1 body)

- **Change** (scripts/generate-release-notes.sh): the verification example in the
  generated release body read `gpg --verify cosmostrix-${TAG#v}-linux-amd64-v3-gnu.tar.gz.asc`
  — two wrongs on one line, first shipped in the v100.0.1 release (this script's
  debut release; every note before v100.0.1 was generated by the old inline
  template). `${TAG#v}` stripped the `v` prefix, but release archives are named
  `cosmostrix-vX.Y.Z-<platform>.tar.gz` — the published note told users to verify
  a file that cannot exist. The `-gnu` suffix is the internal build ID (what
  `--version` prints via `infer_build_id()`), never part of a release archive
  name. Fixed to `cosmostrix-${TAG}-linux-amd64-v3.tar.gz.asc`, so the v100.0.2
  note renders `cosmostrix-v100.0.2-linux-amd64-v3.tar.gz.asc` — a byte-for-byte
  real asset name.
- **Stale-docs companion** (docs/VERIFY_RELEASE.md): the Key details line still
  named `56B96F3109F4B924` with a 2029-08-09 expiry — both stale. The v100.0.1
  artifacts are signed by the rotated subkey `3C9EB25BF0407781` (expires
  2028-08-23) and the tag by `2B52187D2AB618A7591D45C9C7F8B07418899C4E` (expires
  2031-08-23), all verified against the keyserver and the published `.asc`
  files. The line now lists the active set with an as-of date and defers to the
  keyserver after future rotations; the expiry-policy paragraph now describes
  the real bounded cycles (~2-year artifact subkeys, 5-year tag subkey) instead
  of the stale singular 1-year claim.
- **Hunt scope**: repo-wide sweep for the same pattern family — `${TAG#v}`,
  `cosmostrix-` asset names without the `v`, and `-gnu` in asset-name
  positions. Every other live reference is correct (README.md install examples,
  docs/VERIFY_RELEASE.md command blocks, AUR PKGBUILD asset selection); the
  remaining `-gnu` hits are the legitimate build ID in `--version` docs and
  era-archive historical snapshots, both intentionally untouched.
- **Verification**: generated the release body locally over the v100.0.0..v100.0.1
  range with TAG=v100.0.2 and confirmed the rendered example matches the real
  asset names character-for-character (all 11 v100.0.1 archives cross-checked
  from the published release); shellcheck + shfmt clean; gate-keepers all green.

### perf: NIGHT-perf-2 (post v100) — CI runs the suite through cargo-nextest in two parallel partitions

- **Change** (ci.yml): the full-suite `cargo test --all --locked` step
  moved out of the build_test job into a new `test_partitions` job —
  a 2-way, count-balanced cargo-nextest matrix (`--partition
  count:<i>/2`, `fail-fast: false`). Each partition restores the
  shared rust-cache, compiles the same test build, and executes its
  half; the run stage halves and now OVERLAPS the debug build instead
  of sequencing after it. The build_test job id is unchanged, so the
  six downstream `needs: build_test` references are untouched (its
  display name is now "Build (debug)" — that is all it does).
- **Why nextest**: per-test process isolation, better scheduling, and
  first-class partition support. `--retries 1` gives one retry for
  tests that flake on a loaded shared runner (the suite carries 486
  sleep/duration-based timing tests); nextest reports any retried test
  as "flaky", so the signal is never silently lost.
- **Dependency policy compliant**: cargo-nextest installs via
  taiki-e/install-action@v2 unpinned — latest upstream release resolved
  at run time, same as cargo-audit in the security job (owner policy
  2026-08-30, docs/workflow/ABOUT_CI.md).
- **Doc-test contract documented**: nextest does not execute doc
  tests; the crate has zero compiled doc tests today (all fenced doc
  blocks are `text`/`ignore`). The ci.yml job comment states that a
  compiled doc test must come with a `cargo test --doc` step.
- **Local story documented** (CONTRIBUTING.md): scripts/build.sh
  already auto-detects `cargo-nextest` on PATH (`NEXTEST_AVAILABLE`)
  and prefers it over plain `cargo test` — a one-time
  `cargo install cargo-nextest --locked` upgrades every local
  build.sh/check-all test run with zero workflow change.
- **Verification**: yamllint + actionlint clean on the restructured
  ci.yml; gate-keepers 19 passed / 0 failed; no Rust source, script,
  or manifest change — CI + docs only, no benchmark per house rule.

### perf: NIGHT-perf-1 (post v100) — the test suite accelerated: opt-level 1 test profile + the MSRV full-suite duplicate retired

- **Problem**: the full suite (about 2950 tests; 2418 in the `test/**`
  mirror wired via 48 `#[path]` includes, 534 inline) executed at
  `profile.dev`'s opt-level = 0 because `profile.test` inherits dev.
  The CPU-heavy simulation tests (engine invariant sweeps across all
  44 themes, black-hole spin math, benchmark statistics) ran 10-30x
  slower than optimized code, and 486 sleep/duration-based timing
  tests padded the wall clock on top. CI made it worse: the msrv and
  build_test jobs pin the SAME RUST_VERSION toolchain, so the complete
  suite ran twice back-to-back on every push (msrv full run, then
  build_test full run on a warm cache).
- **Fix 1 — `[profile.test] opt-level = 1`** (Cargo.toml): the whole
  test build now optimizes at level 1. Compile cost rises ~20-30%;
  runtime drops hard on the compute-bound tests. Debug info, unpacked
  split-debuginfo, incremental compilation, and codegen-units stay
  inherited from dev, so backtraces and rebuild speed are unchanged.
  `profile.dev` (cargo run / cargo build) and every release/pro
  profile are untouched — zero production-surface change.
- **Fix 2 — MSRV smoke** (ci.yml): the msrv job now runs
  `cargo test --all --locked lock_` — the full test tree still
  COMPILES on the MSRV toolchain (the MSRV contract), plus the ~175
  engine invariant lock-suite tests (the `lock_` prefix families
  across all three dragon engines) as the runtime smoke. The complete
  suite runs exactly once per push, on the stable build_test job.
  FreeBSD keeps its full run (cross-platform validation; a different
  OS surface is a different contract).
- **Test count unchanged**: nothing was deleted, skipped, or gated
  behind features — this is pure execution speed. Zero compiled doc
  tests exist (all 40 fenced doc blocks are `text`/`ignore`), so the
  suite is exactly the bin-target unit tests.
- **Verification**: manifest parsed and build-graph constructed under
  the new profile (cargo test --no-run progress observed); yamllint +
  actionlint clean on ci.yml; gate-keepers 19 passed / 0 failed
  (incl. the version-sync guard that scans ci.yml for RUST_VERSION
  drift — untouched).

### cleanup: NIGHT-cleanup-2 (post v100) — the dragon engine lock protocol retired: RULES.md/KEY.md/dragon-history.sh removed, engine READMEs simplified

- **Owner decision (2026-09-20)**: the per-engine LOCK/UNLOCK protocol
  was too strict a maintenance burden. Every engine change had to
  carry an UNLOCK entry in the touched engine's `KEY.md` + `RULES.md`,
  and the `dragon-history.sh --since-lock` audit trail had to be
  re-anchored on every lock round. The engines keep their LTS quality
  the simple way: the CI invariant suites (the `lock.rs` test families
  under `test/engine/*/`) already assert each engine's public contract
  on every commit — the protocol layered ceremony on top of what CI
  enforces mechanically.
- **Removed (7 files, ~2.4k lines)**: `src/engine/{cosmic,chroma,crystal}_dragon_engine/RULES.md`
  (the full UNLOCK protocol + logs), the matching per-engine
  `KEY.md` signature logs, and `scripts/dragon-history.sh` (the
  history wrapper whose `LOCK_AT` boundary needed manual updates).
  Historical content is preserved in the era changelogs and archived
  audits, which follow the historical-snapshot contract and stay
  untouched.
- **Engine READMEs simplified**: each dragon engine README dropped the
  lock ceremony (KEY.md/RULES.md callouts, Modification Protocol,
  UNLOCK History, Documentation Lock, lock signature blocks) and keeps
  the engineering reference: audit findings, A/B benchmark tables,
  topology, phase history, and owner decisions. The "What This Lock
  Means" framing became "Stability Status" pointing at the CI
  invariant suites as the enforcement mechanism.
- **Live references re-anchored**: `docs/THREE_DRAGON_ENGINES.md` lock
  status section rewritten as "Engine history" (the plain
  `git log --oneline -- src/engine/...` command stays as the history
  method); `docs/FUTURE_BACKLOG.md` ACCEPTED-tool-noise note updated
  for the removed KEY.md/RULES.md snapshots; `src/diagnostics/info.rs`
  chroma phase-history pointer re-anchored to the engine README (2
  comment sites); the chroma `colors_custom/strictness.rs` module doc
  dropped its stale KEY.md UNLOCK reference. Historical mentions in
  CHANGELOG era files, docs/archive, and docs/audits stay verbatim
  (timestamped records).
- **Not touched**: `docs/RULES.md` (project conventions), `src/RULES.md`
  (module policy), and `src/RULES_LOC.md` (LOC cap) are separate live
  files that share the name but not the protocol; the `lock.rs`
  invariant suites themselves remain fully in force.

### cleanup: NIGHT-cleanup-1 (post v100) — dead-script sweep: 27 retired one-off hunt harnesses removed, live references re-anchored

- **Method**: a full reference map of every tracked file (CI workflows,
  build.sh + gate-keepers.sh wiring, install/release scripts, active
  docs, Rust tests, and the script-to-script Python import graph)
  classified all 61 scripts by reference class. One-off hunt/repro
  harnesses with zero live wiring — their contracts already locked
  in-tree as Rust regression tests and their evidence already recorded
  in docs/research, docs/audits, CHANGELOG, and benchmark/bench-labs —
  were removed; every removal was individually verified against the
  map before deletion.
- **Removed (27 scripts, ~330 KB; scripts/ now 34 files)**: the
  depthtest e2e family (depthtest-2 through -8 plus the shared
  depth-test-config.py generator), the PTY helper cluster that nothing
  live imports (ansi_screen.py, nh2_pty_harness.py, nh2_raw_capture.py,
  nh15_restart_e2e.py), the NIGHT-hunt repro harnesses
  (night_h34_style_sweep, night_h38_force_repaint_classifier,
  night_h38_supermassive_testconf_repro, night_h41_msg_modey_repro,
  night_h40_entry_budget_e2e, nh32_crown_blink_audit,
  custom_features_stresstest.sh), the scene smoke probes
  (genesis/neural/quasar_smoke), the HUD e2e island
  (hud_long_scene_e2e, hud_order_e2e, intro_lead_e2e), and the
  remaining one-offs (endurance_probe, stress_test_bounds).
- **Hunt-beyond-the-signal catches**: (a) nh2_shift_harness.py was
  initially dead-classified but the import graph showed the live
  night_cbg34_e2e.py imports its Screen class — KEPT and re-documented
  as the shared PTY Screen library; (b) ansi_screen.py looked wired
  (two in-scripts references) but both referrers were themselves dead —
  transitively dead, removed; (c) the depthtest family's "formal E2E
  pin" claims inside Rust comments were stale the moment the harnesses
  went away — the in-tree Rust locks are now named as the pins.
- **Live references updated (stale-path domain)**: KNOWN_ISSUES.md
  workaround section, docs/LIVE_RELOAD_BEHAVIOR.md sections 19-20
  (four backticked harness citations reworded with removal notes),
  tests_monolith/residue.rs and config_apply_tests/strict_mode.rs
  comments (the stale-hunt.py file-path contract), and the
  nh2_shift_harness.py module docstring. Historical records were left
  untouched per the FUTURE_BACKLOG contract (era changelogs,
  docs/research and docs/audits snapshots, KEY.md dated signoffs,
  bench-labs AB evidence).
- **Scope**: no production Rust code touched; the binary is bit-for-bit
  unaffected, so no benchmark A/B was run per the house rule for
  docs/scripts-only changes. Verification: docs-audit.py (no new
  live-doc broken refs), stale-hunt.py (0 stale paths), ruff +
  shellcheck on the surviving scripts, permissions/headers checks, and
  the build.sh check-all gate.

### audit: NIGHT-optimized-1 (post v100) — the master optimization audit: peak verified across ten dimensions, no change warranted

- **Method**: dimension-by-dimension sweeps with on-host evidence,
  each probed for a >5 % measurable gain (the cosmic-dragon UNLOCK
  bar) or a zero-cost redundancy removal. None qualified; per the
  owner's standing rule (already-peak = skip, no over-engineering)
  the correct deliverable is the verification record.
- **Verdicts (evidence in
  `benchmark/bench-labs/night_optimized1/AUDIT_REPORT.md`)**:
  hot-path allocation PEAK (0 allocs/frame steady-state); per-frame
  clock discipline PEAK (per-frame/event only — the hidden per-cell
  now() was hunted out long ago); frame pacing PEAK (hybrid
  spin-sleep + dead-PTY/clock-jump/resize guards); release profile
  PEAK (o3/lto-fat/cgu-1/strip; `panic=unwind` deliberate for the
  catch_unwind containment contract); PGO PRESENT (+4.5 % median
  fps on record); dead code MINIMAL + DELIBERATE (13 documented
  allows; the unused_imports sites are LOC-split re-export
  patterns); redundant functions INTENTIONAL PARALLELISM (the
  13-scene / 12-style architecture, adjudicated); dependency surface
  PEAK (11 direct deps, all production-used per-crate, minimal
  features); cold start PEAK (`-V` in ~1 ms); long-run stability
  PEAK (drift is warm-up, endurance machinery in place).
- **Fresh HEAD measurements** captured on this host (cinematic
  ≈ 28.9–29.2 K fps, monolith ≈ 85.3–85.9 K fps; gini/entropy/dirty
  cells in the report table) sit on the historical regression line —
  no drift since the last LTS round. The same-session
  cybersecurity-1 commit is A/B-verified noise-neutral, so the
  verdict carries forward.
- **Scope**: documentation only (this entry + the bench-labs audit
  record). No production code touched, no benchmark run per house
  rule (docs-only); the cited measurements were already captured as
  the cybersecurity A/B baseline.

### security: NIGHT-cybersecurity-1 (post v100) — the master security audit: report-family escape injection closed, unsafe inventory re-verified

- **Method**: full capability-class re-sweeps (secrets, spawn sites,
  env surface, filesystem, panic vectors, dependency sources,
  pipe-to-shell) plus a line-by-line soundness re-review of every
  production `unsafe` site, and a sink-coverage analysis of every raw
  `print!`/`println!`/`println_safe!` site in the tree.
- **Finding (proven with a hostile-config PoC)**: the
  `--list-*`/`--show-scene` report family interpolated user-derived
  config strings RAW. `--show-scene` echoed a live `ESC [2J` byte
  (od-verified `033`) from an unvalidated `scene-custom.<name>.rain`
  VALUE — config VALUES are not charset-gated at collection (source
  validation happens later, at cloud-config build time), and the
  custom charset/palette name loops plus the hidden-block warning
  lines held the same class (their collectors gate name length and
  key shape, not name charset). Same shared-config threat model the
  S-night-R4 diagnostic guard closed; this family was missed then.
- **Fix (sink guards, S-night-R4 architecture)**: `escape_ctrl`
  routing at the report sinks — `show_custom_scene_text` /
  `list_custom_scenes_text` (whole-string sink, robust to future
  field additions) in `src/scene_custom/display.rs`; the custom
  charset + palette name loops and `hidden_block_warning_lines`
  (truncate-first-then-escape, so a `\u00XX` literal can never split
  across the 24-char cut) in `src/config/list_printers.rs`. Post-fix
  the PoC renders as the visible `glyph\u001b[2Jx` literal. Scene
  NAMES were already source-gated (`is_valid_profile_name`) and the
  parser rejects control bytes in KEYS (name-vector probes did not
  pass end-to-end) — the name-side guards are defense-in-depth,
  pinned by unit tests at the sink.
- **unsafe re-inventory (doc truth)**: the 2026-08-05 SECURITY_AUDIT
  "15 sites + 1 unsafe fn" snapshot had drifted with the v100-era
  refactors (terminal split, fork_guard extraction, posix_time
  consolidation, config_io fstat, watchdog isatty, the Termux
  non-blocking write family). Current classified truth: **47
  occurrences across 14 files — 35 production + 12 test-gated**,
  every production site re-reviewed sound with SAFETY documentation;
  the one gap (`utc_tm()`'s time/gmtime_r/assume_init calls missing
  the SAFETY twins their `local_tm()` counterparts carry) restored.
  SECURITY_AUDIT.md §1/§5 refreshed with dated NIGHT-cybersecurity-1
  notes; the escape_ctrl module doc now covers the report family.
- **Other sweeps (all clean)**: no secrets in tree; all deps
  registry-sourced and production-used; no shell spawns; no
  curl|sh; production parse paths assert/panic/unwrap-free; the
  message, charset and diagnostic escape hardening verified layered.
- **Tests**: 4 new regression tests (2 display-sink hostile-input,
  2 hidden-block-warning hostile/plain) — all green with the suite's
  targeted modules.
- **A/B**: 10 s release benches (cinematic + monolith, 2 runs each)
  — fps/entropy/gini/dirty-cells all within ±1.1 % with the scenes
  disagreeing on the sign (noise signature); the bench frame path
  executes none of the changed code. Evidence:
  `benchmark/bench-labs/night_cybersecurity1/AB_REPORT.md`.

### fix: the build.rs LOC-cap regression left by the hunt-2/hunt-3 test growth

- **Root cause**: the NIGHT-hunt-2 vcs-info parser tests and the
  NIGHT-hunt-3 epoch-constant/sub-minute suite grew `build.rs` to
  939 lines, past the 800-line hard cap enforced by
  `scripts/check-rs-loc.sh` — leaving `build.sh check-all` and
  gate-keepers RED at the LOC stage ever since (the prior session
  verified its edits via targeted runs, not the full gate).
- **Fix**: the house-sanctioned self-declaring marker
  (`// LOC_EXEMPT:` on line 3, per `src/RULES_LOC.md`): the build
  script is a single-file cargo contract, its test suite must stay
  in-file for the documented standalone runner
  (`rustc --edition 2021 --test build.rs` — cargo never executes
  build-script tests), and the helpers under test have no home
  outside the build script. build.rs joins the eight existing
  self-declared exemptions; no code moved.
- **Verification**: `check-rs-loc.sh` OK (8 files self-declare
  exemption, 0 unexempt violations); `build.sh check-all -q` exit 0
  inside the 2-minute local cap; gate-keepers 16/16 (permissions
  restored to the 644/755 convention after a clone-umask artifact,
  no content change — git tracks only the exec bit).
- **Scope**: 1 line in `build.rs`. No behavioral change, no
  benchmark per house rule.

### audit: NIGHT-hunt-4 (post v100) — the stale/burden/duplicate cross-audit driven to zero true positives

- **Method**: both house audit tools executed against the full tree,
  then every finding adjudicated against the FUTURE_BACKLOG
  migration table and the historical-record policy (source code =
  truth; live claims fixed; as-of-writing records preserved).
  Beyond the tools: present-tense count sweeps (themes, rain styles,
  test totals, source-file totals), duplicate-heading analysis
  across live docs, and duplicate-comment analysis in production
  code.
- **Fix 1 — the audit tool's own false positives**:
  `scripts/stale-hunt.py` reported 5 stale CLI flags, all
  `--test-threads` — the cargo/libtest harness flag referenced by
  the test-parallelism audit comments (2026-09-14, the 32-thread
  stress methodology). It belongs to the runner, not the cosmostrix
  clap surface; added to `EXTERNAL_TOOL_FLAGS` (beside its sibling
  `--nocapture`). Stale references: 5 -> 0.
- **Fix 2 — a tracked fix that was never applied**: the
  FUTURE_BACKLOG benchmark table row for
  `benchmark/bench-labs/PGO_AB_20260823.md` says "moved to
  docs/archive/research/IPC_RESEARCH.md", and the header note
  claimed the BENCH_LABS sweep was done — but this file still
  pointed at the pre-archive `docs/research/` path. Re-pointed;
  the row is now truthful.
- **Fix 3 — a misleading historical path**:
  `benchmark/bench-labs/night_research7_dna/AB_REPORT.md` credited
  its comparison to `scripts/ab_compare_nr7_dna.py` — a
  session-local comparator never committed to the repo (the other
  A/B reports name no such path). Reworded as a session-local,
  never-committed script so no reader hunts for a file that does
  not exist.
- **Adjudicated (kept, now recorded so future auditors do not
  re-litigate)**: the per-entry "Files changed" path records inside
  `src/engine/cosmic_dragon_engine/RULES.md` — as-of-commit-time
  paths (the test-mirror migration later moved `src/**/tests/` to
  the `test/` tree); added to the FUTURE_BACKLOG intentional-history
  note. Everything else the tools flag today is already covered by
  the existing adjudication classes: changelog/era historical
  entries, dated audit/research snapshots, the FUTURE_BACKLOG
  migration table itself, ENDURANCE/RELEASE_GUARD/RULES removal
  notes, incubator "at that time" narration, and illustrative
  example paths.
- **Hunted beyond the owner's brief, nothing stale found**: live-doc
  present-tense claims verified accurate (44 themes / 13 rain
  styles / 3 engines; the only "~1500+ tests"/"43 themes"/"2800
  green" hits sit inside dated historical records where they were
  accurate at time of writing); duplicate H2 headings across live
  docs are generic structure ("See Also", "Defense-in-Depth"), not
  content duplication; the production-code duplicate comments
  ("Palette slot adopted at spawn" x12, the 800-LOC split notes,
  per-scene field docs) are the intentional parallel-scene
  architecture — de-duplicating them would need shared types
  (an architecture change) or doc removal, both worse than the
  duplication.
- **Verification**: `scripts/stale-hunt.py` 0 stale flags/paths/
  modules (505 .rs files scanned); `scripts/docs-audit.py` section-1
  findings reduced to adjudicated classes only; markdownlint,
  codespell, and shellcheck clean on every touched file.
- **Scope**: 1 script (stale-hunt.py), 3 docs (PGO_AB report,
  nr7-dna report, FUTURE_BACKLOG adjudication note). No production
  Rust code, no benchmark per house rule.

### docs: NIGHT-docs-1 (post v100) — the monolithic CHANGELOG split into per-era files

- **Root cause**: `CHANGELOG.md` had grown to 8,235 lines / 587 KB in a
  single file. The structure was historical debt: no release-notes cut
  has happened since v50.0.0-beta.6, so every entry since — the entire
  v80 line, the Z-master campaigns, all v100.0.0-nightly.1 hunts, and
  every post-v100 NIGHT task — accumulated inside ONE `## Unreleased`
  section. Readers waded through ~4,000 lines of completed eras before
  reaching current work, and every future entry made the file worse.
- **Fix**: split by release era into root-level files, entries moved
  verbatim (immutable historical record, original order preserved):
  `CHANGELOG-V100-ERA.md` (the v100.0.0-nightly.1 hunts, 1,725 lines),
  `CHANGELOG-V80-ERA.md` (the v80 line: S-master hunts, crystal dragon,
  Z-master harmony, 2,291 lines), and `CHANGELOG-V50-ERA.md` (the v50
  pre-release line plus the condensed v13-v25 releases, 237 lines).
  The live `CHANGELOG.md` keeps the Unreleased (post-v100) section,
  the condensed origin story, and a new era index — 8,235 lines down
  to 4,083. The `## v4.0.0` and `## v3.9.0` headings stay in the live
  file: they are tripwire-locked by `test/docs_tests/metadata.rs`
  (`changelog_has_v400_entry_above_v390`), and the pre-v13 archive's
  tripwire note documents this contract.
- **Verification**: split executed by a boundary-asserted script (each
  cut line re-asserted before any write; probes confirm every era
  heading survives exactly once across the four files; v4.0.0 above
  v3.9.0 invariant preserved). `test/docs_tests/metadata.rs` untouched
  and still green; markdownlint, the SPDX header check, the disclaimer
  check, and codespell pass on all four files; permissions 644.
- **Scope**: `CHANGELOG.md` + three new era files + one cross-reference
  refresh (`docs/HUD.md` HUD expansion history now points at the v50
  and v80 era files). No code touched, no benchmark per house rule.

### docs: NIGHT-docs-2 (post v100) — the donation addresses cryptographically verified, and the Solana line now names USDT (SPL)

- **Verification**: all three README receive addresses were verified
  offline before any edit: the Ethereum address passes the EIP-55
  mixed-case checksum (keccak-256 of the lowercase hex — a single
  mistyped character fails it), the Bitcoin address decodes as a
  valid bech32m P2TR (witness version 1, 32-byte x-only program,
  `bc` mainnet human-readable part — Taproot, not native SegWit),
  and the Solana address base58-decodes to exactly 32 bytes (an
  ed25519 public key encoding).
- **Fix**: the Solana line said `SOL` only, while the owner's
  donation intent includes USDT over the Solana network — the same
  ed25519 address receives USDT (SPL) natively. The line now reads
  `SOL` / `USDT` (SPL), matching the Ethereum line's explicit
  ERC-20 naming, and the section intro records the verification
  method so future edits re-verify instead of trusting copy-paste.
- **Scope**: README donation section text only. No code, no
  addresses changed, no benchmark (docs-only change per house rule).

### fix: NIGHT-hunt-1 (post v100) — the black hole center ball froze after ~1.6 days (unbounded f32 spin phase) + full 13-scene long-session audit

- **Root cause**: the sorgonemous_intrascals ball rim's `spin_phase`
  is the only ball clock that never resets, and it accumulated as an
  UNBOUNDED f32 (`spin_phase += omega * dt`, read through `cos()` and
  the rim conveyor's bucket floor). f32 carries a 24-bit mantissa: at
  60 fps the per-frame increment (~0.0086 rad at the default speed 12)
  falls below the ulp of the accumulated phase once the phase passes
  1/(60 × 1.19e-7) ≈ 140,000 s of accumulated spin — about 1.6 days,
  INDEPENDENT of speed (both sides of the inequality scale with
  omega). The `+=` then rounds back to the same value every frame:
  the rim conveyor and the Doppler lobe stop moving and the ball pins
  at center — exactly the owner's report (fresh start plays the
  orbital rotation, >1 day sticks).
- **Fix**: the family's amortized wrap, applied to the last unwrapped
  member. `spin_phase` wraps into [0, 2π) once past 128 turns
  (`BLACK_HOLE_SPIN_PHASE_WRAP_LIMIT`, matching the vortex arm_phase
  and dna_helix phase precedent; quasar wraps at 64). The wrap is
  exact for both consumers: the Doppler lobe reads the phase through
  `cos()` (2π-periodic), and the rim conveyor now reduces the angle
  difference with `rem_euclid(TAU)` before the sector floor, so whole
  turns from the wrap cancel identically and the glyph pattern never
  scrambles at a wrap boundary. At the 128-turn limit the ulp is
  ~6e-5 rad — three orders below the smallest per-frame increment
  the slowest supported speed produces.
- **Scene-family audit (all 13 rain types + both sibling engines)**:
  quasar pulse/prec/disk/halo phases wrap at 64 turns (infall and
  jets have bounded lifecycles); vortex arm_phase wraps at 128 turns
  (NIGHT-hunter-10); dna_helix phase wraps at 128 turns; murmuration
  breath_phase wraps (boid ages are threshold-only or write-only);
  neural streamer/pulse ages are lifecycle-bounded (Neuron.age is
  write-only); flux's fixed-step accumulator drains with a backlog
  drop; monolith streams and solar_flare loops run bounded lifecycle
  clocks (reset at each phase transition); physarum agents carry
  per-particle lifetime caps; aeolian/dragon/glyph/lorenz have no
  unbounded phase accumulators (event-driven or physically bounded);
  the chroma gradient normalizes angles by += 2π (not an
  accumulator); crystal's sensor timestamps are Instant-based pause
  bookkeeping. The black hole spin phase was the single unguarded
  member of the family.
- **Verification**: new regression test
  `black_hole_ball_spin_survives_multiday_sessions` fast-forwards
  100 h of sim time in 1 h steps, then steps 60 real-time frames and
  asserts the phase still advances at the co-rotation rate (mod 2π)
  and stays under the wrap limit. Verified to FAIL on the pre-fix
  tree (the phase hard-freezes at the accumulated scale) and PASS
  with the fix. Full black_hole suite 69/69; the pre-existing
  co-rotation contract test still green. A/B 10 s benchmark on the
  scene (before vs after, dev profile): density_gini 0.5631 → 0.5589,
  frame_entropy 5.421 → 5.434 bits, dirty cells/frame 113.26 →
  113.37 (+0.10%, the per-cell `rem_euclid` cost), avg fps 3576 →
  3533 (-1.2%, within debug-build run-to-run noise) — no visual or
  performance regression.
- **Scope**: one production file (`black_hole.rs`: wrap limit, wrap
  step, conveyor consumer) + one regression test. No config, CLI,
  or scene API surface touched.

### fix: NIGHT-hunt-2 (post v100) — the commit id vanished from cargo-install builds (-V/--version showed "(unknown)", HUD cid went blank)

- **Root cause**: `build.rs` resolved the commit sha through a
  two-step chain (`git rev-parse --short=7 HEAD`, then the
  `GITHUB_SHA` env var) and dead-ended at an empty string. A
  `cargo install cosmostrix` build extracts the crates.io tarball to
  `~/.cargo/registry/src/…` — there is no `.git` directory and no
  `GITHUB_SHA`, so `COSMOSTRIX_GIT_SHA` was compiled in as `""`. The
  version report fell back to "unknown" (`Build: … (unknown)`), and
  the HUD cid row rendered a BLANK line because
  `option_env!("COSMOSTRIX_GIT_SHA").unwrap_or("unknown")` returns
  `Some("")` — set but empty — never reaching the fallback.
- **Fix**: a third resolution step reads `.cargo_vcs_info.json` —
  the file cargo itself embeds in every published tarball with the
  sha1 of the packaging commit (verified against the real published
  cosmostrix v100.0.0 tarball downloaded from crates.io: sha1
  6c51147…). Zero workflow changes, zero new files, zero
  build-dependencies: the parser is pure std string extraction, the
  shared hex-validation/truncation logic was factored into
  `normalize_short_sha`, and a `cargo:rerun-if-changed` trigger was
  added for the file. Display sites hardened against set-but-empty:
  `hud_init.rs` and `startup_verbose.rs` now treat empty as
  "unknown" (mirroring `diagnostics::info::build_commit_short()`),
  and the two bench sinks (`bench_json.rs`, `bench_report.rs`) route
  through the same helper so reports never emit an empty git_sha.
- **Verification**: `cargo check --all-targets` clean; the standalone
  build-script suite passes 9/9 with three new tests covering the
  vcs-info parser (real clean/dirty document shapes, uppercase hex,
  malformed/truncated documents) and `normalize_short_sha`
  (truncation, trimming, rejection). The real published v100.0.0
  tarball was downloaded from crates.io and confirmed to carry
  `.cargo_vcs_info.json` with the release sha1.
- **Scope**: `build.rs` + four display sites + two doc files
  (`docs/HUD.md` cid sections updated to describe the three-step
  chain). No behavioral benchmark run — the change is compile-time
  metadata plumbing with zero per-frame cost (the HUD string is
  built once in `new()`).

### fix: NIGHT-hunt-3 (post v100) — the build-script epoch drift: two wrong constants that cargo test could never catch

- **Root cause**: the `build.rs` test suite claimed two epoch
  constants that silently disagreed with their documented calendar
  dates: `1_709_210_440` was labeled 2024-02-29 12:34:00 UTC but
  actually decodes to 2024-02-29 12:40:40 UTC (`date -u
  -d @1709210440`), and `1_787_930_200` was labeled 2026-08-04
  15:30:00 UTC but actually decodes to 2026-08-28 15:16:40 UTC
  (`date -u -d @1787930200`). The suite never failed in CI because
  `cargo test` NEVER executes build-script tests — cargo compiles
  `build.rs` as a build dependency, not a test target, so the
  assertions were invisible to every green run.
- **Fix**: both constants replaced with the date-verified values
  (`1_709_210_040` and `1_785_857_400`, each recomputed via `date -u
  -d '<ISO date>' +%s`), a new sub-minute truncation test added
  (`build_time_format_truncates_sub_minute_seconds` — seconds are
  truncated toward the past minute, never rounded up), and the
  standalone runner documented in the test module header plus
  `docs/MAINTENANCE.md` (Quick Reference row, dedicated section, and
  a dormant-mode invariant): `rustc --edition 2021 --test build.rs
  -o /tmp/cosmostrix-build-script-tests && /tmp/…`.
- **Verification**: standalone runner executed locally — the
  pre-fix suite failed 1 of 5 (`build_time_format_matches_known_unix_epochs`);
  the post-fix suite passes 6/6. `cargo fmt --all --check` clean.
- **Scope**: `build.rs` tests + `docs/MAINTENANCE.md` only — no
  production code touched, no A/B benchmark per house rule.

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
