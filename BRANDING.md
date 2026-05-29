# Cosmostrix Brand Guidelines

This document defines the visual identity and communication standards for the
Cosmostrix project. It ensures consistent branding across all touchpoints —
from the GitHub repository to documentation, releases, and community presence.

---

## 1. Brand Identity

**Cosmostrix** is a high-performance cinematic Matrix rain renderer for the terminal, built in Rust.
The brand reflects:

- **Precision** — engineered performance, SIMD optimization, zero-compromise rendering
- **Atmosphere** — cinematic, immersive, cosmic visual experience
- **Craftsmanship** — mature open-source project with rigorous CI/CD and cross-platform support

The brand sits at the intersection of **systems engineering** and **visual art**.

---

## 2. Name Usage

### 2.1. Correct forms

| Context | Format |
|---|---|
| Running text / prose | Cosmostrix |
| Titles / headings | Cosmostrix |
| Code / CLI | `cosmostrix` (lowercase) |
| All-caps hero (README hero only) | COSMOSTRIX |
| With article | "the Cosmostrix project", "Cosmostrix renderer" |

### 2.2. Incorrect forms

- ~~CosmoStrix~~ (no internal capitalization)
- ~~COSMOSTRIX~~ (except README hero section)
- ~~cosmostrix~~ in prose (use capitalized form)
- ~~Cosmo~~ as abbreviation (use full name)

### 2.3. First mention

In external articles or documentation, the first mention should include context:

> Cosmostrix is a high-performance cinematic Matrix rain renderer for the terminal.

Subsequent mentions may use "Cosmostrix" alone.

---

## 3. Logo

### 3.1. Logo file

The official logo is located at [`assets/logo.png`](assets/logo.png).

### 3.2. Usage rules

- **Minimum size**: 64px width for print, 32px for digital
- **Clear space**: maintain padding equal to at least 25% of the logo height
  on all sides
- **Background**: designed for dark backgrounds; avoid placing on busy or
  light-colored surfaces without a dark container
- **Aspect ratio**: always preserve the original square aspect ratio — do not
  stretch or distort

### 3.3. Do

- Use the official logo file without modification
- Place on dark or neutral backgrounds
- Maintain clear space around the logo
- Use at appropriate sizes for the medium

### 3.4. Don't

- Modify, recolor, or add effects to the logo
- Stretch, rotate, or skew the logo
- Place on clashing backgrounds without contrast
- Use the logo as a bullet point or inline icon
- Create your own version of the logo

---

## 4. Color Palette

The Cosmostrix palette is derived from the project's cinematic, cosmic
aesthetic — a dark base with vibrant green phosphor accents, inspired by
classic terminal displays and deep-space visuals.

### 4.1. Primary colors

| Role | Color | Hex | RGB | Usage |
|---|---|---|---|---|
| Background | Void Black | `#0A0A0A` | 10, 10, 10 | Page backgrounds, containers |
| Surface | Deep Space | `#121212` | 18, 18, 18 | Cards, panels, code blocks |
| Surface elevated | Nebula Dark | `#1A1A1A` | 26, 26, 26 | Elevated elements, borders |
| Text primary | Phosphor White | `#E0E0E0` | 224, 224, 224 | Body text, headings |
| Text secondary | Dim Star | `#888888` | 136, 136, 136 | Captions, metadata, muted text |

### 4.2. Accent colors

| Role | Color | Hex | RGB | Usage |
|---|---|---|---|---|
| Accent primary | Cosmostrix Green | `#40C000` | 64, 192, 0 | Links, highlights, active states |
| Accent bright | Phosphor Glow | `#80C040` | 128, 192, 64 | Logo glow, emphasis, hover states |
| Accent warm | Solar Flare | `#C0C040` | 192, 192, 64 | Warnings, secondary highlights |
| Accent bright white | Star Core | `#C0C080` | 192, 192, 128 | Bright text on dark backgrounds |

### 4.3. Semantic colors

| Role | Color | Hex | Usage |
|---|---|---|---|
| Success | `#40C000` | Build passed, tests green |
| Warning | `#C0C040` | Deprecated, caution |
| Error | `#C04040` | Build failed, critical |
| Info | `#4080C0` | Informational notices |

