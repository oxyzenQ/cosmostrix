// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cinematic intro selection — one file per style.
//!
//! `--intro <type>` picks the cinematic studio-logo animation played
//! before the rain engine takes over (`cosmostrix --intro cosmic` /
//! `--intro logo` / `--intro none`, default: logo; also settable via
//! `intro = "logo"` in config.toml — CLI flag wins).
//!
//! | Style    | Animation                                             | Length |
//! |----------|-------------------------------------------------------|--------|
//! | `logo`   | cosmostrix Logo: laser charge → ignition glow → dissolve → rain | ~4.5 s |
//! | `cosmic` | Cosmic Burst: singularity → explosion → morph → rain  | ~5 s   |
//! | `none`   | No intro; skip straight to the rain engine.           | 0 s    |
//!
//! ## One file per style (owner refactor mandate)
//!
//! Each style owns ONE file in this directory — everything about the
//! style lives there: phase timing constants, art, particle spawn/update
//! math, render loop, and unit tests. This file (`mod.rs`) holds only
//! the shared skeleton: the `IntroType` enum (moved here from
//! `config/mod.rs` so the whole subsystem is plug-and-play), the shared
//! particle infrastructure (pool, RNG, lerp, render helpers), and the
//! dispatcher that routes `IntroType` to the correct style module.
//!
//! ## How to add intro style #4 (plug-and-play recipe)
//!
//! 1. Copy the closest existing `<style>.rs` to a new file (e.g.
//!    `eclipse.rs`) and rewrite its phase math + doc comment. Keep the
//!    entry-point shape: `pub(super) fn run_<style>_intro(...) ->
//!    std::io::Result<()>`, polling [`should_skip`] each frame and
//!    bumping frames via [`end_frame`].
//! 2. In this file: add the `mod` declaration, the enum variant (with
//!    `#[value(name = "...")]`), and one arm in the [`run_intro`]
//!    dispatch match.
//! 3. Sweep the value surfaces outside this directory: the
//!    `--intro` help block (`cli/help_detail.rs`), README intro bullet
//!    + CLI reference, `testconf` validation docs, and CHANGELOG.
//!
//! No other style's file needs to change — that isolation is the point
//! of the directory layout.
//!
//! ## Skip policy
//!
//! Only `q` / `Q` (case-insensitive) skips mid-animation; SIGTERM /
//! SIGHUP / SIGQUIT exit via the graceful-shutdown flag; Ctrl+C
//! (SIGINT) is deprecated. All other keys are drained and ignored so
//! stray keypresses cannot cut the cinematic short. On terminals below
//! [`MIN_INTRO_COLS`] × [`MIN_INTRO_LINES`] the intro is skipped with a
//! pre-alt-screen stderr notice (emitted by `interactive::mod`).
//!
//! ## Legacy note
//!
//! This directory replaced the v20 layout (`interactive/intro.rs`
//! dispatcher + `interactive/intro_cosmic.rs` +
//! `interactive/intro_logo/`) in the v52 one-file-per-style refactor —
//! same shape as `src/msg_fill_style/`. The Linux `/proc` metrics
//! helpers that used to squat in `intro.rs` moved home to
//! `sysstat/procstat.rs`.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crossterm::event::Event;
use crossterm::style::Color;

use crate::cell::Cell;
use crate::chroma_dragon_engine::intro_colors::NEON_GREEN_FALLBACK;
use crate::cloud::Cloud;
use crate::frame::Frame;
use crate::palette::color_to_rgb;
use crate::terminal::{is_terminal_gone, Terminal};

// Key-modifier guard + shutdown flags live in `interactive`; re-exported
// at the `interactive` facade so this crate-root module reaches them
// without the whole submodules becoming pub(crate).
use crate::interactive::{is_unmodified_or_shift, FRAME_COUNTER, GRACEFUL_SHUTDOWN};

// ─────────────────────────────────────────────────────────────────────────────
// Intro type enum + dispatcher
// ─────────────────────────────────────────────────────────────────────────────

