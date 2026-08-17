// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
// Minimal full-redraw Matrix rain in Rust.
// Writes every cell every frame via ANSI escape sequences to stdout.
// Usage: BENCH_COLS=120 BENCH_LINES=40 BENCH_FRAMES=100 ./matrix_rust
use std::env;
use std::io::{self, Write};
use std::time::Instant;

// Simple LCG pseudo-random (no external crate needed)
struct Lcg(u64);
impl Lcg {
    fn new() -> Self { Lcg(0x123456789ABCDEF0) }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0
    }
    fn rand_bool(&mut self) -> bool { self.next() & 1 == 1 }
}

fn main() {
    let cols: usize = env::var("BENCH_COLS").ok().and_then(|s| s.parse().ok()).unwrap_or(120);
    let lines: usize = env::var("BENCH_LINES").ok().and_then(|s| s.parse().ok()).unwrap_or(40);
    let frames: usize = env::var("BENCH_FRAMES").ok().and_then(|s| s.parse().ok()).unwrap_or(100);

    let mut rng = Lcg::new();
    let mut buf = vec![' '; cols * lines];
    let mut heads: Vec<usize> = (0..cols).map(|_| (rng.next() as usize) % lines).collect();

    let start = Instant::now();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut total_bytes = 0usize;

    for _ in 0..frames {
        for c in 0..cols {
            heads[c] = (heads[c] + 1) % lines;
            buf[heads[c] * cols + c] = if rng.rand_bool() { '0' } else { '1' };
            if heads[c] > 0 { buf[(heads[c] - 1) * cols + c] = ' '; }
        }
        let mut s = String::with_capacity(cols * lines * 6 + 20);
        s.push_str("\x1b[H");
        for r in 0..lines {
            for c in 0..cols {
                let ch = buf[r * cols + c];
                if ch != ' ' { s.push_str("\x1b[32m"); s.push(ch); }
                else { s.push(' '); }
            }
            s.push('\n');
        }
        total_bytes += s.len();
        out.write_all(s.as_bytes()).unwrap();
        out.flush().unwrap();
    }

    let elapsed = start.elapsed().as_secs_f64();
    eprintln!("RUST: frames={} elapsed={:.3} fps={:.1} bytes={}", frames, elapsed, frames as f64 / elapsed, total_bytes);
}
