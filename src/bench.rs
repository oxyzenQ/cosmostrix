// Copyright (c) 2026 rezky_nightky

use std::env;
use std::time::{Duration, Instant};

use crate::constants::{
    BENCH_ELAPSED_MIN_S, DENSITY_AUTO_DEFAULT_COLS, DENSITY_AUTO_DEFAULT_LINES,
    MAX_TERMINAL_COLS, MAX_TERMINAL_LINES,
};
use crate::frame::Frame;

use super::{effective_density, CloudConfig};

pub fn run_benchmark(cfg: &CloudConfig) -> std::io::Result<()> {
    let bench_frames = cfg.bench_frames.expect("bench_frames must be set");

    let (w, h) = (
        env::var("COSMOSTRIX_BENCH_COLS")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(DENSITY_AUTO_DEFAULT_COLS)
            .min(MAX_TERMINAL_COLS),
        env::var("COSMOSTRIX_BENCH_LINES")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(DENSITY_AUTO_DEFAULT_LINES)
            .min(MAX_TERMINAL_LINES),
    );

    let density = effective_density(cfg.base_density, w, h, cfg.fullwidth, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);

    let mut frame = Frame::new(w, h, cloud.palette.bg);

    let target_period = Duration::from_secs_f64(1.0 / cfg.target_fps);
    cloud.set_max_sim_delta(target_period);

    let warmup_frames = (bench_frames / 10).clamp(10, 200);
    let mut sim_now = Instant::now();

    for _ in 0..warmup_frames {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        frame.clear_dirty();
    }

    let start = Instant::now();
    for _ in 0..bench_frames {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        frame.clear_dirty();
    }
    let elapsed_s = start.elapsed().as_secs_f64().max(BENCH_ELAPSED_MIN_S);
    let fps = (bench_frames as f64) / elapsed_s;

    println!("BENCH:");
    println!("  cols: {}", w);
    println!("  lines: {}", h);
    println!("  frames: {}", bench_frames);
    println!("  elapsed_s: {:.6}", elapsed_s);
    println!("  frames_per_s: {:.3}", fps);
    Ok(())
}
