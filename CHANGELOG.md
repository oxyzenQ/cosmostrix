# Changelog
<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

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

## Released

v100.1.3-dragon.1

v100.1.2

v100.1.2-night.2

v100.1.2-night.1

v100.1.1

v100.1.0

v100.1.0-night.2

v100.1.0-night.1

v100.0.17
v100.0.16
v100.0.15
v100.0.14
v100.0.13
v100.0.12
v100.0.11
v100.0.10
v100.0.10-night.1
v100.0.9
v100.0.8

## Unreleased

The next release cycle starts here: entries land in this section as
they merge, and move to a per-era archive file under `docs/archive/`
when the version line closes.

### audit: night-cafe-mclaren-4 - the message family refuses attached values outright: space and equals are the only accepted forms (-m, -mb, -mfs)

- The gap: the owner's second probe after mclaren-3 - "masih bisa …
  aneh". mclaren-3 made `-mb<text>` expand to a CORRECT bordered
  message, but the token was still silently ACCEPTED as message data.
  The strict contract the owner is driving at is refusal, not
  reinterpretation: a flag-shaped token whose tail is message text
  must error, exactly like a typo'd long flag does (`--mbx` → clap's
  unknown-argument error). mclaren-3's expansion is superseded - both
  fixes kill the silent swallow, this one kills it harder.
- The full-flag audit that scoped the rule: 12 short flags exist
  (`-c -C -f -S -d -s -m -h -V -v -b -g -l -M`). Every value-taking
  short EXCEPT the message family already errors loudly on attached
  garbage, proven against the binary: `-fabc` → "invalid value for
  --fps: abc", `-czzz` → "unknown color", `-Czzz` → "unknown charset",
  `-bzzz`/`-Mzzz` → invalid value, `-gzzz` → expected NUM1,NUM2,
  `-dzzz`/`-Szzz` → invalid value, `-sfoo` → boolean cluster then
  loud fps error (prevalidation also pins the canonical grammar for
  the attached spellings). Free-form message text is the ONE value
  class that cannot be validated - garbage silently SUCCEEDS there
  and nowhere else - so the refusal lands exactly on the family:
  `-m<text>`, `-mb<text>`, `-mfs<style>` (valid style suffixes
  included) all exit 2 with a clap-format tip naming the accepted
  forms. The validated lanes keep their getopt attached convention
  (`-f60`, `-S12` verified end-to-end).
- The refusal retires the old attached side effects with it: an
  attached `-m` message starting with "fs" or "b" (`-mfss is my
  message`, `-mblaze`) no longer resolves to a shorthand - it is
  refused with the tip. Space and equals stay: `-m "hello"`,
  `-m=hello` (clap-native, verified), `-mb "hello"`, `-mb=hello`,
  `-mfs fade`, `-mfs=fade`. Bare `-mb`/`-mfs` keep their
  missing-value clap errors (night-dragon-fly-2 contract).