/// Whether an intro animation reached its natural end. The intro runner
/// (`event_loop_intro.rs`) feeds this into the message-reveal lead logic:
/// on [`IntroOutcome::Completed`] the armed lead stands (the message
/// appears shortly after the intro, the tuned feel); on
/// [`IntroOutcome::CutShort`] the lead is cut so the message reveals
/// immediately — nothing is hiding it anymore (v52 owner bug report:
/// skipping the intro used to leave a dead 6 s wait).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntroOutcome {
    /// All phases played to the natural end.
    Completed,
    /// Cut short or never started: user pressed q, a shutdown signal
    /// arrived, the terminal was below the intro floor, or the intro
    /// type was `None`.
    CutShort,
}

/// Which cinematic intro to play before the rain engine takes over.
/// Exposed as a clap `ValueEnum` for CLI parsing (`--intro`) and
/// consumed by the dispatcher below. Referenced from `config/mod.rs`
/// (the `intro` field), `cli/app.rs`, bench enrichment, and verbose
/// startup output via `crate::intro_style::IntroType`.
///
/// * `Cosmic` — Cosmic Burst: singularity → explosion → morph → rain.
/// * `Logo`   — cosmostrix Logo: laser charge → ignition → dissolve → rain.
/// * `None`   — No intro; skip straight to the rain engine.
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroType {
    #[value(name = "cosmic")]
    Cosmic,
    #[value(name = "logo")]
    Logo,
    #[value(name = "none")]
    None,
}

/// Minimum terminal size for any intro to play. Below this, skip with a
/// stderr notice. v25 responsive: lowered from 80×24 to 10×5 — the
/// intros now dynamically scale their ASCII art to fit the terminal
/// (see logo::scale_art and cosmic::scale_cosmic_art), so the hard floor
/// is only for absurdly tiny terminals where even a scaled-down logo
/// would be unreadable.
pub(crate) const MIN_INTRO_COLS: u16 = 10;
pub(crate) const MIN_INTRO_LINES: u16 = 5;

/// Frame period for all intro animations. ~30 FPS — intros are mostly
/// particle motion, so 30 FPS is smooth without burning CPU.
pub(crate) const INTRO_FRAME_PERIOD: Duration = Duration::from_millis(33);

/// Particle pool capacity. Pre-allocated once; reused via free-list.
/// 512 × 48 B = 24 KiB — negligible. Both Cosmic Burst and Logo intros
/// share this pool size; peak concurrent particle counts stay well below
/// 512 in either intro.
pub(crate) const PARTICLE_POOL_SIZE: usize = 512;

