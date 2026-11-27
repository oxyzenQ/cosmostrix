# AXP1-ZnQ (Architect's eXecution Protocol 1) - the maintenance bot's front door
<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

## Identity

**AXP1-ZnQ** stands for **Architect's eXecution Protocol 1**. The
short name is the owner's own shorthand (the AXP root names the
protocol, the ZnQ suffix carries the owner's handle), and this
sentence exists so the expansion can never go stale again: every doc
that mentions the bot names what it stands for, on the owner's
standing law. The name was adopted by night-solid-defense-1 for the
Sunday supply-chain sentinel; night-learning-into-darkness-1 added
the protocol's second half - the address catalog - and made this
document the name's front door.

The bot's one law: **no CI lane marries a hardcoded server again.**
The owner named the bottleneck after watching lanes die on
timeout / unable-to-connect / not-found failures whose root cause was
never the code - it was an address carved into a workflow that a bad
hour could sink. The answer is a tested address pool the bot curates,
not a hardcoded default the tree prays for, maintained independently
of any single lane so the estate adapts on its own and the owner is
pulled in only when the bot says CALL THE OWNER.

## The catalog (repos-for-ci/*.txt)

`repos-for-ci/` at the repo root holds one `.txt` per address family
(seven ship today: `keyservers.txt`, `rustup-mirrors.txt`,
`ubuntu-old-releases.txt`, `google-android-ndk.txt`, `crates-io.txt`,
`github-api.txt`, `github-web.txt`). The law of every catalog file:

- one candidate address per line; the whole trimmed line IS the
  address (a trailing comment on the same line would corrupt it -
  comments occupy their own line, starting with `#`);
- policy lives in the file itself, as `# key: value` directive lines:
  `purpose` (who consumes this family), `probe-path` (the artifact the
  probe fetches - always what the lane actually consumes, never a
  generic landing page), and `max-age-days` (the freshness ceiling, or
  `off` for families whose artifact is legitimately frozen);
- the authored order is the owner's trust ranking - canonical first,
  verified mirrors after. It is never a latency leaderboard; see the
  write law below;
