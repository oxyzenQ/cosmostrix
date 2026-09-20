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