/// Entry point — dispatch to the appropriate intro style module based on
/// `intro_type`. Returns [`IntroOutcome::CutShort`] immediately for `None`
/// or when the terminal is too small, so the caller can cut the
/// message-reveal lead in every did-not-play path.
///
/// # Skip behavior
///
/// The intro can be exited early by pressing `q` (case-insensitive)
/// or by sending SIGTERM / SIGHUP / SIGQUIT (handled via [`GRACEFUL_SHUTDOWN`]).
/// Ctrl+C (SIGINT) is deprecated — only 'q' exits cosmostrix.
/// No other key skips — the intro ignores stray keypresses so accidental
/// presses of space / enter / arrows don't cut the cinematic short.
///
/// # Benchmark mode
///
/// `--benchmark`, `--bench-frames`, and `--bench-all` all `return` from
/// `main.rs` before `interactive::run_interactive()` is ever called, so
/// this function is never reached in benchmark mode. The intro therefore
/// cannot perturb benchmark measurements — no extra CPU, no terminal
/// writes, no particle allocation.
///
/// Below `MIN_INTRO_COLS × MIN_INTRO_LINES`, the intro is skipped with
/// a stderr notice.
///
/// Reuses the existing `Terminal` / `Frame` / `Cell` pipeline. Zero
/// per-frame heap allocation (particle pool is pre-allocated and reused).
pub(crate) fn run_intro(
    term: &mut Terminal,
    frame: &mut Frame,
    cloud: &Cloud,
    w: u16,
    h: u16,
    intro_type: IntroType,
    logo_color: (u8, u8, u8),
) -> std::io::Result<IntroOutcome> {
    if intro_type == IntroType::None {
        return Ok(IntroOutcome::CutShort);
    }

    // Terminal-size guard. Below MIN_INTRO_COLS × MIN_INTRO_LINES the intros
    // clip badly, so skip the cinematic. The user-facing warning for this
    // case is emitted by the caller (event_loop.rs) BEFORE the alternate
    // screen is entered — printing here would leak into the rain matrix
    // (AB-10 rain-screen cleanliness).
    if w < MIN_INTRO_COLS || h < MIN_INTRO_LINES {
        return Ok(IntroOutcome::CutShort);
    }

    match intro_type {
        IntroType::Cosmic => cosmic::run_cosmic_intro(term, frame, cloud, w, h, logo_color),
        IntroType::Logo => logo::run_logo_intro(term, frame, cloud, w, h, logo_color),
        IntroType::None => Ok(IntroOutcome::CutShort),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared particle infrastructure (used by cosmic.rs and logo.rs)
// ─────────────────────────────────────────────────────────────────────────────

/// Parsed particle representation. 48 bytes — fits ~1.3 per cache line.
/// `active` is the free-list flag; dead particles are skipped during
/// update and render.
///
/// `angle` and `speed` are the polar-coordinate form of velocity. We
/// store them alongside `vx`/`vy` because:
/// * Cosmic Burst Phase 2 (burst) needs `angle` for spiral motion (angle += spiral_rate).
/// * Cosmic Burst Phase 3 (morph) needs `speed` for deceleration.
/// * Logo dissolve phase uses `vx`/`vy` directly for rain-fall motion.
/// * `vx`/`vy` are kept as the cartesian cache for rendering.
#[derive(Clone, Copy)]
pub(crate) struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub ch: char,
    /// Particle color stored as RGB triple. Avoids `Color` enum tag overhead
    /// and lets us lerp between cosmic and palette colors trivially.
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub life: f32,
    pub max_life: f32,
    /// Current direction in radians (0 = right, π/2 = down). Updated each
    /// frame by `spiral_rate` during Cosmic Burst Phase 2.
    pub angle: f32,
    /// Current speed in cells per second. Decelerates during Cosmic Burst
    /// Phase 3 morph. For Logo dissolve, holds the rain-fall speed.
    pub speed: f32,
    /// Per-particle angular velocity (radians per second). Sampled at
    /// spawn time from `[SPIRAL_RATE_MIN, SPIRAL_RATE_MAX)` by Cosmic Burst.
    /// Unused (zero) by Logo dissolve particles.
    pub spiral_rate: f32,
    pub active: bool,
}

impl Particle {
    pub(crate) const INACTIVE: Self = Self {
        x: 0.0,
        y: 0.0,
        vx: 0.0,
        vy: 0.0,
        ch: ' ',
        r: 0,
        g: 0,
        b: 0,
        life: 0.0,
        max_life: 0.0,
        angle: 0.0,
        speed: 0.0,
        spiral_rate: 0.0,
        active: false,
    };
}

/// Tiny xorshift32 RNG — avoids pulling `rand` into this module. Seeded
/// from `Instant::now()` so each intro run looks slightly different.
pub(crate) struct XorShift(pub u32);

impl XorShift {
    pub(crate) fn new(seed: u32) -> Self {
        // Avoid the all-zero state which would lock the generator.
        Self(if seed == 0 { 0xDEAD_BEEF } else { seed })
    }
    #[inline]
    pub(crate) fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Uniform float in `[0.0, 1.0)`.
    #[inline]
    pub(crate) fn next_f32(&mut self) -> f32 {
        // 24-bit mantissa for uniform distribution.
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Pre-allocated particle pool with a free-list stack. The pool itself
/// stores `Particle` values; the free-list stores indices into the pool
/// so spawning is O(1) pop, killing is O(1) flag flip.
pub(crate) struct ParticlePool {
    pub particles: Vec<Particle>,
    pub free: Vec<usize>,
}

impl ParticlePool {
    pub(crate) fn new() -> Self {
        let particles = vec![Particle::INACTIVE; PARTICLE_POOL_SIZE];
        let free = (0..PARTICLE_POOL_SIZE).collect();
        Self { particles, free }
    }

    #[inline]
    pub(crate) fn spawn(&mut self, p: Particle) -> bool {
        if let Some(i) = self.free.pop() {
            self.particles[i] = p;
            true
        } else {
            false
        }
    }

    #[inline]
    pub(crate) fn kill(&mut self, i: usize) {
        self.particles[i].active = false;
        self.free.push(i);
    }

    /// Number of particles currently active in the pool. Test-only —
    /// production rendering uses the free-list length directly when
    /// deciding whether to spawn. Tests use this to assert pool state
    /// after spawn/kill operations.
    #[inline]
    #[cfg(test)]
    pub(crate) fn active_count(&self) -> usize {
        PARTICLE_POOL_SIZE - self.free.len()
    }
}

/// Linear interpolation between two `f32` values.
#[inline]
pub(crate) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear interpolation between two RGB triples.
///
/// Routes through the Chroma Dragon OKLab perceptual blend pipeline
/// (`gradient::oklab_blend_rgb`) so the intro animation's color
/// transitions are perceptually uniform — same as the rain palette
/// gradients. This ensures the cinematic intro dissolves seamlessly
/// into the rain matrix without a visible color shift or muddiness.
///
/// Lightness L is linearly interpolated in OKLab. Chroma (a, b)
/// uses polar interpolation (shortest-arc hue rotation) to keep
/// saturation high through the midpoint — even on opposing-hue
/// transitions like purple → green.
///
/// Replaces the previous `chroma::legacy::blend_toward_rgb` (integer
/// sRGB linear blend) which produced muddy midpoints on
/// opposing-hue transitions.
///
/// Owner mandate: every color-processing site must route through the
/// chroma dragon pipeline (primary), with legacy fallback for non-
/// TrueColor terminals. The intro animation is the first thing the user
/// sees — its colors should be indistinguishable from the rain color
/// that follows, so the cinematic intro dissolves seamlessly into the
/// rain matrix without a visible color shift.
#[inline]
pub(crate) fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    crate::chroma_dragon_engine::gradient::oklab_blend_rgb(a.0, a.1, a.2, b.0, b.1, b.2, t)
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers used by both intro style modules
// ─────────────────────────────────────────────────────────────────────────────

/// Seed an [`XorShift`] RNG from wall-clock nanos. Each intro run gets a
/// different particle pattern, which keeps repeat viewings fresh.
pub(crate) fn seed_rng() -> XorShift {
    let seed = Instant::now()
        .elapsed()
        .as_nanos()
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(0x1234_5678) as u32;
    XorShift::new(seed)
}

/// Pull the brightest palette color (typically the head color) as the
/// rain target. If the palette is empty, fall back to neon green.
pub(crate) fn palette_target_rgb(cloud: &Cloud) -> (u8, u8, u8) {
    cloud
        .palette
        .colors
        .last()
        .copied()
        .map(color_to_rgb)
        .unwrap_or(NEON_GREEN_FALLBACK)
}

/// Rain charset for the morph / dissolve phases. Empty pool → binary
/// fallback (`['0', '1']`). The returned Vec is owned because it's
/// computed once at intro start and then borrowed for the duration.
pub(crate) fn rain_chars(cloud: &Cloud) -> Vec<char> {
    if cloud.char_pool.is_empty() {
        vec!['0', '1']
    } else {
        cloud.char_pool.clone()
    }
}

/// Deterministic pseudo-random float in `[0, 1)` used by glyph-swap paths
/// that don't have an RNG handle threaded through. Uses a per-call linear
/// congruential step seeded by an atomic counter, which is good enough
/// for cosmetic glyph variation.
#[inline]
pub(crate) fn rng_freehand() -> f32 {
    use std::sync::atomic::{AtomicU32, Ordering as AOrdering};
    static STATE: AtomicU32 = AtomicU32::new(0x1357_9BDF);
    let mut s = STATE.load(AOrdering::Relaxed);
    if s == 0 {
        s = 0x2468_ACE0;
    }
    s ^= s << 13;
    s ^= s >> 17;
    s ^= s << 5;
    STATE.store(s, AOrdering::Relaxed);
    (s >> 8) as f32 / (1u32 << 24) as f32
}

/// Returns `true` if a key event should cause the intro to skip.
///
/// Extracted from [`should_skip`] so the skip policy is unit-testable
/// without a real TTY: only `q` / `Q` (case-insensitive quit) skips.
/// Every other key — including modifiers, arrows, function keys, and
/// `Enter` / `Space` — is ignored.
///
/// Ctrl+C (SIGINT) is deprecated — only 'q' exits cosmostrix.
/// SIGTERM/SIGHUP/SIGQUIT are handled separately by the signal handler
/// setting [`GRACEFUL_SHUTDOWN`], so we don't need to match them here.
#[inline]
fn is_skip_key(key_event: &crossterm::event::KeyEvent) -> bool {
    // Modifier allowlist: accept ONLY bare 'q' (KeyModifiers::NONE) or
    // Shift+'Q' (KeyModifiers::SHIFT). Reject ALL other modifier bits:
    // CONTROL, ALT, SUPER, HYPER, META. Uses the canonical guard from
    // input.rs::is_unmodified_or_shift() so the rules stay consistent
    // across all key handling paths (handle_keybinding, HUD toggles, intro).
    //
    // Without this guard, Super+Q / Ctrl+Q / Alt+Q would skip the intro,
    // which is inconsistent with the main event loop where those combos
    // are silently rejected. The user must press bare 'q' (or Shift+Q
    // which produces 'Q' with SHIFT modifier) to skip.
    //
    // Kitty CSI-u note: kitty-protocol terminals report Shift+q as the
    // base codepoint + SHIFT (`Char('q') + SHIFT`), while legacy
    // terminals report the shifted char (`Char('Q') + SHIFT`). The
    // case-insensitive match below already accepts BOTH shapes, so no
    // normalize_shifted_char() call is needed here — the two terminal
    // families behave identically on this path by construction.
    //
    // CapsLock is a keyboard state, NOT a KeyModifiers bit — it changes
    // which Char the terminal reports ('q' → 'Q' when CapsLock is on).
    // crossterm tags that uppercase char with SHIFT, which the allowlist
    // accepts — CapsLock+Q skipping the intro is accepted behavior.
    if !is_unmodified_or_shift(key_event.modifiers) {
        return false;
    }
    if let crossterm::event::KeyCode::Char(c) = key_event.code {
        c == 'q' || c == 'Q'
    } else {
        false
    }
}

/// Drain the terminal event queue non-blocking. Returns `true` if the
/// intro should skip — but only when the user pressed `q` (case-
/// insensitive) or when [`GRACEFUL_SHUTDOWN`] is set (SIGTERM / SIGHUP / SIGQUIT).
///
/// All other key events are drained and ignored. This is deliberate: the
/// intros run for ~4.5–5 s (Logo ~4.5 s, Cosmic Burst ~5 s), and accidental
/// presses of space / enter / arrow keys should not cut them short. The user
/// always has a fast exit via `q`. Ctrl+C (SIGINT) is deprecated.
///
/// # Why not "any key skips"?
///
/// * The intros are short enough that an "any key" skip is a footgun — a
///   stray keypress from a window manager focus change would abort it.
/// * `q` is the canonical "quit" key throughout cosmostrix's interactive
///   mode, so reusing it here keeps the mental model consistent.
/// * SIGTERM / SIGHUP / SIGQUIT remain hard exits for users who can't or
///   won't press `q` (e.g. piped input, scripted kills). Ctrl+C
///   (SIGINT) is deprecated — only 'q' exits cosmostrix.
///
/// # Terminal-gone guard
///
/// When the user force-closes the terminal during the intro, the PTY master
/// disappears. `poll_event(0)` returns `Ok(true)` instantly and forever
/// (POLLHUP makes the fd perpetually "readable"), and `read_event()` returns
/// `Err(EIO)`. The old loop used `if let Ok(...)` for `read_event`, which
/// silently swallowed the EIO — causing the loop to spin at 100% CPU for
/// 20 seconds until the watchdog fired.
///
/// The fix mirrors the main rain loop's drain logic (`event_loop.rs`):
/// detect EIO/EBADF/BrokenPipe from both `poll_event` and `read_event`, and
/// return `Ok(true)` (skip intro) so the normal shutdown path runs. This
/// drops post-SIGHUP CPU burn from 20s to < 1ms during the intro window.
pub(crate) fn should_skip() -> std::io::Result<bool> {
    if GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
        return Ok(true);
    }
    loop {
        match Terminal::poll_event(Duration::from_millis(0)) {
            Ok(false) => return Ok(false),
            Ok(true) => {}
            Err(e) if is_terminal_gone(&e) => return Ok(true),
            Err(e) => return Err(e),
        }
        match Terminal::read_event() {
            Ok(Event::Key(key_event)) => {
                if is_skip_key(&key_event) {
                    return Ok(true);
                }
                // All other keys are drained and ignored — the intro
                // continues playing. See `is_skip_key` for the rationale.
            }
            Ok(_) => {}
            Err(e) if is_terminal_gone(&e) => return Ok(true),
            Err(_) => {}
        }
        // Defensive: re-check the signal flag each iteration so a SIGHUP
        // arriving mid-drain breaks the loop within one iteration rather
        // than spinning until the next poll_event returns false.
        if GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
            return Ok(true);
        }
    }
}

/// Render a single particle cell at `(x, y)` with the given color,
/// interpolating toward black by the inverse life ratio (so particles
/// fade as they age). `bold` controls the cell's bold flag.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_particle_cell(
    frame: &mut Frame,
    w: u16,
    h: u16,
    x: f32,
    y: f32,
    ch: char,
    rgb: (u8, u8, u8),
    bg: Option<Color>,
    life_t: f32,
    bold: bool,
) {
    let xi = x as u16;
    let yi = y as u16;
    if xi < w && yi < h {
        let faded = lerp_rgb((0, 0, 0), rgb, life_t);
        frame.set_force(
            xi,
            yi,
            Cell {
                ch,
                fg: Some(Color::Rgb {
                    r: faded.0,
                    g: faded.1,
                    b: faded.2,
                }),
                bg,
                bold,
            },
        );
    }
}

/// Bump the watchdog frame counter and sleep for one frame period.
/// Used by every intro style module at the end of each frame loop
/// iteration.
pub(crate) fn end_frame(term: &mut Terminal, frame: &mut Frame) -> std::io::Result<()> {
    term.draw(frame)?;
    FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::thread::sleep(INTRO_FRAME_PERIOD);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Style module declarations
// ─────────────────────────────────────────────────────────────────────────────

// `cosmic` is visible outside this directory only for `BURST_CHARS`
// (double-width glyph audit in `tests/width_guard.rs`). `logo` is fully
// encapsulated behind the dispatch above.
pub(crate) mod cosmic;
mod logo;

#[cfg(test)]
#[path = "../../test/intro_style/tests.rs"]
mod tests;
