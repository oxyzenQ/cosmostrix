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
fn hud_has_twenty_five_cached_lines_after_night_hunter_9() {
    // Regression guard: the HUD must have exactly 25 cached rows after
    // the NIGHT-hunter-9 expansion that added rain (row 19) above dcel
    // (now row 20). Previous counts: 22 (v80.0.0-beta.1 reorder), 24
    // (Z-master-1X round 5 dcel/tcel expansion). If a future change
    // adds or removes a row, this test will catch it.
    let h = HudState::new();
    assert_eq!(
        h.cached_lines.len(),
        25,
        "HUD must have 25 cached rows after the NIGHT-hunter-9 rain expansion"
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
        }, // idx 23
        // NIGHT-hunter-9: 1 new entry for the rain row above dcel.
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 24 → row 24 (screensize, head)
    ];
    h.refresh_colors(&palette);
    // Top row (fps, idx 0) = palette[0] = RGB(0, 50, 0) brightened to RGB(0, 200, 0)
    assert_eq!(
        h.cached_lines[0].0,
        Color::Rgb { r: 0, g: 200, b: 0 },
        "top row (fps) must use palette[0] (brightened dim) — rain tail at top"
    );
    // Row 7 (prs, idx 7) = palette[7] = RGB(220, 240, 150) — max=240
    // >= TARGET_V(200), returned as-is. With a 25-stop palette + 25 HUD
    // rows, t = 7/24.0 maps to palette[7] exactly (1:1 mapping).
    assert_eq!(
        h.cached_lines[7].0,
        Color::Rgb {
            r: 220,
            g: 240,
            b: 150
        },
        "row 7 (prs) must use palette[7] — near head but not the head"
    );
    // Bottom row (screensize, idx 24 — NIGHT-hunter-9: moved down from
    // row 23 to row 24) = palette[24] = RGB(255,255,255) — head.
    assert_eq!(
        h.cached_lines[24].0,
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        },
        "bottom row (screensize) must use palette[24] (head) — bright head at bottom"
    );
    // Middle rows should NOT be white — they should be the intermediate
    // green stops, not the head.
    assert_ne!(
        h.cached_lines[2].0,
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        },
        "row 2 (max) must NOT use head — only bottom row gets head"
    );
    assert_ne!(
        h.cached_lines[4].0,
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        },
        "row 4 (cpu) must NOT use head — only bottom row gets head"
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
        h.cached_lines[24].0,
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
        h.cached_lines[24].0,
        Color::Rgb {
            r: 255,
            g: 176,
            b: 0
        },
        "second refresh (immediate): bottom = amber head"
    );
}

