// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! HUD initialization + panel frame writing — extracted from
//! `hud/mod.rs` to keep that file under the 800-LOC cap.
//!
//! v80.0.0-beta.3 (branch `hud-scifi-dashboard`, owner decision D + B
//! from `docs/research/HUD_LAYOUT_MASTERCLASS_RESEARCH.md`): the HUD
//! renders as a bottom-center **sci-fi panel grid** — a rounded
//! `╭╮╰╯` frame (Option B layout) carrying the complete-rounded-frame
//! finish + tail accent (Option D style). All 24 owner-mandated
//! metrics are preserved; only the geometry and the visual language
//! changed. See the research doc for the decision trail and the
//! trade-offs accepted (fixed width, bottom-center placement).

use std::time::{Duration, Instant};

use crate::runtime::ColorScheme;
use crossterm::style::Color;

use super::{DirtyCellTracker, FrameMode, HUD_CPU_INTERVAL, HUD_METRIC_INTERVAL, HUD_RSS_INTERVAL};
use crate::interactive::activity::FrameTimeTracker;

// ── v80.0.0-beta.3 panel geometry (Option B, X-1 fixed width) ───────────
//
// The panel is a FIXED-SIZE rectangle anchored bottom-center. Fixed
// geometry is the X-1 mandate from the research doc: a dynamic-width
// center-anchored panel re-dirties its whole footprint every time a
// 1 Hz value-length change shifts the center — visible horizontal
// jitter plus a full-block dirty re-send. A fixed footprint means the
// anchor never moves, the center never moves, and the dirty-cell
// economy (INV-4) keeps steady-state cost near zero: `frame.set`
// short-circuits on content equality, so only genuinely changing
// cells (uptime seconds, fps) get re-sent.

/// Total panel width in terminal columns, INCLUDING the two border
/// columns (`╭`/`│`/`╰` on the left, `╮`/`│`/`╯` on the right).
///
/// Derived: 3 grid cells × [`HUD_GRID_CELL_W`] (14) + 2 one-column
/// gutters between cells + 2 border columns = 3×14 + 2 + 2 = 46.
/// 46 fits the 80-column minimum terminal (INV-8) with 17 columns of
/// margin on each side at 80 cols.
pub(super) const HUD_PANEL_WIDTH: u16 = 46;

/// Interior width (between the border columns) = 44 = 3×14 + 2 gutters.
pub(super) const HUD_PANEL_INNER: u16 = 44;

/// Fixed width of one grid cell (a metric's `label: value` text,
/// padded/truncated to exactly this many columns at the 1 Hz
/// composition tick).
///
/// 14 fits every realistic metric value exactly:
/// `scn: cinematic` (14), `max: 999.999ms` (14), `rss: 1023.9MiB` (14),
/// `cpu: 999.99%` (13), `glth: default` (13), `cid: 6ed244b` (12).
/// Pathological caps (`p99: 9999.999ms` = 15, a 14-char custom preset
/// under `chr:` = 19) truncate — the honest fixed-width trade-off the
/// owner accepted with Option B. Tofu-safety: truncation is
/// char-based (`chars().take()`), so UTF-8 boundaries are preserved.
pub(super) const HUD_GRID_CELL_W: usize = 14;

/// Total panel block height in rows: 11 panel rows + 1 tail-accent row.
///
/// Row map (panel-local):
/// - row 0  : header strip  `╭── fps: …  tgt: … ──╮` (bright)
/// - row 1  : spacer        `│ … blanks … │`
/// - rows 2..=8 : grid rows `│ cell cell cell │` (7 rows × 3 cells)
/// - row 9  : spacer        `│ … blanks … │`
/// - row 10 : footer strip  `╰── 200x50 auto ──╯` (bright)
/// - row 11 : tail accent   `▼` centered below the frame (bright)
///
/// 12 rows anchored bottom-center keeps the `-mb` message box clean
/// on terminals ≥ ~32 rows (research §6: threshold = HUD rows +
/// message-box height ≈ 12 + 5..9) — better than the research
/// doc's 13-row estimate because the header/footer strips carry
/// metrics instead of dedicating rows to them.
pub(super) const HUD_PANEL_ROWS: u16 = 12;

