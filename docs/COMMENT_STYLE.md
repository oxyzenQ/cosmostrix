<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Comment Style Guide — Rust Source Comments

> **Audit 2026-09-04 (policy change, owner mandate)**: the owner
> re-reported markdown-looking emphasis (`**test**`-style bold and
> `*test*`-style italic) in comments across `src/*`, calling the raw
> source "inconsistent — descriptions read like md/mdx documents pasted
> into comments". The 2026-08-19 resolution below ("it's valid rustdoc,
> keep it") is SUPERSEDED: a sweep removed every decorative emphasis
> marker (378 markers across 130 file-passes: bold, italic, and
> multi-line bold spans) from all comment types. Comments are now plain
> prose. This document codifies the new contract, and
> `scripts/check-comment-style.py` (wired into `gate-keepers.sh`)
> enforces it so the drift cannot return.

> **Audit 2026-08-19 (superseded)**: owner reported `*abc*` and
> ` ```txt ` patterns in source comments; the audit then concluded they
> were valid rustdoc and kept them. The owner's 2026-09-04 re-report
> rejected that resolution: raw-source readability beats rendered-doc
> emphasis. Kept from that audit: the comment-type taxonomy (section 1)
> and the code-fence language table (section 2.1).

## 1. Comment Types

| Prefix | Type | Purpose | Renders in `cargo doc`? |
|--------|------|---------|------------------------|
| `//!` | Module doc | Top-of-file module documentation | OK Yes |
| `///` | Item doc | Documentation for a fn / struct / enum / const | OK Yes |
| `//` | Line comment | Implementation note, NOT user-facing | X No |
| `/* */` | Block comment | Rare; multi-line implementation note | X No |

**Rule**: anything user-facing (API contract, behavior, rationale that a
user/contributor needs to understand the public surface) goes in `///`
or `//!`. Implementation details (why this loop is unrolled, why this
magic number is 256 and not 255) go in `//`.

## 2. Markdown in Doc Comments (`///` and `//!`)

Rustdoc renders CommonMark, but this codebase deliberately restricts
itself to the FUNCTIONAL subset. Decorative emphasis is banned — the
raw source must read as clean prose.

| Syntax | Meaning | Allowed? | When to use |
|--------|---------|----------|-------------|
| `*italic*` | Emphasis | NO — banned 2026-09-04 | Use plain words instead |
| `**bold**` | Strong emphasis | NO — banned 2026-09-04 | Use plain words (or CAPS for a warning label) |
| `` `code` `` | Inline code | OK Yes | For type names, function names, identifiers, short expressions |
| ` ```text ` | Plain-text block | OK Yes | ASCII art, benchmark output (content is NEVER swept) |
| ` ```toml ` | TOML block | OK Yes | Config examples |
| ` ```rust ` | Rust block (tested) | OK Yes | Doctest examples — `cargo test` executes them |
| ` ```no_run ` | Compile-only Rust | OK Yes | Examples that would block / loop |
| ` ```ignore ` | Non-compiled Rust | OK Yes | Illustrative Rust that won't compile |
| ` ```json ` | JSON block | OK Yes | Benchmark JSON output |
| `[link]` | Hyperlink | OK Yes | Cross-references |
| `## Heading` | Section heading | OK Yes | Organizing long doc comments |

**Why backticks stay while asterisks go**: backticks carry
information (this exact identifier / literal expression) and are the
universal rustdoc convention; a comment without them would be harder to
read. Asterisk emphasis carries only decoration — removing it loses
nothing in the raw source and only loses visual weight in rendered
docs. That trade was decided in favor of raw-source readability.

### 2.1 Code Fence Language Consistency

**Standardized languages used in this codebase**:

- ` ```text ` for plain-text blocks (ASCII art, benchmark output samples)
- ` ```toml ` for TOML config examples
- ` ```rust ` (implicit, no language tag) — used for doctests
- ` ```no_run ` for compile-only Rust examples
- ` ```ignore ` for non-compiling Rust illustrations
- ` ```json ` for JSON output examples

**NOT acceptable**:

