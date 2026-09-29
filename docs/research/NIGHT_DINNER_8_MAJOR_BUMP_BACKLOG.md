<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-dinner-8 — the major-bump backlog burn-down: signal-hook 0.4 lands compiler-proven, rand 0.10 gets the determinism treatment, and the dragon's heart stays untouched

Owner directive (2026-09-30): "ya gue approve ini sampai nothing
remainings: Backlog major bump tinggal signal-hook 0.4 (1-2 jam) dan
rand 0.10 (4-6 jam + kampanye A/B) — tinggal bilang kalau mau lanjut."

The directive continues the NIGHT-dinner-5 relax policy under the
dragon boundary stated there: the skin may be upgraded, the heart is
never edited — a dependency change is allowed exactly as far as it
never alters the critical core engine (the render engines and their
frame path), and for rand, the one crate whose output IS the picture
on screen, "never alters" is proven with a determinism check plus an
A/B benchmark campaign, not assumed.

## Task state at start (2026-09-30)

| Item | Status at task start | Est. effort (audit table) |
|------|---------------------|---------------------------|
| notify 8 | DONE 2026-09-29 (NIGHT-dinner-5 follow-up — pin bump only) | — |
| signal-hook 0.4 | pending | 1-2 h |
| rand 0.10 | pending | 4-6 h + A/B campaign |
| sha2 0.11 | HOLD (owner decision, revisit quarterly) | — |

## Part 1 — signal-hook 0.3.18 → 0.4.4: the third pure pin bump

### The claim versus the compiler