#[test]
fn refresh_colors_gradient_uses_twenty_five_distinct_stops() {
    // HD-01: 25 HUD rows now use 25 distinct palette stops (one per row),
    // sweeping the full chroma dragon gradient top→bottom.
    // Z-master-1X round 5: bumped from 22 → 24 stops (dcel + tcel added).
    // NIGHT-hunter-9: bumped from 24 → 25 stops (rain added above dcel).
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
        }, // idx 23 → row 23
        // NIGHT-hunter-9: 1 new entry for rain (row 19, above dcel).
        // The bottom row (screensize) shifted down from row 23 to row 24.
        Color::Rgb {
            r: 220,
            g: 200,
            b: 180,
        }, // idx 24 → row 24 (screensize, head)
    ];
    h.refresh_colors(&palette);
    // All palette entries have max channel >= TARGET_V(200), so brighten
    // returns each as-is. This isolates the gradient mapping test from
    // the brightening math (covered separately by brighten_color_* tests).
    //
    // Z-master-1X round 5: with 24 palette entries + 24 HUD rows, t = i/23.0
    // (NIGHT-hunter-9: bumped to 25/25, t = i/24.0 — but this test asserts
    // distinctness, not the exact count, so the bump is safe.)
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
        "25-row HUD gradient must produce >=20 distinct colors with a 25-stop palette (got {distinct_count}) — banded gradient would indicate an interpolation regression"
    );
    // Boundary rows (0 and 24) must still be exact — t=0.0 and t=1.0
    // land on integer positions with no floating-point drift.
    assert_eq!(
        h.cached_lines[0].0, palette[0],
        "row 0 must use palette[0] exactly (t=0.0, no interpolation)"
    );
    assert_eq!(
        h.cached_lines[24].0, palette[24],
        "row 24 must use palette[24] exactly (t=1.0, no interpolation)"
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
    // never mutated — `update_metrics` skips row 22 entirely so the
    // commit hash remains stable across the entire process lifetime.
    // The owner needs to read the commit hash without quitting cosmostrix.
    // Z-master-1X round 5: cid moved from row 19 to row 21 (dcel/tcel
    // inserted at rows 19-20 above cid).
    // NIGHT-hunter-9: cid moved down again from row 21 to row 22
    // (rain inserted at row 19 above dcel, pushing everything down).
    let h = HudState::new();
    let (_, cid_line) = &h.cached_lines[22];
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
fn compute_chroma_gradient_25_sweeps_full_palette_range() {
    // HD-01 regression: verify the 25-stop chroma gradient helper maps
    // the first and last HUD rows to the corresponding palette boundary
    // stops. Row 0 → palette[0] (t=0.0), row 24 → palette[n-1] (t=1.0).
    // Z-master-1X round 5: bumped from 22 → 24 stops to add dcel + tcel.
    // NIGHT-hunter-9: bumped from 24 → 25 stops to add rain above dcel.
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
    let colors = compute_chroma_gradient_25(&palette);
    // Row 0 = palette[0] = RGB(50,0,0) brightened to RGB(200,0,0).
    // t=0.0 maps exactly to palette[0] — no interpolation needed.
    assert_eq!(colors[0], Color::Rgb { r: 200, g: 0, b: 0 });
    // Row 24 = palette[15] = RGB(100,100,100) brightened to RGB(200,200,200).
    // t=1.0 maps exactly to palette[n-1] — the last stop. max channel is
    // 100, scaled by 200/100 = 2.0x to reach the TARGET_V=200 floor.
    // (100 * 200 / 100 = 200.) This is the screensize row (row 24, owner-
    // mandated bottom — NIGHT-hunter-9 moved it down from 23) — the
    // chroma gradient sweeps from palette[0] at the top (dim tail) to
    // palette[n-1] at the bottom (bright head).
    assert_eq!(
        colors[24],
        Color::Rgb {
            r: 200,
            g: 200,
            b: 200
        },
        "screensize row (idx 24) must use the last palette stop brightened to TARGET_V=200"
    );
}

// HB-01 regression test: HUD width shrink (e.g., tgt drops " idle" suffix)
// must clear the previously-occupied trailing cells immediately.
//
// v80.0.0-beta.1 chroma border update: when current_width shrinks to 13,
// col 13 becomes the new border position (the border tracks current_width
// directly, not max(cur,prev)). The stale 'e' at col 13 is replaced by
// the border char '│' — the HB-01 bug (stale 'e' visible) is still fixed,
// just via border replacement instead of blanking. To also verify that
// TEXT area trailing cells are still blanked (not just replaced by
// border), we check cell (10,1) which held 'i' from "idle" and is now
// in the padding area (blanked to ' ').
#[test]
fn hud_write_to_frame_clears_trailing_cells_when_width_shrinks() {
    let mut h = HudState::new();
    h.toggle(); // make visible
                // 14-char string ending in "idle" (mirrors owner repro: " tgt: 144 idle").
    h.cached_lines[1].1 = " tgt: 144 idle".to_string();
    h.current_width = 14;
    h.prev_width = 14;

    let cols = 40u16;
    let mut frame = crate::frame::Frame::new(cols, 8, None);
    h.write_to_frame(&mut frame, cols, None);

    assert!(
        frame.get(13, 1).is_some(),
        "precondition: cell (13,1) must exist after wide write"
    );

    h.cached_lines[1].1 = " tgt: 144".to_string();
    h.current_width = 13; // shrunk

    h.write_to_frame(&mut frame, cols, None);

    // Cell (10, 1) held 'i' from "idle" — now in the padding area, must
    // be blanked (this is the classic HB-01 text-area clearing check).
    let cleared = frame
        .get(10, 1)
        .expect("cell (10,1) must exist after shrink");
    assert_eq!(
        cleared.ch, ' ',
        "cell (10,1) must be blanked — held stale 'i' from previous wide text (HB-01 bug)"
    );
    assert!(cleared.fg.is_none(), "cell (10,1) fg must be None");

    // Cell (13, 1) held 'e' from "idle" — now the border position
    // (current_width = 13). The stale 'e' is replaced by '│', not blanked.
    let border_cell = frame
        .get(13, 1)
        .expect("cell (13,1) must exist after shrink");
    assert_ne!(
        border_cell.ch, 'e',
        "cell (13,1) must not hold stale 'e' — replaced by border (HB-01 still fixed)"
    );
    assert_eq!(
        border_cell.ch, '│',
        "cell (13,1) is the new border position (current_width = 13)"
    );
}

// v80.0.0-beta.1 HUD chroma border regression test (owner mandate 2026-09-02):
// The HUD draws an L-shape border (right + bottom) using the same chroma
// dragon palette integration as the message border (BC-01..05). This test
// verifies the border shape, character set, and per-row color sweep.
//
// v80.0.0-beta.1 edge fade update (owner mandate 2026-09-02, "visual 9/10"):
// the border edges fade toward the screen edge (top-left corner of screen)
// so the border "emerges from shadow". Right edge: row 0 = max fade, row 23
// = no fade. Bottom edge: col 0 = max fade, col cur = no fade (corner
// anchor). This test verifies the fade behavior (faded at screen-edge ends,
// full-bright at the anchor corner).
#[test]
fn hud_border_draws_l_shape_with_chroma_colors() {
    let mut h = HudState::new();
    h.toggle(); // make visible

    // Set distinct colors per row so we can verify the per-row sweep on
    // the right edge. Row 0 (top) is dim, row 23 (bottom) is bright —
    // matches the HUD's own gradient direction.
    let test_color = |i: usize| -> Color {
        Color::Rgb {
            r: (i * 10) as u8,
            g: (100 + i * 5) as u8,
            b: 200,
        }
    };
    for i in 0..24 {
        h.cached_lines[i].0 = test_color(i);
        h.cached_lines[i].1 = format!(" row{}", i);
    }
    h.current_width = 10;
    h.prev_width = 10;

    let cols = 40u16;
    // 30 rows so the bottom border at row 24 is in-bounds.
    let mut frame = crate::frame::Frame::new(cols, 30, None);
    // bg = None → draw_border defaults to black for the fade target.
    h.write_to_frame(&mut frame, cols, None);

    let head_color = test_color(23);

    // Right border: col = 10 (hud_width), rows 0..23, char '│'.
    // Edge fade: row 0 (top) = max fade (blended toward black), row 23
    // (bottom) = no fade (full head_color).
    for row in 0..24u16 {
        let cell = frame
            .get(10, row)
            .unwrap_or_else(|| panic!("right border cell at (10,{}) must exist", row));
        assert_eq!(
            cell.ch, '│',
            "right border at (10,{}) must be the vertical box-drawing char",
            row
        );
        assert!(!cell.bold, "border must not be bold");

        // Fade factor: 0.6 * (1.0 - row / 23.0). row 23 → 0.0 (no fade),
        // row 0 → 0.6 (max fade). Verify behavior:
        // - row 23: fg == test_color(23) (no fade, full color)
        // - row 0..22: fg != test_color(row) (faded toward black)
        if row == 23 {
            assert_eq!(
                cell.fg,
                Some(test_color(23)),
                "right border at row 23 (head anchor) must have NO fade — full color"
            );
        } else {
            assert_ne!(
                cell.fg,
                Some(test_color(row as usize)),
                "right border at row {} must be faded toward bg (not full color)",
                row
            );
            // Faded color should be darker (each channel < original when
            // bg is black and original channel > 0).
            if let Some(Color::Rgb { r, g, b }) = cell.fg {
                let orig = test_color(row as usize);
                if let Color::Rgb {
                    r: or_,
                    g: og,
                    b: ob,
                } = orig
                {
                    assert!(
                        r <= or_ && g <= og && b <= ob,
                        "faded color at row {} must be <= original on each channel (blend toward black)",
                        row
                    );
                }
            }
        }
    }

    // Bottom border: row = 24, cols 0..9, char '─'.
    // Edge fade: col 0 (left) = max fade, col 9 (just before corner) =
    // small fade. The corner at col 10 is tested separately below.
    for col in 0..10u16 {
        let cell = frame
            .get(col, 24)
            .unwrap_or_else(|| panic!("bottom border cell at ({},24) must exist", col));
        assert_eq!(
            cell.ch, '─',
            "bottom border at ({},24) must be the horizontal box-drawing char",
            col
        );
        // All bottom cells are faded (col 0 max, col 9 small) except
        // the corner at col 10 which has no fade. Cols 0..9 are all
        // faded (factor > 0 because col < cur=10).
        assert_ne!(
            cell.fg,
            Some(head_color),
            "bottom border at col {} must be faded toward bg (not full head_color)",
            col
        );
        if let Some(Color::Rgb { r, g, b }) = cell.fg {
            if let Color::Rgb {
                r: hr,
                g: hg,
                b: hb,
            } = head_color
            {
                assert!(
                    r <= hr && g <= hg && b <= hb,
                    "faded bottom color at col {} must be <= head_color on each channel",
                    col
                );
            }
        }
    }

    // Corner: (10, 24), char '╯' (light up-left corner), color = head_color
    // (NO fade — the corner is the bright anchor point).
    let corner = frame
        .get(10, 24)
        .expect("corner cell at (10,24) must exist");
    assert_eq!(
        corner.ch, '╯',
        "corner at (10,24) must be the up-left corner char"
    );
    assert_eq!(
        corner.fg,
        Some(head_color),
        "corner must use the head color (NO fade — anchor point)"
    );
}

// v80.0.0-beta.1 edge fade gradient test: verify the fade is a LINEAR ramp
// from max fade (screen-edge end) to no fade (anchor end). The right edge
// row 0 should be more faded than row 11 (midpoint), which should be more
// faded than row 23 (no fade). This catches a bug where the fade is
// applied uniformly instead of as a gradient.
#[test]
fn hud_border_edge_fade_is_linear_gradient() {
    let mut h = HudState::new();
    h.toggle();

    // Use a single bright color for all rows so the fade is the only
    // variable (no per-row chroma sweep confounding the comparison).
    let bright = Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    };
    for i in 0..24 {
        h.cached_lines[i].0 = bright;
        h.cached_lines[i].1 = format!(" row{}", i);
    }
    h.current_width = 10;
    h.prev_width = 10;

    let cols = 40u16;
    let mut frame = crate::frame::Frame::new(cols, 30, None);
    h.write_to_frame(&mut frame, cols, None);

    // Right edge: extract the R channel (as a proxy for brightness) at
    // rows 0, 11, 23. With bg=black, fade blends toward (0,0,0), so
    // R channel = 200 * (1 - fade). row 0 fade=0.6 → R=80. row 11
    // fade≈0.31 → R≈138. row 23 fade=0.0 → R=200.
    let r_at = |row: u16| -> u8 {
        if let Some(Color::Rgb { r, .. }) = frame.get(10, row).and_then(|c| c.fg) {
            r
        } else {
            panic!("no fg at row {}", row)
        }
    };
    let r0 = r_at(0);
    let r11 = r_at(11);
    let r23 = r_at(23);

    // Linear gradient: r0 < r11 < r23 (strictly increasing brightness).
    assert!(
        r0 < r11,
        "row 0 (R={}) must be darker than row 11 (R={}) — linear fade gradient",
        r0,
        r11
    );
    assert!(
        r11 < r23,
        "row 11 (R={}) must be darker than row 23 (R={}) — linear fade gradient",
        r11,
        r23
    );
    // row 23 has no fade → R == 200 (full bright).
    assert_eq!(
        r23, 200,
        "row 23 (anchor) must have NO fade — R == 200 (full bright)"
    );
    // row 0 has max fade (0.6) → R ≈ 80 (200 * 0.4, with integer
    // rounding in lerp_u8 the exact value is 81 — assert a range to
    // be robust to the rounding).
    assert!(
        r0 <= 85,
        "row 0 (screen-edge end) must have max fade — R <= 85 (got {}), expected ~80",
        r0
    );
    assert!(
        r0 >= 75,
        "row 0 (screen-edge end) fade must not be too aggressive — R >= 75 (got {})",
        r0
    );
}

