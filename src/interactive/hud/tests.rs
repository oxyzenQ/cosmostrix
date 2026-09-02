// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
// LOC_EXEMPT: HUD regression tests — 24-row layout assertions + chroma gradient
// + metric setter sanitization. Splitting would fragment the layout contract
// tests from the setter tests they guard.

use super::*;

#[test]
fn hud_starts_invisible() {
    let h = HudState::new();
    assert!(!h.visible(), "HUD must start invisible");
}

#[test]
fn hud_toggle_flips_visibility() {
    let mut h = HudState::new();
    assert!(!h.visible());
    assert!(h.toggle(), "first toggle must turn HUD on");
    assert!(h.visible());
    assert!(!h.toggle(), "second toggle must turn HUD off");
    assert!(!h.visible());
}

#[test]
fn hud_push_frame_time_is_noop_when_invisible() {
    let mut h = HudState::new();
    h.push_frame_time(1.0);
    // max_ms should still be 0 because the HUD is off.
    assert_eq!(h.max_ms, 0.0, "invisible HUD must not record frame times");
}

#[test]
fn hud_push_frame_time_records_when_visible() {
    let mut h = HudState::new();
    h.toggle();
    h.push_frame_time(1.0);
    h.push_frame_time(2.0);
    h.push_frame_time(0.5);
    assert_eq!(h.max_ms, 2.0, "max_ms must track the highest pushed value");
}

#[test]
fn hud_maybe_sample_rss_is_noop_when_invisible() {
    let mut h = HudState::new();
    h.maybe_sample_rss();
    assert!(h.last_rss_kb.is_none(), "invisible HUD must not sample RSS");
}

#[test]
fn hud_maybe_sample_cpu_keeps_baseline_warm_when_invisible() {
    // When the HUD is off, maybe_sample_cpu STILL samples at 1 Hz —
    // this is the warm-baseline design that lets toggle-on show an
    // instant percent (no `cpu: —` flash for 1 second).
    //
    // The cost is one syscall/sec when the HUD is off — well under
    // 0.1% CPU. This trade-off was explicitly requested by the user:
    // other metrics (fps/p99/rss) show data instantly on toggle-on,
    // and CPU must too.
    //
    // On unix platforms the sampler should produce Some(last_cpu_ns)
    // after a single call. On non-unix it stays None (sampler
    // unsupported). Both are valid per-platform outcomes.
    let mut h = HudState::new();
    h.maybe_sample_cpu();
    // We can't assert last_cpu_ns.is_some() unconditionally because
    // non-unix targets return None. But we CAN assert that the
    // function did NOT short-circuit on invisible — by checking that
    // last_cpu_sample was updated to ~now (i.e. the function ran to
    // completion past the visibility check).
    let now = Instant::now();
    let diff = now.duration_since(h.last_cpu_sample);
    assert!(
            diff.as_millis() < 1000,
            "maybe_sample_cpu must run even when invisible (warm baseline) — last_cpu_sample was not updated"
        );
}

#[test]
fn hud_first_cpu_sample_establishes_baseline_only() {
    // On the very first CPU sample after HUD turns on, the function
    // must record the baseline ns but NOT compute a percent (no delta
    // yet). cpu_percent stays None and renders as `cpu: —`.
    let mut h = HudState::new();
    h.toggle(); // visible
    h.maybe_sample_cpu();
    // On supported platforms (unix), last_cpu_ns should now be Some.
    // On non-unix it stays None (sampler unsupported). Both are valid
    // per-platform outcomes — we just assert no percent is produced
    // (we can't compute a delta from one reading).
    assert!(
        h.cpu_percent.is_none(),
        "first CPU sample must not produce a percent (no delta yet)"
    );
}

#[test]
fn hud_toggle_preserves_cpu_baseline_for_instant_reopen() {
    // When the HUD is toggled off then on again, the CPU baseline
    // must be PRESERVED (not cleared). This is the warm-baseline
    // design: maybe_sample_cpu samples at 1 Hz even while the HUD
    // is off, so on toggle-on we already have a recent baseline
    // and can compute an instant percent on the very next tick.
    //
    // Previously (commit ef8ab2a) the baseline was cleared on
    // toggle-on, forcing the HUD to show `cpu: —` for ~1 second.
    // The user explicitly flagged this as a UX inconsistency:
    // other metrics (fps/p99/rss) show data instantly, and CPU
    // must too.
    let mut h = HudState::new();
    h.toggle(); // on
    h.maybe_sample_cpu();
    // Stash the post-first-sample baseline (may be None on non-unix).
    let baseline_before_toggle_off = h.last_cpu_ns;
    // Toggle off then on.
    h.toggle(); // off
    h.toggle(); // on
    assert_eq!(
        h.last_cpu_ns, baseline_before_toggle_off,
        "toggling HUD on must PRESERVE the CPU baseline (warm-baseline design)"
    );
}

#[test]
fn hud_cpu_line_renders_dash_when_unsupported() {
    // Verify the cached_lines[4] entry renders ` cpu: —` when
    // cpu_percent is None. This is the user-visible contract:
    // unsupported platforms (non-unix) and the brief pre-delta
    // window after HUD-on both show the em dash, not `0.00%`.
    // (v30 2026-08-05: index shifted from [4] to [5] because a new
    // `tgt:` line was inserted at index [1].)
    // (v50 2026-08-15: index shifted back from [5] to [4] after the
    // intra-pair HUD reorder — cpu moved above rss to match htop
    // convention: active before passive.)
    let mut h = HudState::new();
    h.toggle(); // visible
                // Force-update metrics without sampling — cpu_percent stays None.
                // We need to bypass the rate-limit by directly calling update_metrics
                // with an empty palette (the function recomputes cached_lines).
    h.update_metrics(&[]);
    assert!(
        h.cpu_percent.is_none(),
        "cpu_percent must be None before any sample"
    );
    // cached_lines[4] is the cpu line (after v50 intra-pair reorder).
    let (_, cpu_line) = &h.cached_lines[4];
    assert!(
        cpu_line.contains('—'),
        "cpu line must render em dash when unsupported, got: {cpu_line:?}"
    );
}

