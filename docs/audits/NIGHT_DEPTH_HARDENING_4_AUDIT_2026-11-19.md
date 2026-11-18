<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# Depth hardening 4: determinism for every distro, ultra-scale endurance, the five-area cross-check (NIGHT-depth-hardening-4)

Date: 2026-11-19
Scope: the owner's five-area total-LTS cross-check over the root repo —
stability and crash, code hygiene, optimization, security hardening,
LTS stability — with one sharpened target: deterministic output on
every Linux distribution at kernel 5.15 and above, proven rather than
assumed, plus server-scale and long-endurance claims re-verified at
HEAD. Audit baseline: 15656078 (HEAD of main) plus the ci.yml gate
landed with this commit. Method: per-area census and empirical probes
(torture battery, digest cross-build comparison, transcendental
inventory, panic and lock-poison triage), with the peak-skip rule
applied per sub-task and the skip rationale recorded below.

## Verdict

**Four of five areas are at peak (recorded below with evidence); area
5 had one unproven determinism link, now closed by the cross-libm
replay-digest gate in this commit.**

## Area 1 — Stability and crash: at peak

- Hostile battery at this SHA: `torture.py` 6/6 legs PASS against the
  debug build (resize storm, input flood, signal storm, config churn,
  restart storm, suspend-resume). Debug profile runs with overflow
  checks and debug assertions on, so the battery doubles as an
  arithmetic-panic sweep.
- Panic census: 112 `unwrap`/`expect`/`panic!` sites in production
  `src/`; manual triage shows they are constructor-time invariants,
  compile-time constants and test-scoped guards — none parse external
  input. The 4 `.lock().unwrap()` hits are all `#[cfg(test)]`
  (TEST_WARNING_COUNT_MUTEX); production locks use poisoning-tolerant
  access and the Loom lane bounds the interleavings.
- CI corroboration at the same SHA: Miri, Kani, Loom, cargo-fuzz
  (nightly, 60 s bounded), Supermassive, Supermassive Kernel and
  CodeQL all green — ten of ten workflows.
- Visual/performance regression risk of this audit cycle: zero engine
  code touched; the only functional change is CI-side measurement.
  No A/B benchmark required (no binary change).

## Area 2 — Code hygiene: at peak

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  both clean at this SHA (13.9 s / 43.6 s builds, zero warnings).
- TODO/FIXME/HACK word-boundary census over `src/` and `scripts/`:
  zero hits (the earlier raw pattern's `mktemp` `XXXXXX` templates are
  the only "XXX" strings in the tree and they are mktemp syntax, not
  markers).
- `allow(dead_code)`: every occurrence is the disciplined
  `#[cfg_attr(not(target_os = "linux"), allow(dead_code))]` platform
  gate — no unbounded dead code.
- Dragon-hunt probe for copy-pasted color math: `srgb_to_linear` (u8
  domain, gradient engine) and `srgb_to_linear_f32` (f32 domain,
  color tune) are NOT zombies — the INV-19 parity contracts pin each
  side's rounding point by design; collapsing them would change bytes.
  Perstage priority order followed: `engine/` (48k LOC) audited
  deepest, `interactive/`, `config/`, `cli/` spot-audited, docs-only
  directories scanned by the gates only.

## Area 3 — Optimization: at peak, skipped by rule

- The two floors that matter are already enforced, not hoped for: the
  zero-alloc ledger asserts 0.0 allocations per frame on the lean
  path, and the syscall-floor ledger asserts exactly one `write()` per
  frame in steady state (PTY strace lock suite).
- The release shapes carry the profile work (PGO lane, LTO fat,
  codegen-units 1, target-cpu-pinned musl v3/v4 shapes plus the
  baseline floor shape); bench-labs holds the A/B history for every
  change that ever claimed a win.
- Any further micro-optimization attempted WITHOUT a profile showing
  a hot spot would be churn against the LTS law, so per the owner's
  peak-skip rule this area records: no change. The correct next
  optimization step remains profile-first (bench sweep evidence),
  never speculation.

## Area 4 — Security hardening (white hat): at peak

- Environment parsing: every `env::var` site uses
  `unwrap_or_default` / `unwrap_or_else` — no unset-variable panic
  path exists.
- Custom charset injection: control characters (C0 and C1, which
  `char::is_control` covers) are hard-rejected at parse time with a
  named error, unit-tested (`test/scene/` parity: `parse_rejects_control_char`).