/// Visual slot → metric-index map. The metrics keep their historical
/// `cached_lines` indices (0..=23, v80.0.0-beta.1 + Z-master-1X round 5
/// order — every content test asserts those indices); this table says
/// WHERE each metric sits in the panel:
///
/// - slots 0-1  : header strip (fps, tgt) — the owner's "FPS on top"
/// - slots 2-22 : grid body, 7 rows × 3 cells, zone order per the
///   approved Option B mock (health → identity → controls → dragons →
///   efficiency → performance core → session)
/// - slot 23    : footer strip (screensize) — the "visual anchor at
///   the very bottom" mandate, now literally the closing line.
///
/// THE single source of truth for panel placement: the grid
/// composition (metrics.rs), the color gradient mapping (colors.rs),
/// and the side-border sweep (this file) all read this one table.
pub(super) const HUD_VISUAL_ORDER: [usize; 24] = [
    0, 1, // header strip: fps, tgt
    6, 7, 8, // grid row 1: ehs, prs, scn
    9, 10, 11, // grid row 2: chr, clr, sped
    12, 13, 14, // grid row 3: dsty, prdr, crdr
    15, 16, 17, // grid row 4: ambt, glth, ctun
    18, 19, 20, // grid row 5: mnst, dcel, tcel
    2, 3, 4, // grid row 6: max, p99, cpu
    5, 21, 22, // grid row 7: rss, cid, up
    23, // footer strip: screensize
];