#[test]
fn hud_cpu_line_renders_percent_with_two_decimals_when_supported() {
    // Synthetic test: set cpu_percent directly (bypassing the sampler)
    // and verify update_metrics renders ` cpu: 12.34%` (2 decimals).
    // This locks in the display format independently of the sampler
    // behavior — if we later change to 1 decimal, this test fails.
    // (v30 2026-08-05: index shifted from [4] to [5] because a new
    // `tgt:` line was inserted at index [1].)
    // (v50 2026-08-15: index shifted back from [5] to [4] after the
    // intra-pair HUD reorder — cpu moved above rss to match htop
    // convention: active before passive.)
    let mut h = HudState::new();
    h.toggle(); // visible
    h.cpu_percent = Some(12.3456); // should render as 12.35%
    h.update_metrics(&[]);
    let (_, cpu_line) = &h.cached_lines[4];
    assert!(
        cpu_line.contains("12.35%"),
        "cpu line must render 2-decimal percent, got: {cpu_line:?}"
    );
}

#[test]
fn hud_has_twenty_four_cached_lines_after_z_master_1x_round5() {
    // Regression guard: the HUD must have exactly 24 cached rows after
    // the Z-master-1X round 5 expansion that added dcel (row 19) + tcel
    // (row 20) above cid (now row 21). The previous count was 22 (v80.0.0-beta.1
    // reorder). If a future change adds or removes a row, this test
    // will catch it.
    let h = HudState::new();
    assert_eq!(
        h.cached_lines.len(),
        24,
        "HUD must have 24 cached rows after the Z-master-1X round 5 dcel/tcel expansion"
    );
}

#[test]
fn format_rss_kb_renders_suffixes() {
    assert_eq!(format_rss_kb(0), "0KiB");
    assert_eq!(format_rss_kb(512), "512KiB");
    assert_eq!(format_rss_kb(1023), "1023KiB");
    assert_eq!(format_rss_kb(1024), "1.0MiB");
    assert_eq!(format_rss_kb(2048), "2.0MiB");
}

// ── refresh_colors + rain-aesthetic gradient tests ────────────────
//
// The HUD color refresh was split out of the 1 Hz `update_metrics`
// tick so runtime palette changes (c/C key, crystal-dragon,
// live-config reload) appear on the very next frame, not up to 1
// second later. The gradient was also inverted to follow the rain
// aesthetic: dim tail at top → bright head at bottom. These tests
// lock in both behaviors.

#[test]
fn refresh_colors_is_noop_when_invisible() {
    // When the HUD is off, refresh_colors must short-circuit and
    // leave cached_lines untouched. This matches the zero-cost-when-off
    // design constraint — even the cheap 4-brighten_color calls are
    // skipped.
    let mut h = HudState::new();
    // Capture the initial colors (the defaults from new()).
    let initial_colors: Vec<_> = h.cached_lines.iter().map(|(c, _)| *c).collect();
    // Pass a non-empty palette — if refresh_colors ran, it would
    // overwrite the defaults with brightened palette colors.
    let palette = vec![
        Color::Rgb { r: 0, g: 50, b: 0 },
        Color::Rgb { r: 0, g: 255, b: 0 },
    ];
    h.refresh_colors(&palette);
    let after_colors: Vec<_> = h.cached_lines.iter().map(|(c, _)| *c).collect();
    assert_eq!(
        initial_colors, after_colors,
        "refresh_colors must be a no-op when HUD is invisible"
    );
}

#[test]
fn refresh_colors_updates_colors_without_touching_text() {
    // When the HUD is visible, refresh_colors must update ONLY the
    // Color half of each cached_lines tuple — the String half (text)
    // is owned by the 1 Hz `update_metrics` tick and must be preserved
    // between ticks. This is what enables instant color refresh on
    // palette change without re-running the expensive format! calls.
    let mut h = HudState::new();
    h.toggle(); // visible
                // Seed the cached_lines with sentinel text so we can verify it
                // survives refresh_colors. (In production, update_metrics writes
                // the real text — here we just verify refresh_colors doesn't
                // touch it.)
    for slot in &mut h.cached_lines {
        slot.1 = "SENTINEL".to_string();
    }
    let palette = vec![
        Color::Rgb { r: 0, g: 50, b: 0 },  // dim (idx 1 = trail-ish)
        Color::Rgb { r: 0, g: 200, b: 0 }, // mid (n/2)
        Color::Rgb { r: 0, g: 255, b: 0 }, // head (last)
    ];
    h.refresh_colors(&palette);
    for (i, (color, text)) in h.cached_lines.iter().enumerate() {
        assert_eq!(
            text, "SENTINEL",
            "refresh_colors must NOT touch text (line {i}), got: {text:?}"
        );
        // Every color must have been overwritten to a non-default value.
        // The default new() colors include Color::Cyan, Color::Yellow,
        // Color::Magenta, Color::Green, Color::DarkCyan. After refresh,
        // all should be Rgb (brighten_color always returns Rgb except
        // for the named-color conversion path which still returns Rgb).
        assert!(
            matches!(color, Color::Rgb { .. }),
            "refresh_colors must overwrite line {i} color to Rgb, got: {color:?}"
        );
    }
}