### 4.4. Color usage notes

- The green accent palette should feel **terminal-phosphor**, not generic
  "startup green." Use muted, slightly desaturated greens rather than neon.
- Avoid using multiple accent colors in the same element — one accent per
  visual unit.
- When in doubt, default to dark backgrounds with green or white text.

---

## 5. Typography

### 5.1. Code and terminal contexts

Use monospace fonts for all code, CLI output, and technical references:

- **Primary**: `JetBrains Mono`, `Fira Code`, or system monospace
- **Fallback**: `Consolas`, `Monaco`, `monospace`

### 5.2. Documentation and prose

- **Headings**: `Inter`, `SF Pro`, or system sans-serif, bold
- **Body**: Same family as headings, regular weight
- **Code inline**: Monospace (as above), with a subtle background (`#1A1A1A`)

### 5.3. Size hierarchy

| Element | Relative size | Weight |
|---|---|---|
| Page title | 2x base | Bold |
| Section heading | 1.5x base | Semibold |
| Subheading | 1.25x base | Medium |
| Body | 1x base | Regular |
| Caption / metadata | 0.85x base | Regular |
| Code inline | 0.9x base | Regular (monospace) |

---

## 6. Tone of Voice

Cosmostrix's communication style should reflect the project's identity:
technically precise but not cold, confident but not arrogant.

### 6.1. Principles

- **Direct and technical** — favor clarity over marketing fluff. Describe what
  the project does and how it performs, not how it "revolutionizes" anything.
- **Confident, not boastful** — let benchmarks and features speak for
  themselves. Avoid superlatives unless backed by data.
- **Concise** — respect the reader's time. Short paragraphs, clear structure,
  scannable formatting.
- **Professional with personality** — this is an open-source project, not a
  corporate whitepaper. A dry humor reference or space metaphor is welcome
  when it fits naturally.

### 6.2. Examples

**Good**:
> Cosmostrix renders cinematic terminal visuals at 240 FPS with AVX-512
> optimized rendering and adaptive CPU throttling.

**Avoid**:
> Cosmostrix is the world's most revolutionary groundbreaking terminal
> experience that will completely transform how you think about terminals!

**Good**:
> Phosphor persistence simulates CRT afterglow for authentic retro aesthetics.

**Avoid**:
> Our amazing phosphor technology creates an unparalleled visual journey!

### 6.3. Commit messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): description

feat(renderer): add parallax depth layer
fix(windows): correct ANSI escape sequence on conhost
perf(core): reduce allocation in hot path
docs(readme): update installation instructions
ci(workflows): add path filtering
```

---

## 7. GitHub Presence

### 7.1. Repository structure

The repo should present a clean, organized first impression:

- README hero: logo + title + tagline + badges (already implemented)
- Topics: `matrix`, `matrix-rain`, `terminal`, `renderer`, `ansi`, `rust`,
  `cli`, `ascii-art`, `cinematic`, `simd`, `terminal-emulator`
- About section: concise description with link to releases

### 7.2. Release notes

Release notes should follow this structure:

```
## v1.x.x

### Highlights
- Key feature or improvement (1-3 items)

### Changes
- feat: new features
- fix: bug fixes
- perf: performance improvements
- chore: maintenance

### Assets
- Platform binaries with SHA-512 checksums
```

### 7.3. Issue and PR templates

- Use technical, specific titles
- Include environment details (OS, terminal, Rust version)
- Attach logs or screenshots when relevant

---

## 8. Third-party Usage

External projects, articles, or distributions referencing Cosmostrix should:

- Use the correct project name and spelling
- Link to the official repository: <https://github.com/oxyzenQ/cosmostrix>
- Not use the logo for commercial purposes without permission (see
  [`TRADEMARK.md`](TRADEMARK.md))
- Attribute the project when redistributing modified versions

---

*This document is a living guide and may be updated as the project evolves.
For trademark and legal usage terms, see [`TRADEMARK.md`](TRADEMARK.md).*
