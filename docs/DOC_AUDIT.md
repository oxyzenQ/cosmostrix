<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Documentation Depth Audit

The master depth audit for the docs estate: one ruleset every
document in the tree is measured against, wherever it lives and
whatever audience it serves. "Master" is the point — a single law for
the whole estate, not a per-doc bar each writer reinvents. The audit
exists because a doc's failure mode is rarely missing words; it is
words in the wrong shape: a conclusion with no mechanism under it, or
a fact told twice so one copy can go stale while the other stays
true. Tell a fact once and the sync burden disappears with the
duplicate; tell it twice and you have signed up to keep two stories
matching forever.

## Scope and exemptions

The audit walks every tracked `*.md` in the tree — the repository
root, the `assets/` notes, and the `docs/` tree alike. The exemption
set is the one the estate already speaks, not a new one: the frozen
records (`docs/archive/**` including the `CHANGELOG-V*-ERA.md` era
files, the dated `docs/research/**` investigation logs, the dated
`docs/audits/**` reports, and the `CHANGELOG.md` released history
below `## Unreleased`) are never rewritten, so their own shape is
exempt — the same frozen-record law the disclaimer injector, the
hype-vocabulary gate and the docs-audit corpus rules honor. Source
code is the single source of truth, and every doc yields to it: when
a doc disagrees with `src/**`, the doc is wrong, and the audit's fix
edits the doc — never the source to match the doc.

## The rules

1. **Tell once — one home per fact, no duplicates.** Every doc-owned
   fact has exactly one home: the single document that owns telling
   it, findable through the registry ([README.md](README.md)'s
   ownership tables and the master index). Every other doc that needs
   the fact links to the home and does not restate it. The worst
   duplicates are the volatile facts — counts, versions, sizes,
   dates, paths that move — because a restated number drifts the
   moment its source moves, and nothing fails loudly when it does.
   When the audit finds a second telling, the fix is a link to the
   home, never a re-sync of both copies: a re-sync is just the next
   drift waiting for its day. The codebase already speaks this law —
   `SPINNER_FRAMES` has one definition and every spinner lane
   delegates to it — and this rule is the docs side of the same
   discipline.

2. **Depth is the bar.** A doc's sections carry the mechanism and
   the why, not only the what: what a thing is, how it works, why
   this shape and not the alternatives, and where the truth lives in
   source. A heading followed by a bare conclusion fails; a rule
   list with no reasoning behind it fails; a playbook that shows
   commands without how to read the output fails. Brevity is not the
   sin — a short section that carries its mechanism passes, and a
   long one that only restates conclusions does not. The bar is the
   density of explanation, not the word count.

3. **The registry is the estate.** [README.md](README.md) is the
   complete map of what exists and what each doc covers. A doc not
   in the registry either gets a row or does not belong in the tree;
   a row with no doc behind it is deleted. A fact that fits no
   owner's scope is either a new owner (a new row, decided like any
   other change) or it does not get told — free-floating facts are
   how orphans and duplicates are born.

4. **Same-pass sync.** A change that moves a fact lands in the
   fact's home doc in the same commit. A pass that changes behavior
   and leaves the home doc for a later docs pass has split the truth
   from its telling; later never comes, and the audit inherits a
   finding. The disclaimer injector's `--check` and the stale-hunt
   gate catch the mechanical residue of a split, but the discipline
   is cheaper than the detection: land the fact with its telling.

5. **Findings land as their own commits.** The audit's output is not
   a report nobody reads — it is fixes. Each finding lands as its
   own internal-research commit: the finding named in the message,
   the rule it violated cited in the body, the fix in the diff. A
   finding the owner defers is a decision, not a disappearance — it
   is named somewhere durable ([FUTURE_BACKLOG.md](FUTURE_BACKLOG.md)
   is the estate's ledger) with its reason, never silently dropped.

## How the audit runs

The audit is a discipline, not a gate script: the rules bind at
review time. The mechanical half of the estate already has its
automated seat — `scripts/gates/docs-audit.py` hunts broken refs,
stale paths, stale counts and duplicate citations across the living
corpus on every gate run — and this document is the human half it
cannot parse: whether a section carries its mechanism, whether a
fact's home is the right one, whether the registry and the tree
agree. A docs-touching pass runs the five rules against the touched
doc and its neighbors, every time. A docs series opens with a full
walk: the registry top to bottom, every doc against every rule,
findings landed per rule 5. Green means: every fact has one home,
every section carries its mechanism, the registry and the tree agree,
and no commit in the series moved a fact without its home doc.

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