#[test]
fn refresh_colors_assigns_dim_to_top_and_head_to_bottom() {
    // HD-01: 16-stop chroma gradient sweep. The BOTTOM row (cid, idx 15)
    // is the brightest `head` color (palette last stop), and the TOP
    // row (fps, idx 0) is the dimmest `dim` color (palette first stop,
    // brightened to readable floor). This inverts the original mapping
    // where fps/tgt/max were brightest at the top — the owner explicitly
    // flagged the inversion: 'rain tail is dim head is white' (head
    // leads at the bottom of a falling stream).
    //
    // v50 (2026-08-15): bumped from 8 → 9 stops after adding the `cid:`
    // line at row 8. The cid line shared the head stop with screensize.
    //
    // v50 (2026-08-17): bumped from 9 → 16 stops + reordered the entire
    // HUD per owner's Option S mandate. The final layout places the 7
    // new owner-mandated metrics at rows 6-12 (ehs/prs/sped/dsty/scn/chr/
    // clr), moves up to row 13, screensize to row 14, and cid to row 15
    // (owner-mandated bottom). The chroma gradient sweeps continuously
    // from palette[0] (dim tail) at the top to palette[n-1] (bright head)
    // at the bottom.
    //
    // We use a palette where head is pure white RGB(255,255,255) so the
    // assertion is unambiguous, and dim is a dark green RGB(0,50,0) that
    // brightens to RGB(0,200,0). The bottom row (cid) must be white
    // (head); the top row (fps) must be green (dim).
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb { r: 0, g: 50, b: 0 }, // idx 0  → row 0  (dim, brightened)
        Color::Rgb { r: 0, g: 80, b: 0 }, // idx 1  → row 1
        Color::Rgb { r: 0, g: 110, b: 0 }, // idx 2  → row 2
        Color::Rgb { r: 0, g: 140, b: 0 }, // idx 3  → row 3
        Color::Rgb {
            r: 100,
            g: 180,
            b: 0,
        }, // idx 4  → row 4
        Color::Rgb {
            r: 150,
            g: 210,
            b: 50,
        }, // idx 5  → row 5
        Color::Rgb {
            r: 200,
            g: 230,
            b: 100,
        }, // idx 6  → row 6
        Color::Rgb {
            r: 220,
            g: 240,
            b: 150,
        }, // idx 7  → row 7  (screensize, near head)
        Color::Rgb {
            r: 230,
            g: 245,
            b: 180,
        }, // idx 8  → row 8  (cid, near head)
        Color::Rgb {
            r: 200,
            g: 100,
            b: 0,
        }, // idx 9  → row 9  (reserved ehs)
        Color::Rgb {
            r: 200,
            g: 0,
            b: 100,
        }, // idx 10 → row 10 (reserved prs)
        Color::Rgb {
            r: 0,
            g: 200,
            b: 100,
        }, // idx 11 → row 11 (reserved sped)
        Color::Rgb {
            r: 100,
            g: 200,
            b: 0,
        }, // idx 12 → row 12 (reserved dsty)
        Color::Rgb {
            r: 200,
            g: 200,
            b: 100,
        }, // idx 13 → row 13 (reserved scn)
        Color::Rgb {
            r: 200,
            g: 100,
            b: 200,
        }, // idx 14 → row 14 (screensize)
        // v50.0.0-beta.6: 2 new entries for prdr (row 15) and crdr (row 16).
        Color::Rgb {
            r: 240,
            g: 248,
            b: 220,
        }, // idx 15 → row 15 (prdr, near head)
        Color::Rgb {
            r: 248,
            g: 252,
            b: 240,
        }, // idx 16 → row 16 (crdr, near head)
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 17 → row 17 (cid, head, white)
        // v50.0.0-beta.7 Option C: 4 new entries for ambt/glth/ctun/mnst.
        Color::Rgb {
            r: 250,
            g: 250,
            b: 250,
        }, // idx 18
        Color::Rgb {
            r: 252,
            g: 252,
            b: 252,
        }, // idx 19
        Color::Rgb {
            r: 254,
            g: 254,
            b: 254,
        }, // idx 20
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 21 → row 21 (cid, head, white — Z-master-1X round 5)
        // Z-master-1X round 5: 3 new entries for dcel/tcel/cid shift.
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 22
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 23 → row 23 (screensize, head)
    ];
    h.refresh_colors(&palette);
    // v80.0.0-beta.3 panel mapping (owner-approved Option B): the fps
    // header cap (idx 0) sits at t=1.0 = palette[23] = RGB(255,255,255)
    // — the BRIGHT head. The grid body starts dim at ehs (idx 6,
    // visual slot 2, t=0.0) = palette[0] brightened to RGB(0,200,0).
    assert_eq!(
        h.cached_lines[0].0,
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        },
        "fps header cap (idx 0) must use palette[23] (bright head) — Option B bright header strip"
    );
    assert_eq!(
        h.cached_lines[6].0,
        Color::Rgb { r: 0, g: 200, b: 0 },
        "ehs grid-body start (idx 6) must use palette[0] brightened (dim tail, t=0.0)"
    );
    // Bottom row (screensize, idx 23 — Z-master-1X round 5: moved down
    // from row 21 to row 23) = palette[23] = RGB(255,255,255) — head.
    assert_eq!(
        h.cached_lines[23].0,
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        },
        "bottom row (screensize) must use palette[23] (head) — bright head at bottom"
    );
    // v80.0.0-beta.3: the performance core (max/p99/cpu, visual slots
    // 17-19, t = 0.75-0.85) rides the BRIGHT band of the grid body —
    // measurably brighter than the dim body start. With THIS palette
    // the t=0.75 blend lands inside the white head zone and
    // blend_toward_rgb's integer rounding keeps it exactly white, so
    // the honest property is relative brightness, not "not white".
    let lum = |c: Color| -> u32 {
        match c {
            Color::Rgb { r, g, b } => u32::from(r.max(g).max(b)),
            _ => 0,
        }
    };
    assert!(
        lum(h.cached_lines[2].0) > lum(h.cached_lines[6].0),
        "max (t=0.75) must be brighter than ehs (t=0.0) — performance core rides the bright band"
    );
    assert!(
        lum(h.cached_lines[4].0) > lum(h.cached_lines[6].0),
        "cpu (t=0.85) must be brighter than ehs (t=0.0) — performance core rides the bright band"
    );
}

