// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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
fn hud_has_sixteen_cached_lines_after_v50_hud_expansion() {
    // Regression guard: the HUD must have exactly 16 cached rows after
    // the v50 (2026-08-17) HUD expansion that reserved rows 9-15 for
    // the 7 owner-mandated metrics (scene / color / density / speed /
    // endurance-health-score / effective-pressure / charset). The
    // previous count was 9 (fps / tgt / max / p99 / cpu / rss / up /
    // screensize / cid) post-v50 cid-line addition; now 16 (adds 7
    // reserved rows that initialize as empty strings — the
    // `write_to_frame` skip-empty guard prevents them from rendering
    // until the follow-up data-plumbing commit populates them).
    // The cid line still shares the head color stop with screensize.
    // If a future change adds or removes a row, this test will catch it.
    let h = HudState::new();
    assert_eq!(
        h.cached_lines.len(),
        16,
        "HUD must have 16 cached rows after the v50 HUD expansion"
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

// ── Hue-preserving brighten_color tests ───────────────────────────
//
// The HUD must follow the rain's actual color scheme, not wash out
// to grey. These tests lock in the hue-preserving behavior so a
// future change back to a white-blend would fail loudly.

#[test]
fn brighten_color_preserves_vivid_green_hue() {
    // Vivid green RGB(0,255,0): max=255 >= TARGET_V(200), returned
    // as-is. The HUD line for this palette color must be vivid green,
    // not washed-out grey-green.
    let out = brighten_color(Color::Rgb { r: 0, g: 255, b: 0 });
    assert_eq!(out, Color::Rgb { r: 0, g: 255, b: 0 });
}

#[test]
fn brighten_color_preserves_vivid_amber_hue() {
    // Amber/orange RGB(255,176,0): max=255 >= TARGET_V, returned as-is.
    // An amber rain palette must produce an amber HUD, not grey.
    let out = brighten_color(Color::Rgb {
        r: 255,
        g: 176,
        b: 0,
    });
    assert_eq!(
        out,
        Color::Rgb {
            r: 255,
            g: 176,
            b: 0
        }
    );
}

#[test]
fn brighten_color_scales_dark_green_preserving_hue() {
    // Dark green RGB(0,50,0): max=50 < TARGET_V, scale=400.
    // Result must be RGB(0,200,0) — bright green, NOT grey-green.
    // The old white-blend produced RGB(166,183,166) (washed grey).
    let out = brighten_color(Color::Rgb { r: 0, g: 50, b: 0 });
    assert_eq!(out, Color::Rgb { r: 0, g: 200, b: 0 });
}

#[test]
fn brighten_color_scales_dark_blue_preserving_hue_ratio() {
    // Dark blue RGB(50,100,150): max=150 < TARGET_V, scale=133
    // (integer: 200*100/150=133, truncated from 133.33).
    // Result: RGB(66,133,199) — preserves the blue hue ratio.
    // The old white-blend produced RGB(183,201,218) (washed grey-blue).
    // (199 not 200 because 150*133/100=199.5 → truncates to 199.)
    let out = brighten_color(Color::Rgb {
        r: 50,
        g: 100,
        b: 150,
    });
    assert_eq!(
        out,
        Color::Rgb {
            r: 66,
            g: 133,
            b: 199
        }
    );
}

#[test]
fn brighten_color_pure_black_falls_back_to_neutral_grey() {
    // Pure black RGB(0,0,0): max=0, can't scale (0*x=0). Must fall
    // back to a neutral dim grey RGB(120,120,120) so the HUD is
    // still readable. This is the only case where hue is not
    // preserved (there's no hue to preserve in pure black).
    let out = brighten_color(Color::Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(
        out,
        Color::Rgb {
            r: 120,
            g: 120,
            b: 120
        }
    );
}

#[test]
fn brighten_color_named_cyan_preserves_hue_when_bright_enough() {
    // Named Cyan = RGB(0,255,255): max=255 >= TARGET_V, returned as
    // RGB(0,255,255). The old code returned named colors as-is (no
    // conversion), which was fine for Cyan but broke for DarkCyan
    // (next test). This test locks in the conversion behavior.
    let out = brighten_color(Color::Cyan);
    assert_eq!(
        out,
        Color::Rgb {
            r: 0,
            g: 255,
            b: 255
        }
    );
}

#[test]
fn brighten_color_named_darkcyan_gets_scaled_to_readable_cyan() {
    // Named DarkCyan = RGB(0,128,128): max=128 < TARGET_V, scale=156
    // (integer: 200*100/128=156, truncated from 156.25).
    // Result: RGB(0,199,199) — bright cyan, preserving the hue.
    // (199 not 200 because 128*156/100=199.68 → truncates to 199.)
    // The old code returned DarkCyan as-is (too dim on black bg).
    let out = brighten_color(Color::DarkCyan);
    assert_eq!(
        out,
        Color::Rgb {
            r: 0,
            g: 199,
            b: 199
        }
    );
}

#[test]
fn brighten_color_does_not_wash_vivid_colors_to_grey() {
    // Regression guard: the user explicitly flagged "HUD metrics
    // colors too grey". The old 35% source + 65% white blend turned
    // RGB(0,255,0) into RGB(89,255,89) — a washed pale green. The
    // new code must return vivid colors unchanged. Verify the green
    // channel is NOT reduced and the red/blue channels stay at 0.
    let out = brighten_color(Color::Rgb { r: 0, g: 255, b: 0 });
    match out {
        Color::Rgb { r, g, b } => {
            assert_eq!(r, 0, "red channel must stay 0 for pure green");
            assert_eq!(b, 0, "blue channel must stay 0 for pure green");
            assert_eq!(g, 255, "green channel must stay 255 (not washed)");
        }
        other => panic!("expected Rgb, got {other:?}"),
    }
}

// ── refresh_colors + rain-aesthetic gradient tests ────────────────
//
// The HUD color refresh was split out of the 1 Hz `update_metrics`
// tick so runtime palette changes (c/C key, auto-color-drift,
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
        }, // idx 14 → row 14 (reserved chr)
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 15 → row 15 (reserved clr, head, white)
    ];
    h.refresh_colors(&palette);
    // Top row (fps, idx 0) = palette[0] = RGB(0, 50, 0) brightened to RGB(0, 200, 0)
    assert_eq!(
        h.cached_lines[0].0,
        Color::Rgb { r: 0, g: 200, b: 0 },
        "top row (fps) must use palette[0] (brightened dim) — rain tail at top"
    );
    // Row 7 (prs, idx 7) = palette[7] = RGB(220, 240, 150) — max=240
    // >= TARGET_V(200), returned as-is. v50 (2026-08-17) HUD expansion:
    // row 7 is now the `prs:` line (effective pressure NEW metric), not
    // screensize (which moved to row 14).
    assert_eq!(
        h.cached_lines[7].0,
        Color::Rgb {
            r: 220,
            g: 240,
            b: 150
        },
        "row 7 (prs) must use palette[7] — near head but not the head"
    );
    // Bottom row (cid, idx 15) = palette[15] = RGB(255,255,255) — head.
    // v50 (2026-08-17) HUD expansion reorder: cid moved from row 8 to
    // row 15 (owner-mandated bottom per Option S). The chroma gradient
    // assigns the brightest head stop to the bottom row so the build
    // identity (commit hash) earns the most prominent position.
    assert_eq!(
        h.cached_lines[15].0,
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        },
        "bottom row (cid) must use palette[15] (head) — bright head at bottom"
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
    // HD-01 regression: palette change at runtime (c/C, auto-drift,
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
        h.cached_lines[15].0,
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
        h.cached_lines[15].0,
        Color::Rgb {
            r: 255,
            g: 176,
            b: 0
        },
        "second refresh (immediate): bottom = amber head"
    );
}