- Mechanics: one pure routing predicate
  (`message_attached_refusal_token`) + one branded error helper
  (`die_message_attached`, clap-format "unexpected argument … found"
  - why-tip via `ux::die_input_with_usage`), checked after the bare
  and equals arms so `-mb=x` still expands and never re-prefixes as
  "-mb" + "=x". The first predicate draft leaked `-mfs=fade`/`-mb=x`
  into the generic -m lane (their "-mf…"/"-mb…" prefix reads as
  attached there) - caught by the spares test before any commit; the
  generic lane now excludes every mfs/mb-lane token. The old
  `die_mfs_typo` + `is_valid_style_value` + the ValueEnum import are
  deleted (the attached lane no longer validates content - it refuses
  the class; the equals form hands garbage to clap's ValueEnum check).
- Ripple: the depthtest expansion-fuzz pool drops `-mhello`,
  `-mfsfade`, `-mfsradar` (the whole attached class is now a designed
  process exit - same reason the invalid-style suffixes were already
  excluded); the prevalidation test's comment now states the ownership
  handoff (prevalidation passes `-mhello`, argv_expand refuses it);
  `--help` documents the refusal on `-m` and `-mb`.
- Proof: 11/11 argv_expand tests (predicate covers the whole family +
  the bare/equals/validated spare list), 40/40 depthtest, full bin
  suite 3126/3126, clippy --all-targets clean, fmt clean. Binary:
  all five refusal probes exit 2 with per-lane tips
  (`-mbxxsaadadala`, `-msbg`, `-mhello`, `-mfsfade`, `-mblaze`); all
  accepted forms render correctly in the verbose dump (`-m sbg` →
  "sbg" border: false, `-m=sbg` → "sbg", `-mb hello`/`-mb=hello` →
  border: true, `-mfs fade`/`-mfs=fade` → fade, `-f60`/`-S12` →
  attached values on validated lanes untouched).

### audit: night-cafe-mclaren-3 - the -mb shorthand honors its own attached form: `-mb<text>` is a bordered message, never a silently unbordered one

- The gap: the owner's probe `cosmostrix -v -mbxxsaadadala` displayed
  message "bxxsaadadala" with border: false. The expansion table
  handled bare `-mb` + value and `-mb=<text>`, but the attached
  `-mb<text>` had no arm - the raw token leaked to clap, which parsed
  it as `-m` with the attached value "b<text>": both semantics of the
  shorthand silently lost (the border dropped AND the leading "b"
  eaten into the text), while a typo'd long flag (`--mbx`) gets
  clap's strict unknown-argument error - the exact asymmetry the
  owner flagged as "should be strict".
- The fix mirrors the lane's own precedents (night-dragon-fly-2 fixed
  the bare form's leak; the -mfs lane documents the principle: a
  shorthand-shaped token must honor the shorthand, never silently
  degrade into a different flag's semantics - "a silent footgun"):
  `-mb<text>` now expands to `--message-border -m <text>` - the token
  does exactly what it spells. Free-form text needs no validation
  (unlike the -mfs style enum), so the attached form is always a
  bordered message, and the equals form is checked first so `-mb=x`
  never re-prefixes as "-mb" + "=x".
- Documented side effect (same class as the -mfs one): an attached
  `-m` message starting with "b" (`-mblaze`) routes to the shorthand
  - bordered "laze". The space form (`-m "blaze …"`) or the equals
  form (`-m=blaze`) reaches a literal unbordered one. The module doc
  table, the side-effect paragraph, and `--help`'s `-mb` entry now
  carry both the form and the side effect.
- Proof: three new expansion tests (attached expansion, equals
  precedence plus "=" inside text, side-effect and pass-through
  guards covering the untouched `-m "blaze"` / `-m=blaze` /
  `--mblaze` lanes) and an end-to-end binary run: `-v -mbxxsaadadala`
  now reports `message: "xxsaadadala" (11 chars, border: true)` in
  the verbose dump (was `"bxxsaadadala"` border: false), while the
  space form `-mb hello` is byte-for-byte unchanged.

### audit: night-cafe-mclaren-2 - every quoted runtime value in verbose and the perf report tracks its engine constant (entry backfilled - the commit landed without its ledger line)

- The gap: two dishonesty-class ants from the owner's live `--verbose`
  session. First, `cadence_secs` quoted its default twice and
  hardcoded the value inside a string literal -
  `60.0s (default (unset — 60.0s); …)`: if
  `CRYSTAL_DRAGON_POLLING_SECS` ever changed, `--verbose` would keep
  claiming 60.0s (the same class as the V80 ambient-snapback audit).
  Second, the perf report showed two unlabeled frame-time averages
  that read as a contradiction: `Timing.rolling_avg_frame_time` is
  the mean WORK time of the last 60 frames (the tail window behind
  instant_fps) while `Latency.avg_frame_time` is the mean WORK time
  of the whole session - 0.144ms vs 0.069ms with no hint why.
- The fix: both cadence sites (startup `verbose.rs`, exit
  `final_state.rs`) use the sibling `ambient_snapback_secs`
  provenance label `default (unset in config)` - no value in the
  string, nothing to go stale, the effective value prints once up
  front. All three frame-time rows now name their window, and the
  `idle_throttle_share` row's hardcoded "after 30s" interpolates
  `IDLE_THRESHOLD_SECS` so it tracks the constant.
- Guardrail: the 11 `msg_fill_style` verbose labels quote pacing
  values as `&'static str` and cannot interpolate - all 11 verified
  against their engine constants, and a new lock test
  (`verbose_label_values_track_engine_constants`) pins every label
  fragment to its constant so a future constant change without a
  label update fails CI instead of lying in `--verbose`. The entry
  itself landed late (this backfill): the lane's own ledger
  discipline applies to the lane's author too.

### audit: night-cafe-mclaren-1 - the version bump goes adaptive: every [package] manifest rides the root version, zero stale on the line

- The gap: the ride-along lane was a hardcoded single-crate list -
  pgo-runner only. `cosmostrix-fuzz` sat at 0.0.0 beside a
  v100.1.3-dragon.1 binary (exactly the provenance hole the
  night-improve-9 lane was built to close), and any crate the owner
  added tomorrow would have drifted forever until a script edit
  taught the lane its path. The lane now discovers every
  `[package]` manifest under the repo root at run time
  (`scripts/lib/ridealong.sh`, the awk state machine parses the
  `[package]` table so a `[[bin]]` name or a dependency version
  never leaks into the crate identity): nested crates, test crates,
  future crates - wherever they live, the next bump adopts them and
  `version-sync` guards them with no script edit.
- The law got stricter, not looser: the pgo-runner birth-version
  courtesy (report 0.1.0 instead of failing) is retired. A
  ride-along crate at any version other than the root's fails
  `--check`, and the next `version-to.sh` run adopts it. The
  adoption landed in this same commit, so CI never sees a red tree:
  `cosmostrix-fuzz` rides 100.1.3-dragon.1 from here on.
- Bonus hunt (found while rewiring): `verify_version` incremented
  its error tally with `((errors++))`, and under `set -e` a
  post-increment from 0 returns nonzero status - the FIRST failed
  check killed the script instead of listing every desync (proven
  with `bash -ec 'e=0; ((e++))'` -> exit 1 before the echo). All
  nine call sites now use the safe `errors=$((errors + 1))` form,
  so a desync report lists the complete failure set.
- The entry script shrank (963 -> 871 lines, well under the 1000
  LOC cap) because the lane is a sourced module now, the same
  de-monolith shape as `scripts/lib/changelog-ledger.sh`.
- Proof, not vibes: a disposable git copy with two invented future
  crates (nested `tools/testlab` with its own lock, `lab/nolock`
  without one) bumped to 100.2.0-mclaren.1 - both adopted, the
  lock-less crate warned honestly and wrote nothing, the ledger
  downgrade gate still refuses a step-down, the post-bump check is
  green, and a hand-drifted crate fails the check with the adopt
  hint. Sim deleted after the run; the real tree carries only the
  fuzz adoption. The MSRV lane (`rust-version-to.sh`, the
  `rust-version` pair) is a different sync with its own gate and
  was deliberately not generalized here - fuzz carries no
  `rust-version` key today, and the owner decides when that lane
  follows.

### audit: night-cafe-2 + night-arc-2 - pins parity/determinism proof, the config verify hint is NOT stale, and the throttle audit verdict

- NIGHT-cafe-2 (actions-pin UX parity + determinism, zylaxion as the
  mature reference): the two-seat split here (check-actions-pins.sh
  gate reads; actions-version-sweep.sh heals) is the documented
  one-implementation design - the verdict grammar is the same lineage
  ("sweep verdict: N healed, N kept, N skipped (mode: M)"), the heal
  rules match (moving-major policy, SHA/local/docker/branch refs
  skipped-for-cause, guard_diff confines heals to .github/workflows/),
  and the mode names are MORE honest than the reference's own output
  (the zylaxion session printed "(mode: warn)" while its --apply run
  rewrote 4 pins; here apply mode prints "(mode: apply)" and warn mode
  never rewrites). Determinism PROVEN with authenticated twin runs:
  byte-identical output ("CI actions pins current - 137 kept, 27
  skipped-for-cause (checked just now)"), zero rewrites, zero tree
  churn - idempotent by construction (a current verdict changes
  nothing; the 6h cache makes repeats free). The anonymous-quota skip
  path is deterministic too (the twin-run diff caught the quota skip
  before the authenticated pass).
- NIGHT-arc-2a (the generated config hint
  `# verify full file: sha512sum <path> or --testconf`): verified NOT
  stale - it names the deliberate two-hash design from NIGHT-cafe-1:
  the --dump-config header template-fingerprint is blake3 (64-hex,
  the owner's zejtron "use blake3 for speed" mandate), while the
  full-file verification is sha512 BY DESIGN so the hint's `sha512sum
  <path>` interops with the standard tool and --testconf prints the
  identical digest. Proven live: `--testconf` file-sha512 ==
  `sha512sum` of the same config, byte-exact. No code change - the
  comment is accurate.
- NIGHT-arc-2b (the "micro lag saat throttle" audit): the power-dragon
  idle path is a deterministic PERIOD step, not a hidden stall -
  IDLE_FPS_FACTOR = 0.5 multiplies the frame period once at the phase
  transition (worst-case one added frame period, <= 6.9 ms at 144 fps,
  imperceptible), the adaptive resync redraw is bounded and periodic
  (adaptive_resync_interval), and the engine never backs up: pressure
  0.000, overshoot 0, budget headroom ~6.8 ms through the transition.
  The felt cadence change at idle engage is the documented power trade
  (arc-1's idle_throttle_share telemetry names the mix explicitly;
  the owner's session showed phase_transitions: 2 with a healthy
  score). No engine change: the sim is determinism-pinned (blake3
  frame digests + engine oracle), and a motion-ramping rewrite would
  churn the proof moat to chase an imperceptible single-frame step.

### fix: night-cafe-5 micro - the sig exit codes are fully documented (0 AUTHENTIC, 1 UNSIGNED, 2 tamper)

- The dragon.1 session audit surfaced the gap (`sig exit=1` on an
  unsigned-commit build with no documented meaning): the -h reference
  only said "Exit code 2 on tamper". The sig entry now states all
  three codes - 0 = AUTHENTIC (6/6), 1 = UNSIGNED (the honest 5/6
  verdict on a build whose commit carries no GPG signature), 2 =
  tamper - so scripts can gate on every outcome the tool emits.

### fix: night-cafe-5 follow-up - the version-sync tests learn the pre-release AUR law

- The cafe-5 script patch shipped, but CI caught the second half of the
  contract: test/docs_tests/metadata.rs asserted
  PKGBUILD/.SRCINFO pkgver == the Cargo version unconditionally, so the
  healed stable-base state (Cargo = 100.1.3-dragon.1, AUR = 100.1.2)
  failed the musl nextest shards. The two tests now branch on the
  pre-release shape (CURRENT_VERSION contains '-'): they assert the AUR
  pkgver stays a hyphen-free pacman version instead of equality, and
  keep the exact-match assertion for stable releases. docs_tests 21/21
  locally; clippy -D warnings clean.

### fix: night-cafe-5 micro - the sig exit codes are fully documented (0 AUTHENTIC, 1 UNSIGNED, 2 tamper)

- The dragon.1 session audit surfaced the gap (`sig exit=1` on an
  unsigned-commit build with no documented meaning): the -h reference
  only said "Exit code 2 on tamper". The sig entry now states all
  three codes - 0 = AUTHENTIC (6/6), 1 = UNSIGNED (the honest 5/6
  verdict on a build whose commit carries no GPG signature), 2 =
  tamper - so scripts can gate on every outcome the tool emits.

### fix: night-cafe-5 - pre-release bumps keep the AUR metadata at the last stable pkgver

- The audit of the owner's v100.1.3-dragon.1 bump session caught the
  ant: the pre-release branch of scripts/dev/version-to.sh ran
  update_pkgbuild unconditionally, writing pacman-ILLEGAL metadata —
  pkgver=100.1.3-dragon.1 into aur/cosmostrix-bin/PKGBUILD and its
  .SRCINFO. Pacman forbids hyphens in pkgver (the hyphen is the
  pkgver-pkgrel separator), so makepkg rejects the file and any manual
  AUR publish of that state would break the package metadata. The
  aur.yml lane was never at risk (its own guard rejects any hyphenated
  tag before sync — AUR only accepts stable packages), but the repo
  carried the invalid state between bumps.
- The law now matches the lane: a pre-release bump leaves the AUR
  files untouched (they keep the last stable pkgver, exactly what is
  live on the AUR), and verify_version's pre-release mode asserts the
  AUR pkgver stays a hyphen-free pacman version plus the .SRCINFO
  regen-sync, instead of expecting equality with the pre-release. The
  next STABLE bump writes the registry version again — no drift path.
- The v100.1.3-dragon.1 state healed: PKGBUILD/.SRCINFO restored to
  the stable 100.1.2 metadata (dual-license disclosure intact).
- Proven by two live runs: the idempotent check on v100.1.3-dragon.1
  now reports "PKGBUILD: pkgver=100.1.2 (stable base, untouched —
  pre-release 100.1.3-dragon.1 never syncs to AUR)" and passes; a real
  v100.1.3-dragon.2 bump moved Cargo/README/pgo-runner to dragon.2
  while the AUR files stayed at 100.1.2, with "All verification checks
  passed" — then the test mutation was wiped.

### fix: night-cafe-4 - the AUR package metadata carries the dual-license disclosure (zero license ants left)

- The last shipped surface declaring AGPL-only licensing was the AUR
  package metadata: `aur/cosmostrix-bin/PKGBUILD` `license=('AGPL3')`
  and its generated `.SRCINFO` `license = AGPL3`. Every `pacman -Si
  cosmostrix-bin` and the AUR web page rendered a bare AGPL license
  with no commercial rung. Both now carry the estate's canonical clean
  expression — `license=('AGPL-3.0-only OR Commercial')` — the same
  string `LICENSE_LINE` renders on `-V` since night-ux-orion-2.
- The `.SRCINFO` stays regenerations-proof: `scripts/dev/
  version-to.sh` (generate_srcinfo_from_pkgbuild) and the aur.yml
  lane both derive the license field VERBATIM from the PKGBUILD's
  array (proven live: sourcing the patched PKGBUILD renders
  `license = AGPL-3.0-only OR Commercial`), so future version bumps
  keep the disclosure — no drift path.
- One stale policy comment healed: scripts/gates/check-headers.sh
  said "project is AGPL-3.0-only licensed" (pre-Masterclass
  phrasing); it now states the dual-license policy.
- Sweep proof (the ant-hunt, repo-wide, excluding target/): every
  remaining bare "AGPL-3.0-only" mention is a legitimate description
  of the AGPL license itself (TRADEMARK.md's source-license
  statement, NOTICE's explicit dual-licensed sentence, LICENSING_FAQ,
  deny.toml's allowlist, historical CHANGELOG entries). The CLI
  surfaces were already correct at HEAD: `-V` prints
  `License: AGPL-3.0-only OR Commercial`, `sig` carries the identity
  block, `--doctor` has no license line, `--help` carries none by
  design. Gates: check-headers 928 files dual, check-version-
  anti-patterns 322 files, docs_tests 21/21, PKGBUILD bash -n,
  cargo build clean under the night-cafe-3 deny - all pass.

### build: night-cafe-3 - the strict-build law is total: warning = failed on every build shape

- Owner law: ANY build whose output carries a warning is a failed build,
  no exceptions. Three seats were already live (ci.yml global
  RUSTFLAGS: "-D warnings" with the re-add discipline on step overrides,
  ci-strict-build.sh wrapping the ci/maintenance/release lanes, and the
  supermassive matrix rustflags). The audit found the gaps and closed
  them all:
  - soak.yml built the release binary bare — a warning-passing build
    sailed through the 24h determinism anchor's precondition. The lane
    now wraps ci-strict-build.sh and carries RUSTFLAGS: "-D warnings".
  - miri.yml compiled through cargo with no deny — the env now carries
    RUSTFLAGS: "-D warnings" (miri shares rustc's frontend; main is
    warning-free, so the lane stays green unless a regression lands).
  - .cargo/config.toml now carries a global [build]
    rustflags = ["-D", "warnings"] block, so plain local cargo
    build/test/clippy/run deny warnings on every machine. The nine
    pro-* aliases each append "-D","warnings" inside their own
    build.rustflags arrays — an alias --config build.rustflags REPLACES
    the global block (cargo precedence), so each release shape must
    carry the deny itself; pro-android (shell alias) and the
    baseline-musl alias define no array and inherit the global block.
    CI behavior is unchanged: the RUSTFLAGS env overrides config files
    entirely and already carries -D warnings.
  - kani.yml is deliberately left without RUSTFLAGS: kani compiles
    through its own verification pipeline and forcing cargo's flag set
    into it is unverified — forcing it blind would risk a false-red
    proof lane, which the honesty contract ranks worse than a scoped
    exception. Documented here rather than hidden.
- Proof: cargo build on main runs clean UNDER the new deny (Finished,
  zero warnings emitted with -D warnings active) — the law passes on
  today's tree; every future warning is a hard failure everywhere.

### fix: night-echo-1 - the Backpressure verdict states the measured pressure instead of a hardcoded 0.000

- The owner flagged it twice across two sessions: the report's Backpressure
  section printed `! pressure_avg/pressure_peak 0.000 = healthy (renderer
  kept up).` directly under `pressure_avg: 0.437` / `pressure_peak: 0.600`
  (then 0.323/0.375) — the advice string hardcoded `0.000`, so the section
  contradicted its own numbers and read like the renderer claiming a
  healthy verdict it had no right to claim. Exactly the misleading-output
  class the honesty contract forbids.
- The verdict is now built from the measured values at the same 3-decimal
  precision the fields display (so the verdict can never disagree with
  them): zero pressure keeps the `0.000 = healthy (renderer kept up)`
  wording, non-zero pressure prints
  `pressure_avg 0.437/pressure_peak 0.600 > 0 = the frame PERIOD exceeded
  target (sleep + poll overshoot, not renderer work — budget_utilization
  N% is the WORK share). For real FPS see TIMING.avg_fps / TIMING.instant_fps.`
  One shared builder (format_backpressure_section) serves the bench and
  the interactive --perf-stats report, so both surfaces get the fix.
- Two unit tests pin both branches end-to-end through the rendered lines
  (render_lines widened to pub(crate) for cross-module test assertions;
  print()/emit() stay the runtime entry points): zero pressure keeps the
  healthy wording, 0.437/0.600 states the measured values and never
  claims 0.000.
- Also verified while reproducing the owner's report: the `Memory (alloc
  ledger)` section and `frames_flushed` ARE unconditional — the owner's
  terminal copy stopped mid-Encoding, which reads as "section missing".
  Full-report capture order documented in the audit manual (Memory is
  LAST, after Encoding and Tier2 xterm.js); `tail -40 report.txt` after
  a `2>` redirect shows everything.

### feat: night-arc-1 - the perf report explains an avg_fps below target: idle_throttle_share

- The owner reported felt "micro lag" and asked for a depth
  architecture audit (smoothness, crystal dragon intelligence, honest
  math). The audit verdict, fully measured: the renderer is not the
  bottleneck — a paced 144fps release bench reads avg work 0.021ms,
  p99 0.027ms, p99.9 0.033ms, max 0.058ms, jitter low (0.3% of the
  6.944ms frame budget); the crystal-dragon drift tick adds one 0.33ms
  sim spike per 3s tick (4.8% of one frame, below perception, crystal
  on vs off otherwise identical); the always-on climate drift is O(1)
  scalar accumulation applied shader-side (the post-hoc
  decode-encode pass was already deleted), so no full-grid
  invalidates. What the owner actually felt is the power_dragon idle
  reduction: after 30s without input the cadence halves (144→72).
  The math locks against the owner's own 22-minute report —
  frame_period_avg 12.873ms sits between the active 6.944ms and the
  idle 13.889ms; the implied 85.4% idle share predicts avg_fps 77.68
  and the report measured exactly 77.68.
- The report never said any of this: `target_fps` showed the BASE
  144.000, `avg_fps` showed 77.680, and nothing connected them — the
  reader was left blaming the renderer or the scheduler. The power
  manager now counts its own idle-paced frames
  (`idle_throttled_frames`, incremented in `begin_frame` when
  `is_idle` returns true) and the exit PERFORMANCE REPORT's Timing
  section prints `idle_throttle_share: N% of frames ran idle-paced
  (power_dragon idle reduction after 30s without input) — avg_fps is
  an active/idle cadence mix, not a renderer shortfall`. Verified
  live: a 38s PTY session crossing the 30s threshold reads
  `11.9% of frames ran idle-paced`. No loop-file change: the counter
  rides the PowerManager the stats builder already receives. If the
  slowdown is never wanted, `power_dragon = false` removes it — the
  report now makes that an informed choice instead of a mystery.

### fix: night-firefly-1 - the runtime surfaces show a custom scene's original-case name, not the lowercase key

- The same bug class NIGHT-dragon-fly-2 closed for charsets was still
  open for scenes: a config `scene = "Tron_legacy"` resolving a
  `[scene-custom.Tron_legacy]` block listed correctly in --list-scenes
  (the NIGHT-cafe-1 original-case fix reached the listings) but the
  startup verbose line printed `scene: tron_legacy` and the HUD
  monitor showed `scn: tron_legacy` — config_apply stores the
  NORMALIZED value (the matching contract) and that string flowed into
  every display state. The owner hit it on the HUD after the listing
  rendered `Tron_legacy`: the two surfaces disagreed about one name.
- The original-case header record now rides the memoized config load
  to the runtime: a display-only swap in main.rs (right after config
  apply, before the verbose block and CloudConfig build) replaces
  args.scene with the block's original spelling via the new
  scene_custom helper, so the verbose `scene:` line, the HUD `scn:`
  line (via CloudConfig.scene_name), the session report and the
  benchmark scene field all show what the config spells. The swap is
  display-only: every scene matching downstream normalizes internally
  (the custom-scene tracker rides scene_custom_name, the live-reload
  applier re-reads the raw value, the collision pre-check ran before
  the swap), builtin scenes and no-config runs pass through unchanged,
  and the s/S cycle behavior is identical (a custom name was never in
  the builtin cycle list).
- One consumer needed hardening for the swapped spelling:
  sync_base_cfg_with_runtime_scene compared scene_name exactly, so a
  startup display name differing only in case from a runtime switch's
  normalized key would have triggered a spurious sync on the first
  unrelated config edit — the compare is now case-insensitive
  (eq_ignore_ascii_case).
- Verified live on the dev binary with a `scene = "Tron_legacy"` +
  `[scene-custom.Tron_legacy]` config: --list-scenes shows
  `Tron_legacy`, the verbose line reads `scene: Tron_legacy`, and a
  real-PTY session (100x36, HUD toggled with `i`) renders
  `scn: Tron_legacy`; `chr: retro` stays verbatim. A unit test pins
  the helper (lowercase key resolves the block's original spelling;
  builtin/empty lookups return None so callers keep their normalized
  name). Same-10s-benchmark sanity: the swap is startup-only.

### feat: night-nightjar-1 - the perf report carries the zero-heap proof (Memory alloc-ledger section)

- The zero-heap frame was a CI-only contract: the README claims
  "0.0 allocs/frame (measured)" and the cosmic_dragon_incubator ledger
  enforces it (INV-18 lean window, INV-25 production-draw window, the
  counting `TraceAlloc` global allocator), but the exit PERFORMANCE
  REPORT showed Timing/Frames/Motion/Latency/Backpressure/Encoding —
  nothing about allocations — so a user quitting a verbose session saw
  no runtime mirror of the claim. The infra was already there and
  always on: `TraceAlloc` is the production global allocator with
  const-init TLS counters (no opt-in, no overhead beyond a slot
  update). The window now opens at frame 120 (startup noise excluded:
  config parse, intro, HUD build; the baseline snaps on the loop
  thread and the exit tail deltas it — same thread, the TLS
  attribution contract) and the report prints a `Memory (alloc
  ledger)` section: window bounds, alloc/dealloc/realloc calls with
  per-frame rates, heap retained bytes, /proc VmData heap virtual
  size, and a verdict. The window honestly includes exit teardown and
  runtime events (mouse/resize/scene/keys) — the verdict says so and
  points at the ledger for the per-frame contract; on long sessions
  the per-frame rate converges toward 0.000. Verified under a
  pseudo-TTY: a 6s verbose session shows `alloc_calls 158 (0.656/frame)`
  (teardown-dominated at 241 window frames), `heap_retained 0 B`
  (everything the window allocated was freed) and `heap_virtual
  11.59 MiB`; sessions shorter than 120 frames report `window too
  short` instead of a diluted number. SessionStats gains the baseline
  field (event_loop_stats builder passthrough), `update_perf_stats`
  gains the one-branch capture, help `--perf-stats` text discloses the
  new section, and the loop file stays exactly at its 800-line cap
  (comment compaction pays for the passthrough line).

### fix: night-dragon-fly-2 - the runtime surfaces show a custom charset's original-case name, not the lowercase key

- A block spelled `[charset-custom.Quantum]` listed correctly as
  `Quantum` in `--list-charsets` (the NIGHT-cafe-1 original-case fix
  reached the listing surfaces only) but the runtime HUD monitor
  showed `chr: quantum`: the startup path normalizes the preset name
  to the lowercase lookup key and carried that same string into every
  display state. The original-case header record now rides the
  memoized config load to the runtime: after the glyph pool loads,
  `original_case_custom_charset_name()` swaps the display name to the
  block's original spelling, so the HUD `chr:` line, the startup
  verbose line (`charset: Quantum (custom, 44 chars)`), the session
  report and the benchmark `charset` field all show what the config
  spells. The swap is display-only: glyph matching already happened,
  every later lookup normalizes internally (the custom loader, the
  live-reload applier), the parser-level key inventory stays lowercase
  by contract, builtin names pass through unchanged, and the runtime
  `s`/`S` cycle behavior is identical (a custom name was never in the
  builtin cycle list). Verified live on the dev binary with a
  `[charset-custom.Quantum]` 44-char block: `--list-charsets`,
  the verbose startup line and the glyph pool all agree on `Quantum`.

### fix: night-dragon-fly-1 - the config-path whitelist mirrors the read side's XDG resolution

- The read side (`default_config_file_path()`) prefers
  `$XDG_CONFIG_HOME/cosmostrix/config.toml` over `$HOME/.config/...`
  on every non-Termux platform, but the write-side whitelist
  (`safepath::is_safe_path`) only carried the `$HOME` prefix. An XDG
  user's `--config-path` output was refused by `--config <that-path>`
  and `--dump-config <that-path>` with "outside allowed directories":
  the binary suggested a path it then rejected. The whitelist now adds
  the `$XDG_CONFIG_HOME/cosmostrix/` prefix on non-Termux platforms, so
  the read and write sides resolve the same tree. Termux keeps the
  HOME-only whitelist — the Termux fix deliberately ignores XDG there
  (it may point at `$PREFIX/etc`, a system location users do not edit),
  and the write side mirrors that ignorance. Verified live on the dev
  binary: the exact repro that used to exit 2 with "outside allowed
  directories" now writes the config, and the overwrite refusal is the
  data-loss guard, not the whitelist. New safepath tests cover the
  allowed XDG prefix, the rejection of XDG paths outside `cosmostrix/`,
  and the Termux XDG ignorance (with `$PREFIX/etc/cosmostrix/` still
  allowed alongside).

### fix: night-cafe-1 - the template fingerprint drops the legacy sha512 fallback (one hash family, one path)

- The blake3 switch landed with a backward-compatible digest-width
  selector: a 128-char sha512 header fingerprint still verified through
  a second comparison path (`drift: none ... sha512 (legacy)`). The
  owner's no-fallback contract closes that door:
  `extract_template_fingerprint` now takes exactly 64 lowercase hex
  chars and nothing else — the 128-char width, the uppercase-hex
  loophole, and the legacy v50 label (`# sha512 (template):`) all
  read as "not found". `--testconf` loses the width branch and always
  compares blake3; the "not found" branch gained a regeneration hint
  (`cosmostrix --dump-config <path> --force`) instead of silently
  re-verifying a legacy header with a retired algorithm. The full-file
  hash (`testconf: file-sha512`, matching `sha512sum` on disk) is a
  separate scope and unchanged — coreutils-verifiable on any machine,
  no `b3sum` install required. Tests flipped from accept-contract to
  reject-contract (legacy width, legacy label, uppercase digest each
  assert `None`), and the header-coverage test comment no longer
  describes the legacy compare path.

### fix: night-hit-2 - the sig ladder names an unsigned commit UNSIGNED (a missing provenance link, not a modification)

- A pristine local build with no GPG used to read
  `SIGNATURE: TAMPERED (5/6 depths verified)` — a lie about the
  artifact: nothing was modified, the provenance chain just stops at
  an unbound commit. The verdict ladder splits the class out (the
  zelynic night-dragon-fly-4 port): `SIGNATURE: UNSIGNED (5/6 depths
  verified)` fires only when the commit-signature depth is the ONE
  failure and its stamp state is `unsigned`, and the report carries
  its own note ("a missing provenance link, not a modification —
  every integrity depth above verifies"). TAMPERED stays the
  tampering-class verdict: any integrity-depth failure (D0-D4), a
  foreign key, a failed cryptographic check, and the fail-closed
  unknown states (no-vcs, unavailable, unstamped) all keep
  `SIGNATURE: TAMPERED`.
- Exit contract: 0 AUTHENTIC, 1 UNSIGNED, 2 TAMPERED — the
  any-failed-depth contract still names the exit (a script gates on
  the code without parsing the text), and a script can now tell a
  missing link from a tampering-class hole. The classifier is a pure
  function (`verdict_class`) unit-tested across the full matrix, so
  the contract does not depend on the build's own stamp; the
  `commit_signature_state` helper (ported from zelynic) splits the
  stamp's state word off the evidence.
- `-V` names its own coverage on the Depth-matrix line: the appended
  hint `(runtime depths; d4/d5 ride cosmostrix sig)` marks the
  runtime-verifiable depths (d0-d3) versus the two `sig`-exclusive
  rows (d4 engine-oracle, d5 commit-signature). The hint appends
  after the layers suffix, so the line's leading shape stays
  byte-stable for the fingerprint-token parsers.
- Sync duty honored in the same commit: the supermassive sig_parity
  gate accepts the new verdict shape (exit 1 + UNSIGNED 5/6 with the
  D5 FAIL row, alongside the 6/6 AUTHENTIC and the fail-closed 5/6
  TAMPERED shapes), `scripts/release/prove-d5-lane.py` relabels the
  unsigned-HEAD expectation, and docs/INNOVATIONS.md documents the
  three-verdict exit contract.
- Verified on a real debug build of this tree (unsigned HEAD):
  `cosmostrix sig` prints `SIGNATURE: UNSIGNED (5/6 depths verified)`
  with D0-D4 all pass, exits 1, and carries the missing-link note;
  `-V` shows `status=verified` with the appended hint.

### fix: night-cafe-1 - custom block names display in their original case and the template fingerprint moves to blake3

- `--list-charsets` rendered every custom charset name lowercased
  (`[charset-custom.T1]` listed as "t1"): the config parser lowercases
  section names for case-insensitive key lookup, which also destroyed
  the user's block-name spelling before any display surface saw it.
  The parser now records each custom-block header paired with the
  spelling as written (`ParsedConfig::original_custom_block_headers`),
  and the three listing surfaces display the original case:
  `--list-charsets` ("T1"), `--list-scenes` ("Nebula"),
  `--list-colors` ("Dusk"). Matching stays case-insensitive end to end
  (`-C t1` still loads `[charset-custom.T1]`; the lookup key, the
  duplicate-header detector and the completeness validator all keep
  their lowercase contract). Verified against the owner's own repro:
  the 44-char `T1` pool lists as `T1   44 chars`, and the legacy
  sha512 fingerprint quoted in the task (`1464e07e...349ea`) recomputes
  bit-for-bit from the current template body.
- The `--dump-config` header's `template-fingerprint` moves from
  SHA-512 (128 hex chars) to blake3 (64 hex chars), per the owner's
  "use blake3 for speed" contract — the digest that `--testconf`
  compares is now `configfile::blake3_hex`, the same hash family the
  depth anchor, the binary self-hash and the replay oracle already
  use. Backward compatible by digest width: configs written before
  this change carry a 128-char sha512 fingerprint and `--testconf`
  compares it against the legacy sha512 digest of the current template
  (`drift: none ... sha512 (legacy)`), so no false drift is reported.
  The full-file hash (`testconf: file-sha512`, matching `sha512sum`
  exactly) is a separate scope and unchanged. Cross-checked against an
  independent Python blake3 implementation (empty string and `abc`
  reference vectors plus the full 8893-byte template body) — the Rust
  digest matches byte-for-byte, so the test vectors cannot pass
  circularly.

### fix: night-seal-gate-1 - the installer's official-build gate matches the materialized -V signature surface again

- `scripts/package/install-on-my-machine.sh` refused every locally
  built binary at the signature gate: its grep expected the
  year-bearing manifest text (`Official Build by rezky_nightky
  (oxyzenQ) 2026`), while `manifest_display()` strips the identity
  year from the `-V` Signature line by contract (the year never
  renders beside the identity on any user-facing surface), so no
  healthy binary could ever pass the gate. Verified on a real
  `pro-linux-amd64-v4-musl` build of this tree: the year-bearing grep
  can never match, while the year-free identity grep, the
  `status=verified` Depth-matrix line, and the five-depth `sig`
  AUTHENTIC verdict all pass. The grep is now year-free and the
  installer comment documents the display contract plus where the
  year-bearing oracle actually lives (the masked words and
  `scripts/release/generate-provenance-words.py`).

### fix: night-depth-cleanup-2 - the supply-chain doc catches up to the verdict it documents

- Full-walk finding A (DOC_AUDIT rule 4, same-pass sync — the audit
  inherited the split): docs/SUPPLY_CHAIN.md still told the
  pre-AXP1-ZnQ (Architect's eXecution Protocol 1) lane story — "weekly cron runs cargo update + cargo
  audit + cargo deny", "resolves them by cargo update, re-auditing,
  pushing updated lockfile", a no-op early-exit step that no longer
  exists, and an advisory-response table where High advisories
  "resolve automatically" — while the source (maintenance.yml) moved
  to the pre-pass / verdict / diff-gate / changed-gated-deny sequence
  with scenario A (cleared) and scenario B (survives = CALL OWNER,
  nothing ships). All five blocks now tell the verdict-era truth,
  including the fact that a `deny.toml` ignore entry cannot silence
  the verdict (the verdict reads the scanner report, not the deny
  policy).
- Finding A2 (rule 1, tell once): SECURITY.md restated the verdict
  mechanism that the SUPPLY_CHAIN.md home owns — trimmed to the
  registry-style pointer so the fact keeps exactly one telling.
- Finding B (rule 1, volatile fact drift): the hardening section
  cited `actions/checkout@v6` as the floating-tag example while the
  workflows actually ride `@v7` — corrected to the real tag.
- Everything else the walk measured came back already at the bar and
  is left untouched: the mechanical seat (scripts/gates/docs-audit.py)
  reports zero broken references, zero stale paths, zero stale counts
  and zero duplicate candidates across the 66-file living corpus;
  every in-scope doc has its registry row (root README or
  docs/README.md); the lock census pin (121 names / 124 entries), the
  12-runtime-deps claim (9 cross-platform, signal-hook/libc on unix,
  ctrlc on windows), the MSRV pair, the cron time and the checkout
  tag were each verified against source before being left alone.
  Frozen records untouched per the audit's exemption set.

### fix: night-fly-1 - the GPG expiry check trusts keyring presence, not recv exit codes

- CI incident (Dependency Maintenance run #46, 2026-11-21): the GPG
  Signing Key Expiry Check job died in six seconds with "error reading
  key: No public key" (exit 2).
- Root cause proven by a live keyserver probe, not assumed:
  keys.openpgp.org serves the master key but strips its user IDs (none
  of the key's identities are verified there), and gpg skips a
  UID-less key on import ("new key but contains no user ID - skipped")
  while still exiting 0. The old `||` chain read that exit 0 as
  success, so the keyserver.ubuntu.com fallback never ran, and
  `--list-keys` found an empty keyring.
- No key rotation: the ubuntu keyserver imports the full key today —
  master `F5324E0967F104D58CE025F347A50AEF4B65AAC2`, artifact subkey
  `3C9EB25BF0407781` (expires 2028-08-23), tag subkey
  `2B52187D2AB618A7591D45C9C7F8B07418899C4E` (2031-08-23), and the
  pre-rotation `56B96F3109F4B924` (2028-08-14) — every date exactly
  matching docs/VERIFY_RELEASE.md, with the purged night-remake-1
  identities riding as revoked (uid:r) records as documented.
- The fetch loop now verifies keyring presence after every server
  attempt and only proceeds when the key actually landed: ubuntu first
  (the documented source of truth), keys.openpgp.org second (in case
  its identities are verified there later). Server output stays
  visible for diagnosability. The non-fatal warn-and-exit-0 contract
  when no server answers is unchanged.
- docs/VERIFY_RELEASE.md: the keys.openpgp.org "mirror" import
  instruction is removed — it imports nothing today (the same
  UID-less skip), so it was a dead instruction; the ubuntu import is
  the only one.

Verified: the exact run block extracted from the edited workflow was
executed in a scratch keyring — it imports the key from ubuntu on the
first attempt and the expiry parse reports all three subkeys OK (641 /
1735 / 631 days remaining); a scratch-keyring probe reproduces both
server behaviors (openpgp.org: skipped, exit 0; ubuntu: imported,
exit 0 with the key present); yamllint clean on the workflow.

### ci: night-fly-1 - the test lanes ride the commit: the weekly cron left every push-triggered workflow

- The owner's CI law (night-fly-1): no lane runs past 15 minutes and
  no test lane carries a schedule — every test fires on the commit to
  main. Six push-triggered test lanes still carried the estate-wide
  Sunday cron (night-consistent-1) as a redundant duplicate burn:
  fuzz.yml, kani.yml, loom.yml, miri.yml, codeql.yml, and
  gitbot-audit.yml. All six already rode every push, so the weekly
  run only re-proved a verdict the commit run had already delivered.
  The cron blocks are removed; each trigger block documents the
  push-triggered law where the cron used to be.
- fuzz.yml also lost the dead branch the schedule carried: the
  `event_name == "schedule"` budget table (900s deep session vs 60s
  push net) collapses to a flat 60s budget, and the header comment
  describes the corpus growth across push sessions instead of the
  weekly deep session.
- soak.yml keeps its Sunday slot, now explicitly documented as
  maintenance-class: the determinism anchor needs ONE long-lived
  process, so micro-splitting it into sub-15-minute per-push shards
  would falsify the test it exists to run. maintenance.yml keeps its
  slot unchanged (pure maintenance). Every job-level
  `timeout-minutes` cap was re-audited and stays as documented:
  ci.yml legs <= 10 minutes, model-checking micro jobs <= 15, publish
  pipelines carry their own deliberate larger caps, the soak window
  IS the test.
- Docs synced to the new trigger map: docs/workflow/ABOUT_CI.md
  (six trigger-table rows + the scheduled-lanes paragraph — "all
  eight scheduled lanes" is now the two maintenance-class lanes),
  docs/SECURITY_AUDIT.md (fuzz budget), docs/MAINTENANCE.md (the
  dormant-mode CI contract), docs/SUPPLY_CHAIN.md (the weekly CVE
  monitoring claim, the compensating-controls audit line, and a
  never-true "daily 00:00 UTC" description of gitbot-audit.yml), and
  the README fuzz-lane row ("900s weekly" dropped).

Verified: yamllint clean on every changed workflow; the YAML
per-job audit re-run reports every job with an explicit
`timeout-minutes`; a residual grep for the removed crons matches
only maintenance.yml and soak.yml; the old trigger-map claims
("900s weekly", "all eight scheduled lanes", the daily 00:00 UTC
audit description) return zero hits in living docs — the surviving
"weekly deep session" mentions describe the removal itself, per the
estate's describe-don't-repeat convention.

### fix: night-fly-1 - the package script SPDX headers stop repeating the handle

- The night-fly-1 zero-residual sweep caught one leftover of the
  night-ux-orion-2 estate-wide SPDX header standard: two package
  scripts (scripts/package/install-on-my-machine.sh and
  scripts/package/uninstall.sh) carried the copyright handle twice —
  a doubled parenthetical suffix after the handle — where every other
  file in the estate carries it exactly once. Both headers now carry
  the single canonical form. Header comment only: no behavior change,
  no permission-bit change (644 stays 644).

Verified: an estate-wide grep for the doubled-handle pattern returns
zero hits across all tracked files.

### feat: night-fly-1 - the signature display identity synced: verification docs expect what gpg actually prints

- The night-remake-1 UID purge reordered the key's identity set on the
  keyservers, so `gpg` now reports the proton UID as the primary —
  the `Good signature from "rezky_nightky (oxyzenQ) <oxyzenQ@proton.me>"`
  line, with the noreply UID on the aka line — but two verification
  surfaces still told readers to expect the pre-purge primary: the
  docs/VERIFY_RELEASE.md "Expected output" line and the
  scripts/release/generate-release-notes.sh "Expected" line that every
  generated release notes page ships. Both now expect the proton
  primary verbatim, with the aka line documented alongside it, matching
  live `gpg` output on a fresh import.
- The artifact-signing subkey entry in docs/VERIFY_RELEASE.md now
  carries the subkey's full fingerprint
  (`3EE4A49C78630A16E176DB2B3C9EB25BF0407781`) next to its key id
  (`3C9EB25BF0407781`) — the full form is exactly what `gpg` prints in
  its "using EDDSA key" line during `gpg --verify`, and without the
  full form in the docs a first-time verifier reads that line as an
  unfamiliar key. The active subkey set was re-verified against
  keyserver.ubuntu.com on 2026-11-21 (both the master fingerprint and
  the subkey full fingerprint resolve to the same primary key; the
  colon dump confirms the two live subkeys, their expiry dates, and
  the pre-rotation artifact subkey exactly as documented).
- Honest finding: the master key did not rotate. The fingerprint that
  triggered this hunt belongs to the documented artifact-signing
  subkey, so every import instruction (README, docs/VERIFY_RELEASE.md,
  the release-notes generator), the maintenance.yml expiry-check key,
  and the PKGBUILD GPG header stay master-fingerprint-anchored
  (`F5324E0967F104D58CE025F347A50AEF4B65AAC2`) and are intentionally
  unchanged.

Verified: keyserver ground truth fetched and parsed (gpg --show-keys
and --with-colons); residual scan across the repo shows zero live
surfaces still expecting the pre-purge primary identity — the
CHANGELOG history keeps its record, the CLA keeps the owner's legal
name, and the engine-code "cosmic dragon" strings are module names,
not contact data. Docs-only change, so the benchmark lane is
intentionally not run.

### feat: night-fly-1 - the maintenance lane tells the two supply chain scenarios apart

- Owner directive 2026-11-21 (the sleeping-owner contract): the Sunday
  dependency lane must not just fail on every advisory like plain
  `cargo audit` does — it must tell a dependency that CAN be fixed
  (patched versions exist; non-blocking, UPDATE ASAP banner, the lane
  and the lock refresh continue) apart from one that CANNOT (no
  patched version anywhere; exit 1, CALL OWNER banner, the run fails
  hard so the scheduled-failure notification reaches the owner, and
  the commit phase never publishes the refresh while the risk stands).
- New `scripts/ci/security-guard.py`: a classifier over the
  `cargo audit --json` report. Tier 1 = advisory carries patched
  versions; Tier 2 = it does not; informational (unmaintained and
  friends) stays listed but never gates. Fail closed throughout: a
  missing tool, a missing lockfile set, a malformed report or any
  non-{0,1} cargo audit exit is an infrastructure failure that exits
  1, because a broken guard must never pass silently. Three advisory
  schema shapes are accepted (the 0.21 advisory-level `patched` list,
  the sibling `versions` object, and the legacy `patched_versions`
  string) so schema drift across cargo audit releases cannot flip a
  tier.
- Audit scope now follows the night-zen-1 law it previously lagged
  behind: the lockfile set comes from the canonical manifest walk
  (`scripts/ci/update-all-workspaces.sh --list`), so the root-only
  `cargo audit` this replaces never audited `fuzz/` and
  `pgo-runner/` in this lane — both now ride the same audit as the
  refresh and the commit phases.
- Both audit steps in maintenance.yml (baseline pre-refresh,
  post-update pre-commit) run the guard with a phase label; the
  post-update phase is the one that gates the bot commit, so a
  refresh that itself introduces an unfixable advisory stops the
  lane before anything lands.
- Ownership split with cargo-deny (deny.toml [advisories]):
  vulnerabilities are warn-level there because the guard owns that
  verdict now; deny keeps `yanked = "deny"` (the arrayref defense),
  bans, licenses and sources, and the ci.yml / release.yml cargo
  audit lanes keep failing on every advisory unchanged. The
  observation-only gitbot-audit.yml keeps its capture-don't-fail
  behavior by design.
- The report surfaces on the GitHub step summary: CALL OWNER
  (`[!CAUTION]`) and UPDATE ASAP (`[!WARNING]`) banners, per-lockfile
  advisory tables (severity, fixed-in requirements, CVE aliases,
  RustSec page links), informational rows, and per-finding
  `::error::` / `::warning::` annotations in the job log.

Verified: the built-in self-test passes 14/14 checks (tier split,
schema-shape tolerance, parser noise tolerance, table escaping,
banner and verdict rendering, fail-closed paths); offline fixture
runs reproduce all three owner scenarios end to end (unfixable exit
1 with CALL OWNER, fixable exit 0 with UPDATE ASAP, malformed report
exit 1 fail-closed); the rendered step summary carries both banners
and the verdict; yamllint clean on the edited workflow.

### feat: night-solid-defense-1 - the supply-chain verdict is AXP1-ZnQ: the estate sentinel comes home

- Owner directive (night-solid-defense-1): the supply chain bot takes
  the estate's sentinel identity - AXP1-ZnQ (Architect's eXecution
  Protocol 1), the same bot the zejtron
  sibling runs (github.com/oxyzenQ/zejtron, the mature reference) -
  and its verdict adopts the mature semantics, not just the name.
- The guard becomes `scripts/ci/supplychain-verdict.py` with the
  mature two-mode contract: `--pre` scans the as-committed lane and
  snapshots the findings; `--post` scans the swept lane, diffs
  against the snapshot, and emits the verdict. Scenario A (fixable)
  now means what the mature bot means: the sweep actually CLEARED
  the finding (the advisory vanished from the swept lane), the job
  stays green and the fix ships in this run, with the loud notice
  that the tree would have carried the advisory for a week without
  the lane. Scenario B (unfixable): the finding SURVIVES the sweep -
  no fixed version at all, or a fix outside the current version
  requirements that `cargo update` cannot take - the job fails hard
  with the CALL OWNER banner and the commit job never runs.
- The hard kind set widens to the estate's mature vocabulary:
  vulnerability, unmaintained, unsound and yanked all gate when they
  survive the sweep; informational notices never gate. The yanked
  kind keys on the exact release (the identity of a yanked version
  is that version), while every other kind keys across version moves
  - an update fixes BY moving the version.
- The verdict now runs unconditionally between the refresh and the
  diff gate: a sweep that produces no diff still owes the owner a
  verdict, so a finding the sweep cannot clear can never hide behind
  a changed-only step condition. The pre-pass gates nothing and
  exits 1 only on an infrastructure failure - a scanner that cannot
  run fails the lane, never passes it.
- The scanner is pinned exactly like the sibling's:
  `cargo-audit@0.22.2` via the install action, so the resolved
  binary never drifts under the lane - red means a new advisory,
  never a new scanner release.
- Every surface signs the sentinel: the step summary title
  ("AXP1-ZnQ supply-chain verdict (night-solid-defense-1)"), the
  FAIL banner ("AXP1-ZnQ SUPPLY-CHAIN VERDICT: FAIL - CALL OWNER"),
  the holds-the-lane block, the pre-pass count line, the
  `::error title=AXP1-ZnQ::` and `::warning title=AXP1-ZnQ::`
  annotations, and the pass line naming the workspace count, the
  cleared count, the hard count and the informational count. The
  raw per-workspace JSON reports land in the run scratch state so a
  red Sunday can be post-mortemed without re-running the scanner.
- deny.toml and SECURITY.md, scripts/README.md: the same contract
  recorded where it is enforced and documented.

Verified: the built-in self-test passes 17/17 (extraction across
both warnings container shapes, the locked-version row, kind
vocabulary, scenario keying including the yanked exact-release key,
the cleared/survived diff, exit mapping, survival messages, TSV
round-trip, parser noise tolerance, summary identity and counts);
ruff check and format clean; yamllint clean on the edited workflow.

### fix: night-solid-defense-1 - the verdict reads the pinned scanner's real report shape

- The owner's anti-gimmick law (verify complete, prove 100 percent
  real) caught this before CI did: the pinned cargo-audit 0.22.2
  binary was run for real against the real advisory database, probed
  with live pins (the documented arrayref 0.3.10 malicious release,
  time 0.1.45 under RUSTSEC-2020-0071, instant 0.1.13 under
  RUSTSEC-2024-0384) - and the probe exposed that the pinned scanner
  emits a flat top-level report (`vulnerabilities`, keyed `warnings`
  groups), not the pre-0.22 `results.list` container the first draft
  parsed. A clean tree masked the gap completely: zero findings
  parse identically under both shapes, so only a real advisory would
  have produced a silent false PASS. The parser now reads both
  shapes, and the self-test replays the probe's exact report shapes
  (19/19 checks).
- The full lifecycle is proven against the real scanner, not
  fixtures alone: the pre-pass stores 3 real findings; the verdict
  fails hard with the CALL OWNER banner and exit 1; a sweep that
  takes the time and arrayref fixes reports them cleared while the
  unmaintained survivor still fails (both scenarios in one verdict);
  a fully cleared lane passes with the cleared count. The step
  summary renders the advisory tables with live RustSec links.

Verified: self-test 19/19; ruff clean; gate-keepers 25 passed,
0 failed, 3 skipped; real-run evidence in /tmp/axp1-probe (probe
report, lifecycle summaries) produced by the official
cargo-audit-x86_64-unknown-linux-musl v0.22.2 release binary.

### fix: night-solid-defense-1 - the deny tier contract key is gone from cargo-deny, so the gate agrees with the verdict by construction

- CI watch on the estate-sentinel commit caught a red lane (Cosmic
  Dragon Guard - CI, run 44935665237): the Dependency policy
  (cargo-deny) job died at config validation with
  "error[deprecated]: this key has been removed" against the
  `vulnerability = "warn"` line the ownership-split commit added -
  current cargo-deny releases deleted the per-class severity level
  keys outright (EmbarkStudios/cargo-deny PR #611), so the key was
  valid last week and is a config error today. The maintenance lane
  itself stayed green because its deny pass is changed-gated and the
  Sunday tree moved no locks; ci.yml runs deny unconditionally and
  caught it, exactly what the watch is for.
- The fix removes the key and the now-impossible ownership split:
  under the mature verdict design the AXP1-ZnQ verdict runs
  upstream of the policy check and fails hard on any finding the
  sweep cannot clear, so deny and the verdict agree by
  construction - no per-class override is needed or possible.
  deny.toml keeps the schema the CI lane accepts (yanked = "deny"
  untouched, the arrayref defense), and both deny.toml and the
  maintenance.yml policy step now record the real contract plus the
  incident.
- Proven with the real tool, not assumed: the official
  cargo-deny 0.22.0 musl release binary validates the fixed config
  with zero errors and, on the same binary, the old config
  reproduces the exact CI error - a differential check that also
  proved config validation passes before the cargo metadata stage
  the sandbox cannot run.

Verified: real-binary A/B (old config: error[deprecated] +
validation failure; fixed config: zero validation errors);
yamllint clean on the touched workflow comment; gate-keepers
25 passed, 0 failed, 3 skipped.

### feat: night-remake-1 - the estate contact purge: every surface answers to the proton address

- The legacy consumer-mail contact address is purged end-to-end and
  replaced with
  `oxyzenQ@proton.me` across every surface that carried it, plain or
  obfuscated: the README contact line, the COMMERCIAL_LICENSE
  licensing blocks (both the body and the contact list), the
  SECURITY reporting address, the CLA contact lines (both), the
  AI_AGENTS_READ_THIS_FILE pointer, and the crate metadata —
  `Cargo.toml` now reads
  `authors = ["rezky_nightky (oxyzenQ) <oxyzenQ@proton.me>"]`.
  The AUR PKGBUILD maintainer header carried the same address in
  its obfuscated no-scrape spelling; it now carries the proton
  address directly. A historical CHANGELOG
  sentence that quoted the obfuscated spelling was rephrased to
  describe the purge instead of repeating the address.
- The GPG verification story stays fingerprint-anchored
  (`F5324E0967F104D58CE025F347A50AEF4B65AAC2`, unchanged) and now
  lists the proton UID — `rezky_nightky (oxyzenQ)
  <oxyzenQ@proton.me>` — as the primary contact UID in
  docs/VERIFY_RELEASE.md, alongside the noreply UID that signatures
  report. The keyserver remains the source of truth for the live
  UID set.
- Scope honesty: the runtime surfaces were already clean — the
  `sig` report's contact line has shown the proton address since
  the masked layers landed, and `-V` carries no email at all (the
  frozen provenance layers never contained one). The frozen claim,
  both fingerprints, and the blake3 anchor are untouched by this
  purge, so `sig` still verifies 5/5 AUTHENTIC. Git author and
  committer emails on this repository were always the noreply
  address; the purge is a content sweep, not a history rewrite (a
  blob-level history purge is an owner-level destructive operation
  and was not performed). A pre-existing permission violation the
  gate caught during the sweep (benchmark/bench-labs/BENCH_LABS.md
  at 600) is fixed to the 644 standard.

Verified: full-residual scan zero hits across plain, obfuscated,
and alternate-domain variants (case-insensitive, hidden files
included); full test suite 3087/3087 green; `cosmostrix -V` and
`sig` byte-proofed on a live build (yearless Signature line,
5/5 AUTHENTIC, contact `oxyzenQ@proton.me`); fmt, clippy-free
build, header gate 919/919, permissions gate 1249/1249, and the
docs/language/emoji/hype/comment-style/name-case gates all clean.
Version untouched (v100.1.0-night.1).

### feat: night-ux-orion-3 - the yearless Signature line: the identity display law reaches -V

- The `-V/--version` Signature line renders the identity without the
  trailing year: `Signature: Cosmic Dragon — Official Build by
  rezky_nightky (oxyzenQ)` — the Masterclass year order (the year
  belongs in FRONT, the Copyright line's `2025-2026` range, never
  behind the handle; the owner's spec line for the estate). The new
  `manifest_display` helper in signature.rs strips the embedded year
  at the single render site, next to `identity_core` (which strips
  the same year for the bare core the `sig` block shows) — one year
  law, two surfaces. The frozen claim itself is untouched: the masked
  layers, both fingerprints, and the blake3 anchor are byte-identical,
  so `sig` still verifies 5/5 AUTHENTIC and the binding law still
  holds. Test oracles updated to pin the yearless line and the
  absence of the trailing year across every `-V` surface.
- Estate scan for the same shape: no other display surface renders
  the year-stamped form (the `sig` identity block is yearless since
  the orion-2 completion; the Depth-matrix line never carried a year;
  the README and INNOVATIONS references describe the frozen embedded
  claim's bytes, not an output, and stay accurate).

Verified: diagnostics suite green (the -V oracles, the binding law,
the sig block), `cosmostrix -V` byte-verified on a live build (the
yearless Signature line, the Masterclass three-line block intact,
`status=verified`), `sig` five-depth AUTHENTIC with the yearless
identity and contact line, fmt/clippy clean. Version untouched
(v100.1.0-night.1), permissions 644/755.

### feat: night-ux-orion-2 - the Masterclass license disclosure: the -V identity block and the estate-wide SPDX header standard

- The `-V/--version` identity block lands the Masterclass format the
  owner specified: `Copyright (c) 2025-2026 rezky_nightky (oxyzenQ)`
  (no colon — the Linux kernel/rustc spelling; the year range rides a
  new `COPYRIGHT_YEAR` constant, bump the back year on new-year
  releases, freeze forever on acquisition), `License: AGPL-3.0-only
  OR Commercial`, and the existing `Source:` line. The Copyright line
  drops the masked email — the name + handle IS the identity.
- The masked authors provenance layer (D1) rotated to the Masterclass
  short form via the generator (`scripts/release/
  generate-provenance-words.py` truth edit + re-run + paste):
  "rezky_nightky (oxyzenQ)", 23 bytes / 3 words, new FNV fingerprint
  (b1a27b10a64c879a) and new blake3 DEPTH_ANCHOR (b3:4c1738aa...).
  The manifest layer (D0) is untouched and the identity-binding law
  still holds (the manifest's identity core is a prefix of the new
  authors string). Test oracles updated to the generator's printed
  cross-check values; the anchor payload framing test pins the new
  64+23 layout.
- Cargo.toml carries the machine-readable dual-license expression:
  `license = "AGPL-3.0-only OR LicenseRef-Commercial"`. The bare word
  "Commercial" is not a valid SPDX term (proven: cargo-deny rejects
  it with "unknown term", which would also break the registry field);
  `LicenseRef-Commercial` is the SPDX-standard machine name for a
  private license and passes cargo-deny licenses + cargo package.
  The human-facing surfaces render the clean "AGPL-3.0-only OR
  Commercial" from the new `LICENSE_LINE` constant (the single
  telling for -V and --docs). The author contact carries the raw
  address per the owner's Masterclass metadata set, superseding the
  earlier obfuscated spelling (the comment law follows the field).
- The `sig` identity block reached the owner's final Masterclass
  shape: the runtime-identity depth row and the identity field render
  the yearless identity core (`rezky_nightky (oxyzenQ)`, via the
  shared `identity_core` helper, now `pub(crate)` — the one telling of
  the marker+year shape; the year stays in the frozen embedded claim,
  never in the handle the report shows), and the identity block gained
  the owner's `contact` field (`contact     oxyzenQ@proton.me`) from
  the new `CONTACT` constant — the same public contact the estate's
  proton contact layer carries. The D1 binding law is untouched (the
  coherence check still strips the manifest year through the shared
  helper).
- The header gate hardened against malformed expressions: the
  substring law alone let a doubled tail through
  (`AGPL-3.0-only OR Commercial OR Commercial`, which is how the one
  occurrence in `scripts/gate-keepers.sh` survived — fixed here). The
  gate now also rejects any identifier line carrying trailing content
  after the Masterclass pair, so the value must END with the exact
  expression (adversarially verified: the doubled form is caught, the
  clean form passes untouched, 919/919 green).
- The estate-wide SPDX header standard: all 919 tracked header-bearing
  files (.rs, .sh, .py, .toml, .yml, .yaml, .md, .c, .gitattributes,
  .markdownlint.yaml) now carry the Masterclass pair —
  `SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)` +
  `SPDX-License-Identifier: AGPL-3.0-only OR Commercial` (markdown in
  the owner's HTML-comment block form). The header gate enforces the
  full expression (substring law: a file still declaring the bare
  AGPL rung fails). NOTICE, the README IP statement, and the in-body
  "All rights reserved." lines migrated to the same year/name form;
  the sibling estate notes updated where they documented the old
  shared 57-byte authors layer.

Verified: diagnostics suite 47/47 green (the provenance rotation's
oracles, binding law, tamper probes, runtime blake3 recompute),
`cosmostrix -V` byte-verified on a live build (the three Masterclass
lines exact, `status=verified` on the new anchor, `sig` five-depth
AUTHENTIC), cargo-deny licenses ok, cargo package ok, header gate
919/919, fmt/clippy clean. Version untouched, permissions 644/755.

### feat: night-ux-orion-1 - the loading lane: a DNA helix for the silent fetch wait

- The `--check-update` fetch (the CLI's one indeterminate wait)
  narrates instead of sitting silent: a background redrawing thread
  paints one grey stderr line - `[Fetching latest release info... ⠋⠏]`
  - while the fetcher child blocks, and erases it before the verdict
  prints. The gate is a terminal stderr; a piped run never sees a
  narration byte.
- Lazy threshold (the owner's auto-detect law): an operation that
  lands inside 100 ms never draws - the fast path's stderr stays
  byte for byte silent, and the indicator appears only when the
  wait is real. The slot clock is deterministic (a pure function of
  elapsed time), so a late redraw repeats a frame instead of
  skipping the sequence.
- The DNA helix: two braille strands running the same ten-frame
  sweep in opposite directions (`HELIX[i]` pairs `SWEEP[i]` with
  `SWEEP[9 - i]`) - the two anti-parallel streams the fetch is
  (the request out, the answer back). The single-strand wave seat
  (`Style::Wave`) exists for the estate's next linear site.
- The fallbacks are detection, never flags: NO_COLOR or a non-UTF-8
  locale drops to the rotating ASCII dash automatically (`-`, `\`,
  `|`, `/`), and the paint rides the existing capability ladder
  through the new `output::grey_err` (truecolor #8B8B8B, 256-color
  index 245, bright black on Color16, plain on mono) - visible by
  construction on dark and light backgrounds alike.
- Byte-contract hygiene: overwrite and clear ride carriage returns
  and spaces only (no CSI erase), every write failure is swallowed
  (the indicator must never be the reason a run dies), and the
  clear blanks cells, not bytes (braille is three bytes per cell).
- Single-source law kept: the benchmark lane's frames re-point to
  the same `SWEEP` table the loading lane renders, so the two
  lanes can never drift; `spin()` now returns the frame cell
  (`&str`) and every display site keeps its format string.
- The old always-draws spinner (night-dragon-maintenance-4) is
  replaced wholesale; the machine's nine new tests live in the
  mirrored `test/output/spinner_tests.rs` (helix table pins, the
  style ladder, the Unicode probe, the lazy slot clock, the
  bracket grammar, the inert/drawn/undrawn machine contracts, the
  cell-width law), plus a `grey_err` ladder pin in grey.rs.

### test: night-frontier-Z8 - the control-loop resonance census: the Power Dragon's coupled backoff axes get their no-limit-cycle proof

- `test/central_control_power_dragon/power_manager/tests_resonance_z8.rs`
  drives the coupled controllers through the adversarial scenario
  matrix (the pacing sawtooth entry 46 measured, the byte sawtooth
  the reader model damps, the combined storm with a thermal pulse
  train, and a clean release window) and asserts at every frame:
  boundedness (both backoffs, the pressure accumulator, the visual
  EMA inside their closed intervals; effective_fps between the
  floor and the base; every signal finite), release (each axis
  decays into the healthy band within the closed-form fall-rate
  frame count; effective_fps returns to base), and
  non-amplification (the ceiling ratchet rides a period-2 ripple
  of exactly one fall step - the honest shape the census found and
  pinned, instead of a pretended flat line).
- Teeth, proven: the storm's raw unclamped accumulation exceeds the
  ceiling (30 frames x 0.05 = 1.5) while the observed value reads
  exactly 1.0 - the clamp is load-bearing, the assertions guard a
  real edge.
- One cfg(test) read-only accessor added
  (reader_backoff_for_test, the visual_pressure precedent); the
  runtime render path and the runtime behavior are untouched.
- Local verification: the lock_ filter passes 153/0 (four census
  scenarios plus the standing locks), fmt-clean, clippy-clean
  across all targets. The filter rides the MSRV lane, the musl
  shards, and the arm64 parity lane on every push. No version bump,
  no dependency change.

### test: night-frontier-Z8 - the conservation-ledger family: six solvers, six named discrete contracts (INV-20)

- `test/engine/cosmic_dragon_engine/cloud/tests/tests_conservation_z8.rs`
  gives every original motion-math system its honest contract, named
  and closed-form: Keplerian orbits get the angular-momentum ledger
  (the infall velocity update is central, so the semi-implicit Euler
  ordering conserves L = x*vy - y*vx EXACTLY per substep outside the
  accretion brake - asserted to f32 rounding on a near-circular orbit
  plus a cloud-level bounded-drift pass; the brake stays disclosed as
  the designed L sink); PIC/FLIP gets the pool ledger (the active
  count is the bookkeeping truth and the speed clamp a hard ceiling,
  every frame); Physarum gets the mass-balance budget (the trail
  total sits inside the closed-form deposit/decay window every
  frame); boids get the flight-band ledger (every active bird inside
  the documented band, no teleport beyond the band's step allowance);
  Lorenz RK4 gets the absorbing-set contract (the honest scope - a
  chaotic system has no monotone energy ledger, so the measured
  envelope plus absorption promptness IS the contract); FABRIK chains
  get the spacing ledger (every segment lands EXACTLY at
  DRAGON_SEGMENT_SPACING from its predecessor, every frame).
- Teeth, proven: the kepler drift helper detects a deliberately
  injected non-central kick - a ledger that cannot see a violation
  cannot enforce one.
- Honest scope: the ledgers read state through the family's existing
  pub(crate) surface (flux motes, lorenz motes, birds, dragons were
  already exposed; physarum's trail field joins the same convention -
  it was the style family's only private state); the runtime render
  path is untouched (entries 32 and 36 see no new surface); the
  bounds are physics bounds, not bit-exact pins.
- Doc truth sync: entry 24's FABRIK mapping read `dna_helix` while
  the solver lives in the dragon style's body chain - the code is
  the truth, corrected in the same landing (INNOVATIONS entry 53
  records the fix).
- Local verification: the lock_ filter passes 149/0 (eight new family
  pins plus the standing locks), fmt-clean, clippy-clean across all
  targets. The MSRV lane, the musl shards, and the arm64 parity lane
  run the filter on every push. No version bump, no dependency
  change; the cfg(test) accessor is the only src touch.

### ci: night-frontier-Z7 - the cross-architecture oracle parity: the pinned engine vector is recomputed on real ARM silicon, and a musl needs bug is fixed

- supermassive.yml grows two legs that close the architecture axis
  the Z1-Z6 proofs never crossed. `sig_parity` runs `sig` on the
  release shape on BOTH arches (x86_64 and the ubuntu-24.04-arm
  runner) and requires exit 0, the five-of-five
  `SIGNATURE: AUTHENTIC` verdict, and the D4 engine-oracle pass row -
  the same compile-time pinned vector the x86_64 lanes
  cross-verified (debug-glibc vs the v3-musl release shape, the
  ci.yml cross-libm leg) now recomputed on real ARM silicon by the
  shipped aarch64 musl binary; a mismatch reports TAMPERED and fails
  the leg, which is the moat working (either outcome is the contract
  functioning), and the sig report uploads so the recomputed digest
  is quotable per arch.
- `arm_lock_suite` runs the `lock_` invariant families (the cosmic
  locks including the zero-alloc ledger INV-18 and the fault-floor
  ledger INV-19, the chroma and crystal locks) under the SHIPPED
  aarch64-unknown-linux-musl target on the ARM runner - the deeper
  half `sig` cannot reach, since the oracle and replay module tests
  do not carry the `lock_` prefix (the D4 vector's in-suite
  cross-check stays with the ci.yml lanes). The compile rides the
  musl_compile levers (line-tables-only, incremental off) with a
  15-minute ceiling (the ARM runner is the slower pole; the
  five-minute law covers battery legs, not the one-compile job).
- Honest scope, recorded in the workflow header and INNOVATIONS
  entry 52: the android shape (pro-android-aarch64) cannot execute
  in CI (a bionic-linked binary does not run on a plain Linux arm
  host), so its parity inherits from the arm64-linux proof plus
  strict-IEEE, and any divergence surfaces on-device through
  `--doctor`'s engine-oracle row.
- A pre-existing workflow bug is fixed in the same landing:
  musl_shards declared `needs: usl_compile]` - a stray bracket made
  the reference a nonexistent job name; the platform tolerated it,
  but the ordering was never guaranteed by spec. The declared
  dependency is now the real one, `musl_compile` (verified by the
  workflow YAML parse: the job list and both `needs` edges).
- Docs: INNOVATIONS.md grows entry 52 (the fifty-two);
  PROOF_ATLAS.md routes row 52 and updates the supermassive
  standing-lane row. No version bump, no dependency change; the
  workflow is the change.

### test: night-frontier-Z7 - the fault-floor ledger (INV-19): the lock suite now proves what the kernel grants, not only what Rust requests

- `test/cosmic_dragon_incubator/tests/fault_ledger.rs` adds INV-19
  over the SAME pinned steady-state window INV-18 uses (the rig is
  shared verbatim): the calling thread's page-fault counters from
  `/proc/thread-self/stat` (per-thread, no allocator hook, no unsafe)
  must read EXACTLY ZERO minor and zero major faults across the 180
  lean frames. The proof rises from "Rust requested nothing" (the
  counting allocator, Kernel Floor 2b) to "the kernel granted
  nothing" (Kernel Floor 2c) — a regression class the allocator
  ledger is structurally blind to (stack growth past the warmup
  high-water mark, arena growth outside the counted calls,
  hook-bypassing allocations) now trips CI.
- The reader is deliberately heap-free (one `File::read` into a
  fixed stack buffer, parsed in place): the first landing's
  `read_to_string` reader measured exactly one arena fault under
  libtest's parallel shared-process execution — precisely the blind
  spot the ledger closes — so the measuring path itself cannot fault
  after the stack primes, and the window measures the frames, not
  the measuring.
- Teeth, proven both directions: the positive control touches 2 MiB
  of fresh pages mid-window and requires the minor count to rise (a
  floor bound, not an exact count — the exact page count of one
  allocation is the kernel's and the allocator's to choose). Local
  verification: the `lock_` filter passes clean end to end, both
  INV-19 directions plus the INV-18 regression check on the shared
  rig; the MSRV lock lane runs the same filter on every push, and
  the supermassive arm64 lane runs it on real ARM silicon.
- Honest scope: the LEAN path (INV-18's exact scope) on Linux; other
  hosts keep the INV-18 allocator contract; exotic environments
  (NUMA page balancing) can inject faults no allocator can cause,
  and the failure message names the counters and the suspected cause.
- No version bump, no dependency change, no runtime code touched:
  the shipped binary is byte-identical. docs/INNOVATIONS.md grows
  entry 51 (the fifty-one); docs/PROOF_ATLAS.md routes row 51.

### build: the release-ledger shape gate joins the push battery (gate-keepers check 22)

- `scripts/gates/check-release-ledger.sh` asserts the writer's own
  shape laws against the real `CHANGELOG.md`, read-only: heading
  count (one `## Released`, one `## Unreleased`), Released above
  Unreleased, the blank spacer, purity inside the section, no
  version line stranded outside it (the incident class), no
  duplicate release lines. The probes are sourced from
  `scripts/lib/changelog-ledger.sh` - one implementation, no copied
  grammar.
- Registered as gate-keepers check 22 and as a step in the ci.yml
  Version sync guard job, so hand-edit drift between bumps fails
  the push instead of waiting for the next release to publish it.

### fix: the release ledger verifies its own writes - the bump's version line can no longer land anywhere but inside ## Released

- `scripts/dev/version-to.sh` ports the zelynic NIGHT-bump-1 laws -
  the fatal class the owner hit on the v80.0.x line: the bump's
  version line rode below the unreleased section because nothing
  ever verified where a write landed. The ledger work moved out of
  the bump's tail into a verified pre-flight
  (`scripts/lib/changelog-ledger.sh`) that runs before any file is
  mutated: the shape law (one `## Released` heading above one
  `## Unreleased`, the blank spacer, bare version lines only - a
  Released block below Unreleased relocates, foreign content
  refuses), the semver law (a bump below the ledger's top refuses
  as the stale-tree downgrade; `--force` records a deliberate
  release-line reset), and the verification law (after any splice
  the file is re-parsed and the line must sit inside `## Released`
  exactly once with Released above Unreleased - any other outcome
  restores the backup and fails the run). A release line stranded
  outside `## Released` heals in place: the strays are deleted and
  the verified splice records the line where it belongs.
- `--check` verification rides the same law: the version line is
  probed positionally inside `## Released`, not grepped anywhere in
  the file - a stranded line fails the check instead of passing it.

### message: default overlay text now reads "Experience with cosmostrix"

- `default_message_text()` (src/types/constants.rs) reworded from
  `"Experience cosmostrix v<CARGO_PKG_VERSION>"` to
  `"Experience with cosmostrix v<CARGO_PKG_VERSION>"` — the default
  bordered overlay in interactive mode reads naturally now. All
  dynamic consumers (startup verbose output, live-reload fallback,
  stuck-cell sweep fixtures) pick the new string up through the
  function; no call site hardcoded the old text.
- Comment and doc references synced so nothing stale survives:
  the generated configfile template, the USER_CONFIG_KEYS comment,
  the phosphor_anomaly wiring note, the live-reload rejection test
  comment, the hunt-36 module doc, and the LIVE_RELOAD_BEHAVIOR.md
  precedence list. The README tagline (`Experience cosmostrix.`)
  is marketing prose, not the runtime default, and stays as-is.
- A/B bench (10 s, cinematic + monolith, 2 runs each,
  `benchmark/bench-labs/night_xenric_msg/`): statistical tie on every
  metric — bench mode never renders the overlay
  (`message_active = false` on all eight runs), so the change is
  rendered-output-only by construction.

### audit: NIGHT-xenric-1 — engine ultra-long-horizon endurance depth audit

- Audit report lands in
  `docs/audits/NIGHT_XENRIC_1_ENGINE_ENDURANCE_AUDIT_2026-11-02.md`:
  the render engine audited against a "useful for millions of years"
  bar, extending the 2026-09-21 overflow audit with a horizon ladder
  and a repo-wide census of session-anchored float conversions.
- Verdict: all surfaces verified at peak, zero code changes. The
  three session-anchored elapsed consumers use the correct
  representation (f64 bucket math, f64 phase with modulo wrap before
  the f32 handoff, u32 hash-only seed); every per-entity timer is
  re-anchored or envelope-bounded; every hot collection is capped,
  drained, or equilibrium-bounded; the self-healing layer covers
  transient corruption. First math wall past the ~585-year OS clock
  envelope is f64-ms precision at ~285,000 years — nothing to fix.
- Record-only round (owner rule: audit-if-peak-skip; docs-only
  changes skip the benchmark).

### license: full relicense from GPL-3.0-only to AGPL-3.0-only

- The open-source license is now AGPL-3.0-only (the license that
  already governs the sibling project). LICENSE now carries the
  verbatim GNU Affero General Public License v3 text; the
  dual-license statement lives in README.md, NOTICE,
  COMMERCIAL_LICENSE.md and docs/LICENSING_FAQ.md exactly as before.
- Every SPDX header across the tree updates from
  `GPL-3.0-only` to `AGPL-3.0-only` (866 files: src, test, scripts,
  docs, benchmark, fuzz, pgo-runner, .github, .cargo, assets). The
  `scripts/gates/check-headers.sh` gate now expects and enforces
  `AGPL-3.0-only`.
- Manifest and packaging follow: `Cargo.toml` `license` field (so
  `cosmostrix --version` and `--doctor` report the new SPDX
  expression via `CARGO_PKG_LICENSE`), `deny.toml` gains
  `AGPL-3.0-only` in the allow list (the workspace crate is part of
  the lock graph cargo-deny scans; the GPL-3.0 entries stay for the
  dependency graph), and the AUR package updates both `PKGBUILD`
  and `.SRCINFO` to `AGPL3`.
- Living docs restate the current terms under AGPL naming:
  README.md (license section, pricing table, IP notice),
  COMMERCIAL_LICENSE.md, docs/LICENSING_FAQ.md, TRADEMARK.md,
  CLA.md, CONTRIBUTING.md, docs/SECURITY_AUDIT.md,
  AI_AGENTS_READ_THIS.md, NOTICE. Historical records
  (archived changelog eras, docs/archive/audits, docs/research,
  benchmark research notes) keep their period-correct prose; only
  their current-law SPDX headers changed.
- The Commercial License path is unchanged: same tiers, same
  pricing, same contact. No version bump rides this commit; the
  owner decides when the license change ships in a release.

### License

- **OEM perpetual path added** — the OEM tier now offers the same two
  paths zelynic ships: annual ($149,999/year) or perpetual ($249,999
  one-time + 20%/year maintenance). Both grant the same redistribution
  rights. Business ($12,999), Enterprise Internal ($64,999), Individual
  ($99) unchanged.
- **Professional tier added** ($1,899/year) — teams up to 5 seats,
  revenue under $500K, priority support with SLA. Fills the 131x
  jump from Individual ($99) to Business ($12,999), which left
  small studios and creative teams without a fitting tier. Zejtron
  already has the analogous tier at the same price; this brings
  cosmostrix to portfolio parity. Individual $99 and Business
  $12,999 are unchanged (byte-identical).

### audit: night-dragon-lts-3 / lts-4 — five-rubric depth audit; three fix commits and one honest report

- Audit report lands in
  `docs/audits/NIGHT_DRAGON_LTS_3_AUDIT_2026-10-31.md`: panic surface
  verified empty, zombies removed, security amplifiers closed, and the
  skip-if-peak verdicts (optimization, concurrency, terminal restore)
  recorded with their reasons.
- Three code commits ride the round: the last production
  `unreachable!` defused plus a `cdf_select` contract pin; verified
  zombie removal (`Cloud::set_async`, the `DRAGON_ACTIVE_*` trio) and
  a 22-line byte-identical Levenshtein consolidation; white-hat
  hardening (Levenshtein 128-byte input cap with sentinel, O_EXCL
  atomic config write, capped `--compare-baseline` read, clamped
  bench warmup).
- Verified per commit: check/fmt/clippy `-D warnings` clean, 110
  targeted unit tests, gate-keepers 20 passed / 0 failed, and a 10 s
  A/B benchmark (base vs HEAD binary) showing identical visuals and
  noise-level perf deltas.

### release: PGO nitro release packages retired — the static musl pair is the full shipped matrix

- `release.yml` drops the `build_pgo` job: the two nitro PGO release
  packages (`cosmostrix-<tag>-linux-amd64-v3-musl-pgo.tar.gz` and the
  v4 twin) are no longer built, signed, or published. Reason: the PGO
  performance edge is not needed in the shipped set and long stability
  wins — every release minute goes to the seven standard packages.
- The publish job's `needs` list drops `build_pgo` accordingly. The
  shipped matrix is 7 archives across 7 OS/arch targets: the static
  musl v3/v4 pair, darwin aarch64, windows x64/arm64, freebsd amd64,
  android aarch64.
- The PGO nitro build stays a LOCAL capability: `./scripts/build.sh
  pgo` (instrument -> benchmark -> optimize), the `pgo-instrument` /
  `pgo-use` cargo profiles, the `pgo-runner` crate, and the `pgo:
  yes/no` doctor line are all untouched. Only the release packaging is
  retired.
- Stale release-matrix doc numbers corrected while auditing the
  removal: MAINTENANCE.md said "11 release archives" (the real count
  was 9) and listed a "linux aarch64" leg that does not exist;
  ABOUT_CI.md said "8-platform binaries" and carried the same phantom
  "Linux aarch64" leg while omitting the FreeBSD release lane.

### license: pricing ladder revision — flagship-premium 5-tier model

- Business raised from $5,199 to $12,999/year; Company tier split
  into Enterprise Internal ($64,999/year, internal-only) and
  OEM / Redistribution ($149,999/year, embedding rights) — the
  previous Company tier conflated internal enterprise use with
  redistribution rights.
- Grandfathering clause added to COMMERCIAL_LICENSE.md: buyers who
  paid before a price move keep their purchased price for their
  purchased term; renewals after term end follow the current table.
- Value Justification section added to COMMERCIAL_LICENSE.md,
  grounding the price in measurable surface (code depth, doc depth,
  tooling depth, six original motion-math systems, three locked
  engines, fourteen rain styles, five-OS cross-platform, seven
  licensed patterns, PGO nitro build).
- README `## Commercial Licensing` table synced to the 5-tier
  ladder with a pointer to the canonical table in
  COMMERCIAL_LICENSE.md.
- docs/LICENSING_FAQ.md synced: the tier-selection, pricing, and
  rebranding answers name the new tiers, and the Company-use
  section becomes Enterprise use.
- Docs-only change: no source, build, or test surface touched, so no
  A/B benchmark per house rule.

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

### build: night-improve-9 — version-to.sh writes the release ledger and adopts the pgo-runner version

- The zelynic NIGHT-improve-76 shape lands: every bump records the
  release line in the root CHANGELOG.md — a bare `vX.Y.Z` line under
  a `## Released` heading above `## Unreleased`, newest first,
  pre-release and stable alike, created on the first bump. The lane
  probes honestly: a missing file, a missing `## Unreleased` anchor,
  or an unexpected ledger shape warns and writes nothing — a ledger
  this script did not create is not one it rewrites.
- The zelynic NIGHT-improve-67 shape lands with it: pgo-runner (the
  standalone `cargo use-pgo` crate, cosmostrix's answer to zelynic's
  ebpf crate) rides the root version from the next bump on — the
  manifest and its own name-anchored lock entry both — instead of
  shipping a drifting 0.1.0 label beside a v100 binary. While it
  sits at its birth version, `version-to.sh --check` and
  `build.sh version-sync` report the pending adoption instead of
  failing, so CI stays green until the owner's next real bump.
- `version-to.sh --check` grows the matching verification steps
  (manifest and lock agreement, ledger line once the section
  exists); `scripts/lib/version-sync.sh`, `scripts/README.md` and
  this file's own prose document the new active-version surfaces.
- Dev tooling only: no runtime code path touched, no A/B benchmark
  per house rule.

### build: night-improve-11 — the package manifest completes the zelynic masterclass package data: the docs.rs documentation link joins the readme pointer

- The `[package]` block gains its last missing masterclass field
  (the zelynic NIGHT-improve-77 lineage):
  `documentation = "https://docs.rs/cosmostrix"`. Verified live
  before the edit: the crate is published (13 registry versions,
  100.0.7 the head) and the docs.rs rustdoc tree is hosted and
  green - docs.rs/cosmostrix resolves HTTP 200. The field is the
  version-less base URL so it always tracks the newest hosted
  build: crates.io renders it in the crate-page sidebar and the
  index API exposes it to every consumer.
- It completes the parity set that landed with the first half of
  this task: `readme = "README.md"` (the pointer that renders the
  whole crate-page body, guaranteed to ride the registry tarball
  even under this manifest's exclude lane) and the author entry in
  the era's obfuscated no-scrape contact spelling (the address it
  carried is purged estate-wide by the night-remake-1 proton
  contact), accepted by crates.io in
  the author `<...>` slot. The manifest now carries the full
  published-crate set: description, five keywords, four validated
  category slugs, license, repository, documentation, readme,
  rust-version, edition.
- Page-data only: no build, test, or version-sync surface reads
  any of the three fields (grep-audited across build.rs and
  scripts/ - metadata.rs asserts versions only). The pgo-runner
  manifest stays untouched on purpose: `publish = false`, never
  hosted on docs.rs, a documentation link there would be a dead
  URL.
- Verification: `cargo verify-project` success, `cargo
  read-manifest` reads the three fields back, `./scripts/build.sh
  check-all -q` green, codespell clean on both touched files.
  Manifest page-data only - no runtime code path touched, no A/B
  benchmark per house rule.

### fix: night-improve-12 — the demo gif retirement lands with its test guards: the two red CI lanes go green

- v100.0.8 removed the deprecated demo asset
  (assets/cosmostrix-video.gif, commit 7014e8f) but left two
  surfaces keyed to it, and both CI lanes that run the bin test
  harness went red (Tests nextest partition 1/2 and the
  supermassive musl workspace nextest):
  docs_tests::assets::video_assets_exist still asserted the gif
  exists alongside the webp.
- The guard follows the retirement: `video_assets_exist` asserts
  the webp only, the `VIDEO_GIF` constant is gone, and the module
  doc records why (the webp encodes straight from the webm source,
  so the gif intermediate no longer ships).
- Stale-reference sweep beside the failing test: assets/tools.md
  drops the gifski recipe (the tool block keyed to the deleted
  file; the ffmpeg webp recipe stays), and the remaining
  cosmostrix-video.gif mentions live only in docs/archive/ history
  files, which stay untouched on purpose.
- Verification: `cargo test --bin cosmostrix docs_tests::assets`
  green (7/7 guards), `cargo fmt --all --check` clean, clippy
  `--bin --tests` zero warnings, codespell clean on both touched
  files. Test-guard sync only - no runtime code path touched, no
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
