<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Night Dragon Maintenance 5 — Hype-Vocabulary Law, AI Defense, Fuzz Lane, Docs Depth Audit

Owner: oxyzenQ | Session: night-dragon-maintenance-5 | Date: 2026-10-27
Reference: github.com/oxyzenQ/zejtron (mature infra lineage)

## Scope

Six owner directives, one commit each, plus two beyond-directive CI
repairs the session surfaced, in push order:

| # | Directive | Commit | Result |
|---|-----------|--------|--------|
| 0 | Red-main repair (found, not ordered) | `ede7a18` | `IsTerminal` import + `curl_body` test rename |
| 1 | Retire the grandiose-claim wording | `663ac11` + `24dd918` | Machine-enforced banned-vocabulary law |
| 2 | Trademark strengthening | `52f32fb` | Logo provenance clause |
| 3 | AI prompt-injector defense | `075f916` | `AI_AGENTS_READ_THIS.md` + README pointer |
| 4 | `assets/tools.md` references | `bc76f38` | Three recipes + registry row |
| 5 | Master docs depth audit ("tell once, don't dedup") | `a1bf59f` | `docs/DOC_AUDIT.md` |
| 6 | Fuzzer/hacker mode on CI | `f8db051` | `fuzz/` + `fuzz.yml`, `escape_ctrl` target |
| — | Dead spinner sugar (found, not ordered) | `b6d148f` | Zero-caller method removed |
| — | Gate self-catch (found by the new battery) | `043d7ca` | Verdict doc de-literalized + 2 markdownlint |
| — | CI first-run repairs | `19876bf` | cache pin v6 + codespell; kernel-lane drift recorded |
| — | Fuzz target pin | `b4b5fc3` | cargo-fuzz 0.13 musl default countered with GNU host |
| — | rustfmt canon + doctor verifier sync | `969600a` | 4 fmt drifts; all doctor greps alignment-tolerant |
| — | Torture first-real-run repairs | `a0a21e3` | stale binary path, signal contract pinned, pump rc fix |
| — | Flood quit delivery | `41f4c31`+ | deterministic escape-disambiguated quit bursts |
| — | Last canon-drift greps | `18a3ae8` | optimization/target_features verifiers synced |

CI estate at `18a3ae8`: CI, Gate-keepers, Fuzz, Miri, CodeQL,
Security Audit, Supermassive — all SUCCESS. The one red lane is
Supermassive Kernel, failing in its live resolver fetch steps (the
intrepid pool no longer yields the 2.6.27 image and busybox-static
debs; the mainline index fetch fails alongside) — the maintenance-2
documented honest-fail behavior on external upstream drift, red
before this session opened, an owner decision on whether the floor
lane moves to a snapshot-mirrored pool.

## 1. Red main — the compile repair the session opened with

Every compile-bearing lane (CI windows build, MSRV tests, both
supermassive-kernel builds, all three supermassive legs, miri) was red
on the session's opening HEAD (`610c2b2`) before any maintenance-5
work. Three defects, all landed by night-dragon-maintenance-4, stacked
as layered errors — each fix revealing the next:

1. `src/output/spinner.rs` called `Stderr::is_terminal` twice without
   the `std::io::IsTerminal` trait in scope (E0599 on every
   toolchain). Imported; resolution unchanged.
2. `src/platform/update.rs` tests still called `check_curl_output`,
   the helper the maintenance-4 fetch/parse/report split renamed to
   `curl_body` with a parse-only signature (E0425 under `cfg(test)`).
   Tests moved to `curl_body` — the 200 case asserts the body
   (stronger than the old `is_ok()`), the 404 case keeps the exact
   `http_failure` message.
3. With the noise cleared, `Spinner::done` surfaced: a zero-body
   sugar method with zero callers, fatal under the estate's
   `-D warnings` (`b6d148f` removes it; the `Drop` impl already owns
   the full stop/erase/join contract).