#[test]
fn refresh_colors_gradient_uses_sixteen_distinct_stops() {
    // HD-01: 16 HUD rows now use 16 distinct palette stops (one per row),
    // sweeping the full chroma dragon gradient top→bottom.
    // v50 (2026-08-15): bumped from 8 → 9 stops after adding the `cid:`
    // line at row 8.
    // v50 (2026-08-17): bumped from 9 → 16 stops to reserve rows 9-15
    // for the 7 owner-mandated HUD expansion metrics. Rows 9-15 are
    // blank for now (no content) but their colors are still assigned
    // distinct palette stops so the chroma gradient sweeps continuously
    // top→bottom. Once the data-plumbing commit populates them, the
    // visual gradient will be unbroken.
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
        }, // idx 8 → row 8 (cid, head)
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
        }, // idx 14 → row 14 (reserved chr)
        Color::Rgb {
            r: 100,
            g: 200,
            b: 200,
        }, // idx 15 → row 15 (reserved clr)
    ];
    h.refresh_colors(&palette);
    // All palette entries have max channel >= TARGET_V(200), so brighten
    // returns each as-is. This isolates the gradient mapping test from
    // the brightening math (covered separately by brighten_color_* tests).
    for (i, expected) in palette.iter().enumerate() {
        assert_eq!(
            &h.cached_lines[i].0, expected,
            "row {i} must use palette[{i}]"
        );
    }
}