- ` ```txt ` (short form) — use ` ```text ` (long form) for clarity. **0 instances in codebase — keep it that way.**
- ` ``` ` (no language) for non-Rust blocks — always specify the language so syntax highlighters work.
- ` ```ts ` or ` ```js ` (TypeScript/JavaScript) — this is a Rust project; no JS examples.

### 2.2 Emphasis Alternatives (replacing italic/bold)

| Old (banned) | New (canonical) |
|--------------|-----------------|
| `**WARNING**: panics on empty input` | `WARNING: panics on empty input` |
| `the *current* phase` | `the current phase` |
| `**NOT worth it** at 60 writes/sec` | `NOT worth it at 60 writes/sec` |
| `**P1: Phase-Aware Pacing** — learns` | `P1: Phase-Aware Pacing — learns` |

CAPS for warning labels is acceptable (searchable, ASCII-only); CAPS
for whole sentences is not (shouting). Most former bold/italic spots
need no replacement at all — the sentence reads the same without the
asterisks.

## 3. Line Comments (`//`)

Line comments are for implementation notes only. They do NOT render in
`cargo doc`, so markdown syntax has no effect — it is visual noise
(same rule as doc comments now: none at all).

**Acceptable** in line comments:

- Plain prose
- ASCII diagrams (no code fence needed — just indent)
- `NOTE:`, `TODO:`, `FIXME:` prefixes (searchable)
- References to issue numbers (`#15`, `Bug #11`)
- References to commits (`commit 07b44b5`)

**NOT acceptable** in line comments:

- `*italic*` or `**bold**` markdown
- ` ```text ` code fences — same reason
- `// TODO:` without context — always explain WHY

### 3.1 Example: Good Line Comment

```rust
// Phase 5 closure (P4-8): we now have malformed_lines + unknown_keys from
// the single load_config_file_full call above — no redundant re-read.
//
// Test bypass: COSMOSTRIX_SKIP_STARTUP_VALIDATION=1 skips this check
// so existing tests that verify apply/fallback logic with invalid values
// still work. Production builds never set this env var.
```

## 4. Section Headers in Doc Comments

For long doc comments (>30 lines), use `## Heading` to organize:

```rust
//! # Module Title
//!
//! ## Overview
//!
//! Brief description.
//!
//! ## Examples
//!
//! ```rust
//! use my_crate::my_fn;
//! assert_eq!(my_fn(2), 4);
//! ```
//!
//! ## See Also
//!
//! - [`RelatedModule`]
```

**Standard section names** (rustdoc convention):

- `# Examples` — runnable examples (doctests)
- `# Panics` — conditions under which the fn panics
- `# Errors` — for `Result`-returning fns
- `# Safety` — for `unsafe` fns
- `# Arguments` — for fns with many params
- `# Returns` — for fns with non-obvious return values
- `# See Also` — cross-references

## 5. Cross-References

Use intradoc links for type references:

```rust
/// Converts a [`ColorScheme`] to its RGB palette.
///
/// See [`crate::palette::build_palette`] for the construction logic.
pub fn scheme_to_palette(scheme: ColorScheme) -> Palette { ... }
```

This renders as a hyperlink in `cargo doc` and works in IDE go-to-definition.

## 6. Audit Findings (2026-09-04 sweep)

| Pattern | Before sweep | After sweep | Status |
|---------|--------------|-------------|--------|
| `**bold**` in `///` / `//!` | 267 lines | 0 | OK Swept |
| `**bold**` multi-line spans | 5 | 0 | OK Swept (manual pass) |
| `*italic*` in `///` / `//!` | ~60 lines | 0 | OK Swept |
| `**bold**` / `*italic*` in `//` | 0 | 0 | OK Never present |
| ` ```text ` code blocks | ~25 | ~25 | OK Preserved (fence-aware sweep) |
| ` ```toml ` code blocks | 5 | 5 | OK Preserved |
| ` ```txt ` (short form) | 0 | 0 | OK Still zero |
| Inline `` `code` `` backticks | many | many | OK Preserved (functional) |

The sweep was fence-aware: content inside doc-comment code fences
(text, toml, json, doctest blocks) was never modified, and inline-code
spans keep their asterisks (e.g. `(channel * fi + 128)` inside
backticks is untouched).

## 7. Enforcement

- `scripts/check-comment-style.py` — gate-keepers check: fails on any
  `**bold**` or `*italic*` emphasis marker in a comment line (fence
  content excluded). Zero-tolerance, no allowlist.
- `cargo clippy::doc_markdown` — catches some markdown issues
- `cargo doc --no-deps` — builds the docs; warnings indicate broken syntax
- Code review — humans verify semantic correctness

If a contributor reintroduces emphasis markers, the gate blocks the
commit; rewrite the sentence in plain prose instead.
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
