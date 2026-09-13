<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-depth-hunt-1 — pre-super-LTS depth audit

Date: 2026-09-13
Scope: full source tree at commit `144e4a1` (HEAD: NIGHT-hunt-36 A/B
report). Owner directive: cosmostrix should be 99 percent free of
hidden bugs, security vulnerabilities, and residual problems before
the super-LTS declaration. Method: five previously-reported task
verifications re-run with current evidence, then fresh security and
hidden-bug sweeps beyond the owner's list (production panic-surface
scan, transposition-family index audit, division/underflow guards,
CI workflow injection review, secrets scan, dynamic probes on the
release binary), with every finding either fixed in this round or
classified benign with evidence.

## Verdict

No security vulnerability, no runtime panic defect, no hidden
rendering bug found. The audit surfaced three real items — all
process/hardening class, all fixed in this round: (1) the aur.yml
release-tag interpolation ran inside a `run:` block (trusted-only
trigger surface, so defense-in-depth, not an open vuln) — hardened
to env indirection; (2) dragon-history.sh --since-lock exposed
THREE post-lock commits that touched the locked cosmic engine
without UNLOCK entries in KEY.md — c523de9 (NIGHT-hunt-36),
fbc73cd (NIGHT-termux-hang), 0a1df6a (neural force-fires) — all
repaired with retroactive entries (the documented c1c7779 remedy);
(3) one
local directory (benchmark/bench-labs/night_hunt36) sat at mode 775
instead of 755 — fixed via check-permissions.sh --fix (invisible to
git, local hygiene only). The h34 style sweep's neural 3-cell
"frozen" reading was classified live static structure with a new,
stronger method (force-repaint frame-state membership probe).
Full suite 2879 passed / 0 failed / 2 ignored (59 s). Gates at HEAD:
cargo fmt clean, clippy -D warnings clean, gate-keepers 18/18.

## Part 1 — verification of the five previously-reported items

Owner asked to re-verify five items reported done by earlier
sessions. Each was re-checked against the current tree, not the
commit messages.

### 1. Three-dragon documentation (real render code, not gimmick)

Verified present and source-accurate: README.md ("not a clone" —
the Cosmic Dragon engine computes only the ~7.5 percent of cells
that change between frames; "No emoji. No wide characters." as a
permanent design constraint; "CPU-only by choice" with the GPU
bitmap-mode evaluation-and-rejection record; the benchmark reports
`gpu_usage: not_applicable`), docs/PHILOSOPHY.md section 1
(CPU-Only, Forever — the terminal-is-a-text-medium rationale),
docs/THREE_DRAGON_ENGINES.md (the engine split, lock protocol,
dragon-history.sh audit method), docs/RENDER_ENGINE.md,
docs/COSMIC_DRAGON_ARCHITECTURE.md, docs/CRYSTAL_DRAGON_ENGINE.md.
Cross-checks: src/engine/ contains exactly the three engine trees
(cosmic/chroma/crystal) with real simulation/color/palette code —
no stub or decorative module found; the 2 s bench probe emits
`"gpu_usage":"not_applicable","gpu_basis":"cosmostrix is a CPU +
stdout renderer; no GPU context is ever created"`. One staleness
fixed this round: the THREE_DRAGON_ENGINES.md title still said
"cosmostrix v50" while its own lock section records the v100 LTS
round — retitled.

### 2. CI path filters for the test/ mirror tree (commit 5553174 context)

Verified in .github/workflows/ci.yml at HEAD: the `paths:` filters
carry `src/**`, `test/**`, and `scripts/**` (plus Cargo.lock and
workflow files), with the header comment documenting the
2026-09-13 `test/**` addition and the deliberate exclusion of
`benchmark/research/*.rs`. A test/-only `.rs` change now triggers
the compiling workflows — the gap that let 5553174 land without CI
is closed. dragon-history.sh and the style-sweep harness (both
exercised this round) also confirm the tree wiring is live.

### 3. bump-rust-to.sh + permission guard + Unix notes

Runtime-verified: `./scripts/bump-rust-to.sh --check 1.98.1` passes
("all Rust version sources in sync at 1.98.1" — rust-toolchain.toml
channel, Cargo.toml MSRV, pgo-runner/Cargo.toml, workflow pins).
check-permissions.sh runs clean after this round's fix: 1089 tracked
files at 644/755 (58 executables), 201 directories at 755, shebang
parity across 58 scripts; it is wired into gate-keepers.sh as check
14 with --fix-all support. The Unix-only platform note
("PLATFORM: UNIX-only ... not for Windows cmd.exe or PowerShell")
is present in the script headers audited this round.

### 4. NIGHT-docs-2 docs freshness