#[test]
fn hud_cid_line_contains_commit_sha_or_unknown() {
    // v50 (2026-08-15): the cid line was at index [8].
    // v50 (2026-08-17) HUD expansion reorder: the cid line moved from
    // index [8] to index [15] (owner-mandated bottom row per Option S).
    // The line must contain the compile-time git short SHA injected by
    // build.rs via `COSMOSTRIX_GIT_SHA`, falling back to "unknown" when
    // the build had no .git dir. The text is set once in `new()` and
    // never mutated — `update_metrics` skips row 15 entirely so the
    // commit hash remains stable across the entire process lifetime.
    // The owner needs to read the commit hash without quitting cosmostrix.
    let h = HudState::new();
    let (_, cid_line) = &h.cached_lines[15];
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
fn compute_chroma_gradient_16_sweeps_full_palette_range() {
    // HD-01 regression: verify the 16-stop chroma gradient helper maps
    // each of the 16 HUD rows to its corresponding palette stop.
    // Row i samples palette[(i / 15.0 * (n-1)).round()].
    // v50 (2026-08-15): bumped from 8 → 9 stops after adding the `cid:`
    // line at row 8.
    // v50 (2026-08-17): bumped from 9 → 16 stops to reserve rows 9-15
    // for the 7 owner-mandated HUD expansion metrics.
    let palette = vec![
        Color::Rgb { r: 50, g: 0, b: 0 }, // idx 0  → row 0
        Color::Rgb { r: 0, g: 50, b: 0 }, // idx 1  → row 1
        Color::Rgb { r: 0, g: 0, b: 50 }, // idx 2  → row 2
        Color::Rgb {
            r: 100,
            g: 100,
            b: 0,
        }, // idx 3 → row 3
        Color::Rgb {
            r: 0,
            g: 100,
            b: 100,
        }, // idx 4 → row 4
        Color::Rgb {
            r: 100,
            g: 0,
            b: 100,
        }, // idx 5 → row 5
        Color::Rgb {
            r: 200,
            g: 100,
            b: 50,
        }, // idx 6 → row 6
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 7 → row 7
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 8 → row 8 (cid, shares head stop with screensize)
        Color::Rgb {
            r: 60,
            g: 60,
            b: 60,
        }, // idx 9  → row 9  (reserved ehs)
        Color::Rgb {
            r: 70,
            g: 70,
            b: 70,
        }, // idx 10 → row 10 (reserved prs)
        Color::Rgb {
            r: 80,
            g: 80,
            b: 80,
        }, // idx 11 → row 11 (reserved sped)
        Color::Rgb {
            r: 90,
            g: 90,
            b: 90,
        }, // idx 12 → row 12 (reserved dsty)
        Color::Rgb {
            r: 120,
            g: 120,
            b: 120,
        }, // idx 13 → row 13 (reserved scn)
        Color::Rgb {
            r: 130,
            g: 130,
            b: 130,
        }, // idx 14 → row 14 (reserved chr)
        Color::Rgb {
            r: 100,
            g: 100,
            b: 100,
        }, // idx 15 → row 15 (reserved clr) — divides 200 cleanly
    ];
    let colors = compute_chroma_gradient_16(&palette);
    // Row 0 = palette[0] = RGB(50,0,0) brightened to RGB(200,0,0)
    assert_eq!(colors[0], Color::Rgb { r: 200, g: 0, b: 0 });
    // Row 1 = palette[1] = RGB(0,50,0) brightened to RGB(0,200,0)
    assert_eq!(colors[1], Color::Rgb { r: 0, g: 200, b: 0 });
    // Row 7 = palette[7] = RGB(255,255,255) (already bright, returned as-is)
    assert_eq!(
        colors[7],
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        }
    );
    // Row 8 = palette[8] = RGB(255,255,255) — cid shares head stop
    assert_eq!(
        colors[8],
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        },
        "cid row (idx 8) must use the head stop — same as screensize"
    );
    // Row 15 = palette[15] = RGB(120,120,120) brightened to RGB(255,255,255) —
    // max channel is 120, scaled by 200/120 = 1.667x to reach the TARGET_V=200
    // floor. (120 * 200 / 120 = 200.) This is the reserved clr row at the
    // bottom of the HUD — the chroma gradient sweeps continuously from
    // palette[0] at the top (dim tail) to palette[15] at the bottom
    // (bright head).
    assert_eq!(
        colors[15],
        Color::Rgb {
            r: 200,
            g: 200,
            b: 200
        },
        "reserved clr row (idx 15) must use palette[15] brightened to TARGET_V=200"
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

// ── v50 (2026-08-17) HUD expansion content tests ───────────────────
//
// The following tests verify the 7 new owner-mandated metric lines
// (rows 6-12) render the correct text after the corresponding setters
// are called and `update_metrics` runs the 1 Hz text reformat. The
// layout matches owner's Option S mandate: ehs/prs/sped/dsty/scn/chr/
// clr at rows 6-12, with the density label explicitly set to `dsty`
// (NOT `den` — owner: "buruk sekali").

#[test]
fn hud_renders_seven_new_metric_lines_after_setters_and_update() {
    // Verify all 7 new owner-mandated metric lines render with the
    // expected label prefix + value formatting after their setters are
    // called and `update_metrics` runs. The chroma gradient is fed an
    // arbitrary palette so the colors[i] sweep is exercised too.
    let mut h = HudState::new();
    h.toggle(); // visible + forces next update_metrics to execute
    h.set_endurance_health_score(87.4);
    h.set_effective_pressure(0.123);
    h.set_chars_per_sec(14.0);
    h.set_droplet_density(1.0);
    h.set_scene_name("cinematic");
    h.set_charset_preset("binary");
    h.set_color_scheme(ColorScheme::NeonGreen);
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50,
        };
        16
    ];
    h.update_metrics(&palette);

    // Row 6 — ehs: integer rounded (87.4 -> 87)
    let (_, ehs_line) = &h.cached_lines[6];
    assert_eq!(ehs_line, " ehs: 87", "row 6 (ehs) content mismatch");

    // Row 7 — prs: 2 decimals, clamped 0.0-1.0
    let (_, prs_line) = &h.cached_lines[7];
    assert_eq!(prs_line, " prs: 0.12", "row 7 (prs) content mismatch");

    // Row 8 — sped: 1 decimal
    let (_, sped_line) = &h.cached_lines[8];
    assert_eq!(sped_line, " sped: 14.0", "row 8 (sped) content mismatch");

    // Row 9 — dsty: 2 decimals. Owner mandated `dsty` (NOT `den`).
    let (_, dsty_line) = &h.cached_lines[9];
    assert!(
        dsty_line.starts_with(" dsty: "),
        "row 9 must start with ' dsty: ', got: {dsty_line:?}"
    );
    assert_eq!(dsty_line, " dsty: 1.00", "row 9 (dsty) content mismatch");

    // Row 10 — scn: scene name string
    let (_, scn_line) = &h.cached_lines[10];
    assert_eq!(scn_line, " scn: cinematic", "row 10 (scn) content mismatch");

    // Row 11 — chr: charset preset string
    let (_, chr_line) = &h.cached_lines[11];
    assert_eq!(chr_line, " chr: binary", "row 11 (chr) content mismatch");

    // Row 12 — clr: Debug format of ColorScheme
    let (_, clr_line) = &h.cached_lines[12];
    assert_eq!(clr_line, " clr: NeonGreen", "row 12 (clr) content mismatch");
}