// Border must NOT draw when hud_width is 0 (empty HUD — no metrics yet).
#[test]
fn hud_border_skips_when_hud_width_zero() {
    let mut h = HudState::new();
    h.toggle();
    h.current_width = 0;
    h.prev_width = 0;

    let cols = 40u16;
    let mut frame = crate::frame::Frame::new(cols, 30, None);
    h.write_to_frame(&mut frame, cols, None);

    // No bottom border char at row 24.
    if let Some(cell) = frame.get(0, 24) {
        assert_ne!(cell.ch, '─', "no bottom border when hud_width is 0");
    }
}

// Border must NOT draw when HUD is invisible (write_to_frame early-returns).
#[test]
fn hud_border_skips_when_invisible() {
    let mut h = HudState::new();
    // Do NOT toggle — HUD stays invisible.
    h.cached_lines[0].0 = Color::Rgb { r: 255, g: 0, b: 0 };
    h.cached_lines[0].1 = " test".to_string();
    h.current_width = 10;
    h.prev_width = 10;

    let cols = 40u16;
    let mut frame = crate::frame::Frame::new(cols, 30, None);
    h.write_to_frame(&mut frame, cols, None);

    // No right border char at col 10.
    if let Some(cell) = frame.get(10, 0) {
        assert_ne!(cell.ch, '│', "no right border when HUD is invisible");
    }
}