impl super::HudState {
    pub(crate) fn new() -> Self {
        // Compile-time git short SHA injected by build.rs via the
        // `COSMOSTRIX_GIT_SHA` env var (see `git_short_sha()` in
        // build.rs — runs `git rev-parse --short=7 HEAD`). Falls back
        // to the literal "unknown" when the build had no `.git` dir
        // (e.g. a tarball release build) so the HUD never panics on a
        // missing env. The value is a `&'static str`, so embedding it
        // in the cached_lines String is a one-time alloc per session —
        // zero per-frame cost.
        let commit_sha = option_env!("COSMOSTRIX_GIT_SHA").unwrap_or("unknown");
        Self {
            visible: false,
            session_start: Instant::now(),
            // v80.0.0-beta.1 pause-freeze: metrics stop while paused (owner bug fix
            // 2026-08-30). See set_metrics_paused() in metrics.rs.
            metrics_paused: false,
            pause_started_at: None,
            paused_total: Duration::ZERO,
            frame_times: FrameTimeTracker::new(),
            // Z-master-1X round 5: dirty-cell tracker for dcel/tcel metrics.
            dirty_cell_tracker: DirtyCellTracker::new(),
            last_metric_update: Instant::now()
                .checked_sub(HUD_METRIC_INTERVAL)
                .unwrap_or_else(Instant::now),
            last_rss_sample: Instant::now()
                .checked_sub(HUD_RSS_INTERVAL)
                .unwrap_or_else(Instant::now),
            last_rss_kb: None,
            last_cpu_sample: Instant::now()
                .checked_sub(HUD_CPU_INTERVAL)
                .unwrap_or_else(Instant::now),
            last_cpu_ns: None,
            cpu_percent: None,
            max_ms: 0.0,
            max_reset_at: Instant::now(),
            p99_ms: 0.0,
            // v30 (2026-08-05): default target_fps to 60.0 — the same default
            // as Args::fps. The event loop calls set_target_fps() at startup
            // with the resolved value (which may be lower on VSCode, etc.).
            target_fps: 60.0,
            frame_mode: FrameMode::Active,
            screen_size: (0, 0, false),
            // v50 (2026-08-17) HUD expansion — initialize the 7 new metrics
            // to neutral defaults. The event loop calls the setters at
            // startup with the resolved values (from cfg / power_manager /
            // endurance_health) so the HUD shows real data from frame 1.
            scene_name: String::new(),
            color_scheme: ColorScheme::Green,
            custom_palette_name: None,
            charset_preset: String::new(),
            droplet_density: 1.0,
            chars_per_sec: 8.0,
            endurance_health_score: 100.0,
            effective_pressure: 0.0,
            // v50.0.0-beta.6: dragon on/off indicators. Defaults match
            // CloudConfig defaults (power_dragon=true, crystal_dragon=false).
            // The event loop calls the setters every frame with the live
            // cfg values, so live-reload changes are reflected immediately.
            power_dragon_on: true,
            crystal_dragon_on: false,
            aggressive_throttle: false,
            // v50.0.0-beta.7 Option C expansion defaults.
            ambient_on: false,
            glitch_level: crate::config::GlitchLevel::None,
            color_tune_custom: false,
            monolith_size: None,
            cached_lines: [
                // ── Performance core (rows 0-5) — unchanged from v50 ──
                (Color::Cyan, String::new()),    // 0: fps
                (Color::Cyan, String::new()),    // 1: tgt — uses `dim` (tail) at runtime
                (Color::Magenta, String::new()), // 2: max line
                (Color::Yellow, String::new()),  // 3: p99 line
                (Color::Cyan, String::new()),    // 4: cpu line
                (Color::Green, String::new()),   // 5: rss line
                // ── Health / pressure (rows 6-7) ──
                (Color::Yellow, String::new()), // 6: ehs
                (Color::Yellow, String::new()), // 7: prs
                // ── Identity lines (rows 8-10) — v80.0.0-beta.1 reorder: moved up
                //    above the user-adjustable controls ──
                (Color::Cyan, String::new()), // 8: scn
                (Color::Cyan, String::new()), // 9: chr
                (Color::Cyan, String::new()), // 10: clr
                // ── User-adjustable live controls (rows 11-12) ──
                (Color::Magenta, String::new()), // 11: sped
                (Color::Magenta, String::new()), // 12: dsty
                // ── Dragon + tuning state (rows 13-18) ──
                (Color::DarkCyan, String::new()), // 13: prdr
                (Color::DarkCyan, String::new()), // 14: crdr
                (Color::DarkCyan, String::new()), // 15: ambt
                (Color::DarkCyan, String::new()), // 16: glth
                (Color::DarkCyan, String::new()), // 17: ctun
                (Color::DarkCyan, String::new()), // 18: mnst
                // ── Cell efficiency (rows 19-20) — Z-master-1X round 5 ──
                // dcel: dirty cell ratio % (rolling avg over 60 frames).
                (Color::DarkCyan, String::new()), // 19: dcel
                // tcel: total cells in the screen (latest sample).
                (Color::DarkCyan, String::new()), // 20: tcel
                // cid line — commit short SHA, static for the entire process
                // lifetime. Row 21 (Z-master-1X round 5: moved down from row
                // 19 to make room for dcel/tcel above it).
                (Color::DarkCyan, format!(" cid: {commit_sha}")),
                // ── Session footer (rows 22-23) ──
                (Color::DarkCyan, String::new()), // 22: up
                (Color::DarkCyan, String::new()), // 23: screensize
            ],
            // v80.0.0-beta.3 panel composition cache — filled at the 1 Hz
            // metric tick (update_metrics → compose_panel in metrics.rs),
            // rendered every frame below. Empty until the first tick;
            // write_to_frame skips rendering while the header is empty
            // (same one-frame contract the old row cache had).
            panel_header: String::new(),
            panel_footer: String::new(),
            panel_grid: std::array::from_fn(|_| std::array::from_fn(|_| String::new())),
        }
    }