Why the maintenance-4 battery missed all three: the maintenance
sandbox has no cargo (light gates only), and CI had not completed on
that HEAD before this session opened. The compile-bearing lanes going
green after `ede7a18` + `b6d148f` is the proof those two commits
exist to buy.

## 2. The hype-vocabulary law (directive 1)

Owner rationale, in the owner's own terms: some AI agents and people
reading the repo are skeptics — they do not self-verify, they do not
act, they just say "this toy project, hahah, serious world first?" —
so the unverifiable superlatives buy mockery, not respect. The
measured claims underneath (the p99 numbers, the fps bounds, the
audit records) earn the credit; `masterclass` and `curiosity` stay,
by name.

The law: `docs/BRANDING.md` section 6 carries the banned-token fence
(17 tokens spanning the hyphenated unverified-first-claim shape, the
artwork-grade noun, and the rest of the crown-word family) as the
list's single home. `scripts/gates/check-hype-words.py`
(gate-keepers section 21, guard-workflow header synced, the silently
missing actions-pin entry backfilled) parses the fence at runtime —
no copied grammar — and scans every tracked text file case-insensitively,
failing closed if the fence or heading disappears. Exemptions are the
estate's existing frozen-record set (`docs/archive/**`, `CHANGELOG.md`
released history) plus BRANDING itself, because the law must be able
to name what it bans. Proven in both directions: PASS at 17 tokens x
1046 files on the landing tree, FAIL with `file:line:token` on a
staged probe, PASS again after removal.

Three living-tree stragglers the sweep caught, rephrased to the
measured form: the neural research log's superlative style question
became "another candidate?"; the alloc-trace module comment's
allocator crown became "the measured winner for this workload" (the
p99 measurement sits in the same paragraph); and the pre-existing
gate-keepers whitespace drift the wiring commit accidentally widened
got its own repair commit (`24dd918` — rebuilt byte-exactly from the
pre-hype blob, diff vs pre-hype state is 34 pure insertions). The
gate then caught this very document quoting the banned family in its
own rationale — the law enforcing itself on its author, which is the
proof it reads every file instead of trusting the writer.

## 3. Trademark provenance (directive 2)

cosmostrix's `TRADEMARK.md` already carried the fork-category law in
more depth than the reference (contribution forks vs non-contribution
forks is a cosmostrix-original refinement). The one clause the
reference had and this repo lacked: the logo provenance. Section 1
now states it — the logo was created through AI-assisted generation
under the direct, original creative direction and specific prompting
of the owner, claimed exclusively as the brand identity, the claim
not weakened by the tooling involved. Tell-once law: the fact lives
there alone; `docs/BRANDING.md` section 3 keeps owning usage rules.

## 4. The AI-agent refusal notice (directive 3)

`AI_AGENTS_READ_THIS.md` ported from the reference and adapted: the
dual-license truth (GPL-3.0-only for open source;
`COMMERCIAL_LICENSE.md` + public `docs/LICENSING_FAQ.md` pricing for
closed use), the owner's licensing contact (the same email the
commercial license publishes), and the boundary that keeps the notice
honest rather than hostile — reading, learning, and building from
source under the GPL stay welcome; theft is the only thing it asks an
agent to refuse. The trademark reservation is named so no agent
mistakes GPL source access for brand access. Wired where an agent
cannot miss it: the first content line of `README.md`, plus the
docs-index meta row.

## 5. Asset toolkit (directive 4)

