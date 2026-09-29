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
10 s A/B bench (cinematic + monolith vs the 6e3d718 baseline binary)
follows the post-commit protocol and is recorded in
[../bench-labs/night_dinner8/AB_REPORT.md](../bench-labs/night_dinner8/AB_REPORT.md).

## Part 2 — rand 0.9.5 → 0.10.x: the dragon-heart item

IN FLIGHT (this task, next commit). The plan, from the audit table
plus a fresh API census:

- Usage census: 217 rand-API lines in `src/` (49 engine files — the
  cosmic dragon cloud IS the heart) + 66 in `test/`. Dominant
  patterns: `rng.sample(Uniform)` (206 calls), `StdRng` (120),
  `seed_from_u64` (21), `.pick(` (13), `rand::rng()` (8),
  `.random_range(` (3).
- Two-layer proof of visual identity, because tests alone cannot
  catch a subtle sequence change: (a) a determinism check — two
  scratch crates (rand 0.9 vs rand 0.10) run the exact engine
  patterns (seeded StdRng + Uniform int/float sampling + pick) and
  the sequences are compared bit-for-bit; (b) the 10 s A/B benchmark
  campaign (baseline vs migrated, release profile, cinematic +
  monolith + an RNG-heavy scene, 2 runs each).

## Cross-references

- `docs/DEPENDENCY_AUDIT.md` — per-dep tables (signal-hook DONE, rand in flight), state list, action plan
- `docs/research/NIGHT_DINNER_5_DEPENDENCY_STRICTNESS.md` — the relax policy, the dragon boundary, the prior two relaxations
- `scripts/harness/signal_smoke.py` — the standing signal regression harness
- `deny.toml` — the signal-hook 0.3.18 skip and its leave-condition
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