- bounded: 10 to 100 active addresses across the whole catalog (the
  owner's band). The bot never invents an address to fill the floor
  and never prunes an active one to shrink the ceiling - a shortfall
  or overflow is a loud `::warning` and a CALL THE OWNER note;
- every shipped line was probe-verified through its family's real
  artifact at authoring time (fourteen of fourteen answered: 14
  alive, 0 down, 0 stale). Candidates that failed the witness were
  excluded on purpose and documented in the file headers - the
  keyserver portals that answer 200 yet serve no key material, the
  university rustup mirrors that 404 on every documented script path,
  the six ubuntu-old candidates that do not carry the EOL archive.

## The commands

All three live in `scripts/ci/`, stdlib-only Python (urllib, socket,
concurrent.futures - no third-party dependency joins the maintenance
surface, the LTS law), and share one probe core
(`axp1znq_probe.py`) so a mirror the resolver picks and a mirror the
refresher keeps are measured the same way.

**The resolver - `axp1znq_resolve.py`** - is the failover half, the
consumer CI lanes call:

```bash
python3 scripts/ci/axp1znq_resolve.py \
  --file repos-for-ci/keyservers.txt --first
```

It walks the catalog top to bottom, probes each address, narrates
every verdict to stderr (line number, reason, milliseconds - the CI
log keeps the audit trail), prints the first ALIVE address to stdout
and exits 0. Line 1 down, the lane moves on; line 5 answering is the
one the lane uses - the owner's scenario, verbatim. `--alive-only`
prints every live address one per line (loop consumers); `--json`
prints the machine table; a catalog with no live address is exit 1 -
a lane must never silently fall back to a dead default. Freshness is
reported in the narration but never blocks the pick: curation is the
refresher's job.

**The refresher - `axp1znq_refresh.py`** - is the catalog half, the
Sunday pass the maintenance lane runs:

```bash
python3 scripts/ci/axp1znq_refresh.py \
  --root repos-for-ci \
  --note-file "${RUNNER_TEMP}/axp1-address-note.txt" \
  --manifest-file "${RUNNER_TEMP}/axp1-address-manifest.txt"
```

It probes every catalog concurrently under each file's own policy,
and writes the survivors back. The write law:

- survivors keep their authored order - the catalog's shape is the
  owner's trust ranking, never a weekly reshuffle a noisy runner
  would churn;
- a dead or stale address becomes a dated tombstone in place
  (`# down 2026-11-27 (unreachable ...) - https://...`), readable
  history the owner can revive by deleting one line;
- each file is bounded by the 100-line cap; over the cap the OLDEST
  tombstones are pruned first, an active address is never pruned;
- the never-empty guard: zero live addresses means the file is NOT
  rewritten (a flaky runner probe must never erase the owner's pool)
  - the lane warns and the note names CALL THE OWNER;
- freshness reads the artifact's **Last-Modified** header, and only
  where an artifact contract exists: a file with no probe-path
  directive (or a line that lands on a bare host root) turns the age
  policy off by itself, because a landing page's Last-Modified dates
  nothing the lane consumes. This is a deliberate fix over the
  sibling estate's first sketch, which read the response `Date`
  header - that header dates the response, not the content, so its
  age read can never fire;
- idempotent: unchanged catalog, unchanged file - no write, no note,
  no churn; `--dry-run` reports and writes nothing;
- the exact changed-path manifest (`--manifest-file`) is what the
  maintenance lane's commit phase stages - never a glob, the same
  exact-path law the lockfiles ride.

**The probe core - `axp1znq_probe.py`** - is the shared verdict
engine: a real GET with TLS verified against the runner's own CA
store (a cert failure is DOWN, never ignored), bounded timeouts, two
attempts, plus a socket-connect path for future host:port families.
Both commands ship an offline `--selftest` (parsing, directive
precedence, tombstone shape, prune order, band law - no network
needed).

## The wiring (the Sunday cadence and the first consumers)

The maintenance lane (`Cosmic Dragon Guard - Dependency Maintenance`,
cron `0 3 * * 0` - Sunday 03:00 UTC = 10:00 WIB, night-consistent-1)
runs the refresher after the dependency sweep's clippy and before the
commit phase - the owner's sequencing law: the address refresh rides
AFTER the dependency update finishes, and the one weekly commit
carries deps AND addresses, with the refresher's note in the commit
body. The two-phase privilege model is unchanged: validate stays
read-only (the note and manifest travel as an artifact), commit stays
contents:write and stages only the lockfile walk plus exactly the
refresh manifest.

The first catalog consumers, each a lane that swapped a hardcoded
server for a resolver call:

| Lane | Family | What the resolver replaced |
|------|--------|----------------------------|
| `maintenance.yml` GPG key fetch | `keyservers.txt` | the hardcoded `keyserver.ubuntu.com keys.openpgp.org` pair (the run #46 incident lane) |
| `supermassive-kernel.yml` floor kernel + busybox fetch | `ubuntu-old-releases.txt` | two hardcoded `old-releases.ubuntu.com` bases |
| `ci.yml` freebsd native build | `rustup-mirrors.txt` | the hardcoded `https://sh.rustup.rs` (picked on the host, passed to the VM through the workspace sync) |
| `release.yml` freebsd native package | `rustup-mirrors.txt` | the hardcoded `https://sh.rustup.rs` (the ci.yml pattern applied verbatim: picked on the host, staged in the gitignored `.auto-update/` dir, passed to the VM through the workspace sync) |
| `scripts/ci/resolve-latest-ndk.py` | `google-android-ndk.txt` | the hardcoded `dl.google.com` manifest URL |

The remaining lanes adopt incrementally - a lane adopts by swapping
its hardcoded address for a resolver call, no new machinery needed,
that is the point of the pool (the crates.io, AUR and GitHub-API
surfaces are cataloged already; their single-host families have no
real alternates today, so a swap would add narration without adding
failover - they wire when a real alternate verifies). The
kernel-mainline DNS-pin workaround in `supermassive-kernel.yml` is a
deliberate non-adoption for now: that block is a documented
three-IP pin with its own incident history, and a lane that complex
earns its change in its own pass.

## The honest edges

- Probe verdicts are network-relative. The authoring sandbox verified
  all fourteen shipped lines, but `api.github.com` rate-limits shared
  IPs (its probe artifact is the quota-free `rate_limit` status
  document precisely so the weekly probe never burns the estate's own
  limit), and a runner-region difference is exactly what the weekly
  curation exists to reconcile.
- The freshness law fires only where an artifact contract exists.
  Families with frozen truth (the intrepid EOL archive, immutable
  crate files, keys) declare `# max-age-days: off` - an age read
  there would fail truth on principle.
- The band is the owner's: 14 active addresses ship today, inside the
  10-to-100 floor and ceiling. Families smaller than the floor
  (rustup: 3 verified, ubuntu-old: 3 verified, keyservers: 2 real
  key-serving hosts of ten candidates probed) ship their real count -
  padding the pool with dead or fake entries would be a lie the
  catalog must not keep; the owner adds candidates as the real world
  offers them and the Sunday run tests every addition.
- `--first` worst case on a dead-first catalog is bounded by attempts
  x timeout per address (both are flags); on a runner the connect
  timeout is what bites.

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