    /// Write the HUD panel into the frame buffer. Called every frame
    /// when visible, BEFORE `term.draw()` so the HUD is part of the
    /// same frame flush — eliminates flicker.
    ///
    /// Uses `frame.set()` (not `set_force`) so unchanged cells aren't
    /// marked dirty — when metrics are stable, only the changing cells
    /// (uptime seconds) get re-sent. The panel footprint is FIXED
    /// (X-1), so the anchor never moves and there is no
    /// shrink/grow residue class at all — the HB-01 clear-on-shrink
    /// machinery of the dynamic-width era is retired with it.
    ///
    /// Anchoring (all saturating — a 80x24 terminal is the floor,
    /// `frame.set` silently clips out-of-bounds cells, INV-8):
    /// - `start_col  = frame.width.saturating_sub(HUD_PANEL_WIDTH) / 2`
    ///   (bottom-CENTER; on narrow terminals the panel clips both
    ///   sides symmetrically).
    /// - `anchor_row = frame.height.saturating_sub(HUD_PANEL_ROWS)`
    ///   (the panel block's top row; the `▼` accent lands on the
    ///   terminal's very last row).
    ///
    /// Render order note: `rain_at()` (and the centered `-mb` message
    /// box inside it) writes FIRST; the HUD writes AFTER — so wherever
    /// the panel overlaps the message box on short terminals, the HUD
    /// wins (INV-5, unchanged from the corner era; the collision
    /// threshold is now ~32 rows instead of ~55 — the Option B
    /// trade-off the owner accepted).
    pub(crate) fn write_to_frame(&mut self, frame: &mut crate::frame::Frame, bg: Option<Color>) {
        if !self.visible {
            return;
        }
        // First-frame guard: the panel text is composed at the 1 Hz
        // metric tick; between toggle-on and that tick the header is
        // still empty (mirrors the old empty-row skip). One frame, at
        // most, before the panel appears.
        if self.panel_header.is_empty() {
            return;
        }
        let start_col = frame.width.saturating_sub(HUD_PANEL_WIDTH) / 2;
        let anchor_row = frame.height.saturating_sub(HUD_PANEL_ROWS);
        // Bright head color — the palette's last stop (the screensize
        // metric's slot-23 color, t=1.0). Used for the header strip,
        // the footer strip, the four rounded corners, and the `▼`
        // tail accent: the panel's "caps + anchor" all share the
        // brightest color, so the eye reads a bright frame closing a
        // gradient body.
        let head = self.cached_lines[23].0;
        let right_col = start_col + HUD_PANEL_WIDTH - 1;

        // ── Row 0: header strip `╭── fps: …  tgt: … ──╮` (bright) ──
        self.set_cell(frame, start_col, anchor_row, '╭', Some(head), bg);
        self.set_cell(frame, right_col, anchor_row, '╮', Some(head), bg);
        for (i, ch) in self.panel_header.chars().enumerate() {
            let x = start_col + 1 + i as u16;
            self.set_cell(frame, x, anchor_row, ch, Some(head), bg);
        }

        // ── Rows 1..=9: spacer + grid body + spacer ──
        for r in 1u16..=9 {
            let row = anchor_row + r;
            // Side borders carry the row's sweep color (dim at the top
            // → bright at the bottom) — the vertical fade story.
            let side = self.panel_side_color(r);
            self.set_cell(frame, start_col, row, '│', Some(side), bg);
            self.set_cell(frame, right_col, row, '│', Some(side), bg);
            if (2..=8).contains(&r) {
                self.write_grid_row(frame, start_col, row, (r - 2) as usize, bg);
            } else {
                // Spacer rows: blank interior (the panel owns its full
                // rect — rain glyphs under it are overwritten).
                for i in 0..HUD_PANEL_INNER {
                    self.set_cell(frame, start_col + 1 + i, row, ' ', None, bg);
                }
            }
        }

        // ── Row 10: footer strip `╰── 200x50 auto ──╯` (bright) ──
        let footer_row = anchor_row + 10;
        self.set_cell(frame, start_col, footer_row, '╰', Some(head), bg);
        self.set_cell(frame, right_col, footer_row, '╯', Some(head), bg);
        for (i, ch) in self.panel_footer.chars().enumerate() {
            let x = start_col + 1 + i as u16;
            self.set_cell(frame, x, footer_row, ch, Some(head), bg);
        }

        // ── Row 11: `▼` tail accent, centered under the frame ──
        // Decorative only (INV-2: ambiguous-width glyph, one cell —
        // tofu on the Basic tier is harmless, it cannot misalign
        // anything because nothing follows it on the row).
        let accent_col = start_col + (HUD_PANEL_WIDTH - 1) / 2;
        self.set_cell(frame, accent_col, anchor_row + 11, '▼', Some(head), bg);
    }

