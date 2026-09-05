// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-beta.1 pause-freeze tests (owner bug fix 2026-08-30).
//!
//! Owner report: "when I pause cosmostrix, some HUD metrics keep
//! running — time, fps, p99, etc. They should stop, and when I resume
//! they must continue with precision."
//!
//! Contract under test (see `metrics.rs` module docs):
//! - `set_metrics_paused(true)` freezes every running metric: uptime
//!   (`up:`), fps/max/p99 (frame-time window), cpu, rss, prs, ehs.
//! - The freeze window matches `cloud.is_paused_or_decelerating()`
//!   (same predicate as the keybinding pause guard).
//! - On resume, uptime continues from EXACTLY where it froze (paused
//!   time is excluded via `paused_total` + the open segment), and the
//!   CPU baseline stayed warm so the first post-resume delta is
//!   computed over a normal ~1 s window, not the whole pause span.

use std::time::Duration;

use super::*;

/// Pausing must freeze the frame-time window: paused pushes are dropped
/// so fps/max/p99 hold their last active values.
#[test]
fn pause_freeze_blocks_frame_time_pushes() {
    let mut h = HudState::new();
    h.toggle(); // visible
    h.push_frame_time(2.0);
    assert_eq!(h.max_ms, 2.0);

    h.set_metrics_paused(true);
    h.push_frame_time(50.0); // paused 4 Hz poll tick — must be dropped
    assert_eq!(
        h.max_ms, 2.0,
        "paused frames must not contaminate max/fps/p99 windows"
    );

    h.set_metrics_paused(false);
    h.push_frame_time(3.0);
    assert_eq!(h.max_ms, 3.0, "resume must re-open the frame-time window");
}

/// Pausing must freeze prs and ehs at their last active values.
#[test]
fn pause_freeze_holds_pressure_and_endurance() {
    let mut h = HudState::new();
    h.set_effective_pressure(0.42);
    h.set_endurance_health_score(87.0);

    h.set_metrics_paused(true);
    h.set_effective_pressure(0.99);
    h.set_endurance_health_score(1.0);
    assert_eq!(h.effective_pressure, 0.42, "prs must hold while paused");
    assert_eq!(h.endurance_health_score, 87.0, "ehs must hold while paused");

    h.set_metrics_paused(false);
    h.set_effective_pressure(0.10);
    h.set_endurance_health_score(95.0);
    assert_eq!(h.effective_pressure, 0.10, "prs must resume after unpause");
    assert_eq!(
        h.endurance_health_score, 95.0,
        "ehs must resume after unpause"
    );
}

/// Repeated announcements of the same state must be no-ops — the event
/// loop calls `set_metrics_paused` EVERY frame, so a duplicate `true`
/// must not re-open the pause segment (which would lose the accumulated
/// paused time and let uptime drift).
#[test]
fn pause_freeze_duplicate_announcements_are_noops() {
    let mut h = HudState::new();
    h.set_metrics_paused(true);
    std::thread::sleep(Duration::from_millis(30));
    h.set_metrics_paused(true); // same state — must not re-stamp
    h.set_metrics_paused(true);
    std::thread::sleep(Duration::from_millis(30));
    h.set_metrics_paused(false);
    // Total paused time ≈ 60 ms (not ~30 ms if a duplicate had re-stamped).
    assert!(
        h.paused_total >= Duration::from_millis(50),
        "duplicate pause announcements must not reset the segment start"
    );
}

/// The uptime math must exclude paused time: `up:` pins at its
/// freeze-time value while paused and continues from exactly there
/// after resume (sub-second precision).
#[test]
fn pause_freeze_uptime_excludes_paused_time() {
    let mut h = HudState::new();
    h.toggle(); // visible
                // Backdate the session by 120 s: uptime should read "02:00".
    h.session_start = Instant::now()
        .checked_sub(Duration::from_secs(120))
        .expect("checked_sub on a 120 s backdate must succeed");
    h.last_metric_update = Instant::now()
        .checked_sub(HUD_METRIC_INTERVAL)
        .unwrap_or_else(Instant::now);

    h.update_metrics(&[]);
    assert_eq!(h.cached_lines[23].1, " up: 02:00");

    // Pause for a moment: uptime must NOT advance.
    h.set_metrics_paused(true);
    std::thread::sleep(Duration::from_millis(60));
    h.last_metric_update = Instant::now()
        .checked_sub(HUD_METRIC_INTERVAL)
        .unwrap_or_else(Instant::now);
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[23].1, " up: 02:00",
        "up: must be frozen while paused"
    );

    // Resume: uptime continues from exactly where it froze (the 60 ms
    // paused span is excluded; the whole-second display still reads
    // 02:00 — precision is preserved, not truncated away).
    h.set_metrics_paused(false);
    std::thread::sleep(Duration::from_millis(20));
    h.last_metric_update = Instant::now()
        .checked_sub(HUD_METRIC_INTERVAL)
        .unwrap_or_else(Instant::now);
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[23].1, " up: 02:00",
        "up: must resume exactly where it froze (paused span excluded)"
    );
    assert!(
        h.paused_total >= Duration::from_millis(50),
        "the paused span must be accumulated into paused_total"
    );
}

