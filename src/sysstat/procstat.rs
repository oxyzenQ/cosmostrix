// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Linux `/proc` process metrics for the HUD endurance panel.
//!
//! `read_self_rss_kb` and `read_self_voluntary_ctxt` are lightweight
//! `/proc` readers sampled at 1 Hz by `interactive/event_loop_p5.rs`
//! (rss row + context-switch rate). They lived in
//! `interactive/intro.rs` since v17 only because that file already
//! existed — the v52 intro_style refactor split the two unrelated
//! concerns and moved these helpers home to the stats subsystem.

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
    // v50 audit C-4: use .nth(17) instead of collecting into Vec (saves
    // one heap allocation per call at 1 Hz cadence).
    after_paren
        .split_whitespace()
        .nth(17)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}