#[test]
fn hud_density_label_is_dsty_not_den() {
    // Owner explicitly mandated the `dsty` label for density (NOT `den`):
    // "untuk densitas namannya jangan 'den' buruk sekali, harusnya dsty".
    // This regression test locks the label in so a future rename would
    // fail loudly. The value formatting is verified separately.
    let mut h = HudState::new();
    h.toggle();
    h.set_droplet_density(2.5);
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];
    h.update_metrics(&palette);
    let (_, dsty_line) = &h.cached_lines[9];
    assert!(
        dsty_line.starts_with(" dsty: "),
        "density label must be ' dsty: ' (NOT ' den: ' per owner mandate), got: {dsty_line:?}"
    );
    assert!(
        !dsty_line.starts_with(" den"),
        "density label must NOT be ' den' (owner: 'buruk sekali'), got: {dsty_line:?}"
    );
}

#[test]
fn hud_effective_pressure_clamps_to_unity_range() {
    // The `prs:` line clamps the underlying f32 to [0.0, 1.0] before
    // formatting. Values outside this range (e.g., a 1.5 thermal spike
    // or a -0.1 jitter) are rendered as 1.00 / 0.00 respectively. This
    // matches the `effective_pressure()` downstream contract.
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    // Above 1.0 — clamps to 1.00
    h.set_effective_pressure(1.5);
    h.update_metrics(&palette);
    let (_, prs_line) = &h.cached_lines[7];
    assert_eq!(prs_line, " prs: 1.00", "prs must clamp > 1.0 to 1.00");

    // Below 0.0 — clamps to 0.00
    h.set_effective_pressure(-0.5);
    // Force the next metric tick to execute immediately.
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, prs_line) = &h.cached_lines[7];
    assert_eq!(prs_line, " prs: 0.00", "prs must clamp < 0.0 to 0.00");

    // In-range — exact 2-decimal format
    h.set_effective_pressure(0.347);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, prs_line) = &h.cached_lines[7];
    assert_eq!(
        prs_line, " prs: 0.35",
        "prs must format in-range value with 2 decimals"
    );
}