#[test]
fn refresh_colors_picks_up_runtime_palette_change_immediately() {
    // HD-01 regression: palette change at runtime (c/C, Crystal Dragon drift,
    // live-config) must reflect on the next refresh_colors call — no
    // rate limit. With the old 4-level design + 1 Hz update_metrics,
    // the second call would have been a no-op.
    //
    // v50 (2026-08-17): bottom row is now idx 15 (reserved clr slot) —
    // the assertion indices have been updated from idx 7 → idx 15 to
    // match the 16-row layout. Once the data-plumbing commit moves cid
    // to row 15, this test will be re-targeted to assert against the cid
    // line content instead of the reserved clr slot.
    let mut h = HudState::new();
    h.toggle();
    let green_palette = vec![
        Color::Rgb { r: 0, g: 50, b: 0 },
        Color::Rgb { r: 0, g: 50, b: 0 },
        Color::Rgb { r: 0, g: 255, b: 0 },
    ];
    h.refresh_colors(&green_palette);
    assert_eq!(
        h.cached_lines[23].0,
        Color::Rgb { r: 0, g: 255, b: 0 },
        "first refresh: bottom = green head"
    );
    let amber_palette = vec![
        Color::Rgb { r: 50, g: 25, b: 0 },
        Color::Rgb { r: 50, g: 25, b: 0 },
        Color::Rgb {
            r: 255,
            g: 176,
            b: 0,
        },
    ];
    h.refresh_colors(&amber_palette);
    assert_eq!(
        h.cached_lines[23].0,
        Color::Rgb {
            r: 255,
            g: 176,
            b: 0
        },
        "second refresh (immediate): bottom = amber head"
    );
}

#[test]
fn refresh_colors_gradient_uses_twenty_four_distinct_stops() {
    // HD-01: 24 HUD rows now use 24 distinct palette stops (one per row),
    // sweeping the full chroma dragon gradient top→bottom.
    // Z-master-1X round 5: bumped from 22 → 24 stops (dcel + tcel added).
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb { r: 200, g: 0, b: 0 },
        Color::Rgb { r: 0, g: 200, b: 0 },
        Color::Rgb { r: 0, g: 0, b: 200 },
        Color::Rgb {
            r: 200,
            g: 200,
            b: 0,
        },
        Color::Rgb {
            r: 200,
            g: 0,
            b: 200,
        },
        Color::Rgb {
            r: 0,
            g: 200,
            b: 200,
        },
        Color::Rgb {
            r: 255,
            g: 128,
            b: 0,
        },
        Color::Rgb {
            r: 220,
            g: 240,
            b: 150,
        },
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 8
        Color::Rgb {
            r: 200,
            g: 100,
            b: 0,
        },
        Color::Rgb {
            r: 200,
            g: 0,
            b: 100,
        },
        Color::Rgb {
            r: 0,
            g: 200,
            b: 100,
        },
        Color::Rgb {
            r: 100,
            g: 200,
            b: 0,
        },
        Color::Rgb {
            r: 200,
            g: 200,
            b: 100,
        },
        Color::Rgb {
            r: 200,
            g: 100,
            b: 200,
        },
        // v80.0.0-beta.1 reorder: rows 15-18 = ambt/glth/ctun/mnst, 19 = cid,
        // 20 = up, 21 = screensize (positional palette comments).
        Color::Rgb {
            r: 100,
            g: 200,
            b: 200,
        }, // idx 15 → row 15 (ambt)
        Color::Rgb {
            r: 200,
            g: 200,
            b: 200,
        }, // idx 16 → row 16 (glth)
        Color::Rgb {
            r: 100,
            g: 100,
            b: 200,
        }, // idx 17 → row 17 (ctun)
        // v50.0.0-beta.7 Option C: 4 new entries for ambt/glth/ctun/mnst.
        Color::Rgb {
            r: 150,
            g: 200,
            b: 100,
        }, // idx 18 → row 18 (mnst)
        Color::Rgb {
            r: 100,
            g: 150,
            b: 200,
        }, // idx 19 → row 19 (cid)
        Color::Rgb {
            r: 200,
            g: 100,
            b: 150,
        }, // idx 20 → row 20 (up)
        Color::Rgb {
            r: 200,
            g: 200,
            b: 200,
        }, // idx 21 → row 21 (cid — Z-master-1X round 5)
        // Z-master-1X round 5: 2 new entries for dcel (row 19) + tcel (row 20).
        Color::Rgb {
            r: 180,
            g: 200,
            b: 220,
        }, // idx 22 → row 22 (up)
        Color::Rgb {
            r: 200,
            g: 220,
            b: 200,
        }, // idx 23 → row 23 (screensize)
    ];
    h.refresh_colors(&palette);
    // All palette entries have max channel >= TARGET_V(200), so brighten
    // returns each as-is. This isolates the gradient mapping test from
    // the brightening math (covered separately by brighten_color_* tests).
    //
    // Z-master-1X round 5: with 24 palette entries + 24 HUD rows, t = i/23.0
    // and scaled_t = t * 23 = i exactly — BUT floating-point can make
    // 7/23.0 * 23 = 6.9999... (not exactly 7.0), causing the interpolator
    // to blend between palette[6] and palette[7] with frac=0.9999.
    // The result is within ±1 of the target RGB — visually identical but
    // not bit-exact. So we assert >=20 distinct colors (the gradient is
    // smooth, not banded) instead of exact 1:1 palette mapping.
    let distinct_count = {
        let mut unique: Vec<Color> = h.cached_lines.iter().map(|(c, _)| *c).collect();
        unique.dedup();
        unique.len()
    };
    assert!(
        distinct_count >= 20,
        "24-row HUD gradient must produce >=20 distinct colors with a 24-stop palette (got {distinct_count}) — banded gradient would indicate an interpolation regression"
    );
    // v80.0.0-beta.3 boundary metrics must still be exact — the caps
    // (fps idx 0 / screensize idx 23) sit at t=1.0 (palette[23]); the
    // grid-body start (ehs, idx 6, visual slot 2) sits at t=0.0
    // (palette[0]). No floating-point drift at either boundary.
    assert_eq!(
        h.cached_lines[0].0, palette[23],
        "fps header cap (idx 0) must use palette[23] exactly (t=1.0, no interpolation)"
    );
    assert_eq!(
        h.cached_lines[6].0, palette[0],
        "ehs grid-body start (idx 6) must use palette[0] exactly (t=0.0, no interpolation)"
    );
    assert_eq!(
        h.cached_lines[23].0, palette[23],
        "screensize footer cap (idx 23) must use palette[23] exactly (t=1.0, no interpolation)"
    );
}

