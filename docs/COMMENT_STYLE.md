<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Comment Style Guide — Rust Source Comments

> **Audit 2026-08-19**: Owner reported seeing `*abc*` and ` ```txt ` patterns
> in source comments, considered them "inconsistent". Deep audit confirmed
> these are **valid rustdoc markdown** that renders correctly in `cargo doc`
> and IDE tooltips — NOT inconsistencies. This document codifies the
> convention so future contributors don't introduce actual inconsistencies.

## 1. Comment Types

| Prefix | Type | Purpose | Renders in `cargo doc`? |
|--------|------|---------|------------------------|
| `//!` | Module doc | Top-of-file module documentation | ✅ Yes |
| `///` | Item doc | Documentation for a fn / struct / enum / const | ✅ Yes |
| `//` | Line comment | Implementation note, NOT user-facing | ❌ No |
| `/* */` | Block comment | Rare; multi-line implementation note | ❌ No |

**Rule**: Anything user-facing (API contract, behavior, rationale that
a user/contributor needs to understand the public surface) goes in
`///` or `//!`. Implementation details (why this loop is unrolled,
why this magic number is 256 and not 255) go in `//`.

## 2. Markdown in Doc Comments (`///` and `//!`)

Rustdoc supports standard CommonMark. The following are **all valid
and should be used when semantically appropriate**:

| Syntax | Meaning | Example | When to use |
|--------|---------|---------|-------------|
| `*italic*` | Emphasis | `the *current* phase` | When calling attention to a word as a concept |
| `**bold**` | Strong emphasis | `**WARNING**: panics on empty input` | For warnings, critical notes |
| `` `code` `` | Inline code | `Returns`Option<Color>`` | For type names, function names, identifiers |
| ` ```text ` | Plain-text code block | ASCII art, benchmark output | For non-syntax-highlighted blocks |
| ` ```toml ` | TOML code block | Config examples | For `[section]` + `key = value` snippets |
| ` ```rust ` | Rust code block (tested) | Doctest examples | For runnable examples — `cargo test` will execute |
| ` ```no_run ` | Rust code block (compiled, not run) | Compile-check examples | For examples that would block / loop |
| ` ```ignore ` | Rust code block (not compiled) | Non-runnable Rust | For illustrative Rust that won't compile |
| ` ```json ` | JSON code block | Benchmark JSON output | For JSON examples |
| `[link]` | Hyperlink | `[Render Engine](RENDER_ENGINE.md)` | For cross-references |
| `## Heading` | Section heading | `## Examples` | For organizing long doc comments |

### 2.1 Code Fence Language Consistency

**Standardized languages used in this codebase**:

- ` ```text ` for plain-text blocks (ASCII art, benchmark output samples) — **25 instances, consistent**
- ` ```toml ` for TOML config examples — used in `configfile.rs`, `scene_custom/mod.rs`, `colors_custom.rs`, `crystal_dragon_engine/ambient/mod.rs`
- ` ```rust ` (implicit, no language tag) — used for doctests
- ` ```no_run ` for compile-only Rust examples — `output.rs:345`
- ` ```ignore ` for non-compiling Rust illustrations — `ux.rs`, `chroma_dragon_engine/post/anomaly/mod.rs`
- ` ```json ` for JSON output examples — `bench/bench_json.rs`

**NOT acceptable**:

- ` ```txt ` (short form) — use ` ```text ` (long form) for clarity. **0 instances in codebase — keep it that way.**
- ` ``` ` (no language) for non-Rust blocks — always specify the language so syntax highlighters work.
- ` ```ts ` or ` ```js ` (TypeScript/JavaScript) — this is a Rust project; no JS examples.

### 2.2 Italic vs Bold vs Code — Semantic Distinction

These three are **NOT interchangeable**. Each serves a distinct purpose:

| Want to call attention to… | Use | Example |
|---------------------------|-----|---------|
| A concept or term | `*italic*` | `the *current* phase` |
| A critical warning | `**bold**` | `**WARNING**: panics on empty input` |
| A type/identifier/keyword | `` `code` `` | `Returns`Option<Color>`` |
| A multi-line code sample | ` ```lang ` block | see above |

**Do NOT** use `**bold**` for emphasis that should be `*italic*`, and
**do NOT** use `` `code` `` for terms that aren't actual code identifiers.

## 3. Line Comments (`//`)

Line comments are for implementation notes only. They do NOT render in
`cargo doc`, so markdown syntax has no effect.

**Acceptable** in line comments:

- Plain prose
- ASCII diagrams (no code fence needed — just indent)
- `NOTE:`, `TODO:`, `FIXME:` prefixes (searchable)
- References to issue numbers (`#15`, `Bug #11`)
- References to commits (`commit 07b44b5`)

**NOT acceptable** in line comments:

- `*italic*` or `**bold**` markdown — it won't render, so it's just visual noise
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

### 3.2 Example: Bad Line Comment (Avoid)

```rust
// *Note*: this is **important**.
// ```text
// some output
// ```
```

The `*Note*` and `**important**` won't render — they're just literal
asterisks. The ` ```text ` fence is also literal. Use plain prose
instead:

```rust
// Note: this is important.
// some output (just indent, no fence)
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

## 6. Audit Findings (2026-08-19)

The deep audit found **no actual inconsistencies** in the codebase:

| Pattern | Count | Status |
|---------|-------|--------|
| `*italic*` in `///` / `//!` | 10 | ✅ All valid rustdoc |
| `**bold**` in `///` / `//!` | many | ✅ All valid rustdoc |
| ` ```text ` code blocks | 25 | ✅ Consistent (no ` ```txt ` short form) |
| ` ```toml ` code blocks | 5 | ✅ Correct for TOML examples |
| ` ```ignore ` / ` ```no_run ` / ` ```json ` | 6 | ✅ Correct rustdoc languages |
| `*italic*` in `//` (non-doc) | 0 | ✅ No misuse |
| ` ```txt ` (short form) | 0 | ✅ Already consistent |

**Conclusion**: The codebase already follows the convention documented
here. This file exists to **codify** the convention so future
contributors don't introduce actual inconsistencies.

## 7. Enforcement

- `cargo clippy::doc_markdown` — catches some markdown issues
- `cargo doc --no-deps` — builds the docs; warnings indicate broken syntax
- Code review — humans should verify semantic correctness (italic vs bold
  vs code)

If a contributor introduces ` ```txt ` (short form) or `*italic*` in a
non-doc comment, the reviewer should request a change per this guide.
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