    /// Write one grid row (panel-local rows 2..=8): three metric cells
    /// (each exactly [`HUD_GRID_CELL_W`] columns, padded at compose
    /// time) separated by one-column blank gutters. Each cell renders
    /// in ITS metric's color — the 24-stop chroma gradient sweeps
    /// through the grid body cell-by-cell (slot order from
    /// [`HUD_VISUAL_ORDER`]), mirroring the message border's per-cell
    /// sweep philosophy (BC-02) applied to text cells.
    fn write_grid_row(
        &self,
        frame: &mut crate::frame::Frame,
        start_col: u16,
        row: u16,
        g: usize,
        bg: Option<Color>,
    ) {
        const CELL_STEP: u16 = HUD_GRID_CELL_W as u16 + 1; // 14 + 1 gutter
        for c in 0..3usize {
            let metric = HUD_VISUAL_ORDER[2 + g * 3 + c];
            let cell_color = self.cached_lines[metric].0;
            let cell_x = start_col + 1 + c as u16 * CELL_STEP;
            for (i, ch) in self.panel_grid[g][c].chars().enumerate() {
                self.set_cell(frame, cell_x + i as u16, row, ch, Some(cell_color), bg);
            }
        }
        // The two gutters (interior offsets 14 and 29) are blank cells
        // the panel must own — otherwise rain glyphs sit between the
        // metric cells.
        for gutter in [HUD_GRID_CELL_W as u16, 2 * CELL_STEP - 1] {
            self.set_cell(frame, start_col + 1 + gutter, row, ' ', None, bg);
        }
    }

    /// Side-border sweep color for panel-local rows 1..=9: the color
    /// of the metric occupying that visual height, so the frame's
    /// vertical edges sweep dim→bright in lockstep with the grid
    /// body. This gradient IS the fade mandate successor — the old
    /// L-border blended toward the screen edge because the HUD hugged
    /// the top-left corner; the floating bottom-center panel instead
    /// fades vertically through the palette sweep (owner "semi
    /// black/fade biar elegant" intent preserved, re-expressed).
    fn panel_side_color(&self, r: u16) -> Color {
        match r {
            1 => self.cached_lines[HUD_VISUAL_ORDER[2]].0, // spacer: body dim start
            2..=8 => {
                let g = (r - 2) as usize;
                self.cached_lines[HUD_VISUAL_ORDER[2 + g * 3]].0 // row's first cell
            }
            _ => self.cached_lines[HUD_VISUAL_ORDER[22]].0, // spacer: body bright end
        }
    }

    /// Single-cell write helper — every HUD frame write goes through
    /// here so the `Cell` construction stays uniform (fg carried,
    /// bg passed through, never bold).
    #[inline]
    fn set_cell(
        &self,
        frame: &mut crate::frame::Frame,
        x: u16,
        y: u16,
        ch: char,
        fg: Option<Color>,
        bg: Option<Color>,
    ) {
        let cell = crate::cell::Cell {
            ch,
            fg,
            bg,
            bold: false,
        };
        frame.set(x, y, cell);
    }
}