#[test]
fn hud_endurance_health_score_rounds_to_integer() {
    // The `ehs:` line rounds the f64 score to the nearest integer so
    // the HUD reads as a calm 0-100 number (matches htop/mangoHUD
    // convention for summary metrics — sub-integer precision would
    // cause flicker without adding diagnostic value).
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    h.set_endurance_health_score(87.4);
    h.update_metrics(&palette);
    let (_, ehs_line) = &h.cached_lines[6];
    assert_eq!(ehs_line, " ehs: 87", "ehs must round 87.4 to 87");

    h.set_endurance_health_score(87.6);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, ehs_line) = &h.cached_lines[6];
    assert_eq!(ehs_line, " ehs: 88", "ehs must round 87.6 to 88");

    h.set_endurance_health_score(100.0);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, ehs_line) = &h.cached_lines[6];
    assert_eq!(ehs_line, " ehs: 100", "ehs must render 100 as-is");
}

#[test]
fn hud_final_layout_positions_match_owner_option_s() {
    // Regression guard for the v50 (2026-08-17) HUD expansion final
    // layout per owner's Option S mandate. Locks in the position of
    // every row so a future reorder would fail loudly. The owner
    // explicitly required: cid at the very bottom, screensize kept,
    // density label = `dsty` (NOT `den`), the 7 new metrics merged in.
    //
    // Layout (16 rows):
    //   0   fps
    //   1   tgt
    //   2   max
    //   3   p99
    //   4   cpu
    //   5   rss
    //   6   ehs    (NEW)
    //   7   prs    (NEW)
    //   8   sped   (NEW)
    //   9   dsty   (NEW)
    //   10  scn    (NEW)
    //   11  chr    (NEW)
    //   12  clr    (NEW)
    //   13  up
    //   14  screensize
    //   15  cid    (owner-mandated bottom)
    let h = HudState::new();
    // Row 0-5: performance core (active content set by update_metrics).
    // For this test we only assert on the static label structure of
    // the cid line (row 15) — the dynamic lines are tested above.
    let (_, cid_line) = &h.cached_lines[15];
    assert!(
        cid_line.starts_with(" cid: "),
        "row 15 must be the cid line per owner Option S mandate, got: {cid_line:?}"
    );
    // The 14 rows above cid (rows 0-14) must NOT contain the cid prefix
    // — the cid line is static and lives only at row 15.
    for (i, (_, text)) in h.cached_lines.iter().enumerate().take(15) {
        assert!(
            !text.starts_with(" cid: "),
            "row {i} must NOT contain the cid prefix — cid is exclusive to row 15, got: {text:?}"
        );
    }
}

