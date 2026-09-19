# Changelog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

cosmostrix uses [SemVer](https://semver.org/). Git tags use a leading `v` (e.g. `v50.0.0`).

The changelog is split into per-era files so this one stays navigable
(split done in NIGHT-docs-1; the former monolith carried every entry
since v50.0.0-beta.6 inside a single Unreleased section):

- Unreleased (post-v100 work) — this file, below.
- [CHANGELOG-V100-ERA.md](CHANGELOG-V100-ERA.md) — the v100.0.0-nightly.1 hunts that built v100.0.0 stable (2026-09-04 to 2026-09-06).
- [CHANGELOG-V80-ERA.md](CHANGELOG-V80-ERA.md) — the v80 line: S-master hunts, the crystal dragon, and the Z-master harmony campaigns (2026-08-30 to 2026-09-03).
- [CHANGELOG-V50-ERA.md](CHANGELOG-V50-ERA.md) — the v50.0.0 pre-release line plus the condensed v13-v25 releases.
- [Pre-v13 archive](docs/archive/CHANGELOG_PRE_V13.md) — pre-v13 history (v2 to v12).

The condensed origin story stays at the bottom of this file. The
`## v4.0.0` and `## v3.9.0` headings are tripwire-locked by
`test/docs_tests/metadata.rs` and must remain in this file (see the
tripwire note in the pre-v13 archive).

---

## Unreleased

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

### fix: retire the version-prefixed demo asset scheme — tests and version-to.sh aligned with the 27f7d4f5 asset refresh

- **Root cause**: the asset refresh in 27f7d4f5 moved the demo assets
  to stable, version-less names (`cosmostrix-video.gif`,
  `cosmostrix-video.webp`, `cosmostrix-4-scene.png`) and dropped the
  five `cosmostrix-v100-demo-*.png` screenshots and
  `cosmostrix-v100-demo.gif`. Three surfaces were still keyed to the
  old scheme and went red: five `docs_tests::assets` guards (expected
  `assets/cosmostrix-v100-demo.gif` and at least three
  `cosmostrix-v100-demo-*.png` files), the `version-to.sh`
  `update_assets`/`update_readme_demo_refs` machinery (would have
  renamed nothing on the next major bump and silently skipped), plus
  the gate-keepers fallout fixed separately (tools.md SPDX/disclaimer/
  MD026 and 664→644 modes).
- **Fix**: `test/docs_tests/assets.rs` rewritten around the new
  invariants — video assets (gif + webp) exist, the 4-scene png
  exists, README references the webp demo and the scene screenshot,
  the video demo appears before the screenshot, and any
  `cosmostrix-v*-demo` file or README reference (the retired scheme,
  any major) is rejected. The version-prefix helpers
  (`major_prefix`/`major_num`) are gone; guards are now
  version-agnostic by construction, so version bumps no longer touch
  asset tests. `version-to.sh` drops the dead
  `update_assets`/`update_readme_demo_refs` functions and the
  `ASSETS_DIR` tracking block (steps renumbered); its ABOUT_CI.md
  documentation already described only the TAG= line, so no doc drift.
- **Verification**: targeted `docs_tests::assets` run 7/7; full suite
  2932 passed / 0 failed (59s); `version-to.sh --check
  100.0.0-rc.1` all-consistent; shellcheck + shfmt clean on the
  edited script; `build.sh check-all -q` exit 0; gate-keepers 18/18.
- **Scope**: test + script + docs only — no production Rust code
  touched, no A/B benchmark per house rule.

### fix: CI test flake — the termdetect env race was three split ENV_LOCKs (run #1970)

- **Root cause**: CI run #1970 failed
  `termdetect::tests_ancestor::dynamic_fps_source_records_term_substring_layer`
  ("standard/unknown fallback" instead of "TERM substring"). The three
  termdetect test modules (`tests.rs`, `tests_ancestor.rs`, `tests_hunt24.rs`)
  each declared their OWN `static ENV_LOCK` — a module-local lock only
  serializes that module's tests against itself, but the default test
  harness runs tests from DIFFERENT modules in parallel threads. A
  `tests.rs` test writing `TERM=xterm-256color` + `TERM_PROGRAM=
  gnome-terminal` (the fallback fixture) could land between a
  `tests_ancestor.rs` test's env setup and its `detect()` call, so the
  substring test read the fallback fixture and recorded the wrong
  layer. Local runs passed by timing luck; CI's smaller runner hit the
  window.
- **Reproduction**: first `--test-threads 32` run on the pre-fix tree
  produced a storm of 15+ cross-module env-race failures across the
  termdetect module — the race was wide open, not a 1-in-a-thousand
  window.
- **Fix**: ONE shared lock. `ENV_LOCK` in `tests.rs` is now
  `pub(super)` and both sibling modules import it as
  `use super::tests::{EnvGuard, ENV_LOCK}` (the same visibility/import
  pattern `EnvGuard` already used). 20/20 stress rounds clean at 32
  threads after the fix; suite still 2932/2932.

### fix: test-parallelism audit — four more latent races hardened + two extreme-contention stragglers documented (32-thread stress)

- Background: stress-running the full suite at `--test-threads 32`
  (8x CI's default parallelism) surfaced the remaining members of the
  same race family as the termdetect flake above. Every fix below is
  verified by a 20-40-round 32-thread stress battery on the touched
  filter; the full suite went from a storm of failures per round to at
  most the two documented stragglers.
- **output warning counter**: the 2026-08-19 audit locked only the two
  exact-count tests in `test/output/output_tests.rs`, guessing the
  concurrent emitters were "config apply tests". 32-thread
  reproduction showed the REAL emitters: the `sanitize_message_text`
  tests in `src/output/message.rs` — wide/CJK/emoji/control-char
  replacement warns via `eprintln_warn_labeled` and bumps the global
  `STARTUP_WARNING_COUNT` from a lock-free module (observed: count 4
  instead of 3). `TEST_WARNING_COUNT_MUTEX` is now `pub(super)` and the
  four warning-emitting sanitize tests hold it. 40/40 stress rounds
  clean.
- **msg reveal t=0 wall-clock leak**: `set_message_elapsed(cloud, text,
  0)` backdated the reveal timeline by exactly 0 ms, so any scheduler
  delay between the helper and the test's `draw_message()` flipped
  alpha > 0 — "content visible at t=0" (observed: 10 cells at alpha 0).
  t=0 now arms the timeline 10 s in the FUTURE, which is
  production-identical to `hold_message_behind_intro`'s intro lead
  (`Instant::elapsed()` saturates to zero until the start passes) and
  makes every "nothing revealed at t=0" assertion deterministic. All
  seven t=0 call sites across the msg_fill/cascade/scorch/radar/
  hologram suites inherit the fix from the shared helper.
- **reveal-budget boundary tests**: `typewriter_reveals_progressively_
  like_pre_v51` and `engrave_reveals_progressively_like_typewriter_
  pacing` asserted exact cell counts at exactly 160 ms — 160/80 = 2.0,
  dead on the 80 ms/char reveal boundary, where 1 ms of harness delay
  flips the count. The engrave champion-contract test compared two
  clouds at 320 ms (320/80 = 4.0, also an exact boundary, with the two
  `draw_message` calls sampling real time at different instants —
  observed flipping one cloud to 3 cells). All three moved to
  dead-center bucket values (200 ms / 360 ms) and the champion test
  additionally pins both clouds to one shared start instant.
- **config HOME race**: `test/config/configfile_tests_inline.rs` had
  zero locks — its own HOME/XDG-removing resolver test could interleave
  with its HOME-derived candidate-path reads (observed:
  `config_candidate_paths_includes_default_path` comparing two reads
  across a HOME swap), and the safepath suite's HOME mutations (guarded
  by their own module-local lock) left the same cross-module window
  open. safepath's `ENV_LOCK` is now the crate's ONE shared HOME-family
  lock (`pub(crate)`, test-build-only visibility): the configfile
  termux-detection, candidate-path and resolver tests import it.
  20/20 stress rounds clean.
- **documented stragglers**: `hunt26_resync_force_does_not_reseed_old_
  writes` (roughly a third of 32-thread full-suite rounds) and
  `dna_drops_spawn_to_sparse_calm_sky_target` (1 sighting in ~15
  rounds) fire only under extreme full-suite contention — zero
  failures at CI's 4-thread default (10 clean local rounds + all CI
  history). Both carry evidence-trail doc comments at the test site;
  left documented rather than chased per the no-over-engineering
  rule.

### fix: NIGHT-hunt-46 & docs-7 (second pass) — README flag audit + the audit tools' own stale truth columns

- **README.md**: full flag-surface audit against the live binary
  (`--help` + behavior probes) found one real stale flag — the
  custom-palette bullet referenced `--colors <name>`, which the binary
  rejects with "unexpected argument" (the correct surfaces are
  `--color <name>`, which accepts custom palette names, and
  `--colors-custom <name>`). Everything else verified against source:
  14 rain styles, 44 themes, 19 chroma lock invariants (INV-1..19),
  screen limits 1024x500 / 7680x4320, the 6-size bench ladder, ambient
  1..=24 entries, notify channel cap 64, MSRV 1.98.1, 24-block charset
  budget, 200-char message boundary, lock-test paths, config-path
  whitelist, and every cross-referenced doc and script path.
- **scripts/stale-hunt.py**: the `ls --version --all` analogy in
  `src/cli/early_returns.rs` (standard early-exit semantics prose) is
  an external-tool behavioral parallel, not a cosmostrix flag —
  allowlisted in EXTERNAL_TOOL_FLAGS with the justification comment.
  Stale flag/path/module counts: 0 across 505 scanned .rs files.
- **scripts/docs-audit.py**: the audit tool's own truth columns were
  the stale-data teacher — path notes still pointed at the
  pre-`src/engine/` layout, and the power-subsystem note had the
  module name words swapped (`central_control_dragon_power` for
  `src/central_control_power_dragon/`, a path that does not exist).
  Count truths refreshed: 2947 `#[test]` fns (2415 in `test/` + 532
  in `src/`; last full run 2932 passing), 508 tracked .rs files,
  44 themes, 19 invariants. All residual section-2/3 hits re-triaged
  as timestamped historical records (CHANGELOG entries, KEY.md
  signoff blockquotes, research snapshots with their headers, and
  docs quoting audit output as its subject).
- **docs/FUTURE_BACKLOG.md**: the registry's own "Current count is
  2527" line refreshed to the current truth, and a second-pass
  re-triage note added to the status header recording this audit's
  findings.

### fix: NIGHT-hunt-48 — repo-wide emoji ban + gate-keepers check 15, and the CI shfmt regression it caught on the way

- **Owner rule (2026-09-14): the project carries no emoji anywhere.**
  `scripts/emoji-audit.py` was a manual, non-blocking, .md-only sweep;
  it is now a strict repo-wide detector wired into `gate-keepers.sh`
  as check #15 (exit 1 on any hit). Scope: every git-tracked file that
  decodes as strict UTF-8 — `.md`, `.rs`, `.sh`, `.py`, `.yml`,
  `.toml`, extensionless text; binaries fail the decode and are
  skipped, so file types cannot escape by extension. Fail classes
  mirror the RULES.md Output Glyph Policy blocks (U+1F000-1FFFF,
  U+2600-27BF, U+2300-23FF, U+2B00-2BFF, variation selectors, ZWJ) —
  the emoji sweep and the symbol-only output gate now enforce the same
  vocabulary. `docs/archive/**` and bench-labs artifacts stay excluded
  (frozen/generated); the two data exemptions carry over (the denylist
  script itself, the `message.rs` sanitizer test input).
- **`--fix` is fail-class-scoped.** The first draft's fix pass replaced
  U+25B6 in `src/engine/cosmic_dragon_engine/cloud/living_rain.rs` — a
  geometric ART glyph in the doc-comment state diagram that the RULES
  classes allow. Caught by diffing the tree before commit, never
  shipped; the fix table is now the fail-class subset of the
  replacement mapping and the two non-fail-class entries (U+25B6,
  U+2139) are dropped.
- **Inaugural strict run found the expected residue and purged it**:
  24 hits across 2 live files — `CHANGELOG.md` (9 marks) and
  `docs/research/RAIN_BORDER_TOUCH_SPARK_RESEARCH.md` (15 marks), all
  check/cross marks from before the v80.0.0-beta.2 reclassification,
  mapped to `OK`/`X` per the established house convention.
- **CI regression found and fixed on the way**: the Gate-keepers
  workflow had been red since the NIGHT-hunt-47-depthbore push —
  commit 39b27e1's gate-keepers.sh edit silently re-indented the file
  from tabs to 8 spaces, failing the CI `shfmt -d` canonical-format
  check (locally invisible because shfmt was never installed).
  Re-canonicalized with `shfmt -w` at the CI-resolved upstream
  (v3.14.1); all other `.sh` files were already clean. Local gate
  tooling now installs the full CI parity set (shellcheck, shfmt,
  yamllint, codespell, ruff) so the gap cannot reopen.
- **Pre-push drill protocol (owner rule, 2026-09-14)**: before every
  big push, run `scripts/depthbore/depthbore.py --parts 15 --quick`
  (RACE-STORM + DRIFT-SOAK, ~75 s) — documented in
  `docs/DEPTHBORE.md`. First drill of this session: 15 PASS / 0 FAIL,
  74 s, verdict CLEAN.

### fix: NIGHT-hunt-47-depthbore — the LTS depth bore + three deep bugs it drilled out (doctor config blindness, testconf cap bypass, fork-guard reparent race)

- **The depth bore** (owner mandate 2026-09-14, after the DeepSeek
  review of where the surviving bugs hide): new flagship harness
  `scripts/depthbore/depthbore.py` — five bores against the real binary
  from a clean-slate zero state to the fully loaded hero state.
  RACE-STORM (signal kills, spawn/kill storm with fork-guard tracking,
  35-resize SIGWINCH storm, 20 ms startup config-write race, 70 ms
  valid/broken live-reload churn), PLATFORM-MATRIX (Termux/tmux/ssh/
  dumb-TERM/NO_COLOR/LANG=C environments; FreeBSD and Windows surfaces
  honestly SKIPped, never claimed from a Linux box), EDGE-CRUSHER
  (1x1 to 10000x10000 geometries, the 24-block-per-family custom
  config bound, the 200-char message boundary, config byte edges),
  CONFIG-FUZZ (38 deterministic mutants that must classify cleanly as
  rc=2-with-stderr-diagnostic or rc=0), and DRIFT-SOAK (bounded 24 h
  proxy: RSS/fd/thread sampling plus output-rate windows under a still
  soak and a reload-churn soak). Inaugural run: 119 PASS / 0 FAIL,
  2 platform skips.
- **Bug 1 — doctor was blind to a present-but-unreadable config**: the
  runtime loader treats an unreadable default config (invalid UTF-8,
  EACCES, past the 1 MiB cap) as "no config" by design, and
  `--testconf` reports it with rc=2 — but `cosmostrix --doctor` printed
  a fully healthy report while the user's settings were silently
  ignored. Fix: the CONFIG FILE report section (`status` + the
  effective fallback source + a testconf hint; see
  `src/doctor/mod.rs::config_file_status`, pinned by five new tests in
  `test/doctor/tests.rs`). Exit codes unchanged — the strict rc=2
  contract stays reserved for parse errors (hunt-44).
- **Bug 2 — `--testconf` bypassed the 1 MiB size cap**: it was the one
  config read path not funneling through `read_config_capped`
  (the S-master-3-v2 invariant), so a 1.2 MB file read unbounded into
  memory AND reported rc=0 "valid" while the runtime refused the very
  same file. Fix: the capped read; both surfaces now agree, and the
  error names the cap.