/// A pause→resume→pause cycle accumulates BOTH spans into
/// `paused_total` (the open-segment state machine closes and re-opens
/// correctly across cycles).
#[test]
fn pause_freeze_accumulates_across_cycles() {
    let mut h = HudState::new();
    h.set_metrics_paused(true);
    std::thread::sleep(Duration::from_millis(30));
    h.set_metrics_paused(false);
    let first = h.paused_total;
    assert!(first >= Duration::from_millis(20));

    h.set_metrics_paused(true);
    std::thread::sleep(Duration::from_millis(30));
    h.set_metrics_paused(false);
    assert!(
        h.paused_total >= first + Duration::from_millis(20),
        "second pause span must add to the first"
    );
    assert!(
        h.pause_started_at.is_none(),
        "segment must be closed when running"
    );
}

/// While paused, the CPU sampler keeps the baseline warm but does not
/// touch the displayed percent. (The baseline tick is what makes the
/// first post-resume delta precise — see maybe_sample_cpu docs.)
/// On non-unix targets the sampler is unsupported and stays None; the
/// freeze contract then trivially holds, so only assert the unix path.
#[cfg(unix)]
#[test]
fn pause_freeze_cpu_sampler_holds_percent_and_warms_baseline() {
    let mut h = HudState::new();
    h.toggle(); // visible
    h.cpu_percent = Some(12.5);

    h.set_metrics_paused(true);
    h.maybe_sample_cpu();
    // Rate limiter: last_cpu_sample was backdated in new(), so this
    // call runs the paused branch — baseline advances, percent holds.
    assert_eq!(h.cpu_percent, Some(12.5), "cpu% must hold while paused");
    assert!(
        h.last_cpu_ns.is_some(),
        "baseline must stay warm while paused"
    );
}

/// The `tgt:` line must stay LIVE during pause — its ` paused` suffix
/// is the only HUD element that keeps updating, so the user sees why
/// the dashboard froze.
#[test]
fn pause_freeze_tgt_suffix_still_renders() {
    let mut h = HudState::new();
    h.toggle();
    h.set_target_fps(30.0);

    h.set_metrics_paused(true);
    h.set_frame_mode(FrameMode::Paused);
    h.last_metric_update = Instant::now()
        .checked_sub(HUD_METRIC_INTERVAL)
        .unwrap_or_else(Instant::now);
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[1].1, " tgt: 30.0 paused",
        "the paused suffix must render while metrics are frozen"
    );
}

/// v80.0.0-alpha.1 S-master-HUNT-5: the `up:` row must render the
/// tiered formatter (minutes survive the 1h boundary, `m` suffix).
///
/// `Instant` on Linux/macOS is bounded by the machine's monotonic
/// uptime (zero point ≈ boot), so a 3661s backdate only succeeds when
/// the host has been up longer than that. Fresh CI runners boot under
/// an hour and hit the fallback branch: there we assert the row is
/// well-formed tier-0 text (the full tier ladder — days/months/years —
/// is locked by the exhaustive `clock::format_uptime_tiered` tests,
/// which need no Instant and cover every boundary).
#[test]
fn uptime_row_uses_tiered_formatter() {
    let mut h = HudState::new();
    h.toggle();
    let backdate = Duration::from_secs(3661); // 1h 01m 01s
    if let Some(then) = Instant::now().checked_sub(backdate) {
        h.session_start = then;
        h.last_metric_update = Instant::now()
            .checked_sub(HUD_METRIC_INTERVAL)
            .unwrap_or_else(Instant::now);
        h.update_metrics(&[]);
        assert_eq!(
            h.cached_lines[23].1, " up: 1h:01m",
            "tier-1 row must show the m suffix and whole-minute truncation"
        );
    } else {
        h.update_metrics(&[]);
        let row = h.cached_lines[23].1.clone();
        assert!(
            row.starts_with(" up: ") && row.chars().count() <= 10,
            "clock range too short for tier-1 backdate — tier-0 fallback must be well-formed: {row:?}"
        );
    }
}

/// S-master-HUNT-23: the `tgt:` line must render the ` drain` suffix when
/// the event loop announces the output-drain backoff frame mode — the
/// terminal's write latency exceeds its frame budget and the cadence is
/// scaled toward the drain rate. Without the suffix, a user on VTE/foot
/// would see `tgt: 36` with no explanation, exactly the confusion the
/// idle suffix was introduced to fix (v30).
#[test]
fn hunts23_tgt_suffix_renders_drain_mode() {
    let mut h = HudState::new();
    h.toggle();
    h.set_target_fps(36.0);

    h.set_frame_mode(FrameMode::Drain);
    h.last_metric_update = Instant::now()
        .checked_sub(HUD_METRIC_INTERVAL)
        .unwrap_or_else(Instant::now);
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[1].1, " tgt: 36.0 drain",
        "the drain suffix must render while the backoff is engaged"
    );
}