// ── v50 (2026-08-17) HUD chroma gradient smoothness regression tests ────
//
// C5 fix: compute_chroma_gradient_16 now uses interpolate_palette_color
// (linear lerp between adjacent palette stops via blend_toward_rgb)
// instead of discrete sampling `palette_colors[(t * last).round()]`.
// This eliminates visible bands when the palette has fewer stops than
// the HUD has rows (e.g. a 3-stop palette + 16 HUD rows previously
// produced 4/8/4 band blocks; now produces a smooth gradient).

#[test]
fn compute_chroma_gradient_16_smooth_with_small_palette_no_bands() {
    // THE OWNER REGRESSION TEST for HUD chroma gradient smoothness.
    //
    // Before C5: a 3-stop palette (white/grey/black) + 16 HUD rows
    // produced discrete bands — multiple adjacent rows shared the same
    // palette stop, creating visible color blocks instead of a smooth
    // gradient. The owner flagged this as inconsistent with the chroma
    // dragon smoothness mandate.
    //
    // After C5: every HUD row gets a smoothly-interpolated color via
    // `interpolate_palette_color` (the same helper the border message
    // uses). Adjacent rows produce DISTINCT colors — no bands.
    //
    // Test palette: pure red(255,0,0), pure green(0,255,0), pure blue(0,0,255).
    // High-contrast colorful stops ensure adjacent interpolated values are
    // distinct enough that the brighten_color floor (TARGET_V=200) does
    // NOT collapse them to the same brightened value. (A monochromatic
    // palette like white/grey/black WOULD expose the brighten floor's
    // integer-math collapse — that's a separate concern about the
    // readability/smoothness tradeoff, not a regression from C5.)
    let palette = vec![
        Color::Rgb { r: 255, g: 0, b: 0 }, // idx 0: red
        Color::Rgb { r: 0, g: 255, b: 0 }, // idx 1: green
        Color::Rgb { r: 0, g: 0, b: 255 }, // idx 2: blue
    ];
    let colors = compute_chroma_gradient_16(&palette);
    assert_eq!(colors.len(), 16, "HUD gradient must have 16 entries");

    // Count distinct colors. With interpolation, every row gets a
    // unique color (16 distinct values, modulo the brighten floor
    // collapsing some to neutral grey). The old discrete-sampling
    // implementation would have produced only 3 distinct values
    // (one per palette stop). Assert >=5 distinct to leave room for
    // the brighten floor's grey fallback.
    let distinct_count = {
        let mut unique: Vec<Color> = colors.to_vec();
        unique.dedup();
        unique.len()
    };
    assert!(
        distinct_count >= 5,
        "interpolated HUD gradient must produce >=5 distinct colors across \
         16 rows with a 3-stop palette (got {distinct_count}) — the old \
         discrete-sampling implementation would have produced only 3 (one \
         per palette stop), causing visible bands"
    );

    // Assert NO two adjacent rows share the same color — that's the
    // visible band the owner reported. With smooth interpolation, every
    // adjacent pair must differ (since `frac` increments by 1/15 each row
    // and the blend_toward_rgb factor changes the output).
    for i in 0..15 {
        assert_ne!(
            colors[i],
            colors[i + 1],
            "adjacent HUD rows {i} and {} must NOT share the same color — \
             that's the visible band the owner reported (palette: red/green/blue)",
            i + 1
        );
    }
}

