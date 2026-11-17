// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! NIGHT-depth-hardening-2 determinism regression tests: the frame
//! carries zero OS entropy. Three former entropy leaks - the wall-clock
//! reseed ladder, the ghost glyph/duration draw, and the first gust
//! idle window - must replay bit-for-bit across independent runs, or
//! every "same seeds -> same picture" claim in the docs goes stale.

use std::time::{Duration, Instant};

use rand::distr::Distribution;
use rand::{SeedableRng, rngs::StdRng};

use super::make_cloud;
use crate::cloud::events::ghost::GhostEvent;
use crate::cloud::living_rain::GustState;

#[test]
fn reseed_ladder_replays_regardless_of_wall_clock() {
    // Two independent sessions at two arbitrary wall-clock instants.
    // Pre-hardening the reseed drew its seed from wall-clock nanos, so
    // these two streams diverged after RNG_RESEED_INTERVAL_SECS.
    let mut session_a = make_cloud();
    let mut session_b = make_cloud();

    let arbitrary_a = Instant::now();
    let arbitrary_b = arbitrary_a + Duration::from_millis(4_242);
    // Force both past the reseed boundary (600 s default interval).
    session_a.last_reseed_time = arbitrary_a - Duration::from_secs(1_000);
    session_b.last_reseed_time = arbitrary_b - Duration::from_secs(1_000);

    session_a.maybe_reseed_rng(arbitrary_a);
    session_b.maybe_reseed_rng(arbitrary_b);

    // Same counter, same ladder step -> identical post-reseed streams.
    let stream_a: Vec<f32> = (0..32)
        .map(|_| session_a.rand_chance.sample(&mut session_a.mt))
        .collect();
    let stream_b: Vec<f32> = (0..32)
        .map(|_| session_b.rand_chance.sample(&mut session_b.mt))
        .collect();
    assert_eq!(
        stream_a, stream_b,
        "the reseed ladder must be wall-clock independent"
    );

    // The ladder must also land on a FRESH stream, not re-seed back to
    // the launch seed (otherwise every 600 s the picture would restart).
    let launch_stream: Vec<f32> = (0..32)
        .map(|_| session_a.rand_chance.sample(&mut StdRng::seed_from_u64(0)))
        .collect();
    assert_ne!(
        stream_a, launch_stream,
        "the first ladder rung must be a fresh stream, not the launch seed"
    );
    assert_eq!(session_a.reseed_counter, 1, "the counter tracks reseeds");
}

#[test]
fn ghost_content_replays_from_the_event_stream() {
    // Ghost glyph and duration come from the scheduler's seeded stream,
    // not ThreadRng: two identically-seeded draws must agree.
    let now = Instant::now();
    for seed in [0xC0DE_C0FFEEu64, 0x1234_5678, 0xDEAD_BEEF] {
        let mut rng_a = StdRng::seed_from_u64(seed);
        let mut rng_b = StdRng::seed_from_u64(seed);
        let ghost_a = GhostEvent::new(10, 5, now, &mut rng_a);
        let ghost_b = GhostEvent::new(10, 5, now, &mut rng_b);
        assert_eq!(
            ghost_a.test_glyph(),
            ghost_b.test_glyph(),
            "ghost glyph must replay from the seeded stream"
        );
        assert_eq!(
            ghost_a.test_duration(),
            ghost_b.test_duration(),
            "ghost duration must replay from the seeded stream"
        );
    }
}

#[test]
fn gust_first_idle_window_replays() {
    // The launch-time gust idle window was the one unseeded draw in the
    // gust machine; it must now replay identically on every session.
    let now_a = Instant::now();
    let now_b = now_a + Duration::from_millis(97);
    let gust_a = GustState::new(now_a);
    let gust_b = GustState::new(now_b);
    assert_eq!(
        gust_a.first_idle_duration(),
        gust_b.first_idle_duration(),
        "the first gust idle window must be deterministic"
    );
}
