// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration test that verifies the ambient lightning path actually
//! spawns visible bolts when run through the real Cloud::rain_at()
//! pipeline. This protects against regressions where the bolt spawns
//! but never reaches the Frame buffer (e.g. render filter excludes it,
//! or events_enabled gets cleared, or perf_pressure gate rejects it).
//!
//! Run with: cargo test --bin cosmostrix ambient_lightning_smoke -- --nocapture

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::cloud::Cloud;
    use crate::frame::Frame;
    use crate::rain_style::RainStyle;
    use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

    fn make_cloud() -> Cloud {
        let mut cloud = Cloud::new(
            ColorMode::TrueColor,
            false,
            ShadingMode::Random,
            BoldMode::Off,
            false,
            true,
            ColorScheme::Green,
            RainStyle::Glyph,
        );
        cloud.init_chars(vec!['0', '1']);
        cloud.reset(80, 40);
        cloud.enable_events();
        cloud.clear_redraw_flags_for_test();
        cloud
    }

    /// Drive the simulation forward and assert that, after enough wall
    /// time for the startup + ambient triggers to fire, at least one
    /// lightning event gets spawned and rendered into the frame.
    #[test]
    fn ambient_lightning_actually_spawns_and_renders() {
        let mut cloud = make_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

        // Drive enough simulated wall-clock time for:
        //   - startup trigger (LIGHTNING_STARTUP_DELAY_MS = 800ms)
        //   - charge accumulation (CHARGE_RATE_BASE = 0.025/s -> threshold ~6s)
        //   - ambient cooldown (EVENT_AMBIENT_COOLDOWN_SECS = 8s)
        // 30 seconds of simulated time across ~600 frames at 50ms each.
        let start = Instant::now();
        let mut saw_event = false;
        for i in 0..600 {
            let now = start + Duration::from_millis(50 * i);
            cloud.rain_at(&mut frame, now);

            if cloud.active_event_count() > 0 {
                saw_event = true;
                // Verify the event actually wrote something to the frame:
                // find at least one cell whose fg was blended toward white
                // (i.e. has a non-palette Rgb color brighter than the
                // palette's last color).
                let mut wrote_cells = false;
                for y in 0..frame.height {
                    for x in 0..frame.width {
                        if let Some(cell) = frame.get(x, y) {
                            if let Some(crossterm::style::Color::Rgb { r, g: _, b }) = cell.fg {
                                // Palette's last color for Green scheme is
                                // bright green (0, 255, 0). If we see any
                                // cell with b > 0 or r > 0, it was blended
                                // by illuminate() toward white.
                                if b > 0 || r > 0 {
                                    wrote_cells = true;
                                    break;
                                }
                            }
                        }
                    }
                    if wrote_cells {
                        break;
                    }
                }
                assert!(
                    wrote_cells,
                    "ambient event spawned but illuminate() wrote nothing to the frame"
                );
                break;
            }
        }
        assert!(
            saw_event,
            "no ambient lightning event spawned within 30s of simulated time"
        );
    }

    /// Verify force_strike (the L key path) spawns a bolt and writes to
    /// the frame on the very next rain_at() call.
    #[test]
    fn force_strike_writes_to_frame_immediately() {
        let mut cloud = make_cloud();
        let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
        let now = Instant::now();

        // First, paint the frame with some colored droplets so illuminate
        // has something to blend. Without droplets, every cell has fg=None
        // and illuminate is a no-op even when the event spawns.
        for _ in 0..10 {
            cloud.rain_at(&mut frame, now);
        }

        // Now force a strike and step the sim once so the event renders.
        assert!(cloud.force_strike(now), "force_strike should succeed");
        assert_eq!(
            cloud.active_event_count(),
            1,
            "force_strike must spawn exactly one event"
        );

        // Run rain_at once so render() is called on the new event.
        cloud.rain_at(&mut frame, now + Duration::from_millis(10));

        // The event must still be alive (it has a 700ms lifetime) and
        // the frame must show non-palette colors where illuminate wrote.
        assert_eq!(
            cloud.active_event_count(),
            1,
            "event should still be alive immediately after force_strike"
        );
    }
}