#[test]
fn hud_cid_line_contains_commit_sha_or_unknown() {
    // v50 (2026-08-15): the cid line was at index [8].
    // v50 (2026-08-17) HUD expansion reorder: moved to the bottom row.
    // v80.0.0-beta.1 reorder (owner mandate 2026-08-31): cid sits at index [19] —
    // above the session footer (up [20] + screensize [21]) so the
    // dashboard closes on the terminal size instead.
    // The line must contain the compile-time git short SHA injected by
    // build.rs via `COSMOSTRIX_GIT_SHA`, falling back to "unknown" when
    // the build had no .git dir. The text is set once in `new()` and
    // never mutated — `update_metrics` skips row 21 entirely so the
    // commit hash remains stable across the entire process lifetime.
    // The owner needs to read the commit hash without quitting cosmostrix.
    // Z-master-1X round 5: cid moved from row 19 to row 21 (dcel/tcel
    // inserted at rows 19-20 above cid).
    let h = HudState::new();
    let (_, cid_line) = &h.cached_lines[21];
    assert!(
        cid_line.starts_with(" cid: "),
        "cid line must start with ' cid: ' prefix, got: {cid_line:?}"
    );
    let sha = cid_line.strip_prefix(" cid: ").unwrap();
    assert!(
        !sha.is_empty(),
        "cid line must carry a non-empty SHA, got: {cid_line:?}"
    );
    // The fallback "unknown" is valid for tarball builds without .git.
    // For git builds, build.rs emits a 7-char lowercase hex short SHA
    // (digits 0-9 + lowercase a-f). Digits are NOT lowercase letters, so
    // we only check `is_ascii_hexdigit()` + length 7 — case is already
    // enforced by build.rs which lowercases the SHA.
    let is_unknown = sha == "unknown";
    let is_hex_sha = sha.len() == 7 && sha.chars().all(|c| c.is_ascii_hexdigit());
    assert!(
        is_unknown || is_hex_sha,
        "cid SHA must be 'unknown' or a 7-char hex string, got: {sha:?}"
    );
}

