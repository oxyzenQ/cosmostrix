# Cosmostrix Brand Guidelines
<!-- SPDX-License-Identifier: GPL-3.0-only -->

Visual identity and communication standards for the Cosmostrix project. Ensures consistent branding across all touchpoints — GitHub repository, documentation, releases, community presence.

## 1. Brand Identity

**Cosmostrix** is a high-performance cinematic Matrix rain renderer for the terminal, built in Rust. The brand sits at the intersection of **systems engineering** and **visual art**, reflecting:

- **Precision** — engineered performance, SIMD optimization, zero-compromise rendering
- **Atmosphere** — cinematic, immersive, cosmic visual experience
- **Craftsmanship** — mature open-source project with rigorous CI/CD and cross-platform support

## 2. Name Usage

| Context | Format |
|---|---|
| Running text / prose / titles / headings | Cosmostrix |
| Code / CLI | `cosmostrix` (lowercase) |
| All-caps hero (README hero only) | COSMOSTRIX |
| With article | "the Cosmostrix project", "Cosmostrix renderer" |

**Incorrect forms**: ~~CosmoStrix~~ (no internal capitalization), ~~COSMOSTRIX~~ (except README hero), ~~cosmostrix~~ in prose, ~~Cosmo~~ as abbreviation. In external articles, the first mention should include context: "Cosmostrix is a high-performance cinematic Matrix rain renderer for the terminal."

## 3. Logo

The official logo is at [`assets/cosmostrix-logo.png`](assets/cosmostrix-logo.png). Usage rules:

- **Minimum size**: 64px width for print, 32px for digital.
- **Clear space**: padding equal to at least 25% of logo height on all sides.
- **Background**: designed for dark backgrounds; avoid busy or light-colored surfaces without a dark container.
- **Aspect ratio**: always preserve the original square aspect ratio — do not stretch or distort.

**Do**: use the official logo file without modification; place on dark/neutral backgrounds; maintain clear space. **Don't**: modify, recolor, add effects, stretch/rotate/skew, place on clashing backgrounds, use as a bullet point or inline icon, or create your own version.

## 4. Color Palette

The Cosmostrix palette is derived from the project's cinematic, cosmic aesthetic — dark base with vibrant green phosphor accents, inspired by classic terminal displays and deep-space visuals.

### Primary + Accent

| Role | Color | Hex | RGB | Usage |
|---|---|---|---|---|
| Background | Void Black | `#0A0A0A` | 10,10,10 | Page backgrounds, containers |
| Surface | Deep Space | `#121212` | 18,18,18 | Cards, panels, code blocks |
| Surface elevated | Nebula Dark | `#1A1A1A` | 26,26,26 | Elevated elements, borders |
| Text primary | Phosphor White | `#E0E0E0` | 224,224,224 | Body text, headings |
| Text secondary | Dim Star | `#888888` | 136,136,136 | Captions, metadata, muted text |
| Accent primary | Cosmostrix Green | `#40C000` | 64,192,0 | Links, highlights, active states |
| Accent bright | Phosphor Glow | `#80C040` | 128,192,64 | Logo glow, emphasis, hover states |
| Accent warm | Solar Flare | `#C0C040` | 192,192,64 | Warnings, secondary highlights |
| Accent bright white | Star Core | `#C0C080` | 192,192,128 | Bright text on dark backgrounds |

### Semantic

| Success | Warning | Error | Info |
|---|---|---|---|
| `#40C000` | `#C0C040` | `#C04040` | `#4080C0` |

The green accent palette should feel **terminal-phosphor**, not generic "startup green" — use muted, slightly desaturated greens rather than neon. Avoid multiple accent colors in the same element; default to dark backgrounds with green or white text.

## 5. Typography

- **Code/terminal**: `JetBrains Mono` (or `Iosevka` / `Monaspace Krypton`; fallback `Consolas`, `Monaco`, `monospace`). Avoid `Fira Code` (ligatures disrupt `0`/`1` rain) and system defaults lacking full Unicode coverage.
- **Documentation prose**: `Inter`, `SF Pro`, or system sans-serif. Headings bold; body regular. Inline code in monospace with subtle background (`#1A1A1A`).

| Element | Relative size | Weight |
|---|---|---|
| Page title | 2x base | Bold |
| Section heading | 1.5x base | Semibold |
| Subheading | 1.25x base | Medium |
| Body | 1x base | Regular |
| Caption / metadata | 0.85x base | Regular |
| Code inline | 0.9x base | Regular (monospace) |

## 6. Tone of Voice

Technically precise but not cold; confident but not arrogant. **Principles**: direct and technical (favor clarity over marketing fluff); confident, not boastful (let benchmarks speak); concise (short paragraphs, scannable formatting); professional with personality (a dry humor reference or space metaphor is welcome when it fits).

**Good**: "Cosmostrix renders cinematic terminal visuals at practical terminal-bounded FPS (60–240 on modern terminals) using diff-based rendering with adaptive CPU throttling."

**Avoid**: "Cosmostrix is the world's most revolutionary groundbreaking terminal experience that will completely transform how you think about terminals!"

**Commit messages** follow [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): description` — e.g. `feat(renderer): add parallax depth layer`, `fix(windows): correct ANSI escape on conhost`, `perf(core): reduce allocation in hot path`.

## 7. GitHub Presence

- **Topics**: `matrix`, `matrix-rain`, `terminal`, `renderer`, `ansi`, `rust`, `cli`, `ascii-art`, `cinematic`, `simd`, `terminal-emulator`.
- **Release notes**: highlights (1-3 items) + changes grouped by conventional-commit type (feat/fix/perf/chore) + assets with SHA-512 checksums.
- **Issues/PRs**: technical, specific titles; include environment details (OS, terminal, Rust version); attach logs or screenshots when relevant.

## 8. Third-party Usage

External projects, articles, or distributions referencing Cosmostrix should: use the correct project name and spelling; link to the official repository (<https://github.com/oxyzenQ/cosmostrix>); not use the logo for commercial purposes without permission (see [`TRADEMARK.md`](TRADEMARK.md)); attribute the project when redistributing modified versions.

*For trademark and legal usage terms, see [`TRADEMARK.md`](TRADEMARK.md).*
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