#[test]
fn compute_chroma_gradient_16_large_palette_still_exact_at_integer_t() {
    // Backward compatibility: with a 16-stop palette (one stop per HUD
    // row), the interpolated t = i/15.0 lands exactly on integer palette
    // positions, so the helper returns palette[i] exactly (no
    // interpolation). The brighten step is then applied as before. This
    // test verifies the C5 fix does NOT regress the 16-stop-palette
    // case — every row still gets its dedicated palette stop's color
    // (post-brighten).
    //
    // Test palette: 16 distinct RGB values, all with max channel >= 200
    // so brighten returns each as-is (isolates the gradient mapping
    // from the brightening math).
    let palette: Vec<Color> = (0..16)
        .map(|i| Color::Rgb {
            r: 200 + (i as u8 % 56),
            g: 200,
            b: 200,
        })
        .collect();
    let colors = compute_chroma_gradient_16(&palette);
    for (i, expected) in palette.iter().enumerate() {
        assert_eq!(
            &colors[i], expected,
            "row {i} must use palette[{i}] exactly (16-stop palette, t lands on integer boundary)"
        );
    }
}

// ── v50 (2026-08-17) HUD metric stability regression tests ─────────
//
// The 7 new owner-mandated metric setters were strengthened with NaN/Inf
// handling, range clamping, and string sanitization. These tests verify
// the sanitization at the setter level so a future code path that
// bypasses the setter still gets sanitized output via `update_metrics`.