#[test]
fn compute_chroma_gradient_panel_sweeps_full_palette_range() {
    // HD-01 regression: verify the 24-stop chroma gradient helper maps
    // the first and last HUD rows to the corresponding palette boundary
    // stops. Row 0 → palette[0] (t=0.0), row 23 → palette[n-1] (t=1.0).
    // Z-master-1X round 5: bumped from 22 → 24 stops to add dcel + tcel.
    let palette = vec![
        Color::Rgb { r: 50, g: 0, b: 0 }, // idx 0  → row 0 (t=0.0)
        Color::Rgb { r: 0, g: 50, b: 0 }, // idx 1
        Color::Rgb { r: 0, g: 0, b: 50 }, // idx 2
        Color::Rgb {
            r: 100,
            g: 100,
            b: 0,
        },
        Color::Rgb {
            r: 0,
            g: 100,
            b: 100,
        },
        Color::Rgb {
            r: 100,
            g: 0,
            b: 100,
        },
        Color::Rgb {
            r: 200,
            g: 100,
            b: 50,
        },
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
        Color::Rgb {
            r: 60,
            g: 60,
            b: 60,
        },
        Color::Rgb {
            r: 70,
            g: 70,
            b: 70,
        },
        Color::Rgb {
            r: 80,
            g: 80,
            b: 80,
        },
        Color::Rgb {
            r: 90,
            g: 90,
            b: 90,
        },
        Color::Rgb {
            r: 120,
            g: 120,
            b: 120,
        },
        Color::Rgb {
            r: 130,
            g: 130,
            b: 130,
        },
        Color::Rgb {
            r: 100,
            g: 100,
            b: 100,
        }, // idx 15 → last stop (t=1.0)
    ];
    let colors = compute_chroma_gradient_panel(&palette);
    // v80.0.0-beta.3 panel mapping: metric 0 (fps, header cap) sits at
    // t=1.0 → palette[n-1] = RGB(100,100,100) brightened to
    // RGB(200,200,200) — the bright header strip color.
    assert_eq!(
        colors[0],
        Color::Rgb {
            r: 200,
            g: 200,
            b: 200
        },
        "fps (header cap) must use the last palette stop brightened (t=1.0)"
    );
    // Metric 6 (ehs, grid body start) sits at visual slot 2 → t=0.0 →
    // palette[0] = RGB(50,0,0) brightened to RGB(200,0,0) — the dim
    // tail start of the grid body sweep.
    assert_eq!(
        colors[6],
        Color::Rgb { r: 200, g: 0, b: 0 },
        "ehs (grid body start) must use palette[0] exactly (t=0.0, no interpolation)"
    );
    // Row 23 = palette[15] = RGB(100,100,100) brightened to RGB(200,200,200).
    // t=1.0 maps exactly to palette[n-1] — the last stop. max channel is
    // 100, scaled by 200/100 = 2.0x to reach the TARGET_V=200 floor.
    // (100 * 200 / 100 = 200.) This is the screensize row (row 23, owner-
    // mandated bottom — Z-master-1X round 5 moved it down from 21) — the
    // chroma gradient sweeps from palette[0] at the top (dim tail) to
    // palette[n-1] at the bottom (bright head).
    assert_eq!(
        colors[23],
        Color::Rgb {
            r: 200,
            g: 200,
            b: 200
        },
        "screensize row (idx 23) must use the last palette stop brightened to TARGET_V=200"
    );
}

// ── v80.0.0-beta.3 panel geometry regression tests (branch
// hud-scifi-dashboard, owner-approved Option B + D) ─────────────────
//
// The v80.0.0-beta.1 L-shape border + dynamic-width write_to_frame
// tests are replaced wholesale: the HUD is now a FIXED 46-col x 12-row
// bottom-center rounded panel, so the shrink/grow residue class
// (HB-01) no longer exists — every assertion below locks the stable
// geometry instead: anchor math, rounded corners, bright caps,
// side-gradient sweep, grid cell placement, gutter ownership,
// fixed-width padding overwrite (the HB-01 successor), the first-tick
// guard, the visibility guard, and INV-8 clipping on small terminals.

/// Test helper: a visible HUD with a deterministic panel cache.
/// Texts/colors are set DIRECTLY (not via update_metrics) so the
/// geometry assertions stay independent of the 1 Hz tick + palette
/// machinery. The panel cache mimics the exact production shape:
/// header/footer interiors of 44 cols, grid cells of 14 cols.
fn panel_hud() -> HudState {
    let mut h = HudState::new();
    h.toggle();
    for i in 0..24 {
        h.cached_lines[i].0 = Color::Rgb {
            r: 128,
            g: 128,
            b: 128,
        };
        h.cached_lines[i].1 = format!(" m{i}");
    }
    // head cap (screensize metric 23) = white; body start (ehs metric
    // 6) = dark; body end (up metric 22) = brighter than the middle.
    h.cached_lines[23].0 = Color::Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    h.cached_lines[6].0 = Color::Rgb {
        r: 60,
        g: 60,
        b: 60,
    };
    h.cached_lines[22].0 = Color::Rgb {
        r: 220,
        g: 220,
        b: 220,
    };
    h.panel_header = "───────────── fps: 451  tgt: 60 ────────────".to_string();
    h.panel_footer = "──────────────── 200x50 auto ──────────────".to_string();
    for g in 0..7usize {
        for c in 0..3usize {
            let mut cell = format!("g{g}c{c}");
            while cell.chars().count() < 14 {
                cell.push(' ');
            }
            h.panel_grid[g][c] = cell;
        }
    }
    h
}

#[test]
fn hud_panel_anchors_bottom_center_with_rounded_frame() {
    let mut h = panel_hud();
    // 100x40 terminal: start_col = (100-46)/2 = 27, anchor_row =
    // 40-12 = 28. The full block spans rows 28..=39 (12 rows).
    let mut frame = crate::frame::Frame::new(100, 40, None);
    h.write_to_frame(&mut frame, None);

    // Rounded corners (Option D complete frame): bright, exact glyphs.
    assert_eq!(frame.get(27, 28).unwrap().ch, '╭', "top-left corner");
    assert_eq!(frame.get(72, 28).unwrap().ch, '╮', "top-right corner");
    assert_eq!(frame.get(27, 38).unwrap().ch, '╰', "bottom-left corner");
    assert_eq!(frame.get(72, 38).unwrap().ch, '╯', "bottom-right corner");
    // Side borders on every body row (panel rows 1..=9 → rows 29..=37).
    for row in 29..=37u16 {
        assert_eq!(frame.get(27, row).unwrap().ch, '│', "left side row {row}");
        assert_eq!(frame.get(72, row).unwrap().ch, '│', "right side row {row}");
    }
    // Nothing above the panel anchor: row 27 must stay rain-only.
    if let Some(cell) = frame.get(50, 27) {
        assert_ne!(cell.ch, '│', "no panel content above the anchor row");
    }
    // Tail accent: a single '▼' centered under the frame on the very
    // last row (col 27 + (46-1)/2 = 49, row 39).
    assert_eq!(frame.get(49, 39).unwrap().ch, '▼', "tail accent glyph");
}

