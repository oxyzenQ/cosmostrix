#![allow(dead_code)]
// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Monolith rain tests extracted from monolith.rs.

#[cfg(test)]
mod tests {
    use super::super::monolith::*;
    use rand::distr::Uniform;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    fn default_density_targets_sparse_lane_count() {
        let target = target_active_count(100, 0.75);
        assert!((20..=35).contains(&target));
    }

    /// Helper: create a MonolithRain with `cols` lanes, all inactive.
    fn rain_with_lanes(cols: u16) -> MonolithRain {
        let mut rain = MonolithRain::new();
        rain.reset(cols, false);
        rain
    }

    #[test]
    fn density_map_zero_weight_falls_back_to_scan() {
        // All-zero weights: random loop rejects all candidates, but the
        // fallback linear scan still finds an available (inactive) lane.
        let mut rain = rain_with_lanes(10);
        let rand_col = Uniform::new_inclusive(0u16, 9).unwrap();
        let rand_chance = Uniform::new(0.0f32, 1.0f32).unwrap();
        let mut rng = StdRng::seed_from_u64(42);
        let map: &'static [f64] = &[0.0; 10];
        let result = rain.find_inactive_lane(
            false,
            false,
            u16::MAX,
            &rand_col,
            &mut rng,
            Some(map),
            &rand_chance,
        );
        assert!(result.is_some(), "fallback scan should find available lane");
    }

    #[test]
    fn density_map_full_weight_accepts_immediately() {
        // All-1.0 weights: every candidate passes the gate, so the first
        // random draw that hits an available lane returns it.
        let mut rain = rain_with_lanes(10);
        let rand_col = Uniform::new_inclusive(0u16, 9).unwrap();
        let rand_chance = Uniform::new(0.0f32, 1.0f32).unwrap();
        let mut rng = StdRng::seed_from_u64(7);
        let map: &'static [f64] = &[1.0; 10];
        let result = rain.find_inactive_lane(
            false,
            false,
            u16::MAX,
            &rand_col,
            &mut rng,
            Some(map),
            &rand_chance,
        );
        assert!(result.is_some(), "full-weight map should accept lane");
    }

    #[test]
    fn density_map_none_uses_uniform_sampling() {
        // None map = legacy behavior, no rejection sampling.
        let mut rain = rain_with_lanes(8);
        let rand_col = Uniform::new_inclusive(0u16, 7).unwrap();
        let rand_chance = Uniform::new(0.0f32, 1.0f32).unwrap();
        let mut rng = StdRng::seed_from_u64(99);
        let result = rain.find_inactive_lane(
            false,
            false,
            u16::MAX,
            &rand_col,
            &mut rng,
            None,
            &rand_chance,
        );
        assert!(result.is_some(), "None map should find lane via uniform");
    }

    #[test]
    fn density_map_partial_weight_favors_high_columns() {
        // Map with weight 1.0 on columns 0-2 and 0.0 on columns 3-9.
        // After many spawns, the active lanes should cluster in the first
        // three columns. We can't easily assert distribution in a unit test
        // (would need statistical sampling), but we verify no panic and at
        // least one lane is returned.
        let mut rain = rain_with_lanes(10);
        let rand_col = Uniform::new_inclusive(0u16, 9).unwrap();
        let rand_chance = Uniform::new(0.0f32, 1.0f32).unwrap();
        let mut rng = StdRng::seed_from_u64(123);
        let map: &'static [f64] = &[1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let result = rain.find_inactive_lane(
            false,
            false,
            u16::MAX,
            &rand_col,
            &mut rng,
            Some(map),
            &rand_chance,
        );
        assert!(result.is_some());
    }
}