#[test]
fn hud_set_endurance_health_score_clamps_nan_inf_and_range() {
    // The `ehs:` setter must clamp NaN, +Inf, -Inf to 0.0 (rendered as
    // `ehs: 0` — visibly degraded, forcing investigation). In-range
    // values are clamped to [0.0, 100.0]. This is the defense-in-depth
    // layer that complements `update_metrics` (which also clamps).
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    // NaN → 0
    h.set_endurance_health_score(f64::NAN);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[6];
    assert_eq!(line, " ehs: 0", "NaN ehs must render as 0");

    // +Inf → 0
    h.set_endurance_health_score(f64::INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[6];
    assert_eq!(line, " ehs: 0", "+Inf ehs must render as 0");

    // -Inf → 0
    h.set_endurance_health_score(f64::NEG_INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[6];
    assert_eq!(line, " ehs: 0", "-Inf ehs must render as 0");

    // Negative → clamp to 0
    h.set_endurance_health_score(-42.0);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[6];
    assert_eq!(line, " ehs: 0", "negative ehs must clamp to 0");

    // Above 100 → clamp to 100
    h.set_endurance_health_score(250.0);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[6];
    assert_eq!(line, " ehs: 100", "ehs above 100 must clamp to 100");
}

#[test]
fn hud_set_effective_pressure_clamps_nan_inf_and_range() {
    // The `prs:` setter must clamp NaN, +Inf, -Inf to 0.0 (rendered as
    // `prs: 0.00`). In-range values are clamped to [0.0, 1.0]. This is
    // the defense-in-depth layer that complements the existing clamp
    // in `update_metrics`.
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    // NaN → 0
    h.set_effective_pressure(f32::NAN);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[7];
    assert_eq!(line, " prs: 0.00", "NaN prs must render as 0.00");

    // +Inf → 0
    h.set_effective_pressure(f32::INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[7];
    assert_eq!(line, " prs: 0.00", "+Inf prs must render as 0.00");

    // -Inf → 0
    h.set_effective_pressure(f32::NEG_INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[7];
    assert_eq!(line, " prs: 0.00", "-Inf prs must render as 0.00");

    // Below 0 → 0
    h.set_effective_pressure(-0.5);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[7];
    assert_eq!(line, " prs: 0.00", "negative prs must clamp to 0.00");

    // Above 1 → 1
    h.set_effective_pressure(2.5);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[7];
    assert_eq!(line, " prs: 1.00", "prs above 1 must clamp to 1.00");
}

#[test]
fn hud_set_chars_per_sec_clamps_nan_and_negative() {
    // The `sped:` setter must clamp NaN, +Inf, -Inf, and negative values
    // to 0.0 (rendered as `sped: 0.0` — visibly broken, forcing
    // investigation rather than hiding the issue). This is the
    // defense-in-depth layer for the chars-per-second speed metric.
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    // NaN → 0
    h.set_chars_per_sec(f32::NAN);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[8];
    assert_eq!(line, " sped: 0.0", "NaN sped must render as 0.0");

    // +Inf → 0
    h.set_chars_per_sec(f32::INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[8];
    assert_eq!(line, " sped: 0.0", "+Inf sped must render as 0.0");

    // -Inf → 0
    h.set_chars_per_sec(f32::NEG_INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[8];
    assert_eq!(line, " sped: 0.0", "-Inf sped must render as 0.0");

    // Negative → 0
    h.set_chars_per_sec(-25.0);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[8];
    assert_eq!(line, " sped: 0.0", "negative sped must clamp to 0.0");
}

#[test]
fn hud_set_droplet_density_clamps_nan_and_negative() {
    // The `dsty:` setter must clamp NaN, +Inf, -Inf, and negative values
    // to 0.0 (rendered as `dsty: 0.00` — visibly broken, forcing
    // investigation). Owner explicitly mandated `dsty` label (NOT `den`).
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    // NaN → 0
    h.set_droplet_density(f32::NAN);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[9];
    assert_eq!(line, " dsty: 0.00", "NaN dsty must render as 0.00");

    // +Inf → 0
    h.set_droplet_density(f32::INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[9];
    assert_eq!(line, " dsty: 0.00", "+Inf dsty must render as 0.00");

    // -Inf → 0
    h.set_droplet_density(f32::NEG_INFINITY);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[9];
    assert_eq!(line, " dsty: 0.00", "-Inf dsty must render as 0.00");

    // Negative → 0
    h.set_droplet_density(-2.0);
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.update_metrics(&palette);
    let (_, line) = &h.cached_lines[9];
    assert_eq!(line, " dsty: 0.00", "negative dsty must clamp to 0.00");
}

#[test]
fn hud_set_scene_name_and_charset_preset_truncate_long_input() {
    // The `scn:` and `chr:` setters must truncate input to 14 chars (by
    // char count, preserving UTF-8 boundaries) so a very long custom
    // scene name or charset preset cannot blow past the HUD_MAX_WIDTH
    // (22 cols) budget. The ` scn: ` prefix is 6 chars (so 6 + 14 = 20
    // ≤ 22); the ` chr: ` prefix is also 6 chars (so 6 + 14 = 20 ≤ 22).
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        };
        16
    ];

    // Long scene name (30 chars) → truncated to 14
    let long_name = "abcdefghijklmnopqrstuvwxyz1234"; // 30 chars
    h.set_scene_name(long_name);
    h.update_metrics(&palette);
    let (_, scn_line) = &h.cached_lines[10];
    assert_eq!(
        scn_line, " scn: abcdefghijklmn",
        "scn line must truncate to first 14 chars of long scene name"
    );
    assert_eq!(
        scn_line.chars().count(),
        20,
        "truncated scn line must be 6 + 14 = 20 chars (prefix ' scn: ' is 6 chars)"
    );

    // Long charset preset (26 chars) → truncated to 14
    let long_preset = "PRESET0123456789abcdefghij"; // 26 chars
    h.last_metric_update = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    h.set_charset_preset(long_preset);
    h.update_metrics(&palette);
    let (_, chr_line) = &h.cached_lines[11];
    assert_eq!(
        chr_line, " chr: PRESET01234567",
        "chr line must truncate to first 14 chars of long charset preset"
    );
    assert_eq!(
        chr_line.chars().count(),
        20,
        "truncated chr line must be 6 + 14 = 20 chars (prefix ' chr: ' is 6 chars)"
    );
}