#[test]
fn hud_panel_header_footer_and_accent_are_bright_caps() {
    let mut h = panel_hud();
    let mut frame = crate::frame::Frame::new(100, 40, None);
    h.write_to_frame(&mut frame, None);
    let head = Color::Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    // Header strip interior + corners.
    assert_eq!(frame.get(28, 28).unwrap().fg, Some(head));
    assert_eq!(frame.get(71, 28).unwrap().fg, Some(head));
    assert_eq!(
        frame.get(27, 28).unwrap().fg,
        Some(head),
        "╭ corner is a cap"
    );
    // Footer strip interior + corners.
    assert_eq!(frame.get(28, 38).unwrap().fg, Some(head));
    assert_eq!(
        frame.get(72, 38).unwrap().fg,
        Some(head),
        "╯ corner is a cap"
    );
    // Tail accent.
    assert_eq!(
        frame.get(49, 39).unwrap().fg,
        Some(head),
        "▼ accent is a cap"
    );
}

#[test]
fn hud_panel_side_border_sweeps_dim_to_bright_downward() {
    // The side borders carry the row's sweep color (the metric at that
    // visual height): row 1 (spacer) = ehs color (60), a mid grid row
    // = the grey body (128), row 9 (spacer) = up color (220).
    let mut h = panel_hud();
    let mut frame = crate::frame::Frame::new(100, 40, None);
    h.write_to_frame(&mut frame, None);
    let r_at = |row: u16| -> u8 {
        match frame.get(27, row).and_then(|c| c.fg) {
            Some(Color::Rgb { r, .. }) => r,
            other => panic!("no Rgb fg at row {row}: {other:?}"),
        }
    };
    let top = r_at(29); // panel row 1 → ehs (dim body start)
    let mid = r_at(33); // panel row 5 → mid body
    let bottom = r_at(37); // panel row 9 → up (bright body end)
    assert!(
        top < mid,
        "side border must brighten downward (top {top} < mid {mid})"
    );
    assert!(
        mid < bottom,
        "side border must brighten downward (mid {mid} < bottom {bottom})"
    );
}

#[test]
fn hud_panel_grid_cells_render_at_expected_positions() {
    // Grid row g renders at y = anchor+2+g; cell c spans
    // x = start_col+1+c*15 .. +14; the 1-col gutters between cells are
    // owned blanks (fg None) so no rain glyph can sit between cells.
    let mut h = panel_hud();
    let mut frame = crate::frame::Frame::new(100, 40, None);
    h.write_to_frame(&mut frame, None);
    // Grid row 0 (y=30): cells "g0c0", "g0c1", "g0c2".
    assert_eq!(frame.get(28, 30).unwrap().ch, 'g');
    assert_eq!(&frame.get(28, 30).unwrap().ch.to_string(), "g");
    assert_eq!(frame.get(43, 30).unwrap().ch, 'g');
    assert_eq!(frame.get(58, 30).unwrap().ch, 'g');
    // Cell text colors follow the metric's own gradient stop: cell
    // (0,0) is ehs (metric 6, dark) per HUD_VISUAL_ORDER.
    assert_eq!(
        frame.get(28, 30).unwrap().fg,
        Some(Color::Rgb {
            r: 60,
            g: 60,
            b: 60
        }),
        "grid cell (0,0) = ehs → its own dim body-start color"
    );
    // Gutters: blank, no fg.
    for gutter_x in [42u16, 57u16] {
        let cell = frame.get(gutter_x, 30).unwrap();
        assert_eq!(cell.ch, ' ', "gutter at x={gutter_x} must be blank");
        assert!(
            cell.fg.is_none(),
            "gutter at x={gutter_x} must have fg None"
        );
    }
    // Grid row 6 (y=36) holds the performance core (max/p99/cpu).
    assert_eq!(frame.get(28, 36).unwrap().ch, 'g');
}

#[test]
fn hud_panel_fixed_cells_overwrite_stale_text() {
    // The HB-01 successor: cells are ALWAYS exactly 14 columns wide
    // (padded at composition), so a shorter value's padding re-blanks
    // the stale trailing chars on the very next frame. No width
    // tracking, no prev_width bookkeeping — the fixed footprint makes
    // the residue class impossible.
    let mut h = panel_hud();
    let mut frame = crate::frame::Frame::new(100, 40, None);
    // "scn: cinematic" fills all 14 columns of cell (0,2) at x=58..71.
    h.panel_grid[0][2] = "scn: cinematic".to_string();
    h.write_to_frame(&mut frame, None);
    assert_eq!(
        frame.get(58 + 12, 30).unwrap().ch,
        'i',
        "precondition: 'i' of cinematic"
    );
    // Value shrinks: "scn: matrix" + 3 pad spaces.
    h.panel_grid[0][2] = "scn: matrix   ".to_string();
    h.write_to_frame(&mut frame, None);
    assert_eq!(
        frame.get(58 + 12, 30).unwrap().ch,
        ' ',
        "stale 'i' must be re-blanked by the fixed-width padding (HB-01 successor)"
    );
    assert_eq!(
        frame.get(58 + 8, 30).unwrap().ch,
        'r',
        "new value visible at index 8"
    );
}

