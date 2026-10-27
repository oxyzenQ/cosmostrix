<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-dragon-maintenance-2 — unfiltered CI, the KVM kernel lanes on the real floor and the newest mainline, and the bugs the port caught on the way

Owner directive (2026-10-27): make the CI's ultimate real function an
end-to-end realtime depth stresstest - build from source only through
the pro aliases (never a bare `cargo release`), boot a KVM micro-VM,
and run the depth battery inside it (low specs on the project's
documented MINIMUM kernel, best specs on the LATEST kernel, root and
user directions as the project's contracts demand); simplify the
trigger model so every push burns every lane with no filtering left;
and depth-audit the minimum-kernel test coverage the docs promise
against what CI actually executes. The sibling estate zejtron
(supermassive-kernel-floor.yml, supermassive-e2e.yml,
scripts/sandbox/sandbox-init.sh) is the reference shape; its
live-run-hardened patterns are ported, not guessed.

Three commits carry the work: `7d2bdb1` (unfiltered triggers),
`54c76a6` (kernel-floor lane), `c5b6fca` (kernel-best lane).

## Part 1 - unfiltered CI (done)

Every push-triggered workflow now runs on EVERY push - any branch,
any tag, any file. The `branches:` and `paths:` filters are removed
from `ci.yml`, `codeql.yml`, `cosmic-dragon-guard.yml`,
`gitbot-audit.yml`, `miri.yml`, and `supermassive.yml`. The rationale
is cost and honesty at once: a filter list was a spelling six
workflows had to keep in sync (the estate-wide sync tax), and a
forgotten entry silently skipped the jobs compiling the changed
surface - the 2026-09-13 `test/**` incident (commit 5553174) is that
exact failure class.

The publish channels stay the deliberate exception, per the owner's
own carve-out: `crates-io.yml` and `release.yml` keep their `v*` tag
gates, `aur.yml` keeps its `repository_dispatch` gate - a registry or
release publish can never ride a blind trigger. Schedules survive as
back-stops (`maintenance.yml` weekly, `gitbot-audit.yml` daily,
`miri.yml` Sunday, `codeql.yml` Monday). The `check-ci-path-filters.py`
gate (gate-keepers check 16) stays in the battery as the guard for
any future filter reintroduction; with zero `paths:` blocks left it
passes vacuously, which is the correct reading of the new posture.

Documentation rewritten so no stale reference survives: ABOUT_CI.md's
workflow table and trigger section (now "Unfiltered triggers"),
release.yml's ci_gate comment and the ci_gate grace-period paragraph
in ABOUT_CI.md (the "docs-only commits the paths filter skips"
rationale no longer exists - there is no filter left to skip them).

## Part 2 - the depth audit: what the docs promise vs what CI executed

docs/SYSTEM_REQUIREMENTS.md fixes the floor precisely:
"Practical minimum: Linux 2.6.27 (December 2008)" - `inotify_init1`
(2.6.27) is the newest hard syscall dependency; everything else is
older or has a documented fallback (`getrandom` 3.17 falls back to
/dev/urandom; io_uring is explicitly NOT used). The audit's verdict
before this task: no lane in the estate ever executed the binary on
anything older than the runner's own 6.x kernel - the floor was
proven only as an at/above agreement check, exactly the "blind" gap
the owner asked the audit to find. The best-specs direction had no
dedicated kernel dimension at all (the runner's incidental kernel
was the only witness).

The supermassive.yml header used to state "No VM leg, on purpose" -
a stance recorded when the VM lanes were judged to burn minutes on
failure classes a tty renderer cannot have. The owner's directive
overrides that stance with a sharper reason: the KERNEL is a real
variable for a process that touches termios, signals, ptys, epoll,
timerfd, signalfd and inotify - libc syscall behavior differs across
kernel generations, and a portability promise ("runs on any Linux
kernel 2.6.27+") is only a promise until it executes at the boundary.
The stance comment is retired with the code that carried it.

## Part 3 - the kernel-floor lane (done)

`supermassive-kernel.yml` job `kernel_floor`: boots the REAL
intrepid (8.10) GA release-day kernel - `linux-image-2.6.27-*-generic`,
resolved from the LIVE old-releases pool (lowest 2.6.27-N build = the
release-day ABI), with loud failure and discovery evidence if the
archive ever drops it - in a KVM micro-VM on the runner. Design
decisions, each with its reason:

| Decision | Reason |
|----------|--------|
| Userland = intrepid `busybox-static` (same pool, era-contemporary) | The floor is a KERNEL claim; dragging a 2026 glibc into a 2008 guest would confound the variable. busybox 1.10-era static binaries carry no kernel floor above their own era. |
| Payload = `pro-linux-amd64-v3-musl` alias build | The owner's from-source law (never bare `cargo release`); musl-static is the docs' "most portable build" shape and needs no dynamic loader in an initramfs guest. |
| Step-level `RUSTFLAGS: "-D warnings -C target-cpu=x86-64-v3"` | The RUSTFLAGS env shadow bug (Part 5). |
| `-machine pc` + `-cpu host` | The 2008 guest predates q35; host-passthrough is load-bearing because the v3 binary needs AVX2 (TCG would SIGILL). |
| Dynamic low envelope | Quarter of the runner's cores (floor 1), eighth of its RAM (floor 1024 MB) - the low-specs shape. |
| Battery = full headless E2E surface, root | `scripts/sandbox/floor-payload.sh`: the info surfaces, `--doctor`, the `--bench-frames` headless render contract (twice, `--no-effects` variant), the fail-loud error paths, the headless guard itself, the panic sweep. The pty/signal depth deliberately lives on the best lane - the busybox console has no pty substrate, and pretending otherwise would be a dishonest assertion. |
| Exit contract | zejtron's proven `isa-debug-exit` + sysrq poweroff + console drain; the `SANDBOX-*` sentinel lines ARE the relay (a VM cannot pass exit codes through qemu). The init gates the 2.6.27 identity strictly - a wrong kernel booted under the floor lane's name fails before any payload. |

## Part 4 - the kernel-best lane (done)

Job `kernel_best`: resolves the NEWEST stable tag from the LIVE
Ubuntu mainline index (release tags only - the resolver's regex
cannot match an -rc entry), boots it in a q35 KVM micro-VM at the
best envelope (all runner cores, three quarters of the RAM), and
runs three directions inside an ubuntu:24.04 userland with python3:

1. the full headless surface - the floor lane's own instrument
   re-run, so the two lanes' rows stay comparable (only the kernel
   differs);
2. the pty quit-path depth as root - the EXISTING supermassive
   harness (`pty_smoke.py` + `pty_assert.py`) executes INSIDE the VM:
   the `--duration` deadline exit, the `q` quit key as live tty
   input bytes, the SIGTERM graceful shutdown, each asserting the
   restore tail;
3. the unprivileged direction - the duration quit-path leg re-run
   as `nobody` (uid 65534, `setpriv`): the docs' no-root contract
   executed on a real kernel.

Resolving instead of pinning is the point: a witness that can
resolve "the newest" can never boot a stale "latest".

## Part 5 - the hunt beyond the directive: bugs the port caught

1. **The RUSTFLAGS env shadow (would have been a silent v3 lie).**
   Cargo's precedence: `RUSTFLAGS` env shadows `build.rustflags`
   config. The cosmostrix pro aliases carry their `-C target-cpu`
   via `build.rustflags` (`.cargo/config.toml`), and the workflows
   set `RUSTFLAGS: "-D warnings"` at workflow level - so ANY CI
   invocation of a pro alias would have silently built a BASELINE
   binary under a v3 name. The release lanes never hit this because
   they spell their own rustflags per matrix cell (the ci.yml v80
   contract: "jobs that need target-cpu overrides append their own
   RUSTFLAGS env ... and MUST re-add -D warnings"). The new lanes'
   build steps carry the alias's exact -C token at step level, and
   supermassive.yml's pty/soak legs moved to the
   `pro-linux-amd64-v3-gnu` alias under the same law - the from-source
   rule now holds across the whole stresstest estate.
2. **The dash-vs-bash payload dispatch (would have been a red VM on
   a green contract).** `/bin/sh` on the ubuntu:24.04 userland is
   dash; the best battery uses bash arrays and a setpriv leg. The
   shared init now dispatches on the interpreter actually present
   (bash when it exists, POSIX sh for the busybox skeleton).
3. **The harness flag split (would have been an argparse refusal).**
   `pty_smoke.py` and `pty_assert.py` take disjoint flags; the best
   battery's leg helper splits the two groups on a `--` separator
   instead of forwarding everything to both.
4. **The ptmx node.** The best lane's initramfs lands `/dev/ptmx`
   (c 5 2) explicitly; devtmpfs populates it on the modern guest,
   but the static node is the fallback if the devtmpfs mount ever
   fails - the pty depth dies without it.

## Part 6 - verification and residual risk, stated honestly

Local gates: yamllint clean on every touched workflow; shfmt
canonical; shellcheck clean on all three sandbox scripts (the floor
battery checked as POSIX sh - no bashisms under busybox ash); bash -n
clean; check-headers, emoji-audit, language-audit, comment-style,
name-case, scripts-loc and permissions gates all green. actionlint is
not installable in the offline sandbox - the CI gate-keepers lane
owns that check and will judge the workflow YAML authoritatively.

Residual risks, deliberately taken: (a) the old-releases pool and
the mainline index are live resolvers - both fail loudly with
evidence rather than silently lying, which is the correct posture for
a witness; (b) the 2.6.27 guest's first boot under qemu 8 / KVM on a
2026 host is the one thing no local sandbox can prove here - if the
era boundary ever misbehaves, the lane's log tail names it loudly
(panic=-1 + -no-reboot + outer timeout guarantee the job cannot
hang); (c) the unfiltered posture means tag pushes now also fire the
branch lanes (a bare `push:` includes tags) - a duplicate run on the
same SHA, accepted for the same reason the posture exists: nothing
can be silently skipped.
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
