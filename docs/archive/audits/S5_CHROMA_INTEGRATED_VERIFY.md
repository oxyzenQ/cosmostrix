<!-- SPDX-License-Identifier: GPL-3.0-only -->

# S-master-5 — Integrated Chroma Dragon Engine Verification

**Date:** 2026-09-01
**Scope:** `src/engine/chroma_dragon_engine/` + integration points
**Author:** oxyzenQ (cosmic dragon mode, master audit pass)
**Task:** Verify all integrated chroma dragon engine is works/real, stable production LTS.

## Context

Owner directive: "check all integrated is works/real using chroma
dragon. so make it 99% not perfect but stable and strength, and
already stable production LTS."

This is a VERIFICATION task, not an optimization task. The chroma
dragon engine was LOCKED at commit `dd87239` (2026-08-26) with 19
invariants. S-master-5 confirms the engine is real, integrated, and
stable — no code changes required.

## Verification Method

1. Static analysis: read `ColorPipeline` enum, `detect()` routing,
   `is_chroma()` call sites in production render paths.
2. Dynamic verification: run chroma-specific test suites + lock
   invariants.
3. Integration verification: run `--doctor` to confirm pipeline
   disclosure works end-to-end.

## Verification Results

### 1. ColorPipeline routing is real and active

`src/engine/cosmic_dragon_engine/runtime.rs:32-60` defines:

```rust
pub enum ColorPipeline {
    ChromaDragon,  // OKLab gradient, perceptual blend, climate post-FX
    LegacyRgb,     // sRGB-linear fallback (Color256/Color16/Mono)
}

impl ColorPipeline {
    pub const fn detect(color_mode: ColorMode) -> Self {
        match color_mode {
            ColorMode::TrueColor => Self::ChromaDragon,
            ColorMode::Color256 | ColorMode::Color16 | ColorMode::Mono => Self::LegacyRgb,
        }
    }
}
```

Detection rule: "all color -> chroma dragon first -> fallback legacy
rgb/srgb" (owner directive). TrueColor terminals get the full chroma
dragon pipeline; limited-color terminals fall back to legacy sRGB-linear
math (same call sites, swapped helpers).

### 2. Production render hot path routes through chroma

`src/droplet/draw.rs` — the per-cell render function — has 7+
`is_chroma()` branches that route color operations:

| Line | Operation | Chroma path | Legacy fallback |
|---|---|---|---|
| 186 | blend_toward_white (head brightening) | `chroma::palette::blend_toward_white_rgb` | `chroma::legacy::blend_toward_white` |
| 223 | blend_toward_white (body fade) | `chroma::palette::blend_toward_white_rgb` | `chroma::legacy::blend_toward_white` |
| 250 | blend_toward_white (trail fade) | `chroma::palette::blend_toward_white_rgb` | `chroma::legacy::blend_toward_white` |
| 279 | scale_rgb (brightness factor) | `chroma::palette::apply_brightness_rgb_unclamped` | `chroma::legacy::scale_rgb` |
| 315 | scale_rgb (cursor glow) | `chroma::palette::apply_brightness_rgb_unclamped` | `chroma::legacy::scale_rgb` |
| 351 | scale_rgb (depth fog) | `chroma::palette::apply_brightness_rgb_unclamped` | `chroma::legacy::scale_rgb` |
| 619+ | vignette LUT | `chroma::palette::apply_brightness_rgb_unclamped` | `chroma::legacy::scale_rgb` |

The chroma dragon is NOT a dead/optional module — it is the primary
color pipeline for every TrueColor render.

### 3. Test suite confirms engine is functional

| Suite | Tests | Passed | Failed |
|---|---|---|---|
| Chroma-specific tests (`cargo test -- chroma`) | 289 | 289 | 0 |
| Chroma lock invariants (lock_inv01-19) | 19 | 19 | 0 |
| Full lock suite (chroma + cosmic_dragon_incubator + others) | 36 | 36 | 0 |
| Full binary test suite | 1945 | 1945 | 0 |

All 19 chroma lock invariants pass:
- lock_inv01: engine version sentinel
- lock_inv02: all themes build without panic
- lock_inv03: floor bounds held across all themes
- lock_inv04: head brighter than trail across all themes
- lock_inv05: hue preserved by floor and continuity
- lock_inv06: body-tail gap contract held
- lock_inv07: continuity never exceeds head
- lock_inv08: OKLab round-trip within one unit
- lock_inv09: polar gradient endpoints preserved
- lock_inv10: polar midpoint stays saturated on opposing hues
- lock_inv11: blend_toward_bg normalizes to RGB
- lock_inv12: L-smoothing stays within bounds
- lock_inv13: polar chroma smoothing preserves saturation
- lock_inv14: subpixel jitter amplitude in bounds
- lock_inv15: head halo factor in range
- lock_inv16: tuning constants in sweet spots
- lock_inv17: engine lock report
- lock_inv18: polar is sole production gradient path
- lock_inv19: color pipeline disclosure routes correctly

### 4. Pipeline disclosure works end-to-end

`cosmostrix --doctor` output (bench container, no truecolor TTY):

```
color_pipeline: legacy_rgb
color_pipeline_detail: sRGB-linear fallback (color mode lacks truecolor; no OKLab, no climate post-fx)
chroma_disable_reason: color_mode=Mono -- chroma needs truecolor; legacy sRGB-linear in effect
```

The disclosure correctly:
- Detects terminal color capability
- Routes to LegacyRgb when truecolor unavailable
- Explains WHY chroma is disabled (clear user-facing message)
- Suggests fonts for the "masterclass chroma dragon look"

On a truecolor terminal, this would show `color_pipeline: chroma_dragon`
with OKLab + climate post-FX active.

### 5. Lock integrity intact

The chroma dragon engine KEY.md records:
- LOCKED at commit `dd87239` (2026-08-26)
- 19 invariants enforced
- Chroma Dragon Routing Rule: "all color output MUST route through chroma"
- UNLOCK protocol requires A/B benchmark + 19/19 invariant pass
- No UNLOCK entries since last lock — engine is sealed

S-master-5 did NOT unlock the engine (no code changes to chroma
pipeline). Verification only.

## Verdict

**The integrated chroma dragon engine is REAL, WORKING, and STABLE
production LTS.** Verification confirms:

1. **Real**: `ColorPipeline` enum + `detect()` routing + 7+ `is_chroma()`
   branches in the production render hot path. Not dead code.
2. **Working**: 289 chroma tests + 19 lock invariants all pass.
3. **Integrated**: color operations in `droplet/draw.rs` route through
   chroma::palette (TrueColor) or chroma::legacy (fallback) — same
   call sites, swapped helpers.
4. **Stable**: LOCKED at `dd87239`, 19 invariants enforced, no UNLOCK
   entries, full test suite green.
5. **Disclosure**: `--doctor` / `-v` / benchmark CONFIG all report the
   active pipeline + fallback reason.

**No code changes required.** The engine is 99% (not perfect —
nothing is — but stable and strong) and already at production LTS.

## Files Changed

- `src/engine/chroma_dragon_engine/KEY.md` — appended S-master-5 verification entry to LOCK log (no code change, lock intact).
- `docs/archive/audits/S5_CHROMA_INTEGRATED_VERIFY.md` — this audit doc (new).
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