// v80.0.0-beta.1 residue fix regression test (owner bug report 2026-09-02):
// When the HUD width shrinks (e.g. `dcel` value gets shorter), the border
// moves LEFT to the new `current_width`. The old border cells at the
// previous (wider) position MUST be blanked — otherwise they leave a
// visible "stain" or "ghost" that looks like a glitch effect.
//
// This test reproduces the exact scenario the owner reported: border at
// col 14 (wide metrics), then shrinks to col 10 (shorter `dcel` value),
// then verifies col 14 is fully blanked (no stale `│`).
#[test]
fn hud_border_clears_stale_cells_when_width_shrinks() {
    let mut h = HudState::new();
    h.toggle(); // make visible

    let test_color = |i: usize| -> Color {
        Color::Rgb {
            r: (i * 10) as u8,
            g: 100,
            b: 200,
        }
    };
    for i in 0..24 {
        h.cached_lines[i].0 = test_color(i);
        h.cached_lines[i].1 = format!(" row{}", i);
    }

    // Frame 1: width = 14 (wide metrics, e.g. dcel shows a long value).
    h.current_width = 14;
    h.prev_width = 14;
    let cols = 40u16;
    let mut frame = crate::frame::Frame::new(cols, 30, None);
    h.write_to_frame(&mut frame, cols, None);

    // Verify border at col 14 (the wide position).
    assert_eq!(
        frame.get(14, 0).unwrap().ch,
        '│',
        "frame 1: right border at col 14"
    );
    assert_eq!(
        frame.get(14, 24).unwrap().ch,
        '╯',
        "frame 1: corner at (14,24)"
    );

    // Frame 2: width shrinks to 10 (dcel value got shorter).
    // prev_width is now 14 (set at end of frame 1's write_to_frame).
    h.current_width = 10;
    h.write_to_frame(&mut frame, cols, None);

    // New border at col 10.
    assert_eq!(
        frame.get(10, 0).unwrap().ch,
        '│',
        "frame 2: new right border at col 10"
    );
    assert_eq!(
        frame.get(10, 24).unwrap().ch,
        '╯',
        "frame 2: new corner at (10,24)"
    );

    // OLD border at col 14 MUST be cleared (no residue/stain).
    let old_right = frame.get(14, 0).expect("old border cell must exist");
    assert_eq!(
        old_right.ch, ' ',
        "frame 2: old right border at col 14 must be blanked (no residue) — was the owner's glitch bug"
    );
    assert!(
        old_right.fg.is_none(),
        "frame 2: old right border fg must be None"
    );

    // OLD bottom border cells at cols 11..=14, row 24 MUST be cleared.
    for col in 11..=14u16 {
        let old_bottom = frame.get(col, 24).expect("old bottom cell must exist");
        assert_eq!(
            old_bottom.ch, ' ',
            "frame 2: old bottom border at ({},24) must be blanked",
            col
        );
    }

    // OLD corner at (14, 24) MUST be cleared (was '╯', now ' ').
    let old_corner = frame.get(14, 24).expect("old corner cell must exist");
    assert_eq!(
        old_corner.ch, ' ',
        "frame 2: old corner at (14,24) must be blanked"
    );
}