The dependency-audit table carried two unverified claims for this
migration: "signal-hook 0.4 changed the iterator::Signals API and
low_level module" and "the flag::register signature may have
changed." The upstream changelog says otherwise: 0.4.0's single
breaking change is `low_level::pipe` taking `OwnedFd` instead of
`IntoRawFd` (signal-hook#196), and the 0.4.1-0.4.4 line is fixes and
packaging only. cosmostrix's entire call surface is five sites in two
files:

- `signal_hook::consts::{SIGCONT, SIGHUP, SIGQUIT, SIGSTOP, SIGTERM, SIGTSTP}` (`src/interactive/signal_handlers.rs`)
- `signal_hook::iterator::Signals::new` + `.forever()` (same file)
- `signal_hook::low_level::raise` (same file — `raise`, not `pipe`)
- `signal_hook::flag::register(SIGINT, Arc<AtomicBool>)` (`src/bench/bench_progress.rs`)

None of them is `low_level::pipe`. The compiler confirmed: pin bump
`0.3` → `0.4`, `cargo update -p signal-hook` (0.3.18 → 0.4.4),
`cargo check --locked --all-targets` clean and clippy
`--all-targets --all-features -D warnings` clean with ZERO source
changes. The 1-2 h estimate, like notify's 2-4 h, was priced against
breaking changes that do not exist on this project's surface.

### The graph consequence: a duplicate that is not ours to fix

crossterm 0.29 depends on the 0.3 line via signal-hook-mio, so
signal-hook 0.3.18 stays in the graph transitively after our direct
dep moves to 0.4.4. `deny.toml` gains a documented
`{ name = "signal-hook", version = "0.3.18" }` skip — the same
pattern as windows-sys — with the leave-condition recorded (the
duplicate leaves when crossterm migrates). Both versions route
through the same signal-hook-registry 1.4.x backend, so there is no
duplicated signal behavior at runtime.

### The functional proof: an 8-check PTY parity harness

A compiler pass proves the code compiles; it does not prove the
terminal still gets cleaned up on SIGTERM. New harness:
`scripts/harness/signal_smoke.py` (spawn recipe from
`scripts/harness/termux_hang_harness.py` — pty.fork so the slave IS
the controlling terminal; the same recipe catches spawn mistakes that
a Popen-based PTY hides).

| # | Check | Baseline 0.3.18 | Migrated 0.4.4 |
|---|-------|-----------------|-----------------|
| 1 | startup liveness (3 s warmup, kernel state S) | PASS | PASS |
| 2 | Ctrl+C keystroke ignored (0x03 byte into the PTY — the only-q policy, bug #15) | PASS | PASS |
| 3 | SIGTSTP suspends (kernel state T) | PASS | PASS |
| 4 | SIGCONT resumes, still rendering | PASS | PASS |
| 5 | SIGTERM graceful exit inside the 3 s grace window (measured: immediate) | PASS | PASS |
| 6 | terminal-restore escapes on the PTY stream (alt-screen, cursor, mouse — all three observed) | PASS | PASS |
| 7 | benchmark liveness at 2.5 s | PASS | PASS |
| 8 | benchmark SIGINT abort — exit 0, `was_interrupted: true` in the JSON (the 0.4 `flag::register` path) | PASS | PASS |

Why check 2 uses a keystroke and not a raw SIGINT: cosmostrix never
installs a SIGINT handler in interactive mode. The "Ctrl+C deprecated"
guarantee is implemented at the keystroke level — raw mode turns
Ctrl+C into a key event the input loop drains and ignores — while a
direct `kill -INT` hits the kernel's default disposition (terminate)
on every signal-hook version, the crate not being involved at all
for unregistered signals. The harness tests the policy path, not the
kernel default.

SIGHUP/SIGQUIT are covered by construction: they sit in the same
`Signals::new([SIGTERM, SIGHUP, SIGQUIT])` list and flow through the
same `forever()` loop as SIGTERM — one mechanism, one proof.

### Gates

2983/2983 tests passed (0 failed, 2 ignored), `cargo fmt --check`
clean, clippy `-D warnings` clean, gate-keepers 21/21 (including the
new harness passing ruff + permission + name-case gates). The
10 s A/B bench vs the 6e3d718 baseline binary (cinematic + monolith
controls, 2 runs each) is performance-neutral and visual-identical
within run noise — monolith flat to four decimals on gini, fps
two-sided around zero (means -0.67%, median -0.18%, peak +0.95%),
cinematic straddling baseline with the usual 3.5K same-binary peak
spread — recorded in
[../bench-labs/night_dinner8/AB_REPORT.md](../bench-labs/night_dinner8/AB_REPORT.md).

## Part 2 — rand 0.9.5 → 0.10.3: the dragon-heart item, done in two import lines

### The census first: deep but narrow

217 rand-API lines in `src/` across 49 engine files (the cosmic
dragon cloud IS the heart) plus 66 in `test/` — but the actual API
surface is seven items: `distr::{Distribution, Uniform}`, `rngs::StdRng`,
`SeedableRng`/`seed_from_u64`, `.sample` (both call directions), the
Result-returning `Uniform::new`/`new_inclusive`, `.random_range`, and
`rand::rng()`. The audit table's "~15 call sites" was a file-count
guess; the dominant pattern is distributions built once (stored as
engine fields: `rand_chance`, `rand_line`, `rand_cpidx`, `rand_len`)
and sampled repeatedly.

### The claim versus the compiler, round four

The audit table priced this at 4-6 hours against "rand 0.10 reworked
the Rng trait, distr module, SeedableRng API; StdRng API changed;
seed_from_u64 may change signature." The upstream 0.10.0 changelog
tells a different story, and every claim is checkable against the
0.10.3 source in the registry:

- rand_core 0.10 renamed `RngCore` → `Rng`, so rand's extension trait
  moved `Rng` → `RngExt` — the one real break on this surface.
- `distr::{Distribution, Uniform}` unchanged; `Uniform::new`/
  `new_inclusive` keep the identical Result-returning signatures;
  `impl Distribution<T> for &D` still exists (both sample call
  directions work).
- `rngs::StdRng`, `SeedableRng`, `seed_from_u64`, `rand::rng()`,
  `.random_range` all unchanged. MSRV 1.85 ≤ the pinned 1.98.1.
- StdRng's backend swapped rand_chacha → chacha20, "but the output
  remains the same" (their claim — proven below).

The migration diff, in full: `use rand::Rng;` → `use rand::RngExt;`
in ghost.rs, and `use rand::Rng;` → `use rand::{Rng, RngExt};` in
living_rain.rs (the low-level `Rng` stays in scope as the minimal
bound for the file's seven generic rng plumbs — `Distribution::sample`
needs exactly that in 0.10, while `RngExt` covers the `.random_range`
call sites). `cargo check --locked --all-targets` is clean beyond
those two lines; the 4-6 h estimate was priced against rework that
does not exist — the fourth consecutive corrected estimate in this
dependency campaign (notify, signal-hook, now rand's three claims).

### The dragon-heart proof: 16,000 draws, bit-for-bit

Tests passing cannot certify sequence identity — a subtly different
sequence still renders a valid-looking rain. So the proof is direct:
twin scratch crates (rand `=0.9.5` vs `=0.10.3`) ran the engine's
exact patterns — seeded `StdRng` via `seed_from_u64` plus `Uniform`
sampling across u8/u16/u32/usize/i64/f32/f64, `new` and
`new_inclusive`, both sample call directions, four different seeds,
2,000 draws per pattern (16,000 total), hashed into rolling
accumulators with the first values printed verbatim:

```text
P1 u32_incl seed42 acc=14750518565326727433 first=[3 12 5 13 20 15 23 9 23 0 ]
P2 f32_new  seed7  acc=10589635599201140335 first=[0.4166409 0.0303173 ...]
P3 f64_incl seed9  acc=11515707078024267162 first=[5.703490606 1.736559845 ...]
P4 usize_new seed100 acc=3839053145641555151
P5 i64_incl  seed555 acc=241753304108680433
P6 u8_incl   seed64  acc=16260058814659461507
P7 u16_new   seed2024 acc=6201053124762786260
P8 u32_incl  seed_deadbeef acc=5211116755195889039
```

Every accumulator and every printed value is identical between the
two versions (archived session-side as `rand_parity_0.9.5.txt` /
`rand_parity_0.10.3.txt`). The chacha20 backend swap preserves
StdRng's output exactly as upstream claimed, and the `seed_from_u64`
derivation is unchanged. Same seeds → same sequences → same picture:
the dragon heart cannot render anything different than it did on
0.9.5. ThreadRng (`rand::rng()`, two ambient-jitter sites — gust idle
durations and ghost events) is OS-seeded and non-deterministic on
every version, so sequence identity there is neither possible nor a
visual-identity concern.

### The graph consequence: a dev-only duplicate

proptest 1.11.0 — already the max stable line — still pins rand 0.9,
so rand 0.9.5, rand_core 0.9.5 and rand_chacha 0.9.0 stay in the DEV
graph (`cargo test` builds only; never compiled into a release
binary). deny.toml gains documented `rand`/`rand_core` 0.9.5 skips
with the leave-condition recorded (proptest's own migration to 0.10).
Release-graph additions: rand 0.10.3, rand_core 0.10.1, chacha20
0.10.2, cpufeatures 0.3.1, getrandom 0.4.3, r-efi 6.0.0 (the latter
lockfile-only on non-UEFI targets).

### Gates

2983/2983 tests passed (RNG-dependent engine invariant sweeps
included — consistent with identical sequences), `cargo fmt --check`
clean, clippy `-D warnings` clean, gate-keepers 21/21. The 10 s A/B
campaign (cinematic + monolith vs the signal-hook binary, release
profile) is recorded in
[../bench-labs/night_dinner8/AB_REPORT.md](../bench-labs/night_dinner8/AB_REPORT.md).

## Backlog state at task end

| Item | Status at task end |
|------|--------------------|
| notify 8 | DONE 2026-09-29 (NIGHT-dinner-5 follow-up) |
| signal-hook 0.4 | DONE 2026-09-30 (this task, part 1 — zero source changes) |
| rand 0.10 | DONE 2026-09-30 (this task, part 2 — two import lines, sequences proven identical) |
| sha2 0.11 | HOLD (owner decision; the audit's revisit cadence is quarterly) |
| generic-array 0.14.9 | upstream exact-pin (leaves only with the sha2 0.11 migration) |

`cargo update --verbose` now prints two `Unchanged` lines, both by
design: the actionable major-bump backlog the owner approved burning
down is empty — "nothing remainings" achieved.

## Cross-references

- `docs/DEPENDENCY_AUDIT.md` — per-dep tables (signal-hook and rand both DONE), state list, action plan
- `docs/research/NIGHT_DINNER_5_DEPENDENCY_STRICTNESS.md` — the relax policy, the dragon boundary, the prior two relaxations
- `scripts/harness/signal_smoke.py` — the standing signal regression harness
- `deny.toml` — the documented skips (signal-hook 0.3.18, rand/rand_core 0.9.5) and their leave-conditions
- `../bench-labs/night_dinner8/AB_REPORT.md` — the A/B records for both migrations
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
