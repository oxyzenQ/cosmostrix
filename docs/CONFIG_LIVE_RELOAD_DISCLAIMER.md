<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Config / Live-Reload Disclaimer — 99% Not 100% Perfect

> **Status**: v80.0.0-beta.2 honesty note (updated from the beta.1
> original). Owner-mandated. This document is the canonical statement
> of cosmostrix's posture toward config and live-reload correctness. It
> does NOT expire when the project reaches stable — it is the stable
> posture.

## TL;DR

cosmostrix's config parser + live-reload watcher is **99% reliable,
not 100% perfect**. It will ship with a small known tail of edge
cases and limitations. We document them honestly instead of chasing
the asymptote of perfect behavior. Perfect means stuck — no way to
evolve. The 1% tail is the project's evolution surface.

## Why we ship 99%, not 100%

Three reasons, in priority order:

1. **Honesty over appearance.** Claiming "100% perfect live-reload"
   would be a lie. Every file-watcher system has race conditions
   with non-atomic editor writes, every TOML parser has scope rules
   users fall foul of, every layering contract (CLI / config / scene /
   ambient / custom block) has corners where intent is ambiguous.
   Pretending otherwise sets the user up for disappointment when they
   hit the corner. Naming the corner up front builds trust.

2. **Limited dev time, unlimited edge cases.** The config surface is
   ~50 keys, 3 custom-block namespaces (`charset-custom`,
   `colors-custom`, `scene-custom`), one phase-scheduler namespace
   (`ambient`), and one tuning sub-table (`color.tune`). The
   combinatorial space of "user writes config edit X while state Y is
   active" is finite but large. Spending 6 months chasing every
   combination is not a good investment when the user-visible payoff
   per fix is small. We fix the high-impact bugs (see
   `docs/LIVE_RELOAD_BEHAVIOR.md` §9 Z-master-1B audit for 5 fixed
   gaps) and document the rest.

3. **Never perfect = still evolving. Perfect = stuck.** A config
   system that claims 100% perfection has frozen its contract — any
   future improvement is a breaking change. A config system that
   admits "we have a small tail of edge cases" keeps the door open
   for v90, v100, v110 to fix them as the project evolves. The
   project's dormancy model (5-10 year maintenance cycle, see
   `docs/MAINTENANCE.md`) means a frozen "100% perfect" claim would
   block fixes that may be obvious in 2031 but were not visible in
   2026.

## What "99% reliable" means in practice

- **The 99%**: every config key in `USER_CONFIG_KEYS` parses, validates,
  and applies on save with correct CLI / config / scene / ambient /
  custom-block precedence. The full per-key matrix is in
  `docs/LIVE_RELOAD_BEHAVIOR.md` §1 and §7. The owner's audit
  (`docs/LIVE_RELOAD_BEHAVIOR.md` §9) verified each of the ~50 keys
  against `rebuild_cloud_config` + the downstream `create_cloud`
  application, with regression tests pinning the behavior.
- **The 1%**: documented limitations — see the next section. These
  are NOT bugs to fix; they are corners of the contract that exist
  for a reason (correctness of a higher-priority invariant) and are
  documented so users understand the layering.

## Known limitations (the 1% tail)

These are documented in detail in `docs/LIVE_RELOAD_BEHAVIOR.md`
§8 "Known Limitations — 99% Not 100% Perfect":

- **Limitation A** — `--verbose | grep` pipe behavior (cosmostrix
  stays in interactive mode; redirect stderr to a file instead).
- **Limitation B** — Multi-terminal config overwrite (each
  `cosmostrix` process is independent; restart all instances after
  `--dump-config --force`).
- **Limitation C** — `color.tune` reset-on-comment — **FIXED** in
  v50.0.0-alpha.7 (kept here as historical context).
- **Limitation D** — Ambient-owned config keys (`scene`, `color`,
  `charset`, `fps`, `speed`, `density`, `glitch-level`) are no-ops
  while the ambient overlay is active (v80.0.0-beta.2: `fps` joined
  the list — the ambient scene owns the frame target; a config-set
  custom palette also loses to the ambient scene's color). Shortkeys
  still work (they outrank ambient until snapback). Workaround:
  comment out ALL `ambient.<HH-MM>` entries to lift the overlay. See
  `docs/LIVE_RELOAD_BEHAVIOR.md` §8 (Limitation D) + §15 (the
  S-master-LOGIC-3 runtime precedence contract) + §14 (the overlay
  lift).
- **Live-reload is not atomic-write safe** with non-atomic editors
  (`echo > config.toml`, `tee`). Use atomic-saving editors (VSCode,
  vim with `writebackup`, Helix, Neovim). On validation failure, the
  previous config is retained; no crash.
- **Live reload watches a single file** (`config.toml`). External
  files referenced by config (custom palette files, etc.) are not
  individually watched — reload triggers on `config.toml` save only.
- **Ambient scheduler uses wall-clock time**. DST spring-forward
  skips entries in the 02:00–02:59 window; DST fall-back fires
  entries in the repeated hour twice. Acceptable per design — the
  scheduler is a convenience, not a cron replacement.
- **Single ambient entry is active all day.** A schedule with only
  one entry wraps via midnight carry-over. Use two entries if you
  want a scene to activate only after a specific time.
- **Restart-only keys**: `intro` and `intro-color` (the intro is a
  one-shot animation, not live-reloadable). Documented, not a gap.

## What this disclaimer is NOT

This disclaimer is NOT:

- **A waiver of bug fixes.** Real bugs (config edit silently no-ops,
  parser misclassifies a known key, validation rejects a valid value)
  are bugs and will be fixed. The owner's `v80.0.0-beta.1` Z-master-1B
  audit fixed 5 such gaps; the next audit will fix more. Open an
  issue if you find one — the source is the truth, the doc is the
  suspect.
- **A license to break the contract.** Existing config keys keep
  their documented behavior across versions. Breaking changes
  require a major version bump and a CHANGELOG entry. The
  disclaimer is about the **tail of edge cases** in the contract's
  corners, not about the contract itself.
- **A replacement for `--testconf`.** Run `cosmostrix --testconf`
  after editing config.toml. It catches typos, structural mistakes
  (key nested under wrong section), value range violations, and
  unknown keys — with targeted hints for each. Most "live-reload
  is broken" reports are actually "I never ran --testconf and the
  error was at startup, not at reload".

## The philosophy in one paragraph

cosmostrix is honest about its limits because the alternative —
pretending to be 100% perfect — is a brittle lie that breaks the
first time a user hits a corner. We document the corners, we name
the trade-offs, we ship the 99%, and we keep the 1% as the surface
where the project will evolve over the next decade. Perfect means
stuck. The 1% tail is the door we leave open.

## Source references

- `docs/LIVE_RELOAD_BEHAVIOR.md` — the full live-reload research
  document (per-key matrix, audit history, limitations, ambient
  overlay contract).
- `docs/PHILOSOPHY.md` §6 "Honesty as a Release Gate" — the
  project-wide honesty posture that this document specializes to
  config/live-reload.
- `src/config/live_config/mod.rs` — the `rebuild_cloud_config`
  implementation (the 99%).
- `src/config/configfile/configfile_promote.rs` — the auto-promote
  decision rule (a v80.0.0-beta.1 fix for one of the 1% corners).
- `src/config/configfile/configfile_dump.rs` — the template config
  (with v80.0.0-beta.1 honesty notes inline at the ambient section).

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
