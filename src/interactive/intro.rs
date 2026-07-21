// SPDX-License-Identifier: GPL-3.0-only

//! v17: Dragon Render intro animation + Linux process metrics helpers.
//! Extracted from event_loop.rs to keep that file under 1K LOC.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crossterm::event::Event;

use crate::frame::Frame;
use crate::terminal::Terminal;

use super::watchdog::{FRAME_COUNTER, GRACEFUL_SHUTDOWN};
use crate::cloud::Cloud;

/// Read this process's current RSS from `/proc/self/status` (Linux only).
#[cfg(target_os = "linux")]
pub(crate) fn read_self_rss_kb() -> u64 {
    // Read VmRSS from /proc/self/status. Lightweight: single line match.
    use std::io::Read;
    let mut file = match std::fs::File::open("/proc/self/status") {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let mut buf = [0u8; 8192];
    let n = file.read(&mut buf).unwrap_or(0);
    let text = std::str::from_utf8(&buf[..n]).unwrap_or("");
    for line in text.split('\n') {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let trimmed = rest.trim();
            let digits_end = trimmed
                .bytes()
                .position(|b| !b.is_ascii_digit())
                .unwrap_or(trimmed.len());
            if digits_end > 0 {
                return trimmed[..digits_end].parse().unwrap_or(0);
            }
        }
    }
    0
}

/// Read voluntary context switches from `/proc/self/stat` (Linux only).
#[cfg(target_os = "linux")]
pub(crate) fn read_self_voluntary_ctxt() -> u64 {
    let stat = match std::fs::read_to_string("/proc/self/stat") {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let after_paren = match stat.rfind(')') {
        Some(idx) => &stat[idx + 1..],
        None => return 0,
    };
    let fields: Vec<&str> = after_paren.split_whitespace().collect();
    fields.get(17).and_then(|s| s.parse().ok()).unwrap_or(0)
}

/// v17: Dragon Render intro. Types "COSMOSTRIX" at center, fades out.
/// Skip with any key. Reuses Frame+Terminal. Increments FRAME_COUNTER
/// so the watchdog doesn't kill the process during the 1.5s animation.
pub(crate) fn run_intro(
    term: &mut Terminal,
    frame: &mut Frame,
    cloud: &Cloud,
    w: u16,
    h: u16,
) -> std::io::Result<()> {
    let text = "COSMOSTRIX";
    let chars: Vec<char> = text.chars().collect();
    let start_row = h / 2;
    let start_col = w.saturating_sub(chars.len() as u16) / 2;
    let color = cloud
        .palette
        .colors
        .last()
        .copied()
        .unwrap_or(crossterm::style::Color::White);
    let bg = cloud.palette.bg;
    let intro_start = Instant::now();
    let duration_ms = 1500u64;
    let frame_period = Duration::from_millis(33); // ~30 FPS

    loop {
        let elapsed = intro_start.elapsed().as_millis() as u64;
        if elapsed >= duration_ms {
            break;
        }
        if GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
            break;
        }
        while Terminal::poll_event(Duration::from_millis(0))? {
            if let Ok(Event::Key(_)) = Terminal::read_event() {
                return Ok(());
            }
        }
        let visible = if elapsed < 800 {
            ((elapsed as f32 / 800.0) * chars.len() as f32).ceil() as usize
        } else if elapsed < 1200 {
            chars.len()
        } else {
            let fade = (elapsed - 1200) as f32 / 300.0;
            chars
                .len()
                .saturating_sub((fade * chars.len() as f32) as usize)
        };
        frame.clear_with_bg(bg);
        for (i, &ch) in chars.iter().enumerate().take(visible) {
            let x = start_col + i as u16;
            if x < w {
                frame.set_force(
                    x,
                    start_row,
                    crate::cell::Cell {
                        ch,
                        fg: Some(color),
                        bg,
                        bold: true,
                    },
                );
            }
        }
        term.draw(frame)?;
        FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::thread::sleep(frame_period);
    }
    Ok(())
}
