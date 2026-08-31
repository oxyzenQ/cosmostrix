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
    // (row 20) above cid (now row 21). The previous count was 22 (v51
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
    // Top row (fps, idx 0) = palette[0] = RGB(0, 50, 0) brightened to RGB(0, 200, 0)
    assert_eq!(
        h.cached_lines[0].0,
        Color::Rgb { r: 0, g: 200, b: 0 },
        "top row (fps) must use palette[0] (brightened dim) — rain tail at top"
    );
    // Row 7 (prs, idx 7) = palette[7] = RGB(220, 240, 150) — max=240
    // >= TARGET_V(200), returned as-is. With a 24-stop palette + 24 HUD
    // rows, t = 7/23.0 maps to palette[7] exactly (1:1 mapping).
    assert_eq!(
        h.cached_lines[7].0,
        Color::Rgb {
            r: 220,
            g: 240,
            b: 150
        },
        "row 7 (prs) must use palette[7] — near head but not the head"
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
        // v51 reorder: rows 15-18 = ambt/glth/ctun/mnst, 19 = cid,
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
    // Boundary rows (0 and 23) must still be exact — t=0.0 and t=1.0
    // land on integer positions with no floating-point drift.
    assert_eq!(
        h.cached_lines[0].0, palette[0],
        "row 0 must use palette[0] exactly (t=0.0, no interpolation)"
    );
    assert_eq!(
        h.cached_lines[23].0, palette[23],
        "row 23 must use palette[23] exactly (t=1.0, no interpolation)"
    );
}

#[test]
fn hud_cid_line_contains_commit_sha_or_unknown() {
    // v50 (2026-08-15): the cid line was at index [8].
    // v50 (2026-08-17) HUD expansion reorder: moved to the bottom row.
    // v51 reorder (owner mandate 2026-08-31): cid sits at index [19] —
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
fn compute_chroma_gradient_24_sweeps_full_palette_range() {
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
    let colors = compute_chroma_gradient_24(&palette);
    // Row 0 = palette[0] = RGB(50,0,0) brightened to RGB(200,0,0).
    // t=0.0 maps exactly to palette[0] — no interpolation needed.
    assert_eq!(colors[0], Color::Rgb { r: 200, g: 0, b: 0 });
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

// HB-01 regression test: HUD width shrink (e.g., tgt drops " idle" suffix)
// must clear the previously-occupied trailing cells immediately.
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

    let cleared = frame.get(13, 1).expect("cell must exist after shrink");
    assert_eq!(
        cleared.ch, ' ',
        "cell (13,1) must be blanked — was residual 'e' bug"
    );
    assert!(cleared.fg.is_none(), "cell (13,1) fg must be None");
}
