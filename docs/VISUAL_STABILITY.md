# Visual Stability — Depth Regression Lab

**Version**: v4.5.0 Phase 3  
**Status**: Regression Lab (no visual redesign)  
**Reference**: v4.0.1 Monolith Rain visual identity

## Purpose

The Depth Regression Lab is a suite of deterministic tests that lock down the v4.0.1/v4.5 Monolith Rain visual identity. These tests exist to prevent future regressions in cinematic depth, empty-space ratio, muddy residue, brightness hierarchy, and transition cleanliness.

This is **not** a visual redesign. It is a protective test suite that future v4.8.0 optimization work MUST pass before merge.

## Visual Identity Invariants

The following invariants define the v4.0.1 Monolith Rain visual identity. All future optimization must preserve these:

### 1. Monolith Rain Depth Remains Stable

Monolith Rain produces a cinematic depth effect with clearly distinguishable brightness levels: Ghost (faintest), Dim, Mid, Hot, and Core (brightest with white bloom). These levels must remain visually distinct on all dark-background color schemes.

- Ghost and Dim cells must use the faintest palette entries (RGB sum < 80 on black backgrounds)
- Mid must be noticeably brighter than Ghost/Dim
- Hot must be strictly brighter than Mid
- Core must bloom with near-white brightness

### 2. Hero/Spine/Trail/Empty-Space Hierarchy

The visual hierarchy must remain distinct:

- **Hero/Core**: Brightest cells at the stream head — visually dominant
- **Spine**: Subtle local vertical accents — never forming continuous walls
- **Trail/Segments**: Medium-brightness body cells that follow the head
- **Ghost**: Faintest afterglow that fades to blank within bounded frames
- **Empty Space**: The majority of the screen must remain blank

No single column should have more than 60% consecutive fill. The overall visible cell ratio must stay below 35% for monolith mode.

### 3. No Muddy Residue Regression

The "flat wall of grey" artifact occurs when ghost/dim cells map to mid-range grey on dark backgrounds, creating an indistinct flat wall instead of clear depth hierarchy. This must never regress.

Three color profile families are guarded: green-dark, blue-dark, and cyan-dark.

### 4. No Full-Height Continuous Wall/Spine

Monolith spine columns must remain local fragments. No column should have a continuous vertical run exceeding 2 cells, and spines should occupy less than 1/3 of any lane's height. This is established by the v3.9.0 Monolith Subtlety Policy: Organic does not mean chaotic, and full-height spine walls are explicitly forbidden.

### 5. Scene Switching Remains Clean

Switching from Monolith to Matrix or Signal must:

- Clear all monolith draw history immediately
- Clear all monolith drawn cells
- Warm-start glyph droplets in the upper quarter of the viewport
- Produce visible dirty cells on the first frame (no blank intermediate screen)

Switching from Glyph to Monolith must reinitialize cleanly with no leftover glyph residue in the monolith draw path.

### 6. Color Remains Sticky Unless Intentionally Changed

Explicit CLI/config/profile color choices remain sticky across simulated runtime:

- `--color sun` stays Sun indefinitely unless user presses c/C or scene changes
- `auto_color_drift` is opt-in only (defaults false)
- Color transitions complete cleanly with no stale palette references

### 7. Bottom Residue Remains Bounded

After sustained high-speed rain (300-500 frames), the bottom rows must not accumulate unbounded residue. The bottom 4-5 rows should stay below 50-70% visible cells depending on mode.

### 8. Top Cells Clear Within Bounded Frames

After high-speed monolith rain stops spawning, the top rows must clear within 4 bounded cleanup frames.

### 9. Resize Reset Clears Stale History

Terminal resize must clear all monolith draw caches, phosphor state, and request semantic invalidation. No stale monolith history from the previous terminal size should persist.

### 10. Charset Transition Does Not Flood Background

When charset transitions complete, the visible cell count must not suddenly flood. The ratio of visible cells before/after transition must be bounded.

### 11. Zactrix Diagnostics Remain Honest

The ZACTRIX ENGINE and ZACTRIX SYSTEM diagnostic sections must honestly report:

- `actual_execution: single-threaded-renderer` (no real parallel execution)
- `terminal_writer: single-owner` (terminal writes never parallelized)
- `compute_parallelism: disabled` (no active parallel compute)
- `render_plan: single-owner` (render pipeline uses single-owner model)
- `runtime_mode: normal` (default operating mode)

## Design Principles

### Parallel Compute, Single-Owner Terminal Writer

The Zactrix Engine may optimize planning and scheduling in future v4.8.0/v5.0.0 releases, but it must never:

- Change terminal writer ownership from single-owner
- Flatten cinematic depth for throughput
- Create parallel terminal writes
- Claim active parallel compute in diagnostics when none exists

### Future v4.8.0 Optimization Must Pass This Lab

Any future optimization that touches the renderer, cloud module, monolith module, phosphor system, or droplet lifecycle must pass all Depth Regression Lab tests. If an optimization cannot pass these tests, it must be redesigned. Zactrix Core may guide scheduling and cache planning, but must not flatten cinematic depth or change terminal writer ownership.

## Test Categories

### Cloud Tests (`src/cloud/tests/`)

| File | Tests | Category |
|------|-------|----------|
| `tests_visual_depth.rs` | `depth_lab_empty_space_ratio_above_threshold` | Empty space invariant |
| `tests_visual_depth.rs` | `depth_lab_glyph_rain_not_dense_wall` | Glyph density bound |
| `tests_visual_depth.rs` | `depth_lab_charset_transition_no_background_flood` | Transition cleanliness |
| `tests_visual_depth.rs` | `depth_lab_color_transition_no_stale_residue_at_frame_level` | Color transition guard |
| `tests_visual_depth.rs` | `depth_lab_brightness_level_four_tier_hierarchy` | Brightness hierarchy |
| `tests_visual_depth.rs` | `depth_lab_sustained_rain_bottom_residue_bounded_300_frames` | Bottom residue bound |
| `tests_visual_depth.rs` | `depth_lab_no_muddy_residue_on_dark_backgrounds` | Anti-muddy guard |
| `tests_monolith.rs` | `depth_lab_monolith_sparse_lane_density_bounded_per_column` | Per-column density |
| `tests_monolith.rs` | `depth_lab_monolith_empty_space_ratio_above_min_threshold` | Empty space ratio |
| `tests_monolith.rs` | `depth_lab_monolith_no_full_height_continuous_wall` | No continuous wall |
| `tests_monolith.rs` | `depth_lab_monolith_bottom_residue_bounded_extended_rain` | Extended bottom bound |
| `tests_scene.rs` | `depth_lab_scene_switch_monolith_to_matrix_clears_phosphor` | Scene switch clean |
| `tests_scene.rs` | `depth_lab_scene_switch_monolith_to_signal_clears_drawn_cells` | Scene switch clean |
| `tests_scene.rs` | `depth_lab_scene_switch_glyph_to_monolith_renders_clean` | Scene switch clean |
| `tests_scene.rs` | `depth_lab_repeated_cycle_never_accumulates_residue` | Repeated cycle guard |

### Diagnostics Tests (`src/docs_tests/zactrix.rs`)

| Test | Guard |
|------|-------|
| `depth_lab_benchmark_actual_execution_is_single_threaded` | Single-threaded invariant |
| `depth_lab_benchmark_terminal_writer_is_single_owner` | Single-owner invariant |
| `depth_lab_benchmark_compute_parallelism_remains_disabled` | No parallel compute |
| `depth_lab_benchmark_render_plan_remains_single_owner` | Single-owner render |
| `depth_lab_no_active_parallel_compute_claimed` | No active claims |
| `depth_lab_info_output_zactrix_system_honest` | Honest diagnostics |
| `depth_lab_visual_stability_doc_exists` | Doc guard |
| `depth_lab_visual_stability_doc_mentions_zactrix_guard` | Zactrix guard doc |

## Existing Guard Tests (Pre-Phase 3)

These existing tests are NOT part of the Phase 3 Depth Regression Lab but provide complementary coverage:

- `monolith_does_not_draw_full_height_continuous_spine` — Spine column guard
- `monolith_rain_is_sparse_compared_to_dense_glyph_rain` — Sparse density guard
- `monolith_bottom_residue_stays_bounded` — Bottom residue bound
- `hero_spine_trail_empty_space_have_distinct_brightness` — Brightness hierarchy
- `monolith_color_for_level_ghost_is_faintest` — Ghost level guard
- `monolith_background_muddy_residue_guard` — Anti-muddy guard
- `fixed_color_sun_stays_sun_across_simulated_minutes` — Color stickiness
- `auto_color_drift_is_opt_in_only` — Drift opt-in guard
- `high_speed_does_not_create_unbounded_bottom_accumulation` — Bottom accumulation
- `stale_bottom_cells_decay_to_blank_within_bounded_time` — Phosphor decay
- `clean_exit_frame_has_no_persistent_ghost_in_bottom_rows` — Clean exit guard