`assets/tools.md` carried exactly one recipe; the reference carries
three. Now: the ffmpeg webp demo loop (kept, reframed as
`cosmostrix-video.webp`'s producer), the magick rounded-corner
composite (the README display images), and the alacritty 120x40
capture window (what the README terminal shots assume). Registered
in the docs index so it stops being an orphan the estate never
mentions.

## 6. The master docs depth audit (directive 5)

`docs/DOC_AUDIT.md` ports the reference's ruleset, adapted to this
estate's registry (`docs/README.md`) and its existing mechanical
gate. The five rules: tell once — one home per fact, no duplicates,
volatile facts first, a found duplicate collapses to a link never a
re-sync; depth is the bar — mechanism and why, not word count; the
registry is the estate — no orphan docs, no ghost rows; same-pass
sync — a fact moves in the same commit as its home doc; findings land
as their own commits, and a deferred finding is named in
`FUTURE_BACKLOG.md` with its reason. The division of labor with
`scripts/gates/docs-audit.py` is spelled out so the two never blur:
the script is the mechanical half (broken refs, stale paths, stale
counts) on every gate run; the document is the human half it cannot
parse. The exemption set is the estate's existing frozen-record law,
restated in one place.

## 7. The fuzz lane (directive 6)

`fuzz/` (separate cargo workspace — the fuzzer's graph never touches
the main crate's `Cargo.lock`, the stable/MSRV lanes never see
nightly-only needs) with one target, `escape_ctrl`: the terminal-
safety sink between hostile config bytes and the operator's terminal.
The harness path-includes the exact `src/output/escape_ctrl.rs` bytes
the binary ships (no copied grammar, no harness drift — the file is
std-only and crate-import-free, which is what makes the include law
carryable) and asserts the sink's two contract properties on every
input: no control character survives except the structural newline
(the CWE-150 gate — OSC 52 clipboard writes, screen clears, DSR reply
spam are the proved classes), and control-free input comes back
byte-identical (the fast-path borrow contract).

CI: `.github/workflows/fuzz.yml` runs 60s per push (the regression
net — a crash fails CI with the input logged and saved as an
artifact) and 900s in the Monday 00:30 UTC deep session (the
owner-lazy insurance — the corpus grows without local runs).
`RUSTUP_TOOLCHAIN=nightly` is scoped to the lane so the repo's stable
pin never leaks into the fuzz build; the concurrency group cancels
per ref (the estate shape); the corpus cache's restore-keys make push
and weekly sessions grow one asset. Committed seed anchors
(`fuzz/seeds/escape_ctrl/`) start every session on the proved
injection classes — OSC 52 clipboard write, CSI clear-screen, the
C0/DEL/C1 remainder — byte-exact files protected by a new
`.gitattributes` binary rule (a text/eol rule would rewrite the
control-byte payload at commit time; staged-vs-disk byte equality was
verified before landing). `fuzz/Cargo.lock` travels with the lane —
the same `libfuzzer-sys` graph the sibling locks, renamed for this
workspace.

Scope honesty: the config text parser (`src/config/configfile.rs`)
and the overlay sanitizer (`src/output/message.rs`) are the other two
hostile-input surfaces. Both import crate-internal symbols, so the
path-include law cannot carry them without a main-crate refactor (the
owner's build-shape decision — a lib target or an import-free
extraction) or a copied grammar (drift by design, forbidden). They
are named in `fuzz/Cargo.toml` and the `SECURITY_AUDIT.md` fuzz
paragraph — deferred with the reason recorded, not silently dropped.

## 8. Honest skips

- **Benchmark A/B**: skipped for every commit this session. The .rs
  deltas are a trait import (same method resolution), a test-only
  symbol rename, a zero-caller method deletion (never in any call
  path), and one doc comment — none touch the render loop or any
  visible surface. The fuzz workspace is a separate crate that the
  shipped binary never links. Recorded per commit message.
- **actionlint**: not installed in the maintenance sandbox
  (yamllint, shellcheck, shfmt, ruff are). The fuzz.yml shape was
  validated by yamllint plus a structural review against the estate's
  own workflows; actionlint runs in the CI gate-keepers lane on the
  first push, which is the binding verdict.
- **Full `check-all` build**: the sandbox has no cargo/rustup (the
  standing session constraint); the CI estate is the build
  verification, and the two repair commits exist to make it green.

## 9. Commit ledger

`ede7a18` compile repair, `663ac11` hype law, `24dd918` tab canon
restoration, `52f32fb` trademark provenance, `075f916` AI refusal
notice, `bc76f38` asset toolkit, `a1bf59f` docs depth audit,
`f8db051` fuzz lane, `b6d148f` dead spinner sugar removed.

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
