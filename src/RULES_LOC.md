<!-- SPDX-License-Identifier: GPL-3.0-only -->

# src/ LOC Limits — Hard 800 / Soft 500

> **Owner mandate 2026-08-28** (per deepseek discussion): tighten the
> gross-line cap from the previous 1,500 to a **hard limit of 800 lines
> per `.rs` file**, with a **soft target of 500 lines** for new files.
>
> This file is the canonical reference for the LOC policy. `docs/RULES.md`
> references it; `scripts/check-rs-loc.sh` enforces the hard limit.

## The Limits

| Limit | Value | Enforcement |
|-------|-------|-------------|
| **Hard limit** | 800 lines | `scripts/check-rs-loc.sh` fails the build. NO exceptions (except generated code — see below). |
| **Soft target** | 500 lines | Recommended for new files. Not enforced, but PR review should flag files drifting above this without justification. |

## Why 800 / 500

- **Agent efficiency**: AI coding agents (and human reviewers) parse
  smaller files faster. A 1500-line file exceeds the typical attention
  window; 800 keeps the whole file readable in one screen.
- **Single responsibility**: forcing splits surfaces cohesive concerns
  that were silently growing inside a "do everything" file. Each
  extraction makes the codebase more navigable.
- **Maintenance cost**: large files accumulate tech debt faster because
  the cost of understanding them before editing is prohibitive. Smaller
  files lower the barrier to safe edits.
- **Deepseek advice**: the owner discussed this with deepseek, which
  recommended 800 hard / 500 soft as the sweet spot — tight enough to
  force decomposition, loose enough to avoid gratuitous splits of
  genuinely cohesive modules.

## Scope

All `.rs` files under `src/`, plus `build.rs`.

**Excluded** from the cap:
- `target/` (build artifacts)
- `.git/`
- Generated code (e.g. `target/`-derived build scripts, vendored
  codegen). If a generated file would exceed 800, it self-declares
  via the `// LOC_EXEMPT:` marker with a comment naming the generator
  (the script has NO hardcoded exclusion list — the marker lives with
  the file; NIGHT-hunter-5 sync).

## How to Refactor a File Over 800

1. **Identify cohesive concerns**: scan the file for clusters of
   functions / impls that share a theme (e.g. "message overlay",
   "border gradient", "input handling"). Each cluster is a candidate
   extraction.
2. **Extract to a sibling file**: create `src/<module>/<concern>.rs`
   (NOT at `src/` root — see `src/RULES.md` single-file policy).
3. **Re-export for API stability**: in the parent `mod.rs`, add
   `pub(crate) use <concern>::{fn_a, fn_b};` so existing call sites
   continue to resolve without changes.
4. **Preserve visibility**: extracted functions keep their original
   `pub`/`pub(crate)`/`pub(super)` visibility. Do NOT widen or narrow
   visibility during extraction.
5. **Run the full gatekeeper**:

   ```bash
   cargo fmt --all --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   ./scripts/gate-keepers.sh
   ./scripts/check-rs-loc.sh   # must show 0 files > 800
   ```

## When NOT to Split

- **Cohesive files close to 800**: if a file is 700-800 lines and its
  contents are genuinely one concern (e.g. a single large `impl` block
  with tightly-coupled methods), leave it. Splitting for the sake of
  hitting 500 introduces artificial boundaries that hurt readability.
- **Test files**: test files live in the mirrored `test/` tree
  (NIGHT-hunter-1, owner mandate 2026-09-04), which is OUTSIDE the
  `src/` scan of `scripts/check-rs-loc.sh` — the cap governs production
  source only. Prefer splitting test files by category (e.g.
  `tests_border.rs`, `tests_phosphor.rs`) when a file grows past
  1000 to keep review manageable.
- **Generated code / genuinely unsplittable files**: vendored codegen
  output or files that cannot be split without decomposing the
  algorithm are exempt via a **self-declaring marker comment**:

  ```rust
  // LOC_EXEMPT: <one-line justification>
  ```

  Place this marker on line 3 (after the copyright + SPDX header).
  `scripts/check-rs-loc.sh` dynamically greps each over-800 file for
  this marker — **no hardcoded file list**. The exemption lives WITH
  the file, so it can never drift out of sync. Removing an exemption
  = delete the marker comment (no script edit needed).

## File Permission Rule (unchanged)

Standard Unix permissions per `src/RULES.md`:
- Directories: 755
- Rust source `.rs`: 644
- Shell scripts `.sh`: 755

Never `chmod 777` or `chmod 755 -R`. Use `git update-index --chmod=-x
<file>` to fix accidentally-executable tracked files.

## Enforcement

- **CI**: `scripts/check-rs-loc.sh` runs in `./scripts/build.sh check-all`
  and in the gatekeeper. Fails the build if any `.rs` file exceeds 800.
- **PR review**: reviewers should reject PRs that add files >800
  without a `// LOC_EXEMPT:` marker comment containing a justification.
- **This file**: canonical reference. Update here first, then propagate
  to `docs/RULES.md` + `scripts/check-rs-loc.sh`.

## Migration Path (2026-08-28 → ongoing)

The previous cap was 1500. Files that were legal under 1500 but exceed
800 are now over-limit. The migration is incremental — each over-limit
file gets its own refactor commit, preserving exact behavior. The A/B
benchmark (10s release) must show <2% avg_fps delta after each batch
of refactors (performance-neutral mandate).

See `CHANGELOG.md` "Refactor (LTS — 99% no visual/performance change)"
section for the commit log of extractions done under this policy.
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