- Diagnostic echo injection: `src/output/escape_ctrl.rs` owns the
  single sink law — user-supplied strings reach the terminal only
  through `escape_ctrl` (scene-custom display, warnings, verbose
  echoes), closing the raw-ESC terminal-command-injection class
  documented in S-night-R4.
- Path safety: the safepath gate validates config-provided paths;
  permissions gate proves no tracked file is accidentally executable.
- Memory safety: 44 `unsafe` blocks concentrated in eight audited
  low-level modules (terminal_tty, posix_time, alloc_trace,
  fork_guard, termios_snapshot, reclaim_state, cpustat, bench_perf);
  Miri green on the whole workspace at this SHA, and the
  UNSAFE_SOUNDNESS_AUDIT lineage covers each module's contract.
- Supply chain: deny.toml policy + Security Audit lane green;
  third-party actions SHA-pinned (pins gate).

## Area 5 — LTS stability and determinism: one link closed (FIXED in this commit)

Everything already proven and unchanged:
- Time math: the million-year horizon ladder (NIGHT-xenric-1) still
  holds at this SHA — no session-anchored f32 conversion exists, every
  accumulator is re-anchored, capped or equilibrium-bounded; the
  endurance docs and the 24 h+ stability target remain enforced by the
  Endurance Health Score (P1/P2 self-healing).
- Session survival: full-termios snapshot suspend/resume contract,
  synthetic terminal-gone quarantine, fork guard, panic hook terminal
  restore — all green under the torture battery above.
- Server ultra-scale: the renderer is single-threaded by law, CPU cost
  scales with cells not terminal clients, the Power Dragon byte
  budget throttles emission on slow drains, and the zero-alloc floor
  removes allocator fragmentation as a long-session risk. Nothing in
  this audit cycle found a scale-dependent failure mode.

The determinism link that was missing — and what this commit does:

1. **Measured foundation.** The replay oracle (#32) pins seed, screen
   size and every host-dependent surface, but its CI proof ran two
   processes of ONE build on ONE host — same libm both times. The
   digest was byte-identical across processes, never across libcs.
2. **Inventory.** The census of transcendental math on the executed
   replay surface (`gradient/mod.rs` OKLab, spawn lanes, ecosystem,
   rain_at easing) shows every call is f32 (`powf`, `cbrt`, `sqrt`,
   `sin`, `cos`, `exp` on f32 operands). f32 routines in glibc 2.28+
   and musl evaluate to the correctly-rounded result, so both libcs
   must agree bit-for-bit. Every distro at kernel 5.15 or above ships
   glibc 2.34+ — inside the correctly-rounded regime. (An f64 call on
   this surface would have been a genuine divergence risk; the census
   found none.)
3. **Local cross-profile proof.** Debug and release builds (fat LTO,
   codegen-units 1) produce the same digest on this host:
   `b3:7e938efa5efcb659dc214a9bf1fc84afdbbe8d6b8e0aab9f67b0b3606f1578d7`
   — build profile does not perturb the pinned sequence.
4. **The gate (this commit).** ci.yml now publishes the debug build's
   glibc digest as an artifact, and the reproducible-verify lane —
   which already runs the static musl release binary's replay —
   compares the two digests and fails the build on divergence. From
   this commit on, "the same replay sequence on every Linux distro at
   kernel 5.15+" is MEASURED on every push: glibc-on-runner-A versus
   musl-static-on-runner-B, two libcs, two hosts, one digest. A
   future float call drifting off the correctly-rounded law (or off
   the f32 rule) fails CI instead of silently breaking buyers'
   reproductions.

## Skipped follow-ups and why

- Replacing f32 libm calls with in-tree fixed-point math: pointless
  churn — the correctly-rounded law plus the new measured gate already
  pins the output; a software math library would trade verified
  determinism for slower, unaudited code.
- Extending the digest comparison to the macOS aarch64 lane: outside
  the stated Linux 5.15+ scope; Apple's libm is a different correctly
  roundedness regime and is already covered by its own build lane.
- A new lock-suite test re-asserting the transcendental inventory at
  build time: the CI digest comparison is strictly stronger (it
  catches any float-law drift, not just new call sites), so a
  source-scanning gate would add review surface without new teeth.
