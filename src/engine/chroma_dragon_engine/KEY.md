<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Chroma Dragon Engine — LTS KEY

> Latest activity on top. This file is the simplified lock/unlock
> signature log. For full audit detail (A/B benchmarks, file lists,
> stability signals), see [README.md](README.md) and [RULES.md](RULES.md).

## LOCK

> S-master-6-v2 LTS visual-impact peak audit + LOCK (2026-09-01, commit
> e93aca5): the masterclass-most-valuable axis. Verified: 12/12 tuning
> constants at sweep-audit-verified sweet spots (PALETTE_FLOOR_RATIO 0.20
> and BODY_TAIL_MAX_GAP_RATIO 2.0 each pinned by named sweep-audit tests;
> head halo / subpixel jitter / column coherence / transition window /
> anomaly cycle / border-touch amplitudes all documented with rejected
> alternatives); shipped visual identity owner-locked (Deep Focus,
> preset battle round 2); all six dragon-engine-v2 innovations live
> (cinematic shading path intentionally runs Bayer + smooth t_param +
> subpixel and skips hue-drift stacking per documented design intent);
> resource efficiency at peak (ColorCache flat SGR buffer, borrow-view
> ShaderCtx, cold-path OKLab, zero steady-state alloc — 565 bit-stable);
> security surface closed (color_tune strict grammar + [0,3] range,
> colors_custom LTS bounds, defensive color_map indexing, zero unsafe in
> engine). A/B 10s TRUECOLOR control pair: fps -0.05%, entropy +0.01%,
> gini -0.00%, color_transition_delta +0.45%, allocs bit-stable 565.
> 289 chroma + 36 lock suite + 1995 full tests green. No code changes —
> any further gain would change output bits or add unmeasurable churn.
> The dragon stays LOCKED at visual peak.
> Detail: docs/archive/audits/S6_V2_CHROMA_VISUAL_PEAK.md.
>
> Signoff: **oxyzenQ** -- 2026-09-01 -- S-master-6-v2 chroma visual-impact peak audit, lock intact

> S-master-5-v2 LTS verification (2026-09-01, commit fe571b3): deeper
> integrated-verification pass over v1 (dd34821). NEW dynamic proof:
> 10s truecolor-forced benchmark shows the chroma pipeline EXECUTING in
> the production hot loop (color_depth=truecolor, color_transition_delta
> 94.71 vs 0 on mono, entropy 4.212 vs 3.295, stability excellent,
> drift -0.24%); --doctor on the forced-truecolor path discloses
> `chroma_dragon` + `oklab gradient, perceptual blend, climate post-fx,
> head halo, l-smoothing` (v1 could only demo the legacy_rgb branch).
> Production-only census: 19/19 engine files have non-test callers
> (zero zombies). All six dragon-engine-v2 innovations verified wired
> always-Some at the DrawCtx construction site (column_coherence_lut,
> hue_drift_offset, subpixel_jitter_amplitude, head_halo_factor, Bayer
> dithering in resolve_cell_color, ghost_base_color from palette darkest
> stop). Fresh counts: 289 chroma tests, 36 lock suite, 1995 full
> binary suite, 0 failed. No code changes — engine stays locked at
> production LTS. Detail: docs/archive/audits/S5_CHROMA_INTEGRATED_VERIFY_V2.md.
>
> Signoff: **oxyzenQ** -- 2026-09-01 -- S-master-5-v2 deeper integrated verification (no code change, lock intact)

> S-master-5 verification (2026-09-01): integrated chroma dragon
> engine confirmed REAL and WORKING. Verification: 289 chroma tests
> pass (0 fail), 19 lock invariants pass (lock_inv01-19), 36 total
> lock-suite tests pass. ColorPipeline::detect() routing verified
> active in production render hot path (droplet/draw.rs: 7+ is_chroma()
> branches for blend_toward_white, scale_rgb, apply_brightness,
> vignette, fog). --doctor discloses active pipeline (chroma_dragon
> when TrueColor, legacy_rgb fallback with clear reason when not).
> No code changes required — engine is stable production LTS as-is.
> Signoff: **oxyzenQ** -- 2026-09-01 -- S-master-5 integrated verification (no code change, lock intact)

> Engine re-locked at commit `dd87239` (2026-08-26). Additive changes
> since prior lock at `deff636`: `colors_custom.rs` gained LTS bounds
> constants (COLORS_CUSTOM_MAX_BLOCKS=100, MAX_RAIN_STOPS=64,
> MAX_NAME_LEN=64) and enforcement in collect_colors_custom. These
> are additive (new constants + new guards, no modification to the
> chroma pipeline: gradient, interpolation, palette routing). Chroma
> Dragon Routing Rule unchanged. Tests: 1710/0/2, clippy clean,
> gate-keepers 8/8.
>
> Signoff: **oxyzenQ** -- 2026-08-26 -- v50.0.0-beta.6 additive bounds re-seal

> Engine re-locked at commit `deff636` after the chroma re-seal audit
> (2026-08-24). Confirms no chroma paths were touched in commits
> `5280ae1` (cosmic-side exp decay consolidation) or `deff636`
> (cosmic-side SHA backfill + README crypto donations) — chroma
> engine is untouched by the v50.0.0-beta.5 masterclass easing
> migration. The chroma UNLOCK at `a5b9345` (brand-name
> normalization, comment-only across `catalog.rs`,
> `intro_colors.rs`, `palette/mod.rs`, `mod.rs`) is closed and
> re-sealed — zero production code touched, no chroma routing
> violation, lock invariants unchanged. Chroma Dragon Routing
> Rule re-verified across all render paths (rain cells
> `resolve_cell_color`, vignette/brightness
> `apply_brightness_rgb_unclamped`, monolith core
> `blend_toward_white_rgb`, message border gradient
> `interpolate_palette_color` — BD-02 corner system intact, intro
> cinematic colors chroma-owned via `intro_colors.rs`, post-FX
> engine-internal, legacy `chroma::legacy` fallbacks). A/B: no
> chroma code touched -> no measurable delta vs `c1c7779`
> baseline. Tests: chroma lock suite 19/19, full binary suite
> 1660/0/2. cargo fmt + clippy + gate-keepers all clean.
>
> Signoff: **oxyzenQ** — 2026-08-24 — chroma re-seal after cosmic-side
> v50.0.0-beta.5 amendments