- **Bug 3 — the fork guard lost the kernel reparent race**: PDEATHSIG
  wakes `cx-term-guard` microseconds after the parent task exits, but
  the kernel can take ~200 ms to finish reparenting, so the old
  `getppid() == 1` check read the DEAD parent's pid and silently
  skipped the terminal restore — the depthbore SIGKILL bore measured
  only 25-75% restore rates (and subreaper containers lose that check
  permanently). Fix: liveness polling on the renderer pid captured at
  fork time (up to 6 s, covering the watchdog's force-exit window),
  gated by a termios-still-broken check so a graceful exit stays
  byte-identical (no trailing restore escapes after the parent's own
  cleanup). Measured after: 8/8 restores on SIGKILL/SIGINT/SIGTERM.
  See `docs/TERMINAL_KILL_CLEANUP.md` and `docs/DEPTHBORE.md`.
- **Gate parity**: `scripts/gate-keepers.sh`'s ruff scan widened from
  `scripts/*.py` (maxdepth 1) to the whole `scripts/` tree, mirroring
  the `.sh` convention — a harness cannot escape the gate by living in
  a subdirectory.

### fix: NIGHT-depthtest-5 & hunt-46 — the static post-config commands (--version/--docs/--check-update) died behind unrelated config errors

- **Flow-separation matrix on the remaining surfaces** (owner
  mandate 2026-09-14, pre-LTS): `--version`, `--docs`, and
  `--check-update` render content no config value can alter, yet a
  single typo'd key in config.toml killed all three behind the rc=2
  config error — while `--help`, the same class of static reference
  content, worked because it sits pre-config. A user with a broken
  config could not even run `cosmostrix --version` for a bug report,
  and `cosmostrix --docs | less` (a documented pipeline-safe surface)
  died behind the config error.
- **Fix**: the Boundary-3-failure rescue — when a config-independent
  command wins the post-config ladder, it is dispatched before the
  die (`cli/early_returns.rs::handle_config_apply_failure`, wired
  from main.rs). Ladder preserved exactly: `--doctor` alone or
  combined keeps the hard death (a config failure IS its diagnostic
  surface, the hunt-44 contract), pre-config commands unaffected,
  order between the rescued commands unchanged (one shared
  `dispatch_post_config` table). Invalid runtime-flag values lose to
  the rescued command — the same inert-flag contract `--help`
  follows.
- **Ambient/crystal-dragon PTY harmony audit** (the second approved
  direction): flagship harness
  `scripts/depthtest8_ambient_crystal_pty.py` (33 assertions) drives
  both engines together at a tuned cadence and observes the
  `ambient_diag` exit-summary counters via a clean 'q' quit —
  ambient startup (builtin + custom scene/palette), mid-run scene
  switch (scheduler refire → rx apply), ambient removal (overlay
  lift → revert), crystal-only drift self-reset, and error ordering
  under the harmony load. All green; the state machine is sound.
- **Comment audit**: `set_palette`'s stale drift-gate reference
  ("rain.rs:923", a file that no longer exists) corrected to point
  at the real condition in `cloud/post_rain.rs`; depthtest-5/6/7
  harnesses brought to ruff check + format green (PLW0602, F541,
  ISC004, SIM115, C401 — pre-existing).
- **Verification**: depthtest-8 33 PASS / 0 FAIL; regressions
  depthtest-5 95, depthtest-6 155, depthtest-7 38, cli-config
  stresstest 47 — all PASS; fmt/clippy/check-all/gate-keepers green;
  benchmark A/B shows no render-loop delta (happy path untouched).
  Full record: `docs/LIVE_RELOAD_BEHAVIOR.md` §20.

### fix: NIGHT-hunt-41 — startup validation silent-ignore for configs with only-unknown keys (msg-modey = true passed silently)

- **NIGHT-hunt-41** (owner fatal report, 2026-09-13, found by manual
  testing): `msg-modey = true` in config.toml passed `cosmostrix -v`
  silently — the binary kept running with the typo'd key dropped.
  `--testconf` already rejected it ("unknown key 'msg-modey' (likely
  typo)" + a did-you-mean hint pointing at `msg-mode`), producing the
  asymmetric "testconf rejects, startup accepts" verdict class the
  owner rejected. Root cause: the startup validation block at
  `src/config/config_apply.rs:135` was gated on
  `!parsed_cfg.values.is_empty()`, but a config whose ONLY key is
  unknown has empty `values` (unknown keys go to
  `parsed.unknown_keys`, not `parsed.values`), so the guard
  short-circuited and Layers 1/1.5/2/3 (malformed / duplicate /
  unknown / strict-value) were ALL skipped. The same short-circuit
  also masked header-only custom blocks (NIGHT-hunt-37 completeness
  contract): a config with only `[scene-custom.x]` and no field
  lines has empty `values` AND empty `unknown_keys` — the only
  signal is `custom_block_headers`.
- **Fix**: the guard now fires when ANY of the parsed-record vectors
  is non-empty (`values`, `unknown_keys`, `malformed_lines`,
  `duplicate_keys`, `duplicate_sections`, `custom_block_headers`).
  The four diagnostic functions (`startup_malformed_error`,
  `startup_duplicate_error`, `startup_unknown_error`,
  `validate_config_strictly_parsed`) all safely early-return on
  empty inputs, so an empty config (legitimate first-run state)
  still passes through. Production builds never set
  `COSMOSTRIX_SKIP_STARTUP_VALIDATION` (test-only bypass).
- **Strength repro**: `scripts/night_h41_msg_modey_repro.py` drives
  the REAL binary through the owner's exact repro plus three
  sibling cases (a different key typo, a mixed known+unknown pair, a
  duplicate key, a known-good control). Uses `--doctor` (a
  POST-config early-return flag) so the binary runs `apply_config`
  (where validation lives) but does NOT enter interactive mode
  (which would die on a headless terminal and mask the verdict).
  Exit 0 = all 12 expectations met.
- **Flagship E2E tester**: `scripts/depth-test-config.py` — the
  owner's mandated "flagship class end to end depth supermassive
  testing for all existing functions on config.toml". Drives the
  real binary through 50 cases across 18 validation classes:
  key typos, value typos (enum/string), value range violations,
  value type mismatches, duplicate keys, duplicate `[section]`
  headers, empty values, separator typos (NIGHT-hunt-38 class),
  header-only blocks (NIGHT-hunt-37), over-cap block counts
  (NIGHT-hunt-40 max-24), over-cap ambient entries, over-length
  block names, missing required fields, invalid hex colors,
  unknown scene in ambient, mixed-syntax garbage, custom-block
  field typos, valid baseline (sanity). Each case asserts the
  expected verdict on the appropriate surface (--testconf,
  startup, runtime). Exit 0 = all 50 expectations met (current
  status: 50 PASS, 0 FAIL).
- **Rust unit tests**: two new in-tree tests in
  `test/config/config_apply_tests/strict_mode.rs` pin the bug class
  directly (`strict_startup_rejects_config_with_only_unknown_key`
  and `strict_startup_rejects_header_only_custom_block`). The
  existing tests used mixed known+unknown configs (which made
  `values` non-empty and the bug never manifested); these two new
  tests use pure-unknown / header-only configs to lock the contract.
- **No visual/perf surface touched**: the validation path runs at
  config-parse time, zero per-frame impact. No A/B bench needed
  (the hunt-39 / hunt-40 benches already proved zero per-frame delta
  for the validation-contract class).

---

### fix: NIGHT-hunt-40 — tighten the custom-namespace entry budget from min 1 / max 64 to min 1 / max 24

- **NIGHT-hunt-40** (owner mandate, 2026-09-13): the four
  user-extensible namespaces (charset-custom, colors-custom,
  scene-custom, ambient schedule) now share a single min-1/max-24
  entry policy, tightening the NIGHT-hunt-39 max-64 ceiling. The
  constants `COLORS_CUSTOM_MAX_BLOCKS`, `CHARSET_CUSTOM_MAX_BLOCKS`,
  `SCENE_CUSTOM_MAX_BLOCKS`, and `AMBIENT_MAX_ENTRIES` all change
  `64 -> 24`. Every cap stays a HARD validation error on every
  surface (`--testconf` exit 2, startup exit 2, live-reload watcher
  reject); the collector silent-skip / truncate stays as
  defense-in-depth for `COSMOSTRIX_SKIP_STARTUP_VALIDATION` bypass
  runs only. Boundary tests renamed `_over_64` -> `_over_24` /
  `_exactly_64` -> `_exactly_24` and remain parameterized on the
  constant. `docs/RULES.md`, `docs/FAQ.md`,
  `docs/AMBIENT_SCHEDULER.md`, and the chroma-dragon `KEY.md` unlock
  trail are updated. The hunt-39 e2e strength script is renamed to
  `scripts/night_h40_entry_budget_e2e.py` and its `CAP` constant
  tracks the new 24 ceiling (no test logic changes — the boundary
  cases parameterize `CAP`). No visual/perf surface touched: the
  validation path runs at config-parse time, so no A/B bench rerun
  is needed (the hunt-39 bench already proved zero per-frame delta
  for this exact contract class).

---

### fix: NIGHT-hunt-38-supermassive + NIGHT-hunt-39 + NIGHT-docs-4 — separator-typo rejection, the msg-mode validator, the 1..=64 entry policy, and the rendering-engine Q/A

- **NIGHT-hunt-38-supermassive** (owner fatal report, found by manual
  testing on commit 6198431): three stale/inconsistent validation
  behaviors for the SAME typo class, now closed with a
  strength-repro script (`scripts/night_h38_supermassive_testconf_repro.py`)
  driving the real binary through all four owner repros:
  1. `set == "x"` silently passed — `split_once('=')` stored `= "x"`
     as the charset value (garbage glyphs) while `--testconf` said
     PASS. The parser now rejects the double-equals line as
     malformed with a targeted `# ERROR: double '='` note
     (`configfile/configfile_syntax.rs`; quoted `set = "=x"` stays
     legal — the guard inspects the raw value before quote-stripping,
     the bug #19 invariant).
  2. `set : "x"` (YAML/JSON habit) got only the generic malformed
     diagnostic; it now gets the targeted `':' is not a TOML
     separator` note. Both typo forms of one mistake now behave
     identically on every surface.
  3. `msg-mode = truee` passed `--testconf` (no validator arm), then
     the runtime printed a bare one-line error and KEPT RUNNING with
     the default — three different verdicts for one typo. The
     `msg-mode` bool arm now matches `parse_bool_config`'s lenient
     vocabulary (true/false, yes/no, on/off, 1/0) uniformly:
     `--testconf` exit 2, startup exit 2, live-reload reject.
  4. The ambient legacy-format migration essay was STALE: it
     recommended `base-scene` — a field removed in v80.0.0-beta.2 —
     so following it produced a fresh unknown-field error. The essay
     (and the module docs) now show the complete seven-field
     `[scene-custom]` contract; a `==` typo on an ambient key can no
     longer masquerade as the legacy multi-field format (the parser
     rejects the line first).
- **NIGHT-hunt-39** (owner mandate): min 1 / max 64 entries for all
  four user-extensible namespaces — `[charset-custom]`,
  `[colors-custom]`, `[scene-custom]` (64 blocks each, down from
  100) and the ambient scheduler (`AMBIENT_MAX_ENTRIES` 64, down
  from a silent 256-collector-truncate; 65+ entries is now a hard
  error on every surface via `validate_ambient_entries`). The
  min-1 side is the NIGHT-hunt-37 completeness contract restated:
  a header-only (zero-entry) block is a hard error. Config
  template comments updated; e2e boundary verification in
  `scripts/night_h39_entry_budget_e2e.py` (64 PASS / 65 FAIL on
  every namespace, both through the real binary).
- **NIGHT-docs-4**: FAQ gains a "Rendering engine" Q/A section —
  does cosmostrix really have an independent rendering engine (yes:
  no TUI framework; crossterm is bounded to setup/teardown/input,
  the hot draw path is hand-written SGR bytes), is the Cosmic
  Dragon really rendering (it IS the renderer — cloud → Frame →
  Terminal::draw), and which dragon paints (exactly one: Cosmic;
  Chroma decides color, Crystal decides when, Power decides rate).
  Plus a "Config validation" Q/A for the typo classes above, and
  the ambient multi-field migration caveat. Stale-doc sweep in the
  same round: `docs/AMBIENT_SCHEDULER.md`'s migration example was
  missing its `rain` line (six of seven fields — copy-pasting it
  failed the completeness rule it sits next to); fixed.
- Tests: +14 (parser typo rejection, ambient essay content + the
  1..=64 boundary trio, colors-custom 64-cap, msg-mode vocabulary
  through both validators). Full suite 2918/0/2. chroma KEY.md:
  UNLOCK + re-lock entry for the `COLORS_CUSTOM_MAX_BLOCKS`
  100→64 constant change (validation contract only, zero pipeline
  math — same class as the hunt-37 unlock).

### feat: NIGHT-hunt-38 — the force-repaint classifier promoted to scripts/ as a standing audit tool + a harness parser fix it uncovered

- Owner approval (2026-09-13, depth-hunt-1 follow-up): the
  force-repaint frame-state membership probe — previously a one-off
  method recorded in the depth-hunt-1 audit — is now
  `scripts/night_h38_force_repaint_classifier.py`. Per style: five
  checkpoints over a 12 s window freeze-detect (the sweep's
  multi-checkpoint rule), then the HUD toggle ('i' on/off) fires
  force_draw_everything, then the classification — frozen + survives
  repaint identical = LIVE STATIC; frozen + changed = ADVANCING;
  frozen + blanked = ORPHAN (a real cleanup bug). Exit 1 on any
  orphan, exit 0 when every frozen cell is frame-state content.
  Verdict on the current binary: all thirteen structured styles
  clean; neural's frozen set is live static (dormant neurons) plus
  advancing cells, matching the depth-hunt-1 probe.
- The promotion immediately paid for itself: the first classifier
  runs flagged 1-2 "orphans" per monolith run. Investigation chain:
  the stuck-cell sweep exempts structured styles (they own their
  cleanup); phosphor ghosts live ~0.4 s (cannot be 12 s frozen); an
  in-process frame-ownership oracle
  (`hunt38_monolith_oracle_no_orphan_cells_persist`, kept as a
  regression test) proved the frame state clean; a raw-stream replay
  with byte-exact write tracing traced the "frozen" glyphs to
  CSI-fragment characters ('[', 'H', 'm', ';', digits) — the PTY
  harness's own parser was painting escape-sequence bodies as
  glyphs.
- Root cause (night_cbg34_e2e.py SyncScreen.feed): the lone-ESC
  strip regex ran BEFORE the trailing-ESC holdback, so a CSI
  introducer split across feed() chunks (reader bursts have
  arbitrary boundaries) was DELETED and the sequence body painted
  as text; those phantom cells persisted for seconds (the app never
  re-emits cells it believes unchanged) and read as frozen cells /
  false orphans. Fix: hold back the trailing ESC before the strip.
  Byte-exact replay repro (feed one byte at a time: the first two
  ESCs of the stream vanished and "[?25l" painted at (0,0)) now
  shows zero fragment glyphs. Every harness consumer — the h34
  style sweep, the classifier, future probes — inherits the fix.
- No runtime Rust code changed (a python harness, a new audit
  script, one new test) — benchmark skipped per the A/B contract.

### docs: NIGHT-docs-3 — FAQ: the OKLab question and the custom-block contract, documented once

- Owner question (2026-09-13): "is cosmostrix use OKlab when using
  color custom on config?" — YES, and the answer now lives in
  `docs/FAQ.md` as the first entry of a Q/A record: a
  `[colors-custom.<name>]` block routes through the same OKLab polar
  gradient engine as built-in themes (`to_palette` ->
  `colors_from_stops` -> `gradient_from_stops_oklab`), resampling the
  user's 2-9 stops to 9 perceptually-uniform samples — which is also
  why the hunt-37 rain-stop ceiling is 9.
- Related entries cover: why OKLab (and why polar, not Cartesian),
  where else OKLab appears (intro blends, transition smoothing, the
  300 ms palette wave), built-in theme parity (steps: 9 in the
  catalog), terminal color-mode fallbacks (256/16/mono quantization
  after the OKLab build), the custom-block completeness contract
  (post-hunt-37), and live-reload rejection semantics.
- Indexed in docs/README.md (Quick Navigation + the docs table).
  Source-file references in every entry (source code is truth).
  Docs-only change — no benchmark per the A/B contract.

### fix: NIGHT-hunt-37 — custom blocks must be complete and strict (rain-stop ceiling 9, required bg, header-only blocks visible)

- Owner mandate (2026-09-13): custom config blocks (charset, colors,
  scene) must be COMPLETE — no missing fields, no overloads or
  duplicates, no empty values, names capped at 64 chars — and the
  caps must be hard errors, never silent drops. The owner's repro: a
  `[colors-custom.test]` block with a 67-stop rain array passed every
  gate (the documented limit was 64, enforced as a silent collector
  truncation), and it must not run.
- colors-custom strictness (`colors_custom/strictness.rs`):
  - rain stops: min 2, max 9 — the ceiling now matches
    `COLORS_CUSTOM_PALETTE_STEPS`, the count the OKLab engine
    resamples every palette to, so stops beyond 9 are provably
    discarded input. Over-limit blocks are a HARD error on all three
    surfaces (startup exit 2, --testconf exit 2, live-reload watcher
    reject). The collector's truncation remains only as
    defense-in-depth for COSMOSTRIX_SKIP_STARTUP_VALIDATION bypass
    runs.
  - `bg` is now REQUIRED: `to_palette` (the load contract every
    surface funnels through) rejects a rain-only block — a complete
    `[colors-custom.<name>]` block defines BOTH bg and rain.
  - `rain` + the deprecated `stops` alias in one block is an overload
    error (both used to be silently concatenated into the gradient
    stop list).
  - the 100-block ceiling is a hard error (was the collectors' last
    silent skip — survivors were unspecified HashMap-order drops).
- Header-only custom blocks are now visible to validation: the config
  parser records every `[scene-custom|colors-custom|charset-custom.
  <name>]` section header in `ParsedConfig::custom_block_headers`
  (dedup + sorted), and `testconf::custom_block_headers` rejects a
  block that defines none of its required fields — a
  `[charset-custom.zen]` with `set` commented out, a
  `[colors-custom.test]` missing bg or rain, a `[scene-custom.x]`
  with every field commented out. Previously such blocks produced
  zero keys and were invisible to every key-level validator. Wired
  into all three surfaces via `validate_config_strictly_parsed`
  (startup + watcher) and inline in `--testconf`.
- Name-shape checks now cover headers: empty names (a bare
  `[colors-custom]` header), invalid characters, and oversized
  names on header-only blocks (previously invisible to the
  key-scanning name-length validators).
- charset-custom and scene-custom gained the same block-count hard
  error (`validate_charset_custom_block_count`,
  `validate_scene_custom_block_count`).
- One shared rain-stop splitter (`split_rain_stop_entries`) now
  serves the collector, --testconf's value checker, and the
  strictness validator — the three hand-written twins (the F-23-1
  drift risk) are deleted.
- Tests: 26 new regression tests (strictness contract, header
  completeness, parser header recording); the colors-custom inline
  tests moved to the test/ mirror tree (NIGHT-hunter-1 convention)
  to hold the 800-LOC cap. Full suite 2905/0/2. e2e:
  custom_features_stresstest.sh extended to 38 cases (all green),
  including the owner's 67-stop repro. Chroma KEY.md UNLOCK entry in
  the same commit (validation contract only — no pipeline math; the
  dragon is re-locked at the new contract).

### fix: NIGHT-hunt-36 — the stuck-cell sweep still never ran on default runs (message gate) + a phosphor transposition in the sweep

- Owner report (NIGHT-hunt-36, 2026-09-13): micro glitch shift rain on
  the glyph type — some rain glyph cells near the top or bottom of the
  screen stay stuck for a long time, needing another droplet to pass
  over them (or a long wait) before they disappear. The hunt-17 sweep
  (7247862) was supposed to own this class; the task re-audits it for
  LTS, and extends the audit to all thirteen other rain types.
- Root cause 1 (the sweep was still dead on every default run): the
  sweep kept a whole-function message gate — `if !self.message.is_empty()
  { return; }` — guarding against overlay false positives. But the
  default interactive config ALWAYS carries the built-in fallback
  message ("Experience a masterpiece with cosmostrix vX", wired in
  build_cloud_cfg whenever msg-mode is on and no -m text is given), so
  `self.message` is never empty on a default run and the sweep NEVER
  executed. Hunt-17 removed the --perf-stats gate; hunt-32 removed the
  structured-style exposure; this gate was the last thing keeping the
  correctness mechanism off — exactly the hunt-32 side note ("the
  default message banner gates the sweep off entirely") read as a
  footnote instead of as the remaining bug.
- Fix 1: per-cell rectangle exemption. `relayout_message` now caches
  the overlay box bounds (`message_sweep_top/bottom/left/right`,
  half-open, refreshed for BOTH the bordered and borderless layouts;
  collapsed to empty when the box does not fit or no message exists).
  The sweep runs with the overlay visible and spares only cells inside
  the box — `draw_message` owns and rewrites that whole region via
  set_force every frame, so nothing there can be stuck. Everything
  outside the rectangle is back under sweep jurisdiction: 4 new fields
  on Cloud, zero new per-frame cost (bounds hoisted once per sweep).
- Root cause 2 (found while fixing 1 — the sweep consulted a
  transposed cell's phosphor energy): the sweep read
  `self.phosphor[i]` with the frame's ROW-major walk index, but the
  phosphor arrays are COLUMN-major (pidx = col *lines + line, see
  phosphor.rs Pass 1). Live decaying ghosts whose transposed slot held
  energy were skipped (orphans stayed stuck), and cells whose
  transposed slot sat at zero were force-cleared while their own slot
  carried live energy — visible as phosphor afterglow flickering off.
  The sweep was the ONLY transposition site in the codebase (audited:
  every other phosphor access derives pidx as col* lines + line).
- Fix 2: the sweep indexes the cell's own column-major slot, with the
  2D coordinates hoisted once per cell (col/line are now computed
  before the phosphor check and shared with the rectangle exemption).
- Thirteen-style audit (the owner's ask): the two bugs live exclusively
  in the Glyph-only sweep path — the hunt-32 droplet-family gate
  returns before either line executes for the thirteen structured
  styles, and the hunt-32 fourteen-style audit table still passes
  untouched (their vacated cells are owned by the monolith-style diff
  cleanup contract). The only cross-style change is the rectangle
  cache write in relayout_message, a pure field assignment with no
  behavioral surface for any style.
- 6 new tests (tests_stuck_cells_hunt36.rs): the sweep runs with the
  fallback message active (the headline regression), the column-major
  phosphor lookup (live ghosts survive), the borderless and bordered
  exemption rectangles (white-box bounds + behavioral edges/corners),
  the rectangle collapse when the box stops fitting, and an end-to-end
  orphan-cleared-through-rain_at repro with the fallback message. Two
  stale tests rewritten to the new contract:
  p4_sweep_skips_when_message_active became
  p4_sweep_with_message_spares_only_overlay_box (outside-box orphans
  are cleared with the overlay visible; inside-box glyphs are spared),
  and hunt25_resync_still_emits_stuck_cell_clears now plants a STALE
  orphan (plant, clear_dirty to stale the per-frame write stamp, zero
  the phosphor slot) — the old plant-then-sweep sequence only ever
  passed against the transposition bug, because phosphor decay Pass 1
  legitimately re-arms cells written this frame. Suite 2879 green
  (0 failed), clippy -D warnings clean, fmt clean.

### audit: NIGHT-depth-hunt-1 — the pre-super-LTS depth audit (security, hidden bugs, five prior-item verification)

- Owner directive: cosmostrix should be 99 percent free of hidden
  bugs, security vulnerabilities, and residual problems before the
  super-LTS declaration. Full report:
  docs/audits/NIGHT_DEPTH_HUNT_1_AUDIT_2026-09-13.md. Verdict: zero
  security vulnerabilities, zero runtime panic defects, zero hidden
  rendering bugs; three process/hardening items found and fixed in
  the same round.
- Five previously-reported items re-verified against the current
  tree (not the commit messages): the three-dragon documentation
  (README/PHILOSOPHY/THREE_DRAGON_ENGINES/RENDER_ENGINE — real
  render code, no gimmick, CPU-only and no-emoji rationale
  cross-checked against the binary's own gpu_usage: not_applicable
  probe), the CI path filters (`src` + `test` + `scripts` trees in
  ci.yml — the 5553174 test-only gap is closed), bump-rust-to.sh
  --check 1.98.1 + the permission guard + the Unix-only notes, the
  NIGHT-docs-2 freshness tools (docs-audit/stale-hunt re-run), and
  the NIGHT-hunt-35 script fleet (gate-keepers 18/18 plus runtime
  smokes of the style-sweep harness, dragon-history, and the audit
  tools).
- Security sweep beyond the archived audits: every unsafe-mentioning
  file inspected (the post-refactor sites — terminal_tty fcntl/write
  FFI, config_io fstat, watchdog isatty, fork_guard, posix_time —
  all sound with SAFETY comments; hosts.rs and clock/mod.rs are
  doc-comment false positives); 9 spawn sites all explicit argv with
  zero shell interpolation anywhere; zero production env writes;
  secrets scan clean; no pull_request_target; the 24h time-scale
  ceiling re-verified in the parser error contract.
- Hidden-bug sweep: a purpose-built production panic-surface scanner
  (test modules stripped, compile-time const-contracts excluded)
  found 36 runtime sites — every one reviewed, all local-invariant
  expects guarded by preceding checks, 0 unsound. The
  transposition-family index audit (the hunt-36 bug class):
  physarum trail_field column-major at every access site, flux_field
  row-major with clamped neighbors and degenerate guards (w,h >= 3),
  the rain_post/phosphor row-major-to-column-major round trip is
  correct with explicit bounds guards. Division sites guarded; zero
  lock-unwraps; degenerate-dimension guards verified.
- Dynamic evidence: release-binary probes (--version/--doctor/2s
  headless bench: 86K fps, 5.10 MiB RSS, -0.42 percent drift), and
  the thirteen-style stuck-cell sweep re-run — 12 styles at zero
  frozen cells; neural's 3 scattered cells classified BENIGN by a
  new, stronger method (force-repaint frame-state membership probe:
  toggle HUD on/off fires force_draw_everything twice; zero cells
  erased, 3 persisted as dormant neurons, 6 changed with brightness
  actively advancing — live static structure per network.rs, not
  orphans).
- Fixes this round: aur.yml release-tag interpolation moved from the
  run: block to env indirection (trusted-only trigger surface, so
  defense-in-depth; the inline pattern was the only run-block
  interpolation left in any workflow); retroactive UNLOCK entries in
  cosmic_dragon_engine/KEY.md for THREE post-lock commits that
  touched the locked engine without one — c523de9 (NIGHT-hunt-36),
  fbc73cd (NIGHT-termux-hang), 0a1df6a (neural force-fires), found
  via dragon-history.sh --since-lock (the c1c7779 failure mode,
  repaired per the documented remedy); the night_hunt36 bench-lab directory fixed
  from 775 to 755 (local-only, git-invisible); THREE_DRAGON_ENGINES
  retitled from the stale v50 to the v100 LTS state; the
  FUTURE_BACKLOG staleness registry's own dispositions corrected
  where files had moved to docs/archive/ but were marked "deleted or
  never created" (SIMD_FEASIBILITY, STABILITY_AUDIT,
  LTS_AUDIT_CONFIG_LIVE_RELOAD, IPC_RESEARCH).
- Gates: full suite 2879 passed / 0 failed / 2 ignored (59 s), cargo
  fmt clean, clippy -D warnings clean, gate-keepers 18/18.
  Benchmarks: not run — no runtime code changed (workflow yaml,
  lock-ledger markdown, docs), the A/B contract does not apply.

### docs: NIGHT-docs-audit round — stale-data purge in live docs + simplified --docs

- Owner task 2026-09-12: "cosmostrix audit to avoid stale data and
  simplify --docs".
- `--docs` rewritten (198 -> 163 lines): every constant re-verified
  against source — the parallax brightness/density/decay table and
  PHOSPHOR_DECAY_RATE were stale (Deep-Focus-era values), the
  density-noise symbol and DropletSpawner ref were stale, PARALLAX
  constants were pointed at the wrong file, and the 14-line chroma
  phase-history block duplicated RULES.md (cut per single-source-of-
  truth). The missing Crystal Dragon section was added so --docs
  finally describes all THREE engines; the intro now matches
  docs/THREE_DRAGON_ENGINES.md. 4 new pinning tests
  (docs_report_mentions_all_three_dragon_engines,
  docs_report_parallax_and_phosphor_numbers_match_source,
  docs_report_has_no_stale_symbols_or_paths) keep the numbers and
  symbols from drifting again.
- Live-doc broken refs fixed per the FUTURE_BACKLOG fix strategy
  (historical records untouched by policy): MAINTENANCE/PHILOSOPHY/
  SECURITY_AUDIT/TERMINAL_LIFECYCLE_MATRIX/ENDURANCE/VISUAL_MODE_AUDIT
  archive path re-points, BENCH_LABS + COMPETITOR_COMPARISON path
  fixes, LIVE_RELOAD_BEHAVIOR test paths re-pointed to the mirrored
  test/ tree, SECURITY_AUDIT unsafe-site paths re-pointed to the
  post-refactor locations (clock/posix_time.rs localtime_r,
  central_control_power_dragon/reclaim_state.rs madvise + callers).
- Gates: fmt/clippy/gate-keepers 17/17 clean, full suite 2870/0;
  check-all hit the 2-minute local cap at the cargo-test stage after
  the recompile (per the owner timeout rule — CI owns the full run;
  all sub-checks verified individually).

### docs: 3-dragon lock round (hunter-34 re-lock) + the simple engine-history method

- Owner request 2026-09-12: lock the three dragon engines after the
  NIGHT-hunter-34 commit and provide a simple method to show the
  commit history of the dragon engine folders via git log.
- Lock round signed at tree `1007714`: cosmic re-locked after the
  shadow-honesty unlock (terminal files — full UNLOCK entries in
  `cosmic_dragon_engine/KEY.md` + `RULES.md`); chroma lock intact
  with a retroactive depthtest-3 unlock entry (colors-custom
  validation hardening, same-commit entry was missed — the c1c7779
  failure mode); crystal lock intact (zero engine commits since
  S-night-R8).
- New: `scripts/dragon-history.sh` — the simple method: full engine
  history, `--since-lock` audit trail (engine commits after LOCK_AT
  each need an UNLOCK entry), `--per-engine` summary, and arbitrary
  commit ranges. The raw one-liner is documented in
  `docs/THREE_DRAGON_ENGINES.md` together with the lock status
  table.
- Docs-only + script change: no engine production code touched, no
  benchmark (docs-only rule); gate-keepers 17/17.

### fix: NIGHT-hunter-34 — color-bg default-background residue family, shadow honesty for the terminal diff renderer

- Owner report (2026-09-12, four reproductions on v100.0.0-beta.1):
  with `color-bg = "default-background"` the screen kept physical
  residue through every semantic event — (1) intro-rain glyphs stuck
  after the logo cinematic and 'r' restart not cleaning them,
  (2) old-scene glyphs/charset/colors stuck after 'x'/'X' scene
  switches and live-reload scene edits, (3) black cells stuck behind
  the moving rain after live-reloading `color-bg` black →
  default-background, (4) the same with a colors-custom bg. Every
  scenario was clean under `color-bg = "black"`.
- Root cause: `LastFrame::reuse_or_new` resets the renderer's shadow
  buffer to `Cell::blank_with_bg(None)` whenever the shadow is
  discarded (semantic_gen mismatch, resize, first draw). The HUNT-27
  cell-skip in the full-redraw path then treats "frame blank ==
  shadow blank" as "nothing to emit" — while the physical screen
  still holds the old content. `color-bg = "black"` masked the whole
  family: the frame blank carries `bg = Some(black)`, differs from
  the reset cell's `bg = None`, and everything gets repainted by
  accident. Under default-background the frame blank is also `None`,
  the skip fires, and the residue survives forever.
- Fix: the shadow now carries `force_full_emit: bool` — every
  constructor path arms it (the physical screen state is UNKNOWN
  after a reset), `Terminal::draw` re-reads it AFTER the potential
  shadow reset and emits EVERY cell once (blank cells included, each
  with its own fg/bg SGR), then clears it. The HUNT-27 idle-resync
  zero-emit optimization (force_repaint with a preserved shadow) is
  untouched.
- Two sibling bugs in the same "lying shadow" family, hunted beyond
  the owner's list (both xterm.js-host paths): (a) the Tier 2
  backpressure-suppressed flush dropped frame bytes AFTER the shadow
  was updated — the old comment claimed "a brief stutter rather than
  a permanent desync", but it was a permanent desync of exactly the
  dropped frame's cells; (b) the RIS reset (xterm.js OOM mitigation)
  wiped the physical screen while the shadow still described the
  pre-RIS content. Both now arm the unknown flag.
- New: `scripts/night_cbg34_e2e.py` — PTY + mini-terminal-emulator
  E2E reproducing all four owner scenarios (plus the 'r' restart
  variant); baseline measured 39/195/141/2554/2352 stuck cells, fixed
  binary measures 0/0/≤2/0/0 (≤2 = single transient phosphor cell,
  threshold 5). 4 new unit tests pin the shadow-side flag contract
  (`test/engine/cosmic_dragon_engine/terminal/cbg34_tests.rs`).
- A/B 10 s benches (release, cinematic + monolith, 2 runs each):
  fps/entropy/gini/dirty-cells all within ±0.4 % — machine noise;
  the steady-state frame path is byte-identical plus one false-bool
  check. Report: `benchmark/bench-labs/night_cbg34/AB_REPORT.md`.
- Docs: `docs/RENDER_ENGINE.md` § 2.8 (shadow honesty contract) +
  the LastFrame data-structure tree updated.

### docs: RUSTSEC-2024-0384 danger evaluation — the `instant` unmaintained advisory is informational, accepted, and tracked

- Owner question (2026-09-12): "is this cargo audit warning
  dangerous?" Verdict: no. The advisory class is `unmaintained`, not
  a CVE — no exploit path, no unsafe surface, no direct usage in
  `src/`. On every native target cosmostrix builds, `instant` is a
  zero-cost passthrough to `std::time::Instant`.
- Dependency chain documented end-to-end: cosmostrix → notify v7.0.0
  (config live-reload watcher) → notify-types v1.0.1 → instant
  0.1.13. The `instant` module under `src/msg_fill_style/` is an
  unrelated internal text-reveal style that shares the name —
  explicitly disambiguated so future audits do not chase a ghost.
- Fix path: upstream only (notify-types migrating to `web-time`).
  The `deny.toml` suppression (since v50.0.0-beta.7) stays, and the
  `cargo audit` warning remains expected output until then; version
  bumps remain an owner decision per project rules.
- Full evaluation: `docs/DEPENDENCY_AUDIT.md` § RUSTSEC-2024-0384.
- Docs-only change: no source touched, no benchmark (docs-only
  rule).

### test: NIGHT-depthtest-4 + hunt-33 — the v100 LTS quick regression matrix, 25 automated probes over the historical A1-J1 bug list

- Owner brief: the historical bug list (broken pipe, CLI typo tips,
  crystal-dragon-secs, config/CLI/ambient precedence, scene-custom
  and colors-custom validation, final runtime state completeness,
  symbol-only output, charset-custom edge cases, color-tune lock,
  scene-custom block simplification) was fixed across v50-v100;
  re-verify it quickly before the stable LTS cut instead of an
  evening of manual terminal babysitting.
- New: `scripts/depthtest4_regression_e2e.py` — one fast probe per
  bug, mapped by id (A1-J1). Phase A asserts validation-surface
  contracts (exit code 2 + exact stderr messages: unknown color,
  BUILT-IN hints, incomplete-block missing-dimension lists, legacy
  key removal hints, wide-char set errors, ambient scene-name
  validation, human-duration secs parsing, custom block listing).
  Phase B drives the real binary through a PTY (intro disabled so
  the watcher spawns immediately) and asserts the runtime contracts:
  cadence 3.0s dump, live-reload bold/fps edits surfacing as
  was-annotations in the final runtime state, the v80 masterclass
  temporal precedence (present config key wins over the CLI lock,
  scene switch under a CLI color lock), the color-tune contract,
  custom-scene loading with block-owned fps, ambient startup
  deferral trace, and the symbol-only warning prefix with zero icon
  glyphs in the stream. Phase C verifies broken-pipe graceful exits
  (doctor/version piped into head). Full run: 25/25 probes PASS
  against v100.0.0-beta.1 in ~41 seconds.
- hunt-33 code audit: every fix site re-verified in source (master
  snapshot diffing in `interactive/final_state.rs`, EPIPE-safe
  `println_safe!` in `output/mod.rs`, the `SCENE_CUSTOM_FIELDS`
  allowlist with the seven required dimensions, the temporal
  precedence engine in `config/live_config/mod.rs`, the
  `validate_field_value_with_cfg` custom-reference validators, and
  the `notify` hybrid watcher with its 750ms content-hash
  heartbeat).
- New finding (low severity, documented): a config edit landing
  inside the cinematic intro window is absorbed by the watcher's
  initial snapshot — the edit is deferred until the next change.
  KNOWN_ISSUES.md gained the section with the workaround
  (`intro = "none"` for scripted start-then-edit sequences).
- Note for the manual checklist: the I1 expectation in the owner's
  historical matrix ("CLI wins permanently") predates the v80
  masterclass temporal contract — current behavior (tested here and
  in `tests_cli_fallback.rs`) is "present config key wins, commented
  out falls back to the CLI lock". Flagged for the owner to confirm
  the contract stands for LTS.
- No source touched, no benchmark (test/docs-only rule).

### docs: NIGHT-docs-2 — the LTS docs completeness audit (source code = truth) and the stale-reference cleanup

- Owner brief: all documents complete before the stable LTS
  release, source code = truth. 101 non-archive .md files audited
  against the source, the live `--help`, and the live `--list-*`
  discovery commands (six checks: stale file paths, config-key
  coverage, live counts, TODO markers, version drift, docs index
  coverage). Audit record:
  `docs/audits/DOCS_LTS_COMPLETENESS_2026-09-11.md`.
- Verdict: config keys 0/41 undocumented; all 55 --help flags
  documented; scene/theme/charset/msg-fill-style counts match the
  live discovery output; zero open TODO markers; zero current
  version mislabeling.
- Fixed: 15 broken cross-references in 7 live docs that pointed at
  files moved to docs/archive/ (CENTRAL_CONTROL_RAINS_USAGE,
  MAINTENANCE, PHILOSOPHY, SECURITY_AUDIT, TERMINAL_LIFECYCLE_MATRIX,
  VISUAL_IDENTITY, LIVE_RELOAD_BEHAVIOR) — all links now resolve at
  their archive location.
- Fixed: RELEASE_GUARD.md Gates 2/4 and the future-release pattern
  taught removed scripts (rc-smoke.sh, release-benchmark-report.sh)
  as live gates — now the current tooling (build.sh check-all,
  gate-keepers, verify-release-build) and the manual 5-run
  benchmark loop; HIST_BENCH.md's removed-script how-tos rewritten
  as historical records.
- Fixed: docs/README.md index gaps — eight live documents were not
  indexed (CENTRAL_CONTROL_POWER_DRAGON, CLI_SUGGESTION_SYSTEM,
  CONFIG_LIVE_RELOAD_DISCLAIMER, DEPENDENCY_AUDIT, SECURITY_AUDIT,
  USAGE_PIPE_REDIRECT, VISUAL_IDENTITY, FUTURE_BACKLOG); every
  docs/*.md file is now indexed (completeness check: zero gaps).
- Verified already-correct: ENDURANCE.md (self-declared historical
  record with a dated removal note), FUTURE_BACKLOG.md (its
  references are the migration table itself), CHANGELOG/research/KEY
  historical records, and the illustrative path examples in
  src/RULES.md and RENDER_ENGINE.md.
- Docs-only change: no source touched, full suite 2863/2863, no
  benchmark (docs-only rule).

### fix: NIGHT-depthtest-3 — oversized custom-block names: silent collector skip promoted to a hard validation error, plus the line-1-to-end config.toml depth stress e2e

- Owner repro (2026-09-11): a complete `[scene-custom.<65+-char
  name>]` block passed `--testconf` (expected: error at the 64-char
  limit) and never appeared in `--list-scenes`, while 4-char names
  listed fine. Root cause: all three custom-block collectors
  (scene-custom, colors-custom, charset-custom) cap names at 64
  chars as a SILENT skip — the dropped block was invisible to the
  completeness/load validators (they iterate the collected map), to
  the list printers, and to the runtime scene lookup, while the
  raw-key reference scan in field_validation blessed the same key.
  Split verdict: `--testconf` PASS vs startup fatal "unknown scene"
  for a `scene = <oversized>` reference, zero listing signal. The
  hunt-24 round had documented this exact edge as out-of-scope
  known behavior; the owner's repro promoted it to a defect.
- Fix (uniform-rejection contract, every surface in lockstep):
  raw-key pre-scans front-load the three block-level validators —
  `scene_custom::validate_scene_custom_name_len` (inside
  `validate_scene_custom_completeness`, extracted to
  name_len.rs for the 800-LOC cap),
  `colors_custom::validate_colors_custom_name_len` (inside
  `validate_colors_custom_blocks`), and the new
  `charset_custom::validate_charset_custom_name_len` wired into
  both shared entry points. `--testconf` exit 2, startup exit 2,
  live-reload watcher reject. Exactly 64 chars stays legal
  (boundary pinned); the collectors and their runtime caps are
  unchanged — the gate sits IN FRONT of the skip.
- CLI dead-ends now name the limit instead of a generic "unknown":
  `--scene-custom`, `--scene`, `--show-scene`, `--colors-custom`
  and the `--charset` built-in path all report the char count and
  the 64-char limit when the name can never match a block.
- `--list-scenes` / `--list-colors` / `--list-charsets` append a
  visible `hidden:` warning line for collector-dropped oversized
  names; the CUSTOM sections print even when every defined block is
  hidden (the owner's exact case). The printers stay non-strict by
  design.
- New flagship e2e harness `scripts/depthtest3_config_e2e.py`
  (44/44 PASS on the real binary via PTY): phase A soaks a config
  with EVERY user key from line 1 (`scene = cinematic`) to the end
  and asserts every applied value from the verbose startup dump;
  phase B pins the name-length contract matrix (3 namespaces x 3
  surfaces + boundary + CLI + listing visibility); phase C proves
  live-reload depth key-by-key (fps/density/speed, color/charset/
  msg-fill-style/glitch, message/scene) using the verbose FINAL
  runtime state as the post-edit oracle, with hue classification
  and a half-width-katakana census on the ANSI stream; phase D
  proves the watcher rejects mid-run invalid and oversized-name
  edits (exit 2). Methodology notes pinned in the research doc
  (half-width katakana range, TOML section scoping, ambient key
  form).
- 17 new unit tests (owner repro shape, boundary, multi-block
  count, CLI dead-end, strict lockstep x3, list helpers x2,
  collector-vs-gate separation); full suite 2863/2863; clippy
  --all-targets --all-features -D warnings clean; build.sh check
  exit 0; gate-keepers 16/16.
- A/B 10 s benchmark (same-pipeline worktree methodology, scenes
  cinematic + sorgonemous_intrascals): FLAT — dirty cells ±0.22%,
  frame entropy ±0.11%, density gini ±0.27% (contract 1%); fps
  within the same-commit rebuild noise band. Evidence:
  `benchmark/bench-labs/depthtest3_ab/`. Research detail:
  `docs/research/NIGHT_DEPTHTEST_3_NAME_LEN_CONFIG_E2E.md`.

### fix: S-night-R4 — terminal escape injection closed in diagnostic sinks

- LTS final audit (S-night-R1 to R8, combined pass). One real
  defect found: a config value carrying a raw ESC byte reached the
  terminal verbatim through the testconf and validation error echo
  (proved with a scene-custom `rain = "glyph<ESC>[2Jx"` probe). On
  the shared-config threat model that is terminal command
  injection — OSC 52 clipboard writes, screen clears and DSR reply
  spam become reachable from a config file the victim was told to
  download.
- Fix: new `src/output/escape_ctrl.rs` renders C0, DEL and C1
  control characters as visible `\u00XX` literals (newline passes
  through as the line separator). Wired at five cold-path sinks:
  the labeled error/warning renderer (covers all labeled call
  sites plus the `die_input` family), the suggestion line helper,
  the three verbose emitters, the live-reload fatal error echo and
  the post-exit debug trace drain. Clean input borrows unchanged —
  every normal diagnostic line renders byte-identical to before.
- Coverage already in place (verified, no changes needed): message
  overlay sanitization on all three intake paths, charset-custom
  control-char rejection, custom block name grammar lock at parse,
  safepath config directory restriction, 1 MiB TOCTOU-safe config
  read cap, 24 h time-scale ceiling.
- Tests: 5 new escape_ctrl unit tests, 2 render_labeled_block
  regression locks; full suite 2846 passed / 0 failed / 2 ignored.
  Also restores the SPDX header on
  `scripts/nh32_crown_blink_audit.py`.

### docs: S-night R1-R8 LTS final audit — verdicts and dragon locks

- R1 stability: 2846 tests green, all 20 scenes clean, extreme
  geometries clean, adversarial CLI values rejected at parse,
  signals follow the documented contracts.
- R2 hygiene: already clean — stale refs are intentional history,
  duplicates are mirrored test fixtures, dead_code sites are
  documented deprecations. No over-engineering applied.
- R3 optimization: at peak (0.002 allocs/frame, stability
  excellent, engines bit-stable locked). Skipped per owner rule.
- R5 LTS: config delete/recreate/empty lifecycle all take the
  documented error path; atomic writes; watcher termination and
  mutex poisoning handled; panic hook bulletproof.
- R6 chroma integration: 350 chroma + 184 lock tests green;
  `--doctor` discloses the chroma_dragon pipeline on truecolor
  terminals.
- R7 visual impact: re-verified at peak, zero code changes, chroma
  KEY.md lock entry added (S-night-R7).
- R8 three-dragon harmony: dynamic 10 s PTY probe shows all three
  engines live together (9,381 distinct 24-bit colors, zero
  256-color fallback, clean exit and restore); lock entries added
  to all three dragon KEY.md files (S-night-R8).
- A/B 10 s (same-pipeline builds): dirty cells, frame entropy and
  density gini all within 0.13 percent — no visual or performance
  regression. Audit detail:
  `docs/audits/LTS_FINAL_AUDIT_2026-09-11.md`.

### fix: NIGHT-hunt-32 — the black hole crown blink root-caused and the hunt-31 genesis revert

- Owner report (the hunt-31 follow-up): the three disks above the
  black hole — the halo triple crown — intermittently glitch and
  blink for seconds to minutes. NOT the startup intro: the genesis
  visual is intended and stays.
- Root cause (proven three ways): the stuck-cell sweep
  (`phosphor_anomaly.rs`) is a droplet-family mechanism whose stuck
  signature — visible glyph, zero phosphor energy, no droplet
  coverage — describes live cells on every structured style.
  `droplets` is empty by contract for the thirteen non-droplet
  styles, the NIGHT-hunter-29 ownership rule zeroes phosphor on
  every drawn cell, and `Frame::set`'s equality skip keeps
  persistently-drawn cells (settled crown riders, the stable
  annulus rungs) out of the dirty list so phosphor decay Pass 1
  never re-arms their energy. Every 600 frames the sweep then
  force-cleared up to 256 live cells row-major from the top of the
  screen — the budget landing squarely on the crowns, the densest
  structure above the ball annulus. Evidence: unit red-green (the
  sweep booked exactly (256, 1) on a steady-state black hole
  pipeline; (0, 0) after the fix), an instrumented real binary
  (msg-mode=false, the owner's clean read: "sweep fired:
  stuck_count=256" at frame 600, repeating every ~10 s), and a
  PTY byte-stream audit (blank-count spikes gone). The default
  message banner gates the sweep off entirely — which is why the
  blink reads as intermittent: it only exists in msg-mode=false
  runs, and pressure episodes (the phosphor pass's skip window)
  widen it from scattered cells to the full 256-cell budget.
- Fix: the sweep is now droplet-family-only
  (`rain_style.is_droplet_family()`) — the same division the
  phosphor pass already uses for trail protection. The thirteen
  structured styles own their vacated cells through the
  monolith-style diff cleanup contract, so the sweep adds nothing
  for them and can no longer eat their live cells. The glyph
  family keeps the full NIGHT-hunter-17 stuck-cell contract
  (planted-cell contrast test).
- Hunt-31 revert (the owner's correction): the intro-handover
  fast-forward (`skip_formation_dwell`,
  `advance_birth_for_intro_handover`, the `run_intro_sequence`
  hook) is reverted whole — the startup keeps its genesis visual:
  the full stellar-collapse formation (seed dot, collapse cross,
  horizon bloom) plays after the intro handover exactly as the
  formation contracts pin. The hunt-31 "flash" was the intended
  cinematic, not a bug.
- 6 new tests (tests_stuck_cells_hunt32.rs): the planted-live-cell
  immunity (black hole + monolith), the glyph contrast contract,
  the steady-state pipeline crossing the 600-frame boundary, the
  pressure-window worst case (colored harness — `color_for_level`
  returns fg None under Mono, which would mask the sweep's
  candidacy), and the fourteen-style audit table (the owner's
  "audit the other rain types" ask — the bug wore all thirteen
  structured styles, monolith through neural). 4 hunt-31 dwell-skip
  tests removed with the revert. Suite 2841 green, clippy
  -D warnings clean, fmt clean, gate-keepers 16/16, build.sh
  check-all pass.

### test: NIGHT-depthtest-2 e2e — the priority and duplicate-name contracts proven on the real binary

- The depth test round the owner called for: CLI & config.toml
  focus, duplicate names for charset/colors/scene custom blocks
  end to end, and the priority contract pinned — "cli is wins on
  startup, config key/shortkey is wins on runtime".
- New harness `scripts/depthtest2_cli_config_e2e.py` (PTY, real
  release binary, no in-process shortcuts): the ANSI stream's
  24-bit fg colors are the source of truth. A hue classifier
  averages BRIGHT cells (R+G+B > 250, the head-level stops) per
  time window and reads channel RATIOS — the shading ladder
  blends between stops, so ratios carry the family signature
  (red G ~ B, gold G >> B, blue B > G > R).
- Four scenarios, fourteen expectations, all green on the pinned
  toolchain: (1) CLI-wins-startup — config color=blue + CLI -c
  red renders RED; (2) config-wins-runtime — a mid-run edit to
  color=gold renders GOLD over the locked CLI red (the
  S-master-LOGIC-3 contract); (3) shortkey-wins-runtime — 'C'
  at t=3 s cycles the scheme instantly (blue -> warm family;
  'C' chosen over 'c' because blue's forward neighbor cyan
  shares the blue channel signature); (4) duplicate names —
  duplicate [charset-custom.x] / [colors-custom.x] /
  [scene-custom.x] headers rejected on all three surfaces
  (--testconf exit 2, startup exit 2, live-reload watcher
  reject + exit 2, the lockstep contract from the previous
  NIGHT-depthtest-2 round).
- Verification of the previous round's in-crate coverage held
  (duplicates: parse + watcher + testconf + startup; priority:
  tests_cli_priority / tests_cli_fallback / config_apply_tests)
  — this round closes the gap those could not: the real binary,
  the real watcher thread, the real inotify path, the real key
  handling, all in one session.
- ruff-clean, 755, standard e2e harness conventions (usage
  docstring, env knobs, exit codes: 0 = all met, 1 = regressed).

### fix: NIGHT-hunt-31-supermassive — the rain-label validation gap, the intro-handover flash, and five stale mfs surfaces

- Owner hunt 1 (the silent typo): `rain = "glyphj"` in a
  `[scene-custom.<name>]` block used to fall through the strict
  validator's catch-all arm — silently passing `--testconf`, startup,
  and the live-reload watcher, then falling back to Glyph at runtime
  (a soft startup warning at best). The `rain` arm now validates
  against the same canonical labels `RainStyle::from_label` accepts
  (case-insensitive, aliases included): `--testconf` errors, startup
  exits 2, the watcher rejects + exits — the uniform-rejection
  contract completed for every scene-custom field. 4 unit tests
  (typo, full label set, case/alias parity, empty value).
- Owner hunt 2 (the flash): the black hole's startup read as a fast
  glitch/flash — the intro handover's one-frame full-screen wipe
  followed by the stellar-collapse dwell (1.9 s of at most five dim
  cells). PTY-audited empirically: the measured dead window was
  1.62 s (237 consecutive empty frames at 120x40) against <= 0.28 s
  for every one of the other thirteen styles (matrix, monolith,
  vortex, flux, lorenz, dragon, physarum, aeolian, solar_flare,
  dna_helix, murmuration, quasar, neural — all audited, all in the
  natural first-spawn band). Fix: the intro handover fast-forwards
  the formation clock past the invisible seed/collapse dwell
  (`skip_formation_dwell`, monotone, black-hole-only) so the handover
  lands on the horizon bloom — continuous content from the first
  post-intro frame. Scene entries (x/X), 'r' restarts, and
  intro-less launches keep the full sequence (their documented
  contracts). Post-fix capture: zero empty runs, bloom content on
  the frame after the wipe. 4 unit tests (landing, monotonicity,
  steady-state reach, non-black-hole no-op).
- Owner hunt 3 (stale docs): the mfs (msg-fill-style) list was stale
  on five surfaces — the count said ten while the registry ships
  eleven (radar, the first spatial reveal style, landed without the
  lists catching up). Fixed: README feature bullet, README CLI-help
  excerpt, the live `--help` style-count line ("Ten" -> "Eleven"),
  the message_draw.rs renderer doc, and the cli/app.rs field doc
  (which also still said the default was Typewriter — the actual
  default is engrave since v80.0.0-beta.2).
- Hunt beyond the owner's finds: the stale default in the app.rs
  doc comment (Typewriter, pre-beta.2), the second stale README
  mfs list (the CLI-help excerpt), and the full fourteen-style
  dead-window audit table that isolated the black hole as the sole
  outlier (the owner suspected the flash general; it was one style).
- Stresstest (end-to-end, real binary): 17-scenario `--testconf`
  matrix (typo, unknown label, over/under-limit numerics,
  non-numeric, unknown key, unknown block field, incomplete block,
  unknown scene, case/alias acceptance, mfs radar accept + typo
  reject, duplicate key) — all expectations met; live-reload PTY
  runs verified both directions (a mid-run typo edit rejects +
  exits 2 with the post-restore error per the documented watcher
  contract; a valid mid-run edit applies and the session runs to
  its full duration, clean exit 0).
- Behavior change note: a config that previously ran with a typo'd
  rain label (silently as Glyph) now fails validation on startup —
  strict by owner mandate ("should strict/error"), consistent with
  the v14 silent-PASS-is-a-bug doctrine. The visual contract of the
  black hole startup changes only in the intro-handover window (the
  seed dot and collapse flare no longer play after the logo/cosmic
  intro; the bloom and accretion are unchanged, and the full
  sequence still plays on scene entry, restart, and intro-less
  launch).

### docs: NIGHT-docs-1 — API stability contract re-pinned to the current major: v100.0.0 stable, v101.0.0 breaking window

- Owner directive: the stability contract must state the current
  epoch — the frozen surface is guaranteed from **v100.0.0**, and
  the next breaking window is **v101.0.0**. Both contract
  statements (README "API Stability" and MAINTENANCE §7) still
  anchored the promise to v50.0.0 with stale example versions
  (v80.0.0-beta.1.0, v50.1.0) that no longer match the versioning
  reality of the v100 line.
- README: baseline re-pinned to v100.0.0; the breaking-change
  sentence now names v101.0.0 as the next window and v100.1.0 as
  the minor-bump example; the MAINTENANCE cross-reference fixed
  from §6 (stale — the promise moved to §7 when the Dormant Mode
  Contract split out) to §7.
- MAINTENANCE §7: same re-pin, with one sentence of historical
  continuity (the freeze originally took effect at v50.0.0 and
  carries forward through every major since) so the contract
  stays honest about its own history.
- Hunt beyond the owner's find: the stale "§6" cross-reference in
  README (a second stale datum in the same paragraph the owner
  flagged), and a sweep confirming no other doc anchors the
  contract to v50.0.0 or the v80 examples. Historical test-run
  records (ENDURANCE, TERMINAL_LIFECYCLE_MATRIX) reference actual
  old build version strings at dated runs — point-in-time facts,
  left untouched.
- Docs-only change: no code touched, so no A/B benchmark (the
  gatekeeper's own rule — do not burn runtime on doc deltas).

### feature: NIGHT-research-26 — the glyph in-hue self-bloom cap round

- Tier three of the NIGHT-research-13 masterclass audit (the
  pin-and-finish round), round three, closing the tier and the
  audit — the audit's highest-scored scene (8.0): the glyph head
  was already the soft kind by construction (the palette's last
  stop, the self-bloom deliberately in-hue — "green to brighter
  green, not white"), with two gaps: the front-layer self-bloom
  (0.234 x 1.20) clamped sub-dominant channels onto their
  saturated sibling (the NeonGreen head stop (195, 255, 205) washed
  to (250, 255, 255) — exactly the white edge the themes' own
  "head stays tinted" principle forbids), and nothing pinned the
  composition.
- The cap: boost_rgb (both the chroma and legacy paths,
  bit-identical) renormalizes its scale against the source's own
  max channel — `scale = min(1 + factor, 255 / max(r, g, b))`.
  The channel ratios hold by construction (in-hue forever); a head
  already at the display edge composes to identity (it has nowhere
  in-hue to go); grey sources stay bit-identical to the retired
  equation (the renormalized product lands at the same clamp); the
  droplet draw-site comment re-pinned to the new equation.
- The tree the audit called for: tests_glyph (the only major style
  without one) — 6 contracts: scene resolution + family
  classification, the in-hue cap (the audit's exact wash case, the
  exact band reads, the grey parity, a 512-step tinted sweep with
  cross-product ratio survival and sub-dominant saturation
  checks), the chroma/legacy bit parity, the cinematic layer
  distribution + length/tail clamps ([0.35, 0.30, 0.35], the
  4-200 band, the 45-percent front tail), the sparse warm-start
  pool lifecycle (seed count, free-list exactness, column
  accounting, top-biased heads), and the entry-ramp fill. Full
  suite 2831 green, fmt and clippy -D warnings clean, gate-keepers
  12/12.
- A/B (10 s, 120x40, wet IO, dev profile, two-run
  discard-warmup, the matrix scene as the glyph style's proxy):
  the changed path is color-only and provably skipped in the mono
  bench (mono resolves fg to None before the effects chain) — the
  same-moment A/A comparison (stashed pre-change build) reads flat:
  avg fps 643.1 vs 657.6-658.7 (within the load-sensitive scene's
  run-to-run spread), dirty cells/frame 1209.2 vs 1207.7-1212.0,
  gini within 0.005, heap retained 0 B. The formal baseline (the
  04:58 batch, 628.7 avg) carries the morning's machine-load
  offset, not a code effect. Docs updated: README scene bullet,
  CHANGELOG feature entry, BENCHMARKING.md A/B record.

### feature: NIGHT-research-25 — the flux cap pin round

- Tier three of the NIGHT-research-13 masterclass audit (the
  pin-and-finish round), round two — the audit's honest no-op: the
  flux style was already Hot-capped by construction (the speed
  ladder's three rungs read Ghost/Mid/Hot and it has no Core rung
  at all; the comet trail only ever descends below its head), the
  only scene of the thirteen needing no brightness change — but
  the cap was unpinned, so the guarantee lived in accident, not
  contract.
- The pin: step_down_level promoted to the ladder surface
  (pub(crate), documented as the trail arm with its defensive Core
  arms explained as dead code that dims, never lifts) and one new
  contract in tests_flux/core.rs — the speed ladder never composes
  above Hot at any speed, direction, or magnitude (representative
  bands, strict thresholds, terminal-velocity jets, f32 extremes,
  and a 512-step deterministic angle/magnitude sweep), with the
  trail pinned below the head at every depth. 21 -> 22 flux tree
  tests, full suite 2825 green (one black-hole infall flake on
  the first run — the documented machine-load class, clean in
  isolation, 72/72 black hole, clean full re-run), fmt and clippy
  -D warnings clean, gate-keepers 12/12.
- A/B (10 s, 120x40, wet IO, dev profile, two-run
  discard-warmup): flux avg fps 3623.3 -> 3636.4 (+0.4%, within
  the run-to-run spread), p99 0.3368 -> 0.3252 ms, median 3688.1
  -> 3689.0, avg dirty cells/frame 16.0 -> 16.1, entropy 3.70 ->
  3.71, gini 0.8937 -> 0.8931, heap retained 0 B — mechanically
  identical, the pin touches no runtime path (visibility only). Docs
  updated: README scene bullet, CHANGELOG feature entry,
  BENCHMARKING.md A/B record.

### feature: NIGHT-research-24 — the solar flare apex soft-light round

- Tier three of the NIGHT-research-13 masterclass audit (the
  pin-and-finish round), round one — the audit's apex finding: the
  corona was already flash-gated in three of four Core paths (the
  0.6 s eruption window, the ladder's fresh-flare rung, the
  footpoint landing punch), but the apex condensation glow was
  flux-gated alone — a heavily-fed loop steps Hot to Core and holds
  it for the whole cooling plateau (the 0.38/s flux decay from
  FLUX_MAX crosses the HOT bound at about 3.4 s, the audit's
  standing window).
- The saturation (loops.rs apex_step_level, the black hole's
  NIGHT-research-11 cap precedent): the apex glow lifts only the
  lower rungs and holds the ceiling at Hot — the fed apex reads
  warm for the whole plateau, never standing white; a Core base
  passes through untouched so the flash windows keep their Core.
- The whole pass-B arc-cell decision extracted into the pure
  loops::arc_cell_level (phase base + landing punch + glow, the
  quasar pure-ladder precedent), step_up_level moved from draw.rs
  to the ladder home; the footpoint punch keeps its one Core rung
  inside its flash window.
- Two new contracts in tests_solar_flare/loops.rs — the standing
  ceiling (no arc cell composes above Hot with the flash windows
  closed, at any phase, flux, or cell zone; the fed apex reads the
  warm ceiling exactly) and the flash windows (the eruption window,
  the fresh-flare rung, and the landing punch all keep their Core).
  22 -> 24 solar flare tree tests, full suite 2824 green, fmt and
  clippy -D warnings clean, gate-keepers 12/12.
- A/B (10 s, 120x40, wet IO, dev profile, two-run
  discard-warmup): solar_flare avg fps 2304.6 -> 2327.0 (confirmation
  2325.1, +0.9%), p99 0.5261 -> 0.5364 ms, median 2330.6 -> 2358.7,
  avg dirty cells/frame 116.5 -> 114.2 (-2.0% — the softer apex
  composition repaints fewer contents through the frame-equality
  fast path), entropy 5.93 -> 5.90, gini 0.5705 -> 0.5796, heap
  retained 0 B — mechanically flat, zero allocation, same pools. Docs
  updated: README scene bullet, CHANGELOG feature entry,
  BENCHMARKING.md A/B record.

### feature: NIGHT-research-23 — the lorenz soft-light round

- Tier two of the NIGHT-research-13 masterclass audit (the
  standing-Core cleanup sweep), round seven — the audit's
  cheapest fix in the entire sweep: level_for_z returned Core
  above the 38.0 hot bound, and every lobe-peak excursion lives
  there (the canonical attractor's peaks cluster near z=40), so
  both butterfly tips held a standing Core cluster — the white
  blend standing on the attractor's own hero geometry.
- One rung, Core to Hot (lorenz.rs level_for_z): the lobe-peak
  zone merges with the lobe body — the peaks read the warm Hot
  ceiling, and the attractor's depth read survives (Hot wings,
  Mid transition, Ghost saddle bridge). LORENZ_Z_HOT stays live:
  the black hole ring's locked composition (level_for_ring_z)
  reuses the shared z semantics and is untouched by the round.
- One new contract in tests_lorenz/core.rs — the z ladder never
  lands Core at any z from the saddle floor to far past the peak
  region, with the Hot/Mid/Ghost zone boundaries pinned. 10 ->
  11 lorenz tests, full suite 2822 green, fmt and clippy -D
  warnings clean, gate-keepers 12/12.
- A/B (10 s, 120x40, wet IO, dev profile, two-run
  discard-warmup): lorenz avg fps 8328.9 -> 8731.0 (+4.8%), median
  8565.7 -> 8843.9, p99 0.1526 -> 0.1467 ms, avg dirty cells/frame
  56.9 -> 53.7 (-5.6% — the peaks' softer composition changes
  fewer drawn contents through the frame-equality fast path),
  entropy 5.19 -> 5.15, gini 0.7331 -> 0.7386, heap retained 0 B
  — zero allocation, same pools, the RNG stream untouched. Docs
  updated: README scene bullet, CHANGELOG feature entry,
  BENCHMARKING.md A/B record.

### feature: NIGHT-research-22 — the neural soft-light round

- Tier two of the NIGHT-research-13 masterclass audit (the
  standing-Core cleanup sweep), round six: the audit's finding —
  pulse heads held Core for the whole 1.8 s burst window (about
  ten times the black hole's whip flash) and the output band
  stepped a standing Hot answer node to Core.
- The soft-light ruling (neural/draw.rs): the burst-lifted pulse
  heads cap at the warm Hot ceiling — the retired flaring branch
  is gone, the pulse-head ladder extracted into the pure
  `pulse_head_level` (the quasar precedent: the ladder leaves the
  inline draw pass so the composition can be pinned); the
  machine's white lives on the neurons' fire flashes (the 0.35 s
  fired-flash tau, untouched) and the genesis ramp. The output
  band's step-up stops at the warm Hot ceiling (`step_up_level`:
  a standing Hot answer node holds, a fired flash keeps its
  white); the answer read survives below the ceiling (Mid steps
  up to Hot). The burst's visible signature survives on the wires
  (the flare rung steps the idle wires up) and the surge.
- Two new contracts in tests_neural/core.rs — the pulse-head
  ladder never lands Core at any genesis cap (the ramp included,
  steady pinned at Hot), and the step-up holds at the warm
  ceiling with the fired flash keeping its white. 16 -> 18 neural
  tests, full suite 2821 green, fmt and clippy -D warnings clean,
  gate-keepers 12/12.
- A/B (10 s, 120x40, wet IO, dev profile, two-run
  discard-warmup): neural avg fps 13718.1 -> 13412.9 (-2.2%, the
  honest mechanical cost of +16% dirty cells — 26.4 -> 30.7: the
  softer ladder walks more rungs as the fired flashes decay, the
  retired Core saturation collapsed several rungs into one, so
  more decaying cells cross level boundaries per frame), median
  13421.0 -> 13529.2, p99 0.1179 -> 0.1006 ms (-14.7%), entropy
  4.42 -> 4.57, gini 0.8266 -> 0.8149 (the busier decay
  transitions spread the dirt more evenly), heap retained 0 B —
  zero allocation, same pools, RNG untouched. Docs updated:
  README scene bullet, CHANGELOG feature entry, BENCHMARKING.md
  A/B record.

### feature: NIGHT-research-21 — the murmuration soft-light round

- Tier two of the NIGHT-research-13 masterclass audit (the
  standing-Core cleanup sweep), round five: the audit's two
  institutionalized standing-Core sources — the speed band (any
  bird above the retired 21 threshold read Core, and a wheeling
  flock routinely holds that band) and the panic floor (panicked
  birds returned Core unconditionally for the 1.2 s window — a
  mass standing-Core event every startle).
- The soft-light ruling (draw.rs speed_level): both standing
  sources cap at the warm Hot ceiling — the scatter burns bright,
  never white, while it lasts; the calm bands keep their gradient
  (fast edge Hot, mid flight Mid, drift Ghost). Core survives
  only in the predator flash — the draw pass's one glyph inside
  its 0.8 s window, untouched by the round. MURM_SPEED_CORE
  (21.0) retires with the rung and its two compile-time asserts.
- The concentration ask (the audit's "halve the jitter the way
  NIGHT-research-12 halved the wobble"): MURM_JITTER_W 16.0 ->
  8.0 — the audit called the old value the largest scatter
  constant in the codebase (about 290x the black hole's converged
  wobble in comparable terms). The same two rolls per bird keep
  the organic life; the flock reads tighter and more cohesive.
- One new contract in tests_murmuration/core.rs — the speed
  ladder never lands Core on a bird (calm or panicked, from the
  drift floor to and past max), with the panic-floor ceiling and
  the calm bands pinned. The startle re-gather test re-pins
  honestly: the tighter flock puts more members inside the fixed
  14-cell panic radius, so the scatter blooms wider and the
  re-gather window doubles (240 -> 480 frames, 7.7 s of sim
  time). 27 -> 28 murmuration tests, full suite 2819 green, fmt
  and clippy -D warnings clean, gate-keepers 12/12.
- A/B (10 s, 120x40, wet IO, dev profile, two-run
  discard-warmup): murmuration avg fps 3140.6 -> 3203.8 (+2.0%),
  median 3148.8 -> 3149.6, p99 0.4061 -> 0.3778 ms (-7.0%), avg
  dirty cells/frame 117.3 -> 111.7 (-4.8% — the tighter flock
  trims wandering cells, the NR16 concentration-signature class),
  entropy 5.26 -> 5.26, gini 0.7243 -> 0.7246, heap retained 0 B
  — zero allocation, same pools, same roll count. Docs updated:
  README scene bullet, CHANGELOG feature entry, BENCHMARKING.md
  A/B record.

### feature: NIGHT-research-20 — the dna_helix soft-light round

- Tier two of the NIGHT-research-13 masterclass audit (the
  standing-Core cleanup sweep), round four — the audit's
  strongest scene (7.5) and the only one whose tree already
  pinned a brightness ladder, pinned in the wrong direction
  (charge 2.0 must equal Core). Three standing Core sites
  retired: fresh nucleotide heads read Core for the first 20% of
  every drop's lifetime (~3 s per drop), the fork wake's charge
  stamp (2.6) decayed through Core for ~1.6 s per written rung,
  and the rung front-face step-up lifted every re-synthesized Hot
  rung to Core through the whole decay band.
- Site one (drops.rs age_level): the fresh soup reads the warm
  Hot ceiling from the moment it enters the sky — the fresh and
  young bands merge at the ceiling, AGE_FRACTION_HOT retires.
- Site two (helix.rs charge_level): the fresh-write blink — a
  new DNA_CHARGE_LEVEL_BLINK bound (2.3): a freshly-written rung
  (charge at max) flashes Core for the ~0.35 s the charge sits
  above the bound, then the replication wake reads the warm Hot
  ceiling while it cools (the audit's "the fresh-write blink
  stays Core" — a moment, not the retired 1.6 s window).
  DNA_CHARGE_LEVEL_CORE (1.5) retires with its asserts at both
  definition sites.
- Site three (draw.rs step_up_level): the front-half rung glow
  stops at the warm Hot ceiling — a standing Hot rung no longer
  composes Core on its front face. The 3D depth read survives
  (Mid bases step up, the back face steps down), and a rung
  inside its fresh-write blink keeps the flash across the whole
  face.
- Re-pinned the wrong-direction ladder contract (charge 2.0 now
  reads Hot, the blink band pinned at 2.3+ and max); three new
  contracts — the fresh-write blink expires under law-3 decay,
  the soup ladder never lands Core at any age fraction, and the
  rung front-face composition (Hot holds, Mid steps up, back
  steps down, the blink keeps its flash). 42 -> 45 dna_helix
  tests, full suite 2818 green, fmt and clippy -D warnings clean,
  gate-keepers 12/12.
- A/B (10 s, 120x40, wet IO, dev profile, two-run
  discard-warmup): dna_helix avg fps 7469.1 -> 7814.4 (+4.6%),
  median 7682.5 -> 7930.0, p99 0.1774 -> 0.1735 ms, avg dirty
  cells/frame 55.6 -> 51.9 (-6.7% — the softer heads and capped
  wake change fewer drawn contents through the frame-equality
  fast path), entropy 4.55 -> 4.46, gini 0.8298 -> 0.8382, heap
  retained 0 B — zero allocation, same pools, the RNG stream
  untouched. Docs updated: README scene bullet, CHANGELOG feature
  entry, BENCHMARKING.md A/B record.

### feature: NIGHT-research-19 — the aeolian soft-light round

- Tier two of the NIGHT-research-13 masterclass audit (the
  standing-Core cleanup sweep), round three: the audit's split
  verdict — the instrument half already enforced the exact
  NIGHT-research-11 rule (knots-only-Core, in prose and in code),
  while the rain half's kinetic ladder sent drop heads to Core at
  |vy| > 3.6 with terminal velocity 4.0. Gravity (1.6/s^2 from the
  1.2 calm entry) drives every free fall past the threshold within
  ~1.5 s, so most of each drop's visible flight stood Core-white —
  the "punch flash" was a standing surface.
- The soft-light ruling (drops.rs kinetic_level): the top rung
  re-grades Core to the Hot warm ceiling — the rain half now
  matches the instrument half's own knots-only-Core policy. Core
  survives only where two packets cross (the interference knots,
  untouched by the round). AEOLIAN_SPEED_CORE (3.6, the retired
  punch threshold) retires with the rung along with its
  compile-time assert; its tuning knowledge folds into the
  AEOLIAN_SPEED_MID doc.
- The comet trails compose one rung dimmer (step_down_level from
  the Hot ceiling: Mid / Ghost / Ghost instead of Hot / Mid /
  Mid) — the whole weather half reads softer, the knots carry the
  scene's only white. Module and README prose re-pinned ("flare
  bright", never white, on the drops).
- One new contract in `tests_aeolian/core.rs` — the kinetic
  ladder never lands Core on a falling glyph (the sweep runs from
  the calm entry through the retired punch band at 3.6 to and past
  terminal, mirrored negatives included), with the Ghost/Mid/Hot
  zone boundaries pinned. 27 -> 28 aeolian tests, full suite 2815
  green, fmt and clippy -D warnings clean, gate-keepers 12/12.
- A/B (10 s, 120x40, wet IO, dev profile, two-run
  discard-warmup): aeolian avg fps 7599.5 -> 7944.2 (+4.5%,
  confirmation 7870.0), median 7430.3 -> 7721.2, p99 0.1706 ->
  0.1728 ms, avg dirty cells/frame 74.8 -> 71.0 (-5.1%, the
  re-graded heads and dimmer trails change fewer drawn contents
  through the frame-equality fast path; confirmation run 71.0),
  entropy 5.65 -> 5.63, gini 0.6285 -> 0.6312, heap retained 0 B —
  zero allocation, same pools, the RNG stream untouched. Docs
  updated: README scene bullet, CHANGELOG feature entry,
  BENCHMARKING.md A/B record.

### feature: NIGHT-research-18 — the physarum soft-light round

- Tier two of the NIGHT-research-13 masterclass audit (the
  standing-Core cleanup sweep), round two: the physarum's own hero
  structure was the offender — the vein network equilibrates above
  the 0.30 trail saturation bound, so the whole signature surface
  read standing Core (the 100% palette + 55% white blend, MONOLITH_
  CORE_WHITE_BLEND) continuously, the code's own comment documenting
  the grading at the 60 Hz reference.
- The soft-light ruling (the vortex ceiling idiom, the audit's
  exact wording "the veins read warm-gold"): `level_for_trail`
  re-grades the top rung Core to Hot — the saturated-vein zone
  merges with the sustained-vein zone, both read the Hot warm
  ceiling (the palette's bright stop, no white blend). The ladder
  now has three zones: vein (Hot), single-particle (Mid),
  exploring (Ghost) — never Core at any trail value.
- PHYSARUM_BRIGHTNESS_HOT (0.30, the retired saturation bound)
  retires with the rung — its tuning knowledge (4+-particle cells
  saturate there) folds into the PHYSARUM_BRIGHTNESS_MID and
  deposit docs; the deposit equilibrium comment re-pins from
  "Hot/Core" to "the Hot warm ceiling".
- The per-particle pace multipliers (0.85-1.15 on turn/move/
  deposit) stay: the audit's round sentence is cap-and-pin, and
  per-particle variance is the organic slime-mold semantics (the
  monolith speed-spread call, an owner decision if it changes).
- One new contract in `tests_physarum/core.rs` — the trail ladder
  never lands Core at any trail value (the sweep runs from the
  exploring floor through the retired saturation region to 50.0),
  with the vein/Mid/Ghost zone boundaries pinned. 15 -> 16 physarum
  tests, full suite 2814 green, fmt and clippy -D warnings clean,
  gate-keepers 12/12.
- A/B (10 s, 120x40, wet IO, dev profile, two-run
  discard-warmup): physarum avg fps 7945.7 -> 7968.1 (+0.3%,
  flat), median 8106.3 -> 8145.6, p99 0.1529 -> 0.1495 ms, avg
  dirty cells/frame 78.5 -> 78.4, entropy 5.45 -> 5.43, gini
  0.6890 -> 0.6923, heap retained 0 B — mechanically honest-flat,
  the same vein cells repainted at the warm stop, zero allocation.
  Docs updated: README scene bullet, CHANGELOG feature entry,
  BENCHMARKING.md A/B record.

### feature: NIGHT-research-17 — the monolith soft-light round

- Tier two of the NIGHT-research-13 masterclass audit (the
  standing-Core cleanup sweep, the owner-approved round order):
  the monolith carried the strain source the black hole's
  NIGHT-research-11 round audited and retired — every Hero
  segment's head cell read Core (the 100% palette + 55% white
  blend, MONOLITH_CORE_WHITE_BLEND) for every frame of the
  cascade's fall, with the breath/hero pulse stacking up to
  +0.20 more white on top.
- The soft-light ruling ported to the family (the dragon's
  NIGHT-research-15 entry-reveal idiom): `segment_level` takes
  the arrival-reveal flag — the Hero head rung reads Core ONLY
  inside a fresh stream's ~1.5 s arrival window
  (MONOLITH_HERO_REVEAL_SECS, stamped by activate_stream, decayed
  by advance on the same capped clock the head motion uses), and
  composes to the Hot warm ceiling for the rest of the fall.
  The Hero body fade (Hot/Hot/Mid), the Medium/Short/Micro rungs
  and the spine ladder are untouched. The per-stream speed spread
  stays: varied cascade speed is deliberate classic semantics
  (the audit's owner call — not a defect).
- The cap lands at the monolith draw site
  (`draw_segments`'s hoisted hero_revealing gate), NOT inside
  color_for_level — that function is the shared brightness
  ladder for lorenz, vortex, dragon, physarum and flux, and an
  in-function cap would have silently re-graded five scenes. No
  RNG draws were added or removed; the draw stream is unchanged.
- The deliberate re-pin dance (the audit's three wrong-direction
  sites): the depth suite's core-bloom assertions and the
  palette-ladder suite's Core-blend comment re-pin to the reveal
  contract — Core still blooms through the shared pipeline (the
  rung the arrival flash composes through), but the standing
  cascade never rides it.
- Four new contracts in `tests_monolith/core.rs` — no kind at
  any position lands Core with the reveal window closed, the
  reveal flash touches only the Hero head rung, every fresh
  stream carries the countdown, and the window expires under
  simulated time (3.2 s). 32 -> 36 monolith tests, full suite
  2813 green, fmt and clippy -D warnings clean, gate-keepers
  12/12.
- A/B (10 s, 120x40, wet IO, dev profile, two-run
  discard-warmup): monolith avg fps 6699.3 -> 6679.3 (-0.3%,
  flat), median 6660.1 -> 6721.8, p99 0.1939 -> 0.1918 ms, avg
  dirty cells/frame 107.3 -> 107.1, entropy 3.92 -> 3.92, gini
  0.8945 -> 0.8946, heap retained 0 B — mechanically
  honest-flat, the same cells repainted at a softer standing
  level, zero allocation. Docs updated: README scene bullet,
  CHANGELOG feature entry, BENCHMARKING.md A/B record.

### feature: NIGHT-research-16 — the vortex lockstep round

- Tier one of the NIGHT-research-13 masterclass audit, round three
  (after the quasar and the dragon; the audit's lowest score at
  5/10): the vortex carried the owner's exact scattered/flying-
  outward complaint class — every mote rolled its own `spin`
  (0.85-1.15) and `fall` (0.80-1.25) multipliers, so motes spawned
  together drifted apart in both angle and depth, dissolving the
  arms into a smeared field.
- The lockstep retirement (`vortex.rs`): the per-mote multipliers
  are gone (fields, rolls and applications) — every mote at the
  same radius advances at the SAME angular and radial speed (the
  black hole's NIGHT-research-11 all-lanes-consistent precedent).
  Under lockstep the motes of an annulus orbit in formation: arms
  read dense, coherent, concentrated spirals, while the K/r
  differential still provides all the shear (the inner annuli lap
  the outer ones). Two RNG draws retire with the rolls.
- The physics label fix: the law omega = K / r is a FLAT rotation
  curve (constant tangential cells/sec — the galaxy rotation-curve
  read), not "Keplerian" (Kepler's third law is r^-1.5, which the
  quasar's disk implements). The constant renames
  VORTEX_KEPLER_K -> VORTEX_ROTATION_K with the honest derivation;
  the module docs, the advance-pass docs and the bounded-omega test
  re-pin to the flat-curve language.
- The soft-light ruling: `level_for_radius` re-grades the Core rung
  to Hot — the retired rung was unreachable at the draw site only
  by pass ordering (absorption deactivates motes below
  VORTEX_CORE_R before draw), an accident a reorder would have
  broken; the standing ceiling is now by construction. Pinned: the
  ladder never lands Core at any radius, and the zone boundaries
  (Ghost rim / Mid band / Hot core zone) are pinned at the
  boundaries.
- Two new contracts in `tests_vortex/core.rs` — the lockstep pace
  (two motes placed at the same radius with different angles
  advance by identical deltas matching the closed-form shared law,
  frozen dt) and the soft warm ladder. 10 -> 12 vortex tests, full
  suite 2809 green.
- A/B (10 s, 120x40, wet IO, dev profile, two-run discard-warmup):
  vortex avg fps 7712.9 -> 7817.8, dirty cells/frame 90.0 -> 88.2
  (the tighter arcs trim off-arm wanderer cells), gini 0.6342 ->
  0.6361 (the arms concentrate), entropy 5.68 -> 5.67, heap
  retained 0 B — the signature shifts track the concentration
  intent, mechanically flat otherwise.

### feature: NIGHT-research-15 — the dragon soft-light round

- Tier one of the NIGHT-research-13 masterclass audit, round two
  (after the quasar): the dragon carried three permanent
  Core-white glyph heads — one per dragon, standing for every
  frame of their 20-second lifetimes — the most literal instance
  of the standing-strain pattern the black hole rounds retired.
- The soft-light cap ported from the black hole's
  NIGHT-research-11 ruling (`dragon/dragon_helpers.rs`):
  `level_for_segment` re-grades the head rung Core to the Hot
  warm ceiling (the palette's bright stop, no white blend) for
  the whole life of the flight; Core survives only in the
  entry-reveal flare — the ~1.5 s arrival flash while the body
  unfurls (the black hole formation-collapse precedent: the head
  burns white as the dragon arrives, then settles to warm for its
  lifetime). The serpentine body fade (Hot / Mid / Ghost thirds)
  is untouched.
- Pinned with three new contracts in `tests_dragon/core.rs` — no
  standing segment ever lands Core at any index or body length,
  the entry flare is the head's one Core moment (body stays soft,
  the empty-body guard reads Hot), and the serpentine zone
  boundaries are pinned. 13 -> 16 dragon tests, full suite 2807
  green.
- Mechanically honest-flat A/B (10 s, 120x40, wet IO, dev profile,
  two-run discard-warmup): cosmic_dragon avg fps 22678.9 ->
  22809.1, dirty cells/frame 31.7 -> 31.8, entropy 4.07 -> 4.06,
  gini 0.8706 -> 0.8709, heap retained 0 B — a level re-grade, the
  same cells repainted softer, zero allocation.

### feature: NIGHT-research-14 — the quasar soft-light round

- Tier one of the NIGHT-research-13 masterclass audit (the owner
  approved the round order): the quasar carried the single
  brightest standing cell in the codebase — the permanently
  Core-bright engine glyph at screen center (the 55% white blend
  the black hole rounds retired), plus standing Core at the disk's
  inner rung, the doppler limb's step-up, the fresh-feed charge
  step-up, and the jet launch collar.
- The soft-light cap ported whole from the black hole's
  NIGHT-research-11 ruling (`quasar/draw.rs`): the disk ladder
  composes through the new `soft_head_level` after the ignition
  cap — the standing disk never lands Core; the jet ladder caps
  BEFORE the knot's step-up so the collar reads warm while the
  knot's traveling pulse remains the beam's one transient Core
  flash; the core cell reads the soft warm ceiling standing with a
  0.85-1.0 pulse breathing (never static-flat), burning Core only
  inside the 2.0 s flare window (the event-gated flash that rides
  the glyph re-roll moment).
- The ladders extracted into pure functions (`disk_level`,
  `jet_level`, `core_cell_level`, `core_cell_factor`) and pinned:
  four new contracts in `tests_quasar/core.rs` — the cap steps
  Core down one rung, the disk composition never lands Core at any
  temperature / line-of-sight / charge state, the beam stays soft
  warm with Core only inside the knot window, and the core cell
  reads soft warm standing and flashes Core on flare. 25 quasar
  tests (was 21), full suite 2804 green.
- Mechanically honest-flat A/B (10 s, 120x40, wet IO, dev profile,
  two-run discard-warmup): avg fps 12866.7 -> 12889.1, dirty
  cells/frame 45.1 -> 45.1, entropy 3.90 -> 3.89, gini 0.8867 ->
  0.8868, heap retained 0 B, drift stable — pure level re-grades,
  zero allocation, same population.

### feature: NIGHT-research-12 — the black hole sparse lower echo, soft ball, rise hold, and arc concentration

- Owner verdict round (9.8/10, four asks decoded): the lower rings'
  particles must be fewer — only a few, "few but substantive and
  elegant", not spam; the ball's white still strains the eyes (the
  soft-light fix had not reached the ball itself); the center ring
  near the hole must read white soft all the way to the end where
  the particles rise to the top ring — not too dim; and the crown
  particles read scattered and flying outward instead of dense and
  concentrated.
- The sparse lower echo (`black_hole.rs` + `style_rain.rs`): the
  halo lane split retires the five-way even round robin for a
  crown-dominant Bresenham family accumulator — CROWN_SHARE 0.84
  split three ways by the crown round robin (0.28 per crown, denser
  than the retired 0.20) and LOWER_SHARE 0.16 split two ways by the
  alternating toggle (0.08 per mirrored arc, a 60 percent cut): the
  crowns read dense and concentrated while the lower family reads
  as a rare elegant echo. The shares sum to 1.0 by a compile-time
  contract.
- The soft ball (`ball_helpers.rs` + `black_hole.rs`): the annulus
  band ladder drops its two thin edge structures (the horizon ring
  and the rim photon line) from Core to the Hot soft warm ceiling
  over a Mid body, and the ball draw site caps the Doppler-lobed
  level through the soft-head cap — the standing surface never
  draws the Core white blend again. Core survives only in the
  formation intro's transient collapse flash and the infall's
  whip-around flash.
- The rise hold (`ring.rs`): `rise_ladder_dist` clamps the
  far-side tier-0 riders' proximity-ladder input at the lens arc's
  radius, so the center ring's rising curve — the lensing arc from
  the disk's end to the crown handoff — composes to the soft warm
  ceiling at every projected distance. The plain fade ladder used
  to dim the rise's flank between the bright crossing band and the
  bright apex (the owner's "too dim" report); the near-side arms
  keep the approved fade.
- The arc concentration (`halo.rs` + `style_rain.rs`): the halo
  family gains its own tight entry spiral (HALO_ENTRY_BOOST 0.04,
  tau 0.6 — the ring's 0.55 drift-in read as riders flying loose of
  the system) and the wobble band halves to 0.055: riders
  materialize at the arc's fringe and melt into a thin dense band,
  igniting at the warm ceiling from the first frame.
- Tests: 72 black hole (was 69); re-pinned the radial band ladder,
  the rim photon line, the halo population split and the stream
  entry read; new contracts for the rise hold, the tight entry and
  the soft ball composition. Full suite 2800 green. Mechanical A/B
  ~flat (the lane re-cut moves population between lanes without
  changing totals; the brightness changes are pure level
  re-grades).

### feature: NIGHT-research-11 — the black hole soft-light cap and the all-lanes consistency

- Owner verdict round (9.1/10, three asks): the head-white is too
  bright — it strained the eyes; the rings above and below the
  shadow read inconsistent with the center ring (speed, density,
  smoothness); and the center ring's near-ball band is too bright.
  The cinematic read the owner asked for: soft, elegant, consistent.
- The soft glyph heads (`ring.rs` + `halo.rs`): `soft_head_level`
  caps every composed glyph-head level one rung below Core, applied
  at the ring draw site after the proximity ladder and inside
  `halo_head_level`. The crowns, the tier-0 crossing band and the
  snug stacks burn the warm Hot ceiling — the full palette without
  the Core blend toward white — while the distance key, the Hot
  floors and the fade ladder keep shaping the band below the
  ceiling exactly as before. The Core white stays reserved for the
  ball's own thin photon structures (the horizon ring and the rim
  photon line) and the infall's transient whip-around flash.
- The all-lanes consistency (`halo.rs` + `style_rain.rs` +
  `black_hole.rs`): the five stream lanes unify with the center
  ring. Lockstep pace — every lane rides the ring's tier-0 mean
  motion (the per-lane Keplerian pace ladder 0.74 / 0.60 / 0.50 is
  retired). A doubled pool (`BLACK_HOLE_HALO_POOL_PER_COL` 2 — one
  rider per lane per two columns) seats each lane's
  semicircle-filtered visible population at the tier-0 main line's
  own linear cell density, proportional at every density setting.
  An even five-way round-robin tag split retires the weighted
  0.72/0.28 family split — the lower arcs carry the same population
  as the crowns. The halo lifetime pins at the ring's own 14 s
  cadence.
- The infall shimmer test hardening (`tests_black_hole/infall.rs`):
  the old end-state age heuristic could not distinguish a respawned
  early-window mote from a true survivor — the halo rework's
  RNG-stream shift exposed the fragility. The harness now tracks
  continuity per frame on the `run_frames` clock contract (an
  absorption deactivates the mote, a respawn resets its age — both
  break the chain), with a shorter 70-frame window seated inside
  the flight distribution.
- Tests: 69 black hole contracts (three re-pinned: the soft warm
  ceiling on the stream heads and the crossing band, the equal
  five-lane population split, the pool-multiplier resize; two new:
  the soft-head cap unit contract, the lockstep pace). Full suite
  2797 green.

### feature: NIGHT-research-10 — the black hole photon line, the triple crown, and the head-white ladder

- Owner feedback round (the Interstellar/NASA imagery read, four
  asks): the real photographs show a thin ring like a line inside
  the ball's disk (the photon ring hugging the shadow's edge — the
  owner asked whether it was needed; it is the trademark of the EHT
  photographs and Gargantua's render, so yes); the upper ring
  should grow from two to three stages so it reads thick like the
  center ring's three-tier stack; the lower ring should double to
  two; and the three upper rings should burn brighter, dominantly
  head-white like the rain-glyph heads — plus the center ring's
  near-ball reach reads a bit dark where the imagery burns
  brightest.
- The rim photon line (`ball_helpers.rs`): the annulus's outer band
  flips to Core — the thin bright line at the shadow's edge. The
  line width is the larger of `BLACK_HOLE_PHOTON_RIM_FRACTION`
  (0.12) of the annulus and `BLACK_HOLE_PHOTON_RIM_MIN_CELLS`
  (1.0 line-height units), so the line stays thin on every terminal
  class yet never collapses to a sub-cell sliver the raster misses
  on the smallest viewports. The Ghost fringe is retired — the Mid
  body runs sharp up to the line (the EHT edge read), and the
  Doppler lobe still breathes the line (Core bumps clamp, the
  opposite lobe steps one rung down).
- The triple crown and the doubled echo (`halo.rs` +
  `style_rain.rs`): the upper family grows to three crowns riding
  the 1.30 / 1.48 / 1.66-radius arcs (the new
  `BLACK_HOLE_HALO_TOP_ARC_FRACTION`, Keplerian pace 0.50 — the
  slowest lane of the system, three visibly distinct moving arcs),
  while the lower family doubles to two mirrored arcs (1.30 and the
  new `BLACK_HOLE_HALO_LOWER_OUTER_ARC_FRACTION` 1.48). The spawn
  split walks the upper family's 0.72 combined share (0.24 per
  crown, 0.14 per lower lane — the five weights partition the pool
  with compile-time contracts) through the same Bresenham
  accumulator, now with a three-step crown round robin
  (`halo_crown_step`) and a lower lane toggle (`halo_lower_lane`):
  every lane fills exactly at its weight on every pool fill, no
  spawn luck. The apex fits every viewport by construction (the
  ball never exceeds 0.55 of the half-height, and 1.78 times that
  stays inside the vertical budget even at the wobble extremes).
- The head-white crowns (`halo.rs` `halo_head_level`): every
  upper-family head floors at Hot (the same floor the disk's snug
  tiers carry) and its proximity-ladder distance input is pulled
  inward by the new `BLACK_HOLE_HALO_CROWN_GAIN` (0.34 outer radii
  — the lensing light-path compression that makes the photon ring
  the brightest structure in the iconic images), so all three
  crowns burn Core white across their reach; the entry-spiral
  drift-in still reads dim and ignites as the rider settles (the
  accretion read survives the gain), and the lower arcs keep the
  plain z-graded ladder — the dimmer mirrored echo.
- The white crossing band (`ring.rs` `ring_head_base`): the tier-0
  equatorial band's head base floors at Hot whenever the head rides
  inside the hot radius, so the center ring's near-ball reach lands
  Core white after the ladder's +2 bump (a Ghost-zone z base used
  to land only Mid — the dim read the owner reported); the far arms
  keep the plain z-graded base and dissolve through the fade ladder
  as before, and the snug tiers keep their stage-2.7 floor
  everywhere.
- Tests: 67 black hole contracts (re-pinned: the radial-band
  contract for the rim ladder, the halo stream split for five
  lanes, the sparser-lower contract for the round robin; new: the
  rim photon line's presence/thinness/outside-wrap plus the 80x24
  one-cell-floor survival, the crown head-white pure-ladder
  contract, the near-ball crossing-band pure-ladder contract). Full
  suite 2795 green.

### feature: NIGHT-research-9 — the black hole masterclass physics pass: disk-corotation infall, width-adaptive composition, and the see-saw window re-cut

- Owner report (three reads): the disk at stage 3 reads
  counter-rotating (captures whipping around the shadow against the
  disk's flow — the disk reads left-to-right at the start, the
  opposite later); on narrow terminals the ball reads too big (it
  should shrink so the disk reads long, with wide terminals scaling
  up into the majestic read); and the disk parked near vertical
  (the 70-110 degree window) clips against the terminal screen
  limits and reads ugly.
- Root cause of the counter-rotation: the infall's spawn drift was
  symmetric noise, so roughly half the captured glyphs carried the
  disk's angular momentum sign and half the opposite — the left-side
  captures in particular are born counter-rotating under a symmetric
  drift, and their whip-arounds circulate against the disk's
  rotational sense at exactly the stage-3 moment the owner watched.
- Fix 1 — corotation by construction (`infall.rs`): each glyph now
  samples a specific angular momentum ell (uniform in the new
  `BLACK_HOLE_INFALL_COROTATION_MIN`/`_MAX` range, outer radii) and
  derives its horizontal drift from the exact angular-momentum
  identity, so the specific angular momentum (x times vy minus y times
  vx) is positive at spawn for every x (the
  ring motes' sign), gravity preserves it exactly (a central force),
  and the accretion brake only decays its magnitude — no live mote
  can ever carry the counter-rotating sign. The ballistic crossing
  of the hole's latitude lands at |x| = ell (the sampled impact
  parameter — the capture-variety spread preserved); the left-side
  feeding streams carry the stronger rightward drift corotation
  requires there, the coherent-vorticity read of ambient material
  sharing the disk's rotation axis. The retired
  `BLACK_HOLE_INFALL_DRIFT_FRACTION` (symmetric ±0.45 drift) is
  deleted with its doc.
- Fix 1 companion — kinetic heat is now the RADIAL approach speed
  (the plunge component): the corotating drift is the ambient
  medium's cold inherited angular momentum, not heat, so the calm
  Ghost entry survives the new drift field (grading total speed
  would have lit every feeding stream Hot at spawn); the periapsis
  whip still flashes (approach speed spikes + the proximity bump).
- Fix 2 — width-adaptive composition (`black_hole.rs` reset pass):
  the ball is capped at `BLACK_HOLE_BALL_WIDTH_MAX` (0.30) of the
  viewport half-width — wherever the cap binds (every viewport up
  to roughly 1.8:1 aspect, the common terminal classes) the shadow
  reads 30% of the terminal width and the disk dominates (the
  narrow-screen read); the disk's scale unit becomes the larger of
  the limiting half-extent and `BLACK_HOLE_DISK_WIDTH_FRACTION`
  (0.72) of the half-width — on wide terminals the disk stretches
  to the projection's 92%-of-half-width clamp (the majestic
  full-width read), and the tier semi-majors key on it
  (`project_ring_mote` gained a `disk_unit` argument). The ring
  proximity ladder rides the stretch gain (`proximity_gain`), the
  Keplerian shear normalization tracks the real semi-major-to-ball
  ratio (`advance_ring_mote` gained a `disk_gain` argument), and
  the tier semi-minors/offsets stay ball-keyed — the stretched disk
  reads thinner, the physical Gargantua proportion.
- Fix 3 — the see-saw window re-cut (`roll.rs`, split from
  `ring.rs` for the 800-line cap): the excursion menu drops the
  85-degree rung (`BLACK_HOLE_ROLL_TILT_DEGS` is now
  [60, 50, 45, 30, 15]) so the whole 70-110 degree near-vertical
  window is excluded outright — the disk never parks or sweeps
  where the screen would clip it. A DYNAMIC tilt cap
  (`RingRoll::set_tilt_cap`) derives from the viewport's vertical
  budget (92% of the half-height over the tier-0 semi-major with
  wobble headroom): viewports that cannot host the 60-degree rung
  get the rung lowered (a stretched wide-screen disk tilts
  shallower — a long thin disk must), and a shrink applied
  mid-hold arms an immediate eased return to the rest line (a
  resize while tilted never leaves the stack clipped for the
  remaining dwell). Style re-entry re-applies the stored cap to
  the fresh scheduler.
- Tests: the see-saw contract re-pinned to the new menu and the
  excluded-window bound; the infall corotation contract pinned two
  ways (4000-sample strict spawn check incl. the impact-parameter
  range, plus the steady-state live-mote invariant); the
  width-adaptive composition contract pinned across the terminal
  classes (120x40, 200x50, 105x64, 80x24 — ball cap, disk stretch,
  92% reach guard); the tilt-cap contract pinned (tight-cap
  excursion bound over 20 minutes of schedule, mid-hold shrink
  recovery); the band/core geometry pins re-derived for the new
  sizing. 64/64 black hole contracts, full suite 2792 green,
  fmt/clippy clean, version untouched.
- A/B benchmark evidence (10s, 120x40, wet IO, run after the
  commit): recorded in docs/BENCHMARKING.md — a performance WIN,
  not a regression: avg fps 2042.2 -> 2330.0 (+14.1%), p99 frame
  time -22.1%, avg dirty cells/frame 184.0 -> 161.0 (-12.5%). The
  win is mechanical: the width-capped ball at 120x40 drops the
  shadow from 11 to 9 line-height radii and the annulus area (the
  per-frame ball cell count) scales with r squared — about a
  third fewer ball cells drawn and diffed every frame. Visual
  signature shifts track the geometry (entropy 6.00 -> 5.96
  noise, gini 0.5642 -> 0.5758 — the smaller shadow concentrates
  the composition slightly).

### fix: NIGHT-depthtest-2 & hunt-30 — CLI/config duplicate-name audit, silent-failure hunt, self-consistent dump-config suggestions, and the pure-English language gate

- Owner report (two transcripts): `--dump-config <existing>` refuses
  to overwrite and suggests writing to
  `~/.config/cosmostrix/config.toml.new` — following that exact
  suggestion then failed with "must have a .toml extension". The
  error message recommended a path the same command rejected.
- Fix: the refusal now suggests `<stem>.new.toml` (final extension
  `.toml`, so the suggestion passes every validation rule the flag
  itself enforces; the review-then-rename workflow is unchanged).
  Message construction lives in the pure
  `dump_config_overwrite_refusal()` for direct regression coverage
  (the suggested path is asserted to survive `validate_config_path`).
- Duplicate-name audit (charset/colors/scene-custom): real TOML
  rejects duplicate keys and duplicate `[section]` headers with a
  hard parse error; cosmostrix's forgiving parser silently merged
  reopened sections and let the last writer silently win duplicate
  keys — a duplicated `[scene-custom.x]` / `[colors-custom.x]` /
  `[charset-custom.x]` block was indistinguishable from a single
  edited one, and a duplicated key (including `ambient.HH-MM`
  slots) silently switched values. `ParsedConfig` now records
  `duplicate_keys` + `duplicate_sections`; the map keeps the
  documented merge/last-wins semantics for the validation-bypass
  path. All three validation surfaces reject in lockstep (the
  S-master-HUNT-2 uniform-rejection contract): startup exit 2
  (Layer 1.5, new `config_apply_diagnostics.rs`),
  `--testconf` (duplicate_diagnostics errors), and the live-reload
  watcher (`validate_and_send` rejects between malformed and
  unknown — an editor that duplicates a block mid-save no longer
  gets a silently-merged "successful" reload).
- Silent-failure hunt: an explicit `--config <path>` that cannot be
  read (missing, unreadable, over the size cap) previously produced
  a silent empty parse and a full run on defaults — a typo'd path
  was indistinguishable from an intentional default run. The read
  failure is now recorded in `ParsedConfig::read_error` and startup
  exits with the real reason. The default-path load is exempt by
  design (missing default config = normal first run, /etc fallback
  applies); an existing empty or all-comments config still applies
  (the deliberate-empty contract is unchanged).
- `.toml` extension check is now case-insensitive: Windows
  filesystems are case-insensitive, so `CONFIG.TOML` is the same
  file there — the CLI layer no longer contradicts the whitelist.
  On Unix a cased miss now lands in the new explicit-read-error
  path with a clear message instead of a bare extension rejection.
- Pure-English rule (owner directive 2026-09-11): commit messages,
  comments, strings, docs and diagnostics are English only. New
  `scripts/language_audit.py` scans every tracked text file and
  fails on human-language content (non-Latin letter runs of 2+,
  non-allowlisted Latin diacritic words), keeping functional
  categories by design: isolated math/unit letters (µs, π/2, Δx),
  charset glyph data lines, and unicode-stress fixture files (each
  exemption documented inline in the script). Wired into
  `gate-keepers.sh` as check 13, so CI enforces the rule. The audit
  passes clean today: the codebase prose was already English; the
  detector + gate make it stay that way.
- hunt-30 (found by the audit, not reported): the FreeBSD
  system-wide config path `/usr/local/etc/cosmostrix/config.toml`
  was whitelisted for reading and documented everywhere, but was
  never a candidate in `config_candidate_paths()` and never the
  loader's default-path fallback (both hardcoded `/etc`). A
  system-wide FreeBSD install silently never loaded its config —
  startup ran pure defaults, `--testconf` and `--config-path`
  reported a missing file, and the live-reload watcher never
  watched the real config. New `system_wide_config_path()` (owned
  by the loader, re-exported through `configfile`) gives both call
  sites one definition: `/usr/local/etc` on FreeBSD (the
  ports/packages convention, with the Linux-style `/etc` kept as a
  trailing fallback candidate for hand-placed configs), `/etc`
  elsewhere — non-FreeBSD behavior is unchanged.
- Tests: 25 new regression tests
  (`test/tests/depthtest_cli_config2.rs`) pinning the suggestion
  validator contract (both owner transcripts), parser duplicate
  detection (custom blocks, case-insensitive sections, root keys,
  ambient slots, promoted-key non-interference), the three-surface
  lockstep, explicit-read-error behavior (missing vs
  empty-existing vs comment-only), the case-insensitive extension
  rule, the dump-template pure-English invariant, and the
  platform-documented system-wide candidate. Full suite:
  2789 passed / 0 failed.

### fix: NIGHT-hunter-29 — resume micro-jump on the structured rain families: the phosphor ownership rule (the black hole strobe, audited and fixed across every rain type)

- Owner report: after the NIGHT-hunter-28 glyph fix, a micro jump
  still read on the black hole at sorgonemous_intrascals when
  resuming from pause at seriously high detail. Directive: depth
  audit ALL rain types for consistency, not just the reported one.
- Root cause (found with a new cross-family instrumented audit,
  not by inspection): the phosphor decay pass fought the
  structured families' own draw pass over persistently-drawn
  cells. A cell the family draws every frame with identical
  content (the ball annulus, slow tier motes, frozen field cells
  during the pause decel and the resume ramp) falls out of the
  phosphor pass's fresh set, and the ghost write (the A19 tracked
  path or the A20 orphan fallback) dims it; the next frame's
  family draw restores the drawn state — a period-2
  full-population brightness strobe, blend-independent, 621 cells
  per frame on the black hole repro (5x its steady diff). At the
  resume instant, when every other motion ramps from frozen, the
  strobe is the only visible movement: the owner's micro jump.
  The glyph family never had the fight (the phosphor's Pass 2
  marks droplet-covered cells fresh every frame); monolith had
  partial immunity (clear_spine_phosphor, spine cells only).
- Fix — the phosphor ownership rule, monolith-precedented: a cell
  the family draws THIS frame is owned by the draw, so its
  phosphor state is zeroed every frame
  (clear_phosphor_metadata, now shared from monolith_helpers at
  `pub(in cloud::type_rain)` scope). Applied to the whole
  structured family tree: black hole, vortex, flux, lorenz,
  dragon, physarum, aeolian, solar flare, DNA helix,
  murmuration, quasar, neural, and monolith's falling segments
  (generalizing the old spine-only immunity). Vacated cells keep
  their clear_cell zeroing, so no family leaves afterglow state
  behind a cell it still owns — the residual churn at the frozen
  instant collapses to the families' documented 2% life-sign
  shimmer (26 cells on the black hole repro, from 621).
- Verification: a new 28-test resume-continuity audit
  (tests_resume_audit_hunt29.rs) drives every rain family — mono
  AND TrueColor variants at production parity — through the real
  pause state machine (BRANCH 3 decel -> settle -> full freeze ->
  BRANCH 2 unpause -> accel ramp -> settle) and asserts the
  per-frame dirty-cell count stays inside the steady envelope
  (spikes must be sustained to read as a jump: the neural
  thought-burst and monolith hero spawns are legitimate 1-3 frame
  events, the strobe held the whole population for the entire
  window). The detector caught the pre-fix black hole failure at
  resume frame 0 with blend 0.000 and 621 dirty cells.
- Hunted beyond the report while auditing: the flux test harness
  never armed the sim cap (max_sim_delta defaults to zero, so the
  family's advance clamps to dt=0 — mirrored the production
  contract in the audit driver); the lorenz baseline needed a
  multi-attractor-cycle sample (its dirty count swings between a
  parked low and a crossing high over seconds).
- A/B benchmark evidence (10s, 120x40 truecolor, run after the
  commit): sorgonemous_intrascals avg fps 13028 -> 15743 (+20.8%),
  avg dirty cells/frame 789.4 -> 179.6 (-77.3%, dirty ratio
  16.45% -> 3.74%), avg render ms 0.0230 -> 0.0062 for +0.005ms
  of sim; monolith avg fps 40234 -> 50252 (+24.9%), dirty
  270.7 -> 107.3 (-60.4%). The strobe had been inflating every
  structured family's steady-state diff — the fix is a net
  performance WIN, and the post-fix (entropy, gini) points are
  the styles' true structure signatures (the strobe's constant
  whole-field flicker had flattened the density and inflated the
  entropy). Numbers recorded in docs/BENCHMARKING.md.

### fix: CI (FreeBSD) — neur_machine_assembles_through_genesis asserted a transient as an invariant

- The FreeBSD-native CI run failed
  `cloud::tests::tests_neural::core::neur_machine_assembles_through_genesis`
  on `!s.active || s.grown >= 1.0` although the local run passed
  2736/2736: the assertion read "every wire complete" at an
  ARBITRARY steady instant, but the plasticity economy legitimately
  keeps one wire mid-regrowth a fraction of the time — and the
  rewire schedule is not platform-stable (libm ulp differences in
  the leak/kick factors shift the fire timing, which shifts the
  shared RNG stream, which moves the rewire clock).
- The test now asserts the real contracts: the no-seam handoff at
  the moment the genesis completes (probed at lit + 1.28 sim-s,
  inside the first rewire clock's 4.5 sim-s jitter floor — the
  Thought seam force-completes every wire, deterministic on every
  platform), and in the steady state "at most one wire
  mid-growth" (the bounded-learning contract: the rewire clock's
  jitter floor 4.5 sim-s exceeds the full retire-plus-regrow chain
  2.5 sim-s, so two successors can never overlap).

### fix: CI (gate-keepers) — ruff and comment-style violations in the NIGHT-hunter-19 batch

- scripts/endurance_probe.py: import block re-sorted (I001), the
  successive-pairs loop migrated to itertools.pairwise (RUF007),
  and two format deviations normalized; scripts/
  hud_long_scene_e2e.py: one format deviation. All auto-fixed via
  ruff check --fix + ruff format; the gate now runs clean locally
  with ruff 0.16.6.
- test/tests/depthtest_cli_config.rs: five decorative markdown
  emphasis markers in module-level doc comments rewritten as
  plain prose per docs/COMMENT_STYLE.md section 2
  (check-comment-style.py: 483 files, 0 markers).

### feature: NIGHT-hunter-27 — 'r' is now a FULL FRESH: the restart re-applies the current scene's builtin defaults, wiping every runtime user change before the from-zero relaunch

- Owner directive: pressing 'r' after runtime changes (color cycles
  via 'c'/'C', charset via 's'/'S', speed via Up/Down, density via
  '['/']', or a live-reloaded config key) returned "your changes,
  reseeded" instead of the scene's own defaults — sorgonemous_intrascals
  came back green, not energy-zen/binary/speed-12/density-0.55.
- The 'r' arm now layers the scene's builtin field set back on top
  BEFORE the NIGHT-lts-3 relaunch, through the same
  `apply_scene_runtime_with_cfg` path the 'x'/'X' scene cycle uses:
  color (also dropping any active custom palette), charset, speed,
  density, glitch level, and rain style all return to the scene's
  values. Custom scenes re-apply their complete `[scene-custom]`
  block layer through the last-applied config map (the same map the
  live-reload rebuild consults).
- Ownership + harmony state is reset to fresh-launch parity: the
  ambient override flags mirror the 'x' cycle contract (snapback
  timer re-arms, palette lock clears), and any mid-flight Crystal
  Dragon drift retires (drift_active/drift_start cleared, poll cycle
  re-armed) so the re-applied palette is ground truth.
- Scene fps is re-asserted through the ambient-fps application point
  (power manager + HUD + effective-config tracker) — the scene
  family owns fps and the Cloud does not pace frames; custom scenes
  with `fps ≠ 60` restore correctly. The key dispatch contract grew
  a `KeyOutcome::FreshScene` variant (was a bare bool) so the event
  loop can perform the follow-up the Cloud cannot.
- Semantics preserved: 'r' remains suppressed while paused or
  decelerating (only 'p' and 'q' respond — the pause isolation
  contract), still relaunches from zero (fresh RNG stream, birth
  choreography replayed, message typewriter restarted), and still
  does not change which scene is active.
- Regression suite: test/interactive/tests_night_hunter27.rs — 7
  tests covering the owner's exact flagship repro, the from-zero
  replay, scene identity, ambient/drift reset, pause suppression,
  custom-scene blocks, and the legacy wake contract for all other
  keys.

### fix: NIGHT-hunter-28 — resume from pause is now phase-continuous: no more brightness-reshuffle pop at high detail

- Owner report: pause (deceleration) reads smooth and elegant, but
  resume "still has a small jump" at seriously high detail. Root
  cause found in `toggle_pause()` BRANCH 2: the unpause path
  re-randomized every live droplet's `advance_remainder`
  (`rand_chance.sample()`). That remainder drives BOTH the head
  cell's brightness ramp (`1.0 + fractional_progress × 0.15` — the
  per-frame "energy building" pulse) and the timing of the next row
  advance, so the re-randomization reshuffled every visible head's
  brightness by up to ±15% in a single frame — a global shimmer pop
  exactly at the resume instant, most visible with many heads, long
  tails, and phosphor afterglow.
- Fix: preserve the frozen phase. The pause freezes remainders
  mid-phase and the resume continues from exactly those values —
  C0-continuous in both brightness and advance timing. The old
  lockstep guard this randomization replaced was for an even older
  bug (zeroing the remainders synchronized them); with
  SPAWN_PHASE_JITTER=true the spawn-time spread survives the
  freeze/thaw, so preservation has no lockstep and no pop.
- Structured families (black hole, vortex, dragon, …) were already
  jump-free via the clamp-and-blend dt contract; this fix closes the
  glyph-family gap.
- Regression suite: two tests in tests_exp_decay.rs lock the exact
  phase equality across the unpause call and the preserved spread
  (no lockstep) after resume.

### fix: NIGHT-hunter-19 & depthtest-1 — CLI + config/live-reload depth audit: 15-test seeded stresstest harness; one startup crash fixed (empty-argv panic)

- Depth audit of the full CLI chain (argv expansion → clap parse →
  canonicalize) and the full live-reload chain (config text parser →
  strict validation → watcher debounce/dedup → rebuild_cloud_config),
  including the capped-read I/O layer and the 24h duration ceiling.
- New harness: test/tests/depthtest_cli_config.rs — 15 deterministic
  (seeded xorshift) stress tests across seven surfaces: argv
  expansion totality, adversarial clap argv, the numeric CLI parsers
  (NaN/inf/unicode-digit/overflow corpora), config-text parser
  permutations (quote/bracket/CRLF/section/array pitfalls), strict
  per-key validation classification, rebuild invariants (finite +
  in-range numerics, non-empty charset pool, key-reflection
  contract), and the full watcher pipeline (parse → validate →
  rebuild) over random mutations of a valid config file.
- Bug found and fixed: `expand_argv_shorthands` indexed `argv[0]`
  directly — a crash-on-startup when the process is exec'd with an
  empty argv list (execve accepts `[NULL]`; `std::env::args_os()`
  then yields zero items). Now guarded with `first()`; clap handles
  the missing program name itself. Locked by a regression test.
- Verified clean: the strict gate classifies every known key ×
  adversarial value deterministically (the S-master-HUNT-2
  lockstep contract holds), the rebuild layer never lets a NaN/inf
  or out-of-range value reach frame pacing or spawn math even when
  validation regresses (defense-in-depth holds), and the malformed
  report diagnostic always traces back to a real input line.

### stability: NIGHT-lts-7 — the HUD metrics depth audit: realtime, accurate, zero measurable overhead, stable; one e2e-harness robustness fix (zero metric-code changes)

- Audited the full HUD metrics surface: hud/ (975-LOC state module +
  metrics + init + colors), event_loop_hud.rs per-frame push,
  sysstat/ samplers, FrameTimeTracker, and both e2e scripts.
- Realtime verified: 1 Hz metric tick with deliberately aligned
  metric/RSS/CPU intervals (same tick, half the fast-path timestamp
  comparisons); colors refresh every frame so palette changes land
  next-frame.
- Accuracy verified contract-by-contract: cpu% is two-sample delta
  math (div-zero guarded, warm baseline for instant toggle-on,
  pause-aware window); rss reads VmRSS with honest "—" fallbacks;
  p99 sorts a stack-anchored 60-slot snapshot (~300 ns); dcel is a
  60-frame rolling average with a pause-freeze contract; uptime
  excludes paused time at sub-second precision with deterministic
  calendar units.
- Overhead measured at zero: A/B on the pro binary (200x56 PTY,
  60 FPS, continuous drain) — HUD off 4.76% vs HUD on 4.47%; the
  delta is run-to-run noise. Structural: one ~2 KiB /proc read per
  second, clear+push_str setters, &'static str mode suffixes,
  dirty-tracked frame writes.
- Stability verified by e2e: hud_order_e2e.py PASS (25/25 labels,
  exact screen-row order); hud_long_scene_e2e.py PASS (27-char
  scene name renders in full, border past the text).
- One finding, fixed: hud_long_scene_e2e.py hardcoded the
  target/pro binary path and produced a FALSE renderer failure on
  release-only machines (the exec died instantly, the empty screen
  read as a defect). BIN now resolves env override > pro > release
  with a clear fatal message when nothing exists.
- Full evidence: docs/research/NIGHT_LTS_7_HUD_METRICS.md.

### fix: NIGHT-hunter-26 — resize no longer half-reloads the message overlay: the mfs reveal, sparks, smoke, and touch pulses all continue through a resize (the 'r' shortkey remains the only full replay)

- Owner report: with msg mode active, resizing the terminal made
  every mfs style "like want reload but just half little" — subtle
  enough to need sharp eyes. Root cause: the resize path's
  `reset_message()` conflated the geometry rebuild (legitimately
  resize-dependent) with the fresh-reveal resets (sidecar wipe +
  movement-detector re-arm). The reveal timeline itself never
  stopped, but every in-flight engrave spark / scorch smoke puff /
  border-touch pulse vanished, and the re-armed detector fired a
  spurious burst at the long-revealed head on the next frame.
- Surgical split: `reset_message()` = `relayout_message()` +
  fresh-reveal resets. Resize (`reset_with_bounds`) and the 'm'/'mb'
  border toggle now call the geometry-only `relayout_message()` —
  a resize is an interrupt, not a replay (same philosophy as the
  Phase D drift-state contract). `set_message`,
  `set_msg_fill_style`, and the 'r' restart keep the full reset —
  the owner-excluded replay paths are pinned by tests to stay
  restarts.
- The engrave/scorch movement detectors now fire on FORWARD head
  movement only: a height-truncating resize moves the reveal head
  BACKWARD (the budget clamps to the smaller wrapped layout), and
  bursting on a backward jump was the phantom re-engraving flash.
- The border-pulse draw pass bounds-checks `msg_idx` against the
  rebuilt grid (pulses survive layout rebuilds now, so a stale
  index is dropped instead of indexing out of bounds).
- Nine new regression tests; full suite 2712 passed / 0 failed.
  Root cause + evidence: docs/research/
  NIGHT_HUNTER_26_MSG_RESIZE_RELOAD.md.

### stability: NIGHT-lts-4 — the master endurance audit: CPU, memory, fd/threads, GPU-freedom, and bloat-freedom all measured at peak; zero code changes (the report + reusable probe are the product)

- New evidence machine: scripts/endurance_probe.py — release binary
  in a PTY (200x56, 144 FPS dynamic default, 12 MB/s marginal
  drain), sampling /proc for CPU, VmRSS, page faults, and context
  switches. Reproducible with RUN_SECS/SAMPLE_SECS/SCENE/SIZE/
  DRAIN_BPS knobs.
- CPU verdict: steady 5.7-7.2% of one core at 144 FPS under
  marginal drain (cinematic + sorgonemous_intrascals). The poll +
  <=500 us spin hybrid, PowerManager effective_fps, thermal shed,
  drain backoff, and the dead-PTY guard all hold it there.
- Memory verdict: NOT a leak — slope decays +1.69 -> +0.56 ->
  +0.32 MB/min across 30 s / 3 min / 6 min runs (asymptotic ~9 MB
  cinematic plateau, ~6.4 MB black hole). A true leak is linear;
  this is scene pools filling once.
- fd count flat at 10, threads flat at 5 (no leaks); minor faults
  1.3-8.6/s (no page churn); voluntary context switches ~131/s
  (sleeping, not spinning).
- No GPU: zero GPU dependencies or code paths (doctor contract:
  "CPU+stdout renderer; no GPU context is ever created").
- No bloat: 10 direct deps, all load-bearing, all
  default-features-minimal; chrono previously removed for libc
  calls; 2.9 MB release binary.
- Full evidence table + method: docs/research/
  NIGHT_LTS_4_ENDURANCE_RESOURCES.md.

### stability: NIGHT-lts-6 — the early-return command ladder is now single-sourced, documented, and test-pinned (owner report: combined commands like `-v -s --dump-config ... --version --doctor` must pick one winner, deterministically, to avoid confusion)

- Owner report: running several print-and-exit commands together
  (`cosmostrix -v -s --dump-config <path> --version --doctor`)
  should never be a guessing game — exactly one must fire, and the
  winner must not depend on the order the flags were typed.
- Removed a dead duplicate dispatch: `main.rs` carried its own
  `if args.doctor` block that fired BEFORE
  `handle_post_config_returns`, making the --doctor branch there
  unreachable (a v50 LOC-refactor leftover). The exact
  ordering-drift class the owner flagged — two owners for one
  command is how behavior silently diverges. --doctor now has a
  single owner: the post-config dispatcher.
- The canonical ladder is now documented in one place
  (`src/cli/early_returns.rs` module docs): parse errors > --help >
  --reset-terminal > --dump-config > --config-path > --testconf >
  --list-scenes > --list-charsets > --list-colors > --show-scene >
  [config apply] > --doctor > --version > --docs > --check-update >
  benchmark > interactive. Pre-config commands work with a broken
  or missing config file; post-config commands report the merged
  config view. Runtime flags (-v, -s, ...) are inert when an early
  return fires (standard early-exit semantics).
- The ladder order is encoded once in two pure classification
  functions (`classify_pre_config` / `classify_post_config` — no
  I/O, no exits) consumed by both dispatchers, so it can never
  silently drift between the two phases.
- Nine new precedence tests pin the full pairwise matrix
  (owner's combined-command case included, both argv orders) plus
  the phase boundaries: pre-config flags are invisible to the
  post-config classifier and vice versa.
- Behavior-preservation verified: the full smoke matrix
  (version/doctor/dump-config/help/docs/config-path/list/testconf,
  14 combinations, both argv orders) is byte-identical before and
  after. Startup latency measured at 2-4 ms (debug build) — the
  early-return paths are already at peak; no optimization needed.

### stability: NIGHT-lts-3 — the 'r' restart now starts from zero like a fresh launch (the owner's black hole report: the restarted hole popped in already formed); restart + dynamic-resize contracts audited and pinned across all fourteen type rains

- Owner report: pressing 'r' on the black hole scene restarted
  into a state "very different from startup" — not a real start
  from zero. Root cause: the 'r' handler ran the resize-semantics
  reset, which rebuilds geometry and empties pools but preserves
  each choreographed family's birth state — the hole's `formed`
  flag survived, so it popped in fully formed instead of replaying
  the formation intro a fresh launch plays. The same divergence
  silently affected DNA helix (molecule pre-born), quasar (engine
  pre-lit) and neural (machine pre-trained).
- New `Cloud::restart_from_zero`: a restart is a relaunch, not a
  resize. It re-seeds the deterministic RNG stream before the full
  reset (so the reset's glitch-clock draw consumes the launch
  stream), re-captures the time anchor, reconstructs the
  ecosystem/drift accumulators at their unevolved defaults,
  re-seeds the ghost-event scheduler's dedicated RNG, clears any
  pause/resume easing in flight, and re-arms the birth
  choreography for the current style (black hole formation, DNA
  genesis, quasar ignition, neural genesis) — the same contract a
  scene entry honors. The plain structured families and the glyph
  family already matched startup through the full reset alone.
- The resize contract is kept distinct and test-pinned: a pure
  resize still keeps the steady state (an interrupt must not
  replay births or snap the visual climate); 'r' is the only
  relaunch path. Dynamic screen size verified for all fourteen
  styles: resize up and down rebuilds every family for the new
  viewport with live in-bounds content.
- Nine new restart tests (choreography replays for the four
  families with the owner's exact repro, the RNG from-zero
  contract, pause/anchor clear, cinematic refill, resize up/down
  for every style, the resize-vs-restart distinction); the
  HUNT-15 restart-clearance loop extended from seven to all
  fourteen styles. Help text, README and RULES.md keybind tables
  now say "restart from zero" instead of "reset animation".
- Zero hot-path code changed (restart_from_zero is the 'r'
  keybind's cold path; the bench harness never presses 'r'), so
  the A/B benches are skipped per the waste guard. Full audit
  table and evidence: docs/research/NIGHT_LTS_3_RESTART_CONSISTENCY.md.

### feature: NIGHT-lts-5 — the x/X scene cycle head swaps to the owner's approved signature order (cinematic, the glyph default, first; sorgonemous_intrascals, the black hole, second)

- Owner approval (2026-09-10): the default signature rain is the
  glyph — scene name `cinematic` — and the second signature is the
  black hole — scene name `sorgonemous_intrascals`. SCENE_ORDER
  positions 1 and 2 swap accordingly; monolith (3) and lorenz (4)
  onward are untouched, so every downstream adjacency keeps its
  NIGHT-lts-2 placement.
- The head of the cycle is now the launch default itself
  (DEFAULT_SCENE = cinematic): a fresh launch and a full x/X tour
  open on the same scene, and the tour's first `x` keystroke lands
  on the black hole.
- Adjacency changes rippled through the suite: cinematic forward
  goes to the black hole (was monolith), monolith backward to the
  black hole (was cinematic), the tail wrap lands on cinematic
  (was the black hole). Four test surfaces updated (scene
  forward/backward/order-pin, interactive uppercase-X) plus two
  comment syncs (interactive forward-order, flux position pin).
- Docs synced: README cycle line, the ambient scheduler
  same-palette gotcha (the head trio `cinematic` /
  `sorgonemous_intrascals` / `monolith` all default to
  `energy-zen` — also fixing the stale `neon-purple` palette name
  en route), scene/mod.rs order comments renumbered.
- Zero render-path code changed (an array head swap plus
  comments); the bench harness drives scenes via `--scene` and
  never exercises the cycle, so the A/B benches would measure
  pure noise — skipped for this change per the task rules'
  waste guard.

### docs: NIGHT-lts-1 stage 1 — the master depth audit of black hole + quasar: both at peak, zero code changes (the report is the product; stage 2 awaits owner approval)

- Owner directive: master depth audit for all scenes and type rains,
  staged pair by pair; stage 1 = blackhole + quasar, report to owner.
- Full read of every advance/draw/spawn hot path across the 13 files
  (5 427 LOC). Panic/unwrap/expect scan: ZERO hits. Const assertions
  pin the physics invariants. 82 contract tests across the two
  styles.
- Verdict: both at peak — dt saturating+clamped+resume-blended, phase
  wraps at 64 turns, clamps at every use site (defensive in depth),
  per-frame hoisting, amortized O(1) free-slot scans, zero hot-loop
  allocations, fraction-based geometry (the dynamic-size contract).
- Five micro-observations quantified and skipped (the
  over-engineering guard): redundant parallel-array bounds checks
  (~0.003 percent of frame cost), sqrt-vs-squared occlusion compare
  (~600 ns/frame), per-mote Kepler powf (already the fast f^-1.5
  form), the u32 generation-counter wrap (unreachable in practice),
  first-frame reserve ordering. None buys a perceivable anything.
- Baseline 10 s pro benches recorded in the report:
  sorgonemous_intrascals 25 914 avg fps / 330.3 dirty cells per
  frame / gini 0.554 / entropy 5.483; quasar 96 525 / 84.8 / 0.711 /
  4.789 — both one to two orders of magnitude beyond any terminal's
  display rate (the quantitative peak verdict).
- Docs-only change — no benchmark per the task rules (the numbers
  above are this build's baseline measurements, not an A/B).

### feature: NIGHT-hunter-18 — the low-terminal cinematic-intro auto-skip (the owner's "no need cinematic mode" half); the high-perf peak audit closes the other half (nothing held back)

- Audit finding: the owner directive was already 80 percent
  implemented — S-master-HUNT-24 ships the effects auto-gate
  (`effects_auto_off_applicable`: console TTY / dumb / pure-CPU
  renderers resolve effects_enabled false at startup, with the
  `[auto-fx]` verbose notice), and the termdetect layer already
  tiers high-perf terminals to peak: dynamic default fps 144
  (60 standard), 240 fps ceiling (30 on xterm.js), phosphor decay
  1.0 (1.3 VTE / 1.6 xterm.js), no ghost brightness cap (0.10 /
  0.15), speed mult 1.0, sync output, kitty keyboard where
  supported, effects on. High-perf = peak: confirmed, nothing to
  unleash.
- The gap was the cinematic intro: the particle-driven intro
  sequence played even on the raw Linux console and dumb terminals
  (the intro's only gates were terminal size and the skip key).
  NIGHT-hunter-18 adds `resolve_intro_type()` in build_cloud_cfg —
  a unit-testable seam mirroring `resolve_effects_enabled`: the
  built-in Logo default resolves to None on the effects-gate
  population (low terminals, the owner's "no need cinematic mode"
  half); an explicit `--intro` value always wins (the CLI-lock
  precedence chain — the user who asks for the intro on a low
  terminal gets it); bench mode resolves to None.
- The `[auto-intro]` verbose diagnostic surfaces the skip decision
  the same way the `[auto-fx]` notice does — only when it actually
  changed the outcome.
- Four new tests (hunter18_intro_gate_tests): high-perf keeps the
  Logo default, low terminals skip it, explicit --intro always
  wins both directions, bench resolves to None.
- Docs synced: the `--intro` and `--no-effects` help sections and
  the README intro bullet now document both auto-gates.
- The change is startup-resolution only — the intro gate sits on
  the interactive path (event_loop), unreachable from the bench
  harness (bench mode pins intro None before the gate), so the
  A/B benches are skipped per the task rules' waste guard.

### feature: the three themed config presets return (cyberpunk_2077, quantum, tron_legacy) — cut in the NIGHT-quality-1 template rewrite, restored verbatim with end-to-end load tests

- Owner directive 2026-09-10: the cyberpunk_2077, quantum and
  tron_legacy presets were removed when NIGHT-quality-1 rewrote the
  dump-config template from 269 to 138 body lines — they return now,
  text-identical to the pre-rewrite blocks.
- Restored: two complete themed scenes (`[scene-custom.cyberpunk_2077]`
  monolith streams / fps 90 / speed 12 / density 0.90 / glitch none;
  `[scene-custom.tron_legacy]` flux field / fps 75 / speed 8 /
  density 0.70 / glitch subtle), two 7-stop palettes
  (`[colors-custom.cyberpunk_2077]` yellow-magenta-cyan on near-black;
  `[colors-custom.tron_legacy]` deep-blue-to-white grid), and three
  charsets (`[charset-custom.quantum]` the math-symbol set,
  `[charset-custom.cyberpunk_2077]` hex + half-width katakana,
  `[charset-custom.tron_legacy]` hex + box-drawing).
- Verified with four new tests (config_apply_tests/template_presets.rs):
  the template carries all seven block anchors; both scene blocks load
  end-to-end through `--scene-custom` (the seven-dimension contract
  resolves the palette reference, the charset reference and the
  numeric quartet); and every glyph of all three charset sets parses
  through the single-width filter (math symbols, half-width katakana,
  box-drawing — none wide, none dropped).
- Template text only — no render-path code changed, so the A/B
  benches are skipped per the task rules' waste guard (startup text
  cannot move fps, dirty cells, gini or entropy).

### feature: NIGHT-lts-2 + NIGHT-lts-5 — the x/X scene cycle re-ordered around the owner's signature pair (black hole leads, glyph default second); the default launch scene approval recorded (cinematic = glyph, first signature; sorgonemous_intrascals = black hole, second signature)

- NIGHT-lts-5 (owner approval, recorded): the default signature rain
  is the glyph — scene name `cinematic`, the launch default
  (DEFAULT_SCENE, already asserted by tests, unchanged) — and the
  second signature is the black hole, scene name
  `sorgonemous_intrascals` (the owner's own coinage, NIGHT-special-1).
- NIGHT-lts-2 (owner directive): the x/X cycle order now leads with
  the signature pair — `sorgonemous_intrascals` first, `cinematic`
  second — then `monolith` (3) and `lorenz` (4); `matrix`, `vortex`,
  `flux` and the style flagships follow; the tail still ends at
  `curiosity` and wraps back to the black hole. One X-press from the
  launch default reaches the black hole; the full 31-scene tour
  starts and ends on the owner's two signatures.
- Adjacency changes rippled through the suite: monolith forward now
  goes to lorenz (was matrix), flux forward to cosmic_dragon (was
  lorenz), physarum forward to aeolian (was the black hole), the
  tail wrap lands on sorgonemous_intrascals (was cinematic). Six
  test surfaces updated (scene forward/backward/wrap/order-pin,
  flux position pin, interactive x/X and kitty Shift+X).
- Docs synced: README cycle line (which was also missing `neural`
  since NIGHT-research-9 — fixed en route), the ambient scheduler
  gotcha (the cinematic/monolith shared-palette note now describes
  the new neighbors), scene/mod.rs order comments renumbered.
- Zero render-path code changed (an array reorder plus comments);
  the bench harness drives scenes via `--scene` and never exercises
  the cycle, so the A/B benches would measure pure noise — skipped
  for this change per the task rules' waste guard.

### stability: NIGHT-hunter-25 part 2 — the hot cell-draw family bundled (one shared CellPaint design for the shader/render pair), two inherited defects repaired

- The six deferred hot-path signatures from part 1 now ride value
  bundles: `CellPaint` (the pair — `resolve_cell_color` and
  `DrawCtx::get_attr` share ONE bundle design, so the shader and the
  renderer can never drift on parameter order again),
  `SolarCellPaint`, `ParticlePaint`, `PostRainInputs`, and
  `StyleCursor` (three `&mut` style refs folded into one). src/
  reaches zero `too_many_arguments` suppressions. Every bundle is
  all-`Copy` scalars and destructured once at the top of the body —
  zero algorithm change; the named fields kill the cross-wire
  hazards (line/col, head_put_line/length, now/t1, three adjacent
  bools) at the engine's hottest call sites.
- En-route LOC split: `CellPaint` pushed shaders/base/mod.rs over
  the 800-line cap — the three shader constants moved to helpers.rs
  (`TRAIL_EXP_LUT` re-exported so every path keeps resolving) and
  the test-support helpers to a new cfg(test) `test_util.rs`, joined
  by a `test_paint()` fixture that keeps the 47 shader test call
  sites one line each. mod.rs lands at 768.
- Inherited defect 1 (verified): the neural commit shipped with a
  red test — `cycle_scene_forward_order` at HEAD still expected
  quasar → classic while src cycles to neural (stash-and-run:
  FAILED). The missed assertions ride this commit.
- Inherited defect 2 (verified by gate-keepers):
  NIGHT_RESEARCH_9_NEURAL.md was the only .md of 185 missing the
  standard disclaimer; injected, the gate reads 16/16.
- Anti-finding recorded: a manual per-droplet `ShaderCtx` hoist
  measured noise-scale contradictory deltas — the `#[inline]` pair
  already folds the chain; the comment on `get_attr` now says so.
- 2671 tests pass, 0 failed; check-all -q exit 0 (well under the
  2-minute local budget); gate-keepers 16/16. A/B 10 s benches:
  performance- and visual-neutral within noise (density gini, frame
  entropy, fps, dirty cells) — the bundle scalar-replaces to the
  same register-level parameter passing.

### feature: NIGHT-research-9 — the neural network, the fourteenth rain style (the rain trains the network)

- The owner-approved runner-up finally seated (the quasar round's
  question answered the other way first): the registry's
  machine-mind domain — nature, life, the cosmos, and now the
  mind. `cosmostrix --scene neural` (cyan palette, binary
  charset, cycle position 15, grouped with the style flagships).
- The architecture: layered integrate-and-fire neurons laid out
  horizontally with the signal flowing DOWNWARD — the rain's own
  direction, so the data literally streams through the machine
  (the input band widest under the sky, tapering to the sparse
  output; the golden-angle y-jitter keeps the lattice organic;
  the output reads one rung hotter — the answer).
- The neuron: a bounded potential integrating kicks, leaking
  exponentially (the forgetting), firing at the threshold into a
  refractory window (a saturated cell sheds load like the real
  substrate), the fired flash decaying and the glyph re-rolling
  on fire (event-gated); the input band idles alive on
  spontaneous Poisson kicks between meals.
- The rain is the data: streamers fall onto the input band
  (columns biased toward the built inputs — the data aims at the
  machine); a landing streamer either births the next neuron
  (the fresh-write economy — the network is literally BUILT from
  the rain) or kicks the nearest input. The pulses: glyph signals
  riding precomputed dendritic paths at rolled speeds, delivering
  the wire's weight on arrival and lighting the wire's glow; the
  thought-burst clock is the drama event (a clump of inputs
  force-fires, the wave crosses the machine, the output flares,
  the data surges).
- The plasticity: the rewire clock retires one healthy wire at a
  time (a slow fade while its riding pulses land) and grows the
  successor in the SAME slot to a NEW target — the wire count is
  constant by construction; the machine's topology rewrites
  itself forever without flooding.
- The genesis (the owner's mandate): every scene entry replays
  the training run — the data falls, the layers materialize from
  the captures, the dendrites reach out in a staggered sweep,
  the wiring completes and the first thought fires (a pure
  resize keeps the trained machine; the bench fast-forwards).
- The wire budget is constant by design: the idle wire draws
  every third path cell (the dashed loom) and the events raise
  the brightness rung, never the cell count — the dirty-cell
  budget survives the drama by construction.
- Style registry surface: `RainStyle::Neural` (labels `neural` /
  `neural_network` / `neuralnet` / `nn`), the NEUR_* calibration
  section with compile-time contracts, the 31-scene catalog (the
  count detectors moved 30 -> 31 as designed), the eight-file
  module split honoring the 800-LOC cap, the neural contracts in
  `tests_neural/`, and `scripts/neural_smoke.py` (the PTY shape
  smoke, quasar_smoke's heir).

### feature: NIGHT-research-8 — the quasar, the thirteenth rain style (the rain feeds the engine)

- Owner-approved pick over the neural-network proposal (the DNA
  genesis round's question): the canonical active galactic
  nucleus mapped to the terminal grid — the black hole rain
  style's LOUD sibling, the same engine running at full power.
  `cosmostrix --scene quasar` (stars palette, braille charset,
  cycle position 14, grouped with the style flagships).
- The engine: a Keplerian accretion disk (omega ~ r^-1.5 — the
  inner ring laps the outer ~6x, the shear IS the rotation read)
  on the classic tilted-ellipse projection, the radial
  temperature ladder, and the doppler beaming collapsed to a
  mono-safe brightness-rung asymmetry (the approaching limb steps
  up, the receding dims — the M87 photograph's signature). The
  core breathes white-hot with a pulse-following glow ring; the
  poles fire precessing relativistic jets (recycling streams with
  per-particle energy shares — the spread that keeps a beam from
  riding in lockstep — knots traveling as flare-launched pulses);
  a sparse halo annulus glides in from beyond the frame and
  breathes with the core.
- The rain is the fuel: infalling streamers spiral in on the
  accelerating plunge, and the capture economy builds the disk
  from them — a landing streamer either births an orbit (charged
  to full, the DNA rung-charge economy's heir) or re-charges the
  nearest-angle orbit. The starvation lesson from the DNA helix
  is a pinned contract (the counters stay honest, tests assert
  it).
- The ignition (the owner's genesis mandate): every scene entry
  replays the birth — the dark cloud falls, the disk condenses
  from the captures, the core lights, the jets push out — four
  continuous phases with an exact no-seam handoff to the steady
  law. A pure resize keeps the burning engine; the bench
  fast-forwards (the Z-6 critical-path contract); 25 test
  contracts pin the sequence and the orchestration.
- The feed-flare clock (the drama event, the murmuration
  startle's heir): a gas clump arrives — the core locks above its
  peak, the infall surges, a knot climbs each beam, and the
  core's glyph re-rolls (event-gated mutation).
- Hunt-find (the owner's mandate): the scene/mod.rs doc header
  still claimed "27 built-in scenes" and a "seven structured
  style flagships" list that predated solar_flare, dna_helix and
  murmuration — both stale for two rounds; corrected to the live
  counts (30 scenes, eleven flagships). The rain_style.rs
  "ten non-droplet styles" comment was likewise two styles
  behind (now twelve). The scene-count change-detectors moved
  29 -> 30 as designed.
- Verification: full suite 2655 passed / 0 failed (was 2630);
  fmt + clippy -D warnings clean; PTY shape smoke
  (scripts/quasar_smoke.py: the dark cloud leaves the core's home
  empty, the steady engine draws the core + beams there); 10 s
  A/B bench on four probes with zero visual regression
  (benchmark/bench-labs/night_research8_quasar/QUASAR_AB.md);
  the quasar itself: 96,350 fps, 84.7 dirty cells/frame.

### feature: NIGHT-research-7 (part 3) — the DNA genesis, the molecule is born before it stands

- Owner round 9/10 feedback: the style entered with the molecule
  already standing — the birth of the genome should be the entry's
  first act. The entry now replays the genesis, the
  molecular-origin story in four continuous phases (the black
  hole's `begin_formation` contract: a pure resize keeps the
  steady state, a scene entry re-forms; the bench fast-forwards —
  one-shot choreography is not steady-state throughput, the Z-6
  critical-path contract). Derived and documented as law 0 in
  `src/engine/cosmic_dragon_engine/cloud/type_rain/dna_helix/mod.rs`,
  the phase math split into `dna_helix/genesis.rs`.
- The soup: the primordial nucleotide rain falls alone for ~1.8
  sim-s (no molecule cell draws; the spawn dial runs its genesis
  multiplier — the broth IS the scene while the molecule is
  absent, then thins through the floor expiry as the genome takes
  over).
- The ladder: the axis spine appears at zero radius and splits
  into the two strands (cubic ease-out, the legibility floor
  lifted through the growth) while the assembly wave writes rungs
  top-down — each crossed rung stamped to max charge with a rolled
  pair, the fork's fresh-write economy borrowed for the birth (the
  genome writes itself into existence, its trail of light decaying
  under law 3 as the wave travels). The rotation is held through
  the window so the flat ladder stays face-on.
- The windup: the twist zips in from the top — above the front the
  strands carry the full steady law, below it the flat extension
  holds the front's angle (the wound top drags the flat tail
  around the axis, the physical read of winding a ribbon from one
  end). At the windup's end the geometry evaluates exactly to the
  steady law — the final front is the full height, no seam, no
  pop. The rotation and the replication fork's clock both resume
  only on the completed genome (no replication before the genome
  exists).
- Hunt-find (the owner's mandate, found and fixed): the shipped
  nucleotide pool STARVED — an absorbed drop never decremented
  the active counter, so every absorption permanently ate one
  unit of the spawn budget; the gate compared the inflated count
  and over a long session the soup decayed to a silent sky (the
  shipped spawn-rate calibration was tuned under the leak's
  cover — `DNA_SPAWN_RATE_MULT` 0.30 -> 0.60 now actually holds
  the lane target the calm-sky dial promises). One counter line
  and one dial; the pool recycles as its own doc always claimed.
- 17 new genesis contracts (tests_dna_helix/genesis.rs: the
  timeline classifier, the clock clamp + one-shot flag, the soup's
  draw/absorption gates, the top-down materialization with fresh
  light, the flat face-on ladder geometry, the windup's exact
  seam identity, the steady-law handoff, the held rotation, the
  fork gate, the fast-forward, the thick-then-thin broth, the
  re-entry replay, the resize keeps-steady, the bench
  fast-forward, the per-frame bounds sweep, the fronts'
  monotonicity, the assembly charge clamp); the two geometry tests
  now pin phase-0 crossings (the face-on birth presentation
  carries a different crossing alignment). Full suite: 2630
  passed / 0 failed (was 2613 — +17). PTY smoke
  (scripts/genesis_smoke.py, the ansi_screen reconstructor): the
  shape signature holds on screen — soup 10 scattered cells, flat
  ladder 90 cells with two straight full-height columns, wound
  steady helix 168 cells with the columns dissolved.
- A/B 10 s (`benchmark/bench-labs/night_research7_dna/genesis/`):
  cinematic 460.0->458.1 dirty / 5.170->5.163 entropy /
  0.638->0.639 gini; aeolian 46.0->45.9 / 4.982->4.977 / 0.645->
  0.646; solar_flare 247.8->249.4 / 6.025->6.050 / 0.325->0.318 —
  zero visual regression, fps deltas (+0.6/-0.1/-0.5%) inside the
  interleaved-run noise band. The dna_helix scene's own profile:
  89,322 fps / 108.0 dirty / 4.971 entropy / 0.672 gini — the
  deltas from A (96,906 / 101.9 / 4.860 / 0.691) are the
  starvation fix working (the soup now sustains its ~11-drop dial
  target instead of decaying inside the window: +6 dirty cells of
  living rain, and the spread light reads as +0.111 entropy /
  -0.019 gini), not a render regression (see GENESIS_AB.md).

### feature: NIGHT-research-7 — the murmuration, the twelfth rain style (the rain is a flock)

- The owner's DeepSeek-researched shortlist, second pick: `murmuration`
  — a Reynolds 1987 boids flock over a spatial hash, with the
  roaming anchor, the breathing cohesion and the clocked predator
  startle. The five laws of the flock are derived in
  `src/engine/cosmic_dragon_engine/cloud/type_rain/murmuration/mod.rs`;
  the calibration ships in `central_control_rains/style_rain.rs`.
- Law 1 (the three forces): separation (inverse-distance push, the
  3-cell minimum spacing IS the bird density), alignment (the mean
  heading match that makes a hundred strokes read as one body),
  cohesion (the weak spring to the local centroid) — semi-implicit
  Euler with the [7, 26] cells/sim-s flight band and a clamped
  jitter walk.
- Law 2 (the neighbor window): the 8-cell radius IS the hash
  bucket size; each bird scans its 3x3 buckets — O(n) pair checks
  (the enabling engineering: 100-220 birds at the cost of a
  30-bird naive scan), the window sized to the starling
  topological number ~7.
- Law 3 (the thought): a roaming anchor (weak attraction, the
  macro travel) + the wall banking (soft inward steering in the
  9-cell margin — birds curve along the edges, never hit them).
- Law 4 (the breathing): the cohesion weight cycles on a slow
  sine — the signature tighten/loosen shape cycles, emergent from
  one scalar.
- Law 5 (the startle): a clocked predator (one Core-bright glyph
  for 0.8 sim-s) kicks every bird inside the 14-cell panic radius
  outward — the flock blooms apart, floors near max, re-gathers.
- Scene `murmuration` at cycle position 13: gold palette
  (previously unclaimed — starlings catching the last sun) +
  minimal charset (the nabla flying-V). Speed 18, density 0.55
  (the flock-size dial — the flock IS the scene), glitch none.
- Full structured-family contract (lane pool, accumulator spawn
  as the staggered entry, one sim clock, palette adoption,
  generation-tagged diff cleanup, all six dispatch chains, the
  live density dial re-sizing the pool through the spawn pass).
- 27 behavior contracts (tests_murmuration): the flight band,
  separation/cohesion/banking physics, the startle saturation,
  the hash window, the anchor roam, the emergent coherence
  (radius of gyration bounded) and separation (closest pair
  bounded), the breathing band, the scatter-and-regather cycle,
  drawn bounds, repaint, pause, transitions, speed scaling,
  sustained boundedness, degenerate terminals. 2613 tests pass
  (+27); clippy -D warnings clean.
- A/B 10 s benchmarks (benchmark/bench-labs/night_research7_murm/):
  zero visual regression on cinematic + aeolian + dna_helix (the
  probes; metrics identical to the third decimal), the new scene's
  own profile at 24.9K fps / 144.5 dirty cells / 4.841 entropy /
  0.685 gini — honest physics cost (every frame integrates every
  bird; the sorgonemous band) and a concentrated flock
  composition.

### feature: NIGHT-research-7 — the DNA helix, the eleventh rain style (the rain writes the genome)

- The owner's DeepSeek-researched shortlist, first pick (the second
  lands as the murmuration): `dna_helix` — a rotating double helix
  of glyph strands spanned by Watson-Crick base-pair rungs, fed by
  a nucleotide soup, periodically swept by a replication fork. The
  five laws of the ladder are derived in
  `src/engine/cosmic_dragon_engine/cloud/type_rain/dna_helix/mod.rs`;
  the executable calibration ships in `central_control_rains/style_rain.rs`.
- Law 1 (the turn): theta(y) = phi + y k with one full turn every
  22 lines — 11 base pairs per turn, B-DNA's 10.5 honored at
  terminal legibility; the molecule rotates as one body on the
  family clock, and the X crossings (the projection of one strand
  passing in front of the other) drift down the screen for free.
- Law 2 (the pairing): a rung every 2 lines carries a Watson-Crick
  pair; the rung ends show the bases (A/T/G/C — semantic identity
  glyphs that survive charset switches, the dragon-head
  precedent), dashed bond glyphs span between them, and every cell
  interpolates the strand depths for the 3D read (front half one
  rung brighter, back half dimmer).
- Law 3 (the recency): each rung carries a hard-clamped synthesis
  charge that decays exponentially — the light shows where the
  genome has been recently written (the corona's deposition
  economy, on a ladder instead of a star).
- Law 4 (the replication): a clocked fork sweeps top-to-bottom —
  dissolving the rungs in its Gaussian window, bowing the strands
  apart (the Y), re-synthesizing fresh pairs behind itself with the
  pair re-rolled (a visible mutation), and trailing a brightness
  wake down the molecule.
- Law 5 (the soup): free nucleotides fall in the capture band
  (terminal velocity + clamped brownian drift); a drop crossing a
  rung line inside the rung's span is absorbed — the charge
  deposits and, on the mutation chance, the pair re-rolls: the rain
  visibly edits the genome it lands on. Calm-sky dial (the
  molecule is the hero).
- Scene `dna_helix` at cycle position 12: neptune palette (the
  iconic deep azure — the classic DNA-illustration blue) + the dna
  charset (A/C/G/T bases, shipped since the charset catalog's dna
  preset — the scene finally gives it its flagship). Speed 14,
  density 0.50, glitch none (the mutation is the drama).
- Full structured-family contract: lane pool, accumulator spawn,
  one sim clock, palette adoption, drawn-cell diff cleanup
  (generation-tagged), style transition arms in all six dispatch
  chains (spawn / advance / draw / semantic / force-draw /
  charset-palette clears).
- 25 behavior contracts (tests_dna_helix): rung registry, rotation
  uniformity, strand mirror + crossings, span breathing, Watson-
  Crick complementarity, bounded charge, the fork travel / dissolve
  / re-synthesis / mutation / bow envelope / re-arm cycle, spawn
  dial, monotone fall, absorption + mutation through the pipeline,
  drawn bounds, repaint without residue, pause freeze, style
  round-trip, speed scaling, sustained boundedness, degenerate
  narrow terminals. 2586 tests pass (+25); clippy -D warnings
  clean.
- A/B 10 s benchmarks (benchmark/bench-labs/night_research7_dna/):
  zero visual regression on cinematic + aeolian + solar_flare (the
  three probes; dirty cells, entropy and gini identical to the
  third decimal — the fps deltas +2.3/+1.5/-1.0% are LTO code
  layout, the bench path adds zero work to the probe scenes), the
  new scene's own profile at 96.6K fps / 102.0 dirty cells /
  4.859 entropy / 0.692 gini — the second-fastest structured
  style after the aeolian, and a concentrated composition (a
  single centered body with empty sky, the black-hole read —
  honest to the single-molecule architecture).

### stability: NIGHT-hunter-25 — the too_many_arguments census, part 1: four stale allows deleted, two cold-path signatures bundled

- Census audit: of the 12 remaining `#[allow(clippy::too_many_arguments)]`
  suppressions, FOUR were already dead (clippy -D warnings passes with
  them deleted): `evaluate_triggers` dropped to 6 args in the v30
  dragon-egg hunt but kept its allow; `CfgInputs` carries one on a
  STRUCT declaration (the lint never fires on structs — a leftover
  from the pre-refactor function it replaced) while `build_cloud_cfg`
  itself now takes one parameter; `Cloud::new` sits at exactly 7 (the
  lint fires above 7). Stale allows are their own defect class: they
  advertise signature debt that no longer exists and silently mask
  future parameter growth.
- Cold-path refactor: `run_verbose_startup` (25 positional parameters,
  same-typed neighbor hazards — two f32 densities, three u16 glitch
  bounds) bundled into `VerboseInputs<'a>` (the CfgInputs pattern).
  En route the dead `custom_palette` param — never read by the body,
  `allow(unused_variables)` band-aid — is dropped; the dump prints
  the palette NAME and BG, both still carried.
- Cold-path refactor: `format_backpressure_section` (11 positionals
  with cross-wirable f64/f32 pairs) bundled into
  `BackpressureStats<'a>`; both call sites construct named fields.
  The old "struct would be overkill" comment predated the 11th
  parameter.
- 2561 tests pass (pure signature refactor, no count change);
  check-all -q exit 0; gate-keepers 10/10; main.rs trimmed to 799 LOC
  after the named-field construction (same precedent as hunter-23).
  PTY smoke: `--verbose` and `--perf-stats` both render through the
  new structs, exit 0. A/B 10 s benches: performance-neutral (after
  side measured faster — noise; the bench loop executes none of the
  changed code).
- Remaining census (6, all hot per-frame paths, next hunt): shaders
  `resolve_cell_color` + render `get_attr` (one bundle design fixes
  both), solar `draw_solar_cell`, intro `render_particle_cell`,
  `post_rain_processing`, bench `emit_cell_lean`.

### stability: NIGHT-hunter-24 — the colors-custom load contract now enforced by every validation surface (hidden split-verdict defect)

The `--testconf`/validation-layer sweep (the F-23-1 drift species'
habitat). One hidden defect, three same-species cleanups.

- Hidden defect fixed: the runtime palette constructor
  (`to_palette`) requires at least 2 parseable rain stops and
  hard-errors below that — but no validation surface ever checked the
  count. A bg-only / single-stop / empty-array `[colors-custom.<name>]`
  block passed `--testconf` (PASS — config is valid) and then: died at
  startup for `--colors-custom`/`color =` (raw to_palette error after
  validation had blessed the file), silently fell back to the brand
  palette for `intro-color =`, and silently no-opped on live reload
  and scene-runtime ambient (debug trace only, HUD never moved). One
  config, four verdicts — the F-23-1 multi-surface disagreement at the
  value boundary. New single-source contract helpers
  (`colors_custom_load_error` + `validate_colors_custom_blocks`) make
  the validation layer ask the loader itself, so the two cannot drift
  apart: all three surfaces (startup exit 2, live-reload watcher
  rejection, `--testconf` FAIL) now reject with the runtime's own
  message. The block-level gate is the colors-custom analogue of the
  scene-custom completeness mandate: a defined block must be able to
  build a palette, referenced or not.
- Elegance fix: the any-of-3 key probe (bg || rain || stops
  contains_key) was hand-copied three times in
  `validate_field_value_with_cfg` — the same duplication shape that
  caused F-23-1. All three branches now use the canonical
  `is_colors_custom_name` + the load-contract check.
- Twin-predicate seed removed: config_hints'
  `is_valid_colors_custom_field_str` (hand-written mirror of
  configfile's private `is_valid_colors_custom_field`, "kept in sync
  via tests") is deleted; the canonical function is promoted to
  `pub(crate)` and shared.
- Docs: the colors_custom module doc claimed `ambient.22-00 =
  <palette>` as a use form — ambient keys name a scene, never a
  palette directly. Stale example corrected.
- Tests: +10 (3 inline contract-helper tests, 7 validation-surface
  tests including a flipped drift-pin — the old
  `only_bg_field_still_accepted` test had pinned the defect).
  2561 total. A/B 10 s benches: zero visual regression, fps deltas
  inside the noise band with interleaved runs (bench path bypasses
  validation entirely).

### stability: NIGHT-hunter-23 — intro-color gate read the wrong key (hidden defect); watcher 8-param signature bundled

The post-exit/final-state + live-reload sweep. Two findings: one
hidden defect (end-to-end repro), one flow-elegance fix.

- Hidden defect fixed: both intro-color validation gates (startup
  hard-error in config_apply, live-reload soft-fail clear) probed
  `colors-custom.{name}.bg` — bg is OPTIONAL in the palette schema and
  the probe was case-sensitive against parser-lowercased keys. A valid
  rain-only palette was a hard startup error for `intro-color = mine`
  while `color = mine` loaded the same palette fine from the same
  file; `intro-color = MINE` failed the same way; a mid-run switch to
  a rain-only palette was silently cleared. `--testconf` said the
  config was valid (its own probe is any-of-3 fields, lowercased) —
  three surfaces disagreed. Both gates now use the canonical
  is_colors_custom_name helper (the same one the `color =` gate uses).
- Elegance fix: the watcher family's last too_many_arguments allow
  carried the watched file TWICE (an &Arc<PathBuf> for the event
  filter + a &Path for the snapshot/read — same file, two views);
  bundled into a WatchSession struct, 8 params -> 2, handler made
  module-private.
- Verification: 2551 tests pass (+5 gate tests); live-reload PTY e2e
  (mid-run intro-color switch to a rain-only palette applied, honestly
  diffed at exit: "mine2" (was "mine")); 10s A/B pro benches —
  monolith visual metrics identical to the third decimal, cinematic
  inside the documented noise band. Full report:
  docs/research/NIGHT_HUNTER_23_INTRO_COLOR_GATE.md.

### stability: NIGHT-hunter-22 (F2) — duration dual-field deleted; duration_s is the single source of truth

The NIGHT-hunter-3 flow audit's wart F2: CloudConfig carried TWO
same-typed duration fields one line apart — the raw `duration`
(verbatim args.duration) and the validated `duration_s` — and the
event loop's end-time computation read one as the trigger then
overrode the value from the other via unwrap_or (dead defensiveness
today, a cross-wire bug the day one field gains another writer).

- CloudConfig.duration (the raw twin) is deleted; duration_s — the
  value main.rs validated (finite, 0.1..=86400, or the 0
  run-forever sentinel) — is the single source of truth, and the
  end_time derivation is a single-source filter+map.
- 12 literal sites updated (1 source fixture + 11 test fixtures);
  3 new source-text contract tests pin the deletion so the twin
  cannot silently return.
- Verification: 2546 tests pass; timed PTY smoke (--duration 0.6
  exits at ~630 ms, no-duration control runs forever); 10s A/B pro
  benches — monolith visual metrics identical to the third decimal,
  cinematic inside the documented noise band. Full report:
  docs/research/NIGHT_HUNTER_22_F2_DURATION_DUAL_FIELD.md.

### stability: NIGHT-hunter-22 (F1) — 9x startup config parse memoized; intro palette path divergence fixed

The NIGHT-hunter-3 flow audit's wart F1: a normal startup re-read and
re-parsed the config file 9 times (11 under --verbose) — and two of
the nine sites had drifted onto DIFFERENT files, hiding a real defect.

- New src/config/configfile_load.rs: the loader family (capped read +
  /etc fallback) moved out of configfile.rs (800-LOC cap) with a
  startup-parse memo keyed by resolved path — one disk read + one
  parse for the whole startup; the live-reload watcher bypasses the
  memo (mid-run edits unaffected); poisoned lock degrades to a fresh
  parse.
- Hidden bug fixed: the intro's custom palette loaded from the
  DEFAULT config path while validation read `--config` — so
  `cosmostrix --config custom.toml --intro-color <custom>` validated
  fine, then silently fell back to the brand intro. The intro now
  reads the ACTIVE config path (cfg.config_path_for_watcher) via the
  extracted, testable `intro_custom_palette` helper — and hits the
  same memo entry validation used.
- Verification: 2543 tests pass (+4: three memo contract tests, one
  intro palette path regression); PTY startup smoke on the real
  binary (config + verbose + post-exit chain) green; 10s A/B pro
  benches — monolith visual metrics identical to the third decimal,
  cinematic inside the documented noise band. Full report:
  docs/research/NIGHT_HUNTER_22_F1_CONFIG_PARSE.md.

### stability: NIGHT-hunter-22 — post-exit printer value-structs; the interactive family reaches zero suppressions

The three remaining `too_many_arguments` suppressions (the post-exit
verbose printers NIGHT-hunter-21 left out of scope) are resolved with
value-struct bundling — the last wart of the signature family.

- New `SessionState` value struct (src/interactive/final_state.rs):
  one point-in-time snapshot of all 23 live-reload-able fields with
  two constructors — `from_startup` (the resolution the session
  launched with) and `from_live` (the effective state at loop exit,
  read from the live Cloud). `set_final_state` 25 params → 1;
  `print_final_runtime_state` 26 params → 2 (snapshot + start
  Instant).
- `TerminalIoStats` bundle in event_loop_finalize.rs: the seven
  unnamed positional `u64` counters (encoding + tier2 tuples) became
  named fields captured once before the terminal drop.
  `print_perf_report` 10 params → 4.
- The final-state family moved out of interactive/mod.rs into
  final_state.rs (794 → 175 LOC); every historical call path keeps
  resolving via facade re-exports.
- Bonus warts closed on the same surface: the duplicated
  color-tune label format (two hand-rolled copies) became one shared
  helper; `print_post_exit_verbose` dropped its `args` parameter
  (scene now read from `cloud_cfg.scene_name`, the same resolution
  the loop launched with — the old re-derivation from `args.scene`
  was a duplicate source); the private `FINAL_GLIITCH_LEVEL` typo
  fixed; the v50 accessors gained the default + round-trip coverage
  they never had (full 23-field storage mapping pinned).
- Pre-existing gatekeeper debt fixed in passing: hunter-21's
  event_loop_ctx.rs doc comments carried bold/italic markdown
  emphasis, violating the 2026-09-04 comment-style rule (4 markers;
  rewritten as plain prose so the gatekeeper is green again).
- Verification: 2539 tests pass (+3 new SessionState mapping tests);
  clippy -D warnings clean; gate-keepers 10/10; 10 s A/B benches:
  monolith visual metrics identical to the third decimal, cinematic
  inside the documented noise band (bench path structurally excludes
  the post-exit family). Full report:
  docs/research/NIGHT_HUNTER_22_FINAL_STATE_VALUES.md.

### stability: NIGHT-hunter-21 — wart #3 resolved: event-loop context-struct refactor (owner mandate)

The NIGHT-hunter-3 flow audit cataloged the rain loop's coupled
mutable state as the largest structural debt ("the context-struct
refactor is the biggest candidate for the next stability gain"). This
hunt executed it: ~45 loop-state locals became one `LoopCtx`
(src/interactive/event_loop_ctx.rs) with domain sub-structs (scene
identity, config layers, ambient state, perf counters, per-frame
observation), and the sibling signatures collapsed accordingly.

- Stability gain, concretely: the old positional lists carried
  same-typed pairs and quadruples the compiler could not guard —
  `base_cfg`/`startup_cfg`/`current_cfg`/`cfg` (four CloudConfig refs
  in apply_config_rebuild), `charset_preset`/`scene_name` (&mut
  String pair), 12 perf accumulators (f64 trio among them), `w`/`h`.
  A transposed call compiled and silently routed state to the wrong
  layer. Every hazard is a named field now — the compiler rejects
  what it previously could not see. The duplicate
  `cfg`/`startup_cfg` threading (content-identical clones) collapsed
  into the single `ctx.config.startup` layer.
- Signature conversions: apply_config_rebuild 23 → 1 param;
  poll_ambient_events 21 → 1; update_perf_stats 22 → 2; and
  sample_p5_health / handle_resize / run_adaptive_throttle /
  try_auto_snapback / revert_ambient_owned_scene / apply_ambient_fps
  similarly. run_self_healer keeps granular distinct-typed mutables +
  a named HealInputs struct (test ergonomics preserved).
  `too_many_arguments` suppressions in the family: 13 → 3 (the
  remainder are post-exit verbose printers, out of scope).
- event_loop.rs: 924 → 795 LOC with the LOC_EXEMPT REMOVED — under
  the 800 cap without exemption for the first time since the v50
  split.
- Verification: 2536 tests pass; clippy -D warnings clean; PTY e2e on
  the refactored loop all green (HUD row order 25/25, hunter-20
  long-scene case, canonical live-reload scene switch mid-rain);
  10 s A/B benches within the documented noise band with identical
  monolith visual metrics. Full report:
  docs/research/NIGHT_HUNTER_21_CTX_REFACTOR.md.

### ux: NIGHT-hunter-20 — HUD 64-column minimum usable width (owner mandate)

Loading a scene with a long name hard-cut the `scn:` HUD metric line:
`scn: example_1234_test_this_long` displayed as `scn: example_1234_t`
(14-char setter truncation designed around the old 24-col HUD width
cap, with the chroma border column landing exactly at the cut so it
read as "hardcut by border"). The owner mandated a minimum usable HUD
width of 64 characters.

- HUD_MAX_WIDTH 24 → 64 (`src/interactive/hud/mod.rs`); the dynamic
  width, padding, and chroma border all track it unchanged, and
  terminals narrower than the HUD keep their graceful degradation.
- Identity-line truncation raised 14 → 58 chars
  (HUD_IDENTITY_VALUE_MAX_CHARS = 64 − 6-char label prefixes) for
  `scn:` (scene) and `chr:` (charset) — the owner's 27-char example
  now renders in full.
- Latent sibling closed: `clr:` custom palette names had NO
  truncation, so a long name pushed its line past the width cap and
  the border landed mid-text (the only genuine border-clip). All
  three identity setters now share the 58-char budget, so no HUD line
  can exceed the cap.
- E2E verification hardening: new `scripts/ansi_screen.py` (mini ANSI
  screen reconstructor) + new `scripts/hud_long_scene_e2e.py` (owner
  case: full name on reconstructed screen row 8, border past text);
  `scripts/hud_order_e2e.py` repaired — it asserted label order in the
  RAW ANSI stream, which is invalid when the differential renderer
  paints one HUD toggle across multiple frame flushes (pre-existing
  red on baseline b8efb15, now green via screen-row assertions).
- 2 new + 1 updated unit test; 10 s A/B bench clean (visual metrics
  identical to the third decimal on monolith; cinematic inside the
  documented noise band; HudState is never constructed on the bench
  path). Full report: docs/research/NIGHT_HUNTER_20_HUD_WIDTH.md.

### stability: NIGHT-hunter-4 — panic-hook worker containment + phosphor full-grid scan guard (two hidden defects, root-caused and pinned)

Depth audit for hidden bugs and premature logic (owner hunt
2026-09-08). Two genuine defects surfaced, both fixed with regression
tests; the full verified-safe catalog is in
docs/research/NIGHT_HUNTER_4_BUG_HUNT.md so the next hunt does not
re-till this ground.

- Panic hook (src/platform/panic_hook.rs): Rust runs the global hook
  BEFORE unwinding, including for panics a worker thread's
  catch_unwind is about to catch and recover from. The old hook
  therefore restored the terminal MID-RAIN on a caught watcher/poller/
  ambient panic (alt screen left while the main loop kept rendering)
  and armed TERMINAL_RESTORED_BY_PANIC — a flag with one store site
  and no reset — so the final Terminal::drop skipped cleanup and
  leaked raw mode / the alt screen at exit. The hook now captures the
  installing (main) thread's id and performs teardown only for
  main-thread panics (the only ones that escape to process death);
  worker panics keep their designed recovery (catch_unwind + AB-10
  buffered diagnostics + poller restart). 3 tests pin the contract.
- Phosphor full-grid scan (cloud/phosphor.rs): the
  semantic-invalidation branch (dirty_all + empty dirty list, the
  clear_with_bg path) iterated CLOUD dimensions but indexed FRAME
  buffers with direct indexing — unguarded, while the sibling
  dirty-index branch has guarded the mirror divergence since HUNT-25.
  Not reachable today (all construction sites pair the dimensions),
  but one refactor away from a per-frame panic. Both loops now guard
  frame bounds with one compare per row / column-leading-cell
  (monotonic break). 4 tests pin the tolerance plus the paired-dims
  control.

A/B 10 s benches (benchmark/bench-labs/night_hunter4/): zero visual
regression — entropy, gini, and dirty-cell populations identical to
the third decimal on cinematic (the phosphor path) and monolith
(structured-family control); fps deltas +0.23% / +1.11% are inside
the documented same-tree noise band.

### audit: NIGHT-hunter-3 — full-process flow audit (start to end): master flow confirmed, three warts cataloged

Owner suspicion after a hidden-bug fix: premature flow, spaghetti
risk. Answer: master flow, not spaghetti — one entry funnel (main),
one state owner per concern, one exit funnel (finalize_session +
Terminal::drop), idempotent defense layers, and a linear
start-to-end pipeline traceable in one pass
(docs/research/NIGHT_HUNTER_3_FLOW_AUDIT.md carries the full flow
map and per-segment verdicts). Three warts cataloged for future
work, all previously acknowledged in-tree or quantified as
negligible by this audit: config.toml is parsed 9x per interactive
startup (~1 ms total, freshness benefit documented),
CloudConfig carries a redundant duration/duration_s dual field
(values coincide by construction today), and event_loop.rs's
20-sibling-module mutable-state coupling (its own LOC_EXEMPT
documents the context-struct refactor prerequisite). No behavior
changed in this task.

### feature: NIGHT-special-4 — the solar flare rain style + solar_flare scene (the rain rides the magnetism); the aurora veil retired

Owner verdict (2026-09-08): the aurora veil rated 5/10 and retired
at the owner's direction — "remove it and change it to solar flare
(corona loops magnetic)". Answered by building its successor in the
same slot: the tenth style stays an original-math flagship (the
NIGHT-special-2 invention directive carried forward — motion DNA
with no existing mathematical reference), now as a corona arcade
instead of a polar veil. The invented system ("the five laws of the
corona", fully derived and documented in
src/engine/cosmic_dragon_engine/cloud/type_rain/solar_flare/mod.rs):

- Law 1, the magnetic carpet: coronal loops (one per ~10 columns)
  root at two footpoints on a granulated photosphere; facing feet
  repel with an inverse-gap force, spans and apex heights breathe
  toward re-rolled anchors on rolled dwells, and the whole arcade
  drifts on a slow global wind. The lifecycle is the carpet's
  turnover — Emerging (the arc grows out of the surface), Stable,
  Erupting, Detaching (the lifted arc rises and dissolves), then
  re-Emerging elsewhere.
- Law 2, the coronal condensation: drops condense near loop tops
  (weighted toward hot loops by tournament selection) and ride the
  legs down with a CLOSED-FORM energy-conserving speed
  v = sqrt(v0^2 + 2 g h (2|s-0.5|)^2) — bounded by construction,
  exact at any dt, monotonically accelerating; the landing is a
  state test on monotone s-motion (no tunneling at any dt, any
  frame rate).
- Law 3, the footpoint deposition: landings charge the loop's flux
  (exponential cooling + hard clamp) and flash the footpoint cells
  — the light in this sky is where the rain has been landing, the
  aurora funnel's arcade heir.
- Law 4, the flare eruption: a loop whose flux crosses the
  threshold destabilizes when the global flare clock allows it (one
  flare at a time — a singular event): the apex stretches ~2x, the
  riders are flung as ballistic ejecta (tangent fling + upward
  kick), a burst sprays from the apex, then the arc lifts off and
  dissolves while a fresh loop emerges. Ejecta fly ballistic arcs
  under stellar gravity and splash heat back into the granules
  they land on.
- Law 5, the shimmer law: field cells (arc filaments, granulation)
  keep the glyph the frame already carries; re-rolls ladder with
  heat — the quiet corona shimmers rarely, the flare flickers hard.

Stability is by construction, not by tuning: every state variable
is hard-bounded (drift/span/height clamps with damped wall
reflection, hard-capped flux, the closed-form speed bound,
age-capped ejecta with wall + surface kills, lane-bounded pool
with a lifetime backstop). Scene: sun palette (the real-color
golden-orange photosphere ramp) + greek charset, cycle position 11
(replacing the retired aurora), the calm-sky weather dial (sparse
coronal rain, the arcade the hero). 22 behavior contracts (carpet
spread, wall bounds, width breath, bounded flux, the flare ladder,
the flare gate, the eruption cycle + ejecta, spawn target, the
energy-conserving descent, the closed-form speed bound, drawn
bounds, diff cleanup, pause freeze, style transitions, speed
scaling, sustained boundedness). One real bug the contracts caught
before ship: the wall clamp's upper bound let a loop's right
footpoint sit one cell past the last column (cx <= cols - margin
admits foot_right = cols); pinned by law1_loops_stay_inside_the_
viewport, fixed to cols - 1 - margin. LOC discipline: the draw
pass split into type_rain/solar_flare/draw.rs at the 800-line hard
cap (the monolith family's monolith_glyphs.rs split pattern).
benchmark/bench-labs/night_special4/AB_REPORT.md carries the 10 s
A/B: zero regression on the cinematic + sorgonemous + aeolian
probes, plus the solar_flare profile.

### feature: NIGHT-special-3 — the aurora rain style + aurora scene (the rain paints the light)

Owner request (2026-09-08, the aeolian's 10/10): "what else can you
do — can an aurora rain type be done?" Answered by building it: the
tenth style, the second original-math flagship (the NIGHT-special-2
invention directive carried forward — motion DNA with no existing
mathematical reference). The invented system ("the five laws of the
polar veil", fully derived and documented in
src/engine/cosmic_dragon_engine/cloud/type_rain/aurora/mod.rs):

- Law 1, the ray lattice: ray beads (one per ~6 columns) drift on a
  global wind (target re-rolled every few seconds, eased toward
  exponentially) while adjacent pairs repel with an inverse-gap
  force — the veil spreads into organic, uneven coverage and
  advects slowly across the sky. Velocities clamp, positions clamp
  with damped wall bounces.
- Law 2, the substorm breath: each bead's emission depth glides
  exponentially toward a private anchor that flips between two
  DISJOINT bands (24-34% and 46-60% of the viewport height) on
  rolled dwell intervals — the quantized two-band choice makes the
  veil read as curtains hanging at distinct altitudes (the layered
  aurora), never a uniform mush; an anchor flip resolves over ~1 s
  (the curtain visibly descends or retreats).
- Law 3, the precipitation funnel: falling drops within seek range
  of the nearest ray's depth bend toward its column with a
  glow-weighted gain — bright fringes attract the weather harder,
  closing the self-organization loop (the sky concentrates its
  light where the rain has been landing).
- Law 4, the emission charge: a drop absorbed at its ray's depth
  deposits its kinetic charge into the fringe glow (exponential
  decay + hard clamp — bounded by construction). The fringe climbs
  the ladder Mid (baseline) / Hot (charged) / Core (the fresh
  landing flare window) and spills into its flanking columns one
  rung dimmer while Hot or brighter.
- Law 5, the shimmer law: curtain cells keep the glyph the frame
  already carries (the fabric identity); re-rolls ladder with
  brightness — the body shimmers rarely, the fringe often.

Stability is by construction, not by tuning: every state variable
is hard-bounded (velocity/position/depth clamps, glow cap,
lane-bounded pool with terminal velocity + lifetime backstop), and
absorption is a STATE test (drop depth >= emission depth), not a
crossing test — no tunneling at any dt, at any frame rate. Scene:
aurora palette (557.7nm green — the oxygen line the real curtains
burn on) + greek charset, cycle position 11, the calm-sky weather
dial (sparse drizzle, the veil the hero). 21 behavior contracts
(lattice spread, wall bounds, two-band breath, bounded glow,
fringe ladder, nearest-ray, spawn target, fall, funnel, absorption
charging the fringe, drawn bounds, diff cleanup, pause freeze,
style transitions, speed scaling, sustained boundedness). Scene
catalog extraction: SCENES moved to src/scene/catalog.rs (the
table outgrew mod.rs's 800-line hard cap — the RULES_LOC
sibling-file recipe, call sites unchanged via re-export).
benchmark/bench-labs/night_special3/AB_REPORT.md carries the 10 s
A/B: zero regression on the cinematic + sorgonemous + aeolian
probes, plus the aurora's own profile.

### feature: NIGHT-special-2 — the aeolian weave, the first original-math rain style + aeolian scene (the rain plays the instrument)

Owner directive (2026-09-08): a new rain type whose motion DNA uses NO
existing mathematical reference — original math derived by the
engineering AI for the terminal medium itself, the LEAP-engine
spirit. The ninth style: glyph rain falls onto invisible horizontal
strings and plucks them. The invented system ("the six laws of the
weave", fully derived and documented in
src/engine/cosmic_dragon_engine/cloud/type_rain/aeolian/mod.rs):

- Law 1, the hop-clock lattice: each channel cell carries excitation
  mass and a private phase clock; a completed clock hops the WHOLE
  mass one cell (merge on arrival, surplus carried — exact rate
  keeping). Two voices: bright mass sprints at 50 cells/s in rigid
  lockstep (zero numerical diffusion — pulses keep their shape for
  their whole lifetime), dim residue crawls at 3. Two earlier
  formulations (a Michaelis blend, a smooth bistable blend) were
  derived, tested, and rejected: both obey a max principle that
  flattens every pulse into a dim ramp within ~1.5 s (no racing
  fronts, no crossing knots) — the design history is preserved in
  the derivation essay.
- Law 2, wall reflection: hops off the screen edge re-enter the
  opposite channel at 0.85 (deferred application — even reflected
  mass moves at most one cell per tick).
- Law 3, self-similar decay: uniform exp shrink — shape-preserving
  fade.
- Law 4, the pluck: a capture injects the drop's kinetic charge
  symmetrically into both channels across a three-cell profile;
  half the captures also ring the string below (the aftershock
  cascade).
- Law 5, the capture field: bright antinodes eat rain, silent
  strings let it through — the feedback loop that self-organizes
  the weather onto the ringing zones, saturated structurally by the
  L1 bound.
- Law 6, resonance seeking: drops bend toward passing wavefronts
  and surf-kick through them (the interference streak).

Stability: a discrete L1 contraction proof (hops are conservative
transfers, decay shrinks, walls return at most what they receive)
bounds the field unconditionally at any dt — the steady state rings
exactly as loud as the rain plays it. FPS invariance wherever
rate x dt < 1; no tunneling ever (sweep order + deferred
reflections). The scene ships the calm-sky weather dial family
(stage-4 DNA: sparse drizzle, trickle cadence, dim entry) so the
instrument stays the hero. Scene: aurora palette + runic charset,
cycle position 10. 24 behavior contracts (the L1 proof's
observables, urgency, FPS invariance, reflection, knots, capture
feedback, pause/resize/transition/speed/trickle). A/B 10 s
benchmarks: zero regression on cinematic + sorgonemous_intrascals
(visual metrics identical to three decimals); the aeolian profile:
158,630 fps, 46 dirty cells/frame, entropy 4.98, gini 0.65 — the
cheapest structured style in the catalog
(benchmark/bench-labs/night_special2/AB_REPORT.md).

### feature: NIGHT-special-1 — the black hole rain style + sorgonemous_intrascals scene (stage 1: the event-horizon ball)

Owner spec (2026-09-07): a new rain style `black_hole` and a new scene
`sorgonemous_intrascals` (energy-zen palette, binary charset) — a
gravitating body instead of a particle field, staged one commit per
stage for owner visual verification. Stage 1 ships the event-horizon
ball: a centered medium ball, geometry dynamic for any screen size
(radius fractions of the viewport's limiting half-extent,
aspect-corrected cell math), a black empty core (never drawn — the
hole itself), and a photon-ring radial brightness ramp (Core band
hugging the horizon, fading outward to Ghost) with a calm surface
shimmer. Stage 2 (the RK4 orbital ring — the lorenz integrator is
attractor-agnostic and reusable) and stage 3 (the glyph infall —
falling glyphs bending elegantly into the core near the ring) follow
per the staged rollout.

Wiring follows the structured-family contract (vortex/lorenz/dragon/
physarum pattern): `RainStyle::BlackHole` (canonical label
`black_hole`, not droplet-family, no spawn-remainder at stage 1), the
`type_rain/black_hole` module (cached annulus geometry rebuilt on
reset, monolith three-pass diff cleanup, a charset-switch glyph
re-roll arm so the ball never carries stale-pool glyphs), dispatch
arms in spawn / runtime_controls / rain_at / scene_runtime /
spawn_reset, constants in style_rain.rs, and the scene entry at cycle
position 9 after physarum. Catalog grows 24 -> 25 scenes; the
scene-count pins, the sorted name list, the x-cycle order pins, the
--scene help list, the scene-custom style label lists, the
configfile dump comments, and the README style-flagship section all
updated.

Gates: cargo fmt clean, clippy 0 warnings, 2427/2427 unit tests
(2420 prior + 6 black-hole behavior contracts + 1 scene pin), build.sh
check-all green (cargo-audit skipped: not installed, same as prior
sessions), gate-keepers 10/10, comment-style 0 emphasis markers, LOC
caps respected.

### feature: NIGHT-special-1 stage 2 — the black hole orbital ring (RK4 Lorenz turbulence on a Keplerian ellipse)

Owner verified stage 1 (the ball) at 10/10 and approved the ring
stage. Motion DNA: each mote is a glyph riding a tilted ellipse
around the ball. The mean motion is Keplerian — the angular rate
scales with the mote's current wobbled radius to the minus
three-halves (Kepler's third law), so motes wobbled inward visibly
outpace ones wobbled outward, the differential rotation of a real
accretion disk. Superposed on that mean flow, the canonical Lorenz
attractor (the same sigma 10 / rho 28 / beta 8/3 system the lorenz
style renders) is integrated per mote with classical RK4: the
attractor's radial coordinate wobbles the orbital radius, its z
displaces the mote out of the disk plane and grades the glyph
brightness through the shared z ladder. Far-side motes passing
inside the ball silhouette are occluded (the hole hides them);
near-side motes cross in front of the annulus and the empty core —
the tilted-disk 3D layering read of the iconic imagery.

Engineering: the per-mote physics lives in
`type_rain/black_hole/ring.rs` (RingMote + RK4 + projection +
occlusion, split from the ball file the way monolith/dragon split
their helpers); `black_hole.rs` owns the mote pool, the spawn
accumulator (deficit-bounded + fractional remainder — BlackHole now
returns true from `uses_spawn_remainder`), the advance clock
(dt-clamp + resume_blend, same as the structured siblings), and the
draw pass (heads + four-cell comet trails, far-side cells skipped,
all drawn cells flowing through the existing three-pass diff
cleanup — now load-bearing as motes vacate cells while orbiting).
Comet trails dim through the family ladder; the motion-gated
shimmer, palette-slot adoption per mote, and lifetime absorption
(14s ± 15%) all follow the structured-family contracts. 19 ring
constants in style_rain.rs (band radii, tilt, Kepler exponent,
attractor normalization, spawn/active ratios), every radius a
multiple of the ball outer radius so the ring scales with any
screen size.

Tests: 6 new ring behavior contracts (spawn + strict orbital
advance per mote, band + viewport bounds, far-side occlusion
cross-checked against the drawn-cell set, style-transition recycle,
lap-pace sanity, RK4 stability regime) plus a compile-time Kepler
exponent pin; the stage-1 core contracts updated for the mote layer
(drawn cells and active count now cover the ball plus the ring).

### feature: NIGHT-special-1 stage 2.1 — owner-feedback iteration: the wide Gargantua disk, the lensing halo, and the co-rotating hole

Owner verified stage 2 at 8/10 with three directional notes, all
validated against the real imagery (EHT M87*, Gargantua): the disk
must read LONG left-to-right of the shadow, particles approaching
the hole's edge must curve UP over the top, and the hole itself must
visibly rotate in sync with the ring (it read as static).

The wide disk: the projection's ellipse is re-proportioned — the
semi-major axis is now a fraction of the viewport unit (clamped to
92% of the half-width so extremes never clip on narrow terminals),
reaching about twice the ball's radius left and right, with a thin
near edge-on semi-minor axis. The old ring-radius/tilt constants are
retired; the Kepler shear input tracks the new a/a_mean.

The lensing halo: the far side no longer hides flat behind the hole
— gravitational lensing bends it over the top. Far-side motes blend
onto a circular halo arc (radius 1.30x the ball, aspect-corrected
round on screen) whose apex sits just above the photon ring; the
blend is smooth in backness so the stream climbs continuously from
the limb, vanishes briefly behind the silhouette, re-emerges on the
upper arc and sails over the top — the iconic halo read.

The co-rotating hole: the ball's rim now spins on the same clock and
the same mean omega as the ring (SPIN_RATE 1.0 = lockstep). Two
coupled mechanisms make it visible: a glyph conveyor (the rim's
pattern is bucketed by angle and the buckets slide around the
annulus with the spin phase — motion-gated deterministic re-rolls,
so the binary charset reads as a circulating 0/1 stream) and a
Doppler-style brightness lobe (cells near the rotating peak brighten
one ladder rung, near the opposite point dim one — the radial band
structure untouched, bump applied at draw time only). The spin
advances even with zero motes active (own clock, not the pool).

Entry spiral: freshly spawned motes materialize 55% beyond the disk
and settle onto it exponentially (tau 0.9 s) — accretion from
outside instead of pop-in, for both the steady-state respawn and the
upcoming formation intro.

Tests: 4 new contracts (the lens lift — apex above the ball top,
near side below center, continuity at the extremes, smooth rise;
the entry spiral's monotonic settle; the spin phase's advance and
lockstep tracking of the ring's mean omega; the band test re-based
on the new geometry with a settled-age filter), 16/16 black-hole
tests green.

### feature: NIGHT-special-1 stage 2.2 — the formation intro: the hole is born, not popped in

Owner question (stage-2 verification round): the black hole
appeared suddenly on start — should it instead follow the real
character (slow fade, small dot exploding, sudden)? Answer: the
physically evocative combination, staged as stellar collapse. A
tiny singularity seed glyph fades in slowly at the viewport center
(1.4 s, brightness ramping up the ladder — the "appearing slowly"
half of the owner's options), the collapse phase flares it to Core
and a four-cell cross around it (0.5 s — the last light of the
collapsing star), the event horizon then blooms outward from the
inside on a cubic ease-out (1.2 s — the "small dot explodes" half,
photon-ring cells appearing first and the outer rim last), and
once the hole is whole the accretion begins: the mote spawn gate
opens and fresh motes drift in on the stage-2.1 entry spiral to
settle onto the ring.

Engineering: the phase math (phase classification, timeline
constants, seed/collapse level ladders, the horizon-bloom
visibility curve) lives in `type_rain/black_hole/formation.rs`;
`black_hole.rs` owns the mutable half — the formation clock riding
the advance pass's dt-wall (pause freezes the birth mid-sequence,
resume continues it, exactly like the motes), the formed flag
gating the spawn accumulator, and the seed-dot renderer (center
glyph + collapse cross, cells flowing through the same three-pass
diff-cleanup stream so the bloom cleanly erases them). Style ENTRY
replays the sequence (`begin_formation` wired in scene_runtime);
a pure resize rebuilds the geometry but keeps the steady state —
the hole re-forms only when the scene is re-entered. The ball's
rim spin runs from frame one, so the surface is already rotating
as the horizon blooms. black_hole.rs takes an LOC_EXEMPT marker
(same call as dragon.rs's entry-reveal: the orchestrator's
draw/spawn/advance passes share the private field set; the
physics, cell helpers and phase math are already split out).

Tests: 4 new contracts (the birth sequence's drawn-cell timeline —
dot only, partial bloom, full ball; the spawn gate closing until
the formed flag flips; resize preserving the steady state; style
re-entry replaying from the singularity), the existing
core/ring contracts fast-forwarded to the steady state via a
shared harness helper, 2440/2440 suite green, fmt/clippy clean,
build.sh check-all exit 0, gate-keepers 10/10, version untouched.

### feature: NIGHT-special-1 stage 2.3 + 2.4 — the Gargantua disk pass, the proximity glow, the see-saw roll, and the three-tier Interstellar stack

Owner verification rounds (stage 2.2 9.5/10, then 9.7/10): the
disk must read like Gargantua's — dense enough to be a solid
horizontal white line through the shadow's middle with sparse
dissolving ends — then, for the last polish, particles near the
hole must burn white while the distant ones fade, the ring must
pivot like a lever (left end up, right end down, through the
vertical and back), and the disk must split into the stacked
lensed bands of the film's imagery.

Stage 2.3 (the solid-line pass): the active-mote floor rose
(0.22 -> 0.55 -> pool cap 1.0) so the steady-state target sits
near 74% of the pool and the comet trails knit the band into the
near-continuous line; the near side's sine squashed to half the
minor axis (the crossing hugs the core's vertical middle); the
occlusion rule re-keyed on the orbit side (sign of sin phi
instead of screen height — near-side cells the z-tilt lifts above
the equator now draw, far-side cells hide anywhere inside the
silhouette); and a radial brightness profile graded the heads by
horizontal orbital position (inner-zone bump, rung fade at the
extremes).

Stage 2.4 (owner 9.7/10 -> the 10/10 ask): the brightness key
moved from the orbital angle to the projected screen distance
from the hole — `proximity_level` steps two rungs up inside the
hot radius (1.32 ball radii: the crossing band across the shadow
AND the whole lensing arc land at Core — the white "head white"),
one rung in the warm belt, and the fade ladder past 1.40 radii
(the ends and the far disk dissolve into Ghost wisps; fresh
entry-spiral motes drift in dark and ignite as they settle).
Distance is rotation-invariant, so the glow rides the hole at
every roll angle. The ring pool now carries three tier bands from
the new `BLACK_HOLE_RING_TIERS` table: tier 0 the approved
equatorial main disk, tier 1 a shorter band above it across the
annulus face, tier 2 the shortest band hugging the rim — the
closest line to the hole — stacked like the film's stepped
lensed bands, with per-tier Kepler pacing (1.6x/2.4x, the inner
bands visibly race), per-tier wobble/tilt scales, and the spawn
weights 52/30/18 (the active floor raised 0.55 -> 0.75 so tier
0's share still covers the equatorial line; the 9.7 solidity
verdict must not regress). The upper tiers skip the lensing arc
and the occlusion rule (lensed images read in front at any
height); their two flow strands straddle the band center, so the
orbit reads as a thin ribbon, not a retraced line. And the whole
stack see-saws: `RingRoll`, a deterministic hash-driven schedule
(no RNG — the advance pass owns no generator), holds the flat
horizontal line ~30 s (the dominant mode), sweeps eased
smoothstep excursions at a fixed angular rate (90 degrees in
~3.7 s) from the tilt menu (90 weighted double, then 60/45/35/25)
with the sign alternating every excursion, and chains 35% of
tilts straight into the next through the rest line — the
continuous lever wave. The projection rotates every mote's
disk-plane offset by the live angle in line-height units before
the cell-aspect conversion: a true Euclidean pivot around the
hole, stack, lensing halo and breathing bands as one rigid body;
the flat mode is reset on style entry, kept across a pure resize.

Tests: the ring tree re-pinned to the new contracts — the
proximity zones (Core at the crossing and the arc, warm belt,
monotonic fade to Ghost), the tier stack (stepping upward,
descending reaches, tier 1 across the annulus face clear of the
horizon, tier 2 hugging above the rim and under the arc apex),
the per-tier Kepler pace differential, the rigid 90-degree pivot
(distance preserved, left end up), the see-saw schedule (flat
dominance, vertical bound, both directions, sustained returns to
rest, bit-exact determinism), the three-tier population through
the live spawn, and the roll engaging through the live clock —
31/31 black hole contracts, 2451/2451 suite green, fmt/clippy
clean, version untouched.

### feature: NIGHT-special-1 stage 2.5 — the snug three-tier stack and the long-dwell see-saw roll

Owner verification (stage 2.4 at 9.8/10) landed two reads plus a
language reminder: the three-tier Interstellar stack must sit
SNUG — tier 2 and tier 3 close to tier 1 and to each other (his
analogy: two objects ten meters apart should read one meter
apart, so the stack reads as a tight family of stacked lines),
the see-saw roll's long dwell must improve to a more special
30 s-or-more hold across the 15-180 degree attitude window with
exactly the 90-degree vertical attitude excluded, and every
comment, doc and commit message in the repo must stay pure
English (the stage-2.4 owner quotes embedded in comments are now
translated).

The snug stack: the tier table's center offsets drop 0.80/1.13 ->
0.22/0.42 ball outer radii (center steps 0.22/0.20), the upper
minors thin 0.08/0.05 -> 0.055/0.04 so the ribbons match the
tight spacing, and the squash/wobble/z-tilt scales trim to keep
the upper lines crisp. The whole family now crosses the shadow's
face just above the equatorial band (tier 1's strands
~0.12-0.28 outer radii above center, tier 2's ~0.35-0.45), under
the 1.30 lensing arc crown — the stacked-arcs-over-the-shadow
read of the film's imagery, no longer a sprawl up the rim.

The see-saw roll: every attitude now parks for a long hold —
36 s at the flat rest line (still the single longest pose, the
Gargantua identity) and 30 s at each tilted excursion (the
improved long duration, previously a 4.5 s punctuation). The
excursion menu re-ladders to the 15-180 degree window with the
vertical 90-degree attitude excluded: shallow 15/30/45/50-degree
tilts, the mid 60, and the steep 85 (a near-vertical diagonal
that keeps the old vertical drama without ever parking on the
excluded attitude). Sweeps stay eased smoothstep at the 0.42
rad/s rate (an 85-degree pivot resolves in ~3.5 s), the sign
still alternates, and the 35% chain chance still sweeps
tilt-to-tilt through the rest line.

Tests: the tier stack contract re-pinned to the snug geometry
(strict upward steps with the one-meter-gap bounds — tier 1's
step within 0.15-0.40 and tier 2's within 0.12-0.30 outer radii,
the stack on the shadow face below the rim, under the arc apex,
descending reaches), and the see-saw contract re-pinned to the
new schedule (menu pin — no 90-degree rung, every rung within
15-85; the 36 s flat hold; the 30 s tilt holds measured through
bit-identical parked runs; the flat line the single longest
pose; the excluded-attitude bound at every sample; both
directions; sustained returns to rest) with the live-clock
engagement test extended past the longer flat hold — 31/31
black hole contracts, suite green, fmt/clippy clean, version
untouched.

### feature: NIGHT-special-1 stage 2.6 — the one-compact-family stack and the halo streams (the doubled upward curve + the mirrored lower stream)

Owner verification (stage 2.5 at 9.9/10) landed four reads: the
snug stack still reads a notch loose — stack 1 must descend a
little from its default center position, stacks 2 and 3 must sit
ALMOST FUSED with it (his wording: the family must read dense),
stacks 1 and 2 must widen a little more; the particles curving
up over the hole must double their density; a NEW stream must
curve down under the hole — the same motion, the opposite
position, slightly fewer particles, the rotation following the
disk; and the whole style must hold the dynamic-screen-size
contract.

The one-compact-family stack: the tier table's center offsets
move to -0.08 / +0.05 / +0.15 ball outer radii (center steps
0.13 / 0.10 — the near-merged grouping where the stage-2.5
spread read a hand's width apart), and the two longest bands
widen (major scales 1.00 / 0.72 -> 1.10 / 0.80). The main disk's
mid-band now sits 0.08 outer radii below the viewport center
(the slight descent), the crossing dip deepening with it; the
upper tiers braid into the space just above the dropped
equatorial band (tier 1's strands ~0.01-0.15 outer radii above
center, tier 2's ~0.12-0.22), the whole family packing into
~0.33 outer radii of vertical span — the dense stacked-lines
read under the 1.30 lensing arc crown.

The halo streams (`halo.rs`, the fifth module of the family
split): a second mote pool (one lane per column, the family
contract) whose riders orbit the ARC CIRCLE around the shadow
instead of the flat ellipse. Each mote rides the full circle
with the ring's own motion DNA — the RK4 Lorenz step (extracted
to a shared `rk4_lorenz_step` core in ring.rs: one attractor,
one integrator, two projections), a Keplerian rate paced to the
arc radius (0.74x — the outer-lane read of Kepler's third law),
the radial coordinate wobbling the arc into a thin plasma band,
the z coordinate grading the brightness, the entry-spiral
drift-in, the comet trail, the motion-gated shimmer. The mote's
tag picks its semicircle: the upper stream co-rides the 1.30
lensing circle — its riders share the road with the far-side
lensed image, doubling the upward-curving population without
touching the ring pool's tier shares; the lower stream mirrors
the circle under the shadow, slightly sparser. The handoffs land
at the arc extremes where the lensing arc already blends into
the equatorial band (particles read as merging into the disk
line and re-emerging from the opposite limb); crossing into the
hidden half retires the trail so the re-emergence never paints
a teleporting tail. No occlusion rule (the arc band spans
1.20-1.40 outer radii, never entering the silhouette); the
see-saw roll rotates the projection with the rest of the system.

The stream split runs a deterministic Bresenham accumulator
(each activation adds the upper share's 0.56 to a fractional
budget that decides the tag), so the 0.56 / 0.44 split holds
EXACTLY on every pool fill — a random pick only holds on
average, and a small pool can land a visibly inverted split on
an unlucky seed. The halo pool's active target mirrors the ring
pool's own base and density multiplier, so the doubling read
holds proportionally at every density setting. Spawn rides the
same deficit-bounded accumulator contract (its own internal
remainder), the formation gate opens the arcs only after the
horizon bloom, and the shared clock/advance/draw passes step
both pools — the cloud layer's spawn/advance/draw arms are
untouched (the pool lives entirely inside the black hole
module). The HUD active count now includes the stream riders.

Dynamic screen size: every halo length is a fraction of the ball
outer radius (itself a fraction of the viewport's limiting
half-extent), the pool rebuilds to the new column count on
reset, and every draw bounds-checks per cell — verified across
resize transitions at 200x60, 120x40, 80x24 and 105x64.

Tests: the tier stack contract re-pinned to the near-merged
geometry (the main disk below center, steps within 0.10-0.30 and
0.04-0.20 outer radii, the whole family packing under 0.40, the
widened reach past the old span, descending lengths), the
crossing dip re-pinned with the descent, the band escape guard
now table-driven per tier, the roll pivot carrying the lowered
band's rest offset rigidly; the halo contracts pin the arc ride
(visible riders strictly on their own semicircles, in viewport,
on the 1.20-1.42 band, outside the silhouette), the doubling
(upper halo + far side past 1.5x the old figure), the exact
Bresenham split (within one mote of the ideal, lower strictly
sparser), the shared rotational sense (angles advance), and the
dynamic-screen-size resize transitions — 36/36 black hole
contracts, 2456/2456 suite green, fmt/clippy clean, LOC
exemption note extended, version untouched.

### feature: NIGHT-special-1 stage 3 — the glyph infall (the rain becomes the accretion material)

Owner green light (stage 2.7 rated 10/10 — masterpiece) opened
the final stage of the rollout: the matrix rain itself falls
into the hole. The third act gives the scene its weather: a
sparse, ambient glyph rain spawning above the viewport that
falls through the hole's gravitational field — straight matrix
lines at the screen's edges, elegant arcs through the system's
reach, captured spirals into the shadow.

Motion DNA (new module `infall.rs`, the sixth of the family
split — it owns its pool, physics and spawn/advance/draw passes
outright, so the orchestrator grows only pass calls and the
LOC cap debt of the main file stays flat): inverse-square
gravity toward the hole with the field's magnitude
smoothstep-blended to zero at the influence edge (2.80 ball
outer radii, just past the tier-0 disk's 2.62 reach — the
bending zone and the disk read as one system), plus an
ACCRETION BRAKE inside the capture radius (1.95 outer radii):
the tangential velocity decays exponentially (0.62 per
sim-second at full strength, the physical read of infalling
material shocking against the disk and radiating angular
momentum away) while the radial plunge is untouched — fly-bys
become tightening inspirals, exactly how real accretion
resolves. A mote crossing the event horizon (the core
fraction) is EATEN: retired, never drawn inside the empty
core, its final flash landing on the photon ring (the
proximity grade's two-rung bump). Integration is sub-stepped
semi-implicit Euler (velocity first, then position — the
symplectic ordering), the substeps capped so no lag spike can
tunnel a mote through the horizon.

Brightness is speed-graded (KINETIC HEAT — the accretion
heating read: the faster the glyph, the brighter the base)
composed with the family's shared proximity ladder: the far
ambient rain reads Ghost through the fade ladder, the
accelerating approach Hot, the periapsis whip Core white.
Sim-time: one sim-second equals one wall-second at the scene's
reference 12 cps, dt_sim = dt_wall x cps x SIM_TIME_PER_CPS —
the speed keys scale positions, velocities and gravity
together, so trajectory shapes are invariant under the speed
setting (the family's speed contract as one scalar).

Family contracts carried onto the third pool: one lane per
column, deficit-bounded spawn accumulator with its own
fractional remainder (held zero through the formation intro —
no rain falls into a half-born hole), lifetime absorption with
±15% variance, per-mote pace variance, palette adoption, comet
trails, motion-gated shimmer, the unified three-pass diff
cleanup (an eaten glyph's streak is cleared by the diff on the
next frame — the contract's most load-bearing use yet), and
the dynamic-screen-size contract end to end (the physics runs
in ball-outer-radius units, the projection multiplies through
the cached outer radius; resize rebuilds the pool and the
spawn/exit envelope).

Constants: fifteen new stage-3 scalars in style_rain.rs with
compile-time contracts (capture inside influence, influence
clears the disk reach, the speed ladder strictly ordered).

Tests (`tests_black_hole/infall.rs`, 19 contracts): spawn +
fall after formation, the formation gate, straight far rain
(beyond the field), the bend toward the hole resolving into
capture, the sub-circular inspiral (radius never climbs above
the launch, ends eaten), the dead-center plunge, the fall's
acceleration, the brake's radial asymmetry, departure despawn,
the kinetic-heat ladder and its composition with proximity,
the never-paints-the-empty-core footprint (heads and trails,
the ring's approved crossing read scoped out), draw bounds,
clean resize, the ambient cap target, style-transition
recycle, pause freeze, speed-key sim-time scaling, and the
shimmer mutation — 56/56 black hole contracts, suite green,
fmt/clippy clean, version untouched.

### feature: NIGHT-special-1 stage 4 — the calm sky (owner-feedback tune: sparse, elegant, uncrowded)

Owner verdict on stage 3 (9/10): "the rain is too much... not
too like spam... clean not crowded." The motion DNA shipped in
stage 3 stays untouched — gravity, the accretion brake, the
inspirals, the horizon eating glyphs — and the WEATHER is
retuned around one principle: the rain is an ambient layer,
never a downpour; the hole stays the hero.

The crowd was the steady population: at the engine's default
density the stage-3 target filled roughly half the screen's
columns with falling streaks (~3.4x the read the owner
approved as "weather"). The population dial moved, not the
pace: the active-target base 0.18 -> 0.05, the density
multiplier 0.35 -> 0.09, the ratio cap 0.55 -> 0.16 (even the
full slider keeps the rain a clear minority of the lanes,
subordinate to the stack). The spawn cadence followed the
smaller budget: the floor 0.8 -> 0.25 spawns per second on
the quietest pools (the multiplier kept at the family's 0.30
so the smaller lane budget still fills — equilibrium settles
to a gentle drizzle, roughly a tenth of the columns, with
replacements arriving one at a time).

The elegance of the drop itself is the dim entry: the
kinetic-heat ladder's Ghost rung moved 1.10 -> 1.45, ABOVE
the fresh fall speed (1.32), so a glyph crossing the top edge
reads dim and quiet — only its own accelerating fall lifts it
up the ladder (Mid as the field takes hold, Hot on approach,
the Core-white periapsis whip). The far ambient rain now
reads Ghost its whole straight fall: the screen's edges stay
dark and calm while the action concentrates where the physics
is.

Tests (`tests_black_hole/infall.rs`, 58 total black hole
contracts): the harness adapted to the sparse steady state —
the rain-steady helper drives past the trickle's fill ramp,
the pool test's flight check widens from "moving down" to
"in flight" (a captured inspiral legitimately travels
upward for a stretch), the shimmer samples several short
windows (fewer glyphs with shorter lives at any instant),
and the approach-brightness zones accumulate across a run of
frames on one continuous clock (the far band is visited in
crossings, not constant occupancy). Two new stage-4
contracts pin the tune: the sparse minority (the default
density's target keeps the rain a small minority of the
lanes, the live pool as sparse) and the trickle cadence (no
frame ever spawns two or more glyphs — rain drifts in, it
never bursts in). Suite 2478/2478, fmt/clippy clean, version
untouched.

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