Re-ran scripts/docs-audit.py and scripts/stale-hunt.py at HEAD.
docs-audit section 3 (stale count claims: "1500+ tests", "43
themes", "226 source files") is fully covered by
docs/FUTURE_BACKLOG.md's staleness registry — every hit is a
timestamped historical record marked "accurate at time of writing"
(CHANGELOG release entries, lock-time evidence lines in KEY.md,
migration notes in chroma RULES.md), which is the project's
documented disposition, not live staleness. Real live-doc staleness
found and fixed this round: FUTURE_BACKLOG.md and docs/ENDURANCE.md
carried broken links to archived/moved files and removed scripts —
repointed/annotated. stale-hunt: 25 stale references, all in test/
comment fixtures (stable by design); 221 duplicate groups, all
test-helper duplication — no action.

### 5. NIGHT-hunt-35 script fleet stability

gate-keepers.sh at HEAD: 18/18 (bash -n, shellcheck, shfmt,
yamllint, actionlint, TOML, markdownlint, codespell, ruff + EXE001
parity, naming, SPDX, LOC guard, rust-version sync, disclaimer,
symbol-only, comment style, language audit, permission guard).
Runtime smokes this round: night_h34_style_sweep.py (13 PTY
sessions — the long harness), dragon-history.sh (--per-engine and
the since-lock audit view), docs-audit.py, stale-hunt.py,
bump-rust-to.sh --check, check-permissions.sh (+ --fix path),
build.sh-backed cargo fmt/clippy, and the release binary probes.
All stable.

## Part 2 — security sweep (fresh, beyond the archived audits)

- Unsafe code: every `unsafe`-mentioning file inspected. The
  post-refactor sites not in the 2026-08-05 audit's list are all
  real and sound: terminal/terminal_tty.rs (NIGHT-termux-hang
  fcntl O_NONBLOCK set/restore + raw libc::write loop, checked
  returns, SAFETY comments; the remaining mentions are its
  #[cfg(test)] pipe fixtures), config/config_io.rs (fstat S_IFREG
  stdout probe, zeroed stat), interactive/watchdog.rs (isatty(1)),
  platform/fork_guard.rs (the PDEATHSIG guard, refactored out of
  main.rs), clock/posix_time.rs (the documented consolidation).
  termdetect/hosts.rs and clock/mod.rs mentions are doc comments,
  not code. No new unsafe in renderer hot paths — policy holds.
- Process spawning: 9 sites, all explicit argv, zero `sh -c` /
  `bash -c` / shell interpolation anywhere in src/ or scripts/
  (grep-verified). build.rs spawns git/rustc (build-time),
  restore.rs spawns stty/reset/tput (--reset-terminal only),
  update.rs spawns curl/wget (--check-update only), pgo-runner
  spawns build.sh (dev-only, not shipped).
- Environment: zero set_var/remove_var in production src/ (all
  #[cfg(test)]-scoped, grep-verified).
- Secrets: git grep for PAT/token prefixes across tracked files —
  clean (credentials live only in the local git credential store).
- CI: no pull_request_target in any workflow (the pwn-risk trigger
  is absent); no `github.event.*` interpolation inside run: blocks
  except aur.yml's release tag — hardened this round; the
  concurrency-group interpolation is a non-shell context (safe by
  construction). Two-phase privilege separation (maintenance.yml),
  pinned AUR host key, read-only token for validate jobs — all as
  documented in SECURITY_AUDIT.md.
- Network: the only network code remains platform/update.rs
  (opt-in --check-update, argv-only curl/wget, 15 s timeout, no
  outbound data). No network deps in Cargo.toml.
- Filesystem: safepath whitelist unchanged; the write surfaces
  remain dump-config/save-baseline under is_safe_path. Local file
  permissions: 1089 files / 201 directories enforced at 644/755 by
  the gate.
- Time-scale ceiling (S-master-HUNT-5): the 24 h cap lives inside
  both duration parsers; the bench-frames watchdog stops at 24 h —
  re-verified in the parser error contract (the 2 s bench run
  exercised the duration path).

## Part 3 — hidden-bug sweep

- Production panic surface: purpose-built scanner (test modules
  stripped, compile-time `const _: () = assert!` contracts excluded)
  over src/ + pgo-runner/. Result: 36 runtime sites, every one
  reviewed — all are local-invariant `expect`s guarded by preceding
  checks (Uniform::new with constant valid ranges, "set above"/
  "checked above" as_mut() pairs, split_once behind key predicates
  that require the dot, bench_frames behind dispatch's `if let
  Some`, count-guarded divisions in activity/hud stats). 0 unsound.
  Compile-time contract asserts: 149+ across style_rain.rs —
  compile-evaluated, zero runtime cost, excellent practice.
- Transposition/index-layout family (the hunt-36 bug class):
  physarum trail_field is column-major (`col * lines + line`)
  at every access site (deposit, draw, test hook) — consistent.
  flux_field uses row-major `j * w + i` with clamped neighbor
  indices and degenerate-viewport guards (w,h >= 3 by
  construction: `.max(2) + 1`). rain_post.rs and phosphor.rs
  recover (col,line) from row-major frame dirty indices then
  convert to the column-major phosphor pidx — the correct
  round-trip, with explicit bounds guards (`col >= cols ||
  line >= lines` -> continue). Zero layout mismatches found.
- Division sites: hot-path divisions all f32/f64 with clamped
  spans or guarded counts (activity.rs count==0 / count<2 guards;
  hud rolling averages guarded; spans derive from positive
  constants). No unguarded integer division by a runtime variable
  found in production paths.
- Locking: zero `.lock().unwrap()/expect()` sites in src/ (the
  threads use channels/atomics; the watchdog/ambient/watcher
  threads are lock-free or catch_unwind-contained per the
  architecture docs).
- Degenerate dimensions: cli_parse rejects w==0/h==0;
  brightness_factors early-returns on 0; FluxField floors at 3x3;
  logo scale guards raw_w/raw_h == 0. The 20-scene extreme-geometry
  matrix (1x1 .. 2000x500) was covered by the S-night-R1 round and
  the frame path is unchanged since.
- Release profile (overflow-checks = false): narrowing casts
  audited as low risk — interactive dims capped at 1024x500, bench
  at 8K (7680 cols), both far under u16/u8 ranges at the cast
  sites; color channels clamp before `as u8`.

## Part 4 — dynamic evidence (release binary, target/release)

- Probes: --version (v100.0.0-beta.1, commit c523de9, unwind,
  strip, fat LTO), --doctor (full diagnostics, no anomalies), and
  a 2 s headless benchmark: 86K avg fps, 5.10 MiB peak RSS,
  56.8 dirty cells/frame, fps drift -0.42 percent, frame jitter
  "low", stability "excellent" — consistent with the documented
  headless envelopes.
- 13-style stuck-cell sweep (night_h34_style_sweep.py, 4 s form +
  12 s window, five checkpoints per style): monolith, vortex, flux,
  lorenz, dragon, physarum, black_hole, aeolian, solar_flare,
  dna_helix, murmuration, quasar — 0 frozen cells. neural: 3
  scattered single cells.
- Neural classification (new method, stronger than the sweep's
  snapshot heuristic): force-repaint frame-state membership probe —
  HUD toggle on/off fires force_draw_everything() twice, resyncing
  the physical screen from the frame state. Result: 0 of the
  frozen-set cells erased; 3 persisted identically (dormant
  neurons — static integrate-and-fire cells per network.rs, whose
  column/line are static by design) and 6 changed with their
  brightness actively advancing (charge accumulation — the cells
  are being re-rendered live every frame). Verdict: frame-state
  content, live static structure — NOT orphans; benign by design.
  The probe script's method is recorded here for future rounds:
  frozen + survives force-repaint = live static; frozen + erased
  by force-repaint = orphan (real cleanup bug).

## Findings and fixes (this round)

| # | Finding | Class | Fix |
|---|---------|-------|-----|
| 1 | aur.yml TAG interpolation inside run: block | hardening (trusted-only surface) | env indirection (RAW_TAG), comment records the rationale |
| 2 | c523de9 touched locked engine, no UNLOCK entry | lock protocol | retroactive UNLOCK entry in cosmic KEY.md (this round's commit) |
| 3 | benchmark/bench-labs/night_hunt36 dir at 775 | local hygiene (git-invisible) | check-permissions.sh --fix, gate now 18/18 |
| 4 | THREE_DRAGON_ENGINES.md title "v50" vs v100 LTS lock section | live-doc staleness | retitled |
| 5 | FUTURE_BACKLOG.md / ENDURANCE.md broken links to archived/moved/removed files | live-doc staleness | repointed / annotated |
| 6 | neural 3-cell sweep reading | classified benign (live static) | evidence recorded (Part 4); no code change |

## Super-LTS readiness

Every prior audit layer (S-night R1-R8, triple-engine locks, hunts
1-36, security/soundness audits, endurance) plus this depth round
agrees: the codebase is at peak for its stated scope. The three
fixes this round were process/documentation class — no runtime
defect survived the sweep. Recommend proceeding to the super-LTS
declaration; the standing follow-ups (cargo audit/deny cadence in
CI, notify v7 pinning, ureq consideration) remain tracked in
SECURITY_AUDIT.md section 10 and FUTURE_BACKLOG.md.

<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 57 active .md files
  (plus historical docs in docs/archive/) and perfect sync is a known
  maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