> Engine re-locked at commit `c1c7779` after the triple-engine LTS deeper
> audit with integrated routing re-verification (2026-08-23). The Chroma
> Dragon Routing Rule was re-swept across every render path: rain cells
> (resolve_cell_color), vignette/brightness
> (apply_brightness_rgb_unclamped), monolith core (blend_toward_white_rgb),
> message border gradient (interpolate_palette_color — BD-02 corner system
> confirmed working), intro cinematic (color constants chroma-owned via
> intro_colors.rs, palette extraction via color_to_rgb, blends via
> oklab_blend_rgb), post-FX (ghost/climate/anomaly engine-internal), and
> legacy Color16/256 fallbacks (chroma::legacy). HUD and CLI colors
> confirmed as the documented diagnostic exception (fixed semantic colors,
> not part of the cinematic pipeline). No routing violations found — zero
> code changes required; the lock is the appropriate action. A/B: avg_fps
> 86,520/86,615 (±0.1% run variance; vs baseline 90,819 the delta is
> cross-session hardware variance), alloc_calls 563 exact-match
> (0.0/frame), density_gini 0.8960 (baseline 0.8961, Δ -0.01%),
> color_transition_delta 0.00, frame_entropy 3.30, jitter=low,
> stability=excellent, drift=stable. Lock suite 19/19, full binary suite
> 1642 passed / 0 failed / 2 ignored.
>
> Signoff: **oxyzenQ** — 2026-08-23T09:27:41Z — triple-engine LTS deeper audit

> Engine re-locked at commit `24fa1be` after final Chroma Dragon integration
> audit (v50.0.0-alpha.7). Deep audit confirmed: ALL Color::Rgb constructors
> in render paths route through Chroma Dragon functions (blend_toward_rgb,
> scale_rgb, apply_brightness_rgb_unclamped, interpolate_palette_color). No
> hardcoded Color::White or Color::Rgb bypassing the engine. Border gradient
> fixed: triangle wave eliminates sharp white->black gap on left border.
> Chroma Dragon Routing Rule codified in RULES.md. 19/19 lock invariants
> pass. A/B: avg_fps 90,819, 0 alloc/frame, stability=excellent. No regression.
>
> Signoff: **oxyzenQ** — 2026-08-22T16:30:00Z — final dragon audit v50.0.0-alpha.7

> Engine re-locked at commit `0a86ff6` after deep zombie audit of
> `shaders/` (~3900 LOC) confirmed clean — zero zombie symbols found.
> All `pub(crate)` items verified with production callers: `ShaderCtx`,
> `CharLoc`, `TRAIL_EXP_LUT`, `column_coherence_perturbation`,
> `hue_drift_offset`, `color_uses_previous_palette`,
> `resolve_cell_color`, `TransitionLabEntry`, `TransitionLTable`,
> `apply_l_smoothing`. No code changes applied — the lock is the
> appropriate action.
>
> Signoff: **oxyzenQ** — 2026-08-22T09:01:59Z — chroma-dragon zombie audit

> **3 Dragon Lock** in commit `69af079` after deeper audit for strengthening
> and stability.
>
> Signoff: **rezky_nightky** — 2026-08-19T14:40:05Z — vision & director
> project cosmostrix

## UNLOCK
>
> **UNLOCK chroma-dragon (comment-only)** at commit `a5b9345`, 2026-08-24T00:30:00Z
>
> **Author**: oxyzenQ (Cosmic Dragon AI Agent)
> **Reason**: Project naming normalization — the capitalized form -> `cosmostrix`
> in comment text across chroma dragon engine files. No production code
> touched; comment/word only.
>
> **Files changed** (comments only):
> - `src/engine/chroma_dragon_engine/catalog.rs` (brand name in comment)
> - `src/engine/chroma_dragon_engine/intro_colors.rs` (brand name in comment)
> - `src/engine/chroma_dragon_engine/palette/mod.rs` (brand name in comment)
> - `src/engine/chroma_dragon_engine/mod.rs` (brand name in module doc)
>
> **A/B delta**: none — zero production code touched.
>
> **Visual audit**: PASS — no code changes; visual identity preserved.
>
> **Tests**: full suite 1656 passed / 0 failed / 2 ignored.
>
> Signoff: **oxyzenQ** — 2026-08-24 — brand name normalization

> Deep zombie audit of `shaders/` in commit `0a86ff6`. Opened audit
> because previous zombie sweep (commit `3587ccb`) skipped this
> directory. Verified zero zombies across 7 source files. Audit
> closed with no code changes.
>
> Signoff: **oxyzenQ** — 2026-08-22T09:01:59Z — chroma-dragon zombie audit

> Stale path refs + EnergyZen missing from `all_schemes()` test helper
> (INV-2 silently skipped v50 masterclass theme) in commit `809a897`.
> Real bug fix: a future regression in EnergyZen's palette construction
> would NOT have been caught by the lock suite. Also 15+ stale doc
> path refs updated.
>
> Signoff: **oxyzenQ** — 2026-08-19T16:36:02Z — chroma-dragon deeper audit
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