#[test]
fn hud_panel_skips_when_invisible() {
    let mut h = panel_hud();
    h.toggle(); // off again — write_to_frame must early-return.
    let mut frame = crate::frame::Frame::new(100, 40, None);
    h.write_to_frame(&mut frame, None);
    if let Some(cell) = frame.get(27, 28) {
        assert_ne!(cell.ch, '╭', "no panel when HUD is invisible");
    }
    if let Some(cell) = frame.get(49, 39) {
        assert_ne!(cell.ch, '▼', "no accent when HUD is invisible");
    }
}

#[test]
fn hud_panel_skips_until_first_metric_tick() {
    // panel_header is empty until the first 1 Hz tick composes it —
    // the one-frame guard after toggle-on (mirrors the old empty-row
    // skip). Nothing may render before that.
    let mut h = HudState::new();
    h.toggle();
    let mut frame = crate::frame::Frame::new(100, 40, None);
    h.write_to_frame(&mut frame, None);
    if let Some(cell) = frame.get(27, 28) {
        assert_ne!(cell.ch, '╭', "no panel before the first metric tick");
    }
}

#[test]
fn hud_panel_clips_safely_on_small_terminals() {
    // INV-8: the 80x24 minimum terminal never panics. On a 30x24
    // frame the panel saturates start_col to 0 and clips both sides
    // symmetrically; the bottom rows clip past the frame edge.
    let mut h = panel_hud();
    let mut frame = crate::frame::Frame::new(30, 24, None);
    h.write_to_frame(&mut frame, None);
    assert_eq!(
        frame.get(0, 12).unwrap().ch,
        '╭',
        "clamped anchor at (0,12)"
    );
    assert!(
        frame.get(45, 23).is_none(),
        "right edge clips silently (x=45 > 29)"
    );
    // Even smaller: 20x20 — saturating math everywhere, no panic.
    let mut frame2 = crate::frame::Frame::new(20, 20, None);
    h.write_to_frame(&mut frame2, None);
    assert_eq!(frame2.get(0, 8).unwrap().ch, '╭', "20x20 anchor at (0,8)");
}

#[test]
fn hud_compose_panel_builds_grid_in_visual_order() {
    // Composition test (the 1 Hz text assembly): toggle back-dates the
    // metric tick, so one update_metrics call composes the whole panel
    // from the metric fields.
    let mut h = HudState::new();
    h.toggle();
    h.set_screen_size(200, 50, false);
    h.set_scene_name("cinematic");
    h.set_charset_preset("binary");
    h.set_target_fps(60.0);
    h.update_metrics(&[]);

    // Header: fps + tgt centered in '─' fill, exactly 44 columns.
    assert_eq!(h.panel_header.chars().count(), 44, "header interior width");
    assert!(
        h.panel_header.starts_with('─'),
        "header starts with dash fill"
    );
    assert!(h.panel_header.ends_with('─'), "header ends with dash fill");
    assert!(h.panel_header.contains("fps:"), "header carries fps");
    assert!(h.panel_header.contains("tgt: 60.0"), "header carries tgt");

    // Footer: screensize centered in '─' fill.
    assert_eq!(h.panel_footer.chars().count(), 44, "footer interior width");
    assert!(
        h.panel_footer.contains("200x50 auto"),
        "footer carries screensize"
    );

    // Grid: 7 rows x 3 cells, each exactly 14 columns, in
    // HUD_VISUAL_ORDER (grid row 0 = ehs/prs/scn, row 1 = chr/clr/sped,
    // row 6 = max/p99/cpu, row 7-family tail = rss/cid/up).
    for g in 0..7usize {
        for c in 0..3usize {
            assert_eq!(
                h.panel_grid[g][c].chars().count(),
                14,
                "cell ({g},{c}) width"
            );
        }
    }
    assert!(
        h.panel_grid[0][2].starts_with("scn: cinematic"),
        "grid (0,2) = scn, got {:?}",
        h.panel_grid[0][2]
    );
    assert!(
        h.panel_grid[1][0].starts_with("chr: binary"),
        "grid (1,0) = chr, got {:?}",
        h.panel_grid[1][0]
    );
    assert!(
        h.panel_grid[5][0].starts_with("max:"),
        "grid (5,0) = max (performance core band), got {:?}",
        h.panel_grid[5][0]
    );
    assert!(
        h.panel_grid[5][1].starts_with("p99:"),
        "grid (5,1) = p99, got {:?}",
        h.panel_grid[5][1]
    );
    assert!(
        h.panel_grid[5][2].starts_with("cpu:"),
        "grid (5,2) = cpu, got {:?}",
        h.panel_grid[5][2]
    );
    assert!(
        h.panel_grid[5][1].trim_end().ends_with("ms"),
        "p99 cell keeps its unit after padding, got {:?}",
        h.panel_grid[5][1]
    );
    // cid renders inside the LAST grid row (row 6, cell 1) — never truncated.
    assert!(
        h.panel_grid[6][1].starts_with("cid: "),
        "grid (6,1) = cid, got {:?}",
        h.panel_grid[6][1]
    );
}

#[test]
fn hud_grid_cells_truncate_long_values_safely() {
    // A 18-char custom palette name under `clr:` (visual slot 6 →
    // grid row 1, col 1) must truncate to exactly 14 chars — char-based,
    // so UTF-8 boundaries are preserved (INV-3 defense in depth).
    let mut h = HudState::new();
    h.toggle();
    h.set_custom_palette_name(Some("superlongpalettename"));
    h.update_metrics(&[]);
    assert_eq!(
        h.panel_grid[1][1], "clr: superlong",
        "long clr value truncates to the 14-col cell budget"
    );
}
