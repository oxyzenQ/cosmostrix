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
fn hud_has_nine_cached_lines_after_v50_cid_addition() {
    // Regression guard: the HUD must have exactly 9 cached lines
    // after the v50 addition of the `cid:` (commit id) line at
    // index [8]. The previous count was 8 (fps / tgt / max / p99 /
    // cpu / rss / up / screensize post-v50 intra-pair reorder); now 9
    // (adds cid at row 8).
    // The cid line shares the head color stop with screensize.
    // If a future change adds or removes a line, this test will
    // catch it.
    let h = HudState::new();
    assert_eq!(
        h.cached_lines.len(),
        9,
        "HUD must have 9 cached lines after the v50 cid-line addition"
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
    // HD-01: 9-stop chroma gradient sweep. The BOTTOM row (cid, idx 8)
    // and the second-bottom row (screensize, idx 7) share the brightest
    // `head` color (palette last stop), and the TOP row (fps, idx 0) is
    // the dimmest `dim` color (palette first stop, brightened to readable
    // floor). This inverts the original mapping where fps/tgt/max were
    // brightest at the top — the owner explicitly flagged the inversion:
    // 'rain tail is dim head is white' (head leads at the bottom of a
    // falling stream).
    //
    // v50: bumped from 8 → 9 stops after adding the `cid:` line at row 8.
    // The cid line shares the head stop with screensize — both are
    // "definitive identity" lines the owner reads to verify the build.
    //
    // We use a palette where head is pure white RGB(255,255,255) so the
    // assertion is unambiguous, and dim is a dark green RGB(0,50,0)
    // that brightens to RGB(0,200,0). The bottom two rows must be white
    // (head); the top row must be green (dim).
    let mut h = HudState::new();
    h.toggle();
    let palette = vec![
        Color::Rgb { r: 0, g: 50, b: 0 }, // idx 0 → line 0 (dim, brightened)
        Color::Rgb { r: 0, g: 80, b: 0 }, // idx 1 → line 1
        Color::Rgb { r: 0, g: 110, b: 0 }, // idx 2 → line 2
        Color::Rgb { r: 0, g: 140, b: 0 }, // idx 3 → line 3
        Color::Rgb {
            r: 100,
            g: 180,
            b: 0,
        }, // idx 4 → line 4
        Color::Rgb {
            r: 150,
            g: 210,
            b: 50,
        }, // idx 5 → line 5
        Color::Rgb {
            r: 200,
            g: 230,
            b: 100,
        }, // idx 6 → line 6
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 7 → line 7 (screensize, head, white)
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 8 → line 8 (cid, head, white — shares stop with screensize)
    ];
    h.refresh_colors(&palette);
    // Top row (fps, idx 0) = palette[0] = RGB(0, 50, 0) brightened to RGB(0, 200, 0)
    assert_eq!(
        h.cached_lines[0].0,
        Color::Rgb { r: 0, g: 200, b: 0 },
        "top row (fps) must use palette[0] (brightened dim) — rain tail at top"
    );
    // Row 7 (screensize, idx 7) = palette[7] = RGB(255, 255, 255)
    assert_eq!(
        h.cached_lines[7].0,
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        },
        "row 7 (screensize) must use palette[7] (head) — rain head near bottom"
    );
    // Bottom row (cid, idx 8) = palette[8] = RGB(255, 255, 255) — shares head stop
    assert_eq!(
        h.cached_lines[8].0,
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        },
        "bottom row (cid) must use palette[8] (head) — shares head stop with screensize"
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
    let mut h = HudState::new();
    h.toggle();
    let green_palette = vec![
        Color::Rgb { r: 0, g: 50, b: 0 },
        Color::Rgb { r: 0, g: 50, b: 0 },
        Color::Rgb { r: 0, g: 255, b: 0 },
    ];
    h.refresh_colors(&green_palette);
    assert_eq!(
        h.cached_lines[7].0,
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
        h.cached_lines[7].0,
        Color::Rgb {
            r: 255,
            g: 176,
            b: 0
        },
        "second refresh (immediate): bottom = amber head"
    );
}

#[test]
fn refresh_colors_gradient_uses_nine_distinct_stops() {
    // HD-01: 9 HUD rows now use 9 distinct palette stops (one per line),
    // sweeping the full chroma dragon gradient top→bottom.
    // v50: bumped from 8 → 9 stops after adding the `cid:` line at row 8.
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
            r: 255,
            g: 255,
            b: 255,
        },
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 8 → line 8 (cid, shares head stop with screensize)
    ];
    h.refresh_colors(&palette);
    for (i, expected) in palette.iter().enumerate() {
        assert_eq!(
            &h.cached_lines[i].0, expected,
            "line {i} must use palette[{i}]"
        );
    }
}

#[test]
fn hud_cid_line_contains_commit_sha_or_unknown() {
    // v50: the cid line at index [8] must contain the compile-time
    // git short SHA injected by build.rs via `COSMOSTRIX_GIT_SHA`,
    // falling back to "unknown" when the build had no .git dir.
    // The text is set once in `new()` and never mutated — the owner
    // needs to read the commit hash without quitting cosmostrix.
    let h = HudState::new();
    let (_, cid_line) = &h.cached_lines[8];
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
fn compute_chroma_gradient_9_sweeps_full_palette_range() {
    // HD-01 regression: verify the 9-stop chroma gradient helper maps
    // each of the 9 HUD lines to its corresponding palette stop.
    // Line i samples palette[(i / 8.0 * (n-1)).round()].
    let palette = vec![
        Color::Rgb { r: 50, g: 0, b: 0 }, // idx 0 → line 0
        Color::Rgb { r: 0, g: 50, b: 0 }, // idx 1 → line 1
        Color::Rgb { r: 0, g: 0, b: 50 }, // idx 2 → line 2
        Color::Rgb {
            r: 100,
            g: 100,
            b: 0,
        }, // idx 3 → line 3
        Color::Rgb {
            r: 0,
            g: 100,
            b: 100,
        }, // idx 4 → line 4
        Color::Rgb {
            r: 100,
            g: 0,
            b: 100,
        }, // idx 5 → line 5
        Color::Rgb {
            r: 200,
            g: 100,
            b: 50,
        }, // idx 6 → line 6
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 7 → line 7
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 8 → line 8 (cid, shares head stop with screensize)
    ];
    let colors = compute_chroma_gradient_9(&palette);
    // Line 0 = palette[0] = RGB(50,0,0) brightened to RGB(200,0,0)
    assert_eq!(colors[0], Color::Rgb { r: 200, g: 0, b: 0 });
    // Line 1 = palette[1] = RGB(0,50,0) brightened to RGB(0,200,0)
    assert_eq!(colors[1], Color::Rgb { r: 0, g: 200, b: 0 });
    // Line 7 = palette[7] = RGB(255,255,255) (already bright, returned as-is)
    assert_eq!(
        colors[7],
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        }
    );
    // Line 8 = palette[8] = RGB(255,255,255) — cid shares head stop
    assert_eq!(
        colors[8],
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        },
        "cid line (idx 8) must use the head stop — same as screensize"
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