// Border must move RIGHT cleanly when width grows (no stale cells expected,
// but verify the new position is drawn and no residue at old position).
#[test]
fn hud_border_grows_right_cleanly() {
    let mut h = HudState::new();
    h.toggle();

    for i in 0..24 {
        h.cached_lines[i].0 = Color::Rgb {
            r: 100,
            g: 100,
            b: 100,
        };
        h.cached_lines[i].1 = format!(" row{}", i);
    }

    // Frame 1: width = 8.
    h.current_width = 8;
    h.prev_width = 8;
    let cols = 40u16;
    let mut frame = crate::frame::Frame::new(cols, 30, None);
    h.write_to_frame(&mut frame, cols, None);
    assert_eq!(frame.get(8, 0).unwrap().ch, '│');

    // Frame 2: width grows to 12.
    h.current_width = 12;
    h.write_to_frame(&mut frame, cols, None);

    // New border at col 12.
    assert_eq!(
        frame.get(12, 0).unwrap().ch,
        '│',
        "frame 2: new right border at col 12"
    );

    // Old border at col 8 should now be part of the text/padding area
    // (the metrics loop blanks it via padding). It must NOT still be '│'.
    let old_cell = frame.get(8, 0).expect("old border cell must exist");
    assert_ne!(
        old_cell.ch, '│',
        "frame 2: old border at col 8 must not still be the vertical char (should be text or blank)"
    );
}
